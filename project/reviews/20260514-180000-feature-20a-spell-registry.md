# Review: Feature #20a — Spell Registry and Cast Resolver (Phase 1)

**PR:** https://github.com/codeinaire/druum-dungeon-crawler/pull/21
**Branch:** `feature-20a-spell-registry`
**Commit:** `e343585`
**Reviewed:** 2026-05-14

## Verdict: APPROVE

| Severity | Count |
| -------- | ----- |
| CRITICAL | 0     |
| HIGH     | 0     |
| MEDIUM   | 1     |
| LOW      | 1     |

## Files Reviewed (Full Coverage)

- `src/data/spells.rs` — full
- `assets/spells/core.spells.ron` — full
- `src/plugins/combat/spell_cast.rs` — full
- `src/plugins/combat/status_effects.rs` — full
- `src/plugins/combat/turn_manager.rs` — full
- `src/plugins/combat/enemy.rs` — full
- `src/plugins/combat/targeting.rs` — full
- `src/plugins/party/inventory.rs` — partial (recompute function only)
- `src/plugins/loading/mod.rs` — relevant section
- `tests/spell_db_loads.rs` — full

## Focus Area Verdicts

### 1. `apply_status_handler` widening — SAFE

Writer audit verified:

- `turn_manager.rs:553` (Defend → DefenseUp): `action.actor` is always a party entity. The `collect_player_actions` state machine iterates `With<PartyMember>` and auto-skips by enemy side; the committed vector is party-only. No enemy receives `DefenseUp` from this path.
- `dungeon/features.rs:447` (poison trap): iterates `party: Query<Entity, With<PartyMember>>` explicitly. No enemy exposure.
- `turn_manager.rs:725` (ApplyStatus spell arm): targets via `resolve_target_with_fallback`. Enemies can be targeted — this is intended (Silence, Paralysis, Sleep on enemies). Previously silently dropped; now correctly applied.
- `turn_manager.rs:751` (Buff spell arm): targets via `resolve_target_with_fallback`. Buff spells in `core.spells.ron` use `Self_` or `AllAllies` from a party caster — `AllAllies` resolves to the party slice, never enemies. Future spell authors adding `Buff(effect: AttackUp, target: AllEnemies)` would apply a buff to enemies, which is semantically correct (enemy self-buff). No accidental party-buff-on-enemy possible via the resolver.
- `status_effects.rs:400` (`check_dead_and_apply`): writes `Dead`, not a stat buff. Safe on enemies and fixes the latent basic-attack death-on-enemy silent-drop bug.

The `EquipmentChangedEvent` nudge at line 229 for `Dead` is confirmed safe: `recompute_derived_stats_on_equipment_change` (`inventory.rs:444`) has no `With<PartyMember>` filter. `EnemyBundle` includes `BaseStats`, `Equipment`, `StatusEffects`, `Experience`, and `DerivedStats` per `enemy.rs:8-10`. The recompute function runs correctly for enemies.

### 2. `tick_status_durations` widening — NO-OP TODAY, SAFE

