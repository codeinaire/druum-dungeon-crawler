//! Auto-map / minimap — Feature #10.
//!
//! Subscribes to [`MovedEvent`] and records visited cells in [`ExploredCells`]
//! keyed by `(floor_number, x, y)`. Painted via `bevy_egui` as a top-right
//! 200×200 overlay during [`DungeonSubState::Exploring`] and as a full-screen
//! view during [`DungeonSubState::Map`].
//!
//! ## Architecture
//!
//! `MinimapPlugin` is registered through [`crate::plugins::ui::UiPlugin`]
//! (Decision D1=C). All state is owned by one [`Resource`]: [`ExploredCells`].
//! The resource is NOT reset on `OnExit(GameState::Dungeon)` — cross-floor
//! persistence is intentional. "New game" reset is a Feature #23 concern.
//!
//! ## System ordering
//!
//! `update_explored_on_move` is ordered `.after(handle_dungeon_input)` (Pitfall 3)
//! AND placed in [`MinimapSet`] so the painter systems can order against it.
//! Painters run in [`bevy_egui::EguiPrimaryContextPass`] schedule; the updater
//! and open/close handler run in `Update`. The schedules are naturally ordered
//! by Bevy's main schedule pipeline, so `Update` systems always execute before
//! `EguiPrimaryContextPass`.
//!
//! ## Dark-zone gate
//!
//! When a `MovedEvent.to` cell has `CellFeatures.dark_zone == true`, the
//! subscriber skips the insert. The painter renders `?` for cells that remain
//! [`ExploredState::Unseen`] AND whose feature flag is `dark_zone == true`.

use bevy::ecs::message::MessageReader;
use bevy::prelude::*;
use bevy_egui::{EguiContexts, EguiPrimaryContextPass, PrimaryEguiContext, egui};
use leafwing_input_manager::prelude::ActionState;
use std::collections::HashMap;

use crate::data::DungeonFloor;
use crate::data::dungeon::Direction;
use crate::plugins::dungeon::{
    DungeonCamera, Facing, GridPosition, MovedEvent, PlayerParty, handle_dungeon_input,
};
use crate::plugins::input::DungeonAction;
use crate::plugins::loading::DungeonAssets;
use crate::plugins::state::{DungeonSubState, GameState};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Width and height of the minimap overlay window in logical screen pixels.
pub(crate) const MINIMAP_OVERLAY_SIZE: f32 = 200.0;

/// Padding from the screen edge in logical pixels.
pub(crate) const MINIMAP_OVERLAY_PAD: f32 = 10.0;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Exploration state of a single dungeon cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ExploredState {
    /// Cell has never been entered.
    #[default]
    Unseen,
    /// Cell has been entered by the player party.
    Visited,
    /// Cell is known via an external source (scroll, telepathy, companion).
    ///
    /// Variant declared in Feature #10 but not produced by any system yet —
    /// Features #12 / #20 will populate this variant at runtime.
    KnownByOther,
}

/// Per-crate-session record of which dungeon cells the party has visited.
///
/// Key: `(floor_number, grid_x, grid_y)`. NOT reset on `OnExit(GameState::Dungeon)` —
/// floors persist across the F9 dev-cycle and across Town↔Dungeon transitions.
/// Reset on "new game" is a Feature #23 / save-integration concern.
///
/// ## Security
///
/// Feature #23 MUST bound `cells.len()` before deserializing a save — an
/// untrusted save could inject billions of entries here.
#[derive(Resource, Default, Debug, Clone)]
pub struct ExploredCells {
    /// Visited cells keyed by `(floor_number, x, y)`.
    pub cells: HashMap<(u32, u32, u32), ExploredState>,
    /// When `true`, the painter renders all cells as [`ExploredState::Visited`]
    /// regardless of their actual state. Dev-only cheat-mode. The field does not
    /// compile in non-dev builds; the painter's branch on `cfg!(feature = "dev")`
    /// evaluates to `false` there.
    #[cfg(feature = "dev")]
    pub show_full: bool,
}

/// System set used to enforce ordering: updater runs before painters.
///
/// Both painter systems are members of this set and call
/// `.after(update_explored_on_move)`. This ensures cells updated in the same
/// frame are visible in the same-frame render (Pitfall 10).
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct MinimapSet;

