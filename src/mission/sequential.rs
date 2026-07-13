use bevy::prelude::*;
use bevy::ecs::message::MessageWriter;
use rand::Rng;
use rand::rngs::StdRng;
use rand::SeedableRng;
use rand::seq::SliceRandom;

use crate::hero::{Hero, HeroInfo, HeroStats, HeroTraits, data::HeroTrait};
use crate::ui::feed::{MissionLogEvent, MissionLogPayload};
// use crate::equipment::{HeroEquipment, GearRarity, BehavioralAffix};
use super::Mission;
use super::entities::{GridPosition, InRoom, CombatStats, MoveRange, HeroToken, EnemyToken, RoomStatus, MissionTurnQueue, VisualPathHistory, ActiveAbilities, LootChest, VisualHit, TelegraphedAttack, Enraged, EnemyAI};
use super::ai::{decide_action, HeroAction};
use super::pathfinding::find_path;

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

pub fn calculate_hero_token_stats(
    info: &HeroInfo,
    stats: &HeroStats,
    equipment: Option<&crate::equipment::HeroEquipment>,
    fatigue: Option<&crate::hero::Fatigue>,
    is_injured: bool,
    equipment_db: &crate::equipment::EquipmentDatabase,
) -> (i32, i32, i32) {
    let mul = |v: i32| -> i32 {
        let mut val = v as f32;
        if is_injured {
            val *= crate::hero::status::INJURED_STAT_MULTIPLIER;
        }
        let current_fatigue = fatigue.map(|f| f.current).unwrap_or(100.0);
        if current_fatigue <= 0.0 {
            val *= 0.5;
        }
        val.floor() as i32
    };
    let str_eff = mul(stats.strength);
    let dex_eff = mul(stats.dexterity);
    let con_eff = mul(stats.constitution);

    let mut hp = con_eff * 3 + info.level as i32 * 5;
    let mut attack = (str_eff + dex_eff) / 2;
    let mut defense = (con_eff + dex_eff) / 2;

    if let Some(eq) = equipment {
        let gear_stats = eq.total_stats(equipment_db, info.class);
        attack += gear_stats.attack;
        defense += gear_stats.defense;
        hp += gear_stats.hp;
    }

    (hp, attack, defense)
}

/// Build or update the sequential turn queue for the mission.
/// If all characters have acted (or the queue is empty), starts a new round.
pub fn build_or_update_turn_queue(
    mut commands: Commands,
    missions_query: Query<(Entity, &Children), With<Mission>>,
    mut turn_queue_query: Query<&mut MissionTurnQueue>,
    combat_stats_query: Query<&CombatStats>,
    enemy_token_query: Query<&EnemyToken>,
    hero_token_query: Query<&HeroToken>,
    hero_equip_query: Query<&crate::equipment::HeroEquipment, With<Hero>>,
) {
    for (mission_entity, children) in &missions_query {
        let Ok(mut turn_queue) = turn_queue_query.get_mut(mission_entity) else {
            // Dispatch spawns missions without a queue (only the save-load
            // path added one) — self-heal so fresh missions tick.
            commands
                .entity(mission_entity)
                .insert(MissionTurnQueue::default());
            continue;
        };

        let needs_rebuild = turn_queue.queue.is_empty() || turn_queue.active_index >= turn_queue.queue.len();

        if needs_rebuild {
            // Create a deterministic RNG seeded with the mission entity ID and the round count
            let mut seed = [0u8; 32];
            let mission_id_bytes = mission_entity.to_bits().to_le_bytes();
            let round_bytes = turn_queue.round_count.to_le_bytes();
            seed[0..8].copy_from_slice(&mission_id_bytes);
            seed[8..16].copy_from_slice(&round_bytes);
            let mut round_rng = StdRng::from_seed(seed);

            // Collect all living children (tokens) that have CombatStats and roll initiative (d20 + DEX)
            let mut candidates: Vec<(Entity, i32)> = Vec::new();
            for &child in children {
                if let Ok(combat) = combat_stats_query.get(child)
                    && combat.hp > 0 {
                        let d20 = round_rng.random_range(1..=20);
                        let mut initiative = d20 + combat.speed;
                        if let Ok(ht) = hero_token_query.get(child) {
                            if let Ok(equipment) = hero_equip_query.get(ht.0) {
                                if equipment.has_affix(crate::equipment::BehavioralAffix::Initiative) {
                                    initiative += 10;
                                }
                            }
                        }
                        candidates.push((child, initiative));
                    }
            }

            if candidates.is_empty() {
                turn_queue.queue.clear();
                turn_queue.active_index = 0;
                turn_queue.combat_round_count = 0;
                continue;
            }

            // Shuffle the candidates to randomize initiative ties
            candidates.shuffle(&mut round_rng);

            // Stable sort descending by initiative (maintaining randomized tie order)
            candidates.sort_by_key(|&(_, initiative)| std::cmp::Reverse(initiative));

            let has_enemies = candidates.iter().any(|&(ent, _)| enemy_token_query.get(ent).is_ok());
            if has_enemies {
                turn_queue.combat_round_count += 1;
            } else {
                turn_queue.combat_round_count = 0;
            }

            // Populate the new turn queue
            turn_queue.queue = candidates.into_iter().map(|(ent, _)| ent).collect();
            turn_queue.active_index = 0;
            turn_queue.round_count += 1;
        }
    }
}

