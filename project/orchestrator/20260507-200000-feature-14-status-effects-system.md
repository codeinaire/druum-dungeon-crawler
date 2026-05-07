# Pipeline Summary — Feature #14: Status Effects System (Research → Plan → Implement → Review)

**Date:** 2026-05-07
**Pipeline scope:** research → plan → implement → review → STOP. Shipping requires explicit user authorization.
**Status:** Pipeline complete. Verification GREEN. Review verdict LGTM with all actionable findings addressed. Awaiting user authorization for commit/ship.

---

## Original task

Drive the full feature pipeline for **Feature #14: Status Effects System** from the Druum (Bevy 0.18 first-person dungeon crawler) roadmap (`project/roadmaps/20260429-01-bevy-dungeon-crawler-roadmap.md` lines 741-786). Difficulty 2.5/5, depends on #11 (already implemented).

**In scope:** central `ApplyStatusEvent` and `StatusTickEvent` messages, `apply_status_handler` (sole mutator of `StatusEffects.effects` with NaN guard, stacking merge, and re-derive trigger), `tick_status_durations`, `tick_on_dungeon_step` reading `MovedEvent`, `apply_poison_damage` and `apply_regen` resolvers, `pub fn` predicates (`is_paralyzed`, `is_asleep`, `is_silenced`) and `check_dead_and_apply` stub for #15, 5 new `StatusEffectType` variants (`AttackUp`, `DefenseUp`, `SpeedUp`, `Regen`, `Silence`) appended at indices 5-9, buff branches in `derive_stats` before the existing `Dead` zero-out, refactor of `apply_poison_trap` from naive `effects.push` to canonical `ApplyStatusEvent` write.

