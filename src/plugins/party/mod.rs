//! Party management plugin — character data, bundle, party-size resource,
//! and dev-only debug-party spawn.
//!
//! Inventory and progression systems land in #12 / #14.

use bevy::prelude::*;

use crate::data::ItemAsset;
use crate::plugins::state::GameState;

pub mod character;
pub mod inventory;

pub use character::{
    ActiveEffect, BaseStats, CharacterName, Class, DerivedStats, Equipment, Experience,
    PartyMember, PartyMemberBundle, PartyRow, PartySize, PartySlot, Race, StatusEffectType,
    StatusEffects, derive_stats,
};

pub use inventory::{
    EquipError, EquipResult, EquipSlot, EquipmentChangedEvent, Inventory, ItemHandleRegistry,
    ItemInstance, ItemKind, equip_item, give_item, populate_item_handle_registry,
    recompute_derived_stats_on_equipment_change, unequip_item,
};

pub struct PartyPlugin;

impl Plugin for PartyPlugin {
    fn build(&self, app: &mut App) {
        // PartySize defaults to 4 (hard cap in v1; #19 may reduce for scenarios).
        app.init_resource::<PartySize>();

        // Register all Reflect-deriving party types so they appear in editor /
        // debug tooling (bevy_egui_inspector, #19/#25).
        app.register_type::<CharacterName>()
            .register_type::<Race>()
            .register_type::<Class>()
            .register_type::<BaseStats>()
            .register_type::<DerivedStats>()
            .register_type::<Experience>()
            .register_type::<PartyRow>()
            .register_type::<PartySlot>()
            .register_type::<Equipment>()
            .register_type::<StatusEffects>()
            .register_type::<PartyMember>()
            .register_type::<ActiveEffect>()
            .register_type::<StatusEffectType>()
            .register_type::<PartySize>();

        // Feature #12 — inventory & equipment data layer. UI lives in #25.
        // `init_asset::<ItemAsset>()` creates the `Assets<ItemAsset>` resource
        // the recompute system reads. `ItemAsset` is nested inside `ItemDb`
        // (loaded via `RonAssetPlugin::<ItemDb>` in LoadingPlugin), so it has
        // no `RonAssetPlugin` of its own and must be registered explicitly.
        //
        // `ItemHandleRegistry` + `populate_item_handle_registry` bridge the
        // loaded `ItemDb.items` into `Assets<ItemAsset>` once on
        // `OnExit(GameState::Loading)` so production code can resolve item IDs
        // to working handles (#21 loot, #18 shop, #19 starting gear).
        app.init_asset::<ItemAsset>()
            .init_resource::<ItemHandleRegistry>()
            .add_message::<EquipmentChangedEvent>()
            .register_type::<Inventory>()
            .register_type::<ItemInstance>()
            .register_type::<EquipSlot>()
            .register_type::<ItemKind>()
            .add_systems(Update, recompute_derived_stats_on_equipment_change)
            .add_systems(OnExit(GameState::Loading), populate_item_handle_registry);

        // Gate: feature = "dev" (NOT cfg(debug_assertions)).
        // Trigger: OnEnter(GameState::Dungeon) (NOT OnEnter(Loading)) — assets
        // are guaranteed loaded by bevy_asset_loader's continue_to_state at
        // Dungeon entry. See #11 plan §Critical Decision 5.
        #[cfg(feature = "dev")]
        {
            app.add_systems(OnEnter(GameState::Dungeon), spawn_default_debug_party);
            // Town is reachable directly from TitleScreen (Feature #18a Leave Town
            // path), so the party must also be spawned on Town entry. The function
            // is idempotent — re-entry from Dungeon is a no-op.
            app.add_systems(OnEnter(GameState::Town), spawn_default_debug_party);
        }
    }
}

