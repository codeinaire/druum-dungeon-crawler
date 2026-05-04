# Pipeline State

**Task:** Drive the full pipeline (research → plan) for Feature #8: 3D Dungeon Renderer (Option B) from the dungeon crawler roadmap. Replace the test scene from #7 (`TestSceneMarker` cubes + ground slab + DirectionalLight) with real 3D geometry generated from `DungeonFloor` data: floor + ceiling tiles, walls per `WallMask`, simple lighting, placeholder solid-color materials. Also fix #7's LOW finding (`src/data/dungeon.rs:18` stale doc-comment). PAUSE at plan-approval; parent dispatches implementer manually because `SendMessage` does not actually resume returned agents (confirmed across Features #3, #4, #5, #6, #7).

**Status:** completed
**Last Completed Step:** 5

## Artifacts

| Step | Description | Artifact                                 |
| ---- | ----------- | ---------------------------------------- |
| 1    | Research    | /Users/nousunio/Repos/Learnings/claude-code/druum/project/research/20260503-220000-feature-8-3d-dungeon-renderer.md |
| 2    | Plan        | /Users/nousunio/Repos/Learnings/claude-code/druum/project/plans/20260503-223000-feature-8-3d-dungeon-renderer.md |
| 3    | Implement   | /Users/nousunio/Repos/Learnings/claude-code/druum/project/implemented/20260504-023000-feature-8-3d-dungeon-renderer.md |
| 4    | Ship        | https://github.com/codeinaire/druum-dungeon-crawler/pull/8 (branch `8-3d-dungeon-renderer` stacked on `7-grid-movement-first-person-camera`, commit `adaeb20`) |
| 5    | Code Review | /Users/nousunio/Repos/Learnings/claude-code/druum/project/reviews/20260503-000000-feature-8-3d-dungeon-renderer.md (verdict: APPROVE, 1 LOW) |

## Implementation Notes (Step 3)

User approved the plan with one explicit override on 2026-05-04: lighting style switched from Etrian-Odyssey scene-wide to **classic Wizardry torchlight** (player-attached PointLight, dark perimeter, oppressive atmosphere). Implementer applied the override:
- No DirectionalLight spawned
- `GlobalAmbientLight { brightness: 50.0 }` (very dim)
- `PointLight` (warm yellow-orange, intensity 1500, range 6.0, no shadows) added as grandchild of `PlayerParty` inside `DungeonCamera` via nested `children!` macro — follows the player automatically, despawns recursively on OnExit.

All other planner defaults retained (per-edge canonical iteration, per-tile Cuboid floor/ceiling slabs, CELL_HEIGHT=3.0, WALL_THICKNESS=0.05, OneWay rendered as Solid on blocking side only, single-file mod.rs, top-level `DungeonGeometry`-tagged entities for cleanup, wall color palette as planned).

**LOC: +358 net** in `src/plugins/dungeon/mod.rs` (997 → 1355). Only 7% over plan estimate — much tighter than #7's 25% overrun. Plus 159 LOC new integration test file `tests/dungeon_geometry.rs` and 1-character doc fix at `src/data/dungeon.rs:18`.

**Entity count for floor_01:** 120 `DungeonGeometry`-tagged (36 floor + 36 ceiling + 48 walls). PointLight is child of camera, not tagged DungeonGeometry — counts excluded from the geometry tests.

Verification: all 7 commands passed with zero warnings. **61 lib + 3 integration default / 62 lib + 3 integration with --features dev**. **Cargo.toml + Cargo.lock byte-unchanged.**

**Two minor deviations from the plan:**
1. Clippy `collapsible_if` triggered Rust 2024 let-chain syntax — nested `if guard { if let Some(x) = ... }` collapsed to `if guard && let Some(x) = ...`.
2. `spawn_party_and_camera` was modified to add the PointLight child (plan said it would stay untouched) — required by the user's torchlight override.

**Manual visual smoke: NOT run by implementer.** Deferred to user verification — the lighting override means they'll specifically want to see the torchlight effect.

## Research Summary (Step 1)

Research is **HIGH-confidence** with all Bevy 0.18 mesh/material/lighting APIs verified on-disk in `~/.cargo/registry/src/index.crates.io-*/bevy_*-0.18.1/`. Entity-count math independently re-verified by the planner against `assets/dungeons/floor_01.dungeon.ron`.

### Recommendations on the 7 architectural questions

