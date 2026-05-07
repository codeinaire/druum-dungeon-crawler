# Review: Feature #13 — Cell Features (Doors, Traps, Teleporters, Spinners)

**PR:** https://github.com/codeinaire/druum-dungeon-crawler/pull/13
**Branch:** `feature/13-cell-features`
**Verdict:** BLOCK
**Date:** 2026-05-07

---

## What was reviewed

11 commits relative to `main` (`d16aaf8`). Source files received full coverage; `.claude/` agent memory, `project/` orchestrator docs, and asset files were skimmed for context.

**Files receiving full review:**
- `src/plugins/dungeon/features.rs` (1176 lines — new file)
- `src/plugins/dungeon/mod.rs` (933 lines — modified)
- `src/plugins/dungeon/tests.rs` (821 lines — modified)
- `src/plugins/loading/mod.rs` (196 lines — modified)
- `src/data/dungeon.rs` (833 lines — modified)
- `src/data/items.rs` (287 lines — modified)
- `src/plugins/audio/sfx.rs` (97 lines — modified)
- `assets/dungeons/floor_01.dungeon.ron`, `floor_02.dungeon.ron`

**Files receiving partial review (schema/test updates only):**
- `src/plugins/audio/mod.rs` (3-line test struct update)
- `src/plugins/ui/minimap.rs` (2-line struct init update)
- `src/data/classes.rs`, `src/plugins/party/mod.rs` (new files from prior features, not this PR's work)

---

## Behavioral delta

Before: `WallType::Door` was passable per `floor.can_move` (asset-level truth table had Door → passable). No cell-feature systems existed. Cross-floor teleport had no state-machine wiring.

After: `CellFeaturesPlugin` adds 9 systems + `tick_screen_wobble` that react to `MovedEvent`. `can_move_with_doors` wrapper in `handle_dungeon_input` overrides Door passability using `DoorStates` (default `Closed` → blocked). `handle_door_interact` toggles doors; locked doors require key lookup across party inventories. Pit/poison/alarm traps apply damage/status/encounter on cell entry. Spinner randomizes facing and attaches 200ms damped-sine camera shake. Same-floor teleporter mutates `GridPosition`/`Facing` in place; cross-floor teleporter emits `TeleportRequested`. `AntiMagicZone` marker added/removed on entry/exit. `TeleportRequested` is consumed by `handle_teleport_request` in `LoadingPlugin`.

---

## Static analysis

- `cargo fmt --check`: clean
- `cargo clippy --all-targets -- -D warnings`: clean
- `cargo test`: 127 lib + 6 integration = 133 tests passing
- `cargo test --features dev`: 130 lib + 6 integration = 136 tests passing
- Bevy 0.18 conformance gates: all pass (zero matches for `derive(Event)`, `EventReader<`, `bevy::utils::HashMap`, `add_event::<`)
- Frozen file audit: the files `state/mod.rs`, `input/mod.rs`, `audio/bgm.rs`, `ui/mod.rs`, `save/mod.rs`, `town/mod.rs`, `combat/mod.rs` are untouched. `audio/mod.rs` has a 3-line test struct update required by the new `AudioAssets` fields — same pattern as #12. `Cargo.toml` and `Cargo.lock` are byte-unchanged.

---

## Pitfall checklist (reviewer focus areas from plan)

| Pitfall | Status |
|---|---|
| 4: `can_move_with_doors` checks Door BEFORE `floor.can_move` | PASS — wrapper reads `WallType` first; Door defaults to Closed (blocks) |
| 5: `unwrap_or_default()` returns `Closed` | PASS — `DoorState::default = Closed` at `features.rs:44-49` |
| 7: `saturating_sub` for pit damage | PASS — `derived.current_hp.saturating_sub(*damage)` at `features.rs:358` |
| 8: `clear()` before `populate_locked_doors` | PASS — `locked_doors.by_edge.clear()` at `features.rs:189` |
| 9: LockedDoor+Open returns `true` | PASS for implementation (`WallType::Door \| WallType::LockedDoor` arm at `mod.rs:342`); test gap noted |
| 11: Spinner `Facing` in `Update` (before `EguiPrimaryContextPass`) | PASS — `apply_spinner` in `Update`; minimap in `EguiPrimaryContextPass` (after `PostUpdate`) |
| ScreenWobble/MovementAnimation race | PASS — `tick_screen_wobble.after(animate_movement)` at `features.rs:170` |

---

## Decision conformance

| Decision | Status |
|---|---|
| D3-α: `PendingTeleport` consumed by `spawn_party_and_camera` via `take()` | PARTIAL — `take()` is correct at `mod.rs:423`; **BUT** see HIGH finding #1 below |
| D10-A: +2 `SfxKind` variants (`SpinnerWhoosh`, `DoorClose`) | PASS |
| D11-A: `floor_02.dungeon.ron` exists, 4×4, entry (1,1) South; `DungeonAssets.floor_02` declared | PASS |
| D14-B: No `rand`; spinner uses `Time::elapsed_secs_f64() % 4` | PASS |

---

## Findings

---

### [HIGH] Cross-floor teleport state machine is broken — player lands at TitleScreen, not Dungeon

**File:** `src/plugins/dungeon/features.rs:214-222` and `src/plugins/loading/mod.rs:118,143-155`

**Issue:** D3-α requires `Loading → Dungeon` for cross-floor re-entry. Two defects prevent this:

**Defect A** — `clear_door_resources` destroys `PendingTeleport.target` on `OnExit(Dungeon)`:

```rust
fn clear_door_resources(
    mut door_states: ResMut<DoorStates>,
    mut locked_doors: ResMut<LockedDoors>,
    mut pending_teleport: ResMut<PendingTeleport>,
) {
    door_states.doors.clear();
    locked_doors.by_edge.clear();
    pending_teleport.target = None;  // ← clears destination BEFORE next OnEnter(Dungeon)
}
```

`handle_teleport_request` sets `PendingTeleport` and calls `next.set(GameState::Loading)`. Bevy then fires `OnExit(Dungeon)` → `clear_door_resources` → `pending_teleport.target = None`. By the time `OnEnter(Dungeon)` runs, the destination is gone; `spawn_party_and_camera` falls back to `floor.entry_point` on floor 1.

**Defect B** — `bevy_asset_loader` is configured `continue_to_state(GameState::TitleScreen)`. There is no redirect system that sends Loading → Dungeon when `PendingTeleport` is set. The actual flow is:

```
Dungeon →(teleport)→ Loading → TitleScreen   (not Dungeon)
```

The doc comment on `handle_teleport_request` says "Loading → Dungeon" but no code implements this path.

The automated test `cross_floor_teleport_publishes_request` only verifies message emission, not the state-machine execution. The implementation summary acknowledges the end-to-end test was deferred. The manual smoke items for cross-floor teleport are unchecked (☐) in the PR description.

**Fix — two-part:**

1. Remove `pending_teleport.target = None;` from `clear_door_resources`. `spawn_party_and_camera` already calls `pt.target.take()` which is the authoritative clear-after-use.

2. Choose one of:
   - **(Simpler — no loading flash):** In `handle_teleport_request`, call `next.set(GameState::Dungeon)` instead of `next.set(GameState::Loading)`. `OnExit(Dungeon)` still fires (despawning current party + geometry), and `OnEnter(Dungeon)` respawns with the pending target.
   - **(Preserves loading flash):** Add a system to `LoadingPlugin` on `OnExit(Loading)` that checks `PendingTeleport` and, if set, calls `next.set(GameState::Dungeon)` — ensuring it runs before bevy_asset_loader's TitleScreen transition takes effect. This requires understanding bevy_asset_loader's scheduling relative to `OnExit` systems.

---

### [MEDIUM] `tick_screen_wobble` accumulates Z-rotation drift; camera tilt persists after each spinner

**File:** `src/plugins/dungeon/features.rs:555`

**Issue:** The wobble is applied as a multiplicative compound across frames:

```rust
transform.rotation *= Quat::from_rotation_z(jitter);
```

Each frame's `jitter` value is added to the running rotation rather than replacing it. Because the damped-sine integrates to a non-zero sum (empirically ~0.043 radians / ~2.5°), the camera retains a permanent Z tilt after each spinner encounter. The drift accumulates with repeated spinner hits. The next movement animation cleans it up (because `animate_movement` snaps to `to_rotation = facing_to_quat(new_dir)` on completion), but the player stands with a visibly tilted camera while stationary after the wobble.

**Fix:** Capture a `base_rotation` in the `ScreenWobble` component (the canonical Y-facing rotation at wobble start) and apply the jitter as an absolute override each frame:

```rust
// In ScreenWobble struct, add:
pub base_rotation: Quat,

// In apply_spinner, capture it:
commands.entity(entity).insert(ScreenWobble {
    elapsed_secs: 0.0,
    duration_secs: 0.2,
    amplitude: 0.15,
    base_rotation: transform.rotation,  // snapshot canonical rotation
});

// In tick_screen_wobble:
let jitter = wobble.amplitude * envelope * oscillation;
transform.rotation = wobble.base_rotation * Quat::from_rotation_z(jitter);
```

This makes each frame idempotent: the final frame where `jitter → 0` snaps back to exactly `base_rotation`.

---

### [MEDIUM] `populate_locked_doors` ordering assumption is unverified and the comment is wrong

**File:** `src/plugins/dungeon/features.rs:194-195`

**Issue:** The comment states:

```rust
// PendingTeleport.target.take() is called by spawn_party_and_camera, which
// runs first in OnEnter(Dungeon).
```

`populate_locked_doors` and `spawn_party_and_camera` are both registered in `OnEnter(GameState::Dungeon)` via separate `add_systems` calls with no `.before()`/`.after()` ordering constraint. Bevy does not guarantee execution order between systems in separate `add_systems` calls, even when they conflict on resource access (conflict forces sequential scheduling, but the ORDER is implementation-defined).

If `populate_locked_doors` runs BEFORE `spawn_party_and_camera`, `PendingTeleport.target` is still `Some(floor_2)` and `populate_locked_doors` correctly loads floor 2's locked doors. If it runs AFTER (as the comment claims without enforcement), `target` is `None` and it silently uses floor 1's locked doors. For the current floor set this is benign (floor_02 has no locked doors), but any future floor 2+ with locked doors would silently populate the wrong lock table.

**Fix:** Add an explicit ordering constraint:

```rust
.add_systems(
    OnEnter(GameState::Dungeon),
    populate_locked_doors.after(spawn_party_and_camera),
)
```

This makes the assumption compile-time-enforced and removes the misleading comment.

---

### [MEDIUM] Missing test: `can_move_with_doors` with `LockedDoor` + `DoorState::Open`

**File:** `src/plugins/dungeon/tests.rs` (gap after line 820)

**Issue:** The plan's Pitfall 9 explicitly required two tests for `can_move_with_doors`: `Door+Closed` → blocked, and `Door+Open` → passable. Both are present. But `LockedDoor+Open` (the unlocked-locked-door path) has no test. This path is the more subtle one — `floor.can_move` returns `false` for `LockedDoor` at the asset level; the wrapper must return `true` when `DoorStates` says `Open`. The implementation is correct (`WallType::Door | WallType::LockedDoor` arm at `mod.rs:342` handles both), but the test coverage gap means a refactor could regress it silently.

**Fix:** Add to `src/plugins/dungeon/tests.rs`:

```rust
#[test]
fn can_move_with_doors_passes_unlocked_locked_door() {
    use crate::data::dungeon::WallType;
    use crate::plugins::dungeon::features::{DoorState, DoorStates};

    let mut floor = make_open_floor(2, 2);
    floor.walls[0][0].east = WallType::LockedDoor;

    let mut door_states = DoorStates::default();
    door_states.doors.insert(
        (GridPosition { x: 0, y: 0 }, Direction::East),
        DoorState::Open,
    );
    assert!(
        super::can_move_with_doors(&floor, &door_states, GridPosition { x: 0, y: 0 }, Direction::East),
        "DoorState::Open allows passage of WallType::LockedDoor (Pitfall 9)"
    );
}
```

---

### [MEDIUM] `handle_door_interact` has no app-level test

**File:** `src/plugins/dungeon/features.rs:234-325`

**Issue:** The most complex system in this PR — which walks party inventories, resolves `ItemInstance` handles, checks `ItemKind::KeyItem` + `key_id` match, and conditionally opens a `LockedDoor` — has zero integration test coverage. The unit tests (`door_state_default_is_closed`, `door_states_resource_round_trip`) only test data structures. The paths "locked door opens with correct key," "locked door blocked without key," and "plain door toggles" are all exercised only by manual smoke (which is unchecked in the PR).

This is a gap for a system with a labeled-break inner loop over inventory entities — the kind of logic that can silently fail on edge cases like empty inventory, missing item asset, or wrong `key_id`.

**Fix:** Add at least two app-level tests to `features.rs::app_tests`:
1. `door_interact_toggles_plain_door` — injects `Interact` action, verifies `DoorStates` transitions `Closed → Open`.
2. `locked_door_opens_with_matching_key` — spawns a party member with a `KeyItem` entity whose `key_id` matches, injects Interact, verifies `DoorState::Open`.

These are addable without a full `InputPlugin` by using the `Res<ActionState<DungeonAction>>`'s internal state — or by setting the action via `leafwing_input_manager`'s `ActionState::press`.

---

## What is done correctly

- **Pitfalls 4, 5, 7, 8, 9 (implementation):** All verified correct.
- **Bevy 0.18 conformance:** Zero `Event`/`EventReader`/`EventWriter`/`bevy::utils::HashMap`/`add_event` uses in new code.
- **D10-A, D11-A, D14-B:** All conform to plan decisions.
- **`can_move_with_doors` wrapper:** Correctly handles both `Door` (default-closed) and `LockedDoor` (blocks unless `DoorStates` says Open) — Pitfalls 4 and 9.
- **Frozen file discipline:** All frozen files are either untouched or have minimal required updates (test struct initialization for new `AudioAssets` fields, `floor_02` field in `DungeonAssets` test helpers).
- **`DoorState::default() = Closed`:** `#[default]` attribute verified at `features.rs:46-47`.
- **`tick_screen_wobble` ordering:** `.after(animate_movement)` correctly wins the rotation last-write race.
- **`apply_spinner` schedule:** Runs in `Update` (before `EguiPrimaryContextPass`); minimap sees new `Facing` same frame.
- **`pit_trap_damages_party` test ordering:** Correctly spawns party before `advance_into_dungeon` to prevent dev-party-spawn interference.
- **Test count:** 9 app-level tests cover pit damage, pit+teleport, poison status, alarm encounter, same-floor teleport, spinner, anti-magic lifecycle, and cross-floor message emission.
- **`populate_locked_doors` clear-first:** Idempotence on `OnEnter(Dungeon)` re-entry (Pitfall 8).
- **Key not consumed (D13):** `handle_door_interact` does not remove the key from inventory.

---

## Review Summary

| Severity | Count |
|---|---|
| CRITICAL | 0 |
| HIGH | 1 |
| MEDIUM | 4 |
| LOW | 0 |

**Verdict: BLOCK**

The single HIGH finding is the cross-floor teleport state machine: `clear_door_resources` destroys `PendingTeleport.target` on `OnExit(Dungeon)`, and `bevy_asset_loader`'s `continue_to_state(TitleScreen)` routes the player to TitleScreen instead of Dungeon. The D3-α feature is claimed as delivered in the PR description but does not work end-to-end. The fix is well-scoped (remove one line from `clear_door_resources`; change `next.set(Loading)` to `next.set(Dungeon)` in `handle_teleport_request`, or add a Loading-exit redirect system).

The MEDIUM findings are non-blocking individually but together represent meaningful test-coverage gaps (LockedDoor+Open, `handle_door_interact`) and a latent rotation-drift bug that will be visible in manual play. Recommend addressing the `ScreenWobble` drift before the feature is considered shippable, as it produces a visually noticeable camera tilt after every spinner encounter.

---

**Files reviewed (full):**
- `/Users/nousunio/Repos/Learnings/claude-code/druum/src/plugins/dungeon/features.rs`
- `/Users/nousunio/Repos/Learnings/claude-code/druum/src/plugins/dungeon/mod.rs`
- `/Users/nousunio/Repos/Learnings/claude-code/druum/src/plugins/dungeon/tests.rs`
- `/Users/nousunio/Repos/Learnings/claude-code/druum/src/plugins/loading/mod.rs`
- `/Users/nousunio/Repos/Learnings/claude-code/druum/src/data/dungeon.rs`
- `/Users/nousunio/Repos/Learnings/claude-code/druum/src/data/items.rs`
- `/Users/nousunio/Repos/Learnings/claude-code/druum/src/plugins/audio/sfx.rs`
