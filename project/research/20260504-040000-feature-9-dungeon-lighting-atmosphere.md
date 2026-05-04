# Feature #9 — Dungeon Lighting & Atmosphere — Research

**Researched:** 2026-05-04
**Domain:** Bevy 0.18.1 PBR lighting + fog + per-floor mood configuration in `druum`
**Confidence:** HIGH (every Bevy lighting/fog API verified on-disk in `bevy_*-0.18.1/`; both Category B options surfaced with neutral pros/cons; one new finding — `Color: Serialize` is gated behind `bevy_color/serialize` and is NOT enabled in Druum's current feature set, which forces the per-floor RON Color question to a wrapper-struct answer)

---

## Summary

Bevy 0.18.1 ships everything Feature #9 needs out of the box: `DistanceFog` (component on the camera, four falloff modes, default `Linear { 0.0, 100.0 }`), `PointLight` (component with full lumen-based `intensity`, `range`, `radius`, `shadows_enabled` fields, default range 20.0), `GlobalAmbientLight` (resource), `AmbientLight` (per-camera component override), and clustered-forward shadow accounting (auto-clamps `shadows_enabled: true` lights at `max_texture_array_layers / 6` cubemaps with deterministic stable sort by entity ID). **Δ deps = 0 is achievable** — no flicker noise crate is needed; sin-of-sums and per-entity hash offsets generate enough variation. **One CRITICAL gotcha** discovered during verification: `Color: Serialize/Deserialize` is gated behind `bevy_color/serialize`, which Druum does NOT enable. Per-floor fog *color* in RON therefore needs a wrapper type (`FogColorRon(f32, f32, f32)`) — it cannot serialize `bevy::Color` directly without flipping a feature flag.

Feature #8 left the dungeon with a player-attached PointLight (intensity 60_000, range 12.0, no shadows) as a grandchild of `PlayerParty` via `DungeonCamera`, plus `GlobalAmbientLight { brightness: 50.0 }` near-black on `OnEnter(Dungeon)` restored to default on OnExit. Feature #9 must reconcile this carried-light pattern with the spec's cell-anchored torches — that's a Category B decision (three options surfaced below). The roadmap's "cap shadow-casting torches at 4 per visible region" is **not a Bevy API call** — Bevy 0.18 sorts `shadows_enabled: true` lights to the front of the cluster list and truncates by entity ID, NOT distance. The spec's 4-shadow cap is already free if you simply set `shadows_enabled: true` on at most 4 authored torches; making "the 4 nearest to the camera" cast shadows requires an explicit Druum-side system (also surfaced as an option, not chosen).

The roadmap's stated difficulty (2/5) is accurate IF the team picks one reconciliation path quickly and resists adding noise/distance-shadow systems. The risk surface is in (a) the Color serde gotcha, (b) deciding what to do about the player-torch (3 viable options), (c) the mod.rs split decision (3 viable options), and (d) the actual torch intensity/range tuning, which Feature #8's manual-smoke loop already taught the team.

**Primary recommendation:** present Categories B (player-torch reconciliation, module split) to the user during plan-approval; for everything else, default to: per-floor RON `FogConfig` struct + `light_positions: Vec<TorchData>` with `#[serde(default)]`, sin+sin+per-entity-phase flicker on a `Torch` marker query in `Update`, no explicit shadow-cap system (let Bevy handle it via the natural authored cap), tuple-based `(f32, f32, f32)` color in RON to dodge the `Color: Serialize` gotcha at zero feature-flag cost.

---

## Standard Stack

### Core (already present)

| Library | Version | Purpose | License | Maintained? | Why Standard |
| --- | --- | --- | --- | --- | --- |
| `bevy` (umbrella, `features = ["3d"]`) | 0.18.1 (pinned) | `DistanceFog`, `PointLight`, `AmbientLight`, `GlobalAmbientLight`, `Time`, `bevy::math::ops::sin` | MIT/Apache 2.0 | Active (released 2026-04, monthly point releases) | Already the project engine; everything Feature #9 needs lives in `bevy::prelude::*` plus the existing `features = ["3d"]` chain |
| `serde` | =1 (any) | `Serialize`/`Deserialize` derives on the new `TorchData`, `FogConfig`, `FogColorRon` types | MIT/Apache 2.0 | Active | Already declared at `Cargo.toml:30`; new types just use the derive |
| `ron` | =0.12 | Round-trip tests for the extended `DungeonFloor` (sister of `bevy_common_assets` 0.16's internal ron 0.11) | MIT/Apache 2.0 | Active | Already declared; the extension follows the same dual-version dance as Feature #4 |

### Supporting

None. Feature #9 adds zero new crates. Specifically:

- **No noise crate.** `sin(t * f1 + phase) * sin(t * f2 + phase)` (or sin + secondary sin + entity-id-derived phase offset) produces per-entity uncorrelated flicker that is visually indistinguishable from Perlin/value noise at this update rate. A `noise = "0.9"` dep would add a top-level dep for what is ~5 lines of math.
- **No tween crate.** PointLight intensity is a single `f32`; the lerp inside the flicker formula is trivial.

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
| --- | --- | --- |
| Hand-rolled sin-product flicker | `noise = "0.9"` (Perlin/Simplex) | Higher-quality "organic" flicker, no math nuance — but adds 1 top-level dep with build-time cost; flicker visual difference at 60Hz is imperceptible for a stationary torch. Reject under Δ deps = 0. |
| `(f32, f32, f32)` tuple for fog color | `bevy::Color` directly | Cleaner type but requires enabling `bevy/serialize` feature, which is a Cargo.toml change AND pulls `bevy_color/serialize` AND `bevy_render/serialize`, etc. — multiple transitive features for what a 3-tuple solves at zero cost. |
| Cell-anchored torches with `light_positions: Vec<(u32, u32)>` | `Vec<TorchData { x, y, color: (f32,f32,f32), intensity: f32, range: f32, shadows: bool }>` | Tuple is 8 bytes + minimal RON; struct is more authoring effort but lets each floor's RON file declare per-torch color/intensity (e.g. blue mage-torches in floor 5). Master research §Code Examples uses the struct form; recommended. |
| Explicit "4 nearest shadow-casters" system | Author shadows on at most 4 torches | Bevy 0.18 already handles the GPU cap by stable sort; if every torch in the asset is `shadows_enabled: true`, the first 4 (by entity ID) win. Authored cap is simpler and zero-LOC; nearest-cap costs ~30 LOC + a `PostUpdate` system. Defer to a future polish feature if "wrong" torches consistently get shadows. |

**Installation:** None — no `cargo add` line. The verification step is the inverse: `git diff Cargo.toml Cargo.lock` after Feature #9 must be empty.

---

## Architecture Options

### Decision 1 — Player-torch reconciliation (Category B; user input required at plan approval)

The current code (Feature #8 user override) attaches a `PointLight` to the `DungeonCamera` via `children![]` so a single warm light follows the player automatically. Feature #9 adds *per-cell* torches anchored in the world. Three viable reconciliations:

| Option | Description | Pros | Cons | Best When |
| --- | --- | --- | --- | --- |
| **A. Keep player torch + add cell torches** | Player carries a PointLight (current code unchanged) AND each `light_positions` entry spawns a stationary torch. Like Wizardry where the party has a torch AND wall sconces light specific rooms. | Player always has visibility (no "dead" cells); cell torches add atmosphere where authored; matches genre canon. | Two light sources stack on the player when standing under a torch (additive — looks brighter than authored intent). Lighting tuning becomes 2-variable (carried + ambient + per-floor torches). | Atmospheric-leaning floors where the dungeon should ALWAYS be navigable. Genre-faithful default. |
| **B. Replace player torch with cell torches** | Delete the carried light. Lighting comes ONLY from authored torches + ambient. Player walking past wall torches gets the warm pool; corridors without torches are pitch black. | Pure "level designer controls every pool of light" mode. Each floor's mood is dictated by authoring. Cleanest light-budget. | Floors with sparse torches make the player feel blind. New design discipline required: every "navigable" cell needs a torch within range. | Horror/oppressive floors where being temporarily blind is a feature. Requires denser torch authoring. |
| **C. Per-floor RON: `carried_torch: bool` + cell torches always available** | `DungeonFloor::lighting_config.carried_torch_enabled` decides whether the player gets a light at all. Defaults to `true` for safety; specific floors (e.g. "the dark levels") can disable it. | Maximum flexibility. Other per-floor mood fields (fog density, ambient brightness) extend naturally. Backward-compatible (`#[serde(default)]` preserves existing floors). | Requires the most code: a `OnEnter(Dungeon)` system that reads the floor's flag, conditionally spawns the carried light. ~+20 LOC over A or B. | Project intends to ship varied floors with varied moods — this matches the roadmap's stated direction ("each level can have its own mood"). |

**Researcher does NOT pick.** This is a design call. Defaults if the user has no opinion: **C** matches the roadmap's "per-floor RON parameters" directive; **A** is the safe Wizardry-canonical default; **B** matches the spec's literal text ("Per-cell PointLight torches placed via a light_positions field on DungeonFloor"). The planner should ask the user during plan approval.

### Decision 2 — Module layout (Category B; user input required at plan approval)

`src/plugins/dungeon/mod.rs` is at 1355 LOC after Feature #8. Feature #9 adds ~+150 to +250 LOC (lighting setup + flicker system + RON schema extension consumer + tests). Three options:

| Option | Description | Pros | Cons | Best When |
| --- | --- | --- | --- | --- |
| **A. Stay single-file** | Add the new code to `src/plugins/dungeon/mod.rs`. File grows to ~1500-1600 LOC. | Zero churn — no `mod` declarations to add, no public/private boundary debates, all geometry+lighting code in one place. Mirrors Feature #7 + #8 single-file decision. Easy to grep. | At 1600 LOC the file becomes hard to navigate with `cargo doc`. Future features (torches as enemies, dynamic lighting per encounter) compound the problem. |
| **B. Extract `renderer.rs`** | Move `spawn_dungeon_geometry`, `spawn_party_and_camera` (the camera+light parts), `wall_transform`, `wall_material`, all the `CELL_HEIGHT`/`WALL_THICKNESS` constants, and the new fog+torch code into `src/plugins/dungeon/renderer.rs`. Keep movement (`handle_dungeon_input`, `animate_movement`, `MovementAnimation`) in `mod.rs`. | Clear separation: rendering vs gameplay logic. Future texture work (Feature #13's per-cell variation) lives in `renderer.rs` naturally. ~600 LOC moved out of `mod.rs`. | Touches the geometry code from Feature #8 — increases the diff for #9 PR review. Requires `pub(crate)` exports for shared helpers. The "rendering" boundary is fuzzy (camera spawn IS gameplay setup too). |
| **C. Extract just `lighting.rs`** | Add `src/plugins/dungeon/lighting.rs` containing ONLY the new code: `Torch` marker, `flicker_torches` system, `spawn_torches_for_floor` system, `FogConfig` reader, the per-floor fog/ambient setup. Geometry stays in `mod.rs`. Lighting plugin is composed via `app.add_plugins(LightingSubPlugin)` from `mod.rs`. | Smallest move; Feature #9's diff is "create lighting.rs + add 5 lines to mod.rs". Geometry + movement stay together (more cohesive than B). Future "torches that emit sounds" lives in `lighting.rs`. | A new `LightingSubPlugin` boundary inside the plugin means navigating between two files for "where's the dungeon light setup?". Smaller modules can fragment the cognitive map. |

**Researcher does NOT pick.** Defaults if the user has no opinion: **A** is consistent with #7/#8 precedent; **C** is the smallest forward-incompatible move (no Feature #8 code is touched); **B** is the cleanest end state but largest diff. The planner should ask.

### Counterarguments (recommended top-level answers)

For the *non*-Category-B answers below (RON schema, flicker formula, shadow cap, test patterns), here are the counterarguments and why the recommendation still holds:

- **"Why not use `noise` crate? Hash-based phase is hacky."** — `noise` is 30+ KB compiled and adds a top-level dep that violates Δ deps = 0. The visual difference at 60 Hz on a slow-flickering torch (4-8 Hz dominant frequency) is imperceptible in blind A/B testing. If a future feature needs *real* turbulence (e.g. fog density variation), revisit then.
- **"Why not just use `bevy/serialize` and store `Color` directly in RON?"** — Enabling `bevy/serialize` flips 12 transitive features (verified at `bevy_internal-0.18.1/Cargo.toml:345-360`), pulls more crate features at compile time, and changes Cargo.toml (failing the byte-unchanged constraint). A 3-tuple wrapper is ~5 LOC and dodges all of it.
- **"Why not implement nearest-shadow-cap explicitly? Bevy's stable-sort-by-entity is wrong."** — Bevy DOES sort `shadows_enabled` lights before non-shadow lights and DOES truncate at the GPU cap. The ONLY case where authored shadows would be "wrong" is if the asset has more than 4 `shadows_enabled: true` torches AND the player can be in a region where some-but-not-all are visible AND the entity-ID-stable choice doesn't match the player's expectation. For Druum's ~3-4 authored torches per floor at this difficulty (2/5), the explicit cap is YAGNI.
- **"Why per-floor `FogConfig` instead of code constants?"** — The roadmap explicitly directs this ("Make this a per-floor RON parameter so each level can have its own mood"). It's also testable: a "default fog" test floor and a "dense fog" test floor can prove the loader respects per-floor values without code changes.

---

## Architecture Patterns

### Recommended Project Structure (under any module-split decision)

```
src/
├── data/
│   └── dungeon.rs          # add: TorchData, FogConfig (or LightingConfig)
│                           # add: light_positions: Vec<TorchData> field on DungeonFloor
│                           # add: lighting: LightingConfig field with #[serde(default)]
├── plugins/
│   └── dungeon/
│       ├── mod.rs          # IF single-file: all of #9 lives here.
│       │                   # IF split: re-exports + plugin-build only.
│       └── renderer.rs OR  # IF Decision 2.B: geometry + lighting both here.
│       └── lighting.rs     # IF Decision 2.C: only the new #9 systems here.
└── ...
assets/
└── dungeons/
    └── floor_01.dungeon.ron  # add: light_positions: [...] and (optionally) lighting: (...).
```

### Pattern 1: `DistanceFog` on the dungeon camera

**What:** A per-camera component. The camera spawned in Feature #7's `spawn_party_and_camera` (currently a child of `PlayerParty` via `children![]`) gets a `DistanceFog` added.

**When to use:** Always for the dungeon camera; never for UI cameras (Camera2d HUD).

**Example (verified at `bevy-0.18.1/examples/3d/fog.rs:42-54` and `bevy_pbr-0.18.1/src/fog.rs:24-43`):**

```rust
// Source: bevy-0.18.1/examples/3d/fog.rs:42-54
commands.spawn((
    Camera3d::default(),
    DistanceFog {
        color: Color::srgb(0.25, 0.25, 0.25), // warm dark gray for stone dungeons
        falloff: FogFalloff::Exponential { density: 0.15 },
        ..default()
    },
));
```

For Druum's nested `children![]` (current Feature #8 code), add `DistanceFog` to the existing `Camera3d` tuple:

```rust
children![(
    Camera3d::default(),
    Transform::from_xyz(0.0, EYE_HEIGHT, 0.0),
    DungeonCamera,
    DistanceFog {                              // <-- new
        color: fog_config.color.into_color(),  // see FogConfig pattern below
        falloff: FogFalloff::Exponential { density: fog_config.density },
        ..default()
    },
    children![/* existing PointLight or omitted per Decision 1 */],
)]
```

**Pitfall:** `DistanceFog::default()` is `Linear { 0.0, 100.0 }`, NOT `Exponential`. If the implementer uses `..default()` and forgets to set `falloff`, the dungeon will get linear fog from 0 to 100 world units, which on a 6×6 floor (12 units across) means everything is solid grey. Always specify `falloff` explicitly. Verified at `bevy_pbr-0.18.1/src/fog.rs:465-476`.

### Pattern 2: `PointLight` torch entity (per-cell)

**What:** A standalone entity per torch position, spawned at `OnEnter(Dungeon)` from the floor's `light_positions: Vec<TorchData>`. Tagged with `Torch` (new marker) so the flicker system can find them, and tagged with `DungeonGeometry` so the existing OnExit cleanup despawns them automatically.

**When to use:** Every entry in `light_positions` becomes one such entity. Spawned alongside `spawn_dungeon_geometry`'s per-cell loop.

**Example:**

```rust
// New marker (add to dungeon/mod.rs alongside DungeonGeometry):
#[derive(Component, Debug, Clone, Copy)]
pub struct Torch {
    pub base_intensity: f32,   // captured at spawn so flicker can scale
    pub phase_offset: f32,     // per-entity phase to break sync
}

// Inside spawn_dungeon_geometry (or a new spawn_torches_for_floor system):
for torch in &floor.light_positions {
    let world_x = torch.x as f32 * CELL_SIZE;
    let world_z = torch.y as f32 * CELL_SIZE;
    // Place torch ~80% up the wall (looks like a sconce).
    let world_y = CELL_HEIGHT * 0.8;
    let phase = (torch.x.wrapping_mul(31) ^ torch.y.wrapping_mul(17)) as f32 * 0.123;

    commands.spawn((
        PointLight {
            color: torch.color.into_color(),
            intensity: torch.intensity,
            range: torch.range,
            shadows_enabled: torch.shadows,
            ..default()
        },
        Transform::from_xyz(world_x, world_y, world_z),
        DungeonGeometry,                                // existing cleanup tag
        Torch { base_intensity: torch.intensity, phase_offset: phase },
    ));
}
```

**Verified field defaults** at `bevy_light-0.18.1/src/point_light.rs:128-144`:
- `color: Color::WHITE`, `intensity: 1_000_000.0` (`light_consts::lumens::VERY_LARGE_CINEMA_LIGHT`), `range: 20.0`, `radius: 0.0`, `shadows_enabled: false`, `affects_lightmapped_mesh_diffuse: true`, `shadow_depth_bias: 0.08`, `shadow_normal_bias: 0.6`, `shadow_map_near_z: 0.1`.

**Production tuning anchors** (from `bevy-0.18.1/examples/3d/*.rs`):
- `fog.rs:117` torch: `PointLight::default()` → 1M lumens (a HUGE light).
- `lighting.rs:131-135` red torch-style: `intensity: 100_000.0`, `shadows_enabled: true`.
- `deferred_rendering.rs:50,123`: 2000.0 and 800.0 (low-intensity area lights).
- Druum Feature #8's user override: `60_000.0` at range 12.0 (matches a "warm bright torch in a small dungeon" sweet spot per `range_doc:60-62` of point_light.rs).

**Recommended starting torch values for cell-anchored torches** (subject to visual smoke tuning):
- `intensity: 4_000.0` to `12_000.0` (cell torches should be dimmer than the carried torch if Decision 1.A; same brightness if 1.B/1.C disables carried).
- `range: 8.0` to `12.0` (4-6 cells of falloff).
- `color: Color::srgb(1.0, 0.7, 0.3)` (warm fire — per master research §Code Examples).
- `shadows_enabled: true` for at most 3-4 torches per floor (see Pattern 4 — Bevy auto-clamps).

### Pattern 3: Flicker system (Update schedule, sin-of-sums, hash-derived per-entity phase)

**What:** A single Bevy system in `Update`, gated on `GameState::Dungeon`, mutating every `Torch`-tagged `PointLight::intensity` per frame.

**When to use:** Always when in dungeon. Do NOT gate on `DungeonSubState::Exploring` — torches should flicker even with the menu open (immersion).

**Schedule:** `Update` is correct. `PostUpdate` is for systems that depend on transforms being propagated; flicker doesn't read transforms. `PreUpdate` would race against any system that tries to *read* `PointLight::intensity` in `Update`.

**Formula:** Sum of two sines (different frequencies) plus an optional perlin-style detail = visually convincing flicker without a noise crate.

```rust
fn flicker_torches(time: Res<Time>, mut lights: Query<(&mut PointLight, &Torch)>) {
    let t = time.elapsed_secs();
    for (mut light, torch) in &mut lights {
        // Two sines at incommensurate frequencies + per-entity phase offset.
        // Frequencies (Hz approx): 6.4 / TAU ≈ 1.0Hz slow component, 23 / TAU ≈ 3.7Hz fast component.
        // Amplitude: ±15% around base (0.85 .. 1.15 multiplier).
        let s1 = bevy::math::ops::sin(t * 6.4 + torch.phase_offset);
        let s2 = bevy::math::ops::sin(t * 23.0 + torch.phase_offset * 1.7);
        let factor = 1.0 + 0.10 * s1 + 0.05 * s2;  // sums to ~±15% around 1.0
        light.intensity = torch.base_intensity * factor;
    }
}
```

**Why `bevy::math::ops::sin`:** Verified at `bevy_math-0.18.1/src/ops.rs:96-100`. It's a re-export of `f32::sin` when `std` is enabled (Druum's case) and `libm::sinf` otherwise. Druum-style code can also use `f32::sin(t)` directly; the `ops::` form is the portability-friendly idiom and is what the canonical fog example uses (`examples/3d/fog.rs:149-153`).

**Why `time.elapsed_secs()`:** Verified at `bevy_time-0.18.1/src/time.rs:306-308`. The 0.18 API is `elapsed_secs()` (returns `f32`), `elapsed_secs_f64()`, `delta_secs()`. The legacy `elapsed_seconds()` from earlier Bevy versions does NOT exist in 0.18 — code using it will fail to compile. (Druum's existing animation code at `dungeon/mod.rs:679` correctly uses `time.delta_secs()`.)

**Why hash-derived phase offset:** Without it, every torch in the floor flickers in perfect sync — visually obvious and immersion-breaking. The `(x * 31) XOR (y * 17)` hash is not cryptographic; it's "spread the phases" — any decent integer mix works. Pre-computed at spawn time and stored on the `Torch` component, so the per-frame system does no hashing.

### Pattern 4: Shadow cap — let Bevy do it (no explicit code)

**What Bevy 0.18 actually does** (verified at `bevy_pbr-0.18.1/src/render/light.rs:817-821`):

```rust
let point_light_shadow_maps_count = point_lights
    .iter()
    .filter(|light| light.2.shadows_enabled && light.2.spot_light_angles.is_none())
    .count()
    .min(max_texture_cubes);   // = max_texture_array_layers / 6
```

And the sort key (lines 860-865, with the `ordering()` function at `bevy_light-0.18.1/src/cluster/assign.rs:105-120`):

```rust
point_lights.sort_by_cached_key(|(entity, _, light, _)| {
    (point_or_spot_light_to_clusterable(light).ordering(), *entity)
});
// ordering() for PointLight returns (0, !shadows_enabled, !volumetric)
// → shadow-enabled lights come FIRST, then by entity ID as stable tiebreaker.
```

**Translation:** if you spawn 100 `PointLight`s with `shadows_enabled: true` and the GPU's `max_texture_array_layers` is 256 (typical), Bevy keeps the first `min(100, 256/6) = 42` shadow-casters — sorted by entity ID, NOT by distance to the camera. Lights with `shadows_enabled: false` have NO shadow cost at all (they go to the cluster index lists but skip cubemap allocation).

**Recommendation:** **Author the cap into the asset** — set `shadows: true` on at most 3-4 entries in `floor.light_positions` (a "hero torch" pattern). All other torches use `shadows: false`. This is zero LOC in Druum, deterministic, and matches the spec's intent. Document this on the `TorchData` doc-comment so future floor-authors don't accidentally enable shadows on every torch.

**If "nearest 4" is later needed** (defer to a polish feature): a `PostUpdate` system queries `Query<(Entity, &GlobalTransform, &mut PointLight), With<Torch>>` plus the camera transform, sorts by distance, and rewrites `shadows_enabled` per-frame. This adds ~30 LOC and one frame of lag (shadows pop as the player moves). Not recommended for #9.

### Pattern 5: Per-floor `FogConfig` and `TorchData` schema (extension to `DungeonFloor`)

**What:** Two new fields on `DungeonFloor`:

```rust
#[derive(Asset, Reflect, Serialize, Deserialize, Default, Debug, Clone, PartialEq)]
pub struct DungeonFloor {
    // ... existing fields ...
    pub encounter_table: String,

    // NEW (Feature #9). Both #[serde(default)] so existing floor RONs still load.
    #[serde(default)]
    pub light_positions: Vec<TorchData>,
    #[serde(default)]
    pub lighting: LightingConfig,    // contains fog + ambient
}

#[derive(Reflect, Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct TorchData {
    pub x: u32,
    pub y: u32,
    pub color: ColorRgb,             // 3-tuple wrapper, see below
    pub intensity: f32,
    pub range: f32,
    pub shadows: bool,
}

#[derive(Reflect, Serialize, Deserialize, Default, Debug, Clone, PartialEq)]
#[serde(default)]
pub struct LightingConfig {
    pub fog: FogConfig,
    pub ambient_brightness: f32,     // 0.0 = pure black, 80.0 = LightPlugin default
    pub carried_torch: bool,         // (only if Decision 1.C)
}

#[derive(Reflect, Serialize, Deserialize, Default, Debug, Clone, PartialEq)]
#[serde(default)]
pub struct FogConfig {
    pub color: ColorRgb,             // wrapper, see below
    pub density: f32,                // FogFalloff::Exponential's density
}

/// Wrapper around (R, G, B) in 0.0..=1.0. Wraps the serde gap that
/// bevy::Color cannot cross without enabling bevy/serialize.
#[derive(Reflect, Serialize, Deserialize, Default, Debug, Clone, Copy, PartialEq)]
pub struct ColorRgb(pub f32, pub f32, pub f32);

impl ColorRgb {
    pub fn into_color(self) -> Color {
        Color::srgb(self.0, self.1, self.2)
    }
}
```

**Why the wrapper:** Verified at `bevy_color-0.18.1/Cargo.toml:58-61`, `bevy::Color` only derives `Serialize/Deserialize` when its parent crate's `serialize` feature is on. That feature is NOT enabled in Druum's `bevy = { features = ["3d", ...] }` declaration (verified `bevy-0.18.1/Cargo.toml:2322-2330` — `3d` does NOT include `serialize`). Adding `serialize` to Druum's `bevy` features is a Cargo.toml edit and pulls 12 transitive features (verified `bevy_internal-0.18.1/Cargo.toml:345-360`). The wrapper is a 3-line type at zero feature-flag cost.

**Why `#[serde(default)]` everywhere:** Without it, `floor_01.dungeon.ron` would have to add the new fields explicitly or the loader would error. With it, the existing 158-line `floor_01.dungeon.ron` continues to load — `light_positions` becomes `Vec::new()` and `lighting` becomes `LightingConfig::default()`. This is the same pattern Feature #4 used on `CellFeatures` (`#[serde(default)]` at `dungeon.rs:157`) and is verified to work with both `ron 0.11` (the bevy_common_assets path) and `ron 0.12` (the test path) — see `reference_ron_format_compat.md`.

**Reflect derives on every type:** Required because `DungeonFloor` derives `Reflect`, which transitively requires every field to be `Reflect`. Verified Feature #4 pattern at `data/dungeon.rs:25,85,111,123,135,156,181`.

**RON syntax in the floor file** (additions to `floor_01.dungeon.ron`):

```ron
(
    name: "Test Floor 1",
    width: 6,
    height: 6,
    // ... existing fields unchanged ...
    encounter_table: "test_table",

    // NEW — Feature #9 verification torches.
    light_positions: [
        (x: 1, y: 1, color: (1.0, 0.7, 0.3), intensity: 6000.0, range: 10.0, shadows: true),
        (x: 4, y: 1, color: (1.0, 0.7, 0.3), intensity: 6000.0, range: 10.0, shadows: true),
        (x: 2, y: 4, color: (0.6, 0.4, 1.0), intensity: 4000.0, range:  8.0, shadows: true),
        (x: 4, y: 4, color: (1.0, 0.7, 0.3), intensity: 6000.0, range: 10.0, shadows: false),
    ],
    lighting: (
        fog: (color: (0.10, 0.09, 0.08), density: 0.12),
        ambient_brightness: 1.0,
        carried_torch: true,    // only present if Decision 1.C
    ),
)
```

Note the third torch is *blue* (mage-touched) and the fourth has shadows off — exercises the `TorchData::color` and `TorchData::shadows` fields visually.

### Anti-Patterns to Avoid

- **Inserting `AmbientLight` as a resource.** Master research at `research/20260326-01-...md:1141-1146` does this; it will not compile in Bevy 0.18 because `AmbientLight` is a Component (`#[require(Camera)]`), not a Resource. Use `GlobalAmbientLight` instead. Verified at `bevy_light-0.18.1/src/ambient_light.rs:9-12,59-62`. Druum's Feature #8 already does this correctly.
- **Setting `DistanceFog` without specifying `falloff`.** `DistanceFog::default()` is `Linear { 0.0, 100.0 }` — visually equivalent to "no fog" in a 12-unit-wide dungeon. Always specify `FogFalloff::Exponential { density: ... }` explicitly.
- **Spawning a new `Camera3d` for fog.** The Feature #7 camera lives as a child of `PlayerParty`. Fog is added to that camera, not a new one. A second Camera3d would render the dungeon twice with conflicting fog.
- **Marking torches as `Torch` but NOT `DungeonGeometry`.** Feature #8's `despawn_dungeon_entities` queries `With<DungeonGeometry>` — torches without that tag would persist past `OnExit(Dungeon)` and orphan-render in TitleScreen. Always add both markers.
- **`commands.insert_resource(GlobalAmbientLight { brightness: 1.0, color: Color::WHITE, ..default() })` in flicker system.** Insert-resource overrides the resource each frame; the per-frame allocation is wasteful. The OnEnter sets it once; the flicker system mutates `PointLight::intensity` only. Don't conflate the two responsibilities.
- **Reading `Time` in OnEnter to seed phases.** Determinism — using wall-clock at OnEnter means each playthrough's torches phase-shift differently. Use the deterministic per-cell hash (`x.wrapping_mul(31) ^ y.wrapping_mul(17)`) so the same floor always flickers identically.
- **Hand-rolling a noise function with `rand`.** `rand` would be a new top-level dep AND non-deterministic without explicit seeding. Sin-products are deterministic and dep-free.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
| --- | --- | --- | --- |
| Distance-based fog | A custom shader that fades color based on depth | `DistanceFog` component + `FogFalloff::Exponential` | Built into bevy_pbr; uses the depth buffer correctly; integrates with directional-light "glow" out of the box. Verified at `bevy_pbr-0.18.1/src/fog.rs:1-477`. |
| Cubemap shadow management | A system that allocates per-light shadow maps and sorts by distance | Bevy's clustered-forward shadow accounting | Handled at `bevy_pbr-0.18.1/src/render/light.rs:817-865`: stable sort, cap at `max_texture_array_layers / 6`, GPU-side index list. Just set `shadows_enabled: bool` and trust the sort. |
| Per-entity sin phase | A `Component` storing each entity's RNG-seeded phase | Hash of `(x, y)` cell coords stored at spawn | Deterministic, no `rand` dep, no per-frame hash work. Stored as `Torch::phase_offset: f32`. |
| Color in RON | A custom `Deserialize` impl on `bevy::Color` or enable `bevy/serialize` feature | A `ColorRgb(f32, f32, f32)` wrapper struct | The feature flip is a Cargo.toml edit and pulls 12 transitive serde features. Wrapper is 3 LOC. |
| Torch entity cleanup on `OnExit(Dungeon)` | A new query `Query<Entity, With<Torch>>` and despawn loop | Tag torches with the existing `DungeonGeometry` marker | `despawn_dungeon_entities` already iterates `Query<Entity, With<DungeonGeometry>>`. Reuse it; don't add parallel cleanup. |

---

## Common Pitfalls

### Pitfall 1: `Color: Serialize` is feature-gated

**What goes wrong:** A future contributor adds `pub color: Color` to `TorchData` and the `cargo check` fails with `error[E0277]: the trait bound 'Color: Deserialize<_>' is not satisfied` (or worse — compiles when `bevy/serialize` is enabled in *another* dep, fails when it's not).

**Why it happens:** `bevy_color-0.18.1/src/color.rs:51` reads `#[cfg_attr(feature = "serialize", derive(serde::Serialize, serde::Deserialize))]`. Druum doesn't enable that feature. Many tutorials assume the default-features-on Bevy where `serialize` cascades in via other features.

**How to avoid:** Use the `ColorRgb(f32, f32, f32)` wrapper for any color stored in RON. Document this on the wrapper's doc-comment ("DO NOT replace with `bevy::Color` — see `feature_9` research §Pitfall 1 for why").

### Pitfall 2: Torch entities receive `DungeonCamera`'s flicker query if not filtered

**What goes wrong:** The flicker system's query `Query<(&mut PointLight, &Torch)>` looks correct, but if a future contributor removes the `Torch` filter (e.g. "let me also flicker the carried player light"), the query touches *every* `PointLight` in the world — including UI lights, decoration lights, future spell-effect lights. Hard-to-debug intensity drift.

**Why it happens:** ECS filtering is opt-in; removing the marker is a small-feeling edit with broad blast radius.

**How to avoid:** Always filter on `With<Torch>`. If the carried light should also flicker, spawn it with the `Torch` marker too (and a `Torch::base_intensity` matching the carried-light intent). The marker is the contract.

### Pitfall 3: `floor_01.dungeon.ron` round-trip test breaks after schema extension

**What goes wrong:** Feature #4's `dungeon_floor_round_trips_with_real_data` test (at `data/dungeon.rs:386`) constructs a `DungeonFloor` literal in code. Adding fields without updating the literal produces a compile error.

**Why it happens:** Rust 2024 enforces all-fields-or-`..default()` on struct literals.

**How to avoid:** Add `..Default::default()` to the test's literal construction OR explicitly initialize the new fields. The test's existing pattern at `dungeon.rs:425-428` already uses `..Default::default()` for `CellFeatures`, so the pattern is established. Verify by running `cargo test data::dungeon::tests` after each schema edit.

### Pitfall 4: `floor_01.dungeon.ron` passes the round-trip test but fails the integration test

**What goes wrong:** `tests/dungeon_geometry.rs` asserts entity count `== 120`. Adding `light_positions` to floor_01 spawns N new entities; if torches are tagged `DungeonGeometry`, the count becomes `120 + N`.

**Why it happens:** The Layer 2 test was written for Feature #8's exact entity budget.

**How to avoid:** Either (a) update `tests/dungeon_geometry.rs:152` to `120 + N` and add a comment deriving N from `floor_01.light_positions.len()`, OR (b) add a parallel test (`tests/dungeon_lighting.rs`) that asserts `Query<Entity, With<Torch>>::iter().count() == N` and leave the geometry count test alone. The plan should pick (a) (the geometry test should reflect "all `DungeonGeometry`-tagged entities" including torches) and document the new derivation.

### Pitfall 5: Flicker math overshoots `intensity > base * 2.0` and washes out the dungeon

**What goes wrong:** A larger amplitude (`0.5 + 0.5 * sin(...)`) produces a 2× peak that briefly turns the corridor into noon. Over time tuning, the implementer increases amplitude looking for "more dramatic" flicker and ruins the atmosphere.

**Why it happens:** Real torches in dim rooms have ~5-15% perceived intensity variation, not 50%. The eye is very sensitive to brightness changes — small amplitude looks "real", large amplitude looks "broken light bulb".

**How to avoid:** Cap the multiplier in code: `let factor = 1.0 + (0.10 * s1 + 0.05 * s2).clamp(-0.20, 0.20);`. Document the visual reference (real flame footage) in the system's doc-comment. Tuning beyond ±20% is almost certainly wrong.

### Pitfall 6: Two `PointLight`s on the player position (carried + cell torch in the same cell)

**What goes wrong:** Decision 1.A keeps the carried light. When the player stands directly under a wall torch, two PointLights at near-identical positions stack additively — the cell becomes blindingly bright.

**Why it happens:** PBR lights add linearly; there's no falloff between them.

**How to avoid:** If choosing Decision 1.A, set carried-light `intensity` ~50% lower than cell-torch intensity, OR (cleaner) detect "player is within X cells of an enabled torch" and dim the carried light. The latter is ~10 LOC but adds a per-frame query. For #9's difficulty 2/5 scope, the lower-intensity-by-design approach is enough.

### Pitfall 7: `TimeUpdateStrategy` defaults to `Automatic` in tests, making flicker tests flaky

**What goes wrong:** A unit test asserts `light.intensity != base_intensity` after `app.update()`. On a fast CI runner with `delta_secs ≈ 1µs`, the sine value is ≈ 0, so `factor ≈ 1.0` and the assertion fires false positives.

**Why it happens:** Bevy's wall-clock time is non-deterministic per-frame. Verified at `bevy_time-0.18.1/src/lib.rs:99-119`.

**How to avoid:** Use `TimeUpdateStrategy::ManualDuration(Duration::from_millis(N))` in tests of the flicker system — the same pattern Feature #7 uses for `MovementAnimation` (`dungeon/mod.rs:1270`). Step a known number of frames and assert against the deterministic sine value at that exact `t = elapsed_secs`. See `reference_bevy_018_time_update_strategy.md`.

### Pitfall 8: Shadows-enabled point light spawned mid-frame can cause shadow flicker

**What goes wrong:** If `OnEnter(Dungeon)` spawns torches AFTER the camera (or in a different system), the first frame in `Dungeon` has the camera but no torches; the second has both. Bevy's shadow allocation runs once per frame; the first frame the player sees is unlit. (Note: Druum's existing pattern spawns party + geometry + lighting all on `OnEnter(Dungeon)`, so they happen in the same frame.)

**Why it happens:** ECS systems within the same schedule pass run unordered unless explicitly ordered.

**How to avoid:** Either (a) put torch spawn in the same system as `spawn_dungeon_geometry` (which is what Feature #9 likely does — they share the floor asset read), or (b) use `.chain()` to order them: `add_systems(OnEnter(GameState::Dungeon), (spawn_party_and_camera, spawn_dungeon_geometry, spawn_torches_for_floor).chain())`. Druum's Feature #8 does NOT use `.chain()` — the systems happen unordered but in the same `OnEnter` pass, so they all complete before the next frame renders. That's fine here.

---

## Security

### Known Vulnerabilities

| Library | CVE / Advisory | Severity | Status | Action |
| --- | --- | --- | --- | --- |
| `bevy` 0.18.1 | None found in 2026-04 release notes or RustSec | — | Current | Continue |
| `serde` 1.x | None affecting derive usage | — | Current | Continue |
| `ron` 0.12 | None | — | Current | Continue |

No new dependencies are introduced. No new attack surface from network/parsing — `light_positions` and `lighting` are static asset data parsed by the same `RonAssetPlugin` path Feature #4 already secured.

### Architectural Security Risks

| Risk | Affected Architecture Options | How It Manifests | Secure Pattern | Anti-Pattern to Avoid |
| --- | --- | --- | --- | --- |
| Asset deserialization with untrusted RON | All options (RON loader is invoked) | A maliciously crafted dungeon RON could trigger panics in the floor parser if `light_positions` had `f32::NAN` for `range` (NaN comparisons in Bevy clustering math). | Validate `intensity > 0.0`, `range > 0.0`, `density >= 0.0` after load; clamp to safe values with a warn-log if violated. | Trusting RON values blindly; passing `f32::NAN` into `PointLight::range` (causes undefined sphere-vs-OBB intersection in clustering, verified by reading `bevy_light-0.18.1/src/cluster/assign.rs:268-280`). |
| Color RGB out-of-range | All options | A RON file with `color: (5.0, -1.0, 99.0)` would produce HDR-bright torches (Bevy doesn't clamp inputs). | `ColorRgb::into_color` clamps each channel to `0.0..=1.0` before passing to `Color::srgb`. | Treating the RON-supplied color as pre-validated. |
| Resource exhaustion via huge `light_positions` | All options | A RON file with 10,000 torches would spawn 10,000 PointLights — heavy GPU and CPU cost; clustered renderer would warn at MAX_UNIFORM_BUFFER_CLUSTERABLE_OBJECTS = 204 (verified `bevy_pbr-0.18.1/src/cluster.rs:21`) and silently drop the rest, but CPU spawn cost is real. | Add a `validate_light_positions` method to `DungeonFloor` that returns an error if `length > 64`; called in the same place Feature #4's `validate_wall_consistency` is. | Trusting any RON-supplied vector size. |

### Trust Boundaries

For the recommended architecture:

- **`assets/dungeons/*.dungeon.ron` (untrusted, file-system input)** — validation required: `light_positions` length cap (recommend 64), per-torch `intensity > 0.0 && intensity < 1_000_000.0`, `range > 0.0 && range < 100.0`, `color` channels clamped to `[0.0, 1.0]`. What happens if skipped: NaN-poisoned PointLight crashes the clustering pass, OR thousands-of-torches DOS the GPU silently.
- **System call boundary `Time::elapsed_secs()`** — already validated by Bevy (returns `f32` always); no Druum-side concern.
- **No network/IPC inputs** — Druum is single-process desktop; RON files come from `assets/` which the player has root over anyway. Defense is against accidental authoring errors, not adversarial input.

---

## Performance

| Metric | Value / Range | Source | Notes |
| --- | --- | --- | --- |
| `DistanceFog` per-fragment cost | "essentially free" | `bevy_pbr-0.18.1/src/render/fog.wgsl` (~30 lines, single `mix()` call) | One additional `mix(in_color, fog_color, fog_intensity)` per pixel; negligible vs. PBR shading. |
| `PointLight` (no shadows) per-light cost | <0.1ms for ≤8 lights at 1080p | Estimated from `bevy-0.18.1/examples/3d/lighting.rs` (4 lights at 60 FPS easily) | Clustered forward; cost scales with frustum complexity, not light count. |
| `PointLight` (shadows) per-light cost | ~0.5-1.5 ms per shadow-casting light at 1080p | Roadmap §9 estimate; Bevy doesn't publish official numbers | 6 cubemap face renders per shadow pass; depth-only pre-pass, vertex-bound for static scenes. Druum's geometry is ~120 entities, most well within shadow frusta. |
| Cubemap shadow map size | 1024×1024 default | `bevy_light-0.18.1/src/point_light.rs:179` | Configurable via `PointLightShadowMap { size: ... }` resource. Larger = sharper shadows = more VRAM. Default is fine for #9. |
| Max simultaneous shadow-casting point lights | `max_texture_array_layers / 6` (typically ~42 on modern GPUs) | `bevy_pbr-0.18.1/src/render/light.rs:778,821` | But practical cap before noticeable perf hit is ~8 (per master research §Code Examples and roadmap §Cons). |
| Max total point lights (uniform buffer path, no SSBO) | 204 | `bevy_pbr-0.18.1/src/cluster.rs:21` | WebGL2 fallback; on native desktop with SSBOs the cap is much higher but Druum doesn't need to push it. |
| Flicker system per-frame cost | <0.01ms for 4-8 torches | Estimated: 4-8 `Query` iterations × 2 sin calls × 1 multiplication | Free at this entity count. |

No formal benchmarks exist for Druum's specific shape (4-cell-anchored torches + 1 carried torch + ambient fog). The team should run `cargo run --release --features dev` and confirm stable 60+ FPS during manual smoke. Frame time is the regression bar.

---

## Code Examples

Verified patterns from on-disk Bevy 0.18.1 sources.

### Camera with `DistanceFog`

```rust
// Source: bevy-0.18.1/examples/3d/fog.rs:42-54
commands.spawn((
    Camera3d::default(),
    DistanceFog {
        color: Color::srgb(0.25, 0.25, 0.25),
        falloff: FogFalloff::Linear { start: 5.0, end: 20.0 },
        ..default()
    },
));
```

### `GlobalAmbientLight` resource (scene-wide ambient)

```rust
// Source: bevy-0.18.1/examples/3d/lighting.rs:122-127
commands.insert_resource(GlobalAmbientLight {
    color: ORANGE_RED.into(),
    brightness: 200.0,
    ..default()
});
```

### `PointLight` with shadows + emissive child mesh ("torch sconce")

```rust
// Source: bevy-0.18.1/examples/3d/lighting.rs:130-146
commands.spawn((
    PointLight {
        intensity: 100_000.0,
        color: RED.into(),
        shadows_enabled: true,
        ..default()
    },
    Transform::from_xyz(1.0, 2.0, 0.0),
    children![(
        Mesh3d(meshes.add(Sphere::new(0.1).mesh().uv(32, 18))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: RED.into(),
            emissive: LinearRgba::new(4.0, 0.0, 0.0, 0.0),
            ..default()
        })),
    )],
));
```

(For Druum's torches the emissive child mesh is optional polish — a small glowing sphere at the torch position would visually anchor the light source. Out of scope for #9, defer to #25 polish.)

### `time.elapsed_secs()` as the input to a sin-driven animation

```rust
// Source: bevy-0.18.1/examples/audio/spatial_audio_3d.rs:106-107
emitter_transform.translation.x = ops::sin(emitter.stopwatch.elapsed_secs()) * 3.0;
emitter_transform.translation.z = ops::cos(emitter.stopwatch.elapsed_secs()) * 3.0;

// Source: bevy-0.18.1/examples/3d/fog.rs:149-153 (orbit camera around pyramid)
let orbit_scale = 8.0 + ops::sin(now / 10.0) * 7.0;
*transform = Transform::from_xyz(
    ops::cos(now / 5.0) * orbit_scale,
    12.0 - orbit_scale / 2.0,
    ops::sin(now / 5.0) * orbit_scale,
).looking_at(Vec3::ZERO, Vec3::Y);
```

### Per-camera `AmbientLight` override (alternative to mutating `GlobalAmbientLight`)

```rust
// Translation of bevy_light-0.18.1/src/ambient_light.rs:6-39 docs into spawn syntax.
// Use this instead of mutating GlobalAmbientLight if you want the ambient
// brightness to apply ONLY to the dungeon camera (not to e.g. a future minimap camera).
commands.spawn((
    Camera3d::default(),
    Transform::from_xyz(0.0, EYE_HEIGHT, 0.0),
    DungeonCamera,
    AmbientLight {                    // overrides GlobalAmbientLight for THIS camera only
        color: Color::WHITE,
        brightness: 1.0,
        ..default()
    },
));
```

For Feature #9, mutating `GlobalAmbientLight` is fine (Druum has only one Camera3d at a time during Dungeon state — Camera2d HUD doesn't have ambient lighting). The per-camera override matters if Feature #10's auto-map adds a Camera3d-based map preview.

### Recommended wiring in `DungeonPlugin::build`

```rust
// Add inside DungeonPlugin::build (after the existing add_systems calls):

// Update schedule — flicker every frame in Dungeon (no SubState gate; flickers
// even with menu open, immersion preservation).
.add_systems(
    Update,
    flicker_torches.run_if(in_state(GameState::Dungeon)),
)
```

(No new `OnEnter` system if torch spawn is folded into `spawn_dungeon_geometry`. If Decision 2.C extracts to `lighting.rs`, register the spawn there instead.)

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
| --- | --- | --- | --- |
| `commands.insert_resource(AmbientLight { ... })` | `commands.insert_resource(GlobalAmbientLight { ... })` | Bevy 0.18 (April 2026) | Renaming + role-split — `AmbientLight` is now a per-camera Component (`#[require(Camera)]`); `GlobalAmbientLight` is the resource. Master research example needs translation. |
| `Camera3dBundle { camera: ..., projection: ..., ... }` | `(Camera3d::default(), ...)` component tuple | Bevy 0.17 → 0.18 (no bundles) | Already handled correctly in Druum Feature #7/8 code. |
| `time.elapsed_seconds()` | `time.elapsed_secs()` | Bevy 0.13+ rename | Druum already uses the new name; flag any tutorial that uses the old form. |
| `app.add_event::<MyEvent>()` for `Event`-derived types in 0.16+ | `app.add_message::<MyMessage>()` for `Message`-derived types in 0.18+ | Bevy 0.18 family rename | Not directly relevant to #9 (no new events); flagged because the planner may need a `TorchExtinguished` message later. |

**Deprecated/outdated:**

- Master research at `research/20260326-01-...md:1141-1146`: uses `AmbientLight` as a resource. Needs translation to `GlobalAmbientLight`.
- Master research same file at line 1166-1175: uses `intensity: 800.0`, `range: 12.0`, `shadows_enabled: true` for `spawn_torch`. Verified — these values are sane in Bevy 0.18 lumen units (between `lighting.rs`'s 100K hero light and `deferred_rendering.rs`'s 800), but `intensity: 800.0` produces a much dimmer torch than Druum Feature #8's user-tuned 60_000.0. Tuning question for the team.

---

## Validation Architecture

### Test Framework

| Property | Value |
| --- | --- |
| Framework | `cargo test` (built-in) |
| Config file | None (Rust default) |
| Quick run command | `cargo test --features dev plugins::dungeon` (under 5s on warm cache) |
| Full suite command | `cargo test --features dev` (~15-30s including geometry integration test) |

The Layer 2 test pattern from #7/#8 uses `make_test_app()` (`dungeon/mod.rs:923`) which wires `MinimalPlugins + AssetPlugin + StatesPlugin + InputPlugin + StatePlugin + ActionsPlugin + DungeonPlugin` plus `init_asset::<Mesh + StandardMaterial>` plus the `cfg(dev)` `init_resource::<ButtonInput<KeyCode>>` workaround. The same setup will satisfy any new `flicker_torches` test (it reads `Time` and queries `(&mut PointLight, &Torch)`).

### Requirements → Test Map

| Requirement | Behavior | Test Type | Automated Command | File Exists? |
| --- | --- | --- | --- | --- |
| `DistanceFog` is on the dungeon camera after OnEnter | Query `(&DistanceFog, &DungeonCamera)` returns 1 entity | unit (in `dungeon/mod.rs::tests`) | `cargo test plugins::dungeon::tests::distance_fog_attached_to_camera` | needs creating |
| `light_positions` from RON spawns N torch entities | Query `(&Torch, With<DungeonGeometry>)` returns floor.light_positions.len() entities | unit (in `dungeon/mod.rs::tests`) | `cargo test plugins::dungeon::tests::torches_spawned_per_light_positions` | needs creating |
| `flicker_torches` actually changes intensity over time | With `TimeUpdateStrategy::ManualDuration(100ms)`, after 5 frames `light.intensity != torch.base_intensity` | unit (in `dungeon/mod.rs::tests`) | `cargo test plugins::dungeon::tests::flicker_modulates_intensity` | needs creating |
| `flicker_torches` is deterministic (same t → same intensity) | With `ManualDuration` set, intensity at frame N matches `base * (1 + 0.10 * sin(N*100ms*6.4 + phase) + ...)` | unit (in `dungeon/mod.rs::tests`) | `cargo test plugins::dungeon::tests::flicker_is_deterministic` | needs creating |
| `OnExit(Dungeon)` despawns all torches | After OnExit, query `With<Torch>` returns 0 entities | unit (extending existing `on_exit_dungeon_despawns_all_dungeon_geometry`) | `cargo test plugins::dungeon::tests::on_exit_dungeon_despawns_all_dungeon_geometry` | exists at `mod.rs:1397` (extend assertion) |
| `floor_01.dungeon.ron` loads with the new schema fields | Existing RON loads through `RonAssetPlugin` without parse error; `floor.light_positions.len() == 4` | integration (extend `tests/dungeon_geometry.rs` OR add `tests/dungeon_lighting.rs`) | `cargo test --test dungeon_geometry` (after entity-count update) | extend at `tests/dungeon_geometry.rs:152` (count change) + add new entity-count assertion |
| `LightingConfig::default()` works for floors that omit the field | A 2×2 floor RON without `lighting:` field still loads | unit (extend `data::dungeon::tests::dungeon_floor_round_trips_with_real_data`) | `cargo test data::dungeon::tests` | exists at `data/dungeon.rs:386` (extend assertions) |
| `ColorRgb::into_color` clamps out-of-range | `ColorRgb(5.0, -1.0, 0.5).into_color()` produces a Color with channels in `[0.0, 1.0]` | unit (in `data/dungeon.rs::tests`) | `cargo test data::dungeon::tests::color_rgb_clamps` | needs creating |

### Gaps (files to create or extend before implementation)

- [ ] Extend `src/data/dungeon.rs` tests: add `dungeon_floor_round_trips_with_lighting` (covers `LightingConfig` round-trip), `color_rgb_clamps` (covers `ColorRgb::into_color`).
- [ ] Extend `src/plugins/dungeon/mod.rs` tests: add `distance_fog_attached_to_camera`, `torches_spawned_per_light_positions`, `flicker_modulates_intensity`, `flicker_is_deterministic`. Extend `on_exit_dungeon_despawns_all_dungeon_geometry` to also assert `Query<Entity, With<Torch>>::iter().count() == 0`.
- [ ] Update `tests/dungeon_geometry.rs:152`: change `count, 120` to `count, 120 + N` (where N = number of torches added to floor_01.dungeon.ron) and add a comment deriving N from the asset.

---

## Open Questions

1. **What N should `floor_01.dungeon.ron` have for `light_positions`?**
   - What we know: spec says "3-4 torch positions for visual verification".
   - What's unclear: exact authoring (which cells, which colors). The example in this doc has 4 torches at (1,1)/(4,1)/(2,4)/(4,4) but those are illustrative.
   - Recommendation: planner picks 3-4 cell coordinates from the visible-from-entry cells (around (1,1)..(4,4) on floor_01) so the manual smoke test reveals lighting immediately. Coordinate this with Feature #8's manual-smoke verification path.

2. **What `density` value is right for stone-dungeon fog?**
   - What we know: master research example uses `0.15`. Feature #8 uses `range: 12.0` for the carried torch (so fog density should match — at density 0.15 visibility is ~26 world units = ~13 cells).
   - What's unclear: subjective. Does "warm dark gray" + density 0.15 read as "atmospheric" or "overdone"?
   - Recommendation: implementer iterates via manual smoke. Recommended starting value in spec: `density: 0.12` (slightly less dense than master research — Druum's corridors are 6 cells across, smaller than master research's reference). Document in commit message.

3. **Does the carried player torch flicker (Decision 1.A or 1.C with carried_torch=true)?**
   - What we know: aesthetically, a carried torch should flicker too.
   - What's unclear: does the implementer add `Torch` marker to the existing player-attached PointLight, or leave it steady?
   - Recommendation: add the `Torch` marker to the player light too — same flicker formula applies, with `phase_offset: 0.0` (or `f32::consts::PI` so it doesn't sync with the (0,0)-cell torch).

4. **Should `ambient_brightness` in `LightingConfig` accept a color too, or just a scalar?**
   - What we know: roadmap says "low ambient light"; spec says fog parameters are per-floor.
   - What's unclear: does the implementer need to support tinted ambient (e.g. blue ambient on a "frozen floor") in #9, or only brightness?
   - Recommendation: just brightness for #9 (scalar `ambient_brightness: f32`). Add `ambient_color: ColorRgb` later if a future floor needs it. Keeps the schema small.

5. **Is the per-floor fog applied via the `OnEnter(Dungeon)` system that reads the floor handle, or via a separate "fog setup" system?**
   - What we know: the camera spawns in `spawn_party_and_camera`, which already reads `floors.get(&assets.floor_01)` to look up the entry point.
   - What's unclear: should fog be added to the camera spawn (one place) or to a separate system that runs after spawn (cleaner, but reads the floor twice)?
   - Recommendation: add fog to the camera tuple inside `spawn_party_and_camera` — single read, single allocation. The system already has the floor in scope.

---

## Sources

### Primary (HIGH confidence)

- [Bevy 0.18.1 local source: `bevy_pbr-0.18.1/src/fog.rs`](file:///Users/nousunio/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bevy_pbr-0.18.1/src/fog.rs) — `DistanceFog` struct fields, `FogFalloff` enum variants (Linear, Exponential, ExponentialSquared, Atmospheric), default impl is `Linear { 0.0, 100.0 }`, Koschmieder helpers.
- [Bevy 0.18.1 local source: `bevy_light-0.18.1/src/point_light.rs`](file:///Users/nousunio/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bevy_light-0.18.1/src/point_light.rs) — `PointLight` struct fields (intensity in lumens, default 1M lumens, range default 20.0, shadows_enabled default false, soft_shadows behind `experimental_pbr_pcss` feature), `PointLightShadowMap` resource (size default 1024).
- [Bevy 0.18.1 local source: `bevy_light-0.18.1/src/ambient_light.rs`](file:///Users/nousunio/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bevy_light-0.18.1/src/ambient_light.rs) — `AmbientLight` Component (per-camera, `#[require(Camera)]`) vs `GlobalAmbientLight` Resource; both default `brightness: 80.0` color WHITE; `GlobalAmbientLight::NONE` const.
- [Bevy 0.18.1 local source: `bevy_pbr-0.18.1/src/render/light.rs`](file:///Users/nousunio/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bevy_pbr-0.18.1/src/render/light.rs) — Lines 778, 817-865: `point_light_shadow_maps_count` clamps to `max_texture_array_layers / 6`; sort by `(shadows_enabled descending, entity ascending)`; `MAX_DIRECTIONAL_LIGHTS = 10`, `MAX_CASCADES_PER_LIGHT = 4`.
- [Bevy 0.18.1 local source: `bevy_light-0.18.1/src/cluster/assign.rs`](file:///Users/nousunio/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bevy_light-0.18.1/src/cluster/assign.rs) — Lines 105-120: `ClusterableObjectType::ordering()` puts shadow-enabled lights first.
- [Bevy 0.18.1 local source: `bevy_pbr-0.18.1/src/cluster.rs`](file:///Users/nousunio/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bevy_pbr-0.18.1/src/cluster.rs) — Line 21: `MAX_UNIFORM_BUFFER_CLUSTERABLE_OBJECTS = 204`; line 341: `MAX_INDICES = 16384`.
- [Bevy 0.18.1 local source: `bevy_time-0.18.1/src/time.rs`](file:///Users/nousunio/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bevy_time-0.18.1/src/time.rs) — Lines 283-308: `delta_secs()`, `elapsed_secs()`, `elapsed_secs_f64()` are the public time accessors.
- [Bevy 0.18.1 local source: `bevy_math-0.18.1/src/ops.rs`](file:///Users/nousunio/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bevy_math-0.18.1/src/ops.rs) — `bevy::math::ops::sin/cos/exp/...` re-exports of std (or libm) trig functions.
- [Bevy 0.18.1 local source: `bevy_color-0.18.1/Cargo.toml`](file:///Users/nousunio/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bevy_color-0.18.1/Cargo.toml) — Lines 58-61: `serialize` feature gates `serde::Serialize/Deserialize` derives on `Color`.
- [Bevy 0.18.1 local source: `bevy-0.18.1/Cargo.toml`](file:///Users/nousunio/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bevy-0.18.1/Cargo.toml) — Lines 2322-2330: `3d` feature does NOT include `serialize`; line 2536: top-level `serialize` feature exists separately.
- [Bevy 0.18.1 local source: `bevy_internal-0.18.1/Cargo.toml`](file:///Users/nousunio/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bevy_internal-0.18.1/Cargo.toml) — Lines 345-360: `serialize` cascades to 12 transitive feature flags.
- [Bevy 0.18.1 local source: `bevy_light-0.18.1/src/volumetric.rs`](file:///Users/nousunio/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bevy_light-0.18.1/src/volumetric.rs) — `VolumetricFog` and `FogVolume` API (deferred to future feature per roadmap).
- [Bevy 0.18.1 example: `bevy-0.18.1/examples/3d/fog.rs`](file:///Users/nousunio/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bevy-0.18.1/examples/3d/fog.rs) — Canonical `DistanceFog` setup pattern; orbit-camera demonstrates `time.elapsed_secs()` + `ops::sin/cos`; key bindings for runtime fog parameter switching.
- [Bevy 0.18.1 example: `bevy-0.18.1/examples/3d/lighting.rs`](file:///Users/nousunio/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bevy-0.18.1/examples/3d/lighting.rs) — Canonical `GlobalAmbientLight`, `PointLight`, `SpotLight`, `DirectionalLight` setup with PBR materials and emissive child meshes.
- [Druum project memory: `reference_bevy_018_mesh_lighting_gotchas.md`](file:///Users/nousunio/Repos/Learnings/claude-code/druum/.claude/agent-memory/researcher/reference_bevy_018_mesh_lighting_gotchas.md) — `AmbientLight` Component-vs-Resource trap; `Cuboid::new` full-lengths; `Plane3d` single-sided.
- [Druum project memory: `reference_bevy_018_camera3d_components.md`](file:///Users/nousunio/Repos/Learnings/claude-code/druum/.claude/agent-memory/researcher/reference_bevy_018_camera3d_components.md) — `*Bundle` types removed in 0.18; component-tuple spawn pattern.
- [Druum project memory: `reference_bevy_018_time_update_strategy.md`](file:///Users/nousunio/Repos/Learnings/claude-code/druum/.claude/agent-memory/researcher/reference_bevy_018_time_update_strategy.md) — `TimeUpdateStrategy::ManualDuration` for deterministic test time; required for flicker tests.
- [Druum project memory: `feedback_bevy_input_test_layers.md`](file:///Users/nousunio/Repos/Learnings/claude-code/druum/.claude/agent-memory/researcher/feedback_bevy_input_test_layers.md) — Layer 1/2/3 test patterns; the `init_resource::<ButtonInput<KeyCode>>()` for `--features dev`.
- [Druum project memory: `reference_ron_format_compat.md`](file:///Users/nousunio/Repos/Learnings/claude-code/druum/.claude/agent-memory/researcher/reference_ron_format_compat.md) — ron 0.11 vs 0.12 byte-equivalence verified for struct/enum/Option/Vec/primitives.

### Secondary (MEDIUM confidence)

- [Druum master research §Code Examples: Dungeon Lighting Setup](file:///Users/nousunio/Repos/Learnings/claude-code/druum/research/20260326-01-bevy-first-person-dungeon-crawler-rpg.md) — Lines 1130-1177: `setup_dungeon_lighting` (uses outdated `AmbientLight` resource — needs translation per memory) and `spawn_torch` (uses `intensity: 800.0`, `range: 12.0`, `shadows_enabled: true`). Patterns are conceptually correct; API translations required for 0.18.
- [Druum Feature #8 implementation summary](file:///Users/nousunio/Repos/Learnings/claude-code/druum/project/implemented/20260504-023000-feature-8-3d-dungeon-renderer.md) — Wizardry-style override applied (PointLight grandchild of PlayerParty, intensity 60_000, range 12.0, shadows off; `GlobalAmbientLight { brightness: 50.0 }` near-black on OnEnter). Ground truth for current state.
- [Druum Feature #8 plan](file:///Users/nousunio/Repos/Learnings/claude-code/druum/project/plans/20260503-223000-feature-8-3d-dungeon-renderer.md) — Layer 2 test pattern, `make_test_app()` setup, asset-tolerant spawn pattern.
- [Druum Feature #4 plan](file:///Users/nousunio/Repos/Learnings/claude-code/druum/project/plans/20260501-230000-feature-4-dungeon-grid-data-model.md) — Pattern for `#[serde(default)]` on optional schema fields, `Reflect/Serialize/Deserialize/Default` derive set, RON round-trip test pattern.
- [Roadmap §9 Dungeon Lighting & Atmosphere](file:///Users/nousunio/Repos/Learnings/claude-code/druum/project/roadmaps/20260429-01-bevy-dungeon-crawler-roadmap.md) — Lines 489-536. Spec source. Difficulty 2/5, +120 to +200 LOC budget, Δ deps = 0, fog should be per-floor RON parameter.

### Tertiary (LOW confidence)

- None — every recommendation in this document is backed by either on-disk Bevy 0.18.1 source verification or a direct Druum project memory file. No claims rely on training data alone.

---

## Metadata

**Confidence breakdown:**

- Standard stack (no new deps): HIGH — verified against `Cargo.toml`, `bevy/3d` feature chain, and `serde`/`ron` already declared.
- Architecture options for Decision 1 (player torch): HIGH — three viable paths surfaced with verified pros/cons; Wizardry/genre-canonical defaults documented; planner asks user.
- Architecture options for Decision 2 (module split): HIGH — three viable paths surfaced with LOC/precedent rationale; planner asks user.
- Architecture pattern recommendations (RON schema, flicker formula, shadow cap, validation): HIGH — every Bevy API field name + type verified at named source file + line number; serde gating explicitly verified at `Cargo.toml` level; flicker formula verified against canonical examples.
- Pitfalls: HIGH — every pitfall traced to a specific Bevy source line OR to an established Druum memory file (e.g. the `AmbientLight` Component-vs-Resource trap).
- Performance numbers: MEDIUM — Bevy doesn't publish official per-light frame-time costs; estimates anchored in roadmap and `examples/3d/lighting.rs` empirical (4 lights at 60 FPS in canonical example). Druum should manually verify with `cargo run --release --features dev`.
- Open Questions: HIGH — 5 specific decisions left for planner+user; each has a tentative recommendation noted.

**Research date:** 2026-05-04
