# Bevy 0.18 State Machine Anatomy

A walkthrough of how `src/plugins/state/mod.rs` works in this project, covering the core Bevy ECS concepts it touches: the `States` and `SubStates` derives, the plugin pattern, system functions, schedules, run conditions, and runtime flow. Use this as a reference when reading or extending state-related code.

## Why this matters for the project

Druum is a state-driven game: the loop is `Loading → TitleScreen → Town → Dungeon → Combat → GameOver`, and most gameplay systems should only run when their relevant state is active. Bevy's `States` machinery makes this declarative — once states are wired up, every later feature gates its systems with `run_if(in_state(GameState::X))` instead of writing imperative state-check code. Getting the state machine right early saves churn in every feature that follows.

## The reference file

```rust
use bevy::log::info;
use bevy::prelude::*;

#[derive(States, Default, Debug, Clone, PartialEq, Eq, Hash)]
pub enum GameState {
    #[default]
    Loading,
    TitleScreen,
    Town,
    Dungeon,
    Combat,
    GameOver,
}

#[derive(SubStates, Default, Debug, Clone, PartialEq, Eq, Hash)]
#[source(GameState = GameState::Dungeon)]
pub enum DungeonSubState {
    #[default]
    Exploring,
    Inventory,
    Map,
    Paused,
    EventDialog,
}

// (CombatPhase and TownLocation follow the same pattern, scoped to
//  GameState::Combat and GameState::Town respectively.)

pub struct StatePlugin;

impl Plugin for StatePlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<GameState>()
            .add_sub_state::<DungeonSubState>()
            .add_sub_state::<CombatPhase>()
            .add_sub_state::<TownLocation>()
            .add_systems(
                Update,
                log_game_state_transition.run_if(state_changed::<GameState>),
            );

        #[cfg(feature = "dev")]
        app.add_systems(Update, cycle_game_state_on_f9);
    }
}

fn log_game_state_transition(state: Res<State<GameState>>) {
    info!("GameState -> {:?}", state.get());
}
```

## Imports

- `bevy::log::info` — pulls in the `info!()` log macro used by the transition logger. Bevy's logger ships with `LogPlugin` (part of `DefaultPlugins`) and routes to stdout by default.
- `bevy::prelude::*` — Bevy's "kitchen sink" import: `App`, `Plugin`, `States`, `SubStates`, `Update`, `Res`, `State`, `NextState`, `state_changed`, `IntoSystemConfigs`, and many more. Convention is that every Bevy file starts with the prelude.

## The `States` derive

```rust
#[derive(States, Default, Debug, Clone, PartialEq, Eq, Hash)]
pub enum GameState {
    #[default]
    Loading,
    ...
}
```

Each derived trait has a specific job:

| Trait | Purpose |
|---|---|
| `States` | The Bevy trait that turns this enum into a state machine. The derive macro generates the wiring (registration, transition events, source plumbing). |
| `Default` | The `States` macro **requires** this — `init_state::<GameState>()` calls `GameState::default()` to determine the initial value. Without it, the macro errors out. |
| `Debug` | So `{:?}` formatting works, used by the logger. |
| `Clone` | Bevy clones state values when transitioning. |
| `PartialEq, Eq, Hash` | Bevy stores states in internal hash maps and compares them every frame. All three are required by the trait bound. |

The `#[default]` attribute on `Loading` says "this is the initial variant." Required because `Default` cannot pick a default for an enum on its own — Rust forces you to be explicit.

**Note:** `Copy` is intentionally *not* derived. The roadmap leaves room for state variants to grow data-carrying forms later (e.g. `Combat { encounter_id: u32 }`). Adding `Copy` now would lock that door.

## The `SubStates` derive and `#[source]` attribute

```rust
#[derive(SubStates, Default, Debug, Clone, PartialEq, Eq, Hash)]
#[source(GameState = GameState::Dungeon)]
pub enum DungeonSubState {
    #[default]
    Exploring,
    ...
}
```

Two differences from `GameState`:

1. **`SubStates` instead of `States`.** A sub-state only exists in the world while its parent is in a specific variant. Leaving the parent variant automatically removes the sub-state's resources — there is no way to be in `DungeonSubState::Inventory` while `GameState == Town`. This is enforced by Bevy at runtime, not by your code.

2. **`#[source(GameState = GameState::Dungeon)]`.** This attribute is the contract: "I am only active when `GameState` equals `GameState::Dungeon`." The derive macro reads this attribute and generates the activate/deactivate plumbing.

