//! Enemy billboard sprite rendering — Feature #17.
//!
//! ## Pipeline
//!
//! ```text
//!     #16's handle_encounter_request spawns Enemy entities with
//!     EnemyVisual + EnemyAnimation (Default-derived via EnemyBundle)
//!                ↓                                                ┌─ DamageTaken (#17)
//!     OnEnter(GameState::Combat) →                                │   ┌─ AttackStart (future)
//!     spawn_enemy_billboards (this module)                        ┃   │   ┌─ Died (future)
//!     attaches Sprite + Sprite3d (bevy_sprite3d) +                ┃   │   │
//!     Transform + EnemyBillboard marker to every Enemy entity.    ┃   │   │
//!     bevy_sprite3d's PostUpdate bundle_builder then fills        ┃   │   │
//!     Mesh3d + MeshMaterial3d from a cached quad.                 ┃   │   │
//!                ↓                                                ┃   │   │
//!     Update systems (gated `in_state(GameState::Combat)`):       ┃   │   │
//!       - face_camera          (rotates each sprite to camera)    ┃   │   │
//!       - advance_enemy_animation (frame counter; state machine)  ┃   │   │
//!       - on_enemy_visual_event (consumes EnemyVisualEvent) ← ━━━━┻━━━┻━━━┛
//!       - damage_shake_tween   (jitter on DamageTaken)
//!       - detect_enemy_damage  (HP delta → DamageTaken producer)
//!                ↓
//!     OnExit(GameState::Combat) → clear_current_encounter (#16 owns)
//!     despawns every Enemy entity (and all its visual components transitively).
//! ```
//!
//! ## Cleanup is free (Pitfall 6)
//!
//! Visual components live on the SAME entity as `Enemy`. `clear_current_encounter`
//! at `combat/encounter.rs:200-215` already sweeps `Query<Entity, With<Enemy>>`
//! and despawns each. Bevy's ref-counted asset cleanup drops the per-enemy
//! `MeshMaterial3d`/`StandardMaterial`/`Handle<Image>` automatically (the
//! `Sprite3dCaches` resource ref-counts the mesh too). DO NOT add a second
//! despawn system.
//!
//! ## Public API for #22 FOEs
//!
//! `spawn_enemy_visual(commands, images, entity, color, position)` is the
//! agnostic spawn helper. `spawn_enemy_billboards` (combat-specific)
//! computes the row layout and calls `spawn_enemy_visual` per enemy. #22
//! will call `spawn_enemy_visual` directly with overworld-grid positions.

use bevy::asset::RenderAssetUsages;
use bevy::image::Image;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy_sprite3d::{Sprite3d, Sprite3dPlugin};

use crate::plugins::combat::enemy::Enemy;
use crate::plugins::combat::encounter::CurrentEncounter;
use crate::plugins::dungeon::DungeonCamera;
use crate::plugins::party::character::DerivedStats;
use crate::plugins::state::GameState;

// ─────────────────────────────────────────────────────────────────────────────
// Constants
// ─────────────────────────────────────────────────────────────────────────────

/// Pixel-to-metre conversion factor for `Sprite3d.pixels_per_metre`.
/// Combined with the image dimensions below, this fixes the world-space
/// size: image is 14×18 px @ pixels_per_metre = 10.0 → 1.4m × 1.8m quad.
const SPRITE_PIXELS_PER_METRE: f32 = 10.0;

/// Placeholder image width in pixels. `bevy_sprite3d` derives the world
/// width as `image.width / pixels_per_metre`. Authored 14 → 1.4m.
const SPRITE_IMAGE_W: u32 = 14;

/// Placeholder image height in pixels. Authored 18 → 1.8m.
const SPRITE_IMAGE_H: u32 = 18;

/// Distance in front of the camera at which enemies stand in the combat row.
const SPRITE_DISTANCE: f32 = 4.0;

/// Horizontal spacing between adjacent enemies in the combat row.
const SPRITE_SPACING: f32 = 1.6;

/// Vertical offset above the camera's eye-height so feet are roughly at floor.
const SPRITE_Y_OFFSET: f32 = 0.8;

/// Default colour when `EnemyVisual.id` doesn't resolve in `EnemyDb`
/// (back-compat with empty-id inline `EnemySpec`).
/// Used as fallback in `combat/encounter.rs` spawn loop and as sentinel
/// when `EnemyVisual` was never resolved from `EnemyDb`.
pub const DEFAULT_PLACEHOLDER_COLOR: [f32; 3] = [0.5, 0.5, 0.5];

/// Damage-shake amplitude (metres of `Transform.translation.x` jitter).
const SHAKE_AMPLITUDE: f32 = 0.08;

