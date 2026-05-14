//! App-level integration test for Feature #20 Phase 2. Loads all three
//! per-class skill trees through `bevy_common_assets::RonAssetPlugin` (the
//! `ron 0.12` parser path) and asserts the resulting `SkillTree` structs match
//! the hand-authored shape in the asset files.
//!
//! Mirrors `tests/spell_db_loads.rs` (the Feature #20 Phase 1 precedent).
//!
//! FROZEN PATHS: the loader expects the assets at:
//!   - `assets/skills/fighter.skills.ron`
//!   - `assets/skills/mage.skills.ron`
//!   - `assets/skills/priest.skills.ron`
//!
//! Do NOT rename these files.
//!
//! See `project/plans/20260514-120000-feature-20-spells-skill-tree.md` Step 3.9.

use bevy::app::AppExit;
use bevy::asset::AssetPlugin;
use bevy::prelude::*;
use bevy::state::app::StatesPlugin;
use bevy_asset_loader::prelude::*;
use bevy_common_assets::ron::RonAssetPlugin;
use druum::data::{NodeGrant, SkillTree, SpellDb, validate_no_cycles};

#[derive(AssetCollection, Resource)]
struct TestSkillAssets {
    #[asset(path = "skills/fighter.skills.ron")]
    fighter: Handle<SkillTree>,
    #[asset(path = "skills/mage.skills.ron")]
    mage: Handle<SkillTree>,
    #[asset(path = "skills/priest.skills.ron")]
    priest: Handle<SkillTree>,
    #[asset(path = "spells/core.spells.ron")]
    spells: Handle<SpellDb>,
}

#[derive(Clone, Eq, PartialEq, Debug, Hash, Default, States)]
enum TestState {
    #[default]
    Loading,
    Loaded,
}

#[test]
fn skill_trees_load_and_validate_no_cycles() {
    App::new()
        .add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            StatesPlugin,
            RonAssetPlugin::<SkillTree>::new(&["skills.ron"]),
            RonAssetPlugin::<SpellDb>::new(&["spells.ron"]),
        ))
        .init_state::<TestState>()
        .add_loading_state(
            LoadingState::new(TestState::Loading)
                .continue_to_state(TestState::Loaded)
                .load_collection::<TestSkillAssets>(),
        )
        .add_systems(Update, timeout.run_if(in_state(TestState::Loading)))
        .add_systems(OnEnter(TestState::Loaded), assert_skill_trees_shape)
        .run();
}

fn timeout(time: Res<Time>) {
    if time.elapsed_secs_f64() > 30.0 {
        panic!(
            "Skill trees did not load in 30 seconds — RonAssetPlugin path likely broken. \
            Check that the files are at assets/skills/<class>.skills.ron and the extension \
            is registered as \"skills.ron\" (no leading dot, double-dot filename)."
        );
    }
}

fn assert_skill_trees_shape(
    assets: Res<TestSkillAssets>,
    skill_trees: Res<Assets<SkillTree>>,
    spell_dbs: Res<Assets<SpellDb>>,
    mut exit: MessageWriter<AppExit>,
) {
    let fighter = skill_trees
        .get(&assets.fighter)
        .expect("fighter skill tree should be loaded");
    let mage = skill_trees
        .get(&assets.mage)
        .expect("mage skill tree should be loaded");
    let priest = skill_trees
        .get(&assets.priest)
        .expect("priest skill tree should be loaded");
    let spell_db = spell_dbs
        .get(&assets.spells)
        .expect("SpellDb should be loaded");

    // ── Fighter ──────────────────────────────────────────────────────────────

    assert_eq!(
        fighter.class_id, "Fighter",
        "fighter tree class_id should be 'Fighter'"
    );
    assert!(
        fighter.nodes.len() >= 6,
        "fighter tree should have at least 6 nodes; got {}",
        fighter.nodes.len()
    );
    assert!(
        validate_no_cycles(fighter).is_ok(),
        "fighter skill tree must be cycle-free: {:?}",
        validate_no_cycles(fighter)
    );
    assert!(
        fighter.root_nodes().count() > 0,
        "fighter tree must have at least one root node"
    );
    assert!(
        fighter.nodes.iter().any(|n| n.min_level > 1),
        "fighter tree must have at least one level-gated node"
    );

    // ── Mage ─────────────────────────────────────────────────────────────────

    assert_eq!(
        mage.class_id, "Mage",
        "mage tree class_id should be 'Mage'"
    );
    assert!(
        mage.nodes.len() >= 8,
        "mage tree should have at least 8 nodes; got {}",
        mage.nodes.len()
    );
    assert!(
        validate_no_cycles(mage).is_ok(),
        "mage skill tree must be cycle-free: {:?}",
        validate_no_cycles(mage)
    );
    assert!(
        mage.root_nodes().count() > 0,
        "mage tree must have at least one root node"
    );
    // Mage should have at least one LearnSpell node that references a real spell.
    let mage_has_valid_spell = mage.nodes.iter().any(|n| {
        if let NodeGrant::LearnSpell(spell_id) = &n.grant {
            spell_db.get(spell_id).is_some()
        } else {
            false
        }
    });
    assert!(
        mage_has_valid_spell,
        "mage tree should have at least one LearnSpell node referencing a spell in SpellDb"
    );

    // ── Priest ───────────────────────────────────────────────────────────────

    assert_eq!(
        priest.class_id, "Priest",
        "priest tree class_id should be 'Priest'"
    );
    assert!(
        priest.nodes.len() >= 8,
        "priest tree should have at least 8 nodes; got {}",
        priest.nodes.len()
    );
    assert!(
        validate_no_cycles(priest).is_ok(),
        "priest skill tree must be cycle-free: {:?}",
        validate_no_cycles(priest)
    );
    assert!(
        priest.root_nodes().count() > 0,
        "priest tree must have at least one root node"
    );
    // Priest should have at least one LearnSpell node that references a real spell.
    let priest_has_valid_spell = priest.nodes.iter().any(|n| {
        if let NodeGrant::LearnSpell(spell_id) = &n.grant {
            spell_db.get(spell_id).is_some()
        } else {
            false
        }
    });
    assert!(
        priest_has_valid_spell,
        "priest tree should have at least one LearnSpell node referencing a spell in SpellDb"
    );

    exit.write(AppExit::Success);
}
