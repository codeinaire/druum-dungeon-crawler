---
name: leafwing-input-manager 0.20 — ActionState Resource requires explicit init
description: InputManagerPlugin does NOT auto-insert ActionState as a Resource; must call init_resource explicitly alongside insert_resource for InputMap
type: feedback
---

In `leafwing-input-manager 0.20.0`, `InputManagerPlugin::<T>` does NOT automatically insert `ActionState<T>` as a Resource. The `update_action_state` system takes `action_state: Option<ResMut<ActionState<A>>>` — it only reads the resource if it already exists.

**Why:** The plugin supports both Resource-based (single-player) and Component-based (per-entity, co-op) ActionState simultaneously. It leaves insertion to the caller. The `action_state_resource` example confirms: call `init_resource::<ActionState<T>>()` explicitly.

**How to apply:** In any plugin that registers `InputManagerPlugin::<T>`, also call:
```rust
app.init_resource::<ActionState<MenuAction>>()
   .init_resource::<ActionState<DungeonAction>>()
   .init_resource::<ActionState<CombatAction>>()
```
If you omit this, `ActionState<T>` will not be a Resource and `app.world().resource::<ActionState<T>>()` will panic.
