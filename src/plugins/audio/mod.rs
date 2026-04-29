use bevy::prelude::*;

/// Audio plugin — music, SFX, ambient layers.
/// Empty for Feature #1; contents land in Feature #6 (audio system).
pub struct AudioPlugin;

impl Plugin for AudioPlugin {
    fn build(&self, _app: &mut App) {}
}