/// Damage-shake duration in seconds.
const SHAKE_DURATION_SECS: f32 = 0.15;

/// Animation frame interval. For placeholder PR (single-frame sprites)
/// this gates state transitions, not frame swaps.
const ANIMATION_FRAME_SECS: f32 = 0.12;

// ─────────────────────────────────────────────────────────────────────────────
// Components
// ─────────────────────────────────────────────────────────────────────────────

/// Marker on every enemy entity rendered as a face-camera billboard sprite.
/// Queried by `face_camera` to know which transforms to rotate.
///
/// This is a project-local marker; the `bevy_sprite3d::Sprite3d` component
/// is the actual rendering primitive on the same entity. Keeping them
/// separate means `face_camera` can filter by `With<EnemyBillboard>` to
/// exclude any future non-enemy sprites that #22 or later features add.
#[derive(Component, Reflect, Default, Debug, Clone, Copy)]
pub struct EnemyBillboard;

/// Visual data for an enemy — resolved from `EnemyDb` at spawn time
/// (see `combat/encounter.rs:367-380`). Lives on the `Enemy` entity
/// so `clear_current_encounter`'s despawn sweep covers it.
///
/// `id` is empty for back-compat with inline `EnemySpec` authored
/// before #17; `spawn_enemy_billboards` falls back to
/// `DEFAULT_PLACEHOLDER_COLOR` in that case.
#[derive(Component, Reflect, Default, Debug, Clone)]
pub struct EnemyVisual {
    pub id: String,
    pub placeholder_color: [f32; 3],
}

/// Animation state for an enemy. The state machine has four named states;
/// `Attacking` and `TakingDamage` return to `Idle` on completion; `Dying`
/// holds its last frame (combat-cleanup handles despawn).
#[derive(Reflect, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AnimState {
    #[default]
    Idle,
    Attacking,
    TakingDamage,
    Dying,
}

/// Per-state frame counts. For the placeholder PR every state has
/// `count = 1` (single-frame solid-colour sprites). Real-art PRs set
/// real counts and add a `TextureAtlas` to the entity; this struct
/// stays the same.
#[derive(Reflect, Debug, Clone, Copy)]
pub struct AnimStateFrames {
    pub idle_count: usize,
    pub attack_count: usize,
    pub damage_count: usize,
    pub dying_count: usize,
}

impl Default for AnimStateFrames {
    fn default() -> Self {
        // Placeholder PR: every state is one frame.
        Self {
            idle_count: 1,
            attack_count: 1,
            damage_count: 1,
            dying_count: 1,
        }
    }
}

/// Animation tracker on an enemy entity. Default-constructible so it
/// participates in `EnemyBundle`'s `..Default::default()` chain.
#[derive(Component, Reflect, Debug, Clone)]
pub struct EnemyAnimation {
    pub state: AnimState,
    pub frame_index: usize,
    pub frame_timer: Timer,
    pub frames: AnimStateFrames,
}

impl Default for EnemyAnimation {
    fn default() -> Self {
        Self {
            state: AnimState::Idle,
            frame_index: 0,
            frame_timer: Timer::from_seconds(ANIMATION_FRAME_SECS, TimerMode::Repeating),
            frames: AnimStateFrames::default(),
        }
    }
}

