# feat(combat): functional spell menu and end-to-end casting (#20c)

## Scope

Phase 3 of Feature #20 — Spells & Skill Trees. Replaces the
`MenuFrame::SpellMenu` stub with a functional two-pane menu (spell list
+ description) and grants the dev party class-appropriate default spells
so the smoke test "open SpellMenu → see Halito → cast it" works
without first visiting the Guild.

Stacked on `feature-20b-skill-trees` (PR #23). Phase 1 is PR #21.

## What changed

### `src/plugins/combat/turn_manager.rs` (+1 modified line)

Added `pub spell_cursor: usize` field to `PlayerInputState`. Defaults
to `0`. Reset on `MenuFrame::SpellMenu` entry in the Main arm's case 2
dispatch. Cat-C-6 = A (saturating non-wrap) matches `main_cursor` and
Guild `node_cursor`.

### `src/plugins/combat/ui_combat.rs` (~+150 modified LOC)

- Extended `paint_combat_screen` signature with four new params:
  `spell_db_assets`, `dungeon_assets`, `known_spells_q`, `mut warned`.
- Extended `handle_combat_input` signature with the same four params.
  Party query gains `&CharacterName` to resolve actor name for Cat-C-5
  log messages.
- `MenuFrame::SpellMenu` painter: renders an egui Window with
  cursor-highlighted `"SpellName (MP N)"` list + description pane.
  Display state computed outside the egui closure to avoid multi-borrow
  lifetime issues with `ResMut`.
- `MenuFrame::SpellMenu` handler: Silence gate unchanged (Decision 34).
  Up/Down moves `spell_cursor` with saturating semantics. Confirm
  dispatches based on `SpellTarget`:
  - `SingleEnemy` → Cat-C-5 alive-enemy pre-check, then push
    `TargetSelect`.
  - `SingleAlly` → push `TargetSelect`.
  - `AllEnemies` / `AllAllies` / `Self_` → commit directly, no target
    prompt.
- Added `init_asset::<SpellDb>()` to `make_test_app`.
- Added `silence_blocks_real_spell_menu` test (+1 test).

### `src/plugins/party/mod.rs` (+12 modified LOC)

`spawn_default_debug_party` (`#[cfg(feature = "dev")]`) now builds
class-appropriate `KnownSpells` and inserts it after
`.insert(Inventory::default())`:

| Character | Class | KnownSpells |
|---|---|---|
| Mira | Mage | `["halito", "katino"]` |
| Father Gren | Priest | `["dios", "matu"]` |
| Aldric, Borin | Fighter | `[]` (default) |

## Cat-C decisions

| Decision | Option chosen | Implementation |
|---|---|---|
| **Cat-C-4** (empty castable list) | **A — paint label, don't auto-pop** | Two sub-cases: `"(no spells)"` when `known_spells.spells.is_empty()`; `"(no castable spells)"` when knows spells but all filtered/MP-short. Player presses Esc. |
| **Cat-C-5** (SingleEnemy + all dead) | **A — pre-check at Confirm time** | Mirrors Attack arm guard at `turn_manager.rs:475-478`. If `enemy_alive.is_empty()`: logs `"{name}: no valid targets for {spell}"`, stays in SpellMenu. |
| **Cat-C-6** (cursor wrap) | **A — saturating non-wrap** | `saturating_sub(1)` for Up; `(cursor + 1).min(len - 1)` for Down. Consistent with `main_cursor` and Guild `node_cursor`. |

## Test deltas

| Phase | Baseline | Added | Total |
|---|---|---|---|
| Phase 1 (PR #21) | 345 | +19 | 364 |
| Phase 2 (PR #23) | 364 | 0 | 364 |
| **Phase 3 (this PR)** | **364** | **+1** | **365** |

The +1 test is `silence_blocks_real_spell_menu`: verifies the Silence
gate (Decision 34) fires on the real painter path (actor has non-empty
`KnownSpells`) and still pops `SpellMenu` to `Main`.

## Pre-merge checklist

- [ ] `cargo check` passes clean
- [ ] `cargo test --lib` → 365/365
- [ ] `cargo clippy --all-targets -- -D warnings` passes clean
- [ ] `cargo check --features dev` passes clean
- [ ] `cargo test --lib --features dev` → 365/365
- [ ] Manual smoke: `cargo run --features dev` — F9 to Dungeon, walk
  into encounter, cursor to Mira, select Spell, confirm Halito + Katino
  listed, pick Halito, target enemy, confirm damage in combat log.
- [ ] Silence smoke: inflict Silence on Mira, confirm SpellMenu
  auto-pops with "(silenced; cannot cast)" log.
- [ ] `git diff --stat Cargo.toml` → no changes (Δ = 0).

## Linked PRs

- PR #21 — Phase 1: spell registry + cast resolver (`feature-20a-spell-registry`)
- PR #23 — Phase 2: skill trees + Guild Skills (`feature-20b-skill-trees`)
- This PR (#Phase 3) stacks on #23.

After Phase 3 merges, follow-up work tracked separately:
- Feature #25 — spell icons (deferred per Q10)
- Spell-sim debug tooling (deferred per Q11)
- `NodeGrant::Resist` consumer-side check (deferred per Q4-resist)
