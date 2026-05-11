//! Enemy ECS components — Feature #15 minimal shape.
//!
//! Enemies REUSE `BaseStats`, `DerivedStats`, `StatusEffects`, `PartyRow`,
//! `Equipment`, `Experience` from `plugins::party::character` and
//! `plugins::party::inventory`. The discriminator is the `Enemy` marker;
//! `PartyMember` is its inverse.
//!
//! `EnemyBundle` includes `Equipment::default()` and `Experience::default()`
//! to satisfy the dropped-`With<PartyMember>` filter in
//! `recompute_derived_stats_on_equipment_change` (D-A5 carve-out, Pitfall 11).
//!
//! Real enemy authoring (asset-driven `EnemyDb` populated from
//! `enemies.ron`) lands in #17. v1 hardcodes 2 placeholder enemies in the
//! `#[cfg(feature = "dev")] spawn_dev_encounter` helper.

use bevy::prelude::*;

use crate::plugins::combat::ai::EnemyAi;
use crate::plugins::combat::enemy_render::{EnemyAnimation, EnemyVisual};
use crate::plugins::party::Equipment;
use crate::plugins::party::character::{
    BaseStats, DerivedStats, Experience, PartyRow, StatusEffects,
};

/// Zero-sized marker on enemy entities.
#[derive(Component, Reflect, Default, Debug, Clone, Copy)]
pub struct Enemy;

/// Display name for enemy entities (rendered in egui combat screen).
#[derive(Component, Reflect, Default, Debug, Clone)]
pub struct EnemyName(pub String);

/// Index within the encounter (0..N). Used for speed tie-break (Decision 14).
#[derive(Component, Reflect, Default, Debug, Clone, Copy)]
pub struct EnemyIndex(pub u32);

/// Enemy entity spawn bundle. Includes `Equipment::default()` and
/// `Experience::default()` to satisfy the (now `PartyMember`-less)
/// recompute query (D-A5 carve-out).
///
/// `visual` and `animation` are populated by Feature #17. `EnemyVisual.id`
/// is empty by default; `combat/encounter.rs` populates it from `EnemySpec.id`
/// after the spawn.
#[derive(Bundle, Default)]
pub struct EnemyBundle {
    pub marker: Enemy,
    pub name: EnemyName,
    pub index: EnemyIndex,
    pub base_stats: BaseStats,
    pub derived_stats: DerivedStats,
    pub status_effects: StatusEffects,
    pub party_row: PartyRow,
    pub equipment: Equipment,
    pub experience: Experience,
    pub ai: EnemyAi,
    // Feature #17 additions:
    pub visual: EnemyVisual,
    pub animation: EnemyAnimation,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enemy_bundle_default_is_alive_marker() {
        let b = EnemyBundle::default();
        assert_eq!(b.derived_stats.current_hp, 0);
        assert_eq!(b.index.0, 0);
    }
}
