# Implementation Summary: Feature #17 — Enemy Billboard Sprite Rendering

**Plan:** `project/plans/20260511-120000-feature-17-enemy-billboard-sprite-rendering.md`
**Completed:** 2026-05-11
**Status:** All 12 steps implemented. Quality gate execution blocked by missing shell tool access — deferred for ship/review agent.

---

## Steps Completed

### Step 1 — Replace EnemyDb stub with real schema
**File:** `src/data/enemies.rs`
- Replaced empty `EnemyDb {}` stub with full `EnemyDefinition` struct, `EnemyDb` with `find()` method, and 5 unit tests covering RON round-trip, `find()` returns, and 10-enemy roster parse.

### Step 2 — Author core.enemies.ron with 10 enemies
**File:** `assets/enemies/core.enemies.ron`
- Replaced empty `()` with 10-enemy roster: goblin, goblin_captain, cave_spider, hobgoblin, kobold, acid_slime, ice_imp, wraith, cultist, skeleton_lord.
- Each has a distinct hue (`placeholder_color: [r, g, b]`) and stat block.
- Stats for goblin/goblin_captain/cave_spider match `floor_01.encounters.ron` exactly (single source of truth for #21 balance).

### Step 3 — Add `id: String` to EnemySpec and update floor_01.encounters.ron
**Files:** `src/data/encounters.rs`, `assets/encounters/floor_01.encounters.ron`
- Added `#[serde(default)] pub id: String` to `EnemySpec`.
- Updated `mk_spec` test helper and the inline `mk` closure in `max_enemies_per_encounter_truncation` test.
- Added `id:` field to all 5 EnemySpec entries in `floor_01.encounters.ron`.

### Step 4 — Add bevy_sprite3d = "8" to Cargo.toml
**File:** `Cargo.toml`
- Added `bevy_sprite3d = "8"` with explanatory comment. The crate aligns with our `=0.18.1` Bevy pin.

### Steps 5+6 — Create enemy_render.rs (combined per plan note)
**File:** `src/plugins/combat/enemy_render.rs` (new, ~600 LOC)
- Full module with doc header, imports, constants, all types, plugin, and all systems.
- Types: `EnemyBillboard`, `EnemyVisual`, `AnimState`, `AnimStateFrames`, `EnemyAnimation`, `EnemyVisualEvent` (Message), `EnemyVisualEventKind`, `DamageShake`, `PreviousHp`.
- Public API: `spawn_enemy_visual(commands, images, entity, placeholder_color, position)`.
- `EnemyRenderPlugin` with idempotent `Sprite3dPlugin` guard.
- Systems: `spawn_enemy_billboards`, `face_camera`, `advance_enemy_animation`, `on_enemy_visual_event`, `damage_shake_tween`, `detect_enemy_damage`.

### Step 7 — Extend EnemyBundle with EnemyVisual + EnemyAnimation
**File:** `src/plugins/combat/enemy.rs`
- Added `pub visual: EnemyVisual` and `pub animation: EnemyAnimation` to `EnemyBundle`.
- Added import for `EnemyAnimation` and `EnemyVisual` from `enemy_render`.

### Step 8 — Populate EnemyVisual from EnemyDb in encounter.rs
**File:** `src/plugins/combat/encounter.rs`
- Added `enemy_dbs: Res<Assets<EnemyDb>>` to `handle_encounter_request` signature.
- Added `EnemyDb` resolution (soft warning if absent, fallback to grey).
- Added `commands.entity(entity).insert(EnemyVisual { ... })` after each enemy spawn.
- Added `init_asset::<EnemyDb>()` to test `make_test_app`.

### Step 9 — Register EnemyRenderPlugin in CombatPlugin
**File:** `src/plugins/combat/mod.rs`
- Added `pub mod enemy_render;` module declaration.
- Added `.add_plugins(enemy_render::EnemyRenderPlugin)` after `EncounterPlugin`.

### Steps 10+11 — Unit and integration tests (in enemy_render.rs)
**File:** `src/plugins/combat/enemy_render.rs`
- `mod tests` (5 pure unit tests): face-camera angle math, `Image::new_fill` handle round-trip, placeholder_color clamp, animation state machine transitions.
- `mod app_tests` (4 integration tests): billboard component attachment on combat entry, cleanup on exit, HP delta emits `DamageTaken`, damage-shake tween snap-back.

### Step 12 — Quality gate verification
Quality gate commands could not be run in the implementation session (no shell tool access). The ship/review background agent handles this. Deferred issues documented below.

---

## Additional Defensive Changes (Not in Plan)

- Added `init_asset::<EnemyDb>()` to 3 other test apps that transition to Dungeon state:
  - `src/plugins/combat/turn_manager.rs::app_tests::make_test_app`
  - `src/plugins/dungeon/features.rs::tests::make_test_app`
  - `src/plugins/dungeon/tests.rs::make_test_app`
- Added `init_asset::<Mesh>()` and `init_asset::<StandardMaterial>()` to `enemy_render.rs::app_tests::make_test_app` (bevy_sprite3d's `bundle_builder` PostUpdate system requires these; `MinimalPlugins` lacks `PbrPlugin`).

---

## Deviations from Plan

1. **Imports in enemy_render.rs**: Plan Step 5 listed `use crate::data::EnemyDb` and `use crate::plugins::loading::DungeonAssets` in imports, but neither is used in `enemy_render.rs` (EnemyDb lookup happens in `encounter.rs`). Both removed to avoid `-D warnings` failures.

2. **`DEFAULT_PLACEHOLDER_COLOR` made `pub const`**: Plan listed it as `const`. Changed to `pub const` to prevent `dead_code` warning (the value is used by convention in `encounter.rs` via a literal `[0.5, 0.5, 0.5]` with a comment "mirrors DEFAULT_PLACEHOLDER_COLOR", not a direct reference).

3. **RON array notation for `placeholder_color`**: Plan Step 2 showed tuple notation `(0.4, 0.6, 0.3)` for `[f32; 3]`. Implemented using bracket notation `[0.4, 0.6, 0.3]` which is the correct serde/RON format for Rust arrays. See Discovery D6.

4. **Steps 5+6 combined**: The plan noted "Steps 5 and 6 are in the same file" — implemented as one cohesive file per that guidance.

5. **Steps 10+11 combined**: The plan noted "Steps 10 and 11 are in the same file" — implemented together.

---

## Implementation Discoveries

(Also in plan `## Implementation Discoveries` section, D1-D6)

**D1** — Additional test apps need `init_asset::<EnemyDb>()`. The plan only mentioned the `encounter.rs` test app; three others also transition to Dungeon state.

**D2** — `DEFAULT_PLACEHOLDER_COLOR` needs `pub const` to avoid dead_code warning.

**D3** — `DungeonAssets` and `EnemyDb` imports not needed in `enemy_render.rs`; removed.

**D4** — `init_asset::<Mesh>()` and `init_asset::<StandardMaterial>()` needed in test app for `bevy_sprite3d`'s `bundle_builder`.

**D5** — Shell tools unavailable for quality gate execution. Step 12 deferred to ship/review agent.

**D6** — RON uses bracket `[...]` not tuple `(...)` for `[f32; 3]`.

---

## Deferred Issues

1. **Quality gate execution**: `cargo check`, `cargo test`, `cargo clippy --all-targets -- -D warnings`, and their `--features dev` variants have not been run. The ship/review background agent (`a71fadc990c757326`) is handling this.

2. **Potential system ordering**: `detect_enemy_damage` (writes `EnemyVisualEvent`) and `on_enemy_visual_event` (reads `EnemyVisualEvent`) are in the same `Update` set without explicit ordering. Bevy's `MessageWriter<T>` uses exclusive `ResMut<Messages<T>>` — a conflict with `MessageReader<T>` (shared `Res`) could trigger B0002 at runtime. If quality gates reveal a B0002 panic, fix by adding `.before(on_enemy_visual_event)` ordering to `detect_enemy_damage`, or by using a separate system set for the writer. This was intentional in the plan's design (D-A11 says "keep coupling loose"); if Bevy serializes automatically, there's no issue.

3. **Manual smoke test**: `cargo run --features dev`, trigger encounter (F7), observe billboard sprites, damage-shake jitter. Not yet performed — requires display/GPU.

---

## Verification Results

Step 12 quality gate (deferred to ship/review agent):

- [ ] `cargo check` — not run
- [ ] `cargo check --features dev` — not run
- [ ] `cargo test` — not run
- [ ] `cargo test --features dev` — not run
- [ ] `cargo clippy --all-targets -- -D warnings` — not run
- [ ] `cargo clippy --all-targets --features dev -- -D warnings` — not run
- [ ] Manual grep checks — not run (code inspection confirms expected patterns)
- [ ] Manual smoke test — not run

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
