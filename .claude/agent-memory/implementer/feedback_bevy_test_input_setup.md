---
name: Bevy 0.18 test setup — init_resource vs InputPlugin for ButtonInput<KeyCode>
description: When testing Bevy systems that require ButtonInput<KeyCode>, add InputPlugin only if you want realistic clearing behavior; use init_resource directly for simpler unit tests where just_pressed must survive to Update
type: feedback
---

In Bevy 0.18 unit tests, do NOT add `InputPlugin` when you need to manually set `just_pressed` on `ButtonInput<KeyCode>` and have it observed in the same `app.update()` call.

**Why:** `InputPlugin` registers `keyboard_input_system` in `PreUpdate`, which calls `keycode_input.bypass_change_detection().clear()` at the top of every frame. This clears `just_pressed` before `Update` runs, so any manually-set press state is erased before your system sees it.

**How to apply:** In test apps that need `ButtonInput<KeyCode>` without the clearing behavior, use:
```rust
app.init_resource::<ButtonInput<KeyCode>>();
```
instead of `app.add_plugins(InputPlugin)`. The resource is present for system parameter validation and manual manipulation, but `keyboard_input_system` is not registered.

Note: this also applies when dev-gated systems (e.g. `cycle_game_state_on_f9`) are registered via `StatePlugin::build` under `#[cfg(feature = "dev")]`. Even tests that don't exercise those systems need the resource initialized, because Bevy 0.18 validates all registered system parameters on every `app.update()`.
