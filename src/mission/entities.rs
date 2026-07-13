//! Mission-scoped entity components, sim/render split, and movement.
//!
//! Tokens are purely logical: stats, position, room, AI target. Rendering is
//! handled via `RenderProxyOf` entities that mirror tokens visually only while
//! their mission is the viewed mission.

use bevy::prelude::*;
use rand::Rng;

use crate::hero::status::INJURED_STAT_MULTIPLIER;
use crate::hero::{Hero, HeroInfo, HeroStats};

use super::MissionParty;
use super::data::{EnemyDatabase, EnemyType, MissionTemplateDatabase, EnemyBehavior};
use super::dungeon::{DungeonMap, RoomType};

/// Tile size in world pixels (must match mission_view).
const TILE_SIZE: f32 = 32.0;

// ── Components ──────────────────────────────────────────────────────

/// Grid position within the dungeon.
#[derive(Component, Debug, Clone, Copy, Reflect)]
pub struct GridPosition {
    pub x: u32,
    pub y: u32,
}

/// Target the entity is pathing toward.
#[derive(Component, Debug, Clone)]
pub struct MoveTarget {
    pub path: Vec<(u32, u32)>,
    pub path_index: usize,
}

/// Combat stats for mission entities (heroes and enemies).
#[derive(Component, Debug, Clone, Reflect)]
pub struct CombatStats {
    pub hp: i32,
    pub max_hp: i32,
    pub attack: i32,
    pub defense: i32,
    pub speed: i32,
}

/// Move range of a token on the grid.
///
/// NOTE: This is ignored as entities are restricted to moving exactly 1 tile per movement turn.
#[derive(Component, Debug, Clone, Copy, Reflect)]
#[reflect(Component)]
pub struct MoveRange {
    pub base: u32,
    pub bonus: u32,
}

impl MoveRange {
    pub fn max(&self) -> u32 {
        self.base + self.bonus
    }
}

/// The turn queue for sequential simulation on this mission.
#[derive(Component, Debug, Default, Reflect)]
#[reflect(Component)]
pub struct MissionTurnQueue {
    pub queue: Vec<Entity>,
    pub active_index: usize,
    pub round_count: u64,
    #[reflect(default)]
    pub combat_round_count: u32,
}

use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Reflect, Serialize, Deserialize, PartialEq, Eq)]
pub struct ActiveAbilityState {
    pub ability_id: String,
    pub remaining_cooldown: u32,
}

#[derive(Component, Debug, Clone, Reflect, Serialize, Deserialize)]
#[reflect(Component)]
pub struct ActiveAbilities {
    pub abilities: Vec<ActiveAbilityState>,
}

/// Marks an entity as a hero token in the mission. Stores the hero roster entity.
#[derive(Component, Debug)]
pub struct HeroToken(pub Entity);

/// Marks an entity as an enemy token in the mission.
#[derive(Component, Debug)]
pub struct EnemyToken {
    pub enemy_type: EnemyType,
    pub xp_reward: u32,
}

/// Custom AI behavior and range for an enemy.
#[derive(Component, Debug, Clone, Reflect)]
pub struct EnemyAI {
    pub behavior: EnemyBehavior,
    pub attack_range: u32,
}

/// A chest containing loot that can be opened by heroes.
#[derive(Component, Debug, Clone, Reflect, Serialize, Deserialize)]
#[reflect(Component)]
pub struct LootChest {
    pub gold_reward: u32,
    pub opened: bool,
}

/// Which room this entity is currently in (index into DungeonMap.rooms).
#[derive(Component, Debug, Clone, Copy)]
pub struct InRoom(pub Option<usize>);

/// Traversed intermediate coordinates for render proxies to walk step-by-step.
#[derive(Component, Debug, Clone, Default)]
pub struct VisualPathHistory {
    pub waypoints: Vec<(u32, u32)>,
}

/// Queue of visual coordinates that the render proxy is currently stepping along.
#[derive(Component, Debug, Clone, Default)]
pub struct VisualPathQueue {
    pub waypoints: Vec<(u32, u32)>,
    pub current_target: Option<(u32, u32)>,
}

