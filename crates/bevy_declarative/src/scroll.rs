//! Scroll input handling — converts mouse wheel events into ScrollPosition updates
//! for any UI node with `OverflowAxis::Scroll`.

use bevy::{
    input::{
        ButtonInput,
        keyboard::KeyCode,
        mouse::{MouseScrollUnit, MouseWheel},
    },
    picking::hover::HoverMap,
    prelude::*,
};

const LINE_HEIGHT: f32 = 21.0;

pub(crate) fn plugin(app: &mut App) {
    app.add_systems(Update, send_scroll_events);
    app.add_observer(on_scroll);
}

#[derive(EntityEvent, Debug)]
#[entity_event(propagate, auto_propagate)]
struct Scroll {
    entity: Entity,
    delta: Vec2,
}

fn send_scroll_events(
    mut mouse_wheel: MessageReader<MouseWheel>,
    hover_map: Res<HoverMap>,
    keys: Res<ButtonInput<KeyCode>>,
    mut commands: Commands,
) {
    let shift_held = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    for event in mouse_wheel.read() {
        let mut delta = -Vec2::new(event.x, event.y);
        if event.unit == MouseScrollUnit::Line {
            delta *= LINE_HEIGHT;
        }
        // Shift+wheel scrolls horizontally — standard pattern for any scroll
        // container. Convert any Y delta into X here; if delta already has X
        // (horizontal wheel/touchpad gesture) leave it alone.
        if shift_held && delta.x == 0.0 && delta.y != 0.0 {
            delta.x = delta.y;
            delta.y = 0.0;
        }
        for pointer_map in hover_map.values() {
            for &entity in pointer_map.keys() {
                commands.trigger(Scroll { entity, delta });
            }
        }
    }
}

fn on_scroll(
    mut scroll: On<Scroll>,
    mut query: Query<(&mut ScrollPosition, &Node, &ComputedNode)>,
) {
    let Ok((mut scroll_position, node, computed)) = query.get_mut(scroll.entity) else {
        return;
    };

    let max_offset = (computed.content_size() - computed.size()) * computed.inverse_scale_factor();
    let delta = &mut scroll.delta;

    // When this node only has horizontal overflow to consume but the wheel
    // delivered a Y delta (the normal case for a vertical mouse wheel),
    // redirect Y → X so a horizontal-only container is still reachable.
    let scrolls_x = node.overflow.x == OverflowAxis::Scroll && max_offset.x > 0.0;
    let consumes_y = node.overflow.y == OverflowAxis::Scroll && max_offset.y > 0.0;
    if scrolls_x && !consumes_y && delta.x == 0.0 && delta.y != 0.0 {
        delta.x = delta.y;
        delta.y = 0.0;
    }

    if node.overflow.x == OverflowAxis::Scroll && delta.x != 0.0 && max_offset.x > 0.0 {
        let new_x = (scroll_position.x + delta.x).clamp(0.0, max_offset.x);
        if new_x != scroll_position.x {
            scroll_position.x = new_x;
            delta.x = 0.0;
        }
    }

    if node.overflow.y == OverflowAxis::Scroll && delta.y != 0.0 && max_offset.y > 0.0 {
        let new_y = (scroll_position.y + delta.y).clamp(0.0, max_offset.y);
        if new_y != scroll_position.y {
            scroll_position.y = new_y;
            delta.y = 0.0;
        }
    }

    if *delta == Vec2::ZERO {
        scroll.propagate(false);
    }
}
