# Sidebar Navigation Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Replace hub-based menu navigation with a persistent left sidebar visible on all gameplay screens, supporting active mission monitoring and background mission execution.

**Architecture:** A gameplay-level root container (row layout) holds a fixed-width sidebar on the left and a flex content area on the right. The sidebar is spawned once on `OnEnter(Screen::Gameplay)` and persists across all `GameTab` transitions. Reactive systems update gold, active missions, and tab highlights. Each screen switches from `ui_root()` to a `content_area()` widget that fills the right portion. Mission simulation systems are decoupled from `GameTab::MissionView` so they run in the background.

**Tech Stack:** Bevy 0.18, bevy_declarative (local), Rust

**Design doc:** `docs/plans/2026-03-31-sidebar-navigation-design.md`

---

### Task 1: Add `overflow_y_scroll()` to bevy_declarative

**Files:**
- Modify: `C:\Users\bullf\dev\games\bevy_declarative\src\style\styled.rs:446-449`

**Step 1: Add the method**

After the existing `overflow_y_hidden` method (line 446), add:

```rust
    fn overflow_y_scroll(mut self) -> Self {
        self.style_mut().overflow.y = OverflowAxis::Scroll;
        self
    }
```

**Step 2: Build to verify**

Run: `cd C:\Users\bullf\dev\games\bevy_declarative && cargo build`
Expected: PASS

**Step 3: Commit**

```bash
cd C:\Users\bullf\dev\games\bevy_declarative
git add src/style/styled.rs
git commit -m "feat: add overflow_y_scroll() method to Styled trait"
```

---

### Task 2: Add `content_area` widget and `SidebarRoot` marker

**Files:**
- Modify: `C:\Users\bullf\dev\games\guild-forge\src\theme\widgets.rs`

**Step 1: Add content_area widget and marker components**

Add to `src/theme/widgets.rs`:

```rust
/// Marker for the gameplay root container (sidebar + content area).
#[derive(Component)]
pub struct GameplayRoot;

/// Marker for the sidebar UI so reactive systems can find it.
#[derive(Component)]
pub struct SidebarRoot;

/// Marker for the gold text display in the sidebar.
#[derive(Component)]
pub struct SidebarGoldText;

/// Marker for the active missions container in the sidebar.
#[derive(Component)]
pub struct SidebarMissionList;

/// Marker for a nav button, storing which tab it navigates to.
#[derive(Component)]
pub struct SidebarNavButton(pub crate::screens::GameTab);

/// A content area that fills the right side of the gameplay layout.
/// Screens use this instead of `ui_root()` when the sidebar is present.
pub fn content_area(name: impl Into<Cow<'static, str>>) -> Div {
    div()
        .col()
        .flex_1()
        .h_full()
        .items_center()
        .gap(px(20.0))
        .overflow_y_hidden()
        .insert((Name::new(name), Pickable::IGNORE))
}
```

**Step 2: Build**

Run: `cargo build`
Expected: PASS

**Step 3: Commit**

```bash
git add src/theme/widgets.rs
git commit -m "feat: add content_area widget and sidebar marker components"
```

---

### Task 3: Remove Hub screen, change default GameTab to Roster

**Files:**
- Delete: `src/screens/hub.rs`
- Modify: `src/screens/mod.rs` — remove `hub` module, remove `Hub` variant from `GameTab`, change default to `Roster`
- Modify: `src/mission/combat.rs:268,342` — change `GameTab::Hub` references to `GameTab::Roster`

**Step 1: Update `src/screens/mod.rs`**

Remove `mod hub;` and `hub::plugin,`. Remove `Hub` variant from `GameTab`. Change `#[default]` to `Roster`:

```rust
#[derive(SubStates, Copy, Clone, Eq, PartialEq, Hash, Debug, Default)]
#[source(Screen = Screen::Gameplay)]
pub enum GameTab {
    #[default]
    Roster,
    Missions,
    PartySelect,
    MissionView,
}
```

**Step 2: Delete `src/screens/hub.rs`**

**Step 3: Fix references to `GameTab::Hub` in `src/mission/combat.rs`**

Line 268 — change:
```rust
next_tab.set(crate::screens::GameTab::Hub);
```
to:
```rust
next_tab.set(crate::screens::GameTab::Roster);
```

