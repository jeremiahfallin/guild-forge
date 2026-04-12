# Guild Management Loop Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add a guild management loop with recruiting (timed applicant board), equipment (class-specific crafting paths), guild buildings, typed materials, and reputation — giving players meaningful between-mission decisions.

**Architecture:** New ECS resources for Materials, Reputation, Buildings, and Equipment. New RON data files define material types, building costs, equipment paths, and conversion recipes. Mission completion awards materials + reputation alongside existing gold/XP. Three new GameTab screens (Guild, Armory, Recruiting) follow the existing sidebar-navigated tab pattern. Applicant arrivals use a real-time timer resource ticked in Update.

**Tech Stack:** Bevy 0.18, bevy_declarative (UI), serde/RON (data), Rust 2024 edition

---

## Phase 1: Resource Foundation

### Task 1: Material Type Definitions

**Files:**
- Create: `src/materials.rs`
- Create: `assets/data/materials.ron`
- Modify: `src/main.rs` (add plugin)

**Step 1: Create the materials module**

In `src/materials.rs`, define the material system:

```rust
use bevy::prelude::*;
use serde::Deserialize;
use std::collections::HashMap;

/// Individual material type identifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize, Reflect)]
pub enum MaterialType {
    // Raw materials
    IronOre,
    RawLeather,
    Wood,
    RawHerbs,
    RoughGems,
    // Refined materials
    SteelIngot,
    CuredLeather,
    Lumber,
    Potion,
    CutGem,
    // Tier 3 refined
    EnchantedSteel,
    DragonLeather,
    ArcaneWood,
    ElixirOfPower,
    PrismaticGem,
}

/// A conversion recipe: input material + count → output material + count.
#[derive(Debug, Clone, Deserialize)]
pub struct ConversionRecipe {
    pub input_type: MaterialType,
    pub input_count: u32,
    pub output_type: MaterialType,
    pub output_count: u32,
    pub workshop_level_required: u32,
}

/// RON file structure for materials data.
#[derive(Debug, Clone, Deserialize)]
pub struct MaterialsData {
    pub conversions: Vec<ConversionRecipe>,
}

/// Resource: loaded conversion recipes.
#[derive(Resource, Debug, Clone)]
pub struct ConversionDatabase(pub Vec<ConversionRecipe>);

/// Resource: the guild's material stockpile.
#[derive(Resource, Debug, Clone, Default)]
pub struct Materials(pub HashMap<MaterialType, u32>);

impl Materials {
    pub fn get(&self, mat: MaterialType) -> u32 {
        self.0.get(&mat).copied().unwrap_or(0)
    }

    pub fn add(&mut self, mat: MaterialType, amount: u32) {
        *self.0.entry(mat).or_insert(0) += amount;
    }

    /// Try to spend materials. Returns false if insufficient.
    pub fn try_spend(&mut self, mat: MaterialType, amount: u32) -> bool {
        let entry = self.0.entry(mat).or_insert(0);
        if *entry >= amount {
            *entry -= amount;
            true
        } else {
            false
        }
    }

    /// Try to spend a list of costs. All-or-nothing.
    pub fn try_spend_all(&mut self, costs: &[(MaterialType, u32)]) -> bool {
        // Check all first
        for &(mat, amount) in costs {
            if self.get(mat) < amount {
                return false;
            }
        }
        // Deduct all
        for &(mat, amount) in costs {
            self.try_spend(mat, amount);
        }
        true
    }
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
}
```

**Step 2: Create the RON data file**

In `assets/data/materials.ron`:

```ron
(
    conversions: [
        // Tier 1 → Tier 2 (Workshop Lv 1)
        (input_type: IronOre,    input_count: 3, output_type: SteelIngot,   output_count: 1, workshop_level_required: 1),
        (input_type: RawLeather, input_count: 3, output_type: CuredLeather, output_count: 1, workshop_level_required: 1),
        (input_type: Wood,       input_count: 3, output_type: Lumber,       output_count: 1, workshop_level_required: 1),
        (input_type: RawHerbs,   input_count: 3, output_type: Potion,       output_count: 1, workshop_level_required: 1),
        (input_type: RoughGems,  input_count: 3, output_type: CutGem,       output_count: 1, workshop_level_required: 1),

        // Tier 2 → Tier 3 (Workshop Lv 2)
        (input_type: SteelIngot,   input_count: 3, output_type: EnchantedSteel,  output_count: 1, workshop_level_required: 2),
        (input_type: CuredLeather, input_count: 3, output_type: DragonLeather,   output_count: 1, workshop_level_required: 2),
        (input_type: Lumber,       input_count: 3, output_type: ArcaneWood,      output_count: 1, workshop_level_required: 2),
        (input_type: Potion,       input_count: 3, output_type: ElixirOfPower,   output_count: 1, workshop_level_required: 2),
        (input_type: CutGem,       input_count: 3, output_type: PrismaticGem,    output_count: 1, workshop_level_required: 2),
    ],
)
```

**Step 3: Register in main.rs**

Add `mod materials;` and `materials::plugin` to the plugin list in `src/main.rs`.

**Step 4: Verify compilation**

Run: `cargo build`
Expected: Compiles with no errors.

**Step 5: Commit**

```bash
git add src/materials.rs assets/data/materials.ron src/main.rs
git commit -m "feat: add materials resource and conversion database"
```

---

### Task 2: Reputation Resource

**Files:**
- Create: `src/reputation.rs`
- Modify: `src/main.rs` (add plugin)

**Step 1: Create the reputation module**

In `src/reputation.rs`:

```rust
use bevy::prelude::*;

/// Guild reputation. Earned from completing missions, gates recruit quality and mission access.
#[derive(Resource, Debug, Clone, Default, Deref, DerefMut)]
pub struct Reputation(pub u32);

/// Reputation tier thresholds — determines recruit quality and mission access.
impl Reputation {
    pub fn tier(&self) -> u32 {
        match self.0 {
            0..100 => 1,
            100..300 => 2,
            300..600 => 3,
            600..1000 => 4,
            _ => 5,
        }
    }
}

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<Reputation>();
}
```

**Step 2: Register in main.rs**

Add `mod reputation;` and `reputation::plugin` to the plugin list.

**Step 3: Verify compilation**

Run: `cargo build`

**Step 4: Commit**

```bash
git add src/reputation.rs src/main.rs
git commit -m "feat: add reputation resource with tier thresholds"
```

---

### Task 3: Update Mission Completion to Award Materials & Reputation

**Files:**
- Modify: `src/mission/data.rs` (add fields to MissionTemplate)
- Modify: `assets/data/mission_templates.ron` (add new fields)
- Modify: `src/mission/mod.rs` (update check_mission_completion system)

**Step 1: Add fields to MissionTemplate**

In `src/mission/data.rs`, add to `MissionTemplate`:

```rust
pub reputation_required: u32,
pub reputation_reward: u32,
pub material_drops: Vec<(MaterialType, u32, u32)>, // (type, min, max)
```