/// Per-mission room visit/clear state. Attached to the Mission entity.
#[derive(Component, Debug, Default)]
pub struct RoomStatus {
    pub visited: Vec<bool>,
    pub cleared: Vec<bool>,
}

impl RoomStatus {
    /// Create a new `RoomStatus` sized for the given dungeon, marking the
    /// entrance room as already visited.
    pub fn new_for_dungeon(map: &DungeonMap) -> Self {
        let mut status = Self {
            visited: vec![false; map.rooms.len()],
            cleared: vec![false; map.rooms.len()],
        };
        if let Some(entrance_idx) = map
            .rooms
            .iter()
            .position(|r| r.room_type == RoomType::Entrance)
        {
            status.visited[entrance_idx] = true;
        }
        status
    }
}

/// Render proxy for a mission token. Lives only while the token's mission is
/// the viewed mission. Its `Transform`/`Visibility` are sync'd each frame from
/// the token's `GridPosition`/`CombatStats`.
#[derive(Component, Debug)]
pub struct RenderProxyOf(pub Entity);

// ── Token spawning helper (called from dispatch_mission) ───────────

/// Spawn logical hero and enemy tokens under the given mission entity.
///
/// Tokens have **only** sim state — no `Sprite`, `Transform`, or `Visibility`.
/// The proxy layer in `mission_view` attaches render components when viewed.
pub fn spawn_tokens_for_mission(
    commands: &mut Commands,
    mission_entity: Entity,
    map: &DungeonMap,
    party: &MissionParty,
    hero_q: &Query<(
        &HeroInfo,
        &HeroStats,
        Option<&crate::equipment::HeroEquipment>,
        &crate::hero::Fatigue,
        Option<&MoveRange>,
        Option<&crate::hero::Epithet>,
        Option<&crate::hero::history::HeroHistory>,
    ), With<Hero>>,
    equipment_db: &crate::equipment::EquipmentDatabase,
    templates: &MissionTemplateDatabase,
    enemy_db: &EnemyDatabase,
    template_id: &str,
    injured_q: &Query<(), With<crate::hero::status::Injured>>,
    class_db: Option<&crate::hero::data::ClassDatabase>,
    modifiers: &[crate::mission::data::MissionModifier],
) {
    // Find entrance room for hero placement
    let entrance = map.entrance_room().unwrap_or(&map.rooms[0]);
    let (entrance_x, entrance_y) = entrance.center();

    // Spawn hero tokens
    for (i, &hero_entity) in party.0.iter().enumerate() {
        let Ok((info, stats, maybe_equipment, fatigue, maybe_move_range, maybe_epithet, maybe_history)) = hero_q.get(hero_entity) else {
            continue;
        };

        // Spread heroes around entrance center
        let offset_x = (i as i32 % 2) - 1;
        let offset_y = (i as i32 / 2) - 1;
        let hx = (entrance_x as i32 + offset_x).clamp(0, map.width as i32 - 1) as u32;
        let hy = (entrance_y as i32 + offset_y).clamp(0, map.height as i32 - 1) as u32;

        let is_injured = injured_q.get(hero_entity).is_ok();
        let is_exhausted = fatigue.current <= 0.0;
        let mul = |v: i32| -> i32 {
            let mut val = v as f32;
            if is_injured {
                val *= INJURED_STAT_MULTIPLIER;
            }
            if is_exhausted {
                val *= 0.5;
            }
            val.floor() as i32
        };
        let str_eff = mul(stats.strength);
        let dex_eff = mul(stats.dexterity);
        let con_eff = mul(stats.constitution);

        // HP = con×3 + level×5
        let mut hp = con_eff * 3 + info.level as i32 * 5;
        let mut attack = (str_eff + dex_eff) / 2;
        let mut defense = (con_eff + dex_eff) / 2;
        let mut speed = dex_eff;

        // Apply veteran perk bonuses
        if let Some(history) = maybe_history {
            for perk in crate::hero::perk::get_earned_perks(history) {
                perk.apply_bonuses(&mut hp, &mut attack, &mut defense, &mut speed);
            }
        }

        // Apply equipment bonuses
        if let Some(equipment) = maybe_equipment {
            let gear_stats = equipment.total_stats(equipment_db, info.class);
            attack += gear_stats.attack;
            defense += gear_stats.defense;
            hp += gear_stats.hp;
        }

        let move_range = maybe_move_range.copied().unwrap_or_else(|| {
            let base_move_range = match info.class {
                crate::hero::data::HeroClass::Rogue | crate::hero::data::HeroClass::Ranger => 4,
                _ => 3,
            };
            MoveRange {
                base: base_move_range,
                bonus: 0,
            }
        });

        let mut starting_abilities = Vec::new();
        if let Some(cdb) = class_db
            && let Some(class_def) = cdb.get(info.class) {
                starting_abilities = class_def
                    .starting_abilities
                    .iter()
                    .map(|id| ActiveAbilityState {
                        ability_id: id.clone(),
                        remaining_cooldown: 0,
                    })
                    .collect();
            }

        let formatted_name = crate::hero::format_hero_name(&info.name, maybe_epithet);
        commands.spawn((
            Name::new(format!("Hero Token: {}", formatted_name)),
            HeroToken(hero_entity),
            GridPosition { x: hx, y: hy },
            InRoom(map.room_at(hx, hy)),
            CombatStats {
                hp,
                max_hp: hp,
                attack,
                defense,
                speed,
            },
            move_range,
            ChildOf(mission_entity),
            ActiveAbilities {
                abilities: starting_abilities,
            },
        ));
    }

    let is_bountiful = modifiers.contains(&crate::mission::data::MissionModifier::Bountiful);

    // Spawn chests in treasure rooms
    for (room_idx, room) in map.rooms.iter().enumerate() {
        if room.room_type == RoomType::Treasure {
            let (cx, cy) = room.center();
            let chest_gold = if is_bountiful { 150 } else { 100 };
            commands.spawn((
                Name::new("Loot Chest"),
                LootChest {
                    gold_reward: chest_gold,
                    opened: false,
                },
                GridPosition { x: cx, y: cy },
                InRoom(Some(room_idx)),
                ChildOf(mission_entity),
            ));
        }
    }

    // Spawn enemy tokens based on mission template
    let Some(template) = templates.0.iter().find(|t| t.id == template_id) else {
        return;
    };
    spawn_enemies_for_mission(commands, mission_entity, map, template, enemy_db, modifiers);
}

