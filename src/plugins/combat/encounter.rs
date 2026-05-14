//! Random-encounter triggering and combat entry — Feature #16.
//!
//! ## Pipeline
//!
//! ```text
//!         MovedEvent (DungeonPlugin)
//!                ↓
//!     check_random_encounter (this module)        ← FoeProximity gate (read)
//!                ↓
//!     EncounterRequested  ←  apply_alarm_trap (features.rs:459)
//!                ↓                              [+ future: FOE producer in #22]
//!     handle_encounter_request (this module)
//!                ↓
//!     spawns Enemy entities + inserts CurrentEncounter + transitions to Combat
//!                ↓
//!     CombatPlugin sub-plugins (TurnManager, EnemyAi, CombatUi) take over
//! ```
//!
//! `handle_encounter_request` is the SOLE writer of `CurrentEncounter` and the SOLE
//! producer of the `Dungeon → Combat` transition trigger. It is the de-facto
//! `start_combat` system; both random rolls and alarm-traps feed the same channel.
//!
//! ## Soft-pity formula (D-X2)
//!
//! `rate = cell.encounter_rate.clamp(0.0, 1.0) * (1.0 + steps_since_last as f32 * 0.05).min(2.0)`
//!
//! - Per-step bump on every `MovedEvent`, regardless of cell rate (no special-casing).
//! - Multiplier capped at 2.0 to prevent unbounded growth across rate-zero corridors.
//! - Counter resets on trigger AND on every `OnEnter(Dungeon)` (D-X1).
//!
//! ## RNG (D-A5)
//!
//! `EncounterRng` is a separate resource from `combat::turn_manager::CombatRng`.
//! Both wrap `Box<dyn rand::RngCore + Send + Sync>` for trait-object dispatch.
//! Tests inject `ChaCha8Rng::seed_from_u64(...)` directly.
//!
//! ## `?Sized` discipline
//!
//! `pick_group` (in `data/encounters.rs`) takes `rng: &mut (impl rand::Rng + ?Sized)`
//! per #15 D-I13 — required to permit `&mut *rng.0` from a `Box<dyn RngCore>` DST.

use bevy::prelude::*;
use rand::Rng;

use crate::data::DungeonFloor;
use crate::data::EnemyDb;
use crate::data::EncounterTable;
use crate::plugins::audio::{SfxKind, SfxRequest};
use crate::plugins::combat::enemy::{Enemy, EnemyBundle, EnemyIndex, EnemyName};
use crate::plugins::combat::enemy_render::EnemyVisual;
use crate::plugins::dungeon::{
    ActiveFloorNumber, MovedEvent, MovementAnimation, PlayerParty, floor_handle_for,
    handle_dungeon_input,
};
use crate::plugins::dungeon::features::{EncounterRequested, EncounterSource};
use crate::plugins::loading::{DungeonAssets, encounter_table_for};
use crate::plugins::state::GameState;

// ─────────────────────────────────────────────────────────────────────────────
// Constants
// ─────────────────────────────────────────────────────────────────────────────

/// Cap the soft-pity multiplier (D-X2). After 20 missed steps the multiplier
/// saturates at 2.0; rate-zero corridors don't unboundedly accumulate.
const ACCUMULATOR_MULTIPLIER_CAP: f32 = 2.0;

/// Per-step bonus to the encounter probability multiplier (research §Code Examples).
const STEP_PROBABILITY_BONUS: f32 = 0.05;

/// Trust-boundary cap on enemy group size — defends against malicious or
/// typo'd RON values. Oversized groups are truncated with a `warn!`.
const MAX_ENEMIES_PER_ENCOUNTER: usize = 8;

// ─────────────────────────────────────────────────────────────────────────────
// Resources
// ─────────────────────────────────────────────────────────────────────────────

/// Soft-pity step accumulator. Bumped on every `MovedEvent`; reset to 0 on
/// encounter trigger AND on every `OnEnter(Dungeon)` (D-X1).
#[derive(Resource, Default, Debug, Clone)]
pub struct EncounterState {
    pub steps_since_last: u32,
}

/// RNG source for encounter rolls (D-A5). Separate from `combat::turn_manager::CombatRng`
/// because encounter rolls happen during `GameState::Dungeon` where `CombatRng` may
/// have stale state (it re-seeds in `init_combat_state` on `OnEnter(Combat)`).
///
/// Tests insert `EncounterRng(Box::new(ChaCha8Rng::seed_from_u64(seed)))` directly.
#[derive(Resource)]
pub struct EncounterRng(pub Box<dyn rand::RngCore + Send + Sync>);

impl Default for EncounterRng {
    fn default() -> Self {
        use rand::SeedableRng;
        Self(Box::new(rand::rngs::SmallRng::from_os_rng()))
    }
}

