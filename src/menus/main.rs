//! The main menu (seen on the title screen).

use bevy::prelude::*;

use crate::{asset_tracking::ResourceHandles, menus::Menu, screens::Screen, theme::widgets};

pub(super) fn plugin(app: &mut App) {
    app.add_systems(OnEnter(Menu::Main), spawn_main_menu);
}

fn spawn_main_menu(mut commands: Commands) {
    let mut menu = widgets::ui_root("Main Menu")
        .insert((GlobalZIndex(2), DespawnOnExit(Menu::Main)))
        .child(widgets::game_button("Play", enter_loading_or_gameplay_screen))
        .child(widgets::game_button("Settings", open_settings_menu))
        .child(widgets::game_button("Credits", open_credits_menu));

    #[cfg(not(target_family = "wasm"))]
    {
        menu = menu.child(widgets::game_button("Exit", exit_app));
    }

    menu.spawn(&mut commands);
}

fn enter_loading_or_gameplay_screen(
    _: On<Pointer<Click>>,
    resource_handles: Res<ResourceHandles>,
    mut next_screen: ResMut<NextState<Screen>>,
) {
    if resource_handles.is_all_done() {
        next_screen.set(Screen::Gameplay);
    } else {
        next_screen.set(Screen::Loading);
    }
}

fn open_settings_menu(_: On<Pointer<Click>>, mut next_menu: ResMut<NextState<Menu>>) {
    next_menu.set(Menu::Settings);
}

fn open_credits_menu(_: On<Pointer<Click>>, mut next_menu: ResMut<NextState<Menu>>) {
    next_menu.set(Menu::Credits);
}

#[cfg(not(target_family = "wasm"))]
fn exit_app(_: On<Pointer<Click>>, mut app_exit: MessageWriter<AppExit>) {
    app_exit.write(AppExit::Success);
}
