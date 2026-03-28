use bevy::ecs::hierarchy::ChildSpawnerCommands;

pub mod div;
pub mod text;

/// Shared interface for things that can be spawned as children.
pub trait Element {
    fn spawn_with_parent(self: Box<Self>, parent: &mut ChildSpawnerCommands);
}

pub use div::{div, Div};
pub use text::{text, TextEl};
