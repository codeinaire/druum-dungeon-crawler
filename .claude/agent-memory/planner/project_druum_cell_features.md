---
name: Druum cell features (Feature #13) decisions
description: Feature #13 plan — D3-α LoadingPlugin carve-out, D9b can_move_with_doors wrapper, additive serde-default schema edits, single-file features.rs, +0 deps in recommended path
type: project
---

Plan-of-record decisions for Feature #13 (Cell Features — Doors, Traps, Teleporters, Spinners) on top of Druum's #4/#7/#8/#10/#12 stack:

**Why:** Captured at planning time so future planners (or a returning implementer) can recall *why* the plan structure is what it is — particularly the carve-outs into otherwise-frozen files and the genuine USER PICK escalations.

**How to apply:** Read before planning #14 (status effects), #15 (combat — first reader of `EncounterRequested`), #23 (save/load — `DoorStates` and `LockedDoors` are floor-scoped and intentionally not persisted), #25 (UI — door-open visual swap deferred here).

## Architecture decisions (10 recommended defaults + 4 USER PICK)

### Auto-resolved (no user input needed)
- **D2c** — Adding `locked_doors: Vec<((u32,u32), Direction, String)>` to `DungeonFloor` is acceptable per project precedent for additive `#[serde(default)]` schema edits. Existing `is_well_formed` and `validate_wall_consistency` are unaffected (no per-cell shape constraint).
- **D4** — Dark zones already handled by #10's minimap subscriber at `minimap.rs:208-211`; #13 only adds anti-magic zone plumbing (no v1 consumer).
- **D8** — `DungeonAction::Interact` already exists at `input/mod.rs:78`, bound to `KeyCode::KeyF` at line 149.
- **D9b** — Side-effect of D9; `handle_dungeon_input` MUST consult `DoorStates` via a new `can_move_with_doors` wrapper.

### Recommended defaults (proceed unless user objects)
- **D1** — Door state per-floor-instance only (cleared `OnExit(Dungeon)`).
- **D2** — `key_id: Option<String>` field on `ItemAsset` paired with `ItemKind::KeyItem`.
- **D2b** — Side-table `locked_doors` on `DungeonFloor` (additive); does NOT change `WallType::LockedDoor` to `LockedDoor(String)`.
- **D5** — Publish `EncounterRequested { source: AlarmTrap }` with logged-only consumer; #16 wires the real combat trigger.
- **D6** — Camera shake via `Quat::from_rotation_z(jitter)` rotation jitter, 200ms damped sine.
- **D7** — Royalty-free `.ogg` files committed to `assets/audio/sfx/` (matches #6 precedent).
- **D9** — `DoorStates: Resource(HashMap<(GridPosition, Direction), DoorState>)`.
- **D12** — Naive push poison stacking (matches #14's expected behavior).
- **D13** — Locked-door keys NOT consumed (Wizardry-style; reusable).
- **D15** — Doors closed-by-default; player presses Interact to open.

### Genuine USER PICK (surface for confirmation)
- **D3** — Cross-floor teleporter: α (re-enter Loading) recommended. β (in-state swap) requires ~80 LOC refactor and bypasses bevy_asset_loader guarantees.
- **D10** — +2 `SfxKind` variants (SpinnerWhoosh + DoorClose) recommended over +5 or +1.
- **D11** — Author `floor_02.dungeon.ron` (~80 lines) recommended; alternatives are deferred manual smoke or mock floor 2 in tests.
- **D14** — Verify `rand` via `cargo tree -i rand` BEFORE writing spinner. Recommended path: present (transitive). Fallback: `Time::elapsed_secs_f64() as u64 % 4`. Last-resort: `+rand = "0.8"` direct dep (breaks 0-deps signal).

## Frozen file carve-outs (4 explicit modifications expected)

**Why these are touched despite "frozen" status:** the Frozen list documents *what should not be touched in the typical case*. #13 has four bounded touches:
1. `src/main.rs` — +1 line `app.add_plugins(CellFeaturesPlugin)`.
2. `src/plugins/dungeon/mod.rs` — `pub mod features` + Phase 6 D9b `can_move_with_doors` wrapper + Phase 7 `PendingTeleport` read in `spawn_party_and_camera` + Phase 8 `floor_handle_for` helper.
3. `src/plugins/loading/mod.rs` — +2 `AudioAssets` SFX fields + Phase 7 `handle_teleport_request` system + Phase 8 `floor_02: Handle<DungeonFloor>` field. Justified because state-machine integration is `LoadingPlugin`'s job, and AudioAssets fields were added in #6 *after* the freeze comment was authored.
4. `src/data/dungeon.rs` and `src/data/items.rs` — single additive `#[serde(default)]` field each. Existing tests pass unchanged.

**How to apply:** when planning #14+, do NOT re-modify these touched lines without re-reading the rationale. The Phase 6 D9b wrapper is the largest single frozen-file edit and changes the runtime semantics of `WallType::Door` (asset-passable → runtime closed-by-default). #14 building on top of #13 should treat `DoorStates` as a read-only consumer and never bypass `can_move_with_doors`.

## Implementation patterns to mirror

- **9 phases, each an atomic commit boundary.** Mirrors #11/#12. `cargo test` passes at every phase exit.
- **Single-file features.rs** at `src/plugins/dungeon/features.rs` — 3 resources + 2 components + 2 messages + 9 systems + tests, ~400-600 LOC. Same single-file precedent as #9, #10, #11, #12.
- **Schema-additive Phase 1** — extend `data/dungeon.rs` and `data/items.rs` first with `#[serde(default)]` fields. Verifies in isolation; no consumer wiring yet.
- **`populate_X` clear-first idempotence pattern** — `populate_locked_doors` mirrors `populate_item_handle_registry` at `inventory.rs:539`. Required for cross-floor re-entry (Pitfall 8).
- **Layer 1/2/3 test split** — Layer-1 pure (no App), Layer-2 MinimalPlugins+AssetPlugin+StatesPlugin+PartyPlugin+CellFeaturesPlugin (no InputPlugin), Layer-3 only for `handle_door_interact` (or mock `init_resource::<ActionState<DungeonAction>>`).

## Things NOT to repeat in #14+ planning

- Do NOT mutate `DungeonFloor::walls` at runtime — read-only contract; door state lives in `DoorStates: Resource`.
- Do NOT add `bevy::utils::HashMap` — removed in 0.18; use `std::collections::HashMap`. Verification gate greps for this.
- Do NOT use `#[derive(Event)]` or `EventReader<T>`/`EventWriter<T>` — Bevy 0.18 family rename. Use `Message` / `MessageReader` / `MessageWriter`.
- Do NOT short-circuit on `floor.can_move == false` for `WallType::LockedDoor` — the wrapper must check `DoorStates` first (Pitfall 9).
- Do NOT consume the key on locked-door unlock (D13). Wizardry-style; reusable.
- Do NOT hand-roll a per-cell `OnEnter` schedule — Bevy doesn't have one. Use `MovedEvent` subscribers exclusively.
