---
name: Druum recompute_derived_stats_on_equipment_change filters With<PartyMember> — drop for enemy buffs
description: The dual-use re-derive pipeline established by #14 (D5α) currently filters With<PartyMember>; #15 needs to drop the filter so enemy DefenseUp/Dead reflect in stats
type: reference
---

The Druum dual-use re-derive pipeline (Feature #14 D5α) routes both equipment changes AND status effect changes through `EquipmentChangedEvent` and `recompute_derived_stats_on_equipment_change` (`inventory.rs:434-494`).

**The filter:** `Query<..., With<PartyMember>>` at `inventory.rs:445`.

**The impact for #15+:** When an enemy gets `DefenseUp` (boss buff spell), `Dead` (we want max_hp = 0), or any other stat-affecting status, the recompute system filters by `With<PartyMember>` and skips. The enemy's `DerivedStats` does not reflect the buff/debuff.

**Why this exists:** #11 / #12 only had party members carrying `Equipment + Inventory + Experience`. The filter was correct at the time.

**The fix for #15 (recommended D-K1):** Drop the `With<PartyMember>` filter. Enemies must spawn with `Equipment::default()` (8 None slots) and `Experience::default()` for the query shape to match. The flatten step iterates 8 None slots and does nothing — cheap.

**Cost:** ~16 bytes per enemy entity (Equipment + Experience are small structs). Carve-out edit on a #12-frozen file (`inventory.rs:445`).

**Alternative (D-K2):** Define a SECOND recompute system for enemies. Adds ~50 LOC; enemies have a slimmer query.

**Recommendation:** D-K1 (drop the filter) is the cleaner ship. Single re-derive code path; buffs work for all combatants. Doc-comment the filter drop to acknowledge dual-use ("party AND enemies share the recompute path; enemies have empty Equipment so flatten is a no-op").

**How to apply:** when researching/planning any feature that introduces enemy entities (#15 combat, #17 enemy DB), explicitly call out that `recompute_derived_stats_on_equipment_change`'s filter must be reconsidered. The frozen-file carve-out for #12's `inventory.rs:445` is the location.
