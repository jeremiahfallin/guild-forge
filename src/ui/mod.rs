//! Game UI systems: toast notifications, overlays.

pub mod banner;
pub mod toast;
pub mod feed;

use bevy::prelude::*;

pub(super) fn plugin(app: &mut App) {
    app.add_plugins(toast::plugin);
    app.add_plugins(feed::plugin);
    app.add_plugins(banner::plugin);
}
