# Review: Feature #6 — Audio System (BGM + SFX)

**PR:** #6 — https://github.com/codeinaire/druum-dungeon-crawler/pull/6
**Verdict:** APPROVE
**Date:** 2026-05-03

## Severity Counts

| Severity | Count |
|----------|-------|
| CRITICAL | 0     |
| HIGH     | 0     |
| MEDIUM   | 0     |
| LOW      | 1     |

## Files Reviewed

Full coverage on all changed source files:
- `src/plugins/audio/mod.rs` (new, ~342 LOC)
- `src/plugins/audio/bgm.rs` (new, ~171 LOC)
- `src/plugins/audio/sfx.rs` (new, ~80 LOC)
- `src/plugins/loading/mod.rs` (modified, +40 LOC)
- `src/main.rs` (verified unchanged)
- `Cargo.toml` / `Cargo.lock` (verified byte-unchanged — hard gate PASSED)
- `assets/audio/{bgm,sfx}/*.ogg` (10 files, verified valid OGG Vorbis, ~4.8 KB each)
- `assets/README.md` (updated audio table)

## Static Analysis

- `cargo check` (default): PASS, zero warnings
- `cargo check --features dev`: PASS, zero warnings
- `cargo clippy --all-targets -- -D warnings`: PASS, zero warnings
- `cargo clippy --all-targets --features dev -- -D warnings`: PASS, zero warnings
- `cargo test` (default): 38 lib + 1 integration = 39 total, all pass
- `cargo test --features dev`: 39 lib + 1 integration = 40 total, all pass
- `git diff Cargo.toml Cargo.lock`: empty (zero new deps confirmed)

## Key Findings

### [LOW] Test comment ordering is misleading in `state_change_to_town_spawns_bgm_entity`

**File:** `src/plugins/audio/mod.rs` (test)

The comments on the two `app.update()` calls after `NextState::set` say:
- Frame 1: "StateTransition runs, GameState becomes Town"
- Frame 2: "play_bgm_for_state runs in Update on the changed frame"

The phrase "on the changed frame" on frame 2 is misleading — it sounds like the system runs _on_ the frame the state changes (frame 1), but it actually means "on the frame that _observes_ the change" (frame 2). The test logic and count are correct; this is purely a documentation concern. Future contributors copying this test pattern may miscount their updates.

Suggested comment wording:
```
app.update(); // StateTransition schedule commits Town; State<GameState> marked changed
app.update(); // Update schedule runs; state_changed::<GameState> is true, play_bgm_for_state fires
```

## Behavioral Delta

The system now:
1. Registers `AudioPlugin` in `main.rs` (was a 9-line empty stub since Feature #1)
2. On every `GameState` transition, fades out current BGM (1s linear) and fades in the new state's BGM (1s linear), with `PlaybackMode::Loop + despawn-on-fade-out` termination
3. Accepts `SfxRequest { kind: SfxKind }` messages (5 variants) and spawns one-shot `(AudioPlayer, PlaybackSettings::DESPAWN, Sfx)` entities per request
4. Loads 10 `.ogg` handles via a new `AudioAssets` collection alongside `DungeonAssets`
5. `ChannelVolumes` resource (API shape for Feature #25) initialized to 1.0 linear on all 4 channels

Zero Cargo.toml / Cargo.lock changes. Bevy's built-in `bevy_audio` + `vorbis` (already transitive via `"3d"` umbrella) used exclusively.

## Verified Checklist

- [x] **Cargo.toml + Cargo.lock byte-unchanged** — hard gate PASSED
- [x] **`AudioPlugin::build`** wires ChannelVolumes resource, `add_message::<SfxRequest>()`, 3 BGM systems in Update, 1 SFX system in Update
- [x] **BGM crossfade** — `play_bgm_for_state` fades out existing Bgm entities then spawns new with FadeIn; tick systems ramp volume via `Volume::fade_towards`; FadeOut despawns (termination guarantee for loop)
- [x] **FadeOut despawn invariant** — `fade_out_tick` despawns at `factor >= 1.0`; looping BGM cannot run forever at silent volume (Pitfall 6)
- [x] **`SfxRequest` derives `Message`** (not `Event`); read with `MessageReader<SfxRequest>`; registered with `add_message::<SfxRequest>()`
- [x] **`SfxKind` has exactly 5 variants**: Footstep, Door, EncounterSting, MenuClick, AttackHit
- [x] **`AudioAssets` separate from `DungeonAssets`** — registered via `.load_collection::<AudioAssets>()` alongside DungeonAssets in LoadingPlugin
- [x] **Tests use marker-component assertions** (Bgm, Sfx, AudioPlayer) not AudioSink — correct for headless CI
- [x] **`audio_output_available` tolerance** — `fade_in_tick` / `fade_out_tick` require `AudioSink` in query; entities without it are skipped (no panic)
- [x] **`play_bgm_for_state` uses `Option<Res<AudioAssets>>`** — tolerant of missing resource, silently skips new spawn if absent
- [x] **State transition deferral in tests** — 2 updates after `NextState::set` before asserting
- [x] **`#[cfg(feature = "dev")] init_resource::<ButtonInput<KeyCode>>()`** in `make_test_app()` — third occurrence of established pattern
- [x] **Silent .ogg placeholders are valid Vorbis** — OggS magic verified, ~4.8 KB each (not zero-byte)
- [x] **No `rand` calls** — permadeath-safe
- [x] **No `bevy_kira_audio`** — confirmed absent from Cargo.toml

## Notable Implementation Deviation (confirmed correct)

`play_bgm_for_state` uses `Query<Entity, With<super::Bgm>>` rather than the plan's `Query<Entity, (With<Bgm>, With<AudioSink>)>`. The deviation is correct and safer: tagging ALL `Bgm` entities with `FadeOut` ensures no entity leaks even if `AudioSink` hasn't been attached yet. The `fade_out_tick` query requires `AudioSink`, so entities without it are safely deferred until the sink appears.

## Manual Smoke (deferred)

`cargo run --features dev` + F9 cycling through all states is executable on real hardware but not covered by automated tests. The plan's smoke test criteria (no decode panics, fade lifecycle visible, no clicks/pops at seam) are not verifiable in this automated review — they remain the pending item on the PR test plan checklist.
