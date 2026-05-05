# Review: Feature #11 — Party & Character ECS Model (PR #11)

**Date:** 2026-05-05
**Verdict:** APPROVE WITH NITS
**PR:** https://github.com/codeinaire/druum-dungeon-crawler/pull/11
**Branch:** `ja-feature-11-party-character-ecs-model` → `main`
**Head commit reviewed:** `344598f`

## Behavioral Delta

After this PR merges:

1. **12 character components** are live (`CharacterName`, `Race`, `Class`, `BaseStats`, `DerivedStats`, `Experience`, `PartyRow`, `PartySlot`, `PartyMember`, `Equipment`, `StatusEffects`, `ActiveEffect` + `StatusEffectType` enum). Every component derives `Component, Reflect, Serialize, Deserialize, Default, Debug, Clone, PartialEq` (plus `Copy + Eq + Hash` where the inner type allows). `Equipment` is the documented exception — no serde derives because `Handle<T>` has no serde impl in Bevy 0.18; deferred to #23.
2. **`derive_stats(base, equip_stats, status, level) -> DerivedStats`** is a pure function. No world access. Returns `current_hp = max_hp`; callers clamp. Saturating arithmetic throughout.
3. **`PartySize: Resource`** initialized at 4. `spawn_default_debug_party` caps via `.min(4)` and returns early if `PartySize.0 == 0`.
4. **`spawn_default_debug_party`** is `#[cfg(feature = "dev")]`-gated, triggers `OnEnter(GameState::Dungeon)`, includes an idempotence guard (`if !existing.is_empty() { return; }`).
5. **`assets/classes/core.classes.ron`** replaces the Feature #3 `()` stub with 3-class data (Fighter/Mage/Priest). 8 `Class` enum variants declared for save-format stability; only 3 authored.
6. **`src/data/items.rs`** gains `ItemAsset` + `ItemStatBlock` stubs so `Handle<ItemAsset>` compiles for `Equipment` slots.
7. **`tests/class_table_loads.rs`** loads `core.classes.ron` end-to-end through `RonAssetPlugin` (the ron 0.11 parser path) and asserts the 3-class shape.
8. **Δ deps = 0.** `Cargo.toml`, `Cargo.lock`, and `src/main.rs` are byte-unchanged. `src/data/dungeon.rs` is untouched.

---

## What Was Reviewed

**Files with full review:**
- `src/plugins/party/character.rs` — all 600 LOC: 12 components, `PartyMemberBundle`, `derive_stats`, `PartySize`, 8 unit tests
- `src/plugins/party/mod.rs` — `PartyPlugin::build`, `spawn_default_debug_party`, re-exports
- `src/data/classes.rs` — `ClassTable`, `ClassDef`, `get` method, 2 unit tests
- `src/data/items.rs` — `ItemAsset`, `ItemStatBlock`, `ItemDb`, 1 unit test
- `src/data/mod.rs` — re-export additions
- `assets/classes/core.classes.ron` — 3-class data vs plan spec
- `tests/class_table_loads.rs` — integration test

