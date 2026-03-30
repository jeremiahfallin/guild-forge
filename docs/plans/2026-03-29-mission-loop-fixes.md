# Mission Loop Fixes Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Fix three game-breaking bugs (tick timing, OnMission cleanup, no rewards) and add a reusable toast notification system.

**Architecture:** Replace fragile float-based tick gating with an explicit `TickJustFired` event. Add mission cleanup on exit. New `src/ui/toast.rs` module provides event-driven toast notifications. Gold resource + XP/level-up logic in combat completion.

**Tech Stack:** Bevy 0.18, bevy_declarative, rand 0.9

---

### Task 1: TickJustFired Event

Replace the `timer.0 > TICK_INTERVAL * 0.1` float checks across 5 systems with a proper Bevy event.

**Files:**
- Modify: `src/mission/entities.rs:77-94` (add event, modify simulation_tick)
- Modify: `src/mission/ai.rs:27-50` (consume event instead of float check)
- Modify: `src/mission/combat.rs:33-47` (hero_combat_system)
- Modify: `src/mission/combat.rs:125-132` (enemy_combat_system)
- Modify: `src/mission/combat.rs:196-205` (update_room_status)
- Modify: `src/mission/combat.rs:234-243` (check_mission_completion)
- Modify: `src/mission/mod.rs:14-30` (register event)

**Step 1: Add the event to entities.rs**

In `src/mission/entities.rs`, add after line 94 (`pub const TICK_INTERVAL`):

```rust
/// Fired once per simulation tick so other systems know when to act.
#[derive(Event)]
pub struct TickJustFired;
```

**Step 2: Modify simulation_tick to send the event**

In `src/mission/entities.rs`, change `simulation_tick` signature and body. Add `mut tick_events: EventWriter<TickJustFired>` parameter. After `timer.0 -= TICK_INTERVAL;` (line 270), add `tick_events.write(TickJustFired);`.

```rust
pub fn simulation_tick(
    time: Res<Time>,
    speed: Res<SimulationSpeed>,
    mut timer: ResMut<SimulationTimer>,
    mut tick_events: EventWriter<TickJustFired>,
    dungeon: Option<Res<crate::screens::mission_view::ActiveDungeon>>,
    mut heroes: Query<
        (&mut GridPosition, &mut MoveTarget, &mut InRoom),
        (With<HeroToken>, Without<EnemyToken>),
    >,
) {
    let Some(dungeon) = dungeon else { return };
    let map = &dungeon.0;

    timer.0 += time.delta_secs() * speed.0;

    if timer.0 < TICK_INTERVAL {
        return;
    }
    timer.0 -= TICK_INTERVAL;
    tick_events.write(TickJustFired);

    // Move heroes along their paths
    for (mut grid_pos, mut target, mut in_room) in &mut heroes {
        if target.path_index >= target.path.len() {
            continue;
        }

        let (nx, ny) = target.path[target.path_index];
        grid_pos.x = nx;
        grid_pos.y = ny;
        in_room.0 = map.room_at(nx, ny);
        target.path_index += 1;
    }
}
```

**Step 3: Update all consumers**

For each of these 5 systems, replace the timer-based early return with an event reader check:

Replace:
```rust
    timer: Res<SimulationTimer>,
    // ...
    if timer.0 > TICK_INTERVAL * 0.1 {
        return;
    }
```

With:
```rust
    mut tick_events: EventReader<TickJustFired>,
    // ...
    if tick_events.read().next().is_none() {
        return;
    }
```

Systems to update:
- `hero_ai_system` in `src/mission/ai.rs:27-50` — replace `timer: Res<SimulationTimer>` param and float check
- `hero_combat_system` in `src/mission/combat.rs:33-47` — replace `timer: Res<SimulationTimer>` param and float check
- `enemy_combat_system` in `src/mission/combat.rs:125-132` — replace `timer: Res<SimulationTimer>` param and float check
- `update_room_status` in `src/mission/combat.rs:196-205` — replace `timer: Res<SimulationTimer>` param and float check
- `check_mission_completion` in `src/mission/combat.rs:234-243` — replace `timer: Res<SimulationTimer>` param and float check

**Step 4: Register the event in mission plugin**

In `src/mission/mod.rs`, add `app.add_event::<entities::TickJustFired>();` inside the `plugin` function, before the system registrations.

**Step 5: Build and run tests**

Run: `cargo build && cargo test --bin guild-forge`
Expected: Compiles with no errors, 5 tests pass.

**Step 6: Commit**

```bash
git add src/mission/
git commit -m "Replace float-based tick gating with TickJustFired event"
```

---

### Task 2: OnMission Cleanup

Clean up mission state when leaving the mission view, regardless of outcome.

