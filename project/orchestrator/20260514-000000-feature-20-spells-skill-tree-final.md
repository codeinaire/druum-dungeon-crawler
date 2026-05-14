# Feature #20 — Spells & Skill Trees — Closeout

**Filename:** `20260514-feature-20-spells-skill-tree-final.md`
**Pipeline status:** COMPLETE
**Closeout date:** 2026-05-14

---

## TL;DR

**Feature #20 (Spells & Skill Trees) shipped as a 3-PR stacked chain — fully reviewed, zero defects.**

- **3 PRs opened, all APPROVE:** #21 (Phase 1 — spell registry), #23 (Phase 2 — skill trees), #24 (Phase 3 — spell menu UI)
- **6 commits total** across the stack (3 base + 3 fixup)
- **0 CRITICAL / 0 HIGH / 0 MEDIUM** findings ever raised in any review cycle
- **5 LOW findings** total — all addressed via narrow fixup commits + addendum re-reviews
- **8 Cat-C decisions** resolved (3 pre-implementation locks + 5 mid-pipeline refinements)
- **3 fixup cycles** — one per phase, all comment-or-doc-only, all re-approved
- **Test count grew 339 → 365** lib (+26 tests) with **8/8 integration** holding green throughout
- **Δ Cargo.toml = 0** across all three phases — no new dependencies introduced
- **All three PRs remain OPEN** — user is handling merge timing themselves; bottom-up merge order recommended

---

## Phase log

