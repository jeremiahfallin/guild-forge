//! Persistent user settings: volumes, ember visuals, window mode/resolution.
//! Stored machine-level at `<data_dir>/guild-forge/settings.ron` — separate
//! from `save.ron` so display/audio config survives save deletion.

use bevy::prelude::*;
use bevy::window::{MonitorSelection, PrimaryWindow, WindowMode};
use serde::{Deserialize, Serialize};

use crate::audio::{MusicVolume, SfxVolume};
use crate::screens::EmberSettings;

/// Seconds after the last change before settings hit the disk.
const SAVE_DEBOUNCE_SECS: f32 = 1.0;

pub(super) fn plugin(app: &mut App) {
    app.insert_resource(load_settings());
    app.add_systems(Startup, apply_settings_on_startup);
    app.add_systems(
        Update,
        (capture_changed_resources, save_settings_when_dirty).chain(),
    );
}

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

/// `<data_dir>/guild-forge/settings.ron` (same directory as `save.ron`).
pub fn settings_path() -> Option<std::path::PathBuf> {
    dirs::data_dir().map(|d| d.join("guild-forge").join("settings.ron"))
}

pub fn write_settings_file(path: &std::path::Path, settings: &GameSettings) {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    match ron::ser::to_string_pretty(settings, Default::default()) {
        Ok(s) => {
            if let Err(e) = std::fs::write(path, s) {
                warn!("Failed to write settings: {e}");
            }
        }
        Err(e) => warn!("Failed to serialize settings: {e}"),
    }
}

fn load_settings() -> GameSettings {
    settings_path()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .map(|s| parse_settings(&s))
        .unwrap_or_default()
}

/// Push loaded settings into the live resources and the window.
fn apply_settings_on_startup(
    settings: Res<GameSettings>,
    mut global_volume: ResMut<GlobalVolume>,
    mut music: ResMut<MusicVolume>,
    mut sfx: ResMut<SfxVolume>,
    mut embers: ResMut<EmberSettings>,
    mut window_q: Query<&mut Window, With<PrimaryWindow>>,
) {
    global_volume.volume = bevy::audio::Volume::Linear(settings.master_volume);
    music.0 = settings.music_volume;
    sfx.0 = settings.sfx_volume;
    embers.density = settings.ember_density;
    embers.warmth = settings.ember_warmth;
    if let Ok(mut window) = window_q.single_mut() {
        apply_window_settings(&settings, &mut window);
    }
}

/// Shared by startup and the settings-menu handlers.
pub fn apply_window_settings(settings: &GameSettings, window: &mut Window) {
    if settings.fullscreen {
        window.mode = WindowMode::BorderlessFullscreen(MonitorSelection::Current);
    } else {
        window.mode = WindowMode::Windowed;
        let (w, h) = settings.windowed_resolution;
        window.resolution.set(w as f32, h as f32);
    }
}

/// Mirror changes in the live resources back into `GameSettings` so the
/// debounced saver sees them. Window fields are written directly by the
/// settings-menu handlers.
fn capture_changed_resources(
    mut settings: ResMut<GameSettings>,
    global_volume: Res<GlobalVolume>,
    music: Res<MusicVolume>,
    sfx: Res<SfxVolume>,
    embers: Res<EmberSettings>,
) {
    if !(global_volume.is_changed()
        || music.is_changed()
        || sfx.is_changed()
        || embers.is_changed())
    {
        return;
    }
    let captured = GameSettings {
        master_volume: global_volume.volume.to_linear(),
        music_volume: music.0,
        sfx_volume: sfx.0,
        ember_density: embers.density,
        ember_warmth: embers.warmth,
        ..settings.clone()
    };
    // Avoid dirtying GameSettings every frame: bypass change detection unless
    // something actually differs.
    if *settings != captured {
        *settings = captured;
    }
}

fn save_settings_when_dirty(
    settings: Res<GameSettings>,
    time: Res<Time>,
    mut countdown: Local<Option<f32>>,
) {
    if settings.is_changed() {
        *countdown = Some(SAVE_DEBOUNCE_SECS);
        return;
    }
    if let Some(remaining) = countdown.as_mut() {
        *remaining -= time.delta_secs();
        if *remaining <= 0.0 {
            *countdown = None;
            if let Some(path) = settings_path() {
                write_settings_file(&path, &settings);
                info!("Settings saved");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_round_trip() {
        let s = GameSettings {
            music_volume: 0.4,
            fullscreen: true,
            windowed_resolution: (1280, 720),
            ..Default::default()
        };
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
    fn save_and_load_file_round_trip() {
        let dir = std::env::temp_dir().join("guild-forge-settings-test");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("settings.ron");
        let s = GameSettings {
            master_volume: 0.7,
            fullscreen: true,
            ..Default::default()
        };
        write_settings_file(&path, &s);
        let loaded = parse_settings(&std::fs::read_to_string(&path).unwrap());
        assert_eq!(loaded, s);
        let _ = std::fs::remove_file(&path);
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
