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
