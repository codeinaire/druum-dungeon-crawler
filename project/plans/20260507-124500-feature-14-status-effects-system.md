# Plan: Feature #14 — Status Effects System

**Date:** 2026-05-07
**Status:** Approved (2026-05-07) — User accepted defaults: D7=A (% of max HP poison/regen formula), D9=A (tick on every dungeon step)
**Research:** `project/research/20260507-115500-feature-14-status-effects-system.md`
**Roadmap:** `project/roadmaps/20260429-01-bevy-dungeon-crawler-roadmap.md` lines 741-786
**Depends on:** Feature #11 (status data types — `StatusEffectType`, `ActiveEffect`, `StatusEffects`, `derive_stats`), Feature #12 (`EquipmentChangedEvent` + `recompute_derived_stats_on_equipment_change`), Feature #13 (`apply_poison_trap` is the canonical refactor target)

---

## Goal

Build the duration-tracked status-effect resolution layer on top of the data types Feature #11 already shipped. Add the canonical `ApplyStatusEvent` message + `apply_status_handler` (the SOLE mutator of `StatusEffects.effects`); wire `tick_status_durations` and the per-effect resolvers (`apply_poison_damage`, `apply_regen`) to a `StatusTickEvent` message that #14 emits on every dungeon step (`MovedEvent`); extend `derive_stats` with `AttackUp/DefenseUp/SpeedUp` buff branches re-fired via the existing `EquipmentChangedEvent` recompute path (D5α); refactor `apply_poison_trap` from a naive `effects.push(...)` to an `ApplyStatusEvent` write. **Defers** to #15: the combat-round emitter of `StatusTickEvent`, the `Blind`/`Confused` enum variants and predicates, action-blocking wiring inside `turn_manager`, and `Dead`-on-zero-HP application.

---

## Approach