// ---------------------------------------------------------------------------
// Plugin
// ---------------------------------------------------------------------------

pub struct MinimapPlugin;

impl Plugin for MinimapPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ExploredCells>()
            .add_systems(
                Update,
                (
                    // Idempotent (Without<PrimaryEguiContext> filter) — runs each
                    // frame in Dungeon but is a no-op once attached. Cannot use
                    // OnEnter because spawn_party_and_camera's Commands::spawn
                    // is deferred — the Camera3d entity isn't queryable until
                    // OnEnter's apply_deferred completes.
                    attach_egui_to_dungeon_camera.run_if(in_state(GameState::Dungeon)),
                    update_explored_on_move
                        .run_if(in_state(GameState::Dungeon))
                        .after(handle_dungeon_input)
                        .in_set(MinimapSet),
                    handle_map_open_close.run_if(in_state(GameState::Dungeon)),
                ),
            )
            .add_systems(
                EguiPrimaryContextPass,
                (
                    // Painter ordering vs `update_explored_on_move` is guaranteed
                    // by schedule topology — the updater runs in `Update`,
                    // painters run in `EguiPrimaryContextPass` (after `PostUpdate`).
                    // No `.after()` constraint needed (and Bevy 0.18.1 silently
                    // ignores cross-schedule ordering anyway:
                    // bevy_ecs-0.18.1/src/schedule/config.rs:358).
                    paint_minimap_overlay
                        .run_if(in_state(DungeonSubState::Exploring))
                        .in_set(MinimapSet),
                    paint_minimap_full
                        .run_if(in_state(DungeonSubState::Map))
                        .in_set(MinimapSet),
                ),
            );

        #[cfg(feature = "dev")]
        app.add_systems(
            Update,
            toggle_show_full_map.run_if(in_state(GameState::Dungeon)),
        );
    }
}

// ---------------------------------------------------------------------------
// Camera setup
// ---------------------------------------------------------------------------

/// Attaches `PrimaryEguiContext` to the dungeon `Camera3d` so the minimap
/// painters' `EguiContexts::ctx_mut()` resolves to a real context.
///
/// `UiPlugin` disables `bevy_egui`'s default auto-attach (which would otherwise
/// permanently bind `PrimaryEguiContext` to the FIRST camera spawned — the
/// loading-screen `Camera2d`, which is despawned before the dungeon Camera3d
/// arrives, leaving every later camera without a context). Each plugin that
/// needs egui therefore attaches the marker to its own camera.
///
/// Runs in `Update` (gated on `GameState::Dungeon`), not `OnEnter`, because
/// `spawn_party_and_camera`'s `Commands::spawn` is deferred — the `Camera3d`
/// entity isn't queryable until OnEnter's apply_deferred completes. The
/// `Without<PrimaryEguiContext>` filter makes this a per-frame no-op once
/// attached; F9 cycles re-attach the new camera on the next Update tick.
fn attach_egui_to_dungeon_camera(
    mut commands: Commands,
    cameras: Query<Entity, (With<DungeonCamera>, Without<PrimaryEguiContext>)>,
) {
    for entity in &cameras {
        commands.entity(entity).insert(PrimaryEguiContext);
    }
}

// ---------------------------------------------------------------------------
// Update systems
// ---------------------------------------------------------------------------

/// Marks the destination cell of each `MovedEvent` as `Visited`.
///
/// Gated on `GameState::Dungeon`. Skips cells where `CellFeatures.dark_zone`
/// is `true` (Pitfall 8) — those cells stay `Unseen` and render `?` on the map.
/// Ordered `.after(handle_dungeon_input)` (Pitfall 3) so the event is already
/// in the message queue when this system runs.
fn update_explored_on_move(
    mut moved: MessageReader<MovedEvent>,
    floors: Res<Assets<DungeonFloor>>,
    dungeon_assets: Option<Res<DungeonAssets>>,
    mut explored: ResMut<ExploredCells>,
) {
    let Some(assets) = dungeon_assets else {
        return;
    };
    let Some(floor) = floors.get(&assets.floor_01) else {
        return;
    };

    for ev in moved.read() {
        let x = ev.to.x as usize;
        let y = ev.to.y as usize;
        if floor.features[y][x].dark_zone {
            // Dark-zone: skip insert. Cell stays Unseen; painter renders `?`.
            continue;
        }
        explored.cells.insert(
            (floor.floor_number, ev.to.x, ev.to.y),
            ExploredState::Visited,
        );
    }
}

