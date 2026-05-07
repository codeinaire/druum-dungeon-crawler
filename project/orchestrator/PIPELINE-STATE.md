# Pipeline State

**Task:** Drive research → plan pipeline (PAUSE at plan-approval, do NOT implement) for Feature #13: Cell Features (Doors, Traps, Teleporters, Spinners) from the Druum Bevy 0.18 dungeon-crawler roadmap. Roadmap §13 at lines 688-737. Difficulty 3/5. Depends on #4 (grid model — `WallType`/`TrapType`/`TeleportTarget`/`CellFeatures` all already shipped), #7 (`MovedEvent` + commit-frame logical state), #8 (3D renderer + `DungeonGeometry` despawn-recursive), #9 (lighting), #10 (minimap — dark-zone gate ALREADY DONE at `minimap.rs:208-211`), #11 (party + `DerivedStats.current_hp` saturating, `StatusEffects.effects.push`), #12 (`ItemKind::KeyItem`, `Inventory(Vec<Entity>)`, `ItemHandleRegistry`). Locked UX: telegraphed spinners (Resolved §4); auto-map facing free via direct `&Facing` Query (`minimap.rs:269,309,318`). Architecture: single new file `src/plugins/dungeon/features.rs` (~400-600 LOC) holding 7-8 systems + 3 resources (`DoorStates`, `LockedDoors`, `PendingTeleport`) + 1 component (`AntiMagicZone`) + 1 marker (`ScreenWobble`) + 2 messages (`TeleportRequested`, `EncounterRequested`); 1 small carve-out into `LoadingPlugin` for cross-floor teleport (D3 Option α); 1 `dungeon/mod.rs::handle_dungeon_input` edit (D9b — `can_move_with_doors` wrapper); 2 additive `#[serde(default)]` schema fields (`ItemAsset.key_id`, `DungeonFloor.locked_doors`); 2 new `SfxKind` variants (`SpinnerWhoosh`, `DoorClose`); 2 new .ogg placeholders. Constraint envelope: 0 new deps (pending D14 `cargo tree -i rand` verification), +400-700 LOC, +6-9 tests, +0.3s compile. Final report at plan-approval MUST be self-contained because `SendMessage` does not actually resume returned agents (confirmed across Features #3-#12); parent dispatches implementer manually after approval.

**Status:** plan APPROVED 2026-05-07 — implementer dispatched
**Last Completed Step:** 6 (pipeline summary written; plan Status: Approved)

## Artifacts

| Step | Description      | Artifact                                                                                                            |
| ---- | ---------------- | ------------------------------------------------------------------------------------------------------------------- |
| 1    | Research         | /Users/nousunio/Repos/Learnings/claude-code/druum/project/research/20260506-080000-feature-13-cell-features.md |
| 2    | Plan             | /Users/nousunio/Repos/Learnings/claude-code/druum/project/plans/20260506-100000-feature-13-cell-features.md (Status: Approved — D3=α, D10=A, D11=A, D14=A) |
| 6    | Pipeline summary | /Users/nousunio/Repos/Learnings/claude-code/druum/project/orchestrator/20260506-100000-feature-13-cell-features-research-plan.md |
| 3    | Implement        | NOT IN SCOPE (parent dispatches after plan approval)                                                                |
| 4    | Ship             | NOT IN SCOPE                                                                                                        |
| 5    | Code Review      | NOT IN SCOPE                                                                                                        |

## User Decisions

