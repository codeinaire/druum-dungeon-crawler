---
name: Druum CombatPhase + CombatAction enum already shipped pre-#15
description: CombatPhase SubStates and leafwing CombatAction Actionlike are already declared and registered before Feature #15 begins
type: reference
---

Before researching or planning Feature #15 (Turn-Based Combat Core) for Druum, know the pre-shipped scaffolding to avoid recommending re-creation:

**`CombatPhase` SubStates enum** at `src/plugins/state/mod.rs:28-36`:

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

Registered at `state/mod.rs:53-56` via `app.add_sub_state::<CombatPhase>()`. **#15 does NOT define this; #15 wires systems gated `.run_if(in_state(CombatPhase::X))`.**

The `EnemyTurn` variant is **vestigial** relative to the action-queue design (the recommended pattern interleaves enemy actions into a single `ExecuteActions` phase sorted by speed). #15 should skip `EnemyTurn` entirely and document the unused variant.

**`CombatAction` leafwing Actionlike enum** at `src/plugins/input/mod.rs:90-98`:

```rust
#[derive(Actionlike, PartialEq, Eq, Hash, Clone, Copy, Debug, Reflect)]
pub enum CombatAction {
    Up, Down, Left, Right, Confirm, Cancel,
}
```

Bound to arrows + WASD + Enter/Space/Escape (`input/mod.rs:225-239`). **This enum is for MENU NAVIGATION; it is NOT the action-queue payload.** The queue payload type for #15 should be named `CombatActionKind` (or similar) to avoid name collision.

**`CombatPlugin` already exists** at `src/plugins/combat/mod.rs` and registers `StatusEffectsPlugin` as a sub-plugin via `app.add_plugins(status_effects::StatusEffectsPlugin)`. **#15 follows the same pattern**: add `TurnManagerPlugin`, `EnemyAiPlugin`, `CombatUiPlugin` as sub-plugins from inside `CombatPlugin::build`.

**How to apply:** when researching/planning #15, do not recommend defining `CombatPhase` or extending `CombatAction` enum (the leafwing version). Reference the existing declarations and explicitly call out that `EnemyTurn` is vestigial. Recommend a separately-named queue payload (`CombatActionKind`) to avoid collision.
