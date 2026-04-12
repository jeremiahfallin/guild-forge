# Save System & Offline Time Bank — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Persist all game state to a RON save file and let players spend accumulated offline time via 1x/2x/3x speed controls.

**Architecture:** New `src/save.rs` module handles serialization/deserialization via DTO structs that mirror ECS state. New `src/time_bank.rs` module manages `OfflineTimeBank` and `GameSpeed` resources plus the fuel-drain tick system. Speed control UI added to the sidebar. Entity references (hero Entity IDs) are resolved via index-mapping during save/load.

**Tech Stack:** Bevy 0.18, serde + RON (already in Cargo.toml), `dirs` crate (new dependency for save file path), `std::time::SystemTime` for timestamps.

---

### Task 1: Add `Serialize` derives to existing types

**Files:**
- Modify: `src/hero/data.rs` — add `Serialize` to `HeroClass`, `HeroTrait`
- Modify: `src/mission/data.rs` — add `Serialize` to `EnemyType`
- Modify: `src/mission/dungeon.rs` — add `Serialize` to `Tile`, `RoomType`, `Room`, `DungeonMap`
- Modify: `src/mission/mod.rs` — add `Serialize` to `MissionProgress`
- Modify: `src/materials.rs` — add `Serialize` to `MaterialType`
- Modify: `src/buildings.rs` — add `Serialize` to `BuildingType`

These types already have `Deserialize`. We just need to add `Serialize` so they can be written to the save file.

**Step 1: Add `use serde::Serialize` and the derive to each file**

`src/hero/data.rs` — find the `HeroClass` and `HeroTrait` enums. They currently derive `Deserialize`. Add `Serialize` next to it. You will need to add `use serde::Serialize;` (or change the existing `use serde::Deserialize;` to `use serde::{Deserialize, Serialize};`).

```rust
// HeroClass — currently:
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize, Reflect)]
// Change to:
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Reflect)]

// HeroTrait — same change
```

`src/mission/data.rs` — `EnemyType` enum. Add `Serialize`. Add `use serde::Serialize;` or update existing import.

```rust
// Currently:
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize, Reflect)]
pub enum EnemyType { ... }
// Change to:
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Reflect)]
```

`src/mission/dungeon.rs` — `Tile`, `RoomType`, `Room`, `DungeonMap`. These have NO serde derives currently. Add both `Serialize, Deserialize` and a `use serde::{Serialize, Deserialize};` import.

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Reflect)]
pub enum Tile { Wall, Floor, Door, Corridor }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Reflect)]
pub enum RoomType { Normal, Entrance, Boss, Treasure }

#[derive(Debug, Clone, Serialize, Deserialize, Reflect)]
pub struct Room { ... }

