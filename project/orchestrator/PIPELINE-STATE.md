# Pipeline State

**Task:** Drive the full pipeline (research → plan) for Feature #6: Audio System (BGM + SFX) from the dungeon crawler roadmap. Add audio support (`bevy_kira_audio` per roadmap, OR built-in `bevy_audio` per research) — typed channels (`Bgm`, `Sfx`, `Ui`, `Ambient`), state-driven BGM crossfades, placeholder royalty-free SFX/BGM assets, ergonomic SFX trigger API for Feature #7+. Pin new dep with `=` after fail-stop crates.io verification (same playbook as Features #3 and #5). PAUSE at plan-approval; parent dispatches implementer manually because `SendMessage` does not actually resume returned agents (confirmed during Features #3, #4, and #5).
**Status:** completed
**Last Completed Step:** 5

## Artifacts

| Step | Description | Artifact                                 |
| ---- | ----------- | ---------------------------------------- |
| 1    | Research    | /Users/nousunio/Repos/Learnings/claude-code/druum/project/research/20260501-235930-feature-6-audio-system.md |
| 2    | Plan        | /Users/nousunio/Repos/Learnings/claude-code/druum/project/plans/20260502-120000-feature-6-audio-system.md |
| 3    | Implement   | /Users/nousunio/Repos/Learnings/claude-code/druum/project/implemented/20260503-010900-feature-6-audio-system.md |
| 4    | Ship        | https://github.com/codeinaire/druum-dungeon-crawler/pull/6 (branch `6-audio-system`, commit `3689244`) |
| 5    | Code Review | /Users/nousunio/Repos/Learnings/claude-code/druum/project/reviews/20260503-120000-feature-6-audio-system.md (verdict: APPROVE, 1 LOW) |

## Implementation Notes (Step 3)

User approved Option A (built-in `bevy_audio`, NOT `bevy_kira_audio`) and silent placeholders on 2026-05-02. Implementer ran the full plan with three minor deviations:

1. **ffmpeg recipe corrected** — `-c:a libvorbis` replaced with `-c:a vorbis -strict -2` (libvorbis absent from homebrew ffmpeg 8.x). Files are valid Vorbis OGG, ~4.7 KB each.
2. **Unused `use bevy::time::Time;` removed** to satisfy `-D warnings`.
3. **`#[cfg(feature = "dev")] init_resource::<ButtonInput<KeyCode>>()` added to test helper** — same trap as Features #2 and #5 (any test using `StatePlugin` under `--features dev` needs this).

Verification: all 6 commands passed with zero warnings. **38 lib + 1 integration tests default / 39 + 1 with `--features dev`**. **Cargo.toml AND Cargo.lock both byte-unchanged** — Option A's promised "zero new deps" delivered.

**Step 1 record-keeping:** `bevy_kira_audio = 0.25.0` (latest as of 2026-05-03) requires `bevy = "^0.18.0"` — Option B was technically viable, but Option A chosen per plan.

**Step 2 verification:** `ffmpeg 8.1` (homebrew) used to generate 10 silent 1-second Vorbis placeholders (5 BGM, 5 SFX), ~50 KB total bundle.

**Manual audible smoke:** 15s `cargo run --features dev` ran without panics. Lewton decoded all 10 silent files cleanly. BGM entity spawned on `OnEnter(TitleScreen)` via `state_changed::<GameState>` → no decode errors, no fade-curve hardware artifacts. F9-cycling not exercised in the automated run; deferred to user smoke testing.

LOC: `src/plugins/audio/{mod.rs (342), bgm.rs (171), sfx.rs (87)}` = 600 total. Above the +360-410 estimate; the extra is doc comments + 6 inline tests.

## Notable cross-feature pattern

