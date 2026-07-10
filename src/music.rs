//! Music states (UX-4): guild/exploration/combat/boss layers with crossfade,
//! plus the mission-event SFX bridge. Placeholder tracks; real music is
//! human-led — swap paths in `MUSIC_TRACKS`.

use bevy::audio::Volume;
use bevy::prelude::*;

use crate::audio::{Music, MusicVolume, effective_volume};
use crate::menus::Menu;
use crate::mission::entities::{EnemyToken, GridPosition, HeroToken};
use crate::mission::{Mission, ViewedMission, combat_overlap, hero_action_range};
use crate::screens::{GameTab, Screen};

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<CurrentMusicState>();
    app.add_systems(Startup, spawn_music_layers);
    app.add_systems(Update, (derive_music_state, crossfade_music).chain());
}

/// The state the crossfade is currently steering toward.
#[derive(Resource, Debug, Default, PartialEq)]
pub struct CurrentMusicState(pub MusicState);

/// Tags one of the four persistent looping layer entities.
#[derive(Component, Debug)]
pub struct MusicLayer(pub MusicState);

/// Which music layer should be audible.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MusicState {
    #[default]
    Guild,
    Exploration,
    Combat,
    Boss,
}

/// (state, asset path, playback speed). Boss reuses the combat track sped up
/// until a real track lands.
pub const MUSIC_TRACKS: &[(MusicState, &str, f32)] = &[
    (MusicState::Guild, "audio/music/Fluffing A Duck.ogg", 1.0),
    (MusicState::Exploration, "audio/music/Fluffing A Duck.ogg", 1.0),
    (
        MusicState::Combat,
        "audio/music/Monkeys Spinning Monkeys.ogg",
        1.0,
    ),
    (
        MusicState::Boss,
        "audio/music/Monkeys Spinning Monkeys.ogg",
        1.25,
    ),
];

/// Full crossfade duration in seconds.
pub const CROSSFADE_SECS: f32 = 0.8;

/// Decide the music state. Boss wins over combat; mission states only apply
/// while actually watching a mission.
pub fn target_state(viewing_mission: bool, in_combat: bool, boss_in_range: bool) -> MusicState {
    if !viewing_mission {
        MusicState::Guild
    } else if boss_in_range {
        MusicState::Boss
    } else if in_combat {
        MusicState::Combat
    } else {
        MusicState::Exploration
    }
}

/// Move `current` toward `target` by at most `max_step`, without overshoot.
pub fn approach(current: f32, target: f32, max_step: f32) -> f32 {
    let delta = target - current;
    current + delta.clamp(-max_step.abs(), max_step.abs())
}

fn spawn_music_layers(mut commands: Commands, asset_server: Res<AssetServer>) {
    for &(state, path, speed) in MUSIC_TRACKS {
        commands.spawn((
            Name::new(format!("Music Layer {state:?}")),
            AudioPlayer::<AudioSource>(asset_server.load(path)),
            PlaybackSettings {
                volume: Volume::Linear(0.0),
                speed,
                ..PlaybackSettings::LOOP
            },
            Music,
            MusicLayer(state),
        ));
    }
}

/// Compute the target state from where the player is and what the viewed
/// mission's tokens are doing. Mirrors the banner/tempo queries.
fn derive_music_state(
    screen: Res<State<Screen>>,
    tab: Option<Res<State<GameTab>>>,
    viewed: Option<Res<ViewedMission>>,
    missions: Query<&Children, With<Mission>>,
    hero_tokens: Query<(&GridPosition, &HeroToken), Without<EnemyToken>>,
    enemy_tokens: Query<(&GridPosition, &EnemyToken), Without<HeroToken>>,
    hero_data: Query<&crate::hero::HeroInfo, With<crate::hero::Hero>>,
    mut current: ResMut<CurrentMusicState>,
) {
    let viewing = screen.get() == &Screen::Gameplay
        && tab.as_ref().is_some_and(|t| t.get() == &GameTab::MissionView)
        && viewed.is_some();

    let (mut in_combat, mut boss_in_range) = (false, false);
    if viewing
        && let Some(viewed) = viewed
        && let Ok(children) = missions.get(viewed.0)
    {
        let mut heroes = Vec::new();
        let mut enemies = Vec::new();
        let mut bosses = Vec::new();
        for &child in children {
            if let Ok((gp, token)) = hero_tokens.get(child) {
                if let Ok(info) = hero_data.get(token.0) {
                    heroes.push((*gp, hero_action_range(&info.class)));
                }
            } else if let Ok((gp, token)) = enemy_tokens.get(child) {
                enemies.push(*gp);
                if token.enemy_type.is_boss() {
                    bosses.push(*gp);
                }
            }
        }
        in_combat = combat_overlap(&heroes, &enemies);
        boss_in_range = combat_overlap(&heroes, &bosses);
    }

    let target = target_state(viewing, in_combat, boss_in_range);
    if current.0 != target {
        info!("Music state -> {target:?}");
        current.0 = target;
    }
}

/// Ramp each layer toward audible/silent and apply the bus-composed volume.
fn crossfade_music(
    current: Res<CurrentMusicState>,
    menu: Res<State<Menu>>,
    global_volume: Res<GlobalVolume>,
    music_volume: Res<MusicVolume>,
    time: Res<Time>,
    mut layers: Query<(&MusicLayer, &mut PlaybackSettings, Option<&mut AudioSink>)>,
) {
    let step = time.delta_secs() / CROSSFADE_SECS;
    let credits_open = menu.get() == &Menu::Credits;
    for (layer, mut playback, sink) in &mut layers {
        let target = if !credits_open && layer.0 == current.0 {
            1.0
        } else {
            0.0
        };
        let now = approach(playback.volume.to_linear(), target, step);
        playback.volume = Volume::Linear(now);
        if let Some(mut sink) = sink {
            sink.set_volume(effective_volume(
                global_volume.volume.to_linear(),
                music_volume.0,
                playback.volume,
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn target_state_decision_table() {
        assert_eq!(target_state(false, true, true), MusicState::Guild);
        assert_eq!(target_state(true, false, false), MusicState::Exploration);
        assert_eq!(target_state(true, true, false), MusicState::Combat);
        assert_eq!(target_state(true, true, true), MusicState::Boss);
        // Boss presence implies combat in practice, but boss must win regardless
        assert_eq!(target_state(true, false, true), MusicState::Boss);
    }

    #[test]
    fn approach_converges_without_overshoot() {
        assert_eq!(approach(0.0, 1.0, 0.25), 0.25);
        assert_eq!(approach(0.9, 1.0, 0.25), 1.0); // clamps at target
        assert_eq!(approach(1.0, 0.0, 0.25), 0.75); // fades down too
        assert_eq!(approach(0.5, 0.5, 0.25), 0.5); // stable at target
    }

    const ALL_STATES: &[MusicState] = &[
        MusicState::Guild,
        MusicState::Exploration,
        MusicState::Combat,
        MusicState::Boss,
    ];

    #[test]
    fn every_state_has_a_track() {
        for state in ALL_STATES {
            let entry = MUSIC_TRACKS.iter().find(|(s, _, _)| s == state);
            let (_, path, speed) = entry.expect("state missing from MUSIC_TRACKS");
            assert!(path.ends_with(".ogg"));
            assert!(*speed > 0.0);
        }
    }
}
