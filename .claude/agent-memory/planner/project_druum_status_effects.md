---
name: Druum status effects (Feature #14) decisions
description: Feature #14 architecture — status_effects.rs sub-plugin, ApplyStatusEvent/StatusTickEvent shapes, EquipmentChangedEvent reuse via EquipSlot::None sentinel, append-only enum extension, resolver-vs-ticker ordering invariant
type: project
---

# Feature #14 Status Effects — planning decisions (frozen at plan time, 2026-05-07)

The full plan: `project/plans/20260507-124500-feature-14-status-effects-system.md`.

## Architecture decisions (locked)

- **D3-α — Module placement:** `src/plugins/combat/status_effects.rs` + new `StatusEffectsPlugin` registered as a sub-plugin of `CombatPlugin` from `combat/mod.rs::CombatPlugin::build` via `app.add_plugins(StatusEffectsPlugin)`. Mirrors `CellFeaturesPlugin` precedent. `main.rs` unchanged.
- **D4-α — Tick architecture:** Single `StatusTickEvent` Message; #14 owns the dungeon-step emitter (`tick_on_dungeon_step` reads `MovedEvent`). #15 will add a one-line combat-round emitter in `turn_manager.rs::round_end`. Decoupled.
- **D5α — Re-derive trigger:** `apply_status_handler` writes `EquipmentChangedEvent { character, slot: EquipSlot::None }` for stat-affecting variants (`AttackUp/DefenseUp/SpeedUp/Dead`). Reuses 100% of `recompute_derived_stats_on_equipment_change` (`inventory.rs:421`) including its caller-clamp. **`EquipSlot::None` already exists at `inventory.rs:120`** — used as the "stat-changed, source not an equipment slot" sentinel.
- **`tick_status_durations` also writes `EquipmentChangedEvent`** when an expiring effect was a stat-modifier (so derive_stats re-runs without buff).
- **Append-only enum extension (Pitfall 5 of #14, Decision 7 of #11):** 5 new variants AFTER `Dead` at `character.rs:236-243`: `AttackUp(5), DefenseUp(6), SpeedUp(7), Regen(8), Silence(9)`. **`Blind`/`Confused` deferred to #15** (no readers in #14 — declaring them now burns save-format slots speculatively).
- **Comment marker locked:** `// HISTORICAL APPEND ORDER — DO NOT REORDER. New variants go at end.` directly above the enum at line 235. Plus a regression test `status_effect_type_dead_serializes_to_index_4` that pins discriminants via `as u8` cast — reorder fails CI.
- **`derive_stats` buff branches BEFORE `Dead` (Pitfall 4):** insert at line 399, between `// ── Status effect post-pass` comment block at line 400 and `Dead` branch at line 403. Locals at lines 385/386/389 (`stat_attack`, `stat_defense`, `stat_speed`) become `let mut`. `Dead` branch becomes LAST with comment `// LAST: zero-out dominates all buffs above.`
- **Buff `magnitude` = multiplier** (`AttackUp 0.5` = +50% attack). Saturating arithmetic: `let bonus = (stat_attack as f32 * effect.magnitude) as u32; stat_attack = stat_attack.saturating_add(bonus);`
- **NaN clamp at trust boundary (Pitfall 6):** `apply_status_handler` calls `let potency = ev.potency.clamp(0.0, 10.0);` BEFORE any merge or push.

## Stacking semantics (D2 — locked)

- Same effect already present → refresh `remaining_turns`, take `existing.magnitude.max(new.potency)`.
- Permanent (Stone/Dead) re-application → no-op.
- `apply_status_handler` is the SOLE mutator of `StatusEffects.effects`. Decision 20: zero callers of `effects.push(...)` outside the handler/ticker pair.

## System ordering (Critical / Open Item 7)

- **Resolver-vs-ticker invariant:** `apply_poison_damage.before(tick_status_durations)` and `apply_regen.before(tick_status_durations)`. Otherwise a duration-1 poison gets removed before dealing its final tick of damage.
- **Emitter-vs-readers:** `tick_on_dungeon_step.before(tick_status_durations).before(apply_poison_damage).before(apply_regen)`.
- **Cross-plugin writer ordering:** `apply_poison_trap.before(apply_status_handler)` registered in `CellFeaturesPlugin::build`. Same-frame consumability for trap → effect.
- `apply_status_handler` ungated (message-driven idles when no events) — matches `apply_poison_handler` pattern from #13.

## Predicate shape (D11 — locked)

- `pub fn is_paralyzed/is_asleep/is_silenced(s: &StatusEffects) -> bool` in `combat/status_effects.rs`. NOT systems.
- `is_blind`/`is_confused` deferred to #15 with their consumers.
- **`pub fn check_dead_and_apply(target, &derived, &mut writer)` stub shipped in #14** for #15 to import; writes `ApplyStatusEvent { effect: Dead, ... }` if `derived.current_hp == 0`. **#14 does NOT auto-apply Dead** (Pitfall 7 — defer combat genre rules to #15).

## Cross-plugin refactor target

- **`apply_poison_trap` at `dungeon/features.rs:412-445`:** signature change drops `&mut StatusEffects` query, gains `MessageWriter<ApplyStatusEvent>`. Body writes `ApplyStatusEvent { potency: 1.0, duration: Some(5) }` per party member. **`POISON_TRAP_POTENCY = 1.0` (NOT 0.0)** — Risk 3: `magnitude == 0.0` would silently produce zero damage with the % formula.
- Existing test `poison_trap_applies_status` (`features.rs:938-978`) needs `app.update()` × 2 (Pitfall 1: cross-plugin same-frame consumability via `.before(apply_status_handler)` is best-effort, two updates is safer).

## D7 / D9 — runtime tuning, both default to A

- **D7 (poison/regen formula) — default A:** `damage = ((max_hp / 20).max(1) as f32 * magnitude) as u32; damage.max(1);` — % of max (5% at magnitude 1.0). Two `.max(1)` floors guard low-HP and low-magnitude truncation. Wizardry-canonical.
- **D9 (dungeon-step tick freq) — default A:** every step (one tick per `MovedEvent` per `PartyMember`). Wizardry-canonical.
- Both surfaced as user-overridable in plan's `## Open Questions`. Swap is a localized formula/system change; no architectural impact.

## Test plan (LOC + count budget)

- LOC: ~440 (within +350-500 envelope).
- Tests: 4 Layer-1 (`combat/status_effects::tests`) + 12 Layer-2 (`combat/status_effects::app_tests`) + 4 new in `character::tests` (discriminant lock, AttackUp buff, order-independence, Dead dominance) = 20 new tests. Above the +8-12 envelope; trim 5-7 lower-value if budget tightens. Highest-value: 5 stacking + 3 resolver + 2 derive_stats invariant.
- `make_test_app_with_dungeon` (separate harness from the no-DungeonPlugin one) needed for the dungeon-step end-to-end test.

## What #14 does NOT ship (deferred)

- UI status icons → #25.
- Save-plugin work → #23.
- Combat-round emitter of `StatusTickEvent` → #15 (one line in `turn_manager::round_end`).
- `Blind`/`Confused` enum variants AND predicates → #15.
- `Dead`-on-zero-HP application → #15 (calls `check_dead_and_apply`).
- Action-blocking wiring → #15 (calls `is_paralyzed/is_asleep/is_silenced`).
- Ailment-curing item/spell → #20.
- Magic resistance / saving throws → #15.
- Status effects on enemies → #15 (broadens query from `With<PartyMember>`).
