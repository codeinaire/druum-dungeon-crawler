# Feature #10 — Auto-Map / Minimap — Research

**Researched:** 2026-05-04
**Domain:** Bevy 0.18.1 plugin work — first dependency on `bevy_egui`; ECS-driven explored-cell tracking + immediate-mode painter UI for full-screen map and corner overlay
**Confidence:** HIGH for codebase verification (#2-#6, #11), HIGH for ColorRgb / Δ-deps discipline; **MEDIUM** for `bevy_egui` API shape and version (training-data only — local cache empty); HIGH for the Step-A verification gate that resolves the MEDIUM

## Summary

Feature #10 adds the genre-mandatory auto-map: a per-cell explored-state map updated on every `MovedEvent`, rendered through `bevy_egui`'s painter API as a full-screen view in `DungeonSubState::Map` and a top-right corner overlay during `DungeonSubState::Exploring`. Of the seven roadmap todos, six are mechanical (resource definition, message subscriber, painter draw calls, sub-state hookup, dark-zone gate, debug toggle); the load-bearing risk is **the first dep added since Feature #5 — `bevy_egui` — whose Bevy 0.18-compatible version cannot be verified from the local Cargo cache** and must be resolved via an upfront `cargo add --dry-run` gate identical to the precedents from Features #3 (`bevy_common_assets`/`bevy_asset_loader`) and #5 (`leafwing-input-manager`).

All other prerequisites are confirmed in the codebase as of HEAD (`gitbutler/workspace`):
- `MovedEvent` exists at `src/plugins/dungeon/mod.rs:192-197`, derives `Message` (not `Event`), carries `from`/`to`/`facing` — perfectly shaped for a minimap subscriber.
- `DungeonSubState::Map` exists at `src/plugins/state/mod.rs:23` (declared in the SubStates block; `OnEnter(DungeonSubState::Map)` and `OnExit(...)` work without further changes).
- `DungeonAction::OpenMap` exists at `src/plugins/input/mod.rs:79` and is bound to `KeyCode::KeyM` (line 150) — no input-enum work needed.
- `DungeonFloor::width` / `.height` are public `u32` fields, `walls` is `Vec<Vec<WallMask>>` indexed `[y][x]` (per `src/data/dungeon.rs:252-256`), and `dark_zone: bool` lives on `CellFeatures` (line 164) — all read-paths exist; `src/data/dungeon.rs` stays frozen.
- The plugin-module pattern is fully established: `audio/{mod.rs,bgm.rs,sfx.rs}` is the precedent for sibling submodules under a parent plugin folder.

**Primary recommendation:** Run a Step-A `cargo add bevy_egui --dry-run` gate to resolve the actual 0.18-compatible version before touching `Cargo.toml` (training data says `0.39.x`; verify). Then implement `MinimapPlugin` as a sibling module (`src/plugins/dungeon/minimap.rs`) registered alongside `DungeonPlugin` in `main.rs` (NOT inside `DungeonPlugin::build` — owns its own resource and OnEnter/OnExit timing, mirrors the audio split). Use egui canvas painter (no render-to-texture) for floor_01's 6x6 grid; defer RTT until floor sizes actually exceed ~30x30. Layer 2 tests cover data flow (`MovedEvent` → `ExploredCells` cell flip, dark-zone bypass, debug-show-full toggle); the visual painter goes to manual smoke per the audio-plugin precedent.

---

## Standard Stack

### Core (existing, unchanged)

| Library | Version | Purpose | License | Maintained? | Why Standard |
|---------|---------|---------|---------|-------------|--------------|
| [bevy](https://crates.io/crates/bevy) | =0.18.1 | Game engine | MIT/Apache-2.0 | HIGH (Bevy Foundation) | Pinned; Δ deps already locked |
| [leafwing-input-manager](https://crates.io/crates/leafwing-input-manager) | =0.20.0 | Input → action mapping | MIT/Apache-2.0 | HIGH | `DungeonAction::OpenMap` already wired |
| [bevy_common_assets](https://crates.io/crates/bevy_common_assets) | =0.16.0 | RON asset loader | MIT/Apache-2.0 | HIGH | Loads `DungeonFloor` already |

### New (Feature #10)

| Library | Version | Purpose | License | Maintained? | Why Standard |
|---------|---------|---------|---------|-------------|--------------|
| [bevy_egui](https://crates.io/crates/bevy_egui) | **TBD — Step A resolves**; training data says `0.39.x` for Bevy 0.18 (MEDIUM) | Immediate-mode UI painter for the map | MIT/Apache-2.0 | Active; `vladbat00/bevy_egui` (formerly `mvlabat/bevy_egui` — repo moved 2025-ish, both names see redirect) | Master research §Don't Hand-Roll line 993; the de-facto egui-in-Bevy bridge with no mature alternative |

### Supporting (no new entries needed for #10)

`ExploredCells` uses `bevy::utils::HashMap` (re-export of `hashbrown::HashMap` already in dep tree via Bevy). Persistence to disk is Feature #23's problem — the resource just needs `#[derive(Resource, Default)]` plus `#[derive(Serialize, Deserialize)]` for the future save layer (serde is already a direct dep).

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|-----------|-----------|----------|
| `bevy_egui` | `bevy_ui` raw shapes | bevy_ui has no canvas/painter primitive — would require spawning per-segment `Node`s with `BackgroundColor`. ~thousands of entities for a 50x50 map; unworkable. Master research §Anti-Pattern explicitly bans bevy_ui for complex UI. |
| `bevy_egui` | Custom `bevy::sprite` 2D camera + `Mesh2d` lines | Possible but reinvents egui's coordinate/zoom/rotation utilities, plus you'd still need a UI framework for the debug toggle. Defeats the "natural place to add bevy_egui" rationale (roadmap line 572). |
| `bevy_egui` | Quill (reactive UI) | Master research §Alternatives notes: younger, less battle-tested. Future-migration material, not a Feature #10 swap. |
| `bevy_egui` | `iced` / `dioxus` Bevy bridges | Both exist as experimental crates; neither has the broad community use of `bevy_egui`. Survivorship bias check: every Bevy crawler/RPG project I can find uses bevy_egui. |
| Render-to-texture (RTT) for the canvas | egui painter direct draw | RTT is more complex and only wins above ~thousands-of-segments per frame. Roadmap §Open Question 5 says "start with egui canvas, upgrade if needed". For 6x6 floor_01 (max ~144 segments), this is decisive — canvas now. |

**Installation (after Step A confirms version):**

```bash
# Verify version FIRST — do NOT edit Cargo.toml yet.
cargo add bevy_egui --dry-run 2>&1 | tee /tmp/bevy-egui-resolve.txt

# Then (after manually verifying the resolved version supports bevy = "0.18.x"):
# Add to [dependencies] in Cargo.toml — pin with =, opt out of egui defaults
# that pull asset/serialize/winit-x11 if Step B audit recommends it.
```

---

## Architecture Options

The roadmap pre-selected `bevy_egui`; the genuine architectural decisions for Feature #10 are how the data lives and how the painter is wired. Five Category-B-ish decisions with distinct trade-offs:

### Option set 1: Plugin module structure

| Option | Description | Pros | Cons | Best When |
|--------|-------------|------|------|-----------|
| **A: Sibling `MinimapPlugin` registered alongside `DungeonPlugin`** | New file `src/plugins/dungeon/minimap.rs` exposes `pub struct MinimapPlugin`; main.rs adds it after `DungeonPlugin`. Mirrors `bgm.rs`/`sfx.rs` exposing private modules but `AudioPlugin` aggregating them. Inverted here: each is its own Plugin. | Clear ownership of `ExploredCells` resource; OnEnter/OnExit timing independent of `DungeonPlugin`'s spawn order; main.rs reads as a registry of features; trivially removable (e.g. for a "no-map" challenge mode). | Two `add_plugins(...)` lines instead of one; if `MinimapPlugin` ever needs to depend on something `DungeonPlugin` initializes, ordering becomes load-bearing. | The map is a parallel concern — it reads `MovedEvent` but doesn't modify the dungeon's authoritative state. (RECOMMENDED) |
| **B: Nested `app.add_plugins(MinimapPlugin)` from inside `DungeonPlugin::build`** | `DungeonPlugin::build` ends with `app.add_plugins(MinimapPlugin)`. main.rs unchanged. | One add_plugins line in main.rs; reinforces "the map is part of the dungeon"; matches some Bevy community patterns (nested plugins). | Ordering coupling — if you ever need MinimapPlugin to run before DungeonPlugin's systems, you'd have to refactor. Tests must include DungeonPlugin to exercise MinimapPlugin. | The minimap is structurally inseparable from dungeon (it isn't). |
| **C: `MinimapPlugin` lives in `src/plugins/ui/`** | Treat the map as a UI feature; `src/plugins/ui/minimap.rs` and `UiPlugin::build` registers it. | Reinforces "all UI lives under ui/"; later UI features (#19 Town UI, #25 polish) cluster naturally. | UiPlugin currently empty/stub; this lands an architecture decision (UI-as-aggregator-plugin) that should be scoped intentionally, not as a side effect of Feature #10. The map needs `MovedEvent` (a dungeon concern) — cross-module coupling. | Once a `UiPlugin` aggregator exists with multiple consumers (Feature #19+). |

**Recommended: A** — sibling `MinimapPlugin`, registered alongside `DungeonPlugin` in `main.rs`. Two reasons: (a) the map is data + view, not state-mutation — keeping it parallel to the dungeon avoids accidental coupling in future features; (b) it's the lowest-risk pattern for Feature #23 save integration: a `MinimapPlugin` that owns `ExploredCells` knows where to register save/load callbacks without `DungeonPlugin` needing to know about them.

### Option set 2: Where `ExploredCells` lives

| Option | Description | Pros | Cons | Best When |
|--------|-------------|------|------|-----------|
| **A: `Resource`** | `app.init_resource::<ExploredCells>()` in `MinimapPlugin::build`. `HashMap<(floor: u32, x: u32, y: u32), ExploredState>`. | Simplest; lives at app level; matches `ChannelVolumes` precedent; one-liner registration. | "Global" feel can grate with strict ECS purists; saves as a singleton blob in #23 (fine for moonshine-save's pattern). | Default. (RECOMMENDED) |
| **B: `Component` on `PlayerParty` entity** | Spawn `PlayerParty` with `ExploredCells` attached. Saves cleanly with party in #23. | Lifecycle follows party (no manual cleanup); matches "data lives on entities" ECS purism. | When PlayerParty is despawned on `OnExit(Dungeon)` (verified at `mod.rs:412-419`), the explored data goes with it — for cross-floor persistence (which the spec wants: `(floor, x, y)` keys imply a single map across all floors), this is wrong. Fixable with a non-despawning entity, but you've then reinvented "Resource". |
| **C: Lazy-built from a more granular source of truth** | E.g. derive from `Vec<MovedEvent>` history. | Single source of truth; replays are trivial. | Replaying every MovedEvent on every map open is O(history); unbounded growth. Solves a problem we don't have. |

**Recommended: A** — Resource. The `(floor, x, y)` cross-floor key explicitly contradicts Option B's lifecycle. Roadmap line 561 says `ExploredCells` becomes part of the save data later (#23) — Resource is what `bevy_save`/`moonshine-save` save the most cleanly.

### Option set 3: Canvas vs render-to-texture

| Option | Description | Pros | Cons | Best When |
|--------|-------------|------|------|-----------|
| **A: egui canvas painter every frame** | `painter.line_segment(...)` / `painter.rect_filled(...)` called per visible cell each frame. | Simplest; no render passes; straightforward zoom/pan; toggles between full/overlay are just rect-bounds changes. | At >5000 segments/frame, egui's tessellator is the bottleneck. | floor_01 at 6x6 (max ~144 segments) — way under threshold. (RECOMMENDED for #10) |
| **B: Render-to-texture once per `ExploredCells` change** | A 2D camera renders into an offscreen texture via `bevy_egui`'s `EguiUserTextures` registration. egui draws the texture as a single image. | Constant per-frame draw cost (one textured quad); scales to arbitrarily large floors. | Adds a 2D camera + image asset + change-detection trigger; coordinate transform between texture pixels and egui display pixels is one more thing to get right; needs invalidation logic when ExploredCells mutates. | Floors approach 50x50 (~10k segments). Defer until then. |

**Recommended: A** — for #10, A. Master research §Open Question 5 endorses this exact ordering: "Start with egui canvas rendering of explored cells. Upgrade to render-to-texture if egui performance is insufficient for large maps." Quantification: Roadmap notes a 50x50 floor would have ~5000 wall segments after dedup; egui's painter handles thousands of primitives per frame easily (master research line 1122: "egui frame time < 1ms for complex UIs"). The threshold for needing RTT is well above floor_01's scale.

### Option set 4: Minimap overlay placement + size

| Option | Description | Pros | Cons | Best When |
|--------|-------------|------|------|-----------|
| **A: Top-right anchored, fixed size 200x200, no background** | `egui::Window::new("minimap").anchor(Align2::RIGHT_TOP, [-10.0, 10.0]).fixed_size([200.0, 200.0]).frame(Frame::none())` — translucent overlay. | Out of the way of the player's forward view; matches Etrian Odyssey/Wizardry remake convention; doesn't fight with the dev grid HUD (top-LEFT corner per `mod.rs:800-806`). | Some players find top-right cluttered with future HUD elements (HP bars). | Default. (RECOMMENDED) |
| **B: Bottom-right or bottom-left** | Same shape, different anchor. | Doesn't compete with dev HUD at all. | Bottom-right competes with future combat-action menus; bottom-left competes with future party-portrait area. |
| **C: Toggleable corner** | `OpenMap` cycles full/overlay/hidden; settings UI eventually picks the corner. | Player-configurable. | Three-state toggle is more code; not Feature #10's scope (settings is #25). |

**Recommended: A** with hardcoded constants (`MINIMAP_OVERLAY_SIZE: f32 = 200.0`, `MINIMAP_OVERLAY_PAD: f32 = 10.0`). Mark them `pub(crate) const` so #25 polish can tune. Not a permanent decision — surface it as Category B.

### Option set 5: `OpenMap` toggle behavior — full-screen ↔ exploring

| Option | Description | Pros | Cons | Best When |
|--------|-------------|------|------|-----------|
| **A: `M` toggles between Exploring and Map; Escape also exits Map** | Pressing `M` from Exploring → SubState::Map; pressing `M` again OR Escape from Map → SubState::Exploring. Both `OpenMap` and `Pause` actions can leave the map. | Standard genre convention — every Wizardry remake works this way. M is the open/close key, Escape is the universal "back". | Two key paths to test; need to handle both `actions.just_pressed(&DungeonAction::OpenMap)` and `actions.just_pressed(&DungeonAction::Pause)` in the Map sub-state's input handler. | Default. (RECOMMENDED) |
| **B: `M` only toggles; Escape opens Pause sub-state** | Strict; M is the map key, Escape is always pause. | Cleaner separation of concerns. | Doesn't match player muscle memory — most players will hit Escape to "back out" of a map view. |
| **C: `M` opens; `M` again does nothing; Escape closes** | Open-only on M. | Simplest input code. | Confusing UX. |

**Recommended: A** — M toggles, Escape also exits. Implementation is one extra `OR` in the input handler.

### Option set 6: Time/turn behavior while map is open

| Option | Description | Pros | Cons | Best When |
|--------|-------------|------|------|-----------|
| **A: Pause everything (no `Update` for dungeon systems while in `Map` sub-state)** | All `run_if(in_state(DungeonSubState::Exploring))` systems naturally pause. Time keeps ticking (Bevy `Time` is global), but no game logic advances. | Already how the codebase works — `handle_dungeon_input` is gated by `in_state(DungeonSubState::Exploring)` (mod.rs:218). Map-open frames are visually static. | Torch flicker also pauses (it's gated on `in_state(GameState::Dungeon)`, NOT on the SubState — so it actually KEEPS flickering). Verify this is the intended behavior. | Default — already works for free. (RECOMMENDED) |
| **B: Time also pauses (e.g. set `TimeUpdateStrategy::Paused`)** | Stops Time::delta from advancing while map is open. | Future-proofing for time-of-day or status-effect ticks. | Map-open is also when players think — pausing real wall-clock could weird out future audio fades. | Once #15 turn-based combat or #20 spell durations land. |

**Recommended: A** — works for free. The fact that torch flicker continues but movement halts is the right behavior (the dungeon "lives" while you read the map).

### Option set 7: `KnownByOther` provenance

| Option | Description | Pros | Cons | Best When |
|--------|-------------|------|------|-----------|
| **A: Define the variant; never write it in #10. Add to `match` arms with the same shading as Visited.** | `enum ExploredState { Unseen, Visited, KnownByOther }` declared but unused beyond the visualization. | Zero scope creep; exhaustive match keeps future writers honest; rendering already differentiates so there's no "unused variant" lint. | Variant exists in code with no callers — looks like dead code in some readers' eyes. | Default. (RECOMMENDED) |
| **B: Don't add the variant; add it later when an item/spell needs it (#12 / #20).** | One less line of type code; simpler match arms. | Future feature needs to add a variant AND update every match site (where #10 already had one); also forces consumers of the resource (e.g. save format) to migrate. | If you're absolutely sure no other feature needs to populate the map. |
| **C: Define a richer enum upfront** (`Unseen, Visited { steps: u32 }, KnownByOther { source: RevealSource }`) | Forward-compat metadata baked in. | YAGNI — no #10 caller wants this; unbounded design space. | Never. |

**Recommended: A** — declare the variant; render it identically to Visited (or with a slight color tint to make the dev "show full map" toggle distinguishable from real exploration). Document "no producer in v1" in the variant doc-comment. Surface as Category B.

---

## Counterarguments — red-team review

Why someone might NOT pick the recommended bundle:

- **"Putting MinimapPlugin separate from DungeonPlugin will mean two test-app setups."** — Response: tests for `MinimapPlugin` need `MinimalPlugins + StatesPlugin + ActionsPlugin + MinimapPlugin + add_message::<MovedEvent>()`. They do NOT need `DungeonPlugin` (and shouldn't depend on it — that's the point). Existing test patterns (audio, input) already build minimal apps without their producer plugins. The duplication is intentional isolation.
- **"The roadmap explicitly says `bevy_egui = "0.39"`. Why are you defending Step A?"** — Response: Two precedents (Feature #3, Feature #5) confirm that "the roadmap version" was wrong by the time we got to it (`bevy_kira_audio` was deviated to native `bevy_audio`; `leafwing-input-manager` resolved to `0.20.0` not `0.18.x`). Resolving the version cost ~5 minutes via `cargo add --dry-run`; assuming the roadmap version costs an unknown amount of debugging if it's wrong. Cheap insurance.
- **"You're scoping out RTT entirely. What if egui canvas surprises us with poor performance?"** — Response: floor_01 is 6x6, ~36 cells, max ~144 segments visible. Even if egui's painter were 1000× slower than benchmarked, 144k operations per frame is in millisecond territory. The threshold for needing RTT is when floor sizes get above ~30x30, which is a Feature #11+ concern. Documented in the implementation summary so #11 knows to revisit.
- **"egui has its own font loading and that conflicts with `default_font`."** — Response: They coexist. `default_font` is for `bevy::Text` widgets; egui's font loading is internal to `egui::Context`. No collision.
- **"Won't `bevy_egui` pull in `winit/x11` features that bloat builds?"** — Response: This is what the Step B feature audit catches. Most modern `bevy_egui` releases default to a minimal feature set; if defaults pull `bevy_winit/x11`, opt out via `default-features = false`. Documented in §Pitfalls.

---

## Architecture Patterns

### Recommended project structure (after #10)

```
src/plugins/dungeon/
    mod.rs               # DungeonPlugin (unchanged at the structural level — owns party + geometry)
    tests.rs             # existing unit + integration tests
    minimap.rs           # NEW — MinimapPlugin, ExploredCells, painter systems
                         #       (plus its own #[cfg(test)] mod tests inline OR
                         #       a sibling minimap_tests.rs if it grows >300 LOC)
src/plugins/mod.rs       # add `pub mod ...` only if we expose MinimapPlugin
                         # via a pub use re-export — otherwise the existing
                         # `pub mod dungeon;` already covers minimap.rs
src/main.rs              # one new line:
                         # `MinimapPlugin,` after `DungeonPlugin,` in add_plugins(...)
```

### Pattern 1: Plugin shape

```rust
// Source: derived from Druum's audio/mod.rs precedent (full file at src/plugins/audio/mod.rs)
// Adapted for the minimap data + view split.

use bevy::prelude::*;
use bevy::utils::HashMap;

use crate::plugins::dungeon::MovedEvent;
use crate::plugins::input::DungeonAction;
use crate::plugins::state::{DungeonSubState, GameState};

#[derive(Resource, Default, Debug, Clone)]
pub struct ExploredCells {
    /// Key: `(floor_number, x, y)`. Floor crosses persist across re-entries.
    pub cells: HashMap<(u32, u32, u32), ExploredState>,
    /// Dev/debug toggle: when true, the painter renders every cell as Visited
    /// regardless of map data. Does NOT mutate `cells`. See `toggle_show_full_map`.
    pub show_full: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ExploredState {
    /// Cell never observed.
    #[default]
    Unseen,
    /// Player has stood in (or adjacent to — TBD per Pitfall 4) this cell.
    Visited,
    /// Revealed by an external source (item, spell, scry — Feature #12 / #20).
    /// Variant declared in #10 but not produced by any system yet.
    KnownByOther,
}

pub struct MinimapPlugin;

impl Plugin for MinimapPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ExploredCells>()
            // Reset on dungeon-exit (NOT on dungeon-enter, so re-entering preserves
            // the player's prior exploration of this floor). Future Feature #11
            // revisits this for cross-session persistence.
            // NOTE: Skipping reset entirely is also defensible — the cells outlive
            // the dungeon state. Surface as a Category B if user disagrees.
            // .add_systems(OnExit(GameState::Dungeon), reset_explored_cells)
            .add_systems(
                Update,
                (
                    update_explored_on_move
                        .run_if(in_state(GameState::Dungeon))
                        .after(crate::plugins::dungeon::handle_dungeon_input_path), // see ordering note
                    toggle_show_full_map.run_if(in_state(GameState::Dungeon)),
                    paint_minimap_overlay
                        .run_if(in_state(DungeonSubState::Exploring)),
                    paint_minimap_full
                        .run_if(in_state(DungeonSubState::Map)),
                    handle_map_open_close.run_if(in_state(GameState::Dungeon)),
                ),
            );
    }
}
```

### Pattern 2: Subscriber on `MovedEvent`

```rust
// Source: derived from src/plugins/dungeon/mod.rs:678 (where MovedEvent is written).
// Bevy 0.18 family rename — read with MessageReader, NOT EventReader.

use bevy::prelude::*;

fn update_explored_on_move(
    mut moved: MessageReader<MovedEvent>,
    floors: Res<Assets<DungeonFloor>>,
    dungeon_assets: Option<Res<DungeonAssets>>,
    mut explored: ResMut<ExploredCells>,
) {
    let Some(assets) = dungeon_assets else { return };
    let Some(floor) = floors.get(&assets.floor_01) else { return };
    let floor_no = floor.floor_number;

    for ev in moved.read() {
        let (x, y) = (ev.to.x, ev.to.y);
        // Dark-zone gate: do NOT update if the destination cell is a dark zone.
        // The stale data persists; the painter shows "?" for dark-zone cells.
        if floor.features[y as usize][x as usize].dark_zone {
            continue;
        }
        explored.cells.insert((floor_no, x, y), ExploredState::Visited);
    }
}
```

### Pattern 3: egui painter (canvas) sketch

```rust
// Source: synthesized from egui's painter API (training data MEDIUM — verify in Step C).
// The painter is a thin wrapper over egui's Shape primitives.

use bevy_egui::{egui, EguiContexts};

fn paint_minimap_full(
    mut contexts: EguiContexts,
    explored: Res<ExploredCells>,
    floors: Res<Assets<DungeonFloor>>,
    dungeon_assets: Option<Res<DungeonAssets>>,
    party: Query<(&GridPosition, &Facing), With<PlayerParty>>,
) {
    let Ok(ctx) = contexts.ctx_mut() else { return };
    let Some(assets) = &dungeon_assets else { return };
    let Some(floor) = floors.get(&assets.floor_01) else { return };
    let Ok((pos, facing)) = party.single() else { return };

    egui::CentralPanel::default()
        .frame(egui::Frame::none().fill(egui::Color32::from_rgb(20, 20, 30)))
        .show(ctx, |ui| {
            let (rect, _resp) = ui.allocate_exact_size(
                ui.available_size(),
                egui::Sense::hover(),
            );
            let painter = ui.painter_at(rect);
            paint_floor_into(&painter, rect, floor, &explored, *pos, facing.0);
        });
}

fn paint_minimap_overlay(/* same args */) {
    // Same idea, but anchored egui::Window with fixed size 200x200.
    egui::Window::new("minimap")
        .anchor(egui::Align2::RIGHT_TOP, [-10.0, 10.0])
        .fixed_size([200.0, 200.0])
        .frame(egui::Frame::none().fill(egui::Color32::from_rgba_premultiplied(20, 20, 30, 200)))
        .title_bar(false)
        .resizable(false)
        .show(ctx, |ui| { /* paint_floor_into with smaller rect */ });
}

// Per-cell + per-wall draws factored into a shared helper so the two paths
// don't drift.
fn paint_floor_into(
    painter: &egui::Painter,
    rect: egui::Rect,
    floor: &DungeonFloor,
    explored: &ExploredCells,
    pos: GridPosition,
    facing: Direction,
) {
    let cell_size = (rect.width() / floor.width as f32).min(rect.height() / floor.height as f32);
    let origin = rect.min;
    let floor_no = floor.floor_number;

    for y in 0..floor.height {
        for x in 0..floor.width {
            let key = (floor_no, x, y);
            let state = if explored.show_full {
                ExploredState::Visited
            } else {
                explored.cells.get(&key).copied().unwrap_or(ExploredState::Unseen)
            };
            let cell_rect = egui::Rect::from_min_size(
                egui::pos2(origin.x + x as f32 * cell_size, origin.y + y as f32 * cell_size),
                egui::vec2(cell_size, cell_size),
            );
            // Per-state shading
            let shade = match state {
                ExploredState::Unseen => egui::Color32::TRANSPARENT,
                ExploredState::Visited => egui::Color32::from_rgb(60, 60, 70),
                ExploredState::KnownByOther => egui::Color32::from_rgb(50, 50, 100),
            };
            painter.rect_filled(cell_rect, 0.0, shade);

            // Dark-zone marker
            if floor.features[y as usize][x as usize].dark_zone {
                painter.text(
                    cell_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    "?",
                    egui::FontId::proportional(cell_size * 0.5),
                    egui::Color32::YELLOW,
                );
            }

            // Walls — use the same dedup rule as src/plugins/dungeon/mod.rs::spawn_dungeon_geometry
            // so the map matches the renderable geometry exactly.
            let walls = &floor.walls[y as usize][x as usize];
            paint_wall_if_visible(painter, cell_rect, walls.north, Side::North);
            paint_wall_if_visible(painter, cell_rect, walls.west, Side::West);
            if y == floor.height - 1 {
                paint_wall_if_visible(painter, cell_rect, walls.south, Side::South);
            }
            if x == floor.width - 1 {
                paint_wall_if_visible(painter, cell_rect, walls.east, Side::East);
            }
        }
    }

    // Player arrow on top
    let player_pos = egui::pos2(
        origin.x + (pos.x as f32 + 0.5) * cell_size,
        origin.y + (pos.y as f32 + 0.5) * cell_size,
    );
    paint_player_arrow(painter, player_pos, cell_size * 0.3, facing);
}
```

### Pattern 4: Open/close handler

```rust
fn handle_map_open_close(
    actions: Res<ActionState<DungeonAction>>,
    current: Res<State<DungeonSubState>>,
    mut next: ResMut<NextState<DungeonSubState>>,
) {
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
}
```

### Anti-Patterns to Avoid

- **Don't query `DungeonAssets` and `Assets<DungeonFloor>` separately if both are present** — combine into a single `Option<...>` early-return helper to mirror the pattern at `mod.rs:622-631`. Avoids three nested `let Some(...) else { return };` per painter system.
- **Don't read `Res<ButtonInput<KeyCode>>` directly for the M key** — `DungeonAction::OpenMap` exists; using KeyCode would defeat Feature #5's whole abstraction and break user-rebinding (Feature #25).
- **Don't reset `ExploredCells` on `OnEnter(GameState::Dungeon)`** — players returning from town to a previously-explored floor expect their map intact. Reset (if needed) only on full game-over or new-game.
- **Don't redraw the entire painter as one mega-shape** — egui handles thousands of separate primitives fine; manual mega-batching adds complexity without measurable benefit at floor_01 scale.
- **Don't put `ExploredCells` on `PlayerParty` as a Component** — the entity is despawned on `OnExit(Dungeon)` (mod.rs:412-419), data goes with it. Resource is the right shelf for cross-floor data.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Immediate-mode UI for the map | Custom 2D painter | `bevy_egui` (selected) | Egui painter has line/rect/text primitives, anchoring, layout, and zoom — re-implementing is a project-sized detour |
| Many-to-many key→action | Direct `KeyCode::KeyM` checks | `DungeonAction::OpenMap` (already wired) | Feature #5 owns this; circumventing breaks rebinding |
| HashMap with frequent insert | `BTreeMap` or hand-rolled grid | `bevy::utils::HashMap` (re-export of hashbrown) | Already in dep tree; ahash is fast enough at floor scale |
| Save serialization for `ExploredCells` | Custom blob | Defer to Feature #23 | The Resource already derives `Default`; #23 will add `Serialize/Deserialize` (the spec change is one derive line) |

---

## Common Pitfalls

### Pitfall 1: `bevy_egui` version drift from the roadmap value

**What goes wrong:** Roadmap was authored 2026-04-29 with `bevy_egui = "0.39"`. By 2026-05-04 a `0.40` may exist (or 0.39 may have been yanked). Pinning the wrong version either (a) fails to resolve, or (b) silently pulls a Bevy version mismatch.
**Why it happens:** bevy_egui releases tracked Bevy minor versions historically — the version-to-Bevy mapping isn't stable across patches (just like `bevy_common_assets = "=0.16.0"` resolved AFTER the roadmap suggested `latest`).
**How to avoid:** Run the Step A `cargo add bevy_egui --dry-run` gate before editing `Cargo.toml`. Halt + escalate if the resolved version doesn't accept `bevy = "0.18.x"`. Same playbook as `leafwing-input-manager` Feature #5 Step A.

### Pitfall 2: Bevy 0.18 family rename — `Message` vs `Event`

**What goes wrong:** Writing `MessageReader<MovedEvent>` is correct in 0.18 but old tutorials show `EventReader`. Mixing the two compiles fine (both traits exist — for now) but the wrong one silently reads no messages.
**Why it happens:** The Bevy 0.18 split renamed the message family (StateTransitionEvent → Message; MovedEvent at `mod.rs:192` is annotated to derive Message, NOT Event).
**How to avoid:** `MovedEvent` derives `Message` — verified at `src/plugins/dungeon/mod.rs:192-197`. Any new subscriber MUST use `MessageReader<MovedEvent>`. Confirmed in memory: `feedback_bevy_0_18_event_message_split.md`.

### Pitfall 3: System ordering — minimap subscriber vs movement system

**What goes wrong:** `update_explored_on_move` runs BEFORE `handle_dungeon_input` writes `MovedEvent`. The reader sees nothing this frame, sees the message NEXT frame — but the test that wrote a key + called `app.update()` once will fail the cell-update assertion.
**Why it happens:** Bevy doesn't guarantee Update system order without explicit `.after()` / `.before()`; `MovedEvent`s can be read-after-write within the same frame ONLY if the writer system runs before the reader.
**How to avoid:** Add `.after(handle_dungeon_input)` to `update_explored_on_move` registration. Note: `handle_dungeon_input` is currently a free function in `dungeon::mod.rs`; making MinimapPlugin in a sibling file means importing it requires `pub` exposure. Two clean paths: (a) make `handle_dungeon_input` `pub(crate)` and import it; (b) use a system set marker on the input system. Either works. (a) is simpler for #10. NB: the alternate "Messages are buffered and read across frames" path also works — by the next frame, the message reader sees it; but tests that count "1 cell flip after 1 update()" will fail under that path, so explicit ordering is safer.

### Pitfall 4: "Visited" semantics — current cell only, or also adjacent?

**What goes wrong:** Implementer flips only the destination cell to `Visited`. Players who can SEE adjacent cells (line-of-sight from a corridor) are confused that the map only fills with cells they've stood ON.
**Why it happens:** Spec says "every `MovedEvent`" — ambiguous whether that means "the cell you moved INTO" or "every cell visible from your new position".
**How to avoid:** Surface as Category B. Recommendation: v1 marks ONLY the destination cell. Adjacent-cell reveal is a #25 polish (a "look ahead" mechanic that can model torchlight or peripheral vision). Document the choice in the `update_explored_on_move` system docstring.

### Pitfall 5: bevy_egui's default features potentially pull heavy transitive deps

**What goes wrong:** Default `bevy_egui` features may include `serde` (for theme persistence), `accesskit` (for screen readers), or platform-specific winit features (`x11`, `wayland`). Druum's Δ-deps discipline says we want only what we need.
**Why it happens:** Crate authors include sensible defaults that span use cases beyond ours.
**How to avoid:** Step B audit reads the resolved crate's `[features]` block. If `default = [...]` includes anything you don't need, opt out: `bevy_egui = { version = "=X.Y.Z", default-features = false, features = ["render"] }` (the exact feature names depend on the resolved version — Step B reveals them). Same audit ran for `leafwing-input-manager` Feature #5 Step B.

### Pitfall 6: `EguiContexts` vs `EguiContext` API drift

**What goes wrong:** Code uses `EguiContexts` system param, but the resolved version expects `EguiContext` (or vice versa). Compiler error is opaque ("trait bound not satisfied").
**Why it happens:** bevy_egui has gone through several context-access API revisions across 0.27→0.30→0.39. Older tutorials show patterns that don't compile against current.
**How to avoid:** Step C (API verification grep) — once the resolved crate is on disk, `grep -rn "pub fn ctx" ~/.cargo/registry/src/.../bevy_egui-X.Y.Z/src/` to find the access pattern. Also check: does `ctx_mut()` return `&mut egui::Context` directly, or `Result<&mut egui::Context, ...>`? Either works — code shape adapts.

### Pitfall 7: `EguiPlugin` requires a primary-context configuration parameter

**What goes wrong:** Recent bevy_egui versions changed `EguiPlugin` from a unit struct to one with fields like `enable_multipass_for_primary_context: bool`. Calling `app.add_plugins(EguiPlugin)` instead of `app.add_plugins(EguiPlugin { enable_multipass_for_primary_context: false })` fails to compile.
**Why it happens:** The plugin was extended to support multi-pass rendering (egui frames spanning multiple Bevy schedule cycles).
**How to avoid:** Step C grep `pub struct EguiPlugin` in the resolved source. Use whatever shape the resolved version has. Document the chosen value in the plan.

### Pitfall 8: dark-zone cells produce confusing player feedback

**What goes wrong:** Player walks into a dark-zone cell, the map cell stays at its previous state (intended). Then they walk OUT. The cell never updates because they were never IN it post-darkzone-flag-flip... wait, that's not the issue. The actual issue: a player who walks into a dark-zone cell gets NO feedback that the cell is dark; they think the map is broken.
**Why it happens:** Silent "skip update" looks identical to "no MovedEvent fired".
**How to avoid:** Three tactics: (a) flash a transient "MAP DISABLED" indicator in the corner overlay when entering a dark-zone cell; (b) show "?" character on dark-zone cells regardless of explored state (the spec says this — see `paint_floor_into` example above); (c) leave the cell's previous-known walls visible but desaturated. Recommendation for v1: (b) — render the "?" — it's already in the spec. (a) is #25 polish.

### Pitfall 9: `bevy_egui` and Druum's existing `Camera2d` (dev grid HUD) coexistence

**What goes wrong:** When running with `--features dev`, `spawn_debug_grid_hud` (mod.rs:781) spawns a `Camera2d` for the dev HUD. `bevy_egui` ALSO needs a 2D context. They might collide on render-order or conflicts in the camera ordering.
**Why it happens:** bevy_egui automatically attaches its render passes to a primary window — it doesn't generally need an explicit Camera2d. But if there's already one with `order: 1`, ordering becomes load-bearing.
**How to avoid:** Step C verify in bevy_egui's docs whether it needs a camera. Most likely it does NOT — egui renders to its own pass. The dev HUD's `Camera2d { order: 1 }` should coexist. If a conflict appears, bump the dev HUD's order higher (it's behind a `cfg(feature = "dev")` so the change is local).

### Pitfall 10: `ExploredCells` resource mutated during render path

**What goes wrong:** A system that reads `Res<ExploredCells>` for painting and another that writes `ResMut<ExploredCells>` for updates run in the same Update phase. Bevy's scheduler will serialize them (they conflict on the resource), but if neither orders against the other, the order is non-deterministic — sometimes the painter sees the latest update, sometimes not.
**Why it happens:** Default scheduling.
**How to avoid:** Make `update_explored_on_move` run `.before(paint_minimap_overlay)` and `.before(paint_minimap_full)`. Easier: use a SystemSet wrapping both painters and order them after the updater.

---

## Security

### Known Vulnerabilities

No known CVEs found for `bevy_egui` or `egui` family as of 2026-05-04 (training data — verify with `cargo audit` post-add). Master research §Security (line 1091) lists `bevy_egui 0.39.x — None found`.

| Library | CVE / Advisory | Severity | Status | Action |
|---------|---------------|----------|--------|--------|
| bevy_egui (Step A version) | None found in training data | — | — | Run `cargo audit` after Step 4 of the implementation plan |
| egui (transitive) | None found | — | — | Same |

### Architectural Security Risks

| Risk | Affected Architecture Options | How It Manifests | Secure Pattern | Anti-Pattern to Avoid |
|------|-------------------------------|------------------|----------------|-----------------------|
| Save-data injection (Feature #23 future) | `ExploredCells` Resource serialization | A crafted save file inserts billions of cell entries → memory pressure | Bound the `cells` HashMap size before deserializing; reject saves whose floor numbers exceed the dungeon's known floor count | Trust raw `serde_ron::from_str` on user-provided save data |
| Wall-state read race | Painter system reading `floors.get(&handle)` while asset reload happens | Returns stale geometry that disagrees with the live render | Guard with `Option` early-return (already the pattern in dungeon spawn paths); accept one frame of "stale" render rather than panic | Unwrap `floors.get(...)` directly |
| `dark_zone` bypass via debug toggle | `show_full` flag | A cheat-mode flag that bypasses dark-zone gating | `show_full` is a DEV/debug feature. Gate behind `#[cfg(feature = "dev")]` if it must NEVER ship enabled. | A runtime resource flag exposed in shipping builds with no UI guard |

### Trust Boundaries

For Feature #10 specifically:

- **MovedEvent input boundary:** `MovedEvent` originates from `handle_dungeon_input` which itself reads `Res<ActionState<DungeonAction>>` — already validated by leafwing's input chain. No further validation needed for `MovedEvent.to.x`/`y` because `handle_dungeon_input` already bounds-checks against `floor.width`/`floor.height` at line 649-655.
- **DungeonFloor asset boundary:** Read-only. Painter pulls `floor.walls[y][x]` after `is_well_formed()` (Feature #4 invariant). Out-of-bounds reads here are an asset-validation failure — Feature #3/#4's responsibility, not #10's.
- **No new boundaries introduced.** #10 reads existing data + writes one new Resource. Save-game serialization (#23 boundary) is the next concern.

---

## Performance

| Metric | Value / Range | Source | Notes |
|--------|---------------|--------|-------|
| egui frame time (complex UI) | < 1 ms | Master research §Performance line 1122 | Includes typical menus; painter primitives are below this |
| egui line/rect throughput | "Thousands of primitives easily" | Master research same line | floor_01 at ~144 primitives × 60 fps = ~8.6k ops/s; well within budget |
| `ExploredCells` HashMap insert | O(1) amortized (ahash) | hashbrown docs | One insert per `MovedEvent` — negligible |
| `bevy_egui` clean compile | +2–4s per roadmap line 569 | Roadmap estimate (training-data MEDIUM) | Verify post-add — egui pulls a non-trivial dep tree (egui itself, epaint, ecolor, emath, ahash) |
| Bundle size impact (debug binary) | +2–4 MB estimate | Training data MEDIUM | egui crates are pure Rust; LTO release builds compress well |

**Performance is NOT a concern at floor_01 scale.** The painter throughput question only becomes load-bearing for >5000-segment maps; floor_01 is two orders of magnitude under.

---

## Code Examples

### Verified pattern: Reading `MovedEvent` with `MessageReader`

```rust
// Source: src/plugins/audio/sfx.rs (Feature #6 SFX consumer pattern)
// https://github.com/codeinaire/druum (private — local file path)
//
// MessageReader is the Bevy 0.18 idiom; same shape works for MovedEvent.

fn update_explored_on_move(
    mut moved: MessageReader<MovedEvent>,
    mut explored: ResMut<ExploredCells>,
    floors: Res<Assets<DungeonFloor>>,
    dungeon_assets: Option<Res<DungeonAssets>>,
) {
    // Collect-into-vec idiom not needed; MessageReader::read() works directly.
    for ev in moved.read() {
        // ... per-message logic ...
    }
}
```

### Verified pattern: Plugin sibling registration in main.rs

```rust
// Source: src/main.rs:11-34 (current Druum plugin tree).
// Add ONE line for MinimapPlugin after DungeonPlugin.

App::new()
    .add_plugins((
        DefaultPlugins.set(/* ... */),
        StatePlugin,
        ActionsPlugin,
        LoadingPlugin,
        DungeonPlugin,
        MinimapPlugin,    // <-- NEW
        CombatPlugin,
        // ... rest unchanged
    ))
    .run();
```

### Verified pattern: SubState OnEnter/OnExit (pre-existing in StatePlugin)

```rust
// Source: src/plugins/state/mod.rs:17-26 (DungeonSubState declaration).
// Map variant is already declared (line 23). OnEnter/OnExit syntax just works.

.add_systems(OnEnter(DungeonSubState::Map), |/* sys */| { /* setup */ })
.add_systems(OnExit(DungeonSubState::Map), |/* sys */| { /* teardown */ })
```

### MEDIUM-confidence pattern: bevy_egui painter call (Step C verifies)

```rust
// Source: training data — bevy_egui 0.30+ pattern.
// VERIFY against the resolved crate's examples directory in Step C.

use bevy_egui::{egui, EguiContexts};

fn paint_minimap_overlay(mut contexts: EguiContexts, /* ... */) {
    // Most modern bevy_egui versions:
    let Ok(ctx) = contexts.ctx_mut() else { return };

    egui::Window::new("minimap")
        .anchor(egui::Align2::RIGHT_TOP, [-10.0, 10.0])
        .fixed_size([200.0, 200.0])
        .show(ctx, |ui| {
            let painter = ui.painter();
            painter.rect_filled(/* ... */);
            painter.line_segment([egui::pos2(0.0, 0.0), egui::pos2(10.0, 10.0)],
                                  egui::Stroke::new(1.0, egui::Color32::WHITE));
        });
}
```

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `EventReader<MovedEvent>` | `MessageReader<MovedEvent>` | Bevy 0.18 family rename | Use `add_message::<T>()` not `add_event::<T>()`; `MessageReader` not `EventReader` (already done in Druum codebase) |
| `EguiPlugin` as unit struct | `EguiPlugin { enable_multipass_for_primary_context: bool }` | bevy_egui ~0.27 onward | Constructor takes a config field — verify shape in Step C |
| `EguiContext` Resource | `EguiContexts` SystemParam (single-context) / `EguiContext` Component (per-window) | bevy_egui 0.18 onward | Single-context apps use `EguiContexts`; multi-window needs per-camera component (defer to #25) |
| egui auto-attached to ALL cameras | Explicit per-camera `EguiContext` component opt-in | bevy_egui 0.30+ | Druum has only the `Camera3d` (DungeonCamera) and dev `Camera2d`; default behavior should be fine |
| Manual `Window` ID lookup | egui's primary-window default | Stable across modern versions | No change needed for Druum |

**Deprecated/outdated patterns to avoid:**
- `EventReader<MovedEvent>` — use `MessageReader` (Bevy 0.18).
- Older `bevy_egui::EguiContext` resource access — use `EguiContexts` SystemParam.
- `app.add_event::<MovedEvent>()` — already wrong; the codebase correctly uses `add_message`.

---

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | Rust built-in `#[cfg(test)]` + `cargo test` |
| Config file | None |
| Quick run command | `cargo test --lib plugins::dungeon::minimap` (after the module is created) |
| Full suite command | `cargo test` and `cargo test --features dev` |

### Requirements → Test Map

| Requirement | Behavior | Test Type | Automated Command | File Exists? |
|-------------|----------|-----------|-------------------|--------------|
| `MovedEvent` flips dest cell to Visited | Layer 2 (App + ActionsPlugin + minimap) | Integration | `cargo test minimap_subscriber_flips_cell_on_move` | ❌ needs creating |
| Dark-zone cells skip the update | Layer 2 | Integration | `cargo test minimap_subscriber_skips_dark_zone_cells` | ❌ needs creating |
| Debug "show full map" toggle | Pure (Resource read) | Unit | `cargo test explored_cells_show_full_renders_all_visited` | ❌ needs creating |
| `OpenMap` action transitions to `Map` SubState | Layer 2 with full leafwing chain | Integration | `cargo test open_map_action_transitions_substate` | ❌ needs creating |
| `OpenMap` from `Map` returns to `Exploring` | Layer 2 | Integration | `cargo test open_map_action_toggles_back` | ❌ needs creating |
| Escape from `Map` returns to `Exploring` | Layer 2 | Integration | `cargo test pause_action_exits_map_substate` | ❌ needs creating |
| `ExploredCells` registered as Resource | Smoke | Integration | `cargo test minimap_plugin_registers_explored_cells` | ❌ needs creating |
| Painter renders without panic in `Map` SubState | Smoke (egui requires render context — see Pitfall) | Manual | n/a | Manual smoke deferred |
| Visual correctness (color, layout, anchor) | Manual | Manual | n/a | Manual smoke deferred |

**Test layer pattern (from memory `feedback_bevy_input_test_layers.md`):**
- **Layer 1 (pure helpers):** Coordinate transform helpers (cell rect calculation, world-to-map mapping). No App needed. ~3-5 tests.
- **Layer 2 (App-driven, no leafwing):** `MovedEvent` subscriber. Use `init_resource::<bevy::input::ButtonInput<KeyCode>>()` bypass; write directly to `Messages<MovedEvent>` via `.write(...)`. ~3-4 tests.
- **Layer 2b (App-driven, full leafwing):** Open/close handler. Requires `MinimalPlugins + StatesPlugin + InputPlugin + ActionsPlugin + MinimapPlugin`. Use `KeyCode::KeyM.press(world_mut())`. ~3 tests.
- **Manual smoke:** Egui rendering can be hard to unit-test (it's gated on a window/render context). Defer the visual check to manual.

### Headless egui testing

`bevy_egui` has SOME headless support via egui's own `Context::run()` for unit-testing UI logic, but the bevy_egui SystemParam path needs a window/RenderApp. Best test strategy:

- **Test the data layer thoroughly** (resource updates, dark-zone gating, show-full toggle).
- **Don't try to assert pixels** in unit tests; it's brittle and not what unit tests are for.
- **Manual smoke** confirms the painter draws sensibly: enter Dungeon, walk a few cells, press M, verify map shape matches the dungeon, exit map, repeat.

### Gaps (files to create before implementation)

- [ ] `src/plugins/dungeon/minimap.rs` — new file holding `MinimapPlugin`, `ExploredCells`, `ExploredState`, painter systems, subscriber, open/close handler. Includes `#[cfg(test)] mod tests;` for the unit tests.
- [ ] `src/plugins/dungeon/minimap_tests.rs` (only if minimap.rs grows past ~400 LOC) — sibling tests file mirroring the `dungeon/{mod.rs, tests.rs}` pattern.

If `minimap.rs` stays under ~400 LOC (likely for #10's scope), keep tests inline and follow the precedent set by `audio/{mod.rs,bgm.rs,sfx.rs}` where each file has its own inline test mod.

---

## Open Questions

1. **`bevy_egui` resolved version + Bevy 0.18 compatibility** (HIGH stakes — blocks all of #10)
   - What we know: roadmap says `0.39`, but #3 and #5 both deviated. The bevy_egui crate is at `vladbat00/bevy_egui` on GitHub.
   - What's unclear: which actual version on crates.io supports `bevy = "0.18.x"` as of 2026-05-04. Could be 0.39, 0.40, or even 0.41.
   - Recommendation: Step A — `cargo add bevy_egui --dry-run` BEFORE editing Cargo.toml. If resolved version doesn't accept `bevy = "0.18.x"`, halt + escalate to user with options: (a) wait, (b) use a community fork, (c) reconsider the egui choice (low — no good alternative). Same playbook as Feature #5 Step A.

2. **`bevy_egui` default features audit** (MEDIUM — Δ-deps discipline)
   - What we know: Druum's existing pattern is to opt out of unneeded defaults (e.g., leafwing-input-manager has `default-features = false, features = ["keyboard", "mouse"]`).
   - What's unclear: which `bevy_egui` defaults are heavy. Common candidates: `serde` (theme persistence), `accesskit`, `manage_clipboard`, platform `winit/x11` chains.
   - Recommendation: Step B — read the resolved crate's `[features]` block; opt out of anything Druum doesn't need. If `default = ["render"]` only, keep defaults. Surface for user review.

3. **`EguiPlugin` config shape**
   - What we know: Recent versions take `EguiPlugin { enable_multipass_for_primary_context: bool }` instead of being a unit struct.
   - What's unclear: which form the resolved version has.
   - Recommendation: Step C — grep `pub struct EguiPlugin` in resolved source. Use whatever shape it has. Most likely `enable_multipass_for_primary_context: false` for #10 (single-pass UI is fine; multipass is for complex zoom-and-clip composites).

4. **`EguiContexts` vs `EguiContext` access**
   - What we know: Modern bevy_egui exposes `EguiContexts` as a SystemParam for single-context apps.
   - What's unclear: API exact shape — does `ctx_mut()` return `&mut Context` or `Result<...>`?
   - Recommendation: Step C — grep `pub fn ctx_mut` in resolved source. Adapt code shape.

5. **Reset `ExploredCells` on dungeon-exit?**
   - What we know: The spec wants the explored data to persist across saves (#23) and is keyed by `(floor, x, y)`.
   - What's unclear: should re-entering Dungeon via F9 (dev) wipe or preserve?
   - Recommendation: Preserve. F9 is a dev-only state cycler, not a "new game" trigger. Document the choice in `MinimapPlugin::build`'s docstring. Surface as Category B.

6. **Adjacent-cell visibility on move** (Pitfall 4)
   - What we know: spec says "every MovedEvent" updates a cell.
   - What's unclear: just the destination, or also adjacent visible cells (line-of-sight)?
   - Recommendation: v1 = destination only. Adjacent reveal is #25 polish (visible-cell algorithm requires line-of-sight + door-aware traversal — non-trivial). Surface as Category B.

7. **`KnownByOther` provenance** (no producer in v1)
   - What we know: variant exists in spec.
   - What's unclear: which feature populates it. Item-revealed map (#12)? Spell (#20)?
   - Recommendation: Declare the variant; render it with a slight tint different from `Visited` so dev "show full" is distinguishable from real exploration; document "no producer in v1" in the variant doc. Surface as Category B.

---

## Cleanest-ship signal for #10

A clean ship means:

1. **Cargo.toml diff is exactly +1 line:**
   ```diff
   + bevy_egui = "=<RESOLVED-VERSION>"   # OR with default-features = false per Step B
   ```
   No other dep added. No bevy version drift. No unrelated transitive bumps.

2. **Cargo.lock diff:** the resolved bevy_egui entry plus its direct deps (egui, epaint, ecolor, emath, ahash, possibly clipboard libs). Quantify expected entries via Step C: `grep -E "^name|^version" ~/.cargo/registry/src/.../bevy_egui-X.Y.Z/Cargo.toml | head -50`. The diff should be reviewable in one screen. Anything else (e.g. an unrelated bevy_render bump) means leafwing-style upward unification — STOP and investigate.

3. **`cargo test`:** new test count goes up by ~7 (the Layer 2 tests from §Validation gaps). Existing tests do not break (the new `MinimapPlugin` does not modify existing app shape outside main.rs).

4. **`cargo test --features dev`:** identical pass count to non-dev OR +1 for a dev-only "show full map" test.

5. **`cargo clippy --all-targets -- -D warnings` and `--features dev`:** zero warnings.

6. **`cargo fmt --check`:** zero diff.

7. **Manual smoke (deferred to user):**
   - `cargo run --features dev`, F9 to Dungeon, walk a few cells; press M. Map shows visited cells in the right shape.
   - Walk on top of a `dark_zone: true` cell (need to author one in floor_01 first OR test with show_full toggle). Cell shows "?" not normal exploration mark.
   - Press M again or Escape; back to exploring; minimap overlay shows in top-right corner.
   - F9 cycle out of Dungeon; F9 back in; previously-explored cells still marked.

8. **Verification that nothing else snuck in:**
   ```bash
   git diff --stat HEAD..feature-10-branch | head -20
   ```
   Expect: Cargo.toml +1 line; Cargo.lock the egui tree; `src/plugins/dungeon/minimap.rs` new file; `src/main.rs` +1 line. No unrelated edits to `state/`, `input/`, `loading/`, `audio/`, `data/`, etc.

---

## Sources

### Primary (HIGH confidence)

- [Local file: src/plugins/dungeon/mod.rs (HEAD)](file:///Users/nousunio/Repos/Learnings/claude-code/druum/src/plugins/dungeon/mod.rs) — `MovedEvent` definition (line 192-197), `DungeonPlugin::build` registration (line 207-233), input handler write site (line 678-682), torch flicker pattern as model for the painter system shape
- [Local file: src/plugins/state/mod.rs (HEAD)](file:///Users/nousunio/Repos/Learnings/claude-code/druum/src/plugins/state/mod.rs) — `DungeonSubState` enum with `Map` variant declared (line 17-26)
- [Local file: src/plugins/input/mod.rs (HEAD)](file:///Users/nousunio/Repos/Learnings/claude-code/druum/src/plugins/input/mod.rs) — `DungeonAction::OpenMap` (line 79), bound to `KeyCode::KeyM` (line 150), test patterns for full-leafwing-chain Layer 2 tests
- [Local file: src/data/dungeon.rs (HEAD)](file:///Users/nousunio/Repos/Learnings/claude-code/druum/src/data/dungeon.rs) — `DungeonFloor` public API (line 249-266), `WallType` enum (line 86-104), `CellFeatures::dark_zone` (line 164), `wall_between` and `can_move` signatures
- [Local file: src/plugins/audio/mod.rs (HEAD)](file:///Users/nousunio/Repos/Learnings/claude-code/druum/src/plugins/audio/mod.rs) — sibling-submodule + plugin-aggregator pattern (the precedent for MinimapPlugin's structure)
- [Local file: src/plugins/dungeon/tests.rs (HEAD)](file:///Users/nousunio/Repos/Learnings/claude-code/druum/src/plugins/dungeon/tests.rs) — `make_test_app` test scaffolding pattern (line 150-179), `MovedEvent` count assertion via `Messages<MovedEvent>::iter_current_update_messages()` (line 273-279)
- [Local file: src/main.rs (HEAD)](file:///Users/nousunio/Repos/Learnings/claude-code/druum/src/main.rs) — current plugin tree shape; one-line addition target
- [Local file: project/research/20260326-01-bevy-first-person-dungeon-crawler-rpg.md](file:///Users/nousunio/Repos/Learnings/claude-code/druum/research/20260326-01-bevy-first-person-dungeon-crawler-rpg.md) — master research; §Open Question 5 (line 1301-1304) endorses egui canvas for the minimap; §Don't Hand-Roll line 993 endorses bevy_egui as the framework
- [Local file: project/plans/20260502-000000-feature-5-input-system-leafwing.md](file:///Users/nousunio/Repos/Learnings/claude-code/druum/project/plans/20260502-000000-feature-5-input-system-leafwing.md) — Steps A/B/C verification gate pattern (line 64-127); the precedent #10 must mirror

### Secondary (MEDIUM confidence)

- [bevy_egui crate page](https://crates.io/crates/bevy_egui) — Version-to-Bevy mapping. Accessed: 2026-05-04 (Step A run will produce the canonical answer).
- [bevy_egui GitHub repo](https://github.com/vladbat00/bevy_egui) — Active maintenance, examples directory. Accessed: 2026-05-04.
- [egui crate page](https://crates.io/crates/egui) — Painter API surface (line, rect, text). Accessed: 2026-05-04.
- [Master research §Performance](file:///Users/nousunio/Repos/Learnings/claude-code/druum/research/20260326-01-bevy-first-person-dungeon-crawler-rpg.md) line 1122 — egui frame time benchmarks. Accessed: 2026-05-04.

### Tertiary (LOW confidence — needs verification)

- Training data (assistant pretraining cutoff Jan 2026): bevy_egui 0.39.x was released 2026-01-14 for Bevy 0.18 — **VERIFY in Step A**.
- Training data: `EguiContexts` SystemParam is the single-context idiom in bevy_egui 0.30+ — **VERIFY in Step C**.
- Training data: `EguiPlugin { enable_multipass_for_primary_context: bool }` is the modern struct shape — **VERIFY in Step C**.

### Verification recipes (for the planner / implementer)

```bash
# Step A: resolve the actual current version
cargo add bevy_egui --dry-run 2>&1 | tee /tmp/bevy-egui-resolve.txt

# Step B: feature audit (after Step A confirms a version like 0.40.0)
cargo info bevy_egui --version <RESOLVED> 2>&1 | grep -A 30 "^features"
# OR (after temporarily adding):
cat ~/.cargo/registry/src/index.crates.io-*/bevy_egui-<RESOLVED>/Cargo.toml | sed -n '/^\[features\]/,/^\[/p'

# Step C: API verification grep
REG=~/.cargo/registry/src/index.crates.io-*/bevy_egui-<RESOLVED>/src
grep -rn "pub struct EguiPlugin" $REG | head -3
grep -rn "pub struct EguiContexts" $REG | head -3
grep -rn "pub fn ctx_mut" $REG | head -3
grep -rn "pub trait" $REG | head -10
ls ~/.cargo/registry/src/index.crates.io-*/bevy_egui-<RESOLVED>/examples/
```

---

## Metadata

**Confidence breakdown:**

- Codebase facts (state machine, MovedEvent, OpenMap action, floor public API, plugin patterns): **HIGH** — verified against HEAD source files cited inline.
- Architecture options (plugin module split, ExploredCells location, canvas vs RTT, overlay placement, open/close behavior): **HIGH** — derived from cited Bevy 0.18 patterns and existing Druum conventions.
- bevy_egui API specifics (EguiContexts, EguiPlugin shape, painter signatures): **MEDIUM** — training-data based, gate-verified by Steps A/B/C.
- bevy_egui version compatibility with Bevy 0.18.1: **MEDIUM** — Step A resolves to HIGH.
- Feature-flag audit (default features, what to opt out of): **MEDIUM** — Step B resolves to HIGH.
- Performance estimates: **MEDIUM** — based on master research's egui benchmarks; floor_01 scale is well below any plausible threshold.
- Pitfalls (10 listed): **HIGH** for those grounded in Druum's own past plans/research (Bevy 0.18 family rename, ColorRgb pattern, version drift); **MEDIUM** for egui-specific ones (Pitfalls 6, 7, 9) until Step C verifies.

**Research date:** 2026-05-04