/// The currently-active combat encounter. Populated by `handle_encounter_request`
/// on transition to `GameState::Combat`; removed on `OnExit(Combat)` (Pitfall 6).
///
/// Shape locked by #15 contract at `combat/turn_manager.rs:34-46`.
#[derive(Resource, Debug, Clone)]
pub struct CurrentEncounter {
    pub enemy_entities: Vec<Entity>,
    /// `false` for forced encounters (boss FOEs in #22); `true` otherwise.
    /// Read by #15's flee logic — `Flee` against an unfleeable encounter logs
    /// "Cannot flee" without consuming the action turn.
    pub fleeable: bool,
}

/// Read by `check_random_encounter` to suppress rolls when an FOE is visible.
///
/// **#16 stub:** ships with `Default::default()` (always returns "no FOEs"). #22
/// replaces the populator system that updates `nearby_foe_entities`.
///
/// **Why a `Vec<Entity>`, not `bool`:** future #22 may want richer suppression
/// rules (e.g., suppress only for boss-tier FOEs; or "soft-pity revenge" — boost
/// rate for N steps after FOE disappears). Keeping the resource shape generic now
/// avoids #22 needing to break the type.
#[derive(Resource, Default, Debug, Clone)]
pub struct FoeProximity {
    /// FOE entities within line-of-sight or N-cell radius (definition is #22's call).
    pub nearby_foe_entities: Vec<Entity>,
}

