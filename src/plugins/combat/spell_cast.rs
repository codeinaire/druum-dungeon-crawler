//! Spell-damage computation — Feature #20 Phase 1.
//!
//! Pure-function module mirroring `damage.rs`. Provides:
//! - `SpellCombatant` — caller-flattened input (spells ignore row, per Q5).
//! - `SpellDamageResult` — outcome of `spell_damage_calc`.
//! - `spell_damage_calc` — pure, seedable, no entity lookups.
//! - `check_mp` / `deduct_mp` — MP gate helpers consumed by the resolver.
//!
//! ## Pure-function discipline (research Pattern 5)
//!
//! No `Mut<T>`, no `Query`, no `Res`, no `Time`, no `Commands`.
//! Variance + crit math mirrors `damage_calc` at `damage.rs:120-131`.
//!
//! ## Spell damage formula (Wizardry-style magic)
//!
//! `raw = (magic_attack + power - magic_defense / 2).max(1)`
//! Variance: 0.7..=1.0 (same as physical).
//! Crit: 1.5x at `accuracy / 5`% (proxy for luck — see Implementation Discoveries
//! in plan §Phase 1: `DerivedStats` does not expose `luck` directly; `accuracy`
//! is the closest derived field that incorporates luck from `BaseStats`).
//!
//! See `project/plans/20260514-120000-feature-20-spells-skill-tree.md`.

use rand::Rng;

use crate::data::spells::{SpellAsset, SpellEffect, MAX_SPELL_DAMAGE, MAX_SPELL_MP_COST};
use crate::plugins::party::character::{DerivedStats, StatusEffects};

// ─────────────────────────────────────────────────────────────────────────────
// Input / output types
// ─────────────────────────────────────────────────────────────────────────────

/// Caller-flattened combatant data for spell resolution.
///
/// Spells ignore rows per planner-resolved Q5 — no `row` field needed.
/// The caller is responsible for building this from the entity's components
/// before calling `spell_damage_calc`.
#[derive(Debug, Clone)]
pub struct SpellCombatant {
    pub name: String,
    pub stats: DerivedStats,
    pub status: StatusEffects,
}

/// Result of a single spell-damage calculation.
///
/// `hit` is deliberately absent — spells never miss in v1 (future polish).
#[derive(Debug, Clone, PartialEq)]
pub struct SpellDamageResult {
    pub damage: u32,
    pub critical: bool,
    pub message: String,
}

// ─────────────────────────────────────────────────────────────────────────────
// Pure functions
// ─────────────────────────────────────────────────────────────────────────────

/// Compute magical damage for one `Damage`-typed spell.
///
/// **Pure** — no `Mut`, no entity lookups, no resource reads.
///
/// Returns a zero-damage result for any non-Damage spell variant (the caller
/// should dispatch on effect type before calling).
///
/// # Formula
///
/// ```text
/// base_power = spell.effect.Damage.power.min(MAX_SPELL_DAMAGE)
/// raw = (magic_attack + base_power - magic_defense.min(180) / 2).max(1)
/// variance: 0.7..=1.0
/// crit 1.5x at accuracy / 5 %
/// ```
pub fn spell_damage_calc(
    caster: &SpellCombatant,
    target: &SpellCombatant,
    spell: &SpellAsset,
    rng: &mut (impl Rng + ?Sized),
) -> SpellDamageResult {
    // Only Damage variants produce damage; guard early for all others.
    let power = match &spell.effect {
        SpellEffect::Damage { power } => *power,
        _ => {
            return SpellDamageResult {
                damage: 0,
                critical: false,
                message: format!(
                    "{} uses {} (non-damage effect).",
                    caster.name, spell.display_name
                ),
            };
        }
    };

    // Consumer-side clamp (belt-and-suspenders: asset is also clamped at load).
    let base_power = power.min(MAX_SPELL_DAMAGE);

    // Wizardry magic formula: magic_attack + spell_power – magic_defense/2, floor 1.
    let raw = (caster.stats.magic_attack as i64
        + base_power as i64
        - (target.stats.magic_defense.min(180) as i64 / 2))
        .max(1) as u32;

    // Variance 0.7..=1.0 (mirrors damage.rs:121).
    let variance = rng.random_range(70..=100u32) as f32 / 100.0;
    let damage = (raw as f32 * variance) as u32;

    // Crit: 1.5x at `accuracy / 5`% chance.
    // Note: plan specifies `luck / 5` but DerivedStats does not expose `luck`
    // directly (it is folded into `accuracy` during derive_stats). Using
    // `accuracy / 5` is the closest available proxy (see plan §Implementation
    // Discoveries). Matches the `damage_calc` shape exactly.
    let crit_chance = (caster.stats.accuracy / 5).min(100);
    let critical = rng.random_range(0..100u32) < crit_chance;
    let damage = if critical {
        (damage as f32 * 1.5) as u32
    } else {
        damage
    };

    let damage = damage.max(1); // floor of 1 for a landing spell hit.

    SpellDamageResult {
        damage,
        critical,
        message: format!(
            "{} casts {} on {} for {} damage{}.",
            caster.name,
            spell.display_name,
            target.name,
            damage,
            if critical { " (CRITICAL)" } else { "" },
        ),
    }
}

/// Returns `true` if `derived` has enough MP to cast `spell`.
///
/// Clamps `spell.mp_cost` to `MAX_SPELL_MP_COST` at the check site
/// (belt-and-suspenders — the RON is already clamped at load).
pub fn check_mp(derived: &DerivedStats, spell: &SpellAsset) -> bool {
    derived.current_mp >= spell.mp_cost.min(MAX_SPELL_MP_COST)
}

