---
name: Bevy 0.18 shadow-cast cap is auto-applied via stable sort by entity ID
description: There is no per-camera "max 4 shadow lights" knob in Bevy 0.18 — the renderer sorts shadows-enabled lights first, then by entity ID, then truncates at max_texture_array_layers/6 cubemaps; nearest-to-camera prioritization is NOT built in
type: reference
---

When researching Bevy 0.18 features that involve N point lights with shadows enabled (e.g. dungeon torches, fire-effect entities), the question "does Bevy automatically pick the N best shadows to render?" is YES — but the selection criterion is NOT distance to camera.

**Verified at `bevy_pbr-0.18.1/src/render/light.rs:817-821`:**

```rust
let point_light_shadow_maps_count = point_lights
    .iter()
    .filter(|light| light.2.shadows_enabled && light.2.spot_light_angles.is_none())
    .count()
    .min(max_texture_cubes);   // = max_texture_array_layers / 6 (typically ~42 on desktop)
```

**Sort key — verified at `render/light.rs:860-865` + `bevy_light-0.18.1/src/cluster/assign.rs:105-120`:**

```rust
point_lights.sort_by_cached_key(|(entity, _, light, _)| {
    (point_or_spot_light_to_clusterable(light).ordering(), *entity)
});

// ordering() for PointLight returns (0, !shadows_enabled, !volumetric)
// So sort order is:
//   1. PointLights with shadows_enabled: true   (ordering tuple starts with 0, false=0)
//   2. PointLights with shadows_enabled: false  (ordering tuple starts with 0, true=1)
//   then by entity ID ascending as the stable tiebreaker.
```

**Translation:**

- Bevy DOES sort `shadows_enabled: true` lights to the front of the cluster list.
- Bevy DOES truncate at the GPU's max cubemap count.
- Bevy does NOT consider distance to camera. The 4 lights that "win" the shadow budget are determined by entity-spawn-order (entity IDs), not visual relevance.
- Lights with `shadows_enabled: false` cost nothing in the shadow pass (no cubemap allocation).

**Practical implications:**

1. **For features authoring N ≤ 4 shadow torches** (Druum dungeon case): set `shadows_enabled: true` only on those N entries in the asset. The clustering math handles the rest at zero LOC. This is the recommended path.

2. **For features needing nearest-N-shadow selection** (e.g. dynamic spawning of fire effects): write a `PostUpdate` system:
   - Query `(Entity, &GlobalTransform, &mut PointLight)` + the camera's `GlobalTransform`.
   - Sort by squared distance.
   - Set `shadows_enabled = true` on the first N, `false` on the rest.
   - Cost: ~30 LOC + per-frame query iteration. Adds 1 frame of "shadow pop" lag as the player moves.

3. **The "MAX_DIRECTIONAL_LIGHTS = 10" + "MAX_CASCADES_PER_LIGHT = 4" constants** at `bevy_pbr-0.18.1/src/render/light.rs:185,191` are the directional-light caps and are unrelated to point-light cubemaps.

4. **WebGL2 fallback caps everything tighter:** `MAX_UNIFORM_BUFFER_CLUSTERABLE_OBJECTS = 204` (verified `bevy_pbr-0.18.1/src/cluster.rs:21`), and `max_texture_cubes = 1` on WebGL (verified `render/light.rs:781`). Native desktop is the comfortable case.

**How to apply:**

When researching Bevy 0.18 features that author multiple shadow-casting point lights, default to "author the cap into the asset" rather than "implement nearest-N selection." Surface the explicit selection only as an opt-in polish feature when there's evidence the entity-ID stable choice produces visually wrong results.
