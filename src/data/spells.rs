//! Spell table schema — stub for Feature #3.
//! Feature #20 fills in real spell definitions; this file is a placeholder
//! so `RonAssetPlugin::<SpellTable>::new(&["spells.ron"])` has a target type.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Asset, Reflect, Serialize, Deserialize, Default, Debug, Clone)]
pub struct SpellTable {
    // Empty body for Feature #3.
}