Line 342 — same change.

**Step 4: Search for any other `GameTab::Hub` references and fix them**

Run: `grep -rn "GameTab::Hub\|GameTab::Hub" src/ --include="*.rs"`

Fix any remaining references to use `GameTab::Roster` or remove them.

**Step 5: Build**

Run: `cargo build`
Expected: PASS (may need to fix additional references found in step 4)

**Step 6: Commit**

```bash
git add -A
git commit -m "feat: remove Hub screen, default GameTab to Roster"
```

---

### Task 4: Create the sidebar module and spawn function

**Files:**
- Create: `src/screens/sidebar.rs`
- Modify: `src/screens/mod.rs` — add `mod sidebar;` and `sidebar::plugin`

**Step 1: Create `src/screens/sidebar.rs`**

```rust
//! Persistent sidebar — spawned once on gameplay enter, lives across all GameTab transitions.

use bevy::prelude::*;
use bevy_declarative::element::div::div;
use bevy_declarative::element::text::text;
use bevy_declarative::style::styled::Styled;
use bevy_declarative::style::values::px;

use crate::{
    economy::Gold,
    mission::{Mission, MissionInfo, MissionProgress, OnMission},
    screens::GameTab,
    theme::{
        palette::*,
        widgets::{self, GameplayRoot, SidebarGoldText, SidebarMissionList, SidebarNavButton, SidebarRoot},
    },
};

pub(super) fn plugin(app: &mut App) {
    app.add_systems(
        OnEnter(crate::screens::Screen::Gameplay),
        spawn_gameplay_root,
    );
    app.add_systems(
        Update,
        (
            update_gold_display.run_if(resource_changed::<Gold>),
            update_active_tab_highlight.run_if(state_changed::<GameTab>),
            update_mission_list,
        )
            .run_if(in_state(crate::screens::Screen::Gameplay)),
    );
}

/// The sidebar width in pixels.
const SIDEBAR_WIDTH: f32 = 220.0;

fn spawn_gameplay_root(mut commands: Commands, gold: Option<Res<Gold>>) {
    let gold_amount = gold.map_or(0, |g| g.0);

    // Gameplay root: row containing sidebar + content area
    let mut root = div()
        .absolute()
        .w_full()
        .h_full()
        .row()
        .insert((
            Name::new("Gameplay Root"),
            GameplayRoot,
            Pickable::IGNORE,
            DespawnOnExit(crate::screens::Screen::Gameplay),
        ));

    // Build sidebar
    let sidebar = build_sidebar(gold_amount);
    root = root.child(sidebar);

    root.spawn(&mut commands);
}

fn build_sidebar(gold_amount: u32) -> bevy_declarative::element::div::Div {
    let mut sidebar = div()
        .col()
        .w(px(SIDEBAR_WIDTH))
        .h_full()
        .bg(Color::srgba(0.08, 0.08, 0.12, 0.95))
        .insert((Name::new("Sidebar"), SidebarRoot));

    // ── Pinned section ──────────────────────────────────────────
    let pinned = div()
        .col()
        .w_full()
        .gap(px(8.0))
        .p(px(12.0))
        // Title
        .child(
            text("Guild Forge")
                .font_size(24.0)
                .color(HEADER_TEXT),
        )
        // Gold
        .child(
            text(format!("Gold: {gold_amount}"))
                .font_size(18.0)
                .color(Color::srgb(0.9, 0.8, 0.2))
                .insert(SidebarGoldText),
        )
        // Divider
        .child(
            div()
                .w_full()
                .h(px(1.0))
                .bg(Color::srgba(0.4, 0.4, 0.5, 0.5)),
        )
        // Nav buttons
        .child(nav_button("Roster", GameTab::Roster))
        .child(nav_button("Missions", GameTab::Missions))
        .child(disabled_nav_button("Armory"))
        // Divider
        .child(
            div()
                .w_full()
                .h(px(1.0))
                .bg(Color::srgba(0.4, 0.4, 0.5, 0.5)),
        )
        // Active Missions header
        .child(
            text("Active Missions")
                .font_size(16.0)
                .color(LABEL_TEXT),
        );

    // ── Scrollable mission list ─────────────────────────────────
    let mission_list = div()
        .col()
        .w_full()
        .flex_1()
        .gap(px(4.0))
        .p(px(12.0))
        .overflow_y_scroll()
        .insert((Name::new("Mission List"), SidebarMissionList));

    sidebar = sidebar.child(pinned).child(mission_list);
    sidebar
}

fn nav_button(label: &str, tab: GameTab) -> bevy_declarative::element::div::Div {
    use crate::theme::interaction::InteractionPalette;

    div()
        .w_full()
        .h(px(40.0))
        .items_center()
        .justify_center()
        .bg(BUTTON_BACKGROUND)
        .rounded(px(4.0))
        .insert((
            Name::new(format!("Nav: {label}")),
            Button,
            SidebarNavButton(tab),
            InteractionPalette {
                none: BUTTON_BACKGROUND,
                hovered: BUTTON_HOVERED_BACKGROUND,
                pressed: BUTTON_PRESSED_BACKGROUND,
            },
        ))
        .on_click(nav_click)
        .child(
            text(label)
                .font_size(18.0)
                .color(BUTTON_TEXT)
                .insert(Pickable::IGNORE),
        )
}

fn disabled_nav_button(label: &str) -> bevy_declarative::element::div::Div {
    div()
        .w_full()
        .h(px(40.0))
        .items_center()
        .justify_center()
        .bg(Color::srgba(0.2, 0.2, 0.2, 0.5))
        .rounded(px(4.0))
        .insert(Name::new(format!("Nav: {label} (disabled)")))
        .child(
            text(format!("{label} (Soon)"))
                .font_size(16.0)
                .color(Color::srgba(0.5, 0.5, 0.5, 0.8)),
        )
}

fn nav_click(
    click: On<Pointer<Click>>,
    buttons: Query<&SidebarNavButton>,
    mut next_tab: ResMut<NextState<GameTab>>,
) {
    if let Ok(nav) = buttons.get(click.event_target()) {
        next_tab.set(nav.0);
    }
}

// ── Reactive update systems ────────────────────────────────────────

fn update_gold_display(
    gold: Res<Gold>,
    mut texts: Query<&mut Text, With<SidebarGoldText>>,
) {
    for mut t in &mut texts {
        **t = format!("Gold: {}", gold.0);
    }
}

fn update_active_tab_highlight(
    tab: Res<State<GameTab>>,
    mut buttons: Query<(&SidebarNavButton, &mut BackgroundColor)>,
) {
    for (nav, mut bg) in &mut buttons {
        if nav.0 == **tab {
            *bg = BackgroundColor(BUTTON_HOVERED_BACKGROUND);
        } else {
            *bg = BackgroundColor(BUTTON_BACKGROUND);
        }
    }
}

fn update_mission_list(
    mut commands: Commands,
    list_q: Query<Entity, With<SidebarMissionList>>,
    missions: Query<(Entity, &MissionInfo, &MissionProgress), With<Mission>>,
    children_q: Query<&Children>,
) {
    let Ok(list_entity) = list_q.single() else {
        return;
    };

    // Despawn existing children
    if let Ok(children) = children_q.get(list_entity) {
        for &child in children.iter() {
            commands.entity(child).despawn();
        }
    }

    // Rebuild mission entries
    for (mission_entity, info, progress) in &missions {
        let status_text = match progress {
            MissionProgress::InProgress => "In Progress",
            MissionProgress::Complete => "Complete",
            MissionProgress::Failed => "Failed",
        };

        let bg_color = match progress {
            MissionProgress::InProgress => Color::srgba(0.2, 0.25, 0.35, 0.8),
            MissionProgress::Complete => Color::srgba(0.15, 0.35, 0.15, 0.8),
            MissionProgress::Failed => Color::srgba(0.35, 0.15, 0.15, 0.8),
        };

        let entry = div()
            .col()
            .w_full()
            .p(px(8.0))
            .gap(px(2.0))
            .bg(bg_color)
            .rounded(px(4.0))
            .insert(WatchMissionButton(mission_entity))
            .on_click(watch_mission)
            .child(
                text(&info.name)
                    .font_size(14.0)
                    .color(HEADER_TEXT),
            )
            .child(
                text(status_text)
                    .font_size(12.0)
                    .color(LABEL_TEXT),
            );

        entry.spawn_as_child_of(&mut commands, list_entity);
    }
}

/// Component on mission entries in the sidebar.
#[derive(Component)]
struct WatchMissionButton(Entity);

fn watch_mission(
    click: On<Pointer<Click>>,
    buttons: Query<&WatchMissionButton>,
    mut next_tab: ResMut<NextState<GameTab>>,
) {
    if let Ok(_button) = buttons.get(click.event_target()) {
        next_tab.set(GameTab::MissionView);
    }
}
```

