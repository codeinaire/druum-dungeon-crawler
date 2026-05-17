//! Static game-data tables.
//!
//! Each typed RON-loaded schema lives in its own submodule:
//! - `dungeon` — `DungeonFloor` (Feature #4 fills in the razor-wall grid)
//! - `items` — `ItemDb`, `ItemAsset`, `ItemStatBlock` (Features #11/#12)
//! - `enemies` — `EnemyDb` (Feature #15)
//! - `classes` — `ClassTable`, `ClassDef` (Feature #11)
//! - `races` — `RaceTable`, `RaceData` (Feature #19)
//! - `skills` — `SkillTree`, `SkillNode`, `NodeGrant` (Feature #20)
//! - `spells` — `SpellDb`, `SpellAsset`, `SpellEffect` (Feature #20 — spells registry)
//! - `town` — `ShopStock`, `RecruitPool`, `TownServices` (Feature #18)

pub mod classes;
pub mod dungeon;
pub mod enemies;
pub mod encounters;
pub mod items;
pub mod races;
pub mod skills;
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
pub use skills::{
    CycleError, MAX_SKILL_NODE_COST, MAX_SKILL_NODE_MIN_LEVEL, MAX_SKILL_TREE_NODES,
    NodeGrant, NodeId, SKILL_POINTS_PER_LEVEL, SkillNode, SkillTree,
    clamp_skill_tree, validate_no_cycles,
};
pub use spells::{
    SpellAsset, SpellDb, SpellEffect, SpellSchool, SpellTarget,
    KNOWN_SPELLS_MAX, MAX_SPELL_DAMAGE, MAX_SPELL_DURATION,
    MAX_SPELL_HEAL, MAX_SPELL_MP_COST, clamp_known_spells,
};
pub use town::{RecruitDef, RecruitPool, ShopEntry, ShopStock, TownServices};
