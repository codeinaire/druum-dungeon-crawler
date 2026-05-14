//! App-level integration test for Feature #7: Grid Movement & First-Person Camera.
//!
//! Verifies that `spawn_party_and_camera` correctly places `PlayerParty +
//! DungeonCamera` at `floor_01.entry_point = (1, 1, North)` when
//! `GameState::Dungeon` is entered with loaded assets.
//!
//! Uses the same pattern as `tests/dungeon_floor_loads.rs` — drives its own
//! `TestState::Loading → TestState::Loaded` cycle using only `DungeonFloor`
//! (not `LoadingPlugin`), inserts a stub `DungeonAssets` with the loaded floor
//! handle, then transitions to `GameState::Dungeon` and asserts the spawn.
//!
//! **What this test covers:** `spawn_party_and_camera` fires on
//! `OnEnter(GameState::Dungeon)` when `DungeonAssets` and `DungeonFloor` are
//! fully loaded. Movement input is covered by the unit tests in
//! `src/plugins/dungeon/mod.rs`.

use bevy::app::AppExit;
use bevy::asset::AssetPlugin;
use bevy::input::InputPlugin;
use bevy::prelude::*;
use bevy::state::app::StatesPlugin;
use bevy_asset_loader::prelude::*;
use bevy_common_assets::ron::RonAssetPlugin;

use druum::data::{DungeonFloor, ItemDb};
use druum::plugins::audio::SfxRequest;
use druum::plugins::combat::CombatPlugin;
use druum::plugins::dungeon::features::CellFeaturesPlugin;
use druum::plugins::dungeon::{DungeonCamera, DungeonPlugin, GridPosition, PlayerParty};
use druum::plugins::input::ActionsPlugin;
use druum::plugins::loading::DungeonAssets;
use druum::plugins::party::PartyPlugin;
use druum::plugins::state::{GameState, StatePlugin};

/// Private loading state — only loads DungeonFloor (avoids AudioAssets/.ogg
/// files which hang in headless test context because the audio subsystem is
/// not available).
#[derive(Clone, Eq, PartialEq, Debug, Hash, Default, States)]
enum TestState {
    #[default]
    Loading,
    Loaded,
}

#[derive(AssetCollection, Resource)]
struct TestFloorAssets {
    #[asset(path = "dungeons/floor_01.dungeon.ron")]
    floor: Handle<DungeonFloor>,
}

#[test]
fn party_spawns_at_entry_point_on_enter_dungeon() {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        AssetPlugin::default(),
        StatesPlugin,
        InputPlugin,
        RonAssetPlugin::<DungeonFloor>::new(&["dungeon.ron"]),
        StatePlugin,
        ActionsPlugin,
        DungeonPlugin,
        CellFeaturesPlugin,
        PartyPlugin,
        CombatPlugin,
    ));

    // spawn_test_scene requires Assets<Mesh> and Assets<StandardMaterial>.
    // In production these are registered by MeshPlugin/PbrPlugin (via DefaultPlugins).
    // In headless integration tests we init them explicitly.
    // Image + TextureAtlasLayout also required by bevy_sprite3d's bundle_builder
    // (EnemyRenderPlugin → Sprite3dPlugin via CombatPlugin). Feature #17.
    app.init_asset::<Mesh>()
        .init_asset::<StandardMaterial>()
        .init_asset::<bevy::image::Image>()
        .init_asset::<bevy::image::TextureAtlasLayout>();

    // PartyPlugin's populate_item_handle_registry fires on OnExit(Loading) and
    // requires Assets<ItemDb>. Register it explicitly since LoadingPlugin is absent.
    app.init_asset::<ItemDb>();

    // EncounterPlugin (inside CombatPlugin) needs Assets<EncounterTable>. Feature #16.
    app.init_asset::<druum::data::EncounterTable>();

    // EnemyRenderPlugin (via CombatPlugin) reads Assets<EnemyDb>. Feature #17.
    app.init_asset::<druum::data::EnemyDb>();

    // handle_dungeon_input writes SfxRequest messages. AudioPlugin registers
    // this in production; in headless tests we register it directly.
    app.add_message::<SfxRequest>();

    // When compiled with --features dev, StatePlugin::build registers
    // cycle_game_state_on_f9 which requires ButtonInput<KeyCode>. Insert
    // directly so keyboard_input_system's clear loop is not registered.
    // Same pattern as src/plugins/state/mod.rs:107, audio/mod.rs:174.
    #[cfg(feature = "dev")]
    app.init_resource::<bevy::input::ButtonInput<KeyCode>>();

    // Drive our own loading cycle for just the DungeonFloor asset.
    app.init_state::<TestState>().add_loading_state(
        LoadingState::new(TestState::Loading)
            .continue_to_state(TestState::Loaded)
            .load_collection::<TestFloorAssets>(),
    );

    // On TestState::Loaded: insert DungeonAssets pointing to the loaded floor,
    // then queue GameState::Dungeon.
    app.add_systems(OnEnter(TestState::Loaded), setup_dungeon_assets_and_enter);

    // Assertion runs in OnEnter(GameState::Dungeon) — one frame AFTER the
    // spawn_party_and_camera system, because both are in the same OnEnter
    // schedule but commands from spawn_party_and_camera are applied between
    // OnEnter and the next schedule. We need an extra frame.
    // Solution: use an Update system that runs once (tracked by a local bool).
    app.add_systems(
        Update,
        assert_party_at_entry_point.run_if(in_state(GameState::Dungeon)),
    );
    // Insert a resource to track whether we've already asserted.
    app.insert_resource(AssertDone(false));

    // Timeout guard.
    app.add_systems(Update, timeout.run_if(in_state(TestState::Loading)));

    app.run();
}

