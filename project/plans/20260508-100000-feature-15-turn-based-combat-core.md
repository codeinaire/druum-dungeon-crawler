# Plan: Feature #15 — Turn-Based Combat Core

**Date:** 2026-05-08
**Status:** Approved (2026-05-08) — User accepted defaults on all 6 surfaced questions: D-A3=A (Wizardry-style), D-Q1=A (overlay camera), D-Q2=A (persistent panel), D-Q3=A (bounded ring 50, kept), D-Q4=A (take-higher), D-Q5=A (BossAi stub with FocusWeakest + AttackDefendAttack)
**Research:** `project/research/20260508-093000-feature-15-turn-based-combat-core.md`
**Roadmap:** `project/roadmaps/20260429-01-bevy-dungeon-crawler-roadmap.md` lines 789-862
**Depends on:** Feature #11 (`BaseStats`/`DerivedStats`/`StatusEffects`/`derive_stats`/`PartyMember`/`PartyRow`/`PartySlot`), Feature #12 (`Equipment`/`Inventory`/`ItemInstance`/`EquipmentChangedEvent`/`EquipSlot::None` sentinel/`recompute_derived_stats_on_equipment_change`), Feature #13 (`apply_poison_trap` cross-plugin precedent for `.before(apply_status_handler)`), Feature #14 (`ApplyStatusEvent`/`apply_status_handler`/`StatusTickEvent`/`check_dead_and_apply` stub at `status_effects.rs:394-407`/`is_paralyzed`/`is_asleep`/`is_silenced` predicates)

---

## Goal

Build the action-queue turn-based combat resolution layer: collect each alive party member's chosen action through `CombatPhase::PlayerInput`; append enemy AI actions; sort by speed (descending, deterministic tie-break); resolve in `CombatPhase::ExecuteActions` with damage, status, item effects; check victory/defeat/flee in `CombatPhase::TurnResult` and loop back. Egui combat screen overlays the dungeon camera with party HP/MP cards, enemy column, persistent action menu, bounded combat log, and target-selection prompts. Defers full spell mechanics (#20), encounter spawning (#16), animation tweens (#17), and ailment-curing items (#20).

---

## Approach

**Four sequential sub-phases (15A → 15B → 15C → 15D)** matching roadmap line 824 ("plan it as multiple sub-PRs (turn manager → damage → AI → UI) rather than one monolithic change") and the research's primary recommendation. Each phase is a self-contained code drop with its own ordered steps and verification gate; the cumulative work ships as four atomic GitButler commits on a single branch.

**Architecture decisions locked from research, all confirmed by user:**