**Step 2: Register in `src/screens/mod.rs`**

Add `mod sidebar;` and `sidebar::plugin,` to the plugin list.

**Step 3: Build**

Run: `cargo build`
Expected: PASS

**Step 4: Commit**

```bash
git add src/screens/sidebar.rs src/screens/mod.rs
git commit -m "feat: add persistent sidebar with gold, nav, and active missions"
```

---

### Task 5: Check bevy_declarative has `spawn_as_child_of`

**Files:**
- Possibly modify: `C:\Users\bullf\dev\games\bevy_declarative\src\element\div.rs`

**Step 1: Search for existing spawn methods**

Run: `grep -rn "fn spawn" C:\Users\bullf\dev\games\bevy_declarative\src\ --include="*.rs"`

Check if `spawn_as_child_of` (or similar like `spawn_child`, `build_child`) exists. If not, we need to add it or use an alternative approach (spawn then `add_child`).

**Step 2: If missing, add `spawn_as_child_of` to Div**

If the method doesn't exist, add to the `Div` impl block:

```rust
/// Spawn this element as a child of an existing entity.
pub fn spawn_as_child_of(self, commands: &mut Commands, parent: Entity) -> Entity {
    let child = self.spawn(commands);
    commands.entity(parent).add_child(child);
    child
}
```

