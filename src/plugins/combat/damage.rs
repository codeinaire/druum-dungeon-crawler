//! Damage computation — Feature #15 Phase 15B.
//!
//! D-A3=A: Wizardry-style multiplicative formula `(A * (100 - D / 2)) / 100`.
//! Variance multiplier 0.7..=1.0; crit 1.5x at `accuracy / 5`% chance.
//!
//! ## Pure function discipline (research Pattern 2, roadmap line 858)
//!
//! `damage_calc` is the SINGLE OWNER of the damage formula and row rules.
//! No entity lookups, no resource reads. The caller flattens
//! `(actor_entity, &Query<...>)` into the `Combatant` struct.
//!
//! ## Row rules (Decision 28, simplified for v1)
//!
//! Front-row attacker with melee weapon vs. back-row defender → damage = 0
//! ("can't reach"). All other combinations → full damage. Real weapon-kind
//! classification is #17 polish.
//!
//! ## Saturating arithmetic
//!
//! All addition uses `saturating_*`. Defends against `u32::MAX` from
//! malicious save data (research §Security trust boundary).

use rand::Rng;

use crate::data::items::ItemAsset;
use crate::plugins::combat::actions::CombatActionKind;
use crate::plugins::party::character::{DerivedStats, PartyRow, StatusEffects};

/// Caller-flattened combatant data. Mirrors the `derive_stats` caller-flatten
/// pattern.
#[derive(Debug, Clone)]
pub struct Combatant {
    pub name: String,
    pub stats: DerivedStats,
    pub row: PartyRow,
    pub status: StatusEffects,
}

/// Result of a single damage calculation.
#[derive(Debug, Clone, PartialEq)]
pub struct DamageResult {
    pub damage: u32,
    pub hit: bool,
    pub critical: bool,
    pub message: String,
}