#[derive(Component, Debug, Clone, Serialize, Deserialize, Reflect)]
pub struct DungeonMap { ... }
```

`src/mission/mod.rs` — `MissionProgress` enum. Add `Serialize, Deserialize`. Add `use serde::{Serialize, Deserialize};`.

```rust
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Reflect)]
pub enum MissionProgress { InProgress, Complete, Failed }
```

`src/materials.rs` — `MaterialType` already has `Deserialize`. Add `Serialize`.

```rust
// Currently:
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize, Reflect)]
pub enum MaterialType { ... }
// Change to:
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Reflect)]
```

`src/buildings.rs` — `BuildingType` already has `Deserialize`. Add `Serialize`.

```rust
// Currently:
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize, Reflect)]
pub enum BuildingType { ... }
// Change to:
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Reflect)]
```

**Step 2: Verify it compiles**

Run: `cargo build 2>&1 | head -20`
Expected: Compiles with no new errors. There may be a linker error if the game is running — that's fine, look only for `error[` lines.

**Step 3: Commit**

```bash
git add src/hero/data.rs src/mission/data.rs src/mission/dungeon.rs src/mission/mod.rs src/materials.rs src/buildings.rs
git commit -m "feat(save): add Serialize derives to types needed for save system"
```

---

### Task 2: Add `dirs` crate and create save module with DTO types

**Files:**
- Modify: `Cargo.toml` — add `dirs = "6"` dependency
- Create: `src/save.rs` — DTO structs, SaveData, save path helper

**Step 1: Add `dirs` dependency**

Add to `[dependencies]` in `Cargo.toml`:
```toml
dirs = "6"
```

**Step 2: Create `src/save.rs` with DTO types and save path helper**

```rust
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

/// Top-level save data structure.
#[derive(Serialize, Deserialize)]
pub struct SaveData {
    pub last_save_timestamp: u64,

    // Resources
    pub gold: u32,
    pub reputation: u32,
    pub banked_seconds: f32,

    // Materials
    pub materials: HashMap<MaterialType, u32>,

    // Buildings
    pub buildings: HashMap<BuildingType, u32>,

    // Hero roster
    pub heroes: Vec<HeroSaveDto>,

    // Applicant board
    pub applicants: Vec<ApplicantSaveDto>,
    pub next_arrival_timer: f32,

    // Training
    pub training_timer: f32,

    // Missions in progress
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

    /// Party: indices into the `heroes` vec in SaveData.
    pub party_hero_indices: Vec<usize>,

    // Dungeon
    pub dungeon_map: DungeonMap,
    pub room_visited: Vec<bool>,
    pub room_cleared: Vec<bool>,

    // Tokens
    pub hero_tokens: Vec<HeroTokenDto>,
    pub enemy_tokens: Vec<EnemyTokenDto>,
}

#[derive(Serialize, Deserialize)]
pub struct HeroTokenDto {
    /// Index into the `heroes` vec in SaveData.
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
```

**Step 3: Register mod in `src/main.rs`**

Add `mod save;` to the mod declarations (alphabetically near `recruiting`).

**Step 4: Verify it compiles**

Run: `cargo build 2>&1 | head -20`
Expected: Compiles successfully.

**Step 5: Commit**

```bash
git add Cargo.toml src/save.rs src/main.rs
git commit -m "feat(save): add save module with DTO types and dirs dependency"
```

---

### Task 3: Implement save (collect + serialize + write)

**Files:**
- Modify: `src/save.rs` — add collection logic, serialization, write-to-disk, plugin, save triggers

This task adds the `perform_save` system that collects all ECS state into `SaveData`, serializes to RON, and writes to disk. It also adds autosave timer, manual save event, and save-on-quit.

**Step 1: Add the save collection and write logic to `src/save.rs`**

Add these imports at the top of `src/save.rs`:

```rust
use bevy::prelude::*;

use crate::buildings::GuildBuildings;
use crate::economy::Gold;
use crate::equipment::HeroEquipment;
use crate::hero::{Hero, HeroInfo, HeroStats, HeroTraits};
use crate::mission::{Mission, MissionDungeon, MissionInfo, MissionParty, OnMission};
use crate::mission::entities::*;
use crate::recruiting::ApplicantBoard;
use crate::reputation::Reputation;
use crate::training::TrainingTimer;
```

Add these types and systems after the DTOs:

```rust
/// Resource: autosave countdown timer.
#[derive(Resource)]
pub struct AutosaveTimer(pub f32);

impl Default for AutosaveTimer {
    fn default() -> Self {
        Self(300.0) // 5 minutes
    }
}

/// Event: manually trigger a save (e.g., from pause menu).
#[derive(Event)]
pub struct ManualSave;

/// Collects all ECS state into a SaveData and writes it to disk.
#[allow(clippy::too_many_arguments)]
fn perform_save(
    mut commands: Commands,
    gold: Res<Gold>,
    reputation: Res<Reputation>,
    materials: Res<crate::materials::Materials>,
    buildings: Res<GuildBuildings>,
    training_timer: Res<TrainingTimer>,
    board: Res<ApplicantBoard>,
    bank: Res<crate::time_bank::OfflineTimeBank>,
    heroes: Query<(Entity, &HeroInfo, &HeroStats, &HeroTraits, &HeroEquipment, Option<&OnMission>), With<Hero>>,
    missions: Query<
        (Entity, &MissionInfo, &MissionProgress, &MissionParty, &MissionDungeon, &RoomStatus, &Children),
        With<Mission>,
    >,
    hero_tokens: Query<(&HeroToken, &GridPosition, &CombatStats, &InRoom, Option<&MoveTarget>), Without<EnemyToken>>,
    enemy_tokens: Query<(&EnemyToken, &GridPosition, &CombatStats, &InRoom), Without<HeroToken>>,
) {
    // Build hero roster — order matters for index references
    let mut hero_entities: Vec<Entity> = Vec::new();
    let mut hero_dtos: Vec<HeroSaveDto> = Vec::new();

    for (entity, info, stats, traits_comp, equipment, on_mission) in &heroes {
        hero_entities.push(entity);
        hero_dtos.push(HeroSaveDto {
            name: info.name.clone(),
            class: info.class,
            level: info.level,
            xp: info.xp,
            xp_to_next: info.xp_to_next,
            stats: HeroStatsSave {
                strength: stats.strength,
                dexterity: stats.dexterity,
                constitution: stats.constitution,
                intelligence: stats.intelligence,
                wisdom: stats.wisdom,
                charisma: stats.charisma,
            },
            traits: traits_comp.0.clone(),
            equipment: HeroEquipmentSave {
                weapon_tier: equipment.weapon_tier,
                armor_tier: equipment.armor_tier,
                accessory_tier: equipment.accessory_tier,
            },
            on_mission: on_mission.is_some(),
        });
    }

    // Helper: find hero index by entity
    let hero_index = |entity: Entity| -> Option<usize> {
        hero_entities.iter().position(|&e| e == entity)
    };

    // Applicants
    let applicant_dtos: Vec<ApplicantSaveDto> = board.applicants.iter().map(|a| {
        ApplicantSaveDto {
            name: a.name.clone(),
            class: a.class,
            traits: a.traits.clone(),
            stats: HeroStatsSave {
                strength: a.stats.strength,
                dexterity: a.stats.dexterity,
                constitution: a.stats.constitution,
                intelligence: a.stats.intelligence,
                wisdom: a.stats.wisdom,
                charisma: a.stats.charisma,
            },
            hire_cost: a.hire_cost,
            time_remaining: a.time_remaining,
        }
    }).collect();

    // Missions
    let mut mission_dtos: Vec<MissionSaveDto> = Vec::new();
    for (_entity, info, progress, party, dungeon, room_status, children) in &missions {
        let party_indices: Vec<usize> = party.0.iter()
            .filter_map(|&e| hero_index(e))
            .collect();

        let mut h_tokens: Vec<HeroTokenDto> = Vec::new();
        let mut e_tokens: Vec<EnemyTokenDto> = Vec::new();

        for child in children.iter() {
            if let Ok((ht, grid, combat, in_room, move_target)) = hero_tokens.get(child) {
                h_tokens.push(HeroTokenDto {
                    roster_index: hero_index(ht.0).unwrap_or(0),
                    grid_x: grid.x,
                    grid_y: grid.y,
                    in_room: in_room.0,
                    hp: combat.hp,
                    max_hp: combat.max_hp,
                    attack: combat.attack,
                    defense: combat.defense,
                    path: move_target.map(|mt| mt.path.clone()),
                    path_index: move_target.map_or(0, |mt| mt.path_index),
                });
            }
            if let Ok((et, grid, combat, in_room)) = enemy_tokens.get(child) {
                e_tokens.push(EnemyTokenDto {
                    enemy_type: et.enemy_type,
                    xp_reward: et.xp_reward,
                    grid_x: grid.x,
                    grid_y: grid.y,
                    in_room: in_room.0,
                    hp: combat.hp,
                    max_hp: combat.max_hp,
                    attack: combat.attack,
                    defense: combat.defense,
                });
            }
        }

        mission_dtos.push(MissionSaveDto {
            template_id: info.template_id.clone(),
            name: info.name.clone(),
            difficulty: info.difficulty,
            progress: *progress,
            rng_seed: 0, // TODO: capture actual RNG seed if we add seeded RNG later
            party_hero_indices: party_indices,
            dungeon_map: dungeon.0.clone(),
            room_visited: room_status.visited.clone(),
            room_cleared: room_status.cleared.clone(),
            hero_tokens: h_tokens,
            enemy_tokens: e_tokens,
        });
    }

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let save_data = SaveData {
        last_save_timestamp: now,
        gold: gold.0,
        reputation: reputation.0,
        banked_seconds: bank.banked_seconds,
        materials: materials.0.clone(),
        buildings: buildings.0.clone(),
        heroes: hero_dtos,
        applicants: applicant_dtos,
        next_arrival_timer: board.next_arrival_timer,
        training_timer: training_timer.0,
        missions: mission_dtos,
    };

    // Serialize and write
    match ron::ser::to_string_pretty(&save_data, ron::ser::PrettyConfig::default()) {
        Ok(ron_string) => {
            if let Some(path) = save_file_path() {
                if let Some(parent) = path.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                match std::fs::write(&path, &ron_string) {
                    Ok(()) => {
                        info!("Game saved to {}", path.display());
                        commands.trigger(crate::ui::toast::ToastEvent {
                            title: "Game saved.".to_string(),
                            body: String::new(),
                            kind: crate::ui::toast::ToastKind::Info,
                        });
                    }
                    Err(e) => warn!("Failed to write save file: {e}"),
                }
            }
        }
        Err(e) => warn!("Failed to serialize save data: {e}"),
    }
}

/// Tick the autosave timer, trigger save when it expires.
fn tick_autosave(
    time: Res<Time>,
    mut timer: ResMut<AutosaveTimer>,
    mut commands: Commands,
) {
    timer.0 -= time.delta_secs();
    if timer.0 <= 0.0 {
        timer.0 = 300.0;
        commands.run_system_cached(perform_save);
    }
}

/// Handle manual save event.
fn handle_manual_save(
    _trigger: On<ManualSave>,
    mut commands: Commands,
) {
    commands.run_system_cached(perform_save);
}

/// Save on app exit.
fn save_on_exit(
    _trigger: On<AppExit>,
    mut commands: Commands,
) {
    commands.run_system_cached(perform_save);
}
```

Note: `perform_save` references `crate::time_bank::OfflineTimeBank` which doesn't exist yet. This task depends on Task 5 existing at least as a stub. We'll handle this by creating a minimal stub in Task 5 first, or by adding this system after Task 5. **The implementation agent should create Task 5's resource stubs before compiling this task.**

Actually, let's restructure: we'll add a temporary `OfflineTimeBank` stub in this task that Task 5 will flesh out. Add to the bottom of `src/save.rs` before the plugin:

We won't do that — instead, implement Task 5 (time_bank resources) before Task 3's save logic that references it. The agent should follow this ordering:
1. Task 1 (derives)
2. Task 2 (DTOs only — no systems yet)
3. Task 5 (time_bank resources — just the resource structs)
4. Task 3 (save logic)
5. Task 4 (load logic)
6. Task 6 (time_bank tick system)
7. Task 7 (speed control UI)
8. Task 8 (plugin wiring + integration)

**Step 2: Add plugin function**

```rust
pub(super) fn plugin(app: &mut App) {
    app.init_resource::<AutosaveTimer>();
    app.add_observer(handle_manual_save);
    app.add_observer(save_on_exit);
    app.add_systems(
        Update,
        tick_autosave.run_if(in_state(crate::screens::Screen::Gameplay)),
    );
}
```

**Step 3: Verify it compiles** (after Task 5 resources exist)

Run: `cargo build 2>&1 | head -20`

**Step 4: Commit**

```bash
git add src/save.rs
git commit -m "feat(save): implement save collection, serialization, and autosave"
```

---

### Task 4: Implement load (deserialize + spawn)

**Files:**
- Modify: `src/save.rs` — add load_save_file function and load system
- Modify: `src/hero/mod.rs` — make `spawn_starter_heroes` skip if save was loaded

**Step 1: Add load logic to `src/save.rs`**

```rust
/// Resource: signals that a save was loaded (prevents starter hero spawn).
#[derive(Resource)]
pub struct SaveLoaded;

/// Attempt to load a save file and restore game state.
/// Called on `OnEnter(Screen::Gameplay)`.
#[allow(clippy::too_many_arguments)]
fn load_save(
    mut commands: Commands,
    existing_heroes: Query<(), With<Hero>>,
) {
    // Only attempt load if no heroes exist yet (first entry into gameplay)
    if !existing_heroes.is_empty() {
        return;
    }

    let Some(path) = save_file_path() else { return };
    let Ok(contents) = std::fs::read_to_string(&path) else { return };
    let save_data: SaveData = match ron::from_str(&contents) {
        Ok(d) => d,
        Err(e) => {
            warn!("Failed to parse save file: {e}");
            return;
        }
    };

    info!("Loading save from {}", path.display());

    // Mark that we loaded a save (skip starter heroes)
    commands.insert_resource(SaveLoaded);

    // Restore resources
    commands.insert_resource(Gold(save_data.gold));
    commands.insert_resource(Reputation(save_data.reputation));
    commands.insert_resource(crate::materials::Materials(save_data.materials));
    commands.insert_resource(GuildBuildings(save_data.buildings));
    commands.insert_resource(TrainingTimer(save_data.training_timer));
    commands.insert_resource(crate::time_bank::OfflineTimeBank {
        banked_seconds: save_data.banked_seconds,
    });

    // Restore applicant board
    let applicants: Vec<crate::recruiting::Applicant> = save_data.applicants.into_iter().map(|a| {
        crate::recruiting::Applicant {
            name: a.name,
            class: a.class,
            traits: a.traits,
            stats: HeroStats {
                strength: a.stats.strength,
                dexterity: a.stats.dexterity,
                constitution: a.stats.constitution,
                intelligence: a.stats.intelligence,
                wisdom: a.stats.wisdom,
                charisma: a.stats.charisma,
            },
            hire_cost: a.hire_cost,
            time_remaining: a.time_remaining,
        }
    }).collect();
    commands.insert_resource(ApplicantBoard {
        applicants,
        next_arrival_timer: save_data.next_arrival_timer,
    });

    // Spawn heroes — track entity IDs for mission cross-references
    let mut hero_entities: Vec<Entity> = Vec::new();
    for dto in &save_data.heroes {
        let entity = commands.spawn((
            Name::new(dto.name.clone()),
            Hero,
            HeroInfo {
                name: dto.name.clone(),
                class: dto.class,
                level: dto.level,
                xp: dto.xp,
                xp_to_next: dto.xp_to_next,
            },
            HeroStats {
                strength: dto.stats.strength,
                dexterity: dto.stats.dexterity,
                constitution: dto.stats.constitution,
                intelligence: dto.stats.intelligence,
                wisdom: dto.stats.wisdom,
                charisma: dto.stats.charisma,
            },
            HeroTraits(dto.traits.clone()),
            HeroEquipment {
                weapon_tier: dto.equipment.weapon_tier,
                armor_tier: dto.equipment.armor_tier,
                accessory_tier: dto.equipment.accessory_tier,
            },
        )).id();
        hero_entities.push(entity);
    }

    // Spawn missions with tokens as children
    for mission_dto in &save_data.missions {
        let party_entities: Vec<Entity> = mission_dto.party_hero_indices.iter()
            .filter_map(|&idx| hero_entities.get(idx).copied())
            .collect();

        let mission_entity = commands.spawn((
            Name::new(format!("Mission: {}", mission_dto.name)),
            Mission,
            MissionInfo {
                template_id: mission_dto.template_id.clone(),
                name: mission_dto.name.clone(),
                difficulty: mission_dto.difficulty,
            },
            mission_dto.progress,
            MissionParty(party_entities.clone()),
            MissionDungeon(mission_dto.dungeon_map.clone()),
            RoomStatus {
                visited: mission_dto.room_visited.clone(),
                cleared: mission_dto.room_cleared.clone(),
            },
        )).id();

        // Mark heroes as on-mission
        for &hero_entity in &party_entities {
            commands.entity(hero_entity).insert(OnMission(mission_entity));
        }

        // Spawn hero tokens as children
        for ht in &mission_dto.hero_tokens {
            let roster_entity = hero_entities.get(ht.roster_index).copied().unwrap_or(Entity::PLACEHOLDER);
            let mut token = commands.spawn((
                Name::new("Hero Token (loaded)"),
                HeroToken(roster_entity),
                GridPosition { x: ht.grid_x, y: ht.grid_y },
                InRoom(ht.in_room),
                CombatStats {
                    hp: ht.hp,
                    max_hp: ht.max_hp,
                    attack: ht.attack,
                    defense: ht.defense,
                },
                ChildOf(mission_entity),
            ));
            if let Some(ref path) = ht.path {
                if ht.path_index < path.len() {
                    token.insert(MoveTarget {
                        path: path.clone(),
                        path_index: ht.path_index,
                    });
                }
            }
        }

        // Spawn enemy tokens as children
        for et in &mission_dto.enemy_tokens {
            commands.spawn((
                Name::new("Enemy Token (loaded)"),
                EnemyToken {
                    enemy_type: et.enemy_type,
                    xp_reward: et.xp_reward,
                },
                GridPosition { x: et.grid_x, y: et.grid_y },
                InRoom(et.in_room),
                CombatStats {
                    hp: et.hp,
                    max_hp: et.max_hp,
                    attack: et.attack,
                    defense: et.defense,
                },
                ChildOf(mission_entity),
            ));
        }
    }

    // Calculate offline time and add to bank
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let elapsed = now.saturating_sub(save_data.last_save_timestamp) as f32;
    let new_banked = (save_data.banked_seconds + elapsed).min(86400.0);
    commands.insert_resource(crate::time_bank::OfflineTimeBank {
        banked_seconds: new_banked,
    });

    info!("Save loaded — {} heroes, {} missions, {:.0}s banked time",
        hero_entities.len(), save_data.missions.len(), new_banked);

    commands.trigger(crate::ui::toast::ToastEvent {
        title: "Save loaded!".to_string(),
        body: format!("Banked time: {}", crate::time_bank::format_banked_time(new_banked)),
        kind: crate::ui::toast::ToastKind::Info,
    });
}
```

**Step 2: Register load system in plugin**

Update the plugin function in `src/save.rs`:

```rust
pub(super) fn plugin(app: &mut App) {
    app.init_resource::<AutosaveTimer>();
    app.add_observer(handle_manual_save);
    app.add_observer(save_on_exit);
    app.add_systems(
        OnEnter(crate::screens::Screen::Gameplay),
        load_save.before(crate::hero::spawn_starter_heroes_label),
    );
    app.add_systems(
        Update,
        tick_autosave.run_if(in_state(crate::screens::Screen::Gameplay)),
    );
}
```

**Step 3: Make starter hero spawn respect SaveLoaded**

In `src/hero/mod.rs`, modify `spawn_starter_heroes` to also check for `SaveLoaded`:

```rust
fn spawn_starter_heroes(
    mut commands: Commands,
    existing_heroes: Query<(), With<Hero>>,
    save_loaded: Option<Res<crate::save::SaveLoaded>>,
    class_db: Res<ClassDatabase>,
    trait_db: Res<TraitDatabase>,
    name_db: Res<NameDatabase>,
) {
    if !existing_heroes.is_empty() || save_loaded.is_some() {
        return;
    }
    // ... rest unchanged
}
```

Also in `src/hero/mod.rs`, make the system public with a label so save can order against it:

```rust
pub(super) fn plugin(app: &mut App) {
    app.add_systems(Startup, load_hero_databases);
    app.add_systems(OnEnter(Screen::Gameplay), spawn_starter_heroes);
}
```

The `load_save` system uses `.before()` ordering, but since both run on `OnEnter(Screen::Gameplay)`, Bevy will run them in the same schedule entry. We need the load to run first. The simplest approach: check `existing_heroes.is_empty()` in spawn_starter_heroes already works because load_save spawns heroes via `Commands`, which are applied at end of stage. So actually we need `SaveLoaded` resource check in spawn_starter_heroes, which is set by load_save in the same stage. Commands are deferred, but `insert_resource` via commands is also deferred...

Better approach: have load_save run in a separate system set or use a flag. The `SaveLoaded` resource won't be visible in the same frame since commands are deferred. Instead, load_save should spawn heroes directly, and spawn_starter_heroes should check `existing_heroes.is_empty()` — which it already does. Since both run in OnEnter and commands are deferred, hero entities from load won't exist yet when spawn_starter_heroes runs.

**Revised approach:** Use system ordering. Have load_save add a `SaveLoaded` resource **non-deferred** using `commands.insert_resource()` won't work... Actually in Bevy 0.18, `OnEnter` systems run in sequence within the same schedule pass. We can use `.chain()` or ordering.

Simplest fix: make `spawn_starter_heroes` and `seed_applicant_board` run **after** load_save by using ordering in the save plugin. Or: load_save sets a `Local<bool>` ... no, that doesn't cross systems.

**Final approach:** The load system should be an exclusive system that directly inserts resources and spawns entities. Or more practically: have the save module expose a `has_save_file()` check, and have `spawn_starter_heroes` call it.

```rust
// In src/save.rs:
/// Returns true if a save file exists.
pub fn has_save_file() -> bool {
    save_file_path().is_some_and(|p| p.exists())
}
```

Then in `spawn_starter_heroes`:
```rust
fn spawn_starter_heroes(
    mut commands: Commands,
    existing_heroes: Query<(), With<Hero>>,
    class_db: Res<ClassDatabase>,
    trait_db: Res<TraitDatabase>,
    name_db: Res<NameDatabase>,
) {
    if !existing_heroes.is_empty() || crate::save::has_save_file() {
        return;
    }
    // ... rest unchanged
}
```

And similarly in `recruiting.rs` `seed_applicant_board`:
```rust
fn seed_applicant_board(...) {
    if !board.applicants.is_empty() || crate::save::has_save_file() {
        return;
    }
    // ... rest unchanged
}
```

This way, if a save file exists, the load system restores everything and the starter systems no-op. If no save file exists, normal startup proceeds.

**Step 4: Verify it compiles**

Run: `cargo build 2>&1 | head -20`

**Step 5: Commit**

```bash
git add src/save.rs src/hero/mod.rs src/recruiting.rs
git commit -m "feat(save): implement load system with entity restoration and offline time calc"
```

---

### Task 5: Create time_bank module with resources

**Files:**
- Create: `src/time_bank.rs` — OfflineTimeBank, GameSpeed resources, format helper, tick system
- Modify: `src/main.rs` — add `mod time_bank;` and register plugin

**Important:** This task should be implemented BEFORE Task 3, since Task 3 references `crate::time_bank::OfflineTimeBank`.

**Step 1: Create `src/time_bank.rs`**

```rust
//! Offline time bank and game speed control.

use bevy::prelude::*;

/// Banked offline seconds, capped at 86,400 (24 hours).
#[derive(Resource, Debug)]
pub struct OfflineTimeBank {
    pub banked_seconds: f32,
}

impl Default for OfflineTimeBank {
    fn default() -> Self {
        Self { banked_seconds: 0.0 }
    }
}

/// Current game speed multiplier (1.0, 2.0, or 3.0).
#[derive(Resource, Debug)]
pub struct GameSpeed(pub f32);

impl Default for GameSpeed {
    fn default() -> Self {
        Self(1.0)
    }
}

/// Format banked seconds as a human-readable string.
/// Under 1 hour: "Xm Ys", otherwise "Xh Ym".
pub fn format_banked_time(seconds: f32) -> String {
    let total_secs = seconds as u32;
    let hours = total_secs / 3600;
    let minutes = (total_secs % 3600) / 60;
    let secs = total_secs % 60;

    if hours > 0 {
        format!("{}h {}m", hours, minutes)
    } else {
        format!("{}m {}s", minutes, secs)
    }
}

/// Drain the offline bank when speed > 1x.
/// Uses `Time<Real>` so drain is based on wall-clock time, not virtual time.
fn tick_offline_bank(
    real_time: Res<Time<Real>>,
    mut bank: ResMut<OfflineTimeBank>,
    mut game_speed: ResMut<GameSpeed>,
    mut virtual_time: ResMut<Time<Virtual>>,
) {
    let drain_rate = game_speed.0 - 1.0;
    if drain_rate <= 0.0 {
        return;
    }

    let dt = real_time.delta_secs();
    bank.banked_seconds -= drain_rate * dt;

    if bank.banked_seconds <= 0.0 {
        bank.banked_seconds = 0.0;
        game_speed.0 = 1.0;
        virtual_time.set_relative_speed(1.0);
    }
}

/// Event: request to change game speed.
#[derive(Event)]
pub struct SetGameSpeed(pub f32);

/// Observer for speed change requests.
fn handle_set_speed(
    trigger: On<SetGameSpeed>,
    bank: Res<OfflineTimeBank>,
    mut game_speed: ResMut<GameSpeed>,
    mut virtual_time: ResMut<Time<Virtual>>,
) {
    let requested = trigger.event().0;

    // Can't go above 1x with no banked time
    if requested > 1.0 && bank.banked_seconds <= 0.0 {
        return;
    }

    game_speed.0 = requested;
    virtual_time.set_relative_speed(requested);
}

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<OfflineTimeBank>();
    app.init_resource::<GameSpeed>();
    app.add_observer(handle_set_speed);
    app.add_systems(
        Update,
        tick_offline_bank.run_if(in_state(crate::screens::Screen::Gameplay)),
    );
}
```

**Step 2: Register in `src/main.rs`**

Add `mod time_bank;` to module declarations. Add `time_bank::plugin` to one of the `add_plugins` calls (the second one has room).

**Step 3: Verify it compiles**

Run: `cargo build 2>&1 | head -20`

**Step 4: Commit**

```bash
git add src/time_bank.rs src/main.rs
git commit -m "feat(time-bank): add OfflineTimeBank, GameSpeed resources, and tick system"
```

---

### Task 6: Add speed control UI to the sidebar

**Files:**
- Modify: `src/screens/sidebar.rs` — add speed buttons and bank display between rep and nav divider
- Modify: `src/theme/widgets.rs` — add `SidebarBankText` marker if needed

**Step 1: Add marker component**

In `src/theme/widgets.rs`, add a new marker:

```rust
#[derive(Component)]
pub struct SidebarBankText;
```

And a speed button component:

```rust
#[derive(Component)]
pub struct SpeedButton(pub f32);
```

**Step 2: Add speed control bar to sidebar**

In `src/screens/sidebar.rs`, import the new types and `time_bank`:

```rust
use crate::time_bank::{GameSpeed, OfflineTimeBank, SetGameSpeed, format_banked_time};
use crate::theme::widgets::{SidebarBankText, SpeedButton};
```

In `build_sidebar`, add the speed control UI after the reputation text and before the first divider. Modify the function signature to accept bank and speed state:

```rust
fn build_sidebar(gold_amount: u32, rep_amount: u32, banked: f32, speed: f32) -> Div {
```

After the reputation `.child(...)`, add:

```rust
        // Speed control
        .child(
            div()
                .row()
                .w_full()
                .gap(px(4.0))
                .items_center()
                .child(speed_btn(1.0, speed))
                .child(speed_btn(2.0, speed))
                .child(speed_btn(3.0, speed))
                .child(
                    text(format!("Bank: {}", format_banked_time(banked)))
                        .font_size(14.0)
                        .color(Color::srgb(0.7, 0.8, 0.9))
                        .insert(SidebarBankText),
                ),
        )
```

Add a helper function `speed_btn`:

```rust
fn speed_btn(multiplier: f32, current_speed: f32) -> bevy_declarative::element::div::Div {
    let label = format!("{}x", multiplier as u32);
    let is_active = (current_speed - multiplier).abs() < 0.01;
    let bg = if is_active {
        Color::srgb(0.3, 0.5, 0.7)
    } else {
        BUTTON_BACKGROUND
    };

    div()
        .p(px(4.0))
        .items_center()
        .justify_center()
        .bg(bg)
        .rounded(px(3.0))
        .insert((
            Button,
            SpeedButton(multiplier),
            crate::theme::interaction::InteractionPalette {
                none: bg,
                hovered: BUTTON_HOVERED_BACKGROUND,
                pressed: BUTTON_PRESSED_BACKGROUND,
            },
        ))
        .on_click(on_speed_click)
        .child(
            text(label)
                .font_size(14.0)
                .color(BUTTON_TEXT)
                .insert(Pickable::IGNORE),
        )
}

fn on_speed_click(
    click: On<Pointer<Click>>,
    buttons: Query<&SpeedButton>,
    mut commands: Commands,
) {
    if let Ok(btn) = buttons.get(click.event_target()) {
        commands.trigger(SetGameSpeed(btn.0));
    }
}
```

**Step 3: Update spawn_gameplay_root to pass bank/speed data**

```rust
fn spawn_gameplay_root(
    mut commands: Commands,
    gold: Option<Res<Gold>>,
    rep: Option<Res<Reputation>>,
    bank: Option<Res<OfflineTimeBank>>,
    speed: Option<Res<GameSpeed>>,
) {
    let gold_amount = gold.map_or(0, |g| g.0);
    let rep_amount = rep.map_or(0, |r| r.0);
    let banked = bank.map_or(0.0, |b| b.banked_seconds);
    let current_speed = speed.map_or(1.0, |s| s.0);

    // ... existing root/sidebar code, passing new args:
    let sidebar = build_sidebar(gold_amount, rep_amount, banked, current_speed);
    // ...
}
```

**Step 4: Add reactive update system for bank text**

```rust
fn update_bank_display(
    bank: Res<OfflineTimeBank>,
    mut texts: Query<&mut Text, With<SidebarBankText>>,
) {
    for mut t in &mut texts {
        **t = format!("Bank: {}", format_banked_time(bank.banked_seconds));
    }
}
```

Register in the sidebar plugin's Update systems:

```rust
update_bank_display.run_if(resource_changed::<OfflineTimeBank>),
```

**Step 5: Verify it compiles**

Run: `cargo build 2>&1 | head -20`

**Step 6: Commit**

```bash
git add src/screens/sidebar.rs src/theme/widgets.rs
git commit -m "feat(time-bank): add speed control UI and bank display to sidebar"
```

---

### Task 7: Wire save and time_bank plugins into main.rs

**Files:**
- Modify: `src/main.rs` — add `save::plugin` and `time_bank::plugin` to add_plugins

This may already be partially done by earlier tasks. Ensure both `mod save;` and `mod time_bank;` are declared, and both plugins are registered.

**Step 1: Add mod declarations if not present**

In `src/main.rs`, add (alphabetically):
```rust
mod save;
mod time_bank;
```

**Step 2: Add plugins**

Add `save::plugin` and `time_bank::plugin` to the second `add_plugins` call:

```rust
app.add_plugins((
    recruiting::plugin,
    reputation::plugin,
    save::plugin,
    screens::plugin,
    theme::plugin,
    time_bank::plugin,
    training::plugin,
    ui::plugin,
));
```

Check that this doesn't exceed 15 elements. Current count in second tuple: recruiting, reputation, screens, theme, training, ui = 6. Adding save + time_bank = 8. Well under 15.

**Step 3: Verify it compiles and runs**

Run: `cargo build 2>&1 | head -20`
Then run the game briefly to confirm no panics.

**Step 4: Commit**

```bash
git add src/main.rs
git commit -m "feat(save): wire save and time_bank plugins into app"
```

---

### Task 8: Integration testing and polish

**Files:**
- Potentially any of the above files for bug fixes

**Step 1: Run the game and test the full save/load cycle**

1. Start the game fresh (delete any existing save file).
2. Play for a bit — hire a hero, start a mission, upgrade a building.
3. Confirm autosave toast appears after 5 minutes (or reduce timer temporarily for testing).
4. Close and reopen — confirm state is restored.
5. Test speed buttons: 1x/2x/3x should work, bank should drain, auto-reset to 1x when empty.

**Step 2: Fix any issues found during testing**

Common issues to watch for:
- `MissionProgress` may need `#[reflect(Component)]` attribute (it's a Component enum — check if Bevy needs this during deserialization; it's only used in our DTO path, so probably fine).
- Entity::PLACEHOLDER fallback in token loading — ensure indices are valid.
- `save_on_exit` with `On<AppExit>` — verify Bevy 0.18 supports observing `AppExit`. If not, use `app.add_systems(Last, save_on_exit_system.run_if(on_event::<AppExit>()))` instead.

**Step 3: Commit any fixes**

```bash
git add -A
git commit -m "fix(save): integration fixes from testing"
```

---

## Implementation Order

Because of cross-references between modules, implement in this order:

1. **Task 1** — Serialize derives (no dependencies)
2. **Task 2** — DTO types (depends on Task 1 for Serialize on referenced types)
3. **Task 5** — time_bank resources (no dependencies, but Task 3 references it)
4. **Task 7** — Wire plugins into main.rs (partial — just mod + plugin registration)
5. **Task 3** — Save logic (depends on Task 2 DTOs + Task 5 OfflineTimeBank)
6. **Task 4** — Load logic (depends on Task 3 existing)
7. **Task 6** — Speed control UI (depends on Task 5 resources)
8. **Task 8** — Integration testing