This requires importing `MaterialType` from `crate::materials`.

**Step 2: Update mission_templates.ron**

Add the new fields to each template. Example for goblin_cave:

```ron
reputation_required: 0,
reputation_reward: 10,
material_drops: [(IronOre, 1, 3), (RawLeather, 1, 2)],
```

For skeleton_crypt (medium difficulty):
```ron
reputation_required: 50,
reputation_reward: 25,
material_drops: [(IronOre, 2, 5), (RoughGems, 1, 3), (RawHerbs, 1, 2)],
```

For orc_stronghold (hard difficulty):
```ron
reputation_required: 150,
reputation_reward: 50,
material_drops: [(IronOre, 3, 6), (RawLeather, 2, 4), (Wood, 2, 5), (RoughGems, 1, 3)],
```

**Step 3: Update check_mission_completion**

In `src/mission/mod.rs`, find the `check_mission_completion` system. After the gold award, add material drops and reputation:

```rust
// After gold reward:
use crate::materials::{Materials, MaterialType};
use crate::reputation::Reputation;
use rand::Rng;

// Award materials
let mut rng = rand::thread_rng();
for &(mat_type, min, max) in &template.material_drops {
    let amount = rng.gen_range(min..=max);
    materials.add(mat_type, amount);
}

// Award reputation
reputation.0 += template.reputation_reward;
```

Add `ResMut<Materials>` and `ResMut<Reputation>` to the system parameters.

**Step 4: Filter mission board by reputation**

In `src/screens/missions.rs`, filter the `MissionTemplateDatabase` display to only show missions where `reputation.0 >= template.reputation_required`. Add `Res<Reputation>` to `spawn_mission_board` parameters.

**Step 5: Verify compilation**

Run: `cargo build`

**Step 6: Commit**

```bash
git add src/mission/data.rs src/mission/mod.rs src/screens/missions.rs assets/data/mission_templates.ron
git commit -m "feat: missions award materials and reputation on completion"
```

---

## Phase 2: Guild Buildings

### Task 4: Building Data & State

**Files:**
- Create: `src/buildings.rs`
- Create: `assets/data/buildings.ron`
- Modify: `src/main.rs` (add plugin)

**Step 1: Create the buildings module**

In `src/buildings.rs`:

```rust
use bevy::prelude::*;
use serde::Deserialize;
use std::collections::HashMap;

use crate::materials::MaterialType;

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

/// Cost to build or upgrade: gold + materials.
#[derive(Debug, Clone, Deserialize)]
pub struct BuildingCost {
    pub gold: u32,
    pub materials: Vec<(MaterialType, u32)>,
}

/// Per-building definition: costs per level.
#[derive(Debug, Clone, Deserialize)]
pub struct BuildingDef {
    pub id: BuildingType,
    pub level_costs: Vec<BuildingCost>, // index 0 = cost for Lv1, index 1 = cost for Lv2, etc.
    pub max_level: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BuildingsData {
    pub buildings: Vec<BuildingDef>,
}

/// Resource: building definitions loaded from RON.
#[derive(Resource, Debug, Clone)]
pub struct BuildingDatabase(pub Vec<BuildingDef>);

impl BuildingDatabase {
    pub fn get(&self, building: BuildingType) -> Option<&BuildingDef> {
        self.0.iter().find(|b| b.id == building)
    }
}

/// Resource: current building levels for the guild.
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
        let barracks_level = self.level(BuildingType::Barracks);
        3 + barracks_level * 2 // Base 3 + 2 per Barracks level
    }

    pub fn max_applicants(&self) -> u32 {
        let office_level = self.level(BuildingType::RecruitmentOffice);
        3 + office_level // Base 3 + 1 per Office level
    }
}

fn load_building_database(mut commands: Commands) {
    let data: BuildingsData =
        ron::from_str(include_str!("../assets/data/buildings.ron"))
            .expect("Failed to parse buildings.ron");
    commands.insert_resource(BuildingDatabase(data.buildings));
}

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<GuildBuildings>();
    app.add_systems(Startup, load_building_database);
}
```

**Step 2: Create buildings.ron**

In `assets/data/buildings.ron`:

```ron
(
    buildings: [
        (
            id: Armory,
            max_level: 3,
            level_costs: [
                (gold: 100, materials: [(IronOre, 5), (Wood, 5)]),
                (gold: 300, materials: [(SteelIngot, 5), (Lumber, 5)]),
                (gold: 700, materials: [(EnchantedSteel, 3), (ArcaneWood, 3)]),
            ],
        ),
        (
            id: TrainingGrounds,
            max_level: 3,
            level_costs: [
                (gold: 80, materials: [(Wood, 8)]),
                (gold: 250, materials: [(Lumber, 6), (IronOre, 4)]),
                (gold: 600, materials: [(ArcaneWood, 4), (SteelIngot, 3)]),
            ],
        ),
        (
            id: Barracks,
            max_level: 3,
            level_costs: [
                (gold: 60, materials: [(Wood, 6), (RawLeather, 3)]),
                (gold: 200, materials: [(Lumber, 5), (CuredLeather, 3)]),
                (gold: 500, materials: [(ArcaneWood, 3), (DragonLeather, 2)]),
            ],
        ),
        (
            id: RecruitmentOffice,
            max_level: 3,
            level_costs: [
                (gold: 50, materials: [(Wood, 4)]),
                (gold: 150, materials: [(Lumber, 4), (CutGem, 2)]),
                (gold: 400, materials: [(ArcaneWood, 3), (PrismaticGem, 1)]),
            ],
        ),
        (
            id: Workshop,
            max_level: 3,
            level_costs: [
                (gold: 75, materials: [(IronOre, 6), (Wood, 4)]),
                (gold: 250, materials: [(SteelIngot, 4), (Lumber, 4)]),
                (gold: 600, materials: [(EnchantedSteel, 3), (ArcaneWood, 3)]),
            ],
        ),
    ],
)
```

**Step 3: Register in main.rs**

Add `mod buildings;` and `buildings::plugin`.

**Step 4: Verify compilation**

Run: `cargo build`

**Step 5: Commit**

```bash
git add src/buildings.rs assets/data/buildings.ron src/main.rs
git commit -m "feat: add guild buildings resource and database"
```

---

### Task 5: Building Upgrade System

**Files:**
- Modify: `src/buildings.rs` (add upgrade event + system)

**Step 1: Add upgrade event and system**

In `src/buildings.rs`, add:

```rust
use crate::economy::Gold;

/// Event: request to upgrade a building.
#[derive(Event)]
pub struct UpgradeBuilding(pub BuildingType);

fn handle_upgrade_building(
    mut events: EventReader<UpgradeBuilding>,
    mut buildings: ResMut<GuildBuildings>,
    building_db: Res<BuildingDatabase>,
    mut gold: ResMut<Gold>,
    mut materials: ResMut<crate::materials::Materials>,
) {
    for event in events.read() {
        let building_type = event.0;
        let current_level = buildings.level(building_type);

        let Some(def) = building_db.get(building_type) else { continue };
        if current_level >= def.max_level { continue; }

        let cost = &def.level_costs[current_level as usize];

        // Check gold
        if gold.0 < cost.gold { continue; }

        // Check materials
        if !cost.materials.iter().all(|&(mat, amt)| materials.get(mat) >= amt) {
            continue;
        }

        // Deduct gold
        gold.0 -= cost.gold;

        // Deduct materials
        for &(mat, amt) in &cost.materials {
            materials.try_spend(mat, amt);
        }

        // Upgrade
        buildings.0.insert(building_type, current_level + 1);
    }
}
```

Update the plugin to add the event and system:

```rust
pub(super) fn plugin(app: &mut App) {
    app.init_resource::<GuildBuildings>();
    app.add_event::<UpgradeBuilding>();
    app.add_systems(Startup, load_building_database);
    app.add_systems(Update, handle_upgrade_building);
}
```

**Step 2: Verify compilation**

Run: `cargo build`

**Step 3: Commit**

```bash
git add src/buildings.rs
git commit -m "feat: add building upgrade event and handler"
```

---

## Phase 3: Equipment System

### Task 6: Equipment Data & Components

**Files:**
- Create: `src/equipment.rs`
- Create: `assets/data/equipment.ron`
- Modify: `src/main.rs` (add plugin)

**Step 1: Create the equipment module**

In `src/equipment.rs`:

```rust
use bevy::prelude::*;
use serde::Deserialize;

use crate::hero::data::HeroClass;
use crate::materials::MaterialType;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize, Reflect)]
pub enum GearSlot {
    Weapon,
    Armor,
    Accessory,
}

impl GearSlot {
    pub const ALL: &[GearSlot] = &[GearSlot::Weapon, GearSlot::Armor, GearSlot::Accessory];
}

/// Stat bonuses from a piece of gear.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct GearStats {
    #[serde(default)]
    pub attack: i32,
    #[serde(default)]
    pub defense: i32,
    #[serde(default)]
    pub hp: i32,
}

/// A single tier of gear in an upgrade path.
#[derive(Debug, Clone, Deserialize)]
pub struct GearTier {
    pub name: String,
    pub tier: u32,
    pub stats: GearStats,
    pub gold_cost: u32,
    pub material_cost: Vec<(MaterialType, u32)>,
    pub armory_level_required: u32,
}

/// A full upgrade path for one class + slot combination.
#[derive(Debug, Clone, Deserialize)]
pub struct GearPath {
    pub class: HeroClass,
    pub slot: GearSlot,
    pub tiers: Vec<GearTier>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct EquipmentData {
    pub paths: Vec<GearPath>,
}

/// Resource: all gear paths loaded from RON.
#[derive(Resource, Debug, Clone)]
pub struct EquipmentDatabase(pub Vec<GearPath>);

impl EquipmentDatabase {
    pub fn get_path(&self, class: HeroClass, slot: GearSlot) -> Option<&GearPath> {
        self.0.iter().find(|p| p.class == class && p.slot == slot)
    }
}

/// Component on hero entities: current gear per slot.
/// Stores the tier index (0 = no gear, 1 = tier 1, etc.)
#[derive(Component, Debug, Clone, Default, Reflect)]
pub struct HeroEquipment {
    pub weapon_tier: u32,
    pub armor_tier: u32,
    pub accessory_tier: u32,
}

impl HeroEquipment {
    pub fn tier(&self, slot: GearSlot) -> u32 {
        match slot {
            GearSlot::Weapon => self.weapon_tier,
            GearSlot::Armor => self.armor_tier,
            GearSlot::Accessory => self.accessory_tier,
        }
    }

    pub fn set_tier(&mut self, slot: GearSlot, tier: u32) {
        match slot {
            GearSlot::Weapon => self.weapon_tier = tier,
            GearSlot::Armor => self.armor_tier = tier,
            GearSlot::Accessory => self.accessory_tier = tier,
        }
    }

    /// Total stat bonuses from all equipped gear.
    pub fn total_stats(&self, db: &EquipmentDatabase, class: HeroClass) -> GearStats {
        let mut total = GearStats::default();
        for &slot in GearSlot::ALL {
            let tier = self.tier(slot);
            if tier > 0 {
                if let Some(path) = db.get_path(class, slot) {
                    if let Some(gear_tier) = path.tiers.get((tier - 1) as usize) {
                        total.attack += gear_tier.stats.attack;
                        total.defense += gear_tier.stats.defense;
                        total.hp += gear_tier.stats.hp;
                    }
                }
            }
        }
        total
    }
}

fn load_equipment_database(mut commands: Commands) {
    let data: EquipmentData =
        ron::from_str(include_str!("../assets/data/equipment.ron"))
            .expect("Failed to parse equipment.ron");
    commands.insert_resource(EquipmentDatabase(data.paths));
}

pub(super) fn plugin(app: &mut App) {
    app.add_systems(Startup, load_equipment_database);
}
```

**Step 2: Create equipment.ron**

In `assets/data/equipment.ron`, define paths for all 5 classes × 3 slots. Here's the full file (abbreviated for non-Warrior classes — follow the same pattern):

