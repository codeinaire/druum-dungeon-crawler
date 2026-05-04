use bevy::prelude::*;
use bevy_egui::EguiPlugin;

pub mod minimap;
pub use minimap::MinimapPlugin;

/// UI shell plugin — HUD, menus, dialogs.
/// `MinimapPlugin` (Feature #10) is the first concrete tenant; further systems
/// land in Features #18 and beyond.
pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(EguiPlugin::default())
            .add_plugins(MinimapPlugin);
    }
}