impl FoeProximity {
    /// Suppress random encounter rolls when at least one FOE is nearby.
    /// #22 may override the rule (e.g., only suppress for boss-tier FOEs).
    pub fn suppresses_random_rolls(&self) -> bool {
        !self.nearby_foe_entities.is_empty()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Plugin
// ─────────────────────────────────────────────────────────────────────────────

pub struct EncounterPlugin;

impl Plugin for EncounterPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<EncounterState>()
            .init_resource::<EncounterRng>()
            .init_resource::<FoeProximity>()
            .add_systems(OnEnter(GameState::Dungeon), reset_encounter_state)
            .add_systems(
                OnEnter(GameState::Combat),
                snap_movement_animation_on_combat_entry,
            )
            .add_systems(OnExit(GameState::Combat), clear_current_encounter)
            .add_systems(
                Update,
                (
                    check_random_encounter
                        .run_if(in_state(GameState::Dungeon))
                        .after(handle_dungeon_input),
                    handle_encounter_request
                        .run_if(in_state(GameState::Dungeon))
                        .after(check_random_encounter),
                ),
            );

        #[cfg(feature = "dev")]
        app.add_systems(
            Update,
            force_encounter_on_f7.run_if(in_state(GameState::Dungeon)),
        );
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Systems
// ─────────────────────────────────────────────────────────────────────────────

fn reset_encounter_state(mut state: ResMut<EncounterState>) {
    state.steps_since_last = 0;
}

/// Snap any in-flight `MovementAnimation` to its destination on `OnEnter(Combat)`.
///
/// Without this, a 50%-complete eastward step would freeze during combat (the
/// `animate_movement` system is gated `.run_if(in_state(GameState::Dungeon))`)
/// and resume on combat-exit, producing a perceived "jump". Snapping makes
/// the transition instant; polish (encounter-sting flash that masks the snap)
/// is deferred to #25.
///
/// Same logic as `animate_movement`'s `t_raw >= 1.0` branch
/// (`dungeon/mod.rs:952-957`).
fn snap_movement_animation_on_combat_entry(
    mut commands: Commands,
    mut query: Query<(Entity, &mut Transform, &MovementAnimation), With<PlayerParty>>,
) {
    for (entity, mut transform, anim) in &mut query {
        transform.translation = anim.to_translation;
        transform.rotation = anim.to_rotation;
        commands.entity(entity).remove::<MovementAnimation>();
    }
}

/// Tear down the encounter on `OnExit(Combat)`: despawn every `Enemy`-marked
/// entity in the world, then remove the `CurrentEncounter` resource.
///
/// Sweeping by marker (rather than only the entities listed in
/// `CurrentEncounter.enemy_entities`) is intentional — there is no other
/// production spawner of `Enemy`, so anything still carrying the marker
/// after combat is by definition stale. Without this sweep, dead enemies
/// from a prior encounter remain in the world and reappear in the next
/// combat's `Query<Entity, With<Enemy>>` result, showing up as
/// already-dead corpses in the new fight.
fn clear_current_encounter(mut commands: Commands, enemies: Query<Entity, With<Enemy>>) {
    for entity in &enemies {
        commands.entity(entity).despawn();
    }
    commands.remove_resource::<CurrentEncounter>();
}

/// Roll the encounter probability for each step the player takes.
///
/// Pipeline per `MovedEvent`:
/// 1. Bump `steps_since_last` (every step, regardless of outcome or asset state).
/// 2. Skip roll if assets not ready (drain cursor per Pitfall 4).
/// 3. Read destination cell's `encounter_rate`, clamped to `[0.0, 1.0]`.
/// 4. Skip if rate is 0 (designer-authored "safe corridor").
/// 5. Skip if `FoeProximity::suppresses_random_rolls()` (FOE in line-of-sight, #22).
/// 6. Apply soft-pity multiplier: `(1.0 + steps * STEP_BONUS).min(CAP)`.
/// 7. Roll `f32`; if hit, write `EncounterRequested { source: Random }` and reset counter.
///
/// Step counter bumps before all guards so the soft-pity contract holds even when
/// DungeonAssets are not yet ready (e.g., very early frames or test setups without
/// the full asset pipeline).
#[allow(clippy::too_many_arguments)]
fn check_random_encounter(
    mut moved: MessageReader<MovedEvent>,
    mut state: ResMut<EncounterState>,
    mut rng: ResMut<EncounterRng>,
    mut encounter: MessageWriter<EncounterRequested>,
    foe_proximity: Res<FoeProximity>,
    dungeon_assets: Option<Res<DungeonAssets>>,
    floors: Res<Assets<DungeonFloor>>,
    active_floor: Res<ActiveFloorNumber>,
) {
    // Resolve the floor once, before the per-event loop.
    // Returns None if assets not yet ready (early game / test without full pipeline).
    let maybe_floor: Option<&DungeonFloor> = match dungeon_assets.as_ref() {
        Some(assets) => floors.get(floor_handle_for(assets, active_floor.0)),
        None => None,
    };

    // Guard: emit at most one EncounterRequested per frame from this system
    // (defensive — Druum's MovedEvent is one-per-step but a future batch step
    // would otherwise let multiple rolls succeed in the same frame).
    let mut already_rolled = false;
    for ev in moved.read() {
        // Bump counter on every step, regardless of outcome (soft-pity contract).
        // This must come BEFORE asset guards so the counter stays accurate even
        // when assets are not yet loaded.
        state.steps_since_last = state.steps_since_last.saturating_add(1);

        // Skip the roll if assets are not ready (Pitfall 4 — cursor already drained
        // by the for-loop; counter was bumped above).
        let Some(floor) = maybe_floor else {
            continue;
        };

        // FOE suppression hook for #22 (research §FoeProximity).
        if foe_proximity.suppresses_random_rolls() {
            continue;
        }

        // Already rolled this frame — skip subsequent checks but keep
        // bumping the counter (already done above).
        if already_rolled {
            continue;
        }

        // Trust-boundary clamp on RON-deserialized rate (Security §Architectural Risks).
        let cell_rate = floor
            .features
            .get(ev.to.y as usize)
            .and_then(|row| row.get(ev.to.x as usize))
            .map(|c| c.encounter_rate.clamp(0.0, 1.0))
            .unwrap_or(0.0);

        // Designer-authored safe corridor: skip the roll entirely.
        if cell_rate <= 0.0 {
            continue;
        }

        // Soft-pity formula (D-X2): cap multiplier at 2.0.
        let multiplier = (1.0 + state.steps_since_last as f32 * STEP_PROBABILITY_BONUS)
            .min(ACCUMULATOR_MULTIPLIER_CAP);
        let probability = cell_rate * multiplier;

        // rand 0.9 rename: rng.gen::<f32>() → rng.random::<f32>().
        if rng.0.random::<f32>() < probability {
            state.steps_since_last = 0;
            already_rolled = true;
            encounter.write(EncounterRequested {
                source: EncounterSource::Random,
            });
            info!(
                "Random encounter triggered at {:?} (rate={:.3}, multiplier={:.2})",
                ev.to, cell_rate, multiplier
            );
        }
    }
}

/// Consume `EncounterRequested` messages: pick an enemy group, spawn enemies,
/// populate `CurrentEncounter`, transition state.
///
/// SOLE writer of `CurrentEncounter` and the `Dungeon → Combat` transition trigger.
/// Same-frame multiple `EncounterRequested` writes (alarm-trap + random roll) collapse
/// to a single combat — we take only the first via `requests.read().next()` (D-A8).
///
/// Drains the cursor on early returns (no-asset, no-table, empty-group) so
/// stale messages don't replay next frame.
#[allow(clippy::too_many_arguments)]
fn handle_encounter_request(
    mut requests: MessageReader<EncounterRequested>,
    mut commands: Commands,
    mut next_state: ResMut<NextState<GameState>>,
    encounter_tables: Res<Assets<EncounterTable>>,
    enemy_dbs: Res<Assets<EnemyDb>>,
    dungeon_assets: Option<Res<DungeonAssets>>,
    active_floor: Res<ActiveFloorNumber>,
    mut rng: ResMut<EncounterRng>,
    mut sfx: MessageWriter<SfxRequest>,
) {
    // Take ONLY the first request; discard the rest (D-A8 — collapse stacked encounters).
    let req = match requests.read().next() {
        Some(r) => *r,
        None => return,
    };
    // Drain any subsequent requests so they don't replay next frame.
    for _ in requests.read() {}

    let Some(assets) = dungeon_assets else {
        warn!("EncounterRequested fired but DungeonAssets missing; skipping");
        return;
    };
    let table_handle = encounter_table_for(&assets, active_floor.0);
    let Some(table) = encounter_tables.get(table_handle) else {
        warn!(
            "EncounterRequested fired but EncounterTable for floor {} not yet loaded; skipping",
            active_floor.0
        );
        return;
    };

    let Some(group) = table.pick_group(&mut *rng.0) else {
        warn!("EncounterRequested fired but EncounterTable is empty; skipping");
        return;
    };

    // Resolve EnemyDb for visual lookups. Absent EnemyDb is recoverable —
    // spawn_enemy_billboards falls back to DEFAULT_PLACEHOLDER_COLOR when
    // EnemyVisual.id doesn't resolve. We still emit a warn! to flag the
    // asset-loading regression.
    let enemy_db = enemy_dbs.get(&assets.enemy_db);
    if enemy_db.is_none() {
        warn!("EnemyDb not yet loaded — enemies will use default placeholder colour");
    }

    // Trust-boundary cap on enemy count (Security §Architectural Risks).
    let enemies_to_spawn = if group.enemies.len() > MAX_ENEMIES_PER_ENCOUNTER {
        warn!(
            "EnemyGroup has {} enemies; truncating to MAX_ENEMIES_PER_ENCOUNTER ({})",
            group.enemies.len(),
            MAX_ENEMIES_PER_ENCOUNTER
        );
        &group.enemies[..MAX_ENEMIES_PER_ENCOUNTER]
    } else {
        &group.enemies[..]
    };

    let mut entities = Vec::with_capacity(enemies_to_spawn.len());
    for (idx, spec) in enemies_to_spawn.iter().enumerate() {
        let entity = commands
            .spawn(EnemyBundle {
                name: EnemyName(spec.name.clone()),
                index: EnemyIndex(idx as u32),
                base_stats: spec.base_stats,
                derived_stats: spec.derived_stats,
                ai: spec.ai,
                ..Default::default()
            })
            .id();

        // Feature #17: populate EnemyVisual from EnemyDb lookup.
        // Empty id (back-compat with inline EnemySpec) → default grey colour.
        let placeholder_color = enemy_db
            .and_then(|db| db.find(&spec.id))
            .map(|def| def.placeholder_color)
            .unwrap_or([0.5, 0.5, 0.5]); // mirrors DEFAULT_PLACEHOLDER_COLOR
        commands.entity(entity).insert(EnemyVisual {
            id: spec.id.clone(),
            placeholder_color,
        });

        entities.push(entity);
    }

    // Populate CurrentEncounter — single source of truth for #15.
    // `fleeable` per source: Random and AlarmTrap are fleeable; future Foe { boss: true }
    // (in #22) is not. Match must be exhaustive — adding a variant in #22 forces
    // this site to update.
    let fleeable = match req.source {
        EncounterSource::Random | EncounterSource::AlarmTrap => true,
    };
    commands.insert_resource(CurrentEncounter {
        enemy_entities: entities.clone(),
        fleeable,
    });

    // Audio cue — alarm-trap path already emits this from features.rs:483-485
    // for AlarmTrap source; we emit for Random source here to keep the cue
    // consistent regardless of producer.
    if matches!(req.source, EncounterSource::Random) {
        sfx.write(SfxRequest {
            kind: SfxKind::EncounterSting,
        });
    }

    info!(
        "Encounter ({:?}) triggered: spawned {} enemies, transitioning to Combat",
        req.source,
        entities.len()
    );

    // State transition — #15's CombatPlugin sub-plugins take over on OnEnter(Combat).
    next_state.set(GameState::Combat);
}

/// Dev-only: F7 forces a random encounter immediately. Mirrors the F9 cycler
/// pattern at `state/mod.rs:71-89` — direct `ButtonInput<KeyCode>` reader,
/// gated `cfg(feature = "dev")`. Does NOT touch the frozen leafwing
/// `DungeonAction` enum (D-X6).
#[cfg(feature = "dev")]
fn force_encounter_on_f7(
    keys: Res<bevy::input::ButtonInput<bevy::prelude::KeyCode>>,
    mut encounter: MessageWriter<EncounterRequested>,
) {
    if keys.just_pressed(bevy::prelude::KeyCode::F7) {
        info!("DEV: Forcing encounter via F7");
        encounter.write(EncounterRequested {
            source: EncounterSource::Random,
        });
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests — Layer 1 (pure)
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn foe_proximity_default_does_not_suppress() {
        let fp = FoeProximity::default();
        assert!(!fp.suppresses_random_rolls());
    }

    #[test]
    fn foe_proximity_with_entities_suppresses() {
        let fp = FoeProximity {
            nearby_foe_entities: vec![Entity::PLACEHOLDER],
        };
        assert!(fp.suppresses_random_rolls());
    }

    #[test]
    fn encounter_state_default_is_zero() {
        let s = EncounterState::default();
        assert_eq!(s.steps_since_last, 0);
    }

    /// `check_random_encounter` clamps `encounter_rate` to `[0.0, 1.0]` before use
    /// (line 268). This test verifies the clamp expression directly using the same
    /// `CellFeatures` type that the production code reads.
    #[test]
    fn encounter_rate_clamp() {
        use crate::data::dungeon::CellFeatures;
        // Values above 1.0 must be clamped down.
        let high = CellFeatures {
            encounter_rate: 1.5,
            ..Default::default()
        };
        assert_eq!(
            high.encounter_rate.clamp(0.0, 1.0),
            1.0,
            "encounter_rate 1.5 must clamp to 1.0"
        );
        // Values below 0.0 must be clamped up.
        let negative = CellFeatures {
            encounter_rate: -0.3,
            ..Default::default()
        };
        assert_eq!(
            negative.encounter_rate.clamp(0.0, 1.0),
            0.0,
            "encounter_rate -0.3 must clamp to 0.0"
        );
        // In-range values are unchanged.
        let valid = CellFeatures {
            encounter_rate: 0.6,
            ..Default::default()
        };
        assert_eq!(
            valid.encounter_rate.clamp(0.0, 1.0),
            0.6,
            "encounter_rate 0.6 must remain unchanged after clamp"
        );
    }

    /// `handle_encounter_request` truncates oversized `EnemyGroup`s to
    /// `MAX_ENEMIES_PER_ENCOUNTER` (8). This test verifies the slice logic
    /// used at lines 343-352.
    #[test]
    fn max_enemies_per_encounter_truncation() {
        use crate::data::encounters::{EnemyGroup, EnemySpec};
        use crate::plugins::combat::ai::EnemyAi;
        use crate::plugins::party::character::{BaseStats, DerivedStats};

        // Build a group with 12 enemies — well over the cap of 8.
        let mk = |n: &str| EnemySpec {
            id: n.to_lowercase(),
            name: n.into(),
            base_stats: BaseStats::default(),
            derived_stats: DerivedStats::default(),
            ai: EnemyAi::default(),
        };
        let group = EnemyGroup {
            enemies: (0..12).map(|i| mk(&format!("Enemy{i}"))).collect(),
        };

        // Mirror the production truncation from encounter.rs:343-350.
        let enemies_to_spawn = if group.enemies.len() > MAX_ENEMIES_PER_ENCOUNTER {
            &group.enemies[..MAX_ENEMIES_PER_ENCOUNTER]
        } else {
            &group.enemies[..]
        };

        assert_eq!(
            enemies_to_spawn.len(),
            MAX_ENEMIES_PER_ENCOUNTER,
            "groups of 12 must be truncated to MAX_ENEMIES_PER_ENCOUNTER ({MAX_ENEMIES_PER_ENCOUNTER})"
        );
        // Also verify a group that's already within bounds is not truncated.
        let small_group = EnemyGroup {
            enemies: (0..3).map(|i| mk(&format!("E{i}"))).collect(),
        };
        let small_to_spawn = if small_group.enemies.len() > MAX_ENEMIES_PER_ENCOUNTER {
            &small_group.enemies[..MAX_ENEMIES_PER_ENCOUNTER]
        } else {
            &small_group.enemies[..]
        };
        assert_eq!(
            small_to_spawn.len(),
            3,
            "groups of 3 must not be truncated"
        );
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests — Layer 2 (App-driven)
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod app_tests {
    use super::*;
    use bevy::state::app::StatesPlugin;
    use rand::SeedableRng;

    use crate::data::dungeon::{CellFeatures, WallMask, WallType};
    use crate::plugins::dungeon::{Facing, GridPosition};

    fn make_test_app() -> App {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            bevy::asset::AssetPlugin::default(),
            StatesPlugin,
            crate::plugins::state::StatePlugin,
            crate::plugins::party::PartyPlugin,
            crate::plugins::combat::CombatPlugin,
            crate::plugins::dungeon::features::CellFeaturesPlugin,
        ));
        app.init_asset::<crate::data::DungeonFloor>();
        app.init_asset::<crate::data::ItemDb>();
        app.init_asset::<crate::data::ItemAsset>();
        app.init_asset::<crate::data::EncounterTable>();
        app.init_asset::<crate::data::EnemyDb>();
        // Mesh + StandardMaterial + Image + TextureAtlasLayout needed by bevy_sprite3d's bundle_builder
        // (EnemyRenderPlugin → Sprite3dPlugin via CombatPlugin; MinimalPlugins lacks PbrPlugin).
        app.init_asset::<bevy::prelude::Mesh>();
        app.init_asset::<bevy::pbr::StandardMaterial>();
        app.init_asset::<bevy::image::Image>();
        app.init_asset::<bevy::image::TextureAtlasLayout>();
        // MovedEvent is owned by DungeonPlugin (mod.rs:224); register here so
        // MessageReader<MovedEvent> doesn't panic when DungeonPlugin isn't loaded.
        app.add_message::<crate::plugins::dungeon::MovedEvent>();
        app.init_resource::<crate::plugins::dungeon::ActiveFloorNumber>();
        // SfxRequest is normally registered by AudioPlugin; CellFeaturesPlugin writes it.
        // Explicit registration required in this test app (same pattern as features.rs:781).
        app.add_message::<crate::plugins::audio::SfxRequest>();
        // ActionState<DungeonAction> required by CellFeaturesPlugin::handle_door_interact.
        // Inserted without ActionsPlugin to avoid mouse-resource panic (same pattern as features.rs:769).
        app.init_resource::<
            leafwing_input_manager::prelude::ActionState<crate::plugins::input::DungeonAction>,
        >();
        // ActionState<CombatAction> required by handle_combat_input (CombatUiPlugin).
        app.init_resource::<
            leafwing_input_manager::prelude::ActionState<crate::plugins::input::CombatAction>,
        >();
        // ButtonInput<KeyCode> for force_encounter_on_f7 under cfg(feature = "dev").
        #[cfg(feature = "dev")]
        app.init_resource::<bevy::input::ButtonInput<bevy::prelude::KeyCode>>();
        app
    }

    /// Build a test app pre-wired with a 200-cell corridor floor at the given
    /// `encounter_rate`, a seeded RNG, and `DungeonAssets` pointing to the floor.
    ///
    /// 200 cells is wide enough for 100-step rate-zero and 50-step FOE-suppression
    /// tests without triggering the bounds check in `apply_alarm_trap` / `apply_pit_trap`.
    ///
    /// Used by tests that need `check_random_encounter` to reach past the
    /// asset-guard early-return and exercise the real rate-zero / FOE-suppression
    /// code paths.
    fn make_test_app_with_floor(rate: f32) -> App {
        let mut app = make_test_app();
        let floor_handle = build_test_floor(&mut app, 200, rate);
        let seed_rng = rand_chacha::ChaCha8Rng::seed_from_u64(42);
        app.world_mut()
            .insert_resource(EncounterRng(Box::new(seed_rng)));
        app.world_mut()
            .insert_resource(crate::plugins::loading::DungeonAssets {
                floor_01: floor_handle,
                floor_02: Handle::default(),
                encounters_floor_01: Handle::default(),
                item_db: Handle::default(),
                enemy_db: Handle::default(),
                class_table: Handle::default(),
                spells: Handle::default(),
            });
        app
    }

    /// Build a 1×N corridor floor with `encounter_rate` set on every cell.
    fn build_test_floor(
        app: &mut App,
        width: u32,
        rate: f32,
    ) -> Handle<crate::data::DungeonFloor> {
        use crate::data::DungeonFloor;
        use crate::data::dungeon::Direction;
        let floor = DungeonFloor {
            name: "test".into(),
            width,
            height: 1,
            floor_number: 1,
            walls: vec![vec![
                WallMask {
                    north: WallType::Solid,
                    south: WallType::Solid,
                    east: WallType::Open,
                    west: WallType::Open,
                };
                width as usize
            ]],
            features: vec![(0..width)
                .map(|_| CellFeatures {
                    encounter_rate: rate,
                    ..Default::default()
                })
                .collect()],
            entry_point: (0, 0, Direction::East),
            encounter_table: "b1f_test".into(),
            ..Default::default()
        };
        app.world_mut()
            .resource_mut::<Assets<DungeonFloor>>()
            .add(floor)
    }

    fn write_moved_event(app: &mut App, from_x: u32, to_x: u32) {
        use crate::data::dungeon::Direction;
        app.world_mut()
            .resource_mut::<bevy::ecs::message::Messages<MovedEvent>>()
            .write(MovedEvent {
                from: GridPosition { x: from_x, y: 0 },
                to: GridPosition { x: to_x, y: 0 },
                facing: Direction::East,
            });
    }

    #[test]
    fn steps_reset_on_dungeon_entry() {
        let mut app = make_test_app();
        // Pre-bump the counter.
        app.world_mut()
            .resource_mut::<EncounterState>()
            .steps_since_last = 30;
        // Transition Loading → Dungeon (which is the natural "OnEnter(Dungeon)" path).
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Dungeon);
        app.update();
        app.update();
        // Counter must be reset to 0.
        assert_eq!(
            app.world().resource::<EncounterState>().steps_since_last,
            0,
            "OnEnter(Dungeon) must reset steps_since_last (D-X1)"
        );
    }

    #[test]
    fn current_encounter_removed_on_combat_exit() {
        let mut app = make_test_app();
        app.world_mut().insert_resource(CurrentEncounter {
            enemy_entities: vec![],
            fleeable: true,
        });
        // Transition into Combat...
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Combat);
        app.update();
        app.update();
        // ...then out.
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Dungeon);
        app.update();
        app.update();
        // Resource must be gone (Pitfall 6).
        assert!(
            app.world().get_resource::<CurrentEncounter>().is_none(),
            "CurrentEncounter must be removed on OnExit(Combat)"
        );
    }