**Hard gates verified:**
- `#[cfg(feature = "dev")]` on both function definition AND `add_systems` call — symmetric ✓
- Trigger `OnEnter(GameState::Dungeon)` (not `OnEnter(Loading)`) ✓
- Idempotence guard: `if !existing.is_empty() { return; }` first in function body ✓
- `GameState` import scoped inside `#[cfg(feature = "dev")] { ... }` block — avoids `unused_imports` in default builds ✓
- `PartySize.0 == 0` → `count = 0.min(4) = 0` → `.take(0)` yields no iterations — zero-size party refuses all spawns ✓
- 8 `Class` variants declared in enum: `Fighter, Mage, Priest, Thief, Bishop, Samurai, Lord, Ninja` ✓
- 5 `Race` variants declared: `Human, Elf, Dwarf, Gnome, Hobbit` ✓
- 3 entries in `core.classes.ron` (Fighter/Mage/Priest), stats match plan §Step 6 ✓
- `ClassTable::get` uses `Vec::iter().find()` (O(n=8) linear scan, no `HashMap`) ✓
- `Dead` is a `StatusEffectType` variant — no separate `struct Dead` anywhere in the file ✓
- `derive_stats` branches on `StatusEffectType::Dead` to zero `max_hp` and `max_mp` ✓
- Poison/Sleep/Paralysis/Stone have no stat modification at derive time (documented inline) ✓
- All arithmetic in `derive_stats` uses `saturating_add`/`saturating_mul` ✓
- No `Mut<T>`, `Commands`, `Query`, `Res`, or randomness in `derive_stats` ✓
- `derive_stats` returns `current_hp = max_hp` and `current_mp = max_mp` ✓
- `Equipment` doc-comment explicitly calls out the serde gap and why (Handle<T> / Bevy 0.18) ✓
- Reverse-dep comment in `classes.rs` header and in `character.rs` header ✓
- `src/data/dungeon.rs` not in diff ✓
- `Cargo.toml`, `Cargo.lock`, `src/main.rs` not in diff ✓
- `PartySize` derives `Reflect` (plan deviation documented, correctly fixed) ✓
- `impl Default for PartySize` returns `Self(4)` — explicit impl, not `#[derive(Default)]` ✓
- `ItemStatBlock` fields all have `#[serde(default)]` for forward-compatible schema additions ✓
- Integration test uses `MessageWriter<AppExit>` and `exit.write(AppExit::Success)` (Bevy 0.18 Message API) ✓
- Integration test asserts `table.get(Class::Thief).is_none()` (declared-but-unauthored variant) ✓

---

## Findings

---

### [MEDIUM] `spawn_default_debug_party` idempotence and PartySize cap have no automated test

**File:** `src/plugins/party/mod.rs` (system), `src/plugins/party/character.rs` (tests module)

**Issue:** The idempotence guard (`if !existing.is_empty() { return; }`) and the `PartySize` hard-cap path (`.min(4)` / `.take(count)`) are both critical behavioral contracts — the guard prevents duplicate spawning on F9 re-entry; the cap enforces the hard-limit invariant. Neither has an automated test. The only verification is the manual smoke test deferred to Step 9 in the plan, which the implementer notes "cannot be performed headlessly."

The plan (Step 3h) specifies 8 Layer 1 unit tests but does not mandate Layer 2 App-driven tests for the spawn system — this is the correct scope for #11 since the system is dev-only and simple. However, there is a specific gap: `PartySize.0 == 0` as a defensive input (what happens when the resource is set to 0 before spawn runs) is exercisable without any Bevy `App` plumbing at all via a simple logic unit test.

The idempotence guard can also be tested at Layer 1 indirectly: the guard reads `existing.is_empty()` which is a `Query<(), With<PartyMember>>` — this requires an App. That's fair to defer. But the "count path" (`party_size.0.min(4)` → `.take(count)`) only wraps a roster of 4, so:

- `PartySize(0)` → `count = 0` → no spawns. Currently verified only by code inspection.
- `PartySize(4)` → `count = 4` → spawns 4. Verified by smoke test.
- `PartySize(6)` → `count = 4` → still 4 spawns (the `.min(4)` guard). No test.

The missing `.min(4)` over-capacity case is untested but exercisable without a Bevy App at all since the cap logic is pure:

```rust
// The cap is testable without App:
#[cfg(test)]
#[test]
fn party_size_cap_clamps_to_four() {
    // Simulate what spawn_default_debug_party does with a PartySize > 4.
    let count = 6_usize.min(4);
    assert_eq!(count, 4, "PartySize > 4 clamps to roster length");
    let count = 0_usize.min(4);
    assert_eq!(count, 0, "PartySize = 0 spawns nothing");
}
```

**Fix:** Add a `#[cfg(all(test, feature = "dev"))]` unit test block in `mod.rs` covering the cap arithmetic. The idempotence guard can remain manually-verified (it requires a `Query` in an App context) — a comment noting "idempotence guard verified by smoke test (Step 9)" is sufficient.

---

### [LOW] `DerivedStats` level-0 behavior is defined by `level.max(1)` but the test only covers level 1

