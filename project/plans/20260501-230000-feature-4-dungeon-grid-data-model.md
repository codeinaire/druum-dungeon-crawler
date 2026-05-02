# Plan: Dungeon Grid Data Model (Feature #4)

**Date:** 2026-05-01
**Status:** Complete
**Research:** ../research/20260501-220000-feature-4-dungeon-grid-data-model.md
**Depends on:** 20260501-164500-bevy-0-18-1-asset-pipeline-feature-3.md

## Goal

Replace the empty `DungeonFloor` stub at `src/data/dungeon.rs` with the razor-wall grid data model — `WallType`, `WallMask`, `Direction`, `CellFeatures`, `TrapType`, `TeleportTarget`, plus `DungeonFloor::can_move`, `wall_between`, and `validate_wall_consistency` — author a non-empty `assets/dungeons/floor_01.dungeon.ron` that exercises every variant, and add an `App`-level integration test that loads the file through `bevy_common_assets::RonAssetPlugin` (the ron 0.11 path) end-to-end. No new dependencies, no Bevy ECS systems, no Plugin impls.

## Approach

The research recommends matching §Pattern 2 from the original Bevy dungeon-crawler research (`research/20260326-01-bevy-first-person-dungeon-crawler-rpg.md:330-466`) almost verbatim, with three deliberate refinements: (1) drop the `CellFeatures::Door` variant from the user-task description because doors already live in `WallType` (a feature appearing in two places is a lazy-design footgun), (2) add `#[serde(default)]` to `CellFeatures` so empty cells emit as `()` in RON instead of forcing every cell to enumerate every field, (3) add `PartialEq` to every type so the new App-level integration test can assert exact field shapes without hand-comparing each one.

The §Pattern 2 store-twice wall representation (each cell carries N/E/S/W) wins decisively for hand-authoring the 20 dungeon floors that must ship before Feature #24's editor lands — it costs ~50% extra bytes per floor (negligible at this project's scale) and one consistency-checking validator (~20 LOC, runs in tests). The alternative store-once approach would force every author to mentally trace neighbor cells to find any wall, which compounds badly with the existing `(y, x)` row-major addressing.

The single biggest correctness risk is the ron 0.11 vs ron 0.12 dependency split flagged by the prior code review (`.claude/agent-memory/code-reviewer/feedback_ron_version_split.md`). `bevy_common_assets 0.16.0` parses RON via `ron 0.11.0` internally; the project's direct `ron = "0.12"` is what `cargo test`'s pure-stdlib round-trip exercises. The two versions emit byte-identical RON for every type shape Feature #4 introduces (verified by direct diff of `ron-0.11.0/tests/123_enum_representation.rs` vs `ron-0.12.1/tests/123_enum_representation.rs`, plus equivalent diffs for `options.rs`, `floats.rs`), but "today the format matches" is not a contract for the future. The mitigation is a new `tests/dungeon_floor_loads.rs` integration test that drives a Bevy `App` through `RonAssetPlugin::<DungeonFloor>::new(&["dungeon.ron"])` and asserts the loaded `DungeonFloor` matches a known shape — this is the verification the prior reviewer deferred from Feature #3.

The y-axis convention question (RQ5) is locked here: `Direction::North = (0, -1)` (y-DOWN screen convention). RON files lay out `walls: [[row 0], [row 1], ...]` with row 0 at the top of the file = top of the screen = "north." Bevy's world coordinates are y-UP; the conversion (`world_z = -grid_y * cell_size`) lives in Feature #8's renderer, not in this data model. A doc comment on `Direction` documents this verbatim so future contributors don't fight it.

The `can_move` semantics matrix (RQ4) is fully specified before writing any tests: out-of-bounds returns `false`; `Open | Door | Illusory | OneWay` return `true`; `Solid | LockedDoor | SecretWall` return `false`. Discovery state for `SecretWall` is intentionally NOT a parameter — `DungeonFloor` is a static, hot-reloadable, file-backed asset; mutable per-save discovery state lives in a separate resource that Feature #13 will own. Feature #7 (movement) layers a `can_move_with_discovery` wrapper on top.

