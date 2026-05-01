//! Item database schema — stub for Feature #3.
//! Feature #11/#12 fill in real item types; this file is a placeholder
//! so `RonAssetPlugin::<ItemDb>::new(&["items.ron"])` has a target type.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Asset, Reflect, Serialize, Deserialize, Default, Debug, Clone)]
pub struct ItemDb {
    // Empty body for Feature #3.
}
