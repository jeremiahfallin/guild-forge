//! Combat resolution: attack rolls, damage, healing, death.
//!
//! All systems iterate missions and walk `Children` so combat stays scoped
//! to each mission's own token pool.

use bevy::prelude::*;
use bevy::ecs::message::MessageWriter;
use rand::Rng;

use crate::localization::{tr, trf};
use crate::economy::Gold;
use crate::hero::status::{Missing, MISSING_DURATION_SECS};
use crate::hero::Favorite;
use crate::hero::{Hero, HeroInfo, HeroStats, HeroTraits, Epithet, format_hero_name};
use crate::ui::toast::{ToastEvent, ToastKind};
use crate::ui::feed::{MissionLogEvent, MissionLogPayload, MissionLogHistory};

fn get_clean_name(name: Option<&Name>, default: &str) -> String {
    if let Some(n) = name {
        let s = n.as_str();
        if let Some(stripped) = s.strip_prefix("Hero Token: ") {
            stripped.to_string()
        } else if let Some(stripped) = s.strip_prefix("Enemy: ") {
            stripped.to_string()
        } else {
            s.to_string()
        }
    } else {
        default.to_string()
    }
}

use super::data::{MissionTemplateDatabase, EventDatabase, EventCheckStat};
use super::entities::*;
use super::{Mission, MissionDungeon, MissionInfo, MissionParty, MissionProgress};



/// Despawn dead enemies. Dead heroes stay as logical entities so the
/// completion system can still read them; their proxies hide via the sync
/// system when `hp <= 0`.
pub fn handle_death_system(
    mut commands: Commands,
    dead_enemies: Query<(Entity, &CombatStats), With<EnemyToken>>,
) {
    for (entity, combat) in &dead_enemies {
        if combat.hp <= 0 {
            commands.entity(entity).despawn();
        }
    }
}

