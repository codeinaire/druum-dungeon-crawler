# Code Review — Feature #16: Encounter System & Random Battles (PR #16)

**Date:** 2026-05-10
**Verdict:** WARNING (non-blocking; no CRITICAL or HIGH issues)
**Reviewed:** PR #16 `feature/16-encounter-system` (HEAD `fac6d39`)
**Context:** First review pass. No prior review comments.

---

## Behavioral delta

This PR bridges dungeon exploration to turn-based combat by adding:

1. `check_random_encounter` — reads `MovedEvent`, applies a soft-pity-scaled probability roll per step, emits `EncounterRequested { source: Random }` on hit
2. `handle_encounter_request` — sole consumer; picks an `EnemyGroup` via `WeightedIndex`, spawns `EnemyBundle`s, inserts `CurrentEncounter`, transitions `GameState::Dungeon → Combat`
3. `EncounterTable` RON asset schema + `floor_01.encounters.ron` (4 enemy groups)
4. Deletion of the `spawn_dev_encounter` dev stub from `turn_manager.rs` (Pitfall 8)
5. `FoeProximity` stub resource, `snap_movement_animation_on_combat_entry`, F7 force-encounter (dev-only)
6. 8 cascade fixes to test apps and struct literal sites necessitated by `DungeonAssets` struct change

---

## Load-bearing invariant checks

| Gate | Command / grep | Result |
|---|---|---|
| Sole-writer: `insert_resource(CurrentEncounter` outside `encounter.rs` and tests | `grep -rn "insert_resource(CurrentEncounter" src/` | PASS — line 376 (production) + line 574 (test fixture) only |
| `spawn_dev_encounter` deleted | `grep -rn "spawn_dev_encounter" src/` | PASS — only in doc comments, no function definition or system registration |
| `encounter_rate.clamp(0.0, 1.0)` present | `encounter.rs:268` | PASS |
| `weight.clamp(1, 10_000)` present | `encounters.rs:84` | PASS |
| `MAX_ENEMIES_PER_ENCOUNTER = 8` present and applied | `encounter.rs:70,343-350` | PASS |
| Rate-zero guard is BEFORE multiplier application | `encounter.rs:272` before `encounter.rs:277` | PASS |
| F7 gated `#[cfg(feature = "dev")]` | `encounter.rs:404,163-167` | PASS |
| F7 uses direct `ButtonInput<KeyCode>`, not leafwing | `encounter.rs:405-410` | PASS |
| System ordering: `check_random_encounter.after(handle_dungeon_input)` | `encounter.rs:156` | PASS |
| System ordering: `handle_encounter_request.after(check_random_encounter)` | `encounter.rs:158-159` | PASS |
| `CurrentEncounter` removed on `OnExit(Combat)` | `encounter.rs:150,200-202` | PASS |
| Soft-pity reset on `OnEnter(Dungeon)` | `encounter.rs:145,175-177` | PASS |
| `EnemyAi` serde derives — no Handle/non-serializable fields | `ai.rs:46-53` (enum with only `u32` field) | PASS |
| `cargo check` | clean | PASS |
| `cargo test` | 205 passed, 0 failed | PASS |
| `cargo clippy --all-targets -- -D warnings` | 0 warnings | PASS |

---

## Cascade fix assessment

All 8 cascade files examined. Each addition is a minimum-viable compilability fix:

| File | Change | Verdict |
|---|---|---|
| `dungeon/features.rs` | `encounters_floor_01: Handle::default()` + `init_asset::<EncounterTable>()` | Minimum-viable |
| `dungeon/tests.rs` | Same DungeonAssets fixture extension | Minimum-viable |
| `ui/minimap.rs` | `encounters_floor_01: Handle::default()` | Minimum-viable |
| `combat/turn_manager.rs` | `init_asset::<EncounterTable>()` + `add_message::<EncounterRequested>()` + 3 more fixture entries | Minimum-viable (recovery D-I9) |
| `combat/ui_combat.rs` | Same pattern | Minimum-viable |
| `tests/dungeon_geometry.rs` | `init_asset::<EncounterTable>()` + DungeonAssets field | Minimum-viable |
| `tests/dungeon_movement.rs` | Same | Minimum-viable |
| `combat/ai.rs` | `Serialize, Deserialize` derives on `EnemyAi` | Required for `EnemySpec` RON serialization |

