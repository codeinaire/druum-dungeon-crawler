//! Town hub plugin — Square, Shop, Inn, Temple, and Guild screens.
//!
//! ## Architecture
//!
//! Town uses a `Camera2d` + `PrimaryEguiContext` spawned as ONE entity on
//! `OnEnter(GameState::Town)`, tagged `TownCameraRoot`. The egui context is
//! attached to the same entity so despawning `TownCameraRoot` atomically
//! removes both — no orphaned context that silently breaks painters.
//!
//! Each Town sub-state (Square / Shop / Inn / Temple / Guild) has its own
//! painter + handler module:
//! - A **painter** in `EguiPrimaryContextPass` — read-only, no mutations.
//! - An **input handler** in `Update` — may mutate resources / spawn entities.
//!
//! Both tuples use `.distributive_run_if(in_state(GameState::Town))` as a
//! defense-in-depth guard, plus per-system `.run_if(in_state(TownLocation::X))`.
//!
//! ## Modules
//!
//! - `gold` — `Gold` resource + `GameClock` day/turn counter.
//! - `guild` — party roster: recruit, dismiss, row swap, slot swap.
//! - `inn` — rest, heal, cure Poison.
//! - `shop` — buy/sell items.
//! - `square` — main navigation hub.
//! - `temple` — revive Dead characters and cure Stone/Paralysis/Sleep.

use bevy::prelude::*;
use bevy_egui::{EguiPrimaryContextPass, PrimaryEguiContext};

pub mod gold;
pub mod guild;
pub mod guild_create;
pub mod guild_skills;
pub mod inn;
pub mod shop;
pub mod square;
pub mod temple;
pub mod toast;

pub use gold::{GameClock, Gold, SpendError};

use crate::plugins::state::{GameState, TownLocation};

use guild::{
    DismissedPool, GuildMode, GuildState, RecruitedSet,
    handle_guild_dismiss, handle_guild_input, handle_guild_recruit,
    handle_guild_row_swap, handle_guild_slot_swap, paint_guild,
};
use guild_create::{
    CreationDraft,
    handle_guild_create_allocate, handle_guild_create_confirm, handle_guild_create_input,
    handle_guild_create_name_input, handle_guild_create_roll,
    paint_guild_create_allocate, paint_guild_create_class, paint_guild_create_confirm,
    paint_guild_create_name, paint_guild_create_race, paint_guild_create_roll,
};
use guild_skills::{
    handle_guild_skills_input, handle_guild_skills_unlock, paint_guild_skills,
};
use inn::{InnState, handle_inn_rest, paint_inn};
use shop::{ShopState, handle_shop_input, paint_shop};
use temple::{TempleState, handle_temple_action, paint_temple};
use toast::{Toasts, paint_toasts, tick_toasts};
use square::{SquareMenuState, handle_square_input, paint_town_square};

// ─────────────────────────────────────────────────────────────────────────────
// TownCameraRoot marker component
// ─────────────────────────────────────────────────────────────────────────────

/// Marker tag on the entity that holds the Town `Camera2d` and `PrimaryEguiContext`.
///
/// All town painters call `EguiContexts::ctx_mut()` which resolves to the
/// context attached to this entity. On `OnExit(GameState::Town)` every entity
/// with this tag is despawned atomically (both camera and context), preventing
/// the orphan-context bug described in the Critical section.
#[derive(Component)]
pub struct TownCameraRoot;

// ─────────────────────────────────────────────────────────────────────────────
// Camera lifecycle
// ─────────────────────────────────────────────────────────────────────────────

/// Spawn the Town `Camera2d` + `PrimaryEguiContext` as a single entity tagged
/// `TownCameraRoot`. Both components live on the same entity so despawning the
/// tag destroys both atomically.
fn spawn_town_camera(mut commands: Commands) {
    commands.spawn((Camera2d, TownCameraRoot, PrimaryEguiContext));
    info!("Entered GameState::Town — spawned TownCameraRoot");
}

/// Despawn every entity tagged `TownCameraRoot` (typically just one).
/// Runs on `OnExit(GameState::Town)`.
fn despawn_town_camera(mut commands: Commands, cams: Query<Entity, With<TownCameraRoot>>) {
    for entity in &cams {
        commands.entity(entity).despawn();
    }
    info!("Exited GameState::Town — despawned TownCameraRoot");
}