```ron
(
    paths: [
        // === WARRIOR ===
        (class: Warrior, slot: Weapon, tiers: [
            (name: "Iron Sword",       tier: 1, stats: (attack: 2, defense: 0, hp: 0), gold_cost: 30,  material_cost: [(IronOre, 3)],      armory_level_required: 1),
            (name: "Steel Sword",      tier: 2, stats: (attack: 4, defense: 0, hp: 0), gold_cost: 80,  material_cost: [(SteelIngot, 2)],   armory_level_required: 2),
            (name: "Enchanted Blade",  tier: 3, stats: (attack: 7, defense: 0, hp: 0), gold_cost: 200, material_cost: [(EnchantedSteel, 2), (CutGem, 1)], armory_level_required: 3),
        ]),
        (class: Warrior, slot: Armor, tiers: [
            (name: "Chainmail",    tier: 1, stats: (attack: 0, defense: 3, hp: 5),  gold_cost: 40,  material_cost: [(IronOre, 4)],      armory_level_required: 1),
            (name: "Plate Armor",  tier: 2, stats: (attack: 0, defense: 5, hp: 10), gold_cost: 100, material_cost: [(SteelIngot, 3)],   armory_level_required: 2),
            (name: "Runic Plate",  tier: 3, stats: (attack: 0, defense: 8, hp: 15), gold_cost: 250, material_cost: [(EnchantedSteel, 2), (PrismaticGem, 1)], armory_level_required: 3),
        ]),
        (class: Warrior, slot: Accessory, tiers: [
            (name: "Iron Shield",  tier: 1, stats: (attack: 0, defense: 2, hp: 3),  gold_cost: 25,  material_cost: [(IronOre, 2), (Wood, 2)],   armory_level_required: 1),
            (name: "Tower Shield", tier: 2, stats: (attack: 0, defense: 4, hp: 5),  gold_cost: 70,  material_cost: [(SteelIngot, 2), (Lumber, 1)], armory_level_required: 2),
            (name: "Aegis",        tier: 3, stats: (attack: 1, defense: 6, hp: 8),  gold_cost: 180, material_cost: [(EnchantedSteel, 1), (PrismaticGem, 1)], armory_level_required: 3),
        ]),

        // === ROGUE ===
        (class: Rogue, slot: Weapon, tiers: [
            (name: "Iron Daggers",     tier: 1, stats: (attack: 3, defense: 0, hp: 0), gold_cost: 30,  material_cost: [(IronOre, 2)],      armory_level_required: 1),
            (name: "Steel Daggers",    tier: 2, stats: (attack: 5, defense: 0, hp: 0), gold_cost: 80,  material_cost: [(SteelIngot, 2)],   armory_level_required: 2),
            (name: "Shadow Blades",    tier: 3, stats: (attack: 8, defense: 0, hp: 0), gold_cost: 200, material_cost: [(EnchantedSteel, 2)], armory_level_required: 3),
        ]),
        (class: Rogue, slot: Armor, tiers: [
            (name: "Leather Armor",    tier: 1, stats: (attack: 0, defense: 2, hp: 3),  gold_cost: 35,  material_cost: [(RawLeather, 4)],    armory_level_required: 1),
            (name: "Studded Leather",  tier: 2, stats: (attack: 0, defense: 3, hp: 5),  gold_cost: 90,  material_cost: [(CuredLeather, 3)],  armory_level_required: 2),
            (name: "Shadowweave",      tier: 3, stats: (attack: 1, defense: 5, hp: 8),  gold_cost: 220, material_cost: [(DragonLeather, 2), (CutGem, 1)], armory_level_required: 3),
        ]),
        (class: Rogue, slot: Accessory, tiers: [
            (name: "Smoke Bombs",      tier: 1, stats: (attack: 1, defense: 1, hp: 0), gold_cost: 20,  material_cost: [(RawHerbs, 3)],      armory_level_required: 1),
            (name: "Poison Vials",     tier: 2, stats: (attack: 2, defense: 1, hp: 0), gold_cost: 60,  material_cost: [(Potion, 2)],        armory_level_required: 2),
            (name: "Cloak of Shadows", tier: 3, stats: (attack: 3, defense: 3, hp: 3), gold_cost: 160, material_cost: [(DragonLeather, 1), (ElixirOfPower, 1)], armory_level_required: 3),
        ]),

        // === MAGE ===
        (class: Mage, slot: Weapon, tiers: [
            (name: "Crystal Staff",    tier: 1, stats: (attack: 3, defense: 0, hp: 0), gold_cost: 35,  material_cost: [(Wood, 2), (RoughGems, 2)], armory_level_required: 1),
            (name: "Arcane Staff",     tier: 2, stats: (attack: 5, defense: 0, hp: 0), gold_cost: 90,  material_cost: [(Lumber, 2), (CutGem, 2)], armory_level_required: 2),
            (name: "Staff of Power",   tier: 3, stats: (attack: 8, defense: 0, hp: 0), gold_cost: 220, material_cost: [(ArcaneWood, 2), (PrismaticGem, 1)], armory_level_required: 3),
        ]),
        (class: Mage, slot: Armor, tiers: [
            (name: "Enchanted Robes",     tier: 1, stats: (attack: 1, defense: 1, hp: 3),  gold_cost: 30,  material_cost: [(RawLeather, 2), (RawHerbs, 2)], armory_level_required: 1),
            (name: "Arcane Vestments",    tier: 2, stats: (attack: 2, defense: 2, hp: 5),  gold_cost: 85,  material_cost: [(CuredLeather, 2), (Potion, 1)], armory_level_required: 2),
            (name: "Robes of the Archmage", tier: 3, stats: (attack: 3, defense: 3, hp: 8), gold_cost: 210, material_cost: [(DragonLeather, 1), (ElixirOfPower, 1)], armory_level_required: 3),
        ]),
        (class: Mage, slot: Accessory, tiers: [
            (name: "Crystal Amulet",  tier: 1, stats: (attack: 2, defense: 0, hp: 2), gold_cost: 25,  material_cost: [(RoughGems, 3)],     armory_level_required: 1),
            (name: "Arcane Focus",    tier: 2, stats: (attack: 3, defense: 0, hp: 3), gold_cost: 70,  material_cost: [(CutGem, 2)],        armory_level_required: 2),
            (name: "Orb of Insight",  tier: 3, stats: (attack: 5, defense: 1, hp: 5), gold_cost: 180, material_cost: [(PrismaticGem, 2)],  armory_level_required: 3),
        ]),

        // === CLERIC ===
        (class: Cleric, slot: Weapon, tiers: [
            (name: "Iron Mace",       tier: 1, stats: (attack: 2, defense: 0, hp: 0), gold_cost: 30,  material_cost: [(IronOre, 3)],       armory_level_required: 1),
            (name: "Steel Mace",      tier: 2, stats: (attack: 3, defense: 0, hp: 0), gold_cost: 75,  material_cost: [(SteelIngot, 2)],    armory_level_required: 2),
            (name: "Holy Avenger",    tier: 3, stats: (attack: 5, defense: 0, hp: 5), gold_cost: 190, material_cost: [(EnchantedSteel, 1), (PrismaticGem, 1)], armory_level_required: 3),
        ]),
        (class: Cleric, slot: Armor, tiers: [
            (name: "Chain Vestments",  tier: 1, stats: (attack: 0, defense: 2, hp: 5),  gold_cost: 35,  material_cost: [(IronOre, 3), (RawLeather, 2)], armory_level_required: 1),
            (name: "Blessed Armor",    tier: 2, stats: (attack: 0, defense: 4, hp: 8),  gold_cost: 95,  material_cost: [(SteelIngot, 2), (CuredLeather, 2)], armory_level_required: 2),
            (name: "Divine Raiment",   tier: 3, stats: (attack: 0, defense: 6, hp: 12), gold_cost: 240, material_cost: [(EnchantedSteel, 1), (DragonLeather, 1), (ElixirOfPower, 1)], armory_level_required: 3),
        ]),
        (class: Cleric, slot: Accessory, tiers: [
            (name: "Herbal Pouch",    tier: 1, stats: (attack: 0, defense: 0, hp: 5),  gold_cost: 20,  material_cost: [(RawHerbs, 4)],     armory_level_required: 1),
            (name: "Healing Totem",   tier: 2, stats: (attack: 0, defense: 1, hp: 8),  gold_cost: 65,  material_cost: [(Potion, 2), (Lumber, 1)], armory_level_required: 2),
            (name: "Relic of Faith",  tier: 3, stats: (attack: 1, defense: 2, hp: 12), gold_cost: 170, material_cost: [(ElixirOfPower, 2)], armory_level_required: 3),
        ]),

        // === RANGER ===
        (class: Ranger, slot: Weapon, tiers: [
            (name: "Short Bow",       tier: 1, stats: (attack: 2, defense: 0, hp: 0), gold_cost: 25,  material_cost: [(Wood, 3), (RawLeather, 1)], armory_level_required: 1),
            (name: "Longbow",         tier: 2, stats: (attack: 4, defense: 0, hp: 0), gold_cost: 75,  material_cost: [(Lumber, 2), (CuredLeather, 1)], armory_level_required: 2),
            (name: "Windrunner Bow",  tier: 3, stats: (attack: 7, defense: 0, hp: 0), gold_cost: 200, material_cost: [(ArcaneWood, 2), (DragonLeather, 1)], armory_level_required: 3),
        ]),
        (class: Ranger, slot: Armor, tiers: [
            (name: "Hide Armor",      tier: 1, stats: (attack: 0, defense: 2, hp: 3),  gold_cost: 30,  material_cost: [(RawLeather, 4)],    armory_level_required: 1),
            (name: "Beastscale Armor", tier: 2, stats: (attack: 0, defense: 3, hp: 5), gold_cost: 80,  material_cost: [(CuredLeather, 3)],  armory_level_required: 2),
            (name: "Wildweave Armor", tier: 3, stats: (attack: 1, defense: 5, hp: 8),  gold_cost: 200, material_cost: [(DragonLeather, 2)], armory_level_required: 3),
        ]),
        (class: Ranger, slot: Accessory, tiers: [
            (name: "Herb Satchel",    tier: 1, stats: (attack: 1, defense: 0, hp: 3), gold_cost: 20,  material_cost: [(RawHerbs, 3), (RawLeather, 1)], armory_level_required: 1),
            (name: "Quiver of Plenty", tier: 2, stats: (attack: 2, defense: 0, hp: 5), gold_cost: 60, material_cost: [(CuredLeather, 2), (Potion, 1)], armory_level_required: 2),
            (name: "Wolf Companion",  tier: 3, stats: (attack: 3, defense: 2, hp: 5), gold_cost: 160, material_cost: [(DragonLeather, 1), (ElixirOfPower, 1)], armory_level_required: 3),
        ]),
    ],
)
```

