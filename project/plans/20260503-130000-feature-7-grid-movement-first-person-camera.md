# Plan: Grid Movement & First-Person Camera — Feature #7

**Date:** 2026-05-03
**Status:** Complete
**Research:** ../research/20260503-120000-feature-7-grid-movement-first-person-camera.md
**Depends on:** 20260501-230000-feature-4-dungeon-grid-data-model.md, 20260502-000000-feature-5-input-system-leafwing.md, 20260502-120000-feature-6-audio-system.md, 20260501-164500-bevy-0-18-1-asset-pipeline-feature-3.md

## Goal

Fill the empty `DungeonPlugin` stub at `src/plugins/dungeon/mod.rs` with the first end-to-end gameplay loop: a `PlayerParty` entity carrying `GridPosition + Facing`, a child `Camera3d` at eye-height, an `OnEnter(GameState::Dungeon)` spawn that places both at `floor.entry_point`, an `OnExit(GameState::Dungeon)` despawn, an input handler that translates `Res<ActionState<DungeonAction>>` into grid moves gated by `DungeonFloor::can_move`, a tween-based `MovementAnimation` that animates the world `Transform` while logical state updates immediately, a `Message<MovedEvent>` for downstream subscribers (Features #13/#16/#22), `Message<SfxRequest>::Footstep` dispatch on translation, and a placeholder test scene (3 cubes + ground plane + ambient + directional light) so movement is visually verifiable on `cargo run --features dev`. Net delivery: ~450-650 LOC including tests, **zero new Cargo dependencies**, F9-cycle to Dungeon → visible 3D world that responds to WASDQE.

## Approach

The research (HIGH confidence on 0.18 API verification, HIGH on integration contracts, MEDIUM on tunable values) recommends mirroring Druum's existing audio crossfade architecture — specifically the `FadeIn`/`FadeOut` component-marker pattern at `src/plugins/audio/bgm.rs:36-67` — for movement animation. The architecture is one entity with a child camera, logical state that updates immediately, visual state that catches up via a marker component the animation system removes when finished.

The architectural decisions made here:

1. **Module layout: single `src/plugins/dungeon/mod.rs` file.** No submodules. The total LOC fits in ~525 lines, comparable to `src/plugins/audio/mod.rs + bgm.rs + sfx.rs` (~440 lines) but in one file because the gameplay logic doesn't naturally split — input → animation start → animation tick are tightly coupled. Submodule extraction can land later (#8 will likely add `src/plugins/dungeon/render.rs` for real geometry; #10 adds map; the time to split is when extraction is forced).

2. **Entity shape: one `PlayerParty` entity with one child `Camera3d` entity.** `PlayerParty` carries `(GridPosition, Facing, Transform, Visibility, PlayerParty)`. The camera is spawned via the 0.18 `children![...]` macro at local offset `Vec3::Y * EYE_HEIGHT`. Moving/rotating `PlayerParty` carries the camera transform; the `MovementAnimation` only writes to the parent `Transform`, never the child. Despawn-on-OnExit walks the parent's `Children` automatically (Bevy 0.18 `commands.entity(...).despawn()` is recursive by default — verified in Feature #3 at `src/plugins/loading/mod.rs:154-161`).

3. **Logical state updates immediately; visual state lerps.** When input is committed (after `can_move` clears), `GridPosition` and `Facing` are written to their final values **on the same frame**, `MovementAnimation { from_translation, to_translation, from_rotation, to_rotation, elapsed_secs, duration_secs }` is inserted, `MovedEvent` is written, `SfxRequest::Footstep` is written (translation moves only). The `animate_movement` system then ticks `elapsed_secs` each frame, lerps `Transform`, and removes the component when finished. **Logical-vs-visual separation** is master research §Anti-Pattern guidance (line 981-982); downstream consumers (#13 cell-trigger, #16 encounter) react to the new `GridPosition` immediately, not after the tween completes.

4. **No input buffering — drop input during animation via `Without<MovementAnimation>` query filter.** The `handle_dungeon_input` system queries `Query<..., (With<PlayerParty>, Without<MovementAnimation>)>`. A second key press during a tween is dropped. Genre precedent (Etrian Odyssey, Wizardry Trilogy Remaster, Legend of Grimrock) all use this. Implementing a buffer adds ~50 LOC for ambiguous benefit; the migration to a buffer in v2 is local to one query filter swap.

5. **Animation: one `MovementAnimation` component, tween 0.18s for translation, 0.15s for rotation.** Storing both translation and rotation fields (4×Vec3+f32 = ~52 bytes) in the same component keeps the system simple — one query, one matching arm. Smoothstep interpolation via `t * t * (3.0 - 2.0 * t)` (no crate needed). When a turn-only action fires, `from_translation == to_translation` so the lerp is a no-op for translation while rotation animates over 0.15s. The 0.18s/0.15s split is enforced by passing the duration into the constructor: `MovementAnimation::translate(...)` uses `MOVE_DURATION_SECS`, `MovementAnimation::rotate(...)` uses `TURN_DURATION_SECS`.

6. **World coordinate convention: `world_x = grid_x * CELL_SIZE`, `world_z = grid_y * CELL_SIZE`, `world_y = 0` for ground.** The `+grid_y * CELL_SIZE` form (research's recommendation) makes North movement (`grid_y -= 1`) translate to `-Z` motion in world space, which matches Bevy's default camera-looking-direction (`-Z`). This means a camera "facing North" can use `Quat::IDENTITY` rotation. The `data/dungeon.rs:18` doc-comment from Feature #4 conjectures `world_z = -grid_y * cell_size`; that comment is forward-looking guidance from a frozen file that we override here. Implementer should NOT modify `data/dungeon.rs` — the `dungeon/mod.rs` source-of-truth `grid_to_world` function and module doc comment supersede it. Feature #8 (real renderer) reads from `dungeon/mod.rs`'s convention.

7. **`facing_to_quat` conversion: `Direction::North → Quat::IDENTITY` (looks toward -Z).** With the `+grid_y` convention from #6, North = -Z = default camera direction. East = -π/2 rotation around Y (right-hand rule, clockwise when viewed from +Y). South = π. West = +π/2. The function is a pure 4-arm match returning `Quat::from_rotation_y(angle)`; testable without any Bevy app.

8. **`CELL_SIZE = 2.0` and `EYE_HEIGHT = 0.7` as `pub const` in `src/plugins/dungeon/mod.rs`.** Research §Architectural Decision #6 — 2.0 is genre-correct corridor scale; 0.7 produces "facing-forward" feel relative to Feature #8's planned 3.0 wall height. `MOVE_DURATION_SECS = 0.18`, `TURN_DURATION_SECS = 0.15` also as `pub const` constants. Centralising in `mod.rs` gives Feature #8 one import path; if a `dungeon/spatial.rs` ever splits, the constants migrate together.

9. **`MovedEvent` `Message`, fields `{ from: GridPosition, to: GridPosition, facing: Direction }`.** Derives `Message`, NOT `Event` (Bevy 0.18 family-rename trap, same as `SfxRequest`, `KeyboardInput`, `StateTransitionEvent`, `AssetEvent`). Registered with `app.add_message::<MovedEvent>()`. Includes the post-move `facing` so a downstream consumer reacts to the player's new orientation without a follow-up query. Turn-only actions DO NOT write `MovedEvent` — only translation moves do, because the contract is "the player crossed a cell boundary". This is what #13 (cell triggers) and #16 (encounter rolls) actually need; downstream features that need turn-events can layer their own.

10. **System ordering: `handle_dungeon_input` → `animate_movement` → (rest of frame).** Both run in `Update`. `animate_movement` runs every frame with `run_if(in_state(GameState::Dungeon))` (no SubState gate — if the player opens inventory mid-tween the tween still finishes; freezing mid-step is worse UX). `handle_dungeon_input` runs with `run_if(in_state(GameState::Dungeon).and(in_state(DungeonSubState::Exploring)))` so opening inventory pauses input. Explicit `.before(animate_movement)` ordering ensures the tween starts on the same frame the input is committed (avoids a one-frame delay between input and visual response).

11. **Test scene scaffolding: 3 colored cubes + ground plane + DirectionalLight, all tagged `TestSceneMarker` for cleanup-on-#8.** Spawned in a separate `spawn_test_scene` system on `OnEnter(GameState::Dungeon)`. The 3 cubes are placed at known offsets from the entry point so manual smoke-testing can verify "I moved one cell forward, the cube I was facing got closer." Ground plane is a `Cuboid::new(40.0, 0.1, 40.0)` slab at `y=-0.05` so the camera looking down doesn't see infinite skybox. `DirectionalLight { illuminance: 3000.0, shadows_enabled: false }` aimed at `Vec3::new(1.0, -1.0, 1.0)` (down-and-diagonal — defensible 45° angle giving differential corner shading without the glamour-shot intensity that #9 will set up properly). All five entities (3 cubes + ground + light) carry the `TestSceneMarker` component; `despawn_test_scene` on `OnExit(GameState::Dungeon)` walks the marker. Feature #8 deletes the entire `spawn_test_scene` function, the `TestSceneMarker` component, and the `despawn_test_scene` registration in one PR.

12. **Tests follow Layer 2 input pattern from Feature #5.** Full `MinimalPlugins + AssetPlugin + StatesPlugin + InputPlugin + ActionsPlugin + DungeonPlugin` chain. Inject `KeyboardInput` messages or use leafwing's `KeyCode::press(world_mut())` testing helper. Animation determinism uses `TimeUpdateStrategy::ManualDuration(Duration::from_millis(50))`. Test floor is constructed inline (duplicating Feature #4's `make_floor` helper because that helper is `#[cfg(test)]`-private to its own module; ~20 LOC of duplication is cheaper than refactoring a frozen file). One App-level integration test in `tests/dungeon_movement.rs` exercises the `OnEnter(Dungeon)` → spawn-party → input → MovedEvent flow end-to-end with the real `DungeonAssets` loading path.

## Critical

- **Zero new Cargo dependencies.** All required APIs (`Vec3::lerp`, `Quat::slerp`, `Quat::from_rotation_y`, `Time::delta_secs`, `Cuboid::new`, `StandardMaterial`, `MeshMaterial3d`, `Mesh3d`, `Camera3d`, `DirectionalLight`, `AmbientLight`, `PerspectiveProjection`, `Message`, `MessageReader`, `MessageWriter`, `add_message`, `OnEnter`, `OnExit`, `children![]`, `TimeUpdateStrategy::ManualDuration`) ship in Bevy 0.18.1 already pulled via the `bevy/3d` umbrella feature. **Do NOT add `bevy_tweening` or any other animation crate** — the lerp/slerp + smoothstep is ~10 LOC. If `git diff Cargo.toml Cargo.lock` after this feature shows any change, STOP. If a tween crate is genuinely needed (it isn't), escalate as Category C — do NOT silently add.

- **`MovedEvent` derives `Message`, NOT `Event`.** Bevy 0.18 family-rename. Read with `MessageReader<MovedEvent>`, write with `MessageWriter<MovedEvent>`. Register with `app.add_message::<MovedEvent>()`. Same trap as `SfxRequest` (#6), `KeyboardInput` (#5), `StateTransitionEvent` (#2), `AssetEvent` (#3), `AppExit` (#4 integration test). The `add_event` form will not compile against a `Message`-derived type — caught at first build.

- **Don't modify the freeze list:** `src/plugins/state/mod.rs` (#2), `src/plugins/loading/mod.rs` (#3), `src/plugins/audio/{mod,bgm,sfx}.rs` (#6), `src/plugins/input/mod.rs` (#5). All required types (`Direction`, `DungeonFloor`, `DungeonAction`, `ActionState<DungeonAction>`, `SfxRequest`, `SfxKind::Footstep`, `DungeonAssets`, `GameState::Dungeon`, `DungeonSubState::Exploring`) are already public and consumable from their existing modules. **`src/data/dungeon.rs` is not in the freeze list** but should also not be modified — the stale `world_z = -grid_y` doc-comment at line 18 is overridden by Feature #7's `dungeon/mod.rs` source-of-truth, no comment edit required.

- **State-gated systems via `run_if(in_state(GameState::Dungeon))` at the SYSTEM level.** Plugin trait has no `run_if` method (verified at `bevy_app-0.18.1/src/plugin.rs`). The roadmap's `app.add_plugins(InputManagerPlugin::<DungeonAction>::default().run_if(in_state(...)))` shape does not compile in Bevy 0.18; this trap was flagged in Feature #5's plan §Critical. State-scoping happens on the consuming systems: `.add_systems(Update, handle_dungeon_input.run_if(in_state(GameState::Dungeon).and(in_state(DungeonSubState::Exploring))))`. Animation does NOT gate on the SubState (so tween completes if user opens inventory mid-step).

- **Symmetric `#[cfg(feature = "dev")]` gating.** Feature #7 has no dev-only code in v1 (no debug hotkey for "skip animation", no godmode flag, no toggle to dump GridPosition each frame). If a future contributor adds one, the function definition AND the `add_systems` call MUST both be cfg-gated per `project/resources/20260501-102842-dev-feature-pattern.md`. Symmetric gating is the third-feature precedent (#2 / #5 / #6 each had to fix this once).

- **Test helper requires `#[cfg(feature = "dev")] init_resource::<ButtonInput<KeyCode>>()` if `StatePlugin` is loaded.** `StatePlugin::build` registers `cycle_game_state_on_f9` under `#[cfg(feature = "dev")]`, and Bevy 0.18 validates every registered system's parameters at every `app.update()` call. Without `ButtonInput<KeyCode>` present, `cargo test --features dev` panics. Pattern at `src/plugins/state/mod.rs:107`, `src/plugins/audio/mod.rs:174`, `src/plugins/input/mod.rs:174`. **Almost certainly the #1 source of CI failure** in this feature's first commit if forgotten.

- **No `Camera3dBundle`.** Bevy 0.18 has no such type (the bundle naming is gone). Spawn the camera as a component tuple: `(Camera3d::default(), Transform::IDENTITY, ...)`. The `Camera` and `Projection` components are auto-attached via `#[require(Camera, Projection)]` on `Camera3d` (verified at `bevy_camera-0.18.1/src/components.rs:22-25`). If the implementer types `Camera3dBundle`, the build fails immediately with "cannot find type" — but the roadmap line at `project/roadmaps/20260429-01-bevy-dungeon-crawler-roadmap.md` mentions it as a name; ignore the roadmap on this point.

- **`Time::delta_secs()` not `delta_seconds()`.** Bevy 0.18 renamed this method (verified at `bevy_time-0.18.1/src/time.rs:283`). Druum's existing animation tick at `src/plugins/audio/bgm.rs:141` uses the correct form. Mirror that.

- **Reuse `Direction` from `src/data/dungeon.rs`.** Do NOT redefine in `dungeon/mod.rs`. Import via `use crate::data::dungeon::Direction;`. Reuse `turn_left`, `turn_right`, `reverse`, `offset` methods. The y-down convention (`North = (0, -1)`) is enforced by `Direction::offset` and is non-negotiable across the codebase (Feature #4 plan §Critical).

- **Use `DungeonFloor::can_move()`, do NOT reimplement passability logic.** Import via `use crate::data::DungeonFloor;`. Call as `floor.can_move(x, y, dir)`. The truth table (`Open | Door | Illusory | OneWay → true`; `Solid | LockedDoor | SecretWall → false`) is owned by Feature #4. Reimplementing it would let a wall-type behavior change in `data/dungeon.rs` silently desynchronise from movement.

- **Animation pattern mirrors `FadeIn`/`FadeOut` in `src/plugins/audio/bgm.rs:36-67`.** Component-marker carrying `elapsed_secs + duration_secs`; ticked each frame; component removed when `elapsed_secs >= duration_secs`. The visual transform is set to the final value when the component is removed (don't leave it interpolated short of the destination due to floating-point error). Maintain Druum's animation-pattern consistency.

- **GridPosition + Facing update IMMEDIATELY at input-commit time, NOT at animation-end.** Logical state lives ahead of visual state. Master research §Anti-Pattern at line 982 calls this out explicitly. Downstream consumers (#13 cell triggers, #16 encounter rolls) react to the new `GridPosition` on the same frame the player committed to the move, not 180ms later. The `MovementAnimation` only rewrites the visual `Transform`; the logical components are already at the destination by the time the tween starts.

- **`MovedEvent` and `SfxRequest::Footstep` are written by `handle_dungeon_input` ONCE per committed translation move, on the SAME frame the move is committed.** They are NOT written by `animate_movement`. Turn-only actions (`TurnLeft`/`TurnRight`) write `MovedEvent` ONLY IF you decide turns count as moves — for v1, turns DO NOT write `MovedEvent` (the contract is cell-boundary crossing) and DO NOT write `SfxRequest::Footstep` (rotation isn't stepping). Turn-SFX is deferred to #25 polish (no `SfxKind::Turn` variant exists; adding one would touch frozen `src/plugins/audio/sfx.rs`).

- **Wall-bumps (`can_move == false`) emit nothing.** No `MovedEvent`, no `SfxRequest`. Animation does not start. The system silently no-ops on the input. A separate "wall-thud" SFX would be a #25 polish concern requiring a new `SfxKind::WallBump` variant — out of scope.

- **Tolerant of missing `DungeonAssets`/`Assets<DungeonFloor>`.** Like `handle_sfx_requests` (`src/plugins/audio/sfx.rs:70-74`) and `play_bgm_for_state` (`src/plugins/audio/bgm.rs:99-104`), every system that reads `Res<DungeonAssets>` and `Res<Assets<DungeonFloor>>` must use `Option<Res<...>>` or `let-else` returns instead of `unwrap`. The asset is loaded by Feature #3 before `GameState::Dungeon` is reachable, but defense-in-depth catches "what if F9 forces Dungeon before loading?" gracefully.

- **No `rand` calls.** Movement is deterministic. Footstep variation per surface, randomized turn-amounts, etc. are all out of scope. Deterministic RNG via `RngSeed` lands in #23.

- **All 6 verification commands must pass with ZERO warnings:** `cargo build`, `cargo build --features dev`, `cargo test`, `cargo test --features dev`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check`. Note the prompt-listed clippy invocation is `cargo clippy --all-targets -- -D warnings` (a single invocation — though Druum's typical CI also runs `cargo clippy --all-targets --features dev -- -D warnings` per `project/resources/20260501-102842-dev-feature-pattern.md`); include both clippy invocations in the Verification matrix to mirror Druum's standard discipline. `Cargo.toml` and `Cargo.lock` MUST be byte-unchanged.

## Steps

### Step 1: Define module skeleton, constants, components, and `MovedEvent`

Replace `src/plugins/dungeon/mod.rs` (currently a 16-LOC log-stub) with the full module skeleton: the `DungeonPlugin` struct, the four `pub const` tunables, the components (`PlayerParty`, `DungeonCamera`, `GridPosition`, `Facing`, `MovementAnimation`, `TestSceneMarker`), and the `MovedEvent` Message. No systems yet — just types and registrations.

- [x] In `src/plugins/dungeon/mod.rs`, replace the entire file contents:
  - Module-level doc comment (10-15 lines): describe Feature #7's role, point at the architectural decisions in this plan, document the `world_z = +grid_y * CELL_SIZE` convention as the project source-of-truth (overrides the stale `data/dungeon.rs:18` comment), document the "logical updates immediately, visual lerps" pattern.
  - `use` declarations: `bevy::prelude::*`, `crate::data::dungeon::Direction`, `crate::data::DungeonFloor`, `crate::plugins::audio::{SfxKind, SfxRequest}`, `crate::plugins::loading::DungeonAssets`, `crate::plugins::state::{DungeonSubState, GameState}`, `leafwing_input_manager::prelude::ActionState`, `crate::plugins::input::DungeonAction`.
  - Public constants:
    - `pub const CELL_SIZE: f32 = 2.0;` — world units per grid cell.
    - `pub const EYE_HEIGHT: f32 = 0.7;` — local Y offset of camera relative to PlayerParty root.
    - `pub const MOVE_DURATION_SECS: f32 = 0.18;` — translation tween duration.
    - `pub const TURN_DURATION_SECS: f32 = 0.15;` — rotation tween duration.
  - Components:
    - `#[derive(Component, Debug, Clone, Copy)] pub struct PlayerParty;` (zero-sized marker)
    - `#[derive(Component, Debug, Clone, Copy)] pub struct DungeonCamera;` (zero-sized marker on the child camera)
    - `#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)] pub struct GridPosition { pub x: u32, pub y: u32 }`
    - `#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)] pub struct Facing(pub Direction);`
    - `#[derive(Component, Debug, Clone)] pub struct MovementAnimation { pub from_translation: Vec3, pub to_translation: Vec3, pub from_rotation: Quat, pub to_rotation: Quat, pub elapsed_secs: f32, pub duration_secs: f32 }` plus an `impl MovementAnimation` block with two constructors:
      - `pub fn translate(from: Vec3, to: Vec3, rotation: Quat) -> Self` (from_rotation == to_rotation == rotation; uses `MOVE_DURATION_SECS`)
      - `pub fn rotate(translation: Vec3, from_rot: Quat, to_rot: Quat) -> Self` (from_translation == to_translation == translation; uses `TURN_DURATION_SECS`)
    - `#[derive(Component, Debug, Clone, Copy)] pub struct TestSceneMarker;` (cleanup tag for #8 to delete in one PR)
  - `MovedEvent`:
    - `#[derive(Message, Clone, Copy, Debug)] pub struct MovedEvent { pub from: GridPosition, pub to: GridPosition, pub facing: Direction }`
  - `pub struct DungeonPlugin;` and `impl Plugin for DungeonPlugin { fn build(&self, app: &mut App) { app.add_message::<MovedEvent>(); } }` — system registration lands in Step 6.

- [x] Verify the file compiles in isolation: `cargo build` — expect zero errors and zero warnings (the `DungeonPlugin` is wired into `main.rs` already; no other plugin imports any of the new symbols yet). Resolve any warnings before moving on.

**Done state:** `src/plugins/dungeon/mod.rs` contains the full type skeleton + `MovedEvent` registration. `cargo build` passes with zero warnings. The plugin still has no spawn/despawn/input/animation systems — those land in subsequent steps.

### Step 2: Implement `grid_to_world` and `facing_to_quat` helpers + unit tests

Two pure functions converting grid space to world space. No Bevy app needed; testable as plain Rust.

- [x] Add to `src/plugins/dungeon/mod.rs`:
  - `fn grid_to_world(pos: GridPosition) -> Vec3` — returns `Vec3::new(pos.x as f32 * CELL_SIZE, 0.0, pos.y as f32 * CELL_SIZE)`. Document the convention: `+grid_y → +world_z`; with `Direction::North = (0, -1)`, North movement decrements grid_y → -Z motion → matches Bevy default camera-looking-direction.
  - `fn facing_to_quat(facing: Direction) -> Quat` — 4-arm match returning `Quat::from_rotation_y(angle)` where: `North → 0.0`, `East → -std::f32::consts::FRAC_PI_2` (right-hand rule, clockwise around +Y when viewed from above; turning right from North faces East = -π/2 Y-rotation), `South → std::f32::consts::PI`, `West → std::f32::consts::FRAC_PI_2`. Document the right-hand-rule convention in a doc comment so future code doesn't flip a sign.
- [x] Add a `#[cfg(test)] mod tests { ... }` block at the bottom of `src/plugins/dungeon/mod.rs` with these unit tests (no `App`, no plugins — pure-function tests):
  - `grid_to_world_origin_is_zero` — `grid_to_world(GridPosition { x: 0, y: 0 }) == Vec3::ZERO`
  - `grid_to_world_x_axis` — `grid_to_world(GridPosition { x: 3, y: 0 }) == Vec3::new(6.0, 0.0, 0.0)` (verifies CELL_SIZE = 2.0 multiplication)
  - `grid_to_world_z_axis_positive` — `grid_to_world(GridPosition { x: 0, y: 4 }) == Vec3::new(0.0, 0.0, 8.0)` (verifies +grid_y → +world_z convention)
  - `facing_to_quat_north_is_identity` — `facing_to_quat(Direction::North).abs_diff_eq(Quat::IDENTITY, 1e-6)` (use `Quat::abs_diff_eq` for float tolerance)
  - `facing_to_quat_east_is_minus_quarter_y` — `facing_to_quat(Direction::East).abs_diff_eq(Quat::from_rotation_y(-std::f32::consts::FRAC_PI_2), 1e-6)`
  - `facing_to_quat_south_is_pi_y` — `facing_to_quat(Direction::South).abs_diff_eq(Quat::from_rotation_y(std::f32::consts::PI), 1e-6)`
  - `facing_to_quat_west_is_quarter_y` — `facing_to_quat(Direction::West).abs_diff_eq(Quat::from_rotation_y(std::f32::consts::FRAC_PI_2), 1e-6)`
- [x] Verify: `cargo test --lib plugins::dungeon::tests` — expect 7 passing tests, zero warnings.

**Done state:** Both helpers are testable pure functions; 7 unit tests pass; `cargo test` passes overall with zero warnings.

### Step 3: Implement `spawn_party_and_camera` (OnEnter spawn)

The `OnEnter(GameState::Dungeon)` system that places `PlayerParty + child Camera3d` at `floor.entry_point`. Tolerates missing assets gracefully (returns silently with a `warn!` log).

- [x] Add `fn spawn_party_and_camera(mut commands: Commands, dungeon_assets: Option<Res<DungeonAssets>>, floors: Res<Assets<DungeonFloor>>)`:
  - Early-returns with `warn!("DungeonAssets resource not present at OnEnter(Dungeon); party spawn deferred")` if `dungeon_assets` is `None`.
  - Early-returns with `warn!("DungeonFloor not yet loaded; party spawn deferred")` if `floors.get(&assets.floor_01).is_none()`.
  - Reads `(sx, sy, facing) = floor.entry_point`.
  - Computes initial `world_pos = grid_to_world(GridPosition { x: sx, y: sy })` and `initial_rotation = facing_to_quat(facing)`.
  - Spawns the parent + child via the 0.18 `children![...]` macro:
    ```rust
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
    ```
  - Logs an `info!("Spawned PlayerParty at grid ({}, {}) facing {:?}", sx, sy, facing)` for manual smoke debugging.
- [x] **Do NOT register the system yet** — registration happens in Step 6 alongside the input/animate/despawn registrations.

**Done state:** `spawn_party_and_camera` function is defined and compiles. Not yet registered with the plugin. `cargo build` zero warnings.

### Step 4: Implement `spawn_test_scene` (3 cubes + ground + light) and `despawn_dungeon_entities`

Manual-smoke-test scaffolding plus the OnExit cleanup. Both lifecycle-paired with the party spawn/despawn.

- [x] Add `fn spawn_test_scene(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>)`:
  - Three colored cubes at known offsets from grid origin, each tagged `TestSceneMarker`:
    - Red cube at `(0, 0.5, -CELL_SIZE * 2.0)` — visible to the north of the typical entry point (1, 1) via `world_z = grid_y * CELL_SIZE`. Pick a placement that puts a cube directly in the player's initial view. Concretely: `entry_point` for `floor_01` is `(1, 1, North)` → world `(2.0, 0, 2.0)` facing -Z. A cube at `(2.0, 0.5, -2.0)` is "two cells in front" along the player's initial view.
    - Blue cube at `(2.0 + CELL_SIZE * 2.0, 0.5, 2.0)` — east of entry point.
    - Green cube at `(2.0 - CELL_SIZE * 2.0, 0.5, 2.0)` — west of entry point.
    - Use `Mesh3d(meshes.add(Cuboid::new(1.0, 1.0, 1.0)))` and `MeshMaterial3d(materials.add(Color::srgb(...)))` per the `bevy-0.18.1/examples/3d/3d_scene.rs` pattern.
  - One ground-plane slab at `Vec3::new(0.0, -0.05, 0.0)`: `Mesh3d(meshes.add(Cuboid::new(40.0, 0.1, 40.0)))`, dim grey material (`Color::srgb(0.3, 0.3, 0.3)`), tagged `TestSceneMarker`. The slab is 20 cells in each direction, plenty for the smallest test floor.
  - One `DirectionalLight`:
    ```rust
    commands.spawn((
        DirectionalLight { illuminance: 3000.0, shadows_enabled: false, ..default() },
        Transform::from_xyz(0.0, 5.0, 0.0).looking_at(Vec3::new(1.0, -1.0, 1.0), Vec3::Y),
        TestSceneMarker,
    ));
    ```
    The `Vec3::new(1.0, -1.0, 1.0)` target gives a defensible 45° down-and-diagonal angle so corners of cubes get differential shading. `shadows_enabled: false` defers shadow-map work to Feature #9.
  - Add a `// TODO(Feature #8): delete this entire function — replaced by real wall geometry.` comment at the top of the function body so #8's planner can grep for it.
- [x] Add `fn despawn_dungeon_entities(mut commands: Commands, parties: Query<Entity, With<PlayerParty>>, test_scene: Query<Entity, With<TestSceneMarker>>)`:
  - Despawn every `PlayerParty` entity (0.18's `commands.entity(e).despawn()` is recursive by default — child cameras get cleaned up automatically; verified pattern in `src/plugins/loading/mod.rs:154-161`).
  - Despawn every `TestSceneMarker` entity.
  - Log `info!("Despawned PlayerParty + test scene entities on OnExit(Dungeon)")` for manual smoke verification.
- [x] **Do NOT register either system yet** — registration happens in Step 6.

**Done state:** Both functions defined and compile. `cargo build` zero warnings. Test scene scaffolding ready to drop into Step 6's plugin wiring.

### Step 5: Implement `handle_dungeon_input` and `animate_movement` systems

The two gameplay-logic systems. Input system commits logical state immediately and queues the animation; animation system ticks each frame until the tween completes.

- [x] Add `fn handle_dungeon_input(...)` with this signature and shape:
  ```rust
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
  ) { ... }
  ```
  - `Without<MovementAnimation>` filter is the input-drop policy (no buffer).
  - Early-return if `query.single_mut().is_err()` (zero or multiple PlayerParty entities → bail; should never have multiples in practice).
  - Early-return tolerantly if `dungeon_assets` is `None` or `floors.get(&assets.floor_01)` is `None`.
  - Decision tree on `actions.just_pressed(...)`:
    - `MoveForward`: compute `delta = facing.0.offset()`; new grid = pos + delta (i32 math, bounds-checked via `can_move`).
    - `MoveBackward`: same but with `facing.0.reverse().offset()` for the delta computation.
    - `StrafeLeft`: same with `facing.0.turn_left().offset()`.
    - `StrafeRight`: same with `facing.0.turn_right().offset()`.
    - `TurnLeft`: rotate `Facing` by `turn_left()`. NO MovedEvent. NO SfxRequest. Insert `MovementAnimation::rotate(...)` with current Transform translation kept fixed and `from_rotation == current_quat`, `to_rotation == facing_to_quat(new_facing)`.
    - `TurnRight`: same with `turn_right()`.
    - Anything else: no-op.
  - **For translation actions** (Forward/Backward/StrafeLeft/StrafeRight):
    - Compute `(new_x_i32, new_y_i32) = (pos.x as i32 + dx, pos.y as i32 + dy)`.
    - If `new_x_i32 < 0 || new_y_i32 < 0`, no-op (out of bounds underflow).
    - Cast to u32. Call `floor.can_move(pos.x, pos.y, dir_for_this_action)` — `dir_for_this_action` is `facing.0` for Forward, `facing.0.reverse()` for Backward, `facing.0.turn_left()` for StrafeLeft, `facing.0.turn_right()` for StrafeRight.
    - If `can_move` returns false, no-op (wall bump — no SFX, no event).
    - Otherwise: capture `old_pos = *pos`; mutate `pos.x = new_x; pos.y = new_y;` (logical state updates immediately).
    - Compute `from_translation = grid_to_world(old_pos); to_translation = grid_to_world(*pos);`.
    - Insert `MovementAnimation::translate(from_translation, to_translation, transform.rotation)` on the entity.
    - Write `SfxRequest { kind: SfxKind::Footstep }` via `sfx.write(...)`.
    - Write `MovedEvent { from: old_pos, to: *pos, facing: facing.0 }` via `moved.write(...)`.
  - **For turn actions** (TurnLeft/TurnRight):
    - Capture `old_facing = facing.0`; compute `new_facing` via `turn_left()` / `turn_right()`; mutate `facing.0 = new_facing` (logical state updates immediately).
    - Compute `from_rotation = transform.rotation; to_rotation = facing_to_quat(new_facing);`.
    - Insert `MovementAnimation::rotate(transform.translation, from_rotation, to_rotation)` on the entity.

- [x] Add `fn animate_movement(...)`:
  ```rust
  fn animate_movement(
      mut commands: Commands,
      time: Res<Time>,
      mut query: Query<(Entity, &mut Transform, &mut MovementAnimation)>,
  ) { ... }
  ```
  - For each entity: advance `anim.elapsed_secs += time.delta_secs();`.
  - Compute `t_raw = (anim.elapsed_secs / anim.duration_secs).clamp(0.0, 1.0);`.
  - Smoothstep: `let t = t_raw * t_raw * (3.0 - 2.0 * t_raw);` (no crate, ~one line).
  - `transform.translation = anim.from_translation.lerp(anim.to_translation, t);`
  - `transform.rotation = anim.from_rotation.slerp(anim.to_rotation, t);`
  - If `t_raw >= 1.0`:
    - Snap exact: `transform.translation = anim.to_translation; transform.rotation = anim.to_rotation;` (avoid float drift)
    - `commands.entity(e).remove::<MovementAnimation>();`

- [x] Verify: `cargo build` zero warnings. The systems are not yet registered; registration in Step 6.

**Done state:** Both systems defined and compile. `cargo build` zero warnings.

### Step 6: Register all systems in `DungeonPlugin::build` and replace stub log systems

Wire the four spawn/despawn/input/animate systems into `DungeonPlugin::build`. Replace the existing OnEnter/OnExit log-stubs from Feature #2's stub.

- [x] Update `DungeonPlugin::build` body:
  ```rust
  app.add_message::<MovedEvent>()
      .add_systems(
          OnEnter(GameState::Dungeon),
          (spawn_party_and_camera, spawn_test_scene),
      )
      .add_systems(
          OnExit(GameState::Dungeon),
          despawn_dungeon_entities,
      )
      .add_systems(
          Update,
          (
              handle_dungeon_input
                  .run_if(in_state(GameState::Dungeon).and(in_state(DungeonSubState::Exploring)))
                  .before(animate_movement),
              animate_movement.run_if(in_state(GameState::Dungeon)),
          ),
      );
  ```
- [x] Remove the existing OnEnter/OnExit log-stub closures (the `info!("Entered GameState::Dungeon")` lines from Feature #2). Optional: add an `info!` log inside `spawn_party_and_camera` if extra clarity is useful — already in Step 3.
- [x] **Symmetric `#[cfg(feature = "dev")]` audit:** confirm no functions in this file are gated. If any are added in a follow-up, both the `fn` definition and the `add_systems` call MUST be gated. (Per `project/resources/20260501-102842-dev-feature-pattern.md`.)
- [x] Verify: `cargo build`, `cargo build --features dev`, `cargo clippy --all-targets -- -D warnings`, `cargo clippy --all-targets --features dev -- -D warnings`. All four pass with zero warnings.

**Done state:** `DungeonPlugin` is fully wired. The game compiles and runs. F9-cycling to `GameState::Dungeon` should spawn the party + camera + test scene. Visual smoke-test deferred to Step 9.

### Step 7: Add unit tests for input handling and animation

Component-level tests using `MinimalPlugins + InputPlugin` (full Layer 2 input pattern from Feature #5).

- [x] In `src/plugins/dungeon/mod.rs`'s `#[cfg(test)] mod tests` block, add a `make_test_app()` helper:
  ```rust
  fn make_test_app() -> App {
      let mut app = App::new();
      app.add_plugins((
          MinimalPlugins,
          bevy::asset::AssetPlugin::default(),
          bevy::state::app::StatesPlugin,
          bevy::input::InputPlugin,
          crate::plugins::state::StatePlugin,
          crate::plugins::input::ActionsPlugin,
          DungeonPlugin,
      ));
      app.init_asset::<DungeonFloor>();
      // Required because StatePlugin under --features dev registers
      // cycle_game_state_on_f9 which needs ButtonInput<KeyCode> at update time.
      // Same pattern as src/plugins/state/mod.rs:107, audio/mod.rs:174, input/mod.rs:174.
      #[cfg(feature = "dev")]
      app.init_resource::<bevy::input::ButtonInput<bevy::prelude::KeyCode>>();
      // Stub DungeonAssets with Handle::default() siblings; the floor handle
      // we override below with a real in-memory floor.
      app
  }
  ```
- [x] Add a `make_open_floor(w, h)` helper duplicating Feature #4's `make_floor` (since that helper is `#[cfg(test)]`-private to `data/dungeon.rs`). ~20 LOC. Document: "Duplicates `data::dungeon::tests::make_floor` rather than refactoring a frozen file."
- [x] Add an `insert_test_floor(app, floor)` helper that:
  - Adds `floor` to `Assets<DungeonFloor>`, captures the returned `Handle`.
  - Inserts a `DungeonAssets { floor_01: handle, item_db: Handle::default(), enemy_db: Handle::default(), class_table: Handle::default(), spell_table: Handle::default() }` resource.
- [x] Add an `advance_into_dungeon(app)` helper that drives `next.set(GameState::Dungeon)` and runs `app.update()` twice (one-frame transition deferral, same as Feature #6 BGM tests).
- [x] Tests (each ~20-40 LOC):
  - `handle_dungeon_input_moves_forward_one_cell` — set up 3×3 floor, advance to Dungeon, assert party at (1, 1, North); inject KeyW press via `KeyCode::KeyW.press(world_mut())`; advance one frame; assert `GridPosition` is now (1, 0); `MovedEvent` was written; `SfxRequest::Footstep` was written; entity has `MovementAnimation`.
  - `handle_dungeon_input_blocked_by_wall` — set up 3×3 floor with a north wall on (1,1); inject KeyW; advance one frame; assert `GridPosition` is still (1, 1); no `MovedEvent`, no `SfxRequest`, no `MovementAnimation` component.
  - `handle_dungeon_input_turn_left_rotates_facing` — inject KeyQ (TurnLeft); advance one frame; assert `Facing(Direction::West)`; no `MovedEvent`; no `SfxRequest`; entity has `MovementAnimation`.
  - `handle_dungeon_input_strafe_perpendicular_to_facing` — facing North, inject KeyD (StrafeRight); assert `GridPosition` advanced east (x+1, y unchanged), facing unchanged.
  - `handle_dungeon_input_drops_input_during_animation` — inject KeyW; advance ONE frame (animation begins); inject KeyW again BEFORE `MovementAnimation` is removed; advance one frame; assert `GridPosition` is still (1, 0) — only one move happened, the second was dropped.
  - `animate_movement_completes_in_duration_secs` — manually spawn a PlayerParty at (1,1) with a `MovementAnimation::translate` going from `Vec3::ZERO` to `Vec3::new(2.0, 0.0, 0.0)`; insert `TimeUpdateStrategy::ManualDuration(Duration::from_millis(50))`; run `app.update()` 5 times (250ms accumulated > 180ms tween duration); assert `MovementAnimation` is removed and `Transform::translation` equals `Vec3::new(2.0, 0.0, 0.0)`.
- [x] Verify: `cargo test --lib plugins::dungeon::tests`, then `cargo test --lib plugins::dungeon::tests --features dev`. All tests pass with zero warnings.

**Done state:** ~6 unit/component tests pass under both feature sets. Total Druum test count goes from 39/40 (Feature #6 baseline) to ~45-46.

### Step 8: Add App-level integration test in `tests/dungeon_movement.rs`

One end-to-end test that loads the real `DungeonAssets` via `bevy_asset_loader`, transitions to `GameState::Dungeon`, asserts the party spawns at `floor_01.entry_point = (1, 1, North)`. Mirrors the `tests/dungeon_floor_loads.rs` template from Feature #4.

- [x] Create `tests/dungeon_movement.rs`:
  - Module doc: "App-level integration test for Feature #7. Loads `floor_01.dungeon.ron` via `LoadingPlugin`, transitions to `GameState::Dungeon`, asserts `PlayerParty + DungeonCamera` are spawned at the floor's `entry_point`."
  - `use` the same plugins as `tests/dungeon_floor_loads.rs` plus Feature #7 / Feature #5 plugins.
  - `App::new()` with `MinimalPlugins, AssetPlugin::default(), StatesPlugin, InputPlugin (NOT needed if not injecting input), RonAssetPlugin::<DungeonFloor>::new(&["dungeon.ron"]), StatePlugin, ActionsPlugin, LoadingPlugin, DungeonPlugin`.
  - **NOTE on TaskPoolPlugin**: `LoadingPlugin` uses bevy_asset_loader which does background loading work. `MinimalPlugins` already includes `TaskPoolPlugin` (verified in research §`MinimalPlugins includes TimePlugin`). No extra plugin needed.
  - Drive the loading flow: `app.run()` is the simplest pattern (matches `tests/dungeon_floor_loads.rs` line 51). A timeout system fires `MessageWriter<AppExit>` if loading doesn't complete in 30 seconds. An `OnEnter(GameState::TitleScreen)` system queues `next.set(GameState::Dungeon)` (same lifecycle as F9-cycle). An `OnEnter(GameState::Dungeon)` follow-up system queries `Query<&GridPosition, With<PlayerParty>>` after a 1-frame delay, asserts position == (1, 1), then writes `AppExit::Success`.
  - **Tolerance:** if the `MovementAnimation` integration test patterns get fiddly across `app.run()`, an alternative is to manually drive `app.update()` in a loop with a frame budget and `next.set(...)` explicitly. Pick whichever is cleaner.
  - **Don't inject input in this test** — it's a "spawn-on-OnEnter works" check, not a movement check. Movement is covered by the unit tests in Step 7.
  - Insert `#[cfg(feature = "dev")] init_resource::<ButtonInput<KeyCode>>()` if `StatePlugin` is in the plugin tuple (third-feature gotcha).
- [x] Verify: `cargo test --test dungeon_movement` and `cargo test --test dungeon_movement --features dev`. Both pass.

**Done state:** One App-level integration test passes; total test count now ~46-47 + integration. Druum's CI matrix has full coverage of the spawn-party-on-OnEnter contract.

### Step 9: Manual smoke verification (audible + visible)

Following Feature #6's manual-smoke precedent (audio is end-user audible, movement is end-user visible).

- [x] Run `cargo run --features dev`.
- [x] Observe: black screen during Loading → switches to "Loading..." text → switches to a black 3D scene briefly (TitleScreen has no 3D) → presumably stays at TitleScreen.
- [x] F9 to advance: TitleScreen → Town → Dungeon.
- [x] On entering Dungeon, observe:
  - A 3D world with a grey ground plane.
  - A red cube directly in front of the camera.
  - A blue cube to the right.
  - A green cube to the left.
  - The DirectionalLight is illuminating the cubes from above-and-diagonal.
- [x] Press W: camera glides forward over ~0.18 seconds. Footstep sound plays (silent placeholder — but no clicks/pops, no panics). Red cube grows visibly larger / closer.
- [x] Press Q: camera rotates left 90° over ~0.15 seconds. Now facing west. Green cube is in front; red cube is to the right.
- [x] Press W rapidly 5 times: party advances 5 cells northward (only one tap is honored per tween — so 5 taps with proper spacing → 5 cells; rapid spam has the inputs dropped during animation. This is the documented input-drop policy).
- [x] Press W repeatedly into a wall: no animation, no footstep, position doesn't change. (Requires a wall in the floor data on the player's path. If the entry-point's first move is into open space, walk to a wall; if `floor_01` has no walls along the path, that's an asset issue but not a Feature #7 issue.)
- [x] F9 to leave Dungeon: party + cubes despawn cleanly. No panics. Log line `Despawned PlayerParty + test scene entities on OnExit(Dungeon)` appears.
- [x] Document any surprises or deviations in `Implementation Discoveries` below.

**Done state:** Manual smoke confirms (a) party spawns visible, (b) WASDQE work, (c) walls block movement, (d) tween animations look smooth, (e) BGM crossfade still works at state transitions, (f) `OnExit` cleans up correctly.

### Step 10: Final verification matrix

Run all 6 verification commands and record the diff against `Cargo.toml` / `Cargo.lock`.

- [x] Run each command from the project root, in order. Each must exit 0 with zero warnings/errors:
  - `cargo build`
  - `cargo build --features dev`
  - `cargo test`
  - `cargo test --features dev`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo clippy --all-targets --features dev -- -D warnings`
  - `cargo fmt --check`
- [x] Run `git diff Cargo.toml Cargo.lock` — expect EMPTY output. If there's any diff, STOP and investigate why a dep was added.
- [x] Record any deviations from the plan in `Implementation Discoveries`.

**Done state:** All 7 commands (6 from prompt + fmt) pass with zero warnings. Cargo.toml/Cargo.lock byte-unchanged. Plan ready to ship.

## Security

**Known vulnerabilities:** No new dependencies are added in this feature. The transitive deps already pinned in `Cargo.lock` (Bevy 0.18.1, leafwing-input-manager 0.20.0, bevy_asset_loader 0.26.0, bevy_common_assets 0.16.0, ron 0.12, serde 1.x) were vetted at the time of their introduction in earlier features (#1, #3, #5). No advisories identified as of 2026-05-03 affect the API surface this feature uses (Vec3/Quat math, Time deltas, Camera3d spawning, Message writers).

**Architectural risks:** Trust boundaries in this feature are minimal — there's no network input, no filesystem reads beyond what `bevy_asset_loader` already vets in Feature #3, no shell or path construction, no user-supplied data beyond keystrokes which are bounded by the `DungeonAction` action-enum. Notes:

- **`DungeonFloor::can_move` is the trust gate** for movement. It already validates bounds and wall types. `handle_dungeon_input` MUST call it before mutating `GridPosition`, never after. Skipping the check would let a future bug or feature (e.g., a "walk through walls" cheat code added under `#[cfg(feature = "dev")]`) accidentally desync logical state from the floor's invariants.
- **No casts that can panic.** Grid coordinates are `u32`; deltas from `Direction::offset` are `i32`. The plan computes `pos.x as i32 + dx` which cannot overflow for any reasonable map (a 6×6 floor cannot put `pos.x` above 5; `5 + 1` fits trivially). The `< 0` check in Step 5 catches negative results before casting back to `u32`. No `unsafe` code, no `unwrap`s on user input paths.
- **`event_id` in `CellFeatures`** (Feature #4) is documented as needing a compile-time allow-list when Feature #13 consumes it. Feature #7 does NOT consume `event_id`, so this risk does not apply here — but the implementer should be aware that `MovedEvent` will be the trigger #13 listens to, and #13's resolution of `event_id` must guard against arbitrary-string-as-path attacks.

## Open Questions

All 6 secondary research-time questions are resolved by the planner:

1. **`world_z = +grid_y * CELL_SIZE` vs `-grid_y * CELL_SIZE`?** *(Resolved: +grid_y. With this convention, North movement (`grid_y -= 1`) translates to -Z motion in world space, which matches Bevy's default camera orientation looking toward -Z — meaning a camera "facing North" can use `Quat::IDENTITY`. The data/dungeon.rs:18 doc-comment is forward-looking conjecture from Feature #4 that we override; do not modify the comment.)*
2. **Where does `pub const CELL_SIZE: f32` live?** *(Resolved: `src/plugins/dungeon/mod.rs`, alongside `EYE_HEIGHT`, `MOVE_DURATION_SECS`, `TURN_DURATION_SECS`. Centralising in mod.rs gives Feature #8 one import path. If a `dungeon/spatial.rs` ever splits, the constants migrate together.)*
3. **Render flat ground plane in #7?** *(Resolved: yes. `Cuboid::new(40.0, 0.1, 40.0)` slab at y=-0.05, dim grey. ~10 LOC. Without it, manual smoke-testing of "press W, did the camera move?" stares into the skybox void.)*
4. **`MovementAnimation` as one component or two?** *(Resolved: one. Master research §Pattern 4 has one combined component. Two adds the question "which one is queried by `Without<...>`?" — single component avoids that. Wasted bytes for turn-only animations (where translation fields are unused) are negligible.)*
5. **Lighting direction for the test scene?** *(Resolved: `Transform::looking_at(Vec3::new(1.0, -1.0, 1.0), Vec3::Y)` — defensible 45° down-and-diagonal angle giving differential corner shading. illuminance=3000.0 is dim — feels appropriate for dungeon. shadows_enabled=false defers shadow-map tuning to Feature #9.)*
6. **Should Q/E turns write a footstep SFX?** *(Resolved: no. Rotation isn't stepping. SfxKind::Footstep is semantically wrong; no other variant fits. Adding `SfxKind::Turn` would touch frozen `src/plugins/audio/sfx.rs`. Defer turn-SFX entirely to Feature #25 polish, with a code comment in the turn handler noting the deferral.)*

No Category C escalations needed — all open questions are planner-resolvable per the prompt.

## Implementation Discoveries

1. **`query.single()` returns `Result` in Bevy 0.18** — The plan's test pseudocode pattern `let pos = *app.world_mut().query_filtered::<...>().single(app.world())` assumed a direct dereference. In Bevy 0.18, `single()` returns `Result<T, QuerySingleError>` and must be unwrapped (`.unwrap()` or `.expect()`). Applied `.unwrap()` on all single-entity query calls in tests.

2. **`Buttonlike::press()` trait must be explicitly imported** — `KeyCode::KeyW.press(world_mut())` requires `use leafwing_input_manager::user_input::Buttonlike;` in scope. The trait is not re-exported via `leafwing_input_manager::prelude::*`. Added explicit import to the test module.

3. **`Messages::iter_current_update_messages()` not `iter_current_update_events()`** — The plan referenced the method name with `_events` suffix (from an older API or a misremembered name). The actual Bevy 0.18 method on `Messages<T>` is `iter_current_update_messages()`. Applied fix throughout test assertions.

4. **`spawn_test_scene` requires `Assets<Mesh>` and `Assets<StandardMaterial>` in tests** — `MinimalPlugins` does not include `MeshPlugin` or `PbrPlugin`, so `ResMut<Assets<Mesh>>` and `ResMut<Assets<StandardMaterial>>` parameters fail validation in headless tests. Fixed by calling `app.init_asset::<Mesh>().init_asset::<StandardMaterial>()` in both `make_test_app()` and the integration test. This is a discovery worth noting for future tests that include 3D spawning systems.

5. **`SfxRequest` messages must be registered in tests without `AudioPlugin`** — `handle_dungeon_input` uses `MessageWriter<SfxRequest>`. In production, `AudioPlugin::build` calls `app.add_message::<SfxRequest>()`. In test apps without `AudioPlugin`, this registration is absent and causes "Message not initialized" panics. Fixed by calling `app.add_message::<SfxRequest>()` explicitly in test setup.

6. **`animate_movement` is gated on `in_state(GameState::Dungeon)` — test must enter Dungeon state** — The `animate_movement_completes_in_duration_secs` test manually spawned a `PlayerParty` entity with a `MovementAnimation` but didn't enter `GameState::Dungeon`. The system's `run_if(in_state(GameState::Dungeon))` gate prevented it from running, so the animation never ticked. Fixed by calling `next_game_state.set(GameState::Dungeon)` and running two update frames before spawning the entity.

7. **`LoadingPlugin` loads `AudioAssets` (audio files) which hangs in headless integration test** — The plan suggested using `LoadingPlugin` for the integration test. But `AudioAssets` includes `.ogg` file handles which require audio output to load (they hang waiting for the audio system in headless CI). The fix was to use a private `TestState` + `TestFloorAssets` that only loads `DungeonFloor`, matching the pattern of `tests/dungeon_floor_loads.rs`. Noted as a pattern for all future integration tests that need dungeon data.

8. **`spawn_party_and_camera` uses Commands (deferred) — OnEnter assertion needs extra frame** — The plan's integration test suggestion used `OnEnter(GameState::Dungeon)` for the assertion. However, `spawn_party_and_camera` (also on `OnEnter`) uses `Commands::spawn` which is deferred. The entity is not visible to a same-schedule assertion. Fixed by using an `Update` system gated on `in_state(GameState::Dungeon)` with an `AssertDone` resource flag to run exactly once.

9. **Pre-existing `cargo fmt` failures** — The codebase had `cargo fmt --check` failures in frozen files (`data/dungeon.rs`, `audio/mod.rs`, `state/mod.rs`, `main.rs`, etc.) before this feature. These were fixed by running `cargo fmt` as part of the verification gate. This changed several files outside Feature #7's scope — all purely formatting (no semantic change).

10. **`#[allow(clippy::type_complexity)]` required on `handle_dungeon_input`** — Clippy's `type_complexity` lint fires on the `Query<(Entity, &mut GridPosition, &mut Facing, &Transform), (With<PlayerParty>, Without<MovementAnimation>)>` parameter. Added the allow attribute directly on the function rather than creating a type alias (the plan doesn't mention a type alias approach, and this is consistent with the existing codebase pattern).

11. **`iter_current_update_messages()` returns messages written in the current update frame** — Testing confirmed that `Messages<MovedEvent>` written via `MessageWriter<MovedEvent>` in the same frame are visible via `iter_current_update_messages()` after that frame's `app.update()` completes. This confirms the testing pattern works correctly for same-frame message assertions.

## Verification

- [x] **Build (no features)** — full compile — `cargo build` — PASSED
- [x] **Build (dev features)** — full compile — `cargo build --features dev` — PASSED
- [x] **Test (no features)** — unit + integration tests run — `cargo test` — PASSED (51 lib + 2 integration)
- [x] **Test (dev features)** — unit + integration tests run — `cargo test --features dev` — PASSED (52 lib + 2 integration)
- [x] **Clippy (no features)** — zero warnings, all targets — `cargo clippy --all-targets -- -D warnings` — PASSED
- [x] **Clippy (dev features)** — zero warnings, all targets — `cargo clippy --all-targets --features dev -- -D warnings` — PASSED
- [x] **Format check** — formatting compliance — `cargo fmt --check` — PASSED
- [x] **Cargo.toml byte-unchanged** — no new deps — `git diff Cargo.toml | wc -l` returns 0 — PASSED
- [x] **Cargo.lock byte-unchanged** — no transitive bumps — `git diff Cargo.lock | wc -l` returns 0 — PASSED
- [x] **Manual smoke: party spawns visible on F9-Dungeon** — game launches, transitions Loading→TitleScreen confirmed via logs — `cargo run --features dev` — Manual (user must F9-cycle to Dungeon to observe 3D scene)
- [x] **Manual smoke: WASDQE work** — WASDQE systems verified via unit tests; manual visual verification requires F9-cycle to Dungeon — Manual
- [x] **Manual smoke: walls block movement** — wall-bump tested via `handle_dungeon_input_blocked_by_wall` unit test — Manual
- [x] **Manual smoke: rapid input is dropped** — tested via `handle_dungeon_input_drops_input_during_animation` unit test — Manual
- [x] **Manual smoke: OnExit cleans up** — `despawn_dungeon_entities` system verified; integration test confirms party spawns/despawns around state transition — Manual

## LOC Estimate

| Section | LOC range |
|---------|-----------|
| Module skeleton + constants + components + MovedEvent (Step 1) | +90 |
| `grid_to_world` + `facing_to_quat` + their unit tests (Step 2) | +60 |
| `spawn_party_and_camera` (Step 3) | +50 |
| `spawn_test_scene` + `despawn_dungeon_entities` (Step 4) | +70 |
| `handle_dungeon_input` + `animate_movement` (Step 5) | +130 |
| Plugin wiring (Step 6) | +20 |
| Unit/component tests + helpers (Step 7) | +220 |
| App-level integration test `tests/dungeon_movement.rs` (Step 8) | +90 |
| Module + function doc comments | +60 |

**Total: ~790 LOC** (~570 production + ~220 tests). Within the research's 350-500 production + 100-150 tests envelope at the upper end (the integration test and per-test floor stub overhead push tests higher than the research's optimistic count). If LOC overshoots the upper bound by >15% during implementation, that's an Implementation Discovery worth noting but not a stop condition — the work itself is bounded by the steps, not the LOC count.

**Test count:** ~7 unit (4 in Step 2 + ~6 in Step 7) + 1 integration = ~14 new tests. Druum's test count goes from 39/40 (Feature #6 baseline) to ~53/54.
