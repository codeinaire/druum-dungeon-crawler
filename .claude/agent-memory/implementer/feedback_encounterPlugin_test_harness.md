---
name: EncounterPlugin test harness requirements post-#16
description: CombatPlugin test apps need init_asset EncounterTable/DungeonFloor, init_resource ActiveFloorNumber, add_message EncounterRequested/SfxRequest; CellFeaturesPlugin test apps also need ActionState<DungeonAction>
type: feedback
---

After Feature #16, `EncounterPlugin` is a sub-plugin of `CombatPlugin`. Test harnesses must explicitly initialize everything EncounterPlugin and CellFeaturesPlugin need.

**Test apps with CombatPlugin but WITHOUT CellFeaturesPlugin** (e.g., turn_manager, ui_combat):

1. `app.init_asset::<crate::data::EncounterTable>();` — `handle_encounter_request` reads it
2. `app.init_asset::<crate::data::DungeonFloor>();` — `check_random_encounter` reads it (runs when combat exits back to Dungeon)
3. `app.init_resource::<crate::plugins::dungeon::ActiveFloorNumber>();` — `check_random_encounter` reads it
4. `app.add_message::<crate::plugins::dungeon::features::EncounterRequested>();` — normally by CellFeaturesPlugin
5. `app.add_message::<crate::plugins::audio::SfxRequest>();` — `handle_encounter_request` writes it; normally by AudioPlugin

**Test apps with CombatPlugin AND CellFeaturesPlugin** (e.g., encounter.rs tests, features.rs tests):

All 5 above still apply, PLUS:
6. `app.add_message::<crate::plugins::audio::SfxRequest>();` — CellFeaturesPlugin systems (apply_poison_trap, apply_alarm_trap, etc.) write SfxRequest; AudioPlugin normally registers it
7. `app.init_resource::<leafwing_input_manager::prelude::ActionState<crate::plugins::input::DungeonAction>>();` — `handle_door_interact` (CellFeaturesPlugin) reads it; insert WITHOUT InputManagerPlugin to avoid mouse panic

**DungeonAssets cascade:** When DungeonAssets gains a new field (like encounters_floor_01), ALL struct literal sites must be updated — including non-combat/dungeon test files like `minimap.rs`.

**Why:** EncounterPlugin is invisible at the call site; cascading system param failures only show up at runtime during cargo test.

**How to apply:** Run `grep -rn "DungeonAssets {" src/ tests/` when DungeonAssets struct changes; audit all make_test_app functions for the full resource list above.
