---
name: Druum encounter system (Feature #16) decisions
description: Feature #16 architecture — Message-pipe encounter trigger composing with apply_alarm_trap, single-file combat/encounter.rs + data/encounters.rs schema, separate EncounterRng from CombatRng, soft-pity cap at 2.0, F7 force-encounter via direct ButtonInput, snap MovementAnimation on combat entry
type: project
---

# Feature #16 Encounter System & Random Battles — planning decisions (frozen at plan time, 2026-05-08)

The full plan: `project/plans/20260508-200000-feature-16-encounter-system-and-random-battles.md`.

## Pipeline shape (locked — D-A1)

**Message-pipe (research Option A):**

```text
MovedEvent (DungeonPlugin) → check_random_encounter → EncounterRequested (Message) → handle_encounter_request → spawns enemies + inserts CurrentEncounter + transitions to Combat
                                                            ↑
                                                  apply_alarm_trap (#13, frozen) writes the same Message — composes
```

- `handle_encounter_request` is the SOLE writer of `CurrentEncounter` and the SOLE producer of the `Dungeon → Combat` transition trigger. Both random rolls AND alarm-traps feed the same channel — one observable seam, two producers.
- Future #22 FOE work adds a third producer to the same channel; consumer is unchanged.

## File ownership (D-A2)

- **`src/plugins/combat/encounter.rs`** (~350 LOC) — owns `EncounterPlugin`, `EncounterState`, `EncounterRng`, `CurrentEncounter`, `FoeProximity` stub, `check_random_encounter`, `handle_encounter_request`, `reset_encounter_state`, `clear_current_encounter`, `snap_movement_animation_on_combat_entry`, `force_encounter_on_f7` (dev-only).
- **`src/data/encounters.rs`** (~120 LOC) — owns `EncounterTable` (Asset), `EncounterEntry`, `EnemyGroup`, `EnemySpec`, and `pick_group` method on `EncounterTable`.
- Mirrors `combat/status_effects.rs` (single-file plugin) and `data/dungeon.rs` (schema + pure-fn methods like `can_move`) precedents.

## Carve-outs (5 explicit edits to frozen files)

1. `dungeon/features.rs` — +1 enum variant: `EncounterSource::Random` (the existing comment "Future: Random (foe roll), Foe (overworld encounter) — surface in #16" gets fulfilled).
2. `loading/mod.rs` — +1 import + +1 `RonAssetPlugin::<EncounterTable>::new(&["encounters.ron"])` registration in existing tuple + +1 `Handle<EncounterTable>` field on `DungeonAssets` + +1 new `pub(crate) fn encounter_table_for(assets, floor_number) -> &Handle<EncounterTable>` (mirrors `floor_handle_for`).
3. `combat/turn_manager.rs` — DELETE `#[cfg(feature = "dev")] spawn_dev_encounter` function (lines 671-716) AND its registration (lines 181-186). Per #15 plan Pitfall 1 lock. Doc-comment on `CurrentEncounter` updated to remove "Test fixtures define their own resource directly" line.
4. `combat/mod.rs` — +2 lines (module declaration + `app.add_plugins(encounter::EncounterPlugin)`).
5. `data/mod.rs` — +2 lines (module declaration + re-export of `EncounterTable, EncounterEntry, EnemyGroup, EnemySpec`).

NO edits to: `Cargo.toml` (0 deps), `main.rs`, `state/mod.rs`, `input/mod.rs` (frozen by #5; F7 via direct ButtonInput), `audio/{mod,bgm,sfx}.rs` (frozen by #6), `dungeon/mod.rs` (frozen by #7-#9), all other combat/ files except mod.rs and turn_manager.rs.

## RNG isolation (D-A5)

`EncounterRng(pub Box<dyn rand::RngCore + Send + Sync>)` is **separate** from `combat::turn_manager::CombatRng`. Reason: encounter rolls happen during `GameState::Dungeon` where `CombatRng` may not yet be initialised (it re-seeds in `init_combat_state` on `OnEnter(Combat)`). Same Box-dyn shape; tests inject `rand_chacha::ChaCha8Rng::seed_from_u64(seed)` directly. Production seeds `SmallRng::from_os_rng()` once via `Default::default()`.

`?Sized` discipline preserved: `pick_group(&self, rng: &mut (impl rand::Rng + ?Sized))` (locked by #15 D-I13).

## Soft-pity formula (D-X2 — cap multiplier at 2.0)

```rust
let multiplier = (1.0 + state.steps_since_last as f32 * 0.05).min(2.0);
let probability = cell.encounter_rate.clamp(0.0, 1.0) * multiplier;
if rng.0.random::<f32>() < probability { /* trigger */ }
```

- Counter bumps on EVERY step regardless of rate (no special-casing rate-zero cells).
- Multiplier capped at 2.0 — prevents unbounded growth across designer-authored "safe corridors".
- Counter resets to 0 on encounter trigger AND on every `OnEnter(Dungeon)` (D-X1).
- `clamp(0.0, 1.0)` on cell rate is a trust-boundary defense (Security §Architectural Risks).

## `CurrentEncounter` resource shape (locked by #15 contract)

```rust
#[derive(Resource, Debug, Clone)]
pub struct CurrentEncounter {
    pub enemy_entities: Vec<Entity>,
    pub fleeable: bool,
}
```

- Inserted by `handle_encounter_request`; removed by `clear_current_encounter` on `OnExit(Combat)` (Pitfall 6).
- `fleeable: true` for `Random | AlarmTrap` (exhaustive match in v1). Future `Foe { boss: bool }` in #22 adds its own arm.

## `FoeProximity` stub (for #22)

```rust
#[derive(Resource, Default, Debug, Clone)]
pub struct FoeProximity {
    pub nearby_foe_entities: Vec<Entity>,
}
impl FoeProximity {
    pub fn suppresses_random_rolls(&self) -> bool { !self.nearby_foe_entities.is_empty() }
}
```

`Vec<Entity>` not `bool` — research recommendation; gives #22 room for richer suppression rules without breaking the type. v1 ships with `Default::default()` (always returns "no FOEs").

## Inline `EnemySpec` (D-A4 / D-X3 — defer ID-refs to #17)

```rust
#[derive(Reflect, Serialize, Deserialize, Debug, Clone)]
pub struct EnemySpec {
    pub name: String,
    pub base_stats: BaseStats,
    pub derived_stats: DerivedStats,
    #[serde(default)]
    pub ai: EnemyAi,
}
```

Migration path when #17 ships `EnemyDb`: add `enemy_id: Option<String>` field (additive, backward-compatible); resolver falls back to inline values when `enemy_id == None`.

## Trust-boundary clamps (Security §Architectural Risks)

- `cell.encounter_rate.clamp(0.0, 1.0)` on read in `check_random_encounter`.
- `MAX_ENEMIES_PER_ENCOUNTER = 8` constant; oversized groups truncated with `warn!` in `handle_encounter_request`.
- Weights pre-clamped to `1..=10000` in `pick_group` (defends against u32-overflow / DoS).

## F7 force-encounter (D-X6)

```rust
#[cfg(feature = "dev")]
fn force_encounter_on_f7(
    keys: Res<bevy::input::ButtonInput<bevy::prelude::KeyCode>>,
    mut encounter: MessageWriter<EncounterRequested>,
) {
    if keys.just_pressed(bevy::prelude::KeyCode::F7) {
        encounter.write(EncounterRequested { source: EncounterSource::Random });
    }
}
```

Direct `ButtonInput<KeyCode>` reader, NOT through leafwing `DungeonAction` enum (frozen by #5). Mirrors F9 state-cycler precedent at `state/mod.rs:71-89`.

## Snap `MovementAnimation` on combat entry (D-A9 / D-X4)

```rust
fn snap_movement_animation_on_combat_entry(
    mut commands: Commands,
    mut query: Query<(Entity, &mut Transform, &MovementAnimation), With<PlayerParty>>,
) {
    for (entity, mut transform, anim) in &mut query {
        transform.translation = anim.to_translation;
        transform.rotation = anim.to_rotation;
        commands.entity(entity).remove::<MovementAnimation>();
    }
}
```

Without this, a 50%-complete tween freezes during combat (animate_movement is gated `.run_if(in_state(GameState::Dungeon))`) and resumes on combat-exit → perceived "jump". Snap is research recommendation; encounter-sting flash polish to mask deferred to #25.

## System ordering (Pitfall 5)

```rust
.add_systems(Update, (
    check_random_encounter
        .run_if(in_state(GameState::Dungeon))
        .after(handle_dungeon_input),
    handle_encounter_request
        .run_if(in_state(GameState::Dungeon))
        .after(check_random_encounter),
))
```

Same shape as `apply_alarm_trap.after(handle_dungeon_input)` at `features.rs:172-174`.

## Cursor-draining discipline (Pitfall 4)

`check_random_encounter` MUST drain `moved.read()` cursor on EVERY early return:

```rust
let Some(assets) = dungeon_assets else {
    for _ in moved.read() {} // drain — Pitfall 4
    return;
};
```

Same shape as `audio/sfx.rs:73-78` precedent.

## Same-frame multi-encounter collapse (D-A8)

`handle_encounter_request` takes `requests.read().next()` only — alarm-trap + same-step random roll on the same cell collapse to one combat. Drain remainder so they don't replay next frame.

## Test patterns

- **Layer 1** (pure tests in `data/encounters.rs::tests` and `combat/encounter.rs::tests`): `pick_group` proportional sampling test (10K iterations at seed 42, ±5% bounds), RON round-trip test, `floor_01.encounters.ron` parses test.
- **Layer 2** (App-driven in `combat/encounter.rs::app_tests`): `make_test_app()` follows the canonical pattern from `turn_manager.rs::app_tests:788-810` — registers `MovedEvent` explicitly via `app.add_message::<crate::plugins::dungeon::MovedEvent>()` (Pitfall 1, mirrors #15 D-I16). Tests cover: `steps_reset_on_dungeon_entry`, `current_encounter_removed_on_combat_exit`, `movement_animation_snaps_on_combat_entry`, `rate_zero_cell_no_encounter_rolls`, `foe_proximity_suppresses_rolls`, `encounter_request_triggers_combat_state` (partially gated on `DungeonAssets` constructibility — D-X11), `force_encounter_on_f7_writes_message` (dev-only).
- Test count delta: +9-10 (within research envelope of +6-10). Total: 191→200 default, 194→204 dev.

## Open questions resolved as Cat-A (no user input needed)

All 7 research open questions had HIGH-confidence researcher recommendations; planner adopted them all:

| Question | Resolution | Decision ID |
|----------|------------|-------------|
| Soft-pity reset on combat-end? | Reset on `OnEnter(Dungeon)` | D-X1 |
| Rate-zero accumulator interaction? | Cap multiplier at 2.0 | D-X2 |
| Inline EnemySpec vs ID-refs? | Inline; defer ID-refs to #17 | D-X3 / D-A4 |
| MovementAnimation snap on Combat entry? | Snap to completion | D-X4 / D-A9 |
| EncounterSource::Random only vs +Foe placeholder? | Random only; #22 adds own | D-X5 |
| force_encounter keybind? | F7 via direct ButtonInput | D-X6 |
| pick_enemy_group location? | Method on EncounterTable in data/encounters.rs | D-X7 / D-A6 |

## What #16 does NOT ship (deferred)

- **Visible enemies on dungeon map (FOEs)** → #22 (FoeProximity stub here; #22 fills in).
- **Per-instance enemy stats from EnemyDb** → #17 (inline EnemySpec for now).
- **Encounter-sting flash transition** → #25 polish.
- **Additional floor encounter tables** → future content authoring.
- **Per-floor base rates / non-fleeable bosses** → #22 / future polish.
- **Save/load support** → #23.
- **Combat hit/death SFX** → #17 polish.

## Commit boundary plan (5 atomic commits via `but commit --message-file`)

1. `feat(combat): add EncounterSource::Random variant for #16` (Step 1)
2. `feat(data): EncounterTable schema (#16 step 1)` (Steps 2-4 together)
3. `feat(loading): register EncounterTable loader and DungeonAssets handle (#16 step 2)` (Step 5)
4. `refactor(combat): delete spawn_dev_encounter dev stub (#16 step 3, #15 plan Pitfall 1)` (Step 6)
5. `feat(combat): EncounterPlugin (#16 step 4 — random encounters and combat entry)` (Steps 7-13 together)

After all 5 commits: full verification gate (cargo check / test / clippy / fmt + grep guards + manual smoke). Then `but push -u origin <branch-name>` and `gh pr create`.
