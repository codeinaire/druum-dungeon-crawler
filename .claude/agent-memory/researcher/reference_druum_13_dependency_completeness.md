---
name: Druum Feature #13 dependencies are unusually complete pre-implementation
description: Five of six "data" pieces #13 needs already exist; floor_01.dungeon.ron was authored to test #13 specifically; minimap dark-zone gate already lives in #10
type: reference
---

When researching Feature #13 (Cell Features — Doors/Traps/Teleporters/Spinners), nearly every input is already shipped. **Read the live source before designing.** Specific verified pre-shipped pieces:

- `WallType::{Door, LockedDoor, SecretWall, OneWay, Illusory}` — `src/data/dungeon.rs:85-104`
- `TrapType::{Pit, Poison, Alarm, Teleport(TeleportTarget)}` — `dungeon.rs:135-149`
- `TeleportTarget { floor, x, y, facing }` — `dungeon.rs:123-130`
- `CellFeatures { trap, teleporter, spinner, dark_zone, anti_magic_zone, encounter_rate, event_id }` — `dungeon.rs:156-174`
- `DungeonAction::Interact` bound to `KeyCode::KeyF` — `src/plugins/input/mod.rs:78, 149`
- `MovedEvent { from, to, facing }` published on commit frame — `dungeon/mod.rs:192-197, 686-690`
- `ItemKind::KeyItem` — `src/plugins/party/inventory.rs:79`
- `ItemHandleRegistry::get(&str) -> Option<&Handle<ItemAsset>>` — `inventory.rs:508-510`
- Dark-zone gate already implemented — `src/plugins/ui/minimap.rs:208-211`
- `floor_01.dungeon.ron` already includes Door (1,1)-East, LockedDoor (3,1)-East, spinner (2,2), Pit (4,4), Teleporter (5,4)→floor 2, dark_zone (1,4), anti_magic_zone (2,4) — verified by reading the file header doc and `dungeon.rs:790-809` integration test

**What's MISSING that #13 must add:**
- `key_id: Option<String>` field on `ItemAsset` (for locked doors to know which key opens which door)
- `locked_doors: Vec<((u32,u32), Direction, String)>` field on `DungeonFloor` (the door_id side-table)
- `SfxKind::SpinnerWhoosh` and `SfxKind::DoorClose` variants
- `floor_02.dungeon.ron` (target of cross-floor teleport — does not exist in DungeonAssets)
- `DoorStates: Resource(HashMap)` for runtime door open/closed state
- `LockedDoors: Resource` populated from floor's `locked_doors`
- `AntiMagicZone` marker component (no consumer yet — plumbing for #14/#15)
- `TeleportRequested` and `EncounterRequested` Messages
- `ScreenWobble` component for spinner camera shake (200ms damped sine)

**Lesson:** Skip the temptation to "design it from scratch". The existing data model is the design. #13's job is wiring, not authoring.
