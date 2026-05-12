# Implementation Summary: Feature #18b — Town Hub: Temple + Guild

**Date:** 2026-05-12
**Plan:** project/plans/20260512-173000-feature-18b-town-temple-guild.md
**Branch target:** feature/18b-town-temple-guild

## Steps Completed

### Phase 1 — Data
- Edited `assets/town/core.town_services.ron`: added `temple_revive_cost_base: 100`, `temple_revive_cost_per_level: 50`, `temple_cure_costs: [(Stone, 250), (Paralysis, 100), (Sleep, 50)]`.
- Edited `src/data/town.rs`: added `MAX_TEMPLE_COST = 100_000` and `MAX_RECRUIT_POOL = 32` constants with doc-comments; added `clamp_recruit_pool` helper mirroring `clamp_shop_stock`; added 3 new tests (`recruit_pool_size_clamped_truncates_oversized`, `clamp_recruit_pool_passes_through_small_pool`, `town_services_round_trips_with_authored_temple_fields`).

### Phase 2 — Temple module
- Created `src/plugins/town/temple.rs` (~285 LOC including tests) with:
  - `TempleState` resource, `TempleMode` enum
  - `revive_cost`, `cure_cost`, `first_curable_status` pure helpers (all unit-tested)
  - `paint_temple` painter (read-only, EguiPrimaryContextPass)
  - `handle_temple_action` Update handler with `#[allow(clippy::too_many_arguments)]`
  - 15 tests covering all specified scenarios including Revive/Cure rejection, gold deduction, Dead-not-curable by Cure mode

### Phase 3 — Guild module
- Created `src/plugins/town/guild.rs` (~680 LOC including tests) with:
  - `DismissedPool` resource (with #23 deferral doc-comment), `GuildState` resource, `GuildMode` enum
  - `next_free_slot` pure helper (unit-tested)
  - `paint_guild` painter (read-only)
  - 5 handler systems: `handle_guild_input`, `handle_guild_recruit`, `handle_guild_dismiss`, `handle_guild_row_swap`, `handle_guild_slot_swap`
  - 15 tests covering recruit/dismiss/row-swap/slot-swap scenarios, Pitfall 2 deferred-command regression, and inventory preservation

### Phase 4 — Placeholder removal
- Removed `pub mod placeholder;` from `src/plugins/town/mod.rs`. The physical file `src/plugins/town/placeholder.rs` was not deleted (no Bash access). Since Rust only compiles declared modules, the file is dead and its tests are no longer run. Physical deletion is required before merge.

### Phase 5 — TownPlugin wiring
- Edited `src/plugins/town/mod.rs`:
  - Module doc-comment updated (removed placeholder mention)
  - `pub mod guild;` and `pub mod temple;` added (alphabetical)
  - `pub mod placeholder;` removed
  - Old `placeholder` imports removed; `guild` and `temple` imports added
  - `TownPlugin::build` extended: `.init_resource::<TempleState>().init_resource::<GuildState>().init_resource::<DismissedPool>()`
  - Painter tuple: `paint_template.run_if(Temple)` + `paint_guild.run_if(Guild)` replace single `paint_placeholder`
  - Handler tuple: 6 new handlers replace `handle_placeholder_input`
  - `mod tests` updated: new imports, `PartySize`/`ButtonInput<KeyCode>` initialized, new handlers registered

## Steps Skipped

None.

## Deviations from Plan

1. **Phase 4 — `placeholder.rs` not physically deleted**: No shell/Bash access available. File is removed from module tree (`pub mod placeholder;` removed), making it dead code that Rust never compiles. Physical deletion must be done via shell: `rm src/plugins/town/placeholder.rs`.

2. **`ButtonInput<KeyCode>` test helpers use `.clear()` not `.release()`**: In MinimalPlugins tests, `ButtonInput::just_pressed` is never auto-cleared (no `InputPlugin`). Used `.clear()` after the first update to prevent double-firing. This is the correct pattern for raw `ButtonInput<KeyCode>` testing.

3. **Slot-swap reads from pre-collected vec instead of `query.get()`**: `handle_guild_slot_swap` reads slot values from the pre-sorted `Vec<(Entity, usize)>` instead of calling `party.get(entity)` a second time. This avoids any ambiguity about sequential read/write borrows on the mutable query.

## Deferred Issues

1. **Physical deletion of `src/plugins/town/placeholder.rs`**: Needs `rm src/plugins/town/placeholder.rs` before the branch is merged.
2. **`DismissedPool` `MapEntities` implementation**: Deferred to Feature #23 per plan.
3. **Status-pick dialog for multi-status characters**: Deferred to Feature #25 polish per plan.
4. **Re-recruit from `DismissedPool`**: Currently, recruit always spawns a fresh entity. Re-use of dismissed entities is deferred to Feature #19+.

## Verification Status

All 6 quality gates require orchestrator execution (implementer has no Bash access):

```bash
# Phase 4 physical cleanup first:
rm /Users/nousunio/Repos/Learnings/claude-code/druum/src/plugins/town/placeholder.rs

# Quality gates:
cd /Users/nousunio/Repos/Learnings/claude-code/druum
cargo check
cargo check --features dev
cargo test
cargo test --features dev
cargo clippy --all-targets -- -D warnings
cargo clippy --all-targets --features dev -- -D warnings
```

Expected test delta: +17 temple + +15 guild + +3 data/town - 2 placeholder = net +33 tests.
Expected baseline: ~293 lib tests (default), ~297 (--features dev), 6 integration tests each.
