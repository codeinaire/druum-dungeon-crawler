# Plan: Bevy 0.18.1 Game State Machine (Feature #2)

**Date:** 2026-04-29
**Status:** Complete
**Research:** ../research/20260429-030000-bevy-0-18-1-state-machine-feature-2.md
**Depends on:** 20260429-022500-bevy-0-18-1-skeleton-init.md

## Goal

Add a `GameState` top-level state machine plus three sub-states (`DungeonSubState`, `CombatPhase`, `TownLocation`) wired through a new `StatePlugin`, with `OnEnter`/`OnExit` log stubs in the dungeon/combat/town plugins, a debug system that logs every `GameState` transition, and a dev-only F9 hotkey that cycles `NextState<GameState>` for manual testing.

## Approach

The research verified directly against the on-disk `bevy_state-0.18.1` and `bevy_input-0.18.1` source that the entire feature is achievable with no new dependencies (the existing `features = ["3d"]` already pulls in `bevy_state`, and `DefaultPlugins` already installs `StatesPlugin`). Implementation follows Option 2 from the research: state code lives at `src/plugins/state/mod.rs` to match the seven-flat-plugin convention from Feature #1.

The single biggest 0.18-specific landmine is the `Event`→`Message` rename — `StateTransitionEvent<S>` is now a `Message`, not an `Event`, so reading transitions requires `MessageReader`, not `EventReader`. We sidestep this entirely by using the canonical `state_changed::<GameState>` run condition for the debug logger (one-line system body, prints the new state value — research §Pattern 4 recommendation). The F9 cycler reads `Res<ButtonInput<KeyCode>>::just_pressed(KeyCode::F9)` and queues `NextState<GameState>::set(...)`. The cycler function and its `add_systems` registration are both `#[cfg(feature = "dev")]` gated so it does not ship in release builds (research §F9 hotkey gating).

`OnEnter`/`OnExit` stubs in `DungeonPlugin`, `CombatPlugin`, and `TownPlugin` are written as zero-arg closures (`|| info!("...")`) — Bevy 0.18 accepts closures as systems, and this is the smallest legitimate stub. Each subsystem plugin gets one new `use crate::plugins::state::GameState;` import; we deliberately do not re-export `GameState` from the crate root (research §Pattern 5: "do not hide where it lives").

A small `#[cfg(test)] mod tests {}` block at the bottom of `src/plugins/state/mod.rs` covers the two highest-value unit checks (default state == `Loading`, F9 cycler advances state) using the `StatesPlugin` minimal-test pattern from `bevy_state-0.18.1/src/app.rs:336-352`. These add ~25 LOC and stay well within the 80-120 LOC budget.

## Critical

- **Use `MessageReader<StateTransitionEvent<S>>`, NOT `EventReader<StateTransitionEvent<S>>`**, if any code in this feature ever needs to read transitions directly. Research §Pitfall 1 — `EventReader` will not compile on 0.18. The recommended `state_changed::<GameState>` run condition avoids this entirely; do not switch to event-reader logging without re-reading the pitfall.
- **`Default` is a hard requirement on every `SubStates` derive**, even though the trait definition does not list it (research §Pitfall 4). The derive macro generates `Self::default()`; omitting `#[derive(Default)]` produces an opaque error from inside macro expansion.
- **`StatePlugin` must be added AFTER `DefaultPlugins`** in `main.rs`. `init_state` and `add_sub_state` panic with `"The 'StateTransition' schedule is missing"` if `StatesPlugin` (transitively from `DefaultPlugins`) was not added first. Tuple order in `add_plugins((...))` is preserved; place `StatePlugin` immediately after `DefaultPlugins`.
- **State transitions are deferred by one frame** (research §Pitfall 2). After `next.set(...)`, the new value is observable in the *next* frame's `Update` (or in `OnEnter` schedules during the same frame's `StateTransition` step). The F9 cycler relies on this: it sets the next state and the debug logger prints it on the following frame. Do not assume the logger sees the new state in the same frame as the F9 press.
- **No new dependencies.** Do not edit `Cargo.toml`. `bevy_state` and `bevy_input` are already pulled in via `features = ["3d"]`; verified by research against `bevy-0.18.1/Cargo.toml:2322-2330` and `Cargo.lock:341, 891, 1400`.
- **No `rand` calls anywhere.** Roadmap design decision #23 will need deterministic RNG seeding later. The F9 cycler uses a deterministic `match` on the current state; do not introduce randomness.
- **Do not mutate `State<S>` directly** — the inner field is `pub(crate)` and there is no setter. Always queue via `next.set(...)` (research §Anti-Patterns).
- **Do not re-export `GameState` from the crate root** to shorten import paths. Each subsystem plugin must `use crate::plugins::state::GameState;` explicitly so its origin is greppable (research §Pattern 5).
- **F9 cycler and its function definition both need `#[cfg(feature = "dev")]`** — one annotation on the `add_systems` call inside `StatePlugin::build`, one on the function itself. Both are required for clean release-build output (research §Pattern 6 closing note).