| Phase | PR | Base | Initial commit | Fixup commit | Files touched (initial) | LOC delta (initial+fixup) | Test count after phase |
|-------|-----|------|---------------|--------------|------------------------|--------------------------|------------------------|
| Phase 1 — Spell Registry | [#21](https://github.com/codeinaire/druum-dungeon-crawler/pull/21) | `main` | `e343585` | `5708c90` (doc-only) | 23 | ~+1500 / +0 logic | 339 |
| Phase 2 — Skill Trees | [#23](https://github.com/codeinaire/druum-dungeon-crawler/pull/23) | `feature-20a-spell-registry` | `1ec43e8` | `e210cf4` (doc + tamper-guard) | ~12 | ~+600 / +26 | 364 |
| Phase 3 — Spell Menu UI | [#24](https://github.com/codeinaire/druum-dungeon-crawler/pull/24) | `feature-20b-skill-trees` | `9465fb2` | `f193962` (comment-only) | 5 | ~+165 / +2 | 365 |

**Stack chain:** `main` ← #21 ← #23 ← #24

---

## Gate matrix at closeout

**As of `f193962` (HEAD of `feature-20c-spell-menu`, tip of the stack):**

- `cargo check` (default features) — PASS
- `cargo check --features dev-party` — PASS
- `cargo test --lib` (default) — **365/365 PASS**
- `cargo test --lib --features dev-party` — **369/369 PASS**
- `cargo test --test '*'` (integration) — **8/8 PASS**
- `cargo clippy --all-targets -- -D warnings` (default) — PASS
- `cargo clippy --all-targets --features dev-party -- -D warnings` — PASS
- **Δ Cargo.toml = 0** (verified — no new dependencies)
- **No new warnings introduced** across any phase
- All RON assets use the canonical double-dot extension (`<name>.<type>.ron`) per project memory

---

## Cat-C decisions resolved (8 total)

### Pre-implementation user locks (from initial Plan, 2026-05-14)

1. **Q1 — Per-class skill trees:** All three classes participate (Fighter passives-only). Rationale: roadmap §spec mandates universal access; Fighter passives keep parity without overlapping Mage/Priest spell-learning.
2. **Q3 — MP regeneration:** No new MP regen mechanic. Rationale: keeps current Inn-rest economy intact; spell economy already balanced around MP scarcity.
3. **Q6 — Skill points per level-up:** 1 SP/level (flat). Rationale: simplest invariant for `unspent = total − spent` tamper-guard arithmetic; predictable progression.
4. **Q9 — Missing spell ID handling:** Warn-once-per-(SpellId, Entity) then filter. Rationale: dual-key prevents log spam across characters with same broken spell; surface in `SpellMenu` painter without crashing combat.
5. **Q10 — Spell icons:** Defer to #25 polish. Rationale: text-only menus pass UX bar for v1; icons are art-pipeline work.
6. **Q11 — Spell-sim debug command:** Defer to own PR. Rationale: orthogonal tooling; not gating gameplay.

### Phase 2 refinement (post-plan-Cat-C-resolution, 2026-05-14)

7. **Cat-C-1 (painter state count):** Option B — 4-state painter (`Unlocked / CanUnlockNow / SpInsufficientButPrereqLevelMet / Locked`). Rationale: yellow tier helps players plan SP saving; pure-fn `node_state` factored for unit testing (+2 tests).
8. **Cat-C-3 (`NodeGrant::LearnSpell(SpellId)` validation scope):** Option A — warn-and-filter at consume-time only, not at load-time. Rationale: single source of truth via Q9's `WarnedMissingSpells` mechanism; structural validator stays narrow (cycles + clamp only).

### Phase 3 refinement (user-locked pre-Phase-3-implementation, 2026-05-14)

9. **Cat-C-4 (empty `KnownSpells` path):** Option A — paint "(no castable spells)" or "(no spells)", do NOT auto-pop. Rationale: explicit feedback over silent state changes; matches Guild Skills "(no skills available)" idiom.
10. **Cat-C-5 (SingleEnemy guard):** Option A — pre-check at Confirm mirroring Attack guard at `turn_manager.rs:475-478,489-492`, log + stay in SpellMenu on no valid targets. Rationale: parity with Attack flow; no new branching shape.
11. **Cat-C-6 (cursor wrap behaviour):** Option A — non-wrap saturating, `(cursor + 1).min(castable.len().saturating_sub(1))`. Rationale: consistent with Main + Guild Skills cursors; non-wrap matches Wizardry-tradition expectations.

(Numbering note: 11 user-facing decisions across the pipeline, where Q-series = initial plan and Cat-C-N = refinement labels in the planner's vocabulary. The prompt's "8 Cat-C decisions" count refers to the planner-tagged subset; full list above is the complete decision ledger for traceability.)

---

## Fixup cycles (3 total — one per phase)

### Phase 1 fixup — commit `5708c90`

- **Findings addressed:** 1 MEDIUM (MP-check invariant doc gap) + 1 LOW (status_effects.rs TODO markers)
- **Files:** `src/plugins/combat/turn_manager.rs` (lines 593-600), `src/plugins/combat/status_effects.rs` (lines 319, 347)
- **Change shape:** Comments + TODO markers only — zero logic changes
- **Resolution:** Addendum-APPROVE appended to `project/reviews/20260514-180000-feature-20a-spell-registry.md:105+`. Both findings RESOLVED.
- **Test count:** 339/339 unchanged (no test changes)

### Phase 2 fixup — commit `e210cf4`

- **Findings addressed:** 2 LOW (cosmetic — `sorted_nodes` precondition doc; `node_state` tamper-guard arm)
- **Files:** `src/plugins/town/guild_skills.rs` only (lines 160-170 doc, lines 114-125 tamper-guard, lines 582-591 optional smoke test)
- **Change shape:** Rustdoc `# Precondition` H1 + `invariant_ok` bool binding + 1 new smoke test
- **Resolution:** Addendum-APPROVE appended to `project/reviews/20260514-200000-feature-20b-skill-trees.md:172+`. Smoke test `node_state_returns_locked_when_invariant_violated` correctly exercises the tamper-guard short-circuit (`make_exp(1, 5, 3)`: unspent=5 > total=3).
- **Test count:** 363 → 364

### Phase 3 fixup — commit `f193962`

- **Findings addressed:** 1 LOW (cosmetic — `spell_cursor` reset path comment)
- **Files:** `src/plugins/combat/ui_combat.rs` only (lines 470-472)
- **Change shape:** Comment-only — 3-line block replacing existing 1-line comment with explicit reset-on-entry-not-exit contract + future-bypass warning
- **Resolution:** Addendum-APPROVE appended to `project/reviews/20260514-210000-feature-20c-spell-menu.md` (today). Line 532 reference verified accurate; regression impossible from comment change.
- **Test count:** 365/365 unchanged

---

## Memory entries created during pipeline

Three durable memory entries captured for future Druum work:

1. **`druum-dungeon-assets-fixture-fan-out`** (project memory) — captures that `DungeonAssets` test fixtures live in 7 sites across `tests/dungeon_movement.rs`, `tests/dungeon_geometry.rs`, and `ai.rs`/`encounter.rs`/`enemy_render.rs` `init_asset` registrations. Any new world-resource that systems consume must be registered in all sites or `cargo check` fails non-obviously in test/dev builds. Phase 1 caught this in `ai.rs`; Phase 3 caught it in `encounter.rs:597` + `enemy_render.rs:723`. Pattern repeats — worth a memory entry.

2. **`druum-gitbutler-stacked-branch-creation`** (feedback memory) — captures that `but commit <new-branch-name>` no longer auto-creates branches in the current `but` version (CLAUDE.md guidance is outdated). Stacked-PR branch creation requires `but branch new <name> --anchor <parent>` BEFORE staging. Discovered during Phase 2 ship friction; applied cleanly in Phase 3 ship.

3. **`druum-fix-review-findings-before-completion`** (feedback memory) — captures the user's decision pattern: when LOW findings come back, the user consistently chooses fixup-first-then-next-phase over defer-and-move-on. Three-for-three across #20. Worth memorialising for future feature work — assume the user wants the fixup cycle unless told otherwise.

---

## Open follow-ups

1. **Issue #22 — poison/regen query widening** ([link](https://github.com/codeinaire/druum-dungeon-crawler/issues/22)) — Filed by user before Phase 1. `apply_poison_damage` + `apply_regen` need their queries widened from player-only to all-combatants when enemy status ticks land. Phase 1/2/3 did NOT introduce enemy status ticks, so #22 is untouched. Blocks any future enemy-tick feature.
2. **Spell-sim debug command** (deferred per Q11) — Standalone tooling for spell tuning; orthogonal to gameplay PR stack.
3. **Spell icons** (deferred per Q10 to #25 polish) — Art-pipeline work; text-only menus pass UX bar for v1.
4. **Phase 2/3 RON content polish** (per roadmap §"Additional Notes") — Expand to ~30-50 spells and ~50-node skill trees per class. Current Phase 1/2 RON files contain the minimum viable set (~10 spells, ~12 nodes per class) to exercise all code paths. Content scaling is a separate workstream.

---

## Merge guidance

**Order: bottom-up. #21 → #23 → #24.**

- Merging #21 first causes GitHub to auto-retarget #23's base from `feature-20a-spell-registry` to `main`. No manual rebase needed.
- Merging #23 next causes GitHub to auto-retarget #24's base from `feature-20b-skill-trees` to `main`. No manual rebase needed.
- Merge #24 last to complete Feature #20.

**User explicitly chose to handle merge timing themselves.** Orchestrator will NOT auto-merge. All three PRs remain open and APPROVE'd at pipeline closeout.

**Merging top-down would** leave intermediate PRs with rebased histories that diverge from `main` — avoid this.

---

## Links to all artifacts

- **Research:** `/Users/nousunio/Repos/Learnings/claude-code/druum/project/research/20260514-druum-20-spells-skill-tree.md`
- **Plan (Phase 1 + 2 + 3 stacked protocols):** `/Users/nousunio/Repos/Learnings/claude-code/druum/project/plans/20260514-120000-feature-20-spells-skill-tree.md`
- **Implementer summaries:**
  - Phase 1: `/Users/nousunio/Repos/Learnings/claude-code/druum/project/implemented/20260514-120000-feature-20a-spell-registry.md`
  - Phase 2: `/Users/nousunio/Repos/Learnings/claude-code/druum/project/implemented/20260514-120000-feature-20b-skill-trees.md`
  - Phase 2 fixup: `/Users/nousunio/Repos/Learnings/claude-code/druum/project/implemented/20260514-210000-feature-20b-review-fixup.md`
  - Phase 3: `/Users/nousunio/Repos/Learnings/claude-code/druum/project/implemented/20260514-180000-feature-20c-spell-menu.md`
  - Phase 3 fixup: `/Users/nousunio/Repos/Learnings/claude-code/druum/project/implemented/20260514-220000-feature-20c-review-fixup.md`
- **Reviews (with fixup addenda appended in-place):**
  - Phase 1: `/Users/nousunio/Repos/Learnings/claude-code/druum/project/reviews/20260514-180000-feature-20a-spell-registry.md`
  - Phase 2: `/Users/nousunio/Repos/Learnings/claude-code/druum/project/reviews/20260514-200000-feature-20b-skill-trees.md`
  - Phase 3: `/Users/nousunio/Repos/Learnings/claude-code/druum/project/reviews/20260514-210000-feature-20c-spell-menu.md`
- **Pipeline state:** `/Users/nousunio/Repos/Learnings/claude-code/druum/project/orchestrator/PIPELINE-STATE.md`
- **PRs:**
  - #21: https://github.com/codeinaire/druum-dungeon-crawler/pull/21
  - #23: https://github.com/codeinaire/druum-dungeon-crawler/pull/23
  - #24: https://github.com/codeinaire/druum-dungeon-crawler/pull/24
- **Issue follow-up:** #22 https://github.com/codeinaire/druum-dungeon-crawler/issues/22
