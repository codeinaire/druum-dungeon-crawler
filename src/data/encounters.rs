//! Encounter table asset schema — Feature #16.
//!
//! `EncounterTable` is loaded as an `Asset` via `bevy_common_assets::RonAssetPlugin`
//! (registered in `loading/mod.rs`). Each floor references its table by handle on
//! `DungeonAssets`; lookup is via `loading::encounter_table_for(&assets, floor_number)`.
//!
//! ## Inline EnemySpec (D-A4)
//!
//! Until #17 ships `EnemyDb`, encounter tables carry full `BaseStats`/`DerivedStats`/
//! `EnemyAi` inline. Migration path: add `enemy_id: Option<String>` to `EnemySpec`
//! (additive); resolver falls back to inline when `None`.
//!
//! ## RON extension (D-X8)
//!
//! Files use the multi-dot extension `*.encounters.ron`. The RON loader is registered
//! via `RonAssetPlugin::<EncounterTable>::new(&["encounters.ron"])` in `loading/mod.rs`
//! (without a leading dot — research §Pitfall 4 of #3 plan).

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::plugins::combat::ai::EnemyAi;
use crate::plugins::party::character::{BaseStats, DerivedStats};

/// Inline enemy spec for #16. Fields mirror `EnemyBundle` (`combat/enemy.rs:39-51`).
///
/// Until #17 ships `EnemyDb`, encounter tables carry full enemy stats inline.
/// Feature #17 added `id: String` (additive `#[serde(default)]`) for visual
/// lookup — empty-id falls back to a default grey placeholder colour.
#[derive(Reflect, Serialize, Deserialize, Debug, Clone)]
pub struct EnemySpec {
    /// Lookup key into `EnemyDb` for visual data. Empty string means
    /// "no visual lookup" — fall back to default grey colour.
    #[serde(default)]
    pub id: String,
    pub name: String,
    pub base_stats: BaseStats,
    pub derived_stats: DerivedStats,
    /// Defaults to `EnemyAi::RandomAttack` (D-Q5=A from #15).
    #[serde(default)]
    pub ai: EnemyAi,
}

/// A group of enemies spawned together for one encounter.
///
/// `enemies.len()` is clamped to `MAX_ENEMIES_PER_ENCOUNTER` (8) by the
/// consumer; oversized groups are truncated with a `warn!`.
#[derive(Reflect, Serialize, Deserialize, Debug, Clone)]
pub struct EnemyGroup {
    pub enemies: Vec<EnemySpec>,
}

/// One entry in an encounter table — a weight + enemy group.
///
/// Weight is `u32` (not `f32`) for byte-stable RON round-trips and to satisfy
/// `WeightedIndex::new`'s integer-summable requirement.
#[derive(Reflect, Serialize, Deserialize, Debug, Clone)]
pub struct EncounterEntry {
    pub weight: u32,
    pub group: EnemyGroup,
}

/// One floor's encounter table. Loaded by `RonAssetPlugin::<EncounterTable>`.
#[derive(Asset, Reflect, Serialize, Deserialize, Default, Debug, Clone)]
pub struct EncounterTable {
    /// Identifier — matches `DungeonFloor.encounter_table` (`data/dungeon.rs:260`).
    pub id: String,
    pub entries: Vec<EncounterEntry>,
}

