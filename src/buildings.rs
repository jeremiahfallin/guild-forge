use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::economy::Gold;
use crate::localization::{tr, trf};
use crate::materials::{MaterialType, Materials};
use crate::ui::toast::{ToastEvent, ToastKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Reflect)]
pub enum BuildingType {
    Armory,
    TrainingGrounds,
    Barracks,
    RecruitmentOffice,
    Workshop,
    Tavern,
    WarRoom,
}

impl BuildingType {
    pub const ALL: &[BuildingType] = &[
        BuildingType::Armory,
        BuildingType::TrainingGrounds,
        BuildingType::Barracks,
        BuildingType::RecruitmentOffice,
        BuildingType::Workshop,
        BuildingType::Tavern,
        BuildingType::WarRoom,
    ];

    pub fn name(&self) -> &'static str {
        match self {
            Self::Armory => tr("building.armory"),
            Self::TrainingGrounds => tr("building.training_grounds"),
            Self::Barracks => tr("building.barracks"),
            Self::RecruitmentOffice => tr("building.recruitment_office"),
            Self::Workshop => tr("building.workshop"),
            Self::Tavern => tr("building.tavern"),
            Self::WarRoom => tr("building.war_room"),
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            Self::Armory => tr("building.armory.desc"),
            Self::TrainingGrounds => tr("building.training_grounds.desc"),
            Self::Barracks => tr("building.barracks.desc"),
            Self::RecruitmentOffice => tr("building.recruitment_office.desc"),
            Self::Workshop => tr("building.workshop.desc"),
            Self::Tavern => tr("building.tavern.desc"),
            Self::WarRoom => tr("building.war_room.desc"),
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

    /// How many missions may run concurrently. War Room raises the ceiling.
    pub fn mission_cap(&self) -> u32 {
        3 + self.level(BuildingType::WarRoom)
    }

    pub fn can_dispatch(&self, active_missions: usize) -> bool {
        active_missions < self.mission_cap() as usize
    }
}

/// Event: request to upgrade a building.
#[derive(Event)]
pub struct UpgradeBuilding(pub BuildingType);

fn handle_upgrade_building(
    trigger: On<UpgradeBuilding>,
    mut commands: Commands,
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

    let new_level = current_level + 1;
    buildings.0.insert(building_type, new_level);

    commands.trigger(ToastEvent {
        title: trf("building.upgraded_toast", &[("name", building_type.name())]),
        body: trf("building.upgraded_body", &[("level", &new_level.to_string())]),
        kind: ToastKind::Success,
        action: None,
    });
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mission_cap_grows_with_war_room() {
        let mut buildings = GuildBuildings::default();
        assert_eq!(buildings.mission_cap(), 3);
        buildings.0.insert(BuildingType::WarRoom, 2);
        assert_eq!(buildings.mission_cap(), 5);
    }

    #[test]
    fn can_dispatch_boundaries() {
        let buildings = GuildBuildings::default(); // cap 3
        assert!(buildings.can_dispatch(0));
        assert!(buildings.can_dispatch(2));
        assert!(!buildings.can_dispatch(3));
        assert!(!buildings.can_dispatch(4));
    }

    #[test]
    fn building_database_includes_war_room() {
        let data: BuildingsData =
            ron::from_str(include_str!("../assets/data/buildings.ron")).unwrap();
        let db = BuildingDatabase(data.buildings);
        let def = db.get(BuildingType::WarRoom).expect("War Room in buildings.ron");
        assert_eq!(def.max_level, 3);
        assert_eq!(def.level_costs.len(), 3);
    }
}
