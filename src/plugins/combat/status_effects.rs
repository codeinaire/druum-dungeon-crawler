//! Status effect resolution layer — Feature #14.
//!
//! Owns:
//!
//! - `ApplyStatusEvent` message (canonical "apply this effect to target").
//! - `StatusTickEvent` message (internal — emitted on every dungeon step;
//!   #15 will add a combat-round emitter).
//! - `apply_status_handler` system (SOLE mutator of `StatusEffects.effects`).
//! - `tick_status_durations` system (decrements `remaining_turns`, removes
//!   expired effects).
//! - `tick_on_dungeon_step` system (fires `StatusTickEvent` on `MovedEvent`).
//! - `apply_poison_damage` and `apply_regen` resolvers (read
//!   `StatusTickEvent`, mutate `DerivedStats.current_hp`).
//! - `pub fn` predicates: `is_paralyzed`, `is_asleep`, `is_silenced`. Used
//!   by #15's `turn_manager` for action gating.
//! - `pub fn check_dead_and_apply`: stub for #15 to call after damage
//!   resolves to apply the `Dead` status.
//!
//! See `project/research/20260507-115500-feature-14-status-effects-system.md`.
//!
//! ## Bevy 0.18 family rename
//!
//! `ApplyStatusEvent` and `StatusTickEvent` derive `Message`, NOT `Event`.
//! Read with `MessageReader<T>`, write with `MessageWriter<T>`. Register with
//! `app.add_message::<T>()`.
//!
//! ## Stacking semantics (D2 of research)
//!
//! `apply_status_handler` is the single mutator. Same effect already present
//! → refresh duration, take `magnitude.max()`. Permanent (Stone/Dead)
//! re-application → no-op. Buff `magnitude` IS the multiplier.
//!
//! ## D5α — re-derive trigger
//!
//! `apply_status_handler` writes `EquipmentChangedEvent` with
//! `slot: EquipSlot::None` (sentinel) for stat-affecting variants
//! (`AttackUp`/`DefenseUp`/`SpeedUp`/`Dead`). This reuses
//! `recompute_derived_stats_on_equipment_change` (`inventory.rs:421`)
//! verbatim — no new recompute path.

use bevy::prelude::*;

use crate::plugins::dungeon::{MovedEvent, handle_dungeon_input};
use crate::plugins::party::{
    ActiveEffect, DerivedStats, EquipSlot, EquipmentChangedEvent, PartyMember, StatusEffectType,
    StatusEffects,
};
use crate::plugins::state::GameState;

// ─────────────────────────────────────────────────────────────────────────────
// Messages
// ─────────────────────────────────────────────────────────────────────────────

/// Canonical "apply this status effect to target" message.
///
/// Every status source — traps (`apply_poison_trap`), enemy spells (#15),
/// items (#20) — writes this. The single `apply_status_handler` system reads
/// it and enforces stacking semantics.
///
/// **Field semantics:**
///
/// - `target`: entity receiving the effect. Typically `PartyMember` in v1.
/// - `effect`: which `StatusEffectType` to apply or refresh.
/// - `potency`: magnitude. For buffs, multiplier (e.g., `0.5` = +50%). For
///   tick effects (Poison/Regen), per-tick magnitude. Clamped to `[0.0, 10.0]`
///   by `apply_status_handler` (Pitfall 6, defensive trust boundary).
/// - `duration`: `Some(n)` for `n` ticks. `None` for permanent (Stone/Dead).
///   For `Stone` and `Dead`, callers MUST pass `None`; the handler does not
///   currently validate this, and passing `Some(n)` would create an unintended
///   temporary petrification/death that `tick_status_durations` removes.
///
/// **No `source` field** (deliberate; YAGNI per research B.1).
///
/// **Every writer of this message must be `.before(apply_status_handler)`** in
/// its plugin's `build` method (same-frame consumability — Risk 1 of #14). See
/// `CellFeaturesPlugin::build` for the cross-plugin pattern.
///
/// `Message`, NOT `Event` — Bevy 0.18 family rename.
#[derive(Message, Clone, Copy, Debug)]
pub struct ApplyStatusEvent {
    pub target: Entity,
    pub effect: StatusEffectType,
    pub potency: f32,
    pub duration: Option<u32>,
}

