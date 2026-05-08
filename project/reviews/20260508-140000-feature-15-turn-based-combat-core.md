# Code Review — Feature #15: Turn-Based Combat Core

**Date:** 2026-05-08  
**Verdict:** LGTM-with-changes (2 MEDIUM findings; no CRITICAL or HIGH)  
**Reviewed:** local working-tree diff (pre-ship gate, no PR yet)

---

## Behavioral delta

This PR introduces the complete turn-based combat resolution layer:

- `TurnManagerPlugin` owns `TurnActionQueue`, `PlayerInputState`, `CombatRng`, `FleeAttempted`, `CombatLog`. Systems: `init_combat_state`/`clear_combat_state` (OnEnter/OnExit), `collect_player_actions` (PlayerInput phase), `sort_queue_by_speed` + `execute_combat_actions` (ExecuteActions phase), `check_victory_defeat_flee` (TurnResult phase).
- `EnemyAiPlugin` owns `enemy_ai_action_select`, runs before `sort_queue_by_speed` in `ExecuteActions`.
- `CombatUiPlugin` owns `attach_egui_to_dungeon_camera`, `paint_combat_screen` (EguiPrimaryContextPass), `handle_combat_input` (Update). SOLE player-side writer of `TurnActionQueue`.
- `damage_calc` is a pure function in `damage.rs`; `resolve_target_with_fallback` is a pure function in `targeting.rs`.
- `party/inventory.rs` carve-out: `With<PartyMember>` filter dropped from `recompute_derived_stats_on_equipment_change` (D-A5).
- `Cargo.toml`: `rand 0.9` direct dep with `os_rng` feature; `rand_chacha 0.9` dev-dep.

---

## Critical constraint compliance