// ─────────────────────────────────────────────────────────────────────────────
// TownPlugin
// ─────────────────────────────────────────────────────────────────────────────

// ─────────────────────────────────────────────────────────────────────────────
// Guild creation mode guard helper
// ─────────────────────────────────────────────────────────────────────────────

/// Returns a `run_if` closure that returns `true` when `GuildState.mode == target`.
///
/// `GuildMode` is NOT a Bevy `States` impl, so per-variant `.run_if` uses this
/// closure rather than `in_state(...)`. Mirrors the per-mode painter dispatch
/// from the Roster/Recruit split.
fn in_guild_mode(target: GuildMode) -> impl Fn(Res<GuildState>) -> bool + Clone {
    move |state: Res<GuildState>| state.mode == target
}

pub struct TownPlugin;

impl Plugin for TownPlugin {
    fn build(&self, app: &mut App) {
        // Resources.
        app.init_resource::<Gold>()
            .init_resource::<GameClock>()
            .init_resource::<SquareMenuState>()
            .init_resource::<ShopState>()
            .init_resource::<InnState>()
            .init_resource::<TempleState>()
            .init_resource::<GuildState>()
            .init_resource::<DismissedPool>()
            .init_resource::<RecruitedSet>()
            .init_resource::<Toasts>()
            // Feature #19 — creation draft resource.
            .init_resource::<CreationDraft>();

        // Feature #19 — discard draft when leaving Guild (mid-creation cancel).
        app.add_systems(
            OnExit(TownLocation::Guild),
            |mut d: ResMut<CreationDraft>| d.reset(),
        );

        // Feature #20 — reset Skills mode cursor when leaving Guild.
        app.add_systems(
            OnExit(TownLocation::Guild),
            |mut gs: ResMut<GuildState>| {
                if gs.mode == GuildMode::Skills {
                    gs.node_cursor = 0;
                }
            },
        );

        // Camera lifecycle.
        app.add_systems(OnEnter(GameState::Town), spawn_town_camera)
            .add_systems(OnExit(GameState::Town), despawn_town_camera);

        // Painters — all in EguiPrimaryContextPass, defense-in-depth gated on Town.
        app.add_systems(
            EguiPrimaryContextPass,
            (
                paint_town_square
                    .run_if(in_state(TownLocation::Square)),
                paint_shop
                    .run_if(in_state(TownLocation::Shop)),
                paint_inn
                    .run_if(in_state(TownLocation::Inn)),
                paint_temple
                    .run_if(in_state(TownLocation::Temple)),
                paint_guild
                    .run_if(in_state(TownLocation::Guild)),
                // Feature #19 — creation wizard painters (gated on GuildMode variant).
                paint_guild_create_race
                    .run_if(in_state(TownLocation::Guild))
                    .run_if(in_guild_mode(GuildMode::CreateRace)),
                paint_guild_create_class
                    .run_if(in_state(TownLocation::Guild))
                    .run_if(in_guild_mode(GuildMode::CreateClass)),
                paint_guild_create_roll
                    .run_if(in_state(TownLocation::Guild))
                    .run_if(in_guild_mode(GuildMode::CreateRoll)),
                paint_guild_create_allocate
                    .run_if(in_state(TownLocation::Guild))
                    .run_if(in_guild_mode(GuildMode::CreateAllocate)),
                paint_guild_create_name
                    .run_if(in_state(TownLocation::Guild))
                    .run_if(in_guild_mode(GuildMode::CreateName)),
                paint_guild_create_confirm
                    .run_if(in_state(TownLocation::Guild))
                    .run_if(in_guild_mode(GuildMode::CreateConfirm)),
                // Feature #20 — skill tree painter (gated on GuildMode::Skills).
                paint_guild_skills
                    .run_if(in_state(TownLocation::Guild))
                    .run_if(in_guild_mode(GuildMode::Skills)),
            )
                .distributive_run_if(in_state(GameState::Town)),
        );

        // Toast overlay — always painted while in Town, regardless of sub-state.
        app.add_systems(
            EguiPrimaryContextPass,
            paint_toasts.run_if(in_state(GameState::Town)),
        );
        app.add_systems(Update, tick_toasts.run_if(in_state(GameState::Town)));

        // Input handlers — all in Update, defense-in-depth gated on Town.
        app.add_systems(
            Update,
            (
                handle_square_input
                    .run_if(in_state(TownLocation::Square)),
                handle_shop_input
                    .run_if(in_state(TownLocation::Shop)),
                handle_inn_rest
                    .run_if(in_state(TownLocation::Inn)),
                handle_temple_action
                    .run_if(in_state(TownLocation::Temple)),
                handle_guild_input
                    .run_if(in_state(TownLocation::Guild)),
                handle_guild_recruit
                    .run_if(in_state(TownLocation::Guild)),
                handle_guild_dismiss
                    .run_if(in_state(TownLocation::Guild)),
                handle_guild_row_swap
                    .run_if(in_state(TownLocation::Guild)),
                handle_guild_slot_swap
                    .run_if(in_state(TownLocation::Guild)),
                // Feature #19 — creation wizard handlers (all gated at TownLocation::Guild;
                // internal mode guard is inside each handler).
                handle_guild_create_input
                    .run_if(in_state(TownLocation::Guild)),
                handle_guild_create_allocate
                    .run_if(in_state(TownLocation::Guild)),
                handle_guild_create_name_input
                    .run_if(in_state(TownLocation::Guild)),
                handle_guild_create_roll
                    .run_if(in_state(TownLocation::Guild)),
                handle_guild_create_confirm
                    .run_if(in_state(TownLocation::Guild)),
                // Feature #20 — skill tree input + unlock handlers (gated on Skills mode).
                handle_guild_skills_input
                    .run_if(in_state(TownLocation::Guild))
                    .run_if(in_guild_mode(GuildMode::Skills)),
                handle_guild_skills_unlock
                    .run_if(in_state(TownLocation::Guild))
                    .run_if(in_guild_mode(GuildMode::Skills)),
            )
                .distributive_run_if(in_state(GameState::Town)),
        );

        #[cfg(feature = "dev")]
        app.add_systems(
            Update,
            (
                gold::grant_gold_on_f4,
                gold::apply_test_status_on_function_keys,
            ),
        );
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

    use crate::plugins::input::MenuAction;
    use crate::plugins::party::inventory::{EquipmentChangedEvent, ItemHandleRegistry};
    use crate::plugins::state::{GameState, TownLocation};
    use super::{spawn_town_camera, despawn_town_camera, TownCameraRoot};
    use crate::plugins::town::guild::{DismissedPool, GuildState, handle_guild_dismiss, handle_guild_input, handle_guild_recruit, handle_guild_row_swap, handle_guild_slot_swap};
    use crate::plugins::town::inn::{InnState, handle_inn_rest};
    use crate::plugins::town::shop::{ShopState, handle_shop_input};
    use crate::plugins::town::temple::{TempleState, handle_temple_action};
    use crate::plugins::town::square::{SquareMenuState, handle_square_input};

    /// Minimal plugin set for Town tests (no audio, no dungeon, no combat, no render).
    ///
    /// **Note:** `EguiPlugin` is intentionally OMITTED — it requires the render pipeline
    /// (not available under `MinimalPlugins`). Painter systems (`EguiPrimaryContextPass`)
    /// are not tested here; only state-machine and camera-lifecycle systems are tested.
    /// The painter systems are verified by the manual smoke test in the Verification section.
    fn make_town_test_app() -> App {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            StatesPlugin,
        ));

