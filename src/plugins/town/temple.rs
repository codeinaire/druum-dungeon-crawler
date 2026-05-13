//! Town Temple screen — revive Dead characters and cure severe status effects.
//!
//! ## Cure set
//!
//! Temple handles `Dead` (Revive mode only), `Stone`, `Paralysis`, and `Sleep`.
//! `Poison` is Inn-only. `Silence` is deferred (no in-dungeon source yet in v1).
//! Buff variants (`AttackUp`/`DefenseUp`/`SpeedUp`/`Regen`) are not cured here.
//!
//! ## Revive invariants
//!
//! Revive writes `current_hp = 1` (Wizardry convention — barely alive after
//! resurrection). The caller-clamp at `inventory.rs:495-499` preserves that 1
//! against the re-derived `max_hp`. Order MUST be:
//! 1. `effects.retain(|e| e.effect_type != Dead)` — remove Dead effect.
//! 2. `derived.current_hp = 1` — set HP BEFORE firing the event.
//! 3. Fire `EquipmentChangedEvent { slot: EquipSlot::None }` — triggers
//!    `recompute_derived_stats_on_equipment_change`, which re-derives stats and
//!    applies the caller-clamp `current_hp = old_current_hp.min(max_hp)`.
//!    Because `max_hp` is now positive (Dead was removed before the event fires),
//!    `1.min(max_hp) = 1`. If the order were reversed, the event fires while Dead
//!    is still present → `max_hp = 0` → clamp `1.min(0) = 0` → player is dead
//!    by HP even after revival.
//!
//! ## Cure invariants
//!
//! Cure auto-picks the first eligible severe status in priority order
//! `Stone > Paralysis > Sleep` (user decision 7; matches Inn's simplicity).
//! The status-pick dialog is deferred to Feature #25 polish.
//! Cure MUST NOT touch `Dead` — `first_curable_status` filters it out explicitly.
//! Do NOT touch `current_hp` on Cure (only Revive needs the hp=1 write).
//!
//! ## Exception path note
//!
//! Both Revive and Cure mutate `StatusEffects.effects` directly via
//! `effects.retain` — this is the documented exception path, NOT going through
//! `apply_status_handler`. Routing through `apply_status_handler` would re-trigger
//! merge logic and only emit `EquipmentChangedEvent` for buff variants, missing
//! Stone/Paralysis/Sleep. Same documented exception as Inn's Poison cure at
//! `inn.rs:153-162`.

use bevy::prelude::*;
use bevy_egui::{EguiContexts, egui};
use leafwing_input_manager::prelude::ActionState;

use crate::data::town::{MAX_TEMPLE_COST, TownServices};
use crate::plugins::input::MenuAction;
use crate::plugins::loading::TownAssets;
use crate::plugins::party::character::{
    CharacterName, DerivedStats, Experience, PartyMember, PartySlot, StatusEffectType, StatusEffects,
};
use crate::plugins::party::inventory::{EquipSlot, EquipmentChangedEvent};
use crate::plugins::state::TownLocation;
use crate::plugins::town::gold::Gold;

// ─────────────────────────────────────────────────────────────────────────────
// TempleState resource
// ─────────────────────────────────────────────────────────────────────────────

/// UI cursor state for the Temple screen.
#[derive(Resource, Default, Debug)]
pub struct TempleState {
    pub mode: TempleMode,
    /// Index into the sorted-by-Entity active party list.
    pub cursor: usize,
    /// Alias for cursor (party_target) — kept for API parity with ShopState.
    pub party_target: usize,
}

/// Which service the Temple is offering.
#[derive(Debug, PartialEq, Eq, Clone, Copy, Default)]
pub enum TempleMode {
    /// Revive a Dead party member (removes `Dead`, sets `current_hp = 1`).
    #[default]
    Revive,
    /// Cure a severe status effect (Stone/Paralysis/Sleep — NOT Dead, NOT Poison).
    Cure,
}

// ─────────────────────────────────────────────────────────────────────────────
// Pure helpers — testable without an App
// ─────────────────────────────────────────────────────────────────────────────

