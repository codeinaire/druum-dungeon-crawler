use bevy::log::info;
use bevy::prelude::*;

use crate::plugins::state::GameState;

/// Dungeon exploration plugin — grid movement, fog of war, encounter triggers.
/// Feature #2 wires OnEnter/OnExit log stubs; gameplay systems land in #4, #7, #8, #11.
pub struct DungeonPlugin;

impl Plugin for DungeonPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::Dungeon), || info!("Entered GameState::Dungeon"))
            .add_systems(OnExit(GameState::Dungeon), || info!("Exited GameState::Dungeon"));
    }
}