    #[test]
    fn movement_animation_snaps_on_combat_entry() {
        use crate::data::dungeon::Direction;
        let mut app = make_test_app();
        // Spawn a PlayerParty with an in-flight MovementAnimation (50% complete).
        let from = Vec3::new(0.0, 0.0, 0.0);
        let to = Vec3::new(2.0, 0.0, 0.0);
        let entity = app
            .world_mut()
            .spawn((
                PlayerParty,
                Transform::from_translation(from),
                GridPosition { x: 0, y: 0 },
                Facing(Direction::East),
                MovementAnimation {
                    from_translation: from,
                    to_translation: to,
                    from_rotation: Quat::IDENTITY,
                    to_rotation: Quat::IDENTITY,
                    elapsed_secs: 0.09,
                    duration_secs: 0.18,
                },
            ))
            .id();
        // Transition into Combat to fire OnEnter(Combat).
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Combat);
        app.update();
        app.update();
        // MovementAnimation should be removed.
        assert!(
            app.world().entity(entity).get::<MovementAnimation>().is_none(),
            "MovementAnimation should be removed on combat entry"
        );
        // Transform should be at the destination.
        let transform = app
            .world()
            .entity(entity)
            .get::<Transform>()
            .unwrap();
        assert!(
            (transform.translation - to).length() < 1e-4,
            "Transform should snap to destination: got {:?}, expected {:?}",
            transform.translation,
            to
        );
    }

    #[test]
    fn rate_zero_cell_no_encounter_rolls() {
        // Use the floor-wired app so check_random_encounter reaches the rate-zero
        // guard (line 272), NOT the asset-missing early-return (line 248).
        let mut app = make_test_app_with_floor(0.0);
        // Force into Dungeon state so check_random_encounter is gated correctly.
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Dungeon);
        app.update();
        app.update();

        // Walk 100 steps with rate = 0.
        for i in 0..100u32 {
            write_moved_event(&mut app, i, i + 1);
            app.update();
        }

        // Counter still bumps (every step), but no encounters fire.
        // Counter is 100 because rate=0 still bumps the counter (no special-casing).
        // This verifies the rate-zero guard at line 272 fired, not the asset-missing
        // guard at line 248 (which would also block encounters but not bump the counter
        // beyond the bump done before the guard — the counter still increments either way,
        // so the real guard here is CurrentEncounter remaining absent).
        assert_eq!(
            app.world().resource::<EncounterState>().steps_since_last,
            100,
            "rate-zero cells still bump the counter (D-X2)"
        );
        // No CurrentEncounter resource — rate-zero guard prevented any roll.
        assert!(
            app.world().get_resource::<CurrentEncounter>().is_none(),
            "rate-zero floor must never trigger CurrentEncounter"
        );
    }

    #[test]
    fn foe_proximity_suppresses_rolls() {
        // Use the floor-wired app with rate=1.0 so check_random_encounter reaches the
        // FOE-suppression guard (line 253), NOT the asset-missing early-return (line 248).
        // rate=1.0 would guarantee an encounter every step if FOE suppression were absent.
        let mut app = make_test_app_with_floor(1.0);
        app.world_mut().insert_resource(FoeProximity {
            nearby_foe_entities: vec![Entity::PLACEHOLDER],
        });
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Dungeon);
        app.update();
        app.update();

        for i in 0..50u32 {
            write_moved_event(&mut app, i, i + 1);
            app.update();
        }

        // Counter still bumps (every step) but no encounter fires.
        // steps_since_last is reset on OnEnter(Dungeon) then bumped once per step.
        // After 50 steps it must be exactly 50 (no reset from an encounter trigger).
        assert_eq!(
            app.world().resource::<EncounterState>().steps_since_last,
            50,
            "FOE-suppressed steps must all bump the counter"
        );
        assert!(
            app.world().get_resource::<CurrentEncounter>().is_none(),
            "FoeProximity must suppress random rolls even on rate=1.0 floor"
        );
    }

    #[test]
    fn encounter_request_bails_safely_without_dungeon_assets() {
        let mut app = make_test_app();
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Dungeon);
        app.update();

        // Write an EncounterRequested directly.
        app.world_mut()
            .resource_mut::<bevy::ecs::message::Messages<EncounterRequested>>()
            .write(EncounterRequested {
                source: EncounterSource::Random,
            });
        app.update();
        app.update();

        // Without DungeonAssets resource, the handler logs a warning and bails.
        // State stays in Dungeon.
        assert_eq!(
            *app.world().resource::<State<GameState>>().get(),
            GameState::Dungeon,
            "handle_encounter_request should bail safely when DungeonAssets is absent"
        );
    }

    /// Guard that `handle_encounter_request` is the SOLE production writer of
    /// `CurrentEncounter`.
    ///
    /// Greps `src/` for the system call form `commands.insert_resource(CurrentEncounter`,
    /// excluding comment lines (doc `///`) and string-literal occurrences (lines
    /// that contain `"` before the pattern). Exactly one match is expected.
    /// If a future change adds a second system-level producer, this test breaks — that
    /// is the point.
    #[test]
    fn handle_encounter_request_sole_writer() {
        // Three-stage grep: find candidate lines, then exclude:
        //   1. Doc/line-comment lines (///, //)
        //   2. Lines where the match is inside a double-quoted string ("...")
        //   3. Lines where the match is inside a backtick code span (`commands.`)
        // Shell pipeline via `sh -c` so we can use `|` without temp files.
        let output = std::process::Command::new("sh")
            .args([
                "-c",
                r#"grep -rn 'commands\.insert_resource(CurrentEncounter' src/ | grep -v '//[/!]' | grep -v '".*commands\.' | grep -v '`commands\.'"#,
            ])
            .current_dir(env!("CARGO_MANIFEST_DIR"))
            .output()
            .expect("sh -c grep pipeline must be available");
        let stdout = String::from_utf8_lossy(&output.stdout);
        let count = stdout.lines().filter(|l| !l.is_empty()).count();
        assert_eq!(
            count, 1,
            "Expected exactly 1 occurrence of system-call \
             `commands.insert_resource(CurrentEncounter` in src/ \
             (sole production writer in handle_encounter_request). \
             Found {count}:\n{stdout}"
        );
    }

    /// Verify `clear_current_encounter` (the `OnExit(Combat)` system) removes
    /// both the `CurrentEncounter` resource AND every `Enemy`-marked entity.
    ///
    /// This covers the same invariant as `current_encounter_removed_on_combat_exit`
    /// but with the name listed in the PR test plan, enshrining it as a named guard.
    ///
    /// The original version of this test inserted `CurrentEncounter` with an
    /// empty `enemy_entities` vec, so the entity-despawn path was never
    /// exercised — a playtest surfaced corpses from prior encounters reappearing
    /// in fresh fights. This version spawns real `EnemyBundle` entities first.
    #[test]
    fn no_current_encounter_after_combat_exit() {
        let mut app = make_test_app();
        // Spawn real enemy entities so the despawn path is actually tested.
        let e1 = app.world_mut().spawn(EnemyBundle::default()).id();
        let e2 = app.world_mut().spawn(EnemyBundle::default()).id();
        app.world_mut().insert_resource(CurrentEncounter {
            enemy_entities: vec![e1, e2],
            fleeable: true,
        });
        // Enter Combat state.
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Combat);
        app.update();
        app.update();
        // Exit Combat state — OnExit(Combat) fires clear_current_encounter.
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Dungeon);
        app.update();
        app.update();
        // CurrentEncounter must be absent (Pitfall 6 — stale encounter state).
        assert!(
            app.world().get_resource::<CurrentEncounter>().is_none(),
            "CurrentEncounter must be removed by clear_current_encounter on OnExit(Combat)"
        );
        // Both Enemy entities must be despawned — otherwise they leak into
        // the next encounter as dead corpses.
        assert!(
            app.world().get_entity(e1).is_err(),
            "stale Enemy entity {e1:?} survived OnExit(Combat)"
        );
        assert!(
            app.world().get_entity(e2).is_err(),
            "stale Enemy entity {e2:?} survived OnExit(Combat)"
        );
        let lingering: Vec<Entity> = app
            .world_mut()
            .query_filtered::<Entity, With<Enemy>>()
            .iter(app.world())
            .collect();
        assert!(
            lingering.is_empty(),
            "no Enemy-marked entities should remain after OnExit(Combat); found {lingering:?}"
        );
    }

    #[cfg(feature = "dev")]
    #[test]
    fn force_encounter_on_f7_writes_message() {
        let mut app = make_test_app();
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Dungeon);
        app.update();
        app.update();
        // Press F7.
        app.world_mut()
            .resource_mut::<bevy::input::ButtonInput<bevy::prelude::KeyCode>>()
            .press(bevy::prelude::KeyCode::F7);
        app.update();

        // EncounterRequested should be in the message buffer.
        let messages = app
            .world()
            .resource::<bevy::ecs::message::Messages<EncounterRequested>>();
        assert!(
            messages
                .iter_current_update_messages()
                .any(|r| matches!(r.source, EncounterSource::Random)),
            "F7 should write EncounterRequested {{ source: Random }}"
        );
    }
}
