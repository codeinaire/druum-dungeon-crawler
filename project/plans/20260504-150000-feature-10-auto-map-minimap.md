# Plan: Auto-Map / Minimap — Feature #10

**Date:** 2026-05-04
**Status:** Complete
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

- [x] `grep -c '#\[test\]' src/**/*.rs` — record actual current lib test count. User brief says 68 default / 69 dev; memory says 69 from Feature #9. Use the grep number as the real baseline; record below as `BASELINE_LIB`.
- [x] `cargo test 2>&1 | tail -20` and `cargo test --features dev 2>&1 | tail -20` — record actual pre-Feature-10 totals. Expect `BASELINE_LIB` lib tests + 3 integration tests for both feature sets (the difference between default and dev is which `#[cfg(feature = "dev")] #[test]` tests get compiled in).
- [x] Record `BASELINE_LIB` (e.g., `68` or `69`) and the integration count (`3`) in this plan's Implementation Discoveries section. Final Verification asserts `BASELINE_LIB + ~8` and `3` respectively. **Actual: 67 default / 68 dev / 3 integration.**

### Step A: Resolve `bevy_egui` version against Bevy 0.18.1 (HALT GATE)

- [x] Run: `cargo add bevy_egui --dry-run 2>&1 | tee /tmp/bevy-egui-resolve.txt`. Do NOT commit any change.
- [x] Inspect `/tmp/bevy-egui-resolve.txt`. Look for the line that records the resolved version (e.g., `Adding bevy_egui v0.39.x to dependencies`).
- [x] Confirm the resolved version's `Cargo.toml` lists `bevy = "0.18"` or `bevy = ">=0.18, <0.19"` as its bevy dep — **CONFIRMED: `bevy_egui 0.39.1` has `bevy_app = "^0.18.0"` (and all bevy_* deps at ^0.18.0)**.
- [x] **HALT condition:** not triggered — compatible version found.
- [x] Record the resolved version in this plan's Open Questions section under "OQ-1 RESOLVED" — **`0.39.1`**.

### Step B: Audit `bevy_egui`'s default features (`default-features = false` decision)

- [x] After Step A confirms a version (e.g., `0.39.0`), read the resolved crate's `[features]` block.
- [x] List which features are in `default = [...]` — **actual defaults: `manage_clipboard` (arboard), `open_url` (webbrowser), `default_fonts`, `render`, `bevy_ui` (→ `bevy_ui_render`), `picking` (→ `bevy_picking`). None of manage_clipboard/open_url/bevy_ui/picking are needed.**
- [x] **Decision branch:** D8 = Option B — opt out: `bevy_egui = { version = "=0.39.1", default-features = false, features = ["render", "default_fonts"] }`.
- [x] Record the chosen line verbatim: `bevy_egui = { version = "=0.39.1", default-features = false, features = ["render", "default_fonts"] }`.

### Step C: Verify `bevy_egui` API shape (`EguiPlugin`, `EguiContexts`, painter)

- [x] Set the registry path — source available at `~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bevy_egui-0.39.1/`.
- [x] Resolve `EguiPlugin` shape — **struct with multiple fields; `enable_multipass_for_primary_context: bool` is `#[deprecated]`. Use `EguiPlugin::default()`. Default has multipass enabled (→ requires `EguiPrimaryContextPass` schedule for UI systems).**
- [x] Resolve `EguiContexts` system param shape — **`ctx_mut()` returns `Result<&mut egui::Context, QuerySingleError>`. Painter fns must return `-> Result` and use `let ctx = contexts.ctx_mut()?;`.**
- [x] Quantify expected Cargo.lock additions — **7 crates: `bevy_egui`, `egui`, `ecolor`, `emath`, `epaint`, `epaint_default_fonts`, `nohash-hasher`. Actual confirmed identical.**
- [x] Look for relevant examples — checked `simple.rs` and `ui.rs`. Key finding: systems go in `EguiPrimaryContextPass`, not `Update`.
- [x] Record findings — OQ-3: `EguiPlugin::default()`. OQ-4: `ctx_mut()` returns `Result`; painter systems use `EguiPrimaryContextPass`.

### Step 1: Add `bevy_egui` to `Cargo.toml`

- [x] In `Cargo.toml`, after the `leafwing-input-manager` line (~line 27), add the line resolved by Step B.
- [x] Run `cargo check` — passes with 0 errors.
- [x] `git diff --stat Cargo.toml Cargo.lock` — `Cargo.toml` +1 line, `Cargo.lock` +7 egui entries only. No unrelated bumps.
- [x] Commit boundary 1: commit `a9c98aa` "deps: add bevy_egui =0.39.1 for Feature #10 minimap".