## Steps

### Step 1: Create `src/plugins/state/mod.rs` with the state enums and `StatePlugin`

- [x] Create the new file `/Users/nousunio/Repos/Learnings/claude-code/druum/src/plugins/state/mod.rs`.
- [x] Add module-level doc comment: `//! Game state machine — top-level GameState plus three SubStates, debug transition logger, and dev-only F9 cycler.`
- [x] Add the imports:
  ```rust
  use bevy::log::info;
  use bevy::prelude::*;
  ```
- [ ] Define `GameState` with these exact derives, in this order:
  ```rust
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
  Verify the variant order matches the roadmap (Loading first because it is `#[default]`). Do NOT add `Copy` (research §Pattern 1: variants may grow data-carrying forms later).
- [x] Define `DungeonSubState`, `CombatPhase`, and `TownLocation` with their `#[source(...)]` attributes. Use the **exact** attribute syntax `#[source(GameState = GameState::Variant)]` (left side type path, right side pattern — research §Pattern 2):
  ```rust
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
  Each SubStates derive **must** include `Default` (research §Pitfall 4: required by the macro even though the trait doesn't list it). Each must have exactly one `#[default]` variant.
- [x] Define the `StatePlugin` struct (unit struct) and its `Plugin` impl. Inside `build`, register `init_state::<GameState>()` first, then chain three `add_sub_state::<...>()` calls, then add the debug logger system gated by `state_changed::<GameState>`:
  ```rust
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
  ```
- [x] Define `log_game_state_transition`:
  ```rust
  fn log_game_state_transition(state: Res<State<GameState>>) {
      info!("GameState -> {:?}", state.get());
  }
  ```
