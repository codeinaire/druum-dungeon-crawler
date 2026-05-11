---
name: bevy_sprite3d 8.0 test app — init Mesh + StandardMaterial + Image + TextureAtlasLayout
description: Any test app that includes CombatPlugin (→ EnemyRenderPlugin → Sprite3dPlugin) under MinimalPlugins must init all four asset stores for bundle_builder
type: feedback
---

`bevy_sprite3d`'s `bundle_builder` PostUpdate system declares these system parameters which Bevy validates at startup:
- `Res<Assets<Image>>` — image store
- `ResMut<Assets<Mesh>>` — mesh cache
- `ResMut<Assets<StandardMaterial>>` — material cache
- `ResMut<Assets<TextureAtlasLayout>>` — atlas layouts (validated even when no atlas is used)

Under `MinimalPlugins` (which lacks `PbrPlugin`, `SpritePlugin`, etc.), none of these are registered automatically.

**Why:** `bundle_builder` panics at system validation time with `Resource does not exist` for any of these four. Originally thought to only need Mesh + StandardMaterial (documented in prior memory entry). Image + TextureAtlasLayout added to the list after Feature #17 recovery run revealed panics in 48 tests across all modules.

**How to apply:** In any test app that includes `CombatPlugin` (which now registers `EnemyRenderPlugin → Sprite3dPlugin`), add ALL four:
```rust
app.init_asset::<bevy::prelude::Mesh>();
app.init_asset::<bevy::pbr::StandardMaterial>();
app.init_asset::<bevy::image::Image>();
app.init_asset::<bevy::image::TextureAtlasLayout>();
```
Affects: all unit test `make_test_app` functions in combat/, dungeon/, and integration tests in tests/. When EnemyRenderPlugin is registered transitively via CombatPlugin in a future test harness, add these four immediately.
