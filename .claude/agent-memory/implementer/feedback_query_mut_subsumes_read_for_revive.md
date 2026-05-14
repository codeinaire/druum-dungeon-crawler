---
name: query-mut-subsumes-read-for-revive
description: When a type-alias query needs both read (snapshot) and mut (Revive exception) access to the same component, use &mut in the alias to avoid B0002
metadata:
  type: feedback
---

When a system type-alias query (e.g., `CombatantCharsQuery`) contains `&StatusEffects` for snapshot-building, and you also need `get_mut()` on the same component for a rare mutation (like the Revive spell exception), having both `Query<&StatusEffects>` (via the alias) and a separate `Query<&mut StatusEffects>` in the same system causes Bevy B0002 mutable aliasing.

**Fix:** Promote the alias to `&'static mut StatusEffects`. Using `&mut` subsumes `&`:
- `.iter()` on the mutable query yields shared `Ref<T>` references — safe for snapshot-building via `.clone()`.
- `.get_mut(target)` on the same query handles the mutation path.
- No separate `Query<&mut StatusEffects>` needed, avoiding B0002.

**Why:** Bevy's system conflict checker sees two queries accessing the same component type — one shared, one exclusive. Even if the code paths are mutually exclusive at runtime, the system scheduler detects the static conflict. Using `&mut` everywhere gives the system exclusive access, which is then available for both read (via Ref) and write paths.

**How to apply:** Any time a snapshot-building loop and a rare mutation arm both need the same component in one system, use `&mut` in the query type alias. Document with a comment: "`&mut` subsumes `&`; iterator yields shared Ref, `.get_mut()` enables the exception arm."

**Example (Feature #20):** `CombatantCharsQuery` promoted from `&'static StatusEffects` to `&'static mut StatusEffects` to allow `CastSpell::Revive` to call `chars.get_mut(target)` directly without B0002.

Related: [[bevy-b0002-query-split-pattern]]
