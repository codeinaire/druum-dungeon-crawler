---
name: Bevy 0.18 "3d" feature umbrella transitively enables bevy_audio + vorbis
description: The "3d" Cargo feature in bevy 0.18 includes "audio" (= bevy_audio + vorbis), so Druum's existing Cargo.toml already wires the built-in audio system; verified at bevy-0.18.1/Cargo.toml:2322-2366
type: reference
---

In Bevy 0.18.1's umbrella `Cargo.toml`, the `"3d"` feature flag is NOT just renderer code — it transitively enables the `audio` feature, which expands to `bevy_audio` plus `vorbis`. Verified at `/Users/nousunio/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bevy-0.18.1/Cargo.toml`:

```toml
# Lines 2322-2330:
3d = [
    "default_app",
    "default_platform",
    "3d_bevy_render",
    "ui",
    "scene",
    "audio",       # <-- Surprise: "3d" pulls audio
    "picking",
]

# Lines 2363-2366:
audio = [
    "bevy_audio",  # <-- the bevy_audio internal crate
    "vorbis",      # <-- enables .ogg/Vorbis decoding
]
```

**Druum's `Cargo.toml`** has `bevy = { version = "=0.18.1", default-features = false, features = ["3d", "png", "ktx2", "zstd_rust", "bevy_text"] }` — so Bevy's built-in `bevy_audio::AudioPlugin` is **already registered by `DefaultPlugins`** (verified at `bevy_internal-0.18.1/src/default_plugins.rs:74-75`). The `Cargo.lock` at lines 4394-4401 confirms `rodio = 0.20.1` and `lewton` (vorbis decoder) are already resolved.

**How to apply:** When researching audio, save/load, or any "3d" feature-gated functionality in Druum:
- Don't claim "we need to add bevy_audio" — it's already there.
- Don't claim ".ogg won't decode" — `vorbis` is on, `lewton` is in lock.
- Recognize this is a surprise — many `bevy_kira_audio` migration guides assume you opt into `bevy_audio` separately or remove it. With `"3d"`, you can't remove it without significant feature-set rework (switch to `default_app + default_platform + bevy_render +...`).
- This is also why two coexisting AudioPlugin instances (built-in + kira) is a real risk to flag in plans — disabling one is non-trivial under the Druum feature shape.

Companion memory: `feedback_bevy_0_18_event_message_split.md` (Message vs Event), `reference_bevy_0_18_local_source.md` (the source-on-disk pattern this verification used).
