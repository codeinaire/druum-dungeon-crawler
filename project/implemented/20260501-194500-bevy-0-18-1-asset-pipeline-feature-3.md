# Implementation Summary: Bevy 0.18.1 Asset Pipeline & RON Loading (Feature #3)

**Date:** 2026-05-01
**Plan:** ../plans/20260501-164500-bevy-0-18-1-asset-pipeline-feature-3.md
**Status:** Complete

## Steps Completed

- **Step 1** — PASS gate: verified `bevy_common_assets = 0.16.0` and `bevy_asset_loader = 0.26.0` both require `^0.18.0` on bevy subcrates. Confirmed `RonAssetPlugin::new(&["dungeon.ron"])` API (no leading dot). Confirmed chained builder syntax `LoadingState::new(...).continue_to_state(...).load_collection::<T>()`.
- **Step 2** — Updated `Cargo.toml`: added `bevy_common_assets = { version = "=0.16.0", features = ["ron"] }` and `bevy_asset_loader = "=0.26.0"` with exact pinning; extended `dev` feature to `["bevy/dynamic_linking", "bevy/file_watcher"]`. Also added `serde = { version = "1", features = ["derive"] }` and `ron = "0.12"` (deviation — see below).
- **Step 3** — Created five data schema files under `src/data/`: `dungeon.rs` (with round-trip test), `items.rs`, `enemies.rs`, `classes.rs`, `spells.rs`. Updated `src/data/mod.rs` with `pub mod` declarations and `pub use` re-exports.
- **Step 4** — Created `src/plugins/loading/mod.rs` with `LoadingPlugin`, `DungeonAssets` (derives `AssetCollection + Resource`), `LoadingScreenRoot` marker, `spawn_loading_screen`, and `despawn_loading_screen`.
- **Step 5** — Added `pub mod loading;` to `src/plugins/mod.rs` (alphabetically between `dungeon` and `party`).
- **Step 6** — Updated `src/main.rs` to import `AssetPlugin` and `LoadingPlugin`; wired `DefaultPlugins.set(AssetPlugin { watch_for_changes_override: Some(cfg!(feature = "dev")), ..default() })` and added `LoadingPlugin` after `StatePlugin` in the plugin tuple.
- **Step 7** — Created five placeholder RON files under `assets/`: `dungeons/floor_01.dungeon.ron`, `items/core.items.ron`, `enemies/core.enemies.ron`, `classes/core.classes.ron`, `spells/core.spells.ron`.
- **Step 8** — Created `assets/README.md` with layout table, hot-reload instructions, contributor guide, and security/trust notes.
- **Step 9** — Final review pass: confirmed no `#[cfg]` attribute misuse, no unused imports, state machine frozen, all invariants hold.

## Steps Skipped

None.

## Deviations from the Plan

1. **Added `serde` and `ron` as explicit Cargo dependencies.** The plan said to omit them since they are transitively present via Bevy. Rust 2024 edition (used by this project) does not allow direct use of transitive crate names in source without explicit declaration. `use serde::{Deserialize, Serialize}` failed with `error[E0432]: unresolved import 'serde'`. Fix: added both with minimal version constraints matching what Bevy already pulls in. This keeps `Cargo.toml` slightly larger but is required for correctness under the edition.

## Issues Deferred

None. All verification items passed. The manual smoke tests (loading screen renders, hot-reload fires, release build excludes `notify`) are deferred to the parent session per the plan.

## Verification Results

| Command | Result |
|---------|--------|
| `cargo check` | PASSED |
| `cargo check --features dev` | PASSED |
| `cargo clippy --all-targets -- -D warnings` | PASSED |
| `cargo clippy --all-targets --features dev -- -D warnings` | PASSED |
| `cargo test` | PASSED (2 tests: `dungeon_floor_round_trips_through_ron`, `gamestate_default_is_loading`) |
| `cargo test --features dev` | PASSED (3 tests: adds `f9_advances_game_state`) |
| `cargo test data::dungeon::tests::dungeon_floor_round_trips_through_ron` | PASSED (1 test, <1 ms) |

## Final LOC

`src/plugins/loading/mod.rs`: **125 lines** (within plan's 150-250 LOC budget).

## Files Written/Modified

- `/Users/nousunio/Repos/Learnings/claude-code/druum/Cargo.toml` — added 4 new dep lines, extended dev feature
- `/Users/nousunio/Repos/Learnings/claude-code/druum/src/data/mod.rs` — replaced placeholder with 5 pub mod + 5 pub use
- `/Users/nousunio/Repos/Learnings/claude-code/druum/src/data/dungeon.rs` — new
- `/Users/nousunio/Repos/Learnings/claude-code/druum/src/data/items.rs` — new
- `/Users/nousunio/Repos/Learnings/claude-code/druum/src/data/enemies.rs` — new
- `/Users/nousunio/Repos/Learnings/claude-code/druum/src/data/classes.rs` — new
- `/Users/nousunio/Repos/Learnings/claude-code/druum/src/data/spells.rs` — new
- `/Users/nousunio/Repos/Learnings/claude-code/druum/src/plugins/mod.rs` — added `pub mod loading;`
- `/Users/nousunio/Repos/Learnings/claude-code/druum/src/plugins/loading/mod.rs` — new
- `/Users/nousunio/Repos/Learnings/claude-code/druum/src/main.rs` — added `AssetPlugin` import + `LoadingPlugin` in tuple
- `/Users/nousunio/Repos/Learnings/claude-code/druum/assets/dungeons/floor_01.dungeon.ron` — new
- `/Users/nousunio/Repos/Learnings/claude-code/druum/assets/items/core.items.ron` — new
- `/Users/nousunio/Repos/Learnings/claude-code/druum/assets/enemies/core.enemies.ron` — new
- `/Users/nousunio/Repos/Learnings/claude-code/druum/assets/classes/core.classes.ron` — new
- `/Users/nousunio/Repos/Learnings/claude-code/druum/assets/spells/core.spells.ron` — new
- `/Users/nousunio/Repos/Learnings/claude-code/druum/assets/README.md` — new
