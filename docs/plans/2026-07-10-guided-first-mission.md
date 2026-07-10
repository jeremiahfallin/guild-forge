# FT-1 Guided First Mission Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** A skippable five-beat guided first session (hire → pick contract → dispatch → watch → graduate) shown as a pinned coach-mark panel, with fresh games starting at 2 heroes + 60 gold.

**Architecture:** New `src/tutorial.rs` holds `TutorialState { step, done, saw_active_mission }` plus a pure `target_step()` decision fn; an Update system feeds it observable state (roster count, current tab, active missions) and a panel renderer rebuilds one coach-mark overlay under `GameplayRoot` on step change. Persistence adds `tutorial_done`/`tutorial_step` primitives to `SaveData` with `tutorial_done` defaulting **true** so existing saves skip.

**Tech Stack:** Bevy 0.18 (binary crate — `cargo test tutorial::`), bevy_declarative UI, existing save pipeline. Design: `docs/plans/2026-07-10-guided-first-mission-design.md`. House test style as in `src/music.rs` / `src/ui/banner.rs`.

---

### Task 1: `tutorial.rs` core — state, decision fn, step text

**Files:**
- Create: `src/tutorial.rs`
- Modify: `src/main.rs` (add `mod tutorial;` after `mod training;`; add `tutorial::plugin,` to the second plugin group after `training::plugin,`)

**Step 1: Create `src/tutorial.rs` with tests first**

```rust
//! FT-1 guided first mission: a skippable five-beat first session shown as a
//! pinned coach-mark panel. Beats advance off observable game state.

use bevy::prelude::*;

/// Prompt text per step. Index = step.
pub const TUTORIAL_STEPS: [&str; 5] = [
    "Welcome, guildmaster! You have two heroes and 60 gold. Hire a third at the Recruiting office.",
    "A full party of three! Open the Mission Board and pick a contract.",
    "Add all three heroes to the party, then hit Dispatch!",
    "Watch the run: exploration is brisk, combat slows time, and the log narrates the fight.",
    "Mission resolved and rewards collected. Upgrade buildings, take harder contracts, recruit again — the guild is yours.",
];

/// Guided-first-mission progress. `step`/`done` persist in the save;
/// `saw_active_mission` is session-local bookkeeping for beat 3.
/// Primitives only in the persisted form (see memory: ron-value-lossy-enums).
#[derive(Resource, Debug, Clone, PartialEq)]
pub struct TutorialState {
    pub step: u32,
    pub done: bool,
    pub saw_active_mission: bool,
}

impl Default for TutorialState {
    fn default() -> Self {
        Self { step: 0, done: false, saw_active_mission: false }
    }
}

/// Decide the step from observable state. Advances at most one beat per call;
/// never goes backward.
pub fn target_step(
    state: &TutorialState,
    roster_count: usize,
    in_party_select: bool,
    active_missions: usize,
) -> u32 {
    if state.done {
        return state.step;
    }
    match state.step {
        0 if roster_count >= 3 => 1,
        1 if in_party_select => 2,
        2 if active_missions > 0 => 3,
        3 if state.saw_active_mission && active_missions == 0 => 4,
        s => s,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(step: u32, saw: bool) -> TutorialState {
        TutorialState { step, done: false, saw_active_mission: saw }
    }

    #[test]
    fn beats_advance_on_their_signals() {
        assert_eq!(target_step(&at(0, false), 2, false, 0), 0);
        assert_eq!(target_step(&at(0, false), 3, false, 0), 1);
        assert_eq!(target_step(&at(1, false), 3, false, 0), 1);
        assert_eq!(target_step(&at(1, false), 3, true, 0), 2);
        assert_eq!(target_step(&at(2, false), 3, false, 0), 2);
        assert_eq!(target_step(&at(2, false), 3, false, 1), 3);
        // Beat 3 needs to have SEEN a mission before resolving on zero
        assert_eq!(target_step(&at(3, false), 3, false, 0), 3);
        assert_eq!(target_step(&at(3, true), 3, false, 1), 3);
        assert_eq!(target_step(&at(3, true), 3, false, 0), 4);
        // Graduation holds until Done/Skip
        assert_eq!(target_step(&at(4, true), 3, false, 0), 4);
    }

    #[test]
    fn done_freezes_progress() {
        let done = TutorialState { step: 1, done: true, saw_active_mission: false };
        assert_eq!(target_step(&done, 3, true, 5), 1);
    }

    #[test]
    fn every_step_has_text() {
        for step in 0..TUTORIAL_STEPS.len() as u32 {
            assert!(!TUTORIAL_STEPS[step as usize].is_empty());
        }
    }
}
```