**All decisions confirmed 2026-05-07.** Genuine USER PICKs: **D3 = α** (re-enter `GameState::Loading` via `LoadingPlugin` carve-out + `PendingTeleport` resource), **D10 = A** (+2 `SfxKind` variants — `SpinnerWhoosh`, `DoorClose`), **D11 = A** (author minimal 4×4 `floor_02.dungeon.ron` + extend `DungeonAssets.floor_02`), **D14 = A** (run `cargo tree -i rand` at start of Phase 5; fall back to D14-B `Time::elapsed_secs_f64()` modulo if absent — implementer reports outcome before writing `apply_spinner`). Recommended defaults accepted: D1 (per-floor-instance door state), D2 (`key_id: Option<String>` on `ItemAsset`), D2b (`DungeonFloor.locked_doors` side-table), D5 (publish `EncounterRequested` + log-only consumer), D6 (rotation-jitter camera shake, 200ms damped sine), D7 (royalty-free .ogg files), D9 (`DoorStates: Resource(HashMap)`), D12 (naive push poison stack), D13 (key NOT consumed — Wizardry-style), D15 (closed-by-default; Interact opens). Auto-resolved by live code: D2c (additive `#[serde(default)]`), D4 (dark zones already done at `minimap.rs:208-211`; anti-magic stub-now), D8 (Interact already wired at `input/mod.rs:78,149`), D9b (side-effect of D9 — `can_move_with_doors` wrapper edit to `handle_dungeon_input`).

## Pipeline Scope