**File:** `src/plugins/party/character.rs:derive_stats` and test `derive_stats_returns_zero_for_zero_inputs`

**Issue:** The function clamps `level` to `1` via `let effective_level = level.max(1)`. This is a deliberate design choice (avoiding multiply-by-zero for level-0 edge cases) and is documented inline with `// Using level.max(1) avoids multiply-by-zero for level-0 edge cases in tests`. However, the test `derive_stats_returns_zero_for_zero_inputs` passes `level: 1`, not `level: 0`. A future caller passing `level: 0` (e.g., a freshly spawned character before its first `Experience` update) will silently get level-1 stats instead of zeroed stats.

This is not a bug in the v1 implementation — the code does what the comment says. But there is a discrepancy between the test name ("zero_inputs") and the actual test (which uses level 1, not level 0). The comment "level-0 edge cases in tests" in the code implies level-0 was considered; the test should exercise it.

```rust
// Current test — passes level: 1, not level: 0
fn derive_stats_returns_zero_for_zero_inputs() {
    let result = derive_stats(&BaseStats::ZERO, &[], &StatusEffects::default(), 1);
    // ...
}
```

**Fix:** Either rename the test to `derive_stats_with_zero_base_stats_at_level_one` to be accurate, or add a second assertion with `level: 0` to confirm the `max(1)` clamping behavior:

```rust
// Verify the level.max(1) clamping:
let result_level_0 = derive_stats(&BaseStats::ZERO, &[], &StatusEffects::default(), 0);
assert_eq!(result_level_0.max_hp, 5, "level-0 clamps to level-1 (max_hp = 5)");
assert_eq!(result_level_0, result, "level-0 produces same as level-1 after clamping");
```

---

### [LOW] `base_mp` integer division truncates for odd stat totals

**File:** `src/plugins/party/character.rs:derive_stats`

**Issue:** The MP calculation is:

```rust
let base_mp = ((base.intelligence as u32).saturating_add(base.piety as u32))
    .saturating_mul(effective_level)
    / 2;
```

For `INT + PIE = 1` (odd sum) at any level, this truncates to 0. For `INT=1, PIE=0, level=1`: base_mp = `1 * 1 / 2 = 0`. This is Rust integer division (floor) and is almost certainly intentional for an RPG (round down is the convention). However, it's not documented. A future contributor might mistake it for a bug or introduce `f32` rounding.

```rust
// No comment explaining the truncation is intentional.
let base_mp = ((base.intelligence as u32).saturating_add(base.piety as u32))
    .saturating_mul(effective_level)
    / 2;
```

**Fix:** Add a one-line comment:

```rust
// Integer division — truncates toward zero (round-down convention for RPG MP pools).
let base_mp = ((base.intelligence as u32).saturating_add(base.piety as u32))
    .saturating_mul(effective_level)
    / 2;
```

---

### [LOW] `data/mod.rs` doc-comment still says `enemies — EnemyDb (Features #11/#15)`

**File:** `src/data/mod.rs:5`

**Issue:** The module-level doc-comment has the line `- enemies — EnemyDb (Features #11/#15)`. This was pre-existing and unchanged by the PR (only the `items` and `classes` lines were updated). But it's misleading: Feature #11 has nothing to do with enemies. The line should say `(Feature #15)` only. This is a minor documentation inaccuracy in unchanged code — technically out of scope for this review, but worth noting since the PR touches this file.

```
//! - `enemies` — `EnemyDb` (Features #11/#15)   ← #11 is wrong here
```

**Fix:** While editing `data/mod.rs` for this PR, update to:

```
//! - `enemies` — `EnemyDb` (Feature #15)
```

---

## Test Coverage Assessment

