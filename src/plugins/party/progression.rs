//! Character progression — XP, level-up, and character creation math.
//!
//! Feature #19 — plan: `project/plans/20260513-120000-feature-19-character-creation.md`.
//!
//! ## Structure
//!
//! - **`ProgressionRng`** — resource wrapping a boxed RNG for production handlers.
//!   Tests pass a seeded `ChaCha8Rng` directly to the pure functions.
//! - **`CombatVictoryEvent`** — `Message` emitted on combat victory; consumed by
//!   `award_combat_xp`.
//! - **Pure functions** — `xp_for_level`, `level_up`, `roll_bonus_pool`,
//!   `allocate_bonus_pool`, `can_create_class`, `recompute_xp_to_next_level`.
//! - **Handler systems** — `award_combat_xp`, `apply_level_up_threshold_system`.
//! - **`PartyProgressionPlugin`** — wires the above into the Bevy app.
//!
//! ## Pure function contract
//!
//! Pure functions take `rng: &mut (impl rand::Rng + ?Sized)`. The `?Sized`
//! bound is required to permit `&mut *boxed_rng.0` from a
//! `Box<dyn RngCore + Send + Sync>`. Matches `damage_calc` and
//! `EncounterTable::pick_group` precedents.

use bevy::prelude::*;
use rand::SeedableRng;
use rand::rngs::SmallRng;

use crate::data::{ClassDef, ClassTable};
use crate::plugins::loading::TownAssets;
use crate::plugins::party::character::{
    BaseStats, Class, DerivedStats, Experience, PartyMember, Race, StatusEffectType, StatusEffects,
    derive_stats,
};

// ─────────────────────────────────────────────────────────────────────────────
// Skill point constant — Feature #20
// ─────────────────────────────────────────────────────────────────────────────

/// Skill points awarded per level-up. Feature #20 (Pattern 6 of research).
/// Mirror-declared (NOT duplicated semantically) in `src/data/skills.rs`
/// via `pub use` to avoid a Phase 2 → Phase 3 forward dep.
pub const SKILL_POINTS_PER_LEVEL: u32 = 1;

// ─────────────────────────────────────────────────────────────────────────────
// ProgressionRng resource
// ─────────────────────────────────────────────────────────────────────────────

/// Resource wrapping a boxed RNG for production progression handlers.
///
/// Tests pass a seeded `ChaCha8Rng` directly to the pure functions and skip
/// this resource — the `?Sized` bound on pure functions permits
/// `&mut *boxed_rng.0`.
#[derive(Resource)]
pub struct ProgressionRng(pub Box<dyn rand::RngCore + Send + Sync>);

impl Default for ProgressionRng {
    fn default() -> Self {
        Self(Box::new(SmallRng::from_os_rng()))
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Messages
// ─────────────────────────────────────────────────────────────────────────────

/// Emitted on combat victory. Consumed by `award_combat_xp`.
///
/// **`Message`, not `Event`** — Bevy 0.18 family rename. Must be registered
/// with `app.add_message::<CombatVictoryEvent>()` in `PartyProgressionPlugin::build`.
///
/// `total_gold: u32` is 0 in v1; combat-gold is deferred to #21+.
#[derive(Message, Debug, Clone, Copy)]
pub struct CombatVictoryEvent {
    pub total_xp: u32,
    pub total_gold: u32,
}

// ─────────────────────────────────────────────────────────────────────────────
// Error types
// ─────────────────────────────────────────────────────────────────────────────

/// Error returned by [`allocate_bonus_pool`] when the allocation sum exceeds
/// the pool.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AllocError {
    /// The sum of allocations exceeds `pool`.
    OverPool { allocated: u32, pool: u32 },
}

/// Error returned by [`can_create_class`] when eligibility fails.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CreateError {
    /// The chosen race is not in `class_def.allowed_races` (and the list is non-empty).
    DisallowedRace { race: Race },
    /// A stat is below the class's required minimum.
    BelowMinStat {
        /// Human-readable stat name (e.g. `"strength"`).
        stat: &'static str,
        required: u16,
        actual: u16,
    },
}

// ─────────────────────────────────────────────────────────────────────────────
// StatGains — returned by level_up
// ─────────────────────────────────────────────────────────────────────────────

