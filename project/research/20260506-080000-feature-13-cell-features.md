# Feature #13 — Cell Features (Doors, Traps, Teleporters, Spinners) — Research

**Researched:** 2026-05-06
**Domain:** Druum / Bevy 0.18.1 / DRPG cell-feature reaction layer
**Confidence:** HIGH on the live ground truth (every dependency #4 / #5 / #6 / #7 / #8 / #9 / #10 / #11 / #12 was read in full at file:line); HIGH on the locked-by-roadmap UX decisions (Spinner = telegraphed, Resolved §4); HIGH on Bevy 0.18 message/system patterns (every pattern reused below has a working precedent in `MovedEvent` + minimap.rs subscriber). MEDIUM on the cross-floor teleporter mechanism (D3) — the recommendation is technically straightforward but has a UX consequence (loading-screen flash) the user may want to reconsider. MEDIUM on the screen-wobble approach (D6) — pure-Transform jitter is the cheapest path but no shake helper exists in the project today, so a small helper system is required.

---

## Summary

Feature #13 lands on top of an already-rich foundation. **Five of the six "data" pieces #13 needs already exist:** `WallType::Door`/`LockedDoor`, `WallType::Open`, `TrapType::{Pit, Poison, Alarm, Teleport}`, `TeleportTarget`, `CellFeatures.{spinner, dark_zone, anti_magic_zone, encounter_rate, event_id}`, and `Direction` are all defined in `src/data/dungeon.rs:85-174` (verified). `DungeonAction::Interact` is bound to `KeyCode::KeyF` in `src/plugins/input/mod.rs:78, 149`. `MovedEvent { from, to, facing }` derives `Message` and is published on the commit frame at `src/plugins/dungeon/mod.rs:686-690`. `ItemKind::KeyItem` ships in #12 (`src/plugins/party/inventory.rs:79`); `ItemHandleRegistry::get(&str) -> Option<&Handle<ItemAsset>>` provides ID→handle lookup (`inventory.rs:508-510`). And critically, `floor_01.dungeon.ron` was authored *to test #13 specifically* — it already includes a `Door` at the (1,1)/(2,1) edge, a `LockedDoor` at (3,1)/(4,1), a `spinner` at (2,2), a `Pit` trap at (4,4), a `dark_zone` at (1,4), an `anti_magic_zone` at (2,4), and a cross-floor `Teleporter` at (5,4). **#13 does not need to author a new floor file** — it consumes the one that's been waiting since #4.

The roadmap's ten-bullet "Broad Todo List" maps onto **one new file** (`src/plugins/dungeon/features.rs`, the file the roadmap names at line 707), **two new SFX variants** (`SfxKind::DoorClose` and `SfxKind::SpinnerWhoosh` — `Door` already exists for "open creak" and "trap snap" generic), **one new `EncounterRequested` Message** (so the `Alarm` trap has a deterministic stub destination), **one new `DoorState` resource** (per-floor-instance HashMap keyed by `(GridPosition, Direction)`), **one new `AntiMagicZone` component** (added on enter, removed on exit) plus minor edits to: `DungeonPlugin::build` (+register message + add 6 systems), `audio/sfx.rs` (+2 SfxKind variants + match arms), `assets/audio/sfx/` (+2 .ogg files), `loading/mod.rs` (+2 SFX handle fields). Net: **0 new dependencies**, +400-600 LOC, +6-9 tests, +2 small audio assets.

The five major architectural decisions (D1 door persistence, D2 key-item representation, D3 cross-floor teleporter, D4 anti-magic/dark-zone scope, D6 screen wobble) all have **strong default recommendations** grounded in existing project patterns. The only one with material UX consequence is D3 — re-entering `GameState::Loading` for cross-floor teleport produces a brief loading flash; the alternative (in-state asset swap) avoids the flash but skips the state-machine guarantees `LoadingPlugin` enforces.

**Primary recommendation:** Implement #13 as a single-file `src/plugins/dungeon/features.rs` containing six `MovedEvent`-driven systems plus an `Interact`-driven door system, all gated `.run_if(in_state(GameState::Dungeon))` and ordered `.after(handle_dungeon_input)`. Use Option α for cross-floor teleport (re-enter `Loading` via a new `TeleportRequested` message that LoadingPlugin owns the consumer for — this is the ONE small carve-out into the otherwise-frozen LoadingPlugin). Use a `key_id: Option<String>` field on `ItemAsset` paired with `ItemKind::KeyItem` for locked doors. Use a `DoorStates: Resource` HashMap keyed by `(GridPosition, Direction)` for per-floor door open/close state (cleared on `OnExit(Dungeon)`). Use a `AntiMagicZone` marker component on `PlayerParty` (added on enter, removed on exit). **Zero new dependencies.**

---

## Live ground truth (the implementer must mirror these)

These are the load-bearing facts from the merged code that contradict or refine the roadmap. Read these before designing anything.

### A. `WallType::Door` / `WallType::LockedDoor` ARE distinct from `CellFeatures` — doors live in `walls`, traps live in `features`

**File:** `/Users/nousunio/Repos/Learnings/claude-code/druum/src/data/dungeon.rs:85-104, 156-174`

```rust
pub enum WallType {
    Open, Solid, Door, LockedDoor, SecretWall, OneWay, Illusory,
}

pub struct CellFeatures {
    pub trap: Option<TrapType>,
    pub teleporter: Option<TeleportTarget>,
    pub spinner: bool,
    pub dark_zone: bool,
    pub anti_magic_zone: bool,
    pub encounter_rate: f32,
    pub event_id: Option<String>,
}
```

The split is **deliberate** (the file-header doc comment at lines 1-8 states: *"no `CellFeatures::Door` variant (doors live in `WallType`)"*). Doors are *edges*; traps and teleporters are *cells*. **#13 must respect this** — the Door interaction system queries by `(GridPosition, Direction)` (an edge), not by cell.

`can_move` truth table at `dungeon.rs:79-83`:
- `Door` is currently passable (`true`) — the asset-level "closed but unlocked" reads as already-passable. **#13 changes this.** The roadmap line 730 says the `Interact` key toggles `Door` open/closed, which means after `#13`, a **runtime override** sits on top of the static asset. See Decision D9 + D1 below.
- `LockedDoor` is currently impassable (`false`). **#13 changes this conditionally** — if the party has a key item with matching `key_id`, attempting to move into a `LockedDoor` cell unlocks it (and reads as passable). See D2.

### B. `TrapType` and `TeleportTarget` are FULLY DEFINED and serializable

**File:** `dungeon.rs:123-149`

```rust
pub struct TeleportTarget {
    pub floor: u32,
    pub x: u32, pub y: u32,
    pub facing: Option<Direction>,  // None = retain current facing
}

pub enum TrapType {
    Pit { damage: u32, target_floor: Option<u32> },
    Poison,
    Alarm,
    Teleport(TeleportTarget),  // Note: trap.Teleport REUSES TeleportTarget
}
```

**Critical observation:** `TrapType::Teleport(TeleportTarget)` and `CellFeatures::teleporter: Option<TeleportTarget>` are **two different feature shapes that share the same payload type**. A cell can have both a teleporter (always-on) AND a trap (one-shot). The `apply_teleporter` system can read whichever is present; the same helper does the work for both. The roadmap line 728 mentions "Implement `apply_teleporter`" — that single system handles both invocation paths.

`TrapType::Pit { target_floor: None }` is "dead-end pit" semantics (per the doc comment at `dungeon.rs:140-141`). Implementer choice: damage-only (no floor change) is the trivial implementation. Damage + drop-to-floor uses the same cross-floor mechanism as Teleporter — share the helper.

### C. `MovedEvent { from, to, facing }` is the established subscription point — same-frame readable

**File:** `/Users/nousunio/Repos/Learnings/claude-code/druum/src/plugins/dungeon/mod.rs:192-197, 207, 686-690`

```rust
#[derive(Message, Clone, Copy, Debug)]
pub struct MovedEvent {
    pub from: GridPosition,
    pub to: GridPosition,
    pub facing: Direction,
}
// ...
moved.write(MovedEvent {
    from: old_pos,
    to: *pos,
    facing: facing.0,
});
```

**Established subscriber pattern** at `src/plugins/ui/minimap.rs:122-125`:
```rust
update_explored_on_move
    .run_if(in_state(GameState::Dungeon))
    .after(handle_dungeon_input)
```

The `.after(handle_dungeon_input)` ordering is the key — it ensures the `MovedEvent` is in the `Messages<MovedEvent>` queue when the subscriber reads. **All six #13 `MovedEvent`-driven systems must mirror this pattern**: `.run_if(in_state(GameState::Dungeon))` AND `.after(handle_dungeon_input)`.

`pub(crate) fn handle_dungeon_input` is exposed at `dungeon/mod.rs:618` specifically for this kind of `.after(...)` ordering — the doc comment at lines 614-616 explicitly names #13's use case.

**Important Δ from roadmap:** the roadmap line 694 says *"Each feature subscribes to `MovedEvent` (or runs `OnEnter` of the cell)"*. Use `MovedEvent` exclusively. There is no per-cell `OnEnter` schedule in Bevy — that would require a per-(GridPosition, x, y) state. Don't introduce one.

### D. `MovedEvent` is published BEFORE the visual tween completes — the new logical state IS the commit frame

**File:** `dungeon/mod.rs:30-34`

> *"`GridPosition` and `Facing` update **immediately** on input-commit (same frame). `MovementAnimation` then lerps the visual `Transform` over the tween duration. Downstream consumers (#13 cell-trigger, #16 encounter) react to the new logical state on the commit frame, not after the tween completes."*

This is **exactly** what #13 needs. Trap damage applies on the commit frame (the moment the player enters the cell), not after the 0.18s movement animation finishes. Spinner randomizes facing on the commit frame; the minimap (which queries `&Facing` directly — `minimap.rs:269, 309`) reflects the new facing the same frame.

### E. `ItemKind::KeyItem` exists; `Inventory(Vec<Entity>)` carries `ItemInstance(Handle<ItemAsset>)`

**Files:**
- `/Users/nousunio/Repos/Learnings/claude-code/druum/src/plugins/party/inventory.rs:60-80, 167-187`
- `/Users/nousunio/Repos/Learnings/claude-code/druum/src/data/items.rs:84-113`

```rust
// inventory.rs:60-80
pub enum ItemKind {
    Weapon, Shield, Armor, Helm, Gloves, Boots, Accessory,
    Consumable, KeyItem,  // <-- already exists
}

// inventory.rs:167-187
pub struct ItemInstance(pub Handle<ItemAsset>);
pub struct Inventory(pub Vec<Entity>);
```

`ItemAsset` ships with **9 fields** (verified `items.rs:84-113`): `id`, `display_name`, `stats`, `kind`, `slot`, `weight`, `value`, `icon_path`, `stackable`. **There is no `key_id` field yet.** D2 below recommends adding it.

`ItemHandleRegistry` (`inventory.rs:500-521`) provides `id: &str -> Option<&Handle<ItemAsset>>` lookup — this is how a quest-reward system gives a key by ID. **#13 can use it but does not need to** — the locked-door system reads `Inventory(Vec<Entity>)` → each entity's `ItemInstance(Handle)` → looks up the `ItemAsset` → checks if `kind == KeyItem` and `key_id` matches.

**`rusty_key` ALREADY EXISTS in `assets/items/core.items.ron`:** `id: "rusty_key", kind: KeyItem, slot: None`. It does not currently have a `key_id` field — adding the field will require a corresponding edit to `core.items.ron` to give the rusty key a `key_id` (e.g., `key_id: Some("rusty_door_01")`). #13 owns that edit.

### F. `DungeonAction::Interact` IS bound to `KeyCode::KeyF` — no input changes needed

**File:** `/Users/nousunio/Repos/Learnings/claude-code/druum/src/plugins/input/mod.rs:78, 149`

```rust
pub enum DungeonAction {
    // ...
    Interact,  // line 78
    // ...
}

// line 149:
.with(Interact, KeyCode::KeyF) // F (NOT Space, NOT E) — avoids TurnRight=E conflict
```

**`#13` does NOT need to add input.** The variant exists, is bound, and is registered. `ActionsPlugin` in main.rs is already wiring the full chain. The Door interaction system reads `Res<ActionState<DungeonAction>>` and checks `actions.just_pressed(&DungeonAction::Interact)`.

### G. SFX path: `SfxRequest { kind: SfxKind }` is the consumer; `SfxKind` has 5 variants, missing 2 we need

**File:** `/Users/nousunio/Repos/Learnings/claude-code/druum/src/plugins/audio/sfx.rs:42-57`

```rust
#[derive(Message, Clone, Copy, Debug)]
pub struct SfxRequest { pub kind: SfxKind }

pub enum SfxKind {
    Footstep,           // wired
    Door,               // wired (one variant — used for both open and close, OR add DoorClose)
    EncounterSting,     // wired
    MenuClick,          // wired
    AttackHit,          // wired
}
```

`AudioAssets` at `loading/mod.rs:50-76` has the matching 5 .ogg path fields:
```rust
sfx_door: Handle<AudioSource>,        // assets/audio/sfx/door.ogg — exists
// (others)
```

**Δ for #13:** Add `SpinnerWhoosh` (mandatory — Resolved §4 demands it), and either (a) reuse `Door` for both open & close OR (b) add `DoorClose`. Adding `DoorClose` is cleaner (different asset; the player's audio cue distinguishes open from close). For traps, `AttackHit` could reuse for `Pit` damage; `Door` (a "creak/snap" sound) reasonably maps to "trap snap"; explicit `TrapTrigger` is cleanest. Surface as Decision D10. **Recommended: add 2 variants — `SpinnerWhoosh` and `DoorClose` — and reuse `Door` for door-open + trap-snap, `AttackHit` for pit damage.** This keeps audio Δ to exactly 2 .ogg files (matches roadmap "+2-4 door textures, trap SFX" budget at line 720).

**Important:** adding any `SfxKind` variant requires:
1. Edit `sfx.rs:51-57` (enum) AND `sfx.rs:78-84` (match arm) — the compiler enforces the latter.
2. Edit `loading/mod.rs:50-76` to add the corresponding `Handle<AudioSource>` field.
3. Add the .ogg file under `assets/audio/sfx/`.

**This requires touching `loading/mod.rs`, which has been "FROZEN post-#3" per project memory.** This is the only #13 carve-out into LoadingPlugin — and only if D10 is "add 2 SFX variants". The freeze is documented as "do not touch [FROZEN] without explicit reason"; adding `AudioAsset` fields is the same surgery the project performed in #6 on the audio path. **The "frozen" status applies to LoadingPlugin's *state-transition logic*, not to the AudioAssets field list** — verified by reading `loading/mod.rs:50-76` (the 5 SFX fields were added in #6, after the freeze comment was authored).

### H. Live `floor_01.dungeon.ron` is already a #13 testbed — no new floor authoring required

**File:** `/Users/nousunio/Repos/Learnings/claude-code/druum/assets/dungeons/floor_01.dungeon.ron:13-29`

The asset's own header comment names the variant coverage:
- `WallType::Door` at east of (1,1) / west of (2,1)
- `WallType::LockedDoor` at east of (3,1) / west of (4,1)
- `CellFeatures::spinner` at (2,2)
- `CellFeatures::trap` at (4,4) = `Pit { damage:5, target_floor: Some(2) }`
- `CellFeatures::teleporter` at (5,4) → floor 2, (1,1), facing South
- `CellFeatures::dark_zone` at (1,4)
- `CellFeatures::anti_magic_zone` at (2,4)

**This is a designed-for-#13 floor.** Authored by #4 (verified `dungeon.rs:790-809` integration test loads it and asserts the shape). #13's testing surface is "walk into each of these cells and verify the system fires". **Caveat:** the teleporter target is `floor 2`, but `floor_02.dungeon.ron` does not exist (only `floor_01` is in `DungeonAssets` at `loading/mod.rs:31-32`). For cross-floor testing, the implementer either (a) authors `floor_02.dungeon.ron` (small RON file, ~80 lines), (b) targets a different floor in a test-only override, or (c) defers cross-floor end-to-end test to manual smoke. Surface as Decision D11.

### I. `MovedEvent` consumer count grows from 1 to 7 — system ordering matters

Pre-#13: only `update_explored_on_move` (`minimap.rs:192-217`) reads `MovedEvent`.
Post-#13: 7 readers (one per cell-feature: door-already-open detection, pit, poison, alarm, teleport-cell, spinner, plus the new dark-zone gate is on the EXISTING minimap subscriber — verified in code already at `minimap.rs:208-211`).

**Bevy `Messages<T>` allows multiple readers** — each reader has its own cursor. No new pattern needed. All 7 systems can run in parallel (no shared mutable state between them — pit only mutates HP, spinner only mutates Facing, teleporter only fires `TeleportRequested`).

**Ordering:** all 7 must be `.after(handle_dungeon_input)` AND `.run_if(in_state(GameState::Dungeon))`. They do NOT need to be ordered relative to each other (different cells = different consumer paths, and a single MovedEvent only triggers one cell's features anyway).

### J. `PartyMember` query path — damage applies to all 4 party members on Pit

**File:** `/Users/nousunio/Repos/Learnings/claude-code/druum/src/plugins/party/character.rs:78-81, 132-145`

`Pit { damage: 5 }` — does it apply to one party member or all four? Wizardry/Etrian convention: **all party members take damage from environmental hazards**. The roadmap line 725 says *"apply damage"* (singular, ambiguous). **Recommended: apply to all `PartyMember` entities** via `Query<&mut DerivedStats, With<PartyMember>>`. The clamp `current_hp = current_hp.saturating_sub(damage)` follows the project's saturating-arithmetic pattern (`character.rs:374-381`).

### K. Status effects: `StatusEffectType::Poison` exists; `StatusEffects` has `effects: Vec<ActiveEffect>` and `has(kind)` method

**File:** `character.rs:235-274`

```rust
pub enum StatusEffectType { Poison, Sleep, Paralysis, Stone, Dead }
pub struct ActiveEffect { effect_type, remaining_turns: Option<u32>, magnitude: f32 }
pub struct StatusEffects { pub effects: Vec<ActiveEffect> }
impl StatusEffects { pub fn has(&self, kind) -> bool { ... } }
```

**For #13 poison trap:** `effects.push(ActiveEffect { effect_type: Poison, remaining_turns: Some(<turns>), magnitude: 0.0 })` for each affected `PartyMember`. The roadmap line 726 says *"apply `StatusEffect::Poison` to the party"*. Per #14's own roadmap (lines 776-782), Poison "ticks per turn in #15's combat turn system" — meaning #13 *applies* the effect; #14 / #15 *resolve* the per-turn damage. **#13's responsibility is purely to push the effect onto the Vec.** Idempotence question: should pushing a second `Poison` extend duration / refresh / stack? **Roadmap §14 line 781** punts ("Document stacking rules") — for #13 v1, just push (mirror the same naive behavior the roadmap §14 will replace). Surface as Decision D12.

`remaining_turns` value: pick a default (e.g., 5). Designer-tunable; surface as a ron field on `TrapType::Poison { turns: u32 }` later. For v1 the trap is just `TrapType::Poison` (no payload — `dungeon.rs:144`), so the duration is hardcoded. **Recommended: 5 turns.**

### L. Existing minimap dark-zone gate — already implemented at `minimap.rs:208-211`

**File:** `/Users/nousunio/Repos/Learnings/claude-code/druum/src/plugins/ui/minimap.rs:206-216`

```rust
for ev in moved.read() {
    let x = ev.to.x as usize;
    let y = ev.to.y as usize;
    if floor.features[y][x].dark_zone {
        // Dark-zone: skip insert. Cell stays Unseen; painter renders `?`.
        continue;
    }
    explored.cells.insert(...);
}
```

**The dark-zone gate is ALREADY DONE in #10.** #13 does NOT need to add anything for dark zones. The roadmap line "implement dark zones" is **already-implemented work**. The only #13-shaped piece left is "anti-magic zones", which has no consumer yet (#14/#15 not built) — see D4.

The minimap also has a `?`-glyph painter for unseen-and-dark cells at `minimap.rs:397-406`. **Confirmed working** in the integration test at `minimap.rs:639-691`.

### M. Stale-roadmap summary

| Roadmap claim (line) | Reality |
|---------------------|---------|
| Line 707: "`src/plugins/dungeon/features.rs` containing all cell-feature systems" | True — file does not yet exist; #13 creates it. |
| Line 708: "New `KeyItem` flag on `Item` (or a tag component)" | Partly true — `ItemKind::KeyItem` already exists (`inventory.rs:79`). What's NEEDED is a `key_id: Option<String>` field on `ItemAsset` so locked doors know which key opens which door. |
| Line 709: "`MovedEvent` subscribers across multiple feature systems" | True — 6 new readers; pattern is established (`.after(handle_dungeon_input)`). |
| Line 710: "`floor_01.dungeon.ron` augmented with one of each feature" | **FALSE — already done.** Floor was authored for #13 in #4. The teleporter targets floor 2 which does not exist; `floor_02.dungeon.ron` may need adding for cross-floor testing (D11). |
| Line 711: "A `TeleportRequested` event that triggers a state-managed floor transition" | True — needs to be defined and registered. The implementer also needs to wire its consumer into `LoadingPlugin` for cross-floor (Option α) — small carve-out into otherwise-frozen plugin. |
| Line 730: "pressing `DungeonAction::Interact` against a `WallType::Door` cell pair toggles open/closed" | True. Note: `Door` is currently passable per `can_move`; this means even before #13, the player can walk through a `Door` without "opening" it. Toggling adds a runtime state layer that **gates passability based on `DoorState`**. See D9. |
| Line 731: "Implement locked-door check (consumes / requires a `KeyItem` from `Inventory`)" | True. **"Consumes" is a designer choice** — Wizardry-style: keys are NOT consumed (they reusable across the dungeon). The roadmap's "consumes / requires" hedge mirrors this: pick one. **Recommended: NOT consumed in v1.** Surface as Decision D13. |
| Line 732: "SFX (door creak, trap snap) routed through #6" | Partly true — needs +2 SfxKind variants AND +2 .ogg files (D10). |
| Line 734: "spinner asset trio: tile texture, whoosh SFX, screen-wobble shader / camera shake" | Partly true. (a) Tile texture: spinner cell has no current visual marker — see D9b for how to render it. (b) Whoosh SFX: D10. (c) Screen wobble: shader OR camera shake — recommend camera shake, see D6. |
| Line 720 asset Δ "+2-4 door textures, trap SFX" | Door textures already exist as colored materials in `dungeon/mod.rs:480-487` (`Color::srgb(0.45, 0.30, 0.15)` brown for Door, `Color::srgb(0.55, 0.20, 0.15)` red for LockedDoor). **No texture work required for door open/close** unless we want a visible "open door looks different from closed door" — see D9. |
| Line 728: "state transition: despawn current floor, load new floor, set new `GridPosition`" | True for cross-floor; same-floor is just a position+facing mutation. See D3. |

---

## Standard Stack

### Core (already in deps — no Δ)

| Library | Version | Purpose | License | Maintained? | Why Standard |
|---------|---------|---------|---------|-------------|--------------|
| [bevy](https://crates.io/crates/bevy) | =0.18.1 | `Component`, `Message`, `Resource`, `Plugin` | MIT/Apache-2.0 | Active | Engine — pinned; no bump permitted. |
| [leafwing-input-manager](https://crates.io/crates/leafwing-input-manager) | =0.20.0 | `Res<ActionState<DungeonAction>>` for door Interact key | ISC | Active | Already wired by #5. |
| [bevy_asset_loader](https://crates.io/crates/bevy_asset_loader) | =0.26.0 | `AudioAssets.sfx_*` handle slots; will receive 2 new fields. | Apache-2.0 | Active | Already wired. |
| [bevy_common_assets](https://crates.io/crates/bevy_common_assets) | =0.16.0 | `RonAssetPlugin::<DungeonFloor>` already loads floor RON. | MIT/Apache-2.0 | Active | Already wired. |
| [serde](https://crates.io/crates/serde) | 1 | `Serialize`/`Deserialize` for new `key_id: Option<String>` field on `ItemAsset` | MIT/Apache-2.0 | Active | Already in deps. |
| [ron](https://crates.io/crates/ron) | 0.12 | Stdlib RON round-trip for new schema fields. | MIT/Apache-2.0 | Active | Already wired. |
| [rand](https://crates.io/crates/rand) | (transitive via bevy) | Spinner: pick a random `Direction`. | MIT/Apache-2.0 | Active | Bevy 0.18 transitively includes `rand`; verified in #15 roadmap line 819. **Confirm via `cargo tree`** before relying — if absent, `fastrand` is the typical alternative; OR use `bevy::math::Vec3::lerp` with a deterministic counter as fallback. **Surface as Decision D14.** |

**`rand` availability check:** the project does not currently declare `rand` directly in Cargo.toml. **Recommended verification before plan-stage:** run `cargo tree -i rand` from project root. If `rand` is transitively present (highly likely via `bevy_audio` / `bevy_pbr`), use it directly. If not, the cleanest path is to derive a random `Direction` from `Time::elapsed_secs_f64() as u64 % 4` — deterministic but acceptable for v1 spinner UX. Add `rand = "0.8"` to Cargo.toml only if both fallbacks are rejected (Δ deps = 1 — surface as D14 cost).

### Supporting (NOT used in #13)

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| [bevy_kira_audio](https://crates.io/crates/bevy_kira_audio) | (rejected #6) | Multi-channel audio | Already rejected in favor of built-in `bevy_audio`. SFX through existing `SfxRequest` Message pattern. |
| [bevy_egui](https://crates.io/crates/bevy_egui) | =0.39.1 | UI for inventory selection on locked doors | Feature #25. v1 uses automatic-key-lookup, no UI prompt. |
| Custom shader for screen wobble | (declined) | Post-process shake effect | Camera-shake via `Transform::rotation` jitter is cheaper and matches existing `MovementAnimation` pattern. See D6. |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|-----------|-----------|----------|
| `DoorStates: Resource(HashMap<(GridPos, Direction), bool>)` | Per-edge component on the wall geometry entity | Components are despawned on `OnExit(Dungeon)` (good — door state should NOT persist across floor change). Resource needs explicit clear on exit. **But:** finding the wall entity for a given `(GridPos, Direction)` requires a query scan; the resource is O(1) lookup. **Recommended: Resource.** Surface as D9. |
| Resource cleared on `OnExit(GameState::Dungeon)` | Resource cleared on `OnEnter(GameState::Dungeon)` | Symmetrical. `OnExit` matches the `despawn_dungeon_entities` pattern at `dungeon/mod.rs:410-425`. **Recommended: OnExit.** |
| Separate `EncounterRequested` Message published by Alarm trap | Alarm trap directly transitions to `GameState::Combat` | The Combat state machinery is deferred to #15. Publishing a Message lets #15 wire its own consumer. **Recommended: publish Message, log-only consumer in v1.** |
| `AntiMagicZone` marker component on `PlayerParty` | Resource bool flag on a singleton | Component scales naturally if multi-cell anti-magic regions ever exist. Adding/removing on `MovedEvent` is the same primitive as `MovementAnimation`'s lifecycle. **Recommended: component.** |
| Camera shake via `Transform::rotation` jitter | Camera shake via `Transform::translation` displacement | Rotation jitter feels like vertigo (genre-correct for "the world spins around you"). Translation jitter is "ground rumbles". **Recommended: rotation jitter for spinner specifically.** |
| `key_id: Option<String>` on `ItemAsset` | New `KeyDb: Asset { keys: Vec<KeyDef> }` resource separate from items | Items as a pool already exist; adding one optional field is byte-cheaper than a new asset type + RonAssetPlugin registration. **Recommended: extend `ItemAsset`.** |

**Installation:** No new dependencies (assumed). If `cargo tree -i rand` returns empty, +1 dep — D14 ratifies the cost. **Cargo.toml is byte-unchanged** in the recommended path.

---

## Architecture Options

Three fundamentally different ways to structure the cross-floor teleport. The other features (door, trap, spinner) have a single sensible architecture; the cross-floor teleport has real options.

### D3: Cross-floor teleporter implementation

| Option | Description | Pros | Cons | Best When |
|--------|-------------|------|------|-----------|
| **α: Re-enter `GameState::Loading` via `TeleportRequested` Message** [RECOMMENDED] | New `TeleportRequested { target: TeleportTarget }` Message. Teleporter system writes it on `MovedEvent` to a teleporter cell. New `handle_teleport_request` system in `LoadingPlugin` reads it, sets `NextState(GameState::Loading)`, and stashes the destination in a small `PendingTeleport: Resource(TeleportTarget)` resource. On entering Loading, the existing asset-loading machinery loads the new floor (or finds it cached); on exit to Dungeon, `spawn_party_and_camera` reads `PendingTeleport` if present and spawns the player at that destination instead of `floor.entry_point`. | Clean state-machine path. The `Loading -> Dungeon` re-entry already runs `OnEnter(Dungeon)` (`dungeon/mod.rs:209-211`) which is `spawn_party_and_camera + spawn_dungeon_geometry`; both are despawn-recursive on `OnExit(Dungeon)`. **Reuses 100% of existing spawn machinery.** Save/load (#23) gets a cleaner integration point ("we save the destination + load") because the state-machine boundary is the natural save point. Brief loading-screen flash is **genre-correct** (Wizardry/Etrian show floor-transition animations). | **Touches `LoadingPlugin`** — adds `handle_teleport_request` system + `PendingTeleport` resource. The "FROZEN post-#3" memory note explicitly cautions against this; this is a small, justified carve-out. Multi-frame transition: roughly 2-3 frames of black-screen flash on a fast machine. | This is the genre-canonical choice. Recommended unless the user explicitly rejects loading-flash UX. |
| β: In-state asset swap | Stay in `Dungeon`. Manually `commands.entity().despawn()` for `Query<Entity, With<DungeonGeometry>>` and the `PlayerParty` root, then re-call `spawn_party_and_camera` and `spawn_dungeon_geometry` with the new floor handle. Update `DungeonAssets.floor_01` to point to the target floor (or store a `Handle<DungeonFloor>` per-active in a separate resource). | No state transition; visually smoother (no loading flash). | (a) Bypasses `bevy_asset_loader`'s loading guarantees — if the target floor's RON isn't loaded yet, the new geometry spawn silently fails. (b) `spawn_party_and_camera` and `spawn_dungeon_geometry` were designed for `OnEnter(Dungeon)` invocation; calling them from a regular Update system is novel territory (refactor required). (c) Doesn't compose with #23 save/load — the natural save point IS the state boundary. (d) `DungeonAssets.floor_01` is `pub` but mutating it mid-game smells wrong; introduces a "current floor handle" abstraction. | Only if (a) the loading-flash UX is rejected, AND (b) the user accepts ~80 LOC of refactor in `dungeon/mod.rs` to factor out the spawn helpers. Not recommended. |
| γ: Hybrid — `Loading` re-enter ONLY if target floor not yet loaded; otherwise in-state swap | Best of both. | (a) Same code-complexity hit as β. (b) Branching path doubles test surface. (c) Requires runtime introspection: `is the target floor's Handle::LoadedWithDependencies?` Bevy 0.18 has `Assets::contains` but that doesn't distinguish "loaded" from "loading". | Not recommended. The loading-flash is genre-correct; optimization is post-v1. |

**Recommended: α — re-enter `GameState::Loading`.** It's the smallest code change, leverages existing despawn-recursive cleanup, composes with future #23, and the "loading flash" is genre-appropriate.

**The carve-out into LoadingPlugin is small:**
1. `pub mod features` in `dungeon/` declares `TeleportRequested: Message`.
2. `LoadingPlugin::build` adds `app.add_message::<TeleportRequested>()` and `app.init_resource::<PendingTeleport>()`.
3. `LoadingPlugin::build` adds `Update` system `handle_teleport_request` (reads `TeleportRequested`, sets `NextState(GameState::Loading)`, populates `PendingTeleport`).
4. `dungeon/mod.rs::spawn_party_and_camera` reads `Option<Res<PendingTeleport>>` — if present, uses its `(x, y, facing)` instead of `floor.entry_point`. Then clears the resource (or removes via `commands.remove_resource`).

**Same-floor teleporter** (a teleporter cell whose `target.floor == current_floor`) is much simpler: mutate `GridPosition` and `Facing` in place, write a `MovedEvent` (so minimap reflects the new position), no state transition. The teleporter system branches on whether `target.floor == current_floor`.

### D9: Door state component shape

| Option | Description | Pros | Cons | Best When |
|--------|-------------|------|------|-----------|
| **A: `DoorStates: Resource(HashMap<(GridPosition, Direction), DoorState>)`** [RECOMMENDED] | One resource holding all door states for the current floor, keyed by edge `(grid_position, direction_to_other_cell)`. `enum DoorState { Closed, Open, Locked, Unlocked }` (Closed/Open for `Door`; Locked/Unlocked tracks whether the player has used a key). Cleared on `OnExit(GameState::Dungeon)`. | O(1) lookup per `(GridPos, Direction)`. Resource lifecycle matches floor lifecycle (cleared on exit, re-populated on enter). One source of truth — no walking entity queries. Easy to test (insert/get/contains). | Decoupled from the visual wall-plate entity. Visual update on door-open requires a separate system that re-reads `DoorStates` and mutates the wall material on the matching `DungeonGeometry` entity. | Recommended. The "decoupled visual" cost is small and gets cleaner separation between data and rendering. |
| B: `DoorState { open: bool }` component on the per-edge wall plate entity | Visual + data co-located on the same entity. | Requires storing `(GridPosition, Direction, edge)` on each wall plate so we can find the right one for a given `Interact` press. Currently wall plates only have `DungeonGeometry` marker (no edge metadata at `dungeon/mod.rs:520-573`). Adding metadata = touching frozen `dungeon/mod.rs` more aggressively than D9-A. | None for v1. |
| C: Mutate `DungeonFloor::walls` at runtime (e.g., `Door` ↔ `Open`) | Trivial — same data path as `can_move`. | **DO NOT DO THIS.** `DungeonFloor` is loaded as `Res<Assets<DungeonFloor>>` — mutating it would corrupt the asset for any other reader, and on hot-reload (`bevy/file_watcher`), the change reverts. Read-only by contract. | Never. |

**Recommended: A — `DoorStates: Resource`.**

```rust
#[derive(Default, Debug, Clone, PartialEq, Eq)]
pub enum DoorState {
    #[default]
    Closed,    // walking through is blocked; Interact toggles to Open
    Open,      // walking through is allowed; Interact toggles to Closed
    Unlocked,  // formerly LockedDoor, now treated as Open after key consumed (or Closed depending on D13)
}

#[derive(Resource, Default)]
pub struct DoorStates {
    pub doors: HashMap<(GridPosition, Direction), DoorState>,
}
```

`can_move` integration: a wrapper helper `can_move_with_doors(floor, doors, x, y, dir) -> bool` that returns `floor.can_move(x, y, dir) && match doors.get((pos, dir)) { Closed | None for Door | Locked for LockedDoor => false, _ => true }`. **`handle_dungeon_input` would need to call this wrapper instead of `floor.can_move`** — a small `dungeon/mod.rs` edit. Surface as D9b.

### D2: KeyItem representation

| Option | Description | Pros | Cons | Best When |
|--------|-------------|------|------|-----------|
| **A: `key_id: Option<String>` on `ItemAsset` paired with `ItemKind::KeyItem`** [RECOMMENDED] | Add one optional field to `ItemAsset`. RON: `key_id: Some("rusty_door_01")`. LockedDoor unlock check: walk the party's combined `Inventory(Vec<Entity>)`, for each `ItemInstance(handle)`, look up the `ItemAsset`, check if `kind == KeyItem` AND `key_id == Some(door_id)`. Door's id is per-edge metadata (or per-LockedDoor entry on a `LockedDoors: Resource`). | Extends existing schema (additive, `#[serde(default)]`). RON authoring is local to the item. Multiple keys can share a `key_id` (a "master key"). | (1) Requires `core.items.ron` edit to the existing `rusty_key` to add the `key_id` field. (2) Requires lookup of "what's the door's id" — resolved by per-LockedDoor `door_id: Option<String>` either on `WallType::LockedDoor(String)` (changes WallType variant — heavy) OR on a separate `LockedDoors: Resource(HashMap<(GridPos, Direction), String>)` populated from a new field on `DungeonFloor`. The Resource path is recommended. |
| B: `KeyItem` tag component on the item entity | Spawn `(ItemInstance(handle), KeyItem(door_id: String))` for each key item entity. | Doesn't extend `ItemAsset` schema. | Per-instance state on a key entity that *should* be intrinsic to the asset. Loses the round-trip schema property. **Awkward ergonomically.** |
| C: Reuse `ItemKind::KeyItem` only; locked door always opens with ANY key item | Trivial — no new field. | Wrong UX. A "rusty key" should not open a "vault door". Genre-incorrect. |

**Recommended: A.** The `key_id` field is `Option<String>` (`#[serde(default)]`) so existing items don't need editing — only `rusty_key` gets `key_id: Some("rusty_door_01")`. The existing 8 items become 9 fields; round-trip test extension is trivial.

**Door-side:** add an optional `door_id: Option<String>` field to `DungeonFloor`. A clean way is a parallel resource `LockedDoors: Resource(HashMap<(GridPos, Direction), String>)` populated on `OnEnter(Dungeon)` from a new `locked_door_ids: HashMap<(u32, u32, Direction), String>` field on `DungeonFloor`. Or simpler: a `Vec<((u32, u32), Direction, String)>` on `DungeonFloor`. Keep it on `DungeonFloor` for asset-driven authoring (no separate file). **Surface as D2b: where does door_id live? On `DungeonFloor` as a side-table, OR on `WallType::LockedDoor(String)` (changes the variant).** Recommended: `DungeonFloor` side-table.

For floor_01, since the LockedDoor exists at edge (3,1)/(4,1) East:
```ron
locked_doors: [
    ((3, 1), East, "rusty_door_01"),
],
```

This requires a `dungeon.rs` schema extension. It's `Vec<(...)>` — additive, `#[serde(default)]`, default empty. Existing tests would still pass. **This DOES require touching `data/dungeon.rs`** which has been in active service since #4. The edit is additive and `#[serde(default)]`, but #4's "is_well_formed" check would need updating. Surface as D2c (small but real).

---

## Architecture Patterns

### Recommended Project Structure

```
src/
├── data/
│   └── dungeon.rs                  # MODIFY — add `locked_doors: Vec<((u32,u32), Direction, String)>` field on DungeonFloor (D2c)
│   └── items.rs                    # MODIFY — add `key_id: Option<String>` field on ItemAsset (D2)
├── plugins/
│   └── dungeon/
│       ├── mod.rs                  # MODIFY — register message TeleportRequested + EncounterRequested; replace floor.can_move with can_move_with_doors wrapper; add `pub mod features;`
│       └── features.rs             # NEW (~400-600 LOC) — all 7 cell-feature systems + DoorStates resource + LockedDoors resource + AntiMagicZone marker + tests
│   └── audio/
│       └── sfx.rs                  # MODIFY — add SfxKind::SpinnerWhoosh, SfxKind::DoorClose, match arms (D10)
│   └── loading/
│       └── mod.rs                  # MODIFY — add 2 sfx_* fields on AudioAssets; add `handle_teleport_request` system + `PendingTeleport: Resource` (D3)
│       └── (assets)
└── ...

assets/
├── audio/sfx/
│   ├── spinner_whoosh.ogg          # NEW — placeholder .ogg (D10)
│   └── door_close.ogg              # NEW — placeholder .ogg (D10)
├── dungeons/
│   ├── floor_01.dungeon.ron        # MODIFY — add `locked_doors: [((3,1), East, "rusty_door_01")]` (D2c)
│   └── floor_02.dungeon.ron        # NEW (optional, for cross-floor test) — minimal 4×4 with entry at (1,1) South (D11)
└── items/
    └── core.items.ron              # MODIFY — add `key_id: Some("rusty_door_01")` to rusty_key entry (D2)

tests/
└── cell_features_loads.rs          # NEW (optional) — integration test for floor_01 round-trip with new locked_doors field
```

**Files NOT touched:**
- `src/main.rs`
- `src/plugins/state/mod.rs`
- `src/plugins/input/mod.rs` (Interact already wired)
- `src/plugins/audio/{mod.rs, bgm.rs}` (only sfx.rs changes)
- `src/plugins/ui/{mod.rs, minimap.rs}` (dark-zone gate already implemented)
- `src/plugins/save/mod.rs` (out of scope, #23)
- `src/plugins/town/mod.rs`
- `src/plugins/combat/mod.rs` (Encounter trap stubs to a Message that #15/#16 will own)
- `src/plugins/party/{mod.rs, character.rs, inventory.rs}` (Inventory query is read-only from #13's POV; no schema changes)
- `src/data/{classes.rs, spells.rs, enemies.rs}`

**Single-file precedent.** Per Decision 4 of #11/#12 (and #9/#10): everything in one `features.rs` file. The 7 systems + 3 resources + 1 component + 2 messages + tests fit comfortably under ~700 LOC.

### Pattern 1: `features.rs` skeleton — plugin additions

```rust
// src/plugins/dungeon/features.rs — NEW file

use bevy::prelude::*;
use leafwing_input_manager::prelude::ActionState;
use std::collections::HashMap;

use crate::data::dungeon::{Direction, TeleportTarget, TrapType, WallType};
use crate::data::DungeonFloor;
use crate::data::ItemAsset;
use crate::plugins::audio::{SfxKind, SfxRequest};
use crate::plugins::dungeon::{
    Facing, GridPosition, MovedEvent, PlayerParty, handle_dungeon_input,
};
use crate::plugins::input::DungeonAction;
use crate::plugins::loading::DungeonAssets;
use crate::plugins::party::{
    ActiveEffect, DerivedStats, Inventory, ItemInstance, ItemKind, PartyMember, StatusEffectType, StatusEffects,
};
use crate::plugins::state::GameState;

// ─── Resources ────────────────────────────────────────────────────────────
#[derive(Resource, Default, Debug)]
pub struct DoorStates {
    pub doors: HashMap<(GridPosition, Direction), DoorState>,
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub enum DoorState {
    #[default]
    Closed,
    Open,
}

#[derive(Resource, Default, Debug)]
pub struct LockedDoors {
    /// Maps edge (grid pos, direction) to the door_id string. Populated from
    /// `DungeonFloor::locked_doors` on OnEnter(Dungeon).
    pub by_edge: HashMap<(GridPosition, Direction), String>,
}

#[derive(Resource, Default, Debug)]
pub struct PendingTeleport {
    /// Set when a teleporter is triggered. Read on the next OnEnter(Dungeon).
    pub target: Option<TeleportTarget>,
}

// ─── Components ───────────────────────────────────────────────────────────
#[derive(Component, Debug, Clone, Copy)]
pub struct AntiMagicZone;

#[derive(Component, Debug, Clone)]
pub struct ScreenWobble {
    pub elapsed_secs: f32,
    pub duration_secs: f32,
    pub amplitude: f32,
}

// ─── Messages ─────────────────────────────────────────────────────────────
#[derive(Message, Clone, Debug)]
pub struct TeleportRequested {
    pub target: TeleportTarget,
}

#[derive(Message, Clone, Copy, Debug)]
pub struct EncounterRequested {
    pub source: EncounterSource,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EncounterSource {
    AlarmTrap,
    // Future: Random, Foe — surfaces in #16
}

// ─── Plugin ───────────────────────────────────────────────────────────────
pub struct CellFeaturesPlugin;

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
                    tick_screen_wobble.run_if(in_state(GameState::Dungeon)),
                ),
            );
    }
}
```

`CellFeaturesPlugin` is **registered in `main.rs`** (one line), parallel to `DungeonPlugin`/`PartyPlugin`/`MinimapPlugin` registrations. It is NOT a sub-plugin of `DungeonPlugin` (Bevy doesn't do plugin nesting cleanly).

### Pattern 2: Door interaction system (Interact key on a door edge)

```rust
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
    if !actions.just_pressed(&DungeonAction::Interact) { return; }
    let Ok((pos, facing)) = party.single() else { return; };
    let Some(assets) = dungeon_assets else { return; };
    let Some(floor) = floors.get(&assets.floor_01) else { return; };

    // What edge is the player facing?
    let edge_dir = facing.0;
    let cell_walls = &floor.walls[pos.y as usize][pos.x as usize];
    let wall = match edge_dir {
        Direction::North => cell_walls.north,
        Direction::South => cell_walls.south,
        Direction::East => cell_walls.east,
        Direction::West => cell_walls.west,
    };

    match wall {
        WallType::Door => {
            // Toggle the door state.
            let key = (*pos, edge_dir);
            let current = door_states.doors.get(&key).copied().unwrap_or_default();
            let new = match current {
                DoorState::Closed => DoorState::Open,
                DoorState::Open => DoorState::Closed,
            };
            door_states.doors.insert(key, new);
            sfx.write(SfxRequest {
                kind: if new == DoorState::Open { SfxKind::Door } else { SfxKind::DoorClose },
            });
        }
        WallType::LockedDoor => {
            // Find the door_id and check inventory for matching key.
            let Some(door_id) = locked_doors.by_edge.get(&(*pos, edge_dir)) else {
                return; // No door_id authored — can't unlock.
            };
            // Walk all party members' inventories.
            let mut has_key = false;
            for inv in &inventory {
                for &item_entity in &inv.0 {
                    let Ok(instance) = instances.get(item_entity) else { continue; };
                    let Some(asset) = items.get(&instance.0) else { continue; };
                    if asset.kind == ItemKind::KeyItem
                        && asset.key_id.as_deref() == Some(door_id.as_str())
                    {
                        has_key = true; break;
                    }
                }
                if has_key { break; }
            }
            if has_key {
                // Unlock — promote to a regular Door (Closed state initially).
                door_states.doors.insert((*pos, edge_dir), DoorState::Open);
                sfx.write(SfxRequest { kind: SfxKind::Door });
                info!("Unlocked door at {:?} {:?} with key '{}'", pos, edge_dir, door_id);
                // D13: don't consume the key.
            } else {
                info!("Locked door at {:?} {:?} requires key '{}'", pos, edge_dir, door_id);
                // No SFX for "you don't have the key" — could add a "click" later (D10b).
            }
        }
        _ => {} // Not a door; no-op.
    }
}
```

### Pattern 3: Pit trap — apply damage on entry

```rust
fn apply_pit_trap(
    mut moved: MessageReader<MovedEvent>,
    floors: Res<Assets<DungeonFloor>>,
    dungeon_assets: Option<Res<DungeonAssets>>,
    mut party: Query<&mut DerivedStats, With<PartyMember>>,
    mut sfx: MessageWriter<SfxRequest>,
    mut teleport: MessageWriter<TeleportRequested>,
) {
    let Some(assets) = dungeon_assets else { return; };
    let Some(floor) = floors.get(&assets.floor_01) else { return; };

    for ev in moved.read() {
        let cell = &floor.features[ev.to.y as usize][ev.to.x as usize];
        let Some(TrapType::Pit { damage, target_floor }) = &cell.trap else { continue; };
        // Apply to all party members (genre convention).
        for mut derived in &mut party {
            derived.current_hp = derived.current_hp.saturating_sub(*damage);
        }
        sfx.write(SfxRequest { kind: SfxKind::AttackHit });
        if let Some(target_floor) = target_floor {
            // Pit drops the party to target_floor at (ev.to.x, ev.to.y) facing same direction.
            teleport.write(TeleportRequested {
                target: TeleportTarget {
                    floor: *target_floor,
                    x: ev.to.x, y: ev.to.y,
                    facing: Some(ev.facing),
                },
            });
        }
    }
}
```

### Pattern 4: Poison trap — push StatusEffect

```rust
fn apply_poison_trap(
    mut moved: MessageReader<MovedEvent>,
    floors: Res<Assets<DungeonFloor>>,
    dungeon_assets: Option<Res<DungeonAssets>>,
    mut party: Query<&mut StatusEffects, With<PartyMember>>,
    mut sfx: MessageWriter<SfxRequest>,
) {
    const POISON_TURNS: u32 = 5; // D12 default
    let Some(assets) = dungeon_assets else { return; };
    let Some(floor) = floors.get(&assets.floor_01) else { return; };
    for ev in moved.read() {
        let cell = &floor.features[ev.to.y as usize][ev.to.x as usize];
        if !matches!(cell.trap, Some(TrapType::Poison)) { continue; }
        for mut effects in &mut party {
            // D12: simple push. Stacking semantics deferred to #14.
            effects.effects.push(ActiveEffect {
                effect_type: StatusEffectType::Poison,
                remaining_turns: Some(POISON_TURNS),
                magnitude: 0.0,
            });
        }
        sfx.write(SfxRequest { kind: SfxKind::Door }); // placeholder hiss; could be SfxKind::TrapTrigger if added
    }
}
```

### Pattern 5: Spinner — randomize Facing on commit frame, apply screen wobble

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
    let Some(assets) = dungeon_assets else { return; };
    let Some(floor) = floors.get(&assets.floor_01) else { return; };
    let Ok((entity, mut facing)) = party.single_mut() else { return; };
    for ev in moved.read() {
        let cell = &floor.features[ev.to.y as usize][ev.to.x as usize];
        if !cell.spinner { continue; }
        // Random new direction. (D14 fallback if rand is not available.)
        let dirs = [Direction::North, Direction::South, Direction::East, Direction::West];
        let idx = (time.elapsed_secs_f64() * 1000.0) as usize % 4;
        // Avoid no-op spin: ensure new != old.
        let new = if dirs[idx] == facing.0 { dirs[(idx + 1) % 4] } else { dirs[idx] };
        facing.0 = new;
        sfx.write(SfxRequest { kind: SfxKind::SpinnerWhoosh });
        // Trigger 200ms screen wobble (D6).
        commands.entity(entity).insert(ScreenWobble {
            elapsed_secs: 0.0,
            duration_secs: 0.2,
            amplitude: 0.15, // radians
        });
    }
}

fn tick_screen_wobble(
    mut commands: Commands,
    time: Res<Time>,
    mut q: Query<(Entity, &mut Transform, &mut ScreenWobble)>,
) {
    for (entity, mut transform, mut wobble) in &mut q {
        wobble.elapsed_secs += time.delta_secs();
        let t = (wobble.elapsed_secs / wobble.duration_secs).clamp(0.0, 1.0);
        // Damped sine: amplitude × sin(8πt) × (1 − t)
        let envelope = (1.0 - t).max(0.0);
        let oscillation = (8.0 * std::f32::consts::PI * t).sin();
        let jitter = wobble.amplitude * envelope * oscillation;
        transform.rotation = transform.rotation * Quat::from_rotation_z(jitter);
        if t >= 1.0 {
            commands.entity(entity).remove::<ScreenWobble>();
        }
    }
}
```

**Note on minimap:** the minimap reads `&Facing` directly (`minimap.rs:269, 309, 318`), so the spinner's `facing.0 = new` mutation reflects on the minimap on the SAME frame. **Zero changes to minimap required.** The auto-map facing update is automatic.

### Pattern 6: Teleporter — same-floor mutate vs cross-floor request

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
    let Some(assets) = dungeon_assets else { return; };
    let Some(floor) = floors.get(&assets.floor_01) else { return; };
    let Ok((mut pos, mut facing, mut transform)) = party.single_mut() else { return; };
    for ev in moved.read() {
        let cell = &floor.features[ev.to.y as usize][ev.to.x as usize];
        let Some(target) = &cell.teleporter else { continue; };
        if target.floor == floor.floor_number {
            // Same-floor: mutate in place.
            let old_pos = *pos;
            pos.x = target.x; pos.y = target.y;
            if let Some(new_facing) = target.facing { facing.0 = new_facing; }
            // Snap visual transform to new world position.
            transform.translation = Vec3::new(target.x as f32 * 2.0, 0.0, target.y as f32 * 2.0);
            // Re-publish MovedEvent so minimap + dark-zone gate fire.
            writer.write(MovedEvent { from: old_pos, to: *pos, facing: facing.0 });
        } else {
            // Cross-floor: request via state-machine.
            teleport.write(TeleportRequested { target: target.clone() });
        }
        sfx.write(SfxRequest { kind: SfxKind::Door }); // placeholder; could add Teleport SFX
    }
}
```

### Pattern 7: Alarm trap — publish EncounterRequested

```rust
fn apply_alarm_trap(
    mut moved: MessageReader<MovedEvent>,
    floors: Res<Assets<DungeonFloor>>,
    dungeon_assets: Option<Res<DungeonAssets>>,
    mut encounter: MessageWriter<EncounterRequested>,
    mut sfx: MessageWriter<SfxRequest>,
) {
    let Some(assets) = dungeon_assets else { return; };
    let Some(floor) = floors.get(&assets.floor_01) else { return; };
    for ev in moved.read() {
        let cell = &floor.features[ev.to.y as usize][ev.to.x as usize];
        if !matches!(cell.trap, Some(TrapType::Alarm)) { continue; }
        info!("Alarm trap triggered at {:?} — encounter requested", ev.to);
        encounter.write(EncounterRequested { source: EncounterSource::AlarmTrap });
        sfx.write(SfxRequest { kind: SfxKind::EncounterSting });
    }
}
```

### Pattern 8: Anti-magic zone — add/remove component on enter/exit

```rust
fn apply_anti_magic_zone(
    mut moved: MessageReader<MovedEvent>,
    floors: Res<Assets<DungeonFloor>>,
    dungeon_assets: Option<Res<DungeonAssets>>,
    party: Query<Entity, With<PlayerParty>>,
    has_zone: Query<(), With<AntiMagicZone>>,
    mut commands: Commands,
) {
    let Some(assets) = dungeon_assets else { return; };
    let Some(floor) = floors.get(&assets.floor_01) else { return; };
    let Ok(entity) = party.single() else { return; };
    for ev in moved.read() {
        let cell = &floor.features[ev.to.y as usize][ev.to.x as usize];
        let in_zone = cell.anti_magic_zone;
        let currently_marked = has_zone.contains(entity);
        if in_zone && !currently_marked {
            commands.entity(entity).insert(AntiMagicZone);
            info!("Entered anti-magic zone at {:?}", ev.to);
        } else if !in_zone && currently_marked {
            commands.entity(entity).remove::<AntiMagicZone>();
            info!("Left anti-magic zone (now at {:?})", ev.to);
        }
    }
}
```

**Future #14/#15 spell-casting systems:** query `Query<(), (With<PlayerParty>, With<AntiMagicZone>)>` — empty result means casting is allowed; non-empty means blocked.

### Pattern 9: Cross-floor teleport handler in LoadingPlugin

```rust
// MODIFY src/plugins/loading/mod.rs

// Add to the imports:
use crate::plugins::dungeon::features::{TeleportRequested, PendingTeleport};

// Add to LoadingPlugin::build (additions after existing add_loading_state):
.add_message::<TeleportRequested>()  // <-- already done by CellFeaturesPlugin; remove this line if duplicate causes issues
.add_systems(Update, handle_teleport_request.run_if(in_state(GameState::Dungeon)))

fn handle_teleport_request(
    mut requests: MessageReader<TeleportRequested>,
    mut pending: ResMut<PendingTeleport>,
    mut next: ResMut<NextState<GameState>>,
) {
    if let Some(req) = requests.read().last() {
        // last() so multiple same-frame requests collapse to the most recent.
        pending.target = Some(req.target.clone());
        next.set(GameState::Loading);
        info!("Teleport requested to floor {} at ({}, {})",
            req.target.floor, req.target.x, req.target.y);
    }
}
```

**MODIFY `dungeon/mod.rs::spawn_party_and_camera`:** read `Option<Res<PendingTeleport>>` and use its values in place of `floor.entry_point` if present, then `commands.remove_resource::<PendingTeleport>()`. Small (~10 LOC) edit.

### Anti-Patterns to Avoid (Druum-specific to #13)

- **DO NOT mutate `DungeonFloor::walls` at runtime.** It's loaded as `Res<Assets<DungeonFloor>>` — read-only. Door state lives in `DoorStates: Resource`.
- **DO NOT add a per-cell schedule.** Bevy doesn't have one; use `MovedEvent` subscribers exclusively.
- **DO NOT use `EventReader<T>`** anywhere. Bevy 0.18: `MessageReader<T>` with `#[derive(Message)]`. See `feedback_bevy_0_18_event_message_split.md`.
- **DO NOT add `bevy::utils::HashMap`.** Removed in 0.18. Use `std::collections::HashMap`.
- **DO NOT call `floor.can_move` directly from `handle_dungeon_input` after #13.** Wrap it in `can_move_with_doors(floor, &door_states, x, y, dir)` so the door state gates passability. **OR** keep `floor.can_move` and add a separate "is the door open?" check in `handle_dungeon_input`. Either way, `handle_dungeon_input` MUST consult `DoorStates` before letting the player walk through a `Door`-typed wall.
- **DO NOT consume the key item on locked-door unlock.** Wizardry-style: keys are reusable. Surface as D13 if the user wants to override.
- **DO NOT pre-implement a `KeyDb` asset type.** Reuse `ItemAsset` with the `key_id` field. Same #11 single-file precedent.
- **DO NOT spawn a new `Camera3d` for the screen-wobble effect.** Mutate the existing `DungeonCamera` (or its parent `PlayerParty`) Transform. The `ScreenWobble` component lifecycle mirrors `MovementAnimation`'s remove-on-completion pattern.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| RON loading for `floor.locked_doors` | Custom serde RON loader | `bevy_common_assets::RonAssetPlugin::<DungeonFloor>` (already wired) | Already done in #3; `#[serde(default)]` makes the additive field backward-compatible. |
| Cross-floor asset loading | Custom `LoadingState` machinery | `bevy_asset_loader` `LoadingState` already in place — re-enter `GameState::Loading` (D3 Option α) | Already done in #3. |
| Per-`MovedEvent` consumer dispatch | A new generic dispatcher | `MessageReader<MovedEvent>` + 6 separate consumer systems | Bevy 0.18 message system — multi-reader is built-in. |
| Spinner randomness | Hand-roll a Mersenne Twister | `rand` (transitively in deps, verify with `cargo tree`) OR `Time::elapsed_secs_f64()` modulo | Trivial source of pseudo-randomness; cryptographic quality not needed. |
| Screen wobble | Custom shader pipeline | `Quat::from_rotation_z(jitter)` on `Transform::rotation` for 200ms | One system, ~30 LOC, no shader work. |
| Floor 2 stub | Hand-author empty floor | Reuse `floor_01` shape — copy + rename + tweak `entry_point` | If D11 says yes, ~80 lines of RON. |

---

## Common Pitfalls

### Pitfall 1: `MovedEvent` may fire before `MovementAnimation` removes; do not use `Without<MovementAnimation>` filter on consumers

**What goes wrong:** the cell-feature consumer query includes `Without<MovementAnimation>` thinking it should only fire after the tween completes. Result: the trap, spinner, or teleporter fires one frame *late* (the frame `MovementAnimation` is removed), or possibly never if the next move starts before the tween completes.

**Why it happens:** `MovementAnimation` is added on the commit frame and removed when `t_raw >= 1.0` in `animate_movement`. `MovedEvent` is published on the commit frame — the same frame `MovementAnimation` is added. So at the moment a consumer runs in the same Update, the player has `MovementAnimation`.

**How to avoid:** The cell-feature systems do NOT need any filter on `MovementAnimation`. They consume `MessageReader<MovedEvent>` — the message is in the queue independent of the animation component. The visual lerp happens AFTER the logical state changes (per the doc comment at `dungeon/mod.rs:30-34`).

### Pitfall 2: `MessageReader::read` advances the cursor — ordering matters across systems reading the same Message

**What goes wrong:** if two systems both read `MovedEvent` with `MessageReader<MovedEvent>`, **both see the message** (each has its own cursor). But if one of them filters and the other does not, the unfiltered one drains the message AND the filtered one drains it too — no double-fire.

**Why it doesn't happen in #13:** every #13 consumer reads from a fresh `MessageReader<MovedEvent>`. Bevy's `MessageReader` is per-system, not shared. **Verified at `bevy_ecs-0.18.1/src/message/reader.rs`** (every `MessageReader<T>` SystemParam gets its own per-system local cursor). The minimap subscriber is therefore not in conflict with #13's subscribers.

**How to avoid:** Just confirm during code review that every consumer takes its own `MessageReader<MovedEvent>` SystemParam.

### Pitfall 3: `floor_02.dungeon.ron` doesn't exist — cross-floor teleporter test will fail at runtime

**What goes wrong:** the live `floor_01.dungeon.ron` has a teleporter at (5,4) targeting `floor: 2`. If a test or manual smoke walks into that cell, the cross-floor handler tries to load floor 2's RON — which is not declared in `DungeonAssets` (`loading/mod.rs:31-32` only knows `floor_01`).

**Why it happens:** `DungeonAssets` is hardcoded to one floor. Adding `floor_02` requires editing `DungeonAssets` (touching loading/mod.rs).

**How to avoid:** D11 — either (a) author `floor_02.dungeon.ron` AND extend `DungeonAssets.floor_02: Handle<DungeonFloor>`, OR (b) defer cross-floor end-to-end test to a per-test override (build a test app with two floor handles), OR (c) document as a known-deferred manual smoke test. **Recommended: (a)** — add a minimal floor_02 (4×4, single room, entry at (1,1) facing South) and one extra `DungeonAssets` field. Cost: ~80 lines of RON + 2 lines of code in loading/mod.rs.

### Pitfall 4: `Door` is currently passable per `can_move` — toggling Closed must update the passability check

**What goes wrong:** the player can walk through a `WallType::Door` even if `DoorState::Closed` because `floor.can_move` returns `true` for `Door` (the asset-level "closed but not locked" reads as passable per `dungeon.rs:79-83`). The runtime layer that gates passability based on `DoorState` is what makes Closed actually closed.

**How to avoid:** `handle_dungeon_input` (in `dungeon/mod.rs:618-718`) calls `floor.can_move(pos.x, pos.y, move_dir)`. **Wrap or replace this call** with a helper that also consults `DoorStates`:

```rust
fn can_move_with_doors(
    floor: &DungeonFloor,
    doors: &DoorStates,
    pos: GridPosition,
    dir: Direction,
) -> bool {
    if !floor.can_move(pos.x, pos.y, dir) { return false; }
    let wall = ...; // (existing match logic)
    if matches!(wall, WallType::Door) {
        let state = doors.doors.get(&(pos, dir)).copied().unwrap_or_default();
        return state == DoorState::Open;
    }
    if matches!(wall, WallType::LockedDoor) {
        let state = doors.doors.get(&(pos, dir)).copied().unwrap_or_default();
        return state == DoorState::Open; // i.e., the player has unlocked it
    }
    true
}
```

**This requires editing `dungeon/mod.rs::handle_dungeon_input`** to take `Res<DoorStates>` and call the new helper. Surface as D9b. This is the largest single edit to a "frozen" module.

### Pitfall 5: Initial door state for `WallType::Door` is closed (not pre-opened)

**What goes wrong:** the floor RON declares `WallType::Door` for a wall, but `DoorStates` is empty initially. The default `DoorState::Closed` means the player can't pass — but `can_move` returned `true` (because Door is passable in the asset). With Pitfall 4's wrapper, default `Closed` blocks passage. **Is this what we want?**

**Genre answer:** Wizardry-style is YES — doors are closed by default; the player must press Interact to open them. Some DRPGs make doors auto-open when the player walks into them. Surface as D15 if the user wants auto-open. **Recommended: closed-by-default; player presses Interact.**

### Pitfall 6: Spinner immediately followed by another move — facing applied 1 frame late?

**What goes wrong:** the player moves into a spinner cell. `apply_spinner` reads the `MovedEvent`, mutates `Facing`. Same Update, the player presses W again (now facing a new direction). The `handle_dungeon_input` in the SAME tick reads `actions.just_pressed(&MoveForward)` and uses `facing.0` — but did `apply_spinner` run before `handle_dungeon_input`?

**Why this matters:** the `.after(handle_dungeon_input)` ordering means `apply_spinner` runs AFTER `handle_dungeon_input`. So if the player presses W on the spinner-frame, the move uses the OLD facing (pre-spin). Next frame, the new facing is in effect. **This is correct genre behavior** — the spin is "between turns", not "during the turn".

**However:** the auto-map (minimap) reads `&Facing` from a query. The minimap painter runs in `EguiPrimaryContextPass` (after PostUpdate), which is AFTER all Update systems including `apply_spinner`. **So the minimap shows the new facing on the same frame** — even though the player's NEXT input uses the old facing. This is the "auto-map updates post-rotation" the roadmap line 729 demands. **Already correct by ordering.**

### Pitfall 7: Pit damage applies to ALL party members — saturating subtract on `current_hp`

**What goes wrong:** `derived.current_hp -= damage` underflows when `damage > current_hp`. Result: u32 wraparound to a huge HP value.

**How to avoid:** Use `saturating_sub`. Already the project pattern (`character.rs:374-381`). The implementer must remember.

### Pitfall 8: `OnEnter(Dungeon)` re-runs after a teleport — `populate_locked_doors` must be idempotent

**What goes wrong:** Option α teleport re-enters `GameState::Loading -> GameState::Dungeon`. `OnEnter(GameState::Dungeon)` fires `populate_locked_doors` again. If the system blindly inserts into `LockedDoors.by_edge`, it stacks duplicate entries.

**How to avoid:** clear `LockedDoors.by_edge` first:
```rust
fn populate_locked_doors(...) {
    locked_doors.by_edge.clear();
    for (pos, dir, id) in &floor.locked_doors {
        locked_doors.by_edge.insert(((*pos).into(), *dir), id.clone());
    }
}
```

Same pattern as `populate_item_handle_registry` at `inventory.rs:539`. **Verified pattern.**

`DoorStates.doors` should also clear on cross-floor — but actually, since cross-floor goes through `OnExit(Dungeon)` (which calls `clear_door_resources`), the resource is naturally reset. **Confirmed safe.**

### Pitfall 9: `WallType::LockedDoor` `can_move` returns `false` — wrapper must override on `DoorState::Open`

**What goes wrong:** floor.can_move(x, y, dir) returns `false` for `WallType::LockedDoor`. After unlocking (`DoorStates.doors[(pos, dir)] = Open`), the wrapper must return `true`. The wrapper code in Pitfall 4 above handles this — but the implementer must NOT short-circuit on `floor.can_move == false` before checking `DoorStates`.

**How to avoid:** verify the wrapper logic in unit tests: a closed locked-door blocks; same locked-door with `DoorStates[(pos, dir)] = Open` allows passage.

### Pitfall 10: `core.items.ron` schema change — every existing test that round-trips items must still pass

**What goes wrong:** adding `key_id: Option<String>` to `ItemAsset` changes its serde shape. Existing tests at `items.rs:115-258` round-trip `ItemAsset` instances; they may assert exact field counts or serialize-then-deserialize-then-compare with `assert_eq`.

**How to avoid:** `#[serde(default)]` on the new `key_id` field means existing RON without `key_id` still parses. The new `Default::default()` is `None`. The existing tests use `..Default::default()` to fill in all fields — they will pick up `key_id: None`. **The tests should pass unchanged.** Verify: re-run `cargo test data::items`.

The integration test `tests/item_db_loads.rs` reads the live `core.items.ron` — adding `key_id` to `rusty_key` in the asset means that file changes; the test should still pass (it doesn't assert on `key_id`).

### Pitfall 11: Spinner whoosh + screen wobble + new facing all on the SAME frame — minimap must redraw

**What goes wrong:** spinner mutates `Facing` on commit frame. Minimap painter runs in `EguiPrimaryContextPass` AFTER Update. **Confirmed:** minimap shows new facing same frame.

**However:** if the spinner system somehow runs AFTER the minimap painter (cross-schedule ordering is "silently ignored" per `minimap.rs:135-137` comment), the minimap would lag by 1 frame. **Verify with a Layer-2 test** — emit a `MovedEvent` to a spinner cell, run `app.update()`, query `Facing`, query minimap state.

---

## Security

### Known Vulnerabilities

| Library | CVE / Advisory | Severity | Status | Action |
|---------|---------------|----------|--------|--------|
| serde 1.x | None found | — | — | Continue using |
| ron 0.12 | None found | — | — | Continue using |
| bevy 0.18.1 | None found | — | — | Continue using |
| bevy_common_assets 0.16 | None found | — | — | Continue using |
| bevy_asset_loader 0.26 | None found | — | — | Continue using |
| leafwing-input-manager 0.20 | None found | — | — | Continue using |

No known CVEs as of 2026-05-06 for any library used in #13. Same status as #11/#12.

### Architectural Security Risks

| Risk | Affected Architecture | How It Manifests | Secure Pattern | Anti-Pattern |
|------|----------------------|------------------|----------------|--------------|
| Malicious `floor_01.dungeon.ron` with massive `locked_doors` Vec | `LockedDoors` resource | `Vec::with_capacity(1B)` from a crafted floor = OOM | Trust boundary is "developer-authored RON". For modding (post-v1), bound at deserialize. Out of scope for v1. | Trust user-supplied dungeon files |
| Trap `damage: u32::MAX` | `apply_pit_trap` | Saturating-sub clamps `current_hp` to 0 — no overflow | `saturating_sub` already used. SAFE. | Direct `-=` without saturating |
| `key_id` containing path-traversal characters (`../../etc/passwd`) | Locked-door check | Used only as a string-equality compare against authored ids; never as a filesystem path | String compare is safe. SAFE. | Using `key_id` as a path |
| `event_id` (existing field, dungeon.rs:173) | Future #13/#14 scripted events | `dungeon.rs:171-173` doc-comment already requires "compile-time allow-list, never as filesystem path or shell command" | **#13 v1 ignores `event_id`** — surface in #14+. SAFE. | Using `event_id` as a path or shell command |
| `target_floor` value (e.g., `u32::MAX`) | Cross-floor teleport | `bevy_asset_loader` would fail to load nonexistent floor handle, leading to permanent Loading-state hang | Bound at runtime: log + reject if `target_floor` doesn't have a corresponding `Handle<DungeonFloor>` slot. For v1, `floor_01` and (D11) `floor_02` are the only valid values. | Trust target_floor values |
| `EncounterRequested` source spoofing | Future #16 encounter system | `EncounterSource::AlarmTrap` is just an enum tag; safe | Tagged enum pattern. SAFE. | String-typed source |
| Locked door unlock without inventory check | `handle_door_interact` | Bug — player walks through any locked door | Compile-time enforced: the `has_key` boolean is checked in the same function as the door promotion. Verify with unit test. | Skipping the key check |

### Trust Boundaries

- **`floor_01.dungeon.ron` from disk:** developer-authored. Schema-validate (clamp Vec lengths, reject negative damage, reject out-of-bounds teleport coordinates) before loading if mod support is added. Out of scope for v1.
- **`core.items.ron` from disk:** developer-authored. `key_id` is a string compare, no filesystem operations. SAFE.
- **No network input.** Single-player game.

### Specific architectural-level guards for #13

1. **Trap damage:** ALWAYS `saturating_sub`. Audit the implementer's code.
2. **Teleport target floor:** validate against the set of declared floor handles in `DungeonAssets`. Log + reject invalid floors (don't crash).
3. **Door state cleanup:** `OnExit(GameState::Dungeon)` MUST clear `DoorStates.doors` so that re-entering Dungeon (or a different floor via teleport) starts fresh. Otherwise old door states leak.
4. **Inventory key check:** the lookup walks ALL party members' inventories. This is correct genre semantics ("any party member's key works for the party"). Performance: O(party_size × items_per_inventory × ItemAsset lookup) = O(4 × ~10 × 1) = O(40) per Interact press = trivial.

---

## Performance

| Metric | Value / Range | Source | Notes |
|--------|--------------|-------|-------|
| `apply_pit_trap` per `MovedEvent` | <10 µs (1 cell read + 4 entity HP updates) | Estimated | Trap triggers happen at human-input rates; cost is negligible |
| `apply_spinner` per `MovedEvent` | <5 µs (1 cell read + 1 facing mutation + 1 SfxRequest write + 1 component insert) | Estimated | Same as above |
| `handle_door_interact` worst-case | O(party_members × items_per_inv) ≈ O(40) lookups per Interact press | std::iter | `core.items.ron` < 12 items; party ≤ 4. Trivial. |
| `tick_screen_wobble` per frame | O(1) — at most one entity has the component | Bevy ECS | Negligible. Removed when duration elapses. |
| `populate_locked_doors` on `OnEnter(Dungeon)` | O(num_locked_doors_in_floor) ≈ O(1-10) | std::iter | Runs once per dungeon entry |
| `MovedEvent` consumer cost (6 systems × 1 message) | O(6) per move, all systems Bevy-parallel-eligible | Bevy ECS | Bevy may parallelize; even serial it's <50 µs total |
| Cross-floor teleport (Option α) | 2-3 frames of black-screen + new floor RON parse + spawn | Bevy state-transition latency | Genre-correct loading flash |

**Performance is NOT a concern for Feature #13.** Cell-feature reactions happen at human-input rates (<5Hz), and each individual system is tiny. The largest cost is the cross-floor teleport's loading-state re-entry, which is by-design.

---

## Code Examples

(Patterns 1-9 above are the working illustrative examples. The skeleton compiles against the live source as verified during research.)

### Example: items.ron edit for `key_id` field

```ron
// MODIFY assets/items/core.items.ron — only the rusty_key entry changes
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

### Example: floor_01.dungeon.ron edit for `locked_doors` field

```ron
// MODIFY assets/dungeons/floor_01.dungeon.ron — append a new field at the bottom
(
    name: "Test Floor 1",
    width: 6, height: 6, floor_number: 1,
    walls: [...],   // unchanged
    features: [...], // unchanged
    entry_point: (1, 1, North),
    encounter_table: "test_table",
    lighting: ...,
    locked_doors: [
        ((3, 1), East, "rusty_door_01"),  // <-- NEW LINE: matches LockedDoor at edge (3,1)/(4,1)
    ],
)
```

### Example: cell_features Layer-1 unit test

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pit_trap_subtracts_damage_saturating() {
        let mut hp = 3u32;
        let damage = 5u32;
        hp = hp.saturating_sub(damage);
        assert_eq!(hp, 0, "saturating_sub clamps to 0, not wraparound");
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
        locked.by_edge.insert((GridPosition::default(), Direction::North), "x".into());
        locked.by_edge.clear();
        locked.by_edge.insert((GridPosition::default(), Direction::North), "x".into());
        assert_eq!(locked.by_edge.len(), 1);
    }
}
```

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|-------------|-----------------|--------------|--------|
| Hidden spinners (classic Wizardry) | Telegraphed spinners (visible icon + SFX + wobble + map updates) | Roadmap §Resolved #4 | More art / SFX / VFX work; no surprise rotations |
| Custom per-cell trigger schedule | Single `MovedEvent` Message subscribed by N systems | Project-internal Decision (#7) | Idiomatic ECS; multi-reader Messages built-in to 0.18 |
| Mutate `DungeonFloor::walls` for door state | `DoorStates: Resource(HashMap)` keyed by edge | This research D9 | Clean asset-vs-runtime separation; hot-reload safe |
| Direct `KeyItem` tag component on item entity | `key_id: Option<String>` field on `ItemAsset` (asset-driven) | This research D2 | One source of truth in RON; fewer entity-component permutations |
| Cross-floor teleport via in-state asset swap | Re-enter `GameState::Loading` for clean asset reload (Option α) | This research D3 | Leverages existing despawn-recursive cleanup; genre-correct loading flash |

**Deprecated patterns to avoid:**
- `EventReader<MovedEvent>` — must be `MessageReader<MovedEvent>`.
- `bevy::utils::HashMap` — gone in 0.18, use `std::collections::HashMap`.
- Mutating loaded asset RON at runtime — read-only contract.

---

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | Rust built-in `#[cfg(test)] mod tests` + `cargo test` |
| Config file | None (Cargo.toml conventions) |
| Quick run command | `cargo test plugins::dungeon::features` |
| Full suite (mirrors #11/#12 7-command gate) | `cargo check && cargo check --features dev && cargo clippy --all-targets -- -D warnings && cargo clippy --all-targets --features dev -- -D warnings && cargo fmt --check && cargo test && cargo test --features dev` |

### Layer split (per `feedback_bevy_input_test_layers.md`)

- **Layer 1 — pure functions (no App):** `DoorState::default`, `DoorStates`/`LockedDoors` round-trip, saturating-sub guards, `EquipSlot::None` early returns. Run with stdlib only. Sub-1ms each.
- **Layer 2 — App-driven (no `InputPlugin`):** `apply_pit_trap` end-to-end (spawn party, write `MovedEvent` for pit cell, `app.update()`, assert `current_hp` reduced). `apply_spinner` end-to-end. `apply_anti_magic_zone` enter+exit lifecycle. Use `MinimalPlugins + AssetPlugin + StatesPlugin + StatePlugin + PartyPlugin + CellFeaturesPlugin`. Pattern from `audio/mod.rs:145-178` and `inventory.rs:780-1003`.
- **Layer 3 — full `InputPlugin` chain for door Interact:** **REQUIRED** for `handle_door_interact` because it reads `Res<ActionState<DungeonAction>>`. Pattern: `MinimalPlugins + StatesPlugin + InputPlugin + ActionsPlugin + StatePlugin + PartyPlugin + DungeonPlugin + CellFeaturesPlugin`. Press `KeyCode::KeyF`, `app.update()`, assert door state flipped. (If too costly, mock `ActionState<DungeonAction>` directly via `init_resource` — see `minimap.rs:580-583`.)

### Requirements → Test Map (proposed)

| Requirement | Behavior | Test Type | Automated Command | File Exists? |
|-------------|----------|-----------|-------------------|--------------|
| Pit trap subtracts damage from all party members | Damage applied | Layer 2 | `cargo test plugins::dungeon::features::tests::pit_trap_damages_party` | ❌ needs creating |
| Pit trap with `target_floor: Some(_)` triggers `TeleportRequested` | Cross-floor | Layer 2 | `cargo test plugins::dungeon::features::tests::pit_trap_with_target_floor_requests_teleport` | ❌ needs creating |
| Poison trap pushes `StatusEffectType::Poison` onto party | Effect applied | Layer 2 | `cargo test plugins::dungeon::features::tests::poison_trap_applies_status` | ❌ needs creating |
| Alarm trap publishes `EncounterRequested` | Message emitted | Layer 2 | `cargo test plugins::dungeon::features::tests::alarm_trap_publishes_encounter` | ❌ needs creating |
| Same-floor teleporter mutates GridPosition + Facing in place | Position changed | Layer 2 | `cargo test plugins::dungeon::features::tests::same_floor_teleport_mutates_in_place` | ❌ needs creating |
| Cross-floor teleporter publishes `TeleportRequested` | Message emitted | Layer 2 | `cargo test plugins::dungeon::features::tests::cross_floor_teleport_publishes_request` | ❌ needs creating |
| Spinner randomizes Facing, emits `SpinnerWhoosh`, attaches `ScreenWobble` | Three effects, one frame | Layer 2 | `cargo test plugins::dungeon::features::tests::spinner_randomizes_facing` | ❌ needs creating |
| Door Interact toggles closed↔open | State flip | Layer 3 OR Layer 2 with mocked ActionState | `cargo test plugins::dungeon::features::tests::door_interact_toggles_state` | ❌ needs creating |
| LockedDoor Interact opens with matching key in inventory | Key check pass | Layer 2 | `cargo test plugins::dungeon::features::tests::locked_door_unlocks_with_key` | ❌ needs creating |
| LockedDoor Interact silent no-op without matching key | Key check fail | Layer 2 | `cargo test plugins::dungeon::features::tests::locked_door_blocks_without_key` | ❌ needs creating |
| AntiMagicZone marker added on enter, removed on exit | Component lifecycle | Layer 2 | `cargo test plugins::dungeon::features::tests::anti_magic_zone_lifecycle` | ❌ needs creating |
| `DungeonFloor` round-trips through RON with new `locked_doors` field | RON serde | Layer 1 (extend existing) | `cargo test data::dungeon::tests::dungeon_floor_round_trips_with_locked_doors` | ❌ needs creating in `data/dungeon.rs` |
| `core.items.ron` loads with new `key_id` field on rusty_key | Integration | `tests/item_db_loads.rs` extension | `cargo test --test item_db_loads` | ✅ exists; extend assertion |

**Roadmap budget +6-8 tests; this maps to 13 above.** Plan can drop 2-3 of the lower-value tests if budget tightens (e.g., merge alarm + cross-floor into one "two messages get published" test).

### Gaps (files to create before implementation)

- [ ] `src/plugins/dungeon/features.rs` — NEW, ~400-600 LOC
- [ ] `src/plugins/dungeon/mod.rs` — MODIFY (add `pub mod features`, register plugin if not done in main.rs; replace `floor.can_move` with `can_move_with_doors` wrapper; add 1 line to spawn_party_and_camera reading `PendingTeleport`)
- [ ] `src/plugins/audio/sfx.rs` — MODIFY (+2 SfxKind variants + match arms)
- [ ] `src/plugins/loading/mod.rs` — MODIFY (+2 sfx_* fields on AudioAssets; add handle_teleport_request system + PendingTeleport resource init)
- [ ] `src/data/dungeon.rs` — MODIFY (add `locked_doors: Vec<((u32,u32), Direction, String)>` field on DungeonFloor; round-trip test)
- [ ] `src/data/items.rs` — MODIFY (add `key_id: Option<String>` field on ItemAsset; round-trip test)
- [ ] `src/main.rs` — MODIFY (add `app.add_plugins(CellFeaturesPlugin)` — one line)
- [ ] `assets/audio/sfx/spinner_whoosh.ogg` — NEW (placeholder synthesis or royalty-free .ogg, see D10)
- [ ] `assets/audio/sfx/door_close.ogg` — NEW (placeholder, see D10)
- [ ] `assets/items/core.items.ron` — MODIFY (add `key_id: Some("rusty_door_01")` to rusty_key)
- [ ] `assets/dungeons/floor_01.dungeon.ron` — MODIFY (add `locked_doors: [((3,1), East, "rusty_door_01")]`)
- [ ] `assets/dungeons/floor_02.dungeon.ron` — NEW (D11; ~80 lines; minimal 4×4 floor for cross-floor testing)
- [ ] `tests/item_db_loads.rs` — MODIFY (extend assertion to verify `rusty_key.key_id == Some("rusty_door_01")`)

**No edits to:** `src/plugins/state/mod.rs`, `src/plugins/input/mod.rs`, `src/plugins/audio/{mod.rs, bgm.rs}`, `src/plugins/ui/{mod.rs, minimap.rs}`, `src/plugins/save/mod.rs`, `src/plugins/town/mod.rs`, `src/plugins/combat/mod.rs`, `src/plugins/party/{mod.rs, character.rs, inventory.rs}`, `src/data/{classes.rs, spells.rs, enemies.rs}`. Same cleanest-ship signal as #11/#12.

---

## The Decisions (D1-D15)

The orchestrator named D1-D10. After research, **D7-D11 are auto-resolvable** from live code or genre canon; D1-D6 + D9b + D12 + D13 are genuine Category B; **D14 + D15** are newly surfaced.

### D1 — Door state persistence

**STATUS: GENUINE Category B (light).** Two options:
- **A: Per-floor-instance only (cleared on `OnExit(Dungeon)`)** [RECOMMENDED]
- B: Persisted across floor transitions (saved by #23)

**Recommended: A.** Doors don't survive floor changes. Persistence is a #23 concern; #13 v1 is "single-floor sandbox + one cross-floor teleport".

### D2 — KeyItem representation

**STATUS: GENUINE Category B.** Three options (see Architecture Options above):
- **A: `key_id: Option<String>` on `ItemAsset` paired with `ItemKind::KeyItem`** [RECOMMENDED]
- B: `KeyItem(door_id)` tag component on item entity
- C: Reuse `ItemKind::KeyItem` only; any key opens any locked door

**Recommended: A.** See full analysis in Architecture Options.

### D2b — Where does `door_id` live for LockedDoors?

**STATUS: GENUINE Category B.** Two options:
- **A: Side-table `locked_doors: Vec<((u32,u32), Direction, String)>` on `DungeonFloor`** [RECOMMENDED]
- B: Change `WallType::LockedDoor` to `WallType::LockedDoor(String)` (variant-payload change)

**Recommended: A.** Side-table is additive (`#[serde(default)]`) and doesn't change WallType variants — no breakage of existing tests. Variant-payload change ripples.

### D2c — `data/dungeon.rs` schema edit (touching #4 territory)

**STATUS: SIDE-EFFECT of D2/D2b.** Adding the `locked_doors` field is a small additive edit. Existing tests at `dungeon.rs:438-808` use `DungeonFloor { ..Default::default() }` patterns — they will pick up `locked_doors: Vec::new()` as default. The "frozen" status of `data/dungeon.rs` is documented for major refactors; small additive `#[serde(default)]` fields are routine. **Recommended: proceed.**

### D3 — Cross-floor teleporter implementation

**STATUS: GENUINE Category B.** Three options (see Architecture Options above):
- **α: Re-enter `GameState::Loading`** [RECOMMENDED]
- β: In-state asset swap
- γ: Hybrid

**Recommended: α.** See full rationale in Architecture Options.

### D4 — Anti-magic / dark zone scope

**STATUS: AUTO-RESOLVED (mostly).** Dark zone is **already handled by #10's minimap subscriber** (`minimap.rs:208-211`); no #13 work for dark zones. Anti-magic zone has no consumer yet (#14/#15 not built); ship the marker component as plumbing.

**Recommended: ship the `AntiMagicZone` marker + add/remove system; no consumer in v1; add a `tracing::info!` log for debug.**

### D5 — Encounter trap stubbing

**STATUS: GENUINE Category B (light).** Two options:
- **A: Publish `EncounterRequested` with logged-only consumer** [RECOMMENDED]
- B: Defer entirely; alarm trap is no-op in v1

**Recommended: A.** Defines the interface for #16 cleanly. Cost is ~10 LOC.

### D6 — Screen wobble implementation

**STATUS: GENUINE Category B.** Two options:
- **A: Camera shake via `Quat::from_rotation_z(jitter)` on `Transform::rotation`, 200ms damped sine** [RECOMMENDED]
- B: Custom shader pipeline

**Recommended: A.** Cheaper, matches existing `MovementAnimation` pattern. ~30 LOC. Surface as D6b: rotation jitter vs. translation jitter (recommend rotation; "the world spins").

### D7 — SFX strategy

**STATUS: GENUINE Category B.** Three options:
- **A: Match #6 actual asset pipeline (royalty-free .ogg files committed to `assets/audio/sfx/`)** [RECOMMENDED]
- B: Procedural synthesis at runtime
- C: Silence — emit `SfxRequest` but no audio

**Recommended: A.** #6 ships 5 .ogg files (`loading/mod.rs:66-75`). Adding 2 more (`spinner_whoosh.ogg`, `door_close.ogg`) follows precedent. If user has no audio source, the user can supply or I can suggest CC0 sources (e.g., freesound.org).

### D8 — `DungeonAction::Interact` confirmation

**STATUS: AUTO-RESOLVED.** `Interact` exists at `input/mod.rs:78` and is bound to KeyCode::KeyF at line 149. **No #13 work for input.**

### D9 — Door state component shape

**STATUS: GENUINE Category B.** Three options (see Architecture Options above):
- **A: `DoorStates: Resource(HashMap<(GridPos, Direction), DoorState>)`** [RECOMMENDED]
- B: `DoorState` component on the wall plate entity
- C: Mutate `DungeonFloor::walls` (DON'T)

**Recommended: A.**

### D9b — `handle_dungeon_input` modification for door passability

**STATUS: SIDE-EFFECT of D9.** `handle_dungeon_input` MUST consult `DoorStates` before letting the player walk through a `Door`. The wrapper helper `can_move_with_doors(floor, doors, x, y, dir)` is the cleanest implementation.

This is the **largest single edit to `dungeon/mod.rs`** in #13. Adds 1 SystemParam (`Res<DoorStates>`) and replaces 1 function call. Surface as a "frozen file edit" item.

### D10 — Number of new `SfxKind` variants

**STATUS: GENUINE Category B.** Three options:
- **A: +2 variants — `SpinnerWhoosh`, `DoorClose`. Reuse `Door` for door-open and trap-snap; reuse `AttackHit` for pit damage. Reuse `EncounterSting` for alarm trap.** [RECOMMENDED]
- B: +5 variants — `SpinnerWhoosh`, `DoorClose`, `TrapTrigger`, `PitDamage`, `Teleport`. More expressive audio.
- C: +1 variant — only `SpinnerWhoosh`. Reuse `Door` for everything else.

**Recommended: A.** Two new variants matches roadmap "+2-4 trap SFX" budget at line 720.

### D11 — `floor_02.dungeon.ron` for cross-floor testing

**STATUS: GENUINE Category B.** Three options:
- **A: Author a minimal `floor_02.dungeon.ron` (~80 lines, 4×4 single room, entry at (1,1) South); add `floor_02: Handle<DungeonFloor>` to `DungeonAssets`** [RECOMMENDED]
- B: Defer cross-floor end-to-end test to manual smoke (no automated test)
- C: Mock floor 2 in tests via custom Asset insertion (no production floor)

**Recommended: A.** Cost is small (~80 lines RON + 2 lines code) and unlocks the cross-floor integration test. The floor doesn't need exotic features — just a room with the player able to walk around.

### D12 — Poison trap stacking semantics

**STATUS: GENUINE Category B.** Two options:
- **A: Naive push — second poison adds another ActiveEffect** [RECOMMENDED]
- B: Refresh duration — second poison resets the existing one's `remaining_turns`

**Recommended: A** — matches #14's expected behavior (which the roadmap §14 line 781 punts). Consistent now-vs-later.

### D13 — Locked-door key consumption

**STATUS: GENUINE Category B.** Two options:
- **A: NOT consumed (Wizardry-style; reusable keys)** [RECOMMENDED]
- B: Consumed on first unlock (one-shot)

**Recommended: A.** Genre canon. The roadmap line 731 is ambiguous ("consumes / requires"); this resolves the ambiguity.

### D14 — `rand` dependency check

**STATUS: BLOCKING but trivial to resolve.** Three options:
- **A: Verify with `cargo tree -i rand`; use directly if present** [RECOMMENDED — likely outcome: present]
- B: Use `Time::elapsed_secs_f64()` modulo as deterministic fallback
- C: Add `rand = "0.8"` to Cargo.toml (Δ deps = +1)

**Recommended: A.** Likely already transitively present. If not, B is acceptable for v1 spinner UX.

### D15 — Doors closed-by-default vs. auto-open on approach

**STATUS: GENUINE Category B (light).** Two options:
- **A: Closed-by-default; player presses Interact to open** [RECOMMENDED]
- B: Auto-open when player walks into the door

**Recommended: A.** Genre canon. Auto-open is a modern accessibility option (D15b for v2 polish).

---

## Out of scope for #13 (defer)

- **Inventory UI for selecting which key to use** → defer to #25. The locked-door system uses automatic-key-lookup ("any party member's matching key works").
- **Save/Load of toggled door state** → defer to #23. Door states are floor-scoped; #23 owns persistence.
- **Real combat triggered by alarm trap** → defer to #16. v1 publishes `EncounterRequested`; consumer is logged-only.
- **Real spell-casting blocked by anti-magic** → defer to #14/#15. v1 attaches/detaches `AntiMagicZone` marker; readers don't exist yet.
- **Multi-cell features / boss arenas** (mentioned as Con of #4) → out of scope.
- **Door opening animation (visual swing)** → defer to #25. v1 just swaps the wall material between solid (closed) and skipped (open). Surface as D9c: wall material on door-open: same brown OR remove geometry entirely?
- **`event_id` scripted events** → defer to #14+. The field exists in `CellFeatures` (`dungeon.rs:171-173`) but #13 v1 does not consume it.
- **Trap detection by Luck stat** → defer to #14 / #21. v1 traps always trigger.
- **Door auto-open accessibility option** → defer to #25 (settings).
- **Spinner with hidden "true facing" inversion** → ruled out by Resolved §4 (telegraphed).
- **Cross-party-member key sharing UI** → not needed; the lookup already walks all party inventories.

---

## Risk Register (what could go wrong)

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| The `dungeon/mod.rs::handle_dungeon_input` edit (D9b) breaks existing tests | MEDIUM | MEDIUM | Run `cargo test plugins::dungeon` immediately after the edit; mock `DoorStates::default()` is the empty-doors case which produces the existing behavior. The wrapper passes through unmodified for non-Door walls. |
| Cross-floor teleport state-transition timing is fragile | MEDIUM | MEDIUM | Use `next.set(GameState::Loading)` immediately on `TeleportRequested` read; populate `PendingTeleport` BEFORE setting state (so the `OnEnter(Loading)` doesn't race). Test with a Layer-2 integration test. |
| `populate_locked_doors` runs before floor RON is loaded | LOW | LOW | The system is gated `OnEnter(GameState::Dungeon)`, which by `bevy_asset_loader` contract only fires after `DungeonAssets` is populated. Tolerate `Option<Res<DungeonAssets>>` defensively. |
| Spinner produces same direction (no-op spin), feels broken | MEDIUM | LOW | Special case in `apply_spinner`: `if new == old { rotate_again }`. Already in Pattern 5 above. |
| Pit damage to ALL party members causes wipe before player understands | MEDIUM | MEDIUM | Designer-balance: keep pit damage modest (5 HP per cell, target_floor present). The `PartyMember` saturating_sub clamps at 0; combat doesn't run yet to distinguish "dead" vs "0 HP". `StatusEffectType::Dead` is set when current_hp reaches 0 (this is #14/#15 concern). |
| `LockedDoors.by_edge` populated on `OnEnter(Dungeon)` — but the F9 cycler can re-enter | MEDIUM | LOW | `populate_locked_doors` clears first (Pitfall 8 mitigation). Idempotent. |
| `key_id: Option<String>` change breaks `tests/item_db_loads.rs` | LOW | LOW | The test asserts on `id`, `kind`, `slot`, `stats.attack` — not `key_id`. Adding the field is invisible to the test. Verify by re-running. |
| Cross-floor teleport leaks `DoorStates` from old floor | LOW | LOW | `clear_door_resources` runs on `OnExit(Dungeon)`. `OnExit` fires before re-entering Loading. Verified by Bevy state-transition order. |
| New `floor_02.dungeon.ron` doesn't validate (`is_well_formed` fails) | LOW | LOW | Use existing `DungeonFloor::is_well_formed` and `validate_wall_consistency` in a unit test. Author the floor minimal. |
| `ScreenWobble` accidentally compounds with `MovementAnimation` | MEDIUM | LOW | Both mutate `Transform::rotation`. Bevy's last-write-wins semantics means whichever system runs last wins. Order them: `animate_movement` before `tick_screen_wobble` so the wobble layers on top. Add `.after(animate_movement)`. |
| `apply_spinner` random direction algorithm is the same on every spinner cell | MEDIUM | LOW | `Time::elapsed_secs_f64()` varies; test by stepping the app multiple times. |
| Adding `pub mod features` to `dungeon/mod.rs` surfaces a circular import (features.rs imports from dungeon/mod.rs) | LOW | MEDIUM | Bevy convention: submodule imports parent items by `use crate::plugins::dungeon::{...}`. No circular issue because the submodule isn't a direct child reverse-dep. Verify with `cargo check`. |
| The new `EncounterRequested` Message is registered in two places (CellFeaturesPlugin + a future #16 plugin) — Bevy panics on duplicate `add_message` | LOW | LOW | Bevy 0.18's `add_message` is idempotent (verified at `bevy_ecs-0.18.1/src/message/mod.rs`). No panic; one registration wins. Document the ownership in the plugin doc comment. |
| Changing `data/dungeon.rs` to add `locked_doors` field causes a subtle bug where `floor.is_well_formed()` doesn't account for it | LOW | LOW | The new field is `Vec<((u32,u32), Direction, String)>` — no per-cell shape constraint. `is_well_formed` only checks `walls`/`features` shapes (`dungeon.rs:374-386`). Safe. |

---

## Open Questions

1. **OQ1: Should `apply_pit_trap` apply damage to ALL party members or just the lead?**
   - What we know: Wizardry/Etrian convention says all members take environmental damage.
   - What's unclear: Whether the user wants this for v1 or prefers a "lead character" damage model that's lighter on UX.
   - Recommendation: All members. Surface to user if they want to override.

2. **OQ2: Does `apply_spinner` exclude the `MoveBackward` and `Strafe` semantics from its randomization?**
   - What we know: Spinner randomizes `Facing`, which then determines `MoveForward` direction.
   - What's unclear: Whether subsequent `MoveBackward` (which uses `facing.reverse()`) and strafe (which uses `facing.turn_left/right()`) should also reflect the new facing.
   - Recommendation: All movement actions use the new `Facing` post-spin (the natural Bevy ECS behavior — they read from the same component). No special handling needed.

3. **OQ3: When does the `(damage_target_floor)` Pit fire its teleport — same frame as damage?**
   - What we know: Pattern 3 emits both `current_hp -= damage` and `TeleportRequested` in the same frame.
   - What's unclear: Whether the player should see the damage flash + flavor text before being whisked to the next floor.
   - Recommendation: Same frame for v1. UI polish is #25 territory.

4. **OQ4: How does `clear_door_resources` interact with `cycle_game_state_on_f9`?**
   - What we know: `OnExit(GameState::Dungeon)` fires when F9 transitions away. `clear_door_resources` then clears `DoorStates`/`LockedDoors`/`PendingTeleport`.
   - What's unclear: If the user F9s mid-teleport (rare), `PendingTeleport` is cleared even though it was meant to flow into the next Dungeon entry.
   - Recommendation: Accept the data loss. F9 is dev-only; this is not a real-user concern.

5. **OQ5: Does `apply_anti_magic_zone` need to run on `OnEnter(Dungeon)` (initial spawn) too, in case the entry_point is in a zone?**
   - What we know: The system reads `MovedEvent`, which is only emitted on a player-initiated move.
   - What's unclear: If `entry_point` is a `anti_magic_zone` cell, the marker won't be added until the first move.
   - Recommendation: Accept as a known limitation for v1; entry_points should not be in zones (designer convention). Surface as Pitfall 12 for documentation. Add a `OnEnter(Dungeon)` handler if it becomes an issue.

6. **OQ6: Does the visual representation of a door change when `DoorState::Open`?**
   - What we know: `dungeon/mod.rs::wall_material` returns the door material for `WallType::Door` (line 304). Once open, the wall plate is still rendered with that material.
   - What's unclear: Whether v1 should despawn the wall plate on `DoorState::Open`, OR leave it visually "closed" while logically passable.
   - Recommendation: Despawn on open OR swap to a "wireframe/hollow" material. Surface as D9c for the planner. Default: leave as-is (player notices via the SFX).

---

## Sources

### Primary (HIGH confidence)

- [Druum source — `src/data/dungeon.rs`](file:///Users/nousunio/Repos/Learnings/claude-code/druum/src/data/dungeon.rs) — verified `WallType` (lines 85-104), `WallMask` (111-117), `TeleportTarget` (123-130), `TrapType` (135-149), `CellFeatures` (156-174), `DungeonFloor` (249-266), `can_move` (279-294)
- [Druum source — `src/plugins/input/mod.rs`](file:///Users/nousunio/Repos/Learnings/claude-code/druum/src/plugins/input/mod.rs) — verified `DungeonAction::Interact` (line 78), keybinding to `KeyCode::KeyF` (line 149)
- [Druum source — `src/plugins/audio/sfx.rs`](file:///Users/nousunio/Repos/Learnings/claude-code/druum/src/plugins/audio/sfx.rs) — verified `SfxRequest` Message (lines 42-45), `SfxKind` 5 variants (51-57), `handle_sfx_requests` consumer (65-91)
- [Druum source — `src/plugins/loading/mod.rs`](file:///Users/nousunio/Repos/Learnings/claude-code/druum/src/plugins/loading/mod.rs) — verified `DungeonAssets` (29-41), `AudioAssets` 10 fields (50-76), `LoadingPlugin::build` (85-118)
- [Druum source — `src/plugins/dungeon/mod.rs`](file:///Users/nousunio/Repos/Learnings/claude-code/druum/src/plugins/dungeon/mod.rs) — verified `MovedEvent` Message (192-197), publication at line 686-690, `handle_dungeon_input` `pub(crate)` (618), `spawn_party_and_camera` reading entry_point (335-402), `spawn_dungeon_geometry` material assignment (480-487), `despawn_dungeon_entities` (410-425)
- [Druum source — `src/plugins/ui/minimap.rs`](file:///Users/nousunio/Repos/Learnings/claude-code/druum/src/plugins/ui/minimap.rs) — verified `update_explored_on_move` `.after(handle_dungeon_input)` (122-125), dark-zone gate (208-211), Facing read directly (269, 309, 318)
- [Druum source — `src/plugins/party/character.rs`](file:///Users/nousunio/Repos/Learnings/claude-code/druum/src/plugins/party/character.rs) — verified `DerivedStats.current_hp` (135), `StatusEffects::has` (271-273), `StatusEffectType::Poison` variant (237-243), `ActiveEffect` shape (253-261), `derive_stats` saturating arithmetic (343-426)
- [Druum source — `src/plugins/party/inventory.rs`](file:///Users/nousunio/Repos/Learnings/claude-code/druum/src/plugins/party/inventory.rs) — verified `ItemKind::KeyItem` (79), `Inventory(Vec<Entity>)` (187), `ItemInstance(Handle)` (167), `ItemHandleRegistry` lookup (500-521), `populate_item_handle_registry` clear-first pattern (539)
- [Druum source — `src/data/items.rs`](file:///Users/nousunio/Repos/Learnings/claude-code/druum/src/data/items.rs) — verified `ItemAsset` 9 fields (84-113); `key_id` does NOT yet exist
- [Druum source — `src/plugins/state/mod.rs`](file:///Users/nousunio/Repos/Learnings/claude-code/druum/src/plugins/state/mod.rs) — verified `DungeonSubState::Map` exists (23), F9 cycler behavior (71-89)
- [Druum asset — `assets/dungeons/floor_01.dungeon.ron`](file:///Users/nousunio/Repos/Learnings/claude-code/druum/assets/dungeons/floor_01.dungeon.ron) — verified Door at (1,1)-East, LockedDoor at (3,1)-East, spinner at (2,2), Pit at (4,4), Teleporter at (5,4) → floor 2, dark_zone at (1,4), anti_magic_zone at (2,4)
- [Druum asset — `assets/items/core.items.ron`](file:///Users/nousunio/Repos/Learnings/claude-code/druum/assets/items/core.items.ron) — verified `rusty_key` exists with `kind: KeyItem, slot: None`; no `key_id` field yet
- [Druum source — `src/plugins/audio/mod.rs`](file:///Users/nousunio/Repos/Learnings/claude-code/druum/src/plugins/audio/mod.rs) — verified `AudioPlugin::build` registers `add_message::<SfxRequest>()` (110-126)
- [Druum source — `src/plugins/audio/bgm.rs`](file:///Users/nousunio/Repos/Learnings/claude-code/druum/src/plugins/audio/bgm.rs) — verified `play_bgm_for_state` state-driven crossfade pattern (82-130) — same shape `handle_teleport_request` will follow
- [Druum source — `Cargo.toml`](file:///Users/nousunio/Repos/Learnings/claude-code/druum/Cargo.toml) — verified bevy `=0.18.1`, leafwing `=0.20.0`, no `rand` direct dep
- [Druum source — `src/plugins/party/mod.rs`](file:///Users/nousunio/Repos/Learnings/claude-code/druum/src/plugins/party/mod.rs) — verified `PartyPlugin::build` registers `populate_item_handle_registry` on `OnExit(GameState::Loading)` (lines 65-68)
- [Druum source — `tests/item_db_loads.rs`](file:///Users/nousunio/Repos/Learnings/claude-code/druum/tests/item_db_loads.rs) — verified the integration test pattern; asserts on `kind`, `slot`, `stats.attack` (not `key_id`)
- [Druum source — `src/plugins/party/inventory.rs::populate_item_handle_registry`](file:///Users/nousunio/Repos/Learnings/claude-code/druum/src/plugins/party/inventory.rs) — verified clear-first idempotent pattern at lines 539-553, applicable to `populate_locked_doors`
- [Roadmap §13](file:///Users/nousunio/Repos/Learnings/claude-code/druum/project/roadmaps/20260429-01-bevy-dungeon-crawler-roadmap.md) — lines 688-737 sourcing feature requirements; line 729 is the Resolved §4 spinner-telegraphed lock
- [Feature #12 research](file:///Users/nousunio/Repos/Learnings/claude-code/druum/project/research/20260505-080000-feature-12-inventory-equipment.md) — confirms #12 ItemAsset shape, the `Handle<ItemAsset>` model, and the integration-test pattern for items.ron
- [Feature #11 research](file:///Users/nousunio/Repos/Learnings/claude-code/druum/project/research/20260504-160000-feature-11-party-character-ecs-model.md) — confirms party shape; `DerivedStats.current_hp` clamp pattern; `StatusEffects.effects.push(...)` is the canonical apply-effect path
- [Feature #12 implementation summary](file:///Users/nousunio/Repos/Learnings/claude-code/druum/project/implemented/20260505-202700-feature-12-inventory-equipment.md) — confirms 9-field `ItemAsset`, `ItemHandleRegistry` exists, 8 starter items in core.items.ron

### Secondary (MEDIUM confidence)

- [Researcher memory — `feedback_bevy_0_18_event_message_split`](file:///Users/nousunio/Repos/Learnings/claude-code/druum/.claude/agent-memory/researcher/feedback_bevy_0_18_event_message_split.md) — confirms 0.18's family rename of Event → Message
- [Researcher memory — `feedback_bevy_input_test_layers`](file:///Users/nousunio/Repos/Learnings/claude-code/druum/.claude/agent-memory/researcher/feedback_bevy_input_test_layers.md) — confirms Layer 1/2/3 test pattern; `handle_door_interact` requires Layer 3 OR `init_resource::<ActionState<DungeonAction>>` mock
- [Researcher memory — `reference_bevy_reflect_018_derive`](file:///Users/nousunio/Repos/Learnings/claude-code/druum/.claude/agent-memory/researcher/reference_bevy_reflect_018_derive.md) — confirms Reflect derive on enums/Option/Vec without extra attrs
- [Researcher memory — `feedback_third_party_crate_step_a_b_c_pattern`](file:///Users/nousunio/Repos/Learnings/claude-code/druum/.claude/agent-memory/researcher/feedback_third_party_crate_step_a_b_c_pattern.md) — confirms zero-dep path is preferred; `rand` check is a Step A/B/C if it's missing
- [Researcher memory — `reference_druum_dungeon_substate_and_input_state`](file:///Users/nousunio/Repos/Learnings/claude-code/druum/.claude/agent-memory/researcher/reference_druum_dungeon_substate_and_input_state.md) — confirms `DungeonSubState::Map` declared, `DungeonAction::OpenMap` bound

### Tertiary (LOW confidence — flagged for validation)

- DRPG genre conventions (party-wide pit damage, telegraphed spinner, key reusability) — sourced from training data on Wizardry/Etrian/Grimrock manuals; not live-verified this session. The Resolved §4 explicit lock satisfies the spinner part; party-wide damage is a default and surfaceable to the user (D-OQ1).
- Random direction algorithm via `Time::elapsed_secs_f64() as u64 % 4` — mathematically valid but quality-of-randomness is rough. Acceptable for v1 spinner UX; if `rand` is available, prefer it (D14).

---

## Metadata

**Confidence breakdown:**

- Live ground truth: HIGH — every dependency surface (#4-#12) read in full at file:line
- Standard stack + dep delta: HIGH — Cargo.toml directly read; recommended path is byte-unchanged. `rand` is the only Δ-cost question (D14).
- Architecture options: HIGH on D9 (DoorStates Resource), D2 (key_id field), D6 (camera shake); MEDIUM on D3 (cross-floor teleport α — multi-frame loading flash is a UX consequence the user may want to weigh)
- Pitfalls 1-12: HIGH — each grounded in #4-#12 precedent or Bevy 0.18 source-verified API behavior
- Decisions D1-D15: HIGH on D7-D11 (auto-resolvable from live code or genre canon); HIGH on D1, D2, D5, D9, D10, D12, D13 (genuine Category B with strong recommended defaults); MEDIUM on D3, D6 (real architectural choices); LOW on D14 (depends on `cargo tree` outcome — deferred verification)
- Tests + validation architecture: HIGH — every test pattern is a direct copy of #11/#12 (Layer 1) or #6/#10 (Layer 2 via MinimalPlugins+AssetPlugin)
- SFX strategy + asset deltas: MEDIUM — adding 2 .ogg files matches roadmap budget but the source of those files is a separate decision

**Research date:** 2026-05-06

**Dep delta:** 0 in the recommended path. **The cleanest-ship signal applies — same as #9, #10, #11, #12.** D14 may push to +1 if `rand` is not transitively present.

**LOC estimate:** +400-700 LOC matches roadmap budget at line 717.
- `src/plugins/dungeon/features.rs` — NEW, ~400-600 LOC (3 resources, 1 component, 2 messages, 8 systems, 5-8 Layer-1+2 tests)
- `src/plugins/dungeon/mod.rs` — MODIFY, +30 LOC (`pub mod features`, `can_move_with_doors` wrapper, `PendingTeleport` read in spawn)
- `src/plugins/audio/sfx.rs` — MODIFY, +6 LOC (2 enum variants, 2 match arms)
- `src/plugins/loading/mod.rs` — MODIFY, +20 LOC (2 SFX fields, `handle_teleport_request` system, `PendingTeleport` resource init)
- `src/data/dungeon.rs` — MODIFY, +15 LOC (locked_doors field + round-trip test)
- `src/data/items.rs` — MODIFY, +5 LOC (key_id field; existing tests pick up via `..Default::default()`)
- `src/main.rs` — MODIFY, +1 LOC (CellFeaturesPlugin registration)
- `assets/dungeons/floor_01.dungeon.ron` — MODIFY, +3 lines (locked_doors entry)
- `assets/dungeons/floor_02.dungeon.ron` — NEW, ~80 lines (D11 minimal floor)
- `assets/items/core.items.ron` — MODIFY, +1 line (key_id on rusty_key)
- `tests/item_db_loads.rs` — MODIFY, +5 LOC (extend assertion)

**Asset Δ:** +2 .ogg (door_close, spinner_whoosh), +1 RON (floor_02), +0 textures (door materials reuse existing `dungeon/mod.rs` palette).

**Test count Δ:** +6-9 tests, matching roadmap budget at line 721. Distribution: 4-5 Layer-1 (resource round-trips, saturating-sub), 4-5 Layer-2 (apply_*_trap end-to-end via MinimalPlugins App), 1 integration (extended `item_db_loads.rs`), 0 Layer-3 (door interact via mocked ActionState — counts as Layer-2).

**Files NOT touched:** `src/plugins/state/mod.rs`, `src/plugins/input/mod.rs`, `src/plugins/audio/{mod.rs, bgm.rs}`, `src/plugins/ui/{mod.rs, minimap.rs}`, `src/plugins/save/mod.rs`, `src/plugins/town/mod.rs`, `src/plugins/combat/mod.rs`, `src/plugins/party/{mod.rs, character.rs, inventory.rs}`, `src/data/{classes.rs, spells.rs, enemies.rs}`. Same cleanest-ship signal as #11/#12.

**Critical orchestrator handoffs:**

1. **The implementer must verify D14 (`rand` availability) BEFORE writing the spinner code.** Run `cargo tree -i rand` from project root. If absent, choose B (Time-based) or accept C (+1 dep).
2. **The `dungeon/mod.rs::handle_dungeon_input` edit (D9b) is the largest single edit to a previously-frozen module.** Audit the wrapper function `can_move_with_doors` carefully — it's the gate between "asset-level passability" and "runtime door state".
3. **Cross-floor teleport (D3 Option α) requires touching `LoadingPlugin`** — small carve-out into the otherwise-frozen plugin. The carve-out is justified (state-machine integration is its job), but document the boundary clearly in the plugin's doc comment.
4. **The schema edits to `data/dungeon.rs` (`locked_doors` field) and `data/items.rs` (`key_id` field) are additive and `#[serde(default)]`** — existing tests should pass unchanged. Verify with `cargo test data::` immediately after the schema edit, before writing the consumer code.
5. **`floor_02.dungeon.ron` is OPTIONAL but recommended (D11)** — if skipped, the cross-floor end-to-end test must be deferred to manual smoke. The plan should explicitly choose.
