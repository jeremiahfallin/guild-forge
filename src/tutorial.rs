//! FT-1 guided first mission: a skippable five-beat first session shown as a
//! pinned coach-mark panel. Beats advance off observable game state.

use bevy::prelude::*;

use crate::hero::Hero;
use crate::mission::{Mission, MissionProgress, active_mission_count};
use crate::screens::{GameTab, Screen};

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
        advance_tutorial.run_if(in_state(Screen::Gameplay).and(tutorial_active)),
    );
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
            assert!(!TUTORIAL_STEPS[step as usize].is_empty());
        }
    }
}
