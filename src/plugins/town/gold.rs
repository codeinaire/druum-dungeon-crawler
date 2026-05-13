//! Party-wide gold resource and `GameClock` day/turn counter — Feature #18a.
//!
//! ## Gold
//!
//! `Gold(u32)` is a party-wide resource — the whole party shares one purse.
//! `try_spend` returns `Err(SpendError)` if insufficient *before* touching the
//! balance; saturating subtraction is defense-in-depth, not the primary guard.
//!
//! ## GameClock
//!
//! Minimal day + turn counters used by the Inn ("day advances when you rest").
//! Future systems (#23 save/load) MUST clamp these on load to prevent gold/day
//! injection from crafted save files.
//!
//! ## Security note
//!
//! `Gold` derives `Serialize`/`Deserialize` for forward compatibility with
//! Feature #23 (save/load). That feature MUST clamp `gold.0` on load to prevent
//! crafted save files from granting unbounded gold. Documented here so the note
//! travels with the type.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

// ─────────────────────────────────────────────────────────────────────────────
// SpendError
// ─────────────────────────────────────────────────────────────────────────────

/// Returned by [`Gold::try_spend`] when the party cannot afford a purchase.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpendError {
    /// The party does not have enough gold.
    InsufficientGold {
        /// How much the party currently has.
        have: u32,
        /// How much the purchase costs.
        need: u32,
    },
}

// ─────────────────────────────────────────────────────────────────────────────
// Gold
// ─────────────────────────────────────────────────────────────────────────────

/// Party-wide gold purse.
///
/// Use `try_spend` to deduct gold after validating sufficiency. Use `earn`
/// to add gold from selling items or other income.
///
/// ## Security note (Feature #23)
/// Deserializing from a save file MUST clamp `gold.0` to a game-design-approved
/// maximum to prevent crafted save-file gold injection.
#[derive(
    Resource, Reflect, Serialize, Deserialize, Default, Debug, Clone, Copy, PartialEq, Eq, Hash,
)]
pub struct Gold(pub u32);

impl Gold {
    /// Attempt to spend `amount` gold.
    ///
    /// Returns `Err(SpendError::InsufficientGold)` without mutating `self` if
    /// `self.0 < amount`. On success, deducts exactly `amount` (saturating as
    /// defense-in-depth — the pre-check makes underflow impossible in practice).
    pub fn try_spend(&mut self, amount: u32) -> Result<(), SpendError> {
        if self.0 < amount {
            return Err(SpendError::InsufficientGold {
                have: self.0,
                need: amount,
            });
        }
        self.0 = self.0.saturating_sub(amount);
        Ok(())
    }

    /// Add `amount` to the gold balance. Saturates at `u32::MAX` rather than
    /// wrapping (defense against overflow in pathological economy flows).
    pub fn earn(&mut self, amount: u32) {
        self.0 = self.0.saturating_add(amount);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// GameClock
// ─────────────────────────────────────────────────────────────────────────────

/// In-game day + turn counter.
///
/// `day` advances when the party rests at the Inn. `turn` advances each
/// combat turn (or each dungeon step, depending on which future system claims
/// it). Both reset-to-0 on new-game (Feature #23).
///
/// ## Security note (Feature #23)
/// Deserializing from a save file MUST clamp these values to prevent injected
/// unreasonable clock states (e.g., day = u32::MAX causing UI overflow).
#[derive(
    Resource, Reflect, Serialize, Deserialize, Default, Debug, Clone, Copy, PartialEq, Eq, Hash,
)]
pub struct GameClock {
    /// Number of days elapsed since the adventure began.
    pub day: u32,
    /// Turn counter within the current day (resets to 0 after Inn rest).
    pub turn: u32,
}

// ─────────────────────────────────────────────────────────────────────────────
// Dev affordance
// ─────────────────────────────────────────────────────────────────────────────

/// Dev-only: F4 grants the party +500 gold for shop testing. Mirrors the F7
/// pattern at `combat/encounter.rs:441-452` — direct `ButtonInput<KeyCode>`
/// reader, gated `cfg(feature = "dev")`.
#[cfg(feature = "dev")]
pub fn grant_gold_on_f4(
    keys: Res<bevy::input::ButtonInput<bevy::prelude::KeyCode>>,
    mut gold: ResMut<Gold>,
) {
    if keys.just_pressed(bevy::prelude::KeyCode::F4) {
        gold.earn(500);
        info!("DEV: granted +500 gold (now {})", gold.0);
    }
}