- [x] Define the dev-only F9 cycler. The function definition itself is `#[cfg(feature = "dev")]` so release builds do not produce a dead-code warning:
  ```rust
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
  Note: the cycle order is `Loading → TitleScreen → Town → Dungeon → Combat → GameOver → Loading`. This matches the variant declaration order so a reader can predict the next state at a glance.

**Done state:** `src/plugins/state/mod.rs` exists and contains four enums (one States, three SubStates), the `StatePlugin` struct + impl, the logger function, and the cfg-gated F9 cycler. No tests yet — those are added in Step 6.

### Step 2: Register the new module in `src/plugins/mod.rs`

- [x] Open `/Users/nousunio/Repos/Learnings/claude-code/druum/src/plugins/mod.rs`.
- [x] Add `pub mod state;` to the existing module list. Place it alphabetically between `save` and `town` so the file remains sorted, OR at the top of the list — either is acceptable; the existing file is already alphabetical (audio, combat, dungeon, party, save, town, ui), so insert `state` between `save` and `town` to preserve that ordering.

**Done state:** `cargo check` resolves `crate::plugins::state` from anywhere in the crate.

### Step 3: Wire `StatePlugin` into `src/main.rs`

- [x] Open `/Users/nousunio/Repos/Learnings/claude-code/druum/src/main.rs`.
- [x] Add `state::StatePlugin` to the `use druum::plugins::{...}` import block, alphabetically (between `save::SavePlugin` and `town::TownPlugin`).
- [x] Add `StatePlugin,` to the `add_plugins((...))` tuple **immediately after `DefaultPlugins`** (research §Pattern 3 hard rule — `init_state` panics if `StatesPlugin` is not already installed). Final tuple order:
  ```rust
  .add_plugins((
      DefaultPlugins,
      StatePlugin,    // must come after DefaultPlugins
      DungeonPlugin,
      CombatPlugin,
      PartyPlugin,
      TownPlugin,
      UiPlugin,
      AudioPlugin,
      SavePlugin,
  ))
  ```

**Done state:** `cargo check` succeeds; `cargo run --features dev` (parent runs this in verification) prints `GameState -> Loading` once on startup because `state_changed` returns true the frame the resource is added.

### Step 4: Add `OnEnter`/`OnExit` stubs to `DungeonPlugin`, `CombatPlugin`, `TownPlugin`

- [x] Open `/Users/nousunio/Repos/Learnings/claude-code/druum/src/plugins/dungeon/mod.rs`. Replace the file body so it imports `bevy::log::info` and `crate::plugins::state::GameState`, and registers two systems:
  ```rust
  use bevy::log::info;
  use bevy::prelude::*;

  use crate::plugins::state::GameState;

  /// Dungeon exploration plugin — grid movement, fog of war, encounter triggers.
  /// Feature #2 wires OnEnter/OnExit log stubs; gameplay systems land in #4, #7, #8, #11.
  pub struct DungeonPlugin;

  impl Plugin for DungeonPlugin {
      fn build(&self, app: &mut App) {
          app.add_systems(OnEnter(GameState::Dungeon), || info!("Entered GameState::Dungeon"))
              .add_systems(OnExit(GameState::Dungeon), || info!("Exited GameState::Dungeon"));
      }
  }
  ```
  The closures are zero-arg (`|| info!(...)`) — Bevy 0.18's `IntoSystem` impl accepts them as the smallest legitimate system body (research §Pattern 5).
- [x] Open `/Users/nousunio/Repos/Learnings/claude-code/druum/src/plugins/combat/mod.rs`. Apply the same pattern with `GameState::Combat` for both schedule labels and the doc comment update:
  ```rust
  use bevy::log::info;
  use bevy::prelude::*;

  use crate::plugins::state::GameState;

  /// Turn-based combat plugin — initiative, actions, damage resolution.
  /// Feature #2 wires OnEnter/OnExit log stubs; gameplay systems land in #12-#16.
  pub struct CombatPlugin;

  impl Plugin for CombatPlugin {
      fn build(&self, app: &mut App) {
          app.add_systems(OnEnter(GameState::Combat), || info!("Entered GameState::Combat"))
              .add_systems(OnExit(GameState::Combat), || info!("Exited GameState::Combat"));
      }
  }
  ```
- [x] Open `/Users/nousunio/Repos/Learnings/claude-code/druum/src/plugins/town/mod.rs`. Apply the same pattern with `GameState::Town`:
  ```rust
  use bevy::log::info;
  use bevy::prelude::*;

  use crate::plugins::state::GameState;

  /// Town hub plugin — shop, inn, temple, guild interactions.
  /// Feature #2 wires OnEnter/OnExit log stubs; gameplay systems land in #19-#22.
  pub struct TownPlugin;

  impl Plugin for TownPlugin {
      fn build(&self, app: &mut App) {
          app.add_systems(OnEnter(GameState::Town), || info!("Entered GameState::Town"))
              .add_systems(OnExit(GameState::Town), || info!("Exited GameState::Town"));
      }
  }
  ```

Do **not** modify `PartyPlugin`, `UiPlugin`, `AudioPlugin`, or `SavePlugin` — they are out of scope for Feature #2 (no roadmap-mandated state-driven entry/exit yet).

**Done state:** `cargo check` and `cargo clippy --all-targets -- -D warnings` both pass. The closures take no arguments and return nothing, so clippy will not flag them. The `info` import is used inside the closure bodies.

### Step 5: Verify clippy cleanliness — confirm release builds compile cleanly without `dev`

- [x] Re-read `src/plugins/state/mod.rs` and confirm both the F9 cycler function and its `add_systems` call inside `StatePlugin::build` are wrapped in `#[cfg(feature = "dev")]`. If only one was wrapped, release builds emit an `unused function: cycle_game_state_on_f9` warning that fails `clippy --all-targets -- -D warnings`. Both must be cfg-gated.
- [x] Confirm no `use` statement imports an item only used inside a `#[cfg(feature = "dev")]` block — if it does, the import itself must also be `#[cfg(feature = "dev")]` to avoid an unused-import warning in non-dev builds. Specifically verify: `ButtonInput`, `KeyCode`, `NextState` are only used in the cycler. If they come solely from `bevy::prelude::*`, no separate import statement exists for them and there is nothing to gate. (`bevy::prelude::*` is used by both the cycler and other code, so it remains unconditional.) If you added explicit imports for any of those types, gate them.
- [x] Confirm the `#[cfg(test)]` test block (added in Step 6) does not leak any items that the non-test build doesn't need.

