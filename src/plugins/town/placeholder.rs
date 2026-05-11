//! Placeholder screen for Temple and Guild — shown until Feature #18b lands.
//!
//! Both `TownLocation::Temple` and `TownLocation::Guild` route here in #18a.
//! When #18b ships, this file is **deleted** and replaced by real `temple.rs`
//! and `guild.rs` painters. That makes #18b a single-file-deletion diff for
//! the placeholder removal — no square-menu layout changes required.

use bevy::prelude::*;
use bevy_egui::{EguiContexts, egui};
use leafwing_input_manager::prelude::ActionState;

use crate::plugins::input::MenuAction;
use crate::plugins::state::TownLocation;

// ─────────────────────────────────────────────────────────────────────────────
// paint_placeholder — EguiPrimaryContextPass, read-only
// ─────────────────────────────────────────────────────────────────────────────

/// Render the "coming soon" screen for Temple and Guild.
///
/// **Read-only** — no `ResMut<T>` or `Commands`. All mutations live in
/// `handle_placeholder_input`.
pub fn paint_placeholder(
    mut contexts: EguiContexts,
    current: Res<State<TownLocation>>,
) -> Result {
    let ctx = contexts.ctx_mut()?;

    let heading = match current.get() {
        TownLocation::Temple => "Temple",
        TownLocation::Guild => "Guild",
        _ => "Coming Soon",
    };

    egui::CentralPanel::default().show(ctx, |ui| {
        ui.vertical_centered(|ui| {
            ui.add_space(40.0);
            ui.heading(heading);
            ui.add_space(16.0);
            ui.label("Coming in Feature #18b");
            ui.add_space(24.0);
            ui.label("[Esc] Return to Town Square");
        });
    });

    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// handle_placeholder_input — Update, mutates
// ─────────────────────────────────────────────────────────────────────────────

/// Handle input on the placeholder screen. Only Cancel (Esc) is active,
/// which returns the player to the Town Square.
pub fn handle_placeholder_input(
    actions: Res<ActionState<MenuAction>>,
    mut next: ResMut<NextState<TownLocation>>,
) {
    if actions.just_pressed(&MenuAction::Cancel) {
        next.set(TownLocation::Square);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::state::app::StatesPlugin;
    use leafwing_input_manager::prelude::*;

    use crate::plugins::state::{GameState, TownLocation};

    fn make_placeholder_app(start_location: TownLocation) -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, StatesPlugin));
        app.init_state::<GameState>();
        app.add_sub_state::<TownLocation>();
        app.init_resource::<ActionState<MenuAction>>();
        app.insert_resource(InputMap::<MenuAction>::default());
        app.add_systems(
            Update,
            handle_placeholder_input
                .run_if(in_state(TownLocation::Temple).or(in_state(TownLocation::Guild))),
        );

        // Enter Town then navigate to the target location.
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Town);
        app.update();
        app.world_mut()
            .resource_mut::<NextState<TownLocation>>()
            .set(start_location);
        app.update();
        app.update();
        app
    }

    /// Pressing Cancel on the Temple placeholder navigates back to Square.
    #[test]
    fn placeholder_cancel_returns_to_square_from_temple() {
        let mut app = make_placeholder_app(TownLocation::Temple);

        app.world_mut()
            .resource_mut::<ActionState<MenuAction>>()
            .press(&MenuAction::Cancel);
        app.update();
        app.world_mut()
            .resource_mut::<ActionState<MenuAction>>()
            .release(&MenuAction::Cancel);
        app.update();

        assert_eq!(
            *app.world().resource::<State<TownLocation>>().get(),
            TownLocation::Square,
            "Cancel on Temple placeholder should return to Square"
        );
    }

    /// Pressing Cancel on the Guild placeholder navigates back to Square.
    #[test]
    fn placeholder_cancel_returns_to_square_from_guild() {
        let mut app = make_placeholder_app(TownLocation::Guild);

        app.world_mut()
            .resource_mut::<ActionState<MenuAction>>()
            .press(&MenuAction::Cancel);
        app.update();
        app.world_mut()
            .resource_mut::<ActionState<MenuAction>>()
            .release(&MenuAction::Cancel);
        app.update();

        assert_eq!(
            *app.world().resource::<State<TownLocation>>().get(),
            TownLocation::Square,
            "Cancel on Guild placeholder should return to Square"
        );
    }
}