The exact attribute syntax matters: `#[source(GameState = GameState::Dungeon)]` — the left side is the type path, the right side is a pattern. A common mistake is swapping the sides; the macro produces an opaque error if you do.

**Gotcha:** `Default` is required on every `SubStates` enum, even though the trait definition does not list it. The macro generates `Self::default()` internally; omitting `#[derive(Default)]` produces a confusing error from inside macro expansion. If you ever see "Self::default not found" coming from a `SubStates`-derived enum, this is the cause.

## Plugins — Bevy's modularity unit

```rust
pub struct StatePlugin;

impl Plugin for StatePlugin {
    fn build(&self, app: &mut App) { ... }
}
```

A unit struct (no fields). Plugins are how Bevy modularizes setup logic: instead of stuffing every `app.add_systems(...)` call into `main.rs`, each subsystem ships its own plugin and `main.rs` wires them together:

```rust
.add_plugins((DefaultPlugins, StatePlugin, DungeonPlugin, ...))
```

The `Plugin` trait has one required method, `build`, which Bevy calls **once** when the app starts. Inside `build` you configure the `App` — register resources, schedules, systems, states, events, etc. Each method on `App` returns `&mut App` so calls chain.

`StatePlugin` must come **after** `DefaultPlugins` in the tuple. `init_state` panics with `"The 'StateTransition' schedule is missing"` if `StatesPlugin` (transitively pulled in by `DefaultPlugins`) hasn't been added first. Tuple order in `add_plugins((...))` is preserved, so place `StatePlugin` immediately after `DefaultPlugins`.

## What `init_state` and `add_sub_state` do

### `init_state::<GameState>()`

Several side effects:

- Inserts `State<GameState>` resource (the *current* state)
- Inserts `NextState<GameState>` resource (the *queued* next state, used by `next.set(...)`)
- Sets the initial value to `GameState::default()` (which is `Loading`)
- Adds the `StateTransition` schedule that runs `OnEnter`/`OnExit` systems
- Registers the `StateTransitionEvent<GameState>` message type

### `add_sub_state::<DungeonSubState>()`

Like `init_state` but reads the `#[source(...)]` attribute. Adds a hidden listener on the parent state — when the parent transitions *into* the source variant, the sub-state's `State<...>` and `NextState<...>` resources are inserted at their default value. When the parent transitions *out*, those resources are removed. This is automatic; you don't write any of the activate/deactivate code yourself.

## Schedules and the `Update` argument

```rust
.add_systems(
    Update,
    log_game_state_transition.run_if(state_changed::<GameState>),
)
```

`Update` is a *schedule* — a labeled phase of work that Bevy runs every frame. Bevy's frame loop runs schedules in a fixed order:

```
First → PreUpdate → StateTransition → Update → PostUpdate → Last
```

(There are also `Startup` schedules that run once at app start, and `OnEnter(X)` / `OnExit(X)` schedules tied to state transitions.)

By passing `Update` as the first argument to `add_systems`, we're saying "run this system once per frame, in the Update phase." Most gameplay systems live in `Update`. `PreUpdate` is for input collection, `PostUpdate` is for rendering preparation, etc.

## Run conditions — `.run_if(...)`

```rust
log_game_state_transition.run_if(state_changed::<GameState>)
```

A run condition is a function returning `bool` that Bevy calls before the system. If false, the system is skipped that frame.

`state_changed::<GameState>` is a Bevy-provided run condition that returns true exactly on the frame where `GameState` transitioned (including the very first frame after `init_state`, because going from "no resource" to "exists with value Loading" counts as a change). On every other frame, the logger is skipped.

Other useful state-related run conditions for later features:

- `in_state(GameState::Dungeon)` — system runs only while in this state
- `not(in_state(GameState::Loading))` — system runs in every state *except* Loading
- `resource_exists::<MyResource>` — system runs only when the resource is present
- `resource_changed::<MyResource>` — fires when the resource was modified

You can combine conditions with `.and()` and `.or()`.

## Conditional compilation — `#[cfg(feature = "dev")]`

```rust
#[cfg(feature = "dev")]
app.add_systems(Update, cycle_game_state_on_f9);
```

This whole `add_systems` line is compiled in only when the `dev` Cargo feature is on. In release builds, the F9 cycler is never registered. See `project/resources/20260501-102842-dev-feature-pattern.md` for the full pattern (including the rule that both the function definition AND its registration must be cfg-gated).

