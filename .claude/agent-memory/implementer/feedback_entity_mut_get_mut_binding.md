---
name: Bevy entity_mut().get_mut() — two named bindings required to avoid E0716
description: app.world_mut().entity_mut(id).get_mut() chain drops EntityWorldMut before stats is used; split into two let bindings
type: feedback
---

The following pattern fails to compile with E0716 (temporary value dropped while borrowed):

```rust
// BROKEN — E0716
let mut stats = app.world_mut().entity_mut(entity).get_mut::<MyComponent>().unwrap();
stats.field = value;
```

The call chain: `world_mut()` returns `&mut World`; `entity_mut(id)` returns `EntityWorldMut<'_>` borrowing from `World`; `get_mut()` returns `Mut<'_, MyComponent>` borrowing from `EntityWorldMut`. The `EntityWorldMut` temporary is freed at the `;` but `stats` still holds a borrow.

**Why:** Rust's borrow checker sees the `EntityWorldMut` as a temporary with no named owner, freed at statement end. The resulting `Mut<T>` reference then points to freed memory.

**How to apply:** Always use two named bindings in tests:
```rust
// CORRECT
let world = app.world_mut();
let mut entity_ref = world.entity_mut(entity);
let mut stats = entity_ref.get_mut::<MyComponent>().unwrap();
stats.field = value;
```
The scoped block `{ ... }` is still needed around this to release the borrow before `app.update()`.
