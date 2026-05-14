//! Spell database schema — Feature #20 (Phase 1).
//!
//! Replaces the 11-line stub from Feature #3. Defines the full `SpellDb`
//! asset schema: `SpellId`, `SpellSchool`, `SpellTarget`, `SpellEffect`,
//! `SpellAsset`, `SpellDb`. Registered via
//! `RonAssetPlugin::<SpellDb>::new(&["spells.ron"])` in `loading/mod.rs`.
//!
//! See `project/plans/20260514-120000-feature-20-spells-skill-tree.md`.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::plugins::party::character::StatusEffectType;

// ─────────────────────────────────────────────────────────────────────────────
// Type alias
// ─────────────────────────────────────────────────────────────────────────────

/// Spell identifier — a plain `String` matching the `id` field in RON assets.
///
/// Matches `ItemAsset.id` / `EnemySpec.id` precedent and the existing
/// `CombatActionKind::CastSpell { spell_id: String }` at `actions.rs:28`.
/// `Handle<T>` is NOT used (Bevy 0.18 does not serialize `Handle<T>`).
pub type SpellId = String;

// ─────────────────────────────────────────────────────────────────────────────
// MAX_* constants (trust boundary — see plan §MAX_* block)
// ─────────────────────────────────────────────────────────────────────────────

/// Maximum allowable MP cost on a `SpellAsset.mp_cost`. Crafted RON values
/// above this clamp at consumer side. Defends against a malicious save
/// setting `mp_cost: u32::MAX` (caster could never cast).
pub const MAX_SPELL_MP_COST: u32 = 999;

/// Maximum `SpellEffect::Damage.power` and `Special` damage proxies.
/// Caps spell damage on the producer side; `spell_damage_calc` saturates.
pub const MAX_SPELL_DAMAGE: u32 = 999;

/// Maximum `SpellEffect::Heal.amount` and `SpellEffect::Revive.hp`.
pub const MAX_SPELL_HEAL: u32 = 999;

/// Maximum `SpellEffect::ApplyStatus.duration` and `SpellEffect::Buff.duration`.
/// 99 rounds is well beyond any v1 spell duration; matches level cap shape.
pub const MAX_SPELL_DURATION: u32 = 99;

/// Maximum spells per character's `KnownSpells.spells` vector. Caps crafted-save
/// `KnownSpells.spells: Vec<SpellId>` of pathological length. Truncated on
/// deserialize; matches `clamp_recruit_pool` at `data/town.rs:120-127`.
pub const KNOWN_SPELLS_MAX: usize = 64;

// ─────────────────────────────────────────────────────────────────────────────
// Enums
// ─────────────────────────────────────────────────────────────────────────────

/// Which school a spell belongs to.
///
/// APPEND-ONLY — reordering shifts serialized discriminants and breaks
/// save data (mirrors `Class`/`Race`/`StatusEffectType` precedent).
#[derive(Reflect, Serialize, Deserialize, Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SpellSchool {
    #[default]
    Mage,
    Priest,
}

/// Who a spell can target.
///
/// APPEND-ONLY for save-format stability.
#[derive(Reflect, Serialize, Deserialize, Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SpellTarget {
    #[default]
    SingleEnemy,
    AllEnemies,
    SingleAlly,
    AllAllies,
    Self_,
}

/// The mechanical effect a spell produces when it resolves.
///
/// No `Eq` or `Hash` because of `f32` fields (matches `ActiveEffect` precedent
/// at `character.rs:282`).
///
/// APPEND-ONLY — new variants go at the END.
#[derive(Reflect, Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum SpellEffect {
    /// Deals magical damage to each target.
    Damage { power: u32 },
    /// Restores HP to each target.
    Heal { amount: u32 },
    /// Applies a status condition to each target.
    ApplyStatus {
        effect: StatusEffectType,
        potency: f32,
        duration: Option<u32>,
    },
    /// Applies a buff (stat amplifier) to each target.
    Buff {
        effect: StatusEffectType,
        potency: f32,
        duration: u32,
    },
    /// Revives a dead ally with `hp` hit points.
    Revive { hp: u32 },
    /// Escape hatch for day-one unimplemented effects; keyed by string variant.
    Special { variant: String },
}

