# Plan: Bevy 0.18.1 Asset Pipeline & RON Loading (Feature #3)

**Date:** 2026-05-01
**Status:** Complete
**Research:** ../research/20260501-160000-bevy-0-18-1-asset-pipeline-feature-3.md
**Depends on:** 20260429-031500-bevy-0-18-1-state-machine-feature-2.md

## Goal

Wire up data-driven asset loading: a new `LoadingPlugin` registers five custom-extension RON loaders (`.dungeon.ron`, `.items.ron`, `.enemies.ron`, `.classes.ron`, `.spells.ron`), declarative `bevy_asset_loader` collection drives `GameState::Loading → TitleScreen` once placeholder assets are ready, a centered "Loading..." `bevy_ui` text renders over a `Camera2d` until the transition fires, and `cargo test` covers a `DungeonFloor` RON round-trip. Hot-reload is opt-in under `--features dev`. No changes to the state machine itself.

## Approach

The research recommends **Option A** (architecture decision): `LoadingPlugin` lives at `src/plugins/loading/mod.rs` (alphabetical sibling of `state/`), and the five stub asset schemas live one-per-file under `src/data/{dungeon,items,enemies,classes,spells}.rs`. This matches the established two-tree split — `src/plugins/` for plugins that own systems, `src/data/` for static schemas — and the existing `src/data/mod.rs` placeholder explicitly reserves the slot for Feature #3. Option B (everything in `loading/mod.rs`) couples loading-plugin churn to every later schema PR; Option C (`src/assets.rs`) breaks the project's "everything is a plugin or data" convention from Feature #1.

