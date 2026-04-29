use bevy::prelude::*;
use druum::plugins::{
    audio::AudioPlugin,
    combat::CombatPlugin,
    dungeon::DungeonPlugin,
    party::PartyPlugin,
    save::SavePlugin,
    town::TownPlugin,
    ui::UiPlugin,
};

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins,
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