Plus an empty plugin for now:

```rust
pub(super) fn plugin(app: &mut App) {
    app.init_resource::<TutorialState>();
}
```

**Step 2:** Wire `mod tutorial;` + `tutorial::plugin,` in `src/main.rs`. Run: `cargo test tutorial::` — expect 3 PASS (red first if you prefer: run before creating the file to see E0583).

**Step 3: Commit**

```bash
git add src/tutorial.rs src/main.rs
git commit -m "feat(ft1): tutorial state machine core"
```

---

### Task 2: Fresh-game hooks — 2 starters, 60 gold, affordable applicant

**Files:**
- Modify: `src/hero/mod.rs:144-162` (`spawn_starter_heroes`)
- Modify: `src/recruiting.rs:179-202` (`seed_applicant_board`), tests at bottom

**Step 1: Failing test for the clamp helper** (bottom of `src/recruiting.rs`; check for an existing `#[cfg(test)]` mod first and extend it if present)

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_seeded_applicant_is_affordable() {
        assert_eq!(clamp_starter_cost(120), 50);
        assert_eq!(clamp_starter_cost(50), 50);
        assert_eq!(clamp_starter_cost(35), 35);
    }
}
```

**Step 2:** Run `cargo test recruiting::` — compile FAIL (helper missing).

**Step 3: Implement**

In `src/recruiting.rs`:

```rust
/// Fresh games start with 60 gold (FT-1); the first seeded applicant must be
/// hireable with it so the tutorial's recruit beat can always complete.
pub const STARTER_APPLICANT_MAX_COST: u32 = 50;

pub fn clamp_starter_cost(cost: u32) -> u32 {
    cost.min(STARTER_APPLICANT_MAX_COST)
}
```

In `seed_applicant_board`, after the seeding loop (line ~201):

```rust
    if let Some(first) = board.applicants.first_mut() {
        first.hire_cost = clamp_starter_cost(first.hire_cost);
    }
```

In `src/hero/mod.rs` `spawn_starter_heroes`: change the loop to `for _ in 0..2`, the log line to `"Spawned 2 starter heroes"`, the doc comment to match, and add starting gold — new param `mut gold: ResMut<crate::economy::Gold>,` and inside the fresh-game branch (after the loop):

```rust
    gold.0 = 60; // FT-1: enough to hire the clamped starter applicant
```

**Step 4:** Run `cargo test` — full suite green (nothing asserts 3 starters today; if something does, update it to 2 deliberately).

**Step 5: Commit**

```bash
git add src/recruiting.rs src/hero/mod.rs
git commit -m "feat(ft1): fresh games start with 2 heroes, 60g, affordable applicant"
```

---

### Task 3: Advance/skip systems + save persistence

**Files:**
- Modify: `src/tutorial.rs`, `src/save.rs:955-969` (SaveData), `src/save.rs:146-151` (load restore), `src/save.rs:502+` (write params) & `817-831` (assemble)

**Step 1: Failing save round-trip test** (in `src/save.rs` tests mod; existing `SaveData` literal constructions in tests around lines 1250/1282 must gain the new fields — set `tutorial_done: true, tutorial_step: 0`)

```rust
    #[test]
    fn tutorial_fields_default_done_for_old_saves() {
        // A pre-FT-1 save has no tutorial fields — it must deserialize as done
        // so existing players never see the tutorial.
        let old_save = r#"(
            version: 1,
            last_save_timestamp: 0,
            gold: 5,
            reputation: 0,
            banked_seconds: 0.0,
            materials: {},
            buildings: {},
            heroes: [],
            applicants: [],
            next_arrival_timer: 0.0,
            training_timer: 0.0,
            missions: [],
        )"#;
        let parsed: SaveData = ron::from_str(old_save).expect("old save parses");
        assert!(parsed.tutorial_done);
        assert_eq!(parsed.tutorial_step, 0);
    }
```

(Mirror the version/field shape of the existing minimal-save test at ~1250 — copy its literal and delete the tutorial fields.)

**Step 2:** Run `cargo test save::` — compile FAIL (fields missing).

**Step 3: Implement**

`SaveData` (after `rescue_offers`):

```rust
    /// FT-1 tutorial. Defaults TRUE so saves predating the field skip it.
    #[serde(default = "default_tutorial_done")]
    pub tutorial_done: bool,
    #[serde(default)]
    pub tutorial_step: u32,
