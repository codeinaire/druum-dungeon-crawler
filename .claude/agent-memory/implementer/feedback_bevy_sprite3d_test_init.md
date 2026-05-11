---
name: bevy_sprite3d 8.0 test app requires Mesh + StandardMaterial init
description: Any test app that uses EnemyRenderPlugin (or adds Sprite3d components) under MinimalPlugins must init_asset Mesh and StandardMaterial
type: feedback
---

`bevy_sprite3d`'s `bundle_builder` PostUpdate system calls `meshes.add(...)` and `materials.add(...)`. Under `MinimalPlugins` (which lacks `PbrPlugin`), these asset stores are not registered automatically.

**Why:** The `bundle_builder` system panics or silently fails if `Assets<Mesh>` or `Assets<StandardMaterial>` are not registered. First discovered during Feature #17 enemy billboard integration tests.

**How to apply:** In any test app that includes `CombatPlugin` (which now registers `EnemyRenderPlugin` → `Sprite3dPlugin`), add:
```rust
app.init_asset::<bevy::prelude::Mesh>();
app.init_asset::<bevy::pbr::StandardMaterial>();
```
This is the same pattern documented in the `[3D spawn systems in tests — must init_asset Mesh + StandardMaterial]` project-level memory. Reiterated here for the specific `EnemyRenderPlugin` context.
