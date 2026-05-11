---
name: Druum #17 enemy billboard sprite rendering — what's already in tree
description: Critical pre-shipped infrastructure for Feature #17 — combat camera reuse, enemy spawn lifecycle, EnemyDb stub, RON-loading wiring all already in place; #17 is a NEW plugin attached to an existing pipeline
type: reference
---

When researching or planning Feature #17 (Enemy Billboard Sprite Rendering) for Druum, do not re-derive these. They are already present in tree as of 2026-05-11:

**Camera contract: NO new Camera3d**
- `combat/ui_combat.rs:3` documents D-Q1=A explicitly: "NO new Camera3d. Overlays the existing dungeon camera."
- `combat/ui_combat.rs:64-71` `attach_egui_to_dungeon_camera` attaches `PrimaryEguiContext` to `Query<Entity, (With<DungeonCamera>, Without<PrimaryEguiContext>)>`.
- `dungeon/mod.rs:88-92` declares `pub struct DungeonCamera` as the marker.
- `dungeon/mod.rs:516-549` spawns Camera3d as a child of PlayerParty inside `spawn_party_and_camera`.
- #17 sprites render in the SAME 3D scene; they query `Query<&GlobalTransform, With<DungeonCamera>>` (the camera is a CHILD entity, so `Transform` is local — `GlobalTransform` is required for world-space billboard math).

**Party preservation across Dungeon ↔ Combat round-trip**
- `dungeon/mod.rs:599-608` `despawn_dungeon_entities` checks `matches!(state.get(), GameState::Combat)` and PRESERVES the PlayerParty (and its camera child) on `OnExit(Dungeon)` only when going to Combat.
- This means `DungeonCamera` is the same entity in dungeon AND combat — no re-spawn, no re-attach.

**Enemy ECS spawn pipeline (#16 already wired)**
- `combat/encounter.rs:309-411` `handle_encounter_request` is the SOLE writer of `CurrentEncounter` and the SOLE spawner of `Enemy`-marked entities.
- Spawn site at `encounter.rs:367-380`: `commands.spawn(EnemyBundle { ... }).id()`.
- `combat/enemy.rs:39-51` `EnemyBundle` is the spawn shape; adding fields with `Default` makes them auto-populate via `..Default::default()`.
- Despawn site at `encounter.rs:200-215` `clear_current_encounter` runs on `OnExit(Combat)` and despawns by `Query<Entity, With<Enemy>>` — sweeps the marker, so visual components attached to the SAME Enemy entity are despawned for free.

**EnemyDb is wired, just empty**
- `data/enemies.rs:1-12` declares `EnemyDb` as `#[derive(Asset, Reflect, Serialize, Deserialize, Default, Debug, Clone)]` with an empty body.
- `loading/mod.rs:111` `RonAssetPlugin::<EnemyDb>::new(&["enemies.ron"])` is registered.
- `loading/mod.rs:42-43` `enemy_db: Handle<EnemyDb>` is in `DungeonAssets`.
- `assets/enemies/core.enemies.ron` exists with placeholder `()`.
- #17 fills the schema (add real fields + derives) and the data file; no new plugin registration.

**EnemySpec inline pattern + documented migration path**
- `data/encounters.rs:8-11` doc comment EXPLICITLY states: "Until #17 ships `EnemyDb`, encounter tables carry full `BaseStats`/`DerivedStats`/`EnemyAi` inline. Migration path: add `enemy_id: Option<String>` to `EnemySpec` (additive); resolver falls back to inline when `None`."
- `assets/encounters/floor_01.encounters.ron` has 4 inline `EnemySpec` rows that need to keep working during the migration.

**"3d" feature already pulls bevy_sprite**
- `Cargo.toml:10` `bevy = { ..., features = ["3d"] }`.
- Verified at `bevy-0.18.1/Cargo.toml:2322-2330`: `3d` → `ui` (among others).
- Verified at `bevy_internal-0.18.1/Cargo.toml:172-176`: `bevy_ui = ["dep:bevy_ui", "bevy_text", "bevy_sprite"]`.
- So `Sprite`, `TextureAtlas`, `TextureAtlasLayout` are in `bevy::prelude` already. Δ Cargo.toml = 0 for the manual fallback path.

**Mesh + Material idiom on disk (HIGH-confidence verification)**
- `bevy-0.18.1/examples/3d/3d_scene.rs:25-28`: `(Mesh3d(meshes.add(Cuboid::new(...))), MeshMaterial3d(materials.add(Color::...)), Transform::from_xyz(...))`.
- `bevy-0.18.1/examples/3d/transparency_3d.rs:36-37`: `StandardMaterial { alpha_mode: AlphaMode::Mask(0.5), unlit: true, ... }`.
- `bevy-0.18.1/examples/3d/3d_shapes.rs:221-247` and `bevy-0.18.1/examples/3d/render_to_texture.rs:30-38`: `Image::new_fill(Extent3d { width, height, depth_or_array_layers: 1 }, TextureDimension::D2, &[r,g,b,a], TextureFormat::Rgba8UnormSrgb, RenderAssetUsages::RENDER_WORLD)` — pattern for runtime-generated textures from a colour value.
- `bevy-0.18.1/examples/2d/sprite_animation.rs:107-145`: `TextureAtlasLayout::from_grid(UVec2::splat(N), cols, rows, None, None)` + `Sprite { texture_atlas: Some(TextureAtlas { layout, index }), ..default() }` — pattern for frame stepping.

**`bevy_sprite3d` 7.x compatibility with Bevy 0.18.1 — UNVERIFIED in any session**
- The roadmap says "use bevy_sprite3d 7.x"; the user has authorised shipping the manual fallback in this PR.
- The Step A/B/C verification gate (per `feedback_third_party_crate_step_a_b_c_pattern.md`) is REQUIRED before any cargo add. Skip and assume manual fallback by default.

**Why:** Feature #17 looks like a fresh 500-LOC rendering feature, but ~80% of the wiring is in tree. The actual #17 scope is:
1. ONE new plugin (`enemy_render.rs`) reading existing pipelines.
2. Fill in the EnemyDb schema (additive — keeps inline EnemySpec working).
3. Populate `core.enemies.ron` with 10 enemies.
4. (Optional follow-up) `bevy_sprite3d` swap behind the Step A/B/C gate.

**How to apply:** When researching/planning #17, cite the file:line locations above rather than re-deriving the architecture. The recommendation should be "compose existing plumbing + add render plugin" not "design from scratch". The face-camera math and AlphaMode::Mask(0.5) choice are the only architectural calls that aren't pre-decided in tree.
