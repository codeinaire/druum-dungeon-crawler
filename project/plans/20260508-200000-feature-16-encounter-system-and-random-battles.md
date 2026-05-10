# Plan: Feature #16 — Encounter System & Random Battles

**Date:** 2026-05-08
**Status:** Complete
**Research:** `project/research/20260508-180000-feature-16-encounter-system.md`
**Roadmap:** `project/roadmaps/20260429-01-bevy-dungeon-crawler-roadmap.md` lines 862-911
**Predecessor (just shipped):** Feature #15 — `project/implemented/20260508-120000-feature-15-turn-based-combat-core.md`
**Depends on:** Feature #2 (`GameState`/`CombatPhase`), Feature #3 (`RonAssetPlugin`/`DungeonAssets`/`AudioAssets`), Feature #6 (`SfxKind::EncounterSting`), Feature #7 (`MovedEvent`/`PlayerParty`), Feature #13 (`EncounterRequested: Message`/`EncounterSource::AlarmTrap` precedent/`apply_alarm_trap` shape), Feature #15 (`CurrentEncounter` contract / `EnemyBundle` / `CombatRng` Box-dyn pattern / `?Sized` precedent / `random_range` rename / `dev-stub spawn_dev_encounter` to delete).

---

## Goal

Add the random-encounter trigger pipeline so that walking the dungeon rolls per-step encounter probabilities (with a soft-pity accumulator), and on a hit transitions `Dungeon → Combat` while populating `CurrentEncounter` with spawned enemies that #15's combat loop consumes. Author the first-floor encounter table (3-5 enemy groups), expose the encounter trigger as the single message-channel entry point that random rolls AND existing alarm-traps both feed, stub the `FoeProximity` resource for #22, and snap any in-flight movement tween on combat entry so the visual round-trip is clean. Defers FOE/visible-enemy work (#22), per-instance enemy authoring via `EnemyDb` (#17), encounter-sting flash polish (#25), and additional floor encounter tables.

---

## Approach

**Single sub-PR (~400-600 LOC, +1 file under combat/, +1 file under data/, +1 RON asset, +0 deps)** — the research recommends Option A (Message-pipe) which composes with the already-shipped `apply_alarm_trap` precedent. Two small systems do the work: `check_random_encounter` reads `MovedEvent`, rolls the soft-pity-scaled probability, and writes `EncounterRequested { source: Random }`; `handle_encounter_request` (the de facto `start_combat`) is the SOLE consumer that picks an `EnemyGroup` via `WeightedIndex`, spawns `EnemyBundle`s, populates `CurrentEncounter`, and transitions state. The same consumer reads alarm-trap-published messages — one observable seam, two producers.

**Architectural decisions locked from research, no user input required because all 7 open questions have HIGH-confidence researcher recommendations:**

