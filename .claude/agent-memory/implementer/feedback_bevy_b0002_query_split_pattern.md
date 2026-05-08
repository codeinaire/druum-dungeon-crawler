---
name: Bevy B0002 — split Query to avoid &T + &mut T in same system
description: Cannot have Query<&T> and Query<&mut T> for same T in one system; split by removing T from read query and using a separate exclusive Query<&mut T>
type: feedback
---

Bevy raises B0002 if a system has both `Query<..., &T, ...>` and `Query<&mut T>` for the same component type `T`. This applies even if the queries target different entity sets.

**Why:** Bevy's scheduler validates query access at compile/schedule time. Overlapping `&T` and `&mut T` accesses are always rejected to preserve aliasing safety.

**How to apply:** Remove `&T` from the read query entirely. Access `T` through the mutable query using `.get()` (which takes `&self` on `Query<&mut T>`).

**Pattern used in execute_combat_actions (Feature #15):**
```rust
// BAD — B0002:
chars: Query<(Entity, &DerivedStats, &StatusEffects, ...)>,
derived_mut: Query<&mut DerivedStats>,

// GOOD:
chars: Query<(Entity, &StatusEffects, &PartyRow, ...)>,  // no DerivedStats
derived_mut: Query<&mut DerivedStats>,                   // sole accessor

// Then pre-collect snapshots:
let snapshots: HashMap<Entity, Snapshot> = chars.iter().map(|(e, s, r, ...)| {
    let derived = derived_mut.get(e).map(|r| *r).unwrap_or_default();  // Copy via *r
    (e, Snapshot { derived, ... })
}).collect();
```

The pre-collection pattern is valid because `chars.iter()` only borrows `chars`, and `derived_mut.get(e)` only borrows `derived_mut`. They access different component tables (no shared components).
