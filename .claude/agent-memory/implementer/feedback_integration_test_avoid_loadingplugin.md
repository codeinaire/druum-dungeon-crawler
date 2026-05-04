---
name: Integration tests that need dungeon data should avoid LoadingPlugin — use private TestState instead
description: LoadingPlugin loads AudioAssets (.ogg files) which hangs in headless CI. Use private TestState + TestFloorAssets pattern from dungeon_floor_loads.rs.
type: feedback
---

`LoadingPlugin` loads both `DungeonAssets` and `AudioAssets` (audio .ogg files). In headless tests, the audio system is unavailable and .ogg loading hangs forever (no AppExit written, no timeout triggered).

The correct pattern for integration tests that need dungeon floor data:
1. Use a private `TestState { Loading, Loaded }` enum
2. Define a `TestFloorAssets` struct with only `floor: Handle<DungeonFloor>`  
3. `init_state::<TestState>()` + `add_loading_state(TestState::Loading → Loaded).load_collection::<TestFloorAssets>()`
4. On `OnEnter(TestState::Loaded)`: insert a stub `DungeonAssets` with `floor_01: floor_assets.floor.clone()` and default handles for other fields
5. Then set `GameState::Dungeon`

**Why:** LoadingPlugin's AudioAssets includes 10 `.ogg` file handles. On headless CI, the audio backend is not available, so these assets never load, blocking the state transition forever.

**How to apply:** Any future integration test that needs `DungeonFloor` data should mirror `tests/dungeon_floor_loads.rs` + `tests/dungeon_movement.rs` — NOT use `LoadingPlugin`.

Reference: `tests/dungeon_movement.rs` (Feature #7) for the complete pattern.
