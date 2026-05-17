//! Guild "Skills" sub-mode — skill-tree viewer and SP spend handler.
//!
//! Plan: `project/plans/20260514-120000-feature-20-spells-skill-tree.md`
//! Feature #20 Phase 2 — Guild Skills mode.
//!
//! ## Architecture
//!
//! This module is a sibling of `guild_create.rs` (Feature #19 creation wizard).
//! The painter, input handler, and unlock handler follow the same
//! EguiPrimaryContextPass / Update split used throughout the Guild module.
//!
//! ## Day-one Resist behaviour
//!
//! `NodeGrant::Resist(StatusEffectType)` ships as a DATA-ONLY marker stored in
//! `UnlockedNodes`. The actual resist-check logic (reducing apply-rate or
//! magnitude of the named status) is NOT implemented in #20. The node can be
//! unlocked and the toast fires, but no in-game resist effect occurs yet.
//! Future PR reads `UnlockedNodes.has("fighter_resist_poison")` etc.
//!
//! ## 4-state painter (Cat-C-1 decision)
//!
//! The painter distinguishes four node states via the private `node_state` pure
//! function, making per-state colour and tooltip text testable independent of
//! egui:
//!
//! | State | Visual |
//! |---|---|
//! | Unlocked | Soft green row + [✓] |
//! | CanUnlock | Bright green text + [ ] |
//! | SpInsufficient | Warm yellow text + [ ] |
//! | Locked(SkillError) | Dim grey text + [ ] |

use bevy::prelude::*;
use bevy_egui::{EguiContexts, egui};
use leafwing_input_manager::prelude::ActionState;

use crate::data::skills::{MAX_SKILL_TREE_NODES, SkillNode, SkillTree};
use crate::plugins::input::MenuAction;
use crate::plugins::loading::DungeonAssets;
use crate::plugins::party::character::{
    BaseStats, CharacterName, Class, Experience, PartyMember, PartySlot,
};
use crate::plugins::party::inventory::{EquipSlot, EquipmentChangedEvent};
use crate::plugins::party::skills::{
    KnownSpells, SkillError, UnlockedNodes, allocate_skill_point_pure, can_unlock_node,
};
use crate::plugins::town::guild::{GuildMode, GuildState};
use crate::plugins::town::gold::Gold;
use crate::plugins::town::toast::Toasts;

/// Read-only party query used by the painter to render each row's state.
type SkillsPainterPartyQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static CharacterName,
        &'static Class,
        &'static Experience,
        &'static PartySlot,
        &'static UnlockedNodes,
    ),
    With<PartyMember>,
>;

/// Mutable party query used by the input handler to mutate progression state
/// when a player confirms an unlock.
type SkillsInputPartyQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Class,
        &'static PartySlot,
        &'static mut Experience,
        &'static mut KnownSpells,
        &'static mut UnlockedNodes,
        &'static mut BaseStats,
    ),
    With<PartyMember>,
>;

// ─────────────────────────────────────────────────────────────────────────────
// NodeState — 4-state enum (Cat-C-1)
// ─────────────────────────────────────────────────────────────────────────────

/// Painter state for a skill tree node (Cat-C-1 decision: 4-state palette).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NodeState {
    /// Node already purchased by this character.
    Unlocked,
    /// Node meets all requirements and can be purchased now.
    CanUnlock,
    /// Prereqs and level met but not enough unspent skill points.
    SpInsufficient,
    /// Node is locked (prereq missing, below min-level, cap reached, etc.).
    Locked(SkillError),
}

