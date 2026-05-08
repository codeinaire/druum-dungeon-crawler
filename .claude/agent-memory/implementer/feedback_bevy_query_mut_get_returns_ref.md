---
name: Bevy Query<&mut T>::get() returns Ref<T> not T
description: Query<&mut T>::get(&self, entity) returns Ref<T> which is not Copy; use .map(|r| *r) not .copied()
type: feedback
---

`Query<&mut T>::get(&self, entity)` returns `Result<Ref<T>, QueryEntityError>`. `Ref<T>` does NOT implement `Copy`, even when `T: Copy`.

Calling `.copied()` on `Result<Ref<T>, E>` is a type error.

**Why:** Bevy's `Ref<T>` is a change-detection wrapper; it tracks whether the value was accessed. It deliberately omits `Copy` so callers can't silently bypass the change-detection protocol.

**How to apply:** When you need a owned copy of a value from `Query<&mut T>::get()`, use:
```rust
derived_mut.get(e).map(|r| *r).unwrap_or_default()
```
The `*r` dereferences `Ref<T>` to `T`, and `T: Copy` allows the copy. This is safe because we're explicitly choosing to discard the change-detection wrapper.
