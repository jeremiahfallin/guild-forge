pub mod colors;
pub mod element;
pub mod events;
mod interaction;
mod scroll;
pub mod style;

use bevy::prelude::*;

pub use interaction::InteractionPalette;

/// Registers scroll input handling and interaction palette systems.
pub struct BevyDeclarativePlugin;

impl Plugin for BevyDeclarativePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((interaction::plugin, scroll::plugin));
    }
}

pub mod prelude {
    pub use crate::colors::*;
    pub use crate::element::*;
    pub use crate::events::*;
    pub use crate::style::*;
    pub use crate::{BevyDeclarativePlugin, InteractionPalette};
}
