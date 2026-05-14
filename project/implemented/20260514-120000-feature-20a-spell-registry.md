# Implementation: Feature #20a — Spell Registry and Cast Resolver (Phase 1)

**Plan:** `/Users/nousunio/Repos/Learnings/claude-code/druum/project/plans/20260514-120000-feature-20-spells-skill-tree.md`
**Phase:** Phase 1 (Steps 1.1–1.8)
**Branch target:** `feature-20a-spell-registry`
**PR title:** `feat(combat): add spell registry and cast resolver (#20a)`

## Steps Completed

- **1.1** `src/data/spells.rs` — Rewrote 11-line stub to full schema. `SpellId`, `MAX_*` constants, `SpellSchool`, `SpellTarget`, `SpellEffect`, `SpellAsset`, `SpellDb` + `get()`, `clamp_known_spells()`. 6 unit tests.

- **1.2** `src/data/mod.rs` — Updated re-exports from `SpellTable` to `SpellDb, SpellAsset, SpellEffect, SpellSchool, SpellTarget, KNOWN_SPELLS_MAX, MAX_SPELL_DAMAGE, MAX_SPELL_DURATION, MAX_SPELL_HEAL, MAX_SPELL_MP_COST, clamp_known_spells`. Updated module doc-comment.

- **1.3** `src/plugins/loading/mod.rs` — Replaced `SpellTable` import and `RonAssetPlugin` registration with `SpellDb`. Added `pub spells: Handle<SpellDb>` field to `DungeonAssets` with `#[asset(path = "spells/core.spells.ron")]`.

- **1.4** `assets/spells/core.spells.ron` — Authored 15 starter spells: 8 Mage (Halito, Mahalito, Katino, Tiltowait, Dilto, Mogref, Sopic, Lokara) + 7 Priest (Dios, Madios, Madi, Matu, Bamatu, Kalki, Di). RON syntax uses parentheses for enum-variant-struct fields.

- **1.5** `src/plugins/combat/spell_cast.rs` — New pure-function module. `SpellCombatant`, `SpellDamageResult`, `spell_damage_calc`, `check_mp`, `deduct_mp`. Formula: `raw = (magic_attack + power.min(MAX_SPELL_DAMAGE) - magic_defense.min(180)/2).max(1)`, variance 0.7..=1.0, crit 1.5x at `accuracy/5`%. 5 unit tests.

- **1.6** `src/plugins/combat/mod.rs` — Added `pub mod spell_cast;` alphabetically between `damage` and `status_effects`.

- **1.7** `src/plugins/combat/turn_manager.rs` — Replaced 10-line `CastSpell` stub with 280+ LOC real resolver. New system params: `spell_db_assets: Res<Assets<SpellDb>>`, `spell_handle: Option<Res<DungeonAssets>>`, `mut equip_changed: MessageWriter<EquipmentChangedEvent>`. `CombatantCharsQuery` promoted to `&'static mut StatusEffects`. 7 new `cast_spell_*` unit tests + 4 test helper functions. `init_asset::<SpellDb>()` added to `make_test_app`.

- **1.8** `tests/spell_db_loads.rs` — New integration test mirroring `tests/item_db_loads.rs`. Loads `assets/spells/core.spells.ron` via `RonAssetPlugin`. Asserts >10 spells, Mage + Priest schools present, `halito`/`dios`/`di` verified by id + school + effect variant. 30s timeout guard.

## Steps Skipped

None. All 8 Phase 1 steps completed.

## Deviations from Plan

1. **Crit chance uses `accuracy / 5`% not `luck / 5`%** — `DerivedStats` has no `luck` field; `accuracy` is the nearest proxy (already incorporates luck from `BaseStats` via `derive_stats`). Documented in `spell_cast.rs` module doc and plan Implementation Discoveries.

