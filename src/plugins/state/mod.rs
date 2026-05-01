//! Game state machine — top-level GameState plus three SubStates, debug transition logger, and dev-only F9 cycler.

use bevy::log::info;
use bevy::prelude::*;

#[derive(States, Default, Debug, Clone, PartialEq, Eq, Hash)]
pub enum GameState {
    #[default]
    Loading,
    TitleScreen,
    Town,
    Dungeon,
    Combat,
    GameOver,
}

#[derive(SubStates, Default, Debug, Clone, PartialEq, Eq, Hash)]
#[source(GameState = GameState::Dungeon)]
pub enum DungeonSubState {
    #[default]
    Exploring,
    Inventory,
    Map,
    Paused,
    EventDialog,
}

#[derive(SubStates, Default, Debug, Clone, PartialEq, Eq, Hash)]
#[source(GameState = GameState::Combat)]
pub enum CombatPhase {
    #[default]
    PlayerInput,
    ExecuteActions,
    EnemyTurn,
    TurnResult,
}

#[derive(SubStates, Default, Debug, Clone, PartialEq, Eq, Hash)]
#[source(GameState = GameState::Town)]
pub enum TownLocation {
    #[default]
    Square,
    Shop,
    Inn,
    Temple,
    Guild,
}

pub struct StatePlugin;

impl Plugin for StatePlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<GameState>()
            .add_sub_state::<DungeonSubState>()
            .add_sub_state::<CombatPhase>()
            .add_sub_state::<TownLocation>()
            .add_systems(
                Update,
                log_game_state_transition.run_if(state_changed::<GameState>),
            );

        #[cfg(feature = "dev")]
        app.add_systems(Update, cycle_game_state_on_f9);
    }
}

fn log_game_state_transition(state: Res<State<GameState>>) {
    info!("GameState -> {:?}", state.get());
}

#[cfg(feature = "dev")]
fn cycle_game_state_on_f9(
    keys: Res<ButtonInput<KeyCode>>,
    current: Res<State<GameState>>,
    mut next: ResMut<NextState<GameState>>,
) {
    if !keys.just_pressed(KeyCode::F9) {
        return;
    }
    let upcoming = match current.get() {
        GameState::Loading     => GameState::TitleScreen,
        GameState::TitleScreen => GameState::Town,
        GameState::Town        => GameState::Dungeon,
        GameState::Dungeon     => GameState::Combat,
        GameState::Combat      => GameState::GameOver,
        GameState::GameOver    => GameState::Loading,
    };
    next.set(upcoming);
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::state::app::StatesPlugin;

    /// Default `GameState` is `Loading` immediately after `init_state`.
    #[test]
    fn gamestate_default_is_loading() {
        let mut app = App::new();
        app.add_plugins(StatesPlugin).add_plugins(StatePlugin);
        // When compiled with the `dev` feature, StatePlugin::build registers
        // cycle_game_state_on_f9 which requires ButtonInput<KeyCode>. We insert
        // the resource directly (without InputPlugin) so the keyboard_input_system
        // clearing loop is not registered — it would clear just_pressed in PreUpdate
        // before Update runs, breaking the F9 test.
        #[cfg(feature = "dev")]
        app.init_resource::<ButtonInput<KeyCode>>();
        app.update();
        assert_eq!(*app.world().resource::<State<GameState>>(), GameState::Loading);
    }

    /// Pressing F9 advances `GameState` to the next variant in cycle order.
    /// Verifies the one-frame deferral: `set` queues the change; the new value
    /// is observable after the next `app.update()` runs the `StateTransition` schedule.
    #[cfg(feature = "dev")]
    #[test]
    fn f9_advances_game_state() {
        let mut app = App::new();
        app.add_plugins(StatesPlugin).add_plugins(StatePlugin);
        // Insert ButtonInput<KeyCode> directly — do NOT add InputPlugin, because
        // keyboard_input_system (registered by InputPlugin) clears just_pressed in
        // PreUpdate before our Update cycler can observe it.
        app.init_resource::<ButtonInput<KeyCode>>();
        app.update(); // realise initial state -> Loading

        // Simulate F9 press.
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::F9);
        app.update(); // cycler runs, queues NextState; transition realised same frame
        app.update(); // post-transition Update sees the new state

        assert_eq!(
            *app.world().resource::<State<GameState>>(),
            GameState::TitleScreen,
        );
    }
}
