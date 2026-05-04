---
name: Commands spawned in OnEnter are not visible to assertions in the same OnEnter schedule
description: Entity spawned via Commands::spawn in OnEnter(S) is not queryable from another OnEnter(S) system — commands are applied between schedules. Use Update system for post-spawn assertions.
type: feedback
---

If `system_a` on `OnEnter(State::X)` calls `commands.spawn(...)`, the entity is NOT visible to `system_b` also on `OnEnter(State::X)`, because commands are applied at the end of the schedule (between OnEnter and Update).

To assert that a spawn happened, put the assertion in an `Update` system gated on `in_state(State::X)`. It will run in the first Update frame after OnEnter, by which time the commands are applied.

Pattern used in `tests/dungeon_movement.rs`:
```rust
app.add_systems(
    Update,
    assert_party_at_entry_point.run_if(in_state(GameState::Dungeon)),
);
app.insert_resource(AssertDone(false)); // flag to run exactly once
```

**Why:** Bevy's command queue is flushed between schedule stages, not within the same stage/schedule.

**How to apply:** In any integration test that asserts on entities spawned in OnEnter, use Update + AssertDone resource flag pattern rather than OnEnter assertion.
