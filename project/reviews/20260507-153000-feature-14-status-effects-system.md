# Code Review — Feature #14: Status Effects System

**Date:** 2026-05-07
**Verdict:** LGTM (with 1 medium and 2 low findings)
**Reviewed:** local working-tree diff against `gitbutler/workspace` HEAD (no PR)

---

## Behavioral delta

The diff introduces the status effects resolution layer:

- `ApplyStatusEvent` / `StatusTickEvent` messages are now registered and consumed.
- `apply_status_handler` is the sole mutator of `StatusEffects.effects`.
- Every dungeon step now emits `StatusTickEvent` per party member via `tick_on_dungeon_step`.
- `apply_poison_damage` and `apply_regen` resolve tick damage/healing against `DerivedStats.current_hp`.
- `tick_status_durations` decrements and prunes expired effects; writes `EquipmentChangedEvent` when a stat modifier is removed (D5α).
- `apply_poison_trap` in `features.rs` is refactored from direct `effects.push` to `ApplyStatusEvent` write.
- `derive_stats` gains `AttackUp/DefenseUp/SpeedUp` buff branches before the existing `Dead` zero-out.
- Five new `StatusEffectType` variants appended at indices 5-9 (append-only, LOCKED order).
- Three predicate functions (`is_paralyzed`, `is_asleep`, `is_silenced`) and a `check_dead_and_apply` stub exported as `pub fn` for Feature #15.

---

## Critical constraint compliance

Every constraint from the plan's "non-negotiable" list was verified:

| Constraint | Status |
|---|---|
| `#[derive(Message)]` only — zero `derive(Event)` / `EventReader` / `EventWriter` | PASS |
| Append-only `StatusEffectType` — new variants at indices 5-9 after `Dead` | PASS |
| `HISTORICAL APPEND ORDER` comment present | PASS |
| `status_effect_type_dead_serializes_to_index_4` regression test covers all 10 variants | PASS |
| `Dead` branch LAST in `derive_stats` — buff loop runs before the `if status.has(Dead)` block | PASS |
| `derive_stats_dead_dominates_buffs` test exists | PASS |
| NaN guard: `is_finite()` explicit check mapping to `0.0`, not bare `clamp` | PASS |
| Subnormal floats pass `is_finite()` → `clamp(0.0, 10.0)` handles them correctly | PASS |
| `apply_status_handler_clamps_nan_to_zero` test exists | PASS |
| System ordering: `tick_on_dungeon_step.before(tick_status_durations/apply_poison_damage/apply_regen)` | PASS |
| `apply_poison_damage.before(tick_status_durations)` | PASS |
| `apply_regen.before(tick_status_durations)` | PASS |
| `duration_one_poison_damages_then_expires_same_frame` test covers the critical ordering contract | PASS |
| `apply_poison_trap.before(apply_status_handler)` in `CellFeaturesPlugin::build` | PASS |
| `effects.push` discipline: zero matches outside `apply_status_handler` (verified by gate grep) | PASS |
| `apply_poison_damage` minimum-damage floor: `((max_hp/20).max(1) as f32 * mag) as u32).max(1)` | PASS |
| `POISON_TRAP_POTENCY = 1.0` (not 0.0) | PASS |
| `check_dead_and_apply` signature matches D13: `(Entity, &DerivedStats, &mut MessageWriter<...>)` | PASS |
| Predicates are `pub fn`, not systems | PASS |
| Stone/Dead re-apply no-op and `apply_status_handler_stone_reapply_is_noop` test | PASS |
| Stacking: duration refresh + `.max(magnitude)` and tests covering both rules | PASS |
| `Cargo.toml` / `Cargo.lock` byte-unchanged | PASS (gate: `cargo check` clean, no new deps) |

---

## Findings

---

### [MEDIUM] `check_dead_and_apply_writes_when_hp_zero` test scheduling is non-deterministic

**File:** `src/plugins/combat/status_effects.rs:817-838`

**Issue:** `system_call_check_dead` is added to `Update` with no `.before()` or `.after()` constraint relative to `apply_status_handler`. The test accounts for this with a conditional `if !has_dead { app.update(); }` that runs a second frame when the scheduler happens to order the writer after the reader. The test is correct in both orderings, but:

1. The true execution path depends on Bevy's internal schedule-build order, which is an implementation detail that could change across patch releases.
2. The conditional branch makes it possible for CI to always exercise one path and miss a subtle regression in the other.

The test does pass (cargo test: GREEN) in both orderings, so this is not a blocking bug. But the non-determinism is unnecessary and the fix is one line.

**Suggested fix:** Add `.before(apply_status_handler)` to the system registration, then simplify to a single `app.update()`:

```rust
app.add_systems(
    bevy::app::Update,
    system_call_check_dead.before(apply_status_handler),
);
let target = spawn_party_member(&mut app, 0);
app.update();
let status = app.world().get::<StatusEffects>(target).unwrap();
assert!(status.has(StatusEffectType::Dead), "Dead applied when hp == 0");
```

---

### [LOW] Plan frozen-file audit needs retroactive amendment for test harness additions

**Files:** `src/plugins/dungeon/tests.rs`, `tests/dungeon_geometry.rs`, `tests/dungeon_movement.rs`

