//! Music states (UX-4): guild/exploration/combat/boss layers with crossfade,
//! plus the mission-event SFX bridge. Placeholder tracks; real music is
//! human-led — swap paths in `MUSIC_TRACKS`.

use bevy::prelude::*;

/// Which music layer should be audible.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MusicState {
    #[default]
    Guild,
    Exploration,
    Combat,
    Boss,
}

impl MusicState {
    pub const ALL: &[MusicState] = &[
        MusicState::Guild,
        MusicState::Exploration,
        MusicState::Combat,
        MusicState::Boss,
    ];
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

    #[test]
    fn every_state_has_a_track() {
        for state in MusicState::ALL {
            let entry = MUSIC_TRACKS.iter().find(|(s, _, _)| s == state);
            let (_, path, speed) = entry.expect("state missing from MUSIC_TRACKS");
            assert!(path.ends_with(".ogg"));
            assert!(*speed > 0.0);
        }
    }
}
