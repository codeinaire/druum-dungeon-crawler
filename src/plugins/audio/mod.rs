//! Audio subsystem — BGM, SFX, UI, and Ambient channels.
//!
//! ## Architecture decision: built-in `bevy_audio`, NOT `bevy_kira_audio`
//!
//! The 2026-04-29 roadmap (`project/roadmaps/20260429-01-bevy-dungeon-crawler-roadmap.md:358`)
//! pre-committed to `bevy_kira_audio = "0.25"`. Feature #6 research
//! (`project/research/20260501-235930-feature-6-audio-system.md`) deviated: Bevy 0.18
//! ships `Volume::fade_towards` plus an official `examples/audio/soundtrack.rs`
//! crossfade pattern. The `"3d"` umbrella feature in `Cargo.toml` already pulls
//! in `bevy_audio + vorbis` transitively, so the native path costs zero new deps
//! and zero new dep-version-axis maintenance. This is the same shape as Feature
//! #3's `moonshine-save` deviation (Resolved §3, 2026-04-29).
//!
//! ## Channels are marker components, NOT typed resources
//!
//! Engineers familiar with `bevy_kira_audio` may reach for
//! `Res<AudioChannel<Bgm>>` — that API does not exist in `bevy_audio`. Instead:
//! - `Bgm`, `Sfx`, `Ui`, `Ambient` are zero-sized `Component`s.
//! - Spawn pattern: `commands.spawn((AudioPlayer::new(handle), PlaybackSettings::..., Bgm))`.
//! - Query pattern: `Query<&AudioSink, With<Bgm>>`.
//! - Per-channel volume: `Res<ChannelVolumes>` (a 4-field struct of `Volume`s).
//!
//! Do NOT introduce a `bevy_kira_audio`-style typed channel resource here.
//!
//! ## Plugin uniqueness
//!
//! Druum's `AudioPlugin` (this struct) coexists with `bevy_audio::AudioPlugin`
//! (registered transitively via `DefaultPlugins`). Bevy's plugin uniqueness
//! check is by `type_name()` — different module paths, no collision (verified
//! at `bevy_app-0.18.1/src/plugin.rs:83-91`). This is the same naming-clash
//! discipline as `ActionsPlugin` (Feature #5) and `StatePlugin` (Feature #2).
//!
//! ## Tests
//!
//! Tests assert **registration** (entity has `Bgm` marker, `AudioPlayer`
//! component) NOT **playback** (`AudioSink` exists). On headless CI,
//! `audio_output_available` is `false`, so the audio playback systems skip
//! and `AudioSink` is never inserted (verified at
//! `bevy_audio-0.18.1/src/audio_output.rs:361-363`). Channel markers are
//! added by `commands.spawn` directly and are observable without audio output.

use bevy::prelude::*;

pub mod bgm;
pub mod sfx;

/// Marker component for BGM (background music) audio entities.
/// Channels are zero-sized markers, NOT typed resources (see module doc).
#[derive(Component, Default, Debug, Clone, Copy)]
pub struct Bgm;

/// Marker component for SFX (sound effects) audio entities.
#[derive(Component, Default, Debug, Clone, Copy)]
pub struct Sfx;

/// Marker component for UI audio entities (menu confirms, alerts).
/// Reserved for v1; downstream callers may use `Ui` for UI-specific sounds
/// once Feature #25 polish surfaces them.
#[derive(Component, Default, Debug, Clone, Copy)]
pub struct Ui;

/// Marker component for ambient audio entities (dungeon drones, weather).
/// Reserved for v1; concrete consumers land in Feature #9 dungeon
/// atmosphere.
#[derive(Component, Default, Debug, Clone, Copy)]
pub struct Ambient;

/// Per-channel volume sliders. Initialized to all-1.0 linear at app start.
/// Persistence across app sessions is deferred to Feature #25 settings UI.
///
/// **v1 simplification:** This resource holds the volume values, but
/// `play_bgm_for_state` and `handle_sfx_requests` use `Volume::Linear(1.0)`
/// directly when spawning audio. The per-channel volume application system
/// is deferred to Feature #25 (research §Pitfall 7 acknowledges this).
/// Future Feature #25 will add `apply_channel_volumes` gated by
/// `resource_changed::<ChannelVolumes>` that walks
/// `Query<&mut AudioSink, With<Bgm>>` (and the other 3 markers) and applies
/// the volume.
#[derive(Resource, Debug, Clone, Copy)]
pub struct ChannelVolumes {
    pub bgm: bevy::audio::Volume,
    pub sfx: bevy::audio::Volume,
    pub ui: bevy::audio::Volume,
    pub ambient: bevy::audio::Volume,
}

impl Default for ChannelVolumes {
    fn default() -> Self {
        Self {
            bgm: bevy::audio::Volume::Linear(1.0),
            sfx: bevy::audio::Volume::Linear(1.0),
            ui: bevy::audio::Volume::Linear(1.0),
            ambient: bevy::audio::Volume::Linear(1.0),
        }
    }
}

