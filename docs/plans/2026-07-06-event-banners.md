# Event Banners (UX-3) Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Floating banners in the mission view ("BOSS ENCOUNTER", "RARE DROP!", "RESCUE WINDOW CLOSING") driven by the UX-1 event stream, showing one at a time and clearing themselves.

**Architecture:** A new `src/ui/banner.rs` module owns a `BannerQueue` resource fed by three producer systems (a `MissionLogEvent` bridge for legendary drops, a boss-overlap detector, a rescue-timer watcher), all scoped to the viewed mission with one-shot markers on the mission entity. A thin render system in `mission_view.rs` animates the active banner (slide in / hold / fade) from pure phase functions.

**Tech Stack:** Bevy 0.18, bevy_declarative for UI nodes, `run_system_once` unit tests (existing house style — see `src/mission/combat.rs` tests).

**Design doc:** `docs/plans/2026-07-06-event-banners-design.md` (decisions: bridge architecture, boss fires on first combat overlap, Legendary-only drops, rescue threshold 30 game-seconds).

---

### Task 1: Banner data types + legendary-drop bridge

**Files:**
- Create: `src/ui/banner.rs`
- Modify: `src/ui/mod.rs` (register module + plugin)
- Test: inline `#[cfg(test)]` in `src/ui/banner.rs`

**Step 1: Create the module with types and a failing test**

Create `src/ui/banner.rs`:

```rust
//! Event banners (UX-3): full-width floating banners in the mission view for
//! banner-worthy moments — boss encounters, legendary drops, rescue windows
//! closing. Producers enqueue; the mission view renders one at a time.

use bevy::prelude::*;
use std::collections::VecDeque;

use crate::equipment::GearRarity;
use crate::mission::ViewedMission;
use crate::ui::feed::{MissionLogEvent, MissionLogPayload};
use bevy::ecs::message::MessageReader;

/// Category of banner — drives styling in the mission view.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BannerKind {
    Boss,
    RareDrop,
    RescueClosing,
}

/// A queued banner announcement.
#[derive(Debug, Clone)]
pub struct BannerRequest {
    pub text: String,
    pub subtitle: Option<String>,
    pub kind: BannerKind,
}

/// Pending banners plus the elapsed time of the one currently showing.
/// `active` is popped from `pending` by the render side when the previous
/// banner finishes.
#[derive(Resource, Debug, Default)]
pub struct BannerQueue {
    pub pending: VecDeque<BannerRequest>,
    pub active: Option<(BannerRequest, f32)>,
    /// Mission the queue currently belongs to — cleared when the view changes.
    pub mission: Option<Entity>,
}

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<BannerQueue>();
    app.add_systems(
        Update,
        (
            detect_drop_banners,
            detect_boss_banner,
            detect_rescue_banner,
        )
            .run_if(resource_exists::<ViewedMission>),
    );
}

/// Promote legendary `GearDrop` log events on the viewed mission to banners.
pub(crate) fn detect_drop_banners(
    mut events: MessageReader<MissionLogEvent>,
    viewed: Res<ViewedMission>,
    mut queue: ResMut<BannerQueue>,
) {
    for event in events.read() {
        if event.mission_entity != viewed.0 {
            continue;
        }
        if let MissionLogPayload::GearDrop {
            item_name, rarity, ..
        } = &event.payload
            && *rarity == GearRarity::Legendary
        {
            queue.pending.push_back(BannerRequest {
                text: "RARE DROP!".to_string(),
                subtitle: Some(item_name.clone()),
                kind: BannerKind::RareDrop,
            });
        }
    }
}
```

(Leave `detect_boss_banner` and `detect_rescue_banner` as empty stubs for now so the plugin compiles:)

```rust
pub(crate) fn detect_boss_banner() {}
pub(crate) fn detect_rescue_banner() {}
```

