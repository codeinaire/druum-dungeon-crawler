//! Dungeon exploration plugin — Feature #7: grid movement + first-person camera.
//!
//! ## Role
//!
//! This module is the primary home for Feature #7's end-to-end gameplay loop:
//! - `PlayerParty` entity with child `Camera3d` at eye height.
//! - `OnEnter(GameState::Dungeon)` spawn at `floor.entry_point`.
//! - `OnExit(GameState::Dungeon)` despawn (recursive — children cleaned up automatically).
//! - `handle_dungeon_input` translates `Res<ActionState<DungeonAction>>` into grid
//!   moves gated by `DungeonFloor::can_move`.
//! - `MovementAnimation` tween component (0.18s translate / 0.15s rotate, smoothstep).
//! - `MovedEvent` message emitted for each committed translation move.
//! - `SfxRequest::Footstep` dispatched for each committed translation move.
//!
//! ## World coordinate convention (source-of-truth)
//!
//! `world_x = grid_x * CELL_SIZE`, `world_z = grid_y * CELL_SIZE`, `world_y = 0.0`.
//! `+grid_y → +world_z`. With `Direction::North = (0, -1)` (y-down grid convention
//! from `src/data/dungeon.rs`), North movement decrements `grid_y` → `-Z` world motion
//! → matches Bevy's default camera-looking direction. A camera facing North uses
//! `Quat::IDENTITY`. The `data/dungeon.rs:18` doc-comment that says `-grid_y` is
//! forward-looking conjecture from Feature #4 that this module supersedes; do NOT
//! modify `data/dungeon.rs`.
//!
//! ## Logical vs visual state
//!
//! `GridPosition` and `Facing` update **immediately** on input-commit (same frame).
//! `MovementAnimation` then lerps the visual `Transform` over the tween duration.
//! Downstream consumers (#13 cell-trigger, #16 encounter) react to the new logical
//! state on the commit frame, not after the tween completes.

use bevy::prelude::*;
use leafwing_input_manager::prelude::ActionState;

use crate::data::DungeonFloor;
use crate::data::dungeon::Direction;
use crate::plugins::audio::{SfxKind, SfxRequest};
use crate::plugins::input::DungeonAction;
use crate::plugins::loading::DungeonAssets;
use crate::plugins::state::{DungeonSubState, GameState};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// World units per grid cell. Genre-correct corridor scale; 3.0 wall height
/// (Feature #8) at 2.0 cell size gives a 1.5× wall-to-corridor ratio.
pub const CELL_SIZE: f32 = 2.0;

/// Local Y offset of the `Camera3d` child relative to the `PlayerParty` root.
/// 0.7 produces a "forward-facing" feel relative to Feature #8's planned 3.0
/// wall height.
pub const EYE_HEIGHT: f32 = 0.7;

/// Duration of a forward/backward/strafe translation tween (seconds).
pub const MOVE_DURATION_SECS: f32 = 0.18;

/// Duration of a turn-left/turn-right rotation tween (seconds).
pub const TURN_DURATION_SECS: f32 = 0.15;

// ---------------------------------------------------------------------------
// Components
// ---------------------------------------------------------------------------

/// Zero-sized marker on the `PlayerParty` root entity. Carries
/// `GridPosition + Facing` as the authoritative logical state.
#[derive(Component, Debug, Clone, Copy)]
pub struct PlayerParty;

/// Zero-sized marker on the child `Camera3d` entity spawned under
/// `PlayerParty`. Used for queries that need to target the camera
/// specifically without touching the parent.
#[derive(Component, Debug, Clone, Copy)]
pub struct DungeonCamera;

/// Logical grid position. Updated immediately on input-commit; the visual
/// `Transform` catches up via `MovementAnimation`.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct GridPosition {
    pub x: u32,
    pub y: u32,
}

/// Logical facing direction. Updated immediately on input-commit; the visual
/// rotation catches up via `MovementAnimation`.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Facing(pub Direction);

/// In-flight animation state. Carries both translation and rotation fields in
/// a single component (master research §Pattern 4) so a single `Without<MovementAnimation>`
/// filter doubles as the input-drop gate. When `from_translation == to_translation`
/// (turn-only) the translation lerp is a no-op. When `from_rotation == to_rotation`
/// (translation-only) the rotation slerp is a no-op.
///
/// Component is removed by `animate_movement` when `elapsed_secs >= duration_secs`.
/// On removal, `Transform` is snapped to the exact destination to avoid float drift.
///
/// Mirrors the `FadeIn`/`FadeOut` lifecycle in `src/plugins/audio/bgm.rs:36-67`.
#[derive(Component, Debug, Clone)]
pub struct MovementAnimation {
    pub from_translation: Vec3,
    pub to_translation: Vec3,
    pub from_rotation: Quat,
    pub to_rotation: Quat,
    pub elapsed_secs: f32,
    pub duration_secs: f32,
}

