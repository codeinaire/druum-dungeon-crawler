---
name: Bevy 0.18 DistanceFog API and four falloff modes
description: DistanceFog is a per-camera Component in bevy_pbr; defaults to FogFalloff::Linear { 0.0, 100.0 } which looks like "no fog" at small scales — implementations MUST specify falloff explicitly to avoid silent visual bug
type: reference
---

`bevy::pbr::DistanceFog` is the cheap depth-based fog used in dungeon-crawler-style atmosphere. It's a Component attached to the same entity as the `Camera3d` it affects.

**Verified at `bevy_pbr-0.18.1/src/fog.rs:49-72`:**

```rust
#[derive(Debug, Clone, Component, Reflect, ExtractComponent)]
#[extract_component_filter(With<Camera>)]
#[reflect(Component, Default, Debug, Clone)]
pub struct DistanceFog {
    pub color: Color,
    pub directional_light_color: Color,    // for "sun glow" effect; Color::NONE to disable
    pub directional_light_exponent: f32,
    pub falloff: FogFalloff,
}
```

**Default impl — verified at `bevy_pbr-0.18.1/src/fog.rs:465-476`:**

```rust
impl Default for DistanceFog {
    fn default() -> Self {
        DistanceFog {
            color: Color::WHITE,
            falloff: FogFalloff::Linear { start: 0.0, end: 100.0 },
            directional_light_color: Color::NONE,
            directional_light_exponent: 8.0,
        }
    }
}
```

**THE TRAP:** `DistanceFog { ..default() }` produces `Linear { 0.0, 100.0 }` fog. On a 6×6 dungeon (12 world units across), this is visually equivalent to "no fog" until distance ~50, then ~12% opacity at the far wall. Looks like nothing happened. The implementer thinks the fog isn't working and starts debugging the wrong thing.

**Always specify `falloff` explicitly:**

```rust
DistanceFog {
    color: Color::srgb(0.10, 0.09, 0.08),
    falloff: FogFalloff::Exponential { density: 0.12 },
    ..default()
}
```

**Four falloff modes — verified at `bevy_pbr-0.18.1/src/fog.rs:97-304`:**

| Variant | Formula | Best For |
| --- | --- | --- |
| `Linear { start: f32, end: f32 }` | `1.0 - clamp((end - distance) / (end - start), 0.0, 1.0)` | Predictable; "artificial" looking; control via two distances. Default. |
| `Exponential { density: f32 }` | `1.0 - 1.0 / (distance * density).exp()` | Most natural; control via single density. **Recommended for dungeons.** |
| `ExponentialSquared { density: f32 }` | `1.0 - 1.0 / (distance * density).squared().exp()` | Slower close, faster far. Good for vast-outdoor scenes. |
| `Atmospheric { extinction: Vec3, inscattering: Vec3 }` | Per-channel; equivalent to `Exponential` when all channels equal. | Realistic atmosphere; computationally most expensive. |

**Convenience constructors — verified at `bevy_pbr-0.18.1/src/fog.rs:309-462`:**

For artistic control via "I want fog to cut off at world distance V" instead of guessing density:
- `FogFalloff::from_visibility(visibility: f32)` — uses revised Koschmieder threshold (5% contrast at V).
- `FogFalloff::from_visibility_contrast(visibility, contrast_threshold)` — explicit contrast.
- `from_visibility_squared`, `from_visibility_color`, `from_visibility_colors` — for Squared/Atmospheric.

For Druum-style dungeons (visibility ~26 cells = 52 world units):
```rust
FogFalloff::from_visibility(52.0)
// equivalent to FogFalloff::Exponential { density: -ln(0.05) / 52.0 ≈ 0.058 }
```

**Performance:** "essentially free" per fragment — adds one `mix(in_color, fog_color, fog_intensity)` call to the PBR fragment shader. Verified at `bevy_pbr-0.18.1/src/render/fog.wgsl` (~30 lines, single mix call).

**Per-StandardMaterial override:** Each `StandardMaterial` has a `fog_enabled: bool` flag (verified at `bevy_pbr-0.18.1/src/fog.rs:46-48` doc) — set to false on materials that should ignore fog (e.g. UI overlays, sky meshes).

**How to apply:**

When researching Bevy 0.18 features that need atmospheric fog:
- Always pair `DistanceFog` with `Camera3d` on the same entity (Component, not Resource).
- Always specify `falloff: FogFalloff::Exponential { density: ... }` explicitly. Never rely on `..default()` for the falloff.
- For "I want X% fog at distance Y" intent, use `FogFalloff::from_visibility(...)` instead of guessing density.
- Recommend the cheap `Exponential` mode unless the project explicitly needs atmospheric scattering.
- Reserve `VolumetricFog` (different API, different file `bevy_light-0.18.1/src/volumetric.rs`) for special zones — it has real GPU cost.
