---
name: CellFeaturesPlugin test harness requirements
description: Tests using DungeonPlugin must also include CellFeaturesPlugin + PartyPlugin + init_asset ItemDb to satisfy handle_dungeon_input's Res<DoorStates>
type: feedback
---

After Feature #13, `handle_dungeon_input` requires `Res<DoorStates>`, which is registered by `CellFeaturesPlugin`. Any test app that includes `DungeonPlugin` must ALSO include `CellFeaturesPlugin` or Bevy will panic with "Resource does not exist" for `Res<DoorStates>`.

**Why:** `can_move_with_doors` was wired into `handle_dungeon_input` as a system param dependency. If `CellFeaturesPlugin` is absent, `DoorStates` resource doesn't exist.

**How to apply:** Any test app (unit or integration) that uses `DungeonPlugin` must now include:
- `CellFeaturesPlugin` — registers `DoorStates`, `LockedDoors`, `PendingTeleport`
- `PartyPlugin` — needed because `CellFeaturesPlugin::populate_locked_doors` runs on `OnEnter(Dungeon)` (safe to omit if tests never enter Dungeon state, but risky)
- `app.init_asset::<ItemDb>()` — `PartyPlugin::populate_item_handle_registry` fires on `OnExit(Loading)` and requires `Assets<ItemDb>`

Affected files updated in Feature #13: `src/plugins/dungeon/tests.rs`, `tests/dungeon_geometry.rs`, `tests/dungeon_movement.rs`, `src/plugins/ui/minimap.rs` (had its own minimap test harness).
