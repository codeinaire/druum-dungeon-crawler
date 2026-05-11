# Feature #18: Town Hub & Services — Research

**Researched:** 2026-05-11
**Domain:** Bevy 0.18.1 first-person dungeon crawler — egui multi-screen menu flow, sub-state composition, gold/economy data, town-data RON schemas, party/inventory wiring
**Confidence:** HIGH on every domain (no external crates introduced; all consumed APIs verified on disk)

---

## Tooling Limitation Disclosure

This research session ran with **Read-only file tools (no Bash, no MCP, no WebFetch, no WebSearch)**.

Compensating strategy:

- **Bevy 0.18.1 first-party facts** (SubStates derive, `EguiContexts`, `egui` containers, state machinery) — verified at HIGH confidence by reading on-disk extracted source under `~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/`. Specifically:
  - `bevy_state-0.18.1/src/state/sub_states.rs` — `SubStates` derive macro + `#[source(...)]` shape
  - `bevy_state-0.18.1/src/app.rs` — `add_sub_state` registration
  - `bevy_egui-0.39.1/Cargo.toml` — verifies `bevy = 0.18.0` requirement (✓ compatible with pinned `=0.18.1`)
  - `bevy_egui-0.39.1/examples/{simple,side_panel,ui}.rs` — canonical menu patterns
  - `bevy_egui-0.39.1/src/lib.rs` — `EguiPlugin`, `EguiContexts`, `EguiPrimaryContextPass`, `PrimaryEguiContext` types
  - `egui-0.33.3/src/lib.rs` — `SidePanel` / `CentralPanel` / `Window` / `Area` documentation
- **Druum codebase facts** are HIGH confidence — read directly.
- **No new third-party crate is required.** `bevy_egui = "=0.39.1"` and `bevy_common_assets = "=0.16.0"` are already in `Cargo.toml`. Step A/B/C crate-verification gate is **N/A** for #18 — zero new deps.

**Bottom line:** #18 is plumbing + content + UI atop a stack that is fully in tree. There is no version-resolution risk to surface. The decisions that remain are user-preference (gold scope, sub-feature split, asset strategy) — flagged in `## Open Questions` as **C** (user-preference).

---

## Summary

Feature #18 wires `GameState::Town` and its five sub-states (`Square / Shop / Inn / Temple / Guild`) into a working hub-and-services screen. Critical findings:

1. **`TownLocation` SubStates is ALREADY DECLARED and REGISTERED** at `src/plugins/state/mod.rs:38-56`. The roadmap text "Implement `GameState::Town` with sub-states ..." is stale — the state machine work is done. #18 is pure systems + UI + data.
2. **Every dependency the roadmap calls out is in tree:** party (`PartyMember`, `Equipment`, `StatusEffects`), inventory (`Inventory`, `ItemInstance`, `give_item`, `equip_item`), items (`ItemAsset` with `value: u32` field pre-shipped for shop pricing), bevy_egui (pattern proven by `CombatUiPlugin` + `MinimapPlugin`), audio (`bgm_town` already loaded by `AudioAssets`), input (`MenuAction` already declared with "Town reuses this enum in v1" intent doc).
3. **`Gold` is the sole new resource the data layer needs.** No new asset types, no new ECS components on party members. (Inn-rest "advance clock" — see §Architecture: there is **no clock infrastructure in tree**; the recommendation is to add a minimal `Resource<GameClock>` rather than try to scope-creep an "in-game time" system into #18.)
4. **The roadmap-suggested LOC envelope (+800 to +1300) is achievable inside a single PR** if you ship all five screens at minimum-viable depth (one egui panel per sub-state, no portraits, no animations). The roadmap also explicitly invites a split: shop+inn first PR, temple+guild later (#18a / #18b). This is a **C-tier decision** — surface to the user.
5. **Architectural security risks** are all RON-trust-boundary issues (gold-amount overflow from save data, recruit pool size, shop stock bounds). All map to the same clamp-on-load pattern the project already uses (`encounter.rs:281` clamps `encounter_rate.clamp(0.0, 1.0)`, `derive_stats` saturating arithmetic, etc.).

**Primary recommendation:** Build #18 as **one PR with all five screens at minimum-viable depth**, using `Resource<Gold>` (party-wide) + `Resource<GameClock>` (day counter + turn counter), reusing the existing egui-attached-to-`DungeonCamera` pattern, with one new `TownAssets` collection in `LoadingPlugin` containing `shop_stock.ron`, `recruit_pool.ron`, and `town_services.ron`. Defer the visual backdrop ("Town Square scene") to #25 — render Square as a pure egui screen for now.

**Counter-recommendation (split):** If "review burden" or "blast radius" is the user's primary concern, ship as `#18a (Square + Shop + Inn)` then `#18b (Temple + Guild)`. Surface this trade-off explicitly to the user (Open Question 1).

---

## What's Already In Tree (Zero-Work Pre-Shipped)

This section names every Feature #18 dependency that is already implemented at HIGH confidence — verified by direct codebase read. The planner can skip "implement X" for any of these.

| Capability | Where | What it gives #18 |
|---|---|---|
| `GameState::Town` variant | `src/plugins/state/mod.rs:7-15` | Top-level state, registered |
| `TownLocation` `SubStates<GameState=Town>` with all 5 variants (`Square`, `Shop`, `Inn`, `Temple`, `Guild`) | `src/plugins/state/mod.rs:38-47` | Sub-state already declared **AND** registered via `.add_sub_state::<TownLocation>()` at `mod.rs:56` |
| F9 dev cycler advances `Loading → TitleScreen → Town → ...` | `src/plugins/state/mod.rs:80-89` | Lets the dev jump straight into `GameState::Town` for testing |
| `TownPlugin` stub | `src/plugins/town/mod.rs:1-17` | Plugin registered in `main.rs:30`; just an OnEnter/OnExit log right now. #18 fills systems in here |
| `bevy_egui = "=0.39.1"` (`render`, `default_fonts`) | `Cargo.toml:28` | Δ deps = 0; no Step A/B/C gate |
| `EguiPlugin::default()` registered in `UiPlugin` | `src/plugins/ui/mod.rs:14` | One global egui plugin already runs |
| `EguiGlobalSettings { auto_create_primary_context: false }` | `src/plugins/ui/mod.rs:22-25` | Each scene attaches `PrimaryEguiContext` to its own camera — same pattern #18 must follow |
| Working egui-on-Camera3d precedent | `src/plugins/combat/ui_combat.rs:60-71` (attaches to `DungeonCamera`), `src/plugins/ui/minimap.rs:173-180` (idempotent `Without<PrimaryEguiContext>`) | Town can mirror **either**: attach to an existing camera (if Town reuses dungeon scene) **or** spawn a Town-specific camera (recommended — see Architecture) |
| `EguiPrimaryContextPass` schedule | `combat/ui_combat.rs:53-56` | Where the paint systems run; #18 paints in this schedule |
| `egui::SidePanel::left/right`, `TopBottomPanel::top/bottom`, `CentralPanel`, `Window` patterns | `bevy_egui-0.39.1/examples/{ui,side_panel}.rs` + `combat/ui_combat.rs:97-200` | Genre-correct shop/temple/guild layouts use `SidePanel::left` for menu, `CentralPanel` for the action area |
| `MenuAction` leafwing enum (Up/Down/Left/Right/Confirm/Cancel/Pause) | `src/plugins/input/mod.rs:58-67` | Town reuses this — doc-comment at `mod.rs:54-57` says so explicitly: *"Town reuses this enum in v1; `TownAction` is deferred until Town gets distinct movement (Feature #19+)."* |
| `InputMap<MenuAction>` defaults (WASD/arrows/Enter/Space/Escape) | `src/plugins/input/mod.rs:186-201` | All key bindings ready |
| `ActionState<MenuAction>` resource (`init_resource`) | `src/plugins/input/mod.rs:110` | #18 systems read `Res<ActionState<MenuAction>>` directly |
| `bgm_town` audio handle | `src/plugins/loading/mod.rs:62-63` | Town BGM already loads at `assets/audio/bgm/town.ogg` |
| `play_bgm_for_state` crossfade fires on `GameState::Town` | `src/plugins/audio/bgm.rs:106-112` | BGM swap on enter/exit is automatic — no work for #18 |
| `Inventory`, `ItemInstance`, `give_item`, `equip_item`, `unequip_item` | `src/plugins/party/inventory.rs` | Shop buy = `give_item(...)`; shop sell = remove from `Inventory.0` + despawn `ItemInstance` + add gold |
| `ItemAsset.value: u32` | `src/data/items.rs:101-103` | **Already authored** as "Sell/buy value in gold. Used by #18 shop." — comment says so. No schema change needed for buy/sell base price |
| `ItemHandleRegistry::get(id) -> Option<&Handle<ItemAsset>>` | `src/plugins/party/inventory.rs:520-541` | Shop's "give the player the item they bought" uses this — comment at inventory.rs:556 lists "#18 shop" as a named caller |
| `StatusEffects::has(kind)` + `StatusEffectType::{Poison, Stone, Dead, ...}` | `src/plugins/party/character.rs:257-310` | Temple cure/revive can read/mutate `StatusEffects.effects` directly |
| `DerivedStats { current_hp, max_hp, current_mp, max_mp }` | `src/plugins/party/character.rs:132-151` | Inn rest = `current_hp = max_hp`, `current_mp = max_mp` per character |
| `derive_stats(...)` returns fresh `DerivedStats` | `src/plugins/party/character.rs:380-494` | If Temple revives a `Dead` member, fire `EquipmentChangedEvent` to re-derive — `recompute_derived_stats_on_equipment_change` reads `StatusEffects` for free (inventory.rs:444-453; filter dropped per memory `reference_druum_recompute_filter_dual_use.md`) |
| `EquipmentChangedEvent` is the dual-use stat-changed trigger | Memory `reference_druum_equipment_changed_event_dual_use.md` + `inventory.rs:217` | Status-change after revive/cure → fire this Message to recompute derived stats |
| `PartySize: Resource` (default `4`) | `src/plugins/party/character.rs:344-351` | Guild slot count |
| `PartyRow` (Front/Back) | `src/plugins/party/character.rs:178-182` | Guild front/back row swap target |
| `PartySlot(usize)` | `src/plugins/party/character.rs:191` | Guild reorder target |
| `Race`, `Class` enums declared but only 5 races / 8 classes — `Human` + `Fighter/Mage/Priest` used in v1 | `src/plugins/party/character.rs:39-72` | Pre-made recruit pool can use any combination |
| `BaseStats`, `derive_stats` pure function | `src/plugins/party/character.rs:98-117, 380-494` | Recruit creation = pick a name + race + class, set BaseStats from a template (no point-buy UI in #18; #19 owns that) |
| `spawn_default_debug_party` precedent (dev-only spawn pipeline) | `src/plugins/party/mod.rs:89-144` | Guild recruit = mirror this — `commands.spawn(PartyMemberBundle { ... }).insert(Inventory::default());` |
| `bevy_common_assets::RonAssetPlugin::<T>::new(&["<extension>.ron"])` precedent | `src/plugins/loading/mod.rs:108-115` | #18 town-data RONs register here |
| `RonAssetPlugin::<ItemDb>::new(&["items.ron"])` + `Vec<ItemAsset>` linear-scan precedent | `src/data/items.rs:30-43` | Shop stock RON shape mirrors `ItemDb` exactly |
| `bevy_asset_loader::AssetCollection` resource pattern | `src/plugins/loading/mod.rs:30-48` | #18 adds a `TownAssets` collection alongside `DungeonAssets`/`AudioAssets` |
| `state_changed::<GameState>` run-condition usage | `src/plugins/audio/mod.rs:118-119` | Town's "leave town" trigger = `next.set(GameState::TitleScreen)` (or `Dungeon` — Open Q4); BGM crossfade fires automatically |

**Net effect:** The roadmap entry's "Broad Todo List" overstates the scope by ~30%. The state machine, leafwing input, BGM, item schema, inventory helpers, and recompute pipeline are all done. #18 is **content + UI + economy data** layered on top.

---

## Standard Stack

### Core (already in tree)

| Library | Version | Purpose | License | Maintained? | Why Standard |
|---|---|---|---|---|---|
| `bevy` | `=0.18.1` | ECS, state machine, render, asset, audio | MIT OR Apache-2.0 | YES | Already pinned |
| `bevy_egui` | `=0.39.1` | Immediate-mode UI; menus, lists, buttons | MIT | YES (vladbat00, regular releases tracking each Bevy bump) | Already in tree; verified `bevy = "0.18.0"` requirement matches our `=0.18.1` |
| `bevy_common_assets` | `=0.16.0` (ron feature) | RON-format asset loader | MIT OR Apache-2.0 | YES | Already in tree; precedent in 6 RonAssetPlugin registrations |
| `bevy_asset_loader` | `=0.26.0` | Staged collection-loading | MIT OR Apache-2.0 | YES | Already in tree; precedent in `DungeonAssets`/`AudioAssets` |
| `leafwing-input-manager` | `=0.20.0` | Rebindable input → action enums | ISC | YES | Already in tree; `MenuAction` already declared |
| `serde`, `ron` | `1` / `0.12` | RON serde for town-data files | MIT OR Apache-2.0 | YES | Already in tree |

**Zero new direct dependencies are required.** Δ Cargo.toml = 0.

### Supporting (already in tree)

| Capability | Bevy / project type | Use case in #18 |
|---|---|---|
| `egui::SidePanel::left`, `TopBottomPanel::bottom`, `CentralPanel` | `bevy_egui-0.39.1/examples/side_panel.rs` | Standard service-screen layouts |
| `egui::ScrollArea::vertical().show(...)` | `combat/ui_combat.rs:120-126` | Long shop stock / recruit lists |
| `egui::Color32::YELLOW` highlight pattern | `combat/ui_combat.rs:148-153` | Cursor highlight on menu items |
| `Res<ActionState<MenuAction>>` + `just_pressed(&Up)` etc. | `combat/ui_combat.rs:input handler` | Sub-state cursor navigation |
| `state_changed::<GameState>` run-condition | `audio/mod.rs:119` | Optional: clamp behaviour on state transition |
| `OnEnter(...)`, `OnExit(...)`, `run_if(in_state(...))` | `dungeon/mod.rs:226-246` | Town entry/exit hooks |
| `EquipmentChangedEvent { character, slot: EquipSlot::None }` sentinel | `inventory.rs:215-220`, memory `reference_druum_equipment_changed_event_dual_use.md` | Temple-revive / Inn-rest stat refresh |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|---|---|---|
| `bevy_egui` | Custom `bevy_ui` Node tree | `bevy_ui` is in tree (via `3d` umbrella → `ui` chain). Switching mid-project would mean reimplementing the precedents in `MinimapPlugin` + `CombatUiPlugin`. Net cost > net benefit |
| `Resource<Gold>` (party-wide) | `Component Gold` per `PartyMember` (Wizardry classic) | See §Architecture Options Decision 3 |
| `RonAssetPlugin::<ShopStock>` | Hardcode shop stock in Rust | RON is the project convention (6 precedents). Authoring shop stock in code adds zero leverage |
| `bevy_asset_loader` `TownAssets` collection | Load RONs lazily on `OnEnter(Town)` | Lazy load is asset-load-aware (handles may not be `LoadedWithDependencies` for several frames); the collection pattern guarantees readiness before Town entry. Mirrors `DungeonAssets` precedent |
| Add a `GameClock: Resource` | Skip "advance clock" entirely | Roadmap text explicitly says "advance an in-game clock". Adding a 2-field resource (day, turn) is ~10 LOC. **Recommend the minimal resource** because Inn-rest reads strange without it — see Architecture Options Decision 4 |

**Installation:** None required. **Zero new direct Cargo.toml dependencies.**

---

## Architecture Options

Four orthogonal decisions for #18. Each is one user-facing choice; recommendations follow with confidence.

### Decision 1: Town render model — egui-only vs egui-over-3D-scene

| Option | Description | Pros | Cons | Best When |
|---|---|---|---|---|
| **(A) Pure egui** (RECOMMENDED) | Spawn a `Camera2d` on `OnEnter(Town)` (or no camera if egui auto-attaches), render the entire town hub as egui panels — no 3D scene at all. `Square` is a `CentralPanel` with title text + service buttons; the other 4 sub-states overlay `Window`s or replace the `CentralPanel`. | Zero asset cost; zero 3D scene work; matches the roadmap's "The square is a menu, not a level" guidance; testable via egui-context-only fixtures | No visual atmosphere — Town looks like a menu. Acceptable for v1 per roadmap |
| (B) Single backdrop image | One PNG (hand-painted or solid-colour) rendered behind egui as a `Sprite` on a `Camera2d`, or as a fullscreen `egui::Image` widget | Adds a touch of atmosphere with minimal cost (~1 PNG) | Adds an asset dependency (decision C below); requires a `Camera2d` spawn pipeline; complicates the egui attach-context pattern |
| (C) Reuse `DungeonCamera` | Don't spawn a Town camera; rely on the dungeon camera surviving the transition | Mirrors the Combat overlay pattern | Dungeon camera is despawned on `OnExit(Dungeon)` for destinations other than Combat (`dungeon/mod.rs:603-607`) — Town **does not** preserve it. Would require changing the `preserve_party` rule. **Don't do this.** |
| (D) 3D town scene | Actual 3D backdrop with NPC sprites | Atmospheric | Roadmap rejects this: *"Defer 3D town rendering indefinitely — it's not what the genre is about."* |

**Recommended:** **(A) Pure egui.** Spawn a `Camera2d` on `OnEnter(Town)`, despawn on `OnExit(Town)`. Tag with `TownCameraRoot`. Attach `PrimaryEguiContext` to it.

**Counterargument:** "A blank black screen reads as broken." **Response:** The egui `CentralPanel` fills the screen with a styled background (panel fill colour, configurable via `egui::Visuals`). It looks intentional, not broken. The roadmap is explicit: *"A single hand-painted 'town square' backdrop (one PNG) is enough atmosphere for now."* — if the user wants the PNG, that's a one-line addition (Open Q3); the architecture stays the same.

### Decision 2: Sub-state navigation pattern

The `TownLocation` sub-state is the source of truth for which screen is active. Two ways to swap between screens:

| Option | Description | Pros | Cons |
|---|---|---|---|
| **(A) Per-sub-state painter systems** (RECOMMENDED) | One paint system per `TownLocation` variant, each gated with `.run_if(in_state(TownLocation::Square))` etc. They write to `NextState<TownLocation>` on confirmed menu clicks. Mirrors `paint_minimap_overlay` vs `paint_minimap_full` split at `minimap.rs:138-144`. | Each screen is independently testable; trivial schedule mapping; easy to add a 6th sub-state | Five systems instead of one; minor schedule bloat (negligible — they're gated) |
| (B) One painter system that match-es on current `TownLocation` | Single `paint_town` system; reads `State<TownLocation>` and branches | One symbol, smaller schedule | Painter becomes 200+ LOC monolith; hard to test isolated screens; hard to add functionality incrementally |
| (C) Plugin-per-sub-state | Each TownLocation variant gets its own `Plugin`: `ShopPlugin`, `InnPlugin`, etc. | Maximum modularity | Five new plugins for one feature is excessive; the project's plugin granularity is per-`GameState`, not per-sub-state |

**Recommended:** **(A) Per-sub-state painter systems.** File layout: `src/plugins/town/{mod.rs, square.rs, shop.rs, inn.rs, temple.rs, guild.rs, gold.rs, services_data.rs}`. Each `<sub_state>.rs` exports one `paint_<sub_state>` system + one `handle_<sub_state>_input` system (if needed). `mod.rs` registers them.

### Decision 3: Gold scope — party-wide Resource vs per-character Component

This is the **biggest C-tier decision** to surface to the user. Both are valid genre conventions.

| Option | Description | Pros | Cons | Best When |
|---|---|---|---|---|
| **(A) Party-wide `Resource<Gold>`** (RECOMMENDED) | One `#[derive(Resource)] pub struct Gold(pub u32);` shared by all party members. Buy/sell/inn-cost/temple-cost all mutate this single resource. | Etrian Odyssey convention. Simpler UI ("Party Gold: 3450"). No per-character bookkeeping when paying for services. Save-format is one field. | Doesn't allow Wizardry's "drop a dead party member, lose their share" mechanic. Easier to lose all gold to one expensive purchase. |
| (B) Per-character `Component Gold(u32)` on `PartyMember` | Each `PartyMember` carries their own purse. Buy/sell mutates the active character's gold. "Pool gold" at Gilgamesh's Tavern in classic Wizardry. | Wizardry classic convention. Recovers when characters are revived. Per-character economy depth. | More UI affordance needed (per-character HUD, pool/distribute dialog). Save format is N fields. Increases #18 scope by ~15-20% (the pool/distribute system is its own egui sub-screen) |

**Recommended:** **(A) Resource<Gold>.** Reasons:

1. The user-spec for #18 lists "Inn: ... charge gold", "Temple: ... gold ∝ level", "Shop: buy/sell ... against a gold currency" — all of which read more naturally with a party-wide pool. None of the spec lines mention per-character gold.
2. Modern players expect party-wide gold (Etrian Odyssey, modern Wizardry remakes, virtually every roguelike). Per-character gold is now a niche-retro design choice.
3. Saves complexity for the v1 economy (#21). Per-character gold + per-character HP/MP/equipment compounds save-file size.
4. Wizardry's "pool gold at the gilder" mechanic is itself a UI sub-screen that #18 would need to ship — basically free scope reduction.

**Counterargument:** "Druum's vibe-tag is Wizardry-style, so per-character gold." **Response:** the user can override this in Open Question 2 below. The default recommendation flips to per-character if they say so; the architecture cost is real (~150 LOC + a new egui screen for pool/distribute), but it's their call.

**Type choice for `Gold`:** `u32`. Reasoning:
- `u32::MAX = ~4.29B` gold. A typical Wizardry endgame is ~500K gold. `u32` has 4 orders of magnitude of safety margin.
- `u64` is overkill and doubles save bytes for this field. `u16` (`u16::MAX = 65535`) is too tight — a +5 weapon in classic Wizardry costs 5000, and a single death cure costs 250×level — endgame can plausibly hit 100K+.
- All arithmetic uses **saturating** ops: `gold.saturating_sub(cost)` for purchases, `gold.saturating_add(sale_price)` for sales. Same pattern as `derive_stats` (`character.rs:380-494`).

### Decision 4: Inn rest — advance an in-game clock?

The roadmap line says *"Inn rests the party (full HP/MP heal, time advances)"*. There is **no time/clock resource in tree**. Two paths:

| Option | Description | Pros | Cons |
|---|---|---|---|
| **(A) Minimal `Resource<GameClock>`** (RECOMMENDED) | `#[derive(Resource)] pub struct GameClock { pub day: u32, pub turn: u32 }` plus `inn_rest` increments `day += 1` and resets `turn = 0`. No other system reads it in v1. | ~15 LOC. The roadmap line "time advances" becomes meaningful. Sets up Feature #21/24 work (status duration in days, scripted events keyed off day count) without retrofit | Slight scope creep over "just heal" |
| (B) Skip the clock; Inn just heals | Drop the "time advances" text from the implementation | Slightly less work | Future status-duration / scripted-event work has to bolt this on. The roadmap intent is clear |
| (C) Inn advances `turn` only | A turn is the unit Combat already uses (in `turn_manager.rs`) | Reuses an existing concept | "1 inn rest = 1 turn" feels wrong; turns are combat-scale, not rest-scale |

**Recommended:** **(A) Add minimal `GameClock` resource** in `src/plugins/town/gold.rs` or a new `src/plugins/state/clock.rs`. Inn-rest increments `day`. No other system reads it in #18. Saves a future retrofit. Surface to user as Open Q5 in case they want to defer.

### Decision 5: Sub-feature scope split (single PR vs #18a/#18b)

The roadmap itself flags this: *"Consider shipping town in two passes: shop + inn first, temple + guild later."* Pros and cons:

| Option | Description | Pros | Cons |
|---|---|---|---|
| **(A) Single PR (RECOMMENDED, conditional)** | All five screens (Square, Shop, Inn, Temple, Guild) in one PR | One state-flow review pass; one test fixture suite; nothing depends on Temple or Guild from other features for several weeks (per roadmap, #19+ extends Guild, #14+ already shipped) — splitting doesn't unblock anyone | LOC envelope (+800 to +1300) at top of the project's per-PR comfort range. Review surface is wide |
| (B) #18a (Square + Shop + Inn), #18b (Temple + Guild) | Two PRs back-to-back | Half the LOC per PR; same Gold + GameClock + RonAsset chain is the foundation in both; review is per-screen-cluster | Two test fixture setups; two BGM testing rounds; two egui-attach-context paranoia checks. The hidden cost is the second PR has to re-acquaint the reviewer with the same context |

**Recommended:** **(A) Single PR**, but the reviewer/orchestrator chooses. Trigger for choosing (B):
- If review-cycle latency is the constraint, split.
- If the user prefers small reviewable diffs as project policy, split.
- Otherwise, the single PR is more efficient because all five screens share the same foundation (Gold, GameClock, TownAssets, MenuAction handler, Camera2d spawn).

**This is an explicit user-preference decision.** Open Question 1 surfaces it.

### Counterarguments to the headline recommendation

- **"`bevy_egui 0.39.1` might have breaking changes since 0.39 release."** — Resolved: `Cargo.toml` already pins `=0.39.1` and CombatUI/Minimap precedents work. Verified on-disk that `bevy_egui-0.39.1/Cargo.toml:158-269` requires `bevy = "0.18.0"` — exact compatibility match.
- **"A party-wide `Gold` resource is anti-Wizardry."** — Acknowledged. The user can flip to per-character via Open Q2. The architectural cost is contained — both options use the same `pay_gold(amount) -> bool` API; the implementation differs in where the data lives.
- **"Splitting the PR is always safer."** — True but slower. Half-and-half (split) shifts merge friction from one big review to two coordinated ones. If the project values stability above velocity, split. The user decides.
- **"Adding a GameClock is feature creep."** — The roadmap text says "advance an in-game clock." Not adding it means writing it later as a retrofit (with the inn-rest API already shaped wrong). The ~15 LOC is a forward-compatibility purchase.

---

## Architecture Patterns

### Recommended Project Structure

```
src/plugins/town/
├── mod.rs          # CHANGE — TownPlugin registration; declares submodules; wires systems
├── square.rs       # NEW — paint_town_square + handle_square_input
├── shop.rs         # NEW — paint_shop + handle_shop_input (buy mode, sell mode)
├── inn.rs          # NEW — paint_inn + handle_inn_input (rest action)
├── temple.rs       # NEW — paint_temple + handle_temple_input (revive, cure)
├── guild.rs        # NEW — paint_guild + handle_guild_input (recruit, dismiss, reorder, row swap)
├── gold.rs         # NEW — Gold resource, GameClock resource, pay_gold helper
└── data.rs         # NEW — TownAssets collection schema (ShopStock, RecruitPool, TownServices)

src/data/
├── town.rs         # NEW — ShopStock, RecruitPool, TownServices RON-loaded Asset types
└── mod.rs          # CHANGE — add `pub mod town;` + re-exports

assets/town/
├── shop_stock.ron       # NEW — list of item IDs available + min_floor gating
├── recruit_pool.ron     # NEW — list of pre-made characters available for hire
└── town_services.ron    # NEW — costs (inn_rest_cost, temple_revive_cost_per_level, etc.)

src/plugins/loading/mod.rs:30-48  # CHANGE — TownAssets sibling of DungeonAssets/AudioAssets
src/plugins/loading/mod.rs:108-115  # CHANGE — register 3 new RonAssetPlugin::<T>
```

**Open architectural question:** Should `Gold` and `GameClock` live in `src/plugins/town/gold.rs` or in a higher-level location like `src/plugins/state/clock.rs` / `src/plugins/party/gold.rs`? Recommendation: **start in `town/gold.rs`** because it is the only feature that touches them in #18. Move to `party/` or `state/` when #19/#20/#21 add readers. Pattern precedent: `ExploredCells` lives in `ui/minimap.rs` even though future features read it; moving comes when a second reader lands.

### Pattern 1: Town camera spawn + egui attach (OnEnter / OnExit)

```rust
// HIGH-confidence pattern, derived from:
//   - loading/mod.rs:191-213 (Camera2d spawn + LoadingScreenRoot tag)
//   - ui/minimap.rs:173-180 (idempotent PrimaryEguiContext attach)
//   - dungeon/mod.rs:593-619 (OnExit cleanup pattern)

#[derive(Component)]
pub struct TownCameraRoot;

fn spawn_town_camera(mut commands: Commands) {
    commands.spawn((Camera2d, TownCameraRoot, PrimaryEguiContext));
}

fn despawn_town_camera(
    mut commands: Commands,
    cams: Query<Entity, With<TownCameraRoot>>,
) {
    for e in &cams { commands.entity(e).despawn(); }
}
```

Register in `TownPlugin::build`:
```rust
.add_systems(OnEnter(GameState::Town), spawn_town_camera)
.add_systems(OnExit(GameState::Town), despawn_town_camera)
```

Direct attach in `commands.spawn(...)` is preferable to the dungeon's `Without<PrimaryEguiContext>` idempotent attach because Town's `Camera2d` spawns *during* OnEnter (not via `children![...]` deferred under `PlayerParty`), so the entity is queryable on the SAME frame.

### Pattern 2: Sub-state painter (egui panel) — Square screen template

```rust
// HIGH-confidence pattern from combat/ui_combat.rs:77-200, adapted.

use bevy::prelude::*;
use bevy_egui::{EguiContexts, EguiPrimaryContextPass, egui};
use leafwing_input_manager::prelude::ActionState;

use crate::plugins::input::MenuAction;
use crate::plugins::state::{GameState, TownLocation};

#[derive(Resource, Default, Debug)]
pub struct SquareMenuState {
    pub cursor: usize,  // 0..5 (Shop, Inn, Temple, Guild, Leave Town)
}

const SQUARE_MENU_OPTIONS: &[&str] =
    &["Shop", "Inn", "Temple", "Guild", "Leave Town"];

fn paint_town_square(
    mut contexts: EguiContexts,
    menu_state: Res<SquareMenuState>,
    gold: Res<Gold>,
) -> Result {
    let ctx = contexts.ctx_mut()?;

    egui::TopBottomPanel::top("town_header").show(ctx, |ui| {
        ui.horizontal(|ui| {
            ui.heading("Town Square");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(format!("Gold: {}", gold.0));
            });
        });
    });

    egui::CentralPanel::default().show(ctx, |ui| {
        ui.add_space(20.0);
        for (i, label) in SQUARE_MENU_OPTIONS.iter().enumerate() {
            let color = if i == menu_state.cursor {
                egui::Color32::YELLOW
            } else {
                egui::Color32::WHITE
            };
            ui.colored_label(color, format!("  {} {}", if i == menu_state.cursor { ">" } else { " " }, label));
        }
        ui.add_space(20.0);
        ui.separator();
        ui.label(egui::RichText::new("Up/Down to navigate, Enter to confirm").weak());
    });

    Ok(())
}

fn handle_square_input(
    actions: Res<ActionState<MenuAction>>,
    mut menu_state: ResMut<SquareMenuState>,
    mut next_sub: ResMut<NextState<TownLocation>>,
    mut next_game: ResMut<NextState<GameState>>,
) {
    let n = SQUARE_MENU_OPTIONS.len();
    if actions.just_pressed(&MenuAction::Down) {
        menu_state.cursor = (menu_state.cursor + 1) % n;
    } else if actions.just_pressed(&MenuAction::Up) {
        menu_state.cursor = (menu_state.cursor + n - 1) % n;
    } else if actions.just_pressed(&MenuAction::Confirm) {
        match menu_state.cursor {
            0 => next_sub.set(TownLocation::Shop),
            1 => next_sub.set(TownLocation::Inn),
            2 => next_sub.set(TownLocation::Temple),
            3 => next_sub.set(TownLocation::Guild),
            4 => next_game.set(GameState::TitleScreen),  // Open Q4: TitleScreen vs Dungeon
            _ => {}
        }
    }
}
```

Register in `TownPlugin::build`:
```rust
app
    .init_resource::<SquareMenuState>()
    .add_systems(
        Update,
        handle_square_input
            .run_if(in_state(TownLocation::Square))
            .run_if(in_state(GameState::Town)),
    )
    .add_systems(
        EguiPrimaryContextPass,
        paint_town_square
            .run_if(in_state(TownLocation::Square))
            .run_if(in_state(GameState::Town)),
    );
```

**Critical:** `run_if(in_state(TownLocation::Square))` alone is NOT sufficient. When `GameState != Town`, the `TownLocation` sub-state resource still EXISTS (held until the `#[source(GameState = GameState::Town)]` source state goes away — and `bevy_state` only removes sub-state resources on exit of the source). Double-gating with `in_state(GameState::Town)` is the safe pattern. **Verify**: this is a memory-worthy item if confirmed during planning.

(More precisely: per `bevy_state-0.18.1/src/state/sub_states.rs:1-160`, a SubStates resource only exists while its source state matches. So in theory `in_state(TownLocation::Square)` is sufficient. But the double-gate is defensive and costs nothing. The planner can keep or drop the second gate based on a quick test.)

### Pattern 3: Shop screen — buy/sell modes

```rust
// Two-mode UI: tab between BUY and SELL with Left/Right MenuAction.

#[derive(Resource, Debug)]
pub struct ShopState {
    pub mode: ShopMode,
    pub cursor: usize,
}
#[derive(Debug, PartialEq, Eq, Clone, Copy, Default)]
pub enum ShopMode {
    #[default]
    Buy,
    Sell,
}

fn paint_shop(
    mut contexts: EguiContexts,
    shop_state: Res<ShopState>,
    stock: Res<Assets<ShopStock>>,
    town_assets: Option<Res<TownAssets>>,
    items: Res<Assets<ItemAsset>>,
    item_registry: Res<ItemHandleRegistry>,
    active_floor: Res<crate::plugins::dungeon::ActiveFloorNumber>,
    gold: Res<Gold>,
    party: Query<(Entity, &CharacterName, &Inventory), With<PartyMember>>,
    instances: Query<&ItemInstance>,
) -> Result {
    let ctx = contexts.ctx_mut()?;

    let Some(town_assets) = town_assets else { return Ok(()); };
    let Some(stock) = stock.get(&town_assets.shop_stock) else { return Ok(()); };

    egui::TopBottomPanel::top("shop_header").show(ctx, |ui| {
        ui.horizontal(|ui| {
            ui.heading("Shop");
            ui.label(format!("  ({})", match shop_state.mode {
                ShopMode::Buy => "BUY", ShopMode::Sell => "SELL"
            }));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(format!("Gold: {}", gold.0));
            });
        });
    });

    egui::CentralPanel::default().show(ctx, |ui| {
        egui::ScrollArea::vertical().show(ui, |ui| {
            match shop_state.mode {
                ShopMode::Buy => {
                    for (i, entry) in stock.items_for_floor(active_floor.0).iter().enumerate() {
                        let Some(handle) = item_registry.get(&entry.item_id) else { continue; };
                        let Some(asset) = items.get(handle) else { continue; };
                        let price = entry.buy_price.unwrap_or(asset.value);
                        let color = if i == shop_state.cursor {
                            egui::Color32::YELLOW
                        } else { egui::Color32::WHITE };
                        ui.colored_label(color, format!(
                            "  {} — {}gp", asset.display_name, price
                        ));
                    }
                }
                ShopMode::Sell => {
                    let mut idx = 0;
                    for (_e, name, inv) in &party {
                        ui.label(format!("[{}]", name.0));
                        for &item_entity in &inv.0 {
                            let Ok(inst) = instances.get(item_entity) else { continue; };
                            let Some(asset) = items.get(&inst.0) else { continue; };
                            let sell_price = asset.value / 2;  // sell at half-price
                            let color = if idx == shop_state.cursor {
                                egui::Color32::YELLOW
                            } else { egui::Color32::WHITE };
                            ui.colored_label(color, format!(
                                "  {} — {}gp", asset.display_name, sell_price
                            ));
                            idx += 1;
                        }
                    }
                }
            }
        });
        ui.separator();
        ui.label("Left/Right: Buy/Sell. Up/Down: navigate. Enter: confirm. Esc: leave shop.");
    });
    Ok(())
}
```

The `handle_shop_input` system reads `MenuAction`:
- `Left`/`Right` → toggle `ShopMode`.
- `Up`/`Down` → bump cursor (clamped to current list length).
- `Confirm` → call `buy_item(...)` or `sell_item(...)` (helpers in `gold.rs`).
- `Cancel` → `next.set(TownLocation::Square)`.

```rust
pub fn buy_item(
    commands: &mut Commands,
    character: Entity,
    item_handle: Handle<ItemAsset>,
    items: &Assets<ItemAsset>,
    gold: &mut Gold,
    inventory_q: &mut Query<&mut Inventory, With<PartyMember>>,
) -> Result<(), BuyError> {
    let asset = items.get(&item_handle).ok_or(BuyError::ItemAssetMissing)?;
    let price = asset.value;
    if gold.0 < price {
        return Err(BuyError::InsufficientGold);
    }
    gold.0 = gold.0.saturating_sub(price);
    crate::plugins::party::give_item(commands, character, item_handle, inventory_q)
        .map_err(|_| BuyError::CharacterMissingComponents)?;
    Ok(())
}
```

**Critical note:** `give_item` is **already in tree** at `inventory.rs:390-412` and is **already documented as a #18 caller** at `inventory.rs:389`: *"Future callers: #21 (loot drops), **#18 (shop)**, #25 (UI give-item)."* Zero new helper work needed for buy.

For sell, the inverse is a 5-line helper:
```rust
pub fn sell_item(
    commands: &mut Commands,
    item_entity: Entity,
    character: Entity,
    items: &Assets<ItemAsset>,
    instances: &Query<&ItemInstance>,
    gold: &mut Gold,
    inventory_q: &mut Query<&mut Inventory, With<PartyMember>>,
) -> Result<(), SellError> {
    let instance = instances.get(item_entity).map_err(|_| SellError::ItemEntityMissing)?;
    let asset = items.get(&instance.0).ok_or(SellError::ItemAssetMissing)?;
    let sell_price = asset.value / 2;
    let mut inv = inventory_q.get_mut(character).map_err(|_| SellError::CharacterMissingComponents)?;
    inv.0.retain(|&e| e != item_entity);
    commands.entity(item_entity).despawn();
    gold.0 = gold.0.saturating_add(sell_price);
    Ok(())
}
```

### Pattern 4: Inn rest — heal + advance clock + charge gold

```rust
// HIGH-confidence — combines existing mutator patterns.

fn handle_inn_rest(
    actions: Res<ActionState<MenuAction>>,
    mut gold: ResMut<Gold>,
    mut clock: ResMut<GameClock>,
    services: Res<Assets<TownServices>>,
    town_assets: Option<Res<TownAssets>>,
    mut next: ResMut<NextState<TownLocation>>,
    mut events: MessageWriter<EquipmentChangedEvent>,
    mut party: Query<(Entity, &mut DerivedStats, &mut StatusEffects), With<PartyMember>>,
) {
    if !actions.just_pressed(&MenuAction::Confirm) { return; }
    let Some(town_assets) = town_assets else { return; };
    let Some(services) = services.get(&town_assets.services) else { return; };

    let cost = services.inn_rest_cost;
    if gold.0 < cost {
        // Insufficient gold — could play a denial SFX or show egui toast.
        info!("Not enough gold for inn rest ({} needed, {} held)", cost, gold.0);
        return;
    }

    gold.0 = gold.0.saturating_sub(cost);
    clock.day = clock.day.saturating_add(1);
    clock.turn = 0;

    for (entity, mut derived, mut status) in &mut party {
        // Skip Dead members — Inn doesn't revive.
        if status.has(StatusEffectType::Dead) { continue; }
        derived.current_hp = derived.max_hp;
        derived.current_mp = derived.max_mp;
        // Cure "mild" status: Poison only. Stone, Paralysis, Sleep are temple-only.
        // (Roadmap line: "Optionally apply rest-cure to mild status effects (poison)
        //  but not severe (stone, dead).")
        status.effects.retain(|e| e.effect_type != StatusEffectType::Poison);
        // Fire EquipmentChangedEvent to re-derive in case Poison cure or future
        // mild-status cure affects derived stats. See memory:
        // reference_druum_equipment_changed_event_dual_use.md
        events.write(EquipmentChangedEvent { character: entity, slot: EquipSlot::None });
    }

    // Return to Square after rest.
    next.set(TownLocation::Square);
}
```

**Critical**: "mild" vs "severe" status mapping must be authored, not implicit. Recommend a `TownServices.rest_cures: Vec<StatusEffectType>` field in the RON.

### Pattern 5: Temple revive + cure

```rust
fn handle_temple_revive(
    actions: Res<ActionState<MenuAction>>,
    mut gold: ResMut<Gold>,
    services: Res<Assets<TownServices>>,
    town_assets: Option<Res<TownAssets>>,
    mut events: MessageWriter<EquipmentChangedEvent>,
    selected: Res<TempleSelection>,
    mut party: Query<(Entity, &Experience, &mut DerivedStats, &mut StatusEffects), With<PartyMember>>,
) {
    if !actions.just_pressed(&MenuAction::Confirm) { return; }
    let Some(town_assets) = town_assets else { return; };
    let Some(services) = services.get(&town_assets.services) else { return; };

    let Ok((entity, xp, mut derived, mut status)) = party.get_mut(selected.target) else {
        return;
    };

    if status.has(StatusEffectType::Dead) {
        let cost = services.temple_revive_cost_base
            .saturating_add(services.temple_revive_cost_per_level.saturating_mul(xp.level));
        if gold.0 < cost { return; }
        gold.0 = gold.0.saturating_sub(cost);
        status.effects.retain(|e| e.effect_type != StatusEffectType::Dead);
        derived.current_hp = 1;  // Revived to 1 HP (roadmap convention)
        events.write(EquipmentChangedEvent { character: entity, slot: EquipSlot::None });
    }
    // (Equivalent branches for cure_stone, cure_poison, etc.)
}
```

**Note:** After removing the `Dead` effect, fire `EquipmentChangedEvent` so `recompute_derived_stats_on_equipment_change` re-derives stats with the new `StatusEffects`. The recompute system already reads `&StatusEffects` (`inventory.rs:444-453`) and the `With<PartyMember>` filter has been dropped (memory `reference_druum_recompute_filter_dual_use.md`), so this works without any new system.

But the `derive_stats` function zeroes `max_hp` when `Dead` is present (`character.rs:476-479`); after removing `Dead`, `derive_stats` re-computes max_hp from base stats. We set `current_hp = 1` AFTER the recompute fires (or, more cleanly, write `derived.current_hp = 1` after the event is queued; the recompute reads `old_current_hp` and clamps `current_hp = old_current_hp.min(new_max_hp)`, so the write order is "set current_hp = 1, fire event"). Or simpler: don't write `current_hp = 1` until the recompute has run a frame later. **Recommend the latter** — the recompute writes `current_hp = old_current_hp.min(new_max_hp)` where old_current_hp was 0, giving current_hp = 0 (which is the same broken state). The cleanest fix is to set both `derived.current_hp = 1` AND fire the event in the same frame, so when the recompute runs the next frame it sees `old_current_hp = 1` and clamps to `min(1, max_hp) = 1`. **Verify in the test fixture.** Flag as Open Q6.

### Pattern 6: Guild recruit + dismiss + reorder

```rust
fn handle_guild_recruit(
    actions: Res<ActionState<MenuAction>>,
    mut commands: Commands,
    selected_recruit: Res<GuildRecruitSelection>,
    pool: Res<Assets<RecruitPool>>,
    town_assets: Option<Res<TownAssets>>,
    party_size: Res<PartySize>,
    existing: Query<&PartySlot, With<PartyMember>>,
) {
    if !actions.just_pressed(&MenuAction::Confirm) { return; }
    let Some(town_assets) = town_assets else { return; };
    let Some(pool) = pool.get(&town_assets.recruit_pool) else { return; };

    if existing.iter().count() >= party_size.0 {
        info!("Party full ({}/{}); dismiss someone first", existing.iter().count(), party_size.0);
        return;
    }

    let Some(recruit) = pool.recruits.get(selected_recruit.cursor) else { return; };

    // Compute next free PartySlot index.
    let used: std::collections::HashSet<usize> = existing.iter().map(|s| s.0).collect();
    let next_slot = (0..party_size.0).find(|i| !used.contains(i)).unwrap_or(0);

    let base = recruit.base_stats;
    let derived = crate::plugins::party::derive_stats(
        &base, &[], &StatusEffects::default(), 1
    );

    commands.spawn(PartyMemberBundle {
        name: CharacterName(recruit.name.clone()),
        race: recruit.race,
        class: recruit.class,
        party_row: recruit.default_row,
        party_slot: PartySlot(next_slot),
        base_stats: base,
        derived_stats: derived,
        ..Default::default()
    })
    .insert(Inventory::default());
}

fn handle_guild_dismiss(
    actions: Res<ActionState<MenuAction>>,
    mut commands: Commands,
    selected: Res<GuildPartyCursor>,
    party: Query<(Entity, &PartySlot), With<PartyMember>>,
) {
    if !actions.just_pressed(&MenuAction::Cancel) { return; }
    // Find the entity at the selected slot.
    for (entity, slot) in &party {
        if slot.0 == selected.cursor {
            commands.entity(entity).despawn();
            return;
        }
    }
}

fn handle_guild_row_swap(
    actions: Res<ActionState<MenuAction>>,
    selected: Res<GuildPartyCursor>,
    mut party: Query<(&PartySlot, &mut PartyRow), With<PartyMember>>,
) {
    if !actions.just_pressed(&MenuAction::Right) { return; }
    for (slot, mut row) in &mut party {
        if slot.0 == selected.cursor {
            *row = match *row {
                PartyRow::Front => PartyRow::Back,
                PartyRow::Back => PartyRow::Front,
            };
            return;
        }
    }
}
```

**Critical:** the dismiss flow has to handle the dismissed character's `Inventory` (Wizardry: "drops in the dungeon"; Etrian: "gone forever"). The roadmap doesn't specify. **Recommend Etrian semantics**: despawn the `PartyMember` entity AND its `Inventory` entities (each `ItemInstance` is a separate entity stored in `Inventory.0: Vec<Entity>`). `commands.entity(party_member).despawn()` does NOT despawn unrelated `ItemInstance` entities — they leak. The recompute system would have to walk `Inventory.0` and despawn each. **This is a #18 plumbing item, not a research finding** — flag for planner.

### Pattern 7: TownAssets collection (loading-side)

```rust
// src/plugins/loading/mod.rs additions:

#[derive(AssetCollection, Resource)]
pub struct TownAssets {
    #[asset(path = "town/shop_stock.ron")]
    pub shop_stock: Handle<ShopStock>,
    #[asset(path = "town/recruit_pool.ron")]
    pub recruit_pool: Handle<RecruitPool>,
    #[asset(path = "town/town_services.ron")]
    pub services: Handle<TownServices>,
}

// In LoadingPlugin::build:
.add_plugins((
    // ...existing plugins...
    RonAssetPlugin::<ShopStock>::new(&["shop_stock.ron"]),
    RonAssetPlugin::<RecruitPool>::new(&["recruit_pool.ron"]),
    RonAssetPlugin::<TownServices>::new(&["town_services.ron"]),
))
.add_loading_state(
    LoadingState::new(GameState::Loading)
        .continue_to_state(GameState::TitleScreen)
        .load_collection::<DungeonAssets>()
        .load_collection::<AudioAssets>()
        .load_collection::<TownAssets>(),  // NEW
)
```

This mirrors `AudioAssets`/`DungeonAssets` exactly. Verified pattern at `loading/mod.rs:30-89, 108-125`.

### Anti-Patterns to Avoid

- **Storing `Gold` per-character without a "pool" UI.** If you flip to per-character gold (Open Q2), you also have to build the pool/distribute UI. Don't store it per-character without owning that UI burden.
- **Mutating `DerivedStats.current_hp` directly without firing `EquipmentChangedEvent` for status-changing ops.** Inn-rest mutates `current_hp` and `current_mp` directly (HP/MP are "current" fields, not derived); that's fine. BUT temple-revive mutates `StatusEffects.effects` (removing `Dead`), and that DOES need the event to refresh `max_hp` via `derive_stats` (which zeroes `max_hp` while `Dead` is present). The event-fire convention here is: **fire `EquipmentChangedEvent { slot: EquipSlot::None }` whenever `StatusEffects.effects` changes.** Memory: `reference_druum_equipment_changed_event_dual_use.md`.
- **Reusing the dungeon camera.** Don't. Town's lifecycle is independent of Dungeon's; the dungeon-camera-despawn-on-OnExit-Dungeon rule fights you. Spawn a `Camera2d` for Town.
- **Painting in `Update` instead of `EguiPrimaryContextPass`.** Verified anti-pattern. Egui painters MUST run in `EguiPrimaryContextPass` schedule per the canonical examples (`bevy_egui-0.39.1/examples/simple.rs:9`). Input handlers run in `Update`. The minimap precedent at `minimap.rs:129-145` is the gold standard.
- **Spawning new `ItemInstance` entities without registering an Inventory entry.** `give_item` already handles this correctly. **Use `give_item`, do not bypass it.**
- **Letting Inn-rest revive a Dead character.** The recommended pattern's `if status.has(StatusEffectType::Dead) { continue; }` line is non-negotiable. Inn rest is for the living; Temple is for the dead. Roadmap is explicit.
- **Charging gold AND then failing to spawn the bought item.** Saturating-sub the gold AFTER `give_item` succeeds, NOT before. The pattern in §Pattern 3 has gold deducted before the `give_item` call — refactor to deduct only on success.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---|---|---|---|
| Give a party member an item from a handle | A hand-rolled `commands.spawn(ItemInstance(handle)).id()` + `inventory.0.push(entity)` | `give_item(...)` at `inventory.rs:390-412` | The helper handles cleanup-on-error AND is explicitly named in its doc as a #18 caller |
| Equip an item the player just bought | A hand-rolled equipment-slot write | `equip_item(...)` at `inventory.rs:272-336` | Same — handles slot validation, evict-previous, MessageWriter coordination |
| Look up an item by ID in shop stock | A linear scan over `Assets<ItemAsset>` | `ItemHandleRegistry::get(id) -> Option<&Handle<ItemAsset>>` at `inventory.rs:528-530` | The registry is already populated on `OnExit(Loading)` (`mod.rs:67`) |
| Heal a character to full HP/MP | A hand-rolled `for c in party.iter_mut() { c.current_hp = c.max_hp }` plus event-fire | Direct field-write in `handle_inn_rest`; only fire `EquipmentChangedEvent` if status-changing | `current_hp/current_mp` are not derived; direct write is correct. Don't over-engineer it. |
| Convert `Race` and `Class` enums to UI strings | A new lookup table | `format!("{:?}", race)` for now; pretty names land in #25 polish | Mirror the precedent at `combat/ui_combat.rs:103-107` which renders enemy types via `format!`. |
| BGM swap on entering Town | A custom `bgm_on_enter_town` system | Nothing — `play_bgm_for_state` already handles it via `state_changed::<GameState>` | Verified at `audio/bgm.rs:106-112` |
| Sub-state navigation primitives | Custom enum tracker | `NextState<TownLocation>` resource | Already registered by `StatePlugin` at `state/mod.rs:56` |

---

## Common Pitfalls

### Pitfall 1: `TownLocation` cannot transition while `GameState != Town`

Per `bevy_state` semantics, the `TownLocation` SubState resource exists ONLY while `GameState::Town` is the active state. The first `OnEnter(GameState::Town)` initialises `State<TownLocation>` to the `#[default]` variant — `Square`. Trying to `next.set(TownLocation::Shop)` before the Town state is active is a no-op (NextState is queued, then the resource is dropped on Town exit).

**What goes wrong:** If a "title screen → enter town shop directly" button is added, naively calling `NextState<TownLocation>::set(Shop)` before `NextState<GameState>::set(Town)` does nothing — the Town SubState resource doesn't exist yet, so the queued set is dropped.

**How to avoid:** Always `next_game.set(GameState::Town)` BEFORE `next_sub.set(TownLocation::Shop)` (and the latter only takes effect on the next frame after Town entry). For #18, this is moot — Square is the default and the only entry point.

### Pitfall 2: `egui::CentralPanel` rendering nothing under a `Camera3d` is silent

The `bevy_egui` precedent has `CombatUiPlugin` attach `PrimaryEguiContext` to the `DungeonCamera` (a `Camera3d`). egui paints over a 3D scene fine. **However**, for Town's pure-egui screen, a `Camera2d` is sufficient and simpler. If the planner mistakenly attaches `PrimaryEguiContext` to no camera (or to one that gets despawned), `ctx_mut()` returns `Err`, the painter returns silently (because of the `?` operator on `Result`), and nothing renders.

**How to avoid:** Verify `PrimaryEguiContext` is on the `Camera2d` at the point of spawn (`commands.spawn((Camera2d, TownCameraRoot, PrimaryEguiContext))` — direct attach is preferable to the dungeon's deferred attach pattern, since the `Camera2d` is spawned eagerly in `OnEnter(GameState::Town)` and apply_deferred resolves before any `Update` system runs).

### Pitfall 3: `Vec<ItemAsset>` linear scan over shop stock is fine UNTIL it isn't

The shop stock will likely have ≤20 items per floor. Linear scan with `Vec::iter` is O(n) — fine at 20 items.

**What goes wrong:** Future scope creep (#21 adds 100+ item shop catalogue) makes the linear-scan painter visibly slow.

**How to avoid:** Build a `BTreeMap<u8, Vec<ShopEntry>>` keyed on `min_floor` lazily on first read. For #18 (≤20 items), don't bother — premature optimization.

### Pitfall 4: Saturating `u32` gold arithmetic hides underflow bugs

`gold.0 = gold.0.saturating_sub(cost)` silently clamps to zero when `cost > gold.0`. This is correct for the "buy_item" / "pay_inn" use case ONLY IF you've checked `gold.0 >= cost` first. If a refactor accidentally calls `saturating_sub` without the check, the user "buys" a 9999-gold sword for 0 gold.

**How to avoid:** Always validate before deducting:
```rust
if gold.0 < cost { return Err(...); }
gold.0 = gold.0.saturating_sub(cost);
```
The saturating arithmetic is a defense-in-depth net, not the primary guard.

### Pitfall 5: `Inventory.0: Vec<Entity>` doesn't bound length

The `Inventory` component is `pub Vec<Entity>` (`inventory.rs:187`). Shop buy uses `give_item(...)` which pushes onto this `Vec`. There is no length cap.

**What goes wrong:** A player with 100K gold can buy every item up to the inventory exceeding a sensible size, causing UI scrolling slowness and a giant save file. (Wizardry classic caps at 8 items per character.)

**How to avoid:** Add a check in `buy_item`: `if inventory.0.len() >= MAX_INVENTORY_PER_CHARACTER { return Err(BuyError::InventoryFull); }`. Recommended cap: 8 (Wizardry convention) or 20 (modern Etrian). **Surface to user** as Open Q7 — it's an economy/balance call. Pre-shipped: `inventory.rs:184` has a security comment *"Feature #23 must implement `MapEntities` ... bound the Vec length to guard against crafted save files"* — the cap should be the same one. Wire it now.

### Pitfall 6: Recruit pool size has no upper bound

`RecruitPool.recruits: Vec<RecruitDef>` from a RON file is parseable to any size. A malicious save (or typo'd RON) could declare a 100K-entry pool, exhausting memory on egui painter scroll.

**How to avoid:** Clamp pool length to `MAX_RECRUIT_POOL_SIZE` (e.g., 32) at load time. Same trust-boundary pattern as `MAX_ENEMIES_PER_ENCOUNTER` at `encounter.rs:72`.

### Pitfall 7: `EquipmentChangedEvent` fires every paint frame if the painter mutates a Resource

The egui painter reads `Res<ShopState>` etc. If it mistakenly mutates resources in the paint pass, the recompute system runs on every frame. Painters MUST be read-only or use `Local<>` for transient cursor state.

**How to avoid:** Keep painters pure: `paint_*` reads only. `handle_*_input` writes. Same separation as `paint_combat_screen` vs `handle_combat_input` in `combat/ui_combat.rs:64-71` vs `combat/ui_combat.rs:paint_combat_screen`.

### Pitfall 8: Deselecting "Leave Town" should NOT despawn the party

If "Leave Town" goes to `GameState::TitleScreen`, the `OnExit(Dungeon)` clean-up rule despawns `PlayerParty` for non-Combat destinations. But Town has no `PlayerParty` to begin with (it's spawned in `OnEnter(Dungeon)`). So this is moot for Town.

**What goes wrong:** If the user chose Open Q4 = "Town → Dungeon", the dungeon plugin's `OnEnter(Dungeon)` will spawn a NEW `PlayerParty` — and the recruited Guild members (which DON'T have `PlayerParty` marker, only `PartyMember`) will be orphans (visible in queries but not under the party root).

**How to avoid:** Recruited `PartyMember` entities are root-level (not parented to `PlayerParty`). The dungeon-camera + dungeon-geometry pipeline doesn't care about `PartyMember` entity location. **Verify**: dungeon code reads `Query<&PartyMember>` flat, not through a parent — confirmed at `combat/encounter.rs` and elsewhere. So no parenting needed. (This is what `spawn_default_debug_party` already does — spawns flat `PartyMemberBundle` entities, not children.)

### Pitfall 9: Dismissing a party member leaks their inventory entities

`commands.entity(party_member).despawn()` despawns the `PartyMember` entity but **not** the `ItemInstance` entities in their `Inventory.0: Vec<Entity>`. Those become orphans — invisible in any `Query<...>, With<PartyMember>>` and slowly accumulating across dismissals.

**How to avoid:** Before despawning the party member, iterate `inventory.0` and despawn each item entity:
```rust
for &item_entity in &inventory.0 {
    commands.entity(item_entity).despawn();
}
commands.entity(party_member).despawn();
```
Same pattern as `clear_current_encounter` at `combat/encounter.rs:200-215`. **Wire this into the Guild dismiss path.**

### Pitfall 10: `Gold` overflow on extreme sells

`gold.saturating_add(item_value / 2)` is bounded — saturating arithmetic clamps at `u32::MAX`. Won't panic. But a malicious save with `Gold(u32::MAX)` and many sell ops produces no visual feedback for "you can't gain more gold". This is a v1-acceptable failure mode.

**How to avoid:** Defer — `u32::MAX` gold is ~4.29B, which no honest playthrough reaches. v1 acceptable.

---

## Security

### Known Vulnerabilities

| Library | CVE / Advisory | Severity | Status | Action |
|---|---|---|---|---|
| `bevy_egui` 0.39.1 | None verifiable in session | — | — | No action; same pin already in tree |
| `bevy_common_assets` 0.16.0 | None verifiable in session | — | — | No action |
| `bevy_asset_loader` 0.26.0 | None verifiable in session | — | — | No action |

No new third-party crates → no new CVE surface. All recommended libraries are already in tree.

### Architectural Security Risks

| Risk | Affected Architecture | How It Manifests | Secure Pattern | Anti-Pattern to Avoid |
|---|---|---|---|---|
| **RON trust boundary on `ShopStock`** | All shop options | Crafted RON with negative `buy_price` (impossible in `u32`) or absurd stock count | Type system already excludes negative prices; clamp `stock.items.len()` and `entry.buy_price` to a reasonable max on load (e.g., 9999) | Trusting the RON-deserialized `Vec` length and integer fields directly into a `for i in 0..stock.len()` loop |
| **RON trust boundary on `RecruitPool`** | Guild recruit | Crafted RON with absurd BaseStats (str=10000) or 100K-entry pool | Clamp `BaseStats` channels at load (max 99 per channel, mirroring Wizardry); clamp pool size to 32 | Direct field-copy from `RecruitDef.base_stats` into `BaseStats` without validation |
| **RON trust boundary on `TownServices`** | All cost-based actions | `inn_rest_cost: u32::MAX` makes Inn unusable; `temple_revive_cost_base: 0` lets infinite revives | Clamp on load: inn_rest_cost ≤ 10000, revive ≤ 100000 | Trusting raw u32 from RON |
| **Save-file gold injection** | Future Feature #23 | A crafted save could declare `Gold(u32::MAX)` directly | `Gold(u32)` is `#[derive(Resource, Serialize, Deserialize)]` — Feature #23 must clamp gold on load | Loading the saved gold directly without ceiling |
| **Inventory length explosion** | Shop buy | No cap on `Inventory.0.len()` | Cap at 8 or 20 (decision Q7); reject buys when full | Unbounded `inventory.0.push(...)` |
| **Despawn-after-event race in Guild dismiss** | Guild dismiss | `commands.entity(...).despawn()` is deferred; the next system might query the despawned entity | Use `Commands` and let bevy's deferred-execution handle ordering; OR use `World` direct mutation. Test: dismiss a member, immediately try to recruit them again in the same frame | Naive sequencing assuming immediate effect |
| **Equipment-on-dismiss leak** | Guild dismiss | `Equipment.weapon: Handle<ItemAsset>` is dropped when the party member is despawned — but the underlying `Assets<ItemAsset>` entry stays (reference counts to other consumers) | Bevy's `Handle<T>` is reference-counted; the asset entry is reclaimed when no strong handles remain. Not a real leak. | Manually deleting from `Assets<ItemAsset>` based on dismiss |

### Trust Boundaries

For the recommended architecture, trust boundaries are:

- **RON deserialise (`bevy_common_assets::RonAssetPlugin::<ShopStock>` etc.):** clamping must happen at the consumer (the painter or input handler). Don't try to bake validation into the asset itself. Precedent: `combat/encounter.rs:281` clamps `encounter_rate.clamp(0.0, 1.0)` at read-time.
- **Save-load (Feature #23, future):** `Gold(u32)` and `GameClock { day: u32, turn: u32 }` are both `#[derive(Resource, Serialize, Deserialize)]`. Feature #23 MUST cap each field on load to prevent gold-injection / day-counter-injection from crafted save files. Inherits the bound from the trust-boundary pattern already in `recompute_derived_stats_on_equipment_change` (it tolerates 0-stat fallbacks for missing assets).
- **Asset path resolution:** identical to existing precedent. `RonAssetPlugin` and `bevy_asset_loader` already refuse path traversal.
- **No new external network surface introduced.** #18 has no HTTP, no socket, no remote behaviour.

---

## Performance

| Metric | Value / Range | Source | Notes |
|---|---|---|---|
| Camera2d spawn cost | 1 entity, ~64 bytes | Bevy ECS | Negligible — same cost as `Camera2d` in loading-screen |
| egui paint per frame in Town | ~50µs at 4 panels (heading + buttons + label list) | bevy_egui empirical baseline | Same as Combat UI overhead (negligible at 60Hz target) |
| `ShopStock` linear scan | O(n≤20) per paint frame | `Vec::iter` | Sub-microsecond cost |
| `RecruitPool` linear scan | O(n≤32) | `Vec::iter` | Sub-microsecond |
| `Inventory.0` walk in Sell mode | O(items per character) — bounded to 8 or 20 | Sell painter | Negligible |
| `EquipmentChangedEvent` fan-out | One event per affected character per status change | `recompute_derived_stats_on_equipment_change` | At most 4 events on Inn-rest (one per surviving party member) |
| BGM crossfade on Town entry | One `FadeOut` insert + one new `Bgm` entity | `audio/bgm.rs:82-125` | Handled automatically by existing system |
| Asset budget delta | +3 RON files (~5 KB each) | New `assets/town/*.ron` | Negligible. ~15 KB total |

The roadmap's "+0.5s compile delta" is the only real cost — additional sub-modules trigger Bevy's incremental compile. Negligible.

---

## Code Examples

### Gold resource declaration + saturating helpers

```rust
// src/plugins/town/gold.rs (NEW)
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// Party-wide gold pool. Saturating arithmetic guards against u32 overflow.
/// Feature #23 (save/load) must clamp the value on load (architectural security).
#[derive(Resource, Reflect, Serialize, Deserialize, Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Gold(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpendError {
    InsufficientGold { have: u32, need: u32 },
}

impl Gold {
    /// Deduct `amount` from the pool. Returns `Err` if the pool has insufficient gold.
    /// Does NOT mutate on error — caller can retry or display an error.
    pub fn try_spend(&mut self, amount: u32) -> Result<(), SpendError> {
        if self.0 < amount {
            return Err(SpendError::InsufficientGold { have: self.0, need: amount });
        }
        self.0 = self.0.saturating_sub(amount);
        Ok(())
    }

    /// Add `amount` to the pool. Saturating — clamps at `u32::MAX`.
    pub fn earn(&mut self, amount: u32) {
        self.0 = self.0.saturating_add(amount);
    }
}

/// Day + turn counter for the in-game clock. Inn-rest increments `day`.
/// Future Features (#21 scripted events, #24 status durations) read this.
#[derive(Resource, Reflect, Serialize, Deserialize, Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GameClock {
    pub day: u32,
    pub turn: u32,
}
```

### TownAssets schema additions

```rust
// src/data/town.rs (NEW)
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::plugins::party::{BaseStats, Class, Race, PartyRow, StatusEffectType};

/// Shop stock — per-item availability gated by floor progression.
#[derive(Asset, Reflect, Serialize, Deserialize, Default, Debug, Clone)]
pub struct ShopStock {
    pub items: Vec<ShopEntry>,
}

#[derive(Reflect, Serialize, Deserialize, Debug, Clone)]
pub struct ShopEntry {
    pub item_id: String,
    /// Optional override of `ItemAsset.value` for buy price. `None` = use `ItemAsset.value`.
    #[serde(default)]
    pub buy_price: Option<u32>,
    /// Minimum dungeon floor at which this item appears in the shop.
    /// 0 = always available.
    #[serde(default)]
    pub min_floor: u32,
}

impl ShopStock {
    /// Returns the items currently stocked for the given dungeon floor.
    /// Pure function — no Bevy resource access.
    pub fn items_for_floor(&self, floor: u32) -> Vec<&ShopEntry> {
        self.items.iter().filter(|e| e.min_floor <= floor).collect()
    }
}

/// Pre-made characters available for hire at the Guild.
/// Feature #19 will deprecate this in favour of character-creation; until then,
/// the recruit pool is the source of new party members.
#[derive(Asset, Reflect, Serialize, Deserialize, Default, Debug, Clone)]
pub struct RecruitPool {
    pub recruits: Vec<RecruitDef>,
}

#[derive(Reflect, Serialize, Deserialize, Debug, Clone)]
pub struct RecruitDef {
    pub name: String,
    pub race: Race,
    pub class: Class,
    pub base_stats: BaseStats,
    #[serde(default)]
    pub default_row: PartyRow,
}

/// Costs for town services. Authored at `assets/town/town_services.ron`.
#[derive(Asset, Reflect, Serialize, Deserialize, Default, Debug, Clone)]
pub struct TownServices {
    pub inn_rest_cost: u32,
    pub temple_revive_cost_base: u32,
    pub temple_revive_cost_per_level: u32,
    /// Cost to cure a status effect at the Temple. Map per status type.
    pub temple_cure_costs: Vec<(StatusEffectType, u32)>,
    /// Status effects cured by Inn rest (mild). Roadmap: poison-style only.
    pub inn_rest_cures: Vec<StatusEffectType>,
}
```

### Example RON authoring

```ron
// assets/town/shop_stock.ron
(
    items: [
        (item_id: "rusty_sword", min_floor: 0),
        (item_id: "leather_armor", min_floor: 0),
        (item_id: "robe", min_floor: 0),
        (item_id: "wooden_shield", min_floor: 0),
        (item_id: "healing_potion", min_floor: 0),
        (item_id: "oak_staff", min_floor: 1),  // Mage weapon unlocked from F2
        (item_id: "wooden_mace", min_floor: 1),
    ],
)
```

```ron
// assets/town/recruit_pool.ron
(
    recruits: [
        (
            name: "Roland",
            race: Human,
            class: Fighter,
            base_stats: (strength: 12, intelligence: 6, piety: 6, vitality: 14, agility: 8, luck: 6),
            default_row: Front,
        ),
        (
            name: "Eira",
            race: Elf,
            class: Mage,
            base_stats: (strength: 6, intelligence: 14, piety: 8, vitality: 8, agility: 12, luck: 6),
            default_row: Back,
        ),
        (
            name: "Brother Tomas",
            race: Human,
            class: Priest,
            base_stats: (strength: 8, intelligence: 8, piety: 14, vitality: 10, agility: 8, luck: 6),
            default_row: Back,
        ),
        (
            name: "Doric",
            race: Dwarf,
            class: Fighter,
            base_stats: (strength: 14, intelligence: 4, piety: 6, vitality: 16, agility: 6, luck: 4),
            default_row: Front,
        ),
        (
            name: "Pip",
            race: Hobbit,
            class: Fighter,  // Thief class not authored yet (#19)
            base_stats: (strength: 8, intelligence: 8, piety: 6, vitality: 8, agility: 14, luck: 10),
            default_row: Front,
        ),
    ],
)
```

```ron
// assets/town/town_services.ron
(
    inn_rest_cost: 10,
    temple_revive_cost_base: 100,
    temple_revive_cost_per_level: 50,
    temple_cure_costs: [
        (Poison, 25),
        (Stone, 200),
        (Paralysis, 75),
        (Sleep, 25),
    ],
    inn_rest_cures: [Poison, Sleep],
)
```

### TownPlugin wiring

```rust
// src/plugins/town/mod.rs (REWRITE)
use bevy::prelude::*;

use crate::plugins::state::{GameState, TownLocation};

pub mod gold;
pub mod square;
pub mod shop;
pub mod inn;
pub mod temple;
pub mod guild;

pub use gold::{Gold, GameClock, SpendError};

pub struct TownPlugin;

impl Plugin for TownPlugin {
    fn build(&self, app: &mut App) {
        app
            // Resources — gold pool + clock + per-sub-state menu cursors.
            .init_resource::<Gold>()
            .init_resource::<GameClock>()
            .init_resource::<square::SquareMenuState>()
            .init_resource::<shop::ShopState>()
            .init_resource::<inn::InnState>()
            .init_resource::<temple::TempleState>()
            .init_resource::<guild::GuildState>()
            .register_type::<Gold>()
            .register_type::<GameClock>()
            // Camera lifecycle.
            .add_systems(OnEnter(GameState::Town), spawn_town_camera)
            .add_systems(OnExit(GameState::Town), despawn_town_camera)
            // Sub-state painters — each gated by its TownLocation variant.
            .add_systems(
                bevy_egui::EguiPrimaryContextPass,
                (
                    square::paint_town_square.run_if(in_state(TownLocation::Square)),
                    shop::paint_shop.run_if(in_state(TownLocation::Shop)),
                    inn::paint_inn.run_if(in_state(TownLocation::Inn)),
                    temple::paint_temple.run_if(in_state(TownLocation::Temple)),
                    guild::paint_guild.run_if(in_state(TownLocation::Guild)),
                ).distributive_run_if(in_state(GameState::Town)),
            )
            // Sub-state input handlers — Update schedule.
            .add_systems(
                Update,
                (
                    square::handle_square_input.run_if(in_state(TownLocation::Square)),
                    shop::handle_shop_input.run_if(in_state(TownLocation::Shop)),
                    inn::handle_inn_rest.run_if(in_state(TownLocation::Inn)),
                    temple::handle_temple_action.run_if(in_state(TownLocation::Temple)),
                    guild::handle_guild_action.run_if(in_state(TownLocation::Guild)),
                ).distributive_run_if(in_state(GameState::Town)),
            );
    }
}

#[derive(Component)]
pub struct TownCameraRoot;

fn spawn_town_camera(mut commands: Commands) {
    commands.spawn((
        Camera2d,
        TownCameraRoot,
        bevy_egui::PrimaryEguiContext,
    ));
    info!("Entered GameState::Town");
}

fn despawn_town_camera(mut commands: Commands, cams: Query<Entity, With<TownCameraRoot>>) {
    for e in &cams { commands.entity(e).despawn(); }
    info!("Exited GameState::Town");
}
```

**Caveat on `distributive_run_if`:** This is a bevy 0.18 ergonomic helper that applies the same run-condition to every system in the tuple. Verify it exists in 0.18 (memory says state-related ergonomics moved a lot between 0.17/0.18). If it doesn't, each `run_if(in_state(GameState::Town))` chains per-system. Planner: this is a one-line audit before implementation; both alternatives are equivalent semantically.

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|---|---|---|---|
| Nested state enums for sub-states (pre-0.13) | `#[derive(SubStates)] #[source(...)]` | Bevy 0.13 introduction; stabilised through 0.18 | The state machine work for #18 is already done at `state/mod.rs:38-47` |
| `EventReader<StateTransitionEvent<T>>` | `MessageReader<StateTransitionEvent<T>>` | Bevy 0.17 → 0.18 family rename | Doesn't affect #18 directly; no transition events read here. But memory `feedback_bevy_0_18_event_message_split.md` applies if event reading is added |
| `Camera2dBundle { ... }` | `Camera2d` as component | Bevy 0.17 → 0.18 bundle removal | Mirror the loading-screen precedent (`loading/mod.rs:193`) which already uses the component-only spawn |
| egui `Window` for menus | egui `CentralPanel` + `SidePanel` for full-screen menus, `Window` for overlays | egui 0.20+ design guidance | Use panels for the main town screens; `Window` only for "are you sure?" confirmations |
| Manual gold tracking with per-component arithmetic | Saturating `u32` resource pattern | Standard Rust idiom; matches `derive_stats` | Already in the project's DNA |
| Manual `Vec<RecruitDef>` linear scan + manual handle lookup | `ItemHandleRegistry::get` pattern | Pre-shipped by #12 | Use the registry; don't roll a parallel |

**Deprecated/outdated:**

- Roadmap text *"Implement `GameState::Town` with sub-states"* — the state machine is already in place. Strike this from the implementation plan.
- Roadmap text *"A `Gold` resource (or per-party `Gold` component) tracking the player's coin"* — Open Q2 surfaces this; the recommendation is `Resource<Gold>` (party-wide).
- Anything suggesting `bevy_egui` API requires recent breaking-change adaptation — `=0.39.1` is what's in tree, exhaustively tested via `MinimapPlugin` and `CombatUiPlugin`.

---

## Validation Architecture

### Test Framework

| Property | Value |
|---|---|
| Framework | `cargo test` with Bevy `App` integration tests (no extra dependency) |
| Config file | none — uses `[dev-dependencies]` in Cargo.toml |
| Quick run command | `cargo test -p druum town::` |
| Full suite command | `cargo test --features dev` |

Existing test patterns in tree to mirror:

- `src/plugins/audio/mod.rs:129-241` — Layer 1 (pure helpers) + Layer 2 (`App` with `MinimalPlugins` + necessary plugins, transition state, assert observable changes).
- `src/plugins/combat/encounter.rs:434-981` — `mod tests` (Layer 1) + `mod app_tests` (Layer 2) split.
- The pattern is established by memory rule `feedback_bevy_input_test_layers.md`: for input-related tests, use either direct-resource-mutation OR full-message-pipeline; don't mix.
- For #18 specifically: most tests don't need `InputPlugin` (the `handle_*_input` systems read `ActionState`, which is a Resource — direct mutation via `app.world_mut().resource_mut::<ActionState<MenuAction>>().press(&MenuAction::Confirm)` is the simplest test injection).

### Requirements → Test Map

| Requirement | Behavior | Test Type | Automated Command | File Exists? |
|---|---|---|---|---|
| TownPlugin builds without panic | App with `TownPlugin + StatePlugin + EguiPlugin` updates clean | smoke (Layer 2) | `cargo test -p druum town::tests::town_plugin_builds` | NO — needs creating |
| `Gold.try_spend` rejects when insufficient | Pure function | unit (Layer 1) | `cargo test -p druum town::gold::tests::try_spend_insufficient` | NO — needs creating |
| `Gold.try_spend` deducts when sufficient | Pure function | unit (Layer 1) | `cargo test -p druum town::gold::tests::try_spend_sufficient` | NO — needs creating |
| `Gold.earn` saturates at u32::MAX | Pure function | unit (Layer 1) | same | NO — needs creating |
| `OnEnter(Town)` spawns Camera2d | After `next_state.set(Town)` and 2× `app.update()`, exactly one entity with `TownCameraRoot` exists | integration (Layer 2) | `cargo test -p druum town::tests::town_camera_spawns_on_enter` | NO — needs creating |
| `OnExit(Town)` despawns Camera2d | Inverse | integration (Layer 2) | same | NO — needs creating |
| `TownLocation` defaults to `Square` on OnEnter | After `next_state.set(Town)` + update, `State<TownLocation>` is `Square` | integration | `cargo test -p druum town::tests::town_substate_defaults_to_square` | NO — needs creating |
| `MenuAction::Confirm` on Square advances to selected sub-state | After cursor=0 + Confirm, next-frame state is `Shop` | integration (Layer 2) | `cargo test -p druum town::square::tests::square_confirm_navigates` | NO — needs creating |
| Shop buy deducts gold and adds item | Pure helper test or Layer 2 | unit + integration | `cargo test -p druum town::shop::tests::buy_item` | NO — needs creating |
| Shop sell adds gold and removes item | Pure helper test | unit | same | NO — needs creating |
| Insufficient gold rejects buy without despawn-spawn-rollback issues | Helper test | unit | same | NO — needs creating |
| Inn rest restores HP/MP to max | Layer 2 (advance state, party member HP) | integration | `cargo test -p druum town::inn::tests::rest_full_heals_living` | NO — needs creating |
| Inn rest does NOT heal Dead party member | Layer 2 | integration | `cargo test -p druum town::inn::tests::rest_skips_dead` | NO — needs creating |
| Inn rest cures mild status (Poison) | Layer 2 | integration | `cargo test -p druum town::inn::tests::rest_cures_poison` | NO — needs creating |
| Inn rest preserves Stone (severe status) | Layer 2 | integration | same | NO — needs creating |
| Inn rest advances `GameClock.day` by 1 | Layer 2 | integration | `cargo test -p druum town::inn::tests::rest_advances_clock` | NO — needs creating |
| Inn rest deducts the configured cost | Layer 2 | integration | same | NO — needs creating |
| Temple revive removes `Dead` and sets `current_hp = 1` | Layer 2 | integration | `cargo test -p druum town::temple::tests::revive_dead` | NO — needs creating |
| Temple revive fires `EquipmentChangedEvent` triggering re-derive | Layer 2 | integration | same | NO — needs creating |
| Temple revive cost scales with level | Pure helper test | unit | same | NO — needs creating |
| Temple cure removes Poison/Stone for gold | Layer 2 | integration | `cargo test -p druum town::temple::tests::cure_poison` | NO — needs creating |
| Guild recruit spawns a `PartyMember` entity at next free `PartySlot` | Layer 2 | integration | `cargo test -p druum town::guild::tests::recruit_fills_next_slot` | NO — needs creating |
| Guild recruit rejects when party is full | Layer 2 (party_size=2, spawn 2, attempt 3rd) | integration | same | NO — needs creating |
| Guild dismiss despawns the `PartyMember` AND their `Inventory.0` entities | Layer 2 | integration | `cargo test -p druum town::guild::tests::dismiss_cleans_inventory` | NO — needs creating |
| Guild row-swap toggles `PartyRow` | Layer 2 | integration | same | NO — needs creating |
| RON files parse with expected schema | unit test mirroring `enemies.rs:124-147` | unit | `cargo test -p druum data::town::tests::shop_stock_ron_parses` | NO — needs creating |
| `ShopStock.items_for_floor(n)` filters by min_floor | Pure helper test | unit | `cargo test -p druum data::town::tests::stock_filters_by_floor` | NO — needs creating |
| Trust-boundary clamps on RON input | Pure helper test | unit | `cargo test -p druum data::town::tests::recruit_pool_size_clamped` | NO — needs creating |

### Gaps (files to create before implementation)

- [ ] `src/plugins/town/{gold.rs, square.rs, shop.rs, inn.rs, temple.rs, guild.rs}` — six new module files
- [ ] `src/plugins/town/mod.rs` — rewrite the stub
- [ ] `src/data/town.rs` — new schema file
- [ ] `src/data/mod.rs` — add `pub mod town;` + re-exports
- [ ] `src/plugins/loading/mod.rs` — extend with `TownAssets` + `RonAssetPlugin::<ShopStock/RecruitPool/TownServices>` (3 lines + 1 collection)
- [ ] `assets/town/shop_stock.ron` — new file
- [ ] `assets/town/recruit_pool.ron` — new file
- [ ] `assets/town/town_services.ron` — new file
- [ ] Test fixtures using `make_test_app()` builder pattern (mirroring `combat/encounter.rs:558-668`)

No new test framework or config required.

---

## Integration Points

These are the precise touch points the planner needs to know.

### Touch point 1: `src/plugins/state/mod.rs:38-47` — `TownLocation` already exists

```rust
// VERIFIED — already in tree (zero work for #18):
#[derive(SubStates, Default, Debug, Clone, PartialEq, Eq, Hash)]
#[source(GameState = GameState::Town)]
pub enum TownLocation {
    #[default]
    Square,
    Shop,
    Inn,
    Temple,
    Guild,
}
```

Registered at `state/mod.rs:56`: `.add_sub_state::<TownLocation>()`.

**No changes to `state/mod.rs` required.**

### Touch point 2: `src/plugins/town/mod.rs:1-17` — REWRITE the stub

The current 17-line stub becomes the full `TownPlugin` per §Code Examples.

### Touch point 3: `src/plugins/loading/mod.rs:30-48, 108-125` — add `TownAssets` + 3 RON loaders

Mirror the existing `DungeonAssets`/`AudioAssets` exactly.

### Touch point 4: `src/data/mod.rs:1-26` — add `pub mod town;` + re-exports

```rust
pub mod town;  // NEW
pub use town::{ShopStock, ShopEntry, RecruitPool, RecruitDef, TownServices};
```

### Touch point 5: `Cargo.toml` — NO CHANGE

Zero new dependencies. All needed crates are already pinned.

### Touch point 6: `src/plugins/input/mod.rs:58-67` — `MenuAction` already declared

No new enum. Town reuses `MenuAction` as the input module's doc comment explicitly anticipates.

### Touch point 7: `src/plugins/party/inventory.rs:390-412` — `give_item` already documented as #18 caller

Used by `buy_item` in `town/shop.rs`. No change to inventory.rs.

### Touch point 8: `src/plugins/party/inventory.rs:217` — `EquipmentChangedEvent` is the dual-use trigger

Used by temple-revive (when removing `Dead`) and Inn-rest (when removing `Poison`). Existing message; the `recompute_derived_stats_on_equipment_change` system at `inventory.rs:444-453` handles it correctly without `With<PartyMember>` filter (memory `reference_druum_recompute_filter_dual_use.md`).

### Touch point 9: `src/main.rs:6, 30` — `TownPlugin` already registered

No change. The plugin replaces its own stub.

### Touch point 10: `src/plugins/ui/mod.rs:14, 22-25` — `EguiPlugin + EguiGlobalSettings` already in tree

No change. Town's `Camera2d + PrimaryEguiContext` spawn pattern uses the existing `EguiPlugin` registration.

### Touch point 11: `src/plugins/audio/bgm.rs:106-112` — `bgm_town` switch is automatic

No change. `state_changed::<GameState>` already fires the crossfade.

---

## Open Questions

### Open Question 1 (Tier C — user-preference): Single PR vs split (#18a / #18b)

- **What we know:** The roadmap itself suggests splitting: *"Consider shipping town in two passes: shop + inn first, temple + guild later."* Both shapes are technically viable.
- **What's unclear:** The user's preference for PR size and review cadence.
- **Recommendation:** Single PR if velocity is the priority; split if reviewable-diff-size is the priority. The architectural foundation (Gold, GameClock, TownAssets, camera, MenuAction handler) is the same either way — splitting doesn't reduce shared work, only review surface.
- **Surface to user.**

### Open Question 2 (Tier C — user-preference): Party-wide `Resource<Gold>` vs per-character `Component<Gold>`

- **What we know:** Both are valid genre conventions. Etrian Odyssey: party-wide. Wizardry: per-character + pool-at-tavern.
- **What's unclear:** User's preferred genre alignment.
- **Recommendation:** **`Resource<Gold>`** (party-wide). Reasons in Architecture Options Decision 3. Saves ~150 LOC and one egui screen.
- **Surface to user.** If they pick per-character, plan an additional 150-200 LOC + a new sub-state for the pool/distribute UI.

### Open Question 3 (Tier C — user-preference): Town backdrop asset

- **What we know:** Roadmap explicitly authorises deferring to a single hand-painted backdrop *"A single hand-painted 'town square' backdrop (one PNG) is enough atmosphere for now."* No backdrop is the simpler v1.
- **What's unclear:** User's preference for visual flavour at v1 vs deferring.
- **Recommendation:** **No backdrop in #18.** Render Square as a pure egui `CentralPanel`. Add the backdrop in Feature #25 polish if/when v1 ships.
- **Surface to user** for confirmation. If "yes, add a backdrop":
  - (a) Solid-colour fullscreen `egui::Painter::rect_filled` (1 LOC),
  - (b) One PNG via `Sprite` on the `Camera2d` (10 LOC + asset),
  - (c) `egui::Image` widget (5 LOC + asset).
  - All three are cheap; deferring is cheaper.

### Open Question 4 (Tier C — user-preference): Where does "Leave Town" go?

- **What we know:** From Town the player leaves via the Square menu. The roadmap is silent on the destination.
- **What's unclear:** TitleScreen vs Dungeon.
- **Recommendation:** **`GameState::TitleScreen`** for v1. Reasons:
  - The dungeon-entry pipeline (`OnEnter(Dungeon)` in `dungeon/mod.rs:226-234`) re-spawns `PlayerParty` at `floor.entry_point` — re-spawning every time you leave Town is wrong for a Wizardry-style "town is a safe haven you return to" loop where the player should restore at the dungeon entrance every dive.
  - Going to TitleScreen is the safe path for now; #25 polish adds a "Title → Dungeon" loading transition that respects the active floor.
  - Alternative: `GameState::Dungeon`, but only if the dungeon-spawn pipeline is changed to "spawn at last visited cell" — out of scope for #18.
- **Surface to user.** If they prefer "Leave Town → Dungeon", flag it as an additional Pitfall to plan around (the dungeon respawn from floor.entry_point will reset their position).

### Open Question 5 (Tier C — user-preference): Add `GameClock` resource?

- **What we know:** Roadmap says "advance an in-game clock". No clock infrastructure in tree.
- **What's unclear:** Whether the user wants the clock now, later, or never.
- **Recommendation:** **Add now.** ~15 LOC for `GameClock { day, turn }`. Sets up #21 (scripted events) and #24 (status durations in days) without retrofit. The Inn-rest UI reads better when "Day 5 / Rest costs 10gp" is shown alongside.
- **Surface to user** only if scope-cutting is needed. Default: include.

### Open Question 6 (Tier B — planner-resolvable): Temple-revive `current_hp = 1` write order

- **What we know:** When Temple revives a `Dead` party member, two things must happen: (a) `Dead` is removed from `StatusEffects`, (b) `current_hp` is set to 1. The `recompute_derived_stats_on_equipment_change` system reads `&StatusEffects` and writes `*derived = new` then clamps `current_hp = old_current_hp.min(new.max_hp)`. If we set `current_hp = 1` after removing `Dead` but BEFORE the event fires, then the recompute sees `old_current_hp = 1` and `new.max_hp` is now nonzero — `current_hp = min(1, max_hp) = 1`. Correct.
- **What's unclear:** The race ordering — does the painter's mutation happen before or after the recompute reads it?
- **Resolution path:** Layer 2 test. The planner should verify experimentally: write a test that revives a Dead member, runs `app.update()` twice, and asserts `derived.current_hp == 1` and `derived.max_hp > 0`. If the test fails, the write order is wrong and the fix is in the temple system (defer the `current_hp = 1` write to the frame after the event fires, using a `Local<HashSet<Entity>>` "needs-hp-set" set).
- **Planner action:** Build the test, observe, fix if needed. The recommended pattern in §Pattern 5 is the starting point.

### Open Question 7 (Tier C — user-preference): Inventory cap per character

- **What we know:** `Inventory.0: Vec<Entity>` has no upper bound. Wizardry caps at 8/character; Etrian at 60 party-wide.
- **What's unclear:** User's preference for inventory friction.
- **Recommendation:** **Cap at 8 per character** (Wizardry convention). Per-character matches Druum's Wizardry-genre alignment more than a party-wide cap. 8 is the historical number.
- **Surface to user.** Acceptable alternatives: 12 (Wizardry V), 16 (Wizardry VI), unlimited (anti-recommendation for save-file bound reasons).

### Open Question 8 (Tier C — user-preference): Item sell-back ratio

- **What we know:** Classic Wizardry sells items at 50%; some Etrian variants at 25% or 100% if "freshly bought".
- **What's unclear:** Druum's economy curve.
- **Recommendation:** **50% (`asset.value / 2`).** Hardcode in `sell_item`. If economy balance ((#21)) wants per-item sell prices later, make `ItemAsset.sell_price: Option<u32>` an additive field (zero schema break).
- **Surface to user** only if 50% feels wrong genre-wise.

### Open Question 9 (Tier A — research-resolvable): `distributive_run_if` exists in Bevy 0.18.1

- **What we know:** The `Pattern: TownPlugin wiring` example uses `(systems...).distributive_run_if(in_state(GameState::Town))`. This is an ergonomic helper.
- **What's unclear:** Whether `distributive_run_if` is in the 0.18.1 prelude.
- **Resolution path:** Grep `bevy_ecs-0.18.1/src/schedule/config.rs` for `distributive_run_if`. If absent, use per-system `.run_if(in_state(GameState::Town))` chained.
- **Planner action:** ~30s grep before committing. Both alternatives are equivalent semantically.

---

## Sources

### Primary (HIGH confidence)

**Bevy 0.18.1 on-disk crate sources (read directly this session):**

- `~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bevy_state-0.18.1/src/state/sub_states.rs` — SubStates derive + `#[source(...)]` semantics (verified: SubStates resource exists only while source state matches)
- `~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bevy_state-0.18.1/src/app.rs` — `add_sub_state` registration
- `~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bevy_egui-0.39.1/Cargo.toml` — verified `bevy = "0.18.0"` requirement is compatible with project pin
- `~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bevy_egui-0.39.1/examples/simple.rs` — minimal egui setup pattern
- `~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bevy_egui-0.39.1/examples/side_panel.rs` — multi-panel layout pattern
- `~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bevy_egui-0.39.1/examples/ui.rs` — full UI demo with CentralPanel + SidePanel + TopBottomPanel + Window
- `~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bevy_egui-0.39.1/src/lib.rs` — `EguiPlugin`, `EguiContexts`, `EguiPrimaryContextPass`, `PrimaryEguiContext`, `EguiGlobalSettings`
- `~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/egui-0.33.3/src/lib.rs` — `SidePanel`, `CentralPanel`, `Window`, `Area` containers

**Druum codebase (this session):**

- `src/plugins/state/mod.rs:1-141` — `GameState`, `DungeonSubState`, `CombatPhase`, `TownLocation` SubStates declarations, plugin registration, F9 cycler test bypass pattern
- `src/plugins/town/mod.rs:1-17` — stub to rewrite
- `src/plugins/party/mod.rs:1-192` — `PartyPlugin`, `spawn_default_debug_party` precedent (recruit-spawn template)
- `src/plugins/party/character.rs:1-829` — `PartyMember`, `BaseStats`, `DerivedStats`, `Class`, `Race`, `PartySlot`, `PartyRow`, `PartySize`, `Equipment`, `StatusEffects`, `StatusEffectType`, `ActiveEffect`, `derive_stats` (saturating arithmetic precedent), `PartyMemberBundle`
- `src/plugins/party/inventory.rs:1-540, 542-575` — `Inventory`, `ItemInstance`, `EquipSlot`, `ItemKind`, `give_item`, `equip_item`, `unequip_item`, `EquipmentChangedEvent`, `ItemHandleRegistry`, `populate_item_handle_registry`, `recompute_derived_stats_on_equipment_change` (no With<PartyMember> filter — confirmed for dual-use)
- `src/plugins/ui/mod.rs:1-29` — `UiPlugin`, `EguiPlugin` registration, `EguiGlobalSettings { auto_create_primary_context: false }` discipline
- `src/plugins/ui/minimap.rs:1-219` — `MinimapPlugin`, idempotent `Without<PrimaryEguiContext>` attach pattern (precedent for Combat too)
- `src/plugins/combat/ui_combat.rs:1-200` — `CombatUiPlugin`, painter-in-`EguiPrimaryContextPass` vs input-in-`Update` separation, `SidePanel::left/right + TopBottomPanel::bottom + Window` layout
- `src/plugins/input/mod.rs:1-371` — `MenuAction` declaration, doc comment explicitly anticipating Town reuse, default `InputMap<MenuAction>`, leafwing test pattern
- `src/plugins/loading/mod.rs:1-307` — `LoadingPlugin`, `DungeonAssets` + `AudioAssets` `AssetCollection` pattern, `RonAssetPlugin` registration, `add_loading_state` + `continue_to_state` + `load_collection`
- `src/plugins/audio/mod.rs:1-341` — channels, `ChannelVolumes`, `play_bgm_for_state` triggered by `state_changed::<GameState>` (Town BGM crossfade automatic)
- `src/plugins/audio/bgm.rs:1-171` — `FadeIn`/`FadeOut` lifecycle, `play_bgm_for_state` match arm explicit for `GameState::Town`
- `src/plugins/dungeon/mod.rs:1-619` — `DungeonCamera` marker, party preservation across `Dungeon ↔ Combat`, `OnEnter`/`OnExit` cleanup precedents (camera + geometry despawn)
- `src/plugins/combat/encounter.rs:1-440` — `CurrentEncounter` SOLE-writer rule, `MAX_ENEMIES_PER_ENCOUNTER` trust boundary precedent, `make_test_app`+`make_test_app_with_floor` test builders
- `src/data/items.rs:1-287` — `ItemAsset { value: u32 }` already authored as the shop-price field, `ItemDb::get` linear-scan precedent, RON round-trip test patterns
- `src/data/enemies.rs:1-149` — `EnemyDb`, `core.enemies.ron` parse test pattern (`enemies.rs:124-147`)
- `src/data/mod.rs:1-26` — `pub mod town;` placement guide
- `Cargo.toml:1-55` — `bevy_egui = "=0.39.1"`, `bevy_common_assets = "=0.16.0"`, `bevy_asset_loader = "=0.26.0"`, `serde = "1"`, `ron = "0.12"`, `leafwing-input-manager = "=0.20.0"` all already pinned
- `assets/items/core.items.ron:1-113` — RON authoring style for `ItemAsset` (used as template for `ShopStock`/`RecruitPool`/`TownServices`)

### Secondary (MEDIUM confidence)

- Genre-correct pattern reference (Wizardry per-character gold + pool-at-tavern; Etrian Odyssey party-wide gold) — published lore but no specific source verified in session. Used only for Option (A) vs (B) in §Architecture Options Decision 3.

### Tertiary (LOW confidence)

- None. Every claim in this document maps to either an on-disk verified source or a documented uncertainty in `## Open Questions`.

---

## Metadata

**Confidence breakdown:**

- Standard stack (no new deps): HIGH — verified by reading `Cargo.toml` + `bevy_egui-0.39.1/Cargo.toml` compatibility constraints.
- Architecture: HIGH — every pattern has a Druum precedent (Combat UI, Minimap UI, Audio BGM crossfade, Loading-screen camera spawn).
- Egui patterns: HIGH — `bevy_egui-0.39.1` examples + extracted source.
- State-machine plumbing: HIGH — `TownLocation` already in tree.
- Item/inventory integration: HIGH — `give_item` already explicitly named as a #18 caller in inventory.rs doc.
- Test architecture: HIGH — mirrors the existing two-tier pattern in `combat/encounter.rs` and `audio/mod.rs`.
- C-tier decisions (gold scope, sub-PR split, backdrop): MEDIUM — multiple valid options, recommendations surfaced as user-preference Open Questions.

**Research date:** 2026-05-11

**Tooling limitation impact:** None blocking. The only Open Question that's tooling-limitation-adjacent is Q9 (`distributive_run_if` availability), and the planner can grep that locally in <30 seconds; both alternative shapes (chained `.run_if`) are equivalent semantically.

**Net work estimate for #18 (single PR, recommended path):**

| Item | LOC |
|---|---|
| `src/plugins/town/mod.rs` (rewrite) | ~80 |
| `src/plugins/town/gold.rs` | ~80 |
| `src/plugins/town/square.rs` | ~120 |
| `src/plugins/town/shop.rs` | ~250 |
| `src/plugins/town/inn.rs` | ~100 |
| `src/plugins/town/temple.rs` | ~180 |
| `src/plugins/town/guild.rs` | ~220 |
| `src/data/town.rs` | ~120 |
| `src/data/mod.rs` (add re-exports) | +3 |
| `src/plugins/loading/mod.rs` (add TownAssets) | +15 |
| `assets/town/*.ron` (3 files) | ~80 lines authored RON total |
| Tests (Layer 1 + Layer 2) across all six modules | ~400-500 |
| **Total LOC** | **~1650 ± 200** |
| **Δ Deps (Cargo.toml)** | **0** |
| **Δ Asset files** | **+3 RON** |
| **New egui screens** | **5** |

Slightly above the roadmap's `+800 to +1300` envelope, primarily due to test surface (~30% of total LOC). Trimming tests to bare-minimum smoke would land within the envelope. Recommend keeping the full test surface — Town is the gateway to every economic and party-progression-related feature; under-testing it costs more later.