pub fn update_room_status(
    mut commands: Commands,
    mut missions: Query<(
        Entity,
        &MissionDungeon,
        &mut RoomStatus,
        &Children,
        Option<&MissionInfo>,
        Option<&mut MissionEventsState>,
        Option<&MissionLogHistory>,
        Option<&mut crate::mission::RescueMission>,
    ), With<Mission>>,
    mut heroes: Query<(Entity, &InRoom, &HeroToken, Option<&Name>, &mut CombatStats), Without<EnemyToken>>,
    enemies: Query<(&InRoom, &CombatStats), With<EnemyToken>>,
    mut active_abilities: Query<&mut ActiveAbilities>,
    ability_db: Option<Res<crate::hero::data::AbilityDatabase>>,
    events_db: Option<Res<EventDatabase>>,
    roster_heroes: Query<(&HeroTraits, &HeroStats), With<Hero>>,
    mut gold_res: Option<ResMut<crate::economy::Gold>>,
    mut log_writer: MessageWriter<MissionLogEvent>,
) {
    for (mission_entity, dungeon, mut room_status, children, info, mut events_state_opt, log_history, mut rescue_mission_opt) in &mut missions {
        let map = &dungeon.0;
        let mut newly_visited: Option<(Entity, usize)> = None;

        // Mark rooms as visited when this mission's heroes enter
        for c in children.iter() {
            if let Ok((entity, in_room, _hero_token, name, mut combat_stats)) = heroes.get_mut(c)
                && let Some(room_idx) = in_room.0
                    && room_idx < room_status.visited.len() && !room_status.visited[room_idx] {
                        room_status.visited[room_idx] = true;
                        newly_visited = Some((entity, room_idx));
                        
                        let hero_name = get_clean_name(name, "Hero");
                        let room_type = map.rooms[room_idx].room_type;
                        let room_name = match room_type {
                            crate::mission::dungeon::RoomType::Entrance => "Entrance",
                            crate::mission::dungeon::RoomType::Boss => "Boss Chamber",
                            crate::mission::dungeon::RoomType::Treasure => "Treasure Vault",
                            crate::mission::dungeon::RoomType::Normal => "Chamber",
                        }.to_string();

                        log_writer.write(MissionLogEvent {
                            mission_entity,
                            payload: MissionLogPayload::RoomEntry {
                                hero_name: hero_name.clone(),
                                room_name,
                            },
                        });

                        let is_entrance = room_type == crate::mission::dungeon::RoomType::Entrance;
                        let has_trapped = info.map(|i| i.modifiers.contains(&super::data::MissionModifier::Trapped)).unwrap_or(false);
                        if !is_entrance && has_trapped {
                            let mut rng = rand::rng();
                            let damage = rng.random_range(5..=15);
                            combat_stats.hp = (combat_stats.hp - damage).max(0);

                            commands.entity(entity).insert(crate::mission::entities::VisualHit {
                                amount: damage,
                                is_hit: true,
                                is_crit: false,
                                effect_type: "Damage".to_string(),
                                source: None,
                                is_signature: false,
                            });

                            log_writer.write(MissionLogEvent {
                                mission_entity,
                                payload: MissionLogPayload::TrapTriggered {
                                    hero_name: hero_name.clone(),
                                    damage,
                                },
                            });
                        }
                    }
        }

        // Trigger mid-mission events if a new chamber was visited
        if let Some((_triggering_hero_ent, room_idx)) = newly_visited {
            let room_type = map.rooms[room_idx].room_type;
            let is_entrance = room_type == crate::mission::dungeon::RoomType::Entrance;

            if !is_entrance {
                if let Some(ref mut events_state) = events_state_opt
                    && let Some(events_db) = events_db.as_ref()
                    && !events_db.0.is_empty() {
                        let events_left = events_state.max_events.saturating_sub(events_state.events_fired);
                        if events_left > 0 {
                            let unvisited_chambers_count = room_status.visited.iter().filter(|&&v| !v).count();
                            let force_trigger = events_left as usize > unvisited_chambers_count;
                            let mut rng = rand::rng();

                            if force_trigger || rng.random_bool(0.5) {
                                // Select event definition, avoiding repeats if possible
                                let mut available_events = events_db.0.clone();
                                if let Some(history) = log_history {
                                    available_events.retain(|evt| {
                                        !history.logs.iter().any(|log| log.text.contains(&evt.name))
                                    });
                                }
                                if available_events.is_empty() {
                                    available_events = events_db.0.clone();
                                }

                                // Filter based on whether this is a rescue mission
                                let is_rescue_mission = rescue_mission_opt.is_some();
                                if is_rescue_mission {
                                    available_events.retain(|evt| evt.id.starts_with("rescue_"));
                                } else {
                                    available_events.retain(|evt| !evt.id.starts_with("rescue_"));
                                }

                                if available_events.is_empty() {
                                    available_events = events_db.0.clone();
                                }

                                // If rescue, prioritize in order: trail -> dropped_gear -> campsite
                                let event_def = if is_rescue_mission {
                                    let order = ["rescue_trail", "rescue_dropped_gear", "rescue_campsite"];
                                    let mut selected = None;
                                    for id in order {
                                        if let Some(evt) = available_events.iter().find(|e| e.id == id) {
                                            selected = Some(evt.clone());
                                            break;
                                        }
                                    }
                                    selected.unwrap_or_else(|| {
                                        available_events[rng.random_range(0..available_events.len())].clone()
                                    })
                                } else {
                                    available_events[rng.random_range(0..available_events.len())].clone()
                                };

                                // Find all living heroes currently in this room
                                let mut room_heroes = Vec::new();
                                for &c in children {
                                    if let Ok((entity, in_room, hero_token, name, combat_stats)) = heroes.get_mut(c) {
                                        if in_room.0 == Some(room_idx) && combat_stats.hp > 0 {
                                            let hero_name = get_clean_name(name, "Hero");
                                            let traits = roster_heroes
                                                .get(hero_token.0)
                                                .map(|(t, _)| HeroTraits(t.0.clone()))
                                                .unwrap_or_else(|_| HeroTraits(vec![]));
                                            room_heroes.push((entity, hero_token.0, hero_name, traits));
                                        }
                                    }
                                }

                                if !room_heroes.is_empty() {
                                    // Prioritize heroes with the matching trait
                                    let selected_hero_info = if let Some(priority_trait) = event_def.priority_trait {
                                        let with_trait: Vec<_> = room_heroes.iter()
                                            .filter(|(_, _, _, traits)| traits.0.contains(&priority_trait))
                                            .collect();
                                        if !with_trait.is_empty() {
                                            Some((*with_trait[rng.random_range(0..with_trait.len())]).clone())
                                        } else {
                                            None
                                        }
                                    } else {
                                        None
                                    };

                                    let (selected_token_ent, selected_roster_ent, hero_name, _) = selected_hero_info.unwrap_or_else(|| {
                                        room_heroes[rng.random_range(0..room_heroes.len())].clone()
                                    });

                                    // Resolve d20 check roll
                                    let mut stat_score = 10;
                                    if let Ok((_, roster_stats)) = roster_heroes.get(selected_roster_ent) {
                                        stat_score = match event_def.check_stat {
                                            EventCheckStat::Strength => roster_stats.strength,
                                            EventCheckStat::Dexterity => roster_stats.dexterity,
                                            EventCheckStat::Constitution => roster_stats.constitution,
                                            EventCheckStat::Intelligence => roster_stats.intelligence,
                                            EventCheckStat::Wisdom => roster_stats.wisdom,
                                            EventCheckStat::Charisma => roster_stats.charisma,
                                        };
                                    }

                                    let modifier = ((stat_score - 10) as f32 / 2.0).floor() as i32;
                                    let d20 = rng.random_range(1..=20);
                                    let check_result = d20 + modifier;
                                    let success = check_result >= event_def.check_difficulty;

                                    // Apply outcomes
                                    if let Ok((_, _, _, _, mut combat_stats)) = heroes.get_mut(selected_token_ent) {
                                        match event_def.id.as_str() {
                                            "rescue_trail" => {
                                                if success {
                                                    combat_stats.hp = (combat_stats.hp + 10).min(combat_stats.max_hp);
                                                    combat_stats.speed += 1;
                                                    commands.entity(selected_token_ent).insert(crate::mission::entities::VisualHit {
                                                        amount: 10,
                                                        is_hit: false,
                                                        is_crit: false,
                                                        effect_type: "Heal".to_string(),
                                                        source: None,
                                                        is_signature: false,
                                                    });
                                                } else {
                                                    combat_stats.hp = (combat_stats.hp - 8).max(0);
                                                    commands.entity(selected_token_ent).insert(crate::mission::entities::VisualHit {
                                                        amount: 8,
                                                        is_hit: true,
                                                        is_crit: false,
                                                        effect_type: "Damage".to_string(),
                                                        source: None,
                                                        is_signature: false,
                                                    });
                                                }
                                            }
                                            "rescue_campsite" => {
                                                if success {
                                                    for &c in children {
                                                        if let Ok((entity, _, _, _, mut cs)) = heroes.get_mut(c) {
                                                            if cs.hp > 0 {
                                                                cs.hp = (cs.hp + 15).min(cs.max_hp);
                                                                commands.entity(entity).insert(crate::mission::entities::VisualHit {
                                                                    amount: 15,
                                                                    is_hit: false,
                                                                    is_crit: false,
                                                                    effect_type: "Heal".to_string(),
                                                                    source: None,
                                                                    is_signature: false,
                                                                });
                                                            }
                                                        }
                                                    }
                                                } else {
                                                    combat_stats.hp = (combat_stats.hp - 10).max(0);
                                                    commands.entity(selected_token_ent).insert(crate::mission::entities::VisualHit {
                                                        amount: 10,
                                                        is_hit: true,
                                                        is_crit: false,
                                                        effect_type: "Damage".to_string(),
                                                        source: None,
                                                        is_signature: false,
                                                    });
                                                }
                                            }
                                            "rescue_dropped_gear" => {
                                                if let Some(ref mut rm) = rescue_mission_opt {
                                                    rm.gear_recovered = true;
                                                }
                                                if success {
                                                    if let Some(ref mut g) = gold_res {
                                                        g.0 += 150;
                                                    }
                                                } else {
                                                    combat_stats.hp = (combat_stats.hp - 12).max(0);
                                                    commands.entity(selected_token_ent).insert(crate::mission::entities::VisualHit {
                                                        amount: 12,
                                                        is_hit: true,
                                                        is_crit: false,
                                                        effect_type: "Damage".to_string(),
                                                        source: None,
                                                        is_signature: false,
                                                    });
                                                }
                                            }
                                            "mysterious_shrine" => {
                                                if success {
                                                    combat_stats.hp = (combat_stats.hp + 15).min(combat_stats.max_hp);
                                                    combat_stats.defense += 2;
                                                    commands.entity(selected_token_ent).insert(crate::mission::entities::VisualHit {
                                                        amount: 15,
                                                        is_hit: false,
                                                        is_crit: false,
                                                        effect_type: "Heal".to_string(),
                                                        source: None,
                                                        is_signature: false,
                                                    });
                                                } else {
                                                    combat_stats.hp = (combat_stats.hp - 10).max(0);
                                                    commands.entity(selected_token_ent).insert(crate::mission::entities::VisualHit {
                                                        amount: 10,
                                                        is_hit: true,
                                                        is_crit: false,
                                                        effect_type: "Damage".to_string(),
                                                        source: None,
                                                        is_signature: false,
                                                    });
                                                }
                                            }
                                            "swinging_blade_trap" => {
                                                if !success {
                                                    combat_stats.hp = (combat_stats.hp - 12).max(0);
                                                    commands.entity(selected_token_ent).insert(crate::mission::entities::VisualHit {
                                                        amount: 12,
                                                        is_hit: true,
                                                        is_crit: false,
                                                        effect_type: "Damage".to_string(),
                                                        source: None,
                                                        is_signature: false,
                                                    });
                                                }
                                            }
                                            "hidden_vault" => {
                                                if success {
                                                    if let Some(ref mut g) = gold_res {
                                                        g.0 += 100;
                                                    }
                                                } else {
                                                    combat_stats.hp = (combat_stats.hp - 8).max(0);
                                                    commands.entity(selected_token_ent).insert(crate::mission::entities::VisualHit {
                                                        amount: 8,
                                                        is_hit: true,
                                                        is_crit: false,
                                                        effect_type: "Damage".to_string(),
                                                        source: None,
                                                        is_signature: false,
                                                    });
                                                }
                                            }
                                            "wandering_merchant" => {
                                                if success {
                                                    combat_stats.hp = (combat_stats.hp + 15).min(combat_stats.max_hp);
                                                    combat_stats.defense += 1;
                                                    commands.entity(selected_token_ent).insert(crate::mission::entities::VisualHit {
                                                        amount: 15,
                                                        is_hit: false,
                                                        is_crit: false,
                                                        effect_type: "Heal".to_string(),
                                                        source: None,
                                                        is_signature: false,
                                                    });
                                                } else {
                                                    if let Some(ref mut g) = gold_res {
                                                        g.0 = g.0.saturating_sub(30);
                                                    }
                                                }
                                            }
                                            "collapsed_floor" => {
                                                if !success {
                                                    combat_stats.hp = (combat_stats.hp - 15).max(0);
                                                    commands.entity(selected_token_ent).insert(crate::mission::entities::VisualHit {
                                                        amount: 15,
                                                        is_hit: true,
                                                        is_crit: false,
                                                        effect_type: "Damage".to_string(),
                                                        source: None,
                                                        is_signature: false,
                                                    });
                                                }
                                            }
                                            "rival_guild_cameo" => {
                                                if success {
                                                    if let Some(ref mut g) = gold_res {
                                                        g.0 += 50;
                                                    }
                                                } else {
                                                    combat_stats.hp = (combat_stats.hp - 10).max(0);
                                                    commands.entity(selected_token_ent).insert(crate::mission::entities::VisualHit {
                                                        amount: 10,
                                                        is_hit: true,
                                                        is_crit: false,
                                                        effect_type: "Damage".to_string(),
                                                        source: None,
                                                        is_signature: false,
                                                    });
                                                }
                                            }
                                            "cursed_fountain" => {
                                                if success {
                                                    combat_stats.hp = (combat_stats.hp + 20).min(combat_stats.max_hp);
                                                    commands.entity(selected_token_ent).insert(crate::mission::entities::VisualHit {
                                                        amount: 20,
                                                        is_hit: false,
                                                        is_crit: false,
                                                        effect_type: "Heal".to_string(),
                                                        source: None,
                                                        is_signature: false,
                                                    });
                                                } else {
                                                    combat_stats.hp = (combat_stats.hp - 15).max(0);
                                                    combat_stats.defense = (combat_stats.defense - 2).max(0);
                                                    commands.entity(selected_token_ent).insert(crate::mission::entities::VisualHit {
                                                        amount: 15,
                                                        is_hit: true,
                                                        is_crit: false,
                                                        effect_type: "Damage".to_string(),
                                                        source: None,
                                                        is_signature: false,
                                                    });
                                                }
                                            }
                                            "ancient_tomb" => {
                                                if success {
                                                    if let Some(ref mut g) = gold_res {
                                                        g.0 += 80;
                                                    }
                                                } else {
                                                    combat_stats.hp = (combat_stats.hp - 14).max(0);
                                                    commands.entity(selected_token_ent).insert(crate::mission::entities::VisualHit {
                                                        amount: 14,
                                                        is_hit: true,
                                                        is_crit: false,
                                                        effect_type: "Damage".to_string(),
                                                        source: None,
                                                        is_signature: false,
                                                    });
                                                }
                                            }
                                            "glowing_mushrooms" => {
                                                if success {
                                                    combat_stats.hp = (combat_stats.hp + 20).min(combat_stats.max_hp);
                                                    commands.entity(selected_token_ent).insert(crate::mission::entities::VisualHit {
                                                        amount: 20,
                                                        is_hit: false,
                                                        is_crit: false,
                                                        effect_type: "Heal".to_string(),
                                                        source: None,
                                                        is_signature: false,
                                                    });
                                                } else {
                                                    combat_stats.hp = (combat_stats.hp - 10).max(0);
                                                    commands.entity(selected_token_ent).insert(crate::mission::entities::VisualHit {
                                                        amount: 10,
                                                        is_hit: true,
                                                        is_crit: false,
                                                        effect_type: "Damage".to_string(),
                                                        source: None,
                                                        is_signature: false,
                                                    });
                                                }
                                            }
                                            "rubble_pile" => {
                                                if success {
                                                    if let Some(ref mut g) = gold_res {
                                                        g.0 += 60;
                                                    }
                                                } else {
                                                    combat_stats.hp = (combat_stats.hp - 10).max(0);
                                                    commands.entity(selected_token_ent).insert(crate::mission::entities::VisualHit {
                                                        amount: 10,
                                                        is_hit: true,
                                                        is_crit: false,
                                                        effect_type: "Damage".to_string(),
                                                        source: None,
                                                        is_signature: false,
                                                    });
                                                }
                                            }
                                            "lucky_coin" => {
                                                if success {
                                                    if let Some(ref mut g) = gold_res {
                                                        g.0 += 30;
                                                    }
                                                } else {
                                                    combat_stats.hp = (combat_stats.hp - 6).max(0);
                                                    commands.entity(selected_token_ent).insert(crate::mission::entities::VisualHit {
                                                        amount: 6,
                                                        is_hit: true,
                                                        is_crit: false,
                                                        effect_type: "Damage".to_string(),
                                                        source: None,
                                                        is_signature: false,
                                                    });
                                                }
                                            }
                                            "ominous_altar" => {
                                                if success {
                                                    combat_stats.defense += 3;
                                                } else {
                                                    combat_stats.hp = (combat_stats.hp - 10).max(0);
                                                    commands.entity(selected_token_ent).insert(crate::mission::entities::VisualHit {
                                                        amount: 10,
                                                        is_hit: true,
                                                        is_crit: false,
                                                        effect_type: "Damage".to_string(),
                                                        source: None,
                                                        is_signature: false,
                                                    });
                                                }
                                            }
                                            "old_library" => {
                                                if success {
                                                    combat_stats.defense += 2;
                                                } else {
                                                    combat_stats.hp = (combat_stats.hp - 10).max(0);
                                                    commands.entity(selected_token_ent).insert(crate::mission::entities::VisualHit {
                                                        amount: 10,
                                                        is_hit: true,
                                                        is_crit: false,
                                                        effect_type: "Damage".to_string(),
                                                        source: None,
                                                        is_signature: false,
                                                    });
                                                }
                                            }
                                            "guild_recruiter" => {
                                                if success {
                                                    combat_stats.hp = (combat_stats.hp + 10).min(combat_stats.max_hp);
                                                    combat_stats.defense += 2;
                                                    commands.entity(selected_token_ent).insert(crate::mission::entities::VisualHit {
                                                        amount: 10,
                                                        is_hit: false,
                                                        is_crit: false,
                                                        effect_type: "Heal".to_string(),
                                                        source: None,
                                                        is_signature: false,
                                                    });
                                                }
                                            }
                                            "bee_hive" => {
                                                if success {
                                                    combat_stats.hp = (combat_stats.hp + 20).min(combat_stats.max_hp);
                                                    commands.entity(selected_token_ent).insert(crate::mission::entities::VisualHit {
                                                        amount: 20,
                                                        is_hit: false,
                                                        is_crit: false,
                                                        effect_type: "Heal".to_string(),
                                                        source: None,
                                                        is_signature: false,
                                                    });
                                                } else {
                                                    combat_stats.hp = (combat_stats.hp - 8).max(0);
                                                    commands.entity(selected_token_ent).insert(crate::mission::entities::VisualHit {
                                                        amount: 8,
                                                        is_hit: true,
                                                        is_crit: false,
                                                        effect_type: "Damage".to_string(),
                                                        source: None,
                                                        is_signature: false,
                                                    });
                                                }
                                            }
                                            "tripwire_bell" => {
                                                if !success {
                                                    combat_stats.hp = (combat_stats.hp - 9).max(0);
                                                    commands.entity(selected_token_ent).insert(crate::mission::entities::VisualHit {
                                                        amount: 9,
                                                        is_hit: true,
                                                        is_crit: false,
                                                        effect_type: "Damage".to_string(),
                                                        source: None,
                                                        is_signature: false,
                                                    });
                                                }
                                            }
                                            "toxic_gas_vent" => {
                                                if !success {
                                                    combat_stats.hp = (combat_stats.hp - 12).max(0);
                                                    commands.entity(selected_token_ent).insert(crate::mission::entities::VisualHit {
                                                        amount: 12,
                                                        is_hit: true,
                                                        is_crit: false,
                                                        effect_type: "Damage".to_string(),
                                                        source: None,
                                                        is_signature: false,
                                                    });
                                                }
                                            }
                                            "abandoned_camp" => {
                                                if success {
                                                    combat_stats.hp = (combat_stats.hp + 10).min(combat_stats.max_hp);
                                                    if let Some(ref mut g) = gold_res {
                                                        g.0 += 40;
                                                    }
                                                    commands.entity(selected_token_ent).insert(crate::mission::entities::VisualHit {
                                                        amount: 10,
                                                        is_hit: false,
                                                        is_crit: false,
                                                        effect_type: "Heal".to_string(),
                                                        source: None,
                                                        is_signature: false,
                                                    });
                                                } else {
                                                    combat_stats.hp = (combat_stats.hp - 7).max(0);
                                                    commands.entity(selected_token_ent).insert(crate::mission::entities::VisualHit {
                                                        amount: 7,
                                                        is_hit: true,
                                                        is_crit: false,
                                                        effect_type: "Damage".to_string(),
                                                        source: None,
                                                        is_signature: false,
                                                    });
                                                }
                                            }
                                            _ => {}
                                        }
                                    }

                                    let outcome_text = if success {
                                        event_def.success_text.clone()
                                    } else {
                                        event_def.failure_text.clone()
                                    };

                                    log_writer.write(MissionLogEvent {
                                        mission_entity,
                                        payload: MissionLogPayload::EventTriggered {
                                            event_name: event_def.name.clone(),
                                            hero_name,
                                            description: event_def.description.clone(),
                                            outcome_text,
                                        },
                                    });

                                    events_state.events_fired += 1;
                                }
                            }
                        }
                    }
            }
        }

        // A room is cleared if no living enemies (from this mission) remain
        for (room_idx, _) in map.rooms.iter().enumerate() {
            let has_living_enemies = children.iter().any(|c| {
                if let Ok((er, ec)) = enemies.get(c) {
                    er.0 == Some(room_idx) && ec.hp > 0
                } else {
                    false
                }
            });
            if !has_living_enemies && room_status.visited.get(room_idx).copied().unwrap_or(false)
                && room_idx < room_status.cleared.len() && !room_status.cleared[room_idx] {
                    room_status.cleared[room_idx] = true;

                    // Reset signature cooldowns for all heroes in this mission
                    for child in children.iter() {
                        if let Ok(mut abils) = active_abilities.get_mut(child) {
                            for ab in &mut abils.abilities {
                                if let Some(ability_def) = ability_db.as_ref().and_then(|db| db.get(&ab.ability_id))
                                    && ability_def.is_signature {
                                        ab.remaining_cooldown = 0;
                                    }
                            }
                        }
                    }
                }
        }
    }
}

