---
name: Druum #18 Town Hub dependencies — what's pre-shipped before research
description: Feature #18 (Town Hub & Services) is plumbing + content layered on a stack that is fully in tree — TownLocation SubStates already declared/registered, ItemAsset.value already authored for shop pricing, give_item explicitly listed as a #18 caller, bgm_town pre-loaded, MenuAction reused per input/mod.rs doc, EquipmentChangedEvent dual-use for status changes. Δ Deps for #18 = 0.
type: reference
---

When planning/implementing Feature #18 (Town Hub & Services), DO NOT re-design any of these — they are already shipped and #18 must mirror them or use them as-is.

**Verified at HIGH confidence (read directly 2026-05-11):**

- **`TownLocation` SubStates is declared AND registered** at `src/plugins/state/mod.rs:38-47` and `mod.rs:56`. Variants: `Square` (default), `Shop`, `Inn`, `Temple`, `Guild`. The roadmap text "Implement `GameState::Town` with sub-states..." is stale — the state machine work is done.
- **`MenuAction` (leafwing) reused for Town input** per the explicit doc comment at `src/plugins/input/mod.rs:54-57`: *"Town reuses this enum in v1; `TownAction` is deferred..."*. Variants are Up/Down/Left/Right/Confirm/Cancel/Pause. `InputMap<MenuAction>` and `ActionState<MenuAction>` both registered.
- **`ItemAsset.value: u32`** is already authored at `src/data/items.rs:101-103` with doc *"Sell/buy value in gold. Used by #18 shop."* — no schema change needed for buy/sell base price.
- **`give_item` helper** at `src/plugins/party/inventory.rs:390-412` is explicitly documented as a #18 caller: *"Future callers: #21 (loot drops), #18 (shop), #25 (UI give-item)."* Use this, do NOT hand-roll the inventory push.
- **`equip_item` / `unequip_item`** at `inventory.rs:272-336, 346-380` are the only sanctioned writers of `Equipment` slots. Use them; their MessageWriter coordination already fires `EquipmentChangedEvent`.
- **`ItemHandleRegistry.get(id) -> Option<&Handle<ItemAsset>>`** at `inventory.rs:528-530` is populated on `OnExit(Loading)` by `populate_item_handle_registry`. Shop uses this to resolve item IDs to handles. No registry-construction work needed.
- **`recompute_derived_stats_on_equipment_change`** at `inventory.rs:444-501` reads `&StatusEffects` and has dropped the `With<PartyMember>` filter (memory `reference_druum_recompute_filter_dual_use.md`). Firing `EquipmentChangedEvent { slot: EquipSlot::None }` after status changes (temple-revive, inn-cure-poison) triggers stat re-derive without parallel pipeline.
- **`bgm_town` audio handle** at `src/plugins/loading/mod.rs:62-63` is part of `AudioAssets`. `play_bgm_for_state` at `audio/bgm.rs:106-112` handles the Town BGM crossfade automatically via `state_changed::<GameState>` — no work for #18 on audio.
- **`spawn_default_debug_party` precedent** at `src/plugins/party/mod.rs:89-144` is the recruit-spawn template: `commands.spawn(PartyMemberBundle { ... }).insert(Inventory::default())`. Guild recruit mirrors this exactly.
- **`PartySize: Resource` (default 4)** at `character.rs:344-351` — Guild slot count.
- **`PartyRow` (Front/Back)** and **`PartySlot(usize)`** at `character.rs:178-191` — Guild row swap and reorder targets.

**Δ Cargo.toml for #18 = 0.** `bevy_egui = "=0.39.1"`, `bevy_common_assets = "=0.16.0"`, `bevy_asset_loader = "=0.26.0"`, `serde`, `ron`, `leafwing-input-manager` all already pinned. NO Step A/B/C crate-verification gate triggered.

**What #18 actually adds:**
- `Resource<Gold>` (saturating u32 helpers) — new resource
- `Resource<GameClock>` (day + turn counter) — new resource (recommended, see research Open Q5)
- Three `Asset`-derived schemas: `ShopStock`, `RecruitPool`, `TownServices` — new types in `src/data/town.rs`
- One `AssetCollection`: `TownAssets` — sibling of `DungeonAssets`/`AudioAssets`
- Five paint systems (one per `TownLocation`) + five input handlers
- Three RON authoring files in `assets/town/`
- One `Camera2d` spawn pipeline tagged `TownCameraRoot` (NOT reusing `DungeonCamera` — the dungeon-camera-despawn-on-OnExit-Dungeon rule fights you; mirror the loading-screen `Camera2d` spawn pattern)

**LOC estimate:** ~1650 ± 200 including tests (~30% of LOC). Trimming tests to smoke would land in the roadmap's +800 to +1300 envelope; the recommendation is to keep the full test surface because Town is the gateway to many later features.

**Plan documents this carefully:** the research document at `project/research/20260511-feature-18-town-hub-and-services.md` enumerates 11 "Touch points" with file:line citations the planner can use directly.