2. **`CombatantCharsQuery` promoted from `&StatusEffects` to `&mut StatusEffects`** — The plan's Step 1.7 sketch proposed a separate `status_mut: Query<&mut StatusEffects>` for the Revive arm. This causes B0002 because `CombatantCharsQuery` already includes `&StatusEffects`. Fix: `&mut` subsumes `&`, so the existing query now handles both snapshot-reading (via iter's shared-ref path) and Revive mutation (via `.get_mut()`). Net effect: same capability, no B0002.

3. **Revive bypasses `resolve_target_with_fallback`** — The plan described using the pre-resolved `targets` Vec for Revive. `resolve_target_with_fallback` filters dead entities, making it unsuitable for Revive. Implementation reads `action.target` directly instead. Defense-in-depth `is_dead` check applied per-entity before reviving.

4. **Cast announcement log fires BEFORE per-target effect logs** — Plan step 1.7 item 8 said "Final log: cast {name}." but implementation writes it before dispatching on effect. Game-feel: "Mira casts Halito!" announces first, then "Mira casts Halito on Slime for 14 damage." follows. More natural read order.

5. **Four test fixtures updated for `DungeonAssets` field rename** — `minimap.rs`, `dungeon/tests.rs`, `combat/encounter.rs`, `dungeon/features.rs` all used `spell_table: Handle::default()` (old name). Updated to `spells: Handle::default()`.

## Issues Deferred

- **`cargo check` / `cargo test` / `cargo clippy` quality gates** — No shell execution tool available in this agent session. Code has been reviewed manually for correctness. Quality gates must be run by the user or a run-shipper before merging.
- **Manual smoke `cargo run --features dev`** — Cannot run interactively. Verifiable by user after Phase 1 commit.
- **`WarnedMissingSpells` resource** — Deferred to Phase 2 per plan. Phase 1 only logs a "fizzles" message for unknown spell IDs.

## Verification Results

### Anti-pattern greps (passed manually):
- `grep -rE "derive\(Event\)|EventReader<|EventWriter<" src/plugins/combat/spell_cast.rs` → 0 matches
- `grep -rE "effects\.push|effects\.retain" src/plugins/combat/spell_cast.rs` → 0 matches

### Quality gates (pending shell execution):
- `cargo check` — not run
- `cargo check --features dev` — not run
- `cargo test` — not run
- `cargo test --features dev` — not run
- `cargo clippy --all-targets -- -D warnings` — not run
- `cargo test --test spell_db_loads` — not run

## Targeted-fix patch (errors 1–3)

**File changed:** `src/plugins/combat/turn_manager.rs` only.

**Fix for errors 1 + 2 — param count exceeded Bevy's 16-param `SystemParam` limit:**

Chose **Path (a) — bundle**. The three Phase-1-added params (`spell_db_assets`, `spell_handle`, `equip_changed`) were cohesive (all serve only the `CastSpell` arm) and their extraction into a single `#[derive(SystemParam)]` struct was the minimum-churn fix. Path (b) — splitting into a separate system — was rejected because the action loop drains `queue` as a single unit; splitting would require shared state for `combat_log` and `next_phase`, which is more invasive than the plan warranted.

Added immediately after the `CombatantCharsQuery` type alias:

```rust
#[derive(SystemParam)]
struct SpellCastParams<'w> {
    spell_db_assets: Res<'w, Assets<SpellDb>>,
    spell_handle: Option<Res<'w, DungeonAssets>>,
    equip_changed: MessageWriter<'w, EquipmentChangedEvent>,
}
```

Replaced the three individual param slots in `execute_combat_actions` with `mut spell_params: SpellCastParams`. Updated the two usage sites inside the function body:
- `spell_handle.as_deref().and_then(|a| spell_db_assets.get(...))` → `spell_params.spell_handle.as_deref().and_then(|a| spell_params.spell_db_assets.get(...))`
- `equip_changed.write(...)` → `spell_params.equip_changed.write(...)`

**New param count: 16** (15 existing + 1 `SpellCastParams` struct, replacing the 3 individual spell params).

