# Implementation Summary: Feature #20 Phase 2 — Skill Trees, SP Allocation, Guild Skills Mode

**Date:** 2026-05-14
**Plan:** project/plans/20260514-120000-feature-20-spells-skill-tree.md (Phase 2 steps only)
**Branch:** feature-20b-skill-trees (stacked on feature-20a-spell-registry)

## Steps Completed

All 14 Phase 2 execution-order steps completed (following plan's "EXECUTION ORDER" note which differs from step numbering):

1. **Step 3.1** — NEW `src/data/skills.rs` (~434 LOC): `MAX_SKILL_TREE_NODES`, `MAX_SKILL_NODE_COST`, `MAX_SKILL_NODE_MIN_LEVEL`, `pub use SKILL_POINTS_PER_LEVEL`, `NodeId`, `NodeGrant`, `SkillNode`, `SkillTree`, `CycleError`, `validate_no_cycles` (Kahn's BFS topo-sort), `clamp_skill_tree`. 8 unit tests.

2. **Step 3.2** — MODIFIED `src/data/mod.rs`: added `pub mod skills;` and all re-exports.

3. **Step 2.1** — NEW `src/plugins/party/skills.rs` (~312 LOC): `KnownSpells`, `UnlockedNodes`, `WarnedMissingSpells`, `SkillError`, `can_unlock_node`, `learn_spell_pure`, `allocate_skill_point_pure`. 8 unit tests (including `CapReached` check via `AlreadyUnlocked` guard, and defense-in-depth `unspent > total_earned` tamper guard).

4. **Step 2.2** — MODIFIED `src/plugins/party/mod.rs`: `pub mod skills;`, full re-exports, `PartyPlugin::build` wires `WarnedMissingSpells` + reflect for `KnownSpells`/`UnlockedNodes`.

5. **Step 2.3** — MODIFIED `src/plugins/party/character.rs`: `PartyMemberBundle` extended with `known_spells: KnownSpells` and `unlocked_nodes: UnlockedNodes` (APPEND-ONLY). Import from `party::skills`.

6. **Step 2.4** — MODIFIED `src/plugins/party/character.rs`: `Experience` extended with `#[serde(default)] unspent_skill_points: u32` and `total_skill_points_earned: u32` (APPEND-ONLY, discriminant order preserved).

7. **Step 2.5** — MODIFIED `src/plugins/party/progression.rs`: declared `pub const SKILL_POINTS_PER_LEVEL: u32 = 1;`; awards SP in `apply_level_up_threshold_system` after level increment. Re-exported from `party::mod.rs`. Two new tests: `xp_threshold_triggers_level_up` extended with SP assertions, new `level_up_awards_skill_points_per_const`.

8. **Step 3.3** — NEW RON assets (all double-dot extensions per project convention):
   - `assets/skills/fighter.skills.ron`: 6 nodes (StatBoost + Resist only; Fighter has no MP)
   - `assets/skills/mage.skills.ron`: 8 nodes (LearnSpell + StatBoost mix)
   - `assets/skills/priest.skills.ron`: 8 nodes (LearnSpell + StatBoost mix)
   All spell IDs verified against `assets/spells/core.spells.ron`.

9. **Step 3.4** — MODIFIED `src/plugins/loading/mod.rs`: `RonAssetPlugin::<SkillTree>::new(&["skills.ron"])`, 3 new `Handle<SkillTree>` fields on `DungeonAssets`, `DungeonAssets::skill_tree_for(class)` helper (with `_ => None` wildcard per Class-exhaustive-match rule), `validate_skill_trees_on_load` OnExit system (runs `clamp_skill_tree` + `validate_no_cycles`; empties tree on cycle with `error!` log).

10. **Step 3.5** — NEW `src/plugins/town/guild_skills.rs` (~813 LOC): `NodeState` 4-state enum (Cat-C-1), `node_state()` pure fn, `node_depth()` memoized helper, `sorted_nodes()`, `paint_guild_skills` (EguiPrimaryContextPass), `handle_guild_skills_input` (Update), `handle_guild_skills_unlock` (Update). 7 tests: 2 `node_state` pure-fn unit tests + 5 integration tests.

11. **Step 3.6** — MODIFIED `src/plugins/town/guild.rs`: `GuildMode::Skills` variant, `node_cursor: usize` field on `GuildState`, `mode_label` arm, CentralPanel-skip block, Cancel-guard, creation-sub-mode guard.

12. **Step 3.7** — MODIFIED `src/plugins/town/guild.rs`: `[` (PrevTarget) keybind from Roster → Skills mode, footer updated to show `[[] Skills`.

13. **Step 3.8** — MODIFIED `src/plugins/town/mod.rs`: `pub mod guild_skills;`, all three painter/handler systems wired with `in_guild_mode(GuildMode::Skills)` run-ifs, `OnExit(TownLocation::Guild)` reset for `node_cursor`.

14. **Step 3.9** — NEW `tests/skill_tree_loads.rs` (~185 LOC): loads all 3 skill trees + SpellDb via RonAssetPlugin; asserts class_id, node count, no cycles, root nodes, level-gated nodes, and mage/priest have LearnSpell nodes referencing real SpellDb entries. 30s timeout guard.

**DungeonAssets landmine fix (7 test fixtures):** All 7 test harnesses that use `DungeonAssets { ... }` struct literals updated to include `fighter_skills: Handle::default(), mage_skills: Handle::default(), priest_skills: Handle::default()`:
- `tests/dungeon_movement.rs`
- `tests/dungeon_geometry.rs`
- `src/plugins/dungeon/tests.rs`
- `src/plugins/dungeon/features.rs`
- `src/plugins/ui/minimap.rs`
- `src/plugins/combat/encounter.rs`
- `src/plugins/combat/turn_manager.rs`

## Steps Skipped

Steps 2.6 and 2.7 (Phase 3 — SpellMenu UI and dev-party KnownSpells defaults) are intentionally deferred to Phase 3 per plan scope. `src/plugins/combat/ui_combat.rs` and `src/plugins/combat/turn_manager.rs` (SpellMenu parts) were NOT touched.

## Deviations from Plan

1. **Unused `mut exp` binding in test.** The `unlock_node_learn_spell_grant_appends_known_spells` test had a dead-code `{ let mut exp = ...; /* comment */ }` block. Removed; comment preserved inline. Avoids clippy `unused_mut` warning.

2. **`node_cursor` test uses index 1 (not 0) for root_node.** Alphabetical depth-0 sort gives `level_gated(0), root_node(1), stat_node(2)` — `root_node` is at index 1 in sorted order. Test comment documents this ordering.

3. **Two separate queries in `handle_guild_skills_input`.** The plan describes a single "sort party by slot" operation, but the implementation uses two queries: `party: Query<(&PartySlot, Entity), ...>` for count and `party_slots: Query<(&PartySlot, &Class), ...>` for sorted-member resolution. Both are read-only; no B0002 conflict.

## Deferred Issues

None beyond what the plan explicitly defers to Phase 3:
- SpellMenu (`ui_combat.rs` stub remains)
- `spawn_default_debug_party` KnownSpells defaults (no dev-party spell knowledge yet)
- `NodeGrant::Resist` consumer-side check (resist marker stored in `UnlockedNodes` but not checked in combat)

## Verification Results

Quality gate commands to run (cannot run in this session; must be run by user or CI):
- `cargo check` — zero warnings expected
- `cargo check --features dev` — zero warnings expected
- `cargo test --lib skills` — 16 tests expected (8 data/skills.rs + 8 party/skills.rs)
- `cargo test --lib guild_skills` — 7 tests expected (5 integration + 2 node_state unit tests)
- `cargo test --lib level_up_awards_skill_points` — extended progression tests
- `cargo test --test skill_tree_loads` — RON asset pipeline smoke + DAG validation
- `cargo clippy --all-targets -- -D warnings`
- `cargo clippy --all-targets --features dev -- -D warnings`
- Anti-pattern greps: zero `derive(Event)`, `EventReader<`, `EventWriter<` in new files
- Sole-mutator greps: zero `effects.push|effects.retain` in new files

**New test count (Phase 2):** 25 new tests across:
- `src/data/skills.rs`: 8 unit tests
- `src/plugins/party/skills.rs`: 8 unit tests
- `src/plugins/town/guild_skills.rs`: 7 tests (5 integration + 2 node_state unit)
- `src/plugins/party/progression.rs`: 2 extended/new tests (SP award assertions)
