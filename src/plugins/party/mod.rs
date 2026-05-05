use bevy::prelude::*;

pub mod character;

/// Party management plugin — character data, inventory, formation.
/// Empty for Feature #1; systems land in Features #9, #10, #17.
pub struct PartyPlugin;

impl Plugin for PartyPlugin {
    fn build(&self, _app: &mut App) {}
}