`StatusTickEvent` is written only by `tick_on_dungeon_step` (iterates `With<PartyMember>`) and test helpers. No combat-round emitter exists for enemies in Phase 1 (the module doc at `status_effects.rs:97-98` explicitly defers this to #15). The widening has zero runtime effect today and correctly positions the system for the future emitter. Confirmed safe.

### 3. `SpellCastParams<'w>` SystemParam — CORRECT

The `'w`-only lifetime is appropriate: all three fields (`Res<'w, Assets<SpellDb>>`, `Option<Res<'w, DungeonAssets>>`, `MessageWriter<'w, EquipmentChangedEvent>`) are world-resource-scoped, not archetype/state-scoped. No `Query<>` fields requiring a `'s` state lifetime. The Bevy 0.18 `#[derive(SystemParam)]` macro generates the correct `impl` for this shape. Clippy + tests pass; the derive is sound.

### 4. Five deviations — ALL ACCEPTABLE

1. **Crit uses `accuracy/5%`**: The plan's Phase 1 step 1.5 explicitly specifies `accuracy/5%` (the research section said `luck/5%` but the plan resolved this before implementation). The implementer aligned with the Phase 1 spec. Not a deviation from the authoritative source.
2. **`&mut StatusEffects` on `CombatantCharsQuery`**: Correct — `&mut` subsumes `&`, avoids B0002, and the Revive arm's `.get_mut()` is the only mutation path. Documented clearly.
3. **Revive bypasses `resolve_target_with_fallback`**: Correct — the helper filters dead entities by design (re-target-on-death rule); Revive must reach dead entities. The defense-in-depth `is_dead` check at line 784 prevents accidental revives of live members.
4. **Announce-before-effects log order**: Game-feel improvement, no correctness impact.
5. **Four test fixture renames**: Mechanical correctness fix for `spell_table → spells` rename.

### 5. Carry-forward concern: `apply_poison_damage` + `apply_regen` still `With<PartyMember>`

These two resolvers at `status_effects.rs:319-338` and `346-367` are safe today because `StatusTickEvent` is written only for party members (`tick_on_dungeon_step` iterates `With<PartyMember>`). If a future feature emits `StatusTickEvent` for enemies, these resolvers would silently skip enemy poison/regen ticks.

**Recommendation:** The implementer's plan note is sufficient for now. File a GitHub issue before Phase 3 ships, so it does not fall off the radar when Phase 2 adds the combat-round tick emitter.

## Findings

### [MEDIUM] MP check uses pre-round snapshot; deduction uses live `derived_mut`

**File:** `src/plugins/combat/turn_manager.rs:594-607`

**Issue:** `check_mp` reads from `actor_derived_snapshot` (built from `entity_snapshots` at the top of `execute_combat_actions`), while `deduct_mp` writes to the live `derived_mut`. If a round ever permits a single character to have two queued actions (e.g., a future haste/double-cast mechanic), the second cast would pass the MP check against stale pre-deduction values.

Today this is safe — one action per member per round is the only possible state. But the divergence is not documented as a known constraint.

**Fix:** Add a comment at the MP check site explaining the snapshot-vs-live split and the one-action-per-member invariant that makes it safe. No code change needed for Phase 1; document before Phase 2's SP mechanic adds complexity.

```rust
// 4. MP check uses the pre-round snapshot (built once before the action loop).
// Safe invariant: each party member commits exactly one action per round, so
// the snapshot MP is always current at check time. If future mechanics allow
// double-cast (haste, etc.), this check must switch to derived_mut.get(actor).
let actor_derived_snapshot = entity_snapshots
    .get(&action.actor)
    .map(|s| s.derived)
    .unwrap_or_default();
```

### [LOW] `apply_poison_damage` / `apply_regen` gap not tracked in a GitHub issue

**File:** `src/plugins/combat/status_effects.rs:319, 346`

**Issue:** The carry-forward gap (two resolvers still `With<PartyMember>`) is noted only in the implementer's plan document and the pipeline state. If Phase 2 adds a combat-round `StatusTickEvent` emitter for enemies without addressing these resolvers, enemy poison/regen ticks will silently no-op. The plan note is easy to miss under time pressure.

**Fix:** File a GitHub issue (e.g., "Widen apply_poison_damage + apply_regen for enemy status ticks (#20 follow-up)") and add a `// TODO(#<issue>)` comment at each function. Suggested timing: before Phase 2 ships.

## Summary

Phase 1 is clean. The five deviations are all correct adaptations. The status-effect filter widening is safe per the writer audit. The `SpellCastParams` SystemParam bundle is architecturally sound. The RON data is well-formed and load-tested. All quality gates passed (339/339 lib tests, 343/343 with dev features, 3/3 integration tests, clippy clean).

**Merge recommendation: merge as-is.** The MEDIUM finding (MP snapshot comment) does not block correctness and can be addressed in the Phase 2 PR as a carry-forward note.
