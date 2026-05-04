//! Input system: gameplay actions routed through `leafwing-input-manager`.
//!
//! ## What this module owns
//!
//! - Three `Actionlike` enums — `MenuAction`, `DungeonAction`, `CombatAction` —
//!   one per game-state context. Per-context enums make state-scoping a
//!   compile-time property: a system reading `Res<ActionState<DungeonAction>>`
//!   in combat code is a compile error, not a runtime bug.
//! - Default keyboard `InputMap<T>` resources for each enum.
//! - The `ActionsPlugin` Plugin impl that registers all three
//!   `InputManagerPlugin::<T>::default()` instances and inserts the default maps.
//!
//! ## What this module does NOT own
//!
//! - The F9 dev cycler in `src/plugins/state/mod.rs`. F9 stays on
//!   `Res<ButtonInput<KeyCode>>` directly. Reasons: (1) F9 is dev-only and never
//!   user-rebindable, so leafwing's main feature (rebinding) is unused; (2)
//!   refactoring would require six `#[cfg(feature = "dev")]` gating points (enum,
//!   InputMap fn, plugin add, insert_resource, system def, add_systems); (3)
//!   the existing F9 test uses the `init_resource::<ButtonInput<KeyCode>>()`
//!   bypass pattern, which would have to switch to a full `InputPlugin` +
//!   `KeyboardInput` message injection. The carve-out is intentional.
//!
//! - A `DevAction` enum. Deferred until the first leafwing-routed dev hotkey
//!   beyond F9 lands. A placeholder enum with one variant adds cfg-gating
//!   surface for zero current callers.
//!
//! - State-scoping via `.run_if(in_state(...))`. That happens inside *consuming*
//!   plugin builds, on the gameplay systems that read `Res<ActionState<T>>`.
//!   The `InputManagerPlugin::<T>::default()` registrations themselves run
//!   unconditionally — Bevy's `Plugin` trait has no `run_if` (verified at
//!   `bevy_app-0.18.1/src/plugin.rs`).
//!
//! ## Consumer pattern (Feature #7+)
//!
//! ```ignore
//! use crate::plugins::input::DungeonAction;
//! use leafwing_input_manager::prelude::*;
//!
//! fn handle_dungeon_movement(actions: Res<ActionState<DungeonAction>>) {
//!     if actions.just_pressed(&DungeonAction::MoveForward) { /* ... */ }
//! }
//!
//! // In DungeonPlugin::build:
//! app.add_systems(Update, handle_dungeon_movement.run_if(in_state(GameState::Dungeon)));
//! ```

use bevy::prelude::*;
use leafwing_input_manager::prelude::*;

/// Menu-style navigation actions. Used in TitleScreen, Town, GameOver,
/// dungeon sub-state menus (Inventory/Map/Paused/EventDialog), and combat
/// "press any key to continue" between phases. Town reuses this enum in v1;
/// `TownAction` is deferred until Town gets distinct movement (Feature #19+).
#[derive(Actionlike, PartialEq, Eq, Hash, Clone, Copy, Debug, Reflect)]
pub enum MenuAction {
    Up,
    Down,
    Left,
    Right,
    Confirm,
    Cancel,
    Pause,
}

/// First-person grid movement and dungeon UI hotkeys. Used in
/// `GameState::Dungeon + DungeonSubState::Exploring`. Modern Wizardry/Etrian
/// convention: WASD or arrows for movement, Q/E for turning, M for map,
/// Tab for inventory, F for interact, Escape for pause.
#[derive(Actionlike, PartialEq, Eq, Hash, Clone, Copy, Debug, Reflect)]
pub enum DungeonAction {
    MoveForward,
    MoveBackward,
    StrafeLeft,
    StrafeRight,
    TurnLeft,
    TurnRight,
    Interact,
    OpenMap,
    OpenInventory,
    Pause,
}

/// Turn-based combat menu navigation. Used in
/// `GameState::Combat + CombatPhase::PlayerInput`. The action enum is
/// defined here; the systems that consume it land in Feature #15.
#[derive(Actionlike, PartialEq, Eq, Hash, Clone, Copy, Debug, Reflect)]
pub enum CombatAction {
    Up,
    Down,
    Left,
    Right,
    Confirm,
    Cancel,
}