/// Compute the gold cost to revive a character at the given `level`.
///
/// Formula: `base + per_level * level`, saturating, clamped to
/// `[1, MAX_TEMPLE_COST]`. The `.max(1)` is the defense-in-depth guard against
/// a RON typo that sets both base and per_level to 0 (would make Revive free).
pub fn revive_cost(services: &TownServices, level: u32) -> u32 {
    services
        .temple_revive_cost_base
        .saturating_add(services.temple_revive_cost_per_level.saturating_mul(level))
        .clamp(1, MAX_TEMPLE_COST)
}

/// Look up the gold cost to cure `kind` at the Temple.
///
/// Returns `None` if `kind` is `Dead` (Revive is the sole path for Dead removal),
/// or if `kind` is not in `services.temple_cure_costs`.
/// Returns `Some(cost.min(MAX_TEMPLE_COST))` on match.
pub fn cure_cost(services: &TownServices, kind: StatusEffectType) -> Option<u32> {
    // Dead cannot be cured — only Revive removes Dead.
    if kind == StatusEffectType::Dead {
        return None;
    }
    services
        .temple_cure_costs
        .iter()
        .find(|(t, _)| *t == kind)
        .map(|(_, cost)| (*cost).min(MAX_TEMPLE_COST))
}

/// Auto-pick the first curable severe status in priority order Stone > Paralysis > Sleep.
///
/// Returns `Some((kind, cost))` for the first curable status found, or `None`
/// if the character has no curable severe status. Poison, Dead, and buff variants
/// are not in the eligible set.
pub fn first_curable_status(
    services: &TownServices,
    status: &StatusEffects,
) -> Option<(StatusEffectType, u32)> {
    let priority = [
        StatusEffectType::Stone,
        StatusEffectType::Paralysis,
        StatusEffectType::Sleep,
    ];
    for kind in &priority {
        if status.has(*kind)
            && let Some(cost) = cure_cost(services, *kind)
        {
            return Some((*kind, cost));
        }
    }
    None
}

// ─────────────────────────────────────────────────────────────────────────────
// paint_temple — EguiPrimaryContextPass, read-only
// ─────────────────────────────────────────────────────────────────────────────

