//! App-level integration test for Feature #8: 3D Dungeon Renderer.
//!
//! Note: Feature #9 added per-cell torch entities during development but
//! removed them before merge (scope reduction). The carried torch is a child
//! of `DungeonCamera` and is not tagged `DungeonGeometry`. Count remains 120.
//!
//! Verifies that `spawn_dungeon_geometry` correctly spawns 120 entities tagged
//! with `DungeonGeometry` when `GameState::Dungeon` is entered with a loaded
//! `floor_01`. The math:
//!   - 36 floor tiles (one per cell, 6×6 grid)
//!   - 36 ceiling tiles (one per cell)
//!   - 48 wall plates (per per-edge canonical iteration rule on floor_01.dungeon.ron):
//!       * 14 north walls renderable (y=0 and y=5 rows fully Solid; y=1 has 2 Solid)
//!       * 22 west walls renderable (outer left column + interior doors/special walls)
//!       *  6 south walls (bottom row y=5, all Solid — outer edge)
//!       *  6 east walls (right column x=5, all Solid — outer edge)
//!
//!   Total: 36 + 36 + 48 = 120.
//!
//! Note: the player PointLight (carried torch) is a child of DungeonCamera
//! (NOT tagged DungeonGeometry — cleaned via PlayerParty parent), so it does
//! NOT appear in this count.
//!
//! Uses the same TestState pattern as tests/dungeon_movement.rs — drives its own
//! TestState::Loading -> TestState::Loaded cycle using only DungeonFloor (not
//! LoadingPlugin), inserts a stub DungeonAssets with the loaded floor handle,
//! then transitions to GameState::Dungeon and asserts the entity count.

use bevy::app::AppExit;
use bevy::asset::AssetPlugin;
use bevy::input::InputPlugin;
use bevy::prelude::*;
use bevy::state::app::StatesPlugin;
use bevy_asset_loader::prelude::*;
use bevy_common_assets::ron::RonAssetPlugin;

use druum::data::DungeonFloor;
use druum::plugins::audio::SfxRequest;
use druum::plugins::dungeon::{DungeonGeometry, DungeonPlugin};
use druum::plugins::input::ActionsPlugin;
use druum::plugins::loading::DungeonAssets;
use druum::plugins::state::{GameState, StatePlugin};

/// Private loading state — only loads DungeonFloor (avoids AudioAssets/.ogg
/// files which hang in headless test context — Feature #6's lesson).
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
fn dungeon_geometry_spawns_for_floor_01() {
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
    ));

    // spawn_dungeon_geometry requires Assets<Mesh> and Assets<StandardMaterial>.
    // In production these are registered by MeshPlugin/PbrPlugin (via DefaultPlugins).
    // In headless integration tests we init them explicitly.
    app.init_asset::<Mesh>().init_asset::<StandardMaterial>();

    // handle_dungeon_input writes SfxRequest messages. AudioPlugin registers
    // this in production; in headless tests we register it directly.
    app.add_message::<SfxRequest>();

    // When compiled with --features dev, StatePlugin::build registers
    // cycle_game_state_on_f9 which requires ButtonInput<KeyCode>. Insert
    // directly so the system's parameter validation does not panic.
    // Same pattern as src/plugins/state/mod.rs:107, audio/mod.rs:174,
    // tests/dungeon_movement.rs:75-76.
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

    // Assertion runs in Update once GameState::Dungeon is active. We use an
    // AssertDone resource so the assertion runs exactly once.
    app.add_systems(
        Update,
        assert_dungeon_geometry_count.run_if(in_state(GameState::Dungeon)),
    );
    app.insert_resource(AssertDone(false));

    // Timeout guard — RonAssetPlugin path errors should not silently hang the test.
    app.add_systems(Update, timeout.run_if(in_state(TestState::Loading)));

    app.run();
}

#[derive(Resource)]
struct AssertDone(bool);

fn timeout(time: Res<Time>) {
    if time.elapsed_secs_f64() > 30.0 {
        panic!("DungeonFloor did not load within 30 seconds — RonAssetPlugin path likely broken");
    }
}

fn setup_dungeon_assets_and_enter(
    floor_assets: Res<TestFloorAssets>,
    mut next_game_state: ResMut<NextState<GameState>>,
    mut commands: Commands,
) {
    commands.insert_resource(DungeonAssets {
        floor_01: floor_assets.floor.clone(),
        item_db: Handle::default(),
        enemy_db: Handle::default(),
        class_table: Handle::default(),
        spell_table: Handle::default(),
    });
    next_game_state.set(GameState::Dungeon);
}

/// Run-once Update system: count `DungeonGeometry` entities, assert == 120,
/// then write `AppExit::Success`.
///
/// Count breakdown for floor_01 (6×6 grid):
///   36 floor tiles + 36 ceiling tiles + 48 wall plates = 120.
/// The player PointLight (carried torch) is a child of DungeonCamera
/// (NOT tagged DungeonGeometry — cleaned via PlayerParty parent).
fn assert_dungeon_geometry_count(
    mut done: ResMut<AssertDone>,
    query: Query<&DungeonGeometry>,
    mut exit: MessageWriter<AppExit>,
) {
    if done.0 {
        return;
    }
    done.0 = true;

    let count = query.iter().count();
    assert_eq!(
        count, 120,
        "Geometry entity count for floor_01 must equal 36 floor + 36 ceiling + 48 walls = 120. \
         If this assertion fails after an asset edit, recount per the canonical iteration rule \
         (north + west of every cell, plus south of bottom row, plus east of right column)."
    );

    exit.write(AppExit::Success);
}