impl EncounterTable {
    /// Pick a weighted-random `EnemyGroup` from the table.
    ///
    /// Returns `None` if the table is empty or all weights are zero.
    ///
    /// `?Sized` permits passing `&mut *rng.0` from a `Box<dyn RngCore + Send + Sync>`
    /// (locked by #15 D-I13).
    pub fn pick_group<'a>(
        &'a self,
        rng: &mut (impl rand::Rng + ?Sized),
    ) -> Option<&'a EnemyGroup> {
        if self.entries.is_empty() {
            return None;
        }
        // Weights are clamped to a sane range to defuse malicious or typo'd
        // RON values (Security trust boundary).
        let weights = self
            .entries
            .iter()
            .map(|e| e.weight.clamp(1, 10_000));
        // rand 0.9: WeightedIndex moved to rand::distr::weighted (was rand::distributions).
        let dist = rand::distr::weighted::WeightedIndex::new(weights).ok()?;
        use rand::prelude::Distribution;
        let idx = dist.sample(rng);
        Some(&self.entries[idx].group)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;

    fn mk_spec(name: &str, hp: u32) -> EnemySpec {
        EnemySpec {
            id: name.to_lowercase(),
            name: name.into(),
            base_stats: BaseStats::default(),
            derived_stats: DerivedStats {
                current_hp: hp,
                max_hp: hp,
                ..Default::default()
            },
            ai: EnemyAi::default(),
        }
    }

    #[test]
    fn pick_group_returns_none_on_empty_table() {
        let table = EncounterTable::default();
        let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(42);
        assert!(table.pick_group(&mut rng).is_none());
    }

    #[test]
    fn pick_group_returns_only_entry_when_single() {
        let table = EncounterTable {
            id: "test".into(),
            entries: vec![EncounterEntry {
                weight: 50,
                group: EnemyGroup {
                    enemies: vec![mk_spec("Goblin", 30)],
                },
            }],
        };
        let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(42);
        let group = table.pick_group(&mut rng).expect("table non-empty");
        assert_eq!(group.enemies[0].name, "Goblin");
    }

    #[test]
    fn pick_group_proportions_match_weights_with_seed() {
        // 50/30/15/5 weighted table; sample 10000 times with seeded RNG;
        // assert empirical proportions are within ±5% of expected.
        let table = EncounterTable {
            id: "test".into(),
            entries: vec![
                EncounterEntry {
                    weight: 50,
                    group: EnemyGroup {
                        enemies: vec![mk_spec("A", 1)],
                    },
                },
                EncounterEntry {
                    weight: 30,
                    group: EnemyGroup {
                        enemies: vec![mk_spec("B", 1)],
                    },
                },
                EncounterEntry {
                    weight: 15,
                    group: EnemyGroup {
                        enemies: vec![mk_spec("C", 1)],
                    },
                },
                EncounterEntry {
                    weight: 5,
                    group: EnemyGroup {
                        enemies: vec![mk_spec("D", 1)],
                    },
                },
            ],
        };
        let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(42);
        let mut counts = [0u32; 4];
        for _ in 0..10_000 {
            let group = table.pick_group(&mut rng).unwrap();
            let idx = ["A", "B", "C", "D"]
                .iter()
                .position(|&s| s == group.enemies[0].name)
                .unwrap();
            counts[idx] += 1;
        }
        // Expected proportions: 50%, 30%, 15%, 5%. Tolerance ±5% (500 samples)
        // because seed 42 is deterministic but the bounds give wiggle room.
        assert!(
            (4500..=5500).contains(&counts[0]),
            "A count out of range: {}",
            counts[0]
        );
        assert!(
            (2500..=3500).contains(&counts[1]),
            "B count out of range: {}",
            counts[1]
        );
        assert!(
            (1000..=2000).contains(&counts[2]),
            "C count out of range: {}",
            counts[2]
        );
        assert!(
            (0..=1000).contains(&counts[3]),
            "D count out of range: {}",
            counts[3]
        );
    }

    #[test]
    fn encounter_table_round_trips_via_ron() {
        let table = EncounterTable {
            id: "b1f_test".into(),
            entries: vec![EncounterEntry {
                weight: 1,
                group: EnemyGroup {
                    enemies: vec![mk_spec("Goblin", 30)],
                },
            }],
        };
        let serialized = ron::ser::to_string(&table).expect("serialize");
        let deserialized: EncounterTable =
            ron::de::from_str(&serialized).expect("deserialize");
        assert_eq!(deserialized.id, table.id);
        assert_eq!(deserialized.entries.len(), 1);
        assert_eq!(deserialized.entries[0].group.enemies[0].name, "Goblin");
    }

    #[test]
    fn floor_01_encounters_ron_parses() {
        let raw = std::fs::read_to_string("assets/encounters/floor_01.encounters.ron")
            .expect("floor_01.encounters.ron exists");
        let table: EncounterTable = ron::de::from_str(&raw).expect("parses cleanly");
        assert_eq!(table.id, "b1f_encounters");
        assert_eq!(table.entries.len(), 4);
        // Sanity: weights sum to 100 (designer convention).
        let sum: u32 = table.entries.iter().map(|e| e.weight).sum();
        assert_eq!(sum, 100);
    }
}
