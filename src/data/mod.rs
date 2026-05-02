//! Static game-data tables.
//!
//! Each typed RON-loaded schema lives in its own submodule:
//! - `dungeon` — `DungeonFloor` (Feature #4 fills in the razor-wall grid)
//! - `items` — `ItemDb` (Features #11/#12)
//! - `enemies` — `EnemyDb` (Features #11/#15)
//! - `classes` — `ClassTable` (Feature #19)
//! - `spells` — `SpellTable` (Feature #20)

pub mod classes;
pub mod dungeon;
pub mod enemies;
pub mod items;
pub mod spells;

pub use classes::ClassTable;
pub use dungeon::{
    CellFeatures, Direction, DungeonFloor, TeleportTarget, TrapType, WallInconsistency, WallMask,
    WallType,
};
pub use enemies::EnemyDb;
pub use items::ItemDb;
pub use spells::SpellTable;
