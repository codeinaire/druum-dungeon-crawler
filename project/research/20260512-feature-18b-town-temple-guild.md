# Feature #18b — Town Hub: Temple + Guild — Research

**Researched:** 2026-05-12
**Domain:** Bevy 0.18.1 ECS + bevy_egui 0.39.1 (town hub follow-up to #18a)
**Confidence:** HIGH — every claim verified against in-tree source or the #18a research doc

## Summary

Feature #18b ships the two remaining Town sub-states deferred from #18a: **Temple** (revive dead, cure severe status effects for gold) and **Guild** (party roster management — recruit from a pre-authored pool, dismiss, reorder slots, swap front/back row). Both screens reuse #18a's painter/handler split, asset pipeline, and gold/clock resources. The work is overwhelmingly *additive*: zero new dependencies, no new sub-states, no RON schema migration. The `RecruitPool` asset and the `temple_*` `TownServices` fields were already authored in #18a so #18b is the *first consumer*, not the schema introducer.

The hardest design question is **dismissed-pool shape** (Resource-of-entities vs. component-marker vs. enum component). A second open question — whether the entire `Dismissed` mechanic can be cut from #18b — is surfaced for the user. The current debug-party spawn (`spawn_default_debug_party` at `src/plugins/party/mod.rs:95`) does not exercise dismissal and v1 has no save/load, so a minimal "always-active recruit, no dismiss" Guild is a viable scope-cut that would close all `Dismissed` plumbing risk.

The pre-#18a research surfaced and resolved most architectural questions for both screens. This document focuses on the gaps #18a left open: status-effect cure boundary (which severe statuses Temple cures), per-level cost formula (no precedent in tree — must pick), dismissed-pool data shape, recruit-spawn correctness, and the maximum-party-size enforcement.

**Primary recommendation:** Ship Temple and Guild as two new files (`src/plugins/town/temple.rs`, `src/plugins/town/guild.rs`) that delete `placeholder.rs`. Use a **`Resource<DismissedPool { entities: Vec<Entity> }>`** for the dismissal mechanism (Option A — least query churn, save-friendly), a **linear `temple_revive_cost_base + temple_revive_cost_per_level * level` formula** (matches the field shape #18a already pre-authored, mirrors classic Wizardry, easy to balance), and Temple cures **`Dead`, `Stone`, `Paralysis`, `Sleep`** (the four "severe" non-buff statuses the player can't recover from in-dungeon). `Poison` stays Inn-only. The buff/regen variants are not cure targets. Guild enforces `PartySize::default() = 4` for recruit cap and minimum-1-member-active for dismiss. **Save/load is out of scope (#23).**

## Standard Stack

### Core

| Library | Version | Purpose | License | Maintained? | Why Standard |
|---|---|---|---|---|---|
| `bevy` | =0.18.1 | ECS, schedule, state machine | MIT/Apache-2.0 | Yes | Already pinned |
| `bevy_egui` | =0.39.1 | Painters for Temple/Guild screens | MIT | Yes | Pattern proven by #18a Inn/Shop/Square |
| `leafwing-input-manager` | =0.20.0 | `Res<ActionState<MenuAction>>` | ISC/Apache-2.0 | Yes | Already pinned |
| `bevy_common_assets` | =0.16.0 | `RonAssetPlugin::<RecruitPool>` (already registered) | MIT/Apache-2.0 | Yes | Already pinned |
| `bevy_asset_loader` | =0.26.0 | `TownAssets` collection (already registered) | MIT/Apache-2.0 | Yes | Already pinned |
| `serde` / `ron` | 1 / 0.12 | RON deserialization | MIT/Apache-2.0 | Yes | Already pinned |

**Δ Cargo.toml = 0.** No new crate additions; no feature flips. Verified at `Cargo.toml:1-37`.

### Supporting

| Library | Version | Purpose | When to Use |
|---|---|---|---|
| `bevy::state::SubStates` | bundled | `TownLocation::{Temple,Guild}` already declared | Already-shipped — no work |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|---|---|---|
| `Resource<DismissedPool { Vec<Entity> }>` | `Component<Dismissed>` marker | Marker requires updating *every* existing `With<PartyMember>` query to `(With<PartyMember>, Without<Dismissed>)` — 26 query sites identified at `src/**/*.rs` (see Pitfalls). Resource is a smaller blast radius. |
| `Vec<Entity>` dismissal | `enum PartyStatus { Active(PartySlot), Dismissed }` | Replacing `PartySlot` is a more invasive carve-out; combat/turn-manager and `derive_stats` callers cache `PartySlot` indirectly via `Query<&PartySlot, With<PartyMember>>` at `combat/turn_manager.rs:233`. Field replacement would touch every combat read site. |
| Cut dismissed-pool entirely | Out-of-scope to #19 | Roadmap §18 Guild lists "active 4-6 + dismissed pool" as v1 scope, but `spawn_default_debug_party` is the only spawn pipeline today and has no concept of dismissal. **This is the cleanest scope-cut available — surface to user.** |

**Installation:** Δ deps = 0. No commands.

## Architecture Options

### Dismissed-pool data shape — three options

Fundamental data-modelling question: where do dismissed party members live in the ECS? Researched by walking every `With<PartyMember>` query site in the codebase.

| Option | Description | Pros | Cons | Best When |
|---|---|---|---|---|
| **(A) `Resource<DismissedPool { entities: Vec<Entity> }>`** (RECOMMENDED) | Dismissed entity stays alive, marker `PartyMember` is REMOVED on dismiss. Resource holds the pool as a registry. Re-recruit: re-add `PartyMember` + new `PartySlot`. | 0 churn to existing `With<PartyMember>` queries (26 sites). Clear separation: active = `With<PartyMember>`, dismissed = look up via the resource. Save-format is naturally a single resource. | Removing/adding `PartyMember` mid-frame is `Commands`-deferred (not visible same frame). Must verify combat/turn-manager doesn't grab a frozen Entity list mid-dismissal. |
| (B) `Component<Dismissed>` marker on the entity | Entity keeps `PartyMember`; gains `Dismissed` zero-sized component on dismiss. Active queries become `(With<PartyMember>, Without<Dismissed>)`. | Single source of truth (the entity owns its state). | **26 query sites need updating** to add `Without<Dismissed>` — `src/plugins/combat/{ai,turn_manager,status_effects,ui_combat,enemy,damage}.rs`, `src/plugins/dungeon/features.rs:259,375,427,894,988`, `src/plugins/party/inventory.rs:279,350,394,652,712,773,945,1003,1073,1184,1273`, `src/plugins/town/{shop,inn}.rs`. High blast radius. |
| (C) `enum PartyStatus { Active(PartySlot), Dismissed }` component replacing `PartySlot` | Status + slot collapse into one enum component. | Type-enforces "dismissed members don't have a slot". | Replaces `PartySlot` (which has 6 in-tree readers: `combat/ai.rs:86`, `combat/turn_manager.rs:233`, `combat/ui_combat.rs:247`, `dungeon/features.rs:259` indirect, `data/town.rs:RecruitDef.default_row`, `character.rs:191` definition). #11 Decision intentionally kept `PartySlot` and the `PartyMember` marker as separate components — collapsing them now re-litigates #11 architecture. |

**Recommended:** **Option A** — least churn, save-friendly, deferred-Commands risk is manageable with a one-frame settle (mirrors `spawn_default_debug_party` pattern that already relies on deferred spawn).

### Counterarguments

Why someone might NOT choose Option A:

- **"Resources don't compose with Bevy's reflection/serde out-of-the-box for `Vec<Entity>`."** — Response: `Inventory(Vec<Entity>)` *already* has this exact problem (`inventory.rs:179-181`), and the project's stance (frozen by #12) is "Feature #23 will implement `MapEntities` for it". `DismissedPool` would mirror that contract — a single fixed-pattern serde impl, not novel work.
- **"You'll forget that a 'PartyMember without the marker' is still in the world."** — Response: this is a documentation discipline issue. Mitigated by a top-of-`guild.rs` invariant comment + a unit test asserting `Dismissed` entities are NOT counted by `Query<&PartyMember>::iter().count()`.
- **"What if dismissal triggers anything in dungeon/combat code?"** — Response: investigated — only the spawn pipeline (`spawn_default_debug_party`) cares about counts, and only when `count == 0` to skip re-spawning. Dismissal in Town doesn't intersect any dungeon/combat path because dismissal can only happen in `GameState::Town`. Verified no system in `src/plugins/{dungeon,combat}/` mutates `PartyMember` or counts it for invariants — they only *read* via `With<PartyMember>` filters.

### Cut-scope alternative — surface to user

A genuinely viable scope cut: **defer dismissed-pool to Feature #19** (Character Creation). #19 needs the same plumbing because creating a new character requires choosing between "add to active party" vs. "stash in pool when party is full". #18b would ship just **Recruit (requires party not full) + Reorder + Row Swap**, skipping Dismiss entirely. This collapses the architectural decision (A vs. B vs. C) and saves ~150 LOC. Surfaced as Open Question 1.

## Architecture Patterns

### Recommended Project Structure

```
src/plugins/town/
├── mod.rs                   # TownPlugin (registered painters + handlers updated)
├── gold.rs                  # unchanged
├── square.rs                # unchanged
├── shop.rs                  # unchanged
├── inn.rs                   # unchanged
├── temple.rs                # NEW — paint_temple + handle_temple_action + revive_cost helper
├── guild.rs                 # NEW — paint_guild + handle_guild_action + dismissed-pool resource
└── placeholder.rs           # DELETED — its Temple/Guild routing is now real
assets/town/
├── core.town_services.ron   # MODIFIED — author concrete temple_* values
├── core.recruit_pool.ron    # unchanged
└── core.shop_stock.ron      # unchanged
```

### Pattern 1: Temple revive + cure (the canonical Inn-mirror)

```rust
// Source: derived from src/plugins/town/inn.rs:106-173 and #18a research §Pattern 5
use bevy::prelude::*;
use bevy_egui::{EguiContexts, egui};
use leafwing_input_manager::prelude::ActionState;

use crate::data::town::{TownServices, MAX_INN_COST};
use crate::plugins::input::MenuAction;
use crate::plugins::loading::TownAssets;
use crate::plugins::party::character::{
    DerivedStats, Experience, PartyMember, StatusEffectType, StatusEffects,
};
use crate::plugins::party::inventory::{EquipSlot, EquipmentChangedEvent};
use crate::plugins::state::TownLocation;
use crate::plugins::town::gold::Gold;

#[derive(Resource, Default, Debug)]
pub struct TempleState {
    /// 0 = Revive, 1 = Cure
    pub mode: TempleMode,
    pub cursor: usize,
    pub party_target: usize,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Default)]
pub enum TempleMode {
    #[default]
    Revive,
    Cure,
}

/// Cap on the *combined* cost a single Temple action can charge.
/// Mirrors `MAX_INN_COST = 10_000` (Phase 10 trust boundary in #18a).
pub const MAX_TEMPLE_COST: u32 = 100_000;

/// Compute revive cost: base + per_level * level, saturating.
/// Pure function — testable without an App.
pub fn revive_cost(services: &TownServices, level: u32) -> u32 {
    let cost = services
        .temple_revive_cost_base
        .saturating_add(services.temple_revive_cost_per_level.saturating_mul(level));
    cost.min(MAX_TEMPLE_COST)
}

/// Look up cure cost for a status type. Returns None if Temple does not cure it.
pub fn cure_cost(services: &TownServices, kind: StatusEffectType) -> Option<u32> {
    services
        .temple_cure_costs
        .iter()
        .find(|(k, _)| *k == kind)
        .map(|(_, cost)| (*cost).min(MAX_TEMPLE_COST))
}

#[allow(clippy::too_many_arguments)]
pub fn handle_temple_action(
    actions: Res<ActionState<MenuAction>>,
    mut temple_state: ResMut<TempleState>,
    mut gold: ResMut<Gold>,
    town_assets: Option<Res<TownAssets>>,
    services_assets: Res<Assets<TownServices>>,
    mut next_sub: ResMut<NextState<TownLocation>>,
    mut writer: MessageWriter<EquipmentChangedEvent>,
    mut party: Query<
        (Entity, &Experience, &mut DerivedStats, &mut StatusEffects),
        With<PartyMember>,
    >,
) {
    if actions.just_pressed(&MenuAction::Cancel) {
        next_sub.set(TownLocation::Square);
        return;
    }
    if !actions.just_pressed(&MenuAction::Confirm) { return; }

    let Some(assets) = town_assets else { return; };
    let Some(services) = services_assets.get(&assets.services) else { return; };

    // Resolve target party member by deterministic Entity ordering (mirror shop.rs:397).
    let mut entities: Vec<(Entity, u32, bool)> = party
        .iter()
        .map(|(e, xp, _d, st)| (e, xp.level, st.has(StatusEffectType::Dead)))
        .collect();
    entities.sort_by_key(|(e, _, _)| *e);
    let Some(&(target, level, is_dead)) = entities.get(temple_state.party_target) else { return; };

    match temple_state.mode {
        TempleMode::Revive => {
            if !is_dead {
                info!("Temple revive: target {:?} is not dead", target);
                return;
            }
            let cost = revive_cost(services, level);
            if gold.0 < cost {
                info!("Temple revive: insufficient gold (have {}, need {})", gold.0, cost);
                return;
            }
            let Ok((_, _xp, mut derived, mut status)) = party.get_mut(target) else { return; };

            // Remove Dead from the effects vec (consistent with Inn cure pattern).
            status.effects.retain(|e| e.effect_type != StatusEffectType::Dead);
            // Wizardry convention: revived to 1 HP, NOT max.
            derived.current_hp = 1;

            // Fire EquipmentChangedEvent — recompute_derived_stats_on_equipment_change
            // (inventory.rs:444, NO With<PartyMember> filter) re-derives max_hp/max_mp
            // (which the `Dead` branch of derive_stats had zeroed). After re-derive,
            // the caller-clamp at inventory.rs:495 caps current_hp = min(old, new_max),
            // so current_hp = 1 stays at 1. Wizardry behaviour preserved.
            writer.write(EquipmentChangedEvent { character: target, slot: EquipSlot::None });

            let _ = gold.try_spend(cost);
            next_sub.set(TownLocation::Square);
            info!("Temple revived {:?} for {} gold (level {})", target, cost, level);
        }
        TempleMode::Cure => {
            // Cure the FIRST eligible severe status on the target.
            // (UI in #25 could let player pick which status; v1 = first-found.)
            let Ok((_, _xp, _d, status)) = party.get(target) else { return; };
            let curable = status.effects.iter().find_map(|e| {
                cure_cost(services, e.effect_type).map(|c| (e.effect_type, c))
            });
            let Some((kind, cost)) = curable else {
                info!("Temple cure: target has no curable status");
                return;
            };
            if gold.0 < cost {
                info!("Temple cure: insufficient gold (have {}, need {})", gold.0, cost);
                return;
            }
            let Ok((_, _xp, _derived, mut status)) = party.get_mut(target) else { return; };
            status.effects.retain(|e| e.effect_type != kind);
            // Dead is also revived-from-this-path? NO — revive is the Revive mode.
            // Cure handles non-Dead severe statuses (Stone/Paralysis/Sleep).
            writer.write(EquipmentChangedEvent { character: target, slot: EquipSlot::None });
            let _ = gold.try_spend(cost);
            next_sub.set(TownLocation::Square);
            info!("Temple cured {:?} of {:?} for {} gold", target, kind, cost);
        }
    }
}
```

**What:** Single Update-schedule handler for both Revive and Cure modes.
**When to use:** Sole owner of `StatusEffects.effects` mutation outside of `apply_status_handler` (Feature #14's canonical mutator).
**Why this works without a recompute filter change:** `recompute_derived_stats_on_equipment_change` at `inventory.rs:444-501` is **filter-free** (verified — the `With<PartyMember>` filter was carved out in #15 D-A5). It reads `StatusEffects` directly, so removing `Dead` from `effects.vec` plus firing `EquipmentChangedEvent { slot: None }` re-derives `max_hp`/`max_mp` from the now-non-zero `derive_stats` Dead branch (`character.rs:476-479`).

### Pattern 2: Guild — Recruit + Dismiss + Reorder + Row swap

```rust
// Source: derived from #18a research §Pattern 6 + verified PartyMemberBundle shape
// at src/plugins/party/character.rs:316-333 + spawn_default_debug_party at mod.rs:131-147

use crate::plugins::party::{
    BaseStats, CharacterName, Class, DerivedStats, Equipment, Experience,
    PartyMember, PartyMemberBundle, PartyRow, PartySize, PartySlot, Race,
    StatusEffects, derive_stats,
};
use crate::plugins::party::inventory::Inventory;
use crate::data::town::RecruitPool;
use crate::plugins::loading::TownAssets;

/// Dismissed-pool resource — Option A (recommended).
///
/// Dismissed members keep their entity alive (preserves XP, equipment history),
/// but lose their `PartyMember` marker so existing `With<PartyMember>` queries
/// (26 sites — see Architecture Options) continue to work unchanged.
///
/// Re-recruit path: pop entity from this Vec, reinsert PartyMember + a fresh
/// PartySlot.
///
/// **Feature #23 (save/load) note:** `Vec<Entity>` does not serialize naturally;
/// the save layer must implement `MapEntities` for this resource (mirrors the
/// already-documented contract for `Inventory(Vec<Entity>)` at
/// `inventory.rs:179-185`).
#[derive(Resource, Default, Debug, Reflect)]
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
    Roster,    // browse active + dismissed
    Recruit,   // pick from RecruitPool
}

pub fn handle_guild_recruit(
    mut commands: Commands,
    guild_state: Res<GuildState>,
    town_assets: Option<Res<TownAssets>>,
    pool_assets: Res<Assets<RecruitPool>>,
    party_size: Res<PartySize>,
    existing: Query<(), With<PartyMember>>,
    // ... (full signature elided for brevity)
) {
    // Verify party not full (#15 hard cap = 4 by Decision 6 of #11).
    let active_count = existing.iter().count();
    if active_count >= party_size.0 {
        info!("Guild recruit: party full ({}/{})", active_count, party_size.0);
        return;
    }
    let Some(assets) = town_assets else { return; };
    let Some(pool) = pool_assets.get(&assets.recruit_pool) else { return; };
    let Some(recruit) = pool.recruits.get(guild_state.cursor) else { return; };

    // Compute next free PartySlot — find lowest unused index in [0, party_size).
    let used_slots: std::collections::HashSet<usize> = existing
        .iter().map(|()| /* slot lookup */ 0).collect(); // simplified
    let next_slot = (0..party_size.0).find(|i| !used_slots.contains(i))
        .unwrap_or(active_count);

    let derived = derive_stats(&recruit.base_stats, &[], &StatusEffects::default(), 1);
    commands
        .spawn(PartyMemberBundle {
            name: CharacterName(recruit.name.clone()),
            race: recruit.race,
            class: recruit.class,
            base_stats: recruit.base_stats,
            derived_stats: derived,
            party_row: recruit.default_row,
            party_slot: PartySlot(next_slot),
            ..Default::default()
        })
        .insert(Inventory::default());
}

pub fn handle_guild_dismiss(
    mut commands: Commands,
    mut pool: ResMut<DismissedPool>,
    party: Query<(Entity, &PartySlot), With<PartyMember>>,
    target_index: usize,
) {
    let mut entries: Vec<(Entity, PartySlot)> = party.iter().map(|(e, s)| (e, *s)).collect();
    entries.sort_by_key(|(e, _)| *e);
    let Some(&(target, _slot)) = entries.get(target_index) else { return; };

    // Minimum-1-active invariant.
    if entries.len() <= 1 {
        info!("Guild dismiss: cannot dismiss the last active member");
        return;
    }

    // Remove PartyMember marker; keep entity alive in DismissedPool.
    commands.entity(target).remove::<PartyMember>();
    pool.entities.push(target);
    info!("Guild: dismissed {:?}", target);
}

pub fn handle_guild_row_swap(
    selected: Res<GuildState>,
    mut party: Query<(Entity, &PartySlot, &mut PartyRow), With<PartyMember>>,
) {
    let mut entries: Vec<(Entity, PartySlot)> = party
        .iter().map(|(e, s, _)| (e, *s)).collect();
    entries.sort_by_key(|(e, _)| *e);
    let Some(&(target, _)) = entries.get(selected.cursor) else { return; };
    if let Ok((_, _, mut row)) = party.get_mut(target) {
        *row = match *row {
            PartyRow::Front => PartyRow::Back,
            PartyRow::Back => PartyRow::Front,
        };
    }
}
```

**What:** One handler per action (recruit / dismiss / row-swap / reorder); all share `GuildState` for cursor + mode.
**When to use:** Sole owner of `PartyMember` add/remove outside `spawn_default_debug_party` (which itself is gated `cfg(feature = "dev")`).

### Pattern 3: Painter mirror (no mutations)

```rust
// Source: directly modelled on src/plugins/town/inn.rs:50-88
pub fn paint_temple(
    mut contexts: EguiContexts,
    gold: Res<Gold>,
    temple_state: Res<TempleState>,
    town_assets: Option<Res<TownAssets>>,
    services_assets: Res<Assets<TownServices>>,
    party: Query<(Entity, &CharacterName, &Experience, &StatusEffects), With<PartyMember>>,
) -> Result {
    let ctx = contexts.ctx_mut()?;
    egui::TopBottomPanel::top("temple_header").show(ctx, |ui| {
        ui.horizontal(|ui| {
            ui.heading("Temple");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(format!("{} Gold", gold.0));
            });
        });
    });
    egui::CentralPanel::default().show(ctx, |ui| {
        // List party with status + per-row cost
        // ...
    });
    Ok(())
}
```

### Anti-Patterns to Avoid

- **Mutating `StatusEffects.effects` from anywhere except the canonical mutator path.** #14 declared `apply_status_handler` as the *sole* mutator. Temple revive/cure must NOT push through that handler (which would re-trigger merge logic and write `EquipmentChangedEvent` itself, but only for the buff variants — missing `Stone`/`Paralysis`/`Sleep`). Instead, Temple directly `effects.retain(|e| e.effect_type != target)` and fires `EquipmentChangedEvent { slot: None }` manually — same shape as Inn at `inn.rs:154-162`. This is documented exception, not a violation: Temple is a cure, not an apply.
- **Despawning a dismissed party member** would drop their `Inventory(Vec<Entity>)` references, orphaning `ItemInstance` entities forever. The recommended approach (Option A) doesn't despawn — it removes the `PartyMember` marker only. The entity lives on with `Inventory`, `Equipment`, `Experience`, `BaseStats`, etc. intact.
- **Calling `derive_stats` directly from `temple.rs`.** Don't reinvent the recompute. Fire `EquipmentChangedEvent { slot: EquipSlot::None }` and let `recompute_derived_stats_on_equipment_change` (inventory.rs:444) do the work — it already reads `StatusEffects` and the filter is dropped.
- **Mixing Temple `Cure` with `Dead`.** The Cure mode should NOT cure `Dead` even if you put `Dead → 0 gold` in `temple_cure_costs`. Revive is its own mode with its own per-level cost formula. Filter `Dead` out of the cure-cost lookup at the helper.
- **Letting Temple silently succeed with cost 0.** `temple_revive_cost_base` defaults to 0 (`#[serde(default)]`). A typo or omitted RON value would make revive free. The authored RON values must be non-zero; add a clamp `cost.max(1)` in `revive_cost` as defense-in-depth.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---|---|---|---|
| Stat re-derivation after status change | A new `recompute_after_temple` system | Fire `EquipmentChangedEvent { slot: EquipSlot::None }` → reuse `recompute_derived_stats_on_equipment_change` (`inventory.rs:444`) | Filter already dropped; signature already correct; #14 wired the same pattern. |
| RON-asset shape for service costs | A new `TempleServices` asset | Use the existing `TownServices` fields (`temple_revive_cost_base`, `temple_revive_cost_per_level`, `temple_cure_costs`) at `data/town.rs:155-167` | Shape was pre-shipped by #18a with `#[serde(default)]` — adding values to the RON file is the entire data task. |
| Per-level cost helper | A new generic "level-cost" function | Author one specific `revive_cost(services, level) -> u32` pure helper in `temple.rs` | Only one caller; abstraction adds no value. Mirrors `clamp_shop_stock` precedent at `data/town.rs:94-97`. |
| Party-size enforcement | A new `MAX_GUILD_RECRUITS` constant | Use `Res<PartySize>` (defaults to 4) at `character.rs:344-351` | Already wired; `spawn_default_debug_party` already respects it. |
| Inventory cleanup on dismiss | A new "drop inventory entities" path | Don't despawn — keep `Inventory` intact on the dismissed entity (Option A). Re-recruit restores everything. | Option A's whole point. |

## Common Pitfalls

### Pitfall 1: `recompute_derived_stats_on_equipment_change` requires `&Experience`, not just any 5-tuple

**What goes wrong:** Naively spawning a recruit with `commands.spawn(PartyMemberBundle { ... })` then expecting `EquipmentChangedEvent` to refresh their stats — the recompute system at `inventory.rs:447-453` queries `(&BaseStats, &Equipment, &StatusEffects, &Experience, &mut DerivedStats)`. If you spawn without `Experience` (defaults are fine — `..Default::default()` includes it), the query mismatches and the event is silently dropped.
**Why it happens:** `PartyMemberBundle` at `character.rs:320-333` does include `Experience: Experience` so `..Default::default()` covers it. The pitfall is only if a future contributor omits `Experience` from the bundle. **As-of #18b: not a real risk if the bundle is used; gotcha if someone manually `commands.spawn((PartyMember, BaseStats::default(), ...))` without the bundle.**
**How to avoid:** Always go through `PartyMemberBundle::default()` then override; never build a partial entity by hand. Mirror `spawn_default_debug_party` at `party/mod.rs:134-146`.

### Pitfall 2: Removing `PartyMember` is `Commands`-deferred — same-frame queries see the stale state

**What goes wrong:** Player presses "Dismiss" in Guild. The handler removes `PartyMember`, then a second handler later in the same frame counts `Query<&PartyMember>::iter().count()` for party-size validation — and gets the old count.
**Why it happens:** `Commands::remove::<T>()` queues the removal until `apply_deferred`; subsequent same-frame systems see the pre-removal state. This is the same shape as the Pitfall 1 in `inventory.rs:264-265` ("the new entity is not queryable in the same frame").
**How to avoid:** Guild dismiss handler must `return` after queuing the removal (no further mutations of party state in the same frame). Recruit handler likewise should not assume same-frame visibility of a dismiss. Test: `dismiss_then_recruit_in_one_frame_does_not_double_count`.

### Pitfall 3: `temple_*` field zeros from #[serde(default)] vs. authored 0

**What goes wrong:** The current `core.town_services.ron` (file at `assets/town/core.town_services.ron`, lines 4-7) does NOT mention `temple_revive_cost_base` etc., so they parse as `0`. A `0` revive cost is a free-revive bug.
**Why it happens:** `#[serde(default)]` was the deliberate forward-compat lever to ship `#18a` without authored Temple values. Now that #18b reads these fields, the RON file MUST be updated.
**How to avoid:** Phase 1 of #18b implementation: author `core.town_services.ron` with non-zero values. The planner's first step. Defense-in-depth: `revive_cost.max(1)` in the helper, so a typo doesn't free-revive.

### Pitfall 4: Cure removing `Dead` would interfere with Revive's specific HP-set semantics

**What goes wrong:** If Temple `Cure` mode is allowed to remove `Dead` (via a `(Dead, X)` entry in `temple_cure_costs`), the cure path *only* removes the effect — it never sets `current_hp = 1`. After the recompute, `derive_stats` re-derives `max_hp > 0`, but the caller-clamp at `inventory.rs:497-499` keeps `current_hp = min(0, max_hp) = 0`. Result: the character is "cured of Dead" but has 0 HP and is treated as Dead by every downstream system that checks `current_hp == 0`.
**Why it happens:** Inn cure uses `effects.retain` only, because Inn doesn't touch Dead. Cure inherits Inn's shape but Dead is a special case.
**How to avoid:** Temple revive (which sets `current_hp = 1` *before* firing the event) is the ONLY path that removes `Dead`. Cure-mode explicitly filters `Dead` out of `cure_cost`'s eligibility (recommended pattern in §Pattern 1 above).

### Pitfall 5: Inventory.0 cap is 8 — recruits must fit, but they also need an empty Inventory

**What goes wrong:** A newly-spawned recruit needs `Inventory::default()` (empty `Vec<Entity>`) attached separately from the bundle — `PartyMemberBundle` doesn't include `Inventory` (verified at `character.rs:320-333`).
**Why it happens:** The plan in #18a Phase 4 of party (memory) and `spawn_default_debug_party` at `party/mod.rs:146` use `.insert(Inventory::default())` as a separate call. The bundle deliberately omits it because not every `PartyMember` user needs inventory — but every recruit does.
**How to avoid:** `commands.spawn(PartyMemberBundle { ... }).insert(Inventory::default())` — same shape as line 146 of `party/mod.rs`. Unit test: `recruit_spawns_with_empty_inventory_component`.

### Pitfall 6: PartySlot uniqueness is not type-enforced

**What goes wrong:** Two party members with `PartySlot(0)` would break combat (turn-manager assumes uniqueness at `combat/turn_manager.rs:233`). Recruit's slot-finding logic must be correct.
**Why it happens:** `PartySlot(pub usize)` is a thin newtype; no invariant enforced at the type level.
**How to avoid:** Recruit's "next free slot" algorithm enumerates `used_slots: HashSet<usize>` from `Query<&PartySlot, With<PartyMember>>` and picks `(0..party_size).find(|i| !used_slots.contains(i))`. Unit test: `recruit_picks_lowest_free_slot_after_dismissal_of_middle`.

### Pitfall 7: Reorder semantics — slot SWAP vs. slot SHIFT

**What goes wrong:** "Reorder slots" can mean two things: (a) swap two members' `PartySlot` values, (b) shift one member to a new slot index, pushing others. Wizardry uses swap (formation cycling); Etrian Odyssey uses arbitrary placement.
**Why it happens:** The roadmap says "Implement dismiss / reorder party slots" without specifying — ambiguous.
**How to avoid:** **Surface as Open Question 3 (Category C decision).** Recommendation: SWAP — it's strictly simpler (1 operation = 2 component writes) and matches the Wizardry feel. Reordering by SHIFT can land in #25 polish if play-test demands it.

### Pitfall 8: Dismissed members can be "stat-changed" by stale `EquipmentChangedEvent` writers

**What goes wrong:** If a status-effect tick (`tick_status_durations` at `status_effects.rs:243-294`) writes `EquipmentChangedEvent { character: dismissed_entity }`, the recompute system at `inventory.rs:444` *will* fire on the dismissed entity (no `With<PartyMember>` filter). Status effects on dismissed members would tick and re-derive stats even though they're not in the active party. This is a slow performance bleed, not a correctness bug.
**Why it happens:** The `With<PartyMember>` carve-out (#15 D-A5) was for enemies, not dismissed members.
**How to avoid:** Status effects don't tick in Town anyway (Town doesn't fire `MovedEvent` or combat turn-ends). Pitfall is theoretical. Document at the top of `guild.rs`: "Dismissed entities retain `StatusEffects` but no system writes `EquipmentChangedEvent` for them in Town."

### Pitfall 9: Spawning a fresh recruit while `spawn_default_debug_party` is gated `cfg(feature = "dev")`

**What goes wrong:** Production builds (no `dev` feature) have NO debug party. If a player goes Town → Guild and recruits without a starting party, the Guild handler's "minimum-1-active" invariant fails *before* the player has even one member.
**Why it happens:** `spawn_default_debug_party` is at `party/mod.rs:76-80` gated `cfg(feature = "dev")`. Production has no other spawn path until #19 (character creation) lands.
**How to avoid:** Two options: (a) ship Guild recruit with NO minimum-1 check (allows recruiting from an empty party); (b) require party non-empty (player can never reach Guild without starting party — current state is `cfg(feature = "dev")` only anyway). **Recommendation:** ship (a) — recruiting from empty is harmless and forward-compatible with #19. Surface to user as Open Question 4.

### Pitfall 10: F4 hotkey grants gold globally — works in Temple/Guild too

**What goes wrong:** `gold::grant_gold_on_f4` at `gold.rs:113-121` is registered unconditionally in `Update` (mod.rs:128-129) when `feature = "dev"` is on. Pressing F4 during Guild recruitment works fine; pressing F4 with a leafwing-bound key in the input layer could conflict. F4 is only bound via raw `ButtonInput<KeyCode>`, so no leafwing collision. Mention as a non-issue but document.
**Why it happens:** F4 is intentionally not leafwing-routed (input/mod.rs:14-22 documents the F9 cycler carve-out; F4 follows the same pattern).
**How to avoid:** None needed — it's by design. Document for the planner.

## Security

### Known Vulnerabilities

| Library | CVE / Advisory | Severity | Status | Action |
|---|---|---|---|---|
| `bevy = 0.18.1` | None found | — | — | No action |
| `bevy_egui = 0.39.1` | None found | — | — | No action |
| `bevy_common_assets = 0.16.0` | None found | — | — | No action |
| `bevy_asset_loader = 0.26.0` | None found | — | — | No action |
| `leafwing-input-manager = 0.20.0` | None found | — | — | No action |
| `serde = 1` | None found in the dependency-pin range | — | — | No action |
| `ron = 0.12` | None found | — | — | No action |

No new dependencies. The exposure is identical to #18a's, which the reviewer cleared.

### Architectural Security Risks

| Risk | Affected Architecture Options | How It Manifests | Secure Pattern | Anti-Pattern to Avoid |
|---|---|---|---|---|
| **RON trust boundary on `temple_*` costs** | Temple revive/cure | Crafted RON could declare `u32::MAX` revive cost (unusable) or 0 (free revive bug) | Author non-zero values; clamp `cost.max(1).min(MAX_TEMPLE_COST)` in `revive_cost`/`cure_cost` helpers | Trusting raw u32 from RON |
| **RON trust boundary on `RecruitPool` size** | Guild recruit | Crafted RON with 100K-entry pool exhausts paint-loop memory | Clamp pool size to `MAX_RECRUIT_POOL = 32` in `guild.rs`'s painter (mirrors `clamp_shop_stock` at `data/town.rs:94-97`) | Iterating `pool.recruits` directly without bound |
| **RON trust boundary on `RecruitDef.base_stats`** | Guild recruit | Crafted RON with `BaseStats { strength: u16::MAX, ... }` produces overpowered recruit | `derive_stats` already saturates at u32 (verified at `character.rs:629-656`) — defense-in-depth. Add `clamp_base_stats` helper that caps each channel at 25 (Wizardry-style ceiling). | Direct field copy from `RecruitDef.base_stats` into bundle |
| **Same-frame `PartyMember` removal masks count check** | Guild dismiss (Pitfall 2) | Two dismisses in one frame might both pass the "min-1-active" guard | One mutation per frame; rely on `Commands` deferral | Counting after queueing |
| **Dismissed entity Inventory persistence** | Guild dismiss + Option A | If a dismissed entity is *also* despawned (by some future Town cleanup system), its `Inventory.0: Vec<Entity>` orphans `ItemInstance` entities | Option A intentionally never despawns. Document at top of `guild.rs`. | `commands.entity(target).despawn()` on dismiss |
| **Temple cure of a status the recruit shouldn't have** | Temple cure mode | Pool entity's RON-authored statuses (not currently a feature) could be cured for gold, draining player funds with no in-tree benefit | `RecruitDef` does NOT declare `status_effects` field — recruits spawn with `StatusEffects::default()`. No risk in #18b. | Adding a `status_effects` field to `RecruitDef` without thought |

### Trust Boundaries

For the recommended architecture, the trust boundary surface is **the three RON files in `assets/town/`** plus the unchanged `core.items.ron` (already cleared in #18a):

- **`core.town_services.ron` `temple_*` fields** — clamp range `[1, MAX_TEMPLE_COST = 100_000]` per cost; clamp `temple_cure_costs.len() <= 32` (more variants than the enum has).
  Skipped: free revive (cost = 0) or unusable Temple (cost = u32::MAX). Both authoring mistakes.
- **`core.recruit_pool.ron` `recruits` vec length** — clamp to `MAX_RECRUIT_POOL = 32` before paint iteration.
  Skipped: paint-loop time explosion if a crafted RON declared millions of entries.
- **`core.recruit_pool.ron` `base_stats` per-channel** — clamp each channel `[0, 25]` (Wizardry-style ceiling). `derive_stats` already saturates at u32; this is genre-appropriate balance, not crash safety.
  Skipped: a 65535-strength recruit dominating combat trivially.

## Performance

| Metric | Value / Range | Source | Notes |
|---|---|---|---|
| `paint_temple` per frame | < 1 ms | Inherits from #18a research §Performance for Inn/Shop painters | Five party-member rows + header; no allocation hot loop |
| `paint_guild` per frame | < 1 ms (5 recruits in pool) | Same | Pool size capped at 32 |
| `handle_temple_action` cost | One `StatusEffects.effects.retain` + one `EquipmentChangedEvent.write` | `inn.rs:153-162` precedent | O(active party members) for the retain, O(1) for the write |
| `handle_guild_recruit` cost | `commands.spawn` + `.insert` (deferred) + slot scan | `party/mod.rs:131-146` precedent | O(party_size) for slot scan; spawn is constant |
| `recompute_derived_stats_on_equipment_change` | Re-derives one character's stats per event | `inventory.rs:444-501` | O(party_size) iteration over event reader; each derive_stats is O(equipment slots + status effects) = O(8 + ≤10) = trivial |

_(No benchmarks needed — all operations are trivially bounded.)_

## Code Examples

### Authored `core.town_services.ron` for #18b

```ron
// Source: derived from #18a's existing core.town_services.ron (line counts authored against MAX_TEMPLE_COST = 100_000)
(
    inn_rest_cost: 10,
    inn_rest_cures: [Poison],

    // Temple — Feature #18b authoring.
    temple_revive_cost_base: 100,
    temple_revive_cost_per_level: 50,
    // Cure costs per status: Dead is intentionally NOT in this list (revive mode owns it).
    temple_cure_costs: [
        (Stone, 250),
        (Paralysis, 100),
        (Sleep, 50),
    ],
)
```

### Pure helper test for `revive_cost`

```rust
// Source: pattern from src/data/town.rs::tests::stock_filters_by_min_floor
#[test]
fn revive_cost_scales_linearly_with_level() {
    let services = TownServices {
        temple_revive_cost_base: 100,
        temple_revive_cost_per_level: 50,
        ..Default::default()
    };
    assert_eq!(revive_cost(&services, 1), 150);  // 100 + 50*1
    assert_eq!(revive_cost(&services, 5), 350);  // 100 + 50*5
    assert_eq!(revive_cost(&services, 0), 100);  // base alone
}

#[test]
fn revive_cost_saturates_at_max_temple_cost() {
    let services = TownServices {
        temple_revive_cost_base: u32::MAX,
        temple_revive_cost_per_level: u32::MAX,
        ..Default::default()
    };
    assert_eq!(revive_cost(&services, 100), MAX_TEMPLE_COST);
}
```

### Layer-2 integration test mirror (Inn → Temple)

```rust
// Source: directly modelled on src/plugins/town/inn.rs:282-330 rest_full_heals_living_party
#[test]
fn revive_dead_member_clears_dead_and_sets_hp_to_1() {
    let mut app = make_temple_test_app();
    app.world_mut().resource_mut::<Gold>().0 = 1000;

    let dead = spawn_party_member(
        &mut app, 20, 10,
        vec![ActiveEffect {
            effect_type: StatusEffectType::Dead,
            remaining_turns: None,
            magnitude: 0.0,
        }],
    );
    app.world_mut().get_mut::<DerivedStats>(dead).unwrap().current_hp = 0;

    // Set TempleMode::Revive, target index 0.
    app.world_mut().resource_mut::<TempleState>().mode = TempleMode::Revive;
    press_confirm(&mut app);
    // Two updates: handler fires the EquipmentChangedEvent;
    // recompute_derived_stats_on_equipment_change runs same-frame in Update.
    app.update();

    let status = app.world().get::<StatusEffects>(dead).unwrap();
    let derived = app.world().get::<DerivedStats>(dead).unwrap();
    assert!(!status.has(StatusEffectType::Dead), "Dead must be removed");
    assert_eq!(derived.current_hp, 1, "Revive sets HP to 1, not max");
    assert!(derived.max_hp > 0, "max_hp must be re-derived from non-zero formula");
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|---|---|---|---|
| Per-character `Gold` purse | Party-wide `Resource<Gold>` | #18a (user decision 2) | Temple charges the party purse — no per-character calculation |
| `Vec<Entity>` party with despawn-on-dismiss | `Resource<DismissedPool>` keeps entity alive (Option A) | This PR (#18b) | Preserves XP/equipment history, save-friendlier, zero query churn |
| Bevy 0.17 `Event` derive | Bevy 0.18 `Message` derive (Bevy family rename) | Project-wide pre-#11 | `EquipmentChangedEvent` already uses `#[derive(Message)]` at `inventory.rs:216`; no migration needed |

**Deprecated/outdated:**

- The #18a research's reference at line 813 to "use `world.spawn_batch(inv.0.iter().map(|e| commands.entity(e).despawn()))`" for inventory cleanup on dismiss is **no longer relevant** under Option A — Option A doesn't despawn dismissed members. The Pitfall 9 dismiss section of #18a research is rendered MOOT by adopting Option A. **Surface for planner: this is a deliberate scope reduction enabled by Option A.**

## Validation Architecture

### Test Framework

| Property | Value |
|---|---|
| Framework | Bevy `MinimalPlugins` + `StatesPlugin` + `AssetPlugin` + leafwing `ActionState` bare-resource bypass |
| Config file | None — Bevy's `#[test]` + `App::new()` pattern |
| Quick run command | `cargo test -p druum town::temple --lib` |
| Full suite command | `cargo test` (260 lib + 6 integration baseline) / `cargo test --features dev` (264 lib + 6 integration baseline) |

### Baseline tests (#18a end-state)

- **Default features:** 260 lib + 6 integration tests pass. (Verified GREEN on 2026-05-11 per #18a orchestrator summary line 105.)
- **`--features dev`:** 264 lib + 6 integration tests pass. (Same source, line 106.)

The 4-test delta between `dev` and default is the F4 `grant_gold_on_f4` system plus F9 cycler plus `dev`-only spawn-debug-party suite. Temple/Guild tests should be feature-agnostic where possible.

### Requirements → Test Map

| Requirement | Behavior | Test Type | Automated Command | File Exists? |
|---|---|---|---|---|
| Temple `revive_cost` formula is `base + per_level * level` saturating | Pure helper | unit | `cargo test -p druum town::temple::tests::revive_cost_scales_linearly_with_level` | NO — needs creating |
| Temple `revive_cost` saturates at `MAX_TEMPLE_COST` | Pure helper | unit | `cargo test -p druum town::temple::tests::revive_cost_saturates_at_max` | NO |
| Temple Revive removes `Dead` AND sets `current_hp = 1` | Integration | layer 2 | `cargo test -p druum town::temple::tests::revive_dead_member_clears_dead_and_sets_hp_to_1` | NO |
| Temple Revive fires `EquipmentChangedEvent { slot: None }` | Integration (proxy HP assertion mirroring inn.rs:441-451) | layer 2 | `cargo test -p druum town::temple::tests::revive_triggers_recompute` | NO |
| Temple Revive rejects when target is not Dead | Integration | layer 2 | `cargo test -p druum town::temple::tests::revive_rejects_non_dead_target` | NO |
| Temple Cure removes Stone for gold (50% of total cost rate) | Integration | layer 2 | `cargo test -p druum town::temple::tests::cure_stone_removes_status_and_deducts_gold` | NO |
| Temple Cure does NOT remove Dead (Revive owns Dead) | Integration | layer 2 | `cargo test -p druum town::temple::tests::cure_mode_does_not_handle_dead` | NO |
| Temple charges gold only on success | Integration | layer 2 | `cargo test -p druum town::temple::tests::cure_rejects_when_insufficient_gold` | NO |
| Guild Recruit spawns `PartyMember` with correct bundle fields | Integration | layer 2 | `cargo test -p druum town::guild::tests::recruit_spawns_party_member` | NO |
| Guild Recruit picks lowest free `PartySlot` | Integration | layer 2 | `cargo test -p druum town::guild::tests::recruit_picks_lowest_free_slot_after_dismissal` | NO |
| Guild Recruit rejects when party is full | Integration | layer 2 | `cargo test -p druum town::guild::tests::recruit_rejects_when_party_full` | NO |
| Guild Recruit attaches empty `Inventory` | Integration | layer 2 | `cargo test -p druum town::guild::tests::recruit_attaches_empty_inventory` | NO |
| Guild Dismiss removes `PartyMember` marker and adds to `DismissedPool` | Integration | layer 2 | `cargo test -p druum town::guild::tests::dismiss_removes_marker_and_adds_to_pool` | NO |
| Guild Dismiss preserves `Inventory.0` entities (Option A) | Integration | layer 2 | `cargo test -p druum town::guild::tests::dismiss_preserves_inventory_entities` | NO |
| Guild Dismiss rejects last-active-member dismissal | Integration | layer 2 | `cargo test -p druum town::guild::tests::dismiss_rejects_last_active` | NO |
| Guild Row Swap toggles Front/Back | Integration | layer 2 | `cargo test -p druum town::guild::tests::row_swap_toggles_front_back` | NO |
| Guild Slot Reorder swaps two `PartySlot` values | Integration | layer 2 | `cargo test -p druum town::guild::tests::slot_reorder_swaps_two_members` | NO |
| `core.town_services.ron` parses with non-zero `temple_*` fields | Unit (RON round-trip) | unit | `cargo test -p druum data::town::tests::town_services_round_trips_with_authored_temple_fields` | NO |
| `clamp_recruit_pool` truncates oversized pool | Unit | unit | `cargo test -p druum data::town::tests::recruit_pool_size_clamped` | NO |
| Six quality gates remain GREEN | All cargo gates | gate | (the six commands from #18a Phase 11) | YES — gates exist |

### Gaps (files to create before implementation)

- [ ] `src/plugins/town/temple.rs` — Temple painter + handler + helpers + tests (~250 LOC)
- [ ] `src/plugins/town/guild.rs` — Guild painter + handlers (recruit/dismiss/reorder/row-swap) + `DismissedPool` resource + tests (~350 LOC)
- [ ] `src/plugins/town/placeholder.rs` — **DELETE this file** (~144 LOC removed)
- [ ] `assets/town/core.town_services.ron` — author concrete `temple_*` values (replace `#[serde(default)]` zeros with `100 / 50 / [(Stone, 250), ...]`)
- [ ] `src/plugins/town/mod.rs` — register `paint_temple` / `handle_temple_action` and `paint_guild` / `handle_guild_*` in their respective schedules; init `TempleState`, `GuildState`, `DismissedPool` resources; remove `placeholder.rs` import + registrations
- [ ] `src/data/town.rs` — add `clamp_recruit_pool` helper + `MAX_RECRUIT_POOL = 32` + `MAX_TEMPLE_COST = 100_000` constants + related tests

## Open Questions

1. **Cut dismissed-pool from #18b entirely? (Tier A — user decision)**
   - What we know: Roadmap §18 explicitly lists "active 4-6 + dismissed pool" as Guild scope. But `spawn_default_debug_party` is the only spawn path today and v1 has no save/load. The dismiss mechanic doesn't unblock any in-tree feature for several PRs.
   - What's unclear: Does the user want a "complete" Guild in this PR, or a minimum-viable Guild (Recruit + Row Swap + Reorder, no Dismiss) that scopes Dismiss to #19 where Character Creation already needs party-roster mechanics?
   - Recommendation: **Surface as Open Question 1.** If user picks "ship Dismiss now," go with Option A (recommended above). If user picks "defer Dismiss to #19," drop ~120 LOC + 4 tests from #18b and `DismissedPool` resource entirely.

2. **Which severe statuses does Temple cure in v1? (Tier A — user decision, Category C)**
   - What we know: `StatusEffectType` has 5 v1 + 5 #14 variants (`character.rs:258-272`). The five "severe" candidates are `Stone`, `Paralysis`, `Sleep`, `Dead` (Revive owns), `Silence`. The five #14 buff/regen variants (`AttackUp`/`DefenseUp`/`SpeedUp`/`Regen`/`Silence`) are different categories. Inn already cures `Poison`.
   - What's unclear: The roadmap says "cure stone, poison, etc for gold" — non-specific. `Silence` is gameplay-novel (Wizardry doesn't have it as a Temple-curable; it's a #14 addition). `Sleep` and `Paralysis` are typically Inn-rest in Wizardry (clear after sleep). Strict-Wizardry would be Temple cures `Stone` + `Dead` only.
   - Recommendation: **Surface as Open Question 2.** Recommended set: `Dead` (Revive), `Stone`, `Paralysis`, `Sleep`. NOT `Poison` (already Inn). NOT `Silence` (deferred, no in-dungeon source yet). NOT buff variants. Document in `temple_cure_costs` RON.

3. **Slot reorder semantics — swap or shift? (Tier B — planner-resolvable but worth flagging)**
   - What we know: Wizardry uses swap; Etrian Odyssey uses shift; the roadmap is silent.
   - What's unclear: Player feel.
   - Recommendation: **SWAP** for simplicity. Two-component-write operation, type-safe, can be polished to SHIFT in #25 if play-testing shows demand. Tag as Open Question 3 if user wants to weigh in.

4. **Recruit while party empty — allow or block? (Tier B — planner-resolvable)**
   - What we know: Pitfall 9 — production builds (no `dev` feature) ship with no debug party. Player can technically reach Town → Guild with zero `PartyMember`s.
   - What's unclear: Should Guild block recruit on `count >= party_size` only, or also block on `count == 0` (forcing some other path to give the player their first member)?
   - Recommendation: **Allow recruit from empty** — forward-compatible with #19 (character creation also spawns the first member). The "min-1-active" check applies to Dismiss only (don't let the player dismiss themselves into oblivion), not Recruit.

5. **Per-status cure cost authoring values (Tier C — planner-resolvable, balance question)**
   - What we know: Wizardry I/II reference costs (1981–1982): `KADORTO` (full revive) = 200 gold × level; `MADI` (cure stone) ~250 gold; `DI` (low-level cure) much cheaper. Modern indie DRPGs typically use 50–500 gold scale.
   - What's unclear: Whether Druum's economy supports those numbers. #18a shop_stock has 7 items unpriced (all use `value` from `core.items.ron`); the `value` field values are unknown from this research scope (Phase 1 of #18b planning should sanity-check).
   - Recommendation: Place-holder set: `Stone = 250`, `Paralysis = 100`, `Sleep = 50`, `temple_revive_cost_base = 100`, `temple_revive_cost_per_level = 50`. Easy to retune in `core.town_services.ron` without code change.

6. **Should Cure mode prompt for which status to cure, or auto-pick first eligible? (Tier C — planner-resolvable, UX)**
   - What we know: A character can have multiple severe statuses simultaneously (e.g., `Stone + Sleep`).
   - What's unclear: UX preference.
   - Recommendation: **Auto-pick first eligible** in v1 (matches the simplicity bar of Inn rest). A dedicated "pick which status to cure" sub-menu is #25 polish. Document at top of `temple.rs`.

7. **`DismissedPool` save-format readiness (Tier B — planner-resolvable, deferred to #23 anyway)**
   - What we know: `Inventory(Vec<Entity>)` at `inventory.rs:187` already has the contract "Feature #23 must implement `MapEntities`" baked in. `DismissedPool(Vec<Entity>)` would inherit the same contract.
   - What's unclear: Whether the planner should add `MapEntities` serde glue now or defer.
   - Recommendation: **Defer to #23** — same path as `Inventory`. Document at the type declaration: `// Feature #23 must implement MapEntities to serialize across sessions.`

## Sources

### Primary (HIGH confidence)

- [In-tree: `src/plugins/party/character.rs`](file:///Users/nousunio/Repos/Learnings/claude-code/druum/src/plugins/party/character.rs) — `PartyMemberBundle`, `BaseStats`, `DerivedStats`, `Equipment`, `Experience`, `PartySlot`, `PartyRow`, `PartySize`, `StatusEffects`, `StatusEffectType`, `derive_stats` — all already shipped.
- [In-tree: `src/plugins/party/inventory.rs`](file:///Users/nousunio/Repos/Learnings/claude-code/druum/src/plugins/party/inventory.rs) — `Inventory`, `ItemInstance`, `EquipmentChangedEvent`, `recompute_derived_stats_on_equipment_change` (filter-free, lines 444-501).
- [In-tree: `src/plugins/combat/status_effects.rs`](file:///Users/nousunio/Repos/Learnings/claude-code/druum/src/plugins/combat/status_effects.rs) — `apply_status_handler` is the canonical mutator; Temple takes a documented exception path for cure/revive.
- [In-tree: `src/plugins/town/inn.rs`](file:///Users/nousunio/Repos/Learnings/claude-code/druum/src/plugins/town/inn.rs) — direct precedent for Temple's painter + handler shape; `effects.retain` pattern at line 154-156.
- [In-tree: `src/plugins/town/mod.rs`](file:///Users/nousunio/Repos/Learnings/claude-code/druum/src/plugins/town/mod.rs) — `TownPlugin` registration pattern; placeholder painter at lines 106-107 + 122-123 is the deletion site.
- [In-tree: `src/plugins/town/placeholder.rs`](file:///Users/nousunio/Repos/Learnings/claude-code/druum/src/plugins/town/placeholder.rs) — DELETE this on #18b.
- [In-tree: `src/data/town.rs`](file:///Users/nousunio/Repos/Learnings/claude-code/druum/src/data/town.rs) — `TownServices`, `RecruitDef`, `RecruitPool`, `ShopStock` — all pre-shipped.
- [In-tree: `assets/town/core.town_services.ron`](file:///Users/nousunio/Repos/Learnings/claude-code/druum/assets/town/core.town_services.ron) — file to author `temple_*` fields into.
- [In-tree: `assets/town/core.recruit_pool.ron`](file:///Users/nousunio/Repos/Learnings/claude-code/druum/assets/town/core.recruit_pool.ron) — 5 recruits, ready for `#18b` Guild.
- [In-tree: `src/plugins/state/mod.rs`](file:///Users/nousunio/Repos/Learnings/claude-code/druum/src/plugins/state/mod.rs) — `TownLocation::{Temple,Guild}` already declared.
- [In-tree: `src/plugins/loading/mod.rs`](file:///Users/nousunio/Repos/Learnings/claude-code/druum/src/plugins/loading/mod.rs) — `TownAssets` collection (lines 95-103) — already wires `RecruitPool` + `TownServices`.
- [Prior research: `project/research/20260511-feature-18-town-hub-and-services.md`](file:///Users/nousunio/Repos/Learnings/claude-code/druum/project/research/20260511-feature-18-town-hub-and-services.md) — comprehensive coverage of all 5 town screens including Temple/Guild patterns at lines 156, 571-682, 1083-1129, 1207-1221. Carries forward unchanged for #18b.
- [Prior plan: `project/plans/20260511-180000-feature-18a-town-square-shop-inn.md`](file:///Users/nousunio/Repos/Learnings/claude-code/druum/project/plans/20260511-180000-feature-18a-town-square-shop-inn.md) — "Deferred to #18b" section (lines 99-117) — explicitly lists every Temple/Guild requirement.
- [Prior implementation summary: `project/implemented/20260511-190000-feature-18a-town-square-shop-inn.md`](file:///Users/nousunio/Repos/Learnings/claude-code/druum/project/implemented/20260511-190000-feature-18a-town-square-shop-inn.md) — `test_count: 35 new tests added in #18a`.
- [Prior orchestrator summary: `project/orchestrator/20260511-215508-feature-18a-town-square-shop-inn.md`](file:///Users/nousunio/Repos/Learnings/claude-code/druum/project/orchestrator/20260511-215508-feature-18a-town-square-shop-inn.md) — 260 lib + 6 integration baseline (default), 264 lib + 6 integration baseline (dev). Used as #18b baseline.
- [Roadmap §18, lines 965-1025](file:///Users/nousunio/Repos/Learnings/claude-code/druum/project/roadmaps/20260429-01-bevy-dungeon-crawler-roadmap.md) — Temple + Guild scope verified.

### Secondary (MEDIUM confidence)

- Memory: `reference_druum_equipment_changed_event_dual_use.md` — confirms `recompute_derived_stats_on_equipment_change` reads `&StatusEffects` (verified in source at `inventory.rs:447-453`).
- Memory: `reference_druum_recompute_filter_dual_use.md` — confirms `With<PartyMember>` filter was dropped in #15 D-A5 (verified in source at `inventory.rs:435-444` and the doc-comment at `inventory.rs:435-443`).
- Memory: `reference_druum_18_town_dependencies_pre_shipped.md` — confirms `TownLocation` SubStates declared, `ItemAsset.value` pre-authored, `MenuAction` reused. Verified in source.
- Wizardry I/II reference (cultural precedent, not load-bearing): revive cost = `200 × level` was the original 1981 formula; modern indie DRPGs use 50–500 scale per service.

### Tertiary (LOW confidence)

- None. All claims trace to in-tree code or prior pipeline artifacts.

## Metadata

**Confidence breakdown:**

- Standard stack: HIGH — no new dependencies; all crates already pinned and exercised by #18a.
- Architecture (dismissed-pool shape): MEDIUM-HIGH for Option A — the recommendation is defensible but represents one of three viable shapes. Counterargument-tested. Surfaced as Open Question 1 (cut-scope is the orthogonal escape).
- Status-effect cure boundary: MEDIUM — Wizardry precedent says Stone-only; modern genre says broader. The recommended set (Dead/Stone/Paralysis/Sleep) is defensible but a balance choice, not a correctness one. Surfaced as Open Question 2.
- Revive cost formula: HIGH — the `temple_revive_cost_base + temple_revive_cost_per_level * level` shape was pre-baked by #18a's RON schema. The recommended values are tuning, not architecture.
- `PartySlot` / `PartyRow` semantics: HIGH — both verified in-tree at `character.rs:184-191` (PartySlot is `pub usize`) and `character.rs:175-182` (PartyRow is `enum { Front, Back }`). No uniqueness enforcement at the type level (Pitfall 6).
- Recruit-spawn pattern: HIGH — `spawn_default_debug_party` at `party/mod.rs:131-146` is the in-tree precedent; `PartyMemberBundle` + `.insert(Inventory::default())` shape is verified.
- Quality-gate template: HIGH — 6 gates verified GREEN on 2026-05-11 in #18a orchestrator summary; baselines locked at 260/6 default + 264/6 dev.
- Pitfalls: HIGH — 10 pitfalls grounded in either #18a research, in-tree source, or both. Pitfall 9 (production builds have no debug party) is the most likely to surprise the planner.

**What NOT to do in #18b (anti-scope-creep guide for the planner):**

- **Do NOT implement character creation** (race/class/name picker, point-buy/rolled stats). That is Feature #19's entire scope. #18b recruits from the static pool only.
- **Do NOT implement `MapEntities` for `DismissedPool` or `Inventory`.** Save/load is Feature #23. The `// Feature #23 must implement MapEntities` doc-comment is the entire forward-compat work.
- **Do NOT rework `Camera2d` / `PrimaryEguiContext` lifecycle.** It is already correct in `town/mod.rs:63-75`. Temple/Guild reuse the existing camera.
- **Do NOT modify `Square` menu options** or `SQUARE_MENU_OPTIONS` array. Temple/Guild routing already wired at `square.rs:25, 110-111`.
- **Do NOT touch `Cargo.toml`.** Δ deps = 0 (verified).
- **Do NOT touch `core.recruit_pool.ron`.** 5 recruits already authored. Updating ages = #19/#21 polish.
- **Do NOT touch `inn.rs`, `shop.rs`, or `square.rs`** except possibly to re-verify their `EguiPrimaryContextPass` painter + `Update` handler split is preserved when Temple/Guild are added to the same tuples.
- **Do NOT change `PartySize::default() = 4`.** Hard cap is locked by Decision 6 of #11 (`character.rs:339-351`).
- **Do NOT collapse `PartySlot` and `PartyMember` into one component.** They are intentionally separate (Decision 5 of #11; `character.rs:184-191`).
- **Do NOT touch `apply_status_handler` in `combat/status_effects.rs`.** Temple takes a documented exception (direct `effects.retain` + manual `EquipmentChangedEvent`), not a routing through that handler.
- **Do NOT add a `DismissedTime` / "rejoin after N days" mechanic.** Roadmap doesn't ask for it. `DismissedPool` is just a registry.
- **Do NOT add Tab-key cycling for character switching.** That is Feature #25 polish per `shop.rs:5-8` and `square.rs:8-11` doc comments.
- **Do NOT add multi-status-pick cure UI.** v1 auto-picks first eligible (Open Question 6).
- **Do NOT add status-cure for buff variants** (`AttackUp`/`DefenseUp`/`SpeedUp`/`Regen`). They aren't "afflictions". They tick naturally via `tick_status_durations`.

**Research date:** 2026-05-12
