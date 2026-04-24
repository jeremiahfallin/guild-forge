//! Combat resolution: attack rolls, damage, healing, death.
//!
//! All systems iterate missions and walk `Children` so combat stays scoped
//! to each mission's own token pool.

use bevy::prelude::*;
use rand::Rng;

use crate::economy::Gold;
use crate::hero::data::HeroTrait;
use crate::hero::status::{Missing, MISSING_DURATION_SECS};
use crate::hero::Favorite;
use crate::hero::{Hero, HeroInfo, HeroStats, HeroTraits};
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
        let p0 = hero_set.p0();
        let intents: Vec<CombatIntent> = children
            .iter()
            .filter_map(|c| p0.get(c).ok())
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
                CombatIntent::Heal { target, luck_bonus } => {
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
        for c in children.iter() {
            let Ok((_, ec, er)) = enemies.get(c) else {
                continue;
            };
            if ec.hp <= 0 {
                continue;
            }
            let Some(room_idx) = er.0 else { continue };

            // Find lowest-HP living hero in same room (within this mission)
            let target = children
                .iter()
                .filter_map(|h| heroes.get(h).ok().map(|(ent, c, r)| (ent, c.hp, r.0)))
                .filter(|(_, hp, r)| *hp > 0 && *r == Some(room_idx))
                .min_by_key(|(_, hp, _)| *hp)
                .map(|(e, _, _)| e);

            let Some(target_entity) = target else {
                continue;
            };
            let Ok((_, mut hero_combat, _)) = heroes.get_mut(target_entity) else {
                continue;
            };

            let roll = rng.random_range(1..=20) + ec.attack;
            let target_ac = hero_combat.defense + 10;
            if roll >= target_ac {
                let damage = (ec.attack / 2 + rng.random_range(1..=3)).max(1);
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
    mut hero_infos: Query<
        (&mut HeroInfo, &mut HeroStats, &crate::hero::HeroGrowth, &mut crate::hero::HeroStatProgress),
        With<Hero>,
    >,
    mut gold: ResMut<Gold>,
    template_db: Res<MissionTemplateDatabase>,
    time: Res<Time<Virtual>>,
    mut materials: ResMut<crate::materials::Materials>,
    mut reputation: ResMut<crate::reputation::Reputation>,
    favorite_q: Query<(), With<Favorite>>,
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
        let all_dead = !mission_heroes.is_empty() && mission_heroes.iter().all(|(_, c)| c.hp <= 0);
        if all_dead {
            *progress = MissionProgress::Failed;
            let expires_at = time.elapsed_secs_f64() + MISSING_DURATION_SECS;

            // Favorite-aware toast title. `favorite_q` is data-less
            // (`With<Favorite>` only) to avoid a HeroInfo access conflict with
            // the mutable `hero_infos` query above; we look up the name
            // separately via `hero_infos`.
            let favorited_name = party.0.iter().find_map(|e| {
                if favorite_q.get(*e).is_ok() {
                    hero_infos.get(*e).ok().map(|(hi, _, _, _)| hi.name.clone())
                } else {
                    None
                }
            });
            let title = match favorited_name {
                Some(name) => format!("{name} is missing!"),
                None => format!("{} — Failed!", info.name),
            };
            commands.trigger(ToastEvent {
                title,
                body: "Party wiped — heroes are missing.".to_string(),
                kind: ToastKind::Failure,
            });

            for &hero_entity in &party.0 {
                commands
                    .entity(hero_entity)
                    .remove::<super::OnMission>()
                    .insert(Missing { expires_at });
            }
            commands.entity(mission_entity).despawn();
            info!("Mission '{}' failed — heroes missing for {MISSING_DURATION_SECS}s", info.name);
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

        let gold_earned = template.map_or(0, |t| {
            rng.random_range(t.gold_reward.min..=t.gold_reward.max)
        });
        gold.0 += gold_earned;

        // Award materials
        if let Some(template) = &template {
            for &(mat_type, min, max) in &template.material_drops {
                let amount = rng.random_range(min..=max);
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
            if let Ok((mut hinfo, mut hstats, hgrowth, mut hprog)) = hero_infos.get_mut(*hero_entity) {
                level_ups += crate::hero::award_xp(
                    &mut hinfo,
                    &mut hstats,
                    hgrowth,
                    &mut hprog,
                    total_xp,
                );
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
        if let Some(template) = &template {
            if template.reputation_reward > 0 {
                body.push_str(&format!(", +{} rep", template.reputation_reward));
            }
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
