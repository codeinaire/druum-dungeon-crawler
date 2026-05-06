---
name: Druum #11 Equipment + derive_stats contract (Feature #12+ must mirror, not redesign)
description: Feature #11 froze a specific shape that downstream features (#12 inventory, #14 progression, #15 combat, #23 save/load) must consume — Equipment uses Handle<ItemAsset>, derive_stats is pure and takes &[ItemStatBlock], ItemInstance entity model is the documented home for per-instance state
type: reference
---

Feature #11 (party + character ECS model) shipped specific decisions that #12 and beyond must consume, NOT redesign. The roadmap §12 text suggests `Equipment` uses `Entity` — that text is **stale**; the actual code uses `Handle<ItemAsset>`.

**Why:** #11's research and plan both surfaced Decision 3 (Equipment = Handle vs Entity) as Category B; the user resolved Handle. The decision is documented inline in the code at `src/plugins/party/character.rs:194-208` so future researchers find it. Reverting requires explicit user re-litigation, not a quiet redesign.

**How to apply (file:line references — verified 2026-05-05):**

1. **`Equipment` is `Option<Handle<ItemAsset>>` per slot, NOT `Entity`** — `src/plugins/party/character.rs:209-219`. Eight slots: `weapon`, `armor`, `shield`, `helm`, `gloves`, `boots`, `accessory_1`, `accessory_2`. **#12 does NOT redefine this component**; it consumes it.

2. **`derive_stats` is PURE and takes `&[ItemStatBlock]`, NOT `&Equipment`** — `src/plugins/party/character.rs:343-348`. The flatten step (read `Equipment` slots → look up handles in `Assets<ItemAsset>` → extract `ItemStatBlock`s → push to a Vec) is the **caller's** responsibility. The function does not access ECS world or assets. **#12's `recompute_derived_stats_on_equipment_change` system owns the flatten step.** Do NOT modify `derive_stats`.

3. **Per-instance state lives on a separate entity** — doc comment at `character.rs:204-205`: *"Per-instance state (enchantment, durability, custom name) lands in #12 as a separate `ItemInstance` entity model."* This is the contract: the `Equipment` slot stores the static-asset handle; the inventory `Vec<Entity>` carries entities with `ItemInstance(Handle<ItemAsset>)` plus optional per-instance components.

4. **`Equipment` cannot derive `Serialize`/`Deserialize`** — `Handle<T>` doesn't implement serde in Bevy 0.18 (verified by absence of serde derive on Equipment vs presence on the other 11 components). Doc comment at `character.rs:196-202` documents this. Save/load (#23) must implement custom serde mapping `Handle<ItemAsset>` ↔ `AssetPath`. **Do NOT add a stub `Serialize` for `Equipment` in #12 — it won't compile.**

5. **Caller-clamp pattern for `current_*` stats** — `derive_stats` returns `current_hp = max_hp` (`character.rs:295-298`). The recompute system in #12 must clamp `current = current.min(new_max)` AFTER calling `derive_stats` to preserve in-combat depletion. Equipping a +20HP amulet does NOT heal you — Wizardry/Etrian convention.

6. **`PartyPlugin::build` registration pattern is locked** — `src/plugins/party/mod.rs:18-50`. New types added in #12 must mirror: `app.register_type::<T>()` for each Reflect-derived type, `app.add_message::<T>()` for each Message, gate dev-only systems on `#[cfg(feature = "dev")]`.

7. **The `Class` enum at `character.rs:62-72` has 8 variants but only 3 (Fighter/Mage/Priest) are authored** in `core.classes.ron`. **Never add an exhaustive `match Class { ... }` without a wildcard arm** — the doc comment at lines 56-58 is explicit: "Use `Option` returns and wildcard arms rather than exhaustive `match`."

8. **`StatusEffectType` v1 variants are pure gates** — `character.rs:235-243`. None modify stats via `magnitude`. Adding magnitude-modifying buffs (#15) requires re-evaluating order-independence; the deferred test `derive_stats_status_order_independent` should be written then.

**Where to grep for the live shape:**
- `src/plugins/party/character.rs` — types + derive_stats
- `src/data/items.rs` — ItemAsset, ItemStatBlock, ItemDb (stub bodies #12 fleshes out)
- `src/data/classes.rs` — ClassTable, ClassDef (#11 frozen-from-day-one precedent for #12 to mirror)
- `src/plugins/party/mod.rs` — PartyPlugin::build registration shape

**Anti-patterns to avoid in any future feature touching this area:**
- Reverting `Equipment` to `Option<Entity>` — would break #11's resolved Decision 3 and force `MapEntities` plumbing.
- Modifying `derive_stats` to take `&Equipment` instead of `&[ItemStatBlock]` — breaks unit-test isolation and the doc-comment contract.
- Adding `Serialize`/`Deserialize` to `Equipment` — won't compile (Handle<T> doesn't impl serde).
- Adding `Inventory` or `ItemInstance` derives that include `Serialize`/`Deserialize` — same Handle<T> issue + Vec<Entity> needs MapEntities.
- Pre-creating `progression.rs` or other party submodule files — single-file precedent (Decision 4 of #11) says new files land with their first system.