Add the same method to `TextEl` if needed.

**Step 3: Build bevy_declarative**

Run: `cd C:\Users\bullf\dev\games\bevy_declarative && cargo build`
Expected: PASS

**Step 4: Commit if changed**

```bash
cd C:\Users\bullf\dev\games\bevy_declarative
git add -A
git commit -m "feat: add spawn_as_child_of method for dynamic child insertion"
```

---

### Task 6: Migrate Roster screen to content_area

**Files:**
- Modify: `src/screens/roster.rs`

**Step 1: Replace `ui_root` with `content_area`, remove Back button**

In `spawn_roster` (line 46) and `refresh_roster_on_selection_change` (line 322), change:

```rust
widgets::ui_root("Roster Screen").insert((DespawnOnExit(GameTab::Roster), RosterUi))
```

to:

```rust
widgets::content_area("Roster Screen").insert((DespawnOnExit(GameTab::Roster), RosterUi))
```

Remove the `top_bar` with the Back button and header. Replace with a smaller contextual header if desired:

```rust
let top_bar = div()
    .row()
    .w_full()
    .items_center()
    .p(px(16.0))
    .child(widgets::header("Roster"));
```

Apply the same changes in `refresh_roster_on_selection_change`.

**Step 2: Build and run**

Run: `cargo build && cargo run`
Expected: Roster displays to the right of sidebar. No Back button.

**Step 3: Commit**

```bash
git add src/screens/roster.rs
git commit -m "feat: migrate Roster screen to content_area layout"
```

---

### Task 7: Migrate Missions screen to content_area

**Files:**
- Modify: `src/screens/missions.rs`

**Step 1: Replace `ui_root` with `content_area`, remove Back button**

In `spawn_mission_board` (line 33), change `widgets::ui_root` to `widgets::content_area`.

Remove the Back button from `top_bar`. Keep the "Mission Board" header:

```rust
let top_bar = div()
    .row()
    .w_full()
    .items_center()
    .p(px(16.0))
    .child(widgets::header("Mission Board"));
```

Remove the `go_back` function (no longer needed).

**Step 2: Build**

Run: `cargo build`
Expected: PASS

**Step 3: Commit**

```bash
git add src/screens/missions.rs
git commit -m "feat: migrate Missions screen to content_area layout"
```

---

### Task 8: Migrate PartySelect screen to content_area

