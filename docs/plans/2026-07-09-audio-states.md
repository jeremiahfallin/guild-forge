# UX-4 Audio States Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Guild/exploration/combat/boss music with crossfade, plus mission-event SFX hooks, all riding the TI-4 volume buses.

**Architecture:** New `src/music.rs` holds a `MusicState` machine: a derivation system computes `CurrentMusicState` from screen/tab + the shared combat-overlap rule (extracted from `update_simulation_tempo` into `mission/mod.rs`), and a crossfade system ramps four persistent looping `AudioPlayer` layers' `PlaybackSettings.volume` toward 1/0, applying `audio::effective_volume(master, music_bus, playback)`. An SFX bridge maps viewed-mission `MissionLogEvent` payloads to placeholder sounds via the existing `sound_effect()` bundle.

**Tech Stack:** Bevy 0.18 (binary crate — `cargo test music::` etc., not `--lib`). Design doc: `docs/plans/2026-07-09-audio-states-design.md`. House test style: `#[cfg(test)]` mod at file bottom, pure fns preferred, `run_system_once` for query helpers.

**Placeholder tracks:** Guild + Exploration = `audio/music/Fluffing A Duck.ogg` (1.0×); Combat = `audio/music/Monkeys Spinning Monkeys.ogg` (1.0×); Boss = same file at 1.25× speed. SFX placeholders = `audio/sound_effects/step{1..4}.ogg`.

---

### Task 1: Extract shared overlap helpers into `mission/mod.rs`

Three call sites duplicate class-range + Manhattan-overlap math: `update_simulation_tempo` (mission/mod.rs:174-229), `detect_boss_banner` (ui/banner.rs:91-146), and the new music derivation. Extract once.

**Files:**
- Modify: `src/mission/mod.rs` (helpers near `active_mission_count`, tests in existing `cap_tests` mod or new `overlap_tests`)
- Modify: `src/ui/banner.rs:112-116` (use helper)

**Step 1: Write failing tests** (new mod at bottom of `src/mission/mod.rs`)

```rust
#[cfg(test)]
mod overlap_tests {
    use super::*;
    use crate::hero::data::HeroClass;
    use entities::GridPosition;

    #[test]
    fn action_ranges_by_class() {
        assert_eq!(hero_action_range(&HeroClass::Ranger), 6);
        assert_eq!(hero_action_range(&HeroClass::Mage), 5);
        assert_eq!(hero_action_range(&HeroClass::Warrior), 1);
    }

    #[test]
    fn combat_overlap_boundary() {
        let heroes = [(GridPosition { x: 0, y: 0 }, 2)];
        // Manhattan distance exactly == range: overlap
        assert!(combat_overlap(&heroes, &[GridPosition { x: 1, y: 1 }]));
        // One further: no overlap
        assert!(!combat_overlap(&heroes, &[GridPosition { x: 2, y: 1 }]));
        // Empty lists: no combat
        assert!(!combat_overlap(&heroes, &[]));
        assert!(!combat_overlap(&[], &[GridPosition { x: 0, y: 0 }]));
    }
}
```

(Confirm `HeroClass::Warrior` exists in `src/hero/data.rs` — if the melee variant is named differently, use whatever non-Ranger/Mage variant exists.)

**Step 2: Run** `cargo test mission::overlap_tests` — expect compile FAIL (helpers missing).

**Step 3: Implement** (in `src/mission/mod.rs`, near `active_mission_count`)

```rust
/// Action range in tiles for combat-overlap checks (encounter enrollment,
/// tempo, music, boss banner). Single source of truth.
pub fn hero_action_range(class: &crate::hero::data::HeroClass) -> u32 {
    match class {
        crate::hero::data::HeroClass::Ranger => 6,
        crate::hero::data::HeroClass::Mage => 5,
        _ => 1,
    }
}

/// True when any hero's action range reaches any enemy (Manhattan distance).
pub fn combat_overlap(
    heroes: &[(entities::GridPosition, u32)],
    enemies: &[entities::GridPosition],
) -> bool {
    heroes.iter().any(|(h_gp, h_range)| {
        enemies
            .iter()
            .any(|e_gp| h_gp.x.abs_diff(e_gp.x) + h_gp.y.abs_diff(e_gp.y) <= *h_range)
    })
}
```

Refactor `update_simulation_tempo` (~lines 192-219): replace the inline range match with `hero_action_range(&info.class)` and the nested distance loops with `in_combat = combat_overlap(&heroes_list, &enemies_list);`. Refactor `detect_boss_banner` in `src/ui/banner.rs`: replace the inline range match with `crate::mission::hero_action_range(&info.class)`; keep its per-boss loop (it needs the boss type for the subtitle) but swap the distance test to reuse `combat_overlap(&[(h_gp, h_range)], &[b_gp])` only if that stays readable — otherwise leave the distance line and just share the range fn.

