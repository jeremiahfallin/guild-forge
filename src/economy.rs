//! Gold and economy tracking.

use bevy::prelude::*;
use crate::screens::Screen;

pub(super) fn plugin(app: &mut App) {
    app.add_systems(OnEnter(Screen::Gameplay), init_gold);
}

/// The guild's gold reserves.
#[derive(Resource, Debug, Default)]
pub struct Gold(pub u32);

fn init_gold(mut commands: Commands, existing: Option<Res<Gold>>) {
    if existing.is_none() {
        commands.insert_resource(Gold(0));
    }
}
