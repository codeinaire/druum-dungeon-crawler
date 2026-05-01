//! Dungeon grid data model — razor-wall representation (Feature #4).
//!
//! Implements §Pattern 2 from the dungeon-crawler research with three
//! deliberate refinements: no `CellFeatures::Door` variant (doors live in
//! `WallType`), `#[serde(default)]` on `CellFeatures` so empty cells emit
//! as `()` in RON, and `PartialEq` on every type for integration-test asserts.
//!
//! See `project/research/20260501-220000-feature-4-dungeon-grid-data-model.md`.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// Cardinal direction in grid space.
///
/// **y-down convention:** `North = (0, -1)`. This matches the RON row layout
/// (`walls[0]` = top of the file = top of the screen = "north") and the
/// auto-map screen origin. Bevy's world coordinates are y-UP; the conversion
/// (`world_z = -grid_y * cell_size`) lives in Feature #8's renderer.
///
/// Offsets:
/// - `North = (0, -1)` — decreasing y
/// - `South = (0,  1)` — increasing y
/// - `East  = (1,  0)` — increasing x
/// - `West  = (-1, 0)` — decreasing x
#[derive(Reflect, Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Direction {
    #[default]
    North,
    South,
    East,
    West,
}

impl Direction {
    /// Turn left (counter-clockwise): North→West, West→South, South→East, East→North.
    pub fn turn_left(self) -> Self {
        match self {
            Direction::North => Direction::West,
            Direction::West => Direction::South,
            Direction::South => Direction::East,
            Direction::East => Direction::North,
        }
    }

    /// Turn right (clockwise): North→East, East→South, South→West, West→North.
    pub fn turn_right(self) -> Self {
        match self {
            Direction::North => Direction::East,
            Direction::East => Direction::South,
            Direction::South => Direction::West,
            Direction::West => Direction::North,
        }
    }

    /// Reverse direction: North↔South, East↔West.
    pub fn reverse(self) -> Self {
        match self {
            Direction::North => Direction::South,
            Direction::South => Direction::North,
            Direction::East => Direction::West,
            Direction::West => Direction::East,
        }
    }

    /// Returns `(dx, dy)`; y-down convention (see enum doc).
    pub fn offset(self) -> (i32, i32) {
        match self {
            Direction::North => (0, -1),
            Direction::South => (0, 1),
            Direction::East => (1, 0),
            Direction::West => (-1, 0),
        }
    }
}

/// The passability type of a single wall face.
///
/// `can_move` truth table:
/// - `Open | Door | Illusory | OneWay` → passable (returns `true`)
/// - `Solid | LockedDoor | SecretWall` → blocked (returns `false`)
///
/// Stored twice per shared wall (each adjacent cell carries its own copy).
/// `validate_wall_consistency` asserts symmetry; `OneWay` is the one allowed
/// asymmetry.
#[derive(Reflect, Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WallType {
    /// Fully open — no wall present.
    #[default]
    Open,
    /// Impassable solid wall.
    Solid,
    /// Closed but unlocked — `can_move` returns `true`.
    Door,
    /// Locked door — `can_move` returns `false` (key check is in Feature #7).
    LockedDoor,
    /// Hidden passage — `can_move` returns `false` until discovered (discovery
    /// state lives in a separate runtime resource, Feature #13).
    SecretWall,
    /// Passable from this side only. The reverse side is stored as `Solid`
    /// (the one allowed asymmetry in `validate_wall_consistency`).
    OneWay,
    /// Illusory wall — looks solid but `can_move` returns `true`.
    Illusory,
}

/// Per-cell wall data. Each field is the wall on that face of the cell.
///
/// Walls are stored twice: cell A's `east` == cell B's `west` for horizontally
/// adjacent cells (and south/north for vertically adjacent cells). The one
/// allowed asymmetry is `WallType::OneWay`.
#[derive(Reflect, Serialize, Deserialize, Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct WallMask {
    pub north: WallType,
    pub south: WallType,
    pub east: WallType,
    pub west: WallType,
}