/// Compute damage for one `Attack` action.
///
/// **Pure** — no `Mut`, no entity lookups, no resource reads. All inputs are
/// provided by the caller.
///
/// # Formula (D-A3=A)
///
/// ```text
/// hit_chance = (accuracy - evasion).max(0).min(100)
/// raw_damage = (attack * (100 - defense.min(180) / 2)) / 100
/// damage = (raw_damage * variance_0.7..1.0).max(1)
/// crit_chance = (accuracy / 5).min(100)
/// if crit: damage = (damage * 1.5).floor
/// ```
pub fn damage_calc(
    attacker: &Combatant,
    defender: &Combatant,
    weapon: Option<&ItemAsset>,
    action: &CombatActionKind,
    rng: &mut (impl Rng + ?Sized),
) -> DamageResult {
    // Only `Attack` computes damage; other actions are resolver-side effects.
    if !matches!(action, CombatActionKind::Attack) {
        return DamageResult {
            damage: 0,
            hit: false,
            critical: false,
            message: format!("{} performs a non-damaging action.", attacker.name),
        };
    }

    // 1. Hit roll (Decision 29).
    let hit_chance = attacker
        .stats
        .accuracy
        .saturating_sub(defender.stats.evasion)
        .min(100);
    let hit = rng.random_range(0..100u32) < hit_chance;
    if !hit {
        return DamageResult {
            damage: 0,
            hit: false,
            critical: false,
            message: format!("{} misses {}.", attacker.name, defender.name),
        };
    }

    // 2. Row check (Decision 28). Simplified: all weapons are melee in v1.
    // Front-row attacker with a weapon vs. back-row defender → can't reach.
    if matches!(attacker.row, PartyRow::Front)
        && matches!(defender.row, PartyRow::Back)
        && weapon.is_some()
    {
        // Future: weapon-kind classification (Bow/Spear) bypasses this rule.
        // v1: any weapon-equipped front-row attacker fails to reach back-row.
        return DamageResult {
            damage: 0,
            hit: true,
            critical: false,
            message: format!(
                "{}'s attack can't reach {} in the back row.",
                attacker.name, defender.name
            ),
        };
    }

    // 3. D-A3=A: Wizardry-style multiplicative damage.
    // Cap defense at 180 to keep `(100 - D/2)` non-negative.
    let raw =
        (attacker.stats.attack as i64 * (100 - defender.stats.defense.min(180) as i64 / 2)) / 100;
    let raw = raw.max(1) as u32;

    // 4. Variance multiplier 0.7..=1.0 (D-A3=A).
    let variance = rng.random_range(70..=100u32) as f32 / 100.0;
    let damage = (raw as f32 * variance) as u32;

    // 5. Crit roll (Decision 29: chance = accuracy / 5 capped at 100).
    let crit_chance = (attacker.stats.accuracy / 5).min(100);
    let critical = rng.random_range(0..100u32) < crit_chance;
    let damage = if critical {
        (damage as f32 * 1.5) as u32
    } else {
        damage
    };

    let damage = damage.max(1); // floor of 1 on positive-attack hits.

    DamageResult {
        damage,
        hit: true,
        critical,
        message: format!(
            "{} {} {} for {} damage{}.",
            attacker.name,
            if critical {
                "critically strikes"
            } else {
                "attacks"
            },
            defender.name,
            damage,
            if critical { " (CRITICAL)" } else { "" },
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;

    fn mk_combatant(
        name: &str,
        attack: u32,
        defense: u32,
        accuracy: u32,
        evasion: u32,
        row: PartyRow,
    ) -> Combatant {
        Combatant {
            name: name.into(),
            stats: DerivedStats {
                attack,
                defense,
                accuracy,
                evasion,
                current_hp: 100,
                max_hp: 100,
                ..Default::default()
            },
            row,
            status: StatusEffects::default(),
        }
    }

    #[test]
    fn damage_calc_defense_greater_than_attack_floors_at_one() {
        let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(42);
        let a = mk_combatant("A", 5, 100, 100, 0, PartyRow::Front);
        let d = mk_combatant("D", 5, 100, 0, 0, PartyRow::Front);
        let r = damage_calc(&a, &d, None, &CombatActionKind::Attack, &mut rng);
        assert!(r.hit);
        assert!(r.damage >= 1, "Damage floor should be at least 1");
    }

    #[test]
    fn damage_calc_zero_attack_floors_at_one() {
        let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(42);
        let a = mk_combatant("A", 0, 100, 100, 0, PartyRow::Front);
        let d = mk_combatant("D", 0, 0, 0, 0, PartyRow::Front);
        let r = damage_calc(&a, &d, None, &CombatActionKind::Attack, &mut rng);
        assert!(r.hit);
        assert!(r.damage >= 1);
    }

    #[test]
    fn damage_calc_misses_when_evasion_high() {
        let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(42);
        // accuracy = 0, evasion = 100 → hit_chance = 0 (saturating_sub floors at 0)
        let a = mk_combatant("A", 20, 0, 0, 0, PartyRow::Front);
        let d = mk_combatant("D", 0, 0, 0, 100, PartyRow::Front);
        let r = damage_calc(&a, &d, None, &CombatActionKind::Attack, &mut rng);
        assert!(!r.hit);
        assert_eq!(r.damage, 0);
    }

    #[test]
    fn damage_calc_hits_when_accuracy_high() {
        let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(42);
        let a = mk_combatant("A", 20, 0, 100, 0, PartyRow::Front);
        let d = mk_combatant("D", 0, 0, 0, 0, PartyRow::Front);
        let r = damage_calc(&a, &d, None, &CombatActionKind::Attack, &mut rng);
        assert!(r.hit);
    }

    #[test]
    fn damage_calc_deterministic_with_same_seed() {
        let a = mk_combatant("A", 20, 0, 100, 0, PartyRow::Front);
        let d = mk_combatant("D", 0, 10, 0, 0, PartyRow::Front);
        let mut rng_a = rand_chacha::ChaCha8Rng::seed_from_u64(42);
        let mut rng_b = rand_chacha::ChaCha8Rng::seed_from_u64(42);
        let r1 = damage_calc(&a, &d, None, &CombatActionKind::Attack, &mut rng_a);
        let r2 = damage_calc(&a, &d, None, &CombatActionKind::Attack, &mut rng_b);
        assert_eq!(
            r1, r2,
            "damage_calc must be deterministic with identical RNG seed"
        );
    }

    #[test]
    fn front_attack_back_with_melee_blocks() {
        let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(42);
        let weapon = crate::data::items::ItemAsset {
            id: "test_sword".into(),
            display_name: "Test Sword".into(),
            ..Default::default()
        };
        let a = mk_combatant("A", 20, 0, 100, 0, PartyRow::Front);
        let d = mk_combatant("D", 0, 0, 0, 0, PartyRow::Back);
        let r = damage_calc(&a, &d, Some(&weapon), &CombatActionKind::Attack, &mut rng);
        assert_eq!(r.damage, 0);
        assert!(r.message.contains("can't reach"));
    }

    #[test]
    fn damage_calc_variance_bounded() {
        // Run 100 trials; assert all non-critical damage values are within
        // the expected variance range.
        let a = mk_combatant("A", 20, 0, 100, 0, PartyRow::Front);
        let d = mk_combatant("D", 0, 10, 0, 0, PartyRow::Front);
        let mut min_dmg = u32::MAX;
        let mut max_dmg = 0u32;
        for seed in 0..100u64 {
            let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(seed);
            let r = damage_calc(&a, &d, None, &CombatActionKind::Attack, &mut rng);
            if r.hit && !r.critical {
                min_dmg = min_dmg.min(r.damage);
                max_dmg = max_dmg.max(r.damage);
            }
        }
        // Expected raw: (20 * (100 - 5)) / 100 = 19. Variance 0.7..1.0 → 13..19.
        // Floor is 1, so min could be 1 if raw were 0, but attack=20 > 0 so raw >= 1.
        if min_dmg != u32::MAX {
            // Only assert if we had at least one non-critical hit in the trials.
            assert!(min_dmg >= 13, "min damage {} below variance floor", min_dmg);
            assert!(
                max_dmg <= 19,
                "max damage {} above variance ceiling",
                max_dmg
            );
        }
    }

    #[test]
    fn damage_calc_non_attack_returns_no_damage() {
        let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(0);
        let a = mk_combatant("A", 20, 0, 100, 0, PartyRow::Front);
        let d = mk_combatant("D", 0, 0, 0, 0, PartyRow::Front);
        let r = damage_calc(&a, &d, None, &CombatActionKind::Defend, &mut rng);
        assert_eq!(r.damage, 0);
        assert!(!r.hit);
    }
}
