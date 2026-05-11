# Implementation Summary: Feature #17 — Enemy Billboard Sprite Rendering

**Plan:** `project/plans/20260511-120000-feature-17-enemy-billboard-sprite-rendering.md`
**Code committed:** 2026-05-11
**Status:** Complete — all 6 quality gates green, all grep checks pass

---

## Steps Completed

### Step 1 — Replace EnemyDb stub with real schema
**File:** `src/data/enemies.rs`
- Replaced empty `EnemyDb {}` stub with full `EnemyDefinition` struct, `EnemyDb` with `find()` method, and 5 unit tests covering RON round-trip, `find()` returns, and 10-enemy roster parse.

### Step 2 — Author core.enemies.ron with 10 enemies
**File:** `assets/enemies/core.enemies.ron`
- Replaced empty `()` with 10-enemy roster: goblin, goblin_captain, cave_spider, hobgoblin, kobold, acid_slime, ice_imp, wraith, cultist, skeleton_lord.
- Each has a distinct hue (`placeholder_color`) and stat block.
- Stats for goblin/goblin_captain/cave_spider match `floor_01.encounters.ron` exactly (single source of truth for #21 balance).
- **Fix applied during verification:** placeholder_color values were authored with bracket notation `[r, g, b]` but RON 0.11.0 represents `[f32; 3]` as tuple `(r, g, b)`. All 10 entries corrected to parentheses.

### Step 3 — Add `id: String` to EnemySpec and update floor_01.encounters.ron
**Files:** `src/data/encounters.rs`, `assets/encounters/floor_01.encounters.ron`
- Added `#[serde(default)] pub id: String` to `EnemySpec`.
- Updated `mk_spec` test helper and the inline `mk` closure in `max_enemies_per_encounter_truncation` test.
- Added `id:` field to all 5 EnemySpec entries in `floor_01.encounters.ron`.

### Step 4 — Add bevy_sprite3d = "8" to Cargo.toml
**File:** `Cargo.toml`
- Added `bevy_sprite3d = "8"` with explanatory comment.

### Steps 5+6 — Create enemy_render.rs (combined per plan note)
**File:** `src/plugins/combat/enemy_render.rs` (new)
- Full module with doc header, imports, constants, all types, plugin, and all systems.
- Types: `EnemyBillboard`, `EnemyVisual`, `AnimState`, `AnimStateFrames`, `EnemyAnimation`, `EnemyVisualEvent` (Message), `EnemyVisualEventKind`, `DamageShake`, `PreviousHp`.
- Public API: `spawn_enemy_visual(commands, images, entity, placeholder_color, position)`.
- `EnemyRenderPlugin` with idempotent `Sprite3dPlugin` guard.
- Systems: `spawn_enemy_billboards`, `face_camera`, `advance_enemy_animation`, `on_enemy_visual_event`, `damage_shake_tween`, `detect_enemy_damage`.
- **Fix applied during verification:** collapsible_if clippy lint in `on_enemy_visual_event` — nested `if kind == ... { if let Ok(...) { ... } }` collapsed to let-chain.
- **Fix applied during verification:** E0716 in test code — `app.world_mut().entity_mut(e).get_mut()` chain required two named bindings to avoid borrow-of-dropped-temporary.

### Step 7 — Extend EnemyBundle with EnemyVisual + EnemyAnimation
**File:** `src/plugins/combat/enemy.rs`
- Added `pub visual: EnemyVisual` and `pub animation: EnemyAnimation` to `EnemyBundle`.

### Step 8 — Populate EnemyVisual from EnemyDb in encounter.rs
**File:** `src/plugins/combat/encounter.rs`
- Added `enemy_dbs: Res<Assets<EnemyDb>>` to `handle_encounter_request` signature.
- Added `EnemyDb` resolution (soft warning if absent, fallback to grey).
- Added `commands.entity(entity).insert(EnemyVisual { ... })` after each enemy spawn.

### Step 9 — Register EnemyRenderPlugin in CombatPlugin
**File:** `src/plugins/combat/mod.rs`
- Added `pub mod enemy_render;` module declaration.
- Added `.add_plugins(enemy_render::EnemyRenderPlugin)` after `EncounterPlugin`.

### Steps 10+11 — Unit and integration tests (in enemy_render.rs)
**File:** `src/plugins/combat/enemy_render.rs`
- `mod tests` (5 pure unit tests): face-camera angle math, `Image::new_fill` handle round-trip, placeholder_color clamp, animation state machine transitions.
- `mod app_tests` (4 integration tests): billboard component attachment on combat entry, cleanup on exit, HP delta emits `DamageTaken`, damage-shake tween snap-back.

### Step 12 — Quality gate verification
All 6 gates run and green. Failures found and fixed — see Deviations section.

---

## Additional Defensive Changes (Not in Plan)

- Added `init_asset::<EnemyDb>()` to 3 other lib test apps that transition to Dungeon state:
  - `src/plugins/combat/turn_manager.rs::app_tests::make_test_app`
  - `src/plugins/dungeon/features.rs::tests::make_test_app`
  - `src/plugins/dungeon/tests.rs::make_test_app`
- Added `init_asset::<bevy::image::Image>()` and `init_asset::<bevy::image::TextureAtlasLayout>()` to ALL test apps that use CombatPlugin (7 lib + 2 integration tests) — required by `bevy_sprite3d::bundle_builder`'s system parameter validation at startup.
- Added `init_asset::<druum::data::EnemyDb>()` to integration tests `tests/dungeon_geometry.rs` and `tests/dungeon_movement.rs` — required by `handle_encounter_request`'s `Res<Assets<EnemyDb>>` parameter.

---

## Deviations from Plan

1. **RON bracket vs parenthesis for `placeholder_color`** (D6): Prior implementer used `[r, g, b]` in `core.enemies.ron` for `[f32; 3]`. RON 0.11.0 expects `(r, g, b)` tuple notation for fixed-size arrays. Fixed during verification.

2. **`init_asset::<Image>()` and `init_asset::<TextureAtlasLayout>()` needed in all test harnesses** (D7): `bevy_sprite3d::bundle_builder` requires these two asset registries at system validation time even if no `Sprite3d` entities exist. The plan's D4 only mentioned Mesh + StandardMaterial. Fixed during verification.

3. **E0716 in test code** (D8): Chained `entity_mut().get_mut()` needed two named bindings. Fixed during verification.

4. **collapsible_if in `on_enemy_visual_event`** (D9): Nested if/if-let collapsed to let-chain per clippy::collapsible_if + Rust 2024 edition. Fixed during verification.

5. **`DEFAULT_PLACEHOLDER_COLOR` made `pub const`**: Plan listed it as `const`. Changed to `pub const` to prevent dead_code warning.

6. **Steps 5+6 combined**: Implemented as one cohesive file per plan guidance.

7. **Steps 10+11 combined**: Implemented together per plan guidance.

---

## Verification Results

- [x] `cargo check` — EXIT:0
- [x] `cargo check --features dev` — EXIT:0
- [x] `cargo test` — EXIT:0 — 225 lib + 6 integration tests pass
- [x] `cargo test --features dev` — EXIT:0 — 229 lib + 6 integration tests pass
- [x] `cargo clippy --all-targets -- -D warnings` — EXIT:0
- [x] `cargo clippy --all-targets --features dev -- -D warnings` — EXIT:0
- [x] `rg 'bevy_sprite3d' Cargo.toml` — 2 matches (comment + dep line)
- [x] `rg 'use bevy_sprite3d' src/plugins/combat/enemy_render.rs` — 2 matches (use stmt + comment)
- [x] `rg 'Sprite3dPlugin' src/plugins/combat/enemy_render.rs` — 4 matches (import, guard comment, is_plugin_added, add_plugins)
- [x] `rg 'Sprite3dBillboard' src/` — 0 matches (renamed to EnemyBillboard)
- [x] `rg 'Sprite3d \{' src/plugins/combat/enemy_render.rs` — 1 match (inside spawn_enemy_visual)
- [x] `rg 'Plane3d' src/` — 0 matches
- [x] `rg 'Camera3d' src/plugins/combat/enemy_render.rs` — 2 matches, both inside app_tests::spawn_test_camera
- [x] `rg 'unlit: true' src/plugins/combat/enemy_render.rs` — 1 match
- [x] `rg 'AlphaMode::' src/plugins/combat/enemy_render.rs` — only AlphaMode::Mask(0.5), no Blend
- [x] `rg '\.despawn\(\)' src/plugins/combat/enemy_render.rs` — 0 matches
- [x] `rg 'RenderAssetUsages::' src/plugins/combat/enemy_render.rs` — only RENDER_WORLD
- [x] `rg '&GlobalTransform' src/plugins/combat/enemy_render.rs` — 2 matches
- [x] `#[derive(Event)]` in enemy_render.rs — 0 matches (doc comment match not a real attribute)
- [x] `rg 'EventReader<|EventWriter<' src/plugins/combat/enemy_render.rs` — 0 matches

---

## Commit

- `9ae9f7f feat(combat): add enemy billboard sprite rendering (#17)` — all code, assets, tests, and test-harness fixes in one atomic commit on `feature/17-enemy-billboard-sprite-rendering`

---

## New Test Coverage (14 new tests)

- `data::enemies::tests::enemy_db_round_trips_via_ron`
- `data::enemies::tests::find_returns_some_for_known_id`
- `data::enemies::tests::find_returns_none_for_unknown_id`
- `data::enemies::tests::find_returns_none_for_empty_id`
- `data::enemies::tests::core_enemies_ron_parses_with_10_enemies`
- `combat::enemy_render::tests::face_camera_angle_at_cardinal_axes`
- `combat::enemy_render::tests::image_new_fill_produces_a_handle`
- `combat::enemy_render::tests::placeholder_color_clamps`
- `combat::enemy_render::tests::animation_attacking_returns_to_idle`
- `combat::enemy_render::tests::animation_dying_holds_last_frame`
- `combat::enemy_render::app_tests::enemies_get_billboard_components_on_combat_entry`
- `combat::enemy_render::app_tests::no_billboard_entities_remain_after_combat_exit`
- `combat::enemy_render::app_tests::hp_delta_emits_damage_taken_event`
- `combat::enemy_render::app_tests::damage_shake_returns_to_base_x`
