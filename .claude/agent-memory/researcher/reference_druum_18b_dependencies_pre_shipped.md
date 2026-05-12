---
name: reference-druum-18b-dependencies-pre-shipped
description: Feature #18b (Town Temple + Guild) — what was pre-shipped by #18a; #18b is wiring + asset authoring + 2 new files + 1 file deletion, not architectural design
metadata:
  type: reference
---

Feature #18b (Town Temple + Guild) was carefully pre-built by Feature #18a. The implementer's task is overwhelmingly *additive* — almost every architectural surface is already in place.

## Pre-shipped by #18a (zero design work needed)

- `TownLocation::Temple` and `TownLocation::Guild` sub-states declared at `src/plugins/state/mod.rs:38-47` and registered via `.add_sub_state::<TownLocation>()` at line 56.
- `TownServices` asset type with `temple_revive_cost_base`, `temple_revive_cost_per_level`, `temple_cure_costs: Vec<(StatusEffectType, u32)>` fields pre-declared at `src/data/town.rs:155-167` with `#[serde(default)]` — adding values to `assets/town/core.town_services.ron` is the entire data task; no schema migration.
- `RecruitPool` + `RecruitDef` types pre-declared at `src/data/town.rs:107-127`. RON file `assets/town/core.recruit_pool.ron` already has 5 recruits authored — zero readers in #18a, #18b is the first consumer.
- `TownAssets` AssetCollection wires all three RON files at `src/plugins/loading/mod.rs:95-103` and `.load_collection::<TownAssets>()` at line 143.
- `RonAssetPlugin::<RecruitPool>` registered at `src/plugins/loading/mod.rs:131`.
- `Resource<Gold>` with `try_spend`/`earn` at `src/plugins/town/gold.rs:56-80`.
- `Resource<GameClock>` at `src/plugins/town/gold.rs:98-103`.
- Dev-only F4 grant-gold hotkey at `src/plugins/town/gold.rs:113-121`.
- `EquipmentChangedEvent` dual-use sentinel `EquipSlot::None` at `src/plugins/party/inventory.rs:216-220`.
- `recompute_derived_stats_on_equipment_change` is FILTER-FREE (`With<PartyMember>` was dropped in #15 D-A5) at `src/plugins/party/inventory.rs:444-501` — works on any entity with the recompute query shape; Temple needs no filter changes.
- `PartyMemberBundle` shape verified at `src/plugins/party/character.rs:320-333` — Recruit must call `commands.spawn(PartyMemberBundle { ... }).insert(Inventory::default())` (bundle excludes Inventory; precedent at `src/plugins/party/mod.rs:146`).
- `PartySize` resource (default 4) at `src/plugins/party/character.rs:344-351`.
- `MenuAction` enum already routed for Town (per `input/mod.rs:54-67` doc comment).

## #18b's actual work

- **CREATE** `src/plugins/town/temple.rs` (~250 LOC) — TempleState + paint_temple + handle_temple_action + pure helpers `revive_cost` / `cure_cost` + tests.
- **CREATE** `src/plugins/town/guild.rs` (~350 LOC) — GuildState + DismissedPool resource + paint_guild + handle_guild_recruit/dismiss/reorder/row_swap + tests.
- **DELETE** `src/plugins/town/placeholder.rs` (~144 LOC) — Temple/Guild routing now goes to real painters.
- **EDIT** `src/plugins/town/mod.rs` — replace placeholder painter/handler registrations with the new Temple/Guild systems; init `TempleState`, `GuildState`, `DismissedPool` resources.
- **EDIT** `assets/town/core.town_services.ron` — author concrete `temple_*` values (replace the implicit `#[serde(default)]` zeros).
- **EDIT** `src/data/town.rs` — add `MAX_TEMPLE_COST = 100_000` + `MAX_RECRUIT_POOL = 32` constants + `clamp_recruit_pool` trust-boundary helper + tests.

**Δ Cargo.toml = 0** — verified at `Cargo.toml:9-37`.

## Critical decision points

1. **Dismissed-pool shape (Category C):** Option A `Resource<DismissedPool { entities: Vec<Entity> }>` is recommended over component-marker (Option B) because Option B would require updating ~26 `With<PartyMember>` query sites across `src/plugins/{combat,dungeon,party,town}/*.rs`.
2. **Status effects Temple cures (Category C):** Recommended set is `Dead` (via Revive mode), `Stone`, `Paralysis`, `Sleep`. Inn already cures `Poison`. Buff variants are not cures.
3. **Revive formula:** `temple_revive_cost_base + temple_revive_cost_per_level * level` saturating, capped at `MAX_TEMPLE_COST`. The shape was pre-baked by the existing field names.

## Baseline test counts (#18a end-state)

- Default: 260 lib + 6 integration tests pass.
- `--features dev`: 264 lib + 6 integration tests pass.

#18b must keep both green and add new tests for Temple + Guild systems (target ~20 new tests).

## Anti-scope-creep markers

- Character creation (race/class/name picker) is Feature #19, NOT #18b.
- Save/load `MapEntities` for `DismissedPool` is Feature #23, NOT #18b — document with a `// Feature #23 must implement MapEntities` comment.
- Multi-status cure picker UI is Feature #25 polish — v1 auto-picks first eligible.
- `Camera2d`/`PrimaryEguiContext` lifecycle is already correct in `town/mod.rs:63-75` — DO NOT touch.
