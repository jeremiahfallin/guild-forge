//! Per-frame lifecycle for Missing and Injured components.
//!
//! Runs in `Update` and reads `Time<Virtual>` so that `GameSpeed` scaling
//! and the offline time bank both flow through naturally. Transitions:
//! Missing expires → insert Injured (return toast) → Injured expires →
//! component removed silently.

use bevy::prelude::*;

use super::status::{Injured, Missing, INJURED_DURATION_SECS};
use super::{Favorite, HeroInfo};
use crate::screens::Screen;
use crate::ui::toast::{ToastEvent, ToastKind};

pub(super) fn plugin(app: &mut App) {
    app.add_systems(
        Update,
        (tick_missing, tick_injured)
            .chain()
            .run_if(in_state(Screen::Gameplay)),
    );
}

fn tick_missing(
    mut commands: Commands,
    time: Res<Time<Virtual>>,
    q: Query<(Entity, &Missing, &HeroInfo, Has<Favorite>)>,
) {
    let now = time.elapsed_secs_f64();
    for (entity, missing, info, is_favorite) in &q {
        if now < missing.expires_at {
            continue;
        }
        commands
            .entity(entity)
            .remove::<Missing>()
            .insert(Injured { expires_at: now + INJURED_DURATION_SECS });

        let kind = if is_favorite { ToastKind::Success } else { ToastKind::Info };
        commands.trigger(ToastEvent {
            title: format!("{} has returned", info.name),
            body: "Injured — stats reduced while they recover.".to_string(),
            kind,
        });
    }
}

fn tick_injured(
    mut commands: Commands,
    time: Res<Time<Virtual>>,
    q: Query<(Entity, &Injured)>,
) {
    let now = time.elapsed_secs_f64();
    for (entity, injured) in &q {
        if now >= injured.expires_at {
            commands.entity(entity).remove::<Injured>();
        }
    }
}
