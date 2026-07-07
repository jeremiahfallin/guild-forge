//! Mission and enemy data definitions loaded from RON files.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::materials::MaterialType;
use crate::hero::data::{AbilityDef, AbilityDatabase, HeroTrait};

/// Enemy type identifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Reflect)]
pub enum EnemyType {
    Goblin,
    Skeleton,
    Slime,
    Orc,
    BossRat,
    GiantRat,
    GoblinArcher,
    GoblinShaman,
    SpiderSwarmer,
}

impl EnemyType {
    /// Boss-tier enemies get the BOSS ENCOUNTER banner and boss-room spawns.
    pub fn is_boss(&self) -> bool {
        matches!(self, Self::BossRat)
    }
}

impl std::fmt::Display for EnemyType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Goblin => write!(f, "Goblin"),
            Self::Skeleton => write!(f, "Skeleton"),
            Self::Slime => write!(f, "Slime"),
            Self::Orc => write!(f, "Orc"),
            Self::BossRat => write!(f, "Boss Rat"),
            Self::GiantRat => write!(f, "Giant Rat"),
            Self::GoblinArcher => write!(f, "Goblin Archer"),
            Self::GoblinShaman => write!(f, "Goblin Shaman"),
            Self::SpiderSwarmer => write!(f, "Spider Swarmer"),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, Reflect)]
pub enum EnemyBehavior {
    #[default]
    Standard,
    Skirmisher,
    Swarmer,
    Shaman,
}

fn default_attack_range() -> u32 {
    1
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
    #[serde(default)]
    pub abilities: Vec<String>,
    #[serde(default)]
    pub behavior: EnemyBehavior,
    #[serde(default = "default_attack_range")]
    pub attack_range: u32,
}

/// Gold reward range for a mission.
#[derive(Debug, Clone, Deserialize)]
pub struct GoldReward {
    pub min: u32,
    pub max: u32,
}

/// Mission modifiers that change simulation or rewards.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Reflect)]
pub enum MissionModifier {
    Foggy,
    Infested,
    CursedGround,
    Bountiful,
    Trapped,
}

impl std::fmt::Display for MissionModifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Foggy => write!(f, "Foggy"),
            Self::Infested => write!(f, "Infested"),
            Self::CursedGround => write!(f, "Cursed Ground"),
            Self::Bountiful => write!(f, "Bountiful"),
            Self::Trapped => write!(f, "Trapped"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Reflect, Default)]
pub enum BiomeType {
    #[default]
    Dungeon,
    Crypt,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Reflect)]
pub enum EnemyFamily {
    Goblinoid,
    Undead,
}

impl EnemyType {
    #[allow(dead_code)]
    pub fn family(&self) -> EnemyFamily {
        match self {
            EnemyType::Skeleton => EnemyFamily::Undead,
            _ => EnemyFamily::Goblinoid,
        }
    }
}

impl BiomeType {
    #[allow(dead_code)]
    pub fn allowed_families(&self) -> Vec<EnemyFamily> {
        match self {
            BiomeType::Dungeon => vec![EnemyFamily::Goblinoid],
            BiomeType::Crypt => vec![EnemyFamily::Undead, EnemyFamily::Goblinoid],
        }
    }
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
    #[serde(default)]
    pub allowed_modifiers: Vec<MissionModifier>,
    #[serde(default)]
    pub biome: BiomeType,
}

/// Database of enemy definitions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Reflect)]
pub enum EventType {
    Shrine,
    Ambush,
    HiddenChamber,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Reflect)]
pub enum EventCheckStat {
    Strength,
    Dexterity,
    Constitution,
    Intelligence,
    Wisdom,
    Charisma,
}

#[derive(Debug, Clone, Deserialize, Reflect)]
pub struct MissionEventDef {
    pub id: String,
    pub name: String,
    pub description: String,
    pub event_type: EventType,
    pub check_stat: EventCheckStat,
    pub check_difficulty: i32,
    pub success_text: String,
    pub failure_text: String,
    pub priority_trait: Option<HeroTrait>,
}

/// Database of mid-mission random events.
#[derive(Resource)]
pub struct EventDatabase(pub Vec<MissionEventDef>);

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

    let abilities_str = include_str!("../../assets/data/abilities.ron");
    let abilities: Vec<AbilityDef> =
        ron::from_str(abilities_str).expect("Failed to parse abilities.ron");
    commands.insert_resource(AbilityDatabase(abilities));

    let narrative_str = include_str!("../../assets/data/narrative_templates.ron");
    let narrative: crate::ui::feed::NarrativeTemplates =
        ron::from_str(narrative_str).expect("Failed to parse narrative_templates.ron");
    commands.insert_resource(narrative);

    let events_str = include_str!("../../assets/data/events.ron");
    let events: Vec<MissionEventDef> =
        ron::from_str(events_str).expect("Failed to parse events.ron");
    commands.insert_resource(EventDatabase(events));

    info!("Mission databases loaded");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hero::data::{ClassDatabase, ClassDef};

    #[test]
    fn test_class_abilities_resolution() {
        // Load class definitions
        let classes_str = include_str!("../../assets/data/classes.ron");
        let classes: Vec<ClassDef> =
            ron::from_str(classes_str).expect("Failed to parse classes.ron");
        let class_db = ClassDatabase(classes);

        // Load ability definitions
        let abilities_str = include_str!("../../assets/data/abilities.ron");
        let abilities: Vec<AbilityDef> =
            ron::from_str(abilities_str).expect("Failed to parse abilities.ron");
        let ability_db = AbilityDatabase(abilities);

        // Verify every class's starting abilities are resolved in the ability database
        for class in &class_db.0 {
            for ability_id in &class.starting_abilities {
                let resolved = ability_db.get(ability_id);
                assert!(
                    resolved.is_some(),
                    "Class {:?} references starting ability '{}' which is missing from abilities.ron!",
                    class.id,
                    ability_id
                );
            }
        }
    }

    #[test]
    #[should_panic(expected = "Failed to parse abilities.ron")]
    fn test_malformed_ability_data_fails_loudly() {
        let malformed_ron = r#"[
            (
                id: "BrokenAbility",
                name: "Broken",
                description: "Malformed fields",
                range: "not-a-number", // Should be u32
                cooldown: 1,
                effect: InvalidEffect,
                ai_priority: Always,
            )
        ]"#;
        let _: Vec<AbilityDef> =
            ron::from_str(malformed_ron).expect("Failed to parse abilities.ron");
    }

    #[test]
    fn test_narrative_templates_load() {
        let templates_str = include_str!("../../assets/data/narrative_templates.ron");
        let _: crate::ui::feed::NarrativeTemplates =
            ron::from_str(templates_str).expect("Failed to parse narrative_templates.ron");
    }

    #[test]
    fn test_biome_enemy_families_validation() {
        let templates_str = include_str!("../../assets/data/mission_templates.ron");
        let templates: Vec<MissionTemplate> =
            ron::from_str(templates_str).expect("Failed to parse mission_templates.ron");
        
        for template in &templates {
            let allowed = template.biome.allowed_families();
            for &(enemy_type, _) in &template.enemy_types {
                let family = enemy_type.family();
                assert!(
                    allowed.contains(&family),
                    "Template '{}' uses enemy '{:?}' of family '{:?}' which is not allowed in biome '{:?}'!",
                    template.name,
                    enemy_type,
                    family,
                    template.biome
                );
            }
        }
    }
}
