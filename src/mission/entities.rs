//! Mission-scoped entity components, spawning, and movement.

use bevy::prelude::*;
use rand::Rng;

use crate::hero::{Hero, HeroInfo, HeroStats};
use crate::screens::GameTab;

use super::data::{EnemyDatabase, EnemyType, MissionTemplateDatabase};
use super::dungeon::{DungeonMap, RoomType};
use super::pathfinding::find_path;
use super::MissionParty;
use super::Mission;

/// Tile size in world pixels (must match mission_view).
const TILE_SIZE: f32 = 32.0;

// ── Components ──────────────────────────────────────────────────────

/// Marks an entity as existing within the mission simulation.
#[derive(Component, Debug)]
pub struct MissionEntity;

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

/// Interpolation state for smooth movement between grid cells.
#[derive(Component, Debug)]
pub struct MoveLerp {
    pub from: Vec3,
    pub to: Vec3,
    pub t: f32,
}

/// Combat stats for mission entities (heroes and enemies).
#[derive(Component, Debug, Clone, Reflect)]
pub struct CombatStats {
    pub hp: i32,
    pub max_hp: i32,
    pub attack: i32,
    pub defense: i32,
}

/// Marks an entity as a hero token in the mission.
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

/// Tracks which rooms have been visited/cleared.
#[derive(Resource, Debug, Default)]
pub struct RoomStatus {
    pub visited: Vec<bool>,
    pub cleared: Vec<bool>,
}

// ── Simulation tick ─────────────────────────────────────────────────

/// Controls simulation speed (1x, 2x, 4x).
#[derive(Resource, Debug)]
pub struct SimulationSpeed(pub f32);

impl Default for SimulationSpeed {
    fn default() -> Self {
        Self(1.0)
    }
}

/// Accumulates time for simulation ticks.
#[derive(Resource, Debug, Default)]
pub struct SimulationTimer {
    pub elapsed: f32,
    /// Set to `true` by `simulation_tick` when a tick fires, checked by
    /// downstream systems, then cleared at the end of the frame.
    pub ticked: bool,
}

/// Interval between simulation ticks in seconds.
pub const TICK_INTERVAL: f32 = 0.5;

// ── Systems ─────────────────────────────────────────────────────────

