# Plan: Feature #13 — Cell Features (Doors, Traps, Teleporters, Spinners)

**Date:** 2026-05-06
**Status:** Approved (2026-05-07) — D3=α, D10=A, D11=A, D14=A; all 10 recommended defaults accepted
**Research:** `project/research/20260506-080000-feature-13-cell-features.md`
**Roadmap:** `project/roadmaps/20260429-01-bevy-dungeon-crawler-roadmap.md` lines 688-737
**Depends on:** Feature #4 (dungeon grid), Feature #7 (grid movement + `MovedEvent`), Feature #8 (3D renderer + wall materials), Feature #10 (minimap + dark-zone gate already wired), Feature #12 (inventory `Inventory(Vec<Entity>)` + `ItemInstance(Handle)`)

---

## Goal

Land the **cell-feature reaction layer** on top of #4/#7/#8/#10/#12. Create a single new file `src/plugins/dungeon/features.rs` containing seven `MovedEvent`-driven systems plus one `Interact`-driven door system, gated `.run_if(in_state(GameState::Dungeon))` and ordered `.after(handle_dungeon_input)`. Add `key_id: Option<String>` to `ItemAsset` (additive, `#[serde(default)]`) and `locked_doors: Vec<((u32,u32), Direction, String)>` to `DungeonFloor` (additive, `#[serde(default)]`). Add 2 `SfxKind` variants (`SpinnerWhoosh`, `DoorClose`) with corresponding `.ogg` placeholder assets. Add a `CellFeaturesPlugin` that owns `DoorStates`/`LockedDoors`/`PendingTeleport` resources, `AntiMagicZone`/`ScreenWobble` components, and `TeleportRequested`/`EncounterRequested` messages. Modify `dungeon/mod.rs::handle_dungeon_input` (D9b) to consult `DoorStates` for door passability; modify `loading/mod.rs` to own the cross-floor teleport handler (D3 Option α). Author a minimal `floor_02.dungeon.ron` (~80 lines) and add `floor_02: Handle<DungeonFloor>` to `DungeonAssets` for cross-floor end-to-end testing. **Zero new dependencies (D14 verified at impl-time).**

---

## Approach

**Architecture Option D3-α (re-enter `GameState::Loading` for cross-floor teleport)** with a tightly-scoped carve-out into `LoadingPlugin`. The carve-out is justified because state-machine integration is `LoadingPlugin`'s job, the `Loading -> Dungeon` re-entry already runs `OnEnter(Dungeon)` (`spawn_party_and_camera + spawn_dungeon_geometry`) which is despawn-recursive on `OnExit(Dungeon)` — reuses 100% of existing spawn machinery. The brief loading flash is genre-correct (Wizardry/Etrian).

**Door state shape D9-A:** a `DoorStates: Resource(HashMap<(GridPosition, Direction), DoorState>)` keyed by edge. Cleared on `OnExit(GameState::Dungeon)`. `DoorState::Closed` is the default (Pitfall 5 / D15: closed-by-default; Interact opens). The runtime override is what makes a `WallType::Door` actually closed — pre-#13, `floor.can_move` returned `true` for Door because the asset-level "closed but unlocked" reads as passable (Pitfall 4). `handle_dungeon_input` now consults `DoorStates` via a new `can_move_with_doors(floor, doors, pos, dir)` wrapper (D9b — the largest single edit to a previously-frozen module).

**Key item shape D2-A:** add `key_id: Option<String>` (`#[serde(default)]`) to `ItemAsset`. Author `key_id: Some("rusty_door_01")` on the existing `rusty_key`. `LockedDoors: Resource` (D2b) holds a HashMap<(GridPosition, Direction), String> populated on `OnEnter(Dungeon)` from a new `locked_doors: Vec<((u32,u32), Direction, String)>` field on `DungeonFloor` (D2c — additive, existing tests pass via `#[serde(default)]`). The Interact handler walks all party inventories, resolves each `ItemInstance(Handle)` to an `ItemAsset`, checks `kind == KeyItem && key_id == Some(door_id)`. Keys are NOT consumed (D13 — Wizardry-style; reusable).

**Telegraphed spinner D6-A:** `apply_spinner` mutates `Facing` on the commit frame, emits `SfxRequest::SpinnerWhoosh`, and attaches a `ScreenWobble { duration_secs: 0.2, amplitude: 0.15 }` component. A separate `tick_screen_wobble` system applies a damped-sine `Quat::from_rotation_z` jitter to the player's `Transform::rotation`, then removes the component when `t >= 1.0`. Ordered `.after(animate_movement)` to win the rotation last-write race (Risk register #ScreenWobble vs MovementAnimation).

**Single-file features.rs precedent** matches #11/#12 (single-file plugin). 3 resources + 2 components + 2 messages + 9 systems + ~6-8 tests fit comfortably under ~600 LOC.

---

## Critical

These are non-negotiable constraints. Violations should fail review:

- **Bevy `=0.18.1` pinned.** No version bump.
- **Zero new dependencies preferred.** D14 (`rand`) is the only candidate; verify at impl-time via `cargo tree -i rand` from project root. If `rand` is transitively present, use it directly. If not, fall back to `Time::elapsed_secs_f64() as u64 % 4` for spinner randomness (Standard Stack §rand). Add `rand = "0.8"` only with explicit user opt-in (surfaces as +1 dep in the verification gate).
- **`#[derive(Message)]`, NOT `#[derive(Event)]`** — Bevy 0.18 family rename. Read with `MessageReader<T>`, write with `MessageWriter<T>`. Register with `app.add_message::<T>()`. The verification gate greps for `derive(Event)` and `EventReader<` — both must return zero matches in `features.rs` and any tests.
- **`std::collections::HashMap`, NOT `bevy::utils::HashMap`** — removed in 0.18. Verification gate greps for `bevy::utils::HashMap` across `src/` — must return zero matches.
- **`Door` is currently passable per `can_move` (Pitfall 4).** The `can_move_with_doors` wrapper in Phase 6 must override passability based on `DoorStates`. Default `DoorState::Closed` blocks passage of `WallType::Door` walls — this is the desired runtime behavior (D15: closed-by-default). The wrapper must NOT short-circuit on `floor.can_move == false` for `WallType::LockedDoor` before checking `DoorStates`; an unlocked locked-door (`DoorStates[(pos, dir)] == DoorState::Open`) must allow passage even though `floor.can_move` returns `false` for `LockedDoor` (Pitfall 9).
- **Saturating arithmetic on pit damage (Pitfall 7).** `apply_pit_trap` MUST use `derived.current_hp.saturating_sub(damage)`. Naive `-=` underflows `u32` when `damage > current_hp` and produces wraparound. Same project pattern as `character.rs:374-381`.
- **`populate_locked_doors` must clear-first (Pitfall 8).** Cross-floor teleport re-enters `OnEnter(Dungeon)`, which would otherwise stack duplicate entries in `LockedDoors.by_edge`. Mirror `populate_item_handle_registry` clear-first pattern at `inventory.rs:539`.
- **`apply_spinner` runs BEFORE the minimap painter (Pitfall 11).** Already automatic via Bevy schedule order: spinner is in `Update`; the minimap painter runs in `EguiPrimaryContextPass` (after `PostUpdate`). The minimap reads `&Facing` directly (`minimap.rs:269, 309, 318`), so the spun facing reflects on the same frame. Verify via Layer-2 test `spinner_randomizes_facing`.
- **Pre-commit hook on `gitbutler/workspace` blocks raw `git commit`** (CLAUDE.md). Implementer must use `but commit --message-file <path>`.

---

## Frozen post-#12 / DO NOT TOUCH

These files are frozen by Features #1–#12 and must not be modified by the #13 implementer except where explicitly listed below. The research doc enumerates these (research §"Files NOT touched"). Diff verification at the end of Phase 9 must show ZERO frozen-file modifications except the four explicit carve-outs.

- `src/plugins/state/mod.rs` — **FROZEN by #2.**
- `src/plugins/input/mod.rs` — **FROZEN by #5.** `DungeonAction::Interact` already exists at line 78 and is bound to `KeyCode::KeyF` at line 149.
- `src/plugins/audio/mod.rs`, `src/plugins/audio/bgm.rs` — **FROZEN by #6.** Only `audio/sfx.rs` is touched.
- `src/plugins/ui/mod.rs`, `src/plugins/ui/minimap.rs` — **FROZEN by #10.** Dark-zone gate already implemented at `minimap.rs:208-211`.
- `src/plugins/save/mod.rs`, `src/plugins/town/mod.rs`, `src/plugins/combat/mod.rs` — **FROZEN / does not exist yet.**
- `src/plugins/party/mod.rs`, `src/plugins/party/character.rs`, `src/plugins/party/inventory.rs` — **FROZEN by #11/#12.** Read-only for #13. The `Inventory(Vec<Entity>)`, `ItemInstance(Handle)`, `ItemKind::KeyItem`, `ItemHandleRegistry`, and `derive_stats` shapes are all consumed but NOT modified.
- `src/data/classes.rs`, `src/data/spells.rs`, `src/data/enemies.rs` — **FROZEN-from-day-one** per project precedent.
- `Cargo.toml`, `Cargo.lock` — **byte-unchanged unless D14 forces +1.** Surface explicitly in Phase 9.

**Explicit carve-outs (these frozen files DO get touched, with bounded edits):**

- `src/main.rs` — **+1 line:** `app.add_plugins(CellFeaturesPlugin)` parallel to `DungeonPlugin`/`PartyPlugin`/etc.
- `src/plugins/dungeon/mod.rs` — **3 small edits:** (a) `pub mod features;` declaration, (b) Phase 6 D9b — replace `floor.can_move(pos.x, pos.y, move_dir)` at line 666 with a new `can_move_with_doors(floor, &door_states, pos, move_dir)` wrapper call (adds `Res<DoorStates>` SystemParam), (c) Phase 7 — `spawn_party_and_camera` reads `Option<Res<PendingTeleport>>` and uses its `(x, y, facing)` instead of `floor.entry_point` if present.
- `src/plugins/loading/mod.rs` — **2 edits:** (a) +2 SFX handle fields on `AudioAssets`, (b) Phase 7 — `handle_teleport_request` system + `PendingTeleport: Resource` registration. The "FROZEN post-#3" comment applies to `LoadingPlugin`'s state-transition logic; adding `AudioAssets` fields is the same surgery #6 performed (verified by reading `loading/mod.rs:50-76` — the 5 SFX fields were added in #6, after the freeze comment was authored).
- `src/data/dungeon.rs` — **1 additive field:** `locked_doors: Vec<((u32,u32), Direction, String)>` on `DungeonFloor` with `#[serde(default)]`. Existing `is_well_formed` and `validate_wall_consistency` are unaffected (no per-cell shape constraint on this field).
- `src/data/items.rs` — **1 additive field:** `key_id: Option<String>` on `ItemAsset` with `#[serde(default)]`. Existing round-trip tests use `..Default::default()` and pick up `key_id: None`.
- `tests/item_db_loads.rs` — **+5 LOC** to extend the `assert_item_db_shape` assertion to verify `rusty_key.key_id == Some("rusty_door_01")`.

---

## Open Decisions Awaiting User Input

The research doc surfaced D1–D15. The prompt classifies them as below. **Auto-resolved** items proceed without user input (the planner has confirmed the recommendation). **Recommended defaults** proceed unless the user objects. **Genuine USER PICK** items must be confirmed before kickoff.

### Auto-resolved (proceed unless user objects)

- **D2c** — `data/dungeon.rs` schema edit (additive `locked_doors` field, `#[serde(default)]`). Existing tests pass unchanged. Side-effect of D2/D2b. **Proceed.**
- **D4** — Anti-magic / dark-zone scope. Dark zone is **already handled** by #10's minimap subscriber at `minimap.rs:208-211` — no #13 work for dark zones. Anti-magic ships as plumbing (marker component + add/remove system + `tracing::info!` log) with no v1 consumer. **Proceed.**
- **D8** — `DungeonAction::Interact` confirmation. Already exists at `input/mod.rs:78`, bound to `KeyCode::KeyF` at line 149. **No #13 input work.**
- **D9b** — `handle_dungeon_input` modification for door passability. Side-effect of D9. The wrapper helper `can_move_with_doors(floor, doors, pos, dir)` is the cleanest implementation. **Proceed (largest frozen-file edit).**

