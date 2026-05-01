# Code Review: Feature #2 — Bevy 0.18.1 Game State Machine

**Date:** 2026-05-01
**PR:** https://github.com/codeinaire/druum-dungeon-crawler/pull/2
**Branch:** 2-game-state-machine → main
**Verdict:** APPROVE

## What Was Reviewed

Full review of all source files changed in PR #2. Coverage:

| File | Coverage |
|---|---|
| `src/plugins/state/mod.rs` | Full |
| `src/main.rs` | Full |
| `src/plugins/mod.rs` | Full |
| `src/plugins/dungeon/mod.rs` | Full |
| `src/plugins/combat/mod.rs` | Full |
| `src/plugins/town/mod.rs` | Full |
| Project/memory docs | Skipped (non-source) |

## Static Analysis

All static analysis passes on local checkout:

| Check | Result |
|---|---|
| `cargo check` | Pass |
| `cargo check --features dev` | Pass |
| `cargo clippy --all-targets -- -D warnings` | Pass |
| `cargo clippy --all-targets --features dev -- -D warnings` | Pass |
| `cargo test` (default) | Pass — 1 test |
| `cargo test --features dev` | Pass — 2 tests |
| `git diff Cargo.lock` | Empty — no new deps |

## Key Findings

### [LOW] F9 test leaves a dangling pending state transition after the third update

**File:** `src/plugins/state/mod.rs:117–137`

The `f9_advances_game_state` test correctly asserts `GameState::TitleScreen` after three `app.update()` calls, but the third update silently queues a second transition (`Town`) that is never consumed.

**Root cause:** `press(KeyCode::F9)` inserts `F9` into `just_pressed`. Without `InputPlugin`, no `ButtonInput::clear()` is called between `app.update()` invocations. `ButtonInput::press` only re-inserts to `just_pressed` if the key is not already in `pressed`, so `just_pressed(F9)` remains `true` through the third update. That means the cycler fires again in update 3's `Update` phase, queuing `next.set(Town)`.

The test passes because the assert reads `State<GameState>` (the committed value, now `TitleScreen`), not `NextState`. But any hypothetical fourth `app.update()` would apply the `Town` transition. The plan notes this at the bottom: "For thorough multi-press testing in future, call `clear_just_pressed`." The deviation is documented and the test intent is correct — it's not a bug in the feature, but the test could be made strictly correct by adding `clear_just_pressed` between updates. Not a blocker.

**Suggested fix for future hardening:**

```rust
// After the first update that runs the cycler:
app.world_mut()
    .resource_mut::<ButtonInput<KeyCode>>()
    .clear_just_pressed(KeyCode::F9);
app.update(); // post-transition Update sees the new state — no phantom re-fire
```

## Items Explicitly Verified

- **Symmetric `#[cfg(feature = "dev")]` gating**: `add_systems` call (line 62) AND function definition (line 71) are both gated. Correct.
- **`OnEnter`/`OnExit` zero-arg closures**: Idiomatic for Bevy 0.18, compiles and passes clippy cleanly.
- **`init_resource` deviation**: Correct fix. `InputPlugin` registers `keyboard_input_system` in `PreUpdate` which clears `just_pressed` before `Update` runs. Using `init_resource` directly avoids the clearing system while satisfying parameter validation.
- **`Default` on all `SubStates` derives**: All three sub-state enums derive `Default`. Each has exactly one `#[default]` variant matching the roadmap (`Exploring`, `PlayerInput`, `Square`).
- **`StatePlugin` position**: Listed immediately after `DefaultPlugins` in `main.rs` plugin tuple. Required — `init_state` panics if `StatesPlugin` (from `DefaultPlugins`) is not yet registered.
- **No `EventReader<StateTransitionEvent>` usage**: Absent. The `state_changed::<GameState>` run condition is used throughout as intended.
- **No `Copy` derive on `GameState`**: Correct omission — variants may carry data later.
- **No `GameState` re-export from crate root**: `lib.rs` exposes only `pub mod plugins`. Each consuming plugin imports `use crate::plugins::state::GameState` explicitly.
- **No new dependencies**: `Cargo.lock` diff is empty.

## Review Summary

| Severity | Count |
|---|---|
| CRITICAL | 0 |
| HIGH | 0 |
| MEDIUM | 0 |
| LOW | 1 |

**Verdict: APPROVE**

The implementation is correct, idiomatic Bevy 0.18.1, and passes all automated checks. The single LOW finding (test leaves a phantom pending transition on the third update) does not affect correctness of the feature or the test outcome — the plan already documents the mitigation for future use. Safe to merge.
