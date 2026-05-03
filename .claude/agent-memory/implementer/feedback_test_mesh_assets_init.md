---
name: Test apps need init_asset for Mesh/StandardMaterial when testing 3D spawn systems
description: MinimalPlugins lacks PbrPlugin/MeshPlugin — any test app running OnEnter systems that spawn 3D geometry must init Mesh and StandardMaterial asset types manually.
type: feedback
---

When a test app (using `MinimalPlugins + AssetPlugin`) runs an `OnEnter` system that spawns 3D mesh entities (e.g., `spawn_test_scene` calling `meshes.add(Cuboid::new(...))`), the `ResMut<Assets<Mesh>>` and `ResMut<Assets<StandardMaterial>>` system parameters will fail validation with "Resource does not exist".

Fix: call `app.init_asset::<Mesh>().init_asset::<StandardMaterial>()` in the test app setup before running any update that triggers the 3D spawn system.

**Why:** `MinimalPlugins` does not include `MeshPlugin` or `PbrPlugin`. These are only available via `DefaultPlugins`. Adding the full `DefaultPlugins` chain in tests is too heavy; `init_asset` just registers the asset type registry without the renderer.

**How to apply:** Any test app that includes a plugin with 3D spawning systems (cubes, planes, lights as `DirectionalLight` components) needs these two init_asset calls. Check for panics with "Resource does not exist" on Mesh or StandardMaterial parameters.
