//! Hero data definitions loaded from RON files.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// The class of a hero, determining their stat growth and abilities.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Reflect)]
pub enum HeroClass {
    Warrior,
    Rogue,
    Mage,
    Cleric,
    Ranger,
}

impl std::fmt::Display for HeroClass {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Warrior => write!(f, "Warrior"),
            Self::Rogue => write!(f, "Rogue"),
            Self::Mage => write!(f, "Mage"),
            Self::Cleric => write!(f, "Cleric"),
            Self::Ranger => write!(f, "Ranger"),
        }
    }
}

/// A personality trait that affects hero behavior and stat growth.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Reflect)]
pub enum HeroTrait {
    Brave,
    Cautious,
    Greedy,
    Loner,
    Leader,
    Cursed,
    Lucky,
}

impl std::fmt::Display for HeroTrait {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Brave => write!(f, "Brave"),
            Self::Cautious => write!(f, "Cautious"),
            Self::Greedy => write!(f, "Greedy"),
            Self::Loner => write!(f, "Loner"),
            Self::Leader => write!(f, "Leader"),
            Self::Cursed => write!(f, "Cursed"),
            Self::Lucky => write!(f, "Lucky"),
        }
    }
}

/// Weighted stat distribution used for class growth and trait modifiers.
#[derive(Debug, Clone, Deserialize)]
pub struct StatWeights {
    pub str: i32,
    pub dex: i32,
    pub con: i32,
    pub int: i32,
    pub wis: i32,
    pub cha: i32,
}

/// A class definition loaded from RON data.
#[derive(Debug, Clone, Deserialize)]
pub struct ClassDef {
    pub id: HeroClass,
    #[allow(dead_code)]
    pub name: String,
    #[allow(dead_code)]
    pub description: String,
    pub stat_weights: StatWeights,
    pub starting_abilities: Vec<String>,
}

/// A trait definition loaded from RON data.
#[derive(Debug, Clone, Deserialize)]
pub struct TraitDef {
    pub id: HeroTrait,
    pub name: String,
    pub description: String,
    pub stat_modifiers: StatWeights,
    #[allow(dead_code)]
    pub tags: Vec<String>,
}

/// Name generation pools loaded from RON data.
#[derive(Debug, Clone, Deserialize)]
pub struct NamePool {
    pub first_names: Vec<String>,
    pub surnames: Vec<String>,
}

/// Database of class definitions, loaded at startup.
#[derive(Resource)]
pub struct ClassDatabase(pub Vec<ClassDef>);

/// Database of trait definitions, loaded at startup.
#[derive(Resource)]
pub struct TraitDatabase(pub Vec<TraitDef>);

/// Database of name pools for hero generation.
#[derive(Resource)]
pub struct NameDatabase(pub NamePool);

impl ClassDatabase {
    pub fn get(&self, class: HeroClass) -> Option<&ClassDef> {
        self.0.iter().find(|c| c.id == class)
    }
}

impl TraitDatabase {
    pub fn get(&self, hero_trait: HeroTrait) -> Option<&TraitDef> {
        self.0.iter().find(|t| t.id == hero_trait)
    }
}

/// The effect type of an ability.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Reflect)]
pub enum AbilityEffect {
    Damage,
    Heal,
    Shield,
    Buff,
    Debuff,
}

/// The rule used by the AI to decide priority for using the ability.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Reflect)]
pub enum AiPriorityRule {
    Always,
    HpBelowPct(f32),
    MultipleEnemiesAdjacent(u32),
    Flanking,
    WarriorRallyingCry,
    RogueAssassinate,
    MageMeteor,
    ClericMassHeal,
    RangerVolley,
}

/// Definition of a class ability loaded from RON.
#[derive(Debug, Clone, Serialize, Deserialize, Reflect)]
pub struct AbilityDef {
    pub id: String,
    pub name: String,
    pub description: String,
    pub range: u32,
    pub cooldown: u32,
    pub effect: AbilityEffect,
    pub ai_priority: AiPriorityRule,
    #[serde(default)]
    pub is_signature: bool,
}

/// Database of all class abilities, loaded at startup.
#[derive(Resource)]
#[allow(dead_code)]
pub struct AbilityDatabase(pub Vec<AbilityDef>);

#[allow(dead_code)]
impl AbilityDatabase {
    pub fn get(&self, id: &str) -> Option<&AbilityDef> {
        self.0.iter().find(|a| a.id == id)
    }
}