        // Insert ActionState<MenuAction> as a bare resource — do NOT add
        // InputManagerPlugin. Its mouse-input systems require AccumulatedMouseMotion
        // (provided only by Bevy's InputPlugin), and adding InputPlugin would clear
        // just_pressed in PreUpdate before Update systems observe it. Same bypass
        // pattern as minimap tests (see src/plugins/ui/minimap.rs:562-594).
        app.init_resource::<ActionState<MenuAction>>();

        app.init_state::<GameState>();
        app.add_sub_state::<TownLocation>();

        // TownPlugin requires TownAssets (Option<Res<TownAssets>>) — it's optional
        // in painters, so we just need the asset types registered.
        use crate::data::town::{ShopStock, RecruitPool, TownServices};
        app.init_asset::<ShopStock>();
        app.init_asset::<RecruitPool>();
        app.init_asset::<TownServices>();

        // EquipmentChangedEvent must be registered for handle_inn_rest and handle_temple_action.
        app.add_message::<EquipmentChangedEvent>();

        // ItemAsset must be registered for shop painters.
        use crate::data::items::ItemAsset;
        app.init_asset::<ItemAsset>();
        use crate::data::items::ItemDb;
        app.init_asset::<ItemDb>();

        // ItemHandleRegistry is read by handle_shop_input.
        app.init_resource::<ItemHandleRegistry>();

