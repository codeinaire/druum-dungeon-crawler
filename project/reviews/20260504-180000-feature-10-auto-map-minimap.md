# Review: Feature #10 — Auto-Map / Minimap (PR #10)

**Date:** 2026-05-04
**Verdict:** APPROVE WITH NITS
**PR:** https://github.com/codeinaire/druum-dungeon-crawler/pull/10
**Branch:** `10-auto-map-minimap` → `main`
**Tip commit reviewed:** `dc536e4` (post-smoke-test fix — reviewed as net diff against `main`)

## Behavioral Delta

After this PR merges:

1. **Explored-cell tracking:** A new `ExploredCells` resource keyed `(floor_number, x, y) → ExploredState` is initialized by `MinimapPlugin`. Every committed translation move fires a `MovedEvent`; `update_explored_on_move` reads the message queue and marks the destination cell `Visited`. Cells with `CellFeatures.dark_zone == true` are skipped (stay `Unseen`; rendered as `?`). The resource is not reset on dungeon exit — floor data persists across F9 cycles and Town↔Dungeon transitions; "new game" reset is Feature #23.

2. **Minimap overlay:** During `DungeonSubState::Exploring`, `paint_minimap_overlay` renders a 200×200 `egui::Area` anchored to the top-right corner. Cells are filled by exploration state; walls drawn per `WallType` (`Solid`, `LockedDoor`, `SecretWall` → wall line; others → open). Player rendered as a yellow triangle rotated by `Facing`.

3. **Full-screen map:** During `DungeonSubState::Map`, `paint_minimap_full` renders a `CentralPanel` filling the entire egui canvas with the same floor grid at a larger scale.

4. **M toggle / Escape exit:** `handle_map_open_close` transitions `DungeonSubState` between `Exploring` and `Map` on `OpenMap` press, and back to `Exploring` on `Pause` (Escape) from `Map`.

5. **Movement while map open:** `handle_dungeon_input` now runs on `GameState::Dungeon` only (previously `GameState::Dungeon && DungeonSubState::Exploring`). Player can walk while looking at the map; the map updates live — Wizardry-canonical.

6. **First non-trivial dep since #5:** `bevy_egui = "=0.39.1"` with `default-features = false, features = ["render", "default_fonts"]`. `Cargo.toml` +1 line; `Cargo.lock` grows by `bevy_egui` + egui 0.33.3 + expected transitive entries.

7. **`UiPlugin` activated:** Previously an empty stub; now adds `EguiPlugin::default()`, disables auto-attach, and registers `MinimapPlugin`.

8. **Dev toggle:** `#[cfg(feature = "dev")]` F8 toggles `ExploredCells::show_full`, rendering all cells as `Visited` for debugging.

---

## What Was Reviewed

**Files with full review:**
- `src/plugins/ui/mod.rs` — UiPlugin activation + EguiGlobalSettings override
- `src/plugins/ui/minimap.rs` — all 773 lines: types, plugin, all systems, pure helpers, full test module
- `src/plugins/dungeon/mod.rs` — net diff: `pub(crate)` on `handle_dungeon_input`, doc-comment update, SubState gate removal, import cleanup
- `Cargo.toml` — `bevy_egui` line
- `Cargo.lock` — transitive dep audit

**External sources consulted:**
- `bevy_egui-0.39.1/src/lib.rs` — verified `EguiGlobalSettings::auto_create_primary_context`, `setup_primary_egui_context_system` conditional, `EguiContexts::ctx_mut()` return type, `PrimaryEguiContext` required-component `#[require(EguiContext)]`, `run_egui_context_pass_loop_system` in `PostUpdate`
- `egui-0.33.3` — verified `Frame::NONE` const exists
- `bevy_ecs-0.18.1/src/schedule/config.rs:358` — confirmed cross-schedule `.after()`/`.before()` constraints are silently ignored