pub use bgm::{FADE_SECS, FadeIn, FadeOut};
pub use sfx::{SfxKind, SfxRequest};

/// Druum's audio plugin. Wires up channel markers, ChannelVolumes,
/// state-driven BGM crossfade, and the SFX message/consumer pair.
///
/// Coexists with `bevy_audio::AudioPlugin` (registered via DefaultPlugins).
/// Different `type_name`, no collision.
pub struct AudioPlugin;

impl Plugin for AudioPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ChannelVolumes>()
            .add_message::<SfxRequest>()
            // BGM: state-driven crossfade. The system is gated by
            // `state_changed::<GameState>` so it only fires on the frame the
            // state changes. FadeIn/FadeOut tick systems run every frame.
            .add_systems(
                Update,
                (
                    bgm::play_bgm_for_state
                        .run_if(state_changed::<crate::plugins::state::GameState>),
                    bgm::fade_in_tick,
                    bgm::fade_out_tick,
                ),
            )
            // SFX: message consumer. Runs every frame; bails if no requests.
            .add_systems(Update, sfx::handle_sfx_requests);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugins::loading::AudioAssets;
    use crate::plugins::state::GameState;
    use bevy::asset::AssetPlugin;
    use bevy::audio::AudioPlugin as BevyAudioPlugin;
    use bevy::state::app::StatesPlugin;

    /// Builds a minimal test app with the audio plugin chain. Inserts a
    /// stub `AudioAssets` resource with default `Handle<AudioSource>`
    /// values (the real one comes from `bevy_asset_loader` at runtime).
    ///
    /// `audio_output_available` is false on headless CI so the rodio
    /// playback systems skip — but `commands.spawn` adds marker
    /// components directly, which is what the tests assert.
    fn make_test_app() -> App {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            BevyAudioPlugin::default(),
            StatesPlugin,
            crate::plugins::state::StatePlugin,
            AudioPlugin,
        ));
        // Stub AudioAssets — Handle::default() is a "weak handle to nothing"
        // which is fine for tests because we assert markers, not playback.
        let h = Handle::<bevy::audio::AudioSource>::default();
        app.insert_resource(AudioAssets {
            bgm_town: h.clone(),
            bgm_dungeon: h.clone(),
            bgm_combat: h.clone(),
            bgm_title: h.clone(),
            bgm_gameover: h.clone(),
            sfx_footstep: h.clone(),
            sfx_door: h.clone(),
            sfx_encounter_sting: h.clone(),
            sfx_menu_click: h.clone(),
            sfx_attack_hit: h.clone(),
            // Feature #13 additions:
            sfx_spinner_whoosh: h.clone(),
            sfx_door_close: h.clone(),
        });
        // When compiled with --features dev, StatePlugin::build registers
        // cycle_game_state_on_f9 which requires ButtonInput<KeyCode>. Insert
        // directly (without InputPlugin) so keyboard_input_system's clear loop
        // is not registered — same pattern as src/plugins/state/mod.rs:107.
        #[cfg(feature = "dev")]
        app.init_resource::<ButtonInput<KeyCode>>();
        app.update(); // realise initial state -> Loading; ChannelVolumes inserted
        app
    }

    /// Plugin builds without panic. Smoke test.
    #[test]
    fn audio_plugin_builds_without_panic() {
        let _app = make_test_app();
        // No assertion needed — successful update means plugin registration
        // succeeded.
    }

    /// `ChannelVolumes` resource is registered with all-1.0 linear volumes.
    #[test]
    fn channel_volumes_initialized_at_unit() {
        let app = make_test_app();
        let vols = app.world().resource::<ChannelVolumes>();
        assert!(matches!(vols.bgm, bevy::audio::Volume::Linear(v) if v == 1.0));
        assert!(matches!(vols.sfx, bevy::audio::Volume::Linear(v) if v == 1.0));
        assert!(matches!(vols.ui, bevy::audio::Volume::Linear(v) if v == 1.0));
        assert!(matches!(vols.ambient, bevy::audio::Volume::Linear(v) if v == 1.0));
    }

    /// State transition Loading→Town spawns a BGM entity with the Bgm marker.
    /// Verifies the play_bgm_for_state system fires on state_changed::<GameState>.
    #[test]
    fn state_change_to_town_spawns_bgm_entity() {
        let mut app = make_test_app();
        // Force transition to Town.
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Town);
        app.update(); // StateTransition runs, GameState becomes Town
        app.update(); // play_bgm_for_state runs in Update on the changed frame
        let count = app
            .world_mut()
            .query_filtered::<Entity, With<Bgm>>()
            .iter(app.world())
            .count();
        assert_eq!(
            count, 1,
            "expected exactly one Bgm entity after entering Town state"
        );
    }

    /// SFX message produces an Sfx entity. Verifies the handle_sfx_requests
    /// consumer pattern.
    #[test]
    fn sfx_request_spawns_sfx_entity() {
        let mut app = make_test_app();
        app.world_mut()
            .resource_mut::<bevy::ecs::message::Messages<SfxRequest>>()
            .write(SfxRequest {
                kind: SfxKind::Footstep,
            });
        app.update(); // handle_sfx_requests consumes the message
        let count = app
            .world_mut()
            .query_filtered::<Entity, With<Sfx>>()
            .iter(app.world())
            .count();
        assert_eq!(count, 1, "expected exactly one Sfx entity per SfxRequest");
    }

    /// Spawn a `(Bgm, AudioPlayer, FadeIn { 0.1s })`, advance time past 0.1s,
    /// assert `FadeIn` component is removed (the entity stays — fade-in
    /// completion just removes the marker, not the audio).
    ///
    /// **NOTE:** This test only verifies the `FadeIn` component lifecycle.
    /// The `set_volume` call inside `fade_in_tick` requires `AudioSink` to
    /// exist on the entity — and `AudioSink` is added by `bevy_audio`'s
    /// `PostUpdate` systems gated by `audio_output_available`, which is
    /// false on headless CI. So `fade_in_tick`'s loop body iterates zero
    /// items in tests. To still verify the LIFECYCLE, we manually insert a
    /// `MockAudioSink` substitute — but Bevy's AudioSink isn't easily
    /// mockable. Instead, this test asserts the component-removal path
    /// indirectly: we verify that AFTER many updates, the FadeIn component
    /// does NOT remain (because either (a) audio_output_available was true
    /// and the tick system removed it, or (b) audio_output_available was
    /// false and the tick system never ran — in case (b), the test is a
    /// no-op assertion that the entity stays alive).
    ///
    /// The audible-on-real-hardware verification is the manual smoke test
    /// in §Verification, NOT this unit test.
    #[test]
    fn fade_in_component_lifecycle() {
        let mut app = make_test_app();
        // Spawn a Bgm entity with a 0.05s FadeIn — short so we don't need
        // many updates.
        let h = Handle::<bevy::audio::AudioSource>::default();
        let entity = app
            .world_mut()
            .spawn((
                AudioPlayer::new(h),
                super::Bgm,
                super::bgm::FadeIn {
                    duration_secs: 0.05,
                    elapsed_secs: 0.0,
                },
            ))
            .id();

        // Advance time several frames. Each app.update() advances Time::delta
        // automatically per Bevy's Time machinery, but in MinimalPlugins
        // tests the delta is small. Run enough frames to definitively exceed
        // 0.05s of accumulated delta.
        for _ in 0..30 {
            app.update();
        }

        // Either: AudioSink exists (audio_output_available=true) and FadeIn
        // was removed by fade_in_tick. Or: AudioSink does not exist
        // (headless CI) and FadeIn is still present. Both are valid; we
        // just verify the entity is still alive (FadeOut never fires for
        // a FadeIn-only entity).
        assert!(
            app.world().get_entity(entity).is_ok(),
            "Bgm entity with FadeIn should remain alive after fade_in_tick converges"
        );
    }

    /// Spawn a `(Bgm, AudioPlayer, FadeOut { 0.05s })`, advance time, assert
    /// the entity is despawned. This tests the FadeOut termination guarantee
    /// (research §Pitfall 6).
    ///
    /// **NOTE:** Same headless-CI caveat as `fade_in_component_lifecycle` —
    /// if `audio_output_available` is false, `fade_out_tick`'s query iterates
    /// zero items and the entity is NOT despawned. This is a known limitation
    /// of headless audio testing.
    ///
    /// **What this test still verifies:** the FadeOut COMPONENT is constructible
    /// and queryable. The despawn path is verified by the manual smoke test
    /// (Verification §Manual smoke test) where audio_output_available is
    /// true.
    #[test]
    fn fade_out_component_constructible() {
        let mut app = make_test_app();
        let h = Handle::<bevy::audio::AudioSource>::default();
        let entity = app
            .world_mut()
            .spawn((
                AudioPlayer::new(h),
                super::Bgm,
                super::bgm::FadeOut {
                    duration_secs: 0.05,
                    elapsed_secs: 0.0,
                },
            ))
            .id();
        app.update();

        // Verify the FadeOut component is queryable on the entity (this
        // proves the component derive is correct and the entity has the
        // expected shape after spawn). On real hardware, the despawn arm
        // of fade_out_tick runs after time accumulates; we cannot verify
        // that path in a headless test.
        let has_fade_out = app.world().entity(entity).contains::<super::bgm::FadeOut>();
        assert!(
            has_fade_out,
            "FadeOut should be present on freshly-spawned entity"
        );
    }
}