impl Default for SpellEffect {
    fn default() -> Self {
        Self::Damage { power: 0 }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Asset structs
// ─────────────────────────────────────────────────────────────────────────────

/// A single spell as a Bevy asset — the full v1 schema.
///
/// `SpellId` (a `String`) is used for cross-references rather than
/// `Handle<SpellAsset>` (which Bevy 0.18 cannot serialize).
#[derive(Reflect, Serialize, Deserialize, Default, Debug, Clone, PartialEq)]
pub struct SpellAsset {
    pub id: SpellId,
    pub display_name: String,
    pub mp_cost: u32,
    /// Minimum level required to cast via the skill tree (authoring hint only;
    /// the resolver gates on MP + Silence, not level).
    pub level: u32,
    pub school: SpellSchool,
    pub target: SpellTarget,
    pub effect: SpellEffect,
    #[serde(default)]
    pub description: String,
    /// Empty string day-one per user decision Q10 (deferred to #25 polish).
    #[serde(default)]
    pub icon_path: String,
}

/// Typed RON asset containing all authored spell definitions.
///
/// Loaded from `assets/spells/core.spells.ron` via
/// `RonAssetPlugin::<SpellDb>::new(&["spells.ron"])`.
///
/// `get` uses a linear scan over `Vec<SpellAsset>` (O(n≤20)) — same rationale
/// as `ItemDb::get` and `ClassTable::get`.
#[derive(Asset, Reflect, Serialize, Deserialize, Default, Debug, Clone, PartialEq)]
pub struct SpellDb {
    pub spells: Vec<SpellAsset>,
}

impl SpellDb {
    /// Look up a `SpellAsset` by its `id` field via linear scan.
    ///
    /// Returns `None` when the id has no entry. 15-20 spells day-one makes a
    /// `HashMap` unnecessary (mirror of `ItemDb::get`).
    pub fn get(&self, id: &str) -> Option<&SpellAsset> {
        self.spells.iter().find(|s| s.id == id)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Truncate `spells` to at most `KNOWN_SPELLS_MAX` entries.
///
/// Defense-in-depth against a crafted save with a 1M-entry `KnownSpells`
/// vector that would freeze the spell-menu painter. Mirrors
/// `clamp_recruit_pool` at `data/town.rs:120-127`.
pub fn clamp_known_spells(spells: &mut Vec<SpellId>) {
    spells.truncate(KNOWN_SPELLS_MAX);
}

// ─────────────────────────────────────────────────────────────────────────────
// Unit tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// `SpellAsset` serializes and deserializes through RON 0.11 without loss.
    #[test]
    fn spell_asset_round_trips_through_ron() {
        let spell = SpellAsset {
            id: "halito".into(),
            display_name: "Halito".into(),
            mp_cost: 2,
            level: 1,
            school: SpellSchool::Mage,
            target: SpellTarget::SingleEnemy,
            effect: SpellEffect::Damage { power: 8 },
            description: "A small bolt of fire.".into(),
            icon_path: String::new(),
        };
        let ron_str = ron::to_string(&spell).expect("serialize failed");
        let back: SpellAsset = ron::from_str(&ron_str).expect("deserialize failed");
        assert_eq!(spell, back);
    }

    /// `SpellDb::get` finds an authored spell by id.
    #[test]
    fn spell_db_get_returns_authored_spell() {
        let db = SpellDb {
            spells: vec![SpellAsset {
                id: "dios".into(),
                display_name: "Dios".into(),
                school: SpellSchool::Priest,
                target: SpellTarget::SingleAlly,
                effect: SpellEffect::Heal { amount: 8 },
                ..Default::default()
            }],
        };
        let found = db.get("dios");
        assert!(found.is_some());
        assert_eq!(found.unwrap().display_name, "Dios");
    }

    /// `SpellDb::get` returns `None` for an unknown id.
    #[test]
    fn spell_db_get_returns_none_for_unknown() {
        let db = SpellDb { spells: vec![] };
        assert!(db.get("nonexistent_spell").is_none());
    }

    /// `SpellEffect::Damage` round-trips through RON.
    #[test]
    fn spell_effect_damage_round_trips() {
        let effect = SpellEffect::Damage { power: 42 };
        let s = ron::to_string(&effect).expect("serialize");
        let back: SpellEffect = ron::from_str(&s).expect("deserialize");
        assert_eq!(effect, back);
    }

    /// `SpellEffect::Revive` round-trips through RON.
    #[test]
    fn spell_effect_revive_round_trips() {
        let effect = SpellEffect::Revive { hp: 1 };
        let s = ron::to_string(&effect).expect("serialize");
        let back: SpellEffect = ron::from_str(&s).expect("deserialize");
        assert_eq!(effect, back);
    }

    /// `clamp_known_spells` truncates a vector longer than `KNOWN_SPELLS_MAX`.
    #[test]
    fn clamp_known_spells_truncates_oversized() {
        let mut spells: Vec<SpellId> = (0..KNOWN_SPELLS_MAX + 10)
            .map(|i| format!("spell_{}", i))
            .collect();
        clamp_known_spells(&mut spells);
        assert_eq!(spells.len(), KNOWN_SPELLS_MAX);
    }
}
