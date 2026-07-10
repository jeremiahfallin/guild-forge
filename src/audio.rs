use bevy::audio::Volume;
use bevy::prelude::*;

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<MusicVolume>();
    app.init_resource::<SfxVolume>();
    app.add_systems(
        Update,
        (
            apply_volumes.run_if(
                resource_changed::<GlobalVolume>
                    .or(resource_changed::<MusicVolume>)
                    .or(resource_changed::<SfxVolume>),
            ),
            apply_volume_to_new_sinks,
        ),
    );
}

/// An organizational marker component that should be added to a spawned [`AudioPlayer`] if it's in the
/// general "music" category (e.g. global background music, soundtrack).
///
/// This can then be used to query for and operate on sounds in that category.
#[derive(Component, Reflect, Default)]
#[reflect(Component)]
pub struct Music;

/// A music audio instance.
pub fn music(handle: Handle<AudioSource>) -> impl Bundle {
    (AudioPlayer(handle), PlaybackSettings::LOOP, Music)
}

/// An organizational marker component that should be added to a spawned [`AudioPlayer`] if it's in the
/// general "sound effect" category (e.g. footsteps, the sound of a magic spell, a door opening).
///
/// This can then be used to query for and operate on sounds in that category.
#[derive(Component, Reflect, Default)]
#[reflect(Component)]
pub struct SoundEffect;

/// A sound effect audio instance.
pub fn sound_effect(handle: Handle<AudioSource>) -> impl Bundle {
    (AudioPlayer(handle), PlaybackSettings::DESPAWN, SoundEffect)
}

/// Music bus volume (linear, 0–2). Multiplied with [`GlobalVolume`].
#[derive(Resource, Debug, Clone, PartialEq)]
pub struct MusicVolume(pub f32);

impl Default for MusicVolume {
    fn default() -> Self {
        Self(1.0)
    }
}

/// Sound-effect bus volume (linear, 0–2). Multiplied with [`GlobalVolume`].
#[derive(Resource, Debug, Clone, PartialEq)]
pub struct SfxVolume(pub f32);

impl Default for SfxVolume {
    fn default() -> Self {
        Self(1.0)
    }
}

/// Final sink volume: master × bus × the sink's own [`PlaybackSettings`] volume.
pub fn effective_volume(master: f32, bus: f32, playback: Volume) -> Volume {
    Volume::Linear(master * bus * playback.to_linear())
}

fn bus_for(is_music: bool, music: &MusicVolume, sfx: &SfxVolume) -> f32 {
    if is_music { music.0 } else { sfx.0 }
}

/// Re-apply volumes to running sinks when master or a bus changes.
/// ([`GlobalVolume`] doesn't apply to already-running audio entities.)
fn apply_volumes(
    global_volume: Res<GlobalVolume>,
    music: Res<MusicVolume>,
    sfx: Res<SfxVolume>,
    mut audio_query: Query<(&PlaybackSettings, &mut AudioSink, Has<Music>)>,
) {
    for (playback, mut sink, is_music) in &mut audio_query {
        let bus = bus_for(is_music, &music, &sfx);
        sink.set_volume(effective_volume(
            global_volume.volume.to_linear(),
            bus,
            playback.volume,
        ));
    }
}

/// Bevy initializes new sinks with master × playback only — fold the bus in
/// on the sink's first frame.
fn apply_volume_to_new_sinks(
    global_volume: Res<GlobalVolume>,
    music: Res<MusicVolume>,
    sfx: Res<SfxVolume>,
    mut new_sinks: Query<(&PlaybackSettings, &mut AudioSink, Has<Music>), Added<AudioSink>>,
) {
    for (playback, mut sink, is_music) in &mut new_sinks {
        let bus = bus_for(is_music, &music, &sfx);
        sink.set_volume(effective_volume(
            global_volume.volume.to_linear(),
            bus,
            playback.volume,
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn effective_volume_multiplies_master_bus_playback() {
        let v = effective_volume(0.5, 0.5, Volume::Linear(2.0));
        assert!((v.to_linear() - 0.5).abs() < 1e-6);
        let muted = effective_volume(0.0, 1.0, Volume::Linear(1.0));
        assert_eq!(muted.to_linear(), 0.0);
    }
}
