use bevy::log::info;
use bevy::prelude::*;

use crate::plugins::state::GameState;

/// Town hub plugin — shop, inn, temple, guild interactions.
/// Feature #2 wires OnEnter/OnExit log stubs; gameplay systems land in #19-#22.
pub struct TownPlugin;

impl Plugin for TownPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::Town), || {
            info!("Entered GameState::Town")
        })
        .add_systems(OnExit(GameState::Town), || info!("Exited GameState::Town"));
    }
}
