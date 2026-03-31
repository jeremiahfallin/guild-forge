//! Event-driven toast notification system.
//!
//! Fire a [`ToastEvent`] from any system via `commands.trigger(ToastEvent { .. })`
//! and a styled notification will appear, auto-dismissing after 5 seconds.

use bevy::prelude::*;
use bevy_declarative::element::div::div;
use bevy_declarative::element::text::text;
use bevy_declarative::style::styled::Styled;
use bevy_declarative::style::values::px;

use crate::theme::palette::*;

pub(super) fn plugin(app: &mut App) {
    app.add_observer(on_toast_event);
    app.add_systems(Update, tick_toasts);
}

/// Fire this event via `commands.trigger(ToastEvent { .. })` to show a toast.
#[derive(Event, Debug, Clone)]
pub struct ToastEvent {
    pub title: String,
    pub body: String,
    pub kind: ToastKind,
}

/// Visual style of the toast.
#[derive(Debug, Clone, Copy, Default)]
pub enum ToastKind {
    Success,
    Failure,
    #[default]
    Info,
}

/// Timer component on toast UI nodes — auto-despawns when expired.
#[derive(Component)]
struct ToastTimer(Timer);

/// Marker for the toast container (anchored bottom-right).
#[derive(Component)]
struct ToastContainer;

/// Marker for individual toast nodes.
#[derive(Component)]
struct ToastNode;

fn on_toast_event(
    trigger: On<ToastEvent>,
    mut commands: Commands,
    container_q: Query<Entity, With<ToastContainer>>,
) {
    let event = trigger.event();

    // Ensure container exists
    let container = if let Ok(entity) = container_q.single() {
        entity
    } else {
        commands
            .spawn((
                Name::new("Toast Container"),
                ToastContainer,
                Node {
                    position_type: PositionType::Absolute,
                    right: Val::Px(20.0),
                    bottom: Val::Px(20.0),
                    flex_direction: FlexDirection::ColumnReverse,
                    row_gap: Val::Px(8.0),
                    ..default()
                },
                GlobalZIndex(100),
                Pickable::IGNORE,
            ))
            .id()
    };

    let border_color = match event.kind {
        ToastKind::Success => Color::srgb(0.2, 0.7, 0.3),
        ToastKind::Failure => Color::srgb(0.8, 0.2, 0.2),
        ToastKind::Info => Color::srgb(0.3, 0.5, 0.8),
    };

    let toast = div()
        .col()
        .w(px(320.0))
        .p(px(12.0))
        .gap(px(4.0))
        .bg(Color::srgba(0.1, 0.1, 0.15, 0.9))
        .rounded(px(8.0))
        .insert((
            Name::new("Toast"),
            ToastNode,
            ToastTimer(Timer::from_seconds(5.0, TimerMode::Once)),
            BorderColor {
                top: border_color,
                right: border_color,
                bottom: border_color,
                left: border_color,
            },
            Pickable::IGNORE,
        ))
        .child(text(&event.title).font_size(20.0).color(HEADER_TEXT))
        .child(text(&event.body).font_size(15.0).color(LABEL_TEXT));

    let toast_entity = toast.spawn(&mut commands).id();
    commands.entity(container).add_child(toast_entity);
}

fn tick_toasts(
    mut commands: Commands,
    time: Res<Time>,
    mut toasts: Query<(Entity, &mut ToastTimer), With<ToastNode>>,
    container_q: Query<(Entity, Option<&Children>), With<ToastContainer>>,
) {
    for (entity, mut timer) in &mut toasts {
        timer.0.tick(time.delta());
        if timer.0.just_finished() {
            commands.entity(entity).despawn();
        }
    }

    // Remove container if empty
    for (container, children) in &container_q {
        let is_empty = children.is_none_or(|c| c.is_empty());
        if is_empty {
            commands.entity(container).despawn();
        }
    }
}
