use bevy::log::info;
use bevy::prelude::*;

use crate::plugins::state::GameState;

pub mod actions;
pub mod ai;
pub mod combat_log;
pub mod damage;
pub mod encounter;
pub mod enemy;
pub mod enemy_render;
pub mod spell_cast;
pub mod status_effects;
pub mod targeting;
pub mod turn_manager;
pub mod ui_combat;

pub use status_effects::*;

/// Turn-based combat plugin — initiative, actions, damage resolution.
///
/// Feature #2 wires OnEnter/OnExit log stubs; gameplay systems land in #14-#16.
///
/// Feature #14 adds `StatusEffectsPlugin` as a sub-plugin.
///
/// Feature #15 adds `TurnManagerPlugin`, `EnemyAiPlugin`, and `CombatUiPlugin`
/// as sub-plugins (turn manager → damage → AI → UI).
///
/// Feature #16 adds `EncounterPlugin` (random rolls + combat entry).
pub struct CombatPlugin;

impl Plugin for CombatPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(status_effects::StatusEffectsPlugin)
            .add_plugins(turn_manager::TurnManagerPlugin)
            .add_plugins(ai::EnemyAiPlugin)
            .add_plugins(ui_combat::CombatUiPlugin)
            .add_plugins(encounter::EncounterPlugin) // Feature #16
            .add_plugins(enemy_render::EnemyRenderPlugin) // Feature #17
            .add_systems(OnEnter(GameState::Combat), || {
                info!("Entered GameState::Combat")
            })
            .add_systems(OnExit(GameState::Combat), || {
                info!("Exited GameState::Combat")
            });
    }
}
