# Feature #15 — Turn-Based Combat Core — Research

**Researched:** 2026-05-08
**Domain:** Druum / Bevy 0.18.1 / DRPG action-queue combat resolution layer
**Confidence:** HIGH on every "what already exists" claim (each cited at file:line, all from the merged tree). HIGH on the canonical Bevy 0.18 patterns reused here (every shape is mirrored from a working precedent: `MovedEvent` / `EquipmentChangedEvent` / `ApplyStatusEvent` / `apply_status_handler` / `recompute_derived_stats_on_equipment_change` / `apply_pit_trap`). HIGH on `CombatPhase` already being declared as a sub-state, on `CombatAction` already being declared as a leafwing `Actionlike`, on the egui-paint precedent (`paint_minimap_overlay` / `paint_minimap_full`), on the test-harness precedent (`features.rs::app_tests::make_test_app`). HIGH on the recommended `TurnActionQueue` / `PlayerInputState` shape and the action-resolver decomposition. MEDIUM on the AI emission pattern (mirrors a precedent but #15 is the first system that *generates* `CombatAction` outside leafwing input). MEDIUM-LOW on the `rand` crate gate — `rand 0.9.4` is in `Cargo.lock` transitively but **NOT** a direct dep; adding it requires the Step A/B/C gate. Surface as a planner call, not a research recommendation.

---

## Executive summary

Feature #15 lands on top of an unusually well-prepared foundation:

1. **`CombatPhase` already exists as a `SubStates` enum** (`state/mod.rs:28-36`) with the four variants the roadmap asks for — `PlayerInput`, `ExecuteActions`, `EnemyTurn`, `TurnResult`. It is registered in `StatePlugin::build` (`state/mod.rs:53-56`) and source-gated to `GameState::Combat`. **#15 has zero new state-enum work**; it ships systems gated `.run_if(in_state(CombatPhase::X))`.
2. **`CombatAction` already exists as a leafwing `Actionlike`** (`input/mod.rs:90-98`) with six menu-navigation variants (`Up/Down/Left/Right/Confirm/Cancel`). The default keymap binds them to arrows + WASD + Enter/Space + Escape (`input/mod.rs:225-239`). **The leafwing enum is for MENU NAVIGATION** — it is NOT the same as the recommended `CombatAction` enum that #15 needs (the action-queue payload). To avoid name collision the recommendation in §Architecture below is to call the queue payload `CombatActionKind` (or similar) and keep the leafwing `CombatAction` as the input-direction enum.
3. **`StatusEffectsPlugin` is registered as a sub-plugin of `CombatPlugin`** (`combat/mod.rs:18`) and the `combat/` directory already contains `status_effects.rs`. The roadmap path `src/plugins/combat/{turn_manager.rs, actions.rs, damage.rs, ai.rs, ui_combat.rs}` (line 808) is the natural extension.
4. **`ApplyStatusEvent` is the canonical "apply effect" message** (`status_effects.rs:79-85`) and `apply_status_handler` enforces stacking. **`Defend` should write `ApplyStatusEvent { effect: DefenseUp, potency: 0.5, duration: Some(1) }`** — NOT a parallel `Defending: Component`. Buffs reuse the existing pipeline; no new state.
5. **`#14`'s `check_dead_and_apply` stub** (`status_effects.rs:394-407`) is the canonical "apply Dead on zero HP" hook. **#15 imports and calls it** after every damage write — the contract is documented at `status_effects.rs:387-393`.
6. **`recompute_derived_stats_on_equipment_change`** (`inventory.rs:434-494`) reads `&StatusEffects` and re-runs `derive_stats`. **`Defend → DefenseUp` flows through this pipeline automatically** because `apply_status_handler` writes `EquipmentChangedEvent { slot: EquipSlot::None }` for stat-affecting variants (`status_effects.rs:222-233`). Zero parallel re-derive logic in #15.
7. **`bevy_egui 0.39.1` is wired through `UiPlugin`** (`ui/mod.rs:13-27`) with `auto_create_primary_context: false`. The `MinimapPlugin::attach_egui_to_dungeon_camera` precedent shows the per-plugin attach pattern. **#15's `ui_combat.rs` mirrors this**: attach `PrimaryEguiContext` to the combat camera (or reuse the dungeon camera if combat happens in-place — see Open Questions D-Q1).
8. **`rand` is in `Cargo.lock` transitively (`rand 0.9.4`) but NOT a direct dep.** Adding `rand` to `[dependencies]` requires the Step A/B/C verification gate per the Druum convention (`feedback_third_party_crate_step_a_b_c_pattern.md`). Bevy 0.18 also ships `bevy_math::sampling::WeightedAliasIndex` — but for a deterministic, seeded RNG, `rand_chacha` (already transitive) is the canonical choice.

The recommended architecture is:

- **Five new files** under `src/plugins/combat/`: `turn_manager.rs`, `actions.rs`, `damage.rs`, `ai.rs`, `ui_combat.rs`. Plus three shared support modules: `enemy.rs` (the `Enemy` ECS components — what #16 will spawn into), `targeting.rs` (the resolver and re-target-on-death edge case), `combat_log.rs` (the bounded ring buffer).
- **A `CombatActionKind` enum** as the queue payload (named to avoid the leafwing `CombatAction` collision), wrapped in a `QueuedAction { actor, kind, target, speed_at_queue_time }` struct.
- **A `TurnActionQueue: Resource(Vec<QueuedAction>)`** sorted by speed descending (high speed first) on transition from `PlayerInput` → `ExecuteActions`.
- **A `PlayerInputState: Resource`** holding which slot is currently choosing (`Option<usize>`), what menu is open (`Option<MenuStack>`), what actions have been collected so far (`Vec<QueuedAction>`).
- **A pure `damage_calc(...)`** function in `damage.rs` returning a `DamageResult { damage: u32, hit: bool, critical: bool, message: String }`.
- **A boundary system** `enemy_ai_action_select` reading `&Enemy` + `&AiBrain` + alive party + RNG, writing into `TurnActionQueue` — mirrors the `apply_poison_trap` precedent of "system writes message; handler resolves".
- **The damage pipeline** is a strict layering: `attacker stats` → `defender stats` → `weapon` → `action modifiers` → `row modifiers` → `crit roll` → `result`. **Row rules live in `damage.rs` only** (single owner — research §Pitfall 5 of roadmap).
- **The egui combat screen** uses `egui::CentralPanel` for the enemy column + party panel layout, modeled after `paint_minimap_full` (`minimap.rs:308-336`). Action menus open as `egui::Window` with `egui::Area` for absolute positioning of the target-selection submenu.
- **Decoupling**: AI just emits `CombatAction`s into the queue (no inline state mutation). UI just calls into target-selection state (no direct queue mutation). Damage is pure (no entity lookups). This is the seam-discipline the roadmap warns about (line 858: *"Every combat bug you ever ship will live at the seams"*).

Total scope: **+5 enum variants** (`CombatActionKind` payload variants — `Attack`, `Defend`, `CastSpell { spell_id: String }` (stub), `UseItem { item: Handle<ItemAsset> }`, `Flee`), **+1 leafwing enum extension** (`CombatAction::OpenMenu` and `CombatAction::Menu1..Menu5` — TBD with planner; the existing 6-variant enum may be enough), **+5 new files** plus **+3 support modules** (~1200-1700 LOC total — within the +1000-1800 envelope from roadmap line 818), **+1-2 deps** (`rand` direct + maybe `rand_chacha` for seeded RNG; both already transitive — Step A/B/C gate required), **+25-30 tests** (within +20-30 envelope from line 822). Cargo.toml: 1-2 lines added. **Defers**: full spell mechanics (#20), encounter spawning (#16), animation tweens (#17), boss-AI scripted patterns (planner call — see Open Questions).

**Primary recommendation:** Implement #15 across **four sequential sub-PRs** mirroring the roadmap's note at line 824 (*"Plan it as multiple sub-PRs (turn manager → damage → AI → UI) rather than one monolithic change"*):

- **Sub-PR 15A — Turn Manager + State Machine** (~350 LOC): `turn_manager.rs`, `actions.rs`, `enemy.rs`, the `TurnActionQueue` / `PlayerInputState` resources, the `CombatActionKind` enum, the `CurrentEncounter` *consumer* contract (no spawner — that's #16), the phase-transition systems. NO ai.rs, NO damage.rs, NO ui_combat.rs in this PR. Tests: queue ordering, phase transitions, `Defend → DefenseUp` integration, victory/defeat detection, `Flee` success/fail.
- **Sub-PR 15B — Damage** (~250 LOC): `damage.rs` + `targeting.rs`. Pure `damage_calc(...)`, `DamageResult`, `resolve_target_with_fallback(...)`. Tests: every damage edge case (defense > attack, criticals, 0-HP, row modifiers, back-row weapon range, etc.), target re-resolution.
- **Sub-PR 15C — AI** (~200 LOC): `ai.rs` + the `EnemyAi` enum + `random_target_attack` + the boss-AI stub (planner call — see Open Questions). Tests: deterministic behavior with seeded RNG, target selection.
- **Sub-PR 15D — UI** (~400-600 LOC): `ui_combat.rs` + `combat_log.rs`. egui screen layout, action menu, target selection, combat log. Tests: layout smoke tests, log truncation; manual smoke for visual fidelity.

**Decisions surfaced for the planner to ask the user about** (these are runtime-feel and design-philosophy calls):

1. **Damage formula choice** (Wizardry-style multiplicative defense vs. Etrian-style subtractive vs. custom hybrid). See §Damage Formulas.
2. **Action menu UX** (persistent action panel vs. modal pop-up per slot). See §UI Architecture.
3. **Combat log shape** (bounded ring buffer with size N vs. unbounded `Vec` cleared on combat end). See §Combat Log.
4. **`Defend` stacking with existing `DefenseUp`** (stack magnitude / refresh duration / ignore if active). See §Defend Integration.
5. **Boss AI scope** (ship `BossAi` enum stub with one or two scripted patterns vs. pure `random_target_attack` with `BossAi` deferred). See §AI Architecture.

---

## Live ground truth (the planner must mirror these)

These are the load-bearing facts from the merged code that contradict, refine, or pre-empt the roadmap. Read these before designing anything.

### A. `CombatPhase` is already a `SubStates` enum — #15 wires it, doesn't define it

**File:** `/Users/nousunio/Repos/Learnings/claude-code/druum/src/plugins/state/mod.rs:28-36`

```rust
#[derive(SubStates, Default, Debug, Clone, PartialEq, Eq, Hash)]
#[source(GameState = GameState::Combat)]
pub enum CombatPhase {
    #[default]
    PlayerInput,
    ExecuteActions,
    EnemyTurn,
    TurnResult,
}
```

Registered at `state/mod.rs:53-56`:

```rust
app.init_state::<GameState>()
    .add_sub_state::<DungeonSubState>()
    .add_sub_state::<CombatPhase>()
    .add_sub_state::<TownLocation>()
```

**Impact for #15:** `CombatPhase` is **frozen FROM #2** — do not modify the enum. The `EnemyTurn` variant in the existing enum is **vestigial relative to the action-queue design** (the roadmap line 794 specifies a single `ExecuteActions` phase that resolves both player and enemy actions in interleaved speed order). **Two options surface:**

- **Use the existing 4-phase enum verbatim:** `PlayerInput` → `ExecuteActions` → `EnemyTurn` → `TurnResult` → loop. Treat `EnemyTurn` as "post-execute resolution + AI selection for next round" and `ExecuteActions` as "drain the queue". This contradicts the action-queue pattern (which interleaves enemies into the same execution phase).
- **Use 3 phases of the existing 4** (skip `EnemyTurn`): `PlayerInput` → `ExecuteActions` → `TurnResult` → `PlayerInput`. Treat `EnemyTurn` as a phase that exists in the enum but is **never entered**. Mark with a comment.

**Recommendation: D-A2 (skip `EnemyTurn`).** The action-queue pattern from research Pattern 5 (roadmap line 795) explicitly interleaves enemy actions into a single `ExecuteActions` phase sorted by speed. A separate `EnemyTurn` phase doesn't match the design and would re-introduce the "player goes, then enemy goes" dichotomy the action-queue replaces. Document the unused variant; do not delete it (frozen by #2). Future #15 evolution can add `RoundEnd` or similar if needed.

The roadmap acknowledges this asymmetry implicitly at line 794-795 ("collect each party member's chosen action through `CombatPhase::PlayerInput`, append enemy AI actions, sort by `speed`, then `CombatPhase::ExecuteActions` resolves them one by one") — `EnemyTurn` doesn't feature in the spec sentence.

### B. `CombatAction` is already a leafwing `Actionlike` — for MENU NAVIGATION, not the action-queue payload

**File:** `/Users/nousunio/Repos/Learnings/claude-code/druum/src/plugins/input/mod.rs:90-98`

```rust
/// Turn-based combat menu navigation. Used in
/// `GameState::Combat + CombatPhase::PlayerInput`. The action enum is
/// defined here; the systems that consume it land in Feature #15.
#[derive(Actionlike, PartialEq, Eq, Hash, Clone, Copy, Debug, Reflect)]
pub enum CombatAction {
    Up,
    Down,
    Left,
    Right,
    Confirm,
    Cancel,
}
```

Default keymap at `input/mod.rs:225-239`:

```rust
fn default_combat_input_map() -> InputMap<CombatAction> {
    use CombatAction::*;
    InputMap::default()
        .with(Up, KeyCode::ArrowUp).with(Up, KeyCode::KeyW)
        .with(Down, KeyCode::ArrowDown).with(Down, KeyCode::KeyS)
        .with(Left, KeyCode::ArrowLeft).with(Left, KeyCode::KeyA)
        .with(Right, KeyCode::ArrowRight).with(Right, KeyCode::KeyD)
        .with(Confirm, KeyCode::Enter).with(Confirm, KeyCode::Space)
        .with(Cancel, KeyCode::Escape)
}
```

**Critical for #15:** This is **menu navigation**, not the queued action payload. **Two distinct types exist in the design:**

- `CombatAction` (existing, leafwing) — keyboard input direction for menu navigation.
- A NEW type that is the queue payload — **rename to `CombatActionKind`** to avoid collision. This holds `Attack { target: Entity }`, `Defend`, `CastSpell { spell_id: String, target: Entity }`, `UseItem { item: Handle<ItemAsset>, target: Entity }`, `Flee`.

**The `CombatAction` enum (leafwing) is FROZEN from #5.** Adding hotkey aliases (e.g., `KeyCode::KeyA` already maps to `Left`; binding `KeyCode::Digit1` for "first action button" would help menu UX) is a planner call. If extension is required, follow the same Step A/B/C verification — but #5 already locks the enum, so adding variants requires a `#5 → #15` carve-out edit.

**Recommended: Do NOT extend `CombatAction` for #15.** The 6 directional + Confirm + Cancel verbs are sufficient for an arrow-driven menu. Number-key shortcuts (1=Attack, 2=Defend, etc.) can be deferred to a polish pass. See Open Questions D-Q3.

### C. `combat/` directory and `CombatPlugin` exist; `StatusEffectsPlugin` is a sub-plugin

**File:** `/Users/nousunio/Repos/Learnings/claude-code/druum/src/plugins/combat/mod.rs`

```rust
pub struct CombatPlugin;

impl Plugin for CombatPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(status_effects::StatusEffectsPlugin)
            .add_systems(OnEnter(GameState::Combat), || info!("Entered GameState::Combat"))
            .add_systems(OnExit(GameState::Combat), || info!("Exited GameState::Combat"));
    }
}
```

**Pattern for #15:** `CombatPlugin` is the umbrella. New sub-plugins for #15:

- `TurnManagerPlugin` — owns `CombatActionKind`, `TurnActionQueue`, `PlayerInputState`, the phase-transition systems.
- `CombatUiPlugin` — owns the egui paint systems for the combat screen (registered in `EguiPrimaryContextPass` schedule per the minimap precedent).
- `EnemyAiPlugin` — owns `EnemyAi`, `random_target_attack`, the AI emit-action system.

Each sub-plugin is registered in `CombatPlugin::build` via `app.add_plugins(...)`. **`main.rs` is unchanged.** This matches the precedent: `StatusEffectsPlugin` registered as a sub-plugin from inside `CombatPlugin::build` (see `combat/mod.rs:18`).

**Alternative: single `CombatPlugin` with all systems registered directly.** Cleaner for small scope, but #15 spans 4 sub-PRs; sub-plugins help isolate test harnesses (each sub-PR's tests can register only the sub-plugin under test, not the entire combat tower). **Recommendation: sub-plugin shape** — same rationale as #14's `StatusEffectsPlugin`.

### D. `ApplyStatusEvent` + `apply_status_handler` is the canonical "apply effect" pipeline — `Defend` must reuse it

**File:** `/Users/nousunio/Repos/Learnings/claude-code/druum/src/plugins/combat/status_effects.rs:79-85`, `177-235`

The `Defend` action in #15 should produce a status effect, not a parallel `Defending: Component` flag. The recommended shape:

```rust
// In execute_combat_actions (#15's resolver):
match action.kind {
    CombatActionKind::Defend => {
        apply_status.write(ApplyStatusEvent {
            target: action.actor,
            effect: StatusEffectType::DefenseUp,
            potency: 0.5,         // +50% defense — see Defend Integration §
            duration: Some(1),    // expires at end of next round (TBD — see Open Questions D-Q4)
        });
    }
    // ... other arms
}
```

**Why this is the right call:**

1. `recompute_derived_stats_on_equipment_change` (`inventory.rs:434-494`) already reads `&StatusEffects` and re-runs `derive_stats` — which includes the `DefenseUp` buff branch (`character.rs:447-449`). Zero new re-derive logic in #15.
2. `apply_status_handler` enforces stacking, NaN-clamps potency, and writes `EquipmentChangedEvent { slot: EquipSlot::None }` for `DefenseUp` (`status_effects.rs:222-233`) — the dual-use sentinel that triggers re-derive via the existing pipeline.
3. `tick_status_durations` (`status_effects.rs:243-294`) already removes the effect after duration expires AND re-fires `EquipmentChangedEvent` to drop the buff from `derive_stats` output.

**The implementer must NOT introduce a `Defending: Component` or any parallel state.** This is the seam-discipline the roadmap warns about (line 858).

**Where the `Defend → DefenseUp` writer lives:** Inside `execute_combat_actions` in `turn_manager.rs`. This system needs `MessageWriter<ApplyStatusEvent>` as a system parameter and is registered with `.before(apply_status_handler)` per the `apply_poison_trap` precedent (`features.rs::CellFeaturesPlugin::build`).

### E. `check_dead_and_apply` is a #15-callable stub from #14

**File:** `/Users/nousunio/Repos/Learnings/claude-code/druum/src/plugins/combat/status_effects.rs:387-407`

```rust
/// #15-callable convenience: writes `ApplyStatusEvent { effect: Dead, ... }`
/// when `derived.current_hp == 0`. #14 does NOT auto-apply Dead inside
/// `apply_poison_damage` (Pitfall 7 — defer combat genre rules to #15).
/// #15's combat resolver imports and calls this after damage resolves.
pub fn check_dead_and_apply(
    target: Entity,
    derived: &DerivedStats,
    writer: &mut MessageWriter<ApplyStatusEvent>,
) {
    if derived.current_hp == 0 {
        writer.write(ApplyStatusEvent {
            target,
            effect: StatusEffectType::Dead,
            potency: 1.0,
            duration: None, // permanent
        });
    }
}
```

**#15 calls `check_dead_and_apply` immediately after each damage write inside `execute_combat_actions`.** The shape:

```rust
fn execute_combat_actions(
    /* ... */
    mut apply_status: MessageWriter<ApplyStatusEvent>,
    /* ... */
) {
    // ... apply damage ...
    derived.current_hp = derived.current_hp.saturating_sub(result.damage);
    check_dead_and_apply(target_entity, &derived, &mut apply_status);
}
```

**Loaded behavior:** `check_dead_and_apply` writes the message; `apply_status_handler` reads and pushes `Dead` next frame. The system ordering: `execute_combat_actions.before(apply_status_handler)` is required so the Dead application is visible in the same `ExecuteActions` phase. **The damage pipeline must NOT manually push `Dead` into `StatusEffects.effects` — that bypasses `apply_status_handler` and breaks the sole-mutator invariant.**

### F. `recompute_derived_stats_on_equipment_change` is the dual-use re-derive pipeline

**File:** `/Users/nousunio/Repos/Learnings/claude-code/druum/src/plugins/party/inventory.rs:434-494`

Already established by #14: `apply_status_handler` writes `EquipmentChangedEvent { character, slot: EquipSlot::None }` for `AttackUp/DefenseUp/SpeedUp/Dead`. The recompute system re-runs `derive_stats` and clamps `current_hp/current_mp` to the new max (handling the Dead → max_hp = 0 case).

**For #15:** `Defend → DefenseUp` flows through this verbatim. `Attack`-buff trigger from a #15 spell or item flows through this verbatim. **Zero new code needed** — just write `ApplyStatusEvent` and the pipeline does the rest.

**Side effect to be aware of:** When `Dead` is applied:
1. `apply_status_handler` writes `EquipmentChangedEvent`.
2. `recompute_derived_stats_on_equipment_change` re-runs `derive_stats` — `derive_stats` zeros `max_hp/max_mp`.
3. Caller-clamp at `inventory.rs:491-492` clamps `current_hp = old_current_hp.min(0) = 0`.

**This is correct and intentional.** A character at 0 HP gets `Dead`, gets re-derived with `max_hp = 0`, and `current_hp` stays 0. The combat-log message ("Aldric falls!") fires from the `execute_combat_actions` resolver, BEFORE the `Dead` event is processed — so the log shows the death at the moment damage hit, not one frame later.

### G. `bevy_egui 0.39.1` is wired through `UiPlugin` with manual context attach

**File:** `/Users/nousunio/Repos/Learnings/claude-code/druum/src/plugins/ui/mod.rs:13-27`, `/Users/nousunio/Repos/Learnings/claude-code/druum/src/plugins/ui/minimap.rs:266-336`

```rust
// ui/mod.rs
.insert_resource(EguiGlobalSettings {
    auto_create_primary_context: false,
    ..default()
})
```

The minimap precedent is the model. Painters run in `EguiPrimaryContextPass` schedule, NOT `Update`. They take `EguiContexts` and call `contexts.ctx_mut()?` to obtain the context. They fail gracefully (early return on `Err`) if the context isn't attached.

**For #15:** `ui_combat.rs` mirrors this — paints to the combat camera's egui context. **The combat camera is a planner question (Open Question D-Q1):** does combat happen on the dungeon camera (party stays in the dungeon, enemies overlay) or on a dedicated combat camera (separate scene)? If dungeon camera: piggyback on `MinimapPlugin::attach_egui_to_dungeon_camera`. If dedicated: spawn a new camera in `OnEnter(GameState::Combat)` and attach `PrimaryEguiContext` to it; despawn on `OnExit`.

**Recommendation: combat happens on the dungeon camera (overlay).** Wizardry/Etrian convention: the first-person view stays; enemies appear in the foreground; menus overlay. This is also the simplest implementation. Defer dedicated combat scene to a future feature if needed (cinematic angles, etc.).

### H. `paint_minimap_overlay` is the egui-paint precedent — `egui::Area` for absolute positioning, `egui::CentralPanel` for full-screen

**File:** `/Users/nousunio/Repos/Learnings/claude-code/druum/src/plugins/ui/minimap.rs:266-336`

The minimap shows two precedent shapes:

- **Overlay** (`egui::Area::new(...).anchor(...).show(ctx, |ui| { ... })`) — absolute-positioned hardcoded-size element.
- **Full-screen** (`egui::CentralPanel::default().frame(...).show(ctx, |ui| { ... })`) — fills the rest of the screen.

For combat:
- **Party HP/MP bar panel** at the bottom: `egui::TopBottomPanel::bottom("party_panel").show(ctx, |ui| { ... })`.
- **Enemy column** down the left: `egui::SidePanel::left("enemy_column").show(ctx, |ui| { ... })`.
- **Action menu** popup: `egui::Window::new("action_menu").anchor(...).show(ctx, |ui| { ... })`.
- **Combat log** along the right: `egui::SidePanel::right("combat_log").show(ctx, |ui| { ... })`.
- **Target selection overlay**: `egui::Area::new("target_selection").anchor(Center, ...).show(ctx, |ui| { ... })`.

The combat-screen layout is detailed in §UI Architecture.

### I. Test-harness precedent — `features.rs::app_tests::make_test_app` (`features.rs:736-783`) is the shape #15 mirrors

The pattern:

```rust
fn make_test_app() -> App {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        AssetPlugin::default(),
        StatesPlugin,
        InputPlugin,
        StatePlugin,
        DungeonPlugin,             // optional for combat tests
        CellFeaturesPlugin,        // optional
        CombatPlugin,              // brings StatusEffectsPlugin
        PartyPlugin,
    ));
    app.init_resource::<ActionState<DungeonAction>>();
    app.init_resource::<ActionState<CombatAction>>();  // for #15 menu nav
    app.init_asset::<DungeonFloor>();
    app.init_asset::<crate::data::ItemDb>();
    app.init_asset::<bevy::prelude::Mesh>();
    app.init_asset::<bevy::pbr::StandardMaterial>();
    app.add_message::<SfxRequest>();
    #[cfg(feature = "dev")]
    app.init_resource::<bevy::input::ButtonInput<bevy::prelude::KeyCode>>();
    app
}
```

**For #15:** Add `app.init_resource::<TurnActionQueue>()`, `app.init_resource::<PlayerInputState>()`, `app.init_resource::<CurrentEncounter>()` (created in `OnEnter(Combat)` normally — for tests, insert directly with a fixture). The test harness pattern is established; #15 just adds the new resources.

For tests that exercise the AI, use a deterministic seeded RNG (see §AI Architecture). For tests that exercise damage formulas, no app needed — `damage_calc(...)` is a pure function (Layer 1 test pattern from #11/#14).

### J. The leafwing test harness IS NOT the same as the bypass harness — see `feedback_bevy_input_test_layers.md`

Per memory: tests that read `Res<ActionState<CombatAction>>` and need the actual leafwing tick must use **full `InputPlugin` + `KeyboardInput` message injection**, NOT the bare `init_resource::<ActionState<CombatAction>>` bypass. The bypass pattern (`features.rs:763`) works for `.press()` calls in tests because no `InputManagerPlugin` is registered to clear `just_pressed` in PreUpdate. **#15's combat menu tests can use either layer**:

- **Layer 1 — direct `ActionState.press()`:** init_resource bypass, no leafwing tick. Fastest; works for testing menu-state transitions in isolation.
- **Layer 2 — full `KeyboardInput` injection:** `MinimalPlugins + InputPlugin + ActionsPlugin + ...`. For end-to-end tests covering "press W, see ActionState reflect MoveForward, see PlayerInputState change accordingly".

**Recommendation: Layer 1 for menu transition unit tests. Layer 2 for one or two end-to-end combat smoke tests.** Same split as #5/#10.

### K. `CurrentEncounter: Resource` is owned by #16 — #15 consumes it

**Roadmap line 868** (Feature #16): *"populate a `CurrentEncounter` resource with the spawned enemies"*

**For #15:** `CurrentEncounter` does NOT exist yet. **#15 must NOT define it as production code** — that's #16's responsibility. **#15 defines it as a TEST FIXTURE only**, and at the production-level publishes a contract (a doc comment in `turn_manager.rs` describing the expected shape). The recommended shape:

```rust
// Contract — full type lives in #16. #15 references this shape.
#[derive(Resource, Debug, Clone)]
pub struct CurrentEncounter {
    pub enemy_entities: Vec<Entity>,
    pub fleeable: bool,
}
```

**For tests in #15**, define `CurrentEncounter` as a test-fixture resource inserted directly. **Phase ordering**: #15 ships first; #16 lands after; #16 then introduces the production `CurrentEncounter` and #15's reads switch to it. **This pattern was successfully used by #14 → #15** (the `StatusTickEvent` combat-round emitter contract is documented in `status_effects.rs:91` and #15 wires it).

**Stub spawn path:** For #15's manual smoke tests and dev-cycler exercise, ship a `dev`-feature-only system that spawns 2-3 placeholder `Enemy` entities on `OnEnter(GameState::Combat)`. This is throwaway code that #16 deletes. Same pattern as `spawn_default_debug_party` (`party/mod.rs:88-126`).

### L. `Enemy` ECS components don't exist — #15 introduces them (minimal); #17 fleshes out

**File:** `src/data/enemies.rs:1-12` is a 12-line stub:

```rust
#[derive(Asset, Reflect, Serialize, Deserialize, Default, Debug, Clone)]
pub struct EnemyDb {
    // Empty body for Feature #3.
}
```

**For #15:** Introduce a minimal `Enemy` ECS shape — enough to support combat resolution, deferring full enemy database to #17:

```rust
// src/plugins/combat/enemy.rs
#[derive(Component, Reflect, Default, Debug, Clone)]
pub struct Enemy;

#[derive(Component, Reflect, Default, Debug, Clone)]
pub struct EnemyName(pub String);

// Reuses CharacterStats, DerivedStats, StatusEffects from party
// (PartyMember marker is the discriminator — Enemy is the inverse).

#[derive(Component, Reflect, Default, Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnemyAi {
    #[default]
    RandomAttack,
    // BossAi { pattern_id: u32 } — see Open Questions D-Q5
}

#[derive(Bundle, Default)]
pub struct EnemyBundle {
    pub marker: Enemy,
    pub name: EnemyName,
    pub base_stats: BaseStats,         // reuses #11 type
    pub derived_stats: DerivedStats,   // reuses #11 type
    pub status_effects: StatusEffects, // reuses #11 type
    pub ai: EnemyAi,
}
```

**Critical: `PartyMember` marker is the discriminator.** Queries that should target only the party use `Query<..., With<PartyMember>>`. Queries that target only enemies use `Query<..., (With<Enemy>, Without<PartyMember>)>`. This avoids a marker explosion while letting the existing `recompute_derived_stats_on_equipment_change` (which filters `With<PartyMember>`) ignore enemies — they don't equip items.

**Alternative: separate `EnemyDerivedStats` / `EnemyBaseStats`.** Avoids reusing the party types. Cost: every system that operates on "any combatant" needs to handle two type families. **Recommendation: REUSE the party types** with `Enemy`/`PartyMember` markers as filters.

**`recompute_derived_stats_on_equipment_change` filter audit:** Currently `With<PartyMember>` (`inventory.rs:445`). Enemies have no `Equipment` (no `equip_item` calls), so they never write `EquipmentChangedEvent`. **No edit needed.** The dual-use of `EquipmentChangedEvent` for status changes (#14, D5α): if an enemy has `DefenseUp` applied to it, `apply_status_handler` writes `EquipmentChangedEvent { character: enemy_entity, slot: EquipSlot::None }` (`status_effects.rs:229-232`) — but `recompute_derived_stats_on_equipment_change` filters `With<PartyMember>` and won't process the enemy (`inventory.rs:449-453` continues silently).

**This is a #15 bug-in-waiting.** Enemy buffs/debuffs would not re-derive. Two fixes:

- **D-K1**: Drop `With<PartyMember>` from `recompute_derived_stats_on_equipment_change`'s query. Costs: enemies must have `BaseStats + Equipment + StatusEffects + Experience + DerivedStats` for the query to match — `Experience` is wasteful but cheap. Equipment can be `Equipment::default()` (empty slots). The recompute path runs once per status change but is cheap.
- **D-K2**: Define a SECOND recompute system for enemies — copies/adapts the party one. Adds ~50 LOC; enemies get a slimmer query (no `Equipment` flatten needed).

**Recommendation: D-K1 (drop the filter)** with a doc-comment explaining the dual-use. Enemy entities spawn with empty `Equipment` and `Experience::default()`. The cost is one query-shape change; benefit is "buffs/debuffs work for everyone with one code path".

The planner should call this out as a load-bearing scope decision (NOT user pick — internal architectural call). See Decisions §D-K1.

### M. Test placement and patterns

For each new file, an inline `#[cfg(test)] mod tests` follows the `features.rs::tests` and `status_effects.rs::tests` precedents:
- **Layer 1 (no `App`)** for pure functions: `damage_calc`, `resolve_target_with_fallback`, `sort_by_speed`, `select_random_target_seeded`. These are sub-millisecond unit tests.
- **Layer 2 (`App`-driven)** for systems-with-resources: `execute_combat_actions`, `enemy_ai_action_select`, the phase transitions. Use the `make_test_app` shape from `features.rs::app_tests`.
- **Layer 3 (integration tests in `tests/`)** for cross-plugin flows. **Probably not needed for #15** — Layer 2 is sufficient because `combat/` is the bottom of the dependency tree (only depends on party/dungeon).

### N. `rand` is in Cargo.lock transitively but NOT a direct dep

Cargo.lock contains `rand 0.9.4` transitively (likely via `bevy_internal` for entity ID generation, or via `winit`). To use `rand` directly in `src/plugins/combat/ai.rs`, **#15 must add it to `Cargo.toml`** — this triggers the **Step A/B/C verification gate** (per `feedback_third_party_crate_step_a_b_c_pattern.md`):

- **Step A:** `cargo add rand --dry-run` — verify the resolved version (likely 0.9.x), check it accepts the project's pinned bevy 0.18.1 (rand has zero bevy dep, so trivially compatible).
- **Step B:** Audit `[features]` — `default = ["std", "std_rng"]`. Probably keep defaults; the project is std.
- **Step C:** Grep API — `rand::rngs::ChaCha8Rng` (deterministic, seedable), `rand::seq::SliceRandom::choose`, `rand::Rng::gen_range`.

**Alternative: `rand_chacha 0.9.0`** (also already transitive) — gives you `ChaCha8Rng` directly without the `rand` umbrella. Smaller surface; no `default-features` to audit.

**Recommendation:** Add **`rand = "0.9"`** with `default-features = false, features = ["std", "std_rng"]` (no `serde` — no need to serialize RNG state). For seeded RNG in tests, `rand::rngs::SmallRng` with a fixed seed is sufficient. **The `rand_chacha` standalone is overkill** unless save/load needs to persist RNG state mid-combat (it doesn't — combat encounters are atomic).

The planner ratifies this in Phase 0 of the implementation plan via the gate. Surface as MEDIUM-confidence pending Step A/B/C resolution.

---

## Stale-roadmap summary

| Roadmap claim (line) | Reality |
|----------------------|---------|
| Line 794: "implement the action-queue combat loop" | Correct. The action-queue model from research Pattern 5 is the recommendation. **`CombatPhase` enum already exists** with the four phase variants. |
| Line 794: `CombatPhase::PlayerInput → ExecuteActions → TurnResult` | Correct, with the `EnemyTurn` variant skipped (vestigial — see D-A2 above). |
| Line 808: `src/plugins/combat/{turn_manager.rs, actions.rs, damage.rs, ai.rs, ui_combat.rs}` | Correct file layout. **#14 adds `status_effects.rs` already**; #15 adds the other 5. Recommend 3 ADDITIONAL support modules: `enemy.rs`, `targeting.rs`, `combat_log.rs`. |
| Line 809: "`Encounter` resource (depends on #16)" | **`CurrentEncounter` does NOT exist yet.** #15 documents the contract; #16 implements the production resource. #15 ships a test fixture. |
| Line 811: "New `CombatLog: Resource` capturing recent events" | Correct. **Recommend bounded ring buffer (`VecDeque` capacity N) over unbounded `Vec`** — see Decisions D-Q3. |
| Line 829: "Implement `TurnActionQueue: Resource` and `PlayerInputState: Resource`" | Correct. Recommended shapes in §Architecture. |
| Line 832: "Implement `sort_by_speed` and `execute_combat_actions` per research Pattern 5" | Correct. Sort is descending (high speed first); ties broken by `(actor_index, slot_index)` for determinism. See §Speed Tie-Breaking. |
| Line 836: "Implement `Attack`, `Defend` (sets a 1-turn `DefenseUp`-equivalent)..." | **Refinement: `Defend` writes `ApplyStatusEvent { effect: DefenseUp, potency: 0.5, duration: Some(1) }`. NOT a separate `Defending: Component`.** Reuse #14's pipeline. |
| Line 837: "Stub `CastSpell` — full implementation deferred to #20" | Correct. Stub the `CombatActionKind::CastSpell { spell_id: String, target: Entity }` variant; the resolver writes a "Spell stub: not yet implemented" combat log entry; #20 fills in. |
| Line 845: "Implement `TargetSelection` resolution (single enemy, all enemies, single ally, all allies, self)" | Correct. The `TargetingMode` enum (single/multi/self) lives in `targeting.rs`. The `resolve_target_with_fallback(...)` pure function handles re-target-on-death. |
| Line 849: "Implement a baseline `random_target_attack` AI for fodder enemies." | Correct. The `enemy_ai_action_select` system reads `EnemyAi` and emits `CombatActionKind::Attack { target: <random alive party member> }`. Seeded RNG for determinism. |
| Line 850: "Stub a `BossAI` enum so boss scripts can be added later" | **Planner call.** Either ship a `BossAi` enum stub with one or two scripted patterns now, OR keep the design pure with only `RandomAttack` and defer `BossAi` to a later feature. See Open Questions D-Q5. |
| Line 858: "Make damage as a *pure* function `(attacker_stats, defender_stats, weapon, action) -> DamageResult`" | Correct. The damage signature recommendation: `damage_calc(attacker: &Combatant, defender: &Combatant, weapon: Option<&ItemAsset>, action: CombatActionKind, rng: &mut impl Rng) -> DamageResult`. RNG is passed by reference for crit roll determinism in tests. |

---

## Standard Stack

### Core (already in deps — no Δ)

| Library | Version | Purpose | License | Maintained? | Why Standard |
|---------|---------|---------|---------|-------------|--------------|
| [bevy](https://crates.io/crates/bevy) | =0.18.1 | `Component`, `Resource`, `Message`, `Plugin`, `SubStates`, queries | MIT/Apache-2.0 | Active | Engine — pinned. |
| [bevy_egui](https://crates.io/crates/bevy_egui) | =0.39.1 | Combat UI screen, action menus, target selection, combat log | MIT/Apache-2.0 | Active | Already in deps for #10 minimap. |
| [leafwing-input-manager](https://crates.io/crates/leafwing-input-manager) | =0.20.0 | Read `ActionState<CombatAction>` for menu navigation | ISC | Active | Already in deps for #5. `CombatAction` enum defined at `input/mod.rs:90`. |
| [serde](https://crates.io/crates/serde) | 1 | `Serialize`/`Deserialize` for new types (auto-derived). | MIT/Apache-2.0 | Active | Already in deps. |
| [bevy_reflect](https://crates.io/crates/bevy_reflect) | (transitive via bevy) | `Reflect` derive on new types — auto. | MIT/Apache-2.0 | Active | Already wired. |

### NEW — Step A/B/C verification gate required

| Library | Recommended Version | Purpose | License | Maintained? | Why Standard |
|---------|---------------------|---------|---------|-------------|--------------|
| [rand](https://crates.io/crates/rand) | "0.9" (likely resolves to 0.9.4, ALREADY transitive in `Cargo.lock`) | Deterministic seeded RNG for AI target selection, crit rolls, hit/miss rolls | MIT/Apache-2.0 | Active | Industry standard for Rust RNG; the obvious choice. **Step A/B/C gate required** — feature: `default-features = false, features = ["std", "std_rng"]` to avoid pulling `serde` for RNG state (saves don't need to persist it). |

### Supporting (NOT used in #15)

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| [rand_chacha](https://crates.io/crates/rand_chacha) | "0.9" | ChaCha8/12/20 deterministic RNG. Already transitive via `rand`. | Use directly only if save/load must persist RNG state mid-combat (it doesn't — combat is atomic). |
| [rand_distr](https://crates.io/crates/rand_distr) | "0.5" | Statistical distributions (Normal, Poisson, etc.). Already transitive. | NOT needed for #15. Use if a future feature needs gaussian damage variation. |
| [bevy_kira_audio](https://crates.io/crates/bevy_kira_audio) | (not in deps) | Sound effects for combat hits | Defer to #17 sprites/animation/audio polish. |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `rand` direct dep + `SmallRng` | `bevy_math::sampling::WeightedAliasIndex` (already in Bevy) | Bevy's sampler is for weighted-table draws; doesn't give `gen_range` ergonomics. **rand is cleaner for the wide range of RNG needs (crit rolls, target selection, hit checks).** |
| `rand_chacha` standalone | `rand` umbrella with `SmallRng` | rand_chacha is overkill for atomic combat; SmallRng is faster and the seeded determinism property is the same for tests. **Recommended: rand + SmallRng.** |
| `CombatActionKind` as enum payload | `CombatActionKind` as component-bundle | Queue must serialize/sort/clone; enums are easier. Components add scheduling friction. **Recommended: enum.** Roadmap brief explicitly calls this out as a decision. |
| Speed tie-break: deterministic `(actor_index, slot_index)` | Speed tie-break: RNG | Roadmap doesn't specify; deterministic is testable. **Recommended: deterministic** — RNG ties are noise. See §Speed Tie-Breaking. |
| Damage formula: Wizardry multiplicative | Damage formula: Etrian subtractive | **PLANNER MUST ASK USER.** Pros/cons in §Damage Formulas. |
| Combat log: bounded ring (`VecDeque`, cap 50) | Combat log: unbounded `Vec` | Bounded never grows unboundedly even on crash; unbounded simplest. **Default recommendation: bounded ring buffer cap 50** — ratify with user. |
| `Defend → DefenseUp` reuses #14 status effect | `Defending: Component` flag | Single-pipeline reuse. **Recommended: status effect** (D above). |
| Boss AI: ship enum stub with 1-2 patterns | Boss AI: pure `RandomAttack` only | Stub costs ~50 LOC and gives boss-fight authoring a hook; pure `RandomAttack` is honest about the v1 scope. **PLANNER MUST ASK USER.** |
| Enemy/Player share `BaseStats`/`DerivedStats` | Enemy has separate `EnemyBaseStats`/`EnemyDerivedStats` | Shared types: drop `With<PartyMember>` from recompute filter (D-K1). Separate: extra recompute system (D-K2). **Recommended: D-K1 (shared types).** |

---

## Architecture Options

Three architectural decisions that shape #15. Two have a strong default (recommend); one is a Category-C user pick.

### D-A1 (Category B — Recommended): Sub-plugin shape under `CombatPlugin`

`CombatPlugin` registers three new sub-plugins via `add_plugins`:

- `TurnManagerPlugin` — owns `TurnActionQueue`, `PlayerInputState`, the phase transitions.
- `EnemyAiPlugin` — owns `EnemyAi` enum, AI emit-action systems.
- `CombatUiPlugin` — owns the egui paint systems.

**Pros:** Test harnesses can register only the sub-plugin under test (faster tests, fewer cascading dependencies). Mirrors `StatusEffectsPlugin` precedent (`combat/mod.rs:18`). Each sub-plugin matches one sub-PR (15A/15C/15D).

**Cons:** More plugin-registration boilerplate. Sub-plugins must be carefully ordered (`TurnManagerPlugin` before `EnemyAiPlugin` because the AI emits into the queue the manager owns — though Bevy's plugin add-order does NOT determine system run-order; system ordering is via `.before/.after`).

**Recommended:** Sub-plugin shape. Cost is trivial (~10 LOC); benefit is meaningful test isolation.

### D-A2 (Category B — Recommended): Skip `CombatPhase::EnemyTurn`

Use only 3 of the 4 declared phases: `PlayerInput → ExecuteActions → TurnResult → PlayerInput`. `EnemyTurn` is vestigial (action-queue interleaves both kinds of action in `ExecuteActions`).

**Pros:** Matches research Pattern 5 verbatim. Single resolver loop simplifies code.

**Cons:** Unused enum variant adds noise. Can't be deleted (state enum frozen by #2).

**Recommended:** Skip `EnemyTurn`. Document with a `// vestigial — see Feature #15 research D-A2` comment in `state/mod.rs` (carve-out edit, frozen-by-#2 needs to acknowledge).

### D-A3 (Category C — User pick): Damage formula

This is a load-bearing design choice that affects combat feel for the entire game. The planner MUST surface this to the user. See §Damage Formulas for full pros/cons and worked examples.

### D-A4 (Category B — Recommended): `CombatActionKind` enum payload (NOT component-bundle)

The queue payload is an enum:

```rust
#[derive(Debug, Clone, Reflect)]
pub enum CombatActionKind {
    Attack { weapon: Option<Handle<ItemAsset>> },
    Defend,
    CastSpell { spell_id: String },         // stub — #20 fills in
    UseItem { item: Handle<ItemAsset> },
    Flee,
}
```

The queue carries `QueuedAction { actor: Entity, kind: CombatActionKind, target: TargetSelection, speed_at_queue_time: u32 }`.

**Pros (vs. component-bundle):**
- Queue is a `Vec<QueuedAction>` — sorts by `speed_at_queue_time` cheaply.
- Cloneable — re-target after a death event doesn't need to spawn a new entity.
- Single match in `execute_combat_actions` is exhaustive, compile-checked.
- Pure `damage_calc` takes the `kind` by reference, no entity lookup.

**Cons:**
- Enum can't carry per-action ECS components (animation playhead, hit-counter). For #15 (no animations) this is fine; #17 may need to upgrade.

**Recommended:** Enum. Defer the upgrade to component-bundle until #17 needs it.

### D-A5 (Category B — Recommended): Drop `With<PartyMember>` from recompute query so enemy buffs work

Per L above: enemies need `BaseStats + Equipment + StatusEffects + Experience + DerivedStats` for the query to match. **Drop the `With<PartyMember>` filter from `recompute_derived_stats_on_equipment_change`** so enemies' `DefenseUp` and `Dead` re-derive correctly.

**Pros:** Single re-derive code path. Buffs work for all combatants.
**Cons:** Carve-out edit on a frozen-from-#12 file (`inventory.rs:445`). Enemies must spawn with `Equipment::default()` and `Experience::default()` — costs ~16 bytes per enemy (negligible).

**Recommended:** D-K1 (drop the filter). Doc-comment on the recompute system to explain the dual-use.

---

## Architecture Patterns

### Recommended Project Structure

```
src/plugins/combat/
├── mod.rs                 # CombatPlugin (existing) — adds 3 new sub-plugins
├── status_effects.rs      # #14 — DO NOT TOUCH except where called out below
├── turn_manager.rs        # NEW: TurnManagerPlugin, TurnActionQueue,
│                          #      PlayerInputState, phase transitions,
│                          #      execute_combat_actions
├── actions.rs             # NEW: CombatActionKind enum, QueuedAction struct,
│                          #      TargetSelection enum
├── damage.rs              # NEW: damage_calc pure fn, DamageResult, row-rule
│                          #      logic — SOLE OWNER
├── ai.rs                  # NEW: EnemyAiPlugin, EnemyAi enum,
│                          #      enemy_ai_action_select, random_target_attack
├── ui_combat.rs           # NEW: CombatUiPlugin, the egui paint systems
├── enemy.rs               # NEW: Enemy, EnemyName, EnemyBundle (minimal —
│                          #      #17 fleshes out)
├── targeting.rs           # NEW: TargetSelection enum + resolve_target_with_fallback
└── combat_log.rs          # NEW: CombatLog resource (bounded VecDeque)
```

**Why split `actions.rs` from `turn_manager.rs`:** the `CombatActionKind` enum is the queue payload — small, pure data. `turn_manager.rs` owns the systems that produce/consume the queue. Splitting keeps `turn_manager.rs` focused on systems and lets `actions.rs` stay a 100-line data file.

**Why `enemy.rs` is its own file:** the `Enemy` ECS shape is consumed by ai.rs (queries), turn_manager.rs (queue resolution), ui_combat.rs (rendering enemy column), and damage.rs (defender stats). Centralizing the type definition keeps the consumers from coupling.

**Why `targeting.rs` is split from `actions.rs`:** the re-target-on-death logic is non-trivial (~30-50 LOC). Keeping it in `actions.rs` would bloat the data file. Consumers in `turn_manager.rs` and `ai.rs` import the resolver.

**Why `combat_log.rs` is its own file:** the resource has its own ring-buffer logic, a `push(message: String)` API, and a planner-decision over the buffer cap. Self-contained and simple.

### Pattern 1 — Action-queue resolution (research Pattern 5)

**What:** Player and enemy actions are buffered in a single sorted queue; resolution is a strict drain.

**When to use:** Every turn — this is the core combat loop.

**Example shape:**

```rust
// src/plugins/combat/turn_manager.rs

#[derive(Resource, Default, Debug, Clone)]
pub struct TurnActionQueue {
    pub queue: Vec<QueuedAction>,
}

// Phase: PlayerInput.
fn collect_player_actions(
    actions: Res<ActionState<CombatAction>>,
    mut state: ResMut<PlayerInputState>,
    mut queue: ResMut<TurnActionQueue>,
    party: Query<(Entity, &PartySlot, &DerivedStats, &StatusEffects), With<PartyMember>>,
    mut next_phase: ResMut<NextState<CombatPhase>>,
) {
    // ... menu state machine driven by ActionState<CombatAction> ...
    // When all alive party members have committed an action:
    //   next_phase.set(CombatPhase::ExecuteActions);
}

// Phase: ExecuteActions.
fn append_enemy_actions_and_sort(
    mut queue: ResMut<TurnActionQueue>,
    enemies: Query<(Entity, &DerivedStats, &EnemyAi, &StatusEffects), (With<Enemy>, Without<PartyMember>)>,
    party: Query<Entity, With<PartyMember>>,
    mut rng: ResMut<CombatRng>,
) {
    // For each alive enemy, call AI to emit an action; append to queue.
    // Sort the WHOLE queue (player + enemy) by speed descending.
    queue.queue.sort_by(|a, b| {
        b.speed_at_queue_time.cmp(&a.speed_at_queue_time)  // descending
            .then(deterministic_tie_break(a, b))
    });
}

fn execute_combat_actions(
    mut queue: ResMut<TurnActionQueue>,
    mut characters: Query<(Entity, &mut DerivedStats, &Equipment, &StatusEffects), Or<(With<PartyMember>, With<Enemy>)>>,
    items: Res<Assets<ItemAsset>>,
    mut apply_status: MessageWriter<ApplyStatusEvent>,
    mut combat_log: ResMut<CombatLog>,
    mut rng: ResMut<CombatRng>,
    mut next_phase: ResMut<NextState<CombatPhase>>,
) {
    // Drain queue in sorted order; for each action:
    //   1. Skip if actor is Dead/Stone (status check).
    //   2. Resolve target (re-target-on-death — see targeting.rs).
    //   3. Match on action.kind: Attack/Defend/CastSpell/UseItem/Flee.
    //   4. Apply damage / status / item.
    //   5. check_dead_and_apply for the target.
    //   6. Push to combat_log.
    // After drain: clear queue, transition to TurnResult.
    next_phase.set(CombatPhase::TurnResult);
}

// Phase: TurnResult.
fn check_victory_defeat_flee(
    party: Query<&DerivedStats, With<PartyMember>>,
    enemies: Query<&DerivedStats, With<Enemy>>,
    flee_attempted: Res<FleeAttempted>,  // set by Flee action
    mut next_state_combat: ResMut<NextState<GameState>>,
    mut next_phase: ResMut<NextState<CombatPhase>>,
) {
    // If all party Dead: GameOver.
    // If all enemies Dead or current_hp == 0: return to Dungeon (or whatever state #16 hands off to).
    // If FleeAttempted::Success: return to Dungeon.
    // Otherwise: cycle back to PlayerInput.
}
```

### Pattern 2 — Pure `damage_calc` (research §Anti-Patterns: separation of concerns)

**What:** Damage is a pure function. No entity lookups, no resource reads, no scheduling — just inputs in, output out.

**When to use:** Every damage computation in #15. Future #20 spell damage reuses this. #17 critical-hit polish reuses this.

**Example signature:**

```rust
// src/plugins/combat/damage.rs

#[derive(Debug, Clone, PartialEq)]
pub struct DamageResult {
    pub damage: u32,
    pub hit: bool,
    pub critical: bool,
    pub message: String,  // pre-formatted for combat log
}

pub fn damage_calc(
    attacker: &Combatant,
    defender: &Combatant,
    weapon: Option<&ItemAsset>,
    action: &CombatActionKind,
    rng: &mut impl rand::Rng,
) -> DamageResult {
    // 1. Hit roll: rng.gen_range(0..100) < attacker.accuracy - defender.evasion.
    // 2. Damage formula (see §Damage Formulas — choice TBD by user).
    // 3. Row modifier: front-row attacker on back-row defender =
    //    weapon.range == Melee → damage = 0 ("out of reach").
    //    Otherwise: full damage.
    // 4. Crit roll: rng.gen_range(0..100) < attacker.luck * 2 → 1.5x damage.
    // 5. Defender DefenseUp / AttackUp from StatusEffects already in attacker.attack
    //    and defender.defense (derive_stats applied them).
    // 6. Format message: "Aldric strikes Goblin for 12 damage."
    DamageResult { /* ... */ }
}

#[derive(Debug, Clone)]
pub struct Combatant {
    pub name: String,
    pub stats: DerivedStats,        // already buff-adjusted
    pub row: PartyRow,              // Front/Back
    pub status: StatusEffects,
}
```

**Key points:**
- Caller flattens `(Entity, Query<...>)` into `Combatant` before calling — same pattern as `derive_stats(base, equip_stats, status, level)` (caller flattens equipment).
- `rng: &mut impl Rng` makes the function testable with `SmallRng::seed_from_u64(...)` for deterministic outputs.
- **Row rules live HERE only.** No other module references `PartyRow::Front`/`PartyRow::Back` for damage decisions. Single owner.
- The `message` field is composed in this function so the combat log doesn't need to know about damage internals.

### Pattern 3 — Targeting with re-target-on-death

**What:** When an action is queued with `target: Entity`, the target may be dead by the time the action resolves. Re-target gracefully.

**When to use:** Every action that has a target (Attack, CastSpell, UseItem with non-self target).

**Example shape:**

```rust
// src/plugins/combat/targeting.rs

#[derive(Debug, Clone)]
pub enum TargetSelection {
    Single(Entity),           // single ally OR single enemy
    AllAllies,
    AllEnemies,
    Self_,
    None,                      // for Flee
}

/// Resolve `selection` to a list of currently-alive entities, falling
/// back to alternatives if the original target died.
///
/// **Re-target rule:**
/// - `Single(entity)` where `entity` is now Dead (or Stone): pick a random
///   live entity from the same side (party or enemy) using `same_side`.
///   If no live entity exists on that side, return empty.
/// - `AllAllies`/`AllEnemies`: filter to live entities only.
/// - `Self_`: return `actor` only if `actor` is alive (else empty).
pub fn resolve_target_with_fallback(
    selection: &TargetSelection,
    actor: Entity,
    actor_side: Side,
    party: &[Entity],          // alive party
    enemies: &[Entity],        // alive enemies
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
                .choose(rng)  // rand::seq::IteratorRandom
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side { Party, Enemy }
```

**Edge cases tested:**
- Original target dies before action resolves → re-target to a random live ally on the same side.
- Original target's whole side is wiped out → return empty; the action no-ops with a "missed" log entry.
- Self-target on a Dead actor → empty (action no-ops).
- AllEnemies after a multi-target spell wipes them all → returns empty (subsequent damage no-ops).

### Pattern 4 — AI emission boundary (system writes message; resolver reads queue)

**What:** AI is purely emit-side. The resolver is purely read-side. They communicate via the `TurnActionQueue` resource — never via inline state mutation.

**Example shape:**

```rust
// src/plugins/combat/ai.rs

fn enemy_ai_action_select(
    enemies: Query<(Entity, &EnemyAi, &DerivedStats, &StatusEffects), (With<Enemy>, Without<PartyMember>)>,
    party: Query<(Entity, &DerivedStats), With<PartyMember>>,
    mut queue: ResMut<TurnActionQueue>,
    mut rng: ResMut<CombatRng>,
    current_phase: Res<State<CombatPhase>>,
) {
    if *current_phase.get() != CombatPhase::ExecuteActions { return; }

    let alive_party: Vec<Entity> = party
        .iter()
        .filter(|(_, d)| d.current_hp > 0)
        .map(|(e, _)| e)
        .collect();

    for (enemy, ai, stats, status) in &enemies {
        // Skip Dead/Stone enemies.
        if status.has(StatusEffectType::Dead) || status.has(StatusEffectType::Stone) {
            continue;
        }
        // Pick action from AI.
        let (kind, target) = match ai {
            EnemyAi::RandomAttack => {
                let target = alive_party.iter().copied().choose(&mut rng.0).unwrap_or(enemy);
                (CombatActionKind::Attack { weapon: None }, TargetSelection::Single(target))
            }
            // EnemyAi::BossAi { pattern_id } => /* see Open Questions D-Q5 */,
        };
        queue.queue.push(QueuedAction {
            actor: enemy,
            kind,
            target,
            speed_at_queue_time: stats.speed,
        });
    }
}
```

**Key boundary properties:**
- AI never reads or mutates `DerivedStats.current_hp` (no inline damage).
- AI never writes `ApplyStatusEvent` directly (only emits queue actions; the resolver writes the status).
- AI is a pure emitter — testable by inserting test fixtures and asserting queue contents.
- AI's RNG is owned in a `CombatRng: Resource(SmallRng)` so tests can deterministically seed.

### Anti-Patterns to Avoid

- **Anti-pattern 1: AI mutates HP directly.** AI must never touch `current_hp`. If it does, you have two damage paths and bugs at the seam (the roadmap's exact warning at line 858).
- **Anti-pattern 2: `Defending: Component`.** Don't add a parallel state for the Defend buff. Use `ApplyStatusEvent { effect: DefenseUp }`. (D above.)
- **Anti-pattern 3: Damage formula with side-effects.** `damage_calc` is pure. Anything that mutates entities goes in the resolver, not the formula.
- **Anti-pattern 4: Multiple owners of row-modifier rules.** Row rules live in `damage.rs` only. Don't replicate in `ai.rs` or `ui_combat.rs`.
- **Anti-pattern 5: UI mutates the queue directly.** UI calls `PlayerInputState::commit_action(...)` which is the SOLE writer that pushes to the queue from the player side. Same sole-mutator pattern as `apply_status_handler` for `StatusEffects.effects`.
- **Anti-pattern 6: Unbounded combat log growth on long fights.** Use a bounded `VecDeque` or clear at combat end. Default recommendation: bounded ring buffer cap 50 (D-Q3).

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| RNG with seedable determinism for tests | A custom xorshift | `rand::rngs::SmallRng` + `seed_from_u64(...)` | Battle-tested, idiomatic, zero overhead. |
| Random selection from a list | A manual `vec[rng.gen_range(0..vec.len())]` | `rand::seq::IteratorRandom::choose(&mut rng)` | Empty-list safe, idiomatic. |
| Sorting a queue stably with tie-break | A custom merge sort | `Vec::sort_by` with a chained `Ordering::then` | Standard library; correct on all collections. |
| Bounded ring buffer | A custom array+head+tail | `std::collections::VecDeque` | Stdlib; pop_front is O(1). |
| State-machine for menu navigation | A flat `match` block on input + bool flags | `bevy::state::SubStates` again, OR a small `MenuStack: Vec<MenuFrame>` | Bevy SubStates are heavy for menus; a `Vec<MenuFrame>` push/pop is the lightweight pattern. **Recommended: `Vec<MenuFrame>` inside `PlayerInputState`** — see §PlayerInputState shape. |
| egui frame composition | A custom CPU-rendered HUD | `bevy_egui::egui::CentralPanel`, `SidePanel`, `TopBottomPanel`, `Window`, `Area` | Already wired; the minimap precedent shows the patterns. |

---

## Common Pitfalls

### Pitfall 1: Adding `CurrentEncounter` as production code in #15 (it's #16's territory)

**What goes wrong:** #15 implementer authors a full `CurrentEncounter: Resource` with spawn logic; #16 lands and re-defines it; merge conflicts.

**Why it happens:** The resource is referenced everywhere in #15's code; instinct is to define it.

**How to avoid:** **Define `CurrentEncounter` as a TEST FIXTURE only in #15**, in a `#[cfg(test)] mod fixture` block. Document the production contract in a doc-comment in `turn_manager.rs`. The dev-cycler stub spawn (`#[cfg(feature = "dev")]`) is allowed for manual smoke; `#16` deletes that stub when it ships its own spawner.

### Pitfall 2: Mutating the `TurnActionQueue` from inside `execute_combat_actions` while iterating

**What goes wrong:** A multi-target action that spawns sub-actions tries to push to the queue mid-iteration → borrow-check error or out-of-iteration enqueue ordering bug.

**Why it happens:** Some genre features (chain attacks, area-of-effect that spawns follow-up effects) want to spawn new actions.

**How to avoid:** **Drain the queue with `std::mem::take` at the start of `execute_combat_actions`.** Iterate the local copy. New sub-actions go into a "next round" buffer (or are processed inline outside the queue). Same pattern as `MessageReader::read` already does — read a snapshot, not a live view.

```rust
fn execute_combat_actions(/* ... */) {
    let actions = std::mem::take(&mut queue.queue);
    for action in actions {
        // Apply; may push to queue.queue if needed for next round.
    }
}
```

### Pitfall 3: Re-target chain breaks when the actor itself dies before its action resolves

**What goes wrong:** Goblin attacks Aldric. Aldric's queued action is "Attack Goblin". The Goblin moves first, kills Aldric. Aldric's queued action tries to resolve; actor is dead.

**Why it happens:** Speed sort doesn't prevent mid-round actor death.

**How to avoid:** **In `execute_combat_actions`, check `is_alive(action.actor)` BEFORE resolving.** Skip the action if actor is dead/stoned. Add a "skipped action" entry to combat log: "Aldric is unable to act."

### Pitfall 4: Status effects (Sleep/Paralysis) prevent action emission, not action queuing

**What goes wrong:** A character at PlayerInput phase already has Sleep applied. The UI lets them queue an action (no gate). The resolver fails silently (skip on `is_asleep`).

**Why it happens:** UI doesn't know to disable the menu for asleep characters.

**How to avoid:** **`collect_player_actions` skips characters where `is_asleep`/`is_paralyzed` is true.** The UI also reads these predicates and shows "Sleeping" / "Paralyzed" in the slot's portrait area. Auto-skip and advance to the next slot.

### Pitfall 5: Front-row vs back-row damage modifier owned by multiple modules

**What goes wrong:** `damage.rs` applies the front/back row reduction; `ai.rs` ALSO applies it when picking a target ("prefer back-row weak targets"); they drift over time and fight a balance bug.

**Why it happens:** Genre intuition says AI should "be smart" about row.

**How to avoid:** **Row rules live in `damage.rs` ONLY.** The AI reads damage from a stub call to `damage_calc(...)` if it wants to reason about expected damage — that way the rule is in one place. **Recommendation: AI in v1 does NOT reason about row.** It picks random alive targets. Smart AI is a #17/#22 polish concern.

### Pitfall 6: `Defend` stacking with a pre-existing `DefenseUp`

**What goes wrong:** Player taps Defend on a character that has a magical `DefenseUp 1.0` already applied. The `apply_status_handler` stacking rule (D2 from #14) takes the higher magnitude — so a Defend (`potency: 0.5`) is silently ignored when the magic buff (`1.0`) is already present.

**Why it happens:** The merge rule was designed for "same effect, refresh duration". Defend is conceptually distinct from a magical buff but uses the same effect type.

**How to avoid:** This is a USER-PICK decision (D-Q4). Three options:
- **A — Same effect:** Defend writes `DefenseUp 0.5, duration 1`. If higher buff exists, no-op (silent — current behavior). Simplest; matches stacking rule.
- **B — Stacks magnitude:** Add `DefenseUpFromDefend` as a separate variant. Multi-buff characters get +50% from Defend AND +100% from magical buff. Adds enum variant; breaks save format if not appended.
- **C — Always refresh:** Defend `apply_status_handler` overrides the merge rule for Defend's writer. Specialized handler.

**Recommendation:** A (same effect). Surface as user pick.

### Pitfall 7: Combat log unbounded growth across a long campaign

**What goes wrong:** A `Vec<String>` combat log grows by ~5 lines per turn, ~50 turns per fight, ~50 fights per dungeon → 12500 strings. Memory grows; egui pagination becomes slow.

**Why it happens:** "Just clear it on combat end" is the obvious-but-wrong fix because the log is a debugging tool for the whole session.

**How to avoid:** **Bounded `VecDeque<CombatLogEntry>` capacity 50.** `push_back` + `pop_front` when over capacity. **Clear on `OnExit(GameState::Combat)`** if the design is "log is per-combat" (cheaper); **Keep across combats with capacity** if the design is "log is per-session" (more useful). User pick D-Q3.

### Pitfall 8: leafwing `CombatAction` enum collision with the recommended `CombatAction` queue payload type

**What goes wrong:** Implementer creates `pub enum CombatAction { Attack, Defend, ... }` in `combat/actions.rs`. There are now TWO `CombatAction` types in the codebase (one leafwing, one queue). `use crate::plugins::combat::CombatAction` collides with `use crate::plugins::input::CombatAction`.

**Why it happens:** Both are intuitively named.

**How to avoid:** **Rename the queue payload to `CombatActionKind`** (or `BattleAction`, or `TurnAction`). Recommendation: `CombatActionKind` — unambiguous and discoverable. Document at the top of `actions.rs` why the rename.

### Pitfall 9: Speed sort gives different results on "same-speed" turns across runs (RNG path or HashMap iteration)

**What goes wrong:** Two characters with `speed: 10` resolve in different order across runs because `Vec::sort_by` is unstable and HashMap iteration is randomized.

**Why it happens:** RNG creep.

**How to avoid:** **Use deterministic tie-break with `(actor_side, slot_index)`.** Same speed → party first (or enemies first — picker's call). Same side same speed → lower slot wins. Document the rule in `turn_manager.rs`. Use `Vec::sort_by` (NOT `sort_unstable_by`) — though for primitive comparisons either works as long as the `then` chain is exhaustive.

```rust
fn deterministic_tie_break(a: &QueuedAction, b: &QueuedAction) -> std::cmp::Ordering {
    use std::cmp::Ordering::*;
    a.actor_side.cmp(&b.actor_side)
        .then(a.slot_index.cmp(&b.slot_index))
        .then(a.actor_id_for_tiebreak.cmp(&b.actor_id_for_tiebreak))
}
```

### Pitfall 10: AI emits actions for Dead enemies (because `is_alive` filter forgotten)

**What goes wrong:** `enemy_ai_action_select` iterates ALL `With<Enemy>` entities. Dead enemies get queued actions that fail in the resolver.

**Why it happens:** Dead enemies aren't despawned; they retain `Enemy` marker.

**How to avoid:** **`enemy_ai_action_select` filters `!status.has(Dead) && !status.has(Stone)`.** Symmetrically, `collect_player_actions` skips party members where `derived.current_hp == 0` OR `status.has(Dead)`. Same logic for `Sleep`/`Paralysis` (Pitfall 4) — but those are skip-the-action, not skip-the-character.

### Pitfall 11: `recompute_derived_stats_on_equipment_change` doesn't see enemy buff changes (current `With<PartyMember>` filter)

Already covered in §L (D-K1 — drop the filter). Repeat-listing here for the planner's pitfall checklist.

### Pitfall 12: `Flee` succeeds inconsistently between Sub-PR 15A test and integration test

**What goes wrong:** `Flee` is RNG-gated (e.g., 50% success). Test seeds the RNG with one value; later integration test runs with default seed and sees different result.

**How to avoid:** **All RNG users in #15 read from a single `CombatRng: Resource(SmallRng)` resource.** Tests insert `CombatRng(SmallRng::seed_from_u64(42))` directly. Production seeds with `SmallRng::from_entropy()` (or `from_os_rng` in newer rand) once at `OnEnter(GameState::Combat)`.

---

## Security

### Known Vulnerabilities

| Library | CVE / Advisory | Severity | Status | Action |
|---------|----------------|----------|--------|--------|
| bevy | none found | — | — | (current pinned 0.18.1 is the project standard) |
| bevy_egui | none found | — | — | (already in deps for #10) |
| leafwing-input-manager | none found | — | — | (already in deps for #5) |
| rand | none found | — | — | Audit before adding (Step A/B/C). RustSec advisory database has no open advisories on rand 0.9.x as of training cutoff; verify at planning time. |
| serde / ron | none found | — | — | (already in use; no new attack surface) |

### Architectural Security Risks

| Risk | Affected | How It Manifests | Secure Pattern | Anti-Pattern to Avoid |
|------|----------|------------------|----------------|-----------------------|
| Crafted `CurrentEncounter` from a save with unbounded enemies | #15's resolver | Save file with `enemy_entities.len() == u32::MAX` causes loop blowup | #23 must bound the encounter size on deserialize. **Out of scope for #15.** | Trusting save data without length cap. |
| AI emits `CombatActionKind::Attack { weapon: malicious_handle }` | `damage_calc` reads `weapon` from `Assets<ItemAsset>` | If `weapon` handle resolves to an asset with `attack: u32::MAX`, damage saturating-arithmetic still produces u32::MAX → instant death | The existing saturating arithmetic in `damage_calc` is the guard; assets are loaded from authored RON. **Risk only manifests if save-load can override item assets.** | Non-saturating arithmetic. |
| `Vec<QueuedAction>` length unbounded | `TurnActionQueue` | Crafted save mid-fight → OOM | #23 should bound queue length on deserialize. **Out of scope for #15** since the queue is not persisted across save (it's cleared at end of round). | Persisting queue mid-round. |
| `CombatLog` unbounded growth | `combat_log.rs` | Log Vec grows over a long session | Bounded `VecDeque` cap (Pitfall 7). | Unbounded `Vec`. |
| RNG state predictability | `CombatRng` | If seed is publicly readable, players can predict crit rolls | Use `from_entropy()` for production; never expose seed. **Tests** seed deterministically. | `seed_from_u64(0)` in production. |
| `Flee` flag race with concurrent damage | `FleeAttempted: Resource` | Flee succeeds → state transitions to Dungeon → damage system races to mutate HP on now-no-longer-in-Combat entities | Single-frame flag write/read. The `TurnResult` phase reads `FleeAttempted` AFTER `ExecuteActions` finishes. Bevy's StateTransition schedule fires once per frame — no race. | State transition mid-system. |

### Trust Boundaries

- **`ApplyStatusEvent` write from #15's resolver:** validated by `apply_status_handler` (potency clamp, duration check). Already covered by #14.
- **`damage_calc` weapon parameter:** validated by `Assets<ItemAsset>` retrieval — handle must resolve. If asset is missing, default to no-weapon damage (warn!).
- **`CombatActionKind::CastSpell { spell_id: String }`:** spell ID is opaque to #15 (deferred to #20). Stub validates the string is non-empty; logs "Spell stub" and no-ops.
- **`TurnActionQueue` from a future save** (#23): bound length, validate each `actor`/`target` Entity exists. Out of scope for #15.

---

## Performance

| Metric | Value / Range | Source | Notes |
|--------|---------------|--------|-------|
| `execute_combat_actions` per-turn cost | O(N actions × M reads per action) — typically N≤8 (4 party + 4 enemy), M≤20 | (synthesized) | Single-digit ms; dwarfed by render. |
| Queue sort | O(N log N), N≤8 → ~24 ops | (synthesized) | Trivial. |
| `damage_calc` per-call | O(1) — no loops | (synthesized) | Pure arithmetic + 2-3 RNG draws. ~100ns. |
| `enemy_ai_action_select` per-turn | O(N enemies × M alive party) — N≤4, M≤4 → 16 ops | (synthesized) | Trivial. |
| `egui` paint cost (combat screen) | ~1-2ms typical (4 party panels + 4 enemy panels + log + menu) | (training data; bevy_egui benchmarks) | Within 60fps budget. egui is immediate-mode; no caching. |
| `CombatLog` `push_back` + `pop_front` (cap 50) | O(1) amortized | (stdlib `VecDeque`) | Constant-time ring. |
| Memory: `TurnActionQueue` | ~8 entries × 80 bytes ≈ 640 bytes | (synthesized) | Trivial. |
| Memory: `CombatLog` (cap 50) | 50 × ~80 bytes ≈ 4 KB | (synthesized) | Trivial. |

No benchmarks needed; the volumes are too small to register on any profile. Performance is **not a #15 concern** at this scope. The roadmap notes line 821: `Compile Δ +1-2s` — this is a build-time concern, not a runtime concern.

---

## Damage Formulas (USER-PICK DECISION D-A3)

Three industry-standard formulas, with worked examples. **The planner MUST surface these to the user before Sub-PR 15B begins.**

Common variables (in all three):
- `A` = attacker's `attack` stat (post-equipment, post-buffs).
- `D` = defender's `defense` stat (post-equipment, post-buffs).
- `WPN` = weapon's authored `attack` value (already folded into `A` via `derive_stats`, so this is implicit).
- `roll` = `rng.gen_range(0..100)`.

### Option A — Wizardry-style (multiplicative)

```
damage = (A * (100 - D / 2)) / 100
       max = A * (100 - D/2) / 100
       min = max * 0.7  (random variance: 0.7-1.0)
       crit (luck * 2 % chance) = max * 1.5
```

**Worked example:** A=20, D=10:
- `(20 * (100 - 5)) / 100 = (20 * 95) / 100 = 19` damage at full roll.
- Min: `19 * 0.7 = 13.3 → 13`.
- Crit: `19 * 1.5 = 28`.
- Defense doubles → D=20: `(20 * 90) / 100 = 18`. Defense quadruples → D=40: `(20 * 80) / 100 = 16`. Damage scales smoothly.

**Pros:**
- High-defense characters scale GRACEFULLY — never become invulnerable.
- Mathematically smooth; easy to balance.
- Wizardry-canonical (the genre this project models).

**Cons:**
- Defense never feels "rock-solid" — even D=99 still takes meaningful damage.
- Buffs feel mild (DefenseUp 0.5 reduces incoming damage by ~3-5 points, not 50%).

### Option B — Etrian-style (subtractive with floor)

```
damage = max(1, A - D)
       crit (luck * 2 % chance) = max(1, (A * 1.5) - D)
       random variance: 0.85-1.15 multiplier on the result
```

**Worked example:** A=20, D=10:
- `20 - 10 = 10` damage.
- Crit: `(30) - 10 = 20`.
- Variance: 8-11 normal hit, 17-23 crit.
- Defense doubles → D=20: `max(1, 20 - 20) = 1` damage (the floor). Defense quadruples → D=40: still `1`.

**Pros:**
- Defense feels MEANINGFUL — high D characters are tanky.
- DefenseUp (+50%) is dramatically impactful.
- Etrian-Odyssey-canonical.

**Cons:**
- High-D characters become functionally invulnerable to low-A enemies → the floor of 1 is unsatisfying.
- AC-overflow phase shifts cause balance cliffs (at one D level the boss does 0; at one less D, the boss does 30).

### Option C — Custom hybrid (subtractive with reduced multiplier)

```
damage = max(1, A - D * 0.5) * (1.0 + small_random_variance)
       crit = damage * 1.5
```

**Worked example:** A=20, D=10:
- `max(1, 20 - 5) = 15` damage.
- Crit: `22.5 → 22`.
- Defense doubles → D=20: `max(1, 20 - 10) = 10`. Defense quadruples → D=40: `max(1, 20 - 20) = 1`.

**Pros:**
- Best of both — D scales meaningfully but doesn't crash to 1 too quickly.
- Easy to tune the `0.5` constant for game feel.

**Cons:**
- Not canonical — players who know the genre may find it unfamiliar.
- Two tuning knobs (the multiplier AND the variance) increase tuning surface.

### Recommendation Summary

The genre this project models is Wizardry/Etrian. The planner should ask the user which feel they want:

> **For Wizardry feel (low-impact buffs, smooth scaling):** Option A.
> **For Etrian feel (impactful buffs, tanky defense):** Option B.
> **For tunable middle-ground:** Option C.

All three are 1-2 hour implementations. The decision affects gameplay feel for the entire game. **Surface to user before Sub-PR 15B.**

---

## UI Architecture

The egui combat screen layout, with rationale.

### Layout sketch

```
┌──────────────────────────────────────────────────────────────────┐
│                                                                  │
│  ┌──────────────────┐                                            │
│  │   Enemy 1         │                                            │
│  │   HP ▰▰▰▰▱▱▱▱   │                                            │
│  └──────────────────┘                                            │
│  ┌──────────────────┐                                            │
│  │   Enemy 2         │                                            │
│  │   HP ▰▰▱▱▱▱▱▱   │                                            │
│  └──────────────────┘            ┌────────────────────────────┐  │
│  ┌──────────────────┐            │   Combat Log               │  │
│  │   Enemy 3         │            │                            │  │
│  │   HP ▰▰▰▰▰▰▰▱   │            │  > Aldric attacks Goblin   │  │
│  └──────────────────┘            │    for 12 damage.          │  │
│                                  │  > Goblin defends.          │  │
│                                  │  > Mira casts Fire on       │  │
│                                  │    Goblin for 18 damage.   │  │
│                                  │  > Goblin falls!            │  │
│                                  │                            │  │
│                                  └────────────────────────────┘  │
│                                                                  │
│  ┌─────────┐ ┌─────────┐ ┌─────────┐ ┌─────────┐                │
│  │ Aldric  │ │  Mira   │ │ Father  │ │  Borin  │                │
│  │ HP ▰▰▰  │ │ HP ▰▰▰  │ │ Gren    │ │ HP ▰▰▰  │                │
│  │ MP ▰▰   │ │ MP ▰▰▰  │ │ HP ▰▰▰  │ │ MP ▱   │                │
│  │ STATUS  │ │ STATUS  │ │ MP ▰▰   │ │ STATUS  │                │
│  └─────────┘ └─────────┘ └─────────┘ └─────────┘                │
│                                                                  │
│  ┌────────────────────────────────────────────────────────────┐ │
│  │ ACTION MENU (only visible during PlayerInput phase)        │ │
│  │  > Attack   Defend   Spell   Item   Flee                   │ │
│  │  Slot: Aldric                                              │ │
│  └────────────────────────────────────────────────────────────┘ │
└──────────────────────────────────────────────────────────────────┘
```

### Panel-by-panel implementation guide

**Enemy column (left side):**
```rust
egui::SidePanel::left("enemy_column")
    .resizable(false)
    .min_width(200.0)
    .show(ctx, |ui| {
        for (entity, name, derived) in &enemies {
            paint_enemy_card(ui, entity, name, derived);
        }
    });
```

**Party panel (bottom):**
```rust
egui::TopBottomPanel::bottom("party_panel")
    .resizable(false)
    .min_height(120.0)
    .show(ctx, |ui| {
        ui.horizontal(|ui| {
            for (entity, name, derived, status) in &party {
                paint_party_card(ui, entity, name, derived, status);
            }
        });
    });
```

**Combat log (right side):**
```rust
egui::SidePanel::right("combat_log")
    .resizable(false)
    .min_width(300.0)
    .show(ctx, |ui| {
        egui::ScrollArea::vertical()
            .stick_to_bottom(true)
            .show(ui, |ui| {
                for entry in &log.entries {
                    ui.label(&entry.message);
                }
            });
    });
```

**Action menu (USER-PICK D-Q2):**
- **Option α — persistent panel at the bottom:** always visible, current slot's name shown, action buttons in a row.
  - Pros: low cognitive load; player can see all actions at all times.
  - Cons: takes up screen real estate even when not the player's turn.
- **Option β — modal popup per slot:** appears when current slot's turn; click outside to dismiss.
  - Pros: minimizes screen clutter.
  - Cons: hides actions when the player is reading the combat state.

```rust
// Option α (persistent)
egui::TopBottomPanel::bottom("action_menu")
    .resizable(false)
    .min_height(60.0)
    .show(ctx, |ui| {
        if let Some(slot) = state.active_slot {
            ui.horizontal(|ui| {
                if ui.button("Attack").clicked() { state.commit_attack(); }
                if ui.button("Defend").clicked() { state.commit_defend(); }
                if ui.button("Spell").clicked()  { state.open_spell_menu(); }
                if ui.button("Item").clicked()   { state.open_item_menu(); }
                if ui.button("Flee").clicked()   { state.commit_flee(); }
            });
        }
    });
```

```rust
// Option β (modal)
if let Some(slot) = state.active_slot {
    egui::Window::new("Action")
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .resizable(false)
        .show(ctx, |ui| { /* same buttons */ });
}
```

**Recommendation:** Option α (persistent). Lower cognitive load is a bigger win than the screen space; long combat sessions will benefit. **Surface as user pick D-Q2.**

**Target selection overlay:** appears as a center-anchored `egui::Window` listing alive enemies (or allies for healing). Player clicks one OR uses arrows + Confirm to select. The selection writes into `PlayerInputState.pending_action`'s `target` field, then the action commits.

```rust
if state.is_selecting_target {
    egui::Window::new("Target")
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .resizable(false)
        .show(ctx, |ui| {
            for (i, enemy_e) in alive_enemies.iter().enumerate() {
                let is_selected = state.target_cursor == Some(i);
                let color = if is_selected { egui::Color32::YELLOW } else { egui::Color32::WHITE };
                ui.colored_label(color, format!("> {}", enemy_names[enemy_e]));
            }
        });
}
```

### `PlayerInputState` shape

```rust
#[derive(Resource, Default, Debug, Clone)]
pub struct PlayerInputState {
    /// Slot of the party member currently choosing.
    /// `None` means all alive members have committed; transition to ExecuteActions.
    pub active_slot: Option<usize>,
    /// Stack of open menus. Top-of-stack is what the player sees.
    pub menu_stack: Vec<MenuFrame>,
    /// Actions committed so far this round.
    pub committed: Vec<QueuedAction>,
    /// When non-None, the player is currently selecting a target.
    pub pending_action: Option<PendingAction>,
    /// Target cursor for arrow-driven selection.
    pub target_cursor: Option<usize>,
}

#[derive(Debug, Clone)]
pub enum MenuFrame {
    Main,
    SpellMenu,
    ItemMenu,
    TargetSelect { mode: TargetSelection, kind: CombatActionKind },
}

#[derive(Debug, Clone)]
pub struct PendingAction {
    pub kind: CombatActionKind,
    pub actor: Entity,
}
```

The `menu_stack` pattern makes "back out of a sub-menu" trivial (`pop`) and "deeper menu" easy (`push`). Cancel collapses to the previous level; Cancel at the top deselects the action.

---

## Combat Log

Bounded ring buffer with a fixed capacity.

```rust
// src/plugins/combat/combat_log.rs

use std::collections::VecDeque;

#[derive(Debug, Clone)]
pub struct CombatLogEntry {
    pub message: String,
    pub turn_number: u32,
}

#[derive(Resource, Debug, Clone)]
pub struct CombatLog {
    pub entries: VecDeque<CombatLogEntry>,
    pub capacity: usize,
}

impl Default for CombatLog {
    fn default() -> Self {
        Self {
            entries: VecDeque::with_capacity(50),  // see USER-PICK D-Q3
            capacity: 50,
        }
    }
}

impl CombatLog {
    pub fn push(&mut self, message: String, turn_number: u32) {
        self.entries.push_back(CombatLogEntry { message, turn_number });
        while self.entries.len() > self.capacity {
            self.entries.pop_front();
        }
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}
```

**Cap-50 rationale:** at ~5 messages per turn, 50 entries = 10 turns of history. egui's `ScrollArea` paints all 50 fast (no virtualization needed). Memory: 50 × ~80 bytes = 4 KB.

**User pick D-Q3:**
- **Option A — bounded ring 50, kept across combats** (recommended): stable memory, persists for debugging long sessions.
- **Option B — bounded ring 50, cleared on `OnExit(GameState::Combat)`**: per-fight log; minimizes confusion across combats.
- **Option C — unbounded `Vec`, cleared on combat end**: no cap; relies on the clear to bound. Risk of crash mid-fight not freeing memory.

**Default recommendation: Option A (bounded ring 50, kept across combats).**

---

## Defend Integration (USER-PICK D-Q4)

The `Defend` action writes `ApplyStatusEvent { effect: DefenseUp, ... }`. Stacking with an existing magical `DefenseUp` from a future #20 buff spell is the load-bearing question.

**Three policies (one is the default):**

- **A — Same-effect, take-higher (current #14 behavior):** Defend `potency = 0.5`. If higher buff exists, Defend is silently no-op'd by the merge rule.
- **B — Stack as separate variant:** Add `DefenseUpFromDefend` enum variant. Composes with magical `DefenseUp` for cumulative defense. Costs a save-format slot.
- **C — Refresh duration regardless:** Specialized handler for Defend writer that overrides the merge rule, refreshing duration even if magnitude is lower.

**Recommendation: A (same-effect, take-higher).** Simplest; matches stacking rule. Players' Defend tap during a strong magical buff is "redundant but graceful". Surface as user pick.

---

## Boss AI (USER-PICK D-Q5)

The roadmap line 850 asks for a `BossAI` enum stub. **Two options:**

### Option A — Ship `BossAi` enum stub with 1-2 patterns now

```rust
#[derive(Component, Reflect, Default, Debug, Clone, Copy)]
pub enum EnemyAi {
    #[default]
    RandomAttack,
    Boss(BossPattern),
}

#[derive(Reflect, Debug, Clone, Copy)]
pub enum BossPattern {
    /// Always targets the lowest-HP party member.
    FocusWeakest,
    /// Cycles 3 actions: Attack, Defend, Attack.
    AttackDefendAttack { turn: u32 },
}
```

- **Pros:** Boss-fight authoring has a hook from day one. #17 (sprites) can ship a boss enemy without a separate AI feature.
- **Cons:** ~50 LOC, ~3 tests. Speculative API surface (the patterns chosen now may not match what designers want later).

### Option B — Pure `RandomAttack` only, defer `BossAi` to a later feature

- **Pros:** Tightest scope; honest about v1.
- **Cons:** First boss authoring requires re-opening the AI module.

**Recommendation: Option A** — ship the `BossAi` enum stub with `FocusWeakest` AND `AttackDefendAttack { turn: u32 }`. Two patterns are enough to validate the pattern type; designer can author more in #17 / future features. Costs ~80 LOC and ~4 tests; benefit is a clean pattern type for future authoring. Surface as user pick.

---

## Speed Tie-Breaking

When two combatants have the same `speed`, the resolution order must be deterministic. Three policies:

- **A — Slot-order:** Party slot N goes before slot N+1. Enemies index 0 before 1, etc. **Recommended** — simplest, deterministic, predictable for players.
- **B — Random:** Tie broken with RNG. Adds RNG creep; harder to reason about combat replays.
- **C — Insertion order:** Whatever order the queue was built in (party first, then enemies). Equivalent to A for the typical case.

**Recommendation: A (slot-order with party-first).** Document the rule in `turn_manager.rs::sort_queue` and add a unit test:

```rust
fn deterministic_sort(queue: &mut Vec<QueuedAction>) {
    queue.sort_by(|a, b| {
        b.speed_at_queue_time.cmp(&a.speed_at_queue_time)  // descending speed
            .then(a.actor_side.cmp(&b.actor_side))         // party (Side::Party=0) before enemy (Side::Enemy=1)
            .then(a.slot_index.cmp(&b.slot_index))         // slot ascending
    });
}
```

---

## Code Examples

Verified patterns from existing project code.

### Example 1 — Plugin shape (`combat/turn_manager.rs`)

```rust
// Source: status_effects.rs:104-135 (StatusEffectsPlugin) — file mirror this.

use bevy::prelude::*;
use crate::plugins::input::CombatAction;  // leafwing menu nav
use leafwing_input_manager::prelude::ActionState;

use crate::plugins::state::{GameState, CombatPhase};
use crate::plugins::combat::status_effects::{ApplyStatusEvent, apply_status_handler};

pub struct TurnManagerPlugin;

impl Plugin for TurnManagerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TurnActionQueue>()
            .init_resource::<PlayerInputState>()
            .init_resource::<CombatLog>()
            .init_resource::<CombatRng>()
            .init_resource::<FleeAttempted>()
            .add_systems(
                OnEnter(GameState::Combat),
                (init_combat_state, log_combat_start).chain(),
            )
            .add_systems(
                OnExit(GameState::Combat),
                (clear_combat_state,),
            )
            .add_systems(
                Update,
                (
                    collect_player_actions
                        .run_if(in_state(CombatPhase::PlayerInput)),
                    append_enemy_actions_and_sort
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
```

### Example 2 — `CombatActionKind` and `QueuedAction`

```rust
// src/plugins/combat/actions.rs

use bevy::prelude::*;
use crate::data::items::ItemAsset;
use crate::plugins::combat::targeting::TargetSelection;

/// Action queued for resolution. Wraps a `CombatActionKind` (pure-data action
/// payload) with the actor entity, target selection, and the speed at which
/// the action was queued (locked at queue-time so mid-round speed buffs don't
/// re-order this round's actions — see Pitfall 9).
///
/// Renamed from "CombatAction" to avoid collision with `crate::plugins::input::CombatAction`
/// (the leafwing menu-navigation enum).
#[derive(Debug, Clone)]
pub struct QueuedAction {
    pub actor: Entity,
    pub kind: CombatActionKind,
    pub target: TargetSelection,
    pub speed_at_queue_time: u32,
    pub actor_side: Side,
    pub slot_index: u32,  // for tie-break
}

/// The action a combatant chose to take this round.
///
/// **`CastSpell` is a stub in #15** — full spell mechanics deferred to #20.
/// **`UseItem` validates the item is `ItemKind::Consumable` in #15.**
#[derive(Debug, Clone, Reflect)]
pub enum CombatActionKind {
    /// Physical attack with the actor's currently-equipped weapon.
    /// `weapon` is captured at queue-time to handle mid-round equip swaps
    /// (won't happen in v1 but design defensively).
    Attack,
    /// Sets a 1-turn DefenseUp via ApplyStatusEvent.
    Defend,
    /// Stub — emits "Spell stub: not yet implemented" to combat log.
    /// Full implementation in #20.
    CastSpell { spell_id: String },
    /// Consume an item from the actor's `Inventory`.
    UseItem { item: Handle<ItemAsset> },
    /// Try to escape combat. RNG-gated success.
    Flee,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Side { Party, Enemy }

impl Side {
    pub fn from_party<F: Fn(Entity) -> bool>(actor: Entity, is_party: F) -> Self {
        if is_party(actor) { Side::Party } else { Side::Enemy }
    }
}
```

### Example 3 — `damage_calc` skeleton (Option A — Wizardry-style)

```rust
// src/plugins/combat/damage.rs

use rand::Rng;
use bevy::prelude::*;
use crate::data::items::ItemAsset;
use crate::plugins::combat::actions::CombatActionKind;
use crate::plugins::party::{DerivedStats, PartyRow, StatusEffects};

#[derive(Debug, Clone)]
pub struct Combatant {
    pub name: String,
    pub stats: DerivedStats,
    pub row: PartyRow,
    pub status: StatusEffects,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DamageResult {
    pub damage: u32,
    pub hit: bool,
    pub critical: bool,
    pub message: String,
}

pub fn damage_calc(
    attacker: &Combatant,
    defender: &Combatant,
    weapon: Option<&ItemAsset>,
    action: &CombatActionKind,
    rng: &mut impl Rng,
) -> DamageResult {
    // 1. Hit roll.
    let hit_chance = attacker.stats.accuracy.saturating_sub(defender.stats.evasion);
    let hit = rng.gen_range(0..100) < hit_chance.min(100) as u32;
    if !hit {
        return DamageResult {
            damage: 0,
            hit: false,
            critical: false,
            message: format!("{} misses {}.", attacker.name, defender.name),
        };
    }

    // 2. Row check.
    let row_modifier = match (attacker.row, defender.row, weapon) {
        (PartyRow::Front, PartyRow::Back, Some(w)) if is_melee(w) => 0.0,
        (PartyRow::Back, _, _) if !weapon_reaches_from_back(weapon) => 0.5,
        _ => 1.0,
    };
    if row_modifier == 0.0 {
        return DamageResult {
            damage: 0,
            hit: true,
            critical: false,
            message: format!("{}'s attack can't reach {}.", attacker.name, defender.name),
        };
    }

    // 3. Wizardry-style damage.
    let raw = (attacker.stats.attack as i64
        * (100 - defender.stats.defense.min(180) as i64 / 2))
        / 100;
    let raw = raw.max(1) as u32;
    let variance = rng.gen_range(70..=100) as f32 / 100.0;
    let damage = (raw as f32 * variance * row_modifier) as u32;

    // 4. Crit roll.
    let crit_chance = attacker.stats.accuracy.min(100);  // luck not in DerivedStats yet
    let critical = rng.gen_range(0..100) < (crit_chance / 5) as u32;
    let damage = if critical { (damage as f32 * 1.5) as u32 } else { damage };

    DamageResult {
        damage: damage.max(1),
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

fn is_melee(_weapon: &ItemAsset) -> bool { /* TODO: weapon kind classification — defer */ true }
fn weapon_reaches_from_back(_weapon: Option<&ItemAsset>) -> bool { /* spears, bows */ false }
```

**Key properties:**
- Pure (no entity lookup, no resource read).
- RNG passed in; tests seed deterministically.
- Saturating arithmetic everywhere (defends against `attack: u32::MAX` from a malicious save).
- Crit chance cap (won't divide by zero or overflow).
- Returns a `DamageResult` with a pre-formatted log message.

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Bevy 0.17 `EventReader<T>` for buffered events | Bevy 0.18 `MessageReader<T>` / `MessageWriter<T>` for buffered messages | Bevy 0.18 release (Mar 2026) | All `#[derive(Event)]` for buffered uses must be `#[derive(Message)]`. Verified by `feedback_bevy_0_18_event_message_split.md` memory. |
| Inline state mutation in AI ("AI does damage") | Pure emit-action AI (writes to queue) | Modern DRPG architecture; research Pattern 5 | Decouples AI from resolver. Removes a class of bugs. |
| `Camera3dBundle`, `PointLightBundle` | Component tuples with `#[require(...)]` auto-attaching deps | Bevy 0.18 | `combat_camera` (if separate) spawns as `Camera3d::default()` not bundle. Reference: `reference_bevy_018_camera3d_components.md`. |
| `EventReader::iter()` | `MessageReader::read()` | Bevy 0.18 family rename | Use `events.read()` not `events.iter()`. |
| Separate damage+animation system | Pure damage_calc + (later) animation observer | research §Anti-Patterns | Animations land in #17; #15 stays headless on visuals. |

**Deprecated/outdated:**
- The roadmap's reference to "research Pattern 5" — this is correct conceptually but the roadmap doesn't pin which research; check current research pattern docs at the time of implementation.

---

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | Cargo built-in (`cargo test`) |
| Config file | `Cargo.toml` (no separate test config) |
| Quick run command | `cargo test --package druum -p druum --test '*' --lib --features dev -- combat::` |
| Full suite command | `cargo test --workspace --all-features` |

### Requirements → Test Map

This is a **draft** test list — Sub-PR planning will refine. Layer 1 = pure function tests, Layer 2 = `App`-driven systems tests.

| # | Requirement | Behavior | Test Type | File | Layer | Exists? |
|---|-------------|----------|-----------|------|-------|---------|
| 1 | Damage formula: defense > attack returns floor of 1 | `damage_calc(A=5, D=20, ...) → damage = 1` | unit | `damage.rs::tests` | L1 | ❌ |
| 2 | Damage formula: zero attack returns floor of 1 | `damage_calc(A=0, ...) → damage = 1` | unit | `damage.rs::tests` | L1 | ❌ |
| 3 | Damage formula: critical hit applies 1.5x | seed RNG to land in crit window; assert 1.5x raw | unit | `damage.rs::tests` | L1 | ❌ |
| 4 | Damage formula: miss when accuracy < evasion | seed RNG to land outside hit window | unit | `damage.rs::tests` | L1 | ❌ |
| 5 | Damage formula: front-row vs back-row weapon range | melee weapon from front to back row → damage = 0 ("out of reach") | unit | `damage.rs::tests` | L1 | ❌ |
| 6 | Damage formula: back-row attacker with bow reaches back-row defender | bow weapon from back to back → full damage | unit | `damage.rs::tests` | L1 | ❌ |
| 7 | Damage formula: variance bounds | seed multiple rolls; assert variance bounded by 70%-100% | unit | `damage.rs::tests` | L1 | ❌ |
| 8 | Speed sort: descending order | queue with [s=10, s=20, s=15] → resolves [s=20, s=15, s=10] | unit | `turn_manager.rs::tests` | L1 | ❌ |
| 9 | Speed sort: tie broken by slot-order (party first, ascending slot) | two s=10 actors, slot 0 and slot 2 → slot 0 first | unit | `turn_manager.rs::tests` | L1 | ❌ |
| 10 | Speed sort: party before enemy on same speed | s=10 party + s=10 enemy → party first | unit | `turn_manager.rs::tests` | L1 | ❌ |
| 11 | Mid-turn death: actor dies before its action resolves → action skipped with log entry | queue [Goblin Attack Aldric, Aldric Attack Goblin] where Goblin's first attack drops Aldric to 0 HP → Aldric's action skipped | system | `turn_manager.rs::app_tests` | L2 | ❌ |
| 12 | Mid-turn death: target dies before action resolves → re-target to alive ally on same side | queue [Aldric Attack Goblin, Mira Attack Goblin] where Aldric kills Goblin first → Mira's action re-targets to next live enemy | system | `turn_manager.rs::app_tests` | L2 | ❌ |
| 13 | Mid-turn death: target's whole side wiped out → action no-ops with miss log | last enemy dies; queued action against enemies has nothing to target | system | `turn_manager.rs::app_tests` | L2 | ❌ |
| 14 | `Defend` writes `DefenseUp` via `ApplyStatusEvent` | resolve Defend; assert StatusEffects has DefenseUp | system | `turn_manager.rs::app_tests` | L2 | ❌ |
| 15 | `Defend → DefenseUp` re-derives stats via existing recompute pipeline | resolve Defend; observe DerivedStats.defense increase next frame | system | `turn_manager.rs::app_tests` | L2 | ❌ |
| 16 | `Defend` expires at end of round (duration: Some(1) ticks down) | resolve Defend; advance one tick; assert DefenseUp gone | system | `turn_manager.rs::app_tests` | L2 | ❌ |
| 17 | `Flee` succeeds RNG-gated → state transitions to Dungeon | seed RNG; resolve Flee; assert NextState<GameState> = Dungeon | system | `turn_manager.rs::app_tests` | L2 | ❌ |
| 18 | `Flee` fails RNG-gated → stays in Combat with log entry | seed RNG to fail; resolve Flee; assert state unchanged + log entry | system | `turn_manager.rs::app_tests` | L2 | ❌ |
| 19 | Victory: all enemies Dead → state transitions out of Combat | seed all enemies HP=1 then attack; assert next round detects victory | system | `turn_manager.rs::app_tests` | L2 | ❌ |
| 20 | Defeat: all party Dead → state transitions to GameOver | seed all party HP=1 then enemy attacks all; assert GameOver | system | `turn_manager.rs::app_tests` | L2 | ❌ |
| 21 | AI: `RandomAttack` always picks an alive party member | seed RNG; run AI 10 times; assert target is always alive party | system | `ai.rs::app_tests` | L2 | ❌ |
| 22 | AI: deterministic with same seed | seed RNG with 42 twice; run AI; assert identical queue | system | `ai.rs::app_tests` | L2 | ❌ |
| 23 | AI: skips Dead enemies (doesn't queue actions for them) | mark enemy Dead; run AI; assert no queue entry for that enemy | system | `ai.rs::app_tests` | L2 | ❌ |
| 24 | AI: when no alive party exist, action emission gracefully no-ops | mark all party Dead; run AI; assert empty queue (no panic) | system | `ai.rs::app_tests` | L2 | ❌ |
| 25 | Sleep prevents action emission in PlayerInput phase | apply Sleep to character; auto-skip in PlayerInput; commits a "Sleeping" no-op | system | `turn_manager.rs::app_tests` | L2 | ❌ |
| 26 | Paralysis prevents action emission | apply Paralysis; auto-skip | system | `turn_manager.rs::app_tests` | L2 | ❌ |
| 27 | Silenced character cannot select Spell action | apply Silence; assert UI gates spell button (or auto-skips spell selection) | system | `turn_manager.rs::app_tests` | L2 | ❌ |
| 28 | `check_dead_and_apply` fires on damage that reduces HP to 0 | apply damage equal to HP; assert next frame Dead status present | system | `turn_manager.rs::app_tests` | L2 | ❌ |
| 29 | Combat log: bounded ring at cap 50 | push 60 entries; assert len == 50 and oldest dropped | unit | `combat_log.rs::tests` | L1 | ❌ |
| 30 | Combat log: clear on `OnExit(Combat)` (if Option B chosen) | enter combat; push entries; exit; assert log cleared | system | `combat_log.rs::app_tests` | L2 | ❌ |
| 31 | UseItem consumes item from inventory | give a healing potion; queue UseItem; resolve; assert HP changed AND inventory item removed | system | `turn_manager.rs::app_tests` | L2 | ❌ |
| 32 | UseItem on KeyItem rejected with log entry | queue UseItem with a key item handle; resolve; assert log "can't use" + inventory unchanged | system | `turn_manager.rs::app_tests` | L2 | ❌ |
| 33 | Stub `CastSpell` writes log entry without crashing | queue CastSpell { "fire" }; resolve; assert log entry "Spell stub" | system | `turn_manager.rs::app_tests` | L2 | ❌ |

**Total tests: 33** (within +20-30 envelope from line 822, with 3 above the upper bound — appropriate for the size of the feature).

### Gaps (files to create before implementation starts)

- [ ] `src/plugins/combat/turn_manager.rs` — covers requirements 8-20, 25-28, 31-33 (Sub-PR 15A)
- [ ] `src/plugins/combat/actions.rs` — covers `CombatActionKind` enum, `QueuedAction` struct, `Side` enum (Sub-PR 15A)
- [ ] `src/plugins/combat/damage.rs` — covers requirements 1-7 (Sub-PR 15B)
- [ ] `src/plugins/combat/targeting.rs` — covers re-target logic (Sub-PR 15B)
- [ ] `src/plugins/combat/ai.rs` — covers requirements 21-24 (Sub-PR 15C)
- [ ] `src/plugins/combat/enemy.rs` — minimal Enemy ECS (Sub-PR 15A or 15C)
- [ ] `src/plugins/combat/combat_log.rs` — covers requirements 29-30 (Sub-PR 15A or 15D)
- [ ] `src/plugins/combat/ui_combat.rs` — Sub-PR 15D, manual smoke + layout snapshot tests
- [ ] Edits to `src/plugins/combat/mod.rs` — register 3 new sub-plugins (Sub-PR 15A starts; 15C / 15D extend)

---

## Sub-PR Decomposition

**Recommended four sequential sub-PRs**, mirroring roadmap line 824.

### Sub-PR 15A — Turn Manager + State Machine (~350 LOC)

**Files:**
- New: `turn_manager.rs`, `actions.rs`, `enemy.rs`, `combat_log.rs`
- Edits: `combat/mod.rs` (+1 sub-plugin), `state/mod.rs` (carve-out comment about EnemyTurn vestige), `inventory.rs` (D-K1 — drop `With<PartyMember>` filter)
- Tests: queue ordering (8-10), Defend integration (14-16), victory/defeat/flee (17-20), Sleep/Paralysis/Silence gates (25-27), check_dead (28), UseItem (31-32), CastSpell stub (33)

**Test count:** ~18 tests

**Decisions blocked by user pick:** D-Q4 (Defend stacking), D-Q3 (combat log shape).

### Sub-PR 15B — Damage (~250 LOC)

**Files:**
- New: `damage.rs`, `targeting.rs`
- Edits: `turn_manager.rs` (call `damage_calc` from resolver — written in Sub-PR 15A as a stub returning fixed damage; replaced here)
- Tests: damage formula edge cases (1-7), targeting edge cases (11-13)

**Test count:** ~10 tests

**Decisions blocked by user pick:** D-A3 (damage formula choice).

### Sub-PR 15C — AI (~200 LOC)

**Files:**
- New: `ai.rs`, additions to `enemy.rs` (EnemyAi enum)
- Edits: `combat/mod.rs` (+1 sub-plugin)
- Tests: AI determinism, target validity, dead-skip, no-alive-party (21-24)

**Test count:** ~4 tests

**Decisions blocked by user pick:** D-Q5 (BossAi scope).

### Sub-PR 15D — UI (~400-600 LOC)

**Files:**
- New: `ui_combat.rs`
- Edits: `combat/mod.rs` (+1 sub-plugin)
- Tests: layout smoke tests, log truncation; manual smoke for visual fidelity

**Test count:** ~3 tests + manual

**Decisions blocked by user pick:** D-Q2 (action menu UX), D-Q1 (combat camera shape — overlay vs separate scene).

---

## Decisions Surfaced for the User

The planner MUST surface these to the user before kickoff (D-A3, D-Q4) or before the relevant sub-PR (D-Q1, D-Q2, D-Q3, D-Q5).

### D-A3 — Damage formula choice (BEFORE Sub-PR 15B)

The genre this project models is Wizardry/Etrian. The planner should ask:

> **For Wizardry feel (low-impact buffs, smooth scaling, defense never trivializes):** Option A.
>
> **For Etrian feel (impactful buffs, tanky defense, AC-overflow phase shifts):** Option B.
>
> **For tunable middle-ground (best of both, two tuning knobs):** Option C.

See §Damage Formulas for full pros/cons and worked examples. **Default if user does not pick: Option A (Wizardry-style).**

### D-Q1 — Combat camera shape (BEFORE Sub-PR 15D)

> **Combat happens on the dungeon camera (overlay):** simplest. Wizardry/Etrian style. Recommended.
>
> **Combat happens on a dedicated combat camera (cinematic):** new camera spawned in `OnEnter(Combat)`. Better for animations and dramatic angles. Defer to #17 if needed.

**Default if user does not pick: overlay (dungeon camera).**

### D-Q2 — Action menu UX (BEFORE Sub-PR 15D)

> **Persistent action panel** (always visible at bottom, action buttons in a row): low cognitive load. Recommended.
>
> **Modal popup per slot** (appears on slot's turn, dismisses): minimizes screen clutter.

**Default if user does not pick: persistent panel.**

### D-Q3 — Combat log shape (BEFORE Sub-PR 15A)

> **Bounded ring buffer (cap 50), kept across combats:** stable memory, persists for debugging.
>
> **Bounded ring buffer (cap 50), cleared on combat end:** per-fight log; minimizes confusion across combats.
>
> **Unbounded `Vec`, cleared on combat end:** simplest; no cap.

**Default if user does not pick: bounded ring buffer cap 50, kept across combats.**

### D-Q4 — Defend stacking with existing DefenseUp (BEFORE Sub-PR 15A)

> **Same-effect, take-higher (current #14 behavior):** Defend silently no-ops if a higher buff exists. Simplest.
>
> **Stack as separate `DefenseUpFromDefend` variant:** Defend always adds +50% on top of any magical buff. Adds save-format slot.
>
> **Refresh duration regardless:** Specialized handler for Defend writer. Most code; most "natural" feel.

**Default if user does not pick: same-effect, take-higher.**

### D-Q5 — Boss AI scope (BEFORE Sub-PR 15C)

> **Ship `BossAi` enum stub with 1-2 patterns now:** `FocusWeakest` and `AttackDefendAttack`. ~80 LOC, ~4 tests. Hooks for #17 boss authoring.
>
> **Pure `RandomAttack` only, defer `BossAi`:** Tightest scope. First boss authoring requires re-opening AI module.

**Default if user does not pick: Option A (ship the stub).**

---

## Open Questions

The plan defers FIVE USER-PICK decisions per `## Decisions Surfaced for the User`. All five default to research-recommended options; all are runtime-feel/design-philosophy calls; all are reversible without code-structure change. **The implementer proceeds with defaults unless the user objects at plan approval.**

Other unresolved questions for the planner to think about:

1. **Do all party members commit one action per round, or can they pass?** Wizardry has "act / defend / parry / cast / use / run" — there is always an action. Recommendation: every alive party member must commit one action per round (no pass).

2. **Does `Flee` apply to whole party (group flee) or per-member (one flees, others stay)?** Wizardry/Etrian: group flee. Recommendation: group flee — the whole party returns to dungeon. Per-member is a design rabbit hole.

3. **Does the round timer have a wall-clock limit?** No — round resolves as fast as the player commits. Animations (later) will pace the resolution.

4. **What happens if all enemies AND all party are killed in the same round (mutual destruction)?** The order of `check_victory_defeat_flee` matters. Recommendation: check defeat first (party Dead → GameOver) — players don't get a "phyrric victory" loss. Document.

5. **Does the queue persist across saves mid-round?** Recommendation: NO — combat is atomic. Saving mid-round is a future feature. Document `TurnActionQueue` is NOT serialized in #15.

6. **Does the `AntiMagicZone` from #13 prevent spell casting in dungeon-tile combat?** Recommendation: defer to #20 spell mechanics. #15 stub `CastSpell` doesn't need to read this.

---

## Sources

### Primary (HIGH confidence — local Bevy 0.18.1 source + the merged Druum tree)

- [Bevy 0.18.1 source — local extraction](file:///Users/nousunio/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bevy-0.18.1) — verified `SubStates` / `Message` / plugin patterns
- [Bevy 0.18.1 `bevy_state`](file:///Users/nousunio/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bevy_state-0.18.1) — `SubStates` source-gating semantics
- [Druum: src/plugins/state/mod.rs](file:///Users/nousunio/Repos/Learnings/claude-code/druum/src/plugins/state/mod.rs) — `CombatPhase`, `GameState`
- [Druum: src/plugins/combat/mod.rs](file:///Users/nousunio/Repos/Learnings/claude-code/druum/src/plugins/combat/mod.rs) — `CombatPlugin` shape
- [Druum: src/plugins/combat/status_effects.rs](file:///Users/nousunio/Repos/Learnings/claude-code/druum/src/plugins/combat/status_effects.rs) — `ApplyStatusEvent`, `apply_status_handler`, `check_dead_and_apply`, predicate pattern
- [Druum: src/plugins/party/character.rs](file:///Users/nousunio/Repos/Learnings/claude-code/druum/src/plugins/party/character.rs) — `BaseStats`, `DerivedStats`, `derive_stats`, `PartyMember`, `PartyRow`, `StatusEffects`, `StatusEffectType`
- [Druum: src/plugins/party/inventory.rs](file:///Users/nousunio/Repos/Learnings/claude-code/druum/src/plugins/party/inventory.rs) — `Equipment`, `Inventory`, `EquipmentChangedEvent`, `recompute_derived_stats_on_equipment_change`, `EquipSlot::None` sentinel, `equip_item`/`unequip_item`/`give_item`
- [Druum: src/plugins/dungeon/mod.rs](file:///Users/nousunio/Repos/Learnings/claude-code/druum/src/plugins/dungeon/mod.rs) — `MovedEvent`, `handle_dungeon_input`, system ordering precedent
- [Druum: src/plugins/dungeon/features.rs](file:///Users/nousunio/Repos/Learnings/claude-code/druum/src/plugins/dungeon/features.rs) — `apply_pit_trap`, `apply_poison_trap` (refactored to ApplyStatusEvent), `make_test_app` precedent
- [Druum: src/plugins/input/mod.rs](file:///Users/nousunio/Repos/Learnings/claude-code/druum/src/plugins/input/mod.rs) — `CombatAction` (leafwing), `default_combat_input_map`
- [Druum: src/plugins/ui/minimap.rs](file:///Users/nousunio/Repos/Learnings/claude-code/druum/src/plugins/ui/minimap.rs) — egui paint pattern, `EguiContexts`, `EguiPrimaryContextPass` schedule
- [Druum: src/plugins/ui/mod.rs](file:///Users/nousunio/Repos/Learnings/claude-code/druum/src/plugins/ui/mod.rs) — `UiPlugin` with `auto_create_primary_context: false`
- [Druum: src/main.rs](file:///Users/nousunio/Repos/Learnings/claude-code/druum/src/main.rs) — plugin registration order
- [Druum: project/research/20260507-115500-feature-14-status-effects-system.md](file:///Users/nousunio/Repos/Learnings/claude-code/druum/project/research/20260507-115500-feature-14-status-effects-system.md) — output shape mirrored, message-only architecture validated
- [Druum: project/plans/20260507-124500-feature-14-status-effects-system.md](file:///Users/nousunio/Repos/Learnings/claude-code/druum/project/plans/20260507-124500-feature-14-status-effects-system.md) — planner consumption shape mirrored
- [Druum: project/reviews/20260507-153000-feature-14-status-effects-system.md](file:///Users/nousunio/Repos/Learnings/claude-code/druum/project/reviews/20260507-153000-feature-14-status-effects-system.md) — review-validated patterns (test-harness, sole-mutator, before-handler ordering)
- [Druum: project/roadmaps/20260429-01-bevy-dungeon-crawler-roadmap.md (lines 789-862)](file:///Users/nousunio/Repos/Learnings/claude-code/druum/project/roadmaps/20260429-01-bevy-dungeon-crawler-roadmap.md) — Feature #15 roadmap entry

### Secondary (MEDIUM confidence — Cargo.lock confirmation, training data on rand 0.9.x)

- `rand` 0.9.4 already transitively present in `/Users/nousunio/Repos/Learnings/claude-code/druum/Cargo.lock` (verified at line 4360). Direct dep addition needs Step A/B/C gate.
- `rand_chacha` 0.9.0 also transitively present (Cargo.lock line 4368). Available as alternative for ChaCha8/12/20.
- `rand_distr` 0.5.1 also transitively present (Cargo.lock line 4387). Not needed for #15.
- bevy_egui 0.39.1 API shape — used in `minimap.rs`; same patterns apply to combat UI. Verified by direct Druum source inspection.

### Tertiary (LOW confidence — research patterns from Druum's prior research/plan documents, RPG genre conventions)

- "Action-queue combat loop / research Pattern 5" — referenced by roadmap; the canonical research reference is the roadmap itself + Wizardry/Etrian Odyssey gameplay convention. Pattern 5 implementation matches.
- Damage formulas (Wizardry-style multiplicative, Etrian subtractive) — taken from genre common knowledge; specific constants are author-tunable.

---

## Metadata

**Confidence breakdown:**

- File layout under `src/plugins/combat/`: HIGH — matches roadmap line 808 + status_effects.rs precedent.
- State machine (skip `EnemyTurn`, use existing 3-phase): HIGH — `CombatPhase` already declared in `state/mod.rs:28-36`.
- `CombatActionKind` enum shape: HIGH — straightforward enum, payload-as-data pattern.
- `Defend → DefenseUp` via `ApplyStatusEvent`: HIGH — #14 pipeline verified, no parallel state needed.
- `CurrentEncounter` test-fixture-only contract: HIGH — `#16` owns; documented contract is the right move.
- `damage_calc` shape and row-rule single-owner: HIGH on shape, MEDIUM on formula (USER PICK D-A3).
- AI emission boundary: HIGH — mirrors `apply_poison_trap` precedent.
- Targeting re-resolution: HIGH on the algorithm; LOW on whether `same-side` random pick is the design feel the user wants (could also be "next-by-slot" — minor).
- Speed tie-breaking: MEDIUM — recommended deterministic but USER could pick RNG.
- `rand` direct dep: MEDIUM-LOW until Step A/B/C gate runs.
- egui screen layout: HIGH on layout primitives (`SidePanel`, `TopBottomPanel`, `Window`) — mirrors `paint_minimap_full`. MEDIUM on persistent vs modal action menu (USER PICK D-Q2).
- Combat log shape: HIGH on bounded VecDeque; MEDIUM on cap size (USER PICK D-Q3).
- Test count and test coverage: HIGH — 33 tests cover all enumerated requirements.
- Sub-PR decomposition (4 sequential): HIGH — matches roadmap line 824 verbatim.
- Boss AI scope: MEDIUM (USER PICK D-Q5).

**Research date:** 2026-05-08
