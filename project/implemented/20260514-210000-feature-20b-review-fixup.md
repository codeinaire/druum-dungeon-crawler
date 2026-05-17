# Implementation: Feature #20b Review Fixup (PR #23)

**Plan:** (inline — no separate plan file; source of truth is review doc)
**Review doc:** `/Users/nousunio/Repos/Learnings/claude-code/druum/project/reviews/20260514-200000-feature-20b-skill-trees.md`
**File modified:** `src/plugins/town/guild_skills.rs`
**Date:** 2026-05-14

## Steps Completed

### Fix A — LOW #1: `# Precondition` doc-comment on `sorted_nodes`

**Location:** Lines 160-170 (`sorted_nodes` function signature is at line 171)

Added a `# Precondition` rustdoc H1 section to the `sorted_nodes` doc-comment explaining:
1. The tree MUST be cycle-free (validated by `validate_no_cycles`)
2. Calling on a cyclic tree causes infinite recursion in `node_depth`
3. Both production call sites guard with `if tree.nodes.is_empty()` after `validate_skill_trees_on_load` empties cyclic trees
4. Recommendation for test fixture authors to run `validate_no_cycles` + `clamp_skill_tree` first

Verbatim base from review lines 101-111; extended with the test-fixture guidance per plan point 5.

### Fix B — LOW #2: Tamper guard in `node_state` `SpInsufficient` re-check arm

**Location:** Lines 114-125 (`Err(SkillError::Insufficient)` arm)

Replaced the two-condition `if prereqs_met && level_met` check with a three-condition check adding:
```rust
let invariant_ok = experience.unspent_skill_points <= experience.total_skill_points_earned;
if prereqs_met && level_met && invariant_ok { ... }
```

Applied verbatim from review lines 126-138. When `unspent > total_earned`, node now shows `Locked(SkillError::Insufficient)` (grey) instead of yellow `SpInsufficient` on tampered saves.

### Optional smoke test added

**Location:** Lines 578-591 (`node_state_returns_locked_when_invariant_violated`)

Added one test confirming `node_state` returns `Locked(SkillError::Insufficient)` when `unspent (5) > total_earned (3)` even though prereqs and level are both met. This exercises the new `invariant_ok` guard directly.

Test count goes from 363 → 364.

## LOC Delta

- `sorted_nodes` doc-comment: +9 lines
- `node_state` tamper guard: +2 lines (one comment, one bool binding; net ±0 on condition count)
- Smoke test: +15 lines
- Total: ~+26 lines to `guild_skills.rs`

## No-change verification

Only `src/plugins/town/guild_skills.rs` was modified. No other files touched.

## Commit message

Written to: `/Users/nousunio/Repos/Learnings/claude-code/druum/project/shipper/feature-20b-fixup-commit-msg.txt`

Subject: `docs(town): address review findings (#23) — node_depth precondition + tamper guard`

Co-author trailer matches Phase 1 fixup (`feature-20a-fixup-commit-msg.txt`) exactly.

## Verification gate

- `cargo check` — expected green (no new types, no API changes)
- `cargo test --lib` — expected 364 (363 base + 1 new smoke test)
- `cargo clippy --all-targets -- -D warnings` — expected green (no new warnings introduced; `invariant_ok` binding is immediately consumed by the `if` condition, no unused-variable risk)

## Deviations from plan

None. Both fixes applied verbatim from review. Smoke test added (plan said optional, accepted based on test-density conventions — all `node_state` paths are now covered).

## Ship instructions

User to manually stage with `but rub zz feature-20b-skill-trees`, then:
```
but commit --message-file project/shipper/feature-20b-fixup-commit-msg.txt
but push -u origin feature-20b-skill-trees
```
