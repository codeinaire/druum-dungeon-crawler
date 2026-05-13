//! Town Guild screen — party roster management (recruit, dismiss, row swap, slot swap).
//!
//! ## Architecture (Option A — DismissedPool resource)
//!
//! Dismiss removes the `PartyMember` marker via `commands.entity(e).remove::<PartyMember>()`,
//! leaving the entity alive. The entity (with all its components — XP, Equipment, Inventory)
//! is pushed into `DismissedPool.entities`. This preserves the `Inventory(Vec<Entity>)`
//! chain: the `ItemInstance` entities inside the bag are not orphaned.
//!
//! Re-recruit from `DismissedPool` is deferred to Feature #18b polish (#19+).
//! In v1, re-recruit spawns a FRESH entity from `RecruitPool` — the dismissed
//! entity remains in the pool for future save/load work.
//!
//! ## Minimum-1-active asymmetry
//!
//! - **Recruit** has NO minimum-active check (forward-compat with #19 Character
//!   Creation where the active roster starts empty). Recruiting from an empty
//!   active party is allowed.
//! - **Dismiss** requires `active_count > 1` to prevent emptying the roster
//!   (would leave no party members to advance into the dungeon).
//!
//! ## Slot reorder semantics
//!
//! Slot swap uses SWAP semantics (user decision 5): two `PartySlot` writes exchange
//! values between two `With<PartyMember>` entities. Two-press UX: first `S` pins
//! the source entity; second `S` resolves the target and performs the swap.
//!
//! ## RecruitDef status effects
//!
//! `RecruitDef` does NOT include a `status_effects` field — recruits spawn with
//! `StatusEffects::default()`. Adding a `status_effects` field to `RecruitDef`
//! later would require Temple-cure-cost-drain risk analysis before shipping.
//!
//! ## Dismissed entity persistence
//!
//! Dismissed entities are NEVER despawned. Despawning would orphan `Inventory.0`'s
//! `ItemInstance` entities (no GC; they would leak). The entity stays alive in the
//! ECS world with all components attached; only the `PartyMember` marker is removed.
//!
//! ## Feature #23 save/load contract
//!
//! `DismissedPool.entities: Vec<Entity>` does not naturally serialize across sessions.
//! Feature #23 must implement `MapEntities` for this resource — same deferral contract
//! as `Inventory(Vec<Entity>)` at `src/plugins/party/inventory.rs:179-181`.
//! Do NOT add `Serialize`/`Deserialize` to `DismissedPool` until #23.

use std::collections::HashSet;

use bevy::prelude::*;
use bevy_egui::{EguiContexts, egui};
use leafwing_input_manager::prelude::ActionState;

use crate::data::town::{MAX_RECRUIT_POOL, RecruitPool, clamp_recruit_pool};
use crate::plugins::input::MenuAction;
use crate::plugins::loading::TownAssets;
use crate::plugins::party::character::{
    CharacterName, Class, Experience, PartyMember, PartyMemberBundle,
    PartyRow, PartySize, PartySlot, Race, StatusEffectType, StatusEffects, derive_stats,
};
use crate::plugins::party::inventory::Inventory;
use crate::plugins::state::TownLocation;
use crate::plugins::town::gold::{GameClock, Gold};

// ─────────────────────────────────────────────────────────────────────────────
// DismissedPool resource
// ─────────────────────────────────────────────────────────────────────────────

/// Registry of dismissed party member entities (Option A architecture).
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

/// Pool indices that have already been recruited. Prevents the same `RecruitDef`
/// from being recruited twice in a single play session.
///
/// **Feature #23 save/load contract:** mirrors `DismissedPool` — `HashSet<usize>`
/// is straightforward to serialize, but the actual save format is owned by #23.
/// Doc-comment only here; no `Serialize`/`Deserialize` derives in #18b.
#[derive(Resource, Default, Debug)]
pub struct RecruitedSet {
    pub indices: std::collections::HashSet<usize>,
}

// ─────────────────────────────────────────────────────────────────────────────
// GuildState resource
// ─────────────────────────────────────────────────────────────────────────────

/// UI cursor state for the Guild screen.
#[derive(Resource, Default, Debug)]
pub struct GuildState {
    pub mode: GuildMode,
    /// Cursor index into the current mode's list (active party or recruit pool).
    pub cursor: usize,
}

/// Which panel the Guild is showing.
#[derive(Debug, PartialEq, Eq, Clone, Copy, Default)]
pub enum GuildMode {
    /// Browse active party — supports Dismiss, Row swap, Slot reorder.
    #[default]
    Roster,
    /// Browse RecruitPool entries — Confirm spawns a new PartyMember.
    Recruit,
}

// ─────────────────────────────────────────────────────────────────────────────
// Pure helper — next-free-slot
// ─────────────────────────────────────────────────────────────────────────────

/// Return the lowest unused slot index in `0..party_size`, or `None` if all
/// slots are occupied.
///
/// `used` is the list of currently-occupied slot indices. Out-of-range entries
/// in `used` are harmless (they simply won't match any `0..party_size` candidate).
pub fn next_free_slot(used: &[usize], party_size: usize) -> Option<usize> {
    let used_set: HashSet<usize> = used.iter().copied().collect();
    (0..party_size).find(|i| !used_set.contains(i))
}

