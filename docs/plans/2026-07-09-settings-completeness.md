# TI-4 Settings Completeness Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Persistent settings (`settings.ron`) with separate music/SFX volume buses and windowed/fullscreen + resolution controls.

**Architecture:** New `src/settings.rs` owns a serde-round-trippable `GameSettings` resource loaded at startup, applied to the live resources (`GlobalVolume`, `MusicVolume`, `SfxVolume`, `EmberSettings`, primary `Window`), captured back on change, and written to disk after a 1s debounce. `src/audio.rs` gains the two bus resources and a generalized volume-apply system (master × bus × playback) keyed off the existing `Music`/`SoundEffect` markers. `src/menus/settings.rs` grows to seven rows reusing the existing widget style.

**Tech Stack:** Bevy 0.18 (binary crate — `cargo test settings::`, not `--lib`), `ron` 0.9, `serde`, `dirs` 6 (already dependencies). Design doc: `docs/plans/2026-07-09-settings-completeness-design.md`.

**Test style (house rules):** `#[cfg(test)] mod tests` at the bottom of the source file; pure functions preferred; temp-dir file tests mirror `save.rs` style.

---

### Task 1: `GameSettings` model, parsing, resolution presets

**Files:**
- Create: `src/settings.rs`
- Modify: `src/main.rs:6-25` (module list), `src/main.rs:78-87` (plugin list)

**Step 1: Create `src/settings.rs` with failing tests**

```rust
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
```

**Step 2: Wire the module and run tests (red → green)**

In `src/main.rs` add `mod settings;` after `mod screens;` (line ~21). No plugin yet.
Run: `cargo test settings::`
Expected: PASS (3 tests). If compile fails on unused warnings only, proceed — the plugin lands in Task 3.

**Step 3: Commit**

```bash
git add src/settings.rs src/main.rs
git commit -m "feat(ti4): GameSettings model with ron parsing and resolution presets"
```

---

### Task 2: Music/SFX volume buses in `src/audio.rs`

**Files:**
- Modify: `src/audio.rs`

**Step 1: Write the failing test** (bottom of `src/audio.rs`)

```rust
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
```

**Step 2: Run to verify it fails**

Run: `cargo test audio::`
Expected: compile FAIL — `effective_volume` not defined (and `Volume` unimported in scope: the impl adds `use bevy::audio::Volume;`).

**Step 3: Implement**

Replace the plugin fn and `apply_global_volume` in `src/audio.rs`:

```rust
use bevy::audio::Volume;

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

/// Music bus volume (linear, 0–2). Multiplied with `GlobalVolume`.
#[derive(Resource, Debug, Clone, PartialEq)]
pub struct MusicVolume(pub f32);

impl Default for MusicVolume {
    fn default() -> Self {
        Self(1.0)
    }
}

/// Sound-effect bus volume (linear, 0–2). Multiplied with `GlobalVolume`.
#[derive(Resource, Debug, Clone, PartialEq)]
pub struct SfxVolume(pub f32);

impl Default for SfxVolume {
    fn default() -> Self {
        Self(1.0)
    }
}

/// Final sink volume: master × bus × the sink's own `PlaybackSettings` volume.
pub fn effective_volume(master: f32, bus: f32, playback: Volume) -> Volume {
    Volume::Linear(master * bus * playback.to_linear())
}

fn bus_for(is_music: bool, music: &MusicVolume, sfx: &SfxVolume) -> f32 {
    if is_music { music.0 } else { sfx.0 }
}

/// Re-apply volumes to running sinks when master or a bus changes.
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
```

Keep the `Music`/`SoundEffect` markers and `music()`/`sound_effect()` constructors unchanged. Delete the old `apply_global_volume`. (Non-music sinks all count as SFX — every current spawn site uses one of the two constructors.)

**Step 4: Run tests**

Run: `cargo test audio:: && cargo test`
Expected: audio test passes; full suite stays green.

**Step 5: Commit**

```bash
git add src/audio.rs
git commit -m "feat(ti4): music and SFX volume buses"
```

---

### Task 3: settings plugin — load, apply, capture, debounced save

**Files:**
- Modify: `src/settings.rs`, `src/main.rs:78-87`

**Step 1: Write the failing test** (append to `settings::tests`)