impl MovementAnimation {
    /// Construct an animation for a forward/backward/strafe move.
    /// Rotation does not change; `from_rotation == to_rotation == rotation`.
    pub fn translate(from: Vec3, to: Vec3, rotation: Quat) -> Self {
        Self {
            from_translation: from,
            to_translation: to,
            from_rotation: rotation,
            to_rotation: rotation,
            elapsed_secs: 0.0,
            duration_secs: MOVE_DURATION_SECS,
        }
    }

    /// Construct an animation for a turn-left/turn-right rotate.
    /// Translation does not change; `from_translation == to_translation == translation`.
    pub fn rotate(translation: Vec3, from_rot: Quat, to_rot: Quat) -> Self {
        Self {
            from_translation: translation,
            to_translation: translation,
            from_rotation: from_rot,
            to_rotation: to_rot,
            elapsed_secs: 0.0,
            duration_secs: TURN_DURATION_SECS,
        }
    }
}

/// Marker tag on every entity spawned by `spawn_test_scene`.
/// Feature #8 deletes the entire `spawn_test_scene` function, this component,
/// and the `despawn_dungeon_entities` query for it in one PR.
#[derive(Component, Debug, Clone, Copy)]
pub struct TestSceneMarker;

// ---------------------------------------------------------------------------
// Messages
// ---------------------------------------------------------------------------

/// Emitted once per committed **translation** move (Forward/Backward/Strafe).
/// Turn-only moves and wall-bumps emit nothing.
///
/// Consumers: Feature #13 (cell triggers), Feature #16 (encounter rolls).
/// Includes post-move `facing` so downstream can react to orientation without
/// a follow-up query.
///
/// Derives `Message`, NOT `Event` — Bevy 0.18 family rename. Read with
/// `MessageReader<MovedEvent>`, register with `app.add_message::<MovedEvent>()`.
#[derive(Message, Clone, Copy, Debug)]
pub struct MovedEvent {
    pub from: GridPosition,
    pub to: GridPosition,
    pub facing: Direction,
}

// ---------------------------------------------------------------------------
// Plugin
// ---------------------------------------------------------------------------

pub struct DungeonPlugin;

impl Plugin for DungeonPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<MovedEvent>()
            .add_systems(
                OnEnter(GameState::Dungeon),
                (spawn_party_and_camera, spawn_test_scene),
            )
            .add_systems(OnExit(GameState::Dungeon), despawn_dungeon_entities)
            .add_systems(
                Update,
                (
                    handle_dungeon_input
                        .run_if(
                            in_state(GameState::Dungeon).and(in_state(DungeonSubState::Exploring)),
                        )
                        .before(animate_movement),
                    animate_movement.run_if(in_state(GameState::Dungeon)),
                ),
            );
    }
}

// ---------------------------------------------------------------------------
// Pure helpers
// ---------------------------------------------------------------------------

/// Convert a logical `GridPosition` to a Bevy world-space `Vec3`.
///
/// Convention: `world_x = grid_x * CELL_SIZE`, `world_z = grid_y * CELL_SIZE`,
/// `world_y = 0.0`. With `Direction::North = (0, -1)`, North movement
/// decrements `grid_y` → `-Z` world motion → matches Bevy's default camera
/// orientation (looking toward -Z). A camera facing North uses `Quat::IDENTITY`.
fn grid_to_world(pos: GridPosition) -> Vec3 {
    Vec3::new(pos.x as f32 * CELL_SIZE, 0.0, pos.y as f32 * CELL_SIZE)
}

/// Convert a `Direction` to a `Quat` representing the world-space Y-rotation
/// for a camera looking in that direction.
///
/// Right-hand rule: positive angles are counter-clockwise when viewed from +Y.
/// - `North → 0.0` (looks toward -Z, Bevy default)
/// - `East  → -π/2` (clockwise 90° when viewed from above; turning right from North)
/// - `South → π`
/// - `West  → +π/2`
fn facing_to_quat(facing: Direction) -> Quat {
    use std::f32::consts::{FRAC_PI_2, PI};
    let angle = match facing {
        Direction::North => 0.0,
        Direction::East => -FRAC_PI_2,
        Direction::South => PI,
        Direction::West => FRAC_PI_2,
    };
    Quat::from_rotation_y(angle)
}

