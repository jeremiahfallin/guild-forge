use bevy::color::Color;
use bevy::ecs::hierarchy::ChildSpawnerCommands;
use bevy::ecs::system::EntityCommands;
use bevy::prelude::{Bundle, Commands, Text};
use bevy::text::{TextColor, TextFont};
use bevy::ui::Node;

use crate::element::Element;
use crate::style::styled::Styled;

/// A text UI element.
pub struct TextEl {
    pub(crate) node: Node,
    pub(crate) content: String,
    pub(crate) color: Option<Color>,
    pub(crate) font_size: Option<f32>,
    pub(crate) insertions: Vec<Box<dyn FnOnce(&mut EntityCommands) + Send + Sync>>,
}

impl TextEl {
    pub fn new(content: impl Into<String>) -> Self {
        Self {
            node: Node::default(),
            content: content.into(),
            color: None,
            font_size: None,
            insertions: Vec::new(),
        }
    }

    /// Insert an arbitrary bundle of components onto this element's entity.
    pub fn insert(mut self, bundle: impl Bundle + Send + Sync + 'static) -> Self {
        self.insertions.push(Box::new(move |ec: &mut EntityCommands| {
            ec.insert(bundle);
        }));
        self
    }

    pub fn color(mut self, color: Color) -> Self {
        self.color = Some(color);
        self
    }

    pub fn font_size(mut self, size: f32) -> Self {
        self.font_size = Some(size);
        self
    }

    // Tailwind text size presets
    pub fn text_xs(self) -> Self { self.font_size(12.0) }
    pub fn text_sm(self) -> Self { self.font_size(14.0) }
    pub fn text_base(self) -> Self { self.font_size(16.0) }
    pub fn text_lg(self) -> Self { self.font_size(18.0) }
    pub fn text_xl(self) -> Self { self.font_size(20.0) }
    pub fn text_2xl(self) -> Self { self.font_size(24.0) }
    pub fn text_3xl(self) -> Self { self.font_size(30.0) }
    pub fn text_4xl(self) -> Self { self.font_size(36.0) }
    pub fn text_5xl(self) -> Self { self.font_size(48.0) }
    pub fn text_6xl(self) -> Self { self.font_size(60.0) }

    pub fn spawn<'a>(self, commands: &'a mut Commands) -> EntityCommands<'a> {
        let text = Text::new(self.content);
        let mut ec = commands.spawn((self.node, text));

        if let Some(color) = self.color {
            ec.insert(TextColor(color));
        }

        if let Some(size) = self.font_size {
            ec.insert(TextFont {
                font_size: size,
                ..Default::default()
            });
        }

        for insertion in self.insertions {
            insertion(&mut ec);
        }

        ec
    }
}

impl Styled for TextEl {
    fn style_mut(&mut self) -> &mut Node {
        &mut self.node
    }
}

impl Element for TextEl {
    fn spawn_with_parent(self: Box<Self>, parent: &mut ChildSpawnerCommands) {
        let text = Text::new(self.content);
        let mut ec = parent.spawn((self.node, text));

        if let Some(color) = self.color {
            ec.insert(TextColor(color));
        }

        if let Some(size) = self.font_size {
            ec.insert(TextFont {
                font_size: size,
                ..Default::default()
            });
        }

        for insertion in self.insertions {
            insertion(&mut ec);
        }
    }
}

/// Creates a new `TextEl` with the given content.
pub fn text(content: impl Into<String>) -> TextEl {
    TextEl::new(content)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ui::Val;

    #[test]
    fn text_stores_content() {
        let t = text("Hello");
        assert_eq!(t.content, "Hello");
    }

    #[test]
    fn text_color_sets_color() {
        let t = text("Hi").color(Color::srgb(1.0, 0.0, 0.0));
        assert!(t.color.is_some());
    }

    #[test]
    fn text_lg_sets_font_size() {
        let t = text("Hi").text_lg();
        assert_eq!(t.font_size, Some(18.0));
    }

    #[test]
    fn text_styled_chaining() {
        let t = text("Hi").text_lg().p_2();
        assert_eq!(t.font_size, Some(18.0));
        assert_eq!(t.node.padding.top, Val::Px(8.0));
    }
}