No behavior changes observed in cascade files beyond what is necessary for compilation.

---

## Findings

---

### [MEDIUM] `rate_zero_cell_no_encounter_rolls` and `foe_proximity_suppresses_rolls` tests exercise the wrong code path

**File:** `src/plugins/combat/encounter.rs:647-702`

**Issue:** Both tests create a test floor via `build_test_floor(...)` and add it to `Assets<DungeonFloor>`, but neither test inserts a `DungeonAssets` resource. Since `check_random_encounter` uses `dungeon_assets: Option<Res<DungeonAssets>>`, the `maybe_floor` binding is always `None`. Every `MovedEvent` bumps the step counter and hits the early `continue` at line 249 — the rate-zero guard (line 272) and the FOE-suppression check (line 253) are never reached.

The `rate_zero_cell_no_encounter_rolls` test title claims to verify "the 2.0× cap doesn't cause bogus rolls on rate-zero corridor cells," but it actually verifies the no-assets bail path. It would pass even if `cell_rate <= 0.0` guard were deleted entirely. The `foe_proximity_suppresses_rolls` test has the same issue with a rate=1.0 floor that would guarantee encounters if actually reached.

The PR body (Reviewer guide) and the test plan (`cargo test rate_zero_cell_no_encounter_rolls`) cite these tests as regression guards for the soft-pity cap invariant — that claim is incorrect.

The production code is correct. The tests pass. But the tests do not guard what they claim to guard.

**Fix:** Wire `DungeonAssets` in the test setup so `maybe_floor` resolves to the test floor. The `dungeon_movement.rs` integration tests show the pattern — `commands.insert_resource(DungeonAssets { floor_01: <handle>, encounters_floor_01: Handle::default(), ... })`. For the encounter tests the `floor_01` handle should be the one returned by `build_test_floor`. Alternatively rename the test to reflect what it actually tests (`no_encounter_when_dungeon_assets_absent`), and add a separate test that wires `DungeonAssets` with a rate-zero floor.

```rust
// In rate_zero_cell_no_encounter_rolls, after build_test_floor:
let floor_handle = build_test_floor(&mut app, 10, 0.0);
app.world_mut().insert_resource(DungeonAssets {
    floor_01: floor_handle,
    encounters_floor_01: Handle::default(),
    // ... other fields with Handle::default()
});
```

---

### [MEDIUM] PR body smoke test lists wrong enemy names

**File:** `project/shipper/feature-16-pr-body.md:137`

**Issue:** The manual smoke test instructions say "enemies are Slimes, Goblins, Kobolds, or Bat Swarms" but `assets/encounters/floor_01.encounters.ron` defines Single Goblin (50%), Pair of Goblins (30%), Goblin Captain (15%), and Cave Spider (5%). No slimes, kobolds, or bat swarms exist in the asset. This will confuse anyone following the manual smoke test checklist.

**Fix:** Update the smoke test line to:

```
- [ ] **Spawned enemy group matches floor_01 table** — enemies are a Single Goblin,
      a Pair of Goblins, a Goblin Captain, or a Cave Spider; no stray dev-stub
      "Goblin 1"/"Goblin 2" pair from the deleted `spawn_dev_encounter`
```

---

### [MEDIUM] Four tests listed in the PR test plan do not exist

**File:** `project/shipper/feature-16-pr-body.md:117-120`

**Issue:** The PR test plan checks off four specific `cargo test` invocations:

- `cargo test handle_encounter_request_sole_writer`
- `cargo test no_current_encounter_after_combat_exit`
- `cargo test encounter_rate_clamp`
- `cargo test max_enemies_per_encounter_truncation`

None of these test functions exist in the codebase. The actual test names are:

- `current_encounter_removed_on_combat_exit` (closest to `no_current_encounter_after_combat_exit`)
- `rate_zero_cell_no_encounter_rolls` (covers accumulator behavior but see MEDIUM above)
- `encounter_request_bails_safely_without_dungeon_assets`

The properties these test names describe (sole-writer verification, encounter_rate clamp, max-enemies truncation) are not exercised by any automated test. The sole-writer guarantee is architectural (the grep gate), and clamp/truncation paths have no unit tests.

