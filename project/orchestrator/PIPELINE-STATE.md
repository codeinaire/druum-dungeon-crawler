# Pipeline State

**Task:** Drive the full pipeline (research → plan) for Feature #7: Grid Movement & First-Person Camera from the dungeon crawler roadmap. This is the FIRST multi-system integration feature — reads `ActionState<DungeonAction>` (#5), `DungeonFloor::can_move` (#4), writes `MessageWriter<SfxRequest>` (#6), defines new `Message<MovedEvent>` for downstream subscribers (#13/#16/#22), owns `GridPosition` + `Facing` + `PlayerParty` components. Spawns `Camera3d` and a placeholder test scene (cubes + lights) so movement is *visually verifiable* before Feature #8 (real 3D dungeon) lands. PAUSE at plan-approval; parent dispatches implementer manually because `SendMessage` does not actually resume returned agents (confirmed during Features #3, #4, #5, and #6).
**Status:** completed
**Last Completed Step:** 5

## Artifacts

| Step | Description | Artifact                                 |
| ---- | ----------- | ---------------------------------------- |
| 1    | Research    | /Users/nousunio/Repos/Learnings/claude-code/druum/project/research/20260503-120000-feature-7-grid-movement-first-person-camera.md |
| 2    | Plan        | /Users/nousunio/Repos/Learnings/claude-code/druum/project/plans/20260503-130000-feature-7-grid-movement-first-person-camera.md |
| 3    | Implement   | /Users/nousunio/Repos/Learnings/claude-code/druum/project/implemented/20260503-195500-feature-7-grid-movement-first-person-camera.md |
| 4    | Ship        | https://github.com/codeinaire/druum-dungeon-crawler/pull/7 (branch `7-grid-movement-first-person-camera`, commit `a9a723e`) |
| 5    | Code Review | /Users/nousunio/Repos/Learnings/claude-code/druum/project/reviews/20260503-210000-feature-7-grid-movement-first-person-camera.md (verdict: APPROVE, 1 LOW deferable to #8) |

## Implementation Notes (Step 3)

User approved the plan as-is on 2026-05-03 (no overrides). Implementer completed the full plan with one notable deviation:

**TestState + TestFloorAssets pattern in `tests/dungeon_movement.rs`** — the plan called for using real `DungeonAssets` loading via `LoadingPlugin`. The implementer hit a hang (likely `LoadingPlugin` waiting on audio assets that aren't in the test harness), worked around it with a private `TestState` enum + `TestFloorAssets` collection that mirrors the prod shape but isolates from `LoadingPlugin`. Documented in `.claude/agent-memory/implementer/feedback_integration_test_avoid_loadingplugin.md`. Reviewer should verify this doesn't compromise test value.

**LOC came in at 997 (vs 790 estimate, ~25% over)** — extra is doc comments + 13 inline tests. Within roadmap budget.

**Cross-cutting cargo fmt normalization** touched 7 "frozen" files (input, state, audio/{mod,sfx}, data/dungeon, loading, town, combat). All changes are pure whitespace re-flow per rustfmt rules — no semantic changes. Likely fmt was never previously run cleanly on these files.

Verification: all 7 commands passed (cargo build / build --features dev / test / test --features dev / clippy --all-targets -D warnings / clippy --all-targets --features dev -D warnings / fmt --check). 51 lib + 2 integration tests default; 52 + 2 with `--features dev`. **Cargo.toml + Cargo.lock byte-unchanged** — zero new deps confirmed.

Manual visual smoke: NOT executed by implementer (deferred to ship/post-merge user verification).

## Research Summary (Step 1)

Research is **HIGH-confidence** with all Bevy 0.18 APIs verified on-disk and integration contracts cited from Druum source.

### Recommendations on the 6 architectural questions

| # | Question | Recommendation | Confidence |
|---|----------|----------------|------------|
| 1 | Movement animation: instant vs tween | **Tween, 0.18s, smoothstep** | HIGH |
| 2 | Turn animation: instant vs tween | **Tween, 0.15s, smoothstep** (consistent with #1) | HIGH |
| 3 | Movement queue / input buffering | **None — drop via `Without<MovementAnimation>` query filter** | HIGH |
| 4 | Test scene scaffolding | **Three colored cubes at known grid coords + ground plane + ambient + directional light** (marker-tagged for #8 deletion) | MEDIUM |
| 5 | Eye height + FOV | **eye_height = 0.7, FOV = π/4 (Bevy default)** | MEDIUM |
| 6 | Cell unit scale | **CELL_SIZE = 2.0 world units** (smaller than master research's 4.0 — genre-correct corridor feel) | MEDIUM |

### Key Bevy 0.18 verified facts

1. **`Camera3dBundle` does NOT exist in 0.18.** Spawn as a component tuple: `(Camera3d::default(), Transform::from_xyz(...).looking_at(...))`. Same for `PointLight`/`DirectionalLight`/`AmbientLight`.
2. **`Time::delta_secs()`** — `delta_seconds()` does not exist.
3. **Animation pattern = component-marker** mirroring Druum's `FadeIn`/`FadeOut` in `src/plugins/audio/bgm.rs:36-67`.
4. **`MovedEvent` derives `Message`** (NOT `Event`), registered with `app.add_message::<MovedEvent>()`.
5. **`TimeUpdateStrategy::ManualDuration(Duration)`** — test-friendly mode for deterministic animation timing.
6. **Manual smoke is executable** for #7 (camera moves, cubes shift on screen).

### LOC + dep impact (research)

- 350-500 production + 100-150 tests = 450-650 total. Deps Δ = 0.

## Plan Summary (Step 2)

**Plan adopts all HIGH-confidence research recommendations** with full architectural rationale. Plan structure: Goal, Approach (12 architectural decisions), Critical (14 pitfalls), 10 commit-ordered Steps, Security, 6 Open Questions all RESOLVED, Implementation Discoveries (template), Verification (15 items), LOC estimate.

### 10 commit-ordered steps

1. **Step 1:** Replace empty `src/plugins/dungeon/mod.rs` stub with full module skeleton — constants (`CELL_SIZE=2.0`, `EYE_HEIGHT=0.7`, `MOVE_DURATION_SECS=0.18`, `TURN_DURATION_SECS=0.15`), components (`PlayerParty`, `DungeonCamera`, `GridPosition`, `Facing`, `MovementAnimation`, `TestSceneMarker`), `MovedEvent` Message, plugin skeleton with `add_message`.
2. **Step 2:** Implement `grid_to_world` and `facing_to_quat` pure helpers + 7 unit tests (no Bevy app).
3. **Step 3:** Implement `spawn_party_and_camera` (OnEnter) — spawns `PlayerParty` + child `Camera3d` at `floor.entry_point` with `children![...]` macro.
4. **Step 4:** Implement `spawn_test_scene` (3 colored cubes + ground plane + DirectionalLight, all `TestSceneMarker`-tagged) + `despawn_dungeon_entities` (OnExit).
5. **Step 5:** Implement `handle_dungeon_input` (with `Without<MovementAnimation>` filter as the input-lock) + `animate_movement` (smoothstep lerp/slerp, snap-on-completion).
6. **Step 6:** Wire all systems in `DungeonPlugin::build` — OnEnter spawns, OnExit despawn, Update for input + animate. State-gating at SYSTEM level via `run_if(in_state(...))`.
7. **Step 7:** Add 6 unit/component tests using `MinimalPlugins + InputPlugin` Layer 2 pattern from Feature #5. Test the input → MovedEvent + SfxRequest flow, wall blocks, turn-only no-event, strafe, input-drop-during-animation.
8. **Step 8:** Add App-level integration test `tests/dungeon_movement.rs` — real `DungeonAssets` loading flow asserts party spawns at `floor_01.entry_point`.
9. **Step 9:** Manual smoke verification (visible cubes, WASDQE work, walls block, OnExit cleans up) — same precedent as Feature #6's audible smoke.
10. **Step 10:** Final 7-command verification matrix + Cargo.toml/Cargo.lock byte-diff check (must be ZERO).

### Architectural decisions baked in

- **Single PlayerParty entity with child Camera3d** via `children![...]` macro. Despawn-recursive on OnExit walks children automatically.
- **Logical-vs-visual separation:** `GridPosition`/`Facing` updated immediately at input-commit time; `MovementAnimation` lerps the `Transform` over 0.18s/0.15s. Downstream consumers (#13/#16/#22) react to logical state on the same frame, not after the tween.
- **One `MovementAnimation` component** with both translation and rotation fields (~52 bytes). Two constructors: `::translate(...)` and `::rotate(...)`.
- **World coordinate convention: `world_z = +grid_y * CELL_SIZE`** (overrides the stale `data/dungeon.rs:18` doc-comment without modifying the frozen file). North movement = -Z motion = matches Bevy's default camera-looking-direction → `Quat::IDENTITY` for North.
- **System ordering:** `handle_dungeon_input.before(animate_movement)`. Both in `Update`. `handle_dungeon_input` gated on `GameState::Dungeon AND DungeonSubState::Exploring`. `animate_movement` gated only on `GameState::Dungeon` (so opening inventory mid-tween still completes the animation).
- **`MovedEvent` written ONLY for translation moves**, NOT turn moves. Turn-only writes nothing (no `MovedEvent`, no `SfxRequest`). Wall-bumps write nothing.
- **Test scene = 3 cubes (red north, blue east, green west of entry point) + 40×0.1×40 grey ground slab + DirectionalLight at 3000 illuminance with `Vec3::new(1.0, -1.0, 1.0)` look-at, shadows_enabled=false.** All `TestSceneMarker`-tagged for one-PR cleanup by Feature #8.
- **Tests use Layer 2 input pattern** (full `InputPlugin` + leafwing `KeyCode::press(world_mut())`). `TimeUpdateStrategy::ManualDuration(50ms)` for animation determinism. `make_open_floor` helper duplicates Feature #4's `make_floor` (since that helper is `#[cfg(test)]`-private to its own module — ~20 LOC of duplication is cheaper than refactoring a frozen file).

### LOC + dep impact

- **~790 LOC total** (~570 production + ~220 tests). Slightly above research envelope at upper end due to `MovedEvent` infra + integration test overhead.
- **Cargo.toml + Cargo.lock byte-unchanged** — Δ deps = 0. If `git diff` shows any change, STOP.

### All 6 secondary research-open questions RESOLVED by planner (NOT escalated as Category C)

1. world_z convention: **+grid_y** (researcher's preference, matches Bevy default camera direction).
2. CELL_SIZE location: **`src/plugins/dungeon/mod.rs`** (one import path for #8).
3. Render flat ground plane in #7: **YES** — part of test scaffolding.
4. MovementAnimation: **ONE component** with both translation+rotation fields.
5. Lighting direction: **`Vec3::new(1.0, -1.0, 1.0)` target with illuminance=3000.0, shadows_enabled=false.**
6. Q/E turns write footstep SFX: **NO** — defer to #25 polish (no SfxKind::Turn variant; would touch frozen sfx.rs).

### Critical risks the planner surfaced (NEW — not in research)

1. **Test floor stub `DungeonAssets` field mismatch.** Tests must construct stub `DungeonAssets { floor_01, item_db: Handle::default(), enemy_db: Handle::default(), class_table: Handle::default(), spell_table: Handle::default() }` — adding a 6th field in a future feature requires updating every test stub. Maintenance seam, not a blocker.
2. **System ordering subtlety:** Without explicit `.before(animate_movement)` ordering, on the input-commit frame `animate_movement` might run BEFORE `handle_dungeon_input`, leaving visual position one frame behind logical position. Plan Step 6 makes the ordering explicit.

### Cleanest-possible-ship signal

`Cargo.toml + Cargo.lock` Δ = 0. If `git diff` shows any change, STOP.

## User Decisions

[awaiting plan-approval — see final report from orchestrator]
