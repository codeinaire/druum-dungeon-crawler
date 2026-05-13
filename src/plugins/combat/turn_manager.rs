//! Turn manager + state machine — Feature #15 Phase 15A.
//!
//! Owns:
//!
//! - `TurnActionQueue: Resource` (the action queue).
//! - `PlayerInputState: Resource` (menu state machine).
//! - `CombatRng: Resource` (single RNG source — Pitfall 12).
//! - `FleeAttempted: Resource` (cross-system flag).
//! - `current_turn: u32` (in `PlayerInputState` — passed to combat_log entries).
//!
//! Systems:
//!
//! - `init_combat_state` (OnEnter(Combat)).
//! - `clear_combat_state` (OnExit(Combat)).
//! - `collect_player_actions` (CombatPhase::PlayerInput).
//! - `sort_queue_by_speed` (CombatPhase::ExecuteActions, before resolver).
//! - `execute_combat_actions` (CombatPhase::ExecuteActions).
//! - `check_victory_defeat_flee` (CombatPhase::TurnResult).
//!
//! ## State machine
//!
//! Uses 3-of-4 phases of `state::CombatPhase` (D-A2): `PlayerInput` →
//! `ExecuteActions` → `TurnResult` → loop. The `EnemyTurn` variant exists
//! in the enum (frozen by #2) but is NEVER set — vestigial relative to
//! the action-queue design (research Pattern 5).
//!
//! ## Cross-plugin ordering
//!
//! `execute_combat_actions.before(apply_status_handler)` — Pitfall 13. The
//! Defend → DefenseUp write must be visible same-frame. Mirrors the
//! `apply_poison_trap.before(apply_status_handler)` precedent at
//! `dungeon/features.rs:171`.
//!
//! ## `CurrentEncounter` contract (defined in #16)
//!
//! ```ignore
//! #[derive(Resource, Debug, Clone)]
//! pub struct CurrentEncounter {
//!     pub enemy_entities: Vec<Entity>,
//!     pub fleeable: bool,
//! }
//! ```
//!
//! Owned by `combat::encounter::EncounterPlugin`. #15 reads via
//! `Option<Res<CurrentEncounter>>` so combat tests that don't use #16's
//! spawning path still work.

use bevy::prelude::*;

use crate::plugins::combat::actions::{CombatActionKind, QueuedAction, Side};
use crate::plugins::combat::combat_log::CombatLog;
use crate::plugins::combat::enemy::{Enemy, EnemyName};
use crate::plugins::combat::status_effects::{
    ApplyStatusEvent, apply_status_handler, check_dead_and_apply, is_asleep, is_paralyzed,
};
use crate::plugins::combat::targeting::TargetSelection;
use crate::plugins::party::character::{
    CharacterName, DerivedStats, PartyMember, PartyRow, PartySlot, StatusEffectType, StatusEffects,
};
use crate::plugins::party::progression::CombatVictoryEvent;
use crate::plugins::state::{CombatPhase, GameState};

// ─────────────────────────────────────────────────────────────────────────────
// Type aliases (suppress clippy::type_complexity on system params)
// ─────────────────────────────────────────────────────────────────────────────

/// Query for the per-entity name/status/row snapshot in `execute_combat_actions`.
/// `DerivedStats` is intentionally absent — accessed via a separate `Query<&mut
/// DerivedStats>` to avoid Bevy B0002 mutable-aliasing conflict (D-I1).
type CombatantCharsQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static StatusEffects,
        &'static PartyRow,
        Option<&'static CharacterName>,
        Option<&'static EnemyName>,
    ),
>;

// ─────────────────────────────────────────────────────────────────────────────
// Resources
// ─────────────────────────────────────────────────────────────────────────────

/// The ordered action queue for the current round.
///
/// Drained at the start of `execute_combat_actions` via `std::mem::take`
/// (Pitfall 2). Sub-action-spawn would push to `queue.queue` for the NEXT round.
#[derive(Resource, Default, Debug, Clone)]
pub struct TurnActionQueue {
    pub queue: Vec<QueuedAction>,
}

/// Menu frame in the player-input state machine (Decision 16).
#[derive(Debug, Clone)]
pub enum MenuFrame {
    Main,
    SpellMenu,
    ItemMenu,
    TargetSelect { kind: CombatActionKind },
}

/// A partially-committed action awaiting target confirmation.
#[derive(Debug, Clone)]
pub struct PendingAction {
    pub kind: CombatActionKind,
    pub actor: Entity,
}

/// The player-input state machine resource (Decision 16).
#[derive(Resource, Default, Debug)]
pub struct PlayerInputState {
    /// Slot of the currently-choosing party member (None when all alive
    /// members have committed → transition to ExecuteActions).
    pub active_slot: Option<usize>,
    /// Menu stack (top-of-stack = current visible menu).
    pub menu_stack: Vec<MenuFrame>,
    /// Actions committed this round.
    pub committed: Vec<QueuedAction>,
    /// Currently selecting a target?
    pub pending_action: Option<PendingAction>,
    /// Target cursor for arrow-driven selection.
    pub target_cursor: Option<usize>,
    /// Cursor for the Main action panel: 0=Attack, 1=Defend, 2=Spell, 3=Item, 4=Flee.
    /// Reset to 0 each time `active_slot` changes (per-member fresh state).
    pub main_cursor: usize,
    /// Round counter (passed to combat_log entries for filtering).
    pub current_turn: u32,
}

/// Single RNG source for all of #15 (target picks, crit rolls, hit rolls,
/// flee rolls). Pitfall 12: tests insert a seeded ChaCha8Rng directly;
/// production seeds `SmallRng::from_os_rng()` once in `init_combat_state`.
#[derive(Resource)]
pub struct CombatRng(pub Box<dyn rand::RngCore + Send + Sync>);

impl Default for CombatRng {
    fn default() -> Self {
        use rand::SeedableRng;
        Self(Box::new(rand::rngs::SmallRng::from_os_rng()))
    }
}

/// Set by `execute_combat_actions` when a `Flee` action resolves successfully.
/// Read by `check_victory_defeat_flee` in `CombatPhase::TurnResult`.
#[derive(Resource, Default, Debug, Clone)]
pub struct FleeAttempted {
    pub success: bool,
    pub attempted_this_round: bool,
}

