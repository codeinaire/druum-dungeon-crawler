# Implementation Summary: Feature #9 — Dungeon Lighting & Atmosphere

**Plan:** `/Users/nousunio/Repos/Learnings/claude-code/druum/project/plans/20260504-050000-feature-9-dungeon-lighting-atmosphere.md`
**Date:** 2026-05-04
**Status:** Cell torches removed in followup commit (2026-05-04). See **Followup correction** section at the bottom of this file. Final scope: fog + carried-torch flicker + per-floor lighting RON (no cell torches authored).

## Steps Completed

All 9 automated steps implemented; Step 10 (manual smoke) deferred to user.

1. **Test relocation (Step 1):** Extracted the `#[cfg(test)] mod tests { ... }` body (lines 772–1436) from `src/plugins/dungeon/mod.rs` into a new sibling `src/plugins/dungeon/tests.rs`. The `mod.rs` now contains only `#[cfg(test)] mod tests;`. Baseline test count preserved identically: 61 lib + 3 integration before and after.

2. **Schema additions (Step 2):** Added `ColorRgb`, `TorchData`, `FogConfig`, `LightingConfig` types to `src/data/dungeon.rs` (second and final allowed exception to the frozen file). Added `light_positions: Vec<TorchData>` and `lighting: LightingConfig` fields to `DungeonFloor`, both `#[serde(default)]`. Re-exported all four from `src/data/mod.rs`. Updated all `DungeonFloor` literal constructions in `tests.rs` and `dungeon.rs::tests` to include the new fields. Added 3 new unit tests: `color_rgb_clamps_out_of_range_channels`, `dungeon_floor_round_trips_with_lighting`, `dungeon_floor_omits_lighting_field_loads`.

3. **RON asset authoring (Step 3):** Added 4 sample torches and a `lighting:` block to `assets/dungeons/floor_01.dungeon.ron`. Torches: (1,1) warm shadows=true, (4,1) warm shadows=true, (2,4) blue mage shadows=true, (4,4) warm shadows=false. Fog density 0.12, ambient 1.0. Both ron 0.12 unit test and ron 0.11 RonAssetPlugin loader confirmed parsing.

4. **Torch component + helpers (Step 4):** Added `Torch { base_intensity, phase_offset }` marker component in the Components section. Added `torch_phase(x, y) -> f32` (deterministic cell-hash) and `flicker_factor(t, phase) -> f32` (two-sine, clamped `[0.80, 1.20]`) pure helpers.

5. **Fog + carried torch flicker (Step 5):** Modified `spawn_party_and_camera` to read `floor.lighting.fog` and add `DistanceFog { color, falloff: FogFalloff::Exponential { density }, ..default() }` to the `Camera3d`. Added `Torch { base_intensity: 60_000.0, phase_offset: PI }` marker to the existing carried `PointLight` — no other carried-torch properties changed (sacrosanct per user override).

6. **Cell torch spawn + per-floor ambient (Step 6):** Added cell-torch spawn loop in `spawn_dungeon_geometry` iterating `floor.light_positions`. Each torch: `PointLight` with `color.into_color()`, `intensity`, `range`, `shadows_enabled`; tagged `DungeonGeometry` (cleanup) + `Torch` (flicker); NaN guard on intensity/range. Replaced hard-coded `brightness: 1.0` with `floor.lighting.ambient_brightness`.

7. **Flicker system (Step 7):** Added `flicker_torches` Update system: `Query<(&mut PointLight, &Torch)>`, runs `run_if(in_state(GameState::Dungeon))`, sets `intensity = base_intensity * flicker_factor(t, phase_offset)`. Registered in `DungeonPlugin::build`.

8. **Layer 2 tests (Step 8):** Added to `tests.rs`: `distance_fog_attached_to_dungeon_camera` (asserts DistanceFog with Exponential falloff on DungeonCamera), `torches_spawned_per_light_positions` (2 cell + 1 carried = 3 total), `flicker_modulates_intensity_over_time` (ManualDuration determinism), `flicker_is_deterministic_for_same_phase_and_t` (pure-function). Extended `on_exit_dungeon_despawns_all_dungeon_geometry` with post-exit Torch count assertion.

9. **Integration test count update (Step 9):** Updated `tests/dungeon_geometry.rs` assertion from 120 to 124 (120 + 4 torches). Updated docstring derivation: `36 + 36 + 48 + 4 = 124`.

10. **Manual smoke test (Step 10):** DEFERRED — see checklist below.

## Steps Skipped

None. All 9 automated steps executed. Step 10 requires runtime visual inspection.

## Deviations from Plan

1. **Explicit `bevy::pbr::{DistanceFog, FogFalloff}` import required.** The plan said these were in `bevy::prelude::*` (HIGH confidence) and an explicit import "shouldn't be needed." In practice the compiler required the explicit import. Added `use bevy::pbr::{DistanceFog, FogFalloff};` to `mod.rs`. No behavioral change.

