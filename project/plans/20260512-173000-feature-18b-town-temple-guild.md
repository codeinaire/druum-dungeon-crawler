# Plan: Feature #18b — Town Hub: Temple + Guild

**Date:** 2026-05-12
**Status:** Draft
**Research:** project/research/20260512-feature-18b-town-temple-guild.md
**Depends on:** 20260511-180000-feature-18a-town-square-shop-inn.md

## Goal

Ship the two Town sub-states deferred from #18a as real screens: **Temple** (revive Dead for `base + per_level * level` gold; cure Stone/Paralysis/Sleep for flat per-status gold) and **Guild** (party roster: recruit from `core.recruit_pool.ron`, dismiss to `Resource<DismissedPool>`, swap two members' `PartySlot` values, toggle `PartyRow::Front`/`Back`). Delete `src/plugins/town/placeholder.rs` and re-wire `TownPlugin` to dispatch its Temple/Guild routes to the new modules. Δ Cargo.toml = 0. No new assets beyond authoring the pre-declared `temple_*` fields in `assets/town/core.town_services.ron`.

## Approach

The Temple+Guild work was deliberately scoped out of #18a; the foundation was authored eagerly so this PR is overwhelmingly *additive*. `TownLocation::Temple` and `TownLocation::Guild` are already declared in `src/plugins/state/mod.rs:38-47`. The `TownServices` schema at `src/data/town.rs:155-167` has `temple_revive_cost_base`, `temple_revive_cost_per_level`, and `temple_cure_costs` pre-declared with `#[serde(default)]` — #18b fills in the values without any schema migration. `RecruitPool` at `src/data/town.rs:107-127` and its 5-entry RON file at `assets/town/core.recruit_pool.ron` are already authored.