/// Dev-only: apply status effects to the first party member for Temple/Inn cure
/// testing.
///
/// - **F1** — `Dead` (test Temple revive)
/// - **F2** — `Poison` (test Inn cure)
/// - **F3** — `Stone` + `Paralysis` + `Sleep` (test Temple multi-status auto-pick
///   priority Stone > Paralysis > Sleep)
///
/// Fires `EquipmentChangedEvent` per affected member so derived stats
/// recompute via the existing pipeline (mirrors the Temple/Inn handler pattern).
#[cfg(feature = "dev")]
pub fn apply_test_status_on_function_keys(
    keys: Res<bevy::input::ButtonInput<bevy::prelude::KeyCode>>,
    mut party: Query<
        (
            bevy::prelude::Entity,
            &crate::plugins::party::character::PartySlot,
            &mut crate::plugins::party::character::StatusEffects,
        ),
        bevy::prelude::With<crate::plugins::party::character::PartyMember>,
    >,
    mut writer: bevy::ecs::message::MessageWriter<
        crate::plugins::party::inventory::EquipmentChangedEvent,
    >,
) {
    use crate::plugins::party::character::{ActiveEffect, StatusEffectType};
    use crate::plugins::party::inventory::{EquipmentChangedEvent, EquipSlot};

    // Target the lowest-PartySlot member (deterministic, matches Temple cursor 0).
    let target_entity = {
        let mut members: Vec<(bevy::prelude::Entity, usize)> =
            party.iter().map(|(e, slot, _)| (e, slot.0)).collect();
        members.sort_by_key(|(_, slot)| *slot);
        match members.first() {
            Some(&(e, _)) => e,
            None => return,
        }
    };

    let Ok((entity, _slot, mut effects)) = party.get_mut(target_entity) else {
        return;
    };

    let mut applied: Vec<StatusEffectType> = Vec::new();
    let mut try_apply = |effects: &mut crate::plugins::party::character::StatusEffects,
                        kind: StatusEffectType| {
        if !effects.has(kind) {
            effects.effects.push(ActiveEffect {
                effect_type: kind,
                remaining_turns: None,
                magnitude: 0.0,
            });
            applied.push(kind);
        }
    };

    if keys.just_pressed(bevy::prelude::KeyCode::F1) {
        try_apply(&mut effects, StatusEffectType::Dead);
    }
    if keys.just_pressed(bevy::prelude::KeyCode::F2) {
        try_apply(&mut effects, StatusEffectType::Poison);
    }
    if keys.just_pressed(bevy::prelude::KeyCode::F3) {
        try_apply(&mut effects, StatusEffectType::Stone);
        try_apply(&mut effects, StatusEffectType::Paralysis);
        try_apply(&mut effects, StatusEffectType::Sleep);
    }

    if !applied.is_empty() {
        writer.write(EquipmentChangedEvent {
            character: entity,
            slot: EquipSlot::None,
        });
        info!("DEV: applied {:?} to {:?}", applied, entity);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// `try_spend` with insufficient gold returns `Err` and leaves gold unchanged.
    #[test]
    fn try_spend_insufficient_returns_err_without_mutation() {
        let mut gold = Gold(10);
        let result = gold.try_spend(50);
        assert!(result.is_err());
        assert_eq!(gold.0, 10, "gold must not change on Err");
        match result {
            Err(SpendError::InsufficientGold { have, need }) => {
                assert_eq!(have, 10);
                assert_eq!(need, 50);
            }
            Ok(()) => panic!("expected Err"),
        }
    }

    /// `try_spend` with exactly the right amount returns `Ok` and zeroes the balance.
    #[test]
    fn try_spend_exact_succeeds() {
        let mut gold = Gold(10);
        let result = gold.try_spend(10);
        assert!(result.is_ok());
        assert_eq!(gold.0, 0);
    }

    /// `try_spend` with more than available returns `Err` (underflow guard).
    #[test]
    fn try_spend_underflow_guard() {
        let mut gold = Gold(0);
        let result = gold.try_spend(1);
        assert!(result.is_err());
        assert_eq!(gold.0, 0, "gold must stay at 0, not wrap");
    }

    /// `earn` saturates at `u32::MAX` rather than wrapping.
    #[test]
    fn earn_saturates_at_u32_max() {
        let mut gold = Gold(u32::MAX);
        gold.earn(1);
        assert_eq!(gold.0, u32::MAX, "earn must saturate, not wrap");
    }

    /// `earn` adds normally when there is no overflow risk.
    #[test]
    fn earn_normal_addition() {
        let mut gold = Gold(100);
        gold.earn(50);
        assert_eq!(gold.0, 150);
    }
}