fn spawn_enemies_for_mission(
    commands: &mut Commands,
    mission_entity: Entity,
    map: &DungeonMap,
    template: &super::data::MissionTemplate,
    enemy_db: &EnemyDatabase,
    modifiers: &[crate::mission::data::MissionModifier],
) {
    let is_infested = modifiers.contains(&crate::mission::data::MissionModifier::Infested);
    let mut rng = rand::rng();

    let enemy_rooms: Vec<usize> = map
        .rooms
        .iter()
        .enumerate()
        .filter(|(_, r)| r.room_type != RoomType::Entrance)
        .map(|(i, _)| i)
        .collect();

    if enemy_rooms.is_empty() {
        return;
    }

    let boss_room_idx = map.rooms.iter().position(|r| r.room_type == RoomType::Boss);

    for &(enemy_type, count) in &template.enemy_types {
        let Some(enemy_def) = enemy_db.get(enemy_type) else {
            continue;
        };

        let mut count = count;
        if is_infested {
            count = count + (count / 2).max(1);
        }

        for _ in 0..count {
            let spawn_in_boss_room = enemy_type.is_boss();
            let room_idx = if spawn_in_boss_room && boss_room_idx.is_some() {
                boss_room_idx.unwrap()
            } else {
                enemy_rooms[rng.random_range(0..enemy_rooms.len())]
            };
            let (ex, ey) = map
                .random_walkable_in_room(room_idx, &mut rng)
                .unwrap_or_else(|| map.rooms[room_idx].center());

            let enemy_speed = match enemy_type {
                EnemyType::Goblin => 14,
                EnemyType::BossRat => 13,
                EnemyType::Skeleton => 8,
                EnemyType::Slime => 5,
                EnemyType::Orc => 10,
                EnemyType::GiantRat => 12,
                EnemyType::GoblinArcher => 13,
                EnemyType::GoblinShaman => 11,
                EnemyType::SpiderSwarmer => 16,
            };

            let starting_abilities = enemy_def
                .abilities
                .iter()
                .map(|id| ActiveAbilityState {
                    ability_id: id.clone(),
                    remaining_cooldown: 0,
                })
                .collect();

            commands.spawn((
                Name::new(format!("Enemy: {}", enemy_def.name)),
                EnemyToken {
                    enemy_type,
                    xp_reward: enemy_def.xp_reward,
                },
                EnemyAI {
                    behavior: enemy_def.behavior,
                    attack_range: enemy_def.attack_range,
                },
                GridPosition { x: ex, y: ey },
                InRoom(Some(room_idx)),
                CombatStats {
                    hp: enemy_def.hp,
                    max_hp: enemy_def.hp,
                    attack: enemy_def.attack,
                    defense: enemy_def.defense,
                    speed: enemy_speed,
                },
                MoveRange {
                    base: 3,
                    bonus: 0,
                },
                ActiveAbilities {
                    abilities: starting_abilities,
                },
                ChildOf(mission_entity),
            ));
        }
    }
}