/// Internal tick message. One per `MovedEvent` per `PartyMember` (D9-A) in
/// #14. #15 will add a `turn_manager::round_end` emitter for combat-round
/// ticks (one line: `for entity in alive_combatants { tick.write(...); }`).
///
/// `Message`, NOT `Event` — Bevy 0.18 family rename.
#[derive(Message, Clone, Copy, Debug)]
pub struct StatusTickEvent {
    pub target: Entity,
}

// ─────────────────────────────────────────────────────────────────────────────
// Plugin
// ─────────────────────────────────────────────────────────────────────────────

/// Owns status-effect systems, messages, and per-effect resolvers for
/// Feature #14. Registered as a sub-plugin of `CombatPlugin` from
/// `combat/mod.rs::CombatPlugin::build`.
pub struct StatusEffectsPlugin;

impl Plugin for StatusEffectsPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<ApplyStatusEvent>()
            .add_message::<StatusTickEvent>()
            .add_systems(
                Update,
                (
                    // Dungeon-step tick — gated; #15 will add a
                    // combat-round emitter without touching this one.
                    tick_on_dungeon_step
                        .run_if(in_state(GameState::Dungeon))
                        .after(handle_dungeon_input)
                        // Emitter-before-readers: same-frame consumability
                        // for the resolvers (Decision 16).
                        .before(tick_status_durations)
                        .before(apply_poison_damage)
                        .before(apply_regen),
                    // Message-driven systems — no state gate (idle when
                    // no events; Pattern 2 of research).
                    apply_status_handler,
                    // Resolvers BEFORE ticker so a duration-1 effect
                    // gets its final tick of damage before being removed
                    // (Critical: resolver-vs-ticker ordering, Open Item 7).
                    apply_poison_damage.before(tick_status_durations),
                    apply_regen.before(tick_status_durations),
                    tick_status_durations,
                ),
            );
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Systems
// ─────────────────────────────────────────────────────────────────────────────

/// Fires `StatusTickEvent` for every `PartyMember` on every dungeon step.
///
/// D9-A (every step) per research recommendation. To switch to D9-B (every
/// Nth step), add a `StepsSinceLastTick: Resource(u32)` and gate on
/// `>= N`. To switch to D9-C (time-based), read `Time::delta` instead.
/// Both are localized changes to this function only.
///
/// Ordered `.after(handle_dungeon_input)` so the player's commit-frame
/// position is the trigger context (mirrors `features.rs:160-187`).
fn tick_on_dungeon_step(
    mut moved: MessageReader<MovedEvent>,
    mut tick: MessageWriter<StatusTickEvent>,
    party: Query<Entity, With<PartyMember>>,
) {
    for _ev in moved.read() {
        for entity in &party {
            tick.write(StatusTickEvent { target: entity });
        }
    }
}

