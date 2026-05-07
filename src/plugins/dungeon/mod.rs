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

use bevy::pbr::{DistanceFog, FogFalloff};
use bevy::prelude::*;
use leafwing_input_manager::prelude::ActionState;

use crate::data::DungeonFloor;
use crate::data::dungeon::{Direction, WallType};
use crate::plugins::audio::{SfxKind, SfxRequest};
use crate::plugins::input::DungeonAction;
use crate::plugins::loading::DungeonAssets;
use crate::plugins::state::GameState;

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
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Hash)]
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

/// Marker on the player-carried `PointLight` (a grandchild of `DungeonCamera`).
/// Filter for the `flicker_torches` system so untagged `PointLight`s are
/// untouched.
///
/// `base_intensity` captures the spawn-time intensity so the flicker formula
/// modulates around it (`light.intensity = base_intensity * factor`); the
/// system never reads `light.intensity` itself, so the flicker remains stable
/// across frames regardless of any one-frame race.
///
/// `phase_offset` is `f32::consts::PI` for the carried torch — chosen so it
/// stays out of sync with any future cell-anchored torches added later.
#[derive(Component, Debug, Clone, Copy)]
pub struct Torch {
    pub base_intensity: f32,
    pub phase_offset: f32,
}

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
                        .run_if(in_state(GameState::Dungeon))
                        .before(animate_movement),
                    animate_movement.run_if(in_state(GameState::Dungeon)),
                    flicker_torches.run_if(in_state(GameState::Dungeon)),
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

/// Returns `true` if the player can move from cell `(pos.x, pos.y)` in direction `dir`,
/// consulting both static `DungeonFloor::can_move` AND runtime `DoorStates`.
///
/// Truth table layered on `floor.can_move`:
/// - `WallType::Door`: `DoorStates[(pos, dir)] == Some(Open)` → passable; else → blocked
///   (default `DoorState::Closed`; D15 — closed-by-default). Pitfall 4: `floor.can_move`
///   returns `true` for Door at the asset level — this wrapper overrides that.
/// - `WallType::LockedDoor`: same — `DoorStates[(pos, dir)] == Some(Open)` → passable (unlocked
///   via `handle_door_interact`). Pitfall 9: `floor.can_move` returns `false` for LockedDoor;
///   the wrapper must return `true` when `DoorStates` says Open.
/// - All other wall types: defer to `floor.can_move` (no DoorStates check).
fn can_move_with_doors(
    floor: &crate::data::DungeonFloor,
    door_states: &crate::plugins::dungeon::features::DoorStates,
    pos: GridPosition,
    dir: Direction,
) -> bool {
    let cell = match floor
        .walls
        .get(pos.y as usize)
        .and_then(|row| row.get(pos.x as usize))
    {
        Some(c) => c,
        None => return false,
    };
    let wall = match dir {
        Direction::North => cell.north,
        Direction::South => cell.south,
        Direction::East => cell.east,
        Direction::West => cell.west,
    };
    use crate::plugins::dungeon::features::DoorState;
    match wall {
        WallType::Door | WallType::LockedDoor => {
            let state = door_states
                .doors
                .get(&(pos, dir))
                .copied()
                .unwrap_or_default();
            state == DoorState::Open
        }
        _ => floor.can_move(pos.x, pos.y, dir),
    }
}

/// Two-sine flicker formula. Returns a multiplier to apply to base intensity.
/// Theoretical peak amplitude is ±15% (sum of two sines at 0.10 + 0.05 weights),
/// but clamped to `[0.80, 1.20]` defensively (Feature #9 research §Pitfall 5 —
/// real torches vary 5-15%; >20% reads as "broken light bulb" not "flame").
fn flicker_factor(t: f32, phase: f32) -> f32 {
    let s1 = bevy::math::ops::sin(t * 6.4 + phase);
    let s2 = bevy::math::ops::sin(t * 23.0 + phase * 1.7);
    (1.0 + 0.10 * s1 + 0.05 * s2).clamp(0.80, 1.20)
}

