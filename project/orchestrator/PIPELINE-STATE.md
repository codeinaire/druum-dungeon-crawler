# Pipeline State

**Task:** Druum issue #20 — Spells & Skill Trees. Three-PR pipeline (Phase 1 / Phase 2 / Phase 3), each phase = research → plan → implement → review → ship cycle. Plan updated 2026-05-14 for three-PR split.
**Status:** in-progress — Phase 1 SHIPPED + APPROVED on PR #21 (stays open for stacking); Phase 2 implementer DISPATCHED 2026-05-14
**Last Completed Step:** 2.2 (Phase 2 plan refinement) — Cat-A fixes + Cat-C decisions applied in-place to plan
**Current Phase:** Phase 2 — IMPLEMENTER DISPATCHED, awaiting completion. User authorized Phase 2 implementer dispatch; STOP at verification gate (do not ship — user runs gates + creates `feature-20b-skill-trees` branch + opens PR at ship time).

## Artifacts

| Step | Description | Artifact                                 |
| ---- | ----------- | ---------------------------------------- |
| 1    | Research    | /Users/nousunio/Repos/Learnings/claude-code/druum/project/research/20260514-druum-20-spells-skill-tree.md |
| 2    | Plan        | /Users/nousunio/Repos/Learnings/claude-code/druum/project/plans/20260514-120000-feature-20-spells-skill-tree.md (Phase 2 section: Stacked-PR protocol added 2026-05-14 ~20:00) |
| 3    | Implement (Phase 1) | /Users/nousunio/Repos/Learnings/claude-code/druum/project/implemented/20260514-120000-feature-20a-spell-registry.md |
| 4    | Ship (Phase 1) | PR: https://github.com/codeinaire/druum-dungeon-crawler/pull/21, Branch: feature-20a-spell-registry, Commits: e343585 (initial) + 5708c90 (doc-only fixup) |
| 5    | Code Review (Phase 1) | /Users/nousunio/Repos/Learnings/claude-code/druum/project/reviews/20260514-180000-feature-20a-spell-registry.md — base APPROVE; fixup addendum APPROVE, safe to merge |
| 5.1.fixup-impl | Doc-only fixup impl (Phase 1, post-review) | Applied: `turn_manager.rs:593-600` MP-check invariant comment + `status_effects.rs:319,347` TODO(#22) markers. Commit `5708c90`. |
| 5.1.fixup-ship | Fixup ship (Phase 1) | Commit `5708c90` pushed to `feature-20a-spell-registry`, live on PR #21. Gates all green (cargo check, clippy --all-targets, cargo test --lib 339/339). |
| 5.1.fixup-review | Targeted re-review of fixup | COMPLETE — addendum appended at `/Users/nousunio/Repos/Learnings/claude-code/druum/project/reviews/20260514-180000-feature-20a-spell-registry.md:105+`. Both findings RESOLVED. |
| 2.2  | Plan (Phase 2 refinement) | COMPLETE — Cat-A fixes + Cat-C-1 (4-state painter) + Cat-C-3 (cycle-only validator) applied in-place to `project/plans/20260514-120000-feature-20-spells-skill-tree.md` (2026-05-14). Test count: 23 → 25 new in Phase 2. |
| 3.2  | Implement (Phase 2, STACKED) | DISPATCHED 2026-05-14 — implementer working on `gitbutler/workspace` (`zz` unassigned); STOP at verification gate; user will create `feature-20b-skill-trees` branch FROM `feature-20a-spell-registry` at ship time |
| 4.2  | Ship (Phase 2, STACKED) | pending — `gh pr create --base feature-20a-spell-registry` (user-driven) |
| 5.2  | Code Review (Phase 2) | pending — orchestrator dispatches after user-driven ship completes |
| 3.3  | Implement (Phase 3) | pending — gated on Phase 2 user confirmation; stacking strategy TBD (likely stack on Phase 2) |
| 4.3  | Ship (Phase 3) | pending — branch `feature-20c-spell-menu` |
| 5.3  | Code Review (Phase 3) | pending |

## User Decisions

All Phase-1-locked decisions retained (see plan §"User Decisions"):

- Q1 — Per-class trees: ALL three classes (Fighter passives-only).
- Q3 — MP regeneration: none new.
- Q6 — Skill points per level-up: 1 SP/level.
- Q9 — Missing spell ID: warn-once-per-(spell,character)-then-filter.
- Q10 — Spell icons: defer to #25 polish.
- Q11 — Spell-sim debug: defer to own PR.
- PR shape: THREE separate PRs.
- GH-issue reconciliation: no separate spec issue; roadmap is source of truth.
- After Phase 1 review: PAUSE — do not auto-start Phase 2. User wants explicit confirmation between phases.

### Phase 1 post-review decisions (2026-05-14)

- **Option 1 chosen:** Address MEDIUM/LOW findings on existing branch via doc-only fixup, then proceed to Phase 2.
- **Phase 2 PR shape:** STACKED — Phase 2 branches from `feature-20a-spell-registry`, `gh pr create --base feature-20a-spell-registry`. When PR #21 merges, GitHub auto-retargets Phase 2's base to main.
- **Phase 3 stacking:** TBD — confirm with user before Phase 3 ship.
- **Issue #22:** Already filed by user for `apply_poison_damage` + `apply_regen` widening carry-forward — https://github.com/codeinaire/druum-dungeon-crawler/issues/22.
- **Re-review scope:** Narrow — verify only the two doc-fixup sites; no full re-review. COMPLETE — both resolved.

### Phase 2 plan-delta decisions (2026-05-14, post-Cat-C-resolution)

- **Cat-C-1 (painter state count):** **Option B — 4-state painter with yellow tier.** Unlocked / can-unlock-now / SP-insufficient-but-prereq+level-met (yellow) / locked. Helps SP-save planning. Pure-fn `node_state(node, experience, unlocked) -> NodeState` factored for testability. Adds 2 new unit tests.
- **Cat-C-3 (`NodeGrant::LearnSpell(SpellId)` validation scope):** **Option A — warn-and-filter at consume-time only.** `validate_skill_trees_on_load` stays structural (cycles + clamp only). Bogus spell IDs flow into `KnownSpells` and surface in Phase 3's `SpellMenu` painter via the Q9 `WarnedMissingSpells: HashSet<(SpellId, Entity)>` warn-once mechanism. Single source of truth.

### Phase 2 plan-delta Cat-A fixes (2026-05-14)

1. **`WarnedMissingSpells` key-shape fix.** `HashSet<SpellId>` → `HashSet<(SpellId, Entity)>` to match user's Q9 decision "warn-once-per-(spell, character)". Plan §Step 2.1 + §Step 2.6 (Phase 3 painter) updated.
2. **Execution-order block added at top of Phase 2 part A.** Step *numbers* preserved for traceability, but execution order is now: 3.1 → 3.2 → 2.1 → 2.2 → 2.3 → 2.4 → 2.5 → 3.3 → 3.4 → 3.5 → 3.6 → 3.7 → 3.8 → 3.9. Each step now compiles green at its own commit. Required because Step 2.1's `can_unlock_node(node: &SkillNode, ...)` and Step 2.3's `PartyMemberBundle.unlocked_nodes: UnlockedNodes` reference types defined in 3.1.
3. **Stacked-PR rebase discipline subsection added to Phase 2 part A** (adjacent to existing top-of-Phase-2 stacked-PR-protocol block). 6-step rebase procedure (`git fetch origin` → `but status` → rebase → re-run gates → `btp` → `gh pr create --base feature-20a-spell-registry`).

### Phase 2 dispatch decisions (2026-05-14, post-implementer-go-ahead)

- **User-driven ship protocol (same as Phase 1):** Implementer STOPS at the Phase 2 verification gate. User runs gates manually + creates `feature-20b-skill-trees` branch via `but branch new` + `but rub zz feature-20b-skill-trees` + `but commit` + `btp` + `gh pr create --base feature-20a-spell-registry`. Orchestrator does NOT run `run-shipper` after Phase 2 implementer completes.
- **Working tree state during Phase 2 implementation:** All changes accumulate in `zz` (unassigned) on `gitbutler/workspace`. No branch creation, no commits, no pushes. The branch is created at ship time by the user.
- **Live-test landmine briefed to implementer:** `tests/dungeon_movement.rs:146-154` and `tests/dungeon_geometry.rs:150-158` are the canonical `DungeonAssets` fixtures. When Step 3.4 adds `fighter_skills`/`mage_skills`/`priest_skills` `Handle<SkillTree>` fields to `DungeonAssets`, BOTH fixtures must be updated with `<field>: Handle::default()` lines, OR the `--test` build will fail with "missing field" errors. This is the same trap that hit Phase 1's `spell_table` → `spells` rename.

## Phase 1 ship details (2026-05-14)

- **Branch:** `feature-20a-spell-registry` from `main`
- **Initial commit:** `e343585`
- **Fixup commit:** `5708c90` (docs(combat): address review findings (#21) — MP-check invariant + TODO(#22) markers)
- **PR:** https://github.com/codeinaire/druum-dungeon-crawler/pull/21
- **Files changed (initial):** 23
- **Files changed (fixup):** 2 (`turn_manager.rs`, `status_effects.rs`) — comments-only
- **Fixup gates:** cargo check / cargo clippy --all-targets -- -D warnings / cargo test --lib 339/339 — all green
- **GitHub PR number is 21 but feature/issue number is #20 Phase 1 (the "20a" suffix). PR #20 was merged feature #19. Roadmap is source of truth.**

## Phase 1 fixup-impl details (2026-05-14)

- **Fix A (MEDIUM):** `src/plugins/combat/turn_manager.rs:593-600` — 8-line invariant comment above the MP-check block in CastSpell arm. Explains snapshot-vs-live split, one-action-per-round invariant, and migration path (`derived_mut.get(actor)`) for future double-cast mechanics.
- **Fix B (LOW):** `src/plugins/combat/status_effects.rs:319-320` and `347-348` — `// TODO(#22): widen to Or<(With<PartyMember>, With<Enemy>)> when Phase 2 adds combat-round StatusTickEvent emitter for enemies — see PR #21 review.` above each of `apply_poison_damage` and `apply_regen`.
- **Commit message file:** `/Users/nousunio/Repos/Learnings/claude-code/druum/project/shipper/feature-20a-fixup-commit-msg.txt`
- **No code changes, no new tests, no other files touched.**

## Phase 2 stacked-PR protocol (added to plan 2026-05-14 ~20:00)

Plan §Phase 2 now includes "Stacked-PR protocol" subsection at the end of the Phase 2 boundary block. Key rules:

- Branch from `feature-20a-spell-registry`, NOT from `main`
- Verify with `but status` BEFORE creating branch — Phase 1 must be the only branch with applied commits
- `gh pr create --base feature-20a-spell-registry --head feature-20b-skill-trees`
- Auto-retarget on Phase 1 merge — no manual action
- Rebase discipline: if Phase 1 receives further fixups, Phase 2 must be rebased before push
- Phase 3 stacking — TBD, confirm with user before ship (default: stack on Phase 2)

## Phase 1 implementer deviations (carry forward to Phase 2 reviewer)

1. Crit chance uses `accuracy / 5`% not `luck / 5`% — DerivedStats has no luck field.
2. `CombatantCharsQuery` uses `&'static mut StatusEffects` (B0002 prevention).
3. Revive bypasses `resolve_target_with_fallback` — that helper filters dead entities; Revive reads `action.target` directly with defense-in-depth `is_dead` check.
4. Cast announcement log fires BEFORE per-target effect logs (game-feel).
5. Four `DungeonAssets` test fixtures updated for `spell_table` → `spells` field rename.
6. **Targeted fix #1 (2026-05-14 follow-up):** `execute_combat_actions` param count exceeded Bevy's 16-tuple `SystemParam` ceiling (was 18). Three Phase-1 spell params (`spell_db_assets`, `spell_handle`, `equip_changed`) collapsed into private `#[derive(SystemParam)] struct SpellCastParams<'w>` in `turn_manager.rs`. Param count now 16 (at ceiling). Added `mut` to `chars: CombatantCharsQuery` for the Revive arm's `chars.get_mut(target)` call. (Note: user later found the actual root cause was a missing `use bevy::ecs::system::SystemParam;` import in `turn_manager.rs` — the bundle is still architecturally correct but the original cascade was the missing import.)

## User-applied follow-up fixes incorporated into commit e343585

1. Added `use bevy::ecs::system::SystemParam;` to `src/plugins/combat/turn_manager.rs:48` (real root cause of "fn isn't IntoSystemSet" cascade — the derive macro path is NOT in `bevy::prelude::*`).
2. Added `app.init_asset::<crate::data::SpellDb>();` to `src/plugins/combat/ai.rs:188`'s `make_test_app` (3 AI tests panicked because `Res<Assets<SpellDb>>` was unregistered).
3. Renamed `spell_table: Handle::default()` → `spells: Handle::default()` in `tests/dungeon_movement.rs:153` and `tests/dungeon_geometry.rs:157`.
4. Status-effect filter widening (`status_effects.rs:178-181` and `status_effects.rs:243-247`) — `Or<(With<PartyMember>, With<Enemy>)>` per Option-A.
5. `CombatantStatusQuery` type alias added to `status_effects.rs` for clippy compliance.

## Reviewer focus areas for Phase 2 (forward-looking, set by Phase 1)

When Phase 2 implementer completes, reviewer should pay particular attention to:

1. **Save-format stability** — appended `unspent_skill_points` + `total_skill_points_earned` to `Experience` must use `#[serde(default)]` (per #19 character-creation precedent). Verify no field reordering or discriminant changes.
2. **DAG validation (`validate_no_cycles`) on RON load** — Kahn's algorithm must fail-fast with `error!` log and produce an empty tree on cycle; do NOT panic. The `OnExit(GameState::Loading)` validation system is the place to run it.
3. **`KnownSpells` populates but is not yet consumed** — Phase 2 must NOT touch combat code. The SpellMenu stub remains. Verify no `turn_manager.rs` / `ui_combat.rs` modifications leak into Phase 2.
4. **Guild "Skills" mode placement** — should be a sibling to `guild_create.rs` (#19), not a special case inside `guild.rs`. Verify file separation discipline.
5. **`apply_poison_damage` + `apply_regen` widening** — does Phase 2's StatusTickEvent (if introduced for enemies) trigger #22 resolution? If yes, widen the queries. If Phase 2 does NOT introduce combat-round status ticks, the TODO(#22) markers remain valid.

## Resume instructions

### After Phase 2 implementer completes (the next user-facing checkpoint)

1. Run orchestrator self-check: file list matches plan's locked list; test-count delta = +25 (339 → 364 lib tests); Δ Cargo.toml = 0.
2. Surface implementer summary + self-check results + any deviations (with file:line refs) to user.
3. PAUSE — user runs verification gates themselves, then creates `feature-20b-skill-trees` branch + commits + pushes + opens stacked PR.
4. Do NOT call `run-shipper` skill. (Same protocol as Phase 1's user-driven ship.)
5. After user reports PR opened: capture PR URL → dispatch `run-reviewer` with Phase 2 focus areas (above).

### Phase 3 stacking — to resolve with user before Phase 3 ship

Default policy: stack Phase 3 on Phase 2 (`--base feature-20b-skill-trees`). If user prefers Phase 3 to wait for Phase 2 to merge first, branch from `main` after merge. Confirm before shipping Phase 3.
