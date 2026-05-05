//! Character ECS components, bundle, and `derive_stats` pure function.
//!
//! Source: `research/20260504-160000-feature-11-party-character-ecs-model.md`
//! Pattern: research §Pattern 3 — components-as-data with serde from day one.
//!
//! **Single-file per Decision 4** (matches #9/#10 single-file precedent).
//! Do NOT pre-split into `inventory.rs` / `progression.rs` — those submodules
//! only make sense when their first systems arrive in #12 / #14.
//!
//! **Type ownership note:** `Class` and `BaseStats` are defined here even
//! though `src/data/classes.rs` also needs them. That creates a one-way
//! reverse dependency (`data/` imports from `plugins/party/character`).
//! This is intentional — see the reverse-dep comment in `data/classes.rs`
//! and the #11 plan §Critical for the rationale.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::data::items::{ItemAsset, ItemStatBlock};

// ─────────────────────────────────────────────────────────────────────────────
// 3a. Identity components
// ─────────────────────────────────────────────────────────────────────────────

/// Display name for a character (party member or NPC).
#[derive(
    Component, Reflect, Serialize, Deserialize, Default, Debug, Clone, PartialEq, Eq, Hash,
)]
pub struct CharacterName(pub String);

/// Playable race.
///
/// Per Decision 2: 5 variants declared, only `Human` used in v1 by
/// `spawn_default_debug_party`. Discriminant order is locked for
/// save-format stability (research §Pitfall 5).
#[derive(
    Component, Reflect, Serialize, Deserialize, Default, Debug, Clone, Copy, PartialEq, Eq, Hash,
)]
pub enum Race {
    #[default]
    Human,
    Elf,
    Dwarf,
    Gnome,
    Hobbit,
}

/// Character class determining stat scaling and spell access.
///
/// Per Decision 1: 8 variants declared; only `Fighter`, `Mage`, and `Priest`
/// have `ClassDef` entries in `core.classes.ron` for v1. The remaining five
/// (`Thief`, `Bishop`, `Samurai`, `Lord`, `Ninja`) are declared to lock the
/// discriminant order for save-format stability.
///
/// `derive_stats` and `ClassTable::get` use `Option` returns and wildcard arms
/// rather than exhaustive `match` so that the unauthored five do not cause
/// compile errors. Never add an exhaustive `match Class { ... }` without a
/// wildcard arm.
#[derive(
    Component, Reflect, Serialize, Deserialize, Default, Debug, Clone, Copy, PartialEq, Eq, Hash,
)]
pub enum Class {
    #[default]
    Fighter,
    Mage,
    Priest,
    Thief,
    Bishop,
    Samurai,
    Lord,
    Ninja,
}

/// Zero-sized marker distinguishing party members from NPCs (#18) and
/// enemies (#15) that share stat components like `CharacterName` and
/// `BaseStats`. Same structural pattern as `PlayerParty` in
/// `src/plugins/dungeon/mod.rs`, `DungeonGeometry`, and `Torch`.
#[derive(
    Component, Reflect, Serialize, Deserialize, Default, Debug, Clone, Copy, PartialEq, Eq, Hash,
)]
pub struct PartyMember;

// ─────────────────────────────────────────────────────────────────────────────
// 3b. Stats components
// ─────────────────────────────────────────────────────────────────────────────

/// Six primary base stats — the immutable core that `derive_stats` scales.
///
/// - `strength`     — physical attack power and carry capacity.
/// - `intelligence` — magic attack and MP pool scaling.
/// - `piety`        — divine magic efficacy and healing power.
/// - `vitality`     — HP pool and physical defense scaling.
/// - `agility`      — speed, accuracy, and evasion.
/// - `luck`         — critical hit rate, trap detection, item drop rates.
#[derive(
    Component, Reflect, Serialize, Deserialize, Default, Debug, Clone, Copy, PartialEq, Eq, Hash,
)]
pub struct BaseStats {
    pub strength: u16,
    pub intelligence: u16,
    pub piety: u16,
    pub vitality: u16,
    pub agility: u16,
    pub luck: u16,
}

