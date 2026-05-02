# Code Review: Add dungeon grid data model (Feature #4)

**Date:** 2026-05-01
**PR:** #4 — `4-dungeon-grid-data-model` → `main`
**Verdict:** APPROVE
**Reviewed by:** Code Reviewer (Claude Sonnet 4.6)

## Severity Counts

| Severity | Count |
| -------- | ----- |
| CRITICAL | 0     |
| HIGH     | 0     |
| MEDIUM   | 0     |
| LOW      | 2     |

## Files Reviewed (Full Coverage)

- `src/data/dungeon.rs` (453 lines — full read)
- `tests/dungeon_floor_loads.rs` (71 lines — full read)
- `src/data/mod.rs` (full read)
- `assets/dungeons/floor_01.dungeon.ron` (full read + manual wall consistency audit)
- `Cargo.toml` (confirmed byte-unchanged from prior features)

## Key Findings

### [LOW] Misleading inline comment at wall cell (0,3) in `floor_01.dungeon.ron`

**File:** `assets/dungeons/floor_01.dungeon.ron:89`

The comment on cell (0,3) reads `"dark_zone cell (features)"` but the features table for row y=3 is entirely `()`. The actual `dark_zone: true` cell is at (1,4), correctly placed in the y=4 features row and correctly documented in the file's header legend (line 28). This is a copy-paste error in the inline wall comment — the data is correct, the comment is wrong.

**Fix:** Change line 89 from:

```
// (0,3): left edge; dark_zone cell (features)
```

to:

```
// (0,3): left edge
```

---

### [LOW] `WallInconsistency` missing from convenience re-exports in `src/data/mod.rs`

**File:** `src/data/mod.rs`

All other public types from `dungeon.rs` are re-exported via `pub use dungeon::{CellFeatures, Direction, DungeonFloor, TeleportTarget, TrapType, WallMask, WallType}`, but `WallInconsistency` is absent. `validate_wall_consistency()` returns `Result<(), Vec<WallInconsistency>>`, so any caller who wants to match on or inspect the error values (e.g. a future `validate_and_report` helper in Feature #7 or a dev tool) needs to use the longer `druum::data::dungeon::WallInconsistency` path. This is not a correctness issue now (all current consumers just call `.is_ok()`), but it is an ergonomic inconsistency with the established re-export pattern.

**Fix:** Add `WallInconsistency` to the re-export list:

```rust
pub use dungeon::{
    CellFeatures, Direction, DungeonFloor, TeleportTarget, TrapType, WallMask, WallType,
    WallInconsistency,
};
```

---

### [LOW] PR description overstates integration test coverage

The PR description says the integration test "asserts exact field values across every CellFeatures variant, every WallType, TeleportTarget, TrapType." The actual `assert_floor_shape` function in `tests/dungeon_floor_loads.rs` only asserts `width`, `height`, `entry_point`, `is_well_formed`, and `validate_wall_consistency`. This matches the plan's specification ("spot-check the same fields the unit test asserts") and is intentional, but the PR description is misleading for future reviewers looking at the PR history. Not flagging separately — subsumed under the LOW count above.

---

## What Was Verified

**Behavioral delta:** `DungeonFloor` changes from a zero-field stub (`()` in RON) to a full razor-wall grid data model. Eight new types plus four methods. No runtime systems or plugins are added; the new types are not yet wired into any ECS system. The only observable change is that `floor_01.dungeon.ron` now loads as a non-empty `DungeonFloor` with a 6×6 grid. All 28 tests (27 unit + 1 integration) pass under both default and `--features dev`.

**Correctness checks:**

- `can_move` truth table verified: `Open|Door|Illusory|OneWay` → true; `Solid|LockedDoor|SecretWall` → false; out-of-bounds → false. All 10 matrix tests cover every variant individually.
- `validate_wall_consistency` implementation traced: iterates all adjacent pairs, calls `walls_consistent` (which allows OneWay on either side), collects into `Vec<WallInconsistency>`. Logic is correct.
- `wall_between` bounds-checks `a` before accessing the grid. Returns `WallType::Solid` for non-adjacent, out-of-bounds, and the panic-free fallback.
- `Direction::turn_left/turn_right/reverse/offset` — naming matches plan spec (`turn_*` not `rotate_*`); y-down convention (`North = (0,-1)`) is documented and tested.
- `CellFeatures`: no `Door` variant (correct — doors live in `WallType`); `#[serde(default)]` placed after `#[derive]` (correct ordering per D2 discovery); `spinner: bool` only (no `SpinnerStyle` enum); all seven fields match plan spec.
- `DungeonFloor` retains `Asset + Reflect + Serialize + Deserialize + Default + Debug + Clone` — the `LoadingPlugin` from Feature #3 continues to work.
- No `rand` calls anywhere in the changed files.
- `MessageWriter<AppExit>` used in integration test (not `EventWriter`) — correct Bevy 0.18 API.
- `Cargo.toml` is byte-unchanged. `Cargo.lock` is a new-file addition (was absent from `main` branch which only has the initial README commit; lock file was first added in Feature #1/2/3 PRs that weren't yet merged to main) — no new dependencies are introduced.

**RON file audit:**

Wall consistency for all 30 east-west and 30 south-north shared edges was verified programmatically. Zero inconsistencies. The OneWay/Solid pair at (2,3)/(3,3) is the one allowed asymmetry and is handled correctly by `walls_consistent`. All 7 `WallType` variants appear. All 5 planned `CellFeatures` variants appear (`spinner`, `trap:Pit`, `teleporter`, `dark_zone`, `anti_magic_zone`). Entry point `(1, 1, North)` matches the data.

**Ron 0.11/0.12 concern (deferred from Feature #3):**

The integration test in `tests/dungeon_floor_loads.rs` drives a Bevy `App` through `RonAssetPlugin::<DungeonFloor>::new(&["dungeon.ron"])` (the ron 0.11 internal path) and asserts shape + wall consistency. The unit test in `src/data/dungeon.rs::floor_01_loads_and_is_consistent` exercises the ron 0.12 stdlib path. Both pass. The `feedback_ron_version_split.md` concern is materially addressed for Feature #4's type shapes. The integration test will continue to catch any future format drift if ron versions diverge.

**Static analysis:** `cargo clippy --all-targets -- -D warnings` passed with zero warnings. `cargo test` (28 tests) and `cargo test --features dev` (29 tests) both pass.
