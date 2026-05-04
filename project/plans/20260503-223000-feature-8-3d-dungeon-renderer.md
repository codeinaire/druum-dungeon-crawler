# Plan: 3D Dungeon Renderer — Feature #8

**Date:** 2026-05-03
**Status:** Complete
**Research:** ../research/20260503-220000-feature-8-3d-dungeon-renderer.md
**Depends on:** 20260501-230000-feature-4-dungeon-grid-data-model.md, 20260503-130000-feature-7-grid-movement-first-person-camera.md

## Goal

Replace Feature #7's placeholder test scene (3 colored cubes + 40×0.1×40 grey ground slab + DirectionalLight, all `TestSceneMarker`-tagged) with real 3D dungeon geometry generated from `Res<Assets<DungeonFloor>>`: per-cell floor + ceiling slabs and per-edge wall plates rendered from each cell's `WallMask`, lit by a scene-wide `DirectionalLight` with a low `GlobalAmbientLight` resource override. After this PR, `cargo run --features dev` shows the player walking through the actual `floor_01` layout — the dungeon now looks like a dungeon. Net delivery: ~+200 to +350 LOC in `src/plugins/dungeon/mod.rs`, +1 new integration test file, **zero new Cargo dependencies**, all 7 verification commands pass with zero warnings.

## Approach

The research (HIGH confidence on all 7 architectural questions plus all Bevy 0.18 API shapes verified on-disk) recommends a tightly-scoped extension of `src/plugins/dungeon/mod.rs` — same file, same `OnEnter(GameState::Dungeon)` schedule, same `OnExit` cleanup. Mirror Feature #7's structure precisely: a pure helper `wall_transform(x, y, dir) -> Transform` reusing the established `world_x = grid_x * CELL_SIZE`, `world_z = +grid_y * CELL_SIZE` convention (do NOT redeclare); a system function with `Option<Res<DungeonAssets>>` and `Res<Assets<DungeonFloor>>` parameters mirroring the asset-tolerant `spawn_party_and_camera` precedent; a new `DungeonGeometry` marker component for cleanup. The architectural decisions made here:

1. **Module layout: keep everything in `src/plugins/dungeon/mod.rs`.** No `render.rs` submodule. The file currently sits at 997 LOC; this feature adds ~+200-350 LOC net, putting it around 1200-1350 LOC — comparable to large Bevy plugin files in the wild and still single-screenful for navigation. Submodule extraction can land in #9 (atmosphere) when there's a forced split (lighting + texture + fog domain). Mirrors the same single-file decision Feature #7 made. The seam this plan creates — a `DungeonGeometry` marker, a `spawn_dungeon_geometry` system, a `wall_transform` pure helper — is clean enough to extract later without churn.

2. **Per-edge iteration: for each cell, render only its `north` and `west` walls; render `south` only on bottom-row cells (y == height - 1) and `east` only on right-column cells (x == width - 1).** This is the deduplication algorithm. Each shared interior wall is stored twice (cell A's east == cell B's west); naively rendering all 4 walls of every cell would double-render every interior wall (z-fighting + 2× geometry cost). The "north + west of every cell, plus south/east of edge cells" rule guarantees each edge is iterated exactly once. `OneWay` walls fall out for free: render-renderable variants and skip `Open | OneWay`, so the OneWay side gets nothing while the paired `Solid` side renders normally.

3. **Per-tile floor + ceiling slabs: one `Cuboid::new(CELL_SIZE, FLOOR_THICKNESS, CELL_SIZE)` per cell for floor + one for ceiling.** Per-tile (not one big merged slab) so Feature #13's per-cell visual variation (spinner icon, dark zone tint, anti-magic zone tint) is trivial to add later — swap the material handle for that cell, no slab subdivision. All floor tiles share one cached `Handle<Mesh>` and one cached `Handle<StandardMaterial>` so Bevy's clustered renderer batches them into a single draw call. Use `Cuboid` (not `Plane3d`) — `Plane3d` mesh builder writes single-sided geometry (verified at `bevy_mesh-0.18.1/src/primitives/dim3/plane.rs:122-132`); a thin `Cuboid` slab is always visible from any angle and matches Feature #7's ground slab precedent.

4. **Walls: thin `Cuboid` plates centered on the cell edge.** Two cached mesh handles: `wall_mesh_ns` of size `(CELL_SIZE, CELL_HEIGHT, WALL_THICKNESS)` for north/south walls (extends along world X, thin along world Z), and `wall_mesh_ew` of size `(WALL_THICKNESS, CELL_HEIGHT, CELL_SIZE)` for east/west walls (extends along world Z, thin along world X). Two natively-oriented meshes is simpler than rotating one mesh by `Quat::from_rotation_y(FRAC_PI_2)` and avoids any normal/lighting subtlety; the cost (one extra mesh handle) is negligible.

5. **Constants: `CELL_HEIGHT = 3.0`, `WALL_THICKNESS = 0.05`, `FLOOR_THICKNESS = 0.05` as new `pub const` in `src/plugins/dungeon/mod.rs`.** Reuse existing `CELL_SIZE = 2.0` from Feature #7 — do NOT redeclare. 3.0 wall height matches the `EYE_HEIGHT = 0.7` doc-comment from #7 (line 51-53 of `dungeon/mod.rs`), gives a 1.5× corridor-width-to-wall-height ratio (claustrophobic Wizardry feel), and matches master research §Pattern 6. 0.05 thickness is invisible at typical viewing distances and avoids z-fighting with adjacent floor/ceiling slabs (true 0.0 thickness produces zero-volume cuboids with undefined normals).

