//! Combat resolution: attack rolls, damage, healing, death.

use bevy::prelude::*;
use rand::Rng;

use crate::economy::Gold;
use crate::hero::data::HeroTrait;
use crate::hero::{Hero, HeroInfo, HeroTraits};
use crate::ui::toast::{ToastEvent, ToastKind};

use super::ai::HeroAction;
use super::data::MissionTemplateDatabase;
use super::dungeon::RoomType;
use super::entities::*;
use super::{Mission, MissionInfo, MissionParty, MissionProgress};

/// Collected combat action to apply after reading hero state.
enum CombatIntent {
    Attack {
        target: Entity,
        attack: i32,
        luck_bonus: i32,
    },
    Heal {
        target: Entity,
        luck_bonus: i32,
    },
}

/// Process hero attacks and healing each simulation tick.
///
/// Uses `ParamSet` to avoid Bevy B0001: the hero-read query and the
/// hero-write query both touch `CombatStats` on `HeroToken` entities.
/// A two-phase collect→apply pattern lets us access them through the
/// same `ParamSet` without holding conflicting borrows.
pub fn hero_combat_system(
    timer: Res<SimulationTimer>,
    mut hero_set: ParamSet<(
        // p0: read heroes
        Query<(&HeroToken, &CombatStats, Option<&HeroAction>), Without<EnemyToken>>,
        // p1: write hero CombatStats (for healing)
        Query<&mut CombatStats, (With<HeroToken>, Without<EnemyToken>)>,
    )>,
    hero_traits: Query<&HeroTraits, With<Hero>>,
    mut enemy_stats: Query<&mut CombatStats, (With<EnemyToken>, Without<HeroToken>)>,
) {
    // Only run right after a tick
    if !timer.ticked {
        return;
    }

    // Phase 1: Collect intents (read-only borrow via p0)
    let intents: Vec<CombatIntent> = hero_set
        .p0()
        .iter()
        .filter_map(|(hero_token, combat, action)| {
            if combat.hp <= 0 {
                return None;
            }
            let action = action?;
            let lucky = hero_traits
                .get(hero_token.0)
                .ok()
                .is_some_and(|t| t.0.contains(&HeroTrait::Lucky));
            let luck_bonus = if lucky { 3 } else { 0 };

            match action {
                HeroAction::Attack(target) => Some(CombatIntent::Attack {
                    target: *target,
                    attack: combat.attack,
                    luck_bonus,
                }),
                HeroAction::Heal(target) => Some(CombatIntent::Heal {
                    target: *target,
                    luck_bonus,
                }),
                _ => None,
            }
        })
        .collect();

    // Phase 2: Apply intents (mutable borrows via p1 and enemy_stats)
    let mut rng = rand::rng();

    for intent in &intents {
        match intent {
            CombatIntent::Attack {
                target,
                attack,
                luck_bonus,
            } => {
                if let Ok(mut enemy_combat) = enemy_stats.get_mut(*target) {
                    if enemy_combat.hp <= 0 {
                        continue;
                    }

                    let roll = rng.random_range(1..=20) + attack + luck_bonus;
                    let target_ac = enemy_combat.defense + 10;

                    if roll >= target_ac {
                        let damage = (attack / 2 + rng.random_range(1..=4)).max(1);
                        enemy_combat.hp -= damage;

                        if enemy_combat.hp <= 0 {
                            info!("Enemy defeated!");
                        }
                    }
                }
            }
            CombatIntent::Heal {
                target,
                luck_bonus,
            } => {
                if let Ok(mut ally_combat) = hero_set.p1().get_mut(*target) {
                    if ally_combat.hp <= 0 || ally_combat.hp >= ally_combat.max_hp {
                        continue;
                    }

                    let heal = rng.random_range(1..=8) + luck_bonus;
                    ally_combat.hp = (ally_combat.hp + heal).min(ally_combat.max_hp);
                }
            }
        }
    }
}

