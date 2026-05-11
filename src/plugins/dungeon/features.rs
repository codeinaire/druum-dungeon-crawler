//! Cell-feature reaction layer — Feature #13.
//!
//! Owns the Bevy systems that react to player movement onto cells with
//! special properties (traps, teleporters, spinners, anti-magic zones)
//! and the door-interaction system that toggles `WallType::Door` open/closed.
//!
//! Subscribes to `MovedEvent` (published by `dungeon/mod.rs`) and
//! `Res<ActionState<DungeonAction>>` for the Interact key.
//!
//! See `project/research/20260506-080000-feature-13-cell-features.md`.
//!
//! ## Bevy 0.18 family rename
//!
//! `TeleportRequested` and `EncounterRequested` derive `Message`, NOT `Event`.
//! Read with `MessageReader<T>`, write with `MessageWriter<T>`.
//! Register with `app.add_message::<T>()`.

use bevy::prelude::*;
use leafwing_input_manager::prelude::ActionState;
use std::collections::HashMap;

use crate::data::DungeonFloor;
use crate::data::ItemAsset;
use crate::data::dungeon::{Direction, TeleportTarget, TrapType, WallType};
use crate::plugins::audio::{SfxKind, SfxRequest};
use crate::plugins::combat::status_effects::ApplyStatusEvent;
use crate::plugins::dungeon::{
    ActiveFloorNumber, Facing, GridPosition, MovedEvent, PlayerParty, animate_movement,
    facing_to_quat, floor_handle_for, handle_dungeon_input, update_active_floor_from_pending,
};
use crate::plugins::input::DungeonAction;
use crate::plugins::loading::DungeonAssets;
use crate::plugins::party::{
    DerivedStats, Inventory, ItemInstance, ItemKind, PartyMember, StatusEffectType,
};
use crate::plugins::state::GameState;

// ---------------------------------------------------------------------------
// Resources
// ---------------------------------------------------------------------------

/// State of a single door edge. Default `Closed` — `WallType::Door` walls
/// are gated by this resource (Pitfall 4: pre-#13, `floor.can_move` returned
/// `true` for Door; the runtime override here makes Closed actually closed).
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub enum DoorState {
    #[default]
    Closed,
    Open,
}

/// Per-floor-instance door state (D1-A — cleared on `OnExit(Dungeon)`).
/// Keyed by edge `(grid_position, direction_to_other_cell)`.
#[derive(Resource, Default, Debug)]
pub struct DoorStates {
    pub doors: HashMap<(GridPosition, Direction), DoorState>,
}

/// Maps locked-door edges to their `door_id` strings. Populated on
/// `OnEnter(Dungeon)` from `DungeonFloor::locked_doors`. Cleared on
/// `OnExit(Dungeon)`. Used by `handle_door_interact` to look up the
/// expected `key_id` for a `WallType::LockedDoor` edge.
#[derive(Resource, Default, Debug)]
pub struct LockedDoors {
    pub by_edge: HashMap<(GridPosition, Direction), String>,
}

/// Pending cross-floor teleport destination. Set by `apply_teleporter`
/// publishing `TeleportRequested`; read by `LoadingPlugin`'s
/// `handle_teleport_request` system; consumed by `spawn_party_and_camera`
/// in `dungeon/mod.rs` on the next `OnEnter(Dungeon)`.
#[derive(Resource, Default, Debug)]
pub struct PendingTeleport {
    pub target: Option<TeleportTarget>,
}

// ---------------------------------------------------------------------------
// Components
// ---------------------------------------------------------------------------

/// Marker on the `PlayerParty` entity while standing in a
/// `CellFeatures::anti_magic_zone` cell. Future #14/#15 spell-casting
/// systems will query `Query<(), (With<PlayerParty>, With<AntiMagicZone>)>`
/// to gate spells.
#[derive(Component, Debug, Clone, Copy)]
pub struct AntiMagicZone;

/// In-flight screen-wobble animation attached to the `PlayerParty` entity
/// after a spinner trigger. Lifecycle mirrors `MovementAnimation`'s
/// remove-on-completion pattern. Damped sine: `amplitude × sin(8πt) × (1 − t)`.
///
/// `base_rotation` is the canonical post-spin rotation (the new facing's
/// `facing_to_quat` value). Each tick sets `transform.rotation = base * jitter`
/// ABSOLUTELY rather than composing a delta — this overrides any
/// `MovementAnimation::translate` "preserve from rotation" overwrite that
/// would otherwise lock a wobbled rotation into the camera permanently. At
/// `t = 1`, jitter is 0 and rotation lands at `base_rotation` exactly.
#[derive(Component, Debug, Clone)]
pub struct ScreenWobble {
    pub elapsed_secs: f32,
    pub duration_secs: f32,
    pub amplitude: f32,
    pub base_rotation: Quat,
}

// ---------------------------------------------------------------------------
// Messages
// ---------------------------------------------------------------------------

/// Published by `apply_teleporter` for cross-floor teleporter cells (and
/// by `apply_pit_trap` for `Pit { target_floor: Some(_) }`). Consumed by
/// `LoadingPlugin::handle_teleport_request`.
///
/// **`Message`, NOT `Event`** — Bevy 0.18 family rename.
#[derive(Message, Clone, Debug)]
pub struct TeleportRequested {
    pub target: TeleportTarget,
}