**Fix:** Either add tests that exercise these paths, or correct the test plan to list the actual test names. Adding the truncation test is the highest-value addition — `max_enemies_per_encounter_truncation` can be a pure unit test that constructs an `EnemyGroup` with 9 enemies and calls the spawn logic via a direct message write.

---

### [LOW] `probability` can exceed 1.0 when `cell_rate × multiplier > 1.0`

**File:** `src/plugins/combat/encounter.rs:279`

**Issue:** `probability = cell_rate * multiplier` is not clamped to `[0.0, 1.0]`. With `cell_rate = 0.6` (a valid clamped value) and multiplier at cap 2.0, `probability = 1.2`. Since `rng.0.random::<f32>()` returns `[0.0, 1.0)`, the check `< 1.2` always succeeds — effectively a 100% encounter rate. At `cell_rate = 0.5` and max multiplier this is certain to trigger.

This is likely intentional design (the PR body says "a hit is statistically near-certain by step 40") and is not a bug given the current floor_01 data (`encounter_rate` not specified in the RON, so it uses `CellFeatures::default()` which would be 0.0, or whatever `floor_01.dungeon.ron` specifies). But it means the 2.0× cap description is slightly misleading — it caps the *multiplier*, not the *probability*, so for any `cell_rate >= 0.5` the encounter is deterministic once the cap is reached.

**Fix:** Document the ceiling behavior in the `check_random_encounter` doc comment, or add `let probability = probability.min(1.0)` for explicitness. Not a bug, but an undocumented invariant.

---

### [LOW] D-I8 commit collapse — single-commit history

**Note:** The plan's 5-commit cadence collapsed to 1 code commit + 1 docs commit due to GitButler's inability to split files at hunk level when all branches share a workspace. The PR body discloses this. For code review purposes, the single commit `19e87a3` is workable — the diff is self-contained, the feature is atomic, and the tests pass. A re-base into the planned 5-commit cadence is not warranted for this PR. This is noted for process improvement only.

---

## Recovery story sanity check

The 9 bugs fixed in the recovery pass (serde derives, rand import, minimap cascade, 5 test-app fixes) are all absent from the final tree. No new stray issues observed beyond those already surfaced above. The recovery was thorough on compilation and clippy; the missing-test-coverage gaps are the only remaining latent issue.

---

## Review Summary

| Severity | Count |
|----------|-------|
| CRITICAL | 0     |
| HIGH     | 0     |
| MEDIUM   | 3     |
| LOW      | 2     |

**Verdict: WARNING**

The core implementation is correct and production-ready. All load-bearing invariants hold: sole-writer on `CurrentEncounter` verified, `spawn_dev_encounter` fully deleted, trust-boundary clamps applied at the load boundary, F7 dev command is properly gated, scheduling order is correct, and soft-pity reset fires on `OnEnter(Dungeon)`. The 205-test suite passes with 0 failures.

The MEDIUMs are:
1. Two app-level tests that claim to guard the rate-zero and FOE-suppression paths but actually exercise the no-assets bail path — the invariants they name in the PR test plan are untested by automation.
2. The PR body smoke test lists wrong enemy names, which would confuse manual testing.
3. Four PR-body test names that don't exist as test functions.

None of these block the implementation from being correct or safe. The orchestrator should not pause the pipeline; these are addressable in a follow-on commit or PR before #17.

**Files reviewed (full coverage):**
- `src/plugins/combat/encounter.rs` (full — 757 lines)
- `src/data/encounters.rs` (full — 231 lines)
- `src/plugins/combat/ai.rs` (full — 444 lines, serde-only change)
- `src/plugins/combat/turn_manager.rs` (cascade region, spawn_dev_encounter deletion confirmed)
- `src/plugins/combat/ui_combat.rs` (cascade region)
- `src/plugins/loading/mod.rs` (encounter_table_for addition, lines 225-240)
- `src/plugins/dungeon/features.rs` (cascade region — EncounterSource::Random and test fixture)
- `src/plugins/ui/minimap.rs` (cascade line)
- `tests/dungeon_geometry.rs` (cascade region)
- `tests/dungeon_movement.rs` (cascade region)
- `assets/encounters/floor_01.encounters.ron` (full)