/// Toggles `DungeonSubState` between `Exploring` and `Map` on `OpenMap` press.
/// `Pause` (Escape) also exits the `Map` substate.
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

/// Dev-only: F8 toggles `show_full` on `ExploredCells`.
///
/// F9 is reserved for the game-state cycler. F8 is chosen for minimap debug.
/// The system AND the field are both `#[cfg(feature = "dev")]`-gated; neither
/// compiles in release builds. The painter's `cfg!(feature = "dev")` branch is
/// similarly gated.
#[cfg(feature = "dev")]
fn toggle_show_full_map(keys: Res<ButtonInput<KeyCode>>, mut explored: ResMut<ExploredCells>) {
    if keys.just_pressed(KeyCode::F8) {
        explored.show_full = !explored.show_full;
        info!("Minimap show_full toggled to {}", explored.show_full);
    }
}

// ---------------------------------------------------------------------------
// Painter systems (EguiPrimaryContextPass)
// ---------------------------------------------------------------------------

/// Overlay painter: 200×200 area anchored to the top-right corner.
/// Runs during `DungeonSubState::Exploring`.
///
/// Uses `egui::Area` (not `Window`) — `Window` + `Frame::NONE` interacts
/// poorly with auto-sizing and produces a zero-size content rect. `Area` is
/// the correct egui primitive for an absolutely-positioned HUD overlay.
fn paint_minimap_overlay(
    mut contexts: EguiContexts,
    explored: Res<ExploredCells>,
    floors: Res<Assets<DungeonFloor>>,
    dungeon_assets: Option<Res<DungeonAssets>>,
    party: Query<(&GridPosition, &Facing), With<PlayerParty>>,
) -> Result {
    let ctx = contexts.ctx_mut()?;
    let Some(assets) = dungeon_assets else {
        return Ok(());
    };
    let Some(floor) = floors.get(&assets.floor_01) else {
        return Ok(());
    };
    let Ok((pos, facing)) = party.single() else {
        return Ok(());
    };

    let size = egui::Vec2::splat(MINIMAP_OVERLAY_SIZE);
    egui::Area::new(egui::Id::new("minimap_overlay"))
        .anchor(
            egui::Align2::RIGHT_TOP,
            [-MINIMAP_OVERLAY_PAD, MINIMAP_OVERLAY_PAD],
        )
        .show(ctx, |ui| {
            let (rect, _response) = ui.allocate_exact_size(size, egui::Sense::hover());
            // Translucent dark background so the overlay reads against bright scenes.
            ui.painter().rect_filled(
                rect,
                0.0,
                egui::Color32::from_rgba_unmultiplied(0, 0, 0, 160),
            );
            paint_floor_into(ui.painter(), rect, floor, &explored, *pos, facing.0);
        });

    Ok(())
}

/// Full-screen painter: fills the central panel.
/// Runs during `DungeonSubState::Map`.
fn paint_minimap_full(
    mut contexts: EguiContexts,
    explored: Res<ExploredCells>,
    floors: Res<Assets<DungeonFloor>>,
    dungeon_assets: Option<Res<DungeonAssets>>,
    party: Query<(&GridPosition, &Facing), With<PlayerParty>>,
) -> Result {
    let ctx = contexts.ctx_mut()?;
    let Some(assets) = dungeon_assets else {
        return Ok(());
    };
    let Some(floor) = floors.get(&assets.floor_01) else {
        return Ok(());
    };
    let Ok((pos, facing)) = party.single() else {
        return Ok(());
    };

    egui::CentralPanel::default()
        .frame(egui::Frame::new().fill(egui::Color32::from_rgb(20, 20, 30)))
        .show(ctx, |ui| {
            let rect = ui.max_rect();
            paint_floor_into(ui.painter(), rect, floor, &explored, *pos, facing.0);
        });

    Ok(())
}