/// Resolve a single character's turn per simulation tick.
pub fn process_sequential_turn(
    mut commands: Commands,
    mut missions_query: Query<(Entity, &super::MissionDungeon, &RoomStatus, &Children, &mut MissionTurnQueue, Option<&super::MissionInfo>), With<super::Mission>>,
    mut token_set: ParamSet<(
        // 0: CombatStats mutable access
        Query<&mut CombatStats>,
        // 1: Read-only access to all tokens to get active entity's details
        Query<(
            Entity,
            Option<&EnemyToken>,
            &GridPosition,
            &InRoom,
            Option<&MoveRange>,
            Option<&HeroToken>,
            Option<&Name>,
            Option<&ActiveAbilities>,
        )>,
        // 2: Read-only enemy details for AI
        Query<(Entity, &GridPosition, &InRoom, &CombatStats, Option<&Name>), With<EnemyToken>>,
        // 3: Read-only hero details for AI/enemies
        Query<(Entity, &CombatStats, &InRoom, &GridPosition, Option<&Name>), (With<HeroToken>, Without<EnemyToken>)>,
        // 4: Mutable access to GridPosition and InRoom for movement updates
        Query<(&mut GridPosition, &mut InRoom)>,
        // 5: Mutable access to ActiveAbilities for cooldown tracking
        Query<&mut ActiveAbilities>,
        // 6: Mutable access to LootChests
        Query<(Entity, &mut LootChest, &GridPosition, &InRoom)>,
    )>,
    mut gold_res: Option<ResMut<crate::economy::Gold>>,
    hero_data_query: Query<(
        &HeroInfo,
        &HeroStats,
        &HeroTraits,
        Option<&crate::hero::Fatigue>,
        Option<&crate::hero::status::Injured>,
    ), With<Hero>>,
    ability_db: Option<Res<crate::hero::data::AbilityDatabase>>,
    equipment_db: Option<Res<crate::equipment::EquipmentDatabase>>,
    mut log_writer: MessageWriter<MissionLogEvent>,
    mut telegraphed_attacks: Query<&mut TelegraphedAttack>,
    enraged_query: Query<&Enraged>,
    enemy_ai_query: Query<&EnemyAI>,
    mut hero_equip_query: Query<&mut crate::equipment::HeroEquipment, With<Hero>>,
) {
    let mut rng = rand::rng();

    for (_mission_entity, dungeon, room_status, children, mut turn_queue, mission_info) in &mut missions_query {
        // TODO: Future Animation Lock Integration
        // To support showing detailed sprite attacking animations in the active view:
        // 1. Add `Option<&AnimationLock>` to `missions_query`.
        // 2. If the mission has `AnimationLock`, skip processing (continue) to pause simulation ticks.
        // 3. When an attack/heal action occurs on the observed mission, trigger a combat animation event
        //    and insert the `AnimationLock` component.
        // 4. The render view will remove `AnimationLock` once the sprite tween sequence completes.
        let map = &dungeon.0;
        let modifiers = match mission_info {
            Some(info) => &info.modifiers[..],
            None => &[],
        };
        let unsafe_rooms: Vec<usize> = children
            .iter()
            .filter_map(|c| telegraphed_attacks.get(c).ok().map(|t| t.target_room))
            .collect();

        // Collect context lists for the AI
        let mission_hero_entities: Vec<Entity> = children
            .iter()
            .filter(|&c| token_set.p3().get(c).is_ok())
            .collect();
        let mission_enemy_entities: Vec<Entity> = children
            .iter()
            .filter(|&c| token_set.p2().get(c).is_ok())
            .collect();

        let enemies_list: Vec<(Entity, GridPosition, InRoom, CombatStats)> = mission_enemy_entities
            .iter()
            .filter_map(|&e| {
                let binding = token_set.p2();
                let (ent, gp, ir, cs, _) = binding.get(e).ok()?;
                Some((ent, *gp, *ir, cs.clone()))
            })
            .collect();

        let allies_list: Vec<(Entity, CombatStats, InRoom, GridPosition)> = mission_hero_entities
            .iter()
            .filter_map(|&e| {
                let binding = token_set.p3();
                let (ent, cs, ir, gp, _) = binding.get(e).ok()?;
                Some((ent, cs.clone(), *ir, *gp))
            })
            .collect();

        let mission_chests: Vec<(Entity, GridPosition, InRoom, bool)> = children
            .iter()
            .filter_map(|c| {
                let binding = token_set.p6();
                let (ent, chest, gp, ir) = binding.get(c).ok()?;
                Some((ent, *gp, *ir, chest.opened))
            })
            .collect();

        loop {
            if turn_queue.active_index >= turn_queue.queue.len() {
                // Round completed
                break;
            }

            let active_entity = turn_queue.queue[turn_queue.active_index];

            // Verify the active entity exists and is alive
            let is_alive = {
                let p0 = token_set.p0();
                if let Ok(combat_stats) = p0.get(active_entity) {
                    combat_stats.hp > 0
                } else {
                    false
                }
            };

            if !is_alive {
                turn_queue.active_index += 1;
                continue;
            }

            // Decrement remaining cooldowns for this active entity before deciding actions
            {
                let mut p5 = token_set.p5();
                if let Ok(mut abils) = p5.get_mut(active_entity) {
                    for ab in &mut abils.abilities {
                        if ab.remaining_cooldown > 0 {
                            ab.remaining_cooldown -= 1;
                        }
                    }
                }
            }

            // Query active token components
            let active_token_info = {
                let active_query = token_set.p1();
                if let Ok((entity, maybe_enemy, grid_pos, in_room, maybe_move_range, maybe_hero_token, name, active_abilities)) =
                    active_query.get(active_entity)
                {
                    let active_enemy_ai = enemy_ai_query.get(entity).ok().cloned();
                    Some((
                        entity,
                        maybe_enemy.is_some(),
                        *grid_pos,
                        *in_room,
                        maybe_move_range.map(|mr| mr.max()).unwrap_or(3),
                        maybe_hero_token.map(|ht| ht.0),
                        get_clean_name(name, if maybe_enemy.is_some() { "Enemy" } else { "Hero" }),
                        active_abilities.cloned(),
                        active_enemy_ai,
                    ))
                } else {
                    None
                }
            };

            let Some((entity, is_enemy, grid_pos, in_room, _move_limit, maybe_hero_roster_entity, active_name, active_abilities, active_enemy_ai)) = active_token_info else {
                turn_queue.active_index += 1;
                continue;
            };

            if is_enemy {
                // --- Enemy Turn ---
                let current_room = in_room.0;

                // 1. Process active telegraphed attack
                let mut resolved_telegraph = false;
                if let Ok(mut telegraph) = telegraphed_attacks.get_mut(entity) {
                    telegraph.turns_remaining -= 1;
                    if telegraph.turns_remaining == 0 {
                        let target_room = telegraph.target_room;
                        resolved_telegraph = true;
 
                        // Deal 20 slam damage to all heroes in target_room
                        for &(hero_ent, ref hero_cs, hr, _) in &allies_list {
                            if hr.0 == Some(target_room) && hero_cs.hp > 0 {
                                let mut p0 = token_set.p0();
                                if let Ok(mut hc) = p0.get_mut(hero_ent) {
                                    hc.hp = (hc.hp - 20).max(0);
                                }
                                commands.entity(hero_ent).insert(VisualHit {
                                    amount: 20,
                                    is_hit: true,
                                    is_crit: false,
                                    effect_type: "Damage".to_string(),
                                    source: Some(entity),
                                    is_signature: false,
                                });
                                let hero_name = {
                                    let p3 = token_set.p3();
                                    p3.get(hero_ent).ok().and_then(|(_, _, _, _, name)| name).map(|n| get_clean_name(Some(n), "Hero")).unwrap_or_else(|| "Hero".to_string())
                                };
                                log_writer.write(MissionLogEvent {
                                    mission_entity: _mission_entity,
                                    payload: MissionLogPayload::Attack {
                                        attacker: active_name.clone(),
                                        defender: hero_name.clone(),
                                        damage: 20,
                                        is_crit: false,
                                        is_hit: true,
                                    },
                                });
                                // Trigger death log if hero dies
                                let check_hp = {
                                    let p0 = token_set.p0();
                                    p0.get(hero_ent).map(|hc| hc.hp).unwrap_or(0)
                                };
                                if check_hp <= 0 {
                                    log_writer.write(MissionLogEvent {
                                        mission_entity: _mission_entity,
                                        payload: MissionLogPayload::Death {
                                            name: hero_name,
                                            is_enemy: false,
                                        },
                                    });
                                }
                            }
                        }
                        commands.entity(entity).remove::<TelegraphedAttack>();
                    } else {
                        // Telegraph is winding up, skip turn
                        resolved_telegraph = true;
                    }
                }

                if resolved_telegraph {
                    turn_queue.active_index += 1;
                    continue;
                }

                // 2. Find target hero based on behavior
                let behavior = active_enemy_ai.as_ref().map(|ai| ai.behavior).unwrap_or(super::data::EnemyBehavior::Standard);
                let attack_range = active_enemy_ai.as_ref().map(|ai| ai.attack_range).unwrap_or(1);

                let target = if behavior == super::data::EnemyBehavior::Swarmer {
                    // Target the closest hero (Manhattan distance)
                    allies_list
                        .iter()
                        .filter(|(_, cs, r, _)| cs.hp > 0 && r.0 == current_room && current_room.is_some())
                        .min_by_key(|(_, _, _, gp)| grid_pos.x.abs_diff(gp.x) + grid_pos.y.abs_diff(gp.y))
                        .map(|(e, _, _, gp)| (*e, *gp))
                } else {
                    // Standard: lowest HP hero
                    allies_list
                        .iter()
                        .filter(|(_, cs, r, _)| cs.hp > 0 && r.0 == current_room && current_room.is_some())
                        .min_by_key(|(_, cs, _, _)| cs.hp)
                        .map(|(e, _, _, gp)| (*e, *gp))
                };

                let attacker_stats = {
                    let p0 = token_set.p0();
                    p0.get(entity).unwrap().clone()
                };

                // 2.5 Skirmisher kiting check
                let mut did_kite = false;
                if behavior == super::data::EnemyBehavior::Skirmisher {
                    let closest_hero = allies_list
                        .iter()
                        .filter(|(_, cs, r, _)| cs.hp > 0 && r.0 == current_room && current_room.is_some())
                        .map(|(e, _, _, gp)| (*e, *gp, grid_pos.x.abs_diff(gp.x) + grid_pos.y.abs_diff(gp.y)))
                        .min_by_key(|(_, _, dist)| *dist);

                    if let Some((_, closest_gp, dist_to_closest)) = closest_hero {
                        if dist_to_closest <= 2 {
                            let candidates = [
                                (grid_pos.x as i32 + 1, grid_pos.y as i32),
                                (grid_pos.x as i32 - 1, grid_pos.y as i32),
                                (grid_pos.x as i32, grid_pos.y as i32 + 1),
                                (grid_pos.x as i32, grid_pos.y as i32 - 1),
                            ];

                            let mut best_tile = None;
                            let mut max_dist = dist_to_closest;

                            for &(cx, cy) in &candidates {
                                if cx >= 0 && cy >= 0 {
                                    let ux = cx as u32;
                                    let uy = cy as u32;
                                    if map.is_walkable(ux, uy) {
                                        let new_dist = ux.abs_diff(closest_gp.x) + uy.abs_diff(closest_gp.y);
                                        if new_dist > max_dist {
                                            max_dist = new_dist;
                                            best_tile = Some((ux, uy));
                                        }
                                    }
                                }
                            }

                            if let Some((nx, ny)) = best_tile {
                                let mut p4 = token_set.p4();
                                if let Ok((mut gp, mut ir)) = p4.get_mut(entity) {
                                    gp.x = nx;
                                    gp.y = ny;
                                    ir.0 = map.room_at(nx, ny);
                                }
                                commands.entity(entity).insert(VisualPathHistory { waypoints: vec![(nx, ny)] });
                                did_kite = true;

                                log_writer.write(MissionLogEvent {
                                    mission_entity: _mission_entity,
                                    payload: MissionLogPayload::Ability {
                                        attacker: active_name.clone(),
                                        defender: "the threat".to_string(),
                                        ability_name: "Kite".to_string(),
                                        amount: 0,
                                        is_hit: true,
                                        is_crit: false,
                                        effect_type: "Movement".to_string(),
                                    },
                                });
                            }
                        }
                    }
                }

                if did_kite {
                    turn_queue.active_index += 1;
                    continue;
                }

                // 3. Evaluate enemy ability casting
                let mut cast_ability_id = None;
                let mut cast_target_entity = None;
                if let Some(ref abils) = active_abilities {
                    for ab in &abils.abilities {
                        if ab.remaining_cooldown == 0 {
                            if ab.ability_id == "Boss Slam" {
                                if !allies_list.iter().filter(|(_, _, r, _)| r.0 == current_room).collect::<Vec<_>>().is_empty() {
                                    cast_ability_id = Some(ab.ability_id.clone());
                                    cast_target_entity = Some(entity);
                                    break;
                                }
                            } else if ab.ability_id == "Boss Summon" {
                                let hp_pct = attacker_stats.hp as f32 / attacker_stats.max_hp.max(1) as f32;
                                if hp_pct < 0.6 && !allies_list.iter().filter(|(_, _, r, _)| r.0 == current_room).collect::<Vec<_>>().is_empty() {
                                    cast_ability_id = Some(ab.ability_id.clone());
                                    cast_target_entity = Some(entity);
                                    break;
                                }
                            } else if ab.ability_id == "Boss Enrage" {
                                if turn_queue.combat_round_count >= 8 && !enraged_query.get(entity).is_ok() {
                                    cast_ability_id = Some(ab.ability_id.clone());
                                    cast_target_entity = Some(entity);
                                    break;
                                }
                            } else if ab.ability_id == "Slash" {
                                if let Some((target_ent, target_gp)) = target {
                                    let dist = grid_pos.x.abs_diff(target_gp.x) + grid_pos.y.abs_diff(target_gp.y);
                                    if dist <= 1 {
                                        cast_ability_id = Some(ab.ability_id.clone());
                                        cast_target_entity = Some(target_ent);
                                        break;
                                    }
                                }
                            } else if let Some(ability_def) = ability_db.as_ref().and_then(|db| db.get(&ab.ability_id)) {
                                let range = ability_def.range;
                                match ability_def.effect {
                                    crate::hero::data::AbilityEffect::Damage | crate::hero::data::AbilityEffect::Debuff => {
                                        if let Some((target_ent, target_gp)) = target {
                                            let dist = grid_pos.x.abs_diff(target_gp.x) + grid_pos.y.abs_diff(target_gp.y);
                                            if dist <= range {
                                                cast_ability_id = Some(ab.ability_id.clone());
                                                cast_target_entity = Some(target_ent);
                                                break;
                                            }
                                        }
                                    }
                                    crate::hero::data::AbilityEffect::Heal => {
                                        let wounded_ally = enemies_list
                                            .iter()
                                            .filter(|(_, gp, r, cs)| {
                                                cs.hp > 0
                                                    && cs.hp < cs.max_hp
                                                    && r.0 == current_room
                                                    && current_room.is_some()
                                                    && (grid_pos.x.abs_diff(gp.x) + grid_pos.y.abs_diff(gp.y) <= range)
                                            })
                                            .min_by_key(|(_, _, _, cs)| cs.hp)
                                            .map(|(e, _, _, _)| e);
                                        if let Some(&ally_ent) = wounded_ally {
                                            cast_ability_id = Some(ab.ability_id.clone());
                                            cast_target_entity = Some(ally_ent);
                                            break;
                                        }
                                    }
                                    crate::hero::data::AbilityEffect::Shield | crate::hero::data::AbilityEffect::Buff => {
                                        let hp_pct = attacker_stats.hp as f32 / attacker_stats.max_hp.max(1) as f32;
                                        let should_cast = match ability_def.ai_priority {
                                            crate::hero::data::AiPriorityRule::HpBelowPct(pct) => hp_pct < pct,
                                            _ => true,
                                        };
                                        if should_cast {
                                            cast_ability_id = Some(ab.ability_id.clone());
                                            cast_target_entity = Some(entity);
                                            break;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                if let Some(ability_id) = cast_ability_id {
                    let cooldown = ability_db
                        .as_ref()
                        .and_then(|db| db.get(&ability_id))
                        .map(|d| d.cooldown)
                        .unwrap_or(3);

                    let mut p5 = token_set.p5();
                    if let Ok(mut abils) = p5.get_mut(entity) {
                        if let Some(ab) = abils.abilities.iter_mut().find(|a| a.ability_id == ability_id) {
                            ab.remaining_cooldown = cooldown;
                        }
                    }

                    if ability_id == "Boss Slam" {
                        commands.entity(entity).insert(TelegraphedAttack {
                            target_room: current_room.unwrap_or(0),
                            turns_remaining: 1,
                        });
                        log_writer.write(MissionLogEvent {
                            mission_entity: _mission_entity,
                            payload: MissionLogPayload::Ability {
                                attacker: active_name.clone(),
                                defender: "the room".to_string(),
                                ability_name: "Boss Slam".to_string(),
                                amount: 0,
                                is_hit: true,
                                is_crit: false,
                                effect_type: "Damage".to_string(),
                            },
                        });
                    } else if ability_id == "Boss Summon" {
                        log_writer.write(MissionLogEvent {
                            mission_entity: _mission_entity,
                            payload: MissionLogPayload::Ability {
                                attacker: active_name.clone(),
                                defender: "giant rats".to_string(),
                                ability_name: "Boss Summon".to_string(),
                                amount: 0,
                                is_hit: true,
                                is_crit: false,
                                effect_type: "Buff".to_string(),
                            },
                        });

                        if let Some(r_idx) = current_room {
                            for _ in 0..2 {
                                let (rx, ry) = map
                                    .random_walkable_in_room(r_idx, &mut rng)
                                    .unwrap_or_else(|| map.rooms[r_idx].center());
                                
                                let rat_token = commands.spawn((
                                    Name::new("Giant Rat"),
                                    EnemyToken {
                                        enemy_type: super::data::EnemyType::GiantRat,
                                        xp_reward: 2,
                                    },
                                    EnemyAI {
                                        behavior: super::data::EnemyBehavior::Swarmer,
                                        attack_range: 1,
                                    },
                                    GridPosition { x: rx, y: ry },
                                    InRoom(Some(r_idx)),
                                    CombatStats {
                                        hp: 10,
                                        max_hp: 10,
                                        attack: 2,
                                        defense: 1,
                                        speed: 12,
                                    },
                                    MoveRange {
                                        base: 3,
                                        bonus: 0,
                                    },
                                    ActiveAbilities {
                                        abilities: vec![],
                                    },
                                    ChildOf(_mission_entity),
                                )).id();

                                turn_queue.queue.push(rat_token);
                            }
                        }
                    } else if ability_id == "Boss Enrage" {
                        commands.entity(entity).insert(Enraged);
                        log_writer.write(MissionLogEvent {
                            mission_entity: _mission_entity,
                            payload: MissionLogPayload::Ability {
                                attacker: active_name.clone(),
                                defender: "himself".to_string(),
                                ability_name: "Boss Enrage".to_string(),
                                amount: 0,
                                is_hit: true,
                                is_crit: false,
                                effect_type: "Buff".to_string(),
                            },
                        });
                    } else if ability_id == "Slash" {
                        if let Some(target_entity) = cast_target_entity {
                            let target_gp = allies_list.iter().find(|(e, _, _, _)| *e == target_entity).map(|(_, _, _, gp)| *gp);
                            let swarmer_bonus = if let Some(tgp) = target_gp {
                                if behavior == super::data::EnemyBehavior::Swarmer {
                                    enemies_list
                                        .iter()
                                        .filter(|(e, gp, _, cs)| {
                                            *e != entity
                                                && cs.hp > 0
                                                && gp.x.abs_diff(tgp.x) + gp.y.abs_diff(tgp.y) <= 1
                                        })
                                        .count() as i32
                                } else {
                                    0
                                }
                            } else {
                                0
                            };

                            let target_name = {
                                let p3 = token_set.p3();
                                p3.get(target_entity).ok().and_then(|(_, _, _, _, name)| name).map(|n| get_clean_name(Some(n), "Hero")).unwrap_or_else(|| "Hero".to_string())
                            };
                            let is_enraged = enraged_query.get(entity).is_ok();

                            let mut p0 = token_set.p0();
                            if let Ok(mut hero_combat) = p0.get_mut(target_entity) {
                                let d20 = rng.random_range(1..=20);
                                let roll = d20 + attacker_stats.attack + swarmer_bonus;
                                let target_ac = hero_combat.defense + 10;
                                let is_hit = roll >= target_ac;
                                let is_crit = d20 == 20;
                                let damage = if is_hit {
                                    let mut base_dmg = (attacker_stats.attack / 2 + rng.random_range(1..=3)).max(1);
                                    if is_enraged {
                                        base_dmg *= 2;
                                    }
                                    base_dmg += swarmer_bonus;
                                    if is_crit {
                                        base_dmg * 2
                                    } else {
                                        base_dmg
                                    }
                                } else {
                                    0
                                };

                                if is_hit {
                                    hero_combat.hp -= damage;
                                }

                                commands.entity(target_entity).insert(VisualHit {
                                    amount: damage,
                                    is_hit,
                                    is_crit,
                                    effect_type: "Damage".to_string(),
                                    source: Some(entity),
                                    is_signature: false,
                                });

                                log_writer.write(MissionLogEvent {
                                    mission_entity: _mission_entity,
                                    payload: MissionLogPayload::Ability {
                                        attacker: active_name.clone(),
                                        defender: target_name.clone(),
                                        ability_name: "Slash".to_string(),
                                        amount: damage,
                                        is_hit,
                                        is_crit,
                                        effect_type: "Damage".to_string(),
                                    },
                                });

                                if is_hit && hero_combat.hp <= 0 {
                                    log_writer.write(MissionLogEvent {
                                        mission_entity: _mission_entity,
                                        payload: MissionLogPayload::Death {
                                            name: target_name,
                                            is_enemy: false,
                                        },
                                    });
                                }
                            }
                        }
                    } else if let Some(ability_def) = ability_db.as_ref().and_then(|db| db.get(&ability_id)) {
                        if let Some(target_entity) = cast_target_entity {
                            let target_name = {
                                if let Ok((_, _, _, _, name)) = token_set.p2().get(target_entity) {
                                    name.map(|n| get_clean_name(Some(n), "Enemy")).unwrap_or_else(|| "Enemy".to_string())
                                } else if let Ok((_, _, _, _, name)) = token_set.p3().get(target_entity) {
                                    name.map(|n| get_clean_name(Some(n), "Hero")).unwrap_or_else(|| "Hero".to_string())
                                } else if target_entity == entity {
                                    active_name.clone()
                                } else {
                                    "Target".to_string()
                                }
                            };

                            let mut amount = 0;

                            match ability_def.effect {
                                crate::hero::data::AbilityEffect::Damage | crate::hero::data::AbilityEffect::Debuff => {
                                    let target_gp = allies_list.iter().find(|(e, _, _, _)| *e == target_entity).map(|(_, _, _, gp)| *gp);
                                    let swarmer_bonus = if let Some(tgp) = target_gp {
                                        if behavior == super::data::EnemyBehavior::Swarmer {
                                            enemies_list
                                                .iter()
                                                .filter(|(e, gp, _, cs)| {
                                                    *e != entity
                                                        && cs.hp > 0
                                                        && gp.x.abs_diff(tgp.x) + gp.y.abs_diff(tgp.y) <= 1
                                                })
                                                .count() as i32
                                        } else {
                                            0
                                        }
                                    } else {
                                        0
                                    };

                                    let is_enraged = enraged_query.get(entity).is_ok();
                                    let mut p0 = token_set.p0();
                                    if let Ok(mut hero_combat) = p0.get_mut(target_entity) {
                                        let d20 = rng.random_range(1..=20);
                                        let roll = d20 + attacker_stats.attack + swarmer_bonus;
                                        let target_ac = hero_combat.defense + 10;
                                        let is_hit = roll >= target_ac;
                                        let is_crit = d20 == 20;
                                        if is_hit {
                                            let mut base_dmg = (attacker_stats.attack / 2 + rng.random_range(1..=3)).max(1);
                                            if ability_id == "Power Shot" {
                                                base_dmg = (attacker_stats.attack / 2 + rng.random_range(2..=5)).max(2);
                                            }
                                            if is_enraged {
                                                base_dmg *= 2;
                                            }
                                            base_dmg += swarmer_bonus;
                                            if is_crit {
                                                base_dmg *= 2;
                                            }
                                            amount = base_dmg;
                                            hero_combat.hp -= base_dmg;
                                        }

                                        commands.entity(target_entity).insert(VisualHit {
                                            amount,
                                            is_hit,
                                            is_crit,
                                            effect_type: "Damage".to_string(),
                                            source: Some(entity),
                                            is_signature: false,
                                        });

                                        log_writer.write(MissionLogEvent {
                                            mission_entity: _mission_entity,
                                            payload: MissionLogPayload::Ability {
                                                attacker: active_name.clone(),
                                                defender: target_name.clone(),
                                                ability_name: ability_def.name.clone(),
                                                amount,
                                                is_hit,
                                                is_crit,
                                                effect_type: "Damage".to_string(),
                                            },
                                        });

                                        if is_hit && hero_combat.hp <= 0 {
                                            log_writer.write(MissionLogEvent {
                                                mission_entity: _mission_entity,
                                                payload: MissionLogPayload::Death {
                                                    name: target_name,
                                                    is_enemy: false,
                                                },
                                            });
                                        }
                                    }
                                }
                                crate::hero::data::AbilityEffect::Heal => {
                                    let mut p0 = token_set.p0();
                                    if let Ok(mut ally_combat) = p0.get_mut(target_entity) {
                                        let heal = rng.random_range(5..=12) + (attacker_stats.attack / 2);
                                        ally_combat.hp = (ally_combat.hp + heal).min(ally_combat.max_hp);
                                        amount = heal;

                                        commands.entity(target_entity).insert(VisualHit {
                                            amount,
                                            is_hit: true,
                                            is_crit: false,
                                            effect_type: "Heal".to_string(),
                                            source: Some(entity),
                                            is_signature: false,
                                        });

                                        log_writer.write(MissionLogEvent {
                                            mission_entity: _mission_entity,
                                            payload: MissionLogPayload::Ability {
                                                attacker: active_name.clone(),
                                                defender: target_name.clone(),
                                                ability_name: ability_def.name.clone(),
                                                amount,
                                                is_hit: true,
                                                is_crit: false,
                                                effect_type: "Heal".to_string(),
                                            },
                                        });
                                    }
                                }
                                crate::hero::data::AbilityEffect::Shield | crate::hero::data::AbilityEffect::Buff => {
                                    let mut p0 = token_set.p0();
                                    if let Ok(mut ally_combat) = p0.get_mut(target_entity) {
                                        let shield = attacker_stats.defense + rng.random_range(3..=6);
                                        ally_combat.hp += shield;
                                        amount = shield;

                                        commands.entity(target_entity).insert(VisualHit {
                                            amount,
                                            is_hit: true,
                                            is_crit: false,
                                            effect_type: "Shield".to_string(),
                                            source: Some(entity),
                                            is_signature: false,
                                        });

                                        log_writer.write(MissionLogEvent {
                                            mission_entity: _mission_entity,
                                            payload: MissionLogPayload::Ability {
                                                attacker: active_name.clone(),
                                                defender: target_name.clone(),
                                                ability_name: ability_def.name.clone(),
                                                amount,
                                                is_hit: true,
                                                is_crit: false,
                                                effect_type: "Shield".to_string(),
                                            },
                                        });
                                    }
                                }
                            }
                        }
                    }

                    turn_queue.active_index += 1;
                    continue;
                }

                // 4. Standard adjacent attack or move closer
                if let Some((target_entity, target_gp)) = target {
                    let dist = grid_pos.x.abs_diff(target_gp.x) + grid_pos.y.abs_diff(target_gp.y);
                    if dist <= attack_range {
                        let target_name = {
                            let p3 = token_set.p3();
                            p3.get(target_entity).ok().and_then(|(_, _, _, _, name)| name).map(|n| get_clean_name(Some(n), "Hero")).unwrap_or_else(|| "Hero".to_string())
                        };
                        let is_enraged = enraged_query.get(entity).is_ok();

                        let swarmer_bonus = if behavior == super::data::EnemyBehavior::Swarmer {
                            enemies_list
                                .iter()
                                .filter(|(e, gp, _, cs)| {
                                    *e != entity
                                        && cs.hp > 0
                                        && gp.x.abs_diff(target_gp.x) + gp.y.abs_diff(target_gp.y) <= 1
                                })
                                .count() as i32
                        } else {
                            0
                        };

                        let mut p0 = token_set.p0();
                        if let Ok(mut hero_combat) = p0.get_mut(target_entity) {
                            let d20 = rng.random_range(1..=20);
                            let roll = d20 + attacker_stats.attack + swarmer_bonus;
                            let target_ac = hero_combat.defense + 10;
                            let is_hit = roll >= target_ac;
                            let is_crit = d20 == 20;
                            let damage = if is_hit {
                                let mut base_dmg = (attacker_stats.attack / 2 + rng.random_range(1..=3)).max(1);
                                if is_enraged {
                                    base_dmg *= 2;
                                }
                                base_dmg += swarmer_bonus;
                                if is_crit {
                                    base_dmg * 2
                                } else {
                                    base_dmg
                                }
                            } else {
                                0
                            };

                            if is_hit {
                                hero_combat.hp -= damage;
                            }

                            commands.entity(target_entity).insert(VisualHit {
                                amount: damage,
                                is_hit,
                                is_crit,
                                effect_type: "Damage".to_string(),
                                source: Some(entity),
                                is_signature: false,
                            });

                            log_writer.write(MissionLogEvent {
                                mission_entity: _mission_entity,
                                payload: MissionLogPayload::Attack {
                                    attacker: active_name.clone(),
                                    defender: target_name.clone(),
                                    damage,
                                    is_crit,
                                    is_hit,
                                },
                            });

                            if is_hit && hero_combat.hp <= 0 {
                                log_writer.write(MissionLogEvent {
                                    mission_entity: _mission_entity,
                                    payload: MissionLogPayload::Death {
                                        name: target_name,
                                        is_enemy: false,
                                    },
                                });
                            }
                        }
                    } else {
                        // Out of range -> Move one tile closer!
                        if let Some(path) = find_path(map, (grid_pos.x, grid_pos.y), (target_gp.x, target_gp.y))
                            && path.len() > 1 {
                                let step_idx = 1;
                                let (nx, ny) = path[step_idx];
                                let mut p4 = token_set.p4();
                                if let Ok((mut gp, mut ir)) = p4.get_mut(entity) {
                                    gp.x = nx;
                                    gp.y = ny;
                                    ir.0 = map.room_at(nx, ny);
                                }
                                let waypoints = path[1..=step_idx].to_vec();
                                commands.entity(entity).insert(VisualPathHistory { waypoints });
                            }
                    }
                }
            } else if let Some(hero_roster_entity) = maybe_hero_roster_entity {
                // --- Hero Turn ---
                if let Ok((info, stats, traits, maybe_fatigue, maybe_injured)) = hero_data_query.get(hero_roster_entity) {
                    let is_injured = maybe_injured.is_some();
                    let active_combat = {
                        let p0 = token_set.p0();
                        p0.get(entity).unwrap().clone()
                    };

                    let mut has_leader_in_room = false;
                    for &hero_token_ent in &mission_hero_entities {
                        if let Ok(combat_stats) = token_set.p0().get(hero_token_ent) {
                            if combat_stats.hp > 0 {
                                if let Ok((_, _, _, in_room_comp, _, maybe_ht, _, _)) = token_set.p1().get(hero_token_ent) {
                                    if in_room_comp.0 == in_room.0 && in_room.0.is_some() {
                                        if let Some(ht) = maybe_ht {
                                            if let Ok((_, _, ht_traits, _, _)) = hero_data_query.get(ht.0) {
                                                if ht_traits.0.contains(&HeroTrait::Leader) {
                                                    has_leader_in_room = true;
                                                    break;
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    let is_near_ally = allies_list.iter().any(|(ally_ent, _, ally_room, ally_gp)| {
                        *ally_ent != entity
                            && ally_room.0 == in_room.0
                            && in_room.0.is_some()
                            && grid_pos.x.abs_diff(ally_gp.x) + grid_pos.y.abs_diff(ally_gp.y) <= 3
                    });
                    
                    let action = decide_action(
                        entity,
                        info,
                        stats,
                        traits,
                        &grid_pos,
                        &in_room,
                        &active_combat,
                        map,
                        room_status,
                        &enemies_list,
                        &mission_enemy_entities,
                        &allies_list,
                        &mission_hero_entities,
                        active_abilities.as_ref(),
                        ability_db.as_deref(),
                        turn_queue.combat_round_count,
                        &mission_chests,
                        &mut rng,
                        modifiers,
                        &unsafe_rooms,
                    );

                    match action {
                        HeroAction::MoveTo(room_idx) => {
                            let room = &map.rooms[room_idx];
                            let (gx, gy) = room.center();
                            if let Some(path) = find_path(map, (grid_pos.x, grid_pos.y), (gx, gy))
                                && path.len() > 1 {
                                    let mut nx = path[1].0;
                                    let mut ny = path[1].1;
                                    if traits.0.contains(&HeroTrait::Loner) {
                                        let mut max_dist_to_ally = 0;
                                        let current_dist_to_target = grid_pos.x.abs_diff(gx) + grid_pos.y.abs_diff(gy);
                                        let candidates = [
                                            (grid_pos.x as i32 + 1, grid_pos.y as i32),
                                            (grid_pos.x as i32 - 1, grid_pos.y as i32),
                                            (grid_pos.x as i32, grid_pos.y as i32 + 1),
                                            (grid_pos.x as i32, grid_pos.y as i32 - 1),
                                        ];
                                        let mut found_steered_tile = false;
                                        let mut best_tile = (nx, ny);
                                        for &(cx, cy) in &candidates {
                                            if cx >= 0 && cy >= 0 {
                                                let ux = cx as u32;
                                                let uy = cy as u32;
                                                if map.is_walkable(ux, uy) {
                                                    let dist_to_target = ux.abs_diff(gx) + uy.abs_diff(gy);
                                                    if dist_to_target < current_dist_to_target {
                                                        let min_ally_dist = allies_list
                                                            .iter()
                                                            .filter(|(ally_ent, _, _, _)| *ally_ent != entity)
                                                            .map(|(_, _, _, ally_gp)| {
                                                                ux.abs_diff(ally_gp.x) + uy.abs_diff(ally_gp.y)
                                                            })
                                                            .min()
                                                            .unwrap_or(999);
                                                        if min_ally_dist > max_dist_to_ally {
                                                            max_dist_to_ally = min_ally_dist;
                                                            best_tile = (ux, uy);
                                                            found_steered_tile = true;
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                        if found_steered_tile {
                                            nx = best_tile.0;
                                            ny = best_tile.1;
                                        }
                                    }
                                    let mut p4 = token_set.p4();
                                    if let Ok((mut gp, mut ir)) = p4.get_mut(entity) {
                                        gp.x = nx;
                                        gp.y = ny;
                                        ir.0 = map.room_at(nx, ny);
                                    }
                                    let waypoints = vec![(nx, ny)];
                                    commands.entity(entity).insert(VisualPathHistory { waypoints });
                                }
                        }
                        HeroAction::MoveToTile(tx, ty) => {
                            if let Some(path) = find_path(map, (grid_pos.x, grid_pos.y), (tx, ty))
                                && path.len() > 1 {
                                    let mut nx = path[1].0;
                                    let mut ny = path[1].1;
                                    if traits.0.contains(&HeroTrait::Loner) {
                                        let mut max_dist_to_ally = 0;
                                        let current_dist_to_target = grid_pos.x.abs_diff(tx) + grid_pos.y.abs_diff(ty);
                                        let candidates = [
                                            (grid_pos.x as i32 + 1, grid_pos.y as i32),
                                            (grid_pos.x as i32 - 1, grid_pos.y as i32),
                                            (grid_pos.x as i32, grid_pos.y as i32 + 1),
                                            (grid_pos.x as i32, grid_pos.y as i32 - 1),
                                        ];
                                        let mut found_steered_tile = false;
                                        let mut best_tile = (nx, ny);
                                        for &(cx, cy) in &candidates {
                                            if cx >= 0 && cy >= 0 {
                                                let ux = cx as u32;
                                                let uy = cy as u32;
                                                if map.is_walkable(ux, uy) {
                                                    let dist_to_target = ux.abs_diff(tx) + uy.abs_diff(ty);
                                                    if dist_to_target < current_dist_to_target {
                                                        let min_ally_dist = allies_list
                                                            .iter()
                                                            .filter(|(ally_ent, _, _, _)| *ally_ent != entity)
                                                            .map(|(_, _, _, ally_gp)| {
                                                                ux.abs_diff(ally_gp.x) + uy.abs_diff(ally_gp.y)
                                                            })
                                                            .min()
                                                            .unwrap_or(999);
                                                        if min_ally_dist > max_dist_to_ally {
                                                            max_dist_to_ally = min_ally_dist;
                                                            best_tile = (ux, uy);
                                                            found_steered_tile = true;
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                        if found_steered_tile {
                                            nx = best_tile.0;
                                            ny = best_tile.1;
                                        }
                                    }
                                    let mut p4 = token_set.p4();
                                    if let Ok((mut gp, mut ir)) = p4.get_mut(entity) {
                                        gp.x = nx;
                                        gp.y = ny;
                                        ir.0 = map.room_at(nx, ny);
                                    }
                                    let waypoints = vec![(nx, ny)];
                                    commands.entity(entity).insert(VisualPathHistory { waypoints });
                                }
                        }
                        HeroAction::Attack(target_entity) => {
                            let active_equipment = hero_equip_query.get(hero_roster_entity).ok();
                            let target_name = {
                                let p2 = token_set.p2();
                                p2.get(target_entity).ok().and_then(|(_, _, _, _, name)| name).map(|n| get_clean_name(Some(n), "Enemy")).unwrap_or_else(|| "Enemy".to_string())
                            };

                            let (is_hit, is_crit, damage, enemy_died) = {
                                let mut p0 = token_set.p0();
                                if let Ok(mut enemy_combat) = p0.get_mut(target_entity)
                                    && enemy_combat.hp > 0 {
                                        let lucky = traits.0.contains(&HeroTrait::Lucky);
                                        let luck_bonus = if lucky { 3 } else { 0 };
                                        let leader_bonus = if has_leader_in_room { 2 } else { 0 };
                                        
                                        let d20 = rng.random_range(1..=20);
                                        let roll = d20 + active_combat.attack + luck_bonus + leader_bonus;
                                        let target_ac = enemy_combat.defense + 10;
                                        let is_hit = roll >= target_ac;
                                        let is_crit = d20 == 20;
                                        let mut damage = if is_hit {
                                            let base_dmg = (active_combat.attack / 2 + rng.random_range(1..=4)).max(1);
                                            if is_crit {
                                                base_dmg * 2
                                            } else {
                                                base_dmg
                                            }
                                        } else {
                                            0
                                        };

                                        if traits.0.contains(&HeroTrait::Loner) && is_near_ally {
                                            damage = (damage as f32 * 0.9).floor() as i32;
                                        }

                                        if is_hit {
                                            enemy_combat.hp -= damage;
                                        }
                                        (is_hit, is_crit, damage, is_hit && enemy_combat.hp <= 0)
                                } else {
                                    (false, false, 0, false)
                                }
                            };

                            if is_hit {
                                // Lifesteal
                                if let Some(eq) = active_equipment {
                                    if eq.has_affix(crate::equipment::BehavioralAffix::Lifesteal) {
                                        let heal_amt = (damage as f32 * 0.3).round() as i32;
                                        if heal_amt > 0 {
                                            let mut p0 = token_set.p0();
                                            if let Ok(mut active_stats) = p0.get_mut(entity) {
                                                active_stats.hp = (active_stats.hp + heal_amt).min(active_stats.max_hp);
                                            }
                                            log_writer.write(MissionLogEvent {
                                                mission_entity: _mission_entity,
                                                payload: MissionLogPayload::Heal {
                                                    healer: active_name.clone(),
                                                    target: active_name.clone(),
                                                    amount: heal_amt,
                                                },
                                            });
                                        }
                                    }
                                }

                                // CleaveOnHit
                                if let Some(eq) = active_equipment {
                                    if eq.has_affix(crate::equipment::BehavioralAffix::CleaveOnHit) {
                                        let cleave_damage = (damage as f32 * 0.5).round() as i32;
                                        if cleave_damage > 0 {
                                            let target_gp = {
                                                let p1 = token_set.p1();
                                                p1.get(target_entity).map(|(_, _, gp, _, _, _, _, _)| *gp).ok()
                                            };
                                            if let Some(tgp) = target_gp {
                                                let p1 = token_set.p1();
                                                let mut adjacent_enemies = Vec::new();
                                                for (other_ent, maybe_enemy, other_gp, _other_room, _, _, other_name, _) in p1.iter() {
                                                    if other_ent != target_entity && other_ent != entity && maybe_enemy.is_some() {
                                                        let dist = tgp.x.abs_diff(other_gp.x) + tgp.y.abs_diff(other_gp.y);
                                                        if dist == 1 {
                                                            adjacent_enemies.push((other_ent, get_clean_name(other_name, "Enemy")));
                                                        }
                                                    }
                                                }

                                                for (adj_ent, adj_name) in adjacent_enemies {
                                                    let mut p0 = token_set.p0();
                                                    if let Ok(mut adj_combat) = p0.get_mut(adj_ent) {
                                                        adj_combat.hp -= cleave_damage;
                                                        let adj_died = adj_combat.hp <= 0;
                                                        commands.entity(adj_ent).insert(VisualHit {
                                                            amount: cleave_damage,
                                                            is_hit: true,
                                                            is_crit: false,
                                                            effect_type: "Damage".to_string(),
                                                            source: Some(entity),
                                                            is_signature: false,
                                                        });
                                                        log_writer.write(MissionLogEvent {
                                                            mission_entity: _mission_entity,
                                                            payload: MissionLogPayload::Attack {
                                                                attacker: active_name.clone(),
                                                                defender: adj_name.clone(),
                                                                damage: cleave_damage,
                                                                is_crit: false,
                                                                is_hit: true,
                                                            },
                                                        });
                                                        if adj_died {
                                                            log_writer.write(MissionLogEvent {
                                                                mission_entity: _mission_entity,
                                                                payload: MissionLogPayload::Death {
                                                                    name: adj_name,
                                                                    is_enemy: true,
                                                                },
                                                            });
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }

                            if is_hit || damage == 0 {
                                commands.entity(target_entity).insert(VisualHit {
                                    amount: damage,
                                    is_hit,
                                    is_crit,
                                    effect_type: "Damage".to_string(),
                                    source: Some(entity),
                                    is_signature: false,
                                });

                                log_writer.write(MissionLogEvent {
                                    mission_entity: _mission_entity,
                                    payload: MissionLogPayload::Attack {
                                        attacker: active_name.clone(),
                                        defender: target_name.clone(),
                                        damage,
                                        is_crit,
                                        is_hit,
                                    },
                                });

                                if enemy_died {
                                    log_writer.write(MissionLogEvent {
                                        mission_entity: _mission_entity,
                                        payload: MissionLogPayload::Death {
                                            name: target_name,
                                            is_enemy: true,
                                        },
                                    });
                                }
                            }
                        }
                        HeroAction::Heal(target_entity) => {
                            let target_name = {
                                let p3 = token_set.p3();
                                p3.get(target_entity).ok().and_then(|(_, _, _, _, name)| name).map(|n| get_clean_name(Some(n), "Hero")).unwrap_or_else(|| "Hero".to_string())
                            };

                            let mut p0 = token_set.p0();
                            if let Ok(mut ally_combat) = p0.get_mut(target_entity)
                                && ally_combat.hp > 0 && ally_combat.hp < ally_combat.max_hp {
                                    let lucky = traits.0.contains(&HeroTrait::Lucky);
                                    let luck_bonus = if lucky { 3 } else { 0 };
                                    let leader_bonus = if has_leader_in_room { 2 } else { 0 };
                                    
                                    let mut heal = rng.random_range(1..=8) + luck_bonus + leader_bonus;
                                    if traits.0.contains(&HeroTrait::Loner) && is_near_ally {
                                        heal = (heal as f32 * 0.9).floor() as i32;
                                    }
                                    if modifiers.contains(&super::data::MissionModifier::CursedGround) {
                                        heal = 0;
                                    }

                                    ally_combat.hp = (ally_combat.hp + heal).min(ally_combat.max_hp);

                                    commands.entity(target_entity).insert(VisualHit {
                                        amount: heal,
                                        is_hit: true,
                                        is_crit: false,
                                        effect_type: "Heal".to_string(),
                                        source: Some(entity),
                                        is_signature: false,
                                    });

                                    log_writer.write(MissionLogEvent {
                                        mission_entity: _mission_entity,
                                        payload: MissionLogPayload::Heal {
                                            healer: active_name.clone(),
                                            target: target_name,
                                            amount: heal,
                                        },
                                    });
                                }
                        }
                        HeroAction::UseAbility(ref ability_id, target_entity) => {
                            let ability_def = ability_db.as_ref().and_then(|db| db.get(ability_id));
                            if let Some(ability_def) = ability_def {
                                if ability_id == "Rallying Cry" {
                                    let mut targets = vec![entity];
                                    let caster_gp = {
                                        if let Ok((_, _, gp, _, _, _, _, _)) = token_set.p1().get(entity) {
                                            Some(*gp)
                                        } else {
                                            None
                                        }
                                    };
                                    if let Some(caster_gp) = caster_gp {
                                        let p1 = token_set.p1();
                                        for (ent, _, gp, _, _, maybe_hero, _, _) in p1.iter() {
                                            if ent != entity && maybe_hero.is_some() {
                                                let dist = caster_gp.x.abs_diff(gp.x) + caster_gp.y.abs_diff(gp.y);
                                                if dist <= 3 {
                                                    targets.push(ent);
                                                }
                                            }
                                        }
                                    }

                                    for &t in &targets {
                                        let mut p0 = token_set.p0();
                                        if let Ok(mut ally_combat) = p0.get_mut(t)
                                            && ally_combat.hp > 0 {
                                                let leader_bonus = if has_leader_in_room { 2 } else { 0 };
                                                let mut amount = active_combat.defense + rng.random_range(10..=20) + leader_bonus;
                                                if traits.0.contains(&HeroTrait::Loner) && is_near_ally {
                                                    amount = (amount as f32 * 0.9).floor() as i32;
                                                }
                                                if modifiers.contains(&super::data::MissionModifier::CursedGround) {
                                                    amount = 0;
                                                }
                                                ally_combat.hp += amount;

                                                let t_name = {
                                                    let p3 = token_set.p3();
                                                    p3.get(t).ok().and_then(|(_, _, _, _, name)| name).map(|n| get_clean_name(Some(n), "Hero")).unwrap_or_else(|| "Hero".to_string())
                                                };
                                                log_writer.write(MissionLogEvent {
                                                    mission_entity: _mission_entity,
                                                    payload: MissionLogPayload::Ability {
                                                        attacker: active_name.clone(),
                                                        defender: t_name,
                                                        ability_name: ability_def.name.clone(),
                                                        amount,
                                                        is_hit: true,
                                                        is_crit: false,
                                                        effect_type: "Buff".to_string(),
                                                    },
                                                });
                                            }
                                    }
                                } else if ability_id == "Assassinate" {
                                    let active_equipment = hero_equip_query.get(hero_roster_entity).ok();
                                    let target_name = {
                                        let p2 = token_set.p2();
                                        p2.get(target_entity).ok().and_then(|(_, _, _, _, name)| name).map(|n| get_clean_name(Some(n), "Enemy")).unwrap_or_else(|| "Enemy".to_string())
                                    };
                                    let (is_hit, is_crit, amount, enemy_died) = {
                                        let mut p0 = token_set.p0();
                                        if let Ok(mut enemy_combat) = p0.get_mut(target_entity)
                                            && enemy_combat.hp > 0 {
                                                let lucky = traits.0.contains(&HeroTrait::Lucky);
                                                let luck_bonus = if lucky { 3 } else { 0 };
                                                let leader_bonus = if has_leader_in_room { 2 } else { 0 };

                                                let d20 = rng.random_range(1..=20);
                                                let roll = d20 + active_combat.attack + luck_bonus + leader_bonus;
                                                let target_ac = enemy_combat.defense + 10;
                                                let is_hit = roll >= target_ac;
                                                let is_crit = d20 == 20;

                                                let mut amount = if is_hit {
                                                    let base_dmg = (active_combat.attack * 3 + rng.random_range(10..=20)).max(1);
                                                    if is_crit { base_dmg * 2 } else { base_dmg }
                                                } else {
                                                    0
                                                };
                                                if traits.0.contains(&HeroTrait::Loner) && is_near_ally {
                                                    amount = (amount as f32 * 0.9).floor() as i32;
                                                }

                                                if is_hit {
                                                    enemy_combat.hp -= amount;
                                                }
                                                (is_hit, is_crit, amount, is_hit && enemy_combat.hp <= 0)
                                        } else {
                                            (false, false, 0, false)
                                        }
                                    };

                                    if is_hit {
                                        if let Some(eq) = active_equipment {
                                            if eq.has_affix(crate::equipment::BehavioralAffix::Lifesteal) {
                                                let heal_amt = (amount as f32 * 0.3).round() as i32;
                                                if heal_amt > 0 {
                                                    let mut p0 = token_set.p0();
                                                    if let Ok(mut active_stats) = p0.get_mut(entity) {
                                                        active_stats.hp = (active_stats.hp + heal_amt).min(active_stats.max_hp);
                                                    }
                                                    log_writer.write(MissionLogEvent {
                                                        mission_entity: _mission_entity,
                                                        payload: MissionLogPayload::Heal {
                                                            healer: active_name.clone(),
                                                            target: active_name.clone(),
                                                            amount: heal_amt,
                                                        },
                                                    });
                                                }
                                            }
                                        }
                                    }

                                    if is_hit || amount == 0 {
                                        commands.entity(target_entity).insert(VisualHit {
                                            amount,
                                            is_hit,
                                            is_crit,
                                            effect_type: "Damage".to_string(),
                                            source: Some(entity),
                                            is_signature: true,
                                        });

                                        log_writer.write(MissionLogEvent {
                                            mission_entity: _mission_entity,
                                            payload: MissionLogPayload::Ability {
                                                attacker: active_name.clone(),
                                                defender: target_name.clone(),
                                                ability_name: ability_def.name.clone(),
                                                amount,
                                                is_hit,
                                                is_crit,
                                                effect_type: "Damage".to_string(),
                                              },
                                        });

                                        if enemy_died {
                                            log_writer.write(MissionLogEvent {
                                                mission_entity: _mission_entity,
                                                payload: MissionLogPayload::Death {
                                                    name: target_name,
                                                    is_enemy: true,
                                                },
                                            });
                                        }
                                    }
                                } else if ability_id == "Meteor" {
                                    let active_equipment = hero_equip_query.get(hero_roster_entity).ok();
                                    let target_gp = {
                                        let p1 = token_set.p1();
                                        p1.get(target_entity).map(|(_, _, gp, _, _, _, _, _)| *gp).ok()
                                    };
                                    let mut targets = vec![target_entity];
                                    if let Some(tgp) = target_gp {
                                        let p1 = token_set.p1();
                                        for (ent, maybe_enemy, gp, _, _, _, _, _) in p1.iter() {
                                            if ent != target_entity && maybe_enemy.is_some() {
                                                let dist = tgp.x.abs_diff(gp.x) + tgp.y.abs_diff(gp.y);
                                                if dist <= 1 {
                                                    targets.push(ent);
                                                }
                                            }
                                        }
                                    }

                                    for &t in &targets {
                                        let t_name = {
                                            let p2 = token_set.p2();
                                            p2.get(t).ok().and_then(|(_, _, _, _, name)| name).map(|n| get_clean_name(Some(n), "Enemy")).unwrap_or_else(|| "Enemy".to_string())
                                        };
                                        let (is_hit, is_crit, amount, enemy_died) = {
                                            let mut p0 = token_set.p0();
                                            if let Ok(mut enemy_combat) = p0.get_mut(t)
                                                && enemy_combat.hp > 0 {
                                                    let lucky = traits.0.contains(&HeroTrait::Lucky);
                                                    let luck_bonus = if lucky { 3 } else { 0 };
                                                    let leader_bonus = if has_leader_in_room { 2 } else { 0 };
     
                                                    let d20 = rng.random_range(1..=20);
                                                    let roll = d20 + active_combat.attack + luck_bonus + leader_bonus;
                                                    let target_ac = enemy_combat.defense + 10;
                                                    let is_hit = roll >= target_ac;
                                                    let is_crit = d20 == 20;
     
                                                    let mut amount = if is_hit {
                                                        let base_dmg = (active_combat.attack * 2 + rng.random_range(5..=15)).max(1);
                                                        if is_crit { base_dmg * 2 } else { base_dmg }
                                                    } else {
                                                        0
                                                    };
                                                    if traits.0.contains(&HeroTrait::Loner) && is_near_ally {
                                                        amount = (amount as f32 * 0.9).floor() as i32;
                                                     }
     
                                                     if is_hit {
                                                         enemy_combat.hp -= amount;
                                                     }
                                                     (is_hit, is_crit, amount, is_hit && enemy_combat.hp <= 0)
                                             } else {
                                                 (false, false, 0, false)
                                             }
                                         };

                                         if is_hit {
                                             if let Some(eq) = active_equipment {
                                                 if eq.has_affix(crate::equipment::BehavioralAffix::Lifesteal) {
                                                     let heal_amt = (amount as f32 * 0.3).round() as i32;
                                                     if heal_amt > 0 {
                                                         let mut p0 = token_set.p0();
                                                         if let Ok(mut active_stats) = p0.get_mut(entity) {
                                                             active_stats.hp = (active_stats.hp + heal_amt).min(active_stats.max_hp);
                                                         }
                                                         log_writer.write(MissionLogEvent {
                                                             mission_entity: _mission_entity,
                                                             payload: MissionLogPayload::Heal {
                                                                 healer: active_name.clone(),
                                                                 target: active_name.clone(),
                                                                 amount: heal_amt,
                                                             },
                                                         });
                                                     }
                                                 }
                                             }
                                         }

                                         if is_hit || amount == 0 {
                                             commands.entity(t).insert(VisualHit {
                                                 amount,
                                                 is_hit,
                                                 is_crit,
                                                 effect_type: "Damage".to_string(),
                                                 source: Some(entity),
                                                 is_signature: true,
                                             });

                                             log_writer.write(MissionLogEvent {
                                                 mission_entity: _mission_entity,
                                                 payload: MissionLogPayload::Ability {
                                                     attacker: active_name.clone(),
                                                     defender: t_name.clone(),
                                                     ability_name: ability_def.name.clone(),
                                                     amount,
                                                     is_hit,
                                                     is_crit,
                                                     effect_type: "Damage".to_string(),
                                                 },
                                             });

                                             if enemy_died {
                                                 log_writer.write(MissionLogEvent {
                                                     mission_entity: _mission_entity,
                                                     payload: MissionLogPayload::Death {
                                                         name: t_name,
                                                         is_enemy: true,
                                                     },
                                                 });
                                             }
                                         }
                                     }
                                } else if ability_id == "Mass Heal" {
                                    let mut targets = vec![entity];
                                    let caster_gp = {
                                        if let Ok((_, _, gp, _, _, _, _, _)) = token_set.p1().get(entity) {
                                            Some(*gp)
                                        } else {
                                            None
                                        }
                                    };
                                    if let Some(caster_gp) = caster_gp {
                                        let p1 = token_set.p1();
                                        for (ent, _, gp, _, _, maybe_hero, _, _) in p1.iter() {
                                            if ent != entity && maybe_hero.is_some() {
                                                let dist = caster_gp.x.abs_diff(gp.x) + caster_gp.y.abs_diff(gp.y);
                                                if dist <= 4 {
                                                    targets.push(ent);
                                                }
                                            }
                                        }
                                    }

                                    for &t in &targets {
                                        let mut p0 = token_set.p0();
                                        if let Ok(mut ally_combat) = p0.get_mut(t)
                                            && ally_combat.hp > 0 {
                                                let mut amount = ally_combat.max_hp - ally_combat.hp;
                                                if traits.0.contains(&HeroTrait::Loner) && is_near_ally {
                                                    amount = (amount as f32 * 0.9).floor() as i32;
                                                }
                                                if modifiers.contains(&super::data::MissionModifier::CursedGround) {
                                                    amount = 0;
                                                }
                                                ally_combat.hp = (ally_combat.hp + amount).min(ally_combat.max_hp);

                                                commands.entity(t).insert(VisualHit {
                                                     amount,
                                                     is_hit: true,
                                                     is_crit: false,
                                                     effect_type: "Heal".to_string(),
                                                     source: Some(entity),
                                                     is_signature: true,
                                                 });

                                                let t_name = {
                                                    let p3 = token_set.p3();
                                                    p3.get(t).ok().and_then(|(_, _, _, _, name)| name).map(|n| get_clean_name(Some(n), "Hero")).unwrap_or_else(|| "Hero".to_string())
                                                };
                                                log_writer.write(MissionLogEvent {
                                                    mission_entity: _mission_entity,
                                                    payload: MissionLogPayload::Ability {
                                                        attacker: active_name.clone(),
                                                        defender: t_name,
                                                        ability_name: ability_def.name.clone(),
                                                        amount,
                                                        is_hit: true,
                                                        is_crit: false,
                                                        effect_type: "Heal".to_string(),
                                                    },
                                                });
                                            }
                                    }
                                } else if ability_id == "Volley" {
                                    let active_equipment = hero_equip_query.get(hero_roster_entity).ok();
                                    let target_gp = {
                                        let p1 = token_set.p1();
                                        p1.get(target_entity).map(|(_, _, gp, _, _, _, _, _)| *gp).ok()
                                    };
                                    let mut targets = vec![target_entity];
                                    if let Some(tgp) = target_gp {
                                        let p1 = token_set.p1();
                                        for (ent, maybe_enemy, gp, _, _, _, _, _) in p1.iter() {
                                            if ent != target_entity && maybe_enemy.is_some() {
                                                let dist = tgp.x.abs_diff(gp.x) + tgp.y.abs_diff(gp.y);
                                                if dist <= 2 {
                                                    targets.push(ent);
                                                }
                                            }
                                        }
                                    }

                                    for &t in &targets {
                                         let t_name = {
                                             let p2 = token_set.p2();
                                             p2.get(t).ok().and_then(|(_, _, _, _, name)| name).map(|n| get_clean_name(Some(n), "Enemy")).unwrap_or_else(|| "Enemy".to_string())
                                         };
                                         let (is_hit, is_crit, amount, enemy_died) = {
                                             let mut p0 = token_set.p0();
                                             if let Ok(mut enemy_combat) = p0.get_mut(t)
                                                 && enemy_combat.hp > 0 {
                                                     let lucky = traits.0.contains(&HeroTrait::Lucky);
                                                     let luck_bonus = if lucky { 3 } else { 0 };
                                                     let leader_bonus = if has_leader_in_room { 2 } else { 0 };
     
                                                     let d20 = rng.random_range(1..=20);
                                                     let roll = d20 + active_combat.attack + luck_bonus + leader_bonus;
                                                     let target_ac = enemy_combat.defense + 10;
                                                     let is_hit = roll >= target_ac;
                                                     let is_crit = d20 == 20;
     
                                                     let mut amount = if is_hit {
                                                         let base_dmg = (active_combat.attack + rng.random_range(3..=10)).max(1);
                                                         if is_crit { base_dmg * 2 } else { base_dmg }
                                                     } else {
                                                         0
                                                     };
                                                     if traits.0.contains(&HeroTrait::Loner) && is_near_ally {
                                                         amount = (amount as f32 * 0.9).floor() as i32;
                                                     }
     
                                                     if is_hit {
                                                         enemy_combat.hp -= amount;
                                                     }
                                                     (is_hit, is_crit, amount, is_hit && enemy_combat.hp <= 0)
                                             } else {
                                                 (false, false, 0, false)
                                             }
                                         };

                                         if is_hit {
                                             if let Some(eq) = active_equipment {
                                                 if eq.has_affix(crate::equipment::BehavioralAffix::Lifesteal) {
                                                     let heal_amt = (amount as f32 * 0.3).round() as i32;
                                                     if heal_amt > 0 {
                                                         let mut p0 = token_set.p0();
                                                         if let Ok(mut active_stats) = p0.get_mut(entity) {
                                                             active_stats.hp = (active_stats.hp + heal_amt).min(active_stats.max_hp);
                                                         }
                                                         log_writer.write(MissionLogEvent {
                                                             mission_entity: _mission_entity,
                                                             payload: MissionLogPayload::Heal {
                                                                 healer: active_name.clone(),
                                                                 target: active_name.clone(),
                                                                 amount: heal_amt,
                                                             },
                                                         });
                                                     }
                                                 }
                                             }
                                         }

                                         if is_hit || amount == 0 {
                                             commands.entity(t).insert(VisualHit {
                                                 amount,
                                                 is_hit,
                                                 is_crit,
                                                 effect_type: "Damage".to_string(),
                                                 source: Some(entity),
                                                 is_signature: true,
                                             });

                                             log_writer.write(MissionLogEvent {
                                                 mission_entity: _mission_entity,
                                                 payload: MissionLogPayload::Ability {
                                                     attacker: active_name.clone(),
                                                     defender: t_name.clone(),
                                                     ability_name: ability_def.name.clone(),
                                                     amount,
                                                     is_hit,
                                                     is_crit,
                                                     effect_type: "Damage".to_string(),
                                                 },
                                             });

                                             if enemy_died {
                                                 log_writer.write(MissionLogEvent {
                                                     mission_entity: _mission_entity,
                                                     payload: MissionLogPayload::Death {
                                                         name: t_name,
                                                         is_enemy: true,
                                                     },
                                                 });
                                             }
                                         }
                                     }
                                } else {
                                    // Default/standard ability execution
                                    let active_equipment = hero_equip_query.get(hero_roster_entity).ok();
                                    let mut amount = 0;
                                    let mut is_hit = true;
                                    let mut is_crit = false;
                                    let target_name = {
                                        if let Ok((_, _, _, _, name)) = token_set.p2().get(target_entity) {
                                            name.map(|n| get_clean_name(Some(n), "Enemy")).unwrap_or_else(|| "Enemy".to_string())
                                        } else if let Ok((_, _, _, _, name)) = token_set.p3().get(target_entity) {
                                            name.map(|n| get_clean_name(Some(n), "Hero")).unwrap_or_else(|| "Hero".to_string())
                                        } else if target_entity == entity {
                                            active_name.clone()
                                        } else {
                                            "Target".to_string()
                                        }
                                    };

                                    match ability_def.effect {
                                        crate::hero::data::AbilityEffect::Damage | crate::hero::data::AbilityEffect::Debuff => {
                                            let mut p0 = token_set.p0();
                                            if let Ok(mut enemy_combat) = p0.get_mut(target_entity)
                                                && enemy_combat.hp > 0 {
                                                    let lucky = traits.0.contains(&HeroTrait::Lucky);
                                                    let luck_bonus = if lucky { 3 } else { 0 };
                                                    let leader_bonus = if has_leader_in_room { 2 } else { 0 };

                                                    let d20 = rng.random_range(1..=20);
                                                    let roll = d20 + active_combat.attack + luck_bonus + leader_bonus;
                                                    let target_ac = enemy_combat.defense + 10;
                                                    is_hit = roll >= target_ac;
                                                    is_crit = d20 == 20;

                                                    if is_hit {
                                                        let base_dmg = (active_combat.attack + rng.random_range(1..=4)).max(1);
                                                        let mut final_dmg = if is_crit { base_dmg * 2 } else { base_dmg };
                                                        if traits.0.contains(&HeroTrait::Loner) && is_near_ally {
                                                            final_dmg = (final_dmg as f32 * 0.9).floor() as i32;
                                                        }
                                                        amount = final_dmg;
                                                        enemy_combat.hp -= final_dmg;

                                                        if let Some(eq) = active_equipment {
                                                            if eq.has_affix(crate::equipment::BehavioralAffix::Lifesteal) {
                                                                let heal_amt = (final_dmg as f32 * 0.3).round() as i32;
                                                                if heal_amt > 0 {
                                                                    let mut p0 = token_set.p0();
                                                                    if let Ok(mut active_stats) = p0.get_mut(entity) {
                                                                        active_stats.hp = (active_stats.hp + heal_amt).min(active_stats.max_hp);
                                                                    }
                                                                    log_writer.write(MissionLogEvent {
                                                                        mission_entity: _mission_entity,
                                                                        payload: MissionLogPayload::Heal {
                                                                            healer: active_name.clone(),
                                                                            target: active_name.clone(),
                                                                            amount: heal_amt,
                                                                        },
                                                                    });
                                                                }
                                                            }
                                                        }
                                                    }
                                                    commands.entity(target_entity).insert(VisualHit {
                                                         amount,
                                                         is_hit,
                                                         is_crit,
                                                         effect_type: "Damage".to_string(),
                                                         source: Some(entity),
                                                         is_signature: false,
                                                     });
                                                }
                                        }
                                        crate::hero::data::AbilityEffect::Heal => {
                                            let mut p0 = token_set.p0();
                                            if let Ok(mut ally_combat) = p0.get_mut(target_entity)
                                                && ally_combat.hp > 0 {
                                                    let leader_bonus = if has_leader_in_room { 2 } else { 0 };
                                                    let mut heal = rng.random_range(5..=15) + (active_combat.attack / 2) + leader_bonus;
                                                    if traits.0.contains(&HeroTrait::Loner) && is_near_ally {
                                                        heal = (heal as f32 * 0.9).floor() as i32;
                                                    }
                                                    ally_combat.hp = (ally_combat.hp + heal).min(ally_combat.max_hp);
                                                    amount = heal;

                                                    commands.entity(target_entity).insert(VisualHit {
                                                         amount,
                                                         is_hit: true,
                                                         is_crit: false,
                                                         effect_type: "Heal".to_string(),
                                                         source: Some(entity),
                                                         is_signature: false,
                                                     });
                                                }
                                        }
                                        crate::hero::data::AbilityEffect::Shield | crate::hero::data::AbilityEffect::Buff => {
                                            let mut p0 = token_set.p0();
                                            if let Ok(mut ally_combat) = p0.get_mut(target_entity)
                                                && ally_combat.hp > 0 {
                                                    let leader_bonus = if has_leader_in_room { 2 } else { 0 };
                                                    let mut shield = active_combat.defense + rng.random_range(3..=8) + leader_bonus;
                                                    if traits.0.contains(&HeroTrait::Loner) && is_near_ally {
                                                        shield = (shield as f32 * 0.9).floor() as i32;
                                                    }
                                                    ally_combat.hp += shield;
                                                    amount = shield;
                                                    commands.entity(target_entity).insert(VisualHit {
                                                         amount,
                                                         is_hit: true,
                                                         is_crit: false,
                                                         effect_type: "Shield".to_string(),
                                                         source: Some(entity),
                                                         is_signature: false,
                                                     });
                                                }
                                        }
                                    }

                                    log_writer.write(MissionLogEvent {
                                        mission_entity: _mission_entity,
                                        payload: MissionLogPayload::Ability {
                                            attacker: active_name.clone(),
                                            defender: target_name.clone(),
                                            ability_name: ability_def.name.clone(),
                                            amount,
                                            is_hit,
                                            is_crit,
                                            effect_type: format!("{:?}", ability_def.effect),
                                        },
                                    });

                                    if (ability_def.effect == crate::hero::data::AbilityEffect::Damage
                                        || ability_def.effect == crate::hero::data::AbilityEffect::Debuff)
                                        && is_hit
                                    {
                                        let p0 = token_set.p0();
                                        if let Ok(enemy_combat) = p0.get(target_entity)
                                            && enemy_combat.hp <= 0 {
                                                log_writer.write(MissionLogEvent {
                                                    mission_entity: _mission_entity,
                                                    payload: MissionLogPayload::Death {
                                                        name: target_name,
                                                        is_enemy: true,
                                                    },
                                                });
                                            }
                                    }
                                }

                                {
                                    let mut p5 = token_set.p5();
                                    if let Ok(mut mut_abils) = p5.get_mut(entity)
                                        && let Some(ab) = mut_abils.abilities.iter_mut().find(|a| a.ability_id == *ability_id) {
                                            ab.remaining_cooldown = ability_def.cooldown;
                                        }
                                }
                            }
                        }
                        HeroAction::OpenChest(chest_entity) => {
                            let mut p6 = token_set.p6();
                            if let Ok((_, mut chest, _, _)) = p6.get_mut(chest_entity) {
                                if !chest.opened {
                                    chest.opened = true;
                                    let gold_amount = chest.gold_reward;
                                    if let Some(gold) = gold_res.as_mut() {
                                        gold.0 += gold_amount;
                                    }
                                    log_writer.write(MissionLogEvent {
                                        mission_entity: _mission_entity,
                                        payload: MissionLogPayload::ChestOpened {
                                            hero_name: active_name.clone(),
                                            gold: gold_amount,
                                        },
                                    });

                                    // Roll for gear drop (e.g. 50% chance)
                                    if let Some(equipment_db) = equipment_db.as_ref() {
                                        if rng.random_bool(0.5) {
                                            let slot = match rng.random_range(0..3) {
                                                0 => crate::equipment::GearSlot::Weapon,
                                                1 => crate::equipment::GearSlot::Armor,
                                                _ => crate::equipment::GearSlot::Accessory,
                                            };

                                            // Roll rarity
                                            let rarity_roll = rng.random_range(0..100);
                                            let rarity = if rarity_roll < 50 {
                                                crate::equipment::GearRarity::Common
                                            } else if rarity_roll < 75 {
                                                crate::equipment::GearRarity::Uncommon
                                            } else if rarity_roll < 90 {
                                                crate::equipment::GearRarity::Rare
                                            } else if rarity_roll < 98 {
                                                crate::equipment::GearRarity::Epic
                                            } else {
                                                crate::equipment::GearRarity::Legendary
                                            };

                                            // Roll affix
                                            let affix = if rarity >= crate::equipment::GearRarity::Rare {
                                                let affix_roll = rng.random_range(0..3);
                                                Some(match affix_roll {
                                                    0 => crate::equipment::BehavioralAffix::Lifesteal,
                                                    1 => crate::equipment::BehavioralAffix::Initiative,
                                                    _ => crate::equipment::BehavioralAffix::CleaveOnHit,
                                                })
                                            } else {
                                                None
                                            };

                                            if let Ok(mut roster_equip) = hero_equip_query.get_mut(hero_roster_entity) {
                                                let current_tier = roster_equip.tier(slot);
                                                let current_rarity = match slot {
                                                    crate::equipment::GearSlot::Weapon => roster_equip.weapon_rarity,
                                                    crate::equipment::GearSlot::Armor => roster_equip.armor_rarity,
                                                    crate::equipment::GearSlot::Accessory => roster_equip.accessory_rarity,
                                                };
                                                
                                                if rarity >= current_rarity {
                                                    let tier_to_use = if current_tier == 0 { 1 } else { current_tier };
                                                    match slot {
                                                        crate::equipment::GearSlot::Weapon => {
                                                            roster_equip.weapon_rarity = rarity;
                                                            roster_equip.weapon_affix = affix;
                                                            if roster_equip.weapon_tier == 0 {
                                                                roster_equip.weapon_tier = 1;
                                                            }
                                                        }
                                                        crate::equipment::GearSlot::Armor => {
                                                            roster_equip.armor_rarity = rarity;
                                                            roster_equip.armor_affix = affix;
                                                            if roster_equip.armor_tier == 0 {
                                                                roster_equip.armor_tier = 1;
                                                            }
                                                        }
                                                        crate::equipment::GearSlot::Accessory => {
                                                            roster_equip.accessory_rarity = rarity;
                                                            roster_equip.accessory_affix = affix;
                                                            if roster_equip.accessory_tier == 0 {
                                                                roster_equip.accessory_tier = 1;
                                                            }
                                                        }
                                                    }

                                                    // Recalculate stats for the token in the simulation
                                                    let (new_hp, new_attack, new_defense) = calculate_hero_token_stats(
                                                        info,
                                                        stats,
                                                        Some(&roster_equip),
                                                        maybe_fatigue,
                                                        is_injured,
                                                        equipment_db,
                                                    );

                                                    let mut p0 = token_set.p0();
                                                    if let Ok(mut active_stats) = p0.get_mut(entity) {
                                                        let hp_diff = new_hp - active_stats.max_hp;
                                                        active_stats.max_hp = new_hp;
                                                        active_stats.hp = (active_stats.hp + hp_diff).max(1);
                                                        active_stats.attack = new_attack;
                                                        active_stats.defense = new_defense;
                                                    }

                                                    let item_name = if let Some(path) = equipment_db.get_path(info.class, slot) {
                                                        if let Some(tier_def) = path.tiers.get((tier_to_use - 1) as usize) {
                                                            tier_def.name.clone()
                                                        } else {
                                                            "Gear".to_string()
                                                        }
                                                    } else {
                                                        "Gear".to_string()
                                                    };

                                                    let stats_desc = format!("+{} Atk, +{} Def, +{} HP", 
                                                        (if let Some(path) = equipment_db.get_path(info.class, slot) {
                                                            path.tiers.get((tier_to_use - 1) as usize).map(|td| td.stats.attack).unwrap_or(0)
                                                        } else { 0 } as f32 * rarity.stat_multiplier()).round() as i32,
                                                        (if let Some(path) = equipment_db.get_path(info.class, slot) {
                                                            path.tiers.get((tier_to_use - 1) as usize).map(|td| td.stats.defense).unwrap_or(0)
                                                        } else { 0 } as f32 * rarity.stat_multiplier()).round() as i32,
                                                        (if let Some(path) = equipment_db.get_path(info.class, slot) {
                                                            path.tiers.get((tier_to_use - 1) as usize).map(|td| td.stats.hp).unwrap_or(0)
                                                        } else { 0 } as f32 * rarity.stat_multiplier()).round() as i32,
                                                    );

                                                    log_writer.write(MissionLogEvent {
                                                        mission_entity: _mission_entity,
                                                        payload: MissionLogPayload::GearDrop {
                                                            hero_name: active_name.clone(),
                                                            item_name,
                                                            rarity,
                                                            affix,
                                                            stats_desc,
                                                        },
                                                    });
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        HeroAction::Hold => {}
                    }

                    commands.entity(entity).insert(action);
                }
            }

            // Action resolved, consume tick
            turn_queue.active_index += 1;
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;
    use crate::hero::data::HeroClass;
    use crate::mission::dungeon::{generate_dungeon, RoomType};
    use crate::mission::entities::ActiveAbilityState;
    use crate::mission::MissionDungeon;

    #[test]
    fn freshly_dispatched_mission_gets_a_turn_queue() {
        // Regression: dispatch_mission spawns missions WITHOUT MissionTurnQueue
        // (only the save-load path added it), so fresh missions never ticked.
        // build_or_update_turn_queue must self-heal the missing component.
        let mut world = World::new();

        let mission_ent = world.spawn(Mission).id(); // no MissionTurnQueue — like dispatch
        let c1 = world
            .spawn(CombatStats { hp: 10, max_hp: 10, attack: 5, defense: 5, speed: 10 })
            .id();
        world.entity_mut(mission_ent).add_child(c1);

        // First run inserts the component; second run populates the queue.
        let _ = world.run_system_once(build_or_update_turn_queue);
        assert!(
            world.get::<MissionTurnQueue>(mission_ent).is_some(),
            "turn queue component should be inserted for queue-less missions"
        );
        let _ = world.run_system_once(build_or_update_turn_queue);
        let queue = world.get::<MissionTurnQueue>(mission_ent).unwrap();
        assert_eq!(queue.queue.len(), 1);
    }

    #[test]
    fn test_initiative_sorting() {
        let mut world = World::new();

        let mission_ent = world.spawn((
            Mission,
            MissionTurnQueue::default(),
        )).id();

        // With speeds separated by more than 20 (the max d20 roll),
        // the initiative order is mathematically guaranteed to follow the speed ordering:
        // c2 (100) > c3 (50) > c1 (10)
        let c1 = world.spawn((
            CombatStats { hp: 10, max_hp: 10, attack: 5, defense: 5, speed: 10 },
        )).id();
        let c2 = world.spawn((
            CombatStats { hp: 10, max_hp: 10, attack: 5, defense: 5, speed: 100 },
        )).id();
        let c3 = world.spawn((
            CombatStats { hp: 10, max_hp: 10, attack: 5, defense: 5, speed: 50 },
        )).id();

        world.entity_mut(mission_ent).add_child(c1);
        world.entity_mut(mission_ent).add_child(c2);
        world.entity_mut(mission_ent).add_child(c3);

        println!("DEBUG TEST INITIATIVE: Children on mission_ent = {:?}", world.get::<Children>(mission_ent));

        let _ = world.run_system_once(build_or_update_turn_queue);

        let queue = world.get::<MissionTurnQueue>(mission_ent).unwrap();
        assert_eq!(queue.queue.len(), 3);
        assert_eq!(queue.queue[0], c2);
        assert_eq!(queue.queue[1], c3);
        assert_eq!(queue.queue[2], c1);
    }

    #[test]
    fn test_deterministic_tie_breaking() {
        let mut world = World::new();

        let mission_ent = world.spawn((
            Mission,
            MissionTurnQueue::default(),
        )).id();

        let mut children = Vec::new();
        for _ in 0..10 {
            let c = world.spawn((
                CombatStats { hp: 10, max_hp: 10, attack: 5, defense: 5, speed: 10 },
            )).id();
            world.entity_mut(mission_ent).add_child(c);
            children.push(c);
        }

        // Build the queue once
        let _ = world.run_system_once(build_or_update_turn_queue);
        let q1 = world.get::<MissionTurnQueue>(mission_ent).unwrap().queue.clone();

        // Reset queue and round count to ensure identical seeding
        world.entity_mut(mission_ent).entry::<MissionTurnQueue>().and_modify(|mut q| {
            q.queue.clear();
            q.active_index = 0;
            q.round_count = 0;
        });

        let _ = world.run_system_once(build_or_update_turn_queue);
        let q2 = world.get::<MissionTurnQueue>(mission_ent).unwrap().queue.clone();

        assert_eq!(q1, q2);

        // Run next round (different round count)
        world.entity_mut(mission_ent).entry::<MissionTurnQueue>().and_modify(|mut q| {
            q.queue.clear();
            q.active_index = 0;
        });

        let _ = world.run_system_once(build_or_update_turn_queue);
        let q3 = world.get::<MissionTurnQueue>(mission_ent).unwrap().queue.clone();

        assert_ne!(q1, q3);
    }

    #[test]
    fn test_per_round_reroll() {
        let mut world = World::new();

        let mission_ent = world.spawn((
            Mission,
            MissionTurnQueue::default(),
        )).id();

        // A small speed difference ensures that the d20 roll can change the turn order across rounds
        let c1 = world.spawn((
            CombatStats { hp: 10, max_hp: 10, attack: 5, defense: 5, speed: 10 },
        )).id();
        let c2 = world.spawn((
            CombatStats { hp: 10, max_hp: 10, attack: 5, defense: 5, speed: 11 },
        )).id();

        world.entity_mut(mission_ent).add_child(c1);
        world.entity_mut(mission_ent).add_child(c2);

        // We will run 10 rounds and check if the order changes at least once
        let mut orders = Vec::new();
        for round in 0..10 {
            world.entity_mut(mission_ent).entry::<MissionTurnQueue>().and_modify(|mut q| {
                q.queue.clear();
                q.active_index = 0;
                q.round_count = round;
            });

            let _ = world.run_system_once(build_or_update_turn_queue);
            let queue = world.get::<MissionTurnQueue>(mission_ent).unwrap().queue.clone();
            orders.push(queue);
        }

        // Check that we got at least one round where c1 went first, and one where c2 went first
        let c1_first = orders.iter().any(|q| q[0] == c1);
        let c2_first = orders.iter().any(|q| q[0] == c2);
        assert!(c1_first, "c1 should go first in at least one round");
        assert!(c2_first, "c2 should go first in at least one round");
    }

    #[test]
    fn test_one_tile_movement() {
        let mut world = World::new();

        // 1. Generate a dungeon map
        let mut rng = StdRng::seed_from_u64(42);
        let mut map = generate_dungeon(40, 30, 4, &mut rng);

        // Ensure we have an entrance and at least one other room
        let entrance_idx = map.rooms.iter().position(|r| r.room_type == RoomType::Entrance).unwrap();
        for (idx, room) in map.rooms.iter_mut().enumerate() {
            if idx != entrance_idx {
                room.room_type = RoomType::Normal;
            }
        }
        let target_idx = (0..map.rooms.len()).find(|&i| i != entrance_idx).unwrap();

        let entrance_room = &map.rooms[entrance_idx];
        let target_room = &map.rooms[target_idx];

        let start_pos = entrance_room.center();
        let target_pos = target_room.center();

        let path = find_path(&map, start_pos, target_pos).expect("Should find path between rooms");
        assert!(path.len() > 2, "Path should be long enough for testing movement");

        // 2. Set up RoomStatus (entrance room is visited, target room is not visited)
        let mut room_status = RoomStatus {
            visited: vec![false; map.rooms.len()],
            cleared: vec![false; map.rooms.len()],
        };
        room_status.visited[entrance_idx] = true;

        // 3. Create the roster hero entity
        let roster_hero = world.spawn((
            Hero,
            HeroInfo {
                name: "Test Hero".to_string(),
                class: HeroClass::Warrior,
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
            HeroTraits(vec![]),
        )).id();

        // 4. Create the mission entity with the dungeon and room status
        let mut turn_queue = MissionTurnQueue::default();
        let mission_ent = world.spawn((
            Mission,
            MissionDungeon(map.clone()),
            room_status,
        )).id();

        // 5. Create the hero token
        let token = world.spawn((
            HeroToken(roster_hero),
            GridPosition { x: start_pos.0, y: start_pos.1 },
            InRoom(Some(entrance_idx)),
            MoveRange {
                base: 4,
                bonus: 0,
            },
            CombatStats { hp: 100, max_hp: 100, attack: 10, defense: 10, speed: 10 },
        )).id();

        world.entity_mut(mission_ent).add_child(token);

        // Put the token in the queue and set active index to 0
        turn_queue.queue = vec![token];
        turn_queue.active_index = 0;
        world.entity_mut(mission_ent).insert(turn_queue);

        // Initialize Messages resource so MessageWriter is resolvable
        world.init_resource::<bevy::ecs::message::Messages<MissionLogEvent>>();

        // 6. Run process_sequential_turn
        world.run_system_once(process_sequential_turn).unwrap();

        // 7. Verify token position moved exactly one tile (index 1 of path)
        let pos = world.get::<GridPosition>(token).unwrap();
        let expected_pos = path[1];
        assert_eq!((pos.x, pos.y), expected_pos);

        // Verify VisualPathHistory has exactly one waypoint (the one step)
        let history = world.get::<VisualPathHistory>(token).unwrap();
        assert_eq!(history.waypoints.len(), 1);
        assert_eq!(history.waypoints[0], expected_pos);
    }

    #[test]
    fn test_tempo_split() {
        let mut world = World::new();

        world.insert_resource(Time::<Fixed>::from_hz(2.0));

        let mission_ent = world.spawn((
            Mission,
        )).id();

        world.insert_resource(crate::mission::ViewedMission(mission_ent));

        // Spawn roster hero to get Mage class info
        let roster_hero = world.spawn((
            Hero,
            HeroInfo {
                name: "Mage Hero".to_string(),
                class: HeroClass::Mage,
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
            HeroTraits(vec![]),
        )).id();

        let hero_token = world.spawn((
            HeroToken(roster_hero),
            GridPosition { x: 0, y: 0 },
            InRoom(None),
            CombatStats { hp: 100, max_hp: 100, attack: 10, defense: 10, speed: 10 },
        )).id();

        let enemy_token = world.spawn((
            EnemyToken {
                enemy_type: crate::mission::data::EnemyType::Goblin,
                xp_reward: 10,
            },
            GridPosition { x: 0, y: 6 }, // Out of range (distance 6 > Mage range 5)
            InRoom(None),
            CombatStats { hp: 10, max_hp: 10, attack: 5, defense: 5, speed: 10 },
        )).id();

        world.entity_mut(mission_ent).add_child(hero_token);
        world.entity_mut(mission_ent).add_child(enemy_token);

        // Run the tempo system
        let _ = world.run_system_once(crate::mission::update_simulation_tempo);

        let time = world.resource::<Time<Fixed>>();
        assert_eq!(time.timestep(), std::time::Duration::from_secs_f32(1.0 / 6.0)); // Should be exploration speed

        // Move enemy in range (distance 5 <= Mage range 5)
        world.entity_mut(enemy_token).insert(GridPosition { x: 0, y: 5 });

        // Run the tempo system again
        let _ = world.run_system_once(crate::mission::update_simulation_tempo);

        let time = world.resource::<Time<Fixed>>();
        assert_eq!(time.timestep(), std::time::Duration::from_secs_f32(1.0 / 1.5)); // Should be combat speed
    }

    #[test]
    fn test_cooldown_abilities() {
        let mut world = World::new();

        // 1. Setup AbilityDatabase
        let abilities_str = include_str!("../../assets/data/abilities.ron");
        let abilities: Vec<crate::hero::data::AbilityDef> =
            ron::from_str(abilities_str).expect("Failed to parse abilities.ron");
        world.insert_resource(crate::hero::data::AbilityDatabase(abilities));

        // 2. Setup Mission and Token
        let mission_ent = world.spawn((
            Mission,
            MissionDungeon(generate_dungeon(40, 30, 2, &mut rand::rng())),
            RoomStatus {
                visited: vec![true; 5],
                cleared: vec![false; 5],
            },
            MissionTurnQueue::default(),
        )).id();

        let roster_hero = world.spawn((
            Hero,
            HeroInfo {
                name: "Test Warrior".to_string(),
                class: HeroClass::Warrior,
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
            HeroTraits(vec![]),
        )).id();

        // Spawn hero token with Slash ability
        let token = world.spawn((
            HeroToken(roster_hero),
            GridPosition { x: 5, y: 5 },
            InRoom(Some(0)),
            CombatStats { hp: 10, max_hp: 10, attack: 10, defense: 10, speed: 10 },
            ActiveAbilities {
                abilities: vec![ActiveAbilityState {
                    ability_id: "Slash".to_string(),
                    remaining_cooldown: 0,
                }],
            },
        )).id();

        // Spawn adjacent enemy so Slash can be used (range 1)
        let enemy = world.spawn((
            EnemyToken {
                enemy_type: crate::mission::data::EnemyType::Goblin,
                xp_reward: 10,
            },
            GridPosition { x: 5, y: 6 },
            InRoom(Some(0)),
            CombatStats { hp: 100, max_hp: 100, attack: 5, defense: 5, speed: 5 },
        )).id();

        world.entity_mut(mission_ent).add_child(token);
        world.entity_mut(mission_ent).add_child(enemy);

        // Put token in turn queue
        world.entity_mut(mission_ent).entry::<MissionTurnQueue>().and_modify(|mut q| {
            q.queue = vec![token];
            q.active_index = 0;
        });

        // Initialize Messages resource
        world.init_resource::<bevy::ecs::message::Messages<MissionLogEvent>>();

        // Run sequential turn process (this should execute Slash)
        world.run_system_once(process_sequential_turn).unwrap();

        // Verify that Slash was used and cooldown was set to 2
        let abils = world.get::<ActiveAbilities>(token).unwrap();
        assert_eq!(abils.abilities[0].remaining_cooldown, 2);

        // Run again. At the start of the next turn, cooldown should decrement to 1
        world.entity_mut(mission_ent).entry::<MissionTurnQueue>().and_modify(|mut q| {
            q.active_index = 0;
        });
        world.run_system_once(process_sequential_turn).unwrap();
        let abils = world.get::<ActiveAbilities>(token).unwrap();
        assert_eq!(abils.abilities[0].remaining_cooldown, 1);
    }

    #[test]
    fn test_ability_priority_heal() {
        let mut world = World::new();

        // Load database
        let abilities_str = include_str!("../../assets/data/abilities.ron");
        let abilities: Vec<crate::hero::data::AbilityDef> =
            ron::from_str(abilities_str).expect("Failed to parse abilities.ron");
        world.insert_resource(crate::hero::data::AbilityDatabase(abilities));

        let mission_ent = world.spawn((
            Mission,
            MissionDungeon(generate_dungeon(40, 30, 2, &mut rand::rng())),
            RoomStatus {
                visited: vec![true; 5],
                cleared: vec![false; 5],
            },
            MissionTurnQueue::default(),
        )).id();

        let roster_hero = world.spawn((
            Hero,
            HeroInfo {
                name: "Test Cleric".to_string(),
                class: HeroClass::Cleric,
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
            HeroTraits(vec![]),
        )).id();

        let token = world.spawn((
            HeroToken(roster_hero),
            GridPosition { x: 5, y: 5 },
            InRoom(Some(0)),
            CombatStats { hp: 10, max_hp: 10, attack: 10, defense: 10, speed: 10 },
            ActiveAbilities {
                abilities: vec![ActiveAbilityState {
                    ability_id: "Heal".to_string(),
                    remaining_cooldown: 0,
                }],
            },
        )).id();

        // Spawn a wounded ally (HP 5/10, which is 50%, below the 70% threshold for Heal)
        let ally = world.spawn((
            HeroToken(roster_hero),
            GridPosition { x: 6, y: 5 },
            InRoom(Some(0)),
            CombatStats { hp: 5, max_hp: 10, attack: 10, defense: 10, speed: 10 },
        )).id();

        // Spawn an enemy so we are in combat
        let enemy = world.spawn((
            EnemyToken {
                enemy_type: crate::mission::data::EnemyType::Goblin,
                xp_reward: 10,
            },
            GridPosition { x: 5, y: 6 },
            InRoom(Some(0)),
            CombatStats { hp: 100, max_hp: 100, attack: 5, defense: 5, speed: 5 },
        )).id();

        world.entity_mut(mission_ent).add_child(token);
        world.entity_mut(mission_ent).add_child(ally);
        world.entity_mut(mission_ent).add_child(enemy);

        // Put token in turn queue
        world.entity_mut(mission_ent).entry::<MissionTurnQueue>().and_modify(|mut q| {
            q.queue = vec![token];
            q.active_index = 0;
        });

        // Initialize Messages resource
        world.init_resource::<bevy::ecs::message::Messages<MissionLogEvent>>();

        // Run sequential turn process (this should cast Heal on the ally)
        world.run_system_once(process_sequential_turn).unwrap();

        // Verify that Heal was cast (ally HP increased, and Cleric cooldown set to 3)
        let abils = world.get::<ActiveAbilities>(token).unwrap();
        assert_eq!(abils.abilities[0].remaining_cooldown, 3);
        let ally_hp = world.get::<CombatStats>(ally).unwrap().hp;
        assert!(ally_hp > 5);
    }

    #[test]
    fn test_ability_priority_fireball() {
        let mut world = World::new();

        // Load database
        let abilities_str = include_str!("../../assets/data/abilities.ron");
        let abilities: Vec<crate::hero::data::AbilityDef> =
            ron::from_str(abilities_str).expect("Failed to parse abilities.ron");
        world.insert_resource(crate::hero::data::AbilityDatabase(abilities));

        let mission_ent = world.spawn((
            Mission,
            MissionDungeon(generate_dungeon(40, 30, 2, &mut rand::rng())),
            RoomStatus {
                visited: vec![true; 5],
                cleared: vec![false; 5],
            },
            MissionTurnQueue::default(),
        )).id();

        let roster_hero = world.spawn((
            Hero,
            HeroInfo {
                name: "Test Mage".to_string(),
                class: HeroClass::Mage,
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
            HeroTraits(vec![]),
        )).id();

        let token = world.spawn((
            HeroToken(roster_hero),
            GridPosition { x: 5, y: 5 },
            InRoom(Some(0)),
            CombatStats { hp: 10, max_hp: 10, attack: 10, defense: 10, speed: 10 },
            ActiveAbilities {
                abilities: vec![ActiveAbilityState {
                    ability_id: "Fireball".to_string(),
                    remaining_cooldown: 0,
                }],
            },
        )).id();

        // Fireball requires MultipleEnemiesAdjacent(2).
        // Let's spawn 2 enemies adjacent to each other.
        // Enemy 1 at (5, 8) - within Mage range 5
        let enemy1 = world.spawn((
            EnemyToken {
                enemy_type: crate::mission::data::EnemyType::Goblin,
                xp_reward: 10,
            },
            GridPosition { x: 5, y: 8 },
            InRoom(Some(0)),
            CombatStats { hp: 10, max_hp: 10, attack: 5, defense: 5, speed: 5 },
        )).id();

        // Enemy 2 at (5, 9) - adjacent to Enemy 1
        let enemy2 = world.spawn((
            EnemyToken {
                enemy_type: crate::mission::data::EnemyType::Goblin,
                xp_reward: 10,
            },
            GridPosition { x: 5, y: 9 },
            InRoom(Some(0)),
            CombatStats { hp: 10, max_hp: 10, attack: 5, defense: 5, speed: 5 },
        )).id();

        world.entity_mut(mission_ent).add_child(token);
        world.entity_mut(mission_ent).add_child(enemy1);
        world.entity_mut(mission_ent).add_child(enemy2);

        // Put token in turn queue
        world.entity_mut(mission_ent).entry::<MissionTurnQueue>().and_modify(|mut q| {
            q.queue = vec![token];
            q.active_index = 0;
        });

        // Initialize Messages resource
        world.init_resource::<bevy::ecs::message::Messages<MissionLogEvent>>();

        // Run sequential turn process (this should cast Fireball)
        world.run_system_once(process_sequential_turn).unwrap();

        // Verify that Fireball was cast (cooldown set to 4)
        let abils = world.get::<ActiveAbilities>(token).unwrap();
        assert_eq!(abils.abilities[0].remaining_cooldown, 4);
    }

    #[test]
    fn test_signature_warrior_rallying_cry() {
        let mut world = World::new();

        // Load database
        let abilities_str = include_str!("../../assets/data/abilities.ron");
        let abilities: Vec<crate::hero::data::AbilityDef> =
            ron::from_str(abilities_str).expect("Failed to parse abilities.ron");
        world.insert_resource(crate::hero::data::AbilityDatabase(abilities));

        let mission_ent = world.spawn((
            Mission,
            MissionDungeon(generate_dungeon(40, 30, 2, &mut rand::rng())),
            RoomStatus {
                visited: vec![true, false],
                cleared: vec![false, false],
            },
            MissionTurnQueue {
                combat_round_count: 1, // Combat Round 1 triggers Rallying Cry
                ..default()
            },
        )).id();

        let roster_hero = world.spawn((
            Hero,
            HeroInfo {
                name: "Test Warrior".to_string(),
                class: HeroClass::Warrior,
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
            HeroTraits(vec![]),
        )).id();

        let token = world.spawn((
            HeroToken(roster_hero),
            GridPosition { x: 5, y: 5 },
            InRoom(Some(0)),
            CombatStats { hp: 10, max_hp: 20, attack: 10, defense: 10, speed: 10 },
            ActiveAbilities {
                abilities: vec![ActiveAbilityState {
                    ability_id: "Rallying Cry".to_string(),
                    remaining_cooldown: 0,
                }],
            },
        )).id();

        let ally = world.spawn((
            HeroToken(roster_hero),
            GridPosition { x: 6, y: 5 }, // adjacent
            InRoom(Some(0)),
            CombatStats { hp: 10, max_hp: 20, attack: 10, defense: 10, speed: 10 },
        )).id();

        let enemy = world.spawn((
            EnemyToken {
                enemy_type: crate::mission::data::EnemyType::Goblin,
                xp_reward: 10,
            },
            GridPosition { x: 5, y: 6 },
            InRoom(Some(0)),
            CombatStats { hp: 100, max_hp: 100, attack: 5, defense: 5, speed: 5 },
        )).id();

        world.entity_mut(mission_ent).add_child(token);
        world.entity_mut(mission_ent).add_child(ally);
        world.entity_mut(mission_ent).add_child(enemy);

        // Put token in turn queue
        world.entity_mut(mission_ent).entry::<MissionTurnQueue>().and_modify(|mut q| {
            q.queue = vec![token];
            q.active_index = 0;
        });

        // Initialize Messages resource
        world.init_resource::<bevy::ecs::message::Messages<MissionLogEvent>>();

        // Run sequential turn process (this should cast Rallying Cry)
        world.run_system_once(process_sequential_turn).unwrap();

        // Verify that Rallying Cry was cast (caster and ally HP boosted)
        let abils = world.get::<ActiveAbilities>(token).unwrap();
        assert_eq!(abils.abilities[0].remaining_cooldown, 999);

        let token_hp = world.get::<CombatStats>(token).unwrap().hp;
        let ally_hp = world.get::<CombatStats>(ally).unwrap().hp;
        assert!(token_hp > 10);
        assert!(ally_hp > 10);

        // Test room clear resets remaining cooldown to 0
        // Despawn enemy so room counts as cleared
        world.despawn(enemy);

        // Run room status update to trigger room cleared transition
        world.run_system_once(crate::mission::combat::update_room_status).unwrap();

        let abils = world.get::<ActiveAbilities>(token).unwrap();
        assert_eq!(abils.abilities[0].remaining_cooldown, 0);
    }

    #[test]
    fn test_signature_rogue_assassinate() {
        let mut world = World::new();

        let abilities_str = include_str!("../../assets/data/abilities.ron");
        let abilities: Vec<crate::hero::data::AbilityDef> =
            ron::from_str(abilities_str).expect("Failed to parse abilities.ron");
        world.insert_resource(crate::hero::data::AbilityDatabase(abilities));

        let mission_ent = world.spawn((
            Mission,
            MissionDungeon(generate_dungeon(40, 30, 2, &mut rand::rng())),
            RoomStatus {
                visited: vec![true],
                cleared: vec![false],
            },
            MissionTurnQueue {
                combat_round_count: 2, // Not round 1, so priority depends entirely on HP threshold
                ..default()
            },
        )).id();

        let roster_hero = world.spawn((
            Hero,
            HeroInfo {
                name: "Test Rogue".to_string(),
                class: HeroClass::Rogue,
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
            HeroTraits(vec![]),
        )).id();

        let token = world.spawn((
            HeroToken(roster_hero),
            GridPosition { x: 5, y: 5 },
            InRoom(Some(0)),
            CombatStats { hp: 10, max_hp: 10, attack: 10, defense: 10, speed: 10 },
            ActiveAbilities {
                abilities: vec![ActiveAbilityState {
                    ability_id: "Assassinate".to_string(),
                    remaining_cooldown: 0,
                }],
            },
        )).id();

        // Enemy at 100% HP (should NOT trigger Assassinate)
        let enemy = world.spawn((
            EnemyToken {
                enemy_type: crate::mission::data::EnemyType::Goblin,
                xp_reward: 10,
            },
            GridPosition { x: 5, y: 6 },
            InRoom(Some(0)),
            CombatStats { hp: 100, max_hp: 100, attack: 5, defense: 5, speed: 5 },
        )).id();

        world.entity_mut(mission_ent).add_child(token);
        world.entity_mut(mission_ent).add_child(enemy);

        world.entity_mut(mission_ent).entry::<MissionTurnQueue>().and_modify(|mut q| {
            q.queue = vec![token];
            q.active_index = 0;
        });

        world.init_resource::<bevy::ecs::message::Messages<MissionLogEvent>>();

        // Run turn. Since enemy HP is 100%, Assassinate is not triggered. Cooldown remains 0.
        world.run_system_once(process_sequential_turn).unwrap();
        let abils = world.get::<ActiveAbilities>(token).unwrap();
        assert_eq!(abils.abilities[0].remaining_cooldown, 0);

        // Lower enemy HP below 30% (e.g., 20/100)
        world.entity_mut(enemy).entry::<CombatStats>().and_modify(|mut cs| {
            cs.hp = 20;
        });

        // Reset turn active index
        world.entity_mut(mission_ent).entry::<MissionTurnQueue>().and_modify(|mut q| {
            q.active_index = 0;
        });

        // Run turn again. Assassinate should trigger and lock.
        world.run_system_once(process_sequential_turn).unwrap();
        let abils = world.get::<ActiveAbilities>(token).unwrap();
        assert_eq!(abils.abilities[0].remaining_cooldown, 999);
    }

    #[test]
    fn test_signature_mage_meteor() {
        let mut world = World::new();

        let abilities_str = include_str!("../../assets/data/abilities.ron");
        let abilities: Vec<crate::hero::data::AbilityDef> =
            ron::from_str(abilities_str).expect("Failed to parse abilities.ron");
        world.insert_resource(crate::hero::data::AbilityDatabase(abilities));

        let mission_ent = world.spawn((
            Mission,
            MissionDungeon(generate_dungeon(40, 30, 2, &mut rand::rng())),
            RoomStatus {
                visited: vec![true],
                cleared: vec![false],
            },
            MissionTurnQueue {
                combat_round_count: 2,
                ..default()
            },
        )).id();

        let roster_hero = world.spawn((
            Hero,
            HeroInfo {
                name: "Test Mage".to_string(),
                class: HeroClass::Mage,
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
            HeroTraits(vec![]),
        )).id();

        let token = world.spawn((
            HeroToken(roster_hero),
            GridPosition { x: 5, y: 5 },
            InRoom(Some(0)),
            CombatStats { hp: 10, max_hp: 10, attack: 100, defense: 10, speed: 10 },
            ActiveAbilities {
                abilities: vec![ActiveAbilityState {
                    ability_id: "Meteor".to_string(),
                    remaining_cooldown: 0,
                }],
            },
        )).id();

        // Spawn a cluster of 3 enemies adjacent to each other.
        // Target enemy at (5, 7)
        let enemy_target = world.spawn((
            EnemyToken { enemy_type: crate::mission::data::EnemyType::Goblin, xp_reward: 10 },
            GridPosition { x: 5, y: 7 },
            InRoom(Some(0)),
            CombatStats { hp: 50, max_hp: 50, attack: 5, defense: 5, speed: 5 },
        )).id();

        // Enemy 2 adjacent to target
        let enemy2 = world.spawn((
            EnemyToken { enemy_type: crate::mission::data::EnemyType::Goblin, xp_reward: 10 },
            GridPosition { x: 5, y: 8 },
            InRoom(Some(0)),
            CombatStats { hp: 50, max_hp: 50, attack: 5, defense: 5, speed: 5 },
        )).id();

        // Enemy 3 adjacent to target
        let enemy3 = world.spawn((
            EnemyToken { enemy_type: crate::mission::data::EnemyType::Goblin, xp_reward: 10 },
            GridPosition { x: 6, y: 7 },
            InRoom(Some(0)),
            CombatStats { hp: 50, max_hp: 50, attack: 5, defense: 5, speed: 5 },
        )).id();

        world.entity_mut(mission_ent).add_child(token);
        world.entity_mut(mission_ent).add_child(enemy_target);
        world.entity_mut(mission_ent).add_child(enemy2);
        world.entity_mut(mission_ent).add_child(enemy3);

        world.entity_mut(mission_ent).entry::<MissionTurnQueue>().and_modify(|mut q| {
            q.queue = vec![token];
            q.active_index = 0;
        });

        world.init_resource::<bevy::ecs::message::Messages<MissionLogEvent>>();

        // Run turn. Since there is a cluster of 3 enemies, Meteor should trigger.
        world.run_system_once(process_sequential_turn).unwrap();
        let abils = world.get::<ActiveAbilities>(token).unwrap();
        assert_eq!(abils.abilities[0].remaining_cooldown, 999);

        // Verify that all 3 enemies took damage
        let hp1 = world.get::<CombatStats>(enemy_target).unwrap().hp;
        let hp2 = world.get::<CombatStats>(enemy2).unwrap().hp;
        let hp3 = world.get::<CombatStats>(enemy3).unwrap().hp;
        assert!(hp1 < 50);
        assert!(hp2 < 50);
        assert!(hp3 < 50);
    }

    #[test]
    fn test_signature_cleric_mass_heal() {
        let mut world = World::new();

        let abilities_str = include_str!("../../assets/data/abilities.ron");
        let abilities: Vec<crate::hero::data::AbilityDef> =
            ron::from_str(abilities_str).expect("Failed to parse abilities.ron");
        world.insert_resource(crate::hero::data::AbilityDatabase(abilities));

        let mission_ent = world.spawn((
            Mission,
            MissionDungeon(generate_dungeon(40, 30, 2, &mut rand::rng())),
            RoomStatus {
                visited: vec![true],
                cleared: vec![false],
            },
            MissionTurnQueue::default(),
        )).id();

        let roster_hero = world.spawn((
            Hero,
            HeroInfo {
                name: "Test Cleric".to_string(),
                class: HeroClass::Cleric,
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
            HeroTraits(vec![]),
        )).id();

        let token = world.spawn((
            HeroToken(roster_hero),
            GridPosition { x: 5, y: 5 },
            InRoom(Some(0)),
            CombatStats { hp: 3, max_hp: 10, attack: 10, defense: 10, speed: 10 }, // Below 40% (3/10)
            ActiveAbilities {
                abilities: vec![ActiveAbilityState {
                    ability_id: "Mass Heal".to_string(),
                    remaining_cooldown: 0,
                }],
            },
        )).id();

        let ally = world.spawn((
            HeroToken(roster_hero),
            GridPosition { x: 6, y: 5 },
            InRoom(Some(0)),
            CombatStats { hp: 3, max_hp: 10, attack: 10, defense: 10, speed: 10 }, // Below 40% (3/10)
        )).id();

        let enemy = world.spawn((
            EnemyToken { enemy_type: crate::mission::data::EnemyType::Goblin, xp_reward: 10 },
            GridPosition { x: 5, y: 7 },
            InRoom(Some(0)),
            CombatStats { hp: 50, max_hp: 50, attack: 5, defense: 5, speed: 5 },
        )).id();

        world.entity_mut(mission_ent).add_child(token);
        world.entity_mut(mission_ent).add_child(ally);
        world.entity_mut(mission_ent).add_child(enemy);

        world.entity_mut(mission_ent).entry::<MissionTurnQueue>().and_modify(|mut q| {
            q.queue = vec![token];
            q.active_index = 0;
        });

        world.init_resource::<bevy::ecs::message::Messages<MissionLogEvent>>();

        // Run turn. Since both Cleric and ally are below 40% HP, Mass Heal should fire.
        world.run_system_once(process_sequential_turn).unwrap();
        let abils = world.get::<ActiveAbilities>(token).unwrap();
        assert_eq!(abils.abilities[0].remaining_cooldown, 999);

        // Verify both are healed to full HP (10)
        let hp1 = world.get::<CombatStats>(token).unwrap().hp;
        let hp2 = world.get::<CombatStats>(ally).unwrap().hp;
        assert_eq!(hp1, 10);
        assert_eq!(hp2, 10);
    }

    #[test]
    fn test_signature_ranger_volley() {
        let mut world = World::new();

        let abilities_str = include_str!("../../assets/data/abilities.ron");
        let abilities: Vec<crate::hero::data::AbilityDef> =
            ron::from_str(abilities_str).expect("Failed to parse abilities.ron");
        world.insert_resource(crate::hero::data::AbilityDatabase(abilities));

        let mission_ent = world.spawn((
            Mission,
            MissionDungeon(generate_dungeon(40, 30, 2, &mut rand::rng())),
            RoomStatus {
                visited: vec![true],
                cleared: vec![false],
            },
            MissionTurnQueue::default(),
        )).id();

        let roster_hero = world.spawn((
            Hero,
            HeroInfo {
                name: "Test Ranger".to_string(),
                class: HeroClass::Ranger,
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
            HeroTraits(vec![]),
        )).id();

        let token = world.spawn((
            HeroToken(roster_hero),
            GridPosition { x: 5, y: 5 },
            InRoom(Some(0)),
            CombatStats { hp: 10, max_hp: 10, attack: 100, defense: 10, speed: 10 },
            ActiveAbilities {
                abilities: vec![ActiveAbilityState {
                    ability_id: "Volley".to_string(),
                    remaining_cooldown: 0,
                }],
            },
        )).id();

        // Spawn a cluster of 3 enemies within range 2 of each other.
        let enemy_target = world.spawn((
            EnemyToken { enemy_type: crate::mission::data::EnemyType::Goblin, xp_reward: 10 },
            GridPosition { x: 5, y: 8 },
            InRoom(Some(0)),
            CombatStats { hp: 50, max_hp: 50, attack: 5, defense: 5, speed: 5 },
        )).id();

        let enemy2 = world.spawn((
            EnemyToken { enemy_type: crate::mission::data::EnemyType::Goblin, xp_reward: 10 },
            GridPosition { x: 5, y: 10 }, // dist 2
            InRoom(Some(0)),
            CombatStats { hp: 50, max_hp: 50, attack: 5, defense: 5, speed: 5 },
        )).id();

        let enemy3 = world.spawn((
            EnemyToken { enemy_type: crate::mission::data::EnemyType::Goblin, xp_reward: 10 },
            GridPosition { x: 7, y: 8 }, // dist 2
            InRoom(Some(0)),
            CombatStats { hp: 50, max_hp: 50, attack: 5, defense: 5, speed: 5 },
        )).id();

        world.entity_mut(mission_ent).add_child(token);
        world.entity_mut(mission_ent).add_child(enemy_target);
        world.entity_mut(mission_ent).add_child(enemy2);
        world.entity_mut(mission_ent).add_child(enemy3);

        world.entity_mut(mission_ent).entry::<MissionTurnQueue>().and_modify(|mut q| {
            q.queue = vec![token];
            q.active_index = 0;
        });

        world.init_resource::<bevy::ecs::message::Messages<MissionLogEvent>>();

        // Run turn. Since there is a cluster of 3 enemies, Volley should trigger.
        world.run_system_once(process_sequential_turn).unwrap();
        let abils = world.get::<ActiveAbilities>(token).unwrap();
        assert_eq!(abils.abilities[0].remaining_cooldown, 999);

        // Verify all 3 enemies took damage
        let hp1 = world.get::<CombatStats>(enemy_target).unwrap().hp;
        let hp2 = world.get::<CombatStats>(enemy2).unwrap().hp;
        let hp3 = world.get::<CombatStats>(enemy3).unwrap().hp;
        assert!(hp1 < 50);
        assert!(hp2 < 50);
        assert!(hp3 < 50);
    }

    #[test]
    fn test_trait_brave_and_cautious_signature() {
        let mut world = World::new();

        let abilities_str = include_str!("../../assets/data/abilities.ron");
        let abilities: Vec<crate::hero::data::AbilityDef> =
            ron::from_str(abilities_str).expect("Failed to parse abilities.ron");
        world.insert_resource(crate::hero::data::AbilityDatabase(abilities));

        let mission_ent1 = world.spawn((
            Mission,
            MissionDungeon(generate_dungeon(40, 30, 2, &mut rand::rng())),
            RoomStatus {
                visited: vec![true; 5],
                cleared: vec![false; 5],
            },
            MissionTurnQueue::default(),
        )).id();

        // 1. Brave Rogue: enemy is at 40% HP. Rogue should Assassinate (Brave threshold is 50%, normal is 30%).
        let roster_hero_brave = world.spawn((
            Hero,
            HeroInfo {
                name: "Brave Rogue".to_string(),
                class: HeroClass::Rogue,
                level: 1,
                xp: 0,
                xp_to_next: 100,
            },
            HeroStats {
                strength: 10, dexterity: 10, constitution: 10,
                intelligence: 10, wisdom: 10, charisma: 10,
            },
            HeroTraits(vec![HeroTrait::Brave]),
        )).id();

        let token_brave = world.spawn((
            HeroToken(roster_hero_brave),
            GridPosition { x: 5, y: 5 },
            InRoom(Some(0)),
            CombatStats { hp: 10, max_hp: 10, attack: 10, defense: 10, speed: 10 },
            ActiveAbilities {
                abilities: vec![ActiveAbilityState {
                    ability_id: "Assassinate".to_string(),
                    remaining_cooldown: 0,
                }],
            },
        )).id();

        let enemy = world.spawn((
            EnemyToken { enemy_type: crate::mission::data::EnemyType::Goblin, xp_reward: 10 },
            GridPosition { x: 5, y: 6 },
            InRoom(Some(0)),
            CombatStats { hp: 4, max_hp: 10, attack: 5, defense: 5, speed: 5 }, // 40% HP
        )).id();

        world.entity_mut(mission_ent1).add_child(token_brave);
        world.entity_mut(mission_ent1).add_child(enemy);

        world.entity_mut(mission_ent1).entry::<MissionTurnQueue>().and_modify(|mut q| {
            q.queue = vec![token_brave];
            q.active_index = 0;
        });

        world.init_resource::<bevy::ecs::message::Messages<MissionLogEvent>>();
        world.run_system_once(process_sequential_turn).unwrap();

        // Brave Rogue should have used Assassinate (cooldown = 999)
        let abils_brave = world.get::<ActiveAbilities>(token_brave).unwrap();
        assert_eq!(abils_brave.abilities[0].remaining_cooldown, 999);

        // 2. Normal Rogue: enemy is at 40% HP. Rogue should NOT Assassinate (threshold is 30%).
        let mission_ent2 = world.spawn((
            Mission,
            MissionDungeon(generate_dungeon(40, 30, 2, &mut rand::rng())),
            RoomStatus {
                visited: vec![true; 5],
                cleared: vec![false; 5],
            },
            MissionTurnQueue::default(),
        )).id();

        let roster_hero_normal = world.spawn((
            Hero,
            HeroInfo {
                name: "Normal Rogue".to_string(),
                class: HeroClass::Rogue,
                level: 1,
                xp: 0,
                xp_to_next: 100,
            },
            HeroStats {
                strength: 10, dexterity: 10, constitution: 10,
                intelligence: 10, wisdom: 10, charisma: 10,
            },
            HeroTraits(vec![]),
        )).id();

        let token_normal = world.spawn((
            HeroToken(roster_hero_normal),
            GridPosition { x: 5, y: 5 },
            InRoom(Some(0)),
            CombatStats { hp: 10, max_hp: 10, attack: 10, defense: 10, speed: 10 },
            ActiveAbilities {
                abilities: vec![ActiveAbilityState {
                    ability_id: "Assassinate".to_string(),
                    remaining_cooldown: 0,
                }],
            },
        )).id();

        let enemy2 = world.spawn((
            EnemyToken { enemy_type: crate::mission::data::EnemyType::Goblin, xp_reward: 10 },
            GridPosition { x: 5, y: 6 },
            InRoom(Some(0)),
            CombatStats { hp: 4, max_hp: 10, attack: 5, defense: 5, speed: 5 },
        )).id();

        world.entity_mut(mission_ent2).add_child(token_normal);
        world.entity_mut(mission_ent2).add_child(enemy2);

        world.entity_mut(mission_ent2).entry::<MissionTurnQueue>().and_modify(|mut q| {
            q.queue = vec![token_normal];
            q.active_index = 0;
        });

        world.run_system_once(process_sequential_turn).unwrap();

        // Normal Rogue should NOT have used Assassinate (cooldown = 0)
        let abils_normal = world.get::<ActiveAbilities>(token_normal).unwrap();
        assert_eq!(abils_normal.abilities[0].remaining_cooldown, 0);
    }

    #[test]
    fn test_trait_cautious_flee() {
        let mut world = World::new();

        let abilities_str = include_str!("../../assets/data/abilities.ron");
        let abilities: Vec<crate::hero::data::AbilityDef> =
            ron::from_str(abilities_str).expect("Failed to parse abilities.ron");
        world.insert_resource(crate::hero::data::AbilityDatabase(abilities));

        let mut dungeon = generate_dungeon(40, 30, 2, &mut rand::rng());
        // Force Room 0 to be Entrance and Room 1 to be Normal/RoomType::Normal
        dungeon.rooms[0].room_type = RoomType::Entrance;
        dungeon.rooms[1].room_type = RoomType::Normal;
        let r1_center = dungeon.rooms[1].center();

        let mission_ent = world.spawn((
            Mission,
            MissionDungeon(dungeon),
            RoomStatus {
                visited: vec![true; 5],
                cleared: vec![false; 5],
            },
            MissionTurnQueue::default(),
        )).id();

        // Spawn Cautious hero at 28% HP (should flee)
        let roster_cautious = world.spawn((
            Hero,
            HeroInfo {
                name: "Cautious Cleric".to_string(),
                class: HeroClass::Cleric,
                level: 1,
                xp: 0,
                xp_to_next: 100,
            },
            HeroStats {
                strength: 10, dexterity: 10, constitution: 10,
                intelligence: 10, wisdom: 10, charisma: 10,
            },
            HeroTraits(vec![HeroTrait::Cautious]),
        )).id();

        // Place them in Room 1 (not the entrance)
        let token_cautious = world.spawn((
            HeroToken(roster_cautious),
            GridPosition { x: r1_center.0, y: r1_center.1 },
            InRoom(Some(1)),
            CombatStats { hp: 28, max_hp: 100, attack: 10, defense: 10, speed: 10 },
            ActiveAbilities { abilities: vec![] },
        )).id();

        // Spawn an enemy in Room 1 to trigger flee/combat scoring
        let enemy = world.spawn((
            EnemyToken { enemy_type: crate::mission::data::EnemyType::Goblin, xp_reward: 10 },
            GridPosition { x: r1_center.0 + 1, y: r1_center.1 },
            InRoom(Some(1)),
            CombatStats { hp: 10, max_hp: 10, attack: 5, defense: 5, speed: 5 },
        )).id();

        world.entity_mut(mission_ent).add_child(token_cautious);
        world.entity_mut(mission_ent).add_child(enemy);

        world.entity_mut(mission_ent).entry::<MissionTurnQueue>().and_modify(|mut q| {
            q.queue = vec![token_cautious];
            q.active_index = 0;
        });

        world.init_resource::<bevy::ecs::message::Messages<MissionLogEvent>>();
        world.run_system_once(process_sequential_turn).unwrap();

        // Cautious Cleric should have moved toward the Entrance (room 0)
        // Let's verify their grid position changed from the starting room center
        let pos_cautious = world.get::<GridPosition>(token_cautious).unwrap();
        assert!(pos_cautious.x != r1_center.0 || pos_cautious.y != r1_center.1);
    }

    #[test]
    fn test_trait_greedy_chest() {
        let mut world = World::new();

        let abilities_str = include_str!("../../assets/data/abilities.ron");
        let abilities: Vec<crate::hero::data::AbilityDef> =
            ron::from_str(abilities_str).expect("Failed to parse abilities.ron");
        world.insert_resource(crate::hero::data::AbilityDatabase(abilities));

        let mission_ent = world.spawn((
            Mission,
            MissionDungeon(generate_dungeon(40, 30, 2, &mut rand::rng())),
            RoomStatus {
                visited: vec![true; 5],
                cleared: vec![false; 5],
            },
            MissionTurnQueue::default(),
        )).id();

        let roster_greedy = world.spawn((
            Hero,
            HeroInfo {
                name: "Greedy Hero".to_string(),
                class: HeroClass::Warrior,
                level: 1,
                xp: 0,
                xp_to_next: 100,
            },
            HeroStats {
                strength: 10, dexterity: 10, constitution: 10,
                intelligence: 10, wisdom: 10, charisma: 10,
            },
            HeroTraits(vec![HeroTrait::Greedy]),
        )).id();

        // Greedy hero at (5, 5)
        let token_greedy = world.spawn((
            HeroToken(roster_greedy),
            GridPosition { x: 5, y: 5 },
            InRoom(Some(0)),
            CombatStats { hp: 10, max_hp: 10, attack: 10, defense: 10, speed: 10 },
            ActiveAbilities { abilities: vec![] },
        )).id();

        // Unopened loot chest at (5, 6) - distance 1
        let chest = world.spawn((
            LootChest { opened: false, gold_reward: 50 },
            GridPosition { x: 5, y: 6 },
            InRoom(Some(0)),
        )).id();

        world.entity_mut(mission_ent).add_child(token_greedy);
        world.entity_mut(mission_ent).add_child(chest);

        world.entity_mut(mission_ent).entry::<MissionTurnQueue>().and_modify(|mut q| {
            q.queue = vec![token_greedy];
            q.active_index = 0;
        });

        world.init_resource::<bevy::ecs::message::Messages<MissionLogEvent>>();
        world.run_system_once(process_sequential_turn).unwrap();

        // The Greedy hero should have opened the chest
        let chest_component = world.get::<LootChest>(chest).unwrap();
        assert!(chest_component.opened);
    }

    #[test]
    fn test_trait_loner() {
        let mut world = World::new();

        let abilities_str = include_str!("../../assets/data/abilities.ron");
        let abilities: Vec<crate::hero::data::AbilityDef> =
            ron::from_str(abilities_str).expect("Failed to parse abilities.ron");
        world.insert_resource(crate::hero::data::AbilityDatabase(abilities));

        let mission_ent = world.spawn((
            Mission,
            MissionDungeon(generate_dungeon(40, 30, 2, &mut rand::rng())),
            RoomStatus {
                visited: vec![true; 5],
                cleared: vec![false; 5],
            },
            MissionTurnQueue::default(),
        )).id();

        let roster_loner = world.spawn((
            Hero,
            HeroInfo {
                name: "Loner Rogue".to_string(),
                class: HeroClass::Rogue,
                level: 1,
                xp: 0,
                xp_to_next: 100,
            },
            HeroStats {
                strength: 10, dexterity: 10, constitution: 10,
                intelligence: 10, wisdom: 10, charisma: 10,
            },
            HeroTraits(vec![HeroTrait::Loner]),
        )).id();

        // Spawn Loner Rogue at (5, 5)
        let token_loner = world.spawn((
            HeroToken(roster_loner),
            GridPosition { x: 5, y: 5 },
            InRoom(Some(0)),
            CombatStats { hp: 10, max_hp: 10, attack: 100, defense: 10, speed: 10 }, // attack = 100
            ActiveAbilities {
                abilities: vec![ActiveAbilityState {
                    ability_id: "Slash".to_string(),
                    remaining_cooldown: 0,
                }],
            },
        )).id();

        // Spawn ally at (6, 5) - distance 1
        let roster_ally = world.spawn((
            Hero,
            HeroInfo {
                name: "Ally".to_string(),
                class: HeroClass::Warrior,
                level: 1,
                xp: 0,
                xp_to_next: 100,
            },
            HeroStats {
                strength: 10, dexterity: 10, constitution: 10,
                intelligence: 10, wisdom: 10, charisma: 10,
            },
            HeroTraits(vec![]),
        )).id();
        let token_ally = world.spawn((
            HeroToken(roster_ally),
            GridPosition { x: 6, y: 5 },
            InRoom(Some(0)),
            CombatStats { hp: 10, max_hp: 10, attack: 10, defense: 10, speed: 10 },
        )).id();

        // Spawn enemy at (5, 6)
        let enemy = world.spawn((
            EnemyToken { enemy_type: crate::mission::data::EnemyType::Goblin, xp_reward: 10 },
            GridPosition { x: 5, y: 6 },
            InRoom(Some(0)),
            CombatStats { hp: 200, max_hp: 200, attack: 5, defense: 5, speed: 5 },
        )).id();

        world.entity_mut(mission_ent).add_child(token_loner);
        world.entity_mut(mission_ent).add_child(token_ally);
        world.entity_mut(mission_ent).add_child(enemy);

        world.entity_mut(mission_ent).entry::<MissionTurnQueue>().and_modify(|mut q| {
            q.queue = vec![token_loner];
            q.active_index = 0;
        });

        world.init_resource::<bevy::ecs::message::Messages<MissionLogEvent>>();
        world.run_system_once(process_sequential_turn).unwrap();

        // Verify the log event shows damage reduced to 90..=93 (normally 101..=104)
        use bevy::ecs::message::MessageReader;
        #[derive(Resource, Default)]
        struct TestLogEvents(Vec<MissionLogEvent>);

        world.insert_resource(TestLogEvents::default());
        let _ = world.run_system_once(|mut events: MessageReader<MissionLogEvent>, mut res: ResMut<TestLogEvents>| {
            for event in events.read() {
                res.0.push(event.clone());
            }
        });

        let log_events = world.remove_resource::<TestLogEvents>().unwrap().0;

        let mut found_dmg = false;
        for ev in log_events {
            if let MissionLogPayload::Ability { amount, is_crit, .. } = ev.payload {
                if is_crit {
                    assert!((181..=187).contains(&amount), "Expected loner reduced crit damage (181..=187), got {}", amount);
                } else {
                    assert!((90..=93).contains(&amount), "Expected loner reduced damage (90..=93), got {}", amount);
                }
                found_dmg = true;
            }
        }
        assert!(found_dmg);
    }

    #[test]
    fn test_trait_leader() {
        let mut world = World::new();

        let abilities_str = include_str!("../../assets/data/abilities.ron");
        let abilities: Vec<crate::hero::data::AbilityDef> =
            ron::from_str(abilities_str).expect("Failed to parse abilities.ron");
        world.insert_resource(crate::hero::data::AbilityDatabase(abilities));

        let mission_ent = world.spawn((
            Mission,
            MissionDungeon(generate_dungeon(40, 30, 2, &mut rand::rng())),
            RoomStatus {
                visited: vec![true; 5],
                cleared: vec![false; 5],
            },
            MissionTurnQueue::default(),
        )).id();

        // Spawn a Leader hero
        let roster_leader = world.spawn((
            Hero,
            HeroInfo {
                name: "Leader Hero".to_string(),
                class: HeroClass::Warrior,
                level: 1,
                xp: 0,
                xp_to_next: 100,
            },
            HeroStats {
                strength: 10, dexterity: 10, constitution: 10,
                intelligence: 10, wisdom: 10, charisma: 10,
            },
            HeroTraits(vec![HeroTrait::Leader]),
        )).id();
        let token_leader = world.spawn((
            HeroToken(roster_leader),
            GridPosition { x: 5, y: 5 },
            InRoom(Some(0)),
            CombatStats { hp: 10, max_hp: 10, attack: 10, defense: 10, speed: 10 },
        )).id();

        // Spawn a Cleric hero (ally) who has "Heal"
        let roster_cleric = world.spawn((
            Hero,
            HeroInfo {
                name: "Cleric".to_string(),
                class: HeroClass::Cleric,
                level: 1,
                xp: 0,
                xp_to_next: 100,
            },
            HeroStats {
                strength: 10, dexterity: 10, constitution: 10,
                intelligence: 10, wisdom: 10, charisma: 10,
            },
            HeroTraits(vec![]),
        )).id();
        let token_cleric = world.spawn((
            HeroToken(roster_cleric),
            GridPosition { x: 5, y: 6 },
            InRoom(Some(0)),
            CombatStats { hp: 10, max_hp: 10, attack: 100, defense: 10, speed: 10 }, // attack = 100 -> heal bonus = 50
            ActiveAbilities {
                abilities: vec![ActiveAbilityState {
                    ability_id: "Heal".to_string(),
                    remaining_cooldown: 0,
                }],
            },
        )).id();

        // Spawn a wounded ally (so cleric can heal them)
        let roster_wounded = world.spawn((
            Hero,
            HeroInfo {
                name: "Wounded".to_string(),
                class: HeroClass::Warrior,
                level: 1,
                xp: 0,
                xp_to_next: 100,
            },
            HeroStats {
                strength: 10, dexterity: 10, constitution: 10,
                intelligence: 10, wisdom: 10, charisma: 10,
            },
            HeroTraits(vec![]),
        )).id();
        let token_wounded = world.spawn((
            HeroToken(roster_wounded),
            GridPosition { x: 6, y: 5 },
            InRoom(Some(0)),
            CombatStats { hp: 5, max_hp: 100, attack: 10, defense: 10, speed: 10 },
        )).id();

        world.entity_mut(mission_ent).add_child(token_leader);
        world.entity_mut(mission_ent).add_child(token_cleric);
        world.entity_mut(mission_ent).add_child(token_wounded);

        world.entity_mut(mission_ent).entry::<MissionTurnQueue>().and_modify(|mut q| {
            q.queue = vec![token_cleric];
            q.active_index = 0;
        });

        world.init_resource::<bevy::ecs::message::Messages<MissionLogEvent>>();
        world.run_system_once(process_sequential_turn).unwrap();

        // Cleric attack = 100, so attack/2 = 50.
        // base heal roll: 5..=15.
        // leader bonus: +2.
        // Total heal should be: 5 + 50 + 2 = 57 to 15 + 50 + 2 = 67.
        use bevy::ecs::message::MessageReader;
        #[derive(Resource, Default)]
        struct TestLogEvents(Vec<MissionLogEvent>);

        world.insert_resource(TestLogEvents::default());
        let _ = world.run_system_once(|mut events: MessageReader<MissionLogEvent>, mut res: ResMut<TestLogEvents>| {
            for event in events.read() {
                res.0.push(event.clone());
            }
        });

        let log_events = world.remove_resource::<TestLogEvents>().unwrap().0;

        let mut found_heal = false;
        for ev in log_events {
            if let MissionLogPayload::Ability { amount, .. } = ev.payload {
                assert!(amount >= 57 && amount <= 67, "Expected leader boosted heal (57..=67), got {}", amount);
                found_heal = true;
            }
        }
        assert!(found_heal);
    }

    #[test]
    fn test_foggy_range_reduction() {
        use crate::mission::data::MissionModifier;
        use crate::mission::ai::decide_action;

        let mut rng = rand::rng();
        let map = generate_dungeon(10, 10, 2, &mut rng);

        let entity = Entity::from_bits(1);
        let info = HeroInfo {
            name: "Robin".to_string(),
            class: HeroClass::Ranger,
            level: 1,
            xp: 0,
            xp_to_next: 100,
        };
        let stats = HeroStats {
            strength: 10,
            dexterity: 10,
            constitution: 10,
            intelligence: 10,
            wisdom: 10,
            charisma: 10,
        };
        let traits = HeroTraits(vec![]);
        let grid_pos = GridPosition { x: 1, y: 1 };
        let in_room = InRoom(Some(0));
        let combat = CombatStats { hp: 10, max_hp: 10, attack: 5, defense: 5, speed: 10 };
        let room_status = RoomStatus { visited: vec![true, true], cleared: vec![false, false] };

        // Place enemy at distance 5 (x: 1, y: 6)
        let enemy_ent = Entity::from_bits(99);
        let enemy_gp = GridPosition { x: 1, y: 6 };
        let enemy_ir = InRoom(Some(0));
        let enemy_cs = CombatStats { hp: 10, max_hp: 10, attack: 5, defense: 5, speed: 10 };

        let enemies = vec![(enemy_ent, enemy_gp, enemy_ir, enemy_cs)];
        let mission_enemies = vec![enemy_ent];
        let allies = vec![];
        let mission_allies = vec![];
        let mission_chests = vec![];

        // 1. Without Foggy: Ranger range is 6. Distance is 5. Ranger should attack.
        let action_normal = decide_action(
            entity,
            &info,
            &stats,
            &traits,
            &grid_pos,
            &in_room,
            &combat,
            &map,
            &room_status,
            &enemies,
            &mission_enemies,
            &allies,
            &mission_allies,
            None,
            None,
            0,
            &mission_chests,
            &mut rng,
            &[],
            &[],
        );
        assert!(matches!(action_normal, HeroAction::Attack(_)));

        // 2. With Foggy: Ranger range is halved to 3. Distance is 5. Ranger should move.
        let action_foggy = decide_action(
            entity,
            &info,
            &stats,
            &traits,
            &grid_pos,
            &in_room,
            &combat,
            &map,
            &room_status,
            &enemies,
            &mission_enemies,
            &allies,
            &mission_allies,
            None,
            None,
            0,
            &mission_chests,
            &mut rng,
            &[MissionModifier::Foggy],
            &[],
        );
        assert!(matches!(action_foggy, HeroAction::MoveToTile(_, _)));
    }

    #[test]
    fn test_cursed_ground_healing_negation() {
        use crate::mission::data::MissionModifier;
        use crate::mission::ai::decide_action;

        let mut rng = rand::rng();
        let map = generate_dungeon(10, 10, 2, &mut rng);

        let entity = Entity::from_bits(1);
        let info = HeroInfo {
            name: "Aidan".to_string(),
            class: HeroClass::Cleric,
            level: 1,
            xp: 0,
            xp_to_next: 100,
        };
        let stats = HeroStats {
            strength: 10,
            dexterity: 10,
            constitution: 10,
            intelligence: 10,
            wisdom: 15,
            charisma: 10,
        };
        let traits = HeroTraits(vec![]);
        let grid_pos = GridPosition { x: 1, y: 1 };
        let in_room = InRoom(Some(0));
        let combat = CombatStats { hp: 10, max_hp: 10, attack: 5, defense: 5, speed: 10 };
        let room_status = RoomStatus { visited: vec![true, true], cleared: vec![false, false] };

        // Wounded ally at distance 2
        let ally_ent = Entity::from_bits(2);
        let ally_cs = CombatStats { hp: 2, max_hp: 10, attack: 5, defense: 5, speed: 10 };
        let ally_ir = InRoom(Some(0));
        let ally_gp = GridPosition { x: 1, y: 3 };

        let enemies = vec![];
        let mission_enemies = vec![];
        let allies = vec![(ally_ent, ally_cs.clone(), ally_ir, ally_gp)];
        let mission_allies = vec![ally_ent];
        let mission_chests = vec![];

        // 1. Without Cursed Ground: Cleric should heal.
        let action_normal = decide_action(
            entity,
            &info,
            &stats,
            &traits,
            &grid_pos,
            &in_room,
            &combat,
            &map,
            &room_status,
            &enemies,
            &mission_enemies,
            &allies,
            &mission_allies,
            None,
            None,
            0,
            &mission_chests,
            &mut rng,
            &[],
            &[],
        );
        assert!(matches!(action_normal, HeroAction::Heal(_)));

        // 2. With Cursed Ground: Cleric cannot heal, should choose standard move/explore.
        let action_cursed = decide_action(
            entity,
            &info,
            &stats,
            &traits,
            &grid_pos,
            &in_room,
            &combat,
            &map,
            &room_status,
            &enemies,
            &mission_enemies,
            &allies,
            &mission_allies,
            None,
            None,
            0,
            &mission_chests,
            &mut rng,
            &[MissionModifier::CursedGround],
            &[],
        );
        assert!(!matches!(action_cursed, HeroAction::Heal(_)));
    }

    #[test]
    fn test_trapped_modifier_trap_triggers() {
        use crate::mission::data::MissionModifier;
        use crate::mission::combat::update_room_status;
        use crate::mission::MissionInfo;

        let mut world = World::new();
        let mut rng = rand::rng();
        let map = generate_dungeon(20, 20, 2, &mut rng);

        let other_idx = map.rooms.iter().position(|r| r.room_type != RoomType::Entrance).unwrap();

        let mission_ent = world.spawn((
            Mission,
            MissionDungeon(map.clone()),
            RoomStatus {
                visited: vec![false, false],
                cleared: vec![false, false],
            },
            MissionInfo {
                template_id: "test".to_string(),
                name: "Test Mission".to_string(),
                difficulty: 1,
                modifiers: vec![MissionModifier::Trapped],
                biome: crate::mission::data::BiomeType::Dungeon,
            },
        )).id();

        // Spawn entering hero
        let hero_token = world.spawn((
            HeroToken(Entity::from_bits(123)),
            InRoom(Some(other_idx)),
            CombatStats { hp: 20, max_hp: 20, attack: 5, defense: 5, speed: 10 },
            ChildOf(mission_ent),
        )).id();

        world.entity_mut(mission_ent).add_child(hero_token);
        world.init_resource::<bevy::ecs::message::Messages<MissionLogEvent>>();
        
        world.run_system_once(update_room_status).unwrap();

        let combat = world.get::<CombatStats>(hero_token).unwrap();
        assert!(combat.hp < 20, "Hero should have taken trap damage");
        assert!(combat.hp >= 5, "Trap damage should be between 5 and 15");

        let visual_hit = world.get::<VisualHit>(hero_token).unwrap();
        assert!(visual_hit.amount >= 5 && visual_hit.amount <= 15);
        assert_eq!(visual_hit.effect_type, "Damage");

        use bevy::ecs::message::MessageReader;
        #[derive(Resource, Default)]
        struct TestLogEvents(Vec<MissionLogEvent>);

        world.insert_resource(TestLogEvents::default());
        let _ = world.run_system_once(|mut events: MessageReader<MissionLogEvent>, mut res: ResMut<TestLogEvents>| {
            for event in events.read() {
                res.0.push(event.clone());
            }
        });

        let log_events = world.remove_resource::<TestLogEvents>().unwrap().0;
        let found_trap = log_events.iter().any(|ev| {
            matches!(ev.payload, MissionLogPayload::TrapTriggered { .. })
        });
        assert!(found_trap, "Log should contain a TrapTriggered event");
    }

    #[test]
    fn test_boss_rat_mechanics() {
        use crate::mission::data::EnemyType;
        let mut world = World::new();

        // 1. Setup AbilityDatabase
        let abilities_str = include_str!("../../assets/data/abilities.ron");
        let abilities: Vec<crate::hero::data::AbilityDef> =
            ron::from_str(abilities_str).expect("Failed to parse abilities.ron");
        world.insert_resource(crate::hero::data::AbilityDatabase(abilities));

        // 2. Setup Mission
        let mut rng = rand::rng();
        let map = generate_dungeon(40, 30, 2, &mut rng);
        let mission_ent = world.spawn((
            Mission,
            MissionDungeon(map.clone()),
            RoomStatus {
                visited: vec![true; 5],
                cleared: vec![false; 5],
            },
            MissionTurnQueue::default(),
        )).id();

        // 3. Spawn Boss Rat Enemy
        let boss_abilities = ActiveAbilities {
            abilities: vec![
                ActiveAbilityState {
                    ability_id: "Boss Slam".to_string(),
                    remaining_cooldown: 0,
                },
                ActiveAbilityState {
                    ability_id: "Boss Summon".to_string(),
                    remaining_cooldown: 0,
                },
                ActiveAbilityState {
                    ability_id: "Boss Enrage".to_string(),
                    remaining_cooldown: 0,
                },
            ],
        };
        let boss_ent = world.spawn((
            EnemyToken {
                enemy_type: EnemyType::BossRat,
                xp_reward: 100,
            },
            Name::new("Boss Rat"),
            GridPosition { x: map.rooms[0].x + 1, y: map.rooms[0].y + 1 },
            InRoom(Some(0)),
            CombatStats { hp: 50, max_hp: 100, attack: 10, defense: 5, speed: 13 },
            boss_abilities,
            ChildOf(mission_ent),
        )).id();

        // 4. Spawn Hero
        let roster_hero = world.spawn((
            Hero,
            HeroInfo {
                name: "Test Warrior".to_string(),
                class: HeroClass::Warrior,
                level: 1,
                xp: 0,
                xp_to_next: 100,
            },
            HeroStats {
                strength: 10, dexterity: 10, constitution: 10,
                intelligence: 10, wisdom: 10, charisma: 10,
            },
            HeroTraits(vec![]),
        )).id();
        let hero_ent = world.spawn((
            HeroToken(roster_hero),
            GridPosition { x: map.rooms[0].x + 2, y: map.rooms[0].y + 2 },
            InRoom(Some(0)),
            CombatStats { hp: 50, max_hp: 50, attack: 10, defense: 5, speed: 10 },
            ChildOf(mission_ent),
        )).id();

        world.entity_mut(mission_ent).add_child(boss_ent);
        world.entity_mut(mission_ent).add_child(hero_ent);

        // Put Boss Rat and Hero in turn queue
        world.entity_mut(mission_ent).entry::<MissionTurnQueue>().and_modify(|mut q| {
            q.queue = vec![boss_ent, hero_ent];
            q.active_index = 0;
            q.combat_round_count = 0;
        });

        world.init_resource::<bevy::ecs::message::Messages<MissionLogEvent>>();

        // --- STEP 1: Boss Slam Telegraph ---
        world.run_system_once(process_sequential_turn).unwrap();

        assert!(world.get::<TelegraphedAttack>(boss_ent).is_some(), "Boss Slam should set a telegraphed attack");
        let abils = world.get::<ActiveAbilities>(boss_ent).unwrap();
        let slam_abil = abils.abilities.iter().find(|a| a.ability_id == "Boss Slam").unwrap();
        assert!(slam_abil.remaining_cooldown > 0, "Boss Slam cooldown should be set");

        // --- STEP 2: Hero Turn & Scatter + Slam Resolution ---
        world.entity_mut(mission_ent).entry::<RoomStatus>().and_modify(|mut rs| {
            rs.visited[1] = true;
        });
        world.entity_mut(mission_ent).entry::<MissionTurnQueue>().and_modify(|mut q| {
            q.active_index = 0;
        });

        let gp_before = world.get::<GridPosition>(hero_ent).unwrap().clone();
        let hp_before = world.get::<CombatStats>(hero_ent).unwrap().hp;
        
        world.run_system_once(process_sequential_turn).unwrap();

        let gp_after = world.get::<GridPosition>(hero_ent).unwrap().clone();
        let hp_after = world.get::<CombatStats>(hero_ent).unwrap().hp;
        
        assert!(gp_before.x != gp_after.x || gp_before.y != gp_after.y, "Hero should have scattered from the unsafe room");
        assert_eq!(hp_before - hp_after, 20, "Boss Slam resolution should deal 20 damage");
        assert!(world.get::<TelegraphedAttack>(boss_ent).is_none(), "TelegraphedAttack should be removed after resolution");

        // --- STEP 3: Boss Summon ---
        // Move hero back to room 0 so Boss Summon can be cast (requires allies in the room)
        world.entity_mut(hero_ent).insert((
            GridPosition { x: map.rooms[0].x + 2, y: map.rooms[0].y + 2 },
            InRoom(Some(0)),
        ));
        world.entity_mut(mission_ent).entry::<MissionTurnQueue>().and_modify(|mut q| {
            q.active_index = 0;
        });

        world.run_system_once(process_sequential_turn).unwrap();

        let queue = world.get::<MissionTurnQueue>(mission_ent).unwrap().queue.clone();
        let rat_count = world.query_filtered::<&EnemyToken, With<EnemyToken>>()
            .iter(&world)
            .filter(|e| e.enemy_type == EnemyType::GiantRat)
            .count();
        assert_eq!(rat_count, 2, "Boss Summon should spawn 2 Giant Rats");
        
        for rat_ent in world.query_filtered::<Entity, With<EnemyToken>>().iter(&world) {
            if world.get::<EnemyToken>(rat_ent).unwrap().enemy_type == EnemyType::GiantRat {
                assert!(queue.contains(&rat_ent), "Giant Rat token should be added to turn queue");
            }
        }

        // --- STEP 4: Boss Enrage ---
        world.entity_mut(mission_ent).entry::<MissionTurnQueue>().and_modify(|mut q| {
            q.active_index = 0;
            q.combat_round_count = 8;
        });

        world.run_system_once(process_sequential_turn).unwrap();

        assert!(world.get::<Enraged>(boss_ent).is_some(), "Boss Rat should become Enraged after 8 rounds");
    }

    #[test]
    fn test_mid_mission_events() {
        use crate::mission::data::{EventDatabase, MissionEventDef, EventType, EventCheckStat};
        use crate::mission::entities::MissionEventsState;
        use crate::ui::feed::MissionLogHistory;
        use crate::mission::combat::update_room_status;
        use crate::hero::HeroTraits;
        let mut world = World::new();

        // 1. Setup EventDatabase with our test events
        let events = vec![
            MissionEventDef {
                id: "mysterious_shrine".to_string(),
                name: "Mysterious Shrine".to_string(),
                description: "a mysterious shrine".to_string(),
                event_type: EventType::Shrine,
                check_stat: EventCheckStat::Wisdom,
                check_difficulty: 10,
                success_text: "meditates and unlocks the shrine's secrets".to_string(),
                failure_text: "touches the runes and triggers a backlash".to_string(),
                priority_trait: Some(HeroTrait::Greedy),
            },
        ];
        world.insert_resource(EventDatabase(events));

        // Setup Gold resource
        world.insert_resource(crate::economy::Gold(50));

        // Setup log messages
        world.init_resource::<bevy::ecs::message::Messages<MissionLogEvent>>();

        // 2. Setup Mission with Dungeon map (with at least 2 rooms)
        let mut rng = rand::rng();
        let map = generate_dungeon(40, 30, 2, &mut rng);
        let mission_ent = world.spawn((
            Mission,
            MissionDungeon(map.clone()),
            RoomStatus {
                visited: vec![true, false, false, false, false],
                cleared: vec![false; 5],
            },
            MissionEventsState {
                events_fired: 0,
                max_events: 5,
            },
            MissionLogHistory::default(),
        )).id();

        // 3. Spawn roster heroes
        let roster_greedy = world.spawn((
            Hero,
            HeroInfo {
                name: "Greedy Hero".to_string(),
                class: HeroClass::Warrior,
                level: 1,
                xp: 0,
                xp_to_next: 100,
            },
            HeroStats {
                strength: 10, dexterity: 10, constitution: 10,
                intelligence: 10, wisdom: 18, charisma: 10,
            },
            HeroTraits(vec![HeroTrait::Greedy]),
        )).id();

        let roster_normal = world.spawn((
            Hero,
            HeroInfo {
                name: "Normal Hero".to_string(),
                class: HeroClass::Warrior,
                level: 1,
                xp: 0,
                xp_to_next: 100,
            },
            HeroStats {
                strength: 10, dexterity: 10, constitution: 10,
                intelligence: 10, wisdom: 8, charisma: 10,
            },
            HeroTraits(vec![]),
        )).id();

        // Spawn token heroes (both in room 1, which will be visited)
        let token_greedy = world.spawn((
            HeroToken(roster_greedy),
            Name::new("Hero Token: Greedy Hero"),
            GridPosition { x: map.rooms[1].x, y: map.rooms[1].y },
            InRoom(Some(1)),
            CombatStats { hp: 30, max_hp: 50, attack: 10, defense: 5, speed: 10 },
            ChildOf(mission_ent),
        )).id();

        let token_normal = world.spawn((
            HeroToken(roster_normal),
            Name::new("Hero Token: Normal Hero"),
            GridPosition { x: map.rooms[1].x, y: map.rooms[1].y },
            InRoom(Some(1)),
            CombatStats { hp: 30, max_hp: 50, attack: 10, defense: 5, speed: 10 },
            ChildOf(mission_ent),
        )).id();

        world.entity_mut(mission_ent).add_child(token_greedy);
        world.entity_mut(mission_ent).add_child(token_normal);

        // Run room status update to trigger Room 1 visit and the event!
        world.run_system_once(update_room_status).unwrap();

        // Check that room status visited for room 1 is now true
        assert!(world.get::<RoomStatus>(mission_ent).unwrap().visited[1]);

        // Check that events_fired is 1!
        assert_eq!(world.get::<MissionEventsState>(mission_ent).unwrap().events_fired, 1);

        // Check the logs written to Messages<MissionLogEvent>.
        use bevy::ecs::message::MessageReader;
        #[derive(Resource, Default)]
        struct TestLogEvents(Vec<MissionLogEvent>);

        world.insert_resource(TestLogEvents::default());
        let _ = world.run_system_once(|mut events: MessageReader<MissionLogEvent>, mut res: ResMut<TestLogEvents>| {
            for event in events.read() {
                res.0.push(event.clone());
            }
        });

        let events_list = world.remove_resource::<TestLogEvents>().unwrap().0;

        assert!(events_list.iter().any(|e| matches!(&e.payload, MissionLogPayload::RoomEntry { .. })));
        
        let event_log = events_list.iter().find(|e| matches!(&e.payload, MissionLogPayload::EventTriggered { .. }));
        assert!(event_log.is_some(), "An event log should have been written");

        if let Some(MissionLogEvent { payload: MissionLogPayload::EventTriggered { event_name: _, hero_name, outcome_text, .. }, .. }) = event_log {
            assert_eq!(hero_name, "Greedy Hero", "The Greedy hero should have been selected because of trait priority");
            
            if outcome_text.contains("meditates") {
                let stats = world.get::<CombatStats>(token_greedy).unwrap();
                assert_eq!(stats.defense, 7, "Defense should be increased by 2");
                assert_eq!(stats.hp, 45, "HP should be healed by 15");
            } else {
                let stats = world.get::<CombatStats>(token_greedy).unwrap();
                assert_eq!(stats.hp, 20, "HP should be damaged by 10");
            }
        }
    }

    #[test]
    fn test_custom_enemy_behaviors() {
        use crate::mission::data::{EnemyType, EnemyBehavior};

        // --- PART A: Skirmisher Kiting ---
        {
            let mut world = World::new();
            let abilities_str = include_str!("../../assets/data/abilities.ron");
            let abilities: Vec<crate::hero::data::AbilityDef> =
                ron::from_str(abilities_str).expect("Failed to parse abilities.ron");
            world.insert_resource(crate::hero::data::AbilityDatabase(abilities));

            let mut rng = rand::rng();
            let map = generate_dungeon(40, 30, 2, &mut rng);
            let mission_ent = world.spawn((
                Mission,
                MissionDungeon(map.clone()),
                RoomStatus {
                    visited: vec![true; 5],
                    cleared: vec![false; 5],
                },
                MissionTurnQueue::default(),
            )).id();

            world.init_resource::<bevy::ecs::message::Messages<MissionLogEvent>>();

            let skirmisher_ent = world.spawn((
                EnemyToken {
                    enemy_type: EnemyType::GoblinArcher,
                    xp_reward: 10,
                },
                EnemyAI {
                    behavior: EnemyBehavior::Skirmisher,
                    attack_range: 5,
                },
                Name::new("Goblin Archer"),
                GridPosition { x: map.rooms[0].x + 2, y: map.rooms[0].y + 2 },
                InRoom(Some(0)),
                CombatStats { hp: 10, max_hp: 10, attack: 5, defense: 2, speed: 10 },
                ActiveAbilities { abilities: vec![] },
                ChildOf(mission_ent),
            )).id();

            let hero_roster = world.spawn((
                Hero,
                HeroInfo {
                    name: "Kite Hero".to_string(),
                    class: HeroClass::Warrior,
                    level: 1, xp: 0, xp_to_next: 100,
                },
                HeroStats { strength: 10, dexterity: 10, constitution: 10, intelligence: 10, wisdom: 10, charisma: 10 },
                HeroTraits(vec![]),
            )).id();

            let hero_ent = world.spawn((
                HeroToken(hero_roster),
                GridPosition { x: map.rooms[0].x + 2, y: map.rooms[0].y + 1 }, // adjacent (distance 1)
                InRoom(Some(0)),
                CombatStats { hp: 20, max_hp: 20, attack: 10, defense: 5, speed: 8 },
                ChildOf(mission_ent),
            )).id();

            world.entity_mut(mission_ent).add_child(skirmisher_ent);
            world.entity_mut(mission_ent).add_child(hero_ent);

            world.entity_mut(mission_ent).entry::<MissionTurnQueue>().and_modify(|mut q| {
                q.queue = vec![skirmisher_ent];
                q.active_index = 0;
                q.combat_round_count = 0;
            });

            world.run_system_once(process_sequential_turn).unwrap();

            let pos_after = world.get::<GridPosition>(skirmisher_ent).unwrap().clone();
            let dist = pos_after.x.abs_diff(map.rooms[0].x + 2) + pos_after.y.abs_diff(map.rooms[0].y + 1);
            assert!(dist > 1, "Skirmisher should have kited and moved away from the hero, new distance is {}", dist);
        }

        // --- PART B: Swarmer Target Selection ---
        {
            let mut world = World::new();
            let abilities_str = include_str!("../../assets/data/abilities.ron");
            let abilities: Vec<crate::hero::data::AbilityDef> =
                ron::from_str(abilities_str).expect("Failed to parse abilities.ron");
            world.insert_resource(crate::hero::data::AbilityDatabase(abilities));

            let mut rng = rand::rng();
            let map = generate_dungeon(40, 30, 2, &mut rng);
            let mission_ent = world.spawn((
                Mission,
                MissionDungeon(map.clone()),
                RoomStatus {
                    visited: vec![true; 5],
                    cleared: vec![false; 5],
                },
                MissionTurnQueue::default(),
            )).id();

            world.init_resource::<bevy::ecs::message::Messages<MissionLogEvent>>();

            let swarmer_ent = world.spawn((
                EnemyToken {
                    enemy_type: EnemyType::Slime,
                    xp_reward: 10,
                },
                EnemyAI {
                    behavior: EnemyBehavior::Swarmer,
                    attack_range: 1,
                },
                Name::new("Slime Swarmer"),
                GridPosition { x: map.rooms[0].x + 2, y: map.rooms[0].y + 2 },
                InRoom(Some(0)),
                CombatStats { hp: 10, max_hp: 10, attack: 100, defense: 2, speed: 10 },
                ActiveAbilities { abilities: vec![] },
                ChildOf(mission_ent),
            )).id();

            let hero_roster_a = world.spawn((
                Hero,
                HeroInfo {
                    name: "Hero A".to_string(),
                    class: HeroClass::Warrior,
                    level: 1, xp: 0, xp_to_next: 100,
                },
                HeroStats { strength: 10, dexterity: 10, constitution: 10, intelligence: 10, wisdom: 10, charisma: 10 },
                HeroTraits(vec![]),
            )).id();
            let hero_ent_a = world.spawn((
                HeroToken(hero_roster_a),
                GridPosition { x: map.rooms[0].x + 2, y: map.rooms[0].y + 6 }, // distance 4
                InRoom(Some(0)),
                CombatStats { hp: 5, max_hp: 20, attack: 10, defense: 5, speed: 8 }, // lower HP
                ChildOf(mission_ent),
            )).id();

            let hero_roster_b = world.spawn((
                Hero,
                HeroInfo {
                    name: "Hero B".to_string(),
                    class: HeroClass::Warrior,
                    level: 1, xp: 0, xp_to_next: 100,
                },
                HeroStats { strength: 10, dexterity: 10, constitution: 10, intelligence: 10, wisdom: 10, charisma: 10 },
                HeroTraits(vec![]),
            )).id();
            let hero_ent_b = world.spawn((
                HeroToken(hero_roster_b),
                GridPosition { x: map.rooms[0].x + 2, y: map.rooms[0].y + 1 }, // distance 1
                InRoom(Some(0)),
                CombatStats { hp: 20, max_hp: 20, attack: 10, defense: 5, speed: 8 }, // higher HP
                ChildOf(mission_ent),
            )).id();

            world.entity_mut(mission_ent).add_child(swarmer_ent);
            world.entity_mut(mission_ent).add_child(hero_ent_a);
            world.entity_mut(mission_ent).add_child(hero_ent_b);

            world.entity_mut(mission_ent).entry::<MissionTurnQueue>().and_modify(|mut q| {
                q.queue = vec![swarmer_ent];
                q.active_index = 0;
            });

            let hp_before_a = world.get::<CombatStats>(hero_ent_a).unwrap().hp;
            let hp_before_b = world.get::<CombatStats>(hero_ent_b).unwrap().hp;

            world.run_system_once(process_sequential_turn).unwrap();

            let hp_after_a = world.get::<CombatStats>(hero_ent_a).unwrap().hp;
            let hp_after_b = world.get::<CombatStats>(hero_ent_b).unwrap().hp;

            assert_eq!(hp_before_a, hp_after_a, "Hero A (lowest HP but far) should not have been attacked");
            assert!(hp_before_b > hp_after_b, "Hero B (closest) should have been attacked");
        }

        // --- PART C: Shaman Healing ---
        {
            let mut world = World::new();
            let abilities_str = include_str!("../../assets/data/abilities.ron");
            let abilities: Vec<crate::hero::data::AbilityDef> =
                ron::from_str(abilities_str).expect("Failed to parse abilities.ron");
            world.insert_resource(crate::hero::data::AbilityDatabase(abilities));

            let mut rng = rand::rng();
            let map = generate_dungeon(40, 30, 2, &mut rng);
            let mission_ent = world.spawn((
                Mission,
                MissionDungeon(map.clone()),
                RoomStatus {
                    visited: vec![true; 5],
                    cleared: vec![false; 5],
                },
                MissionTurnQueue::default(),
            )).id();

            world.init_resource::<bevy::ecs::message::Messages<MissionLogEvent>>();

            let shaman_ent = world.spawn((
                EnemyToken {
                    enemy_type: EnemyType::GoblinShaman,
                    xp_reward: 10,
                },
                EnemyAI {
                    behavior: EnemyBehavior::Shaman,
                    attack_range: 1,
                },
                Name::new("Goblin Shaman"),
                GridPosition { x: map.rooms[0].x + 2, y: map.rooms[0].y + 2 },
                InRoom(Some(0)),
                CombatStats { hp: 10, max_hp: 10, attack: 5, defense: 2, speed: 10 },
                ActiveAbilities {
                    abilities: vec![ActiveAbilityState {
                        ability_id: "Heal".to_string(),
                        remaining_cooldown: 0,
                    }],
                },
                ChildOf(mission_ent),
            )).id();

            let wounded_orc_ent = world.spawn((
                EnemyToken {
                    enemy_type: EnemyType::Orc,
                    xp_reward: 25,
                },
                EnemyAI {
                    behavior: EnemyBehavior::Standard,
                    attack_range: 1,
                },
                Name::new("Wounded Orc"),
                GridPosition { x: map.rooms[0].x + 2, y: map.rooms[0].y + 1 },
                InRoom(Some(0)),
                CombatStats { hp: 5, max_hp: 30, attack: 5, defense: 5, speed: 10 },
                ActiveAbilities { abilities: vec![] },
                ChildOf(mission_ent),
            )).id();

            world.entity_mut(mission_ent).add_child(shaman_ent);
            world.entity_mut(mission_ent).add_child(wounded_orc_ent);

            world.entity_mut(mission_ent).entry::<MissionTurnQueue>().and_modify(|mut q| {
                q.queue = vec![shaman_ent];
                q.active_index = 0;
            });

            let hp_before_orc = world.get::<CombatStats>(wounded_orc_ent).unwrap().hp;
            world.run_system_once(process_sequential_turn).unwrap();
            let hp_after_orc = world.get::<CombatStats>(wounded_orc_ent).unwrap().hp;

            assert!(hp_after_orc > hp_before_orc, "Orc ally should have been healed by the Goblin Shaman");
        }
    }

    #[test]
    fn test_gear_rarity_and_affixes() {
        use crate::equipment::{HeroEquipment, GearSlot, GearRarity, BehavioralAffix, EquipmentDatabase, GearPath, GearTier, GearStats};
        use crate::hero::data::HeroClass;

        let mut world = World::new();

        // Initialize database with some simple gear definitions
        let weapon_path = GearPath {
            class: HeroClass::Warrior,
            slot: GearSlot::Weapon,
            tiers: vec![
                GearTier {
                    name: "Basic Sword".to_string(),
                    tier: 1,
                    stats: GearStats { attack: 10, defense: 2, hp: 5 },
                    gold_cost: 10,
                    material_cost: vec![],
                    armory_level_required: 1,
                }
            ],
        };
        let db = EquipmentDatabase(vec![weapon_path]);
        world.insert_resource(db);

        // 1. Verify Rarity Stat Multipliers
        let mut eq = HeroEquipment::default();
        eq.weapon_tier = 1;
        eq.weapon_rarity = GearRarity::Common;
        let stats_common = eq.total_stats(world.resource::<EquipmentDatabase>(), HeroClass::Warrior);
        assert_eq!(stats_common.attack, 10);

        eq.weapon_rarity = GearRarity::Uncommon; // multiplier 1.2
        let stats_uncommon = eq.total_stats(world.resource::<EquipmentDatabase>(), HeroClass::Warrior);
        assert_eq!(stats_uncommon.attack, 12);

        eq.weapon_rarity = GearRarity::Rare; // multiplier 1.5
        let stats_rare = eq.total_stats(world.resource::<EquipmentDatabase>(), HeroClass::Warrior);
        assert_eq!(stats_rare.attack, 15);

        eq.weapon_rarity = GearRarity::Epic; // multiplier 2.0
        let stats_epic = eq.total_stats(world.resource::<EquipmentDatabase>(), HeroClass::Warrior);
        assert_eq!(stats_epic.attack, 20);

        eq.weapon_rarity = GearRarity::Legendary; // multiplier 2.5
        let stats_legendary = eq.total_stats(world.resource::<EquipmentDatabase>(), HeroClass::Warrior);
        assert_eq!(stats_legendary.attack, 25);

        // 2. Verify Initiative Affix in build_or_update_turn_queue
        let mission_ent = world.spawn((
            Mission,
            MissionTurnQueue::default(),
        )).id();

        let roster_hero_init = world.spawn((
            Hero,
            HeroEquipment {
                weapon_tier: 1,
                weapon_rarity: GearRarity::Rare,
                weapon_affix: Some(BehavioralAffix::Initiative),
                ..Default::default()
            },
        )).id();

        let token_init = world.spawn((
            HeroToken(roster_hero_init),
            CombatStats { hp: 50, max_hp: 50, attack: 10, defense: 5, speed: 10 },
        )).id();

        let roster_hero_normal = world.spawn((
            Hero,
            HeroEquipment::default(),
        )).id();
        let token_normal = world.spawn((
            HeroToken(roster_hero_normal),
            CombatStats { hp: 50, max_hp: 50, attack: 10, defense: 5, speed: 10 },
        )).id();

        world.entity_mut(mission_ent).add_child(token_init);
        world.entity_mut(mission_ent).add_child(token_normal);

        let _ = world.run_system_once(build_or_update_turn_queue);

        // Verify queue is populated
        let queue = world.get::<MissionTurnQueue>(mission_ent).unwrap();
        assert_eq!(queue.queue.len(), 2);

        // 3. Verify Lifesteal Affix
        let mut world2 = World::new();
        world2.init_resource::<bevy::ecs::message::Messages<MissionLogEvent>>();
        let map = generate_dungeon(40, 30, 2, &mut rand::rng());
        let mission_ent2 = world2.spawn((
            Mission,
            MissionDungeon(map.clone()),
            RoomStatus {
                visited: vec![true; 5],
                cleared: vec![false; 5],
            },
            MissionTurnQueue::default(),
        )).id();

        let roster_hero_lifesteal = world2.spawn((
            Hero,
            HeroInfo {
                name: "Lifesteal Hero".to_string(),
                class: HeroClass::Warrior,
                level: 1, xp: 0, xp_to_next: 100,
            },
            HeroStats { strength: 10, dexterity: 10, constitution: 10, intelligence: 10, wisdom: 10, charisma: 10 },
            HeroTraits(vec![]),
            HeroEquipment {
                weapon_tier: 1,
                weapon_rarity: GearRarity::Rare,
                weapon_affix: Some(BehavioralAffix::Lifesteal),
                ..Default::default()
            },
        )).id();

        let token_lifesteal = world2.spawn((
            HeroToken(roster_hero_lifesteal),
            GridPosition { x: map.rooms[0].x, y: map.rooms[0].y },
            InRoom(Some(0)),
            CombatStats { hp: 10, max_hp: 100, attack: 20, defense: 10, speed: 10 },
            ChildOf(mission_ent2),
        )).id();

        let enemy = world2.spawn((
            EnemyToken {
                enemy_type: crate::mission::data::EnemyType::GiantRat,
                xp_reward: 5,
            },
            Name::new("Target Rat"),
            GridPosition { x: map.rooms[0].x + 1, y: map.rooms[0].y },
            InRoom(Some(0)),
            CombatStats { hp: 100, max_hp: 100, attack: 5, defense: 0, speed: 5 },
            ChildOf(mission_ent2),
        )).id();

        world2.entity_mut(mission_ent2).add_child(token_lifesteal);
        world2.entity_mut(mission_ent2).add_child(enemy);

        world2.entity_mut(mission_ent2).entry::<MissionTurnQueue>().and_modify(|mut q| {
            q.queue = vec![token_lifesteal];
            q.active_index = 0;
        });

        world2.entity_mut(token_lifesteal).insert(ActiveAbilities { abilities: vec![] });
        world2.run_system_once(process_sequential_turn).unwrap();

        let stats_after = world2.get::<CombatStats>(token_lifesteal).unwrap();
        assert!(stats_after.hp > 10, "Lifesteal should have healed the hero, HP is {}", stats_after.hp);

        // 4. Verify CleaveOnHit Affix
        let mut world3 = World::new();
        world3.init_resource::<bevy::ecs::message::Messages<MissionLogEvent>>();
        let map3 = generate_dungeon(40, 30, 2, &mut rand::rng());
        let mission_ent3 = world3.spawn((
            Mission,
            MissionDungeon(map3.clone()),
            RoomStatus {
                visited: vec![true; 5],
                cleared: vec![false; 5],
            },
            MissionTurnQueue::default(),
        )).id();

        let roster_hero_cleave = world3.spawn((
            Hero,
            HeroInfo {
                name: "Cleave Hero".to_string(),
                class: HeroClass::Warrior,
                level: 1, xp: 0, xp_to_next: 100,
            },
            HeroStats { strength: 10, dexterity: 10, constitution: 10, intelligence: 10, wisdom: 10, charisma: 10 },
            HeroTraits(vec![]),
            HeroEquipment {
                weapon_tier: 1,
                weapon_rarity: GearRarity::Rare,
                weapon_affix: Some(BehavioralAffix::CleaveOnHit),
                ..Default::default()
            },
        )).id();

        let token_cleave = world3.spawn((
            HeroToken(roster_hero_cleave),
            GridPosition { x: map3.rooms[0].x, y: map3.rooms[0].y },
            InRoom(Some(0)),
            CombatStats { hp: 100, max_hp: 100, attack: 20, defense: 10, speed: 10 },
            ChildOf(mission_ent3),
        )).id();

        let enemy_main = world3.spawn((
            EnemyToken {
                enemy_type: crate::mission::data::EnemyType::GiantRat,
                xp_reward: 5,
            },
            Name::new("Rat Main"),
            GridPosition { x: map3.rooms[0].x + 1, y: map3.rooms[0].y },
            InRoom(Some(0)),
            CombatStats { hp: 100, max_hp: 100, attack: 5, defense: 0, speed: 5 },
            ChildOf(mission_ent3),
        )).id();

        let enemy_cleaved = world3.spawn((
            EnemyToken {
                enemy_type: crate::mission::data::EnemyType::GiantRat,
                xp_reward: 5,
            },
            Name::new("Rat Cleaved"),
            GridPosition { x: map3.rooms[0].x + 1, y: map3.rooms[0].y + 1 },
            InRoom(Some(0)),
            CombatStats { hp: 100, max_hp: 100, attack: 5, defense: 0, speed: 5 },
            ChildOf(mission_ent3),
        )).id();

        world3.entity_mut(mission_ent3).add_child(token_cleave);
        world3.entity_mut(mission_ent3).add_child(enemy_main);
        world3.entity_mut(mission_ent3).add_child(enemy_cleaved);

        world3.entity_mut(mission_ent3).entry::<MissionTurnQueue>().and_modify(|mut q| {
            q.queue = vec![token_cleave];
            q.active_index = 0;
        });

        world3.entity_mut(token_cleave).insert(ActiveAbilities { abilities: vec![] });
        world3.run_system_once(process_sequential_turn).unwrap();

        let stats_cleaved = world3.get::<CombatStats>(enemy_cleaved).unwrap();
        assert!(stats_cleaved.hp < 100, "Cleaved target should have taken splash damage, HP is {}", stats_cleaved.hp);
    }
}