use bevy::prelude::*;

/// Guild reputation. Earned from completing missions, gates recruit quality and mission access.
#[derive(Resource, Debug, Clone, Default, Deref, DerefMut)]
pub struct Reputation(pub u32);

impl Reputation {
    pub fn tier(&self) -> u32 {
        match self.0 {
            0..100 => 1,
            100..300 => 2,
            300..600 => 3,
            600..1000 => 4,
            _ => 5,
        }
    }
}

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<Reputation>();
}