// ---------------------------------------------------------------------------
// Shared paint helper
// ---------------------------------------------------------------------------

/// Shared floor-painting helper. Called by both overlay and full-screen painters.
///
/// Renders cells, walls, dark-zone `?` glyphs, and a player-position arrow.
fn paint_floor_into(
    painter: &egui::Painter,
    rect: egui::Rect,
    floor: &DungeonFloor,
    explored: &ExploredCells,
    pos: GridPosition,
    facing: Direction,
) {
    if floor.width == 0 || floor.height == 0 {
        return;
    }

    let cell_size = (rect.width() / floor.width as f32).min(rect.height() / floor.height as f32);

    // Centre the grid in the rect.
    let grid_w = cell_size * floor.width as f32;
    let grid_h = cell_size * floor.height as f32;
    let origin = egui::pos2(
        rect.min.x + (rect.width() - grid_w) * 0.5,
        rect.min.y + (rect.height() - grid_h) * 0.5,
    );

    let wall_color = egui::Color32::from_rgb(180, 150, 100);
    let wall_stroke = egui::Stroke::new(1.5, wall_color);

    for y in 0..floor.height {
        for x in 0..floor.width {
            let cell_rect = cell_rect_for(origin, cell_size, x, y);

            // Determine effective exploration state.
            #[cfg(feature = "dev")]
            let effective_state = if explored.show_full {
                ExploredState::Visited
            } else {
                explored
                    .cells
                    .get(&(floor.floor_number, x, y))
                    .copied()
                    .unwrap_or_default()
            };
            #[cfg(not(feature = "dev"))]
            let effective_state = explored
                .cells
                .get(&(floor.floor_number, x, y))
                .copied()
                .unwrap_or_default();

            // Cell fill per exploration state.
            let fill = match effective_state {
                ExploredState::Unseen => egui::Color32::TRANSPARENT,
                ExploredState::Visited => egui::Color32::from_rgb(60, 60, 70),
                ExploredState::KnownByOther => egui::Color32::from_rgb(50, 50, 100),
            };
            if fill != egui::Color32::TRANSPARENT {
                painter.rect_filled(cell_rect, 0.0, fill);
            }

            // Dark-zone `?` glyph for unseen dark-zone cells.
            let dark_zone = floor.features[y as usize][x as usize].dark_zone;
            if dark_zone && effective_state == ExploredState::Unseen {
                painter.text(
                    cell_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    "?",
                    egui::FontId::proportional(cell_size * 0.6),
                    egui::Color32::from_rgb(200, 100, 100),
                );
            }

            // Walls: north + west edges for each cell; south/east for boundary cells.
            let walls = &floor.walls[y as usize][x as usize];
            if is_solid(walls.north) {
                painter.line_segment([cell_rect.left_top(), cell_rect.right_top()], wall_stroke);
            }
            if is_solid(walls.west) {
                painter.line_segment([cell_rect.left_top(), cell_rect.left_bottom()], wall_stroke);
            }
            if y == floor.height - 1 && is_solid(walls.south) {
                painter.line_segment(
                    [cell_rect.left_bottom(), cell_rect.right_bottom()],
                    wall_stroke,
                );
            }
            if x == floor.width - 1 && is_solid(walls.east) {
                painter.line_segment(
                    [cell_rect.right_top(), cell_rect.right_bottom()],
                    wall_stroke,
                );
            }
        }
    }

    // Player arrow: small triangle pointing in facing direction.
    let cx = origin.x + (pos.x as f32 + 0.5) * cell_size;
    let cy = origin.y + (pos.y as f32 + 0.5) * cell_size;
    let r = cell_size * 0.35;
    let arrow_pts = arrow_triangle(egui::pos2(cx, cy), r, facing);
    painter.add(egui::Shape::convex_polygon(
        arrow_pts,
        egui::Color32::from_rgb(255, 230, 80),
        egui::Stroke::NONE,
    ));
}

// ---------------------------------------------------------------------------
// Pure helpers
// ---------------------------------------------------------------------------