Add the test module at the bottom:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;
    use bevy::ecs::message::Messages;

    fn send_drop(world: &mut World, mission: Entity, rarity: GearRarity) {
        world
            .resource_mut::<Messages<MissionLogEvent>>()
            .write(MissionLogEvent {
                mission_entity: mission,
                payload: MissionLogPayload::GearDrop {
                    hero_name: "Sera".to_string(),
                    item_name: "Doomblade".to_string(),
                    rarity,
                    affix: None,
                    stats_desc: "+5 ATK".to_string(),
                },
            });
    }

    #[test]
    fn legendary_drop_on_viewed_mission_enqueues_banner() {
        let mut world = World::new();
        world.init_resource::<Messages<MissionLogEvent>>();
        world.init_resource::<BannerQueue>();
        let mission = world.spawn_empty().id();
        world.insert_resource(ViewedMission(mission));

        send_drop(&mut world, mission, GearRarity::Legendary);
        let _ = world.run_system_once(detect_drop_banners);

        let queue = world.resource::<BannerQueue>();
        assert_eq!(queue.pending.len(), 1);
        let req = &queue.pending[0];
        assert_eq!(req.kind, BannerKind::RareDrop);
        assert_eq!(req.text, "RARE DROP!");
        assert_eq!(req.subtitle.as_deref(), Some("Doomblade"));
    }

    #[test]
    fn epic_drop_does_not_enqueue_banner() {
        let mut world = World::new();
        world.init_resource::<Messages<MissionLogEvent>>();
        world.init_resource::<BannerQueue>();
        let mission = world.spawn_empty().id();
        world.insert_resource(ViewedMission(mission));

        send_drop(&mut world, mission, GearRarity::Epic);
        let _ = world.run_system_once(detect_drop_banners);

        assert!(world.resource::<BannerQueue>().pending.is_empty());
    }

    #[test]
    fn legendary_drop_on_unviewed_mission_does_not_enqueue() {
        let mut world = World::new();
        world.init_resource::<Messages<MissionLogEvent>>();
        world.init_resource::<BannerQueue>();
        let viewed = world.spawn_empty().id();
        let other = world.spawn_empty().id();
        world.insert_resource(ViewedMission(viewed));

        send_drop(&mut world, other, GearRarity::Legendary);
        let _ = world.run_system_once(detect_drop_banners);

        assert!(world.resource::<BannerQueue>().pending.is_empty());
    }
}
```

Register in `src/ui/mod.rs`:

```rust
pub mod banner;
pub mod toast;
pub mod feed;

use bevy::prelude::*;

pub(super) fn plugin(app: &mut App) {
    app.add_plugins(toast::plugin);
    app.add_plugins(feed::plugin);
    app.add_plugins(banner::plugin);
}
```

**Step 2: Run the tests — they must fail first**

Write the test module FIRST (with `use super::*` against the not-yet-written systems), run, watch the compile failure, then add the implementation and re-run. If you wrote both at once, at minimum verify each assertion by temporarily breaking the rarity match (`GearRarity::Epic`) and watching the first test fail for the right reason, then restore.

Run: `cargo test --lib ui::banner`
Expected: 3 passed.

**Step 3: Full check + commit**

Run: `cargo test && cargo clippy --all-targets -- -D warnings`

```bash
git add src/ui/banner.rs src/ui/mod.rs
git commit -m "feat(ux3): banner queue + legendary-drop bridge from feed stream"
```

---

### Task 2: Boss-encounter detection

**Files:**
- Modify: `src/ui/banner.rs` (replace `detect_boss_banner` stub)
- Modify: `src/mission/data.rs` (add `EnemyType::is_boss`)

**Step 1: Add `is_boss` to `EnemyType`** (`src/mission/data.rs`, next to the `Display` impl):

```rust
impl EnemyType {
    /// Boss-tier enemies get the BOSS ENCOUNTER banner and boss-room spawns.
    pub fn is_boss(&self) -> bool {
        matches!(self, Self::BossRat)
    }
}
```

While here, replace the hardcoded check at `src/mission/entities.rs:351`
(`let spawn_in_boss_room = enemy_type == EnemyType::BossRat;`) with
`let spawn_in_boss_room = enemy_type.is_boss();` so the definition stays single-sourced.

**Step 2: Write the failing tests**

The one-shot marker goes on the mission entity:

```rust
/// One-shot bookkeeping so boss / rescue banners fire once per mission.
#[derive(Component, Debug, Default)]
pub struct BannersFired {
    pub boss: bool,
    pub rescue: bool,
}
```

Tests (in the existing `mod tests`). Mirror the combat-range overlap setup used by `update_simulation_tempo` (`src/mission/mod.rs:174-229`): mission entity with children carrying `HeroToken`/`EnemyToken` + `GridPosition`; the hero roster entity carries `HeroInfo` (class Warrior → range 1).

```rust
use crate::mission::entities::{EnemyToken, GridPosition, HeroToken};
use crate::mission::data::EnemyType;
use crate::hero::{Hero, HeroInfo};

