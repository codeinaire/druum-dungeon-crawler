---
name: Druum EquipmentChangedEvent is the canonical "stat-changed" trigger
description: recompute_derived_stats_on_equipment_change subscribes to EquipmentChangedEvent and reads &StatusEffects — meaning ANY status-mutator can fire it to trigger a re-derive
type: reference
---

`recompute_derived_stats_on_equipment_change` at `src/plugins/party/inventory.rs:421-481` is named for equipment but is structurally a "re-run derive_stats" system:

```rust
pub fn recompute_derived_stats_on_equipment_change(
    mut events: MessageReader<EquipmentChangedEvent>,
    items: Res<Assets<ItemAsset>>,
    mut characters: Query<
        (&BaseStats, &Equipment, &StatusEffects, &Experience, &mut DerivedStats),
        With<PartyMember>,
    >,
) { ... }
```

It reads `&StatusEffects` and passes it to `derive_stats(...)`. **This means any system that mutates `StatusEffects` can fire `EquipmentChangedEvent { character, slot: EquipSlot::None }` to trigger a re-derive that picks up the status change** — without needing a separate event type or a separate system.

The caller-clamp pattern at `inventory.rs:475-479` is preserved:
```rust
let old_current_hp = derived.current_hp;
let old_current_mp = derived.current_mp;
*derived = new;
derived.current_hp = old_current_hp.min(derived.max_hp);
derived.current_mp = old_current_mp.min(derived.max_mp);
```

— meaning equipping a high-VIT armor doesn't refill HP, and applying `AttackUp` doesn't accidentally bump current_hp upward either.

**The `slot: EquipSlot::None` value** is an existing sentinel ("no field maps to None" per `EquipSlot::write` at `inventory.rs:137-139`) — using it for the "stat-changed-but-not-equipment" case is a clean reuse of existing semantics. The system body's flatten-equipment loop is wasted work for status-only changes (it iterates 8 always-empty Equipment slots) but the cost is negligible.

**Naming caveat:** Once #14 lands, `EquipmentChangedEvent` is semantically a "stat-changed event". Update its doc-comment at `inventory.rs:194-202` to reflect dual-use; consider rename in #25 polish (not a blocker).

**How to apply:** When designing systems that mutate `DerivedStats` indirectly (#14 buffs, #15 leveling debuffs, #20 stat-altering consumables), prefer firing `EquipmentChangedEvent` over building a parallel re-derive pipeline. The existing system handles flatten + clamp + caller-clamp correctly; reuse don't duplicate.