### Recommended defaults (proceed unless user objects)

- **D1** — Door state persistence: per-floor-instance only (cleared on `OnExit(Dungeon)`). Cross-floor persistence is a #23 concern. **Recommended.**
- **D2** — KeyItem representation: `key_id: Option<String>` on `ItemAsset` paired with `ItemKind::KeyItem`. **Recommended.**
- **D2b** — `door_id` location: side-table `locked_doors: Vec<((u32,u32), Direction, String)>` on `DungeonFloor`. Additive; doesn't ripple WallType variants. **Recommended.**
- **D5** — Encounter trap stubbing: publish `EncounterRequested { source: EncounterSource::AlarmTrap }` with logged-only consumer. Defines the interface for #16 cleanly. **Recommended.**
- **D6** — Screen wobble: `Quat::from_rotation_z(jitter)` rotation jitter on player Transform, 200ms damped sine. Cheaper than a custom shader; matches `MovementAnimation` lifecycle. **Recommended.**
- **D7** — SFX strategy: royalty-free `.ogg` files committed to `assets/audio/sfx/` (matches #6 precedent at `loading/mod.rs:66-75`). User supplies the 2 .ogg files OR the implementer can suggest CC0 sources (e.g., freesound.org). **Recommended.**
- **D9** — Door state component shape: `DoorStates: Resource(HashMap<(GridPosition, Direction), DoorState>)`. **Recommended.**
- **D12** — Poison stacking: naive push (each trap entry adds another `ActiveEffect`). Matches #14's expected behavior (which the roadmap §14 line 781 punts). **Recommended.**
- **D13** — Locked-door key consumption: NOT consumed (Wizardry-style; reusable). **Recommended.**
- **D15** — Doors closed-by-default; player presses Interact to open. Genre canon. **Recommended.**

### Genuine USER PICK — confirm before kickoff

- **D3 — Cross-floor teleporter implementation:**
  - **α (RECOMMENDED):** Re-enter `GameState::Loading` via `TeleportRequested` Message; `LoadingPlugin` owns the consumer and a `PendingTeleport: Resource`; `spawn_party_and_camera` reads `PendingTeleport` to override `floor.entry_point`. Brief loading-screen flash is **genre-correct**. Smallest code change. Composes with #23 save/load.
  - β: In-state asset swap. Smoother visually but bypasses `bevy_asset_loader` guarantees and requires ~80 LOC of refactor in `dungeon/mod.rs`. Not recommended.
  - γ: Hybrid. Doubles test surface. Not recommended.
  - **Decision needed:** Confirm α or escalate.

- **D10 — Number of new `SfxKind` variants:**
  - **A (RECOMMENDED):** +2 variants — `SpinnerWhoosh`, `DoorClose`. Reuse `Door` for door-open and trap-snap; reuse `AttackHit` for pit damage; reuse `EncounterSting` for alarm. Matches roadmap "+2-4 trap SFX" budget at line 720.
  - B: +5 variants (`SpinnerWhoosh`, `DoorClose`, `TrapTrigger`, `PitDamage`, `Teleport`). More expressive audio.
  - C: +1 variant (only `SpinnerWhoosh`). Minimum-touch.
  - **Decision needed:** Confirm A (the plan assumes A throughout).

- **D11 — `floor_02.dungeon.ron` for cross-floor testing:**
  - **A (RECOMMENDED):** Author a minimal `floor_02.dungeon.ron` (~80 lines, 4×4 single room, entry at (1,1) South); add `floor_02: Handle<DungeonFloor>` field to `DungeonAssets`. Cost: ~80 lines RON + 2 lines code. Unlocks the cross-floor end-to-end integration test.
  - B: Defer cross-floor end-to-end test to manual smoke. Saves ~80 lines RON but no automated coverage.
  - C: Mock floor 2 in tests via custom Asset insertion (no production floor).
  - **Decision needed:** Confirm A. If user picks B, drop Phase 8's floor_02 work and convert the cross-floor test to a manual checklist item.

- **D14 — `rand` dependency check:**
  - **A (RECOMMENDED):** Verify with `cargo tree -i rand` BEFORE writing the spinner. If present, use directly. Likely outcome: present (transitively via bevy_audio / bevy_pbr).
  - B: Use `Time::elapsed_secs_f64() as u64 % 4` deterministic fallback. Acceptable for v1 spinner UX.
  - C: Add `rand = "0.8"` to Cargo.toml (Δ deps = +1 — breaks the byte-unchanged signal).
  - **Decision needed:** Confirm A as the verification approach. The implementer reports outcome at Phase 5 kick-off and chooses B or C if A reveals `rand` is missing.

---

## Pitfalls

The 11 research-flagged pitfalls below appear as guards inside the relevant Phase below; this section is the central reference and the prompt's required lift. Mirrors the Feature #12 plan's Pitfalls section format.

### Pitfall 4 — `Door` is currently passable per `can_move` — toggling Closed must update the passability check

**Where it bites:** `floor.can_move(pos.x, pos.y, move_dir)` returns `true` for `WallType::Door` (asset-level "closed but unlocked"). Without a runtime override, the player walks through every `Door` regardless of `DoorState`.

**Guard in plan:** Phase 6 wraps `floor.can_move` with `can_move_with_doors(floor, &door_states, pos, dir)` and modifies `handle_dungeon_input` to use the wrapper. Default `DoorState::Closed` (returned via `unwrap_or_default()` for unmapped keys) blocks `WallType::Door` walls. **Verification:** 2 unit tests in Phase 6 — empty `DoorStates` + `WallType::Door` reads as Closed → blocked; `DoorStates::insert((pos,dir), Open)` + `WallType::Door` → passable.

### Pitfall 5 — Initial door state for `WallType::Door` is closed (not pre-opened)

**Where it bites:** with the Pitfall 4 wrapper in place and `DoorState::default() = Closed`, the player CANNOT walk through any `Door` until they press Interact. **This IS the desired UX (D15: closed-by-default; player presses Interact to open).**

**Guard in plan:** Phase 4 declares `DoorState` with `#[default] Closed` and Phase 5 implements `handle_door_interact` which toggles `Closed ↔ Open` on `Interact` press against a `Door` edge. **Verification:** unit test `door_state_default_is_closed` (Phase 4); Layer-2 test `door_interact_toggles_state` (Phase 5).

### Pitfall 7 — Pit damage applies to ALL party members; saturating subtract on `current_hp`

**Where it bites:** `derived.current_hp -= damage` underflows `u32` when `damage > current_hp` → wraparound to a huge HP value.

**Guard in plan:** Phase 5 `apply_pit_trap` uses `derived.current_hp = derived.current_hp.saturating_sub(*damage)`. Same pattern as `character.rs:374-381`. **Verification:** Layer-1 unit test `pit_trap_subtracts_damage_saturating` (Phase 4); Layer-2 test `pit_trap_damages_party` (Phase 5).

### Pitfall 8 — `populate_locked_doors` must be idempotent (cross-floor re-entry)

**Where it bites:** D3-α teleport re-enters `OnEnter(Dungeon)`. Without clear-first, `LockedDoors.by_edge` stacks duplicate entries.

**Guard in plan:** Phase 5 `populate_locked_doors` body starts with `locked_doors.by_edge.clear()`. Mirrors `populate_item_handle_registry` clear-first pattern at `inventory.rs:539`. **Verification:** Layer-1 unit test `locked_doors_clear_idempotent` (Phase 4).

### Pitfall 9 — `WallType::LockedDoor` `can_move` returns `false`; wrapper must override on `DoorState::Open`

**Where it bites:** `floor.can_move` short-circuits to `false` for LockedDoor walls. After unlocking (via `handle_door_interact` writing `DoorStates[(pos,dir)] = Open`), the wrapper must return `true` even though `floor.can_move` says `false`.

**Guard in plan:** Phase 6 `can_move_with_doors` checks the wall type FIRST. If `WallType::LockedDoor` and `DoorStates[(pos,dir)] == Open`, return `true`. Otherwise fall through to `floor.can_move`. **Verification:** 2 unit tests in Phase 6 — `LockedDoor + DoorStates empty` → blocked; `LockedDoor + DoorStates::insert(Open)` → passable.

### Pitfall 11 — Spinner whoosh + screen wobble + new facing all on the SAME frame; minimap must redraw

**Where it bites:** if `apply_spinner` somehow runs AFTER the minimap painter (cross-schedule ordering is silently ignored), the minimap shows the OLD facing for one frame.

**Guard in plan:** Bevy automatic schedule ordering means `Update` runs before `EguiPrimaryContextPass`; the minimap reads `&Facing` directly so the spun facing reflects on the same frame. Phase 5 Layer-2 test `spinner_randomizes_facing` emits a `MovedEvent` to a spinner cell, runs `app.update()`, queries `Facing`, and asserts the new direction.

### Additional pitfalls (1, 2, 3, 6, 10, 12) — guarded inline in Steps

Pitfalls 1, 2, 3, 6, 10, and 12 from the research doc are guarded inline in the relevant phase steps (no `Without<MovementAnimation>` filter on consumers; per-system `MessageReader` cursors; defensive `Option<Res<DungeonAssets>>`; spinner ordering vs. input; `#[serde(default)]` on schema additions; entry-point convention to avoid OQ5 anti-magic-on-spawn). Detail in the relevant Step.

---

## Steps

The implementation proceeds in **9 phases**, each one a single atomic commit boundary. Every phase's exit criterion is `cargo test` passing. Phases 1-4 build the data and skeleton; Phase 5 lights up the systems; Phase 6 modifies the frozen `dungeon/mod.rs::handle_dungeon_input`; Phase 7 lights up cross-floor teleport; Phase 8 authors `floor_02.dungeon.ron`; Phase 9 is the final 7-command verification gate.

### Phase 1 — Data schema additive edits (`src/data/items.rs` + `src/data/dungeon.rs`)

The smallest possible change first: extend two schemas additively. Existing tests must pass unchanged because every new field uses `#[serde(default)]`.

- [x] In `src/data/items.rs`, add a new field to `ItemAsset` (currently 9 fields at lines 84-113) directly after `stackable: bool`:

  ```rust
  /// Optional key identifier — only meaningful when `kind == ItemKind::KeyItem`.
  /// Read by Feature #13's `handle_door_interact` when the player presses
  /// Interact against a `WallType::LockedDoor`. Default `None` for non-key items.
  #[serde(default)]
  pub key_id: Option<String>,
  ```

  Keep the existing `#[derive(Asset, Reflect, Serialize, Deserialize, Default, Debug, Clone, PartialEq)]` — `Option<String>` is `Default::default() = None`. No new derives needed.
- [x] In `src/data/items.rs::tests`, extend the round-trip tests to set `key_id: Some("test_door_01".into())` in one test and `key_id: None` (default) in another. The existing `item_asset_round_trips_through_ron` test pattern handles this — populate the field, serialize via `ron::ser::to_string_pretty`, deserialize, `assert_eq!`. Add a new test `item_asset_round_trips_with_key_id`:

  ```rust
  #[test]
  fn item_asset_round_trips_with_key_id() {
      let original = ItemAsset {
          id: "rusty_key".into(),
          display_name: "Rusty Key".into(),
          kind: ItemKind::KeyItem,
          slot: EquipSlot::None,
          key_id: Some("rusty_door_01".into()),
          ..Default::default()
      };
      let serialized = ron::ser::to_string_pretty(&original, ron::ser::PrettyConfig::default())
          .expect("ItemAsset should serialize");
      let parsed: ItemAsset = ron::de::from_str(&serialized).expect("ItemAsset should round-trip");
      assert_eq!(parsed, original);
  }
  ```
- [x] In `src/data/dungeon.rs`, add a new field to `DungeonFloor` (currently 8 fields at lines 249-266) directly after `lighting`:

  ```rust
  /// Locked-door identifiers for the locked-door check in Feature #13.
  /// Each entry: `((cell_x, cell_y), facing_direction, door_id)`.
  /// `door_id` matches an `ItemAsset.key_id` for the locked-door unlock test.
  /// Default empty so existing floors that omit the field still parse.
  #[serde(default)]
  pub locked_doors: Vec<((u32, u32), Direction, String)>,
  ```

  Keep the existing `#[derive(Asset, Reflect, Serialize, Deserialize, Default, Debug, Clone, PartialEq)]`. `Vec<T>` defaults to empty.
- [x] In `src/data/dungeon.rs::tests`, extend the existing `DungeonFloor` round-trip tests to populate the new field. Add a new test `dungeon_floor_round_trips_with_locked_doors`:

  ```rust
  #[test]
  fn dungeon_floor_round_trips_with_locked_doors() {
      let mut floor = make_floor(2, 2);
      floor.locked_doors = vec![((0, 0), Direction::East, "test_door".into())];
      let serialized = ron::ser::to_string_pretty(&floor, ron::ser::PrettyConfig::default())
          .expect("DungeonFloor should serialize");
      let parsed: DungeonFloor = ron::de::from_str(&serialized)
          .expect("DungeonFloor should round-trip");
      assert_eq!(parsed.locked_doors, floor.locked_doors);
  }
  ```
- [x] **Verification (atomic commit boundary):**
  - `cargo test data::items::tests` — passes (existing + 1 new).
  - `cargo test data::dungeon::tests` — passes (existing + 1 new).
  - `cargo build` — succeeds (no consumers yet).
  - `cargo test` — full suite passes.

### Phase 2 — Asset edits (`assets/items/core.items.ron` + `assets/dungeons/floor_01.dungeon.ron`)

Fill the schema additions with the actual gameplay values. Verifies via integration test extension.

- [x] In `assets/items/core.items.ron`, add `key_id: Some("rusty_door_01")` to the existing `rusty_key` entry (currently lines 100-109). Insert a new line after `icon_path`:

  ```ron
  (
      id: "rusty_key",
      display_name: "Rusty Key",
      kind: KeyItem,
      slot: None,
      stats: (),
      weight: 0,
      value: 0,
      icon_path: "ui/icons/items/rusty_key.png",
      key_id: Some("rusty_door_01"),  // <-- NEW LINE
  ),
  ```
- [x] In `assets/dungeons/floor_01.dungeon.ron`, append a new top-level field `locked_doors` directly before the closing parenthesis (currently line 164):

  ```ron
  )
  // ... existing fields ...
  lighting: ( ... ),
  locked_doors: [
      ((3, 1), East, "rusty_door_01"),  // matches LockedDoor at edge (3,1)/(4,1)
  ],
  )
  ```

  Match the existing comment header at lines 17-29 by extending it:
  ```
  //   Locked-door ids:
  //     "rusty_door_01" — east of (3,1) / west of (4,1) — opened by `rusty_key`
  ```
- [x] In `tests/item_db_loads.rs`, extend `assert_item_db_shape` (currently lines 62-118) to verify the new `key_id` field. Insert AFTER the existing `assert_eq!(key.slot, EquipSlot::None, ...)` at line 115:

  ```rust
  // Feature #13 — rusty_key carries the door_id of the locked door at floor_01:(3,1) East.
  assert_eq!(
      key.key_id,
      Some("rusty_door_01".to_string()),
      "rusty_key.key_id should be Some(\"rusty_door_01\")"
  );
  ```
- [x] **Verification (atomic commit boundary):**
  - `cargo test --test item_db_loads` — passes.
  - `cargo test` — full suite passes (the existing `dungeon::tests::floor_01_loads` integration test, if present, picks up the new `locked_doors` field via `#[serde(default)]` and remains green).
  - `cargo run --features dev` — manual smoke: game reaches Dungeon without an asset-load panic. (Pitfall 10: schema change is invisible to the runtime because every consumer ignores the new fields until Phase 4-5 land.)

### Phase 3 — Audio additions (`src/plugins/audio/sfx.rs` + `src/plugins/loading/mod.rs`)

Two SFX variants (D10-A: `SpinnerWhoosh` + `DoorClose`) plus their `.ogg` placeholder assets and `AudioAssets` handle slots. The match arm in `handle_sfx_requests` enforces exhaustiveness — the compiler catches any forgotten variant.

- [x] In `src/plugins/audio/sfx.rs`, add 2 variants to `SfxKind` (currently 5 variants at lines 51-57):

  ```rust
  pub enum SfxKind {
      Footstep,
      Door,
      EncounterSting,
      MenuClick,
      AttackHit,
      // Feature #13 additions:
      SpinnerWhoosh,
      DoorClose,
  }
  ```
- [x] In the same file, extend the `match req.kind` block in `handle_sfx_requests` (currently lines 78-84) with 2 arms:

  ```rust
  let handle = match req.kind {
      SfxKind::Footstep => audio_assets.sfx_footstep.clone(),
      SfxKind::Door => audio_assets.sfx_door.clone(),
      SfxKind::EncounterSting => audio_assets.sfx_encounter_sting.clone(),
      SfxKind::MenuClick => audio_assets.sfx_menu_click.clone(),
      SfxKind::AttackHit => audio_assets.sfx_attack_hit.clone(),
      // Feature #13 additions:
      SfxKind::SpinnerWhoosh => audio_assets.sfx_spinner_whoosh.clone(),
      SfxKind::DoorClose => audio_assets.sfx_door_close.clone(),
  };
  ```
- [x] In `src/plugins/loading/mod.rs`, add 2 fields to `AudioAssets` (currently 10 fields at lines 50-76) directly after `sfx_attack_hit`:

  ```rust
  // Feature #13 additions:
  #[asset(path = "audio/sfx/spinner_whoosh.ogg")]
  pub sfx_spinner_whoosh: Handle<AudioSource>,
  #[asset(path = "audio/sfx/door_close.ogg")]
  pub sfx_door_close: Handle<AudioSource>,
  ```
- [x] Add the 2 placeholder `.ogg` files at `assets/audio/sfx/spinner_whoosh.ogg` and `assets/audio/sfx/door_close.ogg`. Per D7-A, royalty-free .ogg files committed alongside the existing 5 (verified at `loading/mod.rs:66-75`). User supplies the audio OR the implementer can suggest CC0 sources (e.g., freesound.org "spinner whoosh", "door close"). For initial development, even silent `.ogg` files are acceptable so long as they are valid OGG containers (Bevy's `AudioSource` decoder rejects empty files).
- [x] **Verification (atomic commit boundary):**
  - `cargo check` — succeeds (the match exhaustiveness compiler check is the primary gate).
  - `cargo run --features dev` — manual smoke: game reaches Dungeon without a "missing asset" panic. (`bevy_asset_loader` blocks state advance until all `AudioAssets` handles report `LoadedWithDependencies` — if either `.ogg` is missing or invalid, the game hangs in `GameState::Loading`. Treat that as a Phase 3 verification gate.)
  - `cargo test` — full suite passes (no consumer for the new variants yet; the existing audio tests don't exercise `SfxKind::SpinnerWhoosh`/`DoorClose`).

### Phase 4 — `features.rs` skeleton: types, resources, components, messages, Layer-1 tests

Land the entire scaffolding for the new file in one phase, with no Bevy systems wired yet. This is the bulk of the new LOC (~150 lines) and includes 4 Layer-1 unit tests that exercise the type machinery without an `App`.

- [ ] Create `src/plugins/dungeon/features.rs` (NEW file). Add the file-level doc comment summarizing the module's purpose and pointing at the research doc:

  ```rust
  //! Cell-feature reaction layer — Feature #13.
  //!
  //! Owns the Bevy systems that react to player movement onto cells with
  //! special properties (traps, teleporters, spinners, anti-magic zones)
  //! and the door-interaction system that toggles `WallType::Door` open/closed.
  //!
  //! Subscribes to `MovedEvent` (published by `dungeon/mod.rs:686-690`) and
  //! `Res<ActionState<DungeonAction>>` for the Interact key.
  //!
  //! See `project/research/20260506-080000-feature-13-cell-features.md`.
  //!
  //! ## Bevy 0.18 family rename
  //!
  //! `TeleportRequested` and `EncounterRequested` derive `Message`, NOT `Event`.
  //! Read with `MessageReader<T>`, write with `MessageWriter<T>`.
  ```
- [ ] Add imports:

  ```rust
  use bevy::prelude::*;
  use leafwing_input_manager::prelude::ActionState;
  use std::collections::HashMap;

  use crate::data::DungeonFloor;
  use crate::data::dungeon::{Direction, TeleportTarget, TrapType, WallType};
  use crate::data::ItemAsset;
  use crate::plugins::audio::{SfxKind, SfxRequest};
  use crate::plugins::dungeon::{
      Facing, GridPosition, MovedEvent, MovementAnimation, PlayerParty, animate_movement,
      handle_dungeon_input,
  };
  use crate::plugins::input::DungeonAction;
  use crate::plugins::loading::DungeonAssets;
  use crate::plugins::party::{
      ActiveEffect, DerivedStats, Inventory, ItemInstance, ItemKind, PartyMember,
      StatusEffectType, StatusEffects,
  };
  use crate::plugins::state::GameState;
  ```
- [ ] Define `DoorState` enum (Pitfall 5 — `#[default] Closed`):

  ```rust
  /// State of a single door edge. Default `Closed` — `WallType::Door` walls
  /// are gated by this resource (Pitfall 4: pre-#13, `floor.can_move` returned
  /// `true` for Door; the runtime override here makes Closed actually closed).
  #[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
  pub enum DoorState {
      #[default]
      Closed,
      Open,
  }
  ```
- [ ] Define `DoorStates` resource:

  ```rust
  /// Per-floor-instance door state (D1-A — cleared on `OnExit(Dungeon)`).
  /// Keyed by edge `(grid_position, direction_to_other_cell)`.
  #[derive(Resource, Default, Debug)]
  pub struct DoorStates {
      pub doors: HashMap<(GridPosition, Direction), DoorState>,
  }
  ```
- [ ] Define `LockedDoors` resource:

  ```rust
  /// Maps locked-door edges to their `door_id` strings. Populated on
  /// `OnEnter(Dungeon)` from `DungeonFloor::locked_doors`. Cleared on
  /// `OnExit(Dungeon)`. Used by `handle_door_interact` to look up the
  /// expected `key_id` for a `WallType::LockedDoor` edge.
  #[derive(Resource, Default, Debug)]
  pub struct LockedDoors {
      pub by_edge: HashMap<(GridPosition, Direction), String>,
  }
  ```
- [ ] Define `PendingTeleport` resource (D3-α):

  ```rust
  /// Pending cross-floor teleport destination. Set by `apply_teleporter`
  /// publishing `TeleportRequested`; read by `LoadingPlugin`'s
  /// `handle_teleport_request` system; consumed by `spawn_party_and_camera`
  /// in `dungeon/mod.rs` on the next `OnEnter(Dungeon)`.
  #[derive(Resource, Default, Debug)]
  pub struct PendingTeleport {
      pub target: Option<TeleportTarget>,
  }
  ```
- [ ] Define `AntiMagicZone` marker component (D4):

  ```rust
  /// Marker on the `PlayerParty` entity while standing in a
  /// `CellFeatures::anti_magic_zone` cell. Future #14/#15 spell-casting
  /// systems will query `Query<(), (With<PlayerParty>, With<AntiMagicZone>)>`
  /// to gate spells.
  #[derive(Component, Debug, Clone, Copy)]
  pub struct AntiMagicZone;
  ```
- [ ] Define `ScreenWobble` component (D6-A):

  ```rust
  /// In-flight screen-wobble animation attached to the `PlayerParty` entity
  /// after a spinner trigger. Lifecycle mirrors `MovementAnimation`'s
  /// remove-on-completion pattern. Damped sine: `amplitude × sin(8πt) × (1 − t)`.
  #[derive(Component, Debug, Clone)]
  pub struct ScreenWobble {
      pub elapsed_secs: f32,
      pub duration_secs: f32,
      pub amplitude: f32,
  }
  ```
- [ ] Define `TeleportRequested` message (D3-α):

  ```rust
  /// Published by `apply_teleporter` for cross-floor teleporter cells (and
  /// by `apply_pit_trap` for `Pit { target_floor: Some(_) }`). Consumed by
  /// `LoadingPlugin::handle_teleport_request`.
  ///
  /// **`Message`, NOT `Event`** — Bevy 0.18 family rename.
  #[derive(Message, Clone, Debug)]
  pub struct TeleportRequested {
      pub target: TeleportTarget,
  }
  ```
- [ ] Define `EncounterRequested` message + `EncounterSource` enum (D5):

  ```rust
  /// Published by `apply_alarm_trap` (and future random-encounter rolls).
  /// Consumed by Feature #16 (combat trigger) — v1 has only a logged stub
  /// consumer in this plugin (see `apply_alarm_trap`).
  ///
  /// **`Message`, NOT `Event`** — Bevy 0.18 family rename.
  #[derive(Message, Clone, Copy, Debug)]
  pub struct EncounterRequested {
      pub source: EncounterSource,
  }

  #[derive(Clone, Copy, Debug, PartialEq, Eq)]
  pub enum EncounterSource {
      AlarmTrap,
      // Future: Random (foe roll), Foe (overworld encounter) — surface in #16.
  }
  ```
- [ ] Add the `CellFeaturesPlugin` STRUCT only (no `Plugin` impl yet — Phase 5):

  ```rust
  /// Owns all cell-feature systems, resources, and messages for #13.
  /// Registered in `main.rs` parallel to `DungeonPlugin`/`PartyPlugin`.
  pub struct CellFeaturesPlugin;
  ```
- [ ] Add a `#[cfg(test)] mod tests` block with 4 Layer-1 tests (no `App` required):

  ```rust
  #[cfg(test)]
  mod tests {
      use super::*;

      #[test]
      fn pit_trap_subtracts_damage_saturating() {
          let mut hp = 3u32;
          let damage = 5u32;
          hp = hp.saturating_sub(damage);
          assert_eq!(hp, 0, "saturating_sub clamps to 0; no underflow wraparound");
      }

      #[test]
      fn door_state_default_is_closed() {
          assert_eq!(DoorState::default(), DoorState::Closed);
      }

      #[test]
      fn door_states_resource_round_trip() {
          let mut states = DoorStates::default();
          let key = (GridPosition { x: 3, y: 1 }, Direction::East);
          states.doors.insert(key, DoorState::Open);
          assert_eq!(states.doors.get(&key).copied(), Some(DoorState::Open));
      }

      #[test]
      fn locked_doors_clear_idempotent() {
          let mut locked = LockedDoors::default();
          let key = (GridPosition { x: 0, y: 0 }, Direction::North);
          locked.by_edge.insert(key, "x".into());
          locked.by_edge.clear();
          locked.by_edge.insert(key, "x".into());
          assert_eq!(locked.by_edge.len(), 1, "clear-first guarantees idempotence");
      }
  }
  ```
- [ ] In `src/plugins/dungeon/mod.rs`, add `pub mod features;` directly after the existing module declarations (e.g., after `use crate::plugins::state::GameState;` or wherever module decls live in the file — verify the location with `head -50 src/plugins/dungeon/mod.rs` at impl time). This makes the new module compilable as part of the dungeon plugin tree.
- [ ] **Verification (atomic commit boundary):**
  - `cargo build` — succeeds (the new types compile in isolation).
  - `cargo test plugins::dungeon::features::tests` — passes (4 Layer-1 tests).
  - `cargo test` — full suite passes.

### Phase 5 — `CellFeaturesPlugin::build` + 9 systems + Layer-2 tests

Implement the 9 systems and the Plugin impl. Wire the plugin into `main.rs`. Add Layer-2 tests for each system. This is the largest phase (~300-400 LOC of system bodies + tests).

- [ ] In `src/plugins/dungeon/features.rs`, implement `populate_locked_doors` (`OnEnter(Dungeon)` system, Pitfall 8 clear-first):

  ```rust
  /// Populate `LockedDoors` from `DungeonFloor::locked_doors`. Clears first
  /// for idempotence across `OnEnter(Dungeon)` re-entries (Pitfall 8 — D3-α
  /// teleport re-enters the state).
  fn populate_locked_doors(
      mut locked_doors: ResMut<LockedDoors>,
      dungeon_assets: Option<Res<DungeonAssets>>,
      floors: Res<Assets<DungeonFloor>>,
  ) {
      locked_doors.by_edge.clear();
      let Some(assets) = dungeon_assets else { return; };
      let Some(floor) = floors.get(&assets.floor_01) else { return; };
      // NOTE: Phase 7 will switch this to read the active floor handle from
      // `PendingTeleport` if present. For now, only floor_01 is loaded.
      for ((x, y), dir, id) in &floor.locked_doors {
          locked_doors
              .by_edge
              .insert((GridPosition { x: *x, y: *y }, *dir), id.clone());
      }
  }
  ```
- [ ] Implement `clear_door_resources` (`OnExit(Dungeon)` system, Security guard #3):

  ```rust
  fn clear_door_resources(
      mut door_states: ResMut<DoorStates>,
      mut locked_doors: ResMut<LockedDoors>,
      mut pending_teleport: ResMut<PendingTeleport>,
  ) {
      door_states.doors.clear();
      locked_doors.by_edge.clear();
      pending_teleport.target = None;
  }
  ```
- [ ] Implement `handle_door_interact` per research §Pattern 2:

  ```rust
  /// Reads `Res<ActionState<DungeonAction>>`; on `Interact` press, looks at the
  /// wall the player is facing. For `WallType::Door`, toggles `DoorState`. For
  /// `WallType::LockedDoor`, walks all party inventories looking for a matching
  /// `ItemKind::KeyItem` with `key_id == door_id`; if found, sets `DoorState::Open`.
  /// Keys are NOT consumed (D13 — Wizardry-style; reusable).
  fn handle_door_interact(
      actions: Res<ActionState<DungeonAction>>,
      party: Query<(&GridPosition, &Facing), With<PlayerParty>>,
      floors: Res<Assets<DungeonFloor>>,
      dungeon_assets: Option<Res<DungeonAssets>>,
      locked_doors: Res<LockedDoors>,
      mut door_states: ResMut<DoorStates>,
      inventory: Query<&Inventory, With<PartyMember>>,
      instances: Query<&ItemInstance>,
      items: Res<Assets<ItemAsset>>,
      mut sfx: MessageWriter<SfxRequest>,
  ) {
      // Body per research §Pattern 2:
      // 1. Early return if !actions.just_pressed(&DungeonAction::Interact).
      // 2. Resolve party single() and floor handle (Option<Res<DungeonAssets>> defensive).
      // 3. Match `wall = floor.walls[pos.y][pos.x].(north|south|east|west)`
      //    based on facing.0.
      // 4. WallType::Door → toggle DoorState; emit SfxKind::Door (open) or
      //    SfxKind::DoorClose (close).
      // 5. WallType::LockedDoor → look up door_id in LockedDoors; walk all
      //    Inventory.0 entities; for each ItemInstance, items.get(handle);
      //    if asset.kind == KeyItem && asset.key_id.as_deref() == Some(door_id),
      //    set DoorStates[(pos, dir)] = Open and emit SfxKind::Door. D13: do NOT
      //    consume the key (no inventory mutation).
      // 6. Other wall types → no-op.
  }
  ```

  See research §Pattern 2 for the full body. Keep the `info!` log messages on unlock for debug observability.
- [ ] Implement `apply_pit_trap` per research §Pattern 3 (Pitfall 7 — saturating_sub):

  ```rust
  fn apply_pit_trap(
      mut moved: MessageReader<MovedEvent>,
      floors: Res<Assets<DungeonFloor>>,
      dungeon_assets: Option<Res<DungeonAssets>>,
      mut party: Query<&mut DerivedStats, With<PartyMember>>,
      mut sfx: MessageWriter<SfxRequest>,
      mut teleport: MessageWriter<TeleportRequested>,
  ) {
      // Body per research §Pattern 3:
      // For each MovedEvent ev, look at floor.features[ev.to.y][ev.to.x].trap.
      // If TrapType::Pit { damage, target_floor }:
      //   - For each PartyMember, derived.current_hp = current_hp.saturating_sub(*damage).
      //   - Emit SfxKind::AttackHit (D10-A reuse).
      //   - If target_floor is Some, emit TeleportRequested { target: TeleportTarget {
      //       floor: *target_floor, x: ev.to.x, y: ev.to.y, facing: Some(ev.facing) }}.
  }
  ```
- [ ] Implement `apply_poison_trap` per research §Pattern 4 (D12 — naive push):

  ```rust
  fn apply_poison_trap(
      mut moved: MessageReader<MovedEvent>,
      floors: Res<Assets<DungeonFloor>>,
      dungeon_assets: Option<Res<DungeonAssets>>,
      mut party: Query<&mut StatusEffects, With<PartyMember>>,
      mut sfx: MessageWriter<SfxRequest>,
  ) {
      const POISON_TURNS: u32 = 5;
      // Body per research §Pattern 4. For each MovedEvent landing on a poison
      // trap cell, push ActiveEffect { effect_type: Poison, remaining_turns:
      // Some(POISON_TURNS), magnitude: 0.0 } onto each PartyMember's StatusEffects.
      // Emit SfxKind::Door (placeholder hiss; D10-A reuse).
  }
  ```
- [ ] Implement `apply_alarm_trap` per research §Pattern 7 (D5 — publish + log):

  ```rust
  fn apply_alarm_trap(
      mut moved: MessageReader<MovedEvent>,
      floors: Res<Assets<DungeonFloor>>,
      dungeon_assets: Option<Res<DungeonAssets>>,
      mut encounter: MessageWriter<EncounterRequested>,
      mut sfx: MessageWriter<SfxRequest>,
  ) {
      // Body per research §Pattern 7. For each MovedEvent landing on an alarm
      // trap cell, publish EncounterRequested { source: AlarmTrap }. Emit
      // SfxKind::EncounterSting (D10-A reuse). Log info!("Alarm trap...").
  }
  ```
- [ ] Implement `apply_teleporter` per research §Pattern 6 — same-floor branch only (cross-floor branch added in Phase 7):

  ```rust
  fn apply_teleporter(
      mut moved: MessageReader<MovedEvent>,
      floors: Res<Assets<DungeonFloor>>,
      dungeon_assets: Option<Res<DungeonAssets>>,
      mut party: Query<(&mut GridPosition, &mut Facing, &mut Transform), With<PlayerParty>>,
      mut writer: MessageWriter<MovedEvent>,
      mut teleport: MessageWriter<TeleportRequested>,
      mut sfx: MessageWriter<SfxRequest>,
  ) {
      // Body per research §Pattern 6 SAME-FLOOR BRANCH ONLY:
      //   if target.floor == floor.floor_number:
      //     - Mutate pos.x = target.x, pos.y = target.y.
      //     - If target.facing is Some, mutate facing.0.
      //     - Snap transform.translation to grid_to_world (use the
      //       same Vec3::new(target.x as f32 * 2.0, 0.0, target.y as f32 * 2.0)
      //       that handle_dungeon_input does — or expose a pub(crate)
      //       grid_to_world helper from dungeon/mod.rs).
      //     - Re-publish MovedEvent { from: old_pos, to: *pos, facing: facing.0 }.
      //   else:
      //     - PHASE 7 ONLY: emit TeleportRequested { target: target.clone() }.
      //     - For Phase 5: leave as `// TODO Phase 7` comment.
      // Emit SfxKind::Door (D10-A reuse; could add Teleport SFX in v2).
  }
  ```

  **Phase 5 only ships the same-floor branch.** Phase 7 adds the cross-floor `TeleportRequested` emit (the relevant `MessageWriter<TeleportRequested>` SystemParam is wired in this phase but not used until Phase 7).
- [ ] Implement `apply_spinner` + `tick_screen_wobble` per research §Pattern 5 (D6-A — rotation jitter):

  ```rust
  fn apply_spinner(
      mut moved: MessageReader<MovedEvent>,
      floors: Res<Assets<DungeonFloor>>,
      dungeon_assets: Option<Res<DungeonAssets>>,
      mut party: Query<(Entity, &mut Facing), With<PlayerParty>>,
      mut commands: Commands,
      mut sfx: MessageWriter<SfxRequest>,
      time: Res<Time>,
  ) {
      // Body per research §Pattern 5:
      //   For each MovedEvent landing on a spinner cell:
      //   - Pick a random Direction NOT equal to current facing (D14: rand
      //     if available, else Time::elapsed_secs_f64() modulo).
      //   - Mutate facing.0 = new_dir.
      //   - Emit SfxKind::SpinnerWhoosh.
      //   - commands.entity(entity).insert(ScreenWobble {
      //       elapsed_secs: 0.0, duration_secs: 0.2, amplitude: 0.15 (radians) }).
  }

  /// Tick the screen-wobble animation. Damped sine: `amplitude × sin(8πt) × (1 − t)`.
  /// Multiplies onto `Transform::rotation` (rotation jitter, D6-A).
  /// `.after(animate_movement)` to win the rotation last-write race (Risk register).
  fn tick_screen_wobble(
      mut commands: Commands,
      time: Res<Time>,
      mut q: Query<(Entity, &mut Transform, &mut ScreenWobble)>,
  ) {
      // Body per research §Pattern 5 tick_screen_wobble.
  }
  ```
- [ ] Implement `apply_anti_magic_zone` per research §Pattern 8:

  ```rust
  fn apply_anti_magic_zone(
      mut moved: MessageReader<MovedEvent>,
      floors: Res<Assets<DungeonFloor>>,
      dungeon_assets: Option<Res<DungeonAssets>>,
      party: Query<Entity, With<PlayerParty>>,
      has_zone: Query<(), With<AntiMagicZone>>,
      mut commands: Commands,
  ) {
      // Body per research §Pattern 8: add/remove AntiMagicZone marker based
      // on whether the destination cell is anti_magic_zone. Log enter/exit.
      // OQ5 caveat: entry_point in a zone won't add the marker until first
      // move. Document as designer convention; no code change.
  }
  ```
- [ ] Implement `Plugin for CellFeaturesPlugin::build` per research §Pattern 1:

  ```rust
  impl Plugin for CellFeaturesPlugin {
      fn build(&self, app: &mut App) {
          app.init_resource::<DoorStates>()
              .init_resource::<LockedDoors>()
              .init_resource::<PendingTeleport>()
              .add_message::<TeleportRequested>()
              .add_message::<EncounterRequested>()
              .add_systems(OnEnter(GameState::Dungeon), populate_locked_doors)
              .add_systems(OnExit(GameState::Dungeon), clear_door_resources)
              .add_systems(
                  Update,
                  (
                      handle_door_interact
                          .run_if(in_state(GameState::Dungeon))
                          .after(handle_dungeon_input),
                      apply_pit_trap
                          .run_if(in_state(GameState::Dungeon))
                          .after(handle_dungeon_input),
                      apply_poison_trap
                          .run_if(in_state(GameState::Dungeon))
                          .after(handle_dungeon_input),
                      apply_alarm_trap
                          .run_if(in_state(GameState::Dungeon))
                          .after(handle_dungeon_input),
                      apply_teleporter
                          .run_if(in_state(GameState::Dungeon))
                          .after(handle_dungeon_input),
                      apply_spinner
                          .run_if(in_state(GameState::Dungeon))
                          .after(handle_dungeon_input),
                      apply_anti_magic_zone
                          .run_if(in_state(GameState::Dungeon))
                          .after(handle_dungeon_input),
                      tick_screen_wobble
                          .run_if(in_state(GameState::Dungeon))
                          .after(animate_movement),  // win the rotation race
                  ),
              );
      }
  }
  ```
- [ ] In `src/main.rs`, register `CellFeaturesPlugin` (the only modification to `main.rs` per the Frozen list). After the existing `AudioPlugin` line at line 32:

  ```rust
  use druum::plugins::{
      audio::AudioPlugin, combat::CombatPlugin, dungeon::{DungeonPlugin, features::CellFeaturesPlugin},
      input::ActionsPlugin, loading::LoadingPlugin, party::PartyPlugin, save::SavePlugin,
      state::StatePlugin, town::TownPlugin, ui::UiPlugin,
  };

  // ... in App::new().add_plugins((...))
              DungeonPlugin,
              CellFeaturesPlugin,  // <-- NEW LINE
              CombatPlugin,
              // ... rest unchanged
  ```

  Verify with `cargo run --features dev` that the plugin is in the chain.
- [ ] Add Layer-2 app-driven tests in `mod app_tests` within `features.rs`. Use `MinimalPlugins + AssetPlugin::default() + StatesPlugin + StatePlugin + PartyPlugin + CellFeaturesPlugin` per research §Validation Architecture. Pattern from `inventory.rs::app_tests` and `audio/mod.rs:145-178`.

  Tests to add (each uses `make_test_app()` factory):

  ```rust
  #[cfg(test)]
  mod app_tests {
      use super::*;
      // ... harness (mirrors inventory.rs:780-1003)

      #[test]
      fn pit_trap_damages_party() { /* ... */ }

      #[test]
      fn pit_trap_with_target_floor_requests_teleport() { /* ... */ }

      #[test]
      fn poison_trap_applies_status() { /* ... */ }

      #[test]
      fn alarm_trap_publishes_encounter() { /* ... */ }

      #[test]
      fn same_floor_teleport_mutates_in_place() { /* ... */ }

      #[test]
      fn spinner_randomizes_facing_and_attaches_wobble() { /* ... */ }

      #[test]
      fn anti_magic_zone_lifecycle() { /* ... */ }
  }
  ```

  Each test:
  1. Builds the test app via `make_test_app()`.
  2. Manually inserts a tiny `DungeonFloor` via `Assets::add` with the relevant feature in the destination cell (e.g., a 2x2 floor with `features[1][1].trap = Some(TrapType::Pit { damage: 5, target_floor: None })`).
  3. Wraps the floor handle in a constructed `DungeonAssets` resource (use `init_resource` + manual mutation, or build a `commands.insert_resource(DungeonAssets { floor_01: handle, ... })` setup system).
  4. Spawns a `PlayerParty` entity with `GridPosition + Facing` + (for damage tests) a `PartyMemberBundle` for the party.
  5. Writes a `MovedEvent` directly via `app.world_mut().write_message(MovedEvent { from: ..., to: GridPosition { x: 1, y: 1 }, facing: ... })`.
  6. Calls `app.update()`.
  7. Asserts: `current_hp` reduced; `Messages<TeleportRequested>` drained with expected payload; `StatusEffects.effects` contains Poison; `Messages<EncounterRequested>` non-empty; `Facing` mutated; `ScreenWobble` component present; `AntiMagicZone` component present/absent.
- [ ] **Verification (atomic commit boundary):**
  - `cargo check` — succeeds (compile errors here surface mismatched function signatures).
  - `cargo check --features dev` — succeeds.
  - `cargo test plugins::dungeon::features::tests` — Layer-1 passes (4).
  - `cargo test plugins::dungeon::features::app_tests` — Layer-2 passes (7).
  - `cargo test` — full suite passes; existing dungeon/minimap/inventory tests must remain green.
  - `cargo run --features dev` — manual smoke: navigate via F9 cycler to Dungeon. Walk into the spinner cell at (2,2) — observe `SpinnerWhoosh` + facing change + brief rotation jitter. Walk into the pit at (4,4) — HP drops on all 4 party members. Walk into the alarm cell (none in floor_01 by default — skip; covered by Layer-2 test only).

### Phase 6 — `handle_dungeon_input` D9b edit (largest frozen-file modification)

Modify the frozen `dungeon/mod.rs::handle_dungeon_input` to consult `DoorStates` for door passability. This is the wrapper that closes Pitfall 4.

- [ ] In `src/plugins/dungeon/mod.rs`, add a new private helper function (place it among the other pure helpers, around line 270 near `wall_transform`):

  ```rust
  /// Returns `true` if the player can move from cell `(pos.x, pos.y)` in
  /// direction `dir`, consulting both static `DungeonFloor::can_move` AND
  /// runtime `DoorStates`.
  ///
  /// Truth table layered on top of `floor.can_move`:
  /// - `WallType::Door`:
  ///   - `DoorStates[(pos, dir)] == Some(Open)` → passable
  ///   - else → blocked (default `DoorState::Closed`; D15 — closed-by-default)
  /// - `WallType::LockedDoor`:
  ///   - `DoorStates[(pos, dir)] == Some(Open)` → passable (player has unlocked it)
  ///   - else → blocked
  /// - All other wall types: defer to `floor.can_move` (no DoorStates check).
  ///
  /// Pitfall 4 (Door is asset-passable; runtime closes by default).
  /// Pitfall 9 (LockedDoor is asset-blocked; runtime opens after unlock).
  fn can_move_with_doors(
      floor: &crate::data::DungeonFloor,
      door_states: &crate::plugins::dungeon::features::DoorStates,
      pos: GridPosition,
      dir: crate::data::dungeon::Direction,
  ) -> bool {
      // Look at the wall type FIRST; the DoorStates override only matters for
      // door types. For Door / LockedDoor, the override fully replaces
      // floor.can_move's verdict.
      let cell = match floor.walls.get(pos.y as usize).and_then(|row| row.get(pos.x as usize)) {
          Some(c) => c,
          None => return false,
      };
      let wall = match dir {
          Direction::North => cell.north,
          Direction::South => cell.south,
          Direction::East => cell.east,
          Direction::West => cell.west,
      };
      use crate::plugins::dungeon::features::DoorState;
      match wall {
          WallType::Door | WallType::LockedDoor => {
              let key = (pos, dir);
              let state = door_states.doors.get(&key).copied().unwrap_or_default();
              state == DoorState::Open
          }
          _ => floor.can_move(pos.x, pos.y, dir),
      }
  }
  ```
- [ ] Modify `handle_dungeon_input` (currently at line 618) to take a new `Res<DoorStates>` SystemParam and call `can_move_with_doors` instead of `floor.can_move`:

  ```rust
  pub(crate) fn handle_dungeon_input(
      mut commands: Commands,
      actions: Res<ActionState<DungeonAction>>,
      dungeon_assets: Option<Res<DungeonAssets>>,
      floors: Res<Assets<DungeonFloor>>,
      door_states: Res<crate::plugins::dungeon::features::DoorStates>,  // <-- NEW
      mut sfx: MessageWriter<SfxRequest>,
      mut moved: MessageWriter<MovedEvent>,
      mut query: Query<
          (Entity, &mut GridPosition, &mut Facing, &Transform),
          (With<PlayerParty>, Without<MovementAnimation>),
      >,
  ) {
      // ... existing body up to the passability check at line 666 ...

      // OLD: if !floor.can_move(pos.x, pos.y, move_dir) { return; }
      // NEW:
      if !can_move_with_doors(floor, &door_states, *pos, move_dir) {
          return;
      }

      // ... rest of body unchanged.
  }
  ```

  This is the **single line replacement** at line 666 plus the **single new SystemParam** in the function signature. Everything else in `handle_dungeon_input` is unchanged.
- [ ] Add 2 unit tests in `src/plugins/dungeon/mod.rs::tests` (or in `features.rs::tests` — pick whichever has the existing `make_floor`-style helper; `dungeon.rs::tests::make_floor` is the natural home, but adding to `dungeon/mod.rs::tests` requires a new test mod). The cleanest approach is to add them in `features.rs::tests` next to `door_states_resource_round_trip`, as pure tests on the wrapper logic.

  Tests:

  ```rust
  // In features.rs::tests (Phase 4's test mod, extended in Phase 6)

  #[test]
  fn can_move_with_doors_blocks_closed_door() {
      // Build a 2x2 floor with WallType::Door at east of (0,0).
      let floor = /* ... 2x2 with Door east face on (0,0) ... */;
      let door_states = DoorStates::default();  // empty — default Closed
      let pos = GridPosition { x: 0, y: 0 };
      assert!(
          !can_move_with_doors(&floor, &door_states, pos, Direction::East),
          "Default DoorState::Closed should block WallType::Door (Pitfall 4)"
      );
  }

  #[test]
  fn can_move_with_doors_passes_open_door() {
      let floor = /* ... 2x2 with Door east face on (0,0) ... */;
      let mut door_states = DoorStates::default();
      door_states.doors.insert(
          (GridPosition { x: 0, y: 0 }, Direction::East),
          DoorState::Open,
      );
      assert!(
          can_move_with_doors(&floor, &door_states, GridPosition { x: 0, y: 0 }, Direction::East),
          "DoorState::Open allows passage of WallType::Door"
      );
  }
  ```

  **Note on test placement:** if `can_move_with_doors` is `pub(crate)` in `dungeon/mod.rs`, importing it into `features.rs::tests` works via `use crate::plugins::dungeon::can_move_with_doors;`. If it stays private, declare the tests in `dungeon/mod.rs::tests` directly. Recommendation: keep it private to `dungeon/mod.rs` (it's an implementation detail of `handle_dungeon_input`), and put the wrapper tests in `dungeon/mod.rs::tests` next to existing dungeon tests.
- [ ] **Verification (atomic commit boundary):**
  - `cargo check` — succeeds.
  - `cargo test plugins::dungeon` — passes (existing tests + 2 new wrapper tests).
  - `cargo test` — full suite passes; existing minimap dark-zone tests + dungeon movement tests must remain green.
  - `cargo run --features dev` — manual smoke: walk into the door at (1,1)/(2,1) East — should be blocked (default Closed). Press F — should toggle Open. Walk through. Press F again from inside (2,1) facing West — should toggle back to Closed.

### Phase 7 — Cross-floor teleport (LoadingPlugin carve-out)

Extend `apply_teleporter` to handle cross-floor case. Add `handle_teleport_request` to `LoadingPlugin`. Modify `spawn_party_and_camera` in `dungeon/mod.rs` to read `PendingTeleport`.

- [ ] In `src/plugins/dungeon/features.rs::apply_teleporter`, replace the `// TODO Phase 7` comment with:

  ```rust
  } else {
      // Cross-floor: request via state-machine (D3-α).
      teleport.write(TeleportRequested { target: target.clone() });
  }
  ```

  Now both branches (same-floor mutate-in-place and cross-floor `TeleportRequested.write`) are live.
- [ ] In `src/plugins/loading/mod.rs`, add the consumer system. The `PendingTeleport` resource is already initialized by `CellFeaturesPlugin`; `LoadingPlugin` registers the message consumer (no duplicate `add_message` — Bevy 0.18's `add_message` is idempotent per Risk register).

  Add the system body:

  ```rust
  /// Consumes `TeleportRequested` and triggers a re-entry into
  /// `GameState::Loading -> GameState::Dungeon` with the destination stashed
  /// in `PendingTeleport`. The next `OnEnter(Dungeon)` reads the destination
  /// and overrides `floor.entry_point`.
  ///
  /// Runs in `Update` while in `GameState::Dungeon`. Reading `requests.read().last()`
  /// collapses multiple same-frame requests to the most recent (e.g., walking
  /// into a chain of teleporters in one tick — last writer wins).
  fn handle_teleport_request(
      mut requests: MessageReader<crate::plugins::dungeon::features::TeleportRequested>,
      mut pending: ResMut<crate::plugins::dungeon::features::PendingTeleport>,
      mut next: ResMut<NextState<GameState>>,
  ) {
      if let Some(req) = requests.read().last() {
          pending.target = Some(req.target.clone());
          next.set(GameState::Loading);
          info!(
              "Teleport requested to floor {} at ({}, {})",
              req.target.floor, req.target.x, req.target.y
          );
      }
  }
  ```

  Wire it in `LoadingPlugin::build` after the existing `add_systems` calls (currently at lines 116-117):

  ```rust
  .add_systems(OnEnter(GameState::Loading), spawn_loading_screen)
  .add_systems(OnExit(GameState::Loading), despawn_loading_screen)
  // Feature #13 cross-floor teleport (D3-α):
  .add_systems(Update, handle_teleport_request.run_if(in_state(GameState::Dungeon)));
  ```
- [ ] In `src/plugins/dungeon/mod.rs::spawn_party_and_camera` (currently lines 335-402), add a new SystemParam `pending_teleport: Option<ResMut<crate::plugins::dungeon::features::PendingTeleport>>` and use its values to override `floor.entry_point` if present:

  ```rust
  fn spawn_party_and_camera(
      mut commands: Commands,
      dungeon_assets: Option<Res<DungeonAssets>>,
      floors: Res<Assets<DungeonFloor>>,
      mut pending_teleport: Option<ResMut<crate::plugins::dungeon::features::PendingTeleport>>,
  ) {
      let Some(assets) = dungeon_assets else { /* ... */ };
      let Some(floor) = floors.get(&assets.floor_01) else { /* ... */ };

      // Feature #13 cross-floor teleport (D3-α):
      // If PendingTeleport is set, use its destination instead of floor.entry_point,
      // then clear it. Otherwise spawn at floor.entry_point as before.
      let (sx, sy, facing) = if let Some(ref mut pt) = pending_teleport {
          if let Some(target) = pt.target.take() {
              let facing = target.facing.unwrap_or(floor.entry_point.2);
              (target.x, target.y, facing)
          } else {
              floor.entry_point
          }
      } else {
          floor.entry_point
      };

      // ... rest of the body unchanged (use sx, sy, facing in place of the
      // existing destructured floor.entry_point binding).
  }
  ```

  **Critical:** `pt.target.take()` clears the resource after use so the next non-teleport `OnEnter(Dungeon)` (e.g., F9 cycle) doesn't accidentally reuse it.

  **For Phase 7 alone:** the `floor` handle being read is still `assets.floor_01`. Cross-floor teleport to floor 2 currently re-loads the same floor_01 (because `DungeonAssets` only has `floor_01`). Phase 8 adds `floor_02` to `DungeonAssets`; until then, the test harness must mock the floor handle in Layer-2 OR the manual smoke targets a same-floor teleport. The cross-floor end-to-end test is added in **Phase 8**.
- [ ] Add 1 Layer-2 test `cross_floor_teleport_publishes_request` in `features.rs::app_tests`:

  ```rust
  #[test]
  fn cross_floor_teleport_publishes_request() {
      let mut app = make_test_app();
      // Build a floor with a teleporter at (1,1) targeting floor 2.
      // ... Insert DungeonAssets with the test floor handle.
      // ... Spawn PlayerParty at (0,1) facing East.
      // Write MovedEvent { from: (0,1), to: (1,1), facing: East }.
      app.update();
      // Assert Messages<TeleportRequested> contains exactly one request with
      // target.floor == 2.
      let messages = app.world().resource::<Messages<TeleportRequested>>();
      // ... drain and assert.
  }
  ```
- [ ] **Verification (atomic commit boundary):**
  - `cargo check` — succeeds (compile errors here surface circular import issues; verify Risk register #29 is mitigated).
  - `cargo test plugins::dungeon::features::app_tests::cross_floor_teleport_publishes_request` — passes.
  - `cargo test` — full suite passes.
  - **No manual smoke yet for cross-floor** — that requires `floor_02` (Phase 8).

### Phase 8 — `floor_02.dungeon.ron` + `DungeonAssets` extension (D11-A)

Author the minimal floor 2 and wire its handle into `DungeonAssets`. Enables the cross-floor end-to-end manual smoke and the corresponding integration test.

- [ ] Create `assets/dungeons/floor_02.dungeon.ron` (NEW file). Minimal 4×4 single room with entry at (1,1) facing South, no features. Use `floor_01.dungeon.ron` as the structural template:

  ```ron
  // Floor 2 — minimal stub for Feature #13 cross-floor teleport testing.
  // 4×4 grid; single room; entry at (1,1) facing South.
  // No CellFeatures (clean test target — the player teleports here from
  // floor 1's (5,4) and lands cleanly).

  (
      name: "Test Floor 2",
      width: 4,
      height: 4,
      floor_number: 2,
      walls: [
          // Row y=0: top outer edge — all Solid
          [
              (north: Solid, south: Solid, east: Solid, west: Solid),
              (north: Solid, south: Open,  east: Solid, west: Solid),
              (north: Solid, south: Open,  east: Solid, west: Solid),
              (north: Solid, south: Solid, east: Solid, west: Solid),
          ],
          // Row y=1: entry point at (1,1)
          [
              (north: Solid, south: Open, east: Open,  west: Solid),
              (north: Open,  south: Open, east: Open,  west: Open),
              (north: Open,  south: Open, east: Solid, west: Open),
              (north: Solid, south: Open, east: Solid, west: Solid),
          ],
          // Row y=2:
          [
              (north: Open,  south: Open,  east: Open,  west: Solid),
              (north: Open,  south: Solid, east: Open,  west: Open),
              (north: Open,  south: Solid, east: Solid, west: Open),
              (north: Open,  south: Open,  east: Solid, west: Solid),
          ],
          // Row y=3: bottom edge
          [
              (north: Open,  south: Solid, east: Solid, west: Solid),
              (north: Solid, south: Solid, east: Solid, west: Solid),
              (north: Solid, south: Solid, east: Solid, west: Solid),
              (north: Open,  south: Solid, east: Solid, west: Solid),
          ],
      ],
      features: [
          [(), (), (), ()],
          [(), (), (), ()],
          [(), (), (), ()],
          [(), (), (), ()],
      ],
      entry_point: (1, 1, South),
      encounter_table: "test_table",
      lighting: (
          fog: (color: (0.10, 0.09, 0.08), density: 0.12),
          ambient_brightness: 1.0,
      ),
      locked_doors: [],
  )
  ```

  Verify wall-symmetry against `validate_wall_consistency` — the layout above is symmetric (no OneWay).
- [ ] In `src/plugins/loading/mod.rs`, add `floor_02: Handle<DungeonFloor>` to `DungeonAssets` (currently 5 fields at lines 30-41):

  ```rust
  #[derive(AssetCollection, Resource)]
  pub struct DungeonAssets {
      #[asset(path = "dungeons/floor_01.dungeon.ron")]
      pub floor_01: Handle<DungeonFloor>,
      // Feature #13 — minimal floor for cross-floor teleport testing (D11-A):
      #[asset(path = "dungeons/floor_02.dungeon.ron")]
      pub floor_02: Handle<DungeonFloor>,
      #[asset(path = "items/core.items.ron")]
      pub item_db: Handle<ItemDb>,
      // ... rest unchanged
  }
  ```

  `bevy_asset_loader` auto-discovers the new field via the `#[asset(...)]` derive — no further `LoadingState::with_collection` change needed (the existing `.load_collection::<DungeonAssets>()` at line 110 picks up all fields).
- [ ] In `src/plugins/dungeon/mod.rs::spawn_party_and_camera`, the simple Phase 7 implementation reads `assets.floor_01` regardless of which floor the player is on. **For Phase 8, this needs a small upgrade:** if `PendingTeleport.target.floor == 2`, read `assets.floor_02`; otherwise read `assets.floor_01`. Add a helper:

  ```rust
  fn floor_handle_for(assets: &DungeonAssets, floor_number: u32) -> &Handle<DungeonFloor> {
      match floor_number {
          1 => &assets.floor_01,
          2 => &assets.floor_02,
          n => {
              warn!("No DungeonFloor handle for floor {n}; falling back to floor_01");
              &assets.floor_01
          }
      }
  }
  ```

  Use it in `spawn_party_and_camera`:

  ```rust
  // Determine the active floor (cross-floor teleport may select floor_02).
  let active_floor_number = pending_teleport
      .as_ref()
      .and_then(|pt| pt.target.as_ref().map(|t| t.floor))
      .unwrap_or(1);
  let floor_handle = floor_handle_for(&assets, active_floor_number);
  let Some(floor) = floors.get(floor_handle) else { /* ... */ };
  ```

  Apply the same change to `populate_locked_doors` in `features.rs` so it reads the active floor's `locked_doors` (currently it hard-codes `assets.floor_01`).

  **Note:** for Phase 8 v1, the active-floor detection happens once on `OnEnter(Dungeon)`. A more robust approach (e.g., a `CurrentFloor: Resource(u32)`) is post-#13 polish.
- [ ] Add 1 Layer-2 integration test `cross_floor_teleport_end_to_end` (or extend the Phase 7 test to verify the resulting floor swap). Build a test app with both `floor_01` and `floor_02` Handles inserted into `DungeonAssets`; emit a `TeleportRequested { target: TeleportTarget { floor: 2, x: 1, y: 1, facing: Some(South) }}`; let `LoadingPlugin::handle_teleport_request` consume it; let the state transition fire (`app.update()` × 2-3 to process state-transitions); assert the player spawned at `(1, 1)` facing South in floor 2.
- [ ] **Verification (atomic commit boundary):**
  - `cargo check` — succeeds.
  - `cargo test` — full suite passes.
  - `cargo run --features dev` — manual smoke: walk to (5,4) on floor_01 (the teleporter cell). Observe brief loading flash. Player spawns on floor_02 at (1,1) facing South.

### Phase 9 — Final verification gate

Mirrors the 7-command gate from Feature #11/#12 plus #13-specific greps.

- [ ] `cargo check` — base build, no features.
- [ ] `cargo check --features dev` — dev build (no startup regressions in the F9 cycler or `spawn_default_debug_party`).
- [ ] `cargo clippy --all-targets -- -D warnings` — base clippy; zero warnings.
- [ ] `cargo clippy --all-targets --features dev -- -D warnings` — dev clippy; zero warnings.
- [ ] `cargo fmt --check` — formatting is clean.
- [ ] `cargo test` — all unit + integration tests pass (base).
- [ ] `cargo test --features dev` — all tests pass (dev).
- [ ] `rg 'derive\(.*\bEvent\b' src/plugins/dungeon/features.rs` — must return ZERO matches. Confirms `Message` (NOT `Event`) is the derive used.
- [ ] `rg '\bEventReader<' src/plugins/dungeon/features.rs tests/` — must return ZERO matches. Confirms `MessageReader` is used in features.rs and any tests.
- [ ] `rg '\bEventWriter<' src/plugins/dungeon/features.rs tests/` — must return ZERO matches. Confirms `MessageWriter` is used.
- [ ] `rg 'bevy::utils::HashMap' src/` — must return ZERO matches. Confirms `std::collections::HashMap` is used everywhere (Bevy 0.18 removed the `bevy::utils` re-export).
- [ ] **Frozen-file diff audit:** `git diff <pre-feature-13-commit-sha> HEAD --name-only`. Filter through the Frozen post-#12 list. Must show ONLY:
  - `src/main.rs` (1-line plugin add)
  - `src/plugins/dungeon/mod.rs` (3 edits: pub mod features, can_move_with_doors wrapper, spawn_party_and_camera PendingTeleport read)
  - `src/plugins/loading/mod.rs` (2 edits: 2 SFX fields, handle_teleport_request system + floor_02 handle)
  - `src/data/dungeon.rs` (1 additive field)
  - `src/data/items.rs` (1 additive field)
  - `tests/item_db_loads.rs` (1 assertion extension)
  - **NEW files only:** `src/plugins/dungeon/features.rs`, `assets/audio/sfx/spinner_whoosh.ogg`, `assets/audio/sfx/door_close.ogg`, `assets/dungeons/floor_02.dungeon.ron`
  - **MODIFIED assets:** `assets/items/core.items.ron`, `assets/dungeons/floor_01.dungeon.ron`
  - **NO modifications to:** `src/plugins/state/mod.rs`, `src/plugins/input/mod.rs`, `src/plugins/audio/mod.rs`, `src/plugins/audio/bgm.rs`, `src/plugins/ui/mod.rs`, `src/plugins/ui/minimap.rs`, `src/plugins/save/mod.rs`, `src/plugins/town/mod.rs`, `src/plugins/combat/mod.rs`, `src/plugins/party/*.rs`, `src/data/classes.rs`, `src/data/spells.rs`, `src/data/enemies.rs`. **Only `src/plugins/audio/sfx.rs` is touched in audio (per Frozen list explicit carve-out).**
- [ ] **Dependency audit:** `git diff <pre-feature-13-commit-sha> HEAD -- Cargo.toml Cargo.lock`. Must be byte-unchanged UNLESS D14 picked Option C (`rand = "0.8"` direct dep). If D14-C, surface the +1 dep explicitly in the verification gate output.
- [ ] **Test-count audit:** `cargo test 2>&1 | grep "test result"` should show: existing tests unchanged + 6-9 new tests (4 Layer-1 in `features.rs::tests`, 7 Layer-2 in `features.rs::app_tests`, 1 extended assertion in `tests/item_db_loads.rs`, 2 wrapper unit tests in `dungeon/mod.rs::tests`, 1 round-trip in `data/dungeon.rs::tests`, 1 round-trip in `data/items.rs::tests`). Total new: ~16 tests. **The roadmap budget at line 721 is +6-8** — if 16 feels excessive, the planner can drop the lower-value Layer-1 round-trip tests for `items.rs`/`dungeon.rs` (covered by the integration test). Recommended: ship 13 (drop 3 lower-value Layer-1).
- [ ] **Manual smoke checklist:** `cargo run --features dev`, navigate via F9 cycler (Title → Loading → TitleScreen → Dungeon). Verify:
  - **Door at (1,1)/(2,1) East:** walk into it from (1,1) facing East — blocked. Press F — open. Walk through. Press F from (2,1) facing West — closed. Walk back — blocked. **PASS criteria:** SfxKind::Door on open, SfxKind::DoorClose on close.
  - **LockedDoor at (3,1)/(4,1) East with `rusty_key`:** for the locked-door test, the implementer needs `rusty_key` in a party member's inventory. Either (a) hardcode `give_item(party, rusty_key_handle)` in a `#[cfg(feature = "dev")]` startup system (NOT in this plan; deferred to manual reasoning during smoke), or (b) skip this manual test and rely on the Layer-2 test `locked_door_unlocks_with_key`. **Recommendation: use the Layer-2 test for this verification; manual smoke verifies only the locked behavior (F press → no unlock, info! log).**
  - **Spinner at (2,2):** walk in — facing changes, SpinnerWhoosh plays, brief rotation jitter, minimap reflects new facing same frame.
  - **Pit at (4,4):** walk in — HP drops on all 4 party members (verify via `--features dev` debug console log if available; or via Bevy inspector).
  - **Teleporter at (5,4) → floor 2:** walk in — brief loading flash — player respawns at (1,1) on floor_02 facing South.
  - **dark_zone at (1,4):** walk in — minimap stays unseen (`?` glyph) for that cell. (Already implemented in #10.)
  - **anti_magic_zone at (2,4):** walk in — `tracing::info!("Entered anti-magic zone at GridPosition { x: 2, y: 4 }")` log appears. Walk out — `tracing::info!("Left anti-magic zone (now at ...)")` log appears.
- [ ] Update the `## Implementation Discoveries` section of THIS plan file with any unexpected findings during implementation.
- [ ] Update planner memory (`.claude/agent-memory/planner/`) with a new `project_druum_cell_features.md` entry summarizing the Feature #13 architectural decisions for future planners.

---

## Security

### Known Vulnerabilities

No known CVEs as of 2026-05-06 (research date) for any library used in #13. The dep set is unchanged from Feature #12; same status as #11/#12.

| Library | Version | Status |
|---------|---------|--------|
| serde | 1.x | No advisories |
| ron | 0.12 | No advisories |
| bevy | =0.18.1 | No advisories |
| bevy_common_assets | =0.16.0 | No advisories |
| bevy_asset_loader | =0.26.0 | No advisories |
| leafwing-input-manager | =0.20.0 | No advisories |
| rand | (transitive — likely via bevy_audio) | No advisories |

### Architectural Risks

The trust boundary for #13 is the on-disk `floor_*.dungeon.ron` and `core.items.ron` files (developer-authored; no modding for v1). The risks below are pre-mitigated by the saturating arithmetic, type-system constraints, and explicit guards in the relevant systems.

| Risk | How it manifests | Guard required by #13 |
|------|------------------|----------------------|
| Crafted `floor_*.dungeon.ron` with `damage: u32::MAX` on a Pit | Underflow / wraparound on `current_hp -= damage` | Phase 5 `apply_pit_trap` MUST use `saturating_sub` (Pitfall 7). Layer-1 test `pit_trap_subtracts_damage_saturating` exercises the case. |
| Crafted `floor_*.dungeon.ron` with massive `locked_doors` Vec | OOM via `Vec::with_capacity(1B)` | Trust boundary is "developer-authored RON". Out of scope for v1. Flag for #23: deserialize-time bound. |
| Crafted teleporter with `target_floor: u32::MAX` | Permanent Loading-state hang (no handle exists) | Phase 8's `floor_handle_for` falls back to `floor_01` with a `warn!` log. **Guard added in Phase 8.** |
| `key_id` containing path-traversal characters (`../etc/passwd`) | Used only as string-equality compare in `handle_door_interact` | String compare is safe; never used as filesystem path. **SAFE by design.** |
| `event_id` (existing `CellFeatures` field) used by future #14 scripted events | Documented at `dungeon.rs:171-173` — must be compile-time allow-list, never path/shell | **#13 v1 ignores `event_id`** entirely. Defer to #14+. |
| `EncounterRequested` source spoofing | Future #16 consumer | `EncounterSource` is a tagged enum (not a string) — type-safe. **SAFE by design.** |
| `DoorStates` not cleared between floors | Stale door-open state leaks to new floor | Phase 5 `clear_door_resources` runs on `OnExit(Dungeon)`. Pitfall 8 verified. |
| `TeleportRequested` race: multiple same-frame requests | Only the last is consumed; intermediate teleporters silently dropped | Phase 7 `handle_teleport_request` reads `requests.read().last()` — explicit "last writer wins" semantics. Documented. |
| Locked door unlock without inventory check | Bug — player walks through any locked door | Phase 5 `handle_door_interact` walks `Inventory(Vec<Entity>)` AND verifies `kind == KeyItem && key_id == Some(door_id)` before promoting `DoorState`. Layer-2 test `locked_door_unlocks_with_key` covers the success path; `locked_door_blocks_without_key` covers the failure path. |

**Trust boundary recap:** `floor_*.dungeon.ron` and `core.items.ron` are the only untrusted-shape inputs to #13. All risks are either pre-mitigated by the type system (saturating math, `u32` non-negativity, tagged-enum source) or explicitly guarded by validation in the relevant system. Save-file integrity is #23's problem. No network input (single-player game).

---

## Implementation Discoveries

### D-I2 — AudioAssets struct literal in audio/mod.rs tests also needs new fields

**File:** `src/plugins/audio/mod.rs`

**Finding:** Same `#[cfg(test)]` struct literal issue as D-I1. `AudioAssets` in audio/mod.rs test helper `make_test_app()` needed `sfx_spinner_whoosh` and `sfx_door_close` fields added (or `..Default::default()` — but `AudioAssets` lacks `Default` derive, so explicit fields required). Added them as `h.clone()` (stub handles like the rest).

**Fix applied:** Added `sfx_spinner_whoosh: h.clone()` and `sfx_door_close: h.clone()` to the stub `AudioAssets` in `audio/mod.rs` test helper. Bounded deviation in frozen file — minimum-correct fix.

### D-I1 — Plan claimed "existing tests pass unchanged" for `#[serde(default)]` fields; Rust struct literals require explicit values

**File:** `src/data/items.rs`, `src/data/dungeon.rs`, `src/plugins/dungeon/tests.rs`, `src/plugins/party/inventory.rs`, `src/plugins/ui/minimap.rs`

**Finding:** `#[serde(default)]` only applies to serde deserialization paths (ron/json). Rust struct literal initializations (in tests) that don't use `..Default::default()` fail to compile when new fields are added. The plan said "existing tests pass unchanged" but this only applies to RON round-trip tests that go through serde. Struct literal tests need `..Default::default()` added.

**Fix applied:** Added `..Default::default()` to all affected struct literal initializations across the 5 files. Also added explicit `locked_doors: Vec::new()` in `make_floor` helper in `data/dungeon.rs` (no `..Default::default()` idiom used there). Also added `locked_doors: Vec::new()` in `make_open_floor` and `make_walled_floor` in `dungeon/tests.rs` for clarity. The `minimap.rs` fix was a bounded deviation in a frozen file — minimum-correct fix.

---

## Estimated impact

Confirms the roadmap budget at lines 717-721:

| Dimension | Roadmap baseline | Plan-of-record |
|-----------|------------------|----------------|
| LOC Δ | +400 to +700 | **+450-650** (`features.rs` ~400-500 + `dungeon/mod.rs` +50 + `loading/mod.rs` +25 + `audio/sfx.rs` +6 + `data/dungeon.rs` +15 + `data/items.rs` +5 + `main.rs` +1 + `floor_01.dungeon.ron` +5 + `floor_02.dungeon.ron` +80 + `core.items.ron` +1 + `tests/item_db_loads.rs` +5) |
| Deps Δ | 0 | **0 (D14-A path) or +1 (D14-C path)** — Cargo.toml byte-unchanged unless D14 forces. |
| Compile Δ | small (+0.3s) | **+0.3s** (one new module, no new deps in recommended path) |
| Asset Δ | +2-4 (door textures, trap SFX) | **+2 .ogg + 1 RON** (D7-A: spinner_whoosh.ogg + door_close.ogg, D11-A: floor_02.dungeon.ron). +0 textures (existing wall materials reused from #8). |
| Test count Δ | +6-8 | **+13** (4 Layer-1 in features.rs::tests, 7 Layer-2 in features.rs::app_tests, 2 wrapper tests in dungeon/mod.rs::tests, 1 round-trip in data/dungeon.rs::tests, 1 round-trip in data/items.rs::tests, 1 assertion extension in tests/item_db_loads.rs). Above the roadmap budget (+6-8). Planner can trim 3-5 lower-value tests if budget tightens; Layer-2 tests are the highest-value coverage. |

**Cleanest-ship signal:** Same as Features #7, #8, #9, #10, #11, #12 — `Cargo.toml` byte-unchanged in the recommended D14-A path.

---

## Verification

- [ ] `data/items.rs` Layer-1 round-trip with `key_id` — Layer-1 unit — `cargo test data::items::tests::item_asset_round_trips_with_key_id` — Automatic
- [ ] `data/dungeon.rs` Layer-1 round-trip with `locked_doors` — Layer-1 unit — `cargo test data::dungeon::tests::dungeon_floor_round_trips_with_locked_doors` — Automatic
- [ ] `features.rs` Layer-1 saturating-sub guard — Layer-1 unit — `cargo test plugins::dungeon::features::tests::pit_trap_subtracts_damage_saturating` — Automatic
- [ ] `features.rs` Layer-1 default DoorState — Layer-1 unit — `cargo test plugins::dungeon::features::tests::door_state_default_is_closed` — Automatic
- [ ] `features.rs` Layer-1 DoorStates round-trip — Layer-1 unit — `cargo test plugins::dungeon::features::tests::door_states_resource_round_trip` — Automatic
- [ ] `features.rs` Layer-1 LockedDoors clear-first — Layer-1 unit — `cargo test plugins::dungeon::features::tests::locked_doors_clear_idempotent` — Automatic
- [ ] `features.rs` Layer-2 pit damages party — Layer-2 integration — `cargo test plugins::dungeon::features::app_tests::pit_trap_damages_party` — Automatic
- [ ] `features.rs` Layer-2 pit with target_floor publishes teleport — Layer-2 integration — `cargo test plugins::dungeon::features::app_tests::pit_trap_with_target_floor_requests_teleport` — Automatic
- [ ] `features.rs` Layer-2 poison applies status — Layer-2 integration — `cargo test plugins::dungeon::features::app_tests::poison_trap_applies_status` — Automatic
- [ ] `features.rs` Layer-2 alarm publishes encounter — Layer-2 integration — `cargo test plugins::dungeon::features::app_tests::alarm_trap_publishes_encounter` — Automatic
- [ ] `features.rs` Layer-2 same-floor teleport — Layer-2 integration — `cargo test plugins::dungeon::features::app_tests::same_floor_teleport_mutates_in_place` — Automatic
- [ ] `features.rs` Layer-2 spinner — Layer-2 integration — `cargo test plugins::dungeon::features::app_tests::spinner_randomizes_facing_and_attaches_wobble` — Automatic
- [ ] `features.rs` Layer-2 anti-magic lifecycle — Layer-2 integration — `cargo test plugins::dungeon::features::app_tests::anti_magic_zone_lifecycle` — Automatic
- [ ] `features.rs` Layer-2 cross-floor teleport publishes — Layer-2 integration — `cargo test plugins::dungeon::features::app_tests::cross_floor_teleport_publishes_request` — Automatic
- [ ] `dungeon/mod.rs` wrapper closed-door blocks — Layer-1 unit — `cargo test plugins::dungeon::tests::can_move_with_doors_blocks_closed_door` — Automatic
- [ ] `dungeon/mod.rs` wrapper open-door passes — Layer-1 unit — `cargo test plugins::dungeon::tests::can_move_with_doors_passes_open_door` — Automatic
- [ ] `core.items.ron` extended `key_id` assertion — Integration — `cargo test --test item_db_loads` — Automatic
- [ ] Cross-floor end-to-end (only if D11-A) — Layer-2 integration — `cargo test plugins::dungeon::features::app_tests::cross_floor_teleport_end_to_end` — Automatic
- [ ] `cargo check && cargo check --features dev` — Build — Automatic
- [ ] `cargo clippy --all-targets -- -D warnings && cargo clippy --all-targets --features dev -- -D warnings` — Lint — Automatic
- [ ] `cargo fmt --check` — Format — Automatic
- [ ] `cargo test && cargo test --features dev` — Full test suite — Automatic
- [ ] No `Event` derive sneaks in — Grep — `rg 'derive\(.*\bEvent\b' src/plugins/dungeon/features.rs` — Automatic (must return ZERO matches)
- [ ] No `EventReader` consumer sneaks in — Grep — `rg '\bEventReader<' src/plugins/dungeon/features.rs tests/` — Automatic (must return ZERO matches)
- [ ] No `EventWriter` consumer sneaks in — Grep — `rg '\bEventWriter<' src/plugins/dungeon/features.rs tests/` — Automatic (must return ZERO matches)
- [ ] No `bevy::utils::HashMap` import — Grep — `rg 'bevy::utils::HashMap' src/` — Automatic (must return ZERO matches)
- [ ] No edits to frozen files (Frozen post-#12 list) — Diff — `git diff <pre-feature-13-sha> HEAD --name-only` filtered through Frozen list — Manual (planner audits)
- [ ] Cargo.toml byte-unchanged (D14-A path) — Diff — `git diff <pre-feature-13-sha> HEAD -- Cargo.toml Cargo.lock` — Manual (or +1 dep if D14-C)
- [ ] Door at (1,1)/(2,1) East: closed-by-default; F opens; walk-through OK; F closes — Smoke — manual `cargo run --features dev` walkthrough — Manual
- [ ] LockedDoor at (3,1)/(4,1) East: blocked without key (verify via Layer-2 test or manual key-grant via dev console) — Smoke — `cargo test plugins::dungeon::features::app_tests::locked_door_unlocks_with_key` (Layer-2) — Automatic
- [ ] Spinner at (2,2): facing changes, SpinnerWhoosh, rotation jitter, minimap reflects new facing same frame — Smoke — manual `cargo run --features dev` — Manual
- [ ] Pit at (4,4): HP drops on all 4 party members — Smoke — manual `cargo run --features dev` (verify via dev console log or Bevy inspector) — Manual
- [ ] Teleporter at (5,4) → floor 2: brief loading flash; player respawns at (1,1) on floor_02 facing South — Smoke — manual `cargo run --features dev` (D11-A required) — Manual
- [ ] dark_zone at (1,4): minimap stays unseen — Smoke — manual `cargo run --features dev` (already implemented in #10; regression check) — Manual
- [ ] anti_magic_zone at (2,4): info! log on enter and exit — Smoke — manual `cargo run --features dev` with log filter — Manual
- [ ] Plan's "Implementation Discoveries" section populated — Documentation — manual review of THIS plan file post-implementation — Manual

---

## Notes for the orchestrator

- **D3, D10, D11, D14** are genuine USER PICK decisions. The plan's body assumes the recommended option (α / A / A / A respectively) throughout. If the user picks a non-recommended option:
  - **D3 → β:** drop Phase 7 LoadingPlugin carve-out; rewrite the cross-floor branch in `apply_teleporter` to despawn+respawn `DungeonGeometry` and `PlayerParty` in-state. ~80 LOC of refactor in `dungeon/mod.rs` (significant scope expansion). Rebuild Phase 7-8 from scratch.
  - **D10 → B (+5 variants):** add 3 more `SfxKind` variants (`TrapTrigger`, `PitDamage`, `Teleport`) to Phase 3, plus 3 more `.ogg` files. ~10 LOC + 3 audio assets.
  - **D10 → C (+1 variant):** drop `DoorClose` from Phase 3; door-close reuses `Door`. ~3 LOC less.
  - **D11 → B/C (defer floor_02):** drop Phase 8 entirely; Phase 7's `cross_floor_teleport_publishes_request` is the only cross-floor coverage; manual smoke for cross-floor is deferred or mocked.
  - **D14 → C (+1 dep):** add `rand = "0.8"` to Cargo.toml. Breaks the byte-unchanged signal — surface in Phase 9 explicitly.
- **D14 verification is BLOCKING** at the start of Phase 5. Implementer runs `cargo tree -i rand` from project root BEFORE writing `apply_spinner`. If the result is non-empty, use `rand` directly. If empty, use `Time::elapsed_secs_f64() as u64 % 4` deterministic fallback. If neither is acceptable to the user, reopen the decision before continuing.
- **The Phase 6 D9b edit to `dungeon/mod.rs::handle_dungeon_input`** is the largest single edit to a previously-frozen module in #13. Audit the wrapper function `can_move_with_doors` carefully — it's the gate between asset-level passability and runtime door state. Existing `dungeon/mod.rs::tests` should still pass (the wrapper is a strict layer on top; default `DoorStates` empty case produces `state == Closed` for door types, which differs from pre-#13 behavior — **this is intentional and documented**).
- **The schema edits to `data/dungeon.rs` and `data/items.rs` are additive and `#[serde(default)]`** — existing tests pass unchanged. Verify with `cargo test data::` immediately after Phase 1 commits before writing consumer code.
- **`floor_02.dungeon.ron`** is OPTIONAL (D11-A vs B/C). The plan assumes D11-A. If user picks B, the cross-floor end-to-end test goes manual and Phase 8 collapses to "skip — manual smoke only".
- **Pre-commit hook on `gitbutler/workspace`** rejects raw `git commit`. The implementer uses `but commit --message-file <path>` (CLAUDE.md). One commit per phase boundary; 9 commits total.