**Issue:** The plan's "Frozen post-#13 / DO NOT TOUCH" section lists four explicit carve-outs (`combat/mod.rs`, `party/character.rs`, `party/inventory.rs`, `dungeon/features.rs`). Three additional files were modified by the verification phase (D-I10, D-I11): `dungeon/tests.rs` and both integration test harnesses, which needed `CombatPlugin` to prevent panics from `MessageWriter<ApplyStatusEvent>::messages failed validation`. These are pure test-only additions with zero behavioral impact, and they fix real panics — they were the right call. However, the plan document does not reflect them as carve-outs.

**Recommended action:** Amend the plan's frozen-file audit to add a fifth bullet: *"`src/plugins/dungeon/tests.rs`, `tests/dungeon_geometry.rs`, `tests/dungeon_movement.rs` — test harnesses only; `CombatPlugin` added (D-I10/D-I11) to register `Messages<ApplyStatusEvent>` required by `apply_status_handler`."* No code change needed.

---

### [LOW] Stone / Dead applied with `duration: Some(n)` creates an unintended temporary effect

**File:** `src/plugins/combat/status_effects.rs:192-197`

**Issue:** `apply_status_handler` prevents RE-applying Stone or Dead when already present, but does not validate that the INITIAL application uses `duration: None`. A caller passing `duration: Some(3)` for Stone is accepted and creates a Stone effect that `tick_status_durations` will decrement and eventually remove. The permanent-effect invariant is enforced by caller convention, not by the handler itself.

No current caller does this (all Stone/Dead callers in scope pass `None`), and the `check_dead_and_apply` stub correctly passes `duration: None`. The risk is a future caller in #15 or beyond inadvertently passing a non-None duration for Stone.

**Suggested fix:** Add a defensive assertion or early clamp:

```rust
// Before the merge/push path:
let duration = if matches!(ev.effect, StatusEffectType::Stone | StatusEffectType::Dead) {
    None // permanents are always permanent
} else {
    ev.duration
};
```

Or, document the invariant explicitly in the `ApplyStatusEvent` doc-comment at the `duration` field:

```rust
/// - For `Stone` and `Dead`, MUST be `None` (permanent). The handler does not
///   validate this; passing `Some(n)` creates an unintended temporary petrification.
```

Either fix is acceptable; the documentation approach is lower risk for a stub-only feature.

---

### [NIT] Test harness CombatPlugin vs. `app.add_message::<ApplyStatusEvent>()` tradeoff

**Files:** `src/plugins/dungeon/tests.rs:165`, `tests/dungeon_geometry.rs:39`, `tests/dungeon_movement.rs:27`

**Issue (discussion only):** The review prompt asks whether pulling in the entire `CombatPlugin` is cleaner than `app.add_message::<ApplyStatusEvent>()` standalone. The tradeoff:

- `CombatPlugin` is simpler to maintain: when #15 adds more systems to `CombatPlugin`, all test harnesses automatically pick them up. No test needs updating.
- `app.add_message::<ApplyStatusEvent>()` standalone is more minimal: only registers the type, doesn't add any systems. Tests that don't care about status-effect behavior are less likely to see surprising system side effects.

The current choice (full `CombatPlugin`) is the right call for `dungeon/tests.rs` (which tests systems that interact with status effects) and `features.rs::app_tests` (same). For `tests/dungeon_geometry.rs` and `tests/dungeon_movement.rs`, which test spatial queries unrelated to combat, the `add_message` approach would be marginally cleaner and would insulate those tests from future #15 system changes. However, since `CombatPlugin` currently only adds `StatusEffectsPlugin` systems and two log stubs, and cargo clippy is clean, there's no concrete harm. No change required; flagging for awareness only.

---

## Review Summary

| Severity | Count |
|----------|-------|
| CRITICAL | 0     |
| HIGH     | 0     |
| MEDIUM   | 1     |
| LOW      | 2     |
| NIT      | 1     |

**Verdict: LGTM**

All non-negotiable constraints from the plan are satisfied. The NaN guard correctly uses `is_finite()` rather than bare `clamp`. System ordering is correctly expressed and tested. The append-only enum invariant is locked by both a comment and a CI regression test. The sole mutator discipline is verified by gate grep. The borrow shapes in `apply_poison_damage` and `tick_status_durations` are safe (Bevy serializes them via `.before()` ordering; no aliasing).

The one MEDIUM finding (`check_dead_and_apply` test non-determinism) is a test hygiene issue, not a production correctness bug. The two LOW findings are a plan-document gap and a missing defensive invariant. None block merge.

**Files reviewed (full coverage):**
- `src/plugins/combat/status_effects.rs` (925 LOC — full)
- `src/plugins/combat/mod.rs` (full)
- `src/plugins/party/character.rs` (full)
- `src/plugins/dungeon/features.rs` (full)
- `src/plugins/dungeon/tests.rs` (partial — first 186 lines of `make_test_app` context)
- `src/plugins/party/inventory.rs` (doc-comment region only, lines 190-219)
- `tests/dungeon_geometry.rs` (partial — harness setup lines 1-55)
- `tests/dungeon_movement.rs` (partial — harness setup lines 1-50)
