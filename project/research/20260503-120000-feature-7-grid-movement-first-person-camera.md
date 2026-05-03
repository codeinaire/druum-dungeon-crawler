# Feature #7: Grid Movement & First-Person Camera — Research

**Researched:** 2026-05-03
**Domain:** Bevy 0.18.1, ECS, 3D transform/animation, first-person grid-based dungeon crawler
**Confidence:** HIGH (Bevy first-party APIs verified on-disk; integration contracts verified on-disk; architectural choices grounded in master research and Druum's prior-feature precedent)

## Summary

Feature #7 turns grid + input + audio (#4/#5/#6) into a visible, navigable first-person dungeon. Recommended shape:

- **Single `PlayerParty` entity** holding `(GridPosition, Facing(Direction), Transform, Visibility)`, with a child `Camera3d` entity at eye height. Spawn on `OnEnter(GameState::Dungeon)`, despawn on `OnExit`.
- **Animation = component-marker pattern** (mirrors Druum's existing `FadeIn`/`FadeOut` in `src/plugins/audio/bgm.rs:36-67`): a `MovementAnimation { from, to, from_rot, to_rot, elapsed_secs, duration_secs }` component is added on input, ticked by an `animate_movement` system, removed when finished. **No queueing**: `handle_movement_input` reads `Without<MovementAnimation>`, so a second press during animation is dropped. This is the "MovementLocked" behavior the prompt asks about — it falls out for free from the query filter, no new flag needed.
- **`MovedEvent` is a `Message`** (Bevy 0.18 family-rename), registered with `app.add_message::<MovedEvent>()`. Written by the system that initiates movement, consumed by `handle_sfx_requests`-style downstream subscribers (Feature #13/#16/#22).
- **Cell size = 2.0 world units, eye height = 0.7, FOV unchanged at PI/4 (45° vertical).** Smaller than master research's 4.0 because Wizardry-style corridors feel claustrophobic at ~2m wide; this also keeps Feature #8's wall geometry tractable (3-meter ceiling with 2m × 3m wall planes is exactly one human-scale corridor).
- **Move animation duration = 0.18s, turn animation duration = 0.15s.** Tweened, smoothstep-interpolated. Tuned values; tunable in #25 polish.
- **Test scaffolding = three colored cubes** at known grid coords, no textures. Just enough visual landmarks to confirm "I moved one cell forward, the cube I was facing is now closer." Delete when Feature #8 lands.

**No new dependencies.** Everything (`Vec3::lerp`, `Quat::slerp`, `Quat::from_rotation_y`, `Time::delta_secs`, `Cuboid::new`, `StandardMaterial`, `MeshMaterial3d`, `Mesh3d`, `Camera3d`, `PointLight`, `DirectionalLight`, `AmbientLight`) ships in Bevy 0.18.1.

LOC estimate: **350–500 LOC** including tests (master research said 250-400; we're a bit higher because the `MovedEvent` registration, child-camera spawn, and three-cube test scene add ~80 LOC the master didn't account for).

**Primary recommendation:** Mirror Druum's audio crossfade architecture precedent — component-marker animation, message-based downstream signaling, `OnEnter`/`OnExit`-driven entity lifecycle — and explicitly inline the master research's Pattern 4 with the corrections noted in `## Integration Contracts` (`Direction::offset` y-down, world_z = +grid_y * CELL_SIZE).

---

## Recommendations Summary (the 6 architectural questions)

| # | Question | Recommendation | Confidence |
|---|----------|----------------|------------|
| 1 | Movement animation: instant vs tween | **Tween, 0.18s, smoothstep** | HIGH |
| 2 | Turn animation: instant vs tween | **Tween, 0.15s, smoothstep** (consistent with #1) | HIGH |
| 3 | Movement queue / input buffering | **None — drop input during animation via `Without<MovementAnimation>` query filter** | HIGH |
| 4 | Test scene scaffolding | **Three colored cubes at known grid coords (placeholder, deleted in #8)** | MEDIUM |
| 5 | Eye height + FOV | **eye_height = 0.7, FOV = π/4 (45° vertical, Bevy default — leave unchanged)** | MEDIUM |
| 6 | Cell unit scale | **CELL_SIZE = 2.0 world units (smaller than master research's 4.0 for genre-correct corridor feel)** | MEDIUM |

Rationale for each is in `## Architectural Decisions` below.

---

## Bevy 0.18 API Verification (HIGH confidence — all on-disk verified)

All paths are absolute under `~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/`.

### Camera3d — components-only, no `Camera3dBundle`

`Camera3d` is a single `Component` with `#[require(Camera, Projection)]`. The "bundle" naming is gone in 0.18.

```rust
// Verified at bevy_camera-0.18.1/src/components.rs:22-25
#[derive(Component, Reflect, Clone)]
#[reflect(Component, Default, Clone)]
#[require(Camera, Projection)]
pub struct Camera3d { /* ... */ }
```

**Canonical 0.18 spawn pattern** (verified at `bevy-0.18.1/examples/3d/3d_scene.rs:39-42`):

```rust
commands.spawn((
    Camera3d::default(),
    Transform::from_xyz(-2.5, 4.5, 9.0).looking_at(Vec3::ZERO, Vec3::Y),
));
```

The required components (`Camera`, `Projection` — defaulting to `PerspectiveProjection`) are auto-attached by Bevy's required-components machinery. No bundle needed. **Roadmap §What This Touches mentions `Camera3dBundle` — that name does NOT exist in 0.18; the planner should ignore it.**

### Transform::looking_at and Transform::from_xyz

Both verified. `looking_at` is a builder (consuming `self`):

```rust
// Verified at bevy_transform-0.18.1/src/components/transform.rs:115-117, 183-186
pub const fn from_xyz(x: f32, y: f32, z: f32) -> Self { /* ... */ }

pub fn looking_at(mut self, target: Vec3, up: impl TryInto<Dir3>) -> Self {
    self.look_at(target, up);
    self
}
```

`Vec3::Y` is the conventional up vector (Bevy is y-up world coords).

### Time::delta_secs — NOT delta_seconds

```rust
// Verified at bevy_time-0.18.1/src/time.rs:283
pub fn delta_secs(&self) -> f32 { self.delta_secs }
```

`delta_seconds` does NOT exist in 0.18 (was renamed). All systems should use `time.delta_secs()`. Consistent with `src/plugins/audio/bgm.rs:141` (Druum already uses this).

### PointLight, DirectionalLight, AmbientLight — components, all using `#[require(...)]`

```rust
// Verified at bevy_light-0.18.1/src/point_light.rs:41-49
#[derive(Component, Debug, Clone, Copy, Reflect)]
#[require(CubemapFrusta, CubemapVisibleEntities, Transform, Visibility, VisibilityClass)]
pub struct PointLight { color, intensity, range, radius, shadows_enabled, ... }

// Verified at bevy_light-0.18.1/src/directional_light.rs:58-68
#[derive(Component, Debug, Clone, Copy, Reflect)]
#[require(Cascades, CascadesFrusta, CascadeShadowConfig, ..., Transform, Visibility, VisibilityClass)]
pub struct DirectionalLight { color, illuminance, shadows_enabled, ... }

// Verified at bevy_light-0.18.1/src/ambient_light.rs:9-12
#[derive(Component, Clone, Debug, Reflect)]
#[require(Camera)]
pub struct AmbientLight { color, brightness, affects_lightmapped_meshes }
```

**Important nuance:** `AmbientLight` requires `Camera` — it must be added to the camera entity, not floating. There is also a `GlobalAmbientLight` *resource* (`bevy_light-0.18.1/src/ambient_light.rs:59-78`) inserted by `LightPlugin` that lights the whole scene by default (`brightness = 80.0`). For Feature #7's basic visibility, the default `GlobalAmbientLight` is sufficient — no explicit ambient setup needed. A single `DirectionalLight` for visibility scaffolding is enough.

For minimum viable visibility scaffolding (Feature #7 only), recommend:

```rust
commands.spawn((
    DirectionalLight {
        illuminance: 3000.0,  // dim — feels appropriate for dungeon
        shadows_enabled: false, // shadows defer to #9
        ..default()
    },
    Transform::from_xyz(0.0, 5.0, 0.0).looking_at(Vec3::new(1.0, 0.0, 1.0), Vec3::Y),
));
```

`DirectionalLight` shines along the entity's forward direction (`bevy_light-0.18.1/src/directional_light.rs:24-25` says "shines along the forward direction"). Use `looking_at` to point it diagonally so corners get differential lighting, not flat-shaded boxes. Lighting refinements (point lights, fog, atmosphere) belong in #9.

### Mesh3d, MeshMaterial3d, StandardMaterial

```rust
// Verified at bevy_mesh-0.18.1/src/components.rs:96-98
#[require(Transform)]
pub struct Mesh3d(pub Handle<Mesh>);

// Verified at bevy_pbr-0.18.1/src/mesh_material.rs:39-41
pub struct MeshMaterial3d<M: Material>(pub Handle<M>);

// Verified at bevy_pbr-0.18.1/src/pbr_material.rs:35
pub struct StandardMaterial { base_color, base_color_texture, ..., perceptual_roughness, metallic, emissive, ... }
```

Canonical spawn pattern for a colored cube (verified at `bevy-0.18.1/examples/3d/3d_scene.rs:25-29`):

```rust
commands.spawn((
    Mesh3d(meshes.add(Cuboid::new(1.0, 1.0, 1.0))),
    MeshMaterial3d(materials.add(Color::srgb_u8(124, 144, 255))),
    Transform::from_xyz(0.0, 0.5, 0.0),
));
```

Note `materials.add(Color::...)` works because `From<Color> for StandardMaterial` exists. For full control, use `materials.add(StandardMaterial { base_color: ..., ..default() })`.

### `Cuboid::new` and `Sphere::default()`

```rust
// Verified at bevy_math-0.18.1/src/primitives/dim3.rs:691-694
pub struct Cuboid { pub half_size: Vec3 }

// Verified at bevy_math-0.18.1/src/primitives/dim3.rs:30-33
pub struct Sphere { pub radius: f32 }
```

`Cuboid::new(width, height, depth)` constructs full-size dimensions internally (half_size = vec3 / 2). Both implement `Meshable` for `meshes.add(Cuboid::new(...))`.

### Message family — `MovedEvent` derives `Message`, NOT `Event`

```rust
// Verified at bevy_app-0.18.1/src/app.rs:411
pub fn add_message<M: Message>(&mut self) -> &mut Self { /* ... */ }
```

```rust
// Pattern Druum already uses, verified at src/plugins/audio/sfx.rs:42-45
#[derive(Message, Clone, Copy, Debug)]
pub struct SfxRequest { pub kind: SfxKind }
```

**For Feature #7:** `MovedEvent` MUST derive `Message`, NOT `Event`. Read with `MessageReader<MovedEvent>`, write with `MessageWriter<MovedEvent>`. Register with `app.add_message::<MovedEvent>()`. Same family-rename trap as `StateTransitionEvent`, `AssetEvent`, `KeyboardInput`, `SfxRequest` (see `.claude/agent-memory/researcher/feedback_bevy_0_18_event_message_split.md`).

### OnEnter / OnExit GameState

Already exercised in Druum (`src/plugins/loading/mod.rs:116-117`, `src/plugins/dungeon/mod.rs:12-13`). The pattern is:

```rust
.add_systems(OnEnter(GameState::Dungeon), spawn_dungeon_player)
.add_systems(OnExit(GameState::Dungeon), despawn_dungeon_entities)
```

`OnEnter`/`OnExit` are schedule labels driven by `StateTransition`. Re-verified — works the same as Feature #3.

### `children![ ... ]` macro for spawning child entities

```rust
// Verified at bevy-0.18.1/examples/3d/parenting.rs:43-49
commands.spawn((
    Mesh3d(cube_handle.clone()),
    MeshMaterial3d(cube_material_handle.clone()),
    Transform::from_xyz(0.0, 0.0, 1.0),
    Rotator,
    children![(
        Mesh3d(cube_handle),
        MeshMaterial3d(cube_material_handle),
        Transform::from_xyz(0.0, 0.0, 3.0),
    )],
));
```

This is the 0.18 idiom for spawn-with-children. The child `Transform` is local-relative; world transform is the parent's transform composed with the child's. **Use this for the player + camera relationship**: the camera is a child of `PlayerParty` at local offset `Vec3::Y * EYE_HEIGHT`. Moving/rotating `PlayerParty` carries the camera.

### Time-deterministic tests via `TimeUpdateStrategy::ManualDuration`

```rust
// Verified at bevy_time-0.18.1/src/lib.rs:104-117
pub enum TimeUpdateStrategy {
    #[default] Automatic,
    ManualInstant(Instant),
    ManualDuration(Duration),
    FixedTimesteps(u32),
}
```

For animation tests, `app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(50)))` then call `app.update()` ten times → time advances exactly 500ms total. This is the right pattern for asserting "after `MOVE_DURATION` seconds, `MovementAnimation` was removed and `GridPosition` is at (1, 0)" without flaky wall-clock dependencies.

### MinimalPlugins includes TimePlugin

```rust
// Verified at bevy_internal-0.18.1/src/default_plugins.rs:139-146
pub struct MinimalPlugins {
    bevy_app:::TaskPoolPlugin,
    bevy_diagnostic:::FrameCountPlugin,
    bevy_time:::TimePlugin,        // YES — Time and TimeUpdateStrategy available
    bevy_app:::ScheduleRunnerPlugin,
    /* ... */
}
```

So tests with `MinimalPlugins` already have `Time` and `TimeUpdateStrategy` registered. No extra plugin needed for animation tests.

---

## Architectural Decisions (with rationale)

### 1. Movement animation: tween (HIGH confidence)

**Decision:** Smoothstep tween, 0.18 seconds.

**Rationale:**
- Master research §Anti-Patterns to Avoid (line 977): *"Skipping the movement interpolation: Instant teleportation between grid cells is disorienting. Always animate transitions, even if brief (200-300ms)."*
- Roadmap §Resolved #4 commits to "Modern telegraphed (Etrian style)" UX. Etrian Odyssey uses ~200-250ms tweens. Wizardry Classic was instant; modern Wizardry Trilogy Remaster (2024) tweens.
- Component-marker animation has zero-cost when not animating: `Query<..., Without<MovementAnimation>>` filters the input handler off; `Query<&mut Transform, &mut MovementAnimation>` for `animate_movement` is empty most frames. No animation = no work.
- Druum already has the precedent: `FadeIn`/`FadeOut` in `src/plugins/audio/bgm.rs` use exactly this elapsed-time-tick + remove-component pattern.
- `~30-60 LOC` is a small budget for this much player-feel improvement.

**Alternative considered:** Instant snap. Rejected because it produces a 90s-Wizardry feel that the roadmap explicitly opted out of, and it's actually MORE work to keep the camera-shake/sound-trigger logic feeling polished without the smoothing cushion.

**Tween crate (`bevy_tweening`)?** Not needed and not recommended. The animation is one Vec3::lerp + one Quat::slerp per frame, in a single system. Adding a crate is over-engineering. (Open Question if planner disagrees.)

**Choice of 0.18s vs master research's 0.25s:** 0.18s feels snappier; 0.25s feels heavy when traversing long corridors. Tunable. The exact value is a Feature #25-polish call; pick something reasonable.

### 2. Turn animation: tween (HIGH confidence)

**Decision:** Smoothstep tween, 0.15 seconds (slightly faster than translation move).

**Rationale:**
- Consistency with #1 (mismatch — instant turn + tweened move — feels broken).
- A 90° rotation through 0.18s feels too slow; the spatial shift is much shorter (just the camera spinning), so the eye accepts a faster duration.
- Same `MovementAnimation` component covers both translation and rotation: `from`, `to`, `from_rotation`, `to_rotation`, `duration_secs`, `elapsed_secs`. No new component needed. (Master research's Pattern 4 has this exact shape — line 660-666.)

### 3. Input queueing policy (HIGH confidence)

**Decision:** Drop input during animation. No buffer.

**Rationale:**
- Roadmap §Cons of Feature #7 acknowledges this is an open UX call: *"do you accept the next move while still animating?"*
- The simplest implementation (no buffer) is achieved via the query filter `With<PlayerParty>, Without<MovementAnimation>` in `handle_movement_input`. Master research's Pattern 4 (line 692) uses this. **No "MovementLocked" flag needed** — the absence of the `MovementAnimation` component IS the lock.
- Etrian Odyssey, Wizardry Trilogy Remaster, and Legend of Grimrock all use input-drop (you cannot enter a move during the animation). Modern players are accustomed to this; it's not a regression.
- A buffer adds: a `PendingMove` resource or component, a `consume_pending_move` system that runs at animation-end, edge cases for "what if pending move turns into a wall?". ~50 extra LOC for ambiguous benefit.
- If a v2 polish call adds buffering, the migration is local: replace `Without<MovementAnimation>` filter with a "if animating, store input" branch. Not refactoring debt.

### 4. Test scene scaffolding (MEDIUM confidence)

**Decision:** Three colored cubes at known grid coords inside `OnEnter(GameState::Dungeon)`. Tag with a `DungeonTestScene` marker. Despawn on `OnExit`.

```rust
// Pseudocode
commands.spawn((Mesh3d(cube_red),  MeshMaterial3d(red_mat),  Transform::from_xyz( 0.0, 0.5, -CELL_SIZE * 2.0), DungeonTestScene));
commands.spawn((Mesh3d(cube_blue), MeshMaterial3d(blue_mat), Transform::from_xyz(CELL_SIZE * 2.0, 0.5, 0.0), DungeonTestScene));
commands.spawn((Mesh3d(cube_green), MeshMaterial3d(green_mat), Transform::from_xyz(-CELL_SIZE * 2.0, 0.5, 0.0), DungeonTestScene));
```

Plus a flat ground plane (Cuboid 0.1 thin) so the camera doesn't look into a black void. ~30 LOC.

**Rationale:**
- A single cube is hard to verify against — you can't tell whether you're moving or just rotating.
- Three cubes at orthogonal positions give visual landmarks that confirm grid coords AND facing direction at a glance.
- This is throwaway code: Feature #8's `generate_dungeon_geometry` will replace it. Mark it with a comment (`// REMOVED IN FEATURE #8`) and `#[cfg(feature = "dev")]`-gate? No — Feature #8 needs to delete it cleanly. A `DungeonTestScene` marker component + a `// TODO(Feature #8): delete this entire fn`-comment is the right level of friction.

**Alternative considered:** Just one cube. Rejected — too sparse to verify camera orientation visually.

**Alternative considered:** Spawn the test scene only `#[cfg(feature = "dev")]`. Rejected — Feature #7 has no real walls yet, so `--no-default-features` runs would be a black void with nothing to look at, defeating manual smoke-testing.

### 5. Eye height + FOV (MEDIUM confidence)

**Decision:**
- **Eye height = 0.7 world units** (camera local Y offset from player root).
- **FOV = leave at default (PI/4 = 45° vertical FOV).**

**Rationale (eye height):**
- The genre target is "human party at the cell center." Bevy is y-up; ground at y=0.
- 1.6 (master research's choice) is realistic-human-meters; but with `CELL_SIZE = 2.0`, a 1.6m camera height with 3.0m ceiling produces a "looking up at ceiling" feel, not a "facing forward" feel.
- 0.7 ≈ knee-level realism. Sounds wrong, but at 2-unit cell size it produces the corridor-eye-level feel of Wizardry. This is a function of `CELL_SIZE`, not absolute realism.
- Tunable. Tune in #8/#9 alongside wall geometry.

**Rationale (FOV):**
- Bevy's default is `PerspectiveProjection { fov: PI/4, near: 0.1, far: 1000.0, aspect_ratio: 1.0 }` (verified at `bevy_camera-0.18.1/src/projection.rs:417-426`).
- `aspect_ratio` is auto-updated by `camera_system` to match the window. Don't set it.
- 45° vertical FOV is on the narrow side for first-person; Wizardry-era games used ~60-90° (because of small screens). For a modern monitor, 45° vertical ≈ 70° horizontal at 16:9 — acceptable. If it feels claustrophobic, bump to 60° vertical (`fov: PI/3`). Defer to playtest in #25.
- **Don't override** unless playtest demands it. Adding a `Projection::Perspective(PerspectiveProjection { fov: PI/3, ..default() })` component to the camera is one line if needed later.

**Confidence MEDIUM:** the values are "this should feel OK based on genre conventions and Bevy default arithmetic." A real first-person playtest could push them either way. Document the decision so #25 polish has the context.

### 6. Cell unit scale (MEDIUM confidence)

**Decision:** `CELL_SIZE = 2.0` world units per grid cell. With Feature #8's wall_height = 3.0, this produces a 2m × 3m corridor — believable human-scale dungeon.

**Rationale:**
- Master research uses 4.0; that's wider than typical Wizardry/Etrian corridors and would produce "ballroom" feel at human scale.
- 2.0 keeps the wall-quad aspect ratio close to 1:1.5 (2m wide × 3m tall), which is what stock Polyhaven and CC0 PBR brick textures are usually authored for.
- Smaller cells = more cells visible per frame at the same FOV. With a typical 6×6 floor, this means more landmarks — better for the cerebral spatial-puzzle feel the roadmap commits to (§Resolved #4).
- Pinning here matters because Feature #8's renderer multiplies grid coords by CELL_SIZE in many places. Changing it later is a global s/r and a rebalance of #9's lighting. Pick once, defend.

**Open Question:** should this be `pub const CELL_SIZE: f32 = 2.0;` in `src/plugins/dungeon/mod.rs` or in a `src/plugins/dungeon/grid.rs`? Recommendation: put it in `mod.rs` for now (Feature #7 owns it); migrate to a shared spatial-constants module if Feature #8 needs to import from a different file path. **The planner should make this call.**

---

## Integration Contracts (verified on-disk)

### Feature #4: Direction enum and DungeonFloor

**File:** `src/data/dungeon.rs`

```rust
// Lines 25-32 — y-DOWN convention. North = (0, -1) = decreasing y.
#[derive(Reflect, Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Direction {
    #[default] North,
    South, East, West,
}

// Lines 36-73 — turn_left, turn_right, reverse, offset
impl Direction {
    pub fn turn_left(self) -> Self;
    pub fn turn_right(self) -> Self;
    pub fn reverse(self) -> Self;
    pub fn offset(self) -> (i32, i32); // North=(0,-1), South=(0,1), East=(1,0), West=(-1,0)
}

// Lines 203-218 — passability predicate
impl DungeonFloor {
    pub fn can_move(&self, x: u32, y: u32, dir: Direction) -> bool;
}

// Line 188 — entry_point: where the player starts, with initial facing
pub entry_point: (u32, u32, Direction);
```

**For Feature #7:**
- Use `Direction::offset()` to compute grid deltas. Matches master research Pattern 4.
- Use `DungeonFloor::can_move(x, y, dir)` for collision check. Do NOT reimplement.
- Use `floor.entry_point` to spawn the player at the floor's start position with correct facing.
- **y-down screen → world-z mapping:** Recommended `world_z = +grid_y * CELL_SIZE` (so North movement → -Z, which matches Bevy's "looking forward = -Z" default camera convention). The data/dungeon.rs comment at line 18 mentions `world_z = -grid_y * cell_size` but that requires the camera/north convention to flip; the `+` form is simpler. **Planner must pick one and document.**

### Feature #5: DungeonAction and ActionState

**File:** `src/plugins/input/mod.rs`

```rust
// Lines 70-82 — all variants Feature #7 needs are present
#[derive(Actionlike, PartialEq, Eq, Hash, Clone, Copy, Debug, Reflect)]
pub enum DungeonAction {
    MoveForward,    // ✓
    MoveBackward,   // ✓
    StrafeLeft,     // ✓
    StrafeRight,    // ✓
    TurnLeft,       // ✓
    TurnRight,      // ✓
    Interact,       // (defer to Feature #13)
    OpenMap,        // (defer to Feature #10)
    OpenInventory,  // (defer to Feature #12)
    Pause,          // (defer to UI work)
}
```

**ActionState API** (verified at `leafwing-input-manager-0.20.0/src/action_state/mod.rs:951-974`):

```rust
fn pressed(&self, action: &A) -> bool;
fn just_pressed(&self, action: &A) -> bool;
fn just_released(&self, action: &A) -> bool;
```

Use `just_pressed` for the movement input (one move per key press, NOT one move per frame held — that would be `pressed`).

**System pattern:**

```rust
fn handle_movement_input(
    actions: Res<ActionState<DungeonAction>>,
    mut sfx: MessageWriter<SfxRequest>,
    mut moved: MessageWriter<MovedEvent>,
    dungeon_assets: Res<DungeonAssets>,
    floors: Res<Assets<DungeonFloor>>,
    query: Query<(Entity, &GridPosition, &Facing, &Transform),
                 (With<PlayerParty>, Without<MovementAnimation>)>,
    mut commands: Commands,
) {
    let Ok((entity, pos, facing, xform)) = query.single() else { return };
    let Some(floor) = floors.get(&dungeon_assets.floor_01) else { return };
    if actions.just_pressed(&DungeonAction::MoveForward) { /* ... */ }
    /* ... */
}
.run_if(in_state(GameState::Dungeon).and(in_state(DungeonSubState::Exploring)))
```

**Critical:** state-gate at the SYSTEM level via `.run_if(in_state(...))`, NOT on the plugin (Feature #5's plan §Critical flagged that anti-pattern; Plugin trait has no `run_if`).

### Feature #6: SfxRequest and SfxKind::Footstep

**File:** `src/plugins/audio/sfx.rs`

```rust
// Lines 42-45 — Message API
#[derive(Message, Clone, Copy, Debug)]
pub struct SfxRequest { pub kind: SfxKind }

// Lines 50-57 — Footstep variant present
pub enum SfxKind {
    Footstep,         // ✓ — Feature #7 writes this on translation movement
    Door,             // (defer to #13)
    EncounterSting,   // (defer to #16)
    MenuClick,
    AttackHit,
}
```

**Write idiom:** documented in `src/plugins/audio/sfx.rs:11-13`:

```rust
fn handle_movement_input(mut sfx: MessageWriter<SfxRequest>, ...) {
    /* ... */
    sfx.write(SfxRequest { kind: SfxKind::Footstep });
}
```

**Key call:** Footstep on `MoveForward/MoveBackward/StrafeLeft/StrafeRight` (translation). NOT on `TurnLeft/TurnRight` (rotation only). When motion is blocked by `can_move == false`, no footstep (that's a wall-bump; defer to #9 polish if a separate "thud" SFX is wanted).

### Feature #3: DungeonAssets

**File:** `src/plugins/loading/mod.rs`

```rust
// Lines 29-41 — handle to the asset
#[derive(AssetCollection, Resource)]
pub struct DungeonAssets {
    #[asset(path = "dungeons/floor_01.dungeon.ron")]
    pub floor_01: Handle<DungeonFloor>,
    /* ... */
}
```

**For Feature #7:**

```rust
fn spawn_player_party(
    dungeon_assets: Res<DungeonAssets>,
    floors: Res<Assets<DungeonFloor>>,
    mut commands: Commands,
) {
    let Some(floor) = floors.get(&dungeon_assets.floor_01) else {
        warn!("DungeonAssets present but floor_01 not yet loaded; party spawn deferred");
        return;
    };
    let (sx, sy, facing) = floor.entry_point;
    /* spawn at world_pos = grid_to_world(sx, sy) with facing rotation */
}
```

**Tolerance:** Like `handle_sfx_requests` in `src/plugins/audio/sfx.rs:70-74`, this system should bail silently if `DungeonAssets` isn't loaded yet. `OnEnter(GameState::Dungeon)` runs before `Res<Assets<DungeonFloor>>::get(handle)` returns Some only if Feature #3's loading state ordering completes — which it does (loading-state finishes before TitleScreen and obviously before Dungeon). But defense-in-depth: `if let Some(...)` rather than `unwrap()`.

### Feature #2: GameState::Dungeon and DungeonSubState::Exploring

**File:** `src/plugins/state/mod.rs`

```rust
// Lines 6-15
pub enum GameState { Loading, TitleScreen, Town, Dungeon, Combat, GameOver }

// Lines 17-26
#[source(GameState = GameState::Dungeon)]
pub enum DungeonSubState { Exploring, Inventory, Map, Paused, EventDialog }
```

**For Feature #7:**
- Spawn party on `OnEnter(GameState::Dungeon)` — the camera and player exist for the entire time we're in Dungeon, even if user opens Inventory.
- Run input-handling systems with `.run_if(in_state(GameState::Dungeon).and(in_state(DungeonSubState::Exploring)))` — opening inventory should pause movement.
- Animation system can run with just `.run_if(in_state(GameState::Dungeon))` — if the player opens inventory mid-tween, the tween still finishes (better UX than freezing mid-step).
- Despawn party on `OnExit(GameState::Dungeon)` — same pattern as `LoadingPlugin::despawn_loading_screen` (`src/plugins/loading/mod.rs:154-161`).

---

## Test Strategy

### Unit tests (run in <1s, no external deps)

In `src/plugins/dungeon/mod.rs` `#[cfg(test)] mod tests`:

| Test | What it verifies | Pattern |
|------|------------------|---------|
| `grid_to_world_returns_origin_for_zero_zero` | conversion correctness | pure-function unit test |
| `grid_to_world_y_down_to_z_back` | y-down screen → +z world (or -z, depending on planner choice) | pure-function unit test |
| `facing_to_quat_north_is_zero_y_rotation` | facing → rotation correctness | pure-function unit test |
| `facing_to_quat_east_is_minus_pi_2_y_rotation` | facing → rotation correctness | pure-function unit test |

### Integration tests (Bevy `App::update`-based, ~1-2s each)

These need the **Layer 2** input test pattern (full `InputPlugin`, leafwing message injection). See `.claude/agent-memory/researcher/feedback_bevy_input_test_layers.md` and existing `src/plugins/input/mod.rs:183-188` for the canonical setup. They also need `TimeUpdateStrategy::ManualDuration` for deterministic animation time-steps.

| Test | What it verifies | Special setup |
|------|------------------|---------------|
| `pressing_w_advances_grid_position_one_cell_north` | `MoveForward` → can_move check → `MovementAnimation` → animation completes → `GridPosition` updated | Spawn a 3×3 test floor as `Asset`; spawn player; `Res<Assets<DungeonFloor>>` accessor |
| `pressing_w_against_wall_does_not_move` | `can_move == false` blocks movement; no animation, no MovedEvent, no SFX | as above with wall on north of (1,1) |
| `pressing_w_writes_movedevent` | `MovedEvent` written when movement initiated | check `Messages<MovedEvent>` after update |
| `pressing_w_writes_footstep_sfx` | `SfxRequest { kind: Footstep }` written on movement | check `Messages<SfxRequest>` |
| `pressing_q_advances_facing_left` | `TurnLeft` → animation → facing updated | as above |
| `second_press_during_animation_is_dropped` | input-queueing-policy verification | press W; advance time half a tween; press W again; assert only one move happened |
| `animation_completes_in_duration` | timing correctness | `TimeUpdateStrategy::ManualDuration(50ms)`; tick 4 times for 0.18s tween (4×50=200ms > 180ms); assert `MovementAnimation` removed and `Transform` matches `to` |

### Test infrastructure gaps

The tests above all need a small in-test `DungeonFloor` constructed inline (NOT loaded from RON — too slow). The existing `src/data/dungeon.rs:332-352` `make_floor(w, h)` helper is `#[cfg(test)]`-private to its own module. **Either expose it via `pub(crate)` or duplicate the helper inline in `src/plugins/dungeon/mod.rs::tests`.** Recommend duplicate (not refactoring Feature #4's frozen file). ~20 extra LOC.

**The integration test will need a stub `DungeonAssets` with a `Handle<DungeonFloor>` registered against an inline-constructed floor.** Use `Assets::<DungeonFloor>::add(floor)` to get a handle; insert `DungeonAssets { floor_01: handle, ...other_handles_default() }` directly into the world. Note that `DungeonAssets` has 5 handles total and they all need defaults — `Handle::default()` works for the four we don't exercise.

### Manual smoke (NOT automated — but executable)

Following the Feature #6 manual-smoke precedent (audio is end-user audible), **movement is end-user visible**:

1. `cargo run --features dev`
2. F9 cycle to `GameState::Dungeon`
3. Verify: visible 3D world, three colored cubes at expected positions
4. Press W: camera glides forward 0.18s, footstep audible. Press Q: camera rotates left 0.15s.
5. Press W against the wall (drive into red cube): no animation, no footstep.
6. Press W rapidly 5 times: party advances 5 cells (input not buffered, so each tap during the previous animation is dropped — but new taps after each completes are honored).

**Document this in the implementation plan as the third pillar of verification, alongside unit + integration tests.** Mirrors Feature #6's audio-smoke approach.

### Test count target

Master research + roadmap §Impact Analysis: "+4-6 movement integration tests." Recommend the **7 listed above**, plus 4 unit tests. Total: ~10-11 new tests. Test count goes from 39/40 to ~50/51. Acceptable.

---

## LOC Estimate (with breakdown)

| Item | LOC range |
|------|-----------|
| `src/plugins/dungeon/mod.rs` plugin glue (move from 16 LOC stub) | +10 |
| `GridPosition`, `Facing`, `PlayerParty`, `DungeonTestScene` markers | +30 |
| `MovementAnimation` component | +20 |
| `MovedEvent` message + registration | +15 |
| `grid_to_world`, `facing_to_quat` helpers | +25 |
| `spawn_player_party` (OnEnter) | +60 |
| `spawn_test_scene` (3 cubes + ground + light) | +50 |
| `despawn_dungeon_entities` (OnExit) | +20 |
| `handle_movement_input` system | +90 |
| `animate_movement` system | +35 |
| Module docs + comments | +50 |
| Tests (4 unit + 7 integration + helpers) | +120 |

**Total: ~525 LOC.** (Higher than master research's 250-400 because of the test-scene scaffolding, MovedEvent infra, and lighting/ground scaffolding the master didn't account for. Lower estimate if planner cuts the test scene to one cube: ~450.)

**Range: 350-500 LOC excluding tests; +100-150 LOC for tests; total 450-650.**

## Dependency impact: 0

No new crates. Verified that `Vec3::lerp`, `Quat::slerp`, `Quat::from_rotation_y`, `Time::delta_secs`, `Cuboid`, `Sphere`, `StandardMaterial`, `Mesh3d`, `MeshMaterial3d`, `Camera3d`, `PointLight`, `DirectionalLight`, `AmbientLight`, `PerspectiveProjection`, `Message`, `MessageReader`, `MessageWriter`, `add_message`, `OnEnter`, `OnExit`, `children![]`, `TimeUpdateStrategy::ManualDuration` all ship in Bevy 0.18.1 (already pulled via `bevy/3d` umbrella). `Cargo.toml` and `Cargo.lock` byte-unchanged.

---

## Risks (verification matrix)

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| `cargo clippy --all-targets -- -D warnings` fails on unused `_pending_move`-style scratch field | LOW | RED | Don't add scratch fields; if needed, prefix with `_` and use it |
| `cargo clippy --all-targets --features dev` fails on dev-only debug fn unused warning | MEDIUM | RED | Symmetric `#[cfg(feature = "dev")]` on EVERY dev-only function AND its `add_systems` registration. (Pattern resource at `project/resources/20260501-102842-dev-feature-pattern.md`. Third-feature gotcha: #2 / #5 / #6 all hit this.) |
| `cargo test` fails because dev-feature `cycle_game_state_on_f9` registered but `ButtonInput<KeyCode>` not present | HIGH | RED | Test helper MUST `#[cfg(feature = "dev")] init_resource::<ButtonInput<KeyCode>>()` if `StatePlugin` is loaded. Pattern: `src/plugins/state/mod.rs:107`, `src/plugins/audio/mod.rs:174`, `src/plugins/input/mod.rs:174`. Third-feature precedent. **Almost certainly the #1 source of CI failure** in #7's first PR. |
| Camera spawned outside `DefaultPlugins` context in tests | LOW | RED | Don't run rendering in unit/integration tests. Use `MinimalPlugins` for animation tests; check transforms/grid pos, not pixels. |
| Input test uses `Layer 1` pattern (direct `ButtonInput::press`) for `ActionState` | MEDIUM | RED | Feature #7's input tests must use Layer 2 (full InputPlugin + `KeyCode::KeyW.press(world_mut())` from leafwing's testing utils). Pattern: `src/plugins/input/mod.rs:183-188`. Don't be tempted by the brevity of Layer 1. |
| `Direction::offset` y-down convention misapplied → North movement goes the wrong way visually | MEDIUM | YELLOW | Plan a `direction_offset_yields_n_at_zero_minus_one` test AND a manual smoke step "press W from entry_point, verify the cube ahead of you gets closer." |
| `MovedEvent` registered with `add_event` instead of `add_message` | MEDIUM | RED | Compile error (won't compile against `Message` derive). Caught at first build. Note the trap explicitly in plan. |
| Cell size constant scattered across files → Feature #8 conflicts | MEDIUM | YELLOW | Define `pub const CELL_SIZE: f32 = 2.0` in `src/plugins/dungeon/mod.rs` and import everywhere. |
| Test floor handle vs. real `DungeonAssets::floor_01` divergence | LOW | YELLOW | Use `Handle::default()` for sibling fields in test stub; document why in inline comment. |
| Camera FOV at 45° feels too narrow → playtest discomfort | MEDIUM | YELLOW | Document the value as tunable; #25 polish revisits. NOT a #7 blocker. |
| Animation tests flaky due to wall-clock `delta_secs` | MEDIUM | RED | Use `TimeUpdateStrategy::ManualDuration(Duration::from_millis(50))` in animation tests for determinism. Verified pattern (`bevy_time-0.18.1/src/lib.rs:104-117`). |

The **6 verification commands** referenced in the prompt are the standard Druum CI matrix. All listed risks are addressable; the dev-feature ButtonInput trap is the highest-likelihood blocker.

---

## Open Questions for the planner

These are genuine decision points where research can't make the call:

1. **`world_z = +grid_y * CELL_SIZE` vs `-grid_y * CELL_SIZE`?**
   - The `data/dungeon.rs:18` doc-comment proposes `-grid_y` (so y-down screen → -z world).
   - The simpler arithmetic-with-default-camera-orientation is `+grid_y` (camera looks down -Z; entry-point facing North = camera looking toward +z if `+grid_y`, OR camera looking toward -z if `-grid_y` and facing rotation flipped accordingly).
   - This is a 5-minute decision driven by which feels easier to reason about; both work. Pick one, document, write the unit test.

2. **Where does `pub const CELL_SIZE: f32` live?**
   - Options: (a) `src/plugins/dungeon/mod.rs`, (b) a new `src/plugins/dungeon/spatial.rs`, (c) `src/data/dungeon.rs` (currently a frozen Feature #4 file — DO NOT modify per prompt).
   - Recommend (a) for now; promote later if Feature #8 needs sharing.

3. **Do we render a flat ground plane in Feature #7, or wait for Feature #8?**
   - Without it, the camera looking down sees infinite skybox color — disorienting in manual smoke.
   - With it, ~10 LOC for a `Cuboid::new(20.0, 0.1, 20.0)` slab at y=-0.05.
   - Recommend yes (in the test scene, deleted by #8). Planner confirms.

4. **`MovementAnimation` — single component for both translation and rotation, or two components?**
   - Master research §Pattern 4 has one combined component (cleaner).
   - Argument for two: turn-only animations (Q/E) wouldn't store unused `from`/`to` Vec3.
   - Recommend ONE component, with both rotation and translation fields used. Simpler. Wasted bytes are negligible. Planner confirms.

5. **Diagonal lighting direction for the directional light in test scene** — purely cosmetic. `Vec3::new(1.0, 0.0, 1.0)` is a defensible 45° angle, but a planner who knows what mood feels right should overrule.

6. **Should turn-only actions (TurnLeft/TurnRight) write a footstep SFX?**
   - Recommend: no (rotation isn't stepping).
   - But: "swish" or "armor-shuffle" sound during turn would be nice. SfxKind::Footstep is technically wrong; no other variant is right; adding `SfxKind::Turn` requires modifying frozen `src/plugins/audio/sfx.rs`. **Recommend defer turn-SFX entirely to #25 polish, with a code comment in the turn handler.** Planner confirms or adds a turn-SFX variant before the freeze list catches it.

---

## Sources

### Primary (HIGH confidence, on-disk-verified)

- [`bevy_camera-0.18.1/src/components.rs`](file:///Users/nousunio/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bevy_camera-0.18.1/src/components.rs) lines 22-25, 25-66 — `Camera3d` component with `#[require(Camera, Projection)]`.
- [`bevy_camera-0.18.1/src/projection.rs`](file:///Users/nousunio/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bevy_camera-0.18.1/src/projection.rs) lines 282-296, 417-426 — `PerspectiveProjection { fov, aspect_ratio, near, far }`, default `fov = PI/4`.
- [`bevy_transform-0.18.1/src/components/transform.rs`](file:///Users/nousunio/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bevy_transform-0.18.1/src/components/transform.rs) lines 105-117, 135-186, 459-461 — `Transform::IDENTITY`, `from_xyz`, `from_translation`, `looking_at`, `look_at`.
- [`bevy_time-0.18.1/src/time.rs`](file:///Users/nousunio/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bevy_time-0.18.1/src/time.rs) lines 283-290 — `Time::delta_secs`, `Time::delta_secs_f64`. NO `delta_seconds`.
- [`bevy_time-0.18.1/src/lib.rs`](file:///Users/nousunio/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bevy_time-0.18.1/src/lib.rs) lines 99-119 — `TimeUpdateStrategy` enum with `ManualDuration` variant for deterministic test time.
- [`bevy_light-0.18.1/src/point_light.rs`](file:///Users/nousunio/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bevy_light-0.18.1/src/point_light.rs) lines 41-49, full struct — `PointLight` component with `#[require(...)]`.
- [`bevy_light-0.18.1/src/directional_light.rs`](file:///Users/nousunio/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bevy_light-0.18.1/src/directional_light.rs) lines 58-68 — `DirectionalLight` component with `#[require(...)]`.
- [`bevy_light-0.18.1/src/ambient_light.rs`](file:///Users/nousunio/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bevy_light-0.18.1/src/ambient_light.rs) lines 9-39, 41-78 — `AmbientLight` component (per-camera) and `GlobalAmbientLight` resource (default brightness 80.0).
- [`bevy_mesh-0.18.1/src/components.rs`](file:///Users/nousunio/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bevy_mesh-0.18.1/src/components.rs) lines 96-98 — `Mesh3d(pub Handle<Mesh>)`.
- [`bevy_pbr-0.18.1/src/mesh_material.rs`](file:///Users/nousunio/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bevy_pbr-0.18.1/src/mesh_material.rs) lines 39-46 — `MeshMaterial3d<M>(pub Handle<M>)`.
- [`bevy_pbr-0.18.1/src/pbr_material.rs`](file:///Users/nousunio/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bevy_pbr-0.18.1/src/pbr_material.rs) line 35 — `StandardMaterial`.
- [`bevy_math-0.18.1/src/primitives/dim3.rs`](file:///Users/nousunio/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bevy_math-0.18.1/src/primitives/dim3.rs) lines 30-33, 691-694 — `Sphere::default()`, `Cuboid::new`.
- [`bevy_app-0.18.1/src/app.rs`](file:///Users/nousunio/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bevy_app-0.18.1/src/app.rs) line 411 — `App::add_message<M: Message>`.
- [`bevy_ecs-0.18.1/src/message/message_writer.rs`](file:///Users/nousunio/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bevy_ecs-0.18.1/src/message/message_writer.rs) line 69 — `MessageWriter::write`.
- [`bevy-0.18.1/examples/3d/3d_scene.rs`](file:///Users/nousunio/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bevy-0.18.1/examples/3d/3d_scene.rs) — canonical 0.18 spawn pattern for Camera3d + Mesh3d + PointLight.
- [`bevy-0.18.1/examples/3d/parenting.rs`](file:///Users/nousunio/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bevy-0.18.1/examples/3d/parenting.rs) — canonical 0.18 child-entity spawn pattern using `children![...]` macro.
- [`bevy-0.18.1/examples/3d/fog.rs`](file:///Users/nousunio/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bevy-0.18.1/examples/3d/fog.rs) lines 30-90 — example of `DistanceFog` on Camera3d (relevant to Feature #9, useful context).
- [`leafwing-input-manager-0.20.0/src/action_state/mod.rs`](file:///Users/nousunio/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/leafwing-input-manager-0.20.0/src/action_state/mod.rs) lines 951-1023 — `ActionState::pressed/just_pressed/just_released`.
- [`leafwing-input-manager-0.20.0/src/user_input/keyboard.rs`](file:///Users/nousunio/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/leafwing-input-manager-0.20.0/src/user_input/keyboard.rs) line 67 — `KeyCode::press(world)` testing helper.

### Druum source-of-truth (HIGH confidence — frozen contracts)

- [`src/plugins/state/mod.rs`](file:///Users/nousunio/Repos/Learnings/claude-code/druum/src/plugins/state/mod.rs) — `GameState::Dungeon`, `DungeonSubState::Exploring`. Frozen.
- [`src/plugins/loading/mod.rs`](file:///Users/nousunio/Repos/Learnings/claude-code/druum/src/plugins/loading/mod.rs) lines 29-41 — `DungeonAssets::floor_01: Handle<DungeonFloor>`. Frozen.
- [`src/plugins/input/mod.rs`](file:///Users/nousunio/Repos/Learnings/claude-code/druum/src/plugins/input/mod.rs) lines 70-82, 183-188 — `DungeonAction` enum and Layer-2 test pattern. Frozen.
- [`src/plugins/audio/sfx.rs`](file:///Users/nousunio/Repos/Learnings/claude-code/druum/src/plugins/audio/sfx.rs) lines 42-57 — `SfxRequest` Message, `SfxKind::Footstep` variant. Frozen.
- [`src/plugins/audio/bgm.rs`](file:///Users/nousunio/Repos/Learnings/claude-code/druum/src/plugins/audio/bgm.rs) lines 36-67, 135-150 — `FadeIn`/`FadeOut` component-marker animation precedent. Pattern to mirror.
- [`src/data/dungeon.rs`](file:///Users/nousunio/Repos/Learnings/claude-code/druum/src/data/dungeon.rs) lines 25-74, 192-218 — `Direction` enum + `offset()` + `DungeonFloor::can_move`. Frozen.
- [`src/plugins/dungeon/mod.rs`](file:///Users/nousunio/Repos/Learnings/claude-code/druum/src/plugins/dungeon/mod.rs) — current empty stub Feature #7 fills.

### Druum project conventions (HIGH confidence)

- [`research/20260326-01-bevy-first-person-dungeon-crawler-rpg.md`](file:///Users/nousunio/Repos/Learnings/claude-code/druum/research/20260326-01-bevy-first-person-dungeon-crawler-rpg.md) — master research §Pattern 4 (lines 637-789) for grid movement; §Anti-Patterns (lines 977, 981-982) for "always animate" + "separate logical and visual state."
- [`project/roadmaps/20260429-01-bevy-dungeon-crawler-roadmap.md`](file:///Users/nousunio/Repos/Learnings/claude-code/druum/project/roadmaps/20260429-01-bevy-dungeon-crawler-roadmap.md) lines 389-435, 48-58 — Feature #7 plan + Resolved §4 (modern telegraphed UX).
- [`project/resources/20260501-102842-dev-feature-pattern.md`](file:///Users/nousunio/Repos/Learnings/claude-code/druum/project/resources/20260501-102842-dev-feature-pattern.md) — symmetric `#[cfg(feature = "dev")]` gating discipline.
- [`project/resources/20260501-104450-bevy-state-machine-anatomy.md`](file:///Users/nousunio/Repos/Learnings/claude-code/druum/project/resources/20260501-104450-bevy-state-machine-anatomy.md) — state-driven system gating + one-frame transition deferral.
- [`.claude/agent-memory/researcher/feedback_bevy_input_test_layers.md`](file:///Users/nousunio/Repos/Learnings/claude-code/druum/.claude/agent-memory/researcher/feedback_bevy_input_test_layers.md) — Layer 1/2/3 input test patterns; Feature #7 needs Layer 2.
- [`.claude/agent-memory/researcher/feedback_bevy_0_18_event_message_split.md`](file:///Users/nousunio/Repos/Learnings/claude-code/druum/.claude/agent-memory/researcher/feedback_bevy_0_18_event_message_split.md) — Message vs Event family-rename in 0.18.

---

## Metadata

**Confidence breakdown:**
- Bevy 0.18 API verification: **HIGH** — every claim grounded in on-disk source with line numbers. Camera3d, Transform, Time, lighting, mesh, message API all directly verified.
- Integration contracts (Direction, can_move, DungeonAction, SfxRequest, DungeonAssets, GameState): **HIGH** — verified directly in `src/`.
- Movement architecture (component-marker animation, no input queue): **HIGH** — the existing `FadeIn`/`FadeOut` precedent in Druum's own audio module is byte-for-byte the same pattern shape; master research §Pattern 4 confirms; roadmap §Resolved #4 confirms tween direction.
- Cell size + eye height + FOV recommendations: **MEDIUM** — defensible based on genre conventions but tunable in playtest. Documented as such.
- Test scene scaffolding shape (3 cubes + ground + light): **MEDIUM** — judgment call about manual-smoke usability vs LOC overhead. Reasonable but a planner could justify "1 cube" at lower LOC.
- LOC estimate (350-500 + 100-150 tests): **MEDIUM** — empirically calibrated against Druum's existing plugin sizes; could vary ±30% based on test-helper duplication choices.

**Research date:** 2026-05-03

**Out-of-scope confirmation:** This research deliberately does not address #8 (real walls), #9 (atmosphere), #10 (auto-map), #13 (cell features), #16 (encounters), #22 (FOEs). Feature #7's `MovedEvent` is the contract those features subscribe to.

**No new dependencies. Cargo.toml + Cargo.lock unchanged.**