impl BaseStats {
    /// All-zero constant, useful in tests and as a known-baseline for
    /// `derive_stats` assertions.
    pub const ZERO: Self = Self {
        strength: 0,
        intelligence: 0,
        piety: 0,
        vitality: 0,
        agility: 0,
        luck: 0,
    };
}

/// Computed stats derived from `BaseStats`, equipment, and status effects.
///
/// Written by callers of `derive_stats`. `Hash` is deliberately omitted:
/// even though all fields are currently `u32`, the computation path passes
/// through `f32` arithmetic (status-effect magnitude in #15) and adding
/// `Hash` then would require a custom impl or float conversion — leaving it
/// off now avoids the footgun.
///
/// **Caller-clamp pattern (OQ1):** `derive_stats` returns `current_hp = max_hp`
/// and `current_mp = max_mp`. Callers in #14 / #15 must clamp:
/// - Level-up → reset to max.
/// - Equipment change → `current = current.min(new_max)`.
#[derive(Component, Reflect, Serialize, Deserialize, Default, Debug, Clone, Copy, PartialEq)]
pub struct DerivedStats {
    pub max_hp: u32,
    pub current_hp: u32,
    pub max_mp: u32,
    pub current_mp: u32,
    pub attack: u32,
    pub defense: u32,
    pub magic_attack: u32,
    pub magic_defense: u32,
    pub speed: u32,
    pub accuracy: u32,
    pub evasion: u32,
}

/// XP tracker with cached `xp_to_next_level` (OQ2: cached, not recomputed
/// on every read). A pure `xp_for_level(level, curve) -> u64` helper is
/// deferred to #14 (progression).
#[derive(
    Component, Reflect, Serialize, Deserialize, Default, Debug, Clone, Copy, PartialEq, Eq, Hash,
)]
pub struct Experience {
    pub level: u32,
    pub current_xp: u64,
    /// Cached threshold for this level transition. Updated by the level-up
    /// system in #14 whenever `current_xp >= xp_to_next_level`.
    pub xp_to_next_level: u64,
}

// ─────────────────────────────────────────────────────────────────────────────
// 3c. Position components
// ─────────────────────────────────────────────────────────────────────────────

/// Formation row per Wizardry/Etrian Odyssey convention.
///
/// - `Front` — melee range; targeted first by enemies.
/// - `Back`  — casters; reduced melee damage taken; some melee skills cannot reach.
#[derive(
    Component, Reflect, Serialize, Deserialize, Default, Debug, Clone, Copy, PartialEq, Eq, Hash,
)]
pub enum PartyRow {
    #[default]
    Front,
    Back,
}

/// Index of this character within the party (0..`PartySize.0`).
///
/// Slot can change (formation reorder in #19); the `PartyMember` marker is
/// invariant. These two concerns are intentionally separate components.
#[derive(
    Component, Reflect, Serialize, Deserialize, Default, Debug, Clone, Copy, PartialEq, Eq, Hash,
)]
pub struct PartySlot(pub usize);

// ─────────────────────────────────────────────────────────────────────────────
// 3d. Equipment component
// ─────────────────────────────────────────────────────────────────────────────

