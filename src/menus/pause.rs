//! The pause menu.

use bevy::{input::common_conditions::input_just_pressed, prelude::*};

use crate::{menus::Menu, screens::Screen, theme::widgets};

pub(super) fn plugin(app: &mut App) {
    app.add_systems(OnEnter(Menu::Pause), spawn_pause_menu);
    app.add_systems(
        Update,
        go_back.run_if(in_state(Menu::Pause).and(input_just_pressed(KeyCode::Escape))),
    );
}

fn spawn_pause_menu(mut commands: Commands) {
    widgets::ui_root("Pause Menu")
        .insert((GlobalZIndex(2), DespawnOnExit(Menu::Pause)))
        .child(widgets::header("Game paused"))
        .child(widgets::game_button("Continue", close_menu))
        .child(widgets::game_button("Settings", open_settings_menu))
        .child(widgets::game_button("Quit to title", quit_to_title))
        .spawn(&mut commands);
}

fn open_settings_menu(_: On<Pointer<Click>>, mut next_menu: ResMut<NextState<Menu>>) {
    next_menu.set(Menu::Settings);
}

fn close_menu(_: On<Pointer<Click>>, mut next_menu: ResMut<NextState<Menu>>) {
    next_menu.set(Menu::None);
}

fn quit_to_title(_: On<Pointer<Click>>, mut next_screen: ResMut<NextState<Screen>>) {
    next_screen.set(Screen::Title);
}

fn go_back(mut next_menu: ResMut<NextState<Menu>>) {
    next_menu.set(Menu::None);
}