`Cargo.toml` does NOT change. `serde` and `ron` are already declared explicitly (Rust 2024 edition requirement, learned from Feature #3). All 6 verification commands (`cargo check`, `cargo check --features dev`, `cargo clippy --all-targets -- -D warnings`, `cargo clippy --all-targets --features dev -- -D warnings`, `cargo test`, `cargo test --features dev`) must pass with zero warnings.

## Critical

- **No new dependencies.** `Cargo.toml` is not modified. If a step seems to want a new crate, that's a bug in the step — re-derive from the existing dep set.
- **No `rand` calls.** Permadeath is in scope; deterministic RNG via `RngSeed` lands in Feature #23. The data model has no RNG need; teleporter destinations are fixed in the asset.
- **No Bevy ECS systems, no Plugin impls.** Feature #4 is pure data. Helper systems and plugins belong in Feature #7+ (movement, dungeon plugin).
- **`DungeonFloor` MUST keep `Asset + Reflect + Serialize + Deserialize + Default + Debug + Clone`.** Feature #3's `LoadingPlugin` (`src/plugins/loading/mod.rs:30-41`) constructs `Handle<DungeonFloor>` and depends on these derives. Removing any of them breaks the loader.
- **Spinner is `CellFeatures::spinner: bool` (no `SpinnerStyle` enum, no `telegraphed: bool` flag)** per roadmap §Resolved #4 (modern telegraphed Etrian style, all 20 floors). A flag for a non-existent classic-mode is YAGNI; if Iron Mode adds classic spinners later it becomes a global setting, not a per-cell field.
- **Drop the `CellFeatures::Door` variant from the user-task enum description.** Doors are wall-side properties; `WallType::Door` and `WallType::LockedDoor` already model them. Two places to model "door" creates a synchronization bug.
- **Razor-wall double-storage REQUIRES a consistency validator.** Each shared wall is stored twice (cell A's east, cell B's west). A `validate_wall_consistency()` method walks every adjacent pair and asserts symmetry; a unit test runs it against the hand-authored `floor_01.dungeon.ron` so authoring errors surface at `cargo test` time, not at runtime.
- **`#[serde(default)]` on `CellFeatures` is non-negotiable.** Without it, every cell's RON entry must enumerate every field — empty cells balloon to `(trap: None, teleporter: None, spinner: false, ...)`. With it, an empty cell emits as `()`.
- **`Vec<Vec<WallMask>>` is indexed `walls[y][x]`, NOT `walls[x][y]`.** Roadmap line 261 acknowledges the convention; never hand-index from outside `dungeon.rs` — every consumer goes through a method (`can_move`, `wall_between`, `cell_at` if added).
- **All 6 verification commands must pass with ZERO warnings** — `cargo check`, `cargo check --features dev`, `cargo clippy --all-targets -- -D warnings`, `cargo clippy --all-targets --features dev -- -D warnings`, `cargo test`, `cargo test --features dev`.
- **The integration test in `tests/dungeon_floor_loads.rs` MUST use `MessageWriter<AppExit>`, not `EventWriter<AppExit>`.** Bevy 0.18's Event/Message family rename — same trap that bit Feature #2 (`StateTransitionEvent` is a `Message`) and Feature #3 (`AssetEvent` is a `Message`).

## Steps

### Step 1: Replace the type definitions in `src/data/dungeon.rs`

Replace the entire file contents with the new shape. The existing `DungeonFloor` struct body becomes the new field set; the existing `#[cfg(test)] mod tests` block is updated in Step 2.

- [x] Open `/Users/nousunio/Repos/Learnings/claude-code/druum/src/data/dungeon.rs`.
- [x] Replace the file's module-level doc comment with: a one-line summary, a reference to research §Pattern 2, and a reference to `project/research/20260501-220000-feature-4-dungeon-grid-data-model.md`.
- [x] Add `pub enum Direction { North, South, East, West }` with derive set `#[derive(Reflect, Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]`. Add the doc comment from research RQ5 verbatim — it documents the y-down convention (North = (0, -1), South = (0, 1), East = (1, 0), West = (-1, 0)) and the rationale (matches RON row layout and auto-map screen origin).
- [x] Add `impl Direction` with four methods, all `pub fn ... (self) -> ...`:
  - `pub fn turn_left(self) -> Self` — North→West, West→South, South→East, East→North.
  - `pub fn turn_right(self) -> Self` — North→East, East→South, South→West, West→North.
  - `pub fn reverse(self) -> Self` — North↔South, East↔West.
  - `pub fn offset(self) -> (i32, i32)` — North=(0,-1), South=(0,1), East=(1,0), West=(-1,0). Doc comment: "Returns (dx, dy); y-down convention (see enum doc)."
- [x] Add `pub enum WallType` with seven variants and derive `#[derive(Reflect, Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Default)]`. Variants in this order: `#[default] Open`, `Solid`, `Door`, `LockedDoor`, `SecretWall`, `OneWay`, `Illusory`. Add a one-line doc comment per variant matching the research RQ7 code example (e.g. `Door` = "Closed but unlocked — can_move returns true.").
- [x] Add `pub struct WallMask { pub north: WallType, pub south: WallType, pub east: WallType, pub west: WallType }` with derive `#[derive(Reflect, Serialize, Deserialize, Default, Debug, Clone, Copy, PartialEq, Eq)]`. Field order matches §Pattern 2.
- [x] Add `pub struct TeleportTarget { pub floor: u32, pub x: u32, pub y: u32, pub facing: Option<Direction> }` with derive `#[derive(Reflect, Serialize, Deserialize, Debug, Clone, PartialEq)]`. NO `Default` — a default `TeleportTarget` would teleport to floor 0 (1, 1, North) which is a meaningless value.
- [x] Add `pub enum TrapType` with four variants and derive `#[derive(Reflect, Serialize, Deserialize, Debug, Clone, PartialEq)]`. Variants in this order: `Pit { damage: u32, target_floor: Option<u32> }`, `Poison`, `Alarm`, `Teleport(TeleportTarget)`. NO `Default` — absence is `Option<TrapType> = None`.
- [x] Add `pub struct CellFeatures` with derive `#[derive(Reflect, Serialize, Deserialize, Default, Debug, Clone, PartialEq)]` and the **`#[serde(default)]`** struct-level attribute (placed AFTER the `derive`, BEFORE the struct definition). Fields in this order, matching the research RQ3 "Recommended final shape": `pub trap: Option<TrapType>`, `pub teleporter: Option<TeleportTarget>`, `pub spinner: bool` (with doc comment "§Resolved #4: telegraphed only — bool is sufficient."), `pub dark_zone: bool` (doc: "Disables auto-map within this cell."), `pub anti_magic_zone: bool` (doc: "Disables spell casting within this cell."), `pub encounter_rate: f32` (doc: "0.0 = no random encounters, 1.0 = encounter every step."), `pub event_id: Option<String>` (doc: "Optional scripted event identifier; resolved at runtime by Feature #13.").
- [x] Add `pub struct DungeonFloor` (replacing the empty stub) with derive `#[derive(Asset, Reflect, Serialize, Deserialize, Default, Debug, Clone, PartialEq)]`. Fields in this order: `pub name: String`, `pub width: u32`, `pub height: u32`, `pub floor_number: u32`, `pub walls: Vec<Vec<WallMask>>` (doc: "Walls stored as `[y][x]` grid. y-down screen convention."), `pub features: Vec<Vec<CellFeatures>>` (doc: "Cell features stored as `[y][x]` grid. Same shape as `walls`."), `pub entry_point: (u32, u32, Direction)`, `pub encounter_table: String`.
- [x] Add `pub struct WallInconsistency` with derive `#[derive(Debug, PartialEq)]` and fields: `pub cell_a: (u32, u32)`, `pub cell_b: (u32, u32)`, `pub direction: Direction`, `pub wall_a: WallType`, `pub wall_b: WallType`. NO Reflect/Serialize — this is a runtime error type, not an asset.
- [x] Add `impl DungeonFloor` block with three methods (bodies in Steps 3-4):
  - `pub fn can_move(&self, x: u32, y: u32, dir: Direction) -> bool` — body in Step 4. Add the full doc comment from research RQ4 (out-of-bounds returns false; Open/Door/Illusory/OneWay pass; Solid/LockedDoor/SecretWall block; does NOT consider discovery or keys).
  - `pub fn wall_between(&self, a: (u32, u32), b: (u32, u32)) -> WallType` — body in Step 4. Doc: "Returns the WallType between two adjacent cells. Returns `WallType::Solid` for non-adjacent or out-of-bounds pairs."
  - `pub fn validate_wall_consistency(&self) -> Result<(), Vec<WallInconsistency>>` — body in Step 3. Doc: "Asserts wall double-storage symmetry. `OneWay` walls are the one allowed asymmetry."
  - `pub fn is_well_formed(&self) -> bool` — body inline now. Returns `self.walls.len() == self.height as usize && self.walls.iter().all(|row| row.len() == self.width as usize) && self.features.len() == self.height as usize && self.features.iter().all(|row| row.len() == self.width as usize)`. Doc: "Asserts width/height match the actual `Vec` dimensions; default-constructed floors return `true` (zero-by-zero)."
- [x] Verify the existing `pub use dungeon::DungeonFloor;` re-export in `src/data/mod.rs:17` still resolves — no change to that file is needed (the public type name `DungeonFloor` is unchanged).
- [x] Run `cargo check` — must compile with zero warnings (the new types compile, the methods have placeholder bodies that will be filled in Steps 3-4 — for Step 1, give them `todo!()` bodies so the file compiles). Run `cargo check --features dev` — same.

**Done state:** `src/data/dungeon.rs` defines all eight types with correct derives. `src/plugins/loading/mod.rs` still compiles unchanged because `DungeonFloor` is still an `Asset`. Method bodies are `todo!()` placeholders. The existing test (Step 2 will update it) may be temporarily commented out or stubbed if it cannot compile against the new shape — note this in Implementation Discoveries if needed. Both `cargo check` and `cargo check --features dev` succeed with zero warnings.

### Step 2: Update the existing `#[cfg(test)] mod tests` for the new shape

The existing test (`dungeon_floor_round_trips_through_ron`) calls `DungeonFloor::default()`. With the new field set, `Default` produces `walls: vec![]`, `width: 0`, `height: 0`, etc. — empty Vec round-trips correctly through serde. The test should still pass; verify it does, then add a sibling test for a hand-built non-trivial floor.

- [x] Open `src/data/dungeon.rs`. Locate the existing `#[cfg(test)] mod tests` block.
- [x] Keep the existing `dungeon_floor_round_trips_through_ron` test as-is. The default `DungeonFloor` now has Vec<Vec<>> fields that round-trip correctly through serde even when empty (verified by inspection of `ron-0.11.0/tests/238_array.rs` and the equivalent in 0.12.1 — sequence emission is depth-stable).
- [x] Add a second test `dungeon_floor_round_trips_with_real_data`. Build a 2×2 `DungeonFloor` in code with: `name: "test"`, `width: 2`, `height: 2`, `floor_number: 1`, `walls: vec![vec![WallMask { north: WallType::Solid, ... }, ...], vec![...]]` populating all four sides for both cells with a mix of variants (at least one `Open`, one `Solid`, one `Door`), `features: vec![vec![CellFeatures::default(), CellFeatures { spinner: true, ..default() }], vec![CellFeatures { trap: Some(TrapType::Poison), ..default() }, CellFeatures::default()]]`, `entry_point: (0, 0, Direction::North)`, `encounter_table: "test_table".into()`. Round-trip through `ron::ser::to_string_pretty` + `ron::de::from_str` and assert the deserialized value `==` the original.
- [x] Run `cargo test data::dungeon::tests` — both round-trip tests pass.
- [x] Run `cargo test --features dev data::dungeon::tests` — same.

**Done state:** Two unit tests in `src/data/dungeon.rs` exercise serde symmetry: one for `Default` (empty grids), one for a hand-built 2×2 floor with multiple `WallType` and `CellFeatures` variants. Both pass under both feature sets.

### Step 3: Implement `validate_wall_consistency` and add its tests

Replace the `todo!()` body in `validate_wall_consistency` and add unit tests for both happy-path and mismatch cases.

- [x] In `src/data/dungeon.rs`, implement `validate_wall_consistency`. Add a free helper function `fn walls_consistent(a: WallType, b: WallType) -> bool` that returns `true` when `a == b`, OR when either `a` or `b` is `WallType::OneWay` (the one allowed asymmetry). Iterate `for y in 0..self.height` and `for x in 0..self.width`. For each cell, check the east-west pair (when `x + 1 < self.width`): compare `self.walls[y as usize][x as usize].east` with `self.walls[y as usize][(x + 1) as usize].west`. If `!walls_consistent(...)`, push a `WallInconsistency { cell_a: (x, y), cell_b: (x + 1, y), direction: Direction::East, wall_a: a, wall_b: b }`. Mirror logic for the south-north pair when `y + 1 < self.height`. Return `Ok(())` if `errors.is_empty()`, `Err(errors)` otherwise.
- [x] Add a test `validate_wall_consistency_passes_on_well_formed`: build a 2×2 `DungeonFloor` where every adjacent wall agrees (e.g. `walls[0][0].east == walls[0][1].west == WallType::Solid`). Assert `floor.validate_wall_consistency().is_ok()`.
- [x] Add a test `validate_wall_consistency_detects_east_west_mismatch`: build a 2×2 `DungeonFloor` where `walls[0][0].east = WallType::Open` but `walls[0][1].west = WallType::Solid`. Assert `floor.validate_wall_consistency()` returns `Err` with one inconsistency, `direction == Direction::East`, `cell_a == (0, 0)`, `cell_b == (1, 0)`.
- [x] Add a test `validate_wall_consistency_detects_north_south_mismatch`: same shape, but with `walls[0][0].south = WallType::Solid` and `walls[1][0].north = WallType::Open`. Assert one inconsistency with `direction == Direction::South`.
- [x] Add a test `validate_wall_consistency_allows_one_way_asymmetry`: build a 2×2 with `walls[0][0].east = WallType::OneWay` and `walls[0][1].west = WallType::Solid`. Assert `is_ok()` (the asymmetry is expected).
- [x] Run `cargo test data::dungeon::tests::validate_wall_consistency` — all four tests pass.
- [x] Run `cargo clippy --all-targets -- -D warnings` — no warnings (expect to need `as usize` casts; if clippy flags `cast_possible_truncation` on `x: u32 -> usize`, that's the existing convention from Pattern 2 and is correct on 32-bit-indexed grids; if it does fire, suppress with `#[allow(clippy::cast_possible_truncation)]` at the function level with a one-line comment explaining the convention).

**Done state:** `validate_wall_consistency` implemented; four unit tests cover happy path, east-west mismatch, north-south mismatch, and OneWay asymmetry. Zero clippy warnings.

### Step 4: Implement `can_move` and `wall_between`, add their tests

Replace the `todo!()` bodies and add the WallType matrix tests for `can_move`.

- [x] Implement `can_move(&self, x: u32, y: u32, dir: Direction) -> bool`: if `x >= self.width || y >= self.height`, return `false`. Otherwise, `let cell = &self.walls[y as usize][x as usize]; let wall = match dir { Direction::North => cell.north, Direction::South => cell.south, Direction::East => cell.east, Direction::West => cell.west, };` and return `matches!(wall, WallType::Open | WallType::Door | WallType::Illusory | WallType::OneWay)`. Note: Pattern 2's body returns true on only `Open | Illusory`. Feature #4 extends to `Door` (Feature #13's animation handles the door-open feel) and `OneWay` (passable from this side; reverse is the neighbor's call).
- [x] Implement `wall_between(&self, a: (u32, u32), b: (u32, u32)) -> WallType`. Determine the direction from `a` to `b` by computing `(b.0 as i32 - a.0 as i32, b.1 as i32 - a.1 as i32)`; match against `(1, 0) => East`, `(-1, 0) => West`, `(0, 1) => South`, `(0, -1) => North`, anything else => return `WallType::Solid` (non-adjacent). Bounds-check `a` against `self.width`/`self.height`; if out of bounds, return `WallType::Solid`. Return the wall on `a`'s side in the computed direction.
- [x] Add the `can_move` test matrix in `#[cfg(test)] mod tests`. For each test, build a small (2×2 or 3×3) `DungeonFloor` with the relevant `WallType` placed precisely:
  - `can_move_returns_false_when_x_out_of_bounds` — `width: 2, height: 2`, call `floor.can_move(2, 0, Direction::East)`, assert `false`.
  - `can_move_returns_false_when_y_out_of_bounds` — call `floor.can_move(0, 2, Direction::South)`, assert `false`.
  - `can_move_blocks_solid` — set `walls[0][0].east = WallType::Solid`, assert `floor.can_move(0, 0, Direction::East) == false`.
  - `can_move_passes_open` — set `walls[0][0].east = WallType::Open`, assert `true`.
  - `can_move_passes_door` — set `walls[0][0].east = WallType::Door`, assert `true`. (Feature #13 will animate; this asset-level check returns true.)
  - `can_move_blocks_locked_door` — set `walls[0][0].east = WallType::LockedDoor`, assert `false`.
  - `can_move_blocks_secret_wall` — set `walls[0][0].east = WallType::SecretWall`, assert `false`. (Discovery is a runtime resource, not an asset field — see Feature #13.)
  - `can_move_passes_illusory` — set `walls[0][0].east = WallType::Illusory`, assert `true`.
  - `can_move_passes_one_way_from_passable_side` — set `walls[0][0].east = WallType::OneWay`, assert `floor.can_move(0, 0, Direction::East) == true`.
  - `can_move_blocks_one_way_from_solid_side` — set `walls[0][1].west = WallType::Solid` (the reverse side of the OneWay above), assert `floor.can_move(1, 0, Direction::West) == false`.
- [x] Add `wall_between` happy-path tests:
  - `wall_between_returns_east_wall_for_eastward_neighbor` — set `walls[0][0].east = WallType::Door`, assert `floor.wall_between((0, 0), (1, 0)) == WallType::Door`.
  - `wall_between_returns_solid_for_non_adjacent_pair` — assert `floor.wall_between((0, 0), (1, 1)) == WallType::Solid`.
  - `wall_between_returns_solid_for_out_of_bounds` — assert `floor.wall_between((5, 5), (6, 5)) == WallType::Solid`.
- [x] Run `cargo test data::dungeon::tests::can_move` — all 10 `can_move` tests pass.
- [x] Run `cargo test data::dungeon::tests::wall_between` — all 3 `wall_between` tests pass.

**Done state:** `can_move` and `wall_between` implemented; 13 unit tests cover the full matrix.

### Step 5: Add `Direction` rotation/offset tests

The existing test set covers `can_move` (which exercises `Direction` indirectly), but not the rotation/offset methods directly.

- [x] In the `#[cfg(test)] mod tests` block, add the following tests (each 2-4 lines):
  - `direction_turn_right_cycles` — assert `North.turn_right() == East`, `East.turn_right() == South`, `South.turn_right() == West`, `West.turn_right() == North`.
  - `direction_turn_left_cycles` — mirror: `North.turn_left() == West`, etc.
  - `direction_reverse_pairs` — `North.reverse() == South`, `South.reverse() == North`, `East.reverse() == West`, `West.reverse() == East`.
  - `direction_offset_is_y_down` — assert `North.offset() == (0, -1)`, `South.offset() == (0, 1)`, `East.offset() == (1, 0)`, `West.offset() == (-1, 0)`. Doc-comment the test referencing the y-down convention.
  - `direction_turn_right_is_inverse_of_turn_left` — for each variant, assert `dir.turn_right().turn_left() == dir` (round-trip property check).
  - `direction_reverse_is_self_inverse` — for each variant, assert `dir.reverse().reverse() == dir`.
- [x] Run `cargo test data::dungeon::tests::direction` — all 6 tests pass.
- [x] Run `cargo test --features dev data::dungeon::tests::direction` — same.

**Done state:** Six `Direction` method tests pass under both feature sets.

### Step 6: Replace `assets/dungeons/floor_01.dungeon.ron` with a hand-authored 6×6 test floor

The placeholder `()` is replaced with a layout that exercises every `WallType` variant, the four `TrapType` permutations needed (at least one `Pit`), one `spinner`, one teleporter, one dark zone, one anti-magic zone. Wall consistency must hold.

**Layout target (the implementer follows this map):**

```
       (0, _)    (1, _)    (2, _)    (3, _)    (4, _)    (5, _)
y=0:   [SSS]    [S.S]    [S.S]    [S.S]    [S.S]    [SSS]      <- top edge
y=1:   [S.S]    [S+S]    [SDS]    [S.L]    [S.S]    [S.S]
y=2:   [S.S]    [S.S]    [SsS]    [S.S]    [S.S]    [S.S]
y=3:   [S.S]    [S.S]    [SiS]    [SoS]    [S.S]    [S.S]
y=4:   [S.S]    [S.S]    [S.S]    [S.S]    [S?S]    [S.S]
y=5:   [SSS]    [S.S]    [S.S]    [S.S]    [S.S]    [SSS]      <- bottom edge

Legend:
  S = Solid wall on outer edge
  . = Open wall
  + = entry_point (1, 1, North)
  D = WallType::Door (between (1, 1) and (2, 1) — east of (1, 1))
  L = WallType::LockedDoor (between (3, 1) and (4, 1) — east of (3, 1))
  s = CellFeatures::spinner true
  i = WallType::Illusory (between (1, 3) and (2, 3) — west of (2, 3))
  o = WallType::OneWay (between (2, 3) and (3, 3) — east of (2, 3))
  ? = CellFeatures::trap Some(Pit { damage: 5, target_floor: Some(2) })
  T = teleporter at (5, 4) -> floor 2 (1, 1, South)
  k = SecretWall (between (4, 2) and (5, 2) — east of (4, 2))

Variant coverage requirements (the implementer verifies before committing):
  - WallType::Open: dozens of internal walls
  - WallType::Solid: outer perimeter (24 walls)
  - WallType::Door: 1 wall
  - WallType::LockedDoor: 1 wall
  - WallType::SecretWall: 1 wall
  - WallType::OneWay: 1 wall (and the reverse side stored as Solid)
  - WallType::Illusory: 1 wall
  - CellFeatures::trap: 1 cell with Pit { damage: 5, target_floor: Some(2) }
  - CellFeatures::spinner: 1 cell with `spinner: true`
  - CellFeatures::teleporter: 1 cell with Some(TeleportTarget { floor: 2, x: 1, y: 1, facing: Some(Direction::South) })
  - CellFeatures::dark_zone: 1 cell with `dark_zone: true`
  - CellFeatures::anti_magic_zone: 1 cell with `anti_magic_zone: true`
```

Concrete file shape (the implementer fills in cells per the map; this is the skeleton):

```ron
(
    name: "Test Floor 1",
    width: 6,
    height: 6,
    floor_number: 1,
    walls: [
        // Row y=0 (top, all SSS — entirely solid as the outer-perimeter top edge)
        [
            (north: Solid, south: Solid, east: Solid, west: Solid),
            // ... 5 more cells
        ],
        // Rows y=1..=4 — internal cells, mostly `(north: Solid, south: Open, east: Open, west: Solid)` with the specific variants above
        // Row y=5 (bottom, all SSS)
        // ...
    ],
    features: [
        // 6 rows x 6 cells. Empty cells are `()`; cells with one feature are e.g. `(spinner: true)`.
        [(), (), (), (), (), ()],
        // ...
    ],
    entry_point: (1, 1, North),
    encounter_table: "test_table",
)
```

- [x] Replace `/Users/nousunio/Repos/Learnings/claude-code/druum/assets/dungeons/floor_01.dungeon.ron` with the hand-authored 6×6 floor following the layout above. Use `()` for empty CellFeatures (the `#[serde(default)]` attribute on the struct lets this work).
- [x] Verify the file's outer perimeter is fully `Solid` so the player cannot walk off the edge.
- [x] Verify wall consistency by hand: every shared wall agrees between adjacent cells (e.g. if `walls[1][1].east = Door`, then `walls[1][2].west = Door`). The `OneWay` wall is the one allowed asymmetry — its reverse side may be `Solid`.
- [x] Add a unit test `floor_01_loads_and_is_consistent` in `src/data/dungeon.rs`'s `#[cfg(test)] mod tests`. Body: read the file via `let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/dungeons/floor_01.dungeon.ron"); let contents = std::fs::read_to_string(&path).expect("read floor_01");` then `let floor: DungeonFloor = ron::de::from_str(&contents).expect("parse floor_01");`. Assert `floor.is_well_formed()`, `floor.width == 6`, `floor.height == 6`, `floor.entry_point == (1, 1, Direction::North)`, `floor.validate_wall_consistency().is_ok()` (and panic with the inconsistency list if it fails — formatted via `{:?}` so the implementer can see which walls disagree).
- [x] Run `cargo test data::dungeon::tests::floor_01_loads_and_is_consistent`. If it fails, the panic message lists every wall that disagrees — fix the RON file based on that list. The validator is the authoring tool.
- [x] Run `cargo test data::dungeon::tests` — entire data::dungeon test suite passes.

**Done state:** `assets/dungeons/floor_01.dungeon.ron` is a valid, hand-authored 6×6 floor exercising every WallType variant and four CellFeatures variants. The `floor_01_loads_and_is_consistent` test parses it through `ron 0.12` (stdlib path) and asserts shape + consistency.

### Step 7: Add the App-level integration test in `tests/dungeon_floor_loads.rs`

This is the verification deferred from Feature #3's review. The integration test drives a Bevy `App` through `RonAssetPlugin::<DungeonFloor>` (the ron 0.11 path) and asserts the loaded `DungeonFloor` matches a known shape — closing the gap between the unit test (ron 0.12 path) and what production loads.

- [x] Create `/Users/nousunio/Repos/Learnings/claude-code/druum/tests/` directory if it does not exist (it is not currently in the repo).
- [x] Create `/Users/nousunio/Repos/Learnings/claude-code/druum/tests/dungeon_floor_loads.rs`. Body, following the pattern from `bevy_asset_loader-0.26.0/tests/multiple_asset_collections.rs:8-46` adapted for RON:
  ```rust
  //! App-level integration test for Feature #4. Loads `floor_01.dungeon.ron`
  //! through `bevy_common_assets::RonAssetPlugin` (the `ron 0.11` parser path)
  //! and asserts the resulting `DungeonFloor` matches the hand-authored shape
  //! in the asset file.
  //!
  //! This is the verification deferred from Feature #3's code review
  //! (`.claude/agent-memory/code-reviewer/feedback_ron_version_split.md`).
  //! The unit-level round-trip test in `src/data/dungeon.rs` exercises only
  //! the `ron 0.12` (project-direct) path; this integration test exercises
  //! the `ron 0.11` (loader-internal, via `bevy_common_assets`) path.

  use bevy::app::AppExit;
  use bevy::asset::AssetPlugin;
  use bevy::prelude::*;
  use bevy::state::app::StatesPlugin;
  use bevy_asset_loader::prelude::*;
  use bevy_common_assets::ron::RonAssetPlugin;
  use druum::data::DungeonFloor;
  use druum::data::dungeon::Direction;

  #[derive(AssetCollection, Resource)]
  struct TestAssets {
      #[asset(path = "dungeons/floor_01.dungeon.ron")]
      floor: Handle<DungeonFloor>,
  }

  #[derive(Clone, Eq, PartialEq, Debug, Hash, Default, States)]
  enum TestState {
      #[default]
      Loading,
      Loaded,
  }

  #[test]
  fn floor_01_loads_through_ron_asset_plugin() {
      App::new()
          .add_plugins((
              MinimalPlugins,
              AssetPlugin::default(),
              StatesPlugin,
              RonAssetPlugin::<DungeonFloor>::new(&["dungeon.ron"]),
          ))
          .init_state::<TestState>()
          .add_loading_state(
              LoadingState::new(TestState::Loading)
                  .continue_to_state(TestState::Loaded)
                  .load_collection::<TestAssets>(),
          )
          .add_systems(Update, timeout.run_if(in_state(TestState::Loading)))
          .add_systems(OnEnter(TestState::Loaded), assert_floor_shape)
          .run();
  }

  fn timeout(time: Res<Time>) {
      if time.elapsed_secs_f64() > 30.0 {
          panic!("DungeonFloor did not load in 30 seconds — RonAssetPlugin path likely broken");
      }
  }

  fn assert_floor_shape(
      assets: Res<TestAssets>,
      floors: Res<Assets<DungeonFloor>>,
      mut exit: MessageWriter<AppExit>, // Bevy 0.18: Message, not Event
  ) {
      let floor = floors
          .get(&assets.floor)
          .expect("DungeonFloor handle should be loaded by now");
      // Spot-check the same fields the unit test asserts. If the ron 0.11 loader
      // and the ron 0.12 unit test ever diverge, this assertion fires.
      assert_eq!(floor.width, 6);
      assert_eq!(floor.height, 6);
      assert_eq!(floor.entry_point, (1, 1, Direction::North));
      assert!(floor.is_well_formed(), "DungeonFloor failed is_well_formed");
      assert!(
          floor.validate_wall_consistency().is_ok(),
          "DungeonFloor failed wall consistency: {:?}",
          floor.validate_wall_consistency()
      );
      exit.write(AppExit::Success);
  }
  ```
- [x] If the public path `druum::data::dungeon::Direction` is not visible from the integration test, add `pub use dungeon::{Direction, WallType, WallMask, CellFeatures, TrapType, TeleportTarget};` to `src/data/mod.rs` alongside the existing `pub use dungeon::DungeonFloor;` re-export. (The data-module re-export pattern is the established Feature #3 exception per `project_druum_asset_pipeline.md`.) Note in Implementation Discoveries if this is needed.
- [x] Run `cargo test --test dungeon_floor_loads` — the integration test passes within ~1-2 seconds (MinimalPlugins doesn't include the renderer, so it's fast).
- [x] Run `cargo test --test dungeon_floor_loads --features dev` — same. (No dev-only systems are involved here, but hot-reload composition must not break the test.)
- [x] Run `cargo clippy --all-targets -- -D warnings` — no warnings (clippy now also checks the integration-test target).
- [x] Run `cargo clippy --all-targets --features dev -- -D warnings` — same.

**Done state:** `tests/dungeon_floor_loads.rs` exists and passes under both feature sets. The ron 0.11 loader-path is now end-to-end-tested. `feedback_ron_version_split.md` is materially addressed.

### Step 8: Final verification — run all 6 commands

Execute the full verification gate. Each command must complete with the expected output (zero warnings, all tests pass).

- [x] Run `cargo check`. Expected: `Finished` with zero warnings, zero errors.
- [x] Run `cargo check --features dev`. Expected: `Finished` with zero warnings, zero errors.
- [x] Run `cargo clippy --all-targets -- -D warnings`. Expected: `Finished` with zero warnings; `-D warnings` ensures any warning would fail. All targets covered: lib, bins, integration tests.
- [x] Run `cargo clippy --all-targets --features dev -- -D warnings`. Expected: same.
- [x] Run `cargo test`. Expected: All tests pass — count summary: ~25 tests in `data::dungeon::tests` (round-trip ×2, validate_wall_consistency ×4, can_move ×10, wall_between ×3, direction ×6, floor_01_loads_and_is_consistent ×1) plus 1 integration test in `dungeon_floor_loads`. Total ~26-28 tests passing across the project.
- [x] Run `cargo test --features dev`. Expected: same test count, same pass status. (Feature #4 introduces no dev-only tests; the dev-features pass exists to verify symmetric compilation.)
- [x] Confirm `Cargo.toml` has not changed. Run `git diff Cargo.toml` — must be empty.
- [x] Confirm `Cargo.lock` has not changed. Run `git diff Cargo.lock` — must be empty (no new deps added; the existing `serde`, `ron`, `bevy_*` set fully covers Feature #4).

**Done state:** All 6 verification commands green with zero warnings. `Cargo.toml` and `Cargo.lock` unchanged. `assets/dungeons/floor_01.dungeon.ron` loads end-to-end through both `ron 0.12` (stdlib unit test path) and `ron 0.11` (RonAssetPlugin integration-test path), producing the same `DungeonFloor` shape.

## Test Inventory

Each test is named (under the existing `data::dungeon::tests` module unless noted) with a one-line statement of what it asserts. The implementer fills in the bodies per the steps above.

**`src/data/dungeon.rs` `#[cfg(test)] mod tests` (unit tests, ron 0.12 path):**

| Test name | Asserts |
|---|---|
| `dungeon_floor_round_trips_through_ron` | (KEEP) `Default::default()` round-trips via `ron 0.12` (stdlib) |
| `dungeon_floor_round_trips_with_real_data` | (NEW) hand-built 2×2 floor with multi-variant walls/features round-trips |
| `validate_wall_consistency_passes_on_well_formed` | (NEW) symmetric 2×2 returns `Ok(())` |
| `validate_wall_consistency_detects_east_west_mismatch` | (NEW) east/west mismatch returns `Err` with one inconsistency |
| `validate_wall_consistency_detects_north_south_mismatch` | (NEW) south/north mismatch returns `Err` with one inconsistency |
| `validate_wall_consistency_allows_one_way_asymmetry` | (NEW) OneWay/Solid pair returns `Ok(())` |
| `can_move_returns_false_when_x_out_of_bounds` | (NEW) `x >= width` returns false |
| `can_move_returns_false_when_y_out_of_bounds` | (NEW) `y >= height` returns false |
| `can_move_blocks_solid` | (NEW) `Solid` wall returns false |
| `can_move_passes_open` | (NEW) `Open` wall returns true |
| `can_move_passes_door` | (NEW) `Door` wall returns true (asset-level; #13 handles animation) |
| `can_move_blocks_locked_door` | (NEW) `LockedDoor` returns false (key check is in #7) |
| `can_move_blocks_secret_wall` | (NEW) `SecretWall` returns false (discovery is in #13) |
| `can_move_passes_illusory` | (NEW) `Illusory` returns true |
| `can_move_passes_one_way_from_passable_side` | (NEW) `OneWay` on the source side returns true |
| `can_move_blocks_one_way_from_solid_side` | (NEW) reverse side stored as `Solid` returns false |
| `wall_between_returns_east_wall_for_eastward_neighbor` | (NEW) returns the wall on `a`'s east side for east neighbor |
| `wall_between_returns_solid_for_non_adjacent_pair` | (NEW) diagonals return `Solid` |
| `wall_between_returns_solid_for_out_of_bounds` | (NEW) out-of-bounds returns `Solid` |
| `direction_turn_right_cycles` | (NEW) right rotation cycles N→E→S→W→N |
| `direction_turn_left_cycles` | (NEW) left rotation cycles N→W→S→E→N |
| `direction_reverse_pairs` | (NEW) reverse swaps N↔S, E↔W |
| `direction_offset_is_y_down` | (NEW) North=(0,-1), South=(0,1), East=(1,0), West=(-1,0) |
| `direction_turn_right_is_inverse_of_turn_left` | (NEW) round-trip property check |
| `direction_reverse_is_self_inverse` | (NEW) `dir.reverse().reverse() == dir` |
| `floor_01_loads_and_is_consistent` | (NEW) `floor_01.dungeon.ron` parses via stdlib `ron 0.12` and is consistent |

**`tests/dungeon_floor_loads.rs` (integration test, ron 0.11 path):**

| Test name | Asserts |
|---|---|
| `floor_01_loads_through_ron_asset_plugin` | (NEW) `floor_01.dungeon.ron` loads through `RonAssetPlugin` (ron 0.11) end-to-end and matches the unit-test's expected shape |

**Total new tests:** 25 unit + 1 integration = 26 (plus the 1 existing test kept for orthogonal coverage = 27 total).

## Security

**Known vulnerabilities:**

No advisories found for `ron 0.11.0`, `ron 0.12.1`, `bevy_common_assets 0.16.0`, `bevy_asset_loader 0.26.0`, or `serde 1` in the extracted CHANGELOGs or in this session's training data. The `ron-0.11.0/tests/307_stack_overflow.rs` test confirms recursion-limit guarding is in place against deeply-nested adversarial RON. (Implementer should run `cargo audit` after Step 8 if available; no `Cargo.lock` changes are expected, so the audit result should be unchanged from Feature #3's state.)

**Architectural risks:**

- **Parser DoS via deeply-nested `Vec<Vec<...>>` in adversarial RON:** mitigated by Bevy's `AssetPlugin::default()` + `UnapprovedPathMode::Forbid` (already enforced in `main.rs` per Feature #3) plus ron's stack-recursion limit. Trust boundary: only files inside `assets/` can be loaded; floor files ship with the binary and are not user-supplied at runtime.
- **Path traversal via `event_id: Option<String>`:** `event_id` is not resolved as a filesystem path in this feature. Feature #13 (which will consume `event_id` for scripted triggers) MUST resolve event IDs against a compile-time allow-list, never as a path or shell command. Add a doc comment on the field flagging this constraint for future implementers.
- **Untrusted save-data importing a `DungeonFloor`:** out of scope here, but Feature #23's save format must reference floors by index/path, not round-trip the entire `DungeonFloor` value. Saves should store only mutable per-run state (party position, discovered cells, etc.) and load floors fresh from `assets/`.

**Trust boundaries (this feature):**

- `assets/dungeons/floor_01.dungeon.ron` is a trusted input — shipped with the binary, fixed at build time. The RON parser handles malformed input via `Result`, not panic. Bevy's `UnapprovedPathMode::Forbid` (Feature #3) blocks loads outside `assets/`.

No new trust boundaries are added in Feature #4.

## Risk Register

1. **Wall-consistency authoring risk.** The hand-written 6×6 `floor_01.dungeon.ron` has 60+ shared walls; one typo flips a wall on one side and not the other, breaking `validate_wall_consistency`. Mitigation: the `floor_01_loads_and_is_consistent` unit test runs the validator at `cargo test` time and prints the inconsistency list; the implementer iterates against that list until `Ok(())`. The validator IS the authoring tool. (Confidence: HIGH that this catches all asymmetries except OneWay-allowed ones.)

2. **ron 0.11 vs ron 0.12 emit-drift risk.** Today the format is byte-identical for every Feature #4 type shape (verified by direct diff of test files in both versions). If a future ron release changes one version's emit and not the other, the unit-level round-trip would still pass while the integration test would catch the regression. Mitigation: the `tests/dungeon_floor_loads.rs` integration test exercises the ron 0.11 path explicitly and is the single guard against this drift. (Confidence: HIGH that the test catches the failure mode; LOW that the failure mode actually materializes given how both libraries are mature.)

3. **Reflect-derive cost on `Vec<Vec<WallMask>>` at app startup.** `bevy_reflect` registers each generic instantiation in the type registry on app build. For `Vec<Vec<WallMask>>` (one nested vec) and the field-deep types (`Option<TrapType>`, `Option<TeleportTarget>`, `Option<String>`), there's some compile-time and tiny startup-time cost. Research RQ7 verified directly against `bevy_reflect-0.18.1/src/impls/alloc/vec.rs:10-20` that the blanket impl handles this; no `#[reflect(...)]` attributes are needed. (Confidence: HIGH; no action required.)

4. **Edition-2024 transitive-crate-name risk.** Feature #3 discovered that Rust 2024 edition does NOT allow direct references to transitively-pulled crates in source — `serde` and `ron` had to be added explicitly. Both are already declared in `Cargo.toml` (lines 26-27). Feature #4 introduces no new transitive crate references; the only crates touched in source are `bevy`, `serde`, `ron`, `bevy_asset_loader`, `bevy_common_assets`, all already declared. Mitigation: Step 8 explicitly verifies `Cargo.toml` is unchanged. (Confidence: HIGH no new declaration is needed.)

5. **Forgetting `#[serde(default)]` on `CellFeatures`.** Without it, every cell's RON entry must enumerate every field — empty cells balloon to ~7 lines instead of `()`. Mitigation: Step 1 explicitly adds the attribute; the `floor_01.dungeon.ron` layout in Step 6 uses `()` for empty cells — if the attribute is missing, parsing fails immediately and the implementer cannot proceed past Step 6. (Confidence: HIGH that this is caught at Step 6.)

6. **Pattern 2's `can_move` returns true on `Open | Illusory` only; this plan extends to `Open | Door | Illusory | OneWay`.** This is a deliberate divergence from §Pattern 2 per research RQ4. The risk is reviewers expecting strict §Pattern 2 conformance. Mitigation: the doc comment on `can_move` (added in Step 1) documents the full matrix verbatim, and Step 4's tests are exhaustive over the matrix. Reviewers can check the doc comment against the test names. (Confidence: HIGH.)

7. **`(y, x)` row-major addressing confusion.** Roadmap line 261 acknowledges this convention is unfamiliar to people used to `(x, y)`. Mitigation: every grid access in `dungeon.rs` is encapsulated in a method (`can_move`, `wall_between`, `validate_wall_consistency`); there is no `pub fn cell_at(...)` that would let external consumers hand-index. The `Direction::offset()` doc comment + the `Direction` enum doc comment both call out y-down explicitly. Feature #7 (movement) and #10 (auto-map) consume `Direction::offset` directly — they don't reach into `walls` themselves. (Confidence: HIGH that consumers stay on the rails.)

8. **`bevy_asset_loader 0.26.0`'s integration test pattern depends on `MinimalPlugins` running an event loop.** Research cites `bevy_asset_loader-0.26.0/tests/multiple_asset_collections.rs:8-46` as the canonical pattern, and the test relies on `App::run()` returning when `MessageWriter<AppExit>::write(AppExit::Success)` is called from within `OnEnter(TestState::Loaded)`. The `timeout` system's 30-second guard catches hangs. Mitigation: the integration test uses the canonical pattern verbatim; the timeout guard (Step 7) prevents indefinite hangs. (Confidence: HIGH; the pattern is upstream-tested.)

## Open Questions

All open questions from the research are resolved with the researcher's recommended defaults applied.

- **A1 (round-trip test scope) — Resolved:** Keep both — a `Default::default()` round-trip plus a hand-built non-trivial 2×2 round-trip. Cheap, orthogonal, covers both serde-symmetry-on-empty-collections and serde-symmetry-on-real-data. (Step 2.)
- **B1 (floor_01 6×6 layout) — Resolved:** Concrete layout specified in Step 6. Exercises every WallType variant (Open, Solid, Door, LockedDoor, SecretWall, OneWay, Illusory), four CellFeatures variants (trap=Pit, spinner, teleporter, dark_zone, anti_magic_zone), and all four `Direction` values (entry_point facing North; teleporter target facing South). Outer perimeter all `Solid`. Wall consistency holds. (Step 6.)
- **B2 (`validate_floor` umbrella method) — Resolved:** Do NOT add an umbrella. `is_well_formed()` and `validate_wall_consistency()` stay as separate methods — each does one thing and is testable independently. An umbrella `validate_floor()` would be a kitchen-sink that hides which check failed. (Step 1.)
- **B3 (Eq on DungeonFloor) — Resolved:** Cannot derive — `f32 encounter_rate` blocks `Eq`. `PartialEq` is sufficient for the integration-test asserts. Not added. (Step 1.)
- **B4 (test placement) — Resolved:** Unit tests in-file (`#[cfg(test)] mod tests`); integration test in `tests/dungeon_floor_loads.rs`. Standard Rust convention. (Steps 2-7.)
- **B5 (RON file naming/compression) — Resolved:** Plain RON. Compression is a build-time concern, not Feature #4. (Step 6.)
- **C1 (KeyId on Door, unify Door/LockedDoor) — Resolved:** Keep §Pattern 2's split — `WallType::Door` and `WallType::LockedDoor` are separate. Adding `KeyId` is Feature #12 territory and would force every door (locked or not) to carry the field. If #12 finds the split painful, refactor then. (Step 1.)
- **C2 (event_id String vs newtype) — Resolved:** `Option<String>` for now. Newtype refactor is one line in Feature #13 with a `From<String>` impl. (Step 1.)
- **Naming conflict — `anti_magic` vs `anti_magic_zone`:** Use `anti_magic_zone: bool` (research RQ3 "Recommended final shape"), matching `dark_zone: bool` symmetry within the same struct. The original §Pattern 2 used `anti_magic`; the research's refinement is preferred. Documented in Step 1. (Resolved by planner; researcher's recommendation applied.)
- **Naming conflict — `turn_left/turn_right/reverse` (research) vs `rotate_left/rotate_right/rotate_180` (user task):** Use research's `turn_left/turn_right/reverse` per task brief direction (Wizardry vocabulary, matches §Pattern 2). Documented in Step 1, Step 5. (Resolved by planner per task-brief recommendation.)

## Implementation Discoveries

**D1: `Direction` requires `Default` derive for `DungeonFloor::Default`.**

The plan specified `Direction` derives as `Reflect, Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash` — without `Default`. Because `DungeonFloor::entry_point` is typed `(u32, u32, Direction)`, the `#[derive(Default)]` on `DungeonFloor` requires `Direction: Default` (tuples derive `Default` only when all elements do). Added `#[derive(..., Default)]` with `#[default] North` to `Direction`. This is correct — `Direction::North` is the natural default (matches RON row 0 = top = "north" screen origin). No downstream impact on any test.

**D2: `#[serde(default)]` ordering on `CellFeatures`.**

The plan specified `#[serde(default)]` "placed AFTER the derive, BEFORE the struct definition", but the first compile attempt placed it before `#[derive(…)]`, triggering compiler warning `legacy_derive_helpers` (error E0277 pre-empted it). The correct ordering in Rust is `#[derive(…)]` first, then `#[serde(default)]`, then `pub struct`. Fixed immediately; no functional impact.

**D3: `src/data/mod.rs` re-export expanded.**

The plan noted "if `druum::data::dungeon::Direction` is not visible from the integration test, add re-exports." The direct path `druum::data::dungeon::Direction` is technically accessible because `dungeon` is `pub mod`, but adding explicit `pub use dungeon::{…}` re-exports to `mod.rs` is cleaner and matches the established Feature #3 pattern. Applied proactively. The integration test uses `druum::data::dungeon::Direction` directly (not the `druum::data::Direction` shortcut) to match the plan's test skeleton verbatim.

**D4: RON format compatibility — ron 0.11 vs ron 0.12 — confirmed compatible.**

The integration test (`tests/dungeon_floor_loads.rs`) exercises the ron 0.11 loader path through `bevy_common_assets::RonAssetPlugin`. The unit test exercises ron 0.12 via stdlib `ron::de::from_str`. Both paths parsed `floor_01.dungeon.ron` correctly on the first try with zero format-related errors. No quirks observed for `WallType` (unit enum variants), `TrapType` (struct variant `Pit { damage, target_floor }`), `TeleportTarget` (named struct), `CellFeatures` (struct with `#[serde(default)]`), or `Direction` (unit enum). The ron 0.11 / 0.12 concern from Feature #3's review (`feedback_ron_version_split.md`) is now materially addressed — format drift did not materialize for any type shape introduced in Feature #4.

**D5: `validate_wall_consistency` zero-cost for authoring.**

The hand-authored 6×6 floor passed wall consistency on the first parse attempt without any iteration against the validator's error output. The outer-perimeter design (row y=0 and y=5 entirely Solid on all four faces, inner rows using Solid only on their outer faces) made symmetry straightforward to maintain manually.

## Verification

- [x] All types compile with the specified derives — `cargo check` — Automatic
- [x] All types compile under dev features — `cargo check --features dev` — Automatic
- [x] No clippy warnings (default features, all targets including integration tests) — `cargo clippy --all-targets -- -D warnings` — Automatic
- [x] No clippy warnings (dev features, all targets) — `cargo clippy --all-targets --features dev -- -D warnings` — Automatic
- [x] All unit tests pass — `cargo test data::dungeon::tests` — Automatic — 26 tests passing
- [x] All integration tests pass — `cargo test --test dungeon_floor_loads` — Automatic — 1 test passing
- [x] All tests pass under default features — `cargo test` — Automatic — 27 unit + 1 integration = 28 tests passing
- [x] All tests pass under dev features — `cargo test --features dev` — Automatic — 28 unit + 1 integration = 29 tests passing (2 extra dev-only state-machine tests)
- [x] `Cargo.toml` is unchanged — `git diff Cargo.toml` outputs nothing — Automatic
- [x] `Cargo.lock` is unchanged — `git diff Cargo.lock` outputs nothing — Automatic
- [x] `floor_01.dungeon.ron` covers every WallType variant — verified: Open (interior), Solid (perimeter), Door, LockedDoor, SecretWall, OneWay, Illusory — Manual
- [x] `floor_01.dungeon.ron` covers at least 4 CellFeatures variants (trap, spinner, teleporter, dark_zone, anti_magic_zone) — all 5 covered — Manual
- [x] The `Direction` enum doc comment documents y-down convention verbatim per RQ5 — confirmed in source — Manual
- [x] The `can_move` doc comment documents the WallType matrix per RQ4 — confirmed in source — Manual
- [ ] `cargo audit` (if available) reports zero advisories — `cargo audit` — Manual (not all dev environments have it installed)

## Plan Completion Criteria

The plan is complete when:

1. All 8 steps are checked off.
2. All 6 verification commands (`cargo check`, `cargo check --features dev`, `cargo clippy --all-targets -- -D warnings`, `cargo clippy --all-targets --features dev -- -D warnings`, `cargo test`, `cargo test --features dev`) report success with zero warnings.
3. `Cargo.toml` and `Cargo.lock` are byte-unchanged from HEAD.
4. `assets/dungeons/floor_01.dungeon.ron` is a non-empty hand-authored 6×6 floor that exercises every WallType variant and 4+ CellFeatures variants, and `floor.validate_wall_consistency().is_ok()`.
5. The integration test `tests/dungeon_floor_loads.rs::floor_01_loads_through_ron_asset_plugin` passes — the deferred `feedback_ron_version_split.md` concern is materially addressed end-to-end.
6. The Implementation Discoveries section is populated with any deviations, wrong assumptions, or API quirks encountered during execution.
