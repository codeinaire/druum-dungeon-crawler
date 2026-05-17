---
name: feedback_dungeonassets_field_addition_landmine
description: Adding fields to DungeonAssets breaks 7+ test harness struct literals — always grep before/after
metadata:
  type: feedback
---

When adding fields to `DungeonAssets` in `src/plugins/loading/mod.rs`, **all test harnesses that construct `DungeonAssets { ... }` struct literals must also be updated** or they will fail to compile (struct update syntax `..Default::default()` is not available since `AssetCollection` does not derive `Default`).

**Why:** DungeonAssets uses `#[derive(AssetCollection, Resource)]` which does not derive `Default`. Any struct literal missing a field is a compile error, not a missing-field warning.

**How to apply:** Before adding a field, run `rg 'DungeonAssets {' src/ tests/` to find all 7+ locations. After adding the field, add `new_field: Handle::default()` to each. The 7 known locations as of Feature #20:
- `tests/dungeon_movement.rs`
- `tests/dungeon_geometry.rs`
- `src/plugins/dungeon/tests.rs`
- `src/plugins/dungeon/features.rs`
- `src/plugins/ui/minimap.rs`
- `src/plugins/combat/encounter.rs`
- `src/plugins/combat/turn_manager.rs`

Also check `src/plugins/town/guild_skills.rs` test's `build_test_app` function (Feature #20 added this one).

Related: [[feedback_dungeonassets_floor_width_for_tests]]
