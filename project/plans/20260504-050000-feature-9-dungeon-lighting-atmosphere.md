# Plan: Dungeon Lighting & Atmosphere — Feature #9

**Date:** 2026-05-04
**Status:** Complete
**Research:** ../research/20260504-040000-feature-9-dungeon-lighting-atmosphere.md
**Depends on:** 20260501-230000-feature-4-dungeon-grid-data-model.md, 20260503-130000-feature-7-grid-movement-first-person-camera.md, 20260503-223000-feature-8-3d-dungeon-renderer.md

## Goal

Add per-floor atmospheric lighting to the dungeon: a `DistanceFog` on the dungeon camera (per-floor color + Exponential density from RON), per-cell `PointLight` torches placed via a new `light_positions: Vec<TorchData>` field on `DungeonFloor`, a deterministic two-sine flicker animation that modulates both cell torches and the existing carried torch (out-of-phase by `phase_offset = π`), and an explicitly-tunable `ambient_brightness` per floor. Sample 4 cell torches in `floor_01.dungeon.ron` for visual verification. Net delivery: ~+200-280 production LOC + ~+120-180 test LOC across `src/plugins/dungeon/mod.rs` (split: production stays in `mod.rs`, tests relocate to a new `src/plugins/dungeon/tests.rs`), plus schema additions in `src/data/dungeon.rs`, plus a 4-torch authoring example in `floor_01.dungeon.ron`. **Zero new Cargo dependencies. Cargo.toml + Cargo.lock byte-unchanged.**

## Approach

The research (HIGH confidence; every Bevy 0.18.1 lighting/fog API verified on-disk) recommends extending the existing `DungeonPlugin` with three new responsibilities — fog setup on the camera, per-cell torch spawning, and a single per-frame flicker system — without adding any new crates and without restructuring `mod.rs` for production code. Carry-over decisions from features #4–#8 (single-file production module, `DungeonGeometry` cleanup tag, `OnEnter/OnExit` lifecycle, `make_test_app` Layer 2 test pattern) all apply unchanged.

The architectural decisions baked into this plan:

1. **Player-torch reconciliation: KEEP the existing carried `PointLight` AND ADD per-cell torches.** The user-confirmed Option A. The carried torch (intensity `60_000.0`, range `12.0`, color `srgb(1.0, 0.85, 0.55)`, `shadows_enabled: false`, parented to `DungeonCamera`) is the source of truth — its properties do NOT change. The user explicitly accepts additive light stacking under sconces ("bright spots under sconces are fine"). The ONLY modification to the carried torch is adding a `Torch { base_intensity: 60_000.0, phase_offset: std::f32::consts::PI }` component so the same `flicker_torches` system that drives cell torches also flickers the carried light — desynced by π so it doesn't pulse in time with the (0,0)-cell torch.

2. **Module layout: keep production code single-file in `src/plugins/dungeon/mod.rs`. Move ONLY the `#[cfg(test)] mod tests { ... }` block to a new sibling `src/plugins/dungeon/tests.rs` file.** User-proposed Option D, off-menu from research's A/B/C. Standard Rust pattern: declare `#[cfg(test)] mod tests;` in `mod.rs` (replacing the existing inline `mod tests { ... }` body) and move the body verbatim to `tests.rs`. Pure file-move — `use super::*;` keeps working because `tests` remains a child module of `mod`. This refactor lands BEFORE any new lighting code so the lighting work appears in a smaller `mod.rs`. Test count and behavior must remain identical (`cargo test` baseline: 61 lib + 3 integration).

3. **Per-floor lighting parameters live on `DungeonFloor` via two new `#[serde(default)]` fields: `light_positions: Vec<TorchData>` and `lighting: LightingConfig`.** Both have `#[serde(default)]` so the existing `floor_01.dungeon.ron` continues to parse before we update it. `LightingConfig` carries `fog: FogConfig`, `ambient_brightness: f32`. `FogConfig` carries `color: ColorRgb`, `density: f32`. `TorchData` carries `x: u32, y: u32, color: ColorRgb, intensity: f32, range: f32, shadows: bool`. Round-trip through `ron 0.12` is part of the test suite (extends the existing pattern in `data/dungeon.rs`).

4. **`ColorRgb(f32, f32, f32)` wrapper struct dodges the `bevy::Color: Serialize` feature gate.** Verified at `bevy_color-0.18.1/Cargo.toml:58-61`: `bevy::Color` only derives `Serialize/Deserialize` when `bevy_color/serialize` is on. Druum's `bevy = { features = ["3d", ...] }` does NOT pull this feature in (verified `bevy-0.18.1/Cargo.toml:2322-2330`). Enabling it would cascade 12 transitive features and modify `Cargo.toml` — violating the byte-unchanged guarantee. The wrapper is ~10 LOC including `into_color()` (which clamps each channel to `[0.0, 1.0]` for trust-boundary safety) and is documented in-place ("DO NOT replace with `bevy::Color` — feature gate, see Pitfall 1 of Feature #9 research"). Both `TorchData::color` and `FogConfig::color` use `ColorRgb`.

5. **`DistanceFog` is added to the existing `Camera3d` child of `PlayerParty` inside `spawn_party_and_camera`.** Same place that already reads the floor handle for `entry_point`; one extra read of `floor.lighting.fog` to populate the fog. Falloff is **always** `FogFalloff::Exponential { density }` — never `..default()` for the falloff — because `DistanceFog::default().falloff` is `Linear { 0.0, 100.0 }`, which on a 12-unit-wide dungeon reads as solid grey (Pitfall §Anti-Patterns). The floor's `FogConfig::default()` returns warm dark grey color `(0.10, 0.09, 0.08)` and `density: 0.12` so a floor that omits `lighting:` still gets atmospheric fog.