## System functions and `Res<T>`

```rust
fn log_game_state_transition(state: Res<State<GameState>>) {
    info!("GameState -> {:?}", state.get());
}
```

Bevy's killer feature: **any function whose parameters all implement `SystemParam` is automatically a system**. No trait implementation, no boilerplate — Bevy's `IntoSystem` machinery handles the conversion when you pass the function to `add_systems`.

System parameters seen in this project so far:

| Parameter | Meaning |
|---|---|
| `Res<T>` | Read-only access to a resource of type `T`. Panics at startup if the resource doesn't exist. |
| `ResMut<T>` | Mutable access to a resource. Same existence rules. |
| `Res<State<S>>` | Read the current value of state machine `S`. |
| `ResMut<NextState<S>>` | Queue a transition. Call `next.set(value)` to schedule the change for the next `StateTransition` phase. |
| `Query<&T>` / `Query<&mut T, With<U>>` | Iterate over components on entities. Not used in this file but ubiquitous elsewhere. |

**State transitions are deferred by one frame.** When you call `next.set(GameState::Town)`, the new value is *not* visible immediately — the change is realized in the next frame's `StateTransition` schedule. Tests must `app.update()` twice after pressing a state-changing input: once for the press to be processed, once for the transition to commit. This is a common source of test confusion.

## Runtime flow — what actually happens

1. **App start.** `StatePlugin::build` runs once. `init_state::<GameState>()` inserts `State<GameState>(Loading)` and `NextState<GameState>(None)`. The three sub-state resources do *not* exist yet (their parent isn't active).
2. **First frame.** `state_changed::<GameState>` returns `true` because the resource was just added. The logger fires, prints `GameState -> Loading`.
3. **Subsequent idle frames.** `state_changed` is false; the logger is skipped.
4. **Some system queues a transition** (e.g. F9 cycler in dev, or an asset-loaded system in feature #3). `next_state.set(GameState::TitleScreen)` is called.
5. **Next frame.** The `StateTransition` schedule runs first; `State<GameState>` updates to `TitleScreen`, any `OnExit(Loading)` and `OnEnter(TitleScreen)` systems fire.
6. **Update phase.** `state_changed` is true again. Logger prints `GameState -> TitleScreen`.

If the new state is `Dungeon`, step 5 also inserts `State<DungeonSubState>(Exploring)` (the sub-state's default) and `NextState<DungeonSubState>(None)`. Leaving `Dungeon` later removes them.

## Common pitfalls

1. **Forgetting `Default` on a `SubStates` derive.** Macro generates `Self::default()`; missing it produces an opaque error.
2. **Forgetting `#[default]` on a variant.** `Default` then can't compile.
3. **Putting `StatePlugin` before `DefaultPlugins`.** Panic at startup: "The 'StateTransition' schedule is missing."
4. **Using `EventReader<StateTransitionEvent<S>>`.** In Bevy 0.18, `StateTransitionEvent<S>` is a `Message`, not an `Event`. Use `MessageReader` instead, or sidestep with the `state_changed` run condition.
5. **Mutating `State<S>` directly.** The inner field is `pub(crate)` and there is no public setter. Always queue via `next.set(...)`. Trying to bypass the queue silently fails or won't compile.
6. **Re-exporting `GameState` from the crate root.** The project intentionally does *not* do this — every consumer writes `use crate::plugins::state::GameState;` explicitly so the origin is greppable.
7. **Expecting `next.set(...)` to take effect this frame.** It doesn't. The new state is observable on the next `app.update()` (after the `StateTransition` schedule runs).

## References

- Bevy 0.18 `bevy_state` source: https://github.com/bevyengine/bevy/tree/v0.18.1/crates/bevy_state
- Bevy book — States: https://bevy.org/learn/quick-start/getting-started/states/
- `src/plugins/state/mod.rs` — the file this document explains
- `src/main.rs` — shows `StatePlugin` placement in the `add_plugins` tuple
- `src/plugins/{dungeon,combat,town}/mod.rs` — show `OnEnter`/`OnExit` system patterns hooked to `GameState`
- `project/plans/20260429-031500-bevy-0-18-1-state-machine-feature-2.md` — full plan with verified-against-source 0.18 API references
- `project/research/20260429-030000-bevy-0-18-1-state-machine-feature-2.md` — research that surfaced the pitfalls listed above
- `project/resources/20260501-102842-dev-feature-pattern.md` — companion doc covering the `dev` feature flag and cfg-gating
