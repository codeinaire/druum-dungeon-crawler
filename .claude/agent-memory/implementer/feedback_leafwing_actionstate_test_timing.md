---
name: leafwing ActionState test pattern — press/update/release/update for single keypress
description: Without leafwing tick machinery, JustPressed never advances; use press/update/release/update cycle to simulate a clean single keypress without double-firing
type: feedback
---

When testing input-driven systems that check `action_state.just_pressed(&action)`:

1. Exclude `ActionsPlugin` (and `InputPlugin`) from test apps entirely — leafwing registers mouse systems that need `AccumulatedMouseMotion` resource which `MinimalPlugins` does not provide; including them panics.
2. Insert `ActionState<DungeonAction>` directly via `app.init_resource::<ActionState<DungeonAction>>()`.
3. Use the press/update/release/update pattern to simulate a single keypress:

```rust
app.world_mut().resource_mut::<ActionState<DungeonAction>>()
    .press(&DungeonAction::OpenMap);
app.update();  // system observes just_pressed, fires, queues state transition
app.world_mut().resource_mut::<ActionState<DungeonAction>>()
    .release(&DungeonAction::OpenMap);
app.update();  // StateTransition realizes the queued state change
```

**Why:** Without leafwing's internal tick system running, `JustPressed` state never advances to `Pressed` between updates. Pressing and then calling `update()` twice without releasing leaves `JustPressed` active for both frames, causing input handlers that fire on `just_pressed` to fire twice, oscillating through state transitions.

**How to apply:** Any test that exercises `just_pressed`-gated logic must use the press/update/release/update cycle. For multi-key sequences (e.g., press M, then press Escape), apply the cycle once per logical keypress in sequence.
