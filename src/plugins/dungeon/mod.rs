//! Dungeon exploration plugin — Feature #7: grid movement + first-person camera;
//! Feature #8: 3D dungeon geometry renderer.
//!
//! ## Role
//!
//! This module is the primary home for the dungeon gameplay loop:
//! - `PlayerParty` entity with child `Camera3d` at eye height.
//! - A `PointLight` child of `DungeonCamera` — Wizardry-style torchlight: dark
//!   perimeter, walls fade with distance, oppressive atmosphere. The light follows
//!   the player automatically because it is a child of `DungeonCamera`.
//! - `OnEnter(GameState::Dungeon)` spawn at `floor.entry_point` + real dungeon
//!   geometry (floor/ceiling slabs + wall plates from `DungeonFloor::walls`).
//! - `OnExit(GameState::Dungeon)` despawn (recursive for children; `DungeonGeometry`
//!   tag for standalone geometry entities).
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
//! `Quat::IDENTITY`.
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
use crate::data::dungeon::{Direction, WallType};
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
/// 1.8 places the eye at 3/5 of the 3.0 cell height — natural human-eye-level
/// for a 1st-person crawler. Tuned during Feature #8 manual smoke (originally
/// 0.7 felt waist-level).
pub const EYE_HEIGHT: f32 = 1.8;

/// Duration of a forward/backward/strafe translation tween (seconds).
pub const MOVE_DURATION_SECS: f32 = 0.18;

/// Duration of a turn-left/turn-right rotation tween (seconds).
pub const TURN_DURATION_SECS: f32 = 0.15;

/// Wall height in world units. With `CELL_SIZE = 2.0` this gives a 1.5×
/// wall-to-corridor ratio — claustrophobic Wizardry feel.
pub const CELL_HEIGHT: f32 = 3.0;

/// Wall plate thickness in world units. Non-zero to avoid degenerate-volume
/// `Cuboid`s (zero thickness produces undefined normals); 0.05 is invisible at
/// typical viewing distances and avoids z-fighting with adjacent floor/ceiling slabs.
pub const WALL_THICKNESS: f32 = 0.05;

/// Floor + ceiling slab thickness. Same value as `WALL_THICKNESS` for consistency.
pub const FLOOR_THICKNESS: f32 = 0.05;

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

