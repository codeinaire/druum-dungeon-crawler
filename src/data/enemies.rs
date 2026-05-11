//! Enemy database schema — Feature #17.
//!
//! `EnemyDb` is loaded as an `Asset` via `bevy_common_assets::RonAssetPlugin`
//! (registered in `loading/mod.rs:111`). Each `EnemyDefinition` carries identity
//! (`id`/`display_name`), stat blocks, AI variant, and visual data
//! (`placeholder_color` for #17 placeholders; `sprite_path` for future real art).
//!
//! ## Authoring contract
//!
//! The roster lives at `assets/enemies/core.enemies.ron`. Each entry must have:
//! - A unique `id` (used by `EnemySpec.id` in encounters to look up visuals).
//! - `placeholder_color: (f32, f32, f32)` — RGB in [0.0, 1.0]; clamped on use.
//! - Optional `sprite_path: Some("enemies/<id>/idle.png")` for future real art.
//!
//! ## Inline-EnemySpec back-compat
//!
//! `EnemySpec.id` is `#[serde(default)]` — existing encounter files without
//! `id` still parse; `spawn_enemy_billboards` falls back to a default grey
//! colour when `id` does not resolve in `EnemyDb`.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::plugins::combat::ai::EnemyAi;
use crate::plugins::party::character::{BaseStats, DerivedStats};

/// One enemy's authored data — identity, stats, AI, visual placeholder.
///
/// `placeholder_color` is normalised RGB in `[0.0, 1.0]`. Channels are
/// clamped at the consumer (trust boundary) — see `spawn_enemy_billboards`.
///
/// `sprite_path` is `None` for the placeholder PR; future real-art PRs
/// populate it with `"enemies/<id>/idle.png"` etc. and the spawn system
/// prefers `Handle<Image>` lookups over generated placeholders.
#[derive(Reflect, Serialize, Deserialize, Debug, Clone)]
pub struct EnemyDefinition {
    pub id: String,
    pub display_name: String,
    pub base_stats: BaseStats,
    pub derived_stats: DerivedStats,
    #[serde(default)]
    pub ai: EnemyAi,
    pub placeholder_color: [f32; 3],
    #[serde(default)]
    pub sprite_path: Option<String>,
}

/// Top-level enemy roster, loaded from `assets/enemies/core.enemies.ron`.
///
/// `RonAssetPlugin::<EnemyDb>::new(&["enemies.ron"])` is registered in
/// `loading/mod.rs:111` (unchanged from the Feature #3 stub).
#[derive(Asset, Reflect, Serialize, Deserialize, Default, Debug, Clone)]
pub struct EnemyDb {
    pub enemies: Vec<EnemyDefinition>,
}

impl EnemyDb {
    /// Look up an enemy by id. Returns `None` if no entry matches.
    ///
    /// Used by `spawn_enemy_billboards` to resolve `EnemySpec.id` →
    /// `EnemyDefinition.placeholder_color`. Empty-id input returns `None`
    /// (back-compat with inline `EnemySpec` in `floor_01.encounters.ron`).
    pub fn find(&self, id: &str) -> Option<&EnemyDefinition> {
        if id.is_empty() {
            return None;
        }
        self.enemies.iter().find(|e| e.id == id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mk_def(id: &str, color: [f32; 3]) -> EnemyDefinition {
        EnemyDefinition {
            id: id.into(),
            display_name: id.into(),
            base_stats: BaseStats::default(),
            derived_stats: DerivedStats::default(),
            ai: EnemyAi::default(),
            placeholder_color: color,
            sprite_path: None,
        }
    }

    #[test]
    fn enemy_db_round_trips_via_ron() {
        let db = EnemyDb {
            enemies: vec![mk_def("goblin", [0.4, 0.6, 0.3])],
        };
        let serialised = ron::ser::to_string(&db).expect("serialize");
        let deserialised: EnemyDb = ron::de::from_str(&serialised).expect("deserialize");
        assert_eq!(deserialised.enemies.len(), 1);
        assert_eq!(deserialised.enemies[0].id, "goblin");
        assert_eq!(deserialised.enemies[0].placeholder_color, [0.4, 0.6, 0.3]);
    }

    #[test]
    fn find_returns_some_for_known_id() {
        let db = EnemyDb {
            enemies: vec![
                mk_def("goblin", [0.4, 0.6, 0.3]),
                mk_def("spider", [0.15, 0.1, 0.2]),
            ],
        };
        let goblin = db.find("goblin").expect("known id");
        assert_eq!(goblin.display_name, "goblin");
    }

    #[test]
    fn find_returns_none_for_unknown_id() {
        let db = EnemyDb { enemies: vec![mk_def("goblin", [0.4, 0.6, 0.3])] };
        assert!(db.find("dragon").is_none());
    }

    #[test]
    fn find_returns_none_for_empty_id() {
        // Back-compat path: EnemySpec.id is "" when authored before #17.
        let db = EnemyDb { enemies: vec![mk_def("goblin", [0.4, 0.6, 0.3])] };
        assert!(db.find("").is_none());
    }

    #[test]
    fn core_enemies_ron_parses_with_10_enemies() {
        // Mirrors the floor_01_encounters_ron_parses pattern at
        // src/data/encounters.rs:221-230.
        let raw = std::fs::read_to_string("assets/enemies/core.enemies.ron")
            .expect("core.enemies.ron exists");
        let db: EnemyDb = ron::de::from_str(&raw).expect("parses cleanly");
        assert_eq!(db.enemies.len(), 10, "10-enemy roster per #17 user decision 2B");
        // Every id must be unique.
        let mut ids: Vec<&String> = db.enemies.iter().map(|e| &e.id).collect();
        ids.sort();
        let unique_count = ids
            .iter()
            .fold((Vec::new(), 0usize), |(mut seen, n), id| {
                if seen.last().is_none_or(|last| last != id) {
                    seen.push(*id);
                    (seen, n + 1)
                } else {
                    (seen, n)
                }
            })
            .1;
        assert_eq!(unique_count, 10, "all 10 enemy ids must be unique");
    }
}