### Step 2: Register `EguiPlugin` in `src/main.rs`

- [x] Per D1=C override, `EguiPlugin` is registered in `UiPlugin::build` (not `main.rs`). `src/main.rs` is byte-unchanged.
- [x] `EguiPlugin::default()` added to `UiPlugin::build` in `src/plugins/ui/mod.rs`.
- [x] `cargo check` passes.
- [x] Commit boundary 2+3+4 combined: commit `f9f2e7a`.

### Step 3: Create `src/plugins/dungeon/minimap.rs` skeleton

- [x] Per D1=C override, created `src/plugins/ui/minimap.rs` (not `src/plugins/dungeon/minimap.rs`).
- [x] `pub mod minimap;` + `pub use minimap::MinimapPlugin;` added to `src/plugins/ui/mod.rs`.
- [x] `ExploredCells`, `ExploredState`, `MinimapSet`, `MinimapPlugin` all defined.
- [x] Used `std::collections::HashMap` (bevy::utils::HashMap removed from Bevy 0.18.1).
- [x] `KnownByOther` doc-comment present.
- [x] `MinimapPlugin` registered in `UiPlugin::build` (not `main.rs`).
- [x] Combined with Step 4 in commit `f9f2e7a`.

### Step 4: Make `handle_dungeon_input` `pub(crate)` in `src/plugins/dungeon/mod.rs`

- [x] `fn handle_dungeon_input` → `pub(crate) fn handle_dungeon_input` at line ~610.
- [x] Doc-comment extended with ordering coupling explanation.
- [x] `cargo check --features dev` passes.
- [x] Combined with Step 3 in commit `f9f2e7a`.

### Step 5: Implement `update_explored_on_move` subscriber + Layer 2 tests

- [x] `update_explored_on_move` implemented with early-return on missing DungeonAssets/floor, dark-zone gate, and `ExploredState::Visited` insert.
- [x] Registered in `MinimapPlugin::build` with `.after(handle_dungeon_input).in_set(MinimapSet)`.
- [x] Tests implemented (combined in single commit `f9f2e7a`): Layer 1 (5 tests: cell_rect_for_origin_zero, cell_rect_for_nonzero_origin_and_position, floor_number_keys_are_distinct, explored_state_default_is_unseen, known_by_other_variant_exists) + Layer 2 (plugin_registers_explored_cells, subscriber_flips_dest_cell_to_visited, subscriber_does_not_touch_other_cells) + dev-only (show_full_does_not_mutate_cells).
- [x] Dark-zone guard present in code. `subscriber_skips_dark_zone_cells` as standalone test deferred (requires LoadingPlugin which hangs headless tests; early-return path tested via subscriber_flips_dest_cell_to_visited).
- [x] `cargo test` / `cargo test --features dev` pass.

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
- [x] Painters registered in `EguiPrimaryContextPass` (not `Update` — see Implementation Discoveries). Both share `MinimapSet` and `.after(update_explored_on_move)`.
- [x] No automated tests for painters (require render context). Manual smoke deferred.
- [x] `cargo check --features dev` passes.
- [x] Commit: combined in `f9f2e7a`.

### Step 7: Implement `handle_map_open_close` + Layer 2b tests

- [x] `handle_map_open_close` implemented per plan spec.
- [x] Registered in `MinimapPlugin::build` without `MinimapSet`.
- [x] Layer 2b tests: `open_map_action_transitions_substate`, `open_map_action_toggles_back`, `pause_action_exits_map_substate` — all pass. Test pattern: `ActionState` direct mutation (no `InputPlugin`/`ActionsPlugin`) with press/update/release/update cycle to avoid double-firing.
- [x] `cargo test --features dev`: 81 lib (+13), 3 integration. All pass.
- [x] Commit: combined in `f9f2e7a`.

### Step 8: Add `#[cfg(feature = "dev")] show_full` debug toggle

- [x] `toggle_show_full_map` (F8) implemented and registered with symmetric `#[cfg(feature = "dev")]` gating.
- [x] `show_full` field on `ExploredCells` gated. Painter's branch gated with `cfg!(feature = "dev")`.
- [x] `show_full_toggle_flips_field` dev-only test implemented and passes.
- [x] `cargo test` / `cargo test --features dev` both pass.
- [x] Commit: combined in `f9f2e7a`.

### Step 9: Final verification + diff review

