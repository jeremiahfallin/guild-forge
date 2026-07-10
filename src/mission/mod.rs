//! Mission system: data, dungeon generation, simulation, AI, and combat.

pub mod ai;
pub mod combat;
pub mod data;
pub mod dungeon;
pub mod entities;
pub mod pathfinding;
pub mod sequential;
pub mod tileset;

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::screens::GameTab;



pub(super) fn plugin(app: &mut App) {
    app.register_type::<entities::MoveRange>();
    app.register_type::<entities::LootChest>();
    app.register_type::<entities::MissionTurnQueue>();
    app.register_type::<entities::ActiveAbilities>();
    app.register_type::<entities::ActiveAbilityState>();
    app.register_type::<crate::hero::data::AbilityDef>();
    app.register_type::<crate::hero::data::AbilityEffect>();
    app.register_type::<crate::hero::data::AiPriorityRule>();
    app.register_type::<crate::ui::feed::MissionLogHistory>();
    app.register_type::<crate::ui::feed::MissionLogEntry>();
    app.register_type::<crate::ui::feed::LogKind>();
    app.register_type::<MissionInfo>();
    app.register_type::<data::MissionModifier>();
    app.register_type::<entities::TelegraphedAttack>();
    app.register_type::<entities::Enraged>();
    app.register_type::<entities::MissionEventsState>();
    app.register_type::<data::EventType>();
    app.register_type::<data::EventCheckStat>();
    app.register_type::<data::BiomeType>();
    app.register_type::<data::EnemyFamily>();
    app.register_type::<RescueMission>();

    app.add_systems(Startup, (data::load_mission_databases, tileset::load_sprites));

    // Fixed simulation tick at 2 Hz. Time<Virtual>::relative_speed scales this
    // naturally for a future "speed up" control.
    app.insert_resource(Time::<Fixed>::from_hz(2.0));

    // All mission simulation runs in FixedUpdate, fully independent of the
    // mission view. Each system iterates missions via `Children` so cost
    // scales with live missions, not total tokens.
    
    // Common systems
    app.add_systems(
        FixedUpdate,
        (
            combat::handle_death_system,
            combat::update_room_status,
            combat::check_mission_completion,
            crate::hero::track_hero_history_system,
            drain_mission_fatigue,
        )
            .chain()
            .run_if(in_state(crate::screens::Screen::Gameplay)),
    );

    // Sequential (Turn-Based) Movement & Combat
    app.add_systems(
        FixedUpdate,
        (
            sequential::build_or_update_turn_queue,
            sequential::process_sequential_turn,
        )
            .chain()
            .run_if(in_state(crate::screens::Screen::Gameplay)),
    );


    // Proxy sync only runs while viewing a mission.
    app.add_systems(
        Update,
        (
            entities::sync_proxy_visuals,
            entities::cleanup_orphaned_proxies,
        )
            .chain()
            .run_if(in_state(GameTab::MissionView)),
    );

    // Dynamic simulation tempo updates
    app.add_systems(
        Update,
        update_simulation_tempo
            .run_if(in_state(crate::screens::Screen::Gameplay)),
    );

    // Sprite animation only runs when viewing a mission.
    app.add_systems(
        Update,
        tileset::animate_sprites.run_if(in_state(GameTab::MissionView)),
    );
}

/// Marker component for mission entities.
#[derive(Component, Debug, Reflect)]
#[reflect(Component)]
pub struct Mission;

/// Core mission information.
#[derive(Component, Debug, Reflect)]
#[reflect(Component)]
pub struct MissionInfo {
    pub template_id: String,
    pub name: String,
    pub difficulty: u32,
    pub modifiers: Vec<data::MissionModifier>,
    pub biome: data::BiomeType,
}

/// The current state of a mission.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Reflect)]
#[reflect(Component)]
pub enum MissionProgress {
    InProgress,
    Complete,
    Failed,
}

/// Missions currently underway — the number the War Room cap is checked
/// against. Resolved missions despawn, but count only `InProgress` to be safe
/// against same-frame resolution.
pub fn active_mission_count(missions: &Query<&MissionProgress, With<Mission>>) -> usize {
    missions
        .iter()
        .filter(|p| **p == MissionProgress::InProgress)
        .count()
}

/// Action range in tiles for combat-overlap checks (encounter enrollment,
/// tempo, music, boss banner). Single source of truth.
pub fn hero_action_range(class: &crate::hero::data::HeroClass) -> u32 {
    match class {
        crate::hero::data::HeroClass::Ranger => 6,
        crate::hero::data::HeroClass::Mage => 5,
        _ => 1,
    }
}

/// True when any hero's action range reaches any enemy (Manhattan distance).
pub fn combat_overlap(
    heroes: &[(entities::GridPosition, u32)],
    enemies: &[entities::GridPosition],
) -> bool {
    heroes.iter().any(|(h_gp, h_range)| {
        enemies
            .iter()
            .any(|e_gp| h_gp.x.abs_diff(e_gp.x) + h_gp.y.abs_diff(e_gp.y) <= *h_range)
    })
}