/// Spawn hero and enemy tokens when entering the mission view.
pub fn spawn_mission_entities(
    mut commands: Commands,
    mission_q: Query<&MissionParty, With<Mission>>,
    hero_q: Query<(&HeroInfo, &HeroStats), With<Hero>>,
    dungeon: Option<Res<crate::screens::mission_view::ActiveDungeon>>,
    templates: Option<Res<MissionTemplateDatabase>>,
    enemy_db: Option<Res<EnemyDatabase>>,
    mission_info_q: Query<&super::MissionInfo, With<Mission>>,
    char_sprites: Option<Res<super::tileset::CharacterSprites>>,
) {
    let Some(dungeon) = dungeon else { return };
    let map = &dungeon.0;

    // Find entrance room for hero placement
    let entrance = map.entrance_room().unwrap_or(&map.rooms[0]);
    let (entrance_x, entrance_y) = entrance.center();

    // Spawn hero tokens
    for party in mission_q.iter() {
        for (i, &hero_entity) in party.0.iter().enumerate() {
            let Ok((info, stats)) = hero_q.get(hero_entity) else {
                continue;
            };

            // Spread heroes around entrance center
            let offset_x = (i as i32 % 2) - 1;
            let offset_y = (i as i32 / 2) - 1;
            let hx = (entrance_x as i32 + offset_x).clamp(0, map.width as i32 - 1) as u32;
            let hy = (entrance_y as i32 + offset_y).clamp(0, map.height as i32 - 1) as u32;

            let pos = tile_world_pos(hx, hy);

            // HP = con×3 + level×5
            let hp = stats.constitution * 3 + info.level as i32 * 5;
            let attack = (stats.strength + stats.dexterity) / 2;
            let defense = (stats.constitution + stats.dexterity) / 2;

            if let Some(ref char_sprites) = char_sprites {
                let entry = &char_sprites.hero;
                commands.spawn((
                    Name::new(format!("Hero Token: {}", info.name)),
                    MissionEntity,
                    HeroToken(hero_entity),
                    GridPosition { x: hx, y: hy },
                    InRoom(map.room_at(hx, hy)),
                    CombatStats {
                        hp,
                        max_hp: hp,
                        attack,
                        defense,
                    },
                    Sprite {
                        image: entry.texture.clone(),
                        texture_atlas: Some(TextureAtlas {
                            layout: entry.layout.clone(),
                            index: 0,
                        }),
                        ..default()
                    },
                    super::tileset::SpriteAnimation::new(entry.frame_count),
                    Transform::from_translation(pos.with_z(5.0)),
                ));
            } else {
                let color = hero_color(&info.class);
                commands.spawn((
                    Name::new(format!("Hero Token: {}", info.name)),
                    MissionEntity,
                    HeroToken(hero_entity),
                    GridPosition { x: hx, y: hy },
                    InRoom(map.room_at(hx, hy)),
                    CombatStats {
                        hp,
                        max_hp: hp,
                        attack,
                        defense,
                    },
                    Sprite {
                        color,
                        custom_size: Some(Vec2::splat(24.0)),
                        ..default()
                    },
                    Transform::from_translation(pos.with_z(5.0)),
                ));
            }
        }
    }

    // Spawn enemy tokens based on mission template
    let template_id = mission_info_q
        .iter()
        .next()
        .map(|info| info.template_id.clone());

    if let (Some(templates), Some(enemy_db), Some(template_id)) =
        (templates, enemy_db, template_id)
    {
        let template = templates.0.iter().find(|t| t.id == template_id);
        if let Some(template) = template {
            spawn_enemies(
                &mut commands,
                map,
                template,
                &enemy_db,
                char_sprites.as_deref(),
            );
        }
    }

    // Initialize room status
    let mut room_status = RoomStatus {
        visited: vec![false; map.rooms.len()],
        cleared: vec![false; map.rooms.len()],
    };
    // Entrance is visited from the start
    if let Some(entrance_idx) = map.rooms.iter().position(|r| r.room_type == RoomType::Entrance) {
        room_status.visited[entrance_idx] = true;
    }
    commands.insert_resource(room_status);
    commands.init_resource::<SimulationSpeed>();
    commands.init_resource::<SimulationTimer>();
}

fn spawn_enemies(
    commands: &mut Commands,
    map: &DungeonMap,
    template: &super::data::MissionTemplate,
    enemy_db: &EnemyDatabase,
    char_sprites: Option<&super::tileset::CharacterSprites>,
) {
    let mut rng = rand::rng();

    // Get non-entrance rooms for enemy placement
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
            // Pick a random non-entrance room
            let room_idx = enemy_rooms[rng.random_range(0..enemy_rooms.len())];
            let room = &map.rooms[room_idx];

            // Random position within the room
            let ex = room.x + rng.random_range(0..room.w);
            let ey = room.y + rng.random_range(0..room.h);

            let pos = tile_world_pos(ex, ey);

            if let Some(char_sprites) = char_sprites {
                let entry = char_sprites.for_enemy(enemy_type);
                commands.spawn((
                    Name::new(format!("Enemy: {}", enemy_def.name)),
                    MissionEntity,
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
                    Sprite {
                        image: entry.texture.clone(),
                        texture_atlas: Some(TextureAtlas {
                            layout: entry.layout.clone(),
                            index: 0,
                        }),
                        ..default()
                    },
                    super::tileset::SpriteAnimation::new(entry.frame_count),
                    Transform::from_translation(pos.with_z(4.0)),
                ));
            } else {
                let color = enemy_color(enemy_type);
                commands.spawn((
                    Name::new(format!("Enemy: {}", enemy_def.name)),
                    MissionEntity,
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
                    Sprite {
                        color,
                        custom_size: Some(Vec2::splat(20.0)),
                        ..default()
                    },
                    Transform::from_translation(pos.with_z(4.0)),
                ));
            }
        }
    }
}