/// Check if each mission is complete (all rooms cleared) or failed (all heroes
/// dead). Awards gold/XP and fires toasts on completion, then despawns the
/// mission entity (children auto-despawn via `ChildOf` `linked_spawn`).
#[allow(clippy::too_many_arguments)]
pub fn check_mission_completion(
    mut commands: Commands,
    mut missions: Query<
        (
            Entity,
            &mut MissionProgress,
            &MissionInfo,
            &MissionParty,
            &RoomStatus,
            &Children,
            &crate::mission::MissionDungeon,
        ),
        With<Mission>,
    >,
    hero_tokens: Query<(&HeroToken, &CombatStats), Without<EnemyToken>>,
    enemy_tokens: Query<&EnemyToken>,
    mut hero_infos: Query<
        (
            &mut HeroInfo,
            &mut HeroStats,
            &crate::hero::HeroGrowth,
            &mut crate::hero::HeroStatProgress,
            &mut crate::hero::Fatigue,
            Option<&Epithet>,
            &mut crate::equipment::HeroEquipment,
        ),
        With<Hero>,
    >,
    mut gold: ResMut<Gold>,
    template_db: Res<MissionTemplateDatabase>,
    time: Res<Time<Virtual>>,
    mut materials: ResMut<crate::materials::Materials>,
    mut reputation: ResMut<crate::reputation::Reputation>,
    favorite_q: Query<(), With<Favorite>>,
    mut log_writer: MessageWriter<MissionLogEvent>,
    mut board: Option<ResMut<crate::screens::missions::MissionBoard>>,
    rescue_missions: Query<&crate::mission::RescueMission>,
    mut histories: Query<&mut crate::hero::history::HeroHistory>,
    missing_q: Query<&Missing>,
) {
    let mut rng = rand::rng();

    for (mission_entity, mut progress, info, party, room_status, children, dungeon) in &mut missions {
        if *progress != MissionProgress::InProgress {
            continue;
        }

        // Gather this mission's hero tokens
        let mission_heroes: Vec<(&HeroToken, &CombatStats)> = children
            .iter()
            .filter_map(|c| hero_tokens.get(c).ok())
            .collect();

        // Failure: all heroes dead
        let all_dead = !mission_heroes.is_empty() && mission_heroes.iter().all(|(_, c)| c.hp <= 0);
        if all_dead {
            *progress = MissionProgress::Failed;
            let now = time.elapsed_secs_f64();
            let mut expires_at = now + MISSING_DURATION_SECS;

            log_writer.write(MissionLogEvent {
                mission_entity,
                payload: MissionLogPayload::Failure,
            });

            // Favorite-aware toast title. `favorite_q` is data-less
            // (`With<Favorite>` only) to avoid a HeroInfo access conflict with
            // the mutable `hero_infos` query above; we look up names
            // separately via `hero_infos`. All favorited casualties are named
            // so the player isn't blindsided by a second favorite they didn't
            // see called out.
            let favorited_names: Vec<String> = party
                .0
                .iter()
                .filter(|e| favorite_q.get(**e).is_ok())
                .filter_map(|e| hero_infos.get(*e).ok().map(|(hi, _, _, _, _, ep, _)| format_hero_name(&hi.name, ep)))
                .collect();
            let title = match favorited_names.len() {
                0 => trf("combat.mission_failed_toast", &[("mission", &info.name)]),
                1 => trf("combat.one_missing_toast", &[("name", &favorited_names[0])]),
                2 => trf(
                    "combat.many_missing_toast",
                    &[("names", &format!("{} & {}", favorited_names[0], favorited_names[1]))],
                ),
                _ => {
                    // n>=3: Oxford-style "A, B & C"
                    let (last, rest) = favorited_names.split_last().unwrap();
                    trf(
                        "combat.many_missing_toast",
                        &[("names", &format!("{} & {}", rest.join(", "), last))],
                    )
                }
            };
            let mut rescue_heroes = party.0.clone();
            if let Ok(rm) = rescue_missions.get(mission_entity) {
                // Combine original missing list with new casualties
                for &h in &rm.rescue_heroes {
                    if !rescue_heroes.contains(&h) {
                        rescue_heroes.push(h);
                    }
                }
            }

            // Align countdowns to maximum expiry
            for &hero_entity in &rescue_heroes {
                if let Ok(missing) = missing_q.get(hero_entity) {
                    if missing.expires_at > expires_at {
                        expires_at = missing.expires_at;
                    }
                }
            }

            for &hero_entity in &rescue_heroes {
                let mut dropped = None;
                if let Ok((_, _, _, _, _, _, mut equip)) = hero_infos.get_mut(hero_entity) {
                    dropped = Some(equip.clone());
                    *equip = crate::equipment::HeroEquipment::default();
                }

                commands
                    .entity(hero_entity)
                    .remove::<super::OnMission>()
                    .insert(Missing {
                        expires_at,
                        dropped_equipment: dropped,
                    });

                if let Ok(mut hist) = histories.get_mut(hero_entity) {
                    hist.add_timeline_entry(trf("timeline.went_missing_short", &[("mission", &info.name)]));
                }
            }

            let mut rescue_offer_idx = None;
            if let Some(b) = board.as_mut() {
                b.offers.clear();
                // Push immediately run rescue offer
                if let Some(t_idx) = template_db.0.iter().position(|t| t.id == info.template_id) {
                    b.rescue_offers.push(crate::screens::missions::RescueOffer {
                        template_idx: t_idx,
                        modifiers: info.modifiers.clone(),
                        map: dungeon.0.clone(),
                        rescue_heroes: rescue_heroes.clone(),
                        expires_at,
                    });
                    rescue_offer_idx = Some(b.rescue_offers.len() - 1);
                }
            }

            commands.trigger(ToastEvent {
                title,
                body: tr("combat.party_wiped_body").to_string(),
                kind: ToastKind::Failure,
                action: rescue_offer_idx.map(|idx| crate::ui::toast::ToastAction::MountRescue { rescue_offer_idx: idx }),
            });

            commands.entity(mission_entity).despawn();
            info!("Mission '{}' failed — heroes missing, rescue generated", info.name);
            continue;
        }

        // Completion: all rooms cleared
        let all_cleared = !room_status.cleared.is_empty() && room_status.cleared.iter().all(|&c| c);
        if !all_cleared {
            continue;
        }

        *progress = MissionProgress::Complete;

        // Look up template for rewards
        let template = template_db.0.iter().find(|t| t.id == info.template_id);

        // XP from enemies in this mission (defeated or otherwise; completion
        // implies all were killed)
        let enemy_xp: u32 = children
            .iter()
            .filter_map(|c| enemy_tokens.get(c).ok())
            .map(|e| e.xp_reward)
            .sum();
        let xp_bonus = template.map_or(0, |t| t.xp_bonus);
        let total_xp = enemy_xp + xp_bonus;

        let is_bountiful = info.modifiers.contains(&super::data::MissionModifier::Bountiful);

        let mut gold_earned = template.map_or(0, |t| {
            rng.random_range(t.gold_reward.min..=t.gold_reward.max)
        });
        if is_bountiful {
            gold_earned = (gold_earned as f32 * 1.5).floor() as u32;
        }
        gold.0 += gold_earned;

        log_writer.write(MissionLogEvent {
            mission_entity,
            payload: MissionLogPayload::Loot {
                gold: gold_earned,
                xp: total_xp,
            },
        });

        // Award materials
        if let Some(template) = &template {
            for &(mat_type, min, max) in &template.material_drops {
                let mut amount = rng.random_range(min..=max);
                if is_bountiful {
                    amount += 1;
                }
                materials.add(mat_type, amount);
            }
            // Award reputation
            reputation.0 += template.reputation_reward;
        }

        // Count survivors and award XP
        let survivors: Vec<Entity> = mission_heroes
            .iter()
            .filter(|(_, cs)| cs.hp > 0)
            .map(|(ht, _)| ht.0)
            .collect();
        let casualties = party.0.len().saturating_sub(survivors.len());

        let mut level_ups = 0u32;
        for hero_entity in &survivors {
            if let Ok((mut hinfo, mut hstats, hgrowth, mut hprog, mut hfatigue, _, _)) = hero_infos.get_mut(*hero_entity) {
                level_ups += crate::hero::award_xp(
                    &mut hinfo,
                    &mut hstats,
                    hgrowth,
                    &mut hprog,
                    Some(&mut hfatigue),
                    total_xp,
                );
            }
        }

        let mut body = trf("combat.rewards_body", &[("gold", &gold_earned.to_string()), ("xp", &total_xp.to_string())]);
        if casualties > 0 {
            body.push_str(&if casualties == 1 {
                tr("combat.casualty_one").to_string()
            } else {
                trf("combat.casualty_many", &[("count", &casualties.to_string())])
            });
        }
        if level_ups > 0 {
            body.push_str(&if level_ups == 1 {
                tr("combat.level_up_one").to_string()
            } else {
                trf("combat.level_up_many", &[("count", &level_ups.to_string())])
            });
        }
        if let Some(template) = &template
            && template.reputation_reward > 0 {
                body.push_str(&trf("combat.rep_reward", &[("rep", &template.reputation_reward.to_string())]));
            }

        commands.trigger(ToastEvent {
            title: trf("combat.mission_complete_toast", &[("mission", &info.name)]),
            body,
            kind: ToastKind::Success,
            action: None,
        });

        // Resolve rescue mission rewards and logs
        let is_rescue = rescue_missions.get(mission_entity).ok();
        if let Some(rescue) = is_rescue {
            for &rescued_entity in &rescue.rescue_heroes {
                let mut restored_gear = false;
                if rescue.gear_recovered {
                    if let Ok(missing) = missing_q.get(rescued_entity) {
                        if let Some(ref dropped) = missing.dropped_equipment {
                            if let Ok((_, _, _, _, _, _, mut equip)) = hero_infos.get_mut(rescued_entity) {
                                *equip = dropped.clone();
                                restored_gear = true;
                            }
                        }
                    }
                }

                commands.entity(rescued_entity).remove::<crate::hero::status::Missing>();
                if let Ok(mut hist) = histories.get_mut(rescued_entity) {
                    hist.rescues_received += 1;
                    let timeline_msg = if restored_gear {
                        format!("Rescued from {} (gear recovered)", info.name)
                    } else {
                        format!("Rescued from {} (gear lost)", info.name)
                    };
                    hist.add_timeline_entry(timeline_msg);
                }
            }
            for &rescuer_entity in &survivors {
                if let Ok(mut hist) = histories.get_mut(rescuer_entity) {
                    hist.rescues_given += 1;
                    hist.add_timeline_entry(format!("Rescued colleagues in {}", info.name));
                }
            }
        }

        for &hero_entity in &party.0 {
            commands.entity(hero_entity).remove::<super::OnMission>();
        }
        if let Some(b) = board.as_mut() {
            b.offers.clear();
        }
        commands.entity(mission_entity).despawn();

        info!(
            "Mission '{}' complete — +{gold_earned}g, +{total_xp}xp",
            info.name
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;
    use crate::mission::dungeon::DungeonMap;
    use crate::mission::data::MissionTemplate;
    use crate::hero::history::HeroHistory;
    use crate::reputation::Reputation;
    use crate::hero::status_tick::tick_missing;

    fn setup_test_board() -> (App, Entity) {
        let mut app = App::new();
        app.init_resource::<Gold>();
        app.init_resource::<Reputation>();
        app.init_resource::<crate::screens::missions::MissionBoard>();
        app.init_resource::<Time<Virtual>>();
        app.init_resource::<crate::materials::Materials>();

        // Register event writer
        app.world_mut().init_resource::<bevy::ecs::message::Messages<MissionLogEvent>>();

        // Set up MissionTemplateDatabase
        let template = MissionTemplate {
            id: "test_temp".to_string(),
            name: "Test Dungeon".to_string(),
            description: "A test dungeon".to_string(),
            reputation_required: 0,
            reputation_reward: 10,
            difficulty: 1,
            gold_reward: crate::mission::data::GoldReward { min: 100, max: 200 },
            xp_bonus: 50,
            rooms_min: 2,
            rooms_max: 3,
            allowed_modifiers: vec![],
            material_drops: vec![],
            biome: crate::mission::data::BiomeType::Dungeon,
            enemy_types: vec![],
        };
        app.insert_resource(MissionTemplateDatabase(vec![template]));

        // Spawn a hero
        let hero = app.world_mut().spawn((
            Hero,
            HeroInfo {
                name: "Alice".to_string(),
                class: crate::hero::data::HeroClass::Warrior,
                level: 1,
                xp: 0,
                xp_to_next: 100,
            },
            HeroStats {
                strength: 10,
                dexterity: 10,
                constitution: 10,
                intelligence: 10,
                wisdom: 10,
                charisma: 10,
            },
            crate::hero::HeroGrowth {
                strength: 1.0,
                dexterity: 1.0,
                constitution: 1.0,
                intelligence: 1.0,
                wisdom: 1.0,
                charisma: 1.0,
            },
            crate::hero::HeroStatProgress::default(),
            crate::hero::Fatigue {
                current: 0.0,
                max_base: 100.0,
            },
            HeroHistory::default(),
            crate::equipment::HeroEquipment::default(),
        )).id();

        (app, hero)
    }

    #[test]
    fn test_party_wipe_triggers_rescue_offer() {
        let (mut app, hero) = setup_test_board();

        // Spawn mission
        let map = DungeonMap {
            width: 40,
            height: 30,
            rooms: vec![],
            tiles: vec![],
        };
        let mission_ent = app.world_mut().spawn((
            Mission,
            MissionInfo {
                template_id: "test_temp".to_string(),
                name: "Test Dungeon".to_string(),
                difficulty: 1,
                modifiers: vec![],
                biome: crate::mission::data::BiomeType::Dungeon,
            },
            MissionProgress::InProgress,
            MissionParty(vec![hero]),
            crate::mission::MissionDungeon(map),
            RoomStatus {
                visited: vec![true],
                cleared: vec![false],
            },
        )).id();

        // Spawn a dead hero token as a child
        let token = app.world_mut().spawn((
            HeroToken(hero),
            CombatStats {
                hp: 0,
                max_hp: 10,
                attack: 5,
                defense: 5,
                speed: 10,
            },
            ChildOf(mission_ent),
        )).id();

        app.world_mut().entity_mut(mission_ent).add_child(token);

        let _ = app.world_mut().run_system_once(check_mission_completion);

        // Verify rescue offer generated on board
        let board = app.world().resource::<crate::screens::missions::MissionBoard>();
        assert_eq!(board.rescue_offers.len(), 1);
        let offer = &board.rescue_offers[0];
        assert_eq!(offer.rescue_heroes, vec![hero]);

        // Verify hero marked as missing
        let missing = app.world().get::<Missing>(hero);
        assert!(missing.is_some());

        // Verify chronicle entry
        let hist = app.world().get::<HeroHistory>(hero).unwrap();
        assert!(hist.timeline.iter().any(|entry| entry.contains("Went missing in Test Dungeon")));
    }

    #[test]
    fn test_rescue_mission_success() {
        let (mut app, rescued_hero) = setup_test_board();

        // Mark Alice as Missing
        app.world_mut().entity_mut(rescued_hero).insert(Missing { expires_at: 100.0, dropped_equipment: None });

        // Spawn rescuer
        let rescuer = app.world_mut().spawn((
            Hero,
            HeroInfo {
                name: "Bob".to_string(),
                class: crate::hero::data::HeroClass::Warrior,
                level: 1,
                xp: 0,
                xp_to_next: 100,
            },
            HeroStats {
                strength: 10,
                dexterity: 10,
                constitution: 10,
                intelligence: 10,
                wisdom: 10,
                charisma: 10,
            },
            crate::hero::HeroGrowth {
                strength: 1.0,
                dexterity: 1.0,
                constitution: 1.0,
                intelligence: 1.0,
                wisdom: 1.0,
                charisma: 1.0,
            },
            crate::hero::HeroStatProgress::default(),
            crate::hero::Fatigue {
                current: 0.0,
                max_base: 100.0,
            },
            HeroHistory::default(),
            crate::equipment::HeroEquipment::default(),
        )).id();

        // Spawn rescue mission
        let map = DungeonMap {
            width: 40,
            height: 30,
            rooms: vec![],
            tiles: vec![],
        };
        let mission_ent = app.world_mut().spawn((
            Mission,
            MissionInfo {
                template_id: "test_temp".to_string(),
                name: "Test Dungeon".to_string(),
                difficulty: 1,
                modifiers: vec![],
                biome: crate::mission::data::BiomeType::Dungeon,
            },
            MissionProgress::InProgress,
            MissionParty(vec![rescuer]),
            crate::mission::MissionDungeon(map),
            RoomStatus {
                visited: vec![true],
                cleared: vec![true], // all rooms cleared -> Complete
            },
            crate::mission::RescueMission {
                rescue_heroes: vec![rescued_hero],
                gear_recovered: false,
            },
        )).id();

        // Spawn surviving rescuer token
        let token = app.world_mut().spawn((
            HeroToken(rescuer),
            CombatStats {
                hp: 10,
                max_hp: 10,
                attack: 5,
                defense: 5,
                speed: 10,
            },
            ChildOf(mission_ent),
        )).id();
        app.world_mut().entity_mut(mission_ent).add_child(token);

        let _ = app.world_mut().run_system_once(check_mission_completion);

        // Verify rescued hero is no longer missing
        assert!(app.world().get::<Missing>(rescued_hero).is_none());

        // Verify histories updated
        let rescued_hist = app.world().get::<HeroHistory>(rescued_hero).unwrap();
        assert_eq!(rescued_hist.rescues_received, 1);
        assert!(rescued_hist.timeline.iter().any(|entry| entry.contains("Rescued from")));

        let rescuer_hist = app.world().get::<HeroHistory>(rescuer).unwrap();
        assert_eq!(rescuer_hist.rescues_given, 1);
        assert!(rescuer_hist.timeline.iter().any(|entry| entry.contains("Rescued colleagues in")));
    }

    #[test]
    fn test_rescue_mission_failure_combines_heroes() {
        let (mut app, rescued_hero) = setup_test_board();

        // Mark Alice as Missing
        app.world_mut().entity_mut(rescued_hero).insert(Missing { expires_at: 100.0, dropped_equipment: None });

        // Spawn rescuer
        let rescuer = app.world_mut().spawn((
            Hero,
            HeroInfo {
                name: "Bob".to_string(),
                class: crate::hero::data::HeroClass::Warrior,
                level: 1,
                xp: 0,
                xp_to_next: 100,
            },
            HeroStats {
                strength: 10,
                dexterity: 10,
                constitution: 10,
                intelligence: 10,
                wisdom: 10,
                charisma: 10,
            },
            crate::hero::HeroGrowth {
                strength: 1.0,
                dexterity: 1.0,
                constitution: 1.0,
                intelligence: 1.0,
                wisdom: 1.0,
                charisma: 1.0,
            },
            crate::hero::HeroStatProgress::default(),
            crate::hero::Fatigue {
                current: 0.0,
                max_base: 100.0,
            },
            HeroHistory::default(),
            crate::equipment::HeroEquipment::default(),
        )).id();

        // Spawn rescue mission that is about to fail
        let map = DungeonMap {
            width: 40,
            height: 30,
            rooms: vec![],
            tiles: vec![],
        };
        let mission_ent = app.world_mut().spawn((
            Mission,
            MissionInfo {
                template_id: "test_temp".to_string(),
                name: "Test Dungeon".to_string(),
                difficulty: 1,
                modifiers: vec![],
                biome: crate::mission::data::BiomeType::Dungeon,
            },
            MissionProgress::InProgress,
            MissionParty(vec![rescuer]),
            crate::mission::MissionDungeon(map),
            RoomStatus {
                visited: vec![true],
                cleared: vec![false],
            },
            crate::mission::RescueMission {
                rescue_heroes: vec![rescued_hero],
                gear_recovered: false,
            },
        )).id();

        // Spawn dead rescuer token
        let token = app.world_mut().spawn((
            HeroToken(rescuer),
            CombatStats {
                hp: 0,
                max_hp: 10,
                attack: 5,
                defense: 5,
                speed: 10,
            },
            ChildOf(mission_ent),
        )).id();
        app.world_mut().entity_mut(mission_ent).add_child(token);

        let _ = app.world_mut().run_system_once(check_mission_completion);

        // Verify new rescue offer combined both Bob (rescuer) and Alice (originally rescued)
        let board = app.world().resource::<crate::screens::missions::MissionBoard>();
        assert_eq!(board.rescue_offers.len(), 1);
        let offer = &board.rescue_offers[0];
        assert!(offer.rescue_heroes.contains(&rescued_hero));
        assert!(offer.rescue_heroes.contains(&rescuer));
        assert_eq!(offer.rescue_heroes.len(), 2);
    }

    #[test]
    fn test_gear_loss_and_recovery_loop() {
        let (mut app, hero) = setup_test_board();

        // Equip Alice with tier 3 rare gear
        let initial_equip = crate::equipment::HeroEquipment {
            weapon_tier: 3,
            weapon_rarity: crate::equipment::GearRarity::Rare,
            weapon_affix: Some(crate::equipment::BehavioralAffix::Lifesteal),
            armor_tier: 2,
            armor_rarity: crate::equipment::GearRarity::Common,
            armor_affix: None,
            accessory_tier: 1,
            accessory_rarity: crate::equipment::GearRarity::Common,
            accessory_affix: None,
        };
        app.world_mut().entity_mut(hero).insert(initial_equip.clone());

        // Spawn a mission that Alice fails
        let map = DungeonMap {
            width: 40,
            height: 30,
            rooms: vec![],
            tiles: vec![],
        };
        let mission_ent = app.world_mut().spawn((
            Mission,
            MissionInfo {
                template_id: "test_temp".to_string(),
                name: "Test Dungeon".to_string(),
                difficulty: 1,
                modifiers: vec![],
                biome: crate::mission::data::BiomeType::Dungeon,
            },
            MissionProgress::InProgress,
            MissionParty(vec![hero]),
            crate::mission::MissionDungeon(map.clone()),
            RoomStatus {
                visited: vec![true],
                cleared: vec![false],
            },
        )).id();

        // Spawn a dead hero token as child
        let token = app.world_mut().spawn((
            HeroToken(hero),
            CombatStats { hp: 0, max_hp: 10, attack: 5, defense: 5, speed: 10 },
            ChildOf(mission_ent),
        )).id();
        app.world_mut().entity_mut(mission_ent).add_child(token);

        let _ = app.world_mut().run_system_once(check_mission_completion);

        // Verify Alice is missing, and her equipment on the roster has been reset/stripped
        let missing = app.world().get::<Missing>(hero).unwrap();
        assert!(missing.dropped_equipment.is_some());
        let saved_gear = missing.dropped_equipment.as_ref().unwrap();
        assert_eq!(saved_gear.weapon_tier, 3);

        let current_equip = app.world().get::<crate::equipment::HeroEquipment>(hero).unwrap();
        assert_eq!(current_equip.weapon_tier, 0); // stripped!

        // Now spawn Bob to do the rescue
        let rescuer = app.world_mut().spawn((
            Hero,
            HeroInfo {
                name: "Bob".to_string(),
                class: crate::hero::data::HeroClass::Warrior,
                level: 1,
                xp: 0,
                xp_to_next: 100,
            },
            HeroStats { strength: 10, dexterity: 10, constitution: 10, intelligence: 10, wisdom: 10, charisma: 10 },
            crate::hero::HeroGrowth { strength: 1.0, dexterity: 1.0, constitution: 1.0, intelligence: 1.0, wisdom: 1.0, charisma: 1.0 },
            crate::hero::HeroStatProgress::default(),
            crate::hero::Fatigue { current: 0.0, max_base: 100.0 },
            HeroHistory::default(),
            crate::equipment::HeroEquipment::default(),
        )).id();

        // Dispatch rescue mission
        let rescue_mission_ent = app.world_mut().spawn((
            Mission,
            MissionInfo {
                template_id: "test_temp".to_string(),
                name: "Rescue Dungeon".to_string(),
                difficulty: 1,
                modifiers: vec![],
                biome: crate::mission::data::BiomeType::Dungeon,
            },
            MissionProgress::InProgress,
            MissionParty(vec![rescuer]),
            crate::mission::MissionDungeon(map),
            RoomStatus {
                visited: vec![true, false],
                cleared: vec![true, true], // all cleared
            },
            crate::mission::RescueMission {
                rescue_heroes: vec![hero],
                gear_recovered: true, // simulated that they recovered the gear
            },
        )).id();

        // Spawn rescuer token
        let r_token = app.world_mut().spawn((
            HeroToken(rescuer),
            CombatStats { hp: 10, max_hp: 10, attack: 5, defense: 5, speed: 10 },
            ChildOf(rescue_mission_ent),
        )).id();
        app.world_mut().entity_mut(rescue_mission_ent).add_child(r_token);

        let _ = app.world_mut().run_system_once(check_mission_completion);

        // Verify Alice has her tier 3 gear restored
        assert!(app.world().get::<Missing>(hero).is_none());
        let restored_equip = app.world().get::<crate::equipment::HeroEquipment>(hero).unwrap();
        assert_eq!(restored_equip.weapon_tier, 3);
        assert_eq!(restored_equip.weapon_rarity, crate::equipment::GearRarity::Rare);

        let hist = app.world().get::<HeroHistory>(hero).unwrap();
        assert!(hist.timeline.iter().any(|entry| entry.contains("gear recovered")));
    }

    #[test]
    fn test_missing_expiration_discards_dropped_gear() {
        let (mut app, hero) = setup_test_board();

        // Set Alice as missing with dropped gear
        let dropped_gear = crate::equipment::HeroEquipment {
            weapon_tier: 5,
            weapon_rarity: crate::equipment::GearRarity::Rare,
            weapon_affix: None,
            armor_tier: 0,
            armor_rarity: crate::equipment::GearRarity::Common,
            armor_affix: None,
            accessory_tier: 0,
            accessory_rarity: crate::equipment::GearRarity::Common,
            accessory_affix: None,
        };
        app.world_mut().entity_mut(hero).insert(Missing {
            expires_at: 10.0,
            dropped_equipment: Some(dropped_gear),
        });
        
        // Strip her current equipment
        app.world_mut().entity_mut(hero).insert(crate::equipment::HeroEquipment::default());

        // Update elapsed time to 15.0 (so timer is expired)
        let mut t = Time::<Virtual>::default();
        t.advance_by(std::time::Duration::from_secs(15));
        app.insert_resource(t);

        // Run tick_missing system
        let _ = app.world_mut().run_system_once(tick_missing);

        // Verify Alice is now Injured, not missing, and her equipment is still tier 0 (lost!)
        assert!(app.world().get::<Missing>(hero).is_none());
        assert!(app.world().get::<crate::hero::status::Injured>(hero).is_some());
        let current_equip = app.world().get::<crate::equipment::HeroEquipment>(hero).unwrap();
        assert_eq!(current_equip.weapon_tier, 0); // lost forever!

        // Verify chronicle entry
        let hist = app.world().get::<HeroHistory>(hero).unwrap();
        assert!(hist.timeline.iter().any(|entry| entry.contains("Returned after being missing (gear lost)")));
    }

    #[test]
    fn test_rescue_event_sequencing() {
        let mut app = App::new();
        app.init_resource::<Gold>();
        app.init_resource::<Reputation>();
        app.init_resource::<crate::screens::missions::MissionBoard>();
        app.init_resource::<Time<Virtual>>();
        app.init_resource::<crate::materials::Materials>();
        app.world_mut().init_resource::<bevy::ecs::message::Messages<MissionLogEvent>>();

        app.insert_resource(crate::ui::feed::NarrativeTemplates {
            attack_hit: vec![],
            attack_crit: vec![],
            attack_miss: vec![],
            heal: vec![],
            death_hero: vec![],
            death_enemy: vec![],
            room_entry: vec![],
            mission_complete: vec![],
            mission_failed: vec![],
        });

        // Setup EventDatabase with rescue events
        let events = vec![
            crate::mission::data::MissionEventDef {
                id: "rescue_trail".to_string(),
                name: "Lost Party's Trail".to_string(),
                description: "trail".to_string(),
                event_type: crate::mission::data::EventType::Ambush,
                check_stat: crate::mission::data::EventCheckStat::Wisdom,
                check_difficulty: 10,
                success_text: "ok".to_string(),
                failure_text: "fail".to_string(),
                priority_trait: None,
            },
            crate::mission::data::MissionEventDef {
                id: "rescue_dropped_gear".to_string(),
                name: "Dropped Gear Cache".to_string(),
                description: "gear".to_string(),
                event_type: crate::mission::data::EventType::HiddenChamber,
                check_stat: crate::mission::data::EventCheckStat::Dexterity,
                check_difficulty: 12,
                success_text: "ok".to_string(),
                failure_text: "fail".to_string(),
                priority_trait: None,
            },
            crate::mission::data::MissionEventDef {
                id: "rescue_campsite".to_string(),
                name: "Lost Party's Campsite".to_string(),
                description: "camp".to_string(),
                event_type: crate::mission::data::EventType::Shrine,
                check_stat: crate::mission::data::EventCheckStat::Constitution,
                check_difficulty: 11,
                success_text: "ok".to_string(),
                failure_text: "fail".to_string(),
                priority_trait: None,
            },
        ];
        app.insert_resource(EventDatabase(events));

        // Spawn a hero
        let hero = app.world_mut().spawn((
            Hero,
            HeroInfo {
                name: "Alice".to_string(),
                class: crate::hero::data::HeroClass::Warrior,
                level: 1,
                xp: 0,
                xp_to_next: 100,
            },
            HeroStats { strength: 10, dexterity: 10, constitution: 10, intelligence: 10, wisdom: 10, charisma: 10 },
            crate::hero::HeroTraits(vec![]),
            crate::equipment::HeroEquipment::default(),
        )).id();

        // Spawn a rescue mission
        let map = DungeonMap {
            width: 40,
            height: 30,
            rooms: vec![
                crate::mission::dungeon::Room {
                    x: 0, y: 0, w: 5, h: 5,
                    room_type: crate::mission::dungeon::RoomType::Entrance,
                },
                crate::mission::dungeon::Room {
                    x: 10, y: 10, w: 5, h: 5,
                    room_type: crate::mission::dungeon::RoomType::Normal,
                },
                crate::mission::dungeon::Room {
                    x: 20, y: 20, w: 5, h: 5,
                    room_type: crate::mission::dungeon::RoomType::Normal,
                },
            ],
            tiles: vec![],
        };

        let mission_ent = app.world_mut().spawn((
            Mission,
            MissionInfo {
                template_id: "test_temp".to_string(),
                name: "Rescue Dungeon".to_string(),
                difficulty: 1,
                modifiers: vec![],
                biome: crate::mission::data::BiomeType::Dungeon,
            },
            MissionProgress::InProgress,
            MissionParty(vec![hero]),
            crate::mission::MissionDungeon(map),
            RoomStatus {
                visited: vec![true, false, false],
                cleared: vec![false, false, false],
            },
            crate::mission::RescueMission {
                rescue_heroes: vec![],
                gear_recovered: false,
            },
            crate::mission::entities::MissionEventsState {
                events_fired: 0,
                max_events: 3,
            },
            crate::ui::feed::MissionLogHistory { logs: vec![] },
        )).id();

        // Spawn hero token inside entrance (room 0)
        let token = app.world_mut().spawn((
            HeroToken(hero),
            CombatStats { hp: 10, max_hp: 10, attack: 5, defense: 5, speed: 10 },
            InRoom(Some(0)),
            ChildOf(mission_ent),
        )).id();
        app.world_mut().entity_mut(mission_ent).add_child(token);

        // Move token to room 1 -> first event should fire (rescue_trail)
        app.world_mut().entity_mut(token).insert(InRoom(Some(1)));
        let _ = app.world_mut().run_system_once(update_room_status);
        let _ = app.world_mut().run_system_once(crate::ui::feed::process_log_events);

        let history = app.world().get::<crate::ui::feed::MissionLogHistory>(mission_ent).unwrap();
        assert!(history.logs.iter().any(|log| log.text.contains("Lost Party's Trail")));

        // Move token to room 2 -> second event should fire (rescue_dropped_gear)
        app.world_mut().entity_mut(token).insert(InRoom(Some(2)));
        let _ = app.world_mut().run_system_once(update_room_status);
        let _ = app.world_mut().run_system_once(crate::ui::feed::process_log_events);

        let history = app.world().get::<crate::ui::feed::MissionLogHistory>(mission_ent).unwrap();
        assert!(history.logs.iter().any(|log| log.text.contains("Dropped Gear Cache")));

        // Verify gear recovered was set to true on the rescue mission component
        let rm = app.world().get::<crate::mission::RescueMission>(mission_ent).unwrap();
        assert!(rm.gear_recovered);
    }
}
