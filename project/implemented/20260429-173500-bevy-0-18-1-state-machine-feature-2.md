# Implementation Summary: Bevy 0.18.1 Game State Machine (Feature #2)

**Date:** 2026-04-29
**Plan:** [../plans/20260429-031500-bevy-0-18-1-state-machine-feature-2.md](../plans/20260429-031500-bevy-0-18-1-state-machine-feature-2.md)

## Steps Completed

- **Step 1** — Created `src/plugins/state/mod.rs` with `GameState`, three `SubStates` enums, `StatePlugin`, `log_game_state_transition`, and `#[cfg(feature = "dev")]`-gated `cycle_game_state_on_f9`.
- **Step 2** — Added `pub mod state;` alphabetically (between `save` and `town`) in `src/plugins/mod.rs`.
- **Step 3** — Added `state::StatePlugin` to the import block and inserted `StatePlugin` immediately after `DefaultPlugins` in `src/main.rs`.
- **Step 4** — Replaced `DungeonPlugin`, `CombatPlugin`, and `TownPlugin` stub bodies with `OnEnter`/`OnExit` log-stub systems using zero-arg closures.
- **Step 5** — Static review pass confirmed symmetric `#[cfg(feature = "dev")]` gating on both the function definition and the `add_systems` call. No unconditional imports of dev-only types.
- **Step 6** — Added `#[cfg(test)] mod tests` block with `gamestate_default_is_loading` and `#[cfg(feature = "dev")] fn f9_advances_game_state`. Required a fix (see Deviations).
- **Step 7** — Final review pass: all convention checks pass.

## Steps Skipped

None.

## Deviations from the Plan

**Test setup: `InputPlugin` replaced with `init_resource::<ButtonInput<KeyCode>>()`**

The plan specified using `StatesPlugin + StatePlugin` for both tests, with no mention of input resources. Under `--features dev`, `StatePlugin::build` registers `cycle_game_state_on_f9` (via `#[cfg(feature = "dev")]`), which requires `ButtonInput<KeyCode>` at system parameter validation time. The plan's suggested `StatesPlugin`-only setup therefore panics on `app.update()` with "Resource does not exist".

Initial fix attempt: add `bevy::input::InputPlugin`. This resolved the panic but broke `f9_advances_game_state` — `InputPlugin` registers `keyboard_input_system` in `PreUpdate`, which calls `keycode_input.bypass_change_detection().clear()` at the start of every frame, clearing `just_pressed` before `Update` runs the cycler.

Correct fix: `app.init_resource::<ButtonInput<KeyCode>>()` directly. This inserts the resource without the clearing system, so manually-set `just_pressed` state survives to the `Update` schedule. Both tests pass under both feature sets.

The `gamestate_default_is_loading` test also needs the resource under `--features dev` (even though it doesn't test F9) because the registered cycler system's parameter validation requires it. A `#[cfg(feature = "dev")]` block inside the test handles this.

## Issues Deferred

None.

## Verification Results

| Check | Status |
|---|---|
| `cargo check` | passed |
| `cargo clippy --all-targets -- -D warnings` | passed |
| `cargo clippy --all-targets --features dev -- -D warnings` | passed |
| `cargo test` (default features) | passed — 1 test |
| `cargo test --features dev` | passed — 2 tests |
| `git diff Cargo.lock` | empty diff — no new deps |
| Manual smoke test (F9 cycles state) | deferred to parent session |
| `cargo audit` | optional, deferred to parent session |

## Final LOC of `src/plugins/state/mod.rs`

139 total lines; approximately 117 non-blank non-comment lines. Within the 120 hard budget.
