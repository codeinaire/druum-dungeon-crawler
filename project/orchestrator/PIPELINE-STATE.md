# Pipeline State

**Task:** Druum issue #20 — Spells & Skill Trees. Three-PR pipeline (Phase 1 / Phase 2 / Phase 3), each phase = research → plan → implement → review → ship cycle. Plan updated 2026-05-14 for three-PR split.
**Status:** in-progress — Phase 1 doc-fixup implementer complete, awaiting user gates+commit+push, then re-review
**Last Completed Step:** 5.1.fixup-impl (Phase 1 fixup: doc-only edits applied per Option-1 review-resolution path)
**Current Phase:** Phase 1 — fixup commit pending user-side commit+push, then targeted re-review, then Phase 2 start

## Artifacts

| Step | Description | Artifact                                 |
| ---- | ----------- | ---------------------------------------- |
| 1    | Research    | /Users/nousunio/Repos/Learnings/claude-code/druum/project/research/20260514-druum-20-spells-skill-tree.md |
| 2    | Plan        | /Users/nousunio/Repos/Learnings/claude-code/druum/project/plans/20260514-120000-feature-20-spells-skill-tree.md |
| 3    | Implement (Phase 1) | /Users/nousunio/Repos/Learnings/claude-code/druum/project/implemented/20260514-120000-feature-20a-spell-registry.md |
| 4    | Ship (Phase 1) | PR: https://github.com/codeinaire/druum-dungeon-crawler/pull/21, Branch: feature-20a-spell-registry, Commit: e343585 |
| 5    | Code Review (Phase 1) | /Users/nousunio/Repos/Learnings/claude-code/druum/project/reviews/20260514-180000-feature-20a-spell-registry.md — APPROVE / merge-as-is. 1 MEDIUM, 1 LOW. |
| 5.1.fixup-impl | Doc-only fixup impl (Phase 1, post-review) | Edits applied: `src/plugins/combat/turn_manager.rs:593-600` (MP-check invariant comment), `src/plugins/combat/status_effects.rs:319-320` and `347-348` (TODO(#22) markers). Commit msg at `/Users/nousunio/Repos/Learnings/claude-code/druum/project/shipper/feature-20a-fixup-commit-msg.txt`. |
| 5.1.fixup-ship | Fixup ship (Phase 1, user-side) | pending — user runs gates + `but rub zz feature-20a-spell-registry` + `but commit --message-file <fixup-msg-path>` + `but push feature-20a-spell-registry` |
| 5.1.fixup-review | Targeted re-review of fixup | pending — narrow scope: verify MP-check comment at `turn_manager.rs:594-607` + TODO markers at `status_effects.rs:319,346` |
| 3.2  | Implement (Phase 2, STACKED) | pending — branch `feature-20b-skill-trees` to be created FROM `feature-20a-spell-registry`, PR base `feature-20a-spell-registry` |
| 4.2  | Ship (Phase 2, STACKED) | pending — `gh pr create --base feature-20a-spell-registry` |
| 5.2  | Code Review (Phase 2) | pending |
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
- **Re-review scope:** Narrow — verify only the two doc-fixup sites; no full re-review.

## Phase 1 ship details (2026-05-14)

- **Branch:** `feature-20a-spell-registry` from `main`
- **Initial commit:** `e343585`
- **Fixup commit:** pending user-side push
- **PR:** https://github.com/codeinaire/druum-dungeon-crawler/pull/21
- **Files changed (initial):** 23
- **Files changed (fixup):** 2 (`turn_manager.rs`, `status_effects.rs`) — comments-only
- **GitHub PR number is 21 but feature/issue number is #20 Phase 1 (the "20a" suffix). PR #20 was merged feature #19. Roadmap is source of truth.**

## Phase 1 fixup-impl details (2026-05-14)

- **Fix A (MEDIUM):** `src/plugins/combat/turn_manager.rs:593-600` — 8-line invariant comment above the MP-check block in CastSpell arm. Explains snapshot-vs-live split, one-action-per-round invariant, and migration path (`derived_mut.get(actor)`) for future double-cast mechanics.
- **Fix B (LOW):** `src/plugins/combat/status_effects.rs:319-320` and `347-348` — `// TODO(#22): widen to Or<(With<PartyMember>, With<Enemy>)> when Phase 2 adds combat-round StatusTickEvent emitter for enemies — see PR #21 review.` above each of `apply_poison_damage` and `apply_regen`.
- **Commit message file:** `/Users/nousunio/Repos/Learnings/claude-code/druum/project/shipper/feature-20a-fixup-commit-msg.txt` — 1-line subject (`docs(combat): address review findings (#21) — MP-check invariant + TODO(#22) markers`) + 3-line body + `Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>` trailer matching `e343585`'s style.
- **No code changes, no new tests, no other files touched.** User to re-run gates.

## Phase 1 implementer deviations (carry forward to reviewer)

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

## Final gate matrix (user-verified 2026-05-14, pre-commit, applies to e343585)

| Gate | Result |
|---|---|
| `cargo check` | pass |
| `cargo check --features dev` | pass |
| `cargo test --lib` | pass 339/339 |
| `cargo test --lib --features dev` | pass 343/343 |
| `cargo test --test '*'` | pass 3/3 (spell_db_loads, item_db, equipping) |
| `cargo clippy --all-targets -- -D warnings` | pass |
| `cargo clippy --all-targets --features dev -- -D warnings` | pass |
| Anti-pattern greps on `spell_cast.rs` | zero matches |

**Fixup commit will require user to re-run gates before push (per user's Step 3 instruction).**

## Reviewer focus areas (user-specified, for initial Phase 1 review — historical)

1. **`apply_status_handler` widening** (`status_effects.rs:178-181`) — is `Or<(With<PartyMember>, With<Enemy>)>` filter safe across all current `ApplyStatusEvent` writers? Confirm no party-only buff (DefenseUp, AttackUp, SpeedUp) can be mistakenly applied to an enemy. Confirm `EquipmentChangedEvent` nudge at line 229 for `Dead` is justified by `EnemyBundle` having `Equipment`/`Experience` (per `enemy.rs:8-10`).
2. **`tick_status_durations` widening** (`status_effects.rs:243-247`) — same widening. `StatusTickEvent` is currently only written for party (per implementer audit). Verify nothing in dungeon-step or combat-round paths writes a tick event for enemies.
3. **The `SpellCastParams` SystemParam bundle** (`turn_manager.rs:96-107`) — does the `<'w>`-only lifetime work correctly against Bevy 0.18 conventions?
4. **5 deviations from plan** in `project/implemented/20260514-120000-feature-20a-spell-registry.md`. Particularly deviation #1 — crit chance uses `accuracy/5%`, not `luck/5%` — does it match damage model intent?
5. **Carry-forward concern**: `apply_poison_damage` + `apply_regen` (`status_effects.rs`) still PartyMember-only. Safe today but flag as known follow-up for any future enemy-poisonable feature. Tracked-issue-now vs. plan-note-sufficient?

## Resolution (2026-05-14)

**Option A chosen by implementer for the status-effect filter widening.** Justification:

1. Preserves the #14/#15 single-mutator invariant for `StatusEffects.effects`.
2. Writer audit confirms widening is safe (no party-only-buff writer targets enemies).
3. Transparently fixes the latent basic-attack `Dead`-on-enemy bug at `turn_manager.rs:547`.
4. `EnemyBundle` already includes `Equipment::default()`/`Experience::default()` per D-A5 carve-out, so `EquipmentChangedEvent` nudge for Dead works on enemies.

**Edge case flagged by implementer:** `apply_poison_damage` and `apply_regen` still filter `With<PartyMember>`. Currently safe because `StatusTickEvent` is only written for party members. If enemies ever need Poison/Regen tick resolution, those two resolvers need the same widening. Carry forward to Phase 2/3 reviewers and to #20 follow-up items. **2026-05-14: Issue #22 filed; TODO(#22) markers now in code (fixup).**

## Resume instructions

### After user completes fixup ship (gates + commit + push)

1. Orchestrator re-spawns `run-reviewer` with NARROW prompt: "verify the two fixup commits resolve the MEDIUM at `turn_manager.rs:594-607` and the LOW at `status_effects.rs:319,346`; no other re-review needed."
2. Output to `project/reviews/<dated>-feature-20a-fixup.md`.
3. If re-review passes: proceed to Phase 2 (stacked).
4. If re-review flags new issues: pause, surface to user.

### Phase 2 stacked-PR protocol (post fixup-review-pass)

1. **Update plan file** `project/plans/20260514-120000-feature-20-spells-skill-tree.md` Phase 2 section: record stacked-PR decision. Add note: "Phase 2 branches from `feature-20a-spell-registry` (NOT main). `gh pr create --base feature-20a-spell-registry`. When Phase 1 PR #21 merges, Phase 2's base auto-retargets to main via GitHub."
2. **Run planner** to refine Phase 2 scope (skill trees + SP allocation, per locked plan). Resolve all open questions in one batch.
3. **Pause for user confirmation** before implementer dispatch. Show Phase 2 plan delta + any Cat-C decisions before code is written.
4. **Phase 3 stacking decision:** confirm with user BEFORE Phase 3 ship.