// ─────────────────────────────────────────────────────────────────────────────
// paint_guild — EguiPrimaryContextPass, read-only
// ─────────────────────────────────────────────────────────────────────────────

/// Render the Guild screen.
///
/// **Read-only** — no `ResMut<T>` or `Commands`. All mutations live in the
/// five handler systems.
#[allow(clippy::too_many_arguments)]
#[allow(clippy::type_complexity)]
pub fn paint_guild(
    mut contexts: EguiContexts,
    gold: Res<Gold>,
    guild_state: Res<GuildState>,
    clock: Res<GameClock>,
    recruited: Res<RecruitedSet>,
    town_assets: Option<Res<TownAssets>>,
    pool_assets: Res<Assets<RecruitPool>>,
    party: Query<(Entity, &CharacterName, &Race, &Class, &Experience, &PartySlot, &PartyRow, &StatusEffects), With<PartyMember>>,
) -> Result {
    let ctx = contexts.ctx_mut()?;

    egui::TopBottomPanel::top("guild_header").show(ctx, |ui| {
        ui.horizontal(|ui| {
            let mode_label = match guild_state.mode {
                GuildMode::Roster => "Guild — Roster",
                GuildMode::Recruit => "Guild — Recruit",
            };
            ui.heading(mode_label);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(format!("{} Gold  |  Day {}", gold.0, clock.day));
            });
        });
    });

    egui::CentralPanel::default().show(ctx, |ui| {
        match guild_state.mode {
            GuildMode::Roster => {
                // Sort by (PartySlot, Entity) for determinism.
                let mut members: Vec<(Entity, &CharacterName, &Race, &Class, &Experience, &PartySlot, &PartyRow, &StatusEffects)> =
                    party.iter().collect();
                members.sort_by_key(|(e, _, _, _, _, slot, _, _)| (slot.0, *e));

                if members.is_empty() {
                    ui.label("(No active members — press R to recruit)");
                } else {
                    for (idx, (_, name, race, class, xp, slot, row, status)) in
                        members.iter().enumerate()
                    {
                        let cursor_marker = if idx == guild_state.cursor { "> " } else { "  " };
                        let status_str = if status.has(StatusEffectType::Dead) {
                            " [Dead]"
                        } else {
                            ""
                        };
                        ui.label(format!(
                            "{}{} — {:?} {:?} Lv{} Slot:{} Row:{:?}{}",
                            cursor_marker,
                            name.0,
                            race,
                            class,
                            xp.level,
                            slot.0,
                            row,
                            status_str,
                        ));
                    }
                }

                ui.add_space(8.0);
                ui.label("[Up/Down] Pick  |  [G] Dismiss  |  [F] Toggle Row  |  [T] Slot Swap target  |  [R] Recruit mode  |  [Esc] Back");
            }
            GuildMode::Recruit => {
                let recruit_pool = town_assets
                    .as_ref()
                    .and_then(|a| pool_assets.get(&a.recruit_pool));

                match recruit_pool {
                    None => {
                        ui.label("(loading recruit pool...)");
                    }
                    Some(pool) => {
                        let recruits = clamp_recruit_pool(pool, MAX_RECRUIT_POOL);
                        if recruits.is_empty() {
                            ui.label("(Recruit pool is empty)");
                        } else {
                            for (idx, recruit) in recruits.iter().enumerate() {
                                let cursor_marker =
                                    if idx == guild_state.cursor { "> " } else { "  " };
                                let taken_marker =
                                    if recruited.indices.contains(&idx) { "  (recruited)" } else { "" };
                                ui.label(format!(
                                    "{}{} — {:?} {:?}  STR:{} INT:{} PIE:{} VIT:{} AGL:{} LCK:{}{}",
                                    cursor_marker,
                                    recruit.name,
                                    recruit.race,
                                    recruit.class,
                                    recruit.base_stats.strength,
                                    recruit.base_stats.intelligence,
                                    recruit.base_stats.piety,
                                    recruit.base_stats.vitality,
                                    recruit.base_stats.agility,
                                    recruit.base_stats.luck,
                                    taken_marker,
                                ));
                            }
                        }
                    }
                }

                ui.add_space(8.0);
                ui.label("[Up/Down] Pick  |  [Enter] Recruit  |  [R] Back to Roster  |  [Esc] Back");
            }
        }
    });

    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// Handler 1 — handle_guild_input (top-level navigation)
// ─────────────────────────────────────────────────────────────────────────────