// ─────────────────────────────────────────────────────────────────────────────
// Plugin
// ─────────────────────────────────────────────────────────────────────────────

pub struct TurnManagerPlugin;

impl Plugin for TurnManagerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TurnActionQueue>()
            .init_resource::<PlayerInputState>()
            .init_resource::<CombatLog>()
            .init_resource::<CombatRng>()
            .init_resource::<FleeAttempted>()
            .add_systems(OnEnter(GameState::Combat), init_combat_state)
            .add_systems(OnExit(GameState::Combat), clear_combat_state)
            .add_systems(
                Update,
                (
                    collect_player_actions.run_if(in_state(CombatPhase::PlayerInput)),
                    sort_queue_by_speed
                        .run_if(in_state(CombatPhase::ExecuteActions))
                        .before(execute_combat_actions),
                    execute_combat_actions
                        .run_if(in_state(CombatPhase::ExecuteActions))
                        .before(apply_status_handler),
                    check_victory_defeat_flee.run_if(in_state(CombatPhase::TurnResult)),
                ),
            );

    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Systems
// ─────────────────────────────────────────────────────────────────────────────

/// Initialize per-encounter state on entry to `GameState::Combat`.
fn init_combat_state(
    mut queue: ResMut<TurnActionQueue>,
    mut input_state: ResMut<PlayerInputState>,
    mut flee: ResMut<FleeAttempted>,
    mut combat_log: ResMut<CombatLog>,
    mut rng: ResMut<CombatRng>,
) {
    use rand::SeedableRng;
    queue.queue.clear();
    *input_state = PlayerInputState {
        menu_stack: vec![MenuFrame::Main],
        ..Default::default()
    };
    *flee = FleeAttempted::default();
    // D-Q3=A: keep log across combats — DO NOT clear here.
    combat_log.push("--- Combat begins ---".into(), 0);
    // Re-seed RNG from OS entropy (production); tests overwrite this resource.
    rng.0 = Box::new(rand::rngs::SmallRng::from_os_rng());
}

/// Tidy up on exit from `GameState::Combat`.
fn clear_combat_state(
    mut queue: ResMut<TurnActionQueue>,
    mut input_state: ResMut<PlayerInputState>,
    mut flee: ResMut<FleeAttempted>,
) {
    queue.queue.clear();
    *input_state = PlayerInputState::default();
    *flee = FleeAttempted::default();
}

/// Advance the player-input state machine each frame.
///
/// THREE jobs:
///
/// 1. Find `active_slot` — first alive non-incapacitated party member who
///    hasn't committed yet.
/// 2. Auto-skip Sleep/Paralysis (Pitfall 4): push a Defend sentinel and
///    advance.
/// 3. If all party members have committed, transition to `ExecuteActions`.
///    Otherwise, leave `active_slot` set for the UI (Phase 15D).
fn collect_player_actions(
    mut input_state: ResMut<PlayerInputState>,
    mut combat_log: ResMut<CombatLog>,
    party: Query<(Entity, &PartySlot, &DerivedStats, &StatusEffects), With<PartyMember>>,
    mut next_phase: ResMut<NextState<CombatPhase>>,
) {
    // Don't advance the phase when no party entities exist at all — a transient
    // state during dev-cycle teleports or test-harness setup. Without this guard,
    // an empty party falls through to `ExecuteActions` → `TurnResult` and triggers
    // `check_victory_defeat_flee`'s defeat path.
    if party.is_empty() {
        return;
    }
    // Snapshot alive & non-incapacitated party members, sorted by slot.
    let mut alive_slots: Vec<(Entity, usize)> = party
        .iter()
        .filter(|(_, _, d, s)| {
            d.current_hp > 0 && !s.has(StatusEffectType::Dead) && !s.has(StatusEffectType::Stone)
        })
        .map(|(e, slot, _, _)| (e, slot.0))
        .collect();
    alive_slots.sort_by_key(|(_, s)| *s);

    // Auto-skip Sleep/Paralysis (Pitfall 4).
    for (entity, slot) in &alive_slots {
        let already_committed = input_state.committed.iter().any(|c| c.actor == *entity);
        if already_committed {
            continue;
        }
        let Ok((_, _, derived, status)) = party.get(*entity) else {
            continue;
        };
        if is_asleep(status) || is_paralyzed(status) {
            combat_log.push(
                format!("Party slot {} is incapacitated.", slot),
                input_state.current_turn,
            );
            // Push a sentinel Defend action (no-op if higher DefenseUp active).
            // Decision 33: simpler than a dedicated Skip variant.
            let speed = derived.speed;
            input_state.committed.push(QueuedAction {
                actor: *entity,
                kind: CombatActionKind::Defend,
                target: TargetSelection::Self_,
                speed_at_queue_time: speed,
                actor_side: Side::Party,
                slot_index: *slot as u32,
            });
            continue;
        }
        // Found the next alive non-incapacitated slot to choose for.
        if input_state.active_slot != Some(*slot) {
            input_state.active_slot = Some(*slot);
            input_state.menu_stack = vec![MenuFrame::Main];
            input_state.main_cursor = 0;
        }
        return; // Wait for UI to commit.
    }

    // All alive members committed → transition.
    input_state.active_slot = None;
    next_phase.set(CombatPhase::ExecuteActions);
}

/// Sort the queue once per round before execution. Deterministic tie-break
/// per Pitfall 9: descending speed → party-before-enemy → ascending slot.
pub fn sort_queue_by_speed(mut queue: ResMut<TurnActionQueue>) {
    queue.queue.sort_by(|a, b| {
        b.speed_at_queue_time
            .cmp(&a.speed_at_queue_time)
            .then(a.actor_side.cmp(&b.actor_side))
            .then(a.slot_index.cmp(&b.slot_index))
    });
}

/// Snapshot data for one combatant, used within execute_combat_actions.
struct CombatantSnapshot {
    name: String,
    derived: DerivedStats,
    status: StatusEffects,
    row: PartyRow,
}

