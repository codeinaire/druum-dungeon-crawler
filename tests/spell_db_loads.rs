//! App-level integration test for Feature #20 Phase 1. Loads
//! `assets/spells/core.spells.ron` through
//! `bevy_common_assets::RonAssetPlugin` (the `ron 0.11` parser path)
//! and asserts the resulting `SpellDb` matches the hand-authored shape
//! in the asset file.
//!
//! Mirrors `tests/item_db_loads.rs` (the Feature #12 precedent).
//!
//! FROZEN PATH: The loader at `loading/mod.rs` expects the asset at
//! `spells/core.spells.ron`. This test uses the same path. Do NOT rename.
//!
//! See `project/plans/20260514-120000-feature-20-spells-skill-tree.md` Step 1.8.

use bevy::app::AppExit;
use bevy::asset::AssetPlugin;
use bevy::prelude::*;
use bevy::state::app::StatesPlugin;
use bevy_asset_loader::prelude::*;
use bevy_common_assets::ron::RonAssetPlugin;
use druum::data::{SpellDb, SpellEffect, SpellSchool, SpellTarget};

#[derive(AssetCollection, Resource)]
struct TestAssets {
    #[asset(path = "spells/core.spells.ron")]
    spell_db: Handle<SpellDb>,
}

#[derive(Clone, Eq, PartialEq, Debug, Hash, Default, States)]
enum TestState {
    #[default]
    Loading,
    Loaded,
}

#[test]
fn spell_db_loads_through_ron_asset_plugin() {
    App::new()
        .add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            StatesPlugin,
            RonAssetPlugin::<SpellDb>::new(&["spells.ron"]),
        ))
        .init_state::<TestState>()
        .add_loading_state(
            LoadingState::new(TestState::Loading)
                .continue_to_state(TestState::Loaded)
                .load_collection::<TestAssets>(),
        )
        .add_systems(Update, timeout.run_if(in_state(TestState::Loading)))
        .add_systems(OnEnter(TestState::Loaded), assert_spell_db_shape)
        .run();
}

fn timeout(time: Res<Time>) {
    if time.elapsed_secs_f64() > 30.0 {
        panic!(
            "SpellDb did not load in 30 seconds — RonAssetPlugin path likely broken. \
            Check that the file is at assets/spells/core.spells.ron and the extension \
            is registered as \"spells.ron\" (no leading dot, double-dot filename)."
        );
    }
}

fn assert_spell_db_shape(
    assets: Res<TestAssets>,
    spell_dbs: Res<Assets<SpellDb>>,
    mut exit: MessageWriter<AppExit>,
) {
    let db = spell_dbs
        .get(&assets.spell_db)
        .expect("SpellDb handle should be loaded by now");

    // Authored 15 spells day-one; allow slack so the asset can grow later.
    assert!(
        db.spells.len() > 10,
        "Expected >10 spells in core.spells.ron; got {}",
        db.spells.len()
    );

    // At least one Mage spell.
    assert!(
        db.spells.iter().any(|s| s.school == SpellSchool::Mage),
        "Expected at least one SpellSchool::Mage spell in db"
    );

    // At least one Priest spell.
    assert!(
        db.spells.iter().any(|s| s.school == SpellSchool::Priest),
        "Expected at least one SpellSchool::Priest spell in db"
    );

    // halito — Mage, SingleEnemy, Damage variant.
    let halito = db.get("halito").expect("halito should be in SpellDb");
    assert_eq!(
        halito.school,
        SpellSchool::Mage,
        "halito.school should be Mage"
    );
    assert_eq!(
        halito.target,
        SpellTarget::SingleEnemy,
        "halito.target should be SingleEnemy"
    );
    assert!(
        matches!(halito.effect, SpellEffect::Damage { .. }),
        "halito.effect should be Damage variant; got {:?}",
        halito.effect
    );

    // dios — Priest, SingleAlly, Heal variant.
    let dios = db.get("dios").expect("dios should be in SpellDb");
    assert_eq!(
        dios.school,
        SpellSchool::Priest,
        "dios.school should be Priest"
    );
    assert_eq!(
        dios.target,
        SpellTarget::SingleAlly,
        "dios.target should be SingleAlly"
    );
    assert!(
        matches!(dios.effect, SpellEffect::Heal { .. }),
        "dios.effect should be Heal variant; got {:?}",
        dios.effect
    );

    // di — Priest, Revive variant.
    let di = db.get("di").expect("di should be in SpellDb");
    assert!(
        matches!(di.effect, SpellEffect::Revive { .. }),
        "di.effect should be Revive variant; got {:?}",
        di.effect
    );

    exit.write(AppExit::Success);
}