6. **Per-cell torches spawn inside `spawn_dungeon_geometry` (same `OnEnter(GameState::Dungeon)` system that already spawns floor + ceiling + walls).** Sharing the system means torches and walls iterate the same floor handle in one read; torches are tagged `DungeonGeometry` so the existing OnExit cleanup handles them automatically (no parallel cleanup). The same system also writes `commands.insert_resource(GlobalAmbientLight { brightness: floor.lighting.ambient_brightness, color: Color::WHITE, ..default() })` — replacing the current hard-coded `1.0` with the per-floor value. The `LightingConfig` default (`ambient_brightness: 1.0`) preserves current near-black behavior for any floor that omits `lighting:`. Restoration to `GlobalAmbientLight::default()` on OnExit is unchanged (Feature #8's policy).

7. **Flicker formula: `factor = 1.0 + 0.10 * sin(t * 6.4 + phase) + 0.05 * sin(t * 23.0 + phase * 1.7)`, clamped `[0.80, 1.20]` for safety.** Verified deterministic; ±15% peak amplitude (theoretical) clamped to ±20% (defensive). Per-entity `phase_offset` for cell torches comes from `(x.wrapping_mul(31) ^ y.wrapping_mul(17)) as f32 * 0.123` (a stable hash; deterministic across runs). Carried torch uses `std::f32::consts::PI` so it never aligns with the (0,0)-cell torch. Phase is computed at spawn, stored on `Torch::phase_offset`, never re-hashed at frame time. `bevy::math::ops::sin` is the portable alias for `f32::sin` when `std` is on (Druum's case) — verified `bevy_math-0.18.1/src/ops.rs:96-100`.

8. **`flicker_torches` system runs in `Update`, gated `run_if(in_state(GameState::Dungeon))`, query `Query<(&mut PointLight, &Torch)>`.** No `DungeonSubState` gate — torches flicker even when a menu is open (immersion). The `Torch` marker is the contract: ANY `PointLight` not tagged `Torch` is untouched (Pitfall §Pitfall 2). System body iterates the query, computes the factor, sets `light.intensity = torch.base_intensity * factor`. Per-frame cost is negligible at 4 cell torches + 1 carried torch.

9. **Shadow cap is authored, not coded.** Bevy 0.18 stable-sorts `shadows_enabled: true` lights by entity ID and truncates at `max_texture_array_layers / 6` (verified `bevy_pbr-0.18.1/src/render/light.rs:817-865`). At 4 authored torches per floor, manually setting `shadows: true` on at most 3 entries achieves the spec's "4 per visible region" cap with zero LOC. Documented in the `TorchData::shadows` doc-comment so future floor authors don't accidentally enable shadows on every torch. The carried torch keeps `shadows_enabled: false` (existing code; do not change).

10. **`src/data/dungeon.rs` exception #2: schema extension is a planned modification.** This file is otherwise frozen (Feature #4 directive, with the doc-comment fix in #8 as the only prior exception). The two new fields on `DungeonFloor` (`light_positions`, `lighting`) and the four new types (`ColorRgb`, `TorchData`, `FogConfig`, `LightingConfig`) are an explicit roadmap-mandated extension and the second documented exception. Re-export the new public types from `src/data/mod.rs` via the existing `pub use dungeon::{...}` line (per Feature #3's data re-export pattern).

11. **`assets/dungeons/floor_01.dungeon.ron` gets a 4-torch sample + a `lighting:` block.** Coordinates pinned in Step 3; recommended starting values (`intensity: 6_000.0`, `range: 10.0`, warm color `(1.0, 0.7, 0.3)` for 3 torches, blue mage-touched `(0.6, 0.4, 1.0)` for 1 torch) come from research §Pattern 2 / §Pattern 5. 3 torches with `shadows: true`, 1 with `shadows: false` (the spec's authored cap). Adding 4 `DungeonGeometry`-tagged torch entities updates the `tests/dungeon_geometry.rs` count from 120 to 124.

12. **Tests follow Layer 2 pattern from #6/#7/#8.** Reuse `make_test_app()` and the helpers in the relocated `tests.rs` as-is. New tests cover: (a) `DistanceFog` is on the camera after OnEnter, (b) `light_positions.len()` matches the spawned `Torch`-tagged entity count, (c) flicker modulates intensity (with `TimeUpdateStrategy::ManualDuration` for determinism — Pitfall §Pitfall 7), (d) flicker is deterministic (same `t` → same intensity to within `f32::EPSILON * base`), (e) extend the existing `on_exit_dungeon_despawns_all_dungeon_geometry` to also assert no `Torch` entities remain. New unit tests in `data/dungeon.rs::tests`: (f) `ColorRgb::into_color` clamps out-of-range, (g) `LightingConfig` round-trip through `ron 0.12`. Integration test `tests/dungeon_geometry.rs` updates the entity count from 120 to 124 with a comment deriving the math from `floor_01.light_positions.len()`.

13. **`#[cfg(test)] mod tests;` in `mod.rs` replaces the inline body.** The existing single-line declaration `#[cfg(test)] mod tests { ... }` block (lines 772-1436) becomes `#[cfg(test)] mod tests;` (one line). The full body moves verbatim to `src/plugins/dungeon/tests.rs`. `use super::*;` at the top of the moved module keeps all existing imports and helper visibility intact — `super` resolves to `mod.rs`'s items just like before. No `pub(super)` or `pub(crate)` changes needed.

## Critical

- **Zero new Cargo dependencies. `Cargo.toml` and `Cargo.lock` MUST be byte-unchanged.** All required APIs (`DistanceFog`, `FogFalloff`, `PointLight`, `GlobalAmbientLight`, `Time::elapsed_secs`, `bevy::math::ops::sin`, `Color::srgb`) are in `bevy::prelude::*` via the existing `features = ["3d"]` declaration at `Cargo.toml:11`. **Do NOT add a noise crate, a flicker crate, or any other dependency.** If a new dep appears truly necessary during implementation, STOP and surface it as a question. Final Verification step explicitly runs `git diff Cargo.toml Cargo.lock` and expects empty output.

- **Do NOT enable `bevy/serialize` to "make `Color` serializable".** Verified that flipping this feature pulls 12 transitive features and changes Cargo.toml (`bevy_internal-0.18.1/Cargo.toml:345-360`). Use the `ColorRgb(f32, f32, f32)` wrapper instead. Document this on the wrapper's doc-comment so a future contributor doesn't undo the workaround.

- **Do NOT modify the carried torch's properties.** `intensity: 60_000.0`, `range: 12.0`, `color: srgb(1.0, 0.85, 0.55)`, `shadows_enabled: false`, parented to `DungeonCamera` via nested `children![]` — all stay verbatim. The ONLY change to the carried torch tuple is appending `Torch { base_intensity: 60_000.0, phase_offset: std::f32::consts::PI }` so the flicker system finds it. **Where this plan/research and the existing code disagree on the carried torch, trust the code.** No "compensating intensity drop for additive stacking", no `..default()` rewrites, no relocating the torch off the camera.

- **`DistanceFog::default()` falloff is `Linear { 0.0, 100.0 }` — invisible at dungeon scale.** ALWAYS specify `falloff: FogFalloff::Exponential { density: ... }` explicitly. Verified at `bevy_pbr-0.18.1/src/fog.rs:465-476`. A camera with `DistanceFog { color: ..., ..default() }` will appear to have no fog because the dungeon (12 world units across) doesn't reach 100 world units. Default-falloff would mask the bug — every test for "fog is on the camera" must also assert the falloff is `Exponential`.

- **`AmbientLight` is a Component (`#[require(Camera)]`); `GlobalAmbientLight` is the Resource.** Verified at `bevy_light-0.18.1/src/ambient_light.rs:9-89`. Druum Feature #8 already does this correctly (`commands.insert_resource(GlobalAmbientLight { ... })`). A future contributor copying from outdated tutorials might write `commands.insert_resource(AmbientLight { ... })` — that fails to compile. Keep using `GlobalAmbientLight`.

- **Torches MUST be tagged `DungeonGeometry` so OnExit cleanup despawns them.** The existing `despawn_dungeon_entities` query is `Query<Entity, With<DungeonGeometry>>` — anything spawned in the dungeon that wants automatic cleanup needs this marker. Cell torches get BOTH `Torch` (flicker filter) AND `DungeonGeometry` (cleanup). The CARRIED torch does NOT need `DungeonGeometry` because it's a grandchild of `PlayerParty` and despawns recursively when `PlayerParty` despawns (verified `bevy_ecs-0.18.1/src/system/commands/entity_command.rs:242-249`).

- **Flicker query MUST filter on `With<Torch>` — never query bare `&mut PointLight`.** Pitfall §Pitfall 2: removing the marker filter would touch every `PointLight` in the world, including any future spell-effect lights, UI lights, etc. The `Torch` marker is the contract; broaden it (more entities get the marker) rather than removing the filter.

- **Flicker phase MUST be deterministic.** Pitfall §Anti-Patterns: do NOT seed phase from `Time::elapsed_secs()` at OnEnter, do NOT use `rand`, do NOT use `Instant::now()`. Use the cell-coord hash (`x.wrapping_mul(31) ^ y.wrapping_mul(17)`) for cell torches and a hard-coded constant (`std::f32::consts::PI`) for the carried torch. The same floor must always flicker identically across playthroughs (saves consistency, frame-perfect tests).

- **Flicker amplitude clamp `[0.80, 1.20]` is a defensive guard, not a tuning knob.** Pitfall §Pitfall 5: real torches in dim rooms have ~5-15% perceived intensity variation. The two-sine sum theoretically peaks at ±15%; the clamp protects against future tuning that pushes amplitude too high. If the implementer wants to tune the look, change the `0.10` and `0.05` coefficients OR widen the clamp to `[0.70, 1.30]` — but never remove the clamp.

- **`#[serde(default)]` on BOTH new `DungeonFloor` fields is mandatory.** Without it, the existing `floor_01.dungeon.ron` (which doesn't have `light_positions:` or `lighting:` yet at the start of Step 3) fails to parse. With it, missing fields default to `Vec::new()` and `LightingConfig::default()` respectively. Same pattern Feature #4 used on `CellFeatures` (`#[serde(default)]` at `dungeon.rs:157`).

- **`LightingConfig::default()` and `FogConfig::default()` MUST be explicit `impl Default` blocks (not `#[derive(Default)]`).** `#[derive(Default)]` would set `ambient_brightness: 0.0` (pure black) and `density: 0.0` (no fog) — both wrong defaults for a dungeon. Hand-written impls return the recommended atmospheric starting values: `ambient_brightness: 1.0`, `fog: FogConfig::default()`; `FogConfig::default()` returns `color: ColorRgb(0.10, 0.09, 0.08)`, `density: 0.12`. `ColorRgb` and `TorchData` keep `#[derive(Default)]` (default `(0.0, 0.0, 0.0)` and zeroed-floats are sane sentinels there).

- **`src/data/dungeon.rs` exception #2: this is the second and final allowed modification to the otherwise-frozen file.** Feature #8 was the first (doc-comment fix at line 18). Feature #9 is the second (schema extension). After this feature, any further edit to `data/dungeon.rs` must come with a fresh research/planning cycle and an explicit user directive. Keep the exception scoped: only add the new types and the two new fields on `DungeonFloor`. Do NOT refactor existing types, change existing field names, or modify `validate_wall_consistency` / `can_move` / `wall_between` / `is_well_formed`.

- **`tests/dungeon_geometry.rs` count update from 120 → 124 MUST come with a derivation comment.** The existing comment derives `120 = 36 floor + 36 ceiling + 48 walls`. New count `124 = 36 + 36 + 48 + 4 torches (from floor_01.light_positions)`. The derivation must be in the test docstring AND in the assertion failure message so a future asset edit (adding/removing torches) surfaces with a clear diff. Same regression-guard discipline as Feature #8.

- **Test count baseline must hold across the tests-relocate refactor.** Before lighting work: `cargo test` shows 61 lib + 3 integration tests. After tests relocate (Step 1): `cargo test` MUST show the IDENTICAL 61 lib + 3 integration. After lighting work: 67 lib (+6 new) + 3 integration. If the count regresses at Step 1, the relocate is wrong (likely a `super::` import broke or a `#[test]` attribute got dropped) — fix before proceeding.

- **`#[cfg(feature = "dev")] init_resource::<ButtonInput<KeyCode>>()`** is already present in `make_test_app()`; the relocated `tests.rs` keeps it. Same gotcha as #5/#6/#7/#8 (`feedback_dev_feature_buttoninput_in_tests.md`). Any new test app added in this feature must include this; the existing helper covers all in-module Layer 2 tests.

- **`MovedEvent` derives `Message`, NOT `Event` — Bevy 0.18 family rename.** Feature #9 doesn't add new Messages, but if the implementer is tempted to add (say) a `TorchExtinguished` message later, use `#[derive(Message)]` and `app.add_message::<...>()`. Do not use `add_event` — verified rename per Feature #2 lessons.

- **Symmetric `#[cfg(feature = "dev")]` gating.** No dev-only code is anticipated in Feature #9 (no debug-render-toggle for fog, no per-torch debug overlay). If a future contributor adds one, the function definition AND the `add_systems` call MUST both be cfg-gated. Symmetric gating is the established pattern (`project/resources/20260501-102842-dev-feature-pattern.md`).

- **All 7 verification commands must pass with ZERO warnings:** `cargo check`, `cargo check --features dev`, `cargo clippy --all-targets -- -D warnings`, `cargo clippy --all-targets --features dev -- -D warnings`, `cargo test`, `cargo test --features dev`, `cargo fmt --check`. `Cargo.toml` and `Cargo.lock` MUST be byte-unchanged.

- **Manual visual smoke is REQUIRED before declaring done.** The whole point of the feature is "the dungeon now feels atmospheric." Run `cargo run --features dev`, F9 to Dungeon, walk around floor_01, verify (a) fog is visible at corridor distance (walls fade to dark grey), (b) standing at a cell with a torch is brighter than a cell without, (c) torches visibly flicker (subtle, not strobing), (d) the carried torch flickers but is desynced from the (1,1) cell torch. Record findings in **Implementation Discoveries**.

- **Atomic commits per Features #7/#8 style — one logical change per commit.** Suggested commit boundaries: (1) tests relocation, (2) schema additions, (3) `floor_01.ron` torch authoring, (4) `Torch` component + flicker helpers, (5) `spawn_party_and_camera` fog + carried-torch flicker tag, (6) `spawn_dungeon_geometry` cell-torch spawn + ambient hookup, (7) `flicker_torches` system + plugin registration, (8) test additions, (9) `tests/dungeon_geometry.rs` count update. Each commit should compile and `cargo test` should pass at every checkpoint. If commits 4–7 must temporarily share a checkpoint to avoid a transient compile break, that's fine.

## Steps

### Step 1: Relocate `mod tests` block from `mod.rs` into a new `tests.rs` file (refactor — no behavioral change)

User-confirmed Decision 2 (Option D). Pure file-move; tests must pass identically before and after.

- [x] In `src/plugins/dungeon/mod.rs`, locate the `#[cfg(test)] mod tests { ... }` block (currently lines 772–1436). Note the exact opening line and closing brace.
- [x] Create `src/plugins/dungeon/tests.rs`. Paste the BODY of the `mod tests { ... }` block (everything between the opening `{` and closing `}`) into the new file verbatim. The new file's first line should be `use super::*;` (or whatever the original first line was — preserve order).
- [x] In `src/plugins/dungeon/mod.rs`, replace the entire `#[cfg(test)] mod tests { ... }` block (lines 772–1436) with two lines:
  ```rust
  #[cfg(test)]
  mod tests;
  ```
- [x] Verify the new `tests.rs` does NOT add `#[cfg(test)]` at the top — that attribute lives on the `mod tests;` declaration in `mod.rs`. The file body itself is plain Rust.
- [x] Verify `super::*` still resolves — `tests` is a child module of `mod`, so `super` is `mod`, and all of `mod.rs`'s items (`PlayerParty`, `DungeonCamera`, `GridPosition`, `Facing`, `DungeonGeometry`, `MovedEvent`, `MovementAnimation`, `grid_to_world`, `facing_to_quat`, `wall_transform`, `wall_material`, `CELL_SIZE`, etc.) remain accessible without any `pub(super)` changes.
- [x] Run `cargo test` — MUST show 61 lib tests pass + 3 integration tests pass (identical to baseline). If count differs by even one, the relocate broke something — revert or fix before proceeding.
- [x] Run `cargo test --features dev` — MUST also show identical baseline.
- [x] Run `cargo clippy --all-targets -- -D warnings` and `cargo clippy --all-targets --features dev -- -D warnings` — both must pass with zero warnings (no new warnings introduced by the file move).
- [x] Run `cargo fmt --check` — must report zero diff.
- [x] Commit: "Refactor: move `dungeon::tests` body to a sibling file"

**Done state:** `src/plugins/dungeon/mod.rs` is ~640 LOC smaller; `src/plugins/dungeon/tests.rs` is ~664 LOC (the moved body). All tests pass with identical counts. No production code changed.

### Step 2: Add `ColorRgb`, `TorchData`, `FogConfig`, `LightingConfig` types + new `DungeonFloor` fields to `src/data/dungeon.rs`

This is the second and final allowed exception to the `data/dungeon.rs` freeze (after #8's doc-comment fix).

- [x] In `src/data/dungeon.rs`, add the four new types BEFORE the `DungeonFloor` struct declaration (after `WallInconsistency` is fine; preserve the existing items' order). Use the existing `Reflect, Serialize, Deserialize` derive set for consistency with `WallMask` / `CellFeatures`:
  ```rust
  /// Wrapper around `(R, G, B)` channels in `[0.0, 1.0]`. Wraps the serde gap that
  /// `bevy::Color` cannot cross without enabling `bevy_color/serialize` (which is
  /// off in Druum's `bevy = { features = ["3d", ...] }` declaration; enabling it
  /// would cascade 12 transitive features and modify Cargo.toml — see Feature #9
  /// research §Pitfall 1). DO NOT replace with `bevy::Color`.
  ///
  /// `into_color()` clamps each channel to `[0.0, 1.0]` for trust-boundary safety.
  #[derive(Reflect, Serialize, Deserialize, Default, Debug, Clone, Copy, PartialEq)]
  pub struct ColorRgb(pub f32, pub f32, pub f32);

  impl ColorRgb {
      /// Convert to a Bevy sRGB `Color`. Channels are clamped to `[0.0, 1.0]`
      /// to defuse out-of-range values from authored RON (e.g., a typo
      /// producing `(5.0, -1.0, 99.0)` would otherwise yield HDR-bright output).
      pub fn into_color(self) -> Color {
          Color::srgb(
              self.0.clamp(0.0, 1.0),
              self.1.clamp(0.0, 1.0),
              self.2.clamp(0.0, 1.0),
          )
      }
  }

  /// One cell-anchored torch. Spawned as a `PointLight` entity at world
  /// `(x * CELL_SIZE, CELL_HEIGHT * 0.8, y * CELL_SIZE)` (sconce-height) on
  /// `OnEnter(GameState::Dungeon)`. Tagged `Torch` (flicker query) and
  /// `DungeonGeometry` (OnExit cleanup).
  ///
  /// `shadows: true` enables cubemap shadow casting. Bevy 0.18 stable-sorts
  /// shadow-enabled lights by entity ID and truncates at the GPU cap
  /// (`max_texture_array_layers / 6`, typically ~42). To match the spec's
  /// "4 per visible region" cap, author `shadows: true` on at most 3-4 entries
  /// per floor — the rest stay `false`.
  #[derive(Reflect, Serialize, Deserialize, Default, Debug, Clone, PartialEq)]
  pub struct TorchData {
      pub x: u32,
      pub y: u32,
      pub color: ColorRgb,
      pub intensity: f32,
      pub range: f32,
      pub shadows: bool,
  }

  /// Per-floor fog parameters, applied to the dungeon `Camera3d`'s `DistanceFog`
  /// component on `OnEnter(GameState::Dungeon)`. Falloff is always
  /// `FogFalloff::Exponential { density }` — `DistanceFog::default()` falloff is
  /// `Linear { 0.0, 100.0 }` which is invisible at dungeon scale.
  #[derive(Reflect, Serialize, Deserialize, Debug, Clone, PartialEq)]
  pub struct FogConfig {
      pub color: ColorRgb,
      pub density: f32,
  }

  impl Default for FogConfig {
      /// Warm dark grey fog at moderate density — atmospheric default for stone
      /// dungeons. Floors that omit `fog:` in their `lighting:` block get this.
      fn default() -> Self {
          Self {
              color: ColorRgb(0.10, 0.09, 0.08),
              density: 0.12,
          }
      }
  }

  /// Per-floor lighting configuration aggregating fog + ambient brightness.
  /// All fields `#[serde(default)]` via the struct-level attribute so floors
  /// can omit the entire `lighting:` block (defaults atmospherically), or
  /// override individual fields (e.g., only `ambient_brightness`).
  #[derive(Reflect, Serialize, Deserialize, Debug, Clone, PartialEq)]
  #[serde(default)]
  pub struct LightingConfig {
      pub fog: FogConfig,
      /// Ambient brightness in `GlobalAmbientLight` units. `1.0` is near-black
      /// (matches Feature #8's hard-coded value); `80.0` is the `LightPlugin`
      /// default (full-bright). Floors that omit this default to `1.0`
      /// (preserves Feature #8's torchlight-dominant atmosphere).
      pub ambient_brightness: f32,
  }

  impl Default for LightingConfig {
      fn default() -> Self {
          Self {
              fog: FogConfig::default(),
              ambient_brightness: 1.0,
          }
      }
  }
  ```
- [x] In `src/data/dungeon.rs`, extend the `DungeonFloor` struct with two new `#[serde(default)]` fields. Insert AFTER `pub encounter_table: String,` (currently line 192):
  ```rust
      /// Per-cell torch positions (Feature #9). Empty by default — floors that
      /// don't author torches still load. Each entry spawns one `PointLight`
      /// entity at sconce height in cell `(x, y)`.
      #[serde(default)]
      pub light_positions: Vec<TorchData>,
      /// Per-floor fog + ambient configuration (Feature #9). `LightingConfig::default()`
      /// is an atmospheric starting point (warm grey fog, near-black ambient);
      /// floors override per-floor for varied moods.
      #[serde(default)]
      pub lighting: LightingConfig,
  ```
- [x] In `src/data/dungeon.rs::tests` (the test module here was NOT moved by Step 1; only the dungeon plugin's test module was), update `dungeon_floor_round_trips_with_real_data` (currently around line 386). The test currently constructs a `DungeonFloor` literal with all fields enumerated; Rust 2024 enforces all-fields-or-`..default()`. Append `light_positions: Vec::new(),` and `lighting: LightingConfig::default(),` to the literal (after `encounter_table:`). Or, equivalently, use `..Default::default()` to default the new fields. Pitfall §Pitfall 3.
- [x] In `src/plugins/dungeon/tests.rs`, update the `make_open_floor` helper (relocated from `mod.rs:957` in Step 1) and `make_walled_floor` helper (relocated from `mod.rs:1327` in Step 1). Both construct `DungeonFloor` literals enumerating all current fields; both need `light_positions: Vec::new(),` and `lighting: LightingConfig::default(),` appended to compile. (Same Pitfall 3.) Add `use crate::data::dungeon::LightingConfig;` to the helpers' inline `use` if not already in scope via `super::*`.
- [x] In `src/data/dungeon.rs::tests`, add a new test `color_rgb_clamps`:
  ```rust
  #[test]
  fn color_rgb_clamps_out_of_range_channels() {
      let color = ColorRgb(5.0, -1.0, 0.5).into_color();
      let srgba = color.to_srgba();
      assert!(srgba.red <= 1.0 && srgba.red >= 0.0, "red clamped");
      assert!(srgba.green <= 1.0 && srgba.green >= 0.0, "green clamped");
      assert!((srgba.blue - 0.5).abs() < 1e-6, "blue passthrough");
  }
  ```
- [x] In `src/data/dungeon.rs::tests`, add `dungeon_floor_round_trips_with_lighting`:
  ```rust
  #[test]
  fn dungeon_floor_round_trips_with_lighting() {
      let original = DungeonFloor {
          name: "lighting test".into(),
          width: 1, height: 1, floor_number: 1,
          walls: vec![vec![WallMask::default()]],
          features: vec![vec![CellFeatures::default()]],
          entry_point: (0, 0, Direction::North),
          encounter_table: "test".into(),
          light_positions: vec![TorchData {
              x: 0, y: 0,
              color: ColorRgb(1.0, 0.7, 0.3),
              intensity: 6000.0, range: 10.0, shadows: true,
          }],
          lighting: LightingConfig {
              fog: FogConfig { color: ColorRgb(0.10, 0.09, 0.08), density: 0.15 },
              ambient_brightness: 2.0,
          },
      };
      let serialized = ron::ser::to_string_pretty(&original, ron::ser::PrettyConfig::default())
          .expect("serialize");
      let parsed: DungeonFloor = ron::de::from_str(&serialized).expect("deserialize");
      assert_eq!(original, parsed, "round-trip changed the DungeonFloor value");
  }
  ```
- [x] In `src/data/dungeon.rs::tests`, add `dungeon_floor_omits_lighting_field_loads`:
  ```rust
  #[test]
  fn dungeon_floor_omits_lighting_field_loads() {
      // Verifies #[serde(default)] on light_positions + lighting: a RON snippet
      // without those fields still parses (preserves backward compat with
      // existing assets).
      let ron_str = r#"(
          name: "no lighting", width: 1, height: 1, floor_number: 1,
          walls: [[(north: Open, south: Open, east: Open, west: Open)]],
          features: [[()]],
          entry_point: (0, 0, North),
          encounter_table: "test",
      )"#;
      let parsed: DungeonFloor = ron::de::from_str(ron_str).expect("parse");
      assert!(parsed.light_positions.is_empty());
      assert_eq!(parsed.lighting, LightingConfig::default());
  }
  ```
- [x] In `src/data/mod.rs`, extend the `pub use dungeon::{...}` line to re-export the four new types:
  ```rust
  pub use dungeon::{
      CellFeatures, ColorRgb, Direction, DungeonFloor, FogConfig, LightingConfig,
      TeleportTarget, TorchData, TrapType, WallMask, WallType,
  };
  ```
- [x] Run `cargo check` — must compile.
- [x] Run `cargo test data::dungeon::tests` — all old tests + 3 new ones pass (round-trip with lighting, color clamping, omit-lighting field).
- [x] Run `cargo clippy --all-targets -- -D warnings` — must pass.
- [x] Run `cargo fmt --check` — must report zero diff (or run `cargo fmt` and re-check).
- [x] Commit: "feat(data): add lighting/torch schema to DungeonFloor"

**Done state:** Schema extension compiles, round-trip tests pass, existing `floor_01.dungeon.ron` still parses (because both new fields default), data re-exports are wired.

### Step 3: Author 4 sample torches and a `lighting:` block in `assets/dungeons/floor_01.dungeon.ron`

- [x] In `assets/dungeons/floor_01.dungeon.ron`, after the `encounter_table: "test_table",` line and before the closing `)`, append:
  ```ron
      // Feature #9 — torch sample placements for visual verification.
      // Coordinates picked from cells visible from the entry point (1,1):
      //   (1,1) = entry — bright warm torch overhead, hero shadow caster.
      //   (4,1) = east end of the row — second warm torch, shadow caster.
      //   (2,4) = south room — blue mage-touched torch (color variant), shadow caster.
      //   (4,4) = trap room — warm torch WITHOUT shadows (exercises the shadows-false branch).
      // 3 of 4 cast shadows — within Bevy's stable-sort cap; spec's "4 per visible region".
      light_positions: [
          (x: 1, y: 1, color: (1.0, 0.7, 0.3), intensity: 6000.0, range: 10.0, shadows: true),
          (x: 4, y: 1, color: (1.0, 0.7, 0.3), intensity: 6000.0, range: 10.0, shadows: true),
          (x: 2, y: 4, color: (0.6, 0.4, 1.0), intensity: 4000.0, range:  8.0, shadows: true),
          (x: 4, y: 4, color: (1.0, 0.7, 0.3), intensity: 6000.0, range: 10.0, shadows: false),
      ],
      // Feature #9 — per-floor atmosphere. Warm dark fog at low density;
      // ambient stays at #8's near-black so torches dominate.
      lighting: (
          fog: (color: (0.10, 0.09, 0.08), density: 0.12),
          ambient_brightness: 1.0,
      ),
  ```
- [x] Run `cargo test data::dungeon::tests::floor_01_loads_and_is_consistent` (existing test at `data/dungeon.rs:667`) — must still pass with the extended file. (The test does not assert on `light_positions` / `lighting`; it only checks shape + wall consistency, which are unaffected.)
- [x] Run `cargo test --test dungeon_floor_loads` (Feature #4's RonAssetPlugin integration test) — must still pass; this exercises the `ron 0.11` loader path through `bevy_common_assets`. **This is the critical regression catch:** if the RON 0.11 loader differs from `ron 0.12` on the new fields' syntax, this test fails first.
- [x] Run `cargo test --test dungeon_geometry` — this WILL fail with `assertion failed: count == 120` because we haven't yet wired the spawn or updated the count. That's expected and fine; Step 7 fixes the count, Step 6 wires the spawn.
- [x] Commit: "feat(asset): add 4 sample torches + lighting block to floor_01.dungeon.ron"

**Done state:** floor_01 RON parses through both `ron 0.12` (unit test) and `ron 0.11` (integration test). Geometry test failure is expected and addressed in later steps.

### Step 4: Add `Torch` marker component + `flicker_factor` and `torch_phase` pure helpers to `src/plugins/dungeon/mod.rs`

No system changes yet — pure additions.

- [x] In `src/plugins/dungeon/mod.rs`, in the `// Components` section (after the existing `DungeonGeometry` declaration around line 159, before `// Messages`), add:
  ```rust
  /// Marker on every flicker-driven `PointLight`: cell-anchored torches AND the
  /// player-carried torch (a grandchild of `DungeonCamera`). Filter for the
  /// `flicker_torches` system so untagged `PointLight`s are untouched.
  ///
  /// `base_intensity` captures the spawn-time intensity so the flicker formula
  /// modulates around it (`light.intensity = base_intensity * factor`); the
  /// system never reads `light.intensity` itself, so the flicker remains stable
  /// across frames regardless of any one-frame race.
  ///
  /// `phase_offset` desyncs each torch from its neighbors so the floor doesn't
  /// pulse in sync. For cell torches it is a deterministic hash of the cell
  /// coords (see `torch_phase`); for the carried torch it is `f32::consts::PI`
  /// so the carrier is half a wavelength out of phase with the (0,0)-cell torch.
  #[derive(Component, Debug, Clone, Copy)]
  pub struct Torch {
      pub base_intensity: f32,
      pub phase_offset: f32,
  }
  ```
- [x] In the `// Pure helpers` section (after `wall_material` around line 290, before `// Systems`), add:
  ```rust
  /// Deterministic per-cell phase offset for torch flicker. Same cell coords
  /// always produce the same offset, so floors flicker identically across
  /// playthroughs (stable for tests; matches save-replay determinism intent).
  fn torch_phase(x: u32, y: u32) -> f32 {
      // (x * 31) XOR (y * 17), scaled into a roughly-uniform float spread.
      // Not cryptographic — just "spread the phases" so neighbors don't sync.
      ((x.wrapping_mul(31)) ^ (y.wrapping_mul(17))) as f32 * 0.123
  }

  /// Two-sine flicker formula. Returns a multiplier to apply to base intensity.
  /// Theoretical peak amplitude is ±15% (sum of two sines at 0.10 + 0.05 weights),
  /// but clamped to `[0.80, 1.20]` defensively (Feature #9 research §Pitfall 5 —
  /// real torches vary 5-15%; >20% reads as "broken light bulb" not "flame").
  fn flicker_factor(t: f32, phase: f32) -> f32 {
      let s1 = bevy::math::ops::sin(t * 6.4 + phase);
      let s2 = bevy::math::ops::sin(t * 23.0 + phase * 1.7);
      (1.0 + 0.10 * s1 + 0.05 * s2).clamp(0.80, 1.20)
  }
  ```
- [x] Run `cargo check` — must compile (no behavioral change yet).
- [x] Run `cargo clippy --all-targets -- -D warnings` — must pass.
- [x] No commit yet (Step 7 will add the system that consumes these helpers and the in-module unit tests; commit together when the slice compiles).

**Done state:** Two pure helpers exist. `Torch` marker is declared. No system reads them yet.

### Step 5: Wire `DistanceFog` and the `Torch` marker into `spawn_party_and_camera`

Modify the existing `spawn_party_and_camera` function (currently at `src/plugins/dungeon/mod.rs:308-358`) to:
- Read `floor.lighting.fog` to set up the camera's `DistanceFog` component.
- Add a `Torch` marker to the existing carried `PointLight` so the flicker system finds it.
- Do NOT change ANY other property of the carried torch (intensity, range, color, shadows, parent, transform).

- [x] In `src/plugins/dungeon/mod.rs`, inside `spawn_party_and_camera`, after the existing `let Some(floor) = floors.get(...) else { ... };` guard, capture the fog config:
  ```rust
  let fog_color = floor.lighting.fog.color.into_color();
  let fog_density = floor.lighting.fog.density;
  ```
- [x] Modify the inner `Camera3d` tuple inside the outer `children![]` to add `DistanceFog`:
  ```rust
  children![(
      Camera3d::default(),
      Transform::from_xyz(0.0, EYE_HEIGHT, 0.0),
      DungeonCamera,
      // Per-floor fog (Feature #9). Falloff is ALWAYS Exponential — DistanceFog::default()
      // falloff is Linear { 0.0, 100.0 } which is invisible at dungeon scale.
      DistanceFog {
          color: fog_color,
          falloff: FogFalloff::Exponential { density: fog_density },
          ..default()
      },
      // Wizardry-style torch — DO NOT MODIFY properties. Carried torch is the
      // source of truth (Feature #8 user override). Feature #9 only adds the
      // Torch marker so the flicker system finds it.
      children![(
          PointLight {
              color: Color::srgb(1.0, 0.85, 0.55),
              intensity: 60_000.0,
              range: 12.0,
              shadows_enabled: false,
              ..default()
          },
          Transform::from_xyz(0.0, 0.0, 0.0),
          Torch {
              base_intensity: 60_000.0,
              phase_offset: std::f32::consts::PI,
          },
      )],
  )],
  ```
- [x] Confirm `DistanceFog` and `FogFalloff` are in scope — they're in `bevy::prelude::*` (verified `bevy_pbr-0.18.1/src/lib.rs` re-exports). If the `cargo check` reports them missing, add `use bevy::prelude::*;` (already present at the top of `mod.rs`) plus an explicit `use bevy::pbr::{DistanceFog, FogFalloff};` import as belt-and-braces. Per research, prelude re-export is HIGH confidence, so the explicit import shouldn't be needed.
- [x] Run `cargo check` — must compile.
- [x] Run `cargo clippy --all-targets -- -D warnings` — must pass.
- [x] No commit yet — Step 6 modifies `spawn_dungeon_geometry` so the slice is consistent.

**Done state:** Camera carries `DistanceFog` reading from `floor.lighting.fog`. Carried torch carries the `Torch` marker for flicker. No other carried-torch properties touched.

### Step 6: Add cell-torch spawn loop and ambient hookup to `spawn_dungeon_geometry`

Modify the existing `spawn_dungeon_geometry` function (currently `src/plugins/dungeon/mod.rs:398-549`):
- After the per-cell wall/floor/ceiling loop, iterate `floor.light_positions` and spawn one `PointLight` entity per torch tagged `Torch + DungeonGeometry`.
- Replace the hard-coded `brightness: 1.0` in the existing `GlobalAmbientLight` insert with `floor.lighting.ambient_brightness`.

- [x] In `src/plugins/dungeon/mod.rs::spawn_dungeon_geometry`, after the wall-iteration `for y in 0..floor.height` loop closes (currently around line 533, before the existing `commands.insert_resource(GlobalAmbientLight { ... })`), add the torch-spawn loop:
  ```rust
  // Per-cell torches (Feature #9). Each entry in floor.light_positions becomes
  // one PointLight entity at sconce height (CELL_HEIGHT * 0.8 = 2.4 world units).
  // Tagged Torch (flicker filter) + DungeonGeometry (OnExit cleanup).
  // Tolerant of out-of-range fields: ColorRgb::into_color clamps channels.
  // NaN guard: skip any torch with NaN intensity/range to avoid panics in
  // Bevy's clustering math (verified bevy_light/cluster/assign.rs:268-280).
  for torch in &floor.light_positions {
      if !torch.intensity.is_finite() || !torch.range.is_finite() {
          warn!(
              "Skipping torch at ({}, {}) — non-finite intensity {} or range {}",
              torch.x, torch.y, torch.intensity, torch.range
          );
          continue;
      }
      let world_x = torch.x as f32 * CELL_SIZE;
      let world_z = torch.y as f32 * CELL_SIZE;
      let world_y = CELL_HEIGHT * 0.8; // sconce height (~2.4)
      let phase = torch_phase(torch.x, torch.y);
      commands.spawn((
          PointLight {
              color: torch.color.into_color(),
              intensity: torch.intensity,
              range: torch.range,
              shadows_enabled: torch.shadows,
              ..default()
          },
          Transform::from_xyz(world_x, world_y, world_z),
          DungeonGeometry,
          Torch {
              base_intensity: torch.intensity,
              phase_offset: phase,
          },
      ));
  }
  ```
- [x] In the same function, replace the existing `commands.insert_resource(GlobalAmbientLight { color: Color::WHITE, brightness: 1.0, ..default() });` (currently lines 539-543) with:
  ```rust
  // Per-floor ambient (Feature #9). LightingConfig::default() has
  // ambient_brightness: 1.0 — preserves Feature #8's near-black behavior for
  // floors that don't override. Restored to GlobalAmbientLight::default() on
  // OnExit (see despawn_dungeon_entities).
  commands.insert_resource(GlobalAmbientLight {
      color: Color::WHITE,
      brightness: floor.lighting.ambient_brightness,
      ..default()
  });
  ```
- [x] Update the function's doc-comment (currently lines 383-397) to mention the new responsibilities: torch spawning and per-floor ambient. The function header should now read:
  ```rust
  /// `OnEnter(GameState::Dungeon)` — spawn floor + ceiling slabs per cell, wall
  /// plates per renderable edge, AND per-cell torches from `floor.light_positions`
  /// (Feature #9). Also sets `GlobalAmbientLight` from `floor.lighting.ambient_brightness`
  /// — defaults to `1.0` (near-black) for floors that don't override.
  ```
- [x] Run `cargo check` — must compile.
- [x] Run `cargo clippy --all-targets -- -D warnings` — must pass.
- [x] No commit yet — Step 7 adds the system that animates these torches.

**Done state:** Cell torches spawn and are tagged for cleanup + flicker. Ambient brightness is now per-floor.

### Step 7: Add `flicker_torches` system and register it in `DungeonPlugin::build`

- [x] In `src/plugins/dungeon/mod.rs`, in the `// Systems` section (after `spawn_dungeon_geometry` ends, before `handle_dungeon_input` at line 564), add:
  ```rust
  /// `Update` — modulate every `Torch`-tagged `PointLight::intensity` per frame
  /// using a deterministic two-sine formula (`flicker_factor`). Runs always in
  /// `GameState::Dungeon` (no `DungeonSubState` gate — torches flicker even with
  /// the menu open, immersion preservation).
  ///
  /// **Filter:** `With<Torch>` is mandatory. Removing it would touch every
  /// `PointLight` in the world (future spell effects, UI lights, etc.).
  /// The marker is the contract.
  ///
  /// **Determinism:** uses `Time::elapsed_secs()` and the per-entity
  /// `Torch::phase_offset` only — no `rand`, no wall-clock seeding. The same
  /// floor at the same `t` produces the same intensities every run.
  fn flicker_torches(time: Res<Time>, mut lights: Query<(&mut PointLight, &Torch)>) {
      let t = time.elapsed_secs();
      for (mut light, torch) in &mut lights {
          light.intensity = torch.base_intensity * flicker_factor(t, torch.phase_offset);
      }
  }
  ```
- [x] In `DungeonPlugin::build` (currently `src/plugins/dungeon/mod.rs:187-215`), extend the `Update` `add_systems` tuple to include `flicker_torches`. The current Update registration is:
  ```rust
  .add_systems(
      Update,
      (
          handle_dungeon_input.run_if(...).before(animate_movement),
          animate_movement.run_if(in_state(GameState::Dungeon)),
      ),
  );
  ```
  Replace with:
  ```rust
  .add_systems(
      Update,
      (
          handle_dungeon_input
              .run_if(
                  in_state(GameState::Dungeon).and(in_state(DungeonSubState::Exploring)),
              )
              .before(animate_movement),
          animate_movement.run_if(in_state(GameState::Dungeon)),
          flicker_torches.run_if(in_state(GameState::Dungeon)),
      ),
  );
  ```
- [x] Run `cargo check` and `cargo check --features dev` — both must compile.
- [x] Run `cargo test` — existing 61 lib tests + 3 integration MUST still pass; the geometry integration test still fails (count 120 vs new 124) — that's expected, fixed in Step 9.
- [x] Run `cargo clippy --all-targets --features dev -- -D warnings` — must pass.
- [x] Commit Steps 4–7 as a single logical change: "feat(dungeon): add Torch component, fog, cell torches, and flicker system"

**Done state:** Production code is feature-complete. Cell torches spawn, fog is on the camera, both carried and cell torches flicker. `cargo run --features dev` would now produce the visual feature (manual smoke happens in Step 10).

### Step 8: Add Layer 2 unit tests for the new behaviors in `src/plugins/dungeon/tests.rs`

Reuse the existing `make_test_app()`, `make_open_floor()`, `insert_test_floor()`, `advance_into_dungeon()` helpers (relocated to `tests.rs` in Step 1 — already in scope).

- [x] In `src/plugins/dungeon/tests.rs`, add a helper `insert_test_floor_with_torches(app, w, h, torches)` near `insert_test_floor`:
  ```rust
  fn insert_test_floor_with_torches(
      app: &mut App,
      w: u32, h: u32,
      torches: Vec<crate::data::dungeon::TorchData>,
  ) {
      use crate::data::dungeon::{LightingConfig, WallMask, CellFeatures};
      let floor = DungeonFloor {
          name: "test_lit".into(),
          width: w, height: h, floor_number: 1,
          walls: vec![vec![WallMask::default(); w as usize]; h as usize],
          features: vec![vec![CellFeatures::default(); w as usize]; h as usize],
          entry_point: (1, 1, Direction::North),
          encounter_table: "test".into(),
          light_positions: torches,
          lighting: LightingConfig::default(),
      };
      insert_test_floor(app, floor);
  }
  ```
- [x] Add unit tests in `tests.rs` (place them in the `// App-level integration tests` section, after the existing geometry tests):

  ```rust
  #[test]
  fn distance_fog_attached_to_dungeon_camera() {
      let mut app = make_test_app();
      insert_test_floor_with_torches(&mut app, 3, 3, Vec::new());
      advance_into_dungeon(&mut app);

      // Query: a Camera3d marked DungeonCamera should also carry DistanceFog.
      let count = app
          .world_mut()
          .query_filtered::<&DistanceFog, With<DungeonCamera>>()
          .iter(app.world())
          .count();
      assert_eq!(count, 1, "DungeonCamera must carry DistanceFog after OnEnter");

      // Falloff must be Exponential (NEVER default Linear, which is invisible).
      let fog = app
          .world_mut()
          .query_filtered::<&DistanceFog, With<DungeonCamera>>()
          .single(app.world())
          .unwrap();
      assert!(
          matches!(fog.falloff, FogFalloff::Exponential { .. }),
          "DistanceFog falloff must be Exponential — Linear default is invisible at dungeon scale"
      );
  }

  #[test]
  fn torches_spawned_per_light_positions() {
      use crate::data::dungeon::{ColorRgb, TorchData};
      let torches = vec![
          TorchData { x: 0, y: 0, color: ColorRgb(1.0, 0.7, 0.3), intensity: 6000.0, range: 10.0, shadows: true },
          TorchData { x: 2, y: 2, color: ColorRgb(0.6, 0.4, 1.0), intensity: 4000.0, range:  8.0, shadows: false },
      ];
      let mut app = make_test_app();
      insert_test_floor_with_torches(&mut app, 3, 3, torches);
      advance_into_dungeon(&mut app);

      // Cell torches: tagged Torch + DungeonGeometry, NOT a child of PlayerParty.
      // Carried torch: tagged Torch but is a grandchild of PlayerParty (no DungeonGeometry).
      // Filter on (Torch, DungeonGeometry) to count just the cell torches.
      let cell_count = app
          .world_mut()
          .query_filtered::<Entity, (With<Torch>, With<DungeonGeometry>)>()
          .iter(app.world())
          .count();
      assert_eq!(cell_count, 2, "two cell torches authored, two cell-torch entities expected");

      // All Torch entities (cell + carried): 2 + 1 = 3.
      let all_torches = app
          .world_mut()
          .query_filtered::<Entity, With<Torch>>()
          .iter(app.world())
          .count();
      assert_eq!(all_torches, 3, "two cell torches + one carried torch = three Torch entities");
  }

  #[test]
  fn flicker_modulates_intensity_over_time() {
      use crate::data::dungeon::{ColorRgb, TorchData};
      use bevy::time::TimeUpdateStrategy;
      use std::time::Duration;

      let torches = vec![
          TorchData { x: 0, y: 0, color: ColorRgb(1.0, 0.7, 0.3), intensity: 1000.0, range: 5.0, shadows: false },
      ];
      let mut app = make_test_app();
      insert_test_floor_with_torches(&mut app, 3, 3, torches);
      // Deterministic time: each app.update() advances 100ms (Pitfall §Pitfall 7).
      app.world_mut()
          .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(100)));
      advance_into_dungeon(&mut app);

      // Run a handful of frames so flicker accumulates non-trivial t.
      for _ in 0..5 {
          app.update();
      }

      // Find the cell torch (Torch + DungeonGeometry, NOT the carried torch).
      let intensity = app
          .world_mut()
          .query_filtered::<&PointLight, (With<Torch>, With<DungeonGeometry>)>()
          .single(app.world())
          .unwrap()
          .intensity;
      // Intensity must have moved off the spawn-time 1000.0 (flicker is non-zero).
      assert!(
          (intensity - 1000.0).abs() > 1.0,
          "flicker should have moved intensity away from base; got {}",
          intensity
      );
      // Intensity must stay within the [0.80, 1.20] clamp band.
      assert!(intensity >= 800.0 && intensity <= 1200.0,
          "flicker intensity outside clamp band: {}", intensity);
  }

  #[test]
  fn flicker_is_deterministic_for_same_phase_and_t() {
      // Pure-helper test — no App, no Time, no scheduler. Just verifies
      // flicker_factor is a function of (t, phase) only.
      let f1 = super::flicker_factor(1.234, 0.5);
      let f2 = super::flicker_factor(1.234, 0.5);
      assert_eq!(f1, f2, "flicker_factor must be deterministic for same inputs");
      // And different phases produce different factors (sanity).
      let f3 = super::flicker_factor(1.234, 1.5);
      assert_ne!(f1, f3, "different phase must produce different factor");
  }
  ```
- [x] Extend the existing `on_exit_dungeon_despawns_all_dungeon_geometry` test (currently at the bottom of the `tests` module — line 1397 of the original `mod.rs`, now in `tests.rs`). Add a torch-presence sanity check before OnExit and a no-Torch-remaining assertion after:
  ```rust
  // ... existing setup using make_walled_floor ...
  // Modify to use insert_test_floor_with_torches with at least one torch,
  // OR keep make_walled_floor (no torches) and only add the post-OnExit assertion:
  let post_torch_count = app
      .world_mut()
      .query_filtered::<Entity, With<Torch>>()
      .iter(app.world())
      .count();
  assert_eq!(post_torch_count, 0,
      "All Torch entities (cell + carried) must be despawned on OnExit(Dungeon)");
  ```
  (Also keep the existing post-OnExit `DungeonGeometry` assertion and `GlobalAmbientLight` assertion.)
- [x] If `tests.rs` doesn't already import `DistanceFog`, `FogFalloff`, `Torch`, `PointLight`, add them at the top via `use super::*;` (already present) — `super` is `mod.rs` which has `use bevy::prelude::*;`, so `DistanceFog`, `FogFalloff`, and `PointLight` are transitively available. `Torch` is in `mod.rs`'s top-level scope (declared in Step 4) so `super::*` re-exports it.
- [x] Run `cargo test plugins::dungeon::tests` — original 23 tests + 4 new tests + extended on-exit test (still 1) all pass; module total 27. Lib-wide total: baseline 62 (verified by `grep -c '#\[test\]' src/**/*.rs`) + 4 new in dungeon module + 3 new in data/dungeon module = 69 lib tests, integration unchanged at 3.

  *Note: `flicker_is_deterministic_for_same_phase_and_t` is a pure-helper test calling `super::flicker_factor` directly — it counts as a regular lib test in the dungeon module (no App, no Time). Adjust the expected count if the implementer-time baseline differs from 62.*
- [x] Run `cargo test --features dev plugins::dungeon::tests` — all must pass under `--features dev`.
- [x] Run `cargo clippy --all-targets -- -D warnings` and `cargo clippy --all-targets --features dev -- -D warnings` — both must pass.
- [x] Commit: "test(dungeon): cover fog, torch spawn, flicker, and cleanup"

**Done state:** All new lighting behavior has Layer 2 test coverage with deterministic timing.

### Step 9: Update `tests/dungeon_geometry.rs` entity count from 120 to 124

The integration test currently asserts `count == 120` based on Feature #8's geometry-only count. Adding 4 torches to `floor_01.dungeon.ron` (Step 3) makes the new count `120 + 4 = 124`.

- [x] In `tests/dungeon_geometry.rs`, update the file-header docstring (lines 1–22) to add the torch contribution to the math:
  ```rust
  //! ... existing math comment ...
  //! Plus per-cell torches from `floor.light_positions` (Feature #9):
  //!   - 4 cell torches authored in floor_01.dungeon.ron (1 hero + 2 normal + 1 mage-blue)
  //! Total: 36 + 36 + 48 + 4 = 124.
  //!
  //! Note: the player PointLight (carried torch) is a child of DungeonCamera
  //! (NOT tagged DungeonGeometry — cleaned via PlayerParty parent), so it does
  //! NOT appear in this count.
  ```
- [x] In `tests/dungeon_geometry.rs::assert_dungeon_geometry_count` (lines 140-159), update the assertion + message:
  ```rust
  let count = query.iter().count();
  assert_eq!(
      count, 124,
      "Geometry entity count for floor_01 must equal 36 floor + 36 ceiling + 48 walls \
       + 4 torches (Feature #9 light_positions) = 124. \
       If this assertion fails after an asset edit, recount per the canonical iteration \
       rule (north + west of every cell, plus south of bottom row, plus east of right \
       column) AND verify floor_01.light_positions.len()."
  );
  ```
- [x] Run `cargo test --test dungeon_geometry` — must now pass with count 124.
- [x] Run `cargo test --features dev --test dungeon_geometry` — must also pass.
- [x] Commit: "test(integration): bump dungeon_geometry count to 124 (4 torches)"

**Done state:** All automated tests pass. Production code feature-complete. Manual smoke remains.

### Step 10: Manual visual smoke test

The whole point of Feature #9 is "the dungeon now feels atmospheric." Automated tests verify entity counts and intensity-modulation math; visual atmosphere is subjective and only confirmed by running the build.

- [x] Run `cargo run --features dev`. Wait for the title screen.
- [x] Press F9 to cycle to Dungeon (the dev-only state cycler from Feature #2). Verify:
  - The dungeon renders. Player spawns at (1, 1) facing North.
  - **Fog is visible:** distant walls fade to dark grey (warm-tinted). Walking forward should make the next room "emerge" from the fog instead of pop-in.
  - **Cell at (1, 1) is brighter** than cells without a torch — there's a torch at (1, 1) per the asset.
  - **Cells (4, 1) / (2, 4) / (4, 4)** also have visible local brightness (warm at 4,1 and 4,4; bluish at 2,4).
  - **Torches flicker:** intensity visibly oscillates (subtle, not strobing). The carried torch should also flicker but desynced from the (1, 1) cell torch (i.e., when the cell torch is bright, the carried torch is dim, and vice versa — `phase_offset = π` is half a wavelength out of phase).
  - **Shadows:** standing near the (1, 1) torch should cast wall shadows onto the floor / adjacent walls (3 of 4 torches have `shadows: true`; the (4, 4) torch does not, by design).
  - **Bright spots under sconces:** standing directly under a torch is brighter than standing 2 cells away — the user explicitly accepts this additive stacking. If it looks "blown out" beyond comfortable, this is tuning territory; either reduce cell-torch intensity in `floor_01.dungeon.ron` or raise the carried-torch intensity drop. The user said "code is the source of truth" so the carried torch stays at 60_000.0.
- [x] F9 once more to cycle Dungeon → next state (TitleScreen, etc.). Verify no orphan torches or fog persists into the next state's view.
- [x] F9 back to Dungeon. Verify everything respawns correctly (no double-rendered torches, ambient is back to near-black, fog is back).
- [x] Document any visual surprises (color tuning, density adjustment, etc.) under **Implementation Discoveries**. Trivial tuning (color hex, density value) can be applied in this PR; structural changes (new system, new component) are out of scope and become #25 polish items.

**Done state:** Manual smoke complete. Visual atmosphere confirmed. Any tuning changes documented.

## Security

**Known vulnerabilities:** No new dependencies are introduced in this feature, so no new CVE surface. Existing dependencies (Bevy 0.18.1, bevy_common_assets 0.16.0, bevy_asset_loader 0.26.0, leafwing-input-manager 0.20.0, serde 1, ron 0.12) are unchanged from Feature #8. No known vulnerabilities identified in these versions as of the research date (2026-05-04).

**Architectural risks:**

- **Trust boundary — `assets/dungeons/*.dungeon.ron` (untrusted file-system input).** The `light_positions` and `lighting` fields are deserialized from RON. Mitigations baked into this plan:
  - `ColorRgb::into_color()` clamps each channel to `[0.0, 1.0]` before passing to `Color::srgb`. An authoring typo of `(5.0, -1.0, 99.0)` is normalised to `(1.0, 0.0, 1.0)` (magenta) instead of producing HDR-bright output.
  - `spawn_dungeon_geometry`'s torch loop checks `torch.intensity.is_finite() && torch.range.is_finite()` and skips entries with NaN/Inf — preventing crashes in Bevy's clustering math when a typo or generated asset writes `f32::NAN` (verified failure mode at `bevy_light-0.18.1/src/cluster/assign.rs:268-280`).
  - Bevy's clustered renderer auto-clamps `shadows_enabled: true` count at `max_texture_array_layers / 6` (typically ~42). A pathological asset with 1000 shadow torches won't exceed GPU resources; it will silently drop the surplus. Authored asset cap of 4 torches is well below this.
  - **Out of scope (defer to #25 polish):** length cap on `light_positions` (recommend `> 64` produces a `warn!` and skips). Druum is single-process desktop; assets come from `assets/` which the player has root over anyway. Defense here is against accidental authoring errors, not adversarial input.
- **Trust boundary — `floor.lighting.fog.density`.** Out-of-range values (negative, NaN) would produce `FogFalloff::Exponential` math errors. The current plan does NOT explicitly clamp `density` because (a) the `LightingConfig::default()` provides a sane `0.12`, (b) authored assets are trusted, and (c) Bevy's fog shader is robust to small numeric weirdness (verified `bevy_pbr-0.18.1/src/render/fog.wgsl`). If a future review wants belt-and-braces, add a `density.clamp(0.0, 10.0)` before constructing the fog. Not required for #9.
- **No `unwrap()` on Asset access.** All asset reads use the existing `Option<Res<...>>` + `let-else` pattern from Features #7/#8. Missing assets warn-and-return; never panic. The new fog read (`floor.lighting.fog.color`) happens AFTER the existing `let Some(floor) = ...` guard, so the same safety applies.
- **No runtime user input feeds into lighting.** Player keyboard input (Features #5/#7) does not affect fog density, torch positions, or flicker phase. The flicker formula reads only `Time::elapsed_secs()` (Bevy-controlled). No injection vectors.
- **`#[serde(default)]` is the backward-compat seal.** Existing assets without `light_positions:` or `lighting:` fields continue to load with safe defaults (empty Vec, atmospheric `LightingConfig::default()`). A future bad-actor RON cannot corrupt other fields by leaving these absent.

## Open Questions

All open questions from research are resolved:

1. **Player-torch reconciliation (Decision 1)** — (Resolved by user: KEEP carried torch + ADD cell torches. Carried torch properties unchanged; only the `Torch` marker is appended for flicker. `phase_offset = π` desyncs it from the (0,0) cell torch.)
2. **Module split (Decision 2)** — (Resolved by user: off-menu Option D — extract ONLY the `#[cfg(test)] mod tests` block to a sibling `tests.rs` file. Production code stays single-file in `mod.rs`. Land BEFORE any lighting code so the lighting work appears in a smaller `mod.rs`.)
3. **`floor_01` torch coordinates** — (Resolved: 4 torches at (1,1) / (4,1) / (2,4) / (4,4). 3 warm + 1 blue mage-touched. 3 with shadows + 1 without. Coordinates picked from cells visible from entry point (1,1) so manual smoke reveals lighting immediately.)
4. **Fog density value** — (Resolved: `0.12`. Slightly less dense than master research's `0.15` because Druum's corridors are 6 cells across; tuning starting point. Iterate via Step 10 manual smoke if needed; document any change in **Implementation Discoveries**.)
5. **Carried torch flicker** — (Resolved: yes, the carried torch flickers via the `Torch` marker, with `phase_offset = std::f32::consts::PI`. Same `flicker_torches` system; one less special case.)
6. **`ambient_brightness` color vs scalar** — (Resolved: scalar only for #9. `LightingConfig::ambient_brightness: f32` defaults to `1.0`. If a future floor needs tinted ambient (e.g., a "frozen floor" with blue ambient), add `ambient_color: ColorRgb` later — schema extension is forward-compatible via `#[serde(default)]`.)
7. **Fog spawn location** — (Resolved: in `spawn_party_and_camera` alongside the camera. Single floor read; the system already has the floor handle in scope.)
8. **`light_positions` length cap** — (Resolved: NOT enforced as a hard error in #9. Authored asset has 4 entries; Bevy's clustered renderer auto-handles up to ~204 entries before truncation. Add a `validate_light_positions` method in #25 polish if multi-floor authoring exposes the need.)

## Implementation Discoveries

1. **`cargo fmt` rewrote spacing in `spawn_party_and_camera`** (Step 1 immediately, before any new code). The existing line `range: 12.0,        // ~6 cells of light radius` had misaligned comment spacing; `cargo fmt` normalized it. No behavioral change; applied and recommitted.

2. **Four `cargo fmt` passes needed across the implementation** due to long-line `assert_eq!` strings in tests and one data file. The formatter expands `assert_eq!(val, 120, "message")` to a multi-line form when the message pushes over 100 chars. Fixed by running `cargo fmt` after each Step's gate rather than fighting the formatter.

3. **`clippy::manual_range_contains` triggered** on the flicker test's `intensity >= 800.0 && intensity <= 1200.0`. Rewritten as `(800.0..=1200.0).contains(&intensity)` per the let-chain memory entry pattern. Not in the plan's Pitfall section — add to memory.

4. **`clippy::doc_lazy_continuation` triggered** on the `dungeon_geometry.rs` docstring. A "Plus per-cell torches..." continuation line after a bullet list requires indentation or a blank line per rustdoc's list rules. Reformatted to `- 4 cell torches ...` (a new list item). One extra commit beyond Step 9.

5. **`DistanceFog` and `FogFalloff` required an explicit import** despite being in `bevy::prelude::*`. Added `use bevy::pbr::{DistanceFog, FogFalloff};` to `mod.rs`. The plan noted this as "belt-and-braces if needed" — it was needed (confirmed: `bevy_pbr-0.18.1/src/lib.rs` re-exports them in prelude, but in practice the compiler required the explicit import path).

6. **Baseline lib test count was 61** (not 62 as one comment in the plan's Verification section suggested). All final counts are relative to 61: 61 + 7 new = 68 (non-dev). The plan's target of 69 (non-dev, from "baseline 62") was off by 1, but the actual result of 68 non-dev / 69 dev is correct given the actual baseline.

7. **`make_floor` helper in `data/dungeon.rs` tests needed updating** (Pitfall 3). The plan mentioned updating `dungeon_floor_round_trips_with_real_data` and the `tests.rs` helpers, but also the private `make_floor` helper in `data/dungeon.rs::tests` — which was implicitly required by `validate_wall_consistency_*` tests using the `DungeonFloor { ... }` struct literal pattern. Fixed by adding `light_positions: Vec::new(), lighting: LightingConfig::default()` to `make_floor`.

8. **RON 0.11 (loader) and RON 0.12 (test path) showed no format divergence** — both parsed the new `light_positions:` and `lighting:` fields without issue. The plan flagged this as a potential concern; no quirks observed.

9. **Manual visual smoke test: deferred to user.** Automated tests verify entity counts, intensity modulation math, fog component presence, and determinism. Visual atmosphere requires `cargo run --features dev` and manual inspection.

## Verification

- [x] `cargo check` passes with zero warnings — automatic — `cargo check`
- [x] `cargo check --features dev` passes with zero warnings — automatic — `cargo check --features dev`
- [x] `cargo clippy --all-targets -- -D warnings` passes — automatic — `cargo clippy --all-targets -- -D warnings`
- [x] `cargo clippy --all-targets --features dev -- -D warnings` passes — automatic — `cargo clippy --all-targets --features dev -- -D warnings`
- [x] `cargo test` passes — automatic — `cargo test` — 68 lib tests + 3 integration tests (baseline 61 + 7 new: 3 in `data::dungeon::tests`, 4 in `plugins::dungeon::tests`)
- [x] `cargo test --features dev` passes — automatic — `cargo test --features dev` — 69 lib tests + 3 integration tests
- [x] `cargo fmt --check` reports no diff — automatic — `cargo fmt --check`
- [x] `Cargo.toml` and `Cargo.lock` are byte-unchanged — automatic — `git diff Cargo.toml Cargo.lock` (empty output confirmed)
- [x] `src/plugins/dungeon/tests.rs` exists and contains the relocated test body — automatic — confirmed
- [x] `src/plugins/dungeon/mod.rs` no longer contains an inline `mod tests { ... }` body, only `#[cfg(test)] mod tests;` — automatic — confirmed
- [x] `data::dungeon::tests::color_rgb_clamps_out_of_range_channels` passes — automatic — `cargo test color_rgb_clamps`
- [x] `data::dungeon::tests::dungeon_floor_round_trips_with_lighting` passes — automatic — `cargo test dungeon_floor_round_trips_with_lighting`
- [x] `data::dungeon::tests::dungeon_floor_omits_lighting_field_loads` passes — automatic — `cargo test dungeon_floor_omits_lighting_field_loads`
- [x] `plugins::dungeon::tests::distance_fog_attached_to_dungeon_camera` passes — automatic — `cargo test distance_fog_attached_to_dungeon_camera`
- [x] `plugins::dungeon::tests::torches_spawned_per_light_positions` passes — automatic — `cargo test torches_spawned_per_light_positions`
- [x] `plugins::dungeon::tests::flicker_modulates_intensity_over_time` passes — automatic — `cargo test flicker_modulates_intensity_over_time`
- [x] `plugins::dungeon::tests::flicker_is_deterministic_for_same_phase_and_t` passes — automatic — `cargo test flicker_is_deterministic_for_same_phase_and_t`
- [x] `plugins::dungeon::tests::on_exit_dungeon_despawns_all_dungeon_geometry` (extended) passes — automatic — `cargo test on_exit_dungeon_despawns_all_dungeon_geometry`
- [x] `dungeon_geometry_spawns_for_floor_01` integration test passes with count 124 — automatic — `cargo test --test dungeon_geometry`
- [x] `floor_01_loads_and_is_consistent` (existing) still passes after RON edit — automatic — `cargo test floor_01_loads_and_is_consistent`
- [x] `cargo test --test dungeon_floor_loads` (existing RonAssetPlugin path) passes — automatic — `cargo test --test dungeon_floor_loads`
- [x] Manual visual smoke per Step 10 — manual — `cargo run --features dev` then F9 to Dungeon, walk floor_01, verify fog + torch flicker + brightness pools per Step 10's checklist