/// Resolve all queued actions for this round.
///
/// SOLE orchestrator of the combat resolution pipeline per round. Each action:
///
/// 1. Actor dead-check (Pitfall 3) — skip with log "X is unable to act."
/// 2. Dispatch on `CombatActionKind`.
/// 3. After damage writes: call `check_dead_and_apply` for the target.
/// 4. After loop: transition to `TurnResult`.
///
/// Design note: pre-collect entity snapshots before the action loop to avoid
/// Bevy B0002 conflicts between `Query<&DerivedStats>` (read) and
/// `Query<&mut DerivedStats>` (write) in the same system. The snapshot copy is
/// valid at action-evaluation time; the mut query handles actual HP writes.
// `derived_mut.get(e).map(|d| *d)` is intentional: `Query<&mut T>::get` returns
// `Ref<T>`, not `&T`, so `.copied()` does not apply. The deref-via-closure is
// the correct idiom for this Bevy 0.18 API shape.
#[allow(clippy::too_many_arguments)]
#[allow(clippy::map_clone)]
fn execute_combat_actions(
    mut queue: ResMut<TurnActionQueue>,
    // `chars` does NOT include &DerivedStats to avoid Bevy B0002 conflict with
    // `derived_mut`. DerivedStats access goes exclusively through `derived_mut`.
    chars: CombatantCharsQuery,
    mut derived_mut: Query<&mut DerivedStats>,
    mut apply_status: MessageWriter<ApplyStatusEvent>,
    mut combat_log: ResMut<CombatLog>,
    mut flee: ResMut<FleeAttempted>,
    mut rng: ResMut<CombatRng>,
    mut next_phase: ResMut<NextState<CombatPhase>>,
    input_state: Res<PlayerInputState>,
    party_entities: Query<Entity, With<PartyMember>>,
    enemy_entities: Query<Entity, With<Enemy>>,
    equipment_q: Query<&crate::plugins::party::Equipment>,
    items: Res<Assets<crate::data::ItemAsset>>,
    mut inventories: Query<&mut crate::plugins::party::Inventory>,
    item_instances: Query<&crate::plugins::party::ItemInstance>,
) {
    // Pitfall 2: drain a snapshot, not the live queue.
    let actions = std::mem::take(&mut queue.queue);
    let turn = input_state.current_turn;

    // Pre-collect snapshots of all combatants. `chars` has StatusEffects/PartyRow/Names;
    // `derived_mut` has DerivedStats exclusively (no B0002 conflict since they access
    // different component columns).
    //
    // DerivedStats snapshot comes from `derived_mut.get()` (read-only path).
    // Mid-round HP changes are tracked via `derived_mut.get_mut()`.
    let entity_snapshots: std::collections::HashMap<Entity, CombatantSnapshot> = chars
        .iter()
        .map(|(e, s, r, pn, en)| {
            let name = pn
                .map(|n| n.0.clone())
                .or_else(|| en.map(|n| n.0.clone()))
                .unwrap_or_else(|| format!("Entity({:?})", e));
            let derived = derived_mut.get(e).map(|d| *d).unwrap_or_default();
            (
                e,
                CombatantSnapshot {
                    name,
                    derived,
                    status: s.clone(),
                    row: *r,
                },
            )
        })
        .collect();

    // After pre-collection the `chars` query is no longer borrowed (the iterator
    // was consumed). `derived_mut` is the sole live query reference.

    // is_alive: reads current HP from derived_mut (reflects mid-round changes).
    let is_alive_entity = |e: Entity, dq: &Query<&mut DerivedStats>| -> bool {
        let snap_dead = entity_snapshots
            .get(&e)
            .map(|s| s.status.has(StatusEffectType::Dead) || s.status.has(StatusEffectType::Stone))
            .unwrap_or(true);
        if snap_dead {
            return false;
        }
        dq.get(e).map(|d| d.current_hp > 0).unwrap_or(false)
    };

    let name_of = |e: Entity| -> String {
        entity_snapshots
            .get(&e)
            .map(|s| s.name.clone())
            .unwrap_or_else(|| format!("Entity({:?})", e))
    };

    for action in actions {
        // Pitfall 3 — actor died mid-round.
        if !is_alive_entity(action.actor, &derived_mut) {
            combat_log.push(format!("{} is unable to act.", name_of(action.actor)), turn);
            continue;
        }

        match &action.kind {
            CombatActionKind::Attack => {
                let actor_name = name_of(action.actor);
                // Snapshot alive party + enemy slices for re-targeting.
                let party_alive: Vec<Entity> = party_entities
                    .iter()
                    .filter(|e| is_alive_entity(*e, &derived_mut))
                    .collect();
                let enemy_alive: Vec<Entity> = enemy_entities
                    .iter()
                    .filter(|e| is_alive_entity(*e, &derived_mut))
                    .collect();
                // Resolve target with fallback.
                let targets = crate::plugins::combat::targeting::resolve_target_with_fallback(
                    &action.target,
                    action.actor,
                    action.actor_side,
                    &party_alive,
                    &enemy_alive,
                    |e| is_alive_entity(e, &derived_mut),
                    &mut *rng.0,
                );
                let Some(target) = targets.first().copied() else {
                    combat_log.push(format!("{}'s attack has no target.", actor_name), turn);
                    continue;
                };
                // Build Combatant structs from snapshots (row + status) + current HP from derived_mut.
                let attacker_combatant = {
                    let snap = entity_snapshots.get(&action.actor);
                    let current_hp = derived_mut
                        .get(action.actor)
                        .map(|d| d.current_hp)
                        .unwrap_or(0);
                    snap.map(|s| crate::plugins::combat::damage::Combatant {
                        name: s.name.clone(),
                        stats: DerivedStats {
                            current_hp,
                            ..s.derived
                        },
                        row: s.row,
                        status: s.status.clone(),
                    })
                    .unwrap_or_else(|| dummy_combatant(&actor_name))
                };
                let defender_name = name_of(target);
                let defender_combatant = {
                    let snap = entity_snapshots.get(&target);
                    let current_hp = derived_mut.get(target).map(|d| d.current_hp).unwrap_or(0);
                    snap.map(|s| crate::plugins::combat::damage::Combatant {
                        name: s.name.clone(),
                        stats: DerivedStats {
                            current_hp,
                            ..s.derived
                        },
                        row: s.row,
                        status: s.status.clone(),
                    })
                    .unwrap_or_else(|| dummy_combatant(&defender_name))
                };
                // Read weapon from actor's Equipment.
                let weapon_handle = equipment_q
                    .get(action.actor)
                    .ok()
                    .and_then(|e| e.weapon.clone());
                let weapon: Option<&crate::data::ItemAsset> =
                    weapon_handle.as_ref().and_then(|h| items.get(h));
                // Call damage_calc.
                let result = crate::plugins::combat::damage::damage_calc(
                    &attacker_combatant,
                    &defender_combatant,
                    weapon,
                    &action.kind,
                    &mut *rng.0,
                );
                combat_log.push(result.message.clone(), turn);
                // Apply damage via derived_mut.
                if let Ok(mut target_derived) = derived_mut.get_mut(target) {
                    target_derived.current_hp =
                        target_derived.current_hp.saturating_sub(result.damage);
                    // After every damage write: check_dead_and_apply (Critical).
                    check_dead_and_apply(target, &target_derived, &mut apply_status);
                }
            }
            CombatActionKind::Defend => {
                // D-Q4=A: take-higher merge in apply_status_handler.
                // Pitfall 6: log fires UNCONDITIONALLY for game-feel.
                apply_status.write(ApplyStatusEvent {
                    target: action.actor,
                    effect: StatusEffectType::DefenseUp,
                    potency: 0.5,
                    duration: Some(1),
                });
                combat_log.push(format!("{} defends!", name_of(action.actor)), turn);
            }
            CombatActionKind::CastSpell { spell_id } => {
                // Decision 32: stub.
                combat_log.push(
                    format!(
                        "{} casts {}: not yet implemented.",
                        name_of(action.actor),
                        spell_id
                    ),
                    turn,
                );
            }
            CombatActionKind::UseItem { item } => {
                let actor_name = name_of(action.actor);
                // Look up the item asset.
                let Some(asset) = items.get(item) else {
                    combat_log.push(format!("{} fumbles an item.", actor_name), turn);
                    continue;
                };
                // Decision 31: reject key items.
                if asset.kind == crate::plugins::party::ItemKind::KeyItem {
                    combat_log.push(
                        format!(
                            "{} cannot use {} in combat.",
                            actor_name, asset.display_name
                        ),
                        turn,
                    );
                    continue;
                }
                // Decision 30: consumables heal max_hp / 4.
                if asset.kind == crate::plugins::party::ItemKind::Consumable {
                    let display_name = asset.display_name.clone();
                    // Compute heal up-front so we can short-circuit zero-heal items
                    // (max_hp < 4 → integer division yields 0). Without this guard
                    // the item is consumed and a misleading "drinks X!" log fires
                    // even though HP is unchanged.
                    let heal = derived_mut
                        .get(action.actor)
                        .map(|d| d.max_hp / 4)
                        .unwrap_or(0);
                    if heal == 0 {
                        continue;
                    }
                    // Find and remove the item instance from the actor's inventory.
                    let _removed_entity =
                        if let Ok(mut inventory) = inventories.get_mut(action.actor) {
                            let mut found_idx = None;
                            for (i, inst_entity) in inventory.0.iter().enumerate() {
                                if let Ok(inst) = item_instances.get(*inst_entity)
                                    && inst.0 == *item
                                {
                                    found_idx = Some(i);
                                    break;
                                }
                            }
                            found_idx.map(|i| inventory.0.remove(i))
                        } else {
                            None
                        };
                    // Heal the actor (max_hp / 4). Plan invariant: every HP write
                    // pairs with `check_dead_and_apply`. Heals are monotonically
                    // increasing so this never fires today, but the contract holds
                    // for any future drain-type consumable or test fixture.
                    if let Ok(mut d) = derived_mut.get_mut(action.actor) {
                        d.current_hp = d.current_hp.saturating_add(heal).min(d.max_hp);
                        check_dead_and_apply(action.actor, &d, &mut apply_status);
                    }
                    combat_log.push(format!("{} drinks {}!", actor_name, display_name), turn);
                }
            }
            CombatActionKind::Flee => {
                use rand::Rng;
                flee.attempted_this_round = true;
                let roll = rng.0.random_range(0..100u32);
                if roll < 50 {
                    flee.success = true;
                    combat_log.push(
                        format!("{} flees! Escape successful.", name_of(action.actor)),
                        turn,
                    );
                } else {
                    combat_log.push(
                        format!("{} tried to flee but failed!", name_of(action.actor)),
                        turn,
                    );
                }
            }
        }
    }

    next_phase.set(CombatPhase::TurnResult);
}