**Done state:** Step 5 is a static review pass — no new code is written. The implementer reads through the file once and confirms cfg gating is symmetric on the dev path. This guards against the most common cause of `clippy --all-targets -- -D warnings` failures for cfg-gated systems.

### Step 6: Add inline `#[cfg(test)] mod tests {}` covering default state and F9 cycle (optional within budget)

This step adds ~25 LOC of inline tests to `src/plugins/state/mod.rs`. Skip this step ONLY if Steps 1-5 already pushed the file over 120 LOC; otherwise include it because the F9-cycle test is the most direct way to verify the most surprising 0.18 API behavior (deferred transitions). Verified test pattern is from `bevy_state-0.18.1/src/app.rs:336-352`.

- [x] Append the following block at the bottom of `src/plugins/state/mod.rs` (after the F9 cycler, after the closing `}` of any prior items):
  ```rust
  #[cfg(test)]
  mod tests {
      use super::*;
      use bevy::state::app::StatesPlugin;

      /// Default `GameState` is `Loading` immediately after `init_state`.
      #[test]
      fn gamestate_default_is_loading() {
          let mut app = App::new();
          app.add_plugins(StatesPlugin).add_plugins(StatePlugin);
          app.update();
          assert_eq!(*app.world().resource::<State<GameState>>(), GameState::Loading);
      }

      /// Pressing F9 advances `GameState` to the next variant in cycle order.
      /// Verifies the one-frame deferral: `set` queues the change; the new value
      /// is observable after the next `app.update()` runs the `StateTransition` schedule.
      #[cfg(feature = "dev")]
      #[test]
      fn f9_advances_game_state() {
          let mut app = App::new();
          app.add_plugins(StatesPlugin).add_plugins(StatePlugin);
          app.update(); // realise initial state -> Loading

          // Simulate F9 press.
          app.world_mut()
              .resource_mut::<ButtonInput<KeyCode>>()
              .press(KeyCode::F9);
          app.update(); // cycler runs, queues NextState; transition realised same frame
          app.update(); // post-transition Update sees the new state

          assert_eq!(
              *app.world().resource::<State<GameState>>(),
              GameState::TitleScreen,
          );
      }
  }
  ```
  Notes on test wiring:
  - `StatesPlugin` lives at `bevy::state::app::StatesPlugin` (verified at `bevy_state-0.18.1/src/app.rs:303`).
  - `*app.world().resource::<State<GameState>>() == GameState::Loading` works because `State<S>` implements `PartialEq<S>` (verified at `bevy_state-0.18.1/src/state/resources.rs:80-84`).
  - The F9 test is `#[cfg(feature = "dev")]` because the cycler function itself is dev-gated; running the test under `cargo test` (default features) would not find the system.
  - Adding `StatesPlugin` explicitly is necessary: in a unit-test `App::new()`, `DefaultPlugins` is not present, so the `StateTransition` schedule that `init_state` requires is not yet there. (This mirrors the pattern in `bevy_state-0.18.1/src/app.rs:336-352`.)
  - For thorough multi-press testing in future, call `app.world_mut().resource_mut::<ButtonInput<KeyCode>>().clear_just_pressed(KeyCode::F9);` between updates. Not needed for this two-update test.