**Step 4: Run** `cargo test` — full suite green (existing banner tests are the regression net for the refactor). `cargo clippy --all-targets` — 65-warning baseline.

**Step 5: Commit**

```bash
git add src/mission/mod.rs src/ui/banner.rs
git commit -m "refactor(ux4): shared hero_action_range/combat_overlap helpers"
```

---

### Task 2: `music.rs` core — state enum, decision fn, fade math, track table

**Files:**
- Create: `src/music.rs`
- Modify: `src/main.rs` (add `mod music;` after `mod mission;`)

**Step 1: Create `src/music.rs` with tests** (tests first — module compiles standalone)

```rust
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
    (MusicState::Combat, "audio/music/Monkeys Spinning Monkeys.ogg", 1.0),
    (MusicState::Boss, "audio/music/Monkeys Spinning Monkeys.ogg", 1.25),
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
```

**Step 2:** Add `mod music;` to `src/main.rs` (module list, after `mod menus;`... keep alphabetical: between `mod menus;` and `mod mission;`). Run `cargo test music::` — expect PASS (3 tests; if compile errors, fix before proceeding).

**Step 3: Commit**

```bash
git add src/music.rs src/main.rs
git commit -m "feat(ux4): music state machine core - states, decision, fade math"
```

---

### Task 3: Layer entities, state derivation, crossfade systems

**Files:**
- Modify: `src/music.rs`, `src/main.rs` (plugin registration)

**Step 1: Implement plugin + systems** (compile-driven; the pure logic is already tested)

```rust
use bevy::audio::Volume;

use crate::audio::{Music, MusicVolume, effective_volume};
use crate::menus::Menu;
use crate::mission::entities::{EnemyToken, GridPosition, HeroToken};
use crate::mission::{ViewedMission, combat_overlap, hero_action_range};
use crate::screens::{GameTab, Screen};

/// The state the crossfade is currently steering toward.
#[derive(Resource, Debug, Default, PartialEq)]
pub struct CurrentMusicState(pub MusicState);

/// Tags one of the four persistent looping layer entities.
#[derive(Component, Debug)]
pub struct MusicLayer(pub MusicState);

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<CurrentMusicState>();
    app.add_systems(Startup, spawn_music_layers);
    app.add_systems(Update, (derive_music_state, crossfade_music).chain());
}

fn spawn_music_layers(mut commands: Commands, asset_server: Res<AssetServer>) {
    for &(state, path, speed) in MUSIC_TRACKS {
        commands.spawn((
            Name::new(format!("Music Layer {state:?}")),
            AudioPlayer(asset_server.load(path)),
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
    missions: Query<&Children, With<crate::mission::Mission>>,
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
        let target = if !credits_open && layer.0 == current.0 { 1.0 } else { 0.0 };
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
```

Register in `src/main.rs` first plugin group after `menus::plugin,`: add `music::plugin,`.

Notes:
- `Screen`/`GameTab` visibility: both are `pub` in `src/screens/mod.rs` (`crate::screens::{GameTab, Screen}`); `Menu` is `pub` in `src/menus/mod.rs`. If `GameTab` is a SubState, `Res<State<GameTab>>` only exists during Gameplay — hence `Option<Res<State<GameTab>>>`.
- The crossfade writes `PlaybackSettings.volume` as the persisted fade level; `audio::apply_volumes` (bus changes) uses the same field, so the two systems agree by construction.

**Step 2: Run** `cargo test && cargo clippy --all-targets` — suite green (101 + 3 music = 104), warnings at 65 baseline. Fix any query/state-type compile mismatches by checking `src/screens/mod.rs` state definitions.

**Step 3: Commit**

```bash
git add src/music.rs src/main.rs
git commit -m "feat(ux4): music layers with state derivation and crossfade"
```

---

### Task 4: Mission-event SFX bridge

**Files:**
- Modify: `src/music.rs`

**Step 1: Write failing test** (append to `music::tests`)

```rust
    #[test]
    fn sfx_mapping_covers_placeholder_events() {
        use crate::ui::feed::MissionLogPayload;
        let ability = MissionLogPayload::Ability {
            attacker: "A".into(),
            defender: "B".into(),
            ability_name: "Fireball".into(),
            amount: 3,
            is_hit: true,
            is_crit: false,
            effect_type: crate::ui::feed::LogKind::Combat,
        };
        assert!(sfx_for(&ability).is_some());
        let death = MissionLogPayload::Death { name: "C".into(), is_hero: true };
        assert!(sfx_for(&death).is_some());
        let room = MissionLogPayload::RoomEntry { hero_name: "A".into(), room_name: "R".into() };
        assert!(sfx_for(&room).is_none());
    }
```