/// Plugin that owns all gameplay input registration.
pub struct ActionsPlugin;

impl Plugin for ActionsPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            InputManagerPlugin::<MenuAction>::default(),
            InputManagerPlugin::<DungeonAction>::default(),
            InputManagerPlugin::<CombatAction>::default(),
        ))
        .init_resource::<ActionState<MenuAction>>()
        .init_resource::<ActionState<DungeonAction>>()
        .init_resource::<ActionState<CombatAction>>()
        .insert_resource(default_menu_input_map())
        .insert_resource(default_dungeon_input_map())
        .insert_resource(default_combat_input_map());
    }
}

fn default_menu_input_map() -> InputMap<MenuAction> {
    use MenuAction::*;
    InputMap::default()
        .with(Up, KeyCode::ArrowUp)
        .with(Up, KeyCode::KeyW)
        .with(Down, KeyCode::ArrowDown)
        .with(Down, KeyCode::KeyS)
        .with(Left, KeyCode::ArrowLeft)
        .with(Left, KeyCode::KeyA)
        .with(Right, KeyCode::ArrowRight)
        .with(Right, KeyCode::KeyD)
        .with(Confirm, KeyCode::Enter)
        .with(Confirm, KeyCode::Space)
        .with(Cancel, KeyCode::Escape)
        .with(Pause, KeyCode::Escape)
}

fn default_dungeon_input_map() -> InputMap<DungeonAction> {
    use DungeonAction::*;
    InputMap::default()
        // Movement (WASD + arrows; arrows STRAFE per modern convention)
        .with(MoveForward, KeyCode::KeyW)
        .with(MoveForward, KeyCode::ArrowUp)
        .with(MoveBackward, KeyCode::KeyS)
        .with(MoveBackward, KeyCode::ArrowDown)
        .with(StrafeLeft, KeyCode::KeyA)
        .with(StrafeLeft, KeyCode::ArrowLeft)
        .with(StrafeRight, KeyCode::KeyD)
        .with(StrafeRight, KeyCode::ArrowRight)
        // Turning (Q/E only — no arrow alternates to avoid overloading arrows)
        .with(TurnLeft, KeyCode::KeyQ)
        .with(TurnRight, KeyCode::KeyE)
        // Interactions and UI hotkeys
        .with(Interact, KeyCode::KeyF) // F (NOT Space, NOT E) — avoids TurnRight=E conflict
        .with(OpenMap, KeyCode::KeyM)
        .with(OpenInventory, KeyCode::Tab)
        .with(Pause, KeyCode::Escape)
}

