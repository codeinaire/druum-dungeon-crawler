use bevy::prelude::*;

/// Dungeon exploration plugin — grid movement, fog of war, encounter triggers.
/// Empty for Feature #1; systems land in Features #4, #7, #8, #11.
pub struct DungeonPlugin;

impl Plugin for DungeonPlugin {
    fn build(&self, _app: &mut App) {}
}
