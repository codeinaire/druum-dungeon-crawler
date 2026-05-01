//! Class table schema — stub for Feature #3.
//! Feature #19 fills in real class definitions; this file is a placeholder
//! so `RonAssetPlugin::<ClassTable>::new(&["classes.ron"])` has a target type.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Asset, Reflect, Serialize, Deserialize, Default, Debug, Clone)]
pub struct ClassTable {
    // Empty body for Feature #3.
}