```

```rust
fn default_tutorial_done() -> bool {
    true
}
```

Load restore (~line 151):

```rust
    commands.insert_resource(crate::tutorial::TutorialState {
        step: save_data.tutorial_step,
        done: save_data.tutorial_done,
        saw_active_mission: false,
    });
```

Write: add `tutorial: Res<crate::tutorial::TutorialState>,` to the save-writing system's params and to the `SaveData` literal:

```rust
        tutorial_done: tutorial.done,
        tutorial_step: tutorial.step,
```

In `src/tutorial.rs`, the advance system + observers:

```rust
use crate::hero::Hero;
use crate::mission::{Mission, MissionProgress, active_mission_count};
use crate::screens::{GameTab, Screen};

/// Marker event: the Skip Tutorial button.
#[derive(Event)]
pub struct SkipTutorial;

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<TutorialState>();
    app.add_observer(handle_skip);
    app.add_systems(
        Update,
        advance_tutorial
            .run_if(in_state(Screen::Gameplay).and(tutorial_active)),
    );
}

pub fn tutorial_active(state: Res<TutorialState>) -> bool {
    !state.done
}

fn handle_skip(_: On<SkipTutorial>, mut state: ResMut<TutorialState>) {
    state.done = true;
    info!("Tutorial skipped/completed at step {}", state.step);
}

fn advance_tutorial(
    mut state: ResMut<TutorialState>,
    heroes: Query<(), With<Hero>>,
    tab: Option<Res<State<GameTab>>>,
    missions: Query<&MissionProgress, With<Mission>>,
) {
    let active = active_mission_count(&missions);
    if active > 0 && state.step >= 2 {
        state.saw_active_mission = true;
    }
    let in_party_select = tab.is_some_and(|t| t.get() == &GameTab::PartySelect);
    let next = target_step(&state, heroes.iter().count(), in_party_select, active);
    if next != state.step {
        info!("Tutorial step {} -> {next}", state.step);
        state.step = next;
    }
}
```

(`bypass_change_detection` is unnecessary — `saw_active_mission` writes dirty the resource, which is fine; the panel renderer keys on step value, not change ticks. If the resource-eq guard from settings.rs style is wanted, compare before writing.)

**Step 4:** `cargo test` full suite + `cargo clippy --all-targets` — green, 65-warning baseline.

**Step 5: Commit**

```bash
git add src/tutorial.rs src/save.rs
git commit -m "feat(ft1): tutorial advance/skip systems and save persistence"
```

---

### Task 4: Coach-mark panel UI

**Files:**
- Modify: `src/tutorial.rs`

**Step 1: Implement the renderer** (compile-driven; pure logic already tested)

```rust
use bevy_declarative::element::div::div;
use bevy_declarative::element::text::text;
use bevy_declarative::style::styled::Styled;
use bevy_declarative::style::values::px;

use crate::theme::widgets;

/// Marker for the coach-mark overlay root.
#[derive(Component)]
struct TutorialPanelUi;