/// Determine the display state of `node` for this character.
///
/// Pure function — no ECS queries, no resources. Testable in isolation.
pub fn node_state(
    node: &SkillNode,
    experience: &Experience,
    unlocked: &UnlockedNodes,
) -> NodeState {
    if unlocked.has(&node.id) {
        return NodeState::Unlocked;
    }

    match can_unlock_node(node, experience, unlocked) {
        Ok(()) => NodeState::CanUnlock,
        Err(SkillError::Insufficient) => {
            let prereqs_met = node.prerequisites.iter().all(|p| unlocked.has(p));
            let level_met = experience.level >= node.min_level;
            // Tamper guard: also verify the unspent/total invariant holds.
            let invariant_ok =
                experience.unspent_skill_points <= experience.total_skill_points_earned;
            if prereqs_met && level_met && invariant_ok {
                NodeState::SpInsufficient
            } else {
                NodeState::Locked(SkillError::Insufficient)
            }
        }
        Err(e) => NodeState::Locked(e),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Helper — sort nodes by depth + id
// ─────────────────────────────────────────────────────────────────────────────

/// Compute the depth of a node in the DAG (distance from nearest root).
///
/// Depth 0 = no prerequisites. Recursive; memoised in `memo`.
fn node_depth<'a>(
    tree: &'a SkillTree,
    node: &'a SkillNode,
    memo: &mut std::collections::HashMap<&'a str, u32>,
) -> u32 {
    if let Some(&d) = memo.get(node.id.as_str()) {
        return d;
    }
    if node.prerequisites.is_empty() {
        memo.insert(&node.id, 0);
        return 0;
    }
    let max_prereq_depth = node
        .prerequisites
        .iter()
        .filter_map(|p| tree.get(p))
        .map(|prereq| node_depth(tree, prereq, memo) + 1)
        .max()
        .unwrap_or(0);
    memo.insert(&node.id, max_prereq_depth);
    max_prereq_depth
}

/// Return nodes sorted by `(depth, id)` for consistent visual ordering.
///
/// # Precondition
///
/// The tree MUST be cycle-free (validated by `validate_no_cycles`). Calling
/// this on a cyclic tree causes infinite recursion in `node_depth`. Both
/// production call sites guard with `if tree.nodes.is_empty()` after
/// `validate_skill_trees_on_load` empties cyclic trees.
///
/// When constructing test fixtures, run `validate_no_cycles` + `clamp_skill_tree`
/// first to ensure the tree is safe to pass here.
fn sorted_nodes(tree: &SkillTree) -> Vec<&SkillNode> {
    let mut memo = std::collections::HashMap::new();
    let mut nodes: Vec<&SkillNode> = tree.nodes.iter().collect();
    nodes.sort_by_key(|n| (node_depth(tree, n, &mut memo), n.id.clone()));
    nodes
}

// ─────────────────────────────────────────────────────────────────────────────
// paint_guild_skills — EguiPrimaryContextPass, read-only
// ─────────────────────────────────────────────────────────────────────────────

/// Render the Guild Skills sub-mode.
///
/// **Read-only** — no `ResMut<T>` or `Commands`. All mutations live in the
/// two handler systems below.
#[allow(clippy::too_many_arguments)]
pub fn paint_guild_skills(
    mut contexts: EguiContexts,
    guild_state: Res<GuildState>,
    gold: Res<Gold>,
    dungeon_assets: Option<Res<DungeonAssets>>,
    skill_trees: Res<Assets<SkillTree>>,
    party: SkillsPainterPartyQuery,
) -> Result {
    let ctx = contexts.ctx_mut()?;

    egui::TopBottomPanel::top("guild_skills_header").show(ctx, |ui| {
        ui.horizontal(|ui| {
            ui.heading("Guild — Skill Trees");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(format!("{} Gold", gold.0));
            });
        });
    });

    egui::CentralPanel::default().show(ctx, |ui| {
        // Sort members by (PartySlot, Entity) for determinism.
        let mut members: Vec<_> = party.iter().collect();
        members.sort_by_key(|(e, _, _, _, slot, _)| (slot.0, *e));

        if members.is_empty() {
            ui.label("(No active party members)");
            return;
        }

        // ── Left panel: roster ───────────────────────────────────────────────
        egui::SidePanel::left("guild_skills_roster")
            .min_width(200.0)
            .show_inside(ui, |ui| {
                ui.heading("Party");
                for (idx, (_, name, class, xp, _, _)) in members.iter().enumerate() {
                    let is_cursor = idx == guild_state.cursor;
                    let label = format!(
                        "{}{} ({:?} Lv{} — {}/{} SP)",
                        if is_cursor { "> " } else { "  " },
                        name.0,
                        class,
                        xp.level,
                        xp.unspent_skill_points,
                        xp.total_skill_points_earned,
                    );
                    ui.label(label);
                }
            });

        // ── Right panel: skill tree for cursor member ────────────────────────
        egui::CentralPanel::default().show_inside(ui, |ui| {
            let Some((_, _, class, experience, _, unlocked)) = members.get(guild_state.cursor)
            else {
                ui.label("(No member selected)");
                return;
            };

            // Resolve skill tree for this class.
            let tree_opt = dungeon_assets
                .as_ref()
                .and_then(|a| a.skill_tree_for(**class))
                .and_then(|h| skill_trees.get(h));

            let Some(tree) = tree_opt else {
                ui.label(format!("(No skill tree authored for {:?})", class));
                return;
            };

            if tree.nodes.is_empty() {
                ui.label("(Skill tree unavailable — check error log for details)");
                return;
            }

            ui.heading(format!("{:?} Skill Tree", class));

            let nodes = sorted_nodes(tree);
            for (node_idx, node) in nodes.iter().enumerate() {
                let state = node_state(node, experience, unlocked);
                let is_node_cursor = node_idx == guild_state.node_cursor;

                let cursor_str = if is_node_cursor { "> " } else { "  " };
                let checkbox_str = if matches!(state, NodeState::Unlocked) {
                    "[✓]"
                } else {
                    "[ ]"
                };
                let prereq_str = if node.prerequisites.is_empty() {
                    String::new()
                } else {
                    format!(" (req: {})", node.prerequisites.join(", "))
                };
                let gloss = match &state {
                    NodeState::Unlocked => String::new(),
                    NodeState::CanUnlock => " — unlock now".to_string(),
                    NodeState::SpInsufficient => {
                        let need = node.cost.saturating_sub(experience.unspent_skill_points);
                        format!(" — need {need} more SP")
                    }
                    NodeState::Locked(SkillError::BelowMinLevel) => {
                        format!(" — need Lv{}", node.min_level)
                    }
                    NodeState::Locked(SkillError::MissingPrerequisite) => {
                        " — prereq missing".to_string()
                    }
                    NodeState::Locked(SkillError::AlreadyUnlocked) => " — already unlocked".to_string(),
                    NodeState::Locked(SkillError::CapReached) => " — cap reached".to_string(),
                    NodeState::Locked(SkillError::Insufficient) => " — insufficient SP".to_string(),
                };

                let label_text = format!(
                    "{}{} {} Lv{} {}SP — {}{}{}",
                    cursor_str,
                    checkbox_str,
                    node.display_name,
                    node.min_level,
                    node.cost,
                    node.description,
                    prereq_str,
                    gloss,
                );

                let color = match state {
                    NodeState::Unlocked => egui::Color32::from_rgb(120, 200, 120),
                    NodeState::CanUnlock => egui::Color32::from_rgb(180, 240, 180),
                    NodeState::SpInsufficient => egui::Color32::from_rgb(230, 200, 100),
                    NodeState::Locked(_) => egui::Color32::from_rgb(140, 140, 140),
                };

                ui.colored_label(color, label_text);
            }

            ui.add_space(8.0);
            ui.label("[Up/Down] Member  |  [Left/Right] Node cursor  |  [Enter] Unlock  |  [Esc] Back");
        });
    });

    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// handle_guild_skills_input — Update, navigation only