fn spawn_boss_fixture(world: &mut World, boss_pos: (u32, u32)) -> Entity {
    let hero = world
        .spawn((
            Hero,
            HeroInfo {
                name: "Alice".to_string(),
                class: crate::hero::data::HeroClass::Warrior,
                level: 1,
                xp: 0,
                xp_to_next: 100,
            },
        ))
        .id();
    let mission = world.spawn_empty().id();
    let hero_token = world
        .spawn((HeroToken(hero), GridPosition { x: 5, y: 5 }))
        .id();
    let boss_token = world
        .spawn((
            EnemyToken { enemy_type: EnemyType::BossRat, xp_reward: 50 },
            GridPosition { x: boss_pos.0, y: boss_pos.1 },
        ))
        .id();
    world.entity_mut(mission).add_children(&[hero_token, boss_token]);
    world.insert_resource(ViewedMission(mission));
    mission
}

#[test]
fn boss_in_combat_range_enqueues_banner_once() {
    let mut world = World::new();
    world.init_resource::<BannerQueue>();
    let _mission = spawn_boss_fixture(&mut world, (6, 5)); // adjacent, warrior range 1

    let _ = world.run_system_once(detect_boss_banner);
    let _ = world.run_system_once(detect_boss_banner); // second tick must not refire

    let queue = world.resource::<BannerQueue>();
    assert_eq!(queue.pending.len(), 1);
    assert_eq!(queue.pending[0].kind, BannerKind::Boss);
    assert_eq!(queue.pending[0].text, "BOSS ENCOUNTER");
    assert_eq!(queue.pending[0].subtitle.as_deref(), Some("Boss Rat"));
}

#[test]
fn boss_out_of_range_does_not_enqueue() {
    let mut world = World::new();
    world.init_resource::<BannerQueue>();
    let _mission = spawn_boss_fixture(&mut world, (20, 20));

    let _ = world.run_system_once(detect_boss_banner);

    assert!(world.resource::<BannerQueue>().pending.is_empty());
}

