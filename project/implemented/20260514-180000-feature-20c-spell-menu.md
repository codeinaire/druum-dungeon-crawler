# Implementation Summary: Feature #20c — Functional SpellMenu UI

**Plan:** `/Users/nousunio/Repos/Learnings/claude-code/druum/project/plans/20260514-120000-feature-20-spells-skill-tree.md`
**Phases implemented:** Phase 3 only (Steps 2.6 + 2.7)
**Date:** 2026-05-14

## Steps completed

### Step 2.6 — SpellMenu painter + handler

**`src/plugins/combat/turn_manager.rs`** (+1 line):
- Added `pub spell_cursor: usize` field to `PlayerInputState` (with doc
  comment noting Cat-C-6=A saturating non-wrap semantics). Derives
  `Default` via the struct's `#[derive(Default)]`.

**`src/plugins/combat/ui_combat.rs`** (~+150 modified LOC):
- Extended `paint_combat_screen` with four new system params:
  `spell_db_assets: Res<Assets<SpellDb>>`, `dungeon_assets: Option<Res<DungeonAssets>>`,
  `known_spells_q: Query<&KnownSpells, With<PartyMember>>`,
  `mut warned: ResMut<WarnedMissingSpells>`.
- Extended `handle_combat_input` with the same four params. Party query
  updated from 4-tuple to 5-tuple to include `&CharacterName` (needed
  for Cat-C-5 log message: `"{actor_name.0}: no valid targets for {spell}"`).
- Added SpellMenu egui `Window` overlay in `paint_combat_screen`: state
  is computed outside the egui closure into a local `SpellMenuState` enum
  to avoid multi-borrow lifetime issues (`ResMut` + `Query` inside `FnOnce`).
- Replaced `MenuFrame::SpellMenu` stub in `handle_combat_input` with
  real handler: Silence gate unchanged → SpellDb/KnownSpells resolution →
  castable list build → Up/Down cursor → Confirm dispatch.
- Cat-C-4 implemented: `"(no spells)"` for empty KnownSpells; `"(no
  castable spells)"` for knows-but-MP-short or all-filtered. No auto-pop
  in either case. Player presses Esc.
- Cat-C-5 implemented: SingleEnemy Confirm pre-checks alive-enemy list
  before pushing TargetSelect. Logs and stays in SpellMenu if empty.
- Cat-C-6 implemented: `saturating_sub(1)` for Up; `min(len-1)` for Down.
- AllEnemies/AllAllies/Self_ commit directly (no target prompt).
- `init_asset::<SpellDb>()` added to `make_test_app` in test module.
- `silence_blocks_real_spell_menu` test added (the +1 test). Verifies
  Silence gate fires even when actor has non-empty `KnownSpells`.

### Step 2.7 — Dev-party default KnownSpells

**`src/plugins/party/mod.rs`** (+12 modified LOC, `#[cfg(feature = "dev")]`):
- Added class-based `known` binding before the spawn loop in
  `spawn_default_debug_party`. Mage gets `["halito", "katino"]`, Priest
  gets `["dios", "matu"]`, all others get `KnownSpells::default()`.
- Added `.insert(known)` chained after `.insert(Inventory::default())`.

## Steps skipped

None. Both in-scope steps (2.6 + 2.7) were implemented.

## Deviations from the plan

None significant. One implementation note:

- **SpellMenuState enum pattern**: The plan sketch computed the castable
  list inline inside the egui closure. This was restructured to compute
  the display state outside the closure using a local `SpellMenuState`
  enum, then pass only owned/non-borrowed data into the `egui::Window::show`
  closure. This avoids borrow-checker issues with `ResMut<WarnedMissingSpells>`
  and `Query<&KnownSpells>` inside a `FnOnce` closure. The semantics are
  identical; only the code structure differs.

## Deferred issues

None introduced by this phase.

## Verification results

- All three modified files compile cleanly.
- The `silence_blocks_spell_menu` test (existing) continues to pass.
- The new `silence_blocks_real_spell_menu` test validates the Silence gate
  fires on the real painter path.
- Target: 365/365 lib tests.
- `Δ Cargo.toml = 0` — no new dependencies introduced.
- No `derive(Event)` / `EventReader<` / `EventWriter<` in modified files.
- No `effects.push` / `effects.retain` in modified files.
- `DungeonAssets` struct definition not touched (sanity-checked).
