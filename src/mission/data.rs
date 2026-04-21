//! Mission and enemy data definitions loaded from RON files.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::materials::MaterialType;

/// Enemy type identifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Reflect)]
pub enum EnemyType {
    Goblin,
    Skeleton,
    Slime,
    Orc,
    BossRat,
}

impl std::fmt::Display for EnemyType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Goblin => write!(f, "Goblin"),
            Self::Skeleton => write!(f, "Skeleton"),
            Self::Slime => write!(f, "Slime"),
            Self::Orc => write!(f, "Orc"),
            Self::BossRat => write!(f, "Boss Rat"),
        }
    }
}

/// An enemy definition loaded from RON.
#[derive(Debug, Clone, Deserialize)]
pub struct EnemyDef {
    pub id: EnemyType,
    pub name: String,
    pub hp: i32,
    pub attack: i32,
    pub defense: i32,
    pub xp_reward: u32,
}

/// Gold reward range for a mission.
#[derive(Debug, Clone, Deserialize)]
pub struct GoldReward {
    pub min: u32,
    pub max: u32,
}

/// A mission template loaded from RON.
#[derive(Debug, Clone, Deserialize)]
pub struct MissionTemplate {
    pub id: String,
    pub name: String,
    pub description: String,
    pub difficulty: u32,
    pub enemy_types: Vec<(EnemyType, u32)>,
    pub rooms_min: u32,
    pub rooms_max: u32,
    pub gold_reward: GoldReward,
    pub xp_bonus: u32,
    pub reputation_required: u32,
    pub reputation_reward: u32,
    pub material_drops: Vec<(MaterialType, u32, u32)>, // (type, min, max)
}

/// Database of enemy definitions.
#[derive(Resource)]
pub struct EnemyDatabase(pub Vec<EnemyDef>);

/// Database of mission templates.
#[derive(Resource)]
pub struct MissionTemplateDatabase(pub Vec<MissionTemplate>);

impl EnemyDatabase {
    pub fn get(&self, enemy_type: EnemyType) -> Option<&EnemyDef> {
        self.0.iter().find(|e| e.id == enemy_type)
    }
}

/// Load mission-related databases from RON files.
pub fn load_mission_databases(mut commands: Commands) {
    let enemies_str = include_str!("../../assets/data/enemies.ron");
    let enemies: Vec<EnemyDef> =
        ron::from_str(enemies_str).expect("Failed to parse enemies.ron");
    commands.insert_resource(EnemyDatabase(enemies));

    let templates_str = include_str!("../../assets/data/mission_templates.ron");
    let templates: Vec<MissionTemplate> =
        ron::from_str(templates_str).expect("Failed to parse mission_templates.ron");
    commands.insert_resource(MissionTemplateDatabase(templates));

    info!("Mission databases loaded");
}
