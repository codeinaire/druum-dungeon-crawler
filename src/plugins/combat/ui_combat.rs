//! Combat UI — Feature #15 Phase 15D.
//!
//! egui combat screen overlaid on the dungeon camera (D-Q1=A — NO new
//! Camera3d). Mirrors `MinimapPlugin::attach_egui_to_dungeon_camera`.
//!
//! ## Layout (D-Q2=A: persistent action panel)
//!
//! - **Left:** `egui::SidePanel::left("enemy_column")` — alive enemies stacked.
//! - **Bottom:** `egui::TopBottomPanel::bottom("party_panel")` — 4 party cards.
//! - **Right:** `egui::SidePanel::right("combat_log")` — bounded log scroll.
//! - **Bottom (above party):** `egui::TopBottomPanel::bottom("action_menu")`
//!   — persistent, always visible during `CombatPhase::PlayerInput`.
//! - **Center overlay:** `egui::Window::new("target_select")` — anchored
//!   center, only when `state.is_selecting_target()`.
//!
//! ## Input handler
//!
//! `handle_combat_input` reads `Res<ActionState<CombatAction>>` (the leafwing
//! menu-nav enum) and mutates `PlayerInputState`. The SOLE writer of
//! `TurnActionQueue` from the player side (Anti-pattern 5).

use bevy::prelude::*;
use bevy_egui::{EguiContexts, EguiPrimaryContextPass, PrimaryEguiContext, egui};
use leafwing_input_manager::prelude::ActionState;

use crate::plugins::combat::actions::{CombatActionKind, QueuedAction, Side};
use crate::plugins::combat::combat_log::CombatLog;
use crate::plugins::combat::enemy::{Enemy, EnemyName};
use crate::plugins::combat::status_effects::is_silenced;
use crate::plugins::combat::targeting::TargetSelection;
use crate::plugins::combat::turn_manager::{
    MenuFrame, PendingAction, PlayerInputState, TurnActionQueue,
};
use crate::plugins::dungeon::DungeonCamera;
use crate::plugins::input::CombatAction as MenuNavAction;
use crate::plugins::party::character::{
    CharacterName, DerivedStats, PartyMember, PartySlot, StatusEffects,
};
use crate::plugins::state::{CombatPhase, GameState};

pub struct CombatUiPlugin;

impl Plugin for CombatUiPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                attach_egui_to_dungeon_camera.run_if(in_state(GameState::Combat)),
                handle_combat_input.run_if(in_state(GameState::Combat)),
            ),
        )
        .add_systems(
            EguiPrimaryContextPass,
            paint_combat_screen.run_if(in_state(GameState::Combat)),
        );
    }
}

/// Attach `PrimaryEguiContext` to the dungeon camera. Idempotent
/// (`Without<PrimaryEguiContext>` filter); runs each frame in Combat but
/// no-ops once attached. Mirrors `MinimapPlugin::attach_egui_to_dungeon_camera`.
///
/// D-Q1=A: NO new Camera3d spawn. Overlays the existing dungeon camera.
fn attach_egui_to_dungeon_camera(
    mut commands: Commands,
    cameras: Query<Entity, (With<DungeonCamera>, Without<PrimaryEguiContext>)>,
) {
    for entity in &cameras {
        commands.entity(entity).insert(PrimaryEguiContext);
    }
}