6. **WallType → material is a pure `fn wall_material(...)` lookup.** Inputs: 3 cached `&Handle<StandardMaterial>` (solid, door, locked) + a `WallType`. Output: `Option<Handle<StandardMaterial>>`. Returns `None` for `Open | OneWay` (no geometry). Returns the solid handle for `Solid | SecretWall | Illusory` (player can't visually tell — reveal is #13). Returns the door handle for `Door`. Returns the locked handle for `LockedDoor`. Extracted as a free `fn` (not a closure) so the test module can verify the mapping without an `App`. v1 palette: Solid grey `(0.50, 0.50, 0.55)`, Door brown `(0.45, 0.30, 0.15)`, LockedDoor dark red `(0.55, 0.20, 0.15)`. Floor `(0.30, 0.28, 0.25)` (warm dark stone). Ceiling `(0.20, 0.20, 0.22)` (cool dark stone). Subjective; iterate via visual smoke.

7. **Lighting: scene-wide `DirectionalLight` (no shadows, marked `DungeonGeometry` for cleanup) + low `GlobalAmbientLight` resource override on OnEnter, restored to default on OnExit.** Etrian Odyssey style. `illuminance: 5000.0`, `shadows_enabled: false` (shadows are #9). DirectionalLight at `Transform::from_xyz(0.0, CELL_HEIGHT * 4.0, 0.0).looking_at(Vec3::new(0.5, -1.0, 0.3), Vec3::Y)` for slight off-axis differential corner shading. Ambient `Color::srgb(0.30, 0.32, 0.40)` × `brightness: 100.0` so geometry is clearly readable in v1 flat-color (without textures, pure ambient + pure directional looks acceptable; #9 will dim ambient and add atmosphere).

8. **GlobalAmbientLight restoration: `commands.insert_resource(GlobalAmbientLight::default())` on OnExit.** Master research's `commands.insert_resource(AmbientLight { ... })` is wrong for Bevy 0.18 — `AmbientLight` is a per-camera Component (`#[require(Camera)]`), and `GlobalAmbientLight` is the resource (verified at `bevy_light-0.18.1/src/ambient_light.rs:9-89`, used at `bevy-0.18.1/examples/3d/lighting.rs:122-127`). On OnExit we explicitly write the `LightPlugin::default()` value back (color: white, brightness: 80.0). When Town (#18) lands, that state's OnEnter can override again — the convention is "every state owns its ambient setting on entry; OnExit resets to default." Document this on the OnExit handler.

9. **`DungeonGeometry` marker component on every spawned mesh + light entity.** Top-level entities (no parent-child hierarchy needed — that adds complexity for no win at this scale). The OnExit despawn query iterates `Query<Entity, With<DungeonGeometry>>` and despawns each. `commands.entity(e).despawn()` is recursive in 0.18 (verified at `bevy_ecs-0.18.1/src/system/commands/entity_command.rs:242-249`), so even if a future feature attaches children to a `DungeonGeometry` entity, cleanup still works.

10. **Clean removal of `TestSceneMarker` infrastructure.** `pub struct TestSceneMarker` (line 140-141), `fn spawn_test_scene` (lines 284-348), the `, spawn_test_scene` from the OnEnter tuple (line 174), the `test_scene: Query<Entity, With<TestSceneMarker>>` parameter and despawn loop (lines 356, 361-363) — all deleted. The `TODO(Feature #8)` comment at line 283 is also removed (it ceases to exist with the function). `PlayerParty` despawn-on-OnExit stays exactly as it was.

11. **Tests follow Layer 2 pattern from Feature #7.** Reuse `make_test_app()` (`dungeon/mod.rs:601-630`) and `make_open_floor` (line 635) helpers as-is; the new spawn system needs the same `Assets<Mesh>` + `Assets<StandardMaterial>` registrations the test app already provides for the deleted `spawn_test_scene`. New unit tests for `wall_transform` (4 tests, one per direction, plus a corner case) and `wall_material` (2 tests, presence/absence). New App-level unit tests for entity counts on a 3×3 open floor (19 entities) and a 3×3 fully-walled floor (43 entities). New integration test in `tests/dungeon_geometry.rs` mirroring `tests/dungeon_movement.rs` exactly: load real `floor_01.dungeon.ron` via `TestState`, assert exact entity count of 121.

12. **Doc-comment fix on `src/data/dungeon.rs:18`.** Single-token edit: `world_z = -grid_y * cell_size` → `world_z = +grid_y * cell_size`. This is the LOW finding from Feature #7's review (which deferred the fix to #8). It is a text-only change with zero functional effect — but per `project_druum_dungeon_grid.md`, `data/dungeon.rs` is treated as frozen. The exception is granted by Feature #7's review note ("Recommend fixing the doc-comment in Feature #8") and the user's explicit task directive. Keep the rest of the file byte-unchanged.

## Critical

- **Zero new Cargo dependencies. `Cargo.toml` and `Cargo.lock` MUST be byte-unchanged.** All required APIs (`Cuboid::new`, `StandardMaterial`, `Mesh3d`, `MeshMaterial3d`, `DirectionalLight`, `GlobalAmbientLight`, `Color::srgb`, `Quat::from_rotation_y`, `Transform::from_xyz`, `Vec3::new`, `commands.entity(e).despawn()`) are in `bevy::prelude::*` via the existing `features = ["3d"]` declaration at `Cargo.toml:11`. **Do NOT add a mesh-merging crate, a tween crate, or any other dependency.** If `git diff Cargo.toml Cargo.lock` after this feature shows any change, STOP. Final Verification step explicitly runs this diff.

- **Bevy 0.18 spawn syntax: `(Mesh3d(handle), MeshMaterial3d(handle), Transform::...)` as a component tuple — NOT bundles.** `PbrBundle` does NOT exist in 0.18 (the bundle naming is gone from the framework). `Mesh3d` and `MeshMaterial3d` are tuple-struct components: `Mesh3d(pub Handle<Mesh>)` and `MeshMaterial3d<M: Material>(pub Handle<M>)` (verified at `bevy_mesh-0.18.1/src/components.rs:95-98` and `bevy_pbr-0.18.1/src/mesh_material.rs:39-41`). Druum's existing `dungeon/mod.rs:295,305,317,329` uses the tuple-constructor pattern — keep that pattern.

- **`Cuboid::new(x, y, z)` takes FULL lengths, not half-extents.** Verified at `bevy_math-0.18.1/src/primitives/dim3.rs:691-712`. A floor slab of `Cuboid::new(2.0, 0.05, 2.0)` is 2.0 units wide / 0.05 thick / 2.0 deep — exactly one cell on the floor plane. Misreading this as half-extents would produce slabs at 4.0 × 0.10 × 4.0, doubling and overlapping all geometry. (`Cuboid` stores `half_size: Vec3` internally, but the constructor takes full lengths.)

- **Do NOT use `Plane3d` for floor or ceiling.** Bevy 0.18's `PlaneMeshBuilder::build` writes single-sided geometry (verified at `bevy_mesh-0.18.1/src/primitives/dim3/plane.rs:122-132`); a `Plane3d` floor would be invisible from the underside, and the ceiling would need a `Quat::from_rotation_x(PI)` flip with subtle UV/normal pitfalls. Use thin `Cuboid` slabs for both — consistent with Feature #7's ground slab and double-sided automatically.

- **Bevy 0.18 `AmbientLight` vs `GlobalAmbientLight`.** `AmbientLight` is a per-camera Component (`#[require(Camera)]` at `bevy_light-0.18.1/src/ambient_light.rs:9-39`). The scene-wide ambient is the resource `GlobalAmbientLight` (same file, lines 41-89). Master research's `commands.insert_resource(AmbientLight { ... })` will not compile in 0.18. Use `commands.insert_resource(GlobalAmbientLight { ... })` instead.

- **`commands.entity(e).despawn()` is recursive in Bevy 0.18.** No separate `despawn_recursive()` API — calling `despawn()` walks descendants automatically (verified at `bevy_ecs-0.18.1/src/system/commands/entity_command.rs:242-249`). The OnExit despawn loop for `Query<Entity, With<DungeonGeometry>>` correctly cleans up everything.

- **Don't modify the freeze list:** `src/plugins/state/mod.rs` (#2), `src/plugins/loading/mod.rs` (#3), `src/plugins/audio/{mod,bgm,sfx}.rs` (#6), `src/plugins/input/mod.rs` (#5). The MOVEMENT systems in `src/plugins/dungeon/mod.rs` from Feature #7 (`spawn_party_and_camera`, `handle_dungeon_input`, `animate_movement`, `MovementAnimation`, `MovedEvent`, `PlayerParty`, `DungeonCamera`, `GridPosition`, `Facing`, `grid_to_world`, `facing_to_quat`, all four tunable consts) MUST NOT change semantically. Only the test scene (`TestSceneMarker` + `spawn_test_scene` + the test-scene despawn branch) is deleted; the new geometry system is added alongside the existing movement systems.

- **`src/data/dungeon.rs` doc-comment fix is the ONLY allowed modification to that file.** Single-token edit at line 18: `-grid_y` → `+grid_y`. No other lines changed. Verified that `data/dungeon.rs` doesn't otherwise need touching for #8: the new system reads through the public `DungeonFloor::can_move`, `floor.walls[][]`, `Direction`, `WallType`, `WallMask` API, all already exported. Reuse — do NOT redeclare any of these grid types in `dungeon/mod.rs`.

- **Reuse `CELL_SIZE`, `EYE_HEIGHT`, `MOVE_DURATION_SECS`, `TURN_DURATION_SECS` from Feature #7.** Add `CELL_HEIGHT`, `WALL_THICKNESS`, `FLOOR_THICKNESS` as new `pub const` constants alongside the existing four — DO NOT redeclare any existing constant.

- **Symmetric `#[cfg(feature = "dev")]` gating.** Feature #8 should not need any dev-only code path (no debug-render-toggle, no per-cell debug overlay — those are #25 polish and #10 auto-map respectively). If a future contributor adds one, the function definition AND the `add_systems` call MUST both be cfg-gated. Symmetric gating is the third-feature precedent (#2 / #5 / #6 / #7 each had to fix this once). Mirror the pattern in `project/resources/20260501-102842-dev-feature-pattern.md` if such a hook lands.

- **Test helper requires `#[cfg(feature = "dev")] init_resource::<ButtonInput<KeyCode>>()` if `StatePlugin` is loaded.** The fifth-feature gotcha is now confirmed across #2/#5/#6/#7 — `StatePlugin::build` registers `cycle_game_state_on_f9` under `#[cfg(feature = "dev")]`, and Bevy 0.18 validates every registered system's parameters at every `app.update()` call. Without `ButtonInput<KeyCode>` present, `cargo test --features dev` panics. The existing `make_test_app()` in `dungeon/mod.rs:627-628` already does this; the new integration test file `tests/dungeon_geometry.rs` MUST also init this resource (line-for-line copy from `tests/dungeon_movement.rs:75-76`).

- **No `rand` calls.** Geometry is deterministic. Permadeath in scope for the project. Random per-cell color jitter, randomized wall plate offsets, etc. are all out of scope. Deterministic RNG via `RngSeed` lands in #23.

- **`MovedEvent` derives `Message`, NOT `Event`.** Bevy 0.18 family-rename. Feature #7 already handled this correctly; #8 doesn't add new Messages but if it ever did (e.g., a future `GeometryReadyEvent`), the same `Message` derive + `app.add_message::<...>()` registration applies. `add_event` form will not compile against a `Message`-derived type — caught at first build.

- **All 7 verification commands must pass with ZERO warnings:** `cargo check`, `cargo check --features dev`, `cargo clippy --all-targets -- -D warnings`, `cargo clippy --all-targets --features dev -- -D warnings`, `cargo test`, `cargo test --features dev`, `cargo fmt --check`. `Cargo.toml` and `Cargo.lock` MUST be byte-unchanged.

- **Manual visual smoke test is REQUIRED before declaring done.** The whole point of Feature #8 is "the dungeon now actually looks like a dungeon." Run `cargo run --features dev`, F9-cycle to `GameState::Dungeon`, walk through `floor_01` with WASDQE, and verify (a) walls are visible where they should be from `floor_01.dungeon.ron`, (b) no z-fighting between walls and floor/ceiling, (c) doors visually distinguish from solid walls, (d) the OneWay wall at (2,3) east is invisible from cell (2,3) and visible from cell (3,3) (the player can walk East from (2,3) into (3,3) and see no wall on the way; standing at (3,3) and looking West shows a wall blocking the way back). Record findings in **Implementation Discoveries**.

- **Test entity-count math for `floor_01` is the regression guard.** Research calculates 121 entities total for `floor_01.dungeon.ron`: 36 floor + 36 ceiling + 48 walls + 1 directional light. The implementer MUST re-verify this count against the asset (read `assets/dungeons/floor_01.dungeon.ron` cell-by-cell) before fixing the test number. If the count differs, document the discrepancy in **Implementation Discoveries**, fix the test to match the verified count, and add a comment in the test deriving the math from the asset (so future asset edits surface as test failures with a clear diff).

## Steps

### Step 1: Add new constants and `DungeonGeometry` marker component

Add the new `pub const` tunables (cell height, wall + floor thickness) and the `DungeonGeometry` marker component to `src/plugins/dungeon/mod.rs`. No system changes yet.

- [x] In `src/plugins/dungeon/mod.rs`, in the `// Constants` section (after `TURN_DURATION_SECS` at line 59), add:
  ```rust
  /// Wall height in world units. With `CELL_SIZE = 2.0` this gives a 1.5× wall-to-corridor
  /// ratio — claustrophobic Wizardry/Etrian feel. Tunable in Feature #25 polish.
  pub const CELL_HEIGHT: f32 = 3.0;

  /// Wall plate thickness in world units. Non-zero to avoid degenerate-volume Cuboids
  /// (zero thickness produces undefined normals); 0.05 is invisible at typical viewing
  /// distances and avoids z-fighting with adjacent floor/ceiling slabs.
  pub const WALL_THICKNESS: f32 = 0.05;

  /// Floor + ceiling slab thickness. Same value as WALL_THICKNESS for visual consistency.
  pub const FLOOR_THICKNESS: f32 = 0.05;
  ```
- [ ] In the `// Components` section (after the existing `Facing` component, before `MovementAnimation` — or add at the end of the components section before `// Messages`), add:
  ```rust
  /// Marker on every entity spawned by `spawn_dungeon_geometry`: floor tiles, ceiling tiles,
  /// wall plates, and the scene-wide directional light. The OnExit cleanup walks this query
  /// to despawn all dungeon geometry on state transition out of `GameState::Dungeon`.
  /// Replaces the Feature #7 `TestSceneMarker` (deleted in Step 2).
  #[derive(Component, Debug, Clone, Copy)]
  pub struct DungeonGeometry;
  ```
- [ ] Verify: `cargo check` and `cargo check --features dev` — expect zero errors and zero warnings. The new symbols are not yet referenced; nothing should break.

**Done state:** Three new public constants and the `DungeonGeometry` component compile cleanly. `TestSceneMarker` still exists for now (deleted in Step 2). Both `cargo check` invocations zero-warning.

### Step 2: Delete `TestSceneMarker`, `spawn_test_scene`, and the test-scene despawn branch

Strip out Feature #7's placeholder scaffolding. The compile will break temporarily because the `OnEnter` tuple at line 174 still references `spawn_test_scene` — that registration is also deleted in this step. Once this step is done, the dungeon shows nothing (movement still works; the player is in a black void) until Step 7 wires in real geometry.

- [ ] In `src/plugins/dungeon/mod.rs`, delete the `TestSceneMarker` component definition (lines ~137-141 in the post-Step-1 file):
  ```rust
  /// Marker tag on every entity spawned by `spawn_test_scene`.
  /// Feature #8 deletes the entire `spawn_test_scene` function, this component,
  /// and the `despawn_dungeon_entities` query for it in one PR.
  #[derive(Component, Debug, Clone, Copy)]
  pub struct TestSceneMarker;
  ```
- [ ] In `DungeonPlugin::build`, change the `OnEnter(GameState::Dungeon)` system tuple (line ~173-175) from:
  ```rust
  .add_systems(
      OnEnter(GameState::Dungeon),
      (spawn_party_and_camera, spawn_test_scene),
  )
  ```
  to:
  ```rust
  .add_systems(
      OnEnter(GameState::Dungeon),
      (spawn_party_and_camera, spawn_dungeon_geometry),
  )
  ```
  (The new system `spawn_dungeon_geometry` is defined in Step 7; the build will fail until then. Acceptable — Steps 3–7 are sequential and the working tree is broken in between.)
- [ ] Delete the entire `fn spawn_test_scene` function (lines ~272-348, including its `// `OnEnter(GameState::Dungeon)` — spawn placeholder ...` doc comment header and the `TODO(Feature #8): delete this entire function` line).
- [ ] In `fn despawn_dungeon_entities` (lines ~353-365 of the Feature #7 file), delete the `test_scene` parameter and its for-loop:
  - Remove `test_scene: Query<Entity, With<TestSceneMarker>>,` from the parameter list.
  - Remove the `for e in &test_scene { commands.entity(e).despawn(); }` block.
  - Update the `info!` log message from `"Despawned PlayerParty + test scene entities on OnExit(Dungeon)"` to `"Despawned PlayerParty + dungeon geometry on OnExit(Dungeon)"` (the new query will be added in Step 7).
- [ ] Verify (intentional partial state): `cargo check` SHOULD fail at this step with "cannot find function `spawn_dungeon_geometry`" and any other knock-on errors from the deletion. This is expected — Step 7 wires the new system. Do NOT push the working tree at this checkpoint.

**Done state:** `TestSceneMarker`, `spawn_test_scene`, and the test-scene despawn loop are gone. `cargo check` fails on the missing `spawn_dungeon_geometry` reference (acceptable). All other Feature #7 movement infrastructure (`spawn_party_and_camera`, `handle_dungeon_input`, `animate_movement`, `MovementAnimation`, etc.) is untouched.

### Step 3: Add `wall_transform` pure helper + unit tests

A pure function returning the world-space `Transform` for a wall plate on a given cell's given face. No Bevy app needed; testable as plain Rust.

- [ ] In `src/plugins/dungeon/mod.rs`, in the `// Pure helpers` section (after `facing_to_quat` at line 222), add:
  ```rust
  /// Returns the world-space `Transform` for a wall plate on the given cell's given face.
  ///
  /// Cell `(grid_x, grid_y)` has center at world `(grid_x * CELL_SIZE, 0.0, grid_y * CELL_SIZE)`
  /// (per `grid_to_world`'s convention; `world_y = 0.0` is floor level). Each wall plate is
  /// positioned at the cell's edge (offset ±CELL_SIZE / 2 in world X or Z), centered vertically
  /// at `world_y = CELL_HEIGHT / 2`.
  ///
  /// Wall meshes are pre-oriented (NS = X-extending, EW = Z-extending), so all four directions
  /// share `Quat::IDENTITY` rotation; the caller picks the right mesh handle for the direction.
  fn wall_transform(grid_x: u32, grid_y: u32, dir: Direction) -> Transform {
      let cx = grid_x as f32 * CELL_SIZE;
      let cz = grid_y as f32 * CELL_SIZE;
      let cy = CELL_HEIGHT / 2.0;
      let half = CELL_SIZE / 2.0;
      let translation = match dir {
          Direction::North => Vec3::new(cx, cy, cz - half),
          Direction::South => Vec3::new(cx, cy, cz + half),
          Direction::East  => Vec3::new(cx + half, cy, cz),
          Direction::West  => Vec3::new(cx - half, cy, cz),
      };
      Transform::from_translation(translation)
  }
  ```
- [ ] In the existing `#[cfg(test)] mod tests { ... }` block (after the existing `facing_to_quat_*` tests at lines ~554-587, before the `// Test app helpers` section), add:
  ```rust
  #[test]
  fn wall_transform_north_on_cell_3_4() {
      // Cell (3, 4): center world (6.0, 0.0, 8.0). North wall at z = 8.0 - 1.0 = 7.0.
      let t = wall_transform(3, 4, Direction::North);
      assert!(t.translation.abs_diff_eq(Vec3::new(6.0, 1.5, 7.0), 1e-6));
      assert!(t.rotation.abs_diff_eq(Quat::IDENTITY, 1e-6));
  }

  #[test]
  fn wall_transform_south_on_cell_3_4() {
      let t = wall_transform(3, 4, Direction::South);
      assert!(t.translation.abs_diff_eq(Vec3::new(6.0, 1.5, 9.0), 1e-6));
  }

  #[test]
  fn wall_transform_east_on_cell_3_4() {
      let t = wall_transform(3, 4, Direction::East);
      assert!(t.translation.abs_diff_eq(Vec3::new(7.0, 1.5, 8.0), 1e-6));
  }

  #[test]
  fn wall_transform_west_on_cell_3_4() {
      let t = wall_transform(3, 4, Direction::West);
      assert!(t.translation.abs_diff_eq(Vec3::new(5.0, 1.5, 8.0), 1e-6));
  }

  #[test]
  fn wall_transform_corner_cell_origin() {
      // Cell (0, 0): negative coords are valid (wall plate is north of the grid origin).
      let t_north = wall_transform(0, 0, Direction::North);
      assert!(t_north.translation.abs_diff_eq(Vec3::new(0.0, 1.5, -1.0), 1e-6));
      let t_west = wall_transform(0, 0, Direction::West);
      assert!(t_west.translation.abs_diff_eq(Vec3::new(-1.0, 1.5, 0.0), 1e-6));
  }
  ```
- [ ] Verify (still in partial-broken state from Step 2): `cargo test --lib plugins::dungeon::tests::wall_transform` — expect 5 passing tests (the rest of the test module may not compile until Step 7 wires `spawn_dungeon_geometry`; that's fine). If the partial-build blocks running these tests, defer the verification to Step 7.

**Done state:** `wall_transform` defined; 5 unit tests added. Tests verify corner cells (negative-going wall positions) and one mid-grid cell with all four directions. Translation matches the formulas in research §"Wall position formulas (concrete examples, used in tests)". Rotation is `Quat::IDENTITY` for all directions (the wall mesh choice — NS vs EW — handles orientation, not the transform).

### Step 4: Add `wall_material` pure helper + unit tests

Pure function mapping `WallType` → `Option<Handle<StandardMaterial>>`. Returns `None` for `Open | OneWay`, `Some(handle)` for the rest.

- [ ] In `src/plugins/dungeon/mod.rs`, in the `// Pure helpers` section (after `wall_transform` from Step 3), add:
  ```rust
  /// Maps `WallType` to its rendering material. Returns `None` for variants that have
  /// no geometry (`Open | OneWay`).
  ///
  /// `Solid`, `SecretWall`, and `Illusory` all share the solid-wall material — the player
  /// cannot visually distinguish them in v1; the reveal mechanic is Feature #13's scope.
  fn wall_material(
      wt: WallType,
      solid: &Handle<StandardMaterial>,
      door: &Handle<StandardMaterial>,
      locked: &Handle<StandardMaterial>,
  ) -> Option<Handle<StandardMaterial>> {
      match wt {
          WallType::Open | WallType::OneWay => None,
          WallType::Solid | WallType::SecretWall | WallType::Illusory => Some(solid.clone()),
          WallType::Door => Some(door.clone()),
          WallType::LockedDoor => Some(locked.clone()),
      }
  }
  ```
- [ ] At the top of the file (line ~36, alongside the existing `use crate::data::dungeon::Direction;`), extend the import to include `WallType`:
  ```rust
  use crate::data::dungeon::{Direction, WallType};
  ```
- [ ] In the `#[cfg(test)] mod tests` block (after the `wall_transform_*` tests from Step 3), add:
  ```rust
  #[test]
  fn wall_material_returns_none_for_passable() {
      let solid: Handle<StandardMaterial> = Handle::default();
      let door: Handle<StandardMaterial> = Handle::default();
      let locked: Handle<StandardMaterial> = Handle::default();
      assert!(wall_material(WallType::Open, &solid, &door, &locked).is_none());
      assert!(wall_material(WallType::OneWay, &solid, &door, &locked).is_none());
  }

  #[test]
  fn wall_material_returns_some_for_blocking() {
      let solid: Handle<StandardMaterial> = Handle::default();
      let door: Handle<StandardMaterial> = Handle::default();
      let locked: Handle<StandardMaterial> = Handle::default();
      // Solid, SecretWall, Illusory all share the solid handle.
      assert!(wall_material(WallType::Solid, &solid, &door, &locked).is_some());
      assert!(wall_material(WallType::SecretWall, &solid, &door, &locked).is_some());
      assert!(wall_material(WallType::Illusory, &solid, &door, &locked).is_some());
      // Door + LockedDoor have their own handles.
      assert!(wall_material(WallType::Door, &solid, &door, &locked).is_some());
      assert!(wall_material(WallType::LockedDoor, &solid, &door, &locked).is_some());
  }
  ```
- [ ] Verify (still partial-broken): same caveat as Step 3 — tests may not run until Step 7.

**Done state:** `wall_material` is a pure free `fn` testable without an `App`. 2 new unit tests cover all 7 `WallType` variants (5 in the "blocking" test plus 2 in the "passable" test). `WallType` import added.

### Step 5: Fix the `data/dungeon.rs:18` doc-comment

Single-token text edit. The LOW finding from Feature #7's review (deferred to #8 per the review's recommendation, line 43-44 of `project/reviews/20260503-210000-feature-7-grid-movement-first-person-camera.md`).

- [ ] In `src/data/dungeon.rs`, change line 18 from:
  ```
  //! (`world_z = -grid_y * cell_size`) lives in Feature #8's renderer.
  ```
  to:
  ```
  //! (`world_z = +grid_y * cell_size`) lives in Feature #8's renderer.
  ```
- [ ] Confirm no other lines change in `src/data/dungeon.rs` — `git diff src/data/dungeon.rs` should show exactly one line modified, with one character changed (`-` → `+`).
- [ ] Verify: `cargo fmt --check` (no formatting drift introduced).

**Done state:** `src/data/dungeon.rs:18` doc-comment matches the implementation in `src/plugins/dungeon/mod.rs`. No other change to `data/dungeon.rs`.

### Step 6: Stage helper closures in `spawn_dungeon_geometry` skeleton

Define the system function signature and the cached-handles preamble (mesh handles, material handles, asset-tolerant guards) — but leave the iteration loop body for Step 7. Splitting this out keeps Step 7 focused on the per-cell loop.

- [ ] In `src/plugins/dungeon/mod.rs`, in the `// Systems` section (after the existing `fn animate_movement` at line ~510), add the system function:
  ```rust
  /// `OnEnter(GameState::Dungeon)` — spawn floor + ceiling slabs per cell, wall plates
  /// per renderable edge, and a scene-wide directional light. Also sets `GlobalAmbientLight`
  /// to the dungeon ambient override.
  ///
  /// Asset-tolerant: warns and returns silently if `DungeonAssets` or the floor handle
  /// is not yet loaded (mirrors `spawn_party_and_camera`).
  ///
  /// Iteration rule (per-edge deduplication): for each cell, render `north` and `west`
  /// walls; render `south` ONLY at the bottom edge (y == height - 1), `east` ONLY at the
  /// right edge (x == width - 1). This guarantees each shared wall is rendered exactly once.
  fn spawn_dungeon_geometry(
      mut commands: Commands,
      mut meshes: ResMut<Assets<Mesh>>,
      mut materials: ResMut<Assets<StandardMaterial>>,
      dungeon_assets: Option<Res<DungeonAssets>>,
      floors: Res<Assets<DungeonFloor>>,
  ) {
      // Asset-tolerant: same pattern as spawn_party_and_camera.
      let Some(assets) = dungeon_assets else {
          warn!("DungeonAssets resource not present at OnEnter(Dungeon); geometry spawn deferred");
          return;
      };
      let Some(floor) = floors.get(&assets.floor_01) else {
          warn!("DungeonFloor not yet loaded; geometry spawn deferred");
          return;
      };

      // Cached mesh handles (one per shape, shared across all cells).
      let floor_mesh   = meshes.add(Cuboid::new(CELL_SIZE, FLOOR_THICKNESS, CELL_SIZE));
      let ceiling_mesh = meshes.add(Cuboid::new(CELL_SIZE, FLOOR_THICKNESS, CELL_SIZE));
      // wall_mesh_ns: extends along world X (north/south walls). Thin along Z.
      let wall_mesh_ns = meshes.add(Cuboid::new(CELL_SIZE, CELL_HEIGHT, WALL_THICKNESS));
      // wall_mesh_ew: extends along world Z (east/west walls). Thin along X.
      let wall_mesh_ew = meshes.add(Cuboid::new(WALL_THICKNESS, CELL_HEIGHT, CELL_SIZE));

      // Cached material handles (one per role).
      let floor_mat = materials.add(StandardMaterial {
          base_color: Color::srgb(0.30, 0.28, 0.25),  // warm dark stone
          ..default()
      });
      let ceiling_mat = materials.add(StandardMaterial {
          base_color: Color::srgb(0.20, 0.20, 0.22),  // cool dark stone
          ..default()
      });
      let wall_solid_mat = materials.add(StandardMaterial {
          base_color: Color::srgb(0.50, 0.50, 0.55),  // cool grey — Solid + SecretWall + Illusory
          ..default()
      });
      let wall_door_mat = materials.add(StandardMaterial {
          base_color: Color::srgb(0.45, 0.30, 0.15),  // brown — Door
          ..default()
      });
      let wall_locked_mat = materials.add(StandardMaterial {
          base_color: Color::srgb(0.55, 0.20, 0.15),  // dark red — LockedDoor (visual warning)
          ..default()
      });

      // Per-cell loop lands in Step 7.

      info!(
          "Spawned dungeon geometry for floor '{}' ({}×{})",
          floor.name, floor.width, floor.height
      );
  }
  ```
  Note: `Color::srgb` may already be in scope via `bevy::prelude::*`. The `..default()` call requires `default()` from the same prelude. No new imports needed.
- [ ] Verify: this step's compile state is "function defined but not called" — the existing `OnEnter` registration from Step 2 already references `spawn_dungeon_geometry`, so adding the function should bring `cargo check` green at this point IF Steps 3–4 also compile. Run `cargo check` and `cargo check --features dev` — expect zero errors. Warnings like "unused variable: `wall_mesh_ns`" / etc. are EXPECTED at this step (the iteration loop in Step 7 will consume them). Do NOT silence the warnings — Step 7 fixes them by using the variables.

**Done state:** `spawn_dungeon_geometry` is defined and registered in `OnEnter(GameState::Dungeon)`. Asset-tolerant guards, mesh + material caches in place. Per-cell iteration loop is empty — runs once on state entry, logs "Spawned dungeon geometry for floor '...'" but spawns nothing visible. Compiler warns about unused variables (acceptable; consumed in Step 7).

### Step 7: Implement the per-cell iteration loop and directional light spawn

The core of the feature. Walks `floor.walls[y][x]` per the canonical "north + west of every cell, plus south/east of edge cells" rule. Spawns floor + ceiling slabs unconditionally; spawns wall plates only when `wall_material` returns `Some`.

- [ ] In `src/plugins/dungeon/mod.rs`, in `fn spawn_dungeon_geometry` (where Step 6's "Per-cell loop lands in Step 7." placeholder comment is), insert the iteration body:
  ```rust
  for y in 0..floor.height {
      for x in 0..floor.width {
          let world_x = x as f32 * CELL_SIZE;
          let world_z = y as f32 * CELL_SIZE;

          // Floor tile: top face flush with world_y = 0.0; cuboid center at -FLOOR_THICKNESS/2.
          commands.spawn((
              Mesh3d(floor_mesh.clone()),
              MeshMaterial3d(floor_mat.clone()),
              Transform::from_xyz(world_x, -FLOOR_THICKNESS / 2.0, world_z),
              DungeonGeometry,
          ));

          // Ceiling tile: bottom face flush with world_y = CELL_HEIGHT; cuboid center at
          // CELL_HEIGHT + FLOOR_THICKNESS/2.
          commands.spawn((
              Mesh3d(ceiling_mesh.clone()),
              MeshMaterial3d(ceiling_mat.clone()),
              Transform::from_xyz(world_x, CELL_HEIGHT + FLOOR_THICKNESS / 2.0, world_z),
              DungeonGeometry,
          ));

          let walls = &floor.walls[y as usize][x as usize];

          // North wall: always rendered (interior cells own their north face).
          if let Some(mat) = wall_material(walls.north, &wall_solid_mat, &wall_door_mat, &wall_locked_mat) {
              commands.spawn((
                  Mesh3d(wall_mesh_ns.clone()),
                  MeshMaterial3d(mat),
                  wall_transform(x, y, Direction::North),
                  DungeonGeometry,
              ));
          }

          // West wall: always rendered.
          if let Some(mat) = wall_material(walls.west, &wall_solid_mat, &wall_door_mat, &wall_locked_mat) {
              commands.spawn((
                  Mesh3d(wall_mesh_ew.clone()),
                  MeshMaterial3d(mat),
                  wall_transform(x, y, Direction::West),
                  DungeonGeometry,
              ));
          }

          // South wall: only at the bottom edge.
          if y == floor.height - 1 {
              if let Some(mat) = wall_material(walls.south, &wall_solid_mat, &wall_door_mat, &wall_locked_mat) {
                  commands.spawn((
                      Mesh3d(wall_mesh_ns.clone()),
                      MeshMaterial3d(mat),
                      wall_transform(x, y, Direction::South),
                      DungeonGeometry,
                  ));
              }
          }

          // East wall: only at the right edge.
          if x == floor.width - 1 {
              if let Some(mat) = wall_material(walls.east, &wall_solid_mat, &wall_door_mat, &wall_locked_mat) {
                  commands.spawn((
                      Mesh3d(wall_mesh_ew.clone()),
                      MeshMaterial3d(mat),
                      wall_transform(x, y, Direction::East),
                      DungeonGeometry,
                  ));
              }
          }
      }
  }

  // Scene-wide directional light, marked DungeonGeometry for OnExit cleanup.
  commands.spawn((
      DirectionalLight {
          illuminance: 5_000.0,
          shadows_enabled: false,
          ..default()
      },
      Transform::from_xyz(0.0, CELL_HEIGHT * 4.0, 0.0)
          .looking_at(Vec3::new(0.5, -1.0, 0.3), Vec3::Y),
      DungeonGeometry,
  ));

  // Override scene-wide ambient light. Restored to default on OnExit (see despawn_dungeon_entities).
  commands.insert_resource(GlobalAmbientLight {
      color: Color::srgb(0.30, 0.32, 0.40),
      brightness: 100.0,
      ..default()
  });
  ```
  Note: `GlobalAmbientLight` is exported via `bevy::prelude::*` in Bevy 0.18 (verified at `bevy_light-0.18.1/src/ambient_light.rs:41-89`); no new import needed. The `..default()` is required because `GlobalAmbientLight` has a third field `affects_lightmapped_meshes: bool`.
- [ ] Verify: `cargo check` and `cargo check --features dev` — expect zero errors AND zero warnings (the unused-variable warnings from Step 6 are now consumed). If `wall_mesh_ns` / `wall_mesh_ew` still warn, the iteration body has a typo — fix before continuing.

**Done state:** `spawn_dungeon_geometry` is fully wired. On `OnEnter(GameState::Dungeon)` with loaded `floor_01`, the system spawns 36 floor + 36 ceiling + 48 wall plates + 1 directional light = 121 `DungeonGeometry` entities, plus inserts the `GlobalAmbientLight` override. `cargo check` zero warnings on both feature configurations.

### Step 8: Wire `DungeonGeometry` cleanup into `despawn_dungeon_entities` + restore ambient light

The OnExit handler now needs to despawn the `DungeonGeometry` entities AND restore `GlobalAmbientLight` to its default. This pairs with Step 7's spawn.

- [ ] In `src/plugins/dungeon/mod.rs`, in `fn despawn_dungeon_entities`, replace the function with:
  ```rust
  /// `OnExit(GameState::Dungeon)` — despawn all `PlayerParty` entities (recursive — child
  /// cameras are cleaned up automatically) and all `DungeonGeometry` entities (floor +
  /// ceiling tiles, wall plates, directional light). Also restores `GlobalAmbientLight`
  /// to its `LightPlugin` default so other states (Town, Combat, etc.) start with a clean
  /// ambient setting; future states own their own ambient override on entry.
  fn despawn_dungeon_entities(
      mut commands: Commands,
      parties: Query<Entity, With<PlayerParty>>,
      dungeon_geometry: Query<Entity, With<DungeonGeometry>>,
  ) {
      for e in &parties {
          commands.entity(e).despawn();
      }
      for e in &dungeon_geometry {
          commands.entity(e).despawn();
      }
      // Restore default ambient light (white, brightness 80.0). Other states will override
      // again on their own OnEnter (#18 Town, future states).
      commands.insert_resource(GlobalAmbientLight::default());
      info!("Despawned PlayerParty + dungeon geometry on OnExit(Dungeon); ambient restored");
  }
  ```
  (The function signature already changed in Step 2 — this step just adds the new query parameter and the resource-restoration call.)
- [ ] Verify: `cargo check` and `cargo check --features dev` — expect zero errors and zero warnings.

**Done state:** OnExit cleanup despawns `PlayerParty` + every `DungeonGeometry` entity (recursively) and restores `GlobalAmbientLight::default()`. F9-cycling Dungeon → TitleScreen → Dungeon does not leak entities or ambient state across cycles.

### Step 9: Add App-level entity-count tests for open and walled 3×3 floors

Two unit tests that exercise the full `spawn_dungeon_geometry` system on small synthetic floors. These pin down the iteration-rule correctness without depending on the asset file.

- [ ] In `src/plugins/dungeon/mod.rs`, in the existing `#[cfg(test)] mod tests` block (after the existing tests, before the closing `}`), add a helper function `make_walled_floor` mirroring `make_open_floor`:
  ```rust
  /// Build a `w × h` floor with EVERY wall set to `Solid` on every cell.
  /// Counterpart to `make_open_floor`. Used for the maximum-renderable-walls
  /// regression test.
  fn make_walled_floor(w: u32, h: u32) -> DungeonFloor {
      use crate::data::dungeon::{CellFeatures, WallMask};
      let solid_mask = WallMask {
          north: WallType::Solid,
          south: WallType::Solid,
          east: WallType::Solid,
          west: WallType::Solid,
      };
      DungeonFloor {
          name: "test_walled".into(),
          width: w,
          height: h,
          floor_number: 1,
          walls: vec![vec![solid_mask; w as usize]; h as usize],
          features: vec![vec![CellFeatures::default(); w as usize]; h as usize],
          entry_point: (1, 1, Direction::North),
          encounter_table: "test_table".into(),
      }
  }
  ```
- [ ] Add the entity-count tests:
  ```rust
  /// Open 3×3 floor: every wall is `Open` (no geometry rendered for any wall).
  /// Expected: 9 floor + 9 ceiling + 0 walls + 1 directional light = 19 entities.
  #[test]
  fn spawn_dungeon_geometry_open_3x3_yields_19_entities() {
      let mut app = make_test_app();
      insert_test_floor(&mut app, make_open_floor(3, 3));
      advance_into_dungeon(&mut app);

      let count = app
          .world_mut()
          .query_filtered::<Entity, With<DungeonGeometry>>()
          .iter(app.world())
          .count();
      assert_eq!(
          count, 19,
          "Open 3×3 floor: 9 floor tiles + 9 ceiling tiles + 0 walls + 1 light = 19"
      );
  }

  /// Fully-walled 3×3 floor: every cell has all 4 walls Solid.
  /// Per the canonical iteration rule:
  ///   - 9 floor tiles + 9 ceiling tiles
  ///   - North walls: 9 (one per cell, regardless of edge)
  ///   - West walls:  9 (one per cell)
  ///   - South walls: 3 (bottom row only, y==2)
  ///   - East walls:  3 (right column only, x==2)
  ///   - Total walls: 9 + 9 + 3 + 3 = 24
  /// Plus 1 directional light.
  /// Expected: 18 + 24 + 1 = 43 entities.
  #[test]
  fn spawn_dungeon_geometry_walled_3x3_yields_43_entities() {
      let mut app = make_test_app();
      insert_test_floor(&mut app, make_walled_floor(3, 3));
      advance_into_dungeon(&mut app);

      let count = app
          .world_mut()
          .query_filtered::<Entity, With<DungeonGeometry>>()
          .iter(app.world())
          .count();
      assert_eq!(
          count, 43,
          "Walled 3×3 floor: 18 floor/ceiling + 24 walls + 1 light = 43 (see test docstring for math)"
      );
  }
  ```
- [ ] Add an OnExit cleanup test:
  ```rust
  /// After OnExit(Dungeon), all DungeonGeometry entities are despawned and
  /// GlobalAmbientLight is restored to its default.
  #[test]
  fn on_exit_dungeon_despawns_all_dungeon_geometry() {
      let mut app = make_test_app();
      insert_test_floor(&mut app, make_walled_floor(3, 3));
      advance_into_dungeon(&mut app);

      // Sanity: geometry is present.
      let pre = app
          .world_mut()
          .query_filtered::<Entity, With<DungeonGeometry>>()
          .iter(app.world())
          .count();
      assert!(pre > 0, "Geometry should be present in Dungeon");

      // Transition to TitleScreen — triggers OnExit(Dungeon).
      app.world_mut()
          .resource_mut::<NextState<GameState>>()
          .set(GameState::TitleScreen);
      app.update(); // state transition realised
      app.update(); // OnExit(Dungeon) systems run

      let post = app
          .world_mut()
          .query_filtered::<Entity, With<DungeonGeometry>>()
          .iter(app.world())
          .count();
      assert_eq!(
          post, 0,
          "All DungeonGeometry entities must be despawned on OnExit(Dungeon)"
      );

      // GlobalAmbientLight should be restored to LightPlugin default.
      let ambient = app.world().resource::<GlobalAmbientLight>();
      let default_ambient = GlobalAmbientLight::default();
      assert_eq!(ambient.brightness, default_ambient.brightness, "Ambient brightness should restore to default");
  }
  ```
- [ ] Verify: `cargo test --lib plugins::dungeon::tests` — expect 5 (Step 3) + 2 (Step 4) + 3 (this step) + the existing 13 from Feature #7 = ~23 passing tests, zero warnings.
- [ ] Verify: `cargo test --features dev plugins::dungeon::tests` — same count, all passing.

**Done state:** Three new App-level tests pin down the iteration rule on synthetic floors. The OnExit test catches both leaks (entities lingering after state transition) and ambient-light restoration. All unit tests pass with both feature configurations.

### Step 10: Add the `tests/dungeon_geometry.rs` integration test

Mirrors `tests/dungeon_movement.rs` exactly — same `TestState` + `TestFloorAssets` pattern (avoids `LoadingPlugin`'s `AudioAssets` hang in headless tests). Loads the real `floor_01.dungeon.ron` via `RonAssetPlugin`; asserts the exact entity count of 121.

- [ ] **Before writing the test, RE-VERIFY the entity count for `floor_01`.** Read `assets/dungeons/floor_01.dungeon.ron` cell-by-cell and recompute:
  - 36 floor tiles + 36 ceiling tiles = 72 (constant — one per cell on a 6×6 grid).
  - 1 directional light.
  - Walls: count per the canonical iteration rule (see research §"Test Strategy" Test 7).
  - The research's calculation gives **48 renderable walls** = 14 north walls + 22 west walls + 6 south walls (bottom edge, all Solid) + 6 east walls (right edge, all Solid). Total **121 entities**.
  - If the count differs from 121 after re-verification, document the discrepancy in **Implementation Discoveries** and update the assertion to the verified count, including a comment in the test file deriving the math.
- [ ] Create `tests/dungeon_geometry.rs` with:
  ```rust
  //! App-level integration test for Feature #8: 3D Dungeon Renderer.
  //!
  //! Verifies that `spawn_dungeon_geometry` correctly spawns 121 entities tagged
  //! with `DungeonGeometry` when `GameState::Dungeon` is entered with a loaded
  //! `floor_01`. The math:
  //!   - 36 floor tiles (one per cell, 6×6 grid)
  //!   - 36 ceiling tiles (one per cell)
  //!   - 48 wall plates (per per-edge canonical iteration rule on floor_01.dungeon.ron):
  //!       * 14 north walls renderable (rows y=0 + y=5 fully Solid; y=1 has 2 Solid; y=2-4 all Open on north faces)
  //!       * 22 west walls renderable (across all rows; see plan for breakdown)
  //!       *  6 south walls (bottom row y=5, all Solid — outer edge)
  //!       *  6 east walls (right column x=5, all Solid — outer edge)
  //!   - 1 directional light
  //! Total: 36 + 36 + 48 + 1 = 121.
  //!
  //! Uses the same TestState pattern as tests/dungeon_movement.rs — drives its own
  //! TestState::Loading -> TestState::Loaded cycle using only DungeonFloor (not
  //! LoadingPlugin), inserts a stub DungeonAssets with the loaded floor handle,
  //! then transitions to GameState::Dungeon and asserts the entity count.

  use bevy::app::AppExit;
  use bevy::asset::AssetPlugin;
  use bevy::input::InputPlugin;
  use bevy::prelude::*;
  use bevy::state::app::StatesPlugin;
  use bevy_asset_loader::prelude::*;
  use bevy_common_assets::ron::RonAssetPlugin;

  use druum::data::DungeonFloor;
  use druum::plugins::audio::SfxRequest;
  use druum::plugins::dungeon::{DungeonGeometry, DungeonPlugin};
  use druum::plugins::input::ActionsPlugin;
  use druum::plugins::loading::DungeonAssets;
  use druum::plugins::state::{GameState, StatePlugin};

  /// Private loading state — only loads DungeonFloor (avoids AudioAssets/.ogg
  /// files which hang in headless test context — Feature #6's lesson).
  #[derive(Clone, Eq, PartialEq, Debug, Hash, Default, States)]
  enum TestState {
      #[default]
      Loading,
      Loaded,
  }

  #[derive(AssetCollection, Resource)]
  struct TestFloorAssets {
      #[asset(path = "dungeons/floor_01.dungeon.ron")]
      floor: Handle<DungeonFloor>,
  }

  #[test]
  fn dungeon_geometry_spawns_for_floor_01() {
      let mut app = App::new();
      app.add_plugins((
          MinimalPlugins,
          AssetPlugin::default(),
          StatesPlugin,
          InputPlugin,
          RonAssetPlugin::<DungeonFloor>::new(&["dungeon.ron"]),
          StatePlugin,
          ActionsPlugin,
          DungeonPlugin,
      ));

      // spawn_dungeon_geometry requires Assets<Mesh> and Assets<StandardMaterial>.
      // In production these are registered by MeshPlugin/PbrPlugin (via DefaultPlugins).
      // In headless integration tests we init them explicitly.
      app.init_asset::<Mesh>().init_asset::<StandardMaterial>();

      // handle_dungeon_input writes SfxRequest messages. AudioPlugin registers
      // this in production; in headless tests we register it directly.
      app.add_message::<SfxRequest>();

      // When compiled with --features dev, StatePlugin::build registers
      // cycle_game_state_on_f9 which requires ButtonInput<KeyCode>. Insert
      // directly so the system's parameter validation does not panic.
      // Same pattern as src/plugins/state/mod.rs:107, audio/mod.rs:174,
      // tests/dungeon_movement.rs:75-76.
      #[cfg(feature = "dev")]
      app.init_resource::<bevy::input::ButtonInput<KeyCode>>();

      // Drive our own loading cycle for just the DungeonFloor asset.
      app.init_state::<TestState>().add_loading_state(
          LoadingState::new(TestState::Loading)
              .continue_to_state(TestState::Loaded)
              .load_collection::<TestFloorAssets>(),
      );

      // On TestState::Loaded: insert DungeonAssets pointing to the loaded floor,
      // then queue GameState::Dungeon.
      app.add_systems(OnEnter(TestState::Loaded), setup_dungeon_assets_and_enter);

      // Assertion runs in Update once GameState::Dungeon is active. We use an
      // AssertDone resource so the assertion runs exactly once.
      app.add_systems(
          Update,
          assert_dungeon_geometry_count.run_if(in_state(GameState::Dungeon)),
      );
      app.insert_resource(AssertDone(false));

      // Timeout guard — RonAssetPlugin path errors should not silently hang the test.
      app.add_systems(Update, timeout.run_if(in_state(TestState::Loading)));

      app.run();
  }

  #[derive(Resource)]
  struct AssertDone(bool);

  fn timeout(time: Res<Time>) {
      if time.elapsed_secs_f64() > 30.0 {
          panic!("DungeonFloor did not load within 30 seconds — RonAssetPlugin path likely broken");
      }
  }

  fn setup_dungeon_assets_and_enter(
      floor_assets: Res<TestFloorAssets>,
      mut next_game_state: ResMut<NextState<GameState>>,
      mut commands: Commands,
  ) {
      commands.insert_resource(DungeonAssets {
          floor_01: floor_assets.floor.clone(),
          item_db: Handle::default(),
          enemy_db: Handle::default(),
          class_table: Handle::default(),
          spell_table: Handle::default(),
      });
      next_game_state.set(GameState::Dungeon);
  }

  /// Run-once Update system: count `DungeonGeometry` entities, assert == 121,
  /// then write `AppExit::Success`.
  fn assert_dungeon_geometry_count(
      mut done: ResMut<AssertDone>,
      query: Query<&DungeonGeometry>,
      mut exit: MessageWriter<AppExit>,
  ) {
      if done.0 {
          return;
      }
      done.0 = true;

      let count = query.iter().count();
      assert_eq!(
          count, 121,
          "Geometry entity count for floor_01 must equal 36 floor + 36 ceiling + 48 walls + 1 light = 121. \
           If this assertion fails after an asset edit, recount per the canonical iteration rule \
           (north + west of every cell, plus south of bottom row, plus east of right column)."
      );

      exit.write(AppExit::Success);
  }
  ```
- [ ] Verify: `cargo test --test dungeon_geometry` — expect 1 passing test, zero warnings.
- [ ] Verify with dev feature: `cargo test --features dev --test dungeon_geometry` — same passing result.

**Done state:** New file `tests/dungeon_geometry.rs` exercises real-asset loading + spawn + count assertion end-to-end. The test number (121) is documented in the file header so future asset edits surface as test failures with a clear migration path.

### Step 11: Manifest byte-diff guard

Defensive check before final verification: the working tree should have introduced ZERO changes to `Cargo.toml` and `Cargo.lock`. If anything has changed, identify why and revert before continuing.

- [ ] Run: `git diff Cargo.toml Cargo.lock` — expect EMPTY output. If non-empty, identify the cause (accidental dep add? formatter touched a comment?) and revert. The plan adds zero deps; a non-empty diff here means something went wrong upstream.
- [ ] Run: `git status -- Cargo.toml Cargo.lock` — expect both files unmodified.

**Done state:** Both manifest files are byte-identical to the pre-Feature-#8 main branch. Verification matrix (Step 12) confirms this with the same commands.

### Step 12: Run the full verification matrix

Run all 7 verification commands as a contiguous block. Any non-zero exit code or any warning is a blocker.

- [ ] `cargo check` — zero errors, zero warnings.
- [ ] `cargo check --features dev` — zero errors, zero warnings.
- [ ] `cargo clippy --all-targets -- -D warnings` — passes (`-D warnings` makes warnings into errors; if anything fires, fix before moving on).
- [ ] `cargo clippy --all-targets --features dev -- -D warnings` — same gate, dev feature on.
- [ ] `cargo test` — all unit + integration tests pass. Expect ~21+ tests in the dungeon module + 1 in `tests/dungeon_geometry.rs` + the pre-existing tests across the rest of the crate.
- [ ] `cargo test --features dev` — same set, all pass with dev feature on.
- [ ] `cargo fmt --check` — no formatting drift.

**Done state:** All 7 commands exit 0 with zero warnings. Working tree is clean except for the planned changes in `src/plugins/dungeon/mod.rs`, `src/data/dungeon.rs:18` (single-token edit), and `tests/dungeon_geometry.rs` (new file).

### Step 13: Manual visual smoke test

The whole point of Feature #8: the dungeon now actually looks like a dungeon. Run the game and verify everything reads correctly.

- [ ] `cargo run --features dev` — wait for the loading screen to advance. The window should open showing the `Loading` state, then transition to `TitleScreen`.
- [ ] Press F9 once (Loading → TitleScreen, if not already there). Press F9 again to cycle to `Town`. Press F9 a third time to cycle to `Dungeon`. (Per the F9 cycle order in `src/plugins/state/mod.rs`.)
- [ ] Visually verify the following on `floor_01`:
  - The player spawns at grid `(1, 1)` facing North (the entry point per `floor_01.dungeon.ron`).
  - The dungeon is bounded — outer walls visible to the north (top row), south (bottom row), east (right column), and west (left column).
  - Floor and ceiling tiles are visible (warm dark stone floor, cooler dark stone ceiling).
  - There is a Door (brown) east of (1, 1) — visible as the player turns East (D key strafes East; Q+D rotates East-then-strafe — or just press E to turn right).
  - There is a LockedDoor (dark red) further east of (3, 1) — visible after walking through the Door at (1, 1) → (2, 1) → looking East.
  - The OneWay wall at (2, 3) east is invisible from cell (2, 3) (player can walk East from (2, 3) into (3, 3)) but visible from cell (3, 3) (looking West shows a wall blocking the way back to (2, 3)).
  - SecretWall at (4, 2) east and Illusory at (1, 3) east both render as solid grey (player cannot visually distinguish from `Solid`; reveal is #13).
  - No z-fighting: no flickering between walls and floors at any viewing angle.
  - Walls have visible thickness when standing close (the 0.05 thickness is barely perceptible — that's intended).
- [ ] F9 once more to cycle Dungeon → TitleScreen (or → next state in the cycle). Verify no entities are visibly leaked (no walls floating in the title screen).
- [ ] F9 back to Dungeon. Verify the dungeon respawns correctly (no double-rendered walls; entity count is the same).
- [ ] If any of the above fails, document under **Implementation Discoveries** and either fix in this PR (if a clear bug) or escalate as a #25 polish item (if a tuning issue).
- [ ] If the wall colors look hard to distinguish (e.g., Door brown reads as too similar to Solid grey under the chosen ambient), tune the constants in Step 6/7 — this is the explicit invitation in the plan §Approach #6. Document any color changes.
- [ ] If the directional light angle creates ugly hot-spots or featureless flat regions, tune the `Transform::from_xyz(...)`/`looking_at(...)` arguments. Document any changes.

**Done state:** Manual smoke test complete. The dungeon is visually verifiable as a real dungeon. Any tuning changes are documented in **Implementation Discoveries**.

## Security

**Known vulnerabilities:** No new dependencies are added in this feature, so no new CVE surface. Existing dependencies (Bevy 0.18.1, bevy_common_assets 0.16.0, bevy_asset_loader 0.26.0, leafwing-input-manager 0.20.0, serde 1, ron 0.12) are unchanged from Feature #7. No known vulnerabilities identified in these versions as of the research date (2026-05-03).

**Architectural risks:**

- **Trust boundary — `floor.walls[][]` indexing.** The iteration loop reads `floor.walls[y as usize][x as usize]` for `y in 0..floor.height` and `x in 0..floor.width`. `DungeonFloor::is_well_formed()` (in `data/dungeon.rs:302-313`) is the contract that `walls.len() == height && all rows are width long`. If a malformed floor asset (e.g., `width: 6, height: 6` but `walls: [[...3 rows of 6...]]`) is loaded, the indexing will panic. **Mitigation:** Feature #4's `is_well_formed()` is meant to be checked at load-time; Feature #8 does NOT add a separate check (per the freeze list — `loading/mod.rs` is frozen). The asset is hand-authored under our control; integration test (`tests/dungeon_floor_loads.rs`, Feature #4) should catch malformed floors before they reach `spawn_dungeon_geometry`. If a runtime panic is observed in QA on a malformed floor, fix is to add `is_well_formed()` validation in #25 polish.
- **No runtime user input feeds into geometry generation.** The system reads only the static asset (`Res<Assets<DungeonFloor>>`); no player-controlled values flow into mesh dimensions, material colors, or spawn coordinates. No injection vectors.
- **Trust boundary — `entry_point` validation is inherited from Feature #7.** `spawn_party_and_camera` uses `floor.entry_point` to place the player; Feature #8 does not consume this field directly. If `entry_point.0 >= floor.width` (out of bounds), Feature #7's spawn places the player at an invalid grid coordinate, but Feature #8's geometry generation is unaffected (it iterates `0..width × 0..height` regardless of entry).
- **No `unwrap()` on Asset access.** All asset reads use the `Option<Res<...>>` + `let-else` pattern from Feature #7 (`spawn_party_and_camera` precedent at lines 234-246). A missing asset returns silently with a warning log, never a panic.

## Open Questions

All open questions from research are resolved:

1. **Per-wall entity vs merged mesh** — (Resolved: per-wall entity. ~121 entities for floor_01, well within Bevy's clustered renderer's batching capacity. Merging would complicate per-wall feature work in Feature #13 and is not justified by current scale.)
2. **Walls per cell vs walls per edge** — (Resolved: per-edge — render `north + west` of every cell, plus `south` on bottom row and `east` on right column. Documented as the "canonical iteration rule" in §Approach #2 and inline in `spawn_dungeon_geometry`.)
3. **Floor + ceiling: combined slab vs per-tile** — (Resolved: per-tile. Bevy auto-batches identical mesh+material pairs into one draw call; same GPU cost as a merged slab, but per-cell flexibility for Feature #13.)
4. **Cell height (world_y units)** — (Resolved: 3.0. Matches master research §Pattern 6, `EYE_HEIGHT` doc-comment in #7, and genre precedent for Wizardry-style corridors.)
5. **Player-attached light vs scene-wide directional** — (Resolved: scene-wide `DirectionalLight` + `GlobalAmbientLight` resource override. Player-attached light is reserved for #9 atmosphere.)
6. **Wall thickness** — (Resolved: 0.05 world units. Avoids degenerate-volume Cuboids and z-fighting with adjacent slabs; invisible at typical viewing distances.)
7. **OneWay walls visual asymmetry** — (Resolved: render only on the side stored as `Solid`; the `OneWay` side gets no geometry. Falls out for free from per-edge iteration when `wall_material` returns `None` for `OneWay`.)
8. **Wall color palette** — (Resolved: cool grey for Solid/SecretWall/Illusory, brown for Door, dark red for LockedDoor; floor warm dark stone, ceiling cool dark stone. Subjective; iterated via Step 13 visual smoke. Documented in §Approach #6.)
9. **DirectionalLight position/orientation** — (Resolved: `(0.0, CELL_HEIGHT * 4.0, 0.0)` looking at `(0.5, -1.0, 0.3)` — high overhead, slightly off-axis for differential corner shading. Tunable via Step 13 visual smoke.)
10. **`GlobalAmbientLight` restoration on OnExit policy** — (Resolved: `commands.insert_resource(GlobalAmbientLight::default())` on OnExit restores `LightPlugin` defaults. Future states (#18 Town) override on their own OnEnter; convention is "every state owns its ambient on entry; OnExit resets to default." Documented in `despawn_dungeon_entities` doc comment.)
11. **Cell-feature visual overrides for v1?** — (Resolved: NO. Spinner/dark zone/anti-magic zone/trap/teleporter visuals are Feature #13's scope. #8 is wall geometry only.)
12. **Module split (`render.rs` submodule)?** — (Resolved: NO submodule. Keep everything in `src/plugins/dungeon/mod.rs`. File grows to ~1200-1350 LOC, still single-screenful for navigation. The `DungeonGeometry` marker + `spawn_dungeon_geometry` system + `wall_transform` helper form a clean seam that #9 (atmosphere) can extract later if forced. Mirrors Feature #7's single-file decision.)
13. **Sequential vs parallel `OnEnter` system run?** — (Resolved: parallel is fine. `spawn_party_and_camera` and `spawn_dungeon_geometry` have no resource conflict — both read `Res<DungeonAssets>` and `Res<Assets<DungeonFloor>>` immutably; only `spawn_dungeon_geometry` writes `ResMut<Assets<Mesh>>`, `ResMut<Assets<StandardMaterial>>`, and `Commands::insert_resource::<GlobalAmbientLight>()`. Bevy's scheduler will parallelize automatically. No `before`/`after` ordering needed in `OnEnter(GameState::Dungeon)` tuple.)

## Implementation Discoveries

**User override applied: Wizardry torchlight instead of Etrian Odyssey scene-wide DirectionalLight.** The user confirmed before implementation that the plan's lighting spec (DirectionalLight + GlobalAmbientLight brightness=100) should be replaced with:
1. No DirectionalLight entity in `spawn_dungeon_geometry`.
2. `GlobalAmbientLight` brightness=50.0 (near-black, not 100.0).
3. `PointLight` child of `DungeonCamera` with intensity=1500.0, range=6.0, color=`srgb(1.0, 0.85, 0.55)`, shadows_enabled=false.
This is the "Wizardry-style torchlight" aesthetic.

**Entity count changes due to override:** The plan stated 121 entities (36+36+48 walls+1 light). With the override removing the DirectionalLight from `DungeonGeometry` (the PointLight is instead a child of DungeonCamera, not tagged DungeonGeometry), the actual `DungeonGeometry` entity count is **120** (36+36+48). The integration test and unit tests were adjusted accordingly (19→18 for open 3×3, 43→42 for walled 3×3).

**Verified entity count for `floor_01.dungeon.ron`:** Manual cell-by-cell recount confirmed 48 renderable walls:
- North walls: 6 (y=0 row) + 2 (y=1: only x=0 and x=5 are Solid) + 0 (y=2,3,4) + 6 (y=5 row) = 14
- West walls: 6 (y=0) + 4 (y=1: x=0 Solid, x=2 Door, x=4 LockedDoor, x=5 Solid) + 2 (y=2: x=0 Solid, x=5 SecretWall) + 3 (y=3: x=0 Solid, x=2 Illusory, x=3 Solid) + 1 (y=4: x=0 Solid) + 6 (y=5) = 22
- South walls (y=5 row only): 6 — all Solid outer edge
- East walls (x=5 column only): 6 — all Solid outer edge
- Total walls: 14+22+6+6 = **48 ✓**
- DungeonGeometry entities: 36+36+48 = **120** (no DirectionalLight)

**Clippy: collapsible_if for nested `if y == ... { if let Some ...`** The plan's iteration loop used nested ifs. Clippy complained about `collapsible_if` and suggested merging into `if y == floor.height - 1 && let Some(mat) = wall_material(...)`. Applied as suggested; this required Rust 2024 edition `let` chains which are stabilised.

**Clippy: doc_lazy_continuation on multi-level list continuation lines.** Both `mod.rs` and `dungeon_geometry.rs` had doc comments with `Total:` or `Expected:` continuation lines after a list that needed blank-line separation. Fixed by adding blank `///` lines before those continuation items.

**`cargo fmt` reformatted the PointLight struct's alignment comments** (column-aligned comment padding) and reformatted two `if let Some(mat) = wall_material(...)` patterns from inline form to expanded form. Applied `cargo fmt` to bring formatting into compliance before the fmt check step.

**LOC delta:** `src/plugins/dungeon/mod.rs`: 997 → 1355 LOC (+358 net). Slightly over the plan's +350 upper bound due to the additional PointLight children spawn in `spawn_party_and_camera` (+15 LOC) and the formatting expansion clippy/fmt required for the if-let patterns. Within the 25% overrun band cited in plan §LOC Estimate.

**Steps collapsed:** Steps 1–10 were implemented as a single contiguous edit session (no intermediate "broken compile" checkpoint). The plan allowed this — steps 1–6 are described as sequential but the intermediate compile-break (missing `spawn_dungeon_geometry`) was never committed. All steps complete.

## Verification

- [x] `cargo check` passes with zero warnings — automatic — `cargo check`
- [x] `cargo check --features dev` passes with zero warnings — automatic — `cargo check --features dev`
- [x] `cargo clippy --all-targets -- -D warnings` passes — automatic — `cargo clippy --all-targets -- -D warnings`
- [x] `cargo clippy --all-targets --features dev -- -D warnings` passes — automatic — `cargo clippy --all-targets --features dev -- -D warnings`
- [x] `cargo test` passes (61 tests in dungeon module + 1 in tests/dungeon_geometry.rs + pre-existing) — automatic — `cargo test`
- [x] `cargo test --features dev` passes — automatic — `cargo test --features dev`
- [x] `cargo fmt --check` reports no diff — automatic — `cargo fmt --check`
- [x] `Cargo.toml` and `Cargo.lock` are byte-unchanged — automatic — `git diff Cargo.toml Cargo.lock` (expect EMPTY output)
- [x] All 5 `wall_transform_*` tests pass — automatic — `cargo test --lib plugins::dungeon::tests::wall_transform`
- [x] `wall_material_returns_none_for_passable` and `wall_material_returns_some_for_blocking` tests pass — automatic — `cargo test --lib plugins::dungeon::tests::wall_material`
- [x] `spawn_dungeon_geometry_open_3x3_yields_18_entities` passes — automatic (count updated to 18 per override: no DirectionalLight in DungeonGeometry)
- [x] `spawn_dungeon_geometry_walled_3x3_yields_42_entities` passes — automatic (count updated to 42 per override)
- [x] `on_exit_dungeon_despawns_all_dungeon_geometry` passes — automatic — `cargo test --lib plugins::dungeon::tests::on_exit_dungeon_despawns_all_dungeon_geometry`
- [x] `dungeon_geometry_spawns_for_floor_01` integration test passes (count == 120, adjusted from plan's 121) — automatic — `cargo test --test dungeon_geometry`
- [x] `src/data/dungeon.rs:18` doc-comment reads `world_z = +grid_y * cell_size` (single-character change) — automatic — `git diff src/data/dungeon.rs` (expect 1 line, 1 character changed)
- [ ] Manual visual smoke: dungeon renders correctly on `cargo run --features dev` per the checklist in Step 13 — manual — `cargo run --features dev` then F9 to Dungeon, walk through floor_01 with WASDQE, verify all bullets in Step 13.

## LOC Estimate

Conservative-realistic estimate (Feature #7 came in 25% over plan; pad accordingly):

| Section | Production LOC | Test LOC | Notes |
|---------|----------------|----------|-------|
| Step 1 (constants + `DungeonGeometry` marker) | +12 | 0 | 3 const decls with doc comments + 1 component |
| Step 2 (deletions: `TestSceneMarker`, `spawn_test_scene`, despawn branch) | -75 | 0 | Lines 137-141 + 272-348 + ~6 lines from despawn handler. Includes doc comments. |
| Step 3 (`wall_transform` + 5 tests) | +20 | +35 | Pure helper with mid-grid + corner-cell tests |
| Step 4 (`wall_material` + 2 tests) | +18 | +25 | 5-arm match + 2 tests covering all 7 WallType variants |
| Step 5 (doc-comment fix in `data/dungeon.rs`) | 0 | 0 | Single-token change |
| Step 6 (`spawn_dungeon_geometry` skeleton: signature, asset guards, mesh/material caches) | +60 | 0 | Doc-comment-heavy; sets up Step 7 |
| Step 7 (per-cell loop + directional light + ambient override) | +90 | 0 | Iteration body, light spawn, resource insert |
| Step 8 (despawn cleanup + ambient restore) | +5 | 0 | Add query + cleanup loop + resource insert |
| Step 9 (`make_walled_floor` helper + 3 App-level tests) | 0 | +75 | Helper + open-3×3 + walled-3×3 + on-exit tests |
| Step 10 (`tests/dungeon_geometry.rs` new file) | 0 | +130 | Mirrors `tests/dungeon_movement.rs` |
| **Net delta in `src/plugins/dungeon/mod.rs`** | **+205** (+130 production, +75 tests minus 0 test-deletions) | | Net file growth from ~997 → ~1175 LOC |
| **Net delta in `src/data/dungeon.rs`** | **±0** (1 line touched, no LOC change) | | Doc-comment edit |
| **`tests/dungeon_geometry.rs`** (new file) | | **+130** | New integration test |
| **Total LOC delta** | **+205** in production-side code | **+130** in tests | |
| **Cargo.toml + Cargo.lock** | **0 bytes** | | Byte-identical guarantee |
| **Compile time delta** | +0.5s estimated (per roadmap) | | One new system, primitive cuboids |
| **Test count delta** | | **+11 tests** (5 + 2 + 3 unit + 1 integration in this plan; possibly trim to 7-9 if a unit test consolidates) | |

If the implementer comes in 25% over (matching #7's overrun), expect +250-260 production LOC + +160 test LOC = +410-420 LOC total. Flag in **Implementation Discoveries** if growth exceeds this band.
