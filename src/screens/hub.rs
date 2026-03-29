//! The guild hub — main navigation screen within gameplay.

use bevy::prelude::*;
use bevy_declarative::element::div::div;
use bevy_declarative::style::styled::Styled;
use bevy_declarative::style::values::px;

use crate::{
    screens::GameTab,
    theme::{palette::*, widgets},
};

pub(super) fn plugin(app: &mut App) {
    app.add_systems(OnEnter(GameTab::Hub), spawn_hub);
}

fn spawn_hub(mut commands: Commands) {
    widgets::ui_root("Guild Hub")
        .insert(DespawnOnExit(GameTab::Hub))
        .child(widgets::header("Guild Forge"))
        .child(
            widgets::label("Manage your guild of adventurers")
                .color(LABEL_TEXT),
        )
        .child(
            div()
                .col()
                .gap(px(12.0))
                .items_center()
                .child(widgets::game_button("Roster", open_roster))
                .child(disabled_button("Missions"))
                .child(disabled_button("Armory")),
        )
        .spawn(&mut commands);
}

/// A visually distinct button for features not yet implemented.
fn disabled_button(label: &str) -> bevy_declarative::element::div::Div {
    div()
        .w(px(380.0))
        .h(px(80.0))
        .items_center()
        .justify_center()
        .bg(Color::srgba(0.3, 0.3, 0.3, 0.5))
        .border_radius(BorderRadius::MAX)
        .insert(Name::new("Disabled Button"))
        .child(
            bevy_declarative::element::text::text(format!("{label} (Coming Soon)"))
                .font_size(32.0)
                .color(Color::srgba(0.6, 0.6, 0.6, 0.8)),
        )
}

fn open_roster(_: On<Pointer<Click>>, mut next_tab: ResMut<NextState<GameTab>>) {
    next_tab.set(GameTab::Roster);
}
