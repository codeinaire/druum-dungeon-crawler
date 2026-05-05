//! Class table schema — FROZEN-from-day-one.
//!
//! Feature #11 fleshed out the schema; subsequent features should not edit
//! in passing — schema changes require their own research + plan round.
//!
//! **Reverse-dep note:** This file imports `Class` and `BaseStats` from
//! `src/plugins/party/character.rs`. That is a one-way dependency inversion
//! (`data/` imports from `plugins/`) — intentional, documented in the #11
//! plan §Critical and in `character.rs`'s file-level doc. The alternative
//! (moving `Class` and `BaseStats` into `data/`) was rejected because it
//! would split character types arbitrarily across two locations.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

// Reverse dep: data/ imports from plugins/. See file-level doc above.
use crate::plugins::party::character::{BaseStats, Class};

/// A typed RON asset containing all authored class definitions.
///
/// Loaded from `assets/classes/core.classes.ron` by
/// `RonAssetPlugin::<ClassTable>::new(&["classes.ron"])` (registered in
/// `LoadingPlugin`).
///
/// `get` uses a linear scan over `Vec<ClassDef>` (O(n=8)) — see the
/// `get` doc for the rationale vs. `HashMap`.
#[derive(Asset, Reflect, Serialize, Deserialize, Default, Debug, Clone, PartialEq)]
pub struct ClassTable {
    pub classes: Vec<ClassDef>,
}

impl ClassTable {
    /// Look up the `ClassDef` for a `Class` variant by linear scan.
    ///
    /// Returns `None` when the variant has no authored entry (e.g., `Thief`
    /// in v1). The 8-class roster makes a `HashMap` unnecessary; `Vec::iter()
    /// .find()` is O(n=8) and trivially fast.
    ///
    /// **`bevy::utils::HashMap` is removed in 0.18** (research §Pitfall 4) —
    /// if a hot-path lookup is ever needed, use `std::collections::HashMap`.
    pub fn get(&self, class: Class) -> Option<&ClassDef> {
        self.classes.iter().find(|c| c.id == class)
    }
}

/// Per-class definition with deterministic per-level growth (no `rand`).
///
/// Field shape authored per Decision 8 (no `rand` crate, Δ deps = 0).
/// `starting_equipment` is omitted — items don't exist yet (Feature #12).
#[derive(Reflect, Serialize, Deserialize, Default, Debug, Clone, PartialEq)]
pub struct ClassDef {
    /// Discriminator: links this definition to the `Class` enum variant.
    pub id: Class,
    /// Human-readable name shown in UI.
    pub display_name: String,
    /// Stat values at level 1 before any equipment.
    pub starting_stats: BaseStats,
    /// Additive increase per level-up (applied by the level-up system in #14).
    pub growth_per_level: BaseStats,
    /// HP gained per level-up.
    pub hp_per_level: u32,
    /// MP gained per level-up.
    pub mp_per_level: u32,
    /// XP required to reach level 2 (base of the exponential curve).
    pub xp_to_level_2: u64,
    /// Multiplier applied per level in the XP curve:
    /// `xp_to_next = xp_to_level_2 * curve_factor ^ (level - 1)`.
    pub xp_curve_factor: f32,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fighter_def() -> ClassDef {
        ClassDef {
            id: Class::Fighter,
            display_name: "Fighter".into(),
            starting_stats: BaseStats {
                strength: 14,
                intelligence: 8,
                piety: 8,
                vitality: 14,
                agility: 10,
                luck: 9,
            },
            growth_per_level: BaseStats {
                strength: 2,
                intelligence: 0,
                piety: 0,
                vitality: 2,
                agility: 1,
                luck: 0,
            },
            hp_per_level: 8,
            mp_per_level: 0,
            xp_to_level_2: 100,
            xp_curve_factor: 1.5,
        }
    }

    fn mage_def() -> ClassDef {
        ClassDef {
            id: Class::Mage,
            display_name: "Mage".into(),
            starting_stats: BaseStats {
                strength: 7,
                intelligence: 14,
                piety: 7,
                vitality: 8,
                agility: 10,
                luck: 10,
            },
            growth_per_level: BaseStats {
                strength: 0,
                intelligence: 2,
                piety: 0,
                vitality: 1,
                agility: 1,
                luck: 1,
            },
            hp_per_level: 4,
            mp_per_level: 6,
            xp_to_level_2: 100,
            xp_curve_factor: 1.5,
        }
    }

    /// Round-trip a `ClassTable` with one `ClassDef` through RON.
    /// Verifies the serde derives are symmetric on the `ron 0.12` path.
    #[test]
    fn class_table_round_trips_through_ron() {
        let original = ClassTable {
            classes: vec![fighter_def()],
        };
        let serialized = ron::ser::to_string_pretty(&original, ron::ser::PrettyConfig::default())
            .expect("serialize ClassTable");
        let parsed: ClassTable = ron::de::from_str(&serialized).expect("deserialize ClassTable");
        assert_eq!(original, parsed, "ClassTable RON round-trip lost fields");
    }

    /// `get` returns the correct `ClassDef` for authored variants and `None`
    /// for declared-but-unauthored variants.
    #[test]
    fn class_table_get_returns_authored_class() {
        let table = ClassTable {
            classes: vec![fighter_def(), mage_def()],
        };
        assert!(
            table.get(Class::Fighter).is_some(),
            "Fighter should be found"
        );
        assert_eq!(table.get(Class::Fighter).unwrap().display_name, "Fighter");
        assert!(table.get(Class::Mage).is_some(), "Mage should be found");
        assert!(
            table.get(Class::Priest).is_none(),
            "Priest not in this table — should return None"
        );
    }
}
