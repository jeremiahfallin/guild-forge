//! The credits menu.

use bevy::{input::common_conditions::input_just_pressed, prelude::*};
use bevy_declarative::element::div::{Div, div};
use bevy_declarative::style::styled::Styled;
use bevy_declarative::style::values::px;

use crate::{asset_tracking::LoadResource, audio::music, menus::Menu, theme::widgets};

pub(super) fn plugin(app: &mut App) {
    app.add_systems(OnEnter(Menu::Credits), spawn_credits_menu);
    app.add_systems(
        Update,
        go_back.run_if(in_state(Menu::Credits).and(input_just_pressed(KeyCode::Escape))),
    );

    app.load_resource::<CreditsAssets>();
    app.add_systems(OnEnter(Menu::Credits), start_credits_music);
}

fn spawn_credits_menu(mut commands: Commands) {
    widgets::ui_root("Credits Menu")
        .insert((GlobalZIndex(2), DespawnOnExit(Menu::Credits)))
        .child(widgets::header("Created by"))
        .child(created_by())
        .child(widgets::header("Assets"))
        .child(assets())
        .child(widgets::game_button("Back", go_back_on_click))
        .spawn(&mut commands);
}

fn created_by() -> Div {
    grid(vec![
        ["Joe Shmoe", "Implemented alligator wrestling AI"],
        ["Jane Doe", "Made the music for the alien invasion"],
    ])
}

fn assets() -> Div {
    grid(vec![
        ["Ducky sprite", "CC0 by Caz Creates Games"],
        ["Button SFX", "CC0 by Jaszunio15"],
        ["Music", "CC BY 3.0 by Kevin MacLeod"],
        [
            "Bevy logo",
            "All rights reserved by the Bevy Foundation, permission granted for splash screen use when unmodified",
        ],
    ])
}

fn grid(content: Vec<[&'static str; 2]>) -> Div {
    let mut g = div().grid().gap_y(px(10.0)).gap_x(px(30.0));
    g.style_mut().grid_template_columns = RepeatedGridTrack::px(2, 400.0);
    g = g.insert(Name::new("Grid"));

    for (i, text) in content.into_iter().flatten().enumerate() {
        let mut lbl = widgets::label(text);
        lbl.style_mut().justify_self = if i.is_multiple_of(2) {
            JustifySelf::End
        } else {
            JustifySelf::Start
        };
        g = g.child(lbl);
    }

    g
}

fn go_back_on_click(_: On<Pointer<Click>>, mut next_menu: ResMut<NextState<Menu>>) {
    next_menu.set(Menu::Main);
}

fn go_back(mut next_menu: ResMut<NextState<Menu>>) {
    next_menu.set(Menu::Main);
}

#[derive(Resource, Asset, Clone, Reflect)]
#[reflect(Resource)]
struct CreditsAssets {
    #[dependency]
    music: Handle<AudioSource>,
}

impl FromWorld for CreditsAssets {
    fn from_world(world: &mut World) -> Self {
        let assets = world.resource::<AssetServer>();
        Self {
            music: assets.load("audio/music/Monkeys Spinning Monkeys.ogg"),
        }
    }
}

fn start_credits_music(mut commands: Commands, credits_music: Res<CreditsAssets>) {
    commands.spawn((
        Name::new("Credits Music"),
        DespawnOnExit(Menu::Credits),
        music(credits_music.music.clone()),
    ));
}
