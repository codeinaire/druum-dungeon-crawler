---
name: DungeonAssets test wiring — Option<Res<>> means test floors are invisible without explicit resource
description: When a system uses Option<Res<DungeonAssets>>, test apps that only call build_test_floor() without inserting DungeonAssets will always see None — floor data never resolves, tests exercise the no-assets bail path instead of the intended guard.
type: feedback
---

When reviewing encounter or dungeon systems that use `Option<Res<DungeonAssets>>`, verify that test app-level tests that build floors via `Assets<DungeonFloor>::add(...)` also call `app.world_mut().insert_resource(DungeonAssets { floor_01: <handle>, ... })`. Without this, the `maybe_floor` binding is always `None` and any rate, FOE, or cell-feature guard in the system loop is never reached.

**Why:** Discovered in PR #16 review — `rate_zero_cell_no_encounter_rolls` and `foe_proximity_suppresses_rolls` both exercised the "no assets" bail path, not the rate-zero guard or FOE suppression check they were named after. Tests passed for the wrong reason.

**How to apply:** In any test that calls `build_test_floor(app, w, rate)` or inserts `Assets<DungeonFloor>` directly, check whether the system under test reads `Option<Res<DungeonAssets>>`. If so, also insert `DungeonAssets` with the returned handle wired to the correct field (`floor_01` or `floor_02`).
