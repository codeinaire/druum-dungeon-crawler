# Plan: Feature #20 — Spells & Skill Trees

**Date:** 2026-05-14
**Status:** Phase 3 Implemented — 2026-05-14. Phase 1 (spell registry) shipped as PR #21. Phase 2 (skill trees + Guild Skills) implemented 2026-05-14. Phase 3 (SpellMenu UI) implemented 2026-05-14; awaiting cargo check/test gate run and user ship sequence.
**Research:** project/research/20260514-druum-20-spells-skill-tree.md
**Depends on:** 20260513-120000-feature-19-character-creation.md, 20260512-173000-feature-18b-town-temple-guild.md, 20260508-100000-feature-15-turn-based-combat-core.md

## User Decisions (locked 2026-05-14)

All planner defaults from the original "Open Questions" section are **ACCEPTED**, with one override on PR shape:

| Question | Decision | Notes |
|---|---|---|
| Q1 — Per-class trees | **ALL three classes** | Fighter gets 6 passive nodes (StatBoost + Resist only; no LearnSpell — Fighter has `mp_per_level: 0`). |
| Q3 — MP regeneration | **None new** | Level-up + Inn rest only. Matches Wizardry/Etrian convention. |
| Q6 — Skill points/level-up | **1 SP/level** | `SKILL_POINTS_PER_LEVEL = 1`. Single const; future polish can swap. |
| Q9 — Missing spell ID | **warn-once-per-(spell,character)-then-filter** | Mirrors `recompute_derived_stats_on_equipment_change` warn pattern. |
| Q10 — Spell icons | **Defer to #25 polish** | `icon_path: ""` for all 15 spells day-one. Day-one painter: name + MP cost only. |
| Q11 — Spell-sim debug | **Defer to own PR** | Balance tooling, not gameplay. Future follow-up. |
| **PR shape (OVERRIDE)** | **THREE separate PRs** | Phase 1 / Phase 2 / Phase 3. Each independently compiles, tests, ships. Orchestrator pauses for confirmation between phases. |
| GH issue reconciliation | No separate spec issue | `gh issue view 20` returns PR #20 (merged #19). Roadmap (`project/roadmaps/20260429-01-bevy-dungeon-crawler-roadmap.md` lines 1079–1127) is the source of truth. |

Cat-A planner-resolved items (Q2 race-spells, Q4 no group targeting, Q5 spells ignore rows, Q7 no respec, Q8 no trees for the 5 unauthored classes) also stand.

### Phase 2 Cat-C resolutions (locked 2026-05-14, post-Phase-1-ship)

| Question | Decision | Notes |
|---|---|---|
| **Cat-C-1** — Painter state count | **Option B — 4-state painter with yellow tier** | Unlocked / can-unlock-now / SP-insufficient-but-otherwise-met (**yellow**) / locked. Helps users plan SP saves across level-ups. Adds 2 `node_state` pure-fn unit tests. See Step 3.5. |
| **Cat-C-3** — `NodeGrant::LearnSpell(SpellId)` validation scope | **Option A — warn-and-filter at consume-time only** | Validator (`validate_skill_trees_on_load`) stays structural: cycles + clamp only. Bogus spell IDs flow into `KnownSpells` and surface in Phase 3's `SpellMenu` painter via the Q9 `WarnedMissingSpells: HashSet<(SpellId, Entity)>` warn-once mechanism. Single source of truth for missing-spell handling. See Step 3.4. |

## Goal

Replace the empty `SpellTable {}` stub at `src/data/spells.rs` and the "not yet implemented" `CastSpell` resolver at `turn_manager.rs:531-541` with a fully data-driven spell registry, a working in-combat spell submenu, and a per-class skill tree that converts level-up skill points into learned spells and passive stat boosts. Day-one scope: 15-20 starter spells across Mage and Priest schools, per-class skill trees for Fighter / Mage / Priest, and a Guild "view tree / spend point" screen. Δ Cargo.toml = 0. All combat resolver work re-uses the existing `ApplyStatusEvent` / `check_dead_and_apply` / `EquipmentChangedEvent { slot: EquipSlot::None }` seams.

## Approach

Ship in **three sub-phases as THREE SEPARATE PRs**, ordered by dependency, each on its own branch:

- **Phase 1 — Spell registry + cast resolver.** `SpellTable→SpellDb` rename (3-file coupled), `SpellId=String`, `Spell{id,mp_cost,target,school,level,effect}`, `SpellEffect` enum (Damage/Heal/ApplyStatus/Buff/Revive/Special). `cast_spell` resolver replaces the `CastSpell` stub at `turn_manager.rs:531-541`. 15 starter spells authored in `assets/spells/core.spells.ron`. Phase 1 ships **without** any UI changes — the SpellMenu stub stays a stub. Phase 1 PR is reviewable on its own because all the seam discipline (sole-mutator, pure spell_damage_calc, MAX_* clamps) lives here.
- **Phase 2 — Skill trees + SP allocation.** `KnownSpells`, `UnlockedNodes`, `SkillPoints` (as `Experience` extension), `WarnedMissingSpells` resource. `SkillTree { nodes, prerequisites }` per-class via `assets/skills/<class>.skills.ron`. `NodeGrant::{LearnSpell, StatBoost, Resist}`. `validate_no_cycles` (Kahn's) at load. SP awarded on level-up. Three class trees authored (Fighter 6 passives, Mage 8 mixed, Priest 8 mixed). **Phase 2 does NOT yet enable spell casting in combat** — the SpellMenu is still a stub. KnownSpells is populated but not consumed.
- **Phase 3 — Combat UI: spell submenu.** Functional `MenuFrame::SpellMenu` (two-pane: spell list + description). Repurpose `MenuAction::PrevTarget` (`[` key) for Skills entry from Guild Roster. Guild `Skills` sub-mode for spending points. End-to-end smoke: cast a spell in combat after a fresh level-up + skill-point spend.

**Each PR is independently shippable.** The compile-and-test gates pass at each phase boundary (see "Phase boundaries" section below for the exact files-per-phase split + per-phase verification). Cross-phase forward dependencies (e.g. Phase 2's `apply_level_up_threshold_system` references `SKILL_POINTS_PER_LEVEL`) are resolved by declaring the constant in the file owned by the phase that introduces it (`progression.rs` in Phase 2; Phase 3's `data/skills.rs` re-exports via `pub use`).

## Phase boundaries

Each phase is its own branch, its own commit, its own PR, its own implement → review → ship cycle. Each phase's compile-and-test gates pass independently. The orchestrator **pauses for explicit user confirmation between phases**.

### Phase 1 — Spell registry + cast resolver

**Branch:** `feature-20a-spell-registry`
**PR title:** `feat(combat): add spell registry and cast resolver (#20a)`
**Scope line for PR body:** Replace the empty `SpellTable` stub with a data-driven `SpellDb` + 15 starter spells, and wire `CastSpell` in `execute_combat_actions` to a real resolver. No UI changes — `SpellMenu` stays a stub until Phase 3.

**Files in this phase (corresponds to Steps 1.1–1.8 below):**

- `src/data/spells.rs` — REWRITE (stub → full schema + MAX_* + clamp helper + 6 unit tests). ~+220 LOC.
- `src/data/mod.rs` — re-exports updated (`SpellTable` → `SpellDb, SpellAsset, SpellEffect, …`). ~+2 modified LOC.
- `src/plugins/loading/mod.rs` — `RonAssetPlugin::<SpellDb>` registration; add `pub spells: Handle<SpellDb>` to `DungeonAssets`. ~+8 modified LOC.
- `assets/spells/core.spells.ron` — REWRITE (4-line stub → 15 spells). ~+150 LOC. **Double-dot filename** — already named correctly.
- `src/plugins/combat/spell_cast.rs` — NEW (pure-fn module: `SpellCombatant`, `spell_damage_calc`, `check_mp`, `deduct_mp` + 5 unit tests). ~+200 LOC.
- `src/plugins/combat/mod.rs` — register `pub mod spell_cast;`. ~+1 modified LOC.
- `src/plugins/combat/turn_manager.rs` — replace CastSpell stub at lines 531–541 with the real resolver; add 7 `cast_spell_*` unit tests via `app_tests`. ~+130 modified LOC for resolver + ~+150 LOC for tests.
- `tests/spell_db_loads.rs` — NEW integration test (mirror of `tests/item_db_loads.rs`). ~+85 LOC.

**Types introduced in Phase 1:** `SpellId`, `SpellSchool`, `SpellTarget`, `SpellEffect`, `SpellAsset`, `SpellDb`, `SpellCombatant`, `SpellDamageResult`.
**Constants introduced in Phase 1:** `MAX_SPELL_MP_COST`, `MAX_SPELL_DAMAGE`, `MAX_SPELL_HEAL`, `MAX_SPELL_DURATION`, `KNOWN_SPELLS_MAX`.

**Test counts at Phase 1 completion:** 6 (`data/spells.rs`) + 5 (`spell_cast.rs`) + 7 (`turn_manager.rs::cast_spell_*`) + 1 (`tests/spell_db_loads.rs`) = **19 new tests**. Plus existing-baseline tests still pass.

**Δ Cargo.toml = 0** at Phase 1.

**Phase 1 verification gate (must pass before merging Phase 1):**

- [ ] `cargo check` and `cargo check --features dev` — zero warnings
- [ ] `cargo test` and `cargo test --features dev` — all existing tests + 19 new tests pass
- [ ] `cargo clippy --all-targets -- -D warnings` and same with `--features dev`
- [ ] `cargo test --test spell_db_loads` — RON load smoke
- [ ] Anti-pattern grep: `! grep -rE "derive\(Event\)|EventReader<|EventWriter<" src/plugins/combat/spell_cast.rs` returns zero matches
- [ ] Sole-mutator grep: `! grep -rE "effects\.push|effects\.retain" src/plugins/combat/spell_cast.rs` returns zero matches (the `effects.retain` in `turn_manager.rs::CastSpell::Revive` is the SOLE permitted use)
- [ ] Manual smoke: `cargo run --features dev` — no panic loading `assets/spells/core.spells.ron`

**Phase 1 is NOT user-facing yet** — the SpellMenu still says "Not implemented" (because the stub remains). Reviewer should expect this.

### Phase 2 — Skill trees + SP allocation + Guild "Skills" mode

**Branch:** `feature-20b-skill-trees`
**PR title:** `feat(party): add skill trees, skill points, and Guild Skills mode (#20b)`
**Scope line for PR body:** Per-class skill trees (Fighter passives / Mage spells / Priest spells), 1 SP awarded per level-up, Guild "Skills" sub-mode to spend points. `KnownSpells` and `UnlockedNodes` components populated but NOT yet consumed by combat (SpellMenu still a stub — that's Phase 3).

**Files in this phase (corresponds to Steps 2.1–2.5 + 3.1–3.9 below, REGROUPED):**

- `src/data/skills.rs` — NEW (MAX_* + `NodeGrant`, `SkillNode`, `SkillTree`, `CycleError`, `validate_no_cycles`, `clamp_skill_tree` + 8 unit tests). ~+250 LOC.
- `src/data/mod.rs` — register `pub mod skills;` + re-exports. ~+4 modified LOC.
- `src/plugins/party/skills.rs` — NEW (`KnownSpells`, `UnlockedNodes`, `WarnedMissingSpells`, `SkillError`, `can_unlock_node`, `learn_spell_pure`, `allocate_skill_point_pure` + 8 unit tests). ~+150 LOC.
- `src/plugins/party/mod.rs` — `pub mod skills;` + re-exports + plugin wiring (`WarnedMissingSpells` init_resource + reflect registration). ~+5 modified LOC.
- `src/plugins/party/character.rs` — extend `PartyMemberBundle` with `KnownSpells` + `UnlockedNodes` fields; append `unspent_skill_points` + `total_skill_points_earned` to `Experience`. ~+10 modified LOC.
- `src/plugins/party/progression.rs` — declare `pub const SKILL_POINTS_PER_LEVEL: u32 = 1;` here (cross-phase home — Phase 3 would have nowhere else to declare it without forward dep); award SP in `apply_level_up_threshold_system` after `level += 1`. ~+10 modified LOC.
- `assets/skills/fighter.skills.ron` — NEW (6 nodes, passives only). ~+50 LOC. **Double-dot filename.**
- `assets/skills/mage.skills.ron` — NEW (8 nodes, mix of LearnSpell + StatBoost). ~+80 LOC.
- `assets/skills/priest.skills.ron` — NEW (8 nodes, similar mix). ~+80 LOC.
- `src/plugins/loading/mod.rs` — `RonAssetPlugin::<SkillTree>` registration; add 3 per-class `Handle<SkillTree>` fields to `DungeonAssets`; `skill_tree_for(class)` helper; `OnExit(GameState::Loading)` validation system that runs `validate_no_cycles + clamp_skill_tree`. ~+42 modified LOC.
- `src/plugins/town/guild_skills.rs` — NEW (`paint_guild_skills`, `handle_guild_skills_input`, `handle_guild_skills_unlock`, private pure `node_state(node, experience, unlocked) -> NodeState` for the Cat-C-1 4-state painter + 5 integration tests + 2 `node_state` unit tests). ~+270 LOC.
- `src/plugins/town/guild.rs` — extend `GuildMode` enum with `Skills` variant; `node_cursor` field on `GuildState`; `[` keybind entry from Roster. ~+12 modified LOC.
- `src/plugins/town/mod.rs` — register `pub mod guild_skills;` + paint/input/unlock systems + run-if gates. ~+10 modified LOC.
- `tests/skill_tree_loads.rs` — NEW integration test (mirror of `tests/class_table_loads.rs`). ~+90 LOC.

**Types introduced in Phase 2:** `NodeId`, `NodeGrant`, `SkillNode`, `SkillTree`, `CycleError`, `KnownSpells`, `UnlockedNodes`, `WarnedMissingSpells`, `SkillError`.
**Constants introduced in Phase 2:** `MAX_SKILL_TREE_NODES`, `MAX_SKILL_NODE_COST`, `MAX_SKILL_NODE_MIN_LEVEL`, `SKILL_POINTS_PER_LEVEL`.

**Test counts at Phase 2 completion:** 8 (`data/skills.rs`) + 8 (`party/skills.rs`) + 5 (`guild_skills.rs` integration) + 2 (`guild_skills.rs` `node_state` unit tests — Cat-C-1 4-state painter) + 1 (extended progression test for SP award) + 1 (`tests/skill_tree_loads.rs`) = **25 new tests** in Phase 2. (Plus Phase 1's 19 still passing.)

**Δ Cargo.toml = 0** at Phase 2.

**Phase 2 verification gate (must pass before merging Phase 2):**

- [ ] All Phase 1 gates still pass (run all tests including Phase 1's)
- [ ] `cargo test --lib skills` — 16 new tests
- [ ] `cargo test --lib guild_skills` — 5 new integration tests + 2 `node_state` pure-fn unit tests = 7 total
- [ ] `cargo test --test skill_tree_loads` — RON load smoke + DAG validation
- [ ] Anti-pattern grep on the new files (skills.rs × 2, guild_skills.rs): zero matches for `derive(Event)`, `EventReader<`, `EventWriter<`
- [ ] Sole-mutator grep: zero matches for `effects.push|effects.retain` in the new files
- [ ] Manual smoke: `cargo run --features dev` — no panic loading any of the 3 skill-tree RONs; Guild Roster footer shows `"[ ] Skills"` keybind hint; pressing `[` enters Skills mode; cursor + Confirm spend a point on a root node and decrements `unspent_skill_points`

**Phase 2 is partially user-facing** — Guild Skills mode is fully functional (you can spend points and watch nodes light up); combat still can't cast spells because Phase 3 hasn't shipped. KnownSpells populates correctly when an unlock is `LearnSpell`, but the combat menu still says "Not implemented." Document this in the PR body.

**Stacked-PR protocol (Phase 2 ship)** — Phase 2 is shipped as a **stacked PR on top of Phase 1**. Until Phase 1's PR #21 merges, Phase 2's PR targets `feature-20a-spell-registry` as its base; when #21 merges to `main`, GitHub auto-retargets Phase 2 to `main`. Concrete rules:

- **Branch from `feature-20a-spell-registry`, NOT from `main`.** `but branch new feature-20b-skill-trees` MUST be run while the working tree is on the `feature-20a-spell-registry` state. Verify with `but status` BEFORE creating the branch — `feature-20a-spell-registry` must be the only branch with applied commits in the workspace. If any other branch shows applied commits, stop and re-check; do NOT create the Phase 2 branch from a polluted state.
- **PR creation:** `gh pr create --base feature-20a-spell-registry --head feature-20b-skill-trees --title "feat(party): add skill trees, skill points, and Guild Skills mode (#20b)" --body-file <path>`.
- **Auto-retarget on Phase 1 merge:** when PR #21 merges into `main`, GitHub automatically retargets PR #20b's base from `feature-20a-spell-registry` to `main`. No manual action required.
- **Rebase discipline:** if Phase 1 receives further fixup commits before merging, Phase 2's branch must be rebased onto the updated `feature-20a-spell-registry` tip before pushing. The shipper agent enforces this.
- **Phase 3 stacking — TBD.** Default policy: stack Phase 3 on Phase 2 (`--base feature-20b-skill-trees`). The orchestrator MUST confirm this with the user before shipping Phase 3 — if the user prefers Phase 3 to wait for Phase 2 to merge first, the branch is created from `main` after merge instead.

### Phase 3 — Combat UI: functional SpellMenu

**Branch:** `feature-20c-spell-menu`
**PR title:** `feat(combat): functional spell menu and end-to-end casting (#20c)`
**Scope line for PR body:** Replace the `MenuFrame::SpellMenu` stub at `ui_combat.rs:457-473` with a functional two-pane menu. Dev party gets default `KnownSpells` so the smoke test is "open Spell submenu, see Halito, cast it."

**Files in this phase (corresponds to Steps 2.6 + 2.7 below, REGROUPED):**

- `src/plugins/combat/ui_combat.rs` — REPLACE SpellMenu stub at lines 457-473 with real two-pane menu; add `spell_cursor` handling; thread `SpellDb` + `DungeonAssets` + `KnownSpells` queries + `WarnedMissingSpells`. ~+150 modified LOC.
- `src/plugins/combat/turn_manager.rs` — add `spell_cursor: usize` field to `PlayerInputState`. ~+1 modified LOC.
- `src/plugins/party/mod.rs` — extend `spawn_default_debug_party` (dev-feature-gated) so Mira knows `["halito", "katino"]` and Father Gren knows `["dios", "matu"]`. ~+12 modified LOC.

**Types introduced in Phase 3:** None (only struct-field additions).
**Constants introduced in Phase 3:** None.

**Test counts at Phase 3 completion:** 1 extended (`silence_blocks_spell_menu` extension in `ui_combat.rs`) = **+1 test** (mostly already-existing). The end-to-end behavior is mostly manual-smoke verified — the unit infrastructure was front-loaded into Phase 1's `cast_spell_*` tests.

**Δ Cargo.toml = 0** at Phase 3.

**Phase 3 verification gate (must pass before merging Phase 3):**

- [ ] All Phase 1+2 gates still pass
- [ ] `cargo test --lib spell` and `cargo test --lib skills` — full coverage runs
- [ ] Clippy clean on all `--features` combinations
- [ ] Manual end-to-end smoke (the **showcase test** for #20):
  - `cargo run --features dev` — F9 to Dungeon, walk into encounter
  - Cursor to Mira; select Spell; confirm Halito + Katino listed with MP cost
  - Pick Halito; target enemy; confirm damage in combat log
  - Cursor to Father Gren; pick Dios; target a wounded ally; confirm HP increase
  - Drain caster's MP; confirm "X lacks MP" log
  - Inflict Silence on a caster; confirm Spell submenu auto-pops with "(silenced; cannot cast)"
  - Win combat for level-up; F9 to Town → Guild; confirm `unspent_skill_points += 1`
  - From Guild Roster press `[`; pick Mira; unlock `mage_combat_2` (after `mage_combat_1`); confirm `mahalito` added to KnownSpells
  - Return to combat; confirm Mahalito now appears in Mira's spell list
- [ ] DAG validation smoke (Phase 2 carryover): introduce a cycle in mage.skills.ron, confirm `error!` log + empty tree, then revert

**Phase 3 is the user-facing payoff.** Reviewer should expect a small diff focused on UI plumbing.

**Stacked-PR protocol (Phase 3 ship)** — Phase 3 is shipped as a **stacked PR on top of Phase 2** (CONFIRMED post-Phase-2 review, 2026-05-14). Until Phase 2's PR #23 merges, Phase 3's PR targets `feature-20b-skill-trees` as its base; when #23 merges to `main` (or to its own base if Phase 1 hasn't merged yet), GitHub auto-retargets Phase 3's base. Three-PR stack at ship time: #21 ← #23 ← #(Phase 3). Concrete rules:

- **Branch from `feature-20b-skill-trees`, NOT from `main` and NOT from `feature-20a-spell-registry`.** Use `but branch new feature-20c-spell-menu --anchor feature-20b-skill-trees` to create the branch stacked on Phase 2's tip. The `--anchor` flag is REQUIRED for stacked branches in the current `but` version.
- **GitButler stacked-branch creation:** `but commit <new-branch-name>` does NOT auto-create branches in the current `but` version — it errors with `Branch '<name>' not found`. The CLAUDE.md guidance saying "creates a NEW branch with that name and route the commit there" is OUTDATED. You MUST run `but branch new feature-20c-spell-menu --anchor feature-20b-skill-trees` BEFORE staging or committing. This was the friction during Phase 2 ship; do not repeat.
- **Verify with `but status` BEFORE creating the branch** — `feature-20a-spell-registry` AND `feature-20b-skill-trees` should both show applied commits in the stack; no other branches should have applied commits. If a stray branch shows applied commits, stop and re-check; do NOT create the Phase 3 branch from a polluted state.
- **PR creation:** `gh pr create --base feature-20b-skill-trees --head feature-20c-spell-menu --title "feat(combat): functional spell menu and end-to-end casting (#20c)" --body-file <path>`.
- **Auto-retarget on Phase 2 merge:** when PR #23 merges, GitHub automatically retargets PR #(Phase 3)'s base from `feature-20b-skill-trees` to whatever PR #23 was targeting (likely `main`, or `feature-20a-spell-registry` if Phase 1 hasn't merged yet). No manual action required for the auto-retarget itself.
- **Rebase discipline (Phase 1 fixups):** if PR #21 receives further fixup commits before merging, BOTH Phase 2 and Phase 3 branches must be rebased onto the updated `feature-20a-spell-registry` tip. Procedure: `git fetch origin` → `but status` (verify clean stack) → rebase `feature-20b-skill-trees` onto updated `feature-20a-spell-registry` tip → rebase `feature-20c-spell-menu` onto updated `feature-20b-skill-trees` tip → re-run all gates (`cargo check`, `cargo test --lib`, `cargo clippy --all-targets -- -D warnings`) → `btp feature-20b-skill-trees` AND `btp feature-20c-spell-menu`.
- **Rebase discipline (Phase 2 fixups):** if PR #23 receives further fixup commits before merging, Phase 3's branch must be rebased onto the updated `feature-20b-skill-trees` tip. Procedure: `git fetch origin` → `but status` (verify clean stack) → rebase `feature-20c-spell-menu` onto updated `feature-20b-skill-trees` tip → re-run gates → `btp feature-20c-spell-menu`.
- **No fourth phase.** Phase 3 is the terminal phase for #20. Subsequent polish work (spell icons #25, spell-sim debug, etc.) ships in its own PR not stacked on this chain.

### Phase orchestration policy

The orchestrator runs Phase 1 → review → ship → PAUSE (await user confirmation to proceed to Phase 2). Then Phase 2 → review → ship → PAUSE (await user confirmation). Then Phase 3 → review → ship → final report.

Between phases, the user may choose to:
- **Proceed immediately** to the next phase (preferred when CI is green and review was clean).
- **Defer** — leave the prior phase's PR open in review for stakeholder input; come back later.
- **Iterate** — apply review fixes to the prior phase before starting the next.

The orchestrator MUST surface the prior phase's PR URL + reviewer summary at every pause.

**Architecture decisions locked from research (A1 + B1 — the primary recommendation):**

- **A1 — `SpellEffect` enum-of-effects (NOT atoms, NOT special-case-Rust-per-spell).** Each spell carries a single `SpellEffect` variant: `Damage { power }` / `Heal { amount }` / `ApplyStatus { effect, potency, duration }` / `Buff { effect, potency, duration }` / `Revive { hp }` / `Special { variant }` (escape hatch). Reflect-derivable; round-trips through RON; adding a new effect category is one Rust variant + RON entries.
- **B1 — Flat node list with prerequisite IDs (NOT coordinate-grid talent matrix).** `SkillTree.nodes: Vec<SkillNode>` where each `SkillNode { id, display_name, cost, min_level, prerequisites: Vec<NodeId>, grant: NodeGrant }`. `NodeGrant = LearnSpell(SpellId) | StatBoost(BaseStats) | Resist(StatusEffectType)`. DAG validation via Kahn's-algorithm topo-sort on asset load; fail-fast with `error!` log on cycles. Up to ~64 nodes/class is plenty for v1.
- **`SpellId = String`** (NOT a newtype, NOT `Handle<SpellAsset>`) — matches `CombatActionKind::CastSpell { spell_id: String }` at `actions.rs:28` byte-for-byte; matches `ItemAsset.id`/`EnemySpec.id` precedent; serializable via serde (`Handle<T>` is NOT in Bevy 0.18, same gotcha as `Equipment` at `character.rs:204-211`).
- **`KnownSpells: Vec<SpellId>`** per-character component (extends `PartyMemberBundle` additively).
- **`SkillPoints` extension on `Experience`** as two appended `#[serde(default)] u32` fields (`unspent_skill_points`, `total_skill_points_earned`). APPEND-ONLY — discriminant order preserved for save-format stability ([[druum-feature-19-character-creation]] §6, Pitfall 2 of research).
- **`spell_damage_calc` as a SEPARATE pure fn** (not an overload of `damage_calc`) — different stat (`magic_attack` vs `attack`), different defense (`magic_defense` vs `defense`), no row-block rule. Mirrors the `damage_calc` shape: caller flattens to a `Combatant`-like input, no `Mut`/`Query`/`Res`, saturating arithmetic, seedable via `rand::Rng + ?Sized`.
- **Resolver dispatch order in `CastSpell` arm** (mirrors existing arms): pre-flight Silence check → MP check → MP deduct → resolve targets via `resolve_target_with_fallback` → dispatch on `SpellEffect` → write `ApplyStatusEvent` and/or apply HP via `derived_mut` → call `check_dead_and_apply` after every HP write → log.
- **`Revive` exception path mirrors Temple revive at `temple.rs:285-330`**: `effects.retain(|e| e.effect_type != StatusEffectType::Dead)` → `current_hp = revive_hp` → write `EquipmentChangedEvent { slot: EquipSlot::None }`. Order matters — Pitfall 4 of #18b.
- **`Buff` variant writes `ApplyStatusEvent`** through the existing #14 pipeline; the take-higher merge rule + dual-use `EquipmentChangedEvent` writer at `status_effects.rs:222-233` triggers `recompute_derived_stats_on_equipment_change` automatically.

**Critical seam discipline (carried forward from #14/#15):**

1. **`apply_status_handler` is the SOLE mutator of `StatusEffects.effects`** (frozen by #14 Decision 20). Spell resolver writes ONLY `ApplyStatusEvent`. The Revive variant is the ONLY exception that calls `effects.retain` directly, and only on `StatusEffectType::Dead`.
2. **`spell_damage_calc` is a pure function** — no `Mut<T>`, no entity lookups, no resource reads. Same shape as `damage_calc`. Variance + crit math borrowed identically.
3. **After every damage write, call `check_dead_and_apply`.** Damage and `Special` variants that touch HP must call it (Heal does not, but pairs with `min(max_hp)` cap).
4. **System ordering: `execute_combat_actions.before(apply_status_handler)`** — already locked by #15.
5. **No new sub-plugin under `CombatPlugin`.** The spell resolver is a `pub fn` consumed by `execute_combat_actions`; the spell-handle resource (`SpellDbHandle`) is registered in `LoadingPlugin`. `KnownSpells` lives in `party/` (it's a progression concern). Skill-tree handlers register under `PartyPlugin`. The Guild "Skills" mode-extension registers under `TownPlugin` (sibling to #19's creation wizard).
6. **`SpellDb::get` is O(N) linear scan** over ~15-20 spells — same shape as `ItemDb::get`, `ClassTable::get`, `EnemyDb::get`. No `HashMap`. (Bevy 0.18 removed `bevy::utils::HashMap`; `std::collections::HashMap` is fine but unnecessary at this size.)
7. **Δ Cargo.toml = 0.** `rand` is already a direct dep at `0.9` with `small_rng`/`std_rng` features; `rand_chacha 0.9` is the dev-dep. Verified at `Cargo.toml:37-40`. No new crates.

**Total scope:**

- **+4 new source files** under `src/`: `data/skills.rs` (NEW); `plugins/combat/spell_cast.rs` (NEW pure-fn helper); `plugins/party/skills.rs` (NEW — `KnownSpells`/`UnlockedNodes` components + `learn_spell`/`allocate_skill_point` pure fns + `SkillTreesAssets` resource); `plugins/town/guild_skills.rs` (NEW — Guild "Skills" mode painter + handlers, sibling to `guild_create.rs`).
- **+1 file REWRITE**: `src/data/spells.rs` (existing 11-line stub → full schema, ~+220 LOC including tests).
- **3 carve-out edits** on existing files: `src/data/mod.rs` (update re-exports), `src/plugins/loading/mod.rs` (register `SkillTree` RonAssetPlugin + add per-class handles to `DungeonAssets`), `src/plugins/party/character.rs` (append `unspent_skill_points` / `total_skill_points_earned` to `Experience`).
- **4 carve-out edits** on combat files: `combat/turn_manager.rs` (replace `CastSpell` stub at lines 531-541, add `SpellDbHandle: Resource` + signature for `spell_db` and `spell_handle`); `combat/ui_combat.rs` (replace `SpellMenu` stub at lines 457-473 with two-pane menu + handler updates); `party/mod.rs` (register `pub mod skills;`, re-export, add plugin); `party/progression.rs` (`apply_level_up_threshold_system` awards `SKILL_POINTS_PER_LEVEL` on level-up — single-line addition).
- **3 carve-out edits** on town files: `town/guild.rs` (extend `GuildMode` enum with `Skills` variant; update `mode_label` match; add `MenuAction::PrevTarget` (`[` key) entry from `Roster` to `Skills`); `town/mod.rs` (wire `paint_guild_skills` + handlers); `town/guild.rs` painter footer update.
- **+4 new RON assets**: `assets/spells/core.spells.ron` (REPLACES 4-line stub with 15-20 spells), `assets/skills/fighter.skills.ron` (NEW), `assets/skills/mage.skills.ron` (NEW), `assets/skills/priest.skills.ron` (NEW).
- **+2 new integration tests**: `tests/spell_db_loads.rs` (NEW — mirror of `tests/item_db_loads.rs`); `tests/skill_tree_loads.rs` (NEW — mirror of `tests/class_table_loads.rs`).
- **+25-30 tests** across unit + integration (see Verification — the test table lists ~28 specific tests; some may collapse during implementation). Net LOC: ~950-1150 (within the roadmap +700-1200 envelope).

## Critical

These are non-negotiable constraints. Violations should fail review.

- **Bevy `=0.18.1` pinned.** No version bump. `Cargo.toml:10` is FROZEN.
- **Δ Cargo.toml = 0.** Same precedent as #19. `rand 0.9` + `rand_chacha 0.9` already pinned; no new crates needed. If the implementer believes a new crate IS needed, STOP and re-research — the research doc verified zero new crates suffice.
- **`#[derive(Message)]`, NOT `#[derive(Event)]`** for any new buffered messages (Bevy 0.18 family rename). `MessageReader<T>` / `MessageWriter<T>` / `app.add_message::<T>()`. Verification gate greps every new `.rs` file for `derive(Event)` / `EventReader<` / `EventWriter<` — ZERO matches.
- **RON files use the DOUBLE-DOT extension** `<name>.<type>.ron` ([[druum-ron-assets-need-double-dot-extension]]). The five new/replacement RON files MUST be: `assets/spells/core.spells.ron` (already correctly named), `assets/skills/fighter.skills.ron`, `assets/skills/mage.skills.ron`, `assets/skills/priest.skills.ron`. The `RonAssetPlugin` registration uses the type-extension WITHOUT a leading dot: `RonAssetPlugin::<SkillTree>::new(&["skills.ron"])`. Single-dot won't load; round-trip unit tests don't catch this — only `cargo run` does. The Verification section MUST include a `cargo run --features dev` smoke step.
- **Existing `SpellTable` type rename: `SpellTable` → `SpellDb`.** Three edits must happen in the SAME commit: `src/data/spells.rs` (type definition); `src/data/mod.rs:30` (re-export — replace `pub use spells::SpellTable;` with `pub use spells::{SpellDb, SpellAsset, SpellEffect, SpellSchool, SpellTarget, MAX_SPELL_MP_COST, ...};`); `src/plugins/loading/mod.rs:135` (loader registration `RonAssetPlugin::<SpellDb>::new(&["spells.ron"])`). Renaming the type without updating the registration = silent loader breakage; see Pitfall 7 of research.
- **`apply_status_handler` is the SOLE mutator of `StatusEffects.effects` (frozen by #14).** Spell resolver writes ONLY `ApplyStatusEvent`. The Revive variant of `SpellEffect` is the SOLE exception: it calls `effects.retain(|e| e.effect_type != StatusEffectType::Dead)` directly, mirroring Temple at `temple.rs:285-330`. NO other `effects.push(...)` / `effects.retain(...)` in any new file.
- **`Experience` is APPEND-ONLY.** New fields go at the END (`unspent_skill_points`, `total_skill_points_earned`). Both `#[serde(default)]` so pre-#20 save data still loads. NEVER reorder existing fields (`level`, `current_xp`, `xp_to_next_level` are frozen since #11).
- **`StatusEffectType` enum is APPEND-ONLY (frozen since #14).** This plan does NOT add new variants. The `SpellEffect::ApplyStatus`/`Buff` variants take an existing `StatusEffectType`. If a spell needs a never-before-applied status (e.g., Blind, Confused), it goes through a future PR that extends `StatusEffectType` first.
- **`Class` enum filter pattern: NEVER `match Class { ... }` exhaustively without a wildcard arm** ([[druum-feature-19-character-creation]] §10). Skill-tree loader iterates `[Class::Fighter, Class::Mage, Class::Priest]` (three authored classes) explicitly; the five declared-but-unauthored classes (`Thief`/`Bishop`/`Samurai`/`Lord`/`Ninja`) get NO skill tree day-one.
- **Spell handle lives on `DungeonAssets`** as an additive `#[asset(path = "spells/core.spells.ron")] pub spells: Handle<SpellDb>` field (NOT a separate `SpellDbHandle: Resource`). Combat reads via `Res<Assets<SpellDb>>` + `Option<Res<DungeonAssets>>` (mirrors the `TownAssets.class_table` pattern from #19). Tests inject a mock `DungeonAssets` directly. The same pattern applies to skill-tree handles in Phase 3.
- **`spell_damage_calc` is PURE.** No `Mut`, no `Query`, no `Res`, no `Time`, no `Commands`. Signature: `fn spell_damage_calc(caster: &SpellCombatant, target: &SpellCombatant, spell: &SpellAsset, rng: &mut (impl rand::Rng + ?Sized)) -> SpellDamageResult`. The `SpellCombatant` flatten is the caller's responsibility. Variance + crit borrowed identically from `damage_calc`. Saturating arithmetic throughout.
- **Caller-clamp contract on HP/MP writes** (locked by #11 + reused by #19): for Heal, `current_hp = current_hp.saturating_add(amount).min(max_hp)`. For MP deduction, `current_mp = current_mp.saturating_sub(spell.mp_cost)`. For Damage, `current_hp = current_hp.saturating_sub(damage)` then `check_dead_and_apply`. For Revive, `effects.retain` → `current_hp = revive_hp` → write `EquipmentChangedEvent` (Pitfall 4 of #18b — order matters; reversing it zeros the player via the recompute's `min(new_max)` clamp).
- **DAG validation runs on asset load.** `validate_no_cycles(&SkillTree) -> Result<(), CycleError>` — Kahn's algorithm topo-sort; fail-fast `error!` log + treat tree as empty. Without this, a cyclic `prerequisites` typo silently locks all nodes (Pitfall 5 of research).
- **`SkillPoints` cap: `unspent_skill_points <= total_skill_points_earned`** as defense-in-depth against save tampering. Re-validate on every spend-point handler invocation (Architectural risk B1 in research §Security).
- **Pre-commit hook on `gitbutler/workspace` rejects raw `git commit` (CLAUDE.md).** Implementer uses `but commit --message-file <path>`. NOTE: `but commit` sweeps all unassigned hunks — see [[but-commit-sweeps-all-zz]] — so the implementer must commit-and-clear at each phase boundary if split commits are wanted, otherwise plan for one bundled commit.
- **MAX_* constants are defense-in-depth at TWO trust boundaries** (RON load + runtime mutation), matching #19's pattern.

## MAX_* constants block

Per project convention ([[druum-feature-19-character-creation]] §Critical), all trust-boundary clamps are declared as `pub const` in a single block at the top of the data module. Implementer must add to `src/data/spells.rs`:

```rust
/// Maximum allowable MP cost on a `SpellAsset.mp_cost`. Crafted RON values
/// above this clamp at consumer side. Defends against a malicious save
/// setting `mp_cost: u32::MAX` (caster could never cast).
pub const MAX_SPELL_MP_COST: u32 = 999;

/// Maximum `SpellEffect::Damage.power` and `Special` damage proxies.
/// Caps spell damage on the producer side; `spell_damage_calc` saturates.
pub const MAX_SPELL_DAMAGE: u32 = 999;

/// Maximum `SpellEffect::Heal.amount` and `SpellEffect::Revive.hp`.
pub const MAX_SPELL_HEAL: u32 = 999;

/// Maximum `SpellEffect::ApplyStatus.duration` and `SpellEffect::Buff.duration`.
/// 99 rounds is well beyond any v1 spell duration; matches level cap shape.
pub const MAX_SPELL_DURATION: u32 = 99;

/// Maximum spells per character's `KnownSpells.spells` vector. Caps crafted-save
/// `KnownSpells.spells: Vec<SpellId>` of pathological length. Truncated on
/// deserialize; matches `clamp_recruit_pool` at `data/town.rs:120-127`.
pub const KNOWN_SPELLS_MAX: usize = 64;
```

And to `src/data/skills.rs`:

```rust
/// Maximum nodes in one `SkillTree.nodes` vector. 64 nodes per class is plenty
/// for v1; future polish can raise this if balance demands.
pub const MAX_SKILL_TREE_NODES: usize = 64;

/// Maximum `SkillNode.cost` (skill points to unlock). Clamped at the
/// `can_unlock_node` use-site; defends against `cost: u32::MAX` (node would be
/// permanently unlockable).
pub const MAX_SKILL_NODE_COST: u32 = 99;

/// Skill points awarded per level-up. Single source of truth for the
/// progression hook in `apply_level_up_threshold_system`. Per user decision Q6
/// (default = 1). To switch to a per-class config, add
/// `ClassDef.skill_points_per_level: u32` as a `#[serde(default)]` additive
/// extension (future polish).
pub const SKILL_POINTS_PER_LEVEL: u32 = 1;

/// Maximum `SkillNode.min_level` (gating). Capped at the engine's level cap
/// (`progression::level_cap()` = 99); kept as a const here to avoid a
/// circular import.
pub const MAX_SKILL_NODE_MIN_LEVEL: u32 = 99;
```

**Trust boundary application (matches #19 pattern):**

| Boundary | Clamp |
|---|---|
| `SpellAsset` deserialize / `SpellDb::get` use-site | `mp_cost.min(MAX_SPELL_MP_COST)`, `Damage.power.min(MAX_SPELL_DAMAGE)`, `Heal.amount.min(MAX_SPELL_HEAL)`, `Revive.hp.min(MAX_SPELL_HEAL)`, `duration.min(MAX_SPELL_DURATION)`. |
| `KnownSpells` deserialize | `clamp_known_spells` helper truncates `spells.len()` to `KNOWN_SPELLS_MAX`. |
| `SkillTree` asset load | `validate_no_cycles(...) → Result<(), CycleError>`; `nodes.len().min(MAX_SKILL_TREE_NODES)`; `node.cost.min(MAX_SKILL_NODE_COST)`; `node.min_level.min(MAX_SKILL_NODE_MIN_LEVEL)`. |
| `can_unlock_node` runtime check | `experience.level >= node.min_level` AND `experience.unspent_skill_points >= node.cost` AND `all node.prerequisites ⊆ unlocked` AND `experience.unspent_skill_points <= experience.total_skill_points_earned` (defense-in-depth). |
| `learn_spell` mutator (Phase 2/3) | `if !known.knows(&id) && known.spells.len() < KNOWN_SPELLS_MAX { known.spells.push(id); }`. |
| Spell resolver in `execute_combat_actions` | `let spell = spell_db.get(&id)?;` clamps at use-site again: `let cost = spell.mp_cost.min(MAX_SPELL_MP_COST);` etc. Belt-and-suspenders. |

## Steps

### Phase 1 (PR #20a) — Spell registry, cast resolver, starter spells

*Branch:* `feature-20a-spell-registry` · *PR title:* `feat(combat): add spell registry and cast resolver (#20a)`
*Scope:* Steps 1.1–1.8 below. SpellMenu UI is INTENTIONALLY left as a stub — that's Phase 3.

- [x] **1.1** Replace `src/data/spells.rs` entirely (~+220 new LOC; existing 11-line stub goes away). Top-of-file doc-comment cites the plan path. Define in order:
  - `pub type SpellId = String;` — matches `ItemAsset.id`/`EnemySpec.id` precedent and the EXISTING `CombatActionKind::CastSpell { spell_id: String }` at `actions.rs:28`.
  - MAX_* constants block (see plan §MAX_* above).
  - `pub enum SpellSchool { #[default] Mage, Priest }` — `Reflect, Serialize, Deserialize, Default, Debug, Clone, Copy, PartialEq, Eq, Hash`. APPEND-ONLY for save-format stability (mirrors `Class`/`Race`/`StatusEffectType`).
  - `pub enum SpellTarget { #[default] SingleEnemy, AllEnemies, SingleAlly, AllAllies, Self_ }` — same derives. APPEND-ONLY.
  - `pub enum SpellEffect { Damage { power: u32 }, Heal { amount: u32 }, ApplyStatus { effect: StatusEffectType, potency: f32, duration: Option<u32> }, Buff { effect: StatusEffectType, potency: f32, duration: u32 }, Revive { hp: u32 }, Special { variant: String } }` — `Reflect, Serialize, Deserialize, Debug, Clone, PartialEq` (no `Eq`/`Hash` due to `f32` per `ActiveEffect` precedent at `character.rs:282`). `impl Default for SpellEffect { fn default() -> Self { Self::Damage { power: 0 } } }`.
  - `pub struct SpellAsset { pub id: SpellId, pub display_name: String, pub mp_cost: u32, pub level: u32, pub school: SpellSchool, pub target: SpellTarget, pub effect: SpellEffect, #[serde(default)] pub description: String, #[serde(default)] pub icon_path: String }` — `Reflect, Serialize, Deserialize, Default, Debug, Clone, PartialEq`.
  - `#[derive(Asset, Reflect, Serialize, Deserialize, Default, Debug, Clone, PartialEq)] pub struct SpellDb { pub spells: Vec<SpellAsset> }` with `impl SpellDb { pub fn get(&self, id: &str) -> Option<&SpellAsset> { self.spells.iter().find(|s| s.id == id) } }`.
  - `pub fn clamp_known_spells(spells: &mut Vec<SpellId>) { spells.truncate(KNOWN_SPELLS_MAX); }` — defense-in-depth helper.
  - 6+ unit tests (see Verification): `spell_asset_round_trips_through_ron`, `spell_db_get_returns_authored_spell`, `spell_db_get_returns_none_for_unknown`, `spell_effect_damage_round_trips`, `spell_effect_revive_round_trips`, `clamp_known_spells_truncates_oversized`.
  **Files:** `src/data/spells.rs` (REWRITE — net ~+210 LOC).

- [x] **1.2** Update `src/data/mod.rs` re-exports (~+2 modified LOC). Replace line 30:
  ```rust
  // OLD: pub use spells::SpellTable;
  pub use spells::{
      SpellAsset, SpellDb, SpellEffect, SpellSchool, SpellTarget,
      KNOWN_SPELLS_MAX, MAX_SPELL_DAMAGE, MAX_SPELL_DURATION,
      MAX_SPELL_HEAL, MAX_SPELL_MP_COST, clamp_known_spells,
  };
  ```
  Also update the module-level doc-comment at line 9 from "`SpellTable` (Feature #20)" to "`SpellDb`, `SpellAsset`, `SpellEffect` (Feature #20 — spells registry)".
  **Files:** `src/data/mod.rs` (+2 modified LOC, +1 modified comment).

- [x] **1.3** Update `src/plugins/loading/mod.rs` (~+8 modified LOC). Three edits, all on lines surrounding the existing `SpellTable` registration:
  - At top imports: replace `use crate::data::SpellTable;` with `use crate::data::SpellDb;`.
  - At line 135: replace `RonAssetPlugin::<SpellTable>::new(&["spells.ron"])` with `RonAssetPlugin::<SpellDb>::new(&["spells.ron"])`. **Critical:** extension stays `"spells.ron"` (NO leading dot). The existing path is already correctly double-dotted ([[druum-ron-assets-need-double-dot-extension]]).
  - In `DungeonAssets` struct, add: `#[asset(path = "spells/core.spells.ron")] pub spells: Handle<SpellDb>,` (or under `TownAssets` — choose `DungeonAssets` because spells are read by combat which is reachable from Dungeon). Add a `// Feature #20 — spell registry, replaces empty SpellTable stub` comment.
  **Files:** `src/plugins/loading/mod.rs` (+8 modified LOC).

- [x] **1.4** Author the 15-20 starter spells in `assets/spells/core.spells.ron` (~+150 new LOC; replaces 4-line stub). Use Wizardry-1 naming conventions for familiarity. Distribute as:
  - 8 Mage spells (Damage + ApplyStatus + 1 Special if needed): Halito (lvl 1 Damage 8 SingleEnemy), Mahalito (lvl 3 Damage 20 AllEnemies — i.e. group), Katino (lvl 1 ApplyStatus Sleep AllEnemies), Tiltowait (lvl 7 Damage 100 AllEnemies — boss-tier), Dilto (lvl 2 ApplyStatus Silence SingleEnemy), Mogref (lvl 1 Buff DefenseUp Self_), Sopic (lvl 2 ApplyStatus Sleep SingleEnemy), Lokara (lvl 4 ApplyStatus Paralysis SingleEnemy).
  - 7 Priest spells (Heal + Buff + Revive): Dios (lvl 1 Heal 8 SingleAlly), Madios (lvl 3 Heal 25 SingleAlly), Madi (lvl 5 Heal 99 AllAllies), Matu (lvl 2 Buff AttackUp AllAllies), Bamatu (lvl 4 Buff DefenseUp AllAllies), Kalki (lvl 2 Buff SpeedUp Self_), Di (lvl 5 Revive hp=1 SingleAlly).
  - Total: 15 spells day-one. Each entry has a `description: "..."` comment-style line in RON. `icon_path: ""` (deferred per Q10 default).
  - File header comment: `// Feature #20 — starter spell registry. 8 Mage + 7 Priest spells.`. Each spell entry is preceded by a one-line `//`-comment naming its school and tier.
  - **NOTE:** RON syntax for enum-variant struct: `effect: Damage(power: 8)` (NOT `effect: Damage { power: 8 }`); for unit variants `effect: Special(variant: "drain_mp")` etc. Verify via the `spell_asset_round_trips_through_ron` unit test BEFORE manual smoke.
  **Files:** `assets/spells/core.spells.ron` (REWRITE — net ~+150 LOC).

- [x] **1.5** Create `src/plugins/combat/spell_cast.rs` (~+200 new LOC, NEW). Top-of-file doc-comment cites plan #20 + Pattern 5 from research. Define:
  - Module imports (paralleling `damage.rs`): `bevy::prelude::*`, `rand::Rng`, types from `crate::data::*`, `crate::plugins::party::character::*`.
  - `pub struct SpellCombatant { pub name: String, pub stats: DerivedStats, pub status: StatusEffects }` — caller-flattens; no `row` needed because spells ignore rows (per planner-resolved Q5).
  - `pub struct SpellDamageResult { pub damage: u32, pub critical: bool, pub message: String }` — mirrors `DamageResult` shape minus `hit` (spells don't miss day-one; future polish can add).
  - `pub fn spell_damage_calc(caster: &SpellCombatant, target: &SpellCombatant, spell: &SpellAsset, rng: &mut (impl Rng + ?Sized)) -> SpellDamageResult`:
    - Extract `power` from `spell.effect` (return zero-damage stub for non-Damage variants).
    - `let base_power = power.min(MAX_SPELL_DAMAGE);` (consumer-side clamp).
    - `let raw = (caster.stats.magic_attack as i64 + base_power as i64 - (target.stats.magic_defense.min(180) as i64 / 2)).max(1) as u32;` (Wizardry-style: magic_attack + spell_power - magic_defense/2, floor 1).
    - Variance 0.7..=1.0 (mirror `damage.rs:121`).
    - Crit 1.5x at `caster.stats.luck / 5` % (NOT accuracy — magic crits read off luck; documented decision).
    - Return `SpellDamageResult { damage, critical, message }`.
  - `pub fn check_mp(derived: &DerivedStats, spell: &SpellAsset) -> bool { derived.current_mp >= spell.mp_cost.min(MAX_SPELL_MP_COST) }` — predicate for use in the resolver.
  - `pub fn deduct_mp(derived: &mut DerivedStats, spell: &SpellAsset) { derived.current_mp = derived.current_mp.saturating_sub(spell.mp_cost.min(MAX_SPELL_MP_COST)); }`.
  - 5+ unit tests (see Verification): `spell_damage_zero_for_non_damage_variant`, `spell_damage_seeded_deterministic`, `spell_damage_caps_at_max_spell_damage`, `check_mp_returns_false_when_insufficient`, `deduct_mp_saturates_at_zero`.
  **Files:** `src/plugins/combat/spell_cast.rs` (+200 new LOC, NEW).

- [x] **1.6** Register `pub mod spell_cast;` in `src/plugins/combat/mod.rs` (~+1 modified LOC). Add to the existing module-list block (line 6-16). NO new plugin — `spell_cast` is a `pub fn` module consumed by `turn_manager`. Order alphabetically between `damage` and `status_effects`.
  **Files:** `src/plugins/combat/mod.rs` (+1 modified LOC).

- [x] **1.7** Replace the `CastSpell` stub at `src/plugins/combat/turn_manager.rs:531-541` with the real resolver (~+130 modified LOC). Add to the system signature of `execute_combat_actions`:
  ```rust
  spell_db_assets: Res<Assets<crate::data::SpellDb>>,
  spell_handle: Option<Res<crate::plugins::loading::DungeonAssets>>,
  ```
  Then replace lines 531-541 with the body sketched in research Pattern 5 (full ~120 LOC arm). Implementation order INSIDE the arm:
  1. `let Some(spell_db) = spell_handle.and_then(|a| spell_db_assets.get(&a.spells)) else { combat_log.push("Spell database not loaded.", turn); continue; };`
  2. `let Some(spell) = spell_db.get(spell_id) else { combat_log.push(format!("{}'s spell fizzles.", name_of(action.actor)), turn); continue; };` — handles missing-spell-id case per Q9 default (silent filter + log).
  3. **Silence gate (defense-in-depth):** `if let Some(s) = chars.get(action.actor).ok().map(|t| t.1) { if is_silenced(s) { combat_log.push(format!("{} is silenced.", name_of(action.actor)), turn); continue; } }` — UI gates first, this is belt-and-suspenders.
  4. **MP check:** `if !spell_cast::check_mp(&actor_derived_snapshot, spell) { combat_log.push(format!("{} lacks MP for {}.", actor_name, spell.display_name), turn); continue; }`.
  5. **MP deduct (BEFORE effect):** `if let Ok(mut d) = derived_mut.get_mut(action.actor) { spell_cast::deduct_mp(&mut d, spell); }`.
  6. **Resolve targets:** same pattern as Attack arm via `resolve_target_with_fallback`. Map `SpellTarget` to `TargetSelection`: `SingleEnemy` → `Single(first_alive_enemy)` from the original `action.target`; `AllEnemies` → `AllEnemies`; `SingleAlly` → `Single(first_alive_ally)`; `AllAllies` → `AllAllies`; `Self_` → `Self_`. (The UI's TargetSelect already gives us a specific Entity for SingleEnemy/Ally; AllEnemies/AllAllies/Self_ are pre-mapped by the UI commit step.)
  7. **Dispatch on `&spell.effect`:**
     - `Damage { power }`: for each target, build `SpellCombatant` from snapshot (current_hp from `derived_mut`), call `spell_damage_calc`, `derived.current_hp = current_hp.saturating_sub(result.damage)`, call `check_dead_and_apply`, push log entry.
     - `Heal { amount }`: for each target, `current_hp = current_hp.saturating_add(amount.min(MAX_SPELL_HEAL)).min(max_hp)`, call `check_dead_and_apply` (defensive — heal never goes negative but consistent with the contract), push log.
     - `ApplyStatus { effect, potency, duration }`: for each target, `apply_status.write(ApplyStatusEvent { target, effect: *effect, potency: *potency, duration: *duration })`. The existing handler clamps potency. Push log.
     - `Buff { effect, potency, duration }`: identical to ApplyStatus but `duration: Some(*duration)`. Push log.
     - `Revive { hp }`: defense-in-depth — `if !target_status.has(Dead) { continue; }`. Then `status.effects.retain(|e| e.effect_type != Dead)` (the ONE exception path) → `derived.current_hp = (*hp).min(MAX_SPELL_HEAL).min(derived.max_hp)` → write `EquipmentChangedEvent { character: target, slot: EquipSlot::None }` (this triggers recompute to re-derive without the Dead status). Order matters; reversing zeros the player ([[druum-feature-18b-temple-guild]] §Critical). Push log.
     - `Special { variant }`: `match variant.as_str() { "drain_mp" => ..., _ => combat_log.push(format!("{} casts {} (unhandled variant: {}).", ...), turn); }`. v1 day-one ships NO Special handlers (escape hatch only).
  8. Final log: `combat_log.push(format!("{} casts {}.", actor_name, spell.display_name), turn);`.
  **Wiring note:** the existing `chars` / `derived_mut` / `apply_status` / `inventories` / `item_instances` queries are sufficient — the resolver only needs an additional `&StatusEffects` access via the existing `chars` query. The `equip_changed` writer for the Revive variant needs a new system param: `mut equip_changed: MessageWriter<EquipmentChangedEvent>`.
  **Files:** `src/plugins/combat/turn_manager.rs` (~+130 modified LOC; net add since stub was ~10 LOC).

- [x] **1.8** Create `tests/spell_db_loads.rs` (~+85 new LOC, NEW). Mirror `tests/item_db_loads.rs` (REFERENCE FILE). Assertions:
  - Spell count > 10 (15 expected; allow ±5 slack so the asset can grow later without breaking).
  - At least one spell with `school: SpellSchool::Mage`.
  - At least one spell with `school: SpellSchool::Priest`.
  - `halito` is in DB with `school: Mage, target: SingleEnemy` and a `SpellEffect::Damage { power: _ }` variant.
  - `dios` is in DB with `school: Priest, target: SingleAlly` and `SpellEffect::Heal { amount: _ }`.
  - `di` is in DB with `SpellEffect::Revive { hp: _ }`.
  - Timeout: panic after 30s if loader doesn't fire ([[druum-ron-assets-need-double-dot-extension]] guard).
  **Files:** `tests/spell_db_loads.rs` (+85 new LOC).

### Phase 2 (PR #20b) — Skill trees, SP allocation, Guild Skills mode (part A: party/skills + Experience)

*Branch:* `feature-20b-skill-trees` · *PR title:* `feat(party): add skill trees, skill points, and Guild Skills mode (#20b)`
*Scope:* Steps 2.1–2.5 below PLUS Steps 3.1–3.9 below.
*Phase 2 SPLIT NOTE:* Steps 2.6 (`MenuFrame::SpellMenu` replacement) and 2.7 (dev-party default `KnownSpells`) are **DEFERRED TO PHASE 3**. KnownSpells/UnlockedNodes get populated in Phase 2 (via Guild Skills unlock) but combat-side consumption is Phase 3 work. The original step numbers below keep their identity for traceability — they are simply executed in the Phase 3 PR.

**EXECUTION ORDER (compile-independent per step):**

The step *numbers* preserve traceability with the research/research-derived structure, but the *execution order* differs from the number sequence because Step 2.1's `can_unlock_node(node: &SkillNode, ...)` and Step 2.3's `PartyMemberBundle.unlocked_nodes: UnlockedNodes` signatures depend on `SkillNode` and `NodeGrant` which are defined in Step 3.1 (`src/data/skills.rs`). To keep every individual step compile-clean when the implementer commits step-by-step, run in this order:

1. **3.1** — `src/data/skills.rs` (NEW) — `SkillNode`, `NodeGrant`, `SkillTree`, `CycleError`, `validate_no_cycles`, `clamp_skill_tree`, MAX_* consts, 8 unit tests.
2. **3.2** — `src/data/mod.rs` — register `pub mod skills;` + re-exports. (Required so Step 2.1's `use crate::data::skills::SkillNode;` resolves.)
3. **2.1** — `src/plugins/party/skills.rs` (NEW) — `KnownSpells`, `UnlockedNodes`, `WarnedMissingSpells`, `SkillError`, `can_unlock_node`, `learn_spell_pure`, `allocate_skill_point_pure`, 8 unit tests.
4. **2.2** — `src/plugins/party/mod.rs` — register `pub mod skills;` + re-exports + `WarnedMissingSpells` plugin wiring.
5. **2.3** — `src/plugins/party/character.rs` — extend `PartyMemberBundle` with `KnownSpells` + `UnlockedNodes` fields.
6. **2.4** — `src/plugins/party/character.rs` — append `unspent_skill_points` + `total_skill_points_earned` to `Experience` (with `#[serde(default)]`).
7. **2.5** — `src/plugins/party/progression.rs` — declare `SKILL_POINTS_PER_LEVEL` + award SP on level-up.
8. **3.3** — Author the 3 `.skills.ron` asset files.
9. **3.4** — `src/plugins/loading/mod.rs` — register `RonAssetPlugin<SkillTree>`, add `Handle<SkillTree>` fields to `DungeonAssets`, `skill_tree_for(class)` helper, `validate_skill_trees_on_load` system.
10. **3.5** — `src/plugins/town/guild_skills.rs` (NEW) — paint/input/unlock systems + 5 integration tests.
11. **3.6** — `src/plugins/town/guild.rs` — extend `GuildMode` with `Skills` variant + `mode_label` arm + CentralPanel-skip block.
12. **3.7** — `src/plugins/town/guild.rs` — `[` keybind entry from Roster.
13. **3.8** — `src/plugins/town/mod.rs` — wire `paint_guild_skills` + handlers under `in_guild_mode(GuildMode::Skills)` run-ifs.
14. **3.9** — `tests/skill_tree_loads.rs` (NEW) — integration test for RON load + DAG validation.

Each step in this order compiles green on its own. Steps left in their original numeric position below — do NOT renumber. The implementer must follow the order above, not the textual order of the headings.

**Stacked-PR rebase discipline (Phase 2 ship):**

Phase 2 stacks on `feature-20a-spell-registry`. If Phase 1 (PR #21) receives any further fixup commits between Phase 2 branch creation and Phase 2 push, Phase 2 MUST be rebased onto the updated `feature-20a-spell-registry` tip BEFORE `but push`. Concrete steps (run from within `gitbutler/workspace`):

1. `git fetch origin` — refresh the remote's view of `feature-20a-spell-registry`.
2. `but status` — confirm `feature-20a-spell-registry` is the only branch with applied commits, and `feature-20b-skill-trees` is the active stack.
3. If `feature-20a-spell-registry`'s tip on origin differs from local: rebase Phase 2's commits onto it via GitButler's rebase flow (or `but pull` on the Phase 1 branch first, then re-apply Phase 2's commits).
4. Re-run `cargo check`, `cargo test --lib`, and `cargo clippy --all-targets -- -D warnings` after rebase.
5. Only then `but push -u origin feature-20b-skill-trees` (or `btp feature-20b-skill-trees`).
6. The PR's base on GitHub remains `feature-20a-spell-registry` — `gh pr create --base feature-20a-spell-registry --head feature-20b-skill-trees ...` (no `--base main`). GitHub auto-retargets the base to `main` when PR #21 merges; no manual action required.

Verification gates for Steps 3.1, 2.1, 2.3, 2.5, 3.4, 3.5 each include a "must compile green at this step's commit" criterion (see "Verification" section). The orchestrator's run-shipper skill is expected to validate this gate before pushing.

- [x] **2.1** Create `src/plugins/party/skills.rs` (~+150 new LOC, NEW). Top-of-file doc-comment cites plan #20. Define:
  - `pub mod skills_v1;` style imports as needed.
  - `pub struct KnownSpells { pub spells: Vec<SpellId> }` with `Component, Reflect, Serialize, Deserialize, Default, Debug, Clone, PartialEq, Eq`.
  - `impl KnownSpells { pub fn knows(&self, id: &str) -> bool { ... }; pub fn learn(&mut self, id: SpellId) { if !self.knows(&id) && self.spells.len() < KNOWN_SPELLS_MAX { self.spells.push(id); } }; pub fn forget(&mut self, id: &str) { self.spells.retain(|s| s != id); } }`. (forget is for tests + future polish; harmless to ship.)
  - `pub struct UnlockedNodes { pub nodes: Vec<NodeId> }` with same derives as `KnownSpells`.
  - `impl UnlockedNodes { pub fn has(&self, id: &str) -> bool { ... }; pub fn unlock(&mut self, id: NodeId, max_nodes: usize) { if !self.has(&id) && self.nodes.len() < max_nodes { self.nodes.push(id); } } }`.
  - `pub struct WarnedMissingSpells { pub set: std::collections::HashSet<(SpellId, Entity)> }` as a `Resource, Default, Debug` — used by the SpellMenu painter (Phase 3) to `warn!` once per missing spell ID **per character** (per Q9 default = "warn-once-per-(spell,character)-then-filter"). The `Entity` half of the tuple is the character that owns the offending `KnownSpells.spells` reference; the resource is consulted as `if warned.set.insert((id.clone(), actor_entity)) { warn!(...) }` so each `(spell,character)` pair logs exactly once per process. Import `use bevy::prelude::Entity;` at the top of `party/skills.rs`.
  - `pub fn learn_spell_pure(known: &mut KnownSpells, id: SpellId) { known.learn(id); }` — pure-fn wrapper for tests.
  - `pub fn allocate_skill_point_pure(experience: &mut Experience, node: &SkillNode) -> Result<(), SkillError>` — returns `Err(SkillError::Insufficient)` if `experience.unspent_skill_points < node.cost`, else decrements `unspent_skill_points`.
  - `pub enum SkillError { Insufficient, BelowMinLevel, MissingPrerequisite, AlreadyUnlocked, CapReached }`.
  - `pub fn can_unlock_node(node: &SkillNode, experience: &Experience, unlocked: &UnlockedNodes) -> Result<(), SkillError>` — checks: not already unlocked, `experience.unspent_skill_points >= node.cost.min(MAX_SKILL_NODE_COST)`, `experience.level >= node.min_level`, all `node.prerequisites ⊆ unlocked.nodes`, AND defense-in-depth `experience.unspent_skill_points <= experience.total_skill_points_earned`.
  - 8+ unit tests (see Verification).
  **Files:** `src/plugins/party/skills.rs` (+150 new LOC, NEW).

- [x] **2.2** Register `pub mod skills;` in `src/plugins/party/mod.rs` (~+5 modified LOC). Add after `pub mod progression;`:
  ```rust
  pub mod skills;
  ```
  And re-export:
  ```rust
  pub use skills::{
      KnownSpells, UnlockedNodes, WarnedMissingSpells, SkillError,
      allocate_skill_point_pure, can_unlock_node, learn_spell_pure,
  };
  ```
  Inside `PartyPlugin::build`:
  ```rust
  app.init_resource::<WarnedMissingSpells>()
      .register_type::<KnownSpells>()
      .register_type::<UnlockedNodes>();
  ```
  **Files:** `src/plugins/party/mod.rs` (+5 modified LOC).

- [x] **2.3** Extend `PartyMemberBundle` at `src/plugins/party/character.rs:320-333` with `pub known_spells: KnownSpells` and `pub unlocked_nodes: UnlockedNodes` (~+4 modified LOC). Both default to empty vectors. APPENDS — do not reorder existing fields. Per the bundle precedent at character.rs:320 (`#[derive(Bundle, Default)]`), the default impl is auto-derived. Add the imports: `use crate::plugins::party::skills::{KnownSpells, UnlockedNodes};` near the file's existing imports.
  **NOTE:** `PartyMemberBundle` is the "frozen by #12" spawning helper per the [[druum-feature-12-inventory-equipment]] memory; the same APPENDING pattern that #19 used for `Experience` applies here. `.insert(Inventory::default())` chains remain unchanged.
  **Files:** `src/plugins/party/character.rs` (+4 modified LOC).

- [x] **2.4** Append two fields to `Experience` at `src/plugins/party/character.rs:159-165` (~+6 modified LOC). Append AT END:
  ```rust
  // NEW for #20 — defaults to 0 so existing save data still loads.
  #[serde(default)]
  pub unspent_skill_points: u32,
  #[serde(default)]
  pub total_skill_points_earned: u32,
  ```
  Update the doc-comment above `Experience` to call out the #20 extension (~+2 lines).
  **Files:** `src/plugins/party/character.rs` (+6 modified LOC).

- [x] **2.5** Add skill-point award to `apply_level_up_threshold_system` at `src/plugins/party/progression.rs:438-471` (~+5 modified LOC).

  **Phase-ordering dependency:** this step references `SKILL_POINTS_PER_LEVEL` which lives in `src/data/skills.rs` (Step 3.1 — Phase 3). To keep Phase 2 self-compilable, declare `SKILL_POINTS_PER_LEVEL` **HERE in Phase 2** at the top of `progression.rs` as a `pub const SKILL_POINTS_PER_LEVEL: u32 = 1;`. Phase 3 Step 3.1 then re-exports it from `data/skills.rs` via `pub use crate::plugins::party::progression::SKILL_POINTS_PER_LEVEL;` (or simply lives in BOTH places with a comment cross-link). Alternative: move Step 2.5 INTO Phase 3 (after Step 3.1) — but that defeats the goal of Phase 2 unlocking playable spell-casting + per-level skill-point accumulation independently. **Decision: declare in `progression.rs` (Phase 2); Step 3.1's MAX_* block in `skills.rs` re-references via `pub use`.**

  After `exp.level = exp.level.saturating_add(1);` at line 453, insert:
  ```rust
  // Feature #20 — award skill points on level-up (Pattern 6 of research).
  exp.unspent_skill_points = exp
      .unspent_skill_points
      .saturating_add(SKILL_POINTS_PER_LEVEL);
  exp.total_skill_points_earned = exp
      .total_skill_points_earned
      .saturating_add(SKILL_POINTS_PER_LEVEL);
  ```
  At the top of `progression.rs` (above the other consts/imports), add:
  ```rust
  /// Skill points awarded per level-up. Feature #20 (Pattern 6 of research).
  /// Mirror-declared (NOT duplicated semantically) in `src/data/skills.rs::SKILL_POINTS_PER_LEVEL`
  /// via `pub use` to avoid a Phase 2 → Phase 3 forward dep.
  pub const SKILL_POINTS_PER_LEVEL: u32 = 1;
  ```
  Update `src/plugins/party/mod.rs` re-exports to include `SKILL_POINTS_PER_LEVEL` from `progression`.
  **Files:** `src/plugins/party/progression.rs` (+8 modified LOC), `src/plugins/party/mod.rs` (+1 modified LOC for re-export).

### Phase 3 (PR #20c) — Combat UI: functional SpellMenu

*Branch:* `feature-20c-spell-menu` · *PR title:* `feat(combat): functional spell menu and end-to-end casting (#20c)`
*Scope:* Steps 2.6 + 2.7 below. The Phase 1 + Phase 2 PRs must be merged to `main` first (or at minimum, both PRs' branches must be parent commits of Phase 3's branch — see "Phase orchestration policy" above).
*Phase-3 forward-dependency check (before starting):* `KnownSpells` is a real component (Phase 2 shipped it); `SpellDb` is loaded (Phase 1 shipped it); `WarnedMissingSpells` resource is init'd (Phase 2). The SpellMenu replacement is purely combat-UI plumbing.

- [x] **2.6** Replace the `SpellMenu` stub at `src/plugins/combat/ui_combat.rs:457-473` with the real two-pane menu (~+150 modified LOC). Add to `paint_combat_screen`'s system signature and the `handle_combat_input` system signature:
  ```rust
  spell_db_assets: Res<Assets<crate::data::SpellDb>>,
  dungeon_assets: Option<Res<crate::plugins::loading::DungeonAssets>>,
  known_spells_q: Query<&crate::plugins::party::KnownSpells, With<PartyMember>>,
  mut warned: ResMut<crate::plugins::party::WarnedMissingSpells>,
  ```
  Add a new field to `PlayerInputState` in `turn_manager.rs`:
  ```rust
  pub spell_cursor: usize, // index into the filtered castable list
  ```
  In `paint_combat_screen` (or its delegate), when `frame == MenuFrame::SpellMenu`:
  - Resolve `spell_db` via `dungeon_assets.and_then(|a| spell_db_assets.get(&a.spells))`. If None: paint "Spells: loading..." and return.
  - Get `known_spells = known_spells_q.get(actor_entity).ok()`. If None: paint "(no spells)" and return.
  - Build `castable: Vec<&SpellAsset> = known_spells.spells.iter().filter_map(|id| { let spell = spell_db.get(id); if spell.is_none() && warned.set.insert((id.clone(), actor_entity)) { warn!("Character {:?}'s KnownSpells references missing spell '{}' (filtered)", actor_entity, id); } spell }).filter(|s| s.mp_cost.min(MAX_SPELL_MP_COST) <= derived.current_mp).collect();`. (Per Q9 default — warn once per `(spell_id, character)` pair, filter silently.)
  - **Cat-C-4 = A (empty castable):** If `castable.is_empty()` AND `known_spells.spells.is_empty()`: paint `"(no spells)"` (existing branch for "character knows nothing"). If `castable.is_empty()` AND `!known_spells.spells.is_empty()`: paint `"(no castable spells)"` — character knows spells but none affordable (MP-short or all filtered as missing). Do NOT auto-pop in either empty case; wait for the user to press Esc. Symmetric UX with `"(no spells)"`.
  - Render a centered egui `Window` titled "Spells": cursor-highlighted list of `"{display_name} (MP {mp_cost})"`, plus the cursor entry's `description` below.
  - Footer label: `"[Up/Down] Pick  |  [Enter] Select target  |  [Esc] Back"`.
  In `handle_combat_input`'s `MenuFrame::SpellMenu` arm:
  - On `MenuNavAction::Up`/`Down`: `input_state.spell_cursor = input_state.spell_cursor.saturating_sub(1)` / `+= 1` clamped to `castable.len() - 1`. **Cat-C-6 = A: Confirmed non-wrap (saturating).** Consistent with Main menu cursor + Guild Skills cursor — do NOT add wrap-around. Saturating is the project default for combat-UI cursors.
  - On `MenuNavAction::Confirm`: guard with `if castable.is_empty() { return; }` (no-op when nothing to confirm). Then `let spell = castable[input_state.spell_cursor].clone();` (own to free borrow before mutating input_state). **Cat-C-5 = A (SingleEnemy + all enemies dead):** Before pushing `MenuFrame::TargetSelect` for `SingleEnemy`, pre-check the alive-enemy list mirroring Attack's guard pattern in `turn_manager.rs:475-478,489-492` — i.e., compute `enemy_alive = enemies.iter().filter(|e| derived.get(*e).map(|d| d.current_hp > 0).unwrap_or(false)).collect::<Vec<_>>()`. If `enemy_alive.is_empty()`: push `combat_log` with `format!("{}: no valid targets for {}", actor_name, spell.display_name)` (resolve `actor_name` via the active-slot's `Name` component, same way Attack's branch resolves it), and `return;` — stay in SpellMenu, no frame push. (Symmetric pre-check for `SingleAlly` is OPTIONAL — defer; party-wipe means combat ended.) For non-empty enemy-alive on `SingleEnemy` / non-empty ally-alive on `SingleAlly`: push `MenuFrame::TargetSelect { kind: CombatActionKind::CastSpell { spell_id: spell.id.clone() } }`. For `AllEnemies`/`AllAllies`/`Self_`, commit directly to queue (no target prompt) — same shape as Defend/Flee commit at `ui_combat.rs:373-409`. Reset `spell_cursor = 0`. Set `active_slot = None` for committed-direct case. **Δ LOC for Cat-C-5 guard: ~+4-6 LOC.**
  - On `MenuNavAction::Cancel`: pop to Main (existing logic at line 328-335 already covers this).
  - Silence gate at line 458-466 stays as-is (defense-in-depth + same-frame UX feedback).
  - Reset `input_state.spell_cursor = 0` when entering SpellMenu (in the Main arm's case 2 dispatch at line 387-390).
  **Files:** `src/plugins/combat/ui_combat.rs` (+150 modified LOC), `src/plugins/combat/turn_manager.rs` (+1 modified line for the `spell_cursor` field).

- [x] **2.7** Update `dev` party default `KnownSpells` (NOT a step in the implementer path proper — gated `#[cfg(feature = "dev")]`). In `src/plugins/party/mod.rs` `spawn_default_debug_party` (fn decl at line 117, body lines 130-171 after Phase 2 ship; the `#[cfg(feature = "dev")]` gate at line 116 stays untouched). The roster array at lines 146-151 already pairs `(name, class, row)`; chain a `.insert(...)` on the existing `commands.spawn(PartyMemberBundle { ... }).insert(Inventory::default())` block (lines 156-168) per-member by indexing on `name` or `class`. Concrete pattern (mirrors the existing `Inventory::default()` insert at line 168):
  ```rust
  let known = match *class {
      Class::Mage => KnownSpells { spells: vec!["halito".into(), "katino".into()] },
      Class::Priest => KnownSpells { spells: vec!["dios".into(), "matu".into()] },
      _ => KnownSpells::default(),
  };
  commands
      .spawn(PartyMemberBundle { /* … existing fields … */ ..Default::default() })
      .insert(Inventory::default())
      .insert(known);
  ```
  Fighters (Aldric, Borin) fall through to `KnownSpells::default()` — empty list, SpellMenu shows "(no spells)". This makes the manual smoke test "win combat, then cast a spell" possible. Without this, the dev party knows nothing and the SpellMenu always shows "(no spells)".

  **Compatibility note:** `KnownSpells` is already a field on `PartyMemberBundle` (per Phase 2 ship, `src/plugins/party/character.rs:347`); the `..Default::default()` in the existing spawn already produces an empty `KnownSpells`. The `.insert(known)` overwrite is the same pattern Phase 2's Step 2.3 documented (component overwrite via `.insert` after `.spawn`).
  **Files:** `src/plugins/party/mod.rs` (+12 modified LOC).

### Phase 2 (PR #20b) — Skill trees, SP allocation, Guild Skills mode (part B: data/skills + Guild Skills UI)

*(Continued from Phase 2 part A above.)* Steps 3.1–3.9 below belong in the same PR #20b — they implement the skill-tree data shape, per-class RON assets, loader registration, and the Guild Skills painter/handlers. Note: the original heading said "Phase 3" but under the user's three-PR split, "Phase 3" is reserved for the combat SpellMenu UI (Steps 2.6 + 2.7). The step numbers below keep their identity for traceability.

- [x] **3.1** Create `src/data/skills.rs` (~+250 new LOC, NEW). Top-of-file doc-comment cites plan #20 + Pattern 3 from research. Define in order:
  - MAX_* constants block (see plan §MAX_* above) — note: `SKILL_POINTS_PER_LEVEL` is re-exported from `progression.rs` (declared there in Phase 2 Step 2.5 to dodge a Phase 2 → Phase 3 forward dep):
    ```rust
    pub use crate::plugins::party::progression::SKILL_POINTS_PER_LEVEL;
    ```
  - `pub type NodeId = String;` — same shape as `SpellId` for consistency.
  - `pub enum NodeGrant { LearnSpell(SpellId), StatBoost(BaseStats), Resist(StatusEffectType) }` — `Reflect, Serialize, Deserialize, Debug, Clone, PartialEq` (no `Eq` because of `Resist(StatusEffectType)` — that's fine).
    - `impl Default for NodeGrant { fn default() -> Self { Self::LearnSpell(String::new()) } }`.
  - `pub struct SkillNode { pub id: NodeId, pub display_name: String, pub cost: u32, #[serde(default)] pub min_level: u32, #[serde(default)] pub prerequisites: Vec<NodeId>, pub grant: NodeGrant, #[serde(default)] pub description: String }` with same derives.
  - `#[derive(Asset, Reflect, Serialize, Deserialize, Default, Debug, Clone, PartialEq)] pub struct SkillTree { pub class_id: String, pub nodes: Vec<SkillNode> }` with `impl SkillTree { pub fn get(&self, id: &str) -> Option<&SkillNode> { ... }; pub fn root_nodes(&self) -> impl Iterator<Item = &SkillNode> { self.nodes.iter().filter(|n| n.prerequisites.is_empty()) } }`.
  - `#[derive(Debug, Clone, PartialEq, Eq)] pub enum CycleError { CycleDetected { involved: Vec<NodeId> }, UnknownPrerequisite { node: NodeId, prereq: NodeId } }`.
  - `pub fn validate_no_cycles(tree: &SkillTree) -> Result<(), CycleError>` — Kahn's algorithm topo-sort. Pseudo:
    ```rust
    pub fn validate_no_cycles(tree: &SkillTree) -> Result<(), CycleError> {
        // Build adjacency + in-degree map.
        let known: HashSet<&str> = tree.nodes.iter().map(|n| n.id.as_str()).collect();
        let mut in_degree: HashMap<&str, usize> = HashMap::new();
        for n in &tree.nodes { in_degree.insert(n.id.as_str(), n.prerequisites.len()); }
        // Validate prerequisites reference real nodes.
        for n in &tree.nodes {
            for p in &n.prerequisites {
                if !known.contains(p.as_str()) {
                    return Err(CycleError::UnknownPrerequisite { node: n.id.clone(), prereq: p.clone() });
                }
            }
        }
        // Kahn's: peel off zero-in-degree nodes one by one.
        let mut queue: VecDeque<&str> = in_degree.iter().filter_map(|(k, v)| if *v == 0 { Some(*k) } else { None }).collect();
        let mut visited = 0usize;
        while let Some(n) = queue.pop_front() {
            visited += 1;
            for other in &tree.nodes {
                if other.prerequisites.iter().any(|p| p == n) {
                    let d = in_degree.get_mut(other.id.as_str()).unwrap();
                    *d -= 1;
                    if *d == 0 { queue.push_back(other.id.as_str()); }
                }
            }
        }
        if visited != tree.nodes.len() {
            let involved = tree.nodes.iter().filter(|n| in_degree.get(n.id.as_str()).copied().unwrap_or(0) > 0).map(|n| n.id.clone()).collect();
            return Err(CycleError::CycleDetected { involved });
        }
        Ok(())
    }
    ```
  - `pub fn clamp_skill_tree(tree: &mut SkillTree)` — truncates `nodes.len()` to `MAX_SKILL_TREE_NODES`; clamps `node.cost` to `MAX_SKILL_NODE_COST`; clamps `node.min_level` to `MAX_SKILL_NODE_MIN_LEVEL`.
  - 7+ unit tests (see Verification): `skill_tree_round_trips_through_ron`, `validate_no_cycles_accepts_linear`, `validate_no_cycles_rejects_self_loop`, `validate_no_cycles_rejects_two_node_cycle`, `validate_no_cycles_rejects_three_node_cycle`, `validate_no_cycles_rejects_unknown_prereq`, `clamp_skill_tree_truncates_oversized`, `clamp_skill_tree_caps_per_node_cost`.
  **Files:** `src/data/skills.rs` (+250 new LOC, NEW).

- [x] **3.2** Update `src/data/mod.rs` to register the new module (~+4 modified LOC). Add `pub mod skills;` after `pub mod races;`. Add re-export:
  ```rust
  pub use skills::{
      CycleError, MAX_SKILL_NODE_COST, MAX_SKILL_NODE_MIN_LEVEL, MAX_SKILL_TREE_NODES,
      NodeGrant, NodeId, SKILL_POINTS_PER_LEVEL, SkillNode, SkillTree,
      clamp_skill_tree, validate_no_cycles,
  };
  ```
  Order alphabetically (between `races` and `spells`). Update the module-doc-comment to list `skills` line: `- skills — SkillTree, SkillNode, NodeGrant (Feature #20)`.
  **Files:** `src/data/mod.rs` (+4 modified LOC).

- [x] **3.3** Author the 3 per-class skill trees as new RON files (~+200 new LOC across 3 files). All double-dotted per [[druum-ron-assets-need-double-dot-extension]].
  - `assets/skills/fighter.skills.ron` (~6 nodes, passives + Resists ONLY; per planner-resolved Q1 default = all 3 classes). Sample shape:
    ```ron
    (
        class_id: "Fighter",
        nodes: [
            (id: "fighter_might",      display_name: "Might",       cost: 1, min_level: 1, prerequisites: [],            grant: StatBoost((strength: 2, vitality: 0, intelligence: 0, piety: 0, agility: 0, luck: 0)), description: "+2 STR."),
            (id: "fighter_endurance",  display_name: "Endurance",   cost: 1, min_level: 1, prerequisites: [],            grant: StatBoost((vitality: 2, strength: 0, intelligence: 0, piety: 0, agility: 0, luck: 0)), description: "+2 VIT."),
            (id: "fighter_resist_poison", display_name: "Iron Gut", cost: 1, min_level: 3, prerequisites: ["fighter_endurance"], grant: Resist(Poison), description: "Resist Poison."),
            (id: "fighter_resist_sleep",  display_name: "Vigilance", cost: 1, min_level: 3, prerequisites: ["fighter_endurance"], grant: Resist(Sleep), description: "Resist Sleep."),
            (id: "fighter_might_2",       display_name: "Might II",  cost: 2, min_level: 5, prerequisites: ["fighter_might"],     grant: StatBoost((strength: 3, vitality: 0, intelligence: 0, piety: 0, agility: 0, luck: 0)), description: "+3 STR."),
            (id: "fighter_resist_paralysis", display_name: "Steel Nerve", cost: 2, min_level: 7, prerequisites: ["fighter_resist_sleep"], grant: Resist(Paralysis), description: "Resist Paralysis."),
        ],
    )
    ```
  - `assets/skills/mage.skills.ron` (~8 nodes, mix of `LearnSpell` + `StatBoost`):
    Nodes: `mage_combat_1` (LearnSpell halito, lvl 1, no prereq); `mage_control_1` (LearnSpell katino, lvl 1, no prereq); `mage_arcane_mind` (StatBoost INT+2, lvl 1, no prereq); `mage_combat_2` (LearnSpell mahalito, lvl 3, prereq mage_combat_1); `mage_control_2` (LearnSpell dilto, lvl 3, prereq mage_control_1); `mage_buff_self` (LearnSpell mogref, lvl 3, prereq mage_arcane_mind); `mage_combat_3` (LearnSpell tiltowait, lvl 7, prereq mage_combat_2); `mage_paralysis` (LearnSpell lokara, lvl 5, prereq mage_control_2).
  - `assets/skills/priest.skills.ron` (~8 nodes):
    Nodes: `priest_heal_1` (LearnSpell dios, lvl 1, no prereq); `priest_buff_atk` (LearnSpell matu, lvl 1, no prereq); `priest_devotion` (StatBoost PIE+2, lvl 1, no prereq); `priest_heal_2` (LearnSpell madios, lvl 3, prereq priest_heal_1); `priest_buff_def` (LearnSpell bamatu, lvl 3, prereq priest_buff_atk); `priest_buff_spd` (LearnSpell kalki, lvl 3, prereq priest_devotion); `priest_revive` (LearnSpell di, lvl 5, prereq priest_heal_2); `priest_heal_3` (LearnSpell madi, lvl 5, prereq priest_heal_2).
  Each file starts with a `// Feature #20 — <class> skill tree; <N> nodes day-one.` comment.
  **Files:** `assets/skills/fighter.skills.ron` (~50 LOC NEW), `assets/skills/mage.skills.ron` (~80 LOC NEW), `assets/skills/priest.skills.ron` (~80 LOC NEW).

- [x] **3.4** Register the skill-tree loader + handles in `src/plugins/loading/mod.rs` (~+12 modified LOC). Add:
  - Import: `use crate::data::SkillTree;`.
  - To `.add_plugins(...)` tuple at line 130-141: `RonAssetPlugin::<SkillTree>::new(&["skills.ron"])`. Order after the `RaceTable` line, with a comment `// Feature #20 — per-class skill trees`.
  - To `DungeonAssets` struct (somewhere around line 100): three new asset-collection fields:
    ```rust
    #[asset(path = "skills/fighter.skills.ron")] pub fighter_skills: Handle<SkillTree>,
    #[asset(path = "skills/mage.skills.ron")]    pub mage_skills:    Handle<SkillTree>,
    #[asset(path = "skills/priest.skills.ron")]  pub priest_skills:  Handle<SkillTree>,
    ```
  - Add a `pub fn skill_tree_for(&self, class: crate::plugins::party::Class) -> Option<&Handle<SkillTree>>` helper on `DungeonAssets`:
    ```rust
    impl DungeonAssets {
        pub fn skill_tree_for(&self, class: crate::plugins::party::Class) -> Option<&Handle<SkillTree>> {
            match class {
                crate::plugins::party::Class::Fighter => Some(&self.fighter_skills),
                crate::plugins::party::Class::Mage => Some(&self.mage_skills),
                crate::plugins::party::Class::Priest => Some(&self.priest_skills),
                _ => None,
            }
        }
    }
    ```
    **NEVER `match Class { ... }` exhaustively without the `_ =>` wildcard** ([[druum-feature-19-character-creation]] §10).
  - **Asset-load-time validation:** `add_systems(OnExit(GameState::Loading), validate_skill_trees_on_load)`. The system reads each loaded tree via the handle and runs `validate_no_cycles + clamp_skill_tree`; logs at `error!` on cycle. (~+30 LOC for the system.) Failed trees are kept but emptied — the Guild paint shows "(skill tree unavailable: cycle detected)".

    **Cat-C-3 decision (locked): validator scope is CYCLES + CLAMP ONLY.** The validator does NOT walk `NodeGrant::LearnSpell(SpellId)` references against `SpellDb` at load time. Reasons:

    1. **Single source of truth for missing-spell handling.** Bogus `SpellId` strings in `NodeGrant::LearnSpell` flow through `handle_guild_skills_unlock` into the character's `KnownSpells` component without complaint. They surface at the natural point of consumption — the Phase 3 `SpellMenu` painter — where the `WarnedMissingSpells: HashSet<(SpellId, Entity)>` resource warn-once-per-`(spell, character)` mechanism handles them per Q9. Adding a load-time check would create a second warning site with different semantics (per-asset-id vs per-character-instance), and asset hot-reload would muddle which copy is canonical.
    2. **Authoring-error visibility is preserved.** A cycle is a structural error (the tree cannot be traversed at all); a bogus `LearnSpell("typo")` is a content error (the tree is fine; the unlock just grants a no-op spell). The structural error must fail-fast because the painter would otherwise loop; the content error has a graceful degradation path (warn-once + filter).
    3. **Asset hot-reload friendliness.** If a designer renames `halito` → `dagger_throw_fire` mid-session, the load-time validator would crash on the cycle check pass; the Phase 3 painter just shows the renamed spell with a one-shot warning on first reference per character.

    **Concretely:** `validate_skill_trees_on_load` runs *only* `validate_no_cycles` (DAG correctness via Kahn's) + `clamp_skill_tree` (size caps). It does NOT iterate `nodes.iter().filter_map(|n| match &n.grant { NodeGrant::LearnSpell(id) => Some(id), _ => None })` and check against `SpellDb`. The Phase 3 SpellMenu painter (Step 2.6) IS responsible for the warn-once filter on `KnownSpells` references — that's where the user-facing degradation lands.
  **Files:** `src/plugins/loading/mod.rs` (+12 modified LOC for handles + ~+30 LOC for the validation system).

- [x] **3.5** Create `src/plugins/town/guild_skills.rs` (~+270 new LOC, NEW — increased from +250 to accommodate Cat-C-1 4-state painter's `node_state` pure fn + 2 unit tests). Top-of-file doc-comment cites plan #20 + the Guild_create sibling-split pattern from #19. Define:
  - Imports paralleling `guild_create.rs`.
  - `pub fn paint_guild_skills(...) -> Result` (read-only) — gated `GuildMode::Skills`. Layout:
    - **Top header:** "Guild — Skill Trees" + `{gold.0} Gold` right (per Guild's existing header pattern).
    - **Left panel:** active party roster (sorted by `(PartySlot, Entity)`); cursor-highlighted; shows `name (Lv{level}, {unspent}/{total} SP)`.
    - **Right panel:** for the cursor-selected party member's class, render the per-class `SkillTree`. Sort nodes by `(prereq_depth, id)` where prereq_depth = depth-from-root in DAG (computed via a closed-form pure fn `node_depth(tree, node) -> u32` that iterates prerequisites' `node_depth + 1` recursively, memoised in a local `HashMap`). Visually:
      ```
      [✓] Might          Lv1 1SP — +2 STR                                           (unlocked)
      [✓] Endurance      Lv1 1SP — +2 VIT                                           (unlocked)
      [ ] Iron Gut       Lv3 1SP — Resist Poison    (req: Endurance) — cursor here  (can unlock now)
      [ ] Vigilance      Lv3 1SP — Resist Sleep     (req: Endurance)                (need 1 more SP)  ← yellow
      [ ] Steel Nerve    Lv7 2SP — Resist Paralysis (req: Vigilance)                (locked: prereq)
      ```
      where `[✓]` = unlocked (in `UnlockedNodes`), `[ ]` = locked.

      **Cat-C-1 decision (locked):** Use a **4-state palette** to distinguish "can-unlock-now" from "SP-insufficient but otherwise ready". This helps SP-save planning — a user looking at the tree can tell at a glance which nodes are reachable by saving the next few level-ups vs which are gated by prerequisites or class-level. The four states are determined by inspecting `can_unlock_node`'s `SkillError` return:

      | State | Visual | Condition | `can_unlock_node` return |
      |---|---|---|---|
      | **Unlocked** | `egui::Color32::from_rgb(120, 200, 120)` (soft green fill on row) + `[✓]` checkbox | `unlocked.has(&node.id)` | n/a — skip the call |
      | **Can unlock now** | `egui::Color32::from_rgb(180, 240, 180)` (bright green text) + `[ ]` checkbox | `can_unlock_node(node, experience, unlocked).is_ok()` | `Ok(())` |
      | **SP-insufficient (otherwise met)** | `egui::Color32::from_rgb(230, 200, 100)` (warm yellow text) + `[ ]` checkbox | prereq + level met but `experience.unspent_skill_points < node.cost` | `Err(SkillError::Insufficient)` |
      | **Locked** | `egui::Color32::from_rgb(140, 140, 140)` (dim grey text) + `[ ]` checkbox | prereq missing, below min-level, already unlocked, or cap-reached | `Err(SkillError::{BelowMinLevel, MissingPrerequisite, AlreadyUnlocked, CapReached})` |

      Reason a column "(need N more SP)" / "(req: Endurance)" / "(need Lv5)" gloss can be derived from the same `SkillError` variant — display this in parentheses at row's end. This makes the painter pure-functional in `(node, experience, unlocked)`; no additional state required.

      **Implementation hint:** factor a private `fn node_state(node: &SkillNode, experience: &Experience, unlocked: &UnlockedNodes) -> NodeState` returning `enum NodeState { Unlocked, CanUnlock, SpInsufficient, Locked(SkillError) }`. The painter then dispatches color + gloss text on `NodeState`. This pure fn is unit-testable independent of egui — adds **2 new unit tests** (`node_state_returns_sp_insufficient_when_prereq_met_but_sp_short`, `node_state_returns_can_unlock_when_all_conditions_met`) on top of the existing 5 integration tests.

      **Test-count delta:** Step 3.5's integration-test count rises from **5 to 7** (2 new pure-fn unit tests for `node_state`). Phase 2 total: 23 → **25 new tests** (8 `data/skills.rs` + 8 `party/skills.rs` + 5 + **2 new** `guild_skills.rs` + 1 progression + 1 integration).
    - **Footer:** `"[Up/Down] Member  |  [Left/Right] Node cursor  |  [Enter] Unlock  |  [Esc] Back"`.
    - **Empty-class fallback:** if the cursor-member's class has no skill tree (e.g., Thief), display `"(No skill tree authored for {class:?})"`.
  - `pub fn handle_guild_skills_input(...)` — gated on `guild_state.mode == GuildMode::Skills`. Branches:
    - `MenuAction::Cancel` → `guild_state.mode = GuildMode::Roster`; reset cursor.
    - `MenuAction::Up`/`Down` → cycle through party members.
    - `MenuAction::Left`/`Right` → cycle through tree nodes (sorted by depth + id).
    - `MenuAction::Confirm` → call `handle_guild_skills_unlock` (separate system due to `ResMut<Assets<SkillTree>>` access).
  - `pub fn handle_guild_skills_unlock(mut commands, mut party: Query<(Entity, &Class, &mut Experience, &mut KnownSpells, &mut UnlockedNodes, &mut BaseStats), With<PartyMember>>, dungeon_assets: Option<Res<DungeonAssets>>, skill_trees: Res<Assets<SkillTree>>, mut equip_changed: MessageWriter<EquipmentChangedEvent>, mut toasts: ResMut<Toasts>, guild_state: Res<GuildState>, actions: Res<ActionState<MenuAction>>)`:
    1. Gate: `if guild_state.mode != GuildMode::Skills || !actions.just_pressed(&MenuAction::Confirm) { return; }`.
    2. Resolve cursor member entity by sorting party by `(PartySlot, Entity)` and indexing by `guild_state.cursor`.
    3. Resolve the cursor-member's class → skill tree handle via `dungeon_assets.skill_tree_for(class)`; get `&SkillTree` via `skill_trees.get(handle)`. If either is `None`, return (or toast `"No skill tree authored"`).
    4. Sort nodes by depth+id (same as painter), index by `guild_state.node_cursor` (new field on `GuildState`).
    5. Defense-in-depth: `can_unlock_node(node, &experience, &unlocked)?` — push toast on `SkillError` and return.
    6. Apply: `experience.unspent_skill_points -= node.cost;`. `unlocked.unlock(node.id.clone(), MAX_SKILL_TREE_NODES);`. Then dispatch on `node.grant`:
       - `LearnSpell(spell_id)`: `known.learn(spell_id.clone());`. Toast `"Learned {spell.display_name}!"`.
       - `StatBoost(delta)`: `base.strength = base.strength.saturating_add(delta.strength); ...` per-field. Then write `EquipmentChangedEvent { character: entity, slot: EquipSlot::None }` to trigger recompute. Toast `"Stat boost applied."`.
       - `Resist(kind)`: **planner decision (Open Q4 below):** day-one stores the resist marker in `UnlockedNodes` only — there is NO `Resists: Component` yet. The actual resist-check (reduces apply-rate or magnitude) is a **future polish** that reads `UnlockedNodes.has("fighter_resist_poison")` etc. Toast `"Resist {kind:?} unlocked."`. This is a deliberate scope-reduction to keep #20 shippable; the data shape is correct; the consumer-side check lands as a future PR. Document this in `guild_skills.rs`'s top-of-file doc-comment.
    7. Push toast on success.
  - Add a new field to `GuildState` in `guild.rs:97-103`: `pub node_cursor: usize` (~+1 modified LOC). Reset to 0 on entering `GuildMode::Skills`.
  - 3-5 integration tests (see Verification).
  **Files:** `src/plugins/town/guild_skills.rs` (+270 new LOC NEW — includes `node_state` pure fn + 2 unit tests for Cat-C-1 4-state painter), `src/plugins/town/guild.rs` (+1 modified LOC for `node_cursor` field).

- [x] **3.6** Extend `GuildMode` enum with `Skills` variant (~+3 modified LOC). At `src/plugins/town/guild.rs:104-126`, APPEND `Skills` after `CreateConfirm`. Update the `mode_label` match in `paint_guild` at line 166-175 to add `GuildMode::Skills => "Guild — Skill Trees"`. Update the painter's CentralPanel-skip block at line 186-194 to ALSO skip on `GuildMode::Skills` (since `paint_guild_skills` paints its own CentralPanel).
  **Files:** `src/plugins/town/guild.rs` (+3 modified LOC).

- [x] **3.7** Add entry-point keybind in existing `handle_guild_input` at `src/plugins/town/guild.rs:254-295` (~+8 modified LOC). When `guild_state.mode == GuildMode::Roster` AND `actions.just_pressed(&MenuAction::PrevTarget)` (the `[` key — already bound at `input/mod.rs:76`, currently used as a no-op in Roster), switch to `guild_state.mode = GuildMode::Skills`; reset `cursor = 0`, `node_cursor = 0`. Document via a comment: `// Feature #20 — '[' enters Skills mode from Roster (NextTarget=']' enters CreateRace per #19).`. Also append `"  |  [[] Skills"` to the painter's footer at line 152 (the Roster footer string).
  **Files:** `src/plugins/town/guild.rs` (+8 modified LOC).

- [x] **3.8** Register `paint_guild_skills` + handlers + plugin wiring in `src/plugins/town/mod.rs` (~+10 modified LOC). Mirror the #19 wiring pattern for `guild_create`. Add `pub mod guild_skills;` and re-export the painter/handlers. Inside `TownPlugin::build`, add to the `EguiPrimaryContextPass` tuple:
  ```rust
  paint_guild_skills.run_if(in_state(TownLocation::Guild).and(in_guild_mode(GuildMode::Skills))),
  ```
  And in the `Update` tuple:
  ```rust
  handle_guild_skills_input.run_if(in_state(TownLocation::Guild).and(in_guild_mode(GuildMode::Skills))),
  handle_guild_skills_unlock.run_if(in_state(TownLocation::Guild).and(in_guild_mode(GuildMode::Skills))),
  ```
  Also add `OnExit(TownLocation::Guild)` reset for any in-flight skill-spend cursor.
  **Files:** `src/plugins/town/mod.rs` (+10 modified LOC).

- [x] **3.9** Create `tests/skill_tree_loads.rs` (~+90 new LOC, NEW). Mirror `tests/class_table_loads.rs` (REFERENCE FILE). Load all 3 per-class trees; for each tree assert:
  - `nodes.len() > 0` (specifically: fighter ~6, mage ~8, priest ~8).
  - `class_id` matches (e.g., `"Fighter"`).
  - `validate_no_cycles(&tree).is_ok()` — fail-fast on any authoring cycle.
  - At least one root node (empty `prerequisites`).
  - At least one node with `min_level > 1`.
  - For mage/priest: at least one node with `NodeGrant::LearnSpell` referencing a spell ID that's in the SpellDb (loaded in the same test app — chain-load both `SpellDb` and `SkillTree`).
  Timeout: panic after 30s.
  **Files:** `tests/skill_tree_loads.rs` (+90 new LOC).

### Phase 4 — Documentation polish + module-doc cleanup

- [ ] **4.1** Update module-level doc-comments and the central `data/mod.rs` summary to reflect the new modules (~+10 modified LOC across 3 files):
  - `src/data/spells.rs` — top doc-comment cites `project/plans/20260514-120000-feature-20-spells-skill-tree.md`.
  - `src/data/skills.rs` — same.
  - `src/plugins/party/skills.rs` — same.
  - `src/plugins/combat/spell_cast.rs` — same.
  - `src/plugins/town/guild_skills.rs` — same; calls out the day-one Resist-marker-only behaviour (Open Q4 below).
  **Files:** module-doc updates across the 4 new files (already counted in those steps; this step is the QA check that the doc-comments cross-link correctly).

## Security

**Known vulnerabilities:** No CVEs against the recommended versions (`bevy 0.18.1`, `bevy_egui 0.39.1`, `bevy_common_assets 0.16.0`, `bevy_asset_loader 0.26.0`, `leafwing-input-manager 0.20.0`, `rand 0.9`, `rand_chacha 0.9`, `ron 0.12`) as of 2026-05-14. Feature #20 introduces zero new direct dependencies. Δ Cargo.toml = 0.

**Architectural risks (apply at the trust boundaries below):**

- **Crafted RON spell exploits** — clamp at the asset-consumer trust boundary:
  - `SpellAsset.mp_cost: u32` — clamp to `[0, MAX_SPELL_MP_COST=999]` in `spell_cast::check_mp` and `spell_cast::deduct_mp`. Without this, `mp_cost: u32::MAX` makes the spell un-castable forever (denial-of-service against the player's own spell list).
  - `SpellEffect::Damage.power: u32` — clamp to `[0, MAX_SPELL_DAMAGE=999]` in `spell_damage_calc` at the use-site. Saturating arithmetic throughout. Without this, `power: u32::MAX` one-shots any boss.
  - `SpellEffect::Heal.amount: u32` and `SpellEffect::Revive.hp: u32` — clamp to `[0, MAX_SPELL_HEAL=999]` at the dispatch site, then `min(max_hp)` cap. Saturating add. Without the cap, a `u32::MAX` heal does nothing wrong (just caps at max_hp) but the audit trail is cleaner with explicit clamping.
  - `SpellEffect::ApplyStatus.potency: f32` and `Buff.potency: f32` — clamped to `[0.0, 10.0]` by the EXISTING `apply_status_handler` at `status_effects.rs:185-189`. Spell resolver just writes the event; potency clamp is handler-side. Defense-in-depth: also reject `NaN`/`Infinity` (matches the `xp_curve_factor` pattern).
  - `SpellEffect::ApplyStatus.duration: Option<u32>` and `Buff.duration: u32` — clamp to `[0, MAX_SPELL_DURATION=99]` at the dispatch site.

- **Crafted RON skill-tree exploits** — clamp at asset load:
  - `SkillTree.nodes.len() > MAX_SKILL_TREE_NODES` — `clamp_skill_tree` truncates on load. Without this, a 1M-node tree exhausts heap and freezes the egui painter.
  - `SkillNode.cost: u32` — clamp to `[0, MAX_SKILL_NODE_COST=99]` in `can_unlock_node`. Without this, `cost: u32::MAX` makes the node permanently unlockable (player can't progress).
  - `SkillNode.min_level: u32` — clamp to `[0, MAX_SKILL_NODE_MIN_LEVEL=99]`. Same rationale.
  - **Cyclic `prerequisites: Vec<NodeId>`** — `validate_no_cycles` runs on asset load (`OnExit(Loading)`); fail-fast `error!` log + tree emptied. Without this, a typo cycle silently locks all nodes via the topo-order traversal failing.
  - **Unknown prerequisite IDs** — `validate_no_cycles` returns `UnknownPrerequisite` for any prereq not in `nodes`. Defense against authoring drift between spell IDs and tree IDs.

- **Crafted save-loaded exploits** — clamp at the runtime mutator trust boundary:
  - `Experience.unspent_skill_points: u32` — invariant `unspent_skill_points <= total_skill_points_earned` enforced in `can_unlock_node`. A crafted save with `unspent: u32::MAX` would let the player unlock the entire tree instantly; the invariant blocks this (you can only spend points you've earned).
  - `KnownSpells.spells: Vec<SpellId>` — `clamp_known_spells` truncates to `KNOWN_SPELLS_MAX=64` on deserialize. Without this, a save with a 1M-entry KnownSpells freezes the SpellMenu painter.
  - `UnlockedNodes.nodes: Vec<NodeId>` — same shape; clamp via `unlock()` method's `max_nodes` parameter (defense-in-depth — already enforced by `MAX_SKILL_TREE_NODES`).
  - **Missing-spell-ID dangling reference** — `KnownSpells` may reference a spell ID that's no longer in `SpellDb` (deprecated asset). The SpellMenu painter `warn!`-once-per-id-per-character + filters silently (per Q9 default). The resolver returns "spell fizzles" log. **Asset-load-time check (defensive future polish):** validate `unlocked.iter().all(|n| tree.get(n).is_some())` per character at `OnExit(Loading)` — deferred to #23 save-load gates because pre-#23 saves don't exist.

- **`Assets<SpellDb>` and `Assets<SkillTree>` mutation** — read-only at runtime. Only the asset loader writes; no system in #20 mutates `Assets<SpellDb>`. The skill-tree spend handler mutates `KnownSpells`/`UnlockedNodes`/`Experience`/`BaseStats` on entities, NOT the trees themselves.

**Trust boundaries summary:**

| Boundary | Validation |
|---|---|
| `core.spells.ron` load | Per-spell: `mp_cost.min(MAX_SPELL_MP_COST)`, `Damage.power.min(MAX_SPELL_DAMAGE)`, `Heal.amount.min(MAX_SPELL_HEAL)`, `Revive.hp.min(MAX_SPELL_HEAL)`, `duration.min(MAX_SPELL_DURATION)`. Defensive `is_finite()` check on `potency: f32` (reject NaN/Infinity). |
| `<class>.skills.ron` load | `validate_no_cycles` (Kahn's); `clamp_skill_tree` (truncate node count, cap per-node cost + min_level). Fail-fast on cycle: tree emptied + `error!` log. |
| `Experience.unspent_skill_points` mutation | `can_unlock_node` enforces `unspent <= total_earned` (defense-in-depth against save tamper). |
| `KnownSpells.spells` mutation | `learn` is the sole mutator; checks `!knows && spells.len() < KNOWN_SPELLS_MAX`. |
| `UnlockedNodes.nodes` mutation | `unlock` is the sole mutator; checks `!has && nodes.len() < max_nodes`. |
| Spell resolver in `execute_combat_actions` | MP gate via `spell_cast::check_mp`; Silence gate via `is_silenced`; missing-spell-ID logged + skipped (no panic). |
| `SpellEffect::Buff` dispatch | Routes through `ApplyStatusEvent` → existing `apply_status_handler` clamp (`potency.clamp(0.0, 10.0)`, NaN handled). |
| `SpellEffect::Revive` dispatch | `effects.retain(|e| e.effect_type != Dead)` → `current_hp = hp.min(MAX_SPELL_HEAL).min(max_hp)` → write `EquipmentChangedEvent`. Order matters (#18b Pitfall 4). |

## Open Questions

**ALL RESOLVED as of 2026-05-14 — see "User Decisions" section at top of plan.** No outstanding questions block implementation. Documented here for traceability:

- **Q1** — Per-class trees: **ALL three classes** (Fighter 6 passives, Mage 8 mixed, Priest 8 mixed).
- **Q3** — MP regeneration: **none new** (level-up + Inn rest only).
- **Q6** — Skill points per level-up: **1 SP/level**.
- **Q9** — Missing SpellId in KnownSpells: **warn-once-per-(spell_id, character)-then-filter**.
- **Q10** — Spell icons: **deferred to #25 polish** (day-one painter: name + MP cost only).
- **Q11** — Spell-sim debug command: **deferred to own PR**.
- **PR shape** — **THREE separate PRs** (Phase 1 / Phase 2 / Phase 3), each with its own implement → review → ship cycle. Orchestrator pauses between phases.
- **GH-issue reconciliation** — RESOLVED: no separate spec issue exists. `gh issue view 20` returns PR #20 (the merged #19). `project/roadmaps/20260429-01-bevy-dungeon-crawler-roadmap.md` lines 1079-1127 is the source of truth.

**Planner-resolved Category A items (research recommendations, no user input needed):**

- **Q2** (race-specific spells) — defer to #25. RaceData has no spell-related fields day-one.
- **Q4** (range/AoE in grid game) — `SpellTarget::SingleEnemy/AllEnemies/SingleAlly/AllAllies/Self_`. No `Group { idx }`.
- **Q5** (front/back row affects spell damage) — spells IGNORE rows; `spell_damage_calc` does NOT check `PartyRow`.
- **Q7** (reset/respec) — NO respec day-one. Temple/Guild has no "reset tree" option. Future polish.
- **Q8** (Bishop hybrid tree) — NO trees for the 5 declared-but-unauthored classes (Thief/Bishop/Samurai/Lord/Ninja). Mirrors #19.
- **Q4-resist consumer** — `NodeGrant::Resist(StatusEffectType)` ships as a DATA-only marker in `UnlockedNodes`. The status-resolver-side check that reduces apply-rate is NOT implemented in #20 — deferred to a follow-up PR.

## Implementation Discoveries

**Phase 1 discoveries (Steps 1.1–1.8):**

1. **`DerivedStats.luck` does not exist.** The plan Step 1.5 says crit chance is `caster.stats.luck / 5` but `DerivedStats` has no `luck` field — luck from `BaseStats` is folded into `accuracy` during `derive_stats`. Implementation uses `accuracy / 5` as the nearest proxy. Documented in `spell_cast.rs` doc-comment.

2. **Bevy B0002 from `CombatantCharsQuery` with `&StatusEffects` + separate `Query<&mut StatusEffects>`.** The plan Step 1.7 noted that the Revive arm needs `chars.get_mut(target)` for `StatusEffects`. Having both `Query<&StatusEffects>` (read) inside `CombatantCharsQuery` AND a separate `Query<&mut StatusEffects>` in the same system causes B0002. Fix: changed `CombatantCharsQuery` to use `&'static mut StatusEffects` throughout — `&mut` subsumes `&`, snapshot-building uses the iterator's shared-ref path, and the Revive arm uses `.get_mut()`. No separate `status_mut` query needed.

3. **Revive targets dead entities, but `resolve_target_with_fallback` filters out dead.** The resolver pre-filters targets using `is_alive_entity()`. For Revive, the intended target IS dead. Fix: the Revive arm bypasses `resolve_target_with_fallback` and reads `action.target` directly (via a match on `TargetSelection`). Defense-in-depth check `is_dead` still applied per entity.

4. **`SpellAsset` import was unused in non-test scope after implementation.** The CastSpell arm clones the `SpellAsset` from `spell_db.get()` and owns it — no need to import the type in non-test scope. Removed from the non-test import block; test helpers use the fully qualified path `crate::data::SpellAsset`.

5. **Four test fixtures used `spell_table: Handle::default()` (old field name).** When renaming `DungeonAssets.spell_table` to `.spells`, four test harnesses broke: `minimap.rs`, `dungeon/tests.rs`, `combat/encounter.rs`, `dungeon/features.rs`. All four were updated to `spells: Handle::default()`.

6. **`but commit` sweeps all unassigned hunks.** Per memory note `feedback_but_commit_sweeps_all_zz.md`: Phase 1 must be committed as ONE atomic commit via the run-shipper. Do not attempt to split into sub-commits mid-phase.

7. **Plan Step 1.7 item 8 says "Final log: cast {name}." but implementation writes the cast log BEFORE dispatching on effect (not after).** The log ordering is: "X casts Spell!" then per-target effect logs. This is intentional for game-feel (announce → resolve → report each hit). The plan's phrasing was ambiguous; "Final log" was interpreted as the pre-dispatch announcement line. No functional impact.

8. **Crit ceiling assertion in `spell_damage_caps_at_max_spell_damage` test.** The test asserts `damage <= MAX_SPELL_DAMAGE * 1.5 + 1` (≈1499). With accuracy=0, crit_chance=0, so crits never fire in that test. The `+ 1` is included as floating-point rounding guard per the existing `damage.rs` test precedent.

**Phase 2 discoveries (Steps 2.1–2.5 + 3.1–3.9):**

9. **Execution order differs from step numbering.** Steps 3.1 (data/skills.rs) and 3.2 (data/mod.rs) had to be executed before Steps 2.1 (party/skills.rs) because `SkillNode` and `NodeGrant` are defined in skills.rs and imported by party/skills.rs. The plan's "EXECUTION ORDER" note already documented this; implementer followed it.

10. **DungeonAssets landmine — 7 test fixtures required update.** Adding 3 new `Handle<SkillTree>` fields to `DungeonAssets` breaks all struct literals in tests that don't use `..Default::default()` (since `Handle<T>` is not `#[derive(Default)]` from the user side). Updated all 7 fixtures: `tests/dungeon_movement.rs`, `tests/dungeon_geometry.rs`, `src/plugins/dungeon/tests.rs`, `src/plugins/dungeon/features.rs`, `src/plugins/ui/minimap.rs`, `src/plugins/combat/encounter.rs`, `src/plugins/combat/turn_manager.rs` — each gets `fighter_skills: Handle::default(), mage_skills: Handle::default(), priest_skills: Handle::default()`.

11. **Experience struct literal breakage.** Adding `unspent_skill_points` and `total_skill_points_earned` to `Experience` broke all struct literals that didn't cover all fields. Fixed by adding `..Default::default()` to: `src/plugins/town/guild.rs:486-490` (handle_guild_recruit), `src/plugins/party/progression.rs:876-880` (xp_threshold_triggers_level_up test), `src/plugins/party/progression.rs:972-976` (level_up_awards_skill_points_per_const test).

12. **node_cursor sorted order in tests.** The `unlock_node_adds_to_unlocked_set_and_deducts_skill_point` test initially would have used `node_cursor = 0` expecting `root_node`. But `sorted_nodes` sorts by `(depth, id)`: all four depth-0 nodes sort alphabetically giving `level_gated(0), root_node(1), stat_node(2)` and `spell_node` at depth 1 goes last. Used `node_cursor = 1` for `root_node` in the test.

13. **Unused `mut exp` binding in test body.** The `unlock_node_learn_spell_grant_appends_known_spells` test had a dead code block `{ let mut exp = ...; /* comment */ }`. Removed the block; comment preserved inline.

**Phase 3 discoveries (Steps 2.6 + 2.7):**

14. **SpellMenuState enum pattern — multi-borrow avoidance.** The plan sketch implied computing the castable list inline inside the `egui::Window::show` closure. However, `mut warned: ResMut<WarnedMissingSpells>` and `known_spells_q: Query<&KnownSpells>` inside a `FnOnce` closure triggers borrow-checker issues. Restructured: compute the full display state into a local `SpellMenuState` enum OUTSIDE the closure, then pass only owned/copied data into the closure. Semantics are identical.

15. **`let...else` with non-diverging else block is invalid Rust.** An initial draft used `let Some(spell_db) = spell_db else { SpellMenuState::Loading };` which is not valid Rust — `let...else` requires the else block to diverge. Restructured to nested `match` expressions.

16. **`handle_combat_input` party query gains `&CharacterName` for Cat-C-5 log message.** The plan's Cat-C-5 requires `"{actor_name}: no valid targets for {spell}"` — this requires the actor's display name, which is only accessible by adding `&CharacterName` to the party query's fetch tuple. Updated from 4-tuple to 5-tuple; all destructuring patterns updated accordingly.

## Verification

All 6 quality gates MUST pass (zero warnings, zero failures):

- [ ] `cargo check` — Automatic
- [ ] `cargo check --features dev` — Automatic
- [ ] `cargo test` — Automatic — expected new baseline: existing-count + ~22 lib tests + ~2 integration tests
- [ ] `cargo test --features dev` — Automatic
- [ ] `cargo clippy --all-targets -- -D warnings` — Automatic
- [ ] `cargo clippy --all-targets --features dev -- -D warnings` — Automatic

**New tests (~22 across files):**

| File | Test | Type |
|---|---|---|
| `src/data/spells.rs` | `spell_asset_round_trips_through_ron` | unit |
| `src/data/spells.rs` | `spell_db_get_returns_authored_spell` | unit |
| `src/data/spells.rs` | `spell_db_get_returns_none_for_unknown` | unit |
| `src/data/spells.rs` | `spell_effect_damage_round_trips` | unit |
| `src/data/spells.rs` | `spell_effect_revive_round_trips` | unit |
| `src/data/spells.rs` | `clamp_known_spells_truncates_oversized` | unit |
| `src/data/skills.rs` | `skill_tree_round_trips_through_ron` | unit |
| `src/data/skills.rs` | `validate_no_cycles_accepts_linear` | unit |
| `src/data/skills.rs` | `validate_no_cycles_rejects_self_loop` | unit |
| `src/data/skills.rs` | `validate_no_cycles_rejects_two_node_cycle` | unit |
| `src/data/skills.rs` | `validate_no_cycles_rejects_three_node_cycle` | unit |
| `src/data/skills.rs` | `validate_no_cycles_rejects_unknown_prereq` | unit |
| `src/data/skills.rs` | `clamp_skill_tree_truncates_oversized` | unit |
| `src/data/skills.rs` | `clamp_skill_tree_caps_per_node_cost` | unit |
| `src/plugins/combat/spell_cast.rs` | `spell_damage_zero_for_non_damage_variant` | unit |
| `src/plugins/combat/spell_cast.rs` | `spell_damage_seeded_deterministic` | unit |
| `src/plugins/combat/spell_cast.rs` | `spell_damage_caps_at_max_spell_damage` | unit |
| `src/plugins/combat/spell_cast.rs` | `check_mp_returns_false_when_insufficient` | unit |
| `src/plugins/combat/spell_cast.rs` | `deduct_mp_saturates_at_zero` | unit |
| `src/plugins/party/skills.rs` | `known_spells_learn_skips_duplicates` | unit |
| `src/plugins/party/skills.rs` | `known_spells_learn_respects_max` | unit |
| `src/plugins/party/skills.rs` | `can_unlock_node_enforces_min_level` | unit |
| `src/plugins/party/skills.rs` | `can_unlock_node_enforces_prereqs` | unit |
| `src/plugins/party/skills.rs` | `can_unlock_node_enforces_skill_point_balance` | unit |
| `src/plugins/party/skills.rs` | `can_unlock_node_rejects_already_unlocked` | unit |
| `src/plugins/party/skills.rs` | `allocate_skill_point_pure_deducts_correctly` | unit |
| `src/plugins/party/skills.rs` | `allocate_skill_point_pure_rejects_insufficient` | unit |
| `src/plugins/combat/turn_manager.rs` | `cast_spell_damage_applies_hp_loss_and_dead_status` | unit (extend `app_tests`) |
| `src/plugins/combat/turn_manager.rs` | `cast_spell_heal_caps_at_max_hp` | unit |
| `src/plugins/combat/turn_manager.rs` | `cast_spell_apply_status_writes_event` | unit |
| `src/plugins/combat/turn_manager.rs` | `cast_spell_revive_restores_hp_to_1_and_clears_dead` | unit |
| `src/plugins/combat/turn_manager.rs` | `cast_spell_insufficient_mp_blocks_cast` | unit |
| `src/plugins/combat/turn_manager.rs` | `cast_spell_silenced_blocks_cast_in_resolver` | unit |
| `src/plugins/combat/turn_manager.rs` | `cast_spell_missing_id_logs_fizzle_no_panic` | unit |
| `src/plugins/combat/ui_combat.rs` | `silence_blocks_spell_menu` (EXTEND existing) | app-level |
| `src/plugins/town/guild_skills.rs` | `unlock_node_adds_to_unlocked_set_and_deducts_skill_point` | integration |
| `src/plugins/town/guild_skills.rs` | `unlock_node_learn_spell_grant_appends_known_spells` | integration |
| `src/plugins/town/guild_skills.rs` | `unlock_node_stat_boost_writes_equipment_changed_event` | integration |
| `src/plugins/town/guild_skills.rs` | `unlock_node_rejects_missing_prereq_with_toast` | integration |
| `src/plugins/town/guild_skills.rs` | `unlock_node_rejects_when_below_min_level_with_toast` | integration |
| `src/plugins/town/guild_skills.rs` | `node_state_returns_can_unlock_when_all_conditions_met` | unit (Cat-C-1) |
| `src/plugins/town/guild_skills.rs` | `node_state_returns_sp_insufficient_when_prereq_met_but_sp_short` | unit (Cat-C-1) |
| `src/plugins/party/progression.rs` | `level_up_awards_skill_points_per_const` (EXTEND existing test) | integration |
| `tests/spell_db_loads.rs` | `spell_db_loads_through_ron_asset_plugin` | integration |
| `tests/skill_tree_loads.rs` | `skill_trees_load_and_validate_no_cycles` | integration |

**Verification commands by test cluster:**

- [ ] `cargo test --lib spell` — Automatic — covers the 6 `data/spells.rs` tests, 5 `spell_cast.rs` tests, 7 `turn_manager.rs::cast_spell_*` tests, and the extended `silence_blocks_spell_menu`. ~19 tests.
- [ ] `cargo test --lib skills` — Automatic — covers the 8 `data/skills.rs` tests + 8 `party/skills.rs` tests. ~16 tests.
- [ ] `cargo test --lib guild_skills` — Automatic — covers the 5 `guild_skills.rs` integration tests + 2 `node_state` pure-fn unit tests (Cat-C-1 4-state painter).
- [ ] `cargo test --lib level_up_awards_skill_points` — Automatic — covers the extended progression test.
- [ ] `cargo test --test spell_db_loads` — Automatic — RON-asset-pipeline integration smoke.
- [ ] `cargo test --test skill_tree_loads` — Automatic — RON-asset-pipeline integration smoke + DAG validation runs in real loader.
- [ ] **Anti-pattern grep:** `! grep -rE "(derive\(Event\)|EventReader<|EventWriter<)" src/plugins/combat/spell_cast.rs src/plugins/party/skills.rs src/plugins/town/guild_skills.rs src/data/skills.rs` — Automatic — must return zero matches.
- [ ] **Sole-mutator grep:** `! grep -rE "(effects\.push|effects\.retain)" src/plugins/combat/spell_cast.rs src/plugins/party/skills.rs src/plugins/town/guild_skills.rs` — Automatic — must return zero matches. The Revive variant in `turn_manager.rs` is the SOLE permitted `effects.retain`, scoped to `StatusEffectType::Dead` only.

**Manual smoke tests (Manual — eyeballs only):**

- [ ] **RON load smoke** — `cargo run --features dev` and confirm NO panic with `"failed to load asset"` for any of `assets/spells/core.spells.ron`, `assets/skills/fighter.skills.ron`, `assets/skills/mage.skills.ron`, `assets/skills/priest.skills.ron`. **Watch out for:** RON files use the DOUBLE-DOT extension `<name>.<type>.ron`. The four files MUST be named exactly as listed above. `RonAssetPlugin::<SkillTree>::new(&["skills.ron"])` (NO leading dot). Single-dot won't load; round-trip unit tests don't catch this — only `cargo run` does. See [[druum-ron-assets-need-double-dot-extension]].
- [ ] **End-to-end spell-cast smoke** — `cargo run --features dev`, F9-cycle to Dungeon → trigger an encounter (F7 dev hotkey or walk into one) → with Mira (mage), navigate to Spell submenu (cursor pos 2), confirm spells listed (Halito, Katino), select Halito, target an enemy, confirm enemy takes damage in the combat log.
- [ ] **Heal/Revive smoke** — Get Father Gren (priest) to a fight where an ally is at low HP. Cast Dios on the ally; confirm `current_hp` increases (bounded by `max_hp`). Have an ally die via combat. Cast Di on the dead ally; confirm `Dead` cleared + `current_hp == 1`.
- [ ] **Silence gate smoke** — Inflict Silence on a party caster via a future encounter (or by manually setting it in a dev cheat). Open Spell submenu; confirm UI shows "(silenced; cannot cast)" + menu auto-pops to Main.
- [ ] **MP gate smoke** — Drain a caster's MP via repeated casting. When `current_mp < spell.mp_cost`, the spell entry shows in the list but pressing Confirm logs "X lacks MP for Y" + does not commit the action.
- [ ] **Skill-point award smoke** — Win combat to level up (e.g., kill enough enemies for total XP > `xp_to_next_level`). Return to Town → Guild → Roster; confirm party member's `unspent_skill_points` increased by 1 per level-up.
- [ ] **Skill-tree spend smoke** — From Guild Roster, press `[` to enter Skills mode. Select a party member. Cursor to a root node (e.g., `mage_combat_1` for Mira). Press Enter; confirm: skill points decremented by 1, node marked `[✓]`, `KnownSpells` gains `halito` (if not already), toast "Learned Halito!" displayed. Return to combat; confirm Halito appears in Mira's spell list.
- [ ] **DAG validation smoke** — Deliberately introduce a cycle in `mage.skills.ron` (e.g., `mage_combat_2.prerequisites: ["mage_combat_3"]` where `mage_combat_3` already requires `mage_combat_2`). Run `cargo run --features dev`; confirm `error!`-log line about cycle + the Guild Skills panel shows "(skill tree unavailable: cycle detected)" for Mage. Revert the cycle.

**Git operations — THREE PRs (use GitButler, not raw git — [[druum CLAUDE.md]]):**

Each phase ships independently as its own branch + commit + PR. Per [[but-commit-sweeps-all-zz]], `but commit` sweeps ALL unassigned hunks, so each phase MUST be a single commit. The implementer commits all Phase N files at once (no partial commits within a phase).

### Phase 1 (PR #20a) — git operations

- [ ] Create branch: `but branch new feature-20a-spell-registry`
- [ ] Stage all Phase 1 hunks: `but rub zz feature-20a-spell-registry`
- [ ] Verify only Phase 1 files are staged: `but status` — expect `src/data/spells.rs`, `src/data/mod.rs`, `src/plugins/loading/mod.rs`, `assets/spells/core.spells.ron`, `src/plugins/combat/spell_cast.rs`, `src/plugins/combat/mod.rs`, `src/plugins/combat/turn_manager.rs`, `tests/spell_db_loads.rs`. NO Phase 2/3 files.
- [ ] Commit: `but commit --message-file <path-to-msg>` (single commit; multi-line message via file).
- [ ] Push: `but push -u origin feature-20a-spell-registry`
- [ ] Open PR: `gh pr create --title "feat(combat): add spell registry and cast resolver (#20a)" --body <body-path>`

### Phase 2 (PR #20b) — git operations

(Run only after user confirms to proceed past Phase 1.)

- [ ] Create branch: `but branch new feature-20b-skill-trees`
- [ ] Stage all Phase 2 hunks: `but rub zz feature-20b-skill-trees`
- [ ] Verify staged files: `but status` — Phase 2 list from "Phase boundaries" section. NO Phase 3 files (no `ui_combat.rs` SpellMenu changes, no dev-party `KnownSpells` defaults).
- [ ] Commit: `but commit --message-file <path-to-msg>`
- [ ] Push: `but push -u origin feature-20b-skill-trees`
- [ ] Open PR: `gh pr create --title "feat(party): add skill trees, skill points, and Guild Skills mode (#20b)" --body <body-path>`

### Phase 3 (PR #20c) — git operations

(Run only after user confirms to proceed past Phase 2.)

- [ ] Create branch: `but branch new feature-20c-spell-menu`
- [ ] Stage Phase 3 hunks: `but rub zz feature-20c-spell-menu`
- [ ] Verify staged files: `but status` — expect ONLY `src/plugins/combat/ui_combat.rs`, `src/plugins/combat/turn_manager.rs` (single-field add), `src/plugins/party/mod.rs` (dev-party defaults).
- [ ] Commit: `but commit --message-file <path-to-msg>`
- [ ] Push: `but push -u origin feature-20c-spell-menu`
- [ ] Open PR: `gh pr create --title "feat(combat): functional spell menu and end-to-end casting (#20c)" --body <body-path>`
