---
name: dev feature — ButtonInput<KeyCode> required in any test app that includes StatePlugin
description: Any test app built with StatePlugin + --features dev must init_resource::<ButtonInput<KeyCode>>() to avoid "Resource does not exist" panic
type: feedback
---

Any `make_test_app()`-style helper that adds `StatePlugin` must also call:

```rust
#[cfg(feature = "dev")]
app.init_resource::<ButtonInput<KeyCode>>();
```

**Why:** Under `--features dev`, `StatePlugin::build` registers `cycle_game_state_on_f9` which requires `ButtonInput<KeyCode>`. Without this init, all tests in the app panic with "Resource does not exist" when run with `cargo test --features dev`.

**How to apply:** Any time a test helper adds `StatePlugin` (or any plugin that transitively adds it), add the cfg-gated `init_resource` call immediately before the first `app.update()`. Do NOT add `InputPlugin` — that registers `keyboard_input_system` which clears `just_pressed` in PreUpdate before Update, breaking F9-style tests.

Established pattern: `src/plugins/state/mod.rs:107`, `src/plugins/audio/mod.rs` (Feature #6).
