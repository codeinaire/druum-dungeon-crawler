//! Layer-3 integration test for Feature #12.
//!
//! Verifies that the production `populate_item_handle_registry` bridge
//! (registered by `PartyPlugin` on `OnExit(GameState::Loading)`) correctly
//! converts a loaded `ItemDb` into working `Handle<ItemAsset>` values, and
//! that those handles drive `recompute_derived_stats_on_equipment_change`
//! to update `DerivedStats` from the loaded asset's stat block.
//!
//! Closes the gap that Layer-2 `app_tests` cannot cover: those mint synthetic
//! handles via `Assets::add(...)` in the test fixture and never exercise the
//! production data path. Without this test, an `init_asset` regression or a
//! broken `OnExit(Loading)` bridge could ship green.
//!
//! ## Why a custom AssetCollection (not LoadingPlugin)?
//!
//! Production `LoadingPlugin` loads `DungeonAssets` + `AudioAssets`. The
//! latter pulls in 10 `Handle<AudioSource>` files which require Bevy's
//! `AudioPlugin` to register the asset type — and `AudioPlugin` requires
//! `DefaultPlugins`, not `MinimalPlugins`. Booting `DefaultPlugins` in tests
//! is heavy and pulls in winit/wgpu (not headless-friendly).
//!
//! Instead, this test composes the production `PartyPlugin` (which owns the
//! bridge registration `OnExit(GameState::Loading) -> populate_item_handle_registry`)
//! with a custom `TestItemAssets` collection that only loads `Handle<ItemDb>`.
//! The state machine (`GameState::Loading -> TitleScreen` driven by
//! `bevy_asset_loader`) is identical to production — only the set of loaded
//! collections differs. The bridge itself is the production code, not a mock.
//!
//! FROZEN PATH: depends on `assets/items/core.items.ron` containing an item
//! with `id = "rusty_sword"` and non-zero `stats.attack`. If that authored
//! content changes, update the lookup ID below.

use bevy::app::AppExit;
use bevy::asset::AssetPlugin;
use bevy::prelude::*;
use bevy::state::app::StatesPlugin;
use bevy_asset_loader::prelude::*;
use bevy_common_assets::ron::RonAssetPlugin;
use druum::data::{ItemAsset, ItemDb};
use druum::plugins::party::{
    DerivedStats, EquipSlot, Equipment, EquipmentChangedEvent, Inventory, ItemHandleRegistry,
    PartyMemberBundle, PartyPlugin, recompute_derived_stats_on_equipment_change,
};
use druum::plugins::state::{GameState, StatePlugin};

/// Minimal AssetCollection: only the items DB, no audio. The production
/// `OnExit(GameState::Loading)` bridge registered by `PartyPlugin` does not
/// care which collections were loaded — it iterates `Assets<ItemDb>` and
/// bridges every entry. So as long as `Handle<ItemDb>` finishes loading
/// before `Loading -> TitleScreen`, the bridge sees the same data the
/// production app would.
#[derive(AssetCollection, Resource)]
struct TestItemAssets {
    #[asset(path = "items/core.items.ron")]
    #[allow(dead_code)] // loaded for its side-effect on Assets<ItemDb>; not read directly
    item_db: Handle<ItemDb>,
}

/// Marker on the test character so the assertion system can find it.
/// Stores the expected attack pulled from the loaded asset so the assertion
/// is a real equality check, not an "is non-zero" tautology.
#[derive(Component)]
struct TestCharacter {
    expected_attack: u32,
}

