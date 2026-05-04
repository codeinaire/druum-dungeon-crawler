# Plan: Auto-Map / Minimap — Feature #10

**Date:** 2026-05-04
**Status:** Draft (awaiting user decisions D1–D7 before implementer dispatch)
**Research:** ../research/20260504-130000-feature-10-auto-map-minimap.md
**Depends on:** 20260502-000000-feature-5-input-system-leafwing.md, 20260503-130000-feature-7-grid-movement-first-person-camera.md, 20260504-050000-feature-9-dungeon-lighting-atmosphere.md

## Goal

Add the auto-map: a `MovedEvent`-driven `ExploredCells` resource that records visited cells per `(floor, x, y)`, painted via `bevy_egui` as a top-right 200×200 overlay during `DungeonSubState::Exploring` and as a full-screen view during `DungeonSubState::Map`. Players press `M` (already bound to `DungeonAction::OpenMap`) to toggle. Dark-zone cells skip exploration updates and render `?`. A `#[cfg(feature = "dev")]` `show_full` toggle reveals all cells for debugging. Net delivery: one new file (`src/plugins/dungeon/minimap.rs`), one new line in `Cargo.toml` (`bevy_egui` with version + features resolved by Steps A/B/C), one new line in `src/main.rs` (`MinimapPlugin`), one `pub(crate)` change in `dungeon/mod.rs` (`handle_dungeon_input` visibility), and the `bevy_egui` dep tree in `Cargo.lock`. **+1 dep total.**

## Approach

The research (HIGH for codebase facts, MEDIUM for `bevy_egui` API specifics until Steps A/B/C verify) recommends a sibling `MinimapPlugin` registered alongside `DungeonPlugin` in `main.rs`, owning a `Resource`-shaped `ExploredCells`, drawn with the egui canvas painter (no render-to-texture for #10). The architecture decision baked in: the minimap is a parallel data + view concern that subscribes to `MovedEvent` but does not modify the dungeon's authoritative state — it lives next to `DungeonPlugin`, not inside it, so its lifecycle, OnEnter/OnExit timing, and Feature #23 save integration evolve independently.

The load-bearing risk is Δ deps = +1: this is the first non-trivial dependency added since Feature #5 (`leafwing-input-manager`). Steps A/B/C mirror the verification gate that Features #3 and #5 used — `cargo add --dry-run` resolves the actual Bevy 0.18-compatible version (training data MEDIUM, says `0.39.x`); Step B reads the resolved crate's `[features]` block to decide on `default-features = false`; Step C greps the resolved source for `EguiPlugin`/`EguiContexts`/`ctx_mut()` shape (versions in the 0.27→0.39 range have meaningfully different APIs). All three gates are upstream of every code edit; failure on Step A halts the pipeline and escalates to user.

After the gates pass, implementation is mechanical: define `ExploredCells` (HashMap-keyed by `(floor, x, y)`) + `ExploredState` enum (Unseen/Visited/KnownByOther — last variant declared but no producer in v1, per Decision 7), wire `update_explored_on_move` (a `MessageReader<MovedEvent>` subscriber gated on `GameState::Dungeon`, ordered `.after(handle_dungeon_input)` per Pitfall 3, with a dark-zone skip per Pitfall 8), and write two painter systems (overlay for Exploring, full-screen for Map) that share a `paint_floor_into` helper. Open/close handler reads `Res<ActionState<DungeonAction>>` for `OpenMap`/`Pause` and toggles `DungeonSubState`. All systems registered through a `MinimapSet` SystemSet so painters strictly run after the updater (Pitfall 10).

Tests follow the Layer 2 pattern from #7/#8/#9: pure helpers (Layer 1) for cell-rect math, App-driven with direct `Messages<MovedEvent>::write(...)` injection (Layer 2) for the subscriber, App-driven with full leafwing chain (Layer 2b) for the open/close handler. The painter is gated on render context and is deferred to manual smoke per the audio precedent.

## Critical

- **+1 Cargo dep only.** The `Cargo.toml` diff is exactly one line for `bevy_egui` (with the resolved version + Step-B-decided feature set). No other deps. No bevy version drift. No unrelated transitive bumps. `Cargo.lock` adds the `bevy_egui` tree (egui, epaint, ecolor, emath, ahash, possibly clipboard libs) — Step C quantifies the expected entries before edit. If Cargo.lock shows an unrelated change (e.g., a `bevy_render` patch bump from upward unification), STOP and investigate before continuing.

- **Steps A/B/C are upstream of EVERY code edit.** Do NOT `git add Cargo.toml`, do NOT create `minimap.rs`, do NOT touch `main.rs` until all three gates have passed and their resolutions are recorded inline in this plan's Open Questions section. The cost of running Steps A/B/C is ~15 minutes; the cost of debugging a wrong-version `bevy_egui` against Bevy 0.18.1 is unknown. Same playbook as Feature #5 Step A.

- **`MovedEvent` derives `Message`, NOT `Event`.** Verified at `src/plugins/dungeon/mod.rs:192–197`. Subscribers use `MessageReader<MovedEvent>`, NOT `EventReader`. Mixing `EventReader` compiles fine but silently reads no messages — same trap that bit Feature #2 originally. Never use `EventReader` in this feature.

- **`src/data/dungeon.rs` is FROZEN.** The schema-extension exception was used by Features #8 and #9; Feature #10 uses the existing public API only (`DungeonFloor::{width, height, walls, features, floor_number}`, `CellFeatures::dark_zone`). NO new fields, NO new types in `data/dungeon.rs`. If the implementer feels they need a schema change, STOP and surface as a question.

- **Plugin sibling pattern.** `MinimapPlugin` is a sibling of `DungeonPlugin` in `main.rs` add_plugins(...). It is NOT nested inside `DungeonPlugin::build`. It is NOT in `src/plugins/ui/`. (Decisions 1, 2, 3 lock this — confirm with user before implementer dispatch.)

- **`ExploredCells` does NOT reset on `OnExit(GameState::Dungeon)`.** Players returning from Town to a previously-explored floor expect their map intact; F9 dev cycler is a state cycler, not a "new game" trigger. Reset (when needed) is a Feature #23 / new-game concern. Any system that resets `ExploredCells` in #10 is wrong.

- **System ordering: updater BEFORE painters.** Pitfall 10. `update_explored_on_move` must run `.after(handle_dungeon_input)` (Pitfall 3) AND `.before` both painter systems. Use a `MinimapSet` SystemSet so both painters share an ordering edge against the updater — fewer brittle direct `.before(...)` calls. Both painters can run in parallel relative to each other (they read disjoint state contexts).

- **`handle_dungeon_input` MUST become `pub(crate)`.** Currently a private free fn in `src/plugins/dungeon/mod.rs`. Without this exposure, `update_explored_on_move` cannot reference it for `.after(...)` ordering. The visibility change is the only modification to `dungeon/mod.rs` (besides the test-count update, if any). Document the cross-module ordering coupling in a doc-comment so a future contributor doesn't make `handle_dungeon_input` private again.

- **Dark-zone gate is mandatory.** When a `MovedEvent.to.x/y` lands on a `CellFeatures.dark_zone == true` cell, `update_explored_on_move` skips the insert. The painter then renders `?` for that cell regardless of its `ExploredState` (which remains `Unseen` because no insert happened). This is the only Pitfall-8-correct UX: silent "no map update" + visible `?` glyph in the rendered cell.

- **`show_full` toggle is `#[cfg(feature = "dev")]`-gated.** Per research §Security: a runtime cheat-mode flag visible in shipping builds is a save-data injection / dark-zone-bypass concern. The `show_full` field on `ExploredCells` and the `toggle_show_full_map` system MUST both be `#[cfg(feature = "dev")]`-gated. The painter's read of `explored.show_full` is also gated (or the field's absence in non-dev builds makes the painter behave as if `show_full == false`).

