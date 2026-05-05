//! Item database schema — stubs for Features #11 and #12.
//!
//! `ItemAsset` and `ItemStatBlock` are the v1 stubs Feature #11 needs so
//! that `Handle<ItemAsset>` resolves in the `Equipment` component's slots.
//! Feature #12 fleshes out per-item enchantment, durability, and custom-name
//! fields. This file is also a placeholder so
//! `RonAssetPlugin::<ItemDb>::new(&["items.ron"])` has a target type.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// Empty item database asset — registered by Feature #3's loader.
///
/// Feature #12 populates this with real item records.
#[derive(Asset, Reflect, Serialize, Deserialize, Default, Debug, Clone)]
pub struct ItemDb {
    // Empty body for Feature #3.
}

/// Per-item stat contribution that `derive_stats` reads.
///
/// v1 schema — `#12` may add more fields (e.g., elemental damage, status
/// chance). All fields are additive bonuses layered on top of `BaseStats`
/// scaling; `derive_stats` sums them from all equipped `ItemStatBlock` slices.
///
/// All fields use `#[serde(default)]` so partial records in RON omit
/// zero-valued fields (saves file noise and allows forward-compatible
/// schema additions).
#[derive(Reflect, Serialize, Deserialize, Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct ItemStatBlock {
    #[serde(default)]
    pub attack: u32,
    #[serde(default)]
    pub defense: u32,
    #[serde(default)]
    pub magic_attack: u32,
    #[serde(default)]
    pub magic_defense: u32,
    #[serde(default)]
    pub accuracy: u32,
    #[serde(default)]
    pub evasion: u32,
    #[serde(default)]
    pub hp_bonus: u32,
    #[serde(default)]
    pub mp_bonus: u32,
}

/// A single item as a Bevy asset — holds the stat contribution for
/// `derive_stats` in v1.
///
/// `Handle<ItemAsset>` is stored in `Equipment` slots so that save/load (#23)
/// gets a serializable asset-path representation without `MapEntities`.
/// Feature #12 adds per-item enchantment, durability, and custom-name fields.
#[derive(Asset, Reflect, Serialize, Deserialize, Default, Debug, Clone)]
pub struct ItemAsset {
    /// Stat contribution applied when this item is equipped.
    pub stats: ItemStatBlock,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Round-trip a non-default `ItemStatBlock` through RON and back.
    /// Verifies the serde derives are symmetric on the `ron 0.12` path.
    /// Pure stdlib + ron 0.12 — no Bevy `App`. Runs in <1 ms.
    ///
    /// Pattern from `src/data/dungeon.rs:438-455`.
    #[test]
    fn item_stat_block_round_trips_through_ron() {
        let original = ItemStatBlock {
            attack: 10,
            defense: 5,
            magic_attack: 3,
            magic_defense: 2,
            accuracy: 7,
            evasion: 4,
            hp_bonus: 20,
            mp_bonus: 15,
        };

        let serialized = ron::ser::to_string_pretty(&original, ron::ser::PrettyConfig::default())
            .expect("serialize ItemStatBlock");

        let parsed: ItemStatBlock =
            ron::de::from_str(&serialized).expect("deserialize ItemStatBlock");

        assert_eq!(original, parsed, "ItemStatBlock RON round-trip lost fields");
    }
}
