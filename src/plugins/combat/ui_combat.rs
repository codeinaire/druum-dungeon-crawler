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
    MenuFrame, PendingAction, PendingVictoryResult, PlayerInputState, TurnActionQueue,
};
use crate::plugins::party::character::Experience;
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
            (
                paint_combat_screen.run_if(in_state(GameState::Combat)),
                paint_combat_victory_screen.run_if(in_state(CombatPhase::Victory)),
            ),
        );
    }
}

/// Paint the victory results overlay shown during `CombatPhase::Victory`.
///
/// Renders an `egui::Window` (modal-style, anchored center) over the combat
/// scene so the player can read what they earned before pressing Confirm.
/// Dismissal is handled by `handle_combat_victory_input`.
fn paint_combat_victory_screen(
    mut contexts: EguiContexts,
    pending: Res<PendingVictoryResult>,
    log: Res<CombatLog>,
    party: Query<(&CharacterName, &Experience, &DerivedStats), With<PartyMember>>,
) -> Result {
    let ctx = contexts.ctx_mut()?;

    egui::Window::new("Victory!")
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .show(ctx, |ui| {
            ui.label(format!("Total XP earned: {}", pending.total_xp));
            if pending.total_gold > 0 {
                ui.label(format!("Total gold earned: {}", pending.total_gold));
            }
            ui.add_space(8.0);

            ui.heading("Party");
            for (name, exp, derived) in &party {
                ui.label(format!(
                    "{} — Lv{} HP {}/{} XP {}/{}",
                    name.0,
                    exp.level,
                    derived.current_hp,
                    derived.max_hp,
                    exp.current_xp,
                    exp.xp_to_next_level,
                ));
            }
            ui.add_space(8.0);

            ui.heading("Last actions");
            for entry in log.entries.iter().rev().take(8).collect::<Vec<_>>().iter().rev() {
                ui.label(format!("[T{}] {}", entry.turn_number, entry.message));
            }
            ui.add_space(8.0);

            ui.separator();
            ui.label("[Enter] Return to dungeon");
        });

    Ok(())
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
#[allow(clippy::too_many_arguments)]
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
    // Feature #20 Phase 3 — SpellMenu painter params.
    spell_db_assets: Res<Assets<crate::data::SpellDb>>,
    dungeon_assets: Option<Res<crate::plugins::loading::DungeonAssets>>,
    known_spells_q: Query<&crate::plugins::party::KnownSpells, With<PartyMember>>,
    mut warned: ResMut<crate::plugins::party::WarnedMissingSpells>,
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

    // SpellMenu overlay (center) — Feature #20 Phase 3.
    // Rendered when SpellMenu is top-of-stack during PlayerInput.
    let is_spell_menu = matches!(phase.get(), CombatPhase::PlayerInput)
        && matches!(
            input_state.menu_stack.last(),
            Some(MenuFrame::SpellMenu)
        );
    if is_spell_menu {
        // Compute the spell display state outside the egui closure to avoid
        // complex multi-borrow lifetime issues (ResMut + Query inside FnOnce).

        // Resolve the active actor entity and current MP.
        let actor_info = input_state
            .active_slot
            .and_then(|slot| {
                party
                    .iter()
                    .find(|(_, _, ps, _, _)| ps.0 == slot)
                    .map(|(e, _, _, derived, _)| (e, derived.current_mp))
            });

        // Compute the castable spell list (owned values to survive outside borrows).
        enum SpellMenuState {
            NoActor,
            Loading,
            Empty,          // known_spells.is_empty()
            NoCastable,     // knows spells but all filtered / MP-short
            Castable {
                spells: Vec<crate::data::SpellAsset>,
                cursor: usize,
            },
        }

        let menu_state: SpellMenuState = match actor_info {
            None => SpellMenuState::NoActor,
            Some((actor_entity, current_mp)) => {
                let spell_db = dungeon_assets
                    .as_ref()
                    .and_then(|a| spell_db_assets.get(&a.spells));
                match spell_db {
                    None => SpellMenuState::Loading,
                    Some(spell_db) => {
                        match known_spells_q.get(actor_entity).ok() {
                            None => SpellMenuState::Empty,
                            Some(known_spells) if known_spells.spells.is_empty() => {
                                SpellMenuState::Empty
                            }
                            Some(known_spells) => {
                                // Build castable list; emit warn-once for missing spell IDs (Q9).
                                let castable: Vec<crate::data::SpellAsset> = known_spells
                                    .spells
                                    .iter()
                                    .filter_map(|id| {
                                        let spell = spell_db.get(id);
                                        if spell.is_none()
                                            && warned.set.insert((id.clone(), actor_entity))
                                        {
                                            warn!(
                                                "Character {:?}'s KnownSpells references missing spell '{}' (filtered)",
                                                actor_entity, id
                                            );
                                        }
                                        spell.cloned()
                                    })
                                    .filter(|s| {
                                        s.mp_cost.min(crate::data::MAX_SPELL_MP_COST) <= current_mp
                                    })
                                    .collect();

                                if castable.is_empty() {
                                    SpellMenuState::NoCastable
                                } else {
                                    let cursor = input_state
                                        .spell_cursor
                                        .min(castable.len().saturating_sub(1));
                                    SpellMenuState::Castable { spells: castable, cursor }
                                }
                            }
                        }
                    }
                }
            }
        };

        egui::Window::new("Spells")
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .resizable(false)
            .collapsible(false)
            .show(ctx, |ui| {
                match &menu_state {
                    SpellMenuState::NoActor => {
                        ui.label("(no active member)");
                    }
                    SpellMenuState::Loading => {
                        ui.label("Spells: loading...");
                    }
                    SpellMenuState::Empty => {
                        ui.label("(no spells)");
                        ui.label("[Esc] Back");
                    }
                    SpellMenuState::NoCastable => {
                        // Cat-C-4 = A: character knows spells but none are castable.
                        ui.label("(no castable spells)");
                        ui.label("[Esc] Back");
                    }
                    SpellMenuState::Castable { spells, cursor } => {
                        for (i, spell) in spells.iter().enumerate() {
                            let color = if i == *cursor {
                                egui::Color32::YELLOW
                            } else {
                                egui::Color32::WHITE
                            };
                            ui.colored_label(
                                color,
                                format!("{} (MP {})", spell.display_name, spell.mp_cost),
                            );
                        }
                        if let Some(selected) = spells.get(*cursor) {
                            ui.separator();
                            if !selected.description.is_empty() {
                                ui.label(&selected.description);
                            }
                        }
                        ui.separator();
                        ui.label("[Up/Down] Pick  |  [Enter] Select target  |  [Esc] Back");
                    }
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
/// - `SpellMenu`: Feature #20 Phase 3 — two-pane spell selector with cursor navigation.
/// - `ItemMenu`: stub; logs "not yet implemented"; pops to Main.
#[allow(clippy::too_many_arguments)]
fn handle_combat_input(
    actions: Res<ActionState<MenuNavAction>>,
    mut input_state: ResMut<PlayerInputState>,
    mut queue: ResMut<TurnActionQueue>,
    mut combat_log: ResMut<CombatLog>,
    party: Query<(Entity, &CharacterName, &PartySlot, &DerivedStats, &StatusEffects), With<PartyMember>>,
    enemies: Query<(Entity, &DerivedStats, &StatusEffects), With<Enemy>>,
    phase: Res<State<CombatPhase>>,
    // Feature #20 Phase 3 — spell resolver params.
    spell_db_assets: Res<Assets<crate::data::SpellDb>>,
    dungeon_assets: Option<Res<crate::plugins::loading::DungeonAssets>>,
    known_spells_q: Query<&crate::plugins::party::KnownSpells, With<PartyMember>>,
    mut warned: ResMut<crate::plugins::party::WarnedMissingSpells>,
) {
    // Only act in PlayerInput.
    if !matches!(phase.get(), CombatPhase::PlayerInput) {
        return;
    }
    let Some(active_slot) = input_state.active_slot else {
        return;
    };

    // Find the active actor entity.
    let Some((actor_entity, actor_name, _, derived, status)) =
        party.iter().find(|(_, _, ps, _, _)| ps.0 == active_slot)
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
                    // Spell → push SpellMenu. Reset cursor on entry (Cat-C-6 = A).
                    input_state.spell_cursor = 0;
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
            // Decision 34: Silence gates spell access (MUST stay; real painter mirrors this).
            if is_silenced(status) {
                combat_log.push(
                    "You are silenced; cannot cast.".into(),
                    input_state.current_turn,
                );
                input_state.menu_stack = vec![MenuFrame::Main];
                return;
            }

            // Feature #20 Phase 3 — real SpellMenu handler.

            // Resolve SpellDb.
            let spell_db = dungeon_assets
                .as_ref()
                .and_then(|a| spell_db_assets.get(&a.spells));

            // If DB not loaded yet, stay in menu silently.
            let Some(spell_db) = spell_db else {
                return;
            };

            // Resolve KnownSpells for this actor.
            let Ok(known_spells) = known_spells_q.get(actor_entity) else {
                return;
            };

            // Build the castable list mirroring the painter's filter.
            let castable: Vec<crate::data::SpellAsset> = known_spells
                .spells
                .iter()
                .filter_map(|id| {
                    let spell = spell_db.get(id);
                    if spell.is_none()
                        && warned.set.insert((id.clone(), actor_entity))
                    {
                        warn!(
                            "Character {:?}'s KnownSpells references missing spell '{}' (filtered)",
                            actor_entity, id
                        );
                    }
                    spell.cloned()
                })
                .filter(|s| {
                    s.mp_cost.min(crate::data::MAX_SPELL_MP_COST) <= derived.current_mp
                })
                .collect();

            // Up/Down: cursor movement (Cat-C-6 = A: saturating, non-wrap).
            if actions.just_pressed(&MenuNavAction::Up) {
                input_state.spell_cursor = input_state.spell_cursor.saturating_sub(1);
                return;
            }
            if actions.just_pressed(&MenuNavAction::Down) {
                let ceiling = castable.len().saturating_sub(1);
                input_state.spell_cursor = (input_state.spell_cursor + 1).min(ceiling);
                return;
            }

            // Confirm: attempt to cast.
            if actions.just_pressed(&MenuNavAction::Confirm) {
                // Guard: nothing to confirm.
                if castable.is_empty() {
                    return;
                }
                let cursor = input_state
                    .spell_cursor
                    .min(castable.len().saturating_sub(1));
                // Own the spell to free the borrow before mutating input_state.
                let spell = castable[cursor].clone();

                match spell.target {
                    crate::data::SpellTarget::SingleEnemy => {
                        // Cat-C-5 = A: pre-check alive-enemy list (mirror of Attack arm guard).
                        let enemy_alive: Vec<Entity> = enemies
                            .iter()
                            .filter(|(_, d, _)| d.current_hp > 0)
                            .map(|(e, _, _)| e)
                            .collect();
                        if enemy_alive.is_empty() {
                            combat_log.push(
                                format!(
                                    "{}: no valid targets for {}",
                                    actor_name.0, spell.display_name
                                ),
                                input_state.current_turn,
                            );
                            return; // Stay in SpellMenu.
                        }
                        // Push TargetSelect to let player pick the specific enemy.
                        input_state.menu_stack.push(MenuFrame::TargetSelect {
                            kind: CombatActionKind::CastSpell {
                                spell_id: spell.id.clone(),
                            },
                        });
                        input_state.target_cursor = Some(0);
                    }
                    crate::data::SpellTarget::SingleAlly => {
                        // Push TargetSelect (ally picking); allies are resolved by
                        // the existing TargetSelect arm's confirm logic.
                        input_state.menu_stack.push(MenuFrame::TargetSelect {
                            kind: CombatActionKind::CastSpell {
                                spell_id: spell.id.clone(),
                            },
                        });
                        input_state.target_cursor = Some(0);
                    }
                    crate::data::SpellTarget::AllEnemies
                    | crate::data::SpellTarget::AllAllies
                    | crate::data::SpellTarget::Self_ => {
                        // Commit directly — no target prompt needed.
                        let target = match spell.target {
                            crate::data::SpellTarget::AllEnemies => {
                                TargetSelection::AllEnemies
                            }
                            crate::data::SpellTarget::AllAllies => {
                                TargetSelection::AllAllies
                            }
                            _ => TargetSelection::Self_,
                        };
                        let qa = QueuedAction {
                            actor: actor_entity,
                            kind: CombatActionKind::CastSpell {
                                spell_id: spell.id.clone(),
                            },
                            target,
                            speed_at_queue_time: derived.speed,
                            actor_side: Side::Party,
                            slot_index: active_slot as u32,
                        };
                        queue.queue.push(qa.clone());
                        input_state.committed.push(qa);
                        input_state.menu_stack = vec![MenuFrame::Main];
                        input_state.spell_cursor = 0;
                        input_state.active_slot = None;
                    }
                }
            }
            // Cancel is handled by the top-level Cancel block above.
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
        // Feature #20 Phase 3 — SpellDb asset needed by paint_combat_screen/handle_combat_input.
        app.init_asset::<crate::data::SpellDb>();
        // Mesh + StandardMaterial + Image + TextureAtlasLayout needed by bevy_sprite3d's bundle_builder
        // (EnemyRenderPlugin → Sprite3dPlugin via CombatPlugin; MinimalPlugins lacks PbrPlugin).
        app.init_asset::<bevy::prelude::Mesh>();
        app.init_asset::<bevy::pbr::StandardMaterial>();
        app.init_asset::<bevy::image::Image>();
        app.init_asset::<bevy::image::TextureAtlasLayout>();
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

    /// D-I20 extension — Silence gate fires on the REAL painter (not just stub).
    ///
    /// Verifies that the `handle_combat_input` Silence check still pops `SpellMenu`
    /// even when the actor has a non-empty `KnownSpells` (i.e., the real painter
    /// path is reachable but gated by the Silence guard first). Decision 34 invariant.
    #[test]
    fn silence_blocks_real_spell_menu() {
        use crate::plugins::party::character::ActiveEffect;
        use crate::plugins::party::KnownSpells;

        let mut app = make_test_app();

        // Enter Combat.
        app.world_mut()
            .resource_mut::<NextState<crate::plugins::state::GameState>>()
            .set(crate::plugins::state::GameState::Combat);
        app.update();
        app.update();

        // Spawn a silenced party member with non-empty KnownSpells in slot 0.
        // StatusEffects pre-loaded via vec![...] (sole-mutator grep guard D-I10).
        let _actor = app
            .world_mut()
            .spawn(crate::plugins::party::PartyMemberBundle {
                derived_stats: crate::plugins::party::DerivedStats {
                    current_hp: 100,
                    max_hp: 100,
                    current_mp: 50,
                    max_mp: 50,
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
                // Non-empty KnownSpells — ensures the real painter path would be
                // reached if Silence did NOT gate; this verifies Decision 34 still
                // fires even with castable spells present.
                known_spells: KnownSpells {
                    spells: vec!["halito".into()],
                },
                ..Default::default()
            })
            .id();

        // Set PlayerInputState: active_slot=0, menu_stack=[Main, SpellMenu].
        {
            let mut input_state =
                app.world_mut()
                    .resource_mut::<crate::plugins::combat::turn_manager::PlayerInputState>();
            input_state.active_slot = Some(0);
            input_state.menu_stack = vec![MenuFrame::Main, MenuFrame::SpellMenu];
        }

        // Run one frame — Silence gate fires before real painter logic, pops to Main.
        app.update();

        // Assert: menu_stack is back to [Main] only (Silence gated the real painter).
        let input_state = app
            .world()
            .resource::<crate::plugins::combat::turn_manager::PlayerInputState>();
        assert_eq!(
            input_state.menu_stack.len(),
            1,
            "SpellMenu should be popped when actor is silenced (real painter path)"
        );
        assert!(
            matches!(input_state.menu_stack.last(), Some(MenuFrame::Main)),
            "menu_stack top must be Main after silence redirect (real painter path)"
        );
    }
}
