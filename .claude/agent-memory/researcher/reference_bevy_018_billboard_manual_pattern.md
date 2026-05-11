---
name: Bevy 0.18 manual billboard / textured-quad pattern
description: First-party Bevy 0.18 components and idioms for rendering a face-camera textured quad in 3D — full reference for replacing bevy_sprite3d with ~50 LOC of stock code
type: reference
---

When Bevy 0.18 needs a sprite-in-3D billboard (e.g. for DRPG enemies, FOEs, NPCs) and the third-party `bevy_sprite3d` is unavailable or unverified, the first-party pattern is straightforward. All references are on-disk verified Bevy 0.18.1 examples.

**Quad mesh:** `Rectangle::new(width, height)` produces a quad in the XY plane with normal +Z. Verified usable for billboarding by inspection — `bevy-0.18.1/examples/3d/3d_shapes.rs:87` (`Extrusion::new(Rectangle::default(), 1.)`).

DO NOT use `Plane3d` — it's a +Y-facing single-sided primitive intended for ground planes (verified at `bevy_mesh-0.18.1/src/primitives/dim3/plane.rs:122-132`, see memory `reference_bevy_018_mesh_lighting_gotchas.md`).

**Material with arbitrary `Handle<Image>`:**
```rust
StandardMaterial {
    base_color_texture: Some(handle),
    unlit: true,                              // critical in low-ambient scenes; PBR sampling otherwise darkens the texture
    alpha_mode: AlphaMode::Mask(0.5),         // hard-edge sprite transparency; order-independent
    ..default()
}
```
Verified: `bevy-0.18.1/examples/3d/render_to_texture.rs:84-93`, `bevy-0.18.1/examples/3d/transparency_3d.rs:32-39`.

`AlphaMode::Mask(0.5)` is the right choice for sprites because:
- Blend mode requires back-to-front sort; jittering distances between adjacent sprites flips frames.
- Mask is binary (alpha > 0.5 visible, else not) — order-independent, no flicker.

**Runtime-generated solid-colour texture** for placeholder art:
```rust
use bevy::asset::RenderAssetUsages;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

let image = Image::new_fill(
    Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
    TextureDimension::D2,
    &[r_u8, g_u8, b_u8, 255_u8],
    TextureFormat::Rgba8UnormSrgb,
    RenderAssetUsages::RENDER_WORLD,
);
let handle = images.add(image);
```
Verified: `bevy-0.18.1/examples/3d/3d_shapes.rs:236-247`.

`RenderAssetUsages::RENDER_WORLD` is correct — the texture lives on the GPU, no CPU readback needed.

**Spawn pattern (no Bundle types in 0.18):**
```rust
commands.spawn((
    Mesh3d(quad_handle.clone()),
    MeshMaterial3d(material_handle),
    Transform::from_translation(world_pos),
    Visibility::default(),
    YourBillboardMarker,
));
```
Verified: `bevy-0.18.1/examples/3d/3d_scene.rs:25-28`. See memory `reference_bevy_018_camera3d_components.md` for the broader no-Bundle pattern.

**Y-axis-locked face-camera math (recommended):**
```rust
fn face_camera(
    camera_q: Query<&GlobalTransform, With<YourCameraMarker>>,
    mut sprites_q: Query<&mut Transform, (With<YourBillboardMarker>, Without<YourCameraMarker>)>,
) {
    let Ok(camera) = camera_q.single() else { return; };
    let camera_pos = camera.translation();
    for mut t in &mut sprites_q {
        let dx = camera_pos.x - t.translation.x;
        let dz = camera_pos.z - t.translation.z;
        let angle = dx.atan2(dz);  // yaw for a quad whose normal is +Z
        t.rotation = Quat::from_rotation_y(angle);
    }
}
```

Key points:
- `Without<YourCameraMarker>` in the second query is required by Bevy's borrow-checker (B0001 disjoint-set rule for mutable Transform access).
- `&GlobalTransform`, NOT `&Transform`, because the camera is usually a child of another entity (e.g. PlayerParty in Druum); local Transform doesn't include the parent translation.
- `atan2(dx, dz)` (note arg order) gives the yaw angle for a quad whose default normal is +Z. For a +X-facing quad swap the args; for `Plane3d` (+Y) the math doesn't apply.
- Y-axis-locked is correct for DRPG-style cameras that don't pitch. If the camera ever pitches, the sprite stays vertical (intentional — sprite-in-3D never tilts).

**Sprite-sheet animation (when real art lands):**
```rust
// Setup:
let layout = TextureAtlasLayout::from_grid(UVec2::splat(64), 8, 1, None, None); // 8 frames, 64×64 each
let layout_handle = atlas_layouts.add(layout);

// Sprite component (in `bevy::prelude` via the ui→bevy_sprite feature chain on `3d`):
Sprite {
    image: texture_handle,
    texture_atlas: Some(TextureAtlas { layout: layout_handle, index: 0 }),
    ..default()
}

// Frame stepping in a system:
if let Some(atlas) = &mut sprite.texture_atlas {
    atlas.index = (atlas.index + 1) % frame_count;
}
```
Verified: `bevy-0.18.1/examples/2d/sprite_animation.rs:56-77` and `:107-145`.

NOTE: `Sprite` is a 2D primitive; for a 3D-spatial billboard you DON'T attach `Sprite` to the entity — you attach `Mesh3d` + `MeshMaterial3d` with `base_color_texture` pointing at the same atlas image. The `TextureAtlas.index` API is only directly usable with `Sprite`; for a 3D quad you either (a) use the full atlas image as a single texture and ignore atlas-based animation for 0.18-native code (b) replace the material's `base_color_texture` per frame with a different cropped Image, or (c) author a custom shader with `uv_transform`. For DRPG placeholder PRs with 1×1 solid-colour textures this complexity doesn't arise.

**For atlas-based animation in 3D:** the simplest path is per-frame swap of `Handle<Image>` on `MeshMaterial3d`. Cache one `Handle<StandardMaterial>` per (enemy_id, animation_frame) tuple; the animation system swaps the material handle. This is N×M materials but Bevy 0.18 dedupes them so it's not a draw-call hit. Alternative: a custom shader that takes `uv_offset` as a uniform — overkill for placeholder PR but standard for shipped sprite-in-3D engines.

**How to apply:** When billboarding sprites in Bevy 0.18 first-party, use Rectangle + StandardMaterial { unlit, AlphaMode::Mask(0.5) } + face_camera system. Use Image::new_fill for runtime colour placeholders. Defer atlas-based animation in 3D until real art needs it; for placeholders, single-frame textures are sufficient.