2. **`cargo fmt` reformatted 5 items** across the implementation: comment spacing in `spawn_party_and_camera` (Step 1), long `assert_eq!` lines in `dungeon.rs::tests` (Step 2), long `assert_eq!` in `tests.rs` (Step 8). Applied `cargo fmt` after each quality gate; committed as part of the relevant step.

3. **`clippy::manual_range_contains`** triggered on `intensity >= 800.0 && intensity <= 1200.0` in the flicker test. Rewritten as `(800.0..=1200.0).contains(&intensity)`. Not in the plan's Pitfall section.

4. **`clippy::doc_lazy_continuation`** triggered on `dungeon_geometry.rs` docstring continuation. Added an extra commit beyond Step 9 to fix the list-item formatting. No behavioral change.

5. **`make_floor` in `data/dungeon.rs::tests`** needed the new fields added (plan mentioned `dungeon_floor_round_trips_with_real_data` and `tests.rs` helpers but not this private helper). Fixed.

6. **Baseline lib test count was 61, not 62.** The plan's Verification section mentioned "baseline 62" in one place; actual count confirmed 61. The final count of 68 (non-dev) + 3 integration is consistent with 61 + 7 new tests.

## Verification Results

| Command | Status | Count/Notes |
|---------|--------|-------------|
| `cargo check` | PASS | zero warnings |
| `cargo check --features dev` | PASS | zero warnings |
| `cargo clippy --all-targets -- -D warnings` | PASS | zero warnings |
| `cargo clippy --all-targets --features dev -- -D warnings` | PASS | zero warnings |
| `cargo test` | PASS | 68 lib + 3 integration (all pass) |
| `cargo test --features dev` | PASS | 69 lib + 3 integration (all pass) |
| `cargo fmt --check` | PASS | zero diff |
| `git diff Cargo.toml Cargo.lock` | PASS | empty — zero new deps |

Individual test verification:
- `color_rgb_clamps_out_of_range_channels` — PASS
- `dungeon_floor_round_trips_with_lighting` — PASS
- `dungeon_floor_omits_lighting_field_loads` — PASS
- `distance_fog_attached_to_dungeon_camera` — PASS
- `torches_spawned_per_light_positions` — PASS
- `flicker_modulates_intensity_over_time` — PASS
- `flicker_is_deterministic_for_same_phase_and_t` — PASS
- `on_exit_dungeon_despawns_all_dungeon_geometry` (extended) — PASS
- `dungeon_geometry_spawns_for_floor_01` (count 124) — PASS
- `floor_01_loads_and_is_consistent` — PASS
- `floor_01_loads_through_ron_asset_plugin` — PASS

## LOC and Dependency Impact

- Production LOC added: ~110 in `mod.rs` (Torch component, helpers, fog hookup, torch spawn loop, flicker system), ~100 in `dungeon.rs` (4 new types + 2 new fields), ~25 in `floor_01.dungeon.ron` (4 torches + lighting block), ~5 in `data/mod.rs` (re-exports)
- Test LOC added: ~140 in `tests.rs` (4 new tests + helper + extended test), ~70 in `dungeon.rs::tests` (3 new tests), ~20 in `dungeon_geometry.rs` (count + doc update)
- New Cargo dependencies: 0 (Cargo.toml and Cargo.lock byte-unchanged)

## Manual Smoke Test Checklist (deferred to user)

Run `cargo run --features dev`, press F9 to enter Dungeon state, then verify:

- [ ] **Dungeon renders.** Player spawns at (1, 1) facing North. No crash or warning spam.
- [ ] **Fog is visible.** Distant walls (3+ cells away) fade to dark warm grey. Walking forward makes the next corridor "emerge" from the fog (no pop-in).
- [ ] **Cell (1, 1) is brighter** than an empty corridor cell — torch is overhead at entry.
- [ ] **Cells (4, 1) and (4, 4)** also show warm local brightness from their torches.
- [ ] **Cell (2, 4)** shows distinctly bluish/purple tint from the mage torch.
- [ ] **Torches flicker** — intensity oscillates visibly but subtly (not strobing). Standing still for 10 seconds should show continuous variation.
- [ ] **Carried torch is desynced from cell torch at (1, 1)** — phase_offset = PI means when the (1,1) cell torch is bright, the carried torch is slightly dimmer, and vice versa. This is subtle but visible when standing directly under the (1,1) sconce.
- [ ] **Shadows visible near (1, 1) torch** (shadows=true): wall corners and the player body cast shadows on floor/walls.
- [ ] **(4, 4) torch does NOT cast shadows** (shadows=false, by design).
- [ ] **F9 to exit Dungeon** — no orphan torches, fog, or bright corridors in the next state.
- [ ] **F9 back to Dungeon** — everything respawns correctly, no double-rendered torches, ambient near-black restored.