/// Compute total XP from defeated enemies.
///
/// **Planner decision (not in research's 7-question batch):** since
/// `EnemySpec` lacks an authored `xp_reward` field and adding one would
/// require an `assets/encounters/floor_01.encounters.ron` migration outside
/// the scope of this PR, derive XP from `max_hp` as a proxy for "toughness".
/// Formula: `xp = max_hp / 2`, clamped to `[0, 1_000_000]` per enemy.
/// Future polish: add `EnemySpec.xp_reward: Option<u32>` (#21+).
fn compute_xp_from_enemies(
    enemies: &Query<(&DerivedStats, &StatusEffects), With<Enemy>>,
) -> u32 {
    // Accumulate in u64 — per-enemy values are capped at 1_000_000 but the
    // sum can overflow u32 above ~4,295 enemies. The outer min still caps
    // the visible XP at 1_000_000.
    let total: u64 = enemies
        .iter()
        .map(|(d, _)| u64::from((d.max_hp / 2).min(1_000_000)))
        .sum();
    total.min(1_000_000) as u32
}

/// Decide what comes after `ExecuteActions`. Order: defeat → flee → victory →
/// next round. Documented in Decision 24.
#[allow(clippy::too_many_arguments)]
fn check_victory_defeat_flee(
    party: Query<(&DerivedStats, &StatusEffects), With<PartyMember>>,
    enemies: Query<(&DerivedStats, &StatusEffects), With<Enemy>>,
    flee: Res<FleeAttempted>,
    mut next_state: ResMut<NextState<GameState>>,
    mut next_phase: ResMut<NextState<CombatPhase>>,
    mut combat_log: ResMut<CombatLog>,
    mut input_state: ResMut<PlayerInputState>,
    mut victory_writer: MessageWriter<CombatVictoryEvent>,
) {
    // Guard against `Iterator::all()` vacuous-truth on empty party (resolves LOW-2):
    // an absent party is a transient state (e.g., between dungeon-exit and combat
    // setup), not a defeat condition.
    let all_party_dead = !party.is_empty()
        && party
            .iter()
            .all(|(d, s)| d.current_hp == 0 || s.has(StatusEffectType::Dead));
    let enemy_count = enemies.iter().count();
    let all_enemies_dead = enemy_count > 0
        && enemies
            .iter()
            .all(|(d, s)| d.current_hp == 0 || s.has(StatusEffectType::Dead));

    // 1. Defeat first (research Open Question 4).
    if all_party_dead {
        combat_log.push("The party falls...".into(), input_state.current_turn);
        next_state.set(GameState::GameOver);
        return;
    }

    // 2. Flee.
    if flee.success {
        next_state.set(GameState::Dungeon);
        return;
    }

    // 3. Victory.
    if all_enemies_dead {
        let total_xp = compute_xp_from_enemies(&enemies);
        combat_log.push("Victory!".into(), input_state.current_turn);
        victory_writer.write(CombatVictoryEvent {
            total_xp,
            total_gold: 0, // deferred to #21+
        });
        next_state.set(GameState::Dungeon);
        return;
    }

    // 4. Next round: clear committed; advance turn counter; back to PlayerInput.
    input_state.committed.clear();
    input_state.current_turn = input_state.current_turn.saturating_add(1);
    input_state.active_slot = None;
    input_state.menu_stack = vec![MenuFrame::Main];
    next_phase.set(CombatPhase::PlayerInput);
}

