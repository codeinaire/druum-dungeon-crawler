//! Town-data RON-loaded schemas — Feature #18a.
//!
//! Three asset types power the Town hub:
//! - [`ShopStock`] — the items available for sale, filtered by minimum floor.
//! - [`RecruitPool`] — candidate NPCs the Guild can recruit (Feature #18b reads this).
//! - [`TownServices`] — costs and cure-lists for the Inn; Temple fields are
//!   pre-declared with `#[serde(default)]` so #18b adds no schema migration.
//!
//! ## Reverse-dep note
//!
//! `RecruitDef` references `Race`, `Class`, `BaseStats`, and `PartyRow` from
//! `plugins::party::character` — the same one-way import direction used by
//! `data/classes.rs`. Types stay in the plugin that semantically owns them;
//! `data/` is the thin schema mirror.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::plugins::party::character::{BaseStats, Class, PartyRow, Race};
use crate::plugins::party::character::StatusEffectType;

// ─────────────────────────────────────────────────────────────────────────────
// Trust-boundary constants (Phase 10)
// ─────────────────────────────────────────────────────────────────────────────

/// Maximum number of shop entries that the painter iterates. Authored RON
/// may contain more — `clamp_shop_stock` truncates the slice at this bound so
/// a malformed/crafted RON file cannot exhaust paint-loop memory.
pub const MAX_SHOP_ITEMS: usize = 99;

/// Maximum allowable `inn_rest_cost` as read from `town_services.ron`.
/// `u32::MAX` would make the Inn unusable; `0` allows unlimited free rests.
/// Capped in `handle_inn_rest` before the gold-check.
pub const MAX_INN_COST: u32 = 10_000;

/// Maximum allowable Temple cost (revive base, revive per-level, or per-status
/// cure) as read from `town_services.ron`. A crafted RON with `u32::MAX` would
/// make the Temple permanently unusable; `0` would allow free revives (caught by
/// the `.max(1)` guard in `revive_cost`). Clamped in `temple::revive_cost` and
/// `temple::cure_cost` before any gold-check.
pub const MAX_TEMPLE_COST: u32 = 100_000;

/// Maximum number of recruits that the Guild painter iterates. Authored RON
/// may contain more — `clamp_recruit_pool` truncates the slice at this bound so
/// a crafted/malformed RON file cannot exhaust paint-loop memory.
pub const MAX_RECRUIT_POOL: usize = 32;

// ─────────────────────────────────────────────────────────────────────────────
// ShopEntry
// ─────────────────────────────────────────────────────────────────────────────

/// One entry in the shop's catalogue.
///
/// `buy_price: None` means "use the item's own `value` field from the `ItemDb`"
/// (the common case — override only when the shop marks up/down a specific item).
///
/// `min_floor` is the deepest dungeon floor the party must have visited before
/// this item appears. `0` = always available.
#[derive(Reflect, Serialize, Deserialize, Default, Debug, Clone, PartialEq)]
pub struct ShopEntry {
    /// String identifier matching `ItemAsset::id` in `core.items.ron`.
    pub item_id: String,
    /// Override price. `None` → use `ItemAsset::value` (the authored sell/buy value).
    #[serde(default)]
    pub buy_price: Option<u32>,
    /// Minimum dungeon floor unlocking this entry (0 = always visible).
    #[serde(default)]
    pub min_floor: u32,
}

// ─────────────────────────────────────────────────────────────────────────────
// ShopStock
// ─────────────────────────────────────────────────────────────────────────────

/// A typed RON asset containing all authored shop entries.
///
/// Loaded from `assets/town/core.shop_stock.ron` by
/// `RonAssetPlugin::<ShopStock>::new(&["shop_stock.ron"])`.
#[derive(Asset, Reflect, Serialize, Deserialize, Default, Debug, Clone)]
pub struct ShopStock {
    pub items: Vec<ShopEntry>,
}