/// Per-field stat gains from a single level-up.
///
/// At level cap (99), all deltas are zero and `new_xp_to_next_level == u64::MAX`
/// so the threshold system never triggers another level-up.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct StatGains {
    pub strength: u16,
    pub intelligence: u16,
    pub piety: u16,
    pub vitality: u16,
    pub agility: u16,
    pub luck: u16,
    pub hp: u32,
    pub mp: u32,
    pub new_xp_to_next_level: u64,
}

// ─────────────────────────────────────────────────────────────────────────────
// Pure functions
// ─────────────────────────────────────────────────────────────────────────────

/// Level cap — single source of truth.
pub fn level_cap() -> u32 {
    99
}

/// Compute the XP required to reach `target_level` from level 1.
///
/// Formula: `xp_to_level_2 * curve_factor ^ (target_level - 2)`.
/// At `target_level == 2`, the exponent is 0 → result is `xp_to_level_2`
/// (which the field name asserts).
///
/// **Trust boundary clamps (from `ClassDef` RON):**
/// - `xp_to_level_2` clamped to `[1, 1_000_000_000]`.
/// - `xp_curve_factor` clamped to `[1.0, 10.0]`; non-finite values are
///   treated as `1.0` (safe default that avoids NaN/Infinity propagation).
///
/// Returns `0` for `target_level <= 1`, and saturates to `u64::MAX` on overflow.
pub fn xp_for_level(target_level: u32, class_def: &ClassDef) -> u64 {
    if target_level <= 1 {
        return 0;
    }

    let base = class_def.xp_to_level_2.clamp(1, 1_000_000_000);

    let raw_factor = class_def.xp_curve_factor;
    let factor = if raw_factor.is_finite() {
        raw_factor.clamp(1.0, 10.0)
    } else {
        1.0
    };

    let exponent = (target_level - 2) as f64;
    let result_f64 = (base as f64) * (factor as f64).powf(exponent);

    if result_f64.is_finite() && result_f64 < u64::MAX as f64 {
        result_f64 as u64
    } else {
        u64::MAX
    }
}

/// Compute `xp_to_next_level` for a character at `level` (i.e., XP to reach
/// `level + 1`). Thin wrapper around `xp_for_level(level + 1, class_def)`.
pub fn xp_to_next_level_for(class_def: &ClassDef, level: u32) -> u64 {
    xp_for_level(level.saturating_add(1), class_def)
}

/// Invalidate the cached `xp_to_next_level` on an `Experience` component.
///
/// **MUST be called** whenever `Experience.level` or the character's `Class`
/// changes. Without this, the threshold system reads a stale value and either
/// levels too slowly or never stops leveling.
///
/// **No production call sites in #19.** This is exported for the
/// class-change system landing in #21+. At spawn time, callers initialize
/// `Experience` explicitly via `xp_to_next_level_for(def, 1)` rather than
/// going through this fn.
pub fn recompute_xp_to_next_level(
    experience: &mut Experience,
    class: Class,
    table: &ClassTable,
) {
    if experience.level >= level_cap() {
        experience.xp_to_next_level = u64::MAX;
        return;
    }
    if let Some(class_def) = table.get(class) {
        experience.xp_to_next_level = xp_to_next_level_for(class_def, experience.level);
    }
    // If class has no authored def, leave the cached value unchanged (safe no-op).
}

/// Compute the stat gains from a single level-up.
///
/// **Pure function** — no mutations. The `rng` parameter is reserved for future
/// stochastic stat growth (Wizardry-style dice rolls). In v1, gains are
/// deterministic: `class_def.growth_per_level` per field + `hp_per_level` / `mp_per_level`.
///
/// At level cap (99), returns all-zero gains with `new_xp_to_next_level = u64::MAX`
/// so the threshold system never triggers another level-up.
pub fn level_up(
    _current: &BaseStats,
    current_level: u32,
    class_def: &ClassDef,
    _rng: &mut (impl rand::Rng + ?Sized),
) -> StatGains {
    if current_level >= level_cap() {
        return StatGains {
            new_xp_to_next_level: u64::MAX,
            ..Default::default()
        };
    }
    let next_level = current_level.saturating_add(1);
    StatGains {
        strength: class_def.growth_per_level.strength,
        intelligence: class_def.growth_per_level.intelligence,
        piety: class_def.growth_per_level.piety,
        vitality: class_def.growth_per_level.vitality,
        agility: class_def.growth_per_level.agility,
        luck: class_def.growth_per_level.luck,
        hp: class_def.hp_per_level,
        mp: class_def.mp_per_level,
        new_xp_to_next_level: xp_for_level(next_level.saturating_add(1), class_def),
    }
}

