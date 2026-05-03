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

use crate::data::{ClassTable, DungeonFloor, EnemyDb, ItemDb, SpellTable};
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
            .add_systems(OnExit(GameState::Loading), despawn_loading_screen);
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
