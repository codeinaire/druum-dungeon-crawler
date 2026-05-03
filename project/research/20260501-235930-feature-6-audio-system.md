# Feature #6 — Audio System (BGM + SFX) — Research

**Researched:** 2026-05-01
**Domain:** Audio in Bevy 0.18.1 — BGM crossfades, SFX, typed channels, integration with `bevy_asset_loader`, plugin ordering, test patterns. Decision question: `bevy_kira_audio` (roadmap default) vs Bevy's built-in `bevy_audio` (already wired transitively, **HIGH-confidence verified on disk**).
**Confidence:** **HIGH** on the built-in `bevy_audio` 0.18.1 fallback path (verified against extracted `bevy_audio-0.18.1/` source plus the official `examples/audio/soundtrack.rs` example which IS a state-driven crossfade). **MEDIUM** on `bevy_kira_audio` specifics (training data only — crate is not extracted on disk and tooling has no live registry/web access). The two are pinned to a single fail-stop gate (RQ1) that can flip the recommendation if `bevy_kira_audio` 0.18 compatibility is unverifiable.

---

## Tooling Limitation Disclosure (read this first)

This research session ran with only `Read`, `Write`, `Grep`, `Glob`, `Edit`. **No Bash, no MCP servers (despite the `context7` system reminder), no WebFetch, no WebSearch.** Same constraint as Features #3, #4, #5 research sessions.

**HIGH-confidence sources (verified directly on disk):**

- `bevy_audio-0.18.1/src/{lib.rs, audio.rs, audio_source.rs, audio_output.rs, sinks.rs, volume.rs}` — built-in audio plugin, `AudioPlayer`, `AudioSink`, `Volume`, `GlobalVolume`, `PlaybackSettings`, `audio_output_available` run condition
- `bevy-0.18.1/Cargo.toml` lines 2322-2330, 2363-2366 — feature umbrella `"3d"` transitively pulls in `audio` (which is `bevy_audio + vorbis`)
- `bevy-0.18.1/examples/audio/{soundtrack.rs, audio.rs, audio_control.rs}` — official Bevy 0.18 examples; `soundtrack.rs` is a working state-driven BGM crossfade
- `bevy_internal-0.18.1/src/default_plugins.rs:74-75` — `DefaultPlugins` registers `bevy_audio::AudioPlugin` when feature `bevy_audio` is on
- `bevy_app-0.18.1/src/plugin.rs:7-92` — Plugin uniqueness is by `type_name()`; different-typed plugins coexist freely
- `Cargo.lock:4394-4401` — current build resolves `rodio = 0.20.1` with `lewton` (vorbis decoder) — confirms .ogg playback already wired
- `src/plugins/audio/mod.rs`, `src/plugins/loading/mod.rs`, `src/plugins/state/mod.rs`, `src/main.rs`, `Cargo.toml` — current project state

**MEDIUM-confidence sources (training data only — could not verify this session):**

- `bevy_kira_audio` published versions on crates.io, exact Bevy 0.18 compat — **NOT extracted on disk** (verified by `Glob /Users/nousunio/.cargo/registry/**/bevy_kira_audio*` returning zero hits; `/kira-*` also zero)
- The original Druum research at `research/20260326-01-bevy-first-person-dungeon-crawler-rpg.md:89,1319` cites `bevy_kira_audio 0.25.0 released 2026-01-14 for Bevy 0.18` — that is training-era information, not live verification
- `bevy_kira_audio`'s public API surface (typed `AudioChannel<T>`, `play().fade_in()`, `stop_with_fadeout`, etc.) — described from training data; the Step A recipe in §RQ1 is the verification gate

**Why MEDIUM is acceptable for the kira path:** the recommendation is **bevy_audio fallback first** because we have HIGH-confidence on it AND it already works in the current build (rodio + lewton resolved, vorbis enabled). Even if the planner picks `bevy_kira_audio` after the Step A verification, every claim about its API is recoverable: the implementer's first compile will surface every divergence at the call site.

**What changes the rating to HIGH for kira:** Running the Step A recipe (§RQ1) and pasting the resolved version + `bevy = "..."` requirement. The planner SHOULD do this before deciding kira-vs-built-in, exactly as Features #3 and #5 did for their new deps.

---

## Recommendation Header (for the planner)

Feature #6 has a **structural decision to make in Step 1 that the roadmap pre-commits to but is not actually obvious**: kira vs the built-in audio. The roadmap and the original research recommend `bevy_kira_audio 0.25.x` for "channels and crossfading out of the box." That recommendation was written 2026-03-26 — before our `Cargo.toml` was finalized with `default-features = false, features = ["3d", ...]`. The "3d" umbrella **already pulls in `bevy_audio` and `vorbis`** (verified at `bevy-0.18.1/Cargo.toml:2322-2330, 2363-2366`), so **Bevy's built-in audio is already running** in our current build. And Bevy 0.18 added `Volume::fade_towards` as a stdlib helper (verified at `bevy_audio-0.18.1/src/volume.rs:240-248`) plus an official `examples/audio/soundtrack.rs` example that demonstrates a state-driven crossfade with manual fade-in / fade-out components. The "channels and crossfading out of the box" argument for kira is **weaker in 0.18 than it was in 0.16-0.17.**

**Top-level recommendation: built-in `bevy_audio` (Option A), not `bevy_kira_audio` (Option B).** Rationale:

1. **HIGH-confidence path:** Every API on the built-in path is verified against on-disk source. The kira path requires a live registry check we cannot perform this session.
2. **Zero new dependencies:** kira would add `bevy_kira_audio + kira + (potentially) symphonia` to Cargo.lock. Built-in adds nothing — `rodio` and `lewton` are already resolved.
3. **Crossfade IS supported:** `Volume::fade_towards` (linear interpolation in linear-volume domain) plus a `FadeIn`/`FadeOut` component pattern, exactly as Bevy's official example does. This is ~30 LOC we own.
4. **Channels become marker-component types we own:** `Bgm`, `Sfx`, `Ui`, `Ambient` as zero-sized `Component` types. Per-channel volume = a single `GlobalVolume`-like resource keyed by channel. Per-channel stop = `commands.entity(e).despawn()` on every entity with that marker.
5. **No third-party API churn:** kira/bevy_kira_audio releases lag Bevy by 1-2 weeks per release; every Bevy minor bump is another version-pin verification. Built-in moves with Bevy.
6. **Roadmap calls out the lag risk explicitly:** §6 Cons lists "kira and bevy_kira_audio releases lag Bevy slightly; verify exact version compat" — this is the recommended caveat, and since we already have a working built-in path, the lag-risk dollar buys us less.

