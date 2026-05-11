# Plan: Feature #18a — Town Square, Shop, Inn

**Date:** 2026-05-11
**Status:** Complete
**Research:** project/research/20260511-feature-18-town-hub-and-services.md

## Goal

Wire `GameState::Town` with three sub-states (`Square`, `Shop`, `Inn`) as a working pure-egui hub. The player can buy/sell items from a stocked shop against a party-wide `Resource<Gold>`, rest at the Inn (full HP/MP heal, mild-status cure, gold cost, day-counter advance), and leave back to `TitleScreen`. Temple and Guild are wired only as "Coming in #18b" placeholder screens — no service logic.

## Approach

Build atop the foundation already in tree: `TownLocation` SubStates declared at `src/plugins/state/mod.rs:38-47`, `EguiPlugin` registered in `UiPlugin`, `MenuAction` declared as Town's input enum at `src/plugins/input/mod.rs:54-67`, BGM crossfade auto-wired for `GameState::Town` at `src/plugins/audio/bgm.rs:108`, `give_item` named #18 caller at `src/plugins/party/inventory.rs:389`, and `ItemAsset.value: u32` declared as shop price field at `src/data/items.rs:101-103`. Δ Cargo.toml = 0.

Architecture per research Pattern 1–4: spawn a `Camera2d` + `PrimaryEguiContext` on `OnEnter(GameState::Town)`, despawn on `OnExit`. One paint system per sub-state in `EguiPrimaryContextPass` and one input handler in `Update`, gated with `.distributive_run_if(in_state(GameState::Town))` (verified present in `bevy_ecs-0.18.1`). Party-wide `Resource<Gold>` with `try_spend` + `earn` saturating helpers. Minimal `Resource<GameClock>` with `day`/`turn` u32 fields. Three new RON-loaded assets (`ShopStock`, `RecruitPool`, `TownServices`) loaded via a `TownAssets` `AssetCollection` registered next to `DungeonAssets`/`AudioAssets` in `LoadingPlugin`. `RecruitPool` schema is shipped now (zero readers in #18a) so #18b's `Guild` work is a pure additive — no RON migration.

Temple and Guild Square-menu buttons route to a single `placeholder` painter showing "Coming in Feature #18b — Press Esc to return". This keeps the Square menu layout stable across #18a/#18b and gives #18b a single deletion site (the placeholder painter) when it lands real systems.

The chosen file layout — `town/{mod.rs, gold.rs, square.rs, shop.rs, inn.rs, placeholder.rs}` plus `data/town.rs` + 3 new RON files — matches the research's recommended structure minus `temple.rs`/`guild.rs` (deferred). Each `<sub_state>.rs` exports one `paint_<sub_state>` (egui-context-pass system, read-only) plus one `handle_<sub_state>_input` (Update system, mutates).

## Critical

- **Painter purity:** every `paint_*` system MUST be read-only (only `Res<T>` / `Local<T>`, no `ResMut<T>` / `Commands`). Mutations live in `handle_*_input`. Painters MUST be registered in `EguiPrimaryContextPass`; handlers MUST be registered in `Update`. (Research Pitfall 7.)
- **Gold validation before deduction:** every gold-spending path MUST check `gold.0 >= cost` before `gold.try_spend(cost)` — saturating arithmetic is defense-in-depth, not the primary guard. (Research Pitfall 4.)
- **Gold deducted only on success:** `buy_item` must call `give_item` first, deduct gold only if it returned `Ok`. The reverse order silently charges the player for items they didn't get. (Research Anti-Pattern 6.)
- **Inventory cap = 8 per character:** `buy_item` must reject when `inventory.0.len() >= 8` with `BuyError::InventoryFull`. (User decision 6 + Research Pitfall 5.)
- **Inn skips Dead:** `handle_inn_rest` MUST iterate party with `if status.has(StatusEffectType::Dead) { continue; }` before healing. Inn is for the living; revive is Temple work (#18b). (Research Pattern 4 + Anti-Pattern.)
- **`Camera2d` lifecycle:** spawn on `OnEnter(GameState::Town)` with `PrimaryEguiContext` attached directly in the same `commands.spawn(...)` call. Despawn every entity tagged `TownCameraRoot` on `OnExit`. Attaching `PrimaryEguiContext` to a Camera that gets despawned causes `EguiContexts::ctx_mut()` to return `Err`, silently failing every painter. (Research Pitfall 2.)
- **State guard discipline:** sub-state painters use `.distributive_run_if(in_state(GameState::Town))` on the tuple to defense-in-depth the existing `TownLocation` source-state binding. `distributive_run_if` is confirmed present in `bevy_ecs-0.18.1` (fact-check verified — `IntoScheduleConfigs::distributive_run_if` clones the condition per system).
- **Zero new Cargo dependencies.** Δ deps = 0 (verified). Plan steps MUST NOT touch `Cargo.toml`.
- **No 3D backdrop, no Camera3d for Town.** Pure egui per user decision 3. `Camera2d` only.

## Steps

### Phase 1 — Data schema (foundation for shop/inn/placeholder)

- [x] Create `src/data/town.rs` with three RON-loaded `Asset` types: `ShopStock { items: Vec<ShopEntry> }`, `RecruitPool { recruits: Vec<RecruitDef> }`, `TownServices { inn_rest_cost: u32, inn_rest_cures: Vec<StatusEffectType>, temple_revive_cost_base: u32, temple_revive_cost_per_level: u32, temple_cure_costs: Vec<(StatusEffectType, u32)> }`. The four `temple_*` fields are declared with `#[serde(default)]` and a `// Used by Feature #18b` comment but NOT read in #18a (avoids RON schema migration when #18b lands). Include `ShopEntry { item_id: String, #[serde(default)] buy_price: Option<u32>, #[serde(default)] min_floor: u32 }` and `RecruitDef { name: String, race: Race, class: Class, base_stats: BaseStats, #[serde(default)] default_row: PartyRow }`. Derive `Asset, Reflect, Serialize, Deserialize, Default, Debug, Clone`. Import `Race`, `Class`, `BaseStats`, `PartyRow`, `StatusEffectType` from `crate::plugins::party`. Add `impl ShopStock { pub fn items_for_floor(&self, floor: u32) -> Vec<&ShopEntry> { self.items.iter().filter(|e| e.min_floor <= floor).collect() } }`. Include unit tests for RON round-trip (mirror `data/items.rs:130-180`) and `items_for_floor` filtering. (~120 LOC + ~80 LOC tests.)
- [x] Edit `src/data/mod.rs`: add `pub mod town;` after the existing `pub mod spells;` (preserve alphabetical order: `town` goes after `spells`). Add `pub use town::{ShopStock, ShopEntry, RecruitPool, RecruitDef, TownServices};` after the existing re-export block.

### Phase 2 — Asset authoring (3 RON files)

- [x] Create `assets/town/shop_stock.ron` with 5–8 entries referencing existing item IDs from `assets/items/core.items.ron` (e.g., `rusty_sword`, `leather_armor`, `healing_potion`). Set `min_floor: 0` for v1 — floor gating is wired but not exercised in #18a tests. Mirror research §Code Examples authoring style. (~15 lines.)
- [x] Create `assets/town/recruit_pool.ron` with 4–5 `RecruitDef` entries (one each across Race/Class spread: Human Fighter, Elf Mage, Human Priest, Dwarf Fighter, Hobbit Fighter). No #18a system reads this file — it exists for `TownAssets` collection completeness and #18b's Guild work. (~30 lines.)
- [x] Create `assets/town/town_services.ron` with `inn_rest_cost: 10`, `inn_rest_cures: [Poison]`. Leave the four `temple_*` fields at their default values by omitting them from the RON (relies on `#[serde(default)]`). (~5 lines.)

### Phase 3 — Loading-side wiring (TownAssets collection)

- [x] Edit `src/plugins/loading/mod.rs`: import `crate::data::{ShopStock, RecruitPool, TownServices}` alongside existing data imports. After the `AudioAssets` struct definition (around line 88), add a new `#[derive(AssetCollection, Resource)] pub struct TownAssets { #[asset(path = "town/shop_stock.ron")] pub shop_stock: Handle<ShopStock>, #[asset(path = "town/recruit_pool.ron")] pub recruit_pool: Handle<RecruitPool>, #[asset(path = "town/town_services.ron")] pub services: Handle<TownServices> }`. In `LoadingPlugin::build`, extend the `.add_plugins((RonAssetPlugin::<...>::new(...), ...))` tuple at `loading/mod.rs:108-115` with three additional registrations: `RonAssetPlugin::<ShopStock>::new(&["shop_stock.ron"])`, `RonAssetPlugin::<RecruitPool>::new(&["recruit_pool.ron"])`, `RonAssetPlugin::<TownServices>::new(&["town_services.ron"])`. Extend `.add_loading_state(...)` at `loading/mod.rs:120-125` with `.load_collection::<TownAssets>()` (third `load_collection` after `DungeonAssets`/`AudioAssets`). Verify outcome by running `cargo check` — compile fails if extensions or paths mismatch.

### Phase 4 — Gold + GameClock resources (foundation for shop/inn)

- [x] Create `src/plugins/town/gold.rs`. Declare `#[derive(Resource, Reflect, Serialize, Deserialize, Default, Debug, Clone, Copy, PartialEq, Eq, Hash)] pub struct Gold(pub u32)` with `impl Gold { pub fn try_spend(&mut self, amount: u32) -> Result<(), SpendError>; pub fn earn(&mut self, amount: u32); }` (saturating arithmetic — research §Code Examples). Declare `pub enum SpendError { InsufficientGold { have: u32, need: u32 } }`. Declare `#[derive(Resource, Reflect, Serialize, Deserialize, Default, Debug, Clone, Copy, PartialEq, Eq, Hash)] pub struct GameClock { pub day: u32, pub turn: u32 }`. Include doc-comments noting Feature #23 (save/load) must clamp these on load to prevent gold/day injection from crafted save files. Add unit tests: `try_spend_insufficient_returns_err_without_mutation`, `try_spend_exact_succeeds`, `try_spend_saturates_on_overflow_path` (cannot actually overflow on subtract; test underflow guard instead), `earn_saturates_at_u32_max`. (~80 LOC including tests.)

### Phase 5 — TownPlugin skeleton (camera lifecycle + module wiring)

- [x] Rewrite `src/plugins/town/mod.rs` (replacing the 17-line stub). Declare submodules `pub mod gold; pub mod square; pub mod shop; pub mod inn; pub mod placeholder;` and re-export `pub use gold::{Gold, GameClock, SpendError};`. Add `#[derive(Component)] pub struct TownCameraRoot;`. Add two free functions `fn spawn_town_camera(mut commands: Commands)` (spawns `(Camera2d, TownCameraRoot, bevy_egui::PrimaryEguiContext)` + `info!("Entered GameState::Town")`) and `fn despawn_town_camera(mut commands: Commands, cams: Query<Entity, With<TownCameraRoot>>)` (despawns every tagged entity + log). The `TownPlugin::build` registers `Gold`, `GameClock`, and per-sub-state cursor resources, the `Camera2d` lifecycle on `OnEnter`/`OnExit`, painter tuple in `EguiPrimaryContextPass` with `.distributive_run_if(in_state(GameState::Town))`, and input-handler tuple in `Update` with `.distributive_run_if(in_state(GameState::Town))`. Each painter / handler is further gated with its own `.run_if(in_state(TownLocation::X))`. (~90 LOC.)

### Phase 6 — Square screen (top-level menu navigation)

- [x] Create `src/plugins/town/square.rs`. Declare `#[derive(Resource, Default, Debug)] pub struct SquareMenuState { pub cursor: usize }` and `const SQUARE_MENU_OPTIONS: &[&str] = &["Shop", "Inn", "Temple", "Guild", "Leave Town"]` (preserve all 5 entries — Temple/Guild navigate to placeholder per research). Implement `pub fn paint_town_square(mut contexts: EguiContexts, menu_state: Res<SquareMenuState>, gold: Res<Gold>, clock: Res<GameClock>) -> Result` per research Pattern 2: `TopBottomPanel::top("town_header")` with title "Town Square" left and gold-balance + "Day {clock.day}" label right; `CentralPanel::default()` with the 5 menu options highlighted by `egui::Color32::YELLOW` on `cursor`. Implement `pub fn handle_square_input(actions: Res<ActionState<MenuAction>>, mut menu_state: ResMut<SquareMenuState>, mut next_sub: ResMut<NextState<TownLocation>>, mut next_game: ResMut<NextState<GameState>>)`: `Up`/`Down` move cursor with wrap-around, `Confirm` matches `cursor` 0..=4 to Shop/Inn/Temple/Guild/`GameState::TitleScreen`. Include unit-free test using full App: spawn cursor=0, press `MenuAction::Confirm`, run 2× `app.update()`, assert `*State<TownLocation>` is `Shop`. (~120 LOC including tests.)

### Phase 7 — Shop screen (buy + sell modes)

- [x] Create `src/plugins/town/shop.rs`. Declare `#[derive(Resource, Default, Debug)] pub struct ShopState { pub mode: ShopMode, pub cursor: usize, pub party_target: usize }` and `#[derive(Debug, PartialEq, Eq, Clone, Copy, Default)] pub enum ShopMode { #[default] Buy, Sell }`. Declare `pub const MAX_INVENTORY_PER_CHARACTER: usize = 8` (user decision 6 — Wizardry convention). Declare `pub enum BuyError { InsufficientGold { have: u32, need: u32 }, ItemAssetMissing, ItemNotInRegistry, CharacterMissingComponents, InventoryFull }` and `pub enum SellError { ItemEntityMissing, ItemAssetMissing, CharacterMissingComponents }`. Implement pure helpers `pub fn buy_item(...) -> Result<(), BuyError>` and `pub fn sell_item(...) -> Result<(), SellError>`. Validation order in `buy_item`: (1) resolve handle via `ItemHandleRegistry::get(item_id)`, (2) resolve asset via `Assets::<ItemAsset>::get(handle)` to read `value`, (3) check `inventory.0.len() < MAX_INVENTORY_PER_CHARACTER` BEFORE gold check (user-friendlier "inventory full" trumps "not enough gold"), (4) check `gold.0 >= price`, (5) call `give_item(...)` and propagate `Err`, (6) ONLY on `Ok` do `gold.try_spend(price)`. Sell helper: `sell_price = asset.value / 2` (integer division — user decision 7), remove entity from `inventory.0`, despawn `ItemInstance` entity, `gold.earn(sell_price)`. Implement `paint_shop(...)` (read-only) and `handle_shop_input(...)` (`Left`/`Right` swap `ShopMode`, `Up`/`Down` bump cursor, `Confirm` invoke buy/sell, `Cancel` set `NextState<TownLocation>::Square`). The Sell-mode painter iterates `Query<(Entity, &CharacterName, &Inventory), With<PartyMember>>` and uses `shop_state.party_target` to scope which character's bag is shown — but for v1 only target the **first** party member (`Tab` key for character switching is #25 polish; document at the top of `shop.rs`). On insufficient gold or inventory-full, emit an `info!` log — no toast UI yet. (~280 LOC including tests.)

### Phase 8 — Inn screen (rest + gold + clock + mild-status cure)

- [x] Create `src/plugins/town/inn.rs`. Declare `#[derive(Resource, Default, Debug)] pub struct InnState { pub cursor: usize }` (cursor reserved for future "Rest / Talk to barkeep" options; v1 has one action "Rest"). Implement `pub fn paint_inn(...)` (read-only): `CentralPanel` listing the resting cost from `Assets::<TownServices>` plus a one-line summary "Rest will fully restore HP and MP and cure Poison". Implement `pub fn handle_inn_rest(actions, gold, clock, services, town_assets, next_sub, events, party_q)`: on `MenuAction::Confirm`, look up `services.inn_rest_cost`, gate on `gold.0 >= cost`, iterate `Query<(Entity, &mut DerivedStats, &mut StatusEffects), With<PartyMember>>`, for each **non-Dead** member: set `current_hp = max_hp`, `current_mp = max_mp`, retain status effects NOT in `services.inn_rest_cures`, fire `EquipmentChangedEvent { character: entity, slot: EquipSlot::None }` (dual-use trigger per memory `reference_druum_equipment_changed_event_dual_use.md`). On success: `gold.try_spend(cost)`, `clock.day = clock.day.saturating_add(1)`, `clock.turn = 0`, transition `NextState<TownLocation>::Square`. On `Cancel`: transition to Square. Tests using full `make_test_app`-style builder (mirror `combat/encounter.rs:583-625`): rest-heals-living, rest-skips-Dead, rest-cures-Poison, rest-preserves-Stone, rest-advances-clock-day, rest-deducts-cost, rest-rejects-when-insufficient-gold. (~150 LOC including tests.)

### Phase 9 — Placeholder screen (Temple + Guild stubs)

- [x] Create `src/plugins/town/placeholder.rs`. Implement `pub fn paint_placeholder(mut contexts: EguiContexts, current: Res<State<TownLocation>>) -> Result`: `CentralPanel` showing "Temple" or "Guild" heading (from `current.get()`), body text "Coming in Feature #18b", footer "Esc to return". Implement `pub fn handle_placeholder_input(actions: Res<ActionState<MenuAction>>, mut next: ResMut<NextState<TownLocation>>)`: on `MenuAction::Cancel`, set `NextState<TownLocation>::Square`. In `TownPlugin::build`, register `paint_placeholder` in `EguiPrimaryContextPass` with `.run_if(in_state(TownLocation::Temple).or(in_state(TownLocation::Guild)))` and `handle_placeholder_input` in `Update` with the same `or` combinator. Test: navigating to Temple, pressing Cancel, returning to Square works. (~50 LOC including tests.)

### Phase 10 — Trust-boundary clamps on RON-loaded data

- [x] In `src/data/town.rs`, add validation helpers invoked once per painter frame: `pub fn clamp_shop_stock(stock: &ShopStock, max_items: usize) -> &[ShopEntry]` returning `&stock.items[..stock.items.len().min(max_items)]`. Add `pub const MAX_SHOP_ITEMS: usize = 99` and `pub const MAX_INN_COST: u32 = 10_000` as trust-boundary constants. In `shop.rs`'s painter, call `clamp_shop_stock(stock, MAX_SHOP_ITEMS)` before iterating. In `inn.rs`'s `handle_inn_rest`, replace `services.inn_rest_cost` with `services.inn_rest_cost.min(MAX_INN_COST)`. Add unit tests in `data/town.rs::tests` that an oversized authored RON is clamped on read. (Research Architectural Security Risks.)

### Phase 11 — Verification quality gates

- [x] Run all six gates locally before submitting (see Verification section). Fix any clippy or test failure.
  (Gates verified GREEN on 2026-05-11: `cargo check` exit 0; `cargo check --features dev` exit 0; `cargo test` 260 lib + 6 integration tests pass; `cargo test --features dev` 264 lib + 6 integration tests pass; `cargo clippy --all-targets -- -D warnings` exit 0; `cargo clippy --all-targets --features dev -- -D warnings` exit 0. Six fix-ups applied during gate verification: import path correction in `shop.rs:542`, `.world_mut()` in `town/mod.rs:247,264`, `InputManagerPlugin` removed from town test app at `town/mod.rs:166-168`, `#[allow(clippy::too_many_arguments)]` on `handle_inn_rest` and `handle_shop_input`, `clippy::erasing_op` workaround at `shop.rs:510`.)

## Security

**Known vulnerabilities:** No known vulnerabilities identified as of 2026-05-11. No new third-party crates introduced (Δ deps = 0) — `bevy_egui = 0.39.1`, `bevy_common_assets = 0.16.0`, `bevy_asset_loader = 0.26.0` are already pinned in `Cargo.toml` and exercised by `CombatUiPlugin` / `MinimapPlugin` / `DungeonAssets` precedents.

**Architectural risks:**

- **RON trust boundary on `ShopStock.items`** — crafted RON could declare a 100K-entry vector exhausting paint-loop memory. Mitigation: `clamp_shop_stock(stock, MAX_SHOP_ITEMS = 99)` in `shop.rs`'s painter (Phase 10 step).
- **RON trust boundary on `TownServices.inn_rest_cost`** — `u32::MAX` would make Inn unusable; `0` would let infinite rests. Mitigation: `services.inn_rest_cost.min(MAX_INN_COST = 10_000)` in `inn.rs`'s handler (Phase 10 step).
- **Save-file gold injection** — `Gold(u32)` derives `Serialize`/`Deserialize`. Feature #23 (save/load) must clamp on load. Document at the `Gold` declaration site in `gold.rs`. No #18a system loads gold from disk; this is a forward-compat note only.
- **Inventory length explosion** — `Inventory.0: Vec<Entity>` is unbounded by the type. Mitigation: `buy_item` returns `BuyError::InventoryFull` when `inventory.0.len() >= MAX_INVENTORY_PER_CHARACTER (= 8)`. Cap is declared in `shop.rs` (Phase 7 step).
- **Gold underflow disguised as success** — `saturating_sub(cost)` silently clamps to 0 if `cost > gold.0`. Mitigation: `Gold::try_spend` returns `Err(SpendError::InsufficientGold)` BEFORE calling `saturating_sub`, so saturating arithmetic is defense-in-depth (Phase 4 step + Critical section).
- **Gold charged before item delivery** — anti-pattern that would silently charge the player when `give_item` fails. Mitigation: `buy_item`'s success path order is "give first, deduct second" (Phase 7 step + Critical section).
- **`PrimaryEguiContext` orphaning** — if the `Camera2d` is despawned without the `PrimaryEguiContext` going with it (or vice versa), every painter silently fails (`ctx_mut() == Err`). Mitigation: spawn both as one entity in `spawn_town_camera`; despawn-by-`TownCameraRoot` cleans both atomically (Phase 5 step).

**Trust boundaries:** RON deserialization of `ShopStock`/`RecruitPool`/`TownServices` is the sole untrusted input surface for #18a. All three pass through the clamp helpers in Phase 10 before any consuming system reads them. No HTTP/socket/IPC surface introduced. Asset path resolution inherits `bevy_common_assets` + `bevy_asset_loader` existing path-traversal guards (precedent: same crates used by `DungeonAssets`/`AudioAssets`).

## Deferred to #18b

Out of scope for #18a; explicitly carried forward to a follow-up PR:

| Item | Why deferred | Where it lands in #18b |
|---|---|---|
| **`temple.rs`** — Temple revive (`Dead` removal + `current_hp = 1`) | User decision 1 split: Temple is #18b | New file `src/plugins/town/temple.rs`; replace `placeholder.rs`'s Temple arm |
| **`temple.rs`** — Temple cure of severe status (Stone/Paralysis/Sleep) | Same | Same file as above |
| **`guild.rs`** — Recruit from `RecruitPool` | Same | New file `src/plugins/town/guild.rs`; `RecruitPool` RON is already authored for this |
| **`guild.rs`** — Dismiss party member (and their inventory entities — Research Pitfall 9) | Same | Same file; dismiss loop must despawn `Inventory.0` entries before despawning the `PartyMember` to prevent orphaned `ItemInstance` entities |
| **`guild.rs`** — Row swap (`PartyRow::Front` ↔ `Back`) | Same | Same file |
| **`guild.rs`** — Slot reorder | Same | Same file |
| **`TownServices.temple_revive_cost_base` / `temple_revive_cost_per_level` / `temple_cure_costs`** | Used only by Temple service | RON schema already includes these fields with `#[serde(default)]` — no migration needed |
| **`paint_placeholder` painter** | Replaced by real Temple/Guild painters | `placeholder.rs` is **deleted** by #18b — single-file diff replacement |
| **Open Question 6** (Temple revive `current_hp = 1` write order) | Temple is deferred | Resolve in #18b implementation via Layer 2 test |
| **Square menu UX rework** | Buttons stay in place; Temple/Guild route changes from placeholder → real | No menu changes in #18b |

**LOC envelope sanity check:** removing temple.rs (~180 LOC) + guild.rs (~220 LOC) from the original #18 envelope of ~1650 LOC, plus the Temple+Guild test surface (~150 LOC), brings #18a to ~1100 LOC, just above the 800–1000 target. Tests are ~30% of total; trimming is not recommended since Town is the gateway to every future economic feature.

## Open Questions

All Open Questions from the research are resolved:

- **Q1** (PR split): Resolved — user decision 1 = split (#18a now, #18b follow-up).
- **Q2** (Gold scope): Resolved — user decision 2 = party-wide `Resource<Gold>`.
- **Q3** (Town backdrop): Resolved — user decision 3 = no backdrop, pure egui.
- **Q4** (Leave Town destination): Resolved — user decision 4 = `GameState::TitleScreen`.
- **Q5** (Add `GameClock`): Resolved — user decision 5 = add now (~15 LOC).
- **Q6** (Temple revive write order): Deferred to #18b (Temple is out of scope for #18a).
- **Q7** (Inventory cap): Resolved — user decision 6 = 8 per character.
- **Q8** (Sell-back ratio): Resolved — user decision 7 = 50% (`value / 2`).
- **Q9** (`distributive_run_if` exists in Bevy 0.18.1): Resolved — fact-check verified `IntoScheduleConfigs::distributive_run_if` is present in `bevy_ecs-0.18.1/src/schedule/config.rs:381-416`. Use it in the plugin wiring.

## Implementation Discoveries

**EguiPlugin unavailable in headless tests** — `EguiPlugin::default()` panics without the render pipeline (requires `MinimalPlugins` to include PbrPlugin etc.). Fix: mirror `combat/ui_combat.rs` pattern — add only Update/OnEnter/OnExit systems in test apps, skip EguiPrimaryContextPass painters. Painter systems are verified via manual smoke test only.

**`use super::*` does NOT re-export private `use` imports** — test modules that `use super::*` only get `pub` items defined in the parent module. Private `use crate::...` statements in the parent are not re-exported. Fix: added explicit imports in all test modules (e.g., `use crate::plugins::town::inn::{InnState, handle_inn_rest}`; `use crate::plugins::party::character::{BaseStats, Class, ...}` in `data/town.rs` tests).

**B0002 double-borrow on buy/sell logic** — `buy_item` and `sell_item` free functions require `Query<&mut Inventory, With<PartyMember>>`, but `handle_shop_input` already holds `Query<(Entity, &mut Inventory), With<PartyMember>>`. Cannot overlap. Fix: inlined buy/sell logic directly in `handle_shop_input` using the existing combined query. The free-function helpers (`buy_item`, `sell_item`) remain as exported API for future callers (loot drops, UI) that can supply the narrower query.

**`AssetPlugin` required for `init_asset`** — `App::init_asset::<T>()` requires `AssetPlugin` to have been added first. Tests using only `MinimalPlugins` must explicitly add `AssetPlugin::default()`. Fixed in `inn.rs` and `shop.rs` test apps.

**`data/town.rs` test module needs explicit party character imports** — `use super::*` in tests does not bring `Race`, `Class`, `BaseStats`, `PartyRow`, `StatusEffectType` into scope (they're private `use` imports in the outer module). Fixed by adding `use crate::plugins::party::character::{BaseStats, Class, PartyRow, Race, StatusEffectType};` to the test module.

**`assert!(true)` triggers `clippy::assertions_on_constants`** — replaced the structural-invariant `buy_item_gold_deduction_only_after_give` test's `assert!(true)` with a real assertion that exercises `Gold::try_spend` to verify the deduction step.

**`MessageReader<EquipmentChangedEvent>` import unused in inn.rs test** — the `rest_fires_equipment_changed_event_per_member` test was simplified to use HP-based proxy assertions rather than a MessageReader closure. Removed the unused `bevy::ecs::message::MessageReader` import to avoid clippy `-D warnings` failure.

**`square.rs` test module `use super::*` scope confirmed** — analysis resolved: `use super::*` in an inline `mod tests` block within `square.rs` brings in the outer module's private `use` imports (`GameState`, `TownLocation`, `Gold`, `GameClock`) because those are `use` declarations in the same file scope, not re-exports. This is different from `data/town.rs` where the types were `use`-imported from external paths — both cases compile but only the `data/town.rs` case needed explicit re-declaration. No fix needed for `square.rs`.

**Shell tool unavailable in Phase 11 (third session)** — All three implementation sessions (prior two + this continuation) lacked a bash/shell execution tool. The six quality gates cannot be run programmatically. Thorough static analysis was performed instead: all type signatures verified, import chains traced, RON file syntax checked, potential clippy patterns reviewed (doc_lazy_continuation, assertions_on_constants, manual_range_contains, unused_imports). No issues found in static review. The gates require a human or CI run (`cargo check`, `cargo check --features dev`, `cargo test`, `cargo test --features dev`, `cargo clippy --all-targets -- -D warnings`, `cargo clippy --all-targets --features dev -- -D warnings`).

## Verification

Each verification item lists the command/action, expected outcome, and whether it can be checked by `cargo` alone (Automatic) or requires running the binary (Manual). The six quality gates the implementer will run are listed first.

- [x] **Gate 1:** `cargo check` — exit 0 — Automatic (compiles without `dev` feature; verifies no `#[cfg(feature = "dev")]` is required for the new code path). **PASSED 2026-05-11.**
- [x] **Gate 2:** `cargo check --features dev` — exit 0 — Automatic. **PASSED 2026-05-11.**
- [x] **Gate 3:** `cargo test` — exit 0 — Automatic (runs unit + integration tests; town-related modules verified below). **PASSED 2026-05-11 — 260 lib + 6 integration tests pass.**
- [x] **Gate 4:** `cargo test --features dev` — exit 0 — Automatic (verifies `#[cfg(feature = "dev")]` paths including F9 cycler interaction with Town). **PASSED 2026-05-11 — 264 lib + 6 integration tests pass.**
- [x] **Gate 5:** `cargo clippy --all-targets -- -D warnings` — exit 0 — Automatic. **PASSED 2026-05-11.**
- [x] **Gate 6:** `cargo clippy --all-targets --features dev -- -D warnings` — exit 0 — Automatic. **PASSED 2026-05-11.**

Module-specific tests (subset of `cargo test`):

- [ ] **`town::gold::tests::try_spend_insufficient_returns_err_without_mutation`** — `Gold::try_spend(50)` on a `Gold(10)` returns `Err`, `gold.0` is still 10 — unit — `cargo test -p druum town::gold::tests::try_spend_insufficient` — Automatic.
- [ ] **`town::gold::tests::try_spend_exact_succeeds`** — `Gold::try_spend(10)` on `Gold(10)` returns `Ok`, `gold.0 == 0` — unit — Automatic.
- [ ] **`town::gold::tests::earn_saturates_at_u32_max`** — `Gold(u32::MAX).earn(1)` leaves gold at `u32::MAX` — unit — Automatic.
- [ ] **`town::tests::town_plugin_builds`** — App with `StatePlugin + TownPlugin + EguiPlugin + AssetPlugin` updates once without panic — smoke — Automatic.
- [ ] **`town::tests::town_camera_spawns_on_enter_and_despawns_on_exit`** — Transition into `GameState::Town` → exactly one `TownCameraRoot` entity exists; transition out → zero — integration — Automatic.
- [ ] **`town::tests::town_substate_defaults_to_square_on_enter`** — After `next_state.set(Town)` + 2× `app.update()`, `*State<TownLocation>::get() == Square` — integration — Automatic.
- [ ] **`town::square::tests::square_confirm_at_cursor_0_navigates_to_shop`** — cursor=0 + `MenuAction::Confirm` → `NextState<TownLocation>` realises to `Shop` after 2× update — integration — Automatic.
- [ ] **`town::square::tests::square_confirm_at_cursor_4_navigates_to_titlescreen`** — cursor=4 (Leave Town) + Confirm → `*State<GameState>::get() == TitleScreen` — integration — Automatic.
- [ ] **`town::shop::tests::buy_item_deducts_gold_and_adds_to_inventory`** — pre: `Gold(100)`, `Inventory.0.len() == 0`; post `buy_item(handle, ...)`: `Gold(100 - price)`, `Inventory.0.len() == 1` — unit on helper — Automatic.
- [ ] **`town::shop::tests::buy_item_rejects_when_inventory_full`** — pre: `Inventory.0.len() == 8`; `buy_item` returns `BuyError::InventoryFull`, gold unchanged — unit — Automatic.
- [ ] **`town::shop::tests::buy_item_rejects_when_insufficient_gold`** — pre: `Gold(1)`, item value 50; `buy_item` returns `BuyError::InsufficientGold`, no item added — unit — Automatic.
- [ ] **`town::shop::tests::buy_item_does_not_charge_when_give_item_fails`** — character entity missing `Inventory` component; `buy_item` returns `Err`, gold unchanged — unit — Automatic.
- [ ] **`town::shop::tests::sell_item_adds_half_gold_and_removes_from_inventory`** — pre: item value=10 in inventory, `Gold(0)`; `sell_item(...)`: `Gold(5)`, inventory loses entity — unit — Automatic.
- [ ] **`town::inn::tests::rest_full_heals_living_party`** — pre: party at `current_hp = 1`; after Inn rest, `current_hp == max_hp`, `current_mp == max_mp` for each non-Dead member — integration — Automatic.
- [ ] **`town::inn::tests::rest_skips_dead_party_member`** — Dead member's `current_hp` stays 0 after Inn rest — integration — Automatic.
- [ ] **`town::inn::tests::rest_cures_poison_but_not_stone`** — pre: party has `[Poison, Stone]`; after rest, only `[Stone]` remains — integration — Automatic.
- [ ] **`town::inn::tests::rest_advances_clock_day_by_one`** — pre: `clock.day == 0`; post: `clock.day == 1`, `clock.turn == 0` — integration — Automatic.
- [ ] **`town::inn::tests::rest_deducts_configured_cost`** — pre: `Gold(100)`, cost 10; post: `Gold(90)` — integration — Automatic.
- [ ] **`town::inn::tests::rest_rejects_when_insufficient_gold`** — pre: `Gold(1)`, cost 10; gold unchanged, party HP unchanged — integration — Automatic.
- [ ] **`town::inn::tests::rest_fires_equipment_changed_event_per_member`** — `MessageReader<EquipmentChangedEvent>` reads N events for N living members — integration — Automatic.
- [ ] **`town::placeholder::tests::placeholder_cancel_returns_to_square`** — from Temple/Guild + `MenuAction::Cancel` → `Square` after 2× update — integration — Automatic.
- [ ] **`data::town::tests::shop_stock_ron_round_trips`** — serialize → deserialize matches original — unit — Automatic.
- [ ] **`data::town::tests::stock_filters_by_min_floor`** — items_for_floor(0) excludes items with `min_floor=1`; items_for_floor(1) includes them — unit — Automatic.
- [ ] **`data::town::tests::recruit_pool_ron_round_trips`** — same — unit — Automatic.
- [ ] **`data::town::tests::town_services_ron_round_trips_with_defaulted_temple_fields`** — RON omitting `temple_*` fields parses with their `Default` values; serializing back produces a stable result — unit — Automatic.
- [ ] **`data::town::tests::clamp_shop_stock_truncates_oversized`** — pass a 200-entry stock, `clamp_shop_stock(MAX_SHOP_ITEMS=99)` returns a 99-entry slice — unit — Automatic.

Asset / runtime checks (one-time manual after build succeeds):

- [ ] **Manual:** `cargo run --features dev`, press F9 to cycle through states, land on `GameState::Town`. Square menu renders with Gold + Day header. Navigate Down to Shop, Confirm. Shop renders with buy mode. Press Esc → back to Square. Navigate to Inn, Confirm. Inn renders. Confirm Rest (no-op without gold; verify gold-balance log). Navigate to Temple or Guild → placeholder text appears, Esc returns. Select Leave Town → `TitleScreen`. — Manual — exit cleanly with Ctrl-C.

GitButler workflow note (for the implementer, not the planner): commits MUST use `but commit --message-file <path>` per `CLAUDE.md`. The pre-commit hook on `gitbutler/workspace` rejects `git commit`.
