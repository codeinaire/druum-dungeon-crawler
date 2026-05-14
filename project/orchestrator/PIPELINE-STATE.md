# Pipeline State

**Task:** Druum issue #20 — Spells & Skill Trees. Three-PR pipeline (Phase 1 / Phase 2 / Phase 3), each phase = research → plan → implement → review → ship cycle. Plan updated 2026-05-14 for three-PR split.
**Status:** in-progress — Phase 1 implementer reports 339/339 lib tests pass; awaiting user gate re-verification before manual ship
**Last Completed Step:** 3 (Phase 1 implement + 3 follow-up user fixes + Option-A widening fix for enemy status application)
**Current Phase:** Phase 1 — fix applied, ready for user-driven manual gate-check + ship

## Artifacts

| Step | Description | Artifact                                 |
| ---- | ----------- | ---------------------------------------- |
| 1    | Research    | /Users/nousunio/Repos/Learnings/claude-code/druum/project/research/20260514-druum-20-spells-skill-tree.md |
| 2    | Plan        | /Users/nousunio/Repos/Learnings/claude-code/druum/project/plans/20260514-120000-feature-20-spells-skill-tree.md |
| 3    | Implement (Phase 1) | /Users/nousunio/Repos/Learnings/claude-code/druum/project/implemented/20260514-120000-feature-20a-spell-registry.md |
| 4    | Ship (Phase 1) | BLOCKED — commit msg and PR body prepared at `project/shipper/feature-20a-commit-msg.txt` and `project/shipper/feature-20a-pr-body.md`. User must run `cargo` gates + GitButler commands manually after the test-failure fix lands. |
| 5    | Code Review (Phase 1) | pending |
| 3.2  | Implement (Phase 2) | pending — gated on Phase 1 user confirmation |
| 4.2  | Ship (Phase 2) | pending — branch `feature-20b-skill-trees` |
| 5.2  | Code Review (Phase 2) | pending |
| 3.3  | Implement (Phase 3) | pending — gated on Phase 2 user confirmation |
| 4.3  | Ship (Phase 3) | pending — branch `feature-20c-spell-menu` |
| 5.3  | Code Review (Phase 3) | pending |

## User Decisions

All locked 2026-05-14 (see plan §"User Decisions"):

- Q1 — Per-class trees: ALL three classes (Fighter passives-only).
- Q3 — MP regeneration: none new.
- Q6 — Skill points per level-up: 1 SP/level.
- Q9 — Missing spell ID: warn-once-per-(spell,character)-then-filter.
- Q10 — Spell icons: defer to #25 polish.
- Q11 — Spell-sim debug: defer to own PR.
- PR shape: THREE separate PRs.
- GH-issue reconciliation: no separate spec issue; roadmap is source of truth.

## Phase 1 implementer deviations (carry forward to reviewer)

1. Crit chance uses `accuracy / 5`% not `luck / 5`% — DerivedStats has no luck field.
2. `CombatantCharsQuery` uses `&'static mut StatusEffects` (B0002 prevention).
3. Revive bypasses `resolve_target_with_fallback` — that helper filters dead entities; Revive reads `action.target` directly with defense-in-depth `is_dead` check.
4. Cast announcement log fires BEFORE per-target effect logs (game-feel).
5. Four `DungeonAssets` test fixtures updated for `spell_table` → `spells` field rename.
6. **Targeted fix #1 (2026-05-14 follow-up):** `execute_combat_actions` param count exceeded Bevy's 16-tuple `SystemParam` ceiling (was 18). Three Phase-1 spell params (`spell_db_assets`, `spell_handle`, `equip_changed`) collapsed into private `#[derive(SystemParam)] struct SpellCastParams<'w>` in `turn_manager.rs`. Param count now 16 (at ceiling). Added `mut` to `chars: CombatantCharsQuery` for the Revive arm's `chars.get_mut(target)` call. (Note: user later found the actual root cause was a missing `use bevy::ecs::system::SystemParam;` import in `turn_manager.rs` — the bundle is still architecturally correct but the original cascade was the missing import.)