**Temple** mirrors `inn.rs`'s painter/handler split (same `EguiPrimaryContextPass` + `Update` schedule placement, same `Camera2d` lifecycle), with two modes — Revive and Cure. Both modes use `effects.retain(|e| e.effect_type != target)` as their sole mutation of `StatusEffects` (the documented exception path from `apply_status_handler`, mirroring Inn's pattern at `inn.rs:153-162`). After mutation, both fire `EquipmentChangedEvent { slot: EquipSlot::None }` so the filter-free `recompute_derived_stats_on_equipment_change` at `inventory.rs:444` re-derives stats. Revive additionally writes `current_hp = 1` *before* the event — the caller-clamp at `inventory.rs:495-499` then preserves that 1 against the re-derived max. Cure auto-picks the first eligible severe status in priority order Stone > Paralysis > Sleep (matches Inn's simplicity).

**Guild** uses `Resource<DismissedPool>` (research §Architecture Options, Option A) — 0 churn to existing `With<PartyMember>` queries (26 sites identified in research). Dismiss removes the `PartyMember` marker via `Commands::entity(e).remove::<PartyMember>()` (deferred — Pitfall 2) and pushes the entity into `pool.entities`. Re-recruit-from-pool pops the entity, re-adds `PartyMember` + a fresh `PartySlot`. Recruit-from-`RecruitPool` spawns via `PartyMemberBundle` (`character.rs:316-333`) + chained `.insert(Inventory::default())` — exact same shape as `spawn_default_debug_party` at `party/mod.rs:131-147`. Slot reorder is SWAP semantics (per user decision 5): two `PartySlot` writes exchange values between two `With<PartyMember>` entities, type-safe and Wizardry-natural. Row swap toggles `PartyRow::Front` ↔ `PartyRow::Back` on the cursor target.

Both screens reuse the existing `TownCameraRoot` `Camera2d` + `PrimaryEguiContext` setup at `town/mod.rs:63-66` — no new camera, no `Camera3d`, pure egui. `Gold::try_spend` returns `Result` for all charges; saturating arithmetic is defense-in-depth. The six quality gates from #18a remain the bar.

## Critical

- **Painter purity:** `paint_temple` and `paint_guild` MUST be read-only (`Res<T>` / `Local<T>` only — no `ResMut<T>` / `Commands`). All mutations live in handler systems in `Update`. (Research Pattern 3 + #18a Critical.)
- **`apply_status_handler` is NOT in the call path:** Temple's revive/cure mutates `StatusEffects.effects` directly via `effects.retain` — this is a documented exception. Do NOT route through `apply_status_handler`; routing through it would re-trigger merge logic and only emit `EquipmentChangedEvent` for buff variants, missing Stone/Paralysis/Sleep. (Research Anti-Pattern + Pitfall.)
- **Revive sets `current_hp = 1` BEFORE firing `EquipmentChangedEvent`:** the caller-clamp at `inventory.rs:495-499` is `derived.current_hp = old_current_hp.min(derived.max_hp)`. If you fire the event first, `derive_stats` returns `current_hp = max_hp`, then the clamp `min(0, max_hp) = 0` zeros the player. Order: `effects.retain` → `current_hp = 1` → write event. (Research Pattern 1 + Pitfall 4.)
- **Cure mode MUST NOT touch `Dead`:** `cure_cost` helper filters `Dead` out of the eligibility lookup. Revive is the sole path that removes `Dead`. Cure on `Dead` would leave `current_hp = 0` and the entity would be Dead-by-HP even after the effect is cleared. (Research Pitfall 4.)
- **`Gold::try_spend` is the sole gold-deduction path:** never decrement `gold.0` directly. Check `gold.0 >= cost` BEFORE `try_spend` (saturating is defense-in-depth, not the guard). (Project precedent — Shop/Inn at #18a.)
- **Recruit must spawn via `PartyMemberBundle` + `.insert(Inventory::default())`:** the bundle deliberately omits `Inventory` (verified at `character.rs:320-333`), so recruits need it chained. Without `Inventory`, downstream `Query<&mut Inventory, With<PartyMember>>` will not match the recruit and items cannot be added. (Research Pitfall 5.)
- **Dismiss must NOT despawn the entity:** Option A's whole correctness rests on the entity living on after `PartyMember` is removed. Despawning would orphan `Inventory.0`'s `ItemInstance` entities (no GC; they leak). The handler `commands.entity(target).remove::<PartyMember>()` and pushes `target` to `pool.entities`. (Research Anti-Pattern + Security risk "Dismissed entity Inventory persistence".)
- **Minimum-1-active applies to Dismiss ONLY:** Dismiss rejects if the active-party count is 1 (would empty the roster). Recruit has NO minimum check — recruiting from empty is allowed (per user decision 6, forward-compatible with #19 Character Creation). Document this asymmetry in a doc-comment on `handle_guild_dismiss`. (Research Open Q4.)
- **Same-frame `Commands` deferral is a real risk:** `commands.entity(e).remove::<PartyMember>()` does not take effect until `apply_deferred`. Each handler must `return` after queueing the mutation; do not count `Query<&PartyMember>::iter().count()` after queueing. Test: `dismiss_then_recruit_in_one_frame_does_not_double_count`. (Research Pitfall 2.)
- **`PartySlot` uniqueness is not type-enforced:** recruit's next-free-slot algorithm uses `let used: HashSet<usize> = party.iter().map(|s| s.0).collect(); (0..party_size.0).find(|i| !used.contains(i))`. Slot reorder is SWAP — two writes, never inserting a duplicate. (Research Pitfall 6.)
- **`temple_*` field zeros from `#[serde(default)]` would free-revive:** `assets/town/core.town_services.ron` currently omits these fields (they parse as 0). Step 1 authors the concrete values; defense-in-depth `cost.max(1)` in `revive_cost` guards against a future RON typo. (Research Pitfall 3.)
- **Zero new Cargo dependencies.** Δ deps = 0. Plan steps MUST NOT touch `Cargo.toml`.
- **No new RON files; no asset renaming.** `core.town_services.ron` already exists at the correct path with the correct double-dot extension (per project memory: Bevy parses the extension from the first dot; the file is registered as `town_services.ron` in `loading/mod.rs:132`). Step 1 only *modifies* the values inside it.

## Steps

### Phase 1 — Data: author Temple costs in the existing RON file

- [x] Edit `assets/town/core.town_services.ron`. After the existing `inn_rest_cost: 10,` + `inn_rest_cures: [Poison],` block (lines 5-6), add four lines:
  ```ron
      temple_revive_cost_base: 100,
      temple_revive_cost_per_level: 50,
      temple_cure_costs: [
          (Stone, 250),
          (Paralysis, 100),
          (Sleep, 50),
      ],
  ```
  Do NOT include `Dead` in `temple_cure_costs` — Revive owns Dead removal. Do NOT rename the file or change its extension. Verify the comment at line 3 ("Feature #18b reads those fields") matches reality after this edit; update if needed.
- [x] Edit `src/data/town.rs`. Add two new public constants in the trust-boundary section (after `MAX_INN_COST` at line 34): `pub const MAX_TEMPLE_COST: u32 = 100_000;` and `pub const MAX_RECRUIT_POOL: usize = 32;`. Add a doc-comment on each mirroring `MAX_INN_COST`'s style (one-paragraph justification + caller note). Also add a `pub fn clamp_recruit_pool(pool: &RecruitPool, max_recruits: usize) -> &[RecruitDef]` helper following the exact shape of `clamp_shop_stock` at `data/town.rs:94-97`. Add three unit tests in the existing `mod tests` block: `recruit_pool_size_clamped_truncates_oversized` (200 entries → 32), `clamp_recruit_pool_passes_through_small_pool` (5 entries → 5), and `town_services_round_trips_with_authored_temple_fields` (parses a RON literal with `temple_revive_cost_base: 100`, asserts non-zero post-parse, re-serializes and re-parses stably — mirror `town_services_ron_round_trips_with_defaulted_temple_fields` at `data/town.rs:314-340`).

### Phase 2 — Temple module

- [x] Create `src/plugins/town/temple.rs` with the following exports. The file mirrors `inn.rs` in structure (module-level doc-comment, painter, handler, tests). Top-of-file doc-comment must call out:
  - Temple cures Dead/Stone/Paralysis/Sleep ONLY; `Poison` is Inn-only.
  - Cure mode auto-picks first eligible severe status in priority order Stone > Paralysis > Sleep (per user decision 7; matches Inn's simplicity).
  - Revive writes `current_hp = 1` (Wizardry convention).
  - Multi-status cure picker UI is deferred to #25 polish.

  Declare resources and modes:
  ```rust
  #[derive(Resource, Default, Debug)]
  pub struct TempleState {
      pub mode: TempleMode,
      pub cursor: usize,
      pub party_target: usize,
  }
  #[derive(Debug, PartialEq, Eq, Clone, Copy, Default)]
  pub enum TempleMode { #[default] Revive, Cure }
  ```

  Declare two pure helpers (testable without an App):
  - `pub fn revive_cost(services: &TownServices, level: u32) -> u32` — `services.temple_revive_cost_base.saturating_add(services.temple_revive_cost_per_level.saturating_mul(level)).max(1).min(MAX_TEMPLE_COST)`. The `.max(1)` is the "free-revive" defense-in-depth guard from research Pitfall 3.
  - `pub fn cure_cost(services: &TownServices, kind: StatusEffectType) -> Option<u32>` — looks up in `services.temple_cure_costs`. Filters `Dead` out: `if kind == StatusEffectType::Dead { return None; }` as the first line. Returns `Some(cost.min(MAX_TEMPLE_COST))` or `None`.

  Declare a priority helper for multi-status cure:
  - `pub fn first_curable_status(services: &TownServices, status: &StatusEffects) -> Option<(StatusEffectType, u32)>` — checks the priority order Stone, Paralysis, Sleep (in that order); for each, if `status.has(kind)` AND `cure_cost(services, kind).is_some()`, return `(kind, cost)`. Returns `None` if no curable status is present.

  Implement `pub fn paint_temple(...) -> Result` (read-only): `TopBottomPanel::top("temple_header")` with title "Temple" left and `{gold.0} Gold` right (mirror `inn.rs:58-64`). `CentralPanel` shows the current mode (Revive / Cure), the cursor-targeted party member (by deterministic Entity sort, mirror `shop.rs:397`), and the computed cost for the current action ("Revive Aldric (lvl 3): 250g" or "Cure Stone: 250g" or "Nothing to cure"). Footer shows `[Tab] Switch mode  |  [Up/Down] Pick member  |  [Enter] Confirm  |  [Esc] Back`.

  Implement `pub fn handle_temple_action(...)` (Update, mutates). Signature follows `handle_inn_rest` at `inn.rs:106-116` plus `mut temple_state: ResMut<TempleState>`. Apply `#[allow(clippy::too_many_arguments)]` (precedent: `inn.rs:106`). On:
  - `MenuAction::Cancel` → `next_sub.set(TownLocation::Square)`, return.
  - `MenuAction::Up`/`Down` → bump `temple_state.party_target` with wrap-around against the active-party count (sort by Entity for determinism).
  - `MenuAction::SwitchMode` (or `Tab` if `MenuAction` lacks a switch-mode variant — verify via grep; if absent, reuse `Left`/`Right` like `shop.rs`'s buy/sell toggle) → toggle `temple_state.mode`.
  - `MenuAction::Confirm` → branch on `temple_state.mode`:
    - **Revive:** resolve target entity (sorted-by-Entity index `temple_state.party_target`). If `!status.has(StatusEffectType::Dead)` → `info!("Temple revive: target is not dead")`, return without charging. Compute `cost = revive_cost(services, xp.level)`. If `gold.0 < cost` → `info!("Temple revive: insufficient gold")`, return without charging. Otherwise: `status.effects.retain(|e| e.effect_type != StatusEffectType::Dead)`, `derived.current_hp = 1`, `writer.write(EquipmentChangedEvent { character: target, slot: EquipSlot::None })`, `let _ = gold.try_spend(cost)`, `next_sub.set(TownLocation::Square)`, `info!`-log success.
    - **Cure:** resolve target entity. Call `first_curable_status(services, status)`. If `None` → `info!("Temple cure: target has no curable status")`, return without charging. Bind `(kind, cost)`. If `gold.0 < cost` → `info!`-log + return. Otherwise: `status.effects.retain(|e| e.effect_type != kind)`, `writer.write(EquipmentChangedEvent { character: target, slot: EquipSlot::None })`, `let _ = gold.try_spend(cost)`, `next_sub.set(TownLocation::Square)`, `info!`-log success.

  Add `#[cfg(test)] mod tests`. Build `make_temple_test_app()` mirroring `inn.rs:198-247`: `MinimalPlugins + AssetPlugin + StatesPlugin`, `init_resource::<ActionState<MenuAction>>()`, `init_state::<GameState>()`, `add_sub_state::<TownLocation>()`, `init_asset::<TownServices>()`, insert a mock `TownAssets` with `temple_revive_cost_base: 100`, `temple_revive_cost_per_level: 50`, `temple_cure_costs: [(Stone, 250), (Paralysis, 100), (Sleep, 50)]`, `add_message::<EquipmentChangedEvent>()`, register `handle_temple_action` in `Update` gated on `TownLocation::Temple`, run 2× `app.update()` to land in Temple. Spawn party-member helper `spawn_party_member(app, hp, mp, level, effects)` (mirror `inn.rs:249-268`). Tests required:
  - `revive_cost_scales_linearly_with_level` — `revive_cost(services, 1) == 150`, `revive_cost(services, 5) == 350`, `revive_cost(services, 0) == 100` (formula `base + per_level * level`, base 100, per_level 50).
  - `revive_cost_saturates_at_max_temple_cost` — `temple_revive_cost_base = u32::MAX`, `temple_revive_cost_per_level = u32::MAX`, level 100 → returns `MAX_TEMPLE_COST`.
  - `revive_cost_guards_against_zero_via_max_1` — `temple_revive_cost_base = 0`, `temple_revive_cost_per_level = 0`, level 1 → returns 1 (not 0).
  - `cure_cost_returns_none_for_dead` — `cure_cost(services, StatusEffectType::Dead) == None` even if `(Dead, X)` is in `temple_cure_costs` (assert by constructing such a services value).
  - `cure_cost_returns_lookup_for_stone` — `cure_cost(services, StatusEffectType::Stone) == Some(250)`.
  - `first_curable_status_picks_stone_when_present` — entity with `[Stone, Sleep]` → returns `Some((Stone, 250))` (priority order).
  - `first_curable_status_picks_paralysis_when_only_paralysis_and_sleep` — entity with `[Paralysis, Sleep]` → `Some((Paralysis, 100))`.
  - `first_curable_status_returns_none_for_no_severe_status` — entity with `[Poison]` only → `None` (Poison is not in `temple_cure_costs`).
  - `revive_dead_member_clears_dead_and_sets_hp_to_1` — spawn dead member, `Gold(1000)`, Revive mode, press Confirm, run 2× update; assert `!status.has(Dead)`, `current_hp == 1`, `max_hp > 0` (re-derived from non-zero formula), `gold.0 == 1000 - 150` (level 1).
  - `revive_rejects_non_dead_target` — spawn living member, Revive mode, press Confirm; assert gold unchanged, target unchanged.
  - `revive_rejects_when_insufficient_gold` — `Gold(50)`, dead member level 5 (cost 350), Revive mode, press Confirm; assert `status.has(Dead)` still, `gold.0 == 50`.
  - `cure_stone_removes_status_and_deducts_gold` — spawn member with `[Stone]`, `Gold(500)`, Cure mode, press Confirm; assert `!status.has(Stone)`, `gold.0 == 250`.
  - `cure_mode_does_not_remove_dead` — spawn member with `[Dead]`, `Gold(1000)`, Cure mode, press Confirm; assert `status.has(Dead)` still, `gold.0 == 1000` (no charge).
  - `cure_rejects_when_insufficient_gold` — `Gold(100)` (Stone cost is 250), Cure mode, member with `[Stone]`, press Confirm; assert `status.has(Stone)` still, `gold.0 == 100`.
  - `cancel_returns_to_square` — press Cancel; assert sub-state is `TownLocation::Square`.

  (~280–320 LOC including tests.)

### Phase 3 — Guild module

- [x] Create `src/plugins/town/guild.rs`. Top-of-file doc-comment must call out:
  - Option A model: dismiss removes `PartyMember` marker; entity stays alive in `DismissedPool.entities`. Re-recruit reuses the entity (preserves XP/equipment/inventory).
  - Recruit allows empty active party (forward-compatible with #19); minimum-1-active applies to Dismiss ONLY.
  - Slot reorder is SWAP semantics (two-write exchange of `PartySlot` values).
  - `RecruitDef` does NOT include status effects — recruits spawn with `StatusEffects::default()`.
  - Dismissed entities are never despawned (despawning would orphan `Inventory.0` `ItemInstance` entities).

  Declare resources and modes:
  ```rust
  /// Dismissed-pool registry (research §Architecture Options Option A).
  ///
  /// **Feature #23 save/load contract:** `Vec<Entity>` does not naturally
  /// serialize across sessions. Feature #23 must implement `MapEntities` for
  /// this resource — same deferral contract as `Inventory(Vec<Entity>)` at
  /// `src/plugins/party/inventory.rs:179-181`. Do NOT derive
  /// `Serialize`/`Deserialize` in #18b.
  #[derive(Resource, Default, Debug)]
  pub struct DismissedPool {
      pub entities: Vec<Entity>,
  }

  #[derive(Resource, Default, Debug)]
  pub struct GuildState {
      pub mode: GuildMode,
      pub cursor: usize,
  }

  #[derive(Debug, PartialEq, Eq, Clone, Copy, Default)]
  pub enum GuildMode {
      #[default]
      Roster,  // browse active party — supports Dismiss, Row swap, Slot reorder
      Recruit, // browse RecruitPool entries — Confirm spawns a new PartyMember
  }
  ```

  Declare a pure helper for next-free-slot lookup (testable without an App):
  - `pub fn next_free_slot(used: &[usize], party_size: usize) -> Option<usize>` — returns the lowest unused `usize` in `0..party_size`, or `None` if all are used. Mirror the algorithm in research §Pattern 2.

  Implement `pub fn paint_guild(...) -> Result` (read-only): `TopBottomPanel::top("guild_header")` with title "Guild" left, `{gold.0} Gold  |  Day {clock.day}` right. `CentralPanel` switches on `guild_state.mode`:
  - **Roster:** list active party members (sorted by `PartySlot` ascending, with Entity as tiebreaker for determinism). Cursor highlight. Show name, race, class, level, `PartySlot`, `PartyRow`, brief status (e.g., "Dead" if `status.has(Dead)`). Empty-party fallback: `ui.label("(No active members — press R to recruit)")`. Footer: `[Up/Down] Pick  |  [D] Dismiss  |  [F] Toggle Row  |  [S] Slot Swap target  |  [R] Recruit mode  |  [Esc] Back`.
  - **Recruit:** list `RecruitPool.recruits` clamped to `MAX_RECRUIT_POOL = 32` via `clamp_recruit_pool` (Phase 1). Each row: name, race, class, base-stat preview. Cursor highlight. Empty/uncapped pool fallback. Footer: `[Up/Down] Pick  |  [Enter] Recruit  |  [R] Back to Roster  |  [Esc] Back`.

  Implement five handler systems (all in `Update`):

  1. `pub fn handle_guild_input(...)` — top-level navigation (`Cancel` → Square; `Up`/`Down` → cursor; `R` → toggle Recruit/Roster mode via direct `Res<ButtonInput<KeyCode>>` since `MenuAction` has no per-screen mode-switch verb — verify by grep; if `MenuAction::SwitchMode` exists reuse it).
  2. `pub fn handle_guild_recruit(mut commands, guild_state, town_assets, pool_assets, party_size, existing_slots: Query<&PartySlot, With<PartyMember>>, ...)` — gated on `guild_state.mode == GuildMode::Recruit`. On `MenuAction::Confirm`:
     - Resolve `RecruitPool` via `Option<Res<TownAssets>>` + `Res<Assets<RecruitPool>>`.
     - Apply `clamp_recruit_pool(&pool, MAX_RECRUIT_POOL)` and pick `recruits[guild_state.cursor]`.
     - Active-party-full guard: `existing_slots.iter().count() >= party_size.0` → `info!("Guild recruit: party full")`, return. NO minimum-active guard.
     - Compute next free slot: `let used: Vec<usize> = existing_slots.iter().map(|s| s.0).collect()`; `let slot = next_free_slot(&used, party_size.0).unwrap_or(0)` (the `unwrap_or` is defensive — should never trigger due to the party-full guard above).
     - Compute level-1 derived stats: `let derived = derive_stats(&recruit.base_stats, &[], &StatusEffects::default(), 1)`.
     - Spawn: `commands.spawn(PartyMemberBundle { name: CharacterName(recruit.name.clone()), race: recruit.race, class: recruit.class, base_stats: recruit.base_stats, derived_stats: derived, party_row: recruit.default_row, party_slot: PartySlot(slot), ..Default::default() }).insert(Inventory::default())`.
     - Set `guild_state.mode = GuildMode::Roster` (UX: jump back to roster after recruit).
     - Do NOT consume gold (recruit is free in v1 — no `recruit_cost` field in `TownServices`).
     - `info!`-log.
  3. `pub fn handle_guild_dismiss(mut commands, mut pool: ResMut<DismissedPool>, guild_state, party: Query<(Entity, &PartySlot), With<PartyMember>>, keys: Res<ButtonInput<KeyCode>>)` — gated on `guild_state.mode == GuildMode::Roster`. On `KeyCode::KeyD` (direct `ButtonInput` since `MenuAction` does not include a Dismiss verb — verify):
     - Resolve target by sorting `party.iter()` by `(PartySlot, Entity)` ascending; pick entry at `guild_state.cursor`. (Sorting by slot keeps the cursor index aligned with the painter's listing order; tiebreaker by Entity for determinism.)
     - Minimum-1-active guard: `let active_count = party.iter().count(); if active_count <= 1 { info!("Guild dismiss: cannot dismiss the last active member"); return; }`.
     - `commands.entity(target).remove::<PartyMember>()` (deferred — Pitfall 2; do not perform further state mutations same frame).
     - `pool.entities.push(target)`.
     - `info!("Guild: dismissed {:?}", target)`.
     - Document the asymmetry: "Recruit has no minimum-active check (forward-compat with #19); dismiss requires count > 1 to prevent emptying the roster."
  4. `pub fn handle_guild_row_swap(guild_state, mut party: Query<(Entity, &PartySlot, &mut PartyRow), With<PartyMember>>, keys: Res<ButtonInput<KeyCode>>)` — gated on `guild_state.mode == GuildMode::Roster`. On `KeyCode::KeyF`:
     - Resolve target the same way as Dismiss (sort by `(PartySlot, Entity)`, pick `guild_state.cursor`).
     - Toggle: `*row = match *row { PartyRow::Front => PartyRow::Back, PartyRow::Back => PartyRow::Front }`.
     - `info!`-log.
  5. `pub fn handle_guild_slot_swap(mut guild_state: ResMut<GuildState>, mut party: Query<(Entity, &mut PartySlot), With<PartyMember>>, keys: Res<ButtonInput<KeyCode>>)` — two-press SWAP UX (per user decision 5):
     - Maintain a `Local<Option<Entity>>` slot-swap-source pin.
     - On `KeyCode::KeyS`: if `pin.is_none()`, pin the current cursor's target Entity; else compute the cursor's current target, perform the swap (read both `PartySlot` values, write them flipped), clear the pin. `info!`-log both events.
     - Do NOT collapse this into one keypress — the two-press shape is the simplest UX without a drag-and-drop layer.

  Plugin wiring is in Phase 5.

  Add `#[cfg(test)] mod tests`. Build `make_guild_test_app()` mirroring `inn.rs:198-247` but also `init_resource::<DismissedPool>()`, `init_resource::<GuildState>()`, `init_asset::<RecruitPool>()`, insert a mock `TownAssets` whose `recruit_pool` handle points to a `RecruitPool` with 5 authored test recruits (mirror `core.recruit_pool.ron`), `init_resource::<PartySize>()`. Register all 5 handlers in `Update` with their respective `GuildMode` gates. Helper `spawn_active_member(app, slot, row)` and `spawn_dismissed_member(app, slot, row)` (the latter spawns without `PartyMember`, adds entity to `DismissedPool.entities`). Tests required:
  - `next_free_slot_picks_lowest_unused` — `next_free_slot(&[0, 2], 4) == Some(1)`.
  - `next_free_slot_returns_none_when_full` — `next_free_slot(&[0, 1, 2, 3], 4) == None`.
  - `next_free_slot_handles_empty_party` — `next_free_slot(&[], 4) == Some(0)`.
  - `recruit_spawns_party_member_with_correct_bundle_fields` — Recruit mode, cursor=0, press Confirm; assert exactly one new `PartyMember` exists with matching name/race/class from the test pool's entry 0.
  - `recruit_picks_lowest_free_slot_after_dismissal` — spawn 4 members at slots 0..=3, dismiss slot 1, recruit; assert the new member has `PartySlot(1)`.
  - `recruit_rejects_when_party_full` — spawn 4 members (`PartySize::default() = 4`), recruit; assert no 5th member spawned and `info!` was logged. (Use `existing.iter().count()` post-update.)
  - `recruit_allows_empty_party` — `PartySize(4)`, 0 active members, recruit cursor=0, Confirm; assert 1 member spawned (no minimum-1-active gate on Recruit).
  - `recruit_attaches_empty_inventory_component` — recruit; assert the new member entity has `Inventory` with `inventory.0.is_empty()`.
  - `dismiss_removes_partymember_marker` — spawn 2 members, Dismiss cursor=0, press D, run 2× update (deferred command); assert exactly 1 `PartyMember` remains.
  - `dismiss_adds_entity_to_pool` — spawn 2, dismiss cursor=0, run 2× update; assert `pool.entities.len() == 1`.
  - `dismiss_preserves_inventory_entities` — spawn 2 members with `Inventory.0 = vec![spawn_entity_a, spawn_entity_b]` on member 0, dismiss member 0, assert `Inventory.0` on the dismissed entity still contains both entries (entity still alive, components retained).
  - `dismiss_rejects_last_active_member` — spawn 1 member, dismiss, run 2× update; assert `PartyMember` still attached, `pool.entities.is_empty()`.
  - `dismiss_then_recruit_in_one_frame_restores_count` — spawn 4 members, dismiss member 0, then recruit from pool[0] within the same `app.update()` cycle (`but cargo test` simulating two handler invocations in one update); after settling, assert `Query<&PartyMember>::iter().count() == 4` and the dismissed entity is in `pool.entities` (recruit spawns a NEW entity, not re-using the dismissed one — re-use is a future polish, out of scope for #18b).
  - `row_swap_toggles_front_to_back_and_back_to_front` — spawn member with `PartyRow::Front`, press F twice; assert `Front → Back → Front`.
  - `slot_swap_exchanges_two_members_slots` — spawn members `(slot=0, slot=2)`, slot-swap pin to first, then slot-swap target to second; assert post-swap slots are `(0→2)` and `(2→0)`, total `PartyMember` count unchanged.
  - `cancel_returns_to_square` — press Cancel; assert sub-state is `TownLocation::Square`.

  (~380–430 LOC including tests.)

### Phase 4 — Delete the placeholder

- [x] Delete `src/plugins/town/placeholder.rs` entirely (~144 LOC removed). (NOTE: file removed from module tree; physical deletion deferred — see Implementation Discoveries #1) Its 2 tests (`placeholder_cancel_returns_to_square_from_temple` and `placeholder_cancel_returns_to_square_from_guild`) are subsumed by `temple::tests::cancel_returns_to_square` and `guild::tests::cancel_returns_to_square` from Phases 2-3.

### Phase 5 — TownPlugin wiring

- [x] Edit `src/plugins/town/mod.rs`:
  - Replace `pub mod placeholder;` (line 30) with `pub mod guild;` + `pub mod temple;` (alphabetical between `inn` and `shop`). Resulting module order: `gold, guild, inn, shop, square, temple`.
  - Remove `use placeholder::{handle_placeholder_input, paint_placeholder};` (line 39). Add:
    ```rust
    use guild::{
        DismissedPool, GuildState,
        handle_guild_dismiss, handle_guild_input, handle_guild_recruit,
        handle_guild_row_swap, handle_guild_slot_swap, paint_guild,
    };
    use temple::{TempleState, handle_temple_action, paint_temple};
    ```
  - In `TownPlugin::build` Resources section (lines 86-90), append `.init_resource::<TempleState>().init_resource::<GuildState>().init_resource::<DismissedPool>()`.
  - In the painter tuple at lines 97-110: replace the line `paint_placeholder.run_if(in_state(TownLocation::Temple).or(in_state(TownLocation::Guild))),` with two lines:
    ```rust
    paint_temple.run_if(in_state(TownLocation::Temple)),
    paint_guild.run_if(in_state(TownLocation::Guild)),
    ```
  - In the input-handler tuple at lines 113-126: replace `handle_placeholder_input.run_if(...)` with:
    ```rust
    handle_temple_action.run_if(in_state(TownLocation::Temple)),
    handle_guild_input.run_if(in_state(TownLocation::Guild)),
    handle_guild_recruit.run_if(in_state(TownLocation::Guild)),
    handle_guild_dismiss.run_if(in_state(TownLocation::Guild)),
    handle_guild_row_swap.run_if(in_state(TownLocation::Guild)),
    handle_guild_slot_swap.run_if(in_state(TownLocation::Guild)),
    ```
    Preserve the surrounding `.distributive_run_if(in_state(GameState::Town))` on the tuple.
  - In the module-level doc-comment block (lines 1-23), strike the "placeholder Temple/Guild screens" wording and the `placeholder` bullet. Replace with: "Each Town sub-state (Square / Shop / Inn / Temple / Guild) has its own painter + handler module."
  - In the test module (lines 137-294), update `use crate::plugins::town::placeholder::handle_placeholder_input;` (line 149) to reference the new modules: replace with `use crate::plugins::town::guild::{handle_guild_dismiss, handle_guild_input, handle_guild_recruit, handle_guild_row_swap, handle_guild_slot_swap, GuildState, DismissedPool};` and `use crate::plugins::town::temple::{handle_temple_action, TempleState};`. In `make_town_test_app` (lines 159-228), append the new resources/registrations:
    ```rust
    app.init_resource::<TempleState>()
        .init_resource::<GuildState>()
        .init_resource::<DismissedPool>();
    ```
    Replace the `handle_placeholder_input.run_if(...)` registration (lines 222-223) with the Temple+Guild handler registrations from above. Three existing tests (`town_plugin_builds`, `town_camera_spawns_on_enter_and_despawns_on_exit`, `town_substate_defaults_to_square_on_enter`) must continue to pass.
- [x] Verify: run `cargo check` to catch any forgotten import or registration. (Verification deferred to orchestrator — no Bash access) Compile-errors here likely indicate a missing `pub fn` export from `temple.rs` or `guild.rs`, or a leftover reference to `placeholder` somewhere.

### Phase 6 — Verification quality gates

- [ ] Run all six gates locally. Fix any clippy or test failure. Both `--features dev` runs must pass too (F4 gold-grant, F9 cycler are in play). (Pending orchestrator verification)

## Security

**Known vulnerabilities:** No known vulnerabilities identified as of 2026-05-12. No new third-party crates introduced (Δ deps = 0) — `bevy = 0.18.1`, `bevy_egui = 0.39.1`, `bevy_common_assets = 0.16.0`, `bevy_asset_loader = 0.26.0`, `leafwing-input-manager = 0.20.0`, `serde = 1`, `ron = 0.12` are all already pinned in `Cargo.toml` and exercised by #18a's already-merged code.

**Architectural risks:**

- **RON trust boundary on `temple_*` costs** — crafted RON could declare `u32::MAX` revive cost (Temple effectively unusable) or `0` cost (free revive bug). Mitigation: `revive_cost` helper clamps via `.max(1).min(MAX_TEMPLE_COST = 100_000)`; `cure_cost` clamps via `.min(MAX_TEMPLE_COST)` and filters `Dead` out as the first line. (Research Architectural Security Risks row 1.)
- **RON trust boundary on `RecruitPool` size** — crafted RON with a 100K-entry pool would exhaust paint-loop memory. Mitigation: `clamp_recruit_pool(&pool, MAX_RECRUIT_POOL = 32)` is called in `paint_guild` before iterating, mirroring `clamp_shop_stock` precedent. (Research Architectural Security Risks row 2.)
- **RON trust boundary on `RecruitDef.base_stats`** — `derive_stats` already saturates at `u32` (verified at `character.rs:481-493`). The defense-in-depth here is genre-balance, not crash safety; #18b does NOT add a per-channel clamp (no design-pressure to ceiling stats; deferred to #25 polish if play-testing demands). Documented as deferred in the implementation comments.
- **Same-frame `PartyMember` removal masks count check** (Research Pitfall 2) — `commands.entity(target).remove::<PartyMember>()` is `apply_deferred`-gated. Two dismisses in one frame could both pass the "min-1-active" guard. Mitigation: each handler returns immediately after queueing the removal; the slot-swap and row-swap handlers operate on `With<PartyMember>` and therefore naturally skip stale entities once the deferral applies. Test `dismiss_then_recruit_in_one_frame_restores_count` covers the multi-handler-per-frame case.
- **Dismissed entity `Inventory.0` persistence** (Research Architectural Security Risks row 5) — if any future system *despawns* a dismissed entity, its `Inventory.0: Vec<Entity>` orphans the `ItemInstance` entities (no GC). Mitigation: Option A intentionally never despawns. Documented at the top of `guild.rs` and reinforced by the test `dismiss_preserves_inventory_entities`.
- **`RecruitDef.status_effects` does not exist** — recruits spawn with `StatusEffects::default()` (verified — `RecruitDef` at `data/town.rs:107-116` does not include a status field). No "cure-for-gold-on-newborn-recruit" exploit possible. Documented in `guild.rs` top comment as a forward-compat warning: "Adding a `status_effects` field to `RecruitDef` later would require Temple-cure-cost-drain risk analysis."
- **Same-frame `PartyMember` add after dismiss** — recruit on the same frame as a dismiss could read the pre-deferral `PartyMember` count. Mitigation: recruit's full-party guard uses `existing.iter().count()` which reflects the pre-deferral state. In the worst case (two same-frame inputs), the recruit could spawn while the dismiss hasn't applied — but the result is a brief 1-frame over-count, not a correctness bug; both deferrals apply on the next `apply_deferred` and the system stabilizes. The deferred-Commands "settle" pattern is the same shape as `spawn_default_debug_party` (#11 precedent).
- **`DismissedPool` save-format** — `Vec<Entity>` does not serialize across sessions; entity IDs are non-stable. Mitigation: Feature #23 must implement `MapEntities` for this resource (deferral contract identical to `Inventory(Vec<Entity>)` at `inventory.rs:179-181`). No `Serialize`/`Deserialize` derive on `DismissedPool` in #18b — the type intentionally cannot be serialized until #23.

**Trust boundaries:** the only untrusted input surface for #18b is the existing RON files in `assets/town/` (already cleared in #18a) plus the newly-authored `temple_*` field values in `core.town_services.ron`. The pre-clamp helpers in Phase 1 (`MAX_TEMPLE_COST`, `MAX_RECRUIT_POOL`, `revive_cost`'s `.max(1)`) bound every RON-sourced value before any consuming system reads it. No new HTTP/socket/IPC surface introduced. Asset path resolution inherits `bevy_common_assets` + `bevy_asset_loader`'s existing guards (same as #18a).

## Open Questions

All 8 open questions from the research are resolved by user decisions (Category A inputs):

- **Q1** (Cut dismissed-pool entirely?) → Resolved: SHIP NOW with `Resource<DismissedPool>` (Option A). ~120 LOC + 4 new tests budgeted in Phase 3.
- **Q2** (Which severe statuses does Temple cure?) → Resolved: `Dead` (Revive mode), `Stone`, `Paralysis`, `Sleep`. NOT `Poison` (Inn-only — confirmed by user). NOT `Silence` (deferred, no in-dungeon source yet). NOT buff variants (`AttackUp`/`DefenseUp`/`SpeedUp`/`Regen`).
- **Q3** (Slot reorder: swap vs. shift?) → Resolved: SWAP — two-write exchange of `PartySlot` values between two `With<PartyMember>` entities. Two-press UX (pin source → select target).
- **Q4** (Recruit-while-empty: allow or block?) → Resolved: ALLOW. Recruit has no minimum-active-party check (forward-compat with #19 Character Creation). Minimum-1-active check applies to Dismiss only. Asymmetry documented in code comments.
- **Q5** (Per-status cure cost authoring values) → Resolved: `Stone = 250`, `Paralysis = 100`, `Sleep = 50` (flat per-status, NOT level-scaled). Revive: `base = 100`, `per_level = 50` (saturating). Worked examples in tests: L1 → 150g, L5 → 350g.
- **Q6** (Multi-status Cure: pick UI vs. auto-pick?) → Resolved: AUTO-PICK first eligible severe status in priority order `Stone > Paralysis > Sleep`. Matches Inn's simplicity. Status-pick dialog is #25 polish.
- **Q7** (`DismissedPool` save format) → Resolved: defer `MapEntities` impl to Feature #23. Doc-comment on the `DismissedPool` resource references the #23 deferral. No serde derive yet — just `#[derive(Resource, Default, Debug)]`.
- **Q8** (RON file extension correctness) → Resolved (from project memory `project_druum_ron_asset_naming.md`): `core.town_services.ron` already uses the correct double-dot convention; the `RonAssetPlugin` registers extension `town_services.ron` at `loading/mod.rs:132`. Step 1 *modifies* the existing file's contents only — no rename, no recreate.

## Implementation Discoveries

1. **Phase 4 — `placeholder.rs` cannot be deleted via Write tool.** Since the implementer has no Bash access, the file `src/plugins/town/placeholder.rs` was not deleted but is no longer declared as a module (the `pub mod placeholder;` line was removed from `mod.rs`). Rust only compiles files declared as modules, so the file is dead/unreachable — functionally equivalent to deletion. The placeholder file's tests are no longer run. This should be manually deleted via shell after implementation.

2. **`ButtonInput<KeyCode>` in MinimalPlugins tests — `clear()` required after first update.** Unlike leafwing's `ActionState`, `ButtonInput<KeyCode>::just_pressed()` is never automatically cleared in MinimalPlugins (no `InputPlugin` → no `ButtonInput::flush` system). Used `clear()` after the first update (system fires) instead of `release()` in test helpers (`press_key_d`, `press_key_f`, `press_key_s`). This prevents double-firing on the second update.

3. **`handle_guild_slot_swap` slot-read approach.** Instead of calling `party.get(entity)` to read `PartySlot` values (which would borrow the mutable query), we pre-collect `Vec<(Entity, usize)>` from `party.iter()` and look up slot values from there. This avoids any question about sequential `get()` vs `get_mut()` borrowing order.

4. **Town mod.rs test module: `add_message::<EquipmentChangedEvent>()` was already registered** in the original test. After the rewrite, we retained one registration (for both `handle_inn_rest` and `handle_temple_action`). No duplicate registration needed.

5. **`DerivedStats` and `Equipment` not needed in `guild.rs` production imports.** Moved to test-only scope. `derive_stats` returns `DerivedStats` but the return type is inferred at call site. `Equipment` is accessed via `PartyMemberBundle { ..Default::default() }` without needing an explicit import.

## Verification

The six quality gates run by the implementer:

- [ ] **Gate 1:** `cargo check` — exit 0 — Automatic. Verifies the new code compiles without the `dev` feature.
- [ ] **Gate 2:** `cargo check --features dev` — exit 0 — Automatic. Verifies #[cfg(feature = "dev")] paths still compile (F4 gold-grant, F9 cycler, debug-party spawn).
- [ ] **Gate 3:** `cargo test` — exit 0 — Automatic. Baseline 260 lib + 6 integration tests must still pass. Δ tests = at minimum `+17` from Temple, `+15` from Guild, `+3` from `data/town.rs` round-trip and clamp helpers, minus the `-2` placeholder tests deleted in Phase 4 = net `+33`. Expected new baseline: `293 lib + 6 integration` (default). Implementer should report actual numbers in Implementation Discoveries.
- [ ] **Gate 4:** `cargo test --features dev` — exit 0 — Automatic. Baseline 264 lib + 6 integration must hold; Δ tests same as Gate 3 (no dev-gated tests added in #18b). Expected new baseline: `297 lib + 6 integration` (dev).
- [ ] **Gate 5:** `cargo clippy --all-targets -- -D warnings` — exit 0 — Automatic.
- [ ] **Gate 6:** `cargo clippy --all-targets --features dev -- -D warnings` — exit 0 — Automatic.

Module-specific tests (subset of Gate 3, individually runnable):

- [ ] **`data::town::tests::recruit_pool_size_clamped_truncates_oversized`** — 200-entry pool → `clamp_recruit_pool` returns 32-entry slice — unit — `cargo test -p druum data::town::tests::recruit_pool_size_clamped_truncates_oversized` — Automatic.
- [ ] **`data::town::tests::clamp_recruit_pool_passes_through_small_pool`** — 5-entry pool → returns all 5 — unit — Automatic.
- [ ] **`data::town::tests::town_services_round_trips_with_authored_temple_fields`** — RON with `temple_revive_cost_base: 100, temple_revive_cost_per_level: 50, temple_cure_costs: [(Stone, 250), (Paralysis, 100), (Sleep, 50)]` parses with non-zero values; re-serialize and re-parse stably — unit — Automatic.
- [ ] **`town::temple::tests::revive_cost_scales_linearly_with_level`** — `revive_cost(services, 1) == 150`, `revive_cost(services, 5) == 350` — unit — `cargo test -p druum town::temple::tests::revive_cost_scales_linearly_with_level` — Automatic.
- [ ] **`town::temple::tests::revive_cost_saturates_at_max_temple_cost`** — base + per_level set to `u32::MAX`, level 100 → returns `MAX_TEMPLE_COST` — unit — Automatic.
- [ ] **`town::temple::tests::revive_cost_guards_against_zero_via_max_1`** — base 0, per_level 0, level 1 → returns 1 (defense-in-depth from research Pitfall 3) — unit — Automatic.
- [ ] **`town::temple::tests::cure_cost_returns_none_for_dead`** — `cure_cost(services, Dead) == None` even if `(Dead, X)` is in the cure-cost list — unit — Automatic.
- [ ] **`town::temple::tests::cure_cost_returns_lookup_for_stone`** — `cure_cost(services, Stone) == Some(250)` — unit — Automatic.
- [ ] **`town::temple::tests::first_curable_status_picks_stone_when_present`** — member with `[Stone, Sleep]` → `Some((Stone, 250))` (priority order Stone > Paralysis > Sleep) — unit — Automatic.
- [ ] **`town::temple::tests::first_curable_status_picks_paralysis_when_only_paralysis_and_sleep`** — `[Paralysis, Sleep]` → `Some((Paralysis, 100))` — unit — Automatic.
- [ ] **`town::temple::tests::first_curable_status_returns_none_for_no_severe_status`** — `[Poison]` → `None` (Poison is not in `temple_cure_costs`) — unit — Automatic.
- [ ] **`town::temple::tests::revive_dead_member_clears_dead_and_sets_hp_to_1`** — spawn dead member level 1 + `Gold(1000)` + Revive mode + Confirm; assert `!status.has(Dead)`, `current_hp == 1`, `max_hp > 0`, `gold.0 == 850` — integration — Automatic.
- [ ] **`town::temple::tests::revive_rejects_non_dead_target`** — living member + Revive + Confirm; assert no change, gold unchanged — integration — Automatic.
- [ ] **`town::temple::tests::revive_rejects_when_insufficient_gold`** — `Gold(50)` + dead lvl-5 (cost 350) + Confirm; assert still Dead, `gold.0 == 50` — integration — Automatic.
- [ ] **`town::temple::tests::cure_stone_removes_status_and_deducts_gold`** — member with Stone + `Gold(500)` + Cure + Confirm; assert `!status.has(Stone)`, `gold.0 == 250` — integration — Automatic.
- [ ] **`town::temple::tests::cure_mode_does_not_remove_dead`** — Dead member + Cure + Confirm; assert still Dead, gold unchanged — integration — Automatic.
- [ ] **`town::temple::tests::cure_rejects_when_insufficient_gold`** — `Gold(100)` + Stone cost 250 + Confirm; assert still Stone, `gold.0 == 100` — integration — Automatic.
- [ ] **`town::temple::tests::cancel_returns_to_square`** — Cancel from Temple → sub-state `Square` — integration — Automatic.
- [ ] **`town::guild::tests::next_free_slot_picks_lowest_unused`** — `next_free_slot(&[0, 2], 4) == Some(1)` — unit — Automatic.
- [ ] **`town::guild::tests::next_free_slot_returns_none_when_full`** — `next_free_slot(&[0, 1, 2, 3], 4) == None` — unit — Automatic.
- [ ] **`town::guild::tests::next_free_slot_handles_empty_party`** — `next_free_slot(&[], 4) == Some(0)` — unit — Automatic.
- [ ] **`town::guild::tests::recruit_spawns_party_member_with_correct_bundle_fields`** — Recruit + Confirm + 2× update; assert one new `PartyMember` with name/race/class from pool[0] — integration — Automatic.
- [ ] **`town::guild::tests::recruit_picks_lowest_free_slot_after_dismissal`** — pre: 4 members at slots 0..=3; dismiss slot 1; recruit; assert new member at `PartySlot(1)` — integration — Automatic.
- [ ] **`town::guild::tests::recruit_rejects_when_party_full`** — 4 members already, recruit + Confirm; assert still 4 members — integration — Automatic.
- [ ] **`town::guild::tests::recruit_allows_empty_party`** — 0 active members, recruit pool[0] + Confirm; assert 1 member spawned (verifies no min-1 guard on Recruit) — integration — Automatic.
- [ ] **`town::guild::tests::recruit_attaches_empty_inventory_component`** — recruit; assert new member has `Inventory` and `inventory.0.is_empty()` — integration — Automatic.
- [ ] **`town::guild::tests::dismiss_removes_partymember_marker`** — spawn 2, dismiss cursor=0, 2× update; assert exactly 1 `PartyMember` — integration — Automatic.
- [ ] **`town::guild::tests::dismiss_adds_entity_to_pool`** — spawn 2, dismiss, 2× update; assert `pool.entities.len() == 1` — integration — Automatic.
- [ ] **`town::guild::tests::dismiss_preserves_inventory_entities`** — dismiss member with non-empty `Inventory.0`; assert dismissed entity still has `Inventory` and entries are intact — integration — Automatic.
- [ ] **`town::guild::tests::dismiss_rejects_last_active_member`** — spawn 1, dismiss, 2× update; assert still 1 `PartyMember`, pool empty — integration — Automatic.
- [ ] **`town::guild::tests::dismiss_then_recruit_in_one_frame_restores_count`** — Pitfall 2 regression test. Spawn 4, dismiss member 0, recruit from pool[0]; after settling, assert `Query<&PartyMember>::iter().count() == 4` and `pool.entities.len() == 1` (dismissed entity is in pool; new entity is a fresh recruit) — integration — Automatic.
- [ ] **`town::guild::tests::row_swap_toggles_front_to_back_and_back_to_front`** — spawn `Front`, press F twice; assert `Front → Back → Front` — integration — Automatic.
- [ ] **`town::guild::tests::slot_swap_exchanges_two_members_slots`** — spawn members at slots 0 and 2, slot-swap pin to first, slot-swap target second; assert post-swap slots are `(2, 0)`, total `PartyMember` count unchanged — integration — Automatic.
- [ ] **`town::guild::tests::cancel_returns_to_square`** — Cancel from Guild → `Square` — integration — Automatic.

Asset / runtime checks (one-time manual after build succeeds):

- [ ] **Manual:** `cargo run --features dev`. Press F9 to advance to `GameState::Town`. Press F4 a few times to grant gold for testing. Navigate to **Temple**: cursor a Dead party member (if none, dev hot-key kill one via debug input — verify against current `dev` features; if absent, accept this as a known limitation of dev parity), select Revive, press Enter; verify the member is no longer Dead and has `current_hp = 1`. Toggle to Cure mode, target a non-Dead member with Stone/Paralysis/Sleep (would need a `dev` route to apply — flag as a polish-item if cumbersome), press Enter; verify the status is removed. Press Esc → back to Square. Navigate to **Guild**: press R for Recruit mode, scroll to pick a recruit, press Enter; verify a new party member appears in Roster. Press D on cursor target; verify they disappear from the active roster. Press F on a target; verify their row toggles. Press S twice on different targets; verify their slots swap. Press Esc → Square. Select Leave Town → `TitleScreen`. Exit cleanly with Ctrl-C. — Manual.

GitButler workflow note (for the implementer, not the planner): per `CLAUDE.md`, commits MUST use `but commit --message-file <path>`; the pre-commit hook on `gitbutler/workspace` rejects `git commit`. Per project memory `feedback_but_commit_sweeps_all_zz.md`, `but commit` sweeps all unassigned hunks — if doing a planned multi-commit split, move unwanted files to a separate applied branch first (`but rub <file> <other-branch>`) before committing. Otherwise accept the single bundled commit.
