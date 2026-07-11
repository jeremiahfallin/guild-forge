//! Equipment system: gear definitions, crafting, and combat bonuses.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::buildings::{BuildingType, GuildBuildings};
use crate::economy::Gold;
use crate::hero::data::HeroClass;
use crate::localization::{tr, trf};
use crate::materials::{MaterialType, Materials};

// ── Data types ─────────────────────────────────────────────────────

/// Which slot a piece of gear occupies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize, Reflect)]
pub enum GearSlot {
    Weapon,
    Armor,
    Accessory,
}

/// Tiers of gear rarity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize, Reflect, Default)]
pub enum GearRarity {
    #[default]
    Common,
    Uncommon,
    Rare,
    Epic,
    Legendary,
}

impl GearRarity {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Common => tr("rarity.common"),
            Self::Uncommon => tr("rarity.uncommon"),
            Self::Rare => tr("rarity.rare"),
            Self::Epic => tr("rarity.epic"),
            Self::Legendary => tr("rarity.legendary"),
        }
    }

    pub fn stat_multiplier(&self) -> f32 {
        match self {
            Self::Common => 1.0,
            Self::Uncommon => 1.2,
            Self::Rare => 1.5,
            Self::Epic => 2.0,
            Self::Legendary => 2.5,
        }
    }
}

/// Behavioral affixes that modify simulation actions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, Reflect)]
pub enum BehavioralAffix {
    Lifesteal,
    Initiative,
    CleaveOnHit,
}

impl GearSlot {
    pub const ALL: &[GearSlot] = &[GearSlot::Weapon, GearSlot::Armor, GearSlot::Accessory];
}

impl std::fmt::Display for GearSlot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Weapon => write!(f, "{}", tr("equipment.weapon")),
            Self::Armor => write!(f, "{}", tr("equipment.armor")),
            Self::Accessory => write!(f, "{}", tr("equipment.accessory")),
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
    #[allow(dead_code)]
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
    pub weapon_rarity: GearRarity,
    pub weapon_affix: Option<BehavioralAffix>,

    pub armor_tier: u32,
    pub armor_rarity: GearRarity,
    pub armor_affix: Option<BehavioralAffix>,

    pub accessory_tier: u32,
    pub accessory_rarity: GearRarity,
    pub accessory_affix: Option<BehavioralAffix>,
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

    /// Check if the hero has the specified affix on any gear slot.
    pub fn has_affix(&self, affix: BehavioralAffix) -> bool {
        self.weapon_affix == Some(affix)
            || self.armor_affix == Some(affix)
            || self.accessory_affix == Some(affix)
    }

    /// Sum up all stat bonuses from equipped gear tiers, applying rarity multipliers.
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
                    let rarity = match slot {
                        GearSlot::Weapon => self.weapon_rarity,
                        GearSlot::Armor => self.armor_rarity,
                        GearSlot::Accessory => self.accessory_rarity,
                    };
                    let mult = rarity.stat_multiplier();
                    total.attack += (gear_tier.stats.attack as f32 * mult).round() as i32;
                    total.defense += (gear_tier.stats.defense as f32 * mult).round() as i32;
                    total.hp += (gear_tier.stats.hp as f32 * mult).round() as i32;
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
    mut commands: Commands,
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

    commands.trigger(crate::ui::toast::ToastEvent {
        title: trf("equipment.crafted_toast", &[("name", &tier_def.name)]),
        body: trf("equipment.crafted_body", &[("hero", &info.name)]),
        kind: crate::ui::toast::ToastKind::Success,
        action: None,
    });

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
    app.register_type::<HeroEquipment>();
    app.register_type::<GearRarity>();
    app.register_type::<BehavioralAffix>();
    app.add_systems(Startup, load_equipment_database);
    app.add_observer(handle_craft_gear);
}