/// Published by `apply_alarm_trap` (and future random-encounter rolls).
/// Consumed by Feature #16 (combat trigger) — v1 has only a logged stub
/// consumer in this plugin.
///
/// **`Message`, NOT `Event`** — Bevy 0.18 family rename.
#[derive(Message, Clone, Copy, Debug)]
pub struct EncounterRequested {
    pub source: EncounterSource,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EncounterSource {
    AlarmTrap,
    /// Random roll triggered by `check_random_encounter` per `MovedEvent`.
    Random,
    // Future: Foe (overworld encounter) — surface in #22.
}

// ---------------------------------------------------------------------------
// Plugin
// ---------------------------------------------------------------------------

/// Owns all cell-feature systems, resources, and messages for #13.
/// Registered in `main.rs` parallel to `DungeonPlugin`/`PartyPlugin`.
pub struct CellFeaturesPlugin;

impl Plugin for CellFeaturesPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DoorStates>()
            .init_resource::<LockedDoors>()
            .init_resource::<PendingTeleport>()
            .add_message::<TeleportRequested>()
            .add_message::<EncounterRequested>()
            .add_systems(
                OnEnter(GameState::Dungeon),
                // .after(update_active_floor_from_pending) ensures the
                // ActiveFloorNumber resource has been refreshed from
                // PendingTeleport before populate reads it.
                populate_locked_doors.after(update_active_floor_from_pending),
            )
            .add_systems(OnExit(GameState::Dungeon), clear_door_resources)
            .add_systems(
                Update,
                (
                    handle_door_interact
                        .run_if(in_state(GameState::Dungeon))
                        .after(handle_dungeon_input),
                    apply_pit_trap
                        .run_if(in_state(GameState::Dungeon))
                        .after(handle_dungeon_input),
                    apply_poison_trap
                        .run_if(in_state(GameState::Dungeon))
                        .after(handle_dungeon_input)
                        .before(crate::plugins::combat::status_effects::apply_status_handler),
                    apply_alarm_trap
                        .run_if(in_state(GameState::Dungeon))
                        .after(handle_dungeon_input),
                    apply_teleporter
                        .run_if(in_state(GameState::Dungeon))
                        .after(handle_dungeon_input),
                    apply_spinner
                        .run_if(in_state(GameState::Dungeon))
                        .after(handle_dungeon_input),
                    apply_anti_magic_zone
                        .run_if(in_state(GameState::Dungeon))
                        .after(handle_dungeon_input),
                    tick_screen_wobble
                        .run_if(in_state(GameState::Dungeon))
                        .after(animate_movement), // win the rotation race (Risk register)
                ),
            );
    }
}

// ---------------------------------------------------------------------------
// Systems — diagnostic
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Systems — state management
// ---------------------------------------------------------------------------

/// Populate `LockedDoors` from `DungeonFloor::locked_doors`. Clears first
/// for idempotence across `OnEnter(Dungeon)` re-entries (Pitfall 8 — D3-α
/// teleport re-enters the state).
fn populate_locked_doors(
    mut locked_doors: ResMut<LockedDoors>,
    dungeon_assets: Option<Res<DungeonAssets>>,
    floors: Res<Assets<DungeonFloor>>,
    active_floor: Res<ActiveFloorNumber>,
) {
    locked_doors.by_edge.clear();
    let Some(assets) = dungeon_assets else {
        return;
    };
    let floor_handle = floor_handle_for(&assets, active_floor.0);
    let Some(floor) = floors.get(floor_handle) else {
        return;
    };
    for ((x, y), dir, id) in &floor.locked_doors {
        locked_doors
            .by_edge
            .insert((GridPosition { x: *x, y: *y }, *dir), id.clone());
    }
}

/// Clear door-state resources on `OnExit(Dungeon)`.
/// Prevents stale door states from leaking to future floor visits (D1 — per-floor-instance).
///
/// Does NOT clear `PendingTeleport` — that resource carries the cross-floor
/// destination ACROSS the `OnExit(Dungeon) → OnEnter(Dungeon)` boundary.
/// The authoritative consumer is `spawn_party_and_camera`, which calls
/// `pt.target.take()` once it has used the destination.
fn clear_door_resources(
    mut door_states: ResMut<DoorStates>,
    mut locked_doors: ResMut<LockedDoors>,
) {
    door_states.doors.clear();
    locked_doors.by_edge.clear();
}

// ---------------------------------------------------------------------------
// Systems — door interaction
// ---------------------------------------------------------------------------

