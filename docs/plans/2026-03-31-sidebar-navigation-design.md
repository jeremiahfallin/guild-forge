# Sidebar Navigation — Design Document

**Date:** 2026-03-31
**Scope:** Replace hub-based navigation with a persistent left sidebar across all gameplay screens

---

## Context

Guild Forge currently uses a Hub screen with buttons (Roster, Missions, Armory) as the central navigation point. Each sub-screen has its own Back button that returns to the Hub. This adds friction — every navigation action requires going through the Hub first.

The sidebar replaces this with persistent, always-visible navigation that also displays guild status (gold) and active missions. This is especially important for the future concurrent-missions feature, where the player needs to monitor multiple missions at a glance.

---

## 1. bevy_declarative Upgrade

### `overflow_y_scroll()` method

**Problem:** The active missions list in the sidebar needs to scroll when there are more missions than vertical space allows. bevy_declarative only has `overflow_y_hidden` (clip) and `overflow_visible`.

**Fix:** Add `overflow_y_scroll()` to the `Styled` trait, setting `overflow.y = OverflowAxis::Scroll` and inserting `ScrollPosition::default()` at spawn time.

**Files:** `bevy_declarative/src/style/styled.rs`

---

## 2. Sidebar Layout

### Structure

```
┌──────────────┬──────────────────────────────────┐
│  SIDEBAR     │                                  │
│  (fixed 220px│      CONTENT AREA                │
│   left)      │      (fills remaining width)     │
│              │                                  │
│ ┌──────────┐ │      Each GameTab's UI renders   │
│ │Guild Forge│ │      here                        │
│ │Gold: 45  │ │                                  │
│ ├──────────┤ │                                  │
│ │ Roster * │ │                                  │
│ │ Missions │ │                                  │
│ │ Armory   │ │                                  │
│ ├──────────┤ │                                  │
│ │ ACTIVE   │ │                                  │
│ │ MISSIONS │ │                                  │
│ │(scrollable)│                                  │
│ │          │ │                                  │
│ │ Goblin.. │ │                                  │
│ │ Skeleton.│ │                                  │
│ └──────────┘ │                                  │
└──────────────┴──────────────────────────────────┘
```

### Pinned Sections (top, never scroll)

1. **Header** — "Guild Forge" title text
2. **Gold display** — "Gold: {amount}", updated reactively when `Gold` resource changes
3. **Navigation buttons** — Roster, Missions, Armory (disabled). Active `GameTab` is visually highlighted (brighter background). Clicking sets the corresponding `GameTab`.
4. **Divider** — Visual separator

### Scrollable Section (fills remaining height)

5. **Active Missions label** — "Active Missions" header
6. **Mission entries** — One row per in-progress mission entity. Shows mission name and status. Clicking sets `GameTab::MissionView` and sets context to that mission. Scrolls independently of the pinned section above.

### Lifecycle

- Spawned on `OnEnter(Screen::Gameplay)` with `DespawnOnExit(Screen::Gameplay)`
- Lives outside any `GameTab` lifecycle — persists across all tab transitions
- `GlobalZIndex` ensures it renders above game content but below modals/pause

### Reactive Updates

- **Gold:** System runs on `resource_changed::<Gold>`, finds the gold text entity by marker component, updates its text.
- **Active missions:** System runs when `Mission` + `MissionProgress` components change (added/removed/mutated), rebuilds the scrollable mission list.
- **Active tab highlight:** System runs on `state_changed::<GameTab>`, updates nav button background colors.

---

## 3. Hub Removal

The Hub screen (`src/screens/hub.rs`) is removed. Its responsibilities are absorbed:

- Gold display → sidebar header
- Navigation buttons → sidebar nav
- "Guild Forge" title → sidebar header

The default `GameTab` changes from `Hub` to `Roster`.

---

## 4. Screen Modifications

### All management screens (Roster, Missions, PartySelect)

- Remove "Back" button — sidebar handles navigation
- Remove per-screen headers (e.g., "Roster", "Mission Board") — the active tab highlight in the sidebar indicates where you are, but screens can keep a smaller contextual header if needed
- Root node changes from `absolute().w_full().h_full()` to a layout that fills the content area to the right of the sidebar

### MissionView

- Remove the "Retreat" button from the UI overlay
- Add a floating "Abort Mission" button anchored bottom-center within the content area
- Navigating away via sidebar leaves the mission running in the background
- Clicking "Abort Mission" stops the mission, triggers cleanup, and navigates to Missions tab

---

## 5. Content Area Widget

New widget helper:

```rust
pub fn content_area(name: impl Into<Cow<'static, str>>) -> Div {
    div()
        .col()
        .flex_1()
        .h_full()
        .items_center()
        .justify_center()
        .gap(px(20.0))
        .insert((Name::new(name), Pickable::IGNORE))
}
```

Each screen uses `content_area("Roster")` instead of `ui_root("Roster")`. The sidebar + content area are arranged in a row by a gameplay-level root container.

---

## 6. Gameplay Root Container

A new persistent root spawned on `OnEnter(Screen::Gameplay)`:

```
Row (absolute, w_full, h_full)
├── Sidebar (w: 220px, h_full, col)
│   ├── Pinned section (col)
│   │   ├── Title
│   │   ├── Gold
│   │   ├── Nav buttons
│   │   └── Divider
│   └── Scrollable section (col, flex_1, overflow_y_scroll)
│       └── Active mission entries
└── Content area (flex_1, h_full)
    └── [Each GameTab's UI spawns here]
```

---

## Deletions

- `src/screens/hub.rs` — Replaced by sidebar
- Hub references in `src/screens/mod.rs`

## Verification

After each step:
1. `cargo build` passes
2. `cargo clippy` is clean
3. App runs: `cargo run`

End-to-end:
- Launch game → Title → Play → Sidebar visible with Roster content
- Gold displays correctly
- Click Missions in sidebar → Missions screen loads, Missions button highlighted
- Dispatch a mission → Active mission appears in sidebar list
- Click active mission → MissionView loads, mission running
- Click Roster in sidebar → Roster loads, mission continues in background
- Click active mission again → Back to MissionView, mission still going
- Abort Mission button → Mission stops, returns to Missions tab