/// Deducts `spell.mp_cost` from `derived.current_mp` using saturating subtraction.
///
/// Clamps `spell.mp_cost` to `MAX_SPELL_MP_COST` at the deduction site.
pub fn deduct_mp(derived: &mut DerivedStats, spell: &SpellAsset) {
    derived.current_mp = derived
        .current_mp
        .saturating_sub(spell.mp_cost.min(MAX_SPELL_MP_COST));
}

// ─────────────────────────────────────────────────────────────────────────────
// Unit tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::spells::{SpellSchool, SpellTarget};
    use rand::SeedableRng;

    fn mk_caster(magic_attack: u32, accuracy: u32) -> SpellCombatant {
        SpellCombatant {
            name: "Caster".into(),
            stats: DerivedStats {
                magic_attack,
                accuracy,
                current_mp: 100,
                max_mp: 100,
                current_hp: 100,
                max_hp: 100,
                ..Default::default()
            },
            status: Default::default(),
        }
    }

    fn mk_target(magic_defense: u32) -> SpellCombatant {
        SpellCombatant {
            name: "Target".into(),
            stats: DerivedStats {
                magic_defense,
                current_hp: 100,
                max_hp: 100,
                ..Default::default()
            },
            status: Default::default(),
        }
    }

    fn mk_damage_spell(power: u32) -> SpellAsset {
        SpellAsset {
            id: "test_spell".into(),
            display_name: "Test Spell".into(),
            mp_cost: 5,
            level: 1,
            school: SpellSchool::Mage,
            target: SpellTarget::SingleEnemy,
            effect: SpellEffect::Damage { power },
            description: String::new(),
            icon_path: String::new(),
        }
    }

    fn mk_heal_spell() -> SpellAsset {
        SpellAsset {
            id: "test_heal".into(),
            display_name: "Test Heal".into(),
            mp_cost: 4,
            level: 1,
            school: SpellSchool::Priest,
            target: SpellTarget::SingleAlly,
            effect: SpellEffect::Heal { amount: 20 },
            description: String::new(),
            icon_path: String::new(),
        }
    }

    /// `spell_damage_calc` returns zero damage for a non-Damage spell variant.
    #[test]
    fn spell_damage_zero_for_non_damage_variant() {
        let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(42);
        let caster = mk_caster(10, 80);
        let target = mk_target(5);
        let spell = mk_heal_spell();
        let result = spell_damage_calc(&caster, &target, &spell, &mut rng);
        assert_eq!(result.damage, 0);
        assert!(!result.critical);
    }

    /// Two calls with the same seed produce identical results.
    #[test]
    fn spell_damage_seeded_deterministic() {
        let caster = mk_caster(20, 80);
        let target = mk_target(10);
        let spell = mk_damage_spell(15);
        let mut rng_a = rand_chacha::ChaCha8Rng::seed_from_u64(99);
        let mut rng_b = rand_chacha::ChaCha8Rng::seed_from_u64(99);
        let r1 = spell_damage_calc(&caster, &target, &spell, &mut rng_a);
        let r2 = spell_damage_calc(&caster, &target, &spell, &mut rng_b);
        assert_eq!(r1, r2);
    }

    /// Damage output is capped by `MAX_SPELL_DAMAGE` consumer-side clamp.
    #[test]
    fn spell_damage_caps_at_max_spell_damage() {
        // Use accuracy=0 so no crits, variance=fixed via seed.
        let caster = mk_caster(0, 0);
        let target = mk_target(0);
        // Power way over the cap.
        let spell = mk_damage_spell(MAX_SPELL_DAMAGE * 2);
        // Even with a power far above MAX_SPELL_DAMAGE the clamped power
        // is MAX_SPELL_DAMAGE=999. raw = (0 + 999 - 0/2).max(1) = 999.
        // With variance 0.7..=1.0, damage ∈ [699, 999]. Floor 1 applies.
        let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(0);
        let result = spell_damage_calc(&caster, &target, &spell, &mut rng);
        // Clamp is working if damage <= 999 * 1.5 (possible crit ceiling).
        assert!(
            result.damage <= (MAX_SPELL_DAMAGE as f32 * 1.5) as u32 + 1,
            "damage {} should not exceed crit ceiling ~1499",
            result.damage
        );
    }

    /// `check_mp` returns false when current_mp is below the spell's cost.
    #[test]
    fn check_mp_returns_false_when_insufficient() {
        let mut derived = DerivedStats {
            current_mp: 1,
            max_mp: 100,
            ..Default::default()
        };
        let spell = SpellAsset {
            mp_cost: 5,
            ..mk_damage_spell(10)
        };
        assert!(!check_mp(&derived, &spell));
        derived.current_mp = 5;
        assert!(check_mp(&derived, &spell));
    }

    /// `deduct_mp` saturates at zero rather than underflowing.
    #[test]
    fn deduct_mp_saturates_at_zero() {
        let mut derived = DerivedStats {
            current_mp: 2,
            max_mp: 100,
            ..Default::default()
        };
        let spell = SpellAsset {
            mp_cost: 10,
            ..mk_damage_spell(5)
        };
        deduct_mp(&mut derived, &spell);
        assert_eq!(derived.current_mp, 0, "saturating_sub should stop at zero");
    }
}
