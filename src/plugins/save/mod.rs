use bevy::prelude::*;

/// Save/load plugin — persistence to disk.
/// Empty for Feature #1; contents land in Feature #23 (save/load).
pub struct SavePlugin;

impl Plugin for SavePlugin {
    fn build(&self, _app: &mut App) {}
}
