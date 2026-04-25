//! Save system: serialize/deserialize full game state to/from a RON file.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::SystemTime;

use crate::buildings::{BuildingType, GuildBuildings};
use crate::economy::Gold;
use crate::equipment::HeroEquipment;
use crate::hero::data::{ClassDatabase, HeroClass, HeroTrait};
use crate::hero::status::{Injured, Missing};
use crate::hero::{
    Favorite, Hero, HeroGrowth, HeroInfo, HeroStatProgress, HeroStats, HeroTraits,
    PersonallyManaged, roll_growth,
};
use crate::materials::{MaterialType, Materials};
use crate::mission::dungeon::DungeonMap;
use crate::mission::entities::{
    CombatStats, EnemyToken, GridPosition, HeroToken, InRoom, MoveTarget, RoomStatus,
};
use crate::mission::{
    Mission, MissionDungeon, MissionInfo, MissionParty, MissionProgress, OnMission,
};
use crate::recruiting::{Applicant, ApplicantBoard};
use crate::reputation::Reputation;
use crate::mission::data::EnemyType;
use crate::time_bank::OfflineTimeBank;
use crate::training::TrainingTimer;
use crate::ui::toast::{ToastEvent, ToastKind};

// ── Plugin ─────────────────────────────────────────────────────────

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<AutosaveTimer>();
    app.add_observer(handle_save);
    app.add_systems(
        OnEnter(crate::screens::Screen::Gameplay),
        load_save,
    );
    app.add_systems(
        Update,
        tick_autosave.run_if(in_state(crate::screens::Screen::Gameplay)),
    );
}

// ── Resources & Events ─────────────────────────────────────────────

/// If a saved Missing timer has less than this many game-seconds remaining
/// when the save is loaded, restore the hero straight to Injured rather
/// than letting Missing tick out almost immediately and trigger a return
/// toast moments after the player loaded.
const NEAR_EXPIRED_MISSING_THRESHOLD_SECS: f64 = 1.0;

/// Timer that fires an autosave every 300 seconds.
#[derive(Resource, Debug)]
pub struct AutosaveTimer(pub f32);

impl Default for AutosaveTimer {
    fn default() -> Self {
        Self(300.0)
    }
}

/// Fire this event to trigger a save (manual or autosave).
#[derive(Event, Debug)]
pub struct SaveGame;

/// Marker resource: indicates a save was loaded this session.
#[derive(Resource)]
pub struct SaveLoaded;

// ── Systems ────────────────────────────────────────────────────────

/// Tick the autosave timer; fire `SaveGame` when it expires.
fn tick_autosave(time: Res<Time<Real>>, mut timer: ResMut<AutosaveTimer>, mut commands: Commands) {
    timer.0 -= time.delta_secs();
    if timer.0 <= 0.0 {
        timer.0 = 300.0;
        commands.trigger(SaveGame);
    }
}