/// Advance the simulation timer and trigger movement ticks.
pub fn simulation_tick(
    time: Res<Time>,
    speed: Res<SimulationSpeed>,
    mut timer: ResMut<SimulationTimer>,
    dungeon: Option<Res<crate::screens::mission_view::ActiveDungeon>>,
    mut heroes: Query<
        (&mut GridPosition, &mut MoveTarget, &mut InRoom),
        (With<HeroToken>, Without<EnemyToken>),
    >,
) {
    let Some(dungeon) = dungeon else { return };
    let map = &dungeon.0;

    // Reset tick flag at start of each frame
    timer.ticked = false;

    timer.elapsed += time.delta_secs() * speed.0;

    if timer.elapsed < TICK_INTERVAL {
        return;
    }
    timer.elapsed -= TICK_INTERVAL;
    timer.ticked = true;

    // Move heroes along their paths
    for (mut grid_pos, mut target, mut in_room) in &mut heroes {
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

/// Sync sprite transforms from grid positions with smooth interpolation.
pub fn sync_sprite_positions(
    time: Res<Time>,
    speed: Res<SimulationSpeed>,
    timer: Res<SimulationTimer>,
    mut query: Query<(&GridPosition, &mut Transform), With<MissionEntity>>,
) {
    // Lerp factor: how far through the current tick we are
    let _lerp_t = (timer.elapsed / TICK_INTERVAL).clamp(0.0, 1.0);

    for (grid_pos, mut transform) in &mut query {
        let target_pos = tile_world_pos(grid_pos.x, grid_pos.y);
        let target_with_z = target_pos.with_z(transform.translation.z);

        // Smooth interpolation toward target
        let speed_factor = 8.0 * speed.0.max(1.0);
        transform.translation = transform
            .translation
            .lerp(target_with_z, (time.delta_secs() * speed_factor).min(1.0));
    }
}

/// Clean up mission entities, hero status, and resources when leaving the mission view.
pub fn cleanup_mission_entities(
    mut commands: Commands,
    entities: Query<Entity, With<MissionEntity>>,
    missions: Query<(Entity, &MissionParty), With<Mission>>,
) {
    // Despawn all mission-scoped sprites/tokens
    for entity in &entities {
        commands.entity(entity).despawn();
    }

    // Remove OnMission from party heroes and despawn mission entity
    for (mission_entity, party) in &missions {
        for &hero_entity in &party.0 {
            commands.entity(hero_entity).remove::<super::OnMission>();
        }
        commands.entity(mission_entity).despawn();
    }

    // Clean up resources
    commands.remove_resource::<RoomStatus>();
    commands.remove_resource::<SimulationSpeed>();
    commands.remove_resource::<SimulationTimer>();
    commands.remove_resource::<crate::screens::mission_view::ActiveDungeon>();
}

// ── Helpers ─────────────────────────────────────────────────────────

/// Convert grid coordinates to world position.
fn tile_world_pos(x: u32, y: u32) -> Vec3 {
    Vec3::new(
        x as f32 * TILE_SIZE + TILE_SIZE / 2.0,
        -(y as f32 * TILE_SIZE + TILE_SIZE / 2.0),
        0.0,
    )
}

/// Get color for a hero token based on class.
fn hero_color(class: &crate::hero::data::HeroClass) -> Color {
    use crate::hero::data::HeroClass;
    match class {
        HeroClass::Warrior => Color::srgb(0.9, 0.2, 0.2),  // Red
        HeroClass::Rogue => Color::srgb(0.6, 0.2, 0.8),    // Purple
        HeroClass::Mage => Color::srgb(0.2, 0.4, 0.9),     // Blue
        HeroClass::Cleric => Color::srgb(0.9, 0.8, 0.2),   // Gold
        HeroClass::Ranger => Color::srgb(0.2, 0.8, 0.3),   // Green
    }
}

/// Get color for an enemy token based on type.
fn enemy_color(enemy_type: EnemyType) -> Color {
    match enemy_type {
        EnemyType::Goblin => Color::srgb(0.3, 0.7, 0.2),   // Green
        EnemyType::Skeleton => Color::srgb(0.9, 0.9, 0.85), // White
        EnemyType::Slime => Color::srgb(0.5, 0.9, 0.3),    // Lime
        EnemyType::Orc => Color::srgb(0.5, 0.15, 0.1),     // Dark red
        EnemyType::BossRat => Color::srgb(0.5, 0.35, 0.2),  // Brown
    }
}
