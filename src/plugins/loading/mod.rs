//! Asset pipeline + loading-screen lifecycle.
//!
//! Owns the `GameState::Loading -> GameState::TitleScreen` transition
//! once `DungeonAssets` reports all handles `LoadedWithDependencies`.
//! No other plugin should set `NextState<GameState>::TitleScreen` —
//! see `project_druum_state_machine.md` for the contract.
//!
//! Feature #3: stub asset types only. Feature #4 fills `DungeonFloor`,
//! Feature #25 replaces the placeholder loading-screen UI with a real
//! title screen.

use bevy::prelude::*;
use bevy_asset_loader::prelude::*;
use bevy_common_assets::ron::RonAssetPlugin;

use crate::data::{ClassTable, DungeonFloor, EncounterTable, EnemyDb, ItemDb, SpellTable};
use crate::plugins::dungeon::features::{PendingTeleport, TeleportRequested};
use crate::plugins::state::GameState;

/// Resource populated by `bevy_asset_loader` once all collection handles
/// report `LoadedWithDependencies`. Both derives required:
/// `AssetCollection` for the loading-state machinery, `Resource` so
/// `bevy_asset_loader` can `commands.insert_resource(populated)`.
/// Forgetting `Resource` produces an opaque trait error at the
/// `load_collection::<DungeonAssets>()` call site (research §Pitfall 4).
///
/// Feature #3 deliberately omits a `Font` field — Bevy's embedded
/// `default_font` (transitively via `features = ["3d"]` -> `default_platform`)
/// renders the "Loading..." text. Feature #25 owns real font loading.
#[derive(AssetCollection, Resource)]
pub struct DungeonAssets {
    #[asset(path = "dungeons/floor_01.dungeon.ron")]
    pub floor_01: Handle<DungeonFloor>,
    // Feature #13 — minimal floor for cross-floor teleport testing (D11-A):
    #[asset(path = "dungeons/floor_02.dungeon.ron")]
    pub floor_02: Handle<DungeonFloor>,
    // Feature #16 — encounter table for floor 1.
    #[asset(path = "encounters/floor_01.encounters.ron")]
    pub encounters_floor_01: Handle<EncounterTable>,
    #[asset(path = "items/core.items.ron")]
    pub item_db: Handle<ItemDb>,
    #[asset(path = "enemies/core.enemies.ron")]
    pub enemy_db: Handle<EnemyDb>,
    #[asset(path = "classes/core.classes.ron")]
    pub class_table: Handle<ClassTable>,
    #[asset(path = "spells/core.spells.ron")]
    pub spell_table: Handle<SpellTable>,
}

/// Audio asset handles populated by `bevy_asset_loader` once all .ogg files
/// finish loading. Kept separate from `DungeonAssets` so a missing audio
/// file does not block dungeon-data tests (research §RQ5).
///
/// Both derives are required: `AssetCollection` for the loading-state
/// machinery, `Resource` so `bevy_asset_loader` can `commands.insert_resource`
/// the populated value. Same trap as `DungeonAssets`.
#[derive(AssetCollection, Resource)]
pub struct AudioAssets {
    // BGM tracks — one per GameState that has music. GameState::Loading has
    // no entry; play_bgm_for_state returns early on Loading (no music while
    // assets resolve).
    #[asset(path = "audio/bgm/town.ogg")]
    pub bgm_town: Handle<AudioSource>,
    #[asset(path = "audio/bgm/dungeon.ogg")]
    pub bgm_dungeon: Handle<AudioSource>,
    #[asset(path = "audio/bgm/combat.ogg")]
    pub bgm_combat: Handle<AudioSource>,
    #[asset(path = "audio/bgm/title.ogg")]
    pub bgm_title: Handle<AudioSource>,
    #[asset(path = "audio/bgm/gameover.ogg")]
    pub bgm_gameover: Handle<AudioSource>,
    // SFX — one per SfxKind variant in src/plugins/audio/sfx.rs.
    #[asset(path = "audio/sfx/footstep.ogg")]
    pub sfx_footstep: Handle<AudioSource>,
    #[asset(path = "audio/sfx/door.ogg")]
    pub sfx_door: Handle<AudioSource>,
    #[asset(path = "audio/sfx/encounter_sting.ogg")]
    pub sfx_encounter_sting: Handle<AudioSource>,
    #[asset(path = "audio/sfx/menu_click.ogg")]
    pub sfx_menu_click: Handle<AudioSource>,
    #[asset(path = "audio/sfx/attack_hit.ogg")]
    pub sfx_attack_hit: Handle<AudioSource>,
    // Feature #13 additions:
    #[asset(path = "audio/sfx/spinner_whoosh.ogg")]
    pub sfx_spinner_whoosh: Handle<AudioSource>,
    #[asset(path = "audio/sfx/door_close.ogg")]
    pub sfx_door_close: Handle<AudioSource>,
}