/// Render the Temple screen.
///
/// **Read-only** — no `ResMut<T>` or `Commands`. All mutations live in
/// `handle_temple_action`.
#[allow(clippy::type_complexity)]
pub fn paint_temple(
    mut contexts: EguiContexts,
    gold: Res<Gold>,
    temple_state: Res<TempleState>,
    town_assets: Option<Res<TownAssets>>,
    services_assets: Res<Assets<TownServices>>,
    party: Query<
        (Entity, &CharacterName, &PartySlot, &StatusEffects, &Experience, &DerivedStats),
        With<PartyMember>,
    >,
) -> Result {
    let ctx = contexts.ctx_mut()?;

    egui::TopBottomPanel::top("temple_header").show(ctx, |ui| {
        ui.horizontal(|ui| {
            ui.heading("Temple");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(format!("{} Gold", gold.0));
            });
        });
    });

    egui::CentralPanel::default().show(ctx, |ui| {
        let services = town_assets
            .as_ref()
            .and_then(|a| services_assets.get(&a.services));

        // Sort by PartySlot for deterministic cursor alignment with the rest of
        // the codebase (Guild, Shop, dev hotkeys all key off PartySlot).
        let mut members: Vec<(Entity, &CharacterName, &PartySlot, &StatusEffects, &Experience, &DerivedStats)> =
            party.iter().collect();
        members.sort_by_key(|(_, _, slot, _, _, _)| slot.0);

        let mode_label = match temple_state.mode {
            TempleMode::Revive => "Revive",
            TempleMode::Cure => "Cure",
        };
        ui.label(format!("Mode: {mode_label}"));
        ui.add_space(4.0);

        if members.is_empty() {
            ui.label("(No active party members)");
        } else {
            let idx = temple_state.party_target.min(members.len().saturating_sub(1));

            // Full party roster with cursor marker + status icons.
            ui.label("Party:");
            for (i, (_, name, slot, status, xp, derived)) in members.iter().enumerate() {
                let cursor_marker = if i == idx { "> " } else { "  " };
                let dead = if status.has(StatusEffectType::Dead) { " [DEAD]" } else { "" };
                let stone = if status.has(StatusEffectType::Stone) { " [STONE]" } else { "" };
                let paralyzed = if status.has(StatusEffectType::Paralysis) { " [PARALYSIS]" } else { "" };
                let asleep = if status.has(StatusEffectType::Sleep) { " [SLEEP]" } else { "" };
                let poison = if status.has(StatusEffectType::Poison) { " [POISON]" } else { "" };
                let line = format!(
                    "{}slot {} — {} (lvl {})  HP:{}/{}{}{}{}{}{}",
                    cursor_marker,
                    slot.0,
                    name.0,
                    xp.level,
                    derived.current_hp,
                    derived.max_hp,
                    dead, stone, paralyzed, asleep, poison,
                );
                if i == idx {
                    ui.colored_label(egui::Color32::YELLOW, line);
                } else {
                    ui.label(line);
                }
            }
            ui.add_space(8.0);

            let (_, _, _, status, xp, _) = members[idx];
            let action_label = if let Some(svc) = services {
                match temple_state.mode {
                    TempleMode::Revive => {
                        let cost = revive_cost(svc, xp.level);
                        if status.has(StatusEffectType::Dead) {
                            format!("Revive (lvl {}): {}g", xp.level, cost)
                        } else {
                            "(target is alive — Revive does nothing)".to_string()
                        }
                    }
                    TempleMode::Cure => {
                        match first_curable_status(svc, status) {
                            Some((kind, cost)) => format!("Cure {:?}: {}g", kind, cost),
                            None => "(no curable status)".to_string(),
                        }
                    }
                }
            } else {
                "(loading...)".to_string()
            };

            ui.label(action_label);
        }

        ui.add_space(16.0);
        ui.label("[Left/Right] Switch mode  |  [Up/Down] Pick member  |  [Enter] Confirm  |  [Esc] Back");
    });

    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// handle_temple_action — Update, mutates
// ─────────────────────────────────────────────────────────────────────────────

