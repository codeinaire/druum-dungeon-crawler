---
name: Druum turn-based combat core (Feature #15) decisions
description: Feature #15 architecture — 4 sub-phase split (15A turn-manager / 15B damage / 15C AI / 15D UI), pure damage_calc, AI emit-only boundary, CombatActionKind rename to dodge leafwing collision, D-A5 With<PartyMember> drop, rand+rand_chacha Step A/B/C
type: project
---

# Feature #15 Turn-Based Combat Core — planning decisions (frozen at plan time, 2026-05-08)

The full plan: `project/plans/20260508-100000-feature-15-turn-based-combat-core.md`.

## Sub-PR split (locked — matches roadmap line 824)

Four sequential sub-phases inside ONE plan file, each an atomic GitButler commit boundary.
- **15A — Turn Manager + State Machine** (~350 LOC, ~18 tests): `turn_manager.rs`, `actions.rs`, `enemy.rs`, `combat_log.rs` + `targeting.rs`/`ai.rs` STUBS for compile-time module discovery (just enums, no bodies). Sub-plugin: `TurnManagerPlugin`. Tests: queue ordering, Defend integration, victory/defeat/flee, Sleep/Paralysis/Silence gates, check_dead invocation, UseItem stub, CastSpell stub, enemy buff re-derive.
- **15B — Damage + Targeting** (~250 LOC, ~10 tests): `damage.rs` full body + `targeting.rs` full body (extends 15A stub). NO new sub-plugin; pure `pub fn`s. Tests: damage formula edge cases, targeting re-resolution.
- **15C — AI** (~200 LOC, ~4 tests): `ai.rs` full body (extends 15A stub) + `EnemyAi` enum 3 variants. Sub-plugin: `EnemyAiPlugin`. Tests: determinism, dead-skip, no-alive-party, BossAttackDefendAttack cycle.
- **15D — UI** (~400-600 LOC, ~3-5 tests + manual smoke): `ui_combat.rs`. Sub-plugin: `CombatUiPlugin`. Tests: paint smoke, input handler, target cursor.

## Architecture decisions (locked)

- **D-A1 — Sub-plugin shape:** 3 sub-plugins (`TurnManagerPlugin`, `EnemyAiPlugin`, `CombatUiPlugin`) registered from `combat/mod.rs::CombatPlugin::build`. 15B ships as `pub fn`s consumed by `TurnManagerPlugin`, NOT as a plugin. Mirrors `StatusEffectsPlugin` precedent.
- **D-A2 — Skip `CombatPhase::EnemyTurn`:** use 3-of-4 phases verbatim (PlayerInput → ExecuteActions → TurnResult → loop). The `EnemyTurn` variant in `state/mod.rs:34` is frozen by #2 and stays in the enum but is NEVER set. Document the vestige in `turn_manager.rs`, NOT in `state/mod.rs`.
- **D-A3 — Damage formula (USER-PICK = A):** Wizardry-style multiplicative `(A * (100 - D / 2)) / 100`. Variance multiplier 0.7..=1.0. Crit chance `accuracy / 5`% capped at 100; crit applies 1.5x. Floor of 1 on positive-attack hits.
- **D-A4 — `CombatActionKind` enum payload:** RENAMED from "CombatAction" to dodge collision with `crate::plugins::input::CombatAction` (the leafwing menu-nav enum, frozen by #5). Variants: `Attack`, `Defend`, `CastSpell { spell_id: String }` (stub), `UseItem { item: Handle<ItemAsset> }`, `Flee`. Wrapped in `QueuedAction { actor, kind, target, speed_at_queue_time, actor_side, slot_index }`. `Side` enum lives in `actions.rs`.
- **D-A5 — Drop `With<PartyMember>` filter from `recompute_derived_stats_on_equipment_change`:** carve-out edit at `inventory.rs:445`. Enemies spawn with `Equipment::default()` and `Experience::default()` to satisfy the now-broader query (~16 bytes per enemy). Doc-comment update explains dual-use. Single re-derive code path for all combatants.
- **D-Q1 — Combat camera (USER-PICK = A):** Overlay on dungeon camera. `CombatUiPlugin::attach_egui_to_dungeon_camera` mirrors `MinimapPlugin::attach_egui_to_dungeon_camera`. NO new `Camera3d`. Wizardry/Etrian convention.
- **D-Q2 — Action menu UX (USER-PICK = A):** Persistent `egui::TopBottomPanel::bottom("action_menu").min_height(60.0)` always visible during PlayerInput. Buttons: Attack/Defend/Spell/Item/Flee.
- **D-Q3 — Combat log (USER-PICK = A):** Bounded `VecDeque<CombatLogEntry>` capacity 50, kept across combats (NOT cleared on OnExit). ~4 KB memory. `clear()` method exists but is unused in v1.
- **D-Q4 — Defend stacking (USER-PICK = A):** Take-higher (existing #14 stacking rule). Defend writes `ApplyStatusEvent { effect: DefenseUp, potency: 0.5, duration: Some(1) }`. The merge silently no-ops when a stronger magical buff is active. Pitfall 6 mitigation: log entry "X defends!" fires UNCONDITIONALLY for game-feel.
- **D-Q5 — Boss AI scope (USER-PICK = A):** `EnemyAi` enum 3 variants — `RandomAttack` (default), `BossFocusWeakest`, `BossAttackDefendAttack { turn: u32 }`. ~80 LOC + 4 tests. Each is a match arm in `enemy_ai_action_select`.

## Critical seam discipline (research §Anti-Patterns + roadmap line 858)

- **`damage_calc` is a pure function.** Signature `(attacker: &Combatant, defender: &Combatant, weapon: Option<&ItemAsset>, action: &CombatActionKind, rng: &mut impl Rng) -> DamageResult`. NO entity lookups, NO `Query`, NO `Res<...>`, NO `Time`, NO `Commands`. `Combatant` struct is the caller-flatten step (mirrors `derive_stats`'s caller-flatten-Equipment pattern). Verification gate: `rg 'Query<' damage.rs` and `rg 'Res<' damage.rs` must be ZERO matches.
- **AI emits actions ONLY.** `enemy_ai_action_select` may NOT touch `current_hp`, MAY NOT write `ApplyStatusEvent`, MAY NOT mutate `StatusEffects`. Single side effect: `queue.queue.push(...)`. Verification: `rg 'current_hp.*=' ai.rs` and `rg 'MessageWriter<ApplyStatusEvent>' ai.rs` and `rg '&mut StatusEffects' ai.rs` must be ZERO.
- **`apply_status_handler` is the SOLE mutator of `StatusEffects.effects`** (frozen from #14 Decision 20). #15's resolver writes `ApplyStatusEvent`, never `effects.push(...)`. Verification: `rg 'effects\.push\(|effects\.retain' src/plugins/combat/{actions,ai,combat_log,damage,enemy,targeting,turn_manager,ui_combat}.rs` must be ZERO.
- **Defend → DefenseUp via existing #14 pipeline.** NO `Defending: Component`, NO new enum variant, NO specialized handler. The take-higher merge rule + existing `recompute_derived_stats_on_equipment_change` (D-A5 carve-out makes this work for enemies too) does all the work.
- **After every damage write, call `check_dead_and_apply`.** Decision: `current_hp = current_hp.saturating_sub(damage)` then `check_dead_and_apply(target, &derived, &mut apply_status)`. NEVER manually push `Dead` into `effects`.
- **Cross-plugin ordering:** `execute_combat_actions.before(crate::plugins::combat::status_effects::apply_status_handler)` so Defend/Dead writes are visible same-frame. Mirrors `apply_poison_trap.before(apply_status_handler)` precedent at `dungeon/features.rs:171` (#14 pattern).
- **UI is the SOLE writer of `TurnActionQueue` from the player side.** `handle_combat_input` in `ui_combat.rs` is the only path; `execute_combat_actions` only DRAINS the queue (Anti-pattern 5).

## `rand` direct dep gate (Step A/B/C per `feedback_third_party_crate_step_a_b_c_pattern.md`)

`rand 0.9.4` and `rand_chacha 0.9.0` both already transitive (`Cargo.lock:4358-4376` verified at research time). Phase 15A Step 1 runs the Step A/B/C gate before adding to `Cargo.toml`.
- **Production:** `rand = { version = "0.9", default-features = false, features = ["std", "std_rng", "small_rng"] }`. `SmallRng::from_os_rng()` in `init_combat_state`.
- **Tests:** `rand_chacha = { version = "0.9", default-features = false, features = ["std"] }` as a `dev-dependency`. `ChaCha8Rng::seed_from_u64(42)` for byte-stable determinism.
- **Single RNG resource:** `CombatRng(pub Box<dyn rand::RngCore + Send + Sync>)`. Tests overwrite the resource directly with a seeded ChaCha8Rng. Pitfall 12 — all `#15` RNG users read from this single source.

## CurrentEncounter contract (Pitfall 1)

`CurrentEncounter: Resource` is owned by #16. #15 references the contract via doc-comment in `turn_manager.rs`; defines a test-fixture-only resource in `app_tests`. Production reads via `Option<Res<CurrentEncounter>>` so #15 ships without it. Dev-stub spawn (`#[cfg(feature = "dev")] spawn_dev_encounter`) bypasses `CurrentEncounter` and spawns 2 hardcoded Goblins; #16 deletes the stub.

```rust
// Documented contract — full type lives in #16:
#[derive(Resource, Debug, Clone)]
pub struct CurrentEncounter {
    pub enemy_entities: Vec<Entity>,
    pub fleeable: bool,
}
```

## Speed tie-break (Pitfall 9 — locked)

```rust
queue.queue.sort_by(|a, b| {
    b.speed_at_queue_time.cmp(&a.speed_at_queue_time)  // descending
        .then(a.actor_side.cmp(&b.actor_side))         // Party (=0) before Enemy (=1)
        .then(a.slot_index.cmp(&b.slot_index))         // ascending slot
});
```

`Vec::sort_by` (stable). `Side: PartialOrd + Ord` derive needed for the second `.cmp()`.

## Mid-turn death rules (Pitfalls 3 + 10)

- **Actor dies before its action resolves:** `is_alive(action.actor)` check at the top of each loop iter in `execute_combat_actions`. Skip with log "{name} is unable to act."
- **Target dies before action resolves:** `resolve_target_with_fallback` re-targets to a random alive entity on the same side as the original target. Side determined by membership in `&[Entity]` party/enemy slices. Side wiped → empty Vec → action no-ops with log "{name}'s attack has no target."
- **AI never emits actions for dead enemies:** filter on `!status.has(Dead) && !status.has(Stone) && current_hp > 0` in `enemy_ai_action_select`.

## `CombatPhase::TurnResult` ordering (Decision 24, Open Question 4 of research)

In `check_victory_defeat_flee`, the order is:
1. **Defeat first** — all party Dead → `GameState::GameOver`. (Players don't get a phyrric victory loss.)
2. **Flee** — `FleeAttempted.success == true` → `GameState::Dungeon`.
3. **Victory** — all enemies Dead → `GameState::Dungeon`.
4. **Next round** — `CombatPhase::PlayerInput`. Increments `current_turn`, clears `committed`.

## Test plan (env. +20-30; we land at 25-30)

- **15A:** ~18 tests (queue 3, Defend 3, victory/defeat/flee 4, Sleep/Paralysis/Silence 3, check_dead 1, UseItem 2, CastSpell 1, enemy re-derive 1).
- **15B:** ~10 tests (damage 7, targeting 3).
- **15C:** ~4 tests (AI determinism, dead-skip, no-party, boss-cycle).
- **15D:** ~3-5 tests + manual smoke.
- **Total: ~35-37 tests** (slightly above upper envelope; appropriate for the size of the feature). Trim 5-7 lower-value if budget tightens.

## What #15 does NOT ship (deferred)

- **Spell mechanics → #20.** `CombatActionKind::CastSpell { spell_id: String }` is a stub variant; resolver writes "Spell stub: not yet implemented" log entry.
- **Encounter spawning → #16.** `CurrentEncounter` is contract-only; dev-stub spawner is throwaway.
- **Animation tweens → #17.** Damage resolves instantly; no per-action animation timing.
- **Enemy authoring → #17.** `EnemyDb` stub stays empty; v1 hardcodes 2 Goblins.
- **Real per-instance enemy state → #17.** Enemies spawn with hardcoded stats.
- **Item heal-amount field → #20 polish.** v1 hardcodes consumable heal = `max_hp / 4`.
- **Real menu navigation Up/Down cursor → #25 polish.** Phase 15D ships simplest-version "Confirm = Attack first enemy"; richer cursor lives in `selected_button: usize` field.
- **Combat hit/death SFX → #17 polish.**
- **Save mid-round → #23.** `TurnActionQueue` is NOT serialized; combat is atomic.

## Carve-out edits to frozen files (4 explicit carve-outs)

- `combat/mod.rs` — +1 module declaration + +1 `add_plugins` per phase (15A/15C/15D each add one).
- `inventory.rs:445` — drop `With<PartyMember>` filter (D-A5). Doc-comment update explains dual-use.
- NO edits to test harnesses (`dungeon/tests.rs`, `dungeon/features.rs::tests`, `tests/dungeon_geometry.rs`, `tests/dungeon_movement.rs`) — they already register `CombatPlugin` per #14 D-I10/D-I11; #15's new resources are owned by sub-plugins INSIDE `CombatPlugin::build` so harnesses pick them up automatically.
- NO edits to `state/mod.rs` (the `EnemyTurn` vestige is documented in `turn_manager.rs`, NOT here — frozen by #2).
- NO edits to `input/mod.rs` (leafwing `CombatAction` enum stays at 6 variants — frozen by #5).
- NO edits to `combat/status_effects.rs` (frozen since #14; `check_dead_and_apply` at lines 394-407 is the contract).
