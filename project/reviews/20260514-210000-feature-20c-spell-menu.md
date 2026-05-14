# Review: Feature #20c — Functional SpellMenu UI (Phase 3)

**PR:** https://github.com/codeinaire/druum-dungeon-crawler/pull/24
**Branch:** `feature-20c-spell-menu` (stacked on `feature-20b-skill-trees`)
**Commit:** `9465fb2`
**Reviewed:** 2026-05-14

## Verdict: APPROVE

| Severity | Count |
| -------- | ----- |
| CRITICAL | 0     |
| HIGH     | 0     |
| MEDIUM   | 0     |
| LOW      | 1     |

## Files Reviewed (Full Coverage)

- `src/plugins/combat/ui_combat.rs` — full (painter + handler + both tests)
- `src/plugins/combat/turn_manager.rs` — `spell_cursor` field and surrounding context
- `src/plugins/party/mod.rs` — `spawn_default_debug_party` dev-party block
- `assets/spells/core.spells.ron` — all spell IDs cross-checked against dev-party defaults

Also inspected (gate-pass fixes only):
- `src/plugins/combat/encounter.rs:597` — `init_asset::<SpellDb>()` addition
- `src/plugins/combat/enemy_render.rs:722` — `init_asset::<SpellDb>()` addition

## Focus Area Verdicts

### 1. Silence gate preservation — CORRECT AND TESTED

`silence_blocks_spell_menu` (line 857): unchanged. The top-level `is_silenced` check at line 603 fires on every frame while `SpellMenu` is top-of-stack, before any other SpellMenu logic executes. No regression.

`silence_blocks_real_spell_menu` (line 926) is the new regression guard. It sets `current_mp: 50` so "halito" (mp_cost: 2) would pass the MP filter on a non-silenced actor — the Silence gate is the only barrier. Test correctly confirms the pop still fires with real `KnownSpells` present.

Schedule ordering: `handle_combat_input` is `Update`, `paint_combat_screen` is `EguiPrimaryContextPass` (runs after `Update`). Handler modifies state → painter sees the result. One-frame window before pop is intentional per plan ("defense-in-depth + same-frame UX feedback").

### 2. `WarnedMissingSpells` warn-once semantics — CORRECT

Both insertion sites (painter line 318, handler line 636) use only `.set.insert((id.clone(), actor_entity))`. No `.clear()`, no overwrite, no `.retain()`. Tuple is `(SpellId, Entity)` — one entry per `(spell_id, character)` pair. Matches Q9. No concurrent aliasing (systems in different schedules).

### 3. Cat-C-4 empty-castable paths — CORRECT

- `SpellMenuState::Empty` — component missing OR `known_spells.spells.is_empty()` → renders `"(no spells)"`.
- `SpellMenuState::NoCastable` — component present, spells non-empty, all filtered → renders `"(no castable spells)"`.

Neither auto-pops. Both show `[Esc] Back`. Symmetric UX as required.

### 4. Cat-C-5 SingleEnemy guard — CORRECT

`enemy_alive` built from live `enemies` query (line 676-680). On empty: logs `"{actor_name}: no valid targets for {spell.display_name}"` (line 682-688), returns without touching `menu_stack`. SpellMenu stays on top. Only `SingleEnemy` is guarded; `SingleAlly` deferred per plan.

### 5. Cat-C-6 saturating cursor — CORRECT

- Up: `saturating_sub(1)` — at 0, stays 0.
- Down: `(cursor + 1).min(castable.len().saturating_sub(1))` — at last index, stays. Empty list: ceiling = 0, cursor cannot exceed 0.
- Entry reset at `Main` arm case 2 (line 532) confirmed.
- Re-clamp on Confirm (line 667-669) is defensive and correct.

### 6. Borrow-checker deviation — CORRECT, SEMANTICS IDENTICAL

`SpellMenuState` enum built with owned data before the `FnOnce` closure. No references to `ResMut<WarnedMissingSpells>` or `Query<&KnownSpells>` cross the closure boundary. State build + closure are within one system invocation — no external observer of intermediate state. Cleaner than inline-in-closure.

### 7. Dev-party defaults — CORRECT

Spell IDs verified against `assets/spells/core.spells.ron`:

