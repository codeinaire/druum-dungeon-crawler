use bevy::prelude::*;

/// UI shell plugin — HUD, menus, dialogs.
/// Empty for Feature #1; systems land in Features #18 and beyond.
pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, _app: &mut App) {}
}