#[test]
fn non_boss_enemy_in_range_does_not_enqueue() {
    // same fixture but EnemyType::Goblin adjacent — build inline
    let mut world = World::new();
    world.init_resource::<BannerQueue>();
    let hero = world.spawn((Hero, HeroInfo {
        name: "Alice".to_string(),
        class: crate::hero::data::HeroClass::Warrior,
        level: 1, xp: 0, xp_to_next: 100,
    })).id();
    let mission = world.spawn_empty().id();
    let ht = world.spawn((HeroToken(hero), GridPosition { x: 5, y: 5 })).id();
    let et = world.spawn((
        EnemyToken { enemy_type: EnemyType::Goblin, xp_reward: 10 },
        GridPosition { x: 6, y: 5 },
    )).id();
    world.entity_mut(mission).add_children(&[ht, et]);
    world.insert_resource(ViewedMission(mission));

    let _ = world.run_system_once(detect_boss_banner);

    assert!(world.resource::<BannerQueue>().pending.is_empty());
}
```

Run: `cargo test --lib ui::banner`
Expected: new tests FAIL (stub does nothing).

**Step 3: Implement `detect_boss_banner`**

Replace the stub. Same shape as `update_simulation_tempo`'s overlap scan, but filtered to boss enemies and gated by `BannersFired`:

```rust
/// Fire the BOSS ENCOUNTER banner the first time a boss-tier enemy on the
/// viewed mission comes within a hero's action range (the same overlap test
/// that flips the sim into combat tempo).
pub(crate) fn detect_boss_banner(
    mut commands: Commands,
    viewed: Res<ViewedMission>,
    missions: Query<(&Children, Option<&BannersFired>)>,
    hero_tokens: Query<(&GridPosition, &HeroToken), Without<EnemyToken>>,
    enemy_tokens: Query<(&GridPosition, &EnemyToken), Without<HeroToken>>,
    hero_data: Query<&HeroInfo, With<Hero>>,
    mut queue: ResMut<BannerQueue>,
) {
    let Ok((children, fired)) = missions.get(viewed.0) else { return };
    if fired.is_some_and(|f| f.boss) {
        return;
    }

    let mut heroes = Vec::new();
    let mut bosses = Vec::new();
    for &child in children {
        if let Ok((gp, token)) = hero_tokens.get(child)
            && let Ok(info) = hero_data.get(token.0)
        {
            let range = match info.class {
                crate::hero::data::HeroClass::Ranger => 6,
                crate::hero::data::HeroClass::Mage => 5,
                _ => 1,
            };
            heroes.push((*gp, range));
        } else if let Ok((gp, token)) = enemy_tokens.get(child)
            && token.enemy_type.is_boss()
        {
            bosses.push((*gp, token.enemy_type));
        }
    }

    for (h_gp, h_range) in &heroes {
        for (b_gp, boss_type) in &bosses {
            let dist = h_gp.x.abs_diff(b_gp.x) + h_gp.y.abs_diff(b_gp.y);
            if dist <= *h_range {
                queue.pending.push_back(BannerRequest {
                    text: "BOSS ENCOUNTER".to_string(),
                    subtitle: Some(boss_type.to_string()),
                    kind: BannerKind::Boss,
                });
                let mut fired = BannersFired::default();
                fired.boss = true;
                // preserve rescue flag if the component already exists
                commands.entity(viewed.0).entry::<BannersFired>()
                    .and_modify(|mut f| f.boss = true)
                    .or_insert(fired);
                return;
            }
        }
    }
}
```

Check `GridPosition` field types before writing the distance line — if `x`/`y` are `i32` (not `u32`), use `(h_gp.x - b_gp.x).unsigned_abs() + ...` and make `range`/`dist` types agree (see how `update_simulation_tempo` does it and copy that exactly).

**Step 4: Run the tests**

Run: `cargo test --lib ui::banner`
Expected: all pass.

**Step 5: Full check + commit**

Run: `cargo test && cargo clippy --all-targets -- -D warnings`

```bash
git add src/ui/banner.rs src/mission/data.rs src/mission/entities.rs
git commit -m "feat(ux3): one-shot BOSS ENCOUNTER banner on first combat overlap"
```

---

### Task 3: Rescue-window watcher

**Files:**
- Modify: `src/ui/banner.rs` (replace `detect_rescue_banner` stub)

**Step 1: Write the failing tests**

`Missing.expires_at` is in `Time<Virtual>` elapsed-seconds (see `src/hero/status.rs`, `src/hero/status_tick.rs:43`). Threshold constant: 30.0.

```rust
use crate::hero::status::Missing;
use crate::mission::RescueMission;