/// Reads `Res<ActionState<DungeonAction>>`; on `Interact` press, looks at the
/// wall the player is facing. For `WallType::Door`, toggles `DoorState`. For
/// `WallType::LockedDoor`, walks all party inventories looking for a matching
/// `ItemKind::KeyItem` with `key_id == door_id`; if found, sets `DoorState::Open`.
/// Keys are NOT consumed (D13 — Wizardry-style; reusable).
#[allow(clippy::too_many_arguments)]
fn handle_door_interact(
    actions: Res<ActionState<DungeonAction>>,
    party: Query<(&GridPosition, &Facing), With<PlayerParty>>,
    floors: Res<Assets<DungeonFloor>>,
    dungeon_assets: Option<Res<DungeonAssets>>,
    active_floor: Res<ActiveFloorNumber>,
    locked_doors: Res<LockedDoors>,
    mut door_states: ResMut<DoorStates>,
    inventory: Query<&Inventory, With<PartyMember>>,
    instances: Query<&ItemInstance>,
    items: Res<Assets<ItemAsset>>,
    mut sfx: MessageWriter<SfxRequest>,
) {
    if !actions.just_pressed(&DungeonAction::Interact) {
        return;
    }
    let Ok((pos, facing)) = party.single() else {
        info!("Interact pressed but no PlayerParty entity");
        return;
    };
    let Some(assets) = dungeon_assets else {
        info!(
            "Interact pressed at {:?} facing {:?} but DungeonAssets not loaded",
            pos, facing.0
        );
        return;
    };
    let floor_handle = floor_handle_for(&assets, active_floor.0);
    let Some(floor) = floors.get(floor_handle) else {
        info!(
            "Interact pressed at {:?} facing {:?} but floor {} not yet loaded",
            pos, facing.0, active_floor.0
        );
        return;
    };

    let edge_dir = facing.0;
    let cell_walls = &floor.walls[pos.y as usize][pos.x as usize];
    let wall = match edge_dir {
        Direction::North => cell_walls.north,
        Direction::South => cell_walls.south,
        Direction::East => cell_walls.east,
        Direction::West => cell_walls.west,
    };

    match wall {
        WallType::Door => {
            let key = (*pos, edge_dir);
            let current = door_states.doors.get(&key).copied().unwrap_or_default();
            let new_state = match current {
                DoorState::Closed => DoorState::Open,
                DoorState::Open => DoorState::Closed,
            };
            door_states.doors.insert(key, new_state);
            info!(
                "Door at {:?} {:?} toggled {:?} → {:?}",
                pos, edge_dir, current, new_state
            );
            sfx.write(SfxRequest {
                kind: if new_state == DoorState::Open {
                    SfxKind::Door
                } else {
                    SfxKind::DoorClose
                },
            });
        }
        WallType::LockedDoor => {
            let Some(door_id) = locked_doors.by_edge.get(&(*pos, edge_dir)) else {
                return;
            };
            let mut has_key = false;
            'outer: for inv in &inventory {
                for &item_entity in &inv.0 {
                    let Ok(instance) = instances.get(item_entity) else {
                        continue;
                    };
                    let Some(asset) = items.get(&instance.0) else {
                        continue;
                    };
                    if asset.kind == ItemKind::KeyItem
                        && asset.key_id.as_deref() == Some(door_id.as_str())
                    {
                        has_key = true;
                        break 'outer;
                    }
                }
            }
            if has_key {
                door_states.doors.insert((*pos, edge_dir), DoorState::Open);
                sfx.write(SfxRequest {
                    kind: SfxKind::Door,
                });
                info!(
                    "Unlocked door at {:?} {:?} with key '{}'",
                    pos, edge_dir, door_id
                );
                // D13: do NOT consume the key.
            } else {
                info!(
                    "Locked door at {:?} {:?} requires key '{}'",
                    pos, edge_dir, door_id
                );
            }
        }
        other => {
            info!(
                "Interact pressed at {:?} facing {:?}: wall is {:?}, not a door",
                pos, edge_dir, other
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Systems — cell feature reactions
// ---------------------------------------------------------------------------

/// Apply pit-trap damage on entry. Saturating subtract guards against u32
/// underflow (Pitfall 7). Emits `TeleportRequested` for cross-floor pits.
fn apply_pit_trap(
    mut moved: MessageReader<MovedEvent>,
    floors: Res<Assets<DungeonFloor>>,
    dungeon_assets: Option<Res<DungeonAssets>>,
    active_floor: Res<ActiveFloorNumber>,
    mut party: Query<&mut DerivedStats, With<PartyMember>>,
    mut sfx: MessageWriter<SfxRequest>,
    mut teleport: MessageWriter<TeleportRequested>,
) {
    let Some(assets) = dungeon_assets else {
        return;
    };
    let floor_handle = floor_handle_for(&assets, active_floor.0);
    let Some(floor) = floors.get(floor_handle) else {
        return;
    };
    for ev in moved.read() {
        let cell = &floor.features[ev.to.y as usize][ev.to.x as usize];
        let Some(TrapType::Pit {
            damage,
            target_floor,
        }) = &cell.trap
        else {
            continue;
        };
        // Apply to all party members (genre convention — Pitfall 7: saturating_sub).
        for mut derived in &mut party {
            derived.current_hp = derived.current_hp.saturating_sub(*damage);
        }
        sfx.write(SfxRequest {
            kind: SfxKind::AttackHit,
        });
        if let Some(target_floor_num) = target_floor {
            teleport.write(TeleportRequested {
                target: TeleportTarget {
                    floor: *target_floor_num,
                    x: ev.to.x,
                    y: ev.to.y,
                    facing: Some(ev.facing),
                },
            });
        }
    }
}

/// Apply poison trap on entry. Writes `ApplyStatusEvent` (handled by
/// `combat/status_effects.rs::apply_status_handler` which enforces stacking).
/// **Refactored in #14:** prior naive `effects.push(...)` removed; the
/// canonical handler is now the single mutator of `StatusEffects.effects`.
///
/// Ordered `.before(apply_status_handler)` in `CellFeaturesPlugin::build`
/// (Pitfall 1 of #14: same-frame consumability).
fn apply_poison_trap(
    mut moved: MessageReader<MovedEvent>,
    floors: Res<Assets<DungeonFloor>>,
    dungeon_assets: Option<Res<DungeonAssets>>,
    active_floor: Res<ActiveFloorNumber>,
    party: Query<Entity, With<PartyMember>>,
    mut apply: MessageWriter<ApplyStatusEvent>,
    mut sfx: MessageWriter<SfxRequest>,
) {
    const POISON_DURATION_TICKS: u32 = 5;
    const POISON_TRAP_POTENCY: f32 = 1.0; // Risk 3: NOT 0.0.

    let Some(assets) = dungeon_assets else {
        return;
    };
    let floor_handle = floor_handle_for(&assets, active_floor.0);
    let Some(floor) = floors.get(floor_handle) else {
        return;
    };
    for ev in moved.read() {
        let cell = &floor.features[ev.to.y as usize][ev.to.x as usize];
        if !matches!(cell.trap, Some(TrapType::Poison)) {
            continue;
        }
        for entity in &party {
            apply.write(ApplyStatusEvent {
                target: entity,
                effect: StatusEffectType::Poison,
                potency: POISON_TRAP_POTENCY,
                duration: Some(POISON_DURATION_TICKS),
            });
        }
        sfx.write(SfxRequest {
            kind: SfxKind::Door,
        }); // placeholder hiss (D10-A reuse — unchanged from #13)
    }
}

/// Alarm trap — publish `EncounterRequested` and log for #16's consumer (D5).
fn apply_alarm_trap(
    mut moved: MessageReader<MovedEvent>,
    floors: Res<Assets<DungeonFloor>>,
    dungeon_assets: Option<Res<DungeonAssets>>,
    active_floor: Res<ActiveFloorNumber>,
    mut encounter: MessageWriter<EncounterRequested>,
    mut sfx: MessageWriter<SfxRequest>,
) {
    let Some(assets) = dungeon_assets else {
        return;
    };
    let floor_handle = floor_handle_for(&assets, active_floor.0);
    let Some(floor) = floors.get(floor_handle) else {
        return;
    };
    for ev in moved.read() {
        let cell = &floor.features[ev.to.y as usize][ev.to.x as usize];
        if !matches!(cell.trap, Some(TrapType::Alarm)) {
            continue;
        }
        info!("Alarm trap triggered at {:?} — encounter requested", ev.to);
        encounter.write(EncounterRequested {
            source: EncounterSource::AlarmTrap,
        });
        sfx.write(SfxRequest {
            kind: SfxKind::EncounterSting,
        });
    }
}

/// Teleporter — same-floor mutate in place; cross-floor emit `TeleportRequested`.
///
/// NOTE: The plan called for re-publishing `MovedEvent` after same-floor teleport
/// so the minimap marks the destination cell in the same frame. However,
/// `MessageWriter<MovedEvent>` (exclusive access) conflicts with the other systems'
/// `MessageReader<MovedEvent>` (shared access) under Bevy's B0002 conflict rule.
/// MinDev fix: omit the re-publish; the destination cell is marked on the player's
/// NEXT move instead. Documented in Implementation Discoveries D-I3.
fn apply_teleporter(
    mut moved: MessageReader<MovedEvent>,
    floors: Res<Assets<DungeonFloor>>,
    dungeon_assets: Option<Res<DungeonAssets>>,
    active_floor: Res<ActiveFloorNumber>,
    mut party: Query<(&mut GridPosition, &mut Facing, &mut Transform), With<PlayerParty>>,
    mut teleport: MessageWriter<TeleportRequested>,
    mut sfx: MessageWriter<SfxRequest>,
) {
    let Some(assets) = dungeon_assets else {
        return;
    };
    let floor_handle = floor_handle_for(&assets, active_floor.0);
    let Some(floor) = floors.get(floor_handle) else {
        return;
    };
    let Ok((mut pos, mut facing, mut transform)) = party.single_mut() else {
        return;
    };
    for ev in moved.read() {
        let cell = &floor.features[ev.to.y as usize][ev.to.x as usize];
        let Some(target) = &cell.teleporter else {
            continue;
        };
        if target.floor == floor.floor_number {
            // Same-floor: mutate in place.
            pos.x = target.x;
            pos.y = target.y;
            if let Some(new_facing) = target.facing {
                facing.0 = new_facing;
                // Snap rotation to match the new facing — same reason as
                // apply_spinner. Without this, the camera stays pointed at
                // the old facing while logic moves to the new direction.
                transform.rotation = facing_to_quat(new_facing);
            }
            // Snap visual transform to new world position (CELL_SIZE = 2.0).
            transform.translation = Vec3::new(target.x as f32 * 2.0, 0.0, target.y as f32 * 2.0);
            // Note: MovedEvent re-publish removed to avoid B0002 conflict.
            // Minimap marks destination on the player's next move instead.
        } else {
            // Cross-floor: request via state-machine (D3-α).
            teleport.write(TeleportRequested {
                target: target.clone(),
            });
        }
        sfx.write(SfxRequest {
            kind: SfxKind::Door,
        }); // placeholder (D10-A reuse)
    }
}

/// Spinner — pick a random direction (D14 fallback: `Time::elapsed_secs_f64`
/// modulo), avoiding no-op spin. Snaps `Transform.rotation` to the new
/// facing AND attaches `ScreenWobble` (D6-A) for the camera-shake feedback.
///
/// The rotation snap is essential: spinner only mutates `Facing`, but
/// nothing else syncs `Transform.rotation` to the new facing on its own
/// (Q/E does it via `MovementAnimation::rotate`; W/A/S/D leaves rotation
/// unchanged). Without this snap, after the wobble ends the camera is
/// pointed at the OLD facing while the player is logically on the new one.
#[allow(clippy::too_many_arguments)]
fn apply_spinner(
    mut moved: MessageReader<MovedEvent>,
    floors: Res<Assets<DungeonFloor>>,
    dungeon_assets: Option<Res<DungeonAssets>>,
    active_floor: Res<ActiveFloorNumber>,
    mut party: Query<(Entity, &mut Facing, &mut Transform), With<PlayerParty>>,
    mut commands: Commands,
    mut sfx: MessageWriter<SfxRequest>,
    time: Res<Time>,
) {
    let Some(assets) = dungeon_assets else {
        return;
    };
    let floor_handle = floor_handle_for(&assets, active_floor.0);
    let Some(floor) = floors.get(floor_handle) else {
        return;
    };
    let Ok((entity, mut facing, mut transform)) = party.single_mut() else {
        return;
    };
    for ev in moved.read() {
        let cell = &floor.features[ev.to.y as usize][ev.to.x as usize];
        if !cell.spinner {
            continue;
        }
        // D14 fallback: deterministic modulo (rand absent; D14-B).
        let dirs = [
            Direction::North,
            Direction::South,
            Direction::East,
            Direction::West,
        ];
        let idx = (time.elapsed_secs_f64() * 1000.0) as usize % 4;
        // Avoid no-op spin: pick the next direction if same as current.
        let new_dir = if dirs[idx] == facing.0 {
            dirs[(idx + 1) % 4]
        } else {
            dirs[idx]
        };
        facing.0 = new_dir;
        let canonical_rotation = facing_to_quat(new_dir);
        transform.rotation = canonical_rotation;
        sfx.write(SfxRequest {
            kind: SfxKind::SpinnerWhoosh,
        });
        commands.entity(entity).insert(ScreenWobble {
            elapsed_secs: 0.0,
            duration_secs: 0.4, // doubled — user feedback "wobble should be stronger"
            amplitude: 0.35,    // radians (~20°) — was 0.15 (~8.6°), too subtle
            base_rotation: canonical_rotation,
        });
    }
}

/// Tick the screen-wobble animation. Damped sine: `amplitude × sin(8πt) × (1 − t)`.
/// Runs `.after(animate_movement)` so the wobble overrides any rotation
/// `animate_movement` set this frame (e.g., from `MovementAnimation::translate`'s
/// "preserve from rotation" semantics, which would otherwise lock a wobbled
/// rotation into the camera and never recover).
///
/// Sets `transform.rotation` ABSOLUTELY each frame as `base_rotation × jitter`.
/// When the envelope reaches zero (t = 1), `jitter = 0` and rotation is
/// exactly `base_rotation` — guaranteed return to the canonical facing.
fn tick_screen_wobble(
    mut commands: Commands,
    time: Res<Time>,
    mut q: Query<(Entity, &mut Transform, &mut ScreenWobble)>,
) {
    for (entity, mut transform, mut wobble) in &mut q {
        wobble.elapsed_secs += time.delta_secs();
        let t = (wobble.elapsed_secs / wobble.duration_secs).clamp(0.0, 1.0);
        let envelope = (1.0 - t).max(0.0);
        let oscillation = (8.0 * std::f32::consts::PI * t).sin();
        let jitter = wobble.amplitude * envelope * oscillation;
        transform.rotation = wobble.base_rotation * Quat::from_rotation_z(jitter);
        if t >= 1.0 {
            commands.entity(entity).remove::<ScreenWobble>();
        }
    }
}

/// Anti-magic zone — add/remove `AntiMagicZone` marker component on enter/exit.
/// Future #14/#15 spell-casting systems query this marker to gate spells.
///
/// OQ5 note: spawning at an anti-magic entry_point won't add the marker until
/// the first move. Designer convention: don't place entry_point in anti-magic zones.
fn apply_anti_magic_zone(
    mut moved: MessageReader<MovedEvent>,
    floors: Res<Assets<DungeonFloor>>,
    dungeon_assets: Option<Res<DungeonAssets>>,
    active_floor: Res<ActiveFloorNumber>,
    party: Query<Entity, With<PlayerParty>>,
    has_zone: Query<(), With<AntiMagicZone>>,
    mut commands: Commands,
) {
    let Some(assets) = dungeon_assets else {
        return;
    };
    let floor_handle = floor_handle_for(&assets, active_floor.0);
    let Some(floor) = floors.get(floor_handle) else {
        return;
    };
    let Ok(entity) = party.single() else {
        return;
    };
    for ev in moved.read() {
        let cell = &floor.features[ev.to.y as usize][ev.to.x as usize];
        let in_zone = cell.anti_magic_zone;
        let currently_marked = has_zone.contains(entity);
        if in_zone && !currently_marked {
            commands.entity(entity).insert(AntiMagicZone);
            info!("Entered anti-magic zone at {:?}", ev.to);
        } else if !in_zone && currently_marked {
            commands.entity(entity).remove::<AntiMagicZone>();
            info!("Left anti-magic zone (now at {:?})", ev.to);
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pit_trap_subtracts_damage_saturating() {
        let mut hp = 3u32;
        let damage = 5u32;
        hp = hp.saturating_sub(damage);
        assert_eq!(hp, 0, "saturating_sub clamps to 0; no underflow wraparound");
    }

    #[test]
    fn door_state_default_is_closed() {
        assert_eq!(DoorState::default(), DoorState::Closed);
    }

    #[test]
    fn door_states_resource_round_trip() {
        let mut states = DoorStates::default();
        let key = (GridPosition { x: 3, y: 1 }, Direction::East);
        states.doors.insert(key, DoorState::Open);
        assert_eq!(states.doors.get(&key).copied(), Some(DoorState::Open));
    }

    #[test]
    fn locked_doors_clear_idempotent() {
        let mut locked = LockedDoors::default();
        let key = (GridPosition { x: 0, y: 0 }, Direction::North);
        locked.by_edge.insert(key, "x".into());
        locked.by_edge.clear();
        locked.by_edge.insert(key, "x".into());
        assert_eq!(
            locked.by_edge.len(),
            1,
            "clear-first guarantees idempotence"
        );
    }
}

// ---------------------------------------------------------------------------
// Layer-2 app-driven tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod app_tests {
    use super::*;
    use crate::data::dungeon::{CellFeatures, LightingConfig, WallMask};
    use crate::plugins::dungeon::{DungeonPlugin, PlayerParty};
    use crate::plugins::loading::DungeonAssets;
    use crate::plugins::party::{
        DerivedStats, PartyMember, PartyMemberBundle, PartyPlugin, StatusEffectType, StatusEffects,
    };
    use crate::plugins::state::StatePlugin;
    use bevy::asset::AssetPlugin;
    use bevy::input::InputPlugin;
    use bevy::state::app::StatesPlugin;

    /// Build a minimal test app with DungeonPlugin + CellFeaturesPlugin + PartyPlugin.
    /// Mirrors dungeon/tests.rs::make_test_app() but adds CellFeaturesPlugin and PartyPlugin.
    ///
    /// `ActionState<DungeonAction>` is inserted as a bare resource WITHOUT
    /// `ActionsPlugin` — the included `InputManagerPlugin` would tick the
    /// state in `PreUpdate` and clear `just_pressed` before our systems read
    /// it (same pattern as minimap.rs:580 — "ActionState alone — no
    /// InputManagerPlugin tick eats just_pressed").
    fn make_test_app() -> App {
        use crate::plugins::input::DungeonAction;
        use leafwing_input_manager::prelude::ActionState;

        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            StatesPlugin,
            InputPlugin,
            StatePlugin,
            DungeonPlugin,
            CellFeaturesPlugin,
            crate::plugins::combat::CombatPlugin,
            PartyPlugin,
        ));
        // ActionState<DungeonAction> read by DungeonPlugin::handle_dungeon_input
        // and CellFeaturesPlugin::handle_door_interact. Inserted without
        // InputManagerPlugin so .press() in tests is observable in the Update
        // schedule of the same frame.
        app.init_resource::<ActionState<DungeonAction>>();
        // DungeonFloor asset type needed for floor handle lookups.
        app.init_asset::<DungeonFloor>();
        // ItemDb needed by PartyPlugin's populate_item_handle_registry (runs on OnExit(Loading)).
        app.init_asset::<crate::data::ItemDb>();
        // EncounterTable needed by EncounterPlugin (inside CombatPlugin) for handle_encounter_request.
        app.init_asset::<crate::data::EncounterTable>(); // Feature #16
        // EnemyDb needed by handle_encounter_request (runs in Dungeon state). Feature #17.
        app.init_asset::<crate::data::EnemyDb>(); // Feature #17
        // Mesh + StandardMaterial needed by DungeonPlugin's spawn_dungeon_geometry.
        app.init_asset::<bevy::prelude::Mesh>();
        app.init_asset::<bevy::pbr::StandardMaterial>();
        // Image + TextureAtlasLayout needed by bevy_sprite3d's bundle_builder (EnemyRenderPlugin via CombatPlugin).
        app.init_asset::<bevy::image::Image>();
        app.init_asset::<bevy::image::TextureAtlasLayout>();
        // SfxRequest messages: written by CellFeaturesPlugin but registered by AudioPlugin.
        // Explicit registration required in tests (same pattern as dungeon/tests.rs:171).
        app.add_message::<SfxRequest>();
        // StatePlugin under --features dev registers cycle_game_state_on_f9 which needs
        // ButtonInput<KeyCode>. Third-feature gotcha confirmed across #2/#5/#6/#13.
        #[cfg(feature = "dev")]
        app.init_resource::<bevy::input::ButtonInput<bevy::prelude::KeyCode>>();
        app
    }

    /// Build a minimal 2×2 open DungeonFloor, injecting the given CellFeatures at (1,1).
    fn make_floor_with_feature(feature: CellFeatures) -> DungeonFloor {
        DungeonFloor {
            name: "test".into(),
            width: 2,
            height: 2,
            floor_number: 1,
            walls: vec![vec![WallMask::default(); 2]; 2],
            features: vec![
                vec![CellFeatures::default(), CellFeatures::default()],
                vec![CellFeatures::default(), feature],
            ],
            entry_point: (0, 0, Direction::North),
            encounter_table: "test_table".into(),
            lighting: LightingConfig::default(),
            locked_doors: Vec::new(),
        }
    }

    /// Insert a DungeonFloor into the app and set DungeonAssets pointing to it.
    fn insert_test_floor(app: &mut App, floor: DungeonFloor) -> Handle<DungeonFloor> {
        let handle = app
            .world_mut()
            .resource_mut::<Assets<DungeonFloor>>()
            .add(floor);
        app.world_mut().insert_resource(DungeonAssets {
            floor_01: handle.clone(),
            floor_02: Handle::default(),
            encounters_floor_01: Handle::default(), // Feature #16
            item_db: Handle::default(),
            enemy_db: Handle::default(),
            class_table: Handle::default(),
            spell_table: Handle::default(),
        });
        handle
    }

    /// Write a MovedEvent directly into the Messages resource (bypasses DungeonPlugin input).
    fn write_moved(app: &mut App, to: GridPosition) {
        app.world_mut()
            .resource_mut::<bevy::ecs::message::Messages<MovedEvent>>()
            .write(MovedEvent {
                from: GridPosition { x: 0, y: 0 },
                to,
                facing: Direction::North,
            });
    }

    /// Transition the app into GameState::Dungeon (required for `.run_if(in_state(Dungeon))`).
    /// Call BEFORE inserting test floors/entities so OnEnter systems fire without assets
    /// (they early-return when DungeonAssets is absent). Then insert floors and entities.
    /// Mirrors dungeon/tests.rs::advance_into_dungeon.
    fn advance_into_dungeon(app: &mut App) {
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Dungeon);
        app.update(); // StateTransition schedule realizes the new state
        app.update(); // OnEnter(Dungeon) systems run (early-return without assets)
    }

    // --- pit_trap_damages_party ---

    #[test]
    fn pit_trap_damages_party() {
        use crate::data::dungeon::TrapType;

        let feature = CellFeatures {
            trap: Some(TrapType::Pit {
                damage: 5,
                target_floor: None,
            }),
            ..Default::default()
        };
        let mut app = make_test_app();

        // Spawn party members BEFORE advance_into_dungeon so that
        // spawn_default_debug_party (--features dev) sees existing members and
        // skips, keeping the test roster isolated (exactly 4 members, HP=10).
        for _ in 0..4 {
            let mut bundle = PartyMemberBundle::default();
            bundle.derived_stats.current_hp = 10;
            app.world_mut().spawn(bundle);
        }

        advance_into_dungeon(&mut app);
        insert_test_floor(&mut app, make_floor_with_feature(feature));

        app.world_mut().spawn((
            PlayerParty,
            GridPosition { x: 1, y: 1 },
            Facing(Direction::North),
            Transform::default(),
        ));

        write_moved(&mut app, GridPosition { x: 1, y: 1 });
        app.update();

        // All party members should have HP reduced from 10 to 5.
        let hps: Vec<u32> = app
            .world_mut()
            .query_filtered::<&DerivedStats, With<PartyMember>>()
            .iter(app.world())
            .map(|d| d.current_hp)
            .collect();
        assert!(
            !hps.is_empty(),
            "party members should exist after pit trap test"
        );
        for hp in &hps {
            assert_eq!(*hp, 5, "pit trap should subtract 5 damage from each member");
        }
    }

    // --- pit_trap_with_target_floor_requests_teleport ---

    #[test]
    fn pit_trap_with_target_floor_requests_teleport() {
        use crate::data::dungeon::TrapType;

        let feature = CellFeatures {
            trap: Some(TrapType::Pit {
                damage: 1,
                target_floor: Some(2),
            }),
            ..Default::default()
        };
        let mut app = make_test_app();
        advance_into_dungeon(&mut app);
        insert_test_floor(&mut app, make_floor_with_feature(feature));

        for _ in 0..4 {
            app.world_mut().spawn(PartyMemberBundle::default());
        }
        app.world_mut().spawn((
            PlayerParty,
            GridPosition { x: 1, y: 1 },
            Facing(Direction::North),
            Transform::default(),
        ));

        write_moved(&mut app, GridPosition { x: 1, y: 1 });
        app.update();

        let count = app
            .world()
            .resource::<bevy::ecs::message::Messages<TeleportRequested>>()
            .iter_current_update_messages()
            .count();
        assert_eq!(
            count, 1,
            "pit trap with target_floor should emit exactly one TeleportRequested"
        );
        let req = app
            .world()
            .resource::<bevy::ecs::message::Messages<TeleportRequested>>()
            .iter_current_update_messages()
            .next()
            .unwrap();
        assert_eq!(
            req.target.floor, 2,
            "TeleportRequested target floor should be 2"
        );
    }

    // --- poison_trap_applies_status ---

    #[test]
    fn poison_trap_applies_status() {
        use crate::data::dungeon::TrapType;

        let feature = CellFeatures {
            trap: Some(TrapType::Poison),
            ..Default::default()
        };
        let mut app = make_test_app();
        advance_into_dungeon(&mut app);
        insert_test_floor(&mut app, make_floor_with_feature(feature));

        for _ in 0..4 {
            app.world_mut().spawn(PartyMemberBundle::default());
        }
        app.world_mut().spawn((
            PlayerParty,
            GridPosition { x: 1, y: 1 },
            Facing(Direction::North),
            Transform::default(),
        ));

        write_moved(&mut app, GridPosition { x: 1, y: 1 });
        app.update(); // apply_poison_trap writes ApplyStatusEvent
        app.update(); // apply_status_handler reads it and pushes to StatusEffects

        let poison_count: usize = app
            .world_mut()
            .query_filtered::<&StatusEffects, With<PartyMember>>()
            .iter(app.world())
            .map(|se| {
                se.effects
                    .iter()
                    .filter(|e| e.effect_type == StatusEffectType::Poison)
                    .count()
            })
            .sum();
        assert!(
            poison_count > 0,
            "poison trap should apply Poison status to at least one party member"
        );
    }

    // --- alarm_trap_publishes_encounter ---

    #[test]
    fn alarm_trap_publishes_encounter() {
        use crate::data::dungeon::TrapType;

        let feature = CellFeatures {
            trap: Some(TrapType::Alarm),
            ..Default::default()
        };
        let mut app = make_test_app();
        advance_into_dungeon(&mut app);
        insert_test_floor(&mut app, make_floor_with_feature(feature));

        app.world_mut().spawn((
            PlayerParty,
            GridPosition { x: 1, y: 1 },
            Facing(Direction::North),
            Transform::default(),
        ));

        write_moved(&mut app, GridPosition { x: 1, y: 1 });
        app.update();

        let count = app
            .world()
            .resource::<bevy::ecs::message::Messages<EncounterRequested>>()
            .iter_current_update_messages()
            .count();
        assert_eq!(
            count, 1,
            "alarm trap should publish exactly one EncounterRequested"
        );
        let req = app
            .world()
            .resource::<bevy::ecs::message::Messages<EncounterRequested>>()
            .iter_current_update_messages()
            .next()
            .unwrap();
        assert_eq!(
            req.source,
            EncounterSource::AlarmTrap,
            "encounter source should be AlarmTrap"
        );
    }

    // --- same_floor_teleport_mutates_in_place ---

    #[test]
    fn same_floor_teleport_mutates_in_place() {
        use crate::data::dungeon::TeleportTarget;

        // Build a 3x3 floor with a same-floor teleporter at (1,1) targeting (2,2).
        let feature = CellFeatures {
            teleporter: Some(TeleportTarget {
                floor: 1, // same floor
                x: 2,
                y: 2,
                facing: Some(Direction::South),
            }),
            ..Default::default()
        };
        let floor = DungeonFloor {
            name: "test".into(),
            width: 3,
            height: 3,
            floor_number: 1,
            walls: vec![vec![WallMask::default(); 3]; 3],
            features: vec![
                vec![CellFeatures::default(); 3],
                vec![CellFeatures::default(), feature, CellFeatures::default()],
                vec![CellFeatures::default(); 3],
            ],
            entry_point: (0, 0, Direction::North),
            encounter_table: "test_table".into(),
            lighting: LightingConfig::default(),
            locked_doors: Vec::new(),
        };
        let mut app = make_test_app();
        advance_into_dungeon(&mut app);
        insert_test_floor(&mut app, floor);

        let party = app
            .world_mut()
            .spawn((
                PlayerParty,
                GridPosition { x: 0, y: 0 },
                Facing(Direction::North),
                Transform::default(),
            ))
            .id();

        // Write a MovedEvent targeting the teleporter cell at (1,1).
        app.world_mut()
            .resource_mut::<bevy::ecs::message::Messages<MovedEvent>>()
            .write(MovedEvent {
                from: GridPosition { x: 0, y: 0 },
                to: GridPosition { x: 1, y: 1 },
                facing: Direction::North,
            });
        app.update();

        let pos = *app.world().entity(party).get::<GridPosition>().unwrap();
        let facing = *app.world().entity(party).get::<Facing>().unwrap();
        assert_eq!(
            pos,
            GridPosition { x: 2, y: 2 },
            "teleporter should mutate GridPosition to destination"
        );
        assert_eq!(
            facing.0,
            Direction::South,
            "teleporter should update facing when target.facing is Some"
        );
    }

    // --- spinner_randomizes_facing_and_attaches_wobble ---

    #[test]
    fn spinner_randomizes_facing_and_attaches_wobble() {
        let feature = CellFeatures {
            spinner: true,
            ..Default::default()
        };
        let mut app = make_test_app();
        advance_into_dungeon(&mut app);
        insert_test_floor(&mut app, make_floor_with_feature(feature));

        let party = app
            .world_mut()
            .spawn((
                PlayerParty,
                GridPosition { x: 1, y: 1 },
                Facing(Direction::North),
                Transform::default(),
            ))
            .id();

        write_moved(&mut app, GridPosition { x: 1, y: 1 });
        app.update();

        // Facing must have changed (spinner changes it to a non-no-op direction).
        let facing = app.world().entity(party).get::<Facing>().unwrap().0;
        assert_ne!(
            facing,
            Direction::North,
            "spinner must change facing (no-op spin avoided)"
        );
        // ScreenWobble component must be attached.
        assert!(
            app.world().entity(party).contains::<ScreenWobble>(),
            "spinner should attach ScreenWobble component"
        );
    }

    // --- anti_magic_zone_lifecycle ---

    #[test]
    fn anti_magic_zone_lifecycle() {
        // Build a 2x2 floor with anti_magic_zone at (1,0).
        let floor = DungeonFloor {
            name: "test".into(),
            width: 2,
            height: 2,
            floor_number: 1,
            walls: vec![vec![WallMask::default(); 2]; 2],
            features: vec![
                vec![
                    CellFeatures::default(),
                    CellFeatures {
                        anti_magic_zone: true,
                        ..Default::default()
                    },
                ],
                vec![CellFeatures::default(), CellFeatures::default()],
            ],
            entry_point: (0, 1, Direction::North),
            encounter_table: "test_table".into(),
            lighting: LightingConfig::default(),
            locked_doors: Vec::new(),
        };
        let mut app = make_test_app();
        advance_into_dungeon(&mut app);
        insert_test_floor(&mut app, floor);

        let party = app
            .world_mut()
            .spawn((
                PlayerParty,
                GridPosition { x: 0, y: 1 },
                Facing(Direction::North),
                Transform::default(),
            ))
            .id();

        // Step 1: move INTO anti_magic_zone at (1,0).
        app.world_mut()
            .resource_mut::<bevy::ecs::message::Messages<MovedEvent>>()
            .write(MovedEvent {
                from: GridPosition { x: 0, y: 1 },
                to: GridPosition { x: 1, y: 0 },
                facing: Direction::North,
            });
        app.update();

        assert!(
            app.world().entity(party).contains::<AntiMagicZone>(),
            "AntiMagicZone component should be added on entry"
        );

        // Step 2: move OUT of anti_magic_zone back to (0,0).
        app.world_mut()
            .resource_mut::<bevy::ecs::message::Messages<MovedEvent>>()
            .write(MovedEvent {
                from: GridPosition { x: 1, y: 0 },
                to: GridPosition { x: 0, y: 0 },
                facing: Direction::West,
            });
        app.update();

        assert!(
            !app.world().entity(party).contains::<AntiMagicZone>(),
            "AntiMagicZone component should be removed on exit"
        );
    }

    // --- door_interact_toggles_closed_to_open ---

    #[test]
    fn door_interact_toggles_closed_to_open() {
        use crate::data::dungeon::WallType;
        use crate::plugins::input::DungeonAction;
        use leafwing_input_manager::prelude::ActionState;

        // 2x2 floor with WallType::Door east of (0, 0).
        let mut floor = DungeonFloor {
            name: "test".into(),
            width: 2,
            height: 2,
            floor_number: 1,
            walls: vec![vec![WallMask::default(); 2]; 2],
            features: vec![vec![CellFeatures::default(); 2]; 2],
            entry_point: (0, 0, Direction::East),
            encounter_table: "test_table".into(),
            lighting: LightingConfig::default(),
            locked_doors: Vec::new(),
        };
        floor.walls[0][0].east = WallType::Door;

        let mut app = make_test_app();
        advance_into_dungeon(&mut app);
        insert_test_floor(&mut app, floor);

        app.world_mut().spawn((
            PlayerParty,
            GridPosition { x: 0, y: 0 },
            Facing(Direction::East),
            Transform::default(),
        ));

        // Default DoorStates is empty — door starts Closed (Pitfall 4 / D15).
        // Press Interact to toggle.
        app.world_mut()
            .resource_mut::<ActionState<DungeonAction>>()
            .press(&DungeonAction::Interact);
        app.update();

        let key = (GridPosition { x: 0, y: 0 }, Direction::East);
        assert_eq!(
            app.world()
                .resource::<DoorStates>()
                .doors
                .get(&key)
                .copied(),
            Some(DoorState::Open),
            "Interact press on Door should toggle Closed → Open"
        );

        // Release Interact, press again — should toggle Open → Closed.
        app.world_mut()
            .resource_mut::<ActionState<DungeonAction>>()
            .release(&DungeonAction::Interact);
        app.update();
        app.world_mut()
            .resource_mut::<ActionState<DungeonAction>>()
            .press(&DungeonAction::Interact);
        app.update();

        assert_eq!(
            app.world()
                .resource::<DoorStates>()
                .doors
                .get(&key)
                .copied(),
            Some(DoorState::Closed),
            "Second Interact press on Door should toggle Open → Closed"
        );
    }

    // --- door_interact_at_floor_01_coords_full_flow ---

    /// Reproduces the user-reported production scenario at floor_01 coords:
    /// player at (1, 1) facing East presses Interact, then can walk through.
    /// This is the cell + direction the floor_01.dungeon.ron Door is authored at.
    #[test]
    fn door_interact_at_floor_01_coords_full_flow() {
        use crate::data::dungeon::WallType;
        use crate::plugins::dungeon::features::{DoorState, DoorStates};
        use crate::plugins::input::DungeonAction;
        use leafwing_input_manager::prelude::ActionState;

        // Mirror floor_01.dungeon.ron: 6×6, walls[1][1].east = Door.
        let mut floor = DungeonFloor {
            name: "test".into(),
            width: 6,
            height: 6,
            floor_number: 1,
            walls: vec![vec![WallMask::default(); 6]; 6],
            features: vec![vec![CellFeatures::default(); 6]; 6],
            entry_point: (1, 1, Direction::East),
            encounter_table: "test_table".into(),
            lighting: LightingConfig::default(),
            locked_doors: Vec::new(),
        };
        floor.walls[1][1].east = WallType::Door;
        floor.walls[1][2].west = WallType::Door; // razor-wall mirror

        let mut app = make_test_app();
        advance_into_dungeon(&mut app);
        insert_test_floor(&mut app, floor);

        app.world_mut().spawn((
            PlayerParty,
            GridPosition { x: 1, y: 1 },
            Facing(Direction::East),
            Transform::default(),
        ));

        // (1) Pre-condition: door is Closed by default — passability blocked.
        // (Re-read the floor by handle so we can call can_move_with_doors directly.)
        let floor_handle = app
            .world()
            .resource::<crate::plugins::loading::DungeonAssets>()
            .floor_01
            .clone();
        let pre_passable = {
            let floor = app
                .world()
                .resource::<Assets<DungeonFloor>>()
                .get(&floor_handle)
                .unwrap();
            let door_states = app.world().resource::<DoorStates>();
            crate::plugins::dungeon::can_move_with_doors(
                floor,
                door_states,
                GridPosition { x: 1, y: 1 },
                Direction::East,
            )
        };
        assert!(
            !pre_passable,
            "default-Closed Door at (1,1) east must block movement"
        );

        // (2) Press Interact (F) — toggles Closed → Open.
        app.world_mut()
            .resource_mut::<ActionState<DungeonAction>>()
            .press(&DungeonAction::Interact);
        app.update();

        let key = (GridPosition { x: 1, y: 1 }, Direction::East);
        assert_eq!(
            app.world()
                .resource::<DoorStates>()
                .doors
                .get(&key)
                .copied(),
            Some(DoorState::Open),
            "F press at (1,1) facing East should set DoorState to Open"
        );

        // (3) Post-condition: door now passable, player can move east.
        let post_passable = {
            let floor = app
                .world()
                .resource::<Assets<DungeonFloor>>()
                .get(&floor_handle)
                .unwrap();
            let door_states = app.world().resource::<DoorStates>();
            crate::plugins::dungeon::can_move_with_doors(
                floor,
                door_states,
                GridPosition { x: 1, y: 1 },
                Direction::East,
            )
        };
        assert!(
            post_passable,
            "after F press, Door at (1,1) east must be passable"
        );
    }

    // --- cross_floor_teleport_publishes_request ---

    #[test]
    fn cross_floor_teleport_publishes_request() {
        use crate::data::dungeon::TeleportTarget;

        // Build a 2x2 floor with a cross-floor teleporter at (1,1) targeting floor 2.
        let feature = CellFeatures {
            teleporter: Some(TeleportTarget {
                floor: 2, // CROSS-FLOOR
                x: 1,
                y: 1,
                facing: Some(Direction::South),
            }),
            ..Default::default()
        };
        let mut app = make_test_app();
        advance_into_dungeon(&mut app);
        insert_test_floor(&mut app, make_floor_with_feature(feature));

        app.world_mut().spawn((
            PlayerParty,
            GridPosition { x: 0, y: 0 },
            Facing(Direction::North),
            Transform::default(),
        ));

        write_moved(&mut app, GridPosition { x: 1, y: 1 });
        app.update();

        let count = app
            .world()
            .resource::<bevy::ecs::message::Messages<TeleportRequested>>()
            .iter_current_update_messages()
            .count();
        assert_eq!(
            count, 1,
            "cross-floor teleporter should emit exactly one TeleportRequested"
        );
        let req = app
            .world()
            .resource::<bevy::ecs::message::Messages<TeleportRequested>>()
            .iter_current_update_messages()
            .next()
            .unwrap();
        assert_eq!(
            req.target.floor, 2,
            "TeleportRequested target should be floor 2"
        );
    }
}
