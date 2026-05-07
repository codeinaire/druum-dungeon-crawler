//! Item database schema — fleshed out by Feature #12.
//!
//! `ItemAsset` and `ItemStatBlock` are the v1 stubs Feature #11 created so
//! that `Handle<ItemAsset>` resolves in the `Equipment` component's slots.
//! Feature #12 extends `ItemAsset` with the full 9-field schema and populates
//! `ItemDb` with a `Vec<ItemAsset>` + a `get` lookup helper.
//!
//! **Reverse-dep note:** This file imports `EquipSlot` and `ItemKind` from
//! `src/plugins/party/inventory`. That is the same one-way dependency-inversion
//! pattern as `src/data/classes.rs` (which imports `Class`/`BaseStats` from
//! `plugins::party::character`). Types stay in the plugin that semantically
//! owns them; `data/` is the thin schema mirror.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

// Reverse dep: data/ imports from plugins/party/inventory. See file-level doc.
// `EquipSlot` and `ItemKind` live in inventory.rs because they are part of the
// item-entity model; `data/items.rs` mirrors them in the asset schema.
use crate::plugins::party::inventory::{EquipSlot, ItemKind};

/// A typed RON asset containing all authored item definitions.
///
/// Loaded from `assets/items/core.items.ron` by
/// `RonAssetPlugin::<ItemDb>::new(&["items.ron"])` (registered in
/// `LoadingPlugin` at `loading/mod.rs:98`).
///
/// `get` uses a linear scan over `Vec<ItemAsset>` (O(n≤12)) — same rationale
/// as `ClassTable::get` at `data/classes.rs:41-43`.
#[derive(Asset, Reflect, Serialize, Deserialize, Default, Debug, Clone)]
pub struct ItemDb {
    pub items: Vec<ItemAsset>,
}

impl ItemDb {
    /// Look up an `ItemAsset` by its `id` field via linear scan.
    ///
    /// Returns `None` when the id has no entry. The ≤12 item roster makes a
    /// `HashMap` unnecessary (mirror of `ClassTable::get`).
    pub fn get(&self, id: &str) -> Option<&ItemAsset> {
        self.items.iter().find(|i| i.id == id)
    }
}

/// Per-item stat contribution that `derive_stats` reads.
///
/// v1 schema — `#15` may add more fields (e.g., elemental damage, status
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

/// A single item as a Bevy asset — the full v1 schema.
///
/// `Handle<ItemAsset>` is stored in `Equipment` slots so that save/load (#23)
/// gets a serializable asset-path representation without `MapEntities`.
///
/// Per-instance state (enchantment, durability, custom name) lives on the
/// `ItemInstance` entity (see `inventory.rs`), NOT on this asset.
///
/// `PartialEq` is derived (absent in the #11 stub) to enable `assert_eq!`
/// in unit tests.
#[derive(Asset, Reflect, Serialize, Deserialize, Default, Debug, Clone, PartialEq)]
pub struct ItemAsset {
    /// Unique string identifier (e.g., `"rusty_sword"`). Matches the `id`
    /// field used in `ItemDb::get`.
    pub id: String,
    /// Human-readable name shown in UI (#25).
    pub display_name: String,
    /// Stat contribution applied when this item is equipped.
    pub stats: ItemStatBlock,
    /// Behavioural classification — determines which systems can act on this item.
    pub kind: ItemKind,
    /// Which `Equipment` slot this item occupies. `EquipSlot::None` for
    /// `Consumable` and `KeyItem` items (they are not equippable).
    pub slot: EquipSlot,
    /// Carry weight (arbitrary units). Used by #18 shop / #21 loot weight tables.
    #[serde(default)]
    pub weight: u32,
    /// Sell/buy value in gold. Used by #18 shop.
    #[serde(default)]
    pub value: u32,
    /// Asset path for the 32×32 placeholder icon (relative to `assets/`).
    /// Resolved by Feature #25 UI; `#12` authors the path string only.
    #[serde(default)]
    pub icon_path: String,
    /// Forward-compat flag for stackable items (potions, arrows).
    /// **No system reads this in v1** — the roadmap (line 681) explicitly
    /// punts stackable logic. Each potion is a unique entity in v1.
    #[serde(default)]
    pub stackable: bool,
    /// Optional key identifier — only meaningful when `kind == ItemKind::KeyItem`.
    /// Read by Feature #13's `handle_door_interact` when the player presses
    /// Interact against a `WallType::LockedDoor`. Default `None` for non-key items.
    #[serde(default)]
    pub key_id: Option<String>,
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

