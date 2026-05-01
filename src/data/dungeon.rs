//! Dungeon floor schema — stub for Feature #3.
//! Feature #4 fills in the razor-wall grid; this file just verifies the
//! `Asset` derive + serde shape so `bevy_common_assets::RonAssetPlugin`
//! can dispatch on `.dungeon.ron`.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Asset, Reflect, Serialize, Deserialize, Default, Debug, Clone)]
pub struct DungeonFloor {
    // Empty body for Feature #3.
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Round-trip a default `DungeonFloor` through RON and back.
    /// Verifies the serde derives are symmetric. Pure stdlib + ron 0.12 —
    /// no Bevy `App`, no `AssetServer`. Runs in <1 ms.
    #[test]
    fn dungeon_floor_round_trips_through_ron() {
        let original = DungeonFloor::default();

        let serialized: String = ron::ser::to_string_pretty(
            &original,
            ron::ser::PrettyConfig::default(),
        ).expect("serialize");

        let parsed: DungeonFloor = ron::de::from_str(&serialized)
            .expect("deserialize");

        let reserialized: String = ron::ser::to_string_pretty(
            &parsed,
            ron::ser::PrettyConfig::default(),
        ).expect("re-serialize");

        assert_eq!(serialized, reserialized,
            "RON round trip lost or reordered fields");
    }
}
