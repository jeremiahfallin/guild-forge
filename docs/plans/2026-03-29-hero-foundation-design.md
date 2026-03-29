# Phase 1: Hero Foundation — Design Document

**Date:** 2026-03-29
**Scope:** Hero data layer, ECS components, UI migration to bevy_declarative, Roster screen
**Approach:** Data-First (bottom-up)

---

## Context

Guild Forge is a guild management sim built on Bevy 0.18. The codebase currently has a working scaffold (menus, screen states, audio, demo player) but no game-specific systems. This phase implements the hero foundation — the data structures, entity system, and roster UI that everything else builds on.

The UI is being fully migrated from the existing `theme/widget.rs` system to `bevy_declarative`, a local fluent UI builder library. Two small upgrades to bevy_declarative are required first.

---

## 1. bevy_declarative Upgrades

### 1a. Widen event handler signatures

**Problem:** `.on_click()` accepts `impl FnMut(On<Pointer<Click>>) + Send + Sync + 'static` but Bevy observers support additional system params (e.g., `Res<...>`, `Query<...>`).

**Fix:** Change all event methods to accept `impl IntoObserverSystem<Pointer<E>, B, M> + Send + Sync + 'static`. The internal `Box<dyn FnOnce(&mut EntityCommands)>` erasure pattern already handles this.

**Files:** `bevy_declarative/src/element/div.rs`

### 1b. Add `.insert(impl Bundle)` method

**Problem:** Can't attach arbitrary components (`InteractionPalette`, `Button`, `Pickable`, `DespawnOnExit`, `GlobalZIndex`) to div/text elements.

**Fix:** Add `.insert()` method that stores bundles for insertion at spawn time. Uses same `Box<dyn FnOnce(&mut EntityCommands)>` pattern as observers.

**Files:** `bevy_declarative/src/element/div.rs`, `bevy_declarative/src/element/text.rs`

---

## 2. Hero Data Layer

### Data files (`assets/data/`)

**`classes.ron`** — Array of class definitions:
```ron
[
    (
        id: Warrior,
        name: "Warrior",
        description: "A frontline fighter...",
        stat_weights: (str: 3, dex: 1, con: 3, int: 0, wis: 1, cha: 1),
        starting_abilities: ["Slash", "Shield Block"],
    ),
    // Rogue, Mage, Cleric, Ranger
]
```

**`traits.ron`** — Array of trait definitions:
```ron
[
    (
        id: Brave,
        name: "Brave",
        description: "Charges into danger without hesitation.",
        stat_modifiers: (str: 1, con: 1, wis: -1),
        tags: ["aggressive", "morale_boost"],
    ),
    // Cautious, Greedy, Loner, Leader, Cursed, Lucky
]
```

**`names.ron`** — Name generation pools:
```ron
(
    first_names: ["Aldric", "Brenna", "Cedric", ...],
    surnames: ["the Bold", "Ironfist", "Shadowmend", ...],
)
```

### Rust types (`src/hero/data.rs`)

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Reflect)]
pub enum HeroClass { Warrior, Rogue, Mage, Cleric, Ranger }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Reflect)]
pub enum HeroTrait { Brave, Cautious, Greedy, Loner, Leader, Cursed, Lucky }

#[derive(Debug, Clone, Deserialize)]
pub struct StatWeights { pub str: i32, pub dex: i32, pub con: i32, pub int: i32, pub wis: i32, pub cha: i32 }

#[derive(Debug, Clone, Deserialize)]
pub struct ClassDef { pub id: HeroClass, pub name: String, pub description: String, pub stat_weights: StatWeights, pub starting_abilities: Vec<String> }

#[derive(Debug, Clone, Deserialize)]
pub struct TraitDef { pub id: HeroTrait, pub name: String, pub description: String, pub stat_modifiers: StatWeights, pub tags: Vec<String> }

#[derive(Debug, Clone, Deserialize)]
pub struct NamePool { pub first_names: Vec<String>, pub surnames: Vec<String> }

#[derive(Resource)] pub struct ClassDatabase(pub Vec<ClassDef>);
#[derive(Resource)] pub struct TraitDatabase(pub Vec<TraitDef>);
#[derive(Resource)] pub struct NameDatabase(pub NamePool);
```

Loading: Manual RON deserialization in an `OnEnter(Screen::Loading)` system using `std::fs::read_to_string` (or Bevy `AssetServer` if we add `bevy_common_assets`). The simpler approach (direct file read) is fine for Phase 1.

---

## 3. Hero ECS Layer (`src/hero/mod.rs`)

### Components

```rust
#[derive(Component, Reflect)] pub struct Hero; // marker

#[derive(Component, Reflect)]
pub struct HeroInfo {
    pub name: String,
    pub class: HeroClass,
    pub level: u32,
    pub xp: u32,
    pub xp_to_next: u32,
}

#[derive(Component, Reflect)]
pub struct HeroStats {
    pub strength: i32,
    pub dexterity: i32,
    pub constitution: i32,
    pub intelligence: i32,
    pub wisdom: i32,
    pub charisma: i32,
}