/// Fixed teleport destination encoded in the dungeon asset.
///
/// No `Default` derive — a default `TeleportTarget` would encode floor 0 at
/// `(0, 0)`, which is a meaningless and potentially confusing value.
#[derive(Reflect, Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct TeleportTarget {
    pub floor: u32,
    pub x: u32,
    pub y: u32,
    /// If `None`, the player retains their current facing after teleport.
    pub facing: Option<Direction>,
}

/// Type of hazard present in a cell.
///
/// No `Default` derive — absence is represented by `Option<TrapType> = None`.
#[derive(Reflect, Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum TrapType {
    /// Fall pit. `damage` is HP lost on trigger; `target_floor` is the floor
    /// the player lands on (or `None` for a dead-end pit).
    Pit { damage: u32, target_floor: Option<u32> },
    /// Poison gas cloud — applies poison status.
    Poison,
    /// Alarm trap — triggers a scripted encounter.
    Alarm,
    /// Teleport trap — moves the player to the embedded destination.
    Teleport(TeleportTarget),
}

/// Optional special properties of a dungeon cell.
///
/// `#[serde(default)]` allows empty cells to be written as `()` in RON
/// instead of enumerating every field. Without this attribute every cell
/// would need to spell out all seven fields, ballooning the floor file.
#[derive(Reflect, Serialize, Deserialize, Default, Debug, Clone, PartialEq)]
#[serde(default)]
pub struct CellFeatures {
    pub trap: Option<TrapType>,
    pub teleporter: Option<TeleportTarget>,
    /// §Resolved #4: telegraphed only — bool is sufficient.
    pub spinner: bool,
    /// Disables auto-map within this cell.
    pub dark_zone: bool,
    /// Disables spell casting within this cell.
    pub anti_magic_zone: bool,
    /// 0.0 = no random encounters, 1.0 = encounter every step.
    pub encounter_rate: f32,
    /// Optional scripted event identifier; resolved at runtime by Feature #13.
    ///
    /// SECURITY: Feature #13 MUST resolve event IDs against a compile-time
    /// allow-list, never as a filesystem path or shell command.
    pub event_id: Option<String>,
}

/// A single dungeon floor — the primary asset type loaded by `RonAssetPlugin`.
///
/// `walls` and `features` are indexed `[y][x]` (row-major, y-down).
/// All external access must go through `can_move`, `wall_between`, or
/// `is_well_formed` — never hand-index from outside this module.
#[derive(Asset, Reflect, Serialize, Deserialize, Default, Debug, Clone, PartialEq)]
pub struct DungeonFloor {
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub floor_number: u32,
    /// Walls stored as `[y][x]` grid. y-down screen convention.
    pub walls: Vec<Vec<WallMask>>,
    /// Cell features stored as `[y][x]` grid. Same shape as `walls`.
    pub features: Vec<Vec<CellFeatures>>,
    pub entry_point: (u32, u32, Direction),
    pub encounter_table: String,
}

impl DungeonFloor {
    /// Returns `true` if the player can move from cell `(x, y)` in direction
    /// `dir`.
    ///
    /// **Truth table:**
    /// - Out-of-bounds `(x, y)` → `false`
    /// - `Open | Door | Illusory | OneWay` → `true`
    /// - `Solid | LockedDoor | SecretWall` → `false`
    ///
    /// Does NOT consider player discovery state (Feature #13) or key inventory
    /// (Feature #7). Feature #7 layers a `can_move_with_discovery` wrapper.
    pub fn can_move(&self, x: u32, y: u32, dir: Direction) -> bool {
        if x >= self.width || y >= self.height {
            return false;
        }
        let cell = &self.walls[y as usize][x as usize];
        let wall = match dir {
            Direction::North => cell.north,
            Direction::South => cell.south,
            Direction::East => cell.east,
            Direction::West => cell.west,
        };
        matches!(
            wall,
            WallType::Open | WallType::Door | WallType::Illusory | WallType::OneWay
        )
    }