/// Marker tag on every entity spawned by `spawn_loading_screen`.
/// `despawn_loading_screen` queries this to clean up on `OnExit`.
#[derive(Component)]
struct LoadingScreenRoot;

pub struct LoadingPlugin;

impl Plugin for LoadingPlugin {
    fn build(&self, app: &mut App) {
        app
            // (1) Register one typed RON loader per asset extension.
            //     Order matters: these MUST be registered before
            //     add_loading_state, because LoadingState begins polling
            //     handles on OnEnter(GameState::Loading) — by which point
            //     the typed loader registry must already know how to
            //     dispatch ".dungeon.ron" -> DungeonFloor, etc.
            //     Use the FULL multi-dot extension WITHOUT a leading dot
            //     (research §Question 2).
            .add_plugins((
                RonAssetPlugin::<DungeonFloor>::new(&["dungeon.ron"]),
                RonAssetPlugin::<ItemDb>::new(&["items.ron"]),
                RonAssetPlugin::<EnemyDb>::new(&["enemies.ron"]),
                RonAssetPlugin::<ClassTable>::new(&["classes.ron"]),
                RonAssetPlugin::<SpellTable>::new(&["spells.ron"]),
                RonAssetPlugin::<EncounterTable>::new(&["encounters.ron"]), // Feature #16
            ))
            // (2) Drive GameState::Loading -> TitleScreen once all
            //     handles in DungeonAssets report LoadedWithDependencies.
            //     bevy_asset_loader handles the next.set(...) internally.
            //     Do NOT add a parallel next.set anywhere.
            .add_loading_state(
                LoadingState::new(GameState::Loading)
                    .continue_to_state(GameState::TitleScreen)
                    .load_collection::<DungeonAssets>()
                    .load_collection::<AudioAssets>(), // Feature #6 — sibling of DungeonAssets
            )
            // (3) Loading-screen UI lifecycle. Camera2d + centered text
            //     are spawned on OnEnter(Loading) and despawned on
            //     OnExit(Loading) — both tagged LoadingScreenRoot.
            .add_systems(OnEnter(GameState::Loading), spawn_loading_screen)
            .add_systems(OnExit(GameState::Loading), despawn_loading_screen)
            // Feature #13 cross-floor teleport (D3-α):
            .add_systems(
                Update,
                handle_teleport_request.run_if(in_state(GameState::Dungeon)),
            )
            // Feature #13 cross-floor teleport (D3-α): bevy_asset_loader's
            // `continue_to_state(TitleScreen)` is fired on every Loading
            // completion. When the player teleports cross-floor, we re-enter
            // Loading with `PendingTeleport.target = Some(_)`. Without this
            // redirect, the player lands on TitleScreen instead of Dungeon.
            .add_systems(
                OnEnter(GameState::TitleScreen),
                redirect_to_dungeon_if_pending,
            );
    }
}

/// Consumes `TeleportRequested` and triggers a re-entry into
/// `GameState::Loading -> GameState::Dungeon` with the destination stashed
/// in `PendingTeleport`. The next `OnEnter(Dungeon)` reads the destination
/// and overrides `floor.entry_point`.
///
/// Runs in `Update` while in `GameState::Dungeon`. Reading `requests.read().last()`
/// collapses multiple same-frame requests to the most recent (e.g., walking
/// into a chain of teleporters in one tick — last writer wins).
fn handle_teleport_request(
    mut requests: MessageReader<TeleportRequested>,
    mut pending: ResMut<PendingTeleport>,
    mut next: ResMut<NextState<GameState>>,
) {
    if let Some(req) = requests.read().last() {
        pending.target = Some(req.target.clone());
        next.set(GameState::Loading);
        info!(
            "Teleport requested to floor {} at ({}, {})",
            req.target.floor, req.target.x, req.target.y
        );
    }
}

