# Pipeline State

**Task:** Drive the full pipeline (research → plan) for Feature #4: Dungeon Grid Data Model from the dungeon crawler roadmap. Implement razor-wall grid types (`WallType`, `WallMask`, `CellFeatures`, `TrapType`, `TeleportTarget`, `Direction`, `DungeonFloor`) extending the existing stub in `src/data/dungeon.rs`. Add `DungeonFloor::can_move()`, `wall_between()`, `validate_wall_consistency()`, and `Direction` rotation/offset helpers. Replace placeholder `assets/dungeons/floor_01.dungeon.ron` with hand-authored 6×6 test floor exercising every CellFeature variant. Add app-level integration test verifying `bevy_common_assets`'s `ron 0.11` loader can load non-empty `DungeonFloor` (deferred concern from Feature #3 review). Bevy =0.18.1, no new deps. PAUSE at plan-approval; parent dispatches implementer manually because `SendMessage` does not actually resume returned agents.
**Status:** in-progress
**Last Completed Step:** 3

## Artifacts

| Step | Description | Artifact                                 |
| ---- | ----------- | ---------------------------------------- |
| 1    | Research    | /Users/nousunio/Repos/Learnings/claude-code/druum/project/research/20260501-220000-feature-4-dungeon-grid-data-model.md |
| 2    | Plan        | /Users/nousunio/Repos/Learnings/claude-code/druum/project/plans/20260501-230000-feature-4-dungeon-grid-data-model.md |
| 3    | Implement   | /Users/nousunio/Repos/Learnings/claude-code/druum/project/implemented/20260501-233000-dungeon-grid-data-model-feature-4.md |
| 4    | Ship        | pending (parent dispatches manually)     |
| 5    | Code Review | pending (parent dispatches manually)     |

## Implementation Notes (Step 3)

User approved the plan with `turn_*` naming on 2026-05-01. Implementer ran the full plan with four minor deviations, all reasonable: `Direction` got a `Default` derive (required by `entry_point: (u32, u32, Direction)`'s `Default` derive); `#[serde(default)]` ordering corrected; `src/data/mod.rs` re-exports added proactively to match Feature #3 pattern; method bodies written in Step 1 instead of staged via `todo!()`.

Verification: all 6 commands passed with zero warnings. 28 tests default / 29 with `--features dev`. **Hard gate `git diff Cargo.toml Cargo.lock` is empty — byte-unchanged.**

**ron 0.11/0.12 compatibility confirmed** — the App-level integration test in `tests/dungeon_floor_loads.rs` loads the new 6×6 hand-authored `floor_01.dungeon.ron` end-to-end through `bevy_common_assets`'s `ron 0.11` loader and asserts exact field values. No format drift observed across all in-scope RON constructs (unit enum variants, struct variants with named fields, `#[serde(default)]` structs, nested Options). The deferred concern from the Feature #3 code review is materially closed.

LOC: `src/data/dungeon.rs` 453 lines, `tests/dungeon_floor_loads.rs` 71 lines, `floor_01.dungeon.ron` 114 lines. Total within plan's ~350 net LOC budget.

## Research Summary

Research is HIGH-confidence on all verifiable claims (RQ2 ron format compat, RQ7 Reflect derive shape, RQ4 can_move semantics, RQ5 y-axis convention) and MEDIUM-confidence on three design calls (RQ1 wall storage, RQ3 CellFeatures shape, RQ6 spinner metadata). Every API claim grounded in extracted on-disk Bevy 0.18.1 / ron 0.11 / ron 0.12 / bevy_common_assets 0.16.0 / bevy_asset_loader 0.26.0 source.

Top-level recommendations adopted by the planner unchanged:
- Store walls TWICE per cell (N/E/S/W) per §Pattern 2 + add `validate_wall_consistency()` test.
- Add App-based integration test in `tests/dungeon_floor_loads.rs` exercising `RonAssetPlugin` end-to-end (deferred from Feature #3 review).
- Use struct-of-optionals `CellFeatures` per §Pattern 2. Drop `Door` from user task's enum description (already a `WallType`). `spinner: bool` (no SpinnerStyle, no telegraphed flag — §Resolved #4 fixes the design).
- `can_move(x, y, dir)` returns false for out-of-bounds, `Solid`, `LockedDoor`, `SecretWall`. Returns true for `Open`, `Door`, `Illusory`, `OneWay`. Discovery state lives outside DungeonFloor.
- y-down (`North = (0, -1)`) per §Pattern 2. Documented in `Direction` doc comment.

## Plan Summary

8 commit-ordered steps (~250 LOC types + impls, ~150 LOC unit tests with 26 tests in-file, ~75 LOC integration test in `tests/dungeon_floor_loads.rs`, plus the hand-authored RON — total ~350 net new LOC). No `Cargo.toml` change. Final verification gate explicitly diffs `Cargo.toml` and `Cargo.lock` to ensure they are byte-unchanged.

Step ordering:
1. Replace type definitions in `src/data/dungeon.rs` (Direction, WallType, WallMask, TeleportTarget, TrapType, CellFeatures, DungeonFloor + WallInconsistency error type)
2. Update existing `#[cfg(test)] mod tests` for the new shape
3. Implement `validate_wall_consistency` and add its tests
4. Implement `can_move` and `wall_between`, add their tests
5. Add `Direction` rotation/offset tests
6. Replace `assets/dungeons/floor_01.dungeon.ron` with a hand-authored 6×6 test floor (entry at (1,1,N); placements specified for Door, LockedDoor, Illusory, OneWay, SecretWall, plus spinner, trap=Pit, teleporter, dark_zone, anti_magic_zone)
7. Add the App-level integration test in `tests/dungeon_floor_loads.rs`
8. Final verification — run all 6 commands, diff Cargo.toml/Cargo.lock

Key planner decisions (called out in plan §Approach and §Critical):
- Adopted research's struct-of-optionals `CellFeatures` (drops `Door` enum variant).
- Adopted research's `turn_left/turn_right/reverse/offset` naming over user-task's `rotate_*` — matches Wizardry vocabulary and §Pattern 2.
- Added `PartialEq` to every type so the integration test can assert exact field shapes without hand-comparing.
- `can_move` returns true on `Open | Door | Illusory | OneWay` (extending §Pattern 2's `Open | Illusory`); the matrix is doc-commented.
- `Cargo.toml` and `Cargo.lock` MUST be byte-unchanged — final verification gate explicitly diffs them.
- All 8 research open questions resolved with researcher's recommended defaults; no new Category C questions surfaced — proceeded straight to writing.

## User Decisions

None required so far. Researcher's open questions all resolved by the planner without escalation. **Plan-approval checkpoint is now active** — the parent session will surface this plan to the user for approval before dispatching the implementer manually.

Pipeline pauses here per task instructions. Resume from Step 3 (Implement) once the user approves the plan in the parent session.
