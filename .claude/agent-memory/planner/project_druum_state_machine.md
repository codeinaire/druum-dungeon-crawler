---
name: Druum state machine decisions (Feature #2)
description: Architectural decisions baked into Feature #2 (Bevy 0.18.1 state machine) that constrain Features #3, #5, and forward
type: project
---

Feature #2 introduces `GameState` (top-level), `DungeonSubState`, `CombatPhase`, `TownLocation` (sub-states) via `StatePlugin` at `src/plugins/state/mod.rs`. Several decisions here will shape later features:

- **`GameState::Loading` is the default and intentionally has no auto-advance.** The F9 dev hotkey is the only way out in Feature #2. **Why:** Feature #3 (asset/RON pipeline) owns the `Loading → TitleScreen` transition and must not be pre-empted. **How to apply:** Feature #3's `LoadingPlugin` is the place to add an `OnEnter(GameState::Loading)` system that kicks off asset loading and a "load complete" trigger that calls `next.set(GameState::TitleScreen)`. Do not auto-advance from anywhere else.

- **F9 hotkey cycler is a `#[cfg(feature = "dev")]`-gated stub, not the real input system.** Lives inside `StatePlugin::build` for now. **Why:** Feature #5 (input system with `leafwing-input-manager`) is the canonical input owner. F9 is a temporary debug affordance to unblock manual testing of states/sub-states until Feature #5 lands. **How to apply:** when Feature #5 ships, the F9 cycler stays for dev convenience but actual gameplay input bindings (movement, menu open, etc.) go through the new input system, not through `Res<ButtonInput<KeyCode>>` reads scattered through plugins. If a 3rd dev-only system accumulates, promote the cycler into a separate `DevPlugin`.

- **Sub-state cycling and sub-state transition logging are deliberately out of scope for Feature #2.** Only top-level `GameState` is cycled by F9 and logged. **Why:** task brief explicitly excluded both to keep the LOC budget tight (+80 to +120 LOC) and to defer per-system manual-test ergonomics until specific features need them. **How to apply:** if a later feature wants to cycle through `DungeonSubState` (e.g. for testing menu screens), add F10/F11 cyclers in that feature's plugin — do not retrofit them into `StatePlugin`. Same for sub-state loggers: one extra `add_systems(Update, log_X_transition.run_if(state_changed::<X>))` line in the relevant plugin.

- **`StateTransitionEvent<S>` is a `Message`, not an `Event`, in Bevy 0.18.** Reading transitions requires `MessageReader`, not `EventReader`. **Why:** the 0.17→0.18 buffered-event split renamed the family. `EventReader<StateTransitionEvent<S>>` will not compile. **How to apply:** any future code that wants to read transitions directly (e.g. for telemetry, for save-on-state-change) must use `MessageReader<StateTransitionEvent<S>>`. The Feature #2 debug logger uses the simpler `state_changed::<S>` run condition to avoid the trap entirely; later code should follow the same pattern unless it specifically needs the previous state.

- **`Default` is required on every `SubStates` derive** (the trait does not list it; the macro generates `Self::default()`). **How to apply:** when adding new sub-states later (e.g. for a future `MenuState`), always derive `Default` and mark exactly one variant with `#[default]`. Manual `impl SubStates` does not need `Default`, but no one in this project should be writing manual impls — use the derive.

- **Subsystem plugins (`DungeonPlugin`, `CombatPlugin`, `TownPlugin`) import `GameState` directly via `use crate::plugins::state::GameState;`.** No re-export from crate root. **Why:** keeping the import path explicit makes ownership greppable; re-exports hide where types live. **How to apply:** new plugins that need state access do the same — never `pub use` `GameState` from `lib.rs` or `plugins/mod.rs`.

- **State transitions are deferred by one frame.** `next.set(...)` queues; the new value is observable in `OnEnter(NewState)` schedules during the same frame's `StateTransition` step, or in the next frame's `Update`. **Why:** `StatesPlugin` inserts `StateTransition` after `PreUpdate`. **How to apply:** never read `State<S>` in the same `Update` system that wrote `NextState<S>` and expect the new value. Put follow-up logic in `OnEnter(...)` schedules instead.

Plan file: `project/plans/20260429-031500-bevy-0-18-1-state-machine-feature-2.md`. Research: `project/research/20260429-030000-bevy-0-18-1-state-machine-feature-2.md`.
