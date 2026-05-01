use bevy::log::info;
use bevy::prelude::*;

use crate::plugins::state::GameState;

/// Turn-based combat plugin — initiative, actions, damage resolution.
/// Feature #2 wires OnEnter/OnExit log stubs; gameplay systems land in #12-#16.
pub struct CombatPlugin;

impl Plugin for CombatPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::Combat), || info!("Entered GameState::Combat"))
            .add_systems(OnExit(GameState::Combat), || info!("Exited GameState::Combat"));
    }
}
