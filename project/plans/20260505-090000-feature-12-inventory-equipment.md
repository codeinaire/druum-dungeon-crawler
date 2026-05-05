# Plan: Feature #12 — Inventory & Equipment

**Date:** 2026-05-05
**Status:** Draft
**Research:** `project/research/20260505-080000-feature-12-inventory-equipment.md`
**Roadmap:** `project/roadmaps/20260429-01-bevy-dungeon-crawler-roadmap.md` lines 638-684
**Depends on:** Feature #11 (party / character ECS model — merged)

---

## Goal

Land the **data layer** for items, equipment, and inventory on top of Feature #11's frozen character schema. Flesh out `ItemAsset` and `ItemDb` (currently stubs), introduce a per-character `Inventory(Vec<Entity>)` component plus an `ItemInstance(Handle<ItemAsset>)` entity model, and ship the canonical `equip` / `unequip` / `give_item` helper functions with `Result<(), EquipError>` slot validation. A `recompute_derived_stats_on_equipment_change` system subscribes to a `Message`-derived `EquipmentChangedEvent` and re-runs `derive_stats` for the affected character. Author 8 starter items in `assets/items/core.items.ron`, generate placeholder PNG icons under `assets/ui/icons/items/`, and add a `tests/item_db_loads.rs` integration test mirroring `tests/class_table_loads.rs`. **Zero new dependencies.** **No UI** (UI is Feature #25). **No save/load** (save/load is Feature #23).

---

## Approach