- **bevy_egui + dev `Camera2d` coexistence is expected to work but flag for manual smoke.** Pitfall 9. Druum's `--features dev` `spawn_debug_grid_hud` (mod.rs:781) spawns a `Camera2d { order: 1 }` for the dev HUD. `bevy_egui` renders to its own pass and does NOT typically need an explicit Camera2d — but verify in Step C, and include a manual smoke item in Verification: with `--features dev`, F9 to Dungeon, both the dev HUD text AND the egui minimap overlay must render simultaneously without flicker or z-fighting.

- **Test count baseline.** User brief gives baseline as 68 lib + 3 integration default / 69 lib + 3 integration with `--features dev`. Memory says #9 ended at 69 lib + 3 integration. The implementer MUST `grep -c '#\[test\]' src/**/*.rs` before starting (Step 0) and use the actual count as the baseline. Expected delta after #10: ~+8 lib tests (Layer 1 ~3, Layer 2 ~5, Layer 2b ~3, but some overlap; aim for ~8 net new), 0 integration tests. Final verification asserts the new total equals `baseline + 8` (allow ±1 if a test gets factored into a helper).

- **All 7 verification commands MUST pass with ZERO warnings:** `cargo check`, `cargo check --features dev`, `cargo clippy --all-targets -- -D warnings`, `cargo clippy --all-targets --features dev -- -D warnings`, `cargo test`, `cargo test --features dev`, `cargo fmt --check`. `git diff --stat` final check must show ONLY: `Cargo.toml` (+1 line), `Cargo.lock` (egui dep tree), `src/plugins/dungeon/minimap.rs` (new), `src/main.rs` (+1 line), `src/plugins/dungeon/mod.rs` (one `pub(crate)` change). NO other files touched.

- **Atomic commits.** Suggested boundaries: (1) Cargo.toml + Cargo.lock — `bevy_egui` dep added, code unchanged; (2) `EguiPlugin` registered in main.rs, code compiles, no MinimapPlugin yet; (3) `minimap.rs` skeleton — `MinimapPlugin`, `ExploredCells`, `ExploredState`, plugin registration, no painter systems yet; (4) `handle_dungeon_input` → `pub(crate)`; (5) `update_explored_on_move` + Layer 2 tests; (6) `paint_minimap_overlay` + `paint_minimap_full` + shared `paint_floor_into` helper; (7) `handle_map_open_close` + Layer 2b tests; (8) `#[cfg(feature = "dev")] show_full` toggle. Each commit should compile and `cargo test` should pass.

- **Manual visual smoke is REQUIRED before declaring done.** The whole point of the feature is "the player can see where they've been." Run `cargo run --features dev`, F9 to Dungeon, walk a few cells, press `M`. Map shows visited cells. Walk on a `dark_zone: true` cell (note: floor_01 currently has none — author one ad-hoc OR test via `show_full`). Cell shows `?`. Press `M` again or Escape — back to exploring; overlay visible top-right. F9 cycle out and back in: previously-explored cells still marked. Record findings in **Implementation Discoveries**.

## Steps

> **Pre-pipeline action (run BEFORE branching):**
>
> ```bash
> git fetch origin
> git log main..origin/main --oneline   # expect at least the PR #9 merge
> git checkout main
> git pull origin main                   # local main is behind GitHub main per orchestrator note
> git checkout -b ja-feature-10-auto-map-minimap
> ```
>
> Branching for #10 must happen from up-to-date main. PR #9 was merged on GitHub but local main is behind — verify the fetch shows the merge before pulling.

### Step 0: Baseline measurement

- [ ] `grep -c '#\[test\]' src/**/*.rs` — record actual current lib test count. User brief says 68 default / 69 dev; memory says 69 from Feature #9. Use the grep number as the real baseline; record below as `BASELINE_LIB`.
- [ ] `cargo test 2>&1 | tail -20` and `cargo test --features dev 2>&1 | tail -20` — record actual pre-Feature-10 totals. Expect `BASELINE_LIB` lib tests + 3 integration tests for both feature sets (the difference between default and dev is which `#[cfg(feature = "dev")] #[test]` tests get compiled in).
- [ ] Record `BASELINE_LIB` (e.g., `68` or `69`) and the integration count (`3`) in this plan's Implementation Discoveries section. Final Verification asserts `BASELINE_LIB + ~8` and `3` respectively.

### Step A: Resolve `bevy_egui` version against Bevy 0.18.1 (HALT GATE)

- [ ] Run: `cargo add bevy_egui --dry-run 2>&1 | tee /tmp/bevy-egui-resolve.txt`. Do NOT commit any change.
- [ ] Inspect `/tmp/bevy-egui-resolve.txt`. Look for the line that records the resolved version (e.g., `Adding bevy_egui v0.39.x to dependencies`).
- [ ] Confirm the resolved version's `Cargo.toml` lists `bevy = "0.18"` or `bevy = ">=0.18, <0.19"` as its bevy dep:
  ```bash
  RESOLVED=<paste-resolved-version-here>
  cat ~/.cargo/registry/src/index.crates.io-*/bevy_egui-${RESOLVED}/Cargo.toml | grep -E '^bevy\s*=' | head -3
  ```
