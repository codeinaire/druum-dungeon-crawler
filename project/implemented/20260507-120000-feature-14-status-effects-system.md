# Implementation Summary — Feature #14: Status Effects System

**Date:** 2026-05-07
**Plan:** `project/plans/20260507-124500-feature-14-status-effects-system.md`
**Branch:** TBD — atomic commit via GitButler is created in a follow-up step. This document is updated after the commit lands.
**Status:** Code complete; verification gate GREEN; awaiting commit.

---

## Steps Completed

All 9 phases implemented (phases 1+2+8 share `character.rs`; phases 3-5 share `status_effects.rs`):

- **Phase 1** — `StatusEffectType` extended with 5 new variants (`AttackUp`/`DefenseUp`/`SpeedUp`/`Regen`/`Silence` at indices 5-9). HISTORICAL APPEND ORDER comment added. `StatusEffectType` and `ActiveEffect.magnitude` doc-comments updated. `status_effect_type_dead_serializes_to_index_4` regression test added.
- **Phase 2** — `derive_stats` buff branches: `stat_attack`/`stat_defense`/`stat_speed` promoted to `let mut`. `AttackUp`/`DefenseUp`/`SpeedUp` buff loop inserted before `Dead` branch (Pitfall 4). `derive_stats` doc-comment updated. `derive_stats_attack_up_buffs_attack` test added.
- **Phase 3** — `src/plugins/combat/status_effects.rs` new file: file-level doc-comment, `ApplyStatusEvent`/`StatusTickEvent` messages, `StatusEffectsPlugin` struct.
- **Phase 4** — `apply_status_handler` (NaN guard via explicit `is_finite()` test — see D-I8 — stacking merge, D5α `EquipmentChangedEvent`), `tick_on_dungeon_step` (D9-A), `tick_status_durations` (decrement/retain_mut/D5α on stat-modifier removal) all implemented.
- **Phase 5** — `apply_poison_damage` (D7-A formula: `((max_hp/20).max(1) * mag) as u32).max(1)`) and `apply_regen` (mirror with `.min(max_hp)` cap) resolvers implemented. Is-predicate functions and `check_dead_and_apply` stub added.
- **Phase 6** — `EquipmentChangedEvent` doc-comment updated in `inventory.rs` to acknowledge dual-use (equip/unequip AND apply_status_handler/tick_status_durations with `EquipSlot::None` sentinel).
- **Phase 7** — `apply_poison_trap` in `features.rs` refactored from naive `effects.push(...)` to `ApplyStatusEvent` write. `.before(apply_status_handler)` added to registration. `poison_trap_applies_status` test updated to `app.update() × 2`. `CombatPlugin` added to `features.rs` test harness (D-I4).
- **Phase 8** — `derive_stats_status_order_independent` and `derive_stats_dead_dominates_buffs` regression tests added to `character.rs::tests`.
- **Phase 9** — Full verification gate executed and GREEN — see Verification Results below.

---

## Steps Skipped

None. All phases implemented.

---

## Deviations from Plan (Static-Analysis Phase)

These were captured in the plan's `## Implementation Discoveries` section as D-I1 through D-I7 during the initial implementation pass.

1. **D-I1: `app.world_mut().write_message(T)` API does not exist.** Plan's test templates used this method. Fixed to use `resource_mut::<Messages<T>>().write(ev)` via helper functions `write_apply_status` and `write_status_tick`.
2. **D-I2: `Messages<T>::len()` does not exist.** Use `iter_current_update_messages().count()` (or, in our tests, observe component side effects instead of counting messages).
3. **D-I3: `check_dead_and_apply` tests moved to `app_tests` (Layer-2).** Plan specified these as Layer-1 tests. The function takes a `&mut MessageWriter<ApplyStatusEvent>` system parameter that cannot be constructed outside a system. Tests use a helper system `system_call_check_dead` registered via `add_systems`.
4. **D-I4: `CombatPlugin` added to `features.rs::make_test_app()`.** Plan didn't mention this. Without it, `apply_status_handler` isn't registered and `poison_trap_applies_status` would fail. (See also D-I10/D-I11 — there are TWO MORE test harnesses needing the same fix; only surfaced when cargo actually ran.)
5. **D-I5: `regen_heals_on_tick_capped_at_max` duration changed from `Some(5)` to `Some(100)`.** Plan used `Some(5)` which would cause Regen to expire after 5 ticks (only ~26 HP healed), failing the cap assertion.
6. **D-I6: 9-phase commits collapsed to 5 — and ultimately 1 — due to same-file edits.** Phases 1+2+8 all touch `character.rs`; GitButler stages files not hunks. The static-analysis phase planned 5 commits; the actual atomic commit ships as a single comprehensive commit per project precedent.
7. **D-I7: Import ordering in `features.rs`.** Cross-plugin import was out of alphabetical order; would have failed `cargo fmt --check`. Fix applied during verification session.

