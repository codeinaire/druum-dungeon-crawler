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
    use crate::data::dungeon::{CellFeatures, LightingConfig, WallMask};
    DungeonFloor {
        name: "test".into(),
        width: w,
        height: h,
        floor_number: 1,
        walls: vec![vec![WallMask::default(); w as usize]; h as usize],
        features: vec![vec![CellFeatures::default(); w as usize]; h as usize],
        entry_point: (1, 1, Direction::North),
        encounter_table: "test_table".into(),
        lighting: LightingConfig::default(),
        locked_doors: Vec::new(),
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
        use crate::data::dungeon::{CellFeatures, LightingConfig, WallMask};
        let floor = DungeonFloor {
            name: "test5x5".into(),
            width: 5,
            height: 5,
            floor_number: 1,
            walls: vec![vec![WallMask::default(); 5]; 5],
            features: vec![vec![CellFeatures::default(); 5]; 5],
            entry_point: (2, 2, Direction::North),
            encounter_table: "test_table".into(),
            lighting: LightingConfig::default(),
            locked_doors: Vec::new(),
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
    use crate::data::dungeon::{CellFeatures, LightingConfig, WallMask};
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
        lighting: LightingConfig::default(),
        locked_doors: Vec::new(),
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

    // Feature #9: the carried torch (only Torch entity) must also be gone.
    // It's a grandchild of PlayerParty, despawned recursively.
    let post_torch_count = app
        .world_mut()
        .query_filtered::<Entity, With<Torch>>()
        .iter(app.world())
        .count();
    assert_eq!(
        post_torch_count, 0,
        "Carried torch must be despawned on OnExit(Dungeon)"
    );
}

// -----------------------------------------------------------------------
// Lighting tests (Feature #9)
// -----------------------------------------------------------------------

#[test]
fn distance_fog_attached_to_dungeon_camera() {
    let mut app = make_test_app();
    insert_test_floor(&mut app, make_open_floor(3, 3));
    advance_into_dungeon(&mut app);

    // Query: a Camera3d marked DungeonCamera should also carry DistanceFog.
    let count = app
        .world_mut()
        .query_filtered::<&DistanceFog, With<DungeonCamera>>()
        .iter(app.world())
        .count();
    assert_eq!(
        count, 1,
        "DungeonCamera must carry DistanceFog after OnEnter"
    );

    // Falloff must be Exponential (NEVER default Linear, which is invisible).
    let fog = app
        .world_mut()
        .query_filtered::<&DistanceFog, With<DungeonCamera>>()
        .single(app.world())
        .unwrap();
    assert!(
        matches!(fog.falloff, FogFalloff::Exponential { .. }),
        "DistanceFog falloff must be Exponential — Linear default is invisible at dungeon scale"
    );
}

#[test]
fn flicker_is_deterministic_for_same_phase_and_t() {
    // Pure-helper test — no App, no Time, no scheduler. Just verifies
    // flicker_factor is a function of (t, phase) only.
    let f1 = super::flicker_factor(1.234, 0.5);
    let f2 = super::flicker_factor(1.234, 0.5);
    assert_eq!(
        f1, f2,
        "flicker_factor must be deterministic for same inputs"
    );
    // And different phases produce different factors (sanity).
    let f3 = super::flicker_factor(1.234, 1.5);
    assert_ne!(f1, f3, "different phase must produce different factor");
}

#[test]
fn flicker_torches_modulates_carried_torch_intensity() {
    // End-to-end coverage: verifies flicker_torches is registered, runs in
    // GameState::Dungeon, and modulates the carried torch's PointLight.intensity.
    // Closes the gap left by removing the cell-torch flicker test during the
    // Feature #9 cell-torch strip — without this test, an accidental drop of
    // flicker_torches from DungeonPlugin::build would pass all other tests.
    let mut app = make_test_app();
    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
        100,
    )));
    insert_test_floor(&mut app, make_open_floor(3, 3));
    advance_into_dungeon(&mut app);
    // Advance one more frame so elapsed_secs > 0 (flicker_factor at t=0 returns
    // 1.0 exactly because both sines are zero — would not catch a skipped run).
    app.update();

    let intensity = app
        .world_mut()
        .query_filtered::<&PointLight, With<Torch>>()
        .single(app.world())
        .unwrap()
        .intensity;

    // Carried torch base_intensity is 60_000; flicker_factor is clamped to
    // [0.80, 1.20], so intensity must land in [48_000, 72_000].
    assert!(
        (48_000.0..=72_000.0).contains(&intensity),
        "Torch intensity {intensity} must be in the flicker band [48_000, 72_000]"
    );
    // And it must not be exactly base_intensity after a non-zero tick (unless
    // both sines hit a zero crossing simultaneously — vanishingly unlikely).
    assert_ne!(
        intensity, 60_000.0,
        "Torch intensity should not be exactly base_intensity after a non-zero tick"
    );
}