    /// Returns the `WallType` between two adjacent cells.
    ///
    /// Returns `WallType::Solid` for non-adjacent or out-of-bounds pairs.
    /// Uses cell `a`'s wall face in the direction of `b`.
    pub fn wall_between(&self, a: (u32, u32), b: (u32, u32)) -> WallType {
        if a.0 >= self.width || a.1 >= self.height {
            return WallType::Solid;
        }
        let dx = b.0 as i32 - a.0 as i32;
        let dy = b.1 as i32 - a.1 as i32;
        let dir = match (dx, dy) {
            (1, 0) => Direction::East,
            (-1, 0) => Direction::West,
            (0, 1) => Direction::South,
            (0, -1) => Direction::North,
            _ => return WallType::Solid,
        };
        let cell = &self.walls[a.1 as usize][a.0 as usize];
        match dir {
            Direction::North => cell.north,
            Direction::South => cell.south,
            Direction::East => cell.east,
            Direction::West => cell.west,
        }
    }

    /// Asserts wall double-storage symmetry across every adjacent cell pair.
    ///
    /// `OneWay` walls are the one allowed asymmetry — a `OneWay`/`Solid` pair
    /// does NOT produce an error.
    ///
    /// Returns `Ok(())` if all walls are consistent, `Err(Vec<WallInconsistency>)`
    /// listing every disagreeing pair.
    pub fn validate_wall_consistency(&self) -> Result<(), Vec<WallInconsistency>> {
        let mut errors: Vec<WallInconsistency> = Vec::new();

        for y in 0..self.height {
            for x in 0..self.width {
                // Check east-west pair with the cell to the right.
                if x + 1 < self.width {
                    let wall_a = self.walls[y as usize][x as usize].east;
                    let wall_b = self.walls[y as usize][(x + 1) as usize].west;
                    if !walls_consistent(wall_a, wall_b) {
                        errors.push(WallInconsistency {
                            cell_a: (x, y),
                            cell_b: (x + 1, y),
                            direction: Direction::East,
                            wall_a,
                            wall_b,
                        });
                    }
                }
                // Check south-north pair with the cell below.
                if y + 1 < self.height {
                    let wall_a = self.walls[y as usize][x as usize].south;
                    let wall_b = self.walls[(y + 1) as usize][x as usize].north;
                    if !walls_consistent(wall_a, wall_b) {
                        errors.push(WallInconsistency {
                            cell_a: (x, y),
                            cell_b: (x, y + 1),
                            direction: Direction::South,
                            wall_a,
                            wall_b,
                        });
                    }
                }
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    /// Returns `true` if `width`/`height` match the actual `Vec` dimensions.
    ///
    /// Default-constructed floors (zero-by-zero) return `true`.
    pub fn is_well_formed(&self) -> bool {
        self.walls.len() == self.height as usize
            && self.walls.iter().all(|row| row.len() == self.width as usize)
            && self.features.len() == self.height as usize
            && self.features.iter().all(|row| row.len() == self.width as usize)
    }
}

/// Describes a wall-symmetry mismatch detected by `validate_wall_consistency`.
#[derive(Debug, PartialEq)]
pub struct WallInconsistency {
    pub cell_a: (u32, u32),
    pub cell_b: (u32, u32),
    /// Direction from `cell_a` to `cell_b`.
    pub direction: Direction,
    /// Wall stored on `cell_a`'s face toward `cell_b`.
    pub wall_a: WallType,
    /// Wall stored on `cell_b`'s face toward `cell_a`.
    pub wall_b: WallType,
}

/// Returns `true` when the two wall values are consistent.
///
/// Two walls agree when they are equal, OR when either side is `OneWay`
/// (the one allowed asymmetry in the double-storage scheme).
fn walls_consistent(a: WallType, b: WallType) -> bool {
    a == b || a == WallType::OneWay || b == WallType::OneWay
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: build a minimal `DungeonFloor` with `w x h` cells, all walls
    /// defaulting to `WallType::Open`.
    fn make_floor(w: u32, h: u32) -> DungeonFloor {
        DungeonFloor {
            name: "test".into(),
            width: w,
            height: h,
            floor_number: 1,
            walls: vec![
                vec![WallMask::default(); w as usize];
                h as usize
            ],
            features: vec![
                vec![CellFeatures::default(); w as usize];
                h as usize
            ],
            entry_point: (0, 0, Direction::North),
            encounter_table: "test_table".into(),
        }
    }

    // -------------------------------------------------------------------------
    // Serde round-trip tests (ron 0.12 path)
    // -------------------------------------------------------------------------

    /// Round-trip a default `DungeonFloor` through RON and back.
    /// Verifies the serde derives are symmetric. Pure stdlib + ron 0.12 —
    /// no Bevy `App`, no `AssetServer`. Runs in <1 ms.
    #[test]
    fn dungeon_floor_round_trips_through_ron() {
        let original = DungeonFloor::default();

        let serialized: String = ron::ser::to_string_pretty(
            &original,
            ron::ser::PrettyConfig::default(),
        )
        .expect("serialize");

        let parsed: DungeonFloor = ron::de::from_str(&serialized).expect("deserialize");

        let reserialized: String = ron::ser::to_string_pretty(
            &parsed,
            ron::ser::PrettyConfig::default(),
        )
        .expect("re-serialize");

        assert_eq!(serialized, reserialized, "RON round trip lost or reordered fields");
    }

    /// Round-trip a hand-built 2×2 `DungeonFloor` with multiple WallType and
    /// CellFeatures variants through ron 0.12.
    #[test]
    fn dungeon_floor_round_trips_with_real_data() {
        let original = DungeonFloor {
            name: "test".into(),
            width: 2,
            height: 2,
            floor_number: 1,
            walls: vec![
                vec![
                    WallMask {
                        north: WallType::Solid,
                        south: WallType::Open,
                        east: WallType::Door,
                        west: WallType::Solid,
                    },
                    WallMask {
                        north: WallType::Solid,
                        south: WallType::Open,
                        east: WallType::Solid,
                        west: WallType::Door,
                    },
                ],
                vec![
                    WallMask {
                        north: WallType::Open,
                        south: WallType::Solid,
                        east: WallType::Open,
                        west: WallType::Solid,
                    },
                    WallMask {
                        north: WallType::Open,
                        south: WallType::Solid,
                        east: WallType::Solid,
                        west: WallType::Open,
                    },
                ],
            ],
            features: vec![
                vec![
                    CellFeatures::default(),
                    CellFeatures {
                        spinner: true,
                        ..Default::default()
                    },
                ],
                vec![
                    CellFeatures {
                        trap: Some(TrapType::Poison),
                        ..Default::default()
                    },
                    CellFeatures::default(),
                ],
            ],
            entry_point: (0, 0, Direction::North),
            encounter_table: "test_table".into(),
        };

        let serialized = ron::ser::to_string_pretty(&original, ron::ser::PrettyConfig::default())
            .expect("serialize");
        let parsed: DungeonFloor = ron::de::from_str(&serialized).expect("deserialize");
        assert_eq!(original, parsed, "round-trip changed the DungeonFloor value");
    }

    // -------------------------------------------------------------------------
    // validate_wall_consistency tests
    // -------------------------------------------------------------------------

    #[test]
    fn validate_wall_consistency_passes_on_well_formed() {
        let mut floor = make_floor(2, 2);
        // Set matching walls on the shared east/west boundary.
        floor.walls[0][0].east = WallType::Solid;
        floor.walls[0][1].west = WallType::Solid;
        // Set matching walls on the shared south/north boundary.
        floor.walls[0][0].south = WallType::Open;
        floor.walls[1][0].north = WallType::Open;
        assert!(floor.validate_wall_consistency().is_ok());
    }

    #[test]
    fn validate_wall_consistency_detects_east_west_mismatch() {
        let mut floor = make_floor(2, 2);
        floor.walls[0][0].east = WallType::Open;
        floor.walls[0][1].west = WallType::Solid;
        let result = floor.validate_wall_consistency();
        let errors = result.unwrap_err();
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].direction, Direction::East);
        assert_eq!(errors[0].cell_a, (0, 0));
        assert_eq!(errors[0].cell_b, (1, 0));
    }

    #[test]
    fn validate_wall_consistency_detects_north_south_mismatch() {
        let mut floor = make_floor(2, 2);
        floor.walls[0][0].south = WallType::Solid;
        floor.walls[1][0].north = WallType::Open;
        let result = floor.validate_wall_consistency();
        let errors = result.unwrap_err();
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].direction, Direction::South);
        assert_eq!(errors[0].cell_a, (0, 0));
        assert_eq!(errors[0].cell_b, (0, 1));
    }

    #[test]
    fn validate_wall_consistency_allows_one_way_asymmetry() {
        let mut floor = make_floor(2, 2);
        floor.walls[0][0].east = WallType::OneWay;
        floor.walls[0][1].west = WallType::Solid;
        assert!(floor.validate_wall_consistency().is_ok());
    }

    // -------------------------------------------------------------------------
    // can_move tests
    // -------------------------------------------------------------------------

    #[test]
    fn can_move_returns_false_when_x_out_of_bounds() {
        let floor = make_floor(2, 2);
        assert!(!floor.can_move(2, 0, Direction::East));
    }

    #[test]
    fn can_move_returns_false_when_y_out_of_bounds() {
        let floor = make_floor(2, 2);
        assert!(!floor.can_move(0, 2, Direction::South));
    }

    #[test]
    fn can_move_blocks_solid() {
        let mut floor = make_floor(2, 2);
        floor.walls[0][0].east = WallType::Solid;
        assert!(!floor.can_move(0, 0, Direction::East));
    }

    #[test]
    fn can_move_passes_open() {
        let mut floor = make_floor(2, 2);
        floor.walls[0][0].east = WallType::Open;
        assert!(floor.can_move(0, 0, Direction::East));
    }

    #[test]
    fn can_move_passes_door() {
        // Feature #13 will animate; asset-level check returns true.
        let mut floor = make_floor(2, 2);
        floor.walls[0][0].east = WallType::Door;
        assert!(floor.can_move(0, 0, Direction::East));
    }

    #[test]
    fn can_move_blocks_locked_door() {
        let mut floor = make_floor(2, 2);
        floor.walls[0][0].east = WallType::LockedDoor;
        assert!(!floor.can_move(0, 0, Direction::East));
    }

    #[test]
    fn can_move_blocks_secret_wall() {
        // Discovery is a runtime resource owned by Feature #13.
        let mut floor = make_floor(2, 2);
        floor.walls[0][0].east = WallType::SecretWall;
        assert!(!floor.can_move(0, 0, Direction::East));
    }

    #[test]
    fn can_move_passes_illusory() {
        let mut floor = make_floor(2, 2);
        floor.walls[0][0].east = WallType::Illusory;
        assert!(floor.can_move(0, 0, Direction::East));
    }

    #[test]
    fn can_move_passes_one_way_from_passable_side() {
        let mut floor = make_floor(2, 2);
        floor.walls[0][0].east = WallType::OneWay;
        assert!(floor.can_move(0, 0, Direction::East));
    }

    #[test]
    fn can_move_blocks_one_way_from_solid_side() {
        let mut floor = make_floor(2, 2);
        floor.walls[0][1].west = WallType::Solid;
        assert!(!floor.can_move(1, 0, Direction::West));
    }

    // -------------------------------------------------------------------------
    // wall_between tests
    // -------------------------------------------------------------------------

    #[test]
    fn wall_between_returns_east_wall_for_eastward_neighbor() {
        let mut floor = make_floor(2, 2);
        floor.walls[0][0].east = WallType::Door;
        assert_eq!(floor.wall_between((0, 0), (1, 0)), WallType::Door);
    }

    #[test]
    fn wall_between_returns_solid_for_non_adjacent_pair() {
        let floor = make_floor(2, 2);
        assert_eq!(floor.wall_between((0, 0), (1, 1)), WallType::Solid);
    }

    #[test]
    fn wall_between_returns_solid_for_out_of_bounds() {
        let floor = make_floor(2, 2);
        assert_eq!(floor.wall_between((5, 5), (6, 5)), WallType::Solid);
    }

    // -------------------------------------------------------------------------
    // Direction method tests
    // -------------------------------------------------------------------------

    #[test]
    fn direction_turn_right_cycles() {
        assert_eq!(Direction::North.turn_right(), Direction::East);
        assert_eq!(Direction::East.turn_right(), Direction::South);
        assert_eq!(Direction::South.turn_right(), Direction::West);
        assert_eq!(Direction::West.turn_right(), Direction::North);
    }

    #[test]
    fn direction_turn_left_cycles() {
        assert_eq!(Direction::North.turn_left(), Direction::West);
        assert_eq!(Direction::West.turn_left(), Direction::South);
        assert_eq!(Direction::South.turn_left(), Direction::East);
        assert_eq!(Direction::East.turn_left(), Direction::North);
    }

    #[test]
    fn direction_reverse_pairs() {
        assert_eq!(Direction::North.reverse(), Direction::South);
        assert_eq!(Direction::South.reverse(), Direction::North);
        assert_eq!(Direction::East.reverse(), Direction::West);
        assert_eq!(Direction::West.reverse(), Direction::East);
    }

    /// y-down convention: North decreases y, South increases y.
    #[test]
    fn direction_offset_is_y_down() {
        assert_eq!(Direction::North.offset(), (0, -1));
        assert_eq!(Direction::South.offset(), (0, 1));
        assert_eq!(Direction::East.offset(), (1, 0));
        assert_eq!(Direction::West.offset(), (-1, 0));
    }

    #[test]
    fn direction_turn_right_is_inverse_of_turn_left() {
        for dir in [
            Direction::North,
            Direction::South,
            Direction::East,
            Direction::West,
        ] {
            assert_eq!(dir.turn_right().turn_left(), dir);
        }
    }

    #[test]
    fn direction_reverse_is_self_inverse() {
        for dir in [
            Direction::North,
            Direction::South,
            Direction::East,
            Direction::West,
        ] {
            assert_eq!(dir.reverse().reverse(), dir);
        }
    }

    // -------------------------------------------------------------------------
    // floor_01 integration (ron 0.12 stdlib path)
    // -------------------------------------------------------------------------

    /// Parse `floor_01.dungeon.ron` through stdlib ron 0.12 and assert shape +
    /// wall consistency. The companion integration test
    /// (`tests/dungeon_floor_loads.rs`) covers the ron 0.11 loader path.
    #[test]
    fn floor_01_loads_and_is_consistent() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("assets/dungeons/floor_01.dungeon.ron");
        let contents = std::fs::read_to_string(&path).expect("read floor_01");
        let floor: DungeonFloor = ron::de::from_str(&contents).expect("parse floor_01");

        assert!(floor.is_well_formed(), "floor_01 is not well-formed");
        assert_eq!(floor.width, 6);
        assert_eq!(floor.height, 6);
        assert_eq!(floor.entry_point, (1, 1, Direction::North));
        assert!(
            floor.validate_wall_consistency().is_ok(),
            "floor_01 wall inconsistencies: {:?}",
            floor.validate_wall_consistency()
        );
    }
}
