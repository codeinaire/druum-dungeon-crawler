---
name: Bevy 0.18 test — just_pressed persists across app.update() without InputPlugin
description: In Bevy unit tests using init_resource instead of InputPlugin, just_pressed is never cleared between app.update() calls; cycler-style systems fire again in subsequent updates, potentially queueing phantom transitions
type: feedback
---

When a Bevy test uses `init_resource::<ButtonInput<KeyCode>>()` (correct fix for avoiding InputPlugin's PreUpdate clear), `just_pressed` is never cleared between `app.update()` calls. `ButtonInput::press` only inserts to `just_pressed` if the key is not already in `pressed`. So after one `press(F9)`, `just_pressed(F9)` returns `true` on every subsequent `app.update()` until `clear_just_pressed(F9)` or `clear()` is explicitly called.

**Why:** `InputPlugin`'s `keyboard_input_system` (in `PreUpdate`) calls `clear()` each frame — that's what normally resets `just_pressed`. Without it, the set is permanent until manually cleared.

**How to apply:** When reviewing Bevy tests that simulate key presses with `init_resource` and call `app.update()` multiple times, check whether the test's `just_pressed` state persists into later updates unexpectedly. For F9-cycler tests, a "third update to observe committed state" also re-fires the cycler and queues a second transition. The test may still pass (because `State<S>` is the committed value, not `NextState`), but leaves a phantom pending transition. Flag as LOW if the test passes; suggest adding `clear_just_pressed` between updates when the test is hardened.

Observed in: `src/plugins/state/mod.rs` test `f9_advances_game_state`, Feature #2, PR #2.
