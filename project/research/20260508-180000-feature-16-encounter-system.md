# Feature #16 — Encounter System & Random Battles — Research

**Researched:** 2026-05-08
**Domain:** Druum / Bevy 0.18.1 / DRPG random-encounter trigger + post-combat dungeon restoration
**Confidence:** HIGH on every "what already exists" claim (each cited at file:line, all from the merged tree post-#15). HIGH on the `CurrentEncounter` shape (already documented in `turn_manager.rs:36-45` as #16's contract). HIGH on `rand 0.9.4` API surface (verified against `Cargo.lock:4360-4368` and `damage.rs`/`turn_manager.rs` working call sites). HIGH on the `MovedEvent` consumer pattern (mirrors `apply_alarm_trap` and `apply_poison_trap`). HIGH on `bevy_common_assets` `RonAssetPlugin` (already loads 5 different RON asset types via `loading/mod.rs:106-110`). MEDIUM on the post-combat state-restoration mechanism (PlayerParty preservation rule at `dungeon/mod.rs:600-608` is in place, but the visual-tween / GridPosition snap interaction during combat round-trip needs explicit testing).

---

## Summary

Feature #16 lands on top of an unusually well-prepared foundation. The hard parts are already done by #2/#13/#15:

1. **`CurrentEncounter` shape is already specified** as #16's contract in `combat/turn_manager.rs:34-46`:
   ```rust
   #[derive(Resource, Debug, Clone)]
   pub struct CurrentEncounter {
       pub enemy_entities: Vec<Entity>,
       pub fleeable: bool,
   }
   ```
   #15 reads via `Option<Res<CurrentEncounter>>` and ships without it; #16 introduces the resource. Test fixtures and the `#[cfg(feature = "dev")] spawn_dev_encounter` (`turn_manager.rs:677-716`) bypass it. **#16 deletes the dev-stub spawner** when it ships its own.
2. **`MovedEvent` is the canonical step-event** (`dungeon/mod.rs:192-197`), declared as `Message` (NOT `Event`), registered via `app.add_message::<MovedEvent>()` in `DungeonPlugin::build` (`dungeon/mod.rs:224`). It carries `from`, `to`, `facing` — exactly what an encounter-roll system needs. The producer is `handle_dungeon_input` (`dungeon/mod.rs:897-902`), which writes only on **committed translation moves** (turn-only and wall-bumps emit nothing). **#16 subscribes via `MessageReader<MovedEvent>` in a system gated `.run_if(in_state(GameState::Dungeon))`** — same shape as `apply_pit_trap` / `apply_poison_trap` / `apply_alarm_trap` at `features.rs:368/420/459`.
3. **`CellFeatures::encounter_rate: f32`** already exists (`data/dungeon.rs:167-168`), serializable via the existing `floor_XX.dungeon.ron` pipeline. Per research §Pitfall 4, it's per-cell, data-driven, in `[0.0, 1.0]`. **No new asset schema** needed for the encounter rate itself — it's already authored.
4. **`EncounterRequested: Message` already exists** (`features.rs:125-128`) with an `EncounterSource` enum (`AlarmTrap`, plus an explicit `Future: Random / Foe` comment at line 133). **The `apply_alarm_trap` system already publishes it** (`features.rs:480-482`). #16's random-roll system writes the same message with `EncounterSource::Random` (a new variant). #16 then adds a single consumer system that reads `EncounterRequested`, populates `CurrentEncounter`, and transitions `GameState::Dungeon → Combat`.
5. **The `Dungeon → Combat → Dungeon` round-trip is already wired.** `despawn_dungeon_entities` preserves `PlayerParty` when transitioning to Combat (`dungeon/mod.rs:600-608`); `cleanup_party_after_combat` despawns it when leaving Combat for any destination *other than* Dungeon (`dungeon/mod.rs:572-591`); `spawn_party_and_camera` is idempotent when a party already exists (`dungeon/mod.rs:454-461`). The dungeon geometry is despawned and respawned on each transition. **#16 needs no new despawn/respawn logic** — it just needs to fire the state transition and provide `CurrentEncounter`.
6. **`rand 0.9.4` is already a direct dep** (`Cargo.toml:33`, `Cargo.lock:4361-4368`) with `features = ["std", "std_rng", "small_rng", "os_rng"]`. **No Step A/B/C gate needed** — #16 inherits #15's RNG plumbing directly.
7. **`rand_chacha 0.9.0` is already a dev-dep** (`Cargo.toml:36`, `Cargo.lock:4371-4378`). Tests use `ChaCha8Rng::seed_from_u64(...)` — verified working at `damage.rs::tests` and `turn_manager.rs::app_tests:813-815`.
8. **`bevy_common_assets::RonAssetPlugin`** is the canonical asset-loading pattern: 5 RON-typed loaders are already registered (`loading/mod.rs:106-110`) — `DungeonFloor`, `ItemDb`, `EnemyDb`, `ClassTable`, `SpellTable`. **#16 adds one more: `EncounterTable`** with the `encounters.ron` extension. The handle is added to `DungeonAssets` (an `AssetCollection`).
9. **`SfxKind::EncounterSting` is already wired** (`audio/sfx.rs:54`, asset at `audio/sfx/encounter_sting.ogg` declared at `loading/mod.rs:75-76`). #16 emits it on encounter-trigger; no audio work to do.
10. **`ActiveFloorNumber: Resource`** already exists (`dungeon/mod.rs:211-218`) and is the authoritative source for "which floor is the player on". #16 reads it to look up the right encounter table.

The encounter-roll algorithm is the canonical research §Code Examples shape (`research:1192-1214`):

```rust
encounter.steps_since_last += 1;
let rate = cell_features.encounter_rate * (1.0 + encounter.steps_since_last as f32 * 0.05);
if rng.gen::<f32>() < rate { /* trigger */ encounter.steps_since_last = 0; }
```

Adapted to Bevy 0.18 / rand 0.9 / Druum's plumbing:

- `rng.gen::<f32>()` becomes `rng.random::<f32>()` (rand 0.9 rename — same as `gen_range` → `random_range` in #15 D-I14).
- The step counter lives in a new `EncounterState: Resource`.
- The trigger writes `EncounterRequested { source: EncounterSource::Random }` rather than directly setting `NextState<GameState>::Combat` — this composes with the existing alarm-trap pathway and lets a single consumer (`handle_encounter_request`) be the SOLE writer of `CurrentEncounter` + the SOLE transition trigger. Single responsibility, easy to test.

Total scope: **+1 new file** under `src/plugins/combat/`: `encounter.rs` (encounter check, table loading, `CurrentEncounter` ownership, `EncounterState` resource, transition trigger, dev `?force_encounter` debug). **+1 new asset schema**: `EncounterTable` in `src/data/encounters.rs`. **+1 RON file** at minimum: `assets/encounters/floor_01.encounters.ron` (3-5 enemy groups). **Carve-outs**: `combat/mod.rs` (+1 line for `EncounterPlugin`), `combat/turn_manager.rs` (delete `spawn_dev_encounter` per Pitfall 1 of #15), `loading/mod.rs::DungeonAssets` (+1 field for the encounter table handle, +1 `RonAssetPlugin::<EncounterTable>::new(...)` registration), `dungeon/features.rs` (+1 enum variant `EncounterSource::Random`). **+6-10 tests** (within roadmap envelope of +6-10). **+0 new deps** — `rand 0.9` is already direct.

**Primary recommendation:** Implement #16 as a single sub-PR (~400-600 LOC) since it's already well-decomposed: encounter rolling and encounter-table authoring are tightly coupled, and the round-trip lifecycle is already established by #15. **Use the `EncounterRequested` message channel** rather than transitioning `GameState` directly — this matches the pattern locked in by `apply_alarm_trap` and gives #16 one observable path to test ("step on encounter cell → message gets written → consumer triggers transition") instead of two. **Keep `EncounterTable` as a separate `Asset` type** loaded via `bevy_common_assets::RonAssetPlugin` — same pattern as `DungeonFloor`, not embedded in `DungeonFloor` itself (separation of concerns; encounter tables can be reused across floors of the same biome).

---

## Standard Stack

### Core (already in tree — no new deps)

| Library | Version | Purpose | License | Why Standard |
|---------|---------|---------|---------|--------------|
| [bevy](https://crates.io/crates/bevy) | =0.18.1 | ECS, `Resource`, `Message`, `MessageReader`, `Asset` | MIT/Apache-2.0 | Already pinned (`Cargo.toml:10`) |
| [bevy_common_assets](https://crates.io/crates/bevy_common_assets) | =0.16.0 | `RonAssetPlugin::<EncounterTable>` for `*.encounters.ron` | MIT/Apache-2.0 | Already used for 5 other RON asset types (`loading/mod.rs:106-110`) |
| [bevy_asset_loader](https://crates.io/crates/bevy_asset_loader) | =0.26.0 | `#[asset(path = "encounters/floor_01.encounters.ron")]` in `DungeonAssets` | MIT/Apache-2.0 | Already wired in `loading/mod.rs:30-45` |
| [serde](https://crates.io/crates/serde) | 1.x | `#[derive(Serialize, Deserialize)]` on `EncounterTable`, `EnemyGroup`, `EnemySpec` | MIT/Apache-2.0 | Already direct dep |
| [ron](https://crates.io/crates/ron) | =0.12 | RON parsing/round-trip tests for the schema | MIT/Apache-2.0 | Already direct dep, used in `data/dungeon.rs::tests` for round-trip validation |
| [rand](https://crates.io/crates/rand) | =0.9.4 | `f32` roll, `WeightedIndex` for weighted enemy-group selection | MIT/Apache-2.0 | Already direct dep with `["std", "std_rng", "small_rng", "os_rng"]` features (`Cargo.toml:33`); `CombatRng` resource pattern at `turn_manager.rs:135` is the model |
| [rand_chacha](https://crates.io/crates/rand_chacha) | =0.9.0 | Seeded `ChaCha8Rng` for deterministic encounter-rate convergence tests | MIT/Apache-2.0 | Already dev-dep (`Cargo.toml:36`); pattern at `turn_manager.rs::app_tests:813-815` |

### Supporting (none required)

No new crates needed for #16. **Crucially: no `rand_distr`** — `WeightedIndex` is in `rand 0.9` itself at `rand::distr::weighted::WeightedIndex` (renamed from `rand::distributions::WeightedIndex` in 0.8). Verified via Cargo.lock: `rand_distr 0.5.1` is in the lock as a transitive dep of `bevy_math` but it's NOT needed for `WeightedIndex` — that's in core `rand`. **Do not add `rand_distr` as a direct dep**; the import is `use rand::distr::weighted::WeightedIndex;`.

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Custom weighted selection (loop summing weights) | `rand::distr::weighted::WeightedIndex` | `WeightedIndex` is canonical, pre-built, O(log n) sampling. Custom loops are O(n) and a known footgun on tie-handling. Use the library. |
| `EncounterTable` as separate asset | Embed `EnemyGroup` list inside `DungeonFloor` itself | Embedding couples encounter authoring to dungeon-floor authoring; separation lets a single encounter table be referenced by multiple floors (see `DungeonFloor.encounter_table: String` at `data/dungeon.rs:260` — already an indirection). The string is the table ID; the resolver looks up the matching `Handle<EncounterTable>`. Existing pattern, keep it. |
| Direct `NextState<GameState>::Combat` transition | `EncounterRequested` message + consumer | The message channel is a single observable seam (one place reads → one place transitions). Direct transition forks the codepath: alarm-trap and random both call `next.set(...)`. Tests can assert the message is written without coupling to state-machine timing. Keep the message. |
| `rand::thread_rng()` (per research example) | `Res<CombatRng>` shared with #15 | Druum already standardised on `CombatRng` for combat-side rolls. For encounters, **use a separate `Res<EncounterRng>`** (NOT `CombatRng`) — encounter rolls happen in `Dungeon` state where `CombatRng` may not be initialised, and merging the two would couple #16's testability to combat-state init. New resource, same `Box<dyn rand::RngCore + Send + Sync>` pattern. |

### Installation

**No installation step.** All deps are in tree.

---

## Architecture Options

The encounter-trigger pipeline has three architecturally distinct shapes. Choose one before writing code.

| Option | Description | Pros | Cons | Best When |
|--------|-------------|------|------|-----------|
| **A: Message-pipe (RECOMMENDED)** | `check_random_encounter` reads `MessageReader<MovedEvent>`, rolls, writes `EncounterRequested { source: Random }` on success. A separate `handle_encounter_request` system (in #16) reads `MessageReader<EncounterRequested>`, picks an `EnemyGroup`, spawns enemies, populates `CurrentEncounter`, transitions state. | Single transition path (one consumer); composes with `apply_alarm_trap`'s existing emit of `EncounterRequested`; tests can assert "encounter rolled → message written" independently of state-machine timing; the consumer is the SOLE writer of `CurrentEncounter`. | One extra layer of indirection: roll-system and resolver-system are separate. | Multiple sources can trigger an encounter (random rolls, alarm traps, future #22 FOEs) — this is exactly Druum's situation. |
| B: Direct state transition | `check_random_encounter` reads `MovedEvent`, rolls, on success directly calls `next.set(GameState::Combat)` and populates `CurrentEncounter` inline. | One system, fewer files. | Forks the encounter codepath — alarm-trap and random encounters do the same thing in two places; FOE encounters in #22 add a third. Drift bug risk (one path forgets to clear `EncounterState.steps_since_last`, the other clears it differently, etc.). | Single-source encounter system (no FOEs, no traps). Not Druum. |
| C: Inline-in-movement-handler | Roll happens inside `handle_dungeon_input` itself, before writing `MovedEvent`. | Step-correlation guaranteed (one system owns the entire "step happened → maybe encounter" loop). | Couples encounter logic to input handling; `handle_dungeon_input` already has `#[allow(clippy::too_many_arguments)]` and adding RNG + `EncounterState` makes it worse; breaks the input/movement/feature separation that #13 established. | Never recommended. Avoid. |

**Recommended: Option A — Message-pipe.** Composes with `apply_alarm_trap`'s existing path; FOE-suppression hook (see §FoeProximity below) is a single read-only resource check in the roll-system; Tests can independently verify the roll math (writes the message) and the resolver (reads the message → spawns enemies + transitions).

### Counterarguments

- **"One extra system file is overkill for #16's scope."** — Response: The extra system is ~30 LOC. The single-consumer invariant is what makes the post-combat restoration testable: you can fire `EncounterRequested` directly in tests without simulating the dungeon. The investment pays for itself the first time #22 (FOE) needs to also call `start_combat`.
- **"Why not just have `start_combat(enemy_group)` as a `pub fn` callable from anywhere?"** — Response: Functions can't transition Bevy state from arbitrary system contexts (you need `ResMut<NextState<GameState>>`). The message channel achieves the same callability while staying inside Bevy's idioms. `start_combat` becomes the consumer system, not a callable function.
- **"The roll-system writes a message; what if no one consumes it?"** — Response: `handle_encounter_request` is registered in the same `EncounterPlugin`. The plugin is the contract. The Bevy runtime guarantees the message is consumed within the same frame (or the next, depending on system ordering — and we explicitly order the consumer `.after(check_random_encounter)`).

---

## Architecture Patterns

### Recommended Project Structure

```
src/
├── data/
│   ├── encounters.rs           # NEW — EncounterTable, EnemyGroup, EnemySpec asset types
│   └── mod.rs                  # EDIT — add `pub mod encounters;` and re-export
├── plugins/
│   ├── combat/
│   │   ├── encounter.rs        # NEW — EncounterPlugin, EncounterState, CurrentEncounter,
│   │   │                       #       check_random_encounter, handle_encounter_request,
│   │   │                       #       force_encounter (dev), FoeProximity stub
│   │   ├── mod.rs              # EDIT — register EncounterPlugin
│   │   └── turn_manager.rs     # EDIT — delete spawn_dev_encounter (Pitfall 1)
│   ├── dungeon/
│   │   └── features.rs         # EDIT — add EncounterSource::Random variant
│   └── loading/
│       └── mod.rs              # EDIT — add Handle<EncounterTable> to DungeonAssets;
│                               #        register RonAssetPlugin::<EncounterTable>
assets/
└── encounters/
    └── floor_01.encounters.ron # NEW
```

### Pattern 1: Encounter-roll system (the canonical algorithm)

**What:** Per-step probability check with linear accumulator (research §Code Examples + §Pitfall 4).

**When to use:** This is THE pattern. Every step's `MovedEvent` triggers exactly one roll.

**Example:**
```rust
// Source: research/20260326-01-bevy-first-person-dungeon-crawler-rpg.md:1192-1214
//         adapted to Bevy 0.18 idioms + rand 0.9 + Druum plumbing.

use bevy::prelude::*;
use rand::Rng;

use crate::data::DungeonFloor;
use crate::plugins::dungeon::{ActiveFloorNumber, MovedEvent, floor_handle_for};
use crate::plugins::dungeon::features::{EncounterRequested, EncounterSource};
use crate::plugins::loading::DungeonAssets;

#[derive(Resource, Default, Debug, Clone)]
pub struct EncounterState {
    /// Steps taken since the last encounter resolution. Reset to 0 on encounter
    /// trigger AND on every `OnEnter(Dungeon)` (returning from combat doesn't
    /// preserve the soft-pity counter — design call, see Open Questions).
    pub steps_since_last: u32,
}

#[derive(Resource)]
pub struct EncounterRng(pub Box<dyn rand::RngCore + Send + Sync>);

impl Default for EncounterRng {
    fn default() -> Self {
        use rand::SeedableRng;
        Self(Box::new(rand::rngs::SmallRng::from_os_rng()))
    }
}

#[allow(clippy::too_many_arguments)]
pub fn check_random_encounter(
    mut moved: MessageReader<MovedEvent>,
    mut state: ResMut<EncounterState>,
    mut rng: ResMut<EncounterRng>,
    mut encounter: MessageWriter<EncounterRequested>,
    foe_proximity: Res<FoeProximity>,        // <-- #22 hook (stub Default in #16)
    dungeon_assets: Option<Res<DungeonAssets>>,
    floors: Res<Assets<DungeonFloor>>,
    active_floor: Res<ActiveFloorNumber>,
) {
    let Some(assets) = dungeon_assets else { return; };
    let floor_handle = floor_handle_for(&assets, active_floor.0);
    let Some(floor) = floors.get(floor_handle) else { return; };
    for ev in moved.read() {
        // FOE suppression — Pitfall 6 of roadmap line 876.
        if foe_proximity.suppresses_random_rolls() {
            continue;
        }
        // Each step bumps the accumulator FIRST.
        state.steps_since_last = state.steps_since_last.saturating_add(1);
        // Read per-cell encounter rate at the destination cell.
        let cell = &floor.features[ev.to.y as usize][ev.to.x as usize];
        if cell.encounter_rate <= 0.0 {
            continue;
        }
        // Soft-pity scaling: each missed step bumps probability 5%.
        // Formula from research §Code Examples (line 1204-1205).
        let rate = cell.encounter_rate
            * (1.0 + state.steps_since_last as f32 * 0.05);
        // rand 0.9 rename: rng.gen::<f32>() → rng.random::<f32>().
        if rng.0.random::<f32>() < rate {
            state.steps_since_last = 0;
            encounter.write(EncounterRequested {
                source: EncounterSource::Random,
            });
        }
    }
}
```

**Anti-pattern note:** Do NOT bump `steps_since_last` only on success. The accumulator must rise on every step regardless of outcome — that's the whole point of soft-pity.

### Pattern 2: Weighted enemy-group selection (`WeightedIndex`)

**What:** Given an `EncounterTable { entries: Vec<(weight, EnemyGroup)> }`, pick one entry by weight.

**When to use:** Inside the `handle_encounter_request` consumer when populating `CurrentEncounter`.

**Example:**
```rust
// rand 0.9 `WeightedIndex` lives at `rand::distr::weighted::WeightedIndex`
// (renamed from `rand::distributions::WeightedIndex` in 0.8).
// Verified: rand 0.9.4 in Cargo.lock:4361-4368.

use rand::distr::weighted::WeightedIndex;
use rand::prelude::Distribution;

pub fn pick_enemy_group<'a>(
    table: &'a EncounterTable,
    rng: &mut (impl rand::Rng + ?Sized),       // <-- ?Sized per #15 D-I13
) -> Option<&'a EnemyGroup> {
    if table.entries.is_empty() { return None; }
    // WeightedIndex returns Err if all weights are zero.
    let weights = table.entries.iter().map(|e| e.weight);
    let dist = WeightedIndex::new(weights).ok()?;
    let idx = dist.sample(rng);
    Some(&table.entries[idx].group)
}
```

**Note on `?Sized`:** `CombatRng`/`EncounterRng` is a `Box<dyn rand::RngCore + Send + Sync>` — a DST. Functions accepting `&mut impl Rng` need `?Sized` to permit `&mut *rng.0` to satisfy the bound. This is locked-in by #15 D-I13 (`damage.rs:67` and `targeting.rs:37`). #16 follows the same convention.

### Pattern 3: `EncounterRequested` consumer (the `start_combat` entry point)

**What:** Single system that owns `CurrentEncounter` writes and the `Dungeon → Combat` transition.

**When to use:** This IS the `start_combat(enemy_group)` API. It's a Bevy system, not a function — it's invoked indirectly via the `EncounterRequested` message.

**Example:**
```rust
#[derive(Resource, Debug, Clone)]
pub struct CurrentEncounter {
    pub enemy_entities: Vec<Entity>,
    pub fleeable: bool,
}

#[allow(clippy::too_many_arguments)]
pub fn handle_encounter_request(
    mut requests: MessageReader<EncounterRequested>,
    mut commands: Commands,
    mut next_state: ResMut<NextState<GameState>>,
    encounter_tables: Res<Assets<EncounterTable>>,
    encounter_table_handle: Res<CurrentEncounterTableHandle>, // #16-owned, set OnEnter(Dungeon)
    mut rng: ResMut<EncounterRng>,
    mut sfx: MessageWriter<SfxRequest>,
) {
    // Bevy 0.18: read all pending requests and process at most one (encounter
    // collisions on the same frame collapse to a single combat).
    let Some(req) = requests.read().next() else { return; };
    // Pick the enemy group from the active floor's table.
    let Some(table) = encounter_tables.get(&encounter_table_handle.0) else {
        warn!("Encounter triggered but EncounterTable not loaded; skipping");
        return;
    };
    let Some(group) = pick_enemy_group(table, &mut *rng.0) else {
        warn!("Encounter triggered but table is empty; skipping");
        return;
    };
    // Spawn enemies. EnemyBundle is the existing post-#15 type at
    // src/plugins/combat/enemy.rs:39-51.
    let mut entities = Vec::with_capacity(group.enemies.len());
    for (idx, spec) in group.enemies.iter().enumerate() {
        let entity = commands.spawn(EnemyBundle {
            name: EnemyName(spec.name.clone()),
            index: EnemyIndex(idx as u32),
            base_stats: spec.base_stats,
            derived_stats: spec.derived_stats,
            ai: spec.ai,
            ..Default::default()  // Equipment::default(), Experience::default() per D-A5
        }).id();
        entities.push(entity);
    }
    // Populate CurrentEncounter — single source of truth.
    commands.insert_resource(CurrentEncounter {
        enemy_entities: entities,
        fleeable: matches!(req.source, EncounterSource::Random | EncounterSource::AlarmTrap),
    });
    // Audio cue.
    sfx.write(SfxRequest { kind: SfxKind::EncounterSting });
    // State transition — #15's CombatPlugin takes over from here.
    next_state.set(GameState::Combat);
}
```

**Critical:** Use `requests.read().next()` and discard the rest if multiple `EncounterRequested` arrived in the same frame (alarm trap + random roll on the same step → still one combat, not two). Document this with a comment.

### Pattern 4: Asset-loading via `RonAssetPlugin` (mirrors `DungeonFloor`)

**What:** Register `EncounterTable` as a typed RON asset.

**Where:** `loading/mod.rs:106-110` is the existing pattern. **Add one line** for `EncounterTable`.

**Example:**
```rust
// In loading/mod.rs LoadingPlugin::build:
.add_plugins((
    RonAssetPlugin::<DungeonFloor>::new(&["dungeon.ron"]),
    RonAssetPlugin::<ItemDb>::new(&["items.ron"]),
    RonAssetPlugin::<EnemyDb>::new(&["enemies.ron"]),
    RonAssetPlugin::<ClassTable>::new(&["classes.ron"]),
    RonAssetPlugin::<SpellTable>::new(&["spells.ron"]),
    RonAssetPlugin::<EncounterTable>::new(&["encounters.ron"]),  // <-- NEW
))
```

And in the `DungeonAssets` `AssetCollection` struct (same file, lines 30-45):

```rust
#[derive(AssetCollection, Resource)]
pub struct DungeonAssets {
    #[asset(path = "dungeons/floor_01.dungeon.ron")]
    pub floor_01: Handle<DungeonFloor>,
    #[asset(path = "dungeons/floor_02.dungeon.ron")]
    pub floor_02: Handle<DungeonFloor>,
    #[asset(path = "encounters/floor_01.encounters.ron")]   // <-- NEW
    pub encounters_floor_01: Handle<EncounterTable>,         // <-- NEW
    // ... existing item_db, enemy_db, class_table, spell_table ...
}
```

**Floor-id resolution:** `DungeonFloor.encounter_table: String` (`data/dungeon.rs:260`) gives the table identifier; `ActiveFloorNumber` (`dungeon/mod.rs:211-218`) gives the floor number. Resolve `Handle<EncounterTable>` from a `floor_handle_for`-style match arm — the same shape as `dungeon/mod.rs:392-401`. **Recommendation:** add a `pub(crate) fn encounter_table_for(assets: &DungeonAssets, floor_number: u32) -> &Handle<EncounterTable>` in `loading/mod.rs` (NOT in `combat/encounter.rs` — keeps the asset-resolution layer co-located with the asset declarations). #16 ships floor_01 only; future floors add match arms.

### Pattern 5: `OnEnter(Dungeon)` reset for the step-counter

**What:** Reset `EncounterState.steps_since_last = 0` on every `OnEnter(GameState::Dungeon)`.

**When to use:** Returning from combat.

**Why:** Without the reset, the soft-pity counter persists across combats — a player who survived a 30-step combat-free streak, fought, and returned would hit a near-guaranteed encounter on their next step. The roadmap §Pitfall 4 wants the soft-pity to be PER-DUNGEON-RUN, not GLOBAL.

**Open question for the planner:** Do we reset `steps_since_last` on combat-end (return to Dungeon), or preserve it for "continuous tension"? See §Open Questions.

**Example:**
```rust
// In EncounterPlugin::build:
app.add_systems(OnEnter(GameState::Dungeon), reset_encounter_state);

fn reset_encounter_state(mut state: ResMut<EncounterState>) {
    state.steps_since_last = 0;
}
```

### Anti-Patterns to Avoid

- **Bumping `steps_since_last` only on success.** Soft-pity requires the counter to rise on every step. Reset to 0 on trigger; bump otherwise.
- **Reading `MovedEvent` in `Update` outside `GameState::Dungeon`.** The roll system MUST be `.run_if(in_state(GameState::Dungeon))` — otherwise you'll roll encounters during combat (which writes `MovedEvent` if there's any visual lerp left in the message buffer). Same shape as `apply_pit_trap` at `features.rs:166`.
- **Writing `EncounterRequested` from inside the message-reader loop without a guard.** A multi-step `MovedEvent` batch (which Druum doesn't produce, but defensive) could write multiple `EncounterRequested` per frame. **The consumer takes only the first** (`requests.read().next()`) — but it's cleaner if the producer emits at most one per frame too. Add a `let mut already_rolled = false;` guard inside `check_random_encounter`.
- **Embedding enemy stats inside the encounter table.** This duplicates `EnemyDb` (the asset type already declared at `data/enemies.rs:8-11`, scheduled to be filled out by #17). For #16, **the encounter table references enemy IDs** (strings), and the consumer looks up the matching `EnemySpec` from `EnemyDb`. Until #17 ships `EnemyDb`, **#16 inlines minimal `EnemySpec { name, base_stats, derived_stats, ai }` directly in the encounter table** (mirrors the `#[cfg(feature = "dev")] spawn_dev_encounter` shape at `turn_manager.rs:683-714`). This is a temporary inlining — when #17 lands, #16's RON files migrate to ID references.
- **Manually populating `CurrentEncounter` from outside `handle_encounter_request`.** That system is the SOLE writer. Tests inject `EncounterRequested` to drive it. Never directly construct a `CurrentEncounter` resource in production code.
- **Using `commands.entity(e).despawn()` on enemies inside `OnExit(Combat)`.** That's not #16's job — `CombatPlugin` (#15) owns the combat-end despawn. #16's only cleanup responsibility is the `CurrentEncounter` resource (`commands.remove_resource::<CurrentEncounter>()` on `OnExit(Combat)`).

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Weighted random selection | Manual cumulative-sum loop with `rng.random_range(0..total)` | `rand::distr::weighted::WeightedIndex::new(weights)` | The library handles edge cases (zero weights, single entry), uses a sorted prefix table for O(log n) sampling, and is well-tested. A handrolled loop is a footgun on the all-zero-weights case. |
| RON deserialization | `ron::de::from_reader` at runtime | `bevy_common_assets::RonAssetPlugin` | Already wired for 5 asset types in `loading/mod.rs:106-110`. Hot-reload (`features = ["dev"]` enables `bevy/file_watcher`) works automatically. Manual loading bypasses Bevy's asset lifecycle. |
| Float roll | `rand::random::<f32>() < rate` (top-level fn) | `rng.random::<f32>() < rate` on a `Box<dyn RngCore + Send + Sync>` | Top-level `rand::random` reads `thread_rng`, which is non-deterministic across test runs. The boxed-dyn pattern lets tests inject `ChaCha8Rng::seed_from_u64(42)`. Same shape as `CombatRng` (`turn_manager.rs:135`). |
| State transition observation | Manually polling `Res<State<GameState>>` in tests | `app.update()` and assert against the post-update state | Bevy's state-transition timing has a one-frame deferral; manual polling is racy. The pattern at `loading/mod.rs::tests:244-267` is the authoritative model. |

---

## Common Pitfalls

### Pitfall 1: `MessageReader<MovedEvent>` not registered in test apps

**What goes wrong:** Tests that spin up `EncounterPlugin` without `DungeonPlugin` panic on `MessageReader<MovedEvent>::messages` validation.

**Why it happens:** `app.add_message::<MovedEvent>()` runs in `DungeonPlugin::build` (`dungeon/mod.rs:224`). Combat-side test apps register it explicitly via `app.add_message::<crate::plugins::dungeon::MovedEvent>();` — see `turn_manager.rs::app_tests:802`.

**How to avoid:** Every test harness in `combat/encounter.rs::app_tests` MUST call `app.add_message::<MovedEvent>()` before running. This is exactly the same fix as #15 D-I16 (which traced this same panic for `tick_on_dungeon_step`). Document it with a comment:
```rust
// MovedEvent is owned by DungeonPlugin (mod.rs:224); register it here so
// MessageReader<MovedEvent> doesn't panic when DungeonPlugin isn't loaded.
app.add_message::<crate::plugins::dungeon::MovedEvent>();
```

### Pitfall 2: Encounter table's enemy specs out-of-sync with `EnemyBundle` shape

**What goes wrong:** RON encodes `EnemySpec { hp: 30 }` but `EnemyBundle` requires `derived_stats: DerivedStats { current_hp: 30, max_hp: 30, ... }`. The deserialized data structure doesn't fit the bundle.

**Why it happens:** Two layers of "enemy authoring" — the encounter-table-level `EnemySpec` and the ECS-level `EnemyBundle` — must agree on field shapes.

**How to avoid:** `EnemySpec` carries `BaseStats` and `DerivedStats` directly (the same types from `party::character`). The conversion in `handle_encounter_request` is mechanical (`spec.base_stats` → `bundle.base_stats`). NO ad-hoc `hp: u32` field on `EnemySpec`. Round-trip tests assert `EncounterTable → EnemySpec → EnemyBundle::default()` preserves all fields.

### Pitfall 3: `steps_since_last` not reset on cross-floor teleport

**What goes wrong:** Player teleports to floor_02 (which has a totally different encounter rate); their accumulator is still high from floor_01 → instant encounter.

**Why it happens:** Cross-floor teleport goes `Dungeon → Loading → TitleScreen → Dungeon` (per `loading/mod.rs:152-181`). The intermediate transitions don't naturally reset `EncounterState`.

**How to avoid:** Reset `steps_since_last` in `OnEnter(GameState::Dungeon)`. This catches both the normal combat-return AND the teleport path (both re-enter Dungeon). Documented at Pattern 5 above. Add a test that simulates Dungeon → Combat → Dungeon and asserts `steps_since_last == 0` post-return.

### Pitfall 4: `MovedEvent` reader cursor not draining when consumer skips

**What goes wrong:** If `dungeon_assets.is_none()` (early-loading frame), `check_random_encounter` returns without `moved.read()` — the reader cursor doesn't advance, and the same `MovedEvent` is re-read on the next frame.

**Why it happens:** Bevy's `MessageReader` is per-system; if a system bails before iterating, its cursor stays put. The next frame re-iterates the same events.

**How to avoid:** Always drain the cursor, even on early returns. Pattern (mirror of `audio/sfx.rs:73-78`):
```rust
let Some(assets) = dungeon_assets else {
    for _ in moved.read() {} // drain cursor
    return;
};
```
But this is academic for #16 — `DungeonAssets` is populated before `OnEnter(Dungeon)` (per `loading/mod.rs:120`), so `dungeon_assets.is_none()` only fires during the brief Loading → Dungeon transition where no `MovedEvent` is produced. **Defensive draining is recommended but not load-bearing.**

### Pitfall 5: `EncounterRequested` consumer races with same-frame movement

**What goes wrong:** Player steps on encounter cell, `MovedEvent` fires, `check_random_encounter` writes `EncounterRequested`, but `handle_encounter_request` runs BEFORE `check_random_encounter` (Bevy's system scheduler doesn't guarantee order without explicit `.before` / `.after`). The encounter is consumed on the next frame, after the player has already taken another step.

**How to avoid:** Order explicitly:
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

### Pitfall 6: `CurrentEncounter` not removed on combat-end

**What goes wrong:** Player wins combat, returns to dungeon, takes one step, hits another encounter. The PREVIOUS combat's `CurrentEncounter` is still in the world; the new one overwrites it (correct), but tests might assert "no CurrentEncounter exists post-combat" and fail.

**How to avoid:** `OnExit(GameState::Combat)`: `commands.remove_resource::<CurrentEncounter>()`. Document explicitly. Test asserts absence of resource post-combat. (Note: `CombatPlugin`'s `clear_combat_state` at `turn_manager.rs:215-223` doesn't touch `CurrentEncounter` — that's because #15 doesn't own it. #16 adds the cleanup.)

### Pitfall 7: Per-cell `encounter_rate = 0.0` cells break the soft-pity contract

**What goes wrong:** Designer authors a 100-cell corridor with `encounter_rate = 0.0`; the soft-pity counter rises to 100 across that corridor; the very first cell with `encounter_rate = 0.01` triggers an instant encounter (1.0 * 0.05 * 100 = 5.0 → guaranteed).

**Why it happens:** The accumulator multiplies the cell rate. Cells with rate `0.0` correctly don't trigger (multiply by zero), but they also don't reset. The next non-zero cell pays for all the accumulated steps.

**How to avoid:** Multiple options:
- **Option A (recommended):** Cap the accumulator multiplier at `2.0` or `3.0` (prevents unbounded growth). The research formula `rate * (1.0 + steps * 0.05)` has no cap — add `.min(3.0)` (or whatever).
- **Option B:** Skip the `steps_since_last` bump on `encounter_rate == 0.0` cells. Designer-friendlier; "safe corridors" don't accumulate tension.
- **Option C:** Reset `steps_since_last` on encounter_rate-zero cells. Most aggressive — corridors are "rest".

This is a planner decision (see §Open Questions). The roadmap doesn't specify; research §Pitfall 4 only says "guaranteed encounter after enough steps", not how the accumulator interacts with rate-zero cells.

### Pitfall 8: `force_encounter` debug command competing with the dev-stub spawner

**What goes wrong:** `#[cfg(feature = "dev")] spawn_dev_encounter` (at `turn_manager.rs:677-716`) ALWAYS runs on `OnEnter(Combat)`. If `?force_encounter` triggers a transition to Combat AND the dev spawner runs, you get the test enemies PLUS the spawned-by-#16 enemies.

**How to avoid:** **Phase 1 of #16 deletes `spawn_dev_encounter` per Pitfall 1 of #15's plan** (which explicitly notes "#16 deletes this stub"). The plan locks this carve-out. After deletion, `?force_encounter` is the sole dev path to combat.

---

## Security

### Known Vulnerabilities

No known CVEs or advisories for the recommended libraries as of 2026-05-08. `rand 0.9.4` is the latest stable; `rand_chacha 0.9.0` is the latest stable; `bevy_common_assets 0.16.0` is in tree. **Action: monitor `cargo audit` post-implementation; flag any new advisory.**

| Library | CVE / Advisory | Severity | Status | Action |
|---------|---------------|----------|--------|--------|
| rand 0.9.4 | None found | — | — | Monitor |
| rand_chacha 0.9.0 | None found | — | — | Monitor |
| bevy_common_assets 0.16.0 | None found | — | — | Monitor |

### Architectural Security Risks

| Risk | Affected Architecture | How It Manifests | Secure Pattern | Anti-Pattern to Avoid |
|------|----------------------|------------------|----------------|----------------------|
| Malicious encounter-table RON | `RonAssetPlugin::<EncounterTable>` | A crafted RON file with extreme stat values (`current_hp: u32::MAX`, `attack: u32::MAX`) could overflow damage calc or DoS via gigantic combat resolution loops | Saturating arithmetic in `damage_calc` (already locked by #15 D-A3 / Pitfall 7); validate `EnemyGroup.enemies.len() <= MAX_ENEMIES_PER_ENCOUNTER` (suggest 8) on load; clamp weights to a sane range (1..=10000) | Trusting RON-deserialized values without bounds checks; iterating an unbounded `enemies` Vec in the spawner |
| Resource exhaustion via encounter loop | `check_random_encounter` | Adversarial encounter rate of `1.5` (out of `[0.0, 1.0]` spec) and step counter overflow could chain encounters every frame | Clamp `cell.encounter_rate.clamp(0.0, 1.0)` on read (defensive — RON authors might typo); cap accumulator multiplier (Pitfall 7) | Accepting raw f32 from asset without clamping |
| Combat-state leak across encounters | `CurrentEncounter` resource | If the previous combat's `CurrentEncounter` survives, its enemy entities (despawned by `clear_combat_state`) become dangling `Entity` references when the next encounter starts | `OnExit(Combat)`: `commands.remove_resource::<CurrentEncounter>()` (Pitfall 6) | Leaving the resource in place "for save-game compatibility" — that's #23's job |

### Trust Boundaries

- **`assets/encounters/*.encounters.ron` (RON deserialization):** Validated by serde at parse time (type-checks all fields). Additional validation in #16: `enemies.len() <= MAX_ENEMIES_PER_ENCOUNTER`, `weight > 0`, `encounter_rate.clamp(0.0, 1.0)`. **Failure mode:** if validation fails, log warning and skip that entry — never panic.
- **`MovedEvent` from `dungeon::handle_dungeon_input`:** Trusted (internal producer). No validation needed.
- **`EncounterRequested` source enum:** Pattern-matched on consumer; `Random | AlarmTrap` fleeable, future `Foe { boss: bool }` non-fleeable. Match must be exhaustive.

---

## Performance

| Metric | Value / Range | Source | Notes |
|--------|---------------|--------|-------|
| Encounter roll per step | < 1 µs | rand 0.9 benchmarks | One f32 RNG call + one comparison + (rare) WeightedIndex sample. Negligible. |
| `EncounterTable` deserialization | < 10 ms for a 100-entry table | bevy_common_assets / serde RON | One-shot at load time; not in the hot loop. Asset is `Handle<EncounterTable>`-cached. |
| `MovedEvent` consumer overhead | < 10 µs / frame | Bevy ECS overhead | One MessageReader + one `Res<EncounterState>` + one `Res<DungeonAssets>` access. Frame budget impact: 0%. |
| Memory: `EncounterTable` (5 entries × 3 enemies each) | ~2 KB | Estimate | Vec of 5 `(u32, EnemyGroup)` tuples; `EnemyGroup` is a Vec of `EnemySpec` (~120 bytes each). Negligible. |

**Performance is NOT a concern for #16.** The encounter check fires at most once per player step (~1/sec at human input rates). All operations are O(1) or O(log n) for the weighted sample.

---

## Code Examples

### `EncounterTable` asset schema (data/encounters.rs)

```rust
// Source: drafted for Druum from research §Pattern 2 conventions.
//         Mirrors the DungeonFloor RON-asset shape at data/dungeon.rs:249-272.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::plugins::combat::ai::EnemyAi;
use crate::plugins::party::character::{BaseStats, DerivedStats};

/// One entry in an encounter table — a weight + enemy group.
///
/// Weight is u32 (not f32) for byte-stable RON round-trips.
/// `WeightedIndex::new` accepts any iterator of summable weights.
#[derive(Reflect, Serialize, Deserialize, Debug, Clone)]
pub struct EncounterEntry {
    pub weight: u32,
    pub group: EnemyGroup,
}

/// A group of enemies spawned together for one encounter.
///
/// `enemies.len()` must be in `1..=MAX_ENEMIES_PER_ENCOUNTER` (8 in v1).
/// The handler validates on spawn; oversized groups are truncated with a warning.
#[derive(Reflect, Serialize, Deserialize, Debug, Clone)]
pub struct EnemyGroup {
    pub enemies: Vec<EnemySpec>,
}

/// Inline enemy spec for #16. Fields mirror EnemyBundle (combat/enemy.rs:39-51).
///
/// **Temporary inlining:** Until #17 ships `EnemyDb`, encounter tables carry
/// full enemy stats inline. When #17 lands, this becomes `enemy_id: String`
/// referencing `EnemyDb` entries.
#[derive(Reflect, Serialize, Deserialize, Debug, Clone)]
pub struct EnemySpec {
    pub name: String,
    pub base_stats: BaseStats,
    pub derived_stats: DerivedStats,
    /// Defaults to `EnemyAi::RandomAttack` (Decision D-Q5=A).
    #[serde(default)]
    pub ai: EnemyAi,
}

/// One floor's encounter table. Loaded by `RonAssetPlugin::<EncounterTable>`.
#[derive(Asset, Reflect, Serialize, Deserialize, Default, Debug, Clone)]
pub struct EncounterTable {
    /// Identifier — matches `DungeonFloor.encounter_table` (data/dungeon.rs:260).
    pub id: String,
    pub entries: Vec<EncounterEntry>,
}
```

**Note on `Reflect` derives:** Per `feedback_bevy_reflect_018_derive.md` precedent, `#[derive(Reflect)]` handles `Vec<T>` and `Option<T>` shapes for typical asset types. Just add the derive — no `#[reflect(...)]` attributes needed.

### `floor_01.encounters.ron` (the first authored content)

```ron
// assets/encounters/floor_01.encounters.ron
// 3-5 enemy groups per the roadmap envelope (line 894).
// Weights sum to 100 for designer ergonomics — but `WeightedIndex` accepts any positive sum.

(
    id: "b1f_encounters",
    entries: [
        // Common: Single Goblin (weight 50)
        (
            weight: 50,
            group: (
                enemies: [
                    (
                        name: "Goblin",
                        base_stats: (strength: 8, intelligence: 4, piety: 4, vitality: 8, agility: 6, luck: 4),
                        derived_stats: (
                            max_hp: 30, current_hp: 30, max_mp: 0, current_mp: 0,
                            attack: 8, defense: 5, magic_attack: 0, magic_defense: 2,
                            speed: 6, accuracy: 60, evasion: 5,
                        ),
                        ai: RandomAttack,
                    ),
                ],
            ),
        ),
        // Uncommon: Pair of Goblins (weight 30)
        (
            weight: 30,
            group: (
                enemies: [
                    /* same Goblin spec, twice */
                ],
            ),
        ),
        // Rare: Goblin Captain (weight 15)
        (
            weight: 15,
            group: (
                enemies: [
                    (
                        name: "Goblin Captain",
                        base_stats: (strength: 12, intelligence: 4, piety: 4, vitality: 12, agility: 6, luck: 4),
                        derived_stats: (
                            max_hp: 60, current_hp: 60, max_mp: 0, current_mp: 0,
                            attack: 12, defense: 8, magic_attack: 0, magic_defense: 2,
                            speed: 6, accuracy: 70, evasion: 5,
                        ),
                        ai: BossFocusWeakest,
                    ),
                ],
            ),
        ),
        // Very rare: Cave Spider (weight 5)
        (
            weight: 5,
            group: (
                enemies: [
                    /* fast, low-HP spider — design-tunable */
                ],
            ),
        ),
    ],
)
```

### `?force_encounter` debug command (dev-only)

The roadmap (line 905) asks for a `?force_encounter` debug command. **No leafwing entry exists for this** (the `DungeonAction` enum at `input/mod.rs:74-85` is frozen post-#13). The cleanest pattern matches the existing F9 debug cycler (`state/mod.rs:71-89`) — a `#[cfg(feature = "dev")]` system that reads `Res<ButtonInput<KeyCode>>` directly without going through leafwing.

```rust
// In encounter.rs:
#[cfg(feature = "dev")]
fn force_encounter_on_f7(
    keys: Res<ButtonInput<KeyCode>>,
    state: Res<State<GameState>>,
    mut encounter: MessageWriter<EncounterRequested>,
) {
    if !matches!(state.get(), GameState::Dungeon) { return; }
    if keys.just_pressed(KeyCode::F7) {
        info!("DEV: Forcing encounter via F7");
        encounter.write(EncounterRequested {
            source: EncounterSource::Random,
        });
    }
}

// In EncounterPlugin::build:
#[cfg(feature = "dev")]
app.add_systems(Update, force_encounter_on_f7.run_if(in_state(GameState::Dungeon)));
```

Mirror the F9 precedent at `state/mod.rs:71-89` exactly. **Do NOT** add a `DungeonAction::ForceEncounter` variant to the leafwing enum — that file is FROZEN by #5 (per `feedback_third_party_crate_step_a_b_c_pattern.md` carve-out discipline).

### Deterministic test harness

```rust
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
            crate::plugins::combat::CombatPlugin,
            crate::plugins::dungeon::features::CellFeaturesPlugin,
            // NB: NOT DungeonPlugin — we register MovedEvent manually below
            // to avoid spinning up geometry / camera systems for unit tests.
        ));
        app.init_asset::<crate::data::DungeonFloor>();
        app.init_asset::<crate::data::ItemDb>();
        app.init_asset::<crate::data::ItemAsset>();
        app.init_asset::<crate::data::EncounterTable>();
        app.add_message::<crate::plugins::dungeon::MovedEvent>();
        app.init_resource::<crate::plugins::dungeon::ActiveFloorNumber>();
        app
    }

    fn seed_test_rng(app: &mut App, seed: u64) {
        let rng = rand_chacha::ChaCha8Rng::seed_from_u64(seed);
        app.world_mut().insert_resource(EncounterRng(Box::new(rng)));
    }

    /// Probability convergence test (seed-stable across runs).
    /// Verifies that over N steps with rate=0.10 and no soft-pity reset,
    /// the trigger count is within expected bounds.
    #[test]
    fn encounter_rate_converges_with_seeded_rng() {
        let mut app = make_test_app();
        seed_test_rng(&mut app, 42);
        // Set up 1×N corridor with rate=0.10 on every cell.
        // Walk N steps; count encounter triggers.
        // Assert count is within expected range for seed 42 (deterministic).
        // The exact assertion value is captured on first successful run.
        // ... implementation ...
    }
}
```

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `rand::thread_rng()` calls scattered throughout | `Res<EncounterRng>` boxed-dyn | Druum convention, locked by #15 | Tests inject `ChaCha8Rng::seed_from_u64(...)` for byte-stable determinism |
| `rng.gen::<f32>()` / `gen_range(...)` | `rng.random::<f32>()` / `random_range(...)` | rand 0.9 (renamed in 0.9 release) | Required by #15 D-I14; #16 follows |
| `rand::distributions::WeightedIndex` | `rand::distr::weighted::WeightedIndex` | rand 0.9 (module restructure) | Import path change; same API |
| `EncounterTable` embedded in dungeon file | Standalone `*.encounters.ron` asset | Druum convention | Encounter tables can be reused across floors of the same biome; `DungeonFloor.encounter_table: String` is the indirection |
| `next.set(GameState::Combat)` direct | `EncounterRequested` message + consumer | Druum convention, locked by #13 | One observable seam; testable independently of state-machine timing |

**Deprecated/outdated patterns to avoid:**
- `rand 0.8` API surface — Druum is on `rand 0.9.4`. Search for `gen_range` / `gen::<f32>` / `rand::distributions` and update.
- Direct `Res<Assets<DungeonFloor>>` indexing of `floor_01` — use `floor_handle_for(&assets, active_floor.0)` (`dungeon/mod.rs:392-401`).
- Embedding combat-spawn logic outside `combat/` — #16 lives in `combat/encounter.rs`, even though the *check* is during dungeon exploration. Rationale: the producer (encounter check) and the consumer (combat trigger) are tightly coupled; co-locating them prevents seam drift.

---

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | Rust built-in `#[cfg(test)]` + `cargo test` |
| Config file | None — Cargo.toml configuration |
| Quick run command | `cargo test --lib combat::encounter::tests` |
| Full suite command | `cargo test` (default) / `cargo test --features dev` (dev surface) |

### Requirements → Test Map

| Requirement | Behavior | Test Type | Automated Command | File Exists? |
|-------------|----------|-----------|-------------------|--------------|
| Encounter probability converges to expected rate | Run 10000 seeded steps at rate=0.10; assert trigger count ≈ 1000 ± 200 | Layer 1 (pure) | `cargo test --lib encounter_rate_converges` | ❌ needs creating |
| Soft-pity counter resets on trigger | Trigger encounter; assert `steps_since_last == 0` | Layer 2 (App) | `cargo test --lib steps_reset_on_trigger` | ❌ needs creating |
| Soft-pity counter resets on `OnEnter(Dungeon)` | Walk 30 steps; transition Dungeon → Combat → Dungeon; assert `steps_since_last == 0` | Layer 2 (App) | `cargo test --lib steps_reset_on_dungeon_entry` | ❌ needs creating |
| `WeightedIndex` picks proportional to weight | Run 10000 selections on a 50/30/15/5 table; assert empirical proportions | Layer 1 (pure) | `cargo test --lib weighted_index_proportional` | ❌ needs creating |
| `EncounterTable` RON round-trip | Serialize → deserialize → compare | Layer 1 (pure) | `cargo test --lib encounter_table_round_trip` | ❌ needs creating |
| `floor_01.encounters.ron` parses cleanly | Load file via `ron::de::from_str`; assert validity | Layer 1 (pure) | `cargo test --lib floor_01_encounters_loads` | ❌ needs creating |
| `EncounterRequested` triggers state transition | Inject message; run frame; assert `state == Combat` | Layer 2 (App) | `cargo test --lib encounter_request_triggers_combat` | ❌ needs creating |
| `CurrentEncounter` populated on transition | Trigger encounter; assert `Res<CurrentEncounter>::enemy_entities.len() > 0` | Layer 2 (App) | `cargo test --lib current_encounter_populated` | ❌ needs creating |
| Post-combat returns to same grid position | Spawn party at (5, 7) facing North; trigger encounter; victory; assert party still at (5, 7) facing North | Layer 2 (App) | `cargo test --lib post_combat_position_preserved` | ❌ needs creating |
| `?force_encounter` (F7 in dev) triggers transition | Inject F7 keypress in dev features; assert `state == Combat` | Layer 2 (dev-only) | `cargo test --features dev --lib force_encounter_on_f7` | ❌ needs creating |
| `FoeProximity::suppresses_random_rolls()` blocks roll | Set `FoeProximity::active = true`; walk 100 steps; assert zero encounters | Layer 2 (App) | `cargo test --lib foe_proximity_suppresses_rolls` | ❌ needs creating |

### Gaps (files to create before implementation)

- [ ] `src/plugins/combat/encounter.rs` — main module with all systems + `#[cfg(test)] mod tests` + `#[cfg(test)] mod app_tests`
- [ ] `src/data/encounters.rs` — schema module with round-trip tests
- [ ] `assets/encounters/floor_01.encounters.ron` — content
- [ ] (No new test config — uses existing Cargo.toml setup)

### Determinism strategy

For probability convergence tests, use the Druum-canonical pattern from `damage.rs::tests` and `turn_manager.rs::app_tests:813-815`:

```rust
fn seed_test_rng(app: &mut App, seed: u64) {
    let rng = rand_chacha::ChaCha8Rng::seed_from_u64(seed);
    app.world_mut().insert_resource(EncounterRng(Box::new(rng)));
}
```

Three convergence tests recommended:
1. **Tight bounds at high N:** N=10000 steps at rate=0.10 → expect ~1000 triggers; assert `[800, 1200]`.
2. **Soft-pity sanity:** N=200 steps at rate=0.01 → without soft-pity ~2 triggers; with soft-pity (5%/step) → mathematically guaranteed by step ~100 (`(1.0 + 100*0.05) * 0.01 = 0.06` per step, accumulating to ~1.0). Assert at least one trigger.
3. **Reset-on-trigger:** Run until first trigger, capture `steps_since_last_at_trigger`; assert post-trigger `steps_since_last == 0`.

---

## API Contract with Feature #15

This is the most important hand-off. Get the signature exactly right.

### `CurrentEncounter` shape (locked by #15)

Already documented at `combat/turn_manager.rs:34-46`:

```rust
#[derive(Resource, Debug, Clone)]
pub struct CurrentEncounter {
    pub enemy_entities: Vec<Entity>,
    pub fleeable: bool,
}
```

**#15 reads it via `Option<Res<CurrentEncounter>>`** — so #16 can ship without #15 needing changes. **#16 must produce exactly this shape** — no extra fields, no rename.

### `start_combat` is NOT a function — it's a system

#15 has NO public `start_combat` function. The combat-entry path is:
1. **Some system** sets `NextState<GameState>::Combat`.
2. `OnEnter(GameState::Combat)` runs `init_combat_state` (`turn_manager.rs:194-212`) — clears the action queue, reseeds RNG.
3. `OnEnter(GameState::Combat)` runs `spawn_dev_encounter` under `feature = "dev"` (`turn_manager.rs:181-186`, `:677-716`) — this is the stub #16 deletes.
4. `CombatPlugin`'s sub-plugins start running (`TurnManagerPlugin`, `EnemyAiPlugin`, `CombatUiPlugin`) — they read alive enemies from `Query<&Enemy>` and alive party from `Query<&PartyMember>`, NOT from `CurrentEncounter` directly.

**#15 doesn't actually consume `CurrentEncounter` in production code yet** (only test fixtures do). The resource is a contract for #16 to fulfill, and a future hook for save/load (#23) and post-combat reward distribution (where the resource is the "what just happened" log).

**Conclusion:** #16's `handle_encounter_request` is the de facto `start_combat`. Its responsibilities (in this order):
1. Spawn `EnemyBundle` entities for each `EnemySpec` in the picked group.
2. `commands.insert_resource(CurrentEncounter { enemy_entities, fleeable })`.
3. `next_state.set(GameState::Combat)`.
4. `sfx.write(SfxRequest { kind: SfxKind::EncounterSting })`.

#15 takes over from `OnEnter(Combat)` onwards — same way the existing dev-stub flow does.

### Critical: delete `spawn_dev_encounter`

Per #15 plan Pitfall 1: *"#16 deletes this stub when it ships its own encounter spawner."* The dev-stub at `turn_manager.rs:677-716` and its registration at `:182-186` MUST be removed by #16. Without removal, a `#[cfg(feature = "dev")]` build doubles up enemies (dev-stub + #16-spawned).

The deletion is mechanical: remove the function and the registration. **Add a regression test** (under `feature = "dev"`): spawn a party, force an encounter via F7, run a frame, assert the enemy count matches what the picked encounter group specifies (no stray "Goblin 1"/"Goblin 2" from the deleted stub).

---

## State Restoration After Combat

The roadmap (line 878) explicitly flags this: *"Re-entering the dungeon after combat must restore exact pre-combat state (cell, facing, animation finished) — easy bug source."*

**Status: already mostly handled by #15's `PlayerParty` preservation rule.**

### What's already in place

- **`despawn_dungeon_entities`** at `dungeon/mod.rs:593-619` preserves `PlayerParty` when transitioning to Combat (`preserve_party = matches!(state.get(), GameState::Combat)`). The party's entity, with its `GridPosition`, `Facing`, and `Transform`, survives `OnExit(Dungeon)` → `OnEnter(Combat)`.
- **`cleanup_party_after_combat`** at `dungeon/mod.rs:572-591` despawns the party only if leaving Combat for a destination *other than* Dungeon (specifically: Town, GameOver, etc.). Returning to Dungeon → party preserved.
- **`spawn_party_and_camera`** at `dungeon/mod.rs:454-461` has an idempotence guard: if a `PlayerParty` already exists, it skips spawning. So the post-combat `OnEnter(Dungeon)` doesn't double-spawn.
- **Geometry is despawned and respawned.** The dungeon walls/floors are NOT preserved (they're tagged `DungeonGeometry` and despawned). The party's logical state survives; the geometry rebuilds from the still-loaded `DungeonFloor` asset. Visually identical.

### What `MovementAnimation` does during the round-trip

Critical detail from the roadmap (line 878 — "animation finished"):
- The `MovementAnimation` component is a per-frame lerp; it removes itself when complete (`dungeon/mod.rs:952-957`).
- Per `dungeon/mod.rs:31-34`: *"`GridPosition` and `Facing` update **immediately** on input-commit (same frame). `MovementAnimation` then lerps the visual `Transform`."* Translation: by the time the encounter triggers, the LOGICAL state (`GridPosition`, `Facing`) is already at the target. Only the visual `Transform` is mid-tween.
- During Combat, the `animate_movement` system is gated `.run_if(in_state(GameState::Dungeon))` (`dungeon/mod.rs:243`). So the tween freezes at whatever progress it had.
- On `OnEnter(Dungeon)` post-combat, the tween resumes (the component is still present on the preserved `PlayerParty`).

**Risk:** A tween that was 50% through an Eastward step gets paused, combat happens, and on return the tween resumes from 50% to 100% — visually correct (it lands on the target cell), but the player perceives a "jump" if combat lasted multiple seconds.

**Recommendation:** **On `OnEnter(GameState::Combat)`, snap the tween to completion.** Add a system in `EncounterPlugin` (NOT in `dungeon/mod.rs` — that file is touched only via carve-out):

```rust
fn snap_movement_animation_on_combat_entry(
    mut commands: Commands,
    mut query: Query<(Entity, &mut Transform, &MovementAnimation)>,
) {
    for (entity, mut transform, anim) in &mut query {
        // Snap to destination — same logic as animate_movement's t_raw >= 1.0 branch.
        transform.translation = anim.to_translation;
        transform.rotation = anim.to_rotation;
        commands.entity(entity).remove::<MovementAnimation>();
    }
}

// In EncounterPlugin::build:
app.add_systems(OnEnter(GameState::Combat), snap_movement_animation_on_combat_entry);
```

**Test:** Trigger an encounter mid-tween (set `MovementAnimation { elapsed_secs: 0.05, duration_secs: 0.18, ... }`), run `OnEnter(Combat)`, assert `MovementAnimation` is removed and `Transform` is at `to_translation`.

**Open question for the planner:** Is this snap behavior obvious to the player, or does it warrant a brief fade? See §Open Questions.

---

## `FoeProximity` Resource Hook for #22

Per roadmap line 876: *"Plan for #22 to publish a `FoeProximity` resource that #16 reads."*

#22 will introduce a "Field-On-Enemy" system (visible enemies on the map that move with the player; Etrian Odyssey-style). When an FOE is in line-of-sight, random encounters should be suppressed — otherwise the player gets a "surprise" battle while staring at a known enemy.

**#16's responsibility:** Define the `FoeProximity` resource shape with a `Default` impl that returns "no FOEs nearby" (so #16 ships without #22). #22 replaces the `Default::default()` and the system that updates it.

### Recommended shape

```rust
// In src/plugins/combat/encounter.rs (#16 owns the type; #22 owns the data).

/// Read by `check_random_encounter` to suppress rolls when an FOE is visible.
///
/// **#16 stub:** ships with `Default` (always returns "no FOEs"). #22 replaces
/// the populator system that updates `nearby_foe_entities`.
///
/// **Why a Vec, not a bool:** future #22 may want richer suppression rules
/// (e.g. "suppress random rolls AND increase encounter rate after the FOE
/// disappears" — soft-pity revenge). Keeping the resource shape generic now
/// avoids #22 needing to break the type.
#[derive(Resource, Default, Debug, Clone)]
pub struct FoeProximity {
    /// FOE entities within line-of-sight or N-cell radius (definition is #22's call).
    pub nearby_foe_entities: Vec<Entity>,
}

impl FoeProximity {
    /// Suppress random encounter rolls when at least one FOE is nearby.
    /// #22 may override the rule (e.g. only suppress for boss-tier FOEs).
    pub fn suppresses_random_rolls(&self) -> bool {
        !self.nearby_foe_entities.is_empty()
    }
}
```

**Registration:** `EncounterPlugin::build` calls `app.init_resource::<FoeProximity>()`. #22 doesn't need to re-register — it just adds the populator system.

**Test:** Insert `FoeProximity { nearby_foe_entities: vec![Entity::PLACEHOLDER] }`; walk 100 steps with `cell.encounter_rate = 1.0`; assert zero `EncounterRequested` messages written.

---

## Open Questions (for the planner)

1. **Soft-pity counter reset semantics — preserve or reset on combat-end?**
   - **What we know:** Roadmap says "guaranteed encounter after enough steps" (line 872). Doesn't specify whether the counter persists across combats.
   - **What's unclear:** Reset on every `OnEnter(Dungeon)` (clean slate per dungeon-run) vs preserve across combats (continuous tension within a dungeon-visit) vs reset on every player rest (Town visit).
   - **Recommendation (this researcher):** Reset on every `OnEnter(Dungeon)` (simplest, most predictable for designers; combat itself is the "tension release"). Document the tradeoff. Surface as a planner pick.

2. **Per-cell `encounter_rate = 0.0` interaction with the soft-pity accumulator (Pitfall 7).**
   - **What we know:** Research formula is `rate * (1.0 + steps * 0.05)`. Designers will author "safe corridors" with `encounter_rate = 0.0`.
   - **What's unclear:** Should accumulator continue to rise on rate-zero cells? Should it cap at some multiplier? Should rate-zero cells reset the counter?
   - **Options (from Pitfall 7):**
     - A: Cap multiplier at e.g. `2.0` (`(1.0 + steps * 0.05).min(2.0)`).
     - B: Skip accumulator bump on rate-zero cells.
     - C: Reset counter on rate-zero cells.
   - **Recommendation (this researcher):** Option A (cap at 2.0). Predictable, easy to reason about, no special-casing of rate-zero. But this is a design call.

3. **Asset format for encounter tables: inline `EnemySpec` vs ID-references to `EnemyDb`.**
   - **What we know:** `EnemyDb` is currently an empty stub (`data/enemies.rs:8-11`); #17 fills it.
   - **What's unclear:** Should #16 ship inline `EnemySpec` (full stats embedded in the encounter table) and migrate to ID-refs in #17? Or wait for #17 first?
   - **Recommendation (this researcher):** Ship inline for #16 (mirrors the `spawn_dev_encounter` stub at `turn_manager.rs:683-714` — same data shape). Migrate to ID-refs in #17 by adding an `enemy_id: Option<String>` field to `EnemySpec` and falling back to `EnemyDb` lookup when present. Backward-compatible.

4. **`MovementAnimation` snap on `OnEnter(Combat)` — is the visual jump acceptable?**
   - **What we know:** Without snapping, the tween freezes mid-stride during combat and resumes on return.
   - **What's unclear:** Player-perception cost — does a 50% tween snap to 100% feel jarring vs the alternative (fade-in transition)?
   - **Recommendation (this researcher):** Ship the snap for v1. Add an `EncounterStingFlash` polish item to #25 (combat-entry transition effect) to mask the snap. Surface to the planner.

5. **`EncounterSource::Random` variant — define here or wait for FOE work?**
   - **What we know:** `EncounterSource::AlarmTrap` already exists; the comment at `features.rs:133` says "Future: Random (foe roll), Foe (overworld encounter) — surface in #16."
   - **What's unclear:** Does #16 add `Random` only, or `Random` + a placeholder `Foe` variant for #22 to fill?
   - **Recommendation (this researcher):** Add `Random` only. #22 adds its own variant when it needs one. This keeps the variant set minimal until there's a real consumer.

6. **`?force_encounter` keybind — F7 (suggested) or something else?**
   - **What we know:** F9 is taken (state cycler at `state/mod.rs:71-89`); F2-F12 are otherwise unused in the current keymap.
   - **What's unclear:** Is F7 ergonomic? Other options: F8, Numpad+, KeyB ("battle"), backtick.
   - **Recommendation (this researcher):** F7 — adjacent to F9 (which is the state-debug keybind), low collision risk with browser/IDE shortcuts. But this is a minor call.

7. **Where does the `pick_enemy_group` function live — `encounter.rs` or `data/encounters.rs`?**
   - **What we know:** It's pure (takes `&EncounterTable`, `&mut Rng`, returns `Option<&EnemyGroup>`).
   - **What's unclear:** Co-locate with the schema (`data/encounters.rs`) for testability; or with the consumer (`encounter.rs`).
   - **Recommendation (this researcher):** `data/encounters.rs` as a method on `EncounterTable` (`impl EncounterTable { pub fn pick_group(&self, rng: &mut ...) -> Option<&EnemyGroup> }`). Mirrors the `DungeonFloor::can_move` precedent at `data/dungeon.rs:285-300`. Pure data + pure logic in the schema module.

---

## Sources

### Primary (HIGH confidence)

- [Druum CLAUDE.md](file:///Users/nousunio/Repos/Learnings/claude-code/druum/CLAUDE.md) — GitButler `but` discipline; `gitbutler/workspace` pre-commit hook
- [Roadmap §16](file:///Users/nousunio/Repos/Learnings/claude-code/druum/project/roadmaps/20260429-01-bevy-dungeon-crawler-roadmap.md) lines 862-911 — feature spec
- [Original deep research §Code Examples](file:///Users/nousunio/Repos/Learnings/claude-code/druum/research/20260326-01-bevy-first-person-dungeon-crawler-rpg.md) lines 1181-1215 — encounter check pattern
- [Original deep research §Pitfall 4](file:///Users/nousunio/Repos/Learnings/claude-code/druum/research/20260326-01-bevy-first-person-dungeon-crawler-rpg.md) lines 1041-1052 — data-driven encounter rates
- [Feature #15 implementation summary](file:///Users/nousunio/Repos/Learnings/claude-code/druum/project/implemented/20260508-120000-feature-15-turn-based-combat-core.md) — `CurrentEncounter` shape, RNG plumbing, dev-stub
- [Feature #15 plan](file:///Users/nousunio/Repos/Learnings/claude-code/druum/project/plans/20260508-100000-feature-15-turn-based-combat-core.md) — Pitfall 1 (`#16 deletes this stub`), Decision 21 (`CombatRng` shape)
- [`combat/turn_manager.rs`](file:///Users/nousunio/Repos/Learnings/claude-code/druum/src/plugins/combat/turn_manager.rs) lines 34-46 — `CurrentEncounter` contract documented in code; lines 135-142 — `CombatRng` resource pattern; lines 677-716 — dev-stub spawner to delete
- [`combat/enemy.rs`](file:///Users/nousunio/Repos/Learnings/claude-code/druum/src/plugins/combat/enemy.rs) lines 39-51 — `EnemyBundle` shape
- [`combat/ai.rs`](file:///Users/nousunio/Repos/Learnings/claude-code/druum/src/plugins/combat/ai.rs) lines 45-53 — `EnemyAi` enum with `Default` (RandomAttack)
- [`dungeon/mod.rs`](file:///Users/nousunio/Repos/Learnings/claude-code/druum/src/plugins/dungeon/mod.rs) lines 192-197 — `MovedEvent`; lines 211-218 — `ActiveFloorNumber`; lines 392-401 — `floor_handle_for`; lines 454-461 — idempotent spawn; lines 572-591 / 593-619 — preserve/cleanup party; lines 897-902 — `MovedEvent` write site
- [`dungeon/features.rs`](file:///Users/nousunio/Repos/Learnings/claude-code/druum/src/plugins/dungeon/features.rs) lines 125-128 — `EncounterRequested`; lines 130-134 — `EncounterSource`; lines 459-487 — `apply_alarm_trap` precedent; lines 366-456 — `MovedEvent` consumer pattern
- [`data/dungeon.rs`](file:///Users/nousunio/Repos/Learnings/claude-code/druum/src/data/dungeon.rs) lines 156-174 — `CellFeatures`; line 167 — `encounter_rate: f32`; line 260 — `encounter_table: String`; lines 285-300 — `can_move` (impl pattern for `pick_group`)
- [`loading/mod.rs`](file:///Users/nousunio/Repos/Learnings/claude-code/druum/src/plugins/loading/mod.rs) lines 30-45 — `DungeonAssets`; lines 105-111 — `RonAssetPlugin` registrations; lines 183-208 — placeholder UI for cleanup pattern
- [`audio/sfx.rs`](file:///Users/nousunio/Repos/Learnings/claude-code/druum/src/plugins/audio/sfx.rs) lines 50-60 — `SfxKind::EncounterSting` already wired
- [`state/mod.rs`](file:///Users/nousunio/Repos/Learnings/claude-code/druum/src/plugins/state/mod.rs) lines 6-15 — `GameState`; lines 17-26 — `DungeonSubState`; lines 71-89 — F9 dev-cycler precedent for F7 force-encounter
- [`input/mod.rs`](file:///Users/nousunio/Repos/Learnings/claude-code/druum/src/plugins/input/mod.rs) lines 14-22 — F9 carve-out rationale (precedent for F7); lines 73-85 — `DungeonAction` (frozen)
- [`Cargo.toml`](file:///Users/nousunio/Repos/Learnings/claude-code/druum/Cargo.toml) lines 25-26 — `bevy_common_assets` / `bevy_asset_loader`; line 33 — `rand 0.9` direct; line 36 — `rand_chacha 0.9` dev-dep
- [`Cargo.lock`](file:///Users/nousunio/Repos/Learnings/claude-code/druum/Cargo.lock) lines 4361-4378 — `rand 0.9.4` + `rand_chacha 0.9.0` versions verified
- [`combat/damage.rs`](file:///Users/nousunio/Repos/Learnings/claude-code/druum/src/plugins/combat/damage.rs) lines 23-67 — pure-function discipline + `?Sized` precedent
- [`combat/mod.rs`](file:///Users/nousunio/Repos/Learnings/claude-code/druum/src/plugins/combat/mod.rs) lines 28-41 — sub-plugin registration pattern

### Secondary (MEDIUM confidence)

- [rand 0.9 docs.rs](https://docs.rs/rand/0.9/rand/distr/weighted/struct.WeightedIndex.html) — `WeightedIndex` API in 0.9 (verified module path: `rand::distr::weighted`). Accessed: 2026-05-08
- [rand 0.9 changelog](https://github.com/rust-random/rand/blob/master/CHANGELOG.md) — `gen_range` → `random_range` rename, `distributions` → `distr` module move. Accessed: 2026-05-08

### Tertiary (LOW confidence)

(None — every claim in this document is rooted in either Druum codebase file:line citations or the research master document.)

---

## Metadata

**Confidence breakdown:**

- Standard stack: HIGH — every dep is already in `Cargo.toml`/`Cargo.lock`, version-pinned, with multiple existing call sites verified in the merged tree.
- Architecture: HIGH — `EncounterRequested` message channel is already declared at `features.rs:125-128`; `apply_alarm_trap` is the exact precedent at `features.rs:459-487`; `CurrentEncounter` shape is locked at `turn_manager.rs:34-46`.
- Pitfalls: HIGH — Pitfalls 1, 2, 5, 6 are direct mirrors of #15's confirmed pitfalls (D-I16, B0002 query split, system ordering, resource cleanup). Pitfall 4 (cursor draining) and Pitfall 7 (rate-zero corridor interaction) are novel to #16 but flagged from first-principles + draining precedent at `audio/sfx.rs:73-78`.
- API contract with #15: HIGH — the `CurrentEncounter` shape, the `spawn_dev_encounter` deletion mandate, the `MessageReader<MovedEvent>` registration in test apps are all documented in the #15 plan and implementation summary.
- `FoeProximity` hook design: MEDIUM — #22 hasn't been planned in detail; the recommended `Vec<Entity>` shape is generous (avoids breaking changes in #22) but `bool active` would be simpler if #22's needs turn out trivial. Surface to planner.
- Test strategy: HIGH — `seed_test_rng` pattern at `turn_manager.rs::app_tests:813-815` is the canonical model.

**Research date:** 2026-05-08