fn spawn_rescue_fixture(world: &mut World, expires_at: f64) -> Entity {
    world.init_resource::<Time<Virtual>>(); // elapsed starts at 0.0
    let missing_hero = world
        .spawn(Missing { expires_at, dropped_equipment: None })
        .id();
    let mission = world
        .spawn(RescueMission { rescue_heroes: vec![missing_hero], gear_recovered: false })
        .id();
    world.insert_resource(ViewedMission(mission));
    mission
}

#[test]
fn rescue_under_30s_enqueues_banner_once() {
    let mut world = World::new();
    world.init_resource::<BannerQueue>();
    let _mission = spawn_rescue_fixture(&mut world, 20.0); // 20s left at t=0

    let _ = world.run_system_once(detect_rescue_banner);
    let _ = world.run_system_once(detect_rescue_banner);

    let queue = world.resource::<BannerQueue>();
    assert_eq!(queue.pending.len(), 1);
    assert_eq!(queue.pending[0].kind, BannerKind::RescueClosing);
    assert_eq!(queue.pending[0].text, "RESCUE WINDOW CLOSING");
}

#[test]
fn rescue_over_30s_does_not_enqueue() {
    let mut world = World::new();
    world.init_resource::<BannerQueue>();
    let _mission = spawn_rescue_fixture(&mut world, 90.0);

    let _ = world.run_system_once(detect_rescue_banner);

    assert!(world.resource::<BannerQueue>().pending.is_empty());
}
```

Run: `cargo test --lib ui::banner` — new tests FAIL.

**Step 2: Implement**

```rust
/// Game-seconds remaining on the Missing window below which the
/// RESCUE WINDOW CLOSING banner fires (window is 120s — last quarter).
pub const RESCUE_BANNER_THRESHOLD_SECS: f64 = 30.0;

/// Fire RESCUE WINDOW CLOSING once when the viewed rescue mission's soonest
/// Missing timer drops under the threshold.
pub(crate) fn detect_rescue_banner(
    mut commands: Commands,
    viewed: Res<ViewedMission>,
    missions: Query<(&RescueMission, Option<&BannersFired>)>,
    missing_q: Query<&Missing>,
    time: Res<Time<Virtual>>,
    mut queue: ResMut<BannerQueue>,
) {
    let Ok((rescue, fired)) = missions.get(viewed.0) else { return };
    if fired.is_some_and(|f| f.rescue) {
        return;
    }
    let now = time.elapsed_secs_f64();
    let soonest = rescue
        .rescue_heroes
        .iter()
        .filter_map(|&h| missing_q.get(h).ok())
        .map(|m| m.expires_at - now)
        .fold(f64::INFINITY, f64::min);
    if soonest.is_finite() && soonest < RESCUE_BANNER_THRESHOLD_SECS {
        queue.pending.push_back(BannerRequest {
            text: "RESCUE WINDOW CLOSING".to_string(),
            subtitle: None,
            kind: BannerKind::RescueClosing,
        });
        commands.entity(viewed.0).entry::<BannersFired>()
            .and_modify(|mut f| f.rescue = true)
            .or_insert(BannersFired { rescue: true, ..default() });
    }
}
```

Note: `BannersFired` needs `boss: false` reachable via `..default()` — it derives `Default`, fine.

**Step 3: Run, then full check + commit**

Run: `cargo test --lib ui::banner`, then `cargo test && cargo clippy --all-targets -- -D warnings`

```bash
git add src/ui/banner.rs
git commit -m "feat(ux3): RESCUE WINDOW CLOSING banner under 30s remaining"
```

---

### Task 4: Queue lifecycle — activation, expiry, view-change clearing

**Files:**
- Modify: `src/ui/banner.rs`

**Step 1: Write the failing tests**

Banner lifetime phases (real seconds): slide-in 0.3, hold 2.5, fade 0.5 → total 3.3. Keep phase math pure so it's trivially testable:

```rust
#[test]
fn tick_promotes_pending_to_active_and_expires_it() {
    let mut world = World::new();
    world.init_resource::<Time>(); // real time, starts at 0
    let mission = world.spawn_empty().id();
    world.insert_resource(ViewedMission(mission));
    let mut queue = BannerQueue::default();
    queue.pending.push_back(BannerRequest {
        text: "BOSS ENCOUNTER".to_string(),
        subtitle: None,
        kind: BannerKind::Boss,
    });
    queue.pending.push_back(BannerRequest {
        text: "RARE DROP!".to_string(),
        subtitle: None,
        kind: BannerKind::RareDrop,
    });
    world.insert_resource(queue);

    let _ = world.run_system_once(tick_banner_queue);
    assert_eq!(
        world.resource::<BannerQueue>().active.as_ref().unwrap().0.kind,
        BannerKind::Boss
    );

    // advance past the banner's total lifetime
    world
        .resource_mut::<Time>()
        .advance_by(std::time::Duration::from_secs_f32(BANNER_TOTAL_SECS + 0.1));
    let _ = world.run_system_once(tick_banner_queue);
    assert_eq!(
        world.resource::<BannerQueue>().active.as_ref().unwrap().0.kind,
        BannerKind::RareDrop,
        "expired banner should be replaced by the next pending one"
    );
    assert!(world.resource::<BannerQueue>().pending.is_empty());
}