| Constraint | Status |
|---|---|
| `#[derive(Message)]` only — zero `derive(Event)` / `EventReader` / `EventWriter` | PASS |
| `damage_calc` is pure — no `Query`, no `Res<Assets>`, no entity lookups | PASS |
| `CombatActionKind` (not `CombatAction`) — zero leafwing-name collisions | PASS |
| Saturating arithmetic in `damage_calc` (`saturating_sub`, `.min(100)`, `.min(180)`) | PASS |
| AI emits to queue ONLY — zero `current_hp.*=`, `MessageWriter<ApplyStatusEvent>`, `&mut StatusEffects` in ai.rs | PASS |
| `effects.push` / `effects.retain` sole-mutator discipline — zero matches in combat/*.rs | PASS |
| `execute_combat_actions.before(apply_status_handler)` registered | PASS |
| `check_dead_and_apply` called after every HP write in Attack arm | PASS |
| `std::mem::take` on queue at start of `execute_combat_actions` (Pitfall 2) | PASS |
| Speed sort uses stable `Vec::sort_by` (Pitfall 9) | PASS |
| `CombatPhase::EnemyTurn` never set; vestigial-note doc-comment in `turn_manager.rs` | PASS |
| `CombatActionKind` enum — no new `CombatAction` leafwing variants added | PASS |
| Defend writes `ApplyStatusEvent { effect: DefenseUp, potency: 0.5, duration: Some(1) }` — no `Defending` component | PASS |
| `EnemyBundle` includes `Equipment::default()` + `Experience::default()` for D-A5 query | PASS |
| `rand 0.9` with `os_rng` feature; `SmallRng::from_os_rng()` in production; `ChaCha8Rng::seed_from_u64` in tests | PASS |
| `sort_queue_by_speed` exported `pub fn` — importable by `EnemyAiPlugin` | PASS |
| `enemy_ai_action_select.before(sort_queue_by_speed).before(execute_combat_actions)` chain | PASS |
| No `Camera3d` spawn in `CombatUiPlugin` — overlay approach | PASS |
| No cross-schedule `.before()`/`.after()` constraints (EguiPrimaryContextPass painters have no ordering calls) | PASS |
| D-I4: `apply_status_handler` `With<PartyMember>` acknowledged; enemy death detected via `current_hp == 0` check in `check_victory_defeat_flee` | PASS (documented) |
| Sleep/Paralysis auto-skip in `collect_player_actions` — sentinel to `committed` only, not to `queue` (Decision 33) | PASS |
| `Cargo.toml` frozen files respected — `main.rs`, `state/mod.rs`, `input/mod.rs` unchanged | PASS |

---

## Findings

---

### [MEDIUM] Four plan-mandated tests are absent

**Files:** `src/plugins/combat/turn_manager.rs`, `src/plugins/combat/ui_combat.rs`

**Issue:** The plan explicitly mandated these specific tests in Phase 15A (Decisions 5, 31, 34, Pitfall 11). None are present in any combat module:

| Test name | Mandated in | What it covers |
|---|---|---|
| `defend_no_ops_when_higher_defense_up_active` | Decision 5 / Pitfall 6 | Defend's Defend→DefenseUp silent no-op when `DefenseUp ≥ 0.5` already active; log fires unconditionally |
| `use_item_rejects_key_items` | Decision 31 | `KeyItem` in `UseItem` arm exits early with "cannot use in combat" log |
| `silence_blocks_spell_menu` | Decision 34 | `SpellMenu` frame with silenced actor → pops to Main with log |
| `enemy_buff_re_derives_stats` | Pitfall 11 | Enemy with `Equipment::default()` receives `EquipmentChangedEvent` and `recompute_derived_stats_on_equipment_change` re-derives correctly |

The production code for all four exists (Defend arm at `turn_manager.rs:486`, KeyItem rejection at `:516`, silence gate in `ui_combat.rs:298`, D-A5 carve-out in `inventory.rs:444`). This is a test coverage gap, not a logic absence.

**Fix:** Add the four tests. `defend_no_ops_when_higher_defense_up_active` requires an app test that pre-inserts `DefenseUp 1.0` on the actor then queues a Defend, runs two `app.update()` calls (for `execute_combat_actions` then `apply_status_handler`), and asserts the `StatusEffects` still has only one `DefenseUp` at `1.0` magnitude. The other three are simpler pure or layer-2 tests.

Sketch of the hardest one (`defend_no_ops_when_higher_defense_up_active`):

```rust
#[test]
fn defend_no_ops_when_higher_defense_up_active() {
    let mut app = make_test_app();
    enter_combat(&mut app);
    seed_test_rng(&mut app, 0);

    // Spawn party member with pre-existing DefenseUp 1.0.
    let actor = app.world_mut().spawn(
        crate::plugins::party::PartyMemberBundle {
            derived_stats: DerivedStats { current_hp: 100, max_hp: 100, speed: 10, ..Default::default() },
            party_slot: PartySlot(0),
            status_effects: StatusEffects {
                effects: vec![ActiveEffect {
                    effect_type: StatusEffectType::DefenseUp,
                    remaining_turns: Some(3),
                    magnitude: 1.0,
                }],
            },
            ..Default::default()
        }
    ).id();
    let _enemy = spawn_enemy(&mut app, 50, 0, 5);

    write_queued_action(&mut app, QueuedAction {
        actor,
        kind: CombatActionKind::Defend,
        target: TargetSelection::Self_,
        speed_at_queue_time: 10,
        actor_side: Side::Party,
        slot_index: 0,
    });

    enter_execute_phase(&mut app);
    app.update(); // allow apply_status_handler to run

    // Log fires unconditionally (Pitfall 6).
    let log = app.world().resource::<CombatLog>();
    assert!(log.entries.iter().any(|e| e.message.contains("defends!")));

    // Existing DefenseUp 1.0 unchanged (take-higher: 0.5 loses).
    let status = app.world().get::<StatusEffects>(actor).unwrap();
    let defense_up = status.effects.iter().find(|e| e.effect_type == StatusEffectType::DefenseUp);
    assert!(defense_up.is_some());
    assert!((defense_up.unwrap().magnitude - 1.0).abs() < 0.001, "DefenseUp magnitude must stay 1.0");
}
```

---

### [MEDIUM] `BossAttackDefendAttack { turn }` counter never increments — boss AI pattern permanently stuck

**File:** `src/plugins/combat/ai.rs:130-143`

**Issue:** The `BossAttackDefendAttack { turn: u32 }` variant's `turn` field is read in `enemy_ai_action_select` to compute `turn % 3`, but it is never mutated. The component is accessed via `&'static EnemyAi` (immutable reference). The boss will emit the same action on every round indefinitely, never cycling the pattern.

The plan (Decision 37 / D-Q5=A) says the pattern "cycles Attack/Defend/Attack based on `turn % 3`" — this implies round-based advancement. A component with `turn = 0` stays at Attack forever; `turn = 1` stays at Defend forever.

No production boss enemies are spawned in v1 (the dev-stub only spawns `EnemyAi::RandomAttack` goblins), so this won't manifest before #17. But if the invariant isn't caught here it will silently persist into #17 boss authoring.

**Fix:** Change the EnemyAi query to mutable access for the `BossAttackDefendAttack` branch, or store the per-round turn counter in a separate `EnemyTurnCounter(pub u32): Component` and increment it in `check_victory_defeat_flee` after each round. The simplest fix is a mut query:

```rust
// Change the query to mut for the Boss variant:
type EnemyAiQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static mut EnemyAi,   // mut to allow turn increment
        &'static EnemyIndex,
        &'static DerivedStats,
        &'static StatusEffects,
    ),
    (With<Enemy>, Without<PartyMember>),
>;

// In the BossAttackDefendAttack match arm, after emitting the action:
EnemyAi::BossAttackDefendAttack { turn } => {
    let action_result = match *turn % 3 { ... };
    *turn = turn.saturating_add(1);   // advance the pattern
    action_result
}
```

Alternatively, increment in `check_victory_defeat_flee` by iterating all `Enemy` entities with `&mut EnemyAi` and advancing only the `BossAttackDefendAttack` variant.

---

## LOW / NIT findings

---

### [LOW] Two AI tests are no-op assertions (pass-if-no-panic only)

**File:** `src/plugins/combat/ai.rs:238-291`

**Issue:** `random_attack_picks_alive_party_member` and `random_attack_skips_dead_enemies` both end with comments acknowledging the assertions are vacuous ("No panic = pass") because `execute_combat_actions` drains the queue in the same frame. The `ai_actions` Vec is collected and then immediately dropped with `let _ = ai_actions`. The tests provide no coverage of the queue contents the AI emitted.

**Suggested fix:** Instrument the test by intercepting at `sort_queue_by_speed` (before execute drains it) or by checking the combat log for action-related entries after execution. Example for `random_attack_picks_alive_party_member`:

```rust
// After app.update() x2, check combat_log for an attack entry.
let log = app.world().resource::<CombatLog>();
let has_attack_log = log.entries.iter().any(|e|
    e.message.contains("attacks") || e.message.contains("misses")
);
assert!(has_attack_log, "AI should have emitted an attack action");
```

This is LOW: no-op tests don't cause false negatives, and `random_attack_skips_dead_enemies` already has the correct guard in production code (`derived.current_hp < 1`).

---

### [LOW] `check_victory_defeat_flee`: vacuous defeat on empty party query

**File:** `src/plugins/combat/turn_manager.rs:587-589`

**Issue:** `party.iter().all(...)` returns `true` when the party query yields zero rows (vacuous truth). If somehow `check_victory_defeat_flee` runs before any `PartyMember` entities are spawned (e.g., a test that omits `spawn_party_member`), it triggers `GameOver` immediately.

In production this is harmless — `CombatPlugin` only registers `init_combat_state` which doesn't guard against no-party. The dev-stub and #16 production encounter spawner both pair with an existing party. But it's a latent trap in tests.

**Suggested fix:** Guard with a party-count check or use `any` instead of negating `all`:

```rust
let party_alive = party.iter().any(|(d, s)| {
    d.current_hp > 0 && !s.has(StatusEffectType::Dead)
});
let all_party_dead = !party_alive && party.iter().count() > 0;
```

---

## Review Summary

| Severity | Count |
|----------|-------|
| CRITICAL | 0     |
| HIGH     | 0     |
| MEDIUM   | 2     |
| LOW      | 2     |

**Verdict: LGTM-with-changes**

The implementation is architecturally sound and correct for v1. All non-negotiable constraints from the plan are satisfied: sole-mutator invariant holds (grep guards verified), `damage_calc` is pure, AI does not touch HP or write `ApplyStatusEvent`, `execute_combat_actions.before(apply_status_handler)` is registered, `check_dead_and_apply` is called after every damage write, the B0002 split is in place, the `std::mem::take` pattern prevents mid-iteration queue mutation. The `rand 0.9` API migration (D-I12 through D-I14) was handled correctly. The `cargo clippy`, `cargo fmt --check`, and all 187 tests pass.

The two MEDIUM findings are pre-ship blockers per the pipeline rules. MEDIUM-1 (missing tests) is straightforward to fix — the code paths exist; the tests were named in the plan and omitted. MEDIUM-2 (`BossAttackDefendAttack` never cycles) is a correctness bug in a stub-only feature; catching it now prevents a silent defect from being inherited by #17 boss authoring.

**Files reviewed (full coverage):**
- `src/plugins/combat/actions.rs` (full — 68 lines)
- `src/plugins/combat/ai.rs` (full — 339 lines)
- `src/plugins/combat/combat_log.rs` (full — 80 lines)
- `src/plugins/combat/damage.rs` (full — 291 lines)
- `src/plugins/combat/enemy.rs` (full — 64 lines)
- `src/plugins/combat/targeting.rs` (full — 146 lines)
- `src/plugins/combat/turn_manager.rs` (full — 1206 lines)
- `src/plugins/combat/ui_combat.rs` (full — 410 lines)
- `src/plugins/combat/mod.rs` (full — 42 lines)
- `src/plugins/party/inventory.rs` (D-A5 region, lines 420-480)
- `Cargo.toml` (full — 52 lines)
- `src/plugins/combat/status_effects.rs` (relevant regions: `apply_status_handler` signature, predicates, `check_dead_and_apply`, `StatusEffectsPlugin::build`)
- `project/plans/20260508-100000-feature-15-turn-based-combat-core.md` (full context — lines 1-260 + decision/pitfall sections)
- `project/implemented/20260508-120000-feature-15-turn-based-combat-core.md` (full — deviations D-I1 through D-I18)