#[derive(Component, Reflect)]
pub struct HeroTraits(pub Vec<HeroTrait>);
```

### Systems

- `spawn_starter_heroes` — On entering `Screen::Gameplay` (if no `Hero` entities exist), spawn 3 random heroes.
- `generate_random_hero(commands, class_db, trait_db, name_db, rng)` — Picks random class, 1-2 traits, generates name, rolls stats using class stat_weights + trait modifiers.

### Stat generation

Base stats: 8 for all. Each class weight point adds 1-2 (random). Trait modifiers applied on top. Result: stats typically range 6-16 at level 1.

---

## 4. UI Migration

### New theme module structure

```
src/theme/
├── mod.rs          — plugin aggregator (unchanged role)
├── palette.rs      — color constants (kept as-is)
├── widgets.rs      — NEW: bevy_declarative-based widget helpers
└── interaction.rs  — kept as-is (global observers for InteractionPalette)
```

**Delete:** `theme/widget.rs` (replaced by `widgets.rs`)

### `theme/widgets.rs` — New widget helpers

```rust
pub fn ui_root(name: impl Into<Cow<'static, str>>) -> Div {
    div()
        .absolute().w_full().h_full()
        .col().items_center().justify_center()
        .gap(px(20.0))
        .insert((Name::new(name), Pickable::IGNORE))
}

pub fn header(content: impl Into<String>) -> TextEl {
    text(content).font_size(40.0).color(HEADER_TEXT)
}

pub fn label(content: impl Into<String>) -> TextEl {
    text(content).font_size(24.0).color(LABEL_TEXT)
}

pub fn game_button<B: Bundle, M>(
    label: impl Into<String>,
    action: impl IntoObserverSystem<Pointer<Click>, B, M> + Send + Sync + 'static,
) -> Div {
    div()
        .w(px(380.0)).h(px(80.0))
        .items_center().justify_center()
        .bg(BUTTON_BACKGROUND)
        .rounded(Val::MAX)
        .insert((
            Button,
            InteractionPalette {
                none: BUTTON_BACKGROUND,
                hovered: BUTTON_HOVERED_BACKGROUND,
                pressed: BUTTON_PRESSED_BACKGROUND,
            },
        ))
        .on_click(action)
        .child(
            text(label).font_size(40.0).color(BUTTON_TEXT)
                .insert(Pickable::IGNORE)
        )
}
```

### Menu migration

Each menu screen (main, pause, settings, credits) replaces `widget::*` calls with the new `widgets::*` calls. Structure stays the same. Example for main menu:

```rust
fn spawn_main_menu(mut commands: Commands) {
    widgets::ui_root("Main Menu")
        .insert((GlobalZIndex(2), DespawnOnExit(Menu::Main)))
        .child(widgets::game_button("Play", enter_loading_or_gameplay_screen))
        .child(widgets::game_button("Settings", open_settings_menu))
        .child(widgets::game_button("Credits", open_credits_menu))
        .child(widgets::game_button("Exit", exit_app))
        .spawn(&mut commands);
}
```

---

## 5. Roster Screen

### Sub-state within Gameplay

```rust
#[derive(SubStates, Debug, Hash, PartialEq, Eq, Clone, Default)]
#[source(Screen = Screen::Gameplay)]
pub enum GameTab {
    #[default]
    Hub,
    Roster,
}
```

### Hub screen (`src/screens/hub.rs`)

Simple centered layout with navigation buttons: "Roster" (active), "Missions" (greyed out), "Armory" (greyed out). Plus the "Guild Forge" title header.

### Roster screen (`src/screens/roster.rs`)

**Layout:** Two-panel split — left 30% hero list, right 70% hero detail.

**Left panel:** Scrollable list of hero entries. Each entry shows name, class icon/text, and level. Clicking an entry updates a `SelectedHero(Entity)` resource.

**Right panel:** When a hero is selected, shows:
- Name + class + level header
- Stat bars (STR, DEX, CON, INT, WIS, CHA) with numeric values
- Traits list with descriptions
- (Future: equipment slots, abilities)

**Selection mechanic:** `SelectedHero(Option<Entity>)` resource. On change, despawn the detail panel and rebuild it. bevy_declarative's non-reactive model means we rebuild the UI tree on data change, which is fine for this use case.

### Navigation

- Hub → Roster: button sets `GameTab::Roster`
- Roster → Hub: "Back" button sets `GameTab::Hub`
- Pause menu still works via `DespawnOnExit` on each tab's UI

---

## 6. Deletions

- `src/demo/` (entire module — mod.rs, player.rs, level.rs, movement.rs, animation.rs)
- `src/theme/widget.rs` (replaced by widgets.rs)
- Remove `demo::plugin` from `main.rs`

---

## Verification

After each step:
1. `cargo build` passes
2. `cargo clippy` is clean
3. App runs: `cargo run`

End-to-end:
- Launch game → Title screen → Click Play → Hub screen with "Roster" button
- Click Roster → See 3 randomly generated heroes in the left panel
- Click a hero → Right panel shows their stats, traits, class
- Press P/Escape → Pause menu works
- Navigate back to Hub via "Back" button
