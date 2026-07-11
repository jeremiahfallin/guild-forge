//! Save system: serialize/deserialize full game state to/from a RON file.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::SystemTime;

use crate::buildings::{BuildingType, GuildBuildings};
use crate::economy::Gold;
use crate::equipment::{HeroEquipment, GearRarity, BehavioralAffix};
use crate::hero::data::{ClassDatabase, HeroClass, HeroTrait};
use crate::hero::status::{Injured, Missing};
use crate::hero::{
    Favorite, Hero, HeroGrowth, HeroInfo, HeroStatProgress, HeroStats, HeroTraits,
    PersonallyManaged, roll_growth,
};
use crate::materials::{MaterialType, Materials};
use crate::mission::dungeon::DungeonMap;
use crate::mission::entities::{
    CombatStats, EnemyToken, GridPosition, HeroToken, InRoom, MoveTarget, RoomStatus, MoveRange,
};
use crate::mission::{
    Mission, MissionDungeon, MissionInfo, MissionParty, MissionProgress, OnMission,
};
use crate::ui::feed::{MissionLogHistory, MissionLogEntry};
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
    mut images: ResMut<Assets<Image>>,
) {
    // Already have heroes — skip (re-entry or already loaded).
    if !existing_heroes.is_empty() {
        return;
    }

    let Some(path) = save_file_path() else {
        return;
    };
    let bak_path = path.with_extension("ron.bak");

    let mut loaded_data = None;

    if path.exists() {
        if let Ok(ron_string) = std::fs::read_to_string(&path) {
            match deserialize_and_migrate(&ron_string) {
                Ok(data) => {
                    loaded_data = Some(data);
                }
                Err(e) => {
                    warn!("Failed to load save file: {e}. Trying backup...");
                }
            }
        }
    }

    if loaded_data.is_none() && bak_path.exists() {
        if let Ok(ron_string) = std::fs::read_to_string(&bak_path) {
            match deserialize_and_migrate(&ron_string) {
                Ok(data) => {
                    loaded_data = Some(data);
                    info!("Restored save game from backup");
                }
                Err(e) => {
                    warn!("Failed to load backup save file: {e}");
                }
            }
        }
    }

    if loaded_data.is_none() && path.exists() {
        let corrupt_path = path.with_extension("ron.corrupt");
        warn!("Save game is corrupted and backup recovery failed. Renaming corrupt file to: {}", corrupt_path.display());
        let _ = std::fs::rename(&path, corrupt_path);
    }

    let Some(save_data) = loaded_data else {
        return;
    };

    info!("Loading save from {}", path.display());

    // ── Restore resources ──────────────────────────────────────────
    commands.insert_resource(Gold(save_data.gold));
    commands.insert_resource(Reputation(save_data.reputation));
    commands.insert_resource(Materials(save_data.materials));
    commands.insert_resource(GuildBuildings(save_data.buildings));
    commands.insert_resource(TrainingTimer(save_data.training_timer));
    commands.insert_resource(crate::tutorial::TutorialState {
        step: save_data.tutorial_step,
        done: save_data.tutorial_done,
        saw_active_mission: false,
    });

    // ── Restore applicant board ────────────────────────────────────
    let applicants: Vec<Applicant> = save_data
        .applicants
        .iter()
        .map(|a| {
            let portrait = a.portrait.as_ref().map(|p| {
                crate::hero::HeroPortrait {
                    base_idx: p.base_idx,
                    hair_idx: p.hair_idx,
                    hair_color: Color::srgba(p.hair_color[0], p.hair_color[1], p.hair_color[2], p.hair_color[3]),
                    gear_idx: p.gear_idx,
                }
            }).unwrap_or_else(|| {
                crate::hero::HeroPortrait::random(&mut rand::rng())
            });
            let image = crate::hero::portrait::composite_portrait(&portrait, None);
            let portrait_handle = Some(images.add(image));

            Applicant {
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
                portrait,
                portrait_handle,
            }
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
                weapon_rarity: dto.equipment.weapon_rarity,
                weapon_affix: dto.equipment.weapon_affix,
                armor_tier: dto.equipment.armor_tier,
                armor_rarity: dto.equipment.armor_rarity,
                armor_affix: dto.equipment.armor_affix,
                accessory_tier: dto.equipment.accessory_tier,
                accessory_rarity: dto.equipment.accessory_rarity,
                accessory_affix: dto.equipment.accessory_affix,
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
            crate::hero::Fatigue {
                current: dto.fatigue_current,
                max_base: dto.fatigue_max_base,
            },
            MoveRange {
                base: dto.move_range_base,
                bonus: dto.move_range_bonus,
            },
            crate::hero::history::HeroHistory {
                missions_run: dto.history.missions_run,
                kills: dto.history.kills,
                near_deaths: dto.history.near_deaths,
                rescues_given: dto.history.rescues_given,
                rescues_received: dto.history.rescues_received,
                lifetime_gold: dto.history.lifetime_gold,
                timeline: if dto.history.timeline.is_empty() {
                    vec![crate::localization::tr("timeline.joined").to_string()]
                } else {
                    dto.history.timeline.clone()
                },
            },
            crate::hero::Epithet(dto.epithet.clone()),
            dto.portrait.as_ref().map(|p| {
                crate::hero::HeroPortrait {
                    base_idx: p.base_idx,
                    hair_idx: p.hair_idx,
                    hair_color: Color::srgba(p.hair_color[0], p.hair_color[1], p.hair_color[2], p.hair_color[3]),
                    gear_idx: p.gear_idx,
                }
            }).unwrap_or_else(|| {
                crate::hero::HeroPortrait::random(&mut rand::rng())
            }),
        ));
        if dto.favorite {
            entity_commands.insert(Favorite);
        }
        if dto.personally_managed {
            entity_commands.insert(PersonallyManaged);
        }
        let now = time.elapsed_secs_f64();
        if let Some(rem) = dto.missing_remaining {
            let dropped = dto.dropped_equipment.as_ref().map(|eq| crate::equipment::HeroEquipment {
                weapon_tier: eq.weapon_tier,
                weapon_rarity: eq.weapon_rarity,
                weapon_affix: eq.weapon_affix,
                armor_tier: eq.armor_tier,
                armor_rarity: eq.armor_rarity,
                armor_affix: eq.armor_affix,
                accessory_tier: eq.accessory_tier,
                accessory_rarity: eq.accessory_rarity,
                accessory_affix: eq.accessory_affix,
            });

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
                entity_commands.insert(Missing {
                    expires_at: now + rem,
                    dropped_equipment: dropped,
                });
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

        let mut mission_cmd = commands.spawn((
            Name::new(mdto.name.clone()),
            Mission,
            MissionInfo {
                template_id: mdto.template_id.clone(),
                name: mdto.name.clone(),
                difficulty: mdto.difficulty,
                modifiers: mdto.modifiers.clone(),
                biome: mdto.biome,
            },
            mdto.progress,
            MissionParty(party_entities.clone()),
            MissionDungeon(mdto.dungeon_map.clone()),
            RoomStatus {
                visited: mdto.room_visited.clone(),
                cleared: mdto.room_cleared.clone(),
            },
            crate::mission::entities::MissionTurnQueue::default(),
            crate::mission::entities::MissionEventsState {
                events_fired: mdto.events_fired,
                max_events: mdto.max_events,
            },
            MissionLogHistory {
                logs: mdto.logs.clone(),
            },
        ));

        if let Some(ref rescue_indices) = mdto.rescue_hero_indices {
            let rescue_entities: Vec<Entity> = rescue_indices
                .iter()
                .filter_map(|&idx| hero_entities.get(idx).copied())
                .collect();
            mission_cmd.insert(crate::mission::RescueMission {
                rescue_heroes: rescue_entities,
                gear_recovered: mdto.rescue_gear_recovered.unwrap_or(false),
            });
        }

        let mission_entity = mission_cmd.id();

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

            let mut starting_abilities = ht.abilities.clone();
            if starting_abilities.is_empty()
                && let Some(roster_class) = save_data.heroes.get(ht.roster_index).map(|h| h.class)
                    && let Some(class_def) = class_db.get(roster_class) {
                        starting_abilities = class_def
                            .starting_abilities
                            .iter()
                            .map(|id| crate::mission::entities::ActiveAbilityState {
                                ability_id: id.clone(),
                                remaining_cooldown: 0,
                            })
                            .collect();
                    }

            let hero_name = save_data.heroes.get(ht.roster_index).map(|h| h.name.clone()).unwrap_or_else(|| "Hero".to_string());
            let mut token = commands.spawn((
                Name::new(format!("Hero Token: {}", hero_name)),
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
                    speed: ht.speed,
                },
                MoveRange {
                    base: ht.move_range_base,
                    bonus: ht.move_range_bonus,
                },
                ChildOf(mission_entity),
                crate::mission::entities::ActiveAbilities {
                    abilities: starting_abilities,
                },
            ));
            if let Some(ref path) = ht.path
                && ht.path_index < path.len() {
                    token.insert(MoveTarget {
                        path: path.clone(),
                        path_index: ht.path_index,
                    });
                }
        }

        // Spawn enemy tokens as children.
        for et in &mdto.enemy_tokens {
            commands.spawn((
                Name::new("Enemy Token".to_string()),
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
                    speed: et.speed,
                },
                MoveRange {
                    base: et.move_range_base,
                    bonus: et.move_range_bonus,
                },
                ChildOf(mission_entity),
            ));
        }
    }

    // Restore MissionBoard resource with rescue offers
    let mut board = crate::screens::missions::MissionBoard::default();
    let virtual_now = time.elapsed_secs_f64();
    for rdto in &save_data.rescue_offers {
        let rescue_entities: Vec<Entity> = rdto
            .rescue_hero_indices
            .iter()
            .filter_map(|&idx| hero_entities.get(idx).copied())
            .collect();
        board.rescue_offers.push(crate::screens::missions::RescueOffer {
            template_idx: rdto.template_idx,
            modifiers: rdto.modifiers.clone(),
            map: rdto.map.clone(),
            rescue_heroes: rescue_entities,
            expires_at: virtual_now + rdto.expires_at_remaining,
        });
    }
    commands.insert_resource(board);

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
        title: crate::localization::tr("save.welcome_back").to_string(),
        body: crate::localization::trf("save.banked_time", &[("time", &formatted)]),
        kind: ToastKind::Info,
        action: None,
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
    tutorial: Res<crate::tutorial::TutorialState>,
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
            &crate::hero::Fatigue,
            Option<&MoveRange>,
            &crate::hero::history::HeroHistory,
            (
                Has<Favorite>,
                Has<PersonallyManaged>,
                Option<&Missing>,
                Option<&Injured>,
                Option<&crate::hero::Epithet>,
                Option<&crate::hero::HeroPortrait>,
            ),
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
            Option<&MissionLogHistory>,
            Option<&crate::mission::entities::MissionEventsState>,
            Option<&crate::mission::RescueMission>,
        ),
        With<Mission>,
    >,
    hero_tokens: Query<
        (
            &HeroToken,
            &GridPosition,
            &CombatStats,
            &InRoom,
            &MoveRange,
            Option<&MoveTarget>,
            Option<&crate::mission::entities::ActiveAbilities>,
        ),
        Without<EnemyToken>,
    >,
    enemy_tokens: Query<
        (&EnemyToken, &GridPosition, &CombatStats, &InRoom, &MoveRange),
        Without<HeroToken>,
    >,
    board: Option<Res<crate::screens::missions::MissionBoard>>,
) {
    // 1. Build hero roster and entity→index mapping.
    let mut hero_dtos = Vec::new();
    let mut entity_to_index: HashMap<Entity, usize> = HashMap::new();

    for (entity, info, stats, traits, equipment, growth, progress, on_mission, fatigue, maybe_move_range, history, (is_favorite, is_managed, missing, injured, maybe_epithet, maybe_portrait)) in &heroes {
        let idx = hero_dtos.len();
        entity_to_index.insert(entity, idx);

        let (mr_base, mr_bonus) = if let Some(mr) = maybe_move_range {
            (mr.base, mr.bonus)
        } else {
            let base = match info.class {
                HeroClass::Rogue | HeroClass::Ranger => 4,
                _ => 3,
            };
            (base, 0)
        };

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
                weapon_rarity: equipment.weapon_rarity,
                weapon_affix: equipment.weapon_affix,
                armor_tier: equipment.armor_tier,
                armor_rarity: equipment.armor_rarity,
                armor_affix: equipment.armor_affix,
                accessory_tier: equipment.accessory_tier,
                accessory_rarity: equipment.accessory_rarity,
                accessory_affix: equipment.accessory_affix,
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
            dropped_equipment: missing.and_then(|m| m.dropped_equipment.as_ref().map(|eq| HeroEquipmentSave {
                weapon_tier: eq.weapon_tier,
                weapon_rarity: eq.weapon_rarity,
                weapon_affix: eq.weapon_affix,
                armor_tier: eq.armor_tier,
                armor_rarity: eq.armor_rarity,
                armor_affix: eq.armor_affix,
                accessory_tier: eq.accessory_tier,
                accessory_rarity: eq.accessory_rarity,
                accessory_affix: eq.accessory_affix,
            })),
            injured_remaining: injured.map(|i| (i.expires_at - time.elapsed_secs_f64()).max(0.0)),
            fatigue_current: fatigue.current,
            fatigue_max_base: fatigue.max_base,
            move_range_base: mr_base,
            move_range_bonus: mr_bonus,
            history: HeroHistorySave {
                missions_run: history.missions_run,
                kills: history.kills,
                near_deaths: history.near_deaths,
                rescues_given: history.rescues_given,
                rescues_received: history.rescues_received,
                lifetime_gold: history.lifetime_gold,
                timeline: history.timeline.clone(),
            },
            epithet: maybe_epithet.and_then(|e| e.0.clone()),
            portrait: maybe_portrait.map(|p| HeroPortraitSave {
                base_idx: p.base_idx,
                hair_idx: p.hair_idx,
                hair_color: {
                    let rgba = p.hair_color.to_srgba();
                    [rgba.red, rgba.green, rgba.blue, rgba.alpha]
                },
                gear_idx: p.gear_idx,
            }),
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
            portrait: Some(HeroPortraitSave {
                base_idx: a.portrait.base_idx,
                hair_idx: a.portrait.hair_idx,
                hair_color: {
                    let rgba = a.portrait.hair_color.to_srgba();
                    [rgba.red, rgba.green, rgba.blue, rgba.alpha]
                },
                gear_idx: a.portrait.gear_idx,
            }),
        })
        .collect();

    // 3. Build mission DTOs.
    let mut mission_dtos = Vec::new();

    for (_entity, info, progress, party, dungeon, room_status, children, maybe_log_history, maybe_events_state, maybe_rescue) in &missions {
        let events_fired = maybe_events_state.map_or(0, |es| es.events_fired);
        let max_events = maybe_events_state.map_or(0, |es| es.max_events);
        // Map party entities to hero roster indices.
        let party_hero_indices: Vec<usize> = party
            .0
            .iter()
            .filter_map(|e| entity_to_index.get(e).copied())
            .collect();

        // Get rescue hero indices if present
        let rescue_hero_indices = maybe_rescue.map(|rm| {
            rm.rescue_heroes
                .iter()
                .filter_map(|e| entity_to_index.get(e).copied())
                .collect()
        });
        let rescue_gear_recovered = maybe_rescue.map(|rm| rm.gear_recovered);

        // Collect hero tokens that are children of this mission.
        let mut hero_token_dtos = Vec::new();
        let mut enemy_token_dtos = Vec::new();

        for child in children.iter() {
            if let Ok((ht, pos, combat, in_room, move_range, move_target, active_abilities)) = hero_tokens.get(child) {
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
                    speed: combat.speed,
                    move_range_base: move_range.base,
                    move_range_bonus: move_range.bonus,
                    path: move_target.as_ref().map(|mt| mt.path.clone()),
                    path_index: move_target.as_ref().map_or(0, |mt| mt.path_index),
                    abilities: active_abilities.map_or(Vec::new(), |aa| aa.abilities.clone()),
                });
            }

            if let Ok((et, pos, combat, in_room, move_range)) = enemy_tokens.get(child) {
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
                    speed: combat.speed,
                    move_range_base: move_range.base,
                    move_range_bonus: move_range.bonus,
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
            logs: maybe_log_history.map(|lh| lh.logs.clone()).unwrap_or_default(),
            modifiers: info.modifiers.clone(),
            events_fired,
            max_events,
            biome: info.biome,
            rescue_hero_indices,
            rescue_gear_recovered,
        });
    }

    // Build rescue offer DTOs.
    let mut rescue_offer_dtos = Vec::new();
    if let Some(b) = board {
        for ro in &b.rescue_offers {
            let rescue_hero_indices: Vec<usize> = ro
                .rescue_heroes
                .iter()
                .filter_map(|e| entity_to_index.get(e).copied())
                .collect();
            rescue_offer_dtos.push(RescueOfferSaveDto {
                template_idx: ro.template_idx,
                modifiers: ro.modifiers.clone(),
                map: ro.map.clone(),
                rescue_hero_indices,
                expires_at_remaining: (ro.expires_at - time.elapsed_secs_f64()).max(0.0),
            });
        }
    }

    // 4. Get unix timestamp.
    let timestamp = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    // 5. Assemble SaveData.
    let save_data = SaveData {
        version: CURRENT_SAVE_VERSION,
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
        rescue_offers: rescue_offer_dtos,
        tutorial_done: tutorial.done,
        tutorial_step: tutorial.step,
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

    let tmp_path = path.with_extension("ron.tmp");
    let bak_path = path.with_extension("ron.bak");

    if let Err(e) = std::fs::write(&tmp_path, ron_string) {
        warn!("Failed to write save file to temp path: {e}");
        let _ = std::fs::remove_file(&tmp_path);
        return;
    }

    if path.exists() {
        if let Err(e) = std::fs::rename(&path, &bak_path) {
            warn!("Failed to backup existing save: {e}");
        }
    }

    if let Err(e) = std::fs::rename(&tmp_path, &path) {
        warn!("Failed to replace save file: {e}");
        return;
    }

    info!("Game saved to {}", path.display());

    // 8. Fire toast.
    commands.trigger(ToastEvent {
        title: "Game Saved".to_string(),
        body: "Game saved.".to_string(),
        kind: ToastKind::Info,
        action: None,
    });
}

