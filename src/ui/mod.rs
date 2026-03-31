//! Game UI systems: toast notifications, overlays.

pub mod toast;

use bevy::prelude::*;

pub(super) fn plugin(app: &mut App) {
    app.add_plugins(toast::plugin);
}
