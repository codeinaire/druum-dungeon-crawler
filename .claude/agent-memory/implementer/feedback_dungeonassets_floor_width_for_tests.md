---
name: DungeonAssets wiring exposes cell-feature bounds panics — floor must be wide enough
description: When a test wires DungeonAssets so the floor IS found, cell-feature systems (apply_alarm_trap, apply_pit_trap) index floor.features[y][x] directly and panic if MovedEvent.to.x >= floor.width
type: feedback
---

When a test inserts `DungeonAssets` pointing to a real floor handle, the cell-feature systems (`apply_alarm_trap`, `apply_pit_trap` in `features.rs:387`) become active and index `floor.features[ev.to.y as usize][ev.to.x as usize]` directly — no bounds guard.

Tests that walk the party N steps must ensure the test floor is at least N+1 cells wide, or keep `to_x` within `[0, width)`.

**Why:** Before DungeonAssets is wired, those systems bail at the `dungeon_assets: Option<Res<DungeonAssets>>` guard. After wiring, the guard passes and out-of-bounds moves panic.

**How to apply:** When building a `make_test_app_with_floor` helper (or any test that wires DungeonAssets), set `build_test_floor(app, width, rate)` with `width >= steps + 1`. For 100-step tests use width=200; for 50-step tests use width=60 or more.
