# Feature #12 — Inventory & Equipment — Research

**Researched:** 2026-05-05
**Domain:** Druum / Bevy 0.18.1 / DRPG inventory + equipment data layer
**Confidence:** HIGH on the live #11 ground truth, on the Bevy 0.18 Message API, on the RON loader pattern, and on the dep-delta analysis. MEDIUM on the slot-validation pattern recommendation (multiple genre-valid options; recommendation is grounded in project precedent, not in a single canonical Bevy idiom). MEDIUM on the items.ron content sketch (designer-balance, not technical).

---

## Summary

Feature #12 lands on top of an unusually-detailed Feature #11 foundation. Crucially, **#11's `Equipment` component already stores `Option<Handle<ItemAsset>>` per slot — not `Option<Entity>` as the roadmap line 647 suggests** — and `derive_stats(...)` already takes `&[ItemStatBlock]` flattened by the caller, not `&Equipment`. The roadmap's whole "item-as-entity" framing is partially obsolete: equipment is keyed by handle, and only the inventory's per-character `Vec<Entity>` carries entity-per-item semantics. This narrows #12's design space considerably and means the planner's primary unresolved questions are: (a) what entity-vs-asset model does `Inventory` use, (b) where does `EquipSlot` enum live, (c) how do `equip`/`unequip` validate slot/kind compatibility, and (d) what does the asset pipeline for `items.ron` and 32×32 icons look like.

This research grounds every type/event in the live Feature #11 code at exact file:line references, distinguishes which roadmap text is now stale, and surfaces five Category B decisions plus three additional ones discovered during investigation. The recommendation is to mirror #11's `Handle<ItemAsset>`-based equip path and use a per-character `Inventory(Vec<Entity>)` component for the *inventory only* — items in the bag are entities (so individual potions can be swapped, used, and given), but equipped items reduce to handles for `derive_stats`'s pure flatten path. Stackable items remain explicitly punted per roadmap.

**Primary recommendation:** Implement #12 as: (1) flesh out `ItemAsset` in `src/data/items.rs` with `kind`, `slot`, `weight`, `value`, plus per-instance state stripped out (kept on the `ItemInstance` entity); (2) create `src/plugins/party/inventory.rs` (NEW file — does not exist yet) holding `Inventory(Vec<Entity>)`, `ItemInstance(Handle<ItemAsset>)`, `EquipSlot` enum, `EquipmentChangedEvent` (a Bevy `Message`), and `equip` / `unequip` / `give_item` systems; (3) add `recompute_derived_stats_on_equipment_change` system that reads `MessageReader<EquipmentChangedEvent>` and calls into #11's existing `derive_stats`; (4) populate `assets/items/core.items.ron` with 8-12 starter items; (5) source 5-10 placeholder 32×32 icons under `assets/ui/icons/items/` (NEW directory). **Zero new dependencies.**

---

## Live #11 ground truth (the implementer must mirror these)

These are the load-bearing facts from the merged #11 code that contradict or refine the roadmap. Read these before designing anything.

### A. `Equipment` already stores `Handle<ItemAsset>`, not `Entity`

**File:** `/Users/nousunio/Repos/Learnings/claude-code/druum/src/plugins/party/character.rs:209-219`

```rust
#[derive(Component, Reflect, Default, Debug, Clone, PartialEq)]
pub struct Equipment {
    pub weapon: Option<Handle<ItemAsset>>,
    pub armor: Option<Handle<ItemAsset>>,
    pub shield: Option<Handle<ItemAsset>>,
    pub helm: Option<Handle<ItemAsset>>,
    pub gloves: Option<Handle<ItemAsset>>,
    pub boots: Option<Handle<ItemAsset>>,
    pub accessory_1: Option<Handle<ItemAsset>>,
    pub accessory_2: Option<Handle<ItemAsset>>,
}
```

Eight slots are already declared. **#12 does not redefine `Equipment`** — it consumes it. The roadmap line 647 ("research §Pattern 3 references `Equipment` fields holding `Entity`") is **stale** — #11 resolved Decision 3 to `Handle<ItemAsset>` and shipped it. See `character.rs:194-208` doc comment for the rationale (Handle serializes for #23 save/load without `MapEntities` dance; per-instance state lives on a separate entity).

The doc-comment also pre-stages #12: *"Per-instance state (enchantment, durability, custom name) lands in #12 as a separate `ItemInstance` entity model."* This is the green light — #12 owns `ItemInstance`.

### B. `derive_stats` is pure and takes `&[ItemStatBlock]`, not `&Equipment`

**File:** `/Users/nousunio/Repos/Learnings/claude-code/druum/src/plugins/party/character.rs:343-348`

```rust
pub fn derive_stats(
    base: &BaseStats,
    equip_stats: &[ItemStatBlock],
    status: &StatusEffects,
    level: u32,
) -> DerivedStats { ... }
```

The function does **NOT** take `&Equipment`. It takes a flattened slice of `ItemStatBlock`. The doc comment at lines 322-326 is explicit: *"Callers are responsible for flattening `Equipment` + `Assets<ItemAsset>` into `&[ItemStatBlock]`; this keeps `derive_stats` testable without asset access."*

This means **#12's `recompute_derived_stats_on_equipment_change` system must own the flatten step** — read each `Option<Handle<ItemAsset>>` slot, look it up in `Res<Assets<ItemAsset>>`, extract the `ItemStatBlock`, build a `Vec<ItemStatBlock>`, then call `derive_stats(&base, &equip_stats, &status, level)`. **Do not modify `derive_stats`.**

### C. `ItemAsset` and `ItemStatBlock` exist as #11 stubs ready to be fleshed out

**File:** `/Users/nousunio/Repos/Learnings/claude-code/druum/src/data/items.rs`

```rust
// items.rs:30-47 — ItemStatBlock with 8 fields, all #[serde(default)]
pub struct ItemStatBlock {
    #[serde(default)] pub attack: u32,
    #[serde(default)] pub defense: u32,
    #[serde(default)] pub magic_attack: u32,
    #[serde(default)] pub magic_defense: u32,
    #[serde(default)] pub accuracy: u32,
    #[serde(default)] pub evasion: u32,
    #[serde(default)] pub hp_bonus: u32,
    #[serde(default)] pub mp_bonus: u32,
}

// items.rs:55-59 — ItemAsset is a one-field stub; #12 fleshes it out
pub struct ItemAsset {
    pub stats: ItemStatBlock,
}
```

`#12` extends `ItemAsset` with `name: String`, `kind: ItemKind`, `slot: EquipSlot`, `weight: u32`, `value: u32`, `icon_path: String` (or `Handle<Image>`), `stackable: bool` (declared but punted — see roadmap line 681).

`ItemDb` (also at `items.rs:13-18`) is currently `pub struct ItemDb {}` — empty body. **#12 fleshes `ItemDb { items: Vec<ItemAsset> }`** OR keeps individual `Handle<ItemAsset>` references and uses `ItemDb` only as the asset-aggregator (see Decision D2 below).

### D. The RON loader path is already wired and the asset path is locked

**File:** `/Users/nousunio/Repos/Learnings/claude-code/druum/src/plugins/loading/mod.rs:33-34, 96-102`

```rust
// In DungeonAssets:
#[asset(path = "items/core.items.ron")]
pub item_db: Handle<ItemDb>,

// In LoadingPlugin::build:
RonAssetPlugin::<ItemDb>::new(&["items.ron"]),
```

**Asset path is `assets/items/core.items.ron`** (the file already exists as a 3-line stub at `/Users/nousunio/Repos/Learnings/claude-code/druum/assets/items/core.items.ron`). This is identical to the path the #11 plan locked for `core.classes.ron`. The roadmap line 659 says `assets/items/items.ron` — that path is **wrong**; use `core.items.ron`. This is the same precedent #11 followed (Pitfall 2 of #11 research).

**`LoadingPlugin` is FROZEN post-#3.** Do not touch `mod.rs`. The handle is already loaded; #12 only edits the body of `ItemDb` and the body of `core.items.ron`.

### E. `Message` is the right derive (NOT `Event`) and `MessageReader<T>` is the consumer

**Verified at:** `/Users/nousunio/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bevy_ecs-0.18.1/src/message/mod.rs:23` (`pub use bevy_ecs_macros::Message;`)

**Project precedents:**
- `MovedEvent` at `src/plugins/dungeon/mod.rs:192-197` — `#[derive(Message, Clone, Copy, Debug)]`, registered with `app.add_message::<MovedEvent>()` at line 207, read with `MessageReader<MovedEvent>` at minimap.rs and elsewhere.
- `SfxRequest` at `src/plugins/audio/sfx.rs:42-45` — same pattern.

**For #12:** `EquipmentChangedEvent` derives `Message`. Do **NOT** derive `Event`. Register with `app.add_message::<EquipmentChangedEvent>()` in `PartyPlugin::build`. Subscribers use `MessageReader<EquipmentChangedEvent>`. The roadmap text says "event" colloquially; the type IS a `Message` in 0.18.

The reverse (`MessageWriter<T>::write(...)`) writer pattern is canonical; `dungeon/mod.rs:686-690` shows it inline.

### F. `PartyPlugin` is registered; debug party spawns 4 members on `OnEnter(Dungeon)`

**File:** `/Users/nousunio/Repos/Learnings/claude-code/druum/src/plugins/party/mod.rs`

- `PartyPlugin` at line 16 — already in `main.rs:32`.
- `init_resource::<PartySize>` at line 21.
- 14 type registrations at lines 25-38 (every Reflect type).
- `spawn_default_debug_party` at line 62, gated `#[cfg(feature = "dev")]` and triggered on `OnEnter(GameState::Dungeon)` at line 47.

**For #12:** add `add_message::<EquipmentChangedEvent>()` to `PartyPlugin::build`. Add the equip/unequip/give_item systems and the recompute system. Add `register_type::<Inventory>()`, `register_type::<ItemInstance>()`, `register_type::<EquipSlot>()`, `register_type::<ItemKind>()`. Module declaration: `pub mod inventory;` underneath `pub mod character;` at line 8.

**Re-export pattern from line 10:** `pub use character::{ ... };`. Mirror with `pub use inventory::{ Inventory, ItemInstance, EquipSlot, ItemKind, EquipmentChangedEvent, EquipError };`.

### G. `DungeonAction::OpenInventory` is already bound to Tab

**File:** `/Users/nousunio/Repos/Learnings/claude-code/druum/src/plugins/input/mod.rs:80, 151`

The variant exists (`DungeonAction::OpenInventory`) and is bound to `KeyCode::Tab`. **#12 does NOT need to add input.** The Inventory UI screen is Feature #25 (out of scope), so #12 doesn't even consume this binding — but the binding exists for #25.

### H. `DungeonSubState::Inventory` is already declared

**File:** `/Users/nousunio/Repos/Learnings/claude-code/druum/src/plugins/state/mod.rs:23`

Already defined. **#12 doesn't add a state**, doesn't add a UI screen, doesn't run on `OnEnter(DungeonSubState::Inventory)`. That's #25.

### Stale-roadmap summary

