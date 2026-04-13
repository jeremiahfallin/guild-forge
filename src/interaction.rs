//! Interaction palette — automatic background color changes on hover/press/release.

use bevy::color::Color;
use bevy::prelude::*;

/// Color palette for interactive UI elements. Attach to any entity with pointer
/// events to automatically update its [`BackgroundColor`] on hover, press, and release.
#[derive(Component, Debug, Reflect)]
#[reflect(Component)]
pub struct InteractionPalette {
    pub none: Color,
    pub hovered: Color,
    pub pressed: Color,
}

pub(crate) fn plugin(app: &mut App) {
    app.add_observer(on_click);
    app.add_observer(on_release);
    app.add_observer(on_over);
    app.add_observer(on_out);
}

fn on_click(
    click: On<Pointer<Click>>,
    mut q: Query<(&InteractionPalette, &mut BackgroundColor)>,
) {
    if let Ok((palette, mut bg)) = q.get_mut(click.event_target()) {
        *bg = palette.pressed.into();
    }
}

fn on_release(
    release: On<Pointer<Release>>,
    mut q: Query<(&InteractionPalette, &mut BackgroundColor)>,
) {
    if let Ok((palette, mut bg)) = q.get_mut(release.event_target()) {
        *bg = palette.hovered.into();
    }
}

fn on_over(
    over: On<Pointer<Over>>,
    mut q: Query<(&InteractionPalette, &mut BackgroundColor)>,
) {
    if let Ok((palette, mut bg)) = q.get_mut(over.event_target()) {
        *bg = palette.hovered.into();
    }
}

fn on_out(
    out: On<Pointer<Out>>,
    mut q: Query<(&InteractionPalette, &mut BackgroundColor)>,
) {
    if let Ok((palette, mut bg)) = q.get_mut(out.event_target()) {
        *bg = palette.none.into();
    }
}
