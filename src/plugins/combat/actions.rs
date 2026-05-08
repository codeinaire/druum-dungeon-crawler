//! Action-queue payload types — Feature #15.
//!
//! `CombatActionKind` is RENAMED from "CombatAction" to avoid collision with
//! `crate::plugins::input::CombatAction` — the leafwing menu-navigation enum
//! (`input/mod.rs:90-98`). They are different concepts: leafwing's enum is
//! keyboard-direction (Up/Down/Left/Right/Confirm/Cancel); this enum is the
//! data payload for queued combat actions (Attack/Defend/CastSpell/UseItem/Flee).
//!
//! See `project/research/20260508-093000-feature-15-turn-based-combat-core.md`
//! Pitfall 8.

use bevy::prelude::*;

use crate::data::items::ItemAsset;
use crate::plugins::combat::targeting::TargetSelection;

/// The action a combatant has queued for this round.
///
/// Renamed from "CombatAction" to avoid collision with the leafwing
/// `CombatAction` enum (Pitfall 8 of Feature #15 research).
#[derive(Debug, Clone, Reflect)]
pub enum CombatActionKind {
    /// Physical attack with the actor's currently-equipped weapon.
    Attack,
    /// Sets a 1-turn DefenseUp via ApplyStatusEvent (D-Q4=A: take-higher).
    Defend,
    /// Stub — emits "Spell stub" combat-log entry. Full implementation #20.
    CastSpell { spell_id: String },
    /// Consume an item from the actor's `Inventory`.
    UseItem { item: Handle<ItemAsset> },
    /// Try to escape combat. RNG-gated 50% success in v1.
    Flee,
}

/// Which side of the battle a combatant belongs to.
///
/// `PartialOrd`/`Ord` are derived so that `Party < Enemy` — used in the speed
/// tie-break comparator (Decision 14 of Feature #15).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Side {
    Party,
    Enemy,
}

/// One queued action. Sortable by `(speed_at_queue_time DESC, actor_side ASC, slot_index ASC)`.
///
/// `speed_at_queue_time` is captured at queue-time, not resolve-time —
/// mid-round speed buffs do NOT reorder this round (Pitfall 9 of Feature #15).
#[derive(Debug, Clone)]
pub struct QueuedAction {
    pub actor: Entity,
    pub kind: CombatActionKind,
    pub target: TargetSelection,
    pub speed_at_queue_time: u32,
    pub actor_side: Side,
    pub slot_index: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn side_orders_party_before_enemy() {
        assert!(Side::Party < Side::Enemy);
    }
}
