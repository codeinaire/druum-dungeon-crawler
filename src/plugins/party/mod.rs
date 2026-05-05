//! Party management plugin — character data, bundle, party-size resource,
//! and dev-only debug-party spawn.
//!
//! Inventory and progression systems land in #12 / #14.

use bevy::prelude::*;

pub mod character;

pub use character::{
    ActiveEffect, BaseStats, CharacterName, Class, DerivedStats, Equipment, Experience,
    PartyMember, PartyMemberBundle, PartyRow, PartySize, PartySlot, Race, StatusEffectType,
    StatusEffects, derive_stats,
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

        // Gate: feature = "dev" (NOT cfg(debug_assertions)).
        // Trigger: OnEnter(GameState::Dungeon) (NOT OnEnter(Loading)) — assets
        // are guaranteed loaded by bevy_asset_loader's continue_to_state at
        // Dungeon entry. See #11 plan §Critical Decision 5.
        #[cfg(feature = "dev")]
        {
            use crate::plugins::state::GameState;
            app.add_systems(OnEnter(GameState::Dungeon), spawn_default_debug_party);
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
    let roster = [
        ("Aldric", Class::Fighter, PartyRow::Front),
        ("Mira", Class::Mage, PartyRow::Front),
        ("Father Gren", Class::Priest, PartyRow::Back),
        ("Borin", Class::Fighter, PartyRow::Back),
    ];

    let count = party_size.0.min(4);
    for (i, (name, class, row)) in roster.iter().enumerate().take(count) {
        commands.spawn(PartyMemberBundle {
            name: CharacterName((*name).into()),
            class: *class,
            race: Race::Human,
            party_row: *row,
            party_slot: PartySlot(i),
            ..Default::default()
        });
    }

    info!("Spawned {} debug party members", count);
}