/// The heroes assigned to a mission.
#[derive(Component, Debug, Reflect)]
#[reflect(Component)]
pub struct MissionParty(pub Vec<Entity>);

/// Marks a hero as currently on a mission. Stores the mission entity.
#[derive(Component, Debug, Reflect)]
#[reflect(Component)]
pub struct OnMission(pub Entity);

/// Tracks which heroes are being rescued by this running mission.
#[derive(Component, Debug, Clone, Reflect)]
#[reflect(Component)]
pub struct RescueMission {
    pub rescue_heroes: Vec<Entity>,
    pub gear_recovered: bool,
}

/// Stores the generated dungeon map on the mission entity. Authoritative
/// source of truth for the dungeon — read by sim systems and by the view
/// layer when rendering.
#[derive(Component, Debug, Clone, Reflect)]
#[reflect(Component)]
pub struct MissionDungeon(pub dungeon::DungeonMap);

/// Tracks which mission entity is currently being viewed in the MissionView.
#[derive(Resource, Debug)]
pub struct ViewedMission(pub Entity);

fn drain_mission_fatigue(
    time: Res<Time<Virtual>>,
    mut query: Query<&mut crate::hero::Fatigue, With<OnMission>>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    // Drain rate: 0.2 stamina points per virtual second
    let drain_amount = 0.2 * dt;
    for mut fatigue in &mut query {
        if fatigue.current > 0.0 {
            fatigue.current = (fatigue.current - drain_amount).max(0.0);
        }
    }
}

pub(crate) fn update_simulation_tempo(
    viewed_mission: Option<Res<ViewedMission>>,
    missions: Query<&Children, With<Mission>>,
    hero_tokens: Query<(&entities::GridPosition, &entities::HeroToken), Without<entities::EnemyToken>>,
    enemy_tokens: Query<&entities::GridPosition, With<entities::EnemyToken>>,
    hero_data: Query<&crate::hero::HeroInfo, With<crate::hero::Hero>>,
    mut time_fixed: ResMut<Time<Fixed>>,
) {

    let mut in_combat = false;

    if let Some(viewed) = viewed_mission
        && let Ok(children) = missions.get(viewed.0) {
            // Check if any hero and enemy action ranges overlap
            let mut heroes_list = Vec::new();
            for &child in children {
                if let Ok((gp, hero_token)) = hero_tokens.get(child)
                    && let Ok(info) = hero_data.get(hero_token.0) {
                        heroes_list.push((*gp, hero_action_range(&info.class)));
                    }
            }

            let mut enemies_list = Vec::new();
            for &child in children {
                if let Ok(gp) = enemy_tokens.get(child) {
                    enemies_list.push(*gp);
                }
            }

            in_combat = combat_overlap(&heroes_list, &enemies_list);
        }

    let target_hz = if in_combat {
        1.5 // Combat is slow (1-2 Hz)
    } else {
        6.0 // Exploration is fast (4-8 Hz)
    };

    time_fixed.set_timestep(std::time::Duration::from_secs_f32(1.0 / target_hz));
}

#[cfg(test)]
mod overlap_tests {
    use super::*;
    use crate::hero::data::HeroClass;
    use entities::GridPosition;

    #[test]
    fn action_ranges_by_class() {
        assert_eq!(hero_action_range(&HeroClass::Ranger), 6);
        assert_eq!(hero_action_range(&HeroClass::Mage), 5);
        assert_eq!(hero_action_range(&HeroClass::Warrior), 1);
    }

    #[test]
    fn combat_overlap_boundary() {
        let heroes = [(GridPosition { x: 0, y: 0 }, 2)];
        // Manhattan distance exactly == range: overlap
        assert!(combat_overlap(&heroes, &[GridPosition { x: 1, y: 1 }]));
        // One further: no overlap
        assert!(!combat_overlap(&heroes, &[GridPosition { x: 2, y: 1 }]));
        // Empty lists: no combat
        assert!(!combat_overlap(&heroes, &[]));
        assert!(!combat_overlap(&[], &[GridPosition { x: 0, y: 0 }]));
    }
}

#[cfg(test)]
mod cap_tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;

    #[test]
    fn active_mission_count_ignores_resolved() {
        let mut world = World::new();
        world.spawn((Mission, MissionProgress::InProgress));
        world.spawn((Mission, MissionProgress::InProgress));
        world.spawn((Mission, MissionProgress::Complete));
        world.spawn((Mission, MissionProgress::Failed));
        world.spawn(MissionProgress::InProgress); // no Mission marker - ignored

        let count = world
            .run_system_once(|q: Query<&MissionProgress, With<Mission>>| {
                active_mission_count(&q)
            })
            .unwrap();
        assert_eq!(count, 2);
    }
}
