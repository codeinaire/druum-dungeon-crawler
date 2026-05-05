use bevy::prelude::*;
use bevy_egui::{EguiGlobalSettings, EguiPlugin};

pub mod minimap;
pub use minimap::MinimapPlugin;

/// UI shell plugin — HUD, menus, dialogs.
/// `MinimapPlugin` (Feature #10) is the first concrete tenant; further systems
/// land in Features #18 and beyond.
pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(EguiPlugin::default())
            // Disable bevy_egui's auto-attach. By default it inserts
            // `PrimaryEguiContext` on the FIRST camera spawned (the loading
            // screen `Camera2d`); when that camera is despawned the context
            // goes with it and `EguiContexts::ctx_mut()` returns Err — silently
            // breaking the minimap painters. Each plugin that needs egui
            // attaches `PrimaryEguiContext` to its own camera instead
            // (see `MinimapPlugin::attach_egui_to_dungeon_camera`).
            .insert_resource(EguiGlobalSettings {
                auto_create_primary_context: false,
                ..default()
            })
            .add_plugins(MinimapPlugin);
    }
}