/// Roll a random bonus pool for character creation (user decision Q1=1B).
///
/// Draws uniformly from `[bonus_pool_min, bonus_pool_max]`. When both are zero
/// (class didn't author them), defaults to `[5, 9]`.
///
/// **Trust boundary:** clamps `bonus_pool_min`/`max` to `[0, 100]` to prevent a
/// crafted RON from giving infinite bonus points.
pub fn roll_bonus_pool(class_def: &ClassDef, rng: &mut (impl rand::Rng + ?Sized)) -> u32 {
    let raw_min = class_def.bonus_pool_min.clamp(0, 100);
    let raw_max = class_def.bonus_pool_max.clamp(0, 100);

    let (lo, hi) = if raw_min == 0 && raw_max == 0 {
        (5u32, 9u32)
    } else {
        (raw_min.min(raw_max), raw_min.max(raw_max))
    };

    if lo == hi {
        return lo;
    }
    rng.random_range(lo..=hi)
}

/// Apply `allocations` and `race_modifiers` to `base`.
///
/// - Checks that `sum(allocations) <= pool`. Returns `Err(OverPool)` if not.
/// - Adds each allocation to the corresponding stat via `saturating_add`.
/// - Applies race modifiers via `saturating_add_signed(field as i16)` (Q3 i16
///   bit-pattern encoding — see `races.rs` module doc).
pub fn allocate_bonus_pool(
    base: &mut BaseStats,
    allocations: &[u16; 6],
    pool: u32,
    race_modifiers: &BaseStats,
) -> Result<(), AllocError> {
    let allocated: u32 = allocations.iter().map(|&v| v as u32).sum();
    if allocated > pool {
        return Err(AllocError::OverPool { allocated, pool });
    }

    // Apply allocations (non-negative).
    base.strength = base.strength.saturating_add(allocations[0]);
    base.intelligence = base.intelligence.saturating_add(allocations[1]);
    base.piety = base.piety.saturating_add(allocations[2]);
    base.vitality = base.vitality.saturating_add(allocations[3]);
    base.agility = base.agility.saturating_add(allocations[4]);
    base.luck = base.luck.saturating_add(allocations[5]);

    // Apply race modifiers (signed i16 bit-pattern encoding).
    base.strength = base
        .strength
        .saturating_add_signed(race_modifiers.strength as i16);
    base.intelligence = base
        .intelligence
        .saturating_add_signed(race_modifiers.intelligence as i16);
    base.piety = base
        .piety
        .saturating_add_signed(race_modifiers.piety as i16);
    base.vitality = base
        .vitality
        .saturating_add_signed(race_modifiers.vitality as i16);
    base.agility = base
        .agility
        .saturating_add_signed(race_modifiers.agility as i16);
    base.luck = base
        .luck
        .saturating_add_signed(race_modifiers.luck as i16);

    Ok(())
}