// ---------------------------------------------------------------------------
// Systems
// ---------------------------------------------------------------------------

/// `OnEnter(GameState::Dungeon)` — spawn the `PlayerParty` root entity at
/// `floor.entry_point`, with a child `Camera3d` at eye height.
///
/// Tolerant of missing assets: logs a warning and returns silently if
/// `DungeonAssets` or the floor handle is not yet ready (e.g., if F9 forces
/// the Dungeon state before loading completes).
fn spawn_party_and_camera(
    mut commands: Commands,
    dungeon_assets: Option<Res<DungeonAssets>>,
    floors: Res<Assets<DungeonFloor>>,
) {
    let Some(assets) = dungeon_assets else {
        warn!("DungeonAssets resource not present at OnEnter(Dungeon); party spawn deferred");
        return;
    };
    let Some(floor) = floors.get(&assets.floor_01) else {
        warn!("DungeonFloor not yet loaded; party spawn deferred");
        return;
    };

    let (sx, sy, facing) = floor.entry_point;
    let start_pos = GridPosition { x: sx, y: sy };
    let world_pos = grid_to_world(start_pos);
    let initial_rotation = facing_to_quat(facing);

    commands.spawn((
        GridPosition { x: sx, y: sy },
        Facing(facing),
        Transform::from_translation(world_pos).with_rotation(initial_rotation),
        Visibility::default(),
        PlayerParty,
        children![(
            Camera3d::default(),
            Transform::from_xyz(0.0, EYE_HEIGHT, 0.0),
            DungeonCamera,
        )],
    ));

    info!(
        "Spawned PlayerParty at grid ({}, {}) facing {:?}",
        sx, sy, facing
    );
}

/// `OnEnter(GameState::Dungeon)` — spawn placeholder test-scene geometry so
/// camera movement is visually verifiable.
///
/// Spawns 3 colored cubes relative to `floor_01.entry_point = (1, 1, North)`:
/// - Red: 2 cells north of entry (directly in front of the initial camera view).
/// - Blue: 2 cells east of entry.
/// - Green: 2 cells west of entry.
///
/// Plus a 40×0.1×40 grey ground slab and a `DirectionalLight`.
/// All entities carry `TestSceneMarker` for one-PR cleanup in Feature #8.
///
/// TODO(Feature #8): delete this entire function — replaced by real wall geometry.
fn spawn_test_scene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Entry point: grid (1, 1) → world (2.0, 0.0, 2.0), facing North (-Z).
    // "In front" = North = decreasing world Z from entry.
    let cube_mesh = meshes.add(Cuboid::new(1.0, 1.0, 1.0));

    // Red cube — directly north of entry, two cells ahead along initial view.
    commands.spawn((
        Mesh3d(cube_mesh.clone()),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.8, 0.1, 0.1),
            ..default()
        })),
        Transform::from_xyz(2.0, 0.5, 2.0 - CELL_SIZE * 2.0),
        TestSceneMarker,
    ));

    // Blue cube — east of entry.
    commands.spawn((
        Mesh3d(cube_mesh.clone()),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.1, 0.1, 0.8),
            ..default()
        })),
        Transform::from_xyz(2.0 + CELL_SIZE * 2.0, 0.5, 2.0),
        TestSceneMarker,
    ));

    // Green cube — west of entry.
    commands.spawn((
        Mesh3d(cube_mesh),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.1, 0.7, 0.1),
            ..default()
        })),
        Transform::from_xyz(2.0 - CELL_SIZE * 2.0, 0.5, 2.0),
        TestSceneMarker,
    ));

    // Ground slab: 40×0.1×40, centered at world origin, y=-0.05 so the top
    // face is at y=0 (flush with the player's feet).
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(40.0, 0.1, 40.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.3, 0.3, 0.3),
            ..default()
        })),
        Transform::from_xyz(0.0, -0.05, 0.0),
        TestSceneMarker,
    ));

    // Directional light — 45° down-and-diagonal for differential corner shading.
    commands.spawn((
        DirectionalLight {
            illuminance: 3000.0,
            shadows_enabled: false,
            ..default()
        },
        Transform::from_xyz(0.0, 5.0, 0.0).looking_at(Vec3::new(1.0, -1.0, 1.0), Vec3::Y),
        TestSceneMarker,
    ));
}

