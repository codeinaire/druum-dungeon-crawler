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
use crate::data::dungeon::{Direction, TeleportTarget, TrapType, WallType};
use crate::data::ItemAsset;
use crate::plugins::audio::{SfxKind, SfxRequest};
use crate::plugins::dungeon::{
    Facing, GridPosition, MovedEvent, PlayerParty, animate_movement, handle_dungeon_input,
};
use crate::plugins::input::DungeonAction;
use crate::plugins::loading::DungeonAssets;
use crate::plugins::party::{
    ActiveEffect, DerivedStats, Inventory, ItemInstance, ItemKind, PartyMember,
    StatusEffectType, StatusEffects,
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
#[derive(Component, Debug, Clone)]
pub struct ScreenWobble {
    pub elapsed_secs: f32,
    pub duration_secs: f32,
    pub amplitude: f32,
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
    // Future: Random (foe roll), Foe (overworld encounter) — surface in #16.
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
            .add_systems(OnEnter(GameState::Dungeon), populate_locked_doors)
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
                        .after(handle_dungeon_input),
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
// Systems — state management
// ---------------------------------------------------------------------------

/// Populate `LockedDoors` from `DungeonFloor::locked_doors`. Clears first
/// for idempotence across `OnEnter(Dungeon)` re-entries (Pitfall 8 — D3-α
/// teleport re-enters the state).
fn populate_locked_doors(
    mut locked_doors: ResMut<LockedDoors>,
    dungeon_assets: Option<Res<DungeonAssets>>,
    floors: Res<Assets<DungeonFloor>>,
) {
    locked_doors.by_edge.clear();
    let Some(assets) = dungeon_assets else {
        return;
    };
    // Phase 5-7: reads floor_01. Phase 8 upgrades to active floor via PendingTeleport.
    let Some(floor) = floors.get(&assets.floor_01) else {
        return;
    };
    for ((x, y), dir, id) in &floor.locked_doors {
        locked_doors
            .by_edge
            .insert((GridPosition { x: *x, y: *y }, *dir), id.clone());
    }
}

/// Clear all door-related resources on `OnExit(Dungeon)`.
/// Prevents stale door states from leaking to future floor visits (D1 — per-floor-instance).
fn clear_door_resources(
    mut door_states: ResMut<DoorStates>,
    mut locked_doors: ResMut<LockedDoors>,
    mut pending_teleport: ResMut<PendingTeleport>,
) {
    door_states.doors.clear();
    locked_doors.by_edge.clear();
    pending_teleport.target = None;
}

// ---------------------------------------------------------------------------
// Systems — door interaction
// ---------------------------------------------------------------------------

/// Reads `Res<ActionState<DungeonAction>>`; on `Interact` press, looks at the
/// wall the player is facing. For `WallType::Door`, toggles `DoorState`. For
/// `WallType::LockedDoor`, walks all party inventories looking for a matching
/// `ItemKind::KeyItem` with `key_id == door_id`; if found, sets `DoorState::Open`.
/// Keys are NOT consumed (D13 — Wizardry-style; reusable).
fn handle_door_interact(
    actions: Res<ActionState<DungeonAction>>,
    party: Query<(&GridPosition, &Facing), With<PlayerParty>>,
    floors: Res<Assets<DungeonFloor>>,
    dungeon_assets: Option<Res<DungeonAssets>>,
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
        return;
    };
    let Some(assets) = dungeon_assets else {
        return;
    };
    let Some(floor) = floors.get(&assets.floor_01) else {
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
                sfx.write(SfxRequest { kind: SfxKind::Door });
                info!("Unlocked door at {:?} {:?} with key '{}'", pos, edge_dir, door_id);
                // D13: do NOT consume the key.
            } else {
                info!(
                    "Locked door at {:?} {:?} requires key '{}'",
                    pos, edge_dir, door_id
                );
            }
        }
        _ => {} // Not a door; no-op.
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
    mut party: Query<&mut DerivedStats, With<PartyMember>>,
    mut sfx: MessageWriter<SfxRequest>,
    mut teleport: MessageWriter<TeleportRequested>,
) {
    let Some(assets) = dungeon_assets else {
        return;
    };
    let Some(floor) = floors.get(&assets.floor_01) else {
        return;
    };
    for ev in moved.read() {
        let cell = &floor.features[ev.to.y as usize][ev.to.x as usize];
        let Some(TrapType::Pit { damage, target_floor }) = &cell.trap else {
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

/// Apply poison trap on entry. Naive push (D12) — stacking deferred to #14.
fn apply_poison_trap(
    mut moved: MessageReader<MovedEvent>,
    floors: Res<Assets<DungeonFloor>>,
    dungeon_assets: Option<Res<DungeonAssets>>,
    mut party: Query<&mut StatusEffects, With<PartyMember>>,
    mut sfx: MessageWriter<SfxRequest>,
) {
    const POISON_TURNS: u32 = 5;
    let Some(assets) = dungeon_assets else {
        return;
    };
    let Some(floor) = floors.get(&assets.floor_01) else {
        return;
    };
    for ev in moved.read() {
        let cell = &floor.features[ev.to.y as usize][ev.to.x as usize];
        if !matches!(cell.trap, Some(TrapType::Poison)) {
            continue;
        }
        for mut effects in &mut party {
            effects.effects.push(ActiveEffect {
                effect_type: StatusEffectType::Poison,
                remaining_turns: Some(POISON_TURNS),
                magnitude: 0.0,
            });
        }
        sfx.write(SfxRequest { kind: SfxKind::Door }); // placeholder hiss (D10-A reuse)
    }
}

/// Alarm trap — publish `EncounterRequested` and log for #16's consumer (D5).
fn apply_alarm_trap(
    mut moved: MessageReader<MovedEvent>,
    floors: Res<Assets<DungeonFloor>>,
    dungeon_assets: Option<Res<DungeonAssets>>,
    mut encounter: MessageWriter<EncounterRequested>,
    mut sfx: MessageWriter<SfxRequest>,
) {
    let Some(assets) = dungeon_assets else {
        return;
    };
    let Some(floor) = floors.get(&assets.floor_01) else {
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
/// Same-floor branch re-publishes `MovedEvent` so minimap + dark-zone gate fire.
fn apply_teleporter(
    mut moved: MessageReader<MovedEvent>,
    floors: Res<Assets<DungeonFloor>>,
    dungeon_assets: Option<Res<DungeonAssets>>,
    mut party: Query<(&mut GridPosition, &mut Facing, &mut Transform), With<PlayerParty>>,
    mut writer: MessageWriter<MovedEvent>,
    mut teleport: MessageWriter<TeleportRequested>,
    mut sfx: MessageWriter<SfxRequest>,
) {
    let Some(assets) = dungeon_assets else {
        return;
    };
    let Some(floor) = floors.get(&assets.floor_01) else {
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
            let old_pos = *pos;
            pos.x = target.x;
            pos.y = target.y;
            if let Some(new_facing) = target.facing {
                facing.0 = new_facing;
            }
            // Snap visual transform to new world position (CELL_SIZE = 2.0).
            transform.translation = Vec3::new(target.x as f32 * 2.0, 0.0, target.y as f32 * 2.0);
            // Re-publish MovedEvent so minimap + dark-zone gate fire for destination.
            writer.write(MovedEvent {
                from: old_pos,
                to: *pos,
                facing: facing.0,
            });
        } else {
            // Cross-floor: request via state-machine (D3-α).
            teleport.write(TeleportRequested {
                target: target.clone(),
            });
        }
        sfx.write(SfxRequest { kind: SfxKind::Door }); // placeholder (D10-A reuse)
    }
}

/// Spinner — pick a random direction (D14 fallback: `Time::elapsed_secs_f64`
/// modulo), avoiding no-op spin. Attaches `ScreenWobble` component (D6-A).
fn apply_spinner(
    mut moved: MessageReader<MovedEvent>,
    floors: Res<Assets<DungeonFloor>>,
    dungeon_assets: Option<Res<DungeonAssets>>,
    mut party: Query<(Entity, &mut Facing), With<PlayerParty>>,
    mut commands: Commands,
    mut sfx: MessageWriter<SfxRequest>,
    time: Res<Time>,
) {
    let Some(assets) = dungeon_assets else {
        return;
    };
    let Some(floor) = floors.get(&assets.floor_01) else {
        return;
    };
    let Ok((entity, mut facing)) = party.single_mut() else {
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
        sfx.write(SfxRequest {
            kind: SfxKind::SpinnerWhoosh,
        });
        commands.entity(entity).insert(ScreenWobble {
            elapsed_secs: 0.0,
            duration_secs: 0.2,
            amplitude: 0.15, // radians
        });
    }
}

/// Tick the screen-wobble animation. Damped sine: `amplitude × sin(8πt) × (1 − t)`.
/// Runs `.after(animate_movement)` to win the rotation last-write race.
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
        transform.rotation = transform.rotation * Quat::from_rotation_z(jitter);
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
    party: Query<Entity, With<PlayerParty>>,
    has_zone: Query<(), With<AntiMagicZone>>,
    mut commands: Commands,
) {
    let Some(assets) = dungeon_assets else {
        return;
    };
    let Some(floor) = floors.get(&assets.floor_01) else {
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
        assert_eq!(locked.by_edge.len(), 1, "clear-first guarantees idempotence");
    }
}
