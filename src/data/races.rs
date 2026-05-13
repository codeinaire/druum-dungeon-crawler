//! Race table schema — Feature #19.
//!
//! **Reverse-dep note:** This file imports `BaseStats` and `Race` from
//! `src/plugins/party/character.rs`. That is a one-way dependency inversion
//! (`data/` imports from `plugins/`) — intentional, documented in the #11
//! plan §Critical and in `character.rs`'s file-level doc. The alternative
//! (moving `Race` and `BaseStats` into `data/`) was rejected because it would
//! split character types arbitrarily across two locations.
//!
//! ## `stat_modifiers` i16 encoding
//!
//! `BaseStats` stores each field as `u16`. Negative race modifiers are encoded
//! as the **two's-complement bit pattern** of the i16 value. For example:
//! - `-1` is stored as `65535` (0xFFFF).
//! - `-2` is stored as `65534` (0xFFFE).
//!
//! At the **apply site** (`allocate_bonus_pool` in `progression.rs`), each
//! field is reinterpreted with `field as i16` and applied via
//! `base.X.saturating_add_signed(modifier as i16)`. This avoids changing
//! `BaseStats` field types and keeps RON authoring straightforward for the
//! common non-negative case.
//!
//! **Document this contract wherever you read `stat_modifiers.X` as i16.**

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::plugins::party::character::{BaseStats, Race};

/// Per-race definition carrying signed stat offsets applied during character
/// creation (Feature #19, user decision Q3: 5 races, balanced -2..=+2 offsets).
///
/// ## `stat_modifiers` encoding
///
/// Each field is `u16` interpreted as `i16` via `field as i16` at the apply
/// site. Negative values are stored as two's-complement bit patterns:
/// - `-1 == 65535 (0xFFFF)`
/// - `-2 == 65534 (0xFFFE)`
///
/// Applied via `base.stat.saturating_add_signed(modifier as i16)` in
/// `allocate_bonus_pool` (progression.rs).
#[derive(Reflect, Serialize, Deserialize, Default, Debug, Clone, PartialEq)]
pub struct RaceData {
    /// Discriminator: links this definition to the `Race` enum variant.
    pub id: Race,
    /// Human-readable name shown in UI.
    pub display_name: String,
    /// Signed i16 stat offsets encoded as u16 bit-patterns. See module-level doc.
    pub stat_modifiers: BaseStats,
    /// Flavour description shown in the creation wizard.
    #[serde(default)]
    pub description: String,
}

/// A typed RON asset containing all authored race definitions.
///
/// Loaded from `assets/races/core.races.racelist.ron` by
/// `RonAssetPlugin::<RaceTable>::new(&["racelist.ron"])` (registered in
/// `LoadingPlugin`).
///
/// `get` uses a linear scan over `Vec<RaceData>` (O(n=5)) — same pattern
/// as `ClassTable::get`.
#[derive(Asset, Reflect, Serialize, Deserialize, Default, Debug, Clone, PartialEq)]
pub struct RaceTable {
    pub races: Vec<RaceData>,
}

impl RaceTable {
    /// Look up `RaceData` for a `Race` variant by linear scan.
    ///
    /// Returns `None` when the variant has no authored entry. The 5-race
    /// roster makes a `HashMap` unnecessary; `Vec::iter().find()` is O(n=5).
    pub fn get(&self, race: Race) -> Option<&RaceData> {
        self.races.iter().find(|r| r.id == race)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn human_data() -> RaceData {
        RaceData {
            id: Race::Human,
            display_name: "Human".into(),
            stat_modifiers: BaseStats::ZERO,
            description: "Balanced; no modifiers.".into(),
        }
    }

    fn elf_data() -> RaceData {
        RaceData {
            id: Race::Elf,
            display_name: "Elf".into(),
            // STR-1, INT+2, PIE+1, VIT-2, AGI+1, LCK-1
            stat_modifiers: BaseStats {
                strength: 65535, // -1 as u16
                intelligence: 2,
                piety: 1,
                vitality: 65534, // -2 as u16
                agility: 1,
                luck: 65535, // -1 as u16
            },
            description: "Magical and dexterous; physically frail.".into(),
        }
    }

    /// Round-trip a `RaceTable` through RON.
    /// Verifies the serde derives are symmetric on the `ron 0.12` path.
    #[test]
    fn race_table_round_trips_through_ron() {
        let original = RaceTable {
            races: vec![human_data(), elf_data()],
        };
        let serialized = ron::ser::to_string_pretty(&original, ron::ser::PrettyConfig::default())
            .expect("serialize RaceTable");
        let parsed: RaceTable = ron::de::from_str(&serialized).expect("deserialize RaceTable");
        assert_eq!(original, parsed, "RaceTable RON round-trip lost fields");
    }

    /// `get` returns the correct `RaceData` for authored variants and `None`
    /// for declared-but-unauthored variants.
    #[test]
    fn race_table_get_returns_authored_race() {
        let table = RaceTable {
            races: vec![human_data(), elf_data()],
        };
        assert!(table.get(Race::Human).is_some(), "Human should be found");
        assert_eq!(table.get(Race::Human).unwrap().display_name, "Human");
        assert!(table.get(Race::Elf).is_some(), "Elf should be found");
        assert!(
            table.get(Race::Dwarf).is_none(),
            "Dwarf not in this table — should return None"
        );
    }

    /// The `stat_modifiers` i16 encoding: -1 stored as 65535, reinterpreted
    /// correctly via `field as i16`.
    #[test]
    fn race_stat_modifier_i16_encoding() {
        let elf = elf_data();
        // strength modifier is -1, stored as 65535
        let str_mod = elf.stat_modifiers.strength as i16;
        assert_eq!(str_mod, -1, "65535u16 as i16 must be -1");
        // vitality modifier is -2, stored as 65534
        let vit_mod = elf.stat_modifiers.vitality as i16;
        assert_eq!(vit_mod, -2, "65534u16 as i16 must be -2");
        // intelligence modifier is +2, stored as 2
        let int_mod = elf.stat_modifiers.intelligence as i16;
        assert_eq!(int_mod, 2, "2u16 as i16 must be 2");
    }
}