This invocation runs research → plan → STOP. After plan approval, parent will manually dispatch implementer (per established Feature #3-#12 pattern). The orchestrator pipeline summary at the end of this run must be self-contained.

## Critical context for resumption

- **Live ground truth (verified by research, file:line):**
  - `WallType::{Door, LockedDoor}` exist in `src/data/dungeon.rs:85-104`. `Door` is currently passable per `can_move`; `LockedDoor` is impassable. #13 changes both via runtime `DoorStates` overlay.
  - `TrapType::{Pit{damage,target_floor}, Poison, Alarm, Teleport(TeleportTarget)}` and `TeleportTarget{floor,x,y,facing}` fully defined in `dungeon.rs:123-149`.
  - `CellFeatures` has all needed fields: `trap, teleporter, spinner, dark_zone, anti_magic_zone, encounter_rate, event_id` (`dungeon.rs:156-174`).
  - `MovedEvent { from, to, facing }` derives `Message`, published on **commit frame** at `dungeon/mod.rs:686-690` (NOT after tween). `handle_dungeon_input` is `pub(crate)` at `dungeon/mod.rs:618` for `.after(...)` ordering.
  - `DungeonAction::Interact` exists at `input/mod.rs:78`, bound to `KeyCode::KeyF` (line 149). **D8 auto-resolved.**
  - `ItemKind::KeyItem` exists at `inventory.rs:79`. `Inventory(Vec<Entity>)` + `ItemInstance(Handle<ItemAsset>)`. `ItemHandleRegistry` at `inventory.rs:500-521`. `ItemAsset` has 9 fields (NO `key_id` yet — #13 adds it as #[serde(default)]).
  - `rusty_key` exists in `assets/items/core.items.ron` with `kind: KeyItem, slot: None` — #13 adds `key_id: Some("rusty_door_01")`.
  - `floor_01.dungeon.ron` already authored as a #13 testbed: Door at (1,1)/(2,1) East, LockedDoor at (3,1)/(4,1) East, spinner at (2,2), Pit at (4,4) targeting floor 2, Teleporter at (5,4) → floor 2, dark_zone at (1,4), anti_magic_zone at (2,4). **D11: floor_02 does NOT exist** — recommend authoring minimal 4×4 (~80 lines) for cross-floor test.
  - **Dark zones ALREADY DONE at `minimap.rs:208-211`** — #13 adds zero work for dark zones. **D4 mostly auto-resolved.**
  - Minimap reads `&Facing` directly via Query (`minimap.rs:269, 309, 318`) — spinner facing change reflects same frame, free. No `SpunEvent` needed.
  - `SfxKind` has 5 variants (`Footstep, Door, EncounterSting, MenuClick, AttackHit`); `AudioAssets` has matching 5 sfx_* handle fields. #13 adds 2 (`SpinnerWhoosh`, `DoorClose`) — touches `audio/sfx.rs` AND `loading/mod.rs` (the LoadingPlugin "freeze" applies to state-transition logic, not the AudioAssets field list).
  - `DerivedStats.current_hp` saturating-sub clamp pattern at `character.rs:374-381`. `StatusEffectType::Poison` + `ActiveEffect{effect_type, remaining_turns:Option<u32>, magnitude:f32}` at `character.rs:235-274`. `StatusEffects.effects.push(...)` is canonical apply path.
- **Recommended architecture:** single-file `src/plugins/dungeon/features.rs` with `CellFeaturesPlugin` registered in `main.rs`. 7 systems (`handle_door_interact`, `apply_pit_trap`, `apply_poison_trap`, `apply_alarm_trap`, `apply_teleporter`, `apply_spinner`, `apply_anti_magic_zone`) all `.run_if(in_state(GameState::Dungeon)).after(handle_dungeon_input)` plus `tick_screen_wobble` (Update, no ordering). Cross-floor teleport: re-enter `GameState::Loading` via `TeleportRequested` Message that LoadingPlugin owns the consumer for + `PendingTeleport: Resource` read on next `OnEnter(Dungeon)` (D3 Option α). Doors: `DoorStates: Resource(HashMap<(GridPos, Direction), DoorState>)` with `Closed | Open` (D9). Locked-door key check: `key_id: Option<String>` field on `ItemAsset` paired with `door_id: String` side-table on `DungeonFloor::locked_doors` (D2 + D2b). `handle_dungeon_input` calls a new `can_move_with_doors` wrapper instead of `floor.can_move` directly (D9b — largest single edit to a frozen module).
- **Decisions surfaced (D1-D15):**
  - **AUTO-RESOLVED:** D2c (additive #[serde(default)]), D4 (dark already done; anti-magic stub-now), D8 (Interact already wired), D9b (side-effect of D9), D14 (verify rand at impl time — likely transitively present)
  - **GENUINE Cat-B (recommended defaults):** D1 (per-floor-instance), D2 (key_id on ItemAsset), D2b (DungeonFloor.locked_doors side-table), D3 (Option α — re-enter Loading), D5 (publish EncounterRequested + log-only consumer), D6 (rotation-jitter camera shake), D7 (royalty-free .ogg matching #6 pipeline), D9 (DoorStates Resource), D10 (+2 SfxKind variants — SpinnerWhoosh + DoorClose), D11 (author minimal floor_02), D12 (naive push poison stack), D13 (key NOT consumed — Wizardry-style), D15 (closed-by-default, Interact to open)
  - **Implementer-time check:** D14 (`cargo tree -i rand` BEFORE writing spinner code; if absent, fall back to `Time::elapsed_secs_f64()` modulo)
  - **Possible D9c surfaced by OQ6:** wall geometry update on `DoorState::Open` — despawn the wall plate vs. swap material vs. leave as-is. Default: leave as-is (player notices via SFX); planner can refine.
- **Risks (TOP 3):**
  1. The `dungeon/mod.rs::handle_dungeon_input` D9b edit (taking `Res<DoorStates>` + replacing `floor.can_move` with `can_move_with_doors`) is the largest "frozen module" edit. Mitigation: `DoorStates::default()` is empty-doors, which produces existing behavior — non-Door walls pass through unchanged.
  2. `ScreenWobble` and `MovementAnimation` both mutate `Transform::rotation` — order with `.after(animate_movement)`.
  3. `populate_locked_doors` must clear-first to be idempotent across teleport re-entries (mirrors `populate_item_handle_registry` at `inventory.rs:539`).
- **GitButler discipline:** implementer + shipper must use `but` not `git` (pre-commit hook on `gitbutler/workspace` blocks raw `git commit`). Working tree currently clean, on `gitbutler/workspace`, `zz` empty, local main even with origin/main per the prior pipeline state doc.