**Hard gates verified:**
- `auto_create_primary_context = false` is set via `insert_resource` (overwrites the `init_resource` default that `EguiPlugin::build` installs) — resource is in final state before any system runs ✓
- `PrimaryEguiContext` carries `#[require(EguiContext)]` — inserting only the marker auto-inserts `EguiContext` ✓
- Dev `Camera2d` (debug grid HUD) cannot receive `PrimaryEguiContext`: auto-attach disabled globally; `attach_egui_to_dungeon_camera` filters `With<DungeonCamera>` ✓
- F9 cycle: `Camera3d` despawn takes `PrimaryEguiContext` with it → new `Camera3d` spawned without it → `attach_egui_to_dungeon_camera` re-attaches on next Update tick (idempotent via `Without<PrimaryEguiContext>`) ✓
- `EguiPrimaryContextPass` triggered from `run_egui_context_pass_loop_system` inside `PostUpdate` — painters always run after `Update` (`update_explored_on_move`) by schedule ordering ✓
- `handle_map_open_close` state machine: no double-transition path, no stuck state ✓
- `data/dungeon.rs` untouched — frozen-file policy honored ✓
- `dungeon/mod.rs` changes: exactly (a) `pub(crate)` on `handle_dungeon_input`, (b) doc-comment update, (c) SubState::Exploring gate removed from run_if, (d) `DungeonSubState` import removed — no other changes ✓
- `bevy_egui` features: `default-features = false, features = ["render", "default_fonts"]` ✓
- `Cargo.lock`: `bevy_egui 0.39.1` + `egui 0.33.3` + expected transitive deps (no unexpected heavy crates: no tokio, no diesel, no opencv) ✓
- `std::collections::HashMap` — correct for Bevy 0.18.1 (`bevy::utils::HashMap` removed) ✓
- `egui::Frame::new().fill(...)` used in `paint_minimap_full`, not deprecated `Frame::none()` ✓
- `egui::Frame::NONE` const verified present in egui 0.33.3 (referenced in doc, not used in final code) ✓
- `EguiPlugin::default()` — non-deprecated form ✓
- `ExploredState` default is `Unseen` — correct ✓
- `KnownByOther` variant present with no producer — intentional per D7 ✓

---

## Findings

---

### [MEDIUM] Painter `.after(update_explored_on_move)` is a cross-schedule no-op

**File:** `src/plugins/ui/minimap.rs:131,135`

**Issue:** Both painter systems are registered in `EguiPrimaryContextPass` with `.after(update_explored_on_move)`, but `update_explored_on_move` is registered in `Update`. Bevy 0.18.1 explicitly documents that cross-schedule `.after()`/`.before()` ordering constraints are **silently ignored** (`bevy_ecs-0.18.1/src/schedule/config.rs:358`: "if `GameSystem::B` is placed in a different schedule than `GameSystem::A`, any ordering calls between them—whether using `.before`, `.after`, or `.chain`—will be silently ignored").