impl ShopStock {
    /// Returns all entries whose `min_floor <= floor`.
    ///
    /// Does **not** apply the `MAX_SHOP_ITEMS` cap — callers should pass the
    /// result through [`clamp_shop_stock`] before iterating in a paint loop.
    pub fn items_for_floor(&self, floor: u32) -> Vec<&ShopEntry> {
        self.items
            .iter()
            .filter(|e| e.min_floor <= floor)
            .collect()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// clamp_shop_stock (trust-boundary helper)
// ─────────────────────────────────────────────────────────────────────────────

/// Return a bounded slice of the shop catalogue, truncated to at most
/// `max_items` entries. Call this in the shop painter before iterating so that
/// a crafted RON with a 100K-entry vector does not exhaust paint-loop time.
///
/// `max_items` is typically [`MAX_SHOP_ITEMS`].
pub fn clamp_shop_stock(stock: &ShopStock, max_items: usize) -> &[ShopEntry] {
    let len = stock.items.len().min(max_items);
    &stock.items[..len]
}

// ─────────────────────────────────────────────────────────────────────────────
// clamp_recruit_pool (trust-boundary helper)
// ─────────────────────────────────────────────────────────────────────────────

/// Return a bounded slice of the recruit pool, truncated to at most
/// `max_recruits` entries. Call this in the Guild painter before iterating so
/// that a crafted RON with a 100K-entry pool does not exhaust paint-loop memory.
///
/// `max_recruits` is typically [`MAX_RECRUIT_POOL`].
pub fn clamp_recruit_pool(pool: &RecruitPool, max_recruits: usize) -> &[RecruitDef] {
    let len = pool.recruits.len().min(max_recruits);
    &pool.recruits[..len]
}

// ─────────────────────────────────────────────────────────────────────────────
// RecruitDef + RecruitPool
// ─────────────────────────────────────────────────────────────────────────────

/// One potential party recruit available in the Guild.
///
/// **Zero readers in Feature #18a.** The struct + RON file are authored now
/// so that Feature #18b's `guild.rs` is a pure additive — no data migration.
#[derive(Reflect, Serialize, Deserialize, Default, Debug, Clone, PartialEq)]
pub struct RecruitDef {
    pub name: String,
    pub race: Race,
    pub class: Class,
    pub base_stats: BaseStats,
    /// Which formation row the recruit prefers. Default = `Front`.
    #[serde(default)]
    pub default_row: PartyRow,
}

/// A typed RON asset containing all authored recruits for the Guild.
///
/// Loaded from `assets/town/core.recruit_pool.ron` by
/// `RonAssetPlugin::<RecruitPool>::new(&["recruit_pool.ron"])`.
/// No #18a system reads this handle — it is loaded for collection completeness
/// and consumed by Feature #18b.
#[derive(Asset, Reflect, Serialize, Deserialize, Default, Debug, Clone)]
pub struct RecruitPool {
    pub recruits: Vec<RecruitDef>,
}

// ─────────────────────────────────────────────────────────────────────────────
// TownServices
// ─────────────────────────────────────────────────────────────────────────────

/// Configuration for Town services (Inn + Temple).
///
/// Loaded from `assets/town/core.town_services.ron` by
/// `RonAssetPlugin::<TownServices>::new(&["town_services.ron"])`.
///
/// Fields prefixed `temple_*` are pre-declared with `#[serde(default)]` so
/// that Feature #18b's Temple logic can add readers without any RON schema
/// migration.
///
/// ## Security note
///
/// `inn_rest_cost` is clamped in `handle_inn_rest` to `MAX_INN_COST` before
/// use. `#[serde(default)]` fields default to zero/empty if omitted from RON,
/// which is the correct forward-compat behaviour.
#[derive(Asset, Reflect, Serialize, Deserialize, Default, Debug, Clone)]
pub struct TownServices {
    /// Gold cost to rest at the Inn. Clamped to [`MAX_INN_COST`] at use-site.
    pub inn_rest_cost: u32,
    /// Status effects cured by a full Inn rest (e.g., `[Poison]`).
    #[serde(default)]
    pub inn_rest_cures: Vec<StatusEffectType>,

    // ── Feature #18b fields (pre-declared, no #18a readers) ─────────────────
    /// Base gold cost to revive a dead character at the Temple.
    /// Used by Feature #18b. Default 0.
    #[serde(default)]
    pub temple_revive_cost_base: u32, // Used by Feature #18b
    /// Additional gold per character level for Temple revive.
    /// Used by Feature #18b. Default 0.
    #[serde(default)]
    pub temple_revive_cost_per_level: u32, // Used by Feature #18b
    /// Per-status cure costs at the Temple: `(StatusEffectType, gold_cost)`.
    /// Used by Feature #18b. Default empty.
    #[serde(default)]
    pub temple_cure_costs: Vec<(StatusEffectType, u32)>, // Used by Feature #18b
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugins::party::character::{BaseStats, Class, PartyRow, Race, StatusEffectType};

    // ── ShopStock ──────────────────────────────────────────────────────────────

    /// Round-trip a `ShopStock` through RON.
    #[test]
    fn shop_stock_ron_round_trips() {
        let original = ShopStock {
            items: vec![
                ShopEntry {
                    item_id: "rusty_sword".to_string(),
                    buy_price: Some(12),
                    min_floor: 0,
                },
                ShopEntry {
                    item_id: "healing_potion".to_string(),
                    buy_price: None,
                    min_floor: 1,
                },
            ],
        };

        let serialized =
            ron::ser::to_string_pretty(&original, ron::ser::PrettyConfig::default())
                .expect("serialize ShopStock");
        let parsed: ShopStock =
            ron::de::from_str(&serialized).expect("deserialize ShopStock");

        assert_eq!(original.items.len(), parsed.items.len());
        assert_eq!(original.items[0].item_id, parsed.items[0].item_id);
        assert_eq!(original.items[0].buy_price, parsed.items[0].buy_price);
        assert_eq!(original.items[1].item_id, parsed.items[1].item_id);
        assert_eq!(original.items[1].buy_price, parsed.items[1].buy_price);
    }

    /// `items_for_floor` returns only entries whose `min_floor <= floor`.
    #[test]
    fn stock_filters_by_min_floor() {
        let stock = ShopStock {
            items: vec![
                ShopEntry {
                    item_id: "rusty_sword".to_string(),
                    buy_price: None,
                    min_floor: 0,
                },
                ShopEntry {
                    item_id: "iron_sword".to_string(),
                    buy_price: None,
                    min_floor: 1,
                },
                ShopEntry {
                    item_id: "steel_sword".to_string(),
                    buy_price: None,
                    min_floor: 2,
                },
            ],
        };

        let floor_0 = stock.items_for_floor(0);
        assert_eq!(floor_0.len(), 1);
        assert_eq!(floor_0[0].item_id, "rusty_sword");

        let floor_1 = stock.items_for_floor(1);
        assert_eq!(floor_1.len(), 2);

        let floor_2 = stock.items_for_floor(2);
        assert_eq!(floor_2.len(), 3);
    }

    /// An oversized stock (200 entries) is truncated by `clamp_shop_stock` to `MAX_SHOP_ITEMS`.
    #[test]
    fn clamp_shop_stock_truncates_oversized() {
        let items: Vec<ShopEntry> = (0..200)
            .map(|i| ShopEntry {
                item_id: format!("item_{i}"),
                buy_price: None,
                min_floor: 0,
            })
            .collect();
        let stock = ShopStock { items };

        let clamped = clamp_shop_stock(&stock, MAX_SHOP_ITEMS);
        assert_eq!(clamped.len(), MAX_SHOP_ITEMS);
    }

    /// A stock with fewer entries than `max_items` is returned in full.
    #[test]
    fn clamp_shop_stock_passes_through_small_stock() {
        let stock = ShopStock {
            items: vec![ShopEntry {
                item_id: "rusty_sword".to_string(),
                buy_price: None,
                min_floor: 0,
            }],
        };
        let clamped = clamp_shop_stock(&stock, MAX_SHOP_ITEMS);
        assert_eq!(clamped.len(), 1);
    }

    // ── RecruitPool ───────────────────────────────────────────────────────────

    /// Round-trip a `RecruitPool` through RON.
    #[test]
    fn recruit_pool_ron_round_trips() {
        let original = RecruitPool {
            recruits: vec![RecruitDef {
                name: "Aldric".to_string(),
                race: Race::Human,
                class: Class::Fighter,
                base_stats: BaseStats {
                    strength: 10,
                    intelligence: 5,
                    piety: 5,
                    vitality: 12,
                    agility: 8,
                    luck: 6,
                },
                default_row: PartyRow::Front,
            }],
        };

        let serialized =
            ron::ser::to_string_pretty(&original, ron::ser::PrettyConfig::default())
                .expect("serialize RecruitPool");
        let parsed: RecruitPool =
            ron::de::from_str(&serialized).expect("deserialize RecruitPool");

        assert_eq!(original.recruits.len(), parsed.recruits.len());
        assert_eq!(original.recruits[0].name, parsed.recruits[0].name);
        assert_eq!(original.recruits[0].race, parsed.recruits[0].race);
        assert_eq!(original.recruits[0].class, parsed.recruits[0].class);
    }

    // ── RecruitPool clamp ─────────────────────────────────────────────────────

    /// An oversized pool (200 entries) is truncated by `clamp_recruit_pool` to
    /// `MAX_RECRUIT_POOL` (32).
    #[test]
    fn recruit_pool_size_clamped_truncates_oversized() {
        let recruits: Vec<RecruitDef> = (0..200)
            .map(|i| RecruitDef {
                name: format!("recruit_{i}"),
                ..Default::default()
            })
            .collect();
        let pool = RecruitPool { recruits };
        let clamped = clamp_recruit_pool(&pool, MAX_RECRUIT_POOL);
        assert_eq!(clamped.len(), MAX_RECRUIT_POOL);
    }

    /// A pool with fewer entries than `max_recruits` is returned in full.
    #[test]
    fn clamp_recruit_pool_passes_through_small_pool() {
        let pool = RecruitPool {
            recruits: vec![
                RecruitDef { name: "a".into(), ..Default::default() },
                RecruitDef { name: "b".into(), ..Default::default() },
                RecruitDef { name: "c".into(), ..Default::default() },
                RecruitDef { name: "d".into(), ..Default::default() },
                RecruitDef { name: "e".into(), ..Default::default() },
            ],
        };
        let clamped = clamp_recruit_pool(&pool, MAX_RECRUIT_POOL);
        assert_eq!(clamped.len(), 5);
    }

    // ── TownServices ─────────────────────────────────────────────────────────

    /// Round-trip `TownServices`, including defaulted temple fields omitted from RON.
    /// Verifies `#[serde(default)]` fields survive the round-trip with sane defaults.
    #[test]
    fn town_services_ron_round_trips_with_defaulted_temple_fields() {
        // RON that omits the temple_* fields (relying on #[serde(default)]).
        let ron_str = r#"(
    inn_rest_cost: 10,
    inn_rest_cures: [Poison],
)"#;

        let parsed: TownServices =
            ron::de::from_str(ron_str).expect("deserialize TownServices with defaults");

        assert_eq!(parsed.inn_rest_cost, 10);
        assert_eq!(parsed.inn_rest_cures, vec![StatusEffectType::Poison]);
        // Defaulted fields:
        assert_eq!(parsed.temple_revive_cost_base, 0);
        assert_eq!(parsed.temple_revive_cost_per_level, 0);
        assert!(parsed.temple_cure_costs.is_empty());

        // Serialize the parsed value and re-parse — stable round-trip.
        let serialized =
            ron::ser::to_string_pretty(&parsed, ron::ser::PrettyConfig::default())
                .expect("serialize TownServices");
        let reparsed: TownServices =
            ron::de::from_str(&serialized).expect("re-deserialize TownServices");
        assert_eq!(parsed.inn_rest_cost, reparsed.inn_rest_cost);
        assert_eq!(parsed.inn_rest_cures, reparsed.inn_rest_cures);
    }

    /// Round-trip `TownServices` with explicitly authored temple fields.
    /// Verifies the values survive parse and that re-serialized RON is stable.
    #[test]
    fn town_services_round_trips_with_authored_temple_fields() {
        let ron_str = r#"(
    inn_rest_cost: 10,
    inn_rest_cures: [Poison],
    temple_revive_cost_base: 100,
    temple_revive_cost_per_level: 50,
    temple_cure_costs: [
        (Stone, 250),
        (Paralysis, 100),
        (Sleep, 50),
    ],
)"#;

        let parsed: TownServices =
            ron::de::from_str(ron_str).expect("deserialize TownServices with temple fields");

        // Assert non-zero post-parse (guards against accidental default-to-0).
        assert_eq!(parsed.temple_revive_cost_base, 100);
        assert_eq!(parsed.temple_revive_cost_per_level, 50);
        assert_eq!(parsed.temple_cure_costs.len(), 3);
        assert!(
            parsed.temple_cure_costs.contains(&(StatusEffectType::Stone, 250)),
            "Stone cure cost should be 250"
        );
        assert!(
            parsed.temple_cure_costs.contains(&(StatusEffectType::Paralysis, 100)),
            "Paralysis cure cost should be 100"
        );
        assert!(
            parsed.temple_cure_costs.contains(&(StatusEffectType::Sleep, 50)),
            "Sleep cure cost should be 50"
        );

        // Stable round-trip: re-serialize and re-parse.
        let serialized =
            ron::ser::to_string_pretty(&parsed, ron::ser::PrettyConfig::default())
                .expect("serialize TownServices with temple fields");
        let reparsed: TownServices =
            ron::de::from_str(&serialized).expect("re-deserialize TownServices with temple fields");
        assert_eq!(parsed.temple_revive_cost_base, reparsed.temple_revive_cost_base);
        assert_eq!(parsed.temple_revive_cost_per_level, reparsed.temple_revive_cost_per_level);
        assert_eq!(parsed.temple_cure_costs, reparsed.temple_cure_costs);
    }
}
