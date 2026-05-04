# Pipeline Summary: Feature #9 — Dungeon Lighting & Atmosphere (research → plan, paused at approval)

**Date:** 2026-05-04
**Status:** Paused at plan-approval. Parent dispatches implementer manually.
**Original task:** Drive research → plan pipeline for Feature #9 from the dungeon crawler roadmap. Add `DistanceFog` (per-floor RON parameters), low warm `GlobalAmbientLight`, per-cell `PointLight` torches placed via `light_positions` field on `DungeonFloor`, flicker animation, shadow-cap of 4 per visible region, sample torches in `floor_01.dungeon.ron`. Reconcile with Feature #8's user-override player-attached torch. Bevy 0.18.1, Δ deps = 0.

## Pipeline path

This run was a **resumed pipeline**. A previous orchestrator completed Step 1 (research) and paused on two Category B decisions. This run picked up after the user answered both, recorded those answers in `PIPELINE-STATE.md`, ran Step 2 (planner), and is pausing at plan-approval per the project's Pause-at-plan-approval pattern (`SendMessage` does not actually resume returned agents — confirmed across Features #3-#8).

## Step 1: Research (already complete, summarized for context)

**Artifact:** `/Users/nousunio/Repos/Learnings/claude-code/druum/project/research/20260504-040000-feature-9-dungeon-lighting-atmosphere.md` (62KB, HIGH confidence)

Every Bevy 0.18.1 lighting/fog API verified on-disk in `bevy_*-0.18.1/`. Δ deps = 0 achievable. Surfaced two Category B decisions that the user resolved before this run.

Key findings the planner consumed:
- `bevy::Color: Serialize/Deserialize` is feature-gated behind `bevy_color/serialize` which Druum does NOT enable. Solution: `ColorRgb(f32, f32, f32)` wrapper struct with `into_color()` builder, keeps Δ deps = 0.
- `DistanceFog::default()` is `Linear { 0.0, 100.0 }` — invisible at dungeon scale. Always specify `falloff: FogFalloff::Exponential { density: 0.12 }` explicitly.
- Spec's "cap shadow-casting torches at 4 per visible region" is NOT a Bevy API. Author `shadows: true` on at most 3-4 entries; trust Bevy's stable sort. Zero LOC.
- Flicker formula: `1.0 + 0.10*sin(t*6.4 + phase) + 0.05*sin(t*23 + phase*1.7)`, ±15% peak amplitude. Per-entity phase from `(x*31)^(y*17)` hash. No noise crate needed.

## User decisions (recorded between Step 1 and Step 2)

**Decision 1 — Player-torch reconciliation: OPTION A with two modifications.**
- KEEP existing player-attached `PointLight` (carried torch) AND ADD cell-anchored torches per the roadmap.
- Do NOT modify carried torch's properties. User quote: *"the code is the source of truth so don't change it."* Where spec/research and existing code disagree on the carried torch, trust the code.
- Carried torch DOES flicker, with `phase_offset = π` to desync from cell torches.
- Per-floor RON `LightingConfig { fog, ambient_brightness, ... }` proceeds as research recommended. NO `carried_torch: bool` toggle (Option C was not selected).

**Decision 2 — Module split: OFF-MENU OPTION D (user-proposed).**
- User rejected Options A/B/C (stay single-file, extract `renderer.rs`, extract `lighting.rs`) and proposed instead: extract the `#[cfg(test)] mod tests { ... }` block from `src/plugins/dungeon/mod.rs` into a separate file. User quote: *"can you move the testing code to somewhere else, that would cut down on the file size a lot."*
- Standard Rust pattern: `#[cfg(test)] mod tests;` in `mod.rs`, body moves to `src/plugins/dungeon/tests.rs`. Pure file-move.
- Refactor lands EARLY (Step 1 of plan), before any new lighting code.
- This is in addition to all §9 lighting work, not a substitute. `mod.rs` stays single-file for production code.

## Step 2: Plan

**Artifact:** `/Users/nousunio/Repos/Learnings/claude-code/druum/project/plans/20260504-050000-feature-9-dungeon-lighting-atmosphere.md`

**Structure:** 10 atomic steps, each single-file/single-change with explicit verification.

1. Relocate `mod tests` block from `mod.rs` to a new `tests.rs` file (refactor — no behavioral change). User Decision 2.
2. Add `ColorRgb`, `TorchData`, `FogConfig`, `LightingConfig` types + new `DungeonFloor` fields to `src/data/dungeon.rs`. Second and final allowed exception to data freeze.
3. Author 4 sample torches and a `lighting:` block in `assets/dungeons/floor_01.dungeon.ron`.
4. Add `Torch` marker component + `flicker_factor` and `torch_phase` pure helpers to `src/plugins/dungeon/mod.rs`.
5. Wire `DistanceFog` and the `Torch` marker into `spawn_party_and_camera`.
6. Add cell-torch spawn loop and ambient hookup to `spawn_dungeon_geometry`.
7. Add `flicker_torches` system and register it in `DungeonPlugin::build`.
8. Add Layer 2 unit tests for the new behaviors in `src/plugins/dungeon/tests.rs`.
9. Update `tests/dungeon_geometry.rs` entity count from 120 to 124.
10. Manual visual smoke test.

