---
name: Druum enemy render (Feature #17) plan revision — bevy_sprite3d 8.0 swap
description: 2026-05-11 revision pivot from manual billboard to bevy_sprite3d 8.0 — critical user-facing finding that the crate does NOT auto-billboard; face_camera system still required
type: project
---

User issued a focused revision to Feature #17 on 2026-05-11 to use `bevy_sprite3d 8.0` (verified Bevy 0.18 compatible) instead of the manual textured-quad path that was the original locked decision (4A).

**Why:** User confirmed `bevy_sprite3d 8.0` supports Bevy 0.18 and asked the planner to pivot. Verified independently via context7 + cargo registry: `bevy_sprite3d v8.0.0 → bevy v0.18.1` (`cargo tree` output), and the crate's README has a version table that explicitly maps `bevy_sprite3d 8.0 ↔ bevy 0.18`. So `=0.18.1` pin is compatible with the crate's `bevy = "0.18.0"` declaration.

**How to apply:** Critical finding for the implementer (and any future plan that pulls in `bevy_sprite3d`):

- **`bevy_sprite3d` is NOT an auto-billboard plugin.** Despite the name, the crate provides `Sprite { image, .. }` + `Sprite3d { pixels_per_metre, unlit, alpha_mode, .. }` components that build a cached textured 3D quad, but it does NOT rotate the quad to face the camera. The crate's own `examples/dungeon.rs` (lines 78 + 496-505) defines a user-authored `face_camera` Update system. The user's revision instruction said "the library handles the billboard rotation, so the manual `face_camera` system is removed" — this is incorrect; the system stays.
- The crate API is component-pair, NOT a builder: `commands.entity(e).insert((Sprite, Sprite3d, Transform, EnemyBillboard))`.
- `Sprite3d` has `#[require(Transform, Mesh3d, MeshMaterial3d<StandardMaterial>, Sprite3dBuilder)]` — bevy_sprite3d's PostUpdate `bundle_builder` system fills the required components with the cached quad. Tests can still assert `Mesh3d` and `MeshMaterial3d` are present after a frame.
- Default `Sprite3d.alpha_mode` is `Mask(0.5)` (constant `DEFAULT_ALPHA_MODE` in `lib.rs:39`), `unlit` is `false`. Always set both explicitly for documentation and to make verification greps meaningful.
- Image dimensions encode aspect ratio: use 14×18 px for a 1.4×1.8m sprite with `pixels_per_metre = 10.0`. 1×1 placeholders would force a square sprite or non-uniform `Transform.scale`.
- `Image::new_fill` images are inserted into `Assets<Image>` synchronously via `images.add(...)`. By the time bevy_sprite3d's PostUpdate `bundle_builder` runs in the same frame, the handle resolves cleanly. **But** if FOEs (#22) ever load images from disk, they MUST wait for `Loaded` state (the crate's `examples/sprite.rs` shows the pattern) — the crate's `bundle_builder` panics if the image handle doesn't resolve.
- Idempotent plugin registration pattern: `if !app.is_plugin_added::<Sprite3dPlugin>() { app.add_plugins(Sprite3dPlugin); }` — lets future plugins also register Sprite3dPlugin without double-add panic.
- The crate's `Sprite3dCaches` resource caches meshes by `[width, height, pivot_x, pivot_y, double_sided, frac_rect.min/max]` and materials by `(image, alpha_mode, unlit, emissive, flip_x, flip_y)`. Same-dimension placeholders share one mesh; per-enemy colours produce per-enemy materials.

**Marker rename:** to avoid collision with `bevy_sprite3d::Sprite3d`, our project-local marker was renamed `Sprite3dBillboard → EnemyBillboard`. `face_camera` query filter becomes `With<EnemyBillboard>`.

**Pipeline state:** Plan revised in-place (no new file). 12 steps now (was 11; +1 for the Cargo.toml step). LOC estimate revised down from +450-550 to +430-530 (net -20 LOC because bevy_sprite3d handles mesh+material construction). Test count unchanged at +14. Pipeline state at `project/orchestrator/PIPELINE-STATE.md` updated.
