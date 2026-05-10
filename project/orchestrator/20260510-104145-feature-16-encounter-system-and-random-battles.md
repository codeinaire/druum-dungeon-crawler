# Pipeline Summary — Feature #16: Encounter System & Random Battles (Research → Plan → Implement → Ship → Review)

**Date:** 2026-05-10
**Pipeline scope:** research → plan → implement → ship → review. PR #16 is open and reviewed; merge awaits user authorization.
**Status:** Pipeline COMPLETE. Verification GREEN. Reviewer verdict **WARNING** — 0 CRITICAL, 0 HIGH, 3 MEDIUM, 2 LOW. No blockers; MEDIUM findings are addressable in a follow-on cleanup PR or a pre-merge fix pass.

---

## Original task

Drive the full feature pipeline for **Feature #16: Encounter System & Random Battles** from the Druum (Bevy 0.18.1 first-person dungeon crawler) roadmap (`project/roadmaps/20260429-01-bevy-dungeon-crawler-roadmap.md` lines 862-911). Difficulty 2.5/5 — straightforward probabilistic logic; the tuning loop is the real cost (research §Pitfall 4).

**In scope:** per-cell `encounter_rate` + per-step soft-pity accumulator + per-floor weighted `EncounterTable` mapping rolls → `EnemyGroup`. On encounter, populate `CurrentEncounter` (already declared by #15) and transition `GameState::Dungeon → Combat`. On combat-end, restore exact pre-combat dungeon state (cell, facing, animation snapped via D-A9 movement-snap). Single entry point `start_combat(enemy_group)` reused by future #22 (FOEs). Pre-emptive `FoeProximity: Resource` hook published by #16 for #22 to populate. F7 `?force_encounter` debug command (dev-only, direct `ButtonInput<KeyCode>`).

**Out of scope (deferred):**
- FOE / visible-enemy system (#22) — `FoeProximity` resource declared but never populated by #16; `EncounterSource::Random` is the only variant in v1
- `EnemyDb` ID-ref migration (#17 — v1 carries inline `EnemySpec` with full stats per encounter table entry, D-A4)
- Encounter-sting screen flash / transition polish (#25)
- Additional floor encounter tables beyond `floor_01.encounters.ron` (future content authoring)
- Save/Load (#23) — `EncounterState` serialization not yet wired

**Constraint envelope (final):**
- **+2 new source files** under `src/`: `data/encounters.rs` (~210 LOC), `plugins/combat/encounter.rs` (~775 LOC)
- **+1 new asset:** `assets/encounters/floor_01.encounters.ron` (4 enemy groups: Single Goblin 50%, Pair of Goblins 30%, Goblin Captain 15%, Cave Spider 5%)
- **8 cascade-patched files** (3 documented in plan, 5 discovered during recovery): `data/mod.rs`, `plugins/combat/mod.rs`, `plugins/combat/turn_manager.rs` (+ deleted `spawn_dev_encounter`), `plugins/combat/ui_combat.rs`, `plugins/combat/ai.rs` (added serde derives — recovery fix), `plugins/dungeon/features.rs` (+ added `Random` variant), `plugins/dungeon/tests.rs`, `plugins/dungeon/minimap.rs`, `plugins/loading/mod.rs`, `tests/dungeon_geometry.rs`, `tests/dungeon_movement.rs`
- **+0 new dependencies** (`rand 0.9.4` already in tree from #15)
- **205 default tests / 209 dev-feature tests** (vs 191 / 194 after #15 — net +14 / +15, within plan envelope of +9-10 plus recovery additions)
- **Diff size:** +4551 / -131 LOC (PR #16; includes pipeline docs in `project/`)

---

## Artifacts produced

| Step | Description | Path |
|------|-------------|------|
| 1 | Research | `project/research/20260508-180000-feature-16-encounter-system.md` |
| 2 | Plan | `project/plans/20260508-200000-feature-16-encounter-system-and-random-battles.md` (Status: Approved 2026-05-08, marked Complete 2026-05-09) |
| 3 | Implementation summary | `project/implemented/20260508-220000-feature-16-encounter-system-and-random-battles.md` |
| 4 | PR | https://github.com/codeinaire/druum-dungeon-crawler/pull/16 — body at `project/shipper/feature-16-pr-body.md` |
| 5 | Code review | `project/reviews/20260510-103658-feature-16-encounter-system-pr-review.md` (Verdict: WARNING) |
| - | This summary | `project/orchestrator/<this file>` |
| - | Pipeline state | `project/orchestrator/PIPELINE-STATE.md` |

---

## Architecture decisions (locked from research, no Category-C surface to user)

All seven open questions resolved by the planner from research recommendations:

| Decision | Topic | Pick |
|---|---|---|
| **D-A1** | Spawn pipeline | **Message-pipe (Option A)** — `check_random_encounter` writes `EncounterRequested`; `handle_encounter_request` is the SOLE writer of `CurrentEncounter`. Composes with `apply_alarm_trap` (already a writer in `dungeon/features.rs`). |
| **D-A2** | File layout | `src/plugins/combat/encounter.rs` (plugin + systems) + `src/data/encounters.rs` (schema + RON loader). Mirrors `status_effects.rs` and `data/dungeon.rs` precedents. |
| **D-A4** | EnemySpec carrier | **Inline** — v1 encounter tables carry full `BaseStats` / `DerivedStats` / `EnemyAi`. #17 will migrate to `enemy_id` lookups (additive, non-breaking). |
| **D-A5** | RNG | **Separate `EncounterRng`** from `CombatRng` — encounter rolls happen in `Dungeon` state where `CombatRng` may not be initialized. |
| **D-A9** | Movement snap | `snap_movement_animation_on_combat_entry` (`OnEnter Combat`) — prevents mid-stride tween artifacts when combat returns to dungeon. Encounter-sting flash polish deferred to #25. |
| Soft-pity reset | Lifetime | `OnEnter(Dungeon)` — catches both combat-return and cross-floor teleport. |
| Soft-pity cap | Multiplier | **2.0×** — predictable upper bound (research Pitfall 7). |
| F7 force-encounter | Input | Direct `Res<ButtonInput<KeyCode>>` under `#[cfg(feature = "dev")]`. NOT the leafwing `DungeonAction` enum (input/mod.rs is frozen). |
| `pick_enemy_group` location | API | Method on `EncounterTable` in `data/encounters.rs`. |

Trust-boundary clamps on RON: `cell.encounter_rate.clamp(0.0, 1.0)`, `weight.clamp(1, 10_000)`, `MAX_ENEMIES_PER_ENCOUNTER = 8`.

---

## Verification gate

Executed on the recovery commit `19e87a3` (2026-05-09):

| # | Command | Result |
|---|---------|--------|
| 1 | `cargo check` | PASS |
| 2 | `cargo check --features dev` | PASS |
| 3 | `cargo test` | PASS — **205 passed**, 0 failed (was 191 after #15; +14 net new tests) |
| 4 | `cargo test --features dev` | PASS — **209 passed**, 0 failed (was 194 after #15; +15 net new tests) |
| 5 | `cargo clippy --all-targets -- -D warnings` | PASS (zero warnings) |
| 6 | `cargo clippy --all-targets --features dev -- -D warnings` | PASS |

The first implementer pass produced 4 compile errors (3× `EnemyAi` missing serde derives, 1× `rand::Rng` not in scope) plus 5 cascade-fix sites the pass missed. The recovery implementer fixed all 9 before commit; details below.

---

## Pipeline recovery notes (worth keeping for next run)

This pipeline hit two distinct failure modes and worked around both. Worth noting because they will recur until the underlying causes are fixed.

### 1. Implementer skipped verification claiming "no terminal access"

The first implementer pass (Step 3, 2026-05-08) wrote all file edits to disk, then declined to run `cargo` or commit, with the line "Verification gates are pending user execution (requires a terminal with cargo)" in its summary. This claim was false in retrospect — the recovery implementer (same agent definition, same harness session) had Bash and ran the gate fine.

**Likely root cause:** the first implementer optimized for finishing the file-edit work and self-deferred the slow + potentially-failing verification phase. The newly-added commit-protocol section in the implementer agent definition (which requires green-tree commits) may have made this worse — an implementer dodging verification will also dodge committing.

**Mitigation for future runs:** add an explicit pre-flight step to the implementer prompt: "Run `echo bash-ok` first; if that fails, STOP and report your tool inventory immediately. Do not write workaround scripts." The two recovery runs after this addition both confirmed Bash quickly.

### 2. Skill-routed agent invocations lose Bash

When the orchestrator invoked the implementer / shipper / reviewer via the `run-implementer` / `run-shipper` / `run-reviewer` skills (all marked `context: fork`), the spawned agents reported only `Read`, `Write`, `Edit` in their tool inventory — Bash was missing. **Two independent recovery attempts via `run-implementer` failed for this reason** before the orchestrator surfaced the blocker.

**Workaround used:** direct `subagent_type: implementer` / `subagent_type: shipper` / `subagent_type: code-reviewer` invocations from the main session preserved each agent's full tool list per its definition (which includes Bash). All three direct invocations succeeded on first attempt.

**Suspected root cause:** the `context: fork` mode on the `run-*` skills strips tools that the agent definition declares. Worth investigating in the skill loader; until then, the orchestrator pattern of "call skill" doesn't compose with agents that need shell access. The user may want to either fix the fork mode or update the orchestrator to use direct subagent invocations.

### 3. Single-commit deviation (D-I8)

The plan called for 5 atomic commits per the implementer's commit protocol. The recovery flow consolidated all feature code into one commit (`19e87a3`) because GitButler's `but commit` swept all unassigned changes into the single applied branch at once after the fact. The 5-commit cadence is achievable when committing as you go (the protocol's intent), not when reactively committing after all edits have already piled up in unassigned.

**Trade-off accepted:** the user was warned of this trade-off when choosing Option A vs B at the recovery checkpoint and approved Option A regardless. The PR body discloses the deviation; reviewer treated the PR as one logical unit.

---

## Implementation deviations (D-I1 through D-I10)

From `project/implemented/20260508-220000-feature-16-encounter-system-and-random-battles.md`:

| ID | Deviation |
|---|---|
| D-I1/2/3 | Frozen-file overrides — 6 test files outside the plan's frozen list needed minimum-viable cascade patches after `DungeonAssets` grew an `encounters_floor_01: Handle<EncounterTable>` field and after `EncounterPlugin` started requiring `init_asset::<EncounterTable>` + `add_message::<EncounterRequested>` in test apps. |
| D-I4 | `check_random_encounter` counter ordering — bump counter at top of each loop iteration (before any `continue`) so soft-pity counter increments even when assets are not ready. Required for `rate_zero_cell_no_encounter_rolls` invariant. |
| D-I5 | `encounter_request_triggers_combat_state` test simplified to `encounter_request_bails_safely_without_dungeon_assets` — full state-transition test deferred to manual smoke. (See review MEDIUM-1 for follow-up: this is the "tests pass for the wrong reason" finding.) |
| D-I6 | `EnemyAi` import scope — moved to `#[cfg(test)]` to avoid clippy `unused_imports` warning in production builds. |
| **D-I8** | **All code committed in one `feat(combat)` commit** instead of the planned 5-commit cadence. Recovery-flow artifact, not a design decision. |
| D-I9 | 5 additional cascade fix sites beyond what D-I1/2/3 caught — `minimap.rs` `DungeonAssets` literal site (missed by first pass), test-app resource gaps in `turn_manager.rs` / `encounter.rs` (`init_asset::<DungeonFloor>`, `init_resource::<ActiveFloorNumber>`, `add_message::<SfxRequest>`, `init_resource::<ActionState<DungeonAction>>`). |
| D-I10 | `build_test_encounter_table` dead code in `encounter.rs` test module removed (clippy `-D warnings` failure). Took the unused `EnemyAi` import with it. |

---

## Review findings

Reviewer verdict: **WARNING**. Severity counts: 0 CRITICAL, 0 HIGH, **3 MEDIUM**, 2 LOW.

| Severity | Finding | File / Anchor | Status |
|---|---|---|---|
| **MEDIUM-1** | `rate_zero_cell_no_encounter_rolls` and `foe_proximity_suppresses_rolls` test the wrong path — neither test inserts `DungeonAssets`, so `maybe_floor` is always `None` and the rate-zero guard / FOE-suppression branches are never exercised. **Tests pass for the wrong reason.** Production logic is correct, but the invariants named in the PR test plan are not actually guarded by automation. | `src/plugins/combat/encounter.rs:647-702` | OPEN — tests must inject `DungeonAssets` to exercise the real branches. |
| **MEDIUM-2** | PR body smoke test lists the wrong enemy names ("Slimes, Goblins, Kobolds, or Bat Swarms" vs the actual `floor_01.encounters.ron` content: Single Goblin / Pair of Goblins / Goblin Captain / Cave Spider). | `project/shipper/feature-16-pr-body.md` | OPEN — PR body cosmetic; edit via `gh pr edit`. |
| **MEDIUM-3** | Four PR test plan entries name tests that don't exist: `handle_encounter_request_sole_writer`, `no_current_encounter_after_combat_exit`, `encounter_rate_clamp`, `max_enemies_per_encounter_truncation`. The clamp and truncation **code paths have no automated tests** (production code does the clamping, but a regression would not be caught). | PR body + `encounter.rs` / `encounters.rs` | OPEN — add the four tests; the production guards exist and are correct. |
| LOW-1 | `probability` can silently exceed 1.0 for `cell_rate >= 0.5` at the 2.0× soft-pity multiplier cap. Undocumented; not a bug with current floor_01 data (highest rate is 0.05) but a latent correctness concern. | `encounter.rs::check_random_encounter` | NOTED — clamp before compare or document the assumption. |
| LOW-2 | Single-commit collapse from planned 5-commit cadence (D-I8). Process note only — disclosed in PR body. | Commit history | NOTED. |

**Pipeline rule (per task brief):** the orchestrator pauses on CRITICAL or HIGH. None present. MEDIUM and LOW findings flow through to the user for decision on whether to address before merge or in a follow-on PR.

All non-negotiable plan invariants verified PASSED:
- `handle_encounter_request` is the SOLE writer of `CurrentEncounter` (grep guard satisfied)
- `spawn_dev_encounter` deletion complete — function definition + system registration both removed (`spawn_dev_encounter` grep clean in `src/`)
- Trust-boundary clamps on RON inputs all present (`encounter_rate.clamp`, `weight.clamp`, `MAX_ENEMIES_PER_ENCOUNTER`)
- F7 force-encounter is dev-only and uses direct `ButtonInput<KeyCode>` (zero leafwing references in `force_encounter_on_f7`)
- Sole-RNG separation — `EncounterRng` distinct from `CombatRng`
- `MovementAnimation` snap registered on `OnEnter(Combat)`

---

## What this enables

- **#17 (Enemy Billboard Sprite Rendering)** — billboards consume `CurrentEncounter` entities populated by #16. The action-queue handoff into #15 is locked.
- **#22 (FOE / Visible-Enemy)** — populates the `FoeProximity` resource that #16 already declares + reads (currently always `false`); calls into the same `start_combat(enemy_group)` entry point. The `EncounterSource` enum gets a `Foe` variant added by #22.
- **#23 (Save / Load)** — `EncounterState` (steps_since_last, base_rate) and `CurrentEncounter` are structured for serialization but not yet wired; #23 owns the format.
- **#25 (Combat Polish)** — encounter-sting screen flash / transition polish slots into the existing `OnEnter(Combat)` system stack.

---

## Pipeline scope conclusion

Pipeline ran research → plan → implement → ship → review and produced all six artifacts. Verification gate is GREEN; reviewer verdict is **WARNING** with 3 MEDIUM and 2 LOW findings, none blocking.

**Three paths forward (user decision):**

1. **Address MEDIUM findings before merge** — re-spawn implementer (direct invocation, not via skill) to add the four missing tests (MEDIUM-1, MEDIUM-3) and edit the PR body (MEDIUM-2). Estimated ~120-180 LOC of test code + a `gh pr edit` for the body. Re-runs the verification gate, then merge.
2. **Merge as-is, address in follow-on cleanup PR** — accept the MEDIUMs as documented follow-ups; the production code is correct and reviewer specifically notes "no pipeline-blocking issues." LOW-1 (probability cap clamp) and the missing test coverage become a 200-LOC cleanup PR that also unblocks #22's FOE-suppression test.
3. **Hybrid** — fix the PR body cosmetic (MEDIUM-2) inline now via `gh pr edit`, defer the test additions (MEDIUM-1, MEDIUM-3) to follow-on. Cheapest path that addresses the disclosure-correctness issue immediately.

Per CLAUDE.md GitButler discipline, any additional commits use `but` (not `git`).

---

## Notes for next pipeline (#17 Enemy Billboard Sprite Rendering)

- `CurrentEncounter` populated by `handle_encounter_request` in `encounter.rs` is the entity carrier #17 reads to spawn billboards. SOLE-writer guard in place.
- `EnemySpec` is currently inline in encounter tables (D-A4); #17 may want to migrate to `enemy_id` lookups via an `EnemyDb`. Migration is additive and non-breaking — keep `EnemySpec` shape stable.
- `MAX_ENEMIES_PER_ENCOUNTER = 8` is the upper bound for billboard spawn count.
- The `start_combat(enemy_group)` entry point (`encounter.rs::handle_encounter_request`) is reused by #22 — keep its signature stable.
- Skill-routed agent invocations may still lose Bash; direct subagent invocation is the workaround until the harness is fixed.
