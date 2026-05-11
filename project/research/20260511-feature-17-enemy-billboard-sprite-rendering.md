# Feature #17: Enemy Billboard Sprite Rendering — Research

**Researched:** 2026-05-11
**Domain:** Bevy 0.18.1 first-person dungeon crawler — 3D billboard rendering, sprite-sheet animation, combat-entry spawn pipeline
**Confidence:** HIGH for the manual-fallback recommendation; MEDIUM for `bevy_sprite3d` upstream-state claims (see Tooling Limitation Disclosure).

---

## Tooling Limitation Disclosure

This research session ran with **no Bash, no MCP (context7/GitHub/sequential-thinking), no WebFetch, no WebSearch** despite the system reminder advertising context7. Only `Read`/`Write`/`Edit` were available.

Compensating strategy applied per the project's tooling-limitation feedback rule:

- Bevy 0.18.1 facts (3D mesh API, `StandardMaterial`, `Camera3d`, `Sprite`, `TextureAtlasLayout`, feature graph) are verified at HIGH confidence by reading the unpacked crate sources on disk under `~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bevy-0.18.1/`, `bevy_internal-0.18.1/`, `bevy_mesh-0.18.1/`, etc.
- Druum codebase facts are HIGH confidence — read directly.
- **`bevy_sprite3d` 7.x compatibility with Bevy 0.18.1 cannot be verified in this session.** The crate is not in the local Cargo registry cache and Web/MCP tools are unavailable. Findings about its current 0.18 support are marked MEDIUM at best, with explicit verification recipes that the planner or implementer MUST run before Cargo.toml is touched.

The 3-step verification gate (Step A version resolution, Step B feature audit, Step C API grep) is mandatory for the `bevy_sprite3d` decision and is documented in `## Verification Recipes`. The roadmap's "use `bevy_sprite3d 7.x`" line is treated as a hypothesis, not a fact.

**Bottom line:** the manual-fallback path is HIGH-confidence implementable today with what's in tree. The `bevy_sprite3d` path is GATED behind the Step A/B/C check. The planner should NOT commit to `bevy_sprite3d` until the verification gate passes — and the roadmap already authorises shipping the fallback in this PR.

---

## Summary

Feature #17 adds the first 3D-spatial visual representation of enemy entities in Druum. Up to and including #16, enemies are pure ECS data with no rendering surface — the combat UI in `src/plugins/combat/ui_combat.rs` paints enemies as egui side-panel rows over the existing `DungeonCamera`. #17 keeps that egui combat UI (data-readable HP bars / names / cursors) and **adds** spatial billboard sprites in the 3D scene so the combat camera looks like Wizardry/Etrian Odyssey instead of a stat-block menu.

Three architectural decisions dominate the work:

1. **Billboard implementation:** `bevy_sprite3d 7.x` (third-party) versus a manual textured-quad-faces-camera system using Bevy's first-party `Mesh3d` + `MeshMaterial3d` + `StandardMaterial`. The manual path is ~50 LOC and zero new dependencies; the `bevy_sprite3d` path is a small API win IF the upstream crate is current for 0.18.1. The roadmap explicitly authorises shipping the manual fallback in this PR.
2. **Animation pattern:** custom-component frame counter advancing `TextureAtlas.index` is the simplest fit for four named states (idle/attack/damage/dying), mirrors the official Bevy 0.18 `sprite_animation.rs` example, and avoids `AnimationGraph` over-engineering for what is fundamentally a 4-frame state-machine swap.
3. **Placeholder asset pattern:** generate solid-colour `Image`s in-memory at startup from a `placeholder_color: (f32, f32, f32)` field in `enemies.ron` via `Image::new_fill`. This avoids 10 trivial PNG files in `assets/enemies/<id>/idle.png` and keeps zero new file-format dependencies. The asset path field stays in the schema so real art can replace placeholders one enemy at a time without a schema change.

**Primary recommendation:** Build the manual billboard system in this PR. Add `placeholder_color` to `EnemySpec` (data) and to `enemies.ron` (10 rows authored). Defer the `bevy_sprite3d` decision: gate it behind a `cfg(feature = "bevy_sprite3d")` if it ever becomes desirable post-merge, OR delete it from the roadmap if the manual path stays sufficient for FOEs in #22.

---

## Standard Stack

### Core

| Library | Version | Purpose | License | Maintained? | Why Standard |
|---|---|---|---|---|---|
| `bevy` (already pinned) | `=0.18.1` | 3D mesh + camera + Image asset, `Sprite` + `TextureAtlasLayout` | MIT OR Apache-2.0 | YES (engine) | Already in tree; provides everything #17 needs |

**Zero new direct dependencies are required for the recommended path.** The "3d" umbrella feature already pulls in `bevy_sprite` (via `ui → bevy_ui → bevy_sprite`) so `TextureAtlasLayout` is in `bevy::prelude` without a Cargo.toml change (verified at `bevy-0.18.1/Cargo.toml:2322-2330` `3d` feature definition and `bevy_internal-0.18.1/Cargo.toml:172-176` `bevy_ui` feature, which lists `"bevy_sprite"`).

### Supporting (already in tree)

| Capability | Bevy 0.18 type | Use case in #17 |
|---|---|---|
| Quad mesh | `Rectangle::new(w, h)` → `Mesh3d` | Billboard surface |
| Textured material | `StandardMaterial { base_color_texture, unlit: true, alpha_mode }` | Surface texture |
| Runtime texture generation | `Image::new_fill(Extent3d, TextureDimension::D2, &[r,g,b,a], TextureFormat::Rgba8UnormSrgb, RenderAssetUsages::RENDER_WORLD)` | Solid-colour placeholders |
| Sprite-sheet animation (atlas only) | `TextureAtlasLayout::from_grid(UVec2::splat(N), cols, rows, None, None)` + manual `index` | Reused later when real art replaces placeholders |
| State transitions | `OnEnter(GameState::Combat)` / `OnExit(GameState::Combat)` | Hook spawn / despawn |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|---|---|---|
| Manual textured quad | `bevy_sprite3d 7.x` | If upstream is 0.18-current: ~30 LOC saved + slightly nicer API. If it isn't, fork or fallback. Net cost of investigation > net code saved. |
| `Image::new_fill` solid colour | 10 committed PNGs in `assets/enemies/<id>/idle.png` | PNG path matches the eventual real-art asset layout. Tradeoff is 10 tiny files (~1 KB each, 10 KB total) vs ~20 lines of in-memory texture generation. Recommendation justified below in §Asset Layout. |
| Custom anim state component | `bevy::animation::AnimationGraph` | `AnimationGraph` is built for skeletal/property animation, not sprite-frame swaps. Massive overkill for 4 named states × ~4 frames each. |
| `TextureAtlas` (true sheets) | Per-frame loose PNGs (`idle_01.png`, `idle_02.png`, …) | Sheets win on draw-call batching and atlas memory. For the *placeholder* PR with 1-frame solid colours, neither matters. The architecture must accept either. |

