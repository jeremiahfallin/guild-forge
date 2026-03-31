use bevy::color::Color;
use bevy::ecs::hierarchy::ChildSpawnerCommands;
use bevy::ecs::system::{EntityCommands, IntoObserverSystem};
use bevy::picking::events::{Click, Out, Over, Press, Release, Pointer};
use bevy::prelude::{Bundle, Commands};
use bevy::ui::{BackgroundColor, BorderRadius, Node, Val};

use crate::element::Element;
use crate::style::styled::Styled;

/// A container UI element, analogous to an HTML `<div>`.
pub struct Div {
    pub(crate) node: Node,
    pub(crate) bg: Option<Color>,
    pub(crate) border_radius: Option<BorderRadius>,
    pub(crate) children: Vec<Box<dyn Element>>,
    pub(crate) observers: Vec<Box<dyn FnOnce(&mut EntityCommands) + Send + Sync>>,
}

impl Div {
    pub fn new() -> Self {
        Self {
            node: Node::default(),
            bg: None,
            border_radius: None,
            children: Vec::new(),
            observers: Vec::new(),
        }
    }

    pub fn bg(mut self, color: Color) -> Self {
        self.bg = Some(color);
        self
    }

    pub fn rounded(mut self, val: Val) -> Self {
        self.border_radius = Some(BorderRadius::all(val));
        self
    }

    pub fn border_radius(mut self, radius: BorderRadius) -> Self {
        self.border_radius = Some(radius);
        self
    }

    pub fn child(mut self, child: impl Element + 'static) -> Self {
        self.children.push(Box::new(child));
        self
    }

    /// Insert an arbitrary bundle of components onto this element's entity.
    pub fn insert(mut self, bundle: impl Bundle + Send + Sync + 'static) -> Self {
        self.observers.push(Box::new(move |ec: &mut EntityCommands| {
            ec.insert(bundle);
        }));
        self
    }

    pub fn on_click<B: Bundle, M>(mut self, callback: impl IntoObserverSystem<Pointer<Click>, B, M> + Send + Sync + 'static) -> Self {
        self.observers.push(Box::new(move |ec: &mut EntityCommands| {
            ec.observe(callback);
        }));
        self
    }

    pub fn on_hover<B: Bundle, M>(mut self, callback: impl IntoObserverSystem<Pointer<Over>, B, M> + Send + Sync + 'static) -> Self {
        self.observers.push(Box::new(move |ec: &mut EntityCommands| {
            ec.observe(callback);
        }));
        self
    }

    pub fn on_hover_out<B: Bundle, M>(mut self, callback: impl IntoObserverSystem<Pointer<Out>, B, M> + Send + Sync + 'static) -> Self {
        self.observers.push(Box::new(move |ec: &mut EntityCommands| {
            ec.observe(callback);
        }));
        self
    }

    pub fn on_press<B: Bundle, M>(mut self, callback: impl IntoObserverSystem<Pointer<Press>, B, M> + Send + Sync + 'static) -> Self {
        self.observers.push(Box::new(move |ec: &mut EntityCommands| {
            ec.observe(callback);
        }));
        self
    }

    pub fn on_release<B: Bundle, M>(mut self, callback: impl IntoObserverSystem<Pointer<Release>, B, M> + Send + Sync + 'static) -> Self {
        self.observers.push(Box::new(move |ec: &mut EntityCommands| {
            ec.observe(callback);
        }));
        self
    }

    pub fn spawn<'a>(self, commands: &'a mut Commands) -> EntityCommands<'a> {
        let mut node = self.node;
        if let Some(radius) = self.border_radius {
            node.border_radius = radius;
        }

        let children = self.children;
        let observers = self.observers;

        let mut ec = if let Some(color) = self.bg {
            commands.spawn((node, BackgroundColor(color)))
        } else {
            commands.spawn(node)
        };

        if !children.is_empty() {
            ec.with_children(|parent| {
                for child in children {
                    child.spawn_with_parent(parent);
                }
            });
        }

        for observer in observers {
            observer(&mut ec);
        }

        ec
    }

    /// Spawn this element as a child of an existing entity.
    pub fn spawn_as_child_of(self, commands: &mut Commands, parent: bevy::prelude::Entity) -> bevy::prelude::Entity {
        let child = self.spawn(commands).id();
        commands.entity(parent).add_child(child);
        child
    }
}

impl Default for Div {
    fn default() -> Self {
        Self::new()
    }
}

impl Styled for Div {
    fn style_mut(&mut self) -> &mut Node {
        &mut self.node
    }
}

impl Element for Div {
    fn spawn_with_parent(self: Box<Self>, parent: &mut ChildSpawnerCommands) {
        let mut node = self.node;
        if let Some(radius) = self.border_radius {
            node.border_radius = radius;
        }

        let children = self.children;
        let observers = self.observers;

        let mut ec = if let Some(color) = self.bg {
            parent.spawn((node, BackgroundColor(color)))
        } else {
            parent.spawn(node)
        };

        if !children.is_empty() {
            ec.with_children(|p| {
                for child in children {
                    child.spawn_with_parent(p);
                }
            });
        }

        for observer in observers {
            observer(&mut ec);
        }
    }
}

/// Creates a new `Div` with default settings.
pub fn div() -> Div {
    Div::new()
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ui::{Display, FlexDirection, Val};

    #[test]
    fn div_default_creates_default_node() {
        let d = div();
        assert_eq!(d.node.display, Display::Flex);
    }

    #[test]
    fn div_styled_chaining() {
        let d = div().flex().col().gap_2().p_4();
        assert_eq!(d.node.flex_direction, FlexDirection::Column);
        assert_eq!(d.node.row_gap, Val::Px(8.0));
        assert_eq!(d.node.padding.top, Val::Px(16.0));
    }

    #[test]
    fn div_bg_sets_color() {
        let d = div().bg(Color::srgb(1.0, 0.0, 0.0));
        assert!(d.bg.is_some());
    }

    #[test]
    fn div_child_adds_children() {
        let d = div().child(div()).child(div());
        assert_eq!(d.children.len(), 2);
    }
}