**Done state:** `cargo test` (default features) runs `gamestate_default_is_loading` and passes. `cargo test --features dev` runs both tests and passes. Tests fit within the 120 LOC budget for the file.

### Step 7: Re-read the final file to confirm LOC budget and convention adherence

- [x] Read `src/plugins/state/mod.rs` end to end. Count non-blank-non-comment LOC. Expect ~85-115 LOC after Steps 1-6 (research §Performance estimated ~90-110). If over 120, identify whether the test block (Step 6) can be trimmed; if under 80, the file is fine.
- [x] Confirm no `EventReader<...>` appears anywhere in the file — only `MessageReader<...>` would be valid for `StateTransitionEvent<S>`, but neither should be needed at all because we use the `state_changed` run condition.
- [x] Confirm every `SubStates` derive lists `Default` (Pitfall 4 guard).
- [x] Confirm every `SubStates` enum has exactly one `#[default]` variant and that variant matches the roadmap (`Exploring` for Dungeon, `PlayerInput` for Combat, `Square` for Town).
- [x] Confirm `cycle_game_state_on_f9` and its `add_systems` registration are both `#[cfg(feature = "dev")]`.
- [x] Confirm `main.rs` lists `StatePlugin` immediately after `DefaultPlugins` in the `add_plugins` tuple — not anywhere else.

**Done state:** All seven checks pass on visual inspection. The plan is ready for the parent session's automated verification (Verification section below).

## Security

**Known vulnerabilities:** None identified as of 2026-04-29. Research §Security checked the local registry for `bevy_state` 0.18.1 and `bevy_input` 0.18.1 and found no advisories. Both are pure-Rust crates with no I/O or untrusted-input parsing — `bevy_state` is type-level state-machine plumbing, `bevy_input` reads OS-level keyboard events through `bevy_winit`. The research documented a `cargo audit` recipe for the parent session to run as a one-time confirmation; not a blocker.

**Architectural risks:**

