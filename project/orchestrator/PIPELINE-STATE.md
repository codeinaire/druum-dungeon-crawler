# Pipeline State

**Task:** Drive research → plan pipeline (PAUSE at plan-approval) for Feature #10: Auto-Map / Minimap from the dungeon crawler roadmap. Add `ExploredCells` resource (HashMap of (floor, x, y) → ExploredState ∈ {Unseen, Visited, KnownByOther}), `MovedEvent` subscriber that flips cells to Visited, egui painter rendering grid + walls (per-WallType color) + explored shading + player arrow, full-screen view in `DungeonSubState::Map`, top-right minimap overlay during `DungeonSubState::Exploring`, `dark_zone: true` cells skip updates with "?" indicator, debug "show full map" toggle. NEW file `src/plugins/dungeon/minimap.rs`. First feature to add a non-trivial dep: `bevy_egui` — researcher must verify the latest 0.18.1-compatible release (NOT just trust roadmap "0.39") and confirm exactly one dep added with no surprise feature flags. `src/data/dungeon.rs` is frozen post-#9; minimap reads floor data through existing public API only. Pre-pipeline action item: local `main` is behind origin/main (PR #9 merged on GitHub) — implementer (or orchestrator before branching) must `git pull origin main`. `MovedEvent` derives `Message` (Bevy 0.18 family rename); subscribers use `MessageReader<MovedEvent>`. Final report at plan-approval MUST be self-contained because `SendMessage` does not actually resume returned agents (confirmed across Features #3-#9); parent dispatches implementer manually after approval.

**Status:** implementation complete — manual smoke + code review + PR remain
**Last Completed Step:** 3 (implementation done; branch 10-auto-map-minimap; commits a9c98aa, f9f2e7a, ebae6df)

## Artifacts

| Step | Description | Artifact                                 |
| ---- | ----------- | ---------------------------------------- |
| 1    | Research    | /Users/nousunio/Repos/Learnings/claude-code/druum/project/research/20260504-130000-feature-10-auto-map-minimap.md |
| 2    | Plan        | /Users/nousunio/Repos/Learnings/claude-code/druum/project/plans/20260504-150000-feature-10-auto-map-minimap.md (Status: Draft — awaiting D1–D7) |
| 6    | Pipeline summary | /Users/nousunio/Repos/Learnings/claude-code/druum/project/orchestrator/20260504-160000-feature-10-auto-map-minimap-research-plan.md |
| 3    | Implement   | /Users/nousunio/Repos/Learnings/claude-code/druum/project/implemented/20260504-150000-feature-10-auto-map-minimap.md |
| 4    | Ship        | NOT IN SCOPE                             |
| 5    | Code Review | NOT IN SCOPE                             |

## User Decisions

(none yet — Category B decisions will be surfaced after research lands)

## Pipeline Scope

This invocation runs research → plan → STOP. After plan approval, parent will manually dispatch implementer (per established Feature #3-#9 pattern). The orchestrator pipeline summary at the end of this run must be self-contained.