/// Spawn four hardcoded debug party members when entering the Dungeon state.
///
/// Includes an idempotence guard: if any `PartyMember` entity already exists
/// (e.g., F9 cycler re-enters Dungeon), the system returns early without
/// spawning duplicates.
///
/// Per Decision 5: gated `#[cfg(feature = "dev")]`, triggered
/// `OnEnter(GameState::Dungeon)`. Per Decision 6: capped at
/// `party_size.0.min(4)`.
#[cfg(feature = "dev")]
fn spawn_default_debug_party(
    mut commands: Commands,
    party_size: Res<PartySize>,
    existing: Query<(), With<PartyMember>>,
) {
    if !existing.is_empty() {
        info!(
            "Skipping debug party spawn: {} party members already exist",
            existing.iter().count()
        );
        return;
    }

    // Hardcoded 4-member roster: Fighter / Mage / Priest / Fighter (Human, all).
    // BaseStats values are mid-tier-but-survivable for the dev smoke test —
    // VIT=12 yields 17 max_hp at level 1 (enough to survive 2-3 Goblin hits in
    // `spawn_dev_encounter`). Without these explicit values, `..Default::default()`
    // would give `BaseStats::ZERO` AND `DerivedStats::default()` (all zeros), and
    // combat's `check_victory_defeat_flee` would see `current_hp == 0` for every
    // member and immediately decide defeat.
    let base = BaseStats {
        strength: 10,
        intelligence: 8,
        piety: 8,
        vitality: 12,
        agility: 10,
        luck: 6,
    };

    let roster = [
        ("Aldric", Class::Fighter, PartyRow::Front),
        ("Mira", Class::Mage, PartyRow::Front),
        ("Father Gren", Class::Priest, PartyRow::Back),
        ("Borin", Class::Fighter, PartyRow::Back),
    ];

    let count = debug_party_count(party_size.0);
    for (i, (name, class, row)) in roster.iter().enumerate().take(count) {
        let derived = derive_stats(&base, &[], &StatusEffects::default(), 1);
        commands
            .spawn(PartyMemberBundle {
                name: CharacterName((*name).into()),
                class: *class,
                race: Race::Human,
                party_row: *row,
                party_slot: PartySlot(i),
                base_stats: base,
                derived_stats: derived,
                ..Default::default()
            })
            // Feature #12: each party member carries its own bag (Wizardry-style).
            .insert(Inventory::default());
    }

    info!("Spawned {} debug party members", count);
}

/// Hardcoded length of the debug-party roster — the upper bound on the count
/// returned by [`debug_party_count`]. Bound to the `roster` array in
/// [`spawn_default_debug_party`]; both must change together.
#[cfg(any(test, feature = "dev"))]
const DEBUG_PARTY_ROSTER_SIZE: usize = 4;

/// How many debug-party members to actually spawn given the configured
/// `PartySize`: `min(party_size, ROSTER_SIZE)`. Pure function — exists as
/// a separate symbol so the cap arithmetic is unit-testable without an `App`.
#[cfg(any(test, feature = "dev"))]
fn debug_party_count(party_size: usize) -> usize {
    party_size.min(DEBUG_PARTY_ROSTER_SIZE)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `PartySize` exceeding the roster length is clamped to the roster length.
    #[test]
    fn debug_party_count_clamps_above_roster_size() {
        assert_eq!(debug_party_count(99), DEBUG_PARTY_ROSTER_SIZE);
        assert_eq!(debug_party_count(usize::MAX), DEBUG_PARTY_ROSTER_SIZE);
    }

    /// `PartySize::default()` (4) gets the full roster.
    #[test]
    fn debug_party_count_at_roster_size_returns_full() {
        assert_eq!(
            debug_party_count(DEBUG_PARTY_ROSTER_SIZE),
            DEBUG_PARTY_ROSTER_SIZE
        );
    }

    /// `PartySize` smaller than the roster returns the configured size.
    #[test]
    fn debug_party_count_below_roster_size_returns_party_size() {
        assert_eq!(debug_party_count(2), 2);
        assert_eq!(debug_party_count(1), 1);
    }

    /// Zero-capacity is permitted — system spawns nothing.
    #[test]
    fn debug_party_count_zero_returns_zero() {
        assert_eq!(debug_party_count(0), 0);
    }
}