/// Check whether the given `race` + `base` stats are eligible for `class_def`.
///
/// Checks (in order):
/// 1. Race is in `allowed_races` (empty list = all races allowed).
/// 2. Each stat in `base` meets the minimum in `class_def.min_stats`.
///
/// **Trust boundary:** `min_stats` fields are clamped to `[3, 18]` (Wizardry
/// hard cap) so a crafted RON `min_stats { strength: 65535 }` doesn't
/// permanently reject every character.
///
/// Returns `Ok(())` if eligible, `Err(CreateError::...)` on the FIRST failing check.
pub fn can_create_class(
    race: Race,
    base: &BaseStats,
    class_def: &ClassDef,
) -> Result<(), CreateError> {
    // Race check.
    if !class_def.allowed_races.is_empty() && !class_def.allowed_races.contains(&race) {
        return Err(CreateError::DisallowedRace { race });
    }

    // Min-stat checks (clamped to [3, 18] at trust boundary).
    let ms = &class_def.min_stats;
    let checks: [(&'static str, u16, u16); 6] = [
        ("strength", ms.strength.clamp(3, 18), base.strength),
        ("intelligence", ms.intelligence.clamp(3, 18), base.intelligence),
        ("piety", ms.piety.clamp(3, 18), base.piety),
        ("vitality", ms.vitality.clamp(3, 18), base.vitality),
        ("agility", ms.agility.clamp(3, 18), base.agility),
        ("luck", ms.luck.clamp(3, 18), base.luck),
    ];

    for (stat, required, actual) in checks {
        // Only enforce when the class authored a non-trivial minimum (> 3 is the
        // Wizardry practical floor; 0 would block everything via the clamp to 3).
        // We skip stats where min_stats is 0 (not authored) — interpret 0 as "no
        // requirement". The trust-boundary clamp only matters for non-zero values.
        let raw_min = match stat {
            "strength" => ms.strength,
            "intelligence" => ms.intelligence,
            "piety" => ms.piety,
            "vitality" => ms.vitality,
            "agility" => ms.agility,
            _ => ms.luck,
        };
        if raw_min == 0 {
            continue; // No requirement authored for this stat.
        }
        if actual < required {
            return Err(CreateError::BelowMinStat {
                stat,
                required,
                actual,
            });
        }
    }

    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// Handler systems
// ─────────────────────────────────────────────────────────────────────────────

/// Split `event.total_xp` among living party members.
///
/// "Living" = not `StatusEffectType::Dead`. Integer-truncates the per-member
/// share (lost remainder is acceptable). Uses `saturating_add` on `current_xp`.
///
/// **Trust boundary:** re-clamps `event.total_xp` to `[0, 1_000_000]` (defense-
/// in-depth against a corrupted `CombatVictoryEvent` that bypasses the producer
/// clamp in `compute_xp_from_enemies`).
pub fn award_combat_xp(
    mut reader: MessageReader<CombatVictoryEvent>,
    mut party: Query<(&mut Experience, &StatusEffects), With<PartyMember>>,
) {
    for event in reader.read() {
        let total_xp = event.total_xp.min(1_000_000) as u64;
        if total_xp == 0 {
            continue;
        }

        let living_count = party
            .iter()
            .filter(|(_, s)| !s.has(StatusEffectType::Dead))
            .count() as u64;

        if living_count == 0 {
            continue;
        }

        let share = total_xp / living_count;
        for (mut exp, status) in &mut party {
            if !status.has(StatusEffectType::Dead) {
                exp.current_xp = exp.current_xp.saturating_add(share);
            }
        }
    }
}

/// For each party member, while `current_xp >= xp_to_next_level` AND
/// `level < level_cap()`, apply the level-up gains and advance the level.
///
/// On level-up, resets `current_hp = new_max_hp` and `current_mp = new_max_mp`
/// (caller-clamp contract from `character.rs:128-131`).
///
/// At level cap 99, `current_xp` accumulates above `xp_to_next_level` and stays
/// there (no information loss — Q7=B; the threshold system early-returns when
/// `level >= level_cap()`).
pub fn apply_level_up_threshold_system(
    mut party: Query<
        (
            &mut Experience,
            &mut BaseStats,
            &mut DerivedStats,
            &Class,
            &StatusEffects,
        ),
        With<PartyMember>,
    >,
    table_assets: Option<Res<Assets<ClassTable>>>,
    town_assets: Option<Res<TownAssets>>,
    mut rng: ResMut<ProgressionRng>,
) {
    // Resolve the class table once per frame.
    let Some(class_assets) = table_assets else {
        return;
    };
    let Some(town) = town_assets else {
        return;
    };
    let Some(table) = class_assets.get(&town.class_table) else {
        return;
    };

    for (mut exp, mut base, mut derived, class, status) in &mut party {
        // Dead characters don't level up.
        if status.has(StatusEffectType::Dead) {
            continue;
        }

        // Level cap — XP accumulates above threshold, no further level-up.
        while exp.current_xp >= exp.xp_to_next_level && exp.level < level_cap() {
            let Some(class_def) = table.get(*class) else {
                break;
            };

            let gains = level_up(&base, exp.level, class_def, &mut *rng.0);

            // Apply stat gains (saturating per-field).
            base.strength = base.strength.saturating_add(gains.strength);
            base.intelligence = base.intelligence.saturating_add(gains.intelligence);
            base.piety = base.piety.saturating_add(gains.piety);
            base.vitality = base.vitality.saturating_add(gains.vitality);
            base.agility = base.agility.saturating_add(gains.agility);
            base.luck = base.luck.saturating_add(gains.luck);

            exp.level = exp.level.saturating_add(1);

            // Feature #20 — award skill points on level-up (Pattern 6 of research).
            exp.unspent_skill_points = exp
                .unspent_skill_points
                .saturating_add(SKILL_POINTS_PER_LEVEL);
            exp.total_skill_points_earned = exp
                .total_skill_points_earned
                .saturating_add(SKILL_POINTS_PER_LEVEL);

            // Re-derive DerivedStats and reset HP/MP to new max (level-up contract).
            let new_derived = derive_stats(&base, &[], status, exp.level);
            derived.max_hp = new_derived.max_hp;
            derived.current_hp = new_derived.max_hp; // reset to max on level-up
            derived.max_mp = new_derived.max_mp;
            derived.current_mp = new_derived.max_mp; // reset to max on level-up
            derived.attack = new_derived.attack;
            derived.defense = new_derived.defense;
            derived.magic_attack = new_derived.magic_attack;
            derived.magic_defense = new_derived.magic_defense;
            derived.speed = new_derived.speed;
            derived.accuracy = new_derived.accuracy;
            derived.evasion = new_derived.evasion;

            // Update the cached XP threshold.
            exp.xp_to_next_level = gains.new_xp_to_next_level;
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Plugin
// ─────────────────────────────────────────────────────────────────────────────

/// Registers the progression RNG, `CombatVictoryEvent`, and the two handler
/// systems.
///
/// No state gating: level-up can happen at the next-frame boundary regardless
/// of `GameState` (defense-in-depth: lets a delayed `CombatVictoryEvent` from
/// the previous frame be drained even after entering `Town`/`Dungeon`).
pub struct PartyProgressionPlugin;

impl Plugin for PartyProgressionPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ProgressionRng>()
            .add_message::<CombatVictoryEvent>()
            .add_systems(
                Update,
                (
                    award_combat_xp,
                    apply_level_up_threshold_system.after(award_combat_xp),
                ),
            );
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use rand_chacha::ChaCha8Rng;

    use crate::data::classes::ClassRequirement;

    // ── Helpers ──────────────────────────────────────────────────────────────

    fn fighter_class_def() -> ClassDef {
        ClassDef {
            id: Class::Fighter,
            display_name: "Fighter".into(),
            starting_stats: BaseStats {
                strength: 14,
                intelligence: 8,
                piety: 8,
                vitality: 14,
                agility: 10,
                luck: 9,
            },
            growth_per_level: BaseStats {
                strength: 2,
                intelligence: 0,
                piety: 0,
                vitality: 2,
                agility: 1,
                luck: 0,
            },
            hp_per_level: 8,
            mp_per_level: 0,
            xp_to_level_2: 100,
            xp_curve_factor: 1.5,
            min_stats: BaseStats {
                strength: 11,
                ..BaseStats::ZERO
            },
            allowed_races: vec![Race::Human, Race::Elf, Race::Dwarf, Race::Gnome, Race::Hobbit],
            advancement_requirements: vec![],
            bonus_pool_min: 5,
            bonus_pool_max: 9,
            stat_penalty_on_change: BaseStats::ZERO,
        }
    }

    fn mage_class_def() -> ClassDef {
        ClassDef {
            id: Class::Mage,
            display_name: "Mage".into(),
            starting_stats: BaseStats {
                strength: 7,
                intelligence: 14,
                piety: 7,
                vitality: 8,
                agility: 10,
                luck: 10,
            },
            growth_per_level: BaseStats {
                strength: 0,
                intelligence: 2,
                piety: 0,
                vitality: 1,
                agility: 1,
                luck: 1,
            },
            hp_per_level: 4,
            mp_per_level: 6,
            xp_to_level_2: 100,
            xp_curve_factor: 1.5,
            min_stats: BaseStats {
                intelligence: 11,
                ..BaseStats::ZERO
            },
            allowed_races: vec![Race::Human, Race::Elf, Race::Gnome, Race::Hobbit],
            advancement_requirements: vec![ClassRequirement {
                from_class: Class::Fighter,
                min_level: 5,
            }],
            bonus_pool_min: 5,
            bonus_pool_max: 9,
            stat_penalty_on_change: BaseStats::ZERO,
        }
    }

    // ── xp_for_level ─────────────────────────────────────────────────────────

    /// Fighter XP curve: L2=100, L3=150, L4=225 (100 * 1.5^n).
    #[test]
    fn xp_curve_matches_formula_fighter() {
        let def = fighter_class_def();
        assert_eq!(xp_for_level(2, &def), 100, "L2 == xp_to_level_2");
        assert_eq!(xp_for_level(3, &def), 150, "L3 == 100 * 1.5");
        assert_eq!(xp_for_level(4, &def), 225, "L4 == 100 * 1.5^2");
    }

    /// Very high level or factor=10 causes overflow → u64::MAX.
    #[test]
    fn xp_curve_saturates_at_u64_max() {
        let mut def = fighter_class_def();
        def.xp_curve_factor = 10.0;
        let result = xp_for_level(1000, &def);
        assert_eq!(result, u64::MAX, "overflow must saturate to u64::MAX");
    }

    /// Non-finite factor (NaN) is treated as 1.0 → returns `xp_to_level_2`.
    #[test]
    fn xp_curve_rejects_non_finite_factor() {
        let mut def = fighter_class_def();
        def.xp_curve_factor = f32::NAN;
        // With factor=1.0 (safe fallback), level 2 = base = 100.
        let result = xp_for_level(2, &def);
        assert_eq!(result, 100, "NaN factor must fall back to 1.0");
    }

    // ── level_up ─────────────────────────────────────────────────────────────

    /// L1 → L2 Fighter gains match `growth_per_level`.
    #[test]
    fn level_up_fighter_l1_to_l2_yields_correct_gains() {
        let def = fighter_class_def();
        let base = def.starting_stats;
        let mut rng = ChaCha8Rng::seed_from_u64(42);
        let gains = level_up(&base, 1, &def, &mut rng);
        assert_eq!(gains.strength, 2);
        assert_eq!(gains.vitality, 2);
        assert_eq!(gains.agility, 1);
        assert_eq!(gains.hp, 8);
        assert_eq!(gains.mp, 0);
        // new_xp_to_next_level = xp_for_level(3, def) = 150
        assert_eq!(gains.new_xp_to_next_level, 150);
    }

    /// At level cap (99), level_up returns all-zero gains.
    #[test]
    fn level_up_at_cap_returns_zero_gains() {
        let def = fighter_class_def();
        let base = def.starting_stats;
        let mut rng = ChaCha8Rng::seed_from_u64(0);
        let gains = level_up(&base, level_cap(), &def, &mut rng);
        assert_eq!(gains.strength, 0);
        assert_eq!(gains.hp, 0);
        assert_eq!(
            gains.new_xp_to_next_level,
            u64::MAX,
            "cap level must return u64::MAX threshold"
        );
    }

    // ── roll_bonus_pool ───────────────────────────────────────────────────────

    /// Seeded RNG produces a known deterministic value.
    #[test]
    fn bonus_pool_roll_seeded_deterministic() {
        let def = fighter_class_def();
        let mut rng = ChaCha8Rng::seed_from_u64(42);
        let pool1 = roll_bonus_pool(&def, &mut rng);
        // Verify it falls within the authored range.
        assert!(
            (5..=9).contains(&pool1),
            "pool {pool1} must be in [5, 9]"
        );
        // Same seed → same first result.
        let mut rng2 = ChaCha8Rng::seed_from_u64(42);
        let pool2 = roll_bonus_pool(&def, &mut rng2);
        assert_eq!(pool1, pool2, "seeded RNG must be deterministic");
    }

    /// Bonus pool result lies within the authored class range.
    #[test]
    fn bonus_pool_uses_class_authored_range() {
        let def = fighter_class_def(); // min=5, max=9
        let mut rng = ChaCha8Rng::seed_from_u64(99);
        for _ in 0..100 {
            let pool = roll_bonus_pool(&def, &mut rng);
            assert!(
                (5..=9).contains(&pool),
                "pool {pool} is outside authored range [5, 9]"
            );
        }
    }

    // ── allocate_bonus_pool ───────────────────────────────────────────────────

    /// Over-allocation is rejected with `OverPool`.
    #[test]
    fn allocate_bonus_rejects_overflow() {
        let mut base = BaseStats {
            strength: 10,
            intelligence: 8,
            piety: 8,
            vitality: 10,
            agility: 8,
            luck: 8,
        };
        let allocations = [5u16, 0, 0, 0, 0, 5]; // sum=10 > pool=9
        let result = allocate_bonus_pool(&mut base, &allocations, 9, &BaseStats::ZERO);
        assert!(matches!(result, Err(AllocError::OverPool { .. })));
    }

    /// Race modifiers are applied via saturating_add_signed (i16 encoding).
    #[test]
    fn allocate_bonus_applies_race_modifiers_via_saturating_add_signed() {
        let mut base = BaseStats {
            strength: 10,
            intelligence: 8,
            piety: 8,
            vitality: 10,
            agility: 8,
            luck: 8,
        };
        // Elf: STR-1, INT+2, PIE+1, VIT-2, AGI+1, LCK-1
        let elf_mods = BaseStats {
            strength: 65535,     // -1 as u16
            intelligence: 2,
            piety: 1,
            vitality: 65534,     // -2 as u16
            agility: 1,
            luck: 65535,         // -1 as u16
        };
        let allocations = [0u16; 6];
        allocate_bonus_pool(&mut base, &allocations, 0, &elf_mods).unwrap();

        assert_eq!(base.strength, 9, "STR 10 + (-1) == 9");
        assert_eq!(base.intelligence, 10, "INT 8 + 2 == 10");
        assert_eq!(base.piety, 9, "PIE 8 + 1 == 9");
        assert_eq!(base.vitality, 8, "VIT 10 + (-2) == 8");
        assert_eq!(base.agility, 9, "AGI 8 + 1 == 9");
        assert_eq!(base.luck, 7, "LCK 8 + (-1) == 7");
    }

    // ── can_create_class ──────────────────────────────────────────────────────

    /// Stat below the class minimum is rejected.
    #[test]
    fn can_create_class_enforces_min_stats() {
        let def = fighter_class_def(); // requires STR >= 11
        let low_str = BaseStats {
            strength: 8, // below 11
            intelligence: 8,
            piety: 8,
            vitality: 10,
            agility: 8,
            luck: 8,
        };
        let result = can_create_class(Race::Human, &low_str, &def);
        assert!(
            matches!(result, Err(CreateError::BelowMinStat { stat: "strength", .. })),
            "expected BelowMinStat for strength, got {:?}",
            result
        );
    }

    /// Disallowed race is rejected.
    #[test]
    fn can_create_class_enforces_allowed_races() {
        let def = mage_class_def(); // Dwarves not in allowed_races
        let good_stats = BaseStats {
            strength: 7,
            intelligence: 14, // meets min INT 11
            piety: 7,
            vitality: 8,
            agility: 10,
            luck: 10,
        };
        let result = can_create_class(Race::Dwarf, &good_stats, &def);
        assert!(
            matches!(result, Err(CreateError::DisallowedRace { race: Race::Dwarf })),
            "expected DisallowedRace for Dwarf, got {:?}",
            result
        );
    }

    /// Eligible race + stats → Ok.
    #[test]
    fn can_create_class_accepts_eligible() {
        let def = fighter_class_def();
        let good_stats = BaseStats {
            strength: 14,
            intelligence: 8,
            piety: 8,
            vitality: 14,
            agility: 10,
            luck: 9,
        };
        assert!(can_create_class(Race::Human, &good_stats, &def).is_ok());
    }

    // ── Integration: XP threshold triggers level-up ───────────────────────────

    /// Place a fighter at level 1 with XP just below threshold, bump over it,
    /// run one Update, assert level incremented and HP reset.
    #[test]
    fn xp_threshold_triggers_level_up() {
        use bevy::asset::AssetPlugin;
        use bevy::state::app::StatesPlugin;

        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), StatesPlugin));

        // Register assets needed by the system.
        app.init_asset::<ClassTable>();
        use crate::data::RaceTable;
        app.init_asset::<RaceTable>();

        // Build a mock class table with the fighter def.
        let mut class_table = ClassTable::default();
        class_table.classes.push(fighter_class_def());
        let class_handle = app
            .world_mut()
            .resource_mut::<Assets<ClassTable>>()
            .add(class_table);

        // Insert a mock TownAssets pointing at our class table.
        use crate::data::{RecruitPool, ShopStock, TownServices};
        app.init_asset::<RecruitPool>();
        app.init_asset::<ShopStock>();
        app.init_asset::<TownServices>();
        let mock_town_assets = TownAssets {
            shop_stock: Handle::default(),
            recruit_pool: Handle::default(),
            services: Handle::default(),
            race_table: Handle::default(),
            class_table: class_handle,
        };
        app.insert_resource(mock_town_assets);

        // Insert seeded ProgressionRng.
        app.insert_resource(ProgressionRng(Box::new(
            ChaCha8Rng::seed_from_u64(42),
        )));

        app.add_message::<CombatVictoryEvent>();
        app.add_systems(Update, apply_level_up_threshold_system);

        // Spawn a level-1 fighter with xp_to_next_level=100, current_xp=120.
        // 120 ∈ [100, 150) — overshoots L2 cumulative threshold (100) but stays
        // below L3 threshold (150), so the drain loop triggers exactly one
        // level-up.
        let base = BaseStats {
            strength: 14,
            intelligence: 8,
            piety: 8,
            vitality: 14,
            agility: 10,
            luck: 9,
        };
        let derived = derive_stats(&base, &[], &StatusEffects::default(), 1);
        let entity = app
            .world_mut()
            .spawn((
                PartyMember,
                Class::Fighter,
                base,
                derived,
                Experience {
                    level: 1,
                    current_xp: 120,
                    xp_to_next_level: 100,
                    ..Default::default()
                },
                StatusEffects::default(),
            ))
            .id();

        app.update();

        let world = app.world();
        let exp = world.get::<Experience>(entity).unwrap();
        assert_eq!(exp.level, 2, "character should have levelled up to 2");
        assert_eq!(exp.current_xp, 120, "XP is not consumed — accumulates");
        // xp_to_next_level for L3 = xp_for_level(3, fighter) = 150
        assert_eq!(
            exp.xp_to_next_level, 150,
            "xp_to_next_level should now be L3 threshold"
        );

        // After level-up, current_hp must equal max_hp (caller-clamp contract).
        let derived = world.get::<DerivedStats>(entity).unwrap();
        assert_eq!(
            derived.current_hp, derived.max_hp,
            "current_hp must be reset to max_hp on level-up"
        );

        // Feature #20 — level-up must award SKILL_POINTS_PER_LEVEL skill points.
        assert_eq!(
            exp.unspent_skill_points,
            SKILL_POINTS_PER_LEVEL,
            "one level-up must award SKILL_POINTS_PER_LEVEL ({}) unspent skill points",
            SKILL_POINTS_PER_LEVEL
        );
        assert_eq!(
            exp.total_skill_points_earned,
            SKILL_POINTS_PER_LEVEL,
            "one level-up must award SKILL_POINTS_PER_LEVEL ({}) total skill points earned",
            SKILL_POINTS_PER_LEVEL
        );
    }

    /// Level-up awards SKILL_POINTS_PER_LEVEL SP per level — extended assertion.
    #[test]
    fn level_up_awards_skill_points_per_const() {
        use bevy::asset::AssetPlugin;
        use bevy::state::app::StatesPlugin;

        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), StatesPlugin));
        app.init_asset::<ClassTable>();
        use crate::data::RaceTable;
        app.init_asset::<RaceTable>();

        let mut class_table = ClassTable::default();
        class_table.classes.push(fighter_class_def());
        let class_handle = app
            .world_mut()
            .resource_mut::<Assets<ClassTable>>()
            .add(class_table);

        use crate::data::{RecruitPool, ShopStock, TownServices};
        app.init_asset::<RecruitPool>();
        app.init_asset::<ShopStock>();
        app.init_asset::<TownServices>();
        let mock_town_assets = TownAssets {
            shop_stock: Handle::default(),
            recruit_pool: Handle::default(),
            services: Handle::default(),
            race_table: Handle::default(),
            class_table: class_handle,
        };
        app.insert_resource(mock_town_assets);
        app.insert_resource(ProgressionRng(Box::new(ChaCha8Rng::seed_from_u64(7))));
        app.add_message::<CombatVictoryEvent>();
        app.add_systems(Update, apply_level_up_threshold_system);

        let base = BaseStats {
            strength: 14,
            intelligence: 8,
            piety: 8,
            vitality: 14,
            agility: 10,
            luck: 9,
        };
        let derived = derive_stats(&base, &[], &StatusEffects::default(), 1);
        // Give enough XP for exactly 2 level-ups. Accumulating semantics (#19 Q7=B):
        //   L1→L2 at 100, L2→L3 at 150, L3→L4 at 225. 200 ≥ 100 + 150 thresholds, < 225.
        let entity = app
            .world_mut()
            .spawn((
                PartyMember,
                Class::Fighter,
                base,
                derived,
                Experience {
                    level: 1,
                    current_xp: 200,
                    xp_to_next_level: 100,
                    ..Default::default()
                },
                StatusEffects::default(),
            ))
            .id();

        app.update();

        let exp = app.world().get::<Experience>(entity).unwrap();
        assert_eq!(exp.level, 3, "should level up twice from current_xp=250");
        assert_eq!(
            exp.unspent_skill_points,
            SKILL_POINTS_PER_LEVEL * 2,
            "two level-ups must award 2 × SKILL_POINTS_PER_LEVEL unspent SP"
        );
        assert_eq!(
            exp.total_skill_points_earned,
            SKILL_POINTS_PER_LEVEL * 2,
            "two level-ups must award 2 × SKILL_POINTS_PER_LEVEL total SP"
        );
    }
}
