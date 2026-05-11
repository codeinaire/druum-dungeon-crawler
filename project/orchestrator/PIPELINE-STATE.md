# Pipeline State

**Task:** Drive the full feature pipeline (research → plan → implement → review → ship) for **Feature #16: Encounter System & Random Battles** in the Druum Bevy 0.18.1 first-person dungeon-crawler RPG. Roadmap §16 at `project/roadmaps/20260429-01-bevy-dungeon-crawler-roadmap.md` lines 862-911. Per-cell `encounter_rate` + per-step accumulator (soft pity) + per-floor encounter table; on encounter, populate `CurrentEncounter` and transition `Dungeon → Combat`; on combat-end restore exact pre-combat dungeon state. `start_combat(enemy_group)` is the single entry point — both random encounters (#16) and FOEs (#22) call it. Pre-emptive `FoeProximity` resource hook for #22. New asset format: `assets/encounters/*.encounters.ron`. Feature #15 just shipped on `main` (`5284f79`) and is the hand-off point.

**Status:** Pipeline COMPLETE. PR open at https://github.com/codeinaire/druum-dungeon-crawler/pull/16. Reviewer verdict **WARNING** (0 CRITICAL / 0 HIGH / 3 MEDIUM / 2 LOW) — no pipeline-blocking issues. Awaiting user direction on whether to address MEDIUM findings before merge.
**Last Completed Step:** 5 (review)

**Recovery history (2026-05-08):**
1. Prior implementer wrote all code to disk, falsely claimed "no terminal access" and skipped verify+commit.
2. Two orchestrator recovery attempts failed — agents spawned via the `run-implementer` skill (`context: fork`) reported no Bash tool.

**Recovery success (2026-05-09 / 2026-05-10):**
- Bypassed the skill-routed path. Direct `subagent_type: implementer` invocation preserved Bash access.
- Recovery implementer fixed 9 compile/test bugs and committed: EnemyAi serde derives, `rand::Rng` import, `minimap.rs` `DungeonAssets` cascade fix, three test-app resource gaps in `turn_manager.rs` / `encounter.rs`, two integration-test fixes, dead code removal.
- Gate results: `cargo check` clean (default + dev), `cargo test` 205 passed, `cargo test --features dev` 209 passed, `cargo clippy --all-targets -- -D warnings` clean (default + dev).
- Direct `subagent_type: shipper` and `subagent_type: code-reviewer` invocations also worked. The skill-routed (`run-shipper` / `run-reviewer`) paths were not retried.

**Commit:** `19e87a3` on `feature/16-encounter-system` — all feature code in one commit (D-I8 deviation from the planned 5-commit cadence; see implementation summary). `fac6d39` adds the docs trailer.

## Artifacts

| Step | Description | Artifact                                 |
| ---- | ----------- | ---------------------------------------- |
| 1    | Research    | `project/research/20260508-180000-feature-16-encounter-system.md` |
| 2    | Plan        | `project/plans/20260508-200000-feature-16-encounter-system-and-random-battles.md` |
| 3    | Implement   | `project/implemented/20260508-220000-feature-16-encounter-system-and-random-battles.md` — complete (SHA `19e87a3` on `feature/16-encounter-system`) |
| 4    | Ship        | PR #16: https://github.com/codeinaire/druum-dungeon-crawler/pull/16 — body at `project/shipper/feature-16-pr-body.md` |
| 5    | Code Review | `project/reviews/20260510-103658-feature-16-encounter-system-pr-review.md` — verdict WARNING (3 MEDIUM, 2 LOW) |

## User Decisions

Plan APPROVED by user (resumed pipeline). All 7 open questions resolved by planner from research recommendations (D-X1 through D-X7) — no Category-C decisions surfaced.

User chose **Option A** at the verify-commit recovery checkpoint: re-launch the implementer with explicit shell-access instructions. Direct invocation succeeded; the skill-routed path was bypassed.