/// Redirect `Loading -> TitleScreen -> Dungeon` when `PendingTeleport` is set.
/// `bevy_asset_loader::continue_to_state(TitleScreen)` is configured statically,
/// so every Loading completion lands on TitleScreen. This system runs on
/// `OnEnter(TitleScreen)` and queues a transition back to Dungeon if a teleport
/// is in flight. Briefly traverses TitleScreen for a single frame; the title-screen
/// UI render is sub-frame and not user-visible at typical frame rates.
fn redirect_to_dungeon_if_pending(
    pending: Res<PendingTeleport>,
    mut next: ResMut<NextState<GameState>>,
) {
    if pending.target.is_some() {
        next.set(GameState::Dungeon);
        info!("Loading complete with PendingTeleport set; redirecting to Dungeon");
    }
}

/// Spawn the placeholder loading-screen UI: a `Camera2d` (required for
/// `bevy_ui` to render — research §Pitfall 5) plus a centered "Loading..."
/// `Text` node. Bevy 0.18's `#[require(...)]` attribute on `Text`
/// auto-attaches the supporting components (TextLayout, TextFont, etc.).
fn spawn_loading_screen(mut commands: Commands) {
    // Camera tagged with the same marker so we despawn it on OnExit.
    commands.spawn((Camera2d, LoadingScreenRoot));

    // Full-screen flex container with the text centered horizontally
    // and vertically.
    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            LoadingScreenRoot,
        ))
        .with_children(|parent| {
            // No font handle: Bevy 0.18 falls back to the embedded
            // default_font (enabled transitively via features = ["3d"]).
            parent.spawn(Text::new("Loading..."));
        });
}

/// Despawn every entity tagged `LoadingScreenRoot`. Bevy 0.18's
/// `commands.entity(e).despawn()` is recursive by default (so the child
/// `Text` node is cleaned up automatically when its parent `Node` is
/// despawned). The `Camera2d` is also despawned — the next feature that
/// needs a camera will spawn its own.
fn despawn_loading_screen(mut commands: Commands, roots: Query<Entity, With<LoadingScreenRoot>>) {
    for e in &roots {
        commands.entity(e).despawn();
    }
}

/// Returns the `EncounterTable` handle for `floor_number` from `DungeonAssets`.
/// Falls back to `floor_01` for unknown floor numbers and emits a warning.
/// Mirrors `dungeon::floor_handle_for` precedent. Future floors add match arms.
pub(crate) fn encounter_table_for(
    assets: &DungeonAssets,
    floor_number: u32,
) -> &Handle<EncounterTable> {
    match floor_number {
        1 => &assets.encounters_floor_01,
        n => {
            warn!("No EncounterTable handle for floor {n}; falling back to floor_01");
            &assets.encounters_floor_01
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::dungeon::TeleportTarget;
    use bevy::state::app::StatesPlugin;

    fn make_redirect_app() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, StatesPlugin));
        app.init_state::<GameState>();
        app.init_resource::<PendingTeleport>();
        app.add_systems(
            OnEnter(GameState::TitleScreen),
            redirect_to_dungeon_if_pending,
        );
        app
    }

    /// Pending teleport set + transition into TitleScreen → redirect to Dungeon.
    /// This is the regression test for Defect B (review): without the redirect,
    /// `bevy_asset_loader::continue_to_state(TitleScreen)` strands the player
    /// on the title screen after a cross-floor teleport.
    #[test]
    fn redirect_fires_when_pending_teleport_set() {
        let mut app = make_redirect_app();

        app.world_mut().resource_mut::<PendingTeleport>().target = Some(TeleportTarget {
            floor: 2,
            x: 1,
            y: 1,
            facing: None,
        });

        // Loading → TitleScreen (simulates bevy_asset_loader's transition).
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::TitleScreen);
        app.update(); // realize TitleScreen + run OnEnter(TitleScreen) systems
        app.update(); // realize the queued Dungeon transition

        assert_eq!(
            *app.world().resource::<State<GameState>>().get(),
            GameState::Dungeon,
            "redirect_to_dungeon_if_pending should re-route TitleScreen → Dungeon"
        );
    }

    /// No pending teleport → stay on TitleScreen (cold-boot path is unaffected).
    #[test]
    fn redirect_no_op_when_pending_teleport_unset() {
        let mut app = make_redirect_app();

        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::TitleScreen);
        app.update();
        app.update();

        assert_eq!(
            *app.world().resource::<State<GameState>>().get(),
            GameState::TitleScreen,
            "without PendingTeleport, the redirect must be a no-op"
        );
    }
}