---

## Additional Fixes (Verification Phase)

The static-analysis-phase implementer's claimed "verification" was a static dry-run; the gate had not actually been executed. This section captures the fixes that only surfaced when `cargo` actually ran. They are also recorded in the plan as D-I8 through D-I12.

8. **D-I8 — `f32::clamp` propagates NaN.** `src/plugins/combat/status_effects.rs:181-186`. Original code `let potency = ev.potency.clamp(0.0, 10.0);` propagates NaN per Rust documented semantics; the test `apply_status_handler_clamps_nan_to_zero` would have failed. Fix: explicit `is_finite()` guard mapping non-finite to 0.0.
9. **D-I9 — Misleading test-side comment** at `src/plugins/combat/status_effects.rs:635` corrected to reflect the actual `is_finite()` guard.
10. **D-I10 — `src/plugins/dungeon/tests.rs::make_test_app` needed `CombatPlugin`.** Different `make_test_app` than the one D-I4 fixed in `features.rs`. 7 dungeon tests panicked on `MessageWriter<ApplyStatusEvent>::messages failed validation` until `CombatPlugin` was registered.
11. **D-I11 — Integration test harnesses needed `CombatPlugin`.** `tests/dungeon_geometry.rs` and `tests/dungeon_movement.rs` are separate crate boundary; each defines its own helper. Both omitted `CombatPlugin` for the same reason as D-I10. **Lesson:** audit ALL `make_test_app` definitions in the repo (`rg 'fn make_test_app|fn build_test_app|App::new\(\)' src/ tests/`), not just the one nearest the modified code.
12. **D-I12 — `cargo fmt` applied tiny style normalizations.** Alphabetized one `use` line, aligned comment block in `StatusEffectType` enum, reformatted one multi-line `assert_eq! `. Zero semantic change.

---

## Verification Results

Full gate executed during verification session 2026-05-07. All checks GREEN:

| Gate | Result |
|---|---|
| `cargo check` | PASS — finished in 1.16s |
| `cargo check --features dev` | PASS — finished in 2.19s |
| `cargo test` | PASS — 159 passed (153 lib + 6 integration), 0 failed |
| `cargo test --features dev` | PASS — 162 passed, 0 failed |
| `cargo clippy --all-targets -- -D warnings` | PASS — clean |
| `cargo clippy --all-targets --features dev -- -D warnings` | PASS — clean |
| `cargo fmt --check` | PASS |
| `rg 'derive\(.*\bEvent\b' src/plugins/combat/status_effects.rs` | 0 matches |
| `rg '\bEventReader<' src/plugins/combat/status_effects.rs tests/` | 0 matches |
| `rg '\bEventWriter<' src/plugins/combat/status_effects.rs tests/` | 0 matches |
| `rg 'effects\.push\(' src/plugins/` | 0 matches outside `apply_status_handler` |

---

## Files Touched

| File | Change |
|------|--------|
| `src/plugins/party/character.rs` | +5 enum variants, +3 `let mut`, buff loop, Dead comment, doc updates, +4 tests (~80 LOC net) |
| `src/plugins/combat/status_effects.rs` | NEW — ~925 LOC (systems, tests, doc); includes `is_finite()` NaN guard |
| `src/plugins/combat/mod.rs` | +3 lines (pub mod, pub use, add_plugins) |
| `src/plugins/party/inventory.rs` | Doc-comment only (+8 lines) |
| `src/plugins/dungeon/features.rs` | `apply_poison_trap` body rewrite, import change, registration `.before(...)`, test update, `CombatPlugin` in test harness (~25 LOC net) |
| `src/plugins/dungeon/tests.rs` | `CombatPlugin` added to `make_test_app` (D-I10) |
| `tests/dungeon_geometry.rs` | `CombatPlugin` added to harness (D-I11) |
| `tests/dungeon_movement.rs` | `CombatPlugin` added to harness (D-I11) |

---

## Branch / Commits

TBD — atomic commits via GitButler are made in a separate step. This document is updated after commits land.

---

## Link to Plan

`project/plans/20260507-124500-feature-14-status-effects-system.md`