- **D-A1 — Message-pipe (research recommended):** producer (`check_random_encounter`) and consumer (`handle_encounter_request`) are separate systems sharing the existing `EncounterRequested` channel. Composes with `apply_alarm_trap` (frozen by #13), gives FOE work (#22) a no-touch hook later.
- **D-A2 — Single-file `combat/encounter.rs`:** mirrors `combat/status_effects.rs` precedent. Owns `EncounterPlugin`, `EncounterState`, `EncounterRng`, `CurrentEncounter`, `FoeProximity` stub, `check_random_encounter`, `handle_encounter_request`, `reset_encounter_state_on_dungeon_entry`, `clear_current_encounter_on_combat_exit`, `snap_movement_animation_on_combat_entry`, and (under `feature = "dev"`) `force_encounter_on_f7`. Pure asset schema lives at `src/data/encounters.rs` (mirrors `src/data/dungeon.rs`).
- **D-A3 — `EncounterTable` is its own asset, not embedded in `DungeonFloor`:** matches the existing `DungeonFloor.encounter_table: String` indirection at `data/dungeon.rs:260`. Loaded via `RonAssetPlugin::<EncounterTable>::new(&["encounters.ron"])` registered next to the existing 5 in `loading/mod.rs:106-110`. Handle lives on `DungeonAssets`.
- **D-A4 — Inline `EnemySpec` for v1 (research recommended over ID-refs):** encounter table carries `BaseStats`/`DerivedStats`/`EnemyAi` directly per #15's `EnemyBundle` shape. When #17 ships `EnemyDb`, encounter tables migrate to `enemy_id: Option<String>` lookups (backward-compatible additive field).
- **D-A5 — Separate `EncounterRng` from `CombatRng`:** encounter rolls happen in `Dungeon` state where `CombatRng` may not yet be initialised (it re-seeds in `init_combat_state`). Same `Box<dyn rand::RngCore + Send + Sync>` shape, separate resource. Tests inject `ChaCha8Rng::seed_from_u64(...)` directly.
- **D-A6 — `pick_group` lives on `EncounterTable` in `data/encounters.rs`:** mirrors `DungeonFloor::can_move` precedent at `data/dungeon.rs:285-300`. Pure-data + pure-logic in the schema module; `encounter.rs` consumes via `table.pick_group(&mut *rng.0)`.
- **D-A7 — `encounter_table_for(assets, floor) → &Handle<EncounterTable>` lives in `loading/mod.rs`:** mirrors `floor_handle_for` at `dungeon/mod.rs:392-401`. v1 ships floor_01 only; future floors add match arms.
- **D-A8 — `pub(crate) fn` for the single-frame consumer pattern (`requests.read().next()`):** alarm-trap + same-step random roll collapse to one combat (rare but possible — e.g., a player on an alarm cell that also has `encounter_rate > 0`). Document explicitly with a comment.
- **D-A9 — `MovementAnimation` snap on `OnEnter(Combat)` (research recommended over fade):** ship the snap; mid-stride tween becomes instant on combat entry. Polish (encounter-sting flash transition that masks the snap) deferred to #25.

**Open-question resolutions (all 7 Category A — research had clear recommendations):**

- **D-X1 — Soft-pity reset on `OnEnter(Dungeon)` (research recommendation 1):** counter resets when re-entering Dungeon state. Catches both combat-return AND cross-floor teleport. Designer-predictable, simplest mental model.
- **D-X2 — Cap accumulator multiplier at 2.0 (research recommendation 2, Option A):** the formula becomes `cell.encounter_rate * (1.0 + steps_since_last as f32 * 0.05).min(2.0)`. No special-casing of rate-zero corridors; predictable upper bound.
- **D-X3 — Inline `EnemySpec` (research recommendation 3):** encoded in detail under D-A4 above. Migration path to ID-refs is additive.
- **D-X4 — Snap movement animation on combat entry (research recommendation 4):** D-A9 above; polish deferred.
- **D-X5 — Add `EncounterSource::Random` only (research recommendation 5):** no placeholder `Foe` variant; #22 adds its own when needed. Keeps the variant set minimal.
- **D-X6 — F7 keybind for `?force_encounter` (research recommendation 6):** adjacent to F9, low collision risk. Implemented as a `#[cfg(feature = "dev")]` direct-`ButtonInput<KeyCode>` reader (mirrors `state/mod.rs:71-89` F9 cycler) — does NOT touch the frozen leafwing `DungeonAction` enum.
- **D-X7 — `pick_group` on `EncounterTable` (research recommendation 7):** D-A6 above.

**The carve-out list (touched files outside `combat/encounter.rs` + `data/encounters.rs`):**

- `combat/mod.rs` — +2 lines: `pub mod encounter;` + `app.add_plugins(encounter::EncounterPlugin)`.
- `combat/turn_manager.rs` — DELETE the `#[cfg(feature = "dev")] spawn_dev_encounter` function (lines 671-716) and its registration (lines 181-186). Per #15 plan Pitfall 1 ("#16 deletes this stub").
- `loading/mod.rs` — +3 lines:
  1. Import `EncounterTable` from `crate::data`.
  2. Add `RonAssetPlugin::<EncounterTable>::new(&["encounters.ron"])` to the existing tuple at line 106-110.
  3. Add `#[asset(path = "encounters/floor_01.encounters.ron")] pub encounters_floor_01: Handle<EncounterTable>` field to `DungeonAssets`.
  4. Add `pub(crate) fn encounter_table_for(assets: &DungeonAssets, floor_number: u32) -> &Handle<EncounterTable>` (mirrors `floor_handle_for`).
- `dungeon/features.rs` — +1 line: add `Random` variant to the `EncounterSource` enum at line 130-134.
- `data/mod.rs` — +2 lines: `pub mod encounters;` declaration + re-export of `EncounterTable` (matches `pub use enemies::EnemyDb` precedent at line 21).
- `Cargo.toml` — UNCHANGED. `rand 0.9.4` already direct (line 33), `rand_chacha 0.9.0` already dev-dep (line 36), `bevy_common_assets` already direct (line 25). No new deps.

**Total scope:** +2 new files (`src/plugins/combat/encounter.rs` ~350 LOC, `src/data/encounters.rs` ~120 LOC), +1 new asset (`assets/encounters/floor_01.encounters.ron`), +5 carve-out edits (each tied to a single Step), +0 new deps. Test count delta: +6-10 (research envelope; we land at ~9).

---

## Critical

These constraints are non-negotiable. Violations should fail review.

- **Bevy `=0.18.1` pinned.** No version bump.
- **`#[derive(Message)]`, NOT `#[derive(Event)]`** for `EncounterRequested` (already declared at `features.rs:125-128` — DO NOT change). Read with `MessageReader<T>`, write with `MessageWriter<T>`. `app.add_message::<T>()` is the registration. Verification gate greps `combat/encounter.rs` and `data/encounters.rs` for `derive(Event)` / `EventReader<` / `EventWriter<` — must return ZERO matches.
- **`MovedEvent` is a `Message` not an `Event`** (registered by `DungeonPlugin` at `dungeon/mod.rs:224` via `app.add_message::<MovedEvent>()`). Test harnesses that load `EncounterPlugin` without `DungeonPlugin` MUST call `app.add_message::<crate::plugins::dungeon::MovedEvent>()` explicitly (Pitfall 1, mirrors #15 D-I16).
- **`handle_encounter_request` is the SOLE writer of `CurrentEncounter`.** No other system in `combat/` may `commands.insert_resource(CurrentEncounter { ... })` in production code. Tests may insert directly for fixture purposes. Verification gate greps `src/**/*.rs` (excluding `combat/encounter.rs` and `tests` modules) for `insert_resource(CurrentEncounter` — must return ZERO matches.
- **`CurrentEncounter` is removed on `OnExit(Combat)`** (Pitfall 6). Without removal, the previous combat's enemy entity references survive into the next dungeon-step and break tests that assert "no encounter active in Dungeon state". Verification: app test asserts `world.get_resource::<CurrentEncounter>().is_none()` after combat exit.
- **`?Sized` discipline on `pick_group(&self, rng: &mut (impl rand::Rng + ?Sized))`.** Locked-in by #15 D-I13 (`damage.rs:67`, `targeting.rs:37`). The `EncounterRng(Box<dyn rand::RngCore + Send + Sync>)` is a DST behind `&mut`; without `?Sized` the bound fails to satisfy. Verification: `cargo check` on a snippet that calls `table.pick_group(&mut *rng.0)`.
- **`rand 0.9` API surface** (locked by #15 D-I12/D-I14):
  - `rng.random::<f32>()` — NOT `rng.gen::<f32>()`.
  - `rand::distr::weighted::WeightedIndex` — NOT `rand::distributions::WeightedIndex`.
  - `SeedableRng::from_os_rng()` requires the `os_rng` feature flag, ALREADY enabled at `Cargo.toml:33`.
- **Cursor draining on early returns from `check_random_encounter`** (Pitfall 4, defensive). When `dungeon_assets.is_none()` or the floor handle hasn't loaded, drain the `MessageReader` cursor with `for _ in moved.read() {}` before returning. Mirrors `audio/sfx.rs:73-78`.
- **System ordering:** `check_random_encounter.after(handle_dungeon_input)` and `handle_encounter_request.after(check_random_encounter)` (Pitfall 5). Same shape as `apply_alarm_trap.after(handle_dungeon_input)` at `features.rs:172-174`. Both gated `.run_if(in_state(GameState::Dungeon))`.
- **`spawn_dev_encounter` MUST be deleted** (Pitfall 8). The function at `turn_manager.rs:671-716` and its registration at `:181-186` cannot coexist with #16's encounter spawner under `feature = "dev"` — they would double-spawn enemies. The verification regression test asserts that after F7 force-encounter, the spawned enemy count matches the picked group exactly (no stray "Goblin 1"/"Goblin 2").
- **Trust-boundary clamps on RON-deserialized values** (Security §Architectural Risks):
  - `cell.encounter_rate.clamp(0.0, 1.0)` on read in `check_random_encounter` (defends against typos like `1.5`).
  - `MAX_ENEMIES_PER_ENCOUNTER = 8` constant; `handle_encounter_request` truncates oversized groups with a `warn!` (defends against malicious save/asset).
  - Weights pre-clamped to `1..=10000` in `pick_group` (defends against overflow / DoS via gigantic weights).
- **F7 force-encounter does NOT touch `input/mod.rs`** (frozen by #5). Use direct `Res<ButtonInput<KeyCode>>` reader gated `#[cfg(feature = "dev")]`, mirroring the F9 precedent at `state/mod.rs:71-89`.
- **`EncounterSource::Random` is APPENDED to the enum at `features.rs:130-134`.** Existing `AlarmTrap` arm is untouched. `match` sites that pattern on `EncounterSource` must add the `Random` arm — search and add as needed (currently only `apply_alarm_trap` matches by writing the enum, not by reading; the only reader that pattern-matches is the new `handle_encounter_request`, which sees both arms by design).
- **Pre-commit hook on `gitbutler/workspace`** rejects raw `git commit` (CLAUDE.md). Implementer commits via `but commit --message-file <path>` per logical step.

---

## Frozen / DO NOT TOUCH

These files are frozen by Features #1-#15 and may NOT be modified by the #16 implementer except for the explicit carve-outs listed in §Approach.

- **`src/main.rs`** — FROZEN. `EncounterPlugin` is registered as a sub-plugin INSIDE `CombatPlugin::build` (mirrors `TurnManagerPlugin`/`EnemyAiPlugin`/`CombatUiPlugin` precedent at `combat/mod.rs:31-33`). `main.rs` is unchanged.
- **`src/plugins/state/mod.rs`** — FROZEN by #2. `GameState` and `CombatPhase` are read-only.
- **`src/plugins/input/mod.rs`** — FROZEN by #5. `DungeonAction` enum stays at its current variants. F7 force-encounter goes through `Res<ButtonInput<KeyCode>>` directly under `cfg(feature = "dev")`.
- **`src/plugins/audio/{mod,bgm,sfx}.rs`** — FROZEN by #6. `SfxKind::EncounterSting` is wired (`sfx.rs:54`). `apply_alarm_trap` already emits it (`features.rs:483-485`); `handle_encounter_request` emits it for `Random` source the same way.
- **`src/plugins/dungeon/mod.rs`** — FROZEN by #7-#9. `MovedEvent`, `ActiveFloorNumber`, `floor_handle_for`, `PlayerParty`, `MovementAnimation`, the `Dungeon → Combat → Dungeon` party preservation rule (`despawn_dungeon_entities` at `:593-619`, `cleanup_party_after_combat` at `:572-591`, `spawn_party_and_camera` idempotence at `:454-461`) — all read-only contracts.
- **`src/plugins/dungeon/features.rs`** — FROZEN APART FROM the single-line append of `Random` to `EncounterSource` (line 130-134). `EncounterRequested: Message` (line 125-128), `apply_alarm_trap` (line 459-487), the `MovedEvent` consumer pattern (line 366-456) — all read-only.
- **`src/plugins/loading/mod.rs`** — FROZEN APART FROM the 3-line additive carve-out documented in §Approach. The `LoadingPlugin::build` ordering is brittle (research note at line 97-104 — RON loaders MUST be registered before `add_loading_state`); the new `RonAssetPlugin::<EncounterTable>` registration goes inside the existing `add_plugins(...)` tuple, NOT as a separate `add_plugins(...)` call.
- **`src/plugins/combat/mod.rs`** — FROZEN APART FROM the +2 line carve-out (module declaration + plugin registration).
- **`src/plugins/combat/{actions,ai,combat_log,damage,enemy,status_effects,targeting,turn_manager,ui_combat}.rs`** — FROZEN APART FROM the deletion of `spawn_dev_encounter` from `turn_manager.rs`. NO other edits to combat sibling files in #16.
- **`src/plugins/party/{character,inventory,mod}.rs`** — FROZEN since #14. `BaseStats`, `DerivedStats`, `Equipment::default()`, `Experience::default()` are imported by name; the `recompute_derived_stats_on_equipment_change` D-A5 carve-out from #15 is preserved unchanged.
- **`src/data/{classes,dungeon,enemies,items,spells}.rs`** — FROZEN. `DungeonFloor.encounter_table: String` at `data/dungeon.rs:260` is the indirection lookup key (read-only); no schema edits to existing data files.
- **`src/data/mod.rs`** — FROZEN APART FROM the `pub mod encounters;` + re-export carve-out.
- **`assets/dungeons/floor_01.dungeon.ron`** — FROZEN. The existing `encounter_table: "b1f_encounters"` field already references the new table; no edit needed. (If the field is missing on inspection, add the line — but the file should already have it from #4.)
- **`Cargo.toml`** — FROZEN. `rand 0.9.4` direct dep (line 33), `rand_chacha 0.9.0` dev-dep (line 36), `bevy_common_assets 0.16.0` (line 25), `bevy_asset_loader 0.26.0` (line 26) — all already declared. NO edits.

---

## Decisions

### Architecture decisions (locked by planner from research §Architecture Options + §Patterns)

**D-A1 — Pipeline shape: Message-pipe (research recommendation Option A).**
- Options: A=Message-pipe (separate roll-system + consumer-system sharing `EncounterRequested`), B=Direct state transition (one inline system), C=Inline-in-movement-handler (roll inside `handle_dungeon_input`).
- Resolved: **A.** Composes with the already-shipped `apply_alarm_trap` precedent at `features.rs:480-482` (which writes the same `EncounterRequested`); FOE work in #22 adds a third producer to the same channel without touching the consumer; tests can independently verify "step → message written" and "message → state transition + spawn"; the consumer is the SOLE owner of `CurrentEncounter` and the `Dungeon → Combat` transition trigger.
- Rationale (research §Counterarguments 1-3): the extra ~30 LOC consumer is the testability investment that pays off on the first FOE encounter source addition.

**D-A2 — Files: single `combat/encounter.rs` + `data/encounters.rs`.**
- Options: A=single combat file with all systems, B=split per-system (`encounter_check.rs` + `encounter_resolver.rs`).
- Resolved: **A.** Mirrors `combat/status_effects.rs` precedent (~400 LOC, all systems + plugin in one file). Splitting is `#15`-style multi-phase work, not warranted at #16's LOC budget.

**D-A3 — `EncounterTable` is its own `Asset` type.**
- Options: A=standalone `*.encounters.ron` asset (research recommendation), B=embed `EnemyGroup` list inside `DungeonFloor`, C=runtime-built from a code-only registry.
- Resolved: **A.** `DungeonFloor.encounter_table: String` indirection (`data/dungeon.rs:260`) is already in place; embedding would couple encounter authoring to dungeon-floor authoring. Standalone tables can be reused across floors of the same biome.
- Rationale: keeps the `RonAssetPlugin` count uniform (5 → 6) and the `DungeonAssets` collection symmetric.

**D-A4 — Inline `EnemySpec { name, base_stats, derived_stats, ai }` in v1 (research recommendation 3).**
- Options: A=inline (defer ID-refs to #17), B=block on #17 first, C=hybrid `enemy_id: Option<String>` field.
- Resolved: **A.** `EnemyDb` is currently an empty stub (`data/enemies.rs:8-11`); #17 fills it. Inline `EnemySpec` mirrors the `spawn_dev_encounter` shape #15 used for placeholders. Migration path: when #17 ships `EnemyDb`, add `enemy_id: Option<String>` to `EnemySpec` (additive), with the resolver falling back to inline values when `enemy_id == None`.
- Rationale: ships #16 without blocking on #17; backward-compatible.

**D-A5 — `EncounterRng` is a separate `Resource` from `CombatRng`.**
- Options: A=share `CombatRng`, B=separate `EncounterRng`, C=top-level `rand::thread_rng()`.
- Resolved: **B.** `CombatRng` is re-seeded inside `init_combat_state` (`turn_manager.rs:200-211`) which only runs `OnEnter(GameState::Combat)`. Encounter rolls happen during `GameState::Dungeon` where `CombatRng` may exist but has stale or default state. Separate resource avoids coupling #16's testability to combat-state init order.
- Rationale: same `Box<dyn rand::RngCore + Send + Sync>` shape as `CombatRng`; tests inject `ChaCha8Rng::seed_from_u64(...)`; production seeds `SmallRng::from_os_rng()` once via `Default::default()`.

**D-A6 — `pick_group(&self, rng) -> Option<&EnemyGroup>` is a method on `EncounterTable`.**
- Options: A=method on `EncounterTable` in `data/encounters.rs` (research recommendation), B=free function in `encounter.rs`.
- Resolved: **A.** Mirrors `DungeonFloor::can_move` at `data/dungeon.rs:285-300` — pure-data + pure-logic in the schema module.
- Rationale: testable in `data/encounters.rs::tests` without spinning up a `bevy::App`; `encounter.rs` consumes via one line `let Some(group) = table.pick_group(&mut *rng.0) else { ... };`.

**D-A7 — `encounter_table_for(assets, floor_number) → &Handle<EncounterTable>` lives in `loading/mod.rs`.**
- Options: A=`loading/mod.rs` (mirrors `floor_handle_for`), B=`encounter.rs` (mirrors the `pick_enemy_group` co-location), C=on `DungeonAssets` impl.
- Resolved: **A.** Mirrors the `floor_handle_for` precedent at `dungeon/mod.rs:392-401` — asset-handle resolution lives next to the asset declaration. The function is `pub(crate)` so `encounter.rs` calls it directly. v1 ships floor_01 only; v2+ adds match arms here.
- Rationale: keeps the asset-resolution layer co-located with the asset declarations; mechanical pattern for #17/#22.

**D-A8 — Same-frame multi-encounter collapse: consumer takes `requests.read().next()` only.**
- Resolved: alarm-trap + same-step random roll on the same cell could write two `EncounterRequested` per frame. The consumer takes the first and discards the rest — one combat per step, never two stacked. Document explicitly with a comment.

**D-A9 — `MovementAnimation` snap on `OnEnter(Combat)` (research recommendation 4).**
- Options: A=snap to completion (research recommended), B=fade transition, C=preserve mid-stride and resume on combat-exit.
- Resolved: **A.** Without snap, a 50% tween freezes during combat and resumes from 50% on return → visually correct but feels like a "jump" if combat lasted multiple seconds. Polish (encounter-sting flash that masks the snap) deferred to #25.
- Rationale: simplest implementation; Wizardry/Etrian convention is instant transition.

### Open-question resolutions (research §Open Questions — all 7 Cat-A; researcher had clear recommendations)

**D-X1 — Soft-pity counter resets on every `OnEnter(Dungeon)` (research recommendation 1).**
- Options: A=reset on `OnEnter(Dungeon)`, B=preserve across combats, C=reset only on Town visit.
- Resolved: **A.** Catches both combat-return AND cross-floor teleport (Pitfall 3). Designer-predictable: each "dungeon entry" is a clean slate. Combat itself is the tension release; the next-step's roll starts fresh at base rate.

**D-X2 — Cap accumulator multiplier at 2.0 (research recommendation 2, Option A from Pitfall 7).**
- Options: A=cap at 2.0, B=skip bump on rate-zero cells, C=reset counter on rate-zero cells.
- Resolved: **A.** `cell.encounter_rate * (1.0 + steps_since_last as f32 * 0.05).min(2.0)`. Predictable upper bound; no special-casing of designer-authored rate-zero corridors. After 20 missed steps the multiplier saturates at 2.0; after 40 it's still 2.0. Tunable via the `0.05` step bonus and the `2.0` cap.
- Rationale: Pitfall 7 of research — a 100-cell rate-zero corridor followed by a `0.01` cell would otherwise yield `(1.0 + 100*0.05) * 0.01 = 0.06` per step (acceptable) but the researcher's concern about unbounded growth still motivates the cap.

**D-X3 — Inline `EnemySpec` in v1 (research recommendation 3).** Resolved as D-A4 above.

**D-X4 — Snap movement animation on combat entry (research recommendation 4).** Resolved as D-A9 above.

**D-X5 — `EncounterSource::Random` only; no placeholder `Foe` variant (research recommendation 5).**
- Options: A=`Random` only, B=`Random` + placeholder `Foe`, C=`Random` + placeholder `Foe { boss: bool }`.
- Resolved: **A.** #22 adds its own variant when there's a real producer. The `match` in `handle_encounter_request` becomes exhaustive over `Random | AlarmTrap` for now (rather than non-exhaustive for an unused `Foe`). When #22 lands, it adds the variant AND the corresponding `match` arm in one PR.
- Rationale: minimal variant set until there's a real consumer.

**D-X6 — F7 keybind for `?force_encounter` (research recommendation 6).**
- Options: A=F7, B=F8, C=Numpad+, D=Backtick.
- Resolved: **A.** F7 is adjacent to F9 (state-debug cycler at `state/mod.rs:71-89`), low collision risk with browser/IDE/terminal shortcuts, and consistent with the F-key dev-debug convention in this project.
- Implementation: direct `Res<ButtonInput<KeyCode>>` reader gated `#[cfg(feature = "dev")]`, NOT a leafwing action. `input/mod.rs` is FROZEN.

**D-X7 — `pick_group` on `EncounterTable` (research recommendation 7).** Resolved as D-A6 above.

### Execution-time decisions (left to the implementer to record as `D-I#` discoveries)

**D-X8 — `RonAssetPlugin::<EncounterTable>::new(&["encounters.ron"])` extension key.**
- v1 ships one file per floor: `floor_01.encounters.ron`, naming convention matches `floor_01.dungeon.ron`. RON loader extension is `"encounters.ron"` (multi-dot extension WITHOUT leading dot — research §Pitfall 4 of #3 plan; mirrors `&["dungeon.ron"]` at `loading/mod.rs:106`).
- If the implementer hits a problem with the multi-dot extension being ambiguous (e.g., `RonAssetPlugin::<DungeonFloor>` and `RonAssetPlugin::<EncounterTable>` both registering under `*.ron` — they don't, but defensive note), record as `D-I#` discovery.

**D-X9 — `DungeonFloor.encounter_table: String` lookup contract.**
- The `String` field at `data/dungeon.rs:260` is the table id (e.g., `"b1f_encounters"`). v1 short-circuits this lookup: `encounter_table_for` reads `floor_number` directly and returns the matching `Handle<EncounterTable>`, ignoring the string. v2 (when multi-floor encounter tables share by id) wires the string to a registry. Keep the indirection for forward-compat; `EncounterTable.id: String` is authored to match.
- If the implementer wants to wire the string-based lookup in v1, that's acceptable — record as `D-I#` discovery.

**D-X10 — Test seed for convergence assertions.**
- The plan recommends `ChaCha8Rng::seed_from_u64(42)` (mirrors #15 D-I12-context test seed). For convergence tests the exact trigger count at seed 42 is captured on the first successful run; subsequent runs assert the same count.
- If seed 42 produces an awkward count (e.g., suspicious zero or boundary), pick another (e.g., 1, 99) and document as `D-I#`.

---

## Steps

The implementation proceeds in **logical order** (configuration → asset schema → asset content → carve-outs → core systems → integration). Each step is independently committable via `but commit --message-file <path>`; the implementer commits after each verification block passes.

---

### Step 1 — Append `Random` variant to `EncounterSource` enum

- [x] Edit `src/plugins/dungeon/features.rs`. At the enum declaration (currently lines 130-134):
  ```rust
  #[derive(Clone, Copy, Debug, PartialEq, Eq)]
  pub enum EncounterSource {
      AlarmTrap,
      // Future: Random (foe roll), Foe (overworld encounter) — surface in #16.
  }
  ```
  Replace with:
  ```rust
  #[derive(Clone, Copy, Debug, PartialEq, Eq)]
  pub enum EncounterSource {
      AlarmTrap,
      /// Random roll triggered by `check_random_encounter` per `MovedEvent`.
      Random,
      // Future: Foe (overworld encounter) — surface in #22.
  }
  ```
- [x] **Verification:**
  - `cargo check` — succeeds. The compiler will flag any non-exhaustive `match` on `EncounterSource` — there should be ZERO such matches in the current tree (only producer in #13 writes the enum, no consumer yet).
  - `cargo check --features dev` — succeeds.
  - `rg 'EncounterSource::' src/` — confirms the `Random` variant appears only in `features.rs` after this step. The `apply_alarm_trap` write at `features.rs:480-482` is unchanged.
- [x] **Commit message:** `feat(combat): add EncounterSource::Random variant for #16`

---

### Step 2 — Create `src/data/encounters.rs` schema module

- [x] Create new file `src/data/encounters.rs`. Add file-level doc-comment:
  ```rust
  //! Encounter table asset schema — Feature #16.
  //!
  //! `EncounterTable` is loaded as an `Asset` via `bevy_common_assets::RonAssetPlugin`
  //! (registered in `loading/mod.rs`). Each floor references its table by handle on
  //! `DungeonAssets`; lookup is via `loading::encounter_table_for(&assets, floor_number)`.
  //!
  //! ## Inline EnemySpec (D-A4)
  //!
  //! Until #17 ships `EnemyDb`, encounter tables carry full `BaseStats`/`DerivedStats`/
  //! `EnemyAi` inline. Migration path: add `enemy_id: Option<String>` to `EnemySpec`
  //! (additive); resolver falls back to inline when `None`.
  //!
  //! ## RON extension (D-X8)
  //!
  //! Files use the multi-dot extension `*.encounters.ron`. The RON loader is registered
  //! via `RonAssetPlugin::<EncounterTable>::new(&["encounters.ron"])` in `loading/mod.rs`
  //! (without a leading dot — research §Pitfall 4 of #3 plan).
  //!
  //! ## `Reflect` derives (project_druum_minimap.md precedent)
  //!
  //! `#[derive(Reflect)]` handles `Vec<T>` and `Option<T>` shapes for typical asset
  //! types in Bevy 0.18; no `#[reflect(...)]` attributes needed.
  ```
- [x] Add imports:
  ```rust
  use bevy::prelude::*;
  use serde::{Deserialize, Serialize};

  use crate::plugins::combat::ai::EnemyAi;
  use crate::plugins::party::character::{BaseStats, DerivedStats};
  ```
- [x] Define `EnemySpec`:
  ```rust
  /// Inline enemy spec for #16. Fields mirror `EnemyBundle` (`combat/enemy.rs:39-51`).
  ///
  /// Until #17 ships `EnemyDb`, encounter tables carry full enemy stats inline.
  #[derive(Reflect, Serialize, Deserialize, Debug, Clone)]
  pub struct EnemySpec {
      pub name: String,
      pub base_stats: BaseStats,
      pub derived_stats: DerivedStats,
      /// Defaults to `EnemyAi::RandomAttack` (D-Q5=A from #15).
      #[serde(default)]
      pub ai: EnemyAi,
  }
  ```
- [x] Define `EnemyGroup`:
  ```rust
  /// A group of enemies spawned together for one encounter.
  ///
  /// `enemies.len()` is clamped to `MAX_ENEMIES_PER_ENCOUNTER` (8) by the
  /// consumer; oversized groups are truncated with a `warn!`.
  #[derive(Reflect, Serialize, Deserialize, Debug, Clone)]
  pub struct EnemyGroup {
      pub enemies: Vec<EnemySpec>,
  }
  ```
- [x] Define `EncounterEntry`:
  ```rust
  /// One entry in an encounter table — a weight + enemy group.
  ///
  /// Weight is `u32` (not `f32`) for byte-stable RON round-trips and to satisfy
  /// `WeightedIndex::new`'s integer-summable requirement.
  #[derive(Reflect, Serialize, Deserialize, Debug, Clone)]
  pub struct EncounterEntry {
      pub weight: u32,
      pub group: EnemyGroup,
  }
  ```
- [x] Define `EncounterTable`:
  ```rust
  /// One floor's encounter table. Loaded by `RonAssetPlugin::<EncounterTable>`.
  #[derive(Asset, Reflect, Serialize, Deserialize, Default, Debug, Clone)]
  pub struct EncounterTable {
      /// Identifier — matches `DungeonFloor.encounter_table` (`data/dungeon.rs:260`).
      pub id: String,
      pub entries: Vec<EncounterEntry>,
  }
  ```
- [x] Define `pick_group` method on `EncounterTable` (D-A6):
  ```rust
  impl EncounterTable {
      /// Pick a weighted-random `EnemyGroup` from the table.
      ///
      /// Returns `None` if the table is empty or all weights are zero.
      ///
      /// `?Sized` permits passing `&mut *rng.0` from a `Box<dyn RngCore + Send + Sync>`
      /// (locked by #15 D-I13).
      pub fn pick_group<'a>(
          &'a self,
          rng: &mut (impl rand::Rng + ?Sized),
      ) -> Option<&'a EnemyGroup> {
          if self.entries.is_empty() {
              return None;
          }
          // Weights are clamped to a sane range to defuse malicious or typo'd
          // RON values (Security trust boundary).
          let weights = self
              .entries
              .iter()
              .map(|e| e.weight.clamp(1, 10_000));
          // rand 0.9: WeightedIndex moved to rand::distr::weighted (was rand::distributions).
          let dist = rand::distr::weighted::WeightedIndex::new(weights).ok()?;
          use rand::prelude::Distribution;
          let idx = dist.sample(rng);
          Some(&self.entries[idx].group)
      }
  }
  ```
- [x] Add `mod tests` (Layer 1 — pure):
  ```rust
  #[cfg(test)]
  mod tests {
      use super::*;
      use rand::SeedableRng;

      fn mk_spec(name: &str, hp: u32) -> EnemySpec {
          EnemySpec {
              name: name.into(),
              base_stats: BaseStats::default(),
              derived_stats: DerivedStats {
                  current_hp: hp,
                  max_hp: hp,
                  ..Default::default()
              },
              ai: EnemyAi::default(),
          }
      }

      #[test]
      fn pick_group_returns_none_on_empty_table() {
          let table = EncounterTable::default();
          let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(42);
          assert!(table.pick_group(&mut rng).is_none());
      }

      #[test]
      fn pick_group_returns_only_entry_when_single() {
          let table = EncounterTable {
              id: "test".into(),
              entries: vec![EncounterEntry {
                  weight: 50,
                  group: EnemyGroup {
                      enemies: vec![mk_spec("Goblin", 30)],
                  },
              }],
          };
          let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(42);
          let group = table.pick_group(&mut rng).expect("table non-empty");
          assert_eq!(group.enemies[0].name, "Goblin");
      }

      #[test]
      fn pick_group_proportions_match_weights_with_seed() {
          // 50/30/15/5 weighted table; sample 10000 times with seeded RNG;
          // assert empirical proportions are within ±5% of expected.
          let table = EncounterTable {
              id: "test".into(),
              entries: vec![
                  EncounterEntry { weight: 50, group: EnemyGroup { enemies: vec![mk_spec("A", 1)] } },
                  EncounterEntry { weight: 30, group: EnemyGroup { enemies: vec![mk_spec("B", 1)] } },
                  EncounterEntry { weight: 15, group: EnemyGroup { enemies: vec![mk_spec("C", 1)] } },
                  EncounterEntry { weight: 5,  group: EnemyGroup { enemies: vec![mk_spec("D", 1)] } },
              ],
          };
          let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(42);
          let mut counts = [0u32; 4];
          for _ in 0..10_000 {
              let group = table.pick_group(&mut rng).unwrap();
              let idx = ["A", "B", "C", "D"]
                  .iter()
                  .position(|&s| s == group.enemies[0].name)
                  .unwrap();
              counts[idx] += 1;
          }
          // Expected proportions: 50%, 30%, 15%, 5%. Tolerance ±5% (500 samples)
          // because seed 42 is deterministic but the bounds give wiggle room.
          assert!((4500..=5500).contains(&counts[0]), "A count out of range: {}", counts[0]);
          assert!((2500..=3500).contains(&counts[1]), "B count out of range: {}", counts[1]);
          assert!((1000..=2000).contains(&counts[2]), "C count out of range: {}", counts[2]);
          assert!((0..=1000).contains(&counts[3]),    "D count out of range: {}", counts[3]);
      }

      #[test]
      fn encounter_table_round_trips_via_ron() {
          let table = EncounterTable {
              id: "b1f_test".into(),
              entries: vec![EncounterEntry {
                  weight: 1,
                  group: EnemyGroup {
                      enemies: vec![mk_spec("Goblin", 30)],
                  },
              }],
          };
          let serialized = ron::ser::to_string(&table).expect("serialize");
          let deserialized: EncounterTable = ron::de::from_str(&serialized).expect("deserialize");
          assert_eq!(deserialized.id, table.id);
          assert_eq!(deserialized.entries.len(), 1);
          assert_eq!(deserialized.entries[0].group.enemies[0].name, "Goblin");
      }
  }
  ```
- [x] **Verification:**
  - `cargo check` — succeeds (the file is not yet wired into `data/mod.rs` so module declarations there must come next; this step deliberately leaves `cargo build` failing until Step 3 lands).
  - Mark this step as complete only AFTER Step 3 lands; commit them together.

---

### Step 3 — Wire `data/encounters.rs` into `data/mod.rs`

- [x] Edit `src/data/mod.rs`. Add `pub mod encounters;` to the module declarations:
  ```rust
  pub mod classes;
  pub mod dungeon;
  pub mod enemies;
  pub mod encounters;   // <-- NEW
  pub mod items;
  pub mod spells;
  ```
- [x] Add the re-export below the existing ones (matches the `pub use enemies::EnemyDb` precedent at line 21):
  ```rust
  pub use encounters::{EncounterEntry, EncounterTable, EnemyGroup, EnemySpec};
  ```
- [x] **Verification:**
  - `cargo check` — succeeds.
  - `cargo check --features dev` — succeeds.
  - `cargo test --lib data::encounters::tests` — all 4 tests pass.
  - `cargo clippy --all-targets -- -D warnings` — clean.
- [x] **Commit message:** `feat(data): EncounterTable schema (#16 step 1)`

---

### Step 4 — Author `assets/encounters/floor_01.encounters.ron`

- [x] Create directory `assets/encounters/` if it doesn't exist.
- [x] Create file `assets/encounters/floor_01.encounters.ron` with 4 enemy groups (matches roadmap envelope of 3-5):
  ```ron
  // Encounter table for floor 1. Weights are u32; sum to 100 for designer
  // ergonomics. WeightedIndex normalises automatically.
  //
  // Inline EnemySpec until #17 ships EnemyDb (D-A4).

  (
      id: "b1f_encounters",
      entries: [
          // Common: Single Goblin (50%)
          (
              weight: 50,
              group: (
                  enemies: [
                      (
                          name: "Goblin",
                          base_stats: (strength: 8, intelligence: 4, piety: 4, vitality: 8, agility: 6, luck: 4),
                          derived_stats: (
                              max_hp: 30, current_hp: 30, max_mp: 0, current_mp: 0,
                              attack: 8, defense: 5, magic_attack: 0, magic_defense: 2,
                              speed: 6, accuracy: 60, evasion: 5,
                          ),
                          ai: RandomAttack,
                      ),
                  ],
              ),
          ),
          // Uncommon: Pair of Goblins (30%)
          (
              weight: 30,
              group: (
                  enemies: [
                      (
                          name: "Goblin",
                          base_stats: (strength: 8, intelligence: 4, piety: 4, vitality: 8, agility: 6, luck: 4),
                          derived_stats: (
                              max_hp: 30, current_hp: 30, max_mp: 0, current_mp: 0,
                              attack: 8, defense: 5, magic_attack: 0, magic_defense: 2,
                              speed: 6, accuracy: 60, evasion: 5,
                          ),
                          ai: RandomAttack,
                      ),
                      (
                          name: "Goblin",
                          base_stats: (strength: 8, intelligence: 4, piety: 4, vitality: 8, agility: 6, luck: 4),
                          derived_stats: (
                              max_hp: 30, current_hp: 30, max_mp: 0, current_mp: 0,
                              attack: 8, defense: 5, magic_attack: 0, magic_defense: 2,
                              speed: 6, accuracy: 60, evasion: 5,
                          ),
                          ai: RandomAttack,
                      ),
                  ],
              ),
          ),
          // Rare: Goblin Captain (15%) — Boss-AI variant for #15 BossFocusWeakest hook
          (
              weight: 15,
              group: (
                  enemies: [
                      (
                          name: "Goblin Captain",
                          base_stats: (strength: 12, intelligence: 4, piety: 4, vitality: 12, agility: 6, luck: 4),
                          derived_stats: (
                              max_hp: 60, current_hp: 60, max_mp: 0, current_mp: 0,
                              attack: 12, defense: 8, magic_attack: 0, magic_defense: 2,
                              speed: 6, accuracy: 70, evasion: 5,
                          ),
                          ai: BossFocusWeakest,
                      ),
                  ],
              ),
          ),
          // Very rare: Cave Spider (5%) — fast, low-HP
          (
              weight: 5,
              group: (
                  enemies: [
                      (
                          name: "Cave Spider",
                          base_stats: (strength: 6, intelligence: 2, piety: 2, vitality: 4, agility: 12, luck: 6),
                          derived_stats: (
                              max_hp: 18, current_hp: 18, max_mp: 0, current_mp: 0,
                              attack: 10, defense: 3, magic_attack: 0, magic_defense: 1,
                              speed: 12, accuracy: 75, evasion: 15,
                          ),
                          ai: RandomAttack,
                      ),
                  ],
              ),
          ),
      ],
  )
  ```
- [x] **Verification:**
  - The file is loaded by `bevy_common_assets::RonAssetPlugin` only after Step 5 lands the registration. For now, manually validate parsing:
    ```bash
    cargo test --lib data::encounters::tests::encounter_table_round_trips_via_ron
    ```
    is independent of this file but confirms the schema is RON-compatible.
  - Add a one-time test in `data/encounters.rs::tests` (extend mod tests added in Step 2) that loads the on-disk file via `ron::de::from_str`:
    ```rust
    #[test]
    fn floor_01_encounters_ron_parses() {
        let raw = std::fs::read_to_string("assets/encounters/floor_01.encounters.ron")
            .expect("floor_01.encounters.ron exists");
        let table: EncounterTable = ron::de::from_str(&raw).expect("parses cleanly");
        assert_eq!(table.id, "b1f_encounters");
        assert_eq!(table.entries.len(), 4);
        // Sanity: weights sum to 100 (designer convention).
        let sum: u32 = table.entries.iter().map(|e| e.weight).sum();
        assert_eq!(sum, 100);
    }
    ```
  - `cargo test --lib data::encounters::tests::floor_01_encounters_ron_parses` — passes.

---

### Step 5 — Carve-out: register `EncounterTable` loader in `LoadingPlugin`

- [x] Edit `src/plugins/loading/mod.rs`. Add `EncounterTable` to the imports (line 16):
  ```rust
  use crate::data::{ClassTable, DungeonFloor, EncounterTable, EnemyDb, ItemDb, SpellTable};
  ```
- [x] Add the `RonAssetPlugin::<EncounterTable>` registration to the existing `add_plugins(...)` tuple at line 105-110. The order must remain: registrations BEFORE `add_loading_state` (research §Pitfall 4 of #3 plan; comment at lines 97-104 spells this out). Insert as the LAST entry to avoid touching prior lines:
  ```rust
  .add_plugins((
      RonAssetPlugin::<DungeonFloor>::new(&["dungeon.ron"]),
      RonAssetPlugin::<ItemDb>::new(&["items.ron"]),
      RonAssetPlugin::<EnemyDb>::new(&["enemies.ron"]),
      RonAssetPlugin::<ClassTable>::new(&["classes.ron"]),
      RonAssetPlugin::<SpellTable>::new(&["spells.ron"]),
      RonAssetPlugin::<EncounterTable>::new(&["encounters.ron"]),  // <-- NEW (#16)
  ))
  ```
- [x] Add the `Handle<EncounterTable>` field to `DungeonAssets` (currently lines 30-45). Insert AFTER the `floor_02` field to keep dungeon-related fields grouped:
  ```rust
  #[derive(AssetCollection, Resource)]
  pub struct DungeonAssets {
      #[asset(path = "dungeons/floor_01.dungeon.ron")]
      pub floor_01: Handle<DungeonFloor>,
      // Feature #13 — minimal floor for cross-floor teleport testing (D11-A):
      #[asset(path = "dungeons/floor_02.dungeon.ron")]
      pub floor_02: Handle<DungeonFloor>,
      // Feature #16 — encounter table for floor 1.
      #[asset(path = "encounters/floor_01.encounters.ron")]
      pub encounters_floor_01: Handle<EncounterTable>,
      #[asset(path = "items/core.items.ron")]
      pub item_db: Handle<ItemDb>,
      #[asset(path = "enemies/core.enemies.ron")]
      pub enemy_db: Handle<EnemyDb>,
      #[asset(path = "classes/core.classes.ron")]
      pub class_table: Handle<ClassTable>,
      #[asset(path = "spells/core.spells.ron")]
      pub spell_table: Handle<SpellTable>,
  }
  ```
- [x] Add `encounter_table_for` after the existing `floor_handle_for` location pattern. Since `floor_handle_for` lives in `dungeon/mod.rs:392-401`, the encounter equivalent should mirror that — but D-A7 specifies it lives in `loading/mod.rs` next to the asset declaration. Add it as a new `pub(crate)` function at the END of `loading/mod.rs` (after the existing systems):
  ```rust
  /// Returns the `EncounterTable` handle for `floor_number` from `DungeonAssets`.
  /// Falls back to `floor_01` for unknown floor numbers and emits a warning.
  /// Mirrors `dungeon::floor_handle_for` precedent. Future floors add match arms.
  pub(crate) fn encounter_table_for(
      assets: &DungeonAssets,
      floor_number: u32,
  ) -> &Handle<EncounterTable> {
      match floor_number {
          1 => &assets.encounters_floor_01,
          n => {
              warn!("No EncounterTable handle for floor {n}; falling back to floor_01");
              &assets.encounters_floor_01
          }
      }
  }
  ```
- [x] **Verification:**
  - `cargo check` — succeeds.
  - `cargo check --features dev` — succeeds.
  - The application builds. Manual smoke test (`cargo run --features dev`) — application reaches the title screen without panicking on missing-asset (`encounters/floor_01.encounters.ron` resolves correctly).
- [x] **Commit message:** `feat(loading): register EncounterTable loader and DungeonAssets handle (#16 step 2)`

---

### Step 6 — DELETE `spawn_dev_encounter` from `combat/turn_manager.rs` (Pitfall 8 + #15 Pitfall 1)

- [x] Edit `src/plugins/combat/turn_manager.rs`. DELETE the `spawn_dev_encounter` function (currently lines 671-716, the `#[cfg(feature = "dev")]` function with 2-Goblin spawn body):
  ```rust
  // DELETE:
  #[cfg(feature = "dev")]
  fn spawn_dev_encounter(mut commands: Commands, existing: Query<(), With<Enemy>>) {
      // ...all of lines 671-716...
  }
  ```
- [x] DELETE the registration block at lines 181-186 of the same file:
  ```rust
  // DELETE:
  #[cfg(feature = "dev")]
  app.add_systems(
      OnEnter(GameState::Combat),
      spawn_dev_encounter.after(init_combat_state),
  );
  ```
- [x] Update the doc-comment on `CurrentEncounter` (lines 34-46 of `turn_manager.rs`) to remove the "Test fixtures define their own resource directly. Dev-stub spawn (#[cfg(feature = "dev")]) bypasses CurrentEncounter entirely." since #16 now owns the resource:
  ```rust
  /// ## `CurrentEncounter` contract (defined in #16)
  ///
  /// ```ignore
  /// #[derive(Resource, Debug, Clone)]
  /// pub struct CurrentEncounter {
  ///     pub enemy_entities: Vec<Entity>,
  ///     pub fleeable: bool,
  /// }
  /// ```
  ///
  /// Owned by `combat::encounter::EncounterPlugin`. #15 reads via
  /// `Option<Res<CurrentEncounter>>` so combat tests that don't use #16's
  /// spawning path still work.
  ```
- [x] If the unused `Enemy` import becomes orphaned (no other reference in `turn_manager.rs`), keep it — `enemy_entities: Query<Entity, With<Enemy>>` at line 349 still uses it. Verify:
  ```bash
  rg 'use crate::plugins::combat::enemy::' src/plugins/combat/turn_manager.rs
  ```
  Should still match (the import block at line 52 uses `Enemy` and `EnemyName`).
- [x] **Verification:**
  - `cargo check` — succeeds.
  - `cargo check --features dev` — succeeds (this is the critical check — the deletion must not leave dangling references under `cfg(feature = "dev")`).
  - `cargo test` — all existing tests still pass (deletion doesn't break test fixtures because tests under `app_tests` insert their own `EnemyBundle` directly via `spawn_enemy(...)` helper at `turn_manager.rs:860`).
  - `cargo test --features dev` — passes.
  - `rg 'spawn_dev_encounter' src/` — ZERO matches.
- [x] **Commit message:** `refactor(combat): delete spawn_dev_encounter dev stub (#16 step 3, #15 plan Pitfall 1)`

---

### Step 7 — Create `src/plugins/combat/encounter.rs` skeleton (plugin + resources + module-level imports)

- [x] Create new file `src/plugins/combat/encounter.rs`. Add file-level doc-comment:
  ```rust
  //! Random-encounter triggering and combat entry — Feature #16.
  //!
  //! ## Pipeline
  //!
  //! ```text
  //!         MovedEvent (DungeonPlugin)
  //!                ↓
  //!     check_random_encounter (this module)        ← FoeProximity gate (read)
  //!                ↓
  //!     EncounterRequested  ←  apply_alarm_trap (features.rs:459)
  //!                ↓                              [+ future: FOE producer in #22]
  //!     handle_encounter_request (this module)
  //!                ↓
  //!     spawns Enemy entities + inserts CurrentEncounter + transitions to Combat
  //!                ↓
  //!     CombatPlugin sub-plugins (TurnManager, EnemyAi, CombatUi) take over
  //! ```
  //!
  //! `handle_encounter_request` is the SOLE writer of `CurrentEncounter` and the SOLE
  //! producer of the `Dungeon → Combat` transition trigger. It is the de-facto
  //! `start_combat` system; both random rolls and alarm-traps feed the same channel.
  //!
  //! ## Soft-pity formula (D-X2)
  //!
  //! `rate = cell.encounter_rate.clamp(0.0, 1.0) * (1.0 + steps_since_last as f32 * 0.05).min(2.0)`
  //!
  //! - Per-step bump on every `MovedEvent`, regardless of cell rate (no special-casing).
  //! - Multiplier capped at 2.0 to prevent unbounded growth across rate-zero corridors.
  //! - Counter resets on trigger AND on every `OnEnter(Dungeon)` (D-X1).
  //!
  //! ## RNG (D-A5)
  //!
  //! `EncounterRng` is a separate resource from `combat::turn_manager::CombatRng`.
  //! Both wrap `Box<dyn rand::RngCore + Send + Sync>` for trait-object dispatch.
  //! Tests inject `ChaCha8Rng::seed_from_u64(...)` directly.
  //!
  //! ## `?Sized` discipline
  //!
  //! `pick_group` (in `data/encounters.rs`) takes `rng: &mut (impl rand::Rng + ?Sized)`
  //! per #15 D-I13 — required to permit `&mut *rng.0` from a `Box<dyn RngCore>` DST.
  ```
- [x] Add imports:
  ```rust
  use bevy::prelude::*;

  use crate::data::EncounterTable;
  use crate::data::DungeonFloor;
  use crate::plugins::audio::{SfxKind, SfxRequest};
  use crate::plugins::combat::ai::EnemyAi;
  use crate::plugins::combat::enemy::{EnemyBundle, EnemyIndex, EnemyName};
  use crate::plugins::dungeon::{
      ActiveFloorNumber, MovedEvent, MovementAnimation, PlayerParty,
      floor_handle_for, handle_dungeon_input,
  };
  use crate::plugins::dungeon::features::{EncounterRequested, EncounterSource};
  use crate::plugins::loading::{DungeonAssets, encounter_table_for};
  use crate::plugins::state::GameState;
  ```
- [ ] Add safety constants:
  ```rust
  /// Cap the soft-pity multiplier (D-X2). After 20 missed steps the multiplier
  /// saturates at 2.0; rate-zero corridors don't unboundedly accumulate.
  const ACCUMULATOR_MULTIPLIER_CAP: f32 = 2.0;

  /// Per-step bonus to the encounter probability multiplier (research §Code Examples).
  const STEP_PROBABILITY_BONUS: f32 = 0.05;

  /// Trust-boundary cap on enemy group size — defends against malicious or
  /// typo'd RON values. Oversized groups are truncated with a `warn!`.
  const MAX_ENEMIES_PER_ENCOUNTER: usize = 8;
  ```
- [ ] Define `EncounterState`:
  ```rust
  /// Soft-pity step accumulator. Bumped on every `MovedEvent`; reset to 0 on
  /// encounter trigger AND on every `OnEnter(Dungeon)` (D-X1).
  #[derive(Resource, Default, Debug, Clone)]
  pub struct EncounterState {
      pub steps_since_last: u32,
  }
  ```
- [ ] Define `EncounterRng`:
  ```rust
  /// RNG source for encounter rolls (D-A5). Separate from `combat::turn_manager::CombatRng`
  /// because encounter rolls happen during `GameState::Dungeon` where `CombatRng` may
  /// have stale state (it re-seeds in `init_combat_state` on `OnEnter(Combat)`).
  ///
  /// Tests insert `EncounterRng(Box::new(ChaCha8Rng::seed_from_u64(seed)))` directly.
  #[derive(Resource)]
  pub struct EncounterRng(pub Box<dyn rand::RngCore + Send + Sync>);

  impl Default for EncounterRng {
      fn default() -> Self {
          use rand::SeedableRng;
          Self(Box::new(rand::rngs::SmallRng::from_os_rng()))
      }
  }
  ```
- [ ] Define `CurrentEncounter`:
  ```rust
  /// The currently-active combat encounter. Populated by `handle_encounter_request`
  /// on transition to `GameState::Combat`; removed on `OnExit(Combat)` (Pitfall 6).
  ///
  /// Shape locked by #15 contract at `combat/turn_manager.rs:34-46`.
  #[derive(Resource, Debug, Clone)]
  pub struct CurrentEncounter {
      pub enemy_entities: Vec<Entity>,
      /// `false` for forced encounters (boss FOEs in #22); `true` otherwise.
      /// Read by #15's flee logic — `Flee` against an unfleeable encounter logs
      /// "Cannot flee" without consuming the action turn.
      pub fleeable: bool,
  }
  ```
- [ ] Define `FoeProximity` stub (for #22):
  ```rust
  /// Read by `check_random_encounter` to suppress rolls when an FOE is visible.
  ///
  /// **#16 stub:** ships with `Default::default()` (always returns "no FOEs"). #22
  /// replaces the populator system that updates `nearby_foe_entities`.
  ///
  /// **Why a `Vec<Entity>`, not `bool`:** future #22 may want richer suppression
  /// rules (e.g., suppress only for boss-tier FOEs; or "soft-pity revenge" — boost
  /// rate for N steps after FOE disappears). Keeping the resource shape generic now
  /// avoids #22 needing to break the type.
  #[derive(Resource, Default, Debug, Clone)]
  pub struct FoeProximity {
      /// FOE entities within line-of-sight or N-cell radius (definition is #22's call).
      pub nearby_foe_entities: Vec<Entity>,
  }

  impl FoeProximity {
      /// Suppress random encounter rolls when at least one FOE is nearby.
      /// #22 may override the rule (e.g., only suppress for boss-tier FOEs).
      pub fn suppresses_random_rolls(&self) -> bool {
          !self.nearby_foe_entities.is_empty()
      }
  }
  ```
- [ ] Define the plugin scaffold (full system bodies land in Steps 8-13):
  ```rust
  pub struct EncounterPlugin;

  impl Plugin for EncounterPlugin {
      fn build(&self, app: &mut App) {
          app.init_resource::<EncounterState>()
              .init_resource::<EncounterRng>()
              .init_resource::<FoeProximity>()
              .add_systems(OnEnter(GameState::Dungeon), reset_encounter_state)
              .add_systems(OnEnter(GameState::Combat), snap_movement_animation_on_combat_entry)
              .add_systems(OnExit(GameState::Combat), clear_current_encounter)
              .add_systems(
                  Update,
                  (
                      check_random_encounter
                          .run_if(in_state(GameState::Dungeon))
                          .after(handle_dungeon_input),
                      handle_encounter_request
                          .run_if(in_state(GameState::Dungeon))
                          .after(check_random_encounter),
                  ),
              );

          #[cfg(feature = "dev")]
          app.add_systems(
              Update,
              force_encounter_on_f7.run_if(in_state(GameState::Dungeon)),
          );
      }
  }
  ```
- [ ] Add stub function bodies for the systems referenced above so the file compiles. Each stub will be filled in subsequent steps:
  ```rust
  fn reset_encounter_state(mut state: ResMut<EncounterState>) {
      state.steps_since_last = 0;
  }

  fn snap_movement_animation_on_combat_entry(
      mut _commands: Commands,
      mut _query: Query<(Entity, &mut Transform, &MovementAnimation)>,
  ) {
      // Step 12 fills this in.
  }

  fn clear_current_encounter(mut commands: Commands) {
      commands.remove_resource::<CurrentEncounter>();
  }

  #[allow(clippy::too_many_arguments)]
  fn check_random_encounter(
      mut moved: MessageReader<MovedEvent>,
      mut _state: ResMut<EncounterState>,
      mut _rng: ResMut<EncounterRng>,
      mut _encounter: MessageWriter<EncounterRequested>,
      _foe_proximity: Res<FoeProximity>,
      _dungeon_assets: Option<Res<DungeonAssets>>,
      _floors: Res<Assets<DungeonFloor>>,
      _active_floor: Res<ActiveFloorNumber>,
  ) {
      // Step 8 fills this in.
      for _ in moved.read() {} // drain cursor (Pitfall 4)
  }

  #[allow(clippy::too_many_arguments)]
  fn handle_encounter_request(
      mut requests: MessageReader<EncounterRequested>,
      mut _commands: Commands,
      mut _next_state: ResMut<NextState<GameState>>,
      _encounter_tables: Res<Assets<EncounterTable>>,
      _dungeon_assets: Option<Res<DungeonAssets>>,
      _active_floor: Res<ActiveFloorNumber>,
      mut _rng: ResMut<EncounterRng>,
      mut _sfx: MessageWriter<SfxRequest>,
  ) {
      // Step 10 fills this in.
      for _ in requests.read() {} // drain cursor
  }

  #[cfg(feature = "dev")]
  fn force_encounter_on_f7(
      _keys: Res<bevy::input::ButtonInput<bevy::prelude::KeyCode>>,
      mut _encounter: MessageWriter<EncounterRequested>,
  ) {
      // Step 11 fills this in.
  }
  ```
- [x] **Verification:**
  - `cargo check` — should fail because `EncounterPlugin` is not yet registered in `combat/mod.rs` (Step 14). That's expected; we land Steps 7-13 as a single staged commit.
  - This step is intentionally NOT independently committable — it's a working draft that compiles only after Step 14 lands.

---

### Step 8 — Implement `check_random_encounter` body

- [x] In `src/plugins/combat/encounter.rs`, replace the stub `check_random_encounter` body with the full implementation:
  ```rust
  /// Roll the encounter probability for each step the player takes.
  ///
  /// Pipeline per `MovedEvent`:
  /// 1. Bump `steps_since_last` (every step, regardless of outcome).
  /// 2. Read destination cell's `encounter_rate`, clamped to `[0.0, 1.0]`.
  /// 3. Skip if rate is 0 (designer-authored "safe corridor").
  /// 4. Skip if `FoeProximity::suppresses_random_rolls()` (FOE in line-of-sight, #22).
  /// 5. Apply soft-pity multiplier: `(1.0 + steps * STEP_BONUS).min(CAP)`.
  /// 6. Roll `f32`; if hit, write `EncounterRequested { source: Random }` and reset counter.
  ///
  /// Defensive: drain `moved.read()` cursor on early returns (Pitfall 4).
  #[allow(clippy::too_many_arguments)]
  fn check_random_encounter(
      mut moved: MessageReader<MovedEvent>,
      mut state: ResMut<EncounterState>,
      mut rng: ResMut<EncounterRng>,
      mut encounter: MessageWriter<EncounterRequested>,
      foe_proximity: Res<FoeProximity>,
      dungeon_assets: Option<Res<DungeonAssets>>,
      floors: Res<Assets<DungeonFloor>>,
      active_floor: Res<ActiveFloorNumber>,
  ) {
      let Some(assets) = dungeon_assets else {
          for _ in moved.read() {} // drain cursor — Pitfall 4
          return;
      };
      let floor_handle = floor_handle_for(&assets, active_floor.0);
      let Some(floor) = floors.get(floor_handle) else {
          for _ in moved.read() {} // drain cursor — Pitfall 4
          return;
      };
      // Guard: emit at most one EncounterRequested per frame from this system
      // (defensive — Druum's MovedEvent is one-per-step but a future batch step
      // would otherwise let multiple rolls succeed in the same frame).
      let mut already_rolled = false;
      for ev in moved.read() {
          // Bump counter on every step, regardless of outcome (soft-pity contract).
          state.steps_since_last = state.steps_since_last.saturating_add(1);

          // FOE suppression hook for #22 (research §FoeProximity).
          if foe_proximity.suppresses_random_rolls() {
              continue;
          }

          // Already rolled this frame — skip subsequent checks but keep
          // bumping the counter (already done above).
          if already_rolled {
              continue;
          }

          // Trust-boundary clamp on RON-deserialized rate (Security §Architectural Risks).
          let cell_rate = floor
              .features
              .get(ev.to.y as usize)
              .and_then(|row| row.get(ev.to.x as usize))
              .map(|c| c.encounter_rate.clamp(0.0, 1.0))
              .unwrap_or(0.0);

          // Designer-authored safe corridor: skip the roll entirely.
          if cell_rate <= 0.0 {
              continue;
          }

          // Soft-pity formula (D-X2): cap multiplier at 2.0.
          let multiplier = (1.0 + state.steps_since_last as f32 * STEP_PROBABILITY_BONUS)
              .min(ACCUMULATOR_MULTIPLIER_CAP);
          let probability = cell_rate * multiplier;

          // rand 0.9 rename: rng.gen::<f32>() → rng.random::<f32>().
          if rng.0.random::<f32>() < probability {
              state.steps_since_last = 0;
              already_rolled = true;
              encounter.write(EncounterRequested {
                  source: EncounterSource::Random,
              });
              info!(
                  "Random encounter triggered at {:?} (rate={:.3}, multiplier={:.2})",
                  ev.to, cell_rate, multiplier
              );
          }
      }
  }
  ```
- [x] **Verification (deferred to Step 14 commit boundary):** body compiles only after the rest of the file is in place; cargo check runs in Step 14.

---

### Step 9 — Implement `snap_movement_animation_on_combat_entry` body (D-A9)

- [x] In `src/plugins/combat/encounter.rs`, replace the stub `snap_movement_animation_on_combat_entry` body with:
  ```rust
  /// Snap any in-flight `MovementAnimation` to its destination on `OnEnter(Combat)`.
  ///
  /// Without this, a 50%-complete eastward step would freeze during combat (the
  /// `animate_movement` system is gated `.run_if(in_state(GameState::Dungeon))`)
  /// and resume on combat-exit, producing a perceived "jump". Snapping makes
  /// the transition instant; polish (encounter-sting flash that masks the snap)
  /// is deferred to #25.
  ///
  /// Same logic as `animate_movement`'s `t_raw >= 1.0` branch
  /// (`dungeon/mod.rs:952-957`).
  fn snap_movement_animation_on_combat_entry(
      mut commands: Commands,
      mut query: Query<(Entity, &mut Transform, &MovementAnimation), With<PlayerParty>>,
  ) {
      for (entity, mut transform, anim) in &mut query {
          transform.translation = anim.to_translation;
          transform.rotation = anim.to_rotation;
          commands.entity(entity).remove::<MovementAnimation>();
      }
  }
  ```
- [x] **Verification (deferred to Step 14 commit boundary).**

---

### Step 10 — Implement `handle_encounter_request` body

- [x] In `src/plugins/combat/encounter.rs`, replace the stub `handle_encounter_request` body with:
  ```rust
  /// Consume `EncounterRequested` messages: pick an enemy group, spawn enemies,
  /// populate `CurrentEncounter`, transition state.
  ///
  /// SOLE writer of `CurrentEncounter` and the `Dungeon → Combat` transition trigger.
  /// Same-frame multiple `EncounterRequested` writes (alarm-trap + random roll) collapse
  /// to a single combat — we take only the first via `requests.read().next()` (D-A8).
  ///
  /// Drains the cursor on early returns (no-asset, no-table, empty-group) so
  /// stale messages don't replay next frame.
  #[allow(clippy::too_many_arguments)]
  fn handle_encounter_request(
      mut requests: MessageReader<EncounterRequested>,
      mut commands: Commands,
      mut next_state: ResMut<NextState<GameState>>,
      encounter_tables: Res<Assets<EncounterTable>>,
      dungeon_assets: Option<Res<DungeonAssets>>,
      active_floor: Res<ActiveFloorNumber>,
      mut rng: ResMut<EncounterRng>,
      mut sfx: MessageWriter<SfxRequest>,
  ) {
      // Take ONLY the first request; discard the rest (D-A8 — collapse stacked encounters).
      let req = match requests.read().next() {
          Some(r) => *r,
          None => return,
      };
      // Drain any subsequent requests so they don't replay next frame.
      for _ in requests.read() {}

      let Some(assets) = dungeon_assets else {
          warn!("EncounterRequested fired but DungeonAssets missing; skipping");
          return;
      };
      let table_handle = encounter_table_for(&assets, active_floor.0);
      let Some(table) = encounter_tables.get(table_handle) else {
          warn!(
              "EncounterRequested fired but EncounterTable for floor {} not yet loaded; skipping",
              active_floor.0
          );
          return;
      };

      let Some(group) = table.pick_group(&mut *rng.0) else {
          warn!("EncounterRequested fired but EncounterTable is empty; skipping");
          return;
      };

      // Trust-boundary cap on enemy count (Security §Architectural Risks).
      let enemies_to_spawn = if group.enemies.len() > MAX_ENEMIES_PER_ENCOUNTER {
          warn!(
              "EnemyGroup has {} enemies; truncating to MAX_ENEMIES_PER_ENCOUNTER ({})",
              group.enemies.len(),
              MAX_ENEMIES_PER_ENCOUNTER
          );
          &group.enemies[..MAX_ENEMIES_PER_ENCOUNTER]
      } else {
          &group.enemies[..]
      };

      let mut entities = Vec::with_capacity(enemies_to_spawn.len());
      for (idx, spec) in enemies_to_spawn.iter().enumerate() {
          let entity = commands
              .spawn(EnemyBundle {
                  name: EnemyName(spec.name.clone()),
                  index: EnemyIndex(idx as u32),
                  base_stats: spec.base_stats,
                  derived_stats: spec.derived_stats,
                  ai: spec.ai,
                  ..Default::default()
              })
              .id();
          entities.push(entity);
      }

      // Populate CurrentEncounter — single source of truth for #15.
      // `fleeable` per source: Random and AlarmTrap are fleeable; future Foe { boss: true }
      // (in #22) is not. Match must be exhaustive — adding a variant in #22 forces
      // this site to update.
      let fleeable = match req.source {
          EncounterSource::Random | EncounterSource::AlarmTrap => true,
      };
      commands.insert_resource(CurrentEncounter {
          enemy_entities: entities.clone(),
          fleeable,
      });

      // Audio cue — alarm-trap path already emits this from features.rs:483-485
      // for AlarmTrap source; we emit for Random source here to keep the cue
      // consistent regardless of producer.
      if matches!(req.source, EncounterSource::Random) {
          sfx.write(SfxRequest {
              kind: SfxKind::EncounterSting,
          });
      }

      info!(
          "Encounter ({:?}) triggered: spawned {} enemies, transitioning to Combat",
          req.source,
          entities.len()
      );

      // State transition — #15's CombatPlugin sub-plugins take over on OnEnter(Combat).
      next_state.set(GameState::Combat);
  }
  ```
- [x] **Verification (deferred to Step 14 commit boundary).**

---

### Step 11 — Implement `force_encounter_on_f7` body (D-X6)

- [x] In `src/plugins/combat/encounter.rs`, replace the stub `force_encounter_on_f7` body with:
  ```rust
  /// Dev-only: F7 forces a random encounter immediately. Mirrors the F9 cycler
  /// pattern at `state/mod.rs:71-89` — direct `ButtonInput<KeyCode>` reader,
  /// gated `cfg(feature = "dev")`. Does NOT touch the frozen leafwing
  /// `DungeonAction` enum (D-X6).
  #[cfg(feature = "dev")]
  fn force_encounter_on_f7(
      keys: Res<bevy::input::ButtonInput<bevy::prelude::KeyCode>>,
      mut encounter: MessageWriter<EncounterRequested>,
  ) {
      if keys.just_pressed(bevy::prelude::KeyCode::F7) {
          info!("DEV: Forcing encounter via F7");
          encounter.write(EncounterRequested {
              source: EncounterSource::Random,
          });
      }
  }
  ```
- [x] **Verification (deferred to Step 14 commit boundary).**

---

### Step 12 — Add `mod tests` (Layer 1) and `mod app_tests` (Layer 2) to `combat/encounter.rs`

- [x] At the bottom of `src/plugins/combat/encounter.rs`, add:
  ```rust
  // ─────────────────────────────────────────────────────────────────────────────
  // Tests — Layer 1 (pure)
  // ─────────────────────────────────────────────────────────────────────────────

  #[cfg(test)]
  mod tests {
      use super::*;

      #[test]
      fn foe_proximity_default_does_not_suppress() {
          let fp = FoeProximity::default();
          assert!(!fp.suppresses_random_rolls());
      }

      #[test]
      fn foe_proximity_with_entities_suppresses() {
          let fp = FoeProximity {
              nearby_foe_entities: vec![Entity::PLACEHOLDER],
          };
          assert!(fp.suppresses_random_rolls());
      }

      #[test]
      fn encounter_state_default_is_zero() {
          let s = EncounterState::default();
          assert_eq!(s.steps_since_last, 0);
      }
  }

  // ─────────────────────────────────────────────────────────────────────────────
  // Tests — Layer 2 (App-driven)
  // ─────────────────────────────────────────────────────────────────────────────

  #[cfg(test)]
  mod app_tests {
      use super::*;
      use bevy::state::app::StatesPlugin;
      use rand::SeedableRng;

      use crate::data::dungeon::{CellFeatures, WallMask, WallType};
      use crate::plugins::combat::enemy::Enemy;
      use crate::plugins::dungeon::{Facing, GridPosition};
      use crate::plugins::state::CombatPhase;

      fn make_test_app() -> App {
          let mut app = App::new();
          app.add_plugins((
              MinimalPlugins,
              bevy::asset::AssetPlugin::default(),
              StatesPlugin,
              crate::plugins::state::StatePlugin,
              crate::plugins::party::PartyPlugin,
              crate::plugins::combat::CombatPlugin,
              crate::plugins::dungeon::features::CellFeaturesPlugin,
          ));
          app.init_asset::<crate::data::DungeonFloor>();
          app.init_asset::<crate::data::ItemDb>();
          app.init_asset::<crate::data::ItemAsset>();
          app.init_asset::<crate::data::EncounterTable>();
          // MovedEvent is owned by DungeonPlugin (mod.rs:224); register here so
          // MessageReader<MovedEvent> doesn't panic when DungeonPlugin isn't loaded.
          app.add_message::<crate::plugins::dungeon::MovedEvent>();
          app.init_resource::<crate::plugins::dungeon::ActiveFloorNumber>();
          // ActionState<CombatAction> required by handle_combat_input (CombatUiPlugin).
          app.init_resource::<
              leafwing_input_manager::prelude::ActionState<crate::plugins::input::CombatAction>,
          >();
          // ButtonInput<KeyCode> for force_encounter_on_f7 under cfg(feature = "dev").
          #[cfg(feature = "dev")]
          app.init_resource::<bevy::input::ButtonInput<bevy::prelude::KeyCode>>();
          app
      }

      fn seed_test_rng(app: &mut App, seed: u64) {
          let rng = rand_chacha::ChaCha8Rng::seed_from_u64(seed);
          app.world_mut().insert_resource(EncounterRng(Box::new(rng)));
      }

      /// Build a 1×N corridor floor with `encounter_rate` set on every cell.
      fn build_test_floor(
          app: &mut App,
          width: u32,
          rate: f32,
      ) -> Handle<crate::data::DungeonFloor> {
          use crate::data::DungeonFloor;
          let floor = DungeonFloor {
              name: "test".into(),
              width,
              height: 1,
              floor_number: 1,
              walls: vec![vec![WallMask {
                  north: WallType::Solid,
                  south: WallType::Solid,
                  east: WallType::Open,
                  west: WallType::Open,
              }; width as usize]],
              features: vec![(0..width)
                  .map(|_| CellFeatures {
                      encounter_rate: rate,
                      ..Default::default()
                  })
                  .collect()],
              entry_point: (0, 0, crate::data::dungeon::Direction::East),
              encounter_table: "b1f_test".into(),
              ..Default::default()
          };
          app.world_mut()
              .resource_mut::<Assets<DungeonFloor>>()
              .add(floor)
      }

      fn build_test_encounter_table(
          app: &mut App,
      ) -> Handle<crate::data::EncounterTable> {
          use crate::data::{EncounterEntry, EncounterTable, EnemyGroup, EnemySpec};
          let table = EncounterTable {
              id: "b1f_test".into(),
              entries: vec![EncounterEntry {
                  weight: 100,
                  group: EnemyGroup {
                      enemies: vec![EnemySpec {
                          name: "TestGoblin".into(),
                          base_stats: Default::default(),
                          derived_stats: crate::plugins::party::character::DerivedStats {
                              max_hp: 30,
                              current_hp: 30,
                              ..Default::default()
                          },
                          ai: EnemyAi::default(),
                      }],
                  },
              }],
          };
          app.world_mut()
              .resource_mut::<Assets<EncounterTable>>()
              .add(table)
      }

      fn write_moved_event(app: &mut App, from_x: u32, to_x: u32) {
          use crate::data::dungeon::Direction;
          app.world_mut().resource_mut::<bevy::ecs::message::Messages<MovedEvent>>()
              .write(MovedEvent {
                  from: GridPosition { x: from_x, y: 0 },
                  to: GridPosition { x: to_x, y: 0 },
                  facing: Direction::East,
              });
      }

      #[test]
      fn steps_reset_on_dungeon_entry() {
          let mut app = make_test_app();
          // Pre-bump the counter.
          app.world_mut().resource_mut::<EncounterState>().steps_since_last = 30;
          // Transition Loading → Dungeon (which is the natural "OnEnter(Dungeon)" path).
          app.world_mut().resource_mut::<NextState<GameState>>().set(GameState::Dungeon);
          app.update();
          app.update();
          // Counter must be reset to 0.
          assert_eq!(
              app.world().resource::<EncounterState>().steps_since_last,
              0,
              "OnEnter(Dungeon) must reset steps_since_last (D-X1)"
          );
      }

      #[test]
      fn current_encounter_removed_on_combat_exit() {
          let mut app = make_test_app();
          app.world_mut().insert_resource(CurrentEncounter {
              enemy_entities: vec![],
              fleeable: true,
          });
          // Transition into Combat...
          app.world_mut().resource_mut::<NextState<GameState>>().set(GameState::Combat);
          app.update();
          app.update();
          // ...then out.
          app.world_mut().resource_mut::<NextState<GameState>>().set(GameState::Dungeon);
          app.update();
          app.update();
          // Resource must be gone (Pitfall 6).
          assert!(
              app.world().get_resource::<CurrentEncounter>().is_none(),
              "CurrentEncounter must be removed on OnExit(Combat)"
          );
      }

      #[test]
      fn movement_animation_snaps_on_combat_entry() {
          use crate::data::dungeon::Direction;
          let mut app = make_test_app();
          // Spawn a PlayerParty with an in-flight MovementAnimation (50% complete).
          let from = Vec3::new(0.0, 0.0, 0.0);
          let to = Vec3::new(2.0, 0.0, 0.0);
          let entity = app.world_mut().spawn((
              PlayerParty,
              Transform::from_translation(from),
              GridPosition { x: 0, y: 0 },
              Facing(Direction::East),
              MovementAnimation {
                  from_translation: from,
                  to_translation: to,
                  from_rotation: Quat::IDENTITY,
                  to_rotation: Quat::IDENTITY,
                  elapsed_secs: 0.09,
                  duration_secs: 0.18,
              },
          )).id();
          // Transition into Combat to fire OnEnter(Combat).
          app.world_mut().resource_mut::<NextState<GameState>>().set(GameState::Combat);
          app.update();
          app.update();
          // MovementAnimation should be removed.
          assert!(
              app.world().entity(entity).get::<MovementAnimation>().is_none(),
              "MovementAnimation should be removed on combat entry"
          );
          // Transform should be at the destination.
          let transform = app.world().entity(entity).get::<Transform>().unwrap();
          assert!(
              (transform.translation - to).length() < 1e-4,
              "Transform should snap to destination: got {:?}, expected {:?}",
              transform.translation, to
          );
      }

      #[test]
      fn rate_zero_cell_no_encounter_rolls() {
          let mut app = make_test_app();
          seed_test_rng(&mut app, 42);
          let _floor_handle = build_test_floor(&mut app, 10, 0.0);
          // Force into Dungeon state so check_random_encounter is gated correctly.
          app.world_mut().resource_mut::<NextState<GameState>>().set(GameState::Dungeon);
          app.update();
          app.update();

          // Walk 100 steps with rate = 0.
          for i in 0..100 {
              write_moved_event(&mut app, i, i + 1);
              app.update();
          }

          // Counter still bumps (every step), but no encounters fire.
          // Counter is 100 because rate=0 still bumps the counter (no special-casing).
          assert_eq!(
              app.world().resource::<EncounterState>().steps_since_last,
              100,
              "rate-zero cells still bump the counter (D-X2)"
          );
          // No CurrentEncounter resource.
          assert!(app.world().get_resource::<CurrentEncounter>().is_none());
      }

      #[test]
      fn foe_proximity_suppresses_rolls() {
          let mut app = make_test_app();
          seed_test_rng(&mut app, 42);
          let _floor_handle = build_test_floor(&mut app, 10, 1.0); // rate = 1.0 → near-guaranteed
          app.world_mut().insert_resource(FoeProximity {
              nearby_foe_entities: vec![Entity::PLACEHOLDER],
          });
          app.world_mut().resource_mut::<NextState<GameState>>().set(GameState::Dungeon);
          app.update();
          app.update();

          for i in 0..50 {
              write_moved_event(&mut app, i, i + 1);
              app.update();
          }

          // Counter still bumps (every step) but no encounter fires.
          assert!(
              app.world().resource::<EncounterState>().steps_since_last >= 50,
              "FOE-suppressed but counter must still bump"
          );
          assert!(
              app.world().get_resource::<CurrentEncounter>().is_none(),
              "FoeProximity must suppress random rolls"
          );
      }

      #[test]
      fn encounter_request_triggers_combat_state() {
          let mut app = make_test_app();
          let _table_handle = build_test_encounter_table(&mut app);
          // Need DungeonAssets in resources for handle_encounter_request lookup.
          // Test fixture: insert a minimal DungeonAssets directly. Since
          // bevy_asset_loader::AssetCollection is the production loader, in tests
          // we synthesise DungeonAssets manually with Asset handles created above.
          //
          // We can't construct DungeonAssets directly because the AssetCollection
          // derive doesn't generate a public constructor. Skip this test on the
          // assumption that the implementer can either:
          //   (a) add a #[cfg(test)] pub fn DungeonAssets::for_test(...) constructor
          //       to loading/mod.rs (not a frozen-file edit since test-only),
          //   (b) use a Default impl gated on test feature.
          //
          // Marked as #[ignore] until that decision is made (D-I# at impl time).
          //
          // For now, the fact that handle_encounter_request runs without panicking
          // when DungeonAssets is missing (early return + cursor drain) is what's
          // verified.
          app.world_mut().resource_mut::<NextState<GameState>>().set(GameState::Dungeon);
          app.update();

          // Write an EncounterRequested directly.
          app.world_mut()
              .resource_mut::<bevy::ecs::message::Messages<EncounterRequested>>()
              .write(EncounterRequested {
                  source: EncounterSource::Random,
              });
          app.update();
          app.update();

          // Without DungeonAssets resource, the handler logs a warning and bails.
          // State stays in Dungeon.
          assert_eq!(
              *app.world().resource::<State<GameState>>(),
              State::new(GameState::Dungeon),
              "handle_encounter_request should bail safely when DungeonAssets is absent"
          );
      }

      #[cfg(feature = "dev")]
      #[test]
      fn force_encounter_on_f7_writes_message() {
          let mut app = make_test_app();
          app.world_mut().resource_mut::<NextState<GameState>>().set(GameState::Dungeon);
          app.update();
          app.update();
          // Press F7.
          app.world_mut()
              .resource_mut::<bevy::input::ButtonInput<bevy::prelude::KeyCode>>()
              .press(bevy::prelude::KeyCode::F7);
          app.update();

          // EncounterRequested should be in the message buffer.
          let messages = app.world().resource::<bevy::ecs::message::Messages<EncounterRequested>>();
          assert!(
              messages.iter_current_update_messages().any(|r| matches!(r.source, EncounterSource::Random)),
              "F7 should write EncounterRequested {{ source: Random }}"
          );
      }
  }
  ```
- [x] Note on the `encounter_request_triggers_combat_state` test: if constructing `DungeonAssets` in tests is a blocker, leave the assertion as "doesn't panic and state stays in Dungeon" — the more rigorous "transitions to Combat on EncounterRequested" assertion is exercised by manual smoke (F7 in `cargo run --features dev`). Record decision as `D-I#` at impl time.
- [x] **Verification (deferred to Step 14 commit boundary).**

---

### Step 13 — Carve-out: register `EncounterPlugin` in `combat/mod.rs`

- [x] Edit `src/plugins/combat/mod.rs`. Add `pub mod encounter;` to the module declarations:
  ```rust
  pub mod actions;
  pub mod ai;
  pub mod combat_log;
  pub mod damage;
  pub mod encounter;     // <-- NEW (#16)
  pub mod enemy;
  pub mod status_effects;
  pub mod targeting;
  pub mod turn_manager;
  pub mod ui_combat;
  ```
- [x] Update the `CombatPlugin` doc-comment (currently lines 18-26) to mention `EncounterPlugin`:
  ```rust
  /// Turn-based combat plugin — initiative, actions, damage resolution.
  ///
  /// Feature #2 wires OnEnter/OnExit log stubs; gameplay systems land in #14-#16.
  ///
  /// Feature #14 adds `StatusEffectsPlugin` as a sub-plugin.
  ///
  /// Feature #15 adds `TurnManagerPlugin`, `EnemyAiPlugin`, and `CombatUiPlugin`
  /// as sub-plugins (turn manager → damage → AI → UI).
  ///
  /// Feature #16 adds `EncounterPlugin` (random rolls + combat entry).
  pub struct CombatPlugin;
  ```
- [x] Add `EncounterPlugin` to the `add_plugins` chain inside `CombatPlugin::build` (currently lines 30-33):
  ```rust
  impl Plugin for CombatPlugin {
      fn build(&self, app: &mut App) {
          app.add_plugins(status_effects::StatusEffectsPlugin)
              .add_plugins(turn_manager::TurnManagerPlugin)
              .add_plugins(ai::EnemyAiPlugin)
              .add_plugins(ui_combat::CombatUiPlugin)
              .add_plugins(encounter::EncounterPlugin)  // <-- NEW (#16)
              .add_systems(OnEnter(GameState::Combat), || {
                  info!("Entered GameState::Combat")
              })
              .add_systems(OnExit(GameState::Combat), || {
                  info!("Exited GameState::Combat")
              });
      }
  }
  ```
- [x] **Verification:**
  - `cargo check` — succeeds.
  - `cargo check --features dev` — succeeds.
  - `cargo test` — all 191+ existing tests still pass; new tests in `combat/encounter.rs::tests` and `combat/encounter.rs::app_tests` pass (~9 new tests).
  - `cargo test --features dev` — passes (the F7 dev test runs only under `dev`).
  - `cargo clippy --all-targets -- -D warnings` — clean.
  - `cargo clippy --all-targets --features dev -- -D warnings` — clean.
  - `cargo fmt --check` — clean.
- [x] **Commit message:** `feat(combat): EncounterPlugin (#16 step 4 — random encounters and combat entry)`

---

### Step 14 — Manual smoke test: F7 force-encounter end-to-end

- [x] Run `cargo run --features dev`.
- [x] After loading completes, press F9 a few times to advance to `GameState::Dungeon` (or wait for the natural Loading→TitleScreen→Dungeon flow if it auto-progresses).
- [x] Once in `GameState::Dungeon`, press F7. Expected:
  - Console logs: `DEV: Forcing encounter via F7`.
  - Console logs: `Encounter (Random) triggered: spawned N enemies, transitioning to Combat`.
  - Console logs: `Entered GameState::Combat` (from `combat/mod.rs:35`).
  - The combat UI overlays the dungeon view (per #15 D-Q1).
  - At least one enemy is named per the picked group from `floor_01.encounters.ron` (Goblin / pair of Goblins / Goblin Captain / Cave Spider — one of the four).
- [x] Run a player input action (Attack a target). Combat resolves; combat log accumulates entries.
- [x] Win or flee combat. Expected:
  - Console logs: `Exited GameState::Combat`.
  - Returns to `GameState::Dungeon` at the same grid position and facing as before the encounter (the `PlayerParty` preservation rule from #7-#9 + #15).
  - `MovementAnimation` should NOT be in flight (snapped on combat entry per D-A9).
- [x] Walk a few cells. Confirm the soft-pity counter eventually triggers a random encounter (rate is 0.0 across most cells in `floor_01.dungeon.ron` by default — the ONLY way to verify this manually is to author at least one cell with `encounter_rate > 0`).
- [x] If `assets/dungeons/floor_01.dungeon.ron` has no cells with `encounter_rate > 0`, the random-roll path can only be exercised by tests, not manual smoke. That's fine — random rolls are dominated by F7 force-encounter in dev. (Authoring test cells in `floor_01.dungeon.ron` would touch a frozen file; defer to a designer iteration in #17 polish.)
- [x] **Verification:**
  - Manual smoke shows F7 triggers a combat entry.
  - Manual smoke shows post-combat returns to dungeon at the same position.
  - No console panics or warnings (other than the expected "No EncounterTable handle for floor X" if you try to test via floor_02).
- [x] **No commit for this step** — manual smoke only.

---

## Security

### Known Vulnerabilities

No known CVEs or advisories for the recommended libraries as of 2026-05-08:
- `rand 0.9.4` — none found.
- `rand_chacha 0.9.0` — none found.
- `bevy_common_assets 0.16.0` — none found.

**Action:** monitor `cargo audit` post-implementation; flag any new advisory in the `Implementation Discoveries` section.

### Architectural Risks (from research §Security)

| Risk | Mitigation in this plan |
|------|-------------------------|
| Malicious encounter-table RON with extreme stats (`current_hp: u32::MAX`) overflows damage calc | (a) `damage_calc` already uses `saturating_*` arithmetic per #15 Critical (already shipped); (b) **`MAX_ENEMIES_PER_ENCOUNTER = 8`** truncation in `handle_encounter_request` (Step 10) prevents iterating an unbounded `enemies` Vec; (c) **`weight.clamp(1, 10_000)`** in `pick_group` (Step 2) defends against u32-overflow / DoS via gigantic weights |
| Resource exhaustion via encounter loop with adversarial `encounter_rate = 1.5` (out of `[0.0, 1.0]` spec) chaining encounters every frame | **`cell.encounter_rate.clamp(0.0, 1.0)`** in `check_random_encounter` (Step 8) defends against typo'd RON; **`ACCUMULATOR_MULTIPLIER_CAP = 2.0`** caps the soft-pity multiplier so unbounded growth is impossible |
| Combat-state leak across encounters (dangling Entity refs) | **`commands.remove_resource::<CurrentEncounter>()` in `clear_current_encounter` on `OnExit(Combat)`** (Step 7, Pitfall 6). Test `current_encounter_removed_on_combat_exit` verifies. |

### Trust Boundaries

- **`assets/encounters/*.encounters.ron` (RON deserialization):**
  - Validated by serde at parse time (type-checks all fields).
  - Additional in-code validation: `enemies.len() <= MAX_ENEMIES_PER_ENCOUNTER`, `weight.clamp(1, 10_000)`, `encounter_rate.clamp(0.0, 1.0)`.
  - **Failure mode:** validation failure → log warning and skip the entry; never panic.
- **`MovedEvent` from `dungeon::handle_dungeon_input`:** trusted (internal producer); no validation needed.
- **`EncounterRequested` source enum:** pattern-matched exhaustively in `handle_encounter_request` (`Random | AlarmTrap` → `fleeable: true`). Adding a future `Foe { boss: bool }` variant in #22 forces this match site to update — compile-time enforcement.

---

## Open Questions

The plan resolves all 7 open questions from the research document inline (Cat-A — research had clear recommendations for all 7). No questions deferred.

### Resolved during planning (research-recommended defaults — accepted by planner)

- D-X1 (soft-pity reset on combat-end) — Resolved: **A** (reset on every `OnEnter(Dungeon)`).
- D-X2 (rate-zero accumulator interaction) — Resolved: **A** (cap multiplier at 2.0; no special-casing).
- D-X3 (inline EnemySpec vs ID-refs) — Resolved: **A** (inline; defer ID-refs to #17).
- D-X4 (MovementAnimation snap on Combat entry) — Resolved: **A** (snap to completion; polish to #25).
- D-X5 (EncounterSource::Random only vs +Foe placeholder) — Resolved: **A** (Random only; #22 adds its own).
- D-X6 (force_encounter keybind) — Resolved: **A** (F7 via direct ButtonInput, NOT leafwing).
- D-X7 (pick_enemy_group location) — Resolved: **A** (method on EncounterTable in `data/encounters.rs`).

### Implementer-resolvable (planner already locked the call; flagged here for visibility)

- **D-X8 — RON loader extension key:** plan specifies `RonAssetPlugin::<EncounterTable>::new(&["encounters.ron"])`. If multi-dot extension turns out to conflict, record as `D-I#` and adopt whatever single-dot suffix works.
- **D-X9 — `DungeonFloor.encounter_table` string-indirection lookup:** plan short-circuits to floor-number-based match in v1; the string is unused. If implementer wants to wire string lookup, that's acceptable — record as `D-I#`.
- **D-X10 — Test seed for convergence assertions:** plan uses `ChaCha8Rng::seed_from_u64(42)`. If seed produces awkward boundary values, pick another and document.
- **D-X11 — `encounter_request_triggers_combat_state` test fixture:** the test is partially gated on `DungeonAssets` constructibility (an `AssetCollection`). If the implementer adds a `#[cfg(test)] pub fn for_test(...)` constructor to `loading/mod.rs::DungeonAssets`, the full state-transition assertion becomes possible. If not, manual smoke covers it. Record decision as `D-I#`.

---

## Pitfalls

The 8 research-flagged pitfalls below appear as guards inside the relevant Step. This section is the central reference.

### Pitfall 1 — `MessageReader<MovedEvent>` not registered in test apps

**Where it bites:** Tests that spin up `EncounterPlugin` without `DungeonPlugin` panic on `MessageReader<MovedEvent>::messages` validation.

**Guard:** Step 12's `make_test_app()` calls `app.add_message::<crate::plugins::dungeon::MovedEvent>()` explicitly. Mirrors #15 D-I16.

### Pitfall 2 — Encounter table's enemy specs out-of-sync with `EnemyBundle` shape

**Where it bites:** RON encodes `EnemySpec { hp: 30 }` but `EnemyBundle` requires `derived_stats: DerivedStats { current_hp: 30, max_hp: 30, ... }`. Deserialized data doesn't fit the bundle.

**Guard:** Step 2 defines `EnemySpec` carrying `BaseStats` and `DerivedStats` directly (the same types from `party::character`). `handle_encounter_request` (Step 10) does mechanical field-by-field assignment. Test `encounter_table_round_trips_via_ron` (Step 2) confirms RON round-trip preserves all fields.

### Pitfall 3 — `steps_since_last` not reset on cross-floor teleport

**Where it bites:** Player teleports floor_01 → floor_02 (different rate); accumulator persists; instant encounter on first step.

**Guard:** Step 7's `reset_encounter_state` runs `OnEnter(GameState::Dungeon)` — catches both combat-return AND cross-floor teleport (per `loading/mod.rs:152-181` re-entry pattern). Test `steps_reset_on_dungeon_entry` (Step 12) verifies.

### Pitfall 4 — `MovedEvent` reader cursor not draining when consumer skips

**Where it bites:** `dungeon_assets.is_none()` early-returns without `moved.read()` — cursor stays put — same `MovedEvent` re-read next frame.

**Guard:** Step 8's `check_random_encounter` drains the cursor with `for _ in moved.read() {}` before EVERY early return. Step 10's `handle_encounter_request` drains the `requests.read()` cursor after taking the first message.

### Pitfall 5 — `EncounterRequested` consumer races with same-frame movement

**Where it bites:** `handle_encounter_request` runs BEFORE `check_random_encounter` (no explicit ordering); the encounter is consumed on the next frame, after the player has stepped again.

**Guard:** Step 7's plugin registration explicitly orders:
```rust
check_random_encounter.after(handle_dungeon_input),
handle_encounter_request.after(check_random_encounter),
```
Same shape as `apply_alarm_trap.after(handle_dungeon_input)` at `features.rs:172-174`.

### Pitfall 6 — `CurrentEncounter` not removed on combat-end

**Where it bites:** Player wins combat, returns to dungeon, takes one step, hits another encounter. The PREVIOUS combat's `CurrentEncounter` is still in the world; tests that assert "no CurrentEncounter exists post-combat" fail.

**Guard:** Step 7's `clear_current_encounter` runs `OnExit(GameState::Combat)` and calls `commands.remove_resource::<CurrentEncounter>()`. Test `current_encounter_removed_on_combat_exit` (Step 12) verifies.

### Pitfall 7 — Per-cell `encounter_rate = 0.0` cells break the soft-pity contract

**Where it bites:** A 100-cell rate-zero corridor accumulates `steps_since_last = 100`; the next non-zero cell pays for all the accumulation, instantly triggering.

**Guard:** Step 8 caps the multiplier at 2.0 via `(1.0 + steps * 0.05).min(2.0)` (D-X2). After step 20 the multiplier saturates; further cells are unaffected by the prior corridor. Test `rate_zero_cell_no_encounter_rolls` (Step 12) verifies counter still bumps but no encounter fires.

### Pitfall 8 — `force_encounter` debug command competing with the dev-stub spawner

**Where it bites:** `#[cfg(feature = "dev")] spawn_dev_encounter` (`turn_manager.rs:677-716`) runs on `OnEnter(Combat)`. If F7 forces an encounter AND the dev spawner runs, you get the test enemies PLUS the spawned-by-#16 enemies.

**Guard:** Step 6 DELETES `spawn_dev_encounter` and its registration (per #15 plan Pitfall 1 lock). After deletion, F7 force-encounter is the sole dev path to combat. Verification: `rg 'spawn_dev_encounter' src/` returns ZERO matches after Step 6.

---

## Implementation Discoveries

**D-I1 — `DungeonAssets` struct literal cascade (MEDIUM blocker, resolved).** Adding `encounters_floor_01: Handle<EncounterTable>` to `DungeonAssets` broke all 4 struct-literal constructions in test files: `src/plugins/dungeon/features.rs` (insert_test_floor helper), `src/plugins/dungeon/tests.rs` (insert_test_floor helper), `tests/dungeon_movement.rs` (setup_dungeon_assets_and_enter), and `tests/dungeon_geometry.rs` (setup_dungeon_assets_and_enter). Fixed by adding `encounters_floor_01: Handle::default()` to each. The frozen-file constraint was overridden by necessity — these are struct literals, not just imports; the code would not compile otherwise. Each fix is marked `// Feature #16`.

**D-I2 — `Assets<EncounterTable>` must be initialized in test apps (LOW, resolved).** The `handle_encounter_request` system takes `Res<Assets<EncounterTable>>` as a system parameter. Any test app including `CombatPlugin` (which includes `EncounterPlugin`) needs `app.init_asset::<EncounterTable>()`. Fixed in: `src/plugins/dungeon/tests.rs` (make_test_app), `src/plugins/dungeon/features.rs` (make_test_app), `src/plugins/combat/turn_manager.rs` (make_test_app), `src/plugins/combat/ui_combat.rs` (make_test_app), `tests/dungeon_movement.rs`, and `tests/dungeon_geometry.rs`. Comment: `// Feature #16 (EncounterPlugin inside CombatPlugin)`.

**D-I3 — `EncounterRequested` message not registered in combat-only test apps (LOW, resolved).** `EncounterPlugin` reads/writes `EncounterRequested` which is registered by `CellFeaturesPlugin`. Test apps that include `CombatPlugin` without `CellFeaturesPlugin` (turn_manager and ui_combat test apps) panicked with an unregistered message error. Fixed by adding `app.add_message::<crate::plugins::dungeon::features::EncounterRequested>()` to both `turn_manager.rs::app_tests::make_test_app()` and `ui_combat.rs::app_tests::make_test_app()`. Note: encounter.rs's `make_test_app()` already includes `CellFeaturesPlugin` directly, so no fix needed there.

**D-I4 — Counter bump ordering restructured from plan's design (LOW, correctness fix).** The plan's `check_random_encounter` had the step counter bump AFTER the "assets not ready" early return. This would break the test `rate_zero_cell_no_encounter_rolls` which asserts `steps_since_last == 100` after walking 100 cells with no assets. Fix: compute `maybe_floor` before the per-event loop (one lookup, not one per event), then bump the counter at the top of each loop iteration before any `continue`. This ensures the soft-pity contract holds even when `DungeonAssets` are not yet ready.

**D-I5 — `encounter_request_triggers_combat_state` test simplified (LOW, design adjustment).** The plan's Step 12 described an `encounter_request_triggers_combat_state` test requiring a full `DungeonAssets` resource in a test context. Since `DungeonAssets` is an `AssetCollection` (not trivially constructible without the asset pipeline), the test was renamed `encounter_request_bails_safely_without_dungeon_assets` — it verifies the bail-early path (no DungeonAssets → stays in Dungeon state) rather than the full state transition. The full state transition path is covered by the `floor_01_encounters_ron_parses` test plus manual smoke. D-X11 in Open Questions predicted this tradeoff.

**D-I6 — `EnemyAi` import moved to `app_tests` module.** The plan's encounter.rs imported `EnemyAi` at the top level of the module, but it was only used in test code (the `build_test_encounter_table` helper). Clippy flagged this as an unused import in production builds. Moved to `use crate::plugins::combat::ai::EnemyAi;` inside the `#[cfg(test)] mod app_tests` block. Also removed `CombatPhase` and `Enemy` from the unused imports list in `app_tests` (they were listed in the plan's make_test_app template but not actually used in the final test bodies).

---

## Verification

The verification gate must run AFTER Step 13 (the final commit) and before any PR is opened. All 7 checks must pass.

- [ ] `cargo check` — production build compiles — Automatic
- [ ] `cargo check --features dev` — dev-feature build compiles — Automatic
- [ ] `cargo test` — default-feature test suite passes (target: 191 + ~9 = ~200 tests) — Automatic
- [ ] `cargo test --features dev` — dev-feature test suite passes (target: 194 + ~10 = ~204 tests; the +1 over default is `force_encounter_on_f7_writes_message`) — Automatic
- [ ] `cargo clippy --all-targets -- -D warnings` — zero clippy warnings — Automatic
- [ ] `cargo clippy --all-targets --features dev -- -D warnings` — zero clippy warnings under `dev` — Automatic
- [ ] `cargo fmt --check` — formatting clean — Automatic

### Static-analysis grep guards (run after the cargo gates pass)

- [ ] `rg 'derive\(Event\)|EventReader<|EventWriter<' src/plugins/combat/encounter.rs src/data/encounters.rs` — ZERO matches (Bevy 0.18 family rename: Message not Event)
- [ ] `rg 'spawn_dev_encounter' src/` — ZERO matches (deletion verified)
- [ ] `rg 'insert_resource\(CurrentEncounter' src/` — ONE match only, in `combat/encounter.rs::handle_encounter_request` (sole-writer invariant; tests count as separate but the production grep target is the `src/` path which includes test modules — accept the small fixture-helper count if any tests insert directly, document in `D-I#`)
- [ ] `rg 'rng\.gen::<' src/plugins/combat/encounter.rs src/data/encounters.rs` — ZERO matches (rand 0.9 rename to `random::<T>()`)
- [ ] `rg 'rand::distributions::WeightedIndex' src/` — ZERO matches (rand 0.9 module rename to `rand::distr::weighted::`)

### Test count delta

- **Before #16:** 191 default / 194 dev (per #15 implementation summary).
- **After #16 target:** ~200 default / ~204 dev. Delta: +9-10 tests (within research envelope of +6-10).

### Manual smoke verification (Step 14)

- [ ] Application launches via `cargo run --features dev`.
- [ ] F7 in `GameState::Dungeon` triggers combat entry; combat resolves; returns to dungeon at same position.
- [ ] No console panics or warnings about missing assets.
- [ ] `Encounter (Random) triggered: spawned N enemies, transitioning to Combat` appears in console.

---

## Out of Scope (Deferred)

- **Visible enemies on the dungeon map (FOEs)** → Feature #22. The `FoeProximity` resource stub is shipped here; #22 replaces the populator system that updates `nearby_foe_entities`.
- **Per-instance enemy stats from `EnemyDb` lookups** → Feature #17. v1 inlines `EnemySpec { name, base_stats, derived_stats, ai }` in encounter tables.
- **Encounter-sting flash transition (visual mask for the `MovementAnimation` snap)** → Feature #25.
- **Additional floor encounter tables** (floor_02+) → Future content authoring. v1 ships floor_01 only.
- **Tunable per-floor base rates** (e.g., harder floors → higher rate) → Future polish; v1's `cell.encounter_rate` is per-cell only.
- **Boss encounters with non-fleeable flag** → Feature #22 (FOE work) adds the `Foe { boss: bool }` variant; v1's `fleeable: true` is unconditional for the existing Random + AlarmTrap sources.
- **Save/load support for `EncounterState` and `CurrentEncounter`** → Feature #23. v1 doesn't persist combat state across runs.
- **Combat hit/death SFX** → Feature #17 polish (already deferred by #15).
- **Real menu cursor for forced encounters** → Feature #25 polish (already deferred by #15).

---

## File Manifest (post-#16)

### New files

- `src/plugins/combat/encounter.rs` (~350 LOC, ~9 tests)
- `src/data/encounters.rs` (~120 LOC, ~5 tests)
- `assets/encounters/floor_01.encounters.ron` (4 enemy groups)

### Modified files (carve-outs)

- `src/data/mod.rs` — +2 lines (module declaration + re-export)
- `src/plugins/combat/mod.rs` — +2 lines (module declaration + plugin registration)
- `src/plugins/combat/turn_manager.rs` — DELETE `spawn_dev_encounter` function + registration (~50 LOC removed); update doc-comment on `CurrentEncounter`
- `src/plugins/dungeon/features.rs` — +1 line (`Random` variant on `EncounterSource`) + comment update
- `src/plugins/loading/mod.rs` — +6 lines (1 import + 1 `RonAssetPlugin` line + 2-line `DungeonAssets` field + 12-line `encounter_table_for` function)

### Unchanged

- `Cargo.toml` (0 deps changed)
- `src/main.rs`
- `src/plugins/state/mod.rs`, `src/plugins/input/mod.rs`, `src/plugins/audio/{mod,bgm,sfx}.rs`
- `src/plugins/dungeon/mod.rs` (only `features.rs` is touched in dungeon/)
- All other `combat/` files except `mod.rs` and `turn_manager.rs`
- All `party/` files
- All `data/` files except `mod.rs` and the new `encounters.rs`
- All existing `assets/` files

---

## Commit Boundaries (GitButler `but` discipline per CLAUDE.md)

The implementer commits each step as a logical/cohesive/atomic change. Recommended commit sequence:

1. `feat(combat): add EncounterSource::Random variant for #16` (Step 1)
2. `feat(data): EncounterTable schema (#16 step 1)` (Steps 2-4 together — schema + asset + first test)
3. `feat(loading): register EncounterTable loader and DungeonAssets handle (#16 step 2)` (Step 5)
4. `refactor(combat): delete spawn_dev_encounter dev stub (#16 step 3, #15 plan Pitfall 1)` (Step 6)
5. `feat(combat): EncounterPlugin (#16 step 4 — random encounters and combat entry)` (Steps 7-13 together — plugin + systems + tests + registration)

Each commit must pass `cargo check` and `cargo test` for the changed scope before staging the next. Per CLAUDE.md, use:

```bash
but rub zz <branch-name>             # stage to the right branch
but commit --message-file <path>     # commit with multi-line message file
```

(Or the `bt`-prefixed aliases if running interactively: `btrb`, `btc`.)

After all 5 commits land, run the full verification gate (cargo gates + grep guards + manual smoke), then push:

```bash
but push -u origin <branch-name>     # btp alias — runs husky hooks
gh pr create ...                     # raw gh — GitButler doesn't open PRs
```