/// Equipment loadout for one character — eight `Option<Handle<ItemAsset>>` slots.
///
/// Per Decision 3: stores `Handle<ItemAsset>`, NOT `Entity`. This keeps
/// `derive_stats` pure (no entity lookups).
///
/// **Serde deviation (discovered in #11 implementation):** `Handle<T>` in Bevy
/// 0.18 does NOT implement `Serialize`/`Deserialize`. The plan stated "Handle
/// serializes cleanly as an asset path" — this is incorrect for Bevy 0.18.
/// `Equipment` therefore cannot derive `Serialize + Deserialize` unlike the
/// other 11 components. Feature #23 (save/load) must implement custom serde
/// for `Equipment` (e.g., serialize each slot as `Option<AssetPath>` and
/// re-resolve handles on load). Tracked in #11 Implementation Discoveries.
///
/// Per-instance state (enchantment, durability, custom name) lands in #12
/// as a separate `ItemInstance` entity model.
///
/// `Hash` is omitted because `Handle<T>` does not implement `Hash` by default.
/// `Eq` is omitted for the same reason (PartialEq is available via Handle's impl).
#[derive(Component, Reflect, Default, Debug, Clone, PartialEq)]
pub struct Equipment {
    pub weapon: Option<Handle<ItemAsset>>,
    pub armor: Option<Handle<ItemAsset>>,
    pub shield: Option<Handle<ItemAsset>>,
    pub helm: Option<Handle<ItemAsset>>,
    pub gloves: Option<Handle<ItemAsset>>,
    pub boots: Option<Handle<ItemAsset>>,
    pub accessory_1: Option<Handle<ItemAsset>>,
    pub accessory_2: Option<Handle<ItemAsset>>,
}

// ─────────────────────────────────────────────────────────────────────────────
// 3e. Status effect components
// ─────────────────────────────────────────────────────────────────────────────

/// V1 negative status set.
///
/// Per Decision 7: `Dead` is a variant here — NOT a separate marker component.
/// `derive_stats` branches on `Dead`; combat (#15) checks
/// `status.has(StatusEffectType::Dead)`. Buffs (`AttackUp`, `DefenseUp`,
/// etc.) are deferred to #15.
///
/// The `magnitude` field on `ActiveEffect` is part of the schema for #15
/// buff types but is **unused by v1 status types in `derive_stats`** — the
/// v1 types are pure gates or tick-on-turn effects with no stat modifier.
#[derive(Reflect, Serialize, Deserialize, Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StatusEffectType {
    #[default]
    Poison,
    Sleep,
    Paralysis,
    Stone,
    Dead,
}

/// One active status instance on a character.
///
/// - `Stone` and `Dead` are non-tickable: use `remaining_turns: None`.
/// - `Poison` ticks per turn in #15.
/// - `magnitude` is part of the schema for #15 buffs (e.g., `AttackUp 0.5`
///   = +50%); v1 status types do not use it.
///
/// `Eq` and `Hash` are omitted because `f32` does not implement them.
#[derive(Reflect, Serialize, Deserialize, Default, Debug, Clone, Copy, PartialEq)]
pub struct ActiveEffect {
    pub effect_type: StatusEffectType,
    /// `None` for permanent/non-tickable effects (Stone, Dead).
    /// `Some(n)` for temporary effects (Poison, Sleep, Paralysis).
    pub remaining_turns: Option<u32>,
    /// Unused by v1 status types; reserved for #15 magnitude-modifying buffs.
    pub magnitude: f32,
}

/// All active status effects on one character.
#[derive(Component, Reflect, Serialize, Deserialize, Default, Debug, Clone, PartialEq)]
pub struct StatusEffects {
    pub effects: Vec<ActiveEffect>,
}