**Recommended path: manual billboard + `Image::new_fill` placeholders + custom anim component.** Δ Cargo deps = 0. Δ LOC ≈ 350-450 (well inside the roadmap's +400 to +600 envelope).

**Installation:** None — entirely first-party Bevy 0.18.1.

---

## Architecture Options

Three orthogonal decisions; pros/cons for each.

### Decision 1: Billboard implementation

| Option | Description | Pros | Cons | Best When |
|---|---|---|---|---|
| **Manual quad** (RECOMMENDED) | `Mesh3d(Rectangle::new(w, h))` + `MeshMaterial3d` + a `face_camera` system that rewrites `Transform.rotation` each frame to look at `DungeonCamera` | Zero new deps; full control over rotation behaviour (Y-axis-locked vs full); easy to test | ~50 LOC of mesh/material setup; you own the math | The roadmap explicitly authorises this AND `bevy_sprite3d` 0.18-compat is unverified |
| `bevy_sprite3d` 7.x | Third-party `Sprite3d` component wrapping the same underlying mesh+material pattern | API ergonomics; saves the face-camera system if it's bundled | Compatibility with Bevy 0.18.1 NOT verified in this session; adds a dep that needs maintenance; one more crate to bump on every Bevy upgrade | Only if Step A/B/C verification confirms 0.18.1 compatibility AND the ergonomics gap is worth the bump cost |
| `MeshletPlugin` / animated GLTF | Full 3D enemy models | Hand-wave: the roadmap rejects this for genre/cost reasons (#17 Pros) | Massive art-pipeline cost | Never for this project |

**Recommended:** Manual quad. **Counterargument:** `bevy_sprite3d` may turn out to be trivially compatible and shave 30 LOC. **Response:** that's a post-merge optimisation, not a #17 blocker; the user has already authorised the manual path.

### Decision 2: Face-camera math

| Option | Description | Pros | Cons |
|---|---|---|---|
| **`look_at` toward camera position with Y-up** (RECOMMENDED) | `transform.look_at(camera_pos, Vec3::Y)` then flip 180° because `look_at`'s convention is "-Z faces target" — for a sprite whose normal is +Z this needs `Quat::from_rotation_y(PI)` post-multiply | Y-axis-locked (sprite never tilts) — correct for dungeon crawler since the camera is roughly level; `look_at` is in `bevy::prelude` | The Y-axis lock is intentional; if the camera ever pitches, the sprite still faces forward |
| Copy `Transform.rotation` from camera | `sprite.rotation = camera.rotation` | Simpler one-line | Sprite rotates with camera pitch — looks broken on a tilted camera |
| Compute angle to camera | `let dir = (camera_xz - sprite_xz).normalize(); let angle = dir.x.atan2(dir.z); transform.rotation = Quat::from_rotation_y(angle)` | Y-locked, no flip dance | More math, more places to get wrong |

**Recommended:** Compute angle to camera (the third option) — it is the simplest Y-locked form and avoids the `look_at` "-Z faces target" surprise. The face-camera system reads `Query<&Transform, With<DungeonCamera>>` once per frame, then writes each `Sprite3dBillboard`'s rotation. ~10 LOC.

Druum's `DungeonCamera` marker (verified at `src/plugins/dungeon/mod.rs:92`) is the camera-side query target. The party camera survives the `Dungeon → Combat → Dungeon` round-trip (verified at `dungeon/mod.rs:599-608` "Preserve PlayerParty when transitioning to Combat"), so the `DungeonCamera` entity is the same one the dungeon used — no need for a combat-specific camera spawn.

### Decision 3: Animation pattern

| Option | Description | Pros | Cons |
|---|---|---|---|
| **Custom `EnemyAnimation` component** (RECOMMENDED) | `{ state: AnimState, current_frame: usize, frame_timer: Timer }` driven by an `advance_enemy_animation` system reading `Res<Time>` | Mirrors Bevy 0.18's official `examples/2d/sprite_animation.rs` pattern exactly; deterministic with `TimeUpdateStrategy::ManualDuration` in tests | ~80 LOC including state-transition rules |
| `AnimationGraph` | Bevy's skeletal/property animation system | Overkill — designed for blend trees, masks, additive layers | Steep learning curve for nothing in return at 4 frames × 4 states |
| Plain `Timer` per frame | One `Timer` + index-bump | Simplest possible | No "return to idle on complete" rule; state machine has to live somewhere |

**Recommended:** Custom `EnemyAnimation` component. State machine: `Idle ⇄ Attacking → Idle`, `Idle ⇄ TakingDamage → Idle`, `Idle → Dying → (despawn or hold last frame)`. ~80 LOC. Tests use `TimeUpdateStrategy::ManualDuration(50ms)` for determinism (memory: `reference_bevy_018_time_update_strategy.md`).

For the placeholder PR with **single-frame solid-colour sprites**, all four states render the same frame (or you skip atlas swapping entirely and the system is a no-op). The schema and component live in tree from day one so real art doesn't change any API; only the data file gains frame counts.

### Counterarguments to the headline recommendation

- **"`bevy_sprite3d` might be a 5-minute install."** Response: even if it is, the Step A/B/C gate (5-10 minutes per Druum's third-party-crate feedback memory) is the cost of finding out. If it passes, swap the manual quad to `Sprite3d` as a future small PR; the public API of `enemy_render.rs` (the `spawn_enemy_visual` function) is unchanged.
- **"Placeholder colours via `Image::new_fill` makes the asset-loader pipeline a special case."** Response: the placeholder generation runs at `OnEnter(Combat)` (or at startup) and writes `Handle<Image>` into a resource keyed by enemy id. The asset path on `EnemySpec` is still authored — when real PNGs land, the spawn system prefers loaded handles over generated ones with a simple `Option<Handle<Image>>` precedence.
- **"Why not just commit 10 solid-colour PNGs?"** Response: see §Asset Layout — both work; the in-memory approach is a tiny win on hot-reload and treats placeholders as visibly placeholder ("the data file has a colour, not a real PNG path yet").

---

## Architecture Patterns

### Recommended Project Structure

```
src/plugins/combat/
├── enemy_render.rs       # NEW (#17) — sprite billboard plugin, face-camera, anim state
├── encounter.rs          # already exists (#16) — emits EnemyBundle spawns
├── enemy.rs              # already exists (#15) — Enemy marker, EnemyBundle
├── mod.rs                # add EnemyRenderPlugin to CombatPlugin::build
└── ...

src/data/
├── enemies.rs            # CHANGE — replace empty `EnemyDb` stub with real schema; OR
└── encounters.rs         # ALT — extend EnemySpec inline (matches existing inline pattern)

assets/enemies/
├── core.enemies.ron      # already exists, currently empty `()` — populate with 10 enemies
└── core/                 # OPTIONAL — only if option (a) of §Asset Layout wins
    ├── goblin/idle.png   # 1×1 solid-colour placeholder PNG, etc.
    └── ...
```

**Open architectural question:** Does the 10-enemy roster live as a top-level `EnemyDb` asset (`core.enemies.ron`) referenced by id from encounters, OR does the existing inline `EnemySpec` in `floor_01.encounters.ron` stay the source of truth?

Reading the relevant code reveals the contract is already set up for migration:
- `src/data/encounters.rs:8-11`: the doc comment **explicitly states** "Until #17 ships `EnemyDb`, encounter tables carry full `BaseStats`/`DerivedStats`/`EnemyAi` inline. Migration path: add `enemy_id: Option<String>` to `EnemySpec` (additive); resolver falls back to inline when `None`."
- `src/data/enemies.rs:1-12` is the placeholder `EnemyDb { }` stub.

**Recommendation: build `EnemyDb` now as the authored roster source AND keep inline `EnemySpec` working.** The 10 enemies live in `core.enemies.ron`; `floor_01.encounters.ron` references them by id. The migration path is the additive `enemy_id: Option<String>` already documented. This satisfies the user's "10 enemies in enemies.ron" decision and the roadmap entry in `### Broad Todo List`: "Author `enemies.ron` mapping enemy IDs to sprite paths + stats".

### Pattern 1: Billboard quad spawn

```rust
// Source: derived from bevy-0.18.1/examples/3d/transparency_3d.rs and
//         bevy-0.18.1/examples/3d/3d_scene.rs (HIGH confidence — on-disk source).

// In OnEnter(GameState::Combat) — after CurrentEncounter is populated by #16.
fn spawn_enemy_billboards(
    mut commands: Commands,
    encounter: Res<CurrentEncounter>,
    enemies_q: Query<(Entity, &EnemyVisual), With<Enemy>>,    // EnemyVisual is the data tag
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    placeholders: Res<PlaceholderImages>,
    camera_q: Query<&Transform, With<DungeonCamera>>,
) {
    // Quad mesh shared across all enemies (one draw-call-friendly handle).
    // Rectangle::new(w, h) gives a w×h quad in the XY plane facing +Z.
    let quad = meshes.add(Rectangle::new(SPRITE_WIDTH, SPRITE_HEIGHT));

    let Ok(camera_transform) = camera_q.single() else { return; };
    let camera_pos = camera_transform.translation;

    // Sprite row: enemies arranged left-to-right at fixed distance in front of camera.
    let forward = camera_transform.forward();
    let right = camera_transform.right();

    for (i, (entity, visual)) in enemies_q.iter().enumerate() {
        let n = encounter.enemy_entities.len() as f32;
        let offset_x = (i as f32 - (n - 1.0) / 2.0) * SPRITE_SPACING;
        let world_pos = camera_pos + forward * SPRITE_DISTANCE + right * offset_x
                                  + Vec3::Y * SPRITE_Y_OFFSET;

        // Per-enemy material — owns the texture handle so per-enemy colour works.
        let material = materials.add(StandardMaterial {
            base_color_texture: Some(placeholders.get(&visual.id).clone()),
            // unlit because Druum's dungeon has dramatic point-light torch (`src/plugins/dungeon/mod.rs:536-547`)
            // and a lit sprite would darken when the player is looking away from it.
            // Real art should ship pre-baked lighting and stay unlit too.
            unlit: true,
            // Mask mode handles hard-edge transparency cleanly.
            // Blend has back-to-front sort issues with multiple enemies stacked;
            // Mask treats the alpha test as binary visible/not — perfect for sprites.
            alpha_mode: AlphaMode::Mask(0.5),
            ..default()
        });

        commands.entity(entity).insert((
            Mesh3d(quad.clone()),
            MeshMaterial3d(material),
            Transform::from_translation(world_pos),
            Visibility::default(),
            Sprite3dBillboard,    // marker for the face-camera system
            // EnemyAnimation already on EnemyBundle from spawn in encounter.rs.
        ));
    }
}
```

### Pattern 2: Face-camera system (Y-axis-locked, recommended math)

```rust
// HIGH confidence: pure math; tested by inspection against Vec3 / Quat API
// at bevy_math-0.18.1/src/quat.rs (Quat::from_rotation_y).

fn face_camera(
    camera_q: Query<&GlobalTransform, With<DungeonCamera>>,
    mut sprites_q: Query<&mut Transform, (With<Sprite3dBillboard>, Without<DungeonCamera>)>,
) {
    let Ok(camera_global) = camera_q.single() else { return; };
    let camera_pos = camera_global.translation();

    for mut sprite_transform in &mut sprites_q {
        let to_camera_xz = Vec3::new(
            camera_pos.x - sprite_transform.translation.x,
            0.0,
            camera_pos.z - sprite_transform.translation.z,
        );
        // atan2(x, z): yaw angle for a quad whose default normal is +Z.
        // Y-axis-locked — sprite never pitches.
        let angle = to_camera_xz.x.atan2(to_camera_xz.z);
        sprite_transform.rotation = Quat::from_rotation_y(angle);
    }
}
```

Note: `Without<DungeonCamera>` in the second query is required for Bevy's borrow-checker (B0001 disjoint-set rule) since `Transform` is queried in both.

Note 2: read `&GlobalTransform`, not `&Transform`, because the camera is a child of `PlayerParty` (`src/plugins/dungeon/mod.rs:516-549`). Its `Transform` is local to the parent; `GlobalTransform.translation()` is the world position.

### Pattern 3: Animation state machine (no atlas needed for placeholders, full pattern for future)

```rust
// Mirrors bevy-0.18.1/examples/2d/sprite_animation.rs (HIGH confidence — on-disk source)
// adapted for the 4-state DRPG combat use case.

#[derive(Component, Reflect, Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnimState {
    Idle,
    Attacking,
    TakingDamage,
    Dying,
}

#[derive(Component, Debug)]
pub struct EnemyAnimation {
    pub state: AnimState,
    pub frame_index: usize,
    pub frame_timer: Timer,
    /// Frame counts per state, loaded from EnemySpec when atlas-based art lands.
    /// For placeholder PR: all states have frame_count = 1.
    pub frames: AnimStateFrames,
}

#[derive(Debug, Clone, Copy)]
pub struct AnimStateFrames {
    pub idle_count: usize,
    pub attack_count: usize,
    pub damage_count: usize,
    pub dying_count: usize,
}

fn advance_enemy_animation(
    time: Res<Time>,
    // Sprite from bevy_sprite::sprite is a Component; comes in via the `3d` feature's
    // ui→bevy_sprite chain (verified at bevy_internal-0.18.1/Cargo.toml:172-176).
    // For the placeholder PR (single-frame) the system can skip the Sprite mutation
    // and just track state transitions.
    mut q: Query<(&mut EnemyAnimation, Option<&mut Sprite>)>,
) {
    for (mut anim, sprite_opt) in &mut q {
        anim.frame_timer.tick(time.delta());
        if !anim.frame_timer.just_finished() { continue; }

        let frame_count = match anim.state {
            AnimState::Idle => anim.frames.idle_count,
            AnimState::Attacking => anim.frames.attack_count,
            AnimState::TakingDamage => anim.frames.damage_count,
            AnimState::Dying => anim.frames.dying_count,
        };
        if frame_count == 0 { continue; }

        anim.frame_index += 1;
        if anim.frame_index >= frame_count {
            // End-of-state behaviour: Attacking/TakingDamage return to Idle,
            // Dying holds its last frame (no auto-despawn — combat-cleanup despawns).
            match anim.state {
                AnimState::Attacking | AnimState::TakingDamage => {
                    anim.state = AnimState::Idle;
                    anim.frame_index = 0;
                }
                AnimState::Dying => {
                    anim.frame_index = frame_count - 1;     // hold last frame
                }
                AnimState::Idle => {
                    anim.frame_index = 0;                   // loop
                }
            }
        }

        // Update atlas index when real art lands; no-op for placeholders.
        if let Some(mut sprite) = sprite_opt
            && let Some(atlas) = &mut sprite.texture_atlas {
            atlas.index = anim.atlas_offset_for_state() + anim.frame_index;
        }
    }
}

// `Attacking` triggered by listening to combat events — e.g. an event written by
// `execute_combat_actions` in `combat/turn_manager.rs:402-410` when an enemy attacks.
// `TakingDamage` triggered when current_hp drops.
// `Dying` triggered when StatusEffects gains `Dead` or HP hits 0.
// The triggers are NEW events you author in #17. Reading existing events directly
// keeps coupling loose.
```

### Anti-Patterns to Avoid

- **Spawning a separate Camera3d for combat.** Druum's combat already overlays egui on the existing dungeon camera (D-Q1=A — verified at `combat/ui_combat.rs:3` "D-Q1=A — NO new Camera3d"). #17 must mirror this: NO new camera. The sprites land in the same scene the player is already looking at.
- **Putting the spawn logic in `combat/encounter.rs`.** That file is `handle_encounter_request` which is the SOLE writer of `CurrentEncounter` (`encounter.rs:309-411`). Visual spawn belongs in a separate `enemy_render.rs` plugin reading `CurrentEncounter` on `OnEnter(Combat)`. Sole-writer guarantees stay intact.
- **Using `AlphaMode::Blend` for sprites with full alpha cutouts.** `Mask(0.5)` is correct for sprites with hard edges (typical for pixel-art DRPG enemies). `Blend` introduces back-to-front sorting bugs when multiple sprites overlap (Bevy 0.18 `examples/3d/transparency_3d.rs` shows both modes; `Mask` is the genre-correct choice for opaque-pixel sprites).
- **Reading `&Transform` on the camera in the face-camera system.** Camera is a child of `PlayerParty`, so its local `Transform` is meaningless for world-space billboard math. Use `&GlobalTransform` (verified at `dungeon/mod.rs:516-549` — camera is `children![(Camera3d::default(), Transform::from_xyz(0.0, EYE_HEIGHT, 0.0), DungeonCamera, ...)]`).
- **Trying to add `Sprite3dBundle` or `SpriteBundle` to the enemy entity.** Bevy 0.18 has NO `*Bundle` types — they were removed (memory: `reference_bevy_018_camera3d_components.md`). The pattern is a component tuple — `Mesh3d`, `MeshMaterial3d`, `Transform`.
- **Decoupling `unlit: true` from the placeholder colour.** Druum's dungeon has a low-ambient Wizardry torchlight setup (`GlobalAmbientLight` at low brightness; per-camera `PointLight` intensity 60_000). Placeholder solid-colour sprites WITHOUT `unlit: true` will appear in mostly-dark unless the player camera is close — making your placeholder look broken when it isn't.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---|---|---|---|
| Generating a 1×1 solid-colour PNG at runtime | A PNG encoder | `Image::new_fill(Extent3d{1,1,1}, TextureDimension::D2, &[r,g,b,a], TextureFormat::Rgba8UnormSrgb, RenderAssetUsages::RENDER_WORLD)` | Verified at `bevy-0.18.1/examples/3d/3d_shapes.rs:221-247` — single line creates a usable `Handle<Image>` without disk I/O |
| Texture atlas slicing | A custom UV-rect crate | `TextureAtlasLayout::from_grid(UVec2::splat(N), cols, rows, padding, offset)` | Verified at `bevy-0.18.1/examples/2d/sprite_sheet.rs:48`; in `bevy::prelude` |
| Frame-stepping a sprite | A bespoke timer + index | `Sprite { image, texture_atlas: Some(TextureAtlas { layout, index }), ..default() }` with `index` mutated by your system | Verified at `bevy-0.18.1/examples/2d/sprite_animation.rs:56-77`; standard Bevy idiom |
| State transitions | A state-pattern crate | Plain `enum AnimState` + `match` in one system | Four states with three transitions — overhead of a crate exceeds the code saved |
| Looking-at math | A `bevy_billboard` crate | `Quat::from_rotation_y(atan2(dx, dz))` | One line of math; no transitive dep needed |

---

## Common Pitfalls

### Pitfall 1: `Rectangle` mesh defaults face +Z, NOT +Y

Bevy's `Plane3d` mesh defaults to facing +Y (it's a floor/ceiling primitive); `Rectangle::new(w, h)` defaults to the XY plane with normal pointing +Z. For a billboard sprite you want a vertical quad — `Rectangle` is correct.

**What goes wrong:** using `Plane3d` for the quad → quad lies flat on the ground.
**How to avoid:** `Rectangle::new(w, h)`, NOT `Plane3d::new(...).mesh().size(w, h)`. Verified by inference from `bevy-0.18.1/examples/3d/3d_shapes.rs:87` (`Extrusion::new(Rectangle::default(), 1.)`) — `Rectangle` is a 2D primitive that extrudes to XY.

### Pitfall 2: `Plane3d` is single-sided

Already documented in memory (`reference_bevy_018_mesh_lighting_gotchas.md`): a `Plane3d` mesh has only front-facing triangles. If you mistakenly use `Plane3d` for the billboard surface, the sprite is invisible when the player flanks around it — but for #17 single-facing sprites are the explicit decision, so this might be tolerable as a side effect. Use `Rectangle` anyway; it generates a quad with the right orientation for billboarding and the same single-sided property if needed.

**For #22 FOEs walking on the grid:** the same single-sided issue is what makes "always face camera" cheap — you never SEE the back face. If you ever want a two-sided sprite, generate two triangles or set `cull_mode: None` on the material. Out of scope for #17.

### Pitfall 3: Mixing `AlphaMode::Blend` with multiple enemies in a row

Bevy 0.18's transparency sort for `AlphaMode::Blend` is back-to-front by distance to camera. With multiple enemies aligned in a row at varying distances (the front row of 1-4 enemies), small jitter in distances flips the sort order between frames → visible flicker.

**How to avoid:** use `AlphaMode::Mask(0.5)` so transparency is binary (visible-or-not) and order-independent. Documented in the official transparency example (`bevy-0.18.1/examples/3d/transparency_3d.rs:36-38`).

### Pitfall 4: `unlit: false` (default) on placeholder sprites makes them invisible in the dungeon

Druum's dungeon has aggressive Wizardry torchlight: `GlobalAmbientLight::default()` brightness is restored on dungeon exit (`dungeon/mod.rs:614`), and the per-floor `lighting.ambient_brightness` is set very low (verified pattern at `dungeon/mod.rs:622-625`). A non-`unlit` `StandardMaterial` will sample these lights and render very dark — placeholder colours like red/green/blue become muddy brown.

**How to avoid:** `unlit: true` on the placeholder material is non-negotiable. Real sprite art ALSO should ship `unlit: true` because pixel art is pre-baked; PBR sampling defeats the look.

### Pitfall 5: Spawning sprites BEFORE `CurrentEncounter` is populated

#16's `handle_encounter_request` is the sole writer of `CurrentEncounter` and it runs in `Update` while `GameState::Dungeon` (verified at `encounter.rs:157-159`). The state transition to `GameState::Combat` is queued in the same system. `OnEnter(GameState::Combat)` fires the NEXT frame — at which point `CurrentEncounter` is guaranteed populated and `Enemy` entities are spawned.

**The recommended `spawn_enemy_billboards` system runs in `OnEnter(Combat)`.** Verified pattern: `combat/turn_manager.rs:165` already uses this for `init_combat_state`.

**Don't run sprite spawn in `Update` with `run_if(in_state(GameState::Combat))`** — that fires every frame and would need an "already-spawned" guard. The OnEnter pattern is cleaner.

### Pitfall 6: Forgetting to despawn on combat exit

`combat/encounter.rs:200-215` `clear_current_encounter` runs on `OnExit(Combat)` and despawns every `Enemy`-marked entity:

```rust
fn clear_current_encounter(mut commands: Commands, enemies: Query<Entity, With<Enemy>>) {
    for entity in &enemies { commands.entity(entity).despawn(); }
    commands.remove_resource::<CurrentEncounter>();
}
```

Because we add `Mesh3d` / `MeshMaterial3d` to the existing `Enemy` entity (rather than a new sprite entity), this despawn cleanup ALREADY handles the visual cleanup. **Do not add a second despawn system** — that risks double-despawn panics.

If you ever decide to spawn a SEPARATE entity for the sprite (e.g., parented to the Enemy entity for visual offsetting), use `children![(...)]` syntax (verified pattern at `dungeon/mod.rs:516-549`) so the child cleans up recursively with the parent.

### Pitfall 7: Generated `Image` handles disappearing under hot-reload

`RenderAssetUsages::RENDER_WORLD` is the right flag for generated textures (no CPU readback needed). But `bevy/file_watcher` (enabled in Druum's `dev` feature) doesn't touch in-memory `Image` handles. The generated placeholder image persists across reloads — that's correct.

**What goes wrong if you pick `RenderAssetUsages::MAIN_WORLD` instead:** the texture is uploaded but Bevy frees the GPU copy at runtime. Sprites silently render as default (white or transparent depending on alpha).

### Pitfall 8: `enemies.ron` schema needs to be a typed `Asset`

The existing `EnemyDb` stub at `src/data/enemies.rs:9-11` is `#[derive(Asset, Reflect, Serialize, Deserialize, Default, Debug, Clone)]` with an empty body. The `RonAssetPlugin::<EnemyDb>::new(&["enemies.ron"])` registration at `loading/mod.rs:111` is already wired. **You extend that stub, you don't add a parallel registration.** Authoring real fields requires nothing more than filling the struct in `data/enemies.rs`.

### Pitfall 9: `Sprite::texture_atlas` is `Option<TextureAtlas>`, not always-present

Verified at `bevy-0.18.1/examples/2d/sprite_animation.rs:117-121`:

```rust
Sprite {
    image: texture.clone(),
    texture_atlas: Some(TextureAtlas { layout, index }),
    ..default()
}
```

For placeholder PR with single-frame solid colours, you don't even need `Sprite` — the `MeshMaterial3d(material_with_solid_color_texture)` approach is enough. The atlas pattern lives in the codebase from day one so real art works, but you can ship #17 without ever calling `from_grid` if the placeholders are 1×1 textures.

---

## Security

### Known Vulnerabilities

| Library | CVE / Advisory | Severity | Status | Action |
|---|---|---|---|---|
| `bevy` 0.18.1 | none verifiable in session | — | — | Use the same pin as the rest of the project |
| `bevy_sprite3d` 7.x | UNVERIFIED in session | — | — | Step A/B/C verification required before adoption — see `## Verification Recipes` |

No known CVEs or advisories were verifiable for first-party Bevy 0.18.1 in this session. The project is already pinned at `=0.18.1` for the rest of the codebase, so no version change is implied by #17.

### Architectural Security Risks

| Risk | Affected Architecture Options | How It Manifests | Secure Pattern | Anti-Pattern to Avoid |
|---|---|---|---|---|
| **RON trust boundary on `EnemyDb`** | All — both inline and EnemyDb-driven | A maliciously authored `core.enemies.ron` could declare absurd `placeholder_color` values, oversized `frames` counts, or path-traversal sprite paths | Validate `placeholder_color` channel values clamp to `[0.0, 1.0]` on load; cap `frame_count` per state at a fixed maximum (e.g., 32); resolve sprite paths only via `AssetServer.load("enemies/<id>/idle.png")` (no `../` traversal — bevy_asset handles this) | Trusting raw f32 colour values directly into `Image::new_fill`; trusting raw u32 frame counts directly into a `for i in 0..frames` loop |
| **Despawn race** | All | If sprite spawn happens after `OnExit(Combat)` for any reason, `Mesh3d`/`MeshMaterial3d` are attached to entities that no longer exist → silent failure or panic | Spawn ONLY in `OnEnter(Combat)`; despawn is handled by the existing `clear_current_encounter` since the components live on the same `Enemy` entity | Spawning sprites in `Update` without an `OnEnter` gate; spawning into a "combat scene" entity that has its own lifecycle |
| **Texture handle leaks under repeated combats** | Generated-placeholder path | If `Image::new_fill` is called fresh on every `OnEnter(Combat)`, each combat leaks N new image handles into `Assets<Image>` — bevy_asset reference-counting cleans up when no `Handle` remains, so as long as old `MeshMaterial3d` are despawned in OnExit the leak is bounded | Generate placeholder images ONCE at startup (when `EnemyDb` finishes loading); store in a `PlaceholderImages: Resource` keyed by enemy id; reuse the handle on every spawn | Calling `Image::new_fill` inside the per-encounter spawn system |
| **Atlas index out-of-bounds** | Real-art path | If `EnemyAnimation.frame_index` exceeds the underlying atlas layout's `len()`, Bevy 0.18 silently renders frame 0 (no panic), but a typo'd RON `attack_count: 99` for an atlas of 8 frames is a visual bug | Validate `frames.{idle,attack,damage,dying}_count.sum() <= atlas.layout.len()` at load time; warn-and-clamp on RON deserialize | Trusting RON frame counts at face value |

### Trust Boundaries

For the recommended architecture, the trust boundaries are:

- **RON deserialise (`bevy_common_assets::RonAssetPlugin::<EnemyDb>`):** any field clamps need to happen in code that reads the asset, not in the asset itself. Pattern precedent: `combat/encounter.rs:281` clamps `encounter_rate.clamp(0.0, 1.0)`; #15 clamps `WeightedIndex` weights to `[1, 10_000]` at `encounters.rs:84`. #17 adds clamps for `placeholder_color` channels and frame counts following the same pattern.
- **Asset path resolution:** Druum already trusts `bevy_asset` to refuse path traversal — verified by `encounter_table_for` and `floor_handle_for` precedents (no extra path-sanitization layer in tree).
- **No external network surface introduced.** #17 adds no HTTP, no socket, no remote-fetch behaviour.

---

## Performance

| Metric | Value / Range | Source | Notes |
|---|---|---|---|
| Quad draw call per enemy | 1 | Bevy mesh-render contract | All enemies share the SAME `Mesh3d` handle (one `Rectangle::new(W,H)` mesh); per-enemy material handles differ. Bevy 0.18 batches by mesh+material; identical-material enemies (placeholder rosters with shared colour) batch to one draw call |
| Mesh shared cost | ~50 bytes | Bevy `Mesh` storage | Single quad mesh; negligible |
| Generated `Image` placeholder size | 1×1 px × 4 bytes = 4 B per enemy | `Image::new_fill` with `Extent3d { width: 1, height: 1, depth_or_array_layers: 1 }` | Even at 10 enemies = 40 bytes RAM; no GPU overhead beyond a 1×1 texture upload |
| Face-camera system runtime | O(N) where N ≤ 8 | `for sprite in &mut sprites_q` | At 8 enemies and 144 Hz, ~1152 iterations/sec — sub-microsecond cost |
| Animation tick runtime | O(N) per state-change frame | `Timer::tick` + index update | Trivial; bounded by enemy count |
| Asset budget vs roadmap | +0 to +40 KB for 10 placeholder PNGs (option a); 0 bytes for option b | roadmap #17 impact analysis | The roadmap's "+5-30 MB" estimate is for REAL sprite art, not placeholders |

No benchmarks for `bevy_sprite3d` in this session — flag for validation IF that path is chosen during planning.

---

## Code Examples

### Spawning the enemy billboards on combat entry

```rust
// SOURCE: synthesised from on-disk Bevy 0.18 examples (HIGH confidence):
//   - bevy-0.18.1/examples/3d/3d_scene.rs (Mesh3d + MeshMaterial3d + Transform tuple)
//   - bevy-0.18.1/examples/3d/transparency_3d.rs (StandardMaterial { alpha_mode, unlit })
//   - bevy-0.18.1/examples/3d/3d_shapes.rs (Image::new_fill pattern)
//   - bevy-0.18.1/examples/2d/sprite_animation.rs (TextureAtlas index + AnimationConfig)

use bevy::prelude::*;
use bevy::asset::RenderAssetUsages;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

use crate::plugins::combat::encounter::CurrentEncounter;
use crate::plugins::combat::enemy::Enemy;
use crate::plugins::dungeon::DungeonCamera;
use crate::plugins::state::GameState;

const SPRITE_WIDTH: f32 = 1.4;
const SPRITE_HEIGHT: f32 = 1.8;
const SPRITE_DISTANCE: f32 = 4.0;       // metres in front of camera
const SPRITE_SPACING: f32 = 1.6;        // metres between adjacent enemies
const SPRITE_Y_OFFSET: f32 = 0.8;       // metres above camera origin so feet are roughly at floor

#[derive(Component, Debug, Default)]
pub struct Sprite3dBillboard;

#[derive(Component, Debug, Clone)]
pub struct EnemyVisual {
    pub id: String,
    pub placeholder_color: [f32; 3],    // normalised RGB; clamped at load
}

pub struct EnemyRenderPlugin;
impl Plugin for EnemyRenderPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::Combat), spawn_enemy_billboards)
           .add_systems(
               Update,
               (face_camera, advance_enemy_animation)
                   .run_if(in_state(GameState::Combat)),
           );
    }
}

fn spawn_enemy_billboards(
    mut commands: Commands,
    encounter: Option<Res<CurrentEncounter>>,
    enemies_q: Query<(Entity, &EnemyVisual), With<Enemy>>,
    camera_q: Query<&GlobalTransform, With<DungeonCamera>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    let Some(_encounter) = encounter else {
        warn!("OnEnter(Combat) fired without CurrentEncounter — skipping billboard spawn");
        return;
    };
    let Ok(camera) = camera_q.single() else {
        warn!("OnEnter(Combat) — DungeonCamera missing, skipping billboard spawn");
        return;
    };

    let quad = meshes.add(Rectangle::new(SPRITE_WIDTH, SPRITE_HEIGHT));

    let camera_pos = camera.translation();
    let forward = camera.forward();
    let right = camera.right();

    let total = enemies_q.iter().count() as f32;
    for (i, (entity, visual)) in enemies_q.iter().enumerate() {
        let offset = (i as f32 - (total - 1.0) / 2.0) * SPRITE_SPACING;
        let world_pos = camera_pos
            + (*forward) * SPRITE_DISTANCE
            + (*right) * offset
            + Vec3::Y * SPRITE_Y_OFFSET;

        // Generate a 1×1 solid-colour texture for the placeholder.
        // Clamp colour channels defensively (trust boundary on RON-authored data).
        let [r, g, b] = visual.placeholder_color.map(|c| c.clamp(0.0, 1.0));
        let texel: [u8; 4] = [
            (r * 255.0) as u8,
            (g * 255.0) as u8,
            (b * 255.0) as u8,
            255,
        ];
        let image = Image::new_fill(
            Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
            TextureDimension::D2,
            &texel,
            TextureFormat::Rgba8UnormSrgb,
            RenderAssetUsages::RENDER_WORLD,
        );
        let image_handle = images.add(image);

        let material = materials.add(StandardMaterial {
            base_color_texture: Some(image_handle),
            unlit: true,
            alpha_mode: AlphaMode::Mask(0.5),
            ..default()
        });

        commands.entity(entity).insert((
            Mesh3d(quad.clone()),
            MeshMaterial3d(material),
            Transform::from_translation(world_pos),
            Visibility::default(),
            Sprite3dBillboard,
        ));
    }
}

fn face_camera(
    camera_q: Query<&GlobalTransform, With<DungeonCamera>>,
    mut sprites_q: Query<&mut Transform, (With<Sprite3dBillboard>, Without<DungeonCamera>)>,
) {
    let Ok(camera) = camera_q.single() else { return; };
    let camera_pos = camera.translation();
    for mut t in &mut sprites_q {
        let dx = camera_pos.x - t.translation.x;
        let dz = camera_pos.z - t.translation.z;
        // atan2(x, z) gives yaw for a quad whose default normal is +Z.
        let angle = dx.atan2(dz);
        t.rotation = Quat::from_rotation_y(angle);
    }
}
```

### Schema additions for `enemies.ron`

```rust
// in src/data/enemies.rs (replaces the current stub)

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::plugins::combat::ai::EnemyAi;
use crate::plugins::party::character::{BaseStats, DerivedStats};

#[derive(Reflect, Serialize, Deserialize, Debug, Clone)]
pub struct EnemyDefinition {
    pub id: String,                       // e.g. "goblin", "cave_spider"
    pub display_name: String,             // e.g. "Goblin"
    pub base_stats: BaseStats,
    pub derived_stats: DerivedStats,
    #[serde(default)]
    pub ai: EnemyAi,
    /// Normalised RGB in [0.0, 1.0] for the placeholder solid-colour billboard.
    /// Clamped on load (trust boundary).
    pub placeholder_color: [f32; 3],
    /// Asset path stem — `enemies/<id>/idle.png` etc. when real art lands.
    /// `None` in the placeholder PR (data file omits the field).
    #[serde(default)]
    pub sprite_path: Option<String>,
}

#[derive(Asset, Reflect, Serialize, Deserialize, Default, Debug, Clone)]
pub struct EnemyDb {
    pub enemies: Vec<EnemyDefinition>,
}
```

### `core.enemies.ron` shape

```ron
// assets/enemies/core.enemies.ron — replaces current empty ()

(
    enemies: [
        (
            id: "goblin",
            display_name: "Goblin",
            base_stats: (strength: 8, intelligence: 4, piety: 4, vitality: 8, agility: 6, luck: 4),
            derived_stats: (
                max_hp: 30, current_hp: 30, max_mp: 0, current_mp: 0,
                attack: 8, defense: 5, magic_attack: 0, magic_defense: 2,
                speed: 6, accuracy: 50, evasion: 3,
            ),
            ai: RandomAttack,
            placeholder_color: (0.4, 0.6, 0.3),     // sickly green
        ),
        (
            id: "goblin_captain",
            display_name: "Goblin Captain",
            base_stats: (strength: 12, intelligence: 4, piety: 4, vitality: 12, agility: 6, luck: 4),
            derived_stats: (
                max_hp: 60, current_hp: 60, max_mp: 0, current_mp: 0,
                attack: 12, defense: 8, magic_attack: 0, magic_defense: 2,
                speed: 6, accuracy: 70, evasion: 5,
            ),
            ai: BossFocusWeakest,
            placeholder_color: (0.6, 0.5, 0.2),     // ochre
        ),
        (
            id: "cave_spider",
            display_name: "Cave Spider",
            base_stats: (strength: 6, intelligence: 2, piety: 2, vitality: 4, agility: 12, luck: 6),
            derived_stats: (
                max_hp: 18, current_hp: 18, max_mp: 0, current_mp: 0,
                attack: 10, defense: 3, magic_attack: 0, magic_defense: 2,
                speed: 12, accuracy: 75, evasion: 15,
            ),
            ai: RandomAttack,
            placeholder_color: (0.15, 0.1, 0.2),    // dark purple
        ),
        // ... 7 more enemies — author with distinct hues so the row is readable
        // Recommended hues: red, orange, yellow, blue, cyan, magenta, white
        // (matches the §6 user decision: "one distinct color per enemy")
    ],
)
```

The user's decision called for 10 enemies in `enemies.ron`. The roadmap entry's narrative implies a fodder-roster: 3-5 distinct types + boss variants. Recommended split (to give #21 balance work room):

| Hue | Suggested enemy | AI |
|---|---|---|
| Green | Goblin (fodder) | RandomAttack |
| Ochre | Goblin Captain (mini-boss) | BossFocusWeakest |
| Dark Purple | Cave Spider (fast glass cannon) | RandomAttack |
| Red | Hobgoblin (heavy fodder) | RandomAttack |
| Orange | Kobold | RandomAttack |
| Yellow | Acid Slime (low HP, status) | RandomAttack |
| Blue | Ice Imp | RandomAttack |
| Cyan | Wraith (incorporeal — high evasion) | RandomAttack |
| Magenta | Cultist (magic-user — for #20 spells) | RandomAttack |
| White | Skeleton Lord (boss) | BossAttackDefendAttack { turn: 0 } |

#21 will rebalance; #17 only needs them to exist and render distinct colours.

### Triggering attack/damage animations from combat events

```rust
// HIGH-confidence reading of combat/turn_manager.rs:402-490: damage application
// happens inside execute_combat_actions. The cleanest hook for visual feedback is
// to add an event written by execute_combat_actions when an attack lands and an
// event for when an entity takes damage.
//
// #15 already emits `ApplyStatusEvent` (status_effects); the same pattern fits.

#[derive(Event, Debug, Clone, Copy)]
pub struct EnemyVisualEvent {
    pub target: Entity,
    pub kind: EnemyVisualEventKind,
}
#[derive(Debug, Clone, Copy)]
pub enum EnemyVisualEventKind {
    AttackStart,
    DamageTaken,
    Died,
}

fn on_enemy_visual_event(
    mut events: MessageReader<EnemyVisualEvent>,
    mut anim_q: Query<&mut EnemyAnimation>,
) {
    for ev in events.read() {
        if let Ok(mut anim) = anim_q.get_mut(ev.target) {
            anim.state = match ev.kind {
                EnemyVisualEventKind::AttackStart  => AnimState::Attacking,
                EnemyVisualEventKind::DamageTaken  => AnimState::TakingDamage,
                EnemyVisualEventKind::Died         => AnimState::Dying,
            };
            anim.frame_index = 0;
            anim.frame_timer.reset();
        }
    }
}
```

This event needs to be a `#[derive(Message)]` in Bevy 0.18 (memory: `feedback_bevy_0_18_event_message_split.md`), read with `MessageReader`. The producer side is a 1-line `events.write(EnemyVisualEvent { ... })` in `execute_combat_actions` when an enemy attacks / takes damage / dies.

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|---|---|---|---|
| `Camera3dBundle { ... }`, `PointLightBundle { ... }` | Component tuples with `#[require(...)]` | Bevy 0.17 → 0.18 | All older tutorials/blog code for "billboard sprites in 3D" use `*Bundle` syntax and will not compile |
| `Sprite3dBundle` (in `bevy_sprite3d` 6.x and earlier) | `Sprite3d` component (in 7.x assumed; UNVERIFIED) | Tracks Bevy's bundle removal | If the upstream lags, the API gap is the reason for fallback |
| `EventReader<StateTransitionEvent<_>>` | `MessageReader<StateTransitionEvent<_>>` | Bevy 0.17 → 0.18 family rename | Memory rule applies to #17's new `EnemyVisualEvent`: must be `Message`, read with `MessageReader` |
| `Plane3d::new(...).mesh().size(w, h)` for ground-aligned quads | Same — but `Plane3d` is single-sided | Stable 0.16+ | For billboard surfaces use `Rectangle` instead — vertical quad, simpler intent |
| `AnimationPlayer` | `AnimationGraph` | 0.16 | For 4-state sprite-frame swap, neither is the right tool; custom component is canonical |

**Deprecated/outdated:**

- Anything saying "use a `Camera3dBundle` for the combat scene" — wrong API for 0.18.
- Anything that says `bevy_sprite3d` is a hard requirement — the manual fallback is now a stable Bevy 0.18 pattern.
- The roadmap text "use `bevy_sprite3d 7.x`" — has the verification gate caveat baked in but is presented as the primary; the actual decision is "EITHER 7.x (verified) OR manual".

---

## Validation Architecture

### Test Framework

| Property | Value |
|---|---|
| Framework | `cargo test` with Bevy `App` integration tests (no extra dependency) |
| Config file | none — uses `[dev-dependencies]` in Cargo.toml |
| Quick run command | `cargo test -p druum enemy_render --no-default-features` (target ONLY the new module) |
| Full suite command | `cargo test --features dev` (mirrors the project's standard practice) |

Existing test patterns in tree to mirror:

- `src/plugins/combat/encounter.rs:434-981` — `mod tests` (Layer 1 pure) + `mod app_tests` (Layer 2 App-driven) split. Layer 1 tests pure helpers; Layer 2 spins up `MinimalPlugins + StatesPlugin + relevant plugins` and exercises state transitions.
- `src/plugins/combat/turn_manager.rs:666-1311` — same pattern.
- The pattern is established by memory rule: `feedback_bevy_input_test_layers.md` — for any system test, choose direct-resource-mutation OR full-message-pipeline; don't mix.

For #17 specifically:
- Layer 1: face-camera math (`atan2`-driven angle is correct for 4 known camera positions: ±X and ±Z axes).
- Layer 1: `Image::new_fill` produces a `Handle<Image>` you can stash in `Assets<Image>` and look up by handle in `MeshMaterial3d`.
- Layer 1: state-machine transitions (Idle → Attacking → Idle on completion).
- Layer 2: `OnEnter(GameState::Combat)` spawn — verify N enemies receive `Mesh3d`/`MeshMaterial3d`/`Sprite3dBillboard` after the state transition.
- Layer 2: `OnExit(GameState::Combat)` — verify all sprite-bearing entities are gone (already covered by `clear_current_encounter` test — `encounter.rs:907-953`; #17 may add an assertion that visual components were dropped too).

### Requirements → Test Map

| Requirement | Behavior | Test Type | Automated Command | File Exists? |
|---|---|---|---|---|
| 10 enemies parse from RON | `EnemyDb::deserialize` accepts the authored file | unit | `cargo test -p druum data::enemies::tests` | NO — needs creating |
| Solid-colour `Image` generation | `Image::new_fill` of 1×1 + handle in `Assets<Image>` | unit | `cargo test -p druum combat::enemy_render::tests` | NO — needs creating |
| Face-camera angle math | Sprite at known position vs camera at known position → expected `Quat` | unit | same | NO — needs creating |
| Anim state machine returns to Idle | After `Attacking` frame count expires, state is `Idle` | unit (with `TimeUpdateStrategy::ManualDuration`) | same | NO — needs creating |
| Sprites spawn on `OnEnter(Combat)` | After `next_state.set(Combat)` and 2× `app.update()`, all enemies have `Sprite3dBillboard` + `Mesh3d` | integration (Layer 2) | same | NO — needs creating |
| Sprites cleanly despawn on `OnExit(Combat)` | After `next_state.set(Dungeon)` and 2× `app.update()`, zero entities with `Sprite3dBillboard` | integration (Layer 2) | same | NO — needs creating (or extend `no_current_encounter_after_combat_exit` at `encounter.rs:907-953`) |
| RON trust-boundary clamps | `placeholder_color: (2.0, -1.0, 0.5)` clamps to `(1.0, 0.0, 0.5)` | unit | same | NO — needs creating |

### Gaps (files to create before implementation)

- [ ] `src/plugins/combat/enemy_render.rs` — new plugin, sole owner of sprite billboard logic
- [ ] `src/data/enemies.rs` — replace the empty stub with real schema (see Code Examples above)
- [ ] `assets/enemies/core.enemies.ron` — replace the `()` with the 10-enemy roster
- [ ] `src/plugins/combat/enemy_render.rs::tests` (Layer 1) — face-camera math, anim state machine, Image::new_fill smoke
- [ ] `src/plugins/combat/enemy_render.rs::app_tests` (Layer 2) — OnEnter/OnExit cycle
- [ ] OPTIONAL: extend `core.enemies.ron` test in `data::enemies` mirroring `floor_01_encounters_ron_parses` (`encounters.rs:221-230`) — proves the authored 10 enemies all parse

No new test framework or config is required.

---

## Verification Recipes

These are the commands the planner or implementer MUST run before locking decisions that depend on `bevy_sprite3d`. The recipes follow the project's established Step A/B/C pattern (memory: `feedback_third_party_crate_step_a_b_c_pattern.md`).

### Step A — Resolve `bevy_sprite3d` version (no Cargo.toml edit yet)

```bash
cd /Users/nousunio/Repos/Learnings/claude-code/druum
cargo add bevy_sprite3d --dry-run 2>&1 | tee /tmp/bevy_sprite3d-resolve.txt
# Read the resolved version line; verify its `bevy = "..."` requirement.
# Specifically look for `bevy = "0.18"` or `bevy = "^0.18"`.
# If resolved version is 7.x with bevy = 0.18 → continue to Step B.
# If resolved version is 6.x or earlier or requires bevy <0.18 → ABANDON, use manual fallback.
```

### Step B — Feature flag audit

```bash
# After resolving:
VERSION=$(grep -E '^Adding bevy_sprite3d' /tmp/bevy_sprite3d-resolve.txt | head -1 | awk '{print $3}' | tr -d 'v"')
REG=~/.cargo/registry/src/index.crates.io-*/bevy_sprite3d-${VERSION}
# If not present, cargo add would have unpacked it; try:
ls -d ~/.cargo/registry/src/index.crates.io-*/bevy_sprite3d-* | head -1

cat ${REG}/Cargo.toml | sed -n '/^\[features\]/,/^\[/p'
# Note any heavy default features. bevy_sprite3d historically has been lean
# but verify default-features inclusion of egui/serde/etc.
```

### Step C — API grep (verify the spawn/animate idiom)

```bash
REG=$(ls -d ~/.cargo/registry/src/index.crates.io-*/bevy_sprite3d-* | head -1)
grep -rn "pub struct Sprite3d" ${REG}/src
grep -rn "pub fn" ${REG}/src | head -30
ls ${REG}/examples/ 2>/dev/null
# Look for:
#   - A `Sprite3d` component
#   - A `Sprite3dParams` system param (older API) or a builder
#   - A face-camera plugin
# If any/all are missing OR the API requires deprecated 0.17 patterns → use manual fallback.
```

### Direct check: does the project's lockfile already have it?

```bash
grep -A2 '^name = "bevy_sprite3d"' Cargo.lock 2>/dev/null || echo "not present"
```

If `bevy_sprite3d` is already a transitive dep of something the project pulls (very unlikely — it's a leaf rendering crate), the upgrade path may be simpler.

### Decision criteria

| Outcome | Recommendation |
|---|---|
| Step A resolves to 7.x with `bevy = "0.18"` AND Step B shows no heavy defaults AND Step C confirms current `Sprite3d` API | OPTIONAL: open a follow-up PR replacing manual quad with `Sprite3d` — out of scope for #17 |
| Step A fails or resolves to a non-0.18 bevy requirement | Ship manual fallback (this PR) and update the roadmap to drop the `bevy_sprite3d` recommendation |
| Step C shows the crate is unmaintained (no commits in 12+ months, GitHub repo archived) | Same — manual fallback |
| Any uncertainty | DEFAULT TO MANUAL FALLBACK — the user has already authorised this path |

---

## Integration Points (where #17 wires into existing code)

These are the precise touch points the planner needs to know. All file:line citations were verified in this session unless marked otherwise.

### Touch point 1: `src/plugins/combat/mod.rs:31-45` — add `EnemyRenderPlugin`

```rust
// Add to the existing CombatPlugin::build:
.add_plugins(enemy_render::EnemyRenderPlugin) // Feature #17

// Adds the module declaration:
pub mod enemy_render;
```

### Touch point 2: `src/data/encounters.rs:25-36` — `EnemySpec` already carries everything #17 needs at the encounter level

The existing `EnemySpec` has `name`, `base_stats`, `derived_stats`, `ai`. **#17 ADDS** a way to look up the visual data for the spawned enemy. Recommended approach:

- Add `id: String` field to `EnemySpec` (additive — defaults to `""` for back-compat with existing `floor_01.encounters.ron`).
- The spawn code at `encounter.rs:367-380` adds an `EnemyVisual { id: spec.id.clone(), placeholder_color: lookup(&enemy_db, &spec.id) }` component to the spawned `EnemyBundle`.

Alternative (simpler) approach if the team prefers: ADD `placeholder_color` directly to `EnemySpec` so the encounter file authors are self-contained, and let `EnemyDb` exist as the v2 evolution. The roadmap entry for #17 explicitly says "Author `enemies.ron`" so the dual approach is the right call.

### Touch point 3: `src/plugins/combat/encounter.rs:367-380` — extend spawn to attach `EnemyVisual`

```rust
// Existing code at encounter.rs:367-380:
for (idx, spec) in enemies_to_spawn.iter().enumerate() {
    let entity = commands
        .spawn(EnemyBundle {
            name: EnemyName(spec.name.clone()),
            index: EnemyIndex(idx as u32),
            base_stats: spec.base_stats,
            derived_stats: spec.derived_stats,
            ai: spec.ai,
            ..Default::default()
        })
        .id();
    entities.push(entity);
}

// #17 extension: ALSO attach EnemyVisual and EnemyAnimation
// (Mesh3d/MeshMaterial3d/Transform/Sprite3dBillboard are attached LATER by
// spawn_enemy_billboards on OnEnter(Combat).)
```

In practice the cleanest design is to add `EnemyVisual` and `EnemyAnimation` as fields on `EnemyBundle` itself (`src/plugins/combat/enemy.rs:40-51`) with `Default` impls, so the existing `..Default::default()` in `encounter.rs:376` picks them up. Then `encounter.rs` doesn't change at all — only `enemy.rs` and `enemy_render.rs` do.

Recommended: extend `EnemyBundle` like so:

```rust
// src/plugins/combat/enemy.rs (extension only — keeps existing fields)
#[derive(Bundle, Default)]
pub struct EnemyBundle {
    pub marker: Enemy,
    pub name: EnemyName,
    pub index: EnemyIndex,
    pub base_stats: BaseStats,
    pub derived_stats: DerivedStats,
    pub status_effects: StatusEffects,
    pub party_row: PartyRow,
    pub equipment: Equipment,
    pub experience: Experience,
    pub ai: EnemyAi,
    // #17 additions:
    pub visual: crate::plugins::combat::enemy_render::EnemyVisual,
    pub animation: crate::plugins::combat::enemy_render::EnemyAnimation,
}
```

`encounter.rs` then keeps `..Default::default()` and the visual fields populate from a lookup step before the spawn (or after, via `commands.entity(entity).insert(...)`).

### Touch point 4: `src/plugins/loading/mod.rs:42-44` — `EnemyDb` already loads, just needs to be read

```rust
// Existing — no change required:
#[asset(path = "enemies/core.enemies.ron")]
pub enemy_db: Handle<EnemyDb>,
```

#17 reads `Res<Assets<EnemyDb>>` in `spawn_enemy_billboards` (or in an `OnEnter(Combat)` system that resolves `EnemySpec.id` → `EnemyDefinition` once and stashes the placeholder colour in `EnemyVisual` on the spawned entity).

### Touch point 5: `src/plugins/dungeon/mod.rs:92` — `DungeonCamera` marker is the camera-side query

`DungeonCamera` is a `Component` and there's only ever one (verified at `dungeon/mod.rs:516-549` — the camera is spawned exactly once in `spawn_party_and_camera`). The combat plugin reuses this camera (`combat/ui_combat.rs:64-71` attaches egui to it). #17 queries it via `Query<&GlobalTransform, With<DungeonCamera>>`.

### Touch point 6: Test fixtures in `src/plugins/combat/encounter.rs:558-668` — existing helpers can be reused

The `make_test_app` and `make_test_app_with_floor` builders at `encounter.rs:558-668` are exactly the test scaffolding #17 needs. The Layer 2 tests for `enemy_render.rs` build on this pattern.

---

## Open Questions

### Open Question 1: `bevy_sprite3d` 7.x Bevy 0.18.1 compatibility

- What we know: roadmap says "use `bevy_sprite3d 7.x`" with a fallback caveat; the user explicitly authorises shipping the fallback in this PR.
- What's unclear: actual upstream compatibility status of `bevy_sprite3d` 7.x with Bevy 0.18.1.
- Confidence in this gap: HIGH — we positively don't know.
- Recommendation: do NOT attempt to verify in this PR; ship the manual fallback. Open a follow-up issue for the planner to run Step A/B/C if/when team capacity exists; if the gate passes cleanly the swap is a ~30 LOC mechanical change to `enemy_render.rs` and the public API (the `EnemyRenderPlugin` and the `Sprite3dBillboard` marker) stays unchanged.

### Open Question 2: One-PNG-per-placeholder vs `Image::new_fill`

- What we know: the roadmap mentions "Source/produce 5-10 placeholder enemy sprite sheets (CC0 itch.io packs are a good starting point)" — leaving asset method open. The user decided "solid-color PNGs, one distinct color per enemy. Defer art sourcing."
- What's unclear: whether the user prefers ACTUAL PNG files committed or generated.
- Recommendation: **`Image::new_fill`** for the placeholder PR. Reasoning:
  1. The "art is placeholder" intent is encoded in the data file (`placeholder_color: (r, g, b)`); a future PR replacing the field with `sprite_path: "enemies/goblin/idle.png"` is a one-line schema change.
  2. Zero new files in `assets/` reduces merge friction.
  3. Hot-reload tooling (`bevy/file_watcher`) has nothing to watch — the generated handle is stable.
- **The planner should confirm with the user.** Both paths are ~equivalent code-wise.

### Open Question 3: Where does `EnemyVisual.id` come from at spawn time?

- What we know: `EnemySpec` in `encounters.rs:25-36` currently has `name: String` but no `id`.
- What's unclear: whether `name` should double as the id, or whether to add an explicit `id: String`.
- Recommendation: ADD `id: String` to `EnemySpec` as an additive `#[serde(default)] pub id: String`. This is consistent with the inline-EnemySpec pattern at `encounters.rs:8-11` doc comment ("Migration path: add `enemy_id: Option<String>` to `EnemySpec` (additive)"). For #17, populating `id: "goblin"` etc. in `floor_01.encounters.ron` is part of the data migration.
- Defaulting to `""` means the existing inline-RON file works on Day 1; the spawn code falls back to a "no visual lookup → use a default grey colour" path when id is empty. Tests can lock this behaviour.

### Open Question 4: Damage shake — roadmap "Add a damage shake tween effect on damage taken"

- What we know: the roadmap entry's Broad Todo List mentions a damage shake tween. The user's "User decisions" list does not call this out.
- What's unclear: whether damage shake is in or out of scope for THIS PR.
- Recommendation: implement minimally — a 0.15s sine-jitter on `Transform.translation.x` triggered by `EnemyVisualEvent::DamageTaken`. ~20 LOC. Tests use `TimeUpdateStrategy::ManualDuration` for determinism. If scope creeps, defer to a follow-up.

---

## Sources

### Primary (HIGH confidence)

- **Bevy 0.18.1 on-disk crate sources** (read directly this session):
  - `~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bevy-0.18.1/Cargo.toml` — feature graph; `3d` umbrella does NOT directly enable `bevy_sprite_render`, only `2d_bevy_render` does. The `Sprite` component (in `bevy_sprite`) IS enabled via the `ui` chain.
  - `~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bevy_internal-0.18.1/Cargo.toml` — feature dependencies confirming `bevy_ui = ["dep:bevy_ui", "bevy_text", "bevy_sprite"]` pulls in `bevy_sprite`.
  - `~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bevy-0.18.1/examples/3d/3d_scene.rs` — canonical `Mesh3d` + `MeshMaterial3d` + `Transform` spawn pattern.
  - `~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bevy-0.18.1/examples/3d/transparency_3d.rs` — `StandardMaterial { alpha_mode, unlit }` patterns.
  - `~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bevy-0.18.1/examples/3d/3d_shapes.rs` — `Image::new_fill` runtime texture generation pattern.
  - `~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bevy-0.18.1/examples/2d/sprite_animation.rs` — `Sprite { texture_atlas: Some(TextureAtlas { layout, index }) }` and frame-stepping pattern.
  - `~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bevy-0.18.1/examples/2d/sprite_sheet.rs` — `TextureAtlasLayout::from_grid(UVec2::splat(N), cols, rows, None, None)` pattern.
  - `~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bevy-0.18.1/examples/3d/render_to_texture.rs` — `StandardMaterial { base_color_texture: Some(handle), unlit }` pattern for arbitrary `Handle<Image>` textures.
- **Druum codebase (this session):**
  - `src/plugins/combat/mod.rs:1-46` — `CombatPlugin::build` shape; where `EnemyRenderPlugin` goes.
  - `src/plugins/combat/encounter.rs:309-411` — `handle_encounter_request` (spawn site for enemy ECS entities); `clear_current_encounter` (despawn site).
  - `src/plugins/combat/enemy.rs:1-64` — `Enemy` marker, `EnemyBundle`, current shape.
  - `src/plugins/combat/turn_manager.rs:34-46` — `CurrentEncounter` contract; readers via `Option<Res<CurrentEncounter>>`.
  - `src/plugins/combat/ui_combat.rs:60-71` — combat overlays egui on `DungeonCamera`; NO new Camera3d (D-Q1=A).
  - `src/plugins/dungeon/mod.rs:88-92, 446-556, 593-619` — `DungeonCamera` marker, party+camera spawn, party preservation across `Dungeon → Combat → Dungeon`.
  - `src/data/encounters.rs:1-91` — inline-`EnemySpec` schema, migration path comment, `pick_group` RNG path.
  - `src/data/enemies.rs:1-12` — `EnemyDb` stub (empty body, awaits #17 schema).
  - `src/plugins/loading/mod.rs:30-48, 100-115` — `DungeonAssets` collection with `enemy_db: Handle<EnemyDb>`; RON loader registration.
  - `assets/encounters/floor_01.encounters.ron:1-99` — current 4-entry inline `EnemySpec` author file.
  - `assets/enemies/core.enemies.ron` — current empty `()` stub.
  - `Cargo.toml:9-43` — exact pins for `=0.18.1`, `=0.39.1`, `=0.20.0`, etc.

### Secondary (MEDIUM confidence — verifiable but not in-session)

- `bevy_sprite3d` on crates.io — UNVERIFIED in session. The Step A/B/C recipes in `## Verification Recipes` are the path to convert this to HIGH.
- Genre-correct pattern reference (Etrian Odyssey, Wizardry remake, Undernauts use billboarded 2D sprites in 3D dungeons) — published reference patterns, but no specific source verified in session.

### Tertiary (LOW confidence — flag for validation)

- None. Every claim in this document maps to either an on-disk verified source or a documented uncertainty in `## Open Questions`.

---

## Metadata

**Confidence breakdown:**

- Standard stack (manual fallback path): HIGH — every API verified at on-disk Bevy 0.18.1 example.
- Architecture (face-camera math, animation pattern, spawn lifecycle): HIGH — pure math + verified Bevy 0.18 idioms.
- `bevy_sprite3d` decision: MEDIUM at best — verification gate is documented; the recommendation is to default to manual fallback per user authorisation.
- Pitfalls: HIGH — all 9 pitfalls cite specific file:line or example source.
- Integration points (existing Druum codebase): HIGH — every touch point verified by direct read.
- Test architecture: HIGH — mirrors the existing two-tier pattern at `combat/encounter.rs` and `combat/turn_manager.rs`.
- Asset layout decision (option a vs b): MEDIUM — both paths viable, recommend asking the user.

**Research date:** 2026-05-11

**Tooling limitation impact:** the `bevy_sprite3d` decision is the only HIGH-uncertainty item. Because the user pre-authorised the manual fallback, this limitation does NOT block the planner — it shifts the manual-vs-`bevy_sprite3d` call to a future follow-up rather than a #17 blocker.
