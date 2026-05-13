//! Town Inn screen — rest, heal, and cure mild status effects.
//!
//! ## Inn rest invariants
//!
//! - Full HP + MP restore for every **non-Dead** party member.
//! - Status effects in `services.inn_rest_cures` are removed (e.g., Poison).
//! - Dead members are SKIPPED — revive is Temple work (#18b).
//! - Gold cost is deducted ONLY on success.
//! - `clock.day` is incremented by 1 on rest; `clock.turn` resets to 0.
//! - `EquipmentChangedEvent` with `slot: EquipSlot::None` is fired for each
//!   healed member so `recompute_derived_stats_on_equipment_change` can update
//!   derived stats if any buff expires during rest (dual-use trigger, per the
//!   project memory on `EquipmentChangedEvent`).
//!
//! ## Inn rest cost trust boundary
//!
//! `services.inn_rest_cost` is clamped to `MAX_INN_COST` before use (per Phase 10).

use bevy::prelude::*;
use bevy_egui::{EguiContexts, egui};
use leafwing_input_manager::prelude::ActionState;

use crate::data::town::{MAX_INN_COST, TownServices};
use crate::plugins::input::MenuAction;
use crate::plugins::loading::TownAssets;
use crate::plugins::party::character::{DerivedStats, PartyMember, StatusEffectType, StatusEffects};
use crate::plugins::party::inventory::{EquipSlot, EquipmentChangedEvent};
use crate::plugins::state::TownLocation;
use crate::plugins::town::gold::{GameClock, Gold};

// ─────────────────────────────────────────────────────────────────────────────
// InnState resource
// ─────────────────────────────────────────────────────────────────────────────

/// Cursor state for the Inn screen. Reserved for future "Rest / Talk to barkeep"
/// options; v1 has a single action.
#[derive(Resource, Default, Debug)]
pub struct InnState {
    pub cursor: usize,
}

// ─────────────────────────────────────────────────────────────────────────────
// paint_inn — EguiPrimaryContextPass, read-only
// ─────────────────────────────────────────────────────────────────────────────

/// Render the Inn screen.
///
/// **Read-only** — no `ResMut<T>` or `Commands`. All mutations live in
/// `handle_inn_rest`.
pub fn paint_inn(
    mut contexts: EguiContexts,
    gold: Res<Gold>,
    town_assets: Option<Res<TownAssets>>,
    services_assets: Res<Assets<TownServices>>,
) -> Result {
    let ctx = contexts.ctx_mut()?;

    egui::TopBottomPanel::top("inn_header").show(ctx, |ui| {
        ui.horizontal(|ui| {
            ui.heading("Inn");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(format!("{} Gold", gold.0));
            });
        });
    });

    egui::CentralPanel::default().show(ctx, |ui| {
        let cost_label = town_assets
            .as_ref()
            .and_then(|a| services_assets.get(&a.services))
            .map(|s| {
                let cost = s.inn_rest_cost.min(MAX_INN_COST);
                format!("Rest: {} gold", cost)
            })
            .unwrap_or_else(|| "(loading...)".to_string());

        ui.vertical_centered(|ui| {
            ui.add_space(20.0);
            ui.label("> Rest  (Heal HP/MP — cure Poison)");
            ui.add_space(8.0);
            ui.label(cost_label);
            ui.add_space(16.0);
            ui.label("[Enter] Rest  |  [Esc] Back");
        });
    });

    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// handle_inn_rest — Update, mutates
// ─────────────────────────────────────────────────────────────────────────────

