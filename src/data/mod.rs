//! Static game-data tables.
//!
//! Each typed RON-loaded schema lives in its own submodule:
//! - `dungeon` — `DungeonFloor` (Feature #4 fills in the razor-wall grid)
//! - `items` — `ItemDb`, `ItemAsset`, `ItemStatBlock` (Features #11/#12)
//! - `enemies` — `EnemyDb` (Feature #15)
//! - `classes` — `ClassTable`, `ClassDef` (Feature #11)
//! - `races` — `RaceTable`, `RaceData` (Feature #19)
//! - `spells` — `SpellTable` (Feature #20)
//! - `town` — `ShopStock`, `RecruitPool`, `TownServices` (Feature #18)

pub mod classes;
pub mod dungeon;
pub mod enemies;
pub mod encounters;
pub mod items;
pub mod races;
pub mod spells;
pub mod town;

pub use classes::{ClassDef, ClassRequirement, ClassTable};
pub use dungeon::{
    CellFeatures, ColorRgb, Direction, DungeonFloor, FogConfig, LightingConfig, TeleportTarget,
    TrapType, WallMask, WallType,
};
pub use encounters::{EncounterEntry, EncounterTable, EnemyGroup, EnemySpec};
pub use enemies::EnemyDb;
pub use items::{ItemAsset, ItemDb, ItemStatBlock};
pub use races::{RaceData, RaceTable};
pub use spells::SpellTable;
pub use town::{RecruitDef, RecruitPool, ShopEntry, ShopStock, TownServices};