/// Flag to prevent the assertion from running more than once.
#[derive(Resource)]
struct AssertDone(bool);

fn timeout(time: Res<Time>) {
    if time.elapsed_secs_f64() > 30.0 {
        panic!("DungeonFloor did not load within 30 seconds — RonAssetPlugin path likely broken");
    }
}

/// Insert a `DungeonAssets` resource using the loaded floor handle, then
/// transition `GameState` to Dungeon. The other `DungeonAssets` fields use
/// `Handle::default()` (weak handles to nothing).
fn setup_dungeon_assets_and_enter(
    floor_assets: Res<TestFloorAssets>,
    mut next_game_state: ResMut<NextState<GameState>>,
    mut commands: Commands,
) {
    commands.insert_resource(DungeonAssets {
        floor_01: floor_assets.floor.clone(),
        floor_02: Handle::default(),
        encounters_floor_01: Handle::default(), // Feature #16
        item_db: Handle::default(),
        enemy_db: Handle::default(),
        class_table: Handle::default(),
        spells: Handle::default(),
        // Feature #20 — skill tree handles (weak default handles for tests)
        fighter_skills: Handle::default(),
        mage_skills: Handle::default(),
        priest_skills: Handle::default(),
    });
    next_game_state.set(GameState::Dungeon);
}

/// Verify `PlayerParty` is at `(1, 1)` (floor_01 entry_point) and that exactly
/// one `DungeonCamera` child entity was spawned, then write `AppExit::Success`.
/// Uses an `AssertDone` resource flag to ensure it runs exactly once.
fn assert_party_at_entry_point(
    mut done: ResMut<AssertDone>,
    party_query: Query<&GridPosition, With<PlayerParty>>,
    camera_query: Query<Entity, With<DungeonCamera>>,
    mut exit: MessageWriter<AppExit>,
) {
    if done.0 {
        return;
    }
    done.0 = true;

    let pos = party_query
        .single()
        .expect("PlayerParty entity should be spawned by spawn_party_and_camera");

    assert_eq!(
        pos.x, 1,
        "PlayerParty x should match floor_01 entry_point x=1"
    );
    assert_eq!(
        pos.y, 1,
        "PlayerParty y should match floor_01 entry_point y=1"
    );

    let camera_count = camera_query.iter().count();
    assert_eq!(
        camera_count, 1,
        "Expected exactly one DungeonCamera child entity"
    );

    exit.write(AppExit::Success);
}