**Out of scope (deferred):**
- `Blind`, `Confused` enum variants — deferred to #15 with their predicate consumers (avoids burning save-format slots speculatively, Pitfall 5).
- `Dead`-on-zero-HP application path — deferred to #15 (`check_dead_and_apply` stub provided).
- Combat-round `StatusTickEvent` emitter — one line in `turn_manager::round_end`, ships with #15.
- Status icon UI — deferred to #25.
- Save-plugin work — none required (effects are already serde-derived; #23).

**Constraint envelope (final):** +495 LOC across 4 source files + 1 new file, **0 new deps** (`Cargo.toml` and `Cargo.lock` byte-unchanged), +20 tests (above the roadmap +8-12 budget; flagged in plan), 0 asset Δ, +0.2s compile.

---

## Artifacts produced

| Step | Description | Path |
|------|-------------|------|
| 1 | Research | `project/research/20260507-115500-feature-14-status-effects-system.md` |
| 2 | Plan | `project/plans/20260507-124500-feature-14-status-effects-system.md` (Status: Approved 2026-05-07) |
| 3 | Implementation summary | `project/implemented/20260507-120000-feature-14-status-effects-system.md` |
| 4 | Code review | `project/reviews/20260507-153000-feature-14-status-effects-system.md` (Verdict: LGTM) |
| - | This summary | `project/orchestrator/20260507-200000-feature-14-status-effects-system.md` |
| - | Pipeline state | `project/orchestrator/PIPELINE-STATE.md` |

---

## User decisions

Two USER PICK decisions were surfaced at plan-approval time. User accepted both defaults:

- **D7 = A** — Per-tick poison/regen damage formula: `((max_hp / 20).max(1) as f32 * magnitude) as u32` with `.max(1)` floor. Wizardry-canonical, scales with character power.
- **D9 = A** — Dungeon-step tick frequency: one tick per `MovedEvent` per `PartyMember`. Wizardry-canonical.

Architecture is identical across A/B/C alternatives for both decisions; switching tuning calls would be localized formula changes only.

---

## Verification gate

Executed against the implemented codebase 2026-05-07:

| Check | Result |
|-------|--------|
| `cargo check` | PASS (1.16s) |
| `cargo check --features dev` | PASS (2.19s) |
| `cargo test` | PASS — 159 tests (153 lib + 6 integration), 0 failed |
| `cargo test --features dev` | PASS — 162 tests, 0 failed |
| `cargo clippy --all-targets -- -D warnings` | PASS |
| `cargo clippy --all-targets --features dev -- -D warnings` | PASS |
| `cargo fmt --check` | PASS (after auto-fmt applied) |
| `rg 'derive(...Event...)' status_effects.rs` | 0 matches (Message-only) |
| `rg 'EventReader<\|EventWriter<' status_effects.rs tests/` | 0 matches |
| `rg 'effects\.push\(' src/plugins/` | only inside `apply_status_handler` and `#[cfg(test)]` test fixtures |
| `Cargo.toml` / `Cargo.lock` | byte-unchanged |

The plan's verification gate ("evidence before assertions") passed in full.

---

## Review findings

Reviewer verdict: **LGTM**. Severity counts: 0 critical, 0 high, 1 medium, 2 low, 1 nit.

| Severity | Finding | Resolution |
|----------|---------|------------|
| MEDIUM | `check_dead_and_apply` test scheduling non-deterministic (`status_effects.rs:817-852`) | FIXED — both tests now register `system_call_check_dead.before(apply_status_handler)`; conditional `if !has_dead { app.update(); }` removed; single `app.update()` per test. |
| LOW 1 | Plan's frozen-file audit needed retroactive amendment for `dungeon/tests.rs` and integration-test harnesses | FIXED — added fifth carve-out bullet in plan §`## Frozen post-#13 / DO NOT TOUCH` documenting D-I10/D-I11. |
| LOW 2 | Stone/Dead-with-`Some(n)` invariant relied on caller convention only | FIXED — added explicit doc-comment on `ApplyStatusEvent.duration` field at `status_effects.rs:67-70` documenting the requirement. |
| NIT | Test harness `CombatPlugin` vs `add_message::<ApplyStatusEvent>()` tradeoff | NO ACTION — flagged for awareness only. Current `CombatPlugin` choice is forward-compatible with #15. |

Post-fix re-run: `cargo test` PASS (159 tests), `cargo fmt --check` PASS.

---

## What this enables

- **#15 (Turn-Based Combat Core)** can `use druum::plugins::combat::status_effects::{is_paralyzed, is_asleep, is_silenced}` for per-action gating and `check_dead_and_apply` for damage post-resolution. The combat-round `StatusTickEvent` emitter is a single line in `turn_manager::round_end`.
- **#16 (Encounter System)** writes `ApplyStatusEvent` for enemy-cast effects via the same canonical path traps already use.
- **#20 (Spell System)** reuses `ApplyStatusEvent` for offensive (Poison, Sleep, Paralysis, Silence) and supportive (Regen, AttackUp, DefenseUp, SpeedUp) spells.
- **#23 (Save / Load)** gets effects-on-disk for free — `StatusEffects` was already serde-derived and the new variant order is locked by `// HISTORICAL APPEND ORDER — DO NOT REORDER` plus the `status_effect_type_dead_serializes_to_index_4` regression test.

---

## Implementation notes worth keeping

These are insights from the run that aren't obvious from the code alone:

1. **`f32::clamp` propagates NaN** — Rust documents this; the original NaN clamp was wrong. Trust-boundary value sanitization for `f32` should always start with `is_finite()` (or equivalent) before `clamp`. Captured as D-I8 in the plan and the impl summary.
2. **Audit ALL `make_test_app` definitions** when adding a new system to a frozen plugin — not just the harness nearest the modified code. The `dungeon` module had three: one in `features.rs::tests` (in scope per the plan), one in `dungeon/tests.rs` (missed), and two integration-test harnesses (`tests/dungeon_geometry.rs`, `tests/dungeon_movement.rs`, also missed). Captured as D-I10/D-I11.
3. **`MessageWriter::messages failed validation`** is the panic signature when a system tries to write a `Message<T>` whose registry was never installed. Adding the source plugin (or `app.add_message::<T>()`) to the test harness fixes it; there is no quieter degradation.
4. **Re-derive trigger reuse** (D5α decision) — passing `EquipSlot::None` as a sentinel through the existing `EquipmentChangedEvent` pipeline avoids a parallel `StatusEffectsChangedEvent` and zero-cost reuses `recompute_derived_stats_on_equipment_change`. Recommended pattern when adding cross-cutting stat triggers.
5. **Resolver-before-ticker ordering is load-bearing.** A duration-1 effect must get its final tick of damage *before* `tick_status_durations` removes it. Test `duration_one_poison_damages_then_expires_same_frame` covers this contract.

---

## Pipeline scope conclusion

Pipeline ran research → plan → implement → review and produced all five artifacts. Verification gate is GREEN; reviewer verdict is LGTM with all actionable findings addressed. Code is committed-pending; PR is awaiting explicit user authorization (per CLAUDE.md GitButler discipline and the original brief).

To ship: user picks a commit strategy (single comprehensive commit titled `feat(combat): Feature #14 — status effects system` is the default per the implementer's planning), then `but push -u origin feature/14-status-effects-system` followed by `gh pr create`.
