//! Party-side skill-tree components and pure helper functions.
//!
//! Plan: `project/plans/20260514-120000-feature-20-spells-skill-tree.md`
//! Feature #20 Phase 2 — Skill trees + SP allocation.
//!
//! ## Components
//!
//! - `KnownSpells` — set of spell IDs the character can cast.
//! - `UnlockedNodes` — set of skill-tree node IDs already purchased.
//!
//! ## Resources
//!
//! - `WarnedMissingSpells` — warn-once registry for missing `SpellId` references.
//!   Used by Phase 3's `SpellMenu` painter per Q9 decision.
//!
//! ## Pure functions
//!
//! - `can_unlock_node` — checks prerequisites, level, SP balance, and the
//!   defense-in-depth `unspent <= total_earned` invariant.
//! - `learn_spell_pure` — thin wrapper; testable without an `App`.
//! - `allocate_skill_point_pure` — deducts `node.cost` from
//!   `experience.unspent_skill_points`.

use std::collections::HashSet;

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::data::skills::{MAX_SKILL_NODE_COST, MAX_SKILL_TREE_NODES, NodeId, SkillNode};
use crate::data::spells::{KNOWN_SPELLS_MAX, SpellId};
use crate::plugins::party::character::Experience;

// ─────────────────────────────────────────────────────────────────────────────
// KnownSpells component
// ─────────────────────────────────────────────────────────────────────────────

/// The set of spell IDs this character knows and can cast.
///
/// Populated by `handle_guild_skills_unlock` when a `LearnSpell` node is
/// purchased. Consumed by the Phase 3 `SpellMenu` painter to build the
/// castable list.
#[derive(Component, Reflect, Serialize, Deserialize, Default, Debug, Clone, PartialEq, Eq)]
pub struct KnownSpells {
    pub spells: Vec<SpellId>,
}

impl KnownSpells {
    /// Returns `true` if this character already knows the given spell.
    pub fn knows(&self, id: &str) -> bool {
        self.spells.iter().any(|s| s == id)
    }

    /// Add the spell if not already known and the cap is not exceeded.
    pub fn learn(&mut self, id: SpellId) {
        if !self.knows(&id) && self.spells.len() < KNOWN_SPELLS_MAX {
            self.spells.push(id);
        }
    }