**Architecture D3-α (RECOMMENDED in research) — sub-plugin shape.** New file `src/plugins/combat/status_effects.rs` housing the entire feature. New struct `StatusEffectsPlugin: Plugin`. Registered as a sub-plugin of `CombatPlugin` from `combat/mod.rs::CombatPlugin::build` via `app.add_plugins(StatusEffectsPlugin)` — this matches the roadmap line 760 path and keeps `CombatPlugin` as the natural home for status logic (which #15's `turn_manager` will read). `main.rs` is unchanged (`CombatPlugin` is already registered there at `main.rs:28`).

**Architecture D4-α — single `StatusTickEvent` message, two emitters.** #14 owns the message AND the dungeon-step emitter (`tick_on_dungeon_step` reads `MovedEvent`, fires `StatusTickEvent { target }` per `PartyMember`). #15 will add a one-line emitter in `turn_manager.rs::round_end` without touching `StatusEffectsPlugin`. The single `tick_status_durations` system + per-effect resolvers (`apply_poison_damage`, `apply_regen`) all subscribe to `StatusTickEvent`. Decoupled cleanly via the same Bevy `Message<T>` primitive as `MovedEvent` / `EquipmentChangedEvent` / `TeleportRequested`.

**Architecture D5α — re-derive triggered via `EquipmentChangedEvent` reuse.** `apply_status_handler` writes `EquipmentChangedEvent { character: target, slot: EquipSlot::None }` for stat-affecting variants (`AttackUp/DefenseUp/SpeedUp/Dead`). Reuses 100% of the existing `recompute_derived_stats_on_equipment_change` pipeline (`inventory.rs:421-481`) including its caller-clamp on `current_hp/current_mp`. `EquipSlot::None` already exists at `inventory.rs:120` as the first variant — used as the "stat-changed, source not an equipment slot" sentinel. Doc-comment update on `EquipmentChangedEvent` (`inventory.rs:193-202`) acknowledges the dual-use.

**Append-only enum extension (Pitfall 5).** 5 new variants append AFTER `Dead` at `character.rs:236-243`: `AttackUp, DefenseUp, SpeedUp, Regen, Silence`. Discriminant indices 5-9. Comment marker `// HISTORICAL APPEND ORDER — DO NOT REORDER. New variants go at end.` above the enum. **`Blind` and `Confused` deferred to #15** — declaring them in #14 burns save-format slots speculatively (research D1).

**Stacking semantics (D2 — roadmap §14 line 780 verbatim with two refinements).** Same effect already present → refresh `remaining_turns`, take `existing.magnitude.max(new.potency)`. Permanent-cure effects (`Stone`/`Dead`) already present → re-application no-op. Buff `magnitude` IS the multiplier (e.g., `AttackUp 0.5` = +50% attack). The handler is the ONLY mutator; this is the policy enforcement point.

**Predicates as `pub fn`, not systems (research §K).** `pub fn is_paralyzed/is_asleep/is_silenced(s: &StatusEffects) -> bool` — same shape as `EquipSlot::read`/`StatusEffects::has`. #15 imports them from `combat/status_effects.rs` and calls them inside `turn_manager::collect_player_actions`. **Defer `is_blind`/`is_confused` to #15** (no consumers in #14; ships with the enum variants when they have readers).

**`apply_poison_trap` refactor (D12 → resolved).** At `features.rs:412-445`, replace the direct `effects.push(...)` (the comment at line 412 names this as "deferred to #14") with an `ApplyStatusEvent` write. `potency: 1.0` (NOT `0.0` — see Risk 3), `duration: Some(5)` (preserves existing `POISON_TURNS = 5`). System signature simplifies — drops `&mut StatusEffects` query, gains `MessageWriter<ApplyStatusEvent>`. Existing test `poison_trap_applies_status` (`features.rs:938-978`) needs `app.update()` called twice to flush trap-write → handler-read (Pitfall 1).

**Single-file plugin precedent.** Same shape as `CellFeaturesPlugin` (`features.rs`) — 2 messages + 1 plugin + 5 systems + 3 predicates + tests in one file (~340 LOC). Total LOC budget: ~440 (within +350-500 envelope). Total test count: 11 new + 1 existing-needing-edit = 12 tests (within +8-12 envelope).

---

## Critical

These are non-negotiable constraints. Violations should fail review:

- **Bevy `=0.18.1` pinned.** No version bump. Zero new dependencies.
- **`#[derive(Message)]`, NOT `#[derive(Event)]`** — Bevy 0.18 family rename. `MessageReader<T>` / `MessageWriter<T>` / `app.add_message::<T>()`. Verification gate greps for `derive(Event)` / `EventReader<` / `EventWriter<` in `combat/status_effects.rs` — must return ZERO matches.
- **Append-only `StatusEffectType` extension (Pitfall 5).** All new variants go AFTER `Dead`. Existing indices 0-4 (`Poison/Sleep/Paralysis/Stone/Dead`) are LOCKED per Decision 7 of #11 (`character.rs:228-230`). Save-format breakage is unrecoverable for existing test fixtures.
- **`Dead` branch LAST in `derive_stats` (Pitfall 4).** The existing `if status.has(StatusEffectType::Dead)` branch at `character.rs:403-407` zeros `max_hp/max_mp`. Buff branches MUST be inserted BEFORE this branch so `Dead`'s zero-out dominates. Comment-tag `Dead` as `// LAST: zero-out dominates all buffs above.`
- **NaN clamp at trust boundary (Pitfall 6).** `apply_status_handler` MUST `clamp(0.0, 10.0)` incoming `ApplyStatusEvent.potency` BEFORE any merge or push. Same saturating-arithmetic pattern as `character.rs:374-381`. `f32::NAN` propagation through `derive_stats` produces NaN-comparisons that read as `false`, breaking combat.
- **System ordering: writers BEFORE handler (Risk 1, Pitfall 1).** Every `ApplyStatusEvent` writer registered with `.before(apply_status_handler)`. Symmetrically, every `StatusTickEvent` emitter registered with `.before(tick_status_durations)`. Enforced for in-`combat/` writers by `StatusEffectsPlugin::build`; for the cross-plugin writer `apply_poison_trap` (in `features.rs`), the `.before(apply_status_handler)` hook is added in `CellFeaturesPlugin::build`.
- **Resolver-vs-ticker ordering (Open Item 7).** `apply_poison_damage` and `apply_regen` MUST run BEFORE `tick_status_durations` reads the same `StatusTickEvent`. Otherwise a duration-1 poison gets removed (decremented from 1 → 0 → retain returns false) before it deals its final tick of damage. Order: `apply_poison_damage.before(tick_status_durations)`, `apply_regen.before(tick_status_durations)`. Test `tick_decrements_duration_after_damage_applied` exercises the contract.
- **`magnitude == 0.0` is a footgun (Risk 3).** `apply_poison_damage` damage formula MUST NOT silently produce zero damage when `magnitude > 0`. The recommended formula `(max_hp / 20).max(1) * magnitude` already protects against div-by-zero on `max_hp`; an additional `.max(1)` floor on the final integer damage value handles the truncation-to-zero edge case (e.g., `(1.0 * 0.4) as u32 = 0`).
- **Pre-commit hook on `gitbutler/workspace`** rejects raw `git commit` (CLAUDE.md). Implementer uses `but commit --message-file <path>`.

---

## Frozen post-#13 / DO NOT TOUCH

These files are frozen by Features #1–#13 and must not be modified by the #14 implementer except for the four explicit carve-outs listed below.

- `src/plugins/state/mod.rs` — **FROZEN by #2.**
- `src/plugins/input/mod.rs` — **FROZEN by #5.**
- `src/plugins/audio/mod.rs`, `src/plugins/audio/bgm.rs`, `src/plugins/audio/sfx.rs` — **FROZEN by #6.** No new SFX in #14.
- `src/plugins/ui/mod.rs`, `src/plugins/ui/minimap.rs` — **FROZEN by #10.** Status icons defer to #25.
- `src/plugins/save/mod.rs`, `src/plugins/town/mod.rs` — **FROZEN / empty stub.** `StatusEffects` already derives `Serialize + Deserialize + Reflect` (`character.rs:264`); save plugin work lands in #23.
- `src/data/dungeon.rs`, `src/data/items.rs`, `src/data/classes.rs`, `src/data/spells.rs`, `src/data/enemies.rs` — **FROZEN.** No data schema changes.
- `assets/dungeons/floor_01.dungeon.ron`, `assets/dungeons/floor_02.dungeon.ron`, `assets/items/core.items.ron` — **FROZEN.** No asset edits.
- `Cargo.toml`, `Cargo.lock` — **byte-unchanged.**
- `src/main.rs` — **FROZEN.** `CombatPlugin` is already registered at line 28; `StatusEffectsPlugin` registers as a sub-plugin from inside `CombatPlugin::build`.

**Explicit carve-outs (these frozen-since-#11/#13 files DO get touched, with bounded edits):**

- `src/plugins/combat/mod.rs` — **+3 lines:** `pub mod status_effects;` declaration, `pub use status_effects::*;` re-export, and `app.add_plugins(StatusEffectsPlugin)` inside `CombatPlugin::build`.
- `src/plugins/party/character.rs` — **5 enum variants appended** after `Dead` at line 242, **buff branches** inserted in `derive_stats` between lines 398 and 403 (before the existing `Dead` branch), **doc-comment updates** at lines 230-234 and 340-342, **3 `let` → `let mut`** at lines 385/386/389 (only `stat_attack`, `stat_defense`, `stat_speed` are buff targets), **+2 tests** in the existing `mod tests` block (the deferred `derive_stats_status_order_independent` at lines 611-615 becomes #14 work — replace the deferral note with the actual test).
- `src/plugins/party/inventory.rs` — **doc-comment update only** on `EquipmentChangedEvent` at lines 193-207. Zero behavioral change. Note dual-use ("Emitted by `equip_item`, `unequip_item`, AND `apply_status_handler`...").
- `src/plugins/dungeon/features.rs` — **`apply_poison_trap` refactor** at lines 412-445 (signature change + body rewrite). **`CellFeaturesPlugin::build` system registration** at lines 168-170 — add `.before(apply_status_handler)` to `apply_poison_trap` (cross-plugin import). **Existing test `poison_trap_applies_status`** at lines 938-978 — change `app.update()` to `app.update(); app.update();` (Pitfall 1).
- `src/plugins/dungeon/tests.rs`, `tests/dungeon_geometry.rs`, `tests/dungeon_movement.rs` — **test harnesses only** (D-I10/D-I11 retroactive amendment): `CombatPlugin` added to each `make_test_app` / integration-test plugin tuple to register `Messages<ApplyStatusEvent>` required by `apply_status_handler`. Without these, every test that exercises `apply_poison_trap` (which the trap refactor now writes through) panics on `MessageWriter::messages failed validation`. Pure test-only additions; zero production behavioral impact.

---

## Decisions

The plan locks the following decisions BEFORE Phase 1. Each decision is either a research-recommendation accepted as default or a load-bearing planner call. Recommended-default-accepted decisions can be overridden at plan-approval time without rework; user-pick-needed are surfaced in `## Open Questions`.

### Recommended defaults accepted (research D-numbers)

1. **D1 — Variants added in #14:** `AttackUp, DefenseUp, SpeedUp, Regen, Silence` (5). `Blind` and `Confused` deferred to #15 (declaring them now burns save-format slots speculatively per Pitfall 5; #15 ships them with their predicate consumers).
2. **D2 — Stacking semantics:** Roadmap §14 line 780 verbatim. Same effect → refresh duration, take higher magnitude. Permanent (Stone/Dead) re-application → no-op. Buff `magnitude` IS the multiplier.
3. **D3 — Module placement:** `src/plugins/combat/status_effects.rs` + `StatusEffectsPlugin` registered as sub-plugin of `CombatPlugin` via `combat/mod.rs::CombatPlugin::build`.
4. **D4 — Tick architecture:** Single `StatusTickEvent` message; #14 owns the dungeon-step emitter (`tick_on_dungeon_step` reads `MovedEvent`); #15 will add the combat-round emitter (one line in `turn_manager.rs::round_end`).
5. **D5 — Re-derive trigger on stat-affecting status changes:** D5α — `apply_status_handler` writes `EquipmentChangedEvent { character: target, slot: EquipSlot::None }` for `AttackUp/DefenseUp/SpeedUp/Dead`. Reuses 100% of `recompute_derived_stats_on_equipment_change` pipeline. Doc-comment on `EquipmentChangedEvent` updated.
6. **D6 — `derive_stats` buff branch placement:** Buff branches BEFORE `Dead`. `Dead` LAST per Pitfall 4. Comment-tag `Dead` as zero-out-dominates.
7. **D8 — Poison-trap potency:** `potency = 1.0`, `duration = Some(5)`. Preserves the existing `POISON_TURNS = 5` constant at `features.rs:421`.
8. **D10 — `Blind`/`Confused` enum slots:** Defer to #15. (Restates D1.)
9. **D11 — Predicate shape:** `pub fn is_paralyzed/is_asleep/is_silenced(s: &StatusEffects) -> bool`. Same shape as `EquipSlot::read`. NOT systems.
10. **D12 — Block-action systems in #14:** None. Predicates only. #15 wires them into `turn_manager`.
11. **D13 — `Dead`-on-zero-HP application:** Defer to #15. **#14 ships a stub `pub fn check_dead_and_apply` for #15 to call** (Open Item 3 confirmed: ship the stub). Signature: `pub fn check_dead_and_apply(target: Entity, derived: &DerivedStats, writer: &mut MessageWriter<ApplyStatusEvent>)`. Body checks `if derived.current_hp == 0` and writes `ApplyStatusEvent { target, effect: Dead, potency: 1.0, duration: None }`. Documented as #15-callable; not registered as a system in #14.
12. **D14 — NaN clamp:** `apply_status_handler` clamps `ev.potency.clamp(0.0, 10.0)` at the trust boundary. Pitfall 6 guard.
13. **D15 — `Hash`/`Eq` on `ActiveEffect`:** Do NOT add. `f32` rationale.

### Planner calls (load-bearing, not surfaced as user picks)

14. **`StatusEffectType` discriminant order (Open Item 5):** `..., Dead, AttackUp, DefenseUp, SpeedUp, Regen, Silence`. Lock with comment marker `// HISTORICAL APPEND ORDER — DO NOT REORDER. New variants go at end.` directly above the enum at `character.rs:235`.
15. **`derive_stats` buff branch location (Open Item 6):** Insert at `character.rs:399` (between the existing `// ── Status effect post-pass` comment block at line 400 and the `Dead` branch at line 403). The locals at `character.rs:385/386/389` (`stat_attack`, `stat_defense`, `stat_speed`) change from `let` to `let mut`. The locals at lines 387/388/390/391/393 (`stat_magic_attack`, `stat_magic_defense`, `stat_accuracy`, `stat_evasion`) stay immutable (no buff variants modify them in #14). The new buff branches use the same saturating arithmetic as the equipment additive stack at lines 374-381: `let bonus = (stat_attack as f32 * effect.magnitude) as u32; stat_attack = stat_attack.saturating_add(bonus);` — never panics; truncates fractional damage; guards against `magnitude * stat` overflow via `saturating_add`.
16. **System ordering inside `StatusEffectsPlugin::build` (Open Item 7):** All systems in `Update`. `tick_on_dungeon_step` gated `.run_if(in_state(Dungeon)).after(handle_dungeon_input)`. `apply_status_handler` ungated, message-driven (idles when no events). `tick_status_durations` ungated. `apply_poison_damage.before(tick_status_durations)` and `apply_regen.before(tick_status_durations)` (resolver-vs-ticker constraint — see Critical). `tick_on_dungeon_step.before(tick_status_durations)` AND `.before(apply_poison_damage).before(apply_regen)` (emitter-before-readers, same-frame consumability). The `apply_status_handler` is independent of the tick chain (different message); only requires `.before(tick_status_durations)` if both run in the same frame and the handler's `EquipmentChangedEvent` write should be observable that frame — recommended ordering: `apply_status_handler` independent (no constraint vs. tick chain).
17. **`EquipSlot::None` reuse (Open Item 4 verified):** `EquipSlot::None` exists at `inventory.rs:120` as the first variant of the `EquipSlot` enum (used by `EquipSlot::read` to return `None` and by `equip_item` to reject as `EquipError::ItemHasNoSlot`). **No new enum variant needed.** D5α reuses this existing variant as the "not really an equipment change" sentinel. Verified via `Read` of `inventory.rs:110-150`.
18. **`check_dead_and_apply` stub shipped (Open Item 3):** Public free function in `combat/status_effects.rs`. Two unit tests: one writes the message when `current_hp == 0`, one no-ops when `current_hp > 0`. #15 imports and calls.
19. **Test placement:** New `mod tests` inside `combat/status_effects.rs` (matches `features.rs::tests` precedent). Layer 1 (no `App`) for stacking-merge logic, predicates, `check_dead_and_apply`. Layer 2 (App-driven via `make_test_app`) for full handler / tick / resolver chain. The deferred test `derive_stats_status_order_independent` at `character.rs:611-615` becomes part of #14 — replace the deferral note with the actual test using the new `AttackUp` variant.
20. **`apply_status_handler` is the SOLE mutator of `StatusEffects.effects`.** No other system pushes to or removes from `effects` directly (except `tick_status_durations` for expiry — which is the documented exception). After the `apply_poison_trap` refactor in Phase 7, ZERO callers of `effects.push(...)` exist outside the handler/ticker pair.

### Recommended defaults — surfaced for user override

21. **D7 — Per-tick poison/regen damage formula** (USER PICK with default):
    - **Recommended (A):** `damage = ((derived.max_hp / 20).max(1) as f32 * effect.magnitude) as u32; let damage = damage.max(1);` — % of max (5% baseline at `magnitude = 1.0`). Wizardry/Etrian-canonical. Scales with character power. The two `.max(1)` floors guard against the `low-HP` (max_hp < 20) and `low-magnitude` (truncation to 0) edge cases.
    - **B:** Flat per tick: `damage = (5.0 * magnitude) as u32; damage.max(1)`. Simple; doesn't scale with character power.
    - **C:** INT-scaled. Deferred (no caster data in #14).
    - **`apply_regen` mirrors symmetrically (Open Item 1):** `let healing = ((derived.max_hp / 20).max(1) as f32 * effect.magnitude) as u32; let healing = healing.max(1);` then `derived.current_hp = derived.current_hp.saturating_add(healing).min(derived.max_hp);`. Same shape as Poison damage but with `saturating_add` and `min(max_hp)` cap. If user picks B, Regen mirrors B accordingly.
22. **D9 — Dungeon-step tick frequency** (USER PICK with default):
    - **Recommended (A):** Every step. One `StatusTickEvent` per `MovedEvent` per `PartyMember`. Wizardry-canonical. Matches the project's overall tone (`dungeon/mod.rs:1-34` references Wizardry).
    - **B:** Every Nth step. Adds a `StepsSinceLastTick: Resource(u32)` counter; `tick_on_dungeon_step` only fires when counter ≥ N (default N=3), then resets. +1 resource, +5 LOC.
    - **C:** Time-based. Reads `Time::delta`. Not Wizardry; less player control.

**If the user does not override D7 or D9 at plan-approval time, the plan proceeds with options A on each.** Both choices affect runtime feel only; the architecture is unchanged across all three options.

---

## Open Questions

The plan defers two USER-PICK decisions (D7 and D9 per `## Decisions` items 21-22). Both default to the research-recommended option (A on each); both are runtime-feel tuning calls, not architectural decisions; both are reversible without code-structure change. **The implementer proceeds with options A unless the user objects at plan approval.**

### Genuine USER PICK — confirm or override before kickoff

- **D7 — Per-tick poison/regen damage formula** (default A: % of max, `(max_hp / 20).max(1) * magnitude`, with `.max(1)` floors).
- **D9 — Dungeon-step tick frequency** (default A: every step).

### Resolved during planning (research-recommended defaults — accepted)

- D1 (`Blind`/`Confused` defer to #15) — Resolved: defer.
- D2 (stacking) — Resolved: roadmap line 780 verbatim, two refinements.
- D3 (module placement) — Resolved: `combat/status_effects.rs` + sub-plugin.
- D4 (tick trigger architecture) — Resolved: single `StatusTickEvent`, two emitters.
- D5 (re-derive trigger) — Resolved: D5α (reuse `EquipmentChangedEvent`).
- D6 (buff branch placement) — Resolved: BEFORE `Dead`.
- D8 (poison-trap potency) — Resolved: `potency = 1.0, duration = Some(5)`.
- D10 (`Blind`/`Confused` enum) — Resolved: defer (restates D1).
- D11 (predicate shape) — Resolved: `pub fn`.
- D12 (block-action systems) — Resolved: none in #14.
- D13 (`Dead`-on-zero-HP) — Resolved: defer; ship stub.
- D14 (NaN clamp) — Resolved: `clamp(0.0, 10.0)`.
- D15 (`Hash`/`Eq`) — Resolved: don't add.

### Resolved during planning (Open Items from prompt)

- Item 1 (Regen formula symmetry) — Resolved: mirror Poison; if D7=A, Regen uses `(max_hp / 20).max(1) * magnitude` heal with `saturating_add().min(max_hp)`.
- Item 2 (`is_blind`/`is_confused` predicates in #14) — Resolved: variants NO, predicates NO. Defer both fully to #15.
- Item 3 (`check_dead_and_apply` stub) — Resolved: ship `pub fn` stub; #15 imports.
- Item 4 (`EquipSlot::None`) — Resolved: variant exists at `inventory.rs:120`. No new variant needed.
- Item 5 (variant discriminant order) — Resolved: `..., Dead, AttackUp, DefenseUp, SpeedUp, Regen, Silence`.
- Item 6 (`derive_stats` buff branch location) — Resolved: insert at line 399, before existing `Dead` branch at 403; locals 385/386/389 become `let mut`.
- Item 7 (per-frame ordering) — Resolved: `apply_poison_damage.before(tick_status_durations)`, `apply_regen.before(tick_status_durations)`.

---

## Pitfalls

The 7 research-flagged pitfalls below appear as guards inside the relevant Phase below; this section is the central reference.

### Pitfall 1 — Refactored `apply_poison_trap` test needs `app.update()` × 2

**Where it bites:** `features.rs:938-978`. Test currently writes `MovedEvent`, `app.update()` once, asserts Poison present. After #14's refactor, the trap writes `ApplyStatusEvent` instead — must be read by `apply_status_handler` in the next system run.

**Guard in plan:** Phase 7 changes the test body to `app.update(); app.update();`. AND `CellFeaturesPlugin::build` adds `.before(apply_status_handler)` to `apply_poison_trap` so production fires same-frame.

### Pitfall 2 — `derive_stats` order-dependence after buffs land

**Where it bites:** Two `AttackUp 0.5` effects could double-stack. The merge rule (D2: take higher magnitude) collapses them at `apply_status_handler` time, so `derive_stats` sees AT MOST ONE `AttackUp`. Order-independence is preserved by the merge invariant.

**Guard in plan:** Phase 8 ships `derive_stats_status_order_independent` test (replacing the deferred-to-#15 note at `character.rs:611-615`). Apply two `AttackUp` instances in different orders, assert same `DerivedStats` output. Regression guard against future contributors adding a duplicate-stack code path.

### Pitfall 3 — `EquipmentChangedEvent` name drift after D5α

**Where it bites:** `apply_status_handler` writes `EquipmentChangedEvent` for non-equipment causes. Future readers may be confused.

**Guard in plan:** Phase 6 updates the doc-comment at `inventory.rs:193-207` to "Emitted by `equip_item`, `unequip_item`, AND `apply_status_handler` when something requires `derive_stats` to re-run." Renaming the type defers to #25 polish.

### Pitfall 4 — `Dead` interaction with buff `magnitude` re-derive

**Where it bites:** If buffs come AFTER `Dead`, the buff math runs on zeroed pools. Wasted work but not incorrect (saturating_add(0) = 0).

**Guard in plan:** Phase 5 inserts buff branches BEFORE the `Dead` branch. `Dead` becomes the LAST status branch; comment-tagged `// LAST: zero-out dominates all buffs above.` Phase 8 adds a regression test `derive_stats_dead_dominates_buffs`.

### Pitfall 5 — Save-format breakage from variant reordering

**Where it bites:** `Reflect`/`serde` serialization uses the discriminant order; reordering breaks every save fixture.

**Guard in plan:** Phase 1 adds the comment marker `// HISTORICAL APPEND ORDER — DO NOT REORDER. New variants go at end.` directly above the `StatusEffectType` enum at `character.rs:235`. New variants append AFTER `Dead`. **Plus a new round-trip test `status_effect_type_dead_serializes_to_index_4`** that asserts the serde-encoded byte for `Dead` is `4` (catches future reorder at CI).

### Pitfall 6 — `magnitude: f32` NaN propagation

**Where it bites:** `f32::NAN * 100 = NAN`; downstream comparisons return false; combat math becomes unstable.

**Guard in plan:** Phase 3 `apply_status_handler` body starts with `let potency = ev.potency.clamp(0.0, 10.0);` BEFORE any merge or push. Tested by `apply_status_handler_clamps_nan_to_zero` (Layer 1).

### Pitfall 7 — Tick race with `Dead` re-application

**Where it bites:** A character at 1 HP gets a poison tick to 0 HP. Should `apply_poison_damage` immediately apply `Dead`?

**Guard in plan:** **Defer to #15.** `apply_poison_damage` uses `saturating_sub` only. `Dead` is applied by #15's combat resolver. **#14 ships `pub fn check_dead_and_apply` (Decision 18) for #15 to call.** Documented as #15-callable.

---

## Steps

The implementation proceeds in **9 phases**, each one a single atomic commit boundary. Every phase's exit criterion is `cargo test` passing. Phases 1-2 build the data-layer additions; Phase 3 lands the `combat/status_effects.rs` skeleton; Phases 4-6 light up the systems; Phase 7 refactors `apply_poison_trap`; Phase 8 adds the order-independence and Dead-dominance tests; Phase 9 is the final 7-command verification gate.

### Phase 1 — `StatusEffectType` extension + comment marker (`character.rs`)

The smallest possible change first: append 5 enum variants. No buff branches yet. No new tests for the variants themselves (Reflect derive auto-handles them); only the comment marker + the discriminant-order regression test.

- [ ] In `src/plugins/party/character.rs`, add a comment marker directly ABOVE the `#[derive(Reflect, ...)]` line at line 235 (between line 234's existing doc-comment and line 235's derive):

  ```rust
  // HISTORICAL APPEND ORDER — DO NOT REORDER. New variants go at end.
  // Save-format stability depends on discriminant indices being stable across
  // versions (Pitfall 5 of #14, Decision 7 of #11). Adding a variant in the
  // middle shifts every saved status effect's serialized byte.
  #[derive(Reflect, Serialize, Deserialize, Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
  pub enum StatusEffectType {
  ```

- [ ] Append 5 new variants AFTER `Dead` at line 242. The block at lines 236-243 becomes:

  ```rust
  pub enum StatusEffectType {
      #[default]
      Poison,        // 0
      Sleep,         // 1
      Paralysis,     // 2
      Stone,         // 3
      Dead,          // 4
      // ── Feature #14 additions (append-only) ────────────────────────────
      AttackUp,      // 5  — multiplier on `attack`
      DefenseUp,     // 6  — multiplier on `defense`
      SpeedUp,       // 7  — multiplier on `speed`
      Regen,         // 8  — heals on tick (mirror of Poison)
      Silence,       // 9  — gates spell action selection (#15 wires in turn_manager)
      // Blind, Confused: deferred to #15 (no readers in #14).
  }
  ```

- [ ] Update the doc-comment at `character.rs:225-234`. Replace the existing block (which says "Buffs ... are deferred to #15") with:

  ```rust
  /// V1 status set + Feature #14 extensions.
  ///
  /// **Append-only enum (Pitfall 5 of #14):** Discriminant indices 0-4 are
  /// LOCKED for save-format stability. Indices 5-9 added in #14. New variants
  /// (e.g., `Blind`, `Confused` in #15) MUST go at end.
  ///
  /// **Buff variants (`AttackUp`, `DefenseUp`, `SpeedUp`):** modify
  /// `derive_stats` output via the `magnitude` field as a multiplier (e.g.,
  /// `AttackUp 0.5` = +50% attack). Re-derive triggered by
  /// `apply_status_handler` writing `EquipmentChangedEvent` with
  /// `slot: EquipSlot::None` (sentinel).
  ///
  /// **`Regen`:** ticks per dungeon step; healing mirrors Poison damage shape.
  ///
  /// **`Silence`:** predicate `is_silenced` available in
  /// `combat/status_effects.rs`; #15 wires into `turn_manager` for
  /// spell-action gating.
  ///
  /// The `magnitude` field on `ActiveEffect` is used by:
  /// - Buffs (`AttackUp`/`DefenseUp`/`SpeedUp`): multiplier (e.g. `0.5` = +50%).
  /// - Tick effects (`Poison`/`Regen`): per-tick magnitude.
  /// - Pure gates (`Sleep`/`Paralysis`/`Stone`/`Dead`/`Silence`): unused; set 0.0.
  ```

- [ ] Update the doc-comment on `ActiveEffect.magnitude` at `character.rs:259`. Replace "Unused by v1 status types; reserved for #15 magnitude-modifying buffs." with:

  ```rust
  /// Magnitude / potency, depending on effect type.
  /// - Buffs: multiplier (e.g., `0.5` = +50% attack).
  /// - Tick effects (Poison, Regen): per-tick magnitude.
  /// - Pure gates (Sleep, Paralysis, Stone, Dead, Silence): unused; set 0.0.
  /// Clamped at the trust boundary by `apply_status_handler` to `[0.0, 10.0]`
  /// (Pitfall 6 of #14).
  pub magnitude: f32,
  ```

- [ ] In `character.rs::tests`, add a regression test for the discriminant order (Pitfall 5):

  ```rust
  #[test]
  fn status_effect_type_dead_serializes_to_index_4() {
      // Locks the historical append order — any future reorder fails CI.
      // ron-encoded enum unit variants serialize to "Dead" by name, not by
      // discriminant byte; this test asserts on the bincode-equivalent
      // discriminant via the `as u8` projection.
      assert_eq!(StatusEffectType::Poison as u8, 0);
      assert_eq!(StatusEffectType::Sleep as u8, 1);
      assert_eq!(StatusEffectType::Paralysis as u8, 2);
      assert_eq!(StatusEffectType::Stone as u8, 3);
      assert_eq!(StatusEffectType::Dead as u8, 4);
      assert_eq!(StatusEffectType::AttackUp as u8, 5);
      assert_eq!(StatusEffectType::Silence as u8, 9);
  }
  ```

- [ ] **Verification (atomic commit boundary):**
  - `cargo build` — succeeds. New variants compile; no consumers yet.
  - `cargo test plugins::party::character::tests` — passes. New `status_effect_type_dead_serializes_to_index_4` is green.
  - `cargo test` — full suite passes (existing `StatusEffects::has` test at `character.rs:599-609` still green).
  - `rg 'register_type::<StatusEffectType>' src/` — confirms registration is automatic via existing line at `party/mod.rs:47` (no edits needed; `#[derive(Reflect)]` covers all variants).

### Phase 2 — `derive_stats` buff branches + 1 new test (`character.rs`)

Lift the existing locals to `let mut` for the three buff-targets; insert the buff loop before the `Dead` branch; update the doc-comment that says #15 owns this. Add `derive_stats_attack_up_buffs_attack` test.

- [ ] In `src/plugins/party/character.rs`, change three locals at lines 385/386/389 from `let` to `let mut`:

  ```rust
  let mut stat_attack = (base.strength as u32).saturating_add(equip_attack);
  let mut stat_defense = (base.vitality as u32 / 2).saturating_add(equip_defense);
  // (lines 387-388 unchanged: stat_magic_attack, stat_magic_defense)
  let mut stat_speed = base.agility as u32;
  // (lines 390-395 unchanged: stat_accuracy, stat_evasion)
  ```

  (Locals 387/388/390/391/393 — `stat_magic_attack`, `stat_magic_defense`, `stat_accuracy`, `stat_evasion` — stay immutable. No buff variant in #14 modifies them.)

- [ ] Insert the buff branches at line 399 (between the existing `// ── Status effect post-pass` comment block at line 400 and the `Dead` branch at line 403). Replace the existing block at lines 400-411:

  ```rust
  // ── Status effect post-pass ──────────────────────────────────────────────
  // The merge rule in `apply_status_handler` guarantees AT MOST ONE of each
  // variant is present, so iterating without a "first wins" or
  // "stack" rule is correct (Pitfall 2 of #14: order-independence preserved
  // by the merge invariant — see test `derive_stats_status_order_independent`).
  for effect in &status.effects {
      match effect.effect_type {
          // ── Buff branches (Feature #14) ──────────────────────────────
          // `magnitude` is a multiplier; saturating arithmetic guards against
          // overflow on extreme values (clamped to [0.0, 10.0] at the trust
          // boundary in apply_status_handler — Pitfall 6).
          StatusEffectType::AttackUp => {
              let bonus = (stat_attack as f32 * effect.magnitude) as u32;
              stat_attack = stat_attack.saturating_add(bonus);
          }
          StatusEffectType::DefenseUp => {
              let bonus = (stat_defense as f32 * effect.magnitude) as u32;
              stat_defense = stat_defense.saturating_add(bonus);
          }
          StatusEffectType::SpeedUp => {
              let bonus = (stat_speed as f32 * effect.magnitude) as u32;
              stat_speed = stat_speed.saturating_add(bonus);
          }
          // Poison, Sleep, Paralysis, Stone, Silence, Regen: not derive-time
          // modifiers. Poison/Regen tick in `combat/status_effects.rs`;
          // Sleep/Paralysis/Silence gate action selection in #15 via predicates;
          // Stone is treated like Dead for targeting in #15.
          _ => {}
      }
  }

  // ── Dead branch — LAST (Pitfall 4 of #14: zero-out dominates buffs above) ──
  if status.has(StatusEffectType::Dead) {
      max_hp = 0;
      max_mp = 0;
  }
  ```

- [ ] Update the doc-comment at `character.rs:337-342` (the block that says "#15 will add magnitude-modifying buff branches"). Replace with:

  ```rust
  /// **Status post-pass:** The buff branches (`AttackUp`/`DefenseUp`/`SpeedUp`)
  /// modify their respective stats by `magnitude` as a multiplier. The merge
  /// rule in `apply_status_handler` guarantees at most one of each variant is
  /// present; iteration is order-independent (test
  /// `derive_stats_status_order_independent`). The `Dead` branch runs LAST
  /// and zeroes `max_hp`/`max_mp` (Pitfall 4 of #14: zero-out dominates). Future
  /// magnitude-modifying variants (#15+) follow the same pattern.
  ```

- [ ] Replace the deferred-to-#15 note at `character.rs:611-615` with the actual order-independence test:

  ```rust
  // Note (#14): the deferred test below now exists, exercising the buff
  // branches added in #14. Phase 8 adds the multi-variant order test.
  #[test]
  fn derive_stats_attack_up_buffs_attack() {
      let base = BaseStats {
          strength: 10,
          ..Default::default()
      };
      let mut status = StatusEffects::default();
      status.effects.push(ActiveEffect {
          effect_type: StatusEffectType::AttackUp,
          remaining_turns: Some(3),
          magnitude: 0.5,  // +50%
      });
      let derived = derive_stats(&base, &[], &status, 1);
      // base.strength (10) + 50% = 15. (no equipment)
      assert_eq!(derived.attack, 15, "AttackUp 0.5 should yield +50% attack");
  }
  ```

  (Note: the multi-variant `derive_stats_status_order_independent` test ships in Phase 8 — it needs both `AttackUp` and `DefenseUp` to exercise order independence.)

- [ ] **Verification (atomic commit boundary):**
  - `cargo build` — succeeds.
  - `cargo test plugins::party::character::tests::derive_stats_attack_up_buffs_attack` — passes.
  - `cargo test plugins::party::character` — all character tests pass; existing `derive_stats_*` tests (e.g., `derive_stats_dead_zeros_pools`) remain green.
  - `cargo test` — full suite passes.

### Phase 3 — `combat/status_effects.rs` skeleton: types, messages, plugin scaffold

Land the `combat/status_effects.rs` file with imports, the 2 messages, the plugin struct, and the `Plugin::build` impl with all 5 systems registered as stubs returning `()`. Wire registration via `combat/mod.rs`. Add the file-level doc-comment. ~120 LOC.

- [ ] Create `src/plugins/combat/status_effects.rs` (NEW file). Add file-level doc-comment:

  ```rust
  //! Status effect resolution layer — Feature #14.
  //!
  //! Owns:
  //! - `ApplyStatusEvent` message (canonical "apply this effect to target").
  //! - `StatusTickEvent` message (internal — emitted on every dungeon step;
  //!   #15 will add a combat-round emitter).
  //! - `apply_status_handler` system (SOLE mutator of `StatusEffects.effects`).
  //! - `tick_status_durations` system (decrements `remaining_turns`, removes
  //!   expired effects).
  //! - `tick_on_dungeon_step` system (fires `StatusTickEvent` on `MovedEvent`).
  //! - `apply_poison_damage` and `apply_regen` resolvers (read
  //!   `StatusTickEvent`, mutate `DerivedStats.current_hp`).
  //! - `pub fn` predicates: `is_paralyzed`, `is_asleep`, `is_silenced`. Used
  //!   by #15's `turn_manager` for action gating.
  //! - `pub fn check_dead_and_apply`: stub for #15 to call after damage
  //!   resolves to apply the `Dead` status.
  //!
  //! See `project/research/20260507-115500-feature-14-status-effects-system.md`.
  //!
  //! ## Bevy 0.18 family rename
  //!
  //! `ApplyStatusEvent` and `StatusTickEvent` derive `Message`, NOT `Event`.
  //! Read with `MessageReader<T>`, write with `MessageWriter<T>`. Register with
  //! `app.add_message::<T>()`.
  //!
  //! ## Stacking semantics (D2 of research)
  //!
  //! `apply_status_handler` is the single mutator. Same effect already present
  //! → refresh duration, take `magnitude.max()`. Permanent (Stone/Dead)
  //! re-application → no-op. Buff `magnitude` IS the multiplier.
  //!
  //! ## D5α — re-derive trigger
  //!
  //! `apply_status_handler` writes `EquipmentChangedEvent` with
  //! `slot: EquipSlot::None` (sentinel) for stat-affecting variants
  //! (`AttackUp`/`DefenseUp`/`SpeedUp`/`Dead`). This reuses
  //! `recompute_derived_stats_on_equipment_change` (`inventory.rs:421`)
  //! verbatim — no new recompute path.
  ```

- [ ] Add imports:

  ```rust
  use bevy::prelude::*;

  use crate::plugins::dungeon::{MovedEvent, handle_dungeon_input};
  use crate::plugins::party::{
      ActiveEffect, DerivedStats, EquipmentChangedEvent, EquipSlot, PartyMember,
      StatusEffectType, StatusEffects,
  };
  use crate::plugins::state::GameState;
  ```

- [ ] Define `ApplyStatusEvent` message:

  ```rust
  /// Canonical "apply this status effect to target" message.
  ///
  /// Every status source — traps (`apply_poison_trap`), enemy spells (#15),
  /// items (#20) — writes this. The single `apply_status_handler` system reads
  /// it and enforces stacking semantics.
  ///
  /// **Field semantics:**
  /// - `target`: entity receiving the effect. Typically `PartyMember` in v1.
  /// - `effect`: which `StatusEffectType` to apply or refresh.
  /// - `potency`: magnitude. For buffs, multiplier (e.g., `0.5` = +50%). For
  ///   tick effects (Poison/Regen), per-tick magnitude. Clamped to `[0.0, 10.0]`
  ///   by `apply_status_handler` (Pitfall 6, defensive trust boundary).
  /// - `duration`: `Some(n)` for `n` ticks. `None` for permanent (Stone/Dead).
  ///
  /// **No `source` field** (deliberate; YAGNI per research B.1).
  ///
  /// `Message`, NOT `Event` — Bevy 0.18 family rename.
  #[derive(Message, Clone, Copy, Debug)]
  pub struct ApplyStatusEvent {
      pub target: Entity,
      pub effect: StatusEffectType,
      pub potency: f32,
      pub duration: Option<u32>,
  }
  ```

- [ ] Define `StatusTickEvent` message:

  ```rust
  /// Internal tick message. One per `MovedEvent` per `PartyMember` (D9-A) in
  /// #14. #15 will add a `turn_manager::round_end` emitter for combat-round
  /// ticks (one line: `for entity in alive_combatants { tick.write(...); }`).
  ///
  /// `Message`, NOT `Event` — Bevy 0.18 family rename.
  #[derive(Message, Clone, Copy, Debug)]
  pub struct StatusTickEvent {
      pub target: Entity,
  }
  ```

- [ ] Add the `StatusEffectsPlugin` struct + `Plugin::build` impl (system bodies are stubs in this phase):

  ```rust
  /// Owns status-effect systems, messages, and per-effect resolvers for
  /// Feature #14. Registered as a sub-plugin of `CombatPlugin` from
  /// `combat/mod.rs::CombatPlugin::build`.
  pub struct StatusEffectsPlugin;

  impl Plugin for StatusEffectsPlugin {
      fn build(&self, app: &mut App) {
          app.add_message::<ApplyStatusEvent>()
              .add_message::<StatusTickEvent>()
              .add_systems(
                  Update,
                  (
                      // Dungeon-step tick — gated; #15 will add a
                      // combat-round emitter without touching this one.
                      tick_on_dungeon_step
                          .run_if(in_state(GameState::Dungeon))
                          .after(handle_dungeon_input)
                          // Emitter-before-readers: same-frame consumability
                          // for the resolvers (Decision 16).
                          .before(tick_status_durations)
                          .before(apply_poison_damage)
                          .before(apply_regen),
                      // Message-driven systems — no state gate (idle when
                      // no events; Pattern 2 of research).
                      apply_status_handler,
                      // Resolvers BEFORE ticker so a duration-1 effect
                      // gets its final tick of damage before being removed
                      // (Critical: resolver-vs-ticker ordering, Open Item 7).
                      apply_poison_damage.before(tick_status_durations),
                      apply_regen.before(tick_status_durations),
                      tick_status_durations,
                  ),
              );
      }
  }
  ```

- [ ] Add 5 stub system bodies (each just an early-return — full bodies in Phases 4-5):

  ```rust
  /// STUB — Phase 4 lands the body.
  fn tick_on_dungeon_step(
      _moved: MessageReader<MovedEvent>,
      _tick: MessageWriter<StatusTickEvent>,
      _party: Query<Entity, With<PartyMember>>,
  ) {}

  /// STUB — Phase 4 lands the body.
  fn apply_status_handler(
      _events: MessageReader<ApplyStatusEvent>,
      _equip_changed: MessageWriter<EquipmentChangedEvent>,
      _characters: Query<&mut StatusEffects, With<PartyMember>>,
  ) {}

  /// STUB — Phase 4 lands the body.
  fn tick_status_durations(
      _ticks: MessageReader<StatusTickEvent>,
      _equip_changed: MessageWriter<EquipmentChangedEvent>,
      _characters: Query<&mut StatusEffects, With<PartyMember>>,
  ) {}

  /// STUB — Phase 5 lands the body.
  fn apply_poison_damage(
      _ticks: MessageReader<StatusTickEvent>,
      _characters: Query<(&StatusEffects, &mut DerivedStats), With<PartyMember>>,
  ) {}

  /// STUB — Phase 5 lands the body.
  fn apply_regen(
      _ticks: MessageReader<StatusTickEvent>,
      _characters: Query<(&StatusEffects, &mut DerivedStats), With<PartyMember>>,
  ) {}
  ```

  (The stubs let the plugin register cleanly so subsequent phases each have a runnable scaffold to layer onto.)

- [ ] Add the predicates and the #15-callable stub:

  ```rust
  /// Returns `true` if `Paralysis` is currently active. #15 imports this and
  /// calls from `turn_manager::collect_player_actions` to skip the character's
  /// turn.
  pub fn is_paralyzed(status: &StatusEffects) -> bool {
      status.has(StatusEffectType::Paralysis)
  }

  /// Returns `true` if `Sleep` is currently active. #15 imports for action gating.
  pub fn is_asleep(status: &StatusEffects) -> bool {
      status.has(StatusEffectType::Sleep)
  }

  /// Returns `true` if `Silence` is currently active. #15 imports for spell-
  /// action gating in `turn_manager::collect_player_actions`.
  pub fn is_silenced(status: &StatusEffects) -> bool {
      status.has(StatusEffectType::Silence)
  }

  /// #15-callable convenience: writes `ApplyStatusEvent { effect: Dead, ... }`
  /// when `derived.current_hp == 0`. #14 does NOT auto-apply Dead inside
  /// `apply_poison_damage` (Pitfall 7 — defer combat genre rules to #15).
  /// #15's combat resolver imports and calls this after damage resolves.
  ///
  /// `target` is the character entity. `derived` is read-only (no mutation).
  /// `writer` writes the message; the handler picks it up next frame.
  pub fn check_dead_and_apply(
      target: Entity,
      derived: &DerivedStats,
      writer: &mut MessageWriter<ApplyStatusEvent>,
  ) {
      if derived.current_hp == 0 {
          writer.write(ApplyStatusEvent {
              target,
              effect: StatusEffectType::Dead,
              potency: 1.0,
              duration: None, // permanent
          });
      }
  }
  ```

- [ ] Create `src/plugins/combat/mod.rs` edits (3 lines added). Currently the file has the `CombatPlugin` stub at lines 1-19. Modify to:

  ```rust
  use bevy::log::info;
  use bevy::prelude::*;

  use crate::plugins::state::GameState;

  pub mod status_effects;
  pub use status_effects::*;

  /// Turn-based combat plugin — initiative, actions, damage resolution.
  /// Feature #2 wires OnEnter/OnExit log stubs; gameplay systems land in #14-#16.
  /// Feature #14 adds `StatusEffectsPlugin` as a sub-plugin (status data and
  /// resolution layer; the combat-round emitter of `StatusTickEvent` ships with
  /// #15's `turn_manager`).
  pub struct CombatPlugin;

  impl Plugin for CombatPlugin {
      fn build(&self, app: &mut App) {
          app.add_plugins(status_effects::StatusEffectsPlugin)
              .add_systems(OnEnter(GameState::Combat), || {
                  info!("Entered GameState::Combat")
              })
              .add_systems(OnExit(GameState::Combat), || {
                  info!("Exited GameState::Combat")
              });
      }
  }
  ```

- [ ] Add a `#[cfg(test)] mod tests` block at the end of `combat/status_effects.rs` with 3 Layer-1 (no `App`) tests for the predicates:

  ```rust
  #[cfg(test)]
  mod tests {
      use super::*;

      // ── Predicates ──────────────────────────────────────────────────

      #[test]
      fn is_paralyzed_returns_true_when_present() {
          let mut status = StatusEffects::default();
          status.effects.push(ActiveEffect {
              effect_type: StatusEffectType::Paralysis,
              remaining_turns: Some(3),
              magnitude: 0.0,
          });
          assert!(is_paralyzed(&status));
          assert!(!is_asleep(&status));
          assert!(!is_silenced(&status));
      }

      #[test]
      fn is_silenced_returns_false_when_absent() {
          let status = StatusEffects::default();
          assert!(!is_silenced(&status));
      }

      #[test]
      fn check_dead_and_apply_writes_when_hp_zero() {
          // Layer 1.5 — needs a tiny App for the MessageWriter, but no plugins.
          let mut app = App::new();
          app.add_message::<ApplyStatusEvent>();
          let entity = app.world_mut().spawn_empty().id();
          let derived = DerivedStats {
              current_hp: 0,
              ..Default::default()
          };
          // Run a one-shot system to invoke the stub.
          app.world_mut().run_system_once(
              move |mut writer: MessageWriter<ApplyStatusEvent>| {
                  check_dead_and_apply(entity, &derived, &mut writer);
              },
          )
          .expect("one-shot ran");
          let messages = app.world().resource::<Messages<ApplyStatusEvent>>();
          assert_eq!(messages.len(), 1, "should write one Dead apply");
      }

      #[test]
      fn check_dead_and_apply_no_op_when_hp_positive() {
          let mut app = App::new();
          app.add_message::<ApplyStatusEvent>();
          let entity = app.world_mut().spawn_empty().id();
          let derived = DerivedStats {
              current_hp: 1,
              ..Default::default()
          };
          app.world_mut().run_system_once(
              move |mut writer: MessageWriter<ApplyStatusEvent>| {
                  check_dead_and_apply(entity, &derived, &mut writer);
              },
          )
          .expect("one-shot ran");
          let messages = app.world().resource::<Messages<ApplyStatusEvent>>();
          assert_eq!(messages.len(), 0, "no apply when HP > 0");
      }
  }
  ```

  (Note: `run_system_once` requires `bevy::prelude::*` import which is already present. If the chosen Bevy 0.18 API differs, fall back to a manual `app.world_mut().resource_mut::<Messages<ApplyStatusEvent>>()` write inside a closure-spawned system. The implementer should verify `run_system_once` is available in 0.18 — it has been since 0.13.)

- [ ] **Verification (atomic commit boundary):**
  - `cargo build` — succeeds. The 5 stubs compile because each early-returns; the plugin registers cleanly.
  - `cargo test plugins::combat::status_effects::tests` — 4 Layer-1 tests pass.
  - `cargo test` — full suite passes.

### Phase 4 — `apply_status_handler` + `tick_on_dungeon_step` + `tick_status_durations` (3 systems wired)

Replace the 3 stubs with full bodies. Cover the canonical handler (with NaN clamp + stacking merge + `EquipmentChangedEvent` re-fire), the dungeon-step emitter, and the duration ticker. ~150 LOC + ~5 Layer-2 tests using `App`-driven harness.

- [ ] In `src/plugins/combat/status_effects.rs`, replace the `apply_status_handler` stub with:

  ```rust
  /// The single mutator of `StatusEffects.effects`. Every other system writes
  /// `ApplyStatusEvent` rather than pushing directly.
  ///
  /// **Stacking rule (D2 of research):**
  /// - Same effect already present: refresh `remaining_turns`, take
  ///   `existing.magnitude.max(new.potency)`.
  /// - Permanent (Stone/Dead) already present: no-op.
  /// - Otherwise: push a new `ActiveEffect`.
  ///
  /// **D5α: writes `EquipmentChangedEvent`** for `AttackUp`/`DefenseUp`/
  /// `SpeedUp`/`Dead` to nudge `derive_stats` to re-run via the existing
  /// recompute path.
  ///
  /// **Pitfall 6:** clamps `potency` to `[0.0, 10.0]` at the trust boundary.
  pub fn apply_status_handler(
      mut events: MessageReader<ApplyStatusEvent>,
      mut equip_changed: MessageWriter<EquipmentChangedEvent>,
      mut characters: Query<&mut StatusEffects, With<PartyMember>>,
  ) {
      for ev in events.read() {
          // Pitfall 6: defensive clamp on f32 trust boundary.
          let potency = ev.potency.clamp(0.0, 10.0);

          let Ok(mut status) = characters.get_mut(ev.target) else { continue };

          // Permanent effects already present: re-application is a no-op.
          if matches!(ev.effect, StatusEffectType::Stone | StatusEffectType::Dead)
              && status.has(ev.effect)
          {
              continue;
          }

          // Stacking merge: refresh duration, take higher magnitude.
          if let Some(existing) = status
              .effects
              .iter_mut()
              .find(|e| e.effect_type == ev.effect)
          {
              existing.remaining_turns = ev.duration;
              existing.magnitude = existing.magnitude.max(potency);
          } else {
              status.effects.push(ActiveEffect {
                  effect_type: ev.effect,
                  remaining_turns: ev.duration,
                  magnitude: potency,
              });
          }

          // D5α: nudge derive_stats re-run for stat-affecting variants.
          // EquipSlot::None is the "stat-changed, source not an equip slot"
          // sentinel. recompute_derived_stats_on_equipment_change reads
          // &StatusEffects and applies the new buff branches automatically.
          if matches!(
              ev.effect,
              StatusEffectType::AttackUp
                  | StatusEffectType::DefenseUp
                  | StatusEffectType::SpeedUp
                  | StatusEffectType::Dead
          ) {
              equip_changed.write(EquipmentChangedEvent {
                  character: ev.target,
                  slot: EquipSlot::None,
              });
          }
      }
  }
  ```

- [ ] Replace the `tick_on_dungeon_step` stub (D9-A: every step):

  ```rust
  /// Fires `StatusTickEvent` for every `PartyMember` on every dungeon step.
  ///
  /// D9-A (every step) per research recommendation. To switch to D9-B (every
  /// Nth step), add a `StepsSinceLastTick: Resource(u32)` and gate on
  /// `>= N`. To switch to D9-C (time-based), read `Time::delta` instead.
  /// Both are localized changes to this function only.
  ///
  /// Ordered `.after(handle_dungeon_input)` so the player's commit-frame
  /// position is the trigger context (mirrors `features.rs:160-187`).
  pub fn tick_on_dungeon_step(
      mut moved: MessageReader<MovedEvent>,
      mut tick: MessageWriter<StatusTickEvent>,
      party: Query<Entity, With<PartyMember>>,
  ) {
      for _ev in moved.read() {
          for entity in &party {
              tick.write(StatusTickEvent { target: entity });
          }
      }
  }
  ```

- [ ] Replace the `tick_status_durations` stub:

  ```rust
  /// Decrements `remaining_turns` and removes expired effects.
  ///
  /// `None` (permanent) effects are skipped — they don't tick.
  ///
  /// **D5α:** writes `EquipmentChangedEvent` if a removed effect was a stat-
  /// modifier (`AttackUp`/`DefenseUp`/`SpeedUp`) so `derive_stats` re-runs.
  pub fn tick_status_durations(
      mut ticks: MessageReader<StatusTickEvent>,
      mut equip_changed: MessageWriter<EquipmentChangedEvent>,
      mut characters: Query<&mut StatusEffects, With<PartyMember>>,
  ) {
      for ev in ticks.read() {
          let Ok(mut status) = characters.get_mut(ev.target) else { continue };

          let mut had_stat_modifier_removed = false;

          status.effects.retain_mut(|e| match e.remaining_turns {
              None => true, // permanent — keep
              Some(0) => {
                  // Expired (defensive — should be caught next-frame normally).
                  if matches!(
                      e.effect_type,
                      StatusEffectType::AttackUp
                          | StatusEffectType::DefenseUp
                          | StatusEffectType::SpeedUp
                      ) {
                      had_stat_modifier_removed = true;
                  }
                  false
              }
              Some(ref mut n) => {
                  *n -= 1;
                  if *n == 0 {
                      if matches!(
                          e.effect_type,
                          StatusEffectType::AttackUp
                              | StatusEffectType::DefenseUp
                              | StatusEffectType::SpeedUp
                          ) {
                          had_stat_modifier_removed = true;
                      }
                      false // becomes 0 → drop
                  } else {
                      true
                  }
              }
          });

          if had_stat_modifier_removed {
              equip_changed.write(EquipmentChangedEvent {
                  character: ev.target,
                  slot: EquipSlot::None,
              });
          }
      }
  }
  ```

- [ ] Extend `mod tests` in `combat/status_effects.rs` with 5 Layer-2 tests using a new `make_test_app` helper. Place the tests in a new submodule `mod app_tests` (mirroring `features.rs::app_tests`):

  ```rust
  #[cfg(test)]
  mod app_tests {
      use super::*;
      use crate::plugins::party::{
          BaseStats, Equipment, Experience, PartyMember, PartyMemberBundle,
      };
      use bevy::state::app::StatesPlugin;

      /// Test harness: MinimalPlugins + StatesPlugin + AssetPlugin +
      /// PartyPlugin + StatusEffectsPlugin. NO DungeonPlugin (we don't need
      /// MovedEvent emission in handler/ticker tests; we write StatusTickEvent
      /// directly).
      ///
      /// Pattern from `features.rs:736-771` adapted for #14.
      fn make_test_app() -> App {
          let mut app = App::new();
          app.add_plugins((
              MinimalPlugins,
              bevy::asset::AssetPlugin::default(),
              StatesPlugin,
              crate::plugins::state::StatePlugin,
              crate::plugins::party::PartyPlugin,
              StatusEffectsPlugin,
          ));
          // PartyPlugin's populate_item_handle_registry runs on OnExit(Loading)
          // and reads Assets<ItemDb>. Tests don't trigger that transition;
          // explicit init_asset is still required as defensive setup.
          app.init_asset::<crate::data::ItemDb>();
          app
      }

      fn spawn_party_member(app: &mut App, current_hp: u32) -> Entity {
          app.world_mut()
              .spawn(PartyMemberBundle {
                  derived_stats: crate::plugins::party::DerivedStats {
                      current_hp,
                      max_hp: 100,
                      max_mp: 50,
                      ..Default::default()
                  },
                  ..Default::default()
              })
              .id()
      }

      // ── Stacking ───────────────────────────────────────────────────

      #[test]
      fn apply_status_handler_pushes_new_effect() {
          let mut app = make_test_app();
          let target = spawn_party_member(&mut app, 100);
          app.world_mut().write_message(ApplyStatusEvent {
              target,
              effect: StatusEffectType::Poison,
              potency: 1.0,
              duration: Some(5),
          });
          app.update();
          let status = app.world().get::<StatusEffects>(target).unwrap();
          assert_eq!(status.effects.len(), 1);
          assert_eq!(status.effects[0].effect_type, StatusEffectType::Poison);
          assert_eq!(status.effects[0].magnitude, 1.0);
      }

      #[test]
      fn apply_status_handler_refreshes_duration() {
          let mut app = make_test_app();
          let target = spawn_party_member(&mut app, 100);
          // First Poison: duration 3.
          app.world_mut().write_message(ApplyStatusEvent {
              target,
              effect: StatusEffectType::Poison,
              potency: 0.5,
              duration: Some(3),
          });
          app.update();
          // Second Poison: duration 5, magnitude 1.0.
          app.world_mut().write_message(ApplyStatusEvent {
              target,
              effect: StatusEffectType::Poison,
              potency: 1.0,
              duration: Some(5),
          });
          app.update();
          let status = app.world().get::<StatusEffects>(target).unwrap();
          assert_eq!(status.effects.len(), 1, "stacking does not duplicate");
          assert_eq!(status.effects[0].remaining_turns, Some(5), "duration refreshed");
          assert_eq!(status.effects[0].magnitude, 1.0, "max magnitude wins");
      }

      #[test]
      fn apply_status_handler_takes_higher_magnitude() {
          let mut app = make_test_app();
          let target = spawn_party_member(&mut app, 100);
          // First: magnitude 1.0.
          app.world_mut().write_message(ApplyStatusEvent {
              target,
              effect: StatusEffectType::Poison,
              potency: 1.0,
              duration: Some(5),
          });
          app.update();
          // Second: lower magnitude 0.3.
          app.world_mut().write_message(ApplyStatusEvent {
              target,
              effect: StatusEffectType::Poison,
              potency: 0.3,
              duration: Some(5),
          });
          app.update();
          let status = app.world().get::<StatusEffects>(target).unwrap();
          assert_eq!(status.effects[0].magnitude, 1.0, "higher magnitude wins");
      }

      #[test]
      fn apply_status_handler_stone_reapply_is_noop() {
          let mut app = make_test_app();
          let target = spawn_party_member(&mut app, 100);
          // First Stone (permanent — None duration).
          app.world_mut().write_message(ApplyStatusEvent {
              target,
              effect: StatusEffectType::Stone,
              potency: 1.0,
              duration: None,
          });
          app.update();
          let len_after_first = app.world().get::<StatusEffects>(target).unwrap().effects.len();
          // Re-apply.
          app.world_mut().write_message(ApplyStatusEvent {
              target,
              effect: StatusEffectType::Stone,
              potency: 1.0,
              duration: None,
          });
          app.update();
          let status = app.world().get::<StatusEffects>(target).unwrap();
          assert_eq!(status.effects.len(), len_after_first, "Stone re-apply no-op");
      }

      #[test]
      fn apply_status_handler_clamps_nan_to_zero() {
          let mut app = make_test_app();
          let target = spawn_party_member(&mut app, 100);
          app.world_mut().write_message(ApplyStatusEvent {
              target,
              effect: StatusEffectType::Poison,
              potency: f32::NAN,
              duration: Some(5),
          });
          app.update();
          let status = app.world().get::<StatusEffects>(target).unwrap();
          // NaN.clamp(0.0, 10.0) returns 0.0 in Rust (NaN-to-min behavior).
          assert_eq!(status.effects[0].magnitude, 0.0, "NaN clamps to 0");
      }

      // ── Tick decrement ─────────────────────────────────────────────

      #[test]
      fn tick_decrements_duration() {
          let mut app = make_test_app();
          let target = spawn_party_member(&mut app, 100);
          app.world_mut().write_message(ApplyStatusEvent {
              target,
              effect: StatusEffectType::Poison,
              potency: 0.0,
              duration: Some(3),
          });
          app.update();
          // Tick once.
          app.world_mut().write_message(StatusTickEvent { target });
          app.update();
          let status = app.world().get::<StatusEffects>(target).unwrap();
          assert_eq!(status.effects[0].remaining_turns, Some(2));
      }

      #[test]
      fn tick_removes_expired() {
          let mut app = make_test_app();
          let target = spawn_party_member(&mut app, 100);
          app.world_mut().write_message(ApplyStatusEvent {
              target,
              effect: StatusEffectType::Poison,
              potency: 0.0,
              duration: Some(1),
          });
          app.update();
          app.world_mut().write_message(StatusTickEvent { target });
          app.update();
          let status = app.world().get::<StatusEffects>(target).unwrap();
          assert!(status.effects.is_empty(), "duration-1 expires after tick");
      }

      #[test]
      fn tick_skips_permanent_effects() {
          let mut app = make_test_app();
          let target = spawn_party_member(&mut app, 100);
          app.world_mut().write_message(ApplyStatusEvent {
              target,
              effect: StatusEffectType::Stone,
              potency: 1.0,
              duration: None,
          });
          app.update();
          for _ in 0..5 {
              app.world_mut().write_message(StatusTickEvent { target });
              app.update();
          }
          let status = app.world().get::<StatusEffects>(target).unwrap();
          assert_eq!(status.effects.len(), 1);
          assert!(status.has(StatusEffectType::Stone));
      }
  }
  ```

  (8 Layer-2 tests above — well within the +8-12 envelope for #14. Of these, 5 cover handler behavior; 3 cover ticker behavior. Phase 5 adds 2 more tests for the resolvers; Phase 7 adapts 1 existing test; Phase 8 adds 2 more tests for `derive_stats` order/Dead-dominance.)

- [ ] **Verification (atomic commit boundary):**
  - `cargo build` — succeeds.
  - `cargo test plugins::combat::status_effects::app_tests` — 8 Layer-2 tests pass.
  - `cargo test plugins::combat::status_effects::tests` — 4 Layer-1 tests pass.
  - `cargo test` — full suite passes (existing dungeon/party/inventory tests remain green).

### Phase 5 — `apply_poison_damage` + `apply_regen` resolvers (with D7-A formula)

Replace the 2 resolver stubs with full bodies. Add 3 Layer-2 tests covering damage-on-tick, heal-on-tick, and the resolver-vs-ticker ordering invariant. ~80 LOC.

- [ ] In `src/plugins/combat/status_effects.rs`, replace the `apply_poison_damage` stub:

  ```rust
  /// On every tick, characters with `Poison` take damage proportional to
  /// `magnitude` and `max_hp`.
  ///
  /// **D7-A formula (default):** `damage = ((max_hp / 20).max(1) as f32 * magnitude) as u32; damage.max(1)`.
  /// At `magnitude = 1.0` this is 5% of max_hp per tick (Wizardry/Etrian
  /// canonical). The two `.max(1)` floors guard against:
  /// - low-HP characters (max_hp < 20 → `max_hp / 20 = 0`).
  /// - low-magnitude truncation (`(1 as f32 * 0.4) as u32 = 0`).
  ///
  /// To switch to D7-B (flat): replace the formula with
  /// `let damage = (5.0 * mag) as u32; damage.max(1);`.
  ///
  /// **Pitfall 7:** does NOT apply `Dead` on zero HP. #15 owns that;
  /// `check_dead_and_apply` is the #15-callable convenience.
  pub fn apply_poison_damage(
      mut ticks: MessageReader<StatusTickEvent>,
      mut characters: Query<(&StatusEffects, &mut DerivedStats), With<PartyMember>>,
  ) {
      for ev in ticks.read() {
          let Ok((status, mut derived)) = characters.get_mut(ev.target) else {
              continue;
          };
          let Some(poison) = status
              .effects
              .iter()
              .find(|e| e.effect_type == StatusEffectType::Poison)
          else {
              continue;
          };
          let base = (derived.max_hp / 20).max(1);
          let damage = ((base as f32 * poison.magnitude) as u32).max(1);
          derived.current_hp = derived.current_hp.saturating_sub(damage);
      }
  }
  ```

- [ ] Replace the `apply_regen` stub:

  ```rust
  /// On every tick, characters with `Regen` heal proportional to `magnitude`
  /// and `max_hp`. Mirrors `apply_poison_damage` symmetrically.
  ///
  /// **D7-A formula (default, mirroring poison):** `heal = ((max_hp / 20).max(1) as f32 * magnitude) as u32; heal.max(1)`.
  /// Capped at `max_hp` via `.min(max_hp)` after `saturating_add`.
  pub fn apply_regen(
      mut ticks: MessageReader<StatusTickEvent>,
      mut characters: Query<(&StatusEffects, &mut DerivedStats), With<PartyMember>>,
  ) {
      for ev in ticks.read() {
          let Ok((status, mut derived)) = characters.get_mut(ev.target) else {
              continue;
          };
          let Some(regen) = status
              .effects
              .iter()
              .find(|e| e.effect_type == StatusEffectType::Regen)
          else {
              continue;
          };
          let base = (derived.max_hp / 20).max(1);
          let healing = ((base as f32 * regen.magnitude) as u32).max(1);
          derived.current_hp = derived
              .current_hp
              .saturating_add(healing)
              .min(derived.max_hp);
      }
  }
  ```

- [ ] Extend `combat/status_effects.rs::app_tests` with 3 more Layer-2 tests:

  ```rust
  // ── Resolvers ──────────────────────────────────────────────────

  #[test]
  fn poison_damages_on_tick() {
      let mut app = make_test_app();
      let target = spawn_party_member(&mut app, 100);  // current_hp=100, max_hp=100
      // Apply Poison magnitude 1.0 → 5% of max_hp = 5 damage.
      app.world_mut().write_message(ApplyStatusEvent {
          target,
          effect: StatusEffectType::Poison,
          potency: 1.0,
          duration: Some(5),
      });
      app.update();
      // Tick once.
      app.world_mut().write_message(StatusTickEvent { target });
      app.update();
      let derived = app.world().get::<crate::plugins::party::DerivedStats>(target).unwrap();
      assert_eq!(derived.current_hp, 95, "5% of max_hp=100 = 5 damage");
  }

  #[test]
  fn regen_heals_on_tick_capped_at_max() {
      let mut app = make_test_app();
      let target = spawn_party_member(&mut app, 1);  // current_hp=1, max_hp=100
      app.world_mut().write_message(ApplyStatusEvent {
          target,
          effect: StatusEffectType::Regen,
          potency: 1.0,
          duration: Some(5),
      });
      app.update();
      app.world_mut().write_message(StatusTickEvent { target });
      app.update();
      let derived = app.world().get::<crate::plugins::party::DerivedStats>(target).unwrap();
      assert_eq!(derived.current_hp, 6, "5% of max_hp=100 = 5 heal; 1+5=6");
      // Heal up to cap.
      for _ in 0..50 {
          app.world_mut().write_message(StatusTickEvent { target });
          app.update();
      }
      let derived = app.world().get::<crate::plugins::party::DerivedStats>(target).unwrap();
      assert_eq!(derived.current_hp, 100, "regen caps at max_hp");
  }

  #[test]
  fn duration_one_poison_damages_then_expires_same_frame() {
      // Verifies the resolver-vs-ticker ordering (Critical / Open Item 7):
      // a duration-1 poison must deal damage BEFORE being removed.
      let mut app = make_test_app();
      let target = spawn_party_member(&mut app, 100);
      app.world_mut().write_message(ApplyStatusEvent {
          target,
          effect: StatusEffectType::Poison,
          potency: 1.0,
          duration: Some(1),
      });
      app.update();
      app.world_mut().write_message(StatusTickEvent { target });
      app.update();
      let derived = app.world().get::<crate::plugins::party::DerivedStats>(target).unwrap();
      let status = app.world().get::<StatusEffects>(target).unwrap();
      assert_eq!(derived.current_hp, 95, "duration-1 poison damaged on its final tick");
      assert!(status.effects.is_empty(), "...and was removed after");
  }
  ```

- [ ] **Verification (atomic commit boundary):**
  - `cargo build` — succeeds.
  - `cargo test plugins::combat::status_effects::app_tests` — 11 Layer-2 tests pass.
  - `cargo test` — full suite passes.

### Phase 6 — `EquipmentChangedEvent` doc-comment update + dungeon-step end-to-end test

The smallest of the phases. Update the doc-comment on `EquipmentChangedEvent` to acknowledge the dual-use (Pitfall 3). Add 1 final Layer-2 test verifying the `tick_on_dungeon_step` → resolver flow end-to-end (requires `DungeonPlugin` in the harness).

- [ ] In `src/plugins/party/inventory.rs`, replace the doc-comment at lines 193-202 with:

  ```rust
  /// Emitted by:
  /// - `equip_item` and `unequip_item` (`inventory.rs`) — equipment slot
  ///   changed.
  /// - `apply_status_handler` (Feature #14, `combat/status_effects.rs`) when a
  ///   status change affects derived stats (`AttackUp`/`DefenseUp`/`SpeedUp`/
  ///   `Dead`). The `slot` field is `EquipSlot::None` in that case (sentinel
  ///   for "stat-changed, source not an equipment slot").
  /// - `tick_status_durations` (#14) when an expiring effect was a stat-
  ///   modifier — same `EquipSlot::None` sentinel.
  ///
  /// Triggers `recompute_derived_stats_on_equipment_change`, which re-runs
  /// `derive_stats` (which sees the new buff branches in #14).
  ///
  /// **`#[derive(Message)]`, NOT `#[derive(Event)]`** — Bevy 0.18 family rename.
  /// Use `MessageReader<EquipmentChangedEvent>` to subscribe, and
  /// `app.add_message::<EquipmentChangedEvent>()` to register. The canonical
  /// project precedent is `MovedEvent` at `dungeon/mod.rs:192-197`.
  ///
  /// The `...Event` suffix is a genre-familiarity convention that matches
  /// `MovedEvent`; `SfxRequest` is the counter-example (message without suffix).
  /// **Naming impurity acknowledged** (the type also fires for non-equipment
  /// stat changes after #14); rename deferred to #25 polish.
  ```

  (Pure doc-comment edit; zero behavioral change.)

- [ ] Add a final Layer-2 test in `combat/status_effects.rs::app_tests` that exercises the full dungeon-step → tick_on_dungeon_step → apply_poison_damage chain. This test needs `DungeonPlugin` in the harness, so add a second `make_test_app_with_dungeon` helper:

  ```rust
  /// Test harness with DungeonPlugin — needed for `MovedEvent` emission
  /// and the cross-plugin `tick_on_dungeon_step` → resolver flow.
  fn make_test_app_with_dungeon() -> App {
      use leafwing_input_manager::prelude::ActionState;
      let mut app = App::new();
      app.add_plugins((
          MinimalPlugins,
          bevy::asset::AssetPlugin::default(),
          StatesPlugin,
          bevy::input::InputPlugin,
          crate::plugins::state::StatePlugin,
          crate::plugins::dungeon::DungeonPlugin,
          crate::plugins::party::PartyPlugin,
          StatusEffectsPlugin,
      ));
      app.init_resource::<ActionState<crate::plugins::input::DungeonAction>>();
      app.init_asset::<crate::data::DungeonFloor>();
      app.init_asset::<crate::data::ItemDb>();
      app.init_asset::<bevy::prelude::Mesh>();
      app.init_asset::<bevy::pbr::StandardMaterial>();
      app.add_message::<crate::plugins::audio::SfxRequest>();
      #[cfg(feature = "dev")]
      app.init_resource::<bevy::input::ButtonInput<bevy::prelude::KeyCode>>();
      app
  }

  #[test]
  fn dungeon_step_triggers_poison_tick_end_to_end() {
      let mut app = make_test_app_with_dungeon();

      // Spawn a party member with 100 HP and apply Poison potency 1.0.
      let target = spawn_party_member(&mut app, 100);
      app.world_mut().write_message(ApplyStatusEvent {
          target,
          effect: StatusEffectType::Poison,
          potency: 1.0,
          duration: Some(5),
      });
      app.update(); // flush handler

      // Emit a MovedEvent (the dungeon-step trigger). Use any from/to/facing.
      use crate::data::dungeon::Direction;
      use crate::plugins::dungeon::{GridPosition, MovedEvent};
      app.world_mut().write_message(MovedEvent {
          from: GridPosition { x: 0, y: 0 },
          to: GridPosition { x: 1, y: 0 },
          facing: Direction::East,
      });
      app.update(); // tick_on_dungeon_step → StatusTickEvent → apply_poison_damage

      let derived = app.world().get::<crate::plugins::party::DerivedStats>(target).unwrap();
      assert_eq!(derived.current_hp, 95, "MovedEvent triggered one poison tick");
  }
  ```

  (Note: this test does NOT require an active `GameState::Dungeon` because it doesn't go through `handle_dungeon_input`; it writes `MovedEvent` directly. However, `tick_on_dungeon_step` is `.run_if(in_state(GameState::Dungeon))`, so the test must transition into Dungeon state first. Use `app.world_mut().resource_mut::<NextState<GameState>>().set(GameState::Dungeon); app.update();` before the `MovedEvent` write. The implementer can mirror the `advance_into_dungeon` helper at `features.rs:773` if it's `pub(crate)` — otherwise inline the state transition.)

- [ ] **Verification (atomic commit boundary):**
  - `cargo build` — succeeds.
  - `cargo test plugins::combat::status_effects::app_tests` — 12 Layer-2 tests pass (the new `dungeon_step_triggers_poison_tick_end_to_end` is green).
  - `cargo test` — full suite passes.

### Phase 7 — Refactor `apply_poison_trap` to write `ApplyStatusEvent`

Refactor `features.rs:412-445` from naive `effects.push(...)` to `ApplyStatusEvent` write. Add `.before(apply_status_handler)` to the system registration in `CellFeaturesPlugin::build`. Update the existing test `poison_trap_applies_status` to call `app.update()` twice (Pitfall 1).

- [ ] In `src/plugins/dungeon/features.rs`, add an import for `ApplyStatusEvent` (around line 25, near the other party imports):

  ```rust
  use crate::plugins::combat::status_effects::ApplyStatusEvent;
  ```

  (`StatusEffectType` is already imported at line 33; `ActiveEffect` import becomes unused — remove it from the use statement at line 32-35 if clippy warns.)

- [ ] Replace the body of `apply_poison_trap` at `features.rs:412-445` with:

  ```rust
  /// Apply poison trap on entry. Writes `ApplyStatusEvent` (handled by
  /// `combat/status_effects.rs::apply_status_handler` which enforces stacking).
  /// **Refactored in #14:** prior naive `effects.push(...)` removed; the
  /// canonical handler is now the single mutator of `StatusEffects.effects`.
  ///
  /// Ordered `.before(apply_status_handler)` in `CellFeaturesPlugin::build`
  /// (Pitfall 1 of #14: same-frame consumability).
  fn apply_poison_trap(
      mut moved: MessageReader<MovedEvent>,
      floors: Res<Assets<DungeonFloor>>,
      dungeon_assets: Option<Res<DungeonAssets>>,
      active_floor: Res<ActiveFloorNumber>,
      party: Query<Entity, With<PartyMember>>,
      mut apply: MessageWriter<ApplyStatusEvent>,
      mut sfx: MessageWriter<SfxRequest>,
  ) {
      const POISON_DURATION_TICKS: u32 = 5;
      const POISON_TRAP_POTENCY: f32 = 1.0; // Risk 3: NOT 0.0.

      let Some(assets) = dungeon_assets else {
          return;
      };
      let floor_handle = floor_handle_for(&assets, active_floor.0);
      let Some(floor) = floors.get(floor_handle) else {
          return;
      };
      for ev in moved.read() {
          let cell = &floor.features[ev.to.y as usize][ev.to.x as usize];
          if !matches!(cell.trap, Some(TrapType::Poison)) {
              continue;
          }
          for entity in &party {
              apply.write(ApplyStatusEvent {
                  target: entity,
                  effect: StatusEffectType::Poison,
                  potency: POISON_TRAP_POTENCY,
                  duration: Some(POISON_DURATION_TICKS),
              });
          }
          sfx.write(SfxRequest {
              kind: SfxKind::Door,
          }); // placeholder hiss (D10-A reuse — unchanged from #13)
      }
  }
  ```

- [ ] In `src/plugins/dungeon/features.rs::CellFeaturesPlugin::build`, add `.before(apply_status_handler)` to the `apply_poison_trap` registration. The block at `features.rs:168-170` becomes:

  ```rust
  apply_poison_trap
      .run_if(in_state(GameState::Dungeon))
      .after(handle_dungeon_input)
      .before(crate::plugins::combat::status_effects::apply_status_handler),
  ```

  (Cross-plugin `.before(...)` works because both systems are in the `Update` schedule and the scheduler resolves cross-plugin ordering by name. If the implementer hits a circular-import issue, fall back to importing `apply_status_handler` at the top of `features.rs` and using the bare name.)

- [ ] In `src/plugins/dungeon/features.rs::app_tests::poison_trap_applies_status` (the existing test at lines 938-978), change `app.update();` at line 961 to:

  ```rust
  app.update(); // apply_poison_trap writes ApplyStatusEvent
  app.update(); // apply_status_handler reads it and pushes to StatusEffects
  ```

  (Pitfall 1. The `.before(apply_status_handler)` registration above SHOULD make a single `app.update()` sufficient because both systems run in the same frame, but Bevy's same-frame system ordering is best-guaranteed when both systems are in the SAME plugin. For cross-plugin ordering, the safer test pattern is `app.update()` × 2 — explicit and obvious. The implementer can try `× 1` first and fall back if the test fails.)

- [ ] **Verification (atomic commit boundary):**
  - `cargo build` — succeeds. The `apply_poison_trap` signature change compiles.
  - `cargo test plugins::dungeon::features::app_tests::poison_trap_applies_status` — passes.
  - `cargo test plugins::combat::status_effects::app_tests` — all 12 Layer-2 tests pass.
  - `cargo test` — full suite passes. Existing dungeon/party/inventory tests remain green.
  - **Behavioral verification:** `rg 'effects\.push\(' src/plugins/` — should match ZERO times outside `combat/status_effects.rs::apply_status_handler` (the handler is the sole mutator per Decision 20).

### Phase 8 — `derive_stats` order-independence + Dead-dominance regression tests

Add the two final tests for `derive_stats` to lock the buff invariants. ~30 LOC of test code, no production-code changes.

- [ ] In `src/plugins/party/character.rs::tests`, add two regression tests after the `derive_stats_attack_up_buffs_attack` test added in Phase 2:

  ```rust
  #[test]
  fn derive_stats_status_order_independent() {
      // Pitfall 2 of #14: the merge rule guarantees AT MOST ONE of each
      // variant is present in StatusEffects; iteration order over different
      // variant types must not change the result.
      let base = BaseStats {
          strength: 10,
          vitality: 10,
          ..Default::default()
      };
      // Order A: AttackUp first, DefenseUp second.
      let mut status_a = StatusEffects::default();
      status_a.effects.push(ActiveEffect {
          effect_type: StatusEffectType::AttackUp,
          remaining_turns: Some(3),
          magnitude: 0.5,
      });
      status_a.effects.push(ActiveEffect {
          effect_type: StatusEffectType::DefenseUp,
          remaining_turns: Some(3),
          magnitude: 0.3,
      });
      // Order B: DefenseUp first, AttackUp second.
      let mut status_b = StatusEffects::default();
      status_b.effects.push(ActiveEffect {
          effect_type: StatusEffectType::DefenseUp,
          remaining_turns: Some(3),
          magnitude: 0.3,
      });
      status_b.effects.push(ActiveEffect {
          effect_type: StatusEffectType::AttackUp,
          remaining_turns: Some(3),
          magnitude: 0.5,
      });
      let a = derive_stats(&base, &[], &status_a, 1);
      let b = derive_stats(&base, &[], &status_b, 1);
      assert_eq!(a.attack, b.attack, "AttackUp/DefenseUp order independent");
      assert_eq!(a.defense, b.defense, "AttackUp/DefenseUp order independent");
  }

  #[test]
  fn derive_stats_dead_dominates_buffs() {
      // Pitfall 4 of #14: Dead branch runs LAST and zeros max_hp/max_mp.
      // Buffs above don't bypass it.
      let base = BaseStats {
          strength: 10,
          vitality: 10,
          ..Default::default()
      };
      let mut status = StatusEffects::default();
      status.effects.push(ActiveEffect {
          effect_type: StatusEffectType::AttackUp,
          remaining_turns: Some(3),
          magnitude: 0.5,
      });
      status.effects.push(ActiveEffect {
          effect_type: StatusEffectType::Dead,
          remaining_turns: None,
          magnitude: 0.0,
      });
      let derived = derive_stats(&base, &[], &status, 1);
      assert_eq!(derived.max_hp, 0, "Dead zeros max_hp");
      assert_eq!(derived.max_mp, 0, "Dead zeros max_mp");
      // Attack is NOT zeroed — Dead doesn't touch offensive stats.
      // Buff still applied: 10 strength + 50% = 15 attack.
      assert_eq!(derived.attack, 15, "Dead doesn't zero attack; buff applies");
  }
  ```

- [ ] **Verification (atomic commit boundary):**
  - `cargo test plugins::party::character::tests::derive_stats_status_order_independent` — passes.
  - `cargo test plugins::party::character::tests::derive_stats_dead_dominates_buffs` — passes.
  - `cargo test` — full suite passes.

### Phase 9 — Final verification gate

Mirrors the 9-command gate from Feature #13 plus #14-specific greps. This phase is verification only; no code changes.

- [ ] `cargo check` — base build, no features.
- [ ] `cargo check --features dev` — dev build (no startup regressions).
- [ ] `cargo clippy --all-targets -- -D warnings` — base clippy; zero warnings.
- [ ] `cargo clippy --all-targets --features dev -- -D warnings` — dev clippy; zero warnings.
- [ ] `cargo fmt --check` — formatting clean.
- [ ] `cargo test` — all unit + integration tests pass (base). Expected count: pre-#14 baseline (~127 lib + 6 integration) + 11 new lib tests (8 in `combat/status_effects::app_tests`, 3 in `combat/status_effects::tests`, 2 in `character::tests`, +1 from existing test still passing) = ~138 lib tests.
- [ ] `cargo test --features dev` — all tests pass (dev).
- [ ] `rg 'derive\(.*\bEvent\b' src/plugins/combat/status_effects.rs` — ZERO matches. Confirms `Message` (NOT `Event`) is the derive used.
- [ ] `rg '\bEventReader<' src/plugins/combat/status_effects.rs tests/` — ZERO matches.
- [ ] `rg '\bEventWriter<' src/plugins/combat/status_effects.rs tests/` — ZERO matches.
- [ ] `rg 'effects\.push\(' src/plugins/` — ZERO matches OUTSIDE `combat/status_effects.rs::apply_status_handler` (Decision 20: handler is sole mutator).
- [ ] **Frozen-file diff audit:** `git diff <pre-#14-commit> HEAD --name-only` confirms permitted modifications only:
  - `src/plugins/combat/status_effects.rs` (NEW)
  - `src/plugins/combat/mod.rs` (+3 lines)
  - `src/plugins/party/character.rs` (variant additions, doc-comment updates, buff branches, +3 tests)
  - `src/plugins/party/inventory.rs` (doc-comment only)
  - `src/plugins/dungeon/features.rs` (apply_poison_trap refactor, system registration, test update)
  - No other files modified.
- [ ] **Dependency audit:** `git diff <pre-#14-commit> HEAD -- Cargo.toml Cargo.lock` — byte-unchanged (zero new deps per Critical).
- [ ] **Test-count audit:** new tests are 11+ (within +8-12 envelope per research):
  - `combat/status_effects::tests` — 4 (3 predicates + `check_dead_and_apply` two cases)
  - `combat/status_effects::app_tests` — 12 (5 stacking, 3 ticker, 3 resolvers, 1 end-to-end)
  - `character::tests` — 3 new (`status_effect_type_dead_serializes_to_index_4`, `derive_stats_attack_up_buffs_attack`, `derive_stats_status_order_independent`, `derive_stats_dead_dominates_buffs` — actually 4 new)
  - Existing test adapted: `features::app_tests::poison_trap_applies_status` (`× 2` update)
  - Total new: 4 + 12 + 4 = 20 new tests. Above the +8-12 envelope. The planner can trim 5-7 lower-value tests if budget tightens; the highest-value tests are the 5 stacking tests, the 3 resolver tests, the 2 derive_stats invariant tests.
- [ ] **Manual smoke checklist:** `cargo run --features dev`, navigate via F9 cycler (Title → Loading → TitleScreen → Dungeon). Verify:
  - **Poison trap at floor_01 (~4,4):** walk into the cell. Status icon (deferred to #25) doesn't show, but observe that on each subsequent step, party HP drops by ~5 (5% of max_hp = 100). After 5 steps, HP no longer drops (effect expired). **PASS criteria:** HP visibly reduces on dungeon steps; effect terminates after 5 ticks; multiple poison applications stack correctly (re-walking the trap re-applies via the canonical handler).
  - **Buff smoke (no in-game source until #15):** SKIP. Buffs are not player-applicable in #14 (no spell/item/trap that grants `AttackUp`). Layer-2 tests cover the buff-via-`derive_stats` path; manual smoke for buffs ships with #15.
  - **Permanent effects (Stone, Dead):** SKIP. No source in #14. Layer-2 test `apply_status_handler_stone_reapply_is_noop` covers the stacking semantics; manual smoke ships with #15/#18.
  - **No regression in #13 features:** doors still open with F, spinners still spin, pit traps still damage. **Critical:** the `apply_poison_trap` refactor must not break existing poison-trap behavior — confirm by walking onto the poison trap and observing the same SFX as pre-#14.
- [ ] Update the `## Implementation Discoveries` section of THIS plan file with any unexpected findings during implementation.
- [ ] Update planner memory (`.claude/agent-memory/planner/`) with a new `project_druum_status_effects.md` entry summarizing the Feature #14 architectural decisions for future planners.

---

## Security

### Known Vulnerabilities

No known CVEs as of 2026-05-07 (research date) for any library used in #14. The dep set is unchanged from Feature #13; same status as #11/#12/#13.

| Library | Version | Status |
|---------|---------|--------|
| serde | 1.x | No advisories |
| ron | 0.12 | No advisories |
| bevy | =0.18.1 | No advisories |
| bevy_common_assets | =0.16.0 | No advisories |
| bevy_asset_loader | =0.26.0 | No advisories |
| leafwing-input-manager | =0.20.0 | No advisories (#14 doesn't use it) |

**Zero new dependencies.** Cargo.toml byte-unchanged.

### Architectural Risks

The trust boundary for #14 is the `ApplyStatusEvent.potency` field (any caller in the codebase, including future modders/scripted events) and the on-disk `StatusEffects` Vec (deferred to #23). The risks below are pre-mitigated by the NaN clamp, the saturating arithmetic, and the merge-rule invariant.

| Risk | How it manifests | Guard required by #14 |
|------|------------------|----------------------|
| `ApplyStatusEvent { potency: f32::NAN }` from any source | NaN propagates through `derive_stats` (`stat * NAN = NAN`); downstream comparisons return false; combat math destabilizes | Phase 4 `apply_status_handler` calls `ev.potency.clamp(0.0, 10.0)` BEFORE any merge or push. NaN clamps to 0.0 (Rust's NaN-clamp behavior). Tested by `apply_status_handler_clamps_nan_to_zero`. |
| `ApplyStatusEvent { potency: f32::INFINITY }` | Saturating arithmetic in `derive_stats` would still produce `u32::MAX` (correct) but the `magnitude` stored in `StatusEffects` would persist as INF, leaking into save files | Same `clamp(0.0, 10.0)` guard caps incoming potency at 10x — well past any sane buff. Phase 4 test already covers NaN; INF is bounded by the same clamp. |
| Crafted save with `StatusEffects.effects.len() == u32::MAX` | OOM at deserialization | Trust boundary is "save file" — deferred to #23. **#14 flags for #23 plan:** `Vec<ActiveEffect>` deserialization must bound length. |
| Crafted `ApplyStatusEvent { effect: Dead }` from a hostile debug command | Auto-kills any party member | The handler doesn't validate the target; #15's combat-state check (is target alive? in combat?) is the right validation point. **#14 flags for #15 plan:** `apply_status_handler` should call a future `validate_target` predicate. |
| Cross-plugin system ordering deferred to next frame (Risk 1) | `apply_poison_trap` writes `ApplyStatusEvent`; if the handler runs first, the message is queued for next frame; player observes 1-frame delay between trap and effect | Phase 7 adds `.before(apply_status_handler)` to `apply_poison_trap` registration. Same-frame consumability guaranteed. Tested by `poison_trap_applies_status` (existing test). |
| `apply_poison_damage` reads stale `magnitude` after handler updates it | `apply_poison_damage` runs in same frame as `apply_status_handler`; if the handler runs after the resolver, the resolver sees the OLD magnitude | NOT a real risk: the handler is triggered by `ApplyStatusEvent` writes (different message); the resolver is triggered by `StatusTickEvent` writes. The two messages have separate cursors; the handler's update is visible to the next-frame resolver read. Documented in the system ordering. |

**Trust boundary recap:** The `ApplyStatusEvent.potency` field is the only untrusted-shape input to #14. The clamp guard at the handler is the single trust-boundary check. Save-file integrity defers to #23. No network input.

---

## Implementation Discoveries

### D-I1 — `app.world_mut().write_message(T)` API does NOT exist in Bevy 0.18

The plan's Phase 3-5 test templates used `app.world_mut().write_message(ApplyStatusEvent { ... })`. This method does not exist in Bevy 0.18. The correct API is `app.world_mut().resource_mut::<Messages<T>>().write(ev)`. All `app_tests` use this pattern, mirroring the existing `write_moved` helper in `features.rs`. **Fix applied:** replaced all plan template test calls with the `resource_mut::<Messages<T>>().write(ev)` pattern via helper functions `write_apply_status` and `write_status_tick`.

### D-I2 — `Messages<T>::len()` does not exist; use `iter_current_update_messages().count()`

The plan's test templates also used `messages.len()` on a `Messages<T>` resource. This method is not available. The correct pattern (per existing codebase usage in features.rs) is `resource::<Messages<T>>().iter_current_update_messages().count()`. **Fix applied:** rewrote the relevant tests to not count messages but instead check observable side effects (StatusEffects component state).

### D-I3 — `check_dead_and_apply` tests moved to `app_tests` (Layer-2) rather than `tests` (Layer-1)

The plan specified `check_dead_and_apply_writes_when_hp_zero` and `check_dead_and_apply_no_op_when_hp_positive` as Layer-1 tests in the `tests` module. However, `check_dead_and_apply` takes a `&mut MessageWriter<ApplyStatusEvent>` parameter — a Bevy system parameter that cannot be constructed outside a system context. **Fix applied:** both tests moved to `app_tests` as Layer-2 tests. They use a helper `system_call_check_dead` system registered via `app.add_systems(Update, system_call_check_dead)` to get a real `MessageWriter` context. The verification checkboxes in this plan's `## Verification` section reference the wrong module paths for these two tests — they should be `app_tests::check_dead_and_apply_*`, not `tests::check_dead_and_apply_*`. **Impact:** Layer-1 test count is 2 (not 4); Layer-2 test count is 14 (not 12); total is unchanged at 16.

### D-I4 — `CombatPlugin` missing from `features.rs` test harness `make_test_app()`

The plan said to add `.before(apply_status_handler)` to `apply_poison_trap` registration and to call `app.update()` × 2 in the `poison_trap_applies_status` test. But without `CombatPlugin` in the test app, `apply_status_handler` is never registered — `ApplyStatusEvent` messages go unread and Poison is never applied, causing the test to fail. **Fix applied:** added `crate::plugins::combat::CombatPlugin` to `make_test_app()` in `features.rs`. Not mentioned in the plan.

### D-I5 — `regen_heals_on_tick_capped_at_max` test duration bug

The plan's Phase 5 test template used `duration: Some(5)` for Regen in `regen_heals_on_tick_capped_at_max`, then expected `current_hp = 100` after 50 more ticks. With `duration: Some(5)`, Regen expires after 5 ticks and only heals ~26 HP (1 + 5×5 = 26). The test would fail at the cap assertion. **Fix applied:** changed duration to `Some(100)` (enough to heal fully within 50 ticks at 5 HP/tick starting from current_hp=1 with max_hp=100). The test now correctly validates the cap at max_hp.

### D-I6 — 9-phase atomic commit structure collapsed due to same-file edits

Phases 1, 2, and 8 all touch `character.rs`. GitButler stages files (not hunks), so making 3 separate commits for these phases requires either hunk-level staging or separate files. Since all code was written together and GitButler's `but stage <file>` is file-level, the 9 atomic commits described in the plan are replaced with a smaller set:
- Commit 1: `character.rs` (phases 1+2+8 combined — enum extension, buff branches, all 4 tests)
- Commit 2: `combat/status_effects.rs` + `combat/mod.rs` (phases 3-5 — new file + plugin wiring)
- Commit 3: `inventory.rs` (phase 6 — doc-comment only)
- Commit 4: `features.rs` (phase 7 — refactor + test update)
- Commit 5: Verification marker (phase 9 — no code changes)

### D-I7 — `ApplyStatusEvent` import in `features.rs` was out of alphabetical order

The previous implementer added `use crate::plugins::combat::status_effects::ApplyStatusEvent;` after the `crate::plugins::loading::DungeonAssets` import in `features.rs`, placing it after alphabetical order (`combat` < `dungeon` < `input` < `loading`). `cargo fmt --check` would have failed. **Fix applied (2026-05-07 verification session):** moved the import to the correct position after `crate::plugins::audio::...` and before `crate::plugins::dungeon::...`.

### D-I8 — `f32::clamp` propagates NaN; explicit `is_finite()` guard required

The plan's Critical / Pitfall 6 said the NaN clamp at the trust boundary maps NaN to 0.0 via `clamp(0.0, 10.0)`. The original implementer wrote `let potency = ev.potency.clamp(0.0, 10.0);` and a code comment claiming "NaN clamps to the lower bound". This is **factually wrong about Rust semantics:** `f32::clamp` propagates NaN through (documented behavior — NaN in, NaN out). The unit test `apply_status_handler_clamps_nan_to_zero` would have failed against the original code. **Fix applied (verification session, 2026-05-07):** wrapped in an `is_finite()` guard so non-finite values explicitly map to 0.0:

```rust
let potency = if ev.potency.is_finite() {
    ev.potency.clamp(0.0, 10.0)
} else {
    0.0
};
```

The test now passes. The misleading code comment was also corrected to say "f32::clamp propagates NaN through, so handle non-finite first."

### D-I9 — Misleading test-side comment about clamp+NaN

`src/plugins/combat/status_effects.rs:635` (test for the NaN clamp) had a comment claiming the handler used `clamp` directly to map NaN to the lower bound. After D-I8's fix, the comment was corrected to say the handler explicitly maps non-finite potency to 0.0 via the `is_finite()` guard.

### D-I10 — `dungeon/tests.rs` has its OWN `make_test_app`; CombatPlugin needed there too

D-I4 added `CombatPlugin` to `features.rs::make_test_app()`. But `src/plugins/dungeon/tests.rs` defines a **separate** `make_test_app` for dungeon-internal tests. Without `CombatPlugin` registered there, 7 dungeon tests panicked on `MessageWriter<ApplyStatusEvent>::messages failed validation` because `apply_poison_trap` (now ordered `.before(apply_status_handler)`) writes `ApplyStatusEvent` but the handler — and the `Messages<ApplyStatusEvent>` resource it registers via `add_message::<T>()` — is missing. **Fix applied (verification session, 2026-05-07):** added `use crate::plugins::combat::CombatPlugin;` and `CombatPlugin` to the `add_plugins` tuple in `tests.rs::make_test_app()`. The 7 failures all turned green.

### D-I11 — Integration tests have third and fourth test harnesses

`tests/dungeon_geometry.rs` and `tests/dungeon_movement.rs` are integration tests (separate crate boundary). Each defines its own helper that builds an App; both omitted `CombatPlugin` for the same reason as D-I10. **Fix applied (verification session, 2026-05-07):** added `use druum::plugins::combat::CombatPlugin;` and `CombatPlugin` in their `add_plugins` tuples. **Lesson:** when adding a plugin that introduces a system with cross-plugin `.before(...)`/`.after(...)` ordering, audit ALL test harnesses in the repo (`rg 'fn make_test_app|fn build_test_app|App::new\(\)' src/ tests/`), not just the one nearest the modified code.

### D-I12 — `cargo fmt` applied tiny style normalizations

After all behavior-affecting fixes (D-I8 through D-I11) were in place, `cargo fmt` rewrote a small set of cosmetic items: alphabetized one `use` line, aligned a comment block in the `StatusEffectType` enum, and reformatted one multi-line `assert_eq! `. Zero semantic change. The verification session ran `cargo fmt --check` afterward (PASS) before declaring the gate green.

---

## Risks (Top 3)

### Risk 1 — `apply_status_handler` ordering against writers (cross-plugin same-frame readability)

**What goes wrong:** `apply_poison_trap` (in `dungeon/features.rs`) writes `ApplyStatusEvent` in `Update`. `apply_status_handler` (in `combat/status_effects.rs`) reads in `Update`. Without explicit ordering, the handler may run BEFORE the writer, causing the message to defer to the next frame. Tests pass (because they call `app.update()` × 2), but production behavior is "1-frame delay between trap and effect" — visible to the player as "I stepped on poison but my icon didn't update until I moved again."

**Likelihood:** Medium (Bevy's default cross-plugin system order is non-deterministic).

**Mitigation:** Phase 7 adds `.before(apply_status_handler)` to `apply_poison_trap` in `CellFeaturesPlugin::build`. **Symmetric pattern for future writers:** every `ApplyStatusEvent` writer (future enemy spell in #15, future item in #20) must be `.before(apply_status_handler)`. Document this as a contract in the doc-comment at the top of `combat/status_effects.rs`. Same shape as `handle_dungeon_input.before(animate_movement)` at `dungeon/mod.rs:241`.

### Risk 2 — Save-format breakage from variant order

**What goes wrong:** A future contributor adds `AttackUp` between `Poison` and `Sleep` in `StatusEffectType`, shifting every saved status effect's discriminant. Existing saves load garbage. The variant order looks alphabetical (`Paralysis, Poison, Sleep, Stone, Dead, AttackUp, ...`) which tempts reordering.

**Likelihood:** Medium-high (alphabetical sort temptation is real; future contributors may not read Pitfall 5 of #11 / Decision 7).

**Mitigation:** Phase 1 adds the comment marker `// HISTORICAL APPEND ORDER — DO NOT REORDER. New variants go at end.` directly above the enum at `character.rs:235`. Plus a regression test `status_effect_type_dead_serializes_to_index_4` that pins the discriminant indices via `as u8` cast — any reorder fails CI.

### Risk 3 — `magnitude: 0.0` poison silent zero damage

**What goes wrong:** Pre-#14, `apply_poison_trap` at `features.rs:439` writes `magnitude: 0.0`. After the refactor, if a future contributor copy-pastes the old shape and writes `potency: 0.0`, the new `apply_poison_damage` formula produces `damage = ((max_hp / 20).max(1) as f32 * 0.0) as u32 = 0` — poison does nothing. Existing test `poison_trap_applies_status` only checks the effect is present, NOT that damage applies on tick.

**Likelihood:** High if the planner overlooks; Medium with the explicit named constant.

**Mitigation:** Phase 7 introduces `const POISON_TRAP_POTENCY: f32 = 1.0;` (NOT `0.0`) and uses the named constant in the `ApplyStatusEvent.potency` field. The `apply_poison_damage` formula has an additional `.max(1)` floor on the integer damage value — even at very low magnitudes, damage is at least 1. Phase 5's test `poison_damages_on_tick` covers the success case (`magnitude = 1.0` → 5 damage); Phase 5's `duration_one_poison_damages_then_expires_same_frame` exercises the resolver-vs-ticker invariant.

---

## Out of scope

What #14 explicitly does NOT do (deferred to later features):

- **UI status icons on party portraits.** Defer to #25 (UI polish). The data is there (`StatusEffects` component); #25 will read it and render an icon per active effect.
- **Save plugin work.** Defer to #23. `StatusEffects` already derives `Serialize + Deserialize + Reflect`. #23 will add the deserialize-time bound on `Vec<ActiveEffect>` length (security flag).
- **Combat-round emitter of `StatusTickEvent`.** Defer to #15. One line in `turn_manager.rs::round_end`: `for entity in alive_combatants { tick.write(StatusTickEvent { target: entity }) }`.
- **`Blind`/`Confused` enum variants AND predicates.** Defer to #15 (with their predicate consumers). Decision 8 / Open Item 2 / Pitfall 5 — declaring them in #14 burns save-format slots speculatively.
- **`Dead`-on-zero-HP auto-application.** Defer to #15. **#14 ships `pub fn check_dead_and_apply` stub for #15 to import and call.** Decision 11 / Pitfall 7.
- **Action-blocking wiring.** Defer to #15. **#14 ships `pub fn is_paralyzed/is_asleep/is_silenced` predicates for #15 to import.** #15 wires them into `turn_manager::collect_player_actions`. Decision 9.
- **`is_blind`/`is_confused` accuracy/random-target logic.** Defer to #15.
- **Ailment-curing potion or spell.** Defer to #20 (consumables). Either uses `RemoveStatusEvent` (new) or `ApplyStatusEvent { duration: Some(0) }` to expire next tick.
- **Magic resistance / saving throws against status apply.** Defer to #15 / #20. The handler currently always succeeds; #15 may interpose a roll between the writer and the handler.
- **Status effect on enemies.** Defer to #15. The handler queries `With<PartyMember>`; #15 will broaden to include enemy entities or split into per-faction variants.
- **`floor_01.dungeon.ron` or `floor_02.dungeon.ron` edits.** Defer (none needed). #13's existing poison trap is sufficient testbed.
- **`SfxKind` additions.** Defer (none needed). The `apply_poison_trap` refactor reuses the existing `SfxKind::Door` placeholder hiss (`features.rs:441-443`).

---

## Verification

- [ ] `character.rs` Layer-1 discriminant-order test — Layer-1 unit — `cargo test plugins::party::character::tests::status_effect_type_dead_serializes_to_index_4` — Automatic
- [ ] `character.rs` Layer-1 AttackUp buff test — Layer-1 unit — `cargo test plugins::party::character::tests::derive_stats_attack_up_buffs_attack` — Automatic
- [ ] `character.rs` Layer-1 order-independence — Layer-1 unit — `cargo test plugins::party::character::tests::derive_stats_status_order_independent` — Automatic
- [ ] `character.rs` Layer-1 Dead dominance — Layer-1 unit — `cargo test plugins::party::character::tests::derive_stats_dead_dominates_buffs` — Automatic
- [ ] `combat/status_effects.rs` Layer-1 predicates — Layer-1 unit — `cargo test plugins::combat::status_effects::tests::is_paralyzed_returns_true_when_present` — Automatic
- [ ] `combat/status_effects.rs` Layer-1 silenced absent — Layer-1 unit — `cargo test plugins::combat::status_effects::tests::is_silenced_returns_false_when_absent` — Automatic
- [ ] `combat/status_effects.rs` Layer-1 check_dead_and_apply writes — Layer-1 unit — `cargo test plugins::combat::status_effects::tests::check_dead_and_apply_writes_when_hp_zero` — Automatic
- [ ] `combat/status_effects.rs` Layer-1 check_dead_and_apply no-op — Layer-1 unit — `cargo test plugins::combat::status_effects::tests::check_dead_and_apply_no_op_when_hp_positive` — Automatic
- [ ] `combat/status_effects.rs` Layer-2 push new effect — Layer-2 integration — `cargo test plugins::combat::status_effects::app_tests::apply_status_handler_pushes_new_effect` — Automatic
- [ ] `combat/status_effects.rs` Layer-2 stacking refresh — Layer-2 integration — `cargo test plugins::combat::status_effects::app_tests::apply_status_handler_refreshes_duration` — Automatic
- [ ] `combat/status_effects.rs` Layer-2 stacking max magnitude — Layer-2 integration — `cargo test plugins::combat::status_effects::app_tests::apply_status_handler_takes_higher_magnitude` — Automatic
- [ ] `combat/status_effects.rs` Layer-2 Stone re-apply no-op — Layer-2 integration — `cargo test plugins::combat::status_effects::app_tests::apply_status_handler_stone_reapply_is_noop` — Automatic
- [ ] `combat/status_effects.rs` Layer-2 NaN clamp — Layer-2 integration — `cargo test plugins::combat::status_effects::app_tests::apply_status_handler_clamps_nan_to_zero` — Automatic
- [ ] `combat/status_effects.rs` Layer-2 tick decrement — Layer-2 integration — `cargo test plugins::combat::status_effects::app_tests::tick_decrements_duration` — Automatic
- [ ] `combat/status_effects.rs` Layer-2 tick removes expired — Layer-2 integration — `cargo test plugins::combat::status_effects::app_tests::tick_removes_expired` — Automatic
- [ ] `combat/status_effects.rs` Layer-2 permanent doesn't tick — Layer-2 integration — `cargo test plugins::combat::status_effects::app_tests::tick_skips_permanent_effects` — Automatic
- [ ] `combat/status_effects.rs` Layer-2 poison damages — Layer-2 integration — `cargo test plugins::combat::status_effects::app_tests::poison_damages_on_tick` — Automatic
- [ ] `combat/status_effects.rs` Layer-2 regen heals capped — Layer-2 integration — `cargo test plugins::combat::status_effects::app_tests::regen_heals_on_tick_capped_at_max` — Automatic
- [ ] `combat/status_effects.rs` Layer-2 duration-1 final tick — Layer-2 integration — `cargo test plugins::combat::status_effects::app_tests::duration_one_poison_damages_then_expires_same_frame` — Automatic
- [ ] `combat/status_effects.rs` Layer-2 dungeon-step end-to-end — Layer-2 integration — `cargo test plugins::combat::status_effects::app_tests::dungeon_step_triggers_poison_tick_end_to_end` — Automatic
- [ ] `features.rs` poison_trap still passes after refactor — Layer-2 integration — `cargo test plugins::dungeon::features::app_tests::poison_trap_applies_status` — Automatic
- [ ] `cargo check && cargo check --features dev` — Build — Automatic
- [ ] `cargo clippy --all-targets -- -D warnings && cargo clippy --all-targets --features dev -- -D warnings` — Lint — Automatic
- [ ] `cargo fmt --check` — Format — Automatic
- [ ] `cargo test && cargo test --features dev` — Full test suite — Automatic
- [ ] No `Event` derive sneaks in — Grep — `rg 'derive\(.*\bEvent\b' src/plugins/combat/status_effects.rs` — Automatic (ZERO matches)
- [ ] No `EventReader` consumer sneaks in — Grep — `rg '\bEventReader<' src/plugins/combat/status_effects.rs tests/` — Automatic (ZERO matches)
- [ ] No `EventWriter` consumer sneaks in — Grep — `rg '\bEventWriter<' src/plugins/combat/status_effects.rs tests/` — Automatic (ZERO matches)
- [ ] Sole-mutator invariant (Decision 20) — Grep — `rg 'effects\.push\(' src/plugins/` — Automatic (ZERO matches OUTSIDE `combat/status_effects.rs::apply_status_handler`)
- [ ] No edits to frozen files — Diff — `git diff <pre-#14-commit> HEAD --name-only` — Manual (cascading struct-literal fixes acceptable; logic changes only in the 5 listed files)
- [ ] Cargo.toml byte-unchanged — Diff — `git diff <pre-#14-commit> HEAD -- Cargo.toml Cargo.lock` — Manual
- [ ] Manual smoke: poison trap inflicts damage on dungeon steps and expires after 5 ticks — Smoke — manual `cargo run --features dev` — Manual
- [ ] Manual smoke: no #13 regressions (doors, spinners, pits, teleporters all work) — Smoke — manual `cargo run --features dev` — Manual
- [ ] Plan's "Implementation Discoveries" section populated — Documentation — manual review of THIS plan file post-implementation — Manual

---

## Notes for the orchestrator

- **D7 and D9 are the only two USER PICK decisions.** Both default to research-recommended option A. If the user picks non-default:
  - **D7 → B (flat damage):** Phase 5 changes the formula in `apply_poison_damage` and `apply_regen` from `(max_hp / 20).max(1) * magnitude` to `5.0 * magnitude`. ~3 LOC each.
  - **D9 → B (every Nth step):** Phase 4 adds a `StepsSinceLastTick: Resource(u32)` and gates `tick_on_dungeon_step` on `>= N` with a reset. +1 resource declaration, +5 LOC body.
  - **D9 → C (time-based):** Phase 4 changes `tick_on_dungeon_step` to read `Time::delta` and accumulate; ~10 LOC body change.
- **Phase 7's existing-test edit** (`features.rs:961` adding a second `app.update()`) is the only change to a previously-frozen test. The justification is Pitfall 1 (cross-plugin same-frame consumability). If the implementer finds that one `app.update()` works (because of `.before(apply_status_handler)`), the test can stay at one update — but two is the safer default.
- **The `#13`-frozen files touched** are limited to: `combat/mod.rs` (3 lines), `party/character.rs` (variant additions + buff branches + tests), `party/inventory.rs` (doc-comment only), `dungeon/features.rs` (one function refactor + one test edit + one system-registration `.before()` clause). Diff audit at Phase 9 must confirm no other frozen files are touched.
- **Pre-commit hook on `gitbutler/workspace`** rejects raw `git commit` (CLAUDE.md). Implementer uses `but commit --message-file <path>`. One commit per phase boundary; 9 commits total.
- **The `register_type::<StatusEffectType>` line at `party/mod.rs:47` does NOT need updating** for the new variants (Decision M of research). `#[derive(Reflect)]` on the enum at `character.rs:235` covers all variants automatically.
- **`SavePlugin` is empty (`save/mod.rs:1-9`).** #14 ships zero save-plugin work. The `StatusEffects` component already derives `Serialize + Deserialize + Reflect` (`character.rs:264`); save-format wiring lands in #23.
- **#15's interface contract from #14:**
  - Import `StatusEffectsPlugin`'s public types: `ApplyStatusEvent`, `StatusTickEvent`, `apply_status_handler`, `tick_status_durations`.
  - Import predicates: `is_paralyzed`, `is_asleep`, `is_silenced`. Wire into `turn_manager::collect_player_actions`.
  - Import stub: `check_dead_and_apply`. Call from combat damage resolver after `current_hp` updates.
  - Add a one-line emitter in `turn_manager::round_end`: `for entity in alive_combatants { tick.write(StatusTickEvent { target: entity }); }`.
  - Add `Blind` and `Confused` enum variants AT END of `StatusEffectType` (indices 10, 11).
  - Add `is_blind` and `is_confused` predicates in `combat/status_effects.rs` (or a new sibling file).