**Files:**
- Modify: `src/mission/entities.rs:309-319` (expand cleanup_mission_entities)
- Modify: `src/mission/combat.rs:234-269` (remove tab transition from check_mission_completion)

**Step 1: Expand cleanup_mission_entities**

In `src/mission/entities.rs`, update `cleanup_mission_entities` to also:
- Remove `OnMission` from all heroes referenced by `MissionParty`
- Despawn mission entities (the `Mission` marker entities)
- Remove `ActiveDungeon` resource

```rust
pub fn cleanup_mission_entities(
    mut commands: Commands,
    entities: Query<Entity, With<MissionEntity>>,
    missions: Query<(Entity, &MissionParty), With<Mission>>,
) {
    // Despawn all mission-scoped sprites/tokens
    for entity in &entities {
        commands.entity(entity).despawn();
    }

    // Remove OnMission from party heroes and despawn mission entity
    for (mission_entity, party) in &missions {
        for &hero_entity in &party.0 {
            commands.entity(hero_entity).remove::<super::OnMission>();
        }
        commands.entity(mission_entity).despawn();
    }

    // Clean up resources
    commands.remove_resource::<RoomStatus>();
    commands.remove_resource::<SimulationSpeed>();
    commands.remove_resource::<SimulationTimer>();
    commands.remove_resource::<crate::screens::mission_view::ActiveDungeon>();
}
```

**Step 2: Keep check_mission_completion but remove immediate tab transition**

The completion system should set `MissionProgress` but NOT transition tabs. Instead, it should only set the progress state. The mission_view "Retreat" button and a new auto-return-on-complete mechanism will handle the transition. For now, keep the tab transition but it will work correctly since cleanup runs on exit.

Actually, the tab transition is fine as-is — `OnExit(GameTab::MissionView)` fires when the state changes, so cleanup will run. No change needed to `check_mission_completion` beyond the tick event fix from Task 1.

**Step 3: Build and run tests**

Run: `cargo build && cargo test --bin guild-forge`
Expected: Compiles, 5 tests pass.

**Step 4: Commit**

```bash
git add src/mission/entities.rs
git commit -m "Clean up OnMission, mission entities, and resources on mission exit"
```

---

### Task 3: Toast Notification Module

Create a reusable event-driven toast system.

**Files:**
- Create: `src/ui/mod.rs`
- Create: `src/ui/toast.rs`
- Modify: `src/main.rs:6-14` (add ui module)
- Modify: `src/main.rs:48-58` (add ui plugin)

**Step 1: Create src/ui/mod.rs**

```rust
//! Game UI systems: toast notifications, overlays.

pub mod toast;

use bevy::prelude::*;

pub(super) fn plugin(app: &mut App) {
    app.add_plugins(toast::plugin);
}
```

**Step 2: Create src/ui/toast.rs**

```rust
//! Event-driven toast notification system.

use bevy::prelude::*;
use bevy_declarative::element::div::div;
use bevy_declarative::element::text::text;
use bevy_declarative::style::styled::Styled;
use bevy_declarative::style::values::px;

use crate::theme::palette::*;

pub(super) fn plugin(app: &mut App) {
    app.add_event::<ToastEvent>();
    app.add_systems(Update, (spawn_toasts, tick_toasts));
}

/// Send this event from any system to display a toast.
#[derive(Event, Debug, Clone)]
pub struct ToastEvent {
    pub title: String,
    pub body: String,
    pub kind: ToastKind,
}

/// Visual style of the toast.
#[derive(Debug, Clone, Copy, Default)]
pub enum ToastKind {
    Success,
    Failure,
    #[default]
    Info,
}

/// Timer component on toast UI nodes — auto-despawns when expired.
#[derive(Component)]
struct ToastTimer(Timer);

/// Marker for the toast container (anchored bottom-right).
#[derive(Component)]
struct ToastContainer;

/// Marker for individual toast nodes.
#[derive(Component)]
struct ToastNode;

fn spawn_toasts(
    mut commands: Commands,
    mut events: EventReader<ToastEvent>,
    container_q: Query<Entity, With<ToastContainer>>,
) {
    for event in events.read() {
        // Ensure container exists
        let container = if let Ok(entity) = container_q.single() {
            entity
        } else {
            commands
                .spawn((
                    Name::new("Toast Container"),
                    ToastContainer,
                    Node {
                        position_type: PositionType::Absolute,
                        right: Val::Px(20.0),
                        bottom: Val::Px(20.0),
                        flex_direction: FlexDirection::ColumnReverse,
                        row_gap: Val::Px(8.0),
                        ..default()
                    },
                    GlobalZIndex(100),
                    Pickable::IGNORE,
                ))
                .id()
        };

        let border_color = match event.kind {
            ToastKind::Success => Color::srgb(0.2, 0.7, 0.3),
            ToastKind::Failure => Color::srgb(0.8, 0.2, 0.2),
            ToastKind::Info => Color::srgb(0.3, 0.5, 0.8),
        };

        let toast = div()
            .col()
            .w(px(320.0))
            .p(px(12.0))
            .gap(px(4.0))
            .bg(Color::srgba(0.1, 0.1, 0.15, 0.9))
            .rounded(px(8.0))
            .insert((
                Name::new("Toast"),
                ToastNode,
                ToastTimer(Timer::from_seconds(5.0, TimerMode::Once)),
                BorderColor(border_color),
                bevy::ui::Border::all(Val::Px(2.0)),
                Pickable::IGNORE,
            ))
            .child(
                text(&event.title)
                    .font_size(20.0)
                    .color(HEADER_TEXT),
            )
            .child(
                text(&event.body)
                    .font_size(15.0)
                    .color(LABEL_TEXT),
            );

        let toast_entity = toast.spawn(&mut commands);
        commands.entity(container).add_child(toast_entity);
    }
}

fn tick_toasts(
    mut commands: Commands,
    time: Res<Time>,
    mut toasts: Query<(Entity, &mut ToastTimer), With<ToastNode>>,
    container_q: Query<(Entity, &Children), With<ToastContainer>>,
) {
    for (entity, mut timer) in &mut toasts {
        timer.0.tick(time.delta());
        if timer.0.finished() {
            commands.entity(entity).despawn();
        }
    }

    // Remove container if empty
    if let Ok((container, children)) = container_q.single() {
        if children.is_empty() {
            commands.entity(container).despawn();
        }
    }
}
```

