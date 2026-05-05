# Pipeline Summary: Feature #11 — Party & Character ECS Model (research → plan → STOP)

**Date:** 2026-05-04
**Pipeline scope:** research → plan → STOP. Parent dispatches implementer manually after plan approval (per established Feature #3-#10 pattern; SendMessage does not actually resume returned agents).
**Status:** Plan delivered. Awaiting user approval before implementer dispatch.
**Suggested branch (for implementer):** `11-party-character-ecs-model`

## Original task

Drive research → plan pipeline (PAUSE at plan-approval) for **Feature #11: Party & Character ECS Model** from the dungeon crawler roadmap. Implement 12 components (`CharacterName`, `Race`, `Class`, `BaseStats`, `DerivedStats`, `Experience`, `PartyRow`, `PartySlot`, `Equipment`, `StatusEffects`, `ActiveEffect`, `StatusEffectType`) with serde derives from start (per research §Pitfall 5), `PartyMemberBundle`, pure `derive_stats(base, equipment, status, level) -> DerivedStats`, `PartySize: Resource`, `spawn_default_debug_party` system, and `assets/data/classes.ron` with 3 classes (Fighter/Mage/Priest only — defer 8-class roster per §Pitfall 6). Race=Human only (per roadmap line 634). PartyPlugin already registered as empty stub at src/main.rs:32.

## Stages run

### Stage 1 — Research (completed in prior orchestrator run)

Research artifact: `/Users/nousunio/Repos/Learnings/claude-code/druum/project/research/20260504-160000-feature-11-party-character-ecs-model.md` (1035 lines).

Key findings (HIGH confidence on all 8 decisions per metadata):
- Recommended approach: **Option A (Components-as-data with `Equipment = Option<Handle<ItemAsset>>`)**
- 12 components decomposed across identity (3) + stats (3) + position (2) + equipment (1) + status (3) layers
- 8 critical pitfalls catalogued (cfg-gating, asset-load timing, HashMap removal in 0.18, serde-from-day-one, Class roster scope discipline, etc.)
- 5 technical Open Questions resolved during planning
- 8 Category C decisions surfaced for user

### Stage 2 — Planning (this run)

Planner re-dispatched with all 8 user-resolved decisions baked in ("all A" defaults). Planner returned the plan with **zero new Category C items** — the 8 user-resolved + 5 technical OQs were exhaustive.

Plan artifact: `/Users/nousunio/Repos/Learnings/claude-code/druum/project/plans/20260504-170000-feature-11-party-character-ecs-model.md`

#### Plan structure

| Aspect | Detail |
|---|---|
| Total steps | 9 (Step 0 baseline + Steps 1-7 implementation + Step 8 docs + Step 9 manual smoke) |
| Atomic commits | 8 (Steps 2+3 combined for the mutual reverse-dep import) |
| Files touched (production) | 7 — `src/plugins/party/mod.rs`, `src/plugins/party/character.rs` (new), `src/data/classes.rs`, `src/data/items.rs`, `src/data/mod.rs`, `assets/classes/core.classes.ron`, `tests/class_table_loads.rs` (new) |
| Files NOT touched | `src/main.rs`, `Cargo.toml`, `Cargo.lock` (cleanest-ship signal: byte-unchanged) |
| Δ deps | 0 |
| Estimated LOC | ~750-950 total (~500-700 in `character.rs`, ~80 in `classes.rs`, ~30 in `items.rs`, ~40 in `mod.rs`, ~50 in integration test, ~50 in 3-class RON) |
| Test count delta | +11 lib tests (8 in `character.rs::tests`, 2 in `classes.rs::tests`, 1 in `items.rs::tests`) + 1 integration test |
| Verification commands | 7 build/lint/test + 11 specific test commands + 3 byte-equality checks + 1 manual smoke |

#### Decisions baked in

All 8 user-resolved Category C decisions ("all A" defaults):

| # | Resolution |
|---|---|
| **1** | Class roster = 3 classes (Fighter, Mage, Priest), with **all 8 enum variants declared** for save-format stability. |
| **2** | Race roster = 5 enum variants declared (Human, Elf, Dwarf, Gnome, Hobbit), with **only Human used** by `spawn_default_debug_party`. |
| **3** | `Equipment` = `Option<Handle<ItemAsset>>`. **Stub `ItemAsset` + `ItemStatBlock` declared in NEW file `src/data/items.rs`**. |
| **4** | Single `src/plugins/party/character.rs` (matches #9/#10 single-file precedent). |
| **5a** | Debug-party gate = `#[cfg(feature = "dev")]` (matches `cycle_game_state_on_f9`). |
| **5b** | Trigger = `OnEnter(GameState::Dungeon)` with idempotence guard. NOT `OnEnter(Loading)`. |
| **6a** | `PartySize::default() = 4` (Grimrock/Etrian-Odyssey standard). |
| **6b** | `PartySize` = hard cap; `spawn_default_debug_party` rejects 5th spawn. |
| **7** | `StatusEffectType` = 5 negatives (Poison, Sleep, Paralysis, Stone, Dead). **`Dead` is a variant, NOT a separate marker.** |
| **8** | `classes.ron` schema = deterministic per-level growth (no `rand`). Δ deps stays 0. |

Five technical OQs from research also resolved at planning time:
1. `derive_stats` returns `current_hp = max_hp`; **callers clamp**.
2. `Experience::xp_to_next_level` is **cached**.
3. All 12 components derive **`Reflect`**.
4. `ItemStatBlock` declared **in #11** in `src/data/items.rs`.
5. `PartyMember` marker and `PartySlot` are **separate components**.

One additional structural decision documented but not surfaced as a question (research recommendation, alternative explicitly rejected): `src/data/classes.rs` imports from `src/plugins/party/character.rs` — a **one-way reverse dep** from `data/` to `plugins/`. Documented inline in both files as intentional.

### Stage 3-5 — NOT IN SCOPE

Per pipeline scope: implement / ship / review are the parent's responsibility after plan approval.

## Hard constraints baked into plan

- Bevy 0.18.1 (verified against research)
- **Δ deps = 0 mandatory** — `Cargo.toml`, `Cargo.lock`, and `src/main.rs` byte-unchanged
- `bevy::utils::HashMap` removed in 0.18 — plan uses `Vec::iter().find()` instead (no HashMap needed for 8-class roster)
- Layer 1 + Layer 2 test pattern with `init_resource::<ButtonInput<KeyCode>>()` gotcha noted under `--features dev`
- Atomic commits (8 commits, one per step except Steps 2+3 combined for mutual reverse-dep import)
- All 12 components derive `Serialize + Deserialize` from the start (Pitfall 5 — non-negotiable)
- NEW files inherit frozen-file convention (treat `src/data/classes.rs`, `src/data/items.rs`, `src/plugins/party/character.rs` with same care as `src/data/dungeon.rs`)
- GitButler workflow per project root `CLAUDE.md`

## Open questions / new Category C items

**Zero.** The 8 user-resolved decisions plus 5 technical OQs were exhaustive. The single new structural choice (reverse dep from `data/classes.rs` to `plugins/party/character.rs`) was resolvable from research recommendations + code-shape pragmatism, so it's documented as a baked-in decision in both Critical and Approach sections rather than re-surfaced.

## Artifacts

| Stage | Artifact |
|---|---|
| Research | `/Users/nousunio/Repos/Learnings/claude-code/druum/project/research/20260504-160000-feature-11-party-character-ecs-model.md` |
| Plan | `/Users/nousunio/Repos/Learnings/claude-code/druum/project/plans/20260504-170000-feature-11-party-character-ecs-model.md` |
| Pipeline state | `/Users/nousunio/Repos/Learnings/claude-code/druum/project/orchestrator/PIPELINE-STATE.md` |

## Follow-up items / notes for next stages

- **Implementer dispatch:** Parent will dispatch the implementer manually with the plan path as input. Suggested branch name: `11-party-character-ecs-model`. The implementer must use GitButler (`but commit`, `but push`) per project root `CLAUDE.md`.
- **Pre-implementation action item:** The plan opens with a "Pre-pipeline action" block (`git fetch origin`, `git checkout main`, `git pull`, `but branch new ja-feature-11-party-character-ecs-model`). Local main was already at PR #10 sha `5f55069` per pipeline state — no rebase needed.
- **Post-implementation:** PartyPlugin is ALREADY registered at `src/main.rs:32` as an empty stub. Plan ADDS to that plugin's `build()`; `src/main.rs` byte-unchanged is the cleanest-ship signal.
- **Future features to track:** Plan's commit messages call out which future features depend on what (#12 = items, #14 = progression, #15 = combat/buffs, #19 = character creation, #23 = save/load). The serde derives shipped here are #23's trust-boundary contract.
