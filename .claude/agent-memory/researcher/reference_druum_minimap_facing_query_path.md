---
name: Druum minimap reads &Facing directly via Query — same-frame UX update is automatic
description: Minimap painter reads Facing from a Query<&Facing>, not from MovedEvent payload — spinner-induced facing changes reflect on the minimap on the SAME commit frame, free
type: reference
---

In `src/plugins/ui/minimap.rs:269, 309, 318`, both painters (`paint_minimap_overlay`, `paint_minimap_full`) take a SystemParam:
```rust
party: Query<(&GridPosition, &Facing), With<PlayerParty>>
```

They read `&Facing` from the live ECS state, NOT from `MovedEvent.facing` payload. The paint runs in `EguiPrimaryContextPass` which fires AFTER `Update` schedule completes.

**Implication for Feature #13 (cell features):**

When `apply_spinner` mutates `facing.0 = new_random_direction` in the `Update` schedule, the minimap painter reads the NEW facing on the same frame. **Zero changes to minimap.rs needed for spinner.** No need for a `SpunEvent` or to publish a synthetic `MovedEvent` from the spinner.

This is the "auto-map updates post-rotation" requirement (Roadmap §13 Resolved #4) — already correct by ordering.

**Same applies to other systems that mutate `Facing` mid-Update** (cross-floor teleport with `target.facing: Some(direction)`, etc.) — the minimap reflects the change instantly.

**Caveat:** if a future change makes the minimap read `MovedEvent.facing` from the payload INSTEAD of from a Query, this property breaks. Audit the minimap source before assuming this still holds.
