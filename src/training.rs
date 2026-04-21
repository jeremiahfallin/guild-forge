use bevy::prelude::*;

use crate::buildings::{BuildingType, GuildBuildings};
use crate::hero::{Hero, HeroInfo, HeroStats};
use crate::mission::OnMission;

const TRAINING_TICK_INTERVAL: f32 = 60.0;

#[derive(Resource, Debug)]
pub struct TrainingTimer(pub f32);

impl Default for TrainingTimer {
    fn default() -> Self {
        Self(TRAINING_TICK_INTERVAL)
    }
}

fn tick_training(
    time: Res<Time>,
    mut timer: ResMut<TrainingTimer>,
    buildings: Res<GuildBuildings>,
    mut heroes: Query<
        (&mut HeroInfo, &mut HeroStats, &crate::hero::HeroGrowth, &mut crate::hero::HeroStatProgress),
        (With<Hero>, Without<OnMission>),
    >,
) {
    let level = buildings.level(BuildingType::TrainingGrounds);
    if level == 0 {
        return;
    }

    timer.0 -= time.delta_secs();
    if timer.0 > 0.0 {
        return;
    }
    timer.0 = TRAINING_TICK_INTERVAL;

    let xp_per_tick = level * 2;

    for (mut info, mut stats, growth, mut progress) in &mut heroes {
        crate::hero::award_xp(&mut info, &mut stats, growth, &mut progress, xp_per_tick);
    }
}

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<TrainingTimer>();
    app.add_systems(Update, tick_training);
}