/// Marker on every entity spawned by `spawn_dungeon_geometry`: floor tiles,
/// ceiling tiles, and wall plates. The `OnExit` cleanup walks this query to
/// despawn all dungeon geometry on state transition out of `GameState::Dungeon`.
/// Replaces Feature #7's `TestSceneMarker` (deleted in Feature #8).
#[derive(Component, Debug, Clone, Copy)]
pub struct DungeonGeometry;

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
                (spawn_party_and_camera, spawn_dungeon_geometry),
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

        #[cfg(feature = "dev")]
        app.add_systems(OnEnter(GameState::Dungeon), spawn_debug_grid_hud)
            .add_systems(OnExit(GameState::Dungeon), despawn_debug_grid_hud)
            .add_systems(
                Update,
                update_debug_grid_hud.run_if(in_state(GameState::Dungeon)),
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

/// Returns the world-space `Transform` for a wall plate on the given cell's given face.
///
/// Cell `(grid_x, grid_y)` has center at world `(grid_x * CELL_SIZE, 0.0, grid_y * CELL_SIZE)`
/// (per `grid_to_world`'s convention; `world_y = 0.0` is floor level). Each wall plate is
/// positioned at the cell's edge (offset ±CELL_SIZE / 2 in world X or Z), centered vertically
/// at `world_y = CELL_HEIGHT / 2`.
///
/// Wall meshes are pre-oriented (NS = X-extending, EW = Z-extending), so all four directions
/// share `Quat::IDENTITY` rotation; the caller picks the right mesh handle for the direction.
fn wall_transform(grid_x: u32, grid_y: u32, dir: Direction) -> Transform {
    let cx = grid_x as f32 * CELL_SIZE;
    let cz = grid_y as f32 * CELL_SIZE;
    let cy = CELL_HEIGHT / 2.0;
    let half = CELL_SIZE / 2.0;
    let translation = match dir {
        Direction::North => Vec3::new(cx, cy, cz - half),
        Direction::South => Vec3::new(cx, cy, cz + half),
        Direction::East => Vec3::new(cx + half, cy, cz),
        Direction::West => Vec3::new(cx - half, cy, cz),
    };
    Transform::from_translation(translation)
}

/// Maps `WallType` to its rendering material. Returns `None` for variants that have
/// no geometry (`Open | OneWay`).
///
/// `Solid`, `SecretWall`, and `Illusory` all share the solid-wall material — the player
/// cannot visually distinguish them in v1; the reveal mechanic is Feature #13's scope.
fn wall_material(
    wt: WallType,
    solid: &Handle<StandardMaterial>,
    door: &Handle<StandardMaterial>,
    locked: &Handle<StandardMaterial>,
) -> Option<Handle<StandardMaterial>> {
    match wt {
        WallType::Open | WallType::OneWay => None,
        WallType::Solid | WallType::SecretWall | WallType::Illusory => Some(solid.clone()),
        WallType::Door => Some(door.clone()),
        WallType::LockedDoor => Some(locked.clone()),
    }
}

// ---------------------------------------------------------------------------
// Systems
// ---------------------------------------------------------------------------

/// `OnEnter(GameState::Dungeon)` — spawn the `PlayerParty` root entity at
/// `floor.entry_point`, with a child `Camera3d` at eye height.
///
/// The `Camera3d` entity carries a child `PointLight` implementing Wizardry-style
/// torchlight: a warm point source that illuminates cells near the player while
/// distant corridors fade to near-black. Because the light is a grandchild of
/// `PlayerParty`, it is despawned recursively when `PlayerParty` is cleaned up on
/// `OnExit(GameState::Dungeon)`.
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
            // Wizardry-style torch: warm point light at the camera's eye position.
            // Positioned at the camera origin (local (0,0,0)) — slightly forward/below
            // adjustments can be made in Feature #25 polish if a "held torch" offset is
            // desired. Despawned recursively with PlayerParty on OnExit(Dungeon).
            children![(
                PointLight {
                    color: Color::srgb(1.0, 0.85, 0.55), // warm yellow-orange torch flame
                    intensity: 4000.0, // bright torch — overpowers near-black ambient
                    range: 8.0,        // ~4 cells of light radius
                    shadows_enabled: false, // shadows are Feature #9
                    ..default()
                },
                Transform::from_xyz(0.0, 0.0, 0.0),
            )],
        )],
    ));

    info!(
        "Spawned PlayerParty at grid ({}, {}) facing {:?}",
        sx, sy, facing
    );
}

/// `OnExit(GameState::Dungeon)` — despawn all `PlayerParty` entities (recursive —
/// child `Camera3d` + torch `PointLight` are cleaned up automatically) and all
/// `DungeonGeometry` entities (floor + ceiling tiles, wall plates). Also restores
/// `GlobalAmbientLight` to its `LightPlugin` default so other states (Town, Combat,
/// etc.) start with a clean ambient setting. Future states own their own ambient
/// override on entry.
fn despawn_dungeon_entities(
    mut commands: Commands,
    parties: Query<Entity, With<PlayerParty>>,
    dungeon_geometry: Query<Entity, With<DungeonGeometry>>,
) {
    for e in &parties {
        commands.entity(e).despawn();
    }
    for e in &dungeon_geometry {
        commands.entity(e).despawn();
    }
    // Restore default ambient light (white, brightness 80.0). Other states will
    // override again on their own OnEnter (#18 Town, future states).
    commands.insert_resource(GlobalAmbientLight::default());
    info!("Despawned PlayerParty + dungeon geometry on OnExit(Dungeon); ambient restored");
}

