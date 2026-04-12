//! Mission-scoped entity components, sim/render split, and movement.
//!
//! Tokens are purely logical: stats, position, room, AI target. Rendering is
//! handled via `RenderProxyOf` entities that mirror tokens visually only while
//! their mission is the viewed mission.

use bevy::prelude::*;
use rand::Rng;

use crate::hero::{Hero, HeroInfo, HeroStats};

use super::Mission;
use super::MissionParty;
use super::data::{EnemyDatabase, EnemyType, MissionTemplateDatabase};
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

/// Which room this entity is currently in (index into DungeonMap.rooms).
#[derive(Component, Debug, Clone, Copy)]
pub struct InRoom(pub Option<usize>);

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
    hero_q: &Query<(&HeroInfo, &HeroStats, Option<&crate::equipment::HeroEquipment>), With<Hero>>,
    equipment_db: &crate::equipment::EquipmentDatabase,
    templates: &MissionTemplateDatabase,
    enemy_db: &EnemyDatabase,
    template_id: &str,
) {
    // Find entrance room for hero placement
    let entrance = map.entrance_room().unwrap_or(&map.rooms[0]);
    let (entrance_x, entrance_y) = entrance.center();

    // Spawn hero tokens
    for (i, &hero_entity) in party.0.iter().enumerate() {
        let Ok((info, stats, maybe_equipment)) = hero_q.get(hero_entity) else {
            continue;
        };

        // Spread heroes around entrance center
        let offset_x = (i as i32 % 2) - 1;
        let offset_y = (i as i32 / 2) - 1;
        let hx = (entrance_x as i32 + offset_x).clamp(0, map.width as i32 - 1) as u32;
        let hy = (entrance_y as i32 + offset_y).clamp(0, map.height as i32 - 1) as u32;

        // HP = con×3 + level×5
        let mut hp = stats.constitution * 3 + info.level as i32 * 5;
        let mut attack = (stats.strength + stats.dexterity) / 2;
        let mut defense = (stats.constitution + stats.dexterity) / 2;

        // Apply equipment bonuses
        if let Some(equipment) = maybe_equipment {
            let gear_stats = equipment.total_stats(equipment_db, info.class);
            attack += gear_stats.attack;
            defense += gear_stats.defense;
            hp += gear_stats.hp;
        }

        commands.spawn((
            Name::new(format!("Hero Token: {}", info.name)),
            HeroToken(hero_entity),
            GridPosition { x: hx, y: hy },
            InRoom(map.room_at(hx, hy)),
            CombatStats {
                hp,
                max_hp: hp,
                attack,
                defense,
            },
            ChildOf(mission_entity),
        ));
    }

    // Spawn enemy tokens based on mission template
    let Some(template) = templates.0.iter().find(|t| t.id == template_id) else {
        return;
    };
    spawn_enemies_for_mission(commands, mission_entity, map, template, enemy_db);
}

fn spawn_enemies_for_mission(
    commands: &mut Commands,
    mission_entity: Entity,
    map: &DungeonMap,
    template: &super::data::MissionTemplate,
    enemy_db: &EnemyDatabase,
) {
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

    for &(enemy_type, count) in &template.enemy_types {
        let Some(enemy_def) = enemy_db.get(enemy_type) else {
            continue;
        };

        for _ in 0..count {
            let room_idx = enemy_rooms[rng.random_range(0..enemy_rooms.len())];
            let room = &map.rooms[room_idx];

            let ex = room.x + rng.random_range(0..room.w);
            let ey = room.y + rng.random_range(0..room.h);

            commands.spawn((
                Name::new(format!("Enemy: {}", enemy_def.name)),
                EnemyToken {
                    enemy_type,
                    xp_reward: enemy_def.xp_reward,
                },
                GridPosition { x: ex, y: ey },
                InRoom(Some(room_idx)),
                CombatStats {
                    hp: enemy_def.hp,
                    max_hp: enemy_def.hp,
                    attack: enemy_def.attack,
                    defense: enemy_def.defense,
                },
                ChildOf(mission_entity),
            ));
        }
    }
}

// ── Simulation systems ─────────────────────────────────────────────

/// Advance hero tokens along their `MoveTarget` path by one cell. Runs first
/// in the `FixedUpdate` chain so downstream systems see fresh positions.
pub fn move_tokens_along_paths(
    missions: Query<(&super::MissionDungeon, &Children), With<Mission>>,
    mut heroes: Query<
        (&mut GridPosition, &mut MoveTarget, &mut InRoom),
        (With<HeroToken>, Without<EnemyToken>),
    >,
) {
    for (dungeon, children) in &missions {
        let map = &dungeon.0;
        for child in children.iter() {
            let Ok((mut grid_pos, mut target, mut in_room)) = heroes.get_mut(child) else {
                continue;
            };
            if target.path_index >= target.path.len() {
                continue;
            }

            let (nx, ny) = target.path[target.path_index];
            grid_pos.x = nx;
            grid_pos.y = ny;
            in_room.0 = map.room_at(nx, ny);
            target.path_index += 1;
        }
    }
}

// ── Render proxy systems (Update schedule) ────────────────────────

/// Sync proxy `Transform` and `Visibility` from their token's state.
///
/// Uses frame-based smoothing toward the target tile position. If the token
/// is gone (despawned this frame), the proxy is skipped; `cleanup_orphaned_proxies`
/// will reap it on its next run.
pub fn sync_proxy_visuals(
    time: Res<Time>,
    tokens: Query<(&GridPosition, &CombatStats), Or<(With<HeroToken>, With<EnemyToken>)>>,
    mut proxies: Query<(&RenderProxyOf, &mut Transform, &mut Visibility)>,
) {
    for (proxy_of, mut transform, mut visibility) in &mut proxies {
        let Ok((grid_pos, stats)) = tokens.get(proxy_of.0) else {
            continue;
        };
        let target_pos = tile_world_pos(grid_pos.x, grid_pos.y);
        let target_with_z = target_pos.with_z(transform.translation.z);

        // Frame-based smoothing — matches old sync_sprite_positions feel.
        // Virtual time scaling naturally speeds this up when the sim is fast.
        let speed_factor = 8.0;
        transform.translation = transform
            .translation
            .lerp(target_with_z, (time.delta_secs() * speed_factor).min(1.0));

        *visibility = if stats.hp <= 0 {
            Visibility::Hidden
        } else {
            Visibility::Visible
        };
    }
}

/// Despawn proxies whose token no longer exists (killed enemies, etc.).
pub fn cleanup_orphaned_proxies(
    mut commands: Commands,
    proxies: Query<(Entity, &RenderProxyOf)>,
    tokens: Query<(), Or<(With<HeroToken>, With<EnemyToken>)>>,
) {
    for (proxy, proxy_of) in &proxies {
        if tokens.get(proxy_of.0).is_err() {
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
    }
}