**Files:**
- Modify: `src/screens/party_select.rs`

**Step 1: Replace `ui_root` with `content_area`, change Cancel to navigate to Missions**

In `spawn_party_select` (line 74) and `refresh_party_select` (line 241), change `widgets::ui_root` to `widgets::content_area`.

Keep the Cancel button but change it to navigate to `GameTab::Missions` (same as current `go_back_to_missions`).

**Step 2: Build**

Run: `cargo build`
Expected: PASS

**Step 3: Commit**

```bash
git add src/screens/party_select.rs
git commit -m "feat: migrate PartySelect screen to content_area layout"
```

---

### Task 9: Decouple mission simulation from MissionView

Currently, all simulation systems in `src/mission/mod.rs` have `.run_if(in_state(GameTab::MissionView))`. Missions must continue running when the player navigates away.

**Files:**
- Modify: `src/mission/mod.rs:19-38`

**Step 1: Change run condition from `GameTab::MissionView` to `Screen::Gameplay`**

Replace:

```rust
    app.add_systems(
        Update,
        (
            entities::simulation_tick,
            ai::hero_ai_system,
            combat::hero_combat_system,
            combat::enemy_combat_system,
            combat::handle_death_system,
            combat::update_room_status,
            combat::check_mission_completion,
            entities::sync_sprite_positions,
        )
            .chain()
            .run_if(in_state(GameTab::MissionView)),
    );
```

with:

```rust
    app.add_systems(
        Update,
        (
            entities::simulation_tick,
            ai::hero_ai_system,
            combat::hero_combat_system,
            combat::enemy_combat_system,
            combat::handle_death_system,
            combat::update_room_status,
            combat::check_mission_completion,
        )
            .chain()
            .run_if(in_state(crate::screens::Screen::Gameplay)),
    );
    // Visual sync only runs when viewing a mission
    app.add_systems(
        Update,
        entities::sync_sprite_positions
            .run_if(in_state(GameTab::MissionView)),
    );
```

**Step 2: Fix `check_mission_completion` — remove auto-navigate to Hub**

In `src/mission/combat.rs`, the `check_mission_completion` function sets `next_tab` on completion/failure (lines 268, 342). Remove these — completed missions should stay in the sidebar list, not force navigation. Remove `next_tab: ResMut<NextState<GameTab>>` from the function signature.

**Step 3: Fix cleanup — don't cleanup on MissionView exit**

In `src/mission/mod.rs:17`, remove:
```rust
app.add_systems(OnExit(GameTab::MissionView), entities::cleanup_mission_entities);
```

Mission cleanup should only happen when a mission completes/fails or is aborted, not when navigating away. Add cleanup to `check_mission_completion` instead — after awarding rewards, call cleanup for that specific mission's entities.

**Step 4: Build**

Run: `cargo build`
Expected: PASS

**Step 5: Run and test**

Run: `cargo run`
Test: Dispatch a mission, navigate to Roster via sidebar, verify no crash and mission simulation continues.

**Step 6: Commit**

```bash
git add src/mission/mod.rs src/mission/combat.rs
git commit -m "feat: decouple mission simulation from MissionView, run in background"
```

---

### Task 10: Migrate MissionView — content_area + Abort button

**Files:**
- Modify: `src/screens/mission_view.rs`

**Step 1: Change MissionView UI**

In `spawn_mission_view`, replace the UI overlay section. Remove `widgets::ui_root("Mission View UI")` and the Retreat button. Replace with:

```rust
// Spawn UI overlay — just the abort button
widgets::content_area("Mission View UI")
    .insert((MissionViewUi, GlobalZIndex(10)))
    .child(
        div()
            .absolute()
            .insert(Node {
                bottom: bevy::ui::Val::Px(20.0),
                ..default()
            })
            .child(widgets::game_button("Abort Mission", abort_mission)),
    )
    .spawn(&mut commands);
```

**Step 2: Replace `go_back` with `abort_mission`**

