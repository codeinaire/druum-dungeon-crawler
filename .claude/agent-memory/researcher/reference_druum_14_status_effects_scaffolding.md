---
name: Druum #14 status-effects scaffolding pre-shipped by #11
description: ALL data types (StatusEffectType, ActiveEffect, StatusEffects, .has(), serde+Reflect derives) shipped in #11; #14 is wiring + 5 new enum variants, NOT designing
type: reference
---

Feature #14 (Status Effects) lands on top of #11's complete data layer. Before designing #14, know what is already shipped:

**Data types — ALL present at `src/plugins/party/character.rs:235-274`:**
```rust
pub enum StatusEffectType { Poison, Sleep, Paralysis, Stone, Dead }   // 5 variants
pub struct ActiveEffect { effect_type, remaining_turns: Option<u32>, magnitude: f32 }
pub struct StatusEffects { pub effects: Vec<ActiveEffect> }
impl StatusEffects { pub fn has(&self, kind: StatusEffectType) -> bool }
```

`StatusEffects` is `#[derive(Component, Reflect, Serialize, Deserialize, Default, Debug, Clone, PartialEq)]` — save-clean out of the box. **No #23 carve-out needed.**

`PartyMemberBundle` already includes `status_effects: StatusEffects` at `character.rs:296`. Every party member spawned by #11 has it.

**Type registrations — already in `party/mod.rs:44-47`:**
```rust
.register_type::<StatusEffects>()
.register_type::<ActiveEffect>()
.register_type::<StatusEffectType>()
```

`#[derive(Reflect)]` on the enum auto-covers new variants — adding `AttackUp` etc. needs zero registration changes.

**`derive_stats` integration is already present at `character.rs:343-426`:** the function takes `&StatusEffects` and already has a `StatusEffectType::Dead` branch zeroing `max_hp/max_mp` (lines 403-407). The doc-comment at lines 340-342 explicitly names #14/#15 as the place to add buff branches.

**`recompute_derived_stats_on_equipment_change` at `inventory.rs:421-481`** already passes `&StatusEffects` into `derive_stats`. The "buff branches added → buff effect applied" pipeline works automatically IF #14's `apply_status_handler` writes `EquipmentChangedEvent` to trigger the re-derive (D5α pattern). This is a one-line addition, not a new system.

**`apply_poison_trap` at `features.rs:412-445`** is the refactor target. Current code: `effects.push(ActiveEffect { ... })` (D12 naive push, comment says "stacking deferred to #14"). #14 refactors to `MessageWriter<ApplyStatusEvent>` write — the comment is the explicit hook.

**The roadmap (line 779) contradicts the doc-comment at `character.rs:230-231`:** roadmap says #14 owns `AttackUp/DefenseUp/SpeedUp/Regen`; comment says "Buffs ... are deferred to #15". Roadmap is the authority — #14 owns the buff variants.

**Save-format stability (Decision 7 of #11 at `character.rs:228-230`)**: discriminant order is locked. New variants MUST go at end (after `Dead`), or every existing save's serialized status_effect index shifts.

**How to apply:** When planning #14, do NOT redesign the data shape. The `ActiveEffect.magnitude` field is the canonical "potency" field — do NOT add a separate `potency` field. Do NOT add a `source: Option<Entity>` field (YAGNI). The roadmap's spec for `ApplyStatusEvent { target, effect, potency, duration }` is exactly right with one refinement: `duration: Option<u32>` (matches `remaining_turns` shape, supports permanent-cure effects).