#[test]
fn view_change_clears_queue() {
    let mut world = World::new();
    world.init_resource::<Time>();
    let mission_a = world.spawn_empty().id();
    let mission_b = world.spawn_empty().id();
    world.insert_resource(ViewedMission(mission_a));
    let mut queue = BannerQueue::default();
    queue.pending.push_back(BannerRequest {
        text: "BOSS ENCOUNTER".to_string(),
        subtitle: None,
        kind: BannerKind::Boss,
    });
    world.insert_resource(queue);

    let _ = world.run_system_once(tick_banner_queue); // binds queue to mission_a, activates
    world.insert_resource(ViewedMission(mission_b));
    let _ = world.run_system_once(tick_banner_queue);

    let queue = world.resource::<BannerQueue>();
    assert!(queue.active.is_none(), "active banner must clear on view change");
    assert!(queue.pending.is_empty());
}

#[test]
fn banner_alpha_phases() {
    assert_eq!(banner_alpha(0.0), 0.0);           // slide-in start
    assert_eq!(banner_alpha(0.3), 1.0);           // hold
    assert_eq!(banner_alpha(2.0), 1.0);           // still holding
    assert!(banner_alpha(3.1) < 1.0);             // fading
    assert_eq!(banner_alpha(BANNER_TOTAL_SECS), 0.0);
}
```

Run: `cargo test --lib ui::banner` — FAIL (symbols missing).

**Step 2: Implement**

```rust
pub const BANNER_SLIDE_SECS: f32 = 0.3;
pub const BANNER_HOLD_SECS: f32 = 2.5;
pub const BANNER_FADE_SECS: f32 = 0.5;
pub const BANNER_TOTAL_SECS: f32 = BANNER_SLIDE_SECS + BANNER_HOLD_SECS + BANNER_FADE_SECS;

/// Opacity over the banner's lifetime: ramp up during slide-in, solid hold,
/// ramp down during fade.
pub fn banner_alpha(elapsed: f32) -> f32 {
    if elapsed <= 0.0 {
        0.0
    } else if elapsed < BANNER_SLIDE_SECS {
        elapsed / BANNER_SLIDE_SECS
    } else if elapsed < BANNER_SLIDE_SECS + BANNER_HOLD_SECS {
        1.0
    } else if elapsed < BANNER_TOTAL_SECS {
        1.0 - (elapsed - BANNER_SLIDE_SECS - BANNER_HOLD_SECS) / BANNER_FADE_SECS
    } else {
        0.0
    }
}

