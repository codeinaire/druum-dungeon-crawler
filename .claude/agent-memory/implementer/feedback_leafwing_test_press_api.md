---
name: leafwing-input-manager 0.20 — use Buttonlike::press() for test input injection
description: KeyCode::X.press(app.world_mut()) is cleaner than manual KeyboardInput construction for test key injection — uses leafwing's own Buttonlike trait
type: feedback
---

In `leafwing-input-manager 0.20.0`, `KeyCode` implements the `Buttonlike` trait which provides a `press(&self, world: &mut World)` method. This writes a correctly-formed `Messages<KeyboardInput>` message internally.

**Why:** The method sets placeholder values for `logical_key` and `window` correctly (matching leafwing's own test suite). Manual `KeyboardInput` construction is more verbose and error-prone.

**How to apply:** In test code with `use leafwing_input_manager::prelude::*` in scope (brings `Buttonlike` into scope), use:
```rust
KeyCode::KeyW.press(app.world_mut());
app.update();
let action_state = app.world().resource::<ActionState<DungeonAction>>();
assert!(action_state.just_pressed(&DungeonAction::MoveForward));
```
No need to import `Messages<KeyboardInput>` or construct `KeyboardInput` manually. The `Buttonlike` trait comes from `leafwing_input_manager::user_input` which is re-exported via `prelude::*`.