    /// Remove a spell by ID. Used in tests and future polish.
    pub fn forget(&mut self, id: &str) {
        self.spells.retain(|s| s != id);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// UnlockedNodes component
// ─────────────────────────────────────────────────────────────────────────────

/// The set of skill-tree node IDs this character has already unlocked.
#[derive(Component, Reflect, Serialize, Deserialize, Default, Debug, Clone, PartialEq, Eq)]
pub struct UnlockedNodes {
    pub nodes: Vec<NodeId>,
}

impl UnlockedNodes {
    /// Returns `true` if the node with the given ID has been unlocked.
    pub fn has(&self, id: &str) -> bool {
        self.nodes.iter().any(|n| n == id)
    }

    /// Unlock a node if not already present and the cap is not exceeded.
    pub fn unlock(&mut self, id: NodeId, max_nodes: usize) {
        if !self.has(&id) && self.nodes.len() < max_nodes {
            self.nodes.push(id);
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// WarnedMissingSpells resource
// ─────────────────────────────────────────────────────────────────────────────

/// Warn-once registry for `KnownSpells` entries that reference a `SpellId` not
/// present in `SpellDb`.
///
/// Phase 3's `SpellMenu` painter inserts `(spell_id.clone(), actor_entity)` on
/// first encounter; subsequent frames silently filter the missing spell. Per
/// Q9 decision: "warn-once-per-(spell,character)-then-filter".
///
/// Phase 2 only `init_resource`s this — the painter that consumes it ships in
/// Phase 3.
#[derive(Resource, Default, Debug)]
pub struct WarnedMissingSpells {
    pub set: HashSet<(SpellId, Entity)>,
}

// ─────────────────────────────────────────────────────────────────────────────
// SkillError enum
// ─────────────────────────────────────────────────────────────────────────────

/// Error variants returned by `can_unlock_node` and `allocate_skill_point_pure`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SkillError {
    /// Insufficient unspent skill points.
    Insufficient,
    /// Character level is below the node's `min_level`.
    BelowMinLevel,
    /// One or more prerequisite nodes are not yet unlocked.
    MissingPrerequisite,
    /// Node is already unlocked.
    AlreadyUnlocked,
    /// `UnlockedNodes` has reached the maximum capacity.
    CapReached,
}

// ─────────────────────────────────────────────────────────────────────────────
// Pure functions
// ─────────────────────────────────────────────────────────────────────────────

/// Check whether the given node can be unlocked for this character.
///
/// Checks (in order):
/// 1. Not already unlocked.
/// 2. Cap not reached (`UnlockedNodes.len() < MAX_SKILL_TREE_NODES`).
/// 3. `experience.level >= node.min_level`.
/// 4. All `node.prerequisites` are in `unlocked.nodes`.
/// 5. `experience.unspent_skill_points >= node.cost.min(MAX_SKILL_NODE_COST)`.
/// 6. Defense-in-depth: `experience.unspent_skill_points <= experience.total_skill_points_earned`.
pub fn can_unlock_node(
    node: &SkillNode,
    experience: &Experience,
    unlocked: &UnlockedNodes,
) -> Result<(), SkillError> {
    if unlocked.has(&node.id) {
        return Err(SkillError::AlreadyUnlocked);
    }
    if unlocked.nodes.len() >= MAX_SKILL_TREE_NODES {
        return Err(SkillError::CapReached);
    }
    if experience.level < node.min_level {
        return Err(SkillError::BelowMinLevel);
    }
    for prereq in &node.prerequisites {
        if !unlocked.has(prereq) {
            return Err(SkillError::MissingPrerequisite);
        }
    }
    let clamped_cost = node.cost.min(MAX_SKILL_NODE_COST);
    if experience.unspent_skill_points < clamped_cost {
        return Err(SkillError::Insufficient);
    }
    // Defense-in-depth: unspent can never exceed total earned.
    if experience.unspent_skill_points > experience.total_skill_points_earned {
        return Err(SkillError::Insufficient);
    }
    Ok(())
}

/// Learn a spell — pure-function wrapper for testability.
pub fn learn_spell_pure(known: &mut KnownSpells, id: SpellId) {
    known.learn(id);
}

/// Deduct `node.cost` skill points from `experience`.
///
/// Returns `Err(SkillError::Insufficient)` if there are not enough points.
/// The caller is expected to call `can_unlock_node` first; this function is a
/// final deduction step.
pub fn allocate_skill_point_pure(
    experience: &mut Experience,
    node: &SkillNode,
) -> Result<(), SkillError> {
    let clamped_cost = node.cost.min(MAX_SKILL_NODE_COST);
    if experience.unspent_skill_points < clamped_cost {
        return Err(SkillError::Insufficient);
    }
    experience.unspent_skill_points -= clamped_cost;
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::skills::NodeGrant;
    use crate::plugins::party::character::BaseStats;

    fn make_node(id: &str, cost: u32, min_level: u32, prereqs: Vec<&str>) -> SkillNode {
        SkillNode {
            id: id.into(),
            display_name: id.into(),
            cost,
            min_level,
            prerequisites: prereqs.into_iter().map(|s| s.to_string()).collect(),
            grant: NodeGrant::StatBoost(BaseStats::ZERO),
            description: String::new(),
        }
    }

    fn make_exp(level: u32, unspent: u32, total: u32) -> Experience {
        Experience {
            level,
            current_xp: 0,
            xp_to_next_level: 100,
            unspent_skill_points: unspent,
            total_skill_points_earned: total,
        }
    }

    /// `KnownSpells::learn` does not add a duplicate.
    #[test]
    fn known_spells_learn_skips_duplicates() {
        let mut known = KnownSpells::default();
        known.learn("halito".into());
        known.learn("halito".into());
        assert_eq!(known.spells.len(), 1);
    }

    /// `KnownSpells::learn` respects `KNOWN_SPELLS_MAX`.
    #[test]
    fn known_spells_learn_respects_max() {
        let mut known = KnownSpells::default();
        for i in 0..=KNOWN_SPELLS_MAX {
            known.learn(format!("spell_{i}"));
        }
        assert_eq!(known.spells.len(), KNOWN_SPELLS_MAX);
    }

    /// `can_unlock_node` rejects when `experience.level < node.min_level`.
    #[test]
    fn can_unlock_node_enforces_min_level() {
        let node = make_node("a", 1, 5, vec![]);
        let exp = make_exp(3, 1, 1); // level 3 < min_level 5
        let unlocked = UnlockedNodes::default();
        assert_eq!(
            can_unlock_node(&node, &exp, &unlocked),
            Err(SkillError::BelowMinLevel)
        );
    }

    /// `can_unlock_node` rejects when a prerequisite is not unlocked.
    #[test]
    fn can_unlock_node_enforces_prereqs() {
        let node = make_node("b", 1, 1, vec!["a"]);
        let exp = make_exp(5, 5, 5);
        let unlocked = UnlockedNodes::default(); // "a" not in unlocked
        assert_eq!(
            can_unlock_node(&node, &exp, &unlocked),
            Err(SkillError::MissingPrerequisite)
        );
    }

    /// `can_unlock_node` rejects when `unspent_skill_points > total_skill_points_earned`.
    #[test]
    fn can_unlock_node_enforces_skill_point_balance() {
        let node = make_node("a", 1, 1, vec![]);
        // unspent > total — crafted-save tamper scenario
        let exp = make_exp(1, 100, 1);
        let unlocked = UnlockedNodes::default();
        assert_eq!(
            can_unlock_node(&node, &exp, &unlocked),
            Err(SkillError::Insufficient)
        );
    }

    /// `can_unlock_node` rejects when the node is already unlocked.
    #[test]
    fn can_unlock_node_rejects_already_unlocked() {
        let node = make_node("a", 1, 1, vec![]);
        let exp = make_exp(1, 5, 5);
        let mut unlocked = UnlockedNodes::default();
        unlocked.unlock("a".into(), MAX_SKILL_TREE_NODES);
        assert_eq!(
            can_unlock_node(&node, &exp, &unlocked),
            Err(SkillError::AlreadyUnlocked)
        );
    }

    /// `allocate_skill_point_pure` deducts `node.cost` from `unspent`.
    #[test]
    fn allocate_skill_point_pure_deducts_correctly() {
        let node = make_node("a", 1, 1, vec![]);
        let mut exp = make_exp(1, 3, 3);
        allocate_skill_point_pure(&mut exp, &node).unwrap();
        assert_eq!(exp.unspent_skill_points, 2);
    }

    /// `allocate_skill_point_pure` returns `Err` when insufficient SP.
    #[test]
    fn allocate_skill_point_pure_rejects_insufficient() {
        let node = make_node("a", 3, 1, vec![]);
        let mut exp = make_exp(1, 1, 1); // only 1 SP but cost is 3
        assert_eq!(
            allocate_skill_point_pure(&mut exp, &node),
            Err(SkillError::Insufficient)
        );
    }
}
