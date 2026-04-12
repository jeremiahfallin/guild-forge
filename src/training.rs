use bevy::prelude::*;

use crate::buildings::{BuildingType, GuildBuildings};
use crate::hero::{Hero, HeroInfo};
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
    mut heroes: Query<&mut HeroInfo, (With<Hero>, Without<OnMission>)>,
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

    for mut info in &mut heroes {
        info.xp += xp_per_tick;
        while info.xp >= info.xp_to_next {
            info.xp -= info.xp_to_next;
            info.level += 1;
            info.xp_to_next = (info.xp_to_next as f32 * 1.5) as u32;
        }
    }
}

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<TrainingTimer>();
    app.add_systems(Update, tick_training);
}