impl StatusEffects {
    /// Returns `true` if `kind` is currently active on this character.
    pub fn has(&self, kind: StatusEffectType) -> bool {
        self.effects.iter().any(|e| e.effect_type == kind)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 3f. PartyMemberBundle
// ─────────────────────────────────────────────────────────────────────────────

/// Convenience bundle for spawning a complete party member entity.
///
/// No `Reflect`/`Serialize`/`Deserialize` — `Bundle` is a spawn helper, not
/// a stored value. Each component inside carries its own serde derives.
#[derive(Bundle, Default)]
pub struct PartyMemberBundle {
    pub marker: PartyMember,
    pub name: CharacterName,
    pub race: Race,
    pub class: Class,
    pub base_stats: BaseStats,
    pub derived_stats: DerivedStats,
    pub experience: Experience,
    pub party_row: PartyRow,
    pub party_slot: PartySlot,
    pub equipment: Equipment,
    pub status_effects: StatusEffects,
}

// ─────────────────────────────────────────────────────────────────────────────
// 3i. PartySize resource
// ─────────────────────────────────────────────────────────────────────────────

/// Maximum number of simultaneous party members (hard cap = 4 in v1).
///
/// Per Decision 6: `spawn_default_debug_party` refuses to spawn the
/// (n+1)th character. Feature #19 (character creation) may reduce this
/// at game-start for specific scenarios; default is always 4.
#[derive(Resource, Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PartySize(pub usize);

impl Default for PartySize {
    fn default() -> Self {
        Self(4)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 3g. derive_stats pure function
// ─────────────────────────────────────────────────────────────────────────────

/// Compute `DerivedStats` from base stats, equipped items, status effects, and level.
///
/// **PURE** — no `Mut<T>`, no entity lookups, no resource reads, no randomness.
/// Callers are responsible for flattening `Equipment` + `Assets<ItemAsset>`
/// into `&[ItemStatBlock]`; this keeps `derive_stats` testable without asset access.
///
/// **Caller-clamp contract:** returns `current_hp = max_hp` and
/// `current_mp = max_mp` as sane defaults. Callers must clamp:
/// - Level-up → reset to max.
/// - Equipment change → `current = current.min(new_max)`.
///
/// **Saturating arithmetic:** all addition and multiplication paths use
/// `saturating_*` to bound the trust boundary on `classes.ron` and
/// `items.ron` data (research §Security; see `derive_stats_saturating_arithmetic`
/// test).
///
/// **V1 status note:** v1 status types (`Poison`, `Sleep`, `Paralysis`,
/// `Stone`, `Dead`) are trivially order-independent because none of them
/// modify a stat via the `magnitude` field — they are pure gates.
/// #15 will add magnitude-modifying buff branches; at that point, order
/// dependence must be re-evaluated and the deferred
/// `derive_stats_status_order_independent` test should be written.
pub fn derive_stats(
    base: &BaseStats,
    equip_stats: &[ItemStatBlock],
    status: &StatusEffects,
    level: u32,
) -> DerivedStats {
    // ── HP / MP from primary stats scaled by level ───────────────────────────
    // VIT drives HP; PIE+INT drive MP.
    // Using level.max(1) avoids multiply-by-zero for level-0 edge cases in tests.
    let effective_level = level.max(1);

    let base_hp = (base.vitality as u32)
        .saturating_mul(effective_level)
        .saturating_add(effective_level.saturating_mul(5));
    let base_mp = ((base.intelligence as u32).saturating_add(base.piety as u32))
        .saturating_mul(effective_level)
        / 2;

    // ── Equipment additive stack ─────────────────────────────────────────────
    let mut equip_attack: u32 = 0;
    let mut equip_defense: u32 = 0;
    let mut equip_magic_attack: u32 = 0;
    let mut equip_magic_defense: u32 = 0;
    let mut equip_accuracy: u32 = 0;
    let mut equip_evasion: u32 = 0;
    let mut equip_hp_bonus: u32 = 0;
    let mut equip_mp_bonus: u32 = 0;

    for item in equip_stats {
        equip_attack = equip_attack.saturating_add(item.attack);
        equip_defense = equip_defense.saturating_add(item.defense);
        equip_magic_attack = equip_magic_attack.saturating_add(item.magic_attack);
        equip_magic_defense = equip_magic_defense.saturating_add(item.magic_defense);
        equip_accuracy = equip_accuracy.saturating_add(item.accuracy);
        equip_evasion = equip_evasion.saturating_add(item.evasion);
        equip_hp_bonus = equip_hp_bonus.saturating_add(item.hp_bonus);
        equip_mp_bonus = equip_mp_bonus.saturating_add(item.mp_bonus);
    }

    // ── Base-stat contributions ──────────────────────────────────────────────
    let stat_attack = (base.strength as u32).saturating_add(equip_attack);
    let stat_defense = (base.vitality as u32 / 2).saturating_add(equip_defense);
    let stat_magic_attack = (base.intelligence as u32).saturating_add(equip_magic_attack);
    let stat_magic_defense = (base.piety as u32 / 2).saturating_add(equip_magic_defense);
    let stat_speed = base.agility as u32;
    let stat_accuracy = (base.agility as u32 / 2)
        .saturating_add(base.luck as u32 / 4)
        .saturating_add(equip_accuracy);
    let stat_evasion = (base.agility as u32 / 4)
        .saturating_add(base.luck as u32 / 4)
        .saturating_add(equip_evasion);

    let mut max_hp = base_hp.saturating_add(equip_hp_bonus);
    let mut max_mp = base_mp.saturating_add(equip_mp_bonus);

    // ── Status effect post-pass (v1 gates only) ──────────────────────────────
    // V1 status types are order-independent (none modify via magnitude).
    // #15 will add magnitude-modifying buff branches here.
    if status.has(StatusEffectType::Dead) {
        // Dead character has no HP or MP pools available.
        max_hp = 0;
        max_mp = 0;
    }
    // Poison, Sleep, Paralysis, Stone: no stat modification at derive time.
    // Poison ticks in #15's combat turn system.
    // Sleep/Paralysis gate action selection in #15.
    // Stone is treated like Dead for targeting purposes in #15.

    DerivedStats {
        max_hp,
        current_hp: max_hp,
        max_mp,
        current_mp: max_mp,
        attack: stat_attack,
        defense: stat_defense,
        magic_attack: stat_magic_attack,
        magic_defense: stat_magic_defense,
        speed: stat_speed,
        accuracy: stat_accuracy,
        evasion: stat_evasion,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Serde round-trip ─────────────────────────────────────────────────────

    /// Round-trip a non-default `BaseStats` through RON and back.
    /// Pattern from `src/data/dungeon.rs:438-455`.
    #[test]
    fn base_stats_round_trips_through_ron() {
        let original = BaseStats {
            strength: 14,
            intelligence: 8,
            piety: 8,
            vitality: 14,
            agility: 10,
            luck: 9,
        };
        let serialized = ron::ser::to_string_pretty(&original, ron::ser::PrettyConfig::default())
            .expect("serialize BaseStats");
        let parsed: BaseStats = ron::de::from_str(&serialized).expect("deserialize BaseStats");
        assert_eq!(original, parsed, "BaseStats RON round-trip lost fields");
    }

    // ── derive_stats: zero baseline ──────────────────────────────────────────

    /// With all-zero base stats and no equipment, `attack` is zero;
    /// `max_hp` and `max_mp` match the level-1 constant baseline
    /// (VIT=0 * level + level * 5 = 5; MP = 0).
    #[test]
    fn derive_stats_returns_zero_for_zero_inputs() {
        let result = derive_stats(&BaseStats::ZERO, &[], &StatusEffects::default(), 1);
        assert_eq!(result.attack, 0);
        assert_eq!(result.max_hp, 5, "level-1 baseline HP (VIT=0 * 1 + 1 * 5)");
        assert_eq!(result.max_mp, 0, "level-1 baseline MP (INT=0 + PIE=0) / 2");
        assert_eq!(result.current_hp, result.max_hp);
        assert_eq!(result.current_mp, result.max_mp);
    }

    // ── derive_stats: equipment stacks additively ────────────────────────────

    /// Two equipped items contribute additively to `attack` and `defense`.
    #[test]
    fn derive_stats_equipment_stacks_additively() {
        let sword = ItemStatBlock {
            attack: 10,
            ..Default::default()
        };
        let armor = ItemStatBlock {
            defense: 5,
            ..Default::default()
        };
        let result = derive_stats(
            &BaseStats::ZERO,
            &[sword, armor],
            &StatusEffects::default(),
            1,
        );
        // ZERO base stats → stat_attack = 0 + equip_attack (10) = 10
        assert_eq!(result.attack, 10, "sword attack stacks");
        // stat_defense = vitality/2 (0) + equip_defense (5) = 5
        assert_eq!(result.defense, 5, "armor defense stacks");
    }

    // ── derive_stats: Dead zeros pools ───────────────────────────────────────

    /// When `Dead` is in the status list, `max_hp` and `max_mp` are zeroed.
    #[test]
    fn derive_stats_dead_zeros_pools() {
        let dead_status = StatusEffects {
            effects: vec![ActiveEffect {
                effect_type: StatusEffectType::Dead,
                ..Default::default()
            }],
        };
        let result = derive_stats(&BaseStats::ZERO, &[], &dead_status, 1);
        assert_eq!(result.max_hp, 0, "Dead zeros max_hp");
        assert_eq!(result.max_mp, 0, "Dead zeros max_mp");
        assert_eq!(result.current_hp, 0);
        assert_eq!(result.current_mp, 0);
    }

    // ── derive_stats: Poison does not modify stats at derive time ────────────

    /// Poison does not modify any stat at derive time — only ticks in #15.
    #[test]
    fn derive_stats_poison_does_not_modify_stats_at_derive_time() {
        let no_status = StatusEffects::default();
        let poison_status = StatusEffects {
            effects: vec![ActiveEffect {
                effect_type: StatusEffectType::Poison,
                ..Default::default()
            }],
        };
        let base = BaseStats {
            strength: 10,
            vitality: 10,
            ..BaseStats::ZERO
        };
        let without = derive_stats(&base, &[], &no_status, 1);
        let with_poison = derive_stats(&base, &[], &poison_status, 1);
        assert_eq!(
            without.max_hp, with_poison.max_hp,
            "Poison must not change max_hp at derive time"
        );
        assert_eq!(
            without.attack, with_poison.attack,
            "Poison must not change attack at derive time"
        );
    }

    // ── derive_stats: saturating arithmetic ─────────────────────────────────

    /// Overflow inputs produce saturated (clamped) results — no panic.
    #[test]
    fn derive_stats_saturating_arithmetic() {
        let maxed_base = BaseStats {
            strength: u16::MAX,
            vitality: u16::MAX,
            intelligence: u16::MAX,
            piety: u16::MAX,
            agility: u16::MAX,
            luck: u16::MAX,
        };
        let maxed_equip = ItemStatBlock {
            attack: u32::MAX,
            defense: u32::MAX,
            magic_attack: u32::MAX,
            magic_defense: u32::MAX,
            accuracy: u32::MAX,
            evasion: u32::MAX,
            hp_bonus: u32::MAX,
            mp_bonus: u32::MAX,
        };
        // Must not panic; all values saturate at u32::MAX.
        let result = derive_stats(
            &maxed_base,
            &[maxed_equip],
            &StatusEffects::default(),
            u32::MAX,
        );
        assert_eq!(result.attack, u32::MAX, "attack saturates at u32::MAX");
    }

    // ── PartySize default ────────────────────────────────────────────────────

    #[test]
    fn party_size_default_is_four() {
        assert_eq!(PartySize::default().0, 4);
    }

    // ── StatusEffects::has ───────────────────────────────────────────────────

    #[test]
    fn status_effects_has_returns_true_for_present_kind() {
        let status = StatusEffects {
            effects: vec![ActiveEffect {
                effect_type: StatusEffectType::Dead,
                ..Default::default()
            }],
        };
        assert!(status.has(StatusEffectType::Dead));
        assert!(!status.has(StatusEffectType::Poison));
    }

    // NOTE: `derive_stats_status_order_independent` is deferred to #15.
    // V1 status types are trivially order-independent because none of them
    // modify stats via the `magnitude` field (all are pure gates).
    // The research-line-583 test (which uses AttackUp) requires #15's buff
    // branches to exist before it can be authored.
}