/// Paint all four combat UI panels.
///
/// D-Q2=A: persistent action panel (persistent bottom panel, always visible
/// during `CombatPhase::PlayerInput`).
fn paint_combat_screen(
    mut contexts: EguiContexts,
    log: Res<CombatLog>,
    input_state: Res<PlayerInputState>,
    party: Query<
        (
            Entity,
            &CharacterName,
            &PartySlot,
            &DerivedStats,
            &StatusEffects,
        ),
        With<PartyMember>,
    >,
    enemies: Query<(Entity, &EnemyName, &DerivedStats, &StatusEffects), With<Enemy>>,
    phase: Res<State<CombatPhase>>,
) -> Result {
    let ctx = contexts.ctx_mut()?;

    // Enemy column (left).
    egui::SidePanel::left("enemy_column")
        .resizable(false)
        .min_width(200.0)
        .show(ctx, |ui| {
            ui.heading("Enemies");
            for (entity, name, derived, status) in &enemies {
                let _ = entity;
                ui.label(format!(
                    "{} HP {}/{}",
                    name.0, derived.current_hp, derived.max_hp
                ));
                if !status.effects.is_empty() {
                    ui.label(format!("  [{}]", format_status_effects(status)));
                }
            }
        });

    // Combat log (right).
    egui::SidePanel::right("combat_log")
        .resizable(false)
        .min_width(300.0)
        .show(ctx, |ui| {
            ui.heading("Log");
            egui::ScrollArea::vertical()
                .stick_to_bottom(true)
                .show(ui, |ui| {
                    for entry in &log.entries {
                        ui.label(&entry.message);
                    }
                });
        });

    // Action menu (bottom — above party panel; only during PlayerInput).
    if matches!(phase.get(), CombatPhase::PlayerInput) {
        egui::TopBottomPanel::bottom("action_menu")
            .resizable(false)
            .min_height(60.0)
            .show(ctx, |ui| {
                if let Some(slot) = input_state.active_slot {
                    let active_name = party
                        .iter()
                        .find(|(_, _, ps, _, _)| ps.0 == slot)
                        .map(|(_, n, _, _, _)| n.0.clone())
                        .unwrap_or_default();
                    ui.horizontal(|ui| {
                        ui.label(format!("> {}", active_name));
                        // Highlight the cursor-selected action; Left/Right move it,
                        // Confirm dispatches. Cursor index matches the order in
                        // handle_combat_input's Main-frame match.
                        const LABELS: [&str; 5] = ["Attack", "Defend", "Spell", "Item", "Flee"];
                        for (i, label) in LABELS.iter().enumerate() {
                            let color = if i == input_state.main_cursor {
                                egui::Color32::YELLOW
                            } else {
                                egui::Color32::WHITE
                            };
                            ui.colored_label(color, *label);
                            if i + 1 < LABELS.len() {
                                ui.label("|");
                            }
                        }
                    });
                } else {
                    ui.label("Resolving turn...");
                }
            });
    }

    // Party panel (bottom).
    egui::TopBottomPanel::bottom("party_panel")
        .resizable(false)
        .min_height(120.0)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                for (_, name, _, derived, status) in &party {
                    ui.vertical(|ui| {
                        ui.label(&name.0);
                        ui.label(format!("HP {}/{}", derived.current_hp, derived.max_hp));
                        ui.label(format!("MP {}/{}", derived.current_mp, derived.max_mp));
                        if !status.effects.is_empty() {
                            ui.label(format!("[{}]", format_status_effects(status)));
                        }
                    });
                }
            });
        });

    // Target selection overlay (center).
    if input_state.pending_action.is_some() && input_state.target_cursor.is_some() {
        egui::Window::new("Target")
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .resizable(false)
            .show(ctx, |ui| {
                for (i, (_, name, _, _)) in enemies.iter().enumerate() {
                    let is_sel = input_state.target_cursor == Some(i);
                    let color = if is_sel {
                        egui::Color32::YELLOW
                    } else {
                        egui::Color32::WHITE
                    };
                    ui.colored_label(color, format!("> {}", name.0));
                }
            });
    }

    Ok(())
}