**Step 3: Wire into main.rs**

In `src/main.rs`, add `mod ui;` after `mod mission;` (around line 11). In the plugin list (line 48-58), add `ui::plugin,` after `theme::plugin,`.

**Step 4: Build and run tests**

Run: `cargo build && cargo test --bin guild-forge`
Expected: Compiles, 5 tests pass.

**Step 5: Commit**

```bash
git add src/ui/ src/main.rs
git commit -m "Add event-driven toast notification system"
```

---

### Task 4: Gold Resource & Rewards

Add gold tracking, XP rewards, level-up, and fire toast events on mission completion.

**Files:**
- Create: `src/economy.rs`
- Modify: `src/main.rs` (add economy module)
- Modify: `src/mission/combat.rs:234-269` (check_mission_completion — fire toast, award rewards)
- Modify: `src/screens/hub.rs` (display gold)
- Modify: `src/hero/mod.rs` (add pub level_up helper)

**Step 1: Create src/economy.rs**

```rust
//! Gold and economy tracking.

use bevy::prelude::*;
use crate::screens::Screen;

pub(super) fn plugin(app: &mut App) {
    app.add_systems(OnEnter(Screen::Gameplay), init_gold);
}

/// The guild's gold reserves.
#[derive(Resource, Debug, Default)]
pub struct Gold(pub u32);

fn init_gold(mut commands: Commands, existing: Option<Res<Gold>>) {
    if existing.is_none() {
        commands.insert_resource(Gold(0));
    }
}
```

**Step 2: Add level_up helper to hero/mod.rs**

In `src/hero/mod.rs`, add a public helper function after `spawn_random_hero`:

```rust
/// Award XP to a hero and handle level-ups. Returns true if leveled up.
pub fn award_xp(info: &mut HeroInfo, xp: u32) -> bool {
    info.xp += xp;
    if info.xp >= info.xp_to_next {
        info.xp -= info.xp_to_next;
        info.level += 1;
        info.xp_to_next = (info.xp_to_next as f32 * 1.5) as u32;
        true
    } else {
        false
    }
}
```

**Step 3: Update check_mission_completion to award rewards and fire toasts**

In `src/mission/combat.rs`, heavily modify `check_mission_completion`. It needs new params:
- `EventWriter<ToastEvent>`
- `Res<MissionTemplateDatabase>` and `Res<EnemyDatabase>` for reward data
- `ResMut<Gold>`
- Query for `&mut HeroInfo` on heroes (via `HeroToken` → entity lookup)
- Query for dead enemies (to sum xp_reward)