/// Rebuild the panel when the step changes (or spawn it on entering gameplay).
fn render_tutorial_panel(
    mut commands: Commands,
    state: Res<TutorialState>,
    gameplay_root: Query<Entity, With<widgets::GameplayRoot>>,
    panel_q: Query<Entity, With<TutorialPanelUi>>,
    mut last_step: Local<Option<u32>>,
) {
    let Ok(root_entity) = gameplay_root.single() else { return };

    if state.done {
        for e in &panel_q {
            commands.entity(e).despawn();
        }
        *last_step = None;
        return;
    }
    if *last_step == Some(state.step) && !panel_q.is_empty() {
        return;
    }
    *last_step = Some(state.step);

    for e in &panel_q {
        commands.entity(e).despawn();
    }

    let step_text = TUTORIAL_STEPS
        .get(state.step as usize)
        .copied()
        .unwrap_or("");
    let is_last = state.step as usize == TUTORIAL_STEPS.len() - 1;

    let mut panel = div()
        .col()
        .max_w(px(560.0))
        .gap(px(8.0))
        .p(px(14.0))
        .bg(Color::srgba(0.12, 0.14, 0.22, 0.95))
        .rounded(px(8.0))
        .border(px(1.0), Color::srgba(0.8, 0.55, 0.05, 0.8))
        .child(
            text(format!("Guide {}/{}", state.step + 1, TUTORIAL_STEPS.len()))
                .font_size(13.0)
                .color(Color::srgba(0.8, 0.55, 0.05, 1.0)),
        )
        .child(text(step_text).font_size(17.0).color(Color::srgb(0.92, 0.92, 0.95)));

    let button_label = if is_last { "Done" } else { "Skip Tutorial" };
    panel = panel.child(
        div()
            .row()
            .justify_end()
            .child(
                div()
                    .p(px(6.0))
                    .pad_x(px(12.0))
                    .bg(Color::srgba(0.25, 0.28, 0.4, 0.9))
                    .rounded(px(6.0))
                    .on_click(|_: On<Pointer<Click>>, mut commands: Commands| {
                        commands.trigger(SkipTutorial);
                    })
                    .child(
                        text(button_label)
                            .font_size(14.0)
                            .color(Color::srgb(0.85, 0.85, 0.9)),
                    ),
            ),
    );

    div()
        .absolute()
        .w_full()
        .row()
        .justify_center()
        .insert((TutorialPanelUi, GlobalZIndex(40), Pickable::IGNORE))
        .style_mut_with(|s| s.top = Val::Px(52.0))
        .child(panel)
        .spawn_as_child_of(&mut commands, root_entity);
}
```

**Adapt to the actual bevy_declarative API while implementing** — check `src/screens/mission_view.rs` `update_banner_ui` (~line 690+) for the exact wrapper idiom (`.absolute()`, `GlobalZIndex`, `Pickable::IGNORE`, `spawn_as_child_of`, and how `top` offset is set — mimic it precisely, including whether `.style_mut()` or a builder method sets `top`). The banner uses `GlobalZIndex(50)`; use 40 so banners still win. `.border`/`.max_w`/`.pad_x` — verify these builder methods exist (grep `fn border`, `fn max_w` in `crates/bevy_declarative`); substitute with `style_mut()` field writes if absent. The Skip click handler must NOT be `Pickable::IGNORE` — only the outer wrapper is (again mirroring how banners keep their wrapper non-interactive; if the wrapper's `Pickable::IGNORE` would block the button, attach it only to the wrapper node as banners do — descendants stay pickable in bevy_picking unless they opt out).

Register in the plugin:

```rust
    app.add_systems(
        Update,
        render_tutorial_panel.run_if(in_state(Screen::Gameplay)),
    );
```

(`render_tutorial_panel` itself handles the done-state despawn, so it runs regardless of `tutorial_active`.)

**Step 2:** `cargo test && cargo clippy --all-targets` — green, baseline warnings.

**Step 3: Commit**

```bash
git add src/tutorial.rs
git commit -m "feat(ft1): coach-mark tutorial panel"
```

---

### Task 5: Final gate — hand verification, tick, merge

**Step 1:** `cargo test` (expect ~115 passing), `cargo clippy --all-targets` (65-warning baseline).

**Step 2: Hand-verify on a fresh save** (memory `driving-game-for-verification`; check the desktop is free and ask before taking the cursor if the user is active):
1. Delete `save.ron`/`save.ron.bak`, `cargo run`, Play. Panel shows beat 1/5; roster has 2 heroes; gold 60.
2. Recruiting tab → hire the first applicant (≤50g) → panel advances to 2/5.
3. Mission Board → Goblin Cave → panel 3/5 on entering party select.
4. Add all 3, Dispatch → panel 4/5 in mission view; panel visible across tab switches.
5. Let the mission resolve (~5 min; watch the output log for `Mission '...' complete`) → panel 5/5 with Done. Click Done → panel gone.
6. Relaunch → panel stays gone (save carries done). Check `save.ron` contains `tutorial_done: true`.
7. Fresh save again → panel back at 1/5 → click Skip Tutorial → gone; relaunch → still gone.
8. Sanity: an OLD save (any save written before this branch) loads without the panel — covered by the serde-default test, spot-check if one exists.

**Step 3:** Tick FT-1 in `docs/steam-release-chunks.md` (~line 143) with date + note (2-hero/60g fresh start, coach-mark panel, save-persisted, skippable; design doc link).

**Step 4: Merge**

```bash
git add docs/steam-release-chunks.md
git commit -m "docs: tick FT-1 — guided first mission shipped"
git checkout main
git merge --no-ff feat/ft1-guided-first-mission -m "Merge branch 'feat/ft1-guided-first-mission' — FT-1 guided first mission"
cargo test
git branch -d feat/ft1-guided-first-mission
```
