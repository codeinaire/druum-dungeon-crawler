---
name: Druum DungeonSubState + DungeonAction state — fully wired pre-#10
description: Confirms what's already in place pre-Feature-#10 — DungeonSubState::Map variant exists, DungeonAction::OpenMap is bound to KeyM, all SubState variants declared, OnEnter/OnExit ready to use
type: reference
---

Verified at HEAD of `gitbutler/workspace` branch on 2026-05-04 during Feature #10 research.

**`src/plugins/state/mod.rs:17-26` — `DungeonSubState` declared with all 5 variants:**
```rust
#[derive(SubStates, Default, Debug, Clone, PartialEq, Eq, Hash)]
#[source(GameState = GameState::Dungeon)]
pub enum DungeonSubState {
    #[default]
    Exploring,
    Inventory,
    Map,
    Paused,
    EventDialog,
}
```

`OnEnter(DungeonSubState::Map)`, `OnExit(DungeonSubState::Map)`, and `in_state(DungeonSubState::Map)` all work — no further StatePlugin work required for #10.

**`src/plugins/input/mod.rs:71-82` — `DungeonAction::OpenMap` declared:**
```rust
pub enum DungeonAction {
    MoveForward,
    MoveBackward,
    StrafeLeft,
    StrafeRight,
    TurnLeft,
    TurnRight,
    Interact,
    OpenMap,         // <-- present
    OpenInventory,
    Pause,
}
```

**`src/plugins/input/mod.rs:150` — bound to `KeyCode::KeyM`:**
```rust
.with(OpenMap, KeyCode::KeyM)
```

**Other pre-wired bindings worth knowing for #10 planning:**
- `Pause` → `KeyCode::Escape` (line 152) — useful for "Escape exits map" UX
- `OpenInventory` → `KeyCode::Tab` (line 151)
- `Interact` → `KeyCode::KeyF` (line 149)

**`src/plugins/dungeon/mod.rs:192-197` — `MovedEvent` shape:**
```rust
#[derive(Message, Clone, Copy, Debug)]
pub struct MovedEvent {
    pub from: GridPosition,
    pub to: GridPosition,
    pub facing: Direction,
}
```

Derives `Message` (NOT `Event`) — Bevy 0.18 family rename. Read with `MessageReader<MovedEvent>`. Registered via `app.add_message::<MovedEvent>()` at `mod.rs:207`. Written at `mod.rs:678` after each successful translation move (NOT on turn-only moves; NOT on wall-bumps).

**Implication:** Feature #10 has zero new state-machine, input-enum, or message-type work. The only new ECS surface is `MinimapPlugin` + `ExploredCells` resource + 4-5 systems.
