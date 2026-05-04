# Review: Feature #9 — Dungeon Lighting & Atmosphere (PR #9)

**Date:** 2026-05-04
**Verdict:** APPROVE WITH NITS
**PR:** https://github.com/codeinaire/druum-dungeon-crawler/pull/9
**Branch:** `9-dungeon-lighting-atmosphere` → `main`

## Behavioral Delta

After this PR merges, the dungeon acquires three atmospheric changes:

1. **Fog:** The `DungeonCamera`'s `Camera3d` carries a `DistanceFog` with `Exponential { density: 0.12 }` falloff, colour read from `floor.lighting.fog` at OnEnter time. Distant corridor walls now fade into warm dark grey.
2. **Flicker:** The carried `PointLight` (intensity 60_000, range 12.0, srgb(1.0, 0.85, 0.55) — properties unchanged from #8) gains a `Torch { base_intensity: 60_000.0, phase_offset: PI }` marker. The new `flicker_torches` Update system modulates every `Torch`-tagged light using a two-sine formula clamped to [0.80, 1.20] × base_intensity, running when `GameState::Dungeon`.
3. **Per-floor ambient:** The hardcoded `brightness: 1.0` in `GlobalAmbientLight` is now sourced from `floor.lighting.ambient_brightness` (defaults to `1.0`). `floor_01.dungeon.ron` explicitly sets `ambient_brightness: 1.0`, preserving #8's near-black atmosphere.

Additionally: the 664-LOC test body is relocated from inline `mod tests { ... }` in `mod.rs` to a sibling `src/plugins/dungeon/tests.rs`. Production code is unchanged by this move.

Net: no cell-anchored torches, no `TorchData`/`light_positions`, no `torch_phase` helper. The followup commit (`b9a7e46`) fully stripped these before the PR was opened. The integration test count stays at 120 (not 124).

---

## What Was Reviewed

**Files with full review:**
- `src/data/dungeon.rs` — 4 new types (`ColorRgb`, `FogConfig`, `LightingConfig`) + 1 new `DungeonFloor` field (`lighting`) + 3 new unit tests
- `src/data/mod.rs` — re-export additions
- `src/plugins/dungeon/mod.rs` — `Torch` component, `flicker_factor` helper, `flicker_torches` system, fog hookup in `spawn_party_and_camera`, ambient hookup in `spawn_dungeon_geometry`
- `src/plugins/dungeon/tests.rs` — relocated test body + 2 new lighting tests (`distance_fog_attached_to_dungeon_camera`, `flicker_is_deterministic_for_same_phase_and_t`) + extended `on_exit_dungeon_despawns_all_dungeon_geometry`
- `assets/dungeons/floor_01.dungeon.ron` — added `lighting:` block
- `tests/dungeon_geometry.rs` — doc-comment update only; count remains 120

**Hard gates verified:**
- Cargo.toml SHA `a1d9078b20b2f7e8851019079bf9e121624f15cd` identical on branch and `main` — PASS (Δ deps = 0)
- No `TorchData`, `light_positions`, or `torch_phase` in net state — confirmed absent
- `DistanceFog` always specifies `FogFalloff::Exponential { density }` explicitly — never relies on `..default()` for the falloff
- Carried-torch properties unchanged: `intensity: 60_000.0`, `range: 12.0`, `color: srgb(1.0, 0.85, 0.55)`, `shadows_enabled: false`, `Transform::from_xyz(0.0, 0.0, 0.0)` — all sacrosanct per #8 user override
- `impl Default for FogConfig` returns `density: 0.12`, `color: ColorRgb(0.10, 0.09, 0.08)` — NOT derive Default (which would give 0.0/black)
- `impl Default for LightingConfig` returns `ambient_brightness: 1.0` — NOT derive Default (which would give 0.0)
- `ColorRgb::into_color` clamps all three channels to `[0.0, 1.0]`
- `FogConfig` omits `#[derive(Default)]`; has explicit `impl Default` — correct
- `LightingConfig` omits `#[derive(Default)]`; has explicit `impl Default` — correct; `#[serde(default)]` on the struct allows partial overrides
- `#[cfg(test)] mod tests;` in `mod.rs` (single declaration, no inline body) — correct
- `tests.rs` opens with `use super::*;` — all parent items remain accessible

---

## Findings

### [MEDIUM] Integration test doc-comment is stale after cell-torch strip

**File:** `tests/dungeon_geometry.rs:1-24` (doc-comment header) and lines 145-149 (assertion message)

**Issue:** The module doc-comment was updated to remove the `+ 4 torches` line from the count derivation (correctly), but the assertion message at the bottom of `assert_dungeon_geometry_count` still reads:

```rust
assert_eq!(
    count, 120,
    "Geometry entity count for floor_01 must equal 36 floor + 36 ceiling + 48 walls = 120. \
     If this assertion fails after an asset edit, recount per the canonical iteration rule \
     (north + west of every cell, plus south of bottom row, plus east of right column)."
);
```

The assertion value (120) is correct for the net diff — no torches in `DungeonGeometry` because cell torches were stripped. However, the module-level doc-comment says:

```
//! Verifies that `spawn_dungeon_geometry` correctly spawns 120 entities tagged
//! with `DungeonGeometry`
```

This is coherent. The actual inconsistency is milder: the implementation summary and the planner memory still reference count 124, but the shipped code and test are 120. This is an artifact of the strip commit — no code bug, but the plan/implemented artifacts mention 124 in several places while the test says 120. The plan-level documents are project artifacts, not shipped code, so this doesn't affect correctness. The test itself is self-consistent.

The genuine concern is that the module doc-comment's bullet-point breakdown:
```
//!       * 14 north walls renderable ...
//!       * 22 west walls renderable ...
//!       *  6 south walls ...
//!       *  6 east walls
```
... is correct for 48 walls, and the total (120) is correct. No action strictly required, but the doc leaves a reader wondering "did the torch strip reduce this from 124 to 120?" without any note explaining the correction. A brief note like "Note: cell-torch entities were removed in the Feature #9 strip commit; this count is geometry only." would clarify intent for future reviewers.

**Fix (optional):** Add a comment to the doc-header:

```rust
//! Note: Feature #9 added per-cell torch entities during development but
//! removed them before merge (scope reduction). The carried torch is a child
//! of DungeonCamera and is not tagged DungeonGeometry. Count remains 120.
```

---

### [MEDIUM] `dungeon_floor_round_trips_with_lighting` test uses `TorchData`-era schema in the plan but actual test does not include `light_positions`

**File:** `src/data/dungeon.rs` — the `dungeon_floor_round_trips_with_lighting` test

**Issue:** This is a positive observation rather than a bug: the test correctly constructs a `DungeonFloor` with a `lighting:` block but without a `light_positions:` field (because `TorchData` and `light_positions` were stripped). The round-trip test exercises `LightingConfig { fog: FogConfig { density: 0.15 }, ambient_brightness: 2.0 }`.

However, the test exercises `density: 0.15` as a round-trip value, while `FogConfig::default()` returns `density: 0.12`. This is intentional — round-trip tests should use non-default values to verify serde actually writes and re-reads the non-default state. This is correct.

**No action required.** Noting for completeness.

---

### [LOW] `Torch` doc-comment references "cell-anchored torches" that no longer exist in this PR

**File:** `src/plugins/dungeon/mod.rs`, the `Torch` component doc-comment

**Issue:** The doc-comment reads:

```rust
/// Marker on the player-carried `PointLight` (a grandchild of `DungeonCamera`).
/// Filter for the `flicker_torches` system so untagged `PointLight`s are
/// untouched.
///
/// `phase_offset` is `f32::consts::PI` for the carried torch — chosen so it
/// stays out of sync with any future cell-anchored torches added later.
```

The phrase "any future cell-anchored torches added later" is accurate (the roadmap does plan cell torches in a later feature), and the `phase_offset = PI` choice was specifically made to desync from them. This is correct and intentional — noted in the brief as "the `phase_offset = π` choice for the carried torch — specifically chosen so the carried torch desyncs from any future cell-anchored torches that might be added later."

**No action required.** The doc-comment is forward-looking and accurate.

---

### [LOW] `flicker_torches` has no test for the intensity-modulation end-to-end path

**File:** `src/plugins/dungeon/tests.rs`

**Issue:** The two new lighting tests are:
1. `distance_fog_attached_to_dungeon_camera` — asserts `DistanceFog` with `Exponential` falloff on `DungeonCamera`. Strong test.
2. `flicker_is_deterministic_for_same_phase_and_t` — pure-function test on `flicker_factor`. Correct and fast.

What's absent is a test that exercises `flicker_torches` end-to-end: advance time via `TimeUpdateStrategy::ManualDuration`, call `app.update()`, query the `PointLight.intensity`, assert it moved away from `base_intensity`. The `flicker_modulates_intensity_over_time` test that appeared in the Step 8 plan was removed during the cell-torch strip because it required cell torches to have a second `Torch` entity with a different phase.

The carried torch alone is sufficient to run this test:

```rust
#[test]
fn flicker_torches_modulates_carried_torch_intensity() {
    use std::time::Duration;
    let mut app = make_test_app();
    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(100)));
    insert_test_floor(&mut app, make_open_floor(3, 3));
    advance_into_dungeon(&mut app);

    // Advance time so elapsed_secs > 0 (flicker_factor(0.0, PI) == 1.0 exactly
    // because sin(0) = 0, so t=0 would not catch a bug where flicker is skipped).
    app.update(); // advances 100ms

    let intensity = app
        .world_mut()
        .query_filtered::<&PointLight, With<Torch>>()
        .single(app.world())
        .unwrap()
        .intensity;

    // The carried torch base_intensity is 60_000. At any non-zero t,
    // flicker_factor returns something in [48_000, 72_000].
    assert!(
        (48_000.0..=72_000.0).contains(&intensity),
        "Torch intensity {intensity} must be in the flicker band [48_000, 72_000]"
    );
    // And it must not be exactly base_intensity (unless t happens to be exactly
    // a zero-crossing of both sines — astronomically unlikely at 0.1s).
    assert_ne!(
        intensity, 60_000.0,
        "Torch intensity should not be exactly base_intensity after a non-zero tick"
    );
}
```

This would cover the gap: the `flicker_torches` system is registered in `DungeonPlugin::build`, runs `in_state(GameState::Dungeon)`, and uses `Time::elapsed_secs()`. Without this test, a future refactor that accidentally unregisters `flicker_torches` or changes its `run_if` condition would pass all existing tests.

**Fix:** Add the above test (or equivalent) to `tests.rs`. Not a blocker — the pure-function test for `flicker_factor` does validate the math; the missing coverage is the system registration path.

---

### [INFO] `dungeon_floor_omits_lighting_field_loads` only asserts `lighting` default, not `light_positions` absence

**File:** `src/data/dungeon.rs` — `dungeon_floor_omits_lighting_field_loads` test

**Observation:** After the strip, `DungeonFloor` no longer has a `light_positions` field, so the test can't assert `parsed.light_positions.is_empty()`. The test correctly asserts only `parsed.lighting == LightingConfig::default()`. The test name says "omits_lighting_field" which maps directly to what it tests. No issue.

---

### [INFO] `tests.rs` contains `handle_dungeon_input_drops_input_during_animation` which re-creates a DungeonFloor inline

**File:** `src/plugins/dungeon/tests.rs` — `handle_dungeon_input_drops_input_during_animation`

**Observation:** This test constructs a `DungeonFloor` literal inline rather than using `make_open_floor`. The inline literal correctly includes `lighting: LightingConfig::default()` (added in the followup strip commit). This is consistent. The use of an inline literal was pre-existing test style; the new field was correctly appended. No issue.

---

## Verified Correct (positive callouts)

**`ColorRgb` wrapper and serde gap:** The doc-comment on `ColorRgb` is explicit and durable: "DO NOT replace with `bevy::Color`" with the reason cited (feature gate, cascade, Cargo.toml change). `into_color()` clamps all three channels. The `FogConfig` and `LightingConfig` doc-comments also explain the `DistanceFog::default()` trap. Future contributors have what they need to not accidentally undo the workaround.

**Explicit `impl Default` over `#[derive(Default)]`:** Both `FogConfig` and `LightingConfig` have hand-written `impl Default` blocks. `FogConfig::default()` gives `density: 0.12` and a warm dark grey. `LightingConfig::default()` gives `ambient_brightness: 1.0`. A derive would give `density: 0.0` (no fog) and `ambient_brightness: 0.0` (pure black) — both wrong. The plan explicitly documented this trap and the implementation correctly avoids it.

**Carried-torch sacrosanct:** `spawn_party_and_camera` diff adds only the `DistanceFog` component and the `Torch { base_intensity: 60_000.0, phase_offset: PI }` marker to the existing `children![]` tuple. `intensity: 60_000.0`, `range: 12.0`, `color: Color::srgb(1.0, 0.85, 0.55)`, `shadows_enabled: false`, `Transform::from_xyz(0.0, 0.0, 0.0)` are all byte-identical to the #8 merge state. User override honored.

**`DistanceFog` always Exponential:** Both the production code and the `distance_fog_attached_to_dungeon_camera` test assert `FogFalloff::Exponential { .. }` explicitly. The test would catch a regression to the invisible `Linear { 0.0, 100.0 }` default.

**`#[serde(default)]` on `LightingConfig` field and on the struct:** The `DungeonFloor.lighting` field has `#[serde(default)]`; additionally `LightingConfig` itself carries `#[serde(default)]` struct-level so individual sub-fields can be omitted. The `dungeon_floor_omits_lighting_field_loads` test verifies backward compat with assets that don't have the `lighting:` block.

**Test relocation is clean:** `mod.rs` now contains only `#[cfg(test)] mod tests;` (two lines). `tests.rs` opens with `use super::*;` and has no `#[cfg(test)]` at the file level (the attribute is on the `mod tests;` declaration in `mod.rs`). All existing tests are present; the relocated helpers `make_open_floor`, `make_walled_floor`, `insert_test_floor`, `advance_into_dungeon`, `make_test_app` compile correctly because `super::*` resolves to `mod.rs`'s namespace.

**Cargo.toml SHA identical:** Both branch and `main` Cargo.toml SHA `a1d9078b20b2f7e8851019079bf9e121624f15cd`. Zero new dependencies. The `bevy/serialize` feature was not enabled.

**`data/dungeon.rs` frozen-file scope:** Only the new types and one new `DungeonFloor` field were added. `validate_wall_consistency`, `can_move`, `wall_between`, `is_well_formed`, existing variants — none were touched. The frozen-file exception is scoped correctly. `TorchData` and `light_positions` were present in intermediate commits but are absent in the net diff.

**`data/mod.rs` re-exports:** `ColorRgb`, `FogConfig`, `LightingConfig` added to `pub use dungeon::{...}`. `TorchData` is absent (correctly stripped). The re-export line does not include `TorchData`.

---

## Review Summary

| Severity | Count |
| -------- | ----- |
| CRITICAL | 0     |
| HIGH     | 0     |
| MEDIUM   | 2     |
| LOW      | 1     |
| INFO     | 2     |

**Verdict: APPROVE WITH NITS**

The two MEDIUM findings are documentation observations, not correctness bugs:
- The integration test count (120) is correct; the surrounding doc context would benefit from a note explaining the strip that happened before merge.
- The `dungeon_floor_round_trips_with_lighting` observation is a non-issue.

The one LOW finding (missing end-to-end `flicker_torches` system test) is a test coverage gap, not a bug in production code. The pure-function `flicker_factor` test covers the math; the missing test would cover system registration. Recommended to add before the next feature that touches the flicker system, but not a merge blocker.

All hard correctness gates pass:
- Carried torch properties unchanged.
- `DistanceFog::default()` falloff trap avoided.
- `impl Default` blocks correct.
- `ColorRgb::into_color` clamps correctly.
- Cargo.toml/Cargo.lock unchanged.
- No dead cell-torch surface area in net diff.
- Test relocation syntactically correct.
- `data/dungeon.rs` freeze honored.
