---
name: Bevy system parameters (MessageWriter, etc.) cannot be constructed outside a system context in tests
description: Functions that take MessageWriter<T> as param must be scheduled as systems via add_systems — cannot be called directly in tests
type: feedback
---

Bevy system parameters like `MessageWriter<T>`, `MessageReader<T>`, `Query<...>`, `Res<T>` cannot be manually constructed outside a running system. This means you cannot call a function that takes `&mut MessageWriter<ApplyStatusEvent>` directly from test code.

**Pattern to test such functions:** Register a thin wrapper system via `app.add_systems(Update, my_wrapper_system)` where the wrapper has the required system parameters and calls the function under test:

```rust
fn test_system_wrapper(
    party: Query<(Entity, &DerivedStats), With<PartyMember>>,
    mut writer: MessageWriter<MyEvent>,
) {
    for (entity, derived) in &party {
        function_under_test(entity, derived, &mut writer);
    }
}

#[test]
fn test_my_function() {
    let mut app = make_test_app();
    app.add_systems(bevy::app::Update, test_system_wrapper);
    // spawn entities, app.update(), assert...
}
```

**Why:** Discovered during Feature #14 when `check_dead_and_apply` tests had to be moved from `mod tests` (Layer-1) to `mod app_tests` (Layer-2) because the function signature includes `&mut MessageWriter<ApplyStatusEvent>`.

**How to apply:** Any `pub fn` that takes Bevy system parameters cannot be unit-tested directly. Always wrap in a test system.