**IMPORTANT:** the payload field names above are from memory of `src/ui/feed.rs:54-110` — open that range first and copy the real field names/types for `Ability`, `Death`, and `RoomEntry` construction. The mapping behavior is the thing under test, not the literals.

**Step 2: Run** `cargo test music::` — expect compile FAIL (`sfx_for` missing).

**Step 3: Implement** (in `src/music.rs`)

```rust
use crate::audio::sound_effect;
use crate::ui::feed::{MissionLogEvent, MissionLogPayload};
use bevy::ecs::message::MessageReader;

/// Max mission SFX spawned per frame — a busy turn shouldn't machine-gun the mixer.
const MAX_SFX_PER_FRAME: usize = 4;

/// Which placeholder pool a log event maps to. `None` = silent until real
/// assets land (human-led).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SfxKind {
    AbilityCast,
    Death,
}

pub fn sfx_for(payload: &MissionLogPayload) -> Option<SfxKind> {
    match payload {
        MissionLogPayload::Ability { .. } => Some(SfxKind::AbilityCast),
        MissionLogPayload::Death { .. } => Some(SfxKind::Death),
        _ => None,
    }
}

/// Placeholder pool: the four step sounds, rotated.
#[derive(Resource)]
struct SfxAssets(Vec<Handle<AudioSource>>);

fn load_sfx_assets(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.insert_resource(SfxAssets(
        (1..=4)
            .map(|i| asset_server.load(format!("audio/sound_effects/step{i}.ogg")))
            .collect(),
    ));
}

/// Play placeholder sounds for banner-worthy log events on the viewed mission.
fn mission_sfx_bridge(
    mut commands: Commands,
    mut events: MessageReader<MissionLogEvent>,
    viewed: Option<Res<ViewedMission>>,
    assets: Option<Res<SfxAssets>>,
    mut rotation: Local<usize>,
) {
    let (Some(viewed), Some(assets)) = (viewed, assets) else {
        events.clear();
        return;
    };
    let mut spawned = 0;
    for event in events.read() {
        if spawned >= MAX_SFX_PER_FRAME {
            break;
        }
        if event.mission_entity != viewed.0 {
            continue;
        }
        if sfx_for(&event.payload).is_some() {
            let handle = assets.0[*rotation % assets.0.len()].clone();
            *rotation = rotation.wrapping_add(1);
            commands.spawn((Name::new("Mission SFX"), sound_effect(handle)));
            spawned += 1;
        }
    }
}
```

Add to the plugin: `load_sfx_assets` in `Startup`, `mission_sfx_bridge` in `Update` chained after `crossfade_music`, `.run_if(bevy::state::condition::in_state(GameTab::MissionView))` — match how other GameTab-gated systems are registered (see `src/mission/mod.rs:79-87`). Check `MissionLogEvent` field name (`mission_entity`) in `src/ui/feed.rs` before wiring.

**Step 4: Run** `cargo test && cargo clippy --all-targets` — green, baseline warnings.

**Step 5: Commit**

```bash
git add src/music.rs
git commit -m "feat(ux4): mission-event SFX bridge with placeholder sounds"
```

---

### Task 5: Final gate — hand verification, tick, merge

**Step 1:** `cargo test` (expect ~105 passing), `cargo clippy --all-targets` (65-warning baseline).

**Step 2: Hand-verify** (check the desktop is free first — see memory `driving-game-for-verification`; the agent can't hear audio, so verify state transitions via the `info!("Music state -> ...")` log lines in the cargo-run output file, and leave listening confirmation to the user):
1. Launch → log shows no state line yet (Guild is default); title screen should be playing the guild track.
2. Play → guild tabs: still Guild (no transition logged).
3. Dispatch a party to Goblin Cave → mission view: log `Music state -> Exploration`.
4. Wait for first encounter (feed shows attacks; tempo drops): log `Music state -> Combat`.
5. If the party reaches the boss room: log `Music state -> Boss`. (BossRat missions end at the boss, so letting it run gets there; if the party dies first, redispatch.)
6. Leave mission view mid-fight → `Music state -> Guild`; return → Combat/Exploration again.
7. No panics in the output file; SFX spawns don't error (look for asset warnings).

**Step 3:** Tick UX-4 in `docs/steam-release-chunks.md` (~line 74) with date + note: state machine + crossfade + SFX bridge shipped on placeholder tracks; real music still human-led (appendix item stays open).

**Step 4: Merge**

```bash
git add docs/steam-release-chunks.md
git commit -m "docs: tick UX-4 — audio states shipped on placeholders"
git checkout main
git merge --no-ff feat/ux4-audio-states -m "Merge branch 'feat/ux4-audio-states' — UX-4 audio states"
cargo test
git branch -d feat/ux4-audio-states
```
