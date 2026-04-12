//! Save system: serialize/deserialize full game state to/from a RON file.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

use crate::buildings::BuildingType;
use crate::hero::data::{HeroClass, HeroTrait};
use crate::materials::MaterialType;
use crate::mission::MissionProgress;
use crate::mission::data::EnemyType;
use crate::mission::dungeon::DungeonMap;

/// Return the save file path: `<data_dir>/guild-forge/save.ron`.
pub fn save_file_path() -> Option<PathBuf> {
    dirs::data_dir().map(|d| d.join("guild-forge").join("save.ron"))
}

/// Returns true if a save file exists on disk.
pub fn has_save_file() -> bool {
    save_file_path().is_some_and(|p| p.exists())
}

/// Top-level save data structure.
#[derive(Serialize, Deserialize)]
pub struct SaveData {
    pub last_save_timestamp: u64,
    pub gold: u32,
    pub reputation: u32,
    pub banked_seconds: f32,
    pub materials: HashMap<MaterialType, u32>,
    pub buildings: HashMap<BuildingType, u32>,
    pub heroes: Vec<HeroSaveDto>,
    pub applicants: Vec<ApplicantSaveDto>,
    pub next_arrival_timer: f32,
    pub training_timer: f32,
    pub missions: Vec<MissionSaveDto>,
}

#[derive(Serialize, Deserialize)]
pub struct HeroStatsSave {
    pub strength: i32,
    pub dexterity: i32,
    pub constitution: i32,
    pub intelligence: i32,
    pub wisdom: i32,
    pub charisma: i32,
}

#[derive(Serialize, Deserialize)]
pub struct HeroEquipmentSave {
    pub weapon_tier: u32,
    pub armor_tier: u32,
    pub accessory_tier: u32,
}

#[derive(Serialize, Deserialize)]
pub struct HeroSaveDto {
    pub name: String,
    pub class: HeroClass,
    pub level: u32,
    pub xp: u32,
    pub xp_to_next: u32,
    pub stats: HeroStatsSave,
    pub traits: Vec<HeroTrait>,
    pub equipment: HeroEquipmentSave,
    pub on_mission: bool,
}

#[derive(Serialize, Deserialize)]
pub struct ApplicantSaveDto {
    pub name: String,
    pub class: HeroClass,
    pub traits: Vec<HeroTrait>,
    pub stats: HeroStatsSave,
    pub hire_cost: u32,
    pub time_remaining: f32,
}

#[derive(Serialize, Deserialize)]
pub struct MissionSaveDto {
    pub template_id: String,
    pub name: String,
    pub difficulty: u32,
    pub progress: MissionProgress,
    pub rng_seed: u64,
    pub party_hero_indices: Vec<usize>,
    pub dungeon_map: DungeonMap,
    pub room_visited: Vec<bool>,
    pub room_cleared: Vec<bool>,
    pub hero_tokens: Vec<HeroTokenDto>,
    pub enemy_tokens: Vec<EnemyTokenDto>,
}

#[derive(Serialize, Deserialize)]
pub struct HeroTokenDto {
    pub roster_index: usize,
    pub grid_x: u32,
    pub grid_y: u32,
    pub in_room: Option<usize>,
    pub hp: i32,
    pub max_hp: i32,
    pub attack: i32,
    pub defense: i32,
    pub path: Option<Vec<(u32, u32)>>,
    pub path_index: usize,
}

#[derive(Serialize, Deserialize)]
pub struct EnemyTokenDto {
    pub enemy_type: EnemyType,
    pub xp_reward: u32,
    pub grid_x: u32,
    pub grid_y: u32,
    pub in_room: Option<usize>,
    pub hp: i32,
    pub max_hp: i32,
    pub attack: i32,
    pub defense: i32,
}