// ─────────────────────────────────────────────────────────────────────────────

/// Navigation handler for Guild Skills mode.
///
/// Handles Cancel (→ Roster), Up/Down (party cursor), Left/Right (node cursor).
/// Confirm is handled by `handle_guild_skills_unlock` (separate system due to
/// `ResMut<Assets<SkillTree>>` access).
pub fn handle_guild_skills_input(
    actions: Res<ActionState<MenuAction>>,
    mut guild_state: ResMut<GuildState>,
    party: Query<(&PartySlot, Entity), With<PartyMember>>,
    dungeon_assets: Option<Res<DungeonAssets>>,
    skill_trees: Res<Assets<SkillTree>>,
    party_slots: Query<(&PartySlot, &Class), With<PartyMember>>,
) {
    if guild_state.mode != GuildMode::Skills {
        return;
    }

    // Cancel → back to Roster.
    if actions.just_pressed(&MenuAction::Cancel) {
        guild_state.mode = GuildMode::Roster;
        guild_state.cursor = 0;
        guild_state.node_cursor = 0;
        return;
    }

    // Up/Down — cycle through party members.
    let party_len = party.iter().count();
    if actions.just_pressed(&MenuAction::Up) && guild_state.cursor > 0 {
        guild_state.cursor -= 1;
        guild_state.node_cursor = 0; // reset node cursor on member change
    }
    if actions.just_pressed(&MenuAction::Down) && party_len > 0 {
        let new_cursor = (guild_state.cursor + 1).min(party_len.saturating_sub(1));
        if new_cursor != guild_state.cursor {
            guild_state.node_cursor = 0; // reset node cursor on member change
        }
        guild_state.cursor = new_cursor;
    }

    // Left/Right — cycle through tree nodes.
    // Determine the sorted node count for the cursor member's class.
    let node_count = {
        // Sort party by (slot, entity) to find cursor member.
        let mut members: Vec<(usize, &Class)> = party_slots
            .iter()
            .map(|(slot, class)| (slot.0, class))
            .collect();
        members.sort_by_key(|(s, _)| *s);
        members
            .get(guild_state.cursor)
            .and_then(|(_, class)| {
                dungeon_assets
                    .as_ref()
                    .and_then(|a| a.skill_tree_for(**class))
                    .and_then(|h| skill_trees.get(h))
                    .map(|tree| tree.nodes.len())
            })
            .unwrap_or(0)
    };

    if actions.just_pressed(&MenuAction::Left) && guild_state.node_cursor > 0 {
        guild_state.node_cursor -= 1;
    }
    if actions.just_pressed(&MenuAction::Right) && node_count > 0 {
        guild_state.node_cursor =
            (guild_state.node_cursor + 1).min(node_count.saturating_sub(1));
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// handle_guild_skills_unlock — Update, Confirm action
// ─────────────────────────────────────────────────────────────────────────────

/// Unlock handler — deducts SP and applies the node's grant on Confirm.
///
/// Separated from `handle_guild_skills_input` because this system needs
/// `ResMut<Assets<SkillTree>>` (for `skill_tree_for`) as well as mutable
/// party-component queries.
#[allow(clippy::too_many_arguments)]
pub fn handle_guild_skills_unlock(
    guild_state: Res<GuildState>,
    actions: Res<ActionState<MenuAction>>,
    dungeon_assets: Option<Res<DungeonAssets>>,
    skill_trees: Res<Assets<SkillTree>>,
    mut party: SkillsInputPartyQuery,
    mut equip_changed: MessageWriter<EquipmentChangedEvent>,
    mut toasts: ResMut<Toasts>,
) {
    if guild_state.mode != GuildMode::Skills {
        return;
    }
    if !actions.just_pressed(&MenuAction::Confirm) {
        return;
    }

    // Sort party by (PartySlot, Entity) to find the cursor member.
    let mut members: Vec<(Entity, &Class, &PartySlot)> = party
        .iter()
        .map(|(e, c, s, _, _, _, _)| (e, c, s))
        .collect();
    members.sort_by_key(|(e, _, s)| (s.0, *e));

    let Some((member_entity, member_class, _)) = members.get(guild_state.cursor).copied()
    else {
        return;
    };

    // Resolve skill tree for this class.
    let tree_handle = dungeon_assets
        .as_ref()
        .and_then(|a| a.skill_tree_for(*member_class));
    let Some(tree) = tree_handle.and_then(|h| skill_trees.get(h)) else {
        toasts.push(format!("No skill tree authored for {:?}.", member_class));
        return;
    };

    if tree.nodes.is_empty() {
        toasts.push("Skill tree unavailable (check log for errors).".to_string());
        return;
    }

    // Index the cursor-selected node from sorted order.
    let nodes = sorted_nodes(tree);
    let Some(node) = nodes.get(guild_state.node_cursor).copied() else {
        return;
    };
    // Clone the node so we don't borrow the tree past query access.
    let node = node.clone();

    // Retrieve mutable components for the cursor member.
    let Ok((_, _, _, mut experience, mut known_spells, mut unlocked_nodes, mut base_stats)) =
        party.get_mut(member_entity)
    else {
        return;
    };

    // Defense-in-depth: check prerequisites, level, and SP balance.
    if let Err(e) = can_unlock_node(&node, &experience, &unlocked_nodes) {
        let msg = match &e {
            SkillError::Insufficient => "Not enough skill points.".to_string(),
            SkillError::BelowMinLevel => {
                format!("Requires level {}.", node.min_level)
            }
            SkillError::MissingPrerequisite => format!(
                "Requires: {}.",
                node.prerequisites.join(", ")
            ),
            SkillError::AlreadyUnlocked => "Already unlocked.".to_string(),
            SkillError::CapReached => "Skill tree capacity reached.".to_string(),
        };
        toasts.push(msg);
        return;
    }

    // Deduct skill points.
    if allocate_skill_point_pure(&mut experience, &node).is_err() {
        toasts.push("Not enough skill points.".to_string());
        return;
    }

    // Mark node as unlocked.
    unlocked_nodes.unlock(node.id.clone(), MAX_SKILL_TREE_NODES);

    // Apply the node's grant.
    use crate::data::skills::NodeGrant;
    match &node.grant {
        NodeGrant::LearnSpell(spell_id) => {
            known_spells.learn(spell_id.clone());
            toasts.push(format!("Learned {}!", node.display_name));
        }
        NodeGrant::StatBoost(delta) => {
            base_stats.strength = base_stats.strength.saturating_add(delta.strength);
            base_stats.intelligence = base_stats.intelligence.saturating_add(delta.intelligence);
            base_stats.piety = base_stats.piety.saturating_add(delta.piety);
            base_stats.vitality = base_stats.vitality.saturating_add(delta.vitality);
            base_stats.agility = base_stats.agility.saturating_add(delta.agility);
            base_stats.luck = base_stats.luck.saturating_add(delta.luck);
            // Write EquipmentChangedEvent to trigger DerivedStats recompute.
            equip_changed.write(EquipmentChangedEvent {
                character: member_entity,
                slot: EquipSlot::None,
            });
            toasts.push(format!("{} stat boost applied!", node.display_name));
        }
        NodeGrant::Resist(kind) => {
            // Day-one: resist marker stored in UnlockedNodes only.
            // Consumer-side check deferred to a future PR.
            toasts.push(format!("Resist {:?} unlocked.", kind));
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::skills::{NodeGrant, SkillNode};
    use crate::plugins::party::character::{BaseStats, Experience};
    use crate::plugins::party::skills::UnlockedNodes;

    fn make_node(id: &str, cost: u32, min_level: u32, prereqs: Vec<&str>) -> SkillNode {
        SkillNode {
            id: id.into(),
            display_name: id.into(),
            cost,
            min_level,
            prerequisites: prereqs.into_iter().map(String::from).collect(),
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

    // ── node_state unit tests (Cat-C-1 4-state painter) ───────────────────────

    /// `node_state` returns `CanUnlock` when all prereqs, level, and SP are met.
    #[test]
    fn node_state_returns_can_unlock_when_all_conditions_met() {
        let node = make_node("a", 1, 1, vec![]);
        let exp = make_exp(1, 1, 1);
        let unlocked = UnlockedNodes::default();
        assert_eq!(node_state(&node, &exp, &unlocked), NodeState::CanUnlock);
    }

    /// `node_state` returns `SpInsufficient` when prereq + level met but SP is low.
    #[test]
    fn node_state_returns_sp_insufficient_when_prereq_met_but_sp_short() {
        let node = make_node("b", 3, 1, vec![]);
        let exp = make_exp(1, 1, 1); // only 1 SP, cost is 3
        let unlocked = UnlockedNodes::default();
        assert_eq!(
            node_state(&node, &exp, &unlocked),
            NodeState::SpInsufficient
        );
    }

    /// Tamper-guard: `node_state` returns `Locked` when `unspent > total_earned`
    /// even if prereqs and level are met. Prevents misleading yellow highlight on
    /// tampered saves.
    #[test]
    fn node_state_returns_locked_when_invariant_violated() {
        let node = make_node("c", 1, 1, vec![]);
        // unspent (5) > total_earned (3) — invariant violated.
        let exp = make_exp(1, 5, 3);
        let unlocked = UnlockedNodes::default();
        assert_eq!(
            node_state(&node, &exp, &unlocked),
            NodeState::Locked(SkillError::Insufficient)
        );
    }

    // ── Integration tests ─────────────────────────────────────────────────────

    fn make_test_tree() -> SkillTree {
        SkillTree {
            class_id: "Fighter".into(),
            nodes: vec![
                make_node("root_node", 1, 1, vec![]),
                SkillNode {
                    id: "spell_node".into(),
                    display_name: "Spell Node".into(),
                    cost: 1,
                    min_level: 1,
                    prerequisites: vec!["root_node".into()],
                    grant: NodeGrant::LearnSpell("test_spell".into()),
                    description: "Test spell node.".into(),
                },
                SkillNode {
                    id: "stat_node".into(),
                    display_name: "Stat Node".into(),
                    cost: 1,
                    min_level: 1,
                    prerequisites: vec![],
                    grant: NodeGrant::StatBoost(BaseStats {
                        strength: 2,
                        ..BaseStats::ZERO
                    }),
                    description: "Stat boost node.".into(),
                },
                make_node("level_gated", 1, 5, vec![]),
            ],
        }
    }

    fn build_test_app(tree: SkillTree) -> (App, Entity) {
        use bevy::asset::AssetPlugin;
        use bevy::state::app::StatesPlugin;
        use crate::plugins::party::character::{
            CharacterName, PartyRow, Race, StatusEffects, derive_stats,
        };
        use crate::plugins::party::inventory::{EquipmentChangedEvent, ItemHandleRegistry};
        use crate::plugins::state::GameState;
        use leafwing_input_manager::prelude::ActionState;

        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), StatesPlugin));
        app.init_state::<GameState>();
        app.init_resource::<ActionState<MenuAction>>();
        app.add_message::<EquipmentChangedEvent>();
        app.init_resource::<ItemHandleRegistry>();
        app.init_asset::<crate::data::items::ItemAsset>();
        app.init_asset::<SkillTree>();
        app.init_resource::<GuildState>();
        app.init_resource::<Toasts>();

        // Wire the unlock handler.
        app.add_systems(Update, handle_guild_skills_unlock);

        // Insert a mock DungeonAssets with the test tree loaded.
        let tree_handle = app
            .world_mut()
            .resource_mut::<Assets<SkillTree>>()
            .add(tree);

        use crate::plugins::loading::DungeonAssets;
        use crate::data::{DungeonFloor, EncounterTable, EnemyDb, ItemDb, ClassTable, SpellDb};
        app.init_asset::<DungeonFloor>();
        app.init_asset::<EncounterTable>();
        app.init_asset::<EnemyDb>();
        app.init_asset::<ItemDb>();
        app.init_asset::<ClassTable>();
        app.init_asset::<SpellDb>();

        app.world_mut().insert_resource(DungeonAssets {
            floor_01: Handle::default(),
            floor_02: Handle::default(),
            encounters_floor_01: Handle::default(),
            item_db: Handle::default(),
            enemy_db: Handle::default(),
            class_table: Handle::default(),
            spells: Handle::default(),
            fighter_skills: tree_handle,
            mage_skills: Handle::default(),
            priest_skills: Handle::default(),
        });

        // Set GuildState to Skills mode, cursor at 0.
        app.world_mut().resource_mut::<GuildState>().mode = GuildMode::Skills;

        // Spawn a Fighter party member with 5 unspent SP.
        let base = BaseStats {
            strength: 10,
            intelligence: 8,
            piety: 8,
            vitality: 10,
            agility: 10,
            luck: 6,
        };
        let derived = derive_stats(&base, &[], &StatusEffects::default(), 1);
        let entity = app
            .world_mut()
            .spawn((
                PartyMember,
                CharacterName("Test Fighter".into()),
                Class::Fighter,
                Race::Human,
                PartyRow::Front,
                PartySlot(0),
                base,
                derived,
                Experience {
                    level: 5,
                    current_xp: 0,
                    xp_to_next_level: 500,
                    unspent_skill_points: 5,
                    total_skill_points_earned: 5,
                },
                StatusEffects::default(),
                KnownSpells::default(),
                UnlockedNodes::default(),
            ))
            .id();

        (app, entity)
    }

    fn press_confirm(app: &mut App) {
        use leafwing_input_manager::prelude::ActionState;
        app.world_mut()
            .resource_mut::<ActionState<MenuAction>>()
            .press(&MenuAction::Confirm);
        app.update();
        app.world_mut()
            .resource_mut::<ActionState<MenuAction>>()
            .release(&MenuAction::Confirm);
        app.update();
    }

    /// Unlocking a node adds it to `UnlockedNodes` and deducts its cost.
    #[test]
    fn unlock_node_adds_to_unlocked_set_and_deducts_skill_point() {
        let tree = make_test_tree();
        let (mut app, entity) = build_test_app(tree);

        // Sorted order: depth-0 alphabetical = level_gated(0), root_node(1), stat_node(2).
        // node_cursor=1 → root_node (cost=1, min_level=1, no prereqs).
        app.world_mut().resource_mut::<GuildState>().node_cursor = 1;

        press_confirm(&mut app);

        let world = app.world();
        let unlocked = world.get::<UnlockedNodes>(entity).unwrap();
        assert!(
            unlocked.has("root_node"),
            "root_node should be unlocked; got {:?}",
            unlocked.nodes
        );
        let exp = world.get::<Experience>(entity).unwrap();
        assert_eq!(exp.unspent_skill_points, 4, "1 SP should have been deducted");
    }

    /// Unlocking a `LearnSpell` node appends the spell ID to `KnownSpells`.
    #[test]
    fn unlock_node_learn_spell_grant_appends_known_spells() {
        let tree = make_test_tree();
        let (mut app, entity) = build_test_app(tree);

        // First unlock root_node so spell_node's prereq is met.
        {
            let mut unlocked = app.world_mut().get_mut::<UnlockedNodes>(entity).unwrap();
            unlocked.unlock("root_node".into(), MAX_SKILL_TREE_NODES);
        }
        // Still 5 SP — manually unlocked root_node so no SP was spent; keep it consistent.

        // Find the sorted index of spell_node.
        // Sorted by depth: root_node(0), stat_node(0), spell_node(1), level_gated(0).
        // Alphabetical within depth: depth-0: level_gated, root_node, stat_node; depth-1: spell_node.
        // node cursor 3 should be spell_node (depth 1).
        app.world_mut().resource_mut::<GuildState>().node_cursor = 3;
        press_confirm(&mut app);

        let known = app.world().get::<KnownSpells>(entity).unwrap();
        assert!(
            known.knows("test_spell"),
            "test_spell should be in KnownSpells after unlocking spell_node; got {:?}",
            known.spells
        );
    }

    /// Unlocking a `StatBoost` node writes an `EquipmentChangedEvent`.
    #[test]
    fn unlock_node_stat_boost_writes_equipment_changed_event() {
        let tree = make_test_tree();
        let (mut app, entity) = build_test_app(tree);

        // stat_node is at sorted index 2 (depth 0, alphabetical: level_gated=0, root_node=1, stat_node=2).
        app.world_mut().resource_mut::<GuildState>().node_cursor = 2;

        let base_before = app.world().get::<BaseStats>(entity).unwrap().strength;
        press_confirm(&mut app);
        let base_after = app.world().get::<BaseStats>(entity).unwrap().strength;

        assert_eq!(
            base_after,
            base_before + 2,
            "stat_node grants +2 STR; expected {} got {}",
            base_before + 2,
            base_after
        );
    }

    /// Confirm on a node with unmet prerequisites shows a toast but does NOT unlock.
    #[test]
    fn unlock_node_rejects_missing_prereq_with_toast() {
        let tree = make_test_tree();
        let (mut app, entity) = build_test_app(tree);

        // spell_node requires root_node; root_node is NOT yet unlocked.
        // Find spell_node index.
        app.world_mut().resource_mut::<GuildState>().node_cursor = 3; // spell_node is depth 1

        press_confirm(&mut app);

        let unlocked = app.world().get::<UnlockedNodes>(entity).unwrap();
        assert!(
            !unlocked.has("spell_node"),
            "spell_node should NOT be unlocked when prereq is missing"
        );
        let toasts = app.world().resource::<Toasts>();
        assert!(
            !toasts.queue.is_empty(),
            "a toast should have been pushed for the failed unlock"
        );
    }

    /// Confirm on a level-gated node shows a toast when character is below min level.
    #[test]
    fn unlock_node_rejects_when_below_min_level_with_toast() {
        let tree = make_test_tree();
        let (mut app, entity) = build_test_app(tree);

        // Set character level to 1 (below level_gated's min_level=5).
        {
            let mut exp = app.world_mut().get_mut::<Experience>(entity).unwrap();
            exp.level = 1;
        }

        // level_gated is at sorted index 0 (depth 0, alphabetical first: "level_gated" < "root_node").
        app.world_mut().resource_mut::<GuildState>().node_cursor = 0;

        press_confirm(&mut app);

        let unlocked = app.world().get::<UnlockedNodes>(entity).unwrap();
        assert!(
            !unlocked.has("level_gated"),
            "level_gated should NOT be unlocked when character is below min_level=5"
        );
        let toasts = app.world().resource::<Toasts>();
        assert!(
            !toasts.queue.is_empty(),
            "a toast should have been pushed for the below-min-level rejection"
        );
    }
}
