use bevy::prelude::*;

/// Turn-based combat plugin — initiative, actions, damage resolution.
/// Empty for Feature #1; systems land in Features #12-#16.
pub struct CombatPlugin;

impl Plugin for CombatPlugin {
    fn build(&self, _app: &mut App) {}
}
