//! Per-class skill trees — `SkillTree`, `SkillNode`, `NodeGrant`, DAG validation.
//!
//! Plan: `project/plans/20260514-120000-feature-20-spells-skill-tree.md`
//! Pattern 3 from research: flat node list with prerequisite IDs, Kahn's
//! topo-sort validation at asset load, MAX_* clamps at two trust boundaries.
//!
//! ## Architecture
//!
//! Each class has exactly one `SkillTree` authored as a RON asset at
//! `assets/skills/<class>.skills.ron`. The asset is loaded via
//! `RonAssetPlugin::<SkillTree>::new(&["skills.ron"])` in `LoadingPlugin`.
//!
//! `SkillTree.nodes` is a flat `Vec<SkillNode>`; prerequisite relationships are
//! expressed as `node.prerequisites: Vec<NodeId>`. `validate_no_cycles` runs
//! Kahn's algorithm over this DAG on `OnExit(GameState::Loading)`; a cycle
//! causes `error!` + tree emptied. `clamp_skill_tree` caps node count, cost,
//! and min_level against the MAX_* constants below.
//!
//! ## Feature #20 Phase 2

use std::collections::{HashMap, HashSet, VecDeque};

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::data::spells::SpellId;
use crate::plugins::party::character::StatusEffectType;

// ─────────────────────────────────────────────────────────────────────────────
// MAX_* constants
// ─────────────────────────────────────────────────────────────────────────────

/// Maximum nodes in one `SkillTree.nodes` vector. 64 nodes per class is plenty
/// for v1; future polish can raise this if balance demands.
pub const MAX_SKILL_TREE_NODES: usize = 64;

/// Maximum `SkillNode.cost` (skill points to unlock). Clamped at the
/// `can_unlock_node` use-site; defends against `cost: u32::MAX` (node would be
/// permanently unlockable).
pub const MAX_SKILL_NODE_COST: u32 = 99;

/// Maximum `SkillNode.min_level` (gating). Capped at the engine's level cap
/// (`progression::level_cap()` = 99); kept as a const here to avoid a
/// circular import.
pub const MAX_SKILL_NODE_MIN_LEVEL: u32 = 99;

/// Skill points awarded per level-up. Single source of truth for the
/// progression hook in `apply_level_up_threshold_system`. Per user decision Q6
/// (default = 1). Mirror-declared in `src/plugins/party/progression.rs`
/// via `pub use` to avoid a Phase 2 → Phase 3 forward dep.
pub use crate::plugins::party::progression::SKILL_POINTS_PER_LEVEL;

// ─────────────────────────────────────────────────────────────────────────────
// NodeId type alias
// ─────────────────────────────────────────────────────────────────────────────

/// Identifier string for a skill tree node — same shape as `SpellId`.
pub type NodeId = String;

// ─────────────────────────────────────────────────────────────────────────────
// NodeGrant enum
// ─────────────────────────────────────────────────────────────────────────────

/// What a skill node grants on unlock.
///
/// - `LearnSpell(SpellId)` — adds the spell to `KnownSpells`.
/// - `StatBoost(BaseStats)` — adds the given stat delta (saturating_add per field).
/// - `Resist(StatusEffectType)` — stores a resist marker in `UnlockedNodes`.
///   Day-one: the data shape is correct; the consumer-side resist-check is deferred
///   to a future PR. See guild_skills.rs top-of-file comment.
#[derive(Reflect, Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum NodeGrant {
    LearnSpell(SpellId),
    StatBoost(crate::plugins::party::character::BaseStats),
    Resist(StatusEffectType),
}

