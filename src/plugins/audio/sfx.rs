//! SFX (sound effects) — `Message<SfxRequest>` consumer.
//!
//! ## Public API
//!
//! Downstream features write `SfxRequest` messages via:
//!
//! ```ignore
//! use crate::plugins::audio::{SfxKind, SfxRequest};
//! use bevy::prelude::*;
//!
//! fn handle_movement(mut sfx: MessageWriter<SfxRequest>) {
//!     sfx.write(SfxRequest { kind: SfxKind::Footstep });
//! }
//! ```
//!
//! No helper function — direct `MessageWriter` is one line and clear
//! (research §RQ8 resolved). The `SfxRequest`/`SfxKind` types are the
//! whole API.
//!
//! ## Consumer
//!
//! `handle_sfx_requests` runs every frame in `Update`. It reads pending
//! `SfxRequest` messages, maps each `SfxKind` to a `Handle<AudioSource>`
//! from `AudioAssets`, and spawns `(AudioPlayer, Sfx, PlaybackSettings::DESPAWN)`
//! per request. `PlaybackSettings::DESPAWN` is the canonical
//! "play-once-then-despawn" idiom (verified at
//! `bevy_audio-0.18.1/src/audio.rs:105-109`).
//!
//! ## Bevy 0.18 Event/Message rename
//!
//! `SfxRequest` derives `Message`, NOT `Event`. Read with `MessageReader`,
//! NOT `EventReader`. Same family-rename trap as `StateTransitionEvent`
//! (Feature #2), `AssetEvent` (Feature #3), and `KeyboardInput` (Feature #5).

use bevy::prelude::*;

use crate::plugins::loading::AudioAssets;

/// Buffered request to play a one-shot SFX. Routed through
/// `Messages<SfxRequest>` for centralized, swappable audio backend
/// (research §RQ8). Downstream features emit; the audio plugin consumes.
#[derive(Message, Clone, Copy, Debug)]
pub struct SfxRequest {
    pub kind: SfxKind,
}

/// Closed enum of SFX kinds the audio module knows how to play. Adding
/// a new variant requires updating both the enum and the `match` in
/// `handle_sfx_requests` — the compiler enforces the latter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SfxKind {
    Footstep,
    Door,
    EncounterSting,
    MenuClick,
    AttackHit,
}

/// Consumes pending `SfxRequest` messages and spawns one `(AudioPlayer,
/// Sfx, PlaybackSettings::DESPAWN)` entity per request.
///
/// Tolerant of missing `AudioAssets` — bails silently if the resource
/// isn't present (e.g., very early frames). Pending messages are still
/// drained by `MessageReader::read`, so they don't accumulate forever.
pub fn handle_sfx_requests(
    mut commands: Commands,
    mut reader: MessageReader<SfxRequest>,
    audio_assets: Option<Res<AudioAssets>>,
) {
    let Some(audio_assets) = audio_assets else {
        // AudioAssets not yet populated — drain messages without spawning.
        // (`reader.read()` advances the cursor; messages won't be re-read.)
        for _ in reader.read() {}
        return;
    };

    for req in reader.read() {
        let handle = match req.kind {
            SfxKind::Footstep => audio_assets.sfx_footstep.clone(),
            SfxKind::Door => audio_assets.sfx_door.clone(),
            SfxKind::EncounterSting => audio_assets.sfx_encounter_sting.clone(),
            SfxKind::MenuClick => audio_assets.sfx_menu_click.clone(),
            SfxKind::AttackHit => audio_assets.sfx_attack_hit.clone(),
        };
        commands.spawn((AudioPlayer::new(handle), PlaybackSettings::DESPAWN, super::Sfx));
    }
}