// ── Simulation systems ─────────────────────────────────────────────



// ── Render proxy systems (Update schedule) ────────────────────────

/// Sync proxy `Transform` and `Visibility` from their token's state.
///
/// Uses frame-based smoothing toward the target tile position. If the token
/// is gone (despawned this frame), the proxy is skipped; `cleanup_orphaned_proxies`
/// will reap it on its next run.
pub fn sync_proxy_visuals(
    mut commands: Commands,
    time: Res<Time>,
    tokens: Query<(Entity, &GridPosition, &CombatStats, Option<&VisualPathHistory>), Or<(With<HeroToken>, With<EnemyToken>)>>,
    chests: Query<(Entity, &GridPosition, &LootChest)>,
    mut proxies: Query<(&RenderProxyOf, &mut Transform, &mut Visibility, &mut VisualPathQueue, Option<&mut Sprite>)>,
) {
    for (proxy_of, mut transform, mut visibility, mut queue, maybe_sprite) in &mut proxies {
        if let Ok((token_entity, grid_pos, stats, maybe_history)) = tokens.get(proxy_of.0) {
            // If the token has a traversed path, consume it and append to proxy's queue
            if let Some(history) = maybe_history {
                queue.waypoints.extend(&history.waypoints);
                commands.entity(token_entity).remove::<VisualPathHistory>();
            }

            // Determine current target position (waypoint or final grid position)
            let target_tile = if let Some(target) = queue.current_target {
                target
            } else if !queue.waypoints.is_empty() {
                let next = queue.waypoints.remove(0);
                queue.current_target = Some(next);
                next
            } else {
                (grid_pos.x, grid_pos.y)
            };

            let target_pos = tile_world_pos(target_tile.0, target_tile.1);
            let target_with_z = target_pos.with_z(transform.translation.z);

            // Frame-based smoothing
            let speed_factor = 8.0;
            transform.translation = transform
                .translation
                .lerp(target_with_z, (time.delta_secs() * speed_factor).min(1.0));

            // If we are close enough to the current waypoint, clear it to advance
            if queue.current_target.is_some() {
                let dist = transform.translation.distance(target_with_z);
                if dist < 2.0 {
                    queue.current_target = None;
                }
            }

            *visibility = if stats.hp <= 0 {
                Visibility::Hidden
            } else {
                Visibility::Visible
            };
        } else if let Ok((_, grid_pos, chest)) = chests.get(proxy_of.0) {
            let target_pos = tile_world_pos(grid_pos.x, grid_pos.y);
            transform.translation = target_pos.with_z(1.0);
            *visibility = Visibility::Visible;
            if let Some(mut sprite) = maybe_sprite {
                sprite.color = if chest.opened { Color::srgb(0.4, 0.4, 0.4) } else { Color::srgb(0.85, 0.65, 0.15) };
            }
        }
    }
}

