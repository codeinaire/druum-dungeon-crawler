# Implementation Summary: Feature #18a — Town Square, Shop, Inn

**Plan:** `project/plans/20260511-180000-feature-18a-town-square-shop-inn.md`
**Date completed:** 2026-05-11
**Sessions:** Three — prior session (phases 1-10) + this continuation session (Phase 11 static analysis)

---

## Files Created

| Path | Description |
|---|---|
| `src/data/town.rs` | RON-loaded schemas: `ShopStock`, `ShopEntry`, `RecruitPool`, `RecruitDef`, `TownServices`; `clamp_shop_stock` trust-boundary helper; `MAX_SHOP_ITEMS`/`MAX_INN_COST` constants |
| `src/plugins/town/gold.rs` | `Gold(u32)` resource with `try_spend`/`earn`; `GameClock { day, turn }` resource; `SpendError` |
| `src/plugins/town/square.rs` | `SquareMenuState`, `paint_town_square` painter, `handle_square_input` handler |
| `src/plugins/town/shop.rs` | `ShopState`, `ShopMode`, `BuyError`, `SellError`, `MAX_INVENTORY_PER_CHARACTER`; `buy_item`/`sell_item` helpers; `paint_shop` painter; `handle_shop_input` handler |
| `src/plugins/town/inn.rs` | `InnState`, `paint_inn` painter, `handle_inn_rest` handler |
| `src/plugins/town/placeholder.rs` | `paint_placeholder` painter, `handle_placeholder_input` handler for Temple/Guild stubs |
| `assets/town/shop_stock.ron` | 7 shop entries (weapons, armor, shield, consumable; all `min_floor: 0`) |
| `assets/town/recruit_pool.ron` | 5 NPC recruits across Race/Class spread (zero readers in #18a; consumed by #18b Guild) |
| `assets/town/town_services.ron` | Inn cost 10 gold, cures Poison; temple_* fields omitted (default via #[serde(default)]) |

## Files Modified

| Path | Reason |
|---|---|
| `src/data/mod.rs` | Added `pub mod town;` and `pub use town::{RecruitDef, RecruitPool, ShopEntry, ShopStock, TownServices};` |
| `src/plugins/loading/mod.rs` | Added `TownAssets` struct + three `RonAssetPlugin` registrations + `.load_collection::<TownAssets>()`; imported `ShopStock`, `RecruitPool`, `TownServices` |
| `src/plugins/town/mod.rs` | Replaced 17-line stub with full `TownPlugin`: camera lifecycle, `TownCameraRoot` marker, painter tuple in `EguiPrimaryContextPass`, handler tuple in `Update`, both using `.distributive_run_if(in_state(GameState::Town))` |

## Deviations from Plan

All deviations are documented in the plan's `## Implementation Discoveries` section. Highlights:

1. **EguiPlugin unavailable in headless tests** — painter systems not tested headlessly; test apps add only Update/OnEnter/OnExit systems. Matches `combat/ui_combat.rs` pattern.
2. **B0002 double-borrow on buy/sell** — `buy_item`/`sell_item` free functions defined and exported but buy/sell logic inlined in `handle_shop_input` to avoid conflicting queries. Free functions remain as API for future callers.
3. **`AssetPlugin` required for `init_asset`** — added to all test apps using `init_asset::<T>()`.
4. **Test module explicit imports** — `use super::*` scope semantics required explicit re-imports in some test modules (notably `data/town.rs`).
5. **`assertions_on_constants` clippy lint** — `buy_item_gold_deduction_only_after_give` test uses a real `Gold::try_spend` assertion instead of `assert!(true)`.
6. **MessageReader import removed** — `rest_fires_equipment_changed_event_per_member` uses HP-proxy assertions; unused `MessageReader` import removed.
7. **Shell tool unavailable (all three sessions)** — Quality gates (cargo check, test, clippy) could not be run programmatically. Thorough static analysis performed instead. See separate note below.

## Quality Gates Status

**GATES NOT RUN** — shell/bash execution was not available in any of the three implementation sessions. Thorough static analysis was performed:

- All type signatures verified against their call sites
- All import chains traced from source to use
- RON file syntax checked against their Rust schema types
- Potential clippy patterns reviewed: `doc_lazy_continuation`, `assertions_on_constants`, `manual_range_contains`, `unused_imports`, `dead_code`, `collapsible_if`
- No issues found in static review

The six gates must be run before shipping:
```
cargo check
cargo check --features dev
cargo test
cargo test --features dev
cargo clippy --all-targets -- -D warnings
cargo clippy --all-targets --features dev -- -D warnings
```

## LOC Count (approximate, new files only)

| File | Approx LOC |
|---|---|
| `src/data/town.rs` | ~342 (including tests) |
| `src/plugins/town/gold.rs` | ~163 |
| `src/plugins/town/square.rs` | ~238 |
| `src/plugins/town/shop.rs` | ~626 |
| `src/plugins/town/inn.rs` | ~452 |
| `src/plugins/town/placeholder.rs` | ~144 |
| `assets/town/*.ron` | ~120 |
| **Total new LOC** | **~2085** (including tests) |

Note: LOC is higher than the plan's ~1100 estimate because test coverage is more thorough than initially planned.

## Test Count Added

New `#[test]` functions across all new files:

| File | Tests |
|---|---|
| `src/data/town.rs` | 6 (shop_stock_ron_round_trips, stock_filters_by_min_floor, clamp_shop_stock_truncates_oversized, clamp_shop_stock_passes_through_small_stock, recruit_pool_ron_round_trips, town_services_ron_round_trips_with_defaulted_temple_fields) |
| `src/plugins/town/gold.rs` | 5 (try_spend_insufficient_returns_err_without_mutation, try_spend_exact_succeeds, try_spend_underflow_guard, earn_saturates_at_u32_max, earn_normal_addition) |
| `src/plugins/town/square.rs` | 4 (square_confirm_at_cursor_0_navigates_to_shop, square_confirm_at_cursor_4_navigates_to_titlescreen, square_down_wraps_cursor, square_up_wraps_cursor) |
| `src/plugins/town/shop.rs` | 8 (max_inventory_per_character_constant_is_8, sell_price_is_half_of_value_integer_division, shop_mode_defaults_to_buy, shop_state_defaults, buy_error_inventory_full_and_insufficient_gold_are_distinct, buy_item_rejects_when_inventory_full, buy_item_rejects_when_insufficient_gold, buy_item_gold_deduction_only_after_give) |
| `src/plugins/town/inn.rs` | 7 (rest_full_heals_living_party, rest_skips_dead_party_member, rest_cures_poison_but_not_stone, rest_advances_clock_day_by_one, rest_deducts_configured_cost, rest_rejects_when_insufficient_gold, rest_fires_equipment_changed_event_per_member) |
| `src/plugins/town/placeholder.rs` | 2 (placeholder_cancel_returns_to_square_from_temple, placeholder_cancel_returns_to_square_from_guild) |
| `src/plugins/town/mod.rs` | 3 (town_plugin_builds, town_camera_spawns_on_enter_and_despawns_on_exit, town_substate_defaults_to_square_on_enter) |
| **Total new tests** | **35** |

## Deferred Issues

1. **Quality gates unrun** — must be run before PR merge. If any gate fails, fix and re-verify.
2. **Manual smoke test** — `cargo run --features dev`, F9 to Town, exercise all sub-states — must be done by a human.
3. **Verification checklist items** — plan's verification section has module-specific test names; some names differ from actual test names (test count and coverage are equivalent).
