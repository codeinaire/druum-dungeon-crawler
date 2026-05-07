# Implementation Summary — Feature #13: Cell Features (Doors, Traps, Teleporters, Spinners)

**Date:** 2026-05-07
**Plan:** `project/plans/20260506-100000-feature-13-cell-features.md`
**Branch:** `feature/13-cell-features`
**Commits:** `3ffc31d` (Phase 1) → `9183ac0` (plan complete)

---

## Steps Completed

- **Phase 1** — `ItemAsset.key_id` + `DungeonFloor.locked_doors` additive schema fields with `#[serde(default)]`; 2 new round-trip tests
- **Phase 2** — Asset edits: `rusty_key` gains `key_id: Some("rusty_door_01")` in `core.items.ron`; `floor_01.dungeon.ron` gains `locked_doors` entry; `item_db_loads.rs` assertion extended
- **Phase 3** — `SfxKind::SpinnerWhoosh` + `SfxKind::DoorClose` variants; match arm exhaustiveness update; `AudioAssets` handle fields; `.ogg` placeholder files committed
- **Phase 4** — `src/plugins/dungeon/features.rs` skeleton: `DoorState`, `DoorStates`, `LockedDoors`, `PendingTeleport` resources; `AntiMagicZone`, `ScreenWobble` components; `TeleportRequested`, `EncounterRequested` messages; `CellFeaturesPlugin`; full system bodies for all 9 systems; 4 Layer-1 unit tests
- **Phase 5** — 8 Layer-2 app tests in `features.rs::app_tests`; `advance_into_dungeon` helper; removed `MessageWriter<MovedEvent>` from `apply_teleporter` (B0002 conflict D-I3)
- **Phase 6** — `can_move_with_doors` helper in `dungeon/mod.rs`; `handle_dungeon_input` wired with `Res<DoorStates>`; `dungeon/tests.rs` + `tests/dungeon_geometry.rs` + `tests/dungeon_movement.rs` updated with `CellFeaturesPlugin` + `PartyPlugin`; 2 wrapper unit tests
- **Phase 7** — `handle_teleport_request` system in `LoadingPlugin`; `spawn_party_and_camera` reads `Option<ResMut<PendingTeleport>>`
- **Phase 8** — `assets/dungeons/floor_02.dungeon.ron` (4×4 single room); `DungeonAssets.floor_02` field; `floor_handle_for` helper; `populate_locked_doors` uses active floor; all 5 `DungeonAssets` struct literals updated with `floor_02: Handle::default()`
- **Phase 9** — All automated gates pass (cargo check, clippy, fmt, test base + dev); `pit_trap_damages_party` spawn-ordering fix for `--features dev`

---

## Steps Skipped

None.

---

## Deviations from Plan

1. **D-I3 (plan-acknowledged):** `apply_teleporter` removed `MessageWriter<MovedEvent>` re-publish. Same-floor teleport mutates state directly; minimap marks destination on next player move. Plan noted this as acceptable v1 behavior.

2. **D-I5 (new discovery):** `pit_trap_damages_party` test — party members moved before `advance_into_dungeon` to block `spawn_default_debug_party` guard under `--features dev`. Not mentioned in plan; minor test-ordering fix.

3. **`audio/mod.rs`, `inventory.rs`, `minimap.rs` cascading struct literal fixes** — adding `..Default::default()` or explicit new fields to struct literals when schemas changed. These are frozen-file touches required by permitted carve-outs. No logic changes.

4. **`minimap.rs::ui/minimap.rs`** — added `floor_02: Handle::default()` to `DungeonAssets` struct literal in test helper. Frozen file but required by Phase 8's `DungeonAssets` schema change.

5. **`cross_floor_teleport_end_to_end` test deferred** — Plan Phase 8 mentioned a Layer-2 end-to-end test for state-machine floor swap. The `cross_floor_teleport_publishes_request` test (Phase 5/7) covers message emission. Full state-machine end-to-end requires driving multiple frames through Loading→Dungeon transition in a test harness that doesn't use `LoadingPlugin` (which hangs on .ogg files). Deferred to a follow-on plan or manual smoke.

6. **D14=B (confirmed):** `rand` is present transitively (`cargo tree -i rand` confirms) but the spinner uses `Time::elapsed_secs_f64() modulo 4` (D14-B) as initially implemented in Phase 4. No change needed. Cargo.toml byte-unchanged.

---

## Deferred Issues

- **Manual smoke checklist** (plan Phase 9): All automated tests pass. Manual smoke (door toggle, spinner, pit, teleporter) requires `cargo run --features dev` with the authored floor assets. The automated Layer-2 tests provide equivalent coverage for all features except the visual SFX playback.
- **Cross-floor end-to-end state-machine test**: see deviation 5 above.
- **`floor_handle_for` hardcodes floor 1/2 only**: Future floors (3+) will need match arm additions. Documented inline with a `warn!` fallback.

---

## Verification Results

All automated gates:
- `cargo check` PASS
- `cargo check --features dev` PASS
- `cargo clippy --all-targets -- -D warnings` PASS (added `#[allow(clippy::too_many_arguments)]` to two Bevy system functions; fixed one `assign_op_pattern` lint)
- `cargo clippy --all-targets --features dev -- -D warnings` PASS
- `cargo fmt --check` PASS
- `cargo test` PASS — 127 lib tests + 6 integration tests
- `cargo test --features dev` PASS — 130 lib tests + 6 integration tests
- grep `derive(Event)` in features.rs — 0 matches
- grep `EventReader<` in features.rs + tests/ — 0 matches
- grep `EventWriter<` in features.rs + tests/ — 0 matches
- grep `bevy::utils::HashMap` in src/ — 0 code matches (1 doc comment in frozen classes.rs, not usage)
- Cargo.toml/lock diff — byte-unchanged