/// Load game state from disk on entering Gameplay (if a save file exists).
fn load_save(
    mut commands: Commands,
    existing_heroes: Query<(), With<Hero>>,
    class_db: Res<ClassDatabase>,
    time: Res<Time<Virtual>>,
) {
    // Already have heroes — skip (re-entry or already loaded).
    if !existing_heroes.is_empty() {
        return;
    }

    // Read the save file.
    let Some(path) = save_file_path() else {
        return;
    };
    let Ok(ron_string) = std::fs::read_to_string(&path) else {
        return;
    };
    let save_data: SaveData = match ron::from_str(&ron_string) {
        Ok(d) => d,
        Err(e) => {
            warn!("Failed to deserialize save file: {e}");
            return;
        }
    };

    info!("Loading save from {}", path.display());

    // ── Restore resources ──────────────────────────────────────────
    commands.insert_resource(Gold(save_data.gold));
    commands.insert_resource(Reputation(save_data.reputation));
    commands.insert_resource(Materials(save_data.materials));
    commands.insert_resource(GuildBuildings(save_data.buildings));
    commands.insert_resource(TrainingTimer(save_data.training_timer));

    // ── Restore applicant board ────────────────────────────────────
    let applicants: Vec<Applicant> = save_data
        .applicants
        .iter()
        .map(|a| Applicant {
            name: a.name.clone(),
            class: a.class,
            traits: a.traits.clone(),
            stats: HeroStats {
                strength: a.stats.strength,
                dexterity: a.stats.dexterity,
                constitution: a.stats.constitution,
                intelligence: a.stats.intelligence,
                wisdom: a.stats.wisdom,
                charisma: a.stats.charisma,
            },
            growth: restore_growth(&a.growth, a.class, &class_db),
            hire_cost: a.hire_cost,
            time_remaining: a.time_remaining,
        })
        .collect();
    commands.insert_resource(ApplicantBoard {
        applicants,
        next_arrival_timer: save_data.next_arrival_timer,
    });

    // ── Spawn heroes — track entities for mission cross-references ─
    let mut hero_entities: Vec<Entity> = Vec::with_capacity(save_data.heroes.len());
    for dto in &save_data.heroes {
        let mut entity_commands = commands.spawn((
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
            restore_growth(&dto.growth, dto.class, &class_db),
            HeroStatProgress {
                strength: dto.progress.strength,
                dexterity: dto.progress.dexterity,
                constitution: dto.progress.constitution,
                intelligence: dto.progress.intelligence,
                wisdom: dto.progress.wisdom,
                charisma: dto.progress.charisma,
            },
        ));
        if dto.favorite {
            entity_commands.insert(Favorite);
        }
        if dto.personally_managed {
            entity_commands.insert(PersonallyManaged);
        }
        let now = time.elapsed_secs_f64();
        if let Some(rem) = dto.missing_remaining {
            if rem < NEAR_EXPIRED_MISSING_THRESHOLD_SECS {
                // Save was taken in the final second of Missing — restoring
                // the tail would just fire the "X has returned" toast moments
                // after load, which feels like a spurious notification. Skip
                // the tail and apply Injured directly with a fresh duration.
                use crate::hero::status::INJURED_DURATION_SECS;
                entity_commands.insert(Injured {
                    expires_at: now + INJURED_DURATION_SECS,
                });
            } else {
                entity_commands.insert(Missing { expires_at: now + rem });
            }
        }
        if let Some(rem) = dto.injured_remaining {
            entity_commands.insert(Injured { expires_at: now + rem });
        }
        let entity = entity_commands.id();
        hero_entities.push(entity);
    }

    // ── Spawn missions with tokens as children ─────────────────────
    for mdto in &save_data.missions {
        let party_entities: Vec<Entity> = mdto
            .party_hero_indices
            .iter()
            .filter_map(|&idx| hero_entities.get(idx).copied())
            .collect();

        let mission_entity = commands
            .spawn((
                Name::new(mdto.name.clone()),
                Mission,
                MissionInfo {
                    template_id: mdto.template_id.clone(),
                    name: mdto.name.clone(),
                    difficulty: mdto.difficulty,
                },
                mdto.progress,
                MissionParty(party_entities.clone()),
                MissionDungeon(mdto.dungeon_map.clone()),
                RoomStatus {
                    visited: mdto.room_visited.clone(),
                    cleared: mdto.room_cleared.clone(),
                },
            ))
            .id();

        // Mark party heroes as on-mission.
        for &hero_entity in &party_entities {
            commands.entity(hero_entity).insert(OnMission(mission_entity));
        }

        // Spawn hero tokens as children.
        for ht in &mdto.hero_tokens {
            let roster_entity = hero_entities
                .get(ht.roster_index)
                .copied()
                .unwrap_or(Entity::PLACEHOLDER);
            let mut token = commands.spawn((
                Name::new(format!("Hero Token")),
                HeroToken(roster_entity),
                GridPosition {
                    x: ht.grid_x,
                    y: ht.grid_y,
                },
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

        // Spawn enemy tokens as children.
        for et in &mdto.enemy_tokens {
            commands.spawn((
                Name::new(format!("Enemy Token")),
                EnemyToken {
                    enemy_type: et.enemy_type,
                    xp_reward: et.xp_reward,
                },
                GridPosition {
                    x: et.grid_x,
                    y: et.grid_y,
                },
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

    // ── Offline time calculation ───────────────────────────────────
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let elapsed = now.saturating_sub(save_data.last_save_timestamp) as f32;
    let new_banked = (save_data.banked_seconds + elapsed).min(86400.0);
    commands.insert_resource(OfflineTimeBank {
        banked_seconds: new_banked,
    });

    // Fire toast with banked time info.
    let formatted = crate::time_bank::format_banked_time(new_banked);
    commands.trigger(ToastEvent {
        title: "Welcome Back!".to_string(),
        body: format!("Banked time: {formatted}"),
        kind: ToastKind::Info,
    });

    // Insert marker resource.
    commands.insert_resource(SaveLoaded);

    info!("Save loaded successfully ({} heroes, {} missions)", hero_entities.len(), save_data.missions.len());
}

/// Observer that performs the full save when `SaveGame` is triggered.
fn handle_save(
    _trigger: On<SaveGame>,
    mut commands: Commands,
    gold: Res<Gold>,
    reputation: Res<Reputation>,
    materials: Res<Materials>,
    buildings: Res<GuildBuildings>,
    training_timer: Res<TrainingTimer>,
    applicant_board: Res<ApplicantBoard>,
    offline_bank: Res<OfflineTimeBank>,
    time: Res<Time<Virtual>>,
    heroes: Query<
        (
            Entity,
            &HeroInfo,
            &HeroStats,
            &HeroTraits,
            &HeroEquipment,
            &HeroGrowth,
            &HeroStatProgress,
            Option<&OnMission>,
            Has<Favorite>,
            Has<PersonallyManaged>,
            Option<&Missing>,
            Option<&Injured>,
        ),
        With<Hero>,
    >,
    missions: Query<
        (
            Entity,
            &MissionInfo,
            &MissionProgress,
            &MissionParty,
            &MissionDungeon,
            &RoomStatus,
            &Children,
        ),
        With<Mission>,
    >,
    hero_tokens: Query<
        (
            &HeroToken,
            &GridPosition,
            &CombatStats,
            &InRoom,
            Option<&MoveTarget>,
        ),
        Without<EnemyToken>,
    >,
    enemy_tokens: Query<
        (&EnemyToken, &GridPosition, &CombatStats, &InRoom),
        Without<HeroToken>,
    >,
) {
    // 1. Build hero roster and entity→index mapping.
    let mut hero_dtos = Vec::new();
    let mut entity_to_index: HashMap<Entity, usize> = HashMap::new();

    for (entity, info, stats, traits, equipment, growth, progress, on_mission, is_favorite, is_managed, missing, injured) in &heroes {
        let idx = hero_dtos.len();
        entity_to_index.insert(entity, idx);

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
            traits: traits.0.clone(),
            equipment: HeroEquipmentSave {
                weapon_tier: equipment.weapon_tier,
                armor_tier: equipment.armor_tier,
                accessory_tier: equipment.accessory_tier,
            },
            on_mission: on_mission.is_some(),
            growth: HeroGrowthSave {
                strength: growth.strength,
                dexterity: growth.dexterity,
                constitution: growth.constitution,
                intelligence: growth.intelligence,
                wisdom: growth.wisdom,
                charisma: growth.charisma,
            },
            progress: HeroStatProgressSave {
                strength: progress.strength,
                dexterity: progress.dexterity,
                constitution: progress.constitution,
                intelligence: progress.intelligence,
                wisdom: progress.wisdom,
                charisma: progress.charisma,
            },
            favorite: is_favorite,
            personally_managed: is_managed,
            missing_remaining: missing.map(|m| (m.expires_at - time.elapsed_secs_f64()).max(0.0)),
            injured_remaining: injured.map(|i| (i.expires_at - time.elapsed_secs_f64()).max(0.0)),
        });
    }

    // 2. Build applicant DTOs.
    let applicant_dtos: Vec<ApplicantSaveDto> = applicant_board
        .applicants
        .iter()
        .map(|a| ApplicantSaveDto {
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
            growth: HeroGrowthSave {
                strength: a.growth.strength,
                dexterity: a.growth.dexterity,
                constitution: a.growth.constitution,
                intelligence: a.growth.intelligence,
                wisdom: a.growth.wisdom,
                charisma: a.growth.charisma,
            },
        })
        .collect();

    // 3. Build mission DTOs.
    let mut mission_dtos = Vec::new();

    for (_entity, info, progress, party, dungeon, room_status, children) in &missions {
        // Map party entities to hero roster indices.
        let party_hero_indices: Vec<usize> = party
            .0
            .iter()
            .filter_map(|e| entity_to_index.get(e).copied())
            .collect();

        // Collect hero tokens that are children of this mission.
        let mut hero_token_dtos = Vec::new();
        let mut enemy_token_dtos = Vec::new();

        for child in children.iter() {
            if let Ok((ht, pos, combat, in_room, move_target)) = hero_tokens.get(child) {
                let roster_index = entity_to_index.get(&ht.0).copied().unwrap_or(0);
                hero_token_dtos.push(HeroTokenDto {
                    roster_index,
                    grid_x: pos.x,
                    grid_y: pos.y,
                    in_room: in_room.0,
                    hp: combat.hp,
                    max_hp: combat.max_hp,
                    attack: combat.attack,
                    defense: combat.defense,
                    path: move_target.as_ref().map(|mt| mt.path.clone()),
                    path_index: move_target.as_ref().map_or(0, |mt| mt.path_index),
                });
            }

            if let Ok((et, pos, combat, in_room)) = enemy_tokens.get(child) {
                enemy_token_dtos.push(EnemyTokenDto {
                    enemy_type: et.enemy_type,
                    xp_reward: et.xp_reward,
                    grid_x: pos.x,
                    grid_y: pos.y,
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
            rng_seed: 0,
            party_hero_indices,
            dungeon_map: dungeon.0.clone(),
            room_visited: room_status.visited.clone(),
            room_cleared: room_status.cleared.clone(),
            hero_tokens: hero_token_dtos,
            enemy_tokens: enemy_token_dtos,
        });
    }

    // 4. Get unix timestamp.
    let timestamp = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    // 5. Assemble SaveData.
    let save_data = SaveData {
        last_save_timestamp: timestamp,
        gold: gold.0,
        reputation: reputation.0,
        banked_seconds: offline_bank.banked_seconds,
        materials: materials.0.clone(),
        buildings: buildings.0.clone(),
        heroes: hero_dtos,
        applicants: applicant_dtos,
        next_arrival_timer: applicant_board.next_arrival_timer,
        training_timer: training_timer.0,
        missions: mission_dtos,
    };

    // 6. Serialize to RON.
    let ron_string =
        match ron::ser::to_string_pretty(&save_data, ron::ser::PrettyConfig::default()) {
            Ok(s) => s,
            Err(e) => {
                warn!("Failed to serialize save data: {e}");
                return;
            }
        };

    // 7. Write to disk.
    let Some(path) = save_file_path() else {
        warn!("Could not determine save file path");
        return;
    };

    if let Some(parent) = path.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            warn!("Failed to create save directory: {e}");
            return;
        }
    }

    if let Err(e) = std::fs::write(&path, ron_string) {
        warn!("Failed to write save file: {e}");
        return;
    }

    info!("Game saved to {}", path.display());

    // 8. Fire toast.
    commands.trigger(ToastEvent {
        title: "Game Saved".to_string(),
        body: "Game saved.".to_string(),
        kind: ToastKind::Info,
    });
}

// ── Helpers ────────────────────────────────────────────────────────

/// Return the save file path: `<data_dir>/guild-forge/save.ron`.
pub fn save_file_path() -> Option<PathBuf> {
    dirs::data_dir().map(|d| d.join("guild-forge").join("save.ron"))
}

/// Returns true if a save file exists on disk.
pub fn has_save_file() -> bool {
    save_file_path().is_some_and(|p| p.exists())
}

// ── DTOs ───────────────────────────────────────────────────────────

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

#[derive(Serialize, Deserialize, Default)]
pub struct HeroGrowthSave {
    pub strength: f32,
    pub dexterity: f32,
    pub constitution: f32,
    pub intelligence: f32,
    pub wisdom: f32,
    pub charisma: f32,
}

#[derive(Serialize, Deserialize, Default)]
pub struct HeroStatProgressSave {
    pub strength: f32,
    pub dexterity: f32,
    pub constitution: f32,
    pub intelligence: f32,
    pub wisdom: f32,
    pub charisma: f32,
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
    #[serde(default)]
    pub growth: HeroGrowthSave,
    #[serde(default)]
    pub progress: HeroStatProgressSave,
    #[serde(default)]
    pub favorite: bool,
    #[serde(default)]
    pub personally_managed: bool,
    #[serde(default)]
    pub missing_remaining: Option<f64>,
    #[serde(default)]
    pub injured_remaining: Option<f64>,
}

#[derive(Serialize, Deserialize)]
pub struct ApplicantSaveDto {
    pub name: String,
    pub class: HeroClass,
    pub traits: Vec<HeroTrait>,
    pub stats: HeroStatsSave,
    pub hire_cost: u32,
    pub time_remaining: f32,
    #[serde(default)]
    pub growth: HeroGrowthSave,
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

// ── Growth backfill ───────────────────────────────────────────────

/// True when every growth component is exactly 0.0 — the signature of a
/// legacy save that predates the growth-rates feature.
fn is_zero_growth(g: &HeroGrowthSave) -> bool {
    g.strength == 0.0
        && g.dexterity == 0.0
        && g.constitution == 0.0
        && g.intelligence == 0.0
        && g.wisdom == 0.0
        && g.charisma == 0.0
}

/// Convert a `HeroGrowthSave` to a `HeroGrowth`. For legacy saves where every
/// field is zero, roll a fresh neutral-quality (0.5) growth from the hero's
/// class so existing heroes aren't permanently frozen at their current stats.
fn restore_growth(
    saved: &HeroGrowthSave,
    class: HeroClass,
    class_db: &ClassDatabase,
) -> HeroGrowth {
    if is_zero_growth(saved) {
        if let Some(class_def) = class_db.get(class) {
            let mut rng = rand::rng();
            return roll_growth(class_def, 0.5, &mut rng);
        }
        // Class not found — fall through to the zeroed value below as a
        // harmless last-resort default.
    }
    HeroGrowth {
        strength: saved.strength,
        dexterity: saved.dexterity,
        constitution: saved.constitution,
        intelligence: saved.intelligence,
        wisdom: saved.wisdom,
        charisma: saved.charisma,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hero_save_dto_round_trips_with_growth() {
        let dto = HeroSaveDto {
            name: "A".into(),
            class: HeroClass::Warrior,
            level: 3,
            xp: 42,
            xp_to_next: 200,
            stats: HeroStatsSave {
                strength: 12,
                dexterity: 10,
                constitution: 14,
                intelligence: 8,
                wisdom: 9,
                charisma: 10,
            },
            traits: vec![],
            equipment: HeroEquipmentSave {
                weapon_tier: 0,
                armor_tier: 0,
                accessory_tier: 0,
            },
            on_mission: false,
            growth: HeroGrowthSave {
                strength: 1.1,
                dexterity: 0.3,
                constitution: 0.8,
                intelligence: 0.0,
                wisdom: 0.4,
                charisma: 0.2,
            },
            progress: HeroStatProgressSave {
                strength: 0.5,
                dexterity: 0.0,
                constitution: 0.2,
                intelligence: 0.0,
                wisdom: 0.1,
                charisma: 0.0,
            },
            favorite: false,
            personally_managed: false,
            missing_remaining: None,
            injured_remaining: None,
        };
        let s = ron::ser::to_string(&dto).unwrap();
        let back: HeroSaveDto = ron::from_str(&s).unwrap();
        assert!((back.growth.strength - 1.1).abs() < 1e-5);
        assert!((back.growth.charisma - 0.2).abs() < 1e-5);
        assert!((back.progress.strength - 0.5).abs() < 1e-5);
    }

    #[test]
    fn legacy_hero_save_dto_without_growth_deserializes_with_defaults() {
        // A RON string missing `growth` and `progress` fields.
        let legacy = r#"(
            name: "L",
            class: Warrior,
            level: 2, xp: 0, xp_to_next: 150,
            stats: (strength: 10, dexterity: 10, constitution: 10,
                    intelligence: 10, wisdom: 10, charisma: 10),
            traits: [],
            equipment: (weapon_tier: 0, armor_tier: 0, accessory_tier: 0),
            on_mission: false,
        )"#;
        let dto: HeroSaveDto = ron::from_str(legacy).unwrap();
        assert!(is_zero_growth(&dto.growth));
        assert_eq!(dto.progress.strength, 0.0);
    }

    #[test]
    fn hero_save_dto_round_trips_favorite_flags() {
        let dto = HeroSaveDto {
            name: "F".into(),
            class: HeroClass::Warrior,
            level: 1,
            xp: 0,
            xp_to_next: 100,
            stats: HeroStatsSave {
                strength: 10, dexterity: 10, constitution: 10,
                intelligence: 10, wisdom: 10, charisma: 10,
            },
            traits: vec![],
            equipment: HeroEquipmentSave {
                weapon_tier: 0, armor_tier: 0, accessory_tier: 0,
            },
            on_mission: false,
            growth: HeroGrowthSave::default(),
            progress: HeroStatProgressSave::default(),
            favorite: true,
            personally_managed: true,
            missing_remaining: None,
            injured_remaining: None,
        };
        let s = ron::ser::to_string(&dto).unwrap();
        let back: HeroSaveDto = ron::from_str(&s).unwrap();
        assert!(back.favorite);
        assert!(back.personally_managed);
    }

    #[test]
    fn hero_save_dto_round_trips_with_missing_and_injured() {
        let dto = HeroSaveDto {
            name: "A".into(),
            class: HeroClass::Warrior,
            level: 1,
            xp: 0,
            xp_to_next: 100,
            stats: HeroStatsSave { strength: 10, dexterity: 10, constitution: 10,
                intelligence: 10, wisdom: 10, charisma: 10 },
            traits: vec![],
            equipment: HeroEquipmentSave { weapon_tier: 0, armor_tier: 0, accessory_tier: 0 },
            on_mission: false,
            growth: HeroGrowthSave::default(),
            progress: HeroStatProgressSave::default(),
            favorite: false,
            personally_managed: false,
            missing_remaining: Some(42.0),
            injured_remaining: Some(200.0),
        };
        let s = ron::to_string(&dto).unwrap();
        let back: HeroSaveDto = ron::from_str(&s).unwrap();
        assert_eq!(back.missing_remaining, Some(42.0));
        assert_eq!(back.injured_remaining, Some(200.0));
    }

    #[test]
    fn hero_save_dto_defaults_missing_and_injured_to_none() {
        // Old-format save (no fields) should deserialize with None.
        let old = r#"(name:"A",class:Warrior,level:1,xp:0,xp_to_next:100,
            stats:(strength:10,dexterity:10,constitution:10,intelligence:10,wisdom:10,charisma:10),
            traits:[],equipment:(weapon_tier:0,armor_tier:0,accessory_tier:0),on_mission:false)"#;
        let back: HeroSaveDto = ron::from_str(old).unwrap();
        assert_eq!(back.missing_remaining, None);
        assert_eq!(back.injured_remaining, None);
    }

    #[test]
    fn legacy_hero_save_dto_without_favorite_flags_defaults_false() {
        // A RON string missing `favorite` and `personally_managed`.
        let legacy = r#"(
            name: "L",
            class: Warrior,
            level: 1, xp: 0, xp_to_next: 100,
            stats: (strength: 10, dexterity: 10, constitution: 10,
                    intelligence: 10, wisdom: 10, charisma: 10),
            traits: [],
            equipment: (weapon_tier: 0, armor_tier: 0, accessory_tier: 0),
            on_mission: false,
        )"#;
        let dto: HeroSaveDto = ron::from_str(legacy).unwrap();
        assert!(!dto.favorite);
        assert!(!dto.personally_managed);
    }

    #[test]
    fn is_zero_growth_detects_all_zero_vs_nonzero() {
        let zero = HeroGrowthSave::default();
        assert!(is_zero_growth(&zero));
        let non_zero = HeroGrowthSave {
            strength: 0.0,
            dexterity: 0.0,
            constitution: 0.0,
            intelligence: 0.0001,
            wisdom: 0.0,
            charisma: 0.0,
        };
        assert!(!is_zero_growth(&non_zero));
    }
}