/// The single mutator of `StatusEffects.effects`. Every other system writes
/// `ApplyStatusEvent` rather than pushing directly.
///
/// **Stacking rule (D2 of research):**
///
/// - Same effect already present: refresh `remaining_turns`, take
///   `existing.magnitude.max(new.potency)`.
/// - Permanent (Stone/Dead) already present: no-op.
/// - Otherwise: push a new `ActiveEffect`.
///
/// **D5α: writes `EquipmentChangedEvent`** for `AttackUp`/`DefenseUp`/
/// `SpeedUp`/`Dead` to nudge `derive_stats` to re-run via the existing
/// recompute path.
///
/// **Pitfall 6:** clamps `potency` to `[0.0, 10.0]` at the trust boundary.
pub fn apply_status_handler(
    mut events: MessageReader<ApplyStatusEvent>,
    mut equip_changed: MessageWriter<EquipmentChangedEvent>,
    mut characters: Query<&mut StatusEffects, With<PartyMember>>,
) {
    for ev in events.read() {
        // Pitfall 6: defensive clamp on f32 trust boundary.
        // `f32::clamp` propagates NaN through, so handle non-finite first.
        let potency = if ev.potency.is_finite() {
            ev.potency.clamp(0.0, 10.0)
        } else {
            0.0
        };

        let Ok(mut status) = characters.get_mut(ev.target) else {
            continue;
        };

        // Permanent effects already present: re-application is a no-op.
        if matches!(ev.effect, StatusEffectType::Stone | StatusEffectType::Dead)
            && status.has(ev.effect)
        {
            continue;
        }

        // Stacking merge: refresh duration, take higher magnitude.
        if let Some(existing) = status
            .effects
            .iter_mut()
            .find(|e| e.effect_type == ev.effect)
        {
            existing.remaining_turns = ev.duration;
            existing.magnitude = existing.magnitude.max(potency);
        } else {
            status.effects.push(ActiveEffect {
                effect_type: ev.effect,
                remaining_turns: ev.duration,
                magnitude: potency,
            });
        }

        // D5α: nudge derive_stats re-run for stat-affecting variants.
        // EquipSlot::None is the "stat-changed, source not an equip slot"
        // sentinel. recompute_derived_stats_on_equipment_change reads
        // &StatusEffects and applies the new buff branches automatically.
        if matches!(
            ev.effect,
            StatusEffectType::AttackUp
                | StatusEffectType::DefenseUp
                | StatusEffectType::SpeedUp
                | StatusEffectType::Dead
        ) {
            equip_changed.write(EquipmentChangedEvent {
                character: ev.target,
                slot: EquipSlot::None,
            });
        }
    }
}

/// Decrements `remaining_turns` and removes expired effects.
///
/// `None` (permanent) effects are skipped — they don't tick.
///
/// **D5α:** writes `EquipmentChangedEvent` if a removed effect was a stat-
/// modifier (`AttackUp`/`DefenseUp`/`SpeedUp`) so `derive_stats` re-runs.
fn tick_status_durations(
    mut ticks: MessageReader<StatusTickEvent>,
    mut equip_changed: MessageWriter<EquipmentChangedEvent>,
    mut characters: Query<&mut StatusEffects, With<PartyMember>>,
) {
    for ev in ticks.read() {
        let Ok(mut status) = characters.get_mut(ev.target) else {
            continue;
        };

        let mut had_stat_modifier_removed = false;

        status.effects.retain_mut(|e| match e.remaining_turns {
            None => true, // permanent — keep
            Some(0) => {
                // Expired (defensive — should be caught next-frame normally).
                if matches!(
                    e.effect_type,
                    StatusEffectType::AttackUp
                        | StatusEffectType::DefenseUp
                        | StatusEffectType::SpeedUp
                ) {
                    had_stat_modifier_removed = true;
                }
                false
            }
            Some(ref mut n) => {
                *n -= 1;
                if *n == 0 {
                    if matches!(
                        e.effect_type,
                        StatusEffectType::AttackUp
                            | StatusEffectType::DefenseUp
                            | StatusEffectType::SpeedUp
                    ) {
                        had_stat_modifier_removed = true;
                    }
                    false // becomes 0 → drop
                } else {
                    true
                }
            }
        });

        if had_stat_modifier_removed {
            equip_changed.write(EquipmentChangedEvent {
                character: ev.target,
                slot: EquipSlot::None,
            });
        }
    }
}