```rust
    #[test]
    fn save_and_load_file_round_trip() {
        let dir = std::env::temp_dir().join("guild-forge-settings-test");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("settings.ron");
        let mut s = GameSettings::default();
        s.master_volume = 0.7;
        s.fullscreen = true;
        write_settings_file(&path, &s);
        let loaded = parse_settings(&std::fs::read_to_string(&path).unwrap());
        assert_eq!(loaded, s);
        let _ = std::fs::remove_file(&path);
    }
```

**Step 2: Run to verify it fails**

Run: `cargo test settings::`
Expected: compile FAIL — `write_settings_file` not defined.

**Step 3: Implement plugin machinery** (in `src/settings.rs`)

```rust
use bevy::window::{MonitorSelection, PrimaryWindow, WindowMode};

use crate::audio::{MusicVolume, SfxVolume};
use crate::screens::EmberSettings;

/// Seconds after the last change before settings hit the disk.
const SAVE_DEBOUNCE_SECS: f32 = 1.0;

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

pub(super) fn plugin(app: &mut App) {
    app.insert_resource(load_settings());
    app.add_systems(Startup, apply_settings_on_startup);
    app.add_systems(Update, (capture_changed_resources, save_settings_when_dirty).chain());
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
    if !(global_volume.is_changed() || music.is_changed() || sfx.is_changed() || embers.is_changed()) {
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
```

Register in `src/main.rs` second plugin group, after `save::plugin,`:

```rust
            settings::plugin,
```

Note: `capture_changed_resources` uses `*settings != captured` to avoid triggering change detection every frame — that's why `GameSettings` derives `PartialEq`. `save_settings_when_dirty` runs chained after capture; `settings.is_changed()` is true on the insert frame, so one harmless save fires ~1s after launch.

**Step 4: Run tests**

Run: `cargo test settings:: && cargo test`
Expected: 4 settings tests pass; suite green. `cargo clippy --all-targets` — no new warnings (baseline 65).

**Step 5: Commit**

```bash
git add src/settings.rs src/main.rs
git commit -m "feat(ti4): persist settings to settings.ron with debounced save"
```

---

### Task 4: Settings menu — music/SFX rows, window mode toggle, resolution cycler

**Files:**
- Modify: `src/menus/settings.rs`

**Step 1: Add the new rows to the grid**

In `settings_grid()` add four label/widget pairs after the master-volume pair (labels styled like the existing ones, `justify_self = JustifySelf::End`):

```rust
    let mut music_label = widgets::label("Music Volume");
    music_label.style_mut().justify_self = JustifySelf::End;
    let mut sfx_label = widgets::label("SFX Volume");
    sfx_label.style_mut().justify_self = JustifySelf::End;
    let mut mode_label = widgets::label("Window Mode");
    mode_label.style_mut().justify_self = JustifySelf::End;
    let mut resolution_label = widgets::label("Resolution");
    resolution_label.style_mut().justify_self = JustifySelf::End;
```

Grid children order: master, music, sfx, ember density, ember warmth, window mode, resolution.

**Step 2: Widgets and handlers** (mirror `global_volume_widget` exactly)

```rust
const MIN_BUS_VOLUME: f32 = 0.0;
const MAX_BUS_VOLUME: f32 = 2.0;

fn music_volume_widget() -> Div { /* buttons lower_music_volume / raise_music_volume, label MusicVolumeLabel */ }
fn sfx_volume_widget() -> Div { /* buttons lower_sfx_volume / raise_sfx_volume, label SfxVolumeLabel */ }

fn lower_music_volume(_: On<Pointer<Click>>, mut v: ResMut<crate::audio::MusicVolume>) {
    v.0 = (v.0 - 0.1).max(MIN_BUS_VOLUME);
}
fn raise_music_volume(_: On<Pointer<Click>>, mut v: ResMut<crate::audio::MusicVolume>) {
    v.0 = (v.0 + 0.1).min(MAX_BUS_VOLUME);
}
// sfx pair identical against SfxVolume

fn window_mode_widget() -> Div {
    let mut container = div().row();
    container.style_mut().justify_self = JustifySelf::Start;
    container
        .insert(Name::new("Window Mode Widget"))
        .child(widgets::game_button_small("", toggle_window_mode).insert_into_label(WindowModeLabel))
}
```

