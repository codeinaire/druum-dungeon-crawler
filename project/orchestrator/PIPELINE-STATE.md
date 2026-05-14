# Pipeline State

**Task:** Druum issue #20 — Spells & Skill Trees. Three-PR pipeline (Phase 1 / Phase 2 / Phase 3), each phase = research → plan → implement → review → ship cycle. Plan updated 2026-05-14 for three-PR split.
**Status:** in-progress — Phase 1 SHIPPED + APPROVED on PR #21; Phase 2 SHIPPED + REVIEWED (APPROVE) on PR #23 (stacked); Phase 2 LOW-fixup IMPLEMENTED (in `zz`), awaiting user ship + re-review.
**Last Completed Step:** 5.2.fixup-impl (Phase 2 LOW fixup impl) — doc-comment + tamper guard + 1 smoke test applied to `guild_skills.rs`. Test count 363 → 364.
**Current Phase:** Phase 2 — fixup ready for user to ship to existing branch `feature-20b-skill-trees`. After ship: re-reviewer dispatch, then Phase 3 planner-refresh + Cat-C pass.

## Artifacts

| Step | Description | Artifact                                 |
| ---- | ----------- | ---------------------------------------- |
| 1    | Research    | /Users/nousunio/Repos/Learnings/claude-code/druum/project/research/20260514-druum-20-spells-skill-tree.md |
| 2    | Plan        | /Users/nousunio/Repos/Learnings/claude-code/druum/project/plans/20260514-120000-feature-20-spells-skill-tree.md (Phase 2 section: Stacked-PR protocol added 2026-05-14 ~20:00; Phase 3 Stacked-PR protocol PENDING addition) |
| 3    | Implement (Phase 1) | /Users/nousunio/Repos/Learnings/claude-code/druum/project/implemented/20260514-120000-feature-20a-spell-registry.md |
| 4    | Ship (Phase 1) | PR: https://github.com/codeinaire/druum-dungeon-crawler/pull/21, Branch: feature-20a-spell-registry, Commits: e343585 (initial) + 5708c90 (doc-only fixup) |
| 5    | Code Review (Phase 1) | /Users/nousunio/Repos/Learnings/claude-code/druum/project/reviews/20260514-180000-feature-20a-spell-registry.md — base APPROVE; fixup addendum APPROVE, safe to merge |
| 5.1.fixup-impl | Doc-only fixup impl (Phase 1, post-review) | Applied: `turn_manager.rs:593-600` MP-check invariant comment + `status_effects.rs:319,347` TODO(#22) markers. Commit `5708c90`. |
| 5.1.fixup-ship | Fixup ship (Phase 1) | Commit `5708c90` pushed to `feature-20a-spell-registry`, live on PR #21. Gates all green (cargo check, clippy --all-targets, cargo test --lib 339/339). |
| 5.1.fixup-review | Targeted re-review of fixup | COMPLETE — addendum appended at `/Users/nousunio/Repos/Learnings/claude-code/druum/project/reviews/20260514-180000-feature-20a-spell-registry.md:105+`. Both findings RESOLVED. |
| 2.2  | Plan (Phase 2 refinement) | COMPLETE — Cat-A fixes + Cat-C-1 (4-state painter) + Cat-C-3 (cycle-only validator) applied in-place to `project/plans/20260514-120000-feature-20-spells-skill-tree.md` (2026-05-14). Test count: 23 → 25 new in Phase 2. |
| 3.2  | Implement (Phase 2, STACKED) | /Users/nousunio/Repos/Learnings/claude-code/druum/project/implemented/20260514-120000-feature-20b-skill-trees.md — 5 deviations documented (DungeonAssets fan-out=7 sites; node_cursor index=1; two read-only queries; mut exp; LOC inflation) |
| 4.2  | Ship (Phase 2, STACKED) | PR: https://github.com/codeinaire/druum-dungeon-crawler/pull/23, Branch: feature-20b-skill-trees, Commit: 1ec43e8 (gate-pass fixes folded in: progression.rs current_xp 250→200, unused DerivedStats, doc list restructure, two query-type aliases). Base: feature-20a-spell-registry (stacked). |
| 5.2  | Code Review (Phase 2) | /Users/nousunio/Repos/Learnings/claude-code/druum/project/reviews/20260514-200000-feature-20b-skill-trees.md — APPROVE; 0 CRITICAL/0 HIGH/0 MEDIUM/2 LOW (cosmetic: node_depth cycle-guard doc, node_state tampered-save SP-insufficient cosmetic). Posted to PR #23. |
| 5.2.fixup-impl | Phase 2 LOW fixup impl | COMPLETE — `src/plugins/town/guild_skills.rs` only file modified. Fix A: lines 160-170 `# Precondition` doc on `sorted_nodes`. Fix B: lines 114-125 `invariant_ok` tamper guard in `node_state` `Err(SkillError::Insufficient)` arm. Optional smoke test added at lines 578-591 (`node_state_returns_locked_when_invariant_violated`). Test count 363 → 364. LOC +~26. Summary: /Users/nousunio/Repos/Learnings/claude-code/druum/project/implemented/20260514-210000-feature-20b-review-fixup.md. Commit msg: /Users/nousunio/Repos/Learnings/claude-code/druum/project/shipper/feature-20b-fixup-commit-msg.txt. |
| 5.2.fixup-ship | Phase 2 LOW fixup ship | pending — user-driven: `but rub zz feature-20b-skill-trees` + `but commit --message-file <path>` + `btp feature-20b-skill-trees`. Appends to existing PR #23. |
| 5.2.fixup-review | Targeted re-review of Phase 2 fixup | pending — narrow scope: verify `guild_skills.rs:160-170` (LOW #1 doc) and `guild_skills.rs:114-125` (LOW #2 tamper guard) only. Append addendum to `project/reviews/20260514-200000-feature-20b-skill-trees.md`. |
| 2.3  | Plan refresh (Phase 3) | pending — verify Phase 3 plan section holds, add "Stacked-PR protocol for Phase 3" subsection (branch from `feature-20b-skill-trees`, `gh pr create --base feature-20b-skill-trees`), surface any new Cat-C questions in one batch. |
| 3.3  | Implement (Phase 3) | pending — gated on user go/no-go after Phase 2 fixup re-review + Phase 3 plan refresh |
| 4.3  | Ship (Phase 3) | pending — branch `feature-20c-spell-menu` (stacked on `feature-20b-skill-trees`) |
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
- **Phase 3 stacking:** Now CONFIRMED — Option A stack on Phase 2 (`feature-20b-skill-trees`).
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

### Phase 2 post-review decisions (2026-05-14, post-APPROVE-verdict)

- **Option B chosen (user, 2026-05-14):** Address Phase 2 LOW findings on existing branch first via cosmetic fixup, then dispatch Phase 3 stacked on `feature-20b-skill-trees`. Same shape as Phase 1's post-review flow.
- **Phase 2 fixup implementer brief:** Narrow — Fix A (doc-comment on `sorted_nodes`) + Fix B (tamper-guard short-circuit in `node_state` `SpInsufficient` arm) verbatim from review file lines 99-138. Optional smoke test approved.
- **Phase 2 fixup ship protocol:** User-driven, same as Phase 1 fixup. Implementer stops at gate. User stages with `but rub zz feature-20b-skill-trees`, commits with `--message-file`, pushes with `btp` to append to existing PR #23 (no new branch, no new PR).
- **Phase 2 fixup re-review scope:** Narrow — verify the two LOW sites only. Append addendum to existing review file at `project/reviews/20260514-200000-feature-20b-skill-trees.md`. Do NOT re-review the base Phase 2 commit.
- **Phase 3 stacking confirmed:** Option A — stack on Phase 2 / `feature-20b-skill-trees`. Plan must gain "Stacked-PR protocol for Phase 3" subsection mirroring Phase 2's. Three-PR stack: #21 ← #23 ← #(Phase 3).

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
- Phase 3 stacking — CONFIRMED, stack on Phase 2 / `feature-20b-skill-trees`.

## Phase 2 LOW fixup-impl details (2026-05-14 ~21:00)

- **File modified:** `src/plugins/town/guild_skills.rs` ONLY
- **Fix A (LOW #1):** Lines 160-170 — `# Precondition` rustdoc H1 section added to `sorted_nodes` doc-comment. States acyclic-tree precondition, references `validate_no_cycles` + `validate_skill_trees_on_load` + production call-site guard pattern, recommends test fixture authors use validators first.
- **Fix B (LOW #2):** Lines 114-125 — `invariant_ok` bool binding added inside `Err(SkillError::Insufficient)` arm of `node_state`. Three-condition guard: `if prereqs_met && level_met && invariant_ok { SpInsufficient } else { Locked(Insufficient) }`. Tampered-save case (`unspent > total_earned`) now shows `Locked` not yellow.
- **Optional smoke test added:** Lines 578-591 — `node_state_returns_locked_when_invariant_violated`. Uses `make_exp(1, 5, 3)`. Test count 363 → 364.
- **LOC delta:** +~26 lines total.
- **Implementation summary:** `/Users/nousunio/Repos/Learnings/claude-code/druum/project/implemented/20260514-210000-feature-20b-review-fixup.md`
- **Commit message file:** `/Users/nousunio/Repos/Learnings/claude-code/druum/project/shipper/feature-20b-fixup-commit-msg.txt`
- **Co-author trailer:** `Claude Opus 4.7 (1M context) <noreply@anthropic.com>` (matches Phase 1 fixup's exact spelling)
- **No code changes outside the two sites.** No other files touched.
- **Gates:** NOT run by implementer (no shell access in subagent env). User to run manually before ship.

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

## Phase 2 post-review state (2026-05-14)

- **Verdict:** APPROVE — mergeable as-is. 0 CRITICAL / 0 HIGH / 0 MEDIUM / 2 LOW.
- **LOW #1 (`guild_skills.rs:136-157`):** `node_depth` recursion has no in-flight cycle guard. Production-safe (two-layer defence: `validate_skill_trees_on_load` empties cyclic trees + both callers check `tree.nodes.is_empty()`). Future-footgun risk if a test constructs a cyclic tree directly. Fix = doc-comment-only `# Precondition` note on `sorted_nodes`. **APPLIED in fixup (lines 160-170).**
- **LOW #2 (`guild_skills.rs:114-123`):** `node_state` returns yellow `SpInsufficient` for tampered-save case (`unspent > total_earned`) instead of `Locked`. Unlock handler still rejects correctly; visual-only discrepancy on an unsupported tampered-save path. Fix = add `invariant_ok = unspent <= total_earned` to the yellow re-check arm. **APPLIED in fixup (lines 114-125).**
- **GitHub posting:** Both PR #21 and PR #23 review bodies posted to GitHub by user.
- **Deviations review (all 5):** All accepted — no fix-up required.

## Resume instructions

### Next user-facing action (PAUSED here — handing back for ship)

Phase 2 LOW fixup is IMPLEMENTED in `zz`. User to run:

1. **Gates:**
   - `cargo check`
   - `cargo test --lib` (expect 364)
   - `cargo clippy --all-targets -- -D warnings`
2. **Ship:**
   - `but rub zz feature-20b-skill-trees`
   - `but commit --message-file /Users/nousunio/Repos/Learnings/claude-code/druum/project/shipper/feature-20b-fixup-commit-msg.txt`
   - `btp feature-20b-skill-trees` (or `but push -u origin feature-20b-skill-trees`)
3. **Report back to orchestrator** with the new commit SHA on PR #23.

### After ship — orchestrator resumes with:

1. **Narrow re-reviewer dispatch.** Pass exact prompt: verify the two LOW sites only (`guild_skills.rs:160-170` doc-comment + `guild_skills.rs:114-125` tamper guard). Append addendum to existing review file. Do NOT re-review base.
2. **Phase 3 plan refresh.** Update plan with "Stacked-PR protocol for Phase 3" subsection. Run planner narrowly scoped to Phase 3, surface any Cat-C questions in one batch.
3. **Pause for user go/no-go on Phase 3 implementer dispatch.** Show fixup re-review verdict + Phase 3 plan delta + Cat-C questions if any. Do NOT auto-dispatch Phase 3 implementer.

### GitButler lesson learned (to save as memory after Phase 2 fixup ships)

`but commit <new-branch-name>` does NOT auto-create the branch — it errors with "Branch not found". To stack: MUST use `but branch new <name> --anchor <parent>` FIRST. CLAUDE.md's "creates a NEW branch with that name and route the commit there" guidance is outdated. Save as feedback memory.

### Phase 3 dispatch checklist (when user confirms post-fixup)

1. Confirm stacking: CONFIRMED — stack on Phase 2 / `feature-20b-skill-trees`.
2. Re-read Phase 3 section of `project/plans/20260514-120000-feature-20-spells-skill-tree.md`.
3. Update plan with "Stacked-PR protocol for Phase 3" subsection (branch from `feature-20b-skill-trees` via `but branch new feature-20c-spell-menu --anchor feature-20b-skill-trees`, `gh pr create --base feature-20b-skill-trees --head feature-20c-spell-menu`, rebase discipline if upstream PRs receive fixups).
4. Run planner narrowly scoped — verify Phase 3 section holds, resolve new Cat-C questions if any, confirm concrete file lists + test counts.
5. Pause for user. Show: fixup re-review verdict + Phase 3 plan delta + Cat-C questions.
6. On go-ahead: dispatch `run-implementer` with plan path + user-driven-ship protocol + DungeonAssets fan-out reminder + WarnedMissingSpells key-shape reminder.
7. Same user-driven ship protocol as Phase 1 + 2 — implementer STOPS at verification gate; user creates `feature-20c-spell-menu` branch + ships.
8. Same review focus protocol — orchestrator dispatches `run-reviewer` with Phase 3 focus areas (SpellMenu integration, MP consumption, target selection, known-but-not-validated spell IDs, Q9 warn-once mechanism).
