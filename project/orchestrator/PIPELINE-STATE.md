# Pipeline State

**Task:** Drive the full feature pipeline (research → plan → implement → review → ship) for **Feature #15: Turn-Based Combat Core** in the Druum Bevy 0.18 dungeon-crawler RPG. Roadmap §15 at `project/roadmaps/20260429-01-bevy-dungeon-crawler-roadmap.md:789-862`. Implements action-queue combat loop (research Pattern 5): `CombatPhase::PlayerInput` → enemy AI fills queue → sort by `speed` → `CombatPhase::ExecuteActions` → `CombatPhase::TurnResult`. Damage as pure function. Stub `CastSpell` (deferred to #20). Populate `CurrentEncounter` via test fixtures (encounter triggers in #16). Largest single feature in roadmap (4/5 difficulty); plan as multiple sub-systems.

**Status:** complete — pipeline ran research → plan → implement → review → fix-pass → ship.
**Last Completed Step:** 6 (ship)

## Artifacts

| Step | Description | Artifact |
| ---- | ----------- | -------- |
| 1    | Research    | `project/research/20260508-093000-feature-15-turn-based-combat-core.md` |
| 2    | Plan        | `project/plans/20260508-100000-feature-15-turn-based-combat-core.md` (Status: Approved 2026-05-08; D-A3=A, D-Q1=A, D-Q2=A, D-Q3=A, D-Q4=A, D-Q5=A) |
| 3    | Implement   | `project/implemented/20260508-120000-feature-15-turn-based-combat-core.md` (D-I1–D-I22 implementation discoveries; verification GREEN; review fixes applied) |
| 4    | Review      | `project/reviews/20260508-140000-feature-15-turn-based-combat-core.md` (Verdict: LGTM-with-changes — 0 CRITICAL, 0 HIGH, 2 MEDIUM, 2 LOW; both MEDIUMs resolved in fix-pass; LOWs deferred) |
| 5    | Pipeline summary | `project/orchestrator/20260508-150000-feature-15-turn-based-combat-core.md` |
| 6    | Ship        | PR #15: https://github.com/codeinaire/druum-dungeon-crawler/pull/15 (branch `feature/15-turn-based-combat-core` at `dcf3d51`) |

## User Decisions (planner checkpoint)

All 6 USER-PICK decisions answered with the recommended Option A:

- **D-A3 = A** — Damage formula: Wizardry multiplicative `(A * (100 - D/2)) / 100`, variance 0.7-1.0, crit 1.5x.
- **D-Q1 = A** — Combat camera: overlay on dungeon camera; reuse existing `attach_egui_to_dungeon_camera`.
- **D-Q2 = A** — Action menu UX: persistent action panel at bottom (~60px permanent).
- **D-Q3 = A** — Combat log shape: bounded ring buffer, capacity 50, kept across combats.
- **D-Q4 = A** — `Defend` stacking with magical `DefenseUp`: take-higher (current #14 rule).
- **D-Q5 = A** — Boss AI scope: ship `BossAi` enum stub with `BossFocusWeakest` + `BossAttackDefendAttack` patterns.

## Verification gate (executed 2026-05-08, post-review fix pass — final)

| Check | Result |
| ----- | ------ |
| `cargo check` | PASS (1.19s) |
| `cargo check --features dev` | PASS (1.28s) |
| `cargo test` | PASS — 191 tests (was 187), 0 failed |
| `cargo test --features dev` | PASS — 194 tests (was 190), 0 failed |
| `cargo clippy --all-targets -- -D warnings` | PASS (zero warnings) |
| `cargo clippy --all-targets --features dev -- -D warnings` | PASS |
| `cargo fmt --check` | PASS (exit 0) |

## Review fixes applied (2026-05-08)

After the reviewer landed verdict LGTM-with-changes, both MEDIUM findings were resolved:

- **MEDIUM-1** — Four plan-mandated tests added (D-I20): `defend_no_ops_when_higher_defense_up_active` (`turn_manager.rs::app_tests`), `use_item_rejects_key_items` (`turn_manager.rs::app_tests`), `silence_blocks_spell_menu` (`ui_combat.rs::app_tests`), `enemy_buff_re_derives_stats` (`ai.rs::app_tests`). The over-specified CombatLog "silenced" assertion in `silence_blocks_spell_menu` was removed because production does not emit that log; menu-state pop assertion preserved per the plan's spec. UX log entry deferred.
- **MEDIUM-2** — `BossAttackDefendAttack { turn }` counter increment fixed (D-I19): `EnemyAi` query upgraded to `&mut`; `*turn = turn.saturating_add(1)` after action select. Cycle test added (`boss_attack_defend_attack_cycles_correctly` in `ai.rs::app_tests`) — 3 ExecuteActions cycles assert `turn == 3` + log contains `"defends!"`.

LOW-1 (vacuous AI tests) and LOW-2 (vacuous defeat path on empty party query) carried as deferred follow-ups, acceptable per reviewer.

## Files shipped

| File | Status | Notes |
| ---- | ------ | ----- |
| `Cargo.toml` | Modified | `rand 0.9` + `rand_chacha 0.9` (dev-dep), `os_rng` feature added |
| `Cargo.lock` | Modified | matching deps |
| `src/plugins/combat/actions.rs` | NEW | `CombatActionKind` enum |
| `src/plugins/combat/ai.rs` | NEW | `EnemyAiPlugin`, `BossAi` enum, queue-only emission |
| `src/plugins/combat/combat_log.rs` | NEW | bounded VecDeque cap=50, kept across combats |
| `src/plugins/combat/damage.rs` | NEW | pure `damage_calc`, Wizardry multiplicative formula |
| `src/plugins/combat/enemy.rs` | NEW | enemy spawn / `Combatant` |
| `src/plugins/combat/targeting.rs` | NEW | `resolve_target_with_fallback` |
| `src/plugins/combat/turn_manager.rs` | NEW | state machine, action queue, speed sort, execute |
| `src/plugins/combat/ui_combat.rs` | NEW | egui overlay, persistent action panel, target selection |
| `src/plugins/combat/mod.rs` | Modified | sub-plugin registrations |
| `src/plugins/party/inventory.rs` | Modified | D-A5 carve-out — drop `With<PartyMember>` filter on `recompute_derived_stats_on_equipment_change` |
| `project/research/...feature-15...md` | NEW | research artifact |
| `project/plans/...feature-15...md` | NEW | plan (Approved → Complete) |
| `project/implemented/...feature-15...md` | NEW | implementation summary (D-I1–D-I22) |
| `project/reviews/...feature-15...md` | NEW | code review (LGTM-with-changes; both MEDIUMs resolved) |
| `project/orchestrator/PIPELINE-STATE.md` | Rewritten | this file |
| `project/orchestrator/20260508-150000-feature-15-turn-based-combat-core.md` | NEW | pipeline summary (with PR URL) |
| `project/shipper/feature-15-commit-msg.txt` | NEW | commit message file |
| `project/shipper/feature-15-pr-body.md` | NEW | reviewer-friendly PR body (TL;DR + mermaid + reviewer guide + scope + risk + test plan) |
| `.claude/agent-memory/...` | NEW/Modified | per-subagent learnings accumulated this session |

## Pipeline scope

Run research → plan → implement → review → ship. User authorized shipping after both MEDIUM findings were resolved.
