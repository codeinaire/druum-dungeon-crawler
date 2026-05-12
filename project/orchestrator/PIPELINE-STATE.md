# Pipeline State

**Task:** Feature #18b — Town Hub & Services: Temple + Guild (follow-up to #18a). Bevy 0.18.1 first-person dungeon-crawler RPG. Implement the two deferred Town sub-state screens: `TownLocation::Temple` (revive dead, cure status, charge gold proportional to level) and `TownLocation::Guild` (party roster view, recruit from `core.recruit_pool.ron`'s 5 pre-authored entries, dismiss to dismissed-pool, reorder slots, front/back row swap). Replaces `src/plugins/town/placeholder.rs` with real `temple.rs` + `guild.rs` painters/handlers. Pure egui only (no 3D backdrop). Zero new Cargo deps. Branch: `feature/18b-town-temple-guild`. 6 quality gates: `cargo check` ±dev, `cargo test` ±dev, `cargo clippy --all-targets ±dev -- -D warnings`. Roadmap source: `project/roadmaps/20260429-01-bevy-dungeon-crawler-roadmap.md` §18.

**Status:** in-progress
**Last Completed Step:** 3 (all 6 quality gates green: cargo check ±dev, 292+6 / 296+6 tests pass, clippy ±dev clean. User fixed 8 bugs during gate verification — see implementation summary update.)

## Artifacts

| Step | Description | Artifact                                 |
| ---- | ----------- | ---------------------------------------- |
| 1    | Research    | `project/research/20260512-feature-18b-town-temple-guild.md` |
| 2    | Plan        | `project/plans/20260512-173000-feature-18b-town-temple-guild.md` |
| 3    | Implement   | `project/implemented/20260512-173000-feature-18b-town-temple-guild.md` (gates green ✅) |
| 4    | Ship        | pending                                  |
| 5    | Code Review | pending                                  |

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