/// Process enemy attacks — enemies always attack the lowest-HP hero in their room.
pub fn enemy_combat_system(
    timer: Res<SimulationTimer>,
    enemies: Query<(&CombatStats, &InRoom), With<EnemyToken>>,
    mut heroes: Query<(Entity, &mut CombatStats, &InRoom), (With<HeroToken>, Without<EnemyToken>)>,
) {
    if !timer.ticked {
        return;
    }

    let mut rng = rand::rng();

    for (enemy_combat, enemy_room) in &enemies {
        if enemy_combat.hp <= 0 {
            continue;
        }

        let Some(room_idx) = enemy_room.0 else {
            continue;
        };

        // Find lowest-HP living hero in same room
        let target = heroes
            .iter()
            .filter(|(_, c, r)| c.hp > 0 && r.0 == Some(room_idx))
            .min_by_key(|(_, c, _)| c.hp)
            .map(|(e, _, _)| e);

        let Some(target_entity) = target else {
            continue;
        };

        let Ok((_, mut hero_combat, _)) = heroes.get_mut(target_entity) else {
            continue;
        };

        // Roll d20 + attack vs hero defense + 10
        let roll = rng.random_range(1..=20) + enemy_combat.attack;
        let target_ac = hero_combat.defense + 10;

        if roll >= target_ac {
            let damage = (enemy_combat.attack / 2 + rng.random_range(1..=3)).max(1);
            hero_combat.hp -= damage;

            if hero_combat.hp <= 0 {
                info!("Hero fell!");
            }
        }
    }
}

/// Despawn dead enemies and handle dead heroes.
pub fn handle_death_system(
    mut commands: Commands,
    dead_enemies: Query<(Entity, &EnemyToken, &CombatStats), With<EnemyToken>>,
    dead_heroes: Query<(Entity, &CombatStats), With<HeroToken>>,
) {
    for (entity, _token, combat) in &dead_enemies {
        if combat.hp <= 0 {
            commands.entity(entity).despawn();
        }
    }

    // Make dead heroes invisible but don't despawn (we need them for results)
    for (entity, combat) in &dead_heroes {
        if combat.hp <= 0 {
            commands.entity(entity).insert(Visibility::Hidden);
        }
    }
}

/// Update room visited/cleared status based on hero positions and enemy state.
pub fn update_room_status(
    timer: Res<SimulationTimer>,
    dungeon: Option<Res<crate::screens::mission_view::ActiveDungeon>>,
    mut room_status: Option<ResMut<RoomStatus>>,
    heroes: Query<&InRoom, (With<HeroToken>, Without<EnemyToken>)>,
    enemies: Query<(&InRoom, &CombatStats), With<EnemyToken>>,
) {
    if !timer.ticked {
        return;
    }

    let Some(dungeon) = dungeon else { return };
    let Some(ref mut room_status) = room_status else { return };
    let map = &dungeon.0;

    // Mark rooms as visited when heroes enter
    for in_room in &heroes {
        if let Some(room_idx) = in_room.0 {
            if room_idx < room_status.visited.len() {
                room_status.visited[room_idx] = true;
            }
        }
    }

    // A room is cleared if no living enemies remain in it
    for (room_idx, _room) in map.rooms.iter().enumerate() {
        let has_living_enemies = enemies
            .iter()
            .any(|(er, ec)| er.0 == Some(room_idx) && ec.hp > 0);
        if !has_living_enemies && room_status.visited.get(room_idx).copied().unwrap_or(false) {
            if room_idx < room_status.cleared.len() {
                room_status.cleared[room_idx] = true;
            }
        }
    }
}

