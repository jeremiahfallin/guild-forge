//! The game's main screen states and transitions between them.

mod gameplay;
mod hub;
mod loading;
mod roster;
mod splash;
mod title;

use bevy::prelude::*;

pub(super) fn plugin(app: &mut App) {
    app.init_state::<Screen>();
    app.add_sub_state::<GameTab>();

    app.add_plugins((
        gameplay::plugin,
        hub::plugin,
        loading::plugin,
        roster::plugin,
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
    Hub,
    Roster,
}