The ordering is still **correct** — `Update` runs before `PostUpdate`, and `EguiPrimaryContextPass` is triggered from inside `PostUpdate` by `run_egui_context_pass_loop_system`. The cells updated in frame N are always visible to painters in the same frame. But the `.after()` constraints are dead code that falsely imply intra-schedule ordering is enforced, and could mislead a future reader into thinking removal is safe (it already is) or that the ordering is fragile (it isn't).

```rust
// In EguiPrimaryContextPass — the .after(...) below is silently ignored;
// update_explored_on_move is in Update, not EguiPrimaryContextPass.
paint_minimap_overlay
    .run_if(in_state(DungeonSubState::Exploring))
    .in_set(MinimapSet)
    .after(update_explored_on_move),  // <-- dead constraint
```

**Fix:** Either remove the `.after(update_explored_on_move)` calls from the painter registrations (the schedule ordering guarantees correctness without them), or replace with an explanatory comment:

```rust
// Painters intentionally have no .after() here — update_explored_on_move
// is in Update; painters are in EguiPrimaryContextPass (inside PostUpdate).
// Update always completes before PostUpdate, so ordering is guaranteed
// by schedule topology, not by intra-schedule constraints.
paint_minimap_overlay
    .run_if(in_state(DungeonSubState::Exploring))
    .in_set(MinimapSet),
```

The module-level doc already describes the schedule ordering correctly ("The schedules are naturally ordered by Bevy's main schedule pipeline") — the `.after()` calls contradict this explanation.

---

### [MEDIUM] `subscriber_flips_dest_cell_to_visited` tests the early-return path, not the happy path

**File:** `src/plugins/ui/minimap.rs:600–626`

**Issue:** The test is named `subscriber_flips_dest_cell_to_visited` but it verifies `explored.cells.len() == 0` — the early-return path when `DungeonAssets` is absent. The comment explains why (`LoadingPlugin omitted to avoid .ogg hang`), but no test in the suite exercises the actual cell-marking logic: `MovedEvent` fired → `DungeonFloor` present → `ExploredCells` gains a `Visited` entry.

The core data mutation (`explored.cells.insert(...)`) is untested at the system level. It is simple code, but a future refactor could silently break the key calculation `(floor.floor_number, ev.to.x, ev.to.y)` without any failing test.

```rust
// Current: asserts cells.len() == 0 (early return path, not the stated behavior)
#[test]
fn subscriber_flips_dest_cell_to_visited() {
    ...
    // DungeonAssets is absent ... The subscriber early-returns safely.
    app.update();
    let explored = app.world().resource::<ExploredCells>();
    assert_eq!(explored.cells.len(), 0);  // tests early-return, not the happy path
}
```

**Fix (two options):**

Option A — rename the test to accurately reflect what it tests:

```rust
#[test]
fn subscriber_early_returns_safely_when_dungeon_assets_absent() { ... }
```

Option B — add a pure-function Layer 1 test for the insert logic directly, since `update_explored_on_move` is a thin wrapper around an `explored.cells.insert`:

```rust
#[test]
fn explored_cells_insert_uses_correct_key() {
    let mut explored = ExploredCells::default();
    explored.cells.insert((1, 3, 5), ExploredState::Visited);
    assert_eq!(
        explored.cells.get(&(1, 3, 5)),
        Some(&ExploredState::Visited),
        "key is (floor_number, x, y)"
    );
    // Wrong floor number should miss
    assert_eq!(explored.cells.get(&(0, 3, 5)), None);
}
```

Option A is a one-line fix. Option B adds real coverage.

---

### [LOW] `is_solid` omits `Door` — regular doors are invisible on the minimap

**File:** `src/plugins/ui/minimap.rs:426–431`

**Issue:** `is_solid` returns `true` for `Solid`, `LockedDoor`, `SecretWall` — correctly drawing wall lines. But `Door` (unlocked, passable door) returns `false` and gets no wall line drawn. On the minimap, a doorway looks identical to an open passage. Players who walk through a door into a new room won't see any door indicator.

This is a plausible Feature #13 deferral (doors will likely get their own rendering treatment when they become interactive). But if the intent is to render doors as passable passages for v1, a comment would prevent future confusion.

```rust
fn is_solid(w: crate::data::dungeon::WallType) -> bool {
    use crate::data::dungeon::WallType;
    matches!(
        w,
        WallType::Solid | WallType::LockedDoor | WallType::SecretWall
        // WallType::Door is intentionally omitted — renders as open passage for v1.
        // Feature #13 will add door state (open/closed/locked) and can render them distinctly.
    )
}
```

**Fix:** Add the explanatory comment above. No code change needed if this is intentional.

---

### [LOW] No test for `attach_egui_to_dungeon_camera` re-attachment on F9 re-entry

**File:** `src/plugins/ui/minimap.rs` (tests module)

**Issue:** Smoke-test bug #1 (the `PrimaryEguiContext` auto-attach to the loading camera) was found manually, not by any test. The fix is correct, but there's no automated regression guard for the re-attachment lifecycle. A Layer 2 test could assert that after driving `GameState::Dungeon` (with a `DungeonCamera` entity manually inserted, to avoid pulling the full DungeonPlugin), the `PrimaryEguiContext` component is attached:

```rust
#[test]
fn attach_egui_marks_dungeon_camera_with_primary_context() {
    let mut app = make_test_app();
    // Drive to Dungeon state.
    app.world_mut()
        .resource_mut::<NextState<GameState>>()
        .set(GameState::Dungeon);
    app.update();

    // Manually spawn a Camera3d with DungeonCamera (no full DungeonPlugin needed).
    app.world_mut().spawn((Camera3d::default(), DungeonCamera));
    app.update(); // attach_egui_to_dungeon_camera runs in Update

    let count = app
        .world()
        .query::<With<PrimaryEguiContext>>()
        .iter(app.world())
        .count();
    assert_eq!(count, 1, "exactly one PrimaryEguiContext after Dungeon entry");
}
```

This would catch a regression where `auto_create_primary_context` is accidentally re-enabled or `attach_egui_to_dungeon_camera` loses its `DungeonCamera` filter.

---

## Review Summary

| Severity | Count |
|----------|-------|
| CRITICAL | 0     |
| HIGH     | 0     |
| MEDIUM   | 2     |
| LOW      | 2     |

**Verdict: APPROVE WITH NITS**

The implementation is correct. The three smoke-test bugs caught in `dc536e4` are properly fixed: `auto_create_primary_context = false` is set at plugin-build time (before any system runs); `egui::Area + allocate_exact_size` replaces the broken `Window + Frame::NONE` overlay; movement runs in both sub-states. The `PrimaryEguiContext` attach pattern is idempotent and handles the F9 re-entry cycle correctly. The `bevy_egui 0.39.1` API usage is verified against source: correct schedule (`EguiPrimaryContextPass`), correct return type (`-> Result`), correct plugin form (`EguiPlugin::default()`), and `#[require(EguiContext)]` on `PrimaryEguiContext` means inserting the marker is sufficient. The `dungeon/mod.rs` changes are minimal and exactly as scoped. `data/dungeon.rs` is untouched.

The two MEDIUM findings are worth fixing before merge: the dead cross-schedule `.after()` constraints create false documentation and a misleadingly named test creates a coverage gap for the core cell-marking logic. Neither is a correctness bug today, but both are maintenance traps. The two LOW findings (door rendering comment, re-attach test) are nice-to-have. None require blocking.
