//! Combat resolution: attack rolls, damage, healing, death.
//!
//! All systems iterate missions and walk `Children` so combat stays scoped
//! to each mission's own token pool.

use bevy::prelude::*;
use rand::Rng;

use crate::economy::Gold;
use crate::hero::data::HeroTrait;
use crate::hero::{Hero, HeroInfo, HeroTraits};
use crate::ui::toast::{ToastEvent, ToastKind};

use super::ai::HeroAction;
use super::data::MissionTemplateDatabase;
use super::entities::*;
use super::{Mission, MissionDungeon, MissionInfo, MissionParty, MissionProgress};

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
pub fn hero_combat_system(
    missions: Query<&Children, With<Mission>>,
    mut hero_set: ParamSet<(
        Query<(Entity, &HeroToken, &CombatStats, Option<&HeroAction>), Without<EnemyToken>>,
        Query<&mut CombatStats, (With<HeroToken>, Without<EnemyToken>)>,
    )>,
    hero_traits: Query<&HeroTraits, With<Hero>>,
    mut enemy_stats: Query<&mut CombatStats, (With<EnemyToken>, Without<HeroToken>)>,
) {
    let mut rng = rand::rng();

    for children in &missions {
        // Phase 1: collect intents for this mission's heroes
        let intents: Vec<CombatIntent> = hero_set
            .p0()
            .iter()
            .filter(|(e, _, _, _)| children.contains(e))
            .filter_map(|(_, hero_token, combat, action)| {
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

        // Phase 2: apply intents
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
}

/// Process enemy attacks — enemies attack the lowest-HP hero in their room
/// from within the same mission.
pub fn enemy_combat_system(
    missions: Query<&Children, With<Mission>>,
    enemies: Query<(Entity, &CombatStats, &InRoom), With<EnemyToken>>,
    mut heroes: Query<(Entity, &mut CombatStats, &InRoom), (With<HeroToken>, Without<EnemyToken>)>,
) {
    let mut rng = rand::rng();

    for children in &missions {
        // Collect this mission's enemies (snapshot) and hero entities
        let mission_enemies: Vec<(Entity, i32, i32, Option<usize>)> = children
            .iter()
            .filter_map(|c| {
                let (_, ec, er) = enemies.get(c).ok()?;
                Some((c, ec.hp, ec.attack, er.0))
            })
            .collect();
        let mission_heroes: Vec<Entity> = children
            .iter()
            .filter(|c| heroes.get(*c).is_ok())
            .collect();

        for (_, hp, attack, room) in &mission_enemies {
            if *hp <= 0 {
                continue;
            }
            let Some(room_idx) = *room else { continue };

            // Find lowest-HP living hero in same room (within this mission)
            let target = mission_heroes
                .iter()
                .filter_map(|&e| heroes.get(e).ok().map(|(ent, c, r)| (ent, c.hp, r.0)))
                .filter(|(_, hp, r)| *hp > 0 && *r == Some(room_idx))
                .min_by_key(|(_, hp, _)| *hp)
                .map(|(e, _, _)| e);

            let Some(target_entity) = target else {
                continue;
            };
            let Ok((_, mut hero_combat, _)) = heroes.get_mut(target_entity) else {
                continue;
            };

            let roll = rng.random_range(1..=20) + *attack;
            let target_ac = hero_combat.defense + 10;
            if roll >= target_ac {
                let damage = (*attack / 2 + rng.random_range(1..=3)).max(1);
                hero_combat.hp -= damage;
                if hero_combat.hp <= 0 {
                    info!("Hero fell!");
                }
            }
        }
    }
}

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

/// Update room visited/cleared status based on hero positions and enemy state,
/// per mission.
pub fn update_room_status(
    mut missions: Query<(&MissionDungeon, &mut RoomStatus, &Children), With<Mission>>,
    heroes: Query<&InRoom, (With<HeroToken>, Without<EnemyToken>)>,
    enemies: Query<(&InRoom, &CombatStats), With<EnemyToken>>,
) {
    for (dungeon, mut room_status, children) in &mut missions {
        let map = &dungeon.0;

        // Mark rooms as visited when this mission's heroes enter
        for c in children.iter() {
            if let Ok(in_room) = heroes.get(c) {
                if let Some(room_idx) = in_room.0 {
                    if room_idx < room_status.visited.len() {
                        room_status.visited[room_idx] = true;
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
            if !has_living_enemies && room_status.visited.get(room_idx).copied().unwrap_or(false) {
                if room_idx < room_status.cleared.len() {
                    room_status.cleared[room_idx] = true;
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
        ),
        With<Mission>,
    >,
    hero_tokens: Query<(&HeroToken, &CombatStats), Without<EnemyToken>>,
    enemy_tokens: Query<&EnemyToken>,
    mut hero_infos: Query<&mut HeroInfo>,
    mut gold: ResMut<Gold>,
    template_db: Res<MissionTemplateDatabase>,
) {
    let mut rng = rand::rng();

    for (mission_entity, mut progress, info, party, room_status, children) in &mut missions {
        if *progress != MissionProgress::InProgress {
            continue;
        }

        // Gather this mission's hero tokens
        let mission_heroes: Vec<(&HeroToken, &CombatStats)> = children
            .iter()
            .filter_map(|c| hero_tokens.get(c).ok())
            .collect();

        // Failure: all heroes dead
        let all_dead =
            !mission_heroes.is_empty() && mission_heroes.iter().all(|(_, c)| c.hp <= 0);
        if all_dead {
            *progress = MissionProgress::Failed;
            commands.trigger(ToastEvent {
                title: format!("{} — Failed!", info.name),
                body: "Party wiped — no rewards".to_string(),
                kind: ToastKind::Failure,
            });
            for &hero_entity in &party.0 {
                commands.entity(hero_entity).remove::<super::OnMission>();
            }
            commands.entity(mission_entity).despawn();
            info!("Mission '{}' failed — all heroes fell!", info.name);
            continue;
        }

        // Completion: all rooms cleared
        let all_cleared =
            !room_status.cleared.is_empty() && room_status.cleared.iter().all(|&c| c);
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

        let gold_earned = template.map_or(0, |t| {
            rng.random_range(t.gold_reward.min..=t.gold_reward.max)
        });
        gold.0 += gold_earned;

        // Count survivors and award XP
        let survivors: Vec<Entity> = mission_heroes
            .iter()
            .filter(|(_, cs)| cs.hp > 0)
            .map(|(ht, _)| ht.0)
            .collect();
        let casualties = party.0.len().saturating_sub(survivors.len());

        let mut level_ups = 0u32;
        for hero_entity in &survivors {
            if let Ok(mut hinfo) = hero_infos.get_mut(*hero_entity) {
                hinfo.xp += total_xp;
                while hinfo.xp >= hinfo.xp_to_next {
                    hinfo.xp -= hinfo.xp_to_next;
                    hinfo.level += 1;
                    hinfo.xp_to_next = (hinfo.xp_to_next as f32 * 1.5) as u32;
                    level_ups += 1;
                }
            }
        }

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
            title: format!("{} — Complete!", info.name),
            body,
            kind: ToastKind::Success,
        });

        for &hero_entity in &party.0 {
            commands.entity(hero_entity).remove::<super::OnMission>();
        }
        commands.entity(mission_entity).despawn();

        info!(
            "Mission '{}' complete — +{gold_earned}g, +{total_xp}xp",
            info.name
        );
    }
}
