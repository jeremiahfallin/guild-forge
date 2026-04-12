use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::buildings::{BuildingType, GuildBuildings};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Reflect)]
pub enum MaterialType {
    // Raw
    IronOre, RawLeather, Wood, RawHerbs, RoughGems,
    // Refined (tier 2)
    SteelIngot, CuredLeather, Lumber, Potion, CutGem,
    // Refined (tier 3)
    EnchantedSteel, DragonLeather, ArcaneWood, ElixirOfPower, PrismaticGem,
}

impl MaterialType {
    /// All material types in display order: raw, then refined, then high-tier.
    pub const ALL: &[MaterialType] = &[
        // Raw
        MaterialType::IronOre, MaterialType::RawLeather, MaterialType::Wood,
        MaterialType::RawHerbs, MaterialType::RoughGems,
        // Refined
        MaterialType::SteelIngot, MaterialType::CuredLeather, MaterialType::Lumber,
        MaterialType::Potion, MaterialType::CutGem,
        // High-tier
        MaterialType::EnchantedSteel, MaterialType::DragonLeather, MaterialType::ArcaneWood,
        MaterialType::ElixirOfPower, MaterialType::PrismaticGem,
    ];

    pub fn name(&self) -> &'static str {
        match self {
            Self::IronOre => "Iron Ore",
            Self::RawLeather => "Raw Leather",
            Self::Wood => "Wood",
            Self::RawHerbs => "Raw Herbs",
            Self::RoughGems => "Rough Gems",
            Self::SteelIngot => "Steel Ingot",
            Self::CuredLeather => "Cured Leather",
            Self::Lumber => "Lumber",
            Self::Potion => "Potion",
            Self::CutGem => "Cut Gem",
            Self::EnchantedSteel => "Enchanted Steel",
            Self::DragonLeather => "Dragon Leather",
            Self::ArcaneWood => "Arcane Wood",
            Self::ElixirOfPower => "Elixir of Power",
            Self::PrismaticGem => "Prismatic Gem",
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct ConversionRecipe {
    pub input_type: MaterialType,
    pub input_count: u32,
    pub output_type: MaterialType,
    pub output_count: u32,
    pub workshop_level_required: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MaterialsData {
    pub conversions: Vec<ConversionRecipe>,
}

#[derive(Resource, Debug, Clone)]
pub struct ConversionDatabase(pub Vec<ConversionRecipe>);

#[derive(Resource, Debug, Clone, Default)]
pub struct Materials(pub HashMap<MaterialType, u32>);

impl Materials {
    pub fn get(&self, mat: MaterialType) -> u32 {
        self.0.get(&mat).copied().unwrap_or(0)
    }

    pub fn add(&mut self, mat: MaterialType, amount: u32) {
        *self.0.entry(mat).or_insert(0) += amount;
    }

    pub fn try_spend(&mut self, mat: MaterialType, amount: u32) -> bool {
        let entry = self.0.entry(mat).or_insert(0);
        if *entry >= amount {
            *entry -= amount;
            true
        } else {
            false
        }
    }

    #[allow(dead_code)]
    pub fn try_spend_all(&mut self, costs: &[(MaterialType, u32)]) -> bool {
        for &(mat, amount) in costs {
            if self.get(mat) < amount {
                return false;
            }
        }
        for &(mat, amount) in costs {
            self.try_spend(mat, amount);
        }
        true
    }
}

#[derive(Event)]
pub struct ConvertMaterials {
    pub recipe_index: usize,
    pub quantity: u32,
}

fn handle_convert_materials(
    trigger: On<ConvertMaterials>,
    mut commands: Commands,
    mut materials: ResMut<Materials>,
    conversion_db: Res<ConversionDatabase>,
    buildings: Res<GuildBuildings>,
) {
    let event = trigger.event();
    let Some(recipe) = conversion_db.0.get(event.recipe_index) else {
        return;
    };

    let workshop_level = buildings.level(BuildingType::Workshop);
    if workshop_level < recipe.workshop_level_required {
        return;
    }

    let available = materials.get(recipe.input_type);
    let max_runs = available / recipe.input_count;
    let runs = event.quantity.min(max_runs);
    if runs == 0 {
        return;
    }

    let input_total = runs * recipe.input_count;
    let output_total = runs * recipe.output_count;

    materials.try_spend(recipe.input_type, input_total);
    materials.add(recipe.output_type, output_total);

    commands.trigger(crate::ui::toast::ToastEvent {
        title: "Materials converted!".to_string(),
        body: format!(
            "{} {} \u{2192} {} {}",
            input_total,
            recipe.input_type.name(),
            output_total,
            recipe.output_type.name()
        ),
        kind: crate::ui::toast::ToastKind::Info,
    });
}

fn load_materials_database(mut commands: Commands) {
    let data: MaterialsData =
        ron::from_str(include_str!("../assets/data/materials.ron"))
            .expect("Failed to parse materials.ron");
    commands.insert_resource(ConversionDatabase(data.conversions));
}

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<Materials>();
    app.add_systems(Startup, load_materials_database);
    app.add_observer(handle_convert_materials);
}