/// `OnEnter(GameState::Dungeon)` — spawn floor + ceiling slabs per cell and wall
/// plates per renderable edge. Also sets `GlobalAmbientLight` to a near-black
/// dungeon override for Wizardry-style torchlight atmosphere; the player's
/// `PointLight` (child of `DungeonCamera`) is the primary illumination source.
///
/// Asset-tolerant: warns and returns silently if `DungeonAssets` or the floor handle
/// is not yet loaded (mirrors `spawn_party_and_camera`).
///
/// ## Iteration rule (per-edge deduplication)
///
/// For each cell, render `north` and `west` walls; render `south` ONLY at the bottom
/// edge (`y == height - 1`), `east` ONLY at the right edge (`x == width - 1`). This
/// guarantees each shared interior wall is rendered exactly once (avoids z-fighting and
/// double geometry). `Open | OneWay` walls skip geometry via `wall_material` returning
/// `None`.
fn spawn_dungeon_geometry(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    dungeon_assets: Option<Res<DungeonAssets>>,
    floors: Res<Assets<DungeonFloor>>,
) {
    // Asset-tolerant: same pattern as spawn_party_and_camera.
    let Some(assets) = dungeon_assets else {
        warn!("DungeonAssets resource not present at OnEnter(Dungeon); geometry spawn deferred");
        return;
    };
    let Some(floor) = floors.get(&assets.floor_01) else {
        warn!("DungeonFloor not yet loaded; geometry spawn deferred");
        return;
    };

    // Cached mesh handles (one per shape, shared across all cells for draw-call batching).
    let floor_mesh = meshes.add(Cuboid::new(CELL_SIZE, FLOOR_THICKNESS, CELL_SIZE));
    let ceiling_mesh = meshes.add(Cuboid::new(CELL_SIZE, FLOOR_THICKNESS, CELL_SIZE));
    // wall_mesh_ns: extends along world X (north/south walls). Thin along Z.
    let wall_mesh_ns = meshes.add(Cuboid::new(CELL_SIZE, CELL_HEIGHT, WALL_THICKNESS));
    // wall_mesh_ew: extends along world Z (east/west walls). Thin along X.
    let wall_mesh_ew = meshes.add(Cuboid::new(WALL_THICKNESS, CELL_HEIGHT, CELL_SIZE));

    // Cached material handles (one per role).
    let floor_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.30, 0.28, 0.25), // warm dark stone
        ..default()
    });
    let ceiling_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.20, 0.20, 0.22), // cool dark stone
        ..default()
    });
    let wall_solid_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.50, 0.50, 0.55), // cool grey — Solid + SecretWall + Illusory
        ..default()
    });
    let wall_door_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.45, 0.30, 0.15), // brown — Door
        ..default()
    });
    let wall_locked_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.55, 0.20, 0.15), // dark red — LockedDoor (visual warning)
        ..default()
    });

    for y in 0..floor.height {
        for x in 0..floor.width {
            let world_x = x as f32 * CELL_SIZE;
            let world_z = y as f32 * CELL_SIZE;

            // Floor tile: top face flush with world_y = 0.0; cuboid center at -FLOOR_THICKNESS/2.
            commands.spawn((
                Mesh3d(floor_mesh.clone()),
                MeshMaterial3d(floor_mat.clone()),
                Transform::from_xyz(world_x, -FLOOR_THICKNESS / 2.0, world_z),
                DungeonGeometry,
            ));

            // Ceiling tile: bottom face flush with world_y = CELL_HEIGHT; cuboid center at
            // CELL_HEIGHT + FLOOR_THICKNESS/2.
            commands.spawn((
                Mesh3d(ceiling_mesh.clone()),
                MeshMaterial3d(ceiling_mat.clone()),
                Transform::from_xyz(world_x, CELL_HEIGHT + FLOOR_THICKNESS / 2.0, world_z),
                DungeonGeometry,
            ));

            let walls = &floor.walls[y as usize][x as usize];

            // North wall: always rendered (every cell owns its north face).
            if let Some(mat) = wall_material(
                walls.north,
                &wall_solid_mat,
                &wall_door_mat,
                &wall_locked_mat,
            ) {
                commands.spawn((
                    Mesh3d(wall_mesh_ns.clone()),
                    MeshMaterial3d(mat),
                    wall_transform(x, y, Direction::North),
                    DungeonGeometry,
                ));
            }

            // West wall: always rendered.
            if let Some(mat) = wall_material(
                walls.west,
                &wall_solid_mat,
                &wall_door_mat,
                &wall_locked_mat,
            ) {
                commands.spawn((
                    Mesh3d(wall_mesh_ew.clone()),
                    MeshMaterial3d(mat),
                    wall_transform(x, y, Direction::West),
                    DungeonGeometry,
                ));
            }

            // South wall: only at the bottom edge.
            if y == floor.height - 1
                && let Some(mat) = wall_material(
                    walls.south,
                    &wall_solid_mat,
                    &wall_door_mat,
                    &wall_locked_mat,
                )
            {
                commands.spawn((
                    Mesh3d(wall_mesh_ns.clone()),
                    MeshMaterial3d(mat),
                    wall_transform(x, y, Direction::South),
                    DungeonGeometry,
                ));
            }

            // East wall: only at the right edge.
            if x == floor.width - 1
                && let Some(mat) = wall_material(
                    walls.east,
                    &wall_solid_mat,
                    &wall_door_mat,
                    &wall_locked_mat,
                )
            {
                commands.spawn((
                    Mesh3d(wall_mesh_ew.clone()),
                    MeshMaterial3d(mat),
                    wall_transform(x, y, Direction::East),
                    DungeonGeometry,
                ));
            }
        }
    }

    // Wizardry-style torchlight: set scene-wide ambient to near-black (1.0 lux ≈
    // moonlight). The player's PointLight (child of DungeonCamera, intensity 4000)
    // dominates near the player and falls off into darkness at corridor distance.
    // Restored to default on OnExit (see despawn_dungeon_entities).
    commands.insert_resource(GlobalAmbientLight {
        color: Color::WHITE,
        brightness: 1.0,
        ..default()
    });

    info!(
        "Spawned dungeon geometry for floor '{}' ({}×{})",
        floor.name, floor.width, floor.height
    );
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
// Debug grid HUD (dev-only)
// ---------------------------------------------------------------------------