/// Returns the `DungeonFloor` handle for `floor_number` from `DungeonAssets`.
/// Falls back to `floor_01` for unknown floor numbers and emits a warning.
/// Feature #13 Phase 8 (D11-A): floor_02 is the only additional floor in v1;
/// future floors follow the same match arm pattern.
pub(crate) fn floor_handle_for(assets: &DungeonAssets, floor_number: u32) -> &Handle<DungeonFloor> {
    match floor_number {
        1 => &assets.floor_01,
        2 => &assets.floor_02,
        n => {
            warn!("No DungeonFloor handle for floor {n}; falling back to floor_01");
            &assets.floor_01
        }
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
    mut pending_teleport: Option<ResMut<crate::plugins::dungeon::features::PendingTeleport>>,
) {
    let Some(assets) = dungeon_assets else {
        warn!("DungeonAssets resource not present at OnEnter(Dungeon); party spawn deferred");
        return;
    };

    // Feature #13 cross-floor teleport (D3-α + D11-A):
    // Determine the active floor number from PendingTeleport (if set), then
    // resolve the correct floor handle. pt.target.take() clears the resource
    // after use so non-teleport re-entries don't reuse it.
    let active_floor_number = pending_teleport
        .as_ref()
        .and_then(|pt| pt.target.as_ref().map(|t| t.floor))
        .unwrap_or(1);
    let floor_handle = floor_handle_for(&assets, active_floor_number);
    let Some(floor) = floors.get(floor_handle) else {
        warn!("DungeonFloor not yet loaded; party spawn deferred");
        return;
    };

    // Override entry_point if teleport target is set; clear after use.
    let (sx, sy, facing) = if let Some(ref mut pt) = pending_teleport {
        if let Some(target) = pt.target.take() {
            let facing = target.facing.unwrap_or(floor.entry_point.2);
            (target.x, target.y, facing)
        } else {
            floor.entry_point
        }
    } else {
        floor.entry_point
    };
    let start_pos = GridPosition { x: sx, y: sy };
    let world_pos = grid_to_world(start_pos);
    let initial_rotation = facing_to_quat(facing);
    let fog_color = floor.lighting.fog.color.into_color();
    let fog_density = floor.lighting.fog.density;

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
            // Per-floor fog (Feature #9). Falloff is ALWAYS Exponential — DistanceFog::default()
            // falloff is Linear { 0.0, 100.0 } which is invisible at dungeon scale.
            DistanceFog {
                color: fog_color,
                falloff: FogFalloff::Exponential {
                    density: fog_density,
                },
                ..default()
            },
            // Wizardry-style torch: warm point light at the camera's eye position.
            // Positioned at the camera origin (local (0,0,0)) — slightly forward/below
            // adjustments can be made in Feature #25 polish if a "held torch" offset is
            // desired. Despawned recursively with PlayerParty on OnExit(Dungeon).
            // DO NOT MODIFY carried torch properties — Feature #8 user override.
            // Feature #9 only appends the Torch marker so the flicker system finds it.
            children![(
                PointLight {
                    color: Color::srgb(1.0, 0.85, 0.55), // warm yellow-orange torch flame
                    intensity: 60_000.0, // bright torch — overpowers near-black ambient
                    range: 12.0,         // ~6 cells of light radius
                    shadows_enabled: false,
                    ..default()
                },
                Transform::from_xyz(0.0, 0.0, 0.0),
                Torch {
                    base_intensity: 60_000.0,
                    phase_offset: std::f32::consts::PI,
                },
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
/// plates per renderable edge. Also sets `GlobalAmbientLight` from
/// `floor.lighting.ambient_brightness` — defaults to `1.0` (near-black) for
/// floors that don't override.
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

    // Per-floor ambient (Feature #9). LightingConfig::default() has
    // ambient_brightness: 1.0 — preserves Feature #8's near-black behavior for
    // floors that don't override. Restored to GlobalAmbientLight::default() on
    // OnExit (see despawn_dungeon_entities).
    commands.insert_resource(GlobalAmbientLight {
        color: Color::WHITE,
        brightness: floor.lighting.ambient_brightness,
        ..default()
    });

    info!(
        "Spawned dungeon geometry for floor '{}' ({}×{})",
        floor.name, floor.width, floor.height
    );
}

/// `Update` (gated on `GameState::Dungeon`) — translate `ActionState<DungeonAction>`
/// input into grid moves. Runs in both `DungeonSubState::Exploring` AND
/// `DungeonSubState::Map` so the player can walk while looking at the map
/// (Wizardry-canonical — the map updates as you move).
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
///
/// **Visibility is `pub(crate)`** so `MinimapPlugin::update_explored_on_move`
/// can use `.after(handle_dungeon_input)` for system ordering (Feature #10
/// Pitfall 3: minimap subscriber must run after input handler to guarantee
/// `MovedEvent` is published before the subscriber reads it in the same frame).
/// Do not make this `pub` (no external consumer needed) or `fn` (ordering
/// coupling would silently break). If you remove `pub(crate)`, `minimap.rs`
/// will produce a compile error on the `.after(...)` ordering call.
#[allow(clippy::type_complexity)]
#[allow(clippy::too_many_arguments)]
pub(crate) fn handle_dungeon_input(
    mut commands: Commands,
    actions: Res<ActionState<DungeonAction>>,
    dungeon_assets: Option<Res<DungeonAssets>>,
    floors: Res<Assets<DungeonFloor>>,
    door_states: Res<crate::plugins::dungeon::features::DoorStates>, // Feature #13 D9b
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
        // Feature #13 D9b: consult DoorStates for Door/LockedDoor passability.
        if !can_move_with_doors(floor, &door_states, *pos, move_dir) {
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

/// `Update` — modulate every `Torch`-tagged `PointLight::intensity` per frame
/// using a deterministic two-sine formula (`flicker_factor`). Runs always in
/// `GameState::Dungeon` (no `DungeonSubState` gate — torches flicker even with
/// the menu open, immersion preservation).
///
/// **Filter:** `With<Torch>` is mandatory. Removing it would touch every
/// `PointLight` in the world (future spell effects, UI lights, etc.).
/// The marker is the contract.
///
/// **Determinism:** uses `Time::elapsed_secs()` and the per-entity
/// `Torch::phase_offset` only — no `rand`, no wall-clock seeding. The same
/// floor at the same `t` produces the same intensities every run.
fn flicker_torches(time: Res<Time>, mut lights: Query<(&mut PointLight, &Torch)>) {
    let t = time.elapsed_secs();
    for (mut light, torch) in &mut lights {
        light.intensity = torch.base_intensity * flicker_factor(t, torch.phase_offset);
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

pub mod features;

#[cfg(test)]
mod tests;