**Step 3: Register in main.rs**

Add `mod equipment;` and `equipment::plugin`.

**Step 4: Verify compilation**

Run: `cargo build`

**Step 5: Commit**

```bash
git add src/equipment.rs assets/data/equipment.ron src/main.rs
git commit -m "feat: add equipment data, HeroEquipment component, and upgrade paths"
```

---

### Task 7: Equipment Crafting System + Integration with Combat

**Files:**
- Modify: `src/equipment.rs` (add craft event + system)
- Modify: `src/hero/mod.rs` (add HeroEquipment to hero spawning)
- Modify: `src/mission/entities.rs` (apply equipment bonuses to token combat stats)

**Step 1: Add crafting event and handler to equipment.rs**

```rust
use crate::buildings::{BuildingType, GuildBuildings};
use crate::economy::Gold;
use crate::materials::Materials;

/// Event: request to craft/upgrade gear for a hero.
#[derive(Event)]
pub struct CraftGear {
    pub hero: Entity,
    pub slot: GearSlot,
}

fn handle_craft_gear(
    mut events: EventReader<CraftGear>,
    mut heroes: Query<(&HeroInfo, &mut HeroEquipment)>,
    equipment_db: Res<EquipmentDatabase>,
    buildings: Res<GuildBuildings>,
    mut gold: ResMut<Gold>,
    mut materials: ResMut<Materials>,
) {
    for event in events.read() {
        let Ok((info, mut equip)) = heroes.get_mut(event.hero) else { continue };
        let current_tier = equip.tier(event.slot);
        let Some(path) = equipment_db.get_path(info.class, event.slot) else { continue };
        let Some(next) = path.tiers.get(current_tier as usize) else { continue };

        // Check armory level
        let armory_level = buildings.level(BuildingType::Armory);
        if armory_level < next.armory_level_required { continue; }

        // Check gold
        if gold.0 < next.gold_cost { continue; }

        // Check materials
        if !next.material_cost.iter().all(|&(mat, amt)| materials.get(mat) >= amt) {
            continue;
        }

        // Deduct costs
        gold.0 -= next.gold_cost;
        for &(mat, amt) in &next.material_cost {
            materials.try_spend(mat, amt);
        }

        // Upgrade
        equip.set_tier(event.slot, current_tier + 1);
    }
}
```

Update the plugin:

```rust
pub(super) fn plugin(app: &mut App) {
    app.add_event::<CraftGear>();
    app.add_systems(Startup, load_equipment_database);
    app.add_systems(Update, handle_craft_gear);
}
```

**Step 2: Add HeroEquipment to hero spawning**

In `src/hero/mod.rs`, in the `spawn_starter_heroes` system (and wherever heroes are spawned), add `HeroEquipment::default()` to the entity bundle.

**Step 3: Apply equipment bonuses in mission token creation**

In `src/mission/entities.rs`, when `CombatStats` are computed for hero tokens from `HeroStats`, look up the hero's `HeroEquipment` component and the `EquipmentDatabase` resource, compute `total_stats()`, and add the bonuses:

```rust
// After base combat stats are calculated:
if let Ok(equipment) = hero_equipment_query.get(hero_entity) {
    let gear_stats = equipment.total_stats(&equipment_db, info.class);
    combat_stats.attack += gear_stats.attack;
    combat_stats.defense += gear_stats.defense;
    combat_stats.max_hp += gear_stats.hp;
    combat_stats.hp += gear_stats.hp;
}
```

**Step 4: Verify compilation**

Run: `cargo build`

**Step 5: Commit**

```bash
git add src/equipment.rs src/hero/mod.rs src/mission/entities.rs
git commit -m "feat: add gear crafting and apply equipment bonuses to combat"
```

---

## Phase 4: Recruiting System

### Task 8: Applicant Board Core

**Files:**
- Create: `src/recruiting.rs`
- Modify: `src/main.rs` (add plugin)

**Step 1: Create the recruiting module**

In `src/recruiting.rs`:

```rust
use bevy::prelude::*;
use rand::Rng;

use crate::buildings::GuildBuildings;
use crate::economy::Gold;
use crate::hero::data::*;
use crate::hero::{Hero, HeroInfo, HeroStats, HeroTraits};
use crate::equipment::HeroEquipment;
use crate::reputation::Reputation;

/// A candidate hero on the applicant board.
#[derive(Debug, Clone)]
pub struct Applicant {
    pub name: String,
    pub class: HeroClass,
    pub traits: Vec<HeroTrait>,
    pub stats: HeroStats,
    pub hire_cost: u32,
    /// Time remaining in seconds before this applicant leaves.
    pub time_remaining: f32,
}

/// Resource: the applicant board.
#[derive(Resource, Debug, Default)]
pub struct ApplicantBoard {
    pub applicants: Vec<Applicant>,
    /// Timer until next applicant arrives (seconds).
    pub next_arrival_timer: f32,
}

/// Interval between new applicant arrivals (seconds).
/// 3600.0 = 1 hour. For testing, you may want to lower this.
const ARRIVAL_INTERVAL: f32 = 3600.0;

/// Min/max availability window for applicants (seconds).
const MIN_AVAILABILITY: f32 = 4.0 * 3600.0; // 4 hours
const MAX_AVAILABILITY: f32 = 8.0 * 3600.0; // 8 hours

fn tick_applicant_board(
    time: Res<Time>,
    mut board: ResMut<ApplicantBoard>,
    buildings: Res<GuildBuildings>,
    reputation: Res<Reputation>,
    class_db: Res<ClassDatabase>,
    trait_db: Res<TraitDatabase>,
    name_db: Res<NameDatabase>,
) {
    let dt = time.delta_secs();

    // Remove expired applicants
    board.applicants.retain_mut(|a| {
        a.time_remaining -= dt;
        a.time_remaining > 0.0
    });

    // Tick arrival timer
    board.next_arrival_timer -= dt;
    if board.next_arrival_timer <= 0.0 {
        board.next_arrival_timer = ARRIVAL_INTERVAL;

        let max_applicants = buildings.max_applicants() as usize;
        if board.applicants.len() < max_applicants {
            let applicant = generate_applicant(&reputation, &class_db, &trait_db, &name_db);
            board.applicants.push(applicant);
        }
    }
}

fn generate_applicant(
    reputation: &Reputation,
    class_db: &ClassDatabase,
    trait_db: &TraitDatabase,
    name_db: &NameDatabase,
) -> Applicant {
    let mut rng = rand::thread_rng();
    let tier = reputation.tier();

    // Pick random class
    let class_list = [HeroClass::Warrior, HeroClass::Rogue, HeroClass::Mage, HeroClass::Cleric, HeroClass::Ranger];
    let class = class_list[rng.gen_range(0..class_list.len())];
    let class_def = class_db.get(class).unwrap();

    // Pick 1-2 traits
    let all_traits = [HeroTrait::Brave, HeroTrait::Cautious, HeroTrait::Greedy, HeroTrait::Loner, HeroTrait::Leader, HeroTrait::Cursed, HeroTrait::Lucky];
    let num_traits = rng.gen_range(1..=2);
    let mut traits = Vec::new();
    let mut available: Vec<_> = all_traits.to_vec();
    for _ in 0..num_traits {
        if available.is_empty() { break; }
        let idx = rng.gen_range(0..available.len());
        traits.push(available.remove(idx));
    }

    // Generate name
    let first = &name_db.0.first_names[rng.gen_range(0..name_db.0.first_names.len())];
    let last = &name_db.0.surnames[rng.gen_range(0..name_db.0.surnames.len())];
    let name = format!("{} {}", first, last);

    // Roll stats with reputation-based floor
    // Higher tier = higher minimum stat rolls
    let stat_floor = 8 + (tier - 1) as i32; // 8, 9, 10, 11, 12
    let stat_ceiling = stat_floor + 2;

    let roll = |weight: i32| -> i32 {
        stat_floor + weight * rng.gen_range(1..=stat_ceiling - stat_floor)
    };

    let w = &class_def.stat_weights;
    let mut stats = HeroStats {
        strength: roll(w.strength),
        dexterity: roll(w.dexterity),
        constitution: roll(w.constitution),
        intelligence: roll(w.intelligence),
        wisdom: roll(w.wisdom),
        charisma: roll(w.charisma),
    };

    // Apply trait modifiers
    for t in &traits {
        if let Some(td) = trait_db.get(*t) {
            stats.strength += td.stat_modifiers.strength;
            stats.dexterity += td.stat_modifiers.dexterity;
            stats.constitution += td.stat_modifiers.constitution;
            stats.intelligence += td.stat_modifiers.intelligence;
            stats.wisdom += td.stat_modifiers.wisdom;
            stats.charisma += td.stat_modifiers.charisma;
        }
    }

    // Calculate hire cost based on stat total
    let stat_total = stats.strength + stats.dexterity + stats.constitution
        + stats.intelligence + stats.wisdom + stats.charisma;
    let hire_cost = 20 + (stat_total as u32) * 2;

    let availability = rng.gen_range(MIN_AVAILABILITY..=MAX_AVAILABILITY);

    Applicant {
        name,
        class,
        traits,
        stats,
        hire_cost,
        time_remaining: availability,
    }
}

/// Event: player hires an applicant by index.
#[derive(Event)]
pub struct HireApplicant(pub usize);

fn handle_hire_applicant(
    mut commands: Commands,
    mut events: EventReader<HireApplicant>,
    mut board: ResMut<ApplicantBoard>,
    mut gold: ResMut<Gold>,
    buildings: Res<GuildBuildings>,
    heroes: Query<Entity, With<Hero>>,
) {
    for event in events.read() {
        let idx = event.0;
        if idx >= board.applicants.len() { continue; }

        // Check roster cap
        let hero_count = heroes.iter().count() as u32;
        if hero_count >= buildings.roster_cap() { continue; }

        let applicant = &board.applicants[idx];

        // Check gold
        if gold.0 < applicant.hire_cost { continue; }

        // Deduct gold
        gold.0 -= applicant.hire_cost;

        // Spawn hero entity
        let applicant = board.applicants.remove(idx);
        commands.spawn((
            Hero,
            HeroInfo {
                name: applicant.name,
                class: applicant.class,
                level: 1,
                xp: 0,
                xp_to_next: 100,
            },
            applicant.stats,
            HeroTraits(applicant.traits),
            HeroEquipment::default(),
        ));
    }
}

/// System to seed the initial board on game start.
fn seed_applicant_board(
    mut board: ResMut<ApplicantBoard>,
    reputation: Res<Reputation>,
    class_db: Res<ClassDatabase>,
    trait_db: Res<TraitDatabase>,
    name_db: Res<NameDatabase>,
    buildings: Res<GuildBuildings>,
) {
    let max = buildings.max_applicants() as usize;
    // Start with 2 applicants
    let initial = 2.min(max);
    for _ in 0..initial {
        let applicant = generate_applicant(&reputation, &class_db, &trait_db, &name_db);
        board.applicants.push(applicant);
    }
    // Set first arrival timer
    board.next_arrival_timer = ARRIVAL_INTERVAL;
}

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<ApplicantBoard>();
    app.add_event::<HireApplicant>();
    app.add_systems(OnEnter(crate::screens::Screen::Gameplay), seed_applicant_board);
    app.add_systems(Update, (tick_applicant_board, handle_hire_applicant));
}
```

**Step 2: Register in main.rs**