/// On every tick, characters with `Poison` take damage proportional to
/// `magnitude` and `max_hp`.
///
/// **D7-A formula (default):** `damage = ((max_hp / 20).max(1) as f32 * magnitude) as u32; damage.max(1)`.
/// At `magnitude = 1.0` this is 5% of max_hp per tick (Wizardry/Etrian
/// canonical). The two `.max(1)` floors guard against:
///
/// - low-HP characters (max_hp < 20 → `max_hp / 20 = 0`).
/// - low-magnitude truncation (`(1 as f32 * 0.4) as u32 = 0`).
///
/// To switch to D7-B (flat): replace the formula with
/// `let damage = (5.0 * mag) as u32; damage.max(1);`.
///
/// **Pitfall 7:** does NOT apply `Dead` on zero HP. #15 owns that;
/// `check_dead_and_apply` is the #15-callable convenience.
fn apply_poison_damage(
    mut ticks: MessageReader<StatusTickEvent>,
    mut characters: Query<(&StatusEffects, &mut DerivedStats), With<PartyMember>>,
) {
    for ev in ticks.read() {
        let Ok((status, mut derived)) = characters.get_mut(ev.target) else {
            continue;
        };
        let Some(poison) = status
            .effects
            .iter()
            .find(|e| e.effect_type == StatusEffectType::Poison)
        else {
            continue;
        };
        let base = (derived.max_hp / 20).max(1);
        let damage = ((base as f32 * poison.magnitude) as u32).max(1);
        derived.current_hp = derived.current_hp.saturating_sub(damage);
    }
}