| Class   | Spell IDs           | Present in RON |
|---------|---------------------|----------------|
| Mage    | `halito`, `katino`  | Lines 9, 32    |
| Priest  | `dios`, `matu`      | Lines 105, 141 |
| Fighter | `[]` (default)      | N/A            |

`KnownSpells` in scope via module-level `pub use skills::{KnownSpells, ...}` (line 35). `.insert(known)` overwrite pattern correct. `#[cfg(feature = "dev")]` gate unchanged.

## Findings

### [LOW] `spell_cursor` not reset on Cancel — defensive documentation gap

**File:** `src/plugins/combat/ui_combat.rs:471-478`

**Issue:** Cancel does not reset `spell_cursor`. This is not a bug in normal play — the next SpellMenu entry via Main arm case 2 always resets it (line 532). But if any future code path pushes `MenuFrame::SpellMenu` directly (bypassing case 2), a stale cursor would persist silently.

**Fix (optional — comment is sufficient):** Add a comment to the Cancel block noting the "reset on entry, not on exit" contract:

```rust
// Cancel: pop submenu (top-of-stack only; Main does nothing).
// NOTE: spell_cursor is reset on *entry* to SpellMenu (Main arm case 2, line 532),
// not on exit. Any future code that pushes SpellMenu directly must reset it.
if actions.just_pressed(&MenuNavAction::Cancel) {
```

---

## Test Coverage Notes

365/365 lib tests, 369/369 with dev features, 8/8 integration tests (gate-pass verified by user). The two Phase 3 tests are well-constructed. Cat-C-4/5/6 paths are not unit-tested in isolation (would require a loaded SpellDb asset), but the logic is simple enough to verify by inspection. The Silence gate tests cover the highest-risk integration path.

## Summary

Phase 3 is clean. All seven focus areas pass. The `SpellMenuState` enum deviation is an improvement over the plan sketch. Dev-party spell IDs are verified against the RON file. The `init_asset::<SpellDb>()` gate-pass fixes follow the established pattern. The single LOW finding is a documentation gap, not a correctness issue.

**Merge recommendation: merge as-is.** The LOW finding can be addressed in a follow-up comment if desired — it is not blocking.

---

## GitHub Posting Status

Per feedback memory, `REQUEST_CHANGES` is blocked on own PRs. Review body written to `/tmp/pr24-review-body.md`. Post via:

```bash
gh pr review 24 --repo codeinaire/druum-dungeon-crawler --comment --body-file /tmp/pr24-review-body.md
```

---

## Merge Order (Bottom-up Stack)

Standard for stacked PRs:

1. Merge PR #21 (`feature-20a-spell-registry`) — already approved
2. Merge PR #23 (`feature-20b-skill-trees`) — already approved
3. Merge PR #24 (`feature-20c-spell-menu`) — this PR

Each PR's base must be updated to `main` after the one below it merges before merging the next.

---

## Addendum — Fixup Commit `f193962` (2026-05-14)

**Scope:** Re-review of `docs(combat): address review findings (#24) — spell_cursor reset path comment`

**What changed:** Comment-only addition at `src/plugins/combat/ui_combat.rs:471-472`. No code logic modified.

**LOW finding status:** RESOLVED.

The added comment:

```rust
// Cancel: pop submenu (top-of-stack only; Main does nothing).
// NOTE: spell_cursor is reset on *entry* to SpellMenu (Main arm case 2, line 532),
// not on exit. Any future code that pushes SpellMenu directly must reset it.
```

This is word-for-word the fix suggested in the LOW finding. The line 532 reference is accurate — `2 => {` is the Main arm case that sets `spell_cursor = 0` before pushing `MenuFrame::SpellMenu`. The forward-looking warning is correctly scoped to the bypass scenario identified in the finding.

**Regression check:** None possible — comment-only change. Gates confirmed green by author (`cargo check` ✓, `cargo test --lib` 365/365, `cargo clippy --all-targets -- -D warnings` ✓).

**Updated verdict: APPROVE — no open findings.**

| Severity | Count |
| -------- | ----- |
| CRITICAL | 0     |
| HIGH     | 0     |
| MEDIUM   | 0     |
| LOW      | 0 (was 1, now resolved) |

**GitHub posting:** Per feedback memory, own-PR `REQUEST_CHANGES` is blocked. Addendum posted via `gh pr review 24 --comment`.
