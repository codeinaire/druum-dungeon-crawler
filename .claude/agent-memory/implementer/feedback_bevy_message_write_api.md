---
name: Bevy 0.18 — write message in test via resource_mut<Messages<T>>
description: app.world_mut().write_message(T) does not exist; use resource_mut::<Messages<T>>().write(ev)
type: feedback
---

`app.world_mut().write_message(T)` does NOT exist in Bevy 0.18. The correct pattern for writing messages in test helpers is:

```rust
app.world_mut()
    .resource_mut::<bevy::ecs::message::Messages<MyEvent>>()
    .write(MyEvent { ... });
```

Similarly, `messages.len()` does not exist on `Messages<T>`. Use `.iter_current_update_messages().count()`.

**Why:** Discovered during Feature #14 implementation. The plan's test templates used the non-existent API.

**How to apply:** Any test helper that needs to write a message without going through a system should use `resource_mut::<Messages<T>>().write(ev)`. Create a helper function like:

```rust
fn write_my_event(app: &mut App, ev: MyEvent) {
    app.world_mut()
        .resource_mut::<bevy::ecs::message::Messages<MyEvent>>()
        .write(ev);
}
```

This mirrors the established pattern in `features.rs::write_moved` (see that function as the canonical example).