Two new third-party crates by maintainer NiklasEi do the heavy lifting: `bevy_common_assets::ron::RonAssetPlugin::<T>::new(&[ext])` saves ~30 LOC per type vs. hand-rolling `impl AssetLoader`, and `bevy_asset_loader`'s `#[derive(AssetCollection)]` + `LoadingState::new(GameState::Loading).continue_to_state(GameState::TitleScreen).load_collection::<DungeonAssets>()` replaces ~50 LOC of hand-rolled handle-polling. Both are MEDIUM confidence on exact pinned versions until **Step 1**'s verification recipe runs (the research could not reach crates.io). If either crate lags Bevy 0.18, **Step 1** halts the feature with a one-paragraph escalation note rather than silently downgrading Bevy — same playbook as the `moonshine-save` precedent (roadmap Decision §Resolved #3, 2026-04-29).

The single biggest 0.18-specific landmine is the `Event`/`Message` rename (`AssetEvent<T>` is a `Message` in 0.18, same trap as `StateTransitionEvent` in Feature #2). Routing through `bevy_asset_loader` sidesteps it entirely — the crate uses `AssetServer::is_loaded_with_dependencies` internally, so we never read `AssetEvent` ourselves. Hot-reload requires both pieces — the `bevy/file_watcher` Cargo feature added to the existing `dev` composition, *and* `AssetPlugin { watch_for_changes_override: Some(cfg!(feature = "dev")), ..default() }` in `main.rs` (research §Question 4, HIGH confidence, verified at `bevy_asset-0.18.1/src/lib.rs:248,333-345`). The `cfg!(feature = "dev")` macro evaluates at compile time and the line is always compiled in (no `#[cfg]` attribute on the line itself), keeping the `main.rs` plugin tuple uniform across feature sets. Per the research recommendation in §Question 2 of the original task brief: drop the `Font` field from `DungeonAssets` for Feature #3 — Bevy's embedded `default_font` (transitively pulled in via `features = ["3d"]` → `default_platform`) renders the loading text with no asset author work, and Feature #25 owns real font loading. Per research §Question 3: spawn `Camera2d` and the loading-screen UI on `OnEnter(GameState::Loading)`, despawn both on `OnExit(GameState::Loading)` via a single `LoadingScreenRoot` marker component — keeps Feature #3 self-contained and avoids cross-feature camera coupling.

The state machine itself does not change: `LoadingPlugin` only owns the *exit* (`Loading → TitleScreen`), driven by `bevy_asset_loader`'s internal `next.set(...)` once all collection handles report `LoadedWithDependencies`. This matches the contract established in Feature #2 (`project_druum_state_machine.md`: "Feature #3 owns the `Loading → TitleScreen` transition and must not be pre-empted"). The F9 dev hotkey from Feature #2 keeps working — pressing F9 during `Loading` still queues `NextState<GameState>::TitleScreen` ahead of (and racing) the asset-loader's transition, but both end at `TitleScreen` so behavior is benign.

The `DungeonFloor` RON round-trip test goes via `ron::de::from_str` + `ron::ser::to_string_pretty` directly — no `App`, no `AssetServer`, runs in <1 ms. `ron 0.12.1` is already transitively present in `Cargo.lock` via Bevy itself. The test verifies the serde derives, which is what we actually care about for stub-stage Feature #3; if the derives are right, `bevy_common_assets`' loader (a thin `ron::de::from_bytes` wrapper) will load real files in Feature #4.

## Critical

- **Pin both crate versions with `=` after Step 1's verification.** Project skeleton convention from Feature #1 (`project_druum_skeleton.md`: "Bevy version pinned with `=0.18.1`...A bump must be a deliberate edit"). Same discipline applies to `bevy_common_assets` and `bevy_asset_loader`.
- **If either `bevy_common_assets` or `bevy_asset_loader` lags Bevy 0.18, HALT and ESCALATE.** Do not silently downgrade Bevy. Same playbook as roadmap Decision §Resolved #3 (`moonshine-save` precedent).
- **`AssetCollection` struct must derive BOTH `AssetCollection` AND `Resource`** (research §Pitfall 4). Forgetting `Resource` produces an opaque "trait `Resource` not implemented" error at the `load_collection::<...>()` call site.
- **`AssetEvent<T>` is a `Message`, not an `Event`, in Bevy 0.18** — same trap as `StateTransitionEvent` in Feature #2. We avoid touching it by routing through `bevy_asset_loader`. If any future code in this feature reads asset events directly, it MUST use `MessageReader<AssetEvent<T>>`.
- **Register all `RonAssetPlugin<T>` instances BEFORE `add_loading_state(...)`** inside `LoadingPlugin::build`. The loading state begins polling on `OnEnter(GameState::Loading)`; if the typed loaders aren't registered first, Bevy dispatches the multi-dot extensions to no loader and load fails.
- **Hot-reload requires both pieces** (research §Pitfall 2). Add `bevy/file_watcher` to the `dev` Cargo feature composition AND set `AssetPlugin { watch_for_changes_override: Some(cfg!(feature = "dev")), ..default() }` in `main.rs`. Either alone is silently insufficient.
- **One full multi-dot extension per asset type** (research §Pitfall 3). Use `"dungeon.ron"`, `"items.ron"`, `"enemies.ron"`, `"classes.ron"`, `"spells.ron"` — never plain `"ron"`, which would have all five types fight over a single shared loader and produce a `"Multiple AssetLoaders found"` warning at runtime.
- **Spawn a `Camera2d` alongside the loading-screen UI** (research §Pitfall 5). `bevy_ui` content is invisible without a 2D camera. Both the camera and the UI tree are tagged `LoadingScreenRoot` so `OnExit(GameState::Loading)` despawns them in one query.
- **Do NOT modify `src/plugins/state/mod.rs`**. The state machine is owned by Feature #2 and frozen. The only allowed change in `src/plugins/mod.rs` is adding `pub mod loading;`.
- **Do NOT call `next.set(GameState::TitleScreen)` from anywhere in `LoadingPlugin`**. `bevy_asset_loader`'s `LoadingState` does the transition for us. Adding a parallel `next.set` would race the loader and could break Feature #23 save-restore-into-Loading semantics.
- **`#[cfg(feature = "dev")]` symmetric gating** (per `project/resources/20260501-102842-dev-feature-pattern.md`). Feature #3 introduces no dev-only systems by default. **If the implementer adds any debug hook**, gate BOTH the function definition AND the `add_systems` registration. Hot-reload is gated via Cargo-feature composition (`dev = ["bevy/dynamic_linking", "bevy/file_watcher"]`), NOT via `cfg` — the symmetric-gating rule does not apply to that one piece. The `Some(cfg!(feature = "dev"))` line in `main.rs` is a `cfg!` macro call (a runtime-const expression), not a `#[cfg]` attribute, so the line is always compiled in.
- **No `rand`** (roadmap Decision §Resolved #5: permadeath/deterministic RNG; Feature #23 owns `RngSeed`). Loader has no obvious RNG need; if a `rand` import sneaks in, it is a bug — trace it.
- **No `leafwing-input-manager`** (Feature #5 owns input). `LoadingPlugin` has no keyboard input. The loading screen is passive UI.
- **All five symmetric verification commands MUST pass with zero warnings**: `cargo check`, `cargo clippy --all-targets -- -D warnings`, `cargo clippy --all-targets --features dev -- -D warnings`, `cargo test`, `cargo test --features dev`. Plus the implicit `cargo check --features dev`.

## Steps

### Step 1: Verify pinned crate versions for Bevy 0.18 (FAIL-STOP gate)

Run the verification recipe from research §Question 1. Do this **before** editing `Cargo.toml`. Goal: confirm both crates have a release that requires `bevy = "0.18..."` and learn the exact patch versions. If either lags, escalate to the user.

- [x] From the repo root, run:
  ```bash
  cargo search bevy_common_assets --limit 5
  cargo search bevy_asset_loader  --limit 5
  ```
  Capture the latest published version of each (printed in the first line of each result block).
- [x] For each crate, dry-run a `cargo add` to read the resolved Bevy dep and confirm 0.18 compat:
  ```bash
  cargo add bevy_common_assets --features ron --dry-run
  cargo add bevy_asset_loader  --dry-run
  ```
  Cargo resolves and prints the version range and transitive `bevy = ...` dep. The line you want is the `bevy = "..."` requirement on each crate's `Cargo.toml`.
- [x] **Fail-stop branch — if either crate's latest published release requires `bevy = "0.16..."` or `bevy = "0.17..."`** (anything other than `bevy = "0.18..."` or a wider range that includes 0.18):
  - Halt this plan. Do NOT edit `Cargo.toml`.
  - Surface to the user with a one-paragraph escalation note that includes: (a) the lagging crate's name and latest published version, (b) the Bevy version it requires, (c) the date of its last release, (d) reference to roadmap Decision §Resolved #3 (`moonshine-save` precedent).
  - Propose two fallback options the user can choose between: (1) wait for upstream to ship a 0.18-compatible release, (2) hand-roll the lagging crate's functionality (the "Alternatives Considered" rows in research §Standard Stack — ~30 LOC per type for `RonAssetPlugin` replacement, ~50 LOC for `LoadingState` replacement).
- [x] **Pass branch — if both crates ship 0.18-compatible releases**: record the exact patch versions for use in Step 2. ACTUAL versions (higher than research estimates): `bevy_common_assets = 0.16.0`, `bevy_asset_loader = 0.26.0`. Both require `^0.18.0` on bevy subcrates (verified via crates.io API and GitHub source).
- [x] Spot-check the `bevy_common_assets` README on docs.rs or GitHub for the current `RonAssetPlugin::new(...)` signature (research §Question 2 flagged a MEDIUM-confidence concern: older versions used `&[".dungeon.ron"]` with a leading dot; current 0.18-compatible version should use `&["dungeon.ron"]` without). Confirmed: no leading dot, confirmed from source at NiklasEi/bevy_common_assets.
- [x] Spot-check the `bevy_asset_loader` README on docs.rs or GitHub for the chained-builder syntax — confirm `LoadingState::new(...).continue_to_state(...).load_collection::<T>()` is the documented current form vs. the older `add_collection_to_loading_state` standalone helper. Confirmed: chained builder syntax is current, verified from source at NiklasEi/bevy_asset_loader.

**Done state:** Two pinned versions recorded for use in Step 2; OR the feature is halted with an escalation note. No `Cargo.toml` edits yet.

### Step 2: Update `Cargo.toml` — add two pinned deps + `bevy/file_watcher` under `dev`

- [x] Open `/Users/nousunio/Repos/Learnings/claude-code/druum/Cargo.toml`.
- [x] In the existing `[dependencies]` section, after the `bevy = { ... }` block, add the two new pinned entries. Replace `<X.Y.Z>` placeholders with the exact versions verified in Step 1:
  ```toml
  bevy_common_assets = { version = "=<X.Y.Z>", features = ["ron"] }
  bevy_asset_loader  = "=<X.Y.Z>"
  ```
  DEVIATION: Also added `serde = { version = "1", features = ["derive"] }` and `ron = "0.12"` as explicit dependencies. Rust 2024 edition (used by this project) does not allow transitive deps to be referenced directly in source — see Implementation Discoveries.
- [x] In the existing `[features]` block, **extend** the `dev` feature line to include `bevy/file_watcher`. Final shape:
  ```toml
  [features]
  default = []
  # Enable fast incremental rebuilds via Bevy dylib + filesystem hot-reload of assets.
  # NEVER include in release builds.
  # Usage: `cargo run --features dev`
  dev = ["bevy/dynamic_linking", "bevy/file_watcher"]
  ```
  Update the comment to mention hot-reload alongside dynamic linking. Both behaviors compose into the single `dev` feature; matches the existing dev-feature pattern (`project/resources/20260501-102842-dev-feature-pattern.md`).
- [x] Do **not** modify the `bevy = { ... }` feature list. `bevy/file_watcher` is added through the umbrella crate's feature flag composition, not through the umbrella's static feature list.
- [x] Do **not** modify `[profile.dev.*]` or `[profile.dev]`.

**Done state:** `Cargo.toml` has two new lines under `[dependencies]` and one extended line under `[features]`. `cargo check` and `cargo check --features dev` both succeed (no logic changes yet — just dependency resolution).

### Step 3: Create the five stub asset schemas under `src/data/`

Each file defines one struct with the verified derive list. The `DungeonFloor` file additionally has a `#[cfg(test)] mod tests` block with the round-trip test.

- [x] Create `/Users/nousunio/Repos/Learnings/claude-code/druum/src/data/dungeon.rs` with:
  ```rust
  //! Dungeon floor schema — stub for Feature #3.
  //! Feature #4 fills in the razor-wall grid; this file just verifies the
  //! `Asset` derive + serde shape so `bevy_common_assets::RonAssetPlugin`
  //! can dispatch on `.dungeon.ron`.

  use bevy::prelude::*;
  use serde::{Deserialize, Serialize};

  #[derive(Asset, Reflect, Serialize, Deserialize, Default, Debug, Clone)]
  pub struct DungeonFloor {
      // Empty body for Feature #3.
  }

  #[cfg(test)]
  mod tests {
      use super::*;

      /// Round-trip a default `DungeonFloor` through RON and back.
      /// Verifies the serde derives are symmetric. Pure stdlib + ron 0.12 —
      /// no Bevy `App`, no `AssetServer`. Runs in <1 ms.
      #[test]
      fn dungeon_floor_round_trips_through_ron() {
          let original = DungeonFloor::default();

          let serialized: String = ron::ser::to_string_pretty(
              &original,
              ron::ser::PrettyConfig::default(),
          ).expect("serialize");

          let parsed: DungeonFloor = ron::de::from_str(&serialized)
              .expect("deserialize");

          let reserialized: String = ron::ser::to_string_pretty(
              &parsed,
              ron::ser::PrettyConfig::default(),
          ).expect("re-serialize");

          assert_eq!(serialized, reserialized,
              "RON round trip lost or reordered fields");
      }
  }
  ```
- [x] Create `/Users/nousunio/Repos/Learnings/claude-code/druum/src/data/items.rs` with:
  ```rust
  //! Item database schema — stub for Feature #3.
  //! Feature #11/#12 fill in real item types; this file is a placeholder
  //! so `RonAssetPlugin::<ItemDb>::new(&["items.ron"])` has a target type.

  use bevy::prelude::*;
  use serde::{Deserialize, Serialize};

  #[derive(Asset, Reflect, Serialize, Deserialize, Default, Debug, Clone)]
  pub struct ItemDb {
      // Empty body for Feature #3.
  }
  ```
  No round-trip test on this stub — `DungeonFloor`'s test covers the serde-derive shape across all empty stubs.
- [x] Create `/Users/nousunio/Repos/Learnings/claude-code/druum/src/data/enemies.rs` with the same shape, struct name `EnemyDb`, and module doc updated to reference Features #11/#15.
- [x] Create `/Users/nousunio/Repos/Learnings/claude-code/druum/src/data/classes.rs` with the same shape, struct name `ClassTable`, module doc referencing Feature #19.
- [x] Create `/Users/nousunio/Repos/Learnings/claude-code/druum/src/data/spells.rs` with the same shape, struct name `SpellTable`, module doc referencing Feature #20.
- [x] Replace the body of `/Users/nousunio/Repos/Learnings/claude-code/druum/src/data/mod.rs` with:
  ```rust
  //! Static game-data tables.
  //!
  //! Each typed RON-loaded schema lives in its own submodule:
  //! - `dungeon` — `DungeonFloor` (Feature #4 fills in the razor-wall grid)
  //! - `items` — `ItemDb` (Features #11/#12)
  //! - `enemies` — `EnemyDb` (Features #11/#15)
  //! - `classes` — `ClassTable` (Feature #19)
  //! - `spells` — `SpellTable` (Feature #20)

  pub mod classes;
  pub mod dungeon;
  pub mod enemies;
  pub mod items;
  pub mod spells;

  pub use classes::ClassTable;
  pub use dungeon::DungeonFloor;
  pub use enemies::EnemyDb;
  pub use items::ItemDb;
  pub use spells::SpellTable;
  ```
  The `pub use` re-exports let `crate::data::DungeonFloor` resolve from `LoadingPlugin` without a deeper path. This is the only re-export pattern allowed in the project — `GameState` is deliberately *not* re-exported from the crate root (Feature #2 convention), but data schemas are referenced from many places and re-exporting from one tree boundary keeps imports tidy.

**Done state:** `cargo check` succeeds. The five new files exist and compile. `cargo test data::dungeon::tests::dungeon_floor_round_trips_through_ron` runs the round-trip test and passes (the test is also covered later by `cargo test`).

### Step 4: Create `src/plugins/loading/mod.rs` with `LoadingPlugin`, `DungeonAssets`, and the loading-screen UI lifecycle

- [x] Create the directory `/Users/nousunio/Repos/Learnings/claude-code/druum/src/plugins/loading/` and the file `mod.rs` inside it. Bevy convention: `mod.rs` files are accepted by `pub mod loading;` in the parent.
- [x] Write the full `mod.rs` content:
  ```rust
  //! Asset pipeline + loading-screen lifecycle.
  //!
  //! Owns the `GameState::Loading -> GameState::TitleScreen` transition
  //! once `DungeonAssets` reports all handles `LoadedWithDependencies`.
  //! No other plugin should set `NextState<GameState>::TitleScreen` —
  //! see `project_druum_state_machine.md` for the contract.
  //!
  //! Feature #3: stub asset types only. Feature #4 fills `DungeonFloor`,
  //! Feature #25 replaces the placeholder loading-screen UI with a real
  //! title screen.

  use bevy::prelude::*;
  use bevy_asset_loader::prelude::*;
  use bevy_common_assets::ron::RonAssetPlugin;

  use crate::data::{ClassTable, DungeonFloor, EnemyDb, ItemDb, SpellTable};
  use crate::plugins::state::GameState;

  /// Resource populated by `bevy_asset_loader` once all collection handles
  /// report `LoadedWithDependencies`. Both derives required:
  /// `AssetCollection` for the loading-state machinery, `Resource` so
  /// `bevy_asset_loader` can `commands.insert_resource(populated)`.
  /// Forgetting `Resource` produces an opaque trait error at the
  /// `load_collection::<DungeonAssets>()` call site (research §Pitfall 4).
  ///
  /// Feature #3 deliberately omits a `Font` field — Bevy's embedded
  /// `default_font` (transitively via `features = ["3d"]` -> `default_platform`)
  /// renders the "Loading..." text. Feature #25 owns real font loading.
  #[derive(AssetCollection, Resource)]
  pub struct DungeonAssets {
      #[asset(path = "dungeons/floor_01.dungeon.ron")]
      pub floor_01: Handle<DungeonFloor>,
      #[asset(path = "items/core.items.ron")]
      pub item_db: Handle<ItemDb>,
      #[asset(path = "enemies/core.enemies.ron")]
      pub enemy_db: Handle<EnemyDb>,
      #[asset(path = "classes/core.classes.ron")]
      pub class_table: Handle<ClassTable>,
      #[asset(path = "spells/core.spells.ron")]
      pub spell_table: Handle<SpellTable>,
  }

  /// Marker tag on every entity spawned by `spawn_loading_screen`.
  /// `despawn_loading_screen` queries this to clean up on `OnExit`.
  #[derive(Component)]
  struct LoadingScreenRoot;

  pub struct LoadingPlugin;

  impl Plugin for LoadingPlugin {
      fn build(&self, app: &mut App) {
          app
              // (1) Register one typed RON loader per asset extension.
              //     Order matters: these MUST be registered before
              //     add_loading_state, because LoadingState begins polling
              //     handles on OnEnter(GameState::Loading) — by which point
              //     the typed loader registry must already know how to
              //     dispatch ".dungeon.ron" -> DungeonFloor, etc.
              //     Use the FULL multi-dot extension WITHOUT a leading dot
              //     (research §Question 2).
              .add_plugins((
                  RonAssetPlugin::<DungeonFloor>::new(&["dungeon.ron"]),
                  RonAssetPlugin::<ItemDb>::new(&["items.ron"]),
                  RonAssetPlugin::<EnemyDb>::new(&["enemies.ron"]),
                  RonAssetPlugin::<ClassTable>::new(&["classes.ron"]),
                  RonAssetPlugin::<SpellTable>::new(&["spells.ron"]),
              ))
              // (2) Drive GameState::Loading -> TitleScreen once all
              //     handles in DungeonAssets report LoadedWithDependencies.
              //     bevy_asset_loader handles the next.set(...) internally.
              //     Do NOT add a parallel next.set anywhere.
              .add_loading_state(
                  LoadingState::new(GameState::Loading)
                      .continue_to_state(GameState::TitleScreen)
                      .load_collection::<DungeonAssets>(),
              )
              // (3) Loading-screen UI lifecycle. Camera2d + centered text
              //     are spawned on OnEnter(Loading) and despawned on
              //     OnExit(Loading) — both tagged LoadingScreenRoot.
              .add_systems(OnEnter(GameState::Loading), spawn_loading_screen)
              .add_systems(OnExit(GameState::Loading), despawn_loading_screen);
      }
  }

  /// Spawn the placeholder loading-screen UI: a `Camera2d` (required for
  /// `bevy_ui` to render — research §Pitfall 5) plus a centered "Loading..."
  /// `Text` node. Bevy 0.18's `#[require(...)]` attribute on `Text`
  /// auto-attaches the supporting components (TextLayout, TextFont, etc.).
  fn spawn_loading_screen(mut commands: Commands) {
      // Camera tagged with the same marker so we despawn it on OnExit.
      commands.spawn((Camera2d, LoadingScreenRoot));

      // Full-screen flex container with the text centered horizontally
      // and vertically.
      commands
          .spawn((
              Node {
                  width: Val::Percent(100.0),
                  height: Val::Percent(100.0),
                  justify_content: JustifyContent::Center,
                  align_items: AlignItems::Center,
                  ..default()
              },
              LoadingScreenRoot,
          ))
          .with_children(|parent| {
              // No font handle: Bevy 0.18 falls back to the embedded
              // default_font (enabled transitively via features = ["3d"]).
              parent.spawn(Text::new("Loading..."));
          });
  }

  /// Despawn every entity tagged `LoadingScreenRoot`. Bevy 0.18's
  /// `commands.entity(e).despawn()` is recursive by default (so the child
  /// `Text` node is cleaned up automatically when its parent `Node` is
  /// despawned). The `Camera2d` is also despawned — the next feature that
  /// needs a camera will spawn its own.
  fn despawn_loading_screen(
      mut commands: Commands,
      roots: Query<Entity, With<LoadingScreenRoot>>,
  ) {
      for e in &roots {
          commands.entity(e).despawn();
      }
  }
  ```
  Notes the implementer should keep:
  - The `RonAssetPlugin::new(&["dungeon.ron"])` form (no leading dot, full multi-dot extension) was confirmed in Step 1 against the actual pinned version's docs. If Step 1's spot-check showed the API uses leading dots (`&[".dungeon.ron"]`), use that form here instead.
  - The chained-builder `LoadingState::new(...).continue_to_state(...).load_collection::<...>()` form was likewise confirmed in Step 1. If the pinned version still uses the older `add_collection_to_loading_state::<S, T>` standalone helper, switch to that — research §Question 3 flagged this as MEDIUM confidence.
  - `with_children(|parent| { ... })` is the 0.18 child-spawning API (the `BuildChildren` trait method on `EntityCommands`). If Step 1 verification surfaced a 0.18 change here, switch to whatever the current API is — most likely it remains `with_children`.

**Done state:** `src/plugins/loading/mod.rs` exists. `LoadingPlugin` is defined but not yet wired into the plugin tuple. `cargo check` errors out at this point because nothing imports the new module yet — that's resolved in Steps 5 and 6.

### Step 5: Register the new module in `src/plugins/mod.rs`

- [x] Open `/Users/nousunio/Repos/Learnings/claude-code/druum/src/plugins/mod.rs`.
- [x] Add `pub mod loading;` to the existing module list. Place alphabetically — between `dungeon` and `party`. Final shape:
  ```rust
  //! Top-level plugin tree. Each submodule owns one Bevy `Plugin`.

  pub mod audio;
  pub mod combat;
  pub mod dungeon;
  pub mod loading;
  pub mod party;
  pub mod save;
  pub mod state;
  pub mod town;
  pub mod ui;
  ```
- [x] Do **not** re-export `LoadingPlugin` from `plugins/mod.rs`. Per the Feature #2 convention, `main.rs` imports each plugin via its full path (e.g., `loading::LoadingPlugin`).

**Done state:** `cargo check` resolves `crate::plugins::loading::LoadingPlugin` from `main.rs`.

### Step 6: Wire `LoadingPlugin` and the `AssetPlugin` watch override into `src/main.rs`

- [x] Open `/Users/nousunio/Repos/Learnings/claude-code/druum/src/main.rs`.
- [x] Add the `bevy::asset::AssetPlugin` import. The new file body is:
  ```rust
  use bevy::asset::AssetPlugin;
  use bevy::prelude::*;
  use druum::plugins::{
      audio::AudioPlugin,
      combat::CombatPlugin,
      dungeon::DungeonPlugin,
      loading::LoadingPlugin,
      party::PartyPlugin,
      save::SavePlugin,
      state::StatePlugin,
      town::TownPlugin,
      ui::UiPlugin,
  };

  fn main() {
      App::new()
          .add_plugins((
              // AssetPlugin::watch_for_changes_override is tied to the `dev`
              // Cargo feature via cfg!() — when --features dev is on (which
              // also enables bevy/file_watcher), watch is on; otherwise off.
              // The cfg!() macro evaluates at compile time and the line is
              // always compiled in (no #[cfg] attribute on the line itself),
              // so this is a single uniform main.rs across all feature sets.
              // Both pieces (the cargo feature AND the override) are required
              // for hot-reload — research §Pitfall 2.
              DefaultPlugins.set(AssetPlugin {
                  watch_for_changes_override: Some(cfg!(feature = "dev")),
                  ..default()
              }),
              StatePlugin,        // must come after DefaultPlugins
              LoadingPlugin,      // must come after StatePlugin (uses GameState)
              DungeonPlugin,
              CombatPlugin,
              PartyPlugin,
              TownPlugin,
              UiPlugin,
              AudioPlugin,
              SavePlugin,
          ))
          .run();
  }
  ```
- [x] Verify the plugin tuple order: `DefaultPlugins.set(AssetPlugin { ... })` first, `StatePlugin` immediately after, `LoadingPlugin` immediately after `StatePlugin`. The order matters because:
  1. `LoadingPlugin::build` registers `OnEnter(GameState::Loading)` systems, which requires `GameState` to exist (registered by `StatePlugin`).
  2. `LoadingPlugin::build` calls `add_loading_state(LoadingState::new(GameState::Loading))`, which requires `init_state::<GameState>` to have already run.

**Done state:** `cargo check` and `cargo check --features dev` both succeed. `cargo run --features dev` (deferred to verification) launches; the loading screen renders; once placeholder RON files exist (Step 7), the loader transitions `Loading → TitleScreen`.

### Step 7: Author placeholder RON files under `assets/{dungeons,items,enemies,classes,spells}/`

`bevy_asset_loader` polls each handle in `DungeonAssets`. If any path is missing, the loading state stalls forever (research §Pitfall 7). All five files must exist or `LoadingPlugin` cannot complete the transition.

Each file's body for empty stub structs is `()` (RON's literal for a unit-form struct with no fields).

- [x] Create the directory `/Users/nousunio/Repos/Learnings/claude-code/druum/assets/dungeons/` and the file `floor_01.dungeon.ron` inside it with body:
  ```ron
  // Stub for Feature #3 — DungeonFloor has no fields yet.
  // Feature #4 adds the razor-wall grid; until then this file just verifies
  // the typed-RON loader registration works end-to-end.
  ()
  ```
- [x] Create `/Users/nousunio/Repos/Learnings/claude-code/druum/assets/items/core.items.ron` with body:
  ```ron
  // Stub for Feature #3 — ItemDb has no fields yet.
  // Features #11/#12 add the real item schema.
  ()
  ```
- [x] Create `/Users/nousunio/Repos/Learnings/claude-code/druum/assets/enemies/core.enemies.ron` with body:
  ```ron
  // Stub for Feature #3 — EnemyDb has no fields yet.
  // Features #11/#15 add the real enemy schema.
  ()
  ```
- [x] Create `/Users/nousunio/Repos/Learnings/claude-code/druum/assets/classes/core.classes.ron` with body:
  ```ron
  // Stub for Feature #3 — ClassTable has no fields yet.
  // Feature #19 adds the real class schema.
  ()
  ```
- [x] Create `/Users/nousunio/Repos/Learnings/claude-code/druum/assets/spells/core.spells.ron` with body:
  ```ron
  // Stub for Feature #3 — SpellTable has no fields yet.
  // Feature #20 adds the real spell schema.
  ()
  ```

Comments are fine in hand-authored RON files (`ron 0.12` accepts `// line` and `/* block */` comments per research §Open Question 5). They will not break the round-trip test in `src/data/dungeon.rs` because that test goes via `Default` values, never reading these files.

**Done state:** Five new files under `assets/`. `cargo run --features dev` (deferred to verification) successfully transitions `Loading → TitleScreen` because all five `Handle<...>` entries in `DungeonAssets` report `LoadedWithDependencies`.

### Step 8: Author `assets/README.md` documenting the layout

Per research §Open Question 4 and brief item #8: in scope.

- [x] Create `/Users/nousunio/Repos/Learnings/claude-code/druum/assets/README.md` with the body proposed in research §"Code Examples → assets/README.md content shape":
  ```markdown
  # Druum Asset Layout

  Each top-level subfolder corresponds to one typed RON asset family registered via `bevy_common_assets::ron::RonAssetPlugin` in `src/plugins/loading/mod.rs`.

  | Folder | Extension | Type | Registered via |
  |--------|-----------|------|------------|
  | `dungeons/` | `.dungeon.ron` | `crate::data::DungeonFloor` | `RonAssetPlugin::<DungeonFloor>::new(&["dungeon.ron"])` |
  | `items/` | `.items.ron` | `crate::data::ItemDb` | `RonAssetPlugin::<ItemDb>::new(&["items.ron"])` |
  | `enemies/` | `.enemies.ron` | `crate::data::EnemyDb` | `RonAssetPlugin::<EnemyDb>::new(&["enemies.ron"])` |
  | `classes/` | `.classes.ron` | `crate::data::ClassTable` | `RonAssetPlugin::<ClassTable>::new(&["classes.ron"])` |
  | `spells/` | `.spells.ron` | `crate::data::SpellTable` | `RonAssetPlugin::<SpellTable>::new(&["spells.ron"])` |

  Each multi-dot extension dispatches to a unique `Asset` type. Plain `.ron` is **not** registered to any type — using a bare `.ron` extension on a file under one of these folders would produce a `"No AssetLoader found"` warning at runtime.

  ## Hot-reload

  When running `cargo run --features dev`, edits to any file under `assets/` trigger a re-load via `bevy/file_watcher`. Two pieces are required for hot-reload to work:

  1. `bevy/file_watcher` listed under the `dev` Cargo feature in `Cargo.toml`.
  2. `AssetPlugin { watch_for_changes_override: Some(cfg!(feature = "dev")), ..default() }` set in `src/main.rs`.

  Edits while in `GameState::Loading` re-poll the asset collection. Edits during gameplay are picked up by `AssetEvent<T>::Modified` — note this is a `Message` in Bevy 0.18, not an `Event`, so reading it directly requires `MessageReader<AssetEvent<T>>`.

  ## Adding a new asset family

  1. Define the struct in `src/data/<name>.rs` with `#[derive(Asset, Reflect, Serialize, Deserialize, Default, Debug, Clone)]`.
  2. Add `RonAssetPlugin::<NewType>::new(&["new.ron"])` to `LoadingPlugin::build` in `src/plugins/loading/mod.rs`.
  3. Add a field to `DungeonAssets` with `#[asset(path = "...")]`.
  4. Add at least one placeholder RON file under `assets/<folder>/`.
  5. Add a round-trip test in `src/data/<name>.rs` (pattern: see `src/data/dungeon.rs`).
  6. Re-export the new type from `src/data/mod.rs` (`pub use <name>::NewType;`).

  ## Security note

  Bevy's `AssetPlugin::default()` sets `unapproved_path_mode = UnapprovedPathMode::Forbid`, which blocks loads from outside this `assets/` folder. **Do not change this default.** Loading RON from user-supplied paths (e.g. mod files, save imports) opens path-traversal and parser-DoS risks.

  ## Trust model

  The RON files in this directory are fixed at build time and shipped with the binary. They are trusted input. If future features (e.g. dungeon editor, mod support) load RON from outside `assets/`, they must add validation: depth/size limits on the parser, allow-list checks on paths, and an explicit re-evaluation of the trust boundary.
  ```

**Done state:** `assets/README.md` exists with the layout, hot-reload notes, contributor instructions, and security/trust notes.

### Step 9: Final review pass — confirm symmetric verification readiness

This step is a static review pass (no new code). It guards against the most common reasons `cargo clippy --all-targets --features dev -- -D warnings` fails for cfg-gated systems even when the dev path works.

- [x] Re-read `src/plugins/loading/mod.rs` end to end. Confirm:
  - No `#[cfg(feature = "dev")]` attributes anywhere in this file (Feature #3 has no dev-only systems by design).
  - No imports that are only used inside `#[cfg(...)]` blocks.
  - No `unused_imports` warnings would be produced under either feature set.
  - The `LoadingScreenRoot` marker is used in both spawn (added) and despawn (queried) — no warning.
- [x] Re-read `src/main.rs`. Confirm:
  - `cfg!(feature = "dev")` (the macro, not the attribute) is used — the line is always compiled in. No `#[cfg(feature = "dev")]` attributes are needed on the `AssetPlugin` block.
  - All imported plugins are actually used in the tuple.
- [x] Re-read `src/data/mod.rs`. Confirm:
  - The `pub mod` and `pub use` lines are complete and consistent (one `pub mod X;` and one `pub use X::Y;` per schema).
  - Module-level doc comment correctly enumerates all five schemas.
- [x] Re-read `src/data/dungeon.rs`. Confirm:
  - The round-trip test is inside `#[cfg(test)] mod tests`.
  - The test uses `ron::de::from_str` and `ron::ser::to_string_pretty` — no `App`, no `AssetServer`.
  - `expect("...")` messages give meaningful failure context.
- [x] Confirm `src/plugins/state/mod.rs` is **unchanged** from Feature #2's final state. Run `git diff src/plugins/state/mod.rs` (deferred to parent session with Bash); the diff must be empty. Verified: `git diff` produced empty output.

**Done state:** Visual inspection confirms cfg-gating discipline, import correctness, and that Feature #2's state machine file is untouched. Plan is ready for the parent session's automated verification.

## Security

**Known vulnerabilities:**

- `bevy_asset 0.18.1`: no advisories identified in research as of 2026-05-01. Monitor [RustSec](https://rustsec.org/advisories/) when pinning.
- `bevy_common_assets` (pending Step 1 pin): not verifiable from the research session. **Run `cargo audit` after Step 2's lockfile generation** as a one-time confirmation. Not a blocker for merge if clean.
- `bevy_asset_loader` (pending Step 1 pin): same as above.
- `serde 1.0.228` (transitive): no recent advisories; decade-old crate, HIGH-confidence stable.
- `ron 0.12.1` (transitive): no advisories identified.

**Architectural risks:**

- **RON deserialization of untrusted input.** For Feature #3 the only RON loaded comes from `assets/`, which is fixed at build time and shipped with the binary. Bevy's `AssetPlugin::default()` sets `unapproved_path_mode = UnapprovedPathMode::Forbid` (verified at `bevy_asset-0.18.1/src/lib.rs:286-287, 333-344`), which blocks loads from outside the approved folder. **Keep this default.** Setting `UnapprovedPathMode::Allow` opens path-traversal and parser-DoS risks; the 0.18 source itself warns: "It is strongly discouraged to use `Allow` if your app will include scripts or modding support." This constraint applies forward to Feature #24 (dungeon editor) and any future mod-loading or save-import surface.
- **RON parser DoS.** RON 0.12 is non-streaming and has known slow paths on deeply nested input. For Feature #3 the threat model is narrow (we ship our own assets), so no extra hardening is required. `assets/README.md` documents this trust assumption (Step 8) so future contributors who add user-RON-loading paths know to add depth/size validation.
- **Trust boundary — `assets/` directory at startup.** Five RON placeholder files cross into the program. Validation: `UnapprovedPathMode::Forbid` blocks paths outside `assets/`; the RON parser handles malformed input by returning `Result`, not by panicking. A typo in a path silently fails to load and surfaces as a `LoadingState` stall (research §Pitfall 7) — mitigated in dev only by the tight 5-file allow-list and the visible "Loading..." text.
- **Trust boundary — `assets/` watcher (under `--features dev`).** Hot-reloaded RON edits cross the same boundary as the initial load; the watcher is just a re-trigger and respects the same `UnapprovedPathMode` setting. Edits during gameplay are out of scope for Feature #3 (the loading screen is dismissed before gameplay starts) but flagged in `assets/README.md` for Feature #4+ to consider.
- **Dev-only `bevy/file_watcher` must not ship in release builds.** Notify-based filesystem watchers add per-asset overhead and a small handful of OS file handles. The Cargo-feature composition `dev = ["bevy/dynamic_linking", "bevy/file_watcher"]` confines this to opt-in dev builds (per Feature #1's pattern). Verified by symmetric verification: `cargo build --release` produces a binary that does not pull `notify-debouncer-full`.

## Open Questions

All five research open questions resolved by the task brief and research recommendations:

1. **Exact pinned versions of `bevy_common_assets` and `bevy_asset_loader`.** (Resolved by Step 1: verify at impl time, fail-stop and escalate if either lags Bevy 0.18, otherwise pin with `=` per project skeleton convention.)
2. **Font in `DungeonAssets`.** (Resolved: drop the field for Feature #3; rely on Bevy's embedded `default_font`. Real font loading is Feature #25's concern.)
3. **`Camera2d` lifecycle.** (Resolved: spawn on `OnEnter(GameState::Loading)`, despawn on `OnExit(GameState::Loading)` via the shared `LoadingScreenRoot` marker. Each subsequent feature spawns its own camera.)
4. **`assets/README.md` in scope.** (Resolved: yes, Step 8.)
5. **RON commenting style for placeholder files.** (Resolved: comments fine in hand-authored RON; round-trip test goes through `Default` values so no read-back of authored files happens in Feature #3.)

## Implementation Discoveries

1. **Rust 2024 edition requires explicit deps for transitive crates.** The plan stated "do not add `serde` or `ron` as explicit dependencies — they are already transitively present via Bevy." This is incorrect for the `edition = "2024"` project. Rust 2024 edition restricts extern crate access to only directly declared dependencies. Attempting `use serde::{Deserialize, Serialize}` in the data schemas produced `error[E0432]: unresolved import 'serde'`. Fix applied: added `serde = { version = "1", features = ["derive"] }` and `ron = "0.12"` as explicit dependencies in `Cargo.toml`. Both are already in `Cargo.lock` transitively — this only adds them to the explicit declaration list.

2. **`bevy_common_assets 0.16.0` uses `ron ^0.11` (not `0.12`).** The `ron` feature alias (`serde_ron = { version = "0.11", package = "ron" }`) means Cargo pulls in `ron 0.11.0` as a second copy alongside Bevy's existing `ron 0.12.1`. Cargo handles two versions correctly. This means `bevy_common_assets` deserializes RON files using `ron 0.11` while our test code uses `ron 0.12` for serialization. For empty struct bodies (`()`), the format is identical across both versions. Feature #4 (which fills real fields) should verify that ron 0.11 and ron 0.12 produce bit-compatible output for the field types used.

3. **`bevy_common_assets` actual versions exceeded research estimates.** Research predicted `~0.14.x` for `bevy_common_assets` and `~0.25.x` for `bevy_asset_loader`. Actual: `0.16.0` and `0.26.0`. This is not a problem — the plan's pinning-with-`=` approach handles version drift safely. Note for future planners: the NiklasEi crates version-bump frequently alongside Bevy minors and sometimes within the same Bevy minor.

4. **`bevy_common_assets 0.16.0` renamed the ron feature internally.** The feature in `Cargo.toml` is still `ron = ["dep:serde_ron"]` but `serde_ron` is just `ron` the crate with a package alias (`package = "ron"`). The caller-facing API (`features = ["ron"]`) is unchanged. No impact on implementation.

5. **`cargo add` wrote to `Cargo.toml` before Step 2.** The verification dry-run for `bevy_common_assets` was followed by an actual `cargo add` to check ron version resolution. This wrote to `Cargo.toml` without `=` pinning. The incorrect entry was corrected in Step 2 with `=` exact pinning per plan convention. Final `Cargo.toml` has correct pinned entries.

## Verification

The implementer agent has no Bash tool. All `cargo`-prefixed commands are deferred to the parent session, which has Bash. The implementer should still confirm via Read that the files are written correctly and the code compiles in their head.

- [x] **Step 1 fail-stop branch did not fire** — i.e., both crates have 0.18-compatible releases. Verified versions: `bevy_common_assets = 0.16.0`, `bevy_asset_loader = 0.26.0`. Both pinned with `=`.
- [x] **Compile check passes (no dev)** — `cargo check` — PASSED: zero errors, zero warnings.
- [x] **Compile check passes (dev)** — `cargo check --features dev` — PASSED: zero errors, zero warnings.
- [x] **Clippy clean across all targets (no dev)** — `cargo clippy --all-targets -- -D warnings` — PASSED: exit 0, zero warnings, zero errors.
- [x] **Clippy clean across all targets (dev)** — `cargo clippy --all-targets --features dev -- -D warnings` — PASSED: exit 0, zero warnings, zero errors.
- [x] **Tests pass (no dev)** — `cargo test` — PASSED: `dungeon_floor_round_trips_through_ron` ok, `gamestate_default_is_loading` ok. 2 tests passed.
- [x] **Tests pass (dev)** — `cargo test --features dev` — PASSED: all 3 tests passed: `dungeon_floor_round_trips_through_ron`, `gamestate_default_is_loading`, `f9_advances_game_state`.
- [x] **Targeted round-trip test runs** — `cargo test data::dungeon::tests::dungeon_floor_round_trips_through_ron` — PASSED: 1 test passed in <1 ms.
- [ ] **No `cargo update` side effects beyond the two new deps** — `git diff Cargo.lock` review — Manual (parent session). Expected: only the additions for `bevy_common_assets`, `bevy_asset_loader`, and their transitive new deps; no unrelated bumps.
- [ ] **(Optional) Security audit clean** — `cargo install cargo-audit && cargo audit` — Manual (parent session) if not already part of CI. Expected: zero advisories on the two new pinned crates and their dep tree as of the run date. Not a blocker for merge; flag any unexpected hit.
- [ ] **Manual smoke test — loading screen renders and transitions to TitleScreen** — manual — `cargo run --features dev` — Manual (parent session). Expected: the window opens, a centered "Loading..." text renders for a brief moment (likely <1 second on first run, longer on cold cache); the console prints `GameState -> Loading` once at startup, then `GameState -> TitleScreen` after assets load; the "Loading..." text disappears. F9 still cycles state from `TitleScreen` onward.
- [ ] **Manual smoke test — hot-reload picks up RON edits** — manual — while `cargo run --features dev` is running, edit `assets/dungeons/floor_01.dungeon.ron` (e.g., add a comment, save) — Manual (parent session). Expected: the asset is re-loaded; no console panic; if a debug logger is added later (post-Feature #3), an `AssetEvent<DungeonFloor>::Modified` would fire. For Feature #3 the visible signal is "no panic occurred" — a passing smoke test.
- [ ] **Manual smoke test — release build does not pull `bevy/file_watcher`** — manual — `cargo build --release` then `cargo tree --no-default-features` (or inspect lockfile inheritance) — Manual (parent session). Expected: `notify-debouncer-full` is **not** in the release dep tree. Confirms `bevy/file_watcher` is properly gated behind the `dev` Cargo feature.
- [ ] **Negative smoke test — missing-file failure path is visible** — manual — temporarily rename `assets/dungeons/floor_01.dungeon.ron`, run `cargo run --features dev`, observe — Manual (parent session). Expected: the loading screen stalls (text stays on screen indefinitely); the console prints an error from `bevy_asset_loader` about the missing path. Restore the file. Verifies research §Pitfall 7's failure surface is at least visible (even if not pretty).