/// Advance the active banner, promote pending ones, and reset the queue when
/// the viewed mission changes.
pub(crate) fn tick_banner_queue(
    viewed: Res<ViewedMission>,
    time: Res<Time>,
    mut queue: ResMut<BannerQueue>,
) {
    if queue.mission != Some(viewed.0) {
        queue.pending.clear();
        queue.active = None;
        queue.mission = Some(viewed.0);
        return;
    }
    if let Some((_, ref mut elapsed)) = queue.active {
        *elapsed += time.delta_secs();
        if *elapsed >= BANNER_TOTAL_SECS {
            queue.active = None;
        }
    }
    if queue.active.is_none()
        && let Some(next) = queue.pending.pop_front()
    {
        queue.active = Some((next, 0.0));
    }
}
```

Register `tick_banner_queue` in the plugin, chained **after** the three detectors:

```rust
app.add_systems(
    Update,
    (
        detect_drop_banners,
        detect_boss_banner,
        detect_rescue_banner,
        tick_banner_queue,
    )
        .chain()
        .run_if(resource_exists::<ViewedMission>),
);
```

Watch the view-change test: on first `tick_banner_queue` run with `queue.mission == None`, the reset branch fires and returns — so the boss banner activates on the *second* tick. If the first test breaks because of this, run `tick_banner_queue` once before pushing requests in the test (or pre-set `queue.mission = Some(mission)`); prefer pre-setting `mission` in the fixtures since that mirrors steady-state.

**Step 3: Run, then full check + commit**

Run: `cargo test --lib ui::banner`, then `cargo test && cargo clippy --all-targets -- -D warnings`

```bash
git add src/ui/banner.rs
git commit -m "feat(ux3): banner queue lifecycle — activation, expiry, view-change reset"
```

---

### Task 5: Render the active banner in the mission view

**Files:**
- Modify: `src/screens/mission_view.rs`

This layer is intentionally thin (positions + colors); it gets hand-verified in Task 6 rather than unit-tested. No test-first here — visual code, per the design doc.

**Step 1: Implement**

Add a marker + render system in `mission_view.rs`. The banner UI must be a **separate root** from `MissionViewUi` (the feed rebuild at `mission_view.rs:690-696` despawns that wholesale every feed change).

```rust
/// Marker for the floating event-banner node.
#[derive(Component)]
struct BannerUi;

