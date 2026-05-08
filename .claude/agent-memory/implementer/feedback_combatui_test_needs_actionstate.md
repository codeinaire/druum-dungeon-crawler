---
name: CombatUiPlugin test harnesses need ActionState<CombatAction> initialized
description: Any test that enters GameState::Combat with CombatPlugin must init_resource::<ActionState<CombatAction>>() without ActionsPlugin
type: feedback
---

`handle_combat_input` in `CombatUiPlugin` uses `Res<ActionState<CombatAction>>`. Any test app that adds `CombatPlugin` and transitions to `GameState::Combat` will panic when this system runs unless `ActionState<CombatAction>` is registered.

**Why:** `handle_combat_input` runs `.run_if(in_state(GameState::Combat))`. Without the resource, Bevy panics when accessing `Res<ActionState<CombatAction>>` inside the system.

**How to apply:** In test harnesses that enter Combat state, add:
```rust
app.init_resource::<leafwing_input_manager::prelude::ActionState<crate::plugins::input::CombatAction>>();
```
Do NOT add `ActionsPlugin` — it registers `InputManagerPlugin::<CombatAction>` which requires `AccumulatedMouseMotion` from `InputPlugin`, causing test panics.

This pattern mirrors minimap tests (ActionState<DungeonAction> without ActionsPlugin) and the existing dungeon test harness which uses `ActionsPlugin` only because it needs actual dungeon input.

**Affected harnesses (post-Feature-#15):**
- `turn_manager.rs::app_tests::make_test_app`
- `ai.rs::app_tests::make_test_app`
- `ui_combat.rs::app_tests::make_test_app`
- Any future test that enters `GameState::Combat` with `CombatPlugin`