- [ ] **HALT condition:** if no version on crates.io accepts `bevy = "0.18.x"`, OR if the dry-run errors with a version-resolution failure, STOP. Do NOT proceed to Step B. Escalate to user with: (a) wait for upstream release, (b) consider a community fork, (c) reconsider `bevy_egui` (low — no good alternative per research §Alternatives Considered).
- [ ] Record the resolved version in this plan's Open Questions section under "OQ-1 RESOLVED".

### Step B: Audit `bevy_egui`'s default features (`default-features = false` decision)

- [ ] After Step A confirms a version (e.g., `0.39.0`), read the resolved crate's `[features]` block:
  ```bash
  RESOLVED=<resolved-version>
  cat ~/.cargo/registry/src/index.crates.io-*/bevy_egui-${RESOLVED}/Cargo.toml | sed -n '/^\[features\]/,/^\[/p'
  ```
- [ ] List which features are in `default = [...]`. Common candidates research warns about: `serde` (theme persistence — Druum doesn't need), `accesskit` (screen reader — out of scope for #10), `manage_clipboard` (Druum doesn't need), `winit/x11`/`winit/wayland` (platform-specific; Druum is macOS-dev today).
- [ ] **Decision branch:**
  - If `default = ["render"]` only (or some equally minimal set), KEEP defaults: `bevy_egui = "=<RESOLVED>"`. Record D8 = Option A.
  - If defaults pull anything Druum doesn't use, opt out: `bevy_egui = { version = "=<RESOLVED>", default-features = false, features = ["render"] }` (or whatever the minimal-render feature is named in this version). Record D8 = Option B with the explicit feature list.
- [ ] Record the chosen line verbatim (the actual text that will go into Cargo.toml in Step 1) in this plan's Open Questions section under "OQ-2 RESOLVED" / "D8 RESOLVED".

### Step C: Verify `bevy_egui` API shape (`EguiPlugin`, `EguiContexts`, painter)

- [ ] Set the registry path:
  ```bash
  RESOLVED=<resolved-version>
  REG=~/.cargo/registry/src/index.crates.io-*/bevy_egui-${RESOLVED}/src
  ```
- [ ] Resolve `EguiPlugin` shape (Pitfall 7):
  ```bash
  grep -rn "pub struct EguiPlugin" $REG | head -3
  grep -A 5 "pub struct EguiPlugin" $REG/lib.rs $REG/plugin.rs 2>/dev/null
  ```
  Record whether it's a unit struct or has fields like `enable_multipass_for_primary_context: bool`. Document the constructor shape used in main.rs Step 2 (e.g., `EguiPlugin` vs `EguiPlugin { enable_multipass_for_primary_context: false }`).
- [ ] Resolve `EguiContexts` system param shape (Pitfall 6):
  ```bash
  grep -rn "pub struct EguiContexts" $REG | head -3
  grep -A 10 "impl.*EguiContexts" $REG/lib.rs 2>/dev/null
  grep -rn "pub fn ctx_mut" $REG | head -3
  ```
  Record whether `ctx_mut()` returns `&mut egui::Context` directly or `Result<&mut egui::Context, ...>`. The painter system pattern adapts: `let ctx = contexts.ctx_mut();` for direct, `let Ok(ctx) = contexts.ctx_mut() else { return };` for Result.
