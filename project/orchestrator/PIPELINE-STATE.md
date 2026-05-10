# Pipeline State

**Task:** Drive the full feature pipeline (research → plan → implement → review → ship) for **Feature #16: Encounter System & Random Battles** in the Druum Bevy 0.18.1 first-person dungeon-crawler RPG. Roadmap §16 at `project/roadmaps/20260429-01-bevy-dungeon-crawler-roadmap.md` lines 862-911. Per-cell `encounter_rate` + per-step accumulator (soft pity) + per-floor encounter table; on encounter, populate `CurrentEncounter` and transition `Dungeon → Combat`; on combat-end restore exact pre-combat dungeon state. `start_combat(enemy_group)` is the single entry point — both random encounters (#16) and FOEs (#22) call it. Pre-emptive `FoeProximity` resource hook for #22. New asset format: `assets/encounters/*.encounters.ron`. Feature #15 just shipped on `main` (`5284f79`) and is the hand-off point.

**Status:** BLOCKED — implementer agent harness lacks Bash tool; verification + commits cannot run from any orchestrator-spawned subagent
**Last Completed Step:** 2 (plan); Step 3 code-complete-but-uncommitted

**Recovery attempts (2026-05-08):**
1. Prior implementer (separate session) wrote all code to disk, falsely claimed "no terminal access" and skipped verify+commit. Discovered after the fact.
2. Orchestrator recovery attempt #1: re-spawned implementer with explicit "you have Bash" briefing. Implementer responded by writing a shell script for the user to run, citing "no shell access in this agent context."
3. Orchestrator recovery attempt #2: re-spawned implementer with directive to STOP and report tool list if Bash absent. Implementer reported its tool inventory contains exactly `Read`, `Write`, `Edit` — no Bash, no Shell, no Execute. Three independent agent sessions with the same outcome.

**Diagnosis:** The `run-implementer` skill in this Claude Code configuration spawns an agent whose tool harness does NOT include Bash. The orchestrator's tool list also does NOT include Bash. Therefore, no agent in the current pipeline can run `cargo check`, `cargo test`, `cargo clippy`, or any `but` / `git` mutation. The user's briefing ("you HAVE Bash access") appears to be incorrect for this harness configuration, OR a tool grant was assumed but never actually exposed to the spawned agents.

**Working tree state:** Feature #16 implementer diff is on disk uncommitted in `unassigned`. Files (per prior implementer summary, not re-verified by reading the diff):
- New: `src/data/encounters.rs`, `src/plugins/combat/encounter.rs`, `assets/encounters/floor_01.encounters.ron`
- Edited (carve-outs per plan): `src/data/mod.rs`, `src/plugins/dungeon/features.rs`, `src/plugins/loading/mod.rs`, `src/plugins/combat/mod.rs`, `src/plugins/combat/turn_manager.rs`
- Edited (cascade fixes outside plan's frozen list, per implementer D-I1/I2/I3): `src/plugins/dungeon/tests.rs`, `src/plugins/combat/ui_combat.rs`, `tests/dungeon_movement.rs`, `tests/dungeon_geometry.rs`

**What CANNOT be completed without external Bash execution:**
- Cargo verification gate (6 commands)
- Any cargo failure fixes (need re-verification loop)
- `but rub` staging + `but commit` for the 5-6 atomic commits
- `but push` + `gh pr create` (shipper)
- Code review on the PR (no PR yet)

## Artifacts

| Step | Description | Artifact                                 |
| ---- | ----------- | ---------------------------------------- |
| 1    | Research    | `project/research/20260508-180000-feature-16-encounter-system.md` |
| 2    | Plan        | `project/plans/20260508-200000-feature-16-encounter-system-and-random-battles.md` |
| 3    | Implement   | `project/implemented/20260508-220000-feature-16-encounter-system-and-random-battles.md` (code on disk; verification + commits BLOCKED on Bash availability) |
| 4    | Ship        | BLOCKED                                  |
| 5    | Code Review | BLOCKED                                  |

## User Decisions

Plan APPROVED by user (resumed pipeline). All 7 open questions resolved by planner from research recommendations (D-X1 through D-X7) — no Category-C decisions surfaced.