## User-applied follow-up fixes since previous handoff (uncommitted in working tree)

1. Added `use bevy::ecs::system::SystemParam;` to `src/plugins/combat/turn_manager.rs` (real root cause of "fn isn't IntoSystemSet" cascade — the derive macro path is NOT in `bevy::prelude::*`). Implementer MUST keep this import.
2. Added `app.init_asset::<crate::data::SpellDb>();` to `src/plugins/combat/ai.rs`'s `make_test_app` (3 AI tests panicked because `Res<Assets<SpellDb>>` was unregistered).
3. Renamed `spell_table: Handle::default()` → `spells: Handle::default()` in `tests/dungeon_movement.rs:153` and `tests/dungeon_geometry.rs:157`.

## Current gate status (user verified 2026-05-14)

| Gate | Result |
|---|---|
| `cargo check` | pass |
| `cargo check --features dev` | pass |
| `cargo clippy --all-targets -- -D warnings` | pass |
| `cargo clippy --all-targets --features dev -- -D warnings` | pass |
| Integration tests (`cargo test --test '*'`) | all 3 pass |
| Anti-pattern greps on `spell_cast.rs` | zero matches |
| `cargo test --lib` | implementer reports 339/339 after Option-A fix — user to re-verify |

## Outstanding test failures (re-spawn implementer to resolve)

**Failing tests:**
1. `plugins::combat::turn_manager::app_tests::cast_spell_damage_applies_hp_loss_and_dead_status` (line 1875): enemy reaches `current_hp == 0` but `StatusEffects::has(Dead)` returns false.
2. `plugins::combat::turn_manager::app_tests::cast_spell_apply_status_writes_event` (line 1972): Silence never lands on enemy target.

**Root cause (verified by orchestrator):**

`apply_status_handler` at `src/plugins/combat/status_effects.rs:177-180` filters `Query<&mut StatusEffects, With<PartyMember>>`. Enemies don't have `PartyMember`, so `characters.get_mut(ev.target)` returns Err and any `ApplyStatusEvent` targeting an enemy is silently dropped. `tick_status_durations` at line 246 has the same filter.

**Important: this is a latent pre-Phase-1 bug, not a Phase-1 regression.** `turn_manager.rs:547` (basic-attack path) also calls `check_dead_and_apply` on enemy targets, which writes `ApplyStatusEvent { Dead }` — that has been silently dropping since before #20. Nobody noticed because `check_victory_defeat_flee` reads `current_hp == 0` directly, not the Dead status. Phase 1's new tests are the first to assert on the Dead status itself.

**Writer audit (relevant to fix scope):**

All `ApplyStatusEvent` writers in `src/`:
- `turn_manager.rs:553` — Defend, targets `action.actor` (party member only via combat menu)
- `turn_manager.rs:725` — spell ApplyStatus arm, targets resolved-target (can be enemy via debuffs like Silence)
- `turn_manager.rs:751` — spell Buff arm, targets resolved-target (normally allies; potency clamp protects against misuse)
- `status_effects.rs:400` — `check_dead_and_apply` helper, targets entity passed in (Dead, called for damaged enemies too)
- `status_effects.rs:*` lines 508-899 — all inside `#[cfg(test)]` module
- `dungeon/features.rs:447` — Poison trap, targets only `&party` (enemies impossible)

**Three options surfaced to user; user preference is Option B but asked for implementer to pick + justify:**