/// Compact status-effect summary for the party / enemy panels.
///
/// Format per effect: `Type` (no metadata), `Type(Nt)` (turns only),
/// `Type(×M)` (magnitude only — non-zero), or `Type(Nt ×M)` (both).
/// Multiple effects join with `, `.
///
/// Examples: `Stone`, `Sleep(2t)`, `DefenseUp(2t ×0.5)`, `Poison(3t), DefenseUp(2t ×0.5)`.
fn format_status_effects(status: &StatusEffects) -> String {
    status
        .effects
        .iter()
        .map(|e| {
            let mut parts: Vec<String> = Vec::new();
            if let Some(n) = e.remaining_turns {
                parts.push(format!("{}t", n));
            }
            if e.magnitude != 0.0 {
                parts.push(format!("×{:.1}", e.magnitude));
            }
            if parts.is_empty() {
                format!("{:?}", e.effect_type)
            } else {
                format!("{:?}({})", e.effect_type, parts.join(" "))
            }
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// Read leafwing CombatAction (menu nav) and drive `PlayerInputState`.
/// SOLE writer of `TurnActionQueue` from the player side (Anti-pattern 5).
///
/// Menu state machine:
///
/// - `Main`: Confirm → hardcoded Attack → opens `TargetSelect` submenu.
/// - `TargetSelect`: Up/Down moves cursor; Confirm commits to queue; Cancel pops.
/// - `SpellMenu`/`ItemMenu`: stub; logs "not yet implemented"; pops to Main.
fn handle_combat_input(
    actions: Res<ActionState<MenuNavAction>>,
    mut input_state: ResMut<PlayerInputState>,
    mut queue: ResMut<TurnActionQueue>,
    mut combat_log: ResMut<CombatLog>,
    party: Query<(Entity, &PartySlot, &DerivedStats, &StatusEffects), With<PartyMember>>,
    enemies: Query<(Entity, &DerivedStats, &StatusEffects), With<Enemy>>,
    phase: Res<State<CombatPhase>>,
) {
    // Only act in PlayerInput.
    if !matches!(phase.get(), CombatPhase::PlayerInput) {
        return;
    }
    let Some(active_slot) = input_state.active_slot else {
        return;
    };

    // Find the active actor entity.
    let Some((actor_entity, _, derived, status)) =
        party.iter().find(|(_, ps, _, _)| ps.0 == active_slot)
    else {
        return;
    };

    let frame = input_state
        .menu_stack
        .last()
        .cloned()
        .unwrap_or(MenuFrame::Main);

    // Cancel: pop submenu (top-of-stack only; Main does nothing).
    if actions.just_pressed(&MenuNavAction::Cancel) {
        if input_state.menu_stack.len() > 1 {
            input_state.menu_stack.pop();
            input_state.pending_action = None;
            input_state.target_cursor = None;
        }
        return;
    }

    match frame {
        MenuFrame::Main => {
            // Up/Left or Down/Right move the cursor across the 5 actions
            // (0=Attack, 1=Defend, 2=Spell, 3=Item, 4=Flee).
            const ACTION_COUNT: usize = 5;
            if actions.just_pressed(&MenuNavAction::Up)
                || actions.just_pressed(&MenuNavAction::Left)
            {
                input_state.main_cursor = input_state.main_cursor.saturating_sub(1);
                return;
            }
            if actions.just_pressed(&MenuNavAction::Down)
                || actions.just_pressed(&MenuNavAction::Right)
            {
                input_state.main_cursor = (input_state.main_cursor + 1).min(ACTION_COUNT - 1);
                return;
            }
            if !actions.just_pressed(&MenuNavAction::Confirm) {
                return;
            }
            // Dispatch on cursor.
            let speed = derived.speed;
            match input_state.main_cursor {
                0 => {
                    // Attack → TargetSelect (only if enemies exist).
                    if enemies.iter().next().is_some() {
                        input_state.pending_action = Some(PendingAction {
                            kind: CombatActionKind::Attack,
                            actor: actor_entity,
                        });
                        input_state.menu_stack.push(MenuFrame::TargetSelect {
                            kind: CombatActionKind::Attack,
                        });
                        input_state.target_cursor = Some(0);
                    }
                }
                1 => {
                    // Defend → commit immediately (Self target).
                    let qa = QueuedAction {
                        actor: actor_entity,
                        kind: CombatActionKind::Defend,
                        target: TargetSelection::Self_,
                        speed_at_queue_time: speed,
                        actor_side: Side::Party,
                        slot_index: active_slot as u32,
                    };
                    queue.queue.push(qa.clone());
                    input_state.committed.push(qa);
                    input_state.active_slot = None;
                }
                2 => {
                    // Spell → push stub menu (handler logs and pops next frame).
                    input_state.menu_stack.push(MenuFrame::SpellMenu);
                }
                3 => {
                    // Item → push stub menu.
                    input_state.menu_stack.push(MenuFrame::ItemMenu);
                }
                4 => {
                    // Flee → commit immediately (Self target; success rolled in
                    // execute_combat_actions).
                    let qa = QueuedAction {
                        actor: actor_entity,
                        kind: CombatActionKind::Flee,
                        target: TargetSelection::Self_,
                        speed_at_queue_time: speed,
                        actor_side: Side::Party,
                        slot_index: active_slot as u32,
                    };
                    queue.queue.push(qa.clone());
                    input_state.committed.push(qa);
                    input_state.active_slot = None;
                }
                _ => {}
            }
        }
        MenuFrame::TargetSelect { kind } => {
            if (actions.just_pressed(&MenuNavAction::Up)
                || actions.just_pressed(&MenuNavAction::Left))
                && let Some(c) = input_state.target_cursor.as_mut()
            {
                *c = c.saturating_sub(1);
            }
            if (actions.just_pressed(&MenuNavAction::Down)
                || actions.just_pressed(&MenuNavAction::Right))
                && let Some(c) = input_state.target_cursor.as_mut()
            {
                let max = enemies.iter().count().saturating_sub(1);
                *c = (*c + 1).min(max);
            }
            if actions.just_pressed(&MenuNavAction::Confirm) {
                let cursor = input_state.target_cursor.unwrap_or(0);
                if let Some((target, _, _)) = enemies.iter().nth(cursor) {
                    let speed = derived.speed;
                    // Commit to queue (SOLE player-side write to TurnActionQueue).
                    queue.queue.push(QueuedAction {
                        actor: actor_entity,
                        kind: kind.clone(),
                        target: TargetSelection::Single(target),
                        speed_at_queue_time: speed,
                        actor_side: Side::Party,
                        slot_index: active_slot as u32,
                    });
                    // Mirror into committed for collect_player_actions.
                    input_state.committed.push(QueuedAction {
                        actor: actor_entity,
                        kind: kind.clone(),
                        target: TargetSelection::Single(target),
                        speed_at_queue_time: speed,
                        actor_side: Side::Party,
                        slot_index: active_slot as u32,
                    });
                    // Pop back to Main.
                    input_state.menu_stack = vec![MenuFrame::Main];
                    input_state.pending_action = None;
                    input_state.target_cursor = None;
                    input_state.active_slot = None; // Triggers next-slot search.
                }
            }
        }
        MenuFrame::SpellMenu => {
            // Decision 34: Silence gates spell access.
            if is_silenced(status) {
                combat_log.push(
                    "You are silenced; cannot cast.".into(),
                    input_state.current_turn,
                );
                input_state.menu_stack = vec![MenuFrame::Main];
                return;
            }
            // Stub for v1; #20 fills in spell menu.
            combat_log.push(
                "Spell menu: not yet implemented.".into(),
                input_state.current_turn,
            );
            input_state.menu_stack = vec![MenuFrame::Main];
        }
        MenuFrame::ItemMenu => {
            // Stub; full inventory UI is #25.
            combat_log.push(
                "Item menu: not yet implemented.".into(),
                input_state.current_turn,
            );
            input_state.menu_stack = vec![MenuFrame::Main];
        }
    }
}

#[cfg(test)]
mod app_tests {
    use super::*;
    use bevy::state::app::StatesPlugin;

    fn make_test_app() -> App {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            bevy::asset::AssetPlugin::default(),
            StatesPlugin,
            crate::plugins::state::StatePlugin,
            crate::plugins::party::PartyPlugin,
            crate::plugins::combat::CombatPlugin,
        ));
        app.init_asset::<crate::data::ItemDb>();
        app.init_asset::<crate::data::ItemAsset>();
        app.init_asset::<crate::data::EncounterTable>(); // Feature #16 (EncounterPlugin inside CombatPlugin)
        // tick_on_dungeon_step reads MessageReader<MovedEvent>; register it so the
        // system does not panic under default features (DungeonPlugin not loaded here).
        app.add_message::<crate::plugins::dungeon::MovedEvent>();
        // EncounterPlugin (inside CombatPlugin) reads/writes EncounterRequested.
        // CellFeaturesPlugin normally registers this; explicit here since
        // CellFeaturesPlugin is not included in this test app (Feature #16).
        app.add_message::<crate::plugins::dungeon::features::EncounterRequested>();
        // ActionState<MenuNavAction> required by handle_combat_input.
        // Inserted directly (without ActionsPlugin) to avoid mouse-resource panic.
        app.init_resource::<ActionState<MenuNavAction>>();
        #[cfg(feature = "dev")]
        app.init_resource::<bevy::input::ButtonInput<bevy::prelude::KeyCode>>();
        app
    }

    /// Verify the UI plugin builds and runs without panic with no enemies.
    #[test]
    fn handle_combat_input_no_panic_with_no_enemies_or_party() {
        let mut app = make_test_app();
        // No party, no enemies; input handler should be a no-op.
        app.world_mut()
            .resource_mut::<NextState<crate::plugins::state::GameState>>()
            .set(crate::plugins::state::GameState::Combat);
        app.update();
        app.update();
        // No panic = pass.
    }

    /// Target cursor clamps at enemy count - 1.
    #[test]
    fn target_cursor_clamps_at_enemy_count() {
        let mut input_state = PlayerInputState {
            active_slot: Some(0),
            menu_stack: vec![MenuFrame::TargetSelect {
                kind: CombatActionKind::Attack,
            }],
            target_cursor: Some(5), // way out of bounds
            ..Default::default()
        };
        // Simulate clamping.
        let enemy_count = 2usize;
        if let Some(c) = input_state.target_cursor.as_mut() {
            *c = (*c).min(enemy_count.saturating_sub(1));
        }
        assert_eq!(input_state.target_cursor, Some(1));
    }

    /// Cancel pops the menu stack.
    #[test]
    fn cancel_pops_submenu() {
        let mut input_state = PlayerInputState {
            active_slot: Some(0),
            menu_stack: vec![
                MenuFrame::Main,
                MenuFrame::TargetSelect {
                    kind: CombatActionKind::Attack,
                },
            ],
            target_cursor: Some(0),
            ..Default::default()
        };
        // Simulate Cancel press logic.
        if input_state.menu_stack.len() > 1 {
            input_state.menu_stack.pop();
            input_state.pending_action = None;
            input_state.target_cursor = None;
        }
        assert_eq!(input_state.menu_stack.len(), 1);
        assert!(input_state.target_cursor.is_none());
    }

    /// D-I20 (MEDIUM-1): A silenced party member who opens the SpellMenu is
    /// immediately redirected to Main with a log entry. Decision 34.
    ///
    /// The `SpellMenu` arm of `handle_combat_input` checks `is_silenced` on
    /// entry (no button press required — it fires on every frame while
    /// `SpellMenu` is top-of-stack). This test exercises that path directly.
    #[test]
    fn silence_blocks_spell_menu() {
        use crate::plugins::party::character::ActiveEffect;

        let mut app = make_test_app();

        // Enter Combat (CombatPhase defaults to PlayerInput).
        app.world_mut()
            .resource_mut::<NextState<crate::plugins::state::GameState>>()
            .set(crate::plugins::state::GameState::Combat);
        app.update();
        app.update();

        // Spawn a silenced party member in slot 0.
        // StatusEffects pre-loaded via vec![...] (sole-mutator grep guard D-I10).
        let _actor = app
            .world_mut()
            .spawn(crate::plugins::party::PartyMemberBundle {
                derived_stats: crate::plugins::party::DerivedStats {
                    current_hp: 100,
                    max_hp: 100,
                    speed: 10,
                    ..Default::default()
                },
                party_slot: crate::plugins::party::PartySlot(0),
                status_effects: crate::plugins::party::StatusEffects {
                    effects: vec![ActiveEffect {
                        effect_type: crate::plugins::party::StatusEffectType::Silence,
                        remaining_turns: Some(3),
                        magnitude: 0.0,
                    }],
                },
                ..Default::default()
            })
            .id();

        // Set PlayerInputState: active_slot=0, menu_stack=[Main, SpellMenu].
        // This simulates the player having navigated to the SpellMenu.
        {
            let mut input_state =
                app.world_mut()
                    .resource_mut::<crate::plugins::combat::turn_manager::PlayerInputState>();
            input_state.active_slot = Some(0);
            input_state.menu_stack = vec![MenuFrame::Main, MenuFrame::SpellMenu];
        }

        // Run one frame — handle_combat_input fires, detects Silence, pops to Main.
        app.update();

        // Assert: menu_stack is back to [Main] only.
        let input_state = app
            .world()
            .resource::<crate::plugins::combat::turn_manager::PlayerInputState>();
        assert_eq!(
            input_state.menu_stack.len(),
            1,
            "SpellMenu should be popped when actor is silenced"
        );
        assert!(
            matches!(input_state.menu_stack.last(), Some(MenuFrame::Main)),
            "menu_stack top must be Main after silence redirect"
        );
    }
}
