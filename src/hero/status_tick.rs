//! Per-frame lifecycle for Missing and Injured components.
//!
//! Runs in `Update` and reads `Time<Virtual>` so that `GameSpeed` scaling
//! and the offline time bank both flow through naturally. Transitions:
//! Missing expires → insert Injured (return toast) → Injured expires →
//! component removed silently.

use bevy::prelude::*;

use super::status::{Injured, Missing, INJURED_DURATION_SECS};
use super::{Favorite, HeroInfo, Fatigue, Hero, HeroStats, Epithet, format_hero_name};
use crate::buildings::{BuildingType, GuildBuildings};
use crate::mission::OnMission;
use crate::screens::Screen;
use crate::ui::toast::{ToastEvent, ToastKind};

/// System set that contains the per-frame Missing/Injured lifecycle ticks.
///
/// UI systems (e.g. the roster rebuild) should run `.after(StatusTickSet)`
/// so that on the frame a status flips, the rebuild observes the new state
/// rather than the just-removed component.
#[derive(SystemSet, Debug, Hash, PartialEq, Eq, Clone)]
pub struct StatusTickSet;

pub(super) fn plugin(app: &mut App) {
    app.add_systems(
        Update,
        (tick_missing, tick_injured, tick_stamina_recovery)
            .chain()
            .in_set(StatusTickSet)
            .run_if(in_state(Screen::Gameplay)),
    );
}

pub(crate) fn tick_missing(
    mut commands: Commands,
    time: Res<Time<Virtual>>,
    q: Query<(Entity, &Missing, &HeroInfo, Has<Favorite>, Option<&Epithet>)>,
    board: Option<ResMut<crate::screens::missions::MissionBoard>>,
    rescue_missions: Query<&crate::mission::RescueMission>,
    mut histories: Query<&mut crate::hero::history::HeroHistory>,
) {
    let now = time.elapsed_secs_f64();

    // Clean up expired rescue offers
    if let Some(mut b) = board {
        b.rescue_offers.retain(|offer| now < offer.expires_at);
    }

    // Gather heroes currently on active rescue missions
    let mut being_rescued = std::collections::HashSet::new();
    for rm in &rescue_missions {
        for &h in &rm.rescue_heroes {
            being_rescued.insert(h);
        }
    }

    for (entity, missing, info, is_favorite, epithet) in &q {
        if being_rescued.contains(&entity) {
            continue;
        }
        if now < missing.expires_at {
            continue;
        }
        commands
            .entity(entity)
            .remove::<Missing>()
            .insert(Injured { expires_at: now + INJURED_DURATION_SECS });

        if let Ok(mut hist) = histories.get_mut(entity) {
            hist.add_timeline_entry(format!("Returned after being missing (gear lost)"));
        }

        let kind = if is_favorite { ToastKind::Success } else { ToastKind::Info };
        commands.trigger(ToastEvent {
            title: format!("{} has returned", format_hero_name(&info.name, epithet)),
            body: "Injured — stats reduced while they recover.".to_string(),
            kind,
            action: None,
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

fn tick_stamina_recovery(
    time: Res<Time<Virtual>>,
    buildings: Res<GuildBuildings>,
    mut query: Query<(&HeroInfo, &HeroStats, &mut Fatigue), (With<Hero>, Without<OnMission>, Without<Missing>)>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    let tavern_level = buildings.level(BuildingType::Tavern);
    let rate_multiplier = 1.0 + (tavern_level as f32 * 0.1);
    let recovery_amount = 0.5 * rate_multiplier * dt;

    for (info, stats, mut fatigue) in &mut query {
        let max = fatigue.max(info.level, stats.constitution);
        if fatigue.current < max {
            fatigue.current = (fatigue.current + recovery_amount).min(max);
        }
    }
}
