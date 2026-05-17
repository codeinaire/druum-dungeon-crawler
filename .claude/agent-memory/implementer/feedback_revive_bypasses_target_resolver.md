---
name: revive-bypasses-target-resolver
description: Revive spells must use action.target directly, not resolve_target_with_fallback — it filters out dead entities which are Revive's intended targets
metadata:
  type: feedback
---

`resolve_target_with_fallback` (at `combat/targeting.rs`) uses `is_alive_entity` to filter valid targets and re-routes to a live fallback if the primary target is dead. For healing/damage/status spells this is correct. For Revive, the dead entity IS the intended target — the fallback routing breaks the spell.

**Fix:** The Revive arm reads `action.target` directly via a `match` on `TargetSelection`:
```rust
let revive_targets: Vec<Entity> = match &action.target {
    TargetSelection::Single(t) => vec![*t],
    TargetSelection::AllAllies => party_entities.iter().collect(),
    _ => targets.clone(), // fallback for edge cases
};
```

Apply an `is_dead` defense-in-depth check per entity before reviving (do not revive an already-alive entity).

**Why:** This mirrors the Temple revive path at `temple.rs` which also bypasses the alive-entity filter and directly accesses the dead character.

**How to apply:** Any future spell or ability that targets dead entities (e.g., Raise, Resurrect, Animate Dead) must bypass the standard `resolve_target_with_fallback` call and read `action.target` directly.
