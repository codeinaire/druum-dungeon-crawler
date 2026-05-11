---
name: Accumulator bump must precede asset-guard early returns
description: When a counter must increment on every event regardless of asset readiness, compute the optional asset handle BEFORE the event loop, bump inside the loop before any continue
type: feedback
---

In `check_random_encounter` (Feature #16), the `steps_since_last` counter must bump on every `MovedEvent` even when `DungeonAssets` are not yet loaded. The plan's initial design placed the asset guard BEFORE the bump, causing the counter to stay at 0 during early frames. 

Correct pattern:
```rust
// Resolve optional asset ONCE before the event loop (one lookup, not N)
let maybe_floor: Option<&DungeonFloor> = match dungeon_assets.as_ref() {
    Some(assets) => floors.get(floor_handle_for(assets, active_floor.0)),
    None => None,
};

for ev in moved.read() {
    // ALWAYS bump first — before any guard/continue
    state.steps_since_last = state.steps_since_last.saturating_add(1);
    
    // Asset guard comes AFTER the bump
    let Some(floor) = maybe_floor else { continue; };
    // ... rate checks, FOE check, roll
}
```

**Why:** The soft-pity accumulator contract requires the counter to reflect all steps taken, not just steps where the encounter roll was attempted. Tests assert the counter value after walking 100 cells through a rate-zero floor — if the bump is gated by asset readiness, this assertion fails.

**How to apply:** Whenever an accumulator must increment unconditionally per event, pre-resolve optional dependencies before the loop (avoid `Option<Res<T>>` inside the loop body), and place the increment at the very top of the loop body before any `continue` or early guard.