fn default_combat_input_map() -> InputMap<CombatAction> {
    use CombatAction::*;
    InputMap::default()
        .with(Up, KeyCode::ArrowUp)
        .with(Up, KeyCode::KeyW)
        .with(Down, KeyCode::ArrowDown)
        .with(Down, KeyCode::KeyS)
        .with(Left, KeyCode::ArrowLeft)
        .with(Left, KeyCode::KeyA)
        .with(Right, KeyCode::ArrowRight)
        .with(Right, KeyCode::KeyD)
        .with(Confirm, KeyCode::Enter)
        .with(Confirm, KeyCode::Space)
        .with(Cancel, KeyCode::Escape)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::input::InputPlugin;
    use bevy::state::app::StatesPlugin;

    /// Build a minimal test app with the full input chain: MinimalPlugins,
    /// StatesPlugin, InputPlugin, ActionsPlugin. This is the OPPOSITE pattern
    /// from Feature #2's F9 test (which uses init_resource::<ButtonInput<KeyCode>>
    /// to bypass keyboard_input_system). Here we need the full chain because
    /// leafwing's update system in PreUpdate reads ButtonInput<KeyCode> AFTER
    /// keyboard_input_system populates it from KeyboardInput messages.
    fn make_test_app() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, StatesPlugin, InputPlugin, ActionsPlugin));
        app.update(); // initialise resources
        app
    }

    /// `ActionsPlugin::build` registers all three InputManagerPlugin instances
    /// and inserts all three default InputMap resources. Smoke test — no input
    /// injection.
    #[test]
    fn actions_plugin_registers_all_inputmaps() {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            StatesPlugin, // ActionsPlugin doesn't use States directly, but
            // future cross-plugin tests will — keep the test
            // setup symmetrical with the injection tests below.
            InputPlugin,
            ActionsPlugin,
        ));
        app.update();

        // All three InputMap resources must be present.
        assert!(
            app.world().contains_resource::<InputMap<MenuAction>>(),
            "InputMap<MenuAction> should be registered by ActionsPlugin"
        );
        assert!(
            app.world().contains_resource::<InputMap<DungeonAction>>(),
            "InputMap<DungeonAction> should be registered by ActionsPlugin"
        );
        assert!(
            app.world().contains_resource::<InputMap<CombatAction>>(),
            "InputMap<CombatAction> should be registered by ActionsPlugin"
        );

        // All three ActionState resources must be present (registered by
        // ActionsPlugin via init_resource::<ActionState<T>>).
        assert!(
            app.world().contains_resource::<ActionState<MenuAction>>(),
            "ActionState<MenuAction> should be registered by ActionsPlugin"
        );
        assert!(
            app.world()
                .contains_resource::<ActionState<DungeonAction>>(),
            "ActionState<DungeonAction> should be registered by ActionsPlugin"
        );
        assert!(
            app.world().contains_resource::<ActionState<CombatAction>>(),
            "ActionState<CombatAction> should be registered by ActionsPlugin"
        );
    }

    /// Pressing W triggers DungeonAction::MoveForward via leafwing's mapping.
    #[test]
    fn dungeon_w_press_triggers_move_forward() {
        let mut app = make_test_app();
        KeyCode::KeyW.press(app.world_mut());
        app.update(); // keyboard_input_system reads message → ButtonInput populated
        // → leafwing maps → ActionState<DungeonAction> updated.

        let action_state = app.world().resource::<ActionState<DungeonAction>>();
        assert!(
            action_state.just_pressed(&DungeonAction::MoveForward),
            "Pressing W should trigger DungeonAction::MoveForward"
        );
    }

    /// Verify the many-to-many binding: ArrowUp also triggers MoveForward.
    #[test]
    fn dungeon_arrow_up_also_triggers_move_forward() {
        let mut app = make_test_app();
        KeyCode::ArrowUp.press(app.world_mut());
        app.update();

        let action_state = app.world().resource::<ActionState<DungeonAction>>();
        assert!(
            action_state.just_pressed(&DungeonAction::MoveForward),
            "Pressing ArrowUp should also trigger DungeonAction::MoveForward (many-to-many)"
        );
    }

    /// Pressing Escape triggers MenuAction::Cancel.
    #[test]
    fn menu_escape_triggers_cancel() {
        let mut app = make_test_app();
        KeyCode::Escape.press(app.world_mut());
        app.update();

        let action_state = app.world().resource::<ActionState<MenuAction>>();
        assert!(
            action_state.just_pressed(&MenuAction::Cancel),
            "Pressing Escape should trigger MenuAction::Cancel"
        );
        // Note: Escape is bound to BOTH Cancel and Pause in MenuAction. Both should
        // fire on the same press — leafwing supports many-to-many in the action
        // direction too.
        assert!(
            action_state.just_pressed(&MenuAction::Pause),
            "Pressing Escape should also trigger MenuAction::Pause (Cancel+Pause both bound to Escape)"
        );
    }

    /// Pressing Enter triggers CombatAction::Confirm.
    #[test]
    fn combat_enter_triggers_confirm() {
        let mut app = make_test_app();
        KeyCode::Enter.press(app.world_mut());
        app.update();

        let action_state = app.world().resource::<ActionState<CombatAction>>();
        assert!(
            action_state.just_pressed(&CombatAction::Confirm),
            "Pressing Enter should trigger CombatAction::Confirm"
        );
    }
}
