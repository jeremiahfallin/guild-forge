//! Persistent user settings: volumes, ember visuals, window mode/resolution.
//! Stored machine-level at `<data_dir>/guild-forge/settings.ron` — separate
//! from `save.ron` so display/audio config survives save deletion.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// Windowed-mode resolution presets, cycled by the settings UI.
pub const RESOLUTION_PRESETS: &[(u32, u32)] =
    &[(1280, 720), (1600, 900), (1920, 1080), (2560, 1440)];

#[derive(Resource, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct GameSettings {
    pub master_volume: f32,
    pub music_volume: f32,
    pub sfx_volume: f32,
    pub ember_density: f32,
    pub ember_warmth: f32,
    pub fullscreen: bool,
    pub windowed_resolution: (u32, u32),
}

impl Default for GameSettings {
    fn default() -> Self {
        Self {
            master_volume: 1.0,
            music_volume: 1.0,
            sfx_volume: 1.0,
            ember_density: 1.0,
            ember_warmth: 1.0,
            fullscreen: false,
            windowed_resolution: (1920, 1080),
        }
    }
}

/// Parse settings from ron, falling back to defaults on any error. Missing
/// fields fill from `Default` (`#[serde(default)]`), so old files keep working
/// as fields are added.
pub fn parse_settings(ron_str: &str) -> GameSettings {
    ron::from_str(ron_str).unwrap_or_default()
}

/// Step to the next/previous preset, wrapping. An unknown current resolution
/// (e.g. hand-edited file) lands on the first preset.
pub fn cycle_resolution(current: (u32, u32), forward: bool) -> (u32, u32) {
    let n = RESOLUTION_PRESETS.len();
    let idx = RESOLUTION_PRESETS.iter().position(|&r| r == current);
    let next = match (idx, forward) {
        (Some(i), true) => (i + 1) % n,
        (Some(i), false) => (i + n - 1) % n,
        (None, _) => 0,
    };
    RESOLUTION_PRESETS[next]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_round_trip() {
        let mut s = GameSettings::default();
        s.music_volume = 0.4;
        s.fullscreen = true;
        s.windowed_resolution = (1280, 720);
        let ron_str = ron::to_string(&s).unwrap();
        assert_eq!(parse_settings(&ron_str), s);
    }

    #[test]
    fn partial_and_corrupt_input_fall_back_to_defaults() {
        // Missing fields fill from Default
        let s = parse_settings("(sfx_volume: 0.25)");
        assert_eq!(s.sfx_volume, 0.25);
        assert_eq!(s.master_volume, 1.0);
        assert!(!s.fullscreen);
        // Garbage falls back wholesale
        assert_eq!(parse_settings("not ron at all"), GameSettings::default());
        // Empty struct body is valid and yields defaults
        assert_eq!(parse_settings("()"), GameSettings::default());
    }

    #[test]
    fn resolution_cycling_wraps_and_recovers() {
        assert_eq!(cycle_resolution((1920, 1080), true), (2560, 1440));
        assert_eq!(cycle_resolution((2560, 1440), true), (1280, 720));
        assert_eq!(cycle_resolution((1280, 720), false), (2560, 1440));
        // Unknown resolution recovers to first preset
        assert_eq!(cycle_resolution((123, 456), true), (1280, 720));
    }
}
