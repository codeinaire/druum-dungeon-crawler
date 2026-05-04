# Review: Feature #8 — 3D Dungeon Renderer (PR #8)

**Date:** 2026-05-03  
**Verdict:** APPROVE  
**PR:** https://github.com/codeinaire/druum-dungeon-crawler/pull/8  
**Branch:** `8-3d-dungeon-renderer` → `main`  

## Severity Counts

| Severity | Count |
| -------- | ----- |
| CRITICAL | 0     |
| HIGH     | 0     |
| MEDIUM   | 0     |
| LOW      | 1     |

## What Was Reviewed

**Files with full review:**
- `src/data/dungeon.rs` — doc-comment fix only (+1/-1)
- `src/plugins/dungeon/mod.rs` — primary changes (+456/-98)
- `tests/dungeon_geometry.rs` — new integration test (+159)

**Hard gates verified:**
- Cargo.toml SHA identical between main and PR branch (`bd231bcf`) — PASS
- Cargo.lock SHA identical between main and PR branch (`ff9f6ccf`) — PASS
- `TestSceneMarker` + `spawn_test_scene`: zero remaining references in repo — PASS
- `GlobalAmbientLight` resource (not `AmbientLight` component) — correct Bevy 0.18 idiom — PASS
- `Cuboid::new(x, y, z)` full-extents usage verified correct — PASS
- No `PbrBundle` — Bevy 0.18 `(Mesh3d, MeshMaterial3d, Transform)` tuple syntax used — PASS
- No `rand` calls — PASS

## Key Findings

### [LOW] `floor_mesh` and `ceiling_mesh` are identical Cuboid handles

`spawn_dungeon_geometry` calls `meshes.add(Cuboid::new(CELL_SIZE, FLOOR_THICKNESS, CELL_SIZE))` twice, creating two separate handles for geometrically identical meshes. Both are then used exclusively via `.clone()`. At floor_01 scale (36 + 36 tiles) this is two duplicate mesh allocations — negligible cost, but could be a single handle. Not a bug; flagging for awareness only.

## Verified Correct (positive callouts)

**Per-edge canonical iteration:** Independently re-counted against floor_01.dungeon.ron:
- North: 14, West: 22, South: 6, East: 6 = 48 wall entities
- Floor + ceiling: 36 + 36 = 72
- Grand total: 120 — matches integration test assertion exactly

**Wall transform math:** All four direction cases verified correct against CELL_SIZE=2.0, CELL_HEIGHT=3.0. Walls flush with floor (y=0) and ceiling (y=3.0).

**OneWay rendering:** (2,3).east=OneWay → no geometry (correct). (3,3).west=Solid → rendered as west-of-cell (correct). The blocking side is visible; the passable side has no plate.

**Rust 2024 let-chain rewrite:** Semantically equivalent to nested if-let. No logic change.

**GlobalAmbientLight lifecycle:** Set to brightness 50.0 on `OnEnter(Dungeon)`, restored to `default()` on `OnExit(Dungeon)`. Clean state machine pattern — future states start with the framework default.

**PointLight despawn:** Grandchild of `PlayerParty` via `DungeonCamera` → recursively despawned with parent on `OnExit`. No `DungeonGeometry` marker needed or added — correct.

**`#[cfg(feature = "dev")] init_resource::<ButtonInput<KeyCode>>()`** present in `tests/dungeon_geometry.rs` — the sixth-feature gotcha is properly anticipated.

**Test coverage:** 9 new unit tests (pure-function `wall_transform` + `wall_material` for all variants) + 3 new integration tests (open 3×3, walled 3×3, OnExit cleanup + ambient restore) + 1 app-level integration test loading the real floor_01 asset.

**Deferred (manual visual verification):** Torchlight atmosphere quality (oppressive feel, fade distance, warm color) is intentionally user-verified; not in scope for automated review.