/// Marker on the dev-only grid-position text overlay. Has its own marker
/// (NOT `DungeonGeometry`) so entity-count tests on real geometry are
/// unaffected; cleanup is via `despawn_debug_grid_hud` on OnExit.
#[cfg(feature = "dev")]
#[derive(Component)]
struct DebugGridHud;

/// `OnEnter(GameState::Dungeon)` (dev-only) — spawn a `Camera2d` UI overlay
/// camera (rendered on top of the 3D scene) and a top-left corner `Text`
/// overlay showing the player's current grid coordinates and facing.
/// Updated every frame by `update_debug_grid_hud`.
///
/// The Camera2d is required because `bevy_ui` text rendering needs a 2D
/// camera target. `Order: 1` puts it above the dungeon's `Camera3d` (default
/// order 0). `ClearColorConfig::None` keeps the 3D scene visible underneath.
#[cfg(feature = "dev")]
fn spawn_debug_grid_hud(mut commands: Commands) {
    commands.spawn((
        Camera2d,
        Camera {
            order: 1,
            clear_color: ClearColorConfig::None,
            ..default()
        },
        DebugGridHud,
    ));

    commands.spawn((
        Text::new("Position: -- | Facing: --"),
        TextFont {
            font_size: 18.0,
            ..default()
        },
        TextColor(Color::srgb(0.95, 0.95, 0.6)),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(8.0),
            left: Val::Px(12.0),
            ..default()
        },
        DebugGridHud,
    ));
}

/// `Update` (dev-only, gated on `GameState::Dungeon`) — refresh the HUD
/// text from the current `PlayerParty`'s `GridPosition` + `Facing`.
#[cfg(feature = "dev")]
fn update_debug_grid_hud(
    party: Query<(&GridPosition, &Facing), With<PlayerParty>>,
    mut hud: Query<&mut Text, With<DebugGridHud>>,
) {
    let Ok((pos, facing)) = party.single() else {
        return;
    };
    let Ok(mut text) = hud.single_mut() else {
        return;
    };
    text.0 = format!("Position: ({}, {}) | Facing: {:?}", pos.x, pos.y, facing.0);
}

