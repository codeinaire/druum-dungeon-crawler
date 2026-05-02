# Plan: Input System with leafwing-input-manager (Feature #5)

**Date:** 2026-05-02
**Status:** Complete
**Research:** ../research/20260501-235900-feature-5-input-system-leafwing.md
**Depends on:** 20260501-230000-feature-4-dungeon-grid-data-model.md

## Goal

Wire up `leafwing-input-manager` as Druum's canonical gameplay-input layer: pin the crate (after a verify-before-pin gate against Bevy 0.18.1), define three context-scoped action enums (`MenuAction`, `DungeonAction`, `CombatAction`), provide keyboard-only default `InputMap`s for each, register all three `InputManagerPlugin::<T>` instances inside one new `ActionsPlugin` at `src/plugins/input/mod.rs`, expose `Res<ActionState<T>>` so future Features (#7 movement, #15 combat, #25 settings UI) can poll input without scattering `KeyCode` matches through the codebase, and prove the wiring with five `KeyboardInput`-message-injection tests.

The F9 cycler in `src/plugins/state/mod.rs` is **not** refactored — it stays on `Res<ButtonInput<KeyCode>>` per research §RQ7. `DevAction` is **deferred** until a second leafwing-routed dev hotkey is needed (rationale in §Critical and §Open Questions).

## Approach

