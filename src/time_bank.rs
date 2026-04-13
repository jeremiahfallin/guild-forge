//! Offline time bank and game speed control.

use bevy::prelude::*;

/// Banked offline seconds, capped at 86,400 (24 hours).
#[derive(Resource, Debug)]
pub struct OfflineTimeBank {
    pub banked_seconds: f32,
}

impl Default for OfflineTimeBank {
    fn default() -> Self {
        Self { banked_seconds: 0.0 }
    }
}

/// Current game speed multiplier (1.0, 2.0, or 3.0).
#[derive(Resource, Debug)]
pub struct GameSpeed(pub f32);

impl Default for GameSpeed {
    fn default() -> Self {
        Self(1.0)
    }
}

/// Format banked seconds as a human-readable string.
/// Under 1 hour: "Xm Ys", otherwise "Xh Ym".
pub fn format_banked_time(seconds: f32) -> String {
    let total_secs = seconds as u32;
    let hours = total_secs / 3600;
    let minutes = (total_secs % 3600) / 60;
    let secs = total_secs % 60;

    if hours > 0 {
        format!("{}h {}m", hours, minutes)
    } else {
        format!("{}m {}s", minutes, secs)
    }
}

/// Drain the offline bank when speed > 1x.
/// Uses `Time<Real>` so drain is based on wall-clock time, not virtual time.
fn tick_offline_bank(
    real_time: Res<Time<Real>>,
    mut bank: ResMut<OfflineTimeBank>,
    mut game_speed: ResMut<GameSpeed>,
    mut virtual_time: ResMut<Time<Virtual>>,
) {
    let drain_rate = game_speed.0 - 1.0;
    if drain_rate <= 0.0 {
        return;
    }

    let dt = real_time.delta_secs();
    bank.banked_seconds -= drain_rate * dt;

    if bank.banked_seconds <= 0.0 {
        bank.banked_seconds = 0.0;
        game_speed.0 = 1.0;
        virtual_time.set_relative_speed(1.0);
    }
}

/// Event: request to change game speed.
#[derive(Event)]
pub struct SetGameSpeed(pub f32);

/// Observer handler for speed change requests.
fn handle_set_speed(
    trigger: On<SetGameSpeed>,
    bank: Res<OfflineTimeBank>,
    mut game_speed: ResMut<GameSpeed>,
    mut virtual_time: ResMut<Time<Virtual>>,
) {
    let requested = trigger.event().0;

    // Can't go above 1x with no banked time
    if requested > 1.0 && bank.banked_seconds <= 0.0 {
        return;
    }

    game_speed.0 = requested;
    virtual_time.set_relative_speed(requested);
}

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<OfflineTimeBank>();
    app.init_resource::<GameSpeed>();
    app.add_observer(handle_set_speed);
    app.add_systems(
        Update,
        tick_offline_bank.run_if(in_state(crate::screens::Screen::Gameplay)),
    );
}
