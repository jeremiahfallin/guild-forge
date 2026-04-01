//! Reusable UI widget helpers built on bevy_declarative.

use std::borrow::Cow;

use bevy::ecs::system::IntoObserverSystem;
use bevy::picking::events::{Click, Pointer};
use bevy::prelude::*;
use bevy_declarative::element::div::{Div, div};
use bevy_declarative::element::text::{TextEl, text};
use bevy_declarative::style::styled::Styled;
use bevy_declarative::style::values::px;

use super::interaction::InteractionPalette;
use super::palette::*;

/// A root UI node that fills the window and centers its content.
pub fn ui_root(name: impl Into<Cow<'static, str>>) -> Div {
    div()
        .absolute()
        .w_full()
        .h_full()
        .col()
        .items_center()
        .justify_center()
        .gap(px(20.0))
        .insert((Name::new(name), Pickable::IGNORE))
}

/// Marker for the gameplay root container (sidebar + content area).
#[derive(Component)]
pub struct GameplayRoot;

/// Marker for the sidebar UI so reactive systems can find it.
#[derive(Component)]
pub struct SidebarRoot;

/// Marker for the gold text display in the sidebar.
#[derive(Component)]
pub struct SidebarGoldText;

/// Marker for the active missions container in the sidebar.
#[derive(Component)]
pub struct SidebarMissionList;

/// Marker for a nav button, storing which tab it navigates to.
#[derive(Component)]
pub struct SidebarNavButton(pub crate::screens::GameTab);

/// A content area that fills the right side of the gameplay layout.
/// Screens use this instead of `ui_root()` when the sidebar is present.
pub fn content_area(name: impl Into<Cow<'static, str>>) -> Div {
    div()
        .col()
        .flex_1()
        .h_full()
        .items_center()
        .gap(px(20.0))
        .overflow_y_hidden()
        .insert((Name::new(name), Pickable::IGNORE))
}

/// A simple header label. Bigger than [`label`].
pub fn header(content: impl Into<String>) -> TextEl {
    text(content).font_size(40.0).color(HEADER_TEXT)
}

/// A simple text label.
pub fn label(content: impl Into<String>) -> TextEl {
    text(content).font_size(24.0).color(LABEL_TEXT)
}

/// A small square button with text and an action defined as an observer.
pub fn game_button_small<B: Bundle, M>(
    label: impl Into<String>,
    action: impl IntoObserverSystem<Pointer<Click>, B, M> + Sync + 'static,
) -> Div {
    div()
        .w(px(30.0))
        .h(px(30.0))
        .items_center()
        .justify_center()
        .bg(BUTTON_BACKGROUND)
        .insert((
            Name::new("Button Small"),
            Button,
            InteractionPalette {
                none: BUTTON_BACKGROUND,
                hovered: BUTTON_HOVERED_BACKGROUND,
                pressed: BUTTON_PRESSED_BACKGROUND,
            },
        ))
        .on_click(action)
        .child(
            text(label)
                .font_size(24.0)
                .color(BUTTON_TEXT)
                .insert(Pickable::IGNORE),
        )
}

/// A large rounded button with text and an action defined as an observer.
pub fn game_button<B: Bundle, M>(
    label: impl Into<String>,
    action: impl IntoObserverSystem<Pointer<Click>, B, M> + Sync + 'static,
) -> Div {
    div()
        .w(px(380.0))
        .h(px(80.0))
        .items_center()
        .justify_center()
        .bg(BUTTON_BACKGROUND)
        .border_radius(BorderRadius::MAX)
        .insert((
            Name::new("Button"),
            Button,
            InteractionPalette {
                none: BUTTON_BACKGROUND,
                hovered: BUTTON_HOVERED_BACKGROUND,
                pressed: BUTTON_PRESSED_BACKGROUND,
            },
        ))
        .on_click(action)
        .child(
            text(label)
                .font_size(40.0)
                .color(BUTTON_TEXT)
                .insert(Pickable::IGNORE),
        )
}