/// `OnExit(GameState::Dungeon)` (dev-only) — despawn the HUD overlay.
#[cfg(feature = "dev")]
fn despawn_debug_grid_hud(mut commands: Commands, hud: Query<Entity, With<DebugGridHud>>) {
    for entity in &hud {
        commands.entity(entity).despawn();
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

    #[test]
    fn wall_transform_north_on_cell_3_4() {
        // Cell (3, 4): center world (6.0, 0.0, 8.0). North wall at z = 8.0 - 1.0 = 7.0.
        let t = wall_transform(3, 4, Direction::North);
        assert!(t.translation.abs_diff_eq(Vec3::new(6.0, 1.5, 7.0), 1e-6));
        assert!(t.rotation.abs_diff_eq(Quat::IDENTITY, 1e-6));
    }

    #[test]
    fn wall_transform_south_on_cell_3_4() {
        let t = wall_transform(3, 4, Direction::South);
        assert!(t.translation.abs_diff_eq(Vec3::new(6.0, 1.5, 9.0), 1e-6));
    }

    #[test]
    fn wall_transform_east_on_cell_3_4() {
        let t = wall_transform(3, 4, Direction::East);
        assert!(t.translation.abs_diff_eq(Vec3::new(7.0, 1.5, 8.0), 1e-6));
    }

    #[test]
    fn wall_transform_west_on_cell_3_4() {
        let t = wall_transform(3, 4, Direction::West);
        assert!(t.translation.abs_diff_eq(Vec3::new(5.0, 1.5, 8.0), 1e-6));
    }

    #[test]
    fn wall_transform_corner_cell_origin() {
        // Cell (0, 0): negative coords are valid (wall plate is north of the grid origin).
        let t_north = wall_transform(0, 0, Direction::North);
        assert!(
            t_north
                .translation
                .abs_diff_eq(Vec3::new(0.0, 1.5, -1.0), 1e-6)
        );
        let t_west = wall_transform(0, 0, Direction::West);
        assert!(
            t_west
                .translation
                .abs_diff_eq(Vec3::new(-1.0, 1.5, 0.0), 1e-6)
        );
    }

    #[test]
    fn wall_material_returns_none_for_passable() {
        let solid: Handle<StandardMaterial> = Handle::default();
        let door: Handle<StandardMaterial> = Handle::default();
        let locked: Handle<StandardMaterial> = Handle::default();
        assert!(wall_material(WallType::Open, &solid, &door, &locked).is_none());
        assert!(wall_material(WallType::OneWay, &solid, &door, &locked).is_none());
    }

    #[test]
    fn wall_material_returns_some_for_blocking() {
        let solid: Handle<StandardMaterial> = Handle::default();
        let door: Handle<StandardMaterial> = Handle::default();
        let locked: Handle<StandardMaterial> = Handle::default();
        // Solid, SecretWall, Illusory all share the solid handle.
        assert!(wall_material(WallType::Solid, &solid, &door, &locked).is_some());
        assert!(wall_material(WallType::SecretWall, &solid, &door, &locked).is_some());
        assert!(wall_material(WallType::Illusory, &solid, &door, &locked).is_some());
        // Door + LockedDoor have their own handles.
        assert!(wall_material(WallType::Door, &solid, &door, &locked).is_some());
        assert!(wall_material(WallType::LockedDoor, &solid, &door, &locked).is_some());
    }

    // -----------------------------------------------------------------------
    // Test app helpers
    // -----------------------------------------------------------------------

    /// Build a minimal test app with the full dungeon plugin chain.
    /// Duplicates the pattern from src/plugins/input/mod.rs and
    /// src/plugins/audio/mod.rs tests.
    ///
    /// `spawn_dungeon_geometry` requires `Assets<Mesh>` and `Assets<StandardMaterial>`.
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
        app.update(); // OnEnter(Dungeon) systems run; party + geometry spawned
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

    // -----------------------------------------------------------------------
    // Geometry tests (Feature #8)
    // -----------------------------------------------------------------------

    /// Build a `w × h` floor with EVERY wall set to `Solid` on every cell.
    /// Counterpart to `make_open_floor`. Used for the maximum-renderable-walls
    /// regression test.
    fn make_walled_floor(w: u32, h: u32) -> DungeonFloor {
        use crate::data::dungeon::{CellFeatures, WallMask};
        let solid_mask = WallMask {
            north: WallType::Solid,
            south: WallType::Solid,
            east: WallType::Solid,
            west: WallType::Solid,
        };
        DungeonFloor {
            name: "test_walled".into(),
            width: w,
            height: h,
            floor_number: 1,
            walls: vec![vec![solid_mask; w as usize]; h as usize],
            features: vec![vec![CellFeatures::default(); w as usize]; h as usize],
            entry_point: (1, 1, Direction::North),
            encounter_table: "test_table".into(),
        }
    }

    /// Open 3×3 floor: every wall is `Open` (no geometry rendered for any wall).
    /// Expected: 9 floor + 9 ceiling + 0 walls = 18 entities.
    /// (No DirectionalLight — torchlight is a child of DungeonCamera, not DungeonGeometry.)
    #[test]
    fn spawn_dungeon_geometry_open_3x3_yields_18_entities() {
        let mut app = make_test_app();
        insert_test_floor(&mut app, make_open_floor(3, 3));
        advance_into_dungeon(&mut app);

        let count = app
            .world_mut()
            .query_filtered::<Entity, With<DungeonGeometry>>()
            .iter(app.world())
            .count();
        assert_eq!(
            count, 18,
            "Open 3×3 floor: 9 floor tiles + 9 ceiling tiles + 0 walls = 18"
        );
    }

    /// Fully-walled 3×3 floor: every cell has all 4 walls Solid.
    ///
    /// Per the canonical iteration rule:
    ///   - 9 floor tiles + 9 ceiling tiles = 18
    ///   - North walls: 9 (one per cell)
    ///   - West walls:  9 (one per cell)
    ///   - South walls: 3 (bottom row only, y==2)
    ///   - East walls:  3 (right column only, x==2)
    ///   - Total walls: 9 + 9 + 3 + 3 = 24
    ///
    /// Expected: 18 + 24 = 42 entities.
    #[test]
    fn spawn_dungeon_geometry_walled_3x3_yields_42_entities() {
        let mut app = make_test_app();
        insert_test_floor(&mut app, make_walled_floor(3, 3));
        advance_into_dungeon(&mut app);

        let count = app
            .world_mut()
            .query_filtered::<Entity, With<DungeonGeometry>>()
            .iter(app.world())
            .count();
        assert_eq!(
            count, 42,
            "Walled 3×3 floor: 18 floor/ceiling + 24 walls = 42 (see test docstring for math)"
        );
    }

    /// After OnExit(Dungeon), all DungeonGeometry entities are despawned and
    /// GlobalAmbientLight is restored to its default.
    #[test]
    fn on_exit_dungeon_despawns_all_dungeon_geometry() {
        let mut app = make_test_app();
        insert_test_floor(&mut app, make_walled_floor(3, 3));
        advance_into_dungeon(&mut app);

        // Sanity: geometry is present.
        let pre = app
            .world_mut()
            .query_filtered::<Entity, With<DungeonGeometry>>()
            .iter(app.world())
            .count();
        assert!(pre > 0, "Geometry should be present in Dungeon");

        // Transition to TitleScreen — triggers OnExit(Dungeon).
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::TitleScreen);
        app.update(); // state transition realised
        app.update(); // OnExit(Dungeon) systems run

        let post = app
            .world_mut()
            .query_filtered::<Entity, With<DungeonGeometry>>()
            .iter(app.world())
            .count();
        assert_eq!(
            post, 0,
            "All DungeonGeometry entities must be despawned on OnExit(Dungeon)"
        );

        // GlobalAmbientLight should be restored to LightPlugin default.
        let ambient = app.world().resource::<GlobalAmbientLight>();
        let default_ambient = GlobalAmbientLight::default();
        assert_eq!(
            ambient.brightness, default_ambient.brightness,
            "Ambient brightness should restore to default"
        );
    }
}
