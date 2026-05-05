//! App-level integration test for Feature #12. Loads
//! `assets/items/core.items.ron` through
//! `bevy_common_assets::RonAssetPlugin` (the `ron 0.11` parser path)
//! and asserts the resulting `ItemDb` matches the hand-authored shape
//! in the asset file.
//!
//! Mirrors `tests/class_table_loads.rs` (the Feature #11 precedent for
//! verifying that the `RonAssetPlugin` ron-0.11 path matches the unit-level
//! ron-0.12 round-trip in `src/data/items.rs::tests`).
//!
//! FROZEN PATH: The loader at `loading/mod.rs:34` expects the asset at
//! `items/core.items.ron`. This test uses the same path. Do NOT rename.

use bevy::app::AppExit;
use bevy::asset::AssetPlugin;
use bevy::prelude::*;
use bevy::state::app::StatesPlugin;
use bevy_asset_loader::prelude::*;
use bevy_common_assets::ron::RonAssetPlugin;
use druum::data::ItemDb;
use druum::plugins::party::{EquipSlot, ItemKind};

#[derive(AssetCollection, Resource)]
struct TestAssets {
    #[asset(path = "items/core.items.ron")]
    item_db: Handle<ItemDb>,
}

#[derive(Clone, Eq, PartialEq, Debug, Hash, Default, States)]
enum TestState {
    #[default]
    Loading,
    Loaded,
}

#[test]
fn item_db_loads_through_ron_asset_plugin() {
    App::new()
        .add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            StatesPlugin,
            RonAssetPlugin::<ItemDb>::new(&["items.ron"]),
        ))
        .init_state::<TestState>()
        .add_loading_state(
            LoadingState::new(TestState::Loading)
                .continue_to_state(TestState::Loaded)
                .load_collection::<TestAssets>(),
        )
        .add_systems(Update, timeout.run_if(in_state(TestState::Loading)))
        .add_systems(OnEnter(TestState::Loaded), assert_item_db_shape)
        .run();
}

fn timeout(time: Res<Time>) {
    if time.elapsed_secs_f64() > 30.0 {
        panic!("ItemDb did not load in 30 seconds — RonAssetPlugin path likely broken");
    }
}

fn assert_item_db_shape(
    assets: Res<TestAssets>,
    item_dbs: Res<Assets<ItemDb>>,
    mut exit: MessageWriter<AppExit>, // Bevy 0.18: Message, not Event
) {
    let db = item_dbs
        .get(&assets.item_db)
        .expect("ItemDb handle should be loaded by now");

    // Eight authored items in v1 (D4=A: 8-item set).
    assert_eq!(db.items.len(), 8, "Expected 8 items in core.items.ron");

    // Rusty Sword — Weapon, slot Weapon, attack 5.
    let sword = db
        .get("rusty_sword")
        .expect("rusty_sword should be in ItemDb");
    assert_eq!(
        sword.kind,
        ItemKind::Weapon,
        "rusty_sword.kind should be Weapon"
    );
    assert_eq!(
        sword.slot,
        EquipSlot::Weapon,
        "rusty_sword.slot should be Weapon"
    );
    assert_eq!(
        sword.stats.attack, 5,
        "rusty_sword.stats.attack should be 5"
    );

    // Healing Potion — Consumable, slot None, no stats.
    let potion = db
        .get("healing_potion")
        .expect("healing_potion should be in ItemDb");
    assert_eq!(
        potion.kind,
        ItemKind::Consumable,
        "healing_potion.kind should be Consumable"
    );
    assert_eq!(
        potion.slot,
        EquipSlot::None,
        "healing_potion.slot should be None"
    );

    // Rusty Key — KeyItem, slot None.
    let key = db.get("rusty_key").expect("rusty_key should be in ItemDb");
    assert_eq!(
        key.kind,
        ItemKind::KeyItem,
        "rusty_key.kind should be KeyItem"
    );
    assert_eq!(key.slot, EquipSlot::None, "rusty_key.slot should be None");

    exit.write(AppExit::Success);
}
