use bevy::log::info;
use bevy::prelude::*;

use crate::plugins::state::GameState;

pub mod status_effects;
pub use status_effects::*;

/// Turn-based combat plugin — initiative, actions, damage resolution.
/// Feature #2 wires OnEnter/OnExit log stubs; gameplay systems land in #14-#16.
/// Feature #14 adds `StatusEffectsPlugin` as a sub-plugin (status data and
/// resolution layer; the combat-round emitter of `StatusTickEvent` ships with
/// #15's `turn_manager`).
pub struct CombatPlugin;

impl Plugin for CombatPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(status_effects::StatusEffectsPlugin)
            .add_systems(OnEnter(GameState::Combat), || {
                info!("Entered GameState::Combat")
            })
            .add_systems(OnExit(GameState::Combat), || {
                info!("Exited GameState::Combat")
            });
    }
}
