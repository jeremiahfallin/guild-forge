//! Mission system: data, dungeon generation, simulation, AI, and combat.

pub mod ai;
pub mod combat;
pub mod data;
pub mod dungeon;
pub mod entities;
pub mod pathfinding;
pub mod tileset;

use bevy::prelude::*;

use crate::screens::GameTab;

pub(super) fn plugin(app: &mut App) {
    app.add_systems(Startup, (data::load_mission_databases, tileset::load_sprites));
    app.add_systems(OnEnter(GameTab::MissionView), entities::spawn_mission_entities);
    app.add_systems(OnExit(GameTab::MissionView), entities::cleanup_mission_entities);
    // Simulation systems run during all of Gameplay so missions continue in background
    app.add_systems(
        Update,
        (
            entities::simulation_tick,
            ai::hero_ai_system,
            combat::hero_combat_system,
            combat::enemy_combat_system,
            combat::handle_death_system,
            combat::update_room_status,
            combat::check_mission_completion,
        )
            .chain()
            .run_if(in_state(crate::screens::Screen::Gameplay))
            .run_if(resource_exists::<entities::SimulationTimer>),
    );
    // Visual sync only runs when viewing a mission
    app.add_systems(
        Update,
        entities::sync_sprite_positions
            .run_if(in_state(GameTab::MissionView)),
    );
    // Sprite animation only runs when viewing a mission
    app.add_systems(
        Update,
        tileset::animate_sprites
            .run_if(in_state(GameTab::MissionView)),
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
}

/// The current state of a mission.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Reflect)]
#[reflect(Component)]
pub enum MissionProgress {
    InProgress,
    Complete,
    Failed,
}

/// The heroes assigned to a mission.
#[derive(Component, Debug, Reflect)]
#[reflect(Component)]
pub struct MissionParty(pub Vec<Entity>);

/// Marks a hero as currently on a mission. Stores the mission entity.
#[derive(Component, Debug, Reflect)]
#[reflect(Component)]
pub struct OnMission(pub Entity);

/// Stores the generated dungeon map on the mission entity so it persists
/// across view transitions and can be restored when re-watching a mission.
#[derive(Component, Debug, Clone, Reflect)]
#[reflect(Component)]
pub struct MissionDungeon(pub dungeon::DungeonMap);

/// Tracks which mission entity is currently being viewed in the MissionView.
#[derive(Resource, Debug)]
pub struct ViewedMission(pub Entity);