impl Default for NodeGrant {
    fn default() -> Self {
        Self::LearnSpell(String::new())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// SkillNode struct
// ─────────────────────────────────────────────────────────────────────────────

/// One node in a class skill tree.
#[derive(Reflect, Serialize, Deserialize, Default, Debug, Clone, PartialEq)]
pub struct SkillNode {
    /// Unique identifier within this tree (e.g., `"fighter_might"`).
    pub id: NodeId,
    /// Human-readable name shown in the Guild Skills UI.
    pub display_name: String,
    /// Skill points required to unlock.
    pub cost: u32,
    /// Minimum character level required.
    #[serde(default)]
    pub min_level: u32,
    /// IDs of nodes that must be unlocked before this one can be purchased.
    #[serde(default)]
    pub prerequisites: Vec<NodeId>,
    /// What this node grants on unlock.
    pub grant: NodeGrant,
    /// Flavour text shown in the Guild Skills UI.
    #[serde(default)]
    pub description: String,
}

// ─────────────────────────────────────────────────────────────────────────────
// SkillTree struct
// ─────────────────────────────────────────────────────────────────────────────

/// Per-class skill tree loaded from `assets/skills/<class>.skills.ron`.
#[derive(Asset, Reflect, Serialize, Deserialize, Default, Debug, Clone, PartialEq)]
pub struct SkillTree {
    /// Class identifier string (e.g., `"Fighter"`).
    pub class_id: String,
    /// Flat list of skill nodes.
    pub nodes: Vec<SkillNode>,
}

impl SkillTree {
    /// Find a node by its ID, O(n).
    pub fn get(&self, id: &str) -> Option<&SkillNode> {
        self.nodes.iter().find(|n| n.id == id)
    }

    /// Iterate root nodes (nodes with no prerequisites).
    pub fn root_nodes(&self) -> impl Iterator<Item = &SkillNode> {
        self.nodes.iter().filter(|n| n.prerequisites.is_empty())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// CycleError enum
// ─────────────────────────────────────────────────────────────────────────────

/// Error returned by `validate_no_cycles` when the skill tree DAG is invalid.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CycleError {
    /// The DAG contains a cycle (topo-sort did not visit all nodes).
    CycleDetected { involved: Vec<NodeId> },
    /// A prerequisite references a node ID not present in the tree.
    UnknownPrerequisite { node: NodeId, prereq: NodeId },
}

// ─────────────────────────────────────────────────────────────────────────────
// validate_no_cycles — Kahn's algorithm topo-sort
// ─────────────────────────────────────────────────────────────────────────────

/// Validate that the skill tree's prerequisite graph is a DAG (no cycles).
///
/// Uses Kahn's algorithm (BFS topo-sort). Returns `Ok(())` if the tree is
/// cycle-free and all prerequisite IDs resolve to real nodes. Returns
/// `Err(CycleError::UnknownPrerequisite { ... })` if a prerequisite ID is not
/// in the tree. Returns `Err(CycleError::CycleDetected { ... })` if a cycle
/// exists.
///
/// **Scope:** structural correctness only (cycles + unknown prereqs). Does NOT
/// walk `NodeGrant::LearnSpell(SpellId)` against `SpellDb` (Cat-C-3 decision).
pub fn validate_no_cycles(tree: &SkillTree) -> Result<(), CycleError> {
    // Build the set of known node IDs.
    let known: HashSet<&str> = tree.nodes.iter().map(|n| n.id.as_str()).collect();

    // Build in-degree map.
    let mut in_degree: HashMap<&str, usize> = HashMap::new();
    for n in &tree.nodes {
        in_degree.insert(n.id.as_str(), n.prerequisites.len());
    }

    // Validate that all prerequisites reference real nodes.
    for n in &tree.nodes {
        for p in &n.prerequisites {
            if !known.contains(p.as_str()) {
                return Err(CycleError::UnknownPrerequisite {
                    node: n.id.clone(),
                    prereq: p.clone(),
                });
            }
        }
    }

    // Kahn's: seed the queue with zero-in-degree nodes.
    let mut queue: VecDeque<&str> = in_degree
        .iter()
        .filter_map(|(k, v)| if *v == 0 { Some(*k) } else { None })
        .collect();

    let mut visited = 0usize;

    while let Some(n_id) = queue.pop_front() {
        visited += 1;
        // For every node that has `n_id` as a prerequisite, decrement its in-degree.
        for other in &tree.nodes {
            if other.prerequisites.iter().any(|p| p.as_str() == n_id) {
                let d = in_degree.get_mut(other.id.as_str()).unwrap();
                *d -= 1;
                if *d == 0 {
                    queue.push_back(other.id.as_str());
                }
            }
        }
    }

    if visited != tree.nodes.len() {
        // Some nodes were never visited — they form a cycle.
        let involved = tree
            .nodes
            .iter()
            .filter(|n| in_degree.get(n.id.as_str()).copied().unwrap_or(0) > 0)
            .map(|n| n.id.clone())
            .collect();
        return Err(CycleError::CycleDetected { involved });
    }

    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// clamp_skill_tree
// ─────────────────────────────────────────────────────────────────────────────

/// Apply MAX_* clamps to a loaded `SkillTree` (defense-in-depth at asset load).
///
/// - Truncates `nodes` to `MAX_SKILL_TREE_NODES`.
/// - Clamps each `node.cost` to `MAX_SKILL_NODE_COST`.
/// - Clamps each `node.min_level` to `MAX_SKILL_NODE_MIN_LEVEL`.
pub fn clamp_skill_tree(tree: &mut SkillTree) {
    tree.nodes.truncate(MAX_SKILL_TREE_NODES);
    for node in &mut tree.nodes {
        node.cost = node.cost.min(MAX_SKILL_NODE_COST);
        node.min_level = node.min_level.min(MAX_SKILL_NODE_MIN_LEVEL);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugins::party::character::BaseStats;

    fn make_linear_tree() -> SkillTree {
        SkillTree {
            class_id: "Fighter".into(),
            nodes: vec![
                SkillNode {
                    id: "a".into(),
                    display_name: "A".into(),
                    cost: 1,
                    min_level: 1,
                    prerequisites: vec![],
                    grant: NodeGrant::StatBoost(BaseStats {
                        strength: 1,
                        ..BaseStats::ZERO
                    }),
                    description: String::new(),
                },
                SkillNode {
                    id: "b".into(),
                    display_name: "B".into(),
                    cost: 1,
                    min_level: 1,
                    prerequisites: vec!["a".into()],
                    grant: NodeGrant::StatBoost(BaseStats {
                        strength: 1,
                        ..BaseStats::ZERO
                    }),
                    description: String::new(),
                },
                SkillNode {
                    id: "c".into(),
                    display_name: "C".into(),
                    cost: 1,
                    min_level: 1,
                    prerequisites: vec!["b".into()],
                    grant: NodeGrant::StatBoost(BaseStats {
                        strength: 1,
                        ..BaseStats::ZERO
                    }),
                    description: String::new(),
                },
            ],
        }
    }

    /// A linear DAG (a → b → c) must pass cycle detection.
    #[test]
    fn validate_no_cycles_accepts_linear() {
        let tree = make_linear_tree();
        assert!(validate_no_cycles(&tree).is_ok());
    }

    /// A node that lists itself as a prerequisite is a self-loop (cycle).
    #[test]
    fn validate_no_cycles_rejects_self_loop() {
        let mut tree = make_linear_tree();
        tree.nodes[0].prerequisites.push("a".into()); // a requires a
        assert!(matches!(
            validate_no_cycles(&tree),
            Err(CycleError::CycleDetected { .. })
        ));
    }

    /// Two nodes each listing the other as a prerequisite form a two-node cycle.
    #[test]
    fn validate_no_cycles_rejects_two_node_cycle() {
        let tree = SkillTree {
            class_id: "Test".into(),
            nodes: vec![
                SkillNode {
                    id: "x".into(),
                    prerequisites: vec!["y".into()],
                    ..Default::default()
                },
                SkillNode {
                    id: "y".into(),
                    prerequisites: vec!["x".into()],
                    ..Default::default()
                },
            ],
        };
        assert!(matches!(
            validate_no_cycles(&tree),
            Err(CycleError::CycleDetected { .. })
        ));
    }

    /// Three-node cycle: a → b → c → a.
    #[test]
    fn validate_no_cycles_rejects_three_node_cycle() {
        let tree = SkillTree {
            class_id: "Test".into(),
            nodes: vec![
                SkillNode {
                    id: "a".into(),
                    prerequisites: vec!["c".into()],
                    ..Default::default()
                },
                SkillNode {
                    id: "b".into(),
                    prerequisites: vec!["a".into()],
                    ..Default::default()
                },
                SkillNode {
                    id: "c".into(),
                    prerequisites: vec!["b".into()],
                    ..Default::default()
                },
            ],
        };
        assert!(matches!(
            validate_no_cycles(&tree),
            Err(CycleError::CycleDetected { .. })
        ));
    }

    /// Prerequisite referencing a non-existent node ID triggers `UnknownPrerequisite`.
    #[test]
    fn validate_no_cycles_rejects_unknown_prereq() {
        let tree = SkillTree {
            class_id: "Test".into(),
            nodes: vec![SkillNode {
                id: "a".into(),
                prerequisites: vec!["nonexistent".into()],
                ..Default::default()
            }],
        };
        assert!(matches!(
            validate_no_cycles(&tree),
            Err(CycleError::UnknownPrerequisite { .. })
        ));
    }

    /// `clamp_skill_tree` truncates a tree with more nodes than `MAX_SKILL_TREE_NODES`.
    #[test]
    fn clamp_skill_tree_truncates_oversized() {
        let mut tree = SkillTree {
            class_id: "Test".into(),
            nodes: (0..=MAX_SKILL_TREE_NODES)
                .map(|i| SkillNode {
                    id: format!("n{i}"),
                    ..Default::default()
                })
                .collect(),
        };
        assert!(tree.nodes.len() > MAX_SKILL_TREE_NODES);
        clamp_skill_tree(&mut tree);
        assert_eq!(tree.nodes.len(), MAX_SKILL_TREE_NODES);
    }

    /// `clamp_skill_tree` caps per-node cost to `MAX_SKILL_NODE_COST`.
    #[test]
    fn clamp_skill_tree_caps_per_node_cost() {
        let mut tree = SkillTree {
            class_id: "Test".into(),
            nodes: vec![SkillNode {
                id: "a".into(),
                cost: u32::MAX,
                ..Default::default()
            }],
        };
        clamp_skill_tree(&mut tree);
        assert_eq!(tree.nodes[0].cost, MAX_SKILL_NODE_COST);
    }

    /// A `SkillTree` round-trips through RON serialization.
    #[test]
    fn skill_tree_round_trips_through_ron() {
        let tree = SkillTree {
            class_id: "Fighter".into(),
            nodes: vec![SkillNode {
                id: "fighter_might".into(),
                display_name: "Might".into(),
                cost: 1,
                min_level: 1,
                prerequisites: vec![],
                grant: NodeGrant::StatBoost(BaseStats {
                    strength: 2,
                    ..BaseStats::ZERO
                }),
                description: "+2 STR.".into(),
            }],
        };

        let serialized = ron::to_string(&tree).expect("RON serialization failed");
        let deserialized: SkillTree =
            ron::from_str(&serialized).expect("RON deserialization failed");
        assert_eq!(tree, deserialized);
    }
}