/// Compute the egui `Rect` for cell `(x, y)` given `origin` and `cell_size`.
///
/// `origin` is the top-left corner of the grid in egui screen coordinates.
/// `x` increases rightward, `y` increases downward (matches the y-down grid
/// convention from `data/dungeon.rs`).
fn cell_rect_for(origin: egui::Pos2, cell_size: f32, x: u32, y: u32) -> egui::Rect {
    let tl = egui::pos2(
        origin.x + x as f32 * cell_size,
        origin.y + y as f32 * cell_size,
    );
    egui::Rect::from_min_size(tl, egui::Vec2::splat(cell_size))
}

/// Returns `true` if the wall type should be drawn as a solid line.
///
/// Regular `Door` is intentionally OMITTED — doors render as open passages
/// on the map (visually identical to `Open`). Distinct door rendering (open/
/// closed/locked icons) is Feature #13's scope. `LockedDoor` and `SecretWall`
/// render as solid because the player can't currently traverse them.
fn is_solid(w: crate::data::dungeon::WallType) -> bool {
    use crate::data::dungeon::WallType;
    matches!(
        w,
        WallType::Solid | WallType::LockedDoor | WallType::SecretWall
    )
}

/// Compute three triangle vertices for a player-direction arrow centered at `center`.
///
/// The triangle points in `facing` direction with a circumradius of `r`.
/// Facing convention: `North = -Y` screen (upward), `East = +X`, etc.
fn arrow_triangle(center: egui::Pos2, r: f32, facing: Direction) -> Vec<egui::Pos2> {
    // Base angle: tip of the arrow in screen-space for each direction.
    // Screen y-down: North is -π/2 (upward on screen), East is 0, South is π/2, West is π.
    use std::f32::consts::{FRAC_PI_2, PI};
    let angle = match facing {
        Direction::North => -FRAC_PI_2,
        Direction::East => 0.0,
        Direction::South => FRAC_PI_2,
        Direction::West => PI,
    };
    // Triangle: tip at `angle`, base vertices at `angle ± 2π/3`.
    let tip = egui::pos2(center.x + r * angle.cos(), center.y + r * angle.sin());
    let left = egui::pos2(
        center.x + r * 0.6 * (angle + 2.4).cos(),
        center.y + r * 0.6 * (angle + 2.4).sin(),
    );
    let right = egui::pos2(
        center.x + r * 0.6 * (angle - 2.4).cos(),
        center.y + r * 0.6 * (angle - 2.4).sin(),
    );
    vec![tip, left, right]
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::asset::AssetPlugin;
    use bevy::state::app::StatesPlugin;
    use leafwing_input_manager::prelude::ActionState;

    // -----------------------------------------------------------------------
    // Layer 1: pure-function tests (no App)
    // -----------------------------------------------------------------------

    #[test]
    fn cell_rect_for_origin_zero() {
        let origin = egui::pos2(0.0, 0.0);
        let r = cell_rect_for(origin, 10.0, 0, 0);
        assert!((r.min.x - 0.0).abs() < 1e-6);
        assert!((r.min.y - 0.0).abs() < 1e-6);
        assert!((r.max.x - 10.0).abs() < 1e-6);
        assert!((r.max.y - 10.0).abs() < 1e-6);
    }

    #[test]
    fn cell_rect_for_nonzero_origin_and_position() {
        let origin = egui::pos2(5.0, 8.0);
        let r = cell_rect_for(origin, 20.0, 2, 3);
        assert!((r.min.x - (5.0 + 2.0 * 20.0)).abs() < 1e-6);
        assert!((r.min.y - (8.0 + 3.0 * 20.0)).abs() < 1e-6);
        assert!((r.max.x - (5.0 + 3.0 * 20.0)).abs() < 1e-6);
        assert!((r.max.y - (8.0 + 4.0 * 20.0)).abs() < 1e-6);
    }

    #[test]
    fn floor_number_keys_are_distinct() {
        let mut map: HashMap<(u32, u32, u32), ExploredState> = HashMap::default();
        map.insert((0, 1, 2), ExploredState::Visited);
        map.insert((1, 1, 2), ExploredState::KnownByOther);
        assert_eq!(map.get(&(0, 1, 2)), Some(&ExploredState::Visited));
        assert_eq!(map.get(&(1, 1, 2)), Some(&ExploredState::KnownByOther));
    }

    #[test]
    fn explored_state_default_is_unseen() {
        assert_eq!(ExploredState::default(), ExploredState::Unseen);
    }

    #[test]
    fn known_by_other_variant_exists() {
        // Compile-time check that the variant is declared.
        let _: ExploredState = ExploredState::KnownByOther;
    }

    // -----------------------------------------------------------------------
    // Layer 2: App-driven tests (MinimapPlugin, no DungeonPlugin)
    //
    // We do NOT include ActionsPlugin in this test app because leafwing's
    // InputManagerPlugin registers mouse-input systems that require
    // AccumulatedMouseMotion — a resource provided by Bevy's InputPlugin.
    // Including InputPlugin would clear just_pressed in PreUpdate before our
    // Update systems can observe it (breaking the F8 toggle test).
    //
    // Instead, ActionState<DungeonAction> is inserted as a bare resource via
    // init_resource::<ActionState<DungeonAction>>() so handle_map_open_close
    // can read it. We mutate it directly in tests to simulate presses.
    // This is the same approach used by the dungeon and state tests.
    // -----------------------------------------------------------------------

    fn make_test_app() -> App {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            StatesPlugin,
            crate::plugins::state::StatePlugin,
        ))
        // ActionState<DungeonAction> is needed by handle_map_open_close.
        // Insert it directly without ActionsPlugin (avoids mouse-resource panic
        // from leafwing's AccumulatedMouseMotion dependency on InputPlugin).
        .init_resource::<ActionState<DungeonAction>>()
        // Assets<DungeonFloor> needed by update_explored_on_move.
        .init_asset::<DungeonFloor>()
        .add_message::<MovedEvent>()
        .add_plugins(MinimapPlugin);
        // ButtonInput<KeyCode> required by StatePlugin::build under --features dev
        // (cycle_game_state_on_f9). Insert WITHOUT InputPlugin so
        // keyboard_input_system does not clear just_pressed before Update runs.
        #[cfg(feature = "dev")]
        app.init_resource::<ButtonInput<KeyCode>>();
        app.update();
        app
    }

    /// `MinimapPlugin` inserts `ExploredCells` as a resource.
    #[test]
    fn plugin_registers_explored_cells() {
        let app = make_test_app();
        assert!(
            app.world().contains_resource::<ExploredCells>(),
            "ExploredCells must be registered by MinimapPlugin"
        );
    }

    /// A `MovedEvent` arrives but `DungeonAssets` is absent — subscriber
    /// must early-return without panic and leave `ExploredCells` empty.
    /// (LoadingPlugin omitted to avoid .ogg hang in headless tests.)
    #[test]
    fn subscriber_early_returns_when_dungeon_assets_absent() {
        use crate::data::dungeon::Direction;

        let mut app = make_test_app();

        // Drive GameState to Dungeon so the system's run_if gate passes.
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Dungeon);
        app.update(); // state transition

        // Write a MovedEvent.
        app.world_mut()
            .resource_mut::<bevy::ecs::message::Messages<MovedEvent>>()
            .write(MovedEvent {
                from: GridPosition { x: 0, y: 0 },
                to: GridPosition { x: 1, y: 0 },
                facing: Direction::East,
            });
        // DungeonAssets is absent — subscriber early-returns safely.
        app.update();

        let explored = app.world().resource::<ExploredCells>();
        assert_eq!(explored.cells.len(), 0);
    }

    /// Happy path: with `DungeonAssets` + a loaded `DungeonFloor` present,
    /// a `MovedEvent` to a non-dark-zone cell flips that cell to `Visited`.
    #[test]
    fn subscriber_flips_dest_cell_to_visited_with_assets() {
        use crate::data::DungeonFloor;
        use crate::data::dungeon::{CellFeatures, Direction, LightingConfig, WallMask};
        use crate::plugins::loading::DungeonAssets;

        let mut app = make_test_app();

        // Build + insert a 2×2 floor with no dark zones.
        let floor = DungeonFloor {
            name: "test".into(),
            width: 2,
            height: 2,
            floor_number: 7,
            walls: vec![vec![WallMask::default(); 2]; 2],
            features: vec![vec![CellFeatures::default(); 2]; 2],
            entry_point: (0, 0, Direction::North),
            encounter_table: "test".into(),
            lighting: LightingConfig::default(),
            locked_doors: Vec::new(),
        };
        let handle = app
            .world_mut()
            .resource_mut::<Assets<DungeonFloor>>()
            .add(floor);
        app.world_mut().insert_resource(DungeonAssets {
            floor_01: handle,
            item_db: Handle::default(),
            enemy_db: Handle::default(),
            class_table: Handle::default(),
            spell_table: Handle::default(),
        });

        // Drive into Dungeon and emit a move to (1, 0).
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Dungeon);
        app.update();
        app.world_mut()
            .resource_mut::<bevy::ecs::message::Messages<MovedEvent>>()
            .write(MovedEvent {
                from: GridPosition { x: 0, y: 0 },
                to: GridPosition { x: 1, y: 0 },
                facing: Direction::East,
            });
        app.update();

        let explored = app.world().resource::<ExploredCells>();
        assert_eq!(explored.cells.len(), 1, "exactly one cell should be marked");
        assert_eq!(
            explored.cells.get(&(7, 1, 0)).copied(),
            Some(ExploredState::Visited),
            "destination cell (floor 7, x=1, y=0) must be Visited"
        );
    }

    /// Smoke-test bug #1 regression guard: `attach_egui_to_dungeon_camera`
    /// must insert `PrimaryEguiContext` on the dungeon `Camera3d` after
    /// state transitions to `Dungeon`. Without this, `EguiContexts::ctx_mut()`
    /// returns Err and painters silently no-op (no minimap renders).
    #[test]
    fn attach_egui_to_dungeon_camera_attaches_marker() {
        use crate::plugins::dungeon::DungeonCamera;

        let mut app = make_test_app();

        // Spawn a stand-in dungeon camera (no Camera3d needed — the attach
        // system filters on DungeonCamera).
        let camera = app.world_mut().spawn(DungeonCamera).id();

        // Drive into Dungeon so the attach system's run_if gate passes.
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Dungeon);
        app.update(); // state transition realises
        app.update(); // attach_egui_to_dungeon_camera runs

        let count = app
            .world_mut()
            .query_filtered::<Entity, With<bevy_egui::PrimaryEguiContext>>()
            .iter(app.world())
            .count();
        assert_eq!(
            count, 1,
            "PrimaryEguiContext must be attached to the dungeon camera"
        );
        assert!(
            app.world()
                .entity(camera)
                .contains::<bevy_egui::PrimaryEguiContext>(),
            "marker must land on the DungeonCamera entity specifically"
        );
    }

    /// When `ExploredCells` starts empty and no event is sent, it stays empty.
    #[test]
    fn subscriber_does_not_touch_other_cells() {
        let mut app = make_test_app();
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Dungeon);
        app.update();
        app.update();
        let explored = app.world().resource::<ExploredCells>();
        assert_eq!(explored.cells.len(), 0);
    }

    /// `show_full` toggle does not affect the `cells` map — it only affects rendering.
    #[cfg(feature = "dev")]
    #[test]
    fn show_full_does_not_mutate_cells() {
        let mut app = make_test_app();
        app.world_mut().resource_mut::<ExploredCells>().show_full = true;
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Dungeon);
        app.update();
        // No DungeonAssets present — cells stays empty. show_full is view-only.
        let explored = app.world().resource::<ExploredCells>();
        assert_eq!(explored.cells.len(), 0);
    }

    // -----------------------------------------------------------------------
    // Layer 2b: open/close handler tests
    //
    // `handle_map_open_close` reads Res<ActionState<DungeonAction>>.
    // We insert ActionState directly (no ActionsPlugin/InputPlugin) and mutate
    // it with .press() to simulate keypresses. This avoids the mouse-resource
    // panic while still exercising the real system logic.
    // -----------------------------------------------------------------------

    fn make_map_toggle_app() -> App {
        let mut app = make_test_app();
        // Advance to Dungeon state (which activates DungeonSubState).
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Dungeon);
        app.update(); // → Dungeon, DungeonSubState initializes to Exploring
        app.update(); // settle
        app
    }

    /// Pressing `OpenMap` in `Exploring` transitions to `Map`.
    ///
    /// Pattern for ActionState without InputManagerPlugin:
    /// 1. Press (JustPressed)
    /// 2. update() — system fires, queues state change
    /// 3. Release immediately to prevent the system from seeing JustPressed again
    /// 4. update() — StateTransition realizes the queued state change
    #[test]
    fn open_map_action_transitions_substate() {
        let mut app = make_map_toggle_app();

        // Verify we start in Exploring.
        assert_eq!(
            *app.world().resource::<State<DungeonSubState>>(),
            DungeonSubState::Exploring,
            "should start in Exploring"
        );

        // Press OpenMap; immediately release after 1 update so the JustPressed
        // state doesn't fire the system a second time before StateTransition.
        app.world_mut()
            .resource_mut::<ActionState<DungeonAction>>()
            .press(&DungeonAction::OpenMap);
        app.update(); // handle_map_open_close fires, queues NextState::Map
        // Release so the second update doesn't re-trigger the handler.
        app.world_mut()
            .resource_mut::<ActionState<DungeonAction>>()
            .release(&DungeonAction::OpenMap);
        app.update(); // StateTransition realizes Map

        assert_eq!(
            *app.world().resource::<State<DungeonSubState>>(),
            DungeonSubState::Map,
            "OpenMap should transition to Map substate"
        );
    }

    /// Pressing `OpenMap` again in `Map` returns to `Exploring`.
    #[test]
    fn open_map_action_toggles_back() {
        let mut app = make_map_toggle_app();

        // First, go to Map (press, update, release, update).
        app.world_mut()
            .resource_mut::<ActionState<DungeonAction>>()
            .press(&DungeonAction::OpenMap);
        app.update();
        app.world_mut()
            .resource_mut::<ActionState<DungeonAction>>()
            .release(&DungeonAction::OpenMap);
        app.update();
        assert_eq!(
            *app.world().resource::<State<DungeonSubState>>(),
            DungeonSubState::Map,
        );

        // Toggle back to Exploring.
        app.world_mut()
            .resource_mut::<ActionState<DungeonAction>>()
            .press(&DungeonAction::OpenMap);
        app.update();
        app.world_mut()
            .resource_mut::<ActionState<DungeonAction>>()
            .release(&DungeonAction::OpenMap);
        app.update();

        assert_eq!(
            *app.world().resource::<State<DungeonSubState>>(),
            DungeonSubState::Exploring,
            "second OpenMap press should return to Exploring"
        );
    }

    /// `Pause` action in `Map` returns to `Exploring`.
    #[test]
    fn pause_action_exits_map_substate() {
        let mut app = make_map_toggle_app();

        // Enter Map.
        app.world_mut()
            .resource_mut::<ActionState<DungeonAction>>()
            .press(&DungeonAction::OpenMap);
        app.update();
        app.world_mut()
            .resource_mut::<ActionState<DungeonAction>>()
            .release(&DungeonAction::OpenMap);
        app.update();
        assert_eq!(
            *app.world().resource::<State<DungeonSubState>>(),
            DungeonSubState::Map,
        );

        // Press Pause (Escape) to exit Map.
        app.world_mut()
            .resource_mut::<ActionState<DungeonAction>>()
            .press(&DungeonAction::Pause);
        app.update();
        app.world_mut()
            .resource_mut::<ActionState<DungeonAction>>()
            .release(&DungeonAction::Pause);
        app.update();

        assert_eq!(
            *app.world().resource::<State<DungeonSubState>>(),
            DungeonSubState::Exploring,
            "Pause in Map should return to Exploring"
        );
    }

    /// dev-only: pressing F8 toggles `show_full`.
    #[cfg(feature = "dev")]
    #[test]
    fn show_full_toggle_flips_field() {
        let mut app = make_test_app();
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Dungeon);
        app.update();

        // Ensure show_full starts false.
        assert!(
            !app.world().resource::<ExploredCells>().show_full,
            "show_full should start false"
        );

        // Press F8 via ButtonInput (no InputPlugin — just_pressed survives to Update).
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::F8);
        app.update();

        assert!(
            app.world().resource::<ExploredCells>().show_full,
            "F8 should toggle show_full to true"
        );
    }
}