/// Check if the mission is complete (all rooms cleared) or failed (all heroes dead).
/// Awards gold, XP, and fires toast notifications on completion.
pub fn check_mission_completion(
    mut commands: Commands,
    timer: Res<SimulationTimer>,
    room_status: Option<Res<RoomStatus>>,
    hero_tokens: Query<(&HeroToken, &CombatStats), Without<EnemyToken>>,
    dead_enemies: Query<&EnemyToken, Without<HeroToken>>,
    mission_entities: Query<Entity, With<MissionEntity>>,
    mut missions: Query<(Entity, &mut MissionProgress, &MissionInfo, &MissionParty), With<Mission>>,
    mut hero_infos: Query<&mut HeroInfo>,
    mut gold: ResMut<Gold>,
    template_db: Res<MissionTemplateDatabase>,
) {
    if !timer.ticked {
        return;
    }

    let Some(room_status) = room_status else { return };

    // Check if all heroes are dead → mission failed
    let all_dead = !hero_tokens.is_empty() && hero_tokens.iter().all(|(_, c)| c.hp <= 0);
    if all_dead {
        for (mission_entity_id, mut progress, info, party) in &mut missions {
            *progress = MissionProgress::Failed;
            commands.trigger(ToastEvent {
                title: format!("{} — Failed!", info.name),
                body: "Party wiped — no rewards".to_string(),
                kind: ToastKind::Failure,
            });

            // Clean up mission entities
            for entity in &mission_entities {
                commands.entity(entity).despawn();
            }
            // Remove OnMission from party heroes
            for &hero_entity in &party.0 {
                commands.entity(hero_entity).remove::<super::OnMission>();
            }
            commands.entity(mission_entity_id).despawn();
        }
        // Clean up resources
        commands.remove_resource::<RoomStatus>();
        commands.remove_resource::<SimulationTimer>();
        commands.remove_resource::<SimulationSpeed>();
        commands.remove_resource::<crate::screens::mission_view::ActiveDungeon>();
        info!("Mission failed — all heroes fell!");
        return;
    }

    // Check if all rooms are cleared → mission complete
    let all_cleared = !room_status.cleared.is_empty()
        && room_status.cleared.iter().all(|&c| c);
    if all_cleared {
        for (mission_entity_id, mut progress, mission_info, party) in &mut missions {
            *progress = MissionProgress::Complete;

            // Look up template for rewards
            let template = template_db.0.iter().find(|t| t.id == mission_info.template_id);

            // Sum XP from all dead enemies (hp <= 0 means they were killed)
            // All enemies in a completed mission count as defeated
            let enemy_xp: u32 = dead_enemies.iter().map(|e| e.xp_reward).sum();

            let xp_bonus = template.map_or(0, |t| t.xp_bonus);
            let total_xp = enemy_xp + xp_bonus;

            // Roll gold reward
            let mut rng = rand::rng();
            let gold_earned = template.map_or(0, |t| {
                rng.random_range(t.gold_reward.min..=t.gold_reward.max)
            });
            gold.0 += gold_earned;

            // Count survivors and award XP
            let survivors: Vec<Entity> = hero_tokens
                .iter()
                .filter(|(_, cs)| cs.hp > 0)
                .map(|(ht, _)| ht.0)
                .collect();
            let casualties = party.0.len().saturating_sub(survivors.len());

            let mut level_ups = 0u32;
            for hero_entity in &survivors {
                if let Ok(mut info) = hero_infos.get_mut(*hero_entity) {
                    info.xp += total_xp;
                    while info.xp >= info.xp_to_next {
                        info.xp -= info.xp_to_next;
                        info.level += 1;
                        info.xp_to_next = (info.xp_to_next as f32 * 1.5) as u32;
                        level_ups += 1;
                    }
                }
            }

            // Build toast body
            let mut body = format!("+{gold_earned}g, +{total_xp}xp");
            if casualties > 0 {
                body.push_str(&format!(
                    " — {} casualt{}",
                    casualties,
                    if casualties == 1 { "y" } else { "ies" }
                ));
            }
            if level_ups > 0 {
                body.push_str(&format!(
                    " — {} level up{}!",
                    level_ups,
                    if level_ups == 1 { "" } else { "s" }
                ));
            }

            commands.trigger(ToastEvent {
                title: format!("{} — Complete!", mission_info.name),
                body,
                kind: ToastKind::Success,
            });

            // Clean up mission entities
            for entity in &mission_entities {
                commands.entity(entity).despawn();
            }
            // Remove OnMission from party heroes
            for &hero_entity in &party.0 {
                commands.entity(hero_entity).remove::<super::OnMission>();
            }
            commands.entity(mission_entity_id).despawn();

            info!("Mission complete — all rooms cleared! +{gold_earned}g, +{total_xp}xp");
        }
        // Clean up resources
        commands.remove_resource::<RoomStatus>();
        commands.remove_resource::<SimulationTimer>();
        commands.remove_resource::<SimulationSpeed>();
        commands.remove_resource::<crate::screens::mission_view::ActiveDungeon>();
    }
}
