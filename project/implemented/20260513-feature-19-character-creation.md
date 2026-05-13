# Implementation Summary: Feature #19 — Character Creation & Class Progression

**Plan:** `project/plans/20260513-120000-feature-19-character-creation.md`
**Date:** 2026-05-13

## Steps Completed

All 19 plan steps (1.1–6.1) implemented.

### Phase 1 — Schema extensions and data layer
- **1.1** Extended `ClassDef` with 6 new `#[serde(default)]` fields: `min_stats`, `allowed_races`, `advancement_requirements`, `bonus_pool_min`, `bonus_pool_max`, `stat_penalty_on_change`. Added `ClassRequirement` struct. Updated test fixtures.
- **1.2** Populated the 6 new fields in `assets/classes/core.classes.ron` for Fighter/Mage/Priest.
- **1.3** Created `src/data/races.rs` with `RaceData`, `RaceTable`, `get()` method, and 3 unit tests.
- **1.4** Wired `RaceTable` through `src/data/mod.rs`.
- **1.5** Created `assets/races/core.races.racelist.ron` with all 5 races (Human/Elf/Dwarf/Gnome/Hobbit), using two's-complement u16 encoding for signed stat modifiers.
- **1.6** Registered `RonAssetPlugin::<RaceTable>` in `LoadingPlugin::build`, extended `TownAssets` with `race_table` and `class_table` fields.

### Phase 2 — Progression module
- **2.1** Created `src/plugins/party/progression.rs` (~400 LOC): `ProgressionRng` resource, `CombatVictoryEvent` message, `AllocError`/`CreateError` error types, `StatGains` struct, 7 pure functions (`level_cap`, `xp_for_level`, `xp_to_next_level_for`, `recompute_xp_to_next_level`, `level_up`, `roll_bonus_pool`, `allocate_bonus_pool`, `can_create_class`), 2 handler systems (`award_combat_xp`, `apply_level_up_threshold_system`), `PartyProgressionPlugin`, and 13 tests.
- **2.2** Registered `PartyProgressionPlugin` in `PartyPlugin::build`, added `pub mod progression` and re-exports.

### Phase 3 — Guild creation wizard
- **3.1** Extended `GuildMode` enum with 6 new variants (`CreateRace`/`CreateClass`/`CreateRoll`/`CreateAllocate`/`CreateName`/`CreateConfirm`).
- **3.2–3.8** Created `src/plugins/town/guild_create.rs` with `CreationDraft` resource, `projected_base_stats` helper, and 6 painters (race, class, roll, allocate, name, confirm).
- **3.9–3.12** Added 5 handler systems: `handle_guild_create_input` (umbrella navigation), `handle_guild_create_allocate` (Left/Right stat allocation), `handle_guild_create_name_input` (KeyboardInput reader), `handle_guild_create_roll` (R key pool roll), `handle_guild_create_confirm` (final validation + pool push).
- **3.13** Wired all 6 painters and 5 handlers in `src/plugins/town/mod.rs` with `in_guild_mode()` helper and `OnExit(TownLocation::Guild)` draft reset.
- **3.14** Added `]`-key (MenuAction::NextTarget) entry point in `handle_guild_input` to start creation from Roster mode.

### Phase 4 — Combat victory XP hook
- **4.1** Added `compute_xp_from_enemies` free function and `CombatVictoryEvent` emission in `check_victory_defeat_flee` in `src/plugins/combat/turn_manager.rs`.

### Phase 5 — Tests
- **5.1–5.2** 13 unit tests + 1 integration test in `progression.rs`.
- **5.3** 3 integration tests in `guild_create.rs`.
- **5.4** 1 unit test in `turn_manager.rs`.

### Phase 6 — Documentation
- **6.1** Module-level doc comments for `progression.rs`, `guild_create.rs`, `races.rs`.

## Deviations from Plan

1. **Double-CentralPanel fix**: `paint_guild` was modified to early-return for all 6 creation sub-modes before rendering its `CentralPanel`. The plan didn't anticipate that both `paint_guild` and the creation painters would try to render `CentralPanel`s simultaneously. The fix adds an early-return guard block before the `CentralPanel` call.

2. **B0002 in `handle_guild_create_confirm`**: Plan sketched both `Res<GuildState>` and `ResMut<GuildState>` as params. Fixed to use only `ResMut<GuildState>`.

3. **Test Dwarf race data**: `creation_rejects_class_below_min_stats` needed a Dwarf entry in the test race table for `projected_base_stats` to return `Some(_)` and reach the `can_create_class` rejection. Added `dwarf_race_data()` to the test app setup.

4. **`SeedableRng` import in `guild_create.rs` tests**: Added `use rand::SeedableRng;` to the test module since the parent module doesn't import it.

5. **Let-chain refactoring**: Nested `if let` patterns converted to Rust 2024 let-chain syntax (`if let A && let B { ... }`) to avoid `clippy::collapsible_if` warnings.

6. **`_ => {}` wildcard arm in `paint_guild` CentralPanel match**: Kept as an exhaustiveness requirement; the compiler cannot prove the early-return block makes the creation variants unreachable at the type level.

7. **`add_message::<KeyboardInput>()` in guild_create tests**: `handle_guild_create_name_input` uses `MessageReader<KeyboardInput>`. In `MinimalPlugins` test apps (without `InputPlugin`), `Messages<KeyboardInput>` is not registered — causing a panic at system param validation time. Fixed by adding `app.add_message::<KeyboardInput>()` to `make_create_test_app()`.

## Deferred Issues

- **Manual smoke tests**: The 4 manual smoke tests in the Verification section (RON load, end-to-end creation, level-up, class-roster filter) require `cargo run --features dev`. These are not automated — deferred to the shipper/reviewer.
- **Quality gates**: 6 automated quality gates (`cargo check`, `cargo check --features dev`, `cargo test`, `cargo test --features dev`, `cargo clippy --all-targets -- -D warnings`, `cargo clippy --all-targets --features dev -- -D warnings`) were not executed in-session (no shell access). Static analysis was performed and all identified issues were fixed. Gates must be run by whoever has shell access.

## Files Modified

- `src/data/classes.rs` — `ClassDef` + `ClassRequirement` extensions, test fixtures updated
- `src/data/mod.rs` — `pub mod races` + re-exports
- `src/data/races.rs` — NEW: `RaceData`, `RaceTable`, tests
- `src/plugins/loading/mod.rs` — `RonAssetPlugin::<RaceTable>`, `TownAssets` fields
- `src/plugins/party/mod.rs` — `pub mod progression`, plugin registration, re-exports
- `src/plugins/party/progression.rs` — NEW: full progression module
- `src/plugins/town/guild.rs` — `GuildMode` extended, `paint_guild` early-return, entry point
- `src/plugins/town/guild_create.rs` — NEW: creation wizard UI + handlers + tests
- `src/plugins/town/mod.rs` — wiring for creation systems
- `src/plugins/town/inn.rs` — `TownAssets` mock updated (2 new fields)
- `src/plugins/town/temple.rs` — `TownAssets` mock updated (2 new fields)
- `src/plugins/combat/turn_manager.rs` — `CombatVictoryEvent` emission on victory
- `assets/classes/core.classes.ron` — 6 new fields for Fighter/Mage/Priest
- `assets/races/core.races.racelist.ron` — NEW: all 5 race definitions