- **Dev-only F9 cycler must not ship in release builds.** Both the function definition and the `add_systems` call are `#[cfg(feature = "dev")]`-gated (Step 1, Step 5 verification). A player accidentally pressing F9 in a shipped binary would skip loading screens or flip to `GameOver`; this is a UX/integrity risk equivalent to debug-menu leakage. The `dev` feature is opt-in only (per `project_druum_skeleton.md` Feature #1 decision: never add `bevy/dynamic_linking` to `default`), so the cycler is automatically excluded from release builds.
- **Do not use `in_state(...)` as a permission check for security-sensitive code paths.** Treat states as flow-control only. Validate gameplay actions against authoritative game data, not the state enum (research §Security). For Feature #2 there are no security-sensitive paths yet, but this constraint applies forward.
- **Trust boundaries:** none introduced by Feature #2. F9 keyboard input arrives via `bevy_winit` from the OS (already trusted); save-game state restoration is Feature #23's concern.

## Open Questions

All three open questions from the research are resolved by the user's task brief:

1. **F9 cycles only `GameState`, not sub-states.** (Resolved: task brief "Out of scope: Sub-state cycling (only top-level `GameState` in F9)".)
2. **Logger logs only `GameState` transitions, not sub-state transitions.** (Resolved: task brief "Out of scope: Sub-state transition logging (only top-level `GameState`)".)
3. **`Loading` is the default state and does not auto-transition.** (Resolved: task brief "Loading is default but don't load anything" — F9 provides the manual escape; auto-transition is Feature #3's job.)

## Implementation Discoveries

**Discovery 1 — `InputPlugin.keyboard_input_system` clears `just_pressed` in PreUpdate (test interference):**
The plan's test pattern (`StatesPlugin + StatePlugin, press(F9), update()`) fails when compiled under `--features dev` because: (a) `StatePlugin::build` registers `cycle_game_state_on_f9` via `#[cfg(feature = "dev")]`, which requires `ButtonInput<KeyCode>` as a resource; (b) the plan suggested adding `InputPlugin` for the resource; but (c) `InputPlugin` also registers `keyboard_input_system` in `PreUpdate`, which calls `keycode_input.bypass_change_detection().clear()` at the top of every frame — so `just_pressed` set manually before `app.update()` is cleared before `Update` runs the cycler.

Fix applied: instead of `add_plugins(InputPlugin)`, call `init_resource::<ButtonInput<KeyCode>>()` directly. This inserts the resource without the clearing system, so manually-set `just_pressed` survives to the `Update` cycler. Both tests now pass under both feature sets.

**Discovery 2 — Both tests need the `ButtonInput<KeyCode>` resource when compiled under `dev` feature:**
Even `gamestate_default_is_loading` (which doesn't test F9) panics under `--features dev` without the resource, because `StatePlugin::build` always registers `cycle_game_state_on_f9` via `#[cfg(feature = "dev")]` and Bevy 0.18's system parameter validation runs during `app.update()`. Solution: `#[cfg(feature = "dev")] app.init_resource::<ButtonInput<KeyCode>>()` inside the `gamestate_default_is_loading` test.

**Discovery 3 — LOC count is ~117 (slightly over 115 target, well under 120 budget):**
The final file is 139 total lines with approximately 117 non-blank non-comment lines. The extra lines vs the 115 target are inline `//` comments inside the test block explaining the InputPlugin/clearing interaction. These are preserved because they document non-obvious Bevy 0.18 behavior that future readers will need. Well within the 120 hard budget.

## Verification

The implementer agent in this environment has no Bash tool. The first three items below are deferred to the parent session, which has Bash. The implementer should still confirm via Read that the files are written correctly and the code compiles in their head.

- [x] **Compile check passes** — `cargo check` — passed with zero errors.
- [x] **Clippy is clean across all targets** — `cargo clippy --all-targets -- -D warnings` — passed with zero warnings.
- [x] **Clippy is also clean with the dev feature** — `cargo clippy --all-targets --features dev -- -D warnings` — passed with zero warnings.
- [x] **Tests pass under default features** — `cargo test` — 1 passed (`gamestate_default_is_loading`), 0 failed.
- [x] **Tests pass under dev features** — `cargo test --features dev` — 2 passed (`gamestate_default_is_loading`, `f9_advances_game_state`), 0 failed.
- [ ] **Manual smoke test — F9 cycles state and console logs print** — manual — `cargo run --features dev`, then press F9 six times — Run by **parent session**. Expected: at startup, console prints `GameState -> Loading` (because `state_changed` returns true the frame the resource is added). Each F9 press produces, on the *following* frame: a `GameState -> X` log line (where X is the next variant in cycle order) plus, when crossing into `Town`, `Dungeon`, or `Combat`, an `Entered GameState::X` line, and when crossing out of those, an `Exited GameState::X` line. After six presses the cycle returns to `Loading`. The window stays responsive throughout; Cmd+Q closes cleanly.
- [x] **No `cargo update` side effects** — `git diff Cargo.lock` — empty diff. Cargo.lock unchanged; no new dependencies introduced.
- [ ] **(Optional) Security audit clean** — manual — `cargo install cargo-audit && cargo audit` — Run by **parent session** if not already part of CI. Expected: zero advisories on `bevy_state` 0.18.1 or `bevy_input` 0.18.1 as of 2026-04-29. Not a blocker for merge; flag any unexpected hit.
