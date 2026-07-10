# UX-4 · Audio States — Design

**Date:** 2026-07-09
**Chunk:** UX-4 (steam-release-chunks.md) — exploration/combat/boss music layers with crossfade plus ability SFX hooks. Code is LLM work; real tracks are human-led.

## Decisions

- **Guild theme everywhere** (user-approved): title + management tabs get a calm base state; mission view gets exploration/combat/boss. Four states, never silent.
- **Boss placeholder = combat track at 1.25× speed** (user-approved): audibly distinct with zero new assets; swapping a real track later is a one-line path change.

## Architecture

**States** — `MusicState { Guild, Exploration, Combat, Boss }` in new `src/music.rs`, held in a `CurrentMusicState` resource. Derivation per frame: viewing a mission → `Boss` when a boss enemy is inside any hero's action range during combat, `Combat` on any hero/enemy range overlap, else `Exploration`; any other screen/tab → `Guild`. The overlap rule is extracted from `update_simulation_tempo` into a shared pure `combat_overlap()` helper in `mission/mod.rs` so tempo and music can't drift apart.

**Crossfade** — four looping `AudioPlayer` entities spawn at startup (one per state, tagged `Music` + `MusicLayer(state)`), all at volume 0. Each frame the fade system ramps each layer's `PlaybackSettings.volume` toward 1 (active) / 0 (inactive) over ~0.8s, then applies `effective_volume(master, music_bus, playback)` — same formula as the TI-4 bus system, so volume sliders compose with fades. Track paths live in one const table (`MUSIC_TRACKS`). While `Menu::Credits` is open all layers duck to 0 (the credits screen spawns its own track).

**SFX hooks** — a bridge system reads `MissionLogEvent` for the viewed mission (same pattern as banners) and maps payload kinds → sounds: `Ability` and `Death` get placeholder step*.ogg picks; other kinds map to `None` until real assets land. Max 4 spawns/frame. Spawning uses the existing `sound_effect()` bundle so the SFX bus applies automatically.

## Testing

Pure helpers unit-tested: `combat_overlap()` (moved from tempo system), `target_state()` decision table, `approach()` fade math (clamp, converge, no overshoot), `MUSIC_TRACKS` covers every state. Audible behavior hand-verified in the running game.

## Out of scope

Real/commissioned tracks, per-biome themes, completion stingers, positional audio, pause-menu ducking.