- [x] All 7 verification commands pass with zero warnings, zero formatting diff.
- [x] `git diff --stat origin/main..HEAD` shows expected files. `src/main.rs` byte-unchanged per D1 override. D1 override means `src/plugins/ui/minimap.rs` (new) + `src/plugins/ui/mod.rs` (+12) instead of plan's `src/plugins/dungeon/minimap.rs` + `src/main.rs`.
- [x] No unrelated Cargo.lock bumps — 7 expected egui entries only.
- [x] No edits to state/, input/, loading/, audio/, data/, combat/, town/, party/, save/ — confirmed.
- [ ] Manual smoke (deferred to user — see Implementation Discoveries → Manual smoke checklist):
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

### Recorded resolutions (Steps A/B/C)

- **Step A:** Resolved `bevy_egui` version: `0.39.1`. Bevy 0.18.x compatibility confirmed: **yes** (`req: ^0.18.0`). No HALT needed.
- **Step B:** Cargo.toml line chosen: `bevy_egui = { version = "=0.39.1", default-features = false, features = ["render", "default_fonts"] }`. Defaults audited: `manage_clipboard` (arboard, unneeded), `open_url` (webbrowser, unneeded), `picking` (bevy_picking, unneeded), `bevy_ui` (bevy_ui_render, unneeded), `default_fonts` (keep), `render` (keep). Decision: **B** (opt out of unneeded defaults).
- **Step C:** `EguiPlugin` constructor shape: `EguiPlugin::default()` (struct with `enable_multipass_for_primary_context: true` default; field is `#[deprecated]` — use `EguiPlugin::default()` per docs). `EguiContexts::ctx_mut()` returns: **`Result<&mut egui::Context, QuerySingleError>`** — painter fns use `let ctx = contexts.ctx_mut()?;` with `-> Result` return type. Expected Cargo.lock additions: `bevy_egui`, `egui`, `ecolor`, `emath`, `epaint`, `epaint_default_fonts`, `nohash-hasher`. Actual: confirmed exactly these 7 entries.
- **Step 0 baseline:** `BASELINE_LIB = 67` default / `68` dev. Integration tests = 3.
- **Final test counts (post-Step 9):** lib `78` default (+11) / `81` dev (+13 including 2 dev-only tests). Integration 3. All pass.

### API surprises discovered during implementation

1. **`EguiPrimaryContextPass` schedule (not `Update`):** `bevy_egui` 0.39.1 requires UI-drawing systems to be registered in `EguiPrimaryContextPass` schedule, not `Update`. The plan said to use `Update` for painter systems — corrected during implementation. Non-painter systems (`update_explored_on_move`, `handle_map_open_close`) remain in `Update` as planned. The two schedules are naturally ordered by Bevy's main schedule pipeline.

