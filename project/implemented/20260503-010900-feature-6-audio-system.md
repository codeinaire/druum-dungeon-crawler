# Implementation Summary: Audio System (BGM + SFX) — Feature #6

**Date:** 2026-05-03
**Plan:** project/plans/20260502-120000-feature-6-audio-system.md
**Status:** Complete

## Steps completed

All 9 steps completed in order with no skips.

- **Step 1:** Kira availability check. `bevy_kira_audio = 0.25.0` found, requires `bevy = "^0.18.0"`. Option B technically viable but not selected. Proceeded with Option A unconditionally.
- **Step 2:** Tooling check. `ffmpeg` found at `/opt/homebrew/bin/ffmpeg` (version 8.1). Path 1 selected.
- **Step 3:** Generated 10 silent .ogg placeholder files under `assets/audio/bgm/` and `assets/audio/sfx/`. Updated `assets/README.md` with audio directory layout table.
- **Step 4:** Added `AudioAssets` struct to `src/plugins/loading/mod.rs` (after `DungeonAssets`) and chained `.load_collection::<AudioAssets>()` to the `LoadingState` builder.
- **Step 5:** Wrote full `src/plugins/audio/mod.rs` — 4 channel marker components, `ChannelVolumes` resource, `AudioPlugin::build` wiring, stub `bgm.rs`/`sfx.rs`, and all 6 unit tests.
- **Step 6:** Replaced `bgm.rs` stub with full crossfade implementation — `FadeIn`/`FadeOut` components, `play_bgm_for_state` system, `fade_in_tick`/`fade_out_tick` tick systems.
- **Step 7:** Replaced `sfx.rs` stub with full `SfxRequest`/`SfxKind` message consumer.
- **Step 8:** Fade-tick lifecycle tests already included in Step 5's test block. All 6 tests confirmed passing under both feature sets.
- **Step 9:** Full verification matrix run. All commands pass with zero warnings. Cargo.toml and Cargo.lock diffs are empty.

## Steps skipped

None.

## Deviations from the plan

1. **ffmpeg `-c:a libvorbis` recipe failed.** The plan's Step 3 recipe used `-c:a libvorbis` but this homebrew ffmpeg 8.1 does not include `libvorbis`. Corrected to the native vorbis encoder with `-strict -2` flag: `ffmpeg -f lavfi -i anullsrc=r=44100:cl=stereo -t 1 -c:a vorbis -strict -2 /tmp/silent.ogg`. Files are valid Vorbis OGG (~4.7 KB each vs plan's "~1-3 KB" estimate) but otherwise identical.

2. **`use bevy::time::Time;` removed from test block.** The plan's Step 8 test code included `use bevy::time::Time;` but this import was unused (the type is only referenced in a doc comment). Removed to prevent the unused-import warning that would fail the `-D warnings` quality gate.

3. **`make_test_app()` required `#[cfg(feature = "dev")]` ButtonInput init.** The plan did not include this in the test helper. When `--features dev` is active, `StatePlugin::build` registers `cycle_game_state_on_f9` which requires `ButtonInput<KeyCode>`. Fix applied matching the established pattern in `src/plugins/state/mod.rs:107`.

## Issues deferred

- Interactive F9-cycle verification of the audio crossfade was not performed in the automated run (background process, no terminal input). The Loading→TitleScreen transition was confirmed no-panic; manual verification of F9 cycling across all states should be performed by a human before the code review step.
- `cargo audit` not run — `cargo-audit` not installed locally (same note as Feature #5 pipeline state).

## Verification results

| Command | Status |
|---------|--------|
| `cargo check` | passed, zero warnings |
| `cargo check --features dev` | passed, zero warnings |
| `cargo clippy --all-targets -- -D warnings` | passed |
| `cargo clippy --all-targets --features dev -- -D warnings` | passed |
| `cargo test` | passed (38 lib tests + 1 integration test) |
| `cargo test --features dev` | passed (39 lib tests + 1 integration test) |
| `git diff Cargo.toml` | empty (zero changes) |
| `git diff Cargo.lock` | empty (zero changes) |

## Manual audible smoke observations

`cargo run --features dev` ran for 15 seconds without panics. Observed:
- `GameState -> Loading` on startup
- `bevy_asset_loader: Loading state is done` — both `DungeonAssets` and `AudioAssets` loaded; all 10 silent .ogg files decoded without error (lewton accepted Vorbis content as expected)
- `GameState -> TitleScreen` auto-advance — `play_bgm_for_state` fired, spawned BGM entity with `Bgm` marker
- No decode panics, no error logs
- Silent placeholders produce no audible output; the volume fade curve runs without hardware-audible artifacts

## Final LOC

- `src/plugins/audio/mod.rs`: 342 lines
- `src/plugins/audio/bgm.rs`: 171 lines
- `src/plugins/audio/sfx.rs`: 87 lines
- No separate integration test file — tests are in `mod.rs::tests`
