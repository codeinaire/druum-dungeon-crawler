//! App-level integration test for Feature #11. Loads
//! `assets/classes/core.classes.ron` through
//! `bevy_common_assets::RonAssetPlugin` (the `ron 0.11` parser path)
//! and asserts the resulting `ClassTable` matches the hand-authored shape
//! in the asset file.
//!
//! Mirrors `tests/dungeon_floor_loads.rs` (the Feature #4 precedent for
//! verifying that the `RonAssetPlugin` ron-0.11 path matches the unit-level
//! ron-0.12 round-trip in `src/data/classes.rs::tests`).

use bevy::app::AppExit;
use bevy::asset::AssetPlugin;
use bevy::prelude::*;
use bevy::state::app::StatesPlugin;
use bevy_asset_loader::prelude::*;
use bevy_common_assets::ron::RonAssetPlugin;
use druum::data::ClassTable;
use druum::plugins::party::Class;

#[derive(AssetCollection, Resource)]
struct TestAssets {
    #[asset(path = "classes/core.classes.ron")]
    class_table: Handle<ClassTable>,
}

#[derive(Clone, Eq, PartialEq, Debug, Hash, Default, States)]
enum TestState {
    #[default]
    Loading,
    Loaded,
}

#[test]
fn class_table_loads_through_ron_asset_plugin() {
    App::new()
        .add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            StatesPlugin,
            RonAssetPlugin::<ClassTable>::new(&["classes.ron"]),
        ))
        .init_state::<TestState>()
        .add_loading_state(
            LoadingState::new(TestState::Loading)
                .continue_to_state(TestState::Loaded)
                .load_collection::<TestAssets>(),
        )
        .add_systems(Update, timeout.run_if(in_state(TestState::Loading)))
        .add_systems(OnEnter(TestState::Loaded), assert_class_table_shape)
        .run();
}

fn timeout(time: Res<Time>) {
    if time.elapsed_secs_f64() > 30.0 {
        panic!("ClassTable did not load in 30 seconds — RonAssetPlugin path likely broken");
    }
}

fn assert_class_table_shape(
    assets: Res<TestAssets>,
    class_tables: Res<Assets<ClassTable>>,
    mut exit: MessageWriter<AppExit>, // Bevy 0.18: Message, not Event
) {
    let table = class_tables
        .get(&assets.class_table)
        .expect("ClassTable handle should be loaded by now");

    // Three authored classes in v1.
    assert_eq!(
        table.classes.len(),
        3,
        "Expected 3 classes (Fighter, Mage, Priest)"
    );

    // Fighter — STR 14, authored in core.classes.ron.
    let fighter = table
        .get(Class::Fighter)
        .expect("Fighter should be in ClassTable");
    assert_eq!(fighter.display_name, "Fighter");
    assert_eq!(
        fighter.starting_stats.strength, 14,
        "Fighter STR should be 14"
    );

    // Mage — INT 14.
    let mage = table
        .get(Class::Mage)
        .expect("Mage should be in ClassTable");
    assert_eq!(
        mage.starting_stats.intelligence, 14,
        "Mage INT should be 14"
    );

    // Priest — PIE 14.
    let priest = table
        .get(Class::Priest)
        .expect("Priest should be in ClassTable");
    assert_eq!(priest.starting_stats.piety, 14, "Priest PIE should be 14");

    // Thief — declared enum variant, NOT authored in core.classes.ron.
    assert!(
        table.get(Class::Thief).is_none(),
        "Thief should not be in ClassTable (declared-but-unauthored variant)"
    );

    exit.write(AppExit::Success);
}