The research recommends `leafwing-input-manager` (the de-facto Bevy input crate, listed in the original 2026-03-26 dungeon-crawler research §Don't Hand-Roll line 989) as the layer for many-to-many key-to-action mapping with a future settings-UI rebinding story. Feature #5 is structurally the same shape as Feature #3: one new external crate behind a Bevy-0.18 compatibility verification fail-stop, plus one new plugin module under `src/plugins/`.

The architectural decisions made here:

1. **Plugin location: `src/plugins/input/mod.rs`** — sibling to `state/`, `dungeon/`, `loading/`, etc. Matches the established 9-flat-plugin pattern from Features #1-#4. The plugin is named `ActionsPlugin` (NOT `InputPlugin`) to avoid colliding with `bevy::input::InputPlugin` whenever both are imported via `bevy::prelude::*`.

2. **Per-context action enums (3 of them, not 4).** `MenuAction`, `DungeonAction`, `CombatAction`. Per-context wins over a mega-enum because it makes state-scoping a compile-time property: a system that reads `Res<ActionState<DungeonAction>>` cannot accidentally handle combat input. `MenuAction` is reused for Town navigation in v1 (no `TownAction` until Town gets distinct movement, which is a Feature #19+ concern). `DevAction` is **deferred** — the research case for landing it now (research §RQ8) is forward compatibility for future leafwing-routed dev hotkeys, but in v1 we have zero such hotkeys (F9 stays direct), and a single-variant placeholder enum adds 6 cfg-gating points (enum + map fn + plugin add + insert resource + tests) for zero current value. When the first real leafwing-routed dev hotkey lands, that feature introduces `DevAction` naturally.

3. **`InputMap<T>` as `Resource`** (global), not per-entity `Component`. Druum is single-player; per-entity is leafwing's 0.15+ idiom for couch co-op rebinding which we don't need. Reassess at Feature #25 if multi-player is ever scoped.

4. **Centralized plugin wiring inside `ActionsPlugin::build`.** All three `InputManagerPlugin::<T>` registrations and all three `insert_resource(default_*_input_map())` calls happen here. Consumer plugins (`DungeonPlugin`, `CombatPlugin`) only `use crate::plugins::input::DungeonAction;` to read the enum types in their systems; they do NOT register input plugins themselves.

5. **NO `.run_if(in_state(...))` on the plugin call.** Plugins are not run-conditioned in Bevy — `run_if` is on `IntoSystemConfigs`, not on `Plugin`. The roadmap's `app.add_plugins(InputManagerPlugin::<DungeonAction>::default().run_if(in_state(GameState::Dungeon)))` line does not compile in Bevy 0.18 (verified at `bevy_app-0.18.1/src/plugin.rs` — no `run_if` on the trait). State-scoping is a *consumer-side* concern: future Feature #7 systems gate themselves with `.run_if(in_state(GameState::Dungeon))` on the systems that read `Res<ActionState<DungeonAction>>`.

6. **Keyboard-only v1, default-features audit pending Step B.** No gamepad bindings, no mouse beyond what leafwing's defaults give us. After the version is resolved, the implementer reads the resolved Cargo.toml `[features]` block and decides whether to keep defaults or opt into `default-features = false, features = [...]` minimal set. Heuristic: keep defaults if they're benign (≤ keyboard + mouse); opt out if defaults pull `bevy_egui` or `asset` (RON-loadable input maps — defer to Feature #25).

7. **Tests use full `InputPlugin` + `KeyboardInput` message injection.** This is the OPPOSITE pattern from Feature #2's F9 test (`init_resource::<ButtonInput<KeyCode>>` bypass). Reason: leafwing's update system in `PreUpdate` reads `ButtonInput<KeyCode>` AFTER `keyboard_input_system` populates it from `KeyboardInput` messages. Without `InputPlugin`, leafwing has no fresh input to consume. Without message injection (i.e. directly mutating `ButtonInput<KeyCode>`), `keyboard_input_system` clears the press at frame start before leafwing reads. The safe pattern is: add `InputPlugin`, write a `KeyboardInput` message into `Messages<KeyboardInput>` (verified type at `bevy_ecs-0.18.1/src/message/messages.rs:95`), call `app.update()`, assert `ActionState<T>::just_pressed(&action)`. Detailed code template in §Step 8.

8. **Step-A-first commit ordering.** No Cargo.toml edit happens until `cargo add leafwing-input-manager --dry-run` resolves a version compatible with our pinned `=0.18.1` Bevy. This is the same fail-stop Feature #3 ran for `bevy_common_assets`/`bevy_asset_loader` and its precedent (`moonshine-save` swap, 2026-04-29) escalates to user if upstream doesn't support our Bevy minor.

## Critical

- **Step A verification gate before Cargo.toml.** Run `cargo add leafwing-input-manager --dry-run` first. Read the resolved version. Inspect the resolved crate's `bevy = "..."` requirement (via `cargo info leafwing-input-manager --version <ver>` OR by reading the resolved `~/.cargo/registry/src/.../leafwing-input-manager-<ver>/Cargo.toml` once the dry-run downloads it). **Do NOT edit `Cargo.toml` until the implementer confirms the resolved version's `bevy` requirement accepts `0.18.1`.** If the resolved version requires `bevy = "0.17"` or older, OR `bevy = "0.19"` or newer, **HALT and escalate to the user** with the same playbook as `moonshine-save` (Feature #3 Resolved §3, 2026-04-29). Options to surface: (a) wait for upstream 0.18-compat, (b) use a community fork, (c) hand-roll a minimal `Actionlike` + `InputMap` + `ActionState` (~150-300 LOC). Do **NOT** silently downgrade Bevy.

- **Pin with `=<resolved-version>`.** Druum's convention (see `Cargo.toml:10, 22-23, 27`) is exact-match `=` for every external dep. Caret `^` would let `cargo update` move us across patch boundaries we haven't tested.

- **Plugin name is `ActionsPlugin`, NOT `InputPlugin`.** Naming our plugin `InputPlugin` collides with `bevy::input::InputPlugin` in any module that imports `bevy::prelude::*`. Per-file aliasing hides intent. Consistency wins.

- **Plugin location is `src/plugins/input/mod.rs`** (sibling pattern). Add `pub mod input;` to `src/plugins/mod.rs`. Wire `ActionsPlugin` into `src/main.rs::add_plugins(...)` after `StatePlugin` (we need `States` machinery available so consumer plugins can `.run_if(in_state(...))` later) and before consuming plugins (`DungeonPlugin`, `CombatPlugin`, etc.). Place it adjacent to `LoadingPlugin` since `LoadingPlugin` depends on `StatePlugin` only and `ActionsPlugin` does too.

- **No `.run_if(in_state(...))` on `app.add_plugins(InputManagerPlugin::<T>::default())`.** This is the exact API shape the roadmap (line 314) gets wrong. Plugins are unconditioned; only systems can be conditioned. Document this in `ActionsPlugin`'s module-level doc comment so future contributors don't try to "fix" it.

- **F9 cycler stays unchanged.** Do not edit `src/plugins/state/mod.rs`. The `f9_advances_game_state` test stays as-is. The existing `init_resource::<ButtonInput<KeyCode>>()` test pattern in `gamestate_default_is_loading` and `f9_advances_game_state` does not need updating (the addition of `ActionsPlugin` to `main.rs` does NOT affect these tests because the tests build their App directly with only `StatesPlugin + StatePlugin`, never including `ActionsPlugin`). Add a note in `ActionsPlugin`'s module-level doc comment explaining why F9 carves out: "F9 is a dev-only hotkey that predates leafwing and reads `ButtonInput<KeyCode>` directly. It is intentionally not routed through `DevAction` because: (1) F9 is never user-rebindable, so leafwing's main feature is unused; (2) refactoring would require six cfg-gating points; (3) the test would have to switch to the full-`InputPlugin` + message-injection pattern. The carve-out is intentional."

- **`DevAction` is deferred to a future feature.** No `DevAction` enum, no `InputManagerPlugin::<DevAction>` registration, no `default_dev_input_map()`. Document the deferral in `ActionsPlugin`'s module-level doc comment so future contributors know where the enum will land when its first variant is needed. Rationale: research §RQ8 marked it "skeleton with placeholder", but the task spec defensibly defers — a placeholder enum has cfg-gating overhead and zero v1 callers. The first leafwing-routed dev hotkey introduces `DevAction` naturally at that time.

- **Tests use full `InputPlugin` + `Messages<KeyboardInput>` injection — NOT `init_resource::<ButtonInput<KeyCode>>`.** The Feature #2 F9 test pattern (the bypass) does not transfer. Detailed code template in §Step 8. Both `MinimalPlugins` and `InputPlugin` are required (research §RQ6 verified at `bevy_input-0.18.1/src/lib.rs:111-116`).

- **`KeyboardInput` is a `Message` in Bevy 0.18, not an `Event`** (verified at `bevy_input-0.18.1/src/keyboard.rs:98`: `#[derive(Message, ...)]`). The test injection writes to `bevy::ecs::message::Messages<KeyboardInput>` via `.write(...)` — NOT `Events<KeyboardInput>` and NOT `world.send_event(...)`. Same family-rename trap that bit `StateTransitionEvent` (Feature #2) and `AssetEvent` (Feature #3).

- **`#[cfg(feature = "dev")]` symmetric gating not needed in Feature #5.** Because `DevAction` is deferred, no enum/plugin/resource needs cfg-gating in this feature. The discipline still applies for the future feature that lands `DevAction`.

- **Rust 2024 edition transitive-dep rule.** `leafwing-input-manager` MUST be in `[dependencies]` because Druum's source code writes `use leafwing_input_manager::...`. This is what `cargo add` does. No surprises here. Same rule that forced `serde` and `ron` to be explicit in Feature #3.

- **All 6 verification commands must pass with ZERO warnings:** `cargo check`, `cargo check --features dev`, `cargo clippy --all-targets -- -D warnings`, `cargo clippy --all-targets --features dev -- -D warnings`, `cargo test`, `cargo test --features dev`. `Cargo.lock` will gain only the `leafwing-input-manager` entry plus its direct deps; no unrelated transitive bumps to other crates we already have.

- **Defensive `&T` reference shape on `ActionState::just_pressed` calls.** Newer leafwing versions take `&T` (e.g. `actions.just_pressed(&DungeonAction::MoveForward)`); older versions took `T` by value. Write the test code with `&` defensively. If compile fails, drop the `&`. Either way, compile error catches it immediately.

## Steps

### Step 1: Run the Step A verification gate (NO Cargo.toml edit yet)

Verify a published `leafwing-input-manager` version is compatible with our pinned `bevy = "=0.18.1"` BEFORE editing any project file. This is a fail-stop gate; the rest of the plan is blocked until this passes.

- [x] From the project root `/Users/nousunio/Repos/Learnings/claude-code/druum`, run:
  ```bash
  cargo add leafwing-input-manager --dry-run 2>&1 | tee /tmp/leafwing-resolve.txt
  ```
  Read the resolved version (the line will look like `Adding leafwing-input-manager v0.X.Y to dependencies`).
- [x] Inspect the resolved crate's bevy requirement. Two acceptable techniques (use whichever works first):
  - **Technique A (preferred):** `cargo info leafwing-input-manager --version <RESOLVED-VERSION> 2>&1 | grep -E "^bevy"` — reads from crates.io metadata.
  - **Technique B (fallback if `cargo info` fails or doesn't show deps):** Run `cargo add leafwing-input-manager@=<RESOLVED-VERSION>` to actually add (this would download into the registry cache), then `cat ~/.cargo/registry/src/index.crates.io-*/leafwing-input-manager-<RESOLVED-VERSION>/Cargo.toml | grep -A 1 "^bevy"`. **If you take Technique B, immediately revert the Cargo.toml change with `git checkout Cargo.toml Cargo.lock` before proceeding** — Step 2 must lock with `=` after this verification.
- [x] **Decision tree:**
  - If the resolved version's bevy req is `0.18`, `^0.18`, `>=0.18, <0.19`, or `=0.18.1` → ACCEPTABLE. Record the resolved version (e.g. `0.18.0` or `0.18.1`) and proceed to Step 2.
  - If the resolved version's bevy req is `0.17` or older → HALT. Escalate to user. Surface options: (a) wait for upstream 0.18-compat, (b) use a community fork, (c) hand-roll. Do NOT downgrade Bevy.
  - If the resolved version's bevy req is `0.19` or newer → HALT. Escalate to user. Same options as above.
- [x] **Capture the resolved version number** for use in Step 2. Note in `Implementation Discoveries` what version resolved and what Bevy req it had.

**Done state:** A confirmed `leafwing-input-manager` version is recorded that supports `bevy = "0.18.1"`. No project files have been edited yet. If escalation was needed, the user has been asked.

### Step 2: Run the Step B feature-flag audit (NO Cargo.toml edit yet)

After Step A resolves a version, inspect the crate's feature flags to determine the minimal feature set Druum needs. Heuristic: keep defaults if they're benign; opt out if defaults pull heavy crates (`bevy_egui`, `asset`).

- [x] If Step 1 used Technique A only (no actual download), the resolved crate is NOT yet on disk. Run `cargo add leafwing-input-manager@=<RESOLVED-VERSION>` to download it (this DOES edit Cargo.toml temporarily — that's fine for inspection; we'll revert and re-add with the right feature set in Step 3).
- [x] Open the resolved Cargo.toml: `cat ~/.cargo/registry/src/index.crates.io-*/leafwing-input-manager-<RESOLVED-VERSION>/Cargo.toml`. Find the `[features]` block.
- [x] **Audit the `default = [...]` list:**
  - If `default` contains `egui`, `asset`, or anything that pulls `bevy_egui` → opt out via `default-features = false` and pick a minimal set.
  - If `default` contains only `keyboard`, `mouse` (or has no defaults) → keep defaults.
- [x] **Verify against the typical-feature list (training-data MEDIUM):** `egui` (binding-display widgets), `asset` (RON-loadable InputMap), `serde` (Serialize/Deserialize for InputMap), `mouse`, `gamepad`, `keyboard`. Druum v1 needs only keyboard. Mouse defaults are fine (small surface area). `egui`/`asset`/`serde` are deferred to Feature #25.
- [x] **Document the feature decision in Implementation Discoveries.** One of these two outcomes:
  - "Defaults are minimal (keyboard + mouse) — keeping defaults. Cargo.toml line: `leafwing-input-manager = "=<ver>"`."
  - "Defaults include `<heavy-feature>` — opting out. Cargo.toml line: `leafwing-input-manager = { version = "=<ver>", default-features = false, features = ["keyboard", "mouse"] }` (adjust feature names to whatever the resolved crate uses)."
- [x] Revert the temporary Cargo.toml/Cargo.lock from Step 1's Technique B if applicable: `git checkout Cargo.toml Cargo.lock`. (If you used Technique A only, nothing to revert.)

**Done state:** The exact `[dependencies]` line for `leafwing-input-manager` has been chosen (with or without `default-features = false`). No project files have been edited yet (any temp edits from Step 1 reverted).

### Step 3: Run the Step C API verification grep (NO Cargo.toml edit yet)

Verify the resolved crate's API surface BEFORE writing any source code that uses it. The implementation depends on three identifiers: the `Actionlike` derive macro, the `InputMap` builder shape, and the `ActionState` query path.

- [x] If the resolved crate is on disk (it should be after Step 2's inspection), skip ahead. Otherwise, run `cargo add leafwing-input-manager@=<ver>` once more (we WILL keep this edit in Step 4 — at this point the gate has passed).
- [x] Run the API verification greps:
  ```bash
  REG=~/.cargo/registry/src/index.crates.io-*/leafwing-input-manager-<RESOLVED-VERSION>/src
  grep -rn "trait MockInput" $REG
  grep -rn "fn just_pressed" $REG
  grep -rn "fn pressed\b" $REG
  grep -rn "pub fn insert" $REG | grep -i "input_map"
  grep -rn "impl<A.*> Plugin" $REG
  grep -rn "pub trait Actionlike" $REG
  ```
- [x] **Record what the greps find.** Specifically:
  - **Does `trait MockInput` exist?** If yes, note the path. If no, the test pattern uses raw `Messages<KeyboardInput>` injection (the path documented in §Step 8 below).
  - **`ActionState::just_pressed` signature:** does it take `T` by value or `&T` by reference? Note the answer.
  - **`InputMap::insert` signature:** does it take `(action, input)` and return `Self` (chainable) or `&mut Self` (builder pattern requiring a `.build()`)? Note the answer.
  - **`InputManagerPlugin` shape:** Is it `InputManagerPlugin::<T>::default()`, or some other constructor? Note the answer.
  - **`Actionlike` trait derives:** What bounds does the trait require? Does the macro expand to `Reflect`-aware code? Note the bound list (`PartialEq + Eq + Hash + Clone + Copy + Debug + Reflect` is the expected superset; some leafwing versions require fewer).
- [x] **If the greps reveal a structurally different API** (e.g. `Actionlike` has been replaced with `InputAction`, or `InputMap` has been replaced with `Bindings`) → adjust the source-code shape in Steps 5-7 accordingly. The architecture (one plugin, three enums, three resources, three plugin registrations) is robust to API-shape differences. The tests in Step 8 are the one place where the exact `KeyboardInput` Bevy primitive is load-bearing — that primitive is verified independent of leafwing.
- [x] **Document the API shape in Implementation Discoveries.** This becomes the source of truth for the rest of the plan's code samples, which are written defensively but may need adjustment.

**Done state:** The resolved leafwing API surface (derive bounds, `InputMap::insert` chainability, `ActionState::just_pressed` arg shape, `MockInput` existence) is documented. The implementer has a concrete answer for "do I write `actions.just_pressed(&Action::X)` or `actions.just_pressed(Action::X)`" before any code is written.

### Step 4: Edit Cargo.toml — add `leafwing-input-manager`

Now the gate has passed. Lock the dep with `=` per project convention.

- [x] Open `/Users/nousunio/Repos/Learnings/claude-code/druum/Cargo.toml`.
- [x] Add a single line to the `[dependencies]` block, immediately after the existing `bevy_asset_loader` entry (Cargo.toml line 23) so the order is: bevy → bevy_common_assets → bevy_asset_loader → leafwing-input-manager → serde → ron. The exact line shape is the one chosen in Step 2:
  - **If keeping defaults:** `leafwing-input-manager = "=<RESOLVED-VERSION>"`
  - **If opting out:** `leafwing-input-manager = { version = "=<RESOLVED-VERSION>", default-features = false, features = ["keyboard", "mouse"] }` (adjust feature names to the actual ones the resolved crate exposes).
- [x] Run `cargo check`. The new dep resolves; `Cargo.lock` is updated. Read the diff: `git diff Cargo.lock | head -60` should show only `leafwing-input-manager` and any direct deps it pulls (e.g. `petitset`, `serde`, plus their tree). **Verify NO unrelated transitive bumps to bevy, bevy_common_assets, bevy_asset_loader, serde, ron.** If `git diff Cargo.lock` shows a bevy minor or other unrelated bump, STOP — that means leafwing's bevy req was looser than expected and Cargo unified upward; revert and re-investigate at Step 1.
- [x] Run `cargo check --features dev` — must succeed with no warnings.

**Done state:** `leafwing-input-manager` is in `Cargo.toml` with the right version pin and feature set. `Cargo.lock` is updated and the diff contains only the new dep tree. `cargo check` passes under both feature sets. No source code changes yet.

### Step 5: Create `src/plugins/input/mod.rs` — define the three action enums and `ActionsPlugin` skeleton

Create the new module with the three enums, an empty `ActionsPlugin` Plugin impl, and module-level documentation. Default `InputMap` constructors are stubbed in this step and filled in Step 6.

- [ ] Create the directory: `mkdir -p /Users/nousunio/Repos/Learnings/claude-code/druum/src/plugins/input` (the `mkdir` is for the directory; the `mod.rs` file is created by the `Write` step below).
- [ ] Create `src/plugins/input/mod.rs` with the following structure (adjust the `Actionlike` derive bound list per Step 3's API verification if needed):

  ```rust
  //! Input system: gameplay actions routed through `leafwing-input-manager`.
  //!
  //! ## What this module owns
  //!
  //! - Three `Actionlike` enums — `MenuAction`, `DungeonAction`, `CombatAction` —
  //!   one per game-state context. Per-context enums make state-scoping a
  //!   compile-time property: a system reading `Res<ActionState<DungeonAction>>`
  //!   in combat code is a compile error, not a runtime bug.
  //! - Default keyboard `InputMap<T>` resources for each enum.
  //! - The `ActionsPlugin` Plugin impl that registers all three
  //!   `InputManagerPlugin::<T>::default()` instances and inserts the default maps.
  //!
  //! ## What this module does NOT own
  //!
  //! - The F9 dev cycler in `src/plugins/state/mod.rs:71-89`. F9 stays on
  //!   `Res<ButtonInput<KeyCode>>` directly. Reasons: (1) F9 is dev-only and never
  //!   user-rebindable, so leafwing's main feature (rebinding) is unused; (2)
  //!   refactoring would require six `#[cfg(feature = "dev")]` gating points (enum,
  //!   InputMap fn, plugin add, insert_resource, system def, add_systems); (3)
  //!   the existing F9 test uses the `init_resource::<ButtonInput<KeyCode>>()`
  //!   bypass pattern, which would have to switch to a full `InputPlugin` +
  //!   `KeyboardInput` message injection. The carve-out is intentional.
  //!
  //! - A `DevAction` enum. Deferred until the first leafwing-routed dev hotkey
  //!   beyond F9 lands. A placeholder enum with one variant adds cfg-gating
  //!   surface for zero current callers.
  //!
  //! - State-scoping via `.run_if(in_state(...))`. That happens inside *consuming*
  //!   plugin builds, on the gameplay systems that read `Res<ActionState<T>>`.
  //!   The `InputManagerPlugin::<T>::default()` registrations themselves run
  //!   unconditionally — Bevy's `Plugin` trait has no `run_if` (verified at
  //!   `bevy_app-0.18.1/src/plugin.rs`).
  //!
  //! ## Consumer pattern (Feature #7+)
  //!
  //! ```ignore
  //! use crate::plugins::input::DungeonAction;
  //! use leafwing_input_manager::prelude::*;
  //!
  //! fn handle_dungeon_movement(actions: Res<ActionState<DungeonAction>>) {
  //!     if actions.just_pressed(&DungeonAction::MoveForward) { /* ... */ }
  //! }
  //!
  //! // In DungeonPlugin::build:
  //! app.add_systems(Update, handle_dungeon_movement.run_if(in_state(GameState::Dungeon)));
  //! ```

  use bevy::prelude::*;
  use leafwing_input_manager::prelude::*;

  /// Menu-style navigation actions. Used in TitleScreen, Town, GameOver,
  /// dungeon sub-state menus (Inventory/Map/Paused/EventDialog), and combat
  /// "press any key to continue" between phases. Town reuses this enum in v1;
  /// `TownAction` is deferred until Town gets distinct movement (Feature #19+).
  #[derive(Actionlike, PartialEq, Eq, Hash, Clone, Copy, Debug, Reflect)]
  pub enum MenuAction {
      Up,
      Down,
      Left,
      Right,
      Confirm,
      Cancel,
      Pause,
  }

  /// First-person grid movement and dungeon UI hotkeys. Used in
  /// `GameState::Dungeon + DungeonSubState::Exploring`. Modern Wizardry/Etrian
  /// convention: WASD or arrows for movement, Q/E for turning, M for map,
  /// Tab for inventory, F for interact, Escape for pause.
  #[derive(Actionlike, PartialEq, Eq, Hash, Clone, Copy, Debug, Reflect)]
  pub enum DungeonAction {
      MoveForward,
      MoveBackward,
      StrafeLeft,
      StrafeRight,
      TurnLeft,
      TurnRight,
      Interact,
      OpenMap,
      OpenInventory,
      Pause,
  }

  /// Turn-based combat menu navigation. Used in
  /// `GameState::Combat + CombatPhase::PlayerInput`. The action enum is
  /// defined here; the systems that consume it land in Feature #15.
  #[derive(Actionlike, PartialEq, Eq, Hash, Clone, Copy, Debug, Reflect)]
  pub enum CombatAction {
      Up,
      Down,
      Left,
      Right,
      Confirm,
      Cancel,
  }

  /// Plugin that owns all gameplay input registration.
  pub struct ActionsPlugin;

  impl Plugin for ActionsPlugin {
      fn build(&self, app: &mut App) {
          app.add_plugins((
              InputManagerPlugin::<MenuAction>::default(),
              InputManagerPlugin::<DungeonAction>::default(),
              InputManagerPlugin::<CombatAction>::default(),
          ))
          .insert_resource(default_menu_input_map())
          .insert_resource(default_dungeon_input_map())
          .insert_resource(default_combat_input_map());
      }
  }

  // Default InputMap constructors are filled in Step 6.
  fn default_menu_input_map() -> InputMap<MenuAction> {
      InputMap::default()
  }

  fn default_dungeon_input_map() -> InputMap<DungeonAction> {
      InputMap::default()
  }

  fn default_combat_input_map() -> InputMap<CombatAction> {
      InputMap::default()
  }
  ```

- [x] **Adjust derives if Step 3's grep revealed a different bound list.** The `Actionlike, PartialEq, Eq, Hash, Clone, Copy, Debug, Reflect` line is the defensive superset; older leafwing versions may not require `Reflect` (always include it for forward-compat — it's free per `reference_bevy_reflect_018_derive.md` for unit enums) or may auto-include some bounds via the macro. Compile error catches drift either way.
- [x] Open `src/plugins/mod.rs` and add `pub mod input;` to the alphabetical list. The full file should now read:
  ```rust
  //! Top-level plugin tree. Each submodule owns one Bevy `Plugin`.

  pub mod audio;
  pub mod combat;
  pub mod dungeon;
  pub mod input;
  pub mod loading;
  pub mod party;
  pub mod save;
  pub mod state;
  pub mod town;
  pub mod ui;
  ```
- [x] Open `src/main.rs` and add `input::ActionsPlugin,` to the `add_plugins(...)` tuple. The new plugin goes after `StatePlugin` (we depend on `States` machinery being registered) and adjacent to `LoadingPlugin` (which is the other plugin that depends only on state). Insert order:
  ```rust
  StatePlugin,
  ActionsPlugin,    // NEW — sits next to LoadingPlugin in dependency order
  LoadingPlugin,
  DungeonPlugin,
  // ... rest unchanged
  ```
  Also update the `use druum::plugins::{...};` import block to include `input::ActionsPlugin,` (alphabetized).
- [x] Run `cargo check`. Must succeed with zero warnings. The empty `InputMap::default()` returns are valid — `InputMap<T>` can be empty (no bindings = no actions ever fire), which is fine for this skeleton step.
- [x] Run `cargo check --features dev`. Must succeed with zero warnings.

**Done state:** `src/plugins/input/mod.rs` exists with three action enums, an `ActionsPlugin` Plugin impl, and stubbed default-map constructors that return empty `InputMap`s. `src/plugins/mod.rs` declares `pub mod input;`. `src/main.rs` registers `ActionsPlugin` in the `add_plugins(...)` tuple. Both `cargo check` and `cargo check --features dev` succeed with zero warnings.

### Step 6: Fill the three `default_*_input_map()` bodies with keyboard bindings

Replace the empty `InputMap::default()` returns with the actual key-to-action bindings per the research §RQ9 keymap matrix and the task spec's resolved decisions (arrows strafe; `Interact = F`).

- [x] Replace the body of `default_menu_input_map()`:
  ```rust
  fn default_menu_input_map() -> InputMap<MenuAction> {
      use MenuAction::*;
      // The exact builder shape (chainable .insert vs builder + .build()) was
      // verified in Step 3. Adjust if Step 3 found a different shape.
      InputMap::default()
          .with(Up,      KeyCode::ArrowUp)
          .with(Up,      KeyCode::KeyW)
          .with(Down,    KeyCode::ArrowDown)
          .with(Down,    KeyCode::KeyS)
          .with(Left,    KeyCode::ArrowLeft)
          .with(Left,    KeyCode::KeyA)
          .with(Right,   KeyCode::ArrowRight)
          .with(Right,   KeyCode::KeyD)
          .with(Confirm, KeyCode::Enter)
          .with(Confirm, KeyCode::Space)
          .with(Cancel,  KeyCode::Escape)
          .with(Pause,   KeyCode::Escape)
  }
  ```
  **NOTE:** the method name `.with(action, input)` is the leafwing 0.13+ idiom; some versions use `.insert(action, input)` (mutable, requires `let mut` and a final return). Per Step 3's verification, use whichever the resolved crate exposes. If neither works, check for a macro form like `InputMap::new([(action, input), ...])`. **Compile error will catch the mismatch immediately.**
- [x] Replace the body of `default_dungeon_input_map()`. Bind both WASD and arrow keys to movement (many-to-many is leafwing's selling point); arrow keys map to STRAFE (modern Wizardry/Etrian convention per task spec — research §Open Question #9 resolved); `Interact = F` to avoid the TurnRight=E conflict (task spec's resolved choice):
  ```rust
  fn default_dungeon_input_map() -> InputMap<DungeonAction> {
      use DungeonAction::*;
      InputMap::default()
          // Movement (WASD + arrows; arrows STRAFE per modern convention)
          .with(MoveForward,   KeyCode::KeyW)
          .with(MoveForward,   KeyCode::ArrowUp)
          .with(MoveBackward,  KeyCode::KeyS)
          .with(MoveBackward,  KeyCode::ArrowDown)
          .with(StrafeLeft,    KeyCode::KeyA)
          .with(StrafeLeft,    KeyCode::ArrowLeft)
          .with(StrafeRight,   KeyCode::KeyD)
          .with(StrafeRight,   KeyCode::ArrowRight)
          // Turning (Q/E only — no arrow alternates to avoid overloading arrows)
          .with(TurnLeft,      KeyCode::KeyQ)
          .with(TurnRight,     KeyCode::KeyE)
          // Interactions and UI hotkeys
          .with(Interact,      KeyCode::KeyF) // F (NOT Space, NOT E) — avoids TurnRight=E conflict
          .with(OpenMap,       KeyCode::KeyM)
          .with(OpenInventory, KeyCode::Tab)
          .with(Pause,         KeyCode::Escape)
  }
  ```
- [x] Replace the body of `default_combat_input_map()` (same shape as MenuAction, no Pause variant — combat doesn't pause):
  ```rust
  fn default_combat_input_map() -> InputMap<CombatAction> {
      use CombatAction::*;
      InputMap::default()
          .with(Up,      KeyCode::ArrowUp)
          .with(Up,      KeyCode::KeyW)
          .with(Down,    KeyCode::ArrowDown)
          .with(Down,    KeyCode::KeyS)
          .with(Left,    KeyCode::ArrowLeft)
          .with(Left,    KeyCode::KeyA)
          .with(Right,   KeyCode::ArrowRight)
          .with(Right,   KeyCode::KeyD)
          .with(Confirm, KeyCode::Enter)
          .with(Confirm, KeyCode::Space)
          .with(Cancel,  KeyCode::Escape)
  }
  ```
- [x] Run `cargo check`. Must succeed with zero warnings. **If `.with(...)` does not exist on `InputMap<T>`, switch to `.insert(...)` (mutable builder) — pattern:**
  ```rust
  let mut map = InputMap::default();
  map.insert(MenuAction::Up, KeyCode::ArrowUp);
  map.insert(MenuAction::Up, KeyCode::KeyW);
  // ... etc
  map
  ```
  Document the shape used in Implementation Discoveries.
- [x] Run `cargo check --features dev`. Must succeed.
- [x] Run `cargo clippy --all-targets -- -D warnings`. Must succeed. Likely lints to watch: `clippy::similar_names` (e.g. `Up` and `Up` from different enums in the same scope is fine because of `use ... ::*` — should not trigger because each fn body has its own scope). If any lint fires, document and either fix or `#[allow(...)]` with a one-line rationale.
- [x] Run `cargo clippy --all-targets --features dev -- -D warnings`. Must succeed.

**Done state:** All three `default_*_input_map()` functions return populated `InputMap<T>` with keyboard bindings per the research RQ9 matrix and task-spec-resolved decisions (arrows strafe; `Interact = F`). All four check/clippy passes succeed under both feature sets.

### Step 7: Add the `actions_plugin_registers_all_inputmaps` smoke test

The first test verifies the plugin builds without panicking and that all three `InputMap<T>` resources exist. This is a fast structural test that does NOT touch keyboard injection — it only exercises the plugin wiring.

- [x] Add a `#[cfg(test)] mod tests` block at the bottom of `src/plugins/input/mod.rs`:
  ```rust
  #[cfg(test)]
  mod tests {
      use super::*;
      use bevy::input::InputPlugin;
      use bevy::state::app::StatesPlugin;

      /// `ActionsPlugin::build` registers all three InputManagerPlugin instances
      /// and inserts all three default InputMap resources. Smoke test — no input
      /// injection.
      #[test]
      fn actions_plugin_registers_all_inputmaps() {
          let mut app = App::new();
          app.add_plugins((
              MinimalPlugins,
              StatesPlugin, // ActionsPlugin doesn't use States directly, but
                            // future cross-plugin tests will — keep the test
                            // setup symmetrical with §Step 8 tests.
              InputPlugin,
              ActionsPlugin,
          ));
          app.update();

          // All three InputMap resources must be present.
          assert!(app.world().contains_resource::<InputMap<MenuAction>>(),
              "InputMap<MenuAction> should be registered by ActionsPlugin");
          assert!(app.world().contains_resource::<InputMap<DungeonAction>>(),
              "InputMap<DungeonAction> should be registered by ActionsPlugin");
          assert!(app.world().contains_resource::<InputMap<CombatAction>>(),
              "InputMap<CombatAction> should be registered by ActionsPlugin");

          // All three ActionState resources must be present (registered by
          // InputManagerPlugin).
          assert!(app.world().contains_resource::<ActionState<MenuAction>>(),
              "ActionState<MenuAction> should be registered by InputManagerPlugin::<MenuAction>");
          assert!(app.world().contains_resource::<ActionState<DungeonAction>>(),
              "ActionState<DungeonAction> should be registered by InputManagerPlugin::<DungeonAction>");
          assert!(app.world().contains_resource::<ActionState<CombatAction>>(),
              "ActionState<CombatAction> should be registered by InputManagerPlugin::<CombatAction>");
      }
  }
  ```
- [x] **Note about the `Resource` trait check on `InputMap<T>`:** Some leafwing versions register `InputMap<T>` per-entity (Component shape) rather than as a Resource. If `app.world().contains_resource::<InputMap<MenuAction>>()` returns false despite our `.insert_resource(default_menu_input_map())` call in Step 5, that means the resolved leafwing version expects `InputMap<T>` to be a Component, not a Resource — STOP and revisit the plan (the task spec and research §RQ3 explicitly chose the Resource shape; if the resolved version does not support that shape, escalate to the user with the trade-off summary). The Resource shape is the standard 0.13-0.17 pattern and HIGH-likelihood the 0.18-compat release retains it; this assertion is a guard.
- [x] Run `cargo test --package druum actions_plugin_registers_all_inputmaps`. Must pass.
- [x] Run `cargo test --features dev --package druum actions_plugin_registers_all_inputmaps`. Must pass.

**Done state:** The smoke test passes under both feature sets, proving `ActionsPlugin` registers all three `InputManagerPlugin::<T>` instances and all three `InputMap<T>` resources, and that `ActionState<T>` resources exist after `InputManagerPlugin` registration.

### Step 8: Add the four `KeyboardInput`-message-injection tests

These tests prove end-to-end key-to-action mapping via the verified Bevy 0.18 input chain: `KeyboardInput` message → `keyboard_input_system` populates `ButtonInput<KeyCode>` → leafwing's update reads it → `ActionState<T>::just_pressed(&action)` returns true.

The shared test scaffolding is identical across all four tests; we factor it into a helper `fn make_test_app() -> App` so each test body is short. The `KeyboardInput` field layout is verified at `bevy_input-0.18.1/src/keyboard.rs:109-139` (six fields: `key_code`, `logical_key`, `state`, `text`, `repeat`, `window`).

- [x] Extend the `#[cfg(test)] mod tests` block in `src/plugins/input/mod.rs` with the helper and the four tests:
  ```rust
  use bevy::ecs::message::Messages;
  use bevy::input::keyboard::{Key, KeyboardInput};
  use bevy::input::ButtonState;

  /// Build a minimal test app with the full input chain: MinimalPlugins,
  /// StatesPlugin, InputPlugin, ActionsPlugin. This is the OPPOSITE pattern
  /// from Feature #2's F9 test (which uses init_resource::<ButtonInput<KeyCode>>
  /// to bypass keyboard_input_system). Here we need the full chain because
  /// leafwing's update system in PreUpdate reads ButtonInput<KeyCode> AFTER
  /// keyboard_input_system populates it from KeyboardInput messages.
  fn make_test_app() -> App {
      let mut app = App::new();
      app.add_plugins((
          MinimalPlugins,
          StatesPlugin,
          InputPlugin,
          ActionsPlugin,
      ));
      app.update(); // initialise resources
      app
  }

  /// Inject a key press by writing a KeyboardInput message at the layer
  /// keyboard_input_system reads from. This flows through the same code path
  /// a real OS press would use. Field layout verified at
  /// bevy_input-0.18.1/src/keyboard.rs:109-139.
  fn inject_key_press(app: &mut App, key: KeyCode, character: &str) {
      app.world_mut()
          .resource_mut::<Messages<KeyboardInput>>()
          .write(KeyboardInput {
              key_code: key,
              logical_key: Key::Character(character.into()),
              state: ButtonState::Pressed,
              text: None,
              repeat: false,
              window: bevy::ecs::entity::Entity::PLACEHOLDER,
          });
  }

  /// Pressing W triggers DungeonAction::MoveForward via leafwing's mapping.
  #[test]
  fn dungeon_w_press_triggers_move_forward() {
      let mut app = make_test_app();
      inject_key_press(&mut app, KeyCode::KeyW, "w");
      app.update(); // keyboard_input_system reads message → ButtonInput populated
                    // → leafwing maps → ActionState<DungeonAction> updated.

      let action_state = app.world().resource::<ActionState<DungeonAction>>();
      assert!(action_state.just_pressed(&DungeonAction::MoveForward),
          "Pressing W should trigger DungeonAction::MoveForward");
  }

  /// Verify the many-to-many binding: ArrowUp also triggers MoveForward.
  #[test]
  fn dungeon_arrow_up_also_triggers_move_forward() {
      let mut app = make_test_app();
      inject_key_press(&mut app, KeyCode::ArrowUp, "");
      app.update();

      let action_state = app.world().resource::<ActionState<DungeonAction>>();
      assert!(action_state.just_pressed(&DungeonAction::MoveForward),
          "Pressing ArrowUp should also trigger DungeonAction::MoveForward (many-to-many)");
  }

  /// Pressing Escape triggers MenuAction::Cancel.
  #[test]
  fn menu_escape_triggers_cancel() {
      let mut app = make_test_app();
      inject_key_press(&mut app, KeyCode::Escape, "");
      app.update();

      let action_state = app.world().resource::<ActionState<MenuAction>>();
      assert!(action_state.just_pressed(&MenuAction::Cancel),
          "Pressing Escape should trigger MenuAction::Cancel");
      // Note: Escape is bound to BOTH Cancel and Pause in MenuAction. Both should
      // fire on the same press — leafwing supports many-to-many in the action
      // direction too.
      assert!(action_state.just_pressed(&MenuAction::Pause),
          "Pressing Escape should also trigger MenuAction::Pause (Cancel+Pause both bound to Escape)");
  }

  /// Pressing Enter triggers CombatAction::Confirm.
  #[test]
  fn combat_enter_triggers_confirm() {
      let mut app = make_test_app();
      inject_key_press(&mut app, KeyCode::Enter, "");
      app.update();

      let action_state = app.world().resource::<ActionState<CombatAction>>();
      assert!(action_state.just_pressed(&CombatAction::Confirm),
          "Pressing Enter should trigger CombatAction::Confirm");
  }
  ```
- [x] **Defensive note on `just_pressed(&Action)` vs `just_pressed(Action)`:** if Step 3's grep showed leafwing takes the action by value (no `&`), drop the `&` from each test's assertion. Compile error catches drift either way.
- [x] **Defensive note on `Messages<T>` import path:** the verified location is `bevy::ecs::message::Messages` (per `bevy_ecs-0.18.1/src/message/messages.rs:95`). If the import errors, try `bevy::ecs::message::Messages` (full path), then `bevy::message::Messages`. Compile error catches drift.
- [x] **Defensive note on `Key::Character("w".into())`:** the type for `logical_key` is `Key` from `bevy::input::keyboard`; `Character` takes a `SmolStr` per `bevy_input-0.18.1/src/keyboard.rs`. `"w".into()` should work via `SmolStr: From<&str>`. If it doesn't, use `Key::Character(smol_str::SmolStr::new("w"))` (and add `smol_str` to the test scope's imports — `bevy_input` already pulls it transitively, but the Rust 2024 edition transitive-dep rule means we may need to add `smol_str` to `[dev-dependencies]`).
- [x] **If the `Messages<KeyboardInput>` injection pattern fails to trigger leafwing's mapping** (the assertion fails at runtime, even though `cargo check` passes), fall back to leafwing's `MockInput` API if Step 3 found it exists:
  ```rust
  use leafwing_input_manager::input_mocking::MockInput;
  app.send_input(KeyCode::KeyW);
  app.update();
  ```
  The `MockInput` API wraps the same primitive but may handle window-entity / repeat-flag edge cases that the manual injection misses. Document in Implementation Discoveries which path was used.
- [x] Run `cargo test --package druum --lib plugins::input::tests`. All five tests (smoke + four message-injection) must pass.
- [x] Run `cargo test --features dev --package druum --lib plugins::input::tests`. All five must pass.

**Done state:** Five tests in `src/plugins/input/mod.rs::tests` cover (1) plugin smoke (Step 7), (2) DungeonAction W → MoveForward, (3) DungeonAction ArrowUp → MoveForward (many-to-many), (4) MenuAction Escape → Cancel + Pause (many-to-many in action direction), (5) CombatAction Enter → Confirm. All pass under both feature sets.

### Step 9: Final verification matrix

Run the full project verification suite to confirm nothing else broke (especially the F9 test, which we explicitly did not touch).

- [x] `cargo check` — must succeed with zero warnings.
- [x] `cargo check --features dev` — must succeed with zero warnings.
- [x] `cargo clippy --all-targets -- -D warnings` — must succeed.
- [x] `cargo clippy --all-targets --features dev -- -D warnings` — must succeed.
- [x] `cargo test` — must pass all tests including the existing `gamestate_default_is_loading` (Feature #2) and the dungeon/data tests from Feature #4.
- [x] `cargo test --features dev` — must pass all tests including the existing `f9_advances_game_state` (Feature #2 dev-gated).
- [x] **Final Cargo.lock diff scope check.** Run `git diff Cargo.lock`. Confirm:
  - `leafwing-input-manager` entry added with the resolved version.
  - Direct deps of leafwing added (likely: `petitset`, possibly `serde` updates if leafwing requires a newer minor — should NOT be the case since serde 1 is decade-stable).
  - **NO unrelated bumps to `bevy`, `bevy_common_assets`, `bevy_asset_loader`, `serde`, `ron`.** If any of these moved, STOP and investigate — leafwing may have a tighter Bevy spec than expected and Cargo unified upward.
- [x] **Final F9 test verification.** The F9 test did not change but its behavior must still be correct. Run `cargo test --features dev f9_advances_game_state` specifically. The test should pass with the same `init_resource::<ButtonInput<KeyCode>>()` bypass pattern it used pre-Feature-#5 (because the test app explicitly uses `StatesPlugin + StatePlugin` and does NOT include `ActionsPlugin`).

**Done state:** All 6 verification commands pass with zero warnings. The F9 test (`f9_advances_game_state`) still passes unchanged under `--features dev`. `Cargo.lock` diff contains only `leafwing-input-manager` and its direct deps with no unrelated transitive bumps.

## Security

**Known vulnerabilities:** None known to research as of 2026-05-01 (research §Security — `leafwing-input-manager` and `bevy_input` 0.18.1 both clean per training-data MEDIUM). After Step 4, run `cargo audit` once and check for any new advisories on `leafwing-input-manager` between research date and implementation date. If `cargo audit` flags anything, treat per advisory severity (HALT for HIGH/CRITICAL; document and proceed for LOW).

**Architectural risks:** None new in Feature #5 — input mapping is a local-machine concern with no network or cross-trust-boundary surface area. The future settings UI (Feature #25) will introduce a file-load surface for user-rebinding RON files; that risk is owned by Feature #25, not this one. Trust boundary for Feature #5: **none** — all bindings are code-defined.

**Defensive notes for forward-compat:** When Feature #25 lands serde-deserialized `InputMap<T>` from RON, the implementer must add a validator that rejects unknown action variants and unknown KeyCode names — research §Security flagged this. Don't load raw user-supplied RON without validation.

## Open Questions

(Two genuine residual decisions surfaced for user awareness; both have defensible defaults baked into the plan above. The user can override by responding to the orchestrator before plan dispatch.)

1. **`DevAction` lands in v1 or defers? — DEFERRED in this plan.** Rationale: a placeholder `DevAction { DebugDummy }` enum adds six cfg-gating points (enum, default-map fn, plugin add, insert_resource, future system def, future add_systems) for zero v1 callers. The first leafwing-routed dev hotkey beyond F9 introduces `DevAction` naturally at that time. Research §RQ8 marked it "skeleton with placeholder" but the task spec explicitly listed deferral as a defensible option ("the plan may DEFER its introduction"). **If user wants `DevAction` landed now**, the plan adds: (a) the enum with one placeholder variant, (b) `default_dev_input_map()` returning empty `InputMap<DevAction>`, (c) `#[cfg(feature = "dev")]` gates on the enum, plugin add, and insert_resource lines (asymmetric gating fails clippy per `feedback_dev_feature_pattern`), (d) one extra test verifying the plugin registers under `--features dev` and is absent without it. Estimated incremental LOC: +30-40 lines + cfg-gating discipline. Low value, manageable cost.

2. **`Interact` keybinding (F vs Space)? — F in this plan.** Rationale: `Interact = E` would conflict with `TurnRight = E`; `Interact = Space` overloads Space (already bound to Confirm in MenuAction/CombatAction — leafwing's many-to-many tolerates this technically, but cross-context muscle-memory bleed-through is real). `F` is the modern dungeon-crawler "use" key (Skyrim, Dishonored, etc.) and avoids both conflicts. **If user prefers Space**, change one line in `default_dungeon_input_map()`: `Interact => KeyCode::Space`. The MenuAction/CombatAction Confirm bindings stay; the Dungeon-vs-Menu state-scope means the user never has both contexts active simultaneously, so the Space overload only matters for the dungeon→pause→resume flow (where Pause is bound to Escape, not Space, so still no real conflict). The trade-off is purely aesthetic; F is the safer default.

(All other research §Open Questions resolved during planning: A1-A4 deferred to implementer's Step 1-3 verification gates; B5-B7 resolved per task spec; C8 = keep F9 direct per task spec; C9 = arrows strafe per task spec; C10 = `MenuAction` reused for Town per task spec; C11 = settings UI / gamepad / HUD all out of scope.)

## Implementation Discoveries

### Step 1: Resolved version and Bevy compatibility

- `cargo add leafwing-input-manager --dry-run` resolved **v0.20.0** (latest as of 2026-05-01).
- The crate's `[dependencies.bevy]` requires `version = "0.18.0-rc.2"`, which uses Cargo's implicit `^` prefix: `>=0.18.0-rc.2, <0.19.0`. Our pinned `bevy = "=0.18.1"` satisfies this range. Gate passed.

### Step 2: Feature flag decision — opt-out of heavy defaults

- Default features for 0.20.0: `[asset, ui, mouse, keyboard, gamepad, picking]`.
- `asset` pulls `bevy/bevy_asset` (RON-loadable input maps, deferred to Feature #25). `ui` pulls `bevy/bevy_ui`. `picking` pulls `bevy/bevy_picking`. These are non-trivial additions.
- Decision: **opted out** with `default-features = false, features = ["keyboard", "mouse"]`.
- Cargo.toml line: `leafwing-input-manager = { version = "=0.20.0", default-features = false, features = ["keyboard", "mouse"] }`.
- Cargo.lock additions: 7 new packages (`leafwing-input-manager`, `leafwing_input_manager_macros`, `dyn-clone`, `dyn-eq`, `dyn-hash`, `enumn`, `serde_flexitos`). No version bumps to existing packages.

### Step 3: API surface verified

- **`Actionlike` derive:** macro from `leafwing_input_manager_macros`. Required derives: `PartialEq, Eq, Hash, Clone, Copy, Debug, Reflect`. The `Typed + TypePath + FromReflect` bounds on the `Actionlike` trait are auto-satisfied by `#[derive(Reflect)]`. Confirmed from `lib.rs` example line 62.
- **`ActionState::just_pressed` signature:** takes `&A` by reference — `action_state.just_pressed(&DungeonAction::MoveForward)`.
- **`InputMap` builder:** `.with(action, button)` — chainable, takes `Self` by value, returns `Self`. Also has mutable `.insert(&mut self, ...) -> &mut Self`. Used `.with(...)` throughout.
- **`InputManagerPlugin` constructor:** `InputManagerPlugin::<T>::default()` — confirmed.
- **No `MockInput` trait** in 0.20.0. Instead, each input type implements `Buttonlike::press(&self, world: &mut World)` directly via `leafwing_input_manager::user_input::Buttonlike`. This writes the `Messages<KeyboardInput>` message on behalf of the caller. `use leafwing_input_manager::prelude::*` brings `Buttonlike` into scope.
- **`ActionState` is NOT auto-inserted by `InputManagerPlugin`.** Verified at `systems.rs:86-105`: `update_action_state` takes `action_state: Option<ResMut<ActionState<A>>>` (reads both Resource and Component shapes). The `action_state_resource` example confirms the setup: `init_resource::<ActionState<T>>()` must be called explicitly. Added to `ActionsPlugin::build`.

### Tests: `Buttonlike::press` API used instead of manual `Messages<KeyboardInput>` construction

- The plan's `inject_key_press` helper (manually constructing `KeyboardInput` structs) was replaced by `KeyCode::KeyW.press(app.world_mut())` via the `Buttonlike` trait method.
- This is strictly better: the `press` method fills placeholder values for `logical_key` and `window` correctly (the same pattern leafwing's own tests use), and is less error-prone than manual field construction.
- `bevy::ecs::message::Messages` and `bevy::input::keyboard::KeyboardInput` imports are NOT needed in test code because `KeyCode::press` is called via the `Buttonlike` trait brought in by `use leafwing_input_manager::prelude::*` (already in scope from `use super::*`).

### `cargo audit` not available

- `cargo-audit` is not installed in this environment. The `cargo audit` step could not be run. No known advisories for `leafwing-input-manager 0.20.0` as of research date (2026-05-01) per training data. Installation: `cargo install cargo-audit`.

## Verification

- [x] **Step A verification gate** — `cargo add leafwing-input-manager --dry-run` resolves a version with `bevy = "0.18"` requirement — Manual (one-time gate, see Step 1).
- [x] **Step B feature audit** — `cargo info leafwing-input-manager --version <ver>` (or registry Cargo.toml inspection) shows defaults are minimal OR opt-out chosen — Manual (Step 2).
- [x] **Step C API verification** — `grep` of resolved crate confirms `Actionlike` derive, `InputMap` builder shape, `ActionState::just_pressed` arg shape, `MockInput` existence — Manual (Step 3).
- [x] **`cargo check`** — `cargo check` — Automatic — must succeed with zero warnings.
- [x] **`cargo check --features dev`** — `cargo check --features dev` — Automatic — must succeed with zero warnings.
- [x] **`cargo clippy --all-targets -- -D warnings`** — `cargo clippy --all-targets -- -D warnings` — Automatic — must succeed.
- [x] **`cargo clippy --all-targets --features dev -- -D warnings`** — `cargo clippy --all-targets --features dev -- -D warnings` — Automatic — must succeed.
- [x] **`cargo test`** — `cargo test` — Automatic — must pass all tests including unchanged `gamestate_default_is_loading`.
- [x] **`cargo test --features dev`** — `cargo test --features dev` — Automatic — must pass all tests including unchanged `f9_advances_game_state`.
- [x] **`actions_plugin_registers_all_inputmaps`** — registers all three `InputManagerPlugin::<T>` and all three `InputMap<T>` resources — unit (App-level) — `cargo test --package druum actions_plugin_registers_all_inputmaps` — Automatic.
- [x] **`dungeon_w_press_triggers_move_forward`** — KeyW message → `ActionState<DungeonAction>::just_pressed(MoveForward)` — integration (App-level with full InputPlugin) — `cargo test --package druum dungeon_w_press_triggers_move_forward` — Automatic.
- [x] **`dungeon_arrow_up_also_triggers_move_forward`** — ArrowUp message → MoveForward (many-to-many binding) — integration — `cargo test --package druum dungeon_arrow_up_also_triggers_move_forward` — Automatic.
- [x] **`menu_escape_triggers_cancel`** — Escape message → MenuAction::Cancel AND MenuAction::Pause (many-to-many in action direction) — integration — `cargo test --package druum menu_escape_triggers_cancel` — Automatic.
- [x] **`combat_enter_triggers_confirm`** — Enter message → CombatAction::Confirm — integration — `cargo test --package druum combat_enter_triggers_confirm` — Automatic.
- [x] **F9 test unchanged** — pre-existing `f9_advances_game_state` passes without modification — integration — `cargo test --features dev f9_advances_game_state` — Automatic.
- [x] **Cargo.lock diff scope** — `git diff Cargo.lock` shows only `leafwing-input-manager` entry plus its direct deps; NO unrelated transitive bumps to `bevy`, `bevy_common_assets`, `bevy_asset_loader`, `serde`, `ron` — Manual (Step 9 final check).
- [ ] **`cargo audit`** — no new advisories on `leafwing-input-manager` since research date — `cargo audit` — NOT RUN (`cargo-audit` not installed in this environment; install with `cargo install cargo-audit` to verify).

## Estimated LOC delta

Per roadmap §Impact (line 322) the budget for Feature #5 is +120-200 LOC. Concrete estimate:

- `src/plugins/input/mod.rs`: +180-200 LOC (3 enums × ~12 lines each = ~36 lines for derives + variants; 3 default-map fns × ~14 lines each = ~42 lines; ActionsPlugin Plugin impl = ~20 lines; module-level doc comment = ~30 lines; 5 tests + helpers in `mod tests` = ~80 lines).
- `src/plugins/mod.rs`: +1 LOC (`pub mod input;`).
- `src/main.rs`: +2 LOC (1 import line, 1 plugin tuple line).
- `Cargo.toml`: +1 LOC (the leafwing dep line).

**Net: +185-205 LOC**, on the high end of the budget but justified by the 5-test coverage and the comprehensive doc comment that documents the F9 and `DevAction` carve-outs for future contributors.
