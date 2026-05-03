---
name: Bevy 0.18 soundtrack.rs example is canonical state-driven BGM crossfade
description: bevy-0.18.1/examples/audio/soundtrack.rs is the official reference implementation for state-driven BGM with linear crossfade — the pattern Druum's audio plugin should mirror
type: reference
---

When researching state-driven music in Bevy 0.18+, the canonical reference is `bevy-0.18.1/examples/audio/soundtrack.rs` (about 150 LOC). It demonstrates:

- A `GameState` resource with `Peaceful` and `Battle` variants (analog of Druum's per-area state).
- A `change_track` system that runs when state changes: tags every existing playing track with `FadeOut`, then spawns a new `(AudioPlayer, FadeIn)` entity for the new state.
- `fade_in` and `fade_out` tick systems that each frame walk a `Query<&mut AudioSink, With<FadeIn or FadeOut>>` and call `sink.set_volume(Volume::SILENT.fade_towards(Volume::Linear(1.0), elapsed/duration))`.
- `Volume::fade_towards(target, factor)` is the linear-interpolation primitive (verified at `bevy_audio-0.18.1/src/volume.rs:240-248`).
- At completion, `fade_in` removes the `FadeIn` component; `fade_out` despawns the entity entirely (auto-cleanup).

**File:** `/Users/nousunio/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bevy-0.18.1/examples/audio/soundtrack.rs`

**Why this matters for Druum:**
- The historic argument for `bevy_kira_audio` was "built-in `bevy_audio` doesn't have crossfade." Bevy 0.18 closed that gap — `Volume::fade_towards` is in stdlib and the example demonstrates the exact pattern needed for Druum's BGM-by-state requirements.
- For Feature #6 (Audio System), the recommendation is to **mirror this example** rather than add `bevy_kira_audio` as a dep. The 30-LOC fade-tick systems are project-owned and easy to debug.
- The example uses `GameState` as a `Resource`, but Druum uses `States` — adapter is the `state_changed::<GameState>` run condition (already in use at `src/plugins/state/mod.rs:59`).

**How to apply:** When asked about Bevy audio crossfades, fades, BGM transitions, or state-driven music — point to this example first. It's HIGH-confidence (extracted on disk), official (Bevy team owned), and current (0.18.1).

Related memory: `reference_bevy_3d_umbrella_pulls_audio.md` (the audio module is already enabled), `reference_bevy_0_18_local_source.md` (how the on-disk verification works).