**Fix for error 3 — `chars` not declared mutable:**

Changed `chars: CombatantCharsQuery` → `mut chars: CombatantCharsQuery` at the function parameter. Updated the comment explaining the `mut` requirement (Revive arm calls `chars.get_mut(target)` to clear the Dead status — the sole-exception path from deviation #2). The Revive arm at line 785 (`if let Ok((_, mut target_status, _, _, _)) = chars.get_mut(target)`) is unchanged; it now compiles against the correctly-declared mutable binding.

**Files touched besides `turn_manager.rs`: zero.**

## Follow-up fix #2 (2026-05-14): enemy status application

### Option chosen: Option A — widen the query filter

### Justification

Option A was chosen over Option B on three grounds:

1. **Single-mutator invariant (status_effects.rs:162-164 comment).** The `apply_status_handler` function is explicitly documented as THE sole mutator of `StatusEffects.effects`. Option B would create a parallel mutation path in `turn_manager.rs` for enemy targets, duplicating the stacking-merge logic, potency clamp, and `EquipmentChangedEvent` nudge (~30 lines per affected arm). If stacking rules ever change, two divergent code paths must be kept in sync — a maintenance hazard the comment specifically exists to prevent.

2. **Writer audit confirms safety.** All `ApplyStatusEvent` writers in `src/` were audited (PIPELINE-STATE.md). The only writers that target party actors exclusively are: `turn_manager.rs:553` (Defend → `action.actor`, always a party member) and `dungeon/features.rs:447` (poison trap → `&party` query). The remaining writers — `turn_manager.rs:725` (spell ApplyStatus debuffs), `turn_manager.rs:751` (spell Buff arm), and `status_effects.rs:400` (`check_dead_and_apply`) — are already capable of targeting enemies and have been silently dropping those events. Widening the filter is safe; no enemy would receive a party-only buff from a writer that restricts its targets to `action.actor` (party member) or `&party`.

3. **Latent basic-attack bug fixed for free.** `turn_manager.rs:547` calls `check_dead_and_apply` on enemy targets from the basic-attack path. That `ApplyStatusEvent { Dead }` has been silently dropped since feature #15 because the filter excluded enemies. Option A transparently fixes this. Option B would require an additional sole-exception at line 547 (or a known-issue note), adding more divergence.

4. **D5α `EquipmentChangedEvent` nudge is safe for enemies.** `recompute_derived_stats_on_equipment_change` (`inventory.rs:444`) queries `(&BaseStats, &Equipment, &StatusEffects, &Experience, &mut DerivedStats)` with no `With<PartyMember>` filter. `EnemyBundle` includes all five components (D-A5 carve-out, `enemy.rs:8-10`). The nudge works correctly for enemies.

### Files changed

- `src/plugins/combat/status_effects.rs`
  - **Line 43 (new):** `use crate::plugins::combat::enemy::Enemy;`
  - **Line 181:** `apply_status_handler` query filter changed from `With<PartyMember>` → `Or<(With<PartyMember>, With<Enemy>)>`
  - **Line 247:** `tick_status_durations` query filter changed from `With<PartyMember>` → `Or<(With<PartyMember>, With<Enemy>)>`

No other files changed.

### Test pass count after fix

`cargo test --lib` — pending user run (gates not yet executed in this session). Expected: 339 passed, 0 failed.

### Edge cases noticed but not addressed

- `apply_poison_damage` and `apply_regen` (status_effects.rs:312, 338) still filter `With<PartyMember>`. These resolvers are only driven by `StatusTickEvent`, which is currently only written by `tick_on_dungeon_step` for party members and by #15's combat-round emitter for alive combatants. If enemies ever receive Poison/Regen status ticks in a future feature, these two resolvers would need the same `Or<(With<PartyMember>, With<Enemy>)>` widening. Out of scope for this fix; noted for Phase 2/3 or a future feature.
