//! The settings menu.
//!
//! Additional settings and accessibility options should go here.

use bevy::{audio::Volume, input::common_conditions::input_just_pressed, prelude::*};
use bevy_declarative::element::div::{Div, div};
use bevy_declarative::style::styled::Styled;
use bevy_declarative::style::values::px;

use crate::{menus::Menu, screens::Screen, theme::widgets};

pub(super) fn plugin(app: &mut App) {
    app.add_systems(OnEnter(Menu::Settings), spawn_settings_menu);
    app.add_systems(
        Update,
        go_back.run_if(in_state(Menu::Settings).and(input_just_pressed(KeyCode::Escape))),
    );

    app.add_systems(
        Update,
        update_global_volume_label.run_if(in_state(Menu::Settings)),
    );
}

fn spawn_settings_menu(mut commands: Commands) {
    widgets::ui_root("Settings Menu")
        .insert((GlobalZIndex(2), DespawnOnExit(Menu::Settings)))
        .child(widgets::header("Settings"))
        .child(settings_grid())
        .child(widgets::game_button("Back", go_back_on_click))
        .spawn(&mut commands);
}

fn settings_grid() -> Div {
    let mut volume_label = widgets::label("Master Volume");
    volume_label.style_mut().justify_self = JustifySelf::End;

    let mut grid = div().grid().gap_y(px(10.0)).gap_x(px(30.0));
    grid.style_mut().grid_template_columns = RepeatedGridTrack::px(2, 400.0);

    grid.insert(Name::new("Settings Grid"))
        .child(volume_label)
        .child(global_volume_widget())
}

fn global_volume_widget() -> Div {
    let mut container = div().row();
    container.style_mut().justify_self = JustifySelf::Start;

    container
        .insert(Name::new("Global Volume Widget"))
        .child(widgets::game_button_small("-", lower_global_volume))
        .child(
            div()
                .insert(Name::new("Current Volume"))
                .pad_x(px(10.0))
                .justify_center()
                .child(widgets::label("").insert(GlobalVolumeLabel)),
        )
        .child(widgets::game_button_small("+", raise_global_volume))
}

const MIN_VOLUME: f32 = 0.0;
const MAX_VOLUME: f32 = 3.0;

fn lower_global_volume(_: On<Pointer<Click>>, mut global_volume: ResMut<GlobalVolume>) {
    let linear = (global_volume.volume.to_linear() - 0.1).max(MIN_VOLUME);
    global_volume.volume = Volume::Linear(linear);
}

fn raise_global_volume(_: On<Pointer<Click>>, mut global_volume: ResMut<GlobalVolume>) {
    let linear = (global_volume.volume.to_linear() + 0.1).min(MAX_VOLUME);
    global_volume.volume = Volume::Linear(linear);
}

#[derive(Component, Reflect)]
#[reflect(Component)]
struct GlobalVolumeLabel;

fn update_global_volume_label(
    global_volume: Res<GlobalVolume>,
    mut label: Single<&mut Text, With<GlobalVolumeLabel>>,
) {
    let percent = 100.0 * global_volume.volume.to_linear();
    label.0 = format!("{percent:3.0}%");
}

fn go_back_on_click(
    _: On<Pointer<Click>>,
    screen: Res<State<Screen>>,
    mut next_menu: ResMut<NextState<Menu>>,
) {
    next_menu.set(if screen.get() == &Screen::Title {
        Menu::Main
    } else {
        Menu::Pause
    });
}

fn go_back(screen: Res<State<Screen>>, mut next_menu: ResMut<NextState<Menu>>) {
    next_menu.set(if screen.get() == &Screen::Title {
        Menu::Main
    } else {
        Menu::Pause
    });
}
