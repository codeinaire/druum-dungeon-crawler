# Implementation: Feature #20c Review Fixup (PR #24)

**Plan:** (inline — no separate plan file; source of truth is review doc)
**Review doc:** `/Users/nousunio/Repos/Learnings/claude-code/druum/project/reviews/20260514-210000-feature-20c-spell-menu.md`
**File modified:** `src/plugins/combat/ui_combat.rs`
**Date:** 2026-05-14

## Steps Completed

### Fix — LOW #1: `spell_cursor` reset path comment in Cancel arm

**Location:** Lines 470-472 (comment block immediately above `if actions.just_pressed(&MenuNavAction::Cancel)`)

Replaced the original single-line comment:

```rust
// Cancel: pop submenu (top-of-stack only; Main does nothing).
```

with the two-line comment block specified verbatim in the review:

```rust
// Cancel: pop submenu (top-of-stack only; Main does nothing).
// NOTE: spell_cursor is reset on *entry* to SpellMenu (Main arm case 2, line 532),
// not on exit. Any future code that pushes SpellMenu directly must reset it.
```

Line 532 reference verified before applying: `input_state.spell_cursor = 0;` is at `ui_combat.rs:532` inside the Main arm `2 =>` (Spell) case. No adjustment needed.

## LOC Delta

- Cancel arm comment: +2 lines
- Total: +2 lines to `src/plugins/combat/ui_combat.rs`

## No-change verification

Only `src/plugins/combat/ui_combat.rs` was modified. No code changes, no test changes.

## Commit message

Written to: `/Users/nousunio/Repos/Learnings/claude-code/druum/project/shipper/feature-20c-fixup-commit-msg.txt`

Subject: `docs(combat): address review findings (#24) — spell_cursor reset path comment`

Co-author trailer matches Phase 1 and Phase 2 fixup style exactly.

## Verification gate

- `cargo check` — green (comment-only, no compilation impact)
- `cargo check --features dev-party` — green
- `cargo test --lib` — 365/365 (unchanged)
- `cargo test --lib --features dev-party` — 369/369 (unchanged)
- `cargo test --test '*'` — 8/8 (unchanged)
- `cargo clippy --all-targets -- -D warnings` — green
- `cargo clippy --all-targets --features dev-party -- -D warnings` — green

## Deviations from plan

None. Comment applied verbatim from review LOW finding. Line 532 reference confirmed accurate before applying.

## Ship instructions

User to stage and commit to `feature-20c-spell-menu` branch:

```
but rub zz feature-20c-spell-menu
but commit --message-file project/shipper/feature-20c-fixup-commit-msg.txt
but push -u origin feature-20c-spell-menu
```
