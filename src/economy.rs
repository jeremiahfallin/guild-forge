//! Gold and economy tracking.

use bevy::prelude::*;

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<Gold>();
}

/// The guild's gold reserves.
#[derive(Resource, Debug, Default)]
pub struct Gold(pub u32);
