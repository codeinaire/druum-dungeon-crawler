---
name: Bevy 0.18 mesh + lighting common gotchas
description: Three pitfalls that catch dungeon-crawler-style mesh research — Plane3d is single-sided, Cuboid::new takes full lengths (not half-extents), AmbientLight is per-camera vs GlobalAmbientLight resource
type: reference
---

When researching 3D rendering features in Bevy 0.18, these three traps catch outdated tutorials and even older research docs.

## 1. `Plane3d` mesh is single-sided

`Plane3d` mesh builder writes only one set of front-facing triangles — no back face.

- Verified at `bevy_mesh-0.18.1/src/primitives/dim3/plane.rs:122-132` — only one set of indices per quad, all winding the same direction.
- Default normal is `+Y` (Y-up), half-size 1.0 — see `bevy_math-0.18.1/src/primitives/dim3.rs:113-120`.
- Translation: a `Plane3d` floor tile is invisible from underneath. A `Plane3d` ceiling tile needs `Quat::from_rotation_x(PI)` to flip its normal downward.

**Workaround for two-sided thin geometry:** use a thin `Cuboid` slab instead. 24 vertices vs 4 is negligible for tile counts under ~10,000.

## 2. `Cuboid::new(x, y, z)` takes FULL lengths, not half-extents

```rust
// Verified at bevy_math-0.18.1/src/primitives/dim3.rs:707-712
impl Cuboid {
    pub const fn new(x_length: f32, y_length: f32, z_length: f32) -> Self {
        Self::from_size(Vec3::new(x_length, y_length, z_length))
    }
}
```

The `half_size: Vec3` field is set internally to `length / 2`. The constructor name `new` is consistent with "full lengths" (not "half_size"). Mistaking the parameters as half-extents produces 2× too-large geometry, a classic and embarrassing bug.

`Cuboid::default()` is a 1.0 × 1.0 × 1.0 box (half-size = 0.5).

## 3. `AmbientLight` is a per-camera Component; `GlobalAmbientLight` is the resource

This is a 0.18 change that traps every research doc written for 0.16/0.17:

```rust
// Verified at bevy_light-0.18.1/src/ambient_light.rs:9-12
#[derive(Component, Clone, Debug, Reflect)]
#[require(Camera)]
pub struct AmbientLight { /* color, brightness, ... */ }

// Verified at bevy_light-0.18.1/src/ambient_light.rs:59-62
#[derive(Resource, Clone, Debug, Reflect)]
pub struct GlobalAmbientLight { /* color, brightness, ... */ }
```

For scene-wide ambient, mutate `Res<GlobalAmbientLight>` (auto-inserted by `LightPlugin`):

```rust
// Verified at bevy-0.18.1/examples/3d/lighting.rs:122-127
commands.insert_resource(GlobalAmbientLight {
    color: ORANGE_RED.into(),
    brightness: 200.0,
    ..default()
});
```

Per-camera override (e.g. for a darker dungeon ambient on the dungeon camera only): attach `AmbientLight` as a component to the `Camera3d` entity.

**Older research / training data trap:** Code like `commands.insert_resource(AmbientLight { ... })` will fail to compile in 0.18 because `AmbientLight` is no longer a `Resource`. Druum's master research at `research/20260326-01-...md:1141-1146` makes this exact mistake; subsequent Bevy 0.18 features should override with the correct API.

## How to apply

When recommending mesh primitives, lighting setups, or any 3D rendering pattern in Bevy 0.18:
- Reach for `Cuboid` over `Plane3d` when two-sided visibility matters.
- Always pass full lengths to `Cuboid::new`, not half-extents.
- Use `GlobalAmbientLight` (resource) for scene-wide ambient; `AmbientLight` (component) only for per-camera overrides.
- When citing master research patterns from before 2026-03, double-check the `AmbientLight` resource-vs-component shape against current 0.18 source.
