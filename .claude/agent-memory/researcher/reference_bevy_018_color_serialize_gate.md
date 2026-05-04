---
name: Bevy 0.18 `Color: Serialize/Deserialize` is feature-gated
description: bevy::Color only derives serde traits when `bevy_color/serialize` feature is enabled, which is NOT included in the `3d` umbrella; Druum-style projects must wrap Color in a tuple/struct for RON-asset use
type: reference
---

In Bevy 0.18.1, `bevy::Color` (the master enum at `bevy_color-0.18.1/src/color.rs:56-77`) derives `serde::Serialize` and `serde::Deserialize` ONLY when the `bevy_color/serialize` feature is enabled.

**Verified at `bevy_color-0.18.1/src/color.rs:51`:**

```rust
#[cfg_attr(feature = "serialize", derive(serde::Serialize, serde::Deserialize))]
```

**Verified at `bevy_color-0.18.1/Cargo.toml:58-61`:**

```toml
serialize = [
    "serde",
    "bevy_math/serialize",
]
```

**The trap for Druum-style projects (umbrella `bevy = { features = ["3d", ...] }`):**

`bevy/3d` does NOT pull in `serialize`. Verified at `bevy-0.18.1/Cargo.toml:2322-2330`:

```toml
3d = [
    "default_app",
    "default_platform",
    "3d_bevy_render",
    "ui",
    "scene",
    "audio",
    "picking",
]
```

None of these recursively include `bevy_internal/serialize` (verified at `bevy_internal-0.18.1/Cargo.toml:345-360`).

So `pub color: Color` on a struct with `#[derive(Serialize, Deserialize)]` will fail to compile with:

```
error[E0277]: the trait bound `Color: Deserialize<_>` is not satisfied
```

**Two ways out:**

1. **Wrapper type (recommended for Δ deps = 0 projects):**

```rust
#[derive(Reflect, Serialize, Deserialize, Default, Debug, Clone, Copy, PartialEq)]
pub struct ColorRgb(pub f32, pub f32, pub f32);

impl ColorRgb {
    pub fn into_color(self) -> Color {
        Color::srgb(
            self.0.clamp(0.0, 1.0),
            self.1.clamp(0.0, 1.0),
            self.2.clamp(0.0, 1.0),
        )
    }
}
```

In RON: `color: (1.0, 0.7, 0.3)`. Zero feature-flag changes. Three lines of helper code.

2. **Enable `bevy/serialize`:** add `"serialize"` to the feature list in `Cargo.toml`. Pulls 12 transitive features (verified `bevy_internal-0.18.1/Cargo.toml:345-360`):

```toml
serialize = [
    "bevy_a11y?/serialize",
    "bevy_color?/serialize",
    "bevy_ecs/serialize",
    "bevy_image?/serialize",
    "bevy_input/serialize",
    "bevy_math/serialize",
    "bevy_scene?/serialize",
    "bevy_time/serialize",
    "bevy_transform/serialize",
    "bevy_ui?/serialize",
    "bevy_window?/serialize",
    "bevy_winit?/serialize",
    "bevy_platform/serialize",
    "bevy_render?/serialize",
]
```

This is a Cargo.toml change AND increases compile time. Avoid unless multiple types need Color serde.

**How to apply:**

When researching any Druum feature that wants to put `bevy::Color` into a RON file (or any other serde-driven path), default to the `ColorRgb` wrapper. Document the pattern with a doc-comment referencing this memory so future contributors don't "fix" the wrapper by enabling the feature.

The same trap applies to other Bevy types with `#[cfg_attr(feature = "serialize", ...)]` derives — `Vec3`, `Transform`, `Quat` (via `bevy_math/serialize`), `LinearRgba` (via `bevy_color/serialize`). Verify per-type at their crate's Cargo.toml when in doubt.