#[test]
fn equipping_loaded_rusty_sword_raises_attack() {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        AssetPlugin::default(),
        StatesPlugin,
        StatePlugin,
        RonAssetPlugin::<ItemDb>::new(&["items.ron"]),
        PartyPlugin,
    ));
    // The dev-feature build of StatePlugin registers `cycle_game_state_on_f9`
    // which reads `Res<ButtonInput<KeyCode>>` — a resource normally added by
    // Bevy's `InputPlugin`, which `MinimalPlugins` doesn't include. Init it
    // here so parameter validation passes. Mirrors the same workaround in
    // `inventory.rs::app_tests::make_test_app`.
    #[cfg(feature = "dev")]
    app.init_resource::<ButtonInput<KeyCode>>();
    app
        // Drive GameState::Loading -> TitleScreen the same way production
        // LoadingPlugin does, but with a smaller asset set.
        .add_loading_state(
            LoadingState::new(GameState::Loading)
                .continue_to_state(GameState::TitleScreen)
                .load_collection::<TestItemAssets>(),
        )
        // Safety net — runs every frame regardless of state. 30s budget
        // covers BOTH a stalled Loading transition AND a stalled TitleScreen
        // assertion (e.g., spawn_test_character_with_loaded_sword silently
        // returning without spawning, which would leave assert_attack_then_exit
        // looping on an empty query).
        .add_systems(Update, timeout)
        // Bridge ran on OnExit(Loading); next state is TitleScreen. Set up
        // the test scenario when we land there.
        .add_systems(
            OnEnter(GameState::TitleScreen),
            spawn_test_character_with_loaded_sword,
        )
        // Assertion runs after recompute, every frame in TitleScreen. First
        // iteration after OnEnter: recompute reads the message we wrote,
        // updates DerivedStats; this system asserts and exits.
        .add_systems(
            Update,
            assert_attack_then_exit
                .after(recompute_derived_stats_on_equipment_change)
                .run_if(in_state(GameState::TitleScreen)),
        )
        .run();
}

fn timeout(time: Res<Time>, state: Res<State<GameState>>) {
    if time.elapsed_secs_f64() > 30.0 {
        panic!(
            "Test did not complete in 30s (currently in {:?}) — check that \
             core.items.ron parses cleanly, PartyPlugin's bridge registration \
             is intact, and spawn_test_character_with_loaded_sword actually \
             spawns the TestCharacter entity",
            state.get()
        );
    }
}

fn spawn_test_character_with_loaded_sword(
    mut commands: Commands,
    registry: Res<ItemHandleRegistry>,
    items: Res<Assets<ItemAsset>>,
    mut writer: MessageWriter<EquipmentChangedEvent>,
) {
    let sword_handle = registry
        .get("rusty_sword")
        .expect(
            "rusty_sword must be in ItemHandleRegistry after the bridge ran on \
             OnExit(GameState::Loading) — check populate_item_handle_registry \
             registration in PartyPlugin::build",
        )
        .clone();

    let asset = items.get(&sword_handle).expect(
        "rusty_sword's Handle<ItemAsset> must resolve in Assets<ItemAsset> — \
         the bridge inserted it but it isn't there now",
    );
    let expected_attack = asset.stats.attack;
    assert!(
        expected_attack > 0,
        "rusty_sword.stats.attack must be > 0 in core.items.ron to make this \
         test meaningful (currently {}); a zero would let the assertion pass \
         even if recompute did nothing",
        expected_attack
    );

    // Spawn directly with the sword in the weapon slot. We bypass `equip_item`
    // because the goal is to verify the recompute path on a real loaded handle,
    // not to re-test the helper (covered by app_tests::equip_emits_message_via_helper).
    let char_entity = commands
        .spawn((
            PartyMemberBundle {
                equipment: Equipment {
                    weapon: Some(sword_handle),
                    ..Default::default()
                },
                ..Default::default()
            },
            Inventory::default(),
            TestCharacter { expected_attack },
        ))
        .id();

    writer.write(EquipmentChangedEvent {
        character: char_entity,
        slot: EquipSlot::Weapon,
    });
}

fn assert_attack_then_exit(
    chars: Query<(&DerivedStats, &TestCharacter)>,
    mut exit: MessageWriter<AppExit>,
) {
    // Commands from OnEnter flush after the StateTransition schedule; on the
    // first Update iteration the entity may not be queryable yet. Wait until
    // it appears.
    let Ok((derived, expected)) = chars.single() else {
        return;
    };

    assert_eq!(
        derived.attack, expected.expected_attack,
        "DerivedStats::attack should reflect rusty_sword.stats.attack ({}) \
         after equip + recompute, but got {}. Bridge or recompute path is broken.",
        expected.expected_attack, derived.attack
    );

    exit.write(AppExit::Success);
}
