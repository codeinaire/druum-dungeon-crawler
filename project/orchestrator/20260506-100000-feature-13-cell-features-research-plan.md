# Pipeline Summary — Feature #13: Cell Features (Doors, Traps, Teleporters, Spinners) (Research → Plan)

**Date:** 2026-05-06
**Pipeline scope:** research → plan → STOP (parent dispatches implementer manually after user approves the plan, per established Feature #3-#12 pattern)
**Status:** Plan ready for user review. Implementation, ship, and review NOT IN SCOPE for this run.

---

## Original task

Drive the research → plan pipeline for **Feature #13: Cell Features (Doors, Traps, Teleporters, Spinners)** from the Druum (Bevy 0.18 first-person dungeon crawler) roadmap (`project/roadmaps/20260429-01-bevy-dungeon-crawler-roadmap.md` lines 688-737). Difficulty 3/5 — each individual feature is small but they all interact with movement (#7), audio (#6), inventory (#12), status effects (#14, future), encounters (#16, future).

**In scope:** seven `MovedEvent`-driven systems (door interact, pit/poison/alarm/teleporter trap, spinner, anti-magic zone) plus screen-wobble tick; one new file `src/plugins/dungeon/features.rs`; two additive `#[serde(default)]` schema fields (`ItemAsset.key_id`, `DungeonFloor.locked_doors`); two new `SfxKind` variants (`SpinnerWhoosh`, `DoorClose`) with `.ogg` placeholders; cross-floor teleport via re-entering `GameState::Loading` (small carve-out into otherwise-frozen `LoadingPlugin`); a wrapper `can_move_with_doors` to layer runtime door state on top of `DungeonFloor::can_move` (one frozen-file edit to `dungeon/mod.rs::handle_dungeon_input`); minimal `floor_02.dungeon.ron` for cross-floor end-to-end testing; `floor_01.dungeon.ron` and `core.items.ron` asset edits for the `locked_doors` and `key_id` fields.

**Out of scope:**
- Inventory UI for selecting which key to use (#25 — locked-door system uses automatic-key-lookup over all party inventories).
- Save/Load of toggled door state (#23).
- Real combat triggered by alarm trap (#16 — v1 publishes `EncounterRequested` with logged-only consumer).
- Real spell-casting blocked by anti-magic zone (#14/#15 — v1 attaches/detaches `AntiMagicZone` marker; readers don't exist yet).
- `event_id` scripted events (#14+ — field exists in `CellFeatures` but #13 v1 ignores it).
- Trap detection by Luck stat (#14/#21).
- Multi-cell features / boss arenas (Con of #4; out of scope).
- Door opening visual swing animation (#25 polish).
- Cross-party-member key sharing UI (lookup already walks all party inventories — no UI needed).

**Constraint envelope:** +400-700 LOC, **0 new deps** (D14: verify `rand` transitively via `cargo tree -i rand` before writing the spinner), +13 tests (above the roadmap +6-8 budget; planner can trim 3-5 lower-value tests if needed), +0.3s compile, +2 .ogg + 1 RON asset Δ. Same cleanest-ship signal as #7-#12.

---

## Artifacts produced

| Step | Description | Path |
|---|---|---|
| 1 | Research | `/Users/nousunio/Repos/Learnings/claude-code/druum/project/research/20260506-080000-feature-13-cell-features.md` |
| 2 | Plan | `/Users/nousunio/Repos/Learnings/claude-code/druum/project/plans/20260506-100000-feature-13-cell-features.md` (Status: Draft — awaiting user OK on D3 / D10 / D11 / D14) |
| - | This summary | `/Users/nousunio/Repos/Learnings/claude-code/druum/project/orchestrator/<this-file>.md` |

---

## What research found (the load-bearing facts)

The researcher read the merged code for every dependency #4 / #5 / #6 / #7 / #8 / #9 / #10 / #11 / #12 in full at file:line. Key findings — the roadmap §13 is mostly accurate, but several pieces are already done OR have subtle integration constraints the implementer must respect:

1. **`floor_01.dungeon.ron` is ALREADY a #13 testbed.** `assets/dungeons/floor_01.dungeon.ron` was authored by #4 with: Door at (1,1)/(2,1) East, LockedDoor at (3,1)/(4,1) East, spinner at (2,2), Pit at (4,4) targeting floor 2, Teleporter at (5,4) → floor 2, dark_zone at (1,4), anti_magic_zone at (2,4). Verified by `dungeon.rs:790-809` integration test. **#13 does not author a new floor_01** — it adds a `locked_doors: [((3,1), East, "rusty_door_01")]` field.

2. **Dark zones ALREADY DONE.** `src/plugins/ui/minimap.rs:208-211` already gates `ExploredCells` insertion on `floor.features[y][x].dark_zone`. **#13 adds zero work for dark zones** — the roadmap line "implement dark zones" is already-implemented work. (D4 mostly auto-resolved.)

3. **Auto-map facing update on spinner is FREE.** Minimap reads `&Facing` directly via `Query` at `minimap.rs:269, 309, 318`. When the spinner mutates `Facing` in `Update`, the minimap painter (running in `EguiPrimaryContextPass` after `PostUpdate`) reflects the new facing on the SAME frame — no `SpunEvent`, no synthetic `MovedEvent` publish needed.

4. **`MovedEvent` is published on the COMMIT FRAME, not after the tween.** `dungeon/mod.rs:686-690` writes `MovedEvent { from, to, facing }` immediately after `GridPosition`/`Facing` mutations, BEFORE `MovementAnimation` finishes its 0.18s lerp. The doc comment at `dungeon/mod.rs:30-34` is explicit: "Downstream consumers (#13 cell-trigger, #16 encounter) react to the new logical state on the commit frame, not after the tween completes." This is exactly what #13 needs for trap-on-step semantics.

5. **`DungeonAction::Interact` is already wired.** `input/mod.rs:78` declares the variant; line 149 binds it to `KeyCode::KeyF`. **#13 does NOT touch the input plugin.** (D8 auto-resolved.)

6. **`Door` is currently passable per `floor.can_move`** (`dungeon.rs:79-83`). The asset-level "closed but unlocked" reads as already-passable. **#13 introduces a runtime layer** (`DoorStates: Resource`) that gates passability on top of `floor.can_move`. **`handle_dungeon_input` MUST be modified** to call a new `can_move_with_doors(floor, doors, pos, dir)` wrapper instead of `floor.can_move` directly. This is the **largest single edit to a frozen module** in #13. (Pitfall 4 / D9b.)

7. **`LockedDoor` is impassable per `floor.can_move`.** After unlocking via `handle_door_interact`, `DoorStates[(pos, dir)] = Open` overrides — the wrapper must NOT short-circuit on `floor.can_move == false` for `LockedDoor`; it must check `DoorStates` first for door types. (Pitfall 9.)

8. **`#11/#12` shipped everything #13 needs from inventory.** `ItemKind::KeyItem` exists at `inventory.rs:79`. `Inventory(Vec<Entity>)` + `ItemInstance(Handle<ItemAsset>)` pattern at `inventory.rs:167-187`. `ItemHandleRegistry::get(&str) -> Option<&Handle<ItemAsset>>` at `inventory.rs:500-521` for ID→handle lookup. `ItemAsset` has 9 fields (verified `items.rs:84-113`); **`key_id` does NOT yet exist** — #13 adds it as `Option<String>` with `#[serde(default)]`. `rusty_key` already exists in `core.items.ron` with `kind: KeyItem, slot: None` — #13 adds `key_id: Some("rusty_door_01")` to that entry.

9. **Cross-floor teleport is heavyweight; same-floor is trivial.** `TrapType::Teleport(TeleportTarget)` and `CellFeatures::teleporter: Option<TeleportTarget>` share the `TeleportTarget` payload — one `apply_teleporter` system handles both invocation paths. Same-floor: mutate `GridPosition` + `Facing` + write a synthetic `MovedEvent` so minimap & dark-zone gate fire. Cross-floor: publish `TeleportRequested` Message to `LoadingPlugin`, which sets `NextState(GameState::Loading)` and stashes the destination in `PendingTeleport: Resource`. On `OnEnter(Dungeon)`, `spawn_party_and_camera` reads `PendingTeleport` if present and uses its `(x, y, facing)` instead of `floor.entry_point`. **The carve-out into `LoadingPlugin` is small** (~25 LOC) and reuses 100% of existing despawn-recursive cleanup. The brief loading flash is genre-correct (Wizardry/Etrian).

10. **`floor_02.dungeon.ron` does NOT exist.** The live `floor_01` has a teleporter at (5,4) targeting `floor: 2`. `DungeonAssets` (`loading/mod.rs:31-32`) only declares `floor_01`. **#13 v1 must author a minimal `floor_02.dungeon.ron`** (~80 lines, 4×4 single room, entry at (1,1) South) AND extend `DungeonAssets` with `floor_02: Handle<DungeonFloor>`. Surface as D11; recommended yes.

11. **`SfxKind` has 5 variants** (`Footstep, Door, EncounterSting, MenuClick, AttackHit`) at `audio/sfx.rs:51-57`. `AudioAssets` (`loading/mod.rs:50-76`) has matching 5 sfx_* handle fields. **#13 adds 2 variants** (`SpinnerWhoosh` + `DoorClose`) — touches `audio/sfx.rs` AND `loading/mod.rs`. The "FROZEN post-#3" comment on `LoadingPlugin` applies to state-transition logic, NOT to the `AudioAssets` field list (verified — the 5 SFX fields were added in #6, after the freeze comment). Reuse `Door` for door-open + trap-snap, `AttackHit` for pit damage, `EncounterSting` for alarm. (D10-A.)

12. **Bevy 0.18 conventions:** `#[derive(Message)]` not `#[derive(Event)]`; `MessageReader<T>` / `MessageWriter<T>` not `EventReader/Writer`; `app.add_message::<T>()` not `add_event`; `std::collections::HashMap` not `bevy::utils::HashMap` (removed). The verification gate greps for these mistakes in `features.rs` and tests.

---

## What the plan delivers (architecture chosen)

**Architecture:** single-file `src/plugins/dungeon/features.rs` (~400-600 LOC) holding:
- **3 resources:** `DoorStates(HashMap<(GridPos, Direction), DoorState>)`, `LockedDoors(HashMap<(GridPos, Direction), String>)`, `PendingTeleport(Option<TeleportTarget>)`
- **2 components:** `AntiMagicZone` (marker, added on enter / removed on exit), `ScreenWobble { elapsed_secs, duration_secs, amplitude }` (200ms damped-sine rotation jitter)
- **2 messages:** `TeleportRequested { target: TeleportTarget }`, `EncounterRequested { source: EncounterSource }` (with `EncounterSource::AlarmTrap` and forward slots for `Random`/`Foe` in #16)
- **1 enum:** `DoorState` (Closed | Open; default Closed per D15)
- **9 systems:**
  - `populate_locked_doors` (`OnEnter(Dungeon)`, clear-first idempotent per Pitfall 8)
  - `clear_door_resources` (`OnExit(Dungeon)`)
  - `handle_door_interact` (`Update.run_if(Dungeon).after(handle_dungeon_input)` — toggles Closed↔Open on Interact; consults inventory for LockedDoor)
  - `apply_pit_trap` (Update — saturating_sub on all party HP; if `target_floor.is_some()`, also publishes `TeleportRequested`)
  - `apply_poison_trap` (Update — push `ActiveEffect{Poison, remaining_turns: Some(5)}` on each party member)
  - `apply_alarm_trap` (Update — publish `EncounterRequested { source: AlarmTrap }`; logged-only consumer)
  - `apply_teleporter` (Update — same-floor branch: mutate `GridPosition`/`Facing`/`Transform`, re-publish `MovedEvent`; cross-floor branch: publish `TeleportRequested`)
  - `apply_spinner` (Update — random `Direction` ≠ current; mutate `Facing`; emit `SfxKind::SpinnerWhoosh`; attach `ScreenWobble`)
  - `tick_screen_wobble` (Update.after(animate_movement) — damped-sine rotation jitter; remove component on completion)
  - `apply_anti_magic_zone` (Update — add/remove `AntiMagicZone` marker on enter/exit)

**Cross-floor teleport (D3 Option α):** `LoadingPlugin` carve-out adds `add_message::<TeleportRequested>()`, `init_resource::<PendingTeleport>()`, and a new `handle_teleport_request` system that reads `TeleportRequested`, populates `PendingTeleport.target`, then `next.set(GameState::Loading)`. `dungeon/mod.rs::spawn_party_and_camera` reads `Option<Res<PendingTeleport>>` and uses its `(x, y, facing)` instead of `floor.entry_point` if present (then `commands.remove_resource::<PendingTeleport>()`). This is the genre-canonical loading-flash flow.

**`handle_dungeon_input` D9b edit:** `dungeon/mod.rs::handle_dungeon_input` (currently calling `floor.can_move(pos.x, pos.y, move_dir)` at line 666) is modified to take `Res<DoorStates>` and call a new `can_move_with_doors(floor, doors, pos, dir)` wrapper. The wrapper checks the wall type FIRST: if `Door` or `LockedDoor`, returns `DoorStates[(pos, dir)] == Some(Open)` (overriding `floor.can_move`); otherwise defers to `floor.can_move`. This closes Pitfalls 4 and 9.

**9 atomic phases** — each phase a single source-coherent commit boundary with `cargo test` passing at exit. The phases are: (1) data schema, (2) asset edits, (3) audio additions, (4) `features.rs` skeleton, (5) `CellFeaturesPlugin` + 9 systems + Layer-2 tests, (6) `handle_dungeon_input` D9b wrapper, (7) cross-floor teleport carve-out, (8) `floor_02.dungeon.ron` + `DungeonAssets.floor_02`, (9) full 7-command verification gate + 4 grep audits + frozen-file diff + manual smoke checklist.

---

## Decisions surfaced (Category B — user input requested)

The research surfaced D1-D15. The plan classifies them as below; the orchestrator surfaces only the genuine USER PICKs for confirmation.

### Auto-resolved by live code (do NOT surface unless user asks)

- **D2c — Schema edit on `data/dungeon.rs`:** Additive `locked_doors` field with `#[serde(default)]`. Existing tests pass unchanged. **Proceed.**
- **D4 — Anti-magic / dark-zone scope:** Dark zone is ALREADY HANDLED at `minimap.rs:208-211` — no #13 work. Anti-magic ships as plumbing only (marker component + `tracing::info! ` log). **Proceed.**
- **D8 — `DungeonAction::Interact`:** Already exists at `input/mod.rs:78`, bound to `KeyCode::KeyF` at line 149. **No #13 input work.**
- **D9b — `handle_dungeon_input` wrapper modification:** Side-effect of D9. The `can_move_with_doors` wrapper is the cleanest implementation. Largest frozen-file edit but bounded. **Proceed.**

### Recommended defaults — proceed unless user objects

- **D1 — Door state persistence:** per-floor-instance only (cleared on `OnExit(Dungeon)`). Cross-floor persistence is a #23 concern. **Recommended.**
- **D2 — KeyItem representation:** `key_id: Option<String>` on `ItemAsset` paired with `ItemKind::KeyItem`. Additive schema, RON-authored. **Recommended.**
- **D2b — `door_id` location:** side-table `locked_doors: Vec<((u32,u32), Direction, String)>` on `DungeonFloor`. Doesn't ripple `WallType` variants. **Recommended.**
- **D5 — Encounter trap stubbing:** publish `EncounterRequested { source: EncounterSource::AlarmTrap }` with logged-only consumer (~10 LOC). Defines the interface for #16 cleanly. **Recommended.**
- **D6 — Screen wobble:** `Quat::from_rotation_z(jitter)` rotation jitter on player Transform, 200ms damped sine. ~30 LOC. **Recommended over a custom shader.**
- **D7 — SFX strategy:** royalty-free .ogg files committed to `assets/audio/sfx/` (matches #6 precedent). User supplies the 2 .ogg files OR the implementer suggests CC0 sources (e.g., freesound.org). **Recommended.**
- **D9 — Door state shape:** `DoorStates: Resource(HashMap<(GridPos, Direction), DoorState>)`. O(1) lookup; cleared on `OnExit`. **Recommended.**
- **D12 — Poison stacking:** naive push (each trap entry adds another `ActiveEffect`). Matches #14's expected behavior (which roadmap §14 line 781 punts). **Recommended.**
- **D13 — Locked-door key consumption:** NOT consumed (Wizardry-style; reusable). Roadmap line 731 hedge ("consumes / requires") resolved here. **Recommended.**
- **D15 — Doors closed-by-default:** player presses Interact to open. Genre canon. **Recommended.**

### Genuine USER PICK — orchestrator wants confirmation before kickoff

- **D3 — Cross-floor teleporter implementation:**
  - **α (RECOMMENDED):** Re-enter `GameState::Loading` via `TeleportRequested` Message; `LoadingPlugin` owns the consumer + `PendingTeleport: Resource`; `spawn_party_and_camera` reads `PendingTeleport` to override `floor.entry_point`. Brief loading-screen flash is genre-correct (Wizardry/Etrian). Smallest code change. Composes with #23 save/load.
  - β: In-state asset swap. Visually smoother but bypasses `bevy_asset_loader` guarantees and requires ~80 LOC of refactor in `dungeon/mod.rs` (significant scope expansion). Not recommended.
  - γ: Hybrid (Loading re-enter only if target floor not yet loaded; otherwise in-state swap). Doubles test surface. Not recommended.

- **D10 — Number of new `SfxKind` variants:**
  - **A (RECOMMENDED):** +2 variants — `SpinnerWhoosh`, `DoorClose`. Reuse `Door` for door-open and trap-snap; reuse `AttackHit` for pit damage; reuse `EncounterSting` for alarm. Matches roadmap "+2-4 trap SFX" budget at line 720.
  - B: +5 variants (`SpinnerWhoosh`, `DoorClose`, `TrapTrigger`, `PitDamage`, `Teleport`). More expressive audio; +3 .ogg files.
  - C: +1 variant (only `SpinnerWhoosh`). Minimum-touch; reuses `Door` for door-open AND door-close.

- **D11 — `floor_02.dungeon.ron` for cross-floor testing:**
  - **A (RECOMMENDED):** Author a minimal `floor_02.dungeon.ron` (~80 lines, 4×4 single room, entry at (1,1) South); add `floor_02: Handle<DungeonFloor>` to `DungeonAssets`. Cost: ~80 lines RON + 2 lines code. Unlocks the cross-floor end-to-end integration test.
  - B: Defer cross-floor end-to-end test to manual smoke. Saves ~80 lines RON but no automated coverage; Phase 8 collapses to "skip — manual smoke only".
  - C: Mock floor 2 in tests via custom Asset insertion (no production floor). Test-only stub.

- **D14 — `rand` dependency check (BLOCKING at impl-time):**
  - **A (RECOMMENDED):** Verify with `cargo tree -i rand` BEFORE writing the spinner. Likely outcome: present (transitively via bevy_audio / bevy_pbr). Use directly if present.
  - B: Use `Time::elapsed_secs_f64() as u64 % 4` deterministic fallback. Acceptable for v1 spinner UX.
  - C: Add `rand = "0.8"` to Cargo.toml (Δ deps = +1 — breaks the 0-deps signal). Surface in Phase 9.
  - **Action:** the implementer reports outcome at Phase 5 kick-off and chooses B or C if A reveals `rand` is missing.

---

## Deviations from the roadmap

The plan deviates from the roadmap text in these documented places — most because the roadmap predates Features #4-#12 and the live source has evolved past the roadmap's framing:

1. **`floor_01.dungeon.ron` is NOT augmented in #13** (roadmap line 710 says it should be). #4 already authored it as a #13 testbed; #13 only adds the `locked_doors` field (Phase 2).
2. **Dark zones require ZERO new code** (roadmap line 694 implies #13 implements them). #10 already wired `minimap.rs:208-211` to skip dark cells.
3. **`KeyItem` representation:** roadmap line 708 is ambiguous ("New `KeyItem` flag on `Item` (or a tag component)"). Plan uses `key_id: Option<String>` field on `ItemAsset` paired with `ItemKind::KeyItem` (not a flag, not a tag component) — D2-A.
4. **Cross-floor teleport via `Loading` re-entry, not in-state swap.** Roadmap line 728 says "despawn current floor, load new floor, set new `GridPosition`" — neutral on which mechanism. Plan uses D3-α (re-enter `Loading`) with a small `LoadingPlugin` carve-out.
5. **Locked-door keys NOT consumed.** Roadmap line 731 ("consumes / requires") resolved per Wizardry canon — D13.
6. **Spinner SFX uses 2 new `SfxKind` variants, not 4.** Roadmap line 732 + line 720 ("+2-4 door textures, trap SFX") — plan picks the lower bound (D10-A).
7. **Door visual representation does not change on Open.** Wall plate still rendered. Player notices via SFX. Visual swap (despawn / wireframe) deferred to #25 polish (OQ6).
8. **Anti-magic zone is plumbing-only in v1.** No #14/#15 consumer exists; ship the marker component + add/remove system + `tracing::info! ` log only.
9. **Alarm trap publishes `EncounterRequested` Message.** No #16 consumer; logged-only.

All deviations are documented inline in the plan's Out-of-scope, Frozen-list, and Open Decisions sections.

---

## Files the implementer will create or modify

### NEW files (4)

- `src/plugins/dungeon/features.rs` (~400-600 LOC) — 3 resources + 2 components + 2 messages + 1 enum + 9 systems + Layer-1 + Layer-2 tests + plugin definition.
- `assets/audio/sfx/spinner_whoosh.ogg` — placeholder (D7-A: royalty-free CC0).
- `assets/audio/sfx/door_close.ogg` — placeholder (D7-A: royalty-free CC0).
- `assets/dungeons/floor_02.dungeon.ron` (~80 lines) — minimal 4×4 single-room floor for cross-floor end-to-end test (D11-A).

### MODIFIED files (8 — explicit frozen-file carve-outs)

- `src/plugins/dungeon/mod.rs` — 3 edits: (a) `pub mod features;` declaration, (b) `can_move_with_doors` wrapper helper + `handle_dungeon_input` calls it instead of `floor.can_move` (D9b), (c) `spawn_party_and_camera` reads `Option<Res<PendingTeleport>>` and uses its `(x, y, facing)` instead of `floor.entry_point` if present (then `commands.remove_resource::<PendingTeleport>()`).
- `src/plugins/loading/mod.rs` — 3 edits: (a) +2 sfx_* handle fields on `AudioAssets` (`sfx_spinner_whoosh`, `sfx_door_close`), (b) `handle_teleport_request` system + `add_message::<TeleportRequested>()` + `init_resource::<PendingTeleport>()` (D3-α carve-out), (c) `floor_02: Handle<DungeonFloor>` field on `DungeonAssets` (D11-A).
- `src/plugins/audio/sfx.rs` — +2 `SfxKind` variants (`SpinnerWhoosh`, `DoorClose`) + 2 match arms in `handle_sfx_requests`.
- `src/data/dungeon.rs` — +1 additive `#[serde(default)]` field `locked_doors: Vec<((u32,u32), Direction, String)>` on `DungeonFloor`; +1 round-trip test.
- `src/data/items.rs` — +1 additive `#[serde(default)]` field `key_id: Option<String>` on `ItemAsset`; +1 round-trip test.
- `src/main.rs` — +1 line: `app.add_plugins(CellFeaturesPlugin)`.
- `assets/dungeons/floor_01.dungeon.ron` — +5 lines: `locked_doors: [((3, 1), East, "rusty_door_01")]` field.
- `assets/items/core.items.ron` — +1 line: `key_id: Some("rusty_door_01")` on `rusty_key` entry.
- `tests/item_db_loads.rs` — +5 LOC: extend `assert_item_db_shape` to verify `rusty_key.key_id`.

### Frozen — DO NOT TOUCH

`src/plugins/state/mod.rs` (FROZEN by #2), `src/plugins/input/mod.rs` (FROZEN by #5; Interact already wired), `src/plugins/audio/mod.rs` + `src/plugins/audio/bgm.rs` (FROZEN by #6; only `audio/sfx.rs` touched), `src/plugins/ui/mod.rs` + `src/plugins/ui/minimap.rs` (FROZEN by #10; dark-zone gate already implemented), `src/plugins/save/mod.rs` + `src/plugins/town/mod.rs` + `src/plugins/combat/mod.rs` (FROZEN / does not exist), `src/plugins/party/mod.rs` + `src/plugins/party/character.rs` + `src/plugins/party/inventory.rs` (FROZEN by #11/#12; read-only for #13), `src/data/classes.rs` + `src/data/spells.rs` + `src/data/enemies.rs` (FROZEN-from-day-one), `Cargo.toml` + `Cargo.lock` (byte-unchanged unless D14-C). The Plan §"Frozen post-#12" enumerates these with a `git diff --name-only` audit in Phase 9.

### Types/events introduced

- **Components:** `AntiMagicZone` (marker), `ScreenWobble { elapsed_secs: f32, duration_secs: f32, amplitude: f32 }`
- **Resources:** `DoorStates(HashMap<(GridPosition, Direction), DoorState>)`, `LockedDoors { by_edge: HashMap<(GridPosition, Direction), String> }`, `PendingTeleport { target: Option<TeleportTarget> }`
- **Messages:** `TeleportRequested { target: TeleportTarget }` (`#[derive(Message)]`), `EncounterRequested { source: EncounterSource }` (`#[derive(Message)]`)
- **Enums:** `DoorState { Closed (default), Open }`, `EncounterSource { AlarmTrap, /* Random + Foe in #16 */ }`
- **Systems:** `populate_locked_doors`, `clear_door_resources`, `handle_door_interact`, `apply_pit_trap`, `apply_poison_trap`, `apply_alarm_trap`, `apply_teleporter`, `apply_spinner`, `tick_screen_wobble`, `apply_anti_magic_zone`, `handle_teleport_request` (the LoadingPlugin one)
- **Helper:** `can_move_with_doors(floor, doors, pos, dir) -> bool` (private in `dungeon/mod.rs`); `floor_handle_for(assets, floor_number) -> &Handle<DungeonFloor>` (private in `dungeon/mod.rs`)
- **Schema additions:** `ItemAsset.key_id: Option<String>` (`#[serde(default)]`), `DungeonFloor.locked_doors: Vec<((u32,u32), Direction, String)>` (`#[serde(default)]`)

---

## Critical pitfalls (lifted from research, mirrored in plan)

1. **Pitfall 4 — `Door` is currently passable per `floor.can_move`.** The `can_move_with_doors` wrapper in Phase 6 closes this. Default `DoorState::Closed` (`unwrap_or_default()`) blocks `WallType::Door` walls. Verified by 2 unit tests in Phase 6.
2. **Pitfall 5 — Default `DoorState::Closed` blocks passage.** This IS the desired UX (D15 closed-by-default). `door_state_default_is_closed` test (Phase 4) confirms.
3. **Pitfall 7 — Saturating arithmetic on pit damage.** `derived.current_hp = derived.current_hp.saturating_sub(*damage)` (Phase 5 `apply_pit_trap`). Same pattern as `character.rs:374-381`. `pit_trap_subtracts_damage_saturating` test (Phase 4) verifies.
4. **Pitfall 8 — `populate_locked_doors` clear-first idempotence.** `OnEnter(Dungeon)` fires every time the player teleports cross-floor (Option α). Without `clear()`, `LockedDoors.by_edge` stacks duplicate entries. Mirrors `populate_item_handle_registry` at `inventory.rs:539`. `locked_doors_clear_idempotent` test (Phase 4) verifies.
5. **Pitfall 9 — `LockedDoor` `can_move` returns `false`.** The wrapper in Phase 6 must check `DoorStates` FIRST for door types; if `Open`, return `true` regardless of `floor.can_move`. Verified by 2 unit tests in Phase 6.
6. **Pitfall 11 — Spinner facing must reflect on minimap same frame.** Already automatic via Bevy schedule order: spinner runs in `Update`; minimap painter runs in `EguiPrimaryContextPass` (after `PostUpdate`). Verified by Layer-2 test `spinner_randomizes_facing` (Phase 5).
7. **Risk register #ScreenWobble vs MovementAnimation.** Both mutate `Transform::rotation`. `tick_screen_wobble` ordered `.after(animate_movement)` to win the last-write race.

---

## Next-step command shape (what the parent dispatches AFTER user approves the plan)

The parent should:

1. **Surface decisions D3, D10, D11, D14 to the user** with the recommended defaults (α / A / A / A respectively). D1, D2, D2b, D5, D6, D7, D9, D12, D13, D15 are recommended defaults — proceed unless user objects. D2c, D4, D8, D9b are auto-resolved.

2. **Once user OKs the plan** (and answers D3/D10/D11/D14), dispatch the implementer skill:

```
Skill(skill: "run-implementer",
      args: "Implement this plan: /Users/nousunio/Repos/Learnings/claude-code/druum/project/plans/20260506-100000-feature-13-cell-features.md.

The original task: Feature #13 Cell Features (Doors, Traps, Teleporters, Spinners) from the Druum dungeon-crawler roadmap.

User decisions during planning:
- D1 (door persistence): per-floor-instance only — RECOMMENDED accepted.
- D2 (KeyItem representation): key_id: Option<String> on ItemAsset — RECOMMENDED accepted.
- D2b (door_id location): DungeonFloor.locked_doors side-table — RECOMMENDED accepted.
- D3 (cross-floor teleport): <USER ANSWER — α | β | γ; α RECOMMENDED>
- D5 (encounter trap stub): publish EncounterRequested + log-only — RECOMMENDED accepted.
- D6 (screen wobble): rotation-jitter camera shake — RECOMMENDED accepted.
- D7 (SFX strategy): royalty-free .ogg files — RECOMMENDED accepted.
- D9 (door state shape): DoorStates: Resource(HashMap) — RECOMMENDED accepted.
- D10 (SfxKind variants): <USER ANSWER — A: +2 (recommended) | B: +5 | C: +1>
- D11 (floor_02 authoring): <USER ANSWER — A: author 80-line stub (recommended) | B: defer to manual | C: mock-only>
- D12 (poison stacking): naive push — RECOMMENDED accepted.
- D13 (key consumption): NOT consumed (Wizardry-style) — RECOMMENDED accepted.
- D14 (rand verification): <USER ANSWER — A: cargo tree -i rand at impl time (recommended) | B: Time::elapsed_secs_f64() fallback | C: rand = \"0.8\" direct dep>
- D15 (door default state): Closed-by-default; Interact to open — RECOMMENDED accepted.

Critical pre-implementation context:
- Working tree is currently clean on `gitbutler/workspace`, `zz` empty. Local main is even with origin/main per the prior pipeline state doc (Feature #12 PR merged via PR #12 — c0cefb3, d16aaf8, 2c8d79f, 10002eb, a0f88de all on main). NO `git pull` is required before branching for #13.
- Use GitButler (`but`) for all history-mutating ops; the `gitbutler/workspace` pre-commit hook blocks raw `git commit`. Read the project CLAUDE.md for the command mapping. Read-only `git log`/`git diff`/`git show` and `gh pr ...` are fine.
- Plan §'Frozen post-#12 / DO NOT TOUCH' enumerates files the implementer must NOT modify. Verify post-implementation with the `git diff --name-only` grep in the verification gate.
- D14 verification is BLOCKING at the start of Phase 5 — run `cargo tree -i rand` from project root BEFORE writing `apply_spinner`. If empty, fall back to D14-B.
- The Phase 6 D9b edit to `dungeon/mod.rs::handle_dungeon_input` is the LARGEST single edit to a previously-frozen module. The wrapper function `can_move_with_doors` is the gate between asset-level passability and runtime door state. Existing dungeon tests should still pass; default empty `DoorStates` produces `Closed` for door types — this differs from pre-#13 behavior and is intentional per Pitfall 5 + D15.
- Schema edits to `data/dungeon.rs` and `data/items.rs` are additive and `#[serde(default)]` — existing tests pass unchanged. Verify with `cargo test data::` immediately after Phase 1 commits.
- `EncounterRequested` and `TeleportRequested` MUST `#[derive(Message)]`, NOT `#[derive(Event)]`. Verification gate greps for `derive(.*\\bEvent\\b)` and `EventReader<` / `EventWriter<` in features.rs and tests/ — must return zero matches.
- `bevy::utils::HashMap` is REMOVED in 0.18 — use `std::collections::HashMap`. Verification gate greps src/ for `bevy::utils::HashMap` — must return zero matches.
- 9 phases; one commit per phase; total 9 commits (or fewer if interdependencies require bundling — see plan's per-phase Verification subsection).
")
```

3. **After the implementer reports complete**, the parent dispatches `/ship` to commit/push/PR (note: implementer commits incrementally per phase; `/ship` may be a no-op if everything is already pushed — adapt as needed), then `run-reviewer` against the resulting PR URL. Both stages are NOT IN SCOPE for this orchestrator run — they happen in a follow-up turn.

**No `git pull` is required** before the implementer branches. Local main is even with origin/main per the prior pipeline state and the `gitStatus` snapshot at session start (`c0cefb3 GitButler Workspace Commit` on top, `d16aaf8` PR #12 merge, `2c8d79f`/`10002eb`/`a0f88de` Feature #12 commits — all on main). Working tree clean. The implementer can branch directly via `but branch new feature/13-cell-features`.

---

## Follow-up items / deferred work

- **Feature #14 (Status Effects):** will own the `tick_status_durations` system that decrements `ActiveEffect.remaining_turns`. #13's `apply_poison_trap` pushes effects onto the Vec; #14 ticks them per turn.
- **Feature #15 (Combat):** will read `With<AntiMagicZone>` on `PlayerParty` to block spell-casting. #13 ships the marker; #15 reads it.
- **Feature #16 (Encounters):** will subscribe to `EncounterRequested { source: EncounterSource }` Message and transition to `GameState::Combat`. #13 publishes the message with `EncounterSource::AlarmTrap`; #16 adds `Random` + `Foe` source variants.
- **Feature #21 (Loot):** will use `ItemHandleRegistry::get("rusty_key")` to grant keys via `give_item`. #13 ships the `key_id` field that loot tables author into.
- **Feature #23 (Save/Load):** will need to persist `DoorStates` (or recompute from a per-floor saved-doors set), `LockedDoors`, `PendingTeleport` (transient — can be skipped), and the active floor number. #13's per-floor-instance lifetime model means door state is reset on every floor change; #23 may want to override.
- **Feature #25 (Inventory UI):** will own the locked-door key-selection UI (if multiple keys could open one door). #13 v1 uses automatic-key-lookup; #25 may add a "which key?" prompt for ambiguous cases.
- **Door visual swing animation** — deferred to #25 polish. v1 leaves the wall plate rendered; player notices door state via SFX.
- **Spinner with hidden "true facing" inversion** — ruled out by Resolved §4 (telegraphed UX is the locked design call).
- **`event_id` scripted events** — `CellFeatures.event_id` field exists but #13 v1 ignores it. Defer to #14+.
- **Trap detection by Luck stat** — defer to #14 / #21. v1 traps always trigger.
- **Hot-reload of `floor_*.dungeon.ron` / `core.items.ron`** — `AssetEvent<DungeonFloor>` + `AssetEvent<ItemAsset>` subscribers are post-v1 `--features dev` enhancements.

---

## Pipeline status

- [x] Step 1 — Research complete: `project/research/20260506-080000-feature-13-cell-features.md`
- [x] Step 2 — Plan complete (Status: Draft, awaiting user OK on D3/D10/D11/D14): `project/plans/20260506-100000-feature-13-cell-features.md`
- [ ] Step 3 — Implement (NOT IN SCOPE for this run; parent dispatches manually after user approval)
- [ ] Step 4 — Ship (NOT IN SCOPE)
- [ ] Step 5 — Code Review (NOT IN SCOPE)
- [x] Step 6 — Pipeline summary: this file
- [x] Step 7 — Pipeline state updated: `project/orchestrator/PIPELINE-STATE.md`

`PIPELINE-STATE.md` is now scoped to Feature #13 and will be updated by the parent on the next pipeline turn (when the implementer is dispatched).
