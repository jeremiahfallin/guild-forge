//! The game's main screen states and transitions between them.

mod armory;
mod gameplay;
mod guild;
mod loading;
pub mod mission_view;
pub mod missions;
mod party_select;
mod recruiting_screen;
mod roster;
mod sidebar;
mod splash;
mod title;

use bevy::prelude::*;

pub(super) fn plugin(app: &mut App) {
    app.init_state::<Screen>();
    app.add_sub_state::<GameTab>();

    app.add_plugins((
        armory::plugin,
        gameplay::plugin,
        guild::plugin,
        loading::plugin,
        mission_view::plugin,
        missions::plugin,
        party_select::plugin,
        recruiting_screen::plugin,
        roster::plugin,
        sidebar::plugin,
        splash::plugin,
        title::plugin,
    ));
}

/// The game's main screen states.
#[derive(States, Copy, Clone, Eq, PartialEq, Hash, Debug, Default)]
pub enum Screen {
    #[default]
    Splash,
    Title,
    Loading,
    Gameplay,
}

/// Sub-state within gameplay for navigating between guild tabs.
#[derive(SubStates, Copy, Clone, Eq, PartialEq, Hash, Debug, Default)]
#[source(Screen = Screen::Gameplay)]
pub enum GameTab {
    #[default]
    Roster,
    Missions,
    PartySelect,
    MissionView,
    Guild,
    Armory,
    Recruiting,
}
