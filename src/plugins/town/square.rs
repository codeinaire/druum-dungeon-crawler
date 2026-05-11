//! Town Square screen — top-level navigation hub for the Town hub.
//!
//! The Square is the default sub-state when entering `GameState::Town`. It
//! shows a menu of 5 options (Shop / Inn / Temple / Guild / Leave Town) and a
//! header with the current gold balance and day counter.
//!
//! ## Tab for character switching (#25 polish)
//!
//! Tab-key cycling between party members for the Shop's sell mode is deferred
//! to Feature #25 (UI polish). In #18a only the first party member's inventory
//! is shown in the Sell panel.

use bevy::prelude::*;
use bevy_egui::{EguiContexts, egui};
use leafwing_input_manager::prelude::ActionState;

use crate::plugins::input::MenuAction;
use crate::plugins::state::{GameState, TownLocation};
use crate::plugins::town::gold::{GameClock, Gold};

// ─────────────────────────────────────────────────────────────────────────────
// Constants
// ─────────────────────────────────────────────────────────────────────────────

const SQUARE_MENU_OPTIONS: &[&str] = &["Shop", "Inn", "Temple", "Guild", "Leave Town"];

// ─────────────────────────────────────────────────────────────────────────────
// SquareMenuState resource
// ─────────────────────────────────────────────────────────────────────────────

/// Per-frame cursor position in the Square menu (0–4 for the 5 options).
#[derive(Resource, Default, Debug)]
pub struct SquareMenuState {
    pub cursor: usize,
}

// ─────────────────────────────────────────────────────────────────────────────
// paint_town_square — EguiPrimaryContextPass, read-only
// ─────────────────────────────────────────────────────────────────────────────

/// Render the Town Square menu.
///
/// **Read-only** — no `ResMut<T>` or `Commands`. All mutations live in
/// `handle_square_input` (runs in `Update`).
pub fn paint_town_square(
    mut contexts: EguiContexts,
    menu_state: Res<SquareMenuState>,
    gold: Res<Gold>,
    clock: Res<GameClock>,
) -> Result {
    let ctx = contexts.ctx_mut()?;

    egui::TopBottomPanel::top("town_header").show(ctx, |ui| {
        ui.horizontal(|ui| {
            ui.heading("Town Square");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(format!("Day {}  |  {} Gold", clock.day, gold.0));
            });
        });
    });

    egui::CentralPanel::default().show(ctx, |ui| {
        ui.vertical_centered(|ui| {
            ui.add_space(20.0);
            for (i, option) in SQUARE_MENU_OPTIONS.iter().enumerate() {
                if i == menu_state.cursor {
                    ui.colored_label(egui::Color32::YELLOW, format!("> {option}"));
                } else {
                    ui.label(*option);
                }
            }
        });
    });

    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// handle_square_input — Update, mutates
// ─────────────────────────────────────────────────────────────────────────────

/// Handle menu navigation in the Town Square.
///
/// - Up/Down: move cursor (wrap-around).
/// - Confirm: navigate to the selected sub-state (or `TitleScreen` for "Leave Town").
pub fn handle_square_input(
    actions: Res<ActionState<MenuAction>>,
    mut menu_state: ResMut<SquareMenuState>,
    mut next_sub: ResMut<NextState<TownLocation>>,
    mut next_game: ResMut<NextState<GameState>>,
) {
    let count = SQUARE_MENU_OPTIONS.len();

    if actions.just_pressed(&MenuAction::Up) {
        menu_state.cursor = if menu_state.cursor == 0 {
            count - 1
        } else {
            menu_state.cursor - 1
        };
    }

    if actions.just_pressed(&MenuAction::Down) {
        menu_state.cursor = (menu_state.cursor + 1) % count;
    }

    if actions.just_pressed(&MenuAction::Confirm) {
        match menu_state.cursor {
            0 => next_sub.set(TownLocation::Shop),
            1 => next_sub.set(TownLocation::Inn),
            2 => next_sub.set(TownLocation::Temple),
            3 => next_sub.set(TownLocation::Guild),
            4 => next_game.set(GameState::TitleScreen),
            _ => {}
        }
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

    use crate::plugins::input::MenuAction;

    fn make_test_app() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, StatesPlugin));
        app.init_state::<GameState>();
        app.add_sub_state::<TownLocation>();
        app.init_resource::<ActionState<MenuAction>>();
        app.insert_resource(InputMap::<MenuAction>::default());
        app.init_resource::<SquareMenuState>();
        app.init_resource::<Gold>();
        app.init_resource::<GameClock>();
        app.add_systems(Update, handle_square_input.run_if(in_state(TownLocation::Square)));
        // Transition into Town so TownLocation sub-state becomes active.
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Town);
        app.update(); // realise Town + Square
        app.update(); // settle
        app
    }

    /// Confirming on cursor=0 (Shop) navigates to `TownLocation::Shop`.
    #[test]
    fn square_confirm_at_cursor_0_navigates_to_shop() {
        let mut app = make_test_app();

        // Ensure cursor is at 0 (default).
        app.world_mut().resource_mut::<SquareMenuState>().cursor = 0;

        // Press Confirm.
        app.world_mut()
            .resource_mut::<ActionState<MenuAction>>()
            .press(&MenuAction::Confirm);
        app.update();

        // Release to avoid double-fire.
        app.world_mut()
            .resource_mut::<ActionState<MenuAction>>()
            .release(&MenuAction::Confirm);
        app.update();

        assert_eq!(
            *app.world().resource::<State<TownLocation>>().get(),
            TownLocation::Shop,
            "cursor=0 + Confirm should navigate to Shop"
        );
    }

    /// Confirming on cursor=4 (Leave Town) sets `GameState::TitleScreen`.
    #[test]
    fn square_confirm_at_cursor_4_navigates_to_titlescreen() {
        let mut app = make_test_app();

        app.world_mut().resource_mut::<SquareMenuState>().cursor = 4;

        app.world_mut()
            .resource_mut::<ActionState<MenuAction>>()
            .press(&MenuAction::Confirm);
        app.update();
        app.world_mut()
            .resource_mut::<ActionState<MenuAction>>()
            .release(&MenuAction::Confirm);
        app.update();

        assert_eq!(
            *app.world().resource::<State<GameState>>().get(),
            GameState::TitleScreen,
            "cursor=4 + Confirm should navigate to TitleScreen"
        );
    }

    /// Down wraps cursor from the last option back to 0.
    #[test]
    fn square_down_wraps_cursor() {
        let mut app = make_test_app();

        app.world_mut().resource_mut::<SquareMenuState>().cursor = SQUARE_MENU_OPTIONS.len() - 1;

        app.world_mut()
            .resource_mut::<ActionState<MenuAction>>()
            .press(&MenuAction::Down);
        app.update();

        assert_eq!(
            app.world().resource::<SquareMenuState>().cursor,
            0,
            "Down from last option should wrap to 0"
        );
    }

    /// Up wraps cursor from 0 to the last option.
    #[test]
    fn square_up_wraps_cursor() {
        let mut app = make_test_app();

        app.world_mut().resource_mut::<SquareMenuState>().cursor = 0;

        app.world_mut()
            .resource_mut::<ActionState<MenuAction>>()
            .press(&MenuAction::Up);
        app.update();

        assert_eq!(
            app.world().resource::<SquareMenuState>().cursor,
            SQUARE_MENU_OPTIONS.len() - 1,
            "Up from 0 should wrap to last option"
        );
    }
}