Add `mod recruiting;` and `recruiting::plugin`.

**Step 3: Verify compilation**

Run: `cargo build`

**Step 4: Commit**

```bash
git add src/recruiting.rs src/main.rs
git commit -m "feat: add applicant board with timed arrivals and hiring"
```

---

## Phase 5: Training Grounds System

### Task 9: Passive XP for Idle Heroes

**Files:**
- Modify: `src/buildings.rs` or create `src/training.rs`

**Step 1: Create training system**

Create `src/training.rs`:

```rust
use bevy::prelude::*;

use crate::buildings::{BuildingType, GuildBuildings};
use crate::hero::{Hero, HeroInfo};
use crate::mission::OnMission;

/// Tick interval for training XP (seconds). Every 60 seconds, idle heroes gain XP.
const TRAINING_TICK_INTERVAL: f32 = 60.0;

#[derive(Resource, Debug)]
pub struct TrainingTimer(pub f32);

impl Default for TrainingTimer {
    fn default() -> Self {
        Self(TRAINING_TICK_INTERVAL)
    }
}

fn tick_training(
    time: Res<Time>,
    mut timer: ResMut<TrainingTimer>,
    buildings: Res<GuildBuildings>,
    mut heroes: Query<&mut HeroInfo, (With<Hero>, Without<OnMission>)>,
) {
    let level = buildings.level(BuildingType::TrainingGrounds);
    if level == 0 { return; }

    timer.0 -= time.delta_secs();
    if timer.0 > 0.0 { return; }
    timer.0 = TRAINING_TICK_INTERVAL;

    let xp_per_tick = level * 2; // 2/4/6 XP per minute at Lv 1/2/3

    for mut info in &mut heroes {
        info.xp += xp_per_tick;
        while info.xp >= info.xp_to_next {
            info.xp -= info.xp_to_next;
            info.level += 1;
            info.xp_to_next = 100 + (info.level as u32 - 1) * 25;
        }
    }
}

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<TrainingTimer>();
    app.add_systems(Update, tick_training);
}
```

**Step 2: Register in main.rs**

Add `mod training;` and `training::plugin`.

**Step 3: Verify compilation**

Run: `cargo build`

**Step 4: Commit**

```bash
git add src/training.rs src/main.rs
git commit -m "feat: add training grounds passive XP for idle heroes"
```

---

## Phase 6: Workshop Bulk Processing System

### Task 10: Workshop Conversion System

**Files:**
- Modify: `src/materials.rs` (add conversion event + system)

**Step 1: Add conversion event and handler**

In `src/materials.rs`, add:

```rust
use crate::buildings::{BuildingType, GuildBuildings};

/// Event: player requests a bulk material conversion.
#[derive(Event)]
pub struct ConvertMaterials {
    pub recipe_index: usize,
    pub quantity: u32, // How many times to run the recipe
}

fn handle_convert_materials(
    mut events: EventReader<ConvertMaterials>,
    mut materials: ResMut<Materials>,
    conversion_db: Res<ConversionDatabase>,
    buildings: Res<GuildBuildings>,
) {
    for event in events.read() {
        let Some(recipe) = conversion_db.0.get(event.recipe_index) else { continue };

        // Check workshop level
        let workshop_level = buildings.level(BuildingType::Workshop);
        if workshop_level < recipe.workshop_level_required { continue; }

        // Calculate max affordable quantity
        let available = materials.get(recipe.input_type);
        let max_runs = available / recipe.input_count;
        let runs = event.quantity.min(max_runs);
        if runs == 0 { continue; }

        // Process
        materials.try_spend(recipe.input_type, runs * recipe.input_count);
        materials.add(recipe.output_type, runs * recipe.output_count);
    }
}
```

Update the plugin:

```rust
pub(super) fn plugin(app: &mut App) {
    app.init_resource::<Materials>();
    app.add_event::<ConvertMaterials>();
    app.add_systems(Startup, load_materials_database);
    app.add_systems(Update, handle_convert_materials);
}
```

**Step 2: Verify compilation**

Run: `cargo build`

**Step 3: Commit**

```bash
git add src/materials.rs
git commit -m "feat: add bulk material conversion system for workshop"
```

---

## Phase 7: UI — New GameTab Screens

### Task 11: Add New GameTab Variants and Sidebar Navigation

**Files:**
- Modify: `src/screens/mod.rs` (add Guild, Armory, Recruiting to GameTab)
- Modify: `src/screens/sidebar.rs` (add nav buttons for new tabs, show reputation + materials summary)

**Step 1: Add GameTab variants**

In `src/screens/mod.rs`, add to the `GameTab` enum:

```rust
pub enum GameTab {
    Roster,
    Missions,
    PartySelect,
    MissionView,
    Guild,      // NEW
    Armory,     // NEW
    Recruiting, // NEW
}
```

**Step 2: Add sidebar nav buttons**

In `src/screens/sidebar.rs`:
- Add nav buttons for Guild, Armory, and Recruiting tabs (following existing pattern with `SidebarNavButton(GameTab::Guild)`, etc.)
- Add a reputation display below the gold display (similar pattern to `SidebarGoldText`)
- The existing disabled "Armory" button placeholder can be replaced with the real one

**Step 3: Verify compilation**

Run: `cargo build`

**Step 4: Commit**

```bash
git add src/screens/mod.rs src/screens/sidebar.rs
git commit -m "feat: add Guild, Armory, Recruiting tabs to sidebar navigation"
```

---

### Task 12: Guild Tab Screen (Buildings)

**Files:**
- Create: `src/screens/guild.rs`
- Modify: `src/screens/mod.rs` (register plugin)

**Step 1: Create guild screen**

In `src/screens/guild.rs`, build a screen that:
- Lists all 5 buildings with current level, description, and upgrade cost
- Each building has an "Upgrade" button that sends `UpgradeBuilding` event
- Button is disabled/greyed if insufficient gold/materials or at max level
- Follows the existing screen pattern: `OnEnter(GameTab::Guild)` spawns UI, `DespawnOnExit`
- Uses bevy_declarative layout matching the roster/missions style

Key layout:
```
┌─────────────────────────────────────┐
│ Guild Hall                          │
├─────────────────────────────────────┤
│ ┌─────────────────────────────────┐ │
│ │ Armory          Lv 1 / 3       │ │
│ │ Craft and upgrade equipment...  │ │
│ │ Upgrade to Lv 2: 300g + ...    │ │
│ │ [Upgrade]                       │ │
│ └─────────────────────────────────┘ │
│ ┌─────────────────────────────────┐ │
│ │ Training Grounds    Lv 0 / 3   │ │
│ │ Heroes gain passive XP...      │ │
│ │ Build: 80g + 8 Wood            │ │
│ │ [Build]                         │ │
│ └─────────────────────────────────┘ │
│ ...                                 │
└─────────────────────────────────────┘
```

**Step 2: Register in screens/mod.rs**