| Roadmap claim | Reality |
|--------------|---------|
| Line 647: "`Equipment` fields holding `Entity`" | False — #11 ships `Handle<ItemAsset>` per Decision 3. |
| Line 659: `assets/items/items.ron` | False — actual path is `assets/items/core.items.ron`. |
| Line 657: "`src/plugins/party/inventory.rs` for components, resources, and equip systems" | True (file does NOT exist yet — #12 creates it). |
| Line 658: "`src/data/items.rs` for item definitions" | Partly true — file exists with stubs; #12 fleshes the body. |
| Line 660: "`EquipmentChangedEvent` that triggers stat re-derivation" | True, but it derives `Message`, not `Event`. |
| Line 675: "`Inventory(Vec<Entity>)` component (decision: per-character, not pooled — Wizardry-style)" | Resolved decision; mirror it. |
| Line 681: stackable items punted | Mirror it; do not design a `StackableItem` component. |

---

## Standard Stack

### Core (already in deps — no Δ)

| Library | Version | Purpose | License | Maintained? | Why Standard |
|---------|---------|---------|---------|-------------|--------------|
| [bevy](https://crates.io/crates/bevy) | =0.18.1 | `Component`, `Message`, `Plugin`, `Asset`, `Handle<T>`, `Reflect` | MIT/Apache-2.0 | Active | Engine — already pinned in Cargo.toml |
| [serde](https://crates.io/crates/serde) | 1 (`derive`) | `#[derive(Serialize, Deserialize)]` on `ItemAsset`, `ItemKind`, `EquipSlot` | MIT/Apache-2.0 | Active | Already in deps. Required for new RON-loaded item definitions. |
| [ron](https://crates.io/crates/ron) | 0.12 | Pure-stdlib round-trip tests for `ItemDb` and `ItemAsset` | MIT/Apache-2.0 | Active | Already in deps. The standard test pattern for #11's `BaseStats` round-trip translates directly. |
| [bevy_common_assets](https://crates.io/crates/bevy_common_assets) | =0.16.0 | `RonAssetPlugin::<ItemDb>` — already registered at `loading/mod.rs:98` | MIT/Apache-2.0 | Active | Loader is wired; #12 changes the body of `ItemDb` only. |
| [bevy_asset_loader](https://crates.io/crates/bevy_asset_loader) | =0.26.0 | `item_db: Handle<ItemDb>` already in `DungeonAssets` at `loading/mod.rs:34` | Apache-2.0 | Active | Already in deps. |

### Supporting (NOT used in #12)

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| [moonshine-save](https://crates.io/crates/moonshine-save) | (deferred to #23) | Selective ECS save | Feature #23. The `ItemInstance` and `Inventory(Vec<Entity>)` shape shipped in #12 must round-trip for #23 — surface this as a sub-requirement (see Decision D2). |
| [bevy_egui](https://crates.io/crates/bevy_egui) | =0.39.1 | Inventory UI | Feature #25. **OUT OF SCOPE for #12.** |
| [smallvec](https://crates.io/crates/smallvec) | (transitive) | `SmallVec<[Entity; 8]>` for `Inventory` | YAGNI for v1. `Vec<Entity>` is fine. |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|-----------|-----------|----------|
| Per-character `Inventory(Vec<Entity>)` | Pooled `PartyInventory: Resource(Vec<Entity>)` | The roadmap line 675 already resolved this to per-character (Wizardry-style). Per-character is the genre canon and avoids the "who owns this potion?" lookup at gift/transfer time. Recommendation: keep per-character. |
| `Inventory(Vec<Entity>)` | `Inventory(Vec<Handle<ItemAsset>>)` (no entity-per-item) | Simpler model; would mean a "potion" is just a handle, not an entity. **Cons:** no per-instance state at all (every potion is identical, no enchantment, no durability). **Pros:** trivially serializable, no `MapEntities`. Surfaced as Decision D2. Recommendation: **keep entity-per-item** because the doc comment at `character.rs:204-205` already promises "Per-instance state (enchantment, durability, custom name) lands in #12 as a separate `ItemInstance` entity model." Reverting that promise would need explicit user approval. |
| `ItemDb { items: Vec<ItemAsset> }` (single bundle asset) | Per-item file: one `*.item.ron` per item, loaded individually | Per-item files scale better but explode the loader registration surface and double the asset-Δ count. The bundle approach is consistent with `ClassTable { classes: Vec<ClassDef> }` from #11. Recommendation: bundle. |
| `Result<(), EquipError>` slot validation | Panic on invalid slot | Hard panics break the game; `Result` is idiomatic Rust. Recommendation: `Result`. See "Slot validation pattern" below. |
| `Result<(), EquipError>` | Silent no-op (log warning) | Silent no-ops mask bugs. UI in #25 wants to surface "can't equip dagger in armor slot" as feedback. Recommendation: `Result` and let callers log/render. |
| Each `equip` / `unequip` is a system | `Commands`-based fallible API on the world | Systems compose better with Bevy's scheduler. Recommendation: systems. |

**Installation:** No new dependencies. Cargo.toml is byte-unchanged for #12. Same cleanest-ship signal as #9, #10, #11.

---

## Architecture Options

Three fundamentally different ways to structure the inventory layer. Pick one before writing code.

| Option | Description | Pros | Cons | Best When |
|--------|-------------|------|------|-----------|
| **A: Hybrid — Equipment uses Handles (#11), Inventory uses Entities** [RECOMMENDED] | `Equipment` slots stay `Option<Handle<ItemAsset>>` exactly as #11 ships. `Inventory(Vec<Entity>)` is a **per-character component** holding entities that each carry an `ItemInstance(Handle<ItemAsset>)` component plus optional per-instance state components (`Enchantment(u8)`, `Durability(u32)` — declared but unused in v1). When the player equips an item from inventory: the entity stays alive, the entity's handle is copied into the appropriate `Equipment` slot, and the entity is removed from the `Inventory(Vec<Entity>)`. When unequipped: the handle is read from `Equipment`, a new entity is spawned with `ItemInstance(handle.clone())`, and pushed onto `Inventory`. | Equipped items integrate with `derive_stats` via the existing handle-flatten path — zero changes to `derive_stats`. Inventory items can carry per-instance state (enchantment, durability) on the entity — the entity-per-item flexibility the roadmap originally wanted. Save/load (#23) for `Equipment` is `Handle<ItemAsset>` (auto-serializable as path) and for `Inventory(Vec<Entity>)` requires `MapEntities` only on the inventory side — half the pain. Stackable potions in #12 v1 are simply N entities with the same handle (suboptimal but matches the roadmap punt at line 681). | Equip/unequip mutates two ECS shapes (the `Equipment` slot and the `Inventory` Vec). Test surface is moderate (4-5 tests cover the happy paths). The "spawn entity on unequip" arm has a subtle ordering trap: `Commands` defers, so the freshly-spawned entity isn't queryable in the same frame — surface as Pitfall 1 below. | Project precedent says "Handle for Equipment, Entity for per-instance state" (`character.rs:204-205`). The per-character `Inventory` matches genre canon. Decision D2 confirms; this is the strongly-recommended option. |
| B: Pure entity model — Equipment also holds Entities | Both `Equipment` and `Inventory` hold `Option<Entity>`. Re-shape `Equipment` to `pub struct Equipment(pub [Option<Entity>; 8])`. Each item entity carries `ItemInstance(Handle<ItemAsset>)`, `EquipSlot`, plus per-instance state. `derive_stats` callers query `Query<&ItemInstance>` to flatten. | Maximum ECS uniformity. Items are first-class entities everywhere. Per-instance state on equipped items is native. | **REVERTS Feature #11's resolved Decision 3 and the doc-comment promise at `character.rs:194-208`.** Requires implementing `MapEntities` on `Equipment` for #23 save/load (the exact pain #11 avoided). `derive_stats`'s pure signature is stressed because the flatten step now needs `Query<&ItemInstance>` which is a system parameter — the wrapper system gains complexity. | Only if the user explicitly chooses to reverse #11's Decision 3. Cost: ~80 LOC of `MapEntities` plumbing + ~40 LOC of `Equipment` redesign + a save/load round-trip integration test that wasn't required before. **Not recommended.** Surface as Decision D6 only if the user asks. |
| C: Pure handle model — Inventory also holds Handles | `Inventory(Vec<Handle<ItemAsset>>)`. No item entities at all. `equip` / `unequip` is a pure `Vec` ↔ `Equipment` slot move. | Simplest. No `MapEntities` ever needed. Inventory serializes as `Vec<AssetPath>` cleanly. | **No per-instance state ever.** Every potion is identical. A "+3 Sword of Sharpness" cannot exist as a unique instance — it would have to be its own `ItemAsset`. Stackable potions are still N entries in the Vec (same as Option A's "N entities" but less general). Reverses the doc-comment promise at `character.rs:204-205`. | Only if the user commits to a "static items" game (every Healing Potion is identical, no enchantments, no Diablo-style affixes). Genre-valid for some DRPGs (e.g., classic Wizardry I-V); modern Wizardry/Etrian use per-instance state. **Surface as Decision D2 alternative.** |

**Recommended: Option A — Hybrid (Equipment = Handle, Inventory = Entity)**

Rationale: it preserves #11's Decision 3 (the implementer of #11 explicitly chose this and shipped it), it honors the doc-comment promise that "per-instance state lands in #12 as a separate `ItemInstance` entity model", it minimizes #23 save/load pain (`MapEntities` is required only on the inventory list, not on equipped slots), and it gives the planner a clean way to gradually layer per-instance components (enchantment, durability) on the inventory entities without touching `Equipment`. The "Commands deferral" pitfall is well-understood (Pitfall 1 below) and the workaround is a one-frame-delay or an immediate-mode `world.spawn`.

### Counterarguments

Why someone might NOT choose Option A:

- **"Two different identifier shapes (Handle vs Entity) is confusing."** — Response: Yes, mildly. But the boundary is crisp: equipped = handle, in-bag = entity. Once an item is *equipped*, its instance state is irrelevant for `derive_stats` (only the static stat block matters), so the handle is a sufficient identifier. The boundary maps to a real semantic: "this sword is currently worn vs. sitting in the bag." Surface explicitly in inline doc comments.

- **"What about a +3 enchanted sword that's currently equipped?"** — Response: This is a known limitation of Option A. The handle in the `Equipment` slot points to the base asset; the +3 modifier would be lost on equip. **For v1, items have no per-instance state**, so this isn't an issue. When per-instance enchantment lands (post-v1, after #21 loot tables), the model evolves to: equipped enchanted items are stored as a separate `EnchantedEquipment` map keyed by `EquipSlot` carrying an `Entity` reference, OR `Equipment` slots become `EquipmentRef::Static(Handle<ItemAsset>) | Enchanted(Entity)`. **For #12, neither shape is required** — surface as Open Question OQ3.

- **"Why not just make Equipment slots Entity from the start (Option B) to future-proof?"** — Response: Because the cost (reverting #11's Decision 3, adding `MapEntities`, retrofitting `derive_stats`'s flatten) is real and immediate, while the benefit (graceful future enchantment) is speculative and post-v1. The project's discipline (`feedback_third_party_crate_step_a_b_c_pattern.md`) is to evolve when the second consumer arrives, not pre-engineer. Surface as Decision D6 if user asks.

---

## Architecture Patterns

### Recommended Project Structure

```
src/
├── data/
│   └── items.rs              # FROZEN-from-day-one — body fleshed out (was 1-field stub)
├── plugins/
│   └── party/
│       ├── mod.rs            # MODIFY — add `pub mod inventory;`, re-exports, register message + types
│       ├── character.rs      # NO CHANGE (FROZEN by #11)
│       └── inventory.rs      # NEW — Inventory, ItemInstance, EquipSlot, ItemKind, EquipmentChangedEvent, equip systems, EquipError
└── ...

assets/
├── items/
│   └── core.items.ron        # MODIFY — replace `()` stub with 8-12 starter items
└── ui/
    └── icons/
        └── items/             # NEW DIR — 5-10 placeholder 32x32 PNGs
            ├── rusty_sword.png
            ├── leather_armor.png
            └── ...

tests/
└── item_db_loads.rs           # NEW — integration test mirroring tests/class_table_loads.rs
```

**No edits to:** `src/main.rs`, `src/plugins/state/mod.rs`, `src/plugins/loading/mod.rs`, `src/plugins/dungeon/mod.rs`, `src/plugins/input/mod.rs`, `src/plugins/audio/mod.rs`, `src/plugins/combat/mod.rs`, `src/plugins/ui/mod.rs`, `src/plugins/save/mod.rs`, `src/plugins/town/mod.rs`, `src/data/dungeon.rs`, `src/data/spells.rs`, `src/data/enemies.rs`, `src/data/classes.rs`, `src/plugins/party/character.rs`. Same cleanest-ship signal as #11.

**Single-file precedent.** Per Decision 4 of #11 (and #9, #10): everything in one `inventory.rs` file. The 12 pieces (5 components, 1 enum × 2, 1 message, 1 error, 4-5 systems, tests) fit comfortably under 800 LOC.

### Pattern 1: ItemAsset fleshed out, ItemDb as bundle

```rust
// src/data/items.rs — replaces #11 stub
//
// Source: mirrors src/data/classes.rs (the FROZEN-from-day-one #11 precedent)

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::plugins::party::inventory::{EquipSlot, ItemKind};
// Note: same one-way reverse-dep as classes.rs — data/ imports from plugins/.

#[derive(Asset, Reflect, Serialize, Deserialize, Default, Debug, Clone)]
pub struct ItemDb {
    pub items: Vec<ItemAsset>,
}

impl ItemDb {
    /// Look up an `ItemAsset` by its `id` field (linear scan; <12 items in v1).
    /// Pattern parallel to `ClassTable::get` at classes.rs:41-43.
    pub fn get(&self, id: &str) -> Option<&ItemAsset> {
        self.items.iter().find(|i| i.id == id)
    }
}

#[derive(Asset, Reflect, Serialize, Deserialize, Default, Debug, Clone)]
pub struct ItemAsset {
    /// Stable, human-readable identifier — used as the lookup key in
    /// `ItemDb::get` and as the asset-path stem for the icon (e.g.
    /// `id = "rusty_sword"` → icon at `ui/icons/items/rusty_sword.png`).
    pub id: String,

    /// Display name shown in inventory UI (#25). Distinct from `id` so
    /// the UI string can be localized later without breaking save files.
    pub display_name: String,

    /// Static stat contribution; flattened by the equip-recompute system
    /// and passed to `derive_stats` as a slice of `ItemStatBlock`.
    pub stats: ItemStatBlock,

    /// Item category — drives validation and UI grouping.
    pub kind: ItemKind,

    /// Which slot this item occupies when equipped. `EquipSlot::None`
    /// for `Consumable` and `KeyItem` kinds.
    pub slot: EquipSlot,

    /// Encumbrance contribution — reserved for v1 (no carry-weight cap yet).
    #[serde(default)]
    pub weight: u32,

    /// Sale value at shops (#18) and loot rolls (#21).
    #[serde(default)]
    pub value: u32,

    /// Asset-relative icon path under `assets/`. Common pattern:
    /// `ui/icons/items/{id}.png`. Handle resolution is deferred to #25
    /// (inventory UI); v1 stores the path as a String to avoid binding
    /// `Image` handles in the data layer.
    #[serde(default)]
    pub icon_path: String,

    /// Roadmap line 681: stackable items are PUNTED. This flag is
    /// declared so the schema is forward-compatible, but no system
    /// reads it in v1. Unique-per-entity is the v1 model.
    #[serde(default)]
    pub stackable: bool,
}

// ItemStatBlock STAYS UNCHANGED from #11 (items.rs:30-47).
// 8 fields, all #[serde(default)], no edits required.
```

### Pattern 2: ItemKind and EquipSlot in inventory.rs

```rust
// src/plugins/party/inventory.rs — NEW file

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// Item category. Drives UI grouping (#25) and validation:
/// only `Weapon`/`Shield`/`Armor`/`Helm`/`Gloves`/`Boots`/`Accessory` kinds
/// can be equipped; `Consumable` and `KeyItem` cannot.
#[derive(Reflect, Serialize, Deserialize, Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ItemKind {
    #[default]
    Weapon,
    Shield,
    Armor,
    Helm,
    Gloves,
    Boots,
    Accessory,
    Consumable,
    KeyItem,
}

/// Equipment slot, mapping 1:1 onto the eight `Option<Handle<ItemAsset>>`
/// fields on the `Equipment` component declared at character.rs:209-219.
///
/// `None` is for items that don't equip (consumables, key items). The
/// `equip` system rejects `None` with `EquipError::ItemHasNoSlot`.
///
/// **Discriminant order is locked** for save-format stability — same
/// rule as `Class`, `Race`, `StatusEffectType` in #11.
#[derive(Reflect, Serialize, Deserialize, Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EquipSlot {
    #[default]
    None,
    Weapon,
    Shield,
    Armor,
    Helm,
    Gloves,
    Boots,
    Accessory1,
    Accessory2,
}
```

### Pattern 3: ItemInstance and Inventory components

```rust
/// One inventory item entity. The handle resolves to the static
/// `ItemAsset` definition. Per-instance state (enchantment, durability,
/// custom name) layers on as additional components on the same entity
/// when post-v1 features need them. **For v1, the entity carries
/// only `ItemInstance` and nothing else** — but the entity model is
/// the open door for that future flexibility.
///
/// **No `Serialize`/`Deserialize`** — `Handle<T>` does not implement
/// serde in Bevy 0.18 (per the doc comment at character.rs:196-202).
/// Save/load (#23) implements custom serde mapping `Handle<ItemAsset>` ↔
/// `AssetPath`, mirroring what #23 must already do for `Equipment`.
#[derive(Component, Reflect, Default, Debug, Clone, PartialEq)]
pub struct ItemInstance(pub Handle<ItemAsset>);

/// Per-character inventory list. Each `Entity` in the Vec carries an
/// `ItemInstance` component. Order matters (UI display order); push to
/// the end on `give_item`, swap-remove on `equip` / consume.
///
/// **No `Serialize`/`Deserialize`** — `Vec<Entity>` requires `MapEntities`
/// for save/load (#23 territory), the same way `Equipment` requires
/// custom Handle-as-AssetPath serde. This is one of two save-bridges
/// #23 must build for #12.
///
/// **Capacity:** `Vec<Entity>` (no cap in v1). A carry-weight system or
/// slot-count cap is post-v1 (#21 loot tables may surface the need).
#[derive(Component, Reflect, Default, Debug, Clone, PartialEq)]
pub struct Inventory(pub Vec<Entity>);
```

### Pattern 4: EquipmentChangedEvent — the canonical Bevy 0.18 Message

```rust
/// Emitted once per successful `equip` / `unequip` on a party member.
/// The recompute system reads these and re-runs `derive_stats` for
/// the affected entity.
///
/// Derives `Message`, NOT `Event` — Bevy 0.18 family rename. Read with
/// `MessageReader<EquipmentChangedEvent>`. Register with
/// `app.add_message::<EquipmentChangedEvent>()` in `PartyPlugin::build`.
///
/// **Pattern source:** `MovedEvent` at src/plugins/dungeon/mod.rs:192-197
/// is the project precedent; same shape, same registration, same reader API.
///
/// Carries the `Entity` of the affected character so the recompute system
/// can target a single entity. (Slot is included for diagnostics / UI
/// post-fade animation in #25 but not used by v1's recompute system.)
#[derive(Message, Clone, Copy, Debug)]
pub struct EquipmentChangedEvent {
    pub character: Entity,
    pub slot: EquipSlot,
}
```

### Pattern 5: EquipError, EquipResult — the slot-validation contract

```rust
/// Reasons an `equip` call may fail. Returned by the helper functions
/// behind the `equip_action_handler` system; the system itself is
/// `fn(...)` (Bevy systems can't return `Result`). The system logs the
/// error at `warn!(...)` level so the UI in #25 can surface it later.
///
/// **Why a Result-returning helper:**
///
/// 1. Composes with future Inventory UI (#25): the UI wants to know
///    *why* an equip failed (kind mismatch vs. item missing) to render
///    a tooltip. A panic or silent no-op loses that information.
///
/// 2. Trivially unit-testable as a pure function: the helper can be
///    called from a Layer 1 test without setting up a full App.
///
/// 3. Project precedent: `DungeonFloor::can_move` at src/data/dungeon.rs
///    returns `bool`; this is the one-step-richer "tell me why it
///    failed" version, appropriate when the failure has multiple causes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EquipError {
    /// `ItemKind::Consumable` or `ItemKind::KeyItem` cannot be equipped.
    ItemHasNoSlot,
    /// The item's `EquipSlot` does not match the requested slot
    /// (e.g., trying to put a `Shield` in the `Weapon` slot).
    SlotMismatch,
    /// The character entity does not have an `Equipment` or
    /// `Inventory` component — caller passed a non-PartyMember entity.
    CharacterMissingComponents,
    /// The item entity does not have an `ItemInstance` component, OR the
    /// referenced `ItemAsset` is not loaded yet.
    ItemMissingComponents,
}

pub type EquipResult = Result<(), EquipError>;
```

### Pattern 6: equip / unequip / give_item — three small systems

The roadmap names three operations; in Bevy each is a tiny system. Show shape, not full body.

```rust
// SHAPE only — full implementation in the plan stage.

/// Equip the item entity in the character's `Equipment` slot.
///
/// 1. Look up `ItemInstance(handle)` on `item_entity`.
/// 2. Resolve the handle to `&ItemAsset` via `Res<Assets<ItemAsset>>`.
/// 3. Validate `asset.kind` allows equipping in `slot`.
/// 4. Validate `asset.slot == slot` (defensive — caller might mis-route).
/// 5. If a previous item is already in that slot, push it back to inventory
///    (spawn a fresh `ItemInstance` entity with the unequipped handle).
/// 6. Write the new handle into the appropriate `Equipment` field.
/// 7. Despawn the inventory entity AND remove from `Inventory(Vec<Entity>)`.
/// 8. Emit `EquipmentChangedEvent { character, slot }`.
pub fn equip_item(
    commands: &mut Commands,
    character: Entity,
    item_entity: Entity,
    slot: EquipSlot,
    items: &Assets<ItemAsset>,
    instances: &Query<&ItemInstance>,
    char_query: &mut Query<(&mut Equipment, &mut Inventory)>,
    writer: &mut MessageWriter<EquipmentChangedEvent>,
) -> EquipResult {
    // ... validation + slot-write + event-emit
}

/// Unequip from `Equipment::slot` and push to `Inventory`.
///
/// 1. Read the handle from the `Equipment::slot` field.
/// 2. Spawn a new `ItemInstance(handle.clone())` entity.
/// 3. Push the entity onto `Inventory(Vec<Entity>)`.
/// 4. Write `None` into the `Equipment::slot` field.
/// 5. Emit `EquipmentChangedEvent { character, slot }`.
pub fn unequip_item(
    commands: &mut Commands,
    character: Entity,
    slot: EquipSlot,
    char_query: &mut Query<(&mut Equipment, &mut Inventory)>,
    writer: &mut MessageWriter<EquipmentChangedEvent>,
) -> EquipResult {
    // ...
}

/// Add an item entity to the character's inventory (e.g., from loot,
/// shop purchase, or a `give_item` cheat hotkey).
///
/// 1. Spawn a new `ItemInstance(handle)` entity.
/// 2. Push onto `character`'s `Inventory(Vec<Entity>)`.
///
/// Does NOT emit `EquipmentChangedEvent` — this only changes inventory,
/// not equipment.
pub fn give_item(
    commands: &mut Commands,
    character: Entity,
    handle: Handle<ItemAsset>,
    char_query: &mut Query<&mut Inventory>,
) -> EquipResult {
    // ...
}
```

**Why these are helper functions, not direct systems:** Bevy systems can't take arbitrary callers; they only consume what's in their parameter list. The three operations above are *helpers* called from a system that maps user input or scripted events into equip operations. The system itself looks like:

```rust
fn handle_equip_actions(
    mut commands: Commands,
    mut requests: MessageReader<EquipRequest>,  // (NOT defined in v1; future hook)
    items: Res<Assets<ItemAsset>>,
    instances: Query<&ItemInstance>,
    mut char_query: Query<(&mut Equipment, &mut Inventory)>,
    mut writer: MessageWriter<EquipmentChangedEvent>,
) {
    for req in requests.read() {
        match equip_item(&mut commands, req.character, req.item, req.slot,
                         &items, &instances, &mut char_query, &mut writer) {
            Ok(()) => {}
            Err(e) => warn!("equip failed: {:?}", e),
        }
    }
}
```

For v1, **no `EquipRequest` message is needed** — the recompute system is the only Bevy-scheduled consumer. The three helper functions are public so future features (#25 inventory UI, #21 loot, #18 shops) can call them directly from their own systems. Plan should confirm this scoping — see Decision D5.

### Pattern 7: recompute_derived_stats_on_equipment_change — the canonical recompute

```rust
/// Subscribe to `EquipmentChangedEvent`, re-run `derive_stats` for
/// each affected character, write the result back, applying the
/// `current_*` clamp pattern documented at character.rs:294-298.
///
/// **Caller-clamp contract:** `derive_stats` returns `current_hp = max_hp`
/// as a sane default. Equipment-change recomputes must clamp:
/// `current = current.min(new_max)` (keep current_hp, don't reset on equip).
/// This is the canonical Wizardry/Etrian behavior — equipping a +20HP
/// amulet does NOT heal you for 20 (only `max_hp` increases).
///
/// **Pattern source:** `update_explored_on_move` at
/// src/plugins/ui/minimap.rs (subscribes to `MovedEvent` via
/// `MessageReader`, mutates a resource per event).
fn recompute_derived_stats_on_equipment_change(
    mut events: MessageReader<EquipmentChangedEvent>,
    items: Res<Assets<ItemAsset>>,
    mut characters: Query<(
        &BaseStats,
        &Equipment,
        &StatusEffects,
        &Experience,
        &mut DerivedStats,
    )>,
) {
    for ev in events.read() {
        let Ok((base, equip, status, xp, mut derived)) =
            characters.get_mut(ev.character)
        else {
            continue;
        };

        // Flatten Equipment slots into a Vec<ItemStatBlock>.
        let mut equip_stats: Vec<ItemStatBlock> = Vec::with_capacity(8);
        for slot in [&equip.weapon, &equip.armor, &equip.shield, &equip.helm,
                     &equip.gloves, &equip.boots, &equip.accessory_1, &equip.accessory_2] {
            if let Some(handle) = slot
                && let Some(asset) = items.get(handle)
            {
                equip_stats.push(asset.stats);
            }
        }

        let new = derive_stats(base, &equip_stats, status, xp.level);

        // Caller-clamp pattern (character.rs:128-131).
        let old_current_hp = derived.current_hp;
        let old_current_mp = derived.current_mp;
        *derived = new;
        derived.current_hp = old_current_hp.min(derived.max_hp);
        derived.current_mp = old_current_mp.min(derived.max_mp);
    }
}
```

This system does NOT touch `derive_stats`'s body. It owns the flatten step. It runs in `Update` gated by `state_changed` is unnecessary — runs every frame, drains messages.

### Anti-Patterns to Avoid (Druum-specific to #12)

- **DO NOT modify `derive_stats` to take `&Equipment`.** It's pure (`character.rs:323`) and #11 deliberately put the flatten step in the caller. A reverted signature breaks all #11 unit tests and the doc-comment contract at `character.rs:322-326`.

- **DO NOT add `Serialize`/`Deserialize` to `Equipment`, `Inventory`, or `ItemInstance`.** `Handle<T>` doesn't implement serde (`character.rs:196-202` documents this for `Equipment`); `Vec<Entity>` requires `MapEntities`. #23 owns those bridges. Adding stub serde now would either fail to compile or produce broken serialization — both worse than missing.

- **DO NOT add a `bevy::utils::HashMap`** for item lookup. `bevy::utils::HashMap` is removed in 0.18 (`#11 Pitfall 4`, `reference_bevy_ecosystem.md`). `ItemDb::get` uses linear scan over `Vec` (consistent with `ClassTable::get` at `classes.rs:41`).

- **DO NOT use `EventReader<T>`** anywhere. Bevy 0.18 family rename: it's `MessageReader<T>` paired with `#[derive(Message)]`. See `feedback_bevy_0_18_event_message_split.md`.

- **DO NOT add a `cfg(debug_assertions)` gate** for any debug-give-item system. Project precedent is `#[cfg(feature = "dev")]` (`#11 Pitfall 1`, `state/mod.rs:62`, `party/mod.rs:44`). If a debug "give all starter items to party member 0" hotkey lands in #12, gate it `#[cfg(feature = "dev")]`.

- **DO NOT add an `EquipRequest` `Message` for v1** unless the inventory UI in #25 demands it. The three `equip_item` / `unequip_item` / `give_item` helper functions are sufficient as a public API — UI in #25 can call them directly from its own systems. (Surface as Decision D5.)

- **DO NOT pre-create `progression.rs`** under `party/`. #14 owns that file. Same #11 single-file precedent.

- **DO NOT spawn `ItemInstance` entities outside the helper functions.** All entity creation goes through `give_item` / `unequip_item` so the `Inventory(Vec<Entity>)` push is centralized — easier to audit, easier to extend.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| RON loading for `ItemDb` | Custom serde RON loader | `bevy_common_assets::RonAssetPlugin::<ItemDb>` (already wired at `loading/mod.rs:98`) | Already done in #3; just flesh out `ItemDb::items` |
| Asset-loading state integration | Custom `LoadingState` machinery | `bevy_asset_loader` (already wired) | Already done in #3; `item_db: Handle<ItemDb>` field already in `DungeonAssets` |
| Item entity ↔ Inventory list lifecycle | Manual ECS bookkeeping | Helper functions (`equip_item`, `unequip_item`, `give_item`) that bundle the Vec push + Commands spawn + EquipmentChangedEvent emit | Centralize the three-action shape; one place to audit |
| Stat re-derivation on equip | Recomputing inline in `equip_item` | Emit `EquipmentChangedEvent`, let `recompute_derived_stats_on_equipment_change` consume | Decouples sender from receiver; UI in #25 can also subscribe to flash a portrait, etc. Same pattern as `MovedEvent` ↔ `update_explored_on_move`. |
| Slot-kind validation | Match-arm-spaghetti inside each system | `EquipError` enum + Result-returning helper | Project precedent for fallible APIs. Composes with #25 UI tooltips. |
| Stackable potion model | `StackableItem { handle, count }` component | **Do nothing** — roadmap line 681 punts. Each potion is a unique entity in v1. | Suboptimal but explicitly punted. Document the punt; do not design. |
| Save/load of `Inventory(Vec<Entity>)` | `MapEntities` impl now | **Do nothing** — #23 owns the save/load bridges for both `Equipment` (Handle ↔ AssetPath) and `Inventory` (Entity remap). | Out of scope. Document the requirement so #23 knows to budget for it. |

---

## Common Pitfalls

### Pitfall 1: `Commands::spawn` is deferred — fresh inventory entity isn't queryable in the same frame

**What goes wrong:** `unequip_item` spawns a new `ItemInstance` entity via `commands.spawn(...)` and pushes the resulting `Entity` onto `Inventory`. **The entity does not exist until `apply_deferred` runs** (between systems in the schedule). If a downstream system in the same `Update` tick queries `&ItemInstance` for the just-spawned entity, the query returns nothing.

**Why it happens:** `Commands` is a deferred mutation queue; its mutations apply at sync points, not immediately. Verified at `bevy_ecs-0.18.1/src/system/commands/mod.rs` (the trait docs are explicit about deferral).

**How to avoid:**
- `equip_item` and `unequip_item` emit `EquipmentChangedEvent` AFTER the `commands.spawn` / push. The recompute system reads the event NEXT frame OR same frame depending on system order. Project precedent at `dungeon/mod.rs:213-222` orders `handle_dungeon_input` before `animate_movement` and the `MovedEvent` is consumed by `update_explored_on_move` (in `Update` schedule, after `handle_dungeon_input`). The event lives in `Messages<T>` — readable across the same `Update` tick.
- DO NOT query `&ItemInstance` for the freshly-spawned entity in the same system; only query in `recompute_derived_stats_on_equipment_change` or later, where `apply_deferred` has run.
- If a synchronous spawn is required (rare), use `world.spawn(...)` exclusive-system access instead of `Commands`. Don't reach for this in v1.

### Pitfall 2: `Equipment` slot field name doesn't match `EquipSlot` variant directly

**What goes wrong:** `Equipment` has fields `weapon`, `armor`, `shield`, `helm`, `gloves`, `boots`, `accessory_1`, `accessory_2`. `EquipSlot` is recommended as `Weapon`, `Armor`, `Shield`, `Helm`, `Gloves`, `Boots`, `Accessory1`, `Accessory2`. Mapping between them requires a match arm (or a helper function).

**Why it happens:** Field names use snake_case (Rust convention); enum variants use PascalCase. The 1:1 mapping is mechanical but easy to mis-type.

**How to avoid:** Implement a single helper:

```rust
impl EquipSlot {
    /// Read the current handle in this slot. Returns `None` for `EquipSlot::None`.
    pub fn read(self, eq: &Equipment) -> Option<&Handle<ItemAsset>> {
        match self {
            EquipSlot::None => None,
            EquipSlot::Weapon => eq.weapon.as_ref(),
            EquipSlot::Shield => eq.shield.as_ref(),
            EquipSlot::Armor => eq.armor.as_ref(),
            EquipSlot::Helm => eq.helm.as_ref(),
            EquipSlot::Gloves => eq.gloves.as_ref(),
            EquipSlot::Boots => eq.boots.as_ref(),
            EquipSlot::Accessory1 => eq.accessory_1.as_ref(),
            EquipSlot::Accessory2 => eq.accessory_2.as_ref(),
        }
    }

    /// Write a handle (or `None`) to this slot.
    pub fn write(self, eq: &mut Equipment, handle: Option<Handle<ItemAsset>>) {
        match self {
            EquipSlot::None => {}
            EquipSlot::Weapon => eq.weapon = handle,
            EquipSlot::Shield => eq.shield = handle,
            // ...
        }
    }
}
```

Centralizes the mapping. Two unit tests (round-trip read+write) verify correctness.

### Pitfall 3: `Handle<ItemAsset>` clone is cheap, but `Asset::get` returns `Option<&T>`

**What goes wrong:** The flatten step `items.get(handle)` returns `Option<&ItemAsset>` because the asset might not be loaded yet. If the recompute runs on a frame where the asset isn't ready, `equip_stats` will be empty — derived stats will look wrong (just base stats, no equipment bonus).

**Why it happens:** Asset loading is asynchronous. `bevy_asset_loader` blocks the `Loading -> TitleScreen` transition until handles report `LoadedWithDependencies`, but a hot-reload or a freshly-spawned handle may temporarily be unloaded.

**How to avoid:**
- v1: ignore the case. The `Loading -> Dungeon` path guarantees `core.items.ron` is loaded by the time the player can equip anything.
- Add a defensive log: `if items.get(handle).is_none() { warn!("ItemAsset not loaded; equipment recompute will produce wrong stats. Item: {:?}", handle); }` so #25 / #23 can surface this if it ever happens.
- Hot-reload concern (re-derive on `AssetEvent<ItemAsset>`): defer to post-v1. Mark this as a `#[cfg(feature = "dev")]` enhancement if balance-tweaking via `--features dev` ever needs it.

### Pitfall 4: `assets/items/items.ron` (roadmap path) is wrong

**What goes wrong:** Following the roadmap line 659 path (`assets/items/items.ron`) creates a file that the existing loader at `loading/mod.rs:33` does not pick up — the loader's `#[asset(path = "items/core.items.ron")]` is the locked path.

**Why it happens:** Roadmap was authored before #3 locked the path; same trap as #11 Pitfall 2.

**How to avoid:** Use `assets/items/core.items.ron`. Edit the body of the existing 3-line stub. **Do NOT create a second file.** Do NOT touch `loading/mod.rs`. (The implementer must read this exact paragraph; this trap has caught two prior features.)

### Pitfall 5: `Reflect`-derived components need `#[derive(Reflect)]` AND `register_type::<T>` AND `app.register_type` call

**What goes wrong:** New types `Inventory`, `ItemInstance`, `EquipSlot`, `ItemKind` need three things to appear in editor / debug tooling:
1. `#[derive(Reflect)]` on the type.
2. The type must implement `Reflect` (for `Component`s, this means `#[derive(Reflect)] + impl Component`).
3. `app.register_type::<T>()` in the plugin.

Forgetting any one produces silent "type not visible in inspector" without a compile error.

**Why it happens:** Reflection is opt-in per type; the plugin registration is decoupled from the derive.

**How to avoid:** Mirror `mod.rs:25-38` from #11 — register every Reflect type. Add to `PartyPlugin::build`:

```rust
.register_type::<Inventory>()
.register_type::<ItemInstance>()
.register_type::<EquipSlot>()
.register_type::<ItemKind>()
.register_type::<EquipmentChangedEvent>()  // optional but helpful for inspector
```

Cross-reference: `reference_bevy_reflect_018_derive.md` confirms `Reflect` derives auto-handle the shapes #12 uses (enums, Vec, Option, Handle).

### Pitfall 6: Stackable items punt creates a UX wart that #21 (loot) will hit first

**What goes wrong:** v1 says "5 healing potions = 5 entities". When loot tables in #21 drop "5x healing potion" from a chest, the implementation must spawn 5 entities and push 5 onto `Inventory`. The inventory list grows fast.

**Why it happens:** Roadmap line 681 explicitly punts.

**How to avoid:**
- Document the punt inline in `inventory.rs`:
  ```rust
  // ROADMAP PUNT (line 681): every potion is a unique entity in v1.
  // A "give 5 healing potions" call spawns 5 entities and pushes 5
  // onto Inventory(Vec<Entity>). When stackability lands (post-v1),
  // either:
  //   (a) add a StackableItem(u32) component on the entity (one entity
  //       represents N items), OR
  //   (b) deduplicate at UI time only (one icon, count badge).
  // Both options preserve the entity-per-item model for non-stackable
  // items. NO design decisions here for v1.
  ```
- Surface as Open Question OQ4 — the planner may want to discuss with the user whether 5-stack potions are a v1 nice-to-have. Default: no, follow the roadmap.

### Pitfall 7: Re-entering `OnEnter(Dungeon)` after a Town visit might re-trigger `spawn_default_debug_party`

**What goes wrong:** `spawn_default_debug_party` at `party/mod.rs:62` has an idempotence guard (lines 67-72). If #12 adds a `give_starter_items_to_debug_party` system gated on `OnEnter(Dungeon)` under `#[cfg(feature = "dev")]`, that system needs the SAME idempotence guard, OR it must subscribe to a "first-spawn-only" message from `spawn_default_debug_party`.

**Why it happens:** F9 cycler can re-enter Dungeon multiple times in a single session (`state/mod.rs:62-92`).

**How to avoid:**
- If a starter-items system is added, gate with `if existing_inventory_is_empty(&inventory_query) { ... }` or piggyback on the same Query check `spawn_default_debug_party` uses.
- Or: don't ship a starter-items system at all in #12. Use the items only via Layer-2 unit tests; in-game testing happens in #21 (loot) when it lands. **Recommended: no starter-items system in #12.** Surface as Decision D4.

### Pitfall 8: `Reflect` on `Handle<T>` needs `#[reflect(Component)]` adjustment

**What goes wrong:** Some Reflect derives on components containing `Handle<T>` need `#[reflect(Component)]` plus the `Reflect` derive on the Handle's type.

**Why it happens:** `Handle<T>` is `Reflect`-able only when `T: TypePath + Asset`. Both conditions are met for `ItemAsset` (via `#[derive(Asset, Reflect)]` at items.rs:55), so this is not actually a problem for #12 — but it's a known footgun for assets that forget `Reflect`.

**How to avoid:** When fleshing out `ItemAsset`, keep `Reflect` in the derive list (already there at items.rs:55). Test: ensure `app.register_type::<ItemAsset>()` doesn't panic. **Already implicitly verified by #11 — the existing stub already registers via `RonAssetPlugin`.** No new action needed.

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

No known CVEs as of 2026-05-05 for any library used in #12. Same status as #11.

### Architectural Security Risks

| Risk | Affected Architecture | How It Manifests | Secure Pattern | Anti-Pattern |
|------|----------------------|------------------|----------------|--------------|
| Malicious `core.items.ron` | `ItemDb`, `ItemAsset` | Crafted item with `attack: u32::MAX` overflows when summed in flatten loop | `derive_stats` already uses `saturating_add` (verified at character.rs:374-381). The flatten loop's `Vec::push` doesn't sum — the saturating arithmetic is in `derive_stats` itself. SAFE. | Direct addition with no overflow guard |
| Malicious save file (#23) injecting massive `Inventory(Vec<Entity>)` | `Inventory` capacity | `Vec::with_capacity(1B)` from a crafted save = OOM | #23 must bound `Inventory` length at deserialize. Out of scope for #12; flag for #23 | Trust save file vec lengths |
| `EquipSlot::None` with a non-`None`-kind item | `equip_item` | Item asset declares `slot: None` but `kind: Weapon`; mismatch | `EquipError::ItemHasNoSlot` rejects all `EquipSlot::None`. **Add a Layer-1 test that exercises this rejection path.** | Silent no-op on bad data |
| `equip_item` with mismatched character entity (e.g., enemy entity) | `equip_item` | Character entity has no `Equipment` component | `Query::get(entity)` returns Err → `EquipError::CharacterMissingComponents` | Panic with `unwrap()` |
| Negative item stats via `serde` | `ItemStatBlock` | All fields are `u32` — no negatives possible | Already enforced by type. SAFE. | Using `i32` and forgetting the clamp |

### Trust Boundaries

- **`core.items.ron` from disk:** assumed-developer-authored. If modding is supported in the future, schema-validate (clamp `value`, `weight`, all stat fields) before loading. **Out of scope for v1.**
- **Save files (#23 territory):** out of scope for #12. The `Reflect` derives shipped here are the trust-boundary surface for #23.
- **No network input.** Single-player game.

---

## Performance

| Metric | Value / Range | Source | Notes |
|--------|--------------|-------|-------|
| `recompute_derived_stats_on_equipment_change` per event | <50µs (4 character flattens × 8 slots × O(1) Asset lookup + 1 derive_stats call) | Estimated from #11 derive_stats <1µs measurement | Equipment changes happen at human-input rates (sub-Hz); cost is negligible |
| `Inventory(Vec<Entity>)` push / swap-remove | O(1) amortized push, O(1) swap_remove, O(n) remove | std::vec docs | <100 items per character expected; not a hot path |
| `ItemDb::get` linear scan | O(n) over <12 items | std::iter::find | Same shape as `ClassTable::get`; not a hot path |
| `Assets<ItemAsset>::get` lookup | O(1) via `AssetId` slotmap | Bevy asset internals | Used per slot per recompute; trivial |
| `EquipmentChangedEvent` `Messages<T>` overhead | ~one cache line per message | Bevy ECS internals | Drained each Update; <10 events per frame typical |
| `core.items.ron` deserialize | <10ms for 12-item table | RON parser | Loaded once at startup |

**Performance is NOT a concern for Feature #12.** The data layer is small, the operations are sub-Hz, and the recompute is event-driven (not polling). `derive_stats` profiling from #11 confirms baseline.

---

## Code Examples

### Example 1: PartyPlugin::build with #12 additions

```rust
// src/plugins/party/mod.rs — modified from #11

use bevy::prelude::*;

pub mod character;
pub mod inventory;  // NEW

pub use character::{ /* #11 re-exports unchanged */ };
pub use inventory::{
    EquipError, EquipResult, EquipSlot, EquipmentChangedEvent,
    Inventory, ItemInstance, ItemKind,
    equip_item, give_item, unequip_item,
    recompute_derived_stats_on_equipment_change,
};

pub struct PartyPlugin;

impl Plugin for PartyPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PartySize>();

        // #11 type registrations (unchanged)
        app.register_type::<CharacterName>()
            .register_type::<Race>()
            .register_type::<Class>()
            .register_type::<BaseStats>()
            .register_type::<DerivedStats>()
            .register_type::<Experience>()
            .register_type::<PartyRow>()
            .register_type::<PartySlot>()
            .register_type::<Equipment>()
            .register_type::<StatusEffects>()
            .register_type::<PartyMember>()
            .register_type::<ActiveEffect>()
            .register_type::<StatusEffectType>()
            .register_type::<PartySize>();

        // #12 NEW
        app.add_message::<EquipmentChangedEvent>()
            .register_type::<Inventory>()
            .register_type::<ItemInstance>()
            .register_type::<EquipSlot>()
            .register_type::<ItemKind>()
            .add_systems(Update, recompute_derived_stats_on_equipment_change);

        // Debug party spawn (gated on dev feature, unchanged from #11)
        #[cfg(feature = "dev")]
        {
            use crate::plugins::state::GameState;
            app.add_systems(OnEnter(GameState::Dungeon), spawn_default_debug_party);
        }
    }
}
```

### Example 2: equip_item helper (full body shape)

```rust
pub fn equip_item(
    commands: &mut Commands,
    character: Entity,
    item_entity: Entity,
    slot: EquipSlot,
    items: &Assets<ItemAsset>,
    instances: &Query<&ItemInstance>,
    char_query: &mut Query<(&mut Equipment, &mut Inventory), With<PartyMember>>,
    writer: &mut MessageWriter<EquipmentChangedEvent>,
) -> EquipResult {
    if slot == EquipSlot::None {
        return Err(EquipError::ItemHasNoSlot);
    }

    let instance = instances.get(item_entity)
        .map_err(|_| EquipError::ItemMissingComponents)?;

    let asset = items.get(&instance.0)
        .ok_or(EquipError::ItemMissingComponents)?;

    if asset.slot == EquipSlot::None || matches!(asset.kind, ItemKind::Consumable | ItemKind::KeyItem) {
        return Err(EquipError::ItemHasNoSlot);
    }

    if asset.slot != slot {
        return Err(EquipError::SlotMismatch);
    }

    let (mut equipment, mut inventory) = char_query.get_mut(character)
        .map_err(|_| EquipError::CharacterMissingComponents)?;

    // If something already equipped here, push it back to inventory.
    if let Some(prev_handle) = slot.read(&equipment).cloned() {
        let prev_entity = commands.spawn(ItemInstance(prev_handle)).id();
        inventory.0.push(prev_entity);
    }

    // Write the new handle into Equipment.
    slot.write(&mut equipment, Some(instance.0.clone()));

    // Remove from inventory list and despawn the inventory entity.
    inventory.0.retain(|&e| e != item_entity);
    commands.entity(item_entity).despawn();

    writer.write(EquipmentChangedEvent { character, slot });

    Ok(())
}
```

### Example 3: items.ron starter content (8 items — illustrative; designer-tunable)

```ron
// assets/items/core.items.ron — replaces `()` stub
// Feature #12 starter items. Stat values are SEEDS for #14/#21 balancing.

(
    items: [
        // Weapons (3)
        (
            id: "rusty_sword",
            display_name: "Rusty Sword",
            stats: ( attack: 5 ),
            kind: Weapon,
            slot: Weapon,
            weight: 3,
            value: 5,
            icon_path: "ui/icons/items/rusty_sword.png",
        ),
        (
            id: "oak_staff",
            display_name: "Oak Staff",
            stats: ( attack: 2, magic_attack: 3 ),
            kind: Weapon,
            slot: Weapon,
            weight: 2,
            value: 8,
            icon_path: "ui/icons/items/oak_staff.png",
        ),
        (
            id: "wooden_mace",
            display_name: "Wooden Mace",
            stats: ( attack: 4, magic_attack: 1 ),
            kind: Weapon,
            slot: Weapon,
            weight: 4,
            value: 7,
            icon_path: "ui/icons/items/wooden_mace.png",
        ),

        // Armor (2)
        (
            id: "leather_armor",
            display_name: "Leather Armor",
            stats: ( defense: 3 ),
            kind: Armor,
            slot: Armor,
            weight: 5,
            value: 10,
            icon_path: "ui/icons/items/leather_armor.png",
        ),
        (
            id: "robe",
            display_name: "Apprentice Robe",
            stats: ( defense: 1, magic_defense: 2, mp_bonus: 5 ),
            kind: Armor,
            slot: Armor,
            weight: 1,
            value: 12,
            icon_path: "ui/icons/items/robe.png",
        ),

        // Shield (1)
        (
            id: "wooden_shield",
            display_name: "Wooden Shield",
            stats: ( defense: 2 ),
            kind: Shield,
            slot: Shield,
            weight: 3,
            value: 6,
            icon_path: "ui/icons/items/wooden_shield.png",
        ),

        // Consumable (1) — declared, NOT equippable
        (
            id: "healing_potion",
            display_name: "Healing Potion",
            stats: (),  // No stat modifiers; the heal effect is #14/#21 territory
            kind: Consumable,
            slot: None,
            weight: 1,
            value: 25,
            icon_path: "ui/icons/items/healing_potion.png",
        ),

        // Key item (1) — declared, NOT equippable
        (
            id: "rusty_key",
            display_name: "Rusty Key",
            stats: (),
            kind: KeyItem,
            slot: None,
            weight: 0,
            value: 0,
            icon_path: "ui/icons/items/rusty_key.png",
        ),
    ],
)
```

This is **8 items**, satisfying roadmap "8-12" lower bound. Planner may extend to 12 (add a helm, gloves, boots, accessory) for slot coverage in the integration test — surface as Decision D7.

### Example 4: Layer-1 unit test for equip_item slot validation (pure stdlib)

```rust
// src/plugins/party/inventory.rs — tests module

#[cfg(test)]
mod tests {
    // Layer 1 tests verify pure functions / Result returns without an App.
    // Tests for the *systems* (which need Bevy App) go in Layer 2 below.

    /// Equipping a Consumable returns ItemHasNoSlot.
    #[test]
    fn equip_consumable_returns_item_has_no_slot() {
        // Setup a tiny Asset<ItemAsset> in-memory (no full app)
        // ...build minimal asset, instance, character, query stubs...
        let result = equip_item_test_harness(
            &healing_potion_asset(),
            EquipSlot::Weapon,
        );
        assert_eq!(result, Err(EquipError::ItemHasNoSlot));
    }

    /// Equipping a Sword in the Armor slot returns SlotMismatch.
    #[test]
    fn equip_sword_in_armor_slot_returns_slot_mismatch() {
        let result = equip_item_test_harness(
            &rusty_sword_asset(),
            EquipSlot::Armor,
        );
        assert_eq!(result, Err(EquipError::SlotMismatch));
    }

    /// Equipping a Sword in the Weapon slot succeeds.
    #[test]
    fn equip_sword_in_weapon_slot_succeeds() {
        let result = equip_item_test_harness(
            &rusty_sword_asset(),
            EquipSlot::Weapon,
        );
        assert_eq!(result, Ok(()));
    }
}
```

### Example 5: Layer-2 integration test — equipping updates DerivedStats

```rust
// src/plugins/party/inventory.rs — tests module (Layer 2 — needs MinimalPlugins + StatesPlugin + AssetPlugin)

#[cfg(test)]
mod app_tests {
    use bevy::asset::AssetPlugin;
    use bevy::state::app::StatesPlugin;
    use super::*;
    use crate::plugins::party::PartyPlugin;
    use crate::plugins::state::StatePlugin;

    fn make_test_app() -> App {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            StatesPlugin,
            StatePlugin,
            PartyPlugin,
        ));
        // (no LoadingPlugin — we manually stub Assets<ItemAsset>)
        app.update();  // Realize initial state
        app
    }

    /// Equipping a +10 attack sword raises DerivedStats::attack by 10.
    /// Unequipping reverses it.
    #[test]
    fn equip_sword_raises_attack_unequip_lowers() {
        let mut app = make_test_app();

        // 1. Author the sword asset directly.
        let mut items = app.world_mut().resource_mut::<Assets<ItemAsset>>();
        let sword_handle = items.add(ItemAsset {
            id: "test_sword".into(),
            display_name: "Test Sword".into(),
            stats: ItemStatBlock { attack: 10, ..Default::default() },
            kind: ItemKind::Weapon,
            slot: EquipSlot::Weapon,
            ..Default::default()
        });

        // 2. Spawn a character with empty Equipment + Inventory + zero base stats.
        let character = app.world_mut().spawn(PartyMemberBundle {
            base_stats: BaseStats::ZERO,
            ..Default::default()
        }).id();
        // Add an Inventory component (not in PartyMemberBundle)
        app.world_mut().entity_mut(character).insert(Inventory::default());

        // 3. Spawn the inventory item entity.
        let item_entity = app.world_mut().spawn(ItemInstance(sword_handle.clone())).id();
        app.world_mut().entity_mut(character)
            .get_mut::<Inventory>().unwrap().0.push(item_entity);

        // 4. Manually emit EquipmentChangedEvent after writing Equipment::weapon
        //    (the helper functions take complex args; this test exercises the
        //    recompute system directly).
        app.world_mut().entity_mut(character)
            .get_mut::<Equipment>().unwrap().weapon = Some(sword_handle.clone());

        let mut writer = app.world_mut()
            .resource_mut::<bevy::ecs::message::Messages<EquipmentChangedEvent>>();
        writer.write(EquipmentChangedEvent {
            character,
            slot: EquipSlot::Weapon,
        });

        // 5. Step the app — recompute system runs.
        app.update();

        // 6. Assert DerivedStats::attack == 10 (BaseStats::ZERO + sword 10).
        let derived = app.world().entity(character).get::<DerivedStats>().unwrap();
        assert_eq!(derived.attack, 10, "Equipping +10 sword should set attack to 10");

        // 7. Unequip — clear Equipment::weapon, emit event, re-step.
        app.world_mut().entity_mut(character)
            .get_mut::<Equipment>().unwrap().weapon = None;
        let mut writer = app.world_mut()
            .resource_mut::<bevy::ecs::message::Messages<EquipmentChangedEvent>>();
        writer.write(EquipmentChangedEvent {
            character,
            slot: EquipSlot::Weapon,
        });
        app.update();

        let derived = app.world().entity(character).get::<DerivedStats>().unwrap();
        assert_eq!(derived.attack, 0, "Unequipping should drop attack back to 0");
    }
}
```

### Example 6: Integration test — `core.items.ron` loads via RonAssetPlugin

```rust
// tests/item_db_loads.rs — NEW file, mirrors tests/class_table_loads.rs

use bevy::app::AppExit;
use bevy::asset::AssetPlugin;
use bevy::prelude::*;
use bevy::state::app::StatesPlugin;
use bevy_asset_loader::prelude::*;
use bevy_common_assets::ron::RonAssetPlugin;
use druum::data::{ItemAsset, ItemDb};
use druum::plugins::party::{EquipSlot, ItemKind};

#[derive(AssetCollection, Resource)]
struct TestAssets {
    #[asset(path = "items/core.items.ron")]
    item_db: Handle<ItemDb>,
}

#[derive(Clone, Eq, PartialEq, Debug, Hash, Default, States)]
enum TestState { #[default] Loading, Loaded }

#[test]
fn item_db_loads_through_ron_asset_plugin() {
    App::new()
        .add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            StatesPlugin,
            RonAssetPlugin::<ItemDb>::new(&["items.ron"]),
        ))
        .init_state::<TestState>()
        .add_loading_state(
            LoadingState::new(TestState::Loading)
                .continue_to_state(TestState::Loaded)
                .load_collection::<TestAssets>(),
        )
        .add_systems(Update, timeout.run_if(in_state(TestState::Loading)))
        .add_systems(OnEnter(TestState::Loaded), assert_item_db_shape)
        .run();
}

fn timeout(time: Res<Time>) {
    if time.elapsed_secs_f64() > 30.0 {
        panic!("ItemDb did not load in 30 seconds");
    }
}

fn assert_item_db_shape(
    assets: Res<TestAssets>,
    item_dbs: Res<Assets<ItemDb>>,
    mut exit: MessageWriter<AppExit>,
) {
    let db = item_dbs.get(&assets.item_db).expect("ItemDb should be loaded");
    assert!(db.items.len() >= 8, "Expected at least 8 starter items");

    let sword = db.get("rusty_sword").expect("rusty_sword should exist");
    assert_eq!(sword.kind, ItemKind::Weapon);
    assert_eq!(sword.slot, EquipSlot::Weapon);
    assert_eq!(sword.stats.attack, 5);

    let potion = db.get("healing_potion").expect("healing_potion should exist");
    assert_eq!(potion.kind, ItemKind::Consumable);
    assert_eq!(potion.slot, EquipSlot::None);  // not equippable

    exit.write(AppExit::Success);
}
```

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|-------------|-----------------|--------------|--------|
| Equipment slots as `Option<Entity>` (master research §Pattern 3 verbatim) | `Option<Handle<Asset>>` for static stats; separate entity for per-instance state | Project-internal Decision (#11 Decision 3) | Save/load uses Handle ↔ AssetPath, no `MapEntities` for equipped items; Inventory still needs MapEntities |
| `Event` + `EventReader` for cross-system messaging | `Message` + `MessageReader` | Bevy 0.18 family rename | Same shape, different macro name |
| Pooled `PartyInventory: Resource` shared across party | Per-character `Inventory(Vec<Entity>)` component | Roadmap line 675 (resolved Wizardry-style) | Idiomatic ECS; matches genre canon |
| Stackable items first-class | Stackable items deferred (`stackable: bool` declared but unused) | Roadmap line 681 (explicit punt) | Each potion is unique entity in v1; #21 / #25 may add a stack abstraction post-v1 |

**Deprecated patterns to avoid:**
- `bevy::utils::HashMap` — gone in 0.18, use `std::collections::HashMap`.
- `EventReader<EquipmentChangedEvent>` — must be `MessageReader<EquipmentChangedEvent>`.
- Storing inventory in a single `GameState` resource — use components per master research §Anti-Patterns.

---

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | Rust built-in `#[cfg(test)] mod tests` + `cargo test` |
| Config file | None (Cargo.toml conventions) |
| Quick run command | `cargo test plugins::party::inventory` |
| Full suite (mirrors #11 7-command gate) | `cargo check && cargo check --features dev && cargo clippy --all-targets -- -D warnings && cargo clippy --all-targets --features dev -- -D warnings && cargo fmt --check && cargo test && cargo test --features dev` |

### Layer split (per `feedback_bevy_input_test_layers.md`)

- **Layer 1 — pure functions (no App):** `EquipError` variant return-path tests, `EquipSlot::read`/`write` round-trip, `ItemAsset` + `ItemDb` RON round-trip. Run with stdlib only. Sub-1ms each.
- **Layer 2 — App-driven (no `InputPlugin`):** `recompute_derived_stats_on_equipment_change` integration, `equip_item` end-to-end (spawn, equip, query DerivedStats, unequip, query again). Use `MinimalPlugins + AssetPlugin + StatesPlugin + StatePlugin + PartyPlugin`. Pattern from `audio/mod.rs:145-178`.
- **Layer 3 — `init_resource::<ButtonInput<KeyCode>>` bridge:** **NOT NEEDED for #12** (no input handling lands).

### Requirements → Test Map

| Requirement | Behavior | Test Type | Automated Command | File Exists? |
|-------------|----------|-----------|-------------------|--------------|
| `ItemAsset` serde RON round-trip | Deserialize == Serialize input | Layer 1 | `cargo test data::items::tests::item_asset_round_trips` | ❌ needs creating |
| `ItemDb` serde RON round-trip | Multi-item table preserves order | Layer 1 | `cargo test data::items::tests::item_db_round_trips` | ❌ needs creating |
| `ItemDb::get` returns Some for known id | Linear-scan lookup works | Layer 1 | `cargo test data::items::tests::item_db_get_returns_authored_item` | ❌ needs creating |
| `EquipSlot::read` / `write` round-trip | Read after write returns same handle | Layer 1 | `cargo test plugins::party::inventory::tests::equip_slot_read_write_round_trip` | ❌ needs creating |
| `equip_item` rejects Consumable with ItemHasNoSlot | Validation works | Layer 1 | `cargo test plugins::party::inventory::tests::equip_consumable_rejected` | ❌ needs creating |
| `equip_item` rejects slot mismatch | Validation works | Layer 1 | `cargo test plugins::party::inventory::tests::equip_slot_mismatch_rejected` | ❌ needs creating |
| `equip_item` succeeds + emits message | Happy path | Layer 2 | `cargo test plugins::party::inventory::app_tests::equip_emits_message` | ❌ needs creating |
| `recompute_derived_stats_on_equipment_change` updates DerivedStats | Stat re-derivation | Layer 2 | `cargo test plugins::party::inventory::app_tests::equip_sword_raises_attack_unequip_lowers` | ❌ needs creating |
| `give_item` pushes onto Inventory | Add path | Layer 2 | `cargo test plugins::party::inventory::app_tests::give_item_pushes_to_inventory` | ❌ needs creating |
| `core.items.ron` loads through RonAssetPlugin | Integration | `tests/item_db_loads.rs` | `cargo test --test item_db_loads` | ❌ needs creating (mirror `tests/class_table_loads.rs`) |

**6-10 tests per roadmap budget — this maps to 10 above (Layer 1: 6, Layer 2: 3, Integration: 1). Plan can drop 1-3 of the Layer 1 round-trips if budget tightens.**

### Gaps (files to create before implementation)

- [ ] `src/plugins/party/inventory.rs` — NEW, ~500-700 LOC (components + enum × 2 + message + error + 4 helper functions + recompute system + Layer 1+2 tests)
- [ ] `src/plugins/party/mod.rs` — MODIFY (add `pub mod inventory;`, re-exports, register message + types, register recompute system)
- [ ] `src/data/items.rs` — MODIFY (flesh out `ItemAsset` and `ItemDb`; add Layer 1 round-trip test for the full schema)
- [ ] `src/data/mod.rs` — MODIFY (re-export `ItemAsset`'s new fields if needed; the existing re-exports at line 22-24 already cover the types)
- [ ] `assets/items/core.items.ron` — MODIFY (replace stub `()` with 8-12 starter items)
- [ ] `assets/ui/icons/items/*.png` — NEW DIR + 5-10 placeholder 32×32 PNGs (see "Asset Icons" section below)
- [ ] `tests/item_db_loads.rs` — NEW (integration test, ~70 LOC)

**No edits to:** `src/main.rs`, `src/plugins/state/mod.rs`, `src/plugins/loading/mod.rs`, `src/plugins/dungeon/mod.rs`, `src/plugins/input/mod.rs`, `src/plugins/audio/mod.rs`, `src/plugins/combat/mod.rs`, `src/plugins/ui/mod.rs`, `src/plugins/save/mod.rs`, `src/plugins/town/mod.rs`, `src/data/dungeon.rs`, `src/data/spells.rs`, `src/data/enemies.rs`, `src/data/classes.rs`, `src/plugins/party/character.rs`. Same cleanest-ship signal as #11.

---

## Asset Icons — convention investigation

### Current state of `assets/`

Verified via direct read of asset files referenced by `loading/mod.rs`:
- `assets/dungeons/floor_01.dungeon.ron` — exists (#4 / #8)
- `assets/items/core.items.ron` — exists, `()` stub
- `assets/enemies/core.enemies.ron` — exists, populated for #15 stub
- `assets/classes/core.classes.ron` — exists, populated by #11
- `assets/spells/core.spells.ron` — exists, stub for #20
- `assets/audio/bgm/*.ogg` — 5 BGM files (#6)
- `assets/audio/sfx/*.ogg` — 5 SFX files (#6)

**No portrait or icon assets exist.** No `assets/ui/`, no `assets/portraits/`, no `assets/textures/`. Feature #12 will create the **first PNG/icon asset directory in the project**.

### Recommended convention for `assets/ui/icons/items/`

| Property | Recommendation | Rationale |
|---------|---------------|-----------|
| Directory | `assets/ui/icons/items/` | Genre-conventional path. `ui/icons/` separates UI assets from the (non-existent yet) `ui/portraits/`, `ui/sprites/` (#17 enemies). Plural "items" mirrors RON path style. |
| File extension | `.png` | Bevy 0.18 supports PNG out of the box (`Cargo.toml:13`: `"png"` feature). KTX2 is also enabled but overkill for 32×32 placeholders. |
| Resolution | **32×32 px** per roadmap line 679 | Genre standard for inventory grids. Wizardry/Etrian/Grimrock use 32 or 48; 32 is sufficient for v1. |
| Color | sRGB, transparent background | PNG with alpha channel. Solid-background icons read poorly on dark dungeon UIs. |
| Naming | `<id>.png` matching `ItemAsset::id` | Stable mapping: `id = "rusty_sword"` → `ui/icons/items/rusty_sword.png`. The `icon_path` field can remain a String for v1; #25 inventory UI resolves to `Handle<Image>` at render time. |
| Source | Placeholder shapes (colored squares with letter codes) for v1 | The roadmap line 679 says "produce 5-10 placeholder 32x32 item icons (PNG)". v1 can ship `RS` on red square for "Rusty Sword", `LA` on brown square for "Leather Armor", etc. **Do NOT block #12 on artwork.** Real icons land in #25 polish. |

### How to produce placeholder PNGs

The user's environment is Darwin/macOS. Three options:

| Method | Pros | Cons |
|--------|------|------|
| **A: Hand-author 32×32 PNGs in any image editor** (Pixelmator, Aseprite, GIMP) | Visible icon shapes; matches what the artist will hand-tune in #25 | Time cost, not in repo natively |
| **B: Generate via ImageMagick** `convert -size 32x32 xc:red -gravity center -pointsize 16 -annotate 0 RS rusty_sword.png` | Reproducible; can be a Makefile target | Requires `convert` installed; the placeholder visual is plain |
| **C: Skip PNGs entirely; have `icon_path` point to nonexistent files for v1** | Zero asset work | Layer-2 / integration tests can't verify icon_path resolution. Roadmap line 679 explicitly says "produce 5-10 placeholder PNGs" — skipping deviates. |

**Recommended: B (ImageMagick) committed via a `Makefile` target or a one-time `scripts/gen_placeholder_icons.sh`.** Each PNG is <1KB. Total asset Δ: 8 PNGs × <1KB = ~8KB. Roadmap budget +5-10 placeholders is satisfied.

Surface as Decision D8: who creates the placeholder icons, what tool, what visual style?

### What about portraits for the debug party?

Not asked; not in scope. Roadmap §11 doesn't mandate party portraits, and `Camera3d`-based dungeon rendering doesn't currently render party member faces. Portraits are #18/#25 territory. Skip for #12.

---

## The Five Roadmap Decisions + Three Newly-Discovered (D1–D8)

The orchestrator named D1–D5. After research, **D1–D3 are auto-resolvable from #11's live code** (no longer Category B); D4–D5 are genuine Category B; **D6–D8 are newly-surfaced Category B decisions** that the planner must forward.

### Decision D1 (orchestrator's): `Equipment` shape — `Entity` vs `Handle`

**STATUS: AUTO-RESOLVED by #11.** `Equipment` ships as `Option<Handle<ItemAsset>>` per slot at `character.rs:209-219`. **NOT a Category B decision** — reverting would mean reopening #11 Decision 3, which the user already approved. Surface ONLY if user explicitly asks to reverse.

**Recommended:** keep `Handle`-based equipment. Use Option A architecture (Hybrid).

### Decision D2 (orchestrator's): `Inventory` shape — `Vec<Entity>` vs `Vec<Handle<ItemAsset>>` vs Resource

**STATUS: GENUINE Category B.** Three live options:

| Option | Description | Pros | Cons |
|--------|-------------|------|------|
| **A: `Inventory(Vec<Entity>)` per character** [RECOMMENDED] | Each item is an entity carrying `ItemInstance(Handle)`. Pushes/swap_removes on the Vec. | Per-instance state (post-v1 enchantment) is native. Roadmap line 675 already resolved this Wizardry-style. Doc-comment promise at `character.rs:204-205` explicitly says "Per-instance state ... lands in #12 as a separate `ItemInstance` entity model." | Save/load (#23) requires `MapEntities` on Inventory. Pitfall 1 (Commands deferral) applies. |
| B: `Inventory(Vec<Handle<ItemAsset>>)` per character | Each inventory entry is a handle. No item entities. | Zero-friction save/load. No `MapEntities`. No Commands deferral. | No per-instance state ever. Reverses doc-comment promise. |
| C: `PartyInventory: Resource(Vec<Entity>)` shared | Single bag for the whole party. | One source of truth; trivial UI lookup. | Defeats per-character genre canon (Wizardry has per-member bags); roadmap line 675 explicitly closes this in favor of A. |

**Recommended:** A. Plan-of-record cost if user picks B: drop `ItemInstance`, drop `Inventory(Vec<Entity>)`, write `Inventory(Vec<Handle<ItemAsset>>)`. ~50 LOC less. But also drops the path to per-instance enchantment — flag that explicitly.

### Decision D3 (orchestrator's): Slot validation — `Result<(), EquipError>` vs panic vs silent

**STATUS: GENUINE Category B but lightly weighted.** Three options:

| Option | Description | Pros | Cons |
|--------|-------------|------|------|
| **A: `Result<(), EquipError>` from helper functions** [RECOMMENDED] | Helpers return Result; system logs `warn!` on Err. | Idiomatic Rust. UI in #25 can consume the Err type for tooltip text. Layer-1 testable. | Slightly more code (~40 LOC for the EquipError enum + match arms). |
| B: Panic on invalid slot | Hard fail. Easy to spot in dev. | Catches bugs during dev. | Crashes in release. UI in #25 must defensively check before calling — duplicates validation. |
| C: Silent no-op (return `()` on failure) | Simplest. | Masks bugs. UI in #25 can't surface "why didn't this equip" feedback. |

**Recommended:** A. Standard Rust pattern; aligns with `DungeonFloor::can_move`'s "tell-me-why" precedent (slightly richer because of the multi-cause failure).

### Decision D4 (orchestrator's): Items.ron exact starter content (8 vs 12, what categories)

**STATUS: GENUINE Category B (designer-balance).** Recommended starter set in "Code Examples §3" above ships **8 items**: 3 weapons, 2 armors, 1 shield, 1 consumable (potion), 1 key item. Roadmap budget is 8-12.

**Sub-options:**
- **8 items** [RECOMMENDED]: minimal viable test surface. All 4 ItemKind variants exercised (Weapon, Armor, Shield, Consumable, KeyItem; no Helm/Gloves/Boots/Accessory — those require more design).
- **12 items**: extend to cover Helm, Gloves, Boots, Accessory slots for full slot coverage in the integration test.
- **6 items**: bare minimum (1 of each equipable kind + 1 consumable). Below roadmap floor.

**Recommended:** 8. Plan can flex to 12 if user wants full slot coverage in the integration test (~30 minutes of designer time per item).

### Decision D5 (orchestrator's): debug `give_starter_items` system — ship in #12 or defer

**STATUS: GENUINE Category B.** Roadmap doesn't mandate this. Two options:

| Option | Description | Pros | Cons |
|--------|-------------|------|------|
| A: Ship `give_starter_items_to_debug_party` gated on `#[cfg(feature = "dev")]`, triggered after `spawn_default_debug_party` | Debug party has a sword and armor in-game for visual smoke testing | +50 LOC, +1 system, requires Pitfall 7 idempotence guard, but useful for #25 UI dev. |
| **B: Skip — items are exercised only via Layer-2 tests** [RECOMMENDED] | Cleaner ship; no dev-only system to maintain. #21 (loot tables) is when the in-game item flow first matters. | Debug party has empty inventory in-game. UI dev (#25) starts with empty bags. |

**Recommended:** B. Skip. #21 is where items first matter for in-game flow; #12 is the data layer.

### Decision D6 (NEW — discovered): Reverse #11 Decision 3 (Equipment = Entity)?

**STATUS: GENUINE Category B but only relevant if user asks.** Surface this only if the user pushes back on the recommendation in D1.

**Recommended:** Do NOT reverse. Keep Handle-based Equipment.

**Cost if reversed:** ~80 LOC of `MapEntities` plumbing + ~40 LOC of `Equipment` redesign + Edit `recompute_derived_stats_on_equipment_change` to query for `&ItemInstance` instead of looking up `&Assets<ItemAsset>` + a save/load round-trip integration test that wasn't required before. **Reverts #11's resolved Decision 3.**

### Decision D7 (NEW — discovered): RON-versus-code declaration of `EquipSlot` ↔ `Equipment`-field map

**STATUS: GENUINE Category B (slight code-quality concern).** Two options:

| Option | Description | Pros | Cons |
|--------|-------------|------|------|
| **A: Hand-written match arms in `EquipSlot::read` and `EquipSlot::write`** [RECOMMENDED] | One match per direction; 8 arms each. | Trivially debuggable. Type-safe. | Verbose (~30 LOC). |
| B: Macro-driven (`macro_rules!` to generate the match arms from a list of `(EquipSlot, field)` pairs) | DRY: 8 entries instead of 16 arms. | Saves ~10 LOC. | Macros add cognitive cost; the 8-slot enum isn't going to grow. YAGNI. |

**Recommended:** A. The 8-slot enum is stable; macros add complexity without payoff.

### Decision D8 (NEW — discovered): Asset placeholder icons — who, what, when

**STATUS: GENUINE Category B.** Three options:

| Option | Description | Pros | Cons |
|--------|-------------|------|------|
| **A: ImageMagick-generated placeholder PNGs (32×32 colored squares with letter codes), checked in by the implementer** [RECOMMENDED] | Plumbing-complete; `icon_path` resolves to a real file; integration test can verify file existence. | ~30 minutes of setup; placeholder visual quality is acceptable for v1. |
| B: Skip the PNGs — `icon_path` references nonexistent files for v1 | Zero asset work | Roadmap line 679 explicitly says "produce 5-10 placeholder PNGs". Deviates without strong reason. UI in #25 will fail to resolve icons immediately. |
| C: Source from a CC0 icon pack (e.g., game-icons.net) | Real-looking icons | Requires asset attribution; takes longer; introduces a license cardinality (CC-BY 3.0 in some packs). |

**Recommended:** A. Placeholder generation script committed at `scripts/gen_placeholder_icons.sh` so future contributors can regenerate. **Surface as a sub-task in the plan** so the implementer knows it's part of #12's Definition of Done.

---

## Open Questions

1. **OQ1: How does `recompute_derived_stats_on_equipment_change` interact with `derive_stats`'s `current_*` clamp contract?**
   - What we know: `derive_stats` returns `current_hp = max_hp` (`character.rs:295-298`). The recompute system must clamp `current = current.min(new_max)` AFTER calling derive_stats to preserve in-combat depletion.
   - What's unclear: Whether the clamp logic should live in `recompute_derived_stats_on_equipment_change` directly or in a shared helper. v1 is fine with inlined clamp; #14 / #15 may surface multiple recompute callers (level-up, status-change) that share the clamp pattern.
   - Recommendation: Inline in #12. Refactor to shared `apply_derived_stats_with_clamp(...)` helper in #14/#15 if it shows up there.

2. **OQ2: Does `give_item` need an idempotence guard like `spawn_default_debug_party`?**
   - What we know: The function spawns one entity per call. Multiple calls produce multiple entities (correct: 5x healing potions = 5 entities).
   - What's unclear: Whether some caller wants "give once, ignore duplicates" semantics (e.g., a quest reward).
   - Recommendation: NO idempotence in #12. Add a separate `give_item_unique(...)` later if #21 (loot) needs it.

3. **OQ3: Per-instance enchantment — when does it land?**
   - What we know: The doc comment at `character.rs:204-205` promises "per-instance state ... lands in #12 as a separate `ItemInstance` entity model." The `ItemInstance` entity is created by #12 — but #12 ships the entity with **only** `ItemInstance(Handle)`, no enchantment/durability components.
   - What's unclear: Whether the doc comment requires #12 to declare empty `Enchantment(u8)` / `Durability(u32)` components now, or whether those land with the first system that reads them (#21 loot, #15 combat).
   - Recommendation: **Do not declare** `Enchantment` / `Durability` in #12. Surface them when the first reader lands. Defer per the project's "second consumer" discipline. The doc-comment promise is satisfied by the existence of the entity model itself; per-instance state can be layered on later.

4. **OQ4: Stackable potions — should the planner re-litigate the punt?**
   - What we know: Roadmap line 681 punts. v1 ships unique entities per potion.
   - What's unclear: Whether the user is willing to pay 1-2 days for a `StackableItem(u32)` shape now to avoid the inventory-bloat issue in #21.
   - Recommendation: Default to PUNT (mirror roadmap). Surface as Decision D9 if planner thinks it's worth re-litigating. Default answer: trust the roadmap.

5. **OQ5: Should `EquipmentChangedEvent` carry the entity that was just equipped/unequipped?**
   - What we know: Pattern 4 above carries `character: Entity` and `slot: EquipSlot`. The recompute system only needs these.
   - What's unclear: Whether UI in #25 / #21 wants the `Handle<ItemAsset>` or the inventory `Entity` to render an animation ("flash the equipped item icon").
   - Recommendation: Keep the lean shape (character + slot) for #12. Add `item: Option<Handle<ItemAsset>>` field later if #25 demands it.

6. **OQ6: Where does `ItemInstance` despawn happen on unequip?**
   - What we know: The recommended `unequip_item` spawns a new `ItemInstance` entity to push to inventory.
   - What's unclear: Whether the despawned-then-respawned pattern is more expensive than mutating an existing entity in place.
   - Recommendation: Keep the spawn/despawn pattern. Bevy entity creation is cheap; the alternative (parking entities in a "zombie" state) is more error-prone.

---

## Sources

### Primary (HIGH confidence)

- [Druum source — `src/plugins/party/character.rs`](file:///Users/nousunio/Repos/Learnings/claude-code/druum/src/plugins/party/character.rs) — lines 209-219 confirm `Equipment` carries `Handle<ItemAsset>`, NOT `Entity`; lines 322-348 confirm `derive_stats` is pure and takes `&[ItemStatBlock]`; lines 194-208 confirm the doc-comment promise for `ItemInstance` in #12
- [Druum source — `src/data/items.rs`](file:///Users/nousunio/Repos/Learnings/claude-code/druum/src/data/items.rs) — confirms `ItemAsset` and `ItemStatBlock` exist as #11 stubs ready to be fleshed out
- [Druum source — `src/plugins/loading/mod.rs`](file:///Users/nousunio/Repos/Learnings/claude-code/druum/src/plugins/loading/mod.rs) — lines 33-34, 96-102 lock the asset path to `assets/items/core.items.ron`
- [Druum source — `src/plugins/dungeon/mod.rs`](file:///Users/nousunio/Repos/Learnings/claude-code/druum/src/plugins/dungeon/mod.rs) — lines 192-197 + 207 + 686-690 confirm the canonical `Message`/`MessageWriter`/`MessageReader` pattern (`MovedEvent`)
- [Druum source — `src/plugins/audio/sfx.rs`](file:///Users/nousunio/Repos/Learnings/claude-code/druum/src/plugins/audio/sfx.rs) — lines 42-45 + 67 confirm the `MessageReader` consumer pattern, mirroring what `recompute_derived_stats_on_equipment_change` will do
- [Druum source — `src/data/classes.rs`](file:///Users/nousunio/Repos/Learnings/claude-code/druum/src/data/classes.rs) — lines 27-30 + 41-43 confirm the `ClassTable { Vec<ClassDef> } + ::get(...)` pattern that `ItemDb { Vec<ItemAsset> } + ::get(...)` mirrors
- [Druum source — `tests/class_table_loads.rs`](file:///Users/nousunio/Repos/Learnings/claude-code/druum/tests/class_table_loads.rs) — full structure of the integration test that `tests/item_db_loads.rs` mirrors
- [Druum source — `src/plugins/input/mod.rs`](file:///Users/nousunio/Repos/Learnings/claude-code/druum/src/plugins/input/mod.rs) — lines 80, 151 confirm `DungeonAction::OpenInventory = Tab` already wired (UI in #25 will consume this; #12 doesn't add input)
- [Druum source — `src/plugins/state/mod.rs`](file:///Users/nousunio/Repos/Learnings/claude-code/druum/src/plugins/state/mod.rs) — line 23 confirms `DungeonSubState::Inventory` already declared
- [Bevy 0.18.1 local source — `Message` trait](file:///Users/nousunio/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bevy_ecs-0.18.1/src/message/mod.rs) — line 23 confirms `pub use bevy_ecs_macros::Message;` is the canonical 0.18 derive
- [Roadmap §12](file:///Users/nousunio/Repos/Learnings/claude-code/druum/project/roadmaps/20260429-01-bevy-dungeon-crawler-roadmap.md) — lines 638-684 source the feature requirements; lines 675 + 681 are explicit decisions to mirror
- [Feature #11 research](file:///Users/nousunio/Repos/Learnings/claude-code/druum/project/research/20260504-160000-feature-11-party-character-ecs-model.md) — confirms #11 Decision 3 (Equipment = Handle), Decision 4 (single-file precedent), and the "FROZEN-from-day-one" pattern that #12 inherits

### Secondary (MEDIUM confidence)

- [Researcher memory — `feedback_bevy_0_18_event_message_split`](file:///Users/nousunio/Repos/Learnings/claude-code/druum/.claude/agent-memory/researcher/feedback_bevy_0_18_event_message_split.md) — confirms 0.18's family rename of `Event → Message`
- [Researcher memory — `feedback_bevy_input_test_layers`](file:///Users/nousunio/Repos/Learnings/claude-code/druum/.claude/agent-memory/researcher/feedback_bevy_input_test_layers.md) — confirms Layer 1/2/3 test pattern
- [Researcher memory — `reference_bevy_reflect_018_derive`](file:///Users/nousunio/Repos/Learnings/claude-code/druum/.claude/agent-memory/researcher/reference_bevy_reflect_018_derive.md) — confirms `#[derive(Reflect)]` handles enums + Vec + Option + Handle without extra attributes
- [Researcher memory — `feedback_third_party_crate_step_a_b_c_pattern`](file:///Users/nousunio/Repos/Learnings/claude-code/druum/.claude/agent-memory/researcher/feedback_third_party_crate_step_a_b_c_pattern.md) — confirms #12 has zero crate additions, so Step A/B/C is N/A

### Tertiary (LOW confidence — flagged for validation)

- DRPG genre conventions for slot count (8 vs 6 vs 4) — sourced from training data on Wizardry/Etrian/Grimrock manuals; no live verification this session. The 8-slot count is locked by #11's `Equipment` (so #12 must mirror).
- Item-as-entity vs item-as-record best practices in 2025/2026 Bevy ECS community — training data only; no live ecosystem search this session. The Hybrid recommendation (Option A) is grounded in #11's resolved decision and the project's own doc-comment, not in external community guidance.

---

## Metadata

**Confidence breakdown:**

- Live #11 ground truth: HIGH — every fact is grounded in direct file:line reads of the merged code.
- Standard stack + dep delta: HIGH — Cargo.toml directly read; zero deltas confirmed.
- Architecture Option A recommendation: HIGH — grounded in #11's resolved Decision 3 + doc-comment promise; reverting requires user override.
- Pitfalls 1-8: HIGH — each is grounded in #11 precedent or Bevy 0.18 source-verified API behavior.
- Decisions D1–D8: HIGH on D1 + D6 (auto-resolvable from #11); HIGH on D2 + D3 + D5 (genuine Category B with strong recommended defaults); MEDIUM on D4 + D7 + D8 (designer/style preferences).
- Tests + validation architecture: HIGH — patterns are direct copies of #11 (Layer 1) and #6/#10 (Layer 2 via MinimalPlugins+AssetPlugin).
- Asset icons recommendation: MEDIUM — directory + extension are HIGH (PNG is in Cargo.toml); the placeholder-generation method is a Category B style choice (D8).

**Research date:** 2026-05-05

**Dep delta:** 0. `Cargo.toml` is byte-unchanged. `Cargo.lock` is byte-unchanged. **The cleanest-ship signal applies — same as #9, #10, #11.**

**LOC estimate:** +400-600 LOC matches roadmap budget.
- `src/plugins/party/inventory.rs` — NEW, ~400-500 LOC (5 components, 2 enums, 1 message, 1 error, 4 helpers, 1 system, 6-8 Layer-1 tests, 2-3 Layer-2 tests)
- `src/plugins/party/mod.rs` — MODIFY, +20 LOC (mod declaration, re-exports, plugin additions)
- `src/data/items.rs` — MODIFY, +50-80 LOC (flesh `ItemAsset` to 8 fields + `ItemDb::get` + Layer-1 round-trip test)
- `src/data/mod.rs` — MODIFY, +0-2 LOC (re-exports may already cover)
- `assets/items/core.items.ron` — MODIFY, ~80 lines of RON
- `tests/item_db_loads.rs` — NEW, ~70 LOC

**Asset Δ:** +1 RON (already exists as stub; counts as a content swap), +5-10 PNG icons (counted as +5-10 toward roadmap "+1 RON, +5-10 icons" estimate at line 669).

**Test count Δ:** +6 to +10 tests, matching roadmap budget at line 670.

**No new dependencies.** Confirmed: every requirement of #12 maps to existing `Cargo.toml` deps.

**Files NOT touched:** `src/main.rs`, `src/plugins/state/mod.rs`, `src/plugins/loading/mod.rs`, `src/plugins/dungeon/mod.rs`, `src/plugins/input/mod.rs`, `src/plugins/audio/mod.rs`, `src/plugins/combat/mod.rs`, `src/plugins/ui/mod.rs`, `src/plugins/save/mod.rs`, `src/plugins/town/mod.rs`, `src/data/dungeon.rs`, `src/data/spells.rs`, `src/data/enemies.rs`, `src/data/classes.rs`, `src/plugins/party/character.rs`.
