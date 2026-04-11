//! Utility-based hero AI: action scoring and decision making.

use bevy::prelude::*;
use rand::Rng;

use crate::hero::{Hero, HeroInfo, HeroStats, HeroTraits};
use crate::hero::data::{HeroClass, HeroTrait};

use super::Mission;
use super::dungeon::{DungeonMap, RoomType};
use super::entities::*;
use super::pathfinding::find_path;

/// The action a hero has decided to take this tick.
#[derive(Component, Debug, Clone)]
pub enum HeroAction {
    /// Move toward a room (index into rooms).
    MoveTo(usize),
    /// Attack an enemy entity.
    Attack(Entity),
    /// Heal an ally entity (Cleric only).
    Heal(Entity),
    /// Stay and hold position.
    Hold,
}

/// Run AI decision-making for all hero tokens each simulation tick.
///
/// Iterates missions and walks `Children` so each mission's heroes only
/// target enemies within their own dungeon.
pub fn hero_ai_system(
    missions: Query<(&super::MissionDungeon, &RoomStatus, &Children), With<Mission>>,
    heroes: Query<
        (
            Entity,
            &HeroToken,
            &GridPosition,
            &InRoom,
            &CombatStats,
            Option<&MoveTarget>,
        ),
        Without<EnemyToken>,
    >,
    hero_data: Query<(&HeroInfo, &HeroStats, &HeroTraits), With<Hero>>,
    enemies: Query<(Entity, &GridPosition, &InRoom, &CombatStats), With<EnemyToken>>,
    ally_stats: Query<(Entity, &CombatStats, &InRoom), (With<HeroToken>, Without<EnemyToken>)>,
    mut commands: Commands,
) {
    let mut rng = rand::rng();

    for (dungeon, room_status, children) in &missions {
        let map = &dungeon.0;

        // Collect this mission's token entities
        let mission_hero_entities: Vec<Entity> = children
            .iter()
            .filter(|c| heroes.get(*c).is_ok())
            .collect();
        let mission_enemy_entities: Vec<Entity> = children
            .iter()
            .filter(|c| enemies.get(*c).is_ok())
            .collect();

        for hero_entity in &mission_hero_entities {
            let Ok((entity, hero_token, grid_pos, in_room, combat, existing_move)) =
                heroes.get(*hero_entity)
            else {
                continue;
            };

            if combat.hp <= 0 {
                continue;
            }

            if let Some(mt) = existing_move {
                if mt.path_index < mt.path.len() {
                    continue; // Still moving
                }
            }

            let Ok((info, stats, traits)) = hero_data.get(hero_token.0) else {
                continue;
            };

            let action = decide_action(
                entity,
                info,
                stats,
                traits,
                grid_pos,
                in_room,
                combat,
                map,
                room_status,
                &enemies,
                &mission_enemy_entities,
                &ally_stats,
                &mission_hero_entities,
                &mut rng,
            );

            match action {
                HeroAction::MoveTo(room_idx) => {
                    let room = &map.rooms[room_idx];
                    let (gx, gy) = room.center();
                    if let Some(path) = find_path(map, (grid_pos.x, grid_pos.y), (gx, gy)) {
                        commands.entity(entity).insert(MoveTarget {
                            path,
                            path_index: 1, // Skip current position
                        });
                    }
                }
                HeroAction::Attack(_) | HeroAction::Heal(_) | HeroAction::Hold => {}
            }

            commands.entity(entity).insert(action);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn decide_action(
    entity: Entity,
    info: &HeroInfo,
    stats: &HeroStats,
    traits: &HeroTraits,
    _grid_pos: &GridPosition,
    in_room: &InRoom,
    combat: &CombatStats,
    map: &DungeonMap,
    room_status: &RoomStatus,
    enemies: &Query<(Entity, &GridPosition, &InRoom, &CombatStats), With<EnemyToken>>,
    mission_enemies: &[Entity],
    allies: &Query<(Entity, &CombatStats, &InRoom), (With<HeroToken>, Without<EnemyToken>)>,
    mission_allies: &[Entity],
    rng: &mut impl Rng,
) -> HeroAction {
    let hp_pct = combat.hp as f32 / combat.max_hp.max(1) as f32;
    let current_room = in_room.0;

    // Find this mission's enemies in the same room
    let enemies_in_room: Vec<(Entity, &CombatStats)> = mission_enemies
        .iter()
        .filter_map(|&e| {
            let (_, _, er, ec) = enemies.get(e).ok()?;
            if ec.hp > 0 && er.0 == current_room && current_room.is_some() {
                Some((e, ec))
            } else {
                None
            }
        })
        .collect();

    // Find this mission's injured allies in the same room
    let injured_allies: Vec<(Entity, &CombatStats)> = mission_allies
        .iter()
        .filter_map(|&e| {
            if e == entity {
                return None;
            }
            let (_, c, ar) = allies.get(e).ok()?;
            if c.hp > 0 && c.hp < c.max_hp && ar.0 == current_room && current_room.is_some() {
                Some((e, c))
            } else {
                None
            }
        })
        .collect();

    let allies_in_room = mission_allies
        .iter()
        .filter_map(|&e| {
            if e == entity {
                return None;
            }
            let (_, c, ar) = allies.get(e).ok()?;
            if c.hp > 0 && ar.0 == current_room && current_room.is_some() {
                Some(())
            } else {
                None
            }
        })
        .count();

    // Score each possible action
    let mut best_action = HeroAction::Hold;
    let mut best_score = 10.0_f32; // Hold base score

    // 1. Attack — if enemies are in the room
    if !enemies_in_room.is_empty() {
        let mut attack_score = 60.0;

        // Bonus for low-HP enemies (finisher instinct)
        let weakest = enemies_in_room
            .iter()
            .min_by_key(|(_, c)| c.hp)
            .unwrap();
        let enemy_hp_pct = weakest.1.hp as f32 / weakest.1.max_hp.max(1) as f32;
        if enemy_hp_pct < 0.3 {
            attack_score += 20.0;
        }

        // Safety in numbers
        attack_score += allies_in_room as f32 * 10.0;

        // Class multiplier
        attack_score *= class_attack_mult(&info.class);

        // Trait multipliers
        for t in &traits.0 {
            attack_score *= trait_attack_mult(t, allies_in_room);
        }

        if attack_score > best_score {
            best_score = attack_score;
            best_action = HeroAction::Attack(weakest.0);
        }
    }

    // 2. Heal — if cleric or high WIS and injured allies present
    let can_heal = info.class == HeroClass::Cleric || stats.wisdom > 14;
    if can_heal && !injured_allies.is_empty() {
        let most_injured = injured_allies
            .iter()
            .max_by_key(|(_, c)| c.max_hp - c.hp)
            .unwrap();
        let missing_pct = 1.0 - (most_injured.1.hp as f32 / most_injured.1.max_hp.max(1) as f32);

        let mut heal_score = 70.0 * missing_pct;

        // Class multiplier
        heal_score *= class_heal_mult(&info.class);

        if heal_score > best_score {
            best_score = heal_score;
            best_action = HeroAction::Heal(most_injured.0);
        }
    }

    // 3. MoveTo — explore unvisited rooms
    for (room_idx, room) in map.rooms.iter().enumerate() {
        if room_status.visited.get(room_idx).copied().unwrap_or(true) {
            continue; // Already visited
        }

        let mut move_score = 50.0;

        if room.room_type == RoomType::Treasure {
            move_score += 10.0;
        }
        if room.room_type == RoomType::Boss {
            // Only go to boss room if other rooms are visited
            let unvisited_non_boss = map.rooms.iter().enumerate().any(|(i, r)| {
                r.room_type != RoomType::Boss
                    && !room_status.visited.get(i).copied().unwrap_or(true)
            });
            if unvisited_non_boss {
                move_score -= 40.0; // Don't rush the boss
            }
        }

        // Low HP discourages exploration
        if hp_pct < 0.3 {
            move_score -= 30.0;
        }

        // Class/trait multipliers
        move_score *= class_move_mult(&info.class);
        for t in &traits.0 {
            move_score *= trait_move_mult(t, room.room_type == RoomType::Treasure);
        }

        if move_score > best_score {
            best_score = move_score;
            best_action = HeroAction::MoveTo(room_idx);
        }
    }

    // 4. Return to visited-but-uncleared rooms (enemies still alive there)
    for (room_idx, _room) in map.rooms.iter().enumerate() {
        let visited = room_status.visited.get(room_idx).copied().unwrap_or(false);
        let cleared = room_status.cleared.get(room_idx).copied().unwrap_or(false);
        if !visited || cleared {
            continue; // Only target rooms we've seen but haven't finished
        }
        // Already in this room — combat will handle it
        if current_room == Some(room_idx) {
            continue;
        }

        let mut return_score = 45.0; // Slightly below explore (50) but well above Hold (10)

        if hp_pct < 0.3 {
            return_score -= 20.0;
        }

        return_score *= class_attack_mult(&info.class); // fighters eager to return

        if return_score > best_score {
            best_score = return_score;
            best_action = HeroAction::MoveTo(room_idx);
        }
    }

    // 5. Flee — move toward entrance if low HP (but not if already there)
    let entrance_idx = map.rooms.iter().position(|r| r.room_type == RoomType::Entrance);
    let already_at_entrance = entrance_idx.is_some_and(|idx| current_room == Some(idx));
    if hp_pct < 0.25 && !already_at_entrance {
        let mut flee_score = 60.0;
        if enemies_in_room.len() >= 2 {
            flee_score += 20.0;
        }

        // Class/trait multipliers
        flee_score *= class_flee_mult(&info.class);
        for t in &traits.0 {
            flee_score *= trait_flee_mult(t);
        }

        if flee_score > best_score {
            if let Some(entrance_idx) = entrance_idx {
                best_action = HeroAction::MoveTo(entrance_idx);
            }
        }
    }

    // Add small randomness to break ties
    let _ = rng.random_range(0.0..2.0_f32);

    best_action
}

// ── Class multipliers ───────────────────────────────────────────────

fn class_attack_mult(class: &HeroClass) -> f32 {
    match class {
        HeroClass::Warrior => 1.3,
        HeroClass::Rogue => 1.1,
        HeroClass::Mage => 1.2,
        HeroClass::Cleric => 0.8,
        HeroClass::Ranger => 1.1,
    }
}

fn class_heal_mult(class: &HeroClass) -> f32 {
    match class {
        HeroClass::Cleric => 1.5,
        HeroClass::Mage => 0.6,
        _ => 0.5,
    }
}

fn class_move_mult(class: &HeroClass) -> f32 {
    match class {
        HeroClass::Ranger => 1.3,
        HeroClass::Rogue => 1.2,
        HeroClass::Mage => 0.9,
        _ => 1.0,
    }
}

fn class_flee_mult(class: &HeroClass) -> f32 {
    match class {
        HeroClass::Warrior => 0.7,
        HeroClass::Rogue => 1.1,
        _ => 1.0,
    }
}

// ── Trait multipliers ───────────────────────────────────────────────

fn trait_attack_mult(t: &HeroTrait, allies_in_room: usize) -> f32 {
    match t {
        HeroTrait::Brave => 1.4,
        HeroTrait::Cautious => 0.8,
        HeroTrait::Cursed => 1.3,
        HeroTrait::Loner if allies_in_room == 0 => 1.3,
        HeroTrait::Loner => 0.8,
        _ => 1.0,
    }
}

fn trait_move_mult(t: &HeroTrait, is_treasure: bool) -> f32 {
    match t {
        HeroTrait::Cautious => 0.7,
        HeroTrait::Greedy if is_treasure => 1.8,
        HeroTrait::Greedy => 1.0,
        _ => 1.0,
    }
}

fn trait_flee_mult(t: &HeroTrait) -> f32 {
    match t {
        HeroTrait::Brave => 0.3,
        HeroTrait::Cautious => 1.5,
        _ => 1.0,
    }
}