Document any visual surprises (color tuning, fog density, torch intensity) in Implementation Discoveries if a follow-up commit is needed. Structural changes (new system, new component) are out of scope for #9 and become #25 polish items.

## Commits on Branch `ja-branch-3`

1. `182f853` — Refactor: move `dungeon::tests` body to a sibling file (Step 1)
2. `2a68870` — feat(data): add lighting/torch schema to DungeonFloor (Step 2)
3. `b992188` — feat(asset): add 4 sample torches + lighting block to floor_01.dungeon.ron (Step 3)
4. `ff043f8` — feat(dungeon): add Torch component, fog, cell torches, and flicker system (Steps 4–7)
5. `4773c89` — test(dungeon): cover fog, torch spawn, flicker, and cleanup (Step 8)
6. `b3aaf4e` — test(integration): bump dungeon_geometry count to 124 (4 torches) (Step 9)
7. `7ec75d5` — fix(integration): fix doc_lazy_continuation clippy lint in dungeon_geometry.rs (Step 9 follow-up)

---

## Followup correction (2026-05-04)

User clarified after smoke-test review that the original Decision 1 answer ("just keep carried torch for now and yes have it flicker, the code is the source of truth so don't change it") was a scope reduction — no cell torches wanted in #9 at all. Orchestrator-side prompt misread the intent as Option A (keep carried + add cell), and dispatched the planner+implementer to add cell torches. Memory captured at `~/.claude/projects/-Users-nousunio-Repos-Learnings-claude-code-druum/memory/feedback_user_answers_to_options.md`.

Per user direction, history was NOT rewritten. A single followup commit removes cell-torch surface area while keeping everything the user did want (fog, carried-torch flicker, test relocation, per-floor lighting RON for fog/ambient).

### What was removed

- `TorchData` struct from `src/data/dungeon.rs` and the `light_positions: Vec<TorchData>` field on `DungeonFloor`.
- `TorchData` re-export from `src/data/mod.rs`.
- 4 torch entries from `assets/dungeons/floor_01.dungeon.ron` (the `light_positions: [...]` block + comment).
- Cell-torch spawn loop in `spawn_dungeon_geometry` (the `for torch in &floor.light_positions` block + NaN guard).
- `torch_phase(x, y)` private helper (no longer called — carried torch uses literal `f32::consts::PI`).
- Two cell-torch tests: `torches_spawned_per_light_positions` and `flicker_modulates_intensity_over_time` (the latter required cell torches to test the system end-to-end).
- The `insert_test_floor_with_torches` test helper.
- Three `light_positions: Vec::new()` entries in test-helper `DungeonFloor` literals (now drops the field entirely).
- Updated doc comments on `Torch`, `spawn_dungeon_geometry`, and the integration-test header to remove cell-torch references.

### What was kept

- Test relocation (`mod.rs` ↔ `tests.rs`) — Decision 2.
- `DistanceFog` on `DungeonCamera` with `Exponential { density }` falloff — drives the "atmosphere" half of §9.
- `Torch` marker component + `flicker_factor` helper + `flicker_torches` system — drives the carried-torch flicker.
- `ColorRgb`, `FogConfig`, `LightingConfig` types + `lighting: LightingConfig` field on `DungeonFloor` + `lighting:` block in `floor_01.dungeon.ron` — per-floor fog/ambient tuning.
- Per-floor `ambient_brightness` flowing into `GlobalAmbientLight` on enter / restored to `default()` on exit.
- Carried-torch `Torch { base_intensity: 60_000.0, phase_offset: PI }` marker on the existing carried `PointLight` — properties otherwise sacrosanct per user override.
- Tests still passing: `distance_fog_attached_to_dungeon_camera`, `flicker_is_deterministic_for_same_phase_and_t`, `on_exit_dungeon_despawns_all_dungeon_geometry` (with carried-torch despawn assertion), 3 ron round-trip tests, all earlier tests.

### Verification (post-strip)

| Command | Result |
|---|---|
| `cargo check` | PASS — zero warnings |
| `cargo check --features dev` | PASS — zero warnings |
| `cargo clippy --all-targets -- -D warnings` | PASS — zero warnings |
| `cargo clippy --all-targets --features dev -- -D warnings` | PASS — zero warnings |
| `cargo fmt --check` | PASS — zero diff |
| `cargo test` | PASS — **66 lib + 3 integration** (down 2 cell-torch tests from 68) |
| `cargo test --features dev` | PASS — **67 lib + 3 integration** (down 2 from 69) |
| `git diff Cargo.toml Cargo.lock` | empty (Δ deps still 0) |

### Manual smoke test still required

The original Step 10 manual smoke test is still pending — but the checklist is now smaller:
- Fog visible at corridor distance
- Subtle carried-torch flicker (not strobing — `[0.80, 1.20]` band on intensity 60_000)
- No 4 cell torches anywhere in floor_01 (regression check on the strip)
- Clean OnExit/OnEnter cycle on F9 (carried torch despawns and respawns; no orphans)
