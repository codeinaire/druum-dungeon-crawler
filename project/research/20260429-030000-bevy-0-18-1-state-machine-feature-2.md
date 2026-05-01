# Bevy 0.18.1 Game State Machine (Feature #2) - Research

**Researched:** 2026-04-29
**Domain:** Bevy 0.18.1 `States` / `SubStates` API wiring for druum DRPG
**Confidence:** HIGH (verified directly against the `bevy_state-0.18.1` and `bevy_input-0.18.1` source code on disk)

## Tooling Limitation Disclosure

This research session ran with only Read / Write / Grep / Glob / Edit. No Bash, no MCP servers (despite the `context7` system reminder), no WebFetch, no WebSearch. Per the saved feedback memory `feedback_tooling_limitations.md`, I am declaring this up front.

**How that was mitigated:** the local Cargo registry already has `bevy-0.18.1`, `bevy_state-0.18.1`, `bevy_input-0.18.1`, `bevy_state_macros-0.18.1`, `bevy_internal-0.18.1`, and `bevy_ecs-0.18.1` extracted at `/Users/nousunio/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/`. **Every API claim in this document was verified by reading the actual 0.18.1 source files on disk** — file paths and line numbers are cited inline. This is HIGH confidence: it is not training-data recall, it is direct source inspection of the exact crate version pinned in `Cargo.lock` (verified at `Cargo.lock:341, 891, 1400`).

The single thing I could not verify locally is the live Bevy 0.17→0.18 migration guide (it lives on the website, not in the source tree). Where I cite it, I mark MEDIUM and provide a verification recipe.

## Summary

Bevy 0.18.1 ships the `States` / `SubStates` machinery in the `bevy_state` crate, transparently re-exported through `bevy::prelude`. Our project's existing `features = ["3d"]` already pulls in `bevy_state` and `DefaultPlugins` already installs `StatesPlugin`, so **no `Cargo.toml` changes are needed** for Feature #2.

The API surface in 0.18.1 is stable and clean:
- `#[derive(States, Default, Debug, Clone, PartialEq, Eq, Hash)]` for top-level enums.
- `#[derive(SubStates, Default, Debug, Clone, PartialEq, Eq, Hash)]` plus `#[source(GameState = GameState::Variant)]` for sub-states.
- Register with `app.init_state::<GameState>().add_sub_state::<DungeonSubState>()`.
- Schedule with `OnEnter(GameState::X)` / `OnExit(GameState::X)`.
- Gate systems with `.run_if(in_state(GameState::X))`.
- Mutate via `next: ResMut<NextState<GameState>>` + `next.set(GameState::Y)`.

The single 0.18-specific landmine: `StateTransitionEvent<S>` is a **`Message`**, not an `Event`. To read it you need `MessageReader<StateTransitionEvent<S>>`, **not** `EventReader<StateTransitionEvent<S>>` — the latter will not compile. Older Bevy 0.16/0.17 examples copied from blog posts or docs.rs latest tend to use `EventReader` and will mislead a careless implementer. This is the single biggest gotcha for this feature.