This is the third confirmed instance (#2, #5, #6) of the `StatePlugin` + `--features dev` test-helper trap. Any future test that adds `StatePlugin` while compiling under `--features dev` MUST also `init_resource::<ButtonInput<KeyCode>>()` because `StatePlugin::build` registers `cycle_game_state_on_f9` whose parameter validation requires the resource. Captured in `.claude/agent-memory/implementer/feedback_bevy_test_input_setup.md`.

## Research Summary (Step 1)

Research is HIGH-confidence and recommends a **deviation from the roadmap**: use Bevy 0.18's built-in `bevy_audio` (Option A) instead of `bevy_kira_audio` (Option B, the roadmap default).

**Key findings (verified against on-disk Bevy 0.18.1 source):**

1. The `"3d"` feature umbrella in `Cargo.toml:10-21` **already transitively enables `bevy_audio + vorbis`** (verified at `bevy-0.18.1/Cargo.toml:2322-2330, 2363-2366`). `Cargo.lock:4394-4401` confirms `rodio = 0.20.1` and `lewton` are already resolved. **Zero new deps for Option A.**
2. Bevy 0.18 ships `Volume::fade_towards` + an official `examples/audio/soundtrack.rs` example demonstrating state-driven BGM crossfade with `FadeIn`/`FadeOut` marker components. ~30 LOC of fade-tick code we own.
3. The historic argument for kira ("channels and crossfading out of the box") is weaker in 0.18. Druum v1 needs BGM-by-state, fire-and-forget SFX, four logical channels, global mute — all deliverable on the built-in path.
4. **Cargo.lock scope is cleaner with Option A:** zero new entries vs. `bevy_kira_audio + kira + transitive` for Option B.
5. **Plugin coexistence trap (Option B):** `bevy_kira_audio::AudioPlugin` and built-in `bevy_audio::AudioPlugin` have different `type_name`s and would coexist (verified at `bevy_app-0.18.1/src/plugin.rs:83-91`), causing audio device contention. Disabling built-in audio under the `"3d"` umbrella is non-trivial Cargo.toml rework.

**Resolved technical questions:**
- Audio assets work cleanly with `bevy_asset_loader` via a new `AudioAssets` collection (NOT bundled into `DungeonAssets` — keeps loading-failure modes scoped per concern).
- `audio_output_available` run condition gates real playback, so headless CI tests work without sound device.
- SFX trigger via `Message<SfxRequest>` + `MessageReader` consumer system (Bevy 0.18 idiom, sidesteps the EventReader rename trap).
- Placeholder assets: tiny 1-frame silent .ogg files committed as bytes (recipe in research §RQ9). NOT empty/zero-byte — `rodio::Decoder::new` would unwrap-panic on empty bytes.
- Plugin order: `AudioPlugin` after `LoadingPlugin` (audio assets must be loaded first) and after `StatePlugin` (reacts to state transitions). Existing slot in `main.rs:39` (between `UiPlugin` and `SavePlugin`) is fine — no reordering needed.

**Step A verification recipe (NOT a fail-stop):** Planner codified `cargo info bevy_kira_audio` as Step 1 of the plan. Even if kira is unavailable for Bevy 0.18, plan continues with Option A — the fallback is HIGH-confidence native audio. Different from Features #3/#5 because no escalation arm needed.

**LOC estimate:** ~+200 (low end of roadmap's +200-350). Deps Δ: 0 (Option A) or +1 (Option B).

## Plan Summary (Step 2)

**Plan adopts Option A (built-in `bevy_audio`)** with full architectural rationale. Plan structure: Goal, Approach (10 architectural decisions), Critical (12 pitfalls), 9 commit-ordered Steps, Security, Open Questions (2 for user awareness), Implementation Discoveries (template), Verification (15 items), LOC estimate.

**9 commit-ordered steps:**
1. **Step 1:** Run `cargo info bevy_kira_audio` to confirm Option B viability (NOT a fail-stop — plan continues regardless).
2. **Step 2:** Confirm `ffmpeg`/`sox` is available for silent-.ogg generation (escalation arm if neither found).
3. **Step 3:** Generate and commit 10 silent .ogg placeholders (5 BGM + 5 SFX) under `assets/audio/{bgm,sfx}/`.
4. **Step 4:** Add `AudioAssets` `AssetCollection` to `src/plugins/loading/mod.rs` — chained onto existing `LoadingState`.
5. **Step 5:** Fill `src/plugins/audio/mod.rs` — channel marker components, `ChannelVolumes` resource, `AudioPlugin` skeleton.
6. **Step 6:** Fill `src/plugins/audio/bgm.rs` — `play_bgm_for_state` (state-change handler), `FadeIn`/`FadeOut` components, fade-tick systems.
7. **Step 7:** Fill `src/plugins/audio/sfx.rs` — `Message<SfxRequest>` + `SfxKind` enum + `handle_sfx_requests` consumer.
8. **Step 8:** Add fade-tick verification tests (6 audio tests).
9. **Step 9:** Final 6-command verification matrix + Cargo.lock diff scope check (must be ZERO) + manual audible smoke test.

**Architectural decisions baked in:**
- Module split: `src/plugins/audio/{mod.rs, bgm.rs, sfx.rs}` (matches roadmap §What This Touches line 359).
- Channels are zero-sized `Component`s (`Bgm`, `Sfx`, `Ui`, `Ambient`); spawn pattern: `commands.spawn((AudioPlayer::new(handle), PlaybackSettings::..., Bgm))`.
- State→BGM mapping: hardcoded `match` in `play_bgm_for_state` (data-driven RON deferred to Feature #25).
- SFX trigger: `Message<SfxRequest>` with global consumer system. **No `play_sfx` helper** — direct `MessageWriter::write` is one line and crystal-clear.
- `AudioAssets` is a NEW collection (NOT bundled into `DungeonAssets`) — keeps loading-failure modes scoped per concern.
- Asset format: `.ogg` (Vorbis) — already supported via `lewton` in Cargo.lock.
- Placeholder content: 1-second silent .ogg files committed as bytes (~50 KB total).
- Crossfade: 1.0s fade-out + 1.0s fade-in via `Volume::fade_towards` linear interpolation.
- Plugin order: existing slot in `main.rs:39` is fine — no main.rs reordering.
- Tests use `MinimalPlugins + AssetPlugin + bevy_audio::AudioPlugin + StatesPlugin + AudioPlugin` chain with stub `AudioAssets`. Tests verify component registration, NOT real playback (`audio_output_available` is false on headless CI).
- No v1 dev hotkeys (no cfg-gating in this feature).

**LOC estimate:** +360-410 source LOC, +10 binary asset files (~30 KB total), +0 deps. High end of roadmap's +200-350 budget; justified by 6-test coverage + comprehensive module-level doc codifying the Option A deviation.

**Cleanest-possible-ship signal:** Cargo.toml Δ = 0, Cargo.lock Δ = 0. If `git diff` shows any change to either after implementation, STOP.

## User Decisions

**Plan-approval checkpoint is now active.** Two genuine residual decisions surfaced for user awareness; both have defensible defaults baked into the plan:

1. **Roadmap deviation: Option A (built-in `bevy_audio`) vs Option B (`bevy_kira_audio`)? — Option A in plan.** The 2026-04-29 roadmap pre-committed to `bevy_kira_audio = "0.25"`. The 2026-05-01 research recommends deviation based on HIGH-confidence on-disk verification. Plan adopts Option A. Same precedent as Feature #3's `moonshine-save` swap. Override path documented in Open Questions §1 if user prefers Option B.

2. **Audible BGM placeholders vs silent? — Silent in plan.** Silent .ogg files are licensing-risk-free, ~50 KB total vs 15-25 MB for audible tracks, and the manual smoke test (Step 9) verifies the fade curve has no clicks/pops at the seam. Override to audible (sox/ffmpeg sine-wave generator at different frequencies per state) is one-line recipe change in Step 3.

Pipeline pauses here per task instructions. Resume from Step 3 (Implement) once the user approves the plan in the parent session.