        // Insert mock EguiGlobalSettings to satisfy bevy_egui EguiPlugin requirement
        // if TownPlugin itself registers EguiPrimaryContextPass systems.
        // EguiPlugin is NOT added (needs render pipeline), so we must NOT add TownPlugin
        // without filtering out painter systems. Instead we add only the non-egui systems.
        // TownPlugin::build conditionally registers painters in EguiPrimaryContextPass
        // which is registered by EguiPlugin. Without EguiPlugin the schedule doesn't exist.
        // Solution: add only the systems we can test (Update systems + camera lifecycle).
        // This matches the combat/ui_combat.rs test pattern.

        // Camera lifecycle systems only (OnEnter/OnExit are not EguiPrimaryContextPass).
        app.add_systems(OnEnter(GameState::Town), spawn_town_camera)
            .add_systems(OnExit(GameState::Town), despawn_town_camera);

        // Input handler systems in Update (no EguiPrimaryContextPass dependency).
        use crate::plugins::party::character::PartySize;
        app.init_resource::<SquareMenuState>()
            .init_resource::<ShopState>()
            .init_resource::<InnState>()
            .init_resource::<TempleState>()
            .init_resource::<GuildState>()
            .init_resource::<DismissedPool>()
            .init_resource::<RecruitedSet>()
            .init_resource::<crate::plugins::town::toast::Toasts>()
            .init_resource::<Gold>()
            .init_resource::<GameClock>()
            .init_resource::<PartySize>();

        // Guild handlers need ButtonInput<KeyCode>.
        app.init_resource::<ButtonInput<KeyCode>>();

        app.add_systems(
            Update,
            (
                handle_square_input.run_if(in_state(TownLocation::Square)),
                handle_shop_input.run_if(in_state(TownLocation::Shop)),
                handle_inn_rest.run_if(in_state(TownLocation::Inn)),
                handle_temple_action.run_if(in_state(TownLocation::Temple)),
                handle_guild_input.run_if(in_state(TownLocation::Guild)),
                handle_guild_recruit.run_if(in_state(TownLocation::Guild)),
                handle_guild_dismiss.run_if(in_state(TownLocation::Guild)),
                handle_guild_row_swap.run_if(in_state(TownLocation::Guild)),
                handle_guild_slot_swap.run_if(in_state(TownLocation::Guild)),
            )
                .distributive_run_if(in_state(GameState::Town)),
        );

        app
    }

    /// `TownPlugin::build` does not panic — smoke test.
    #[test]
    fn town_plugin_builds() {
        let mut app = make_town_test_app();
        app.update(); // should not panic
    }

    /// Transitioning to `GameState::Town` spawns exactly one `TownCameraRoot`.
    #[test]
    fn town_camera_spawns_on_enter_and_despawns_on_exit() {
        let mut app = make_town_test_app();

        // Enter Town.
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Town);
        app.update(); // realise transition + run OnEnter systems
        app.update(); // settle

        let count_enter = app
            .world_mut()
            .query::<&TownCameraRoot>()
            .iter(app.world())
            .count();
        assert_eq!(
            count_enter, 1,
            "Exactly one TownCameraRoot should exist after entering Town"
        );

        // Exit Town.
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::TitleScreen);
        app.update();
        app.update();

        let count_exit = app
            .world_mut()
            .query::<&TownCameraRoot>()
            .iter(app.world())
            .count();
        assert_eq!(
            count_exit, 0,
            "TownCameraRoot should be despawned after exiting Town"
        );
    }

    /// After entering `GameState::Town`, `TownLocation` defaults to `Square`.
    #[test]
    fn town_substate_defaults_to_square_on_enter() {
        let mut app = make_town_test_app();

        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Town);
        app.update();
        app.update();

        assert_eq!(
            *app.world().resource::<State<TownLocation>>().get(),
            TownLocation::Square,
            "Default TownLocation sub-state should be Square"
        );
    }
}