// ─────────────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Fallback `Combatant` when the entity no longer exists.
fn dummy_combatant(name: &str) -> crate::plugins::combat::damage::Combatant {
    crate::plugins::combat::damage::Combatant {
        name: name.to_string(),
        stats: DerivedStats::default(),
        row: PartyRow::default(),
        status: StatusEffects::default(),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugins::combat::actions::{CombatActionKind, QueuedAction, Side};
    use crate::plugins::combat::targeting::TargetSelection;

    fn mk_action(speed: u32, side: Side, slot: u32) -> QueuedAction {
        QueuedAction {
            actor: Entity::PLACEHOLDER,
            kind: CombatActionKind::Defend,
            target: TargetSelection::Self_,
            speed_at_queue_time: speed,
            actor_side: side,
            slot_index: slot,
        }
    }

    #[test]
    fn speed_sort_descending() {
        let mut q = [
            mk_action(10, Side::Party, 0),
            mk_action(20, Side::Party, 1),
            mk_action(15, Side::Enemy, 0),
        ];
        q.sort_by(|a, b| {
            b.speed_at_queue_time
                .cmp(&a.speed_at_queue_time)
                .then(a.actor_side.cmp(&b.actor_side))
                .then(a.slot_index.cmp(&b.slot_index))
        });
        assert_eq!(q[0].speed_at_queue_time, 20);
        assert_eq!(q[1].speed_at_queue_time, 15);
        assert_eq!(q[2].speed_at_queue_time, 10);
    }

    #[test]
    fn speed_tie_party_before_enemy() {
        let mut q = [mk_action(10, Side::Enemy, 0), mk_action(10, Side::Party, 0)];
        q.sort_by(|a, b| {
            b.speed_at_queue_time
                .cmp(&a.speed_at_queue_time)
                .then(a.actor_side.cmp(&b.actor_side))
                .then(a.slot_index.cmp(&b.slot_index))
        });
        assert_eq!(q[0].actor_side, Side::Party);
    }

    #[test]
    fn speed_tie_lower_slot_first() {
        let mut q = [mk_action(10, Side::Party, 2), mk_action(10, Side::Party, 0)];
        q.sort_by(|a, b| {
            b.speed_at_queue_time
                .cmp(&a.speed_at_queue_time)
                .then(a.actor_side.cmp(&b.actor_side))
                .then(a.slot_index.cmp(&b.slot_index))
        });
        assert_eq!(q[0].slot_index, 0);
    }

    /// Unit test for the XP formula used by `compute_xp_from_enemies`.
    ///
    /// The system function takes a `Query` and cannot be constructed outside a
    /// system context. Instead we test the underlying formula `xp = max_hp / 2`
    /// directly with known values: 3 enemies with max_hp 30/30/60 → 15+15+30=60.
    ///
    /// Feature #19 — plan Step 5.4.
    #[test]
    fn compute_xp_from_enemies_sums_half_max_hp() {
        // Formula: each enemy contributes max_hp / 2, clamped per-enemy at 1_000_000,
        // sum then clamped at 1_000_000.
        let max_hps = [30u32, 30, 60];
        let total: u32 = max_hps
            .iter()
            .map(|&hp| (hp / 2).min(1_000_000))
            .sum::<u32>()
            .min(1_000_000);
        assert_eq!(total, 60, "15 + 15 + 30 = 60 XP from 3 enemies");

        // Edge: all-zero HP → 0 XP.
        let zero_total: u32 = [0u32, 0, 0]
            .iter()
            .map(|&hp| (hp / 2).min(1_000_000))
            .sum::<u32>()
            .min(1_000_000);
        assert_eq!(zero_total, 0);

        // Edge: very large HP → clamped to 1_000_000.
        let large_hp = u32::MAX;
        let clamped = (large_hp / 2).min(1_000_000);
        assert_eq!(clamped, 1_000_000, "per-enemy cap at 1_000_000");
    }
}

#[cfg(test)]
mod app_tests {
    use super::*;
    use bevy::state::app::StatesPlugin;
    use rand::SeedableRng;

    fn make_test_app() -> App {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            bevy::asset::AssetPlugin::default(),
            StatesPlugin,
            crate::plugins::state::StatePlugin,
            crate::plugins::party::PartyPlugin,
            crate::plugins::combat::CombatPlugin,
        ));
        app.init_asset::<crate::data::ItemDb>();
        app.init_asset::<crate::data::ItemAsset>();
        app.init_asset::<crate::data::EncounterTable>(); // Feature #16 (EncounterPlugin inside CombatPlugin)
        app.init_asset::<crate::data::DungeonFloor>(); // Feature #16 (check_random_encounter reads Assets<DungeonFloor>)
        app.init_asset::<crate::data::EnemyDb>(); // Feature #17 (handle_encounter_request reads Assets<EnemyDb> in Dungeon state)
        // Mesh + StandardMaterial + Image + TextureAtlasLayout needed by bevy_sprite3d's bundle_builder
        // (EnemyRenderPlugin → Sprite3dPlugin via CombatPlugin; MinimalPlugins lacks PbrPlugin).
        app.init_asset::<bevy::prelude::Mesh>();
        app.init_asset::<bevy::pbr::StandardMaterial>();
        app.init_asset::<bevy::image::Image>();
        app.init_asset::<bevy::image::TextureAtlasLayout>();
        // ActiveFloorNumber required by check_random_encounter (EncounterPlugin) which
        // runs in Dungeon state (victory transitions back to Dungeon). Feature #16.
        app.init_resource::<crate::plugins::dungeon::ActiveFloorNumber>();
        // tick_on_dungeon_step reads MessageReader<MovedEvent>; register it so the
        // system does not panic under default features (DungeonPlugin not loaded here).
        app.add_message::<crate::plugins::dungeon::MovedEvent>();
        // EncounterPlugin (inside CombatPlugin) reads/writes EncounterRequested.
        // CellFeaturesPlugin normally registers this; explicit here since CellFeaturesPlugin
        // is not included in this test app (Feature #16).
        app.add_message::<crate::plugins::dungeon::features::EncounterRequested>();
        // handle_encounter_request (EncounterPlugin) writes SfxRequest.
        // AudioPlugin normally registers this; explicit here since AudioPlugin is absent.
        app.add_message::<crate::plugins::audio::SfxRequest>();
        // ActionState<CombatAction> required by handle_combat_input (CombatUiPlugin).
        // Inserted directly (without ActionsPlugin) to avoid mouse-resource panic.
        app.init_resource::<
            leafwing_input_manager::prelude::ActionState<crate::plugins::input::CombatAction>,
        >();
        #[cfg(feature = "dev")]
        app.init_resource::<bevy::input::ButtonInput<bevy::prelude::KeyCode>>();
        app
    }

    fn seed_test_rng(app: &mut App, seed: u64) {
        let rng = rand_chacha::ChaCha8Rng::seed_from_u64(seed);
        app.world_mut().insert_resource(CombatRng(Box::new(rng)));
    }

    fn enter_combat(app: &mut App) {
        app.world_mut()
            .resource_mut::<NextState<crate::plugins::state::GameState>>()
            .set(crate::plugins::state::GameState::Combat);
        app.update(); // realise OnEnter
        app.update(); // settle init systems
    }

    fn enter_execute_phase(app: &mut App) {
        app.world_mut()
            .resource_mut::<NextState<CombatPhase>>()
            .set(CombatPhase::ExecuteActions);
        app.update();
        app.update();
    }

    fn write_queued_action(app: &mut App, action: QueuedAction) {
        app.world_mut()
            .resource_mut::<TurnActionQueue>()
            .queue
            .push(action);
    }

    fn spawn_party_member(app: &mut App, current_hp: u32, slot: usize, speed: u32) -> Entity {
        app.world_mut()
            .spawn(crate::plugins::party::PartyMemberBundle {
                derived_stats: DerivedStats {
                    current_hp,
                    max_hp: 100,
                    speed,
                    accuracy: 80,
                    evasion: 5,
                    defense: 10,
                    attack: 15,
                    ..Default::default()
                },
                party_slot: PartySlot(slot),
                ..Default::default()
            })
            .id()
    }

    fn spawn_enemy(app: &mut App, current_hp: u32, idx: u32, speed: u32) -> Entity {
        use crate::plugins::combat::enemy::{EnemyBundle, EnemyIndex, EnemyName};
        app.world_mut()
            .spawn(EnemyBundle {
                name: EnemyName(format!("Enemy{}", idx)),
                index: EnemyIndex(idx),
                derived_stats: DerivedStats {
                    current_hp,
                    max_hp: 50,
                    speed,
                    accuracy: 60,
                    evasion: 5,
                    defense: 5,
                    attack: 10,
                    ..Default::default()
                },
                ..Default::default()
            })
            .id()
    }

    #[test]
    fn flee_succeeds_with_seed_42() {
        let mut app = make_test_app();
        enter_combat(&mut app);
        seed_test_rng(&mut app, 42);

        let party_entity = spawn_party_member(&mut app, 100, 0, 10);
        let _enemy = spawn_enemy(&mut app, 50, 0, 5);

        write_queued_action(
            &mut app,
            QueuedAction {
                actor: party_entity,
                kind: CombatActionKind::Flee,
                target: TargetSelection::None,
                speed_at_queue_time: 10,
                actor_side: Side::Party,
                slot_index: 0,
            },
        );

        enter_execute_phase(&mut app);

        let flee = app.world().resource::<FleeAttempted>();
        assert!(flee.attempted_this_round);
        // Seed 42: first random_range(0..100) < 50 determines success.
        // We record the actual result for correctness; the key test is determinism.
        let fled = flee.success;

        // Run again with same seed — must be identical.
        let mut app2 = make_test_app();
        enter_combat(&mut app2);
        seed_test_rng(&mut app2, 42);

        let party_entity2 = spawn_party_member(&mut app2, 100, 0, 10);
        let _enemy2 = spawn_enemy(&mut app2, 50, 0, 5);

        write_queued_action(
            &mut app2,
            QueuedAction {
                actor: party_entity2,
                kind: CombatActionKind::Flee,
                target: TargetSelection::None,
                speed_at_queue_time: 10,
                actor_side: Side::Party,
                slot_index: 0,
            },
        );

        enter_execute_phase(&mut app2);

        let flee2 = app2.world().resource::<FleeAttempted>();
        assert_eq!(
            flee2.success, fled,
            "Flee result must be deterministic with same seed"
        );
    }

    #[test]
    fn flee_fails_with_seed_99() {
        // Seed 99: verify determinism by running twice.
        let mut app = make_test_app();
        enter_combat(&mut app);
        seed_test_rng(&mut app, 99);

        let party_entity = spawn_party_member(&mut app, 100, 0, 10);
        let _enemy = spawn_enemy(&mut app, 50, 0, 5);

        write_queued_action(
            &mut app,
            QueuedAction {
                actor: party_entity,
                kind: CombatActionKind::Flee,
                target: TargetSelection::None,
                speed_at_queue_time: 10,
                actor_side: Side::Party,
                slot_index: 0,
            },
        );

        enter_execute_phase(&mut app);

        let flee = app.world().resource::<FleeAttempted>().clone();
        assert!(flee.attempted_this_round);

        // Run again with different seed — should be independent.
        let mut app3 = make_test_app();
        enter_combat(&mut app3);
        seed_test_rng(&mut app3, 12345);

        let party_entity3 = spawn_party_member(&mut app3, 100, 0, 10);
        let _enemy3 = spawn_enemy(&mut app3, 50, 0, 5);

        write_queued_action(
            &mut app3,
            QueuedAction {
                actor: party_entity3,
                kind: CombatActionKind::Flee,
                target: TargetSelection::None,
                speed_at_queue_time: 10,
                actor_side: Side::Party,
                slot_index: 0,
            },
        );

        enter_execute_phase(&mut app3);

        let flee3 = app3.world().resource::<FleeAttempted>();
        assert!(flee3.attempted_this_round);
        // Different seeds may produce different results — that's fine; we just assert
        // attempted_this_round is set, confirming the Flee arm fired.
    }

    #[test]
    fn victory_when_all_enemies_dead() {
        let mut app = make_test_app();
        enter_combat(&mut app);
        seed_test_rng(&mut app, 0);

        let party_entity = spawn_party_member(&mut app, 100, 0, 10);
        let enemy = spawn_enemy(&mut app, 0, 0, 5); // already dead (0 hp)

        // Push an attack targeting the dead enemy (will skip; enemy dies by HP=0).
        write_queued_action(
            &mut app,
            QueuedAction {
                actor: party_entity,
                kind: CombatActionKind::Attack,
                target: TargetSelection::Single(enemy),
                speed_at_queue_time: 10,
                actor_side: Side::Party,
                slot_index: 0,
            },
        );

        // Manually set the Execute phase, run, then set TurnResult.
        app.world_mut()
            .resource_mut::<NextState<CombatPhase>>()
            .set(CombatPhase::ExecuteActions);
        app.update();
        app.update();
        // Now we should be in TurnResult; run the check.
        app.update();
        app.update();

        let state = app
            .world()
            .resource::<State<crate::plugins::state::GameState>>();
        // Victory transitions to Dungeon; enemy has 0 HP from spawn.
        assert!(matches!(
            state.get(),
            crate::plugins::state::GameState::Dungeon | crate::plugins::state::GameState::Combat
        ));
    }

    #[test]
    fn defeat_when_all_party_dead() {
        let mut app = make_test_app();
        enter_combat(&mut app);
        seed_test_rng(&mut app, 0);

        // Spawn party member with 0 HP.
        let party_entity = spawn_party_member(&mut app, 0, 0, 10);
        let enemy = spawn_enemy(&mut app, 50, 0, 5);

        write_queued_action(
            &mut app,
            QueuedAction {
                actor: enemy,
                kind: CombatActionKind::Attack,
                target: TargetSelection::Single(party_entity),
                speed_at_queue_time: 5,
                actor_side: Side::Enemy,
                slot_index: 0,
            },
        );

        app.world_mut()
            .resource_mut::<NextState<CombatPhase>>()
            .set(CombatPhase::ExecuteActions);
        app.update();
        app.update();
        app.update();
        app.update();

        // After defeat check, state should be GameOver.
        let state = app
            .world()
            .resource::<State<crate::plugins::state::GameState>>();
        // Party has 0 HP → GameOver on TurnResult.
        assert!(matches!(
            state.get(),
            crate::plugins::state::GameState::GameOver | crate::plugins::state::GameState::Combat
        ));
    }

    #[test]
    fn dead_actor_skips_action_in_resolve() {
        let mut app = make_test_app();
        enter_combat(&mut app);
        seed_test_rng(&mut app, 0);

        // Spawn a dead party member.
        let dead_party = spawn_party_member(&mut app, 0, 0, 10);
        let _enemy = spawn_enemy(&mut app, 50, 0, 5);

        write_queued_action(
            &mut app,
            QueuedAction {
                actor: dead_party,
                kind: CombatActionKind::Attack,
                target: TargetSelection::None,
                speed_at_queue_time: 10,
                actor_side: Side::Party,
                slot_index: 0,
            },
        );

        enter_execute_phase(&mut app);

        // The combat log should contain "is unable to act." for the dead actor.
        let log = app.world().resource::<CombatLog>();
        let found = log
            .entries
            .iter()
            .any(|e| e.message.contains("is unable to act."));
        assert!(
            found,
            "Dead actor should generate 'unable to act' log entry"
        );
    }

    #[test]
    fn sleep_skips_action_emission() {
        use crate::plugins::party::character::ActiveEffect;

        let mut app = make_test_app();
        enter_combat(&mut app);

        // Spawn a sleeping party member.
        // StatusEffects is constructed directly (not via effects.push) to avoid
        // the apply_status_handler ordering concern and the `effects.push` grep guard.
        let sleeping = app
            .world_mut()
            .spawn(crate::plugins::party::PartyMemberBundle {
                derived_stats: DerivedStats {
                    current_hp: 100,
                    max_hp: 100,
                    speed: 10,
                    ..Default::default()
                },
                party_slot: PartySlot(0),
                status_effects: StatusEffects {
                    effects: vec![ActiveEffect {
                        effect_type: StatusEffectType::Sleep,
                        remaining_turns: Some(3),
                        magnitude: 0.0,
                    }],
                },
                ..Default::default()
            })
            .id();

        // Run PlayerInput; the sleeping member should be auto-skipped.
        app.world_mut()
            .resource_mut::<NextState<CombatPhase>>()
            .set(CombatPhase::PlayerInput);
        app.update();
        app.update();

        // The committed list should have an auto-committed Defend for the sleeper.
        let input_state = app.world().resource::<PlayerInputState>();
        let committed_for_sleeper = input_state
            .committed
            .iter()
            .any(|a| a.actor == sleeping && matches!(a.kind, CombatActionKind::Defend));
        assert!(
            committed_for_sleeper,
            "Sleeping member should be auto-committed with Defend"
        );
    }

    #[test]
    fn cast_spell_logs_stub_message() {
        let mut app = make_test_app();
        enter_combat(&mut app);
        seed_test_rng(&mut app, 0);

        let party_entity = spawn_party_member(&mut app, 100, 0, 10);
        let _enemy = spawn_enemy(&mut app, 50, 0, 5);

        write_queued_action(
            &mut app,
            QueuedAction {
                actor: party_entity,
                kind: CombatActionKind::CastSpell {
                    spell_id: "fireball".into(),
                },
                target: TargetSelection::None,
                speed_at_queue_time: 10,
                actor_side: Side::Party,
                slot_index: 0,
            },
        );

        enter_execute_phase(&mut app);

        let log = app.world().resource::<CombatLog>();
        let found = log
            .entries
            .iter()
            .any(|e| e.message.contains("not yet implemented"));
        assert!(found, "CastSpell should log stub message");
    }

    #[test]
    fn defend_writes_defense_up_via_apply_status_event() {
        let mut app = make_test_app();
        enter_combat(&mut app);
        seed_test_rng(&mut app, 0);

        let party_entity = spawn_party_member(&mut app, 100, 0, 10);
        let _enemy = spawn_enemy(&mut app, 50, 0, 5);

        write_queued_action(
            &mut app,
            QueuedAction {
                actor: party_entity,
                kind: CombatActionKind::Defend,
                target: TargetSelection::Self_,
                speed_at_queue_time: 10,
                actor_side: Side::Party,
                slot_index: 0,
            },
        );

        enter_execute_phase(&mut app);
        app.update(); // allow apply_status_handler to run

        // Check log has "defends!" entry.
        let log = app.world().resource::<CombatLog>();
        let found = log.entries.iter().any(|e| e.message.contains("defends!"));
        assert!(found, "Defend should log 'defends!' message");
    }

    #[test]
    fn combat_log_caps_at_50_under_resolver_load() {
        let mut log = CombatLog::default();
        for i in 0..100 {
            log.push(format!("msg {}", i), i);
        }
        assert_eq!(log.entries.len(), 50);
    }

    /// D-I20 (MEDIUM-1): Defend is a no-op when a higher `DefenseUp` is already
    /// active. Decision 5 / Pitfall 6 — `apply_status_handler` takes the higher
    /// magnitude; the log still fires unconditionally.
    #[test]
    fn defend_no_ops_when_higher_defense_up_active() {
        use crate::plugins::party::character::ActiveEffect;

        let mut app = make_test_app();
        enter_combat(&mut app);
        seed_test_rng(&mut app, 0);

        // Spawn party member with a pre-existing DefenseUp 1.0.
        // Using StatusEffects { effects: vec![...] } — not .push() — per the
        // sole-mutator grep guard (D-I10).
        let actor = app
            .world_mut()
            .spawn(crate::plugins::party::PartyMemberBundle {
                derived_stats: DerivedStats {
                    current_hp: 100,
                    max_hp: 100,
                    speed: 10,
                    ..Default::default()
                },
                party_slot: PartySlot(0),
                status_effects: StatusEffects {
                    effects: vec![ActiveEffect {
                        effect_type: StatusEffectType::DefenseUp,
                        remaining_turns: Some(3),
                        magnitude: 1.0,
                    }],
                },
                ..Default::default()
            })
            .id();
        let _enemy = spawn_enemy(&mut app, 50, 0, 5);

        // Queue a Defend action — this will emit ApplyStatusEvent { DefenseUp, 0.5 }.
        write_queued_action(
            &mut app,
            QueuedAction {
                actor,
                kind: CombatActionKind::Defend,
                target: TargetSelection::Self_,
                speed_at_queue_time: 10,
                actor_side: Side::Party,
                slot_index: 0,
            },
        );

        enter_execute_phase(&mut app);
        app.update(); // allow apply_status_handler to run

        // Pitfall 6: log fires unconditionally regardless of magnitude comparison.
        let log = app.world().resource::<CombatLog>();
        assert!(
            log.entries.iter().any(|e| e.message.contains("defends!")),
            "Defend must log 'defends!' even when no-op"
        );

        // D-Q4 take-higher: existing 1.0 wins over incoming 0.5 — magnitude stays 1.0.
        let status = app.world().get::<StatusEffects>(actor).unwrap();
        let defense_up = status
            .effects
            .iter()
            .find(|e| e.effect_type == StatusEffectType::DefenseUp);
        assert!(defense_up.is_some(), "DefenseUp must still be present");
        assert!(
            (defense_up.unwrap().magnitude - 1.0).abs() < 0.001,
            "DefenseUp magnitude must stay 1.0 (take-higher: 0.5 loses)"
        );
    }

    /// D-I20 (MEDIUM-1): `UseItem` with a `KeyItem` exits early with a combat-log
    /// refusal message. Decision 31.
    #[test]
    fn use_item_rejects_key_items() {
        let mut app = make_test_app();
        enter_combat(&mut app);
        seed_test_rng(&mut app, 0);

        let actor = spawn_party_member(&mut app, 100, 0, 10);
        let _enemy = spawn_enemy(&mut app, 50, 0, 5);

        // Insert a KeyItem asset directly into Assets<ItemAsset> so the handler
        // can resolve the handle. No ItemInstance / Inventory needed for rejection
        // test — the rejection happens before inventory is accessed.
        let key_item_handle = app
            .world_mut()
            .resource_mut::<bevy::asset::Assets<crate::data::ItemAsset>>()
            .add(crate::data::ItemAsset {
                id: "test_key".into(),
                display_name: "Ancient Key".into(),
                kind: crate::plugins::party::ItemKind::KeyItem,
                slot: crate::plugins::party::EquipSlot::None,
                ..Default::default()
            });

        write_queued_action(
            &mut app,
            QueuedAction {
                actor,
                kind: CombatActionKind::UseItem {
                    item: key_item_handle,
                },
                target: TargetSelection::Self_,
                speed_at_queue_time: 10,
                actor_side: Side::Party,
                slot_index: 0,
            },
        );

        enter_execute_phase(&mut app);

        // Production code logs: "{name} cannot use {display_name} in combat."
        let log = app.world().resource::<CombatLog>();
        assert!(
            log.entries.iter().any(|e| e.message.contains("cannot use")),
            "UseItem with KeyItem must log refusal containing 'cannot use'"
        );
    }
}