```rust
pub fn check_mission_completion(
    mut tick_events: EventReader<TickJustFired>,
    room_status: Option<Res<RoomStatus>>,
    hero_tokens: Query<(&HeroToken, &CombatStats), Without<EnemyToken>>,
    mut missions: Query<(&mut MissionProgress, &super::MissionInfo, &MissionParty), With<Mission>>,
    mut next_tab: ResMut<NextState<crate::screens::GameTab>>,
    mut toast_events: EventWriter<crate::ui::toast::ToastEvent>,
    mut gold: Option<ResMut<crate::economy::Gold>>,
    mut hero_info: Query<&mut crate::hero::HeroInfo, With<crate::hero::Hero>>,
    templates: Option<Res<super::data::MissionTemplateDatabase>>,
) {
    if tick_events.read().next().is_none() {
        return;
    }

    let Some(room_status) = room_status else { return };

    // Count living/dead heroes
    let total_heroes = hero_tokens.iter().count();
    let dead_heroes = hero_tokens.iter().filter(|(_, c)| c.hp <= 0).count();
    let all_dead = total_heroes > 0 && dead_heroes == total_heroes;

    let all_cleared = !room_status.cleared.is_empty()
        && room_status.cleared.iter().all(|&c| c);

    if !all_dead && !all_cleared {
        return; // Mission still in progress
    }

    for (mut progress, mission_info, party) in &mut missions {
        if all_dead {
            *progress = MissionProgress::Failed;

            toast_events.write(crate::ui::toast::ToastEvent {
                title: format!("{} — Failed!", mission_info.name),
                body: "Party wiped — no rewards".to_string(),
                kind: crate::ui::toast::ToastKind::Failure,
            });
        } else {
            *progress = MissionProgress::Complete;

            // Roll gold reward
            let mut rng = rand::rng();
            let gold_amount = templates
                .as_ref()
                .and_then(|t| t.0.iter().find(|t| t.id == mission_info.template_id))
                .map(|t| rng.random_range(t.gold_reward.min..=t.gold_reward.max))
                .unwrap_or(0);

            if let Some(ref mut gold) = gold {
                gold.0 += gold_amount;
            }

            // Calculate XP: template bonus (per surviving hero)
            let xp_per_hero = templates
                .as_ref()
                .and_then(|t| t.0.iter().find(|t| t.id == mission_info.template_id))
                .map(|t| t.xp_bonus)
                .unwrap_or(0);

            // Award XP to surviving heroes
            let casualties = dead_heroes;
            let surviving = total_heroes - casualties;
            for &hero_entity in &party.0 {
                // Check if this hero's token is alive
                let is_alive = hero_tokens
                    .iter()
                    .any(|(ht, cs)| ht.0 == hero_entity && cs.hp > 0);
                if is_alive {
                    if let Ok(mut info) = hero_info.get_mut(hero_entity) {
                        crate::hero::award_xp(&mut info, xp_per_hero);
                    }
                }
            }

            let casualty_text = match casualties {
                0 => "no casualties".to_string(),
                1 => "1 casualty".to_string(),
                n => format!("{n} casualties"),
            };

            toast_events.write(crate::ui::toast::ToastEvent {
                title: format!("{} — Complete!", mission_info.name),
                body: format!("+{gold_amount}g, +{xp_per_hero}xp — {casualty_text}"),
                kind: crate::ui::toast::ToastKind::Success,
            });
        }
    }

    next_tab.set(crate::screens::GameTab::Hub);
}
```

**Step 4: Display gold on hub**

In `src/screens/hub.rs`, add a gold display. Modify `spawn_hub` to take `gold: Option<Res<crate::economy::Gold>>` and display it below the subtitle:

```rust
fn spawn_hub(mut commands: Commands, gold: Option<Res<crate::economy::Gold>>) {
    let gold_text = gold.map(|g| format!("Gold: {}", g.0)).unwrap_or_default();

    widgets::ui_root("Guild Hub")
        .insert(DespawnOnExit(GameTab::Hub))
        .child(widgets::header("Guild Forge"))
        .child(
            widgets::label("Manage your guild of adventurers")
                .color(LABEL_TEXT),
        )
        .child(
            widgets::label(gold_text)
                .color(Color::srgb(0.9, 0.8, 0.2)),
        )
        // ... rest unchanged
```

**Step 5: Wire economy module into main.rs**

Add `mod economy;` and `economy::plugin,` in the appropriate places in `src/main.rs`.

**Step 6: Build and run tests**

Run: `cargo build && cargo test --bin guild-forge`
Expected: Compiles, 5 tests pass.

**Step 7: Commit**

```bash
git add src/economy.rs src/main.rs src/mission/combat.rs src/screens/hub.rs src/hero/mod.rs
git commit -m "Add gold, XP rewards, level-up, and toast notifications on mission completion"
```

---

### Task 5: Final Verification & Push

**Step 1: Run full test suite**

Run: `cargo test --bin guild-forge`
Expected: 5 tests pass.

**Step 2: Run clippy**

Run: `cargo clippy`
Expected: No errors.

**Step 3: Runtime smoke test**

Run: `cargo run`
Navigate: Hub → Missions → Select mission → Party Select → Dispatch → Watch dungeon → Mission completes/fails → Verify toast appears → Verify heroes available in roster → Verify gold updated on hub.

**Step 4: Push to GitHub**

```bash
git push -u origin main
```