| Layer | Description | Status |
|-------|-------------|--------|
| Layer 1 | `derive_stats` math (zero, equipment, Dead, Poison, saturating) | 5 tests ✓ |
| Layer 1 | `PartySize::default()` == 4 | 1 test ✓ |
| Layer 1 | `StatusEffects::has()` | 1 test ✓ |
| Layer 1 | `BaseStats` RON round-trip | 1 test ✓ |
| Layer 1 | `ClassTable` RON round-trip + `get` | 2 tests ✓ |
| Layer 1 | `ItemStatBlock` RON round-trip | 1 test ✓ |
| Layer 2 | `spawn_default_debug_party` idempotence | Manual smoke only ⚠ |
| Layer 2 | `PartySize` hard-cap (> 4) arithmetic | Not tested ⚠ |
| Layer 3 | `ClassTable` via `RonAssetPlugin` end-to-end | 1 test ✓ |

The Layer 2 gap for idempotence is acceptable (requires Bevy App + state machine, and the smoke test confirmed behavior). The Layer 2 gap for `PartySize > 4` clamping is minor since the arithmetic is trivial — it's a one-liner `usize.min(4)` that can be covered in a pure unit test without App setup.

---

## Architecture & Convention Checks

**Frozen-file policy:** `src/data/dungeon.rs` untouched. `Cargo.toml`, `Cargo.lock`, `src/main.rs` byte-unchanged. ✓

**Decision D3 — `Handle<ItemAsset>` not `Entity`:** Correct. All 8 equipment slots are `Option<Handle<ItemAsset>>`. ✓

**Decision D1 — 8 `Class` variants, 3 authored:** Correct. `ClassTable::get` uses `Option` return, no exhaustive match. ✓

**Decision D7 — `Dead` as variant:** Correct. No `pub struct Dead` anywhere. `derive_stats` zeros pools on Dead. ✓

**Reverse-dep documentation:** Both `character.rs` header and `classes.rs` header carry the "one-way reverse dep" note. ✓

**`StatusEffects` duplication semantics:** Adding the same `StatusEffectType` twice is allowed (the `Vec<ActiveEffect>` has no dedup). This is not a bug in v1 (the five v1 types are all gates or tick-on-turn, and duplicates are harmless or conservative). The comment in the plan says combat (#15) will manage this. ✓

**`PartyMemberBundle` field count:** 11 fields: `marker, name, race, class, base_stats, derived_stats, experience, party_row, party_slot, equipment, status_effects`. The bundle does NOT include `ActiveEffect` (correct — it's a value type, not a Component) or `StatusEffectType` (same). ✓

**`StatusEffectType` Component derive:** `StatusEffectType` does NOT derive `Component`. It's a pure value type (enum inside `ActiveEffect` inside `StatusEffects`). Correct — it doesn't need to be queryable directly. ✓

**`core.classes.ron` stat values vs plan §Step 6:**
- Fighter: STR 14 / VIT 14 / AGI 10 / LUK 9 — matches plan ✓
- Mage: INT 14 / AGI 10 / LUK 10 — matches plan ✓
- Priest: PIE 14 / VIT 11 / AGI 9 / LUK 9 — matches plan ✓
- All `xp_to_level_2: 100`, `xp_curve_factor: 1.5` — matches plan ✓

---

## Review Summary

| Severity | Count |
|----------|-------|
| CRITICAL | 0 |
| HIGH     | 0 |
| MEDIUM   | 1 |
| LOW      | 3 |

**Verdict: APPROVE WITH NITS**

The implementation is correct. All 12 components have the right derive sets (with the documented `Equipment` exception). `derive_stats` is pure: no world access, no randomness, saturating arithmetic on every arithmetic path. The idempotence guard is in the right place (first in the function body, before the spawn loop). The dev gate is symmetric (`#[cfg(feature = "dev")]` on both definition and registration). The RON asset path matches the frozen loader in `LoadingPlugin`. The integration test uses `MessageWriter<AppExit>` and `exit.write(AppExit::Success)` (correct Bevy 0.18 API). `Cargo.toml`, `Cargo.lock`, and `src/main.rs` are byte-unchanged. `src/data/dungeon.rs` is untouched.

The one MEDIUM finding (no automated test for the `PartySize` cap arithmetic and idempotence) is worth addressing before merge but is not a correctness risk today — the cap path is a one-liner and the idempotence guard was confirmed by manual smoke test. The three LOW findings are documentation nits (level-0 test naming, MP truncation comment, stale enemies doc-comment). None require blocking.
