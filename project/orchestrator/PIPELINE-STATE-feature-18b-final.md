# Pipeline State — Feature #18b (archived)

**Task:** Feature #18b — Town Hub & Services: Temple + Guild (follow-up to #18a). Bevy 0.18.1 first-person dungeon-crawler RPG. Implement the two deferred Town sub-state screens: `TownLocation::Temple` (revive dead, cure status, charge gold proportional to level) and `TownLocation::Guild` (party roster view, recruit from `core.recruit_pool.ron`'s 5 pre-authored entries, dismiss to dismissed-pool, reorder slots, front/back row swap). Replaces `src/plugins/town/placeholder.rs` with real `temple.rs` + `guild.rs` painters/handlers. Pure egui only (no 3D backdrop). Zero new Cargo deps. Branch: `feature/18b-town-temple-guild`. 6 quality gates: `cargo check` ±dev, `cargo test` ±dev, `cargo clippy --all-targets ±dev -- -D warnings`. Roadmap source: `project/roadmaps/20260429-01-bevy-dungeon-crawler-roadmap.md` §18.

**Status:** archived (Steps 1-3 completed in pipeline; Steps 4-5 completed manually outside the orchestrator — PR #18b shipped as commit `fc6ceef feat(town): add Temple revive/cure and Guild roster screens (#18b)`, and a code review summary exists at `project/reviews/20260513-000000-feature-18b-town-temple-guild-pr-review.md`).
**Last Completed Step:** 3 (orchestrator-driven); Steps 4-5 completed out-of-band.

## Artifacts

| Step | Description | Artifact                                 |
| ---- | ----------- | ---------------------------------------- |
| 1    | Research    | `project/research/20260512-feature-18b-town-temple-guild.md` |
| 2    | Plan        | `project/plans/20260512-173000-feature-18b-town-temple-guild.md` |
| 3    | Implement   | `project/implemented/20260512-173000-feature-18b-town-temple-guild.md` (gates green) |
| 4    | Ship        | merged as commit `fc6ceef feat(town): add Temple revive/cure and Guild roster screens (#18b)` |
| 5    | Code Review | `project/reviews/20260513-000000-feature-18b-town-temple-guild-pr-review.md` |

## User Decisions

All 8 user decisions pre-resolved before plan step (defaults across the board):

1. **Dismiss scope:** Ship now with `Resource<DismissedPool>` (Option A from research) — ~120 LOC, +4 tests
2. **Temple cure set:** `Dead` + `Stone` + `Paralysis` + `Sleep` (Inn keeps `Poison`)
3. **Revive cost formula:** `base=100 + per_level*level=50` saturating; L1=150g, L5=350g (tested)
4. **Cure cost values:** `Stone=250`, `Paralysis=100`, `Sleep=50` (flat per-status, NOT level-scaled)
5. **Slot reorder semantics:** SWAP (two-write op exchanging `PartySlot` values)
6. **Recruit while party empty:** ALLOW (forward-compat with #19); min-1-active check applies to Dismiss ONLY
7. **Multi-status Cure UX:** Auto-pick first eligible severe status, priority order Stone > Paralysis > Sleep
8. **`DismissedPool` save format:** Defer `MapEntities` to #23 (same contract as `Inventory`); doc-comment only

## Residual working-tree state at archive time

Two tiny uncommitted hunks remained in `src/plugins/town/guild.rs` and `src/plugins/town/temple.rs` (response to PR review). Per the user, these are coexisting with the start of Feature #19 and will be parked appropriately before shipping #19.
