//! Equipment system: gear definitions, crafting, and combat bonuses.

use bevy::prelude::*;
use serde::Deserialize;

use crate::buildings::{BuildingType, GuildBuildings};
use crate::economy::Gold;
use crate::hero::data::HeroClass;
use crate::materials::{MaterialType, Materials};

// ── Data types ─────────────────────────────────────────────────────

/// Which slot a piece of gear occupies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize, Reflect)]
pub enum GearSlot {
    Weapon,
    Armor,
    Accessory,
}

impl GearSlot {
    pub const ALL: &[GearSlot] = &[GearSlot::Weapon, GearSlot::Armor, GearSlot::Accessory];
}

impl std::fmt::Display for GearSlot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Weapon => write!(f, "Weapon"),
            Self::Armor => write!(f, "Armor"),
            Self::Accessory => write!(f, "Accessory"),
        }
    }
}

/// Stat bonuses granted by a piece of gear.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct GearStats {
    #[serde(default)]
    pub attack: i32,
    #[serde(default)]
    pub defense: i32,
    #[serde(default)]
    pub hp: i32,
}

/// One tier within a gear upgrade path.
#[derive(Debug, Clone, Deserialize)]
pub struct GearTier {
    pub name: String,
    pub tier: u32,
    pub stats: GearStats,
    pub gold_cost: u32,
    #[serde(default)]
    pub material_cost: Vec<(MaterialType, u32)>,
    pub armory_level_required: u32,
}

/// A full upgrade path for a class+slot combination.
#[derive(Debug, Clone, Deserialize)]
pub struct GearPath {
    pub class: HeroClass,
    pub slot: GearSlot,
    pub tiers: Vec<GearTier>,
}

/// Top-level RON deserialization wrapper.
#[derive(Debug, Clone, Deserialize)]
pub struct EquipmentData {
    pub paths: Vec<GearPath>,
}

/// Database of all gear paths, loaded at startup.
#[derive(Resource, Debug, Clone)]
pub struct EquipmentDatabase(pub Vec<GearPath>);

impl EquipmentDatabase {
    /// Look up the gear path for a given class and slot.
    pub fn get_path(&self, class: HeroClass, slot: GearSlot) -> Option<&GearPath> {
        self.0.iter().find(|p| p.class == class && p.slot == slot)
    }
}

// ── Hero equipment component ───────────────────────────────────────

/// Tracks a hero's current gear tier in each slot.
#[derive(Component, Debug, Clone, Reflect, Default)]
#[reflect(Component)]
pub struct HeroEquipment {
    pub weapon_tier: u32,
    pub armor_tier: u32,
    pub accessory_tier: u32,
}

impl HeroEquipment {
    /// Get the current tier for a slot.
    pub fn tier(&self, slot: GearSlot) -> u32 {
        match slot {
            GearSlot::Weapon => self.weapon_tier,
            GearSlot::Armor => self.armor_tier,
            GearSlot::Accessory => self.accessory_tier,
        }
    }

    /// Set the tier for a slot.
    pub fn set_tier(&mut self, slot: GearSlot, tier: u32) {
        match slot {
            GearSlot::Weapon => self.weapon_tier = tier,
            GearSlot::Armor => self.armor_tier = tier,
            GearSlot::Accessory => self.accessory_tier = tier,
        }
    }

    /// Sum up all stat bonuses from equipped gear tiers.
    pub fn total_stats(&self, db: &EquipmentDatabase, class: HeroClass) -> GearStats {
        let mut total = GearStats::default();
        for &slot in GearSlot::ALL {
            let tier = self.tier(slot);
            if tier == 0 {
                continue;
            }
            if let Some(path) = db.get_path(class, slot) {
                // Tiers are 1-indexed in the data; vec is 0-indexed
                if let Some(gear_tier) = path.tiers.get((tier - 1) as usize) {
                    total.attack += gear_tier.stats.attack;
                    total.defense += gear_tier.stats.defense;
                    total.hp += gear_tier.stats.hp;
                }
            }
        }
        total
    }
}

// ── Crafting event & observer ──────────────────────────────────────

/// Event: request to craft/upgrade gear for a hero.
#[derive(Event)]
pub struct CraftGear {
    pub hero: Entity,
    pub slot: GearSlot,
}

fn handle_craft_gear(
    trigger: On<CraftGear>,
    mut heroes: Query<(&crate::hero::HeroInfo, &mut HeroEquipment)>,
    equipment_db: Res<EquipmentDatabase>,
    buildings: Res<GuildBuildings>,
    mut gold: ResMut<Gold>,
    mut materials: ResMut<Materials>,
) {
    let event = trigger.event();
    let Ok((info, mut equipment)) = heroes.get_mut(event.hero) else {
        warn!("CraftGear: hero entity {:?} not found", event.hero);
        return;
    };

    let current_tier = equipment.tier(event.slot);
    let next_tier = current_tier + 1;

    // Look up the gear path
    let Some(path) = equipment_db.get_path(info.class, event.slot) else {
        warn!("CraftGear: no gear path for {:?}/{:?}", info.class, event.slot);
        return;
    };

    // Find the next tier definition (tiers are 1-indexed, vec is 0-indexed)
    let Some(tier_def) = path.tiers.get((next_tier - 1) as usize) else {
        warn!("CraftGear: already at max tier for {:?}/{:?}", info.class, event.slot);
        return;
    };

    // Check armory level
    let armory_level = buildings.level(BuildingType::Armory);
    if armory_level < tier_def.armory_level_required {
        warn!(
            "CraftGear: armory level {} < required {}",
            armory_level, tier_def.armory_level_required
        );
        return;
    }

    // Check gold
    if gold.0 < tier_def.gold_cost {
        warn!("CraftGear: not enough gold ({} < {})", gold.0, tier_def.gold_cost);
        return;
    }

    // Check materials
    if !tier_def
        .material_cost
        .iter()
        .all(|&(mat, amt)| materials.get(mat) >= amt)
    {
        warn!("CraftGear: insufficient materials");
        return;
    }

    // Deduct costs
    gold.0 -= tier_def.gold_cost;
    for &(mat, amt) in &tier_def.material_cost {
        materials.try_spend(mat, amt);
    }

    // Upgrade tier
    equipment.set_tier(event.slot, next_tier);
    info!(
        "Crafted {} for hero (tier {})",
        tier_def.name, next_tier
    );
}

// ── Startup & plugin ───────────────────────────────────────────────

fn load_equipment_database(mut commands: Commands) {
    let data: EquipmentData =
        ron::from_str(include_str!("../assets/data/equipment.ron"))
            .expect("Failed to parse equipment.ron");
    commands.insert_resource(EquipmentDatabase(data.paths));
    info!("Equipment database loaded");
}

pub(super) fn plugin(app: &mut App) {
    app.add_systems(Startup, load_equipment_database);
    app.add_observer(handle_craft_gear);
}
