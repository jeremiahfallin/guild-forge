use bevy::prelude::*;
use serde::Deserialize;
use std::collections::HashMap;

use crate::economy::Gold;
use crate::materials::{MaterialType, Materials};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize, Reflect)]
pub enum BuildingType {
    Armory,
    TrainingGrounds,
    Barracks,
    RecruitmentOffice,
    Workshop,
}

impl BuildingType {
    pub const ALL: &[BuildingType] = &[
        BuildingType::Armory,
        BuildingType::TrainingGrounds,
        BuildingType::Barracks,
        BuildingType::RecruitmentOffice,
        BuildingType::Workshop,
    ];

    pub fn name(&self) -> &'static str {
        match self {
            Self::Armory => "Armory",
            Self::TrainingGrounds => "Training Grounds",
            Self::Barracks => "Barracks",
            Self::RecruitmentOffice => "Recruitment Office",
            Self::Workshop => "Workshop",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            Self::Armory => "Craft and upgrade equipment for your heroes.",
            Self::TrainingGrounds => "Heroes gain passive XP while idle.",
            Self::Barracks => "Increases your guild's roster capacity.",
            Self::RecruitmentOffice => "More applicants with better quality.",
            Self::Workshop => "Convert raw materials into refined ones.",
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct BuildingCost {
    pub gold: u32,
    pub materials: Vec<(MaterialType, u32)>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BuildingDef {
    pub id: BuildingType,
    pub level_costs: Vec<BuildingCost>,
    pub max_level: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BuildingsData {
    pub buildings: Vec<BuildingDef>,
}

#[derive(Resource, Debug, Clone)]
pub struct BuildingDatabase(pub Vec<BuildingDef>);

impl BuildingDatabase {
    pub fn get(&self, building: BuildingType) -> Option<&BuildingDef> {
        self.0.iter().find(|b| b.id == building)
    }
}

#[derive(Resource, Debug, Clone)]
pub struct GuildBuildings(pub HashMap<BuildingType, u32>);

impl Default for GuildBuildings {
    fn default() -> Self {
        let mut map = HashMap::new();
        for &b in BuildingType::ALL {
            map.insert(b, 0);
        }
        Self(map)
    }
}

impl GuildBuildings {
    pub fn level(&self, building: BuildingType) -> u32 {
        self.0.get(&building).copied().unwrap_or(0)
    }

    pub fn roster_cap(&self) -> u32 {
        3 + self.level(BuildingType::Barracks) * 2
    }

    pub fn max_applicants(&self) -> u32 {
        3 + self.level(BuildingType::RecruitmentOffice)
    }
}

/// Event: request to upgrade a building.
#[derive(Event)]
pub struct UpgradeBuilding(pub BuildingType);

fn handle_upgrade_building(
    trigger: On<UpgradeBuilding>,
    mut buildings: ResMut<GuildBuildings>,
    building_db: Res<BuildingDatabase>,
    mut gold: ResMut<Gold>,
    mut materials: ResMut<Materials>,
) {
    let building_type = trigger.event().0;
    let current_level = buildings.level(building_type);

    let Some(def) = building_db.get(building_type) else { return };
    if current_level >= def.max_level { return; }

    let cost = &def.level_costs[current_level as usize];

    if gold.0 < cost.gold { return; }
    if !cost.materials.iter().all(|&(mat, amt)| materials.get(mat) >= amt) {
        return;
    }

    gold.0 -= cost.gold;
    for &(mat, amt) in &cost.materials {
        materials.try_spend(mat, amt);
    }

    buildings.0.insert(building_type, current_level + 1);
}

fn load_building_database(mut commands: Commands) {
    let data: BuildingsData =
        ron::from_str(include_str!("../assets/data/buildings.ron"))
            .expect("Failed to parse buildings.ron");
    commands.insert_resource(BuildingDatabase(data.buildings));
}

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<GuildBuildings>();
    app.add_observer(handle_upgrade_building);
    app.add_systems(Startup, load_building_database);
}
