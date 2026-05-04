# Implementation Summary: Feature #8 — 3D Dungeon Renderer

**Plan:** `/Users/nousunio/Repos/Learnings/claude-code/druum/project/plans/20260503-223000-feature-8-3d-dungeon-renderer.md`
**Date:** 2026-05-04
**Status:** Complete (manual smoke test pending user execution)

## Steps Completed

All 13 plan steps implemented in a single session:

1. Added `CELL_HEIGHT = 3.0`, `WALL_THICKNESS = 0.05`, `FLOOR_THICKNESS = 0.05` constants and `DungeonGeometry` marker component.
2. Deleted `TestSceneMarker`, `spawn_test_scene`, and the test-scene despawn branch from `despawn_dungeon_entities`.
3. Added `wall_transform(grid_x, grid_y, dir) -> Transform` pure helper + 5 unit tests.
4. Added `wall_material(wt, solid, door, locked) -> Option<Handle<...>>` pure helper + 2 unit tests.
5. Fixed `src/data/dungeon.rs:18` doc-comment: `-grid_y` → `+grid_y` (single-char edit).
6. Wrote `spawn_dungeon_geometry` system signature, asset guards, and cached mesh/material handles.
7. Implemented per-cell iteration loop + Wizardry torchlight ambient override (user override applied: no DirectionalLight, `GlobalAmbientLight { brightness: 50.0 }` instead of 100.0).
8. Updated `despawn_dungeon_entities` to include `DungeonGeometry` query and `GlobalAmbientLight::default()` restore.
9. Added `make_walled_floor` helper + 3 App-level entity-count tests (adjusted for user override).
10. Created `tests/dungeon_geometry.rs` integration test (entity count 120, adjusted from plan's 121).
11. Ran manifest byte-diff guard — Cargo.toml and Cargo.lock unchanged.
12. Ran full verification matrix — all 7 commands pass.
13. Manual visual smoke test: game launches and loads cleanly; user must press F9 to cycle to Dungeon for visual verification.

**User override applied (Wizardry torchlight instead of plan's Etrian Odyssey DirectionalLight):**
- No `DirectionalLight` entity in `spawn_dungeon_geometry`.
- `GlobalAmbientLight { color: Color::WHITE, brightness: 50.0 }` (near-black, not 100.0).
- `PointLight { color: srgb(1.0, 0.85, 0.55), intensity: 1500.0, range: 6.0, shadows_enabled: false }` spawned as grandchild of `PlayerParty` via `children!` inside `DungeonCamera`. Local transform at `(0, 0, 0)` — at the camera eye position.

## Steps Skipped

None. All 13 steps executed.

## Deviations from Plan

1. **Entity counts adjusted throughout** (19→18 open 3×3, 43→42 walled 3×3, 121→120 for floor_01) because the PointLight is not tagged `DungeonGeometry` — it's a child of DungeonCamera.
2. **`spawn_party_and_camera` modified** (plan said it would stay untouched) — the PointLight addition required adding a nested `children!` block inside the existing DungeonCamera spawn. This is the correct implementation of the user override. The movement logic is semantically unchanged.
3. **No intermediate broken-compile checkpoint** — steps 1–7 were written atomically. The plan's note that "the build will break between steps" was not violated in practice because all changes were applied before running `cargo check`.
4. **`collapsible_if` clippy fix** — the nested `if y == floor.height - 1 { if let Some(mat) = ...` pattern was merged into `if y == floor.height - 1 && let Some(mat) = ...` (Rust 2024 let-chain syntax). The plan's code snippet used the nested form but the let-chain form is correct for this edition.
5. **`cargo fmt` reformatted** PointLight field alignment comments and two `if let` expressions from inline to expanded form. Applied before fmt check.

## Verification Results

| Command | Status |
|---------|--------|
| `cargo check` | passed — zero warnings |
| `cargo check --features dev` | passed — zero warnings |
| `cargo clippy --all-targets -- -D warnings` | passed — zero warnings |
| `cargo clippy --all-targets --features dev -- -D warnings` | passed — zero warnings |
| `cargo test` | passed — 61 unit tests + 3 integration tests |
| `cargo test --features dev` | passed — 62 unit tests + 3 integration tests |
| `cargo fmt --check` | passed |
| `git diff Cargo.toml Cargo.lock` | empty — byte-unchanged |
| Manual visual smoke | pending user execution |

Test counts:
- Unit tests: +11 new (5 wall_transform + 2 wall_material + 3 geometry/OnExit entity-count tests)
- Integration tests: +1 new (dungeon_geometry.rs)

## Final LOC

- `src/plugins/dungeon/mod.rs`: 997 → **1355 LOC** (+358 net)
- `src/data/dungeon.rs`: unchanged (1 character edited, 0 net LOC change)
- `tests/dungeon_geometry.rs`: **159 LOC** (new file)

The +358 is just outside the plan's +350 upper bound but within the 25% overrun band (+420 expected with overrun). The extra ~8 LOC came from the PointLight nested-children addition in `spawn_party_and_camera` and the fmt-expanded if-let patterns.

## Cargo.toml and Cargo.lock

Both files are byte-identical to the pre-Feature-#8 commit. Zero new dependencies.

## Manual Visual Smoke Observations

Game launches cleanly. Loading → TitleScreen transition verified by log:
```
INFO druum::plugins::state: GameState -> Loading
INFO bevy_asset_loader: Loading state done
INFO druum::plugins::state: GameState -> TitleScreen
```

No error or warning logs related to geometry spawn or lighting. To complete the visual smoke test, user should:
1. `cargo run --features dev`
2. Press F9 three times to cycle Loading → TitleScreen → Town → Dungeon
3. Verify: walls render at correct positions, floor/ceiling visible, torchlight effect (warm nearby, dark distant), OneWay asymmetry at (2,3) east, no z-fighting
4. If lighting is too dark: increase `intensity` (currently 1500.0) or `range` (currently 6.0)
5. If lighting is too bright: decrease `intensity` or `brightness` (currently 50.0)

## Issues Deferred

- Shadows: `shadows_enabled: false` on PointLight. Feature #9 owns shadows.
- Texture variation: all geometry uses flat `StandardMaterial` colors. Feature #13 owns per-cell variation.
- Per-cell debug overlay: Feature #10 (auto-map) and #25 (polish).
- "Held torch" offset: PointLight is at camera origin `(0, 0, 0)`. A slightly-forward/below position could improve the "held torch" feel. Deferred to #25.
