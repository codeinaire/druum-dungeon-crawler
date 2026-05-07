---
name: --features dev party spawn ordering in tests
description: Under --features dev, spawn_default_debug_party fires on OnEnter(Dungeon); spawn test party members BEFORE advance_into_dungeon to trigger the guard
type: feedback
---

When testing systems that query `With<PartyMember>` under `--features dev`, `PartyPlugin::spawn_default_debug_party` fires on `OnEnter(GameState::Dungeon)` and spawns 4 debug members with default-initialized `DerivedStats` (HP=0). This contaminates queries that expect only test-spawned members.

**Why:** The guard `if !existing.is_empty() { return }` at the top of `spawn_default_debug_party` skips spawning if any `PartyMember` entity already exists. If test party members are spawned AFTER `advance_into_dungeon`, the guard fires before the test members exist → debug members are created.

**How to apply:** Always spawn test party members (with known HP/state) BEFORE calling `advance_into_dungeon`. This ensures the guard triggers on OnEnter and skips the debug spawn, leaving only the test's members in scope. Applies to any test that:
1. Uses `make_test_app()` with `PartyPlugin` included
2. Asserts on `PartyMember` component state (HP, status effects, inventory)
3. Is run under `--features dev`