/// On every tick, characters with `Regen` heal proportional to `magnitude`
/// and `max_hp`. Mirrors `apply_poison_damage` symmetrically.
///
/// **D7-A formula (default, mirroring poison):** `heal = ((max_hp / 20).max(1) as f32 * magnitude) as u32; heal.max(1)`.
/// Capped at `max_hp` via `.min(max_hp)` after `saturating_add`.
fn apply_regen(
    mut ticks: MessageReader<StatusTickEvent>,
    mut characters: Query<(&StatusEffects, &mut DerivedStats), With<PartyMember>>,
) {
    for ev in ticks.read() {
        let Ok((status, mut derived)) = characters.get_mut(ev.target) else {
            continue;
        };
        let Some(regen) = status
            .effects
            .iter()
            .find(|e| e.effect_type == StatusEffectType::Regen)
        else {
            continue;
        };
        let base = (derived.max_hp / 20).max(1);
        let healing = ((base as f32 * regen.magnitude) as u32).max(1);
        derived.current_hp = derived
            .current_hp
            .saturating_add(healing)
            .min(derived.max_hp);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Predicates
// ─────────────────────────────────────────────────────────────────────────────

/// Returns `true` if `Paralysis` is currently active. #15 imports this and
/// calls from `turn_manager::collect_player_actions` to skip the character's
/// turn.
pub fn is_paralyzed(status: &StatusEffects) -> bool {
    status.has(StatusEffectType::Paralysis)
}

/// Returns `true` if `Sleep` is currently active. #15 imports for action gating.
pub fn is_asleep(status: &StatusEffects) -> bool {
    status.has(StatusEffectType::Sleep)
}

/// Returns `true` if `Silence` is currently active. #15 imports for spell-
/// action gating in `turn_manager::collect_player_actions`.
pub fn is_silenced(status: &StatusEffects) -> bool {
    status.has(StatusEffectType::Silence)
}

// ─────────────────────────────────────────────────────────────────────────────
// #15-callable stub
// ─────────────────────────────────────────────────────────────────────────────

/// #15-callable convenience: writes `ApplyStatusEvent { effect: Dead, ... }`
/// when `derived.current_hp == 0`. #14 does NOT auto-apply Dead inside
/// `apply_poison_damage` (Pitfall 7 — defer combat genre rules to #15).
/// #15's combat resolver imports and calls this after damage resolves.
///
/// `target` is the character entity. `derived` is read-only (no mutation).
/// `writer` writes the message; the handler picks it up next frame.
pub fn check_dead_and_apply(
    target: Entity,
    derived: &DerivedStats,
    writer: &mut MessageWriter<ApplyStatusEvent>,
) {
    if derived.current_hp == 0 {
        writer.write(ApplyStatusEvent {
            target,
            effect: StatusEffectType::Dead,
            potency: 1.0,
            duration: None, // permanent
        });
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Predicates ──────────────────────────────────────────────────────────

    #[test]
    fn is_paralyzed_returns_true_when_present() {
        let mut status = StatusEffects::default();
        status.effects.push(ActiveEffect {
            effect_type: StatusEffectType::Paralysis,
            remaining_turns: Some(3),
            magnitude: 0.0,
        });
        assert!(is_paralyzed(&status));
        assert!(!is_asleep(&status));
        assert!(!is_silenced(&status));
    }

    #[test]
    fn is_silenced_returns_false_when_absent() {
        let status = StatusEffects::default();
        assert!(!is_silenced(&status));
    }

    // check_dead_and_apply tests live in app_tests (need MessageWriter App context).
}

#[cfg(test)]
mod app_tests {
    use super::*;
    use bevy::ecs::message::Messages;
    use bevy::state::app::StatesPlugin;

    /// Write a message directly into the Messages resource.
    /// Mirrors the pattern from `features.rs::write_moved`.
    fn write_apply_status(app: &mut App, ev: ApplyStatusEvent) {
        app.world_mut()
            .resource_mut::<Messages<ApplyStatusEvent>>()
            .write(ev);
    }

    fn write_status_tick(app: &mut App, target: Entity) {
        app.world_mut()
            .resource_mut::<Messages<StatusTickEvent>>()
            .write(StatusTickEvent { target });
    }

    /// Test harness: MinimalPlugins + StatesPlugin + AssetPlugin +
    /// PartyPlugin + StatusEffectsPlugin. NO DungeonPlugin (we don't need
    /// MovedEvent emission in handler/ticker tests; we write StatusTickEvent
    /// directly).
    ///
    /// Pattern from `features.rs:736-771` adapted for #14.
    fn make_test_app() -> App {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            bevy::asset::AssetPlugin::default(),
            StatesPlugin,
            crate::plugins::state::StatePlugin,
            crate::plugins::party::PartyPlugin,
            StatusEffectsPlugin,
        ));
        // PartyPlugin's populate_item_handle_registry runs on OnExit(Loading)
        // and reads Assets<ItemDb>. Tests don't trigger that transition;
        // explicit init_asset is still required as defensive setup.
        app.init_asset::<crate::data::ItemDb>();
        #[cfg(feature = "dev")]
        app.init_resource::<bevy::input::ButtonInput<bevy::prelude::KeyCode>>();
        app
    }

    fn spawn_party_member(app: &mut App, current_hp: u32) -> Entity {
        app.world_mut()
            .spawn(crate::plugins::party::PartyMemberBundle {
                derived_stats: crate::plugins::party::DerivedStats {
                    current_hp,
                    max_hp: 100,
                    max_mp: 50,
                    ..Default::default()
                },
                ..Default::default()
            })
            .id()
    }

    // ── Stacking ─────────────────────────────────────────────────────────────

    #[test]
    fn apply_status_handler_pushes_new_effect() {
        let mut app = make_test_app();
        let target = spawn_party_member(&mut app, 100);
        write_apply_status(
            &mut app,
            ApplyStatusEvent {
                target,
                effect: StatusEffectType::Poison,
                potency: 1.0,
                duration: Some(5),
            },
        );
        app.update();
        let status = app.world().get::<StatusEffects>(target).unwrap();
        assert_eq!(status.effects.len(), 1);
        assert_eq!(status.effects[0].effect_type, StatusEffectType::Poison);
        assert_eq!(status.effects[0].magnitude, 1.0);
    }

    #[test]
    fn apply_status_handler_refreshes_duration() {
        let mut app = make_test_app();
        let target = spawn_party_member(&mut app, 100);
        // First Poison: duration 3.
        write_apply_status(
            &mut app,
            ApplyStatusEvent {
                target,
                effect: StatusEffectType::Poison,
                potency: 0.5,
                duration: Some(3),
            },
        );
        app.update();
        // Second Poison: duration 5, magnitude 1.0.
        write_apply_status(
            &mut app,
            ApplyStatusEvent {
                target,
                effect: StatusEffectType::Poison,
                potency: 1.0,
                duration: Some(5),
            },
        );
        app.update();
        let status = app.world().get::<StatusEffects>(target).unwrap();
        assert_eq!(status.effects.len(), 1, "stacking does not duplicate");
        assert_eq!(
            status.effects[0].remaining_turns,
            Some(5),
            "duration refreshed"
        );
        assert_eq!(status.effects[0].magnitude, 1.0, "max magnitude wins");
    }

    #[test]
    fn apply_status_handler_takes_higher_magnitude() {
        let mut app = make_test_app();
        let target = spawn_party_member(&mut app, 100);
        // First: magnitude 1.0.
        write_apply_status(
            &mut app,
            ApplyStatusEvent {
                target,
                effect: StatusEffectType::Poison,
                potency: 1.0,
                duration: Some(5),
            },
        );
        app.update();
        // Second: lower magnitude 0.3.
        write_apply_status(
            &mut app,
            ApplyStatusEvent {
                target,
                effect: StatusEffectType::Poison,
                potency: 0.3,
                duration: Some(5),
            },
        );
        app.update();
        let status = app.world().get::<StatusEffects>(target).unwrap();
        assert_eq!(status.effects[0].magnitude, 1.0, "higher magnitude wins");
    }

    #[test]
    fn apply_status_handler_stone_reapply_is_noop() {
        let mut app = make_test_app();
        let target = spawn_party_member(&mut app, 100);
        // First Stone (permanent — None duration).
        write_apply_status(
            &mut app,
            ApplyStatusEvent {
                target,
                effect: StatusEffectType::Stone,
                potency: 1.0,
                duration: None,
            },
        );
        app.update();
        let len_after_first = app
            .world()
            .get::<StatusEffects>(target)
            .unwrap()
            .effects
            .len();
        // Re-apply.
        write_apply_status(
            &mut app,
            ApplyStatusEvent {
                target,
                effect: StatusEffectType::Stone,
                potency: 1.0,
                duration: None,
            },
        );
        app.update();
        let status = app.world().get::<StatusEffects>(target).unwrap();
        assert_eq!(
            status.effects.len(),
            len_after_first,
            "Stone re-apply no-op"
        );
    }

    #[test]
    fn apply_status_handler_clamps_nan_to_zero() {
        let mut app = make_test_app();
        let target = spawn_party_member(&mut app, 100);
        write_apply_status(
            &mut app,
            ApplyStatusEvent {
                target,
                effect: StatusEffectType::Poison,
                potency: f32::NAN,
                duration: Some(5),
            },
        );
        app.update();
        let status = app.world().get::<StatusEffects>(target).unwrap();
        // `f32::clamp` propagates NaN; the handler explicitly maps non-finite
        // potency to 0.0 at the trust boundary.
        assert_eq!(status.effects[0].magnitude, 0.0, "NaN clamps to 0");
    }

    // ── Tick decrement ────────────────────────────────────────────────────────

    #[test]
    fn tick_decrements_duration() {
        let mut app = make_test_app();
        let target = spawn_party_member(&mut app, 100);
        write_apply_status(
            &mut app,
            ApplyStatusEvent {
                target,
                effect: StatusEffectType::Poison,
                potency: 0.0,
                duration: Some(3),
            },
        );
        app.update();
        // Tick once.
        write_status_tick(&mut app, target);
        app.update();
        let status = app.world().get::<StatusEffects>(target).unwrap();
        assert_eq!(status.effects[0].remaining_turns, Some(2));
    }

    #[test]
    fn tick_removes_expired() {
        let mut app = make_test_app();
        let target = spawn_party_member(&mut app, 100);
        write_apply_status(
            &mut app,
            ApplyStatusEvent {
                target,
                effect: StatusEffectType::Poison,
                potency: 0.0,
                duration: Some(1),
            },
        );
        app.update();
        write_status_tick(&mut app, target);
        app.update();
        let status = app.world().get::<StatusEffects>(target).unwrap();
        assert!(status.effects.is_empty(), "duration-1 expires after tick");
    }

    #[test]
    fn tick_skips_permanent_effects() {
        let mut app = make_test_app();
        let target = spawn_party_member(&mut app, 100);
        write_apply_status(
            &mut app,
            ApplyStatusEvent {
                target,
                effect: StatusEffectType::Stone,
                potency: 1.0,
                duration: None,
            },
        );
        app.update();
        for _ in 0..5 {
            write_status_tick(&mut app, target);
            app.update();
        }
        let status = app.world().get::<StatusEffects>(target).unwrap();
        assert_eq!(status.effects.len(), 1);
        assert!(status.has(StatusEffectType::Stone));
    }

    // ── Resolvers ─────────────────────────────────────────────────────────────

    #[test]
    fn poison_damages_on_tick() {
        let mut app = make_test_app();
        let target = spawn_party_member(&mut app, 100); // current_hp=100, max_hp=100
        // Apply Poison magnitude 1.0 → 5% of max_hp = 5 damage.
        write_apply_status(
            &mut app,
            ApplyStatusEvent {
                target,
                effect: StatusEffectType::Poison,
                potency: 1.0,
                duration: Some(5),
            },
        );
        app.update();
        // Tick once.
        write_status_tick(&mut app, target);
        app.update();
        let derived = app
            .world()
            .get::<crate::plugins::party::DerivedStats>(target)
            .unwrap();
        assert_eq!(derived.current_hp, 95, "5% of max_hp=100 = 5 damage");
    }

    #[test]
    fn regen_heals_on_tick_capped_at_max() {
        let mut app = make_test_app();
        let target = spawn_party_member(&mut app, 1); // current_hp=1, max_hp=100
        // duration: Some(100) — enough ticks to fully heal from 1 to 100
        // (5 HP/tick × 20 ticks needed; 100 ticks available).
        write_apply_status(
            &mut app,
            ApplyStatusEvent {
                target,
                effect: StatusEffectType::Regen,
                potency: 1.0,
                duration: Some(100),
            },
        );
        app.update();
        write_status_tick(&mut app, target);
        app.update();
        let derived = app
            .world()
            .get::<crate::plugins::party::DerivedStats>(target)
            .unwrap();
        assert_eq!(derived.current_hp, 6, "5% of max_hp=100 = 5 heal; 1+5=6");
        // Heal up to cap (50 more ticks; 5 HP/tick → 250 HP heal → clamps at 100).
        for _ in 0..50 {
            write_status_tick(&mut app, target);
            app.update();
        }
        let derived = app
            .world()
            .get::<crate::plugins::party::DerivedStats>(target)
            .unwrap();
        assert_eq!(derived.current_hp, 100, "regen caps at max_hp");
    }

    #[test]
    fn duration_one_poison_damages_then_expires_same_frame() {
        // Verifies the resolver-vs-ticker ordering (Critical / Open Item 7):
        // a duration-1 poison must deal damage BEFORE being removed.
        let mut app = make_test_app();
        let target = spawn_party_member(&mut app, 100);
        write_apply_status(
            &mut app,
            ApplyStatusEvent {
                target,
                effect: StatusEffectType::Poison,
                potency: 1.0,
                duration: Some(1),
            },
        );
        app.update();
        write_status_tick(&mut app, target);
        app.update();
        let derived = app
            .world()
            .get::<crate::plugins::party::DerivedStats>(target)
            .unwrap();
        let status = app.world().get::<StatusEffects>(target).unwrap();
        assert_eq!(
            derived.current_hp, 95,
            "duration-1 poison damaged on its final tick"
        );
        assert!(status.effects.is_empty(), "...and was removed after");
    }

    // ── check_dead_and_apply ──────────────────────────────────────────────────

    // Helper systems that call check_dead_and_apply; scheduled once via
    // add_systems so they have a real MessageWriter system parameter.
    fn system_call_check_dead(
        party: Query<(Entity, &crate::plugins::party::DerivedStats), With<PartyMember>>,
        mut writer: MessageWriter<ApplyStatusEvent>,
    ) {
        for (entity, derived) in &party {
            check_dead_and_apply(entity, derived, &mut writer);
        }
    }

    #[test]
    fn check_dead_and_apply_writes_when_hp_zero() {
        let mut app = make_test_app();
        app.add_systems(
            bevy::app::Update,
            system_call_check_dead.before(apply_status_handler),
        );
        let target = spawn_party_member(&mut app, 0); // current_hp = 0
        app.update();
        let status = app.world().get::<StatusEffects>(target).unwrap();
        assert!(
            status.has(StatusEffectType::Dead),
            "Dead applied when hp == 0"
        );
    }

    #[test]
    fn check_dead_and_apply_no_op_when_hp_positive() {
        let mut app = make_test_app();
        app.add_systems(
            bevy::app::Update,
            system_call_check_dead.before(apply_status_handler),
        );
        let target = spawn_party_member(&mut app, 50); // current_hp > 0
        app.update();
        let status = app.world().get::<StatusEffects>(target).unwrap();
        assert!(
            !status.has(StatusEffectType::Dead),
            "no Dead applied when hp > 0"
        );
    }

    // ── End-to-end dungeon-step test ──────────────────────────────────────────

    /// Test harness with DungeonPlugin — needed for `MovedEvent` emission
    /// and the cross-plugin `tick_on_dungeon_step` → resolver flow.
    fn make_test_app_with_dungeon() -> App {
        use leafwing_input_manager::prelude::ActionState;
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            bevy::asset::AssetPlugin::default(),
            StatesPlugin,
            bevy::input::InputPlugin,
            crate::plugins::state::StatePlugin,
            crate::plugins::dungeon::DungeonPlugin,
            crate::plugins::dungeon::features::CellFeaturesPlugin,
            crate::plugins::party::PartyPlugin,
            StatusEffectsPlugin,
        ));
        app.init_resource::<ActionState<crate::plugins::input::DungeonAction>>();
        app.init_asset::<crate::data::DungeonFloor>();
        app.init_asset::<crate::data::ItemDb>();
        app.init_asset::<bevy::prelude::Mesh>();
        app.init_asset::<bevy::pbr::StandardMaterial>();
        app.add_message::<crate::plugins::audio::SfxRequest>();
        #[cfg(feature = "dev")]
        app.init_resource::<bevy::input::ButtonInput<bevy::prelude::KeyCode>>();
        app
    }

    #[test]
    fn dungeon_step_triggers_poison_tick_end_to_end() {
        let mut app = make_test_app_with_dungeon();

        // Spawn a party member BEFORE advance_into_dungeon so that
        // spawn_default_debug_party (--features dev) sees existing members
        // and skips (same pattern as features.rs tests).
        let target = spawn_party_member(&mut app, 100);

        // Transition into Dungeon state (required for .run_if(in_state(Dungeon))).
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Dungeon);
        app.update(); // StateTransition realizes the new state
        app.update(); // OnEnter(Dungeon) systems run

        // Apply Poison via the canonical handler.
        write_apply_status(
            &mut app,
            ApplyStatusEvent {
                target,
                effect: StatusEffectType::Poison,
                potency: 1.0,
                duration: Some(5),
            },
        );
        app.update(); // flush handler

        // Emit a MovedEvent (the dungeon-step trigger).
        use crate::data::dungeon::Direction;
        use crate::plugins::dungeon::{GridPosition, MovedEvent};
        app.world_mut()
            .resource_mut::<Messages<MovedEvent>>()
            .write(MovedEvent {
                from: GridPosition { x: 0, y: 0 },
                to: GridPosition { x: 1, y: 0 },
                facing: Direction::East,
            });
        app.update(); // tick_on_dungeon_step → StatusTickEvent → apply_poison_damage

        let derived = app
            .world()
            .get::<crate::plugins::party::DerivedStats>(target)
            .unwrap();
        assert_eq!(
            derived.current_hp, 95,
            "MovedEvent triggered one poison tick"
        );
    }
}