    /// Round-trip a fully-populated `ItemAsset` through RON.
    /// Verifies all 9 fields survive the serde round-trip, including
    /// the `EquipSlot` and `ItemKind` enum fields.
    #[test]
    fn item_asset_round_trips_through_ron() {
        let original = ItemAsset {
            id: "rusty_sword".to_string(),
            display_name: "Rusty Sword".to_string(),
            stats: ItemStatBlock {
                attack: 5,
                defense: 0,
                magic_attack: 0,
                magic_defense: 0,
                accuracy: 0,
                evasion: 0,
                hp_bonus: 0,
                mp_bonus: 0,
            },
            kind: ItemKind::Weapon,
            slot: EquipSlot::Weapon,
            weight: 2,
            value: 10,
            icon_path: "assets/ui/icons/items/rusty_sword.png".to_string(),
            stackable: false,
            ..Default::default()
        };

        let serialized = ron::ser::to_string_pretty(&original, ron::ser::PrettyConfig::default())
            .expect("serialize ItemAsset");
        let parsed: ItemAsset = ron::de::from_str(&serialized).expect("deserialize ItemAsset");
        assert_eq!(original, parsed, "ItemAsset RON round-trip lost fields");
    }

    /// Round-trip an `ItemDb` with two items, asserting order is preserved.
    #[test]
    fn item_db_round_trips_through_ron() {
        let original = ItemDb {
            items: vec![
                ItemAsset {
                    id: "rusty_sword".to_string(),
                    display_name: "Rusty Sword".to_string(),
                    kind: ItemKind::Weapon,
                    slot: EquipSlot::Weapon,
                    stats: ItemStatBlock {
                        attack: 5,
                        ..Default::default()
                    },
                    weight: 2,
                    value: 10,
                    icon_path: String::new(),
                    stackable: false,
                    ..Default::default()
                },
                ItemAsset {
                    id: "healing_potion".to_string(),
                    display_name: "Healing Potion".to_string(),
                    kind: ItemKind::Consumable,
                    slot: EquipSlot::None,
                    stats: ItemStatBlock::default(),
                    weight: 0,
                    value: 50,
                    icon_path: String::new(),
                    stackable: false,
                    ..Default::default()
                },
            ],
        };

        let serialized = ron::ser::to_string_pretty(&original, ron::ser::PrettyConfig::default())
            .expect("serialize ItemDb");
        let parsed: ItemDb = ron::de::from_str(&serialized).expect("deserialize ItemDb");

        assert_eq!(
            parsed.items.len(),
            2,
            "ItemDb round-trip must preserve item count"
        );
        assert_eq!(
            parsed.items[0].id, "rusty_sword",
            "order preserved: first item"
        );
        assert_eq!(
            parsed.items[1].id, "healing_potion",
            "order preserved: second item"
        );
    }

    /// Round-trip an `ItemAsset` with a `key_id` set through RON.
    /// Verifies the new `#[serde(default)]` field survives serialization/deserialization.
    #[test]
    fn item_asset_round_trips_with_key_id() {
        let original = ItemAsset {
            id: "rusty_key".into(),
            display_name: "Rusty Key".into(),
            kind: ItemKind::KeyItem,
            slot: EquipSlot::None,
            key_id: Some("rusty_door_01".into()),
            ..Default::default()
        };
        let serialized = ron::ser::to_string_pretty(&original, ron::ser::PrettyConfig::default())
            .expect("ItemAsset should serialize");
        let parsed: ItemAsset =
            ron::de::from_str(&serialized).expect("ItemAsset should round-trip");
        assert_eq!(parsed, original);
    }

    /// `ItemDb::get` returns `Some` for an authored item and `None` for a
    /// missing one.
    #[test]
    fn item_db_get_returns_authored_item() {
        let db = ItemDb {
            items: vec![ItemAsset {
                id: "rusty_sword".to_string(),
                display_name: "Rusty Sword".to_string(),
                kind: ItemKind::Weapon,
                slot: EquipSlot::Weapon,
                stats: ItemStatBlock {
                    attack: 5,
                    ..Default::default()
                },
                weight: 2,
                value: 10,
                icon_path: String::new(),
                stackable: false,
                ..Default::default()
            }],
        };

        let found = db.get("rusty_sword");
        assert!(found.is_some(), "rusty_sword should be found");
        assert_eq!(found.unwrap().id, "rusty_sword");

        let missing = db.get("nonexistent_item");
        assert!(missing.is_none(), "missing item should return None");
    }
}