```rust
fn abort_mission(
    _: On<Pointer<Click>>,
    mut commands: Commands,
    entities: Query<Entity, With<MissionEntity>>,
    missions: Query<(Entity, &MissionParty), With<Mission>>,
    mut next_tab: ResMut<NextState<GameTab>>,
) {
    // Cleanup mission entities
    for entity in &entities {
        commands.entity(entity).despawn();
    }
    for (mission_entity, party) in &missions {
        for &hero_entity in &party.0 {
            commands.entity(hero_entity).remove::<crate::mission::OnMission>();
        }
        commands.entity(mission_entity).despawn();
    }
    commands.remove_resource::<RoomStatus>();
    commands.remove_resource::<SimulationSpeed>();
    commands.remove_resource::<SimulationTimer>();
    commands.remove_resource::<ActiveDungeon>();

    next_tab.set(GameTab::Missions);
}
```

Add required imports for `MissionEntity`, `MissionParty`, `Mission`, `OnMission`, `RoomStatus`, `SimulationSpeed`, `SimulationTimer`.

**Step 3: Update cleanup_mission_view — only clean visuals, not mission data**

The `cleanup_mission_view` function (OnExit(MissionView)) should only clean up the dungeon rendering and camera, NOT mission entities. The mission keeps running. Remove entity despawning from it — keep only camera reset and DungeonRoot/MissionViewUi despawn.

**Step 4: Build and run**

Run: `cargo build && cargo run`
Expected: MissionView shows dungeon with "Abort Mission" at bottom. Clicking sidebar nav leaves mission running.

**Step 5: Commit**

```bash
git add src/screens/mission_view.rs
git commit -m "feat: migrate MissionView to content_area with Abort button"
```

---

### Task 11: Wire mission completion cleanup

**Files:**
- Modify: `src/mission/combat.rs`

**Step 1: Add cleanup after mission completion/failure**

In `check_mission_completion`, after awarding rewards and firing the toast for completion, add entity cleanup for the completed mission:

```rust
// Clean up completed mission entities
for entity in &hero_tokens {
    commands.entity(entity.0).despawn();  // despawn token, not real hero
}
// dead_enemies are already despawned by handle_death_system, but any remaining:
// (handled by MissionEntity marker)

// Remove OnMission from party heroes
for &hero_entity in &party.0 {
    commands.entity(hero_entity).remove::<crate::mission::OnMission>();
}
commands.entity(mission_entity_id).despawn();
```

Do the same in the failure branch.

Also clean up resources: `RoomStatus`, `SimulationTimer`, `SimulationSpeed`, `ActiveDungeon`.

Note: This needs careful refactoring since the current function uses `Query` borrows. The entity ID for the mission needs to be captured. Adjust the mission query to include `Entity`:

The existing query already has: `Query<(&mut MissionProgress, &MissionInfo, &MissionParty), With<Mission>>` — add `Entity` to the tuple.

**Step 2: Despawn all MissionEntity-tagged entities on completion**

Add a parameter for `entities: Query<Entity, With<MissionEntity>>` and despawn them all in both the completion and failure branches.

**Step 3: Build and run**

Run: `cargo build && cargo run`
Expected: After mission completes, heroes return to available state, mission disappears from sidebar.

**Step 4: Commit**

```bash
git add src/mission/combat.rs
git commit -m "feat: cleanup mission entities on completion/failure"
```

---

### Task 12: Final integration test and cleanup

**Files:**
- Possibly modify: any files with remaining compilation errors

**Step 1: Full build**

Run: `cargo build`
Fix any remaining compilation errors.

**Step 2: Run clippy**

Run: `cargo clippy`
Fix any warnings.

**Step 3: End-to-end manual test**

Run: `cargo run`

Verify:
1. Game starts → Sidebar visible on left, Roster content on right
2. Gold displays "Gold: 0" in sidebar
3. Roster tab is highlighted in sidebar
4. Click "Missions" in sidebar → Mission board loads, Missions tab highlighted
5. Click a mission → PartySelect loads
6. Dispatch a mission → MissionView loads, mission appears in sidebar under "Active Missions"
7. Click "Roster" in sidebar → Roster loads, mission entry still visible in sidebar
8. Click the active mission in sidebar → MissionView loads, mission still running
9. Mission completes → Toast fires, mission removed from sidebar, heroes available again
10. "Abort Mission" button aborts and navigates to Missions

**Step 4: Commit any fixes**

```bash
git add -A
git commit -m "fix: integration fixes for sidebar navigation"
```
