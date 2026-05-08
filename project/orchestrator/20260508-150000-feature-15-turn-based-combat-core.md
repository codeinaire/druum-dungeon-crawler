# Pipeline Summary — Feature #15: Turn-Based Combat Core (Research → Plan → Implement → Review)

**Date:** 2026-05-08
**Pipeline scope:** research → plan → implement → review → STOP. Shipping requires explicit user authorization.
**Status:** Pipeline complete. Verification GREEN. Review verdict **LGTM-with-changes** (2 MEDIUM findings, 0 CRITICAL/HIGH). Awaiting user direction: ship as-is with MEDIUM findings as follow-up, or run another implementer pass to address them first.

---

## Original task

Drive the full feature pipeline for **Feature #15: Turn-Based Combat Core** from the Druum (Bevy 0.18 first-person dungeon crawler) roadmap (`project/roadmaps/20260429-01-bevy-dungeon-crawler-roadmap.md` lines 789-862). Difficulty 4/5 — the largest single feature in the roadmap.

**In scope:** action-queue combat loop (research Pattern 5) — `CombatPhase::PlayerInput` → enemy AI fills queue → sort by `speed` → `CombatPhase::ExecuteActions` → `CombatPhase::TurnResult`. `damage_calc` as a pure function (Wizardry-style multiplicative formula). Stub `CastSpell` (full impl deferred to #20). `CurrentEncounter` populated via test fixtures (production triggers in #16). Three new sub-plugins (`TurnManagerPlugin`, `EnemyAiPlugin`, `CombatUiPlugin`) registered under existing `CombatPlugin`. Egui combat UI overlays existing dungeon camera (no new `Camera3d`). Boss AI stub (`BossFocusWeakest` + `BossAttackDefendAttack`) for #17 hook.

**Out of scope (deferred):**
- Full spell mechanics (#20 fills `CombatActionKind::CastSpell` body)
- Production encounter spawning (#16 owns `CurrentEncounter`)
- Animation tweens (#17)
- Per-instance enemy authoring (#17)
- Ailment-curing items (#20)
- Combat-round `StatusTickEvent` emission (one-line addition in `turn_manager::round_end`, ships with later combat polish)

**Constraint envelope (final):**
- **+8 new files** under `src/plugins/combat/`: `actions.rs`, `ai.rs`, `combat_log.rs`, `damage.rs`, `enemy.rs`, `targeting.rs`, `turn_manager.rs`, `ui_combat.rs`
- **3 carve-out edits** on previously frozen-area files: `combat/mod.rs` (sub-plugin registrations), `party/inventory.rs` (D-A5: dropped `With<PartyMember>` filter), `Cargo.toml` (rand 0.9 direct dep + rand_chacha 0.9 dev-dep)
- **0 edits** required to `dungeon` test harnesses — `CombatPlugin` already registered there per #14 D-I10/D-I11
- **+1 direct dep** (`rand = "0.9"` with `["std", "std_rng", "small_rng", "os_rng"]` features), **+1 dev-dep** (`rand_chacha = "0.9"`)
- **187 default tests / 190 dev-feature tests** (vs 159 / 162 after #14 — net +28 tests, within plan envelope of +25-30)
- **LOC delta:** ~1200-1400 (within plan budget of +1000-1800)
- **0 asset changes**

---

## Artifacts produced

| Step | Description | Path |
|------|-------------|------|
| 1 | Research | `project/research/20260508-093000-feature-15-turn-based-combat-core.md` |
| 2 | Plan | `project/plans/20260508-100000-feature-15-turn-based-combat-core.md` (Status: Approved 2026-05-08) |
| 3 | Implementation summary | `project/implemented/20260508-120000-feature-15-turn-based-combat-core.md` |
| 4 | Code review | `project/reviews/20260508-140000-feature-15-turn-based-combat-core.md` (Verdict: LGTM-with-changes) |
| - | This summary | `project/orchestrator/<this file>` |
| - | Pipeline state | `project/orchestrator/PIPELINE-STATE.md` |

---

## User decisions

Six USER PICK / Architecture decisions were surfaced at plan-approval time. User accepted all six recommended defaults (Option A across the board):

| Decision | Topic | Pick |
|---|---|---|
| **D-A3** | Damage formula | **A** — Wizardry multiplicative `(A * (100 - D/2)) / 100`, variance 0.7-1.0, crit 1.5x |
| **D-Q1** | Combat camera | **A** — Overlay on dungeon camera; reuse existing `attach_egui_to_dungeon_camera`. No new `Camera3d` |
| **D-Q2** | Action menu UX | **A** — Persistent action panel at bottom (~60px permanent) |
| **D-Q3** | Combat log shape | **A** — Bounded `VecDeque<CombatLogEntry>` capacity 50, kept across combats (~4 KB) |
| **D-Q4** | Defend × magical `DefenseUp` stacking | **A** — Take-higher (current #14 rule). Defend's 0.5 silently no-ops when ≥1.0 magical buff active. No new variant |
| **D-Q5** | Boss AI scope | **A** — `EnemyAi` enum stub with `RandomAttack` + two scripted boss patterns (`BossFocusWeakest`, `BossAttackDefendAttack`). ~80 LOC + 4 tests |

Additional decisions implied by the task brief:
- Plan structured as four ordered sub-PR phases (15A turn manager → 15B damage → 15C AI → 15D UI) inside one plan file.
- `CastSpell` stubbed; full spell impl deferred to #20.
- `CurrentEncounter` populated via test fixtures; encounter triggers in #16.
- `Defend` writes `DefenseUp` via the existing `ApplyStatusEvent` pipeline (no parallel mechanism).

---

## Verification gate

Executed against the implemented codebase 2026-05-08 by user (orchestrator and implementer subagents lacked Bash in this session):

| Check | Result |
|---|---|
| `cargo check` | PASS — 3.57s |
| `cargo check --features dev` | PASS — 1.73s |
| `cargo test` | PASS — 187 passed, 0 failed (default features) |
| `cargo test --features dev` | PASS — 190 passed, 0 failed |
| `cargo clippy --all-targets -- -D warnings` | PASS (zero warnings) |
| `cargo clippy --all-targets --features dev -- -D warnings` | PASS |
| `cargo fmt --check` | PASS (after `cargo fmt` was run once to auto-fix manual line-wrapping artifacts; final `--check` exit 0; no logic changes) |

The plan's verification gate ("evidence before assertions") passed in full. The implementation required two fix passes (D-I12 through D-I18) to land green; details captured in the implementation summary.

---

## Review findings

Reviewer verdict: **LGTM-with-changes**. Severity counts: 0 CRITICAL, 0 HIGH, **2 MEDIUM**, 2 LOW.

| Severity | Finding | File / Anchor | Status |
|---|---|---|---|
| **MEDIUM-1** | Four plan-mandated tests absent: `defend_no_ops_when_higher_defense_up_active` (Decision 5 / Pitfall 6), `use_item_rejects_key_items` (Decision 31), `silence_blocks_spell_menu` (Decision 34), `enemy_buff_re_derives_stats` (Pitfall 11). All four production code paths exist and are correct; only the tests were omitted. | `turn_manager.rs`, `ui_combat.rs` | OPEN — straightforward to add; reviewer included a code sketch for the hardest one |
| **MEDIUM-2** | `BossAttackDefendAttack { turn }` counter never increments because `EnemyAi` is queried immutably. Boss will emit the same action every round rather than cycling Attack/Defend/Attack. No boss enemies are spawned in v1 so doesn't manifest yet, but will silently corrupt #17 boss authoring. | `ai.rs:130-143` | OPEN — fix is `&mut EnemyAi` query and `*turn = turn.saturating_add(1)` after each emission |
| LOW-1 | Two AI tests (`random_attack_picks_alive_party_member`, `random_attack_skips_dead_enemies`) end with vacuous `let _ = ai_actions` — pass-if-no-panic only. Production logic is correct; the tests provide no coverage of the queue contents emitted. | `ai.rs:238-291` | NOTED — does not cause false negatives |
| LOW-2 | `check_victory_defeat_flee` uses `party.iter().all(...)` which is vacuously true on an empty query — would trigger `GameOver` immediately if combat ran with zero `PartyMember` entities. Latent test trap; not reachable in production via #16's encounter spawner. | `turn_manager.rs:587-589` | NOTED — guard with `.any()` + count check would harden |

**Pipeline rule (per task brief):** "Blocking issues require another implementer pass; non-blocking (LOW/NIT) get noted but don't block the pipeline summary." MEDIUM sits between those — surfacing for explicit user direction below.

All non-negotiable constraints from the plan PASSED:
- Sole-mutator invariant on `StatusEffects.effects` (zero `effects.push` / `effects.retain` outside `apply_status_handler`)
- `damage_calc` is a pure function (no `Query`, no `Res<Assets>`, no entity lookups)
- AI emits to queue ONLY (zero `current_hp.*=`, `MessageWriter<ApplyStatusEvent>`, `&mut StatusEffects` in `ai.rs`)
- `execute_combat_actions.before(apply_status_handler)` registered
- `check_dead_and_apply` called after every HP write
- B0002 query split via `CombatantSnapshot` pre-collection
- `std::mem::take` on queue at start of `execute_combat_actions` (Pitfall 2)
- `CombatActionKind` (not `CombatAction`) — zero leafwing-name collisions
- No `Camera3d` spawn in `CombatUiPlugin` — overlay approach
- `rand 0.9` migration (`os_rng` feature, `?Sized` on `dyn Rng` params, `random_range` rename) handled correctly
- D-A5 carve-out (drop `With<PartyMember>` from `recompute_derived_stats_on_equipment_change`) verified

---

## Implementation deviations worth keeping (D-I1 through D-I18)

These are insights from the run that aren't obvious from the code alone:

1. **D-I1 / D-I2 — B0002 query conflict on `DerivedStats`:** the initial `execute_combat_actions` design read and wrote `DerivedStats` in the same system. Resolved via `CombatantSnapshot` pre-collection + a `derived_mut: Query<&mut DerivedStats>` sole accessor. Pattern likely repeats in any multi-target combat resolver.
2. **D-I3 — `ActionState<CombatAction>` test harness:** `handle_combat_input` requires `Res<ActionState<CombatAction>>`. All three combat test harnesses (`turn_manager.rs`, `ai.rs`, `ui_combat.rs`) use `init_resource::<ActionState<CombatAction>>()` directly without `ActionsPlugin` (avoids leafwing's mouse-resource panic in headless tests).
3. **D-I4 — `apply_status_handler` `With<PartyMember>` filter:** enemies do not receive `Dead` status from `check_dead_and_apply` because of #14's `With<PartyMember>` filter. Enemy death is correctly detected via `current_hp == 0` in `check_victory_defeat_flee`. Acceptable for v1; #17 can add enemy status persistence.
4. **D-I12 — `rand 0.9` `from_os_rng` requires `os_rng` feature:** in rand 0.9, `SeedableRng::from_os_rng()` is feature-gated. `Cargo.toml` must include `"os_rng"` in the feature list.
5. **D-I13 — `?Sized` bound on `&mut impl Rng` parameters:** `impl Trait` implies `Sized` by default. Passing `&mut *rng.0` (a `dyn RngCore + Send + Sync` DST) requires `&mut (impl Rng + ?Sized)` to coerce.
6. **D-I14 — `gen_range` renamed to `random_range` in rand 0.9:** four call sites needed update across `damage.rs` and `turn_manager.rs`.
7. **D-I16 — Test harnesses need `app.add_message::<MovedEvent>()`:** the `tick_on_dungeon_step` system from `StatusEffectsPlugin` reads `MessageReader<MovedEvent>`; `MovedEvent` is registered by `DungeonPlugin` in production but must be explicitly added to combat test apps.
8. **D-I18 — Manual rustfmt diff:** the implementer's manual line-wrapping had trailing-whitespace artifacts that rustfmt normalized differently. `cargo fmt` (no `--check`) auto-fixed; downstream `cargo fmt --check` is exit 0.

The full deviation log lives in `project/implemented/20260508-120000-feature-15-turn-based-combat-core.md` §`## Deviations from Plan`.

---

## What this enables

- **#16 (Encounter System)** can populate `CurrentEncounter` from production triggers; the action-queue spec is locked.
- **#17 (Combat Polish + Boss Enemies)** can author bosses by spawning `EnemyAi::BossFocusWeakest` or `EnemyAi::BossAttackDefendAttack { turn: 0 }` (after MEDIUM-2 is fixed). Animation hooks slot into `execute_combat_actions` resolver branches.
- **#20 (Spell System)** plugs into `CombatActionKind::CastSpell` — the queue carries the spell ID; the resolver dispatches to spell handlers and emits `ApplyStatusEvent` for status spells.
- **#23 (Save / Load)** gets combat resources for free — `TurnActionQueue`/`PlayerInputState`/`CombatRng`/`CombatLog` are already structured to be skipped or serialized; #15 doesn't fight save formatting.

---

## Pipeline scope conclusion

Pipeline ran research → plan → implement → review and produced all five artifacts. Verification gate is GREEN; reviewer verdict is **LGTM-with-changes** with 2 MEDIUM and 2 LOW findings.

**Ship is awaiting user authorization.** Two paths forward:

1. **Address MEDIUM findings first** — re-spawn implementer to add the four missing tests and fix `BossAttackDefendAttack` mut-query, re-run verification gate, then ship. Estimated ~150-200 LOC of test code + a 5-line query change.
2. **Ship as-is** — accept MEDIUMs as documented follow-ups for an immediate cleanup PR; surface the boss-AI bug as a known-issue note before #17 begins.

Per CLAUDE.md GitButler discipline, shipping uses `but` (not `git`) for all history mutations.

---

## Post-review fix pass (2026-05-08)

User chose path 1 — address both MEDIUM findings before ship. Implementer dispatched; both fixes landed and verification gate re-ran GREEN.

### Resolutions

| Finding | Resolution | Discovery ID |
|---|---|---|
| **MEDIUM-1** — Four plan-mandated tests absent (`defend_no_ops_when_higher_defense_up_active`, `use_item_rejects_key_items`, `silence_blocks_spell_menu`, `enemy_buff_re_derives_stats`) | Four tests added across `turn_manager.rs::app_tests` and `ai.rs::app_tests` and `ui_combat.rs::app_tests`. Sole-mutator invariants preserved (all use `vec![...]` for `StatusEffects` initialization; no `.push()` / `.retain()`). `silence_blocks_spell_menu` asserts on menu-state pop only — the over-specified CombatLog "silenced" assertion was removed because production does not emit that log; a UX log entry is deferred to a separate plan. | **D-I20** |
| **MEDIUM-2** — `BossAttackDefendAttack { turn }` counter never increments | `EnemyAiQuery` type alias changed from `&'static EnemyAi` to `&'static mut EnemyAi`. `match ai` switched to `match &mut *ai` so the `turn` arm receives `&mut u32`; `*turn = turn.saturating_add(1)` runs after action emission. Previous static pattern-logic test replaced with a full app test that runs 3 `ExecuteActions` cycles and asserts the boss emits Defend on round 2 and `turn == 3` after round 3. | **D-I19** |

### Implementation discoveries from the fix pass

- **D-I19** — Switching `EnemyAi` from immutable to mutable in the query required `mut` binding on the `for` destructure and `match &mut *ai` to satisfy the borrow checker. Grep guards (`current_hp.*=`, `&mut StatusEffects`, `MessageWriter<ApplyStatusEvent>` in `ai.rs`) all remained at zero matches — the mutability is on `EnemyAi`, not on combat-actor state, so AI's "queue-only emitter" discipline is preserved.
- **D-I20** — Two of the four tests required injecting state via direct `Messages<T>` resource writes rather than `MessageWriter` (`enemy_buff_re_derives_stats` writes an `EquipmentChangedEvent` directly). This matches the pattern from #14's `status_effects.rs` app tests and avoids needing a system to write the message inside the test.
- **D-I21** *(implicit)* — `silence_blocks_spell_menu` revealed that production code does not emit a CombatLog entry when the silenced player tries to open the spell menu. The UI just pops the menu stack back to `Main`. The test was scoped to assertion of the menu-state pop only. A "you are silenced" log message is a UX improvement that should be planned separately.
- **D-I22** *(implicit)* — `use_item_rejects_key_items` does not need an inventory at all — the rejection happens before inventory access. The test inserts a `KeyItem` `ItemAsset` directly into `Assets<ItemAsset>` and asserts the log contains `"cannot use"`.

### Re-verification gate (2026-05-08, post-fix)

| # | Command | Result |
|---|---------|--------|
| 1 | `cargo check` | PASS — 1.19s |
| 2 | `cargo check --features dev` | PASS — 1.28s |
| 3 | `cargo test` | PASS — **191 passed**, 0 failed (was 187; +4 net new tests) |
| 4 | `cargo test --features dev` | PASS — **194 passed**, 0 failed (was 190; +4 net new tests) |
| 5 | `cargo clippy --all-targets -- -D warnings` | PASS (zero warnings) |
| 6 | `cargo clippy --all-targets --features dev -- -D warnings` | PASS |
| 7 | `cargo fmt --check` | PASS (exit 0) |

**Both MEDIUMs resolved. LOW-1 / LOW-2 carried as deferred follow-ups (acceptable per reviewer).**

### Ship

User authorized shipping. Branch `feature/15-turn-based-combat-core` pushed to `origin` at SHA `dcf3d51` (2 commits: `25cfad4 add core of turn based combat` + `dcf3d51 add memory and other stuff`).

**PR:** https://github.com/codeinaire/druum-dungeon-crawler/pull/15
**Title:** `feat(combat): turn-based combat core (action-queue loop)`
**Body:** `project/shipper/feature-15-pr-body.md` (TL;DR, mermaid flow diagram, reviewer guide, scope, risk, future deps, test plan checkboxes).
**Commit message:** `project/shipper/feature-15-commit-msg.txt`.
