# Pipeline State

**Task:** Feature #18a — Town Hub & Services (SPLIT: Square + Shop + Inn this PR; Temple + Guild deferred to #18b). Bevy 0.18.1 first-person dungeon-crawler RPG. Implement `GameState::Town` with sub-states `Square`, `Shop`, `Inn`. Square = pure-egui menu (no 3D backdrop). Shop = buy/sell items against party-wide `Resource<Gold>` (u32, saturating), 50% sell-back ratio, stock bounded by floor progression. Inn = rest party (full HP/MP heal, advance in-game clock, charge gold). Adds `Resource<GameClock>` (~15 LOC: day + turn counters). Inventory cap = 8 per character (Wizardry convention). "Leave Town" → `GameState::TitleScreen`. Temple + Guild explicitly deferred to follow-up #18b. Difficulty 3/5, mostly UI (heavy egui). Depends on #2 (party), #11 (inventory), #12 (status effects). Branch: `feature/18a-town-square-shop-inn`. Research: `project/research/20260511-feature-18-town-hub-and-services.md`.

**Status:** COMPLETE
**Last Completed Step:** 6 (summary)

## Artifacts

| Step | Description | Artifact                                 |
| ---- | ----------- | ---------------------------------------- |
| 1    | Research    | `project/research/20260511-feature-18-town-hub-and-services.md` |
| 2    | Plan        | `project/plans/20260511-180000-feature-18a-town-square-shop-inn.md` (Status: Complete) |
| 3    | Implement   | `project/implemented/20260511-190000-feature-18a-town-square-shop-inn.md` — all 6 quality gates GREEN (cargo check x2, cargo test x2 [260+6 / 264+6 pass], cargo clippy x2) |
| 4    | Ship        | `project/shipper/feature-18a-pr-body.md`; PR https://github.com/codeinaire/druum-dungeon-crawler/pull/18 open on branch `feature/18a-town-square-shop-inn` (commit `3486971`) |
| 5    | Code Review | `project/reviews/20260511-215220-feature-18a-town-square-shop-inn-pr-review.md` — Verdict APPROVE (1 MEDIUM, 1 LOW; non-blocking) |
| 6    | Summary     | `project/orchestrator/20260511-215508-feature-18a-town-square-shop-inn.md` |

## Key research findings

- **Δ deps = 0** — `bevy_egui = 0.39.1` already present; verified on disk against `bevy = 0.18.1`.
- **`TownLocation` SubStates already declared + registered** at `src/plugins/state/mod.rs:38-56` with all five variants. The roadmap text is stale on this.
- **`ItemAsset.value: u32` already documented as "#18 shop price"** at `src/data/items.rs:101-103`. No item-schema change required.
- **BGM crossfade for `GameState::Town` already wired** at `src/plugins/audio/bgm.rs:106-112`. `bgm_town` handle already loaded.
- **`MenuAction` already documented as "Town reuses this enum in v1"** at `src/plugins/input/mod.rs:54-67`.
- **`EquipmentChangedEvent` is the dual-use stat-changed trigger** — Temple revive/cure fires this to re-derive stats via existing recompute system (Temple is in #18b, but pattern noted).
- Primary recommendation (full #18): single PR with all five screens at minimum-viable depth.
- Scope decision: user chose **alternative split** — #18a = Square+Shop+Inn this PR; #18b = Temple+Guild follow-up.

## User Decisions

| # | Decision | Resolved value |
|---|----------|----------------|
| 1 | PR scope | **ALTERNATIVE** — Split: #18a (Square + Shop + Inn) this PR; Temple + Guild deferred to #18b |
| 2 | Gold model | Party-wide `Resource<Gold>` (u32, saturating) |
| 3 | Town backdrop | None — pure egui ("the square is a menu, not a level") |
| 4 | "Leave Town" destination | `GameState::TitleScreen` |
| 5 | `GameClock` | Add now (~15 LOC: day + turn counters) |
| 6 | Inventory cap | 8 per character (Wizardry convention) |
| 7 | Sell-back ratio | 50% (`value / 2`) |

## Quality Gate Verification (2026-05-11)

All six gates run from the top-level conversation, ALL GREEN:

| Gate | Command | Result |
|------|---------|--------|
| 1 | `cargo check` | exit 0 |
| 2 | `cargo check --features dev` | exit 0 |
| 3 | `cargo test` | 260 lib + 6 integration tests pass |
| 4 | `cargo test --features dev` | 264 lib + 6 integration tests pass |
| 5 | `cargo clippy --all-targets -- -D warnings` | exit 0 |
| 6 | `cargo clippy --all-targets --features dev -- -D warnings` | exit 0 |

Six fix-ups applied during verification (documented in implementation summary):
1. `src/plugins/town/shop.rs:542` — import path correction for `ItemKind`
2. `src/plugins/town/mod.rs:247,264` — `.world_mut()` for mutable query access
3. `src/plugins/town/mod.rs:166-168` — `InputManagerPlugin` removed from test app
4. `src/plugins/town/inn.rs` `handle_inn_rest` — `#[allow(clippy::too_many_arguments)]`
5. `src/plugins/town/shop.rs` `handle_shop_input` — `#[allow(clippy::too_many_arguments)]`
6. `src/plugins/town/shop.rs:510` — `clippy::erasing_op` workaround
