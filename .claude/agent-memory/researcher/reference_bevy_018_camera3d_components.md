---
name: Bevy 0.18 Camera3d / lights are components, not bundles
description: Bevy 0.18 removed Camera3dBundle, PointLightBundle, DirectionalLightBundle — they're components with #[require(...)] now. Spawn pattern is a component tuple.
type: reference
---

In Bevy 0.18, the legacy `*Bundle` types are GONE. Every primary 3D scene type is a `Component` with `#[require(...)]` attributes that auto-attach the supporting components.

**Verified at on-disk source:**

- `Camera3d` — `bevy_camera-0.18.1/src/components.rs:22-25`. `#[require(Camera, Projection)]`. `Projection` defaults to `PerspectiveProjection { fov: PI/4, near: 0.1, far: 1000.0, aspect_ratio: 1.0 }` (auto-updated by `camera_system`).
- `PointLight` — `bevy_light-0.18.1/src/point_light.rs:41-49`. `#[require(CubemapFrusta, CubemapVisibleEntities, Transform, Visibility, VisibilityClass)]`.
- `DirectionalLight` — `bevy_light-0.18.1/src/directional_light.rs:58-68`. Lights along entity's forward direction.
- `AmbientLight` — `bevy_light-0.18.1/src/ambient_light.rs:9-12`. **Per-camera** (`#[require(Camera)]`); for global, use `GlobalAmbientLight` *resource* instead.
- `Mesh3d(pub Handle<Mesh>)` — `bevy_mesh-0.18.1/src/components.rs:96-98`. `#[require(Transform)]`.
- `MeshMaterial3d<M>(pub Handle<M>)` — `bevy_pbr-0.18.1/src/mesh_material.rs:39-41`.

**Canonical 0.18 spawn pattern** (verified at `bevy-0.18.1/examples/3d/3d_scene.rs:25-42`):

```rust
commands.spawn((
    Mesh3d(meshes.add(Cuboid::new(1.0, 1.0, 1.0))),
    MeshMaterial3d(materials.add(Color::srgb_u8(124, 144, 255))),
    Transform::from_xyz(0.0, 0.5, 0.0),
));
commands.spawn((
    Camera3d::default(),
    Transform::from_xyz(-2.5, 4.5, 9.0).looking_at(Vec3::ZERO, Vec3::Y),
));
commands.spawn((
    PointLight { shadows_enabled: true, ..default() },
    Transform::from_xyz(4.0, 8.0, 4.0),
));
```

**Child entity spawn** uses the `children![]` macro (`bevy-0.18.1/examples/3d/parenting.rs:43-49`):

```rust
commands.spawn((
    Transform::IDENTITY,
    Player,
    children![(Camera3d::default(), Transform::from_xyz(0.0, EYE_HEIGHT, 0.0))],
));
```

**Trap:** old Bevy tutorials and 0.17-and-earlier docs say `commands.spawn(Camera3dBundle { ... })`. Will not compile in 0.18. The component-tuple form is the only correct shape.

**How to apply:** When researching Druum features that involve cameras, lights, or meshes, recommend the component-tuple spawn pattern explicitly. When reading older Bevy material from training data, treat any `*Bundle` reference as a 0.17-and-earlier signal that needs translation.
