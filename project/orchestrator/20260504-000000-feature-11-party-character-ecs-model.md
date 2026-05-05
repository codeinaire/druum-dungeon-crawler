PIPELINE_STATE_RESET

# Pipeline State

**Task:** Drive research → plan pipeline (PAUSE at plan-approval) for **Feature #11: Party & Character ECS Model** from the dungeon crawler roadmap. Implement 12 components (CharacterName, Race, Class, BaseStats, DerivedStats, Experience, PartyRow, PartySlot, Equipment, StatusEffects, ActiveEffect, StatusEffectType) with serde derives from start (per research §Pitfall 5), `PartyMemberBundle`, pure `derive_stats(base, equipment, status, level) -> DerivedStats`, `PartySize: Resource`, `spawn_default_debug_party` system, and `assets/data/classes.ron` with 3 classes (Fighter/Mage/Priest only — defer 8-class roster per §Pitfall 6). Race=Human only (per roadmap line 634). Roadmap calls for `src/plugins/party/{character.rs, inventory.rs, progression.rs}` multi-file split — surface as Decision 4 since it conflicts with #9's "don't pre-architect" precedent. PartyPlugin already registered as empty stub at src/main.rs:32. PR #10 just merged; local main forwarded to origin/main (5f55069). PIPELINE: research → plan → STOP. Final report at plan stage MUST be self-contained because SendMessage does not actually resume returned agents (confirmed across Features #3-#10); parent dispatches implementer manually. Suggested branch: 11-party-character-ecs-model.

**Status:** in-progress
**Last Completed Step:** 0

## Artifacts

| Step | Description | Artifact                                 |
| ---- | ----------- | ---------------------------------------- |
| 1    | Research    | pending                                  |
| 2    | Plan        | pending                                  |
| 3    | Implement   | NOT IN SCOPE                             |
| 4    | Ship        | NOT IN SCOPE                             |
| 5    | Code Review | NOT IN SCOPE                             |

## User Decisions

(none yet — Category B decisions will be surfaced after research lands)

## Pipeline Scope

This invocation runs research → plan → STOP. After plan approval, parent will manually dispatch implementer (per established Feature #3-#10 pattern). The orchestrator pipeline summary at the end of this run must be self-contained. Pre-pipeline action item resolved: local `main` is at 5f55069 (PR #10 merged on GitHub).