**Where kira is worth the swap (defer to Feature #25 audio-polish):**

- High-quality timeline tweens with non-linear curves (kira's `Tween` with `Easing` is genuinely nicer than our manual `f32` interpolation if we get fancy).
- Spatial audio with proper HRTF (built-in `bevy_audio` is "simple left-right stereo panning" — verified in `audio.rs:55-57`).
- Sample-accurate scheduling (e.g. quantized music transitions on bar boundaries — Druum doesn't need this).
- Modular DSP graphs (sends, busses, pitch-shifting at runtime).

None of those are Feature #6 needs. Druum v1 wants: BGM-by-state with crossfade, fire-and-forget SFX, four logical channels, a global mute. The built-in path delivers all of that.

**Six things the planner must NOT skip:**

1. **Run the §RQ1 Step A verification recipe BEFORE locking the architecture choice.** If `bevy_kira_audio` has a published Bevy-0.18-compat release, the planner can choose either path with confidence. If it does NOT, **the recommendation collapses to built-in audio with no escalation step** — that's the strength of the proposed plan vs. Features #3/#5 (kira is not a fail-stop because we have a HIGH-confidence native fallback). Document the decision in the plan.
2. **The roadmap line 358 (`bevy_kira_audio = "0.25"`) is a pre-commitment, not a verified pin.** It is acceptable to deviate to built-in audio and document why. Features #3 and #5 set the precedent for deviating from roadmap pre-commitments after research.
3. **Channels are NOT a built-in `bevy_audio` concept.** With Option A, "channels" become marker components (`Bgm`, `Sfx`, `Ui`, `Ambient`) plus per-channel resources we define. With Option B, channels are kira-typed (`AudioChannel<Bgm>`). The plan must spec the channel type explicitly.
4. **`AudioPlayer` requires non-empty bytes for an .ogg.** Empty/zero-byte placeholder files will panic in `rodio::Decoder::new` (verified via training data on rodio's behaviour; on-disk `bevy_audio-0.18.1/src/audio_source.rs:97-103` shows the unwrap). For placeholders, use a tiny **1-frame silent .ogg** committed as bytes — see §RQ9 for the recipe.
5. **Tests do NOT need real audio playback.** `audio_output_available` (verified at `bevy_audio-0.18.1/src/audio_output.rs:361-363`) is the run condition that gates the playback systems. On a CI/test machine with no audio device, `AudioOutput::default()` warns "No audio device found" and the gate is `false` — tests can register components, query them, and assert their existence/state without actually playing sound. **No special test-mode plugin is needed.**
6. **`AudioSource` plays through `bevy_asset_loader` cleanly.** `bevy_audio::AudioSource` derives `Asset` (verified at `bevy_audio-0.18.1/src/audio_source.rs:7-21`). The `#[asset(path = "audio/bgm/explore.ogg")] pub bgm_explore: Handle<AudioSource>` pattern works on either path (kira's `AudioSource` mirrors the `Asset` trait shape per training data). Add audio handles to **a new `AudioAssets` collection**, not the existing `DungeonAssets` — keeps loading-screen failure modes scoped per concern (a missing .ogg should not block a "DungeonFloor" test).

---

## Summary

Feature #6 adds music (BGM) and sound effects (SFX) to Druum via four logical "channels" (`Bgm`, `Sfx`, `Ui`, `Ambient`), with a state-driven crossfade on `OnEnter(GameState::Town)` and `OnEnter(GameState::Dungeon)`. The roadmap recommended `bevy_kira_audio` because that's been the de-facto Bevy audio choice for 2 years. **In Bevy 0.18, the built-in `bevy_audio` closes most of the gap that historically justified kira**: `Volume::fade_towards` lands a linear-interpolation primitive in stdlib, the official `soundtrack.rs` example demonstrates exactly the state-driven crossfade pattern Druum needs, and our current Cargo.toml already transitively enables `bevy_audio + vorbis` via the `"3d"` feature umbrella.

The **HIGH-confidence on-disk verification** of the built-in path makes it the primary recommendation. The kira path stays viable as Option B, gated behind a verification recipe (§RQ1 Step A) that the planner can run in 30 seconds. **There is no fail-stop in this feature** — unlike Features #3 and #5, where the new dep was load-bearing, here the native fallback is genuinely production-quality for our use case.

The implementation breakdown is approximately:

- **+30 LOC** for the four channel marker components and a `ChannelVolumes` resource
- **+50 LOC** for `play_bgm_for_state` (state-change handler) plus `FadeIn` / `FadeOut` components and their tick systems (mirrors `examples/audio/soundtrack.rs:99-132`)
- **+30 LOC** for SFX: an `SfxRequest` Bevy 0.18 `Message` plus a consumer system that spawns `AudioPlayer` with the `Sfx` marker
- **+20 LOC** for a thin `play_sfx(commands, handle, kind)` helper re-export so downstream features (Feature #7 footsteps, Feature #15 attack hits) write one line not five
- **+30 LOC** for `AudioAssets` collection (a new `AssetCollection` derive struct) + integration with `LoadingPlugin`
- **+30 LOC** for tests covering: plugin registers all four channels, BGM-change system spawns expected entity on state transition, SFX message-driven SFX spawn
- **+10 LOC** for module split: `mod.rs`, `bgm.rs`, `sfx.rs` (matches the original research §Project Structure)
- **+5-8 placeholder audio assets** as committed bytes — see §RQ9 for the synthesis recipe.

That's ~+200 LOC, low end of the roadmap's +200-350 estimate. Roadmap deps Δ was +1; this plan's deps Δ is **0** (Option A) or +1 (Option B if kira chosen post-verification).

**Primary recommendation:** Implement `AudioPlugin` at `src/plugins/audio/mod.rs` with submodules `bgm.rs` and `sfx.rs`. Use built-in `bevy_audio` via Option A. Define four marker components (`Bgm`, `Sfx`, `Ui`, `Ambient`) as zero-sized `Component`s. State-driven BGM is a system on `state_changed::<GameState>` that spawns a new `AudioPlayer` with `(Bgm, FadeIn)` and adds `FadeOut` to all current `(Bgm, AudioSink)` entities. SFX is a `Message` (`SfxRequest`), with a consumer system on `MessageReader<SfxRequest>` that spawns `AudioPlayer` with `(Sfx, PlaybackSettings::DESPAWN)`. Channel volumes are a `Resource<ChannelVolumes>` (4× `Volume`), updated each frame against query-by-marker. Audio assets are a new `#[derive(AssetCollection, Resource)] pub struct AudioAssets` added to `LoadingPlugin`'s `load_collection`. Placeholder assets are tiny silent .ogg files committed as bytes (recipe in §RQ9). Tests use `MinimalPlugins + AssetPlugin + bevy_audio::AudioPlugin + StatesPlugin + AudioPlugin` — `audio_output_available` gates real playback, so the tests work on headless CI.

---

## Standard Stack

### Core (Option A — recommended)

| Library | Version | Purpose | License | Maintained? | Why Standard |
| ------- | ------- | ------- | ------- | ----------- | ------------ |
| `bevy_audio` (re-exported via `bevy::audio` and `bevy::prelude`) | 0.18.1 (transitively pulled by `bevy/3d` umbrella; verified at `bevy-0.18.1/Cargo.toml:2322-2330, 2363-2366`) | `AudioPlayer`, `AudioSink`, `AudioSource`, `Volume`, `GlobalVolume`, `PlaybackSettings`. Component-driven 0.18 spawn model. | MIT/Apache-2.0 | Yes — co-released with Bevy core | Built-in. The 0.18 native primitives close the historic kira-vs-builtin gap for BGM/SFX use cases. |
| `bevy_asset` (already in use via Feature #3) | 0.18.1 | `AssetServer`, `Handle<AudioSource>`. The `Asset` derive on `AudioSource`. | MIT/Apache-2.0 | Yes | Built-in. |
| `bevy_asset_loader` | `=0.26.0` (already pinned in `Cargo.toml:23`; no upgrade needed) | `AssetCollection` derive. Adding `AudioAssets` is purely additive. | MIT/Apache-2.0 | Active | Already in use; reuse. |
| `rodio` | `0.20.1` (already in `Cargo.lock:4394-4401` via `bevy_audio-0.18.1/Cargo.toml:72-74`) | Underlying audio backend. **NOT a direct dep of Druum** — accessed via `bevy::audio`. | MIT/Apache-2.0 | Active | Transitive. |
| `lewton` | (already in `Cargo.lock:4400`, transitively via `rodio`) | Pure-Rust .ogg/Vorbis decoder. Confirms our `vorbis` feature is wired end-to-end. | MIT | Active | Transitive — no action needed. |

### Core (Option B — alternative, requires §RQ1 verification)

| Library | Version | Purpose | License | Maintained? | Why Standard |
| ------- | ------- | ------- | ------- | ----------- | ------------ |
| `bevy_kira_audio` | **MEDIUM-confidence pin: `=0.25.x` (per the original Druum research §Sources line 1319 "v0.25.0 released for Bevy 0.18"; not verified this session — run Step A in §RQ1)** | Typed `AudioChannel<Bgm>`/`AudioChannel<Sfx>`, `audio.play(handle).fade_in(...)`, `audio.stop().fade_out(...)`. Built on `kira` audio engine. | MIT/Apache-2.0 (typical for NiklasEi crates; verify when pinning) | Active — same maintainer as `bevy_common_assets` and `bevy_asset_loader`, tracks Bevy minors | Listed in `research/20260326-01-bevy-first-person-dungeon-crawler-rpg.md:89` §Standard Stack as "Active (0.25.0 released 2026-01-14 for Bevy 0.18)". |
| `kira` | (training-era: `0.10.x`; transitive dep of `bevy_kira_audio`) | The actual audio engine. Sample-accurate scheduling, modular graphs. | Apache-2.0 / MIT (typical) | Active | Would arrive transitively. |

### Supporting

| Library | Version | Purpose | When to Use |
| ------- | ------- | ------- | ----------- |
| `bevy_state` (already used) | 0.18.1 | `state_changed::<GameState>` run condition + `OnEnter(GameState::X)` schedules for BGM-change triggers. | Used in `play_bgm_for_state`. |
| `bevy_ecs::message` (already used) | 0.18.1 | `Messages<T>`, `MessageReader<T>`, `MessageWriter<T>` for `SfxRequest`. (In 0.18, what was `Event` for buffered events is now `Message` — see §RQ10 trap.) | Used in SFX trigger API. |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
| ---------- | --------- | -------- |
| Built-in `bevy_audio` (Option A) | `bevy_kira_audio` (Option B) | Kira gives nicer tween easings, sample-accurate scheduling, real spatial audio. Druum's needs (BGM-by-state with linear crossfade + fire-and-forget SFX) don't justify the +1 dep, +1 version-pin axis, and lag-on-Bevy-bumps cost. **Reassess at Feature #25 audio polish if a Druum-specific kira capability becomes load-bearing.** |
| Marker-component channels | `kira`-typed `AudioChannel<T>` resources | Kira's typed channels are ergonomic — `audio.play(handle).channel::<Bgm>()`. With Option A, "channel" is a marker component on every spawned `AudioPlayer` entity (`commands.spawn((AudioPlayer::new(h), Bgm))`). The query path is `Query<&AudioSink, With<Bgm>>` — same length as kira's `Res<AudioChannel<Bgm>>` for v1's needs. |
| Hardcoded `match` for state→BGM | RON-driven `HashMap<GameState, Handle<AudioSource>>` resource | Roadmap line 379 explicitly suggests "`OnEnter(GameState::Town)` plays town BGM". RON-driven is data-driven and skips a recompile per BGM swap — but it requires an extra .ron file and a load-time validator. v1: hardcoded `match` (5 BGM slots = 5 lines). v2: data-driven if the BGM library grows. See §RQ7. |
| `MessageWriter<SfxRequest>` for SFX | Direct `commands.spawn(AudioPlayer::new(handle))` from anywhere | Direct-spawn is fewer indirections but couples every gameplay system to the audio plugin's component types and the channel-marker discipline. The `SfxRequest`-message indirection lets an SFX implementation switch to kira later without touching gameplay code, and gives one centralized place to apply per-channel volume. See §RQ8. |
| Royalty-free placeholder track | Tiny synthesized silent .ogg | A real CC0 track gives audible verification of fades; a silent .ogg is licensing-risk-free but un-auditory. v1: silent placeholders (committed as bytes; recipe in §RQ9). The implementer or an artist can later swap in real CC0 tracks via `git mv` + content swap. |

**Installation (Option A — recommended):**

```bash
# No new crates needed — bevy_audio is already enabled transitively via "3d".
# Verify the assumption holds:
cd /Users/nousunio/Repos/Learnings/claude-code/druum
grep -E '^(name|version)' Cargo.lock | grep -A1 -E '"(rodio|lewton)"'
# Expected: rodio 0.20.1, lewton 0.x.

# Confirm the umbrella feature wiring by reading lines 2322-2330 and 2363-2366
# of bevy-0.18.1/Cargo.toml:
sed -n '2322,2330p;2363,2366p' \
  /Users/nousunio/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bevy-0.18.1/Cargo.toml
```

**Installation (Option B — requires §RQ1 verification first):**

```bash
# Step A (verification — run BEFORE editing Cargo.toml):
cd /Users/nousunio/Repos/Learnings/claude-code/druum
cargo info bevy_kira_audio
# Read the latest published version. If it's >= 0.21 (training-era number; could be 0.25 per
# the prior research), check its `bevy = "..."` requirement on docs.rs or its Cargo.toml on
# crates.io: e.g. `cargo info bevy_kira_audio --version 0.25.0`. Confirm bevy = "0.18"
# (or compatible like "^0.18", ">=0.18, <0.19", "0.18.0-rc.x").
# If no Bevy-0.18-compat release exists → fall back to Option A. **Do NOT escalate to user**;
# Option A is a HIGH-confidence native path and the recommended choice.

# Step B (lock with `=` after verifying):
cargo add bevy_kira_audio@=<RESOLVED-VERSION> --no-default-features --features ogg
# (See §RQ2 for the feature-flag rationale; minimal viable set is "ogg" only.)

# Step C (post-add — confirm Cargo.lock + the 6 verification commands still pass):
cargo check
cargo check --features dev
cargo clippy --all-targets -- -D warnings
cargo clippy --all-targets --features dev -- -D warnings
cargo test
cargo test --features dev
```

---

## Architecture Options

| Option | Description | Pros | Cons | Best When |
| ------ | ----------- | ---- | ---- | --------- |
| **A. Built-in `bevy_audio`** (recommended) | Use Bevy's native `AudioPlayer` + `AudioSink` components. Channels = marker components. Crossfade = `Volume::fade_towards` ticking on `FadeIn`/`FadeOut` components. SFX = `MessageReader<SfxRequest>` consumer system. Mirrors `bevy-0.18.1/examples/audio/soundtrack.rs`. | HIGH-confidence on-disk-verified APIs; zero new deps; native to Bevy upgrade cadence; tests work on headless CI; LOC-cheap. | Linear interpolation only (no easing curves); spatial audio is "stereo panning" only; no sample-accurate scheduling. | **Druum v1.** All BGM/SFX/UI/Ambient needs in this game. |
| **B. `bevy_kira_audio`** (roadmap default; gated behind §RQ1) | Add `bevy_kira_audio` dep. Use typed `AudioChannel<Bgm>`/`AudioChannel<Sfx>` resources. `audio.play(handle).fade_in(...)` for crossfade. Coexist with built-in `bevy_audio` (different plugin types, no collision per `bevy_app-0.18.1/src/plugin.rs:83-91`). | Eased tween curves; sample-accurate scheduling (future-proof for music quantization); modular DSP if needed; "channels" are typed not marker-components. | +1 dep; lags Bevy minor releases by 1-2 weeks (Cons line 355 in roadmap); two AudioPlugin instances in the build (more attack surface for tests, panics, plugin-ordering surprises); training-era version may be wrong by 2026-05-01. |  Rare-for-this-game cases like timeline-scheduled BGM stings, true spatial audio (HRTF), modular audio busses. **Defer to Feature #25 if not needed earlier.** |
| C. Hand-rolled rodio with no Bevy plugin | Open a rodio sink directly, manage threads manually. | Maximum control. | Defeats Bevy ECS; unmaintainable; no run-condition gating; reinvents the wheel. | **Never.** Listed only for completeness. |

**Recommended:** **Option A** — built-in `bevy_audio`. The 0.18-era ergonomics close the historic gap with kira for our use case, and the HIGH-confidence verification path is a real planning advantage.

### Counterarguments

Why someone might NOT choose Option A:

- **"The roadmap and original research both recommended kira."** — **Response:** the original research is from 2026-03-26, before Bevy 0.18 stabilized `Volume::fade_towards` and before Druum's Cargo.toml settled on `default-features = false, features = ["3d"]` (which transitively pulls vorbis-enabled `bevy_audio`). The roadmap pre-commitment is a starting point, not a constraint — Features #3 and #5 set the precedent for revising roadmap deps after research surfaces a better path. Document the deviation in the plan and move on.
- **"Kira is industry standard for Bevy audio."** — **Response:** kira was the standard because pre-0.18 `bevy_audio` lacked fade primitives and channel-volume idioms. With `Volume::fade_towards` + `GlobalVolume` + the official `examples/audio/soundtrack.rs` pattern, the standard is shifting. The argument is correct historically and getting weaker each Bevy minor.
- **"What if we need to add real spatial audio (HRTF) for FOEs in Feature #22?"** — **Response:** Feature #22 (FOE rendering) is marked 3.5/5 difficulty and "+1 dep" already; **adding kira at Feature #22** is fine and isolated to a single seam (the `Sfx` channel implementation). The marker-component design lets us swap the channel's underlying playback engine without touching gameplay callers. This is a forward-compatible decision.
- **"Manual fade-in/fade-out components are 30 LOC of busywork."** — **Response:** they're 30 LOC of well-typed busywork that we own and can debug. Kira's tweens are more LOC, just hidden in the dep. The official Bevy example proves the pattern is mainstream.
- **"What if `bevy_kira_audio` has a feature we'll need later?"** — **Response:** then we add it later. Feature #25 audio polish is the right place to evaluate kira after we have shipped audio at v1 and know what specifically is missing. Adding kira pre-emptively is a YAGNI violation.

Why someone might NOT choose Option B:

- See the Cons column above. Plus: kira-doesn't-coexist-cleanly-with-bevy_audio is a known sharp edge in older versions — kira's docs traditionally suggest **disabling Bevy's `bevy_audio` feature** (which we cannot do without losing the `"3d"` umbrella semantics, since `audio` is part of `"3d"` per `bevy-0.18.1/Cargo.toml:2322-2330`). **Verify the kira plugin's coexistence story in §RQ6 Step A** before locking Option B.

---

## Architecture Patterns

### Recommended Project Structure (Option A)

```
src/
├── plugins/
│   ├── audio/
│   │   ├── mod.rs        # AudioPlugin, channel marker components, ChannelVolumes resource
│   │   ├── bgm.rs        # state→BGM mapping, FadeIn/FadeOut tick systems
│   │   └── sfx.rs        # SfxRequest message + consumer system + play_sfx helper
│   ├── loading/          # extends DungeonAssets pattern with new AudioAssets collection
│   └── ...
└── ...
assets/
├── audio/
│   ├── bgm/
│   │   ├── town.ogg          # ~50 KB silent placeholder (see §RQ9 recipe)
│   │   ├── dungeon.ogg
│   │   ├── combat.ogg
│   │   ├── title.ogg
│   │   └── gameover.ogg
│   ├── sfx/
│   │   ├── footstep.ogg
│   │   ├── door.ogg
│   │   ├── encounter_sting.ogg
│   │   ├── menu_click.ogg
│   │   └── attack_hit.ogg
│   ├── ambient/              # (deferred to Feature #9 dungeon atmosphere unless v1 ships one)
│   └── ui/                   # (deferred unless distinct UI sounds beyond menu_click)
```

### Pattern 1: Marker-component channels + AudioPlayer spawn

**What:** Treat each "channel" as a unit-struct `Component`. Spawn `AudioPlayer` entities with the channel marker component attached. Per-channel queries are `Query<E, With<ChannelMarker>>`. Per-channel volume control walks the entity list and calls `AudioSink::set_volume`.

**When to use:** Every BGM/SFX/UI/Ambient sound spawn in Druum.

**Example:**

```rust
// Source: src/plugins/audio/mod.rs (proposed; based on bevy-0.18.1/examples/audio/soundtrack.rs)
use bevy::prelude::*;

#[derive(Component)] pub struct Bgm;
#[derive(Component)] pub struct Sfx;
#[derive(Component)] pub struct Ui;
#[derive(Component)] pub struct Ambient;

#[derive(Resource, Default)]
pub struct ChannelVolumes {
    pub bgm: bevy::audio::Volume,
    pub sfx: bevy::audio::Volume,
    pub ui: bevy::audio::Volume,
    pub ambient: bevy::audio::Volume,
}

// Spawn a BGM track:
fn spawn_town_bgm(mut commands: Commands, audio_assets: Res<AudioAssets>) {
    commands.spawn((
        AudioPlayer::new(audio_assets.bgm_town.clone()),
        PlaybackSettings {
            mode: bevy::audio::PlaybackMode::Loop,
            volume: bevy::audio::Volume::SILENT,  // start silent for fade-in
            ..default()
        },
        Bgm,
        FadeIn { duration_secs: 1.0, elapsed_secs: 0.0 },
    ));
}
```

### Pattern 2: State-driven BGM crossfade

**What:** A system gated by `state_changed::<GameState>` that (a) attaches `FadeOut` to every existing `(Bgm, AudioSink)` entity, then (b) spawns a new `(AudioPlayer, Bgm, FadeIn)` for the new state's BGM track.

**When to use:** `OnEnter` for each state that has a distinct BGM. Mirrors `examples/audio/soundtrack.rs:63-94`.

**Example:**

```rust
// Source: bevy-0.18.1/examples/audio/soundtrack.rs:63-94 (verbatim pattern, adapted for Druum's state enum)
fn play_bgm_for_state(
    mut commands: Commands,
    bgm_query: Query<Entity, (With<Bgm>, With<AudioSink>)>,
    audio_assets: Res<AudioAssets>,
    state: Res<State<GameState>>,
) {
    // Fade out everything currently playing on the BGM channel.
    for e in &bgm_query {
        commands.entity(e).insert(FadeOut::default());
    }

    // Pick the next track. Hardcoded match for v1 (§RQ7 — defer RON-driven map to v2).
    let track = match state.get() {
        GameState::Town       => audio_assets.bgm_town.clone(),
        GameState::Dungeon    => audio_assets.bgm_dungeon.clone(),
        GameState::Combat     => audio_assets.bgm_combat.clone(),
        GameState::TitleScreen => audio_assets.bgm_title.clone(),
        GameState::GameOver   => audio_assets.bgm_gameover.clone(),
        GameState::Loading    => return,  // no music while loading
    };

    commands.spawn((
        AudioPlayer::new(track),
        PlaybackSettings {
            mode: bevy::audio::PlaybackMode::Loop,
            volume: bevy::audio::Volume::SILENT,
            ..default()
        },
        Bgm,
        FadeIn::default(),
    ));
}
```

Registration in plugin (the `state_changed::<GameState>` run condition is verified at `bevy_state-0.18.1/src/state.rs` and currently used in `src/plugins/state/mod.rs:59`):

```rust
app.add_systems(Update, play_bgm_for_state.run_if(state_changed::<GameState>));
```

### Pattern 3: Linear-fade tick on FadeIn/FadeOut components

**What:** Two systems (`fade_in_tick`, `fade_out_tick`) iterate over entities with `FadeIn` / `FadeOut`, advance an elapsed counter against `Time::delta`, and call `AudioSink::set_volume(Volume::Linear(...).fade_towards(...))`. When elapsed >= duration, remove the marker (`FadeIn`) or despawn the entity (`FadeOut`).

**When to use:** Every fade transition. Mirrors `examples/audio/soundtrack.rs:99-132`.

**Example:**

```rust
// Source: derived from bevy-0.18.1/examples/audio/soundtrack.rs:99-132
#[derive(Component, Default)]
struct FadeIn { duration_secs: f32, elapsed_secs: f32 }

#[derive(Component, Default)]
struct FadeOut { duration_secs: f32, elapsed_secs: f32 }

fn fade_in_tick(
    mut commands: Commands,
    mut q: Query<(Entity, &mut AudioSink, &mut FadeIn)>,
    time: Res<Time>,
) {
    for (e, mut sink, mut fade) in &mut q {
        fade.elapsed_secs += time.delta_secs();
        let factor = (fade.elapsed_secs / fade.duration_secs).clamp(0.0, 1.0);
        sink.set_volume(bevy::audio::Volume::SILENT.fade_towards(
            bevy::audio::Volume::Linear(1.0),
            factor,
        ));
        if factor >= 1.0 {
            commands.entity(e).remove::<FadeIn>();
        }
    }
}

fn fade_out_tick(
    mut commands: Commands,
    mut q: Query<(Entity, &mut AudioSink, &mut FadeOut)>,
    time: Res<Time>,
) {
    for (e, mut sink, mut fade) in &mut q {
        fade.elapsed_secs += time.delta_secs();
        let factor = (fade.elapsed_secs / fade.duration_secs).clamp(0.0, 1.0);
        sink.set_volume(bevy::audio::Volume::Linear(1.0).fade_towards(
            bevy::audio::Volume::SILENT,
            factor,
        ));
        if factor >= 1.0 {
            commands.entity(e).despawn();
        }
    }
}
```

The `Volume::fade_towards` signature and behaviour are verified at `bevy_audio-0.18.1/src/volume.rs:240-248`; `Time::delta_secs` is from `bevy_time-0.18.1`.

### Pattern 4: SFX via `Message` indirection

**What:** Define `SfxRequest { kind: SfxKind, position: Option<Vec3> }` as a `Message` (Bevy 0.18 buffered-event idiom; see §RQ10). Downstream features call `MessageWriter<SfxRequest>::write(SfxRequest { ... })`. A consumer system in the audio plugin (`MessageReader<SfxRequest>`) maps `SfxKind` → `Handle<AudioSource>` and spawns a `(AudioPlayer, Sfx, PlaybackSettings::DESPAWN)` entity per request.

**When to use:** Every SFX trigger from gameplay code. Footsteps in Feature #7, attack hits in Feature #15, menu clicks anywhere.

**Example:**

```rust
// Source: src/plugins/audio/sfx.rs (proposed)
#[derive(Message, Clone, Copy, Debug)]   // 0.18: Message NOT Event — see §RQ10
pub struct SfxRequest {
    pub kind: SfxKind,
}

#[derive(Clone, Copy, Debug)]
pub enum SfxKind {
    Footstep,
    Door,
    EncounterSting,
    MenuClick,
    AttackHit,
}

fn handle_sfx_requests(
    mut commands: Commands,
    mut reader: MessageReader<SfxRequest>,   // 0.18: MessageReader NOT EventReader
    audio_assets: Res<AudioAssets>,
) {
    for req in reader.read() {
        let handle = match req.kind {
            SfxKind::Footstep        => audio_assets.sfx_footstep.clone(),
            SfxKind::Door            => audio_assets.sfx_door.clone(),
            SfxKind::EncounterSting  => audio_assets.sfx_encounter_sting.clone(),
            SfxKind::MenuClick       => audio_assets.sfx_menu_click.clone(),
            SfxKind::AttackHit       => audio_assets.sfx_attack_hit.clone(),
        };
        commands.spawn((
            AudioPlayer::new(handle),
            PlaybackSettings::DESPAWN,   // verified at bevy_audio-0.18.1/src/audio.rs:106-108
            Sfx,
        ));
    }
}
```

`PlaybackSettings::DESPAWN` is the canonical "play-once-then-despawn-the-entity" idiom (verified at `bevy_audio-0.18.1/src/audio.rs:105-109`). It avoids needing a manual cleanup system for one-shot SFX.

### Anti-Patterns to Avoid

- **Don't spawn audio outside a Bevy system.** All audio interaction must go through `Commands` / `MessageWriter` so it lives inside Bevy's scheduler. (Trivial-sounding but easy to violate when adding "Just play this sound NOW" from a startup hack.)
- **Don't query for `AudioSink` immediately after spawning `AudioPlayer`.** `AudioSink` is added by Bevy's audio output system in PostUpdate AFTER asset load completes. The pattern is: spawn `(AudioPlayer, Bgm)`, then the next frame `(Bgm, AudioSink)` is queryable. This bites tests that try to `app.update()` once and assert sink existence — they need at least 2-3 updates.
- **Don't call `AudioSink::set_volume` on a despawned entity.** `FadeOut::tick` despawns at completion; if any other system holds a stale `Entity`, it'll get a `QueryDoesNotMatch` panic. Use `Query<&mut AudioSink, With<FadeOut>>` not raw entity references.
- **Don't put a `bevy_kira_audio::AudioPlugin` in the plugin tuple expecting it to silently replace built-in `bevy_audio`.** Bevy's plugin uniqueness check is by `type_name()` (verified at `bevy_app-0.18.1/src/plugin.rs:83-91`); two differently-typed audio plugins coexist. Per-platform audio device contention can cause one to fail silently. If choosing Option B, **explicitly disable** built-in `bevy_audio` by removing it from the feature umbrella — but Druum's `"3d"` umbrella pulls it in automatically (line 2363-2366 of `bevy/Cargo.toml`), so Option B requires either dropping the `"3d"` umbrella in favor of finer-grained features (significant Cargo.toml churn) or accepting two coexisting audio backends. **This is the strongest single argument against Option B.**
- **Don't use `EventReader<SfxRequest>`.** It's `MessageReader` in 0.18 — see §RQ10. Same trap as `StateTransitionEvent` in Feature #2 and `AssetEvent` in Feature #3.
- **Don't load an empty .ogg file.** `rodio::Decoder::new` will panic on `UnrecognizedFormat`. Use a real silent-but-valid .ogg (recipe in §RQ9).

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
| ------- | ----------- | ----------- | --- |
| Audio playback engine | A `cpal`/`rodio` thread harness | Built-in `bevy_audio` (Option A) or `bevy_kira_audio` (Option B) | Bevy already wires platform output, ECS integration, asset loading. Hand-rolled audio threads always get spawned/dropped during state transitions in surprising orders. |
| .ogg / Vorbis decoding | Custom decoder | `rodio + lewton` (transitive via Bevy's `vorbis` feature, already enabled) | Decoders are deceptively complex — error tolerance, sample rate conversion, end-of-stream cleanup. |
| Asset-loading-state machinery for audio | Custom poll-loop on `Res<AssetServer>` | `bevy_asset_loader::AssetCollection` — already in use for `DungeonAssets` | Established pattern. Adding `AudioAssets` is one new struct + one line in `LoadingPlugin::build`. |
| Fade-in/fade-out interpolation | Custom `f32` lerp | `Volume::fade_towards` (Option A) or `kira::Tween` (Option B) | Volume is a non-trivial domain (linear vs decibels). Bevy 0.18 added the helper specifically. |

---

## Common Pitfalls

### Pitfall 1: "channel" is not a built-in concept in `bevy_audio`

**What goes wrong:** Engineers familiar with `bevy_kira_audio` reach for `Res<AudioChannel<Bgm>>` and find no such API in `bevy_audio`. They then over-design with `app.init_resource::<AudioChannel<Bgm>>()` boilerplate.

**Why it happens:** Convention from kira-era audio code. Bevy's component-driven 0.18 model treats every audio play as an entity, so the channel concept is a marker.

**How to avoid:** With Option A, "channel = unit struct + `derive(Component)` + spawn alongside `AudioPlayer`." Plus a `ChannelVolumes` resource for the four volume sliders. Document the decision in the audio module's module-level doc so future contributors don't try to introduce a kira-style typed channel resource.

### Pitfall 2: `AudioSink` is not present immediately after `AudioPlayer` is spawned

**What goes wrong:** Test asserts `Query<&AudioSink, With<Bgm>>::single()` succeeds after one `app.update()` — `Err(QuerySingleError::NoEntities)`.

**Why it happens:** Bevy's audio output systems (in `PostUpdate`, gated by `audio_output_available`) check the asset server, and only after the `AudioSource` asset is loaded do they insert the `AudioSink` component. On a freshly-spawned `AudioPlayer` with a `Handle` that hasn't yet finished loading, the sink doesn't exist.

**How to avoid:** In tests, use `Query<Entity, With<Bgm>>` (the marker is added by `commands.spawn`) or run multiple `app.update()` cycles. For production code, design systems to be tolerant — `if let Ok(sink) = q.single() { ... }` not `q.single().unwrap()`.

### Pitfall 3: Two `AudioPlugin` instances coexist invisibly under Option B

**What goes wrong:** With kira added without removing built-in `bevy_audio`, both plugins request the audio output device. On macOS this often means kira gets the device and `bevy_audio` silently fails to play (rodio's `OutputStream::try_default` returns `Err`, the `audio_output_available` run condition flips false, no sound from any built-in `AudioPlayer`). On Linux/PipeWire, the behaviour is more variable.

**Why it happens:** Bevy's plugin uniqueness check is by `type_name`; `bevy_audio::AudioPlugin` and `bevy_kira_audio::AudioPlugin` have different type names, so both register without complaint. They then race for the audio device.

**How to avoid:** If choosing Option B, **disable Bevy's built-in `bevy_audio`** by switching from `features = ["3d", ...]` to a finer-grained feature set that excludes `audio` (which is a sub-component of "3d" per `bevy-0.18.1/Cargo.toml:2322-2330`). This is a bigger Cargo.toml change than Feature #6 should incur. **This is a primary reason Option A is recommended.**

### Pitfall 4: `Event` vs `Message` rename for `SfxRequest`

**What goes wrong:** `EventReader<SfxRequest>` doesn't compile under Bevy 0.18 even though every blog post and pre-0.18 example uses `Event` and `EventReader`.

**Why it happens:** The 0.17→0.18 buffered-event split renamed buffered events to `Message` family. See `.claude/agent-memory/researcher/feedback_bevy_0_18_event_message_split.md`.

**How to avoid:** Define `SfxRequest` with `#[derive(Message)]` and read with `MessageReader<SfxRequest>`. App registration uses `app.add_message::<SfxRequest>()` (or `add_event` may also exist as an alias — verify on first compile).

### Pitfall 5: Hot-reload of audio assets adds latency

**What goes wrong:** With `--features dev` (which sets `bevy/file_watcher` and `AssetPlugin::watch_for_changes_override = Some(true)`), modifying an .ogg file at runtime triggers a re-decode while the game is playing. For a long BGM file this can stutter or briefly drop out.

**Why it happens:** Bevy's hot-reload re-runs the asset pipeline on file change. For audio, the rodio backend doesn't pre-buffer aggressively, so the brief pause is audible.

**How to avoid:** Two options. (a) Document this in the audio module — known dev-only quirk, doesn't affect release. (b) Skip hot-reload for audio specifically — but Bevy 0.18 doesn't expose per-asset-type hot-reload toggles, so this would need extra plumbing. **Recommend Option (a) — accept the dev-mode quirk**, no plumbing.

### Pitfall 6: `PlaybackSettings::Loop` keeps playing while the entity has `FadeOut`

**What goes wrong:** Designer expects "fade out and stop"; the loop playback mode keeps the audio going indefinitely while the volume rolls toward zero. The fade-out tick eventually despawns the entity, which DOES stop the audio — but if the system is misconfigured (e.g. the despawn arm is unreachable due to a `> 1.0` factor mismatch), the loop runs forever at silent volume.

**Why it happens:** `PlaybackMode::Loop` is independent of volume; volume = 0 doesn't pause playback.

**How to avoid:** Always make sure `fade_out_tick` despawns at `factor >= 1.0`. Add a test: spawn-with-fadeout, advance the timer past duration, assert entity is gone. Treat it as the canonical termination guarantee.

### Pitfall 7: Channel volume application is not automatic

**What goes wrong:** Designer changes `ChannelVolumes::sfx` from `1.0` to `0.5` expecting all SFX to soften — nothing happens.

**Why it happens:** `ChannelVolumes` is a Bevy `Resource`; it's data, not a behaviour. Changes don't propagate without a system reading the resource and applying it to query targets.

**How to avoid:** Add a system `apply_channel_volumes` gated by `resource_changed::<ChannelVolumes>`, that walks `Query<&mut AudioSink, With<Bgm>>` (and the other three markers) and sets `sink.set_volume(channel_volumes.bgm * sink.volume())`. **Or:** keep it simple in v1 — channel volumes are the **only** volume the spawned `AudioPlayer` uses (one place to read), and the per-track volume is pinned to 1.0 internally. Document the v1 simplification; revisit at Feature #25.

### Pitfall 8: `audio_output_available` in tests on CI

**What goes wrong:** A test asserts an `AudioSink` exists; it never does because CI has no audio device, `AudioOutput.stream_handle.is_none()`, the run condition is false, and the audio playback systems never run.

**Why it happens:** `audio_output_available` (verified at `bevy_audio-0.18.1/src/audio_output.rs:361-363`) is a deliberate guard so headless environments don't crash.

**How to avoid:** Test the **registration** (entity has `Bgm` marker, `AudioPlayer` component, expected handle) rather than the **playback** (sink exists, plays sound). The marker components are added by `commands.spawn(...)` directly and are observable in tests without audio output.

---

## Security

### Known Vulnerabilities

| Library | CVE / Advisory | Severity | Status | Action |
| ------- | -------------- | -------- | ------ | ------ |
| `bevy_audio` 0.18.1 | None found in extracted source (no `SECURITY.md`, no advisory references) | — | — | Native to Bevy 0.18.1 — patched on Bevy upgrade cadence. |
| `rodio` 0.20.1 | None found this session — verify via `cargo audit` post-implementation | — | Status not verified live | Run `cargo audit` after Cargo.lock changes. |
| `lewton` (.ogg decoder) | Not searched; unverified | — | Unknown | Run `cargo audit` after Cargo.lock changes. Note: it's a transitive dep, not a direct one. |
| `bevy_kira_audio` (Option B only) | None found this session — verify on Step A | — | Unknown | Search `https://github.com/NiklasEi/bevy_kira_audio/security` and `cargo audit` after locking. |
| `kira` (Option B transitive) | None found this session | — | Unknown | Same as above. |

**Action item for the planner:** Add a `cargo audit` step to the verification sequence post-implementation (deferred from Feature #5 review per `project/orchestrator/PIPELINE-STATE.md:25` — "cargo-audit not installed locally").

### Architectural Security Risks

| Risk | Affected Architecture Options | How It Manifests | Secure Pattern | Anti-Pattern to Avoid |
| ---- | ----------------------------- | ---------------- | -------------- | --------------------- |
| Untrusted audio file decoded → decoder bug → crash or memory corruption | Both A and B | If users can drop their own .ogg into `assets/` (e.g. modding), a malformed file could trigger a `lewton` or `kira-decoder` bug. | **For Druum v1:** assets directory is project-controlled; no user upload. Safe. **For modding (later):** validate file size, sniff magic bytes, sandbox the decoder if possible. | Loading arbitrary audio paths from save data or runtime user input. |
| Volume DoS — clipping at extreme values | Both | A bug that sets `Volume::Linear(1e6)` could blast user's speakers or push the audio backend into pathological CPU paths. | Clamp `set_volume` callers to `[0.0, 1.0]` or `[0.0, 2.0]` at most. The `ChannelVolumes` resource should expose a setter that clamps. | Direct `sink.set_volume(arbitrary_user_input)`. |
| Audio resource leaks | Both — Option A more, since you spawn entities | Spawning thousands of `(AudioPlayer, PlaybackSettings::ONCE)` entities without cleanup leaks audio sinks. | Use `PlaybackSettings::DESPAWN` for one-shots (the canonical idiom). Use `FadeOut` then despawn for fading transitions. | Never despawning long-finished `AudioPlayer` entities. |

### Trust Boundaries

For Druum v1, the audio subsystem has only one boundary that matters:

- **Boundary:** the `assets/audio/` directory contents — the bytes loaded into `Handle<AudioSource>`. Validation: file format (Bevy's vorbis decoder validates Vorbis frames; malformed input panics with `UnrecognizedFormat` before reaching the audio output thread). For v1 with project-controlled assets, no further validation needed. If we ever support user-supplied audio (modding, savefile-embedded SFX), revisit.

No untrusted user input flows into the audio system in v1. SFX are gated by the `SfxKind` enum — the gameplay code emits a typed enum variant, not a path or a handle from a save file.

---

## Performance

| Metric | Value / Range | Source | Notes |
| ------ | ------------- | ------ | ----- |
| Audio decode CPU (Vorbis, 44.1kHz stereo) | ~1-2% of one core for one stream | training-era rodio benchmarks (LOW-confidence) | Druum max concurrent streams: 1 BGM + 1 ambient + 1-2 SFX overlap = 3-4 streams = ~5% of one core. Trivial. |
| Audio playback latency (rodio) | typically 50-200ms on macOS CoreAudio | training-era; not verified | "latency" here is from `commands.spawn(AudioPlayer)` to first sample audible. For Druum's needs (BGM doesn't care; SFX should be < 100ms), this is acceptable on the recommended Option A path. |
| Bundle size (Option A) | +5-15 MB for placeholder audio | depends on track length and bitrate (§RQ9) | Silent placeholder .ogg files are tiny (~1 KB each). Real BGM: ~3-5 MB per track at 128 kbps mono. |
| Bundle size delta (Option B) | + binary size of `bevy_kira_audio + kira + (transitive deps)` — training-era ~500 KB-2 MB compiled | Not verified | Listed in roadmap §Cons line 372: "+3-10 MB" assuming kira; under Option A the binary cost is zero (rodio + lewton already in build). |
| Compile time delta (Option A) | ~0s — no new compilation work | Cargo.lock check: rodio + lewton already resolved | Already-compiled deps. |
| Compile time delta (Option B) | "+1-2s clean" per roadmap §Impact Analysis | Roadmap projection | First compile of bevy_kira_audio + kira. |

**No live benchmark data found this session — flag for validation during Feature #25 polish if performance becomes an issue.**

---

## Code Examples

Verified against on-disk Bevy 0.18.1 sources or official Bevy examples.

### Example 1 — Spawn a looping BGM track with fade-in

```rust
// Source: bevy-0.18.1/examples/audio/soundtrack.rs:84-92 (verbatim pattern)
commands.spawn((
    AudioPlayer(track),
    PlaybackSettings {
        mode: bevy::audio::PlaybackMode::Loop,
        volume: Volume::SILENT,
        ..default()
    },
    FadeIn,    // (Bevy example uses a unit-struct marker; we extend with timer fields)
));
```

### Example 2 — One-shot SFX that auto-despawns

```rust
// Source: derived from bevy_audio-0.18.1/src/audio.rs:105-109 (PlaybackSettings::DESPAWN constant)
commands.spawn((
    AudioPlayer::new(audio_assets.sfx_door.clone()),
    PlaybackSettings::DESPAWN,
    Sfx,   // Druum's marker-component channel
));
```

### Example 3 — Linear fade-in tick

```rust
// Source: bevy-0.18.1/examples/audio/soundtrack.rs:99-115 (verbatim)
fn fade_in(
    mut commands: Commands,
    mut audio_sink: Query<(&mut AudioSink, Entity), With<FadeIn>>,
    timer: Res<GameStateTimer>,
) {
    for (mut audio, entity) in audio_sink.iter_mut() {
        audio.set_volume(
            Volume::SILENT.fade_towards(Volume::Linear(1.0), timer.0.elapsed_secs() / FADE_TIME),
        );
        if timer.0.elapsed_secs() >= FADE_TIME {
            audio.set_volume(Volume::Linear(1.0));
            commands.entity(entity).remove::<FadeIn>();
        }
    }
}
```

### Example 4 — `Volume::fade_towards` semantics

```rust
// Source: bevy_audio-0.18.1/src/volume.rs:240-248 (verbatim)
pub fn fade_towards(&self, target: Volume, factor: f32) -> Self {
    let current_linear = self.to_linear();
    let target_linear = target.to_linear();
    let factor_clamped = factor.clamp(0.0, 1.0);

    let interpolated = current_linear + (target_linear - current_linear) * factor_clamped;
    Volume::Linear(interpolated)
}
```

This is **linear interpolation in the linear-volume domain** (equivalent to perceptually slightly-non-natural at very low volumes; for Druum's use case this is fine and matches what the official example uses).

### Example 5 — `bevy_audio::AudioPlugin` plugin signature

```rust
// Source: bevy_audio-0.18.1/src/lib.rs:81-105 (verbatim)
impl Plugin for AudioPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(self.global_volume)
            .insert_resource(DefaultSpatialScale(self.default_spatial_scale))
            .configure_sets(
                PostUpdate,
                AudioPlaybackSystems
                    .run_if(audio_output_available)
                    .after(TransformSystems::Propagate),
            )
            .add_systems(
                PostUpdate,
                (update_emitter_positions, update_listener_positions).in_set(AudioPlaybackSystems),
            )
            .init_resource::<AudioOutput>();

        #[cfg(any(feature = "mp3", feature = "flac", feature = "wav", feature = "vorbis"))]
        {
            app.add_audio_source::<AudioSource>();
            app.init_asset_loader::<AudioLoader>();
        }

        app.add_audio_source::<Pitch>();
    }
}
```

This is **the plugin DefaultPlugins registers for us** when feature `bevy_audio` (transitive via `"3d"`) is on. Confirms: `AudioOutput` resource present, audio playback systems gated by `audio_output_available`, `AudioSource` asset loader auto-registered when `vorbis` is on.

---

## Per-Question Findings

### RQ1 — `bevy_kira_audio` Bevy 0.18 compatibility (HIGH priority)

**Answer (with Tooling Disclosure):** I cannot verify on-disk because `bevy_kira_audio` is not extracted in the local registry (verified by `Glob /Users/nousunio/.cargo/registry/**/bevy_kira_audio*` returning zero hits). Training-era data plus the original Druum research at `research/20260326-01-bevy-first-person-dungeon-crawler-rpg.md:89,1319` cite `bevy_kira_audio 0.25.0 released 2026-01-14 for Bevy 0.18`. **MEDIUM confidence.**

**Step A verification recipe (the planner runs this before locking the architecture choice):**

```bash
cd /Users/nousunio/Repos/Learnings/claude-code/druum
# Get the latest published version of bevy_kira_audio:
cargo info bevy_kira_audio
# Read the latest version (e.g. 0.25.0). Then check its bevy dep:
cargo info bevy_kira_audio --version 0.25.0   # adjust version per first command's output
# Look for: "bevy = ..." in the [dependencies] section. Confirm it allows 0.18.1.
# (e.g. "bevy = 0.18", "bevy = 0.18.0-rc.x", or "^0.18".)

# Sanity dry-run (does NOT modify Cargo.toml):
cargo add bevy_kira_audio --dry-run --no-default-features --features ogg
# This will print the resolved version and feature set without touching the project.
```

**HALT condition:** if NO published version supports `bevy = "0.18"`, **fall back to Option A** — DO NOT escalate to user. Option A is a HIGH-confidence native path and the recommended architecture. Document the kira-not-available finding in the plan as "verified Option A" rather than as a blocker. (This is the major divergence from Features #3/#5: there, the new dep was load-bearing — kira here is not.)

**Different-version fallback:** if `bevy_kira_audio` only supports an older Bevy minor (e.g. 0.17), do NOT downgrade Bevy. Use Option A.

**Why MEDIUM is acceptable:** Step A is a 30-second check; the planner can promote to HIGH before locking the decision.

### RQ2 — Feature flags on `bevy_kira_audio` (HIGH priority, only relevant if Option B chosen)

**Training-era state:** `bevy_kira_audio` typically exposes:
- Format features: `ogg` (default), `mp3`, `flac`, `wav`, `settings_loader` (loads .ron settings files).
- Default features: usually `ogg` only, sometimes `ogg + settings_loader`.

**Recommended minimal viable set for Option B:** `default-features = false, features = ["ogg"]`. Rationale:
- `ogg` is mandatory — Druum's placeholder assets are .ogg.
- `mp3`/`flac`/`wav` are NOT needed — adds binary size for no v1 benefit.
- `settings_loader` adds a runtime-loaded .ron settings file capability — NOT needed v1; defer to Feature #25.

**Verification recipe (Step A continued):**

```bash
cargo info bevy_kira_audio --version <RESOLVED-VERSION>
# Read the [features] section. Confirm: "ogg" exists, "default" includes "ogg" (or you set
# default-features = false and add "ogg" explicitly).
```

**Mutually-exclusive features:** training data does NOT indicate any. Verify in Step A.

**Confidence:** MEDIUM. Verify on Step A.

### RQ3 — Typed audio channels API in `bevy_kira_audio` (HIGH priority, only Option B)

**Training-era API:**

```rust
// Define a channel:
#[derive(Resource)]
struct Bgm;     // unit-struct, used as a type parameter; bevy_kira_audio uses it as marker.

// Register channels in plugin:
app.add_audio_channel::<Bgm>();    // typed registration

// Play on a channel:
fn play_town_bgm(audio: Res<AudioChannel<Bgm>>, asset_server: Res<AssetServer>) {
    audio.play(asset_server.load("audio/bgm/town.ogg")).looped();
}

// Per-channel volume (for future Feature #25):
fn set_bgm_volume(audio: Res<AudioChannel<Bgm>>) {
    audio.set_volume(0.5);
}
```

**The typed channel resource is `AudioChannel<T>`**; `add_audio_channel::<T>()` is the registration call.

**Confidence:** MEDIUM (training-era; verify on first compile under Option B).

### RQ4 — Crossfade / tween API in `bevy_kira_audio` (HIGH priority, only Option B)

**Training-era API:** kira uses `Tween` with a `Duration` and an `Easing` enum. `bevy_kira_audio` exposes:

```rust
// Fade in:
audio.play(handle).fade_in(AudioTween::new(Duration::from_secs(1), AudioEasing::Linear));

// Fade out current:
audio.stop().fade_out(AudioTween::new(Duration::from_secs(1), AudioEasing::Linear));

// (Sometimes there's a `crossfade` helper on the channel; sometimes you do it manually.)
```

The Duration is `core::time::Duration` (verified by analogy with kira's API; not on-disk).

**There is typically NOT a single `crossfade` helper on `AudioChannel`** — the pattern is "stop with fadeout" + "play with fadein" issued in the same frame.

**Confidence:** MEDIUM. Verify on Step A by reading `bevy_kira_audio`'s docs.rs.

### RQ5 — Integration with `bevy_asset_loader` (HIGH priority)

**HIGH-confidence answer:**

`bevy_audio::AudioSource` derives `Asset` (verified at `bevy_audio-0.18.1/src/audio_source.rs:7`):

```rust
#[derive(Asset, Debug, Clone, TypePath)]
pub struct AudioSource { pub bytes: Arc<[u8]> }
```

`bevy_asset_loader`'s `AssetCollection` derive accepts any `Handle<T> where T: Asset`. So:

```rust
#[derive(AssetCollection, Resource)]
pub struct AudioAssets {
    #[asset(path = "audio/bgm/town.ogg")]
    pub bgm_town: Handle<AudioSource>,
    #[asset(path = "audio/bgm/dungeon.ogg")]
    pub bgm_dungeon: Handle<AudioSource>,
    // ... etc
}
```

**Add it to `LoadingPlugin::build` chained onto the existing builder:**

```rust
.add_loading_state(
    LoadingState::new(GameState::Loading)
        .continue_to_state(GameState::TitleScreen)
        .load_collection::<DungeonAssets>()
        .load_collection::<AudioAssets>(),  // <-- Feature #6 adds this
)
```

**Recommendation:** create a NEW `AudioAssets` collection, do NOT add audio to `DungeonAssets`. Reasons:
1. **Concern separation.** A missing .ogg shouldn't block "did I load the dungeon definition correctly?" tests.
2. **Per-feature isolation.** Feature #6 owns `AudioAssets`; Feature #3 owns `DungeonAssets`.
3. **Test minimal-fixture surface.** Future audio-only tests can `load_collection::<AudioAssets>()` without dragging in the dungeon-floor RON.

For Option B (`bevy_kira_audio`), kira's audio asset type is typically `bevy_kira_audio::AudioSource` — different type with the same shape. Same `AssetCollection` pattern works:

```rust
// Option B (kira):
#[asset(path = "audio/bgm/town.ogg")]
pub bgm_town: Handle<bevy_kira_audio::AudioSource>,
```

**Confidence:** HIGH for Option A (verified on-disk). MEDIUM for Option B (training-era API).

### RQ6 — Plugin registration order (HIGH priority)

**HIGH-confidence answer:**

`AudioPlugin` (Druum's local audio plugin) needs:

- **AFTER `DefaultPlugins`** — the built-in `bevy_audio::AudioPlugin` is registered inside `DefaultPlugins` (verified at `bevy_internal-0.18.1/src/default_plugins.rs:74-75`); without `DefaultPlugins` first, `AudioOutput` resource doesn't exist and audio systems panic.
- **Order vs `LoadingPlugin`:** parallel — Druum's audio plugin doesn't need audio assets at `build` time (handles are loaded at runtime via `bevy_asset_loader`). However, the `play_bgm_for_state` system DOES need `AudioAssets` resource present, which only gets inserted on `OnEnter(GameState::TitleScreen)` (after `LoadingState` finishes). System has to be tolerant: `Option<Res<AudioAssets>>` and bail if absent.
- **Order vs `StatePlugin`:** parallel — `StatesPlugin` is in `DefaultPlugins`, comes first; Druum's `StatePlugin` configures sub-states. Druum's `AudioPlugin` reacts to `state_changed::<GameState>`, which works as soon as `init_state::<GameState>()` has run.

**Updated `main.rs` plugin tuple (deviation from current shape — current is fine, the audio module just needs filling in):**

```rust
.add_plugins((
    DefaultPlugins.set(AssetPlugin {
        watch_for_changes_override: Some(cfg!(feature = "dev")),
        ..default()
    }),
    StatePlugin,        // after DefaultPlugins
    ActionsPlugin,
    LoadingPlugin,      // adds AudioAssets to its load_collection chain
    DungeonPlugin,
    CombatPlugin,
    PartyPlugin,
    TownPlugin,
    UiPlugin,
    AudioPlugin,        // *** Feature #6 fills this in; existing slot in main.rs ***
    SavePlugin,
))
```

**For Option B:** `bevy_kira_audio::AudioPlugin` (kira's plugin) registers alongside the built-in `bevy_audio::AudioPlugin` (which is in `DefaultPlugins`). They have different `type_name`s so coexist (verified at `bevy_app-0.18.1/src/plugin.rs:83-91`). This is the **major Option B sharp edge** — see Pitfall 3. Option B requires verifying on Step A whether the kira plugin's docs recommend disabling Bevy's built-in audio.

**Confidence:** HIGH for Option A (verified on-disk). MEDIUM for Option B coexistence behaviour.

### RQ7 — State→BGM mapping mechanism (MEDIUM priority)

**Recommendation:** **Hardcoded `match` for v1.** Defer data-driven RON-based mapping to v2 (after the BGM library has more than 5 entries OR after Feature #25 audio-polish surfaces a need for swap-without-recompile).

**Tradeoff:**

| Approach | Pros | Cons |
| -------- | ---- | ---- |
| Hardcoded `match` (recommended) | 5 lines of code; type-checked; refactor-friendly (rename a state, the `match` is exhaustive). | Recompile to swap a track. |
| RON-driven `Resource<HashMap<GameState, Handle<AudioSource>>>` | Hot-reload swaps tracks without recompile; future-proof for procedural BGM. | Extra .ron file; load-time validation; non-obvious failure mode if a `GameState` variant has no entry. |

**Roadmap line 379** suggests "`OnEnter(GameState::Town)` plays town BGM" — that's the `match`-style hardcoded path. The roadmap implicitly supports v1 simplicity. Going data-driven now is over-engineering.

**Confidence:** HIGH (this is a design call, not an API claim).

### RQ8 — SFX trigger API (MEDIUM priority)

**Recommendation:** **Option A (from the brief): `MessageWriter<SfxRequest>` with a global consumer system.**

**Tradeoff vs alternatives:**

| Option | Pros | Cons |
| ------ | ---- | ---- |
| **A. `MessageWriter<SfxRequest>` + consumer** (recommended) | One indirection; centralizes SFX logic; can swap audio backend later (kira ↔ built-in) without changing gameplay code; easy to mute/throttle/replay SFX in dev tools. | One more type to register; one more system to schedule. |
| B. Direct `commands.spawn(AudioPlayer::new(handle))` from anywhere | Fewer indirections. | Couples gameplay code to audio plugin's internals; bypasses channel-marker discipline; bypasses `ChannelVolumes`; harder to mute/throttle later. |
| C. Helper function `play_sfx(commands, handle, kind)` re-exported | Ergonomic call site (one line). | Still couples — function lives in audio module but takes `Commands` as arg; can be wrapped on top of B or A but not standalone. |

**The recommended pattern combines A and C:** the `Message`-based machinery is the truth of the API; `play_sfx` is a thin re-exported helper that calls `MessageWriter::write`. Like:

```rust
// In src/plugins/audio/sfx.rs:
pub fn play_sfx(mut writer: MessageWriter<SfxRequest>, kind: SfxKind) {
    writer.write(SfxRequest { kind });
}
```

But `play_sfx` as a function isn't a system — it's a helper that downstream systems would call. The clean pattern is:

```rust
// Downstream feature (e.g. Feature #7 movement):
fn handle_movement(mut sfx: MessageWriter<SfxRequest>, ...) {
    if just_moved {
        sfx.write(SfxRequest { kind: SfxKind::Footstep });
    }
}
```

Direct `MessageWriter<SfxRequest>` is one line and crystal clear. **No helper needed.**

**Confidence:** HIGH (design call).

### RQ9 — Asset content strategy (MEDIUM priority — the implementer-can't-download constraint matters)

**Recommendation:** **Ship tiny silent .ogg files committed as bytes.** Smallest LOC, zero licensing risk, zero placeholder content debt.

**The "implementer cannot download or generate audio at runtime" constraint** rules out (a) build-time synthesis (would require a build script + a runtime audio library + unverified Cargo deps) and (c) silent zero-byte files (would panic in `rodio::Decoder::new`).

**The recipe (run ONCE, commit the resulting .ogg files into `assets/audio/`):**

The implementer needs a tool that produces a valid silent .ogg. Two paths:

**Path 1 — `ffmpeg` one-liner (most likely available on the dev machine):**

```bash
# Generate a 1-second silent stereo .ogg (~1-3 KB):
ffmpeg -f lavfi -i anullsrc=r=44100:cl=stereo -t 1 -c:a libvorbis -q:a 0 silent.ogg

# Then duplicate per channel:
mkdir -p assets/audio/bgm assets/audio/sfx
cp silent.ogg assets/audio/bgm/town.ogg
cp silent.ogg assets/audio/bgm/dungeon.ogg
cp silent.ogg assets/audio/bgm/combat.ogg
cp silent.ogg assets/audio/bgm/title.ogg
cp silent.ogg assets/audio/bgm/gameover.ogg
cp silent.ogg assets/audio/sfx/footstep.ogg
cp silent.ogg assets/audio/sfx/door.ogg
cp silent.ogg assets/audio/sfx/encounter_sting.ogg
cp silent.ogg assets/audio/sfx/menu_click.ogg
cp silent.ogg assets/audio/sfx/attack_hit.ogg
rm silent.ogg
git add assets/audio/
git commit -m "Add silent .ogg placeholders for Feature #6"
```

**Path 2 — if `ffmpeg` is not available, use `sox`:**

```bash
sox -n -r 44100 -c 2 silent.ogg trim 0 1
```

**Path 3 — if neither tool is available, embed a hex-dumped 1-second silent .ogg literal in a const byte array and use Bevy's embedded-asset machinery.** This is more work; prefer Paths 1 or 2.

**Why silent placeholder over CC0 royalty-free track:**
- **Audible verification of fades is overrated for v1.** Fade behaviour can be unit-tested directly (assert volume curve in `fade_in_tick` test).
- **CC0 attribution** is a per-file metadata problem — committing five tracks means five licenses to track in a `LICENSES.md`. Silent files have no rightsholder.
- **Asset bundle size.** Five real BGM tracks at 3-5 MB each = 15-25 MB committed; silent placeholders are <50 KB total. Listed in roadmap §Asset Δ.
- **Replacement is easy.** When real BGM lands (Feature #25 polish), `git mv assets/audio/bgm/town_silent.ogg assets/audio/bgm/town.ogg` (no rename needed if same filename).

**Document the placeholder status in `assets/README.md`** and link this research doc.

**Confidence:** HIGH (design call; tooling availability is a planner-verify item).

### RQ10 — Bevy 0.18 Event/Message rename (LOW priority but always check)

**HIGH-confidence answer:**

The `Message` family in Bevy 0.18 covers what was `Event` for buffered events in 0.17 (verified in `feedback_bevy_0_18_event_message_split.md`). For Feature #6 specifically:

- **`SfxRequest`:** define with `#[derive(Message)]`. Read with `MessageReader<SfxRequest>`. Register with `app.add_message::<SfxRequest>()` (or `add_event` if it's a deprecated alias — verify on first compile).
- **`StateTransitionEvent<GameState>`:** **don't read directly.** Use the `state_changed::<GameState>` run condition instead — same pattern as `src/plugins/state/mod.rs:59`. This sidesteps the rename.
- **`AssetEvent<AudioSource>`:** **don't read directly.** `bevy_asset_loader` handles asset-load-completion semantics for us via `LoadingState::continue_to_state`. We never need to poll `AssetEvent`.

**No new traps identified for Feature #6.** The audio module's reactive types are limited to one user-defined `Message` (`SfxRequest`); both BGM-trigger and asset-availability are routed through higher-level Bevy facilities (`state_changed` run condition + `AudioAssets` resource gated by `LoadingState`).

**Confidence:** HIGH (verified against existing project patterns and on-disk source).

### RQ11 — Crate health signals (LOW priority, only Option B)

**Training-era state:**
- `bevy_kira_audio`: maintained by NiklasEi (also maintains `bevy_common_assets` and `bevy_asset_loader`, both already in Druum). Strong release cadence, tracks Bevy minors within 1-2 weeks. License: typically MIT/Apache-2.0.
- `kira` (transitive): maintained by tesselode, active development.

**Notable open issues for Bevy 0.18 use:**
- Historic kira-vs-bevy_audio coexistence quirk (Pitfall 3) is the most reported issue across versions. Resolution generally is "disable `bevy_audio` feature" — but the Druum `"3d"` umbrella makes that harder than usual.
- macOS audio device contention has been reported on multi-stream setups (training era; verify against current `bevy_kira_audio` issue tracker on Step A).

**GitHub URL (training-era, verify):** `https://github.com/NiklasEi/bevy_kira_audio`

**Verification recipe (Step A continued):**

```bash
# Check the GitHub release page or docs.rs index for the latest published version:
# https://crates.io/crates/bevy_kira_audio/versions
# https://docs.rs/bevy_kira_audio/latest/bevy_kira_audio/
# The crate's README usually states "Bevy version compatibility" near the top.
```

**Confidence:** MEDIUM (training-era).

### RQ12 — Test patterns (LOW priority but planner cares)

**HIGH-confidence answer:**

**Pattern 1 — Plugin registers all four channel marker components.** Channels are unit-struct types; the test asserts the plugin doesn't panic at build time:

```rust
#[test]
fn audio_plugin_builds_without_panic() {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default(), bevy_audio::AudioPlugin::default(), AudioPlugin));
    app.update();
    // No assertion needed — successful update means plugin registration succeeded.
}
```

**Pattern 2 — BGM-change system spawns a `(AudioPlayer, Bgm)` entity on state transition.** This works on headless CI because we test the marker component, not the audio sink:

```rust
#[test]
fn state_change_to_town_spawns_bgm_entity() {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        AssetPlugin::default(),
        bevy_audio::AudioPlugin::default(),
        StatesPlugin,
        AudioPlugin,
    ));
    // Insert a stub AudioAssets resource (real one comes from bevy_asset_loader at runtime).
    let stub_handle = Handle::<AudioSource>::default();
    app.insert_resource(AudioAssets {
        bgm_town: stub_handle.clone(),
        bgm_dungeon: stub_handle.clone(),
        bgm_combat: stub_handle.clone(),
        bgm_title: stub_handle.clone(),
        bgm_gameover: stub_handle.clone(),
        sfx_footstep: stub_handle.clone(),
        sfx_door: stub_handle.clone(),
        sfx_encounter_sting: stub_handle.clone(),
        sfx_menu_click: stub_handle.clone(),
        sfx_attack_hit: stub_handle.clone(),
    });
    app.update();  // realise initial state -> Loading

    // Force transition to Town:
    app.world_mut().resource_mut::<NextState<GameState>>().set(GameState::Town);
    app.update();   // StateTransition runs
    app.update();   // play_bgm_for_state runs in Update

    // Assert a BGM entity now exists:
    let count = app.world_mut().query_filtered::<Entity, With<Bgm>>().iter(app.world()).count();
    assert_eq!(count, 1, "expected one Bgm entity to exist after entering Town state");
}
```

This **mirrors the Feature #5 leafwing test pattern** (full plugin chain + `app.update()` to advance state machine). It does NOT require audio output to be available — `audio_output_available` is false on CI, the rodio playback systems skip, but the `Bgm` marker is still added by `commands.spawn`.

**Pattern 3 — SFX message produces a spawn.** Inject a `SfxRequest` message, run the consumer, assert an `(AudioPlayer, Sfx)` entity exists:

```rust
#[test]
fn sfx_request_spawns_sfx_entity() {
    let mut app = App::new();
    // ... same plugin setup as above ...
    app.world_mut().resource_mut::<Messages<SfxRequest>>().write(SfxRequest { kind: SfxKind::Footstep });
    app.update();   // handle_sfx_requests consumes the message
    let count = app.world_mut().query_filtered::<Entity, With<Sfx>>().iter(app.world()).count();
    assert_eq!(count, 1, "expected one Sfx entity per SfxRequest");
}
```

**Why this matters:** the test layer is **Layer 3 from `feedback_bevy_input_test_layers.md`** — the audio plugin's reactive type is `Message<SfxRequest>`, and the message-injection pattern is the same as Feature #5's `KeyboardInput` injection. Plus we use `Res<State<GameState>>::is_changed` style transitions (Layer 1-style for state) — both layers in one test setup.

**Confidence:** HIGH for Pattern 1, HIGH for Pattern 3, MEDIUM for Pattern 2 (the state-transition timing has known one-frame-deferral quirks per `src/plugins/state/mod.rs:130-131` — tests need to `app.update()` twice after `next.set`; verified pattern in existing F9 test).

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
| ------------ | ---------------- | ------------ | ------ |
| `bevy_kira_audio` was de-facto required for fade-friendly audio | Bevy 0.18 ships `Volume::fade_towards` + official soundtrack-with-crossfade example | 2026-03 (Bevy 0.18.0 release) | Closes the historic "kira is required" gap for BGM/SFX use cases. |
| `EventReader<AssetEvent<T>>` for asset-load polling | `LoadingState` from `bevy_asset_loader` (already in use for `DungeonAssets`); event readers replaced by run conditions | 2026-03 (Bevy 0.18 buffered-event split) | Sidesteps the Event-vs-Message rename for asset events. |
| Per-entity `AudioBundle` (Bevy 0.13 style) | `AudioPlayer` Component with `#[require(PlaybackSettings)]` (verified at `bevy_audio-0.18.1/src/audio.rs:248-253`) | 0.18 | Component-driven 0.18 model. Older blog posts using `AudioBundle::new(...)` will not compile. |
| `bevy::audio::Volume::new(0.5)` constructor | `bevy::audio::Volume::Linear(0.5)` enum variant or `Volume::Decibels(-6.0)` | 0.18 | Volume is now an enum, not a struct. |

**Deprecated/outdated:**

- **`AudioBundle`** — does not exist in 0.18. Old tutorials show `commands.spawn(AudioBundle { source: handle, settings: PlaybackSettings::LOOP })`; in 0.18 it's `commands.spawn((AudioPlayer::new(handle), PlaybackSettings::LOOP))`.
- **`Volume::new(f32)`** — replaced by `Volume::Linear(f32)`.
- **Pre-0.18 `bevy_kira_audio` API examples** — the API likely has 0.18-shaped breaks. Verify Step A.

---

## Validation Architecture

### Test Framework

| Property | Value |
| -------- | ----- |
| Framework | Rust stdlib `#[test]` + Bevy `App::update()` integration tests (consistent with existing project) |
| Config file | None — Cargo's default test runner |
| Quick run command | `cargo test --features dev` (5-10s incremental) |
| Full suite command | `cargo test && cargo test --features dev` (30-60s clean) |

### Requirements → Test Map

| Requirement | Behavior | Test Type | Automated Command | File Exists? |
| ----------- | -------- | --------- | ----------------- | ------------ |
| Plugin registers without panic | `AudioPlugin::build` runs fully under `MinimalPlugins + AssetPlugin + bevy_audio::AudioPlugin + AudioPlugin` | unit | `cargo test --test audio_plugin_builds` | NEEDS CREATING (in `src/plugins/audio/mod.rs`) |
| Channel marker components are types | Compile-test that `Bgm`, `Sfx`, `Ui`, `Ambient` are `Component`s | unit (compile-time) | `cargo check` | needs creating |
| BGM-change system spawns Bgm entity on state transition | State transition `Loading -> Town` triggers `play_bgm_for_state` to spawn `(AudioPlayer, Bgm, FadeIn)` | integration (full plugin chain) | `cargo test state_change_to_town_spawns_bgm_entity` | needs creating |
| SFX message produces spawn | `MessageWriter<SfxRequest>::write(SfxKind::Footstep)` causes `(AudioPlayer, Sfx)` entity to exist after one update | integration | `cargo test sfx_request_spawns_sfx_entity` | needs creating |
| ChannelVolumes resource is initialized | Plugin inserts default `ChannelVolumes` (all 1.0) | unit | `cargo test channel_volumes_initialized_at_unit` | needs creating |
| `FadeIn` ticks reach completion | Spawn `(AudioPlayer, FadeIn { duration: 0.1, elapsed: 0.0 })`, advance time by 0.1s, assert `FadeIn` removed | unit (would need time advancement helper) | `cargo test fade_in_completes_at_duration` | needs creating |
| `FadeOut` ticks despawn | Spawn `(AudioPlayer, FadeOut { ... })`, advance time, assert entity gone | unit | `cargo test fade_out_despawns_at_duration` | needs creating |
| `state_changed` run condition fires on every state change | (Verified by virtue of state_change_to_town_spawns_bgm_entity passing — covered transitively) | — | — | covered |

### Gaps (files to create before implementation)

- [ ] `src/plugins/audio/mod.rs` — currently a 9-line stub; needs filling in with `AudioPlugin` impl, channel components, `ChannelVolumes`, plus the test mod.
- [ ] `src/plugins/audio/bgm.rs` — new file. `play_bgm_for_state`, `FadeIn`/`FadeOut` components, tick systems.
- [ ] `src/plugins/audio/sfx.rs` — new file. `SfxRequest` message, `SfxKind` enum, `handle_sfx_requests` consumer.
- [ ] `assets/audio/bgm/{town,dungeon,combat,title,gameover}.ogg` — silent placeholders (5 files).
- [ ] `assets/audio/sfx/{footstep,door,encounter_sting,menu_click,attack_hit}.ogg` — silent placeholders (5 files).
- [ ] `src/plugins/loading/mod.rs` — extend `DungeonAssets` (NO — keep separate per RQ5 recommendation) → ADD `AudioAssets` collection alongside, register additional `RonAssetPlugin` (none — audio uses `AudioSource`'s built-in loader, not RON).

_(No test framework setup needed — Cargo defaults plus existing `MinimalPlugins`/`StatesPlugin` machinery cover Feature #6 tests.)_

---

## Open Questions

1. **Will `bevy_kira_audio` Step A verification (Option B) reveal a published Bevy-0.18-compat version?**
   - What we know: training-era data points to 0.25.0 released 2026-01-14 (per the original research). It is Featured Crate territory under NiklasEi.
   - What's unclear: whether 0.25.0's `bevy = "..."` declaration accepts `=0.18.1` (vs e.g. `0.18.0-rc.x` only).
   - Recommendation: planner runs the Step A recipe in §RQ1 before locking the architecture. **If it returns a valid compat version**, the planner can choose Option A (recommended) or Option B. **If it doesn't**, Option A is the only remaining choice — and that's fine. Document the result either way.

2. **Does the dev machine have `ffmpeg` or `sox` for the silent-.ogg recipe?**
   - What we know: the implementer cannot download or generate audio at runtime — assets must be committed.
   - What's unclear: whether either common tool is installed on the dev machine.
   - Recommendation: planner does `which ffmpeg; which sox` as a Step 1 check. If neither is present, document a Path-3 hex-dumped fallback (the hex bytes for a known-valid 1-second silent .ogg can be embedded in a small Rust source file and written via a `build.rs` once-only, but that adds a build script which is itself a planning concern). **Likely path:** `ffmpeg` is widely available on macOS/Linux dev boxes. Confirm before locking.

3. **Should `ChannelVolumes` be persisted across app sessions?**
   - What we know: Feature #25 (settings UI) will persist user-chosen volumes.
   - What's unclear: whether `ChannelVolumes` persists across runs in v1 or starts at default each launch.
   - Recommendation: defer to Feature #25. v1 should initialize `ChannelVolumes` to `(1.0, 1.0, 1.0, 1.0)` linear at every app start. Document this v1 behaviour in the audio module's module doc.

4. **What happens when `OnEnter(GameState::Loading)` fires after an initial GameOver→Loading cycle (F9)?**
   - What we know: `play_bgm_for_state` returns early on `GameState::Loading` (no music) and fades out the existing BGM.
   - What's unclear: should the BGM cleanly end on the loading-state re-entry?
   - Recommendation: the system fades out everything then returns — that's the desired behaviour. Test: cycle through F9 to GameOver, then to Loading, assert `(Bgm, AudioSink)` count = 0 after fade-out duration.

5. **Are there any `bevy_kira_audio`-specific dev hotkeys that would need cfg-gating under `dev`?**
   - What we know: nothing planned for v1.
   - What's unclear: whether kira's debug overlay or per-channel-mute hotkeys should be wired in dev mode.
   - Recommendation: defer to Feature #25 if Option B is chosen. v1 has no dev audio hotkeys.

---

## Sources

### Primary (HIGH confidence — verified directly on disk)

- [Bevy 0.18.1 source — `bevy_audio` crate](file:///Users/nousunio/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bevy_audio-0.18.1/) — `AudioPlugin`, `AudioPlayer`, `AudioSink`, `AudioSource`, `Volume::fade_towards`, `audio_output_available`, `PlaybackSettings::DESPAWN`. All claims about the built-in audio API are verified by reading `src/{lib.rs, audio.rs, audio_source.rs, audio_output.rs, sinks.rs, volume.rs}`.
- [Bevy 0.18.1 source — `bevy_internal::default_plugins`](file:///Users/nousunio/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bevy_internal-0.18.1/src/default_plugins.rs) — `DefaultPlugins` registers `bevy_audio::AudioPlugin` at line 74-75 when feature `bevy_audio` is on.
- [Bevy 0.18.1 umbrella `Cargo.toml`](file:///Users/nousunio/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bevy-0.18.1/Cargo.toml) lines 2322-2330, 2363-2366 — confirms `"3d"` feature umbrella → `audio` → `bevy_audio + vorbis`.
- [Bevy 0.18.1 official examples — `examples/audio/soundtrack.rs`](file:///Users/nousunio/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bevy-0.18.1/examples/audio/soundtrack.rs) — verbatim pattern for state-driven crossfade. Lines 63-94 (state-change handler), 99-115 (fade-in tick), 119-132 (fade-out tick).
- [Bevy 0.18.1 official examples — `examples/audio/{audio.rs, audio_control.rs}`](file:///Users/nousunio/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bevy-0.18.1/examples/audio/) — basic playback, sink control.
- [Bevy 0.18.1 source — `bevy_app::plugin`](file:///Users/nousunio/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bevy_app-0.18.1/src/plugin.rs) lines 1-92 — `Plugin` trait, uniqueness by `type_name()`, multiple-typed plugins coexist.
- [Druum `Cargo.lock` lines 4394-4401](file:///Users/nousunio/Repos/Learnings/claude-code/druum/Cargo.lock) — confirms `rodio = 0.20.1` with `lewton` (vorbis) already resolved.
- [Druum `src/main.rs`](file:///Users/nousunio/Repos/Learnings/claude-code/druum/src/main.rs) — current plugin registration tuple, `AudioPlugin` slot already exists.
- [Druum `src/plugins/loading/mod.rs`](file:///Users/nousunio/Repos/Learnings/claude-code/druum/src/plugins/loading/mod.rs) — `DungeonAssets` pattern; `AudioAssets` will mirror the structure.
- [Druum `src/plugins/state/mod.rs:59`](file:///Users/nousunio/Repos/Learnings/claude-code/druum/src/plugins/state/mod.rs) — `state_changed::<GameState>` run condition pattern, used by `play_bgm_for_state`.
- [Druum `src/plugins/input/mod.rs:236-298`](file:///Users/nousunio/Repos/Learnings/claude-code/druum/src/plugins/input/mod.rs) — Layer-2 test pattern reference (full plugin chain + message injection).

### Secondary (MEDIUM confidence — training-era; gated by Step A verification)

- [`bevy_kira_audio` on crates.io](https://crates.io/crates/bevy_kira_audio) — published version, Bevy compat. Verify in §RQ1 Step A. Accessed: not this session.
- [`bevy_kira_audio` GitHub by NiklasEi](https://github.com/NiklasEi/bevy_kira_audio) — issue tracker, README, version history. Verify in §RQ1 Step A. Accessed: not this session.
- [`kira` audio engine on crates.io](https://crates.io/crates/kira) — version, dependencies. Accessed: not this session.
- [Original Druum research §Standard Stack line 89](file:///Users/nousunio/Repos/Learnings/claude-code/druum/research/20260326-01-bevy-first-person-dungeon-crawler-rpg.md) — "bevy_kira_audio 0.25.0 released 2026-01-14 for Bevy 0.18". Training-era citation, not live verification.
- [Original Druum research §Sources line 1319](file:///Users/nousunio/Repos/Learnings/claude-code/druum/research/20260326-01-bevy-first-person-dungeon-crawler-rpg.md) — "v0.25.0 for Bevy 0.18. Accessed: 2026-03-26". Two months stale by 2026-05-01.

### Tertiary (LOW confidence — referenced but not used as authoritative)

- Training-era articles on `bevy_kira_audio` typed channels and tween APIs — not cited inline; flagged as the basis for §RQ3 and §RQ4 MEDIUM-confidence claims.

---

## Metadata

**Confidence breakdown:**

- Standard stack (Option A — built-in): **HIGH** — every API verified on-disk in `bevy_audio-0.18.1/`. The "3d" → "audio" → "bevy_audio + vorbis" chain is verified at `bevy-0.18.1/Cargo.toml:2322-2366`. Tests will work on headless CI per `audio_output_available` run-condition logic.
- Standard stack (Option B — kira): **MEDIUM** — based on training data; gated behind §RQ1 Step A. Crate not extracted on disk.
- Architecture options: **HIGH** — three options exhaustively enumerated, recommendation is justified by HIGH-confidence native path verification + a clear forward-compat seam.
- Patterns 1-4: **HIGH** for Patterns 1-3 (verified against `bevy-0.18.1/examples/audio/soundtrack.rs`), **HIGH** for Pattern 4 (Message-based, verified against existing project Message usage in `feedback_bevy_0_18_event_message_split.md`).
- Pitfalls 1-8: **HIGH** for 1-2, 4-8 (verified or tied to verified reference); **MEDIUM** for 3 (Option B coexistence — verifiable on Step A).
- Test patterns: **HIGH** for Pattern 1, 3; **MEDIUM** for Pattern 2 (state-transition deferral may need Layer-3 init_resource pattern from `feedback_bevy_input_test_layers.md` if a `--features dev` test interferes; verify on first compile).
- Asset content strategy (RQ9): **HIGH** for the silent-placeholder design; **MEDIUM** for the `ffmpeg`/`sox` availability assumption (planner verifies on Step 1).

**Research date:** 2026-05-01
