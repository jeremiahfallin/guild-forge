//! FT-1 guided first mission: a skippable five-beat first session shown as a
//! pinned coach-mark panel. Beats advance off observable game state.

use bevy::prelude::*;
use bevy_declarative::element::div::div;
use bevy_declarative::element::text::text;
use bevy_declarative::style::styled::Styled;
use bevy_declarative::style::values::px;

use crate::localization::{tr, trf};
use crate::hero::Hero;
use crate::mission::{Mission, MissionProgress, active_mission_count};
use crate::screens::{GameTab, Screen};
use crate::theme::widgets;

/// Prompt key per step. Index = step.
pub const TUTORIAL_STEPS: [&str; 5] = [
    "tutorial.step0",
    "tutorial.step1",
    "tutorial.step2",
    "tutorial.step3",
    "tutorial.step4",
];

/// Guided-first-mission progress. `step`/`done` persist in the save;
/// `saw_active_mission` is session-local bookkeeping for beat 3.
/// Primitives only in the persisted form (see memory: ron-value-lossy-enums).
#[derive(Resource, Debug, Clone, PartialEq, Default)]
pub struct TutorialState {
    pub step: u32,
    pub done: bool,
    pub saw_active_mission: bool,
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

/// Marker event: the Skip Tutorial / Done button.
#[derive(Event)]
pub struct SkipTutorial;

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<TutorialState>();
    app.add_observer(handle_skip);
    app.add_systems(
        Update,
        (advance_tutorial.run_if(tutorial_active), render_tutorial_panel)
            .chain()
            .run_if(in_state(Screen::Gameplay)),
    );
}

/// Marker for the coach-mark overlay root.
#[derive(Component)]
struct TutorialPanelUi;

/// Rebuild the pinned panel when the step changes; clear it once done.
fn render_tutorial_panel(
    mut commands: Commands,
    state: Res<TutorialState>,
    gameplay_root: Query<Entity, With<widgets::GameplayRoot>>,
    panel_q: Query<Entity, With<TutorialPanelUi>>,
    mut last_step: Local<Option<u32>>,
) {
    let Ok(root_entity) = gameplay_root.single() else {
        return;
    };

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
        .map(|key| tr(key))
        .unwrap_or("");
    let is_last = state.step as usize == TUTORIAL_STEPS.len() - 1;
    let accent = Color::srgba(0.8, 0.55, 0.05, 0.9);

    let mut panel_box = div()
        .col()
        .w(px(560.0))
        .gap(px(8.0))
        .p(px(14.0))
        .bg(Color::srgba(0.1, 0.12, 0.2, 0.95))
        .rounded(px(8.0))
        .insert(BorderColor::all(accent));
    panel_box.style_mut().border = UiRect::all(Val::Px(1.0));

    panel_box = panel_box
        .child(
            text(trf("tutorial.guide_counter", &[("step", &(state.step + 1).to_string()), ("total", &TUTORIAL_STEPS.len().to_string())]))
                .font_size(13.0)
                .color(accent),
        )
        .child(
            text(step_text)
                .font_size(17.0)
                .color(Color::srgb(0.92, 0.92, 0.95)),
        );

    let button_label = if is_last { tr("tutorial.done") } else { tr("tutorial.skip") };
    panel_box = panel_box.child(
        div().row().justify_end().child(
            div()
                .p(px(6.0))
                .pad_x(px(12.0))
                .bg(Color::srgba(0.25, 0.28, 0.4, 0.9))
                .rounded(px(6.0))
                .insert(Button)
                .on_click(
                    |_: On<Pointer<Click>>, mut commands: Commands| {
                        commands.trigger(SkipTutorial);
                    },
                )
                .child(
                    text(button_label)
                        .font_size(14.0)
                        .color(Color::srgb(0.85, 0.85, 0.9)),
                ),
        ),
    );

    let mut wrapper = div()
        .absolute()
        .w_full()
        .row()
        .justify_center()
        .insert((TutorialPanelUi, GlobalZIndex(40), Pickable::IGNORE));
    wrapper.style_mut().top = Val::Px(52.0);
    wrapper
        .child(panel_box)
        .spawn_as_child_of(&mut commands, root_entity);
}

pub fn tutorial_active(state: Res<TutorialState>) -> bool {
    !state.done
}

fn handle_skip(_: On<SkipTutorial>, mut state: ResMut<TutorialState>) {
    state.done = true;
    info!("Tutorial finished/skipped at step {}", state.step);
}

fn advance_tutorial(
    mut state: ResMut<TutorialState>,
    heroes: Query<(), With<Hero>>,
    tab: Option<Res<State<GameTab>>>,
    missions: Query<&MissionProgress, With<Mission>>,
) {
    let active = active_mission_count(&missions);
    if active > 0 && state.step >= 2 && !state.saw_active_mission {
        state.saw_active_mission = true;
    }
    let in_party_select = tab.is_some_and(|t| t.get() == &GameTab::PartySelect);
    let next = target_step(&state, heroes.iter().count(), in_party_select, active);
    if next != state.step {
        info!("Tutorial step {} -> {next}", state.step);
        state.step = next;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(step: u32, saw: bool) -> TutorialState {
        TutorialState {
            step,
            done: false,
            saw_active_mission: saw,
        }
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
        let done = TutorialState {
            step: 1,
            done: true,
            saw_active_mission: false,
        };
        assert_eq!(target_step(&done, 3, true, 5), 1);
    }

    #[test]
    fn every_step_has_text() {
        for step in 0..TUTORIAL_STEPS.len() as u32 {
            assert!(!tr(TUTORIAL_STEPS[step as usize]).is_empty());
        }
    }
}