/// Despawn proxies whose token or chest no longer exists.
pub fn cleanup_orphaned_proxies(
    mut commands: Commands,
    proxies: Query<(Entity, &RenderProxyOf)>,
    tokens: Query<(), Or<(With<HeroToken>, With<EnemyToken>)>>,
    chests: Query<(), With<LootChest>>,
) {
    for (proxy, proxy_of) in &proxies {
        let token_exists = tokens.get(proxy_of.0).is_ok();
        let chest_exists = chests.get(proxy_of.0).is_ok();
        if !token_exists && !chest_exists {
            commands.entity(proxy).despawn();
        }
    }
}

// ── Helpers ─────────────────────────────────────────────────────────

/// Convert grid coordinates to world position.
pub fn tile_world_pos(x: u32, y: u32) -> Vec3 {
    Vec3::new(
        x as f32 * TILE_SIZE + TILE_SIZE / 2.0,
        -(y as f32 * TILE_SIZE + TILE_SIZE / 2.0),
        0.0,
    )
}

/// Get color for a hero token based on class (fallback when sprites missing).
pub fn hero_color(class: &crate::hero::data::HeroClass) -> Color {
    use crate::hero::data::HeroClass;
    match class {
        HeroClass::Warrior => Color::srgb(0.9, 0.2, 0.2),
        HeroClass::Rogue => Color::srgb(0.6, 0.2, 0.8),
        HeroClass::Mage => Color::srgb(0.2, 0.4, 0.9),
        HeroClass::Cleric => Color::srgb(0.9, 0.8, 0.2),
        HeroClass::Ranger => Color::srgb(0.2, 0.8, 0.3),
    }
}

/// Get color for an enemy token based on type (fallback when sprites missing).
pub fn enemy_color(enemy_type: EnemyType) -> Color {
    match enemy_type {
        EnemyType::Goblin => Color::srgb(0.3, 0.7, 0.2),
        EnemyType::Skeleton => Color::srgb(0.9, 0.9, 0.85),
        EnemyType::Slime => Color::srgb(0.5, 0.9, 0.3),
        EnemyType::Orc => Color::srgb(0.5, 0.15, 0.1),
        EnemyType::BossRat => Color::srgb(0.5, 0.35, 0.2),
        EnemyType::GiantRat => Color::srgb(0.4, 0.3, 0.2),
        EnemyType::GoblinArcher => Color::srgb(0.2, 0.6, 0.3),
        EnemyType::GoblinShaman => Color::srgb(0.4, 0.8, 0.4),
        EnemyType::SpiderSwarmer => Color::srgb(0.3, 0.1, 0.4),
    }
}

/// Event component placed on a token entity to trigger visual hit feedback.
#[derive(Component, Debug, Clone)]
pub struct VisualHit {
    pub amount: i32,
    pub is_hit: bool,
    pub is_crit: bool,
    pub effect_type: String, // "Damage", "Heal", "Shield"
    pub source: Option<Entity>,
    pub is_signature: bool,
}

/// Marker component indicating that a token's death has already spawned a death poof.
#[derive(Component, Debug, Clone)]
pub struct ProcessedDeath;

/// Represents a telegraphed boss attack targeting a specific room.
#[derive(Component, Debug, Clone, Reflect)]
#[reflect(Component)]
pub struct TelegraphedAttack {
    pub target_room: usize,
    pub turns_remaining: u32,
}

/// Marker component indicating that an enemy is enraged and deals double damage.
#[derive(Component, Debug, Clone, Reflect)]
#[reflect(Component)]
pub struct Enraged;

/// Tracks the state of mid-mission random events for a mission.
#[derive(Component, Debug, Clone, Copy, Reflect)]
#[reflect(Component)]
pub struct MissionEventsState {
    pub events_fired: u32,
    pub max_events: u32,
}