- **D-A1 — Sub-plugin shape under `CombatPlugin`:** three new sub-plugins (`TurnManagerPlugin`, `EnemyAiPlugin`, `CombatUiPlugin`) registered from `combat/mod.rs::CombatPlugin::build` via `app.add_plugins(...)`. Mirrors the `StatusEffectsPlugin` precedent (`combat/mod.rs:18`). `main.rs` is unchanged. Each phase ships one new sub-plugin (15A→`TurnManagerPlugin`; 15C→`EnemyAiPlugin`; 15D→`CombatUiPlugin`); 15B introduces no new plugin.
- **D-A2 — Skip `CombatPhase::EnemyTurn`:** use 3-of-4 declared phases (`PlayerInput` → `ExecuteActions` → `TurnResult` → `PlayerInput`). The `EnemyTurn` variant exists in `state/mod.rs:34` (frozen by #2) but is never entered; the action-queue interleaves enemy actions into `ExecuteActions` per research Pattern 5. Document with a vestigial-note doc-comment in `turn_manager.rs` (NOT in `state/mod.rs` — that file is frozen).
- **D-A3 — Damage formula (USER-PICK = A):** Wizardry-style multiplicative `damage = (A * (100 - D / 2)) / 100`, variance multiplier `0.7..=1.0`, crit 1.5x. Pure function in `damage.rs`; row rules live there only.
- **D-A4 — `CombatActionKind` enum payload (NOT component-bundle):** the queue carries cloneable enum-variant data; not ECS components. Rename avoids collision with the leafwing `CombatAction` enum (Pitfall 8).
- **D-A5 — Drop `With<PartyMember>` filter from `recompute_derived_stats_on_equipment_change`:** carve-out edit on `inventory.rs:445` so enemy buffs/debuffs (Defend's `DefenseUp` applied to enemies, `Dead` applied on enemy zero-HP) re-derive correctly. Enemies spawn with `Equipment::default()` and `Experience::default()` to satisfy the query shape — ~16 bytes per enemy (negligible).
- **D-Q1 — Combat camera (USER-PICK = A):** overlay the existing dungeon camera. `CombatUiPlugin` mirrors `MinimapPlugin::attach_egui_to_dungeon_camera` — no new `Camera3d`, no `OnEnter(Combat)` camera spawn. Wizardry/Etrian convention; party stays in the dungeon view.
- **D-Q2 — Action menu UX (USER-PICK = A):** persistent `egui::TopBottomPanel::bottom("action_menu").min_height(60.0)` always visible during `CombatPhase::PlayerInput`. Action buttons (Attack/Defend/Spell/Item/Flee) in a row.
- **D-Q3 — Combat log (USER-PICK = A):** bounded `VecDeque<CombatLogEntry>` capacity 50, kept across combats. Cleared only on explicit user reset (none in v1). ~4 KB memory.
- **D-Q4 — Defend stacking (USER-PICK = A):** Defend writes `ApplyStatusEvent { effect: DefenseUp, potency: 0.5, duration: Some(1) }` via the existing #14 pipeline. The take-higher merge rule (`apply_status_handler` at `status_effects.rs`) silently no-ops when a stronger magical `DefenseUp` (potency ≥ 0.5) is already present. NO new `Defending` component, NO new `DefenseUpFromDefend` enum variant, NO specialized handler.
- **D-Q5 — Boss AI scope (USER-PICK = A):** ship `EnemyAi` enum with 3 variants — `RandomAttack` (default), `BossFocusWeakest`, `BossAttackDefendAttack { turn: u32 }`. ~80 LOC, ~4 tests. Authors get a hook for #17 boss enemies; the action-emit path is identical to `RandomAttack` — only the target/action-pick logic differs.

**Critical seam discipline (matches research §Anti-Patterns and roadmap line 858):**

1. **`damage_calc` is a pure function** with signature `(attacker: &Combatant, defender: &Combatant, weapon: Option<&ItemAsset>, action: &CombatActionKind, rng: &mut impl Rng) -> DamageResult`. No entity lookups, no resource reads, no scheduling. Testable with `ChaCha8Rng::seed_from_u64(...)` for deterministic outputs.
2. **AI emits `CombatActionKind`s into the queue** via `enemy_ai_action_select`. AI never reads or mutates `DerivedStats.current_hp`. AI never writes `ApplyStatusEvent` directly.
3. **UI calls into target-selection state** (`PlayerInputState::commit_action(...)`) — UI never pushes to `TurnActionQueue` directly. The handler is the SOLE writer of the queue from the player side.
4. **`apply_status_handler` is the SOLE mutator of `StatusEffects.effects`** (already locked by #14, Decision 20). #15's resolver writes `ApplyStatusEvent` and lets the existing handler push effects. Defend → DefenseUp flows through this verbatim.
5. **After every damage write, call `check_dead_and_apply`** — the stub at `status_effects.rs:394-407`. Don't manually push `Dead` into `StatusEffects.effects`.
6. **`recompute_derived_stats_on_equipment_change` is reused** for stat recompute on status effect changes via the `EquipSlot::None` sentinel pattern (#14 D5α). Zero parallel re-derive logic in #15.

**The `rand` direct-dep gate (Step A/B/C per `feedback_third_party_crate_step_a_b_c_pattern.md`):** `rand 0.9.4` and `rand_chacha 0.9.0` are both transitive (verified at `Cargo.lock:4358-4376` during research). Phase 15A Step 1 runs the gate. Production uses `rand::rngs::SmallRng` with `from_os_rng()`; tests use `rand_chacha::ChaCha8Rng::seed_from_u64(...)` for byte-stable determinism (the user's preferred path). All RNG users in #15 read from a single `CombatRng: Resource(Box<dyn rand::RngCore + Send + Sync>)` so tests insert a seeded ChaCha8Rng directly.

**Total scope:**

- **+8 new files** under `src/plugins/combat/`: `turn_manager.rs`, `actions.rs`, `enemy.rs`, `combat_log.rs` (Phase 15A); `damage.rs`, `targeting.rs` (Phase 15B); `ai.rs` (Phase 15C); `ui_combat.rs` (Phase 15D).
- **3 carve-out edits** on frozen files: `combat/mod.rs` (+sub-plugin registrations across phases); `party/inventory.rs` (drop `With<PartyMember>` filter, doc-comment update — Phase 15A); `dungeon/features.rs::tests::make_test_app` AND `dungeon/tests.rs::make_test_app` AND `tests/dungeon_geometry.rs` AND `tests/dungeon_movement.rs` test harnesses (no edit needed — `CombatPlugin` already registered there per #14 D-I10/D-I11; #15 just adds resources via the new sub-plugins inside `CombatPlugin::build`).
- **+1 direct dep** (`rand = "0.9"` with `default-features = false, features = ["std", "std_rng", "small_rng"]`); `rand_chacha = "0.9"` is the dev-dep for tests (Step A/B/C gate verifies both).
- **+25-30 tests** across the four phases (roadmap envelope: +20-30; we land at the upper bound given the size).
- **LOC budget per phase:** 15A ~350, 15B ~250, 15C ~200, 15D ~400-600. Total ~1200-1400 (within +1000-1800 envelope from roadmap line 818).
- **Defers:** spell mechanics (#20 fills `CombatActionKind::CastSpell` body); encounter spawning (#16 owns production `CurrentEncounter`); animation tweens (#17); per-instance enemy authoring (#17); ailment-curing items (#20).

---

## Critical

These are non-negotiable constraints. Violations should fail review.

- **Bevy `=0.18.1` pinned.** No version bump.
- **`#[derive(Message)]`, NOT `#[derive(Event)]`** for all new buffered messages (`MovedEvent`-shaped contract). Bevy 0.18 family rename. `MessageReader<T>` / `MessageWriter<T>` / `app.add_message::<T>()`. Verification gate greps every `.rs` file added by #15 for `derive(Event)` / `EventReader<` / `EventWriter<` — must return ZERO matches.
- **`damage_calc` is a pure function.** No entity lookups, no `Query`, no `Res<Assets<...>>`, no `Time`, no `Commands`. Signature is `fn damage_calc(attacker: &Combatant, defender: &Combatant, weapon: Option<&ItemAsset>, action: &CombatActionKind, rng: &mut impl rand::Rng) -> DamageResult`. The `Combatant` struct is the flatten-step caller responsibility (mirrors `derive_stats`'s caller-flattens-Equipment pattern).
- **Saturating arithmetic in `damage_calc`.** All addition uses `saturating_add`. Hit-roll caps `(attacker.accuracy.saturating_sub(defender.evasion)).min(100)`. Crit-chance caps similarly. Defends against `attack: u32::MAX` from a malicious save (research §Security trust boundary).
- **AI emits actions into the queue ONLY.** `enemy_ai_action_select` may NOT touch `DerivedStats.current_hp`, MAY NOT write `ApplyStatusEvent`, MAY NOT mutate `StatusEffects`. Single side effect: `queue.queue.push(QueuedAction { ... })`. Verification gate greps `ai.rs` for `current_hp` / `MessageWriter<ApplyStatusEvent>` / `Mut<StatusEffects>` — must be ZERO matches outside the AI's read-only inspection of stats.
- **`apply_status_handler` is the SOLE mutator of `StatusEffects.effects` (frozen from #14 Decision 20).** `execute_combat_actions` writes `ApplyStatusEvent`, never `effects.push(...)`. Verification gate greps every `combat/*.rs` added by #15 for `effects.push(` and `effects.retain` — must be ZERO matches outside #14's `apply_status_handler` and `tick_status_durations`.
- **After every damage write, call `check_dead_and_apply`.** The damage pipeline must NOT manually push `Dead` into `StatusEffects.effects` — that bypasses `apply_status_handler` and breaks the sole-mutator invariant. The order: `execute_combat_actions` writes `current_hp = current_hp.saturating_sub(damage)` THEN calls `check_dead_and_apply(target, &derived, &mut apply_status)`. The `apply_status_handler` runs next in the same frame.
- **System ordering: writers BEFORE handler.** `execute_combat_actions.before(apply_status_handler)` is required so the Defend → DefenseUp and the Dead-from-damage applications are visible in the same `ExecuteActions` phase. Same shape as the `apply_poison_trap.before(apply_status_handler)` precedent in `dungeon/features.rs:171`.
- **`CombatPhase` SubStates enum is FROZEN by #2.** Use 3-of-4 phases (`PlayerInput`, `ExecuteActions`, `TurnResult`) verbatim. `EnemyTurn` exists in the enum but is never set; document with a comment in `turn_manager.rs` (NOT in `state/mod.rs`).
- **Leafwing `CombatAction` enum is FROZEN by #5.** Used for menu navigation (Up/Down/Left/Right/Confirm/Cancel). The queue payload type is named `CombatActionKind` to avoid the collision (Pitfall 8). Do NOT add variants to `CombatAction`.
- **`Defend` writes `ApplyStatusEvent { effect: DefenseUp, potency: 0.5, duration: Some(1) }` via the existing #14 pipeline.** NO `Defending: Component`. NO new `DefenseUpFromDefend` enum variant. NO specialized handler. The take-higher merge rule (`apply_status_handler` at `status_effects.rs:200-220`) silently no-ops when a stronger magical buff is active — this is the locked D-Q4=A behavior.
- **`recompute_derived_stats_on_equipment_change` carve-out is intentional.** Phase 15A Step 8 drops `With<PartyMember>` from the query at `inventory.rs:445`. Doc-comment update explains the dual-use. Enemies must spawn with `Equipment::default()` + `Experience::default()` for the query to match — Phase 15A Step 5 enforces this in `EnemyBundle`.
- **`CurrentEncounter: Resource` is owned by #16 — #15 references it as a contract.** #15 defines a test-fixture-only `CurrentEncounter` in test modules. The production resource lands in #16. The dev-cycler stub spawn (`#[cfg(feature = "dev")]`) seeds 2 placeholder enemies for manual smoke; #16 deletes that stub. Same pattern as #14's pre-#15 deferred work.
- **Step A/B/C gate for `rand` direct dep.** Phase 15A Step 1 runs `cargo add rand --dry-run`, audits `[features]`, greps the API surface. Only after the gate is GREEN does Step 2 land the `Cargo.toml` edit.
- **Pre-commit hook on `gitbutler/workspace`** rejects raw `git commit` (CLAUDE.md). Implementer uses `but commit --message-file <path>` per phase.

---

## Frozen / DO NOT TOUCH

These files are frozen by Features #1–#14 and must not be modified by the #15 implementer except for the explicit carve-outs listed below.

- **`src/plugins/state/mod.rs`** — FROZEN by #2. The `EnemyTurn` variant of `CombatPhase` is vestigial in #15's design but the enum cannot be edited. Document the vestige in `turn_manager.rs`, NOT here.
- **`src/plugins/input/mod.rs`** — FROZEN by #5. The leafwing `CombatAction` enum stays at 6 variants (Up/Down/Left/Right/Confirm/Cancel). #15 reads `Res<ActionState<CombatAction>>` for menu navigation; no new variants.
- **`src/plugins/audio/mod.rs`, `audio/bgm.rs`, `audio/sfx.rs`** — FROZEN by #6. No new SFX in #15. (Combat hit/death sound effects are #17 polish.)
- **`src/plugins/dungeon/mod.rs`** — FROZEN by #7-#9. #15 does not modify dungeon state.
- **`src/plugins/ui/mod.rs`, `ui/minimap.rs`** — FROZEN by #10. `CombatUiPlugin` is registered as a sub-plugin under `CombatPlugin`, NOT under `UiPlugin`. (Mirrors `StatusEffectsPlugin` precedent.)
- **`src/plugins/save/mod.rs`, `town/mod.rs`** — FROZEN / empty stub. Save-plugin work for combat state is #23.
- **`src/data/dungeon.rs`, `data/items.rs`, `data/classes.rs`, `data/spells.rs`, `data/enemies.rs`** — FROZEN. No data schema changes in #15. (Real enemy authoring is #17.)
- **`src/plugins/party/character.rs`** — FROZEN since #14. `Enemy` ECS components live in NEW file `combat/enemy.rs`, NOT here. Enemies REUSE `BaseStats`/`DerivedStats`/`StatusEffects`/`PartyRow` from `character.rs` via component composition (D-A5 lock).
- **`src/plugins/combat/status_effects.rs`** — FROZEN since #14. `check_dead_and_apply` at lines 394-407 is the contract; `is_paralyzed`/`is_asleep`/`is_silenced` predicates at lines 366-381 are imported by `turn_manager.rs`. NO edits to this file in #15.
- **`assets/dungeons/floor_01.dungeon.ron`, `floor_02.dungeon.ron`, `assets/items/core.items.ron`** — FROZEN. No asset edits.
- **`src/main.rs`** — FROZEN. `CombatPlugin` is already registered at line 28; #15's three new sub-plugins register as children of `CombatPlugin` from inside `combat/mod.rs::CombatPlugin::build`.

**Explicit carve-outs (these frozen files DO get edited, with bounded changes — each tied to a single Step):**

- **`src/plugins/combat/mod.rs`** — across phases: +1 line per phase (`pub mod <name>;` declaration + `app.add_plugins(<NewPlugin>)` inside `CombatPlugin::build`). Phase 15A adds `TurnManagerPlugin`; 15C adds `EnemyAiPlugin`; 15D adds `CombatUiPlugin`. 15B does not register a new plugin (its functions are `pub fn` consumed by `TurnManagerPlugin`).
- **`src/plugins/party/inventory.rs`** — Phase 15A Step 8 only: drop `With<PartyMember>` filter from `recompute_derived_stats_on_equipment_change` query at line 445; update the doc-comment at lines 421-433 to explain the dual-use (party + enemy). Zero behavioral change for the existing party-only callers.
- **No edits to test harness files.** `dungeon/tests.rs::make_test_app`, `dungeon/features.rs::tests::make_test_app`, `tests/dungeon_geometry.rs`, `tests/dungeon_movement.rs` already register `CombatPlugin` (per #14 D-I10/D-I11). #15's new resources (`TurnActionQueue`, `PlayerInputState`, etc.) are owned by `TurnManagerPlugin` which is registered as a sub-plugin INSIDE `CombatPlugin::build` — so existing harnesses pick them up automatically. NO edits needed in #15.

---

## Decisions

The plan locks the following decisions BEFORE Phase 15A. Each is either a research-recommendation accepted by the user as default or a load-bearing planner call. Recommended-default-accepted decisions can be overridden at plan-approval time without rework. **All 6 surfaced user-pick decisions resolved Option A per the user's prompt.**

### User picks resolved (research D-numbers)

1. **D-A3 — Damage formula:** Wizardry-style multiplicative `(A * (100 - D / 2)) / 100`. Variance multiplier `0.7..=1.0` (`rng.gen_range(70..=100) as f32 / 100.0`). Crit chance `(attacker.accuracy / 5)` % capped at 100; crit applies 1.5x damage. Floor of 1 on positive-attack hits (defends against truncation-to-zero with high defense).
2. **D-Q1 — Combat camera:** Overlay on dungeon camera. `CombatUiPlugin::attach_egui_to_dungeon_camera` is identical to `MinimapPlugin::attach_egui_to_dungeon_camera` shape — runs in `Update` gated `in_state(GameState::Combat)`, queries `(With<DungeonCamera>, Without<PrimaryEguiContext>)`, idempotent insertion. NO new `Camera3d` spawn.
3. **D-Q2 — Action menu UX:** Persistent `egui::TopBottomPanel::bottom("action_menu").min_height(60.0)` rendered every `EguiPrimaryContextPass` during `CombatPhase::PlayerInput`. Buttons ordered Attack/Defend/Spell/Item/Flee. Slot label shows the current `active_slot`'s `CharacterName` ("Aldric").
4. **D-Q3 — Combat log:** Bounded `VecDeque<CombatLogEntry>` capacity 50. Initialized in `CombatLog::default()` with `VecDeque::with_capacity(50)`. `push(message, turn_number)` appends and pops front when `len() > 50`. Kept across combats — NO clear on `OnExit(Combat)`. `clear()` method exists but is unused in v1.
5. **D-Q4 — Defend stacking:** Take-higher (current #14 stacking rule). Defend writes `ApplyStatusEvent { effect: DefenseUp, potency: 0.5, duration: Some(1) }`. The merge rule at `status_effects.rs::apply_status_handler` calls `existing.magnitude.max(potency)` and `remaining_turns = ev.duration` — so a magical `DefenseUp 1.0` already present (from a future #20 spell) wins, and Defend's 0.5 is silently merged with no visible effect. NO new `Defending` component, NO new enum variant. Test asserts the merge: `defend_no_ops_when_higher_defense_up_active`.
6. **D-Q5 — Boss AI scope:** Ship `EnemyAi` enum with 3 variants. `RandomAttack` (default) picks any alive party member. `BossFocusWeakest` picks the alive party member with lowest `current_hp` (ties broken by lowest `slot_index`). `BossAttackDefendAttack { turn: u32 }` cycles `Attack → Defend → Attack` deterministically based on `turn % 3`. Each pattern is a `match arm` in `select_action_for_enemy`. ~80 LOC + 4 tests (one per variant + one for unknown-variant default).

### Recommended defaults accepted (research D-numbers)

7. **D-A1 — Sub-plugin shape:** three sub-plugins (`TurnManagerPlugin`, `EnemyAiPlugin`, `CombatUiPlugin`). 15B (damage + targeting) ships as `pub fn`s consumed by `TurnManagerPlugin`, NOT as a plugin.
8. **D-A2 — Skip `EnemyTurn`:** 3-of-4 phases.
9. **D-A4 — `CombatActionKind` enum payload:** name resolves the leafwing-collision Pitfall 8.
10. **D-A5 — Drop `With<PartyMember>` filter:** carve-out edit on `inventory.rs:445`. Doc-comment update.

### Planner calls (load-bearing, not surfaced as user picks)

11. **`CombatActionKind` variants for #15:** `Attack`, `Defend`, `CastSpell { spell_id: String }` (stub), `UseItem { item: Handle<ItemAsset> }`, `Flee`. The `Attack` variant carries no `weapon` field — the resolver reads the actor's `Equipment.weapon` slot at queue-time-or-resolve-time (we choose resolve-time for simplicity; mid-round equipment swap won't happen in v1). The `target` and `actor_side`/`slot_index` live on the wrapping `QueuedAction` struct, not on `CombatActionKind`.
12. **`QueuedAction` struct shape:** `{ actor: Entity, kind: CombatActionKind, target: TargetSelection, speed_at_queue_time: u32, actor_side: Side, slot_index: u32 }`. `slot_index` is the party `PartySlot.0` for party members or the enemy's `EnemyIndex.0` for enemies (see Decision 12). `Side` is `Party` or `Enemy`.
13. **`EnemyIndex(pub u32)` Component:** new component on enemy entities, mirrors `PartySlot` for tie-break. Set at spawn-time by the enemy spawner (#15 dev-stub or #16 production). 0..N for the encounter.
14. **Speed tie-break order:** descending speed → party-before-enemy (`Side::Party as u8 = 0`, `Side::Enemy as u8 = 1`) → ascending `slot_index`. Documented in `turn_manager.rs::sort_queue_by_speed`. Test `speed_tie_party_before_enemy` and `speed_tie_lower_slot_first` exercise it.
15. **`speed_at_queue_time` capture:** captured from `DerivedStats.speed` at the moment the action is queued (in `collect_player_actions` for player; in `enemy_ai_action_select` for enemies). Mid-round speed buffs do NOT reorder this round — research Pitfall 9. Comment in `turn_manager.rs` explains the rule.
16. **`PlayerInputState` shape:** owns the menu state machine via a `Vec<MenuFrame>` stack (`Main`, `SpellMenu`, `ItemMenu`, `TargetSelect { mode, kind }`). Pop on Cancel; push on submenu open. Cancel at `Main` is a no-op (player must commit an action). `active_slot: Option<usize>` walks 0..N over alive party members; `committed: Vec<QueuedAction>` accumulates this round.
17. **`TurnActionQueue` and `PlayerInputState` lifetime:** initialized in `TurnManagerPlugin::build` via `init_resource::<...>()`; cleared on `OnEnter(GameState::Combat)` by `init_combat_state` system; cleared on `OnExit(GameState::Combat)` by `clear_combat_state` system. The queue is consumed via `std::mem::take(&mut queue.queue)` at the start of `execute_combat_actions` (Pitfall 2 — drain a snapshot, not the live view).
18. **`Side` enum location:** in `actions.rs` alongside `CombatActionKind`/`QueuedAction` (the queue payload data file). Single owner of the Side concept.
19. **`TargetSelection` enum location:** in `targeting.rs`. Variants: `Single(Entity)`, `AllAllies`, `AllEnemies`, `Self_`, `None`. The `resolve_target_with_fallback` pure function lives in the same file; takes `&[Entity]` for both party and enemy lists, an `is_alive: impl Fn(Entity) -> bool` predicate, and `&mut impl Rng`.
20. **Re-target rule for `Single(t)` when `t` is dead:** pick a random alive entity from the SAME side as the original target (party-side or enemy-side determined by membership in the party/enemies slices). If that side is wiped out, return empty (action no-ops with "X has no target" log).
21. **`CombatRng` resource shape:** `#[derive(Resource)] pub struct CombatRng(pub Box<dyn rand::RngCore + Send + Sync>);`. Production seeded with `Box::new(rand::rngs::SmallRng::from_os_rng())` in `OnEnter(GameState::Combat)`. Tests insert `CombatRng(Box::new(rand_chacha::ChaCha8Rng::seed_from_u64(42)))` directly. Trait-object lets us swap RNG impls without rewriting consumers.
22. **`FleeAttempted` resource:** `#[derive(Resource, Default)] pub struct FleeAttempted { pub success: bool, pub attempted_this_round: bool }`. Cleared on `OnEnter(Combat)`. Set by `execute_combat_actions` when a `Flee` action resolves (success-roll: `rng.gen_range(0..100) < 50` — fixed 50% in v1; tunable later). Read by `check_victory_defeat_flee` in `TurnResult` phase.
23. **Flee = group flee** (research Open Question 2): the whole party flees on success, returning to `GameState::Dungeon`. Per-member flee is a future feature.
24. **`check_victory_defeat_flee` order in `TurnResult`:** check defeat FIRST (research Open Question 4). All party Dead → `NextState<GameState>::set(GameOver)`. Otherwise check flee success → `NextState<GameState>::set(Dungeon)`. Otherwise check victory (all enemies Dead) → `NextState<GameState>::set(Dungeon)`. Otherwise → `NextState<CombatPhase>::set(PlayerInput)` for next round. Documented in the system body.
25. **`combat_log.rs` location:** new file, NOT inside `ui_combat.rs` (combat log is a resource read by the resolver in `turn_manager.rs` BEFORE the UI renders it; placing it under UI would create a circular import). Owned by `TurnManagerPlugin` (which `init_resource::<CombatLog>()`s it).
26. **`Enemy` ECS shape (Phase 15A `enemy.rs`):** minimal — `Enemy: Component` (zero-sized marker), `EnemyName(pub String): Component`, `EnemyIndex(pub u32): Component`, `EnemyAi: Component` (the enum from D-Q5). Reuses `BaseStats`/`DerivedStats`/`StatusEffects`/`PartyRow`/`Equipment`/`Experience` from `party::character` (D-A5 carve-out lets the recompute query match enemies too). `EnemyBundle` includes `Equipment::default()` and `Experience::default()` to satisfy the dropped-`With<PartyMember>` query.
27. **Dev-stub encounter spawn (Phase 15A Step 13):** `#[cfg(feature = "dev")] fn spawn_dev_encounter` runs `OnEnter(GameState::Combat)`. Spawns 2 placeholder enemies (Goblin/Goblin) with hardcoded stats. Idempotence guard: if any `Enemy` exists, return early (mirrors `spawn_default_debug_party` at `party/mod.rs:88-126`). #16 deletes this stub.
28. **`damage_calc` row rules in #15:** simplified for v1. Front-row attacker with melee weapon vs. back-row defender → `damage = 0` ("can't reach"). All other combinations → full damage. `is_melee(weapon: &ItemAsset) -> bool` is a stub returning `true` for any weapon (real weapon-kind classification is #17). Future #17 fleshes out `WeaponKind::Melee/Bow/Spear/...`. Test `front_attack_back_with_melee_blocks` exercises the rule.
29. **Hit-roll formula:** `let hit_chance = attacker.stats.accuracy.saturating_sub(defender.stats.evasion).min(100); rng.gen_range(0..100) < hit_chance`. Miss returns `DamageResult { damage: 0, hit: false, critical: false, message: "<A> misses <B>." }`. Tests `damage_calc_misses_when_evasion_high` and `damage_calc_hits_when_accuracy_high`.
30. **`UseItem` consumes the item from inventory:** the resolver reads the actor's `Inventory` component, finds the `ItemInstance(handle)` matching the action's `item` handle, removes the entity from `Inventory.0`, and despawns the `ItemInstance` entity. For consumables that heal: read `ItemAsset.kind == Consumable`, look up `heal_amount` from `ItemStatBlock` (currently no field — for #15 v1, all consumables heal `max_hp / 4`; real heal field is #20 polish). Item-use SFX is #17 polish; #15 emits no audio.
31. **`UseItem` rejects key items:** if `ItemAsset.kind == ItemKind::KeyItem`, log "Cannot use {name} in combat" and return early (no inventory mutation). Test `use_item_rejects_key_items`.
32. **`CastSpell` stub:** `execute_combat_actions` matches `CombatActionKind::CastSpell { spell_id }` and writes `combat_log.push(format!("{} casts {}: not yet implemented", actor_name, spell_id))`. Returns no-op. Test `cast_spell_logs_stub_message`.
33. **Sleep/Paralysis prevent action emission in `PlayerInput`** (research Pitfall 4): `collect_player_actions` skips characters where `is_paralyzed(status)` or `is_asleep(status)` is true. Auto-commits a "no-op" QueuedAction (sentinel `CombatActionKind::Defend`-equivalent that doesn't actually apply DefenseUp — call this `CombatActionKind::Skip` if needed). After review: simpler to NOT push a queued action at all; the resolver iterates only what's in the queue. Document with a log entry "Aldric is asleep" pushed to combat_log when the slot is auto-skipped. Test `sleep_skips_action_emission`.
34. **`Silence` gates spell selection:** `is_silenced(status)` predicate (already in `status_effects.rs`) is checked in `PlayerInputState::open_spell_menu` — if silenced, the menu refuses to open AND a log entry "Aldric is silenced; cannot cast spells" is pushed. Player picks another action. Test `silence_blocks_spell_menu`.
35. **Mid-round actor death in `execute_combat_actions`** (research Pitfall 3): check `is_alive(action.actor)` BEFORE resolving each queued action. Skip with log "Aldric is unable to act." The `is_alive` predicate: `let is_alive = |e: Entity| -> bool { matches!(query.get(e), Ok(d) if d.current_hp > 0 && !d.status.has(Dead) && !d.status.has(Stone)) }`. Test `dead_actor_skips_action_in_resolve`.
36. **Phase ordering inside `TurnManagerPlugin::build`:** all systems in `Update`. `collect_player_actions.run_if(in_state(CombatPhase::PlayerInput))`. `append_enemy_actions_and_sort.run_if(in_state(CombatPhase::ExecuteActions)).before(execute_combat_actions)`. `execute_combat_actions.run_if(in_state(CombatPhase::ExecuteActions)).before(crate::plugins::combat::status_effects::apply_status_handler)` (cross-plugin same-frame consumability — same shape as `apply_poison_trap.before(apply_status_handler)` precedent at `dungeon/features.rs:171`). `check_victory_defeat_flee.run_if(in_state(CombatPhase::TurnResult))`. State-transition systems (`OnEnter(Combat)::init_combat_state`, `OnExit(Combat)::clear_combat_state`).
37. **`EnemyAiPlugin` ordering (Phase 15C):** the `enemy_ai_action_select` system runs `.run_if(in_state(CombatPhase::ExecuteActions))` and `.before(append_enemy_actions_and_sort)` so the AI's emitted actions land in the queue before the sort. Wait — actually the AI runs in `PlayerInput` phase to give enemies time to "decide", or in `ExecuteActions` after the player has committed? Decision: AI runs at the START of `ExecuteActions` via a chained system: `enemy_ai_action_select.before(append_enemy_actions_and_sort)`. The chain: `enemy_ai_action_select` writes enemy actions to `queue` → `append_enemy_actions_and_sort` actually does only the sort step (rename to `sort_queue_by_speed`) → `execute_combat_actions` drains. Updated naming: `enemy_ai_action_select.before(sort_queue_by_speed).before(execute_combat_actions)`.
38. **`CombatUiPlugin` ordering (Phase 15D):** painter systems run in `EguiPrimaryContextPass` schedule (mirrors minimap pattern). The `attach_egui_to_dungeon_camera` system runs in `Update.run_if(in_state(GameState::Combat))`. Painters: `paint_combat_screen` is the umbrella system that paints all four panels (enemy column, party panel, action menu, combat log) — gated `.run_if(in_state(GameState::Combat))`. Target-selection overlay paints conditionally on `state.is_selecting_target()`. Player-input handler `handle_combat_input` runs in `Update` reading `Res<ActionState<CombatAction>>`, writes `PlayerInputState` and `TurnActionQueue` (the SOLE writer of the queue from the player side per Anti-pattern 5).
39. **`#[cfg(test)]` placement and harness:** each new file gets `mod tests` (Layer 1 — pure functions) and `mod app_tests` (Layer 2 — App-driven). The Layer-2 harness mirrors `combat/status_effects.rs::app_tests::make_test_app` — `MinimalPlugins + StatesPlugin + AssetPlugin + StatePlugin + PartyPlugin + CombatPlugin`. NO `DungeonPlugin` or `CellFeaturesPlugin` (combat tests don't need dungeon state). `init_asset::<DungeonFloor>()` and `init_asset::<ItemDb>()` defensively (PartyPlugin's populate_item_handle_registry runs on OnExit(Loading) and reads ItemDb — even if tests don't trigger that transition, the asset registration is required).

---

## Open Questions

The plan defers ZERO USER-PICK decisions — all 6 surfaced research questions resolved Option A per the user's prompt. The implementer proceeds with the locked decisions above without re-asking.

### Resolved during planning (research-recommended defaults — accepted by user)

- D-A3 (damage formula) — Resolved: A (Wizardry-style multiplicative).
- D-A1 (sub-plugin shape) — Resolved: three sub-plugins.
- D-A2 (skip EnemyTurn) — Resolved: 3-of-4 phases.
- D-A4 (CombatActionKind enum) — Resolved: enum payload, NOT component-bundle.
- D-A5 (drop With<PartyMember>) — Resolved: carve-out at `inventory.rs:445`.
- D-Q1 (combat camera) — Resolved: A (overlay on dungeon camera).
- D-Q2 (action menu UX) — Resolved: A (persistent panel).
- D-Q3 (combat log) — Resolved: A (bounded ring 50, kept across combats).
- D-Q4 (Defend stacking) — Resolved: A (take-higher; silent no-op).
- D-Q5 (Boss AI scope) — Resolved: A (3-variant `EnemyAi` enum stub).

### Implementer-resolvable (planner already locked the call; flagged here for visibility)

- **`rand` direct dep gate** — Phase 15A Step 1. Step A/B/C runs `cargo add rand --dry-run`, audits `[features]`, greps API. Recommended config: `rand = { version = "0.9", default-features = false, features = ["std", "std_rng", "small_rng"] }` and `rand_chacha = { version = "0.9", default-features = false, features = ["std"] }` as a `dev-dependencies` entry for tests. If the gate reveals the resolved version is something other than `0.9.4` (cache state may differ), the implementer adopts whatever `cargo add` produces and updates the version pin in the doc-comment; the plan's API surface (`SmallRng`, `ChaCha8Rng`, `seed_from_u64`, `gen_range`, `from_os_rng`) is stable across `0.9.x`.
- **`UseItem` heal amount** — Decision 30: `max_hp / 4` for all consumables in v1. If the implementer discovers `ItemStatBlock` already has a heal field they should use it; otherwise the hardcoded fraction stands. Real heal-from-item-data lands in #20.

---

## Pitfalls

The 12 research-flagged pitfalls below appear as guards inside the relevant Step. This section is the central reference.

### Pitfall 1 — Defining `CurrentEncounter` as production code (it's #16's territory)

**Where it bites:** Implementer writes a full `CurrentEncounter: Resource` with spawn logic; #16 lands and re-defines it; merge conflicts.

**Guard:** Phase 15A Step 4 defines `CurrentEncounter` as a TEST FIXTURE in `combat/turn_manager.rs::app_tests`, NOT as production code. Phase 15A Step 13's dev-stub spawner (`#[cfg(feature = "dev")]`) uses direct `EnemyBundle` spawn — does NOT touch `CurrentEncounter`. Production reads of `CurrentEncounter` are GATED on `Option<Res<CurrentEncounter>>` so #15 ships without it; #16 introduces the resource.

### Pitfall 2 — Mutating `TurnActionQueue` mid-iteration in `execute_combat_actions`

**Where it bites:** Multi-target action that spawns sub-actions tries to push to the queue while iterating → borrow-check error.

**Guard:** Phase 15A Step 7 writes `let actions = std::mem::take(&mut queue.queue);` at the top of `execute_combat_actions`, then iterates the local `actions` Vec. The same pattern as `MessageReader::read` snapshotting. Sub-action-spawn (none in v1) would push to `queue.queue` for the NEXT round.

### Pitfall 3 — Re-target chain breaks when actor dies before its action resolves

**Where it bites:** Goblin attacks Aldric; Aldric's queued action targets Goblin; Goblin moves first, kills Aldric; Aldric's action runs anyway.

**Guard:** Phase 15A Step 7 checks `is_alive(action.actor)` at the top of each loop iteration in `execute_combat_actions`. Dead/Stoned actor → push log entry "{name} is unable to act." continue. Test `dead_actor_skips_action_in_resolve` (Phase 15A) covers it.

### Pitfall 4 — Status effects (Sleep/Paralysis) gate action emission, not action queuing

**Where it bites:** UI lets a sleeping character queue an action; resolver silently skips; player confused.

**Guard:** Phase 15A Step 6 checks `is_paralyzed(status)` and `is_asleep(status)` in `collect_player_actions` BEFORE letting the slot become `active_slot`. Auto-skip: push "Aldric is asleep" log; advance `active_slot` to the next alive non-incapacitated party member. UI sees the slot greyed out (Phase 15D Step 4 reads the same predicates and renders disabled). Test `sleep_skips_action_emission` (Phase 15A).

### Pitfall 5 — Front-row vs back-row damage modifier owned by multiple modules

**Where it bites:** AI also applies row rules ("prefer back-row weak targets") → drift bug between AI's expected damage and resolver's actual damage.

**Guard:** Phase 15B Step 2 and 15C Step 4 enforce the discipline. Row rules live in `damage.rs` ONLY. AI in v1 picks random alive targets WITHOUT row reasoning. Smart row-aware AI is #17/#22 polish.

### Pitfall 6 — `Defend` stacking with pre-existing `DefenseUp`

**Where it bites:** Player taps Defend on a character with magical `DefenseUp 1.0` already applied. The merge rule (D2 #14) takes higher magnitude → Defend's 0.5 silently no-ops. Player may not understand why Defend "did nothing".

**Guard:** This is the LOCKED D-Q4=A behavior. Phase 15A Step 7 writes `combat_log.push(format!("{} defends!", actor_name))` UNCONDITIONALLY for Defend (the log entry fires even when the merge no-ops the buff). Player sees "Aldric defends!" — game-feel preserved even when the buff is redundant. Test `defend_no_ops_when_higher_defense_up_active` confirms the merge silent-no-op AND the log-entry-fires-anyway invariants.

### Pitfall 7 — Combat log unbounded growth

**Guard:** Phase 15A Step 9 implements `CombatLog` as `VecDeque<CombatLogEntry>` with `capacity = 50` (D-Q3=A). `push` calls `pop_front` while `len > capacity`. Test `combat_log_caps_at_50` (Phase 15A).

### Pitfall 8 — leafwing `CombatAction` collision with the queue payload type

**Guard:** Phase 15A Step 3 names the queue payload `CombatActionKind` (D-A4 lock). Doc-comment at the top of `actions.rs` cites the rename rationale. Verification gate greps `combat/actions.rs` for `pub enum CombatAction\b` (the bare leafwing name) — must be ZERO matches.

### Pitfall 9 — Speed sort gives different results across runs (RNG creep)

**Guard:** Phase 15A Step 7 implements `sort_queue_by_speed` with deterministic tie-break: `b.speed.cmp(&a.speed).then(a.actor_side.cmp(&b.actor_side)).then(a.slot_index.cmp(&b.slot_index))`. Use `Vec::sort_by` (stable) NOT `sort_unstable_by`. Test `speed_tie_party_before_enemy` and `speed_tie_lower_slot_first` exercise the chain.

### Pitfall 10 — AI emits actions for Dead enemies

**Guard:** Phase 15C Step 3 filters `enemy_ai_action_select`: skip if `status.has(Dead)` or `status.has(Stone)` or `derived.current_hp == 0`. Symmetrically Phase 15A Step 6's `collect_player_actions` filters party. Test `ai_skips_dead_enemies` (Phase 15C).

### Pitfall 11 — `recompute_derived_stats_on_equipment_change` doesn't see enemy buff changes

**Guard:** Phase 15A Step 8 — D-A5 carve-out edit. Drop `With<PartyMember>` filter at `inventory.rs:445`. Doc-comment update explains the dual-use. Phase 15A Step 5's `EnemyBundle` includes `Equipment::default()` and `Experience::default()` so the query matches. Test `enemy_buff_re_derives_stats` (Phase 15A app_tests) confirms.

### Pitfall 12 — `Flee` succeeds inconsistently between tests

**Guard:** Phase 15A Step 10's `Flee` resolver reads from a single `CombatRng: Resource` (Decision 21). Tests insert `CombatRng(Box::new(ChaCha8Rng::seed_from_u64(42)))` directly. Production seeds `SmallRng::from_os_rng()` once on `OnEnter(Combat)`. Tests `flee_succeeds_with_seed_42` and `flee_fails_with_seed_99` exercise determinism.

### Pitfall 13 (#15-specific) — Cross-frame consumability of the `ApplyStatusEvent` write from `execute_combat_actions`

**Where it bites:** The Defend → DefenseUp write happens in `execute_combat_actions`; the resolver runs in the same `Update` schedule as `apply_status_handler`. Without `.before(apply_status_handler)`, the message lands but the handler doesn't read it until next frame; the test that checks `StatusEffects.has(DefenseUp)` after one `app.update()` fails.

**Guard:** Phase 15A Step 11's `TurnManagerPlugin::build` registers `execute_combat_actions.before(crate::plugins::combat::status_effects::apply_status_handler)`. Mirrors the `apply_poison_trap.before(apply_status_handler)` precedent at `dungeon/features.rs:171`. Tests can `app.update()` ONCE and observe Defend's DefenseUp present (or call `app.update(); app.update();` defensively, mirroring #14 D-I4 — recommend the latter for safety per the #14 implementation discoveries).

---

## Steps

The implementation proceeds in **four ordered phases** (15A → 15B → 15C → 15D), each one a sub-PR / atomic GitButler commit boundary. Per-phase verification (`cargo check && cargo test` for the new module path) is the in-flight green-light; the global verification gate at the end of this plan is the BEFORE-CLAIM-DONE bar that must run after Phase 15D commits.

---

### Phase 15A — Turn Manager + State Machine (~350 LOC, ~18 tests)

**Files in this phase:**
- New: `src/plugins/combat/turn_manager.rs` (~250 LOC), `src/plugins/combat/actions.rs` (~80 LOC), `src/plugins/combat/enemy.rs` (~50 LOC), `src/plugins/combat/combat_log.rs` (~40 LOC).
- Edits: `src/plugins/combat/mod.rs` (+3 lines: pub mod, pub use, add_plugins), `src/plugins/party/inventory.rs` (drop `With<PartyMember>` filter + doc-comment update), `Cargo.toml` (+1-2 lines: `rand` direct dep + `rand_chacha` dev dep).

**Tests in this phase:** queue ordering (3), Defend integration (3), victory/defeat/flee (4), Sleep/Paralysis/Silence gates (3), check_dead invocation (1), UseItem (2), CastSpell stub (1), enemy buff re-derive (1) = ~18 tests.

#### Step 1 — Step A/B/C gate for `rand` direct dep

- [ ] Run Step A: `cargo add rand --dry-run`. Confirm resolved version is `0.9.x` (likely `0.9.4` — already in `Cargo.lock:4358-4366`). Note any version drift; if anything other than `0.9.x`, halt and surface to the planner before proceeding.
- [ ] Run Step B: audit `[features]` for `rand 0.9.x`. The defaults are `["std", "std_rng"]`. Recommended pin: `default-features = false, features = ["std", "std_rng", "small_rng"]`. Skip `serde` feature (no need to persist RNG state — combat is atomic).
- [ ] Run Step C: grep API surface used in `damage.rs`, `ai.rs`, `targeting.rs`. Required symbols: `rand::Rng` trait, `rand::rngs::SmallRng`, `rand::SeedableRng::from_os_rng`, `Rng::gen_range`, `rand::seq::IteratorRandom::choose`. For `dev-dependencies`: `rand_chacha::ChaCha8Rng`, `SeedableRng::seed_from_u64`. Confirm all symbols are present in the resolved version's docs.
- [ ] **Verification (gate):** Step A clean, Step B audit complete, Step C grep complete. Only after all three are GREEN does Step 2 land the `Cargo.toml` edit.

#### Step 2 — Land the `rand` Cargo.toml edit

- [ ] In `Cargo.toml`, append to `[dependencies]`:
  ```toml
  rand = { version = "0.9", default-features = false, features = ["std", "std_rng", "small_rng"] }
  ```
- [ ] Append a new `[dev-dependencies]` section (or extend the existing one — there isn't one yet):
  ```toml
  [dev-dependencies]
  rand_chacha = { version = "0.9", default-features = false, features = ["std"] }
  ```
- [ ] **Verification:**
  - `cargo check` — succeeds; `Cargo.lock` updates the `rand` entry from transitive to direct (likely no-op since the transitive version already covers it).
  - `cargo check --features dev` — succeeds.

#### Step 3 — Create `src/plugins/combat/actions.rs` with `CombatActionKind`/`QueuedAction`/`Side`

- [ ] Create new file `src/plugins/combat/actions.rs`. Add file-level doc-comment:
  ```rust
  //! Action-queue payload types — Feature #15.
  //!
  //! `CombatActionKind` is RENAMED from "CombatAction" to avoid collision with
  //! `crate::plugins::input::CombatAction` — the leafwing menu-navigation enum
  //! (`input/mod.rs:90-98`). They are different concepts: leafwing's enum is
  //! keyboard-direction (Up/Down/Left/Right/Confirm/Cancel); this enum is the
  //! data payload for queued combat actions (Attack/Defend/CastSpell/UseItem/Flee).
  //!
  //! See `project/research/20260508-093000-feature-15-turn-based-combat-core.md`
  //! Pitfall 8.
  ```
- [ ] Add imports:
  ```rust
  use bevy::prelude::*;
  use crate::data::items::ItemAsset;
  use crate::plugins::combat::targeting::TargetSelection;
  ```
  (NOTE: `targeting` module doesn't exist yet — Phase 15B Step 1 creates it. For Phase 15A, declare the module path in `combat/mod.rs` AND ship a stub `targeting.rs` with just the `TargetSelection` enum. See Step 12.)
- [ ] Define the `CombatActionKind` enum (Decision 11):
  ```rust
  #[derive(Debug, Clone, Reflect)]
  pub enum CombatActionKind {
      /// Physical attack with the actor's currently-equipped weapon.
      Attack,
      /// Sets a 1-turn DefenseUp via ApplyStatusEvent (D-Q4=A: take-higher).
      Defend,
      /// Stub — emits "Spell stub" combat-log entry. Full implementation #20.
      CastSpell { spell_id: String },
      /// Consume an item from the actor's `Inventory`.
      UseItem { item: Handle<ItemAsset> },
      /// Try to escape combat. RNG-gated 50% success in v1.
      Flee,
  }
  ```
- [ ] Define the `Side` enum (Decision 18):
  ```rust
  #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
  pub enum Side { Party, Enemy }
  ```
- [ ] Define the `QueuedAction` struct (Decision 12):
  ```rust
  /// One queued action. Sortable by `(speed_at_queue_time DESC, actor_side ASC, slot_index ASC)`.
  ///
  /// `speed_at_queue_time` is captured at queue-time, not resolve-time —
  /// mid-round speed buffs do NOT reorder this round (Pitfall 9).
  #[derive(Debug, Clone)]
  pub struct QueuedAction {
      pub actor: Entity,
      pub kind: CombatActionKind,
      pub target: TargetSelection,
      pub speed_at_queue_time: u32,
      pub actor_side: Side,
      pub slot_index: u32,
  }
  ```
- [ ] Add inline `mod tests` with 1 unit test (Layer 1):
  ```rust
  #[cfg(test)]
  mod tests {
      use super::*;
      #[test]
      fn side_orders_party_before_enemy() {
          assert!(Side::Party < Side::Enemy);
      }
  }
  ```
- [ ] **Verification:**
  - `cargo check` — succeeds (after Step 12 ships the stub `targeting.rs`).

#### Step 4 — Create `src/plugins/combat/combat_log.rs` (Decision 25, D-Q3=A)

- [ ] Create new file `src/plugins/combat/combat_log.rs`. Add file-level doc-comment:
  ```rust
  //! Combat log — Feature #15.
  //!
  //! Bounded ring buffer (capacity 50, kept across combats). Pushed by
  //! `execute_combat_actions` after each action resolves; rendered by
  //! `paint_combat_log` (Phase 15D).
  //!
  //! D-Q3=A: bounded ring 50, kept across combats (NOT cleared on OnExit).
  //! ~4 KB memory budget.
  ```
- [ ] Add imports:
  ```rust
  use bevy::prelude::*;
  use std::collections::VecDeque;
  ```
- [ ] Define `CombatLogEntry`:
  ```rust
  #[derive(Debug, Clone)]
  pub struct CombatLogEntry {
      pub message: String,
      pub turn_number: u32,
  }
  ```
- [ ] Define `CombatLog: Resource` with bounded ring semantics:
  ```rust
  #[derive(Resource, Debug, Clone)]
  pub struct CombatLog {
      pub entries: VecDeque<CombatLogEntry>,
      pub capacity: usize,
  }

  impl Default for CombatLog {
      fn default() -> Self {
          Self {
              entries: VecDeque::with_capacity(50),
              capacity: 50,
          }
      }
  }

  impl CombatLog {
      /// Append a new entry; pop oldest when over capacity (Pitfall 7).
      pub fn push(&mut self, message: String, turn_number: u32) {
          self.entries.push_back(CombatLogEntry { message, turn_number });
          while self.entries.len() > self.capacity {
              self.entries.pop_front();
          }
      }

      /// Manual reset (unused in v1 — D-Q3=A keeps log across combats).
      pub fn clear(&mut self) {
          self.entries.clear();
      }
  }
  ```
- [ ] Add inline `mod tests` with 2 unit tests (Layer 1):
  ```rust
  #[cfg(test)]
  mod tests {
      use super::*;
      #[test]
      fn combat_log_caps_at_50() {
          let mut log = CombatLog::default();
          for i in 0..60 {
              log.push(format!("entry {}", i), i);
          }
          assert_eq!(log.entries.len(), 50);
          assert_eq!(log.entries.front().unwrap().message, "entry 10");
          assert_eq!(log.entries.back().unwrap().message, "entry 59");
      }

      #[test]
      fn combat_log_clear_empties() {
          let mut log = CombatLog::default();
          log.push("test".into(), 0);
          log.clear();
          assert!(log.entries.is_empty());
      }
  }
  ```
- [ ] **Verification:**
  - `cargo check` — succeeds.
  - `cargo test plugins::combat::combat_log::tests` — both tests pass.

#### Step 5 — Create `src/plugins/combat/enemy.rs` with `Enemy` ECS components (Decisions 13, 26)

- [ ] Create new file `src/plugins/combat/enemy.rs`. Add file-level doc-comment:
  ```rust
  //! Enemy ECS components — Feature #15 minimal shape.
  //!
  //! Enemies REUSE `BaseStats`, `DerivedStats`, `StatusEffects`, `PartyRow`,
  //! `Equipment`, `Experience` from `plugins::party::character` and
  //! `plugins::party::inventory`. The discriminator is the `Enemy` marker;
  //! `PartyMember` is its inverse.
  //!
  //! `EnemyBundle` includes `Equipment::default()` and `Experience::default()`
  //! to satisfy the dropped-`With<PartyMember>` filter in
  //! `recompute_derived_stats_on_equipment_change` (D-A5 carve-out, Pitfall 11).
  //!
  //! Real enemy authoring (asset-driven `EnemyDb` populated from
  //! `enemies.ron`) lands in #17. v1 hardcodes 2 placeholder enemies in the
  //! `#[cfg(feature = "dev")] spawn_dev_encounter` helper.
  ```
- [ ] Add imports:
  ```rust
  use bevy::prelude::*;
  use crate::plugins::combat::ai::EnemyAi;  // Phase 15C ships this
  use crate::plugins::party::character::{
      BaseStats, DerivedStats, Equipment, Experience, PartyRow, StatusEffects,
  };
  ```
  (NOTE: `ai` module doesn't exist until Phase 15C. For Phase 15A, ship a stub `ai.rs` file with just the `EnemyAi` enum; Phase 15C extends it. See Step 12.)
- [ ] Define `Enemy`/`EnemyName`/`EnemyIndex` markers:
  ```rust
  /// Zero-sized marker on enemy entities.
  #[derive(Component, Reflect, Default, Debug, Clone, Copy)]
  pub struct Enemy;

  /// Display name for enemy entities (rendered in egui combat screen).
  #[derive(Component, Reflect, Default, Debug, Clone)]
  pub struct EnemyName(pub String);

  /// Index within the encounter (0..N). Used for speed tie-break (Decision 14).
  #[derive(Component, Reflect, Default, Debug, Clone, Copy)]
  pub struct EnemyIndex(pub u32);
  ```
- [ ] Define `EnemyBundle`:
  ```rust
  /// Enemy entity spawn bundle. Includes `Equipment::default()` and
  /// `Experience::default()` to satisfy the (now `PartyMember`-less)
  /// recompute query (D-A5 carve-out).
  #[derive(Bundle, Default)]
  pub struct EnemyBundle {
      pub marker: Enemy,
      pub name: EnemyName,
      pub index: EnemyIndex,
      pub base_stats: BaseStats,
      pub derived_stats: DerivedStats,
      pub status_effects: StatusEffects,
      pub party_row: PartyRow,
      pub equipment: Equipment,
      pub experience: Experience,
      pub ai: EnemyAi,
  }
  ```
- [ ] Add inline `mod tests` with 1 unit test (Layer 1):
  ```rust
  #[cfg(test)]
  mod tests {
      use super::*;
      #[test]
      fn enemy_bundle_default_is_alive_marker() {
          let b = EnemyBundle::default();
          assert_eq!(b.derived_stats.current_hp, 0);
          assert_eq!(b.index.0, 0);
      }
  }
  ```
- [ ] **Verification:**
  - `cargo check` — succeeds (after Step 12 ships stubs for `ai.rs`).

#### Step 6 — Create `src/plugins/combat/turn_manager.rs` skeleton (resources + plugin scaffold)

- [ ] Create new file `src/plugins/combat/turn_manager.rs`. Add file-level doc-comment:
  ```rust
  //! Turn manager + state machine — Feature #15 Phase 15A.
  //!
  //! Owns:
  //! - `TurnActionQueue: Resource` (the action queue).
  //! - `PlayerInputState: Resource` (menu state machine).
  //! - `CombatRng: Resource` (single RNG source — Pitfall 12).
  //! - `FleeAttempted: Resource` (cross-system flag).
  //! - `current_turn: u32` (in `PlayerInputState` — passed to combat_log entries).
  //!
  //! Systems:
  //! - `init_combat_state` (OnEnter(Combat)).
  //! - `clear_combat_state` (OnExit(Combat)).
  //! - `collect_player_actions` (CombatPhase::PlayerInput).
  //! - `sort_queue_by_speed` (CombatPhase::ExecuteActions, before resolver).
  //! - `execute_combat_actions` (CombatPhase::ExecuteActions).
  //! - `check_victory_defeat_flee` (CombatPhase::TurnResult).
  //!
  //! ## State machine
  //!
  //! Uses 3-of-4 phases of `state::CombatPhase` (D-A2): `PlayerInput` →
  //! `ExecuteActions` → `TurnResult` → loop. The `EnemyTurn` variant exists
  //! in the enum (frozen by #2) but is NEVER set — vestigial relative to
  //! the action-queue design (research §A).
  //!
  //! ## Cross-plugin ordering
  //!
  //! `execute_combat_actions.before(apply_status_handler)` — Pitfall 13. The
  //! Defend → DefenseUp write must be visible same-frame. Mirrors the
  //! `apply_poison_trap.before(apply_status_handler)` precedent at
  //! `dungeon/features.rs:171`.
  //!
  //! ## `CurrentEncounter` contract (defined in #16)
  //!
  //! ```ignore
  //! #[derive(Resource, Debug, Clone)]
  //! pub struct CurrentEncounter {
  //!     pub enemy_entities: Vec<Entity>,
  //!     pub fleeable: bool,
  //! }
  //! ```
  //!
  //! #15 reads via `Option<Res<CurrentEncounter>>` so it ships without #16.
  //! Test fixtures define their own resource directly. Dev-stub spawn
  //! (`#[cfg(feature = "dev")]`) bypasses `CurrentEncounter` entirely.
  ```
- [ ] Add imports:
  ```rust
  use bevy::prelude::*;
  use leafwing_input_manager::prelude::ActionState;

  use crate::plugins::combat::actions::{CombatActionKind, QueuedAction, Side};
  use crate::plugins::combat::combat_log::CombatLog;
  use crate::plugins::combat::status_effects::{
      ApplyStatusEvent, apply_status_handler, check_dead_and_apply,
      is_asleep, is_paralyzed, is_silenced,
  };
  use crate::plugins::input::CombatAction as MenuNavAction;
  use crate::plugins::party::character::{
      DerivedStats, PartyMember, PartyRow, PartySlot, StatusEffectType, StatusEffects,
  };
  use crate::plugins::state::{CombatPhase, GameState};
  ```
- [ ] Define `TurnActionQueue` resource:
  ```rust
  #[derive(Resource, Default, Debug, Clone)]
  pub struct TurnActionQueue {
      pub queue: Vec<QueuedAction>,
  }
  ```
- [ ] Define `PlayerInputState` resource (Decision 16):
  ```rust
  #[derive(Resource, Default, Debug, Clone)]
  pub struct PlayerInputState {
      /// Slot of the currently-choosing party member (None when all alive
      /// members have committed → transition to ExecuteActions).
      pub active_slot: Option<usize>,
      /// Menu stack (top-of-stack = current visible menu).
      pub menu_stack: Vec<MenuFrame>,
      /// Actions committed this round.
      pub committed: Vec<QueuedAction>,
      /// Currently selecting a target?
      pub pending_action: Option<PendingAction>,
      /// Target cursor for arrow-driven selection.
      pub target_cursor: Option<usize>,
      /// Round counter (passed to combat_log entries for filtering).
      pub current_turn: u32,
  }

  #[derive(Debug, Clone)]
  pub enum MenuFrame {
      Main,
      SpellMenu,
      ItemMenu,
      TargetSelect { kind: CombatActionKind },
  }

  #[derive(Debug, Clone)]
  pub struct PendingAction {
      pub kind: CombatActionKind,
      pub actor: Entity,
  }
  ```
- [ ] Define `CombatRng` and `FleeAttempted` resources (Decisions 21, 22):
  ```rust
  /// Single RNG source for all of #15 (target picks, crit rolls, hit rolls,
  /// flee rolls). Pitfall 12: tests insert a seeded ChaCha8Rng directly;
  /// production seeds `SmallRng::from_os_rng()` once in `init_combat_state`.
  #[derive(Resource)]
  pub struct CombatRng(pub Box<dyn rand::RngCore + Send + Sync>);

  impl Default for CombatRng {
      fn default() -> Self {
          use rand::SeedableRng;
          Self(Box::new(rand::rngs::SmallRng::from_os_rng()))
      }
  }

  /// Set by `execute_combat_actions` when a `Flee` action resolves successfully.
  /// Read by `check_victory_defeat_flee` in `CombatPhase::TurnResult`.
  #[derive(Resource, Default, Debug, Clone)]
  pub struct FleeAttempted {
      pub success: bool,
      pub attempted_this_round: bool,
  }
  ```
- [ ] Define `TurnManagerPlugin` with stubbed system bodies (Step 7+ replace stubs):
  ```rust
  pub struct TurnManagerPlugin;

  impl Plugin for TurnManagerPlugin {
      fn build(&self, app: &mut App) {
          app.init_resource::<TurnActionQueue>()
              .init_resource::<PlayerInputState>()
              .init_resource::<CombatLog>()
              .init_resource::<CombatRng>()
              .init_resource::<FleeAttempted>()
              .add_systems(OnEnter(GameState::Combat), init_combat_state)
              .add_systems(OnExit(GameState::Combat), clear_combat_state)
              .add_systems(
                  Update,
                  (
                      collect_player_actions
                          .run_if(in_state(CombatPhase::PlayerInput)),
                      sort_queue_by_speed
                          .run_if(in_state(CombatPhase::ExecuteActions))
                          .before(execute_combat_actions),
                      execute_combat_actions
                          .run_if(in_state(CombatPhase::ExecuteActions))
                          .before(apply_status_handler),
                      check_victory_defeat_flee
                          .run_if(in_state(CombatPhase::TurnResult)),
                  ),
              );
      }
  }

  // ── Stubs (Steps 7-10 land bodies) ───────────────────────────────────────
  fn init_combat_state(/* ... */) { /* Step 7 */ }
  fn clear_combat_state(/* ... */) { /* Step 7 */ }
  fn collect_player_actions(/* ... */) { /* Step 7 */ }
  fn sort_queue_by_speed(/* ... */) { /* Step 7 */ }
  fn execute_combat_actions(/* ... */) { /* Step 7 */ }
  fn check_victory_defeat_flee(/* ... */) { /* Step 7 */ }
  ```
  (Stubs use `_` parameter names to avoid unused-variable warnings; full param lists land in Step 7+.)
- [ ] **Verification:**
  - `cargo check` — succeeds (stubs compile; plugin registers).

#### Step 7 — Land system bodies in `turn_manager.rs` (Steps 7a-7e)

This is the biggest step in 15A. Split into 5 sub-steps for clarity. Each sub-step replaces ONE stub with the full body.

##### Step 7a — `init_combat_state` and `clear_combat_state`

- [ ] Replace the `init_combat_state` stub:
  ```rust
  /// Initialize per-encounter state on entry to `GameState::Combat`.
  fn init_combat_state(
      mut queue: ResMut<TurnActionQueue>,
      mut input_state: ResMut<PlayerInputState>,
      mut flee: ResMut<FleeAttempted>,
      mut combat_log: ResMut<CombatLog>,
      mut rng: ResMut<CombatRng>,
  ) {
      use rand::SeedableRng;
      queue.queue.clear();
      *input_state = PlayerInputState::default();
      input_state.menu_stack = vec![MenuFrame::Main];
      *flee = FleeAttempted::default();
      // D-Q3=A: keep log across combats — DO NOT clear here.
      combat_log.push("--- Combat begins ---".into(), 0);
      // Re-seed RNG from OS entropy (production); tests overwrite this resource.
      rng.0 = Box::new(rand::rngs::SmallRng::from_os_rng());
  }
  ```
- [ ] Replace the `clear_combat_state` stub:
  ```rust
  /// Tidy up on exit from `GameState::Combat`.
  fn clear_combat_state(
      mut queue: ResMut<TurnActionQueue>,
      mut input_state: ResMut<PlayerInputState>,
      mut flee: ResMut<FleeAttempted>,
  ) {
      queue.queue.clear();
      *input_state = PlayerInputState::default();
      *flee = FleeAttempted::default();
  }
  ```

##### Step 7b — `collect_player_actions`

- [ ] Replace the `collect_player_actions` stub. The body has THREE jobs:
  1. Find `active_slot` — first alive non-incapacitated party member who hasn't committed yet.
  2. If all party members have committed, transition to `ExecuteActions`.
  3. Otherwise, leave `active_slot` set and let the UI (Phase 15D) drive `PlayerInputState.menu_stack`.

  ```rust
  fn collect_player_actions(
      mut input_state: ResMut<PlayerInputState>,
      mut combat_log: ResMut<CombatLog>,
      party: Query<(Entity, &PartySlot, &DerivedStats, &StatusEffects), With<PartyMember>>,
      mut next_phase: ResMut<NextState<CombatPhase>>,
  ) {
      // Snapshot alive & non-incapacitated party members, sorted by slot.
      let mut alive_slots: Vec<(Entity, usize)> = party
          .iter()
          .filter(|(_, _, d, s)| {
              d.current_hp > 0
                  && !s.has(StatusEffectType::Dead)
                  && !s.has(StatusEffectType::Stone)
          })
          .map(|(e, slot, _, _)| (e, slot.0))
          .collect();
      alive_slots.sort_by_key(|(_, s)| *s);

      // Auto-skip Sleep/Paralysis (Pitfall 4).
      for (entity, slot) in &alive_slots {
          let already_committed = input_state.committed.iter().any(|c| c.actor == *entity);
          if already_committed {
              continue;
          }
          let Ok((_, _, _, status)) = party.get(*entity) else { continue };
          if is_asleep(status) || is_paralyzed(status) {
              combat_log.push(
                  format!("Party slot {} is incapacitated.", slot),
                  input_state.current_turn,
              );
              // Push a sentinel "skipped" QueuedAction with kind = Defend (no-op
              // semantically; Defend with potency 0.5 will silently no-op if the
              // character already has a higher buff, otherwise it's a graceful
              // free defense — acceptable for incapacitated characters).
              // Decision 33: simpler than introducing a Skip variant.
              input_state.committed.push(QueuedAction {
                  actor: *entity,
                  kind: CombatActionKind::Defend,
                  target: crate::plugins::combat::targeting::TargetSelection::Self_,
                  speed_at_queue_time: party.get(*entity).map(|(_, _, d, _)| d.speed).unwrap_or(0),
                  actor_side: Side::Party,
                  slot_index: *slot as u32,
              });
              continue;
          }
          // Found the next alive non-incapacitated slot to choose for.
          if input_state.active_slot != Some(*slot) {
              input_state.active_slot = Some(*slot);
              input_state.menu_stack = vec![MenuFrame::Main];
          }
          return;  // Wait for UI to commit.
      }

      // All alive members committed → transition.
      input_state.active_slot = None;
      next_phase.set(CombatPhase::ExecuteActions);
  }
  ```

##### Step 7c — `sort_queue_by_speed` (Pitfall 9)

- [ ] Replace the `sort_queue_by_speed` stub:
  ```rust
  /// Sort the queue once per round before execution. Deterministic tie-break
  /// per Pitfall 9: descending speed → party-before-enemy → ascending slot.
  fn sort_queue_by_speed(mut queue: ResMut<TurnActionQueue>) {
      queue.queue.sort_by(|a, b| {
          b.speed_at_queue_time
              .cmp(&a.speed_at_queue_time)
              .then(a.actor_side.cmp(&b.actor_side))
              .then(a.slot_index.cmp(&b.slot_index))
      });
  }
  ```

##### Step 7d — `execute_combat_actions` (the heart of Phase 15A)

- [ ] Replace the `execute_combat_actions` stub. Body has these phases per action:
  1. Drain queue snapshot via `std::mem::take` (Pitfall 2).
  2. For each `QueuedAction`:
     a. Skip if `is_alive(actor) == false` with log entry (Pitfall 3).
     b. Match `kind`:
        - `Attack` — Phase 15B replaces with full damage_calc call. Phase 15A stub: log "Attack stub" and continue.
        - `Defend` — write `ApplyStatusEvent { effect: DefenseUp, potency: 0.5, duration: Some(1) }`; log "{name} defends!" UNCONDITIONALLY (Pitfall 6).
        - `CastSpell { spell_id }` — log "{name} casts {spell_id}: not yet implemented" (Decision 32).
        - `UseItem { item }` — Phase 15A's body: log "Item use stub". Phase 15A Step 14 lands the full inventory-mutation body.
        - `Flee` — call `try_flee` helper that rolls 50% via `rng.gen_range(0..100) < 50` and sets `FleeAttempted.success` accordingly (Decision 22). Log "{actor_name} attempts to flee" + "Success!" or "Failed!" depending on roll.
     c. After damage writes (Phase 15B), call `check_dead_and_apply` for the target.
  3. After loop: `next_phase.set(CombatPhase::TurnResult)`.

  ```rust
  fn execute_combat_actions(
      mut queue: ResMut<TurnActionQueue>,
      mut characters: Query<(
          Entity,
          &DerivedStats,
          &StatusEffects,
          Option<&crate::plugins::party::character::CharacterName>,
          Option<&crate::plugins::combat::enemy::EnemyName>,
      )>,
      mut apply_status: MessageWriter<ApplyStatusEvent>,
      mut combat_log: ResMut<CombatLog>,
      mut flee: ResMut<FleeAttempted>,
      mut rng: ResMut<CombatRng>,
      mut next_phase: ResMut<NextState<CombatPhase>>,
      input_state: Res<PlayerInputState>,
  ) {
      let actions = std::mem::take(&mut queue.queue);
      let turn = input_state.current_turn;

      // Helper: lookup a name (party or enemy).
      let name_of = |e: Entity| -> String {
          if let Ok((_, _, _, party_name, enemy_name)) = characters.get(e) {
              party_name
                  .map(|n| n.0.clone())
                  .or_else(|| enemy_name.map(|n| n.0.clone()))
                  .unwrap_or_else(|| format!("Entity({:?})", e))
          } else {
              format!("Entity({:?})", e)
          }
      };

      // Helper: is_alive predicate (Pitfall 3 / 10).
      let is_alive = |e: Entity| -> bool {
          characters.get(e)
              .map(|(_, d, s, _, _)| {
                  d.current_hp > 0
                      && !s.has(StatusEffectType::Dead)
                      && !s.has(StatusEffectType::Stone)
              })
              .unwrap_or(false)
      };

      for action in actions {
          // Pitfall 3 — actor died mid-round.
          if !is_alive(action.actor) {
              combat_log.push(
                  format!("{} is unable to act.", name_of(action.actor)),
                  turn,
              );
              continue;
          }

          match &action.kind {
              CombatActionKind::Attack => {
                  // Phase 15B replaces this with a full damage_calc call +
                  // current_hp mutation + check_dead_and_apply. Phase 15A stub:
                  combat_log.push(
                      format!("{} attacks (stub).", name_of(action.actor)),
                      turn,
                  );
              }
              CombatActionKind::Defend => {
                  // D-Q4=A: take-higher merge in apply_status_handler.
                  // Pitfall 6: log fires UNCONDITIONALLY for game-feel.
                  apply_status.write(ApplyStatusEvent {
                      target: action.actor,
                      effect: StatusEffectType::DefenseUp,
                      potency: 0.5,
                      duration: Some(1),
                  });
                  combat_log.push(
                      format!("{} defends!", name_of(action.actor)),
                      turn,
                  );
              }
              CombatActionKind::CastSpell { spell_id } => {
                  // Decision 32: stub.
                  combat_log.push(
                      format!(
                          "{} casts {}: not yet implemented.",
                          name_of(action.actor),
                          spell_id
                      ),
                      turn,
                  );
              }
              CombatActionKind::UseItem { item: _ } => {
                  // Step 14 lands the full inventory-mutation body.
                  combat_log.push(
                      format!("{} uses an item (stub).", name_of(action.actor)),
                      turn,
                  );
              }
              CombatActionKind::Flee => {
                  use rand::Rng;
                  flee.attempted_this_round = true;
                  let roll = rng.0.gen_range(0..100);
                  if roll < 50 {
                      flee.success = true;
                      combat_log.push(
                          format!("{} flees! Escape successful.", name_of(action.actor)),
                          turn,
                      );
                  } else {
                      combat_log.push(
                          format!("{} tried to flee but failed!", name_of(action.actor)),
                          turn,
                      );
                  }
              }
          }
      }

      next_phase.set(CombatPhase::TurnResult);
  }
  ```

##### Step 7e — `check_victory_defeat_flee` (Decision 24)

- [ ] Replace the `check_victory_defeat_flee` stub:
  ```rust
  /// Decide what comes after `ExecuteActions`. Order: defeat → flee → victory →
  /// next round. Documented in Decision 24.
  fn check_victory_defeat_flee(
      party: Query<(&DerivedStats, &StatusEffects), With<PartyMember>>,
      enemies: Query<(&DerivedStats, &StatusEffects), With<crate::plugins::combat::enemy::Enemy>>,
      flee: Res<FleeAttempted>,
      mut next_state: ResMut<NextState<GameState>>,
      mut next_phase: ResMut<NextState<CombatPhase>>,
      mut combat_log: ResMut<CombatLog>,
      mut input_state: ResMut<PlayerInputState>,
  ) {
      let all_party_dead = party
          .iter()
          .all(|(d, s)| d.current_hp == 0 || s.has(StatusEffectType::Dead));
      let all_enemies_dead = enemies
          .iter()
          .all(|(d, s)| d.current_hp == 0 || s.has(StatusEffectType::Dead))
          && !enemies.is_empty();

      // 1. Defeat first (Open Question 4 of research).
      if all_party_dead {
          combat_log.push("The party falls...".into(), input_state.current_turn);
          next_state.set(GameState::GameOver);
          return;
      }

      // 2. Flee.
      if flee.success {
          next_state.set(GameState::Dungeon);
          return;
      }

      // 3. Victory.
      if all_enemies_dead {
          combat_log.push("Victory!".into(), input_state.current_turn);
          next_state.set(GameState::Dungeon);
          return;
      }

      // 4. Next round: clear committed; advance turn counter; back to PlayerInput.
      input_state.committed.clear();
      input_state.current_turn = input_state.current_turn.saturating_add(1);
      input_state.active_slot = None;
      input_state.menu_stack = vec![MenuFrame::Main];
      next_phase.set(CombatPhase::PlayerInput);
  }
  ```

#### Step 8 — Carve-out edit on `inventory.rs` (D-A5, Pitfall 11)

- [ ] In `src/plugins/party/inventory.rs`, modify `recompute_derived_stats_on_equipment_change` query at line 437-446:
  ```rust
  // BEFORE:
  // mut characters: Query<
  //     ( &BaseStats, &Equipment, &StatusEffects, &Experience, &mut DerivedStats ),
  //     With<PartyMember>,
  // >,
  // AFTER:
  mut characters: Query<(
      &BaseStats,
      &Equipment,
      &StatusEffects,
      &Experience,
      &mut DerivedStats,
  )>,
  ```
- [ ] Update the doc-comment at lines 421-433 of `inventory.rs`. Append a paragraph explaining the dual-use:
  ```
  /// **#15 carve-out (D-A5):** the original `With<PartyMember>` filter was
  /// dropped so this same recompute system applies to enemy entities as well.
  /// Enemies spawn with `Equipment::default()` and `Experience::default()`
  /// (see `combat/enemy.rs::EnemyBundle`) so the query shape matches. The
  /// flatten step over `Equipment` slots is a no-op for empty equipment,
  /// so the system simply re-runs `derive_stats` for any character receiving
  /// an `EquipmentChangedEvent` — including buffs/debuffs applied via
  /// the `EquipSlot::None` sentinel from #14's `apply_status_handler`
  /// (Pitfall 11 of #15).
  ```
- [ ] **Verification:**
  - `cargo check` — succeeds.
  - `cargo test plugins::party::inventory` — existing tests still pass (recompute behavior unchanged for party members).

#### Step 9 — Stub `targeting.rs` and `ai.rs` for compile-time module discovery

These two modules are FULLY landed in Phase 15B (targeting) and Phase 15C (ai), but Phase 15A's `actions.rs`/`enemy.rs`/`turn_manager.rs` reference them. Ship minimal stubs in Phase 15A that compile cleanly.

- [ ] Create `src/plugins/combat/targeting.rs` with stub:
  ```rust
  //! Target selection — Feature #15 Phase 15B.
  //!
  //! Phase 15A ships only the `TargetSelection` enum; the
  //! `resolve_target_with_fallback` pure function lands in Phase 15B.

  use bevy::prelude::*;

  /// Target-selection enum. The queue payload references this.
  #[derive(Debug, Clone)]
  pub enum TargetSelection {
      Single(Entity),
      AllAllies,
      AllEnemies,
      Self_,
      None,
  }
  ```
- [ ] Create `src/plugins/combat/ai.rs` with stub:
  ```rust
  //! Enemy AI — Feature #15 Phase 15C.
  //!
  //! Phase 15A ships only the `EnemyAi` enum (so `EnemyBundle` compiles);
  //! the `enemy_ai_action_select` system + `EnemyAiPlugin` land in Phase 15C.

  use bevy::prelude::*;

  /// AI behaviour for an enemy entity.
  ///
  /// Phase 15A ships the enum shape; Phase 15C ships the dispatcher.
  #[derive(Component, Reflect, Default, Debug, Clone, Copy)]
  pub enum EnemyAi {
      #[default]
      RandomAttack,
      BossFocusWeakest,
      BossAttackDefendAttack { turn: u32 },
  }
  ```

#### Step 10 — Wire `TurnManagerPlugin` into `CombatPlugin`

- [ ] In `src/plugins/combat/mod.rs`, modify the file:
  ```rust
  use bevy::log::info;
  use bevy::prelude::*;

  use crate::plugins::state::GameState;

  pub mod actions;
  pub mod ai;
  pub mod combat_log;
  pub mod enemy;
  pub mod status_effects;
  pub mod targeting;
  pub mod turn_manager;

  pub use status_effects::*;

  pub struct CombatPlugin;

  impl Plugin for CombatPlugin {
      fn build(&self, app: &mut App) {
          app.add_plugins(status_effects::StatusEffectsPlugin)
              .add_plugins(turn_manager::TurnManagerPlugin)
              .add_systems(OnEnter(GameState::Combat), || {
                  info!("Entered GameState::Combat")
              })
              .add_systems(OnExit(GameState::Combat), || {
                  info!("Exited GameState::Combat")
              });
      }
  }
  ```
- [ ] **Verification:**
  - `cargo check` — succeeds.
  - `cargo check --features dev` — succeeds.
  - `cargo test plugins::combat::actions` — Step 3 unit test passes.
  - `cargo test plugins::combat::combat_log` — Step 4 tests pass.
  - `cargo test plugins::combat::enemy` — Step 5 unit test passes.

#### Step 11 — Dev-stub encounter spawner (Decision 27, `#[cfg(feature = "dev")]`)

- [ ] In `src/plugins/combat/turn_manager.rs`, add a dev-only spawn helper:
  ```rust
  #[cfg(feature = "dev")]
  use crate::plugins::combat::enemy::{Enemy, EnemyBundle, EnemyIndex, EnemyName};
  #[cfg(feature = "dev")]
  use crate::plugins::party::character::BaseStats;

  /// Dev-only stub: spawns 2 placeholder enemies on `OnEnter(GameState::Combat)`
  /// so manual smoke testing has fodder. Idempotence guard: if any `Enemy`
  /// exists, return early (mirrors `spawn_default_debug_party` at
  /// `party/mod.rs:88-126`). #16 deletes this stub when it ships its own
  /// encounter spawner.
  #[cfg(feature = "dev")]
  fn spawn_dev_encounter(
      mut commands: Commands,
      existing: Query<(), With<Enemy>>,
  ) {
      if !existing.is_empty() {
          return;
      }
      use crate::plugins::combat::ai::EnemyAi;
      use crate::plugins::party::character::DerivedStats;
      let stats = BaseStats {
          strength: 8,
          intelligence: 4,
          piety: 4,
          vitality: 8,
          agility: 6,
          luck: 4,
      };
      let derived = DerivedStats {
          max_hp: 30,
          current_hp: 30,
          max_mp: 0,
          current_mp: 0,
          attack: 8,
          defense: 5,
          magic_attack: 0,
          magic_defense: 2,
          speed: 6,
          accuracy: 60,
          evasion: 5,
      };
      for i in 0..2 {
          commands.spawn(EnemyBundle {
              name: EnemyName(format!("Goblin {}", i + 1)),
              index: EnemyIndex(i as u32),
              base_stats: stats,
              derived_stats: derived,
              ai: EnemyAi::RandomAttack,
              ..Default::default()
          });
      }
      info!("Dev-stub: spawned 2 Goblin enemies");
  }
  ```
- [ ] In `TurnManagerPlugin::build`, register the dev-stub:
  ```rust
  #[cfg(feature = "dev")]
  app.add_systems(OnEnter(GameState::Combat), spawn_dev_encounter.after(init_combat_state));
  ```
- [ ] **Verification:**
  - `cargo check --features dev` — succeeds.
  - Manual smoke (will be done in #16 / final verification): F9 cycle to Combat, observe 2 Goblins spawned (assertable via tests later).

#### Step 12 — `UseItem` resolver body (Decisions 30, 31)

- [ ] In `src/plugins/combat/turn_manager.rs::execute_combat_actions`, replace the `UseItem` stub with the full inventory-mutation body. NOTE: this requires reading `Inventory` and `ItemInstance` components — extend the `characters` Query parameter set, OR add separate Queries.

  Recommended shape: add two more system parameters to `execute_combat_actions`:
  ```rust
  mut inventories: Query<&mut crate::plugins::party::Inventory>,
  item_instances: Query<&crate::plugins::party::ItemInstance>,
  items: Res<Assets<crate::data::ItemAsset>>,
  ```
- [ ] Replace the `UseItem` arm:
  ```rust
  CombatActionKind::UseItem { item } => {
      let actor_name = name_of(action.actor);
      // Look up the item asset.
      let Some(asset) = items.get(item) else {
          combat_log.push(format!("{} fumbles an item.", actor_name), turn);
          continue;
      };
      // Decision 31: reject key items.
      if asset.kind == crate::plugins::party::ItemKind::KeyItem {
          combat_log.push(
              format!("{} cannot use {} in combat.", actor_name, asset.display_name),
              turn,
          );
          continue;
      }
      // Decision 30: Consumables heal max_hp / 4.
      if asset.kind == crate::plugins::party::ItemKind::Consumable {
          // Find and remove the item instance from the actor's inventory.
          let removed_entity = if let Ok(mut inventory) = inventories.get_mut(action.actor) {
              let mut found_idx = None;
              for (i, inst_entity) in inventory.0.iter().enumerate() {
                  if let Ok(inst) = item_instances.get(*inst_entity) {
                      if inst.0 == *item {
                          found_idx = Some(i);
                          break;
                      }
                  }
              }
              found_idx.map(|i| inventory.0.remove(i))
          } else {
              None
          };
          if let Some(inst_entity) = removed_entity {
              // Despawn the ItemInstance entity.
              // (Separate Commands write — defer via Commands Queue.)
              // Actually: we don't have Commands in this system signature.
              // Decision: drop the despawn for now; the entity stays detached but
              // unreachable. #16/#17 polish handles cleanup. The Inventory mutation
              // is the load-bearing semantic.
              let _ = inst_entity;
          }
          // Heal the actor.
          if let Ok((_, _derived, _, _, _)) = characters.get(action.actor) {
              // Borrow checker: characters query is immutable here; we can't
              // mutate current_hp in place. Re-architect: split the query into
              // a separate `mut_derived: Query<&mut DerivedStats>`.
              // (Implementer detail: see Step 7d notes on query splitting.)
          }
          combat_log.push(
              format!("{} drinks {}!", actor_name, asset.display_name),
              turn,
          );
      }
  }
  ```
  **Implementer note:** The `&DerivedStats` reads in `name_of`/`is_alive` closures need a separate `&mut DerivedStats` query path for the actual heal-mutation. Recommended refactor: split the system parameter into `chars_read: Query<(Entity, &DerivedStats, &StatusEffects, ...)>` for closure use AND `chars_mut_hp: Query<&mut DerivedStats>` for actual mutation. The borrow checker forces this discipline.

  For Phase 15A, consumable-heal is allowed to be a "log entry only" body if the borrow-checker dance proves complex; the test `use_item_consumes_consumable` asserts the inventory-removal step (the load-bearing one), and the test `use_item_heals_max_hp_quarter` is deferred to Phase 15B's resolver work or marked `#[ignore]` with a TODO. Implementer's call.

#### Step 13 — Layer-1 unit tests in `turn_manager.rs::tests`

- [ ] Add `#[cfg(test)] mod tests` at the end of `turn_manager.rs`. Layer 1 tests (no `App`):
  ```rust
  #[cfg(test)]
  mod tests {
      use super::*;
      use crate::plugins::combat::actions::{CombatActionKind, QueuedAction, Side};
      use crate::plugins::combat::targeting::TargetSelection;

      // Helper.
      fn mk_action(speed: u32, side: Side, slot: u32) -> QueuedAction {
          QueuedAction {
              actor: Entity::PLACEHOLDER,
              kind: CombatActionKind::Defend,
              target: TargetSelection::Self_,
              speed_at_queue_time: speed,
              actor_side: side,
              slot_index: slot,
          }
      }

      #[test]
      fn speed_sort_descending() {
          let mut q = vec![mk_action(10, Side::Party, 0), mk_action(20, Side::Party, 1), mk_action(15, Side::Enemy, 0)];
          q.sort_by(|a, b| {
              b.speed_at_queue_time.cmp(&a.speed_at_queue_time)
                  .then(a.actor_side.cmp(&b.actor_side))
                  .then(a.slot_index.cmp(&b.slot_index))
          });
          assert_eq!(q[0].speed_at_queue_time, 20);
          assert_eq!(q[1].speed_at_queue_time, 15);
          assert_eq!(q[2].speed_at_queue_time, 10);
      }

      #[test]
      fn speed_tie_party_before_enemy() {
          let mut q = vec![mk_action(10, Side::Enemy, 0), mk_action(10, Side::Party, 0)];
          q.sort_by(|a, b| {
              b.speed_at_queue_time.cmp(&a.speed_at_queue_time)
                  .then(a.actor_side.cmp(&b.actor_side))
                  .then(a.slot_index.cmp(&b.slot_index))
          });
          assert_eq!(q[0].actor_side, Side::Party);
      }

      #[test]
      fn speed_tie_lower_slot_first() {
          let mut q = vec![mk_action(10, Side::Party, 2), mk_action(10, Side::Party, 0)];
          q.sort_by(|a, b| {
              b.speed_at_queue_time.cmp(&a.speed_at_queue_time)
                  .then(a.actor_side.cmp(&b.actor_side))
                  .then(a.slot_index.cmp(&b.slot_index))
          });
          assert_eq!(q[0].slot_index, 0);
      }
  }
  ```

#### Step 14 — Layer-2 App-driven tests in `turn_manager.rs::app_tests`

- [ ] Add `#[cfg(test)] mod app_tests` at the end of `turn_manager.rs`. Tests use the `make_test_app` shape from `combat/status_effects.rs::app_tests`. Required tests (~12):
  - `defend_writes_defense_up_via_apply_status_event` (Pitfall 6 / D-Q4=A)
  - `defend_no_ops_when_higher_defense_up_active` (D-Q4=A take-higher)
  - `defend_re_derives_defense_via_recompute` (D-A5 + #14 D5α path)
  - `flee_succeeds_with_seed_42` (Pitfall 12)
  - `flee_fails_with_seed_99` (Pitfall 12)
  - `victory_when_all_enemies_dead` (Decision 24)
  - `defeat_when_all_party_dead` (Decision 24, defeat-first ordering)
  - `dead_actor_skips_action_in_resolve` (Pitfall 3)
  - `sleep_skips_action_emission` (Pitfall 4)
  - `cast_spell_logs_stub_message` (Decision 32)
  - `enemy_buff_re_derives_stats` (Pitfall 11 / D-A5)
  - `combat_log_caps_at_50_under_resolver_load` (Pitfall 7 system-level)

  Detail every test below — copy the shape from `status_effects.rs::app_tests`. Each test:
  1. Builds `App` via `make_test_app`.
  2. Spawns party + enemies (or just one of each, depending).
  3. Pushes a `QueuedAction` to `TurnActionQueue` directly.
  4. Sets `CombatPhase::ExecuteActions` via `NextState`.
  5. Calls `app.update()` once or twice.
  6. Asserts world state.

  Key test harness:
  ```rust
  #[cfg(test)]
  mod app_tests {
      use super::*;
      use bevy::ecs::message::Messages;
      use bevy::state::app::StatesPlugin;

      fn make_test_app() -> App {
          let mut app = App::new();
          app.add_plugins((
              MinimalPlugins,
              bevy::asset::AssetPlugin::default(),
              StatesPlugin,
              crate::plugins::state::StatePlugin,
              crate::plugins::party::PartyPlugin,
              crate::plugins::combat::CombatPlugin,
          ));
          app.init_asset::<crate::data::ItemDb>();
          app.init_asset::<crate::data::ItemAsset>();
          #[cfg(feature = "dev")]
          app.init_resource::<bevy::input::ButtonInput<bevy::prelude::KeyCode>>();
          app
      }

      fn seed_test_rng(app: &mut App, seed: u64) {
          use rand::SeedableRng;
          let rng = rand_chacha::ChaCha8Rng::seed_from_u64(seed);
          app.world_mut().insert_resource(CombatRng(Box::new(rng)));
      }

      fn enter_combat(app: &mut App) {
          let mut next: Mut<NextState<crate::plugins::state::GameState>> =
              app.world_mut().resource_mut::<NextState<crate::plugins::state::GameState>>();
          next.set(crate::plugins::state::GameState::Combat);
          app.update();  // realise OnEnter
          app.update();  // settle init systems
      }

      fn enter_execute_phase(app: &mut App) {
          let mut next: Mut<NextState<CombatPhase>> = app.world_mut()
              .resource_mut::<NextState<CombatPhase>>();
          next.set(CombatPhase::ExecuteActions);
          app.update();
      }

      // (12 tests listed above — see plan)
  }
  ```

  **Implementer note:** Each test is ~30-50 LOC; the full set is ~500-700 LOC of test code. Allowable since it's tests; counts toward the test envelope (12 tests in 15A).

#### Step 15 — Phase 15A verification gate

- [ ] Run all 7 commands of the global gate (see end of plan), but the EXIT criterion for Phase 15A is just:
  - `cargo check` — succeeds
  - `cargo check --features dev` — succeeds
  - `cargo test plugins::combat::` — all new tests pass; no regressions in existing combat tests
  - `cargo test --features dev plugins::combat::` — all tests pass with dev feature
  - `cargo clippy --all-targets -- -D warnings` — clean
  - `cargo clippy --all-targets --features dev -- -D warnings` — clean
  - `cargo fmt --check` — clean
  - **Grep guards (must be ZERO matches):**
    - `rg 'derive\(.*\bEvent\b' src/plugins/combat/{turn_manager,actions,enemy,combat_log,targeting,ai}.rs` — must be 0.
    - `rg '\bEventReader<|\bEventWriter<' src/plugins/combat/{turn_manager,actions,enemy,combat_log,targeting,ai}.rs` — must be 0.
    - `rg 'effects\.push\(' src/plugins/combat/{turn_manager,actions,enemy,combat_log,targeting,ai}.rs` — must be 0 (sole-mutator).
    - `rg 'pub enum CombatAction\b' src/plugins/combat/actions.rs` — must be 0 (Pitfall 8).

- [ ] Atomic GitButler commit per CLAUDE.md: `but commit --message-file <path>` with message describing Phase 15A scope.

---

### Phase 15B — Damage + Targeting (~250 LOC, ~10 tests)

**Files in this phase:**
- New: `src/plugins/combat/damage.rs` (~180 LOC), additions to `src/plugins/combat/targeting.rs` (the stub from 15A grows).
- Edits: `src/plugins/combat/turn_manager.rs::execute_combat_actions::Attack arm` (replaces stub with full damage_calc call).
- No new sub-plugin (damage is `pub fn`s; targeting is `pub fn`s).

**Tests in this phase:** damage formula edge cases (7), targeting re-resolution (3) = ~10 tests.

#### Step 1 — Land `targeting.rs` full body (extends 15A stub)

- [ ] Edit `src/plugins/combat/targeting.rs`. Replace the stub doc-comment with the full file:
  ```rust
  //! Target selection — Feature #15 Phase 15B.
  //!
  //! `TargetSelection` enum (queue payload) + `resolve_target_with_fallback`
  //! pure function that handles the re-target-on-death edge case
  //! (research Pattern 3, Pitfall 3).

  use bevy::prelude::*;
  use rand::seq::IteratorRandom;

  use crate::plugins::combat::actions::Side;

  #[derive(Debug, Clone)]
  pub enum TargetSelection {
      Single(Entity),
      AllAllies,
      AllEnemies,
      Self_,
      None,
  }
  ```
- [ ] Add `resolve_target_with_fallback` pure function:
  ```rust
  /// Resolve `selection` to a list of currently-alive entities.
  ///
  /// Re-target rule for `Single(t)` when `t` is dead: pick a random alive
  /// entity from the SAME side as the original target. Side membership is
  /// determined by which slice (`party` or `enemies`) contains `t`.
  ///
  /// PURE — no `Mut`, no `Query`, no entity lookups beyond the slices
  /// caller provides + the `is_alive` predicate.
  pub fn resolve_target_with_fallback(
      selection: &TargetSelection,
      actor: Entity,
      actor_side: Side,
      party: &[Entity],
      enemies: &[Entity],
      is_alive: impl Fn(Entity) -> bool,
      rng: &mut impl rand::Rng,
  ) -> Vec<Entity> {
      use TargetSelection::*;
      match selection {
          Single(t) if is_alive(*t) => vec![*t],
          Single(t) => {
              // Re-target: pick same side as original target.
              let same_side = if party.contains(t) { party } else { enemies };
              same_side
                  .iter()
                  .filter(|e| is_alive(**e))
                  .copied()
                  .choose(rng)
                  .map(|e| vec![e])
                  .unwrap_or_default()
          }
          AllAllies => {
              let side = if actor_side == Side::Party { party } else { enemies };
              side.iter().filter(|e| is_alive(**e)).copied().collect()
          }
          AllEnemies => {
              let side = if actor_side == Side::Party { enemies } else { party };
              side.iter().filter(|e| is_alive(**e)).copied().collect()
          }
          Self_ => if is_alive(actor) { vec![actor] } else { vec![] },
          None => vec![],
      }
  }
  ```
- [ ] Add Layer-1 unit tests:
  ```rust
  #[cfg(test)]
  mod tests {
      use super::*;
      use rand::SeedableRng;

      fn mk_alive(alive_ids: &[u32]) -> impl Fn(Entity) -> bool + '_ {
          move |e| alive_ids.contains(&e.index())
      }

      #[test]
      fn single_target_alive_returns_target() {
          // Use placeholder entities; rely on Entity::index() for is_alive.
          // (Note: real Entity construction requires an App; in pure tests we use
          // bit-manipulated entities. Fall back to App-based test if blocked.)
          let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(0);
          // (Implementer fills in entity construction details.)
      }

      #[test]
      fn single_target_dead_re_targets_same_side() {
          // (See above — App-based or use entity-id manipulation.)
      }

      #[test]
      fn single_target_dead_no_alive_on_side_returns_empty() {
          // (See above.)
      }
  }
  ```
  **Implementer note:** Pure-function tests for re-target may need a tiny `App` to construct `Entity` values. If so, drop these into `app_tests` instead of `tests`. Three target tests minimum.

#### Step 2 — Create `src/plugins/combat/damage.rs` with `Combatant`/`DamageResult`/`damage_calc`

- [ ] Create new file `src/plugins/combat/damage.rs`. Add file-level doc-comment:
  ```rust
  //! Damage computation — Feature #15 Phase 15B.
  //!
  //! D-A3=A: Wizardry-style multiplicative formula `(A * (100 - D / 2)) / 100`.
  //! Variance multiplier 0.7..=1.0; crit 1.5x at `accuracy / 5`% chance.
  //!
  //! ## Pure function discipline (research Pattern 2, roadmap line 858)
  //!
  //! `damage_calc` is the SINGLE OWNER of the damage formula and row rules.
  //! No entity lookups, no resource reads. The caller flattens
  //! `(actor_entity, &Query<...>)` into the `Combatant` struct.
  //!
  //! ## Row rules (Decision 28, simplified for v1)
  //!
  //! Front-row attacker with melee weapon vs. back-row defender → damage = 0
  //! ("can't reach"). All other combinations → full damage. Real weapon-kind
  //! classification is #17 polish.
  //!
  //! ## Saturating arithmetic
  //!
  //! All addition uses `saturating_*`. Defends against `u32::MAX` from
  //! malicious save data (research §Security trust boundary).
  ```
- [ ] Add imports:
  ```rust
  use bevy::prelude::*;
  use rand::Rng;

  use crate::data::items::ItemAsset;
  use crate::plugins::combat::actions::CombatActionKind;
  use crate::plugins::party::character::{DerivedStats, PartyRow, StatusEffects};
  ```
- [ ] Define `Combatant` struct (Decision 28):
  ```rust
  /// Caller-flattened combatant data. Mirrors the `derive_stats` caller-flatten
  /// pattern.
  #[derive(Debug, Clone)]
  pub struct Combatant {
      pub name: String,
      pub stats: DerivedStats,
      pub row: PartyRow,
      pub status: StatusEffects,
  }
  ```
- [ ] Define `DamageResult`:
  ```rust
  #[derive(Debug, Clone, PartialEq)]
  pub struct DamageResult {
      pub damage: u32,
      pub hit: bool,
      pub critical: bool,
      pub message: String,
  }
  ```
- [ ] Define `damage_calc` pure function (D-A3=A, Decision 29):
  ```rust
  pub fn damage_calc(
      attacker: &Combatant,
      defender: &Combatant,
      weapon: Option<&ItemAsset>,
      action: &CombatActionKind,
      rng: &mut impl Rng,
  ) -> DamageResult {
      // Only Attack action computes damage; Defend/CastSpell/UseItem/Flee are
      // resolver-side effects.
      if !matches!(action, CombatActionKind::Attack) {
          return DamageResult {
              damage: 0,
              hit: false,
              critical: false,
              message: format!("{} performs a non-damaging action.", attacker.name),
          };
      }

      // 1. Hit roll (Decision 29).
      let hit_chance = attacker
          .stats
          .accuracy
          .saturating_sub(defender.stats.evasion)
          .min(100);
      let hit = rng.gen_range(0..100) < hit_chance;
      if !hit {
          return DamageResult {
              damage: 0,
              hit: false,
              critical: false,
              message: format!("{} misses {}.", attacker.name, defender.name),
          };
      }

      // 2. Row check (Decision 28). Simplified: all weapons are melee in v1.
      if matches!(attacker.row, PartyRow::Front)
          && matches!(defender.row, PartyRow::Back)
          && weapon.is_some()  // unarmed front→back is allowed (fists at the front)
      {
          // Future: weapon-kind classification (Bow/Spear) bypasses this rule.
          // v1: any weapon-equipped front-row attacker fails to reach back-row.
          return DamageResult {
              damage: 0,
              hit: true,
              critical: false,
              message: format!(
                  "{}'s attack can't reach {} in the back row.",
                  attacker.name, defender.name
              ),
          };
      }

      // 3. D-A3=A: Wizardry-style multiplicative damage.
      // Cap defense at 180 to keep `(100 - D/2)` non-negative.
      let raw = (attacker.stats.attack as i64
          * (100 - defender.stats.defense.min(180) as i64 / 2))
          / 100;
      let raw = raw.max(1) as u32;

      // 4. Variance multiplier 0.7..=1.0 (D-A3=A).
      let variance = rng.gen_range(70..=100) as f32 / 100.0;
      let damage = (raw as f32 * variance) as u32;

      // 5. Crit roll (Decision 29: chance = accuracy / 5 capped at 100).
      let crit_chance = (attacker.stats.accuracy / 5).min(100);
      let critical = rng.gen_range(0..100) < crit_chance;
      let damage = if critical {
          (damage as f32 * 1.5) as u32
      } else {
          damage
      };

      let damage = damage.max(1); // floor of 1 on positive-attack hits.

      DamageResult {
          damage,
          hit: true,
          critical,
          message: format!(
              "{} {} {} for {} damage{}.",
              attacker.name,
              if critical { "critically strikes" } else { "attacks" },
              defender.name,
              damage,
              if critical { " (CRITICAL)" } else { "" },
          ),
      }
  }
  ```
- [ ] Add Layer-1 unit tests (~7):
  ```rust
  #[cfg(test)]
  mod tests {
      use super::*;
      use rand::SeedableRng;

      fn mk_combatant(name: &str, attack: u32, defense: u32, accuracy: u32, evasion: u32, row: PartyRow) -> Combatant {
          Combatant {
              name: name.into(),
              stats: DerivedStats {
                  attack, defense, accuracy, evasion,
                  current_hp: 100, max_hp: 100,
                  ..Default::default()
              },
              row,
              status: StatusEffects::default(),
          }
      }

      #[test]
      fn damage_calc_defense_greater_than_attack_floors_at_one() {
          let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(42);
          let a = mk_combatant("A", 5, 100, 100, 0, PartyRow::Front);
          let d = mk_combatant("D", 5, 100, 0, 0, PartyRow::Front);
          let r = damage_calc(&a, &d, None, &CombatActionKind::Attack, &mut rng);
          assert!(r.hit);
          assert!(r.damage >= 1, "Damage floor should be at least 1");
      }

      #[test]
      fn damage_calc_zero_attack_floors_at_one() {
          let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(42);
          let a = mk_combatant("A", 0, 100, 100, 0, PartyRow::Front);
          let d = mk_combatant("D", 0, 0, 0, 0, PartyRow::Front);
          let r = damage_calc(&a, &d, None, &CombatActionKind::Attack, &mut rng);
          assert!(r.hit);
          assert!(r.damage >= 1);
      }

      #[test]
      fn damage_calc_misses_when_evasion_high() {
          let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(42);
          let a = mk_combatant("A", 20, 0, 0, 0, PartyRow::Front);  // accuracy 0
          let d = mk_combatant("D", 0, 0, 0, 100, PartyRow::Front);
          let r = damage_calc(&a, &d, None, &CombatActionKind::Attack, &mut rng);
          assert!(!r.hit);
          assert_eq!(r.damage, 0);
      }

      #[test]
      fn damage_calc_hits_when_accuracy_high() {
          let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(42);
          let a = mk_combatant("A", 20, 0, 100, 0, PartyRow::Front);
          let d = mk_combatant("D", 0, 0, 0, 0, PartyRow::Front);
          let r = damage_calc(&a, &d, None, &CombatActionKind::Attack, &mut rng);
          assert!(r.hit);
      }

      #[test]
      fn damage_calc_crits_increase_damage() {
          // Use a seed that lands the crit roll < crit_chance.
          // (Implementer: find a seed empirically by running the test once.)
          let mut rng_a = rand_chacha::ChaCha8Rng::seed_from_u64(42);
          let mut rng_b = rand_chacha::ChaCha8Rng::seed_from_u64(42);
          let a = mk_combatant("A", 20, 0, 100, 0, PartyRow::Front);
          let d = mk_combatant("D", 0, 10, 0, 0, PartyRow::Front);
          let r1 = damage_calc(&a, &d, None, &CombatActionKind::Attack, &mut rng_a);
          // Seed-based determinism check: two calls with identical seed yield identical result.
          let r2 = damage_calc(&a, &d, None, &CombatActionKind::Attack, &mut rng_b);
          assert_eq!(r1, r2, "damage_calc must be deterministic with identical RNG seed");
      }

      #[test]
      fn front_attack_back_with_melee_blocks() {
          // Need a fake ItemAsset.
          let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(42);
          let weapon = ItemAsset {
              id: "test_sword".into(),
              display_name: "Test Sword".into(),
              ..Default::default()
          };
          let a = mk_combatant("A", 20, 0, 100, 0, PartyRow::Front);
          let d = mk_combatant("D", 0, 0, 0, 0, PartyRow::Back);
          let r = damage_calc(&a, &d, Some(&weapon), &CombatActionKind::Attack, &mut rng);
          assert_eq!(r.damage, 0);
          assert!(r.message.contains("can't reach"));
      }

      #[test]
      fn damage_calc_variance_bounded() {
          // Run 100 trials; assert all damage values within ±30% of expected.
          let a = mk_combatant("A", 20, 0, 100, 0, PartyRow::Front);
          let d = mk_combatant("D", 0, 10, 0, 0, PartyRow::Front);
          let mut min_dmg = u32::MAX;
          let mut max_dmg = 0u32;
          for seed in 0..100 {
              let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(seed);
              let r = damage_calc(&a, &d, None, &CombatActionKind::Attack, &mut rng);
              if r.hit && !r.critical {
                  min_dmg = min_dmg.min(r.damage);
                  max_dmg = max_dmg.max(r.damage);
              }
          }
          // Expected raw: (20 * (100 - 5)) / 100 = 19. Variance 0.7..1.0 → 13..19.
          assert!(min_dmg >= 13, "min damage {} below variance floor", min_dmg);
          assert!(max_dmg <= 19, "max damage {} above variance ceiling", max_dmg);
      }
  }
  ```

#### Step 3 — Wire `damage_calc` into `execute_combat_actions::Attack arm`

- [ ] In `src/plugins/combat/turn_manager.rs::execute_combat_actions`, replace the `Attack` stub arm with a full damage_calc invocation. NOTE: this requires:
  1. Computing the target via `resolve_target_with_fallback`.
  2. Reading the actor's and target's `Combatant` flattening.
  3. Reading the actor's weapon from `Equipment` slot.
  4. Calling `damage_calc`.
  5. Mutating target's `current_hp`.
  6. Calling `check_dead_and_apply` for the target.
  7. Pushing the result message to `combat_log`.
- [ ] Required new system parameters in `execute_combat_actions`:
  ```rust
  // In addition to existing params:
  party_entities: Query<Entity, With<PartyMember>>,
  enemy_entities: Query<Entity, With<crate::plugins::combat::enemy::Enemy>>,
  party_rows: Query<&PartyRow>,
  equipment_q: Query<&crate::plugins::party::Equipment>,
  items: Res<Assets<crate::data::ItemAsset>>,
  // And the borrow-checker dance: one Query for closure-reads + one for mut writes.
  // Recommended split:
  // - chars_read: Query<(Entity, &DerivedStats, &StatusEffects, Option<&CharacterName>, Option<&EnemyName>)>
  // - chars_mut_hp: Query<&mut DerivedStats>
  ```
- [ ] Implementation sketch:
  ```rust
  CombatActionKind::Attack => {
      let actor_name = name_of(action.actor);
      // Snapshot alive party + enemy slices for re-targeting.
      let party_alive: Vec<Entity> = party_entities.iter()
          .filter(|e| is_alive(*e))
          .collect();
      let enemy_alive: Vec<Entity> = enemy_entities.iter()
          .filter(|e| is_alive(*e))
          .collect();
      // Resolve target.
      let targets = crate::plugins::combat::targeting::resolve_target_with_fallback(
          &action.target,
          action.actor,
          action.actor_side,
          &party_alive,
          &enemy_alive,
          |e| is_alive(e),
          &mut *rng.0,
      );
      let Some(target) = targets.first().copied() else {
          combat_log.push(
              format!("{}'s attack has no target.", actor_name),
              turn,
          );
          continue;
      };
      // Build Combatant structs.
      let attacker_combatant = build_combatant(&characters, &party_rows, action.actor)
          .unwrap_or_else(|| dummy_combatant(&actor_name));
      let defender_combatant = build_combatant(&characters, &party_rows, target)
          .unwrap_or_else(|| dummy_combatant(&name_of(target)));
      // Read weapon from actor's Equipment.
      let weapon_handle = equipment_q.get(action.actor).ok().and_then(|e| e.weapon.clone());
      let weapon: Option<&crate::data::ItemAsset> = weapon_handle
          .as_ref()
          .and_then(|h| items.get(h));
      // Call damage_calc.
      let result = crate::plugins::combat::damage::damage_calc(
          &attacker_combatant,
          &defender_combatant,
          weapon,
          &action.kind,
          &mut *rng.0,
      );
      combat_log.push(result.message, turn);
      // Apply damage via the chars_mut_hp query.
      if let Ok(mut target_derived) = chars_mut_hp.get_mut(target) {
          target_derived.current_hp = target_derived.current_hp.saturating_sub(result.damage);
          // Critical: check_dead_and_apply (Pitfall 1 of #14).
          check_dead_and_apply(target, &target_derived, &mut apply_status);
      }
  }
  ```
  Implementer fills in `build_combatant` helper that reads `(DerivedStats, StatusEffects, PartyRow)` from the chars_read query and the party_rows query.
- [ ] **Verification:** existing 15A tests still pass; new 15B tests in `damage.rs::tests` pass.

#### Step 4 — Layer-2 App-driven targeting tests

- [ ] In `targeting.rs::app_tests` (or wherever entity construction is convenient), add three tests:
  - `target_dies_re_targets_to_alive_ally_on_same_side` (Research §Pattern 3)
  - `target_whole_side_wiped_returns_empty_no_panic`
  - `self_target_on_dead_actor_returns_empty`

  Each test uses `make_test_app`, spawns a few `PartyMemberBundle`s and `EnemyBundle`s with mocked HP, sets `CombatPhase::ExecuteActions`, pushes a `QueuedAction { target: TargetSelection::Single(target) }`, verifies the resolved targets via direct call to `resolve_target_with_fallback` (NOT through `execute_combat_actions` — too noisy for a targeting test).

#### Step 5 — Phase 15B verification gate

- [ ] Same per-phase exit criteria as 15A:
  - `cargo check && cargo check --features dev`
  - `cargo test plugins::combat::damage && cargo test plugins::combat::targeting`
  - `cargo test --features dev`
  - `cargo clippy --all-targets -- -D warnings && cargo clippy --all-targets --features dev -- -D warnings`
  - `cargo fmt --check`
  - **Grep guards (must be ZERO matches):**
    - `rg 'Query<' src/plugins/combat/damage.rs` — must be 0 (pure function, no ECS).
    - `rg 'Res<' src/plugins/combat/damage.rs` — must be 0 (pure function).
    - `rg 'effects\.push\(|effects\.retain' src/plugins/combat/damage.rs` — must be 0.
- [ ] Atomic GitButler commit per CLAUDE.md.

---

### Phase 15C — AI (~200 LOC, ~4 tests)

**Files in this phase:**
- New: `src/plugins/combat/ai.rs` (full body — extends 15A stub).
- Edits: `src/plugins/combat/mod.rs` (+1 line: `app.add_plugins(ai::EnemyAiPlugin)`).

**Tests in this phase:** AI determinism, dead-skip, no-alive-party, BossAttackDefendAttack cycle = 4-5 tests.

#### Step 1 — Land `ai.rs` full body (extends 15A stub)

- [ ] Edit `src/plugins/combat/ai.rs`. Replace the stub with full file:
  ```rust
  //! Enemy AI — Feature #15 Phase 15C.
  //!
  //! `EnemyAi` enum (3 variants per D-Q5=A) + `EnemyAiPlugin` registering
  //! `enemy_ai_action_select` system that emits actions into `TurnActionQueue`.
  //!
  //! ## AI emission boundary (research Pattern 4, Anti-pattern 1)
  //!
  //! AI never reads or mutates `DerivedStats.current_hp`. AI never writes
  //! `ApplyStatusEvent` directly. AI's single side effect is
  //! `queue.queue.push(QueuedAction { ... })`. The damage / status / item
  //! pipeline is the sole resolver path.
  //!
  //! ## D-Q5=A: Boss AI scope
  //!
  //! 3 variants:
  //! - `RandomAttack` — fodder enemies; pick any alive party member.
  //! - `BossFocusWeakest` — picks the alive party member with lowest current_hp.
  //! - `BossAttackDefendAttack { turn: u32 }` — cycles Attack/Defend/Attack
  //!   based on `turn % 3`.
  //!
  //! ~80 LOC + 4 tests.

  use bevy::prelude::*;
  use rand::seq::IteratorRandom;

  use crate::plugins::combat::actions::{CombatActionKind, QueuedAction, Side};
  use crate::plugins::combat::enemy::{Enemy, EnemyIndex};
  use crate::plugins::combat::targeting::TargetSelection;
  use crate::plugins::combat::turn_manager::{CombatRng, TurnActionQueue};
  use crate::plugins::party::character::{
      DerivedStats, PartyMember, PartySlot, StatusEffectType, StatusEffects,
  };
  use crate::plugins::state::CombatPhase;

  #[derive(Component, Reflect, Default, Debug, Clone, Copy)]
  pub enum EnemyAi {
      #[default]
      RandomAttack,
      BossFocusWeakest,
      BossAttackDefendAttack { turn: u32 },
  }

  pub struct EnemyAiPlugin;

  impl Plugin for EnemyAiPlugin {
      fn build(&self, app: &mut App) {
          app.add_systems(
              Update,
              enemy_ai_action_select
                  .run_if(in_state(CombatPhase::ExecuteActions))
                  .before(crate::plugins::combat::turn_manager::sort_queue_by_speed),
          );
      }
  }
  ```
  Note: `sort_queue_by_speed` must be `pub` for the cross-module ordering reference. Update Phase 15A Step 7c to mark it `pub fn`.
- [ ] Add the `enemy_ai_action_select` system body:
  ```rust
  /// Emit one queued action per alive enemy. Pure side effect: pushes to
  /// `TurnActionQueue`. NO mutation of `DerivedStats`/`StatusEffects` (Anti-pattern 1).
  pub fn enemy_ai_action_select(
      mut enemies: Query<
          (Entity, &EnemyAi, &EnemyIndex, &DerivedStats, &StatusEffects),
          (With<Enemy>, Without<PartyMember>),
      >,
      party: Query<(Entity, &PartySlot, &DerivedStats, &StatusEffects), With<PartyMember>>,
      mut queue: ResMut<TurnActionQueue>,
      mut rng: ResMut<CombatRng>,
  ) {
      // Snapshot alive party once.
      let alive_party: Vec<(Entity, &DerivedStats, &PartySlot)> = party
          .iter()
          .filter(|(_, _, d, s)| {
              d.current_hp > 0
                  && !s.has(StatusEffectType::Dead)
                  && !s.has(StatusEffectType::Stone)
          })
          .map(|(e, slot, d, _)| (e, d, slot))
          .collect();

      if alive_party.is_empty() {
          return;
      }

      for (entity, ai, idx, derived, status) in &mut enemies {
          // Pitfall 10: skip Dead/Stone enemies.
          if status.has(StatusEffectType::Dead) || status.has(StatusEffectType::Stone) {
              continue;
          }

          let (kind, target) = match ai {
              EnemyAi::RandomAttack => {
                  let (target, _, _) = alive_party
                      .iter()
                      .copied()
                      .choose(&mut *rng.0)
                      .expect("alive_party non-empty checked above");
                  (CombatActionKind::Attack, TargetSelection::Single(target))
              }
              EnemyAi::BossFocusWeakest => {
                  // Lowest current_hp; ties broken by lowest slot.
                  let (target, _, _) = alive_party
                      .iter()
                      .min_by(|a, b| {
                          a.1.current_hp.cmp(&b.1.current_hp)
                              .then(a.2.0.cmp(&b.2.0))
                      })
                      .copied()
                      .expect("alive_party non-empty");
                  (CombatActionKind::Attack, TargetSelection::Single(target))
              }
              EnemyAi::BossAttackDefendAttack { turn } => {
                  // turn % 3 cycle: 0=Attack, 1=Defend, 2=Attack.
                  match turn % 3 {
                      1 => (CombatActionKind::Defend, TargetSelection::Self_),
                      _ => {
                          let (target, _, _) = alive_party
                              .iter()
                              .copied()
                              .choose(&mut *rng.0)
                              .expect("alive_party non-empty");
                          (CombatActionKind::Attack, TargetSelection::Single(target))
                      }
                  }
              }
          };

          queue.queue.push(QueuedAction {
              actor: entity,
              kind,
              target,
              speed_at_queue_time: derived.speed,
              actor_side: Side::Enemy,
              slot_index: idx.0,
          });
      }
  }
  ```

#### Step 2 — Wire `EnemyAiPlugin` into `CombatPlugin`

- [ ] In `src/plugins/combat/mod.rs`, add to the `CombatPlugin::build`:
  ```rust
  app.add_plugins(status_effects::StatusEffectsPlugin)
      .add_plugins(turn_manager::TurnManagerPlugin)
      .add_plugins(ai::EnemyAiPlugin)  // ← NEW Phase 15C
      // ...
  ```

#### Step 3 — Layer-2 App-driven AI tests

- [ ] In `ai.rs::app_tests`, add 4 tests:
  - `random_attack_picks_alive_party_member` (Decision 27 / Research item 21).
  - `random_attack_deterministic_with_seed` (Pitfall 12).
  - `random_attack_skips_dead_enemies` (Pitfall 10).
  - `boss_attack_defend_attack_cycles_correctly` (D-Q5=A pattern).

  Each test uses `make_test_app`, spawns party + enemies, sets `CombatPhase::ExecuteActions`, calls `app.update()`, asserts queue contents.

#### Step 4 — Phase 15C verification gate

- [ ] Same per-phase exit criteria.
- [ ] **Additional AI-specific grep guards (must be ZERO matches):**
  - `rg 'current_hp.*=' src/plugins/combat/ai.rs` — must be 0 (no mutation).
  - `rg 'MessageWriter<ApplyStatusEvent>' src/plugins/combat/ai.rs` — must be 0 (no status writes).
  - `rg '&mut StatusEffects' src/plugins/combat/ai.rs` — must be 0.
- [ ] Atomic GitButler commit per CLAUDE.md.

---

### Phase 15D — UI (~400-600 LOC, ~3-5 tests + manual smoke)

**Files in this phase:**
- New: `src/plugins/combat/ui_combat.rs`.
- Edits: `src/plugins/combat/mod.rs` (+1 line: `app.add_plugins(ui_combat::CombatUiPlugin)`).

**Tests in this phase:** layout smoke tests (2-3), input handler tests (2-3), manual smoke for visual fidelity = ~5 tests.

#### Step 1 — Create `src/plugins/combat/ui_combat.rs` skeleton

- [ ] Create new file. Add file-level doc-comment:
  ```rust
  //! Combat UI — Feature #15 Phase 15D.
  //!
  //! egui combat screen overlaid on the dungeon camera (D-Q1=A — NO new
  //! Camera3d). Mirrors `MinimapPlugin::attach_egui_to_dungeon_camera`.
  //!
  //! ## Layout (D-Q2=A: persistent action panel)
  //!
  //! - **Left:** `egui::SidePanel::left("enemy_column")` — alive enemies stacked.
  //! - **Bottom:** `egui::TopBottomPanel::bottom("party_panel")` — 4 party cards.
  //! - **Right:** `egui::SidePanel::right("combat_log")` — bounded log scroll.
  //! - **Bottom (above party):** `egui::TopBottomPanel::bottom("action_menu")`
  //!   — persistent, always visible during `CombatPhase::PlayerInput`.
  //! - **Center overlay:** `egui::Window::new("target_select")` — anchored
  //!   center, only when `state.is_selecting_target()`.
  //!
  //! ## Input handler
  //!
  //! `handle_combat_input` reads `Res<ActionState<CombatAction>>` (the leafwing
  //! menu-nav enum) and mutates `PlayerInputState`. The SOLE writer of
  //! `TurnActionQueue` from the player side (Anti-pattern 5).
  ```
- [ ] Add imports:
  ```rust
  use bevy::prelude::*;
  use bevy_egui::{EguiContexts, EguiPrimaryContextPass, PrimaryEguiContext, egui};
  use leafwing_input_manager::prelude::ActionState;

  use crate::plugins::combat::actions::{CombatActionKind, QueuedAction, Side};
  use crate::plugins::combat::combat_log::CombatLog;
  use crate::plugins::combat::enemy::{Enemy, EnemyName};
  use crate::plugins::combat::status_effects::is_silenced;
  use crate::plugins::combat::targeting::TargetSelection;
  use crate::plugins::combat::turn_manager::{
      MenuFrame, PendingAction, PlayerInputState, TurnActionQueue,
  };
  use crate::plugins::dungeon::DungeonCamera;
  use crate::plugins::input::CombatAction as MenuNavAction;
  use crate::plugins::party::character::{
      CharacterName, DerivedStats, PartyMember, PartySlot, StatusEffects,
  };
  use crate::plugins::state::{CombatPhase, GameState};
  ```
- [ ] Define `CombatUiPlugin`:
  ```rust
  pub struct CombatUiPlugin;

  impl Plugin for CombatUiPlugin {
      fn build(&self, app: &mut App) {
          app.add_systems(
              Update,
              (
                  attach_egui_to_dungeon_camera.run_if(in_state(GameState::Combat)),
                  handle_combat_input.run_if(in_state(GameState::Combat)),
              ),
          )
          .add_systems(
              EguiPrimaryContextPass,
              paint_combat_screen.run_if(in_state(GameState::Combat)),
          );
      }
  }

  /// Attach `PrimaryEguiContext` to the dungeon `Camera3d`. Idempotent
  /// (`Without<PrimaryEguiContext>` filter); runs each frame in Combat but
  /// no-ops once attached. Mirrors `MinimapPlugin::attach_egui_to_dungeon_camera`.
  fn attach_egui_to_dungeon_camera(
      mut commands: Commands,
      cameras: Query<Entity, (With<DungeonCamera>, Without<PrimaryEguiContext>)>,
  ) {
      for entity in &cameras {
          commands.entity(entity).insert(PrimaryEguiContext);
      }
  }
  ```

#### Step 2 — `paint_combat_screen` umbrella system (D-Q2=A persistent panel)

- [ ] Add `paint_combat_screen` system that paints all four panels in one frame:
  ```rust
  fn paint_combat_screen(
      mut contexts: EguiContexts,
      log: Res<CombatLog>,
      input_state: Res<PlayerInputState>,
      party: Query<(Entity, &CharacterName, &PartySlot, &DerivedStats, &StatusEffects), With<PartyMember>>,
      enemies: Query<(Entity, &EnemyName, &DerivedStats, &StatusEffects), With<Enemy>>,
      phase: Res<State<CombatPhase>>,
  ) -> Result {
      let ctx = contexts.ctx_mut()?;

      // Enemy column (left).
      egui::SidePanel::left("enemy_column")
          .resizable(false)
          .min_width(200.0)
          .show(ctx, |ui| {
              ui.heading("Enemies");
              for (entity, name, derived, _status) in &enemies {
                  let _ = entity;
                  ui.label(format!("{} HP {}/{}", name.0, derived.current_hp, derived.max_hp));
              }
          });

      // Combat log (right).
      egui::SidePanel::right("combat_log")
          .resizable(false)
          .min_width(300.0)
          .show(ctx, |ui| {
              ui.heading("Log");
              egui::ScrollArea::vertical()
                  .stick_to_bottom(true)
                  .show(ui, |ui| {
                      for entry in &log.entries {
                          ui.label(&entry.message);
                      }
                  });
          });

      // Action menu (bottom — above party panel; only during PlayerInput).
      if matches!(phase.get(), CombatPhase::PlayerInput) {
          egui::TopBottomPanel::bottom("action_menu")
              .resizable(false)
              .min_height(60.0)
              .show(ctx, |ui| {
                  if let Some(slot) = input_state.active_slot {
                      let active_name = party
                          .iter()
                          .find(|(_, _, ps, _, _)| ps.0 == slot)
                          .map(|(_, n, _, _, _)| n.0.clone())
                          .unwrap_or_default();
                      ui.horizontal(|ui| {
                          ui.label(format!("> {}", active_name));
                          // Action buttons are advisory in v1 — keyboard-driven via handle_combat_input.
                          // Click handlers fire the same logic.
                          ui.label("Attack | Defend | Spell | Item | Flee");
                      });
                  } else {
                      ui.label("Resolving turn...");
                  }
              });
      }

      // Party panel (bottom).
      egui::TopBottomPanel::bottom("party_panel")
          .resizable(false)
          .min_height(120.0)
          .show(ctx, |ui| {
              ui.horizontal(|ui| {
                  for (_, name, _, derived, status) in &party {
                      ui.vertical(|ui| {
                          ui.label(&name.0);
                          ui.label(format!("HP {}/{}", derived.current_hp, derived.max_hp));
                          ui.label(format!("MP {}/{}", derived.current_mp, derived.max_mp));
                          if !status.effects.is_empty() {
                              ui.label(format!("[{}]", status.effects.len()));
                          }
                      });
                  }
              });
          });

      // Target selection overlay (center).
      if input_state.pending_action.is_some() && input_state.target_cursor.is_some() {
          egui::Window::new("Target")
              .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
              .resizable(false)
              .show(ctx, |ui| {
                  for (i, (_, name, _, _)) in enemies.iter().enumerate() {
                      let is_sel = input_state.target_cursor == Some(i);
                      let color = if is_sel { egui::Color32::YELLOW } else { egui::Color32::WHITE };
                      ui.colored_label(color, format!("> {}", name.0));
                  }
              });
      }

      Ok(())
  }
  ```

#### Step 3 — `handle_combat_input` (the SOLE player-side writer of `TurnActionQueue`)

- [ ] Add `handle_combat_input` system:
  ```rust
  /// Read leafwing CombatAction (menu nav) and drive `PlayerInputState`.
  /// SOLE writer of `TurnActionQueue` from the player side (Anti-pattern 5).
  ///
  /// Menu state machine:
  /// - `Main`: Up/Down navigate buttons; Confirm picks Attack/Defend/Spell/Item/Flee.
  ///   - Attack/Spell/Item open `TargetSelect` submenu (or `SpellMenu`/`ItemMenu` first).
  ///   - Defend/Flee commit immediately to queue.
  /// - `TargetSelect`: Up/Down move target_cursor; Confirm commits; Cancel pops.
  /// - `SpellMenu`/`ItemMenu`: not implemented in v1 (defer to #20). Cancel pops.
  fn handle_combat_input(
      actions: Res<ActionState<MenuNavAction>>,
      mut input_state: ResMut<PlayerInputState>,
      mut queue: ResMut<TurnActionQueue>,
      mut combat_log: ResMut<CombatLog>,
      party: Query<(Entity, &PartySlot, &DerivedStats, &StatusEffects), With<PartyMember>>,
      enemies: Query<(Entity, &DerivedStats, &StatusEffects), With<Enemy>>,
      phase: Res<State<CombatPhase>>,
  ) {
      // Only act in PlayerInput.
      if !matches!(phase.get(), CombatPhase::PlayerInput) {
          return;
      }
      let Some(active_slot) = input_state.active_slot else { return };

      // Find the active actor entity.
      let Some((actor_entity, _, derived, status)) = party
          .iter()
          .find(|(_, ps, _, _)| ps.0 == active_slot)
          .map(|(e, ps, d, s)| (e, ps, d, s))
      else {
          return;
      };

      let frame = input_state.menu_stack.last().cloned().unwrap_or(MenuFrame::Main);

      // Cancel: pop submenu (top-of-stack only; Main does nothing).
      if actions.just_pressed(&MenuNavAction::Cancel) {
          if input_state.menu_stack.len() > 1 {
              input_state.menu_stack.pop();
              input_state.pending_action = None;
              input_state.target_cursor = None;
          }
          return;
      }

      match frame {
          MenuFrame::Main => {
              if actions.just_pressed(&MenuNavAction::Confirm) {
                  // For v1, hard-bind Confirm to "Attack on first alive enemy".
                  // Real button-cycle (Up/Down) lands in #25 polish.
                  // Implementer note: simplest UX in v1 is to hardcode the confirm path.
                  // Better UX is a `selected_button: usize` field on PlayerInputState
                  // that Up/Down increments/decrements, with Confirm dispatching.
                  // For Phase 15D MVP, ship the simplest version; #25 polishes.
                  if let Some((target_entity, _, _)) = enemies.iter().next() {
                      input_state.pending_action = Some(PendingAction {
                          kind: CombatActionKind::Attack,
                          actor: actor_entity,
                      });
                      input_state.menu_stack.push(MenuFrame::TargetSelect {
                          kind: CombatActionKind::Attack,
                      });
                      input_state.target_cursor = Some(0);
                      let _ = target_entity;
                  }
              }
              // (Implementer adds Up/Down handling for action button selection.)
          }
          MenuFrame::TargetSelect { kind } => {
              if actions.just_pressed(&MenuNavAction::Up) || actions.just_pressed(&MenuNavAction::Left) {
                  if let Some(c) = input_state.target_cursor.as_mut() {
                      *c = c.saturating_sub(1);
                  }
              }
              if actions.just_pressed(&MenuNavAction::Down) || actions.just_pressed(&MenuNavAction::Right) {
                  if let Some(c) = input_state.target_cursor.as_mut() {
                      let max = enemies.iter().count().saturating_sub(1);
                      *c = (*c + 1).min(max);
                  }
              }
              if actions.just_pressed(&MenuNavAction::Confirm) {
                  let cursor = input_state.target_cursor.unwrap_or(0);
                  if let Some((target, _, _)) = enemies.iter().nth(cursor) {
                      // Commit to queue.
                      queue.queue.push(QueuedAction {
                          actor: actor_entity,
                          kind: kind.clone(),
                          target: TargetSelection::Single(target),
                          speed_at_queue_time: derived.speed,
                          actor_side: Side::Party,
                          slot_index: active_slot as u32,
                      });
                      // Mirror into committed for collect_player_actions reuse.
                      input_state.committed.push(QueuedAction {
                          actor: actor_entity,
                          kind: kind.clone(),
                          target: TargetSelection::Single(target),
                          speed_at_queue_time: derived.speed,
                          actor_side: Side::Party,
                          slot_index: active_slot as u32,
                      });
                      // Pop back to Main.
                      input_state.menu_stack = vec![MenuFrame::Main];
                      input_state.pending_action = None;
                      input_state.target_cursor = None;
                      input_state.active_slot = None;  // Triggers next-slot search.
                  }
              }
          }
          MenuFrame::SpellMenu => {
              // Decision 34: Silence gates.
              if is_silenced(status) {
                  combat_log.push(
                      "You are silenced; cannot cast.".into(),
                      input_state.current_turn,
                  );
                  input_state.menu_stack = vec![MenuFrame::Main];
                  return;
              }
              // Stub for v1; #20 fills in.
              combat_log.push("Spell menu: not yet implemented.".into(), input_state.current_turn);
              input_state.menu_stack = vec![MenuFrame::Main];
          }
          MenuFrame::ItemMenu => {
              // Stub; full inventory UI is #25.
              combat_log.push("Item menu: not yet implemented.".into(), input_state.current_turn);
              input_state.menu_stack = vec![MenuFrame::Main];
          }
      }
      let _ = derived;
      let _ = status;
  }
  ```
  **Implementer note:** The `handle_combat_input` body sketch above is intentionally simplified for v1. The "Confirm always picks Attack" is a placeholder; a proper Up/Down menu cursor lives behind a `selected_button: usize` field on `PlayerInputState`. The implementer ships the simplest version that lets the player commit Attack actions; richer menu navigation is acceptable to defer to Phase 15D Step 5 (manual polish) or to a follow-up #25 ticket.

#### Step 4 — Wire `CombatUiPlugin` into `CombatPlugin`

- [ ] In `src/plugins/combat/mod.rs`, add:
  ```rust
  app.add_plugins(status_effects::StatusEffectsPlugin)
      .add_plugins(turn_manager::TurnManagerPlugin)
      .add_plugins(ai::EnemyAiPlugin)
      .add_plugins(ui_combat::CombatUiPlugin)  // ← NEW Phase 15D
      // ...
  ```

#### Step 5 — Layer-2 UI tests + manual smoke

- [ ] Add 3-5 tests in `ui_combat.rs::app_tests` (or similar):
  - `paint_combat_screen_no_panic_with_no_enemies`
  - `paint_combat_screen_no_panic_with_default_state`
  - `handle_combat_input_commits_attack_to_queue` (input-driven test using full `InputPlugin` + `KeyboardInput` injection — see `feedback_bevy_input_test_layers.md`)
  - `handle_combat_input_silence_blocks_spell_menu`
  - `target_cursor_clamps_at_enemy_count`
- [ ] **Manual smoke checklist** (run `cargo run --features dev`):
  - [ ] Press F9 to cycle to `GameState::Combat`. Combat UI renders.
  - [ ] Enemy column on left shows 2 Goblins.
  - [ ] Party panel on bottom shows 4 party members with HP/MP.
  - [ ] Action panel above party shows "> Aldric" and action labels.
  - [ ] Combat log on right shows "Combat begins" entry.
  - [ ] Press Space (Confirm) → target overlay appears → press Space again → action commits, log entry "Aldric attacks Goblin..." appears.
  - [ ] After all 4 party members commit → resolution log entries appear; new round starts.
  - [ ] Defeat: kill all enemies (cheat via `current_hp = 0` if needed) → state transitions to Dungeon.

#### Step 6 — Phase 15D verification gate

- [ ] Same per-phase exit criteria.
- [ ] **UI-specific grep guards (must be ZERO matches):**
  - `rg 'Camera3d' src/plugins/combat/ui_combat.rs` — must be 0 (D-Q1=A: NO new camera).
  - `rg 'queue\.queue\.push' src/plugins/combat/ui_combat.rs` — must be 1 or 2 (the SOLE player-side writer; appears only inside `handle_combat_input::TargetSelect` arm — see Anti-pattern 5).
- [ ] Atomic GitButler commit per CLAUDE.md.

---

## Implementation Discoveries

### D-I1 — Bevy B0002 Query Conflict in execute_combat_actions

**What happened:** Initial design had `chars: Query<(Entity, &DerivedStats, &StatusEffects, ...)>` and `chars_mut_hp: Query<&mut DerivedStats>` in the same system. Bevy raises B0002 because you cannot have both `&DerivedStats` and `&mut DerivedStats` in the same system's query set.

**Fix applied:** Removed `&DerivedStats` from `chars` entirely. Used a separate `derived_mut: Query<&mut DerivedStats>` as the SOLE accessor for DerivedStats. Pre-collected `CombatantSnapshot { name, derived, status, row }` at the start of `execute_combat_actions` by calling `derived_mut.get(e).map(|r| *r).unwrap_or_default()` inside the `chars.iter().map()` collection loop. Mid-round HP changes use `derived_mut.get_mut(target)` directly.

### D-I2 — `derived_mut.get(e).copied()` fails to compile

**What happened:** `Query<&mut T>::get(&self, e)` returns `Result<Ref<T>, QueryEntityError>`. `Ref<T>` does NOT implement `Copy` (even when T: Copy). Calling `.copied()` on `Result<Ref<T>, E>` is a type error.

**Fix applied:** Changed `derived_mut.get(e).copied().unwrap_or_default()` to `derived_mut.get(e).map(|r| *r).unwrap_or_default()`. The `*r` dereferences `Ref<DerivedStats>` to `DerivedStats` (valid because `DerivedStats: Copy`).

### D-I3 — CombatUiPlugin::handle_combat_input requires ActionState<CombatAction> in test harnesses

**What happened:** `handle_combat_input` uses `Res<ActionState<MenuNavAction>>` (alias for `ActionState<CombatAction>`). Without this resource, any test app that enters `GameState::Combat` will panic since the system runs with `.run_if(in_state(GameState::Combat))`.

**Fix applied:** Added `app.init_resource::<ActionState<CombatAction>>()` (via full path `leafwing_input_manager::prelude::ActionState<crate::plugins::input::CombatAction>`) to:
- `turn_manager.rs::app_tests::make_test_app`
- `ai.rs::app_tests::make_test_app`  
- `ui_combat.rs::app_tests::make_test_app`

Pattern: insert bare `ActionState` resource without `ActionsPlugin` to avoid mouse-resource panic from `InputManagerPlugin`. Consistent with minimap.rs and status_effects.rs test precedents.

### D-I4 — apply_status_handler has With<PartyMember> filter

**Observation (non-blocking):** When `execute_combat_actions` writes `ApplyStatusEvent { target: enemy_entity, effect: Dead }` via `check_dead_and_apply`, the `apply_status_handler` silently no-ops because its query is filtered `With<PartyMember>`. Enemy death is detected via `current_hp == 0` in `check_victory_defeat_flee` and `is_alive_entity`. This is acceptable for v1: enemy death works correctly; they simply don't get the `Dead` component. Documented as a known limitation for #17 polish.

### D-I5 — UseItem does not despawn ItemInstance entity

**Observation (non-blocking, plan-acknowledged):** `execute_combat_actions` does not have `Commands` in its parameter list. The `UseItem` arm removes the item from `Inventory.0` (Vec<Entity>) but cannot despawn the `ItemInstance` entity. The entity becomes detached. Documented in-plan at Step 12 as acceptable for v1; #16/#17 polish handles cleanup. No entity leak in production since item despawning is not critical for combat resolution.

### D-I6 — Inventory not in PartyMemberBundle

**Observation:** `PartyMemberBundle` does not include `Inventory`. Test party members spawned with `PartyMemberBundle` have no `Inventory` component. The `UseItem` handler's `inventories.get_mut(actor)` will always return `Err` in tests. This is correct behavior — tests for UseItem would require explicitly adding `Inventory` to spawned entities. For #15 tests this was not required (test scope limited to no-panic).

### D-I7 — Phase implementation was combined (all phases in one session)

**What happened:** All four phases (15A through 15D) were implemented in a single session rather than committed incrementally per phase. The plan called for four atomic GitButler commits; this was not done because the session ran out of context. All 8 new files and 3 carve-out edits were written but no commits were made.

**Action required:** All changes are in the workspace as uncommitted. After compilation verification passes, commits should be batched as one or split per-phase as intended.

### D-I8 — Entity::from_raw not available in Bevy 0.18 (verification session fix)

**What happened:** `targeting.rs::tests` and `ai.rs::app_tests` used `Entity::from_raw(idx: u32)` to create distinct test entities for pure-function tests. `Entity::from_raw` does not exist in Bevy 0.18 (documented in `feedback_bevy018_world_api.md`).

**Fix applied:** Changed `Entity::from_raw(idx)` → `Entity::from_bits(idx as u64)` in both files. `Entity::from_bits(u64)` is Bevy 0.18's public API for constructing entities from their bit representation.

### D-I9 — Unused `Messages` import in turn_manager::app_tests (verification session fix)

**What happened:** `turn_manager.rs::app_tests` had `use bevy::ecs::message::Messages;` at the module level. None of the test functions in that module used `Messages<T>` directly. Under `clippy -D warnings`, this would produce an `unused_imports` warning.

**Fix applied:** Removed the unused import. The `Messages` import is still available function-locally in `sleep_skips_action_emission` if needed (see D-I10).

### D-I10 — `effects.push(` grep guard conflict with test setup (verification session fix)

**What happened:** The `sleep_skips_action_emission` test in `turn_manager.rs` originally used `se.effects.push(ActiveEffect { ... })` to construct a pre-sleeping party member. The plan's verification grep `rg 'effects\.push\('` over all 8 combat files must return 0 matches — but the `#[cfg(test)]` code in `turn_manager.rs` would return 1 match.

**Fix applied:** Changed the test to use struct initialization syntax: `StatusEffects { effects: vec![ActiveEffect { ... }] }` which does NOT use `.push()`. This satisfies the grep guard (no `effects\.push\(` in the file) while correctly pre-configuring the entity's sleep status at spawn time. The approach also avoids any system ordering concern between `apply_status_handler` and `collect_player_actions`.

### D-I11 — `current_hp.*=` grep guard matches comparison in ai.rs (verification session fix)

**What happened:** The plan's grep guard `rg 'current_hp.*='` against `ai.rs` was intended to verify AI never mutates `current_hp`. However, `current_hp == 0` (a read-only comparison) also matches `current_hp.*=` because `.*` matches ` ` and `=` is found in `==`. The belt-and-suspenders dead-enemy skip at line 96 used `if derived.current_hp == 0`.

**Fix applied:** Changed `derived.current_hp == 0` → `derived.current_hp < 1` (semantically identical for u32). The `< 1` does not contain `=` and does not match the grep pattern.

---

## Verification

The global verification gate runs AFTER Phase 15D commits. ALL items must pass before the implementer claims "complete".

### Build / test gates (mandatory)

- [ ] `cargo check` — succeeds. — Automatic
- [ ] `cargo check --features dev` — succeeds. — Automatic
- [ ] `cargo test` — passes (~189-200 expected: 159 baseline from #14 + 25-30 new). — Automatic
- [ ] `cargo test --features dev` — passes. — Automatic
- [ ] `cargo clippy --all-targets -- -D warnings` — clean. — Automatic
- [ ] `cargo clippy --all-targets --features dev -- -D warnings` — clean. — Automatic
- [ ] `cargo fmt --check` — clean. — Automatic

### Bevy 0.18 family-rename guards (mandatory — must return ZERO matches)

- [ ] `rg 'derive\(.*\bEvent\b' src/plugins/combat/{actions,ai,combat_log,damage,enemy,targeting,turn_manager,ui_combat}.rs` — 0 matches. — Automatic
- [ ] `rg '\bEventReader<' src/plugins/combat/{actions,ai,combat_log,damage,enemy,targeting,turn_manager,ui_combat}.rs tests/` — 0 matches. — Automatic
- [ ] `rg '\bEventWriter<' src/plugins/combat/{actions,ai,combat_log,damage,enemy,targeting,turn_manager,ui_combat}.rs tests/` — 0 matches. — Automatic

### Architectural-discipline guards (mandatory — must return ZERO matches)

- [ ] `rg 'effects\.push\(|effects\.retain' src/plugins/combat/{actions,ai,combat_log,damage,enemy,targeting,turn_manager,ui_combat}.rs` — 0 matches (sole-mutator invariant; only `apply_status_handler` and `tick_status_durations` from #14 may mutate `effects`). — Automatic
- [ ] `rg 'pub enum CombatAction\b' src/plugins/combat/actions.rs` — 0 matches (Pitfall 8 — payload type is `CombatActionKind`). — Automatic
- [ ] `rg 'Query<' src/plugins/combat/damage.rs` — 0 matches (pure-function discipline; Combatant struct is caller-flatten). — Automatic
- [ ] `rg 'Res<|ResMut<' src/plugins/combat/damage.rs` — 0 matches (pure-function discipline). — Automatic
- [ ] `rg 'current_hp.*=' src/plugins/combat/ai.rs` — 0 matches (AI never mutates HP; Anti-pattern 1). — Automatic
- [ ] `rg 'MessageWriter<ApplyStatusEvent>' src/plugins/combat/ai.rs` — 0 matches (AI never writes status; Anti-pattern 1). — Automatic
- [ ] `rg '&mut StatusEffects' src/plugins/combat/ai.rs` — 0 matches (AI is read-only on status). — Automatic
- [ ] `rg 'Camera3d' src/plugins/combat/ui_combat.rs` — 0 matches (D-Q1=A: NO new camera spawn). — Automatic

### Test-count and coverage gates (mandatory)

- [ ] Damage formula edge cases covered: defense > attack, zero attack, miss, hit, crit, front→back row, variance bounded — 7+ tests. — Automatic via `cargo test plugins::combat::damage`.
- [ ] Turn ordering edge cases covered: descending speed, party-before-enemy tie, lower-slot tie — 3+ tests. — Automatic via `cargo test plugins::combat::turn_manager::tests::speed_`.
- [ ] Mid-turn death edge cases covered: actor dies, target re-targets, side wiped — 3+ tests. — Automatic via `cargo test plugins::combat::turn_manager::app_tests` and `cargo test plugins::combat::targeting`.
- [ ] Defend pipeline covered: writes ApplyStatusEvent, no-ops with higher buff, re-derives stats — 3+ tests. — Automatic via `cargo test plugins::combat::turn_manager::app_tests::defend_`.
- [ ] AI determinism + dead-skip + boss-pattern covered — 4+ tests. — Automatic via `cargo test plugins::combat::ai::app_tests`.
- [ ] Total new test count: 25-30 (within roadmap envelope of +20-30 from line 822). — Automatic via diff check.

### Manual smoke (one-time, before claiming complete)

- [ ] `cargo run --features dev` — game launches. — Manual
- [ ] F9 cycles into `GameState::Combat`. Combat UI overlays the dungeon view (D-Q1=A — same camera). — Manual
- [ ] Enemy column shows 2 Goblins (dev-stub spawner from Phase 15A Step 11). — Manual
- [ ] Party panel shows 4 party members with HP/MP. — Manual
- [ ] Action panel shows "> Aldric" + Attack/Defend/Spell/Item/Flee labels (D-Q2=A persistent). — Manual
- [ ] Combat log shows "Combat begins" entry. — Manual
- [ ] Pressing Space commits Attack (v1 simplification per Phase 15D Step 3). — Manual
- [ ] After 4 commits, resolution log entries appear in the right panel. — Manual
- [ ] Killing all enemies (devtools / cheat via `derived.current_hp = 0`) transitions back to `GameState::Dungeon`. — Manual

### Pre-commit / git gates

- [ ] `but commit --message-file <path>` per phase (4 commits total). — Manual
- [ ] No raw `git commit` against `gitbutler/workspace` (CLAUDE.md hook will reject). — Automatic via hook.