/// Top-level navigation handler for the Guild screen.
///
/// Handles Cancel (→ Square), Up/Down (cursor), and R (toggle Roster/Recruit mode).
#[allow(clippy::too_many_arguments)]
pub fn handle_guild_input(
    actions: Res<ActionState<MenuAction>>,
    mut guild_state: ResMut<GuildState>,
    mut next_sub: ResMut<NextState<TownLocation>>,
    party: Query<(), With<PartyMember>>,
    town_assets: Option<Res<TownAssets>>,
    pool_assets: Res<Assets<RecruitPool>>,
) {
    if actions.just_pressed(&MenuAction::Cancel) {
        next_sub.set(TownLocation::Square);
        return;
    }

    // R toggles between Roster and Recruit mode (MenuAction::Recruit, KeyR).
    if actions.just_pressed(&MenuAction::Recruit) {
        guild_state.mode = match guild_state.mode {
            GuildMode::Roster => GuildMode::Recruit,
            GuildMode::Recruit => GuildMode::Roster,
        };
        guild_state.cursor = 0;
        return;
    }

    // Cursor movement.
    let list_len = match guild_state.mode {
        GuildMode::Roster => party.iter().count(),
        GuildMode::Recruit => {
            town_assets
                .as_ref()
                .and_then(|a| pool_assets.get(&a.recruit_pool))
                .map(|p| clamp_recruit_pool(p, MAX_RECRUIT_POOL).len())
                .unwrap_or(0)
        }
    };

    if actions.just_pressed(&MenuAction::Up) && guild_state.cursor > 0 {
        guild_state.cursor -= 1;
    }
    if actions.just_pressed(&MenuAction::Down) && list_len > 0 {
        guild_state.cursor = (guild_state.cursor + 1).min(list_len.saturating_sub(1));
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Handler 2 — handle_guild_recruit
// ─────────────────────────────────────────────────────────────────────────────

/// Recruit a new party member from the `RecruitPool` on `MenuAction::Confirm`.
///
/// Gated on `GuildMode::Recruit`. Spawns a fresh `PartyMemberBundle` +
/// `Inventory::default()`. No minimum-active check on Recruit (forward-compat
/// with #19 Character Creation where the active roster may start empty).
#[allow(clippy::too_many_arguments)]
pub fn handle_guild_recruit(
    mut commands: Commands,
    actions: Res<ActionState<MenuAction>>,
    mut guild_state: ResMut<GuildState>,
    mut recruited: ResMut<RecruitedSet>,
    mut toasts: ResMut<crate::plugins::town::toast::Toasts>,
    party_size: Res<PartySize>,
    existing_slots: Query<&PartySlot, With<PartyMember>>,
    town_assets: Option<Res<TownAssets>>,
    pool_assets: Res<Assets<RecruitPool>>,
) {
    if guild_state.mode != GuildMode::Recruit {
        return;
    }
    if !actions.just_pressed(&MenuAction::Confirm) {
        return;
    }

    let Some(assets) = town_assets else {
        return;
    };
    let Some(pool) = pool_assets.get(&assets.recruit_pool) else {
        return;
    };

    let recruits = clamp_recruit_pool(pool, MAX_RECRUIT_POOL);
    let Some(recruit) = recruits.get(guild_state.cursor) else {
        info!("Guild recruit: cursor {} out of range (pool len {})", guild_state.cursor, recruits.len());
        return;
    };

    // Dedup guard — each pool index can be recruited at most once per session.
    if recruited.indices.contains(&guild_state.cursor) {
        info!("Guild recruit: '{}' (pool index {}) already recruited", recruit.name, guild_state.cursor);
        toasts.push(format!("{} is already in your party.", recruit.name));
        return;
    }

    // Party-full guard.
    let current_count = existing_slots.iter().count();
    if current_count >= party_size.0 {
        info!("Guild recruit: party full ({}/{})", current_count, party_size.0);
        toasts.push(format!("Party is full ({}/{}).", current_count, party_size.0));
        return;
    }

    // Find the lowest unused slot.
    let used: Vec<usize> = existing_slots.iter().map(|s| s.0).collect();
    let slot = next_free_slot(&used, party_size.0).unwrap_or(0);

    // Derive level-1 stats.
    let derived = derive_stats(
        &recruit.base_stats,
        &[],
        &StatusEffects::default(),
        1,
    );

    commands
        .spawn(PartyMemberBundle {
            name: CharacterName(recruit.name.clone()),
            race: recruit.race,
            class: recruit.class,
            base_stats: recruit.base_stats,
            derived_stats: derived,
            party_row: recruit.default_row,
            party_slot: PartySlot(slot),
            ..Default::default()
        })
        .insert(Inventory::default());

    // Mark this pool index as taken so the painter can show "(recruited)" and
    // the handler rejects a duplicate press.
    recruited.indices.insert(guild_state.cursor);

    // Jump back to roster after recruiting.
    guild_state.mode = GuildMode::Roster;

    toasts.push(format!("{} has joined your party!", recruit.name));
    info!(
        "Guild: recruited '{}' ({:?} {:?}) into slot {}",
        recruit.name, recruit.race, recruit.class, slot
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Handler 3 — handle_guild_dismiss
// ─────────────────────────────────────────────────────────────────────────────

/// Dismiss the cursor-targeted active party member on `KeyCode::D`.
///
/// Gated on `GuildMode::Roster`. Removes the `PartyMember` marker (deferred
/// via Commands) and pushes the entity into `DismissedPool.entities`.
///
/// **Minimum-1-active guard:** rejects if `active_count <= 1` to prevent
/// emptying the roster. Recruit has NO equivalent minimum check
/// (forward-compat with #19 Character Creation).
///
/// **Deferred-command note:** `commands.entity(e).remove::<PartyMember>()` does
/// not take effect until `apply_deferred`. Do not read the party count after
/// queueing — it still reflects the pre-removal state.
pub fn handle_guild_dismiss(
    mut commands: Commands,
    mut pool: ResMut<DismissedPool>,
    guild_state: Res<GuildState>,
    party: Query<(Entity, &PartySlot, &CharacterName), With<PartyMember>>,
    actions: Res<ActionState<MenuAction>>,
    mut toasts: ResMut<crate::plugins::town::toast::Toasts>,
) {
    if guild_state.mode != GuildMode::Roster {
        return;
    }
    if !actions.just_pressed(&MenuAction::Dismiss) {
        return;
    }

    let active_count = party.iter().count();
    if active_count <= 1 {
        info!("Guild dismiss: cannot dismiss the last active member");
        toasts.push("Cannot dismiss the last party member.");
        return;
    }

    // Sort by (PartySlot, Entity) — same order as the painter.
    let mut members: Vec<(Entity, &PartySlot, &CharacterName)> = party.iter().collect();
    members.sort_by_key(|(e, slot, _)| (slot.0, *e));

    let Some(&(target, _, name)) = members.get(guild_state.cursor) else {
        info!(
            "Guild dismiss: cursor {} out of range (party len {})",
            guild_state.cursor,
            members.len()
        );
        return;
    };
    let name_str = name.0.clone();

    // Remove PartyMember marker (deferred). Do NOT despawn — preserves Inventory chain.
    commands.entity(target).remove::<PartyMember>();
    pool.entities.push(target);
    toasts.push(format!("Dismissed {name_str}."));
    info!("Guild: dismissed {:?}", target);
}

// ─────────────────────────────────────────────────────────────────────────────
// Handler 4 — handle_guild_row_swap
// ─────────────────────────────────────────────────────────────────────────────

/// Toggle the `PartyRow` of the cursor-targeted member on `KeyCode::F`.
///
/// Gated on `GuildMode::Roster`. Front → Back, Back → Front.
pub fn handle_guild_row_swap(
    guild_state: Res<GuildState>,
    mut party: Query<(Entity, &PartySlot, &CharacterName, &mut PartyRow), With<PartyMember>>,
    actions: Res<ActionState<MenuAction>>,
    mut toasts: ResMut<crate::plugins::town::toast::Toasts>,
) {
    if guild_state.mode != GuildMode::Roster {
        return;
    }
    if !actions.just_pressed(&MenuAction::RowSwap) {
        return;
    }

    let mut members: Vec<(Entity, usize)> = party
        .iter()
        .map(|(e, slot, _, _)| (e, slot.0))
        .collect();
    members.sort_by_key(|(e, slot)| (*slot, *e));

    let Some(&(target, _)) = members.get(guild_state.cursor) else {
        return;
    };

    if let Ok((_, _, name, mut row)) = party.get_mut(target) {
        *row = match *row {
            PartyRow::Front => PartyRow::Back,
            PartyRow::Back => PartyRow::Front,
        };
        toasts.push(format!("{} moved to {:?} row.", name.0, *row));
        info!("Guild: toggled row for {:?} → {:?}", target, *row);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Handler 5 — handle_guild_slot_swap
// ─────────────────────────────────────────────────────────────────────────────

/// Two-press SWAP UX for slot reordering on `KeyCode::S`.
///
/// Gated on `GuildMode::Roster`. First `S` pins the cursor-targeted entity as
/// the swap source. Second `S` resolves the new cursor target and exchanges
/// their `PartySlot` values.
///
/// **SWAP semantics:** only two writes, never inserting a duplicate slot.
pub fn handle_guild_slot_swap(
    guild_state: Res<GuildState>,
    mut party: Query<(Entity, &mut PartySlot), With<PartyMember>>,
    name_query: Query<&CharacterName, With<PartyMember>>,
    actions: Res<ActionState<MenuAction>>,
    mut toasts: ResMut<crate::plugins::town::toast::Toasts>,
    mut pin: Local<Option<Entity>>,
) {
    if guild_state.mode != GuildMode::Roster {
        return;
    }
    if !actions.just_pressed(&MenuAction::SlotSwap) {
        return;
    }

    // Build the sorted member list (slot, entity) for cursor resolution.
    let mut members: Vec<(Entity, usize)> = party
        .iter()
        .map(|(e, slot)| (e, slot.0))
        .collect();
    members.sort_by_key(|(e, slot)| (*slot, *e));

    let Some(&(cursor_entity, _)) = members.get(guild_state.cursor) else {
        return;
    };

    match *pin {
        None => {
            // First press — pin the source.
            *pin = Some(cursor_entity);
            let name = name_query.get(cursor_entity).map(|n| n.0.clone()).unwrap_or_else(|_| "?".into());
            toasts.push(format!("Slot swap: pinned {name} — press T on another member."));
            info!("Guild slot-swap: pinned source {:?}", cursor_entity);
        }
        Some(source) => {
            // Second press — perform the swap.
            *pin = None;

            if source == cursor_entity {
                // Same entity — no-op.
                toasts.push("Slot swap: cancelled (same member).");
                info!("Guild slot-swap: source and target are the same, cancelling");
                return;
            }

            // Read both slots from the pre-sorted members vec (avoids a second query borrow).
            let slot_source_opt = members.iter().find(|(e, _)| *e == source).map(|(_, s)| *s);
            let slot_target_opt = members.iter().find(|(e, _)| *e == cursor_entity).map(|(_, s)| *s);

            let (Some(s), Some(t)) = (slot_source_opt, slot_target_opt) else {
                info!("Guild slot-swap: failed to resolve slots");
                return;
            };

            let name_a = name_query.get(source).map(|n| n.0.clone()).unwrap_or_else(|_| "?".into());
            let name_b = name_query.get(cursor_entity).map(|n| n.0.clone()).unwrap_or_else(|_| "?".into());

            // Write the swapped values.
            if let Ok((_, mut slot)) = party.get_mut(source) {
                slot.0 = t;
            }
            if let Ok((_, mut slot)) = party.get_mut(cursor_entity) {
                slot.0 = s;
            }

            toasts.push(format!("Swapped {name_a} ↔ {name_b}."));
            info!(
                "Guild slot-swap: exchanged slots {:?}({}) ↔ {:?}({})",
                source, s, cursor_entity, t
            );
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::asset::AssetPlugin;
    use bevy::state::app::StatesPlugin;
    use leafwing_input_manager::prelude::*;

    use bevy::asset::Handle;
    use crate::data::town::{RecruitDef, RecruitPool};
    use crate::plugins::input::MenuAction;
    use crate::plugins::loading::TownAssets;
    use crate::plugins::party::character::{
        BaseStats, Class, Experience, PartyMember, PartyMemberBundle,
        PartyRow, PartySize, PartySlot, Race, StatusEffects, derive_stats,
    };
    use crate::plugins::party::inventory::Inventory;
    use crate::plugins::state::{GameState, TownLocation};
    use crate::plugins::town::gold::{GameClock, Gold};

    fn make_test_recruit_pool() -> RecruitPool {
        let base = BaseStats {
            strength: 10,
            intelligence: 8,
            piety: 8,
            vitality: 12,
            agility: 10,
            luck: 6,
        };
        RecruitPool {
            recruits: vec![
                RecruitDef {
                    name: "Ser Edran".to_string(),
                    race: Race::Human,
                    class: Class::Fighter,
                    base_stats: base,
                    default_row: PartyRow::Front,
                },
                RecruitDef {
                    name: "Lyris".to_string(),
                    race: Race::Elf,
                    class: Class::Mage,
                    base_stats: base,
                    default_row: PartyRow::Back,
                },
                RecruitDef {
                    name: "Brother Talos".to_string(),
                    race: Race::Human,
                    class: Class::Priest,
                    base_stats: base,
                    default_row: PartyRow::Back,
                },
                RecruitDef {
                    name: "Brak Ironfist".to_string(),
                    race: Race::Dwarf,
                    class: Class::Fighter,
                    base_stats: base,
                    default_row: PartyRow::Front,
                },
                RecruitDef {
                    name: "Pip Nimblefoot".to_string(),
                    race: Race::Hobbit,
                    class: Class::Fighter,
                    base_stats: base,
                    default_row: PartyRow::Front,
                },
            ],
        }
    }

    fn make_guild_test_app() -> App {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            StatesPlugin,
        ));
        app.init_state::<GameState>();
        app.add_sub_state::<TownLocation>();

        app.init_resource::<Gold>();
        app.init_resource::<GameClock>();
        app.init_resource::<GuildState>();
        app.init_resource::<DismissedPool>();
        app.init_resource::<RecruitedSet>();
        app.init_resource::<crate::plugins::town::toast::Toasts>();
        app.init_resource::<PartySize>();
        app.init_resource::<ActionState<MenuAction>>();
        app.insert_resource(InputMap::<MenuAction>::default());
        app.init_resource::<ButtonInput<KeyCode>>();

        app.init_asset::<RecruitPool>();
        let pool = make_test_recruit_pool();
        let pool_handle = app
            .world_mut()
            .resource_mut::<Assets<RecruitPool>>()
            .add(pool);

        use crate::data::town::{ShopStock, TownServices};
        app.init_asset::<ShopStock>();
        app.init_asset::<TownServices>();

        let mock_town_assets = TownAssets {
            shop_stock: Handle::default(),
            recruit_pool: pool_handle,
            services: Handle::default(),
        };
        app.insert_resource(mock_town_assets);

        // Register all five guild handler systems.
        app.add_systems(
            Update,
            (
                handle_guild_input.run_if(in_state(TownLocation::Guild)),
                handle_guild_recruit.run_if(in_state(TownLocation::Guild)),
                handle_guild_dismiss.run_if(in_state(TownLocation::Guild)),
                handle_guild_row_swap.run_if(in_state(TownLocation::Guild)),
                handle_guild_slot_swap.run_if(in_state(TownLocation::Guild)),
            ),
        );

        // Transition into Town / Guild.
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Town);
        app.update();
        app.world_mut()
            .resource_mut::<NextState<TownLocation>>()
            .set(TownLocation::Guild);
        app.update();
        app.update();

        app
    }

    fn spawn_active_member(app: &mut App, slot: usize, row: PartyRow) -> Entity {
        let base = BaseStats {
            strength: 10,
            intelligence: 8,
            piety: 8,
            vitality: 12,
            agility: 10,
            luck: 6,
        };
        let derived = derive_stats(&base, &[], &StatusEffects::default(), 1);
        app.world_mut()
            .spawn(PartyMemberBundle {
                name: CharacterName("Test".into()),
                race: Race::Human,
                class: Class::Fighter,
                base_stats: base,
                derived_stats: derived,
                party_row: row,
                party_slot: PartySlot(slot),
                ..Default::default()
            })
            .insert(Inventory::default())
            .id()
    }

    #[allow(dead_code)]
    fn spawn_dismissed_member(app: &mut App, slot: usize, row: PartyRow) -> Entity {
        // Spawns without PartyMember marker; adds entity to DismissedPool.
        let base = BaseStats::default();
        let derived = derive_stats(&base, &[], &StatusEffects::default(), 1);
        let entity = app.world_mut()
            .spawn((
                CharacterName("Dismissed".into()),
                Race::Human,
                Class::Fighter,
                base,
                derived,
                row,
                PartySlot(slot),
                StatusEffects::default(),
                Experience::default(),
                Inventory::default(),
            ))
            .id();
        app.world_mut()
            .resource_mut::<DismissedPool>()
            .entities
            .push(entity);
        entity
    }

    fn press_confirm(app: &mut App) {
        app.world_mut()
            .resource_mut::<ActionState<MenuAction>>()
            .press(&MenuAction::Confirm);
        app.update();
        app.world_mut()
            .resource_mut::<ActionState<MenuAction>>()
            .release(&MenuAction::Confirm);
        app.update();
    }

    fn press_cancel(app: &mut App) {
        app.world_mut()
            .resource_mut::<ActionState<MenuAction>>()
            .press(&MenuAction::Cancel);
        app.update();
        app.world_mut()
            .resource_mut::<ActionState<MenuAction>>()
            .release(&MenuAction::Cancel);
        app.update();
    }

    fn press_key_d(app: &mut App) {
        app.world_mut()
            .resource_mut::<ActionState<MenuAction>>()
            .press(&MenuAction::Dismiss);
        app.update();
        app.world_mut()
            .resource_mut::<ActionState<MenuAction>>()
            .release(&MenuAction::Dismiss);
        app.update();
    }

    fn press_key_f(app: &mut App) {
        app.world_mut()
            .resource_mut::<ActionState<MenuAction>>()
            .press(&MenuAction::RowSwap);
        app.update();
        app.world_mut()
            .resource_mut::<ActionState<MenuAction>>()
            .release(&MenuAction::RowSwap);
        app.update();
    }

    fn press_key_s(app: &mut App) {
        app.world_mut()
            .resource_mut::<ActionState<MenuAction>>()
            .press(&MenuAction::SlotSwap);
        app.update();
        app.world_mut()
            .resource_mut::<ActionState<MenuAction>>()
            .release(&MenuAction::SlotSwap);
        app.update();
    }

    // ── Pure helper unit tests ────────────────────────────────────────────────

    #[test]
    fn next_free_slot_picks_lowest_unused() {
        assert_eq!(next_free_slot(&[0, 2], 4), Some(1));
    }

    #[test]
    fn next_free_slot_returns_none_when_full() {
        assert_eq!(next_free_slot(&[0, 1, 2, 3], 4), None);
    }

    #[test]
    fn next_free_slot_handles_empty_party() {
        assert_eq!(next_free_slot(&[], 4), Some(0));
    }

    // ── Recruit integration tests ─────────────────────────────────────────────

    #[test]
    fn recruit_spawns_party_member_with_correct_bundle_fields() {
        let mut app = make_guild_test_app();
        app.world_mut().resource_mut::<GuildState>().mode = GuildMode::Recruit;
        app.world_mut().resource_mut::<GuildState>().cursor = 0;

        press_confirm(&mut app);

        let mut q = app
            .world_mut()
            .query_filtered::<(&CharacterName, &Race, &Class), With<PartyMember>>();
        let members: Vec<_> = q.iter(app.world()).collect();
        assert_eq!(members.len(), 1, "One party member should exist after recruit");
        let (name, race, class) = members[0];
        assert_eq!(name.0, "Ser Edran");
        assert!(matches!(race, Race::Human));
        assert!(matches!(class, Class::Fighter));
    }

    /// Recruiting the same `RecruitedSet` index twice spawns only ONE entity.
    /// `RecruitedSet` tracks taken pool indices to prevent duplicate recruits.
    #[test]
    fn recruit_same_pool_index_twice_only_spawns_once() {
        let mut app = make_guild_test_app();
        app.world_mut().resource_mut::<GuildState>().mode = GuildMode::Recruit;
        app.world_mut().resource_mut::<GuildState>().cursor = 0;

        // First press → recruits pool index 0.
        press_confirm(&mut app);

        // Guild auto-switches to Roster after success — reset back to Recruit
        // to attempt the duplicate.
        app.world_mut().resource_mut::<GuildState>().mode = GuildMode::Recruit;
        app.world_mut().resource_mut::<GuildState>().cursor = 0;
        press_confirm(&mut app);

        let count = app
            .world_mut()
            .query_filtered::<(), With<PartyMember>>()
            .iter(app.world())
            .count();
        assert_eq!(count, 1, "Second recruit on same pool index must be rejected");

        let recruited = app.world().resource::<RecruitedSet>();
        assert_eq!(recruited.indices.len(), 1);
        assert!(recruited.indices.contains(&0));
    }

    #[test]
    fn recruit_picks_lowest_free_slot_after_dismissal() {
        let mut app = make_guild_test_app();

        // Spawn 4 members at slots 0..=3.
        for i in 0..4 {
            spawn_active_member(&mut app, i, PartyRow::Front);
        }

        // Dismiss slot 1 (cursor=1, sorted by slot).
        app.world_mut().resource_mut::<GuildState>().cursor = 1;
        press_key_d(&mut app);
        // apply_deferred — wait one more update.
        app.update();

        // Now 3 active members: slots 0, 2, 3. Lowest free = 1.
        // Switch to Recruit mode and recruit.
        app.world_mut().resource_mut::<GuildState>().mode = GuildMode::Recruit;
        app.world_mut().resource_mut::<GuildState>().cursor = 0;
        press_confirm(&mut app);

        // Find the new recruit.
        let mut q = app
            .world_mut()
            .query_filtered::<(&CharacterName, &PartySlot), With<PartyMember>>();
        let members: Vec<_> = q.iter(app.world()).collect();
        let new_recruit = members
            .iter()
            .find(|(name, _)| name.0 == "Ser Edran")
            .expect("Ser Edran should have been recruited");
        assert_eq!(new_recruit.1.0, 1, "New recruit should be in slot 1 (lowest free)");
    }

    #[test]
    fn recruit_rejects_when_party_full() {
        let mut app = make_guild_test_app();
        // PartySize::default() == 4 — spawn 4 members.
        for i in 0..4 {
            spawn_active_member(&mut app, i, PartyRow::Front);
        }

        app.world_mut().resource_mut::<GuildState>().mode = GuildMode::Recruit;
        app.world_mut().resource_mut::<GuildState>().cursor = 0;
        press_confirm(&mut app);

        let count = app
            .world_mut()
            .query_filtered::<(), With<PartyMember>>()
            .iter(app.world())
            .count();
        assert_eq!(count, 4, "Party should remain at 4 when full");
    }

    #[test]
    fn recruit_allows_empty_party() {
        let mut app = make_guild_test_app();
        // No active members — recruit should succeed (no min-1-active guard on Recruit).
        app.world_mut().resource_mut::<GuildState>().mode = GuildMode::Recruit;
        app.world_mut().resource_mut::<GuildState>().cursor = 0;
        press_confirm(&mut app);

        let count = app
            .world_mut()
            .query_filtered::<(), With<PartyMember>>()
            .iter(app.world())
            .count();
        assert_eq!(count, 1, "Should have spawned 1 member into an empty party");
    }

    #[test]
    fn recruit_attaches_empty_inventory_component() {
        let mut app = make_guild_test_app();
        app.world_mut().resource_mut::<GuildState>().mode = GuildMode::Recruit;
        app.world_mut().resource_mut::<GuildState>().cursor = 0;
        press_confirm(&mut app);

        let mut q = app
            .world_mut()
            .query_filtered::<&Inventory, With<PartyMember>>();
        let inventories: Vec<_> = q.iter(app.world()).collect();
        assert_eq!(inventories.len(), 1);
        assert!(inventories[0].0.is_empty(), "Recruit should start with an empty inventory");
    }

    // ── Dismiss integration tests ─────────────────────────────────────────────

    #[test]
    fn dismiss_removes_partymember_marker() {
        let mut app = make_guild_test_app();
        spawn_active_member(&mut app, 0, PartyRow::Front);
        spawn_active_member(&mut app, 1, PartyRow::Back);

        app.world_mut().resource_mut::<GuildState>().cursor = 0;
        press_key_d(&mut app);
        app.update(); // extra settle for deferred commands

        let count = app
            .world_mut()
            .query_filtered::<(), With<PartyMember>>()
            .iter(app.world())
            .count();
        assert_eq!(count, 1, "Exactly 1 PartyMember should remain after dismiss");
    }

    #[test]
    fn dismiss_adds_entity_to_pool() {
        let mut app = make_guild_test_app();
        spawn_active_member(&mut app, 0, PartyRow::Front);
        spawn_active_member(&mut app, 1, PartyRow::Back);

        app.world_mut().resource_mut::<GuildState>().cursor = 0;
        press_key_d(&mut app);
        app.update();

        let pool_len = app.world().resource::<DismissedPool>().entities.len();
        assert_eq!(pool_len, 1, "Dismissed entity should be in DismissedPool");
    }

    #[test]
    fn dismiss_preserves_inventory_entities() {
        let mut app = make_guild_test_app();

        // Spawn two active members.
        let m0 = spawn_active_member(&mut app, 0, PartyRow::Front);
        spawn_active_member(&mut app, 1, PartyRow::Back);

        // Give member 0 a non-empty inventory (two fake item entities).
        let item_a = app.world_mut().spawn_empty().id();
        let item_b = app.world_mut().spawn_empty().id();
        app.world_mut()
            .get_mut::<Inventory>(m0)
            .unwrap()
            .0
            .extend([item_a, item_b]);

        // Dismiss cursor 0 (m0).
        app.world_mut().resource_mut::<GuildState>().cursor = 0;
        press_key_d(&mut app);
        app.update();

        // m0 entity still exists (not despawned); its Inventory is intact.
        let inventory = app.world().get::<Inventory>(m0).expect("m0 should still exist");
        assert_eq!(inventory.0.len(), 2, "Inventory entities must be preserved on dismiss");
        assert!(inventory.0.contains(&item_a));
        assert!(inventory.0.contains(&item_b));
    }

    #[test]
    fn dismiss_rejects_last_active_member() {
        let mut app = make_guild_test_app();
        spawn_active_member(&mut app, 0, PartyRow::Front);

        app.world_mut().resource_mut::<GuildState>().cursor = 0;
        press_key_d(&mut app);
        app.update();

        let count = app
            .world_mut()
            .query_filtered::<(), With<PartyMember>>()
            .iter(app.world())
            .count();
        assert_eq!(count, 1, "Last active member should NOT be dismissed");

        let pool_len = app.world().resource::<DismissedPool>().entities.len();
        assert_eq!(pool_len, 0, "Pool should be empty when dismiss was rejected");
    }

    #[test]
    fn dismiss_then_recruit_in_one_frame_restores_count() {
        let mut app = make_guild_test_app();
        for i in 0..4 {
            spawn_active_member(&mut app, i, PartyRow::Front);
        }

        // Dismiss cursor=0 in Roster mode.
        app.world_mut().resource_mut::<GuildState>().cursor = 0;
        press_key_d(&mut app);

        // Switch to Recruit and recruit in the same "session" (before dismiss settles).
        app.world_mut().resource_mut::<GuildState>().mode = GuildMode::Recruit;
        app.world_mut().resource_mut::<GuildState>().cursor = 0;
        press_confirm(&mut app);

        // After settling:
        app.update();
        app.update();

        let count = app
            .world_mut()
            .query_filtered::<(), With<PartyMember>>()
            .iter(app.world())
            .count();
        assert_eq!(count, 4, "Count should restore to 4 after dismiss+recruit");

        let pool_len = app.world().resource::<DismissedPool>().entities.len();
        assert_eq!(pool_len, 1, "One dismissed entity should be in the pool");
    }

    // ── Row swap integration tests ────────────────────────────────────────────

    #[test]
    fn row_swap_toggles_front_to_back_and_back_to_front() {
        let mut app = make_guild_test_app();
        let member = spawn_active_member(&mut app, 0, PartyRow::Front);

        // First F: Front → Back.
        press_key_f(&mut app);
        let row = *app.world().get::<PartyRow>(member).unwrap();
        assert!(matches!(row, PartyRow::Back), "First F should toggle Front → Back");

        // Second F: Back → Front.
        press_key_f(&mut app);
        let row = *app.world().get::<PartyRow>(member).unwrap();
        assert!(matches!(row, PartyRow::Front), "Second F should toggle Back → Front");
    }

    // ── Slot swap integration tests ───────────────────────────────────────────

    #[test]
    fn slot_swap_exchanges_two_members_slots() {
        let mut app = make_guild_test_app();
        let m0 = spawn_active_member(&mut app, 0, PartyRow::Front);
        let m2 = spawn_active_member(&mut app, 2, PartyRow::Front);

        // Sort order: cursor 0 → m0 (slot 0), cursor 1 → m2 (slot 2).
        // First S: pin cursor 0 (m0).
        app.world_mut().resource_mut::<GuildState>().cursor = 0;
        press_key_s(&mut app);

        // Second S: target cursor 1 (m2).
        app.world_mut().resource_mut::<GuildState>().cursor = 1;
        press_key_s(&mut app);

        let slot_m0 = app.world().get::<PartySlot>(m0).unwrap().0;
        let slot_m2 = app.world().get::<PartySlot>(m2).unwrap().0;
        assert_eq!(slot_m0, 2, "m0 should now have slot 2");
        assert_eq!(slot_m2, 0, "m2 should now have slot 0");

        // Total count unchanged.
        let count = app
            .world_mut()
            .query_filtered::<(), With<PartyMember>>()
            .iter(app.world())
            .count();
        assert_eq!(count, 2, "Party count should be unchanged after slot swap");
    }

    // ── Navigation test ───────────────────────────────────────────────────────

    #[test]
    fn cancel_returns_to_square() {
        let mut app = make_guild_test_app();
        press_cancel(&mut app);
        assert_eq!(
            *app.world().resource::<State<TownLocation>>().get(),
            TownLocation::Square
        );
    }
}