**LOC estimate:** ~+200-280 production LOC + ~+120-180 test LOC. Net `mod.rs` size DROPS (because Step 1 removes ~640 lines of tests, Step 4-7 add ~200-280 LOC of production).

**Δ deps = 0:** Confirmed achievable. `Cargo.toml` and `Cargo.lock` byte-unchanged. Verification step explicitly runs `git diff Cargo.toml Cargo.lock` and expects empty output.

**Critical guardrails baked into the plan (Critical section):**
- Carried torch sacrosanct (user quote propagated as a Critical bullet)
- `ColorRgb` wrapper with in-place doc-comment "DO NOT replace with `bevy::Color`" against future undo
- `DistanceFog::default()` falloff trap explicitly trapped in tests (assert `Exponential` falloff, not just presence)
- `data/dungeon.rs` exception #2 explicitly bounded — no other refactors to that file
- `tests/dungeon_geometry.rs` count update 120 → 124 with derivation comment
- Test count baseline preserved across Step 1 refactor (62 lib + 3 integration; planner caught and corrected the pipeline state's 61 baseline via `grep -c '#\[test\]'`)
- Atomic commits scheduled per Features #7/#8 style — one logical change per commit

**Open questions:** ALL RESOLVED. The plan documents all 8 open questions with how each was resolved (user decisions, defaults from research, or planner judgment). No Category C questions remain — plan is implementable without further user clarification.

**Notable planner decisions:**
- Step ordering puts the tests-relocate refactor first (per user Decision 2/Option D), then schema, then asset, then production code in 4 atomic backend steps, then tests, then integration count, then manual smoke.
- `LightingConfig::default()` and `FogConfig::default()` are explicit `impl Default` blocks (not `#[derive(Default)]`) — derive would set `ambient_brightness: 0.0` (pure black) and `density: 0.0` (no fog), both wrong defaults for a dungeon. Hand-written defaults return `ambient_brightness: 1.0`, `color: ColorRgb(0.10, 0.09, 0.08)`, `density: 0.12`.
- `ColorRgb::into_color` clamps each channel to `[0.0, 1.0]` for trust-boundary safety on RON-loaded values.
- Carried-torch flicker hooks via the existing nested `children![]` PointLight tuple — `Torch { base_intensity: 60_000.0, phase_offset: std::f32::consts::PI }` appended; all other carried-torch properties untouched per user directive.
- Cell-torch spawn lives inside `spawn_dungeon_geometry` (one floor read shared with wall iteration); torches tagged `Torch + DungeonGeometry` for flicker filter + OnExit cleanup.
- NaN guard added to cell-torch spawn loop (research §Security architectural risk).
- Test count corrected from pipeline-state's 61-lib baseline to actual 62 (planner verified via `grep -c '#\[test\]'`); plan targets 69 lib + 3 integration after Feature #9.
- Helper updates: `make_open_floor`/`make_walled_floor` (in tests.rs after Step 1 relocate) need the two new fields appended to compile.

**Memory:** planner wrote `project_druum_dungeon_lighting.md`; `MEMORY.md` index updated with one-line hook.

## Step 3-5: Out of scope for this run

Implementer, ship, and code review are dispatched manually by the parent after plan approval (per Features #3-#8 pause-at-plan-approval pattern).

## Verification expectations after implementation

The plan's Verification section enumerates all checks the implementer must satisfy:
- `cargo check`, `cargo check --features dev`, `cargo clippy --all-targets -- -D warnings`, `cargo clippy --all-targets --features dev -- -D warnings`, `cargo test`, `cargo test --features dev`, `cargo fmt --check` — all zero warnings
- `Cargo.toml` and `Cargo.lock` byte-unchanged (`git diff Cargo.toml Cargo.lock` empty)
- `src/plugins/dungeon/tests.rs` exists and contains the relocated test body
- 7 new unit tests pass by name (3 in `data::dungeon::tests`, 4 in `plugins::dungeon::tests`)
- Integration tests pass with count 124 (was 120) and the existing `floor_01_loads_and_is_consistent` still passes
- Manual visual smoke per Step 10

## Pipeline artifacts

- Pipeline state: `/Users/nousunio/Repos/Learnings/claude-code/druum/project/orchestrator/PIPELINE-STATE.md`
- Research: `/Users/nousunio/Repos/Learnings/claude-code/druum/project/research/20260504-040000-feature-9-dungeon-lighting-atmosphere.md`
- Plan: `/Users/nousunio/Repos/Learnings/claude-code/druum/project/plans/20260504-050000-feature-9-dungeon-lighting-atmosphere.md`

## What the user needs to approve before implementation

Nothing requires user override. The plan resolves both Category B decisions per user direction, has zero Category C questions, and explicitly schedules every step with verification. The user only needs to confirm "proceed with the plan" before the parent dispatches the implementer.

The only thing the user might want to spot-check before approving:
- The 4 torch coordinates picked for `floor_01.dungeon.ron` (1,1) / (4,1) / (2,4) / (4,4) — chosen for visibility from entry point and 3 warm + 1 blue mage-touched aesthetic. If a different layout is preferred, change Step 3 before implementation.
- Fog density `0.12` (slightly less dense than master research's `0.15` because Druum's corridors are 6 cells across) — tunable in Step 10 manual smoke if it reads too dense or too thin.

Both are tuning knobs, not architectural decisions; they can also be tweaked after implementation in **Implementation Discoveries** without re-planning.