/// `OnExit(GameState::Dungeon)` — despawn all `PlayerParty` entities
/// (recursive — child cameras are cleaned up automatically) and all
/// `TestSceneMarker` entities.
fn despawn_dungeon_entities(
    mut commands: Commands,
    parties: Query<Entity, With<PlayerParty>>,
    test_scene: Query<Entity, With<TestSceneMarker>>,
) {
    for e in &parties {
        commands.entity(e).despawn();
    }
    for e in &test_scene {
        commands.entity(e).despawn();
    }
    info!("Despawned PlayerParty + test scene entities on OnExit(Dungeon)");
}

/// `Update` (gated on `GameState::Dungeon && DungeonSubState::Exploring`) —
/// translate `ActionState<DungeonAction>` input into grid moves.
///
/// **Input-drop policy:** the query filter `Without<MovementAnimation>` means
/// any key press during an in-flight tween is silently dropped. No buffer.
///
/// **Logical-first:** `GridPosition` and `Facing` are written to their final
/// values on the commit frame; `MovementAnimation` lerps the visual `Transform`
/// afterward. Downstream consumers (#13, #16) see the new logical state
/// immediately, not after the tween completes.
///
/// Tolerant of missing `DungeonAssets` / unloaded `DungeonFloor`.
#[allow(clippy::type_complexity)]
fn handle_dungeon_input(
    mut commands: Commands,
    actions: Res<ActionState<DungeonAction>>,
    dungeon_assets: Option<Res<DungeonAssets>>,
    floors: Res<Assets<DungeonFloor>>,
    mut sfx: MessageWriter<SfxRequest>,
    mut moved: MessageWriter<MovedEvent>,
    mut query: Query<
        (Entity, &mut GridPosition, &mut Facing, &Transform),
        (With<PlayerParty>, Without<MovementAnimation>),
    >,
) {
    // Bail if there is no PlayerParty, or if assets are missing.
    let Ok((entity, mut pos, mut facing, transform)) = query.single_mut() else {
        return;
    };
    let Some(assets) = dungeon_assets else {
        return;
    };
    let Some(floor) = floors.get(&assets.floor_01) else {
        return;
    };

    // --- Translation actions ---
    let maybe_translation_dir: Option<Direction> =
        if actions.just_pressed(&DungeonAction::MoveForward) {
            Some(facing.0)
        } else if actions.just_pressed(&DungeonAction::MoveBackward) {
            Some(facing.0.reverse())
        } else if actions.just_pressed(&DungeonAction::StrafeLeft) {
            Some(facing.0.turn_left())
        } else if actions.just_pressed(&DungeonAction::StrafeRight) {
            Some(facing.0.turn_right())
        } else {
            None
        };

    if let Some(move_dir) = maybe_translation_dir {
        let (dx, dy) = move_dir.offset();
        let new_x_i32 = pos.x as i32 + dx;
        let new_y_i32 = pos.y as i32 + dy;

        // Bounds check: reject negative coordinates before casting.
        if new_x_i32 < 0 || new_y_i32 < 0 {
            return;
        }

        // Passability check: wall-bumps are silent no-ops.
        if !floor.can_move(pos.x, pos.y, move_dir) {
            return;
        }

        let old_pos = *pos;
        pos.x = new_x_i32 as u32;
        pos.y = new_y_i32 as u32;

        let from_translation = grid_to_world(old_pos);
        let to_translation = grid_to_world(*pos);

        commands.entity(entity).insert(MovementAnimation::translate(
            from_translation,
            to_translation,
            transform.rotation,
        ));

        sfx.write(SfxRequest {
            kind: SfxKind::Footstep,
        });
        moved.write(MovedEvent {
            from: old_pos,
            to: *pos,
            facing: facing.0,
        });
        return;
    }

    // --- Turn actions ---
    // Note: turn-SFX deferred to Feature #25 — no SfxKind::Turn variant exists
    // in this feature; adding one would touch frozen src/plugins/audio/sfx.rs.
    if actions.just_pressed(&DungeonAction::TurnLeft) {
        let old_facing = facing.0;
        facing.0 = old_facing.turn_left();
        let from_rotation = transform.rotation;
        let to_rotation = facing_to_quat(facing.0);
        commands.entity(entity).insert(MovementAnimation::rotate(
            transform.translation,
            from_rotation,
            to_rotation,
        ));
    } else if actions.just_pressed(&DungeonAction::TurnRight) {
        let old_facing = facing.0;
        facing.0 = old_facing.turn_right();
        let from_rotation = transform.rotation;
        let to_rotation = facing_to_quat(facing.0);
        commands.entity(entity).insert(MovementAnimation::rotate(
            transform.translation,
            from_rotation,
            to_rotation,
        ));
    }
}

