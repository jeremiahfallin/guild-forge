//! Hero status helpers: Missing / Injured lifecycle constants and pure
//! formatters used by both the tick system and the roster UI.

use bevy::prelude::*;

/// How long (in game-seconds) a hero stays Missing before returning Injured.
pub const MISSING_DURATION_SECS: f64 = 120.0;
/// How long (in game-seconds) the Injured stat penalty persists after return.
pub const INJURED_DURATION_SECS: f64 = 300.0;
/// Multiplier applied to STR/DEX/CON while Injured.
pub const INJURED_STAT_MULTIPLIER: f32 = 0.75;

/// Marks a hero as absent after a mission wipe. `expires_at` is in the
/// `Time<Virtual>` elapsed-seconds frame.
#[derive(Component, Debug, Clone, Copy, Reflect)]
#[reflect(Component)]
pub struct Missing {
    pub expires_at: f64,
}

/// Temporary stat-penalty state applied when a Missing hero returns.
#[derive(Component, Debug, Clone, Copy, Reflect)]
#[reflect(Component)]
pub struct Injured {
    pub expires_at: f64,
}

/// Format a remaining-seconds value as `m:ss`. Negative / zero → `"0:00"`.
pub fn format_countdown(remaining_secs: f64) -> String {
    let total = remaining_secs.max(0.0).ceil() as u64;
    let minutes = total / 60;
    let seconds = total % 60;
    format!("{minutes}:{seconds:02}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_countdown_formats_whole_minutes() {
        assert_eq!(format_countdown(120.0), "2:00");
    }

    #[test]
    fn format_countdown_pads_seconds() {
        assert_eq!(format_countdown(65.0), "1:05");
    }

    #[test]
    fn format_countdown_rounds_up_partial_second() {
        assert_eq!(format_countdown(59.2), "1:00");
    }

    #[test]
    fn format_countdown_clamps_negative_to_zero() {
        assert_eq!(format_countdown(-3.0), "0:00");
        assert_eq!(format_countdown(0.0), "0:00");
    }
}
