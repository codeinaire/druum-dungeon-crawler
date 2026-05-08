//! Target selection — Feature #15 Phase 15B.
//!
//! `TargetSelection` enum (queue payload) + `resolve_target_with_fallback`
//! pure function that handles the re-target-on-death edge case
//! (research Pattern 3, Pitfall 3).

use bevy::prelude::*;
use rand::seq::IteratorRandom;

use crate::plugins::combat::actions::Side;

/// Target-selection enum. The queue payload references this.
#[derive(Debug, Clone)]
pub enum TargetSelection {
    Single(Entity),
    AllAllies,
    AllEnemies,
    Self_,
    None,
}

/// Resolve `selection` to a list of currently-alive entities.
///
/// Re-target rule for `Single(t)` when `t` is dead: pick a random alive
/// entity from the SAME side as the original target. Side membership is
/// determined by which slice (`party` or `enemies`) contains `t`.
///
/// **PURE** — no `Mut`, no `Query`, no entity lookups beyond the slices
/// caller provides + the `is_alive` predicate.
pub fn resolve_target_with_fallback(
    selection: &TargetSelection,
    actor: Entity,
    actor_side: Side,
    party: &[Entity],
    enemies: &[Entity],
    is_alive: impl Fn(Entity) -> bool,
    rng: &mut (impl rand::Rng + ?Sized),
) -> Vec<Entity> {
    use TargetSelection::*;
    match selection {
        Single(t) if is_alive(*t) => vec![*t],
        Single(t) => {
            // Re-target: pick same side as original target.
            let same_side = if party.contains(t) { party } else { enemies };
            same_side
                .iter()
                .filter(|e| is_alive(**e))
                .copied()
                .choose(rng)
                .map(|e| vec![e])
                .unwrap_or_default()
        }
        AllAllies => {
            let side = if actor_side == Side::Party {
                party
            } else {
                enemies
            };
            side.iter().filter(|e| is_alive(**e)).copied().collect()
        }
        AllEnemies => {
            let side = if actor_side == Side::Party {
                enemies
            } else {
                party
            };
            side.iter().filter(|e| is_alive(**e)).copied().collect()
        }
        Self_ => {
            if is_alive(actor) {
                vec![actor]
            } else {
                vec![]
            }
        }
        None => vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;

    /// Build a fake entity from a raw index for pure tests.
    /// Uses `Entity::from_bits` since `Entity::from_raw` is not available in Bevy 0.18.
    fn e(idx: u32) -> Entity {
        Entity::from_bits(idx as u64)
    }

    #[test]
    fn single_target_alive_returns_target() {
        let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(0);
        let target = e(5);
        let party = vec![e(1), e(2)];
        let enemies = vec![target, e(6)];
        let result = resolve_target_with_fallback(
            &TargetSelection::Single(target),
            e(1),
            Side::Party,
            &party,
            &enemies,
            |e| e != Entity::PLACEHOLDER, // all alive
            &mut rng,
        );
        assert_eq!(result, vec![target]);
    }

    #[test]
    fn single_target_dead_re_targets_same_side() {
        let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(0);
        let dead_target = e(5);
        let alive_enemy = e(6);
        let party = vec![e(1)];
        let enemies = vec![dead_target, alive_enemy];
        let result = resolve_target_with_fallback(
            &TargetSelection::Single(dead_target),
            e(1),
            Side::Party,
            &party,
            &enemies,
            |ent| ent == alive_enemy || ent == e(1), // dead_target is dead
            &mut rng,
        );
        assert_eq!(result, vec![alive_enemy]);
    }

    #[test]
    fn single_target_dead_no_alive_on_side_returns_empty() {
        let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(0);
        let dead_target = e(5);
        let party = vec![e(1)];
        let enemies = vec![dead_target]; // only enemy is dead
        let result = resolve_target_with_fallback(
            &TargetSelection::Single(dead_target),
            e(1),
            Side::Party,
            &party,
            &enemies,
            |ent| ent == e(1), // only party member is alive
            &mut rng,
        );
        assert!(result.is_empty());
    }
}
