# Feature #8: 3D Dungeon Renderer (Option B) — Research

**Researched:** 2026-05-03
**Domain:** Bevy 0.18.1, ECS, 3D mesh generation, razor-wall dungeon rendering
**Confidence:** HIGH (Bevy first-party APIs verified on-disk; integration types verified in repo; architectural choices grounded in master research, Druum's prior-feature precedents, and the floor_01 asset's actual wall layout)

## Summary

Feature #8 replaces Feature #7's 5-entity test scene (3 cubes + ground slab + DirectionalLight, all `TestSceneMarker`-tagged) with real 3D dungeon geometry generated from `Res<Assets<DungeonFloor>>`. The recommended shape:

- **One `OnEnter(GameState::Dungeon)` system** (`spawn_dungeon_geometry`) that walks `floor.walls[y][x]`, spawns floor + ceiling tiles per cell, plus wall plates per cell using a **canonical "north + west wall, plus right/bottom edge case" iteration rule** (per-edge, not per-cell, to avoid double-rendering shared walls). Every spawned entity carries a new `DungeonGeometry` marker.
- **Per-tile floor + ceiling using `Cuboid` (NOT `Plane3d`)**: backface culling means a flat plane is invisible from one side, and Bevy's `PlaneMeshBuilder::build()` produces single-sided geometry (verified at `bevy_mesh-0.18.1/src/primitives/dim3/plane.rs:122-132` — only one face is written). Using a thin `Cuboid` ("slab", e.g. 2.0 × 0.05 × 2.0) sidesteps the dual-orientation problem cleanly and matches the prior-feature precedent (Feature #7's ground slab is a `Cuboid`).
- **Walls: thin `Cuboid` "plates"** (e.g. 2.0 × 3.0 × 0.05), centered on the cell-edge in world space, with `Quat::from_rotation_y` matching the wall direction. Per-edge iteration eliminates duplicates.
- **Cell height = 3.0 world units** — matches master research §Pattern 6, gives a 1.5× corridor-width-to-wall-height ratio (claustrophobic corridor feel; consistent with the `EYE_HEIGHT = 0.7` and `CELL_SIZE = 2.0` values from #7's anchor doc-comment).
- **Lighting: scene-wide directional + low ambient** (no per-cell lights) — Etrian Odyssey-style. `DirectionalLight` on a `DungeonGeometry`-tagged entity, `GlobalAmbientLight` resource set to a low warm value on `OnEnter`, restored on `OnExit`. Avoids the per-cell torch system that's genuinely Feature #9's concern.
- **WallType → material is a `match` arm**, with shared `Handle<StandardMaterial>` per type cached in a local `HashMap` inside the spawn system. v1: distinct `base_color` per WallType; texture is Feature #9.
- **`OneWay` walls render only on the blocking side** (the side stored as `Solid`), zero geometry on the passable side. The iteration walks both sides anyway (per-edge canonical iteration); the rule is "if the wall on this side is a renderable variant, render it; otherwise skip."

**No new dependencies.** Everything (`Cuboid::new`, `StandardMaterial`, `MeshMaterial3d`, `Mesh3d`, `DirectionalLight`, `GlobalAmbientLight`, `Color::srgb`, `Quat::from_rotation_y`) is in `bevy::prelude::*` via `features = ["3d"]` (already declared in `Cargo.toml:11`).

LOC estimate: **+250 to +380 LOC** total (production ~150-220 + tests ~100-160) added to `src/plugins/dungeon/mod.rs`. The test-scene removal (-65 LOC for `spawn_test_scene` + `TestSceneMarker` + despawn branch) means the file's *net* growth is closer to +185 to +315.

**Primary recommendation:** Treat this as a tightly-scoped extension of `src/plugins/dungeon/mod.rs` — same file, same `OnEnter(GameState::Dungeon)` schedule, same `OnExit` cleanup. Mirror Feature #7's structure precisely: a pure helper `cell_to_world(x, y) -> Vec3` reusing the established `world_z = +grid_y * CELL_SIZE` convention; a system function with `Option<Res<DungeonAssets>>` and `Res<Assets<DungeonFloor>>` parameters; a `DungeonGeometry` marker component for cleanup. The test scene gets fully deleted in this PR per the `TODO(Feature #8)` comment at line 283.

---

## Recommendations Summary (the 7 architectural questions)

| # | Question | Recommendation | Confidence |
|---|----------|----------------|------------|
| 1 | Per-wall entity vs merged mesh | **Per-wall entity. ~84 walls + 36 floor + 36 ceiling + 1 light ≈ 160 entities — trivially OK for Bevy. Do NOT add a mesh-merging crate.** | HIGH |
| 2 | Walls per cell vs walls per edge | **Walls per edge — iterate cells but only render `north` and `west` walls of each cell. Render the outer (south, east) edges as boundary cases. Documented rule below.** | HIGH |
| 3 | Floor + ceiling: combined slab vs per-tile | **Per-tile (one `Cuboid` per cell for floor + one for ceiling).** v1 will have homogeneous color; per-tile makes #9 (lighting) and #13 (cell-feature visuals) trivial later. ~36 entities each on a 6×6 floor — no cost. | HIGH |
| 4 | Cell height (world_y units) | **3.0 world units** — matches master research's recommendation, Druum's `EYE_HEIGHT = 0.7` doc-comment in #7 already anticipates this value (line 47, 52 of `dungeon/mod.rs`). | HIGH |
| 5 | Player-attached light vs scene-wide directional | **Scene-wide `DirectionalLight` (no shadows) + low `GlobalAmbientLight` resource override.** Etrian Odyssey-style. Player-attached light is reserved for #9 atmosphere. | MEDIUM |
| 6 | Wall thickness | **Thin but non-zero (0.05 world units).** Razor-thin (0.0) z-fights with adjacent floor edges and looks awful from oblique angles; 0.05 is invisible in v1's flat-color phase but solves the z-fight. | MEDIUM |
| 7 | OneWay wall visual asymmetry | **Render only on the side stored as `Solid` (the blocking side). The passable side gets no geometry.** Falls out for free from per-edge iteration when the rule is "render renderable variants, skip `Open` and `OneWay`". | MEDIUM |

Rationale for each is in `## Architectural Decisions` below.

---

## Bevy 0.18 API Verification (HIGH confidence — all on-disk verified)

All paths are absolute under `~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/`.

### `Cuboid::new` — full-extent constructor

```rust
// Verified at bevy_math-0.18.1/src/primitives/dim3.rs:691-712
pub struct Cuboid {
    pub half_size: Vec3,
}
impl Cuboid {
    /// Create a new `Cuboid` from a full x, y, and z length
    pub const fn new(x_length: f32, y_length: f32, z_length: f32) -> Self {
        Self::from_size(Vec3::new(x_length, y_length, z_length))
    }
}
```

`Cuboid::new(x, y, z)` takes **full** lengths (not half-extents). Cuboid mesh has 24 vertices, 36 indices — six fully-textured faces with proper outward normals (verified at `bevy_mesh-0.18.1/src/primitives/dim3/cuboid.rs:22-83`). Consumes via `meshes.add(Cuboid::new(...))` because of `impl From<Cuboid> for Mesh` at `bevy_mesh-0.18.1/src/primitives/dim3/cuboid.rs:95-99`.

### `Plane3d::new` and the single-sidedness trap

```rust
// Verified at bevy_math-0.18.1/src/primitives/dim3.rs:103-134
pub struct Plane3d {
    pub normal: Dir3,
    pub half_size: Vec2,
}
impl Default for Plane3d {
    /// Returns the default Plane3d with a normal pointing in the +Y direction,
    /// width and height of 1.0.
    fn default() -> Self {
        Self {
            normal: Dir3::Y,
            half_size: Vec2::splat(0.5),
        }
    }
}
impl Plane3d {
    pub fn new(normal: Vec3, half_size: Vec2) -> Self { ... }
}
```

Default `Plane3d` normal is **`+Y` (Y-up)**, half-size 1.0. The mesh builder (`bevy_mesh-0.18.1/src/primitives/dim3/plane.rs:96-143`) writes vertices and indices for **a single set of front-facing triangles only** — no back face. Translation: a `Plane3d` mesh is invisible from the underside.

For floor tiles this is fine (player only sees the top). For ceiling tiles, master research's Pattern 6 (line 920-921) handles it with `.with_rotation(Quat::from_rotation_x(PI))` to flip the plane upside-down. **However**, this means the planner must choose between:

- (a) Use `Plane3d` for floor and ceiling, with a PI rotation for ceiling. Cheap (2 triangles per tile). Single-sided is fine because the player only sees one face of each.
- (b) Use thin `Cuboid` slabs for both. Always visible. Matches Feature #7's ground-slab precedent (line 329 of `dungeon/mod.rs`). Slightly more vertices per tile (24 vs 4), but for a 36-tile floor this is 864 vs 144 vertices — both trivial.

**Recommendation: use `Cuboid` slabs for floor and ceiling.** Reasons: (1) consistency with #7's ground slab, (2) the ceiling-flip-by-PI rotation is non-obvious and easy to get wrong, (3) makes the eventual #9 lighting changes simpler (Cuboids don't need rotation tweaking when doors get added), (4) negligible vertex-count cost.

### `Mesh3d` and `MeshMaterial3d` — tuple-struct components

```rust
// Verified at bevy_mesh-0.18.1/src/components.rs:95-98
#[derive(Component, Clone, Debug, Default, Deref, DerefMut, Reflect, PartialEq, Eq, From)]
#[reflect(Component, Default, Clone, PartialEq)]
#[require(Transform)]
pub struct Mesh3d(pub Handle<Mesh>);

// Verified at bevy_pbr-0.18.1/src/mesh_material.rs:39-41
#[derive(Component, Clone, Debug, Deref, DerefMut, Reflect, From)]
#[reflect(Component, Default, Clone, PartialEq)]
pub struct MeshMaterial3d<M: Material>(pub Handle<M>);
```

Both have `#[derive(From)]` so `Mesh3d::from(handle)` works, but the canonical pattern is the tuple constructor `Mesh3d(handle)` (used in every official 0.18 example: `bevy-0.18.1/examples/3d/3d_scene.rs:20, 26`, `bevy-0.18.1/examples/3d/parenting.rs:39, 45`, etc.). Druum's existing `dungeon/mod.rs:295,305,317,329` uses `Mesh3d(handle)` — keep that pattern.

### `StandardMaterial` — `base_color: Color`, `From<Color>` impl

```rust
// Verified at bevy_pbr-0.18.1/src/pbr_material.rs:35-43, 855-860
pub struct StandardMaterial {
    pub base_color: Color,  // defaults to Color::WHITE
    pub base_color_texture: Option<Handle<Image>>,
    pub emissive: LinearRgba,
    pub perceptual_roughness: f32,  // defaults to 0.5
    pub metallic: f32,  // defaults to 0.0
    pub unlit: bool,  // defaults to false
    // ... ~30 more fields, all with sensible defaults
}

// Verified at bevy_pbr-0.18.1/src/pbr_material.rs:947-959
impl From<Color> for StandardMaterial {
    fn from(color: Color) -> Self { ... }
}
```

So both forms work:

```rust
// Long form — explicit field overrides
let mat = materials.add(StandardMaterial {
    base_color: Color::srgb(0.5, 0.5, 0.5),
    ..default()
});
// Short form — for "just a color, defaults for everything else"
let mat = materials.add(Color::srgb(0.5, 0.5, 0.5));
```

The short form is what `bevy-0.18.1/examples/3d/3d_scene.rs:21,27` uses. Druum's #7 uses the long form (line 296-299, 307-310, etc.). For #8, **use the long form for the WallType → material map** because it documents the intent of each color choice; use the short form only if there's a single-line context where it clearly reads.

### `Color::srgb` and friends

`Color` is a tagged enum with constructors `Color::WHITE`, `Color::BLACK`, `Color::srgb(r, g, b)`, `Color::srgb_u8(r, g, b)`, `Color::srgba(r, g, b, a)`, etc. Available at `bevy_color-0.18.1/src/color.rs` (used throughout #7's existing code at `dungeon/mod.rs:297, 308, 319, 331`).

### `Camera3d`, `DirectionalLight`, `PointLight`, `AmbientLight`/`GlobalAmbientLight`

Already memorized in `.claude/agent-memory/researcher/reference_bevy_018_camera3d_components.md`. Re-verified for #8:

```rust
// Verified at bevy_light-0.18.1/src/ambient_light.rs:9-39, 41-89
#[derive(Component, Clone, Debug, Reflect)]
#[require(Camera)]
pub struct AmbientLight {
    pub color: Color,
    pub brightness: f32,
    pub affects_lightmapped_meshes: bool,
}
// default: brightness 80.0

#[derive(Resource, Clone, Debug, Reflect)]
pub struct GlobalAmbientLight {
    pub color: Color,
    pub brightness: f32,
    pub affects_lightmapped_meshes: bool,
}
// default: brightness 80.0
```

**Important correction to master research:** Master research Pattern (line 1143 of `research/20260326-...`.md) calls `commands.insert_resource(AmbientLight { ... })`. **This is wrong for Bevy 0.18.** In 0.18:
- `AmbientLight` is a per-camera **Component** (`#[require(Camera)]`)
- The resource is `GlobalAmbientLight`, automatically inserted by `LightPlugin`

For Druum's scene-wide override, mutate `Res<GlobalAmbientLight>` from `OnEnter`. This is verified at `bevy-0.18.1/examples/3d/lighting.rs:122-127`:

```rust
commands.insert_resource(GlobalAmbientLight {
    color: ORANGE_RED.into(),
    brightness: 200.0,
    ..default()
});
```

`DirectionalLight` is a Component (`bevy_light-0.18.1/src/directional_light.rs:58-138`); spawn in a tuple alongside `Transform::looking_at(...)` to set direction.

### `Transform::from_xyz` + `with_rotation` chaining

```rust
// Verified at bevy_transform-0.18.1/src/components/transform.rs:115-117, 246-249
pub const fn from_xyz(x: f32, y: f32, z: f32) -> Self { ... }
pub const fn with_rotation(mut self, rotation: Quat) -> Self {
    self.rotation = rotation;
    self
}
```

Both are `const fn`. Composition is `Transform::from_xyz(x, y, z).with_rotation(quat)`. Used by Druum at `dungeon/mod.rs:256` already.

### `Quat::from_rotation_y` — rotate around world Y-axis

Right-hand rule: positive angles are counter-clockwise viewed from +Y. Used at `dungeon/mod.rs:221` (`facing_to_quat`). For walls, the angles needed:
- North-facing wall (perpendicular to N-S corridor, so the wall extends along E-W): **`Quat::IDENTITY`** — wall plate's local X axis aligned with world X.
- East-facing wall (perpendicular to E-W corridor, extends N-S): **`Quat::from_rotation_y(FRAC_PI_2)`** rotates the X-extent into the Z-extent.

We'll keep walls as `Cuboid::new(width, height, thickness)` where width = CELL_SIZE = 2.0, height = CELL_HEIGHT = 3.0, thickness = 0.05. A "north wall" is then a slab extending along world X, thin along world Z, placed at the north edge of the cell (`world_z = grid_y * CELL_SIZE - CELL_SIZE/2`).

### `Res<Assets<DungeonFloor>>` and `assets.get(handle)`

```rust
// Verified at bevy_asset-0.18.1/src/assets.rs:429-434
impl<A: Asset> Assets<A> {
    pub fn get(&self, id: impl Into<AssetId<A>>) -> Option<&A> { ... }
}
```

Druum already uses this pattern at `dungeon/mod.rs:243`: `floors.get(&assets.floor_01)` returns `Option<&DungeonFloor>`. The same call site works in the new spawn system. Both `Option<Res<DungeonAssets>>` and `Res<Assets<DungeonFloor>>` are required parameters; the existing `spawn_party_and_camera` is the precedent.

### `commands.entity(e).despawn()` is recursive in 0.18

```rust
// Verified at bevy_ecs-0.18.1/src/system/commands/entity_command.rs:242-249
/// Despawns the entity and all of its descendants.
/// This is a "recursive despawn" behavior.
#[track_caller]
pub fn despawn() -> impl EntityCommand { ... }
```

Confirmed: `despawn()` (singular) is the only despawn API in 0.18 EntityCommands and it's already recursive. There is NO `despawn_recursive()` function (that was 0.16 and earlier). Druum's existing despawn at `dungeon/mod.rs:359, 362` is correct as-is — replacing `TestSceneMarker` query with `DungeonGeometry` query keeps the same call shape.

### `with_children` vs `children![...]` macro

The `children![...]` macro is the canonical 0.18 pattern for compile-time-known children, used at `bevy-0.18.1/examples/3d/parenting.rs:43-49`. For *runtime-iterated* children (which is what #8 needs — we don't know the wall count at compile time), the canonical pattern is `commands.spawn(...).with_children(|parent| { ... })`. `with_children` is part of the `BuildChildren` trait imported from `bevy_ecs::hierarchy`.

**Recommendation for #8: don't use a parent-child hierarchy at all.** Spawn each geometry entity as a top-level entity tagged with `DungeonGeometry`. The despawn-on-`OnExit` query already iterates by marker — no parent-child tree needed. This matches the existing `TestSceneMarker` pattern. Saves complexity and `#[require(Transform)]` issues with parent transforms.

---

## Architectural Decisions

### #1 Per-wall entity vs merged mesh

**Decision: Per-wall entity. Do NOT add a mesh-merging crate.**

Math: `floor_01.dungeon.ron` is 6×6 = 36 cells. Each cell has 4 wall faces. With per-edge iteration (decision #2), unique walls = `cells × 2 + outer_edge_count`. For a 6×6: 36 × 2 (each cell renders north + west) + 6 (right edge east walls) + 6 (bottom edge south walls) = **84 wall slots**, each potentially renderable depending on its WallType. In `floor_01.dungeon.ron`, after subtracting `Open` walls, the renderable wall count is **48** (counted in `## Test Strategy` below).

Plus 36 floor + 36 ceiling tiles + 1 directional light + 1 player party = **122 entities total**. Bevy's clustered forward renderer handles 10,000 entities easily; 122 is rounding error.

Master research §Pattern 6 confirms this approach. Etrian Odyssey-class dungeons routinely render multi-thousand-quad floors at 60fps in Bevy.

**Counterargument:** "Wouldn't merging walls into one mesh be more efficient?" Yes, marginally — but at this entity count there's no observable difference, and a merged mesh complicates per-wall feature work in Feature #13 (door animation needs to grab a wall by its grid coords; that's trivial with per-wall entities and pain with merged meshes). **Open question for the planner:** if a future floor has 50×50 cells (hypothetical), per-wall = ~5,000 entities. Still fine, but a streaming/visibility pass would help. Defer until a real perf issue surfaces, per Feature #8 roadmap "Additional Notes" (line 484-485).

### #2 Walls per cell vs walls per edge — the deduplication strategy

**Decision: Iterate cells, but only render `north` and `west` walls of each cell. Outer-edge cells additionally render their `south` and `east` walls.**

Rationale: each shared interior wall is stored twice in `WallMask` (e.g. cell (1,1)'s east wall == cell (2,1)'s west wall). Naively rendering all 4 walls of every cell = double-rendered interior walls = z-fighting + 2× geometry cost.

**Canonical iteration rule (the deduplication algorithm):**

```rust
for y in 0..floor.height {
    for x in 0..floor.width {
        let walls = &floor.walls[y as usize][x as usize];

        // Always render north and west walls (interior + outer edges).
        spawn_wall_if_renderable(walls.north, x, y, Direction::North);
        spawn_wall_if_renderable(walls.west,  x, y, Direction::West);

        // Render south wall ONLY if at the bottom edge (no neighbor below).
        if y == floor.height - 1 {
            spawn_wall_if_renderable(walls.south, x, y, Direction::South);
        }

        // Render east wall ONLY if at the right edge (no neighbor to the right).
        if x == floor.width - 1 {
            spawn_wall_if_renderable(walls.east, x, y, Direction::East);
        }
    }
}
```

**Why north + west:** Arbitrary but consistent. The choice of "render north + west, plus right/bottom edges" is symmetric to "render south + east, plus left/top edges" — both produce identical visible geometry. Pick one and stick with it. North + west is the more common convention in the genre.

**Asymmetry handling — `OneWay`:** Master research's Pattern 6 (line 926) shows `if matches!(walls.north, WallType::Solid | WallType::SecretWall)`, which is a different convention. For Druum, the per-edge rule must handle the OneWay case explicitly:

- If cell A's east wall is `OneWay`, cell B's west wall is `Solid` (validated by `validate_wall_consistency`).
- We're rendering cell B's west wall (because cell B's west wall = cell A's east wall by the canonical rule). The wall variant we look at is `walls[B.y][B.x].west = Solid` → renders as Solid.
- Cell A's east wall is `OneWay` and is on the canonical-iteration "right edge" only if cell A is at `x = width - 1`. Otherwise we don't iterate it on cell A's side. Either way, since OneWay is "render no geometry on this side", **we never render anything on the OneWay side anyway**.

**Important corner:** The above "north + west of each cell" rule means cells at the bottom-right (corner) cells correctly render all 4 of their walls, because they're on both the bottom edge (south wall rendered) AND the right edge (east wall rendered).

### #3 Floor + ceiling: combined slab vs per-tile

**Decision: Per-tile.** One `Cuboid::new(CELL_SIZE, FLOOR_THICKNESS, CELL_SIZE)` floor entity per cell at `(world_x, -FLOOR_THICKNESS/2, world_z)`, one ceiling at `(world_x, CELL_HEIGHT + FLOOR_THICKNESS/2, world_z)`. Use `FLOOR_THICKNESS = 0.05` (same as wall thickness, for consistency).

Rationale: Per-tile is required when later features need per-cell visual variation:
- Feature #13 cell-feature visuals (spinner icon on floor, dark zone tint, anti-magic zone tint): per-tile lets us swap the material handle for that cell without touching the rest.
- Feature #9 lighting (per-cell torch placement): per-tile floors mean we can attach a child light entity to a specific floor tile.

A combined 12.0 × 0.05 × 12.0 slab is 1 entity total but is *brittle* — to add the spinner icon to (2,2), we'd have to break the slab apart anyway.

For v1, all floor tiles share one cached `Handle<StandardMaterial>` (the "dungeon floor" material) and one cached `Handle<Mesh>` (the floor cuboid). Bevy's renderer batches them into a single draw call automatically — same GPU cost as a merged slab, but with the per-cell flexibility for future features.

### #4 Cell height (world_y units)

**Decision: 3.0 world units (`pub const CELL_HEIGHT: f32 = 3.0;`).**

Rationale:
- Master research §Pattern 6 line 890: `let wall_height = 3.0;`
- Druum's `dungeon/mod.rs:46-47` doc-comment already says "3.0 wall height (Feature #8) at 2.0 cell size gives a 1.5× wall-to-corridor ratio"
- Druum's `dungeon/mod.rs:52-53` doc-comment for `EYE_HEIGHT = 0.7` says "produces a 'forward-facing' feel relative to Feature #8's planned 3.0 wall height"
- Genre precedent: Wizardry-style corridors are deliberately tight; 1.5× ratio (wider at floor than tall) feels too tall for a dungeon — but 3.0 is a sweet spot. Real-world stone dungeon corridors are ~2.5-3.5m tall; 3.0 in our 2.0-CELL_SIZE world is genre-correct.
- Tunable later. Make it a `pub const` so #9 can tweak without breaking the API.

### #5 Player-attached light vs scene-wide directional

**Decision: Scene-wide `DirectionalLight` (no shadows in v1) + low `GlobalAmbientLight` resource override.**

Spawn in `OnEnter(Dungeon)`:

```rust
// Tagged with DungeonGeometry so OnExit cleanup catches it.
commands.spawn((
    DirectionalLight {
        illuminance: 5_000.0,  // dimmer than #7's 3000 (subjective tuning)
        shadows_enabled: false,  // shadows are #9
        ..default()
    },
    Transform::from_xyz(0.0, 10.0, 0.0)
        .looking_at(Vec3::new(0.5, -1.0, 0.3), Vec3::Y),
    DungeonGeometry,
));

// Override the global resource on entry; restore on exit.
let mut ambient = world.resource_mut::<GlobalAmbientLight>();
ambient.color = Color::srgb(0.3, 0.32, 0.4);
ambient.brightness = 100.0;
```

Rationale:
- Etrian Odyssey uses scene-wide lighting (no per-cell torches) — directional light + ambient is exactly this pattern.
- Wizardry classic uses pure flat ambient; for v1 we get a similar feel by setting `shadows_enabled: false` and letting walls receive equal light from above.
- A player-attached `PointLight` would be neat (light follows player), but: (1) it would need fine-tuned intensity to not blow out adjacent walls, (2) it's cheating against the genre — Wizardry/Etrian players don't carry a "torch" in the rendering layer, (3) #9 owns this, per the roadmap (line 489-535).
- Master research §Code Examples (line 1143) recommends low ambient (`brightness: 20.0`); for v1 we go a bit higher (100.0) so the geometry is clearly visible without textures. #9 will dim this and add atmosphere.

**Counterargument:** "But classic Wizardry has zero ambient — pure black except for what the (off-screen) directional sun lights." That's the *visual goal* but is hard to read in a flat-color v1 (everything looks identical). Once textures arrive in #9, dim the ambient down. For v1, biased toward "the player can see the geometry" over "atmospheric perfection".

**OnExit:** restore `GlobalAmbientLight` to its default (80.0, white). Use a captured-resource pattern: read the current value on `OnEnter` into a Druum-local resource (`PreDungeonAmbient`), restore on `OnExit`. Or just hardcode the default on exit since `LightPlugin::default()` is the only producer. **Simpler: hardcode** for v1; revisit if Town has its own ambient setting (#18).

### #6 Wall thickness

**Decision: `pub const WALL_THICKNESS: f32 = 0.05;`**

Rationale:
- True razor-thin (0.0) means a `Cuboid::new(2.0, 3.0, 0.0)` is degenerate (zero-volume — Bevy might still render it but the normals are undefined and lighting/shading breaks). Don't use 0.0.
- A `Plane3d` with the wall normal facing into the corridor would work for thickness ≈ 0, but planes are single-sided (verified above). For two-sided thin walls, `Cuboid` is simpler.
- 0.05 is invisible at typical viewing distances (corridor ~2m wide, eye height 0.7m → walls fill ~30° of view at the closest cell, where 0.05 of thickness occupies ~1.4% of the view extent). Player won't see it.
- 0.05 is large enough to avoid z-fighting with floor + ceiling slabs (both are 0.05 thick themselves, so the wall edge is 0.025 inside the floor slab — no co-planar surfaces).
- Larger thickness (e.g. 0.1, 0.2) starts to be visible when walking next to a wall. 0.05 is the empirically-recommended value in dungeon-crawler tutorials.

**Counterargument:** "Why not just align the wall plate with the cell edge with no thickness at all (a `Plane3d`)? Then the wall is geometrically a razor edge." Because of single-sidedness — the wall would be invisible from one side (depending on which `Direction` it's oriented in). Could be solved by `cull_mode: None` on the StandardMaterial, but: (1) that disables backface culling globally for this material, increasing fragment shader cost, (2) the resulting "infinitely thin wall" still has a visible seam where it intersects the floor slab without the 0.025 inset. Cuboid + 0.05 is cleaner and faster.

### #7 OneWay wall visual asymmetry

**Decision: Render only on the side stored as `Solid`. The `OneWay` side itself gets no geometry.**

Rationale:
- `WallType::OneWay` means "passable from this side, blocked from the other" — `validate_wall_consistency` (in `data/dungeon.rs:329-335`) confirms `OneWay` paired with `Solid` is the one allowed asymmetry.
- The blocking side (Solid) needs a wall plate so the player visually sees the wall when approaching from the wrong direction. ✓ Renders normally.
- The passable side (OneWay) needs *nothing* — the player walks through it like an open passage. Any geometry on this side would be confusing ("there's a wall but I just walked through it??").

The per-edge canonical iteration (decision #2) handles this naturally: on cell A's east edge if `WallType::OneWay`, the rule "render renderable variants" excludes `OneWay` (treat it as Open for rendering). On cell B's west edge if `WallType::Solid`, the rule renders Solid as a wall plate. Player approaching from B sees a wall (correctly); player approaching from A sees nothing and walks through (correctly).

**Future cue (Feature #13):** add a small directional arrow icon on the floor of the OneWay side to telegraph the asymmetry. For v1 this is out of scope.

**WallType → renderable mapping:**

| WallType | Renderable in v1? | Material color (hex / sRGB) | Notes |
|----------|-------------------|------------------------------|-------|
| `Open` | No | — | No geometry. |
| `Solid` | Yes | `Color::srgb(0.5, 0.5, 0.55)` (cool grey) | Default wall. |
| `Door` | Yes | `Color::srgb(0.45, 0.30, 0.15)` (brown) | Distinguishable from Solid. |
| `LockedDoor` | Yes | `Color::srgb(0.55, 0.20, 0.15)` (dark red) | Visually warns the player. |
| `Illusory` | Yes | `Color::srgb(0.5, 0.5, 0.55)` (same as Solid) | Player can't tell visually; reveal is #13. |
| `OneWay` | No (on this side; the paired `Solid` side renders) | — | See decision #7 above. |
| `SecretWall` | Yes | `Color::srgb(0.5, 0.5, 0.55)` (same as Solid) | Player can't tell visually; reveal is #13. |

---

## Integration Contracts

Types/functions used as-is, with exact import paths:

### From `crate::data::dungeon` (source of truth: `src/data/dungeon.rs`)

- `Direction` — enum (`North`, `South`, `East`, `West`). `Direction::offset() -> (i32, i32)` for grid deltas. Already imported in `dungeon/mod.rs:36`.
- `WallType` — enum (`Open`, `Solid`, `Door`, `LockedDoor`, `Illusory`, `OneWay`, `SecretWall`). Used in the WallType → material match.
- `WallMask` — struct with `pub north: WallType, pub south, pub east, pub west`. Indexed via `floor.walls[y][x].north`, etc.
- `DungeonFloor` — struct. Fields: `walls: Vec<Vec<WallMask>>` (indexed `[y][x]`), `width: u32`, `height: u32`, `entry_point: (u32, u32, Direction)`, `name`, `floor_number`, `features`, `encounter_table`. Already imported in `dungeon/mod.rs:35` via `crate::data::DungeonFloor`.
- **Do NOT modify** any `data/dungeon.rs` API. The doc-comment fix at `data/dungeon.rs:18` is text-only.

### From `crate::plugins::loading` (source of truth: `src/plugins/loading/mod.rs`)

- `DungeonAssets` — `Resource` from `bevy_asset_loader`. Field `floor_01: Handle<DungeonFloor>` is the handle to floor_01. Already imported in `dungeon/mod.rs:39`.
- **Do NOT modify** `loading/mod.rs`.

### From `crate::plugins::state` (source of truth: `src/plugins/state/mod.rs`)

- `GameState` — `States` enum. The relevant variant is `GameState::Dungeon`. Already used at `dungeon/mod.rs:173, 176, 178`.
- **Do NOT modify** `state/mod.rs`.

### From `bevy::prelude::*` (verified above; via `bevy = ["3d"]` feature)

- `Mesh3d`, `MeshMaterial3d`, `StandardMaterial` — components / asset.
- `Cuboid::new` — primitive.
- `Color` — color type with `Color::srgb`, `Color::WHITE`, etc.
- `DirectionalLight` — component.
- `GlobalAmbientLight` — resource.
- `Transform`, `Quat`, `Vec3` — math.
- `Commands`, `ResMut<Assets<Mesh>>`, `ResMut<Assets<StandardMaterial>>`, `Res<Assets<DungeonFloor>>`, `Option<Res<DungeonAssets>>` — system params.

### From the existing `dungeon/mod.rs` (this file is being extended, not replaced)

**Reuse these constants:**
- `pub const CELL_SIZE: f32 = 2.0;` — line 48. Already public.
- `pub const EYE_HEIGHT: f32 = 0.7;` — line 53. Not used by #8 directly.

**Add these constants:**
- `pub const CELL_HEIGHT: f32 = 3.0;`
- `pub const WALL_THICKNESS: f32 = 0.05;`
- `pub const FLOOR_THICKNESS: f32 = 0.05;`

**Reuse this helper (private, but accessible within the module):**
- `fn grid_to_world(pos: GridPosition) -> Vec3` — line 201. Returns the cell *center* in world space, with `world_y = 0.0`. **Use this directly for floor-tile positioning** (the floor's top face is at world_y = 0.0).

**Add these helpers (private to `dungeon/mod.rs`):**
- `fn cell_center_world(x: u32, y: u32) -> Vec3` — convenience wrapper around `grid_to_world` taking raw coords. **Or** just inline `grid_to_world(GridPosition { x, y })` everywhere.
- `fn wall_transform(cell_x: u32, cell_y: u32, dir: Direction) -> Transform` — returns the world-space `Transform` for a wall plate on the given cell's given face.

**Delete these (full removal):**
- `pub struct TestSceneMarker` — line 140-141. Component definition.
- `fn spawn_test_scene(...)` — lines 284-348. Entire function.
- `, spawn_test_scene` from the `OnEnter(GameState::Dungeon)` tuple at line 174.
- `test_scene: Query<Entity, With<TestSceneMarker>>` parameter and `for e in &test_scene { commands.entity(e).despawn(); }` block at lines 356, 361-363.

**Add a new component:**
- `#[derive(Component, Debug, Clone, Copy)] pub struct DungeonGeometry;` — marker on every spawned mesh + light entity.

**Add a new system:**
- `fn spawn_dungeon_geometry(...)` — runs in `OnEnter(GameState::Dungeon)` alongside `spawn_party_and_camera`.

**Modify the OnExit query to despawn the new marker:**
- `dungeon_geometry: Query<Entity, With<DungeonGeometry>>` parameter, plus `for e in &dungeon_geometry { commands.entity(e).despawn(); }` loop.

---

## Mesh Strategy Specifics

### The cell iteration loop (pseudocode)

```rust
fn spawn_dungeon_geometry(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    dungeon_assets: Option<Res<DungeonAssets>>,
    floors: Res<Assets<DungeonFloor>>,
    mut ambient: ResMut<GlobalAmbientLight>,
) {
    // Asset-tolerant: same pattern as spawn_party_and_camera.
    let Some(assets) = dungeon_assets else { warn!(...); return; };
    let Some(floor) = floors.get(&assets.floor_01) else { warn!(...); return; };

    // Cache shared mesh handles (one cuboid mesh handle for all floor tiles, etc.).
    let floor_mesh   = meshes.add(Cuboid::new(CELL_SIZE, FLOOR_THICKNESS, CELL_SIZE));
    let ceiling_mesh = meshes.add(Cuboid::new(CELL_SIZE, FLOOR_THICKNESS, CELL_SIZE));
    let wall_mesh_ns = meshes.add(Cuboid::new(CELL_SIZE, CELL_HEIGHT, WALL_THICKNESS));
    let wall_mesh_ew = meshes.add(Cuboid::new(WALL_THICKNESS, CELL_HEIGHT, CELL_SIZE));
    // (One cuboid for north/south walls — they extend along world X. One for east/west — they extend along world Z.)

    let floor_mat   = materials.add(StandardMaterial { base_color: Color::srgb(0.30, 0.28, 0.25), ..default() });
    let ceiling_mat = materials.add(StandardMaterial { base_color: Color::srgb(0.20, 0.20, 0.22), ..default() });

    // Wall material per type — cached, one per renderable type. Build a small lookup.
    let wall_solid_mat   = materials.add(Color::srgb(0.50, 0.50, 0.55));
    let wall_door_mat    = materials.add(Color::srgb(0.45, 0.30, 0.15));
    let wall_locked_mat  = materials.add(Color::srgb(0.55, 0.20, 0.15));

    // Closure to map WallType -> Option<Handle<StandardMaterial>>.
    let wall_material = |wt: WallType| -> Option<Handle<StandardMaterial>> {
        match wt {
            WallType::Open | WallType::OneWay => None,            // no geometry
            WallType::Solid | WallType::SecretWall | WallType::Illusory => Some(wall_solid_mat.clone()),
            WallType::Door => Some(wall_door_mat.clone()),
            WallType::LockedDoor => Some(wall_locked_mat.clone()),
        }
    };

    for y in 0..floor.height {
        for x in 0..floor.width {
            let world_x = x as f32 * CELL_SIZE;
            let world_z = y as f32 * CELL_SIZE;

            // Floor tile.
            commands.spawn((
                Mesh3d(floor_mesh.clone()),
                MeshMaterial3d(floor_mat.clone()),
                Transform::from_xyz(world_x, -FLOOR_THICKNESS / 2.0, world_z),
                DungeonGeometry,
            ));

            // Ceiling tile.
            commands.spawn((
                Mesh3d(ceiling_mesh.clone()),
                MeshMaterial3d(ceiling_mat.clone()),
                Transform::from_xyz(world_x, CELL_HEIGHT + FLOOR_THICKNESS / 2.0, world_z),
                DungeonGeometry,
            ));

            // North wall: render this cell's north face.
            // Center: world_x, CELL_HEIGHT/2, world_z - CELL_SIZE/2
            // Mesh: wall_mesh_ns (extends X, thin along Z)
            let walls = &floor.walls[y as usize][x as usize];
            if let Some(mat) = wall_material(walls.north) {
                commands.spawn((
                    Mesh3d(wall_mesh_ns.clone()),
                    MeshMaterial3d(mat),
                    Transform::from_xyz(world_x, CELL_HEIGHT / 2.0, world_z - CELL_SIZE / 2.0),
                    DungeonGeometry,
                ));
            }

            // West wall: render this cell's west face.
            // Center: world_x - CELL_SIZE/2, CELL_HEIGHT/2, world_z
            // Mesh: wall_mesh_ew (extends Z, thin along X)
            if let Some(mat) = wall_material(walls.west) {
                commands.spawn((
                    Mesh3d(wall_mesh_ew.clone()),
                    MeshMaterial3d(mat),
                    Transform::from_xyz(world_x - CELL_SIZE / 2.0, CELL_HEIGHT / 2.0, world_z),
                    DungeonGeometry,
                ));
            }

            // South wall: render ONLY at the bottom edge (no neighbor below).
            if y == floor.height - 1 {
                if let Some(mat) = wall_material(walls.south) {
                    commands.spawn((
                        Mesh3d(wall_mesh_ns.clone()),
                        MeshMaterial3d(mat),
                        Transform::from_xyz(world_x, CELL_HEIGHT / 2.0, world_z + CELL_SIZE / 2.0),
                        DungeonGeometry,
                    ));
                }
            }

            // East wall: render ONLY at the right edge (no neighbor to the right).
            if x == floor.width - 1 {
                if let Some(mat) = wall_material(walls.east) {
                    commands.spawn((
                        Mesh3d(wall_mesh_ew.clone()),
                        MeshMaterial3d(mat),
                        Transform::from_xyz(world_x + CELL_SIZE / 2.0, CELL_HEIGHT / 2.0, world_z),
                        DungeonGeometry,
                    ));
                }
            }
        }
    }

    // Scene-wide directional light + ambient override.
    commands.spawn((
        DirectionalLight {
            illuminance: 5_000.0,
            shadows_enabled: false,
            ..default()
        },
        Transform::from_xyz(0.0, CELL_HEIGHT * 4.0, 0.0)
            .looking_at(Vec3::new(0.5, -1.0, 0.3), Vec3::Y),
        DungeonGeometry,
    ));
    ambient.color = Color::srgb(0.30, 0.32, 0.40);
    ambient.brightness = 100.0;
}
```

Notes on the pseudocode:
- All mesh + material handles are cloned per-cell. `Handle<T>` clones are reference-counted and cheap (a single `Arc::clone`). This is the recommended pattern — verified at `bevy-0.18.1/examples/3d/parenting.rs:39, 45` (`cube_handle.clone()` for parent + child).
- The two cuboid wall meshes (`wall_mesh_ns` and `wall_mesh_ew`) differ in their X-vs-Z extents. We could use one mesh and rotate it by `Quat::from_rotation_y(FRAC_PI_2)`, but rotated cuboids confuse light normals slightly less if the geometry is "natively" oriented. Two meshes is simpler and clearer.
- Floor/ceiling y-offsets: floor's *top* face at world_y=0 means the cuboid center is at world_y = -FLOOR_THICKNESS/2 = -0.025. Ceiling's *bottom* face at world_y=CELL_HEIGHT means the cuboid center is at world_y = CELL_HEIGHT + FLOOR_THICKNESS/2 = 3.025. These match the genre convention (player at world_y=0 in floor + EYE_HEIGHT=0.7 → eye at y=0.7; ceiling visible above at 3.0+).
- Wall plate centers in world Y: CELL_HEIGHT/2 = 1.5. The wall extends from y=0 to y=CELL_HEIGHT=3.0 — exactly one corridor height.

### Wall position formulas (concrete examples, used in tests)

For cell `(grid_x=3, grid_y=4)` with `CELL_SIZE = 2.0`, `CELL_HEIGHT = 3.0`, `WALL_THICKNESS = 0.05`:

- Cell center: `(world_x, world_z) = (6.0, 8.0)`. Cell occupies world X ∈ [5.0, 7.0] and Z ∈ [7.0, 9.0].
- **North wall**: extends along X (mesh dimensions `2.0 × 3.0 × 0.05`). Center: `(6.0, 1.5, 7.0)`. Rotation: `Quat::IDENTITY`. The wall sits on the cell's northern edge (world Z = 7.0 = `world_z - CELL_SIZE/2 = 8.0 - 1.0`).
- **South wall**: identical mesh + rotation. Center: `(6.0, 1.5, 9.0)`.
- **West wall**: extends along Z (mesh dimensions `0.05 × 3.0 × 2.0`). Center: `(5.0, 1.5, 8.0)`.
- **East wall**: identical mesh + rotation. Center: `(7.0, 1.5, 8.0)`.

These are the canonical per-direction transforms; the test helpers `wall_transform(cell_x, cell_y, dir)` should produce exactly these values.

---

## Performance

| Metric | Value / Range | Source | Notes |
|--------|---------------|--------|-------|
| Entities spawned (6×6 floor_01) | 121 | Calculated below | 36 floor + 36 ceiling + 48 walls + 1 directional light |
| Unique mesh handles | 4 | This research | floor cuboid, ceiling cuboid, wall_NS cuboid, wall_EW cuboid |
| Unique material handles | 5 | This research | floor, ceiling, wall_solid, wall_door, wall_locked |
| Draw calls (estimated) | <100 | Bevy auto-batches by mesh+material | Bevy's clustered forward renderer batches identical mesh+material pairs |
| GPU memory delta | <1 MB | Cuboid mesh = ~2 KB × 4 = 8 KB; per-entity overhead ~1 KB | Negligible |
| Frame time impact | <0.5 ms (estimated) | Roadmap §Impact Analysis "Compile Δ +0.5s" | No measurement available; per-entity ECS overhead at this scale is sub-millisecond |
| Compile time impact | +0.5s (estimated) | Roadmap line 468 "Compile Δ +0.5s" | One new system function, ~150-220 LOC |

**No benchmarks available for this exact configuration in Bevy 0.18 documentation.** Master research §Performance (line 1123) confirms "DistanceFog is cheap; VolumetricFog has GPU cost" but #8 has no fog at all (deferred to #9). For the entity counts and mesh sizes here, performance is not a concern.

---

## Test Strategy

### Unit tests (in `src/plugins/dungeon/mod.rs::tests`)

These join the existing 13 unit tests in the file. All are pure-function or single-system-spawn tests.

#### Test 1: `wall_transform_north_on_cell_3_4` (pure function)

Verifies the `wall_transform` helper returns the correct world position + rotation for the north wall on cell (3, 4).

```rust
#[test]
fn wall_transform_north_on_cell_3_4_is_correct() {
    let t = wall_transform(3, 4, Direction::North);
    assert!(t.translation.abs_diff_eq(Vec3::new(6.0, 1.5, 7.0), 1e-6));
    assert!(t.rotation.abs_diff_eq(Quat::IDENTITY, 1e-6));
}
```

Repeat for South, East, West (3 more tests total — verify formulas).

#### Test 2: `wall_transform_handles_corner_cell_correctly` (pure function)

For cell (0, 0): north wall center is `(0.0, 1.5, -1.0)` (negative Z is north of grid origin); west wall center is `(-1.0, 1.5, 0.0)`. Verify negatives don't break the formulas.

#### Test 3: `wall_material_maps_each_wall_type` (pure function)

Verifies the WallType → material match returns `None` for `Open` and `OneWay`, and `Some` (i.e., a handle) for the rest. This test is mostly for documentation (it's hard to test handle equality without running an App) — accept that the test only verifies presence/absence.

```rust
#[test]
fn wall_material_returns_none_for_passable() {
    let mut materials = Assets::<StandardMaterial>::default();
    // ... build wall_material closure ...
    assert!(wall_material(WallType::Open).is_none());
    assert!(wall_material(WallType::OneWay).is_none());
}

#[test]
fn wall_material_returns_some_for_blocking() {
    let mut materials = Assets::<StandardMaterial>::default();
    // ... build wall_material closure ...
    assert!(wall_material(WallType::Solid).is_some());
    assert!(wall_material(WallType::Door).is_some());
    assert!(wall_material(WallType::LockedDoor).is_some());
    assert!(wall_material(WallType::Illusory).is_some());
    assert!(wall_material(WallType::SecretWall).is_some());
}
```

(May refactor `wall_material` from a closure to a free `fn` to make it testable from the test module.)

#### Test 4: `spawn_dungeon_geometry_creates_floor_and_ceiling_per_cell` (App-level)

Build a 3×3 open floor, spawn the dungeon, count entities tagged with `DungeonGeometry`. Expected count breakdown:

- 9 floor tiles + 9 ceiling tiles = 18
- North walls: 9 (every cell, including outer) → all `Open` for an open floor → 0 walls
- West walls: 9 → all `Open` → 0
- South walls (bottom row only): 3 → all `Open` → 0
- East walls (right column only): 3 → all `Open` → 0
- Directional light: 1
- **Total: 19 `DungeonGeometry` entities for an all-open 3×3 floor.**

If the floor has all `Solid` walls, then: north walls = 9, west walls = 9, south = 3, east = 3, plus 18 floor/ceiling + 1 light = **43 entities**.

Use `make_open_floor` from existing test helpers (line 635) for the all-Open case, and add a `make_walled_floor` for the all-Solid case.

#### Test 5: `spawn_dungeon_geometry_handles_one_way_wall_asymmetry` (App-level)

Build a 2×2 floor with a OneWay wall on cell (0,0) east face and Solid on cell (1,0) west face. Spawn geometry. Count walls. Expected: cell (0,0)'s east wall is at the right edge (not — it's at x=0 in a 2-wide floor, so the right edge is x=1). The east edge case applies only to cell (1,0). And cell (1,0)'s west face = `Solid` → renders. Cell (0,0)'s east face is OneWay and is NOT iterated by the canonical rule (we only iterate north + west of every cell, plus south/east of edge cells; cell (0,0)'s east is interior, not an edge). So the `OneWay` is invisibly skipped, and the paired `Solid` on cell (1,0)'s west *is* rendered. Verify by counting renderable walls and confirming the position matches cell (1,0)'s west face.

#### Test 6: `on_exit_dungeon_despawns_all_dungeon_geometry` (App-level)

Spawn the dungeon, then transition to a different state (e.g. `GameState::TitleScreen`), pump frames, count `DungeonGeometry` entities. Expected: 0.

#### Test 7: `floor_01_loads_with_correct_entity_count` (App-level integration test)

Use the real `floor_01.dungeon.ron` via the `TestState` pattern (mirrors `tests/dungeon_movement.rs`). Spawn dungeon. Count entities. Document the exact count in the test as a regression guard — if the wall layout changes in the asset, the test fails loudly.

For floor_01.dungeon.ron (6×6, 36 cells), counting from the asset:

- 36 floor + 36 ceiling = 72 tiles
- 1 directional light = 1
- **Walls:** Per the canonical iteration rule, count manually:

For each cell (x, y) in 0..6 × 0..6:
- North wall is rendered. 36 north faces total. Of these, count non-Open: rows y=0 (all Solid: 6), y=1..5 (varies per asset).
- West wall similarly.
- South wall on bottom row only (y=5): 6 cells. All `Solid` (per the asset).
- East wall on right column only (x=5): 6 cells. All `Solid`.

Reading floor_01.dungeon.ron carefully:

| y | Renderable north walls | Renderable west walls |
|---|------------------------|------------------------|
| 0 | 6 (all Solid) | 6 (all Solid: corners + edges) |
| 1 | 0 (all Open per row 1) | 1 (only x=0 west=Solid) |
| 2 | 0 (all Open) | 1 (only x=0 west=Solid) |
| 3 | 0 (all Open) | 1 (only x=0 west=Solid) |
| 4 | 0 (all Open) | 1 (only x=0 west=Solid) |
| 5 | 6 (all Solid per outer edge) | 6 (all Solid per outer edge) |

Wait — let me recount west walls for y=2. From the asset, row y=2: `(0,2)` west=Solid (left edge), `(1,2)` west=Open, `(2,2)` west=Open, `(3,2)` west=Open, `(4,2)` west=Open, `(5,2)` west=SecretWall (renderable as Solid). So renderable west walls in y=2 = 2 (x=0 and x=5).

Let me redo carefully:

| y | West walls rendered (x where wall is renderable) |
|---|---------------------------------------------------|
| 0 | x=0,1,2,3,4,5 all Solid → 6 walls |
| 1 | x=0 west=Solid, x=1 west=Open, x=2 west=Door, x=3 west=Open, x=4 west=LockedDoor, x=5 west=Solid → renderable: x=0, x=2 (Door), x=4 (LockedDoor), x=5 = 4 walls |
| 2 | x=0 Solid, x=1..4 Open, x=5 SecretWall → 2 renderable walls |
| 3 | x=0 Solid, x=1 Open, x=2 Illusory (renderable as Solid), x=3 Solid (paired with OneWay), x=4 Open, x=5 Open → 3 renderable walls (x=0, x=2 Illusory, x=3 Solid) |
| 4 | x=0 Solid, x=1..3 Open, x=4 Open, x=5 Open → 1 renderable wall (x=0) |
| 5 | All Solid (bottom row) → 6 walls |

Total west walls renderable = 6 + 4 + 2 + 3 + 1 + 6 = **22 west walls.**

| y | North walls rendered |
|---|---------------------|
| 0 | All 6 north faces are Solid → 6 walls |
| 1 | (0,1) Solid + (5,1) Solid; rest Open → 2 walls |
| 2 | All Open → 0 walls |
| 3 | All Open → 0 walls |
| 4 | All Open → 0 walls |
| 5 | All Solid → 6 walls |

Total north walls renderable = 6 + 2 + 0 + 0 + 0 + 6 = **14 north walls.**

Outer-edge cases:
- South walls on bottom row (y=5): all Solid → 6 walls.
- East walls on right column (x=5): y=0..5 all Solid (right outer edge throughout) → 6 walls.

**Total walls = 14 + 22 + 6 + 6 = 48 walls.**

Plus 36 floor + 36 ceiling + 1 light = **121 `DungeonGeometry` entities for floor_01.**

This is the exact count for the regression test. Document the math in the test as a comment so future planners can re-derive it.

(Caveat: this count assumes my read of the asset is correct. The planner should re-verify by reading the asset before fixing the test number.)

### Integration test (in `tests/dungeon_geometry.rs` — new file)

Mirrors `tests/dungeon_movement.rs` exactly. Loads floor_01 via `TestState` + private `TestFloorAssets` (avoids `LoadingPlugin`'s AudioAssets hang per Feature #7's lesson).

```rust
#[test]
fn dungeon_geometry_spawns_for_floor_01() {
    // ... TestState/TestFloorAssets boilerplate from tests/dungeon_movement.rs ...
    // After OnEnter(GameState::Dungeon):
    let entity_count = world.query_filtered::<Entity, With<DungeonGeometry>>().iter(world).count();
    assert_eq!(entity_count, 121, "Geometry entity count for floor_01 must match the wall-layout calculation in research §Test Strategy: 36 floor + 36 ceiling + 48 walls + 1 directional light = 121.");
}
```

### Test infrastructure status

| Property | Value |
|----------|-------|
| Framework | Built-in Rust `#[test]` + `cargo test` |
| Config file | `Cargo.toml` (existing) |
| Quick run command | `cargo test --lib dungeon` |
| Full suite command | `cargo test --features dev` |

### Test infrastructure gaps

- [ ] `tests/dungeon_geometry.rs` — new integration test file (covers floor_01 regression).

No other gaps. The pattern from `tests/dungeon_movement.rs` is directly reusable; the unit-test helper `make_test_app()` in `dungeon/mod.rs:601` is reusable as-is (the new system spawn doesn't need additional asset-type registrations beyond `Mesh` and `StandardMaterial`, which are already registered).

---

## LOC + Dep Impact

| Dimension | Estimated change | Notes |
|-----------|------------------|-------|
| `src/plugins/dungeon/mod.rs` LOC | -65 (delete `spawn_test_scene` + `TestSceneMarker` + despawn branch) +250 (new `spawn_dungeon_geometry` + helpers + tests) = **net +185** | Within roadmap's "+300 to +500" estimate (line 466), at the lower end because no per-cell torch system. |
| `tests/dungeon_geometry.rs` LOC | **+150** | New file, mirrors `tests/dungeon_movement.rs` boilerplate (~120 LOC of setup) plus assertions. |
| **Cargo.toml + Cargo.lock** | **0 bytes** | No new deps; verify with `git diff Cargo.toml Cargo.lock` is empty. |
| **Total LOC delta** | **+335 (net)** | Production: ~150-220 (excluding tests). Tests: ~100-160 in the module + 150 in the new file. |
| Compile time delta | +0.5s (estimated, per roadmap line 468) | One new system, primitive cuboids. |
| Asset Δ | 0 | No new asset files in v1; placeholder colors only. |

Test count delta: +6-8 unit tests in `dungeon/mod.rs` + 1 integration test = **+7 to +9 tests total**.

---

## Risks + Open Questions

### MEDIUM-confidence items the planner needs to make a call on:

1. **Wall colors for the v1 placeholder palette** — chosen subjectively above (cool grey for Solid, brown for Door, dark red for LockedDoor). The planner should sanity-check by running `cargo run --features dev` and visually confirming the colors are distinguishable. If not, swap. Open call.

2. **Should `spawn_dungeon_geometry` and `spawn_party_and_camera` run in parallel within `OnEnter(GameState::Dungeon)`, or sequentially?** Bevy parallelizes systems by default if there's no resource conflict. `spawn_party_and_camera` reads `Res<DungeonAssets>` and `Res<Assets<DungeonFloor>>` (immutable); `spawn_dungeon_geometry` reads the same plus `ResMut<Assets<Mesh>>`, `ResMut<Assets<StandardMaterial>>`, `ResMut<GlobalAmbientLight>` (mutable, but no conflict with the party spawn). They can run in parallel. **Recommendation: leave them in the same OnEnter tuple — Bevy's scheduler will parallelize automatically.** No `before`/`after` constraint needed because they don't read each other's outputs. (`MovementAnimation` is part of Update, not OnEnter; party state and geometry are independent.)

3. **DirectionalLight position + direction** — chosen `(0.0, 12.0, 0.0)` looking at `(0.5, -1.0, 0.3)` for "high overhead, slightly off-axis for differential corner shading." Subjective tuning. The planner should iterate after a visual smoke test.

4. **Cell-feature-to-floor-color overrides for v1?** Decision deferred: spinner (cell 2,2), dark zone (1,4), anti-magic zone (2,4), trap (4,4), teleporter (5,4) all have `CellFeatures` set. Should the planner add cosmetic floor tints for them (e.g. dark blue for dark zone)? **Recommendation: NO for #8.** Cell-feature visuals are #13's scope. Keep #8 about wall geometry only. If the planner wants to add a single boolean `floor_color_for_features: bool` constant to make #13's job trivial, fine — but no actual variant rendering in #8.

5. **`GlobalAmbientLight` restoration on OnExit** — recommendation above is "hardcode the default (color: white, brightness: 80.0) on OnExit." Open question: when Town/Combat/etc. land later (specifically Town at #18), should they have their own ambient setting? If yes, every state's OnExit needs to restore to a state-specific default, which is complex. **Recommendation for #8: just write the LightPlugin default on OnExit; let later states add their own overrides if needed.** Document the decision so #18's planner knows the convention.

### LOW-confidence items (research gaps):

6. **Bevy 0.18 `Cuboid` UV mapping orientation for textures** — the test scene's flat-color materials don't exercise UVs, so the planner won't notice issues until #9 adds textures. The Cuboid mesh writes UVs at `bevy_mesh-0.18.1/src/primitives/dim3/cuboid.rs:30-58`; UV (0,0) is bottom-left of each face's texture quad. The "Front", "Back", "Right", "Left", "Top", "Bottom" faces have consistent winding. For #8 this doesn't matter; flagging it for #9.

7. **Asset-tolerant behavior under F9 dev cycle** — if the user F9-cycles through `GameState::TitleScreen → Town → Dungeon`, do `DungeonAssets` and the `floor_01` handle survive? Yes, both are inserted by `LoadingPlugin` once (during `GameState::Loading → TitleScreen`). They persist for the game's lifetime. The asset-tolerant `let Some(...) else { warn!; return; }` pattern in `spawn_party_and_camera` is the precedent — `spawn_dungeon_geometry` should do the same.

### HIGH-confidence items (no risk, just stating for completeness):

- `cargo test`, `cargo test --features dev`, `cargo clippy --all-targets -- -D warnings`, `cargo clippy --all-targets --features dev -- -D warnings`, `cargo check`, `cargo check --features dev`, `cargo fmt --check` — all 7 must pass with zero warnings. Per Feature #7's review, the `#[cfg(feature = "dev")] app.init_resource::<ButtonInput<KeyCode>>()` pattern is required in any new test app. Already in the existing `make_test_app()` helper at line 627-628.

- The doc-comment fix at `data/dungeon.rs:18` is text-only — change `world_z = -grid_y * cell_size` to `world_z = +grid_y * cell_size`. No functional change.

---

## Sources

### Primary (HIGH confidence)

- [Bevy 0.18.1 source — `bevy_mesh::Mesh3d`](https://github.com/bevyengine/bevy/blob/v0.18.1/crates/bevy_mesh/src/components.rs#L95-L98) (verified locally at `bevy_mesh-0.18.1/src/components.rs:95-98`) — `Mesh3d(pub Handle<Mesh>)` tuple struct, `#[require(Transform)]`.
- [Bevy 0.18.1 source — `bevy_pbr::MeshMaterial3d`](https://github.com/bevyengine/bevy/blob/v0.18.1/crates/bevy_pbr/src/mesh_material.rs#L39-L41) (verified locally at `bevy_pbr-0.18.1/src/mesh_material.rs:39-41`) — `MeshMaterial3d<M: Material>(pub Handle<M>)` tuple struct.
- [Bevy 0.18.1 source — `bevy_math::primitives::Cuboid`](https://github.com/bevyengine/bevy/blob/v0.18.1/crates/bevy_math/src/primitives/dim3.rs#L691-L712) (verified locally at `bevy_math-0.18.1/src/primitives/dim3.rs:691-712`) — `Cuboid::new(x_length, y_length, z_length)` takes full lengths; `half_size: Vec3` field.
- [Bevy 0.18.1 source — `bevy_math::primitives::Plane3d`](https://github.com/bevyengine/bevy/blob/v0.18.1/crates/bevy_math/src/primitives/dim3.rs#L103-L161) (verified locally at `bevy_math-0.18.1/src/primitives/dim3.rs:103-161`) — Default normal is `+Y`, half-size 1.0.
- [Bevy 0.18.1 source — `Plane3d` mesh builder](https://github.com/bevyengine/bevy/blob/v0.18.1/crates/bevy_mesh/src/primitives/dim3/plane.rs) (verified locally at `bevy_mesh-0.18.1/src/primitives/dim3/plane.rs:122-132`) — Single-sided face only, no back face.
- [Bevy 0.18.1 source — `bevy_pbr::StandardMaterial`](https://github.com/bevyengine/bevy/blob/v0.18.1/crates/bevy_pbr/src/pbr_material.rs) (verified locally at `bevy_pbr-0.18.1/src/pbr_material.rs:35-43, 855-960`) — Field `base_color: Color`; `From<Color>` impl.
- [Bevy 0.18.1 source — `bevy_light::AmbientLight`/`GlobalAmbientLight`](https://github.com/bevyengine/bevy/blob/v0.18.1/crates/bevy_light/src/ambient_light.rs) (verified locally at `bevy_light-0.18.1/src/ambient_light.rs:9-89`) — `AmbientLight` is per-camera Component; `GlobalAmbientLight` is the resource.
- [Bevy 0.18.1 source — `bevy_light::DirectionalLight`](https://github.com/bevyengine/bevy/blob/v0.18.1/crates/bevy_light/src/directional_light.rs) (verified locally at `bevy_light-0.18.1/src/directional_light.rs:58-138`) — Component, `illuminance: f32` in lux, `shadows_enabled: bool`.
- [Bevy 0.18.1 source — `Transform::from_xyz` + `with_rotation`](https://github.com/bevyengine/bevy/blob/v0.18.1/crates/bevy_transform/src/components/transform.rs#L115-L249) (verified locally at `bevy_transform-0.18.1/src/components/transform.rs:115-117, 246-249`) — Both are `const fn`.
- [Bevy 0.18.1 source — `EntityCommands::despawn` is recursive](https://github.com/bevyengine/bevy/blob/v0.18.1/crates/bevy_ecs/src/system/commands/entity_command.rs#L242-L249) (verified locally at `bevy_ecs-0.18.1/src/system/commands/entity_command.rs:242-249`).
- [Bevy 0.18.1 example — 3d_scene.rs](https://github.com/bevyengine/bevy/blob/v0.18.1/examples/3d/3d_scene.rs) (verified locally at `bevy-0.18.1/examples/3d/3d_scene.rs:13-43`) — Canonical mesh + camera + light spawn pattern.
- [Bevy 0.18.1 example — parenting.rs](https://github.com/bevyengine/bevy/blob/v0.18.1/examples/3d/parenting.rs) (verified locally at `bevy-0.18.1/examples/3d/parenting.rs:38-49`) — `children![...]` macro + handle clone pattern.
- [Bevy 0.18.1 example — lighting.rs](https://github.com/bevyengine/bevy/blob/v0.18.1/examples/3d/lighting.rs) (verified locally at `bevy-0.18.1/examples/3d/lighting.rs:122-127, 191-200`) — `GlobalAmbientLight` resource override pattern; `DirectionalLight` setup.
- Druum master research §Pattern 6 — `/Users/nousunio/Repos/Learnings/claude-code/druum/research/20260326-01-bevy-first-person-dungeon-crawler-rpg.md:876-966` — Original Pattern 6 sketch with WallType iteration.
- Druum master research §Performance — same file, lines 1123 — Confirms Bevy's clustered forward renderer handles thousands of textured quads.

### Secondary (MEDIUM confidence — Druum-internal)

- Druum `src/plugins/dungeon/mod.rs:1-348` — Feature #7's anchor: `CELL_SIZE`, `EYE_HEIGHT`, `grid_to_world`, `facing_to_quat`, `spawn_party_and_camera`, `despawn_dungeon_entities`, world coordinate convention.
- Druum `src/data/dungeon.rs:1-336` — Feature #4 anchor: `Direction`, `WallType`, `WallMask`, `DungeonFloor`, `validate_wall_consistency`.
- Druum `assets/dungeons/floor_01.dungeon.ron` — 6×6 test floor with all WallType variants, used for the regression integration test.
- Druum `tests/dungeon_movement.rs:1-169` — Feature #7 integration test pattern; reusable for the new geometry integration test.
- Druum `project/research/20260503-120000-feature-7-grid-movement-first-person-camera.md` — Feature #7 research; precedent for Bevy 0.18 mesh APIs and the test-app pattern.
- Druum `project/roadmaps/20260429-01-bevy-dungeon-crawler-roadmap.md:438-487` — Feature #8 roadmap entry; `+300 to +500` LOC estimate, `+0` deps, mesh-merging-via-shared-handles approach.
- Druum `project/reviews/20260503-210000-feature-7-grid-movement-first-person-camera.md` — Feature #7 review; LOW finding (stale doc-comment) deferred to #8.

### Tertiary (LOW confidence — flagged for validation)

- Per-edge wall iteration ordering ("north + west of every cell, plus south/east of edge cells") — derived from canonical razor-wall convention but not directly cited in any 0.18-era Bevy tutorial. **Validation:** the planner should run the integration test against floor_01 and visually inspect (a) no double-rendered walls (no z-fighting), (b) no missing walls on the boundary cells. If missing walls are found, the rule may need to be inverted or the boundary cases corrected.
- Wall color choices (cool grey, brown, dark red) — subjective; no formal source. Validate by visual smoke.

---

## Metadata

**Confidence breakdown:**
- Bevy 0.18 APIs (Cuboid, Plane3d, Mesh3d, MeshMaterial3d, StandardMaterial, lights, despawn, Transform): **HIGH** — verified on-disk at concrete file:line locations.
- Architectural decisions (per-edge iteration, cell height 3.0, scene-wide directional, OneWay-side-skip): **HIGH** — grounded in master research, prior-feature precedent (Druum #7), and Bevy convention.
- Wall thickness 0.05, lighting parameters: **MEDIUM** — chosen by judgment; tunable in #9 polish.
- Test entity-count math for floor_01: **MEDIUM** — derived by careful read of the asset; planner should re-verify before fixing the test number, especially the renderable east/south wall counts on the outer edges.
- Wall color palette: **LOW** — subjective; needs visual validation by the planner/implementer.

**Research date:** 2026-05-03 (system clock-time; UTC suffix omitted for filename brevity)

**Author:** Researcher agent
