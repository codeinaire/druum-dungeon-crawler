# Pipeline State

**Task:** Druum issue #20 — Spells & Skill Trees. Three-PR pipeline (Phase 1 / Phase 2 / Phase 3), each phase = research → plan → implement → review → ship cycle. Plan updated 2026-05-14 for three-PR split.
**Status:** COMPLETE (2026-05-14) — All 3 phases shipped, reviewed, fixup-cycled, and re-reviewed. 3 PRs open and APPROVE'd. User handles merge timing themselves. Closeout summary: `/Users/nousunio/Repos/Learnings/claude-code/druum/project/orchestrator/20260514-000000-feature-20-spells-skill-tree-final.md`.
**Last Completed Step:** 5.3.fixup-review (narrow re-review addendum, Phase 3 fixup commit `f193962`). Verdict: ADDENDUM-APPROVE. Closeout summary written.
**Current Phase:** Pipeline COMPLETE. Awaiting user-driven merge (bottom-up #21 → #23 → #24).

## Artifacts

| Step | Description | Artifact                                 |
| ---- | ----------- | ---------------------------------------- |
| 1    | Research    | /Users/nousunio/Repos/Learnings/claude-code/druum/project/research/20260514-druum-20-spells-skill-tree.md |
| 2    | Plan        | /Users/nousunio/Repos/Learnings/claude-code/druum/project/plans/20260514-120000-feature-20-spells-skill-tree.md (Phase 2 + Phase 3 Stacked-PR protocols added 2026-05-14) |
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
| 5.2.fixup-ship | Phase 2 LOW fixup ship | COMPLETE — Commit `e210cf4` pushed to `feature-20b-skill-trees`, live on PR #23. Gates all green (cargo check, cargo test --lib 364/364, cargo clippy --all-targets -- -D warnings). Subject: `docs(town): address review findings (#23) — node_depth precondition + tamper guard`. |
| 5.2.fixup-review | Targeted re-review of Phase 2 fixup | COMPLETE — addendum appended at `/Users/nousunio/Repos/Learnings/claude-code/druum/project/reviews/20260514-200000-feature-20b-skill-trees.md:172+`. Verdict: ADDENDUM-APPROVE. Both LOW findings RESOLVED. Smoke test correctly exercises tamper-guard path (make_exp(1,5,3): unspent=5 > total=3). No regressions. Queued for user posting to PR #23 via `gh pr review 23 --comment --body-file <addendum-extract>`. |
| 2.3  | Plan refresh (Phase 3) | COMPLETE — Cat-C-4/5/6 user-locked decisions inlined into plan Step 2.6 (lines 543, 547, 548). Cat-C-4 = A (paint "(no castable spells)", don't pop). Cat-C-5 = A (pre-check SingleEnemy at Confirm, mirror Attack guard at `turn_manager.rs:475-478,489-492`, log + stay). Cat-C-6 = A (non-wrap saturating, consistent with Main + Guild Skills). |
| 3.3  | Implement (Phase 3) | COMPLETE 2026-05-14 — implementer summary: `/Users/nousunio/Repos/Learnings/claude-code/druum/project/implemented/20260514-180000-feature-20c-spell-menu.md`. Files modified (exact match to plan): `src/plugins/combat/ui_combat.rs` (~+150 LOC + new `silence_blocks_real_spell_menu` test), `src/plugins/combat/turn_manager.rs` (+1 line: `pub spell_cursor: usize` field on `PlayerInputState` at line 159), `src/plugins/party/mod.rs` (~+12 LOC dev-party `KnownSpells` defaults). Cat-C-4/5/6 all implemented per spec. Δ Cargo.toml = 0 confirmed. One implementation note: SpellMenuState enum extracted outside egui closure to dodge multi-borrow on `ResMut<WarnedMissingSpells>` + `Query<&KnownSpells>` — semantics identical. Plan checkboxes 2.6 + 2.7 flipped to `[x]`. |
| 4.3  | Ship (Phase 3, STACKED) | PR: https://github.com/codeinaire/druum-dungeon-crawler/pull/24, Branch: feature-20c-spell-menu, Commit: 9465fb2 (gate-pass fixes folded in: encounter.rs:597 + enemy_render.rs:723 init_asset::<SpellDb>() additions, same pattern as Phase 1's ai.rs fix). Base: feature-20b-skill-trees (stacked). Three-PR chain: #21 ← #23 ← #24. Gates all green (cargo check both variants, cargo test --lib 365/365 default + 369/369 dev, cargo test --test '*' 8/8, cargo clippy --all-targets -- -D warnings both variants). |
| 5.3  | Code Review (Phase 3) | COMPLETE 2026-05-14 — /Users/nousunio/Repos/Learnings/claude-code/druum/project/reviews/20260514-210000-feature-20c-spell-menu.md. Verdict: APPROVE. 0 CRITICAL / 0 HIGH / 0 MEDIUM / 1 LOW (cosmetic doc-comment only: `spell_cursor` not reset on Cancel at ui_combat.rs:471-478, defensive future-proofing). All 7 focus areas verified correct. Borrow-checker deviation (SpellMenuState extraction) accepted as semantics-identical structural improvement. Review body POSTED to PR #24 by user (1 review comment live). |
| 5.3.fixup-impl | Phase 3 LOW fixup impl | COMPLETE 2026-05-14 ~22:00 — `src/plugins/combat/ui_combat.rs` only file modified. Single change: lines 470-472 — replaced one-line `// Cancel: pop submenu (top-of-stack only; Main does nothing).` with a 3-line block adding `NOTE: spell_cursor is reset on *entry* to SpellMenu (Main arm case 2, line 532), not on exit. Any future code that pushes SpellMenu directly must reset it.` Line 532 reference verified accurate before applying. LOC +2. No code changes, no test changes — test count unchanged at 365/365 lib + 369/369 dev + 8/8 integration. Summary: /Users/nousunio/Repos/Learnings/claude-code/druum/project/implemented/20260514-220000-feature-20c-review-fixup.md. Commit msg: /Users/nousunio/Repos/Learnings/claude-code/druum/project/shipper/feature-20c-fixup-commit-msg.txt. Subject: `docs(combat): address review findings (#24) — spell_cursor reset path comment`. |
| 5.3.fixup-ship | Phase 3 LOW fixup ship | COMPLETE 2026-05-14 — Commit `f193962` pushed to `feature-20c-spell-menu`, live on PR #24. Gates all green (cargo check, cargo test --lib 365/365, cargo clippy --all-targets -- -D warnings). |
| 5.3.fixup-review | Targeted re-review of Phase 3 fixup | COMPLETE 2026-05-14 — addendum appended to `/Users/nousunio/Repos/Learnings/claude-code/druum/project/reviews/20260514-210000-feature-20c-spell-menu.md`. Verdict: ADDENDUM-APPROVE — 0 open findings. Comment-only change at `ui_combat.rs:471-472` correctly resolves the LOW; line 532 reference verified accurate; regression impossible from comment change. Addendum body queued at `/tmp/pr24-fixup-addendum.md` for user posting via `gh pr review 24 --comment --body-file /tmp/pr24-fixup-addendum.md`. |
| 6   | Closeout summary | COMPLETE 2026-05-14 — `/Users/nousunio/Repos/Learnings/druum/project/orchestrator/20260514-000000-feature-20-spells-skill-tree-final.md` (note: actual path is `/Users/nousunio/Repos/Learnings/claude-code/druum/project/orchestrator/20260514-000000-feature-20-spells-skill-tree-final.md`). All sections per closeout brief: TL;DR, phase log, gate matrix, Cat-C ledger, fixup cycles, memory entries, open follow-ups, merge guidance, artifact links. |

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

### Phase 2 plan-delta decisions (2026-05-14, post-Cat-C-resolution)

- **Cat-C-1 (painter state count):** **Option B — 4-state painter with yellow tier.** Unlocked / can-unlock-now / SP-insufficient-but-prereq+level-met (yellow) / locked. Helps SP-save planning. Pure-fn `node_state(node, experience, unlocked) -> NodeState` factored for testability. Adds 2 new unit tests.
- **Cat-C-3 (`NodeGrant::LearnSpell(SpellId)` validation scope):** **Option A — warn-and-filter at consume-time only.** `validate_skill_trees_on_load` stays structural (cycles + clamp only). Bogus spell IDs flow into `KnownSpells` and surface in Phase 3's `SpellMenu` painter via the Q9 `WarnedMissingSpells: HashSet<(SpellId, Entity)>` warn-once mechanism. Single source of truth.

### Phase 2 plan-delta Cat-A fixes (2026-05-14)

1. **`WarnedMissingSpells` key-shape fix.** `HashSet<SpellId>` → `HashSet<(SpellId, Entity)>` to match user's Q9 decision "warn-once-per-(spell, character)". Plan §Step 2.1 + §Step 2.6 (Phase 3 painter) updated.
2. **Execution-order block added at top of Phase 2 part A.** Each step now compiles green at its own commit.
3. **Stacked-PR rebase discipline subsection added to Phase 2 part A.**

### Phase 2 dispatch decisions (2026-05-14, post-implementer-go-ahead)

- **User-driven ship protocol (same as Phase 1):** Implementer STOPS at the Phase 2 verification gate. User runs gates manually + creates `feature-20b-skill-trees` branch via `but branch new` + `but rub zz feature-20b-skill-trees` + `but commit` + `btp` + `gh pr create --base feature-20a-spell-registry`. Orchestrator does NOT run `run-shipper` after Phase 2 implementer completes.
- **Working tree state during Phase 2 implementation:** All changes accumulate in `zz` (unassigned) on `gitbutler/workspace`. No branch creation, no commits, no pushes. The branch is created at ship time by the user.
- **Live-test landmine briefed to implementer:** `tests/dungeon_movement.rs:146-154` and `tests/dungeon_geometry.rs:150-158` are the canonical `DungeonAssets` fixtures.

### Phase 2 post-review decisions (2026-05-14, post-APPROVE-verdict)

- **Option B chosen (user, 2026-05-14):** Address Phase 2 LOW findings on existing branch first via cosmetic fixup, then dispatch Phase 3 stacked on `feature-20b-skill-trees`. Same shape as Phase 1's post-review flow.
- **Phase 2 fixup implementer brief:** Narrow — Fix A (doc-comment on `sorted_nodes`) + Fix B (tamper-guard short-circuit in `node_state` `SpInsufficient` arm) verbatim from review file lines 99-138. Optional smoke test approved.
- **Phase 2 fixup ship protocol:** User-driven, same as Phase 1 fixup. Implementer stops at gate. User stages with `but rub zz feature-20b-skill-trees`, commits with `--message-file`, pushes with `btp` to append to existing PR #23 (no new branch, no new PR).
- **Phase 2 fixup re-review scope:** Narrow — verify the two LOW sites only. Append addendum to existing review file at `project/reviews/20260514-200000-feature-20b-skill-trees.md`. Do NOT re-review the base Phase 2 commit. COMPLETE 2026-05-14 — ADDENDUM-APPROVE.
- **Phase 3 stacking confirmed:** Option A — stack on Phase 2 / `feature-20b-skill-trees`. Plan must gain "Stacked-PR protocol for Phase 3" subsection mirroring Phase 2's. Three-PR stack: #21 ← #23 ← #(Phase 3).

### Phase 3 Cat-C decisions (2026-05-14, user-locked pre-implementation)

- **Cat-C-4 (empty KnownSpells path):** Option A — paint "(no castable spells)" or "(no spells)", do NOT auto-pop.
- **Cat-C-5 (SingleEnemy guard):** Option A — pre-check at Confirm mirroring Attack guard at `turn_manager.rs:475-478,489-492`, log + stay in SpellMenu on no valid targets.
- **Cat-C-6 (cursor wrap):** Option A — non-wrap saturating, consistent with Main + Guild Skills.

### Phase 3 post-review decisions (2026-05-14)

- **Decision 1 — Review posting:** USER POSTED `/tmp/pr24-review-body.md` to PR #24 via `gh pr review 24 --comment --body-file <path>`. PR #24 now has 1 review comment.
- **Decision 2 — LOW fixup:** YES — same protocol as Phase 1/2. User wants the fixup despite orchestrator's "skip is also valid" framing. Consistent across all three phases of #20.
- **Decision 3 — Merge order:** User will handle merge timing themselves. NO auto-merge. After fixup cycle completes, orchestrator proceeds to closeout (`write-orchestrator-summary`) and leaves all three PRs open for user-driven merging. Bottom-up recommendation (#21 → #23 → #24) noted as guidance.

### GitButler stacked-branch discovery (2026-05-14, Phase 2 ship friction)

- **`but commit <new-branch-name>` does NOT auto-create branches** in the current `but` version — errors with `Branch '<name>' not found`. To stack: MUST use `but branch new <name> --anchor <parent>` FIRST. The CLAUDE.md guidance saying "creates a NEW branch with that name and route the commit there" is OUTDATED.
- **Phase 3 implication:** branch creation MUST be `but branch new feature-20c-spell-menu --anchor feature-20b-skill-trees` BEFORE staging. (Already applied in Phase 3 ship.)
- **Memory note to save:** as feedback memory under slug `druum-gitbutler-stacked-branch-creation` after Phase 3 ships.

## Phase 1 ship details (2026-05-14)

- **Branch:** `feature-20a-spell-registry` from `main`
- **Initial commit:** `e343585`
- **Fixup commit:** `5708c90`
- **PR:** https://github.com/codeinaire/druum-dungeon-crawler/pull/21
- **Files changed (initial):** 23
- **Files changed (fixup):** 2 — comments-only
- **Fixup gates:** cargo check / cargo clippy --all-targets -- -D warnings / cargo test --lib 339/339 — all green

## Phase 1 fixup-impl details (2026-05-14)

- **Fix A (MEDIUM):** `src/plugins/combat/turn_manager.rs:593-600` — 8-line invariant comment above the MP-check block in CastSpell arm.
- **Fix B (LOW):** `src/plugins/combat/status_effects.rs:319-320` and `347-348` — TODO(#22) markers.
- **Commit message file:** `/Users/nousunio/Repos/Learnings/claude-code/druum/project/shipper/feature-20a-fixup-commit-msg.txt`

## Phase 2 stacked-PR protocol (added to plan 2026-05-14 ~20:00)

Plan §Phase 2 now includes "Stacked-PR protocol" subsection. Key rules:
- Branch from `feature-20a-spell-registry`, NOT from `main`
- `gh pr create --base feature-20a-spell-registry --head feature-20b-skill-trees`
- Auto-retarget on Phase 1 merge — no manual action
- Rebase discipline: if Phase 1 receives further fixups, Phase 2 must be rebased before push
- Phase 3 stacking — CONFIRMED, stack on Phase 2 / `feature-20b-skill-trees`.

## Phase 2 LOW fixup details (2026-05-14 ~21:00)

- **File modified:** `src/plugins/town/guild_skills.rs` ONLY
- **Fix A (LOW #1):** Lines 160-170 — `# Precondition` rustdoc H1 section added to `sorted_nodes`.
- **Fix B (LOW #2):** Lines 114-125 — `invariant_ok` bool binding added inside `Err(SkillError::Insufficient)` arm of `node_state`.
- **Optional smoke test added:** Lines 582-591 — `node_state_returns_locked_when_invariant_violated`. Test count 363 → 364.
- **Ship commit:** `e210cf4` pushed 2026-05-14, live on PR #23.

## Phase 2 LOW fixup-review details (2026-05-14 ~22:00)

- **Verdict:** ADDENDUM-APPROVE
- **Both LOW findings:** Fully resolved.
- **Addendum location:** Appended to `/Users/nousunio/Repos/Learnings/claude-code/druum/project/reviews/20260514-200000-feature-20b-skill-trees.md` at line 172+.

## Phase 3 ship details (2026-05-14)

- **Branch:** `feature-20c-spell-menu` (stacked on `feature-20b-skill-trees` via `but branch new feature-20c-spell-menu --anchor feature-20b-skill-trees`)
- **Initial commit:** `9465fb2`
- **PR:** https://github.com/codeinaire/druum-dungeon-crawler/pull/24
- **Base:** `feature-20b-skill-trees` (PR #23) — three-PR chain #21 ← #23 ← #24
- **Files changed:** 5 (ui_combat.rs ~+150 LOC, turn_manager.rs +1 line, party/mod.rs ~+12 LOC, encounter.rs +1 line init_asset, enemy_render.rs +1 line init_asset)
- **Gates verified by user:** cargo check both variants ✓, cargo test --lib 365/365 default + 369/369 dev ✓, cargo test --test '*' 8/8 ✓, cargo clippy --all-targets -- -D warnings both variants ✓
- **Gate-pass fixes folded into commit `9465fb2`:** `encounter.rs:597` + `enemy_render.rs:723` `init_asset::<SpellDb>()` additions (same pattern as Phase 1's ai.rs fix — `handle_combat_input` system now requires `Res<Assets<SpellDb>>`).
- **One accepted deviation:** `SpellMenuState` enum extracted outside `egui::Window::show` closure (`ui_combat.rs:285-345` builds, `:351-389` renders). Borrow checker forced — `ResMut<WarnedMissingSpells>` + `Query<&KnownSpells>` cannot both cross `FnOnce` boundary. Semantics identical, code cleaner. Documented in implementer summary lines 53-63.

## Phase 3 review verdict (2026-05-14)

- **Verdict:** APPROVE — mergeable as-is. 0 CRITICAL / 0 HIGH / 0 MEDIUM / 1 LOW.
- **LOW (cosmetic, `ui_combat.rs:471-478`):** Cancel pops `SpellMenu` without resetting `spell_cursor`. Not a bug — entry always resets via Main arm case 2 (line 532). Future-proofing comment recommended. POSTED to PR #24 (review body via `gh pr review 24 --comment --body-file`).
- **All 7 focus areas verified correct:**
  1. Silence gate preservation — both existing + new sibling tests assert real menu behavior.
  2. `WarnedMissingSpells` warn-once semantics — `(SpellId, Entity)` tuple, `.set.insert(...)` at painter:318 + handler:636 only.
  3. Cat-C-4 dual empty-state painter — `Empty` vs `NoCastable`, distinct messages, neither auto-pops.
  4. Cat-C-5 SingleEnemy guard — live `enemies` query, log-and-stay on empty, mirrors Attack guard.
  5. Cat-C-6 saturating cursor — `(cursor + 1).min(castable.len().saturating_sub(1))` correct, entry-reset on Main case 2.
  6. Borrow-checker deviation — accepted as semantics-identical structural improvement (owned-data enum before closure).
  7. Dev-party defaults — `halito` + `katino` (Mage), `dios` + `matu` (Priest), `[]` (Fighter), all IDs verified against `core.spells.ron`.

## Phase 3 LOW fixup-impl details (2026-05-14 ~22:00)

- **File modified:** `src/plugins/combat/ui_combat.rs` ONLY
- **Single change:** Lines 470-472 — replaced one-line existing comment with 3-line block adding `NOTE: spell_cursor is reset on *entry* to SpellMenu (Main arm case 2, line 532), not on exit. Any future code that pushes SpellMenu directly must reset it.` Verbatim from reviewer's LOW suggestion.
- **Line 532 verification:** Confirmed accurate before applying — `input_state.spell_cursor = 0;` is at `ui_combat.rs:532` inside Main arm `2 =>` (Spell) case.
- **LOC:** +2 lines (comment-only)
- **Test count:** Unchanged — 365/365 lib + 369/369 dev-party + 8/8 integration.
- **Commit message file:** `/Users/nousunio/Repos/Learnings/claude-code/druum/project/shipper/feature-20c-fixup-commit-msg.txt`
- **Implementation summary:** `/Users/nousunio/Repos/Learnings/claude-code/druum/project/implemented/20260514-220000-feature-20c-review-fixup.md`
- **Ship status:** PAUSED — awaiting user-driven gates + `but rub zz feature-20c-spell-menu` + `but commit --message-file project/shipper/feature-20c-fixup-commit-msg.txt` + `btp feature-20c-spell-menu` to append to PR #24 (no new branch, no new PR).

## Phase 1 implementer deviations (carry-forward documented)

1. Crit chance uses `accuracy / 5`% not `luck / 5`%.
2. `CombatantCharsQuery` uses `&'static mut StatusEffects`.
3. Revive bypasses `resolve_target_with_fallback`.
4. Cast announcement log fires BEFORE per-target effect logs.
5. Four `DungeonAssets` test fixtures updated.
6. Targeted fix #1: `SpellCastParams<'w>` struct + missing `SystemParam` import.

## Reviewer focus areas for Phase 3 (already executed)

All 7 priority areas verified. See Phase 3 review at `/Users/nousunio/Repos/Learnings/claude-code/druum/project/reviews/20260514-210000-feature-20c-spell-menu.md`.

## Resume instructions

### Next user-facing action (PAUSED here — Phase 3 LOW fixup impl COMPLETE)

User to run Phase 3 fixup ship steps (mirrors Phase 1 + Phase 2 fixup ship protocol):

1. **Gates** (expect all green — comment-only change, test count unchanged):
   ```
   cargo check
   cargo check --features dev-party
   cargo test --lib                                       # expect 365/365
   cargo test --lib --features dev-party                  # expect 369/369
   cargo test --test '*'                                  # expect 8/8
   cargo clippy --all-targets -- -D warnings
   cargo clippy --all-targets --features dev-party -- -D warnings
   ```

2. **Stage + commit + push** (append to existing branch — no new branch, no new PR):
   ```
   but rub zz feature-20c-spell-menu
   but commit --message-file project/shipper/feature-20c-fixup-commit-msg.txt
   btp feature-20c-spell-menu
   ```

   This appends the fixup commit to existing PR #24.

### After ship — orchestrator resumes:

- **Narrow re-review:** Append addendum to existing Phase 3 review file at `/Users/nousunio/Repos/Learnings/claude-code/druum/project/reviews/20260514-210000-feature-20c-spell-menu.md` (DO NOT create new file). Verify the LOW finding at `ui_combat.rs:471-478` is resolved. Base commit `9465fb2` already approved — do NOT re-review unchanged code. Queue addendum body at `/tmp/pr24-fixup-addendum.md` for user to post.
- **Feature #20 closeout:** Run `write-orchestrator-summary` to produce final closeout summary at `project/orchestrator/<dated>-feature-20-spells-skill-tree-final.md`. Include: all 3 phases shipped (PR #21, #23, #24), all fixup cycles documented, final test counts (365 lib + 8 integration), Δ Cargo.toml = 0 across all phases, all Cat-C decisions resolved (8 total), memory entries created, Issue #22 as open follow-up, merge order recommendation (bottom-up #21 → #23 → #24, user handles timing).
- **Mark pipeline COMPLETE** in this state file.
- **Hand back to user** with: closeout summary path, final test count + gate matrix, merge guidance reminder, open follow-ups.
- **Do NOT auto-merge anything.**

### Merge order rationale (bottom-up #21 → #23 → #24)

Standard for stacked PRs: merging the bottom of the stack first causes GitHub to auto-retarget the next PR's base to `main`. Merging top-down would leave the lower PRs with rebased histories that don't match `main`.

1. Merge PR #21 (Phase 1 — spell registry). #23 auto-retargets to `main`.
2. Merge PR #23 (Phase 2 — skill trees). #24 auto-retargets to `main`.
3. Merge PR #24 (Phase 3 — spell menu). Feature #20 complete.

**User has signaled they will handle merge timing themselves — orchestrator will NOT auto-merge.**

**Issue #22 caveat:** carry-forward filed by user before Phase 1 — `apply_poison_damage` + `apply_regen` query widening when combat-round status ticks are added for enemies. Phase 2 + 3 did not introduce that, so #22 remains open. Should be considered a Feature #20 follow-up.