- [ ] Quantify expected `Cargo.lock` additions for the diff review (cleanest-ship signal #2):
  ```bash
  grep -E "^name|^version" ~/.cargo/registry/src/index.crates.io-*/bevy_egui-${RESOLVED}/Cargo.toml | head -50
  ```
  Record expected new Cargo.lock entries (e.g., `bevy_egui`, `egui`, `epaint`, `ecolor`, `emath`, `ahash`, possibly clipboard libs). Final Verification compares actual `git diff Cargo.lock` against this list.
- [ ] Look for relevant examples in the resolved crate to confirm painter API patterns:
  ```bash
  ls ~/.cargo/registry/src/index.crates.io-*/bevy_egui-${RESOLVED}/examples/ 2>/dev/null
  ```
- [ ] Record findings in this plan's Open Questions section under "OQ-3 RESOLVED" and "OQ-4 RESOLVED".

### Step 1: Add `bevy_egui` to `Cargo.toml`

- [ ] In `Cargo.toml`, after the `leafwing-input-manager` line (~line 27), add the line resolved by Step B (verbatim — either `bevy_egui = "=<RESOLVED>"` for Option A or the `default-features = false, features = [...]` form for Option B).
- [ ] Run `cargo check`. Expect a successful compile (no consumers yet). If `cargo check` errors with a version conflict, STOP — Step A's resolution may have been incomplete; investigate.
- [ ] `git diff --stat Cargo.toml Cargo.lock` — verify `Cargo.toml` shows `+1` line, `Cargo.lock` shows the bevy_egui tree from Step C's expected list. If Cargo.lock has unrelated entries (e.g., a `bevy_render` patch bump), STOP and investigate.
- [ ] Commit boundary 1: "deps: add bevy_egui =<RESOLVED> for Feature #10 minimap" (plus Cargo.lock).

### Step 2: Register `EguiPlugin` in `src/main.rs`

- [ ] In `src/main.rs`, after `use druum::plugins::{...}` on line 3-7, add `use bevy_egui::EguiPlugin;` (or wherever the path resolved to in Step C).
- [ ] In the `add_plugins((...))` tuple, add the `EguiPlugin` line BEFORE `DungeonPlugin` (so egui is initialized before any plugin that might want a context). Use the constructor shape from Step C — most likely `EguiPlugin { enable_multipass_for_primary_context: false }` (single-pass UI is sufficient for #10), but use whatever Step C revealed.
- [ ] Run `cargo check` and `cargo run --features dev` (manual: F9 to Dungeon, confirm the game still launches and renders normally, no panics from egui). The minimap is not yet wired; this step verifies `EguiPlugin` registration doesn't break anything.
- [ ] Commit boundary 2: "feat(main): register EguiPlugin for Feature #10".

### Step 3: Create `src/plugins/dungeon/minimap.rs` skeleton

- [ ] Create file `src/plugins/dungeon/minimap.rs`. Add the file declaration to `src/plugins/dungeon/mod.rs` (likely just adds `mod minimap;` near the existing `#[cfg(test)] mod tests;` declaration).
- [ ] Wait — the sibling module needs to be exposed for `MinimapPlugin` to be importable from `main.rs`. The convention from Feature #9 (`audio/{mod.rs, bgm.rs, sfx.rs}`) is `pub mod minimap;` in the parent `mod.rs`, then `pub use minimap::MinimapPlugin;` if a re-export is desired, OR import via `crate::plugins::dungeon::minimap::MinimapPlugin` in main.rs. Use whichever matches the audio module's public-export pattern (read `src/plugins/audio/mod.rs` to confirm — likely `pub use bgm::BgmPlugin; pub use sfx::SfxPlugin;` style).
- [ ] In `minimap.rs`, define:
  - `pub struct MinimapPlugin;` with `impl Plugin for MinimapPlugin` (build body initially just `app.init_resource::<ExploredCells>();` — systems wire in later steps).
  - `#[derive(Resource, Default, Debug, Clone)] pub struct ExploredCells { pub cells: HashMap<(u32, u32, u32), ExploredState>, #[cfg(feature = "dev")] pub show_full: bool }`. Use `bevy::utils::HashMap` (re-export of hashbrown — already in dep tree, no new dep).
  - `#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)] pub enum ExploredState { #[default] Unseen, Visited, KnownByOther }`. Doc-comment on `KnownByOther` MUST say "Variant declared in Feature #10 but not produced by any system yet — Features #12/#20 will populate."
  - `#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)] pub struct MinimapSet;` — used for ordering painter systems strictly after the updater (Pitfall 10).
- [ ] In `minimap.rs`, wire imports: `use bevy::prelude::*; use bevy::utils::HashMap; use crate::plugins::dungeon::MovedEvent; use crate::plugins::dungeon::handle_dungeon_input; use crate::plugins::input::DungeonAction; use crate::plugins::loading::DungeonAssets; use crate::plugins::state::{DungeonSubState, GameState};`. (`handle_dungeon_input` import requires Step 4's `pub(crate)` change.)
- [ ] In `src/main.rs`, add `MinimapPlugin` to the `add_plugins((...))` tuple, AFTER `DungeonPlugin` (sibling registration, Decision 1 Option A). The new line goes immediately after line 27 (`DungeonPlugin,`).
- [ ] Run `cargo check --features dev`. Expect the import for `handle_dungeon_input` to fail until Step 4 — that's expected. If the rest of the skeleton has unrelated errors, fix before proceeding.
- [ ] Commit boundary 3 (after Step 4 lands, since the import fails standalone): combine Steps 3 and 4 in one commit "feat(dungeon): minimap.rs skeleton + handle_dungeon_input pub(crate)".

### Step 4: Make `handle_dungeon_input` `pub(crate)` in `src/plugins/dungeon/mod.rs`

- [ ] In `src/plugins/dungeon/mod.rs`, find the `fn handle_dungeon_input(...)` declaration (line ~610). Change `fn` to `pub(crate) fn`.
- [ ] Add a doc-comment line (or extend the existing one) noting: "Visibility is `pub(crate)` so `MinimapPlugin::update_explored_on_move` can `.after(handle_dungeon_input)` for system ordering (Feature #10 Pitfall 3)."
- [ ] Run `cargo check --features dev`. The skeleton in Step 3 should now compile (assuming the rest of the file is valid).
- [ ] `cargo test` — baseline must hold (`BASELINE_LIB + 0`); no new tests yet, no behavior change.
- [ ] Commit boundary 3 (combined with Step 3): "feat(dungeon): minimap.rs skeleton + handle_dungeon_input pub(crate)".

### Step 5: Implement `update_explored_on_move` subscriber + Layer 2 tests

- [ ] In `minimap.rs`, write `fn update_explored_on_move(mut moved: MessageReader<MovedEvent>, floors: Res<Assets<DungeonFloor>>, dungeon_assets: Option<Res<DungeonAssets>>, mut explored: ResMut<ExploredCells>)`. Body matches research §Pattern 2:
  - Early-return if `dungeon_assets` or the floor handle resolves to nothing.
  - For each `MovedEvent`, look up `floor.features[ev.to.y as usize][ev.to.x as usize].dark_zone`. If `true`, `continue` (skip — Pitfall 8 dark-zone gate; map cell remains `Unseen`).
  - Otherwise, `explored.cells.insert((floor.floor_number, ev.to.x, ev.to.y), ExploredState::Visited)`.
- [ ] In `MinimapPlugin::build`, register: `app.add_systems(Update, update_explored_on_move.run_if(in_state(GameState::Dungeon)).after(handle_dungeon_input).in_set(MinimapSet))`. The `.after(handle_dungeon_input)` is mandatory (Pitfall 3); the `MinimapSet` membership lets painters in Steps 6-7 order against the updater (Pitfall 10).
- [ ] In `minimap.rs`, add `#[cfg(test)] mod tests { ... }` block (inline — keep under ~400 LOC per research §Validation Architecture). Test framework: full `MinimalPlugins + StatesPlugin + add_message::<MovedEvent>() + MinimapPlugin` (no `DungeonPlugin` — by design, the minimap is testable without the dungeon). Use the dev-feature `init_resource::<ButtonInput<KeyCode>>()` bypass per `feedback_dev_feature_buttoninput_in_tests.md`.
- [ ] Layer 1 tests (~3, pure helpers — these become real if cell-rect math gets factored out of the painter; if all rendering math is inlined in `paint_floor_into`, Layer 1 tests may be 0):
  - `cell_rect_for_origin_and_size_returns_expected_position` — verify (cell_size, x, y, origin) → expected `egui::Rect`.
  - `cell_rect_handles_zero_origin` — degenerate-case math.
  - `floor_no_zero_keys_distinct_from_floor_no_one` — verify HashMap key tuple ordering.
- [ ] Layer 2 tests (~5, App + minimap, no leafwing — write `MovedEvent`s directly via `app.world_mut().resource_mut::<Messages<MovedEvent>>().write(MovedEvent { ... })`):
  - `subscriber_flips_dest_cell_to_visited` — write a `MovedEvent { from: (0,0), to: (1,0), facing: East }`, `app.update()`, assert `explored.cells.get(&(0, 1, 0)) == Some(ExploredState::Visited)`.
  - `subscriber_skips_dark_zone_cells` — author a tiny test floor with `features[0][1].dark_zone = true`, write a `MovedEvent` to (1,0), assert the entry is NOT inserted.
  - `subscriber_does_not_touch_other_cells` — write one `MovedEvent`, assert only the destination key changed.
  - `plugin_registers_explored_cells` — smoke test that `app.world().contains_resource::<ExploredCells>()`.
  - `explored_state_default_is_unseen` — pure `ExploredState::default() == ExploredState::Unseen` test.
  - `known_by_other_variant_declared` — smoke test that `ExploredState::KnownByOther` exists (compile-time check via `let _: ExploredState = ExploredState::KnownByOther;`).
- [ ] If `#[cfg(feature = "dev")]`: also test `subscriber_with_show_full_does_not_mutate_cells_directly` — set `explored.show_full = true`, write a `MovedEvent` to a non-dark-zone cell, assert `explored.cells.get(&(...)) == Some(Visited)` (subscriber still inserts; `show_full` only affects rendering, not data).
- [ ] Run `cargo test --features dev`. Expect baseline + ~5-8 tests passing. Run `cargo test` (default) — same delta minus any dev-only tests.
- [ ] Commit boundary 4: "feat(minimap): MovedEvent subscriber + dark-zone gate + Layer 2 tests".

### Step 6: Implement `paint_minimap_overlay` (top-right 200×200) and `paint_minimap_full` (CentralPanel) — shared helper

- [ ] In `minimap.rs`, write the painter systems following research §Pattern 3. Use the `EguiContexts` API shape Step C resolved (e.g., `let Ok(ctx) = contexts.ctx_mut() else { return };` if it returns `Result`, or `let ctx = contexts.ctx_mut();` if it returns `&mut Context` directly).
  - `fn paint_minimap_overlay(mut contexts: EguiContexts, explored: Res<ExploredCells>, floors: Res<Assets<DungeonFloor>>, dungeon_assets: Option<Res<DungeonAssets>>, party: Query<(&GridPosition, &Facing), With<PlayerParty>>)` — `egui::Window::new("minimap").anchor(egui::Align2::RIGHT_TOP, [-10.0, 10.0]).fixed_size([200.0, 200.0]).frame(...).title_bar(false).resizable(false).show(ctx, |ui| paint_floor_into(...))`.
  - `fn paint_minimap_full(...)` — same args; uses `egui::CentralPanel::default().frame(egui::Frame::none().fill(egui::Color32::from_rgb(20, 20, 30))).show(ctx, |ui| paint_floor_into(...))`.
  - `fn paint_floor_into(painter: &egui::Painter, rect: egui::Rect, floor: &DungeonFloor, explored: &ExploredCells, pos: GridPosition, facing: Direction)` — shared body. Per research §Pattern 3:
    - Compute `cell_size = (rect.width() / floor.width as f32).min(rect.height() / floor.height as f32)`.
    - For each `(x, y)` in `0..width × 0..height`:
      - Determine effective state: `if cfg!(feature = "dev") && explored.show_full { ExploredState::Visited } else { explored.cells.get(&(floor.floor_number, x, y)).copied().unwrap_or(ExploredState::Unseen) }`.
      - Shade per state: `Unseen → Color32::TRANSPARENT`, `Visited → Color32::from_rgb(60, 60, 70)`, `KnownByOther → Color32::from_rgb(50, 50, 100)` (slight blue tint to distinguish from `Visited` so dev `show_full` looks different from real exploration).
      - `painter.rect_filled(cell_rect, 0.0, shade)`.
      - If `floor.features[y][x].dark_zone`, draw `?` glyph centered in cell with `painter.text(...)`.
      - Walls: read `floor.walls[y][x]`; draw the `north` and `west` edges (mirroring the dedup rule from `spawn_dungeon_geometry`); also draw `south` if `y == floor.height - 1` and `east` if `x == floor.width - 1`. Use a free helper `paint_wall_if_visible(painter, cell_rect, wall_type, side)` so wall-color logic doesn't drift between full/overlay.
    - Player arrow: compute `(pos.x + 0.5) * cell_size`, `(pos.y + 0.5) * cell_size`; draw a small triangle pointing in `facing` direction (use `painter.add(egui::Shape::convex_polygon(...))` with 3 points rotated by facing).
- [ ] In `MinimapPlugin::build`, register: `app.add_systems(Update, (paint_minimap_overlay.run_if(in_state(DungeonSubState::Exploring)), paint_minimap_full.run_if(in_state(DungeonSubState::Map))).in_set(MinimapSet).after(update_explored_on_move))`. Both painters share `MinimapSet` membership and the explicit `.after(update_explored_on_move)` ordering (Pitfall 10).
- [ ] No automated tests for the painter — it requires a render context. Manual smoke is the test (Verification step).
- [ ] `cargo check --features dev` and `cargo run --features dev` (manual: F9 to Dungeon; walk a few cells; confirm no panic). The painter's correctness is verified in the final manual smoke.
- [ ] Commit boundary 5: "feat(minimap): paint_minimap_overlay + paint_minimap_full + paint_floor_into helper".

### Step 7: Implement `handle_map_open_close` + Layer 2b tests

- [ ] In `minimap.rs`, write `fn handle_map_open_close(actions: Res<ActionState<DungeonAction>>, current: Res<State<DungeonSubState>>, mut next: ResMut<NextState<DungeonSubState>>)`. Body per research §Pattern 4:
  ```rust
  match current.get() {
      DungeonSubState::Exploring if actions.just_pressed(&DungeonAction::OpenMap) => {
          next.set(DungeonSubState::Map);
      }
      DungeonSubState::Map
          if actions.just_pressed(&DungeonAction::OpenMap)
              || actions.just_pressed(&DungeonAction::Pause) =>
      {
          next.set(DungeonSubState::Exploring);
      }
      _ => {}
  }
  ```
- [ ] Register in `MinimapPlugin::build`: `app.add_systems(Update, handle_map_open_close.run_if(in_state(GameState::Dungeon)))`. No `MinimapSet` membership needed — the open/close handler doesn't read or write `ExploredCells`.
- [ ] Layer 2b tests (~3, App + full leafwing chain — `MinimalPlugins + StatesPlugin + InputPlugin + ActionsPlugin + MinimapPlugin + add_message::<MovedEvent>()`). Use the same pattern as `src/plugins/input/mod.rs::tests::dungeon_w_press_triggers_move_forward` (KeyCode press → `app.update()` → assert on state). Need to also force `GameState::Dungeon` and a `DungeonSubState` initial value:
  - `open_map_action_transitions_substate` — start in `DungeonSubState::Exploring`, press `KeyCode::KeyM`, `app.update()` twice (one for input chain, one for state transition), assert `DungeonSubState::Map`.
  - `open_map_action_toggles_back` — start in `DungeonSubState::Map`, press `KeyCode::KeyM`, two updates, assert `DungeonSubState::Exploring`.
  - `pause_action_exits_map_substate` — start in `DungeonSubState::Map`, press `KeyCode::Escape`, two updates, assert `DungeonSubState::Exploring`.
- [ ] `cargo test --features dev` — assert baseline + ~5-8 + 3 = `BASELINE_LIB + ~8` tests pass. Adjust if the actual count differs (some Layer 1 tests may not exist if helpers stayed inlined).
- [ ] Commit boundary 6: "feat(minimap): handle_map_open_close + Layer 2b tests".

### Step 8: Add `#[cfg(feature = "dev")] show_full` debug toggle

- [ ] In `minimap.rs`, write `#[cfg(feature = "dev")] fn toggle_show_full_map(keys: Res<ButtonInput<KeyCode>>, mut explored: ResMut<ExploredCells>) { if keys.just_pressed(KeyCode::F8) { explored.show_full = !explored.show_full; info!("Minimap show_full toggled to {}", explored.show_full); } }`. Choose F8 (or another unused dev key — F9 is reserved for state cycling). Document the chosen key in a doc-comment.
- [ ] Register in `MinimapPlugin::build` under `#[cfg(feature = "dev")]`: `#[cfg(feature = "dev")] app.add_systems(Update, toggle_show_full_map.run_if(in_state(GameState::Dungeon)));`. Symmetric gating: definition AND registration both behind `#[cfg(feature = "dev")]` (matches Druum's established pattern, e.g., `cycle_game_state_on_f9`).
- [ ] Optional Layer 2 dev-only test: `#[cfg(feature = "dev")] #[test] fn show_full_toggle_flips_field` — press F8 via `init_resource::<ButtonInput<KeyCode>>()` bypass + manual `.press(KeyCode::F8)`, `app.update()`, assert `explored.show_full == true`. Note: this requires the dev-feature ButtonInput bypass per the test memory; do NOT add `InputPlugin`.
- [ ] `cargo test --features dev` and `cargo test` — both must pass. Default-build `cargo test` will compile out the `show_full` field, the toggle system, and any dev-only test; verify with `grep '#\[cfg(feature = "dev")' src/plugins/dungeon/minimap.rs` that gating is symmetric.
- [ ] Commit boundary 7: "feat(minimap): #[cfg(feature = \"dev\")] show_full debug toggle".

### Step 9: Final verification + diff review

- [ ] Run all 7 verification commands listed in Verification section. ALL must pass with zero warnings, zero formatting diff.
- [ ] `git diff --stat HEAD` (or against `main` if branch hasn't been merged yet). Verify the file list matches expected:
  - `Cargo.toml` (+1 line for `bevy_egui`)
  - `Cargo.lock` (the bevy_egui dep tree quantified in Step C)
  - `src/plugins/dungeon/minimap.rs` (new file, ~250-400 LOC including tests)
  - `src/plugins/dungeon/mod.rs` (`pub mod minimap;` declaration + `pub(crate)` change on `handle_dungeon_input`)
  - `src/main.rs` (+1 line for `MinimapPlugin`, +1 line for `EguiPlugin`, +1 use line)
  - NO other files touched. If `state/`, `input/`, `loading/`, `audio/`, `data/`, `combat/`, `town/`, `party/`, `save/`, `ui/` show in the diff, STOP and review.
- [ ] Manual smoke checklist (deferred to user for sign-off — record outcomes in Implementation Discoveries):
  - [ ] `cargo run --features dev`. Game launches without panic.
  - [ ] F9 to Dungeon. Walk a few cells (W/A/S/D). No panic.
  - [ ] Top-right 200×200 minimap overlay visible in `Exploring` substate, shows visited cells in dark grey.
  - [ ] Player position arrow visible on the overlay, points in the current facing direction.
  - [ ] Press M. Full-screen map appears. Same data, larger render.
  - [ ] Press M again. Returns to `Exploring` substate. Overlay re-visible.
  - [ ] Open map again with M, then press Escape. Returns to `Exploring`.
  - [ ] If floor_01 has a `dark_zone: true` cell (note: it does not at HEAD; either author one ad-hoc OR test via `show_full` toggle): walk on it, verify cell shows `?`, verify the cell is NOT marked `Visited` in the explored data.
  - [ ] F8 (dev-only): toggles `show_full`. Map fills with all cells as `Visited`. F8 again to disable.
  - [ ] F9 cycle: Dungeon → Combat → GameOver → Loading → TitleScreen → Town → Dungeon. On the second Dungeon entry, previously-explored cells are STILL marked (Resource not reset on `OnExit(GameState::Dungeon)`).
  - [ ] No visible flicker or z-fighting between the dev `Camera2d` HUD (top-left position text) and the egui minimap overlay (top-right). Both render simultaneously without conflict (Pitfall 9 manual verification).
- [ ] Update Implementation Discoveries section of this plan with: actual test counts, any unexpected egui API quirks discovered in implementation, any feature-flag changes Steps A/B/C revealed, any deviation from the planned approach.
- [ ] Commit boundary 8 (final): "docs(minimap): plan Implementation Discoveries + manual smoke results".

## Security

### Known Vulnerabilities

No CVEs found for `bevy_egui` or `egui` family as of 2026-05-04 (research §Security). However, the resolved version comes from Step A — re-verify post-add:

- [ ] Run `cargo audit` after Step 1. Expect zero advisories for `bevy_egui` and its transitive deps. If any appear, STOP and surface to user with the advisory text and severity.

### Architectural Risks

| Risk | How It Manifests | Secure Pattern in #10 |
|------|------------------|------------------------|
| Save-data injection (Feature #23 future) | Crafted save inserts billions of `ExploredCells` entries | Bound `cells.len()` before deserializing (Feature #23 concern; #10 just notes the field will need a max-entries gate). |
| `dark_zone` bypass via `show_full` flag | Cheat-mode flag visible in shipping builds bypasses the gate | `show_full` and `toggle_show_full_map` are BOTH `#[cfg(feature = "dev")]`-gated. The painter's branch on `cfg!(feature = "dev") && explored.show_full` evaluates to `false` in non-dev builds (the field doesn't exist there). |
| Painter reads stale `Assets<DungeonFloor>` during reload | Stale geometry disagreeing with the live render | Guard with `Option` early-return (matches existing pattern in `dungeon/mod.rs:622-631`). Painters use `let Some(...) else { return };` for both `dungeon_assets` and `floors.get(...)`. |

### Trust Boundaries

- **`MovedEvent` input boundary:** Already validated by `handle_dungeon_input` (bounds-checked against `floor.width`/`floor.height` at lines 649-655). No further validation needed for `ev.to.x`/`y` in the subscriber. The `(floor_number, x, y)` HashMap key is composed of u32s; no overflow risk at floor scale.
- **`DungeonFloor` asset boundary:** Read-only. Painter uses `floor.features[y][x].dark_zone` and `floor.walls[y][x]`. Both rely on Feature #4's `is_well_formed()` invariant — out-of-bounds reads here are an asset-validation failure, not #10's responsibility. The `Option` early-return on `floors.get(...)` covers the asset-reload race.
- **No new boundaries introduced.** #10 reads existing data + writes one new Resource. Save-game serialization (#23 boundary) is the next concern.

## Open Questions

The following decisions REQUIRE USER INPUT before the implementer is dispatched. The research recommendation is presented as the default — confirm or override.

> **Per memory `feedback_user_answers_to_options.md`:** if your answer is prose like "just X for now", I will ASK ONE CLARIFYING QUESTION before treating it as Option A. Please answer with the option letter (A/B/C) explicitly to avoid round-trips.

### D1 — Plugin module structure

- **A (recommended):** Sibling `MinimapPlugin` registered in `main.rs` alongside `DungeonPlugin`. New file `src/plugins/dungeon/minimap.rs`. Two `add_plugins` lines.
- **B:** Nested — `app.add_plugins(MinimapPlugin)` from inside `DungeonPlugin::build`. main.rs unchanged.
- **C:** Move minimap to `src/plugins/ui/minimap.rs`, register through `UiPlugin`.

**Research argues for A:** (a) the map is data + view, not state-mutation — keeping it parallel to the dungeon avoids accidental coupling in future features; (b) lowest-risk for Feature #23 save integration. Choose:

### D2 — Where `ExploredCells` lives

- **A (recommended):** `Resource` (`app.init_resource::<ExploredCells>()`). HashMap keyed by `(floor, x, y)`.
- **B:** `Component` on `PlayerParty` entity.
- **C:** Lazy-built from a `Vec<MovedEvent>` history.

**Research argues for A:** (a) `(floor, x, y)` cross-floor key contradicts Option B (PlayerParty despawns on `OnExit(Dungeon)` per `mod.rs:412-419`, taking the data with it); (b) Option C is O(history) on every map open — solves a problem we don't have. Choose:

### D3 — Canvas vs render-to-texture

- **A (recommended for #10):** egui canvas painter direct draw every frame.
- **B:** Render-to-texture once per `ExploredCells` change.

**Research argues for A:** floor_01 at 6×6 (~144 segments) is two orders of magnitude under egui's threshold. RTT is a Feature #11+ concern when floor sizes approach 30×30+. Master research §Open Question 5 endorses "start with egui canvas, upgrade if needed." Choose:

### D4 — Minimap overlay placement + size

- **A (recommended):** Top-right anchored, fixed 200×200, translucent frame, no background fill.
- **B:** Bottom-right or bottom-left.
- **C:** Toggleable corner (cycle full/overlay/hidden).

**Research argues for A:** (a) doesn't conflict with the dev grid HUD which sits in top-LEFT (`mod.rs:800-806`); (b) matches Etrian Odyssey/Wizardry remake convention; (c) Option C is settings-UI work (Feature #25 polish). Recommended constants `MINIMAP_OVERLAY_SIZE: f32 = 200.0`, `MINIMAP_OVERLAY_PAD: f32 = 10.0`, marked `pub(crate) const` so #25 can tune. Choose:

### D5 — `OpenMap` toggle behavior

- **A (recommended):** M toggles between Exploring and Map; Escape (`Pause` action) also exits Map.
- **B:** M only toggles. Escape does nothing in Map (or routes to a Paused substate, undefined for #10).
- **C:** M opens; M while open is a no-op; only Escape closes.

**Research argues for A:** standard genre convention; one extra `OR` in the input handler. Players' muscle memory expects Escape to "back out." Choose:

### D6 — Visited semantics on move

- **A (recommended for v1):** Mark only the destination cell as `Visited`.
- **B:** Mark destination + all line-of-sight adjacent cells (defer to #25 polish per research recommendation).

**Research argues for A:** Option B requires a line-of-sight + door-aware traversal algorithm (non-trivial); Option A satisfies the spec's "every MovedEvent" reading; the visualization difference is small at floor_01 scale. Choose:

### D7 — `KnownByOther` provenance

- **A (recommended):** Declare the variant; render it slightly tinted (e.g. `Color32::from_rgb(50, 50, 100)`) so dev `show_full` is distinguishable from real exploration. No producer in v1; doc-comment names #12 / #20 as future producers.
- **B:** Don't add the variant until #12 / #20 lands.
- **C:** Define a richer enum upfront (`Visited { steps: u32 }`, `KnownByOther { source: RevealSource }`).

**Research argues for A:** zero scope creep, exhaustive match keeps future writers honest, no "unused variant" lint because the painter renders it. Option B forces #12/#20 to add the variant AND update every match site (where #10 already had one). Option C is YAGNI. Choose:

### D8 — `bevy_egui` features opt-out (RESOLVED BY STEP B)

This decision is NOT user-facing — it's resolved by the Step B feature audit. The plan structure:

- **A:** Keep `bevy_egui` defaults. Cargo.toml line: `bevy_egui = "=<RESOLVED>"`.
- **B:** Opt out: `bevy_egui = { version = "=<RESOLVED>", default-features = false, features = ["render", ...] }` with the explicit minimal feature list.

**Resolution path:** Step B reads the resolved crate's `[features]` block. If `default = ["render"]` only, choose A. If defaults pull `accesskit` / `serde` / `winit/x11` chains that Druum doesn't need, choose B with the minimal feature list. The implementer does NOT need user input on D8 — Step B's audit determines the answer. Record the resolution inline in this plan after Step B runs.

### Resolved-during-research questions (no user action needed)

- **OQ-1 (`bevy_egui` resolved version):** RESOLVED BY STEP A (records the resolved version inline in this plan). HALT GATE if no compatible version exists.
- **OQ-2 (default features audit):** RESOLVED BY STEP B (D8 above).
- **OQ-3 (`EguiPlugin` config shape):** RESOLVED BY STEP C (records the constructor pattern inline; either unit struct or `EguiPlugin { enable_multipass_for_primary_context: false }`).
- **OQ-4 (`EguiContexts` `ctx_mut()` return shape):** RESOLVED BY STEP C (records whether painter systems use direct deref or `Result` early-return).
- **OQ-5 (Reset `ExploredCells` on dungeon-exit?):** RESOLVED — preserve. Documented in `Critical` section above. F9 dev cycle preserves explored cells. Reset on full new-game / Feature #23 only.

## Implementation Discoveries

[Empty — populate during implementation with: actual test counts; Step A/B/C resolutions; any egui API surprise discovered in implementation; any deviation from the planned approach; manual smoke results.]

### Recorded resolutions (fill in during Steps A/B/C)

- **Step A:** Resolved `bevy_egui` version: `<TBD>`. Bevy 0.18.x compatibility confirmed: `<yes/no>`. If no, escalation outcome: `<TBD>`.
- **Step B:** Cargo.toml line chosen: `<verbatim line>`. Default features audited: `<list>`. Decision (A or B): `<TBD>`.
- **Step C:** `EguiPlugin` constructor shape: `<TBD>`. `EguiContexts::ctx_mut()` returns: `<&mut Context | Result<&mut Context, _>>`. Expected Cargo.lock additions: `<list of crate names>`.
- **Step 0 baseline:** `BASELINE_LIB = <NN>`, integration tests = 3.
- **Final test counts (post-Step 9):** lib `<NN+~8>`, integration 3.

## Verification

### Pre-pipeline

- [ ] Local main is up to date — `cd /Users/nousunio/Repos/Learnings/claude-code/druum && git fetch origin && git log main..origin/main --oneline` shows zero new commits — Manual

### Step gates

- [ ] Step A resolved `bevy_egui` version is recorded inline in this plan (Implementation Discoveries → Recorded resolutions) AND accepts `bevy = "0.18.x"` — Manual
- [ ] Step B feature audit recorded inline; D8 decision made (A or B with explicit feature list) — Manual
- [ ] Step C `EguiPlugin` shape, `EguiContexts::ctx_mut()` return type, expected Cargo.lock additions all recorded inline — Manual

### Compilation + lint

- [ ] `cargo check` passes — Build — `cargo check` — Automatic
- [ ] `cargo check --features dev` passes — Build — `cargo check --features dev` — Automatic
- [ ] `cargo clippy --all-targets -- -D warnings` zero warnings — Lint — `cargo clippy --all-targets -- -D warnings` — Automatic
- [ ] `cargo clippy --all-targets --features dev -- -D warnings` zero warnings — Lint — `cargo clippy --all-targets --features dev -- -D warnings` — Automatic
- [ ] `cargo fmt --check` zero diff — Format — `cargo fmt --check` — Automatic

### Tests

- [ ] `cargo test` passes with `BASELINE_LIB + ~8` lib tests + 3 integration tests — Test — `cargo test 2>&1 | tail -5` — Automatic
- [ ] `cargo test --features dev` passes with same delta (or +1 for the dev-only `show_full_toggle_flips_field` test if added) — Test — `cargo test --features dev 2>&1 | tail -5` — Automatic
- [ ] No previously-passing tests regress — Test — visual diff against Step 0 baseline — Manual
- [ ] `cargo audit` reports zero advisories for `bevy_egui` and its transitive deps — Security — `cargo audit 2>&1 | grep -i 'bevy_egui\|egui'` — Automatic

### Diff review (cleanest-ship signal)

- [ ] `git diff --stat` shows ONLY: `Cargo.toml` (+1), `Cargo.lock` (egui dep tree per Step C list), `src/plugins/dungeon/minimap.rs` (new), `src/plugins/dungeon/mod.rs` (`pub mod minimap;` + `pub(crate)` on `handle_dungeon_input`), `src/main.rs` (+1 use, +1 EguiPlugin, +1 MinimapPlugin) — Diff — `git diff --stat HEAD` — Manual
- [ ] No unrelated transitive bumps in `Cargo.lock` (e.g., a bevy_render patch bump) — Diff — `git diff Cargo.lock | grep -E '^[-+]name|^[-+]version' | head -50` — Manual
- [ ] No edits to `state/`, `input/`, `loading/`, `audio/`, `data/`, `combat/`, `town/`, `party/`, `save/`, `ui/` — Diff — `git diff --stat HEAD | grep -v 'minimap\|mod.rs\|main.rs\|Cargo'` returns empty — Manual

### Manual smoke (deferred to user for sign-off)

- [ ] `cargo run --features dev` launches without panic — Manual
- [ ] F9 to Dungeon, walk W/A/S/D — minimap overlay visible top-right, cells fill with grey on visit — Manual
- [ ] Press M — full-screen map appears with same data — Manual
- [ ] Press M again — returns to Exploring; overlay visible — Manual
- [ ] Press M, then Escape — returns to Exploring (Pause action exits Map) — Manual
- [ ] Walk on a `dark_zone` cell (author one ad-hoc OR test via `show_full`): cell renders `?`; `ExploredCells` does NOT mark it Visited — Manual
- [ ] F8 toggles `show_full` (dev-only) — map fills with all cells visited — Manual
- [ ] F9 cycle (Dungeon → Combat → ... → Dungeon): previously-explored cells STILL marked on second entry — Manual
- [ ] Dev `Camera2d` HUD (top-left position text) and egui overlay (top-right) coexist without flicker or z-fighting — Manual (Pitfall 9)
- [ ] Implementation Discoveries section of this plan updated with smoke results before declaring done — Manual
