//! Enemy database schema — stub for Feature #3.
//! Features #11/#15 fill in real enemy types; this file is a placeholder
//! so `RonAssetPlugin::<EnemyDb>::new(&["enemies.ron"])` has a target type.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Asset, Reflect, Serialize, Deserialize, Default, Debug, Clone)]
pub struct EnemyDb {
    // Empty body for Feature #3.
}