| # | Question | Recommendation | Confidence |
|---|----------|----------------|------------|
| 1 | Per-wall entity vs merged mesh | **Per-wall entity. ~121 entities for floor_01 — trivially OK; no mesh-merging crate.** | HIGH |
| 2 | Walls per cell vs walls per edge | **Per-edge: render north + west of every cell, plus south on bottom row, east on right column.** | HIGH |
| 3 | Floor + ceiling: combined slab vs per-tile | **Per-tile (one Cuboid each per cell). Bevy auto-batches identical mesh+material into one draw call.** | HIGH |
| 4 | Cell height (world_y units) | **CELL_HEIGHT = 3.0** (matches master research and `dungeon/mod.rs:47` doc-comment). | HIGH |
| 5 | Player-attached light vs scene-wide directional | **Scene-wide DirectionalLight + GlobalAmbientLight resource override.** Etrian Odyssey-style. | MEDIUM |
| 6 | Wall thickness | **WALL_THICKNESS = 0.05** (non-zero to avoid z-fighting; invisible in v1). | MEDIUM |
| 7 | OneWay walls visual asymmetry | **Render only the side stored as `Solid` (blocking side); passable side gets no geometry. Falls out for free from per-edge iteration.** | MEDIUM |

### Key Bevy 0.18 verified facts