- **Option A (widen filter):** Change `apply_status_handler` and `tick_status_durations` queries to `Query<&mut StatusEffects, Or<(With<PartyMember>, With<Enemy>)>>`. Pro: single-source-of-truth for status logic (stacking merge, potency clamp, Stone/Dead permanence, EquipmentChangedEvent nudge for stat-modifier removal — all reused for enemies for free). Also fixes the latent basic-attack `Dead`-on-enemy bug. The writer audit shows party-only buffs reach the handler only via writers that target party members (`action.actor` in Defend), so widening the filter is safe.
- **Option B (sole-exception in turn_manager):** Add a `chars.get_mut(target)` block in the spell Damage/ApplyStatus arms that mutates enemy `StatusEffects` directly when the target lacks `PartyMember`. Mirrors the existing Revive pattern. Con: duplicates ~30 lines of stacking/clamping/nudge logic from `apply_status_handler` per arm and per writer site, and does NOT fix the latent basic-attack bug at line 547 unless duplicated there too. Maintenance hazard if buff stacking rules change.
- **Option C (drop Dead-on-enemy entirely):** Rely solely on `current_hp == 0` for enemy death; update failing tests; remove ApplyStatus effect for enemies. User flagged this as a gameplay regression (debuff spells become no-ops on enemies). Reject.

**Orchestrator recommendation:** Option A. Reasoning: (a) it matches the existing #14/#15 invariant that `apply_status_handler` is THE single mutator of `StatusEffects.effects` (status_effects.rs:162-164 comment); breaking that invariant with sole-exceptions creates two divergent code paths for the same operation. (b) The writer audit shows widening is safe — no writer would incorrectly buff enemies. (c) It transparently fixes the latent basic-attack `Dead`-on-enemy bug. (d) `EnemyBundle` already includes `Equipment::default()`+`Experience::default()` specifically so the `recompute_derived_stats_on_equipment_change` path works for enemies (see `enemy.rs:8-10` and the D-A5 carve-out), so the `EquipmentChangedEvent` nudge for Dead is harmless on enemies.

But the user explicitly said the implementer should **pick and justify**, so the implementer should weigh both and proceed with whichever they can justify on the merits.

## Resolution (2026-05-14)

**Option A chosen by implementer.** Files modified:

- `src/plugins/combat/status_effects.rs:43` — added `use crate::plugins::combat::enemy::Enemy;`
- `src/plugins/combat/status_effects.rs:181` — widened `apply_status_handler` filter to `Or<(With<PartyMember>, With<Enemy>)>`
- `src/plugins/combat/status_effects.rs:247` — widened `tick_status_durations` filter to `Or<(With<PartyMember>, With<Enemy>)>`
- `project/implemented/20260514-120000-feature-20a-spell-registry.md` — appended "Follow-up fix #2" section

Implementer's justification (full text in summary):

1. Preserves the #14/#15 single-mutator invariant for `StatusEffects.effects`.
2. Writer audit confirms widening is safe (no party-only-buff writer targets enemies).
3. Transparently fixes the latent basic-attack `Dead`-on-enemy bug at `turn_manager.rs:547`.
4. `EnemyBundle` already includes `Equipment::default()`/`Experience::default()` per D-A5 carve-out, so `EquipmentChangedEvent` nudge for Dead works on enemies.

**Edge case flagged by implementer:** `apply_poison_damage` and `apply_regen` still filter `With<PartyMember>`. Currently safe because `StatusTickEvent` is only written for party members. If enemies ever need Poison/Regen tick resolution, those two resolvers need the same widening. Carry forward to Phase 2/3 reviewers and to #20 follow-up items.

## Resume instructions (after user gate re-verification)

1. User re-runs `cargo test --lib` (expect 339/339), `cargo clippy --all-targets -- -D warnings` (expect clean), `cargo check --features dev`, `cargo clippy --all-targets --features dev -- -D warnings`, `cargo test --test '*'`.
2. User stages + commits + pushes via GitButler per `project/shipper/feature-20a-commit-msg.txt` and `project/shipper/feature-20a-pr-body.md`, then opens PR.
3. User reports PR URL back to orchestrator.
4. Orchestrator updates Ship row with PR URL + branch, sets `Last Completed Step: 4`, runs `run-reviewer` with PR URL (Step 5).
5. After review, surface findings to user. Pause for user go-ahead before Phase 2.