/// `Update` (gated on `GameState::Dungeon`, no SubState gate so tweens
/// complete even if the player opens inventory mid-step) — advance
/// `MovementAnimation` each frame, lerping the visual `Transform`.
///
/// Smoothstep: `t = raw_t² × (3 − 2×raw_t)`.
/// When the tween completes, the transform is snapped to the exact destination
/// to avoid float drift, and `MovementAnimation` is removed.
fn animate_movement(
    mut commands: Commands,
    time: Res<Time>,
    mut query: Query<(Entity, &mut Transform, &mut MovementAnimation)>,
) {
    for (entity, mut transform, mut anim) in &mut query {
        anim.elapsed_secs += time.delta_secs();
        let t_raw = (anim.elapsed_secs / anim.duration_secs).clamp(0.0, 1.0);
        // Smoothstep
        let t = t_raw * t_raw * (3.0 - 2.0 * t_raw);

        transform.translation = anim.from_translation.lerp(anim.to_translation, t);
        transform.rotation = anim.from_rotation.slerp(anim.to_rotation, t);

        if t_raw >= 1.0 {
            // Snap to exact destination to avoid float drift.
            transform.translation = anim.to_translation;
            transform.rotation = anim.to_rotation;
            commands.entity(entity).remove::<MovementAnimation>();
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    use bevy::input::InputPlugin;
    use bevy::state::app::StatesPlugin;
    use bevy::time::TimeUpdateStrategy;
    use leafwing_input_manager::user_input::Buttonlike;

    use crate::plugins::input::ActionsPlugin;
    use crate::plugins::state::StatePlugin;

    // -----------------------------------------------------------------------
    // Pure-function unit tests (no App)
    // -----------------------------------------------------------------------

    #[test]
    fn grid_to_world_origin_is_zero() {
        assert_eq!(grid_to_world(GridPosition { x: 0, y: 0 }), Vec3::ZERO);
    }

    #[test]
    fn grid_to_world_x_axis() {
        assert_eq!(
            grid_to_world(GridPosition { x: 3, y: 0 }),
            Vec3::new(6.0, 0.0, 0.0)
        );
    }

    #[test]
    fn grid_to_world_z_axis_positive() {
        assert_eq!(
            grid_to_world(GridPosition { x: 0, y: 4 }),
            Vec3::new(0.0, 0.0, 8.0)
        );
    }

    #[test]
    fn facing_to_quat_north_is_identity() {
        assert!(
            facing_to_quat(Direction::North).abs_diff_eq(Quat::IDENTITY, 1e-6),
            "North should map to Quat::IDENTITY"
        );
    }

    #[test]
    fn facing_to_quat_east_is_minus_quarter_y() {
        assert!(
            facing_to_quat(Direction::East)
                .abs_diff_eq(Quat::from_rotation_y(-std::f32::consts::FRAC_PI_2), 1e-6),
            "East should map to -π/2 Y rotation"
        );
    }

    #[test]
    fn facing_to_quat_south_is_pi_y() {
        assert!(
            facing_to_quat(Direction::South)
                .abs_diff_eq(Quat::from_rotation_y(std::f32::consts::PI), 1e-6),
            "South should map to π Y rotation"
        );
    }

    #[test]
    fn facing_to_quat_west_is_quarter_y() {
        assert!(
            facing_to_quat(Direction::West)
                .abs_diff_eq(Quat::from_rotation_y(std::f32::consts::FRAC_PI_2), 1e-6),
            "West should map to +π/2 Y rotation"
        );
    }

    // -----------------------------------------------------------------------
    // Test app helpers
    // -----------------------------------------------------------------------

    /// Build a minimal test app with the full dungeon plugin chain.
    /// Duplicates the pattern from src/plugins/input/mod.rs and
    /// src/plugins/audio/mod.rs tests.
    ///
    /// `spawn_test_scene` requires `Assets<Mesh>` and `Assets<StandardMaterial>`.
    /// These are normally registered by `MeshPlugin` / `PbrPlugin` (both pulled
    /// in via `DefaultPlugins` at runtime). In tests we init them explicitly so
    /// the minimal app can run `OnEnter(GameState::Dungeon)` without panicking.
    fn make_test_app() -> App {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            bevy::asset::AssetPlugin::default(),
            StatesPlugin,
            InputPlugin,
            StatePlugin,
            ActionsPlugin,
            DungeonPlugin,
        ));
        app.init_asset::<DungeonFloor>();
        // Init mesh and material asset registries so spawn_test_scene's
        // ResMut<Assets<Mesh>> and ResMut<Assets<StandardMaterial>> parameters
        // are satisfied without the full PbrPlugin chain.
        app.init_asset::<Mesh>();
        app.init_asset::<StandardMaterial>();
        // Register SfxRequest messages. In production, AudioPlugin does this.
        // In tests we only have DungeonPlugin (which registers MovedEvent) but
        // handle_dungeon_input also writes SfxRequest — so the message type must
        // be initialized before the system can run.
        app.add_message::<SfxRequest>();
        // Required because StatePlugin under --features dev registers
        // cycle_game_state_on_f9 which needs ButtonInput<KeyCode> at update time.
        // Same pattern as src/plugins/state/mod.rs:107, audio/mod.rs:174,
        // input/mod.rs:174. Third-feature gotcha now confirmed across #2/#5/#6.
        #[cfg(feature = "dev")]
        app.init_resource::<bevy::input::ButtonInput<bevy::prelude::KeyCode>>();
        app
    }

    /// Build a minimal 3×3 open `DungeonFloor`. Duplicates Feature #4's
    /// `make_floor` (which is `#[cfg(test)]`-private to `data/dungeon.rs`).
    /// ~20 LOC of duplication is cheaper than refactoring a frozen file.
    fn make_open_floor(w: u32, h: u32) -> DungeonFloor {
        use crate::data::dungeon::{CellFeatures, WallMask};
        DungeonFloor {
            name: "test".into(),
            width: w,
            height: h,
            floor_number: 1,
            walls: vec![vec![WallMask::default(); w as usize]; h as usize],
            features: vec![vec![CellFeatures::default(); w as usize]; h as usize],
            entry_point: (1, 1, Direction::North),
            encounter_table: "test_table".into(),
        }
    }

    /// Insert a `DungeonFloor` into the app's `Assets<DungeonFloor>` and
    /// set a `DungeonAssets` resource pointing to it (and default handles
    /// for the other assets).
    fn insert_test_floor(app: &mut App, floor: DungeonFloor) {
        let handle = app
            .world_mut()
            .resource_mut::<Assets<DungeonFloor>>()
            .add(floor);
        app.world_mut().insert_resource(DungeonAssets {
            floor_01: handle,
            item_db: Handle::default(),
            enemy_db: Handle::default(),
            class_table: Handle::default(),
            spell_table: Handle::default(),
        });
    }

    /// Transition the app into `GameState::Dungeon` and pump two frames
    /// (one-frame state-transition deferral, same pattern as Feature #6 BGM tests).
    fn advance_into_dungeon(app: &mut App) {
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Dungeon);
        app.update(); // StateTransition schedule realises the new state
        app.update(); // OnEnter(Dungeon) systems run; party + test scene spawned
    }

    // -----------------------------------------------------------------------
    // Input / movement tests
    // -----------------------------------------------------------------------

    /// Forward move: W key advances GridPosition north (y decrements by 1).
    #[test]
    fn handle_dungeon_input_moves_forward_one_cell() {
        let mut app = make_test_app();
        insert_test_floor(&mut app, make_open_floor(3, 3));
        advance_into_dungeon(&mut app);

        // Assert party spawned at entry point (1, 1).
        let pos = *app
            .world_mut()
            .query_filtered::<&GridPosition, With<PlayerParty>>()
            .single(app.world())
            .unwrap();
        assert_eq!(pos, GridPosition { x: 1, y: 1 });

        // Inject W (MoveForward).
        KeyCode::KeyW.press(app.world_mut());
        app.update();

        // GridPosition should now be (1, 0) — one cell north.
        let pos = *app
            .world_mut()
            .query_filtered::<&GridPosition, With<PlayerParty>>()
            .single(app.world())
            .unwrap();
        assert_eq!(
            pos,
            GridPosition { x: 1, y: 0 },
            "MoveForward should decrement y by 1"
        );

        // MovementAnimation should be present (tween in flight).
        let has_anim = app
            .world_mut()
            .query_filtered::<&MovementAnimation, With<PlayerParty>>()
            .iter(app.world())
            .count()
            > 0;
        assert!(has_anim, "MovementAnimation should be inserted on move");

        // MovedEvent should have been written.
        let moved_count = app
            .world()
            .resource::<bevy::ecs::message::Messages<MovedEvent>>()
            .iter_current_update_messages()
            .count();
        assert_eq!(
            moved_count, 1,
            "MovedEvent should be written on translation move"
        );

        // SfxRequest::Footstep should have been written.
        let sfx_count = app
            .world()
            .resource::<bevy::ecs::message::Messages<SfxRequest>>()
            .iter_current_update_messages()
            .count();
        assert_eq!(
            sfx_count, 1,
            "SfxRequest should be written on translation move"
        );
    }

    /// Wall bump: W key against a Solid north wall does nothing.
    #[test]
    fn handle_dungeon_input_blocked_by_wall() {
        use crate::data::dungeon::WallType;

        let mut floor = make_open_floor(3, 3);
        // Add a Solid north wall on cell (1, 1).
        floor.walls[1][1].north = WallType::Solid;

        let mut app = make_test_app();
        insert_test_floor(&mut app, floor);
        advance_into_dungeon(&mut app);

        // Inject W.
        KeyCode::KeyW.press(app.world_mut());
        app.update();

        // GridPosition should be unchanged.
        let pos = *app
            .world_mut()
            .query_filtered::<&GridPosition, With<PlayerParty>>()
            .single(app.world())
            .unwrap();
        assert_eq!(
            pos,
            GridPosition { x: 1, y: 1 },
            "Wall bump should not change position"
        );

        // No MovementAnimation.
        let anim_count = app
            .world_mut()
            .query_filtered::<&MovementAnimation, With<PlayerParty>>()
            .iter(app.world())
            .count();
        assert_eq!(
            anim_count, 0,
            "Wall bump should not insert MovementAnimation"
        );

        // No MovedEvent.
        let moved_count = app
            .world()
            .resource::<bevy::ecs::message::Messages<MovedEvent>>()
            .iter_current_update_messages()
            .count();
        assert_eq!(moved_count, 0, "Wall bump should not emit MovedEvent");

        // No SfxRequest.
        let sfx_count = app
            .world()
            .resource::<bevy::ecs::message::Messages<SfxRequest>>()
            .iter_current_update_messages()
            .count();
        assert_eq!(sfx_count, 0, "Wall bump should not emit SfxRequest");
    }

    /// Turn left: Q key rotates Facing counter-clockwise (North → West).
    #[test]
    fn handle_dungeon_input_turn_left_rotates_facing() {
        let mut app = make_test_app();
        insert_test_floor(&mut app, make_open_floor(3, 3));
        advance_into_dungeon(&mut app);

        // Inject Q (TurnLeft).
        KeyCode::KeyQ.press(app.world_mut());
        app.update();

        // Facing should be West.
        let facing = *app
            .world_mut()
            .query_filtered::<&Facing, With<PlayerParty>>()
            .single(app.world())
            .unwrap();
        assert_eq!(
            facing.0,
            Direction::West,
            "TurnLeft from North should yield West"
        );

        // MovementAnimation should be present.
        let has_anim = app
            .world_mut()
            .query_filtered::<&MovementAnimation, With<PlayerParty>>()
            .iter(app.world())
            .count()
            > 0;
        assert!(has_anim, "TurnLeft should insert MovementAnimation");

        // No MovedEvent.
        let moved_count = app
            .world()
            .resource::<bevy::ecs::message::Messages<MovedEvent>>()
            .iter_current_update_messages()
            .count();
        assert_eq!(moved_count, 0, "TurnLeft should NOT emit MovedEvent");

        // No SfxRequest.
        let sfx_count = app
            .world()
            .resource::<bevy::ecs::message::Messages<SfxRequest>>()
            .iter_current_update_messages()
            .count();
        assert_eq!(sfx_count, 0, "TurnLeft should NOT emit SfxRequest");
    }

    /// Strafe right: D key moves east (x+1) while facing stays North.
    #[test]
    fn handle_dungeon_input_strafe_perpendicular_to_facing() {
        let mut app = make_test_app();
        insert_test_floor(&mut app, make_open_floor(3, 3));
        advance_into_dungeon(&mut app);

        // Inject D (StrafeRight — moves East when facing North).
        KeyCode::KeyD.press(app.world_mut());
        app.update();

        let pos = *app
            .world_mut()
            .query_filtered::<&GridPosition, With<PlayerParty>>()
            .single(app.world())
            .unwrap();
        assert_eq!(
            pos,
            GridPosition { x: 2, y: 1 },
            "StrafeRight should increase x by 1"
        );

        let facing = *app
            .world_mut()
            .query_filtered::<&Facing, With<PlayerParty>>()
            .single(app.world())
            .unwrap();
        assert_eq!(
            facing.0,
            Direction::North,
            "StrafeRight should not change facing"
        );
    }

    /// Input drop: pressing W during an in-flight animation is silently dropped.
    #[test]
    fn handle_dungeon_input_drops_input_during_animation() {
        let mut app = make_test_app();
        insert_test_floor(&mut app, make_open_floor(5, 5));
        // Use a 3×3 floor but with entry at (1,1) and 5×5 to have room.
        // Re-insert a 5×5 floor with entry (1,1).
        {
            use crate::data::dungeon::{CellFeatures, WallMask};
            let floor = DungeonFloor {
                name: "test5x5".into(),
                width: 5,
                height: 5,
                floor_number: 1,
                walls: vec![vec![WallMask::default(); 5]; 5],
                features: vec![vec![CellFeatures::default(); 5]; 5],
                entry_point: (2, 2, Direction::North),
                encounter_table: "test_table".into(),
            };
            insert_test_floor(&mut app, floor);
        }
        advance_into_dungeon(&mut app);

        // First W — should move.
        KeyCode::KeyW.press(app.world_mut());
        app.update();

        let pos_after_first = *app
            .world_mut()
            .query_filtered::<&GridPosition, With<PlayerParty>>()
            .single(app.world())
            .unwrap();
        // Entry was (2,2), moved north → (2,1).
        assert_eq!(pos_after_first, GridPosition { x: 2, y: 1 });

        // Animation should be in flight.
        let has_anim = app
            .world_mut()
            .query_filtered::<&MovementAnimation, With<PlayerParty>>()
            .iter(app.world())
            .count()
            > 0;
        assert!(has_anim, "Animation should be in flight after first move");

        // Second W during animation — should be dropped.
        KeyCode::KeyW.press(app.world_mut());
        app.update();

        let pos_after_second = *app
            .world_mut()
            .query_filtered::<&GridPosition, With<PlayerParty>>()
            .single(app.world())
            .unwrap();
        assert_eq!(
            pos_after_second, pos_after_first,
            "Second W during animation should be dropped"
        );
    }

    /// Animation completes: after enough frames, `MovementAnimation` is removed
    /// and `Transform` snaps to the destination.
    #[test]
    fn animate_movement_completes_in_duration_secs() {
        let mut app = make_test_app();
        // Use ManualDuration so we control elapsed time precisely.
        app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
            50,
        )));

        // Advance into Dungeon state so `animate_movement.run_if(in_state(GameState::Dungeon))`
        // actually fires. No DungeonAssets needed because spawn_party_and_camera
        // returns early (None asset) without panicking.
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Dungeon);
        app.update(); // state transition
        app.update(); // OnEnter systems run (party spawn deferred — no assets)

        // Spawn a bare PlayerParty entity with a MovementAnimation.
        let from = Vec3::ZERO;
        let to = Vec3::new(2.0, 0.0, 0.0);
        app.world_mut().spawn((
            PlayerParty,
            Transform::from_translation(from),
            Visibility::default(),
            MovementAnimation::translate(from, to, Quat::IDENTITY),
        ));

        // 50ms × 5 = 250ms > 180ms (MOVE_DURATION_SECS). Tween should complete.
        for _ in 0..5 {
            app.update();
        }

        let transform = *app
            .world_mut()
            .query_filtered::<&Transform, With<PlayerParty>>()
            .single(app.world())
            .unwrap();
        assert_eq!(
            transform.translation, to,
            "Transform should snap to destination after tween completes"
        );

        let has_anim = app
            .world_mut()
            .query_filtered::<&MovementAnimation, With<PlayerParty>>()
            .iter(app.world())
            .count()
            > 0;
        assert!(
            !has_anim,
            "MovementAnimation should be removed after tween completes"
        );
    }
}
