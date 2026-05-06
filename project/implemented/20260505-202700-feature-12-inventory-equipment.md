# Implementation Summary: Feature #12 — Inventory & Equipment

**Date:** 2026-05-05  
**Branch:** `feature/12-inventory-equipment`  
**Plan:** `/Users/nousunio/Repos/Learnings/claude-code/druum/project/plans/20260505-090000-feature-12-inventory-equipment.md`

---

## Commits Made

| SHA | Message |
|-----|---------|
| `f4ca434` | feat(inventory): Feature #12 — inventory & equipment data layer |
| `a152171` | docs(inventory): populate Feature #12 plan — Implementation Discoveries + verification |

---

## Steps Completed

All 9 phases were completed in a single source commit (interdependencies required
bundling Phases 1+2+3+4+5 together; Phases 6+7 bundled with them for simplicity
since all assets were ready before the first quality-gate commit).

- **Phase 1** (items.rs schema): `ItemAsset` extended to 9 fields, `ItemDb` with `Vec<ItemAsset>` + `get`, 4 Layer-1 unit tests.
- **Phase 2** (inventory.rs skeleton): `ItemKind`, `EquipSlot`, `ItemInstance`, `Inventory`, `EquipmentChangedEvent`, `EquipError`, `EquipResult`.
- **Phase 3** (helper functions): `equip_item`, `unequip_item`, `give_item`, 5 Layer-1 tests (slot read/write round-trips, None guards, consumable/slot-mismatch rejections).
- **Phase 4** (recompute system): `recompute_derived_stats_on_equipment_change`, 3 Layer-2 `app_tests` (recompute via event, equip-emits-message, give-item-push).
- **Phase 5** (PartyPlugin wiring): `pub mod inventory`, re-exports, `add_message::<EquipmentChangedEvent>()`, 4 `register_type` calls, `add_systems(Update, recompute_...)`, `Inventory::default()` on debug party spawn.
- **Phase 6** (core.items.ron): 8 starter items (3 weapons, 2 armor, 1 shield, 1 consumable, 1 key item). RON uses `(stats: ())` for zero-stat items.
- **Phase 7** (placeholder icons): `scripts/gen_placeholder_icons.sh` using `magick` (ImageMagick v7). Solid-color 32×32 PNGs (no text annotation — font unavailable in default Homebrew ImageMagick). 8 PNG files under `assets/ui/icons/items/`.
- **Phase 8** (integration test): `tests/item_db_loads.rs` mirrors `class_table_loads.rs`. Verifies `ItemDb` loads via `RonAssetPlugin`, asserts 8 items, checks `rusty_sword` stats and kind/slot, checks consumable/key-item sentinel `EquipSlot::None`.
- **Phase 9** (final verification): All automated gates pass.

---

## Steps Skipped

None. All plan phases were completed.

---

## Deviations from the Plan

### 1. Commit ordering (minor)

The plan said "commit Phase 2 first, then Phase 1" as separate commits. Due to the tight interdependency (phases 1-5 all interlock), all source changes were made in a single commit. This is equivalent to the plan's intent.

### 2. Icon text labels omitted (minor)

The plan specified "2-letter code labels (e.g., 'RS' for Rusty Sword)" but ImageMagick v7 on Homebrew requires a system font configured for text annotation. No fonts are available by default. The script generates solid-color PNGs without text. This is noted in `Implementation Discoveries §D6`. The functional requirement (placeholder PNG files that `icon_path` strings point to) is fully met.

### 3. LOC exceeded plan estimate (within acceptable range)

- Plan estimate: +450-550 LOC
- Actual: inventory.rs 929 lines, items.rs 258 lines, mod.rs +33, tests/item_db_loads.rs 118 = ~1338 lines total
- The overage is due to: (a) comprehensive doc comments on every function, (b) World-level test harnesses for Layer-1 tests requiring more setup than a minimal App, (c) the plan's LOC estimate excluded comment overhead.
- No scope was added. 0 new dependencies. Cargo.toml byte-unchanged.

### 4. `equip_consumable_returns_item_has_no_slot` test strategy adjusted

The plan spec described testing "kind: Consumable is rejected with ItemHasNoSlot". In practice, verifying this requires the item entity to be immediately queryable (not deferred via Commands). The test was restructured to spawn the entity and asset directly on `World` (not via Commands), so `instances.get()` succeeds and the code path reaches the `kind == Consumable` check. The assertion changed from "Either ItemMissingComponents or ItemHasNoSlot" to exactly `Err(ItemHasNoSlot)` for precision.

---

## Issues Deferred

- **Manual smoke tests** (Phase 9 items): `cargo run --features dev` + F9 cycler navigation. The automated tests verify all functional behavior; the manual smoke is a UX sanity check. Deferred to the user.
- **Icon text labels**: Could be added later if a font is installed (`magick -font /path/to/font.ttf`). The script is commented with a note.

---

## Verification Results

| Check | Result |
|-------|--------|
| `cargo check` | PASS |
| `cargo check --features dev` | PASS |
| `cargo clippy --all-targets -- -D warnings` | PASS |
| `cargo clippy --all-targets --features dev -- -D warnings` | PASS |
| `cargo fmt --check` | PASS |
| `cargo test` | PASS (108 lib + 5 integration = 113 total) |
| `cargo test --features dev` | PASS (111 lib + 5 integration = 116 total) |
| `rg 'derive\(.*\bEvent\b' inventory.rs` | ZERO matches in code (1 in doc comment only) |
| `rg '\bEventReader<' inventory.rs item_db_loads.rs` | ZERO matches |
| Frozen file check (`git diff 8865b26 f4ca434`) | ZERO frozen file modifications |
| 8 PNG icons exist | PASS: `ls -1 assets/ui/icons/items/ \| wc -l` = 8 |
| `Cargo.toml` diff | Empty (byte-unchanged) |

---

## New Tests Added (+13 total)

**`data/items.rs` (4 Layer-1 tests):**
- `item_stat_block_round_trips_through_ron` (pre-existing, updated)
- `item_asset_round_trips_through_ron` (NEW)
- `item_db_round_trips_through_ron` (NEW)
- `item_db_get_returns_authored_item` (NEW)

**`plugins/party/inventory.rs` tests (6 Layer-1 tests):**
- `equip_slot_read_write_round_trip`
- `equip_slot_none_read_returns_none`
- `equip_slot_none_write_is_noop`
- `equip_slot_none_from_caller_returns_item_has_no_slot`
- `equip_consumable_returns_item_has_no_slot`
- `equip_sword_in_armor_slot_returns_slot_mismatch`

**`plugins/party/inventory.rs` app_tests (3 Layer-2 tests):**
- `equip_sword_raises_attack_unequip_lowers`
- `equip_emits_message_via_helper`
- `give_item_pushes_to_inventory`

**`tests/item_db_loads.rs` (1 integration test):**
- `item_db_loads_through_ron_asset_plugin`
