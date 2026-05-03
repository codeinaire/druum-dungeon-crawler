//! BGM (background music) — state-driven crossfade.
//!
//! On every `state_changed::<GameState>` frame, `play_bgm_for_state`:
//! 1. Adds `FadeOut` to all entities currently tagged `(Bgm, AudioSink)`,
//!    starting their volume rolloff toward `Volume::SILENT`.
//! 2. Spawns a new `(AudioPlayer, Bgm, FadeIn, PlaybackSettings::Loop)` entity
//!    with the new state's BGM track at silent volume, ramping up.
//!
//! `fade_in_tick` and `fade_out_tick` run every frame, advance per-entity
//! elapsed time, and apply `Volume::fade_towards` linearly. When elapsed
//! reaches the duration, `FadeIn` is removed (entity stays alive) and
//! `FadeOut` despawns the entity (the canonical termination guarantee for
//! looping playback that would otherwise run forever at silent volume —
//! research §Pitfall 6).
//!
//! Crossfade duration is 1 second (research §Open Question Q5 resolved).
//! Both fade-in and fade-out use the same constant.
//!
//! Mirrors `bevy-0.18.1/examples/audio/soundtrack.rs:63-132` verbatim with
//! adjustments for Druum's GameState enum.

use bevy::audio::{AudioSink, PlaybackMode, Volume};
use bevy::prelude::*;

use crate::plugins::loading::AudioAssets;
use crate::plugins::state::GameState;

/// Crossfade duration in seconds. Applies symmetrically to fade-in and
/// fade-out. Tunable in Feature #25 polish if 1 second feels off.
pub const FADE_SECS: f32 = 1.0;

/// Component on a freshly-spawned BGM entity. Volume ramps from
/// `SILENT` to `Linear(1.0)` over `duration_secs`. When `elapsed_secs`
/// reaches `duration_secs`, the component is removed (entity stays).
#[derive(Component, Debug)]
pub struct FadeIn {
    pub duration_secs: f32,
    pub elapsed_secs: f32,
}

impl Default for FadeIn {
    fn default() -> Self {
        Self {
            duration_secs: FADE_SECS,
            elapsed_secs: 0.0,
        }
    }
}

/// Component on a soon-to-end BGM entity. Volume ramps from
/// `Linear(1.0)` to `SILENT` over `duration_secs`. When `elapsed_secs`
/// reaches `duration_secs`, the entity is despawned (terminates looping
/// playback that volume=0 alone would not stop).
#[derive(Component, Debug)]
pub struct FadeOut {
    pub duration_secs: f32,
    pub elapsed_secs: f32,
}

impl Default for FadeOut {
    fn default() -> Self {
        Self {
            duration_secs: FADE_SECS,
            elapsed_secs: 0.0,
        }
    }
}

/// On every `state_changed::<GameState>` frame: fade out current BGM, spawn
/// new BGM for the new state.
///
/// Returns early on `GameState::Loading` — no music while assets resolve
/// (research §Open Question Q4 resolved). The fade-out arm still runs
/// before the early return: any BGM currently playing fades out, then
/// the new state has no replacement track.
///
/// Tolerant of missing `AudioAssets` (e.g., during the `Loading -> TitleScreen`
/// transition — `AudioAssets` is inserted by `bevy_asset_loader` on the
/// SAME frame the state transitions to TitleScreen, but resource ordering
/// in that frame is not guaranteed). If `AudioAssets` is absent, the system
/// silently skips spawning the new BGM (the fade-out arm has already run).
pub fn play_bgm_for_state(
    mut commands: Commands,
    bgm_query: Query<Entity, With<super::Bgm>>,
    audio_assets: Option<Res<AudioAssets>>,
    state: Res<State<GameState>>,
) {
    // (1) Fade out everything currently on the BGM channel. We tag with
    //     FadeOut whether or not AudioSink exists yet — the tick system
    //     handles both cases tolerantly via Query<&mut AudioSink, With<FadeOut>>.
    for e in &bgm_query {
        commands.entity(e).insert(FadeOut::default());
    }

    // (2) Pick the new state's BGM track. Hardcoded match (research §RQ7
    //     resolved — defer RON-driven map to Feature #25 polish). The match
    //     is exhaustive over GameState; adding a new variant is a compile
    //     error here, surfacing the choice "what BGM plays in this state?".
    let Some(audio_assets) = audio_assets else {
        // AudioAssets not yet populated — fade out completed but no new
        // track spawn. Acceptable in early frames or in tests with stubbed
        // resources missing.
        return;
    };

    let track = match state.get() {
        GameState::Loading => return, // No music while loading.
        GameState::Town => audio_assets.bgm_town.clone(),
        GameState::Dungeon => audio_assets.bgm_dungeon.clone(),
        GameState::Combat => audio_assets.bgm_combat.clone(),
        GameState::TitleScreen => audio_assets.bgm_title.clone(),
        GameState::GameOver => audio_assets.bgm_gameover.clone(),
    };

    commands.spawn((
        AudioPlayer::new(track),
        PlaybackSettings {
            mode: PlaybackMode::Loop,
            volume: Volume::SILENT,
            ..default()
        },
        super::Bgm,
        FadeIn::default(),
    ));
}

/// Tick `FadeIn` components every frame. Advance elapsed time, apply
/// linear interpolation via `Volume::fade_towards`. When `factor >= 1.0`,
/// remove the component (entity stays — it's now playing at full volume).
///
/// Iterates over `Query<(Entity, &mut AudioSink, &mut FadeIn)>`. Entities
/// with `FadeIn` but without `AudioSink` are skipped — the sink hasn't yet
/// been added by `bevy_audio`'s output systems (research §Pitfall 2).
/// They'll be picked up on a subsequent frame once the audio source loads.
pub fn fade_in_tick(
    mut commands: Commands,
    mut q: Query<(Entity, &mut AudioSink, &mut FadeIn)>,
    time: Res<Time>,
) {
    for (e, mut sink, mut fade) in &mut q {
        fade.elapsed_secs += time.delta_secs();
        let factor = (fade.elapsed_secs / fade.duration_secs).clamp(0.0, 1.0);
        sink.set_volume(Volume::SILENT.fade_towards(Volume::Linear(1.0), factor));
        if factor >= 1.0 {
            sink.set_volume(Volume::Linear(1.0));
            commands.entity(e).remove::<FadeIn>();
        }
    }
}

/// Tick `FadeOut` components every frame. Advance elapsed time, apply
/// linear interpolation toward silence. When `factor >= 1.0`, despawn
/// the entity. The despawn is the canonical termination guarantee for
/// looping playback (research §Pitfall 6 — without it, a fading loop
/// runs forever at silent volume).
///
/// Same `AudioSink`-may-not-exist tolerance as `fade_in_tick`.
pub fn fade_out_tick(
    mut commands: Commands,
    mut q: Query<(Entity, &mut AudioSink, &mut FadeOut)>,
    time: Res<Time>,
) {
    for (e, mut sink, mut fade) in &mut q {
        fade.elapsed_secs += time.delta_secs();
        let factor = (fade.elapsed_secs / fade.duration_secs).clamp(0.0, 1.0);
        sink.set_volume(Volume::Linear(1.0).fade_towards(Volume::SILENT, factor));
        if factor >= 1.0 {
            commands.entity(e).despawn();
        }
    }
}
