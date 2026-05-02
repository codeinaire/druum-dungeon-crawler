---
name: Bevy 0.18 input tests — three layers, three test patterns
description: Tests of code reading ButtonInput<KeyCode> directly use init_resource bypass; tests of leafwing ActionState require full InputPlugin; the patterns are NOT interchangeable
type: feedback
---

In Druum, there are three distinct test patterns for input-reading systems, and they are NOT interchangeable. Confusing them produces tests that compile, run, but assert against the wrong frame's state.

**Layer 1 — Code reads `Res<ButtonInput<KeyCode>>` DIRECTLY (e.g. F9 cycler):**

```rust
app.init_resource::<ButtonInput<KeyCode>>();  // bypass InputPlugin
// press, app.update(), assert — works because keyboard_input_system isn't registered
```

This is the Feature #2 pattern, documented in `.claude/agent-memory/implementer/feedback_bevy_test_input_setup.md`. The bypass works because there's no `keyboard_input_system` clearing the press in PreUpdate.

**Layer 2 — Code reads `Res<ActionState<T>>` from leafwing (Feature #5+):**

```rust
app.add_plugins((MinimalPlugins, StatesPlugin, InputPlugin, ActionsPlugin));
// FULL InputPlugin REQUIRED — leafwing's update reads ButtonInput<KeyCode> AFTER
// keyboard_input_system populates it from KeyboardInput messages.
//
// Inject KeyboardInput MESSAGE (NOT direct ButtonInput::press, which races
// against keyboard_input_system's frame-start clear):
app.world_mut().resource_mut::<Messages<KeyboardInput>>().write(KeyboardInput {
    key_code: KeyCode::KeyW,
    logical_key: Key::Character("w".into()),
    state: ButtonState::Pressed,
    text: None, repeat: false,
    window: Entity::PLACEHOLDER,
});
app.update(); // keyboard_input_system reads message → fills ButtonInput → leafwing reads

// Or, if the resolved leafwing version has MockInput:
// app.send_input(KeyCode::KeyW);
// app.update();
```

The Feature #2 bypass pattern (`init_resource::<ButtonInput<KeyCode>>()`) does NOT work for Layer 2 — leafwing depends on `keyboard_input_system` running. Direct `ButtonInput::press` ALSO does not work — `keyboard_input_system`'s frame-start `clear()` wipes the press before leafwing reads it.

**Layer 3 — System validation but no real input testing:**

When `--features dev` registers `cycle_game_state_on_f9`, even tests that don't exercise F9 still need `ButtonInput<KeyCode>` as a resource for system-param validation:

```rust
#[cfg(feature = "dev")]
app.init_resource::<ButtonInput<KeyCode>>();
```

This is the bridge between Layer 1 (the direct-read code path) and the test that doesn't care about input — registered system needs the resource to exist.

**Why:** Bevy 0.18's `keyboard_input_system` (verified at `bevy_input-0.18.1/src/keyboard.rs:163-198`) calls `keycode_input.bypass_change_detection().clear()` at the top of every PreUpdate. Manual `ButtonInput::press` calls survive only if no `keyboard_input_system` runs between the press and the read.

**How to apply:** When designing tests for input-driven systems in Druum, identify which layer:
- Direct `ButtonInput<KeyCode>` consumer (rare; F9 dev hotkey is the only one) → Layer 1
- `ActionState<T>` consumer (gameplay, post-Feature-#5) → Layer 2 with full InputPlugin
- Test doesn't exercise input but the app has dev-cfg systems registered → Layer 3 (init_resource for validation)

Mixing patterns produces "compiles, runs, asserts wrong" — flag in code review.