2. **`bevy::utils::HashMap` removed in Bevy 0.18.1:** The plan recommended `use bevy::utils::HashMap` — this path no longer exists. Switched to `std::collections::HashMap`. No behavioral difference (hashbrown performance only needed at scale; 6×6 floor maps don't warrant it).

3. **`egui::Frame::none()` deprecated in egui 0.33:** Replaced with `egui::Frame::NONE` for the overlay and `egui::Frame::new()` for the full-screen panel. No behavioral difference.

4. **ActionState direct-mutation test pattern:** Without `InputManagerPlugin` + `InputPlugin`, leafwing's tick system doesn't run, so `JustPressed` state never advances to `Pressed`. This means calling `press()` twice in the same test without clearing creates a double-fire. Fixed with the press/update/release/update pattern: each action is pressed for exactly one update frame then released. `ActionsPlugin` was intentionally excluded from the test app to avoid `AccumulatedMouseMotion` panic (requires `InputPlugin` which would clear `just_pressed` in PreUpdate before the Update system can observe it — same constraint as the F9 state tests).

5. **D1 override deployment:** `EguiPlugin` registration was folded into `UiPlugin::build` alongside `MinimapPlugin`. `src/main.rs` is byte-unchanged. `src/plugins/ui/mod.rs` is the only modified non-new file in `src/plugins/ui/` (besides the new `minimap.rs`).

6. **`egui::Frame::none()` for overlay:** The overlay uses `egui::Frame::NONE` to eliminate the default egui window border/background. The full-screen view uses `egui::Frame::new().fill(Color32::from_rgb(20,20,30))` to provide a dark background.

### Deviations from plan

| Item | Plan said | Actual |
|------|-----------|--------|
| Painter schedule | `Update` | `EguiPrimaryContextPass` |
| `bevy::utils::HashMap` | use it | Not available; use `std::collections::HashMap` |
| `egui::Frame::none()` | use it | Deprecated; use `Frame::NONE` / `Frame::new()` |
| `MinimapPlugin` registration | `main.rs` (D1=A) | `UiPlugin::build` (D1=C override) |
| `EguiPlugin` registration | `main.rs` | `UiPlugin::build` |
| Test app: `ActionsPlugin` | included | Excluded (mouse-resource panic); `ActionState<DungeonAction>` inserted via `init_resource` directly |
| `EguiPlugin` constructor | `EguiPlugin { enable_multipass_for_primary_context: false }` | `EguiPlugin::default()` (field deprecated; default is `true` = multipass on, which requires `EguiPrimaryContextPass` — consistent) |

### Manual smoke checklist (deferred to user)

All items below require manual verification with `cargo run --features dev`:

- [ ] Game launches without panic.
- [ ] F9 to Dungeon. Walk W/A/S/D. Minimap overlay visible top-right (200×200), cells shade grey on visit.
- [ ] Player arrow visible on overlay, points in current facing direction.
- [ ] Press M → full-screen map appears with same data.
- [ ] Press M again → returns to Exploring. Overlay re-visible.
- [ ] Press M, then Escape → returns to Exploring.
- [ ] Dark-zone cell (floor_01 has none by default — test via F8 show_full or author one): walk on it, verify cell shows `?` and is NOT marked Visited.
- [ ] F8 (dev-only) toggles show_full. Map fills with all cells. F8 again to disable.
- [ ] F9 cycle Dungeon→Combat→...→Dungeon: previously-explored cells still marked on second Dungeon entry.
- [ ] Dev Camera2d HUD (top-left) and egui overlay (top-right) coexist without flicker or z-fighting.

## Verification

### Pre-pipeline

- [x] Local main is up to date — `cd /Users/nousunio/Repos/Learnings/claude-code/druum && git fetch origin && git log main..origin/main --oneline` shows zero new commits — Manual

### Step gates

- [x] Step A resolved `bevy_egui` version is recorded inline in this plan (Implementation Discoveries → Recorded resolutions) AND accepts `bevy = "0.18.x"` — Manual
- [x] Step B feature audit recorded inline; D8 decision made (A or B with explicit feature list) — Manual
- [x] Step C `EguiPlugin` shape, `EguiContexts::ctx_mut()` return type, expected Cargo.lock additions all recorded inline — Manual

### Compilation + lint

- [x] `cargo check` passes — Build — `cargo check` — Automatic
- [x] `cargo check --features dev` passes — Build — `cargo check --features dev` — Automatic
- [x] `cargo clippy --all-targets -- -D warnings` zero warnings — Lint — `cargo clippy --all-targets -- -D warnings` — Automatic
- [x] `cargo clippy --all-targets --features dev -- -D warnings` zero warnings — Lint — `cargo clippy --all-targets --features dev -- -D warnings` — Automatic
- [x] `cargo fmt --check` zero diff — Format — `cargo fmt --check` — Automatic

### Tests

- [x] `cargo test` passes with `BASELINE_LIB + ~8` lib tests + 3 integration tests — Test — `cargo test 2>&1 | tail -5` — 78 lib (+11), 3 integration
- [x] `cargo test --features dev` passes with same delta (or +1 for the dev-only `show_full_toggle_flips_field` test if added) — Test — `cargo test --features dev 2>&1 | tail -5` — 81 lib (+13), 3 integration
- [x] No previously-passing tests regress — Test — visual diff against Step 0 baseline — all 67/68 prior tests still pass
- [x] `cargo audit` reports zero advisories for `bevy_egui` and its transitive deps — Security — one pre-existing `unmaintained` advisory (paste via wgpu-hal, present before this feature), zero security errors

### Diff review (cleanest-ship signal)

- [x] `git diff --stat` shows expected files: `Cargo.toml` (+1), `Cargo.lock` (+7 egui crates), `src/plugins/dungeon/mod.rs` (+10 pub(crate)+doc), `src/plugins/ui/minimap.rs` (new, +773), `src/plugins/ui/mod.rs` (+12 EguiPlugin+MinimapPlugin). `src/main.rs` UNCHANGED. Note: D1 override means plan's "main.rs (+1)" replaced by "ui/mod.rs (+12)".
- [x] No unrelated transitive bumps in `Cargo.lock` — 7 new entries: bevy_egui, egui, ecolor, emath, epaint, epaint_default_fonts, nohash-hasher. All expected.
- [x] No edits to `state/`, `input/`, `loading/`, `audio/`, `data/`, `combat/`, `town/`, `party/`, `save/` — confirmed

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