/// Reads only the `version` field of a save file; every other field is
/// skipped. Missing `version` means a v0 (pre-versioning) save.
#[derive(Deserialize)]
struct SaveVersionProbe {
    #[serde(default)]
    version: u32,
}

pub fn deserialize_and_migrate(ron_string: &str) -> Result<SaveData, String> {
    // Deserialize the version first, then the full SaveData directly from the
    // source string. Never round-trip the data through `ron::Value`: RON's
    // Value type is lossy — unit enum variants (HeroClass, Tile, ...) parse
    // to plain unit values and lose their variant names, so a save containing
    // any hero fails to deserialize back and gets flagged corrupt on load.
    let probe: SaveVersionProbe = ron::from_str(ron_string)
        .map_err(|e| format!("Failed to parse raw RON save: {e}"))?;
    let mut version = probe.version;

    if version > CURRENT_SAVE_VERSION {
        return Err(format!("Save file version ({version}) is newer than supported ({CURRENT_SAVE_VERSION})"));
    }

    while version < CURRENT_SAVE_VERSION {
        let next_version = version + 1;
        info!("Migrating save version from {version} to {next_version}");
        match next_version {
            // v0 -> v1 only stamps the version number; `SaveData`'s serde
            // defaults already accept the v0 layout, so no rewrite is needed.
            // Future structural migrations must rewrite the RON *string* (or
            // a dedicated DTO), not a `ron::Value` tree — see above.
            1 => {}
            _ => {
                return Err(format!("No migration defined to reach version {next_version}"));
            }
        }
        version = next_version;
    }

    let save_data: SaveData = ron::from_str(ron_string)
        .map_err(|e| format!("Failed to deserialize migrated save: {e}"))?;

    Ok(save_data)
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

pub const CURRENT_SAVE_VERSION: u32 = 1;

fn default_save_version() -> u32 {
    1
}

/// Top-level save data structure.
#[derive(Serialize, Deserialize)]
pub struct SaveData {
    #[serde(default = "default_save_version")]
    pub version: u32,
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
    #[serde(default)]
    pub rescue_offers: Vec<RescueOfferSaveDto>,
    /// FT-1 tutorial. Defaults TRUE so saves predating the field skip it.
    #[serde(default = "default_tutorial_done")]
    pub tutorial_done: bool,
    #[serde(default)]
    pub tutorial_step: u32,
}

fn default_tutorial_done() -> bool {
    true
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

#[derive(Serialize, Deserialize, Default, Clone)]
pub struct HeroEquipmentSave {
    pub weapon_tier: u32,
    #[serde(default)]
    pub weapon_rarity: GearRarity,
    #[serde(default)]
    pub weapon_affix: Option<BehavioralAffix>,

    pub armor_tier: u32,
    #[serde(default)]
    pub armor_rarity: GearRarity,
    #[serde(default)]
    pub armor_affix: Option<BehavioralAffix>,

    pub accessory_tier: u32,
    #[serde(default)]
    pub accessory_rarity: GearRarity,
    #[serde(default)]
    pub accessory_affix: Option<BehavioralAffix>,
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

fn default_fatigue_current() -> f32 {
    100.0
}
fn default_fatigue_max_base() -> f32 {
    100.0
}

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct HeroHistorySave {
    #[serde(default)]
    pub missions_run: u32,
    #[serde(default)]
    pub kills: u32,
    #[serde(default)]
    pub near_deaths: u32,
    #[serde(default)]
    pub rescues_given: u32,
    #[serde(default)]
    pub rescues_received: u32,
    #[serde(default)]
    pub lifetime_gold: u32,
    #[serde(default)]
    pub timeline: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct HeroPortraitSave {
    pub base_idx: u32,
    pub hair_idx: u32,
    pub hair_color: [f32; 4],
    pub gear_idx: u32,
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
    pub dropped_equipment: Option<HeroEquipmentSave>,
    #[serde(default)]
    pub injured_remaining: Option<f64>,
    #[serde(default = "default_fatigue_current")]
    pub fatigue_current: f32,
    #[serde(default = "default_fatigue_max_base")]
    pub fatigue_max_base: f32,
    #[serde(default = "default_move_range_base")]
    pub move_range_base: u32,
    #[serde(default)]
    pub move_range_bonus: u32,
    #[serde(default)]
    pub history: HeroHistorySave,
    #[serde(default)]
    pub epithet: Option<String>,
    #[serde(default)]
    pub portrait: Option<HeroPortraitSave>,
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
    #[serde(default)]
    pub portrait: Option<HeroPortraitSave>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RescueOfferSaveDto {
    pub template_idx: usize,
    pub modifiers: Vec<crate::mission::data::MissionModifier>,
    pub map: DungeonMap,
    pub rescue_hero_indices: Vec<usize>,
    pub expires_at_remaining: f64,
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
    #[serde(default)]
    pub logs: Vec<MissionLogEntry>,
    #[serde(default)]
    pub modifiers: Vec<crate::mission::data::MissionModifier>,
    #[serde(default)]
    pub events_fired: u32,
    #[serde(default)]
    pub max_events: u32,
    #[serde(default)]
    pub biome: crate::mission::data::BiomeType,
    #[serde(default)]
    pub rescue_hero_indices: Option<Vec<usize>>,
    #[serde(default)]
    pub rescue_gear_recovered: Option<bool>,
}

fn default_token_speed() -> i32 {
    10
}

fn default_move_range_base() -> u32 {
    3
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
    #[serde(default = "default_token_speed")]
    pub speed: i32,
    #[serde(default = "default_move_range_base")]
    pub move_range_base: u32,
    #[serde(default)]
    pub move_range_bonus: u32,
    pub path: Option<Vec<(u32, u32)>>,
    pub path_index: usize,
    #[serde(default)]
    pub abilities: Vec<crate::mission::entities::ActiveAbilityState>,
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
    #[serde(default = "default_token_speed")]
    pub speed: i32,
    #[serde(default = "default_move_range_base")]
    pub move_range_base: u32,
    #[serde(default)]
    pub move_range_bonus: u32,
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
    if is_zero_growth(saved)
        && let Some(class_def) = class_db.get(class) {
            let mut rng = rand::rng();
            return roll_growth(class_def, 0.5, &mut rng);
        }
        // Class not found — fall through to the zeroed value below as a
        // harmless last-resort default.
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
    fn test_save_migration_v0_to_v1() {
        let v0_raw = r#"(
            last_save_timestamp: 12345,
            gold: 500,
            reputation: 5,
            banked_seconds: 10.0,
            materials: {},
            buildings: {},
            heroes: [],
            applicants: [],
            next_arrival_timer: 1.0,
            training_timer: 2.0,
            missions: [],
        )"#;

        let migrated = deserialize_and_migrate(v0_raw).expect("v0 save should migrate to v1");
        assert_eq!(migrated.version, 1);
        assert_eq!(migrated.gold, 500);
    }

    #[test]
    fn tutorial_fields_default_done_for_old_saves() {
        // A pre-FT-1 save has no tutorial fields — it must deserialize as done
        // so existing players never see the tutorial.
        let old_save = r#"(
            version: 1,
            last_save_timestamp: 12345,
            gold: 500,
            reputation: 5,
            banked_seconds: 10.0,
            materials: {},
            buildings: {},
            heroes: [],
            applicants: [],
            next_arrival_timer: 1.0,
            training_timer: 2.0,
            missions: [],
        )"#;

        let parsed = deserialize_and_migrate(old_save).expect("old save parses");
        assert!(parsed.tutorial_done);
        assert_eq!(parsed.tutorial_step, 0);
    }

    #[test]
    fn test_save_backup_and_recovery() {
        use std::fs;

        let temp_dir = std::env::temp_dir().join("guild_forge_test_backup");
        let _ = fs::create_dir_all(&temp_dir);
        let save_path = temp_dir.join("save.ron");
        let bak_path = temp_dir.join("save.ron.bak");

        let _ = fs::remove_file(&save_path);
        let _ = fs::remove_file(&bak_path);

        let original = SaveData {
            version: CURRENT_SAVE_VERSION,
            last_save_timestamp: 12345,
            gold: 333,
            reputation: 3,
            banked_seconds: 0.0,
            materials: HashMap::new(),
            buildings: HashMap::new(),
            heroes: vec![],
            applicants: vec![],
            next_arrival_timer: 0.0,
            training_timer: 0.0,
            missions: vec![],
            rescue_offers: vec![],
            tutorial_done: true,
            tutorial_step: 0,
        };
        let serialized = ron::ser::to_string_pretty(&original, ron::ser::PrettyConfig::default()).unwrap();
        fs::write(&bak_path, &serialized).unwrap();
        fs::write(&save_path, "corrupted save content").unwrap();

        let mut loaded_data = None;

        if save_path.exists() {
            if let Ok(ron_string) = fs::read_to_string(&save_path) {
                match deserialize_and_migrate(&ron_string) {
                    Ok(data) => {
                        loaded_data = Some(data);
                    }
                    Err(_) => {}
                }
            }
        }

        if loaded_data.is_none() && bak_path.exists() {
            if let Ok(ron_string) = fs::read_to_string(&bak_path) {
                match deserialize_and_migrate(&ron_string) {
                    Ok(data) => {
                        loaded_data = Some(data);
                    }
                    Err(_) => {}
                }
            }
        }

        let save_data = loaded_data.expect("Should fall back to backup file");
        assert_eq!(save_data.gold, 333);

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_corrupt_save_fallback_no_crash() {
        use std::fs;
        let temp_dir = std::env::temp_dir().join("guild_forge_test_corrupt");
        let _ = fs::create_dir_all(&temp_dir);
        let save_path = temp_dir.join("save.ron");
        let bak_path = temp_dir.join("save.ron.bak");

        fs::write(&save_path, "corrupt").unwrap();
        fs::write(&bak_path, "corrupt").unwrap();

        let r1 = fs::read_to_string(&save_path).ok().and_then(|s| deserialize_and_migrate(&s).ok());
        let r2 = fs::read_to_string(&bak_path).ok().and_then(|s| deserialize_and_migrate(&s).ok());

        assert!(r1.is_none());
        assert!(r2.is_none());

        if r1.is_none() && r2.is_none() && save_path.exists() {
            let corrupt_path = save_path.with_extension("ron.corrupt");
            let _ = fs::rename(&save_path, &corrupt_path);
            assert!(corrupt_path.exists());
            assert!(!save_path.exists());
            let _ = fs::remove_file(&corrupt_path);
        }

        let _ = fs::remove_dir_all(&temp_dir);
    }

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
                ..Default::default()
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
            dropped_equipment: None,
            injured_remaining: None,
            fatigue_current: 85.0,
            fatigue_max_base: 100.0,
            move_range_base: 3,
            move_range_bonus: 0,
            history: HeroHistorySave::default(),
            epithet: None,
            portrait: None,
        };
        let s = ron::ser::to_string(&dto).unwrap();
        let back: HeroSaveDto = ron::from_str(&s).unwrap();
        assert!((back.growth.strength - 1.1).abs() < 1e-5);
        assert!((back.growth.charisma - 0.2).abs() < 1e-5);
        assert!((back.progress.strength - 0.5).abs() < 1e-5);
        assert_eq!(back.fatigue_current, 85.0);
        assert_eq!(back.fatigue_max_base, 100.0);
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
                ..Default::default()
            },
            on_mission: false,
            growth: HeroGrowthSave::default(),
            progress: HeroStatProgressSave::default(),
            favorite: true,
            personally_managed: true,
            missing_remaining: None,
            dropped_equipment: None,
            injured_remaining: None,
            fatigue_current: 100.0,
            fatigue_max_base: 100.0,
            move_range_base: 3,
            move_range_bonus: 0,
            history: HeroHistorySave::default(),
            epithet: None,
            portrait: None,
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
            equipment: HeroEquipmentSave { weapon_tier: 0, armor_tier: 0, accessory_tier: 0, ..Default::default() },
            on_mission: false,
            growth: HeroGrowthSave::default(),
            progress: HeroStatProgressSave::default(),
            favorite: false,
            personally_managed: false,
            missing_remaining: Some(42.0),
            dropped_equipment: None,
            injured_remaining: Some(200.0),
            fatigue_current: 100.0,
            fatigue_max_base: 100.0,
            move_range_base: 3,
            move_range_bonus: 0,
            history: HeroHistorySave::default(),
            epithet: None,
            portrait: None,
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
    fn legacy_hero_save_dto_without_fatigue_defaults_100() {
        // Old-format save (no fatigue fields) should deserialize with 100.0.
        let old = r#"(name:"A",class:Warrior,level:1,xp:0,xp_to_next:100,
            stats:(strength:10,dexterity:10,constitution:10,intelligence:10,wisdom:10,charisma:10),
            traits:[],equipment:(weapon_tier:0,armor_tier:0,accessory_tier:0),on_mission:false)"#;
        let back: HeroSaveDto = ron::from_str(old).unwrap();
        assert_eq!(back.fatigue_current, 100.0);
        assert_eq!(back.fatigue_max_base, 100.0);
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

    #[test]
    fn hero_save_dto_round_trips_with_history() {
        let dto = HeroSaveDto {
            name: "H".into(),
            class: HeroClass::Warrior,
            level: 1,
            xp: 0,
            xp_to_next: 100,
            stats: HeroStatsSave { strength: 10, dexterity: 10, constitution: 10,
                intelligence: 10, wisdom: 10, charisma: 10 },
            traits: vec![],
            equipment: HeroEquipmentSave { weapon_tier: 0, armor_tier: 0, accessory_tier: 0, ..Default::default() },
            on_mission: false,
            growth: HeroGrowthSave::default(),
            progress: HeroStatProgressSave::default(),
            favorite: false,
            personally_managed: false,
            missing_remaining: None,
            dropped_equipment: None,
            injured_remaining: None,
            fatigue_current: 100.0,
            fatigue_max_base: 100.0,
            move_range_base: 3,
            move_range_bonus: 0,
            history: HeroHistorySave {
                missions_run: 5,
                kills: 12,
                near_deaths: 2,
                rescues_given: 1,
                rescues_received: 0,
                lifetime_gold: 150,
                timeline: vec!["Joined the guild".to_string(), "Defeated Boss Rat".to_string()],
            },
            epithet: Some("the Savior".to_string()),
            portrait: None,
        };
        let s = ron::to_string(&dto).unwrap();
        let back: HeroSaveDto = ron::from_str(&s).unwrap();
        assert_eq!(back.epithet, Some("the Savior".to_string()));
        assert_eq!(back.history.missions_run, 5);
        assert_eq!(back.history.kills, 12);
        assert_eq!(back.history.near_deaths, 2);
        assert_eq!(back.history.rescues_given, 1);
        assert_eq!(back.history.rescues_received, 0);
        assert_eq!(back.history.lifetime_gold, 150);
        assert_eq!(back.history.timeline.len(), 2);
        assert_eq!(back.history.timeline[1], "Defeated Boss Rat");
    }

    /// Regression test: a save taken while a mission is dispatched must
    /// survive the exact round-trip the game performs — serialized with
    /// `ron::ser::to_string_pretty` in `handle_save`, then read back through
    /// `deserialize_and_migrate` in `load_save`. If this fails, the save is
    /// flagged corrupt on relaunch and the player loads into an empty guild.
    #[test]
    fn full_save_with_dispatched_mission_round_trips_through_migration() {
        use crate::mission::dungeon::{Room, RoomType, Tile};

        let hero = |name: &str, on_mission: bool| HeroSaveDto {
            name: name.into(),
            class: HeroClass::Warrior,
            level: 2,
            xp: 10,
            xp_to_next: 150,
            stats: HeroStatsSave {
                strength: 12, dexterity: 10, constitution: 11,
                intelligence: 9, wisdom: 10, charisma: 8,
            },
            traits: vec![],
            equipment: HeroEquipmentSave {
                weapon_tier: 1, armor_tier: 1, accessory_tier: 0,
                ..Default::default()
            },
            on_mission,
            growth: HeroGrowthSave {
                strength: 1.0, dexterity: 0.5, constitution: 0.8,
                intelligence: 0.2, wisdom: 0.3, charisma: 0.1,
            },
            progress: HeroStatProgressSave::default(),
            favorite: false,
            personally_managed: false,
            missing_remaining: None,
            dropped_equipment: None,
            injured_remaining: None,
            fatigue_current: 92.5,
            fatigue_max_base: 100.0,
            move_range_base: 3,
            move_range_bonus: 0,
            history: HeroHistorySave::default(),
            epithet: None,
            portrait: Some(HeroPortraitSave {
                base_idx: 1,
                hair_idx: 2,
                hair_color: [0.5, 0.3, 0.1, 1.0],
                gear_idx: 0,
            }),
        };

        let mission = MissionSaveDto {
            template_id: "goblin_cave".into(),
            name: "Goblin Cave".into(),
            difficulty: 1,
            progress: MissionProgress::InProgress,
            rng_seed: 0,
            party_hero_indices: vec![2],
            dungeon_map: DungeonMap {
                width: 3,
                height: 2,
                tiles: vec![
                    Tile::Wall, Tile::Floor, Tile::Door,
                    Tile::Corridor, Tile::Floor, Tile::Wall,
                ],
                rooms: vec![Room { x: 1, y: 0, w: 1, h: 1, room_type: RoomType::Entrance }],
            },
            room_visited: vec![true],
            room_cleared: vec![false],
            hero_tokens: vec![HeroTokenDto {
                roster_index: 2,
                grid_x: 1,
                grid_y: 0,
                in_room: Some(0),
                hp: 20,
                max_hp: 25,
                attack: 5,
                defense: 3,
                speed: 10,
                move_range_base: 3,
                move_range_bonus: 0,
                path: Some(vec![(1, 0), (1, 1)]),
                path_index: 0,
                abilities: vec![crate::mission::entities::ActiveAbilityState {
                    ability_id: "power_strike".into(),
                    remaining_cooldown: 1,
                }],
            }],
            enemy_tokens: vec![EnemyTokenDto {
                enemy_type: EnemyType::Goblin,
                xp_reward: 10,
                grid_x: 2,
                grid_y: 1,
                in_room: Some(0),
                hp: 8,
                max_hp: 8,
                attack: 3,
                defense: 1,
                speed: 10,
                move_range_base: 3,
                move_range_bonus: 0,
            }],
            logs: vec![MissionLogEntry {
                text: "Sella entered the Entrance".into(),
                kind: crate::ui::feed::LogKind::RoomEntry,
                hero_name: Some("Sella".into()),
            }],
            modifiers: vec![crate::mission::data::MissionModifier::Foggy],
            events_fired: 1,
            max_events: 3,
            biome: crate::mission::data::BiomeType::Dungeon,
            rescue_hero_indices: None,
            rescue_gear_recovered: None,
        };

        let save_data = SaveData {
            version: CURRENT_SAVE_VERSION,
            last_save_timestamp: 1_783_660_000,
            gold: 250,
            reputation: 4,
            banked_seconds: 12.5,
            materials: HashMap::from([(MaterialType::IronOre, 3)]),
            buildings: HashMap::new(),
            heroes: vec![hero("Aldric", false), hero("Brenna", false), hero("Sella", true)],
            applicants: vec![],
            next_arrival_timer: 30.0,
            training_timer: 5.0,
            missions: vec![mission],
            rescue_offers: vec![],
            tutorial_done: true,
            tutorial_step: 0,
        };

        // Exactly what handle_save writes to disk.
        let ron_string =
            ron::ser::to_string_pretty(&save_data, ron::ser::PrettyConfig::default())
                .expect("save should serialize");

        // Exactly what load_save runs on relaunch.
        let loaded = deserialize_and_migrate(&ron_string)
            .expect("a save with a dispatched mission should load back");

        assert_eq!(loaded.heroes.len(), 3);
        assert!(loaded.heroes[2].on_mission);
        assert_eq!(loaded.missions.len(), 1);
        assert_eq!(loaded.missions[0].party_hero_indices, vec![2]);
        assert_eq!(loaded.missions[0].hero_tokens.len(), 1);
        assert_eq!(loaded.missions[0].hero_tokens[0].roster_index, 2);
    }

    #[test]
    fn legacy_hero_save_dto_without_history_defaults() {
        let old = r#"(name:"A",class:Warrior,level:1,xp:0,xp_to_next:100,
            stats:(strength:10,dexterity:10,constitution:10,intelligence:10,wisdom:10,charisma:10),
            traits:[],equipment:(weapon_tier:0,armor_tier:0,accessory_tier:0),on_mission:false)"#;
        let back: HeroSaveDto = ron::from_str(old).unwrap();
        assert_eq!(back.history.missions_run, 0);
        assert_eq!(back.history.kills, 0);
        assert_eq!(back.history.near_deaths, 0);
        assert_eq!(back.history.rescues_given, 0);
        assert_eq!(back.history.rescues_received, 0);
        assert_eq!(back.history.lifetime_gold, 0);
        assert!(back.history.timeline.is_empty());
    }
}
