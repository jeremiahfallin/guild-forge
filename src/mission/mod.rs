//! Mission system: data, dungeon generation, simulation, AI, and combat.

pub mod ai;
pub mod combat;
pub mod data;
pub mod dungeon;
pub mod entities;
pub mod pathfinding;

use bevy::prelude::*;

use crate::screens::GameTab;

pub(super) fn plugin(app: &mut App) {
    app.add_systems(Startup, data::load_mission_databases);
    app.add_systems(OnEnter(GameTab::MissionView), entities::spawn_mission_entities);
    app.add_systems(OnExit(GameTab::MissionView), entities::cleanup_mission_entities);
    app.add_systems(
        Update,
        (
            // Simulation tick: advance timer & move heroes
            entities::simulation_tick,
            // AI: decide actions for heroes
            ai::hero_ai_system,
            // Combat: resolve attacks and healing
            combat::hero_combat_system,
            combat::enemy_combat_system,
            // Cleanup: handle death, update room status
            combat::handle_death_system,
            combat::update_room_status,
            // Check win/lose conditions
            combat::check_mission_completion,
            // Visual: sync sprite positions smoothly
            entities::sync_sprite_positions,
        )
            .chain()
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