/// Handle input in the Temple screen.
///
/// On `MenuAction::Cancel`: return to Square immediately.
/// On `MenuAction::Up`/`Down`: move cursor through active party (sorted by Entity).
/// On `MenuAction::Left`/`Right`: toggle Revive / Cure mode.
/// On `MenuAction::Confirm`: execute the current mode's action on the cursor target.
///
/// ## Revive order (CRITICAL — see module doc)
/// 1. `effects.retain(|e| e.effect_type != Dead)` — remove Dead FIRST.
/// 2. `derived.current_hp = 1` — set HP BEFORE firing the event.
/// 3. `writer.write(EquipmentChangedEvent { slot: EquipSlot::None })`.
/// 4. `gold.try_spend(cost)`.
#[allow(clippy::too_many_arguments)]
#[allow(clippy::type_complexity)]
pub fn handle_temple_action(
    actions: Res<ActionState<MenuAction>>,
    mut gold: ResMut<Gold>,
    mut temple_state: ResMut<TempleState>,
    town_assets: Option<Res<TownAssets>>,
    services_assets: Res<Assets<TownServices>>,
    mut next_sub: ResMut<NextState<TownLocation>>,
    mut writer: MessageWriter<EquipmentChangedEvent>,
    mut toasts: ResMut<crate::plugins::town::toast::Toasts>,
    mut party: Query<(Entity, &PartySlot, &CharacterName, &mut DerivedStats, &mut StatusEffects, &Experience), With<PartyMember>>,
) {
    if actions.just_pressed(&MenuAction::Cancel) {
        next_sub.set(TownLocation::Square);
        return;
    }

    // Mode toggle via Left/Right (MenuAction has no SwitchMode variant).
    if actions.just_pressed(&MenuAction::Left) || actions.just_pressed(&MenuAction::Right) {
        temple_state.mode = match temple_state.mode {
            TempleMode::Revive => TempleMode::Cure,
            TempleMode::Cure => TempleMode::Revive,
        };
        return;
    }

    // Cursor navigation.
    let party_count = party.iter().count();
    if actions.just_pressed(&MenuAction::Up) {
        if party_count > 0 {
            temple_state.party_target =
                temple_state.party_target.saturating_sub(1).max(0);
        }
        return;
    }
    if actions.just_pressed(&MenuAction::Down) {
        if party_count > 0 {
            temple_state.party_target =
                (temple_state.party_target + 1).min(party_count.saturating_sub(1));
        }
        return;
    }

    if !actions.just_pressed(&MenuAction::Confirm) {
        return;
    }

    let Some(assets) = town_assets else {
        return;
    };
    let Some(services) = services_assets.get(&assets.services) else {
        return;
    };

    // Resolve target by PartySlot order (deterministic, matches painter +
    // dev hotkeys + Guild/Shop conventions).
    let mut members: Vec<(Entity, usize)> = party
        .iter()
        .map(|(e, slot, _, _, _, _)| (e, slot.0))
        .collect();
    members.sort_by_key(|(_, slot)| *slot);
    let Some(&(target, _)) = members.get(temple_state.party_target) else {
        info!("Temple: no party member at cursor {}", temple_state.party_target);
        return;
    };

    let Ok((_, _, name, mut derived, mut status, xp)) = party.get_mut(target) else {
        return;
    };
    let name_str = name.0.clone();

    match temple_state.mode {
        TempleMode::Revive => {
            if !status.has(StatusEffectType::Dead) {
                info!("Temple revive: target is not dead");
                toasts.push(format!("{name_str} is alive — nothing to revive."));
                return;
            }

            let cost = revive_cost(services, xp.level);
            if gold.0 < cost {
                info!(
                    "Temple revive: insufficient gold (have {}, need {})",
                    gold.0, cost
                );
                toasts.push(format!("Not enough gold to revive {name_str} (need {cost}g)."));
                return;
            }

            // CRITICAL ORDER: retain → set hp=1 → fire event → deduct gold.
            status
                .effects
                .retain(|e| e.effect_type != StatusEffectType::Dead);
            derived.current_hp = 1;
            writer.write(EquipmentChangedEvent {
                character: target,
                slot: EquipSlot::None,
            });
            let _ = gold.try_spend(cost);
            toasts.push(format!("{name_str} has been revived! ({cost}g)"));
            info!(
                "Temple revived {:?} for {} gold (level {})",
                target, cost, xp.level
            );
        }
        TempleMode::Cure => {
            let Some((kind, cost)) = first_curable_status(services, &status) else {
                info!("Temple cure: target has no curable status");
                toasts.push(format!("{name_str} has no curable status."));
                return;
            };

            if gold.0 < cost {
                info!(
                    "Temple cure: insufficient gold (have {}, need {})",
                    gold.0, cost
                );
                toasts.push(format!("Not enough gold to cure {kind:?} (need {cost}g)."));
                return;
            }

            // Retain removes the status; do NOT touch current_hp on Cure.
            status.effects.retain(|e| e.effect_type != kind);
            writer.write(EquipmentChangedEvent {
                character: target,
                slot: EquipSlot::None,
            });
            let _ = gold.try_spend(cost);
            toasts.push(format!("Cured {kind:?} on {name_str}. ({cost}g)"));
            info!(
                "Temple cured {:?} on {:?} for {} gold",
                kind, target, cost
            );
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::asset::AssetPlugin;
    use bevy::state::app::StatesPlugin;
    use leafwing_input_manager::prelude::*;

    use bevy::asset::Handle;
    use crate::data::town::TownServices;
    use crate::plugins::input::MenuAction;
    use crate::plugins::loading::TownAssets;
    use crate::plugins::party::character::{
        ActiveEffect, BaseStats, DerivedStats, Experience, PartyMember, StatusEffectType,
        StatusEffects, derive_stats,
    };
    use crate::plugins::party::inventory::EquipmentChangedEvent;
    use crate::plugins::state::{GameState, TownLocation};
    use crate::plugins::town::gold::Gold;

    fn make_temple_services() -> TownServices {
        TownServices {
            inn_rest_cost: 10,
            inn_rest_cures: vec![StatusEffectType::Poison],
            temple_revive_cost_base: 100,
            temple_revive_cost_per_level: 50,
            temple_cure_costs: vec![
                (StatusEffectType::Stone, 250),
                (StatusEffectType::Paralysis, 100),
                (StatusEffectType::Sleep, 50),
            ],
        }
    }

    fn make_temple_test_app() -> App {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            StatesPlugin,
        ));
        app.init_state::<GameState>();
        app.add_sub_state::<TownLocation>();

        app.init_resource::<Gold>();
        app.init_resource::<TempleState>();
        app.init_resource::<crate::plugins::town::toast::Toasts>();
        app.init_resource::<ActionState<MenuAction>>();
        app.insert_resource(InputMap::<MenuAction>::default());

        app.init_asset::<TownServices>();
        let services = make_temple_services();
        let services_handle = app
            .world_mut()
            .resource_mut::<Assets<TownServices>>()
            .add(services);
        let mock_town_assets = TownAssets {
            shop_stock: Handle::default(),
            recruit_pool: Handle::default(),
            services: services_handle,
        };
        app.insert_resource(mock_town_assets);

        app.add_message::<EquipmentChangedEvent>();
        app.add_systems(
            Update,
            handle_temple_action.run_if(in_state(TownLocation::Temple)),
        );

        // Transition into Town / Temple.
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Town);
        app.update();
        app.world_mut()
            .resource_mut::<NextState<TownLocation>>()
            .set(TownLocation::Temple);
        app.update();
        app.update();

        app
    }

    fn spawn_party_member(
        app: &mut App,
        hp: u32,
        mp: u32,
        level: u32,
        effects: Vec<ActiveEffect>,
    ) -> Entity {
        let base = BaseStats {
            vitality: 10,
            intelligence: 8,
            piety: 8,
            ..Default::default()
        };
        let mut derived = derive_stats(&base, &[], &StatusEffects { effects: effects.clone() }, level.max(1));
        derived.max_hp = hp.max(1);
        derived.max_mp = mp.max(1);
        derived.current_hp = if effects.iter().any(|e| e.effect_type == StatusEffectType::Dead) {
            0
        } else {
            derived.max_hp
        };
        derived.current_mp = derived.max_mp;

        let xp = Experience {
            level: level.max(1),
            ..Default::default()
        };

        // PartySlot index is irrelevant to these tests — they spawn one member
        // and expect cursor 0 to target it. Using a counter would be overkill.
        app.world_mut().spawn((
            PartyMember,
            PartySlot(0),
            CharacterName("Test Hero".to_string()),
            base,
            derived,
            StatusEffects { effects },
            xp,
        )).id()
    }

    fn make_dead_effect() -> ActiveEffect {
        ActiveEffect {
            effect_type: StatusEffectType::Dead,
            remaining_turns: None,
            magnitude: 0.0,
        }
    }

    fn make_status_effect(kind: StatusEffectType) -> ActiveEffect {
        ActiveEffect {
            effect_type: kind,
            remaining_turns: None,
            magnitude: 0.0,
        }
    }

    fn press_confirm(app: &mut App) {
        app.world_mut()
            .resource_mut::<ActionState<MenuAction>>()
            .press(&MenuAction::Confirm);
        app.update();
        app.world_mut()
            .resource_mut::<ActionState<MenuAction>>()
            .release(&MenuAction::Confirm);
        app.update();
    }

    fn press_cancel(app: &mut App) {
        app.world_mut()
            .resource_mut::<ActionState<MenuAction>>()
            .press(&MenuAction::Cancel);
        app.update();
        app.world_mut()
            .resource_mut::<ActionState<MenuAction>>()
            .release(&MenuAction::Cancel);
        app.update();
    }

    // ── Pure helper unit tests ────────────────────────────────────────────────

    #[test]
    fn revive_cost_scales_linearly_with_level() {
        let svc = make_temple_services();
        assert_eq!(revive_cost(&svc, 0), 100, "level 0: base=100 + per_level*0=100");
        assert_eq!(revive_cost(&svc, 1), 150, "level 1: 100 + 50*1 = 150");
        assert_eq!(revive_cost(&svc, 5), 350, "level 5: 100 + 50*5 = 350");
    }

    #[test]
    fn revive_cost_saturates_at_max_temple_cost() {
        let svc = TownServices {
            temple_revive_cost_base: u32::MAX,
            temple_revive_cost_per_level: u32::MAX,
            ..Default::default()
        };
        assert_eq!(revive_cost(&svc, 100), MAX_TEMPLE_COST);
    }

    #[test]
    fn revive_cost_guards_against_zero_via_max_1() {
        let svc = TownServices {
            temple_revive_cost_base: 0,
            temple_revive_cost_per_level: 0,
            ..Default::default()
        };
        assert_eq!(revive_cost(&svc, 1), 1, "0 base + 0 per_level must return 1 via .max(1)");
    }

    #[test]
    fn cure_cost_returns_none_for_dead() {
        // Even if Dead is explicitly in the cure-cost list, it must return None.
        let svc = TownServices {
            temple_cure_costs: vec![
                (StatusEffectType::Dead, 999),
                (StatusEffectType::Stone, 250),
            ],
            ..Default::default()
        };
        assert_eq!(cure_cost(&svc, StatusEffectType::Dead), None);
    }

    #[test]
    fn cure_cost_returns_lookup_for_stone() {
        let svc = make_temple_services();
        assert_eq!(cure_cost(&svc, StatusEffectType::Stone), Some(250));
    }

    #[test]
    fn first_curable_status_picks_stone_when_present() {
        let svc = make_temple_services();
        let status = StatusEffects {
            effects: vec![
                make_status_effect(StatusEffectType::Stone),
                make_status_effect(StatusEffectType::Sleep),
            ],
        };
        assert_eq!(
            first_curable_status(&svc, &status),
            Some((StatusEffectType::Stone, 250))
        );
    }

    #[test]
    fn first_curable_status_picks_paralysis_when_only_paralysis_and_sleep() {
        let svc = make_temple_services();
        let status = StatusEffects {
            effects: vec![
                make_status_effect(StatusEffectType::Paralysis),
                make_status_effect(StatusEffectType::Sleep),
            ],
        };
        assert_eq!(
            first_curable_status(&svc, &status),
            Some((StatusEffectType::Paralysis, 100))
        );
    }

    #[test]
    fn first_curable_status_returns_none_for_no_severe_status() {
        let svc = make_temple_services();
        let status = StatusEffects {
            effects: vec![make_status_effect(StatusEffectType::Poison)],
        };
        assert_eq!(first_curable_status(&svc, &status), None);
    }

    // ── Integration tests ─────────────────────────────────────────────────────

    #[test]
    fn revive_dead_member_clears_dead_and_sets_hp_to_1() {
        let mut app = make_temple_test_app();
        app.world_mut().resource_mut::<Gold>().0 = 1000;

        let member = spawn_party_member(&mut app, 20, 10, 1, vec![make_dead_effect()]);
        // Override hp to 0 to mark the dead state.
        app.world_mut()
            .get_mut::<DerivedStats>(member)
            .unwrap()
            .current_hp = 0;

        press_confirm(&mut app);

        let status = app.world().get::<StatusEffects>(member).unwrap();
        assert!(!status.has(StatusEffectType::Dead), "Dead should be removed");

        let derived = app.world().get::<DerivedStats>(member).unwrap();
        assert_eq!(derived.current_hp, 1, "Revived member must have current_hp = 1");
        assert!(derived.max_hp > 0, "max_hp should be > 0 after revive");

        let gold = app.world().resource::<Gold>();
        assert_eq!(gold.0, 850, "Gold should be 1000 - 150 (level 1 cost)");
    }

    #[test]
    fn revive_rejects_non_dead_target() {
        let mut app = make_temple_test_app();
        app.world_mut().resource_mut::<Gold>().0 = 1000;

        let member = spawn_party_member(&mut app, 20, 10, 1, vec![]);

        press_confirm(&mut app);

        let status = app.world().get::<StatusEffects>(member).unwrap();
        // No change — was never Dead.
        assert!(!status.has(StatusEffectType::Dead));
        let gold = app.world().resource::<Gold>();
        assert_eq!(gold.0, 1000, "Gold should not change when target is not dead");
    }

    #[test]
    fn revive_rejects_when_insufficient_gold() {
        let mut app = make_temple_test_app();
        app.world_mut().resource_mut::<Gold>().0 = 50; // cost for level 5 is 350

        let member = spawn_party_member(&mut app, 20, 10, 5, vec![make_dead_effect()]);
        app.world_mut()
            .get_mut::<DerivedStats>(member)
            .unwrap()
            .current_hp = 0;

        press_confirm(&mut app);

        let status = app.world().get::<StatusEffects>(member).unwrap();
        assert!(status.has(StatusEffectType::Dead), "Still dead when insufficient gold");
        let gold = app.world().resource::<Gold>();
        assert_eq!(gold.0, 50, "Gold unchanged when revive fails");
    }

    #[test]
    fn cure_stone_removes_status_and_deducts_gold() {
        let mut app = make_temple_test_app();
        app.world_mut().resource_mut::<Gold>().0 = 500;
        // Switch to Cure mode.
        app.world_mut().resource_mut::<TempleState>().mode = TempleMode::Cure;

        let member = spawn_party_member(
            &mut app,
            20,
            10,
            1,
            vec![make_status_effect(StatusEffectType::Stone)],
        );

        press_confirm(&mut app);

        let status = app.world().get::<StatusEffects>(member).unwrap();
        assert!(!status.has(StatusEffectType::Stone), "Stone should be removed");
        let gold = app.world().resource::<Gold>();
        assert_eq!(gold.0, 250, "Gold should be 500 - 250 (Stone cost)");
    }

    #[test]
    fn cure_mode_does_not_remove_dead() {
        let mut app = make_temple_test_app();
        app.world_mut().resource_mut::<Gold>().0 = 1000;
        app.world_mut().resource_mut::<TempleState>().mode = TempleMode::Cure;

        let member = spawn_party_member(&mut app, 20, 10, 1, vec![make_dead_effect()]);
        app.world_mut()
            .get_mut::<DerivedStats>(member)
            .unwrap()
            .current_hp = 0;

        press_confirm(&mut app);

        let status = app.world().get::<StatusEffects>(member).unwrap();
        assert!(status.has(StatusEffectType::Dead), "Dead must remain in Cure mode");
        let gold = app.world().resource::<Gold>();
        assert_eq!(gold.0, 1000, "Gold must not change when Cure has nothing to do");
    }

    #[test]
    fn cure_rejects_when_insufficient_gold() {
        let mut app = make_temple_test_app();
        app.world_mut().resource_mut::<Gold>().0 = 100; // Stone costs 250
        app.world_mut().resource_mut::<TempleState>().mode = TempleMode::Cure;

        let member = spawn_party_member(
            &mut app,
            20,
            10,
            1,
            vec![make_status_effect(StatusEffectType::Stone)],
        );

        press_confirm(&mut app);

        let status = app.world().get::<StatusEffects>(member).unwrap();
        assert!(status.has(StatusEffectType::Stone), "Stone should remain when gold insufficient");
        let gold = app.world().resource::<Gold>();
        assert_eq!(gold.0, 100, "Gold must not change on failed cure");
    }

    #[test]
    fn cancel_returns_to_square() {
        let mut app = make_temple_test_app();
        press_cancel(&mut app);
        assert_eq!(
            *app.world().resource::<State<TownLocation>>().get(),
            TownLocation::Square
        );
    }
}