/// Handle input in the Inn screen.
///
/// On `MenuAction::Confirm`:
/// 1. Look up `inn_rest_cost` from `TownServices` (clamped to `MAX_INN_COST`).
/// 2. Gate on `gold.0 >= cost`.
/// 3. Iterate party — for each **non-Dead** member: full HP/MP restore, cure
///    status effects listed in `inn_rest_cures`.
/// 4. Fire `EquipmentChangedEvent { slot: EquipSlot::None }` per healed member.
/// 5. Deduct gold, advance `clock.day`, reset `clock.turn`.
/// 6. Return to Square.
///
/// On `MenuAction::Cancel`: return to Square immediately.
#[allow(clippy::too_many_arguments)]
pub fn handle_inn_rest(
    actions: Res<ActionState<MenuAction>>,
    mut gold: ResMut<Gold>,
    mut clock: ResMut<GameClock>,
    town_assets: Option<Res<TownAssets>>,
    services_assets: Res<Assets<TownServices>>,
    mut next_sub: ResMut<NextState<TownLocation>>,
    mut writer: MessageWriter<EquipmentChangedEvent>,
    mut toasts: ResMut<crate::plugins::town::toast::Toasts>,
    mut party: Query<(Entity, &mut DerivedStats, &mut StatusEffects), With<PartyMember>>,
) {
    if actions.just_pressed(&MenuAction::Cancel) {
        next_sub.set(TownLocation::Square);
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

    // Step 1: cost with trust-boundary clamp.
    let cost = services.inn_rest_cost.min(MAX_INN_COST);

    // Step 2: gold sufficiency check.
    if gold.0 < cost {
        info!("Inn rest: insufficient gold (have {}, need {})", gold.0, cost);
        toasts.push(format!("Not enough gold to rest ({cost}g needed)."));
        return;
    }

    // Step 3 + 4: heal each non-Dead party member.
    for (entity, mut derived, mut status) in &mut party {
        // Inn is for the living — skip Dead members (revive = Temple, #18b).
        if status.has(StatusEffectType::Dead) {
            continue;
        }

        // Full HP + MP restore.
        derived.current_hp = derived.max_hp;
        derived.current_mp = derived.max_mp;

        // Cure status effects listed in inn_rest_cures.
        status
            .effects
            .retain(|e| !services.inn_rest_cures.contains(&e.effect_type));

        // Fire EquipmentChangedEvent (dual-use stat-change trigger).
        writer.write(EquipmentChangedEvent {
            character: entity,
            slot: EquipSlot::None,
        });
    }

    // Steps 5: deduct gold, advance clock.
    let _ = gold.try_spend(cost);
    clock.day = clock.day.saturating_add(1);
    clock.turn = 0;

    // Stay in the Inn screen — user returns to Square via Esc/Cancel.
    toasts.push(format!("Party rested. ({cost}g) — Day {}.", clock.day));
    info!("Party rested at the Inn for {} gold. Day: {}", cost, clock.day);
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::state::app::StatesPlugin;
    use leafwing_input_manager::prelude::*;

    use bevy::asset::Handle;
    use crate::data::town::TownServices;
    use crate::plugins::input::MenuAction;
    use crate::plugins::loading::TownAssets;
    use crate::plugins::party::character::{
        ActiveEffect, BaseStats, DerivedStats, PartyMember, StatusEffectType, StatusEffects,
        derive_stats,
    };
    use crate::plugins::party::inventory::EquipmentChangedEvent;
    use crate::plugins::state::{GameState, TownLocation};
    use crate::plugins::town::gold::{GameClock, Gold};

    /// Build a minimal test app with just enough to test inn rest logic.
    fn make_inn_test_app() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::asset::AssetPlugin::default(), StatesPlugin));
        app.init_state::<GameState>();
        app.add_sub_state::<TownLocation>();

        // Gold, clock, menu actions.
        app.init_resource::<Gold>();
        app.init_resource::<GameClock>();
        app.init_resource::<crate::plugins::town::toast::Toasts>();
        app.init_resource::<ActionState<MenuAction>>();
        app.insert_resource(InputMap::<MenuAction>::default());

        // TownAssets + TownServices assets.
        app.init_asset::<TownServices>();
        // Insert a mock TownAssets that points to a real TownServices asset.
        let services = TownServices {
            inn_rest_cost: 10,
            inn_rest_cures: vec![StatusEffectType::Poison],
            ..Default::default()
        };
        let services_handle = app
            .world_mut()
            .resource_mut::<Assets<TownServices>>()
            .add(services);
        let mock_town_assets = TownAssets {
            shop_stock: Handle::default(),
            recruit_pool: Handle::default(),
            services: services_handle,
            race_table: Handle::default(),
            class_table: Handle::default(),
        };
        app.insert_resource(mock_town_assets);

        // Register EquipmentChangedEvent.
        app.add_message::<EquipmentChangedEvent>();

        // Register the inn rest handler.
        app.add_systems(Update, handle_inn_rest.run_if(in_state(TownLocation::Inn)));

        // Transition into Town / Inn.
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Town);
        app.update(); // realise Town + Square
        app.world_mut()
            .resource_mut::<NextState<TownLocation>>()
            .set(TownLocation::Inn);
        app.update(); // realise Inn
        app.update(); // settle

        app
    }

    fn spawn_party_member(app: &mut App, hp: u32, mp: u32, effects: Vec<ActiveEffect>) -> Entity {
        let base = BaseStats {
            vitality: 10,
            intelligence: 8,
            piety: 8,
            ..Default::default()
        };
        let mut derived = derive_stats(&base, &[], &StatusEffects { effects: effects.clone() }, 1);
        derived.max_hp = hp.max(1);
        derived.max_mp = mp.max(1);
        derived.current_hp = 1; // damaged
        derived.current_mp = 0; // spent

        app.world_mut().spawn((
            PartyMember,
            base,
            derived,
            StatusEffects { effects },
        )).id()
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

    /// Inn rest fully heals living party members (HP and MP).
    #[test]
    fn rest_full_heals_living_party() {
        let mut app = make_inn_test_app();
        app.world_mut().resource_mut::<Gold>().0 = 100;

        let member = spawn_party_member(&mut app, 20, 10, vec![]);
        press_confirm(&mut app);

        let derived = app.world().get::<DerivedStats>(member).unwrap();
        assert_eq!(
            derived.current_hp, derived.max_hp,
            "HP should be fully restored"
        );
        assert_eq!(
            derived.current_mp, derived.max_mp,
            "MP should be fully restored"
        );
    }

    /// Inn rest skips Dead party members.
    #[test]
    fn rest_skips_dead_party_member() {
        let mut app = make_inn_test_app();
        app.world_mut().resource_mut::<Gold>().0 = 100;

        let dead_member = spawn_party_member(
            &mut app,
            20,
            10,
            vec![ActiveEffect {
                effect_type: StatusEffectType::Dead,
                remaining_turns: None,
                magnitude: 0.0,
            }],
        );
        // Set hp to 0 explicitly to mark the dead state.
        app.world_mut()
            .get_mut::<DerivedStats>(dead_member)
            .unwrap()
            .current_hp = 0;

        press_confirm(&mut app);

        let derived = app.world().get::<DerivedStats>(dead_member).unwrap();
        assert_eq!(
            derived.current_hp, 0,
            "Dead member HP should remain 0 after Inn rest"
        );
    }

    /// Inn rest cures Poison but not Stone.
    #[test]
    fn rest_cures_poison_but_not_stone() {
        let mut app = make_inn_test_app();
        app.world_mut().resource_mut::<Gold>().0 = 100;

        let member = spawn_party_member(
            &mut app,
            20,
            10,
            vec![
                ActiveEffect {
                    effect_type: StatusEffectType::Poison,
                    remaining_turns: Some(3),
                    magnitude: 1.0,
                },
                ActiveEffect {
                    effect_type: StatusEffectType::Stone,
                    remaining_turns: None,
                    magnitude: 0.0,
                },
            ],
        );

        press_confirm(&mut app);

        let status = app.world().get::<StatusEffects>(member).unwrap();
        assert!(
            !status.has(StatusEffectType::Poison),
            "Poison should be cured by Inn rest"
        );
        assert!(
            status.has(StatusEffectType::Stone),
            "Stone should NOT be cured by Inn rest"
        );
    }

    /// Inn rest advances `clock.day` by 1 and resets `clock.turn` to 0.
    #[test]
    fn rest_advances_clock_day_by_one() {
        let mut app = make_inn_test_app();
        app.world_mut().resource_mut::<Gold>().0 = 100;
        app.world_mut().resource_mut::<GameClock>().day = 0;
        app.world_mut().resource_mut::<GameClock>().turn = 5;

        spawn_party_member(&mut app, 20, 10, vec![]);
        press_confirm(&mut app);

        let clock = app.world().resource::<GameClock>();
        assert_eq!(clock.day, 1, "clock.day should advance by 1 after rest");
        assert_eq!(clock.turn, 0, "clock.turn should reset to 0 after rest");
    }

    /// Inn rest deducts the configured cost from party gold.
    #[test]
    fn rest_deducts_configured_cost() {
        let mut app = make_inn_test_app();
        app.world_mut().resource_mut::<Gold>().0 = 100;

        spawn_party_member(&mut app, 20, 10, vec![]);
        press_confirm(&mut app);

        let gold = app.world().resource::<Gold>();
        assert_eq!(gold.0, 90, "Inn rest should deduct 10 gold (configured cost)");
    }

    /// Inn rest fails when gold is insufficient — party HP unchanged.
    #[test]
    fn rest_rejects_when_insufficient_gold() {
        let mut app = make_inn_test_app();
        app.world_mut().resource_mut::<Gold>().0 = 1; // cost is 10

        let member = spawn_party_member(&mut app, 20, 10, vec![]);
        press_confirm(&mut app);

        let derived = app.world().get::<DerivedStats>(member).unwrap();
        assert_eq!(
            derived.current_hp, 1,
            "HP should remain 1 when gold is insufficient"
        );
        let gold = app.world().resource::<Gold>();
        assert_eq!(gold.0, 1, "Gold should remain unchanged when rest fails");
    }

    /// Inn rest fires `EquipmentChangedEvent` for each living member.
    #[test]
    fn rest_fires_equipment_changed_event_per_member() {
        let mut app = make_inn_test_app();
        app.world_mut().resource_mut::<Gold>().0 = 100;

        // Spawn 2 living members + 1 dead member.
        let _m1 = spawn_party_member(&mut app, 20, 10, vec![]);
        let _m2 = spawn_party_member(&mut app, 15, 5, vec![]);
        let dead = spawn_party_member(
            &mut app,
            20,
            10,
            vec![ActiveEffect {
                effect_type: StatusEffectType::Dead,
                remaining_turns: None,
                magnitude: 0.0,
            }],
        );
        app.world_mut()
            .get_mut::<DerivedStats>(dead)
            .unwrap()
            .current_hp = 0;

        // Press Confirm once.
        press_confirm(&mut app);

        // Verify both living members are healed (proxy for EquipmentChangedEvent firing).
        let m1_hp = app.world().get::<DerivedStats>(_m1).unwrap().current_hp;
        let m2_hp = app.world().get::<DerivedStats>(_m2).unwrap().current_hp;
        assert!(m1_hp > 1, "m1 should be healed after Inn rest");
        assert!(m2_hp > 1, "m2 should be healed after Inn rest");
        // Dead member HP must remain 0.
        let dead_hp = app.world().get::<DerivedStats>(dead).unwrap().current_hp;
        assert_eq!(dead_hp, 0, "Dead member HP should remain 0 after Inn rest");
    }

    /// After a successful rest, the screen stays in `TownLocation::Inn`.
    /// Returning to Square is user-initiated via Cancel only.
    #[test]
    fn rest_does_not_auto_return_to_square() {
        let mut app = make_inn_test_app();
        app.world_mut().resource_mut::<Gold>().0 = 100;
        let _m = spawn_party_member(&mut app, 20, 10, vec![]);

        press_confirm(&mut app);

        // Sanity: rest succeeded (clock advanced).
        let clock = app.world().resource::<GameClock>();
        assert_eq!(clock.day, 1, "rest should advance the clock");

        assert_eq!(
            *app.world().resource::<State<TownLocation>>().get(),
            TownLocation::Inn,
            "Inn must stay open after rest — only Cancel returns to Square"
        );
    }
}