Add `mod guild;` and `guild::plugin` to the screens plugin list.

**Step 3: Verify compilation**

Run: `cargo build`

**Step 4: Commit**

```bash
git add src/screens/guild.rs src/screens/mod.rs
git commit -m "feat: add guild tab screen with building management"
```

---

### Task 13: Armory Tab Screen (Equipment Crafting)

**Files:**
- Create: `src/screens/armory.rs`
- Modify: `src/screens/mod.rs` (register plugin)

**Step 1: Create armory screen**

In `src/screens/armory.rs`, build a two-panel screen:

Left panel: hero list (only non-deployed heroes), click to select.
Right panel: selected hero's 3 gear slots with current tier, next upgrade name/cost, and "Craft" button.

Key layout:
```
┌──────────────┬──────────────────────────┐
│ Heroes       │ Gear — Aldric (Warrior)  │
│              │                          │
│ > Aldric  ★  │ Weapon: Iron Sword (T1)  │
│   Brin       │  → Steel Sword: 80g +    │
│   Cora       │    2 Steel Ingot         │
│              │  [Craft]                  │
│              │                          │
│              │ Armor: Chainmail (T1)     │
│              │  → Plate Armor: 100g +   │
│              │    3 Steel Ingot         │
│              │  [Craft]                  │
│              │                          │
│              │ Accessory: (none)         │
│              │  → Iron Shield: 25g +    │
│              │    2 Iron Ore, 2 Wood    │
│              │  [Craft]                  │
└──────────────┴──────────────────────────┘
```

Clicking "Craft" sends `CraftGear { hero, slot }` event.

**Step 2: Register in screens/mod.rs**

Add `mod armory;` and `armory::plugin`.

**Step 3: Verify compilation**

Run: `cargo build`

**Step 4: Commit**

```bash
git add src/screens/armory.rs src/screens/mod.rs
git commit -m "feat: add armory tab screen for equipment crafting"
```

---

### Task 14: Recruiting Tab Screen (Applicant Board)

**Files:**
- Create: `src/screens/recruiting.rs`
- Modify: `src/screens/mod.rs` (register plugin)

**Step 1: Create recruiting screen**

In `src/screens/recruiting.rs`, build a screen showing:

- List of current applicants with name, class, traits, key stats, hire cost, and time remaining
- "Hire" button per applicant (sends `HireApplicant(index)` event)
- Hire button disabled if insufficient gold or roster full
- Roster count display: "Heroes: 3/5"
- Time remaining displayed as "Xh Ym" countdown

Key layout:
```
┌─────────────────────────────────────┐
│ Applicant Board     Heroes: 3 / 5  │
├─────────────────────────────────────┤
│ ┌─────────────────────────────────┐ │
│ │ Kira Thornwall — Rogue          │ │
│ │ Traits: Brave, Lucky            │ │
│ │ STR 10  DEX 14  CON 9          │ │
│ │ INT 8   WIS 8   CHA 11         │ │
│ │ Cost: 85g      Leaves in: 6h 23m│ │
│ │ [Hire]                          │ │
│ └─────────────────────────────────┘ │
│ ┌─────────────────────────────────┐ │
│ │ Bram Ashford — Cleric           │ │
│ │ ...                             │ │
│ └─────────────────────────────────┘ │
└─────────────────────────────────────┘
```

**Step 2: Register in screens/mod.rs**

Add `mod recruiting;` and `recruiting::plugin` (note: this is the screen module, distinct from the `src/recruiting.rs` system module).

**Step 3: Verify compilation**

Run: `cargo build`

**Step 4: Commit**

```bash
git add src/screens/recruiting.rs src/screens/mod.rs
git commit -m "feat: add recruiting tab screen with applicant board UI"
```

---

### Task 15: Workshop Sub-Screen (in Guild Tab or standalone)

**Files:**
- Create: `src/screens/workshop.rs` OR integrate into `src/screens/guild.rs`

**Step 1: Build workshop UI**

The workshop can be a section within the Guild tab (if Workshop is built) or a separate tab. Recommended: section within Guild tab, shown when Workshop level >= 1.

Shows available conversion recipes (filtered by Workshop level), with:
- Input material icon/name + count per conversion
- Output material icon/name + count per conversion
- Current stock of input material
- Max conversions possible
- Quantity selector (slider or +/- buttons)
- "Convert" button (sends `ConvertMaterials` event)

**Step 2: Verify compilation**

Run: `cargo build`

**Step 3: Commit**

```bash
git add src/screens/guild.rs  # or workshop.rs
git commit -m "feat: add workshop bulk conversion UI to guild screen"
```

---

## Phase 8: Integration & Polish

### Task 16: Material & Reputation Display in Sidebar

**Files:**
- Modify: `src/screens/sidebar.rs`

**Step 1: Add reputation and material summary**

Below the gold display, add:
- Reputation display: "Rep: 150 (Tier 2)" — reactive, updates on change
- Compact material counts (show non-zero materials as small icons/labels)

**Step 2: Verify compilation and visual check**

Run: `cargo run` and verify sidebar displays new info.

**Step 3: Commit**

```bash
git add src/screens/sidebar.rs
git commit -m "feat: show reputation and materials in sidebar"
```

---

### Task 17: Toast Notifications for Guild Actions

**Files:**
- Modify: systems that handle events (buildings.rs, equipment.rs, recruiting.rs, materials.rs)

**Step 1: Add toast events on successful actions**

After successful building upgrade: toast "Armory upgraded to Lv 2!"
After successful craft: toast "Crafted Steel Sword for Aldric!"
After successful hire: toast "Kira Thornwall joined the guild!"
After successful conversion: toast "Converted 9 Iron Ore → 3 Steel Ingot"
After mission completion: toast "Mission complete! +30g, +3 Iron Ore, +2 Leather, +15 Rep"

Use the existing toast system in `src/ui/toast.rs`.

**Step 2: Verify compilation**

Run: `cargo build`

**Step 3: Commit**

```bash
git add src/buildings.rs src/equipment.rs src/recruiting.rs src/materials.rs src/mission/mod.rs
git commit -m "feat: add toast notifications for guild management actions"
```

---

### Task 18: Final Integration Test — Full Loop Playthrough

**Step 1: Manual playtest**

Run `cargo run` and verify the full loop:
1. Start game — 3 heroes, 2 applicants on board
2. Send heroes on goblin_cave mission
3. Mission completes — verify gold, materials, reputation awarded
4. Check applicant board — verify candidates with countdown timers
5. Build Workshop (if enough materials from starter gold)
6. Convert materials at Workshop
7. Build Armory
8. Craft first weapon for a hero
9. Hire an applicant
10. Send upgraded hero on next mission — verify gear bonuses apply
11. Build Training Grounds — verify idle heroes gain XP over time

**Step 2: Fix any issues found**

**Step 3: Final commit**

```bash
git add -A
git commit -m "fix: integration fixes from guild management playtest"
```