Adopt **Architecture Option A — Hybrid (Equipment = Handle, Inventory = Entity)** from the research doc. `Equipment` already ships with `Option<Handle<ItemAsset>>` per slot (Feature #11 Decision 3, locked at `src/plugins/party/character.rs:209-219`); we consume that shape unchanged. The bag side uses a per-character `Inventory(Vec<Entity>)` component holding entities that each carry an `ItemInstance(Handle<ItemAsset>)` component. Equipping copies the handle into the appropriate `Equipment` slot and despawns the inventory entity; unequipping spawns a fresh `ItemInstance` entity and pushes it onto `Inventory`. The `recompute_derived_stats_on_equipment_change` system flattens `Equipment` slots into `Vec<ItemStatBlock>` and calls #11's pure `derive_stats(&base, &equip_stats, &status, level)` — `derive_stats` itself is **never modified**. All file changes are confined to `src/plugins/party/inventory.rs` (NEW), `src/plugins/party/mod.rs` (additive), `src/data/items.rs` (body fleshed out), `assets/items/core.items.ron` (body fleshed out), `assets/ui/icons/items/*.png` (NEW dir, generated), and `tests/item_db_loads.rs` (NEW). `Cargo.toml` is byte-unchanged — same cleanest-ship signal as Features #7, #8, #9, #11.

---

## Out of scope

The following are **explicitly out of scope** for #12. The implementer must NOT touch them; they belong to other features:

- **Inventory UI screen** (`egui` panels, drag/drop, tooltips, slot grid). Owned by **Feature #25**. The plan does NOT consume `DungeonAction::OpenInventory`, does NOT spawn UI on `OnEnter(DungeonSubState::Inventory)`, does NOT touch `bevy_egui`.
- **Save/load remap** for `Inventory(Vec<Entity>)` and `Equipment` (`MapEntities` for the Vec, custom `Handle ↔ AssetPath` serde for equipment). Owned by **Feature #23**. The plan does NOT add `Serialize`/`Deserialize` to `Inventory`, `ItemInstance`, or `Equipment`. (Note: `Equipment` already does NOT derive serde per `character.rs:196-202` — leave that as-is.)
- **Stackable items.** Roadmap line 681 explicitly punts. The plan declares `stackable: bool` on `ItemAsset` for forward-compat but **no system reads it in v1**. Each potion is a unique entity. No `StackableItem` component, no stack-merge logic.
- **Per-instance state on items** — `Enchantment(u8)`, `Durability(u32)`, `CustomName(String)`. Doc-comment promise at `character.rs:204-205` is satisfied by the existence of the `ItemInstance` entity model itself; concrete state components land when the first reader appears (likely #21 loot / #15 combat).
- **Input handling.** `DungeonAction::OpenInventory` is already bound to Tab at `src/plugins/input/mod.rs:80, 151`. Feature #12 does NOT consume this binding; #25 will.
- **Combat consumption** (drinking a potion to heal). Owned by Feature #15. The `ItemKind::Consumable` variant exists but no system in #12 reduces inventory on use.
- **Loot tables / shop integration.** Owned by Features #18, #21. The `value: u32` and `weight: u32` fields exist for those consumers; #12 ships them as data only.
- **Hot-reload of `core.items.ron`.** No `AssetEvent<ItemAsset>` subscriber. Hot-reload re-derive is a post-v1 `--features dev` enhancement.

---

## Frozen post-#11 / DO NOT TOUCH

These files are frozen by Features #1–#11 and must not be modified by the #12 implementer. Changes here represent regressions that broke prior features. The research doc explicitly enumerates these (research §"No edits to:"):

- `src/plugins/party/character.rs` — **FROZEN by #11.** All character types, `Equipment` shape, `derive_stats` signature, `PartyMemberBundle` are locked. The doc-comments at lines 194-208 describe the contract #12 must mirror. Do NOT modify `Equipment` to hold `Entity`. Do NOT modify `derive_stats` to take `&Equipment`. Do NOT add fields to `PartyMemberBundle`.
- `src/plugins/loading/mod.rs` — **FROZEN post-#3.** `RonAssetPlugin::<ItemDb>::new(&["items.ron"])` registration at line 98 and `item_db: Handle<ItemDb>` at line 34 are already wired. The asset path is locked to `items/core.items.ron` (line 33). Do NOT add a new asset-collection field, do NOT change the path, do NOT register a new RonAssetPlugin.
- `src/plugins/state/mod.rs` — **FROZEN.** `DungeonSubState::Inventory` already exists at line 23. #12 does not add a state, does not run `OnEnter(DungeonSubState::Inventory)`.
- `src/plugins/input/mod.rs` — **FROZEN by #5.** `DungeonAction::OpenInventory = KeyCode::Tab` already exists at lines 80, 151. #12 does not consume this binding.
- `src/plugins/dungeon/mod.rs` — **FROZEN by #4/#7/#8/#9/#10.**
- `src/plugins/audio/mod.rs`, `src/plugins/audio/sfx.rs` — **FROZEN by #6.**
- `src/plugins/combat/mod.rs` — does not exist yet (#15 owns).
- `src/plugins/ui/mod.rs`, `src/plugins/ui/minimap.rs` — **FROZEN by #10.**
- `src/plugins/save/mod.rs`, `src/plugins/town/mod.rs` — do not exist yet (#23, #18).
- `src/data/dungeon.rs`, `src/data/spells.rs`, `src/data/enemies.rs`, `src/data/classes.rs` — **FROZEN-from-day-one** per project precedent (#9 doc-comment exception is the only writeup-level allowed touch on `data/dungeon.rs`).
- `src/main.rs` — no change. `PartyPlugin` is already registered at line 32.
- `Cargo.toml`, `Cargo.lock` — **byte-unchanged.** Zero new dependencies.

The complete enumerated list lives in research §Architecture Patterns "No edits to" line.

---

## Open Decisions Awaiting User Input

The research doc surfaced D1–D8. **D1, D6** are auto-resolved by #11's already-shipped code. **D2, D3, D7** have strong recommended defaults — proceeding unless user objects. **D4, D5, D8** are genuine designer/style preferences worth confirming with the user before kickoff.

D-numbers below match the research doc exactly so the user can cross-reference.

### D1 — `Equipment` shape: `Entity` vs `Handle` — RESOLVED (auto)

**Status:** Auto-resolved by Feature #11.
**Resolution:** `Equipment` ships as `Option<Handle<ItemAsset>>` per slot at `character.rs:209-219`. Reverting requires reopening #11 Decision 3, which the user already approved. Plan keeps Handle-based equipment.

### D2 — `Inventory` shape: `Vec<Entity>` vs `Vec<Handle>` vs Resource — RECOMMENDED, proceed unless user objects

**Recommendation:** `Inventory(Vec<Entity>)` per character (Option A in research §D2).

**Tradeoff summary:**
- **A: `Inventory(Vec<Entity>)` per character [RECOMMENDED].** Each item is an entity carrying `ItemInstance(Handle)`. Per-instance state (post-v1 enchantment) is native. Honors doc-comment promise at `character.rs:204-205`. Cost: requires `MapEntities` for #23 save/load (acceptable; #23 will already have one for `Equipment`'s Handle ↔ AssetPath custom serde).
- **B: `Inventory(Vec<Handle<ItemAsset>>)` per character.** ~50 LOC less, zero MapEntities pain ever, but no per-instance state ever. Reverses doc-comment promise.
- **C: `PartyInventory: Resource(Vec<Entity>)` shared.** Defeats per-character genre canon (Wizardry has per-member bags); roadmap line 675 explicitly closes this.

**If user picks B:** drop `ItemInstance` (handles live directly in the Vec); equip path becomes a simple Vec ↔ slot move. Plan-of-record cost: -50 LOC inventory.rs, -1 helper function, -2 tests, -0 LOC in `derive_stats` (unchanged), but loses path to per-instance enchantment without revisiting the model.

### D3 — Slot validation: `Result<(), EquipError>` vs panic vs silent — RECOMMENDED, proceed unless user objects

**Recommendation:** `Result<(), EquipError>` from the helper functions; the Bevy system that wraps them logs `warn!(...)` on `Err`.

**Tradeoff summary:**
- **A: `Result<(), EquipError>` [RECOMMENDED].** Idiomatic Rust. UI in #25 can consume the Err type for tooltip text ("can't equip dagger in armor slot"). Layer-1 testable as a pure function. Adds ~40 LOC (the `EquipError` enum + match arms).
- **B: Panic.** Catches dev-time bugs but crashes in release. UI must duplicate validation defensively.
- **C: Silent no-op.** Masks bugs. UI can't surface "why didn't this equip" feedback.

### D4 — Items.ron starter content: 8 vs 12 items — USER PICK

**Recommendation:** **8 items** (3 weapons, 2 armor, 1 shield, 1 consumable, 1 key item). Within roadmap budget (8-12). Exercises 5 of 9 `ItemKind` variants.

**Tradeoff summary:**
- **A: 8 items [RECOMMENDED — minimal viable test surface].** Covers `Weapon`, `Armor`, `Shield`, `Consumable`, `KeyItem`. Does NOT exercise `Helm`, `Gloves`, `Boots`, `Accessory` slots end-to-end (they're declared but no item authored).
- **B: 12 items.** Adds 1 helm, 1 gloves, 1 boots, 1 accessory item — full slot coverage in the integration test. ~30 minutes of designer-stat tuning per item. Maps cleanly to roadmap 12 ceiling.
- **C: 6 items.** Below roadmap floor. Not recommended.

The illustrative content for the 8-item set is in research §Code Examples Example 3.

### D5 — Debug `give_starter_items_to_debug_party` system — USER PICK

**Recommendation:** **Skip** for #12 (Option B in research §D5). Items are exercised only via Layer-2 tests; in-game item flow first matters for Feature #21 (loot).

**Tradeoff summary:**
- **A: Ship `give_starter_items_to_debug_party` gated on `#[cfg(feature = "dev")]`, triggered after `spawn_default_debug_party`.** +50 LOC, +1 system, requires the same idempotence guard as `spawn_default_debug_party` (Pitfall 7). Useful for #25 UI dev later, but #25 is far away.
- **B: Skip [RECOMMENDED].** Cleaner ship. Debug party has empty inventory in-game. UI dev (#25) starts with empty bags — that's the realistic state when the player first enters the dungeon anyway.

### D6 — Reverse #11 Decision 3 (Equipment = Entity)? — RESOLVED unless user asks

**Status:** Auto-resolved (do not reverse). Surface only if user pushes back on D1/D2 with a desire to make `Equipment` hold `Entity`.

**Cost if reversed:** ~80 LOC `MapEntities` plumbing + ~40 LOC `Equipment` redesign + edit `recompute_derived_stats_on_equipment_change` to query `&ItemInstance` instead of `&Assets<ItemAsset>` + a save/load round-trip integration test that wasn't required before. **Reverts #11's resolved Decision 3** which is the exact work #11 already shipped.

### D7 — RON-vs-code declaration of `EquipSlot` ↔ `Equipment` field map — RECOMMENDED, proceed unless user objects

**Recommendation:** Hand-written match arms in `EquipSlot::read` and `EquipSlot::write` (Option A in research §D7).

**Tradeoff summary:**
- **A: Hand-written match arms [RECOMMENDED].** Two `match` blocks, 8 arms each (~30 LOC). Trivially debuggable, type-safe. The 8-slot enum is stable; macros add cognitive cost without payoff.
- **B: `macro_rules!` to generate match arms from a list of `(EquipSlot, field)` pairs.** Saves ~10 LOC. YAGNI for an 8-slot enum.

### D8 — Asset placeholder icons: who, what, when — USER PICK

**Recommendation:** **A — ImageMagick-generated placeholder PNGs**, committed via a one-time `scripts/gen_placeholder_icons.sh` script. 32×32 PNGs with letter-code labels (e.g., "RS" for Rusty Sword on red square).

**Tradeoff summary:**
- **A: ImageMagick-generated, script-committed [RECOMMENDED].** ~30 minutes of setup. Visual quality acceptable for v1. `icon_path` resolves to a real file; future contributors can regenerate. Total asset Δ: ~8 PNGs × <1KB ≈ 8KB.
- **B: Skip the PNGs — `icon_path` references nonexistent files for v1.** Zero asset work. Roadmap line 679 explicitly says "produce 5-10 placeholder PNGs"; deviates without strong reason.
- **C: Source from CC0 icon pack (e.g., game-icons.net).** Real-looking icons but requires attribution, license-cardinality concerns (CC-BY 3.0 in some packs). Slower.

**Sub-questions for the user under Option A:**
- Is ImageMagick `convert` already installed on the dev machine? (Confirmed available on macOS via Homebrew; if missing, the implementer will `brew install imagemagick`.)
- Should the script be committed under `scripts/` (NEW dir) or `tools/`? Recommendation: `scripts/`.

---

## Steps

The implementation proceeds in **9 phases**, each one a single atomic commit boundary. Every phase's exit criterion is `cargo test` passing. Phases 1-4 build the data and logic layer; Phase 5 wires the plugin; Phase 6 ships the asset content; Phase 7 generates icons; Phase 8 validates with integration tests; Phase 9 is the final verification gate.

### Phase 1 — Flesh out `src/data/items.rs`

Builds the data-only layer first; no plugin coupling, no entity model. This phase is independently compilable.

- [ ] In `src/data/items.rs`, replace the empty `ItemDb { }` body with `ItemDb { pub items: Vec<ItemAsset> }` and add an inherent `impl ItemDb { pub fn get(&self, id: &str) -> Option<&ItemAsset> { self.items.iter().find(|i| i.id == id) } }` (mirror `ClassTable::get` at `src/data/classes.rs:41-43`).
- [ ] In the same file, extend `ItemAsset` from the current 1-field stub to the 8-field schema: `id: String`, `display_name: String`, `stats: ItemStatBlock` (kept), `kind: ItemKind`, `slot: EquipSlot`, `weight: u32` (`#[serde(default)]`), `value: u32` (`#[serde(default)]`), `icon_path: String` (`#[serde(default)]`), `stackable: bool` (`#[serde(default)]`). Keep `#[derive(Asset, Reflect, Serialize, Deserialize, Default, Debug, Clone)]`.
- [ ] Add the import line `use crate::plugins::party::inventory::{EquipSlot, ItemKind};` at the top of `src/data/items.rs`. This is the same one-way reverse-dep pattern as `src/data/classes.rs` (which imports `Class`/`Race` from `plugins::party`). Add a doc comment on the import explaining the shape mirror.
- [ ] Update the file-level doc comment at `src/data/items.rs:1-7` to reflect that #12 has fleshed out the schema. Note that `ItemStatBlock` is unchanged from #11.
- [ ] Add a Layer-1 unit test `item_asset_round_trips_through_ron()` to `mod tests` (mirror the existing `item_stat_block_round_trips_through_ron` test): construct an `ItemAsset` with all 8 fields populated, serialize via `ron::ser::to_string_pretty`, deserialize, assert equality. Add `PartialEq` to the `ItemAsset` derive list (currently absent — required for `assert_eq!`).
- [ ] Add a Layer-1 unit test `item_db_round_trips_through_ron()`: construct `ItemDb { items: vec![…2 items…] }`, round-trip, assert order preserved.
- [ ] Add a Layer-1 unit test `item_db_get_returns_authored_item()`: construct `ItemDb`, call `db.get("rusty_sword")`, assert `Some(&...)` with matching id.
- [ ] **Verification:** `cargo test data::items::tests` passes; `cargo build` succeeds. (Build will fail until Phase 2 lands `EquipSlot` and `ItemKind` — that's expected; Phase 1 commit is reordered AFTER Phase 2 to keep the tree green at every commit boundary. See "Commit ordering note" below.)

**Commit ordering note:** Because `ItemAsset` references `EquipSlot` and `ItemKind` (defined in Phase 2's new file), Phase 1 cannot land before Phase 2 in commit order. **Final commit order: Phase 2 first, then Phase 1.** The phases stay numbered 1-9 for plan readability; the implementer commits Phase 2 first.

### Phase 2 — Create `src/plugins/party/inventory.rs` skeleton

Lands all the new types in one file; no plugin glue yet. This phase is the bulk of the new LOC (~400 lines).

- [ ] Create `src/plugins/party/inventory.rs` as a NEW file. Add the file-level doc comment summarizing the module's purpose (item entity model + equipment helpers + recompute system) and pointing at the doc-comment promise at `character.rs:204-205`.
- [ ] Add imports at the top: `use bevy::prelude::*;`, `use serde::{Deserialize, Serialize};`, `use crate::data::{ItemAsset, ItemStatBlock};`, `use crate::plugins::party::character::{BaseStats, Equipment, Experience, PartyMember, StatusEffects, DerivedStats, derive_stats};`.
- [ ] Define `ItemKind` enum with 9 variants per research §Pattern 2: `#[default] Weapon, Shield, Armor, Helm, Gloves, Boots, Accessory, Consumable, KeyItem`. Derives: `Reflect, Serialize, Deserialize, Default, Debug, Clone, Copy, PartialEq, Eq, Hash`. Doc comment names the validation contract (only first 7 are equippable; `Consumable`/`KeyItem` reject equip with `EquipError::ItemHasNoSlot`).
- [ ] Define `EquipSlot` enum with 9 variants per research §Pattern 2: `#[default] None, Weapon, Shield, Armor, Helm, Gloves, Boots, Accessory1, Accessory2`. Same derives as `ItemKind`. Doc comment notes discriminant order is locked for save-format stability (same rule as `Class`, `Race`, `StatusEffectType` in #11).
- [ ] Add `impl EquipSlot` block with two methods:
  - `pub fn read(self, eq: &Equipment) -> Option<&Handle<ItemAsset>>`: 9-arm match returning the field-by-field handle. `EquipSlot::None => None`. Maps PascalCase variant → snake_case field (research §Pitfall 2).
  - `pub fn write(self, eq: &mut Equipment, handle: Option<Handle<ItemAsset>>)`: 9-arm match writing the handle. `EquipSlot::None => {}` (no-op, defensive). Note: this method is only called internally from `equip_item` / `unequip_item`; the `None` arm is defensive.
- [ ] Define `ItemInstance(pub Handle<ItemAsset>)` component per research §Pattern 3. Derives: `Component, Reflect, Default, Debug, Clone, PartialEq`. **No `Serialize`/`Deserialize`** — `Handle<T>` doesn't implement serde in Bevy 0.18; #23 owns that bridge. Doc comment must repeat the no-serde rationale (mirror `character.rs:196-202`).
- [ ] Define `Inventory(pub Vec<Entity>)` component per research §Pattern 3. Same derives as `ItemInstance` (no serde). Doc comment names the `MapEntities` requirement for #23.
- [ ] Define `EquipmentChangedEvent { pub character: Entity, pub slot: EquipSlot }` per research §Pattern 4. Derives: `Message, Clone, Copy, Debug`. **MUST be `#[derive(Message)]`, NOT `#[derive(Event)]`** — Bevy 0.18 family rename. Doc comment names `MovedEvent` at `dungeon/mod.rs:192-197` as the canonical project precedent and notes the `...Event` suffix is a genre-familiarity convention (matches `MovedEvent`, `SfxRequest` is the message-without-suffix counter-example).
- [ ] Define `EquipError` enum per research §Pattern 5 with 4 variants: `ItemHasNoSlot, SlotMismatch, CharacterMissingComponents, ItemMissingComponents`. Derives: `Debug, Clone, Copy, PartialEq, Eq`. Each variant has a doc comment explaining when it fires.
- [ ] Define `pub type EquipResult = Result<(), EquipError>;`.
- [ ] **Verification:** `cargo build` passes (Phase 2 file compiles alone — at this point Phase 1's `data/items.rs` references `EquipSlot`/`ItemKind` from this module, so Phase 2 commits FIRST).

### Phase 3 — Implement `equip_item`, `unequip_item`, `give_item` helpers

Three small public functions that mutate `Equipment` and `Inventory`. They take a `&mut Commands` and a few queries; not Bevy systems themselves (Bevy systems can't be called directly from arbitrary callers).

- [ ] In `src/plugins/party/inventory.rs`, add `pub fn equip_item(commands: &mut Commands, character: Entity, item_entity: Entity, slot: EquipSlot, items: &Assets<ItemAsset>, instances: &Query<&ItemInstance>, char_query: &mut Query<(&mut Equipment, &mut Inventory), With<PartyMember>>, writer: &mut MessageWriter<EquipmentChangedEvent>) -> EquipResult`. Body per research §Code Examples Example 2:
  1. Reject `slot == EquipSlot::None` with `EquipError::ItemHasNoSlot`.
  2. Read `ItemInstance` from `instances.get(item_entity)`; map error to `EquipError::ItemMissingComponents`.
  3. Resolve `items.get(&instance.0)` to `&ItemAsset`; on `None`, return `EquipError::ItemMissingComponents`.
  4. Reject `asset.slot == EquipSlot::None || matches!(asset.kind, ItemKind::Consumable | ItemKind::KeyItem)` with `EquipError::ItemHasNoSlot`.
  5. Reject `asset.slot != slot` with `EquipError::SlotMismatch`.
  6. `char_query.get_mut(character)` → tuple `(&mut Equipment, &mut Inventory)`; map error to `EquipError::CharacterMissingComponents`.
  7. If `slot.read(&equipment).cloned()` is `Some(prev_handle)`: spawn a new entity `commands.spawn(ItemInstance(prev_handle)).id()`, push onto `inventory.0`. (Research §Pitfall 1 — note the new entity is queryable next frame, not same frame.)
  8. `slot.write(&mut equipment, Some(instance.0.clone()))` — write the new handle.
  9. `inventory.0.retain(|&e| e != item_entity)` — remove from inventory list.
  10. `commands.entity(item_entity).despawn()` — despawn the inventory entity.
  11. `writer.write(EquipmentChangedEvent { character, slot })`.
  12. Return `Ok(())`.
- [ ] Add `pub fn unequip_item(commands: &mut Commands, character: Entity, slot: EquipSlot, char_query: &mut Query<(&mut Equipment, &mut Inventory), With<PartyMember>>, writer: &mut MessageWriter<EquipmentChangedEvent>) -> EquipResult`. Body:
  1. Reject `slot == EquipSlot::None` with `EquipError::ItemHasNoSlot`.
  2. `char_query.get_mut(character)` → `(&mut Equipment, &mut Inventory)`; on Err, return `EquipError::CharacterMissingComponents`.
  3. Read `slot.read(&equipment).cloned()` — if `None`, return `Ok(())` (idempotent — unequipping an empty slot is a no-op success).
  4. Spawn `commands.spawn(ItemInstance(handle.clone())).id()`; push onto `inventory.0`.
  5. `slot.write(&mut equipment, None)`.
  6. `writer.write(EquipmentChangedEvent { character, slot })`.
  7. Return `Ok(())`.
- [ ] Add `pub fn give_item(commands: &mut Commands, character: Entity, handle: Handle<ItemAsset>, char_query: &mut Query<&mut Inventory, With<PartyMember>>) -> EquipResult`. Body:
  1. Spawn `commands.spawn(ItemInstance(handle)).id()`.
  2. `char_query.get_mut(character)` → `&mut Inventory`; on Err, despawn the just-spawned entity (cleanup) and return `EquipError::CharacterMissingComponents`.
  3. Push the entity onto `inventory.0`.
  4. **Does NOT emit `EquipmentChangedEvent`** — this only changes inventory, not equipment. (Doc comment must call this out.)
  5. Return `Ok(())`.
- [ ] Add a doc comment at the top of the helper-functions section explaining why these are free functions (not systems): Bevy systems consume only their parameter list; helpers compose with future callers (#21 loot, #18 shop, #25 UI). Cross-reference research §Pattern 6.
- [ ] Add Layer-1 unit tests in `mod tests`:
  - `equip_consumable_returns_item_has_no_slot`: build minimal in-memory `ItemAsset { kind: Consumable, slot: None, ... }` and an `Assets<ItemAsset>` add, assert `equip_item(...)` returns `Err(ItemHasNoSlot)`.
  - `equip_sword_in_armor_slot_returns_slot_mismatch`: assert `Err(SlotMismatch)`.
  - `equip_slot_read_write_round_trip`: for each `EquipSlot::Weapon..Accessory2` variant, write a `Handle::default()` to a fresh `Equipment::default()`, then read it back, assert equality.
  - `equip_slot_none_read_returns_none`: `EquipSlot::None.read(&Equipment::default())` returns `None`.
  - `equip_slot_none_write_is_noop`: `EquipSlot::None.write(&mut Equipment::default(), Some(Handle::default()))` does not modify the Equipment (all 8 fields stay `None`).

  These Layer-1 tests are pure-function tests that build the `Assets<ItemAsset>` resource directly without an `App`. The pattern requires a small test harness (~30 LOC) that:
  1. Creates a `World` with `init_resource::<Assets<ItemAsset>>` + `init_resource::<Messages<EquipmentChangedEvent>>`.
  2. Spawns the test entity / item with `world.spawn(...)`.
  3. Acquires `Commands`, queries, and a `MessageWriter` via a `world.run_system_once(closure)` shape, OR uses `World::commands()` + `World::query()` directly.

  **Pattern source:** mirror the test harnesses already used at `src/plugins/party/character.rs::tests` (it uses pure functions with no `App`). If the helper signature requires an `App`, the test moves to Layer 2 (`mod app_tests`) — see Phase 4.

- [ ] **Verification:** `cargo test plugins::party::inventory::tests` passes; `cargo build --features dev` succeeds.

### Phase 4 — Implement `recompute_derived_stats_on_equipment_change` system

The single Bevy system that subscribes to `EquipmentChangedEvent` and re-runs `derive_stats` for the affected character. This is the recompute pipeline.

- [ ] In `src/plugins/party/inventory.rs`, add `pub fn recompute_derived_stats_on_equipment_change(mut events: MessageReader<EquipmentChangedEvent>, items: Res<Assets<ItemAsset>>, mut characters: Query<(&BaseStats, &Equipment, &StatusEffects, &Experience, &mut DerivedStats), With<PartyMember>>)`. Body per research §Pattern 7:
  1. For each `ev in events.read()`:
     a. `characters.get_mut(ev.character)` → `let-else continue` (not a panic — character may have been despawned between event emit and read).
     b. Build `let mut equip_stats: Vec<ItemStatBlock> = Vec::with_capacity(8);`.
     c. Iterate the 8 slots `[&equip.weapon, &equip.armor, &equip.shield, &equip.helm, &equip.gloves, &equip.boots, &equip.accessory_1, &equip.accessory_2]`. For each `Some(handle)`, look up `items.get(handle)`; if `Some(asset)`, push `asset.stats`. **Defensive:** if `items.get(handle)` returns `None` (asset not loaded — research §Pitfall 3), log `warn!("ItemAsset not loaded; recompute will produce wrong stats. Slot: {:?}", slot_name)` and skip the slot (don't push 0 — let derive_stats run on whatever IS loaded).
     d. Call `let new = derive_stats(base, &equip_stats, status, xp.level);`.
     e. Caller-clamp pattern (research §Pattern 7, character.rs:128-131): `let old_current_hp = derived.current_hp; let old_current_mp = derived.current_mp; *derived = new; derived.current_hp = old_current_hp.min(derived.max_hp); derived.current_mp = old_current_mp.min(derived.max_mp);`.
- [ ] Add a Layer-2 integration test `equip_sword_raises_attack_unequip_lowers` in a new `#[cfg(test)] mod app_tests` per research §Code Examples Example 5:
  - `make_test_app()` builds an `App` with `MinimalPlugins, AssetPlugin::default(), StatesPlugin, StatePlugin, PartyPlugin`. (Note: `PartyPlugin` registers the message + system in Phase 5, so this test's earlier-passing requires Phase 5 to land before this test runs — the COMMIT for this test goes in the SAME phase as Phase 5.)
  - Author a sword `ItemAsset` directly via `Assets::add`. Spawn a character with `PartyMemberBundle::default()` and `.insert(Inventory::default())`. Spawn an `ItemInstance(sword_handle)` entity, push onto inventory.
  - Manually mutate `Equipment::weapon = Some(handle.clone())` AND emit an `EquipmentChangedEvent` directly (the test exercises the recompute system, not the helper functions).
  - `app.update()`. Assert `DerivedStats::attack == 10` (or whatever the test sword's attack is).
  - Then clear `Equipment::weapon = None`, emit another `EquipmentChangedEvent`, `app.update()`. Assert attack drops back to 0.
- [ ] Add a second Layer-2 test `equip_emits_message_via_helper`: build the same app, call `equip_item(...)` via a one-shot system (`World::run_system_once`), drain `Messages<EquipmentChangedEvent>` after `app.update()`, assert exactly one event emitted with the correct `character` and `slot`.
- [ ] Add a third Layer-2 test `give_item_pushes_to_inventory`: spawn character, call `give_item` via a one-shot system, assert `Inventory::0.len() == 1`.
- [ ] **Verification:** Layer-2 tests are committed alongside Phase 5 (which lands the plugin registration). Until Phase 5 lands, only Layer-1 tests in this file compile.

### Phase 5 — Wire into `PartyPlugin::build`

Single edit to `src/plugins/party/mod.rs` to register the message, types, and system. This is where Phase 2-4 become live in the running app.

- [ ] In `src/plugins/party/mod.rs`, add `pub mod inventory;` directly after `pub mod character;` at line 8.
- [ ] Add re-exports from `inventory`. After the existing `pub use character::{...}` block, add:

  ```rust
  pub use inventory::{
      EquipError, EquipResult, EquipSlot, EquipmentChangedEvent,
      Inventory, ItemInstance, ItemKind,
      equip_item, give_item, unequip_item,
      recompute_derived_stats_on_equipment_change,
  };
  ```

- [ ] Inside `impl Plugin for PartyPlugin::build`, AFTER the existing 14 `register_type` calls, add a contiguous block:

  ```rust
  // Feature #12: inventory + equipment messaging
  app.add_message::<EquipmentChangedEvent>()
      .register_type::<Inventory>()
      .register_type::<ItemInstance>()
      .register_type::<EquipSlot>()
      .register_type::<ItemKind>()
      .add_systems(Update, recompute_derived_stats_on_equipment_change);
  ```

  Doc-comment the block: `// Feature #12 — inventory & equipment data layer. UI lives in #25.`
- [ ] **Inventory insertion on debug party (does NOT modify `character.rs`):** if D5 is **A** (ship `give_starter_items_to_debug_party`), it lands here. If D5 is **B** (skip), then `Inventory` is still inserted onto debug party members so #25's UI dev has a consistent model. Inside `spawn_default_debug_party` at `mod.rs:62-96`, change the per-member `commands.spawn(PartyMemberBundle { ... })` call to ALSO insert `Inventory::default()`:

  ```rust
  commands.spawn(PartyMemberBundle { ... }).insert(Inventory::default());
  ```

  This is the ONLY change to `spawn_default_debug_party`'s body. Doc-comment the insert: `// Feature #12: each party member carries its own bag (Wizardry-style).`
- [ ] **Verification:** `cargo build && cargo build --features dev && cargo test`. Confirm that `cargo test plugins::party::inventory::app_tests` now passes (the Layer-2 tests committed in Phase 4 require this phase's plugin registration to be live).

### Phase 6 — Author `assets/items/core.items.ron` body

Replace the 3-line stub with the chosen starter set. Uses RON's tuple-record syntax (the same as `core.classes.ron`).

- [ ] Replace the contents of `assets/items/core.items.ron` with the 8-item starter set per research §Code Examples Example 3 — items: `rusty_sword` (Weapon, attack 5), `oak_staff` (Weapon, attack 2 + magic_attack 3), `wooden_mace` (Weapon, attack 4 + magic_attack 1), `leather_armor` (Armor, defense 3), `robe` (Armor, defense 1 + magic_defense 2 + mp_bonus 5), `wooden_shield` (Shield, defense 2), `healing_potion` (Consumable, no stats), `rusty_key` (KeyItem, no stats).
- [ ] Each item record uses the 8-field shape: `id`, `display_name`, `stats`, `kind`, `slot`, `weight`, `value`, `icon_path`. Default `stackable: false` is omitted (`#[serde(default)]`).
- [ ] If D4 is **B** (12 items), add 4 more: 1 helm, 1 gloves, 1 boots, 1 accessory. Otherwise stop at 8.
- [ ] **Verification:** `cargo build && cargo run --features dev` (manual smoke). The game should reach the dungeon without an asset-load panic. (The integration test in Phase 8 also verifies parsing.)

### Phase 7 — Generate placeholder PNG icons

Per D8 Option A (recommended). Creates the `assets/ui/` and `assets/ui/icons/items/` directories along the way (project has no `assets/ui/` yet — research §Asset Icons).

- [ ] Create `scripts/gen_placeholder_icons.sh` (NEW file). Body uses ImageMagick `convert` to generate one 32×32 PNG per item, with a colored background and a 2-letter code centered. One line per item:

  ```sh
  convert -size 32x32 xc:'#a04040' -gravity center -pointsize 14 -annotate 0 RS \
      assets/ui/icons/items/rusty_sword.png
  ```

  Choose distinct colors per item-kind: weapons red-ish, armor brown, shield gray, consumable green, key item gold. Letter codes: RS, OS, WM, LA, RB, WS, HP, RK (matches the 8-item set).
- [ ] Make the script executable: `chmod +x scripts/gen_placeholder_icons.sh`. Add a usage doc comment at the top: `# Run once: scripts/gen_placeholder_icons.sh. Re-runnable; idempotent.`
- [ ] Run the script: `mkdir -p assets/ui/icons/items && ./scripts/gen_placeholder_icons.sh`. Confirm 8 PNG files are created.
- [ ] If D4 is **B** (12 items), extend the script with 4 more `convert` lines.
- [ ] **Verification:** `ls -la assets/ui/icons/items/` shows the expected files; each file is <5KB. `cargo run --features dev` does not panic at the asset-load step. (The `icon_path` strings are *not* loaded by any system in #12; this is a forward-compat plumbing step. The integration test in Phase 8 will only verify the RON parsing, not the icon files.)

### Phase 8 — Integration test `tests/item_db_loads.rs`

Mirrors `tests/class_table_loads.rs` exactly. Verifies that the live `RonAssetPlugin` path successfully loads `core.items.ron`.

- [ ] Create `tests/item_db_loads.rs` (NEW file) per research §Code Examples Example 6:
  - Imports: `bevy::app::AppExit`, `bevy::asset::AssetPlugin`, `bevy::prelude::*`, `bevy::state::app::StatesPlugin`, `bevy_asset_loader::prelude::*`, `bevy_common_assets::ron::RonAssetPlugin`, `druum::data::{ItemAsset, ItemDb}`, `druum::plugins::party::{EquipSlot, ItemKind}`.
  - `TestAssets` resource with `#[asset(path = "items/core.items.ron")] item_db: Handle<ItemDb>`.
  - `TestState` enum with `Loading` (default) → `Loaded`.
  - The test function `item_db_loads_through_ron_asset_plugin` builds an `App` with `MinimalPlugins, AssetPlugin::default(), StatesPlugin, RonAssetPlugin::<ItemDb>::new(&["items.ron"])`, registers `TestState`, sets up the `LoadingState` to `continue_to_state(TestState::Loaded)` and `load_collection::<TestAssets>`, then schedules `assert_item_db_shape` on `OnEnter(TestState::Loaded)` and a `timeout` system on `Update.run_if(in_state(TestState::Loading))`. Calls `.run()`.
  - `timeout(time: Res<Time>)` panics after 30 seconds.
  - `assert_item_db_shape(assets: Res<TestAssets>, item_dbs: Res<Assets<ItemDb>>, mut exit: MessageWriter<AppExit>)`:
    - Resolve `db = item_dbs.get(&assets.item_db).expect(...)`.
    - Assert `db.items.len() >= 8` (or `== 12` if D4 is B).
    - Resolve `sword = db.get("rusty_sword").expect(...)`. Assert `sword.kind == ItemKind::Weapon`, `sword.slot == EquipSlot::Weapon`, `sword.stats.attack == 5`.
    - Resolve `potion = db.get("healing_potion").expect(...)`. Assert `potion.kind == ItemKind::Consumable`, `potion.slot == EquipSlot::None`.
    - `exit.write(AppExit::Success)`.
- [ ] **Verification:** `cargo test --test item_db_loads`. Test must complete in <30 seconds (the timeout); on a typical dev machine it's <2 seconds.

### Phase 9 — Final verification gate

Mirrors the 7-command gate from Feature #11. Catches anything Phases 1-8 missed.

- [ ] `cargo check` — base build, no features.
- [ ] `cargo check --features dev` — dev build (no startup regressions in `spawn_default_debug_party` after the `Inventory::default()` insert).
- [ ] `cargo clippy --all-targets -- -D warnings` — base clippy; zero warnings.
- [ ] `cargo clippy --all-targets --features dev -- -D warnings` — dev clippy; zero warnings.
- [ ] `cargo fmt --check` — formatting is clean.
- [ ] `cargo test` — all unit + integration tests pass (base).
- [ ] `cargo test --features dev` — all tests pass (dev).
- [ ] `rg 'derive\(.*\bEvent\b' src/plugins/party/inventory.rs` — must return ZERO matches. Confirms `Message` (NOT `Event`) is the derive used (research §Pitfall — Bevy 0.18 family rename).
- [ ] `rg '\bEventReader<' src/plugins/party/inventory.rs && rg '\bEventReader<' tests/item_db_loads.rs` — must return ZERO matches. Confirms `MessageReader` is used.
- [ ] **Manual smoke:** `cargo run --features dev`, navigate via F9 cycler (Title → Loading → TitleScreen → Dungeon). Confirm:
  - No asset-load panic on the new 8-item RON.
  - No startup regression: 4 debug party members spawn with `Inventory::default()` (verify via `--features dev` console log output and Bevy inspector if available).
  - `Tab` does NOTHING (intentional — input is bound but no consumer in #12).
- [ ] Update the `## Implementation Discoveries` section of THIS plan file with any unexpected findings during implementation.
- [ ] Update planner memory (`.claude/agent-memory/planner/`) with a new `project_druum_inventory_equipment.md` entry summarizing the Feature #12 architectural decisions for future planners.

---

## Security

### Known Vulnerabilities

No known CVEs as of 2026-05-05 (research date) for any library used in #12. The dep set is unchanged from Feature #11; same status as #11.

| Library | Version | Status |
|---------|---------|--------|
| serde | 1.x | No advisories |
| ron | 0.12 | No advisories |
| bevy | =0.18.1 | No advisories |
| bevy_common_assets | =0.16.0 | No advisories |
| bevy_asset_loader | =0.26.0 | No advisories |

### Architectural Risks

The trust boundary for #12 is the on-disk `core.items.ron` file, treated as developer-authored (no modding for v1). The risks below are pre-mitigated by the saturating arithmetic already in `derive_stats` and by the type system; explicit guards in #12 close the remaining gaps.

| Risk | How it manifests | Guard required by #12 |
|------|------------------|----------------------|
| Crafted `core.items.ron` with `attack: u32::MAX` | Stat overflow when summed in flatten loop | `derive_stats` uses `saturating_add` (verified at `character.rs:374-381`). Flatten loop in `recompute_derived_stats_on_equipment_change` only `Vec::push`es — no arithmetic in flatten. **SAFE** — no new guard needed. |
| `EquipSlot::None` paired with `kind: Weapon` | Bad data shape passes through `equip_item` unchecked | Step 4 of `equip_item` rejects `slot == None || matches!(kind, Consumable | KeyItem)` with `EquipError::ItemHasNoSlot`. **Layer-1 unit test required** (`equip_consumable_returns_item_has_no_slot`). |
| `equip_item` called with non-`PartyMember` entity (e.g., enemy) | Character entity has no `Equipment` component | `Query::get_mut(entity)` returns `Err` → `EquipError::CharacterMissingComponents`. **No `unwrap()` in helper bodies.** |
| Negative item stats via deserialize | Cannot happen | All stat fields on `ItemStatBlock` are `u32` — no negatives possible. **SAFE by type.** |
| Inventory length attack via crafted save file (#23 territory) | `Vec::with_capacity(1B)` from a malicious save = OOM | Out of scope for #12. **Flag for #23:** `Inventory(Vec<Entity>)` deserialization must bound length. |
| Hot-reload of `core.items.ron` mid-game | Existing equipped handles point to invalidated assets | Out of scope for v1. Defensive `warn!(...)` in flatten loop logs the case if it occurs. |

**Trust boundary recap:** `core.items.ron` is the only untrusted-shape input to #12; all 5 risks above are either pre-mitigated by the type system (saturating math, `u32` non-negativity) or explicitly guarded by `EquipError` returns in `equip_item`. Save-file integrity is #23's problem. No network input (single-player game).

---

## Pitfalls

The 4 pitfalls below are research-flagged. Each appears as a guard inside the relevant Step above; this section is the central reference.

### Pitfall 1 — `Commands::spawn` is deferred; fresh inventory entity isn't queryable in the same frame

**Where it bites:** `unequip_item` and the "previous-item-pushed-back" arm of `equip_item` both `commands.spawn(ItemInstance(...))`. The new entity does NOT exist until `apply_deferred` runs (between systems). A downstream system that queries `&ItemInstance` for the just-spawned entity in the same frame returns nothing.

**Guard in plan:** Step "Phase 3 — equip_item body §7" emits `EquipmentChangedEvent` AFTER the `commands.spawn` and `inventory.0.push`. The recompute system reads the event in `Update` AFTER `apply_deferred` has run. **Do NOT query `&ItemInstance` for the freshly-spawned entity in the same system;** only query in `recompute_derived_stats_on_equipment_change` or later. Doc-comment inside `equip_item` and `unequip_item` calls this out.

### Pitfall 2 — `EquipSlot` variant ↔ `Equipment` field name mapping (PascalCase vs snake_case)

**Where it bites:** `Equipment` has fields `weapon`, `armor`, `shield`, `helm`, `gloves`, `boots`, `accessory_1`, `accessory_2` (snake_case). `EquipSlot` enum variants are `Weapon`, `Armor`, ..., `Accessory1`, `Accessory2` (PascalCase). Direct enum-to-field mapping is mechanical but easy to mis-type, especially `Accessory1` ↔ `accessory_1`.

**Guard in plan:** Step "Phase 2 — `impl EquipSlot::read/write`" centralizes the mapping in two `match` blocks. Step "Phase 3 — Layer-1 test `equip_slot_read_write_round_trip`" verifies all 8 slots round-trip. **Do NOT reach for the field names directly anywhere except inside `read`/`write`.** All other code paths must go through `slot.read(...)` and `slot.write(...)`.

### Pitfall 3 — `Handle<ItemAsset>` clone cheap but `Asset::get` returns `Option<&T>`

**Where it bites:** The flatten step `items.get(handle)` returns `Option<&ItemAsset>`. The asset might not be loaded if hot-reload or a freshly-spawned handle has temporarily unloaded.

**Guard in plan:** Step "Phase 4 — recompute body §1c" adds a defensive `warn!(...)` when `items.get(handle)` is `None` and skips that slot in the flatten loop. **Do NOT push `ItemStatBlock::default()` as a fallback** — that would silently produce 0 stats and mask the failure. Skipping is honest; the `warn!` surfaces the issue.

For v1: assume the `Loading -> Dungeon` state path guarantees the bundle asset is loaded; this guard is purely defensive for future hot-reload (#dev-features) work.

### Pitfall 4 — `assets/items/core.items.ron` (NOT `items.ron`)

**Where it bites:** Roadmap line 659 says `assets/items/items.ron`. The actual loader at `loading/mod.rs:33` expects `items/core.items.ron` (no `core.` was actually a stale doc — `loading/mod.rs:33` is the locked path). Following the roadmap creates a file the loader doesn't see.

**Guard in plan:** Step "Phase 6" explicitly says `assets/items/core.items.ron` and notes that the file already exists as a 3-line stub. **Do NOT create a second file.** **Do NOT touch `loading/mod.rs`** — the path is locked and the LoadingPlugin is FROZEN post-#3.

### Additional pitfall — `EquipSlot::None` for `Consumable`/`KeyItem`

The `EquipSlot::None` variant's purpose is to give `Consumable` and `KeyItem` a sentinel value so they round-trip cleanly through RON (`slot: None`). The `equip_item` validation must reject *both*:
- `slot == EquipSlot::None` (caller asking to equip into nowhere — nonsensical), AND
- `asset.slot == EquipSlot::None || matches!(asset.kind, ItemKind::Consumable | ItemKind::KeyItem)` (asset declares it's not equippable).

The double-check (caller-side AND asset-side) is intentional defense-in-depth: a `Consumable` with a wrongly-authored `slot: Weapon` is rejected by the kind check; a `Weapon` with `slot: None` is rejected by the slot check. Both rejections return `EquipError::ItemHasNoSlot`. Layer-1 test `equip_consumable_returns_item_has_no_slot` exercises one path; `equip_slot_mismatch_rejected` exercises the slot-mismatch path; explicitly add a third test `equip_weapon_with_slot_none_in_asset_rejected` for completeness if the user wants exhaustive coverage.

---

## Implementation Discoveries

*(Empty at plan time. Populate during implementation with unexpected findings, wrong assumptions, API quirks, edge cases, and fixes applied. Mirror Features #9, #10, #11 — the implementer fills this in commit-by-commit.)*

---

## Estimated impact

Confirms the roadmap budget at line 666-670:

| Dimension | Roadmap baseline | Plan-of-record |
|-----------|------------------|----------------|
| LOC Δ | +400 to +600 | **+450-550** (`inventory.rs` ~400 + `mod.rs` +20 + `items.rs` +60 + `core.items.ron` ~80 lines + `tests/item_db_loads.rs` ~70) |
| Deps Δ | 0 | **0 — Cargo.toml byte-unchanged** |
| Compile Δ | small (+0.3s) | **+0.3s** (one new module, no new deps) |
| Asset Δ | +1 RON, +5-10 icons | **+1 RON (replacement) + 8 icons** (D4 = 8 items, D8 = ImageMagick) |
| Test count Δ | +6-10 | **+9** (Layer-1: 6 — round-trips, slot read/write, validation rejects; Layer-2: 3 — recompute, equip-emit, give-item-push; +1 integration via `tests/item_db_loads.rs` = 10 total). |

**Cleanest-ship signal:** Same as Features #7, #8, #9, #11 — `Cargo.toml` byte-unchanged, `Cargo.lock` byte-unchanged.

---

## Verification

- [ ] `data/items.rs` Layer-1 round-trip tests pass — Layer-1 unit — `cargo test data::items::tests` — Automatic
- [ ] `inventory.rs` Layer-1 slot-validation tests pass — Layer-1 unit — `cargo test plugins::party::inventory::tests` — Automatic
- [ ] `inventory.rs` Layer-2 recompute test passes — Layer-2 integration — `cargo test plugins::party::inventory::app_tests::equip_sword_raises_attack_unequip_lowers` — Automatic
- [ ] `inventory.rs` Layer-2 message-emit test passes — Layer-2 integration — `cargo test plugins::party::inventory::app_tests::equip_emits_message_via_helper` — Automatic
- [ ] `inventory.rs` Layer-2 give-item test passes — Layer-2 integration — `cargo test plugins::party::inventory::app_tests::give_item_pushes_to_inventory` — Automatic
- [ ] `core.items.ron` parses through `RonAssetPlugin` — Integration — `cargo test --test item_db_loads` — Automatic
- [ ] `cargo check && cargo check --features dev` — Build — Automatic
- [ ] `cargo clippy --all-targets -- -D warnings && cargo clippy --all-targets --features dev -- -D warnings` — Lint — Automatic
- [ ] `cargo fmt --check` — Format — Automatic
- [ ] `cargo test && cargo test --features dev` — Full test suite — Automatic
- [ ] No `Event` derive sneaks in — Grep — `rg 'derive\(.*\bEvent\b' src/plugins/party/inventory.rs` — Automatic (must return ZERO matches; only `Message` is allowed)
- [ ] No `EventReader` consumer sneaks in — Grep — `rg '\bEventReader<' src/plugins/party/inventory.rs tests/item_db_loads.rs` — Automatic (must return ZERO matches)
- [ ] No edits to frozen files — Grep — `git diff --name-only main HEAD -- src/plugins/party/character.rs src/plugins/loading/mod.rs src/plugins/state/mod.rs src/plugins/input/mod.rs src/plugins/dungeon/mod.rs src/plugins/audio/ src/plugins/ui/ src/data/dungeon.rs src/data/spells.rs src/data/enemies.rs src/data/classes.rs src/main.rs Cargo.toml Cargo.lock` — Automatic (must return ZERO matches)
- [ ] No asset-load panic at startup — Smoke — `cargo run --features dev` and navigate to Dungeon via F9 cycler — Manual
- [ ] 4 debug party members spawn with empty `Inventory` — Smoke — `cargo run --features dev` and confirm console logs `Spawned 4 debug party members`; Inventory component visible in inspector if available — Manual
- [ ] Tab key has no effect (no UI consumer in #12) — Smoke — In-game press Tab; nothing should happen — Manual
- [ ] 8 placeholder PNG icons exist — File-existence — `ls -1 assets/ui/icons/items/ | wc -l` — Automatic (must equal 8 for D4=A or 12 for D4=B)
- [ ] Plan's "Implementation Discoveries" section populated — Documentation — manual review of THIS plan file post-implementation — Manual

---

## Notes for the orchestrator

- D2, D3, D7 are RECOMMENDED defaults — proceed unless the user objects when surfacing this plan.
- D4 (item count: 8 vs 12), D5 (debug starter-items system: ship vs skip), D8 (icon production approach: ImageMagick vs skip vs CC0 pack) are genuine USER PICK decisions — surface in the orchestrator's final report.
- D1, D6 are auto-resolved by #11's already-shipped code — only surface if user asks to reverse Equipment shape.
- The implementer MUST commit Phase 2 before Phase 1 (Phase 1's `data/items.rs` references types defined in Phase 2's `inventory.rs`). The plan documents this in the Phase 1 commit-ordering note.
- The Layer-2 tests in `inventory.rs` require Phase 5's plugin registration to be live; implementer should commit Phase 4 + Phase 5 together as a single logical commit (or as two commits where Phase 4 lands the test code marked `#[ignore]`, and Phase 5 lands the plugin registration AND removes the `#[ignore]`). Pick whichever boundary the implementer prefers; both keep the tree green.
