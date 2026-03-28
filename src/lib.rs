pub mod colors;
pub mod element;
pub mod events;
pub mod style;

pub mod prelude {
    pub use crate::colors::*;
    pub use crate::element::div::*;
    pub use crate::element::text::*;
    pub use crate::events::*;
    pub use crate::style::styled::*;
    pub use crate::style::values::*;
}