1. **`Cuboid::new(x, y, z)` takes FULL lengths, not half-extents.** Verified at `bevy_math-0.18.1/src/primitives/dim3.rs:691-712`.
2. **`Plane3d` is single-sided** — recommendation is `Cuboid` slabs for floor + ceiling (consistent with #7's ground slab; sidesteps PI rotation pitfalls).
3. **`AmbientLight` is a per-camera Component in 0.18** (`#[require(Camera)]`). Scene-wide ambient is the **`GlobalAmbientLight` resource**. Master research is wrong on this.
4. **`Mesh3d(handle)` and `MeshMaterial3d(handle)` are tuple-struct components.**
5. **`commands.entity(e).despawn()` is recursive** in 0.18 — no separate `despawn_recursive()`.
6. **`materials.add(Color::srgb(...))` works directly** via `From<Color> for StandardMaterial`.
7. **All needed primitives in `bevy::prelude::*`** via existing `features = ["3d"]` at `Cargo.toml:11`.

### Items the researcher flagged for the planner

- Wall color palette is subjective (visual smoke iterates).
- DirectionalLight position/orientation is subjective.
- `GlobalAmbientLight` restoration on OnExit policy across future states (#18 Town).

## Plan Summary (Step 2)

**Plan adopts all 7 research recommendations** with full architectural rationale and resolves 6 additional micro-decisions surfaced during planning. Plan structure: Goal, Approach (12 architectural decisions), Critical (15 pitfalls), 13 commit-ordered Steps, Security, 13 Open Questions all RESOLVED, Implementation Discoveries (template), Verification (16 items with both automated + manual), conservative-realistic LOC estimate.

### 13 commit-ordered steps

1. Add new constants (`CELL_HEIGHT = 3.0`, `WALL_THICKNESS = 0.05`, `FLOOR_THICKNESS = 0.05`) and `DungeonGeometry` marker component.
2. Delete `TestSceneMarker`, `spawn_test_scene` function, and the test-scene despawn branch.
3. Add `wall_transform` pure helper + 5 unit tests (one per direction + corner cell).
4. Add `wall_material` pure helper + 2 unit tests covering all 7 WallType variants.
5. Fix `data/dungeon.rs:18` doc-comment (`-grid_y` → `+grid_y` — single-token edit, the only allowed mod to that frozen file).
6. Stage `spawn_dungeon_geometry` skeleton: signature, asset guards, mesh/material caches.
7. Implement per-cell iteration loop + DirectionalLight spawn + GlobalAmbientLight resource insert.
8. Wire `DungeonGeometry` cleanup into `despawn_dungeon_entities` + restore `GlobalAmbientLight::default()` on OnExit.
9. Add `make_walled_floor` test helper + 3 App-level tests (open 3×3 = 19 entities, walled 3×3 = 43 entities, on-exit despawns all).
10. Add new `tests/dungeon_geometry.rs` integration test mirroring `tests/dungeon_movement.rs` exactly; load real `floor_01.dungeon.ron`, assert exact entity count of 121.
11. Manifest byte-diff guard (`git diff Cargo.toml Cargo.lock` must be EMPTY).
12. Run the full 7-command verification matrix (zero warnings).
13. Manual visual smoke test (`cargo run --features dev`, F9 to Dungeon, walk floor_01 with WASDQE, verify walls/doors/OneWay asymmetry render correctly).

### Architectural decisions baked in

- **Single-file: keep everything in `src/plugins/dungeon/mod.rs`.** No `render.rs` submodule. File grows to ~1200 LOC, comparable to large Bevy plugin files. Mirrors #7's single-file decision.
- **Per-edge canonical iteration:** render `north + west` of every cell, plus `south` on bottom row, `east` on right column. Each shared interior edge is iterated exactly once. `OneWay` falls out for free: `wall_material` returns `None` for `Open | OneWay`.
- **Two cached wall meshes** (`wall_mesh_ns` and `wall_mesh_ew`) with native orientations — simpler than rotating a single mesh by `Quat::from_rotation_y(FRAC_PI_2)`.
- **Top-level entities, no parent-child hierarchy** — adds complexity for no win at this scale. Single `DungeonGeometry` marker on every spawned mesh + light entity.
- **`GlobalAmbientLight::default()` restored on OnExit** (the LightPlugin default). Convention: every state owns its ambient setting on entry; OnExit resets to default. Town (#18) overrides on its own OnEnter.
- **Tests reuse the Layer 2 pattern** from #7. New tests need same `Assets<Mesh>` + `Assets<StandardMaterial>` registrations the test app already provides. Integration test mirrors `tests/dungeon_movement.rs` line-for-line, including the `#[cfg(feature = "dev")] init_resource::<ButtonInput<KeyCode>>()` 5th-feature gotcha.
- **Entity count = 121 for `floor_01`** (planner independently recounted: 14 north + 22 west + 6 south + 6 east = 48 walls + 36 floor + 36 ceiling + 1 light = 121).

### LOC + dep impact

- **~+205 production LOC** + **~+130 test LOC** = **~+335 LOC net**.
- File `src/plugins/dungeon/mod.rs`: ~997 → ~1175 LOC (Step 2 deletes -75, Steps 1/3/4/6/7/8/9 add +280).
- New file: `tests/dungeon_geometry.rs` (~+130 LOC).
- One-character change: `src/data/dungeon.rs:18`.
- **Cargo.toml + Cargo.lock byte-unchanged** — Δ deps = 0.
- If implementer comes in 25% over (matching #7), expect ~+410 LOC total. Flagged for **Implementation Discoveries**.

### All 13 plan-level open questions RESOLVED (NOT escalated as Category C)

7 from research (above) + 6 surfaced during planning:

8. Wall color palette → cool grey for Solid/SecretWall/Illusory, brown for Door, dark red for LockedDoor; warm dark stone floor, cool dark stone ceiling. Subjective; iterate via Step 13 visual smoke.
9. DirectionalLight position/orientation → `(0.0, CELL_HEIGHT * 4.0, 0.0)` looking at `(0.5, -1.0, 0.3)`. Tunable via Step 13.
10. GlobalAmbientLight restoration on OnExit → `commands.insert_resource(GlobalAmbientLight::default())` (LightPlugin default).
11. Cell-feature visual overrides for v1 → NO; that's Feature #13.
12. Module split → NO submodule for #8; #9 may extract.
13. Sequential vs parallel OnEnter system run → parallel; no resource conflict requires explicit ordering.

### Critical risks the planner surfaced (NEW — beyond research)

1. **`AmbientLight` Component vs `GlobalAmbientLight` Resource confusion** — master research's spawn snippet would not compile in 0.18. Plan §Critical and §Approach #8 spell out the correct API.
2. **`Cuboid::new` is full-length, not half-extents** — misreading would double-and-overlap all geometry. Plan §Critical highlights this with the `bevy_math` line citation.
3. **`Plane3d` single-sided trap** — the planner explicitly recommends NOT using `Plane3d`, even though master research's example used it. Use `Cuboid` slabs for floor + ceiling.
4. **5th-feature `init_resource::<ButtonInput<KeyCode>>()` test gotcha** — needs to be in the new `tests/dungeon_geometry.rs` integration file too. Plan Step 10 makes this explicit, line-for-line copy from `tests/dungeon_movement.rs:75-76`.
5. **Manual visual smoke is mandatory** — not just nice-to-have. Specific verification: walls render where the data says, no z-fighting, doors visually distinct, OneWay asymmetry verifiable from cells (2,3) ↔ (3,3).

### Cleanest-possible-ship signal

`Cargo.toml + Cargo.lock` Δ = 0. If `git diff` shows any change, STOP.

## User Decisions

[awaiting plan-approval — see final report from orchestrator]
