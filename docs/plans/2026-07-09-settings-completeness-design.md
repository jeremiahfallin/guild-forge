# TI-4 · Settings Completeness — Design

**Date:** 2026-07-09
**Chunk:** TI-4 (steam-release-chunks.md) — resolution/fullscreen, separate music/SFX volume buses, key rebinding if feasible.

## Decisions

- **Key rebinding: skipped** (user-approved). The game is mouse-driven; the only bound keys are Escape/P (pause) and the dev console. Revisit if keyboard shortcuts grow.
- **Persistence: `settings.ron` beside `save.ron`** (user-approved) at `<data_dir>/guild-forge/settings.ron`. Machine-level, survives save deletion and new games.

## Architecture

**`GameSettings` resource** (new module `src/settings.rs`): master/music/sfx volumes, ember density/warmth, window mode, windowed resolution. All fields `#[serde(default)]`; corrupt or missing file falls back to `Default`. Loaded at startup and applied to the live resources (`GlobalVolume`, `EmberSettings`, primary `Window`); any change is captured back into `GameSettings` and written to disk after a ~1s debounce.

**Audio buses**: new `MusicVolume` and `SfxVolume` resources (linear 0–2, default 1). `apply_global_volume` in `src/audio.rs` generalizes to: sink volume = master × bus × `PlaybackSettings` volume, bus chosen by the existing `Music` / `SoundEffect` markers. Pure helper `effective_volume(master, bus, playback) -> Volume` carries the math (unit-testable; also used when spawning new sounds is not needed — sinks are updated by the system on resource change). This is the seam UX-4 audio states will use.

**Display**: window-mode toggle Windowed ↔ `BorderlessFullscreen(Current)` (exclusive fullscreen intentionally skipped) plus a resolution preset cycler (1280×720, 1600×900, 1920×1080, 2560×1440) applied to the primary `Window` when windowed; while fullscreen the choice takes effect on return to windowed.

**UI** (`src/menus/settings.rs`): grid grows to seven rows — Master/Music/SFX volume, Ember Density/Warmth, Window Mode (single toggle button), Resolution (`<` value `>` cycler). All reuse the existing small-button widget style; no new widget types.

## Testing

Serde round-trip of `GameSettings`; parse of an empty/partial ron string yields defaults (forward compat); volume clamping; resolution cycling wraps both directions; `effective_volume` multiplier. UI/window behavior hand-verified in the running game.

## Out of scope

Key rebinding, exclusive fullscreen, monitor selection, UI scale, audio tracks themselves (UX-4).
