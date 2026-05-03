//! App-level integration test for Feature #4. Loads `floor_01.dungeon.ron`
//! through `bevy_common_assets::RonAssetPlugin` (the `ron 0.11` parser path)
//! and asserts the resulting `DungeonFloor` matches the hand-authored shape
//! in the asset file.
//!
//! This is the verification deferred from Feature #3's code review
//! (`.claude/agent-memory/code-reviewer/feedback_ron_version_split.md`).
//! The unit-level round-trip test in `src/data/dungeon.rs` exercises only
//! the `ron 0.12` (project-direct) path; this integration test exercises
//! the `ron 0.11` (loader-internal, via `bevy_common_assets`) path.

use bevy::app::AppExit;
use bevy::asset::AssetPlugin;
use bevy::prelude::*;
use bevy::state::app::StatesPlugin;
use bevy_asset_loader::prelude::*;
use bevy_common_assets::ron::RonAssetPlugin;
use druum::data::DungeonFloor;
use druum::data::dungeon::Direction;

#[derive(AssetCollection, Resource)]
struct TestAssets {
    #[asset(path = "dungeons/floor_01.dungeon.ron")]
    floor: Handle<DungeonFloor>,
}

#[derive(Clone, Eq, PartialEq, Debug, Hash, Default, States)]
enum TestState {
    #[default]
    Loading,
    Loaded,
}

#[test]
fn floor_01_loads_through_ron_asset_plugin() {
    App::new()
        .add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            StatesPlugin,
            RonAssetPlugin::<DungeonFloor>::new(&["dungeon.ron"]),
        ))
        .init_state::<TestState>()
        .add_loading_state(
            LoadingState::new(TestState::Loading)
                .continue_to_state(TestState::Loaded)
                .load_collection::<TestAssets>(),
        )
        .add_systems(Update, timeout.run_if(in_state(TestState::Loading)))
        .add_systems(OnEnter(TestState::Loaded), assert_floor_shape)
        .run();
}

fn timeout(time: Res<Time>) {
    if time.elapsed_secs_f64() > 30.0 {
        panic!("DungeonFloor did not load in 30 seconds — RonAssetPlugin path likely broken");
    }
}

fn assert_floor_shape(
    assets: Res<TestAssets>,
    floors: Res<Assets<DungeonFloor>>,
    mut exit: MessageWriter<AppExit>, // Bevy 0.18: Message, not Event
) {
    let floor = floors
        .get(&assets.floor)
        .expect("DungeonFloor handle should be loaded by now");
    // Spot-check the same fields the unit test asserts. If the ron 0.11 loader
    // and the ron 0.12 unit test ever diverge, this assertion fires.
    assert_eq!(floor.width, 6);
    assert_eq!(floor.height, 6);
    assert_eq!(floor.entry_point, (1, 1, Direction::North));
    assert!(floor.is_well_formed(), "DungeonFloor failed is_well_formed");
    assert!(
        floor.validate_wall_consistency().is_ok(),
        "DungeonFloor failed wall consistency: {:?}",
        floor.validate_wall_consistency()
    );
    exit.write(AppExit::Success);
}