**Primary recommendation:** Implement `GameState`, the three sub-states, and a `StatePlugin` in `src/plugins/state/mod.rs` (consistent with the established 7-flat-plugin pattern from Feature #1). Use the `state_changed::<GameState>` run-condition for the debug logger (simpler than reading the message stream, and prints exactly the *new* state value). Wire the F9 hotkey inside the same `StatePlugin` for now and gate it behind `#[cfg(feature = "dev")]` so the cycler does not ship in release builds.

## Standard Stack

### Core

| Library | Version | Purpose | License | Maintained? | Why Standard |
| ------- | ------- | ------- | ------- | ----------- | ------------ |
| `bevy_state` (re-exported via `bevy::state` and `bevy::prelude`) | 0.18.1 | The `States` / `SubStates` traits, `State<S>` / `NextState<S>` resources, `OnEnter`/`OnExit` schedules, `in_state`/`state_changed` run conditions | MIT/Apache-2.0 (inherits Bevy) | Yes — actively co-released with Bevy core | Built-in; the only canonical state machine for Bevy. No third-party alternative is in mainstream use for top-level game state. |
| `bevy_input` (re-exported via `bevy::input` and `bevy::prelude`) | 0.18.1 | `ButtonInput<KeyCode>` resource, `KeyCode::F9` variant | MIT/Apache-2.0 | Yes | Canonical built-in input polling resource since 0.13. The skeleton already initialises it via `DefaultPlugins → InputPlugin`. |

### Supporting

| Library | Version | Purpose | When to Use |
| ------- | ------- | ------- | ----------- |
| `bevy_log` (re-exported via `bevy::log`) | 0.18.1 | `info!`, `debug!`, `trace!` macros | All `OnEnter`/`OnExit` placeholder logs and the transition logger. Already enabled by `DefaultPlugins → LogPlugin`. |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
| ---------- | --------- | -------- |
| `States` + `SubStates` + run-conditions | Hand-rolled enum + `Resource` + manual gating in every system | Would require re-implementing transition events, OnEnter/OnExit schedules, and the deactivation guarantee for sub-states. No upside for our use case. |
| `add_sub_state` (which forces the source-state derive) | `add_computed_state` + manual `ComputedStates` impl | More flexibility (multi-state sources, no `Default` requirement, custom `should_exist` logic). Unnecessary here — every sub-state has exactly one parent and a single default variant. Documented as the "more complex situations" path in `bevy_state-0.18.1/src/state/sub_states.rs:54`. |

**Installation:** None required. `bevy = { version = "=0.18.1", default-features = false, features = ["3d", ...] }` already enables `bevy_state` transitively (`bevy-0.18.1/Cargo.toml:2322-2330` defines `"3d"` to include `default_app`, which at line 2437-2446 enables `bevy_state`).

## Architecture Options

The roadmap surfaces two organisational choices: where the state code lives, and how the F9 hotkey is gated. These are independent.

### Option 1: `src/state.rs` at crate root

| Option | Description | Pros | Cons | Best When |
| ------ | ----------- | ---- | ---- | --------- |
| Crate-root file | Put `GameState`, `DungeonSubState`, `CombatPhase`, `TownLocation`, and `StatePlugin` in `src/state.rs`; declare `pub mod state;` in `lib.rs`. | Slightly less filesystem nesting; imports read `use druum::state::GameState`. | Breaks the established "everything is a plugin under `src/plugins/`" convention from Feature #1. The state module is functionally a plugin (registers resources, adds systems) — putting it elsewhere creates a special case. |  Library has no plugin convention or has only a handful of small modules. |

### Option 2: `src/plugins/state/mod.rs` (RECOMMENDED)

| Option | Description | Pros | Cons | Best When |
| ------ | ----------- | ---- | ---- | --------- |
| Plugin module | Put everything in `src/plugins/state/mod.rs`; add `pub mod state;` to `src/plugins/mod.rs`; re-export `StatePlugin` symmetrically with `DungeonPlugin`, `CombatPlugin`, etc. | Matches the `project_druum_skeleton.md` decision: "7 flat plugins under `src/plugins/`". Every architectural unit is a `Plugin`. New contributors find state where they expect it. `main.rs` adds `StatePlugin` in the same tuple as the other plugins. | One extra directory and one extra path segment. | The codebase has already committed to a plugin-per-subsystem convention — which druum has. |

**Recommended:** Option 2 (`src/plugins/state/mod.rs`).

### Counterarguments

Why someone might NOT choose Option 2:

- **"State is so foundational it deserves to be visible at crate root."** — *Response:* it is foundational, but so is the dungeon plugin (every floor, every encounter goes through it). The convention of "plugins live in `src/plugins/`" is not "non-foundational stuff lives in plugins" — it is the architecture. Promoting state to crate root sends the wrong signal to anyone learning the layout. The `StatePlugin` symbol is what other plugins reference, and `use druum::plugins::state::{StatePlugin, GameState}` reads fine.
- **"The original roadmap entry suggested either path."** — *Response:* the roadmap was written before Feature #1 locked in the seven-plugin convention. The skeleton memory now constrains this decision. Following Feature #1's pattern is more important than honouring the roadmap's "either-or" language.
- **"Feature #2 might end up with non-state code that doesn't fit a plugin module."** — *Response:* it won't. The Feature #2 scope is exactly: state enums, plugin, debug logger, OnEnter/OnExit stubs, F9 cycler. All five fit cleanly inside one plugin module.

### F9 hotkey gating

| Option | Description | Pros | Cons |
| ------ | ----------- | ---- | ---- |
| Always-on F9 | F9 cycler system added unconditionally inside `StatePlugin`. | Simplest. Always available even in release-debug builds. | Cycler will ship in release artefacts — small footprint, but a player could press F9 and nuke their game state. |
| `#[cfg(feature = "dev")]` gating (RECOMMENDED) | Cycler system gated on the existing `dev` Cargo feature (already used to enable `dynamic_linking`). | Cycler exists during `cargo run --features dev` (the development workflow per `project_druum_skeleton.md`); compiled out of release. Aligns with the existing dev-vs-release split. | One extra `#[cfg]` annotation. |
| Separate `dev` plugin | Move the cycler into a `DevPlugin` registered only under the dev feature. | Cleanest separation. | Premature for one system. Refactor when the project has 3+ dev-only systems — likely Feature #4 or later. |

**Recommended:** `#[cfg(feature = "dev")]` on the cycler `add_systems` call inside `StatePlugin::build`. Promote to a separate `DevPlugin` later when more dev-only systems accumulate.

## Architecture Patterns

### Recommended Project Structure

```
src/
├── plugins/
│   ├── mod.rs           # add `pub mod state;`
│   ├── state/
│   │   └── mod.rs       # GameState, three SubStates, StatePlugin, F9 cycler, debug logger
│   ├── dungeon/mod.rs   # add OnEnter(GameState::Dungeon) stub
│   ├── combat/mod.rs    # add OnEnter(GameState::Combat) stub
│   ├── town/mod.rs      # add OnEnter(GameState::Town) stub
│   ├── party/mod.rs     # unchanged
│   ├── ui/mod.rs        # unchanged
│   ├── audio/mod.rs     # unchanged
│   └── save/mod.rs      # unchanged
├── lib.rs               # unchanged
└── main.rs              # add StatePlugin to the add_plugins tuple (one line)
```

### Pattern 1: State enum derive macro signature

**What:** The `States` derive needs the trait bounds `'static + Send + Sync + Clone + PartialEq + Eq + Hash + Debug` (verified at `bevy_state-0.18.1/src/state/states.rs:64`). For `init_state` you also need `FromWorld`, which `Default` blanket-implements. So the canonical derive list is `States, Default, Debug, Clone, PartialEq, Eq, Hash`.

**When to use:** every top-level state enum.

**Example (verified):**
```rust
// Source: bevy_state-0.18.1/src/state/states.rs lines 28-34 (test fixture)
//         and the prose example at lines 122-131 of lib.rs.
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
```

Notes:
- `Copy` is shown in some upstream examples (`bevy_state-0.18.1/src/state/states.rs:28`) but is **not required** by the trait. Skip it for our enums — they're not all-bits-Copy candidates conceptually (states may grow data-carrying variants later).
- `Eq` is required by the trait but is implied by `PartialEq` only for plain enums; deriving it explicitly is the safe form.
- The order of derives is irrelevant to the compiler. Putting `States` first is a Bevy-codebase convention and reads well in greps.

### Pattern 2: SubStates with `#[source(...)]` attribute

**What:** A `SubStates` derive needs everything `States` needs *plus* a single `#[source(SourceType = SourceType::Variant)]` attribute. The macro at `bevy_state_macros-0.18.1/src/states.rs:84-137` parses the right-hand side as a Rust **pattern** (`Pat::parse_multi`, line 50) and emits `matches!(sources, GameState::Dungeon).then_some(Self::default())` for `should_exist`. Because the generated body calls `Self::default()`, **`Default` is a hard requirement on every derived `SubStates` enum** even though the trait itself does not list it.

**When to use:** every sub-state that should auto-mount/un-mount with a single parent state variant.

**Example (verified):**
```rust
// Source: bevy_state-0.18.1/src/state/sub_states.rs lines 24-32 (the macro's prose example).
//         Verified attribute syntax against bevy_state_macros-0.18.1/src/states.rs:42-82.

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

#[derive(SubStates, Default, Debug, Clone, PartialEq, Eq, Hash)]
#[source(GameState = GameState::Combat)]
pub enum CombatPhase {
    #[default]
    PlayerInput,
    ExecuteActions,
    EnemyTurn,
    TurnResult,
}

#[derive(SubStates, Default, Debug, Clone, PartialEq, Eq, Hash)]
#[source(GameState = GameState::Town)]
pub enum TownLocation {
    #[default]
    Square,
    Shop,
    Inn,
    Temple,
    Guild,
}
```

Critical attribute syntax notes (from reading `bevy_state_macros-0.18.1/src/states.rs`):
- The form is `#[source(TYPE = TYPE::VARIANT)]`. The left side is a **type path** (parsed as `nested.path`), the right side is a **pattern**. So `#[source(GameState = GameState::Dungeon)]` is correct; `#[source(GameState::Dungeon)]` is **not**.
- You can supply only **one** `#[source(...)]` attribute per derive. Multiple sources require a manual `impl SubStates for X` (covered in `sub_states.rs:60-95`).
- For struct-variant matches (e.g. `AppState::InGame { paused: false }`) you can put a more specific pattern on the right side because `Pat::parse_multi` accepts arbitrary patterns. We don't need this in Feature #2.

### Pattern 3: Registering states with `App`

**What:** Use the `AppExtStates` extension trait (in `bevy::state::prelude`, re-exported via `bevy::prelude` at `bevy_state-0.18.1/src/lib.rs:80`). Method names are `init_state::<S>` for top-level and `add_sub_state::<S>` for sub-states. They are idempotent (`bevy_state-0.18.1/src/app.rs:108-111, 203-206`).

**When to use:** in your `Plugin::build` impl.

**Example (verified):**
```rust
// Source: bevy_state-0.18.1/src/app.rs:90-114 (init_state) and :180-209 (add_sub_state).

use bevy::prelude::*;
// (also exposed: bevy::state::app::AppExtStates if you prefer the full path)

pub struct StatePlugin;

impl Plugin for StatePlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<GameState>()
            .add_sub_state::<DungeonSubState>()
            .add_sub_state::<CombatPhase>()
            .add_sub_state::<TownLocation>();
    }
}
```

Important ordering rule (from `bevy_state-0.18.1/src/app.rs:96-99`): `init_state` and `add_sub_state` both `expect("The 'StateTransition' schedule is missing. Did you forget to add StatesPlugin or DefaultPlugins before calling init_state?")`. **`StatePlugin` must be added after `DefaultPlugins`** in `main.rs`. This is automatic if both are in the same `add_plugins((...))` tuple — `DefaultPlugins` is a `PluginGroup` and its members are inserted before `StatePlugin` because tuple-order is preserved. Our existing `main.rs` already places `DefaultPlugins` first, so this is fine.

### Pattern 4: Debug transition logger (RECOMMENDED: `state_changed` run condition)

**What:** Two viable paths: (a) a system gated on `state_changed::<GameState>` that reads `Res<State<GameState>>`, or (b) a system that uses `MessageReader<StateTransitionEvent<GameState>>` and reads `exited`/`entered` directly.

Both work. I recommend (a) because:
- It uses the canonical run condition `state_changed::<S>` provided by Bevy (`bevy_state-0.18.1/src/condition.rs:165-170`).
- The system body is one line; the new state is `*current_state` (because `State<S>` derefs to `S`).
- It avoids the 0.18 `Event`/`Message` rename trap entirely.

The downside of (a) is that you cannot easily print the *previous* state from inside the system. For a manual-testing debug logger this is acceptable — `OnEnter`/`OnExit` already log entry/exit. If you ever want before/after in one log line, switch to (b).

**Example (verified):**
```rust
// Source: bevy_state-0.18.1/src/condition.rs:103-108 (in_state) and :165-170 (state_changed).

use bevy::prelude::*;
use bevy::log::info;

fn log_game_state_transition(state: Res<State<GameState>>) {
    info!("GameState -> {:?}", state.get());
}

// inside StatePlugin::build:
app.add_systems(Update, log_game_state_transition.run_if(state_changed::<GameState>));
```

If you prefer the message-reader path (cleaner before/after logging), this is the verified shape — note `MessageReader`, **not** `EventReader`:

```rust
// Source: bevy_state-0.18.1/src/state/transitions.rs:213-217 (last_transition helper)
//         and src/state_scoped.rs:74 / :142 (idiomatic usage in bevy itself).

use bevy::prelude::*;          // brings MessageReader into scope
use bevy::log::info;

fn log_game_state_transition(
    mut transitions: MessageReader<StateTransitionEvent<GameState>>,
) {
    for event in transitions.read() {
        info!(
            "GameState transition: {:?} -> {:?}",
            event.exited, event.entered,
        );
    }
}

// inside StatePlugin::build:
app.add_systems(Update, log_game_state_transition);
```

### Pattern 5: OnEnter / OnExit placeholder stubs in subsystem plugins

**What:** Each of `DungeonPlugin`, `CombatPlugin`, `TownPlugin` registers a system on `OnEnter(GameState::X)` (and optionally `OnExit(GameState::X)`) that just logs.

**Example:**
```rust
// src/plugins/dungeon/mod.rs
use bevy::prelude::*;
use bevy::log::info;
use crate::plugins::state::GameState;

pub struct DungeonPlugin;

impl Plugin for DungeonPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::Dungeon), || {
            info!("Entered GameState::Dungeon");
        })
        .add_systems(OnExit(GameState::Dungeon), || {
            info!("Exited GameState::Dungeon");
        });
    }
}
```

Closures are accepted as systems in 0.18 because Bevy's `IntoSystem` impl covers them. The empty-arg closure form (`|| { info!(...) }`) is the smallest legitimate system.

**Cross-plugin import note:** the subsystem plugins now need to import `GameState` from `crate::plugins::state`. This creates one import edge per subsystem plugin — unavoidable and intentional. Do not re-export `GameState` from the crate root just to shorten imports; that hides where it lives.

### Pattern 6: F9 hotkey cycler (dev-only)

**What:** Read `Res<ButtonInput<KeyCode>>` (`bevy_input-0.18.1/src/button_input.rs:125`, registered by `InputPlugin` at `bevy_input-0.18.1/src/lib.rs:114`), check `just_pressed(KeyCode::F9)` (line 188 of button_input.rs; `KeyCode::F9` at `bevy_input-0.18.1/src/keyboard.rs:669`), then `next.set(...)` to advance.

**Example (with `#[cfg(feature = "dev")]` gating recommendation):**
```rust
// inside src/plugins/state/mod.rs

use bevy::prelude::*;

// Cycle order: Loading -> TitleScreen -> Town -> Dungeon -> Combat -> GameOver -> Loading
fn cycle_game_state_on_f9(
    keys: Res<ButtonInput<KeyCode>>,
    current: Res<State<GameState>>,
    mut next: ResMut<NextState<GameState>>,
) {
    if !keys.just_pressed(KeyCode::F9) {
        return;
    }
    let upcoming = match current.get() {
        GameState::Loading     => GameState::TitleScreen,
        GameState::TitleScreen => GameState::Town,
        GameState::Town        => GameState::Dungeon,
        GameState::Dungeon     => GameState::Combat,
        GameState::Combat      => GameState::GameOver,
        GameState::GameOver    => GameState::Loading,
    };
    next.set(upcoming);
}

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
```

The `#[cfg(feature = "dev")]` block is added on the second `app.add_systems(...)` call only; the cycler function itself can also be `#[cfg(feature = "dev")]` to avoid an unused-function warning in release builds. Both annotations are needed for cleanest output.

### Anti-Patterns to Avoid

- **Using `EventReader<StateTransitionEvent<S>>`.** It will not compile on 0.18.1 because `StateTransitionEvent` is a `#[derive(Message)]`, not `#[derive(Event)]`. The reader is `MessageReader`. (Source: `bevy_state-0.18.1/src/state/transitions.rs:64-72`.) This is the single biggest 0.17→0.18 trap for state code. Older blog posts and even some `docs.rs/bevy/latest` snippets still show `EventReader`.
- **Calling `init_state` before `DefaultPlugins`.** It will panic at startup because the `StateTransition` schedule does not exist yet (`bevy_state-0.18.1/src/app.rs:96-99`). Always add `DefaultPlugins` first.
- **Mutating `State<S>` directly.** `State<S>` only exposes `get(&self) -> &S`; the inner field is `pub(crate)` (`bevy_state-0.18.1/src/state/resources.rs:58`). Always queue transitions via `NextState<S>::set`. Direct mutation is impossible without unsafe; don't try to work around it.
- **Reading `State<S>` immediately after `next.set(...)` in the same system and expecting the new value.** State transitions are deferred to the `StateTransition` schedule (runs after `PreUpdate`), so within the same frame's `Update` you still see the old value until the *next* frame's `Update`. (Source: `StatesPlugin::build` at `bevy_state-0.18.1/src/app.rs:305-312` shows `schedule.insert_after(PreUpdate, StateTransition)`.)
- **Forgetting `Default` on a `SubStates` enum.** The derive macro generates `Self::default()` (`bevy_state_macros-0.18.1/src/states.rs:122`); without `#[derive(Default)]` and a `#[default]` variant the code will not compile.
- **Trying to use a sub-state in `OnEnter` / `in_state` while the parent state is wrong.** The sub-state's `State<S>` resource is *removed from the world* when the parent isn't in the source variant (`bevy_state-0.18.1/src/state/sub_states.rs:7-8`, behaviour at `app.rs:161` `commands.remove_resource::<State<S>>()`). Run-conditions on a non-existent state return `false` from `in_state` (line 105-107 of `condition.rs`) — your system simply won't run, which is the desired behaviour but can be confusing if you expected a panic.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
| ------- | ----------- | ----------- | --- |
| State enum + run-condition gating | A `#[derive(Resource)] enum GameMode { ... }` plus manual `if mode == X` checks scattered across systems | `States` + `run_if(in_state(...))` | Bevy's `States` gives you OnEnter/OnExit schedules, transition events, automatic sub-state lifecycle, and run conditions for free. Hand-rolling all of that is 200+ LOC for no upside. |
| Sub-state lifecycle (sub-state only exists when parent is X) | `Option<Resource<DungeonSubState>>` plus manual insert/remove on parent transitions | `SubStates` derive with `#[source(...)]` | The derive generates `should_exist` and registers a system in `StateTransition::DependentTransitions` that adds/removes the resource for free (`bevy_state-0.18.1/src/state/sub_states.rs:170-175`). Hand-rolling is error-prone. |
| Cycling input for dev test | Custom event-bus or scripted controller | `Res<ButtonInput<KeyCode>>::just_pressed` directly | Three lines. The "scripted controller" pattern (Feature #5 with `leafwing-input-manager`) is for player-facing input remapping, not dev hotkeys. |

## Common Pitfalls

### Pitfall 1: The `Message` vs `Event` rename (the big one)

**What goes wrong:** code copy-pasted from older Bevy versions or some recent blog posts uses `EventReader<StateTransitionEvent<GameState>>`. Compiler error: `StateTransitionEvent` does not implement `Event`. New contributors waste 20-40 minutes searching docs.

**Why it happens:** Bevy 0.18 split the buffered-event API away from the trigger-based event API. Buffered events (the kind you read with a reader) are now `Message`/`MessageReader`/`MessageWriter`. The struct kept its `StateTransitionEvent` name (by-value continuity) but is now `#[derive(Message)]` (`bevy_state-0.18.1/src/state/transitions.rs:64`). `EventReader`/`EventWriter` are NOT in `bevy::prelude` for 0.18 — only `Message`/`MessageReader`/`MessageWriter` are (`bevy_ecs-0.18.1/src/lib.rs:81`).

**How to avoid:** always use `MessageReader<StateTransitionEvent<S>>` for reading state transitions. Reference the bevy_state source's own `state_scoped.rs:74, 142` as the authoritative usage.

### Pitfall 2: One-frame-deferred transitions

**What goes wrong:** a system calls `next.set(GameState::Combat)` and then a follow-up system in the same frame reads `Res<State<GameState>>` expecting it to be `Combat`. It will be the OLD state. Subtle game-flow bugs (e.g. spawning the combat scene immediately after `set`).

**Why it happens:** `StatesPlugin` inserts the `StateTransition` schedule **after `PreUpdate`** for the runtime tick (`bevy_state-0.18.1/src/app.rs:308`). So within a single `Update` step, all systems see whatever state was set at the *start* of the frame. The new state is observable in the next frame's `Update` (or in `OnEnter(NewState)` schedules during the same frame's `StateTransition` step, which is the supported way to react).

**How to avoid:** never read `State<S>` in the same `Update` system that wrote `NextState<S>` and expect the change. Put follow-up logic in `OnEnter(GameState::X)` instead. For Feature #2 the F9 cycler does NOT have this problem because its only effect is queueing `next.set(...)`; the debug logger picks it up next frame, which is fine for manual testing.

### Pitfall 3: Sub-state ordering on app startup

**What goes wrong:** a system tries to read `State<DungeonSubState>` very early in startup and finds the resource missing.

**Why it happens:** sub-states are only inserted when the parent state matches their `#[source(...)]` variant. At startup the default `GameState` is `Loading`, so `DungeonSubState`, `CombatPhase`, and `TownLocation` resources do **not exist** in the world. They will be created the first time the parent enters its source variant.

**How to avoid:** when querying a sub-state, always use `Option<Res<State<...>>>` or use `in_state(SubState::Variant)` run-conditions — `in_state` already returns `false` if the resource is absent (`bevy_state-0.18.1/src/condition.rs:103-108`). For Feature #2 we don't read the sub-states yet, so this is documented for future features.

### Pitfall 4: `Default` is a hard requirement on `SubStates` (not stated in the trait)

**What goes wrong:** a contributor reads the `SubStates` trait definition, sees no `Default` bound (`bevy_state-0.18.1/src/state/sub_states.rs:148-156`), omits `#[derive(Default)]`, and gets an opaque error from inside the macro expansion.

**Why it happens:** the derive macro generates `Self::default()` (`bevy_state_macros-0.18.1/src/states.rs:122`) but the *trait* doesn't require `Default`. Manual `impl SubStates` does not need `Default`; the derive does.

**How to avoid:** for every `#[derive(SubStates)]`, also derive `Default` and mark exactly one variant with `#[default]`. This research doc and any plan should make `Default` non-negotiable in the derive list.

### Pitfall 5: The `init_state` signature requires `FromWorld`, not `Default` directly

**What goes wrong:** rare — but a contributor manually implementing `States` for a custom type without `Default` will hit a `FromWorld` bound error that doesn't mention `Default`.

**Why it happens:** `init_state<S: FreelyMutableState + FromWorld>` (`bevy_state-0.18.1/src/app.rs:34`). `Default` blanket-impls `FromWorld`, so deriving `Default` is the path of least resistance and the path the official examples take. If you don't want `Default` (e.g. starting state should be configured via the world), implement `FromWorld` manually or use `insert_state` instead of `init_state`.

**How to avoid:** for Feature #2 just derive `Default` everywhere. Document this in the plan so no one tries to remove `Default` to "clean up" derives later.

## Security

### Known Vulnerabilities

| Library | CVE / Advisory | Severity | Status | Action |
| ------- | -------------- | -------- | ------ | ------ |
| `bevy_state` 0.18.1 | none found locally | — | — | None — pure-Rust, no I/O, no parsing of untrusted input |
| `bevy_input` 0.18.1 | none found locally | — | — | None — reads OS-provided keyboard events through `bevy_winit`, no untrusted-input parsing |

I cannot run `cargo audit` or query the RustSec advisory DB live in this session. Verification recipe for the planner/implementer to run before locking the plan:

```bash
cd /Users/nousunio/Repos/Learnings/claude-code/druum
cargo install cargo-audit       # if not already installed
cargo audit
```

Expected outcome: zero advisories for `bevy_state` and `bevy_input` 0.18.1 as of 2026-04-29. Any unexpected hit should be flagged before merging Feature #2.

### Architectural Security Risks

| Risk | Affected Architecture Options | How It Manifests | Secure Pattern | Anti-Pattern to Avoid |
| ---- | ----------------------------- | ---------------- | -------------- | --------------------- |
| Dev-only F9 cycler shipping in release builds | "Always-on F9" gating option | A player presses F9 by accident and skips the loading screen / nukes their save / flips to GameOver. Not a security risk per se, but a UX/integrity risk equivalent to debug-menu leakage. | `#[cfg(feature = "dev")]` on the cycler `add_systems` call (the recommended option). | Adding the cycler unconditionally and trusting future-you to remove it. |
| Future state-machine misuse: trusting `State<S>` for security decisions | All options | If later code ever decides "this action is allowed because state == Town", an attacker who can inject a `next.set(GameState::Town)` call (modded client, save-game tampering) bypasses the check. | Treat states as flow-control only. Validate gameplay actions against authoritative game data, not the state enum. | Using `in_state(...)` as a permission check for security-sensitive code paths. |

### Trust Boundaries

For a single-player local game with no network/save-load yet (saves arrive in Feature #23 per the roadmap), Feature #2 introduces no new trust boundaries. The keyboard input that drives F9 is already trusted (it comes through `bevy_winit` from the OS). When save/load lands later, restored state should be validated against an enum allowlist before being inserted via `insert_state` — out of scope here, flagged for Feature #23.

## Performance

| Metric | Value / Range | Source | Notes |
| ------ | ------------- | ------ | ----- |
| Per-frame cost of `state_changed::<S>` run condition | One `is_changed()` check on a single resource | `bevy_state-0.18.1/src/condition.rs:165-170` | Cost is one resource lookup + one `bool` field read. Negligible. |
| Per-frame cost of `MessageReader<StateTransitionEvent<S>>` | One reader-cursor advance per frame; only nonzero work when a transition was queued | `bevy_state-0.18.1/src/state/transitions.rs:212-217` | Negligible — bevy uses this internally for its own scheduling. |
| LOC delta vs. roadmap estimate | +80 to +120 LOC predicted; Pattern 1-6 above sum to roughly +90-110 LOC | Roadmap §Feature #2 Impact Analysis | Within range. |
| Cargo dependencies added | 0 | This research | `bevy_state` and `bevy_input` are already pulled in via `features = ["3d"]`. |
| Cold compile delta | Negligible (no new crates) | Inferred — no benchmark available | Verify by running `cargo build` before/after the implementation if curious. |

No formal benchmarks exist for state-machine throughput at the scale a game will hit — state transitions are inherently rare events (a few per second at most). Performance is not a concern for Feature #2.

## Code Examples

Complete reference assembly of Patterns 1-6 above — what the final `src/plugins/state/mod.rs` will roughly look like (counts ≈85 LOC including blank lines and comments, well within the 80-120 budget). This is illustrative only; do not paste verbatim into source — leave LOC distribution to the planner/implementer.

```rust
// src/plugins/state/mod.rs (illustrative)

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

#[derive(SubStates, Default, Debug, Clone, PartialEq, Eq, Hash)]
#[source(GameState = GameState::Combat)]
pub enum CombatPhase {
    #[default]
    PlayerInput,
    ExecuteActions,
    EnemyTurn,
    TurnResult,
}

#[derive(SubStates, Default, Debug, Clone, PartialEq, Eq, Hash)]
#[source(GameState = GameState::Town)]
pub enum TownLocation {
    #[default]
    Square,
    Shop,
    Inn,
    Temple,
    Guild,
}

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

#[cfg(feature = "dev")]
fn cycle_game_state_on_f9(
    keys: Res<ButtonInput<KeyCode>>,
    current: Res<State<GameState>>,
    mut next: ResMut<NextState<GameState>>,
) {
    if !keys.just_pressed(KeyCode::F9) {
        return;
    }
    let upcoming = match current.get() {
        GameState::Loading     => GameState::TitleScreen,
        GameState::TitleScreen => GameState::Town,
        GameState::Town        => GameState::Dungeon,
        GameState::Dungeon     => GameState::Combat,
        GameState::Combat      => GameState::GameOver,
        GameState::GameOver    => GameState::Loading,
    };
    next.set(upcoming);
}
```

`src/plugins/dungeon/mod.rs` after the change (illustrative):

```rust
use bevy::log::info;
use bevy::prelude::*;
use crate::plugins::state::GameState;

pub struct DungeonPlugin;

impl Plugin for DungeonPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::Dungeon), || info!("Entered GameState::Dungeon"))
            .add_systems(OnExit(GameState::Dungeon), || info!("Exited GameState::Dungeon"));
    }
}
```

Same pattern for `CombatPlugin` (using `GameState::Combat`) and `TownPlugin` (using `GameState::Town`).

`src/main.rs` change — add `StatePlugin` to the existing tuple (single-line edit):

```rust
.add_plugins((
    DefaultPlugins,
    StatePlugin,         // <-- new; must come after DefaultPlugins, which it does
    DungeonPlugin,
    CombatPlugin,
    PartyPlugin,
    TownPlugin,
    UiPlugin,
    AudioPlugin,
    SavePlugin,
))
```

`src/plugins/mod.rs` adds one line: `pub mod state;`.

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
| ------------ | ---------------- | ------------ | ------ |
| `Input<KeyCode>` resource | `ButtonInput<KeyCode>` resource | 0.13 (rename) | Old name no longer exists in 0.18; older tutorials are wrong. |
| `EventReader<StateTransitionEvent<S>>` | `MessageReader<StateTransitionEvent<S>>` | 0.18 | The 0.17→0.18 split of buffered-events into the `Message*` family. Older tutorials (incl. some `docs.rs/bevy/latest` snippets that still reference older crates) will not compile. |
| `app.add_state::<S>()` (very old) | `app.init_state::<S>()` | 0.13 (rename) | Documented historical name; long gone. |
| `Schedule::OnEnter(state)` style attributes | `OnEnter(GameState::X)` schedule label values | unchanged through 0.18 | This part is stable. |
| Manual `Resource` / hand-rolled state | `States` / `SubStates` derive | 0.10 onwards | The current built-in is canonical; do not reinvent. |

**Deprecated/outdated:**
- Anything using `Input<KeyCode>` — replaced by `ButtonInput<KeyCode>`.
- Anything using `EventReader<StateTransitionEvent<S>>` — replaced by `MessageReader<StateTransitionEvent<S>>` in 0.18.

## Validation Architecture

### Test Framework

| Property | Value |
| -------- | ----- |
| Framework | `cargo test` (built-in Rust); Bevy systems can be tested via `App::update()` against an isolated `App` instance |
| Config file | `Cargo.toml` (no special test framework); no integration test directory exists yet |
| Quick run command | `cargo test --no-run` (compile-only, ~5-30s after first build) |
| Full suite command | `cargo test` (will be empty for now, but should not error) |
| Smoke command | `cargo run --features dev` then visually verify F9 cycles state and logs print |

### Requirements → Test Map

| Requirement | Behavior | Test Type | Automated Command | File Exists? |
| ----------- | -------- | --------- | ----------------- | ------------ |
| `GameState::Loading` is the default after `init_state` | Inspect `Res<State<GameState>>` after one `App::update()` | unit / integration (Bevy `App` test) | `cargo test gamestate_default_is_loading` | ❌ needs creating |
| `init_state` + `add_sub_state` calls compile and don't panic | App constructs without error | unit (Bevy `App` test) | `cargo test stateplugin_builds` | ❌ needs creating |
| F9 cycler advances state (dev-feature only) | Press F9, verify `State<GameState>` transitions to next variant | integration (Bevy `App` test with `MinimalPlugins` + `StatePlugin` + simulated `ButtonInput` mutation) | `cargo test --features dev f9_cycles_state` | ❌ needs creating |
| OnEnter/OnExit stubs print on transition | Visual / log inspection | manual smoke | `cargo run --features dev` then watch stdout | n/a |
| Sub-states are inserted/removed with parent transitions | Transition `GameState` to `Dungeon` and verify `Res<State<DungeonSubState>>` exists; transition away and verify it is removed | integration (Bevy `App` test) | `cargo test substate_lifecycle` | ❌ needs creating (optional but recommended) |

### Gaps (files to create before implementation)

- [ ] `src/plugins/state/mod.rs` — the new state plugin (this is the feature itself, not a test; included here for the planner's "files to create" inventory)
- [ ] `src/plugins/state/tests.rs` (or a `#[cfg(test)] mod tests {}` inline at the bottom of `mod.rs`) — covers the four `cargo test` rows above. Inline `#[cfg(test)]` modules are standard Rust convention and avoid spawning a new file.
- [ ] No new top-level test infrastructure files needed; `cargo test` works out of the box.

A minimal Bevy 0.18 unit test pattern (verified shape from `bevy_state-0.18.1/src/app.rs:336-352` — the bevy_state crate's own tests):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use bevy::app::App;
    use bevy::state::app::StatesPlugin;

    #[test]
    fn gamestate_default_is_loading() {
        let mut app = App::new();
        app.add_plugins(StatesPlugin);  // minimal — no DefaultPlugins for unit tests
        app.add_plugins(StatePlugin);
        app.update();                    // run one frame so states are realised
        assert_eq!(*app.world().resource::<State<GameState>>(), GameState::Loading);
    }
}
```

Notes on test wiring:
- Use `StatesPlugin` directly (not `DefaultPlugins`) for fast unit tests. `StatesPlugin` is at `bevy::state::app::StatesPlugin` (`bevy_state-0.18.1/src/app.rs:303`).
- `*app.world().resource::<State<GameState>>() == GameState::Loading` works because `State<S>` implements `PartialEq<S>` (`bevy_state-0.18.1/src/state/resources.rs:80-84`) and `Deref` to `S`.
- Simulating F9 in a test: `app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::F9);` then `app.update();`. This is a light test of the cycler logic; for thorough coverage, also call `clear_just_pressed(KeyCode::F9)` between updates.

## Open Questions

1. **Do we want F9 to cycle *only* `GameState`, or also cycle the active sub-state when in Dungeon/Combat/Town?**
   - What we know: the user's task brief says "cycles `NextState<GameState>` for manual testing". Sub-state cycling is not requested.
   - What's unclear: whether the planner wants to extend the cycler to F10/F11 to cycle sub-states for richer manual testing, or defer to Feature #5 input system.
   - Recommendation: stick with F9 + GameState only for Feature #2. Add F10/F11 sub-state cyclers only if a follow-up feature requests them.

2. **Should the debug logger also log sub-state transitions?**
   - What we know: the brief says "logs every `GameState` transition". Sub-states are not mentioned.
   - What's unclear: future debugging convenience.
   - Recommendation: ship Feature #2 with `GameState`-only logging, and add sub-state loggers later as gaps appear during Feature #4-#7 development. Each is one extra `add_systems` line.

3. **Will the `Loading` default state cause any issue with the empty `LoadingPlugin` from Feature #3 not yet existing?**
   - What we know: the project starts in `GameState::Loading` because of `#[default]` and `init_state`. There is currently no system anywhere that transitions out of `Loading`.
   - What's unclear: whether Feature #2 should auto-transition out of `Loading` to `TitleScreen` after one frame, for development convenience.
   - Recommendation: do nothing. Default to `Loading`. The F9 hotkey provides the manual escape hatch in dev builds. Auto-advance is Feature #3's job (asset pipeline → `TitleScreen`).

## Sources

### Primary (HIGH confidence)

All paths below are absolute on the current machine; the line numbers are exact.

- [bevy_state-0.18.1/src/lib.rs](file:///Users/nousunio/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bevy_state-0.18.1/src/lib.rs) — module structure, prelude exports (lines 77-97), required `StatesPlugin`.
- [bevy_state-0.18.1/src/state/states.rs](file:///Users/nousunio/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bevy_state-0.18.1/src/state/states.rs) — `States` trait definition (line 64), required derives, canonical example (lines 28-58).
- [bevy_state-0.18.1/src/state/sub_states.rs](file:///Users/nousunio/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bevy_state-0.18.1/src/state/sub_states.rs) — `SubStates` trait (line 148), `#[source(...)]` example (lines 24-32), manual-impl alternative (lines 100-141).
- [bevy_state_macros-0.18.1/src/states.rs](file:///Users/nousunio/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bevy_state_macros-0.18.1/src/states.rs) — derive macro source: `#[source(...)]` parsing (lines 41-82) and `Self::default()` codegen (line 122).
- [bevy_state-0.18.1/src/app.rs](file:///Users/nousunio/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bevy_state-0.18.1/src/app.rs) — `AppExtStates` trait (lines 18-78), `init_state` impl (lines 90-114), `add_sub_state` impl (lines 180-209), `StatesPlugin` schedule wiring (lines 303-312).
- [bevy_state-0.18.1/src/state/resources.rs](file:///Users/nousunio/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bevy_state-0.18.1/src/state/resources.rs) — `State<S>` (lines 52-92), `NextState<S>` enum + `set`/`set_if_neq`/`reset` (lines 118-159).
- [bevy_state-0.18.1/src/state/transitions.rs](file:///Users/nousunio/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bevy_state-0.18.1/src/state/transitions.rs) — `OnEnter`/`OnExit`/`OnTransition` schedule labels (lines 16-36), `StateTransitionEvent` derive(Message) (line 64), `last_transition` helper (lines 213-217).
- [bevy_state-0.18.1/src/condition.rs](file:///Users/nousunio/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bevy_state-0.18.1/src/condition.rs) — `state_exists`, `in_state`, `state_changed` run conditions (lines 47-170).
- [bevy_state-0.18.1/src/state/freely_mutable_state.rs](file:///Users/nousunio/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bevy_state-0.18.1/src/state/freely_mutable_state.rs) — `FreelyMutableState` trait, transition-system registration.
- [bevy_state-0.18.1/src/state_scoped.rs](file:///Users/nousunio/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bevy_state-0.18.1/src/state_scoped.rs) — idiomatic `MessageReader<StateTransitionEvent<S>>` usage in bevy itself (lines 74, 142).
- [bevy_input-0.18.1/src/lib.rs](file:///Users/nousunio/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bevy_input-0.18.1/src/lib.rs) — `InputPlugin` registers `ButtonInput<KeyCode>` resource (line 114), prelude re-exports (lines 46-65).
- [bevy_input-0.18.1/src/button_input.rs](file:///Users/nousunio/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bevy_input-0.18.1/src/button_input.rs) — `ButtonInput<T>` struct (line 125), `pressed` (157), `just_pressed` (188), `just_released` (207).
- [bevy_input-0.18.1/src/keyboard.rs](file:///Users/nousunio/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bevy_input-0.18.1/src/keyboard.rs) — `KeyCode` enum (line 269), `KeyCode::F9` variant (line 669).
- [bevy_internal-0.18.1/src/prelude.rs](file:///Users/nousunio/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bevy_internal-0.18.1/src/prelude.rs) — prelude re-exports of `bevy_state::prelude::*` (line 95-96) and `bevy_input::prelude::*` (line 3, transitively).
- [bevy_internal-0.18.1/src/default_plugins.rs](file:///Users/nousunio/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bevy_internal-0.18.1/src/default_plugins.rs) — `StatesPlugin` is part of `DefaultPlugins` (lines 84-85).
- [bevy-0.18.1/Cargo.toml](file:///Users/nousunio/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bevy-0.18.1/Cargo.toml) — feature graph: `"3d"` includes `default_app` (line 2322-2330), `default_app` includes `bevy_state` (line 2437-2446).
- [bevy_ecs-0.18.1/src/lib.rs](file:///Users/nousunio/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bevy_ecs-0.18.1/src/lib.rs) — prelude exports `Message`, `MessageReader`, `MessageWriter`, `Messages` (line 81); does NOT export `EventReader`/`EventWriter`.
- [bevy_ecs-0.18.1/src/message/mod.rs](file:///Users/nousunio/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bevy_ecs-0.18.1/src/message/mod.rs) — `Message` is the buffered-event family in 0.18 (lines 35-49 docstring).
- [Cargo.lock](file:///Users/nousunio/Repos/Learnings/claude-code/druum/Cargo.lock) — verified versions: `bevy = "0.18.1"` (line 341), `bevy_input = "0.18.1"` (line 891), `bevy_state = "0.18.1"` (line 1400).

### Secondary (MEDIUM confidence)

- [Bevy 0.17→0.18 Migration Guide on bevy.org](https://bevy.org/learn/migration-guides/0-17-to-0-18/) — the public migration guide describing the `Event` → `Message` rename. Could not be fetched live in this session; the rename itself is verified directly from the 0.18.1 source. Verification recipe: open the page in a browser and check the "Events to Messages" or similar section. Accessed: not in this session.
- [docs.rs/bevy/0.18.1/bevy/state/index.html](https://docs.rs/bevy/0.18.1/bevy/state/index.html) — official rustdoc for the exact pinned version. Equivalent to the source files cited above; consult if you want hyperlinked browsing. Could not be fetched live in this session.
- [Existing project research doc 20260326-01](/Users/nousunio/Repos/Learnings/claude-code/druum/research/20260326-01-bevy-first-person-dungeon-crawler-rpg.md) — the prior research that informed the roadmap. Its `Pattern 1: Game State Machine with SubStates` example (lines 247-328) is mostly correct against 0.18.1 but predates the `Event`→`Message` rename and does not cover the `MessageReader` requirement.

### Tertiary (LOW confidence)

- None used. Every claim in this document was traced to a primary source on disk.

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — the entire stack is built into Bevy 0.18.1 and verified by reading the crate source directly.
- Architecture (file-layout choice): HIGH — derived from the Feature #1 plugin convention captured in `project_druum_skeleton.md`; the convention is established and the recommendation is consistent.
- Architecture (F9 dev-feature gating): HIGH — leverages the `dev` Cargo feature already in `Cargo.toml:27` from Feature #1.
- Pitfalls: HIGH — every pitfall cited has a file/line reference in the bevy_state-0.18.1 source.
- Performance: MEDIUM — no formal benchmarks fetched (no live network); the qualitative claims (negligible cost) are supported by the size of the per-frame work shown in the source. State transitions are inherently rare events, so this MEDIUM does not gate Feature #2.
- Security: MEDIUM — no `cargo audit` was run in this session. Verification recipe provided in the Security section. The architectural risks are HIGH-confidence (just process design).

**Research date:** 2026-04-29

**Implementer's must-do checklist (collated from this doc, in order):**
1. Create `src/plugins/state/mod.rs` with `GameState`, the three sub-states (each `#[derive(SubStates, Default, Debug, Clone, PartialEq, Eq, Hash)]` + `#[source(GameState = GameState::Variant)]`), `StatePlugin`, the debug logger, and the `#[cfg(feature = "dev")]`-gated F9 cycler.
2. Add `pub mod state;` to `src/plugins/mod.rs`.
3. Add `StatePlugin` to the `add_plugins((...))` tuple in `src/main.rs`, immediately after `DefaultPlugins`.
4. Add `OnEnter` / `OnExit` placeholder log systems to `DungeonPlugin`, `CombatPlugin`, `TownPlugin` (each imports `GameState` from `crate::plugins::state`).
5. Use `MessageReader<StateTransitionEvent<S>>` if you switch from `state_changed` to event-reader logging — never `EventReader`.
6. Test with `cargo run --features dev` and press F9 repeatedly; expect six log lines cycling through the six `GameState` variants, plus an "Entered/Exited" line each time you cross `Town`/`Dungeon`/`Combat`.
7. Run `cargo audit` once to confirm no advisories on `bevy_state` 0.18.1.