/// Visual feedback request — fires the animation state machine and
/// (for `DamageTaken`) the damage-shake tween. Producers: `detect_enemy_damage`
/// in this module (HP-delta watcher for `DamageTaken`); future hooks in
/// `turn_manager.rs::execute_combat_actions` for `AttackStart`/`Died`.
///
/// `#[derive(Message)]`, NOT `Event` — Bevy 0.18 family rename.
#[derive(Message, Debug, Clone, Copy)]
pub struct EnemyVisualEvent {
    pub target: Entity,
    pub kind: EnemyVisualEventKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnemyVisualEventKind {
    AttackStart,
    DamageTaken,
    Died,
}

/// In-flight damage-shake animation on an enemy. Removed by
/// `damage_shake_tween` when `elapsed_secs >= SHAKE_DURATION_SECS`.
/// On removal, the system snaps `Transform.translation.x` to `base_x`
/// to avoid float drift.
///
/// Mirrors the `MovementAnimation` lifecycle pattern at
/// `dungeon/mod.rs:117-153`.
#[derive(Component, Debug, Clone, Copy)]
pub struct DamageShake {
    pub base_x: f32,
    pub elapsed_secs: f32,
}

/// Per-frame snapshot of `DerivedStats.current_hp` from the previous
/// frame. `detect_enemy_damage` compares this against the live value
/// to emit `EnemyVisualEvent::DamageTaken` on HP decreases.
///
/// Default value is 0; the first frame's compare against 0 trivially
/// "no damage" since current_hp starts populated. Auto-inserted on
/// the first frame after spawn by `detect_enemy_damage` itself for
/// any Enemy entity missing it.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct PreviousHp(pub u32);

// ─────────────────────────────────────────────────────────────────────────────
// Public spawn API (reused by #22 FOEs)
// ─────────────────────────────────────────────────────────────────────────────

/// Public API for spawning an enemy's visual layer at a known world position.
/// Combat-specific spawn (`spawn_enemy_billboards`) computes positions for the
/// combat row; #22 FOEs will compute overworld grid positions and call this.
///
/// `entity` is an existing `Enemy` entity. This function INSERTS visual
/// components into it — it does NOT spawn a new entity. Cleanup is the
/// responsibility of whoever despawns `entity` (combat: `clear_current_encounter`;
/// FOEs: #22 will own its own despawn path).
///
/// Generates the placeholder texture from `placeholder_color` (clamped to
/// `[0.0, 1.0]` per channel at the trust boundary). The 14×18 px image
/// dimensions encode the desired 1.4m × 1.8m aspect ratio (with
/// `pixels_per_metre = SPRITE_PIXELS_PER_METRE = 10.0`).
///
/// `bevy_sprite3d::bundle_builder` (PostUpdate) populates the cached
/// `Mesh3d` + `MeshMaterial3d<StandardMaterial>` automatically from the
/// `Sprite3d` + `Sprite` components inserted here.
pub fn spawn_enemy_visual(
    commands: &mut Commands,
    images: &mut Assets<Image>,
    entity: Entity,
    placeholder_color: [f32; 3],
    position: Vec3,
) {
    // Trust-boundary clamp on RON-deserialized colour values (research
    // §Architectural Security Risks). Mirrors the precedent at
    // combat/encounter.rs:281 for encounter_rate.clamp(0.0, 1.0).
    let [r, g, b] = placeholder_color.map(|c| c.clamp(0.0, 1.0));
    let texel: [u8; 4] = [
        (r * 255.0) as u8,
        (g * 255.0) as u8,
        (b * 255.0) as u8,
        255,
    ];

    // 14×18 solid-colour Image generated in-memory. Image::new_fill repeats
    // the 4-byte texel across the full extent. RENDER_WORLD (not MAIN_WORLD)
    // per Pitfall 7 — MAIN_WORLD has the GPU copy freed at runtime.
    let image = Image::new_fill(
        Extent3d {
            width: SPRITE_IMAGE_W,
            height: SPRITE_IMAGE_H,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &texel,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    );
    let image_handle = images.add(image);

    // bevy_sprite3d builds the cached textured quad (Mesh3d + MeshMaterial3d)
    // from these two components in its PostUpdate `bundle_builder` system.
    // unlit per Pitfall 4 — Druum's low-ambient + carried-torch setup would
    // render placeholder colours muddy if PBR-sampled. Mask per Pitfall 3 —
    // back-to-front sort flicker under Blend when enemies are in a row.
    commands.entity(entity).insert((
        Sprite {
            image: image_handle,
            ..default()
        },
        Sprite3d {
            pixels_per_metre: SPRITE_PIXELS_PER_METRE,
            unlit: true,
            alpha_mode: AlphaMode::Mask(0.5),
            ..default()
        },
        Transform::from_translation(position),
        Visibility::default(),
        EnemyBillboard,
    ));
}

// ─────────────────────────────────────────────────────────────────────────────
// Plugin
// ─────────────────────────────────────────────────────────────────────────────

pub struct EnemyRenderPlugin;

impl Plugin for EnemyRenderPlugin {
    fn build(&self, app: &mut App) {
        // Register Sprite3dPlugin idempotently so #22's FOE plugin (which
        // will also use bevy_sprite3d) can register either plugin without
        // panicking on double-add.
        if !app.is_plugin_added::<Sprite3dPlugin>() {
            app.add_plugins(Sprite3dPlugin);
        }

        app.register_type::<EnemyBillboard>()
            .register_type::<EnemyVisual>()
            .register_type::<EnemyAnimation>()
            .add_message::<EnemyVisualEvent>()
            .add_systems(OnEnter(GameState::Combat), spawn_enemy_billboards)
            .add_systems(
                Update,
                (
                    face_camera,
                    advance_enemy_animation,
                    on_enemy_visual_event,
                    damage_shake_tween,
                    detect_enemy_damage,
                )
                    .run_if(in_state(GameState::Combat)),
            );
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Systems
// ─────────────────────────────────────────────────────────────────────────────

fn spawn_enemy_billboards(
    mut commands: Commands,
    encounter: Option<Res<CurrentEncounter>>,
    enemies_q: Query<(Entity, &EnemyVisual), With<Enemy>>,
    camera_q: Query<&GlobalTransform, With<DungeonCamera>>,
    mut images: ResMut<Assets<Image>>,
) {
    // Guard: encounter must be populated (research §Pitfall 5).
    if encounter.is_none() {
        warn!("OnEnter(Combat) fired without CurrentEncounter — skipping billboard spawn");
        return;
    }
    // Guard: camera must exist (research §Pitfall 5 — robust to test setups without DungeonPlugin).
    let Ok(camera) = camera_q.single() else {
        warn!("OnEnter(Combat) — DungeonCamera missing, skipping billboard spawn");
        return;
    };

    // No shared_quad allocation needed — bevy_sprite3d caches meshes
    // internally in `Sprite3dCaches.mesh_cache` keyed by image dimensions +
    // pivot + double_sided + atlas. All 10 enemies share the same 14×18 px
    // image dimensions → one cached mesh. (Materials still differ per enemy
    // because colours differ.)

    let camera_pos = camera.translation();
    let forward = camera.forward();
    let right = camera.right();

    let total = enemies_q.iter().count() as f32;
    for (i, (entity, visual)) in enemies_q.iter().enumerate() {
        let offset = (i as f32 - (total - 1.0) / 2.0) * SPRITE_SPACING;
        let world_pos = camera_pos
            + (*forward) * SPRITE_DISTANCE
            + (*right) * offset
            + Vec3::Y * SPRITE_Y_OFFSET;

        spawn_enemy_visual(
            &mut commands,
            &mut images,
            entity,
            visual.placeholder_color,
            world_pos,
        );
    }

    info!(
        "Spawned billboards for {} enemies on OnEnter(Combat)",
        total as usize
    );
}

fn face_camera(
    camera_q: Query<&GlobalTransform, With<DungeonCamera>>,
    mut sprites_q: Query<
        &mut Transform,
        (With<EnemyBillboard>, Without<DungeonCamera>),
    >,
) {
    let Ok(camera) = camera_q.single() else {
        return;
    };
    let camera_pos = camera.translation();
    for mut t in &mut sprites_q {
        // atan2(dx, dz) gives yaw for a quad whose default normal is +Z.
        // bevy_sprite3d's internal quad() builder produces a +Z-facing quad
        // (see lib.rs `Mesh::ATTRIBUTE_NORMAL = [0,0,1]`), so this math
        // works without any flip dance.
        // Y-axis-locked — sprite never pitches even if camera does.
        let dx = camera_pos.x - t.translation.x;
        let dz = camera_pos.z - t.translation.z;
        let angle = dx.atan2(dz);
        t.rotation = Quat::from_rotation_y(angle);
    }
}

fn advance_enemy_animation(
    time: Res<Time>,
    mut q: Query<&mut EnemyAnimation>,
) {
    for mut anim in &mut q {
        anim.frame_timer.tick(time.delta());
        if !anim.frame_timer.just_finished() {
            continue;
        }

        let frame_count = match anim.state {
            AnimState::Idle => anim.frames.idle_count,
            AnimState::Attacking => anim.frames.attack_count,
            AnimState::TakingDamage => anim.frames.damage_count,
            AnimState::Dying => anim.frames.dying_count,
        };
        if frame_count == 0 {
            continue;
        }

        anim.frame_index += 1;
        if anim.frame_index >= frame_count {
            match anim.state {
                AnimState::Attacking | AnimState::TakingDamage => {
                    anim.state = AnimState::Idle;
                    anim.frame_index = 0;
                }
                AnimState::Dying => {
                    anim.frame_index = frame_count - 1; // hold last frame
                }
                AnimState::Idle => {
                    anim.frame_index = 0; // loop
                }
            }
        }
    }
}

fn on_enemy_visual_event(
    mut events: MessageReader<EnemyVisualEvent>,
    mut anim_q: Query<&mut EnemyAnimation>,
    mut commands: Commands,
    transform_q: Query<&Transform, With<EnemyBillboard>>,
) {
    for ev in events.read() {
        // 1. Update animation state.
        if let Ok(mut anim) = anim_q.get_mut(ev.target) {
            anim.state = match ev.kind {
                EnemyVisualEventKind::AttackStart => AnimState::Attacking,
                EnemyVisualEventKind::DamageTaken => AnimState::TakingDamage,
                EnemyVisualEventKind::Died => AnimState::Dying,
            };
            anim.frame_index = 0;
            anim.frame_timer.reset();
        }

        // 2. For DamageTaken: kick off the shake tween (insert DamageShake).
        //    Only attach if the entity has a Transform AND EnemyBillboard
        //    (i.e., it has been spawned by spawn_enemy_billboards). Stash the
        //    current x as base_x so the tween snaps back exactly.
        if ev.kind == EnemyVisualEventKind::DamageTaken
            && let Ok(transform) = transform_q.get(ev.target)
        {
            commands.entity(ev.target).insert(DamageShake {
                base_x: transform.translation.x,
                elapsed_secs: 0.0,
            });
        }
    }
}

fn damage_shake_tween(
    time: Res<Time>,
    mut commands: Commands,
    mut q: Query<(Entity, &mut Transform, &mut DamageShake)>,
) {
    for (entity, mut transform, mut shake) in &mut q {
        shake.elapsed_secs += time.delta_secs();
        if shake.elapsed_secs >= SHAKE_DURATION_SECS {
            // Snap back to base_x and remove the component.
            transform.translation.x = shake.base_x;
            commands.entity(entity).remove::<DamageShake>();
            continue;
        }
        // Sine-driven jitter, attenuates as t → 1.0.
        let t = shake.elapsed_secs / SHAKE_DURATION_SECS;
        let phase = t * std::f32::consts::TAU * 4.0; // 4 wobbles over the tween
        let envelope = 1.0 - t; // linear attenuation
        let offset = phase.sin() * SHAKE_AMPLITUDE * envelope;
        transform.translation.x = shake.base_x + offset;
    }
}

fn detect_enemy_damage(
    mut q: Query<(Entity, &DerivedStats, Option<&mut PreviousHp>), With<Enemy>>,
    mut commands: Commands,
    mut events: MessageWriter<EnemyVisualEvent>,
) {
    for (entity, stats, prev_opt) in &mut q {
        match prev_opt {
            Some(mut prev) => {
                if stats.current_hp < prev.0 {
                    events.write(EnemyVisualEvent {
                        target: entity,
                        kind: EnemyVisualEventKind::DamageTaken,
                    });
                }
                prev.0 = stats.current_hp;
            }
            None => {
                // First frame for this entity — seed PreviousHp from current.
                // No DamageTaken event on the seeding frame.
                commands
                    .entity(entity)
                    .insert(PreviousHp(stats.current_hp));
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests — Layer 1 (pure, no App)
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // Test 1: face-camera angle math at 4 cardinal axes.
    #[test]
    fn face_camera_angle_at_cardinal_axes() {
        // Helper mirroring the production math in `face_camera`.
        fn angle(camera_pos: Vec3, sprite_pos: Vec3) -> Quat {
            let dx = camera_pos.x - sprite_pos.x;
            let dz = camera_pos.z - sprite_pos.z;
            Quat::from_rotation_y(dx.atan2(dz))
        }
        let sprite = Vec3::ZERO;

        // Camera at +Z (in front of sprite, world's +Z direction):
        // dx=0, dz=1, atan2(0, 1) = 0 → identity rotation.
        let q_north = angle(Vec3::new(0.0, 0.0, 1.0), sprite);
        assert!((q_north.to_euler(EulerRot::YXZ).0 - 0.0).abs() < 1e-4);

        // Camera at +X:
        // dx=1, dz=0, atan2(1, 0) = π/2.
        let q_east = angle(Vec3::new(1.0, 0.0, 0.0), sprite);
        assert!((q_east.to_euler(EulerRot::YXZ).0 - std::f32::consts::FRAC_PI_2).abs() < 1e-4);

        // Camera at -Z:
        // dx=0, dz=-1, atan2(0, -1) = π.
        let q_south = angle(Vec3::new(0.0, 0.0, -1.0), sprite);
        let south_yaw = q_south.to_euler(EulerRot::YXZ).0;
        assert!(
            (south_yaw - std::f32::consts::PI).abs() < 1e-4
                || (south_yaw + std::f32::consts::PI).abs() < 1e-4,
            "expected ±π, got {south_yaw}"
        );

        // Camera at -X:
        // dx=-1, dz=0, atan2(-1, 0) = -π/2.
        let q_west = angle(Vec3::new(-1.0, 0.0, 0.0), sprite);
        assert!((q_west.to_euler(EulerRot::YXZ).0 + std::f32::consts::FRAC_PI_2).abs() < 1e-4);
    }

    // Test 2: Image::new_fill produces a usable Handle<Image>.
    #[test]
    fn image_new_fill_produces_a_handle() {
        let texel: [u8; 4] = [255, 0, 0, 255]; // pure red
        let image = Image::new_fill(
            Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
            TextureDimension::D2,
            &texel,
            TextureFormat::Rgba8UnormSrgb,
            RenderAssetUsages::RENDER_WORLD,
        );
        let mut images = Assets::<Image>::default();
        let handle = images.add(image);
        // Round-trip: handle must resolve back to the same data.
        let fetched = images.get(&handle).expect("handle resolves");
        assert_eq!(fetched.data.as_ref().expect("data set").len(), 4);
        assert_eq!(fetched.data.as_ref().unwrap()[0], 255, "R channel");
        assert_eq!(fetched.data.as_ref().unwrap()[1], 0, "G channel");
        assert_eq!(fetched.data.as_ref().unwrap()[2], 0, "B channel");
        assert_eq!(fetched.data.as_ref().unwrap()[3], 255, "A channel");
    }

    // Test 3: placeholder_color clamps to [0.0, 1.0].
    #[test]
    fn placeholder_color_clamps() {
        let inputs: [f32; 3] = [2.0, -1.0, 0.5];
        let clamped: [f32; 3] = inputs.map(|c| c.clamp(0.0, 1.0));
        assert_eq!(clamped, [1.0, 0.0, 0.5]);
    }

    // Test 4: animation state machine — Attacking → Idle after frame count expires.
    #[test]
    fn animation_attacking_returns_to_idle() {
        let mut anim = EnemyAnimation {
            state: AnimState::Attacking,
            frame_index: 0,
            frame_timer: Timer::from_seconds(ANIMATION_FRAME_SECS, TimerMode::Repeating),
            frames: AnimStateFrames {
                idle_count: 1,
                attack_count: 4,
                damage_count: 1,
                dying_count: 1,
            },
        };
        // Simulate 4 frame-timer-finish ticks.
        for _ in 0..4 {
            anim.frame_timer
                .tick(std::time::Duration::from_secs_f32(ANIMATION_FRAME_SECS));
            if !anim.frame_timer.just_finished() {
                continue;
            }
            let count = match anim.state {
                AnimState::Idle => anim.frames.idle_count,
                AnimState::Attacking => anim.frames.attack_count,
                AnimState::TakingDamage => anim.frames.damage_count,
                AnimState::Dying => anim.frames.dying_count,
            };
            anim.frame_index += 1;
            if anim.frame_index >= count {
                match anim.state {
                    AnimState::Attacking | AnimState::TakingDamage => {
                        anim.state = AnimState::Idle;
                        anim.frame_index = 0;
                    }
                    AnimState::Dying => anim.frame_index = count - 1,
                    AnimState::Idle => anim.frame_index = 0,
                }
            }
        }
        assert_eq!(
            anim.state,
            AnimState::Idle,
            "Attacking returns to Idle after attack_count frames"
        );
        assert_eq!(anim.frame_index, 0, "frame_index resets on state change");
    }

    // Test 5: animation state machine — Dying holds last frame.
    #[test]
    fn animation_dying_holds_last_frame() {
        let mut anim = EnemyAnimation {
            state: AnimState::Dying,
            frame_index: 0,
            frame_timer: Timer::from_seconds(ANIMATION_FRAME_SECS, TimerMode::Repeating),
            frames: AnimStateFrames {
                idle_count: 1,
                attack_count: 1,
                damage_count: 1,
                dying_count: 3,
            },
        };
        // Tick enough frames to exceed dying_count.
        for _ in 0..10 {
            anim.frame_timer
                .tick(std::time::Duration::from_secs_f32(ANIMATION_FRAME_SECS));
            if !anim.frame_timer.just_finished() {
                continue;
            }
            anim.frame_index += 1;
            if anim.frame_index >= anim.frames.dying_count {
                match anim.state {
                    AnimState::Dying => anim.frame_index = anim.frames.dying_count - 1,
                    _ => unreachable!(),
                }
            }
        }
        assert_eq!(anim.state, AnimState::Dying, "Dying must not transition out");
        assert_eq!(anim.frame_index, 2, "Dying holds last frame (count - 1)");
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests — Layer 2 (App-driven integration)
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod app_tests {
    use super::*;
    use bevy::state::app::StatesPlugin;
    use bevy::time::TimeUpdateStrategy;
    use std::time::Duration;

    use crate::data::EnemyDb;
    use crate::data::dungeon::Direction;
    use crate::plugins::combat::enemy::{Enemy, EnemyBundle, EnemyName};
    use crate::plugins::combat::encounter::CurrentEncounter;
    use crate::plugins::dungeon::{DungeonCamera, Facing, GridPosition, PlayerParty};

    /// Build a minimal test app that exercises EnemyRenderPlugin's lifecycle.
    /// Mirrors `combat/encounter.rs:558-593` make_test_app.
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
        app.init_asset::<crate::data::SpellDb>();
        app.init_asset::<crate::data::EncounterTable>();
        app.init_asset::<EnemyDb>();
        // Mesh + StandardMaterial + Image + TextureAtlasLayout needed by bevy_sprite3d's bundle_builder PostUpdate system.
        // MinimalPlugins lacks PbrPlugin; init explicitly (same pattern as dungeon/tests.rs).
        app.init_asset::<bevy::prelude::Mesh>();
        app.init_asset::<bevy::pbr::StandardMaterial>();
        app.init_asset::<Image>();
        app.init_asset::<bevy::image::TextureAtlasLayout>();
        app.add_message::<crate::plugins::dungeon::MovedEvent>();
        app.init_resource::<crate::plugins::dungeon::ActiveFloorNumber>();
        app.add_message::<crate::plugins::audio::SfxRequest>();
        app.init_resource::<
            leafwing_input_manager::prelude::ActionState<crate::plugins::input::DungeonAction>,
        >();
        app.init_resource::<
            leafwing_input_manager::prelude::ActionState<crate::plugins::input::CombatAction>,
        >();
        #[cfg(feature = "dev")]
        app.init_resource::<bevy::input::ButtonInput<bevy::prelude::KeyCode>>();
        app
    }

    /// Spawn a fake PlayerParty entity with a child Camera3d carrying the
    /// DungeonCamera marker — `spawn_enemy_billboards` reads GlobalTransform
    /// from this query.
    fn spawn_test_camera(app: &mut App) {
        app.world_mut().spawn((
            PlayerParty,
            Transform::from_translation(Vec3::ZERO),
            GridPosition { x: 0, y: 0 },
            Facing(Direction::East),
            GlobalTransform::default(),
            children![(
                Camera3d::default(),
                Transform::from_xyz(0.0, 0.7, 0.0),
                GlobalTransform::default(),
                DungeonCamera,
            )],
        ));
    }

    /// Spawn N `Enemy` entities directly (bypassing the encounter pipeline)
    /// and populate `CurrentEncounter` so spawn_enemy_billboards has both
    /// the resource and the entities to attach visuals to.
    fn spawn_test_encounter(app: &mut App, count: usize) -> Vec<Entity> {
        let mut entities = Vec::with_capacity(count);
        for i in 0..count {
            let entity = app
                .world_mut()
                .spawn(EnemyBundle {
                    name: EnemyName(format!("Test{i}")),
                    visual: EnemyVisual {
                        id: format!("test{i}"),
                        placeholder_color: [0.5, 0.5, 0.5],
                    },
                    ..Default::default()
                })
                .id();
            entities.push(entity);
        }
        app.world_mut().insert_resource(CurrentEncounter {
            enemy_entities: entities.clone(),
            fleeable: true,
        });
        entities
    }

    // Integration test 1: OnEnter(Combat) attaches Sprite + Sprite3d + EnemyBillboard
    // (and `bevy_sprite3d::bundle_builder` fills Mesh3d + MeshMaterial3d via #[require]).
    #[test]
    fn enemies_get_billboard_components_on_combat_entry() {
        let mut app = make_test_app();
        spawn_test_camera(&mut app);
        let entities = spawn_test_encounter(&mut app, 3);

        // Trigger OnEnter(Combat).
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Combat);
        app.update();
        app.update();

        for entity in &entities {
            let world = app.world();
            // Our explicit inserts.
            assert!(
                world.entity(*entity).get::<Sprite>().is_some(),
                "entity {entity:?} must have Sprite after OnEnter(Combat)"
            );
            assert!(
                world.entity(*entity).get::<Sprite3d>().is_some(),
                "entity {entity:?} must have Sprite3d after OnEnter(Combat)"
            );
            assert!(
                world.entity(*entity).get::<EnemyBillboard>().is_some(),
                "entity {entity:?} must have EnemyBillboard after OnEnter(Combat)"
            );
            // Filled by bevy_sprite3d's bundle_builder in PostUpdate.
            // (Each #[require(...)] target is inserted as default; bundle_builder
            // then populates real values.)
            assert!(
                world.entity(*entity).get::<Mesh3d>().is_some(),
                "entity {entity:?} must have Mesh3d (auto-filled by bevy_sprite3d)"
            );
            assert!(
                world.entity(*entity).get::<MeshMaterial3d<StandardMaterial>>().is_some(),
                "entity {entity:?} must have MeshMaterial3d (auto-filled by bevy_sprite3d)"
            );
        }
    }

    // Integration test 2: OnExit(Combat) sweeps all billboard entities (via clear_current_encounter).
    #[test]
    fn no_billboard_entities_remain_after_combat_exit() {
        let mut app = make_test_app();
        spawn_test_camera(&mut app);
        spawn_test_encounter(&mut app, 3);

        // Enter Combat...
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Combat);
        app.update();
        app.update();

        // ...then exit.
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Dungeon);
        app.update();
        app.update();

        // Zero entities with EnemyBillboard remain (despawned via
        // clear_current_encounter sweeping `With<Enemy>`).
        let count = app
            .world_mut()
            .query::<&EnemyBillboard>()
            .iter(app.world())
            .count();
        assert_eq!(count, 0, "all billboard entities must despawn on OnExit(Combat)");

        // Defence-in-depth: zero Sprite3d entities either (would catch a
        // bug where someone adds Sprite3d to a non-Enemy entity in a
        // future feature without proper cleanup).
        let sprite_count = app
            .world_mut()
            .query::<&Sprite3d>()
            .iter(app.world())
            .count();
        assert_eq!(
            sprite_count,
            0,
            "all Sprite3d entities must despawn on OnExit(Combat)"
        );

        // And no Enemy entities either (regression: cleanup is one-pass).
        let enemy_count = app
            .world_mut()
            .query::<&Enemy>()
            .iter(app.world())
            .count();
        assert_eq!(enemy_count, 0, "all Enemy entities must despawn on OnExit(Combat)");
    }

    // Integration test 3: HP delta on an Enemy emits EnemyVisualEvent::DamageTaken.
    #[test]
    fn hp_delta_emits_damage_taken_event() {
        let mut app = make_test_app();
        spawn_test_camera(&mut app);
        let entities = spawn_test_encounter(&mut app, 1);
        let enemy = entities[0];

        // Enter Combat so detect_enemy_damage runs.
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Combat);
        app.update();
        app.update(); // PreviousHp seeded with starting current_hp (0 for default bundle)

        // Mutate Enemy's current_hp downward — but first set a non-zero starting
        // HP so we can decrease it.
        {
            let world = app.world_mut();
            let mut entity_ref = world.entity_mut(enemy);
            let mut stats = entity_ref.get_mut::<DerivedStats>().unwrap();
            stats.current_hp = 30;
        }
        app.update(); // PreviousHp now sees 30 (delta from 0 — not counted as damage; PreviousHp had been seeded to 0)

        // NOTE: the seeding-frame semantic is intentional — first detection of
        // a current_hp increase is NOT damage. To assert damage, drop HP next.
        {
            let world = app.world_mut();
            let mut entity_ref = world.entity_mut(enemy);
            let mut stats = entity_ref.get_mut::<DerivedStats>().unwrap();
            stats.current_hp = 25; // -5 damage
        }
        app.update();

        // Read the Messages directly to assert DamageTaken fired.
        let messages = app
            .world()
            .resource::<bevy::ecs::message::Messages<EnemyVisualEvent>>();
        let mut cursor = messages.get_cursor();
        let events: Vec<&EnemyVisualEvent> = cursor.read(messages).collect();
        assert!(
            events
                .iter()
                .any(|e| e.target == enemy && e.kind == EnemyVisualEventKind::DamageTaken),
            "DamageTaken event must fire on HP decrease; got: {events:?}"
        );
    }

    // Integration test 4: damage-shake tween perturbs and then snaps back to base_x.
    #[test]
    fn damage_shake_returns_to_base_x() {
        let mut app = make_test_app();
        spawn_test_camera(&mut app);
        let entities = spawn_test_encounter(&mut app, 1);
        let enemy = entities[0];

        // Enter Combat to attach Transform/Sprite/Sprite3d/EnemyBillboard.
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Combat);
        app.update();
        app.update();

        // Snapshot the spawn-time x position.
        let base_x = app
            .world()
            .entity(enemy)
            .get::<Transform>()
            .unwrap()
            .translation
            .x;

        // Emit DamageTaken to kick off the tween.
        app.world_mut()
            .resource_mut::<bevy::ecs::message::Messages<EnemyVisualEvent>>()
            .write(EnemyVisualEvent {
                target: enemy,
                kind: EnemyVisualEventKind::DamageTaken,
            });

        // Drive time forward past SHAKE_DURATION_SECS using deterministic
        // ManualDuration. 0.16s > 0.15s — guaranteed to expire the tween.
        app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(160)));
        app.update(); // process event, insert DamageShake
        app.update(); // tween advances; elapsed_secs += 0.16 → ≥ SHAKE_DURATION_SECS → snap + remove

        // Tween must be gone.
        assert!(
            app.world().entity(enemy).get::<DamageShake>().is_none(),
            "DamageShake must be removed after SHAKE_DURATION_SECS"
        );
        // Transform must be snapped back to base_x.
        let final_x = app
            .world()
            .entity(enemy)
            .get::<Transform>()
            .unwrap()
            .translation
            .x;
        assert!(
            (final_x - base_x).abs() < 1e-4,
            "Transform.translation.x must snap back to base_x: got {final_x}, expected {base_x}"
        );
    }
}