fn update_banner_ui(
    mut commands: Commands,
    queue: Res<crate::ui::banner::BannerQueue>,
    gameplay_root: Query<Entity, With<widgets::GameplayRoot>>,
    existing: Query<Entity, With<BannerUi>>,
) {
    use crate::ui::banner::{banner_alpha, BannerKind};

    // One node per frame — despawn and rebuild (matches the feed's approach).
    for entity in &existing {
        commands.entity(entity).despawn();
    }
    let Some((req, elapsed)) = queue.active.as_ref() else { return };
    let Ok(root_entity) = gameplay_root.single() else { return };

    let alpha = banner_alpha(*elapsed);
    let (accent, label_color) = match req.kind {
        BannerKind::Boss => (Color::srgba(0.75, 0.15, 0.1, alpha), Color::srgba(1.0, 0.85, 0.8, alpha)),
        BannerKind::RareDrop => (Color::srgba(0.8, 0.55, 0.05, alpha), Color::srgba(1.0, 0.95, 0.75, alpha)),
        BannerKind::RescueClosing => (Color::srgba(0.8, 0.45, 0.1, alpha), Color::srgba(1.0, 0.9, 0.75, alpha)),
    };
    // Slide down from -20px to final 60px over the slide phase.
    let slide = (elapsed / crate::ui::banner::BANNER_SLIDE_SECS).min(1.0);
    let top = -20.0 + 80.0 * slide;

    let mut banner = bevy_declarative::element::div::div()
        .absolute()
        .col()
        .items_center()
        .p(px(14.0))
        .bg(Color::srgba(0.08, 0.06, 0.05, 0.9 * alpha))
        .rounded(px(6.0))
        .insert((BannerUi, BorderColor::all(accent), Pickable::IGNORE, GlobalZIndex(50)));
    banner.style_mut().border = UiRect::all(Val::Px(2.0));
    banner.style_mut().top = Val::Px(top);
    banner.style_mut().left = Val::Percent(50.0);
    banner.style_mut().margin.left = Val::Px(-170.0); // center a ~340px banner
    banner.style_mut().width = Val::Px(340.0);

    banner = banner.child(
        bevy_declarative::element::text::text(req.text.clone())
            .font_size(26.0)
            .color(label_color)
            .insert(Pickable::IGNORE),
    );
    if let Some(ref subtitle) = req.subtitle {
        banner = banner.child(
            bevy_declarative::element::text::text(subtitle.clone())
                .font_size(15.0)
                .color(Color::srgba(0.85, 0.82, 0.78, alpha))
                .insert(Pickable::IGNORE),
        );
    }
    let id = banner.build(&mut commands);
    commands.entity(root_entity).add_child(id);
}
```

**Adapt to the local bevy_declarative API** — check how `spawn_mission_view_ui` (`mission_view.rs:570`) actually materializes elements (it may be `.build(...)`, `.spawn(...)`, or an insertion helper; copy its pattern exactly, including how children attach to `GameplayRoot`). Also match how `BorderColor`/`GlobalZIndex` are used elsewhere; if `GlobalZIndex` isn't used in this codebase, check what the toast module uses to stay on top and copy that.

Register in the mission-view plugin's `Update` chain (`mission_view.rs:34-54`), after `update_mission_feed_ui`:

```rust
update_mission_feed_ui,
update_banner_ui,
```

Also add `OnExit(GameTab::MissionView)` cleanup: extend `cleanup_mission_view` to despawn `BannerUi` entities (look at what it already despawns and add the marker query).

**Step 2: Compile + full suite**

Run: `cargo test && cargo clippy --all-targets -- -D warnings`
Expected: clean. (Rendering correctness is Task 6.)

**Step 3: Commit**

```bash
git add src/screens/mission_view.rs
git commit -m "feat(ux3): render active event banner in the mission view"
```

---

### Task 6: Hand verification + close the chunk

**Step 1: Verify in the running game**

Run: `cargo run`

- Dispatch a party to a difficulty-3+ mission, watch it: when the party reaches the boss, **BOSS ENCOUNTER** slides in top-center, holds ~2.5s, fades. It must not refire in the same mission.
- Legendary drops are rare; to verify **RARE DROP!** deterministically, temporarily force `GearRarity::Legendary` in loot resolution (find the rarity roll via `grep -n "Legendary" src/equipment.rs`), observe the banner, then **revert the hack before committing**.
- Wipe a party (send a weak level-1 solo hero to a hard mission), mount the rescue, watch the rescue mission until under 30 game-seconds remain (use game-speed controls to fast-forward): **RESCUE WINDOW CLOSING** appears once.
- Trigger two banners close together (legendary hack + boss room) and confirm they show sequentially, not overlapping.

**Step 2: Tick the chunk**

In `docs/steam-release-chunks.md`, change the UX-3 line to:

```markdown
- [x] **UX-3 · Event banners** *(✓ 2026-07-06)* — floating banners in the mission view ("BOSS ENCOUNTER", "RARE DROP!", "RESCUE WINDOW CLOSING"), driven by the UX-1 event stream.
  Touches: feed module, `mission_view.rs` · Needs: UX-1
  Done when: the three banner-worthy moments interrupt the eye reliably and clear themselves.
```

**Step 3: Final gate + commit**

Run: `cargo test && cargo clippy --all-targets -- -D warnings`
Expected: all green, no warnings.

```bash
git add docs/steam-release-chunks.md
git commit -m "docs: tick UX-3 — event banners shipped"
```

**Step 4: Finish the branch**

REQUIRED SUB-SKILL: superpowers:finishing-a-development-branch — merge `feat/ux3-event-banners` back to `main` (project convention: merge commits with a summary line, see `git log`).