*(Check `widgets::game_button_small`'s construction — if the label text entity isn't directly insertable, build the button like the existing widgets: a small button plus a labeled div in the row, `WindowModeLabel` on a `widgets::label("")` child, exactly like the volume value labels.)*

```rust
fn toggle_window_mode(
    _: On<Pointer<Click>>,
    mut settings: ResMut<crate::settings::GameSettings>,
    mut window_q: Query<&mut Window, With<bevy::window::PrimaryWindow>>,
) {
    settings.fullscreen = !settings.fullscreen;
    if let Ok(mut window) = window_q.single_mut() {
        crate::settings::apply_window_settings(&settings, &mut window);
    }
}

fn cycle_resolution_prev(_: On<Pointer<Click>>, settings: ResMut<crate::settings::GameSettings>, window_q: Query<&mut Window, With<bevy::window::PrimaryWindow>>) { cycle(settings, window_q, false); }
fn cycle_resolution_next(_: On<Pointer<Click>>, settings: ResMut<crate::settings::GameSettings>, window_q: Query<&mut Window, With<bevy::window::PrimaryWindow>>) { cycle(settings, window_q, true); }

fn cycle(
    mut settings: ResMut<crate::settings::GameSettings>,
    mut window_q: Query<&mut Window, With<bevy::window::PrimaryWindow>>,
    forward: bool,
) {
    settings.windowed_resolution =
        crate::settings::cycle_resolution(settings.windowed_resolution, forward);
    if !settings.fullscreen && let Ok(mut window) = window_q.single_mut() {
        crate::settings::apply_window_settings(&settings, &mut window);
    }
}
```

Resolution widget: `<` / `>` small buttons around a `ResolutionLabel` (like the volume widgets).

Label components + update systems (registered in the plugin's `Update` set alongside the existing three, `run_if(in_state(Menu::Settings))`):

```rust
fn update_music_volume_label(v: Res<crate::audio::MusicVolume>, mut label: Single<&mut Text, With<MusicVolumeLabel>>) {
    label.0 = format!("{:3.0}%", v.0 * 100.0);
}
// sfx analogous

fn update_window_mode_label(settings: Res<crate::settings::GameSettings>, mut label: Single<&mut Text, With<WindowModeLabel>>) {
    label.0 = if settings.fullscreen { "Fullscreen".into() } else { "Windowed".into() };
}

fn update_resolution_label(settings: Res<crate::settings::GameSettings>, mut label: Single<&mut Text, With<ResolutionLabel>>) {
    let (w, h) = settings.windowed_resolution;
    label.0 = format!("{w} × {h}");
}
```

**Step 3: Run the suite + clippy**

Run: `cargo test && cargo clippy --all-targets`
Expected: green; warning count unchanged (65 baseline).

**Step 4: Commit**

```bash
git add src/menus/settings.rs
git commit -m "feat(ti4): settings menu rows for buses, window mode, resolution"
```

---

### Task 5: Final gate — hand verification, tick chunk, merge

**Step 1:** `cargo test` (expect 96 + ~5 new ≈ 101 passing), `cargo clippy --all-targets` (no new warnings vs 65 baseline).

**Step 2: Hand-verify** (screen availability permitting — see memory `driving-game-for-verification`; do NOT drive the desktop while the user is using it):
1. Launch; open Settings from title. Seven rows render.
2. Toggle Window Mode → borderless fullscreen; toggle back → windowed at the chosen resolution.
3. Cycle resolution presets; window resizes live in windowed mode.
4. Lower SFX to 0 → button hover/click sounds go silent; master still affects credits music (Music bus check there).
5. Quit and relaunch → settings restored from `%APPDATA%..\guild-forge\settings.ron` (dirs::data_dir).
6. Delete `settings.ron` → launches with defaults, no crash; corrupt it → defaults, no crash.

**Step 3:** Tick TI-4 in `docs/steam-release-chunks.md` (line ~161) with date + note: rebinding skipped (mouse-driven game, decision noted), settings persist to settings.ron.

**Step 4: Merge**

```bash
git add docs/steam-release-chunks.md
git commit -m "docs: tick TI-4 — settings completeness shipped"
git checkout main
git merge --no-ff feat/ti4-settings -m "Merge branch 'feat/ti4-settings' — TI-4 settings completeness"
cargo test
git branch -d feat/ti4-settings
```
