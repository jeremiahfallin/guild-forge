//! The mission view screen — renders the dungeon tiles and spawns render
//! proxies for the viewed mission's tokens. The simulation itself runs
//! headlessly in `FixedUpdate`; this screen is purely a visual window.

use bevy::prelude::*;
use bevy::camera::ScalingMode;

use bevy_declarative::style::styled::Styled;

use crate::{
    mission::{
        Mission, MissionDungeon, MissionParty, ViewedMission,
        entities::{
            CombatStats, EnemyToken, GridPosition, HeroToken, RenderProxyOf,
            hero_color, enemy_color, tile_world_pos,
        },
        tileset::{CharacterSprites, SpriteAnimation},
    },
    hero::{Hero, HeroInfo},
    screens::GameTab,
    theme::widgets,
};

/// Tile size in world pixels.
const TILE_SIZE: f32 = 32.0;

pub(super) fn plugin(app: &mut App) {
    app.add_systems(OnEnter(GameTab::MissionView), spawn_mission_view);
    app.add_systems(OnExit(GameTab::MissionView), cleanup_mission_view);
    app.add_systems(
        Update,
        (
            rebuild_view_on_viewed_change,
            update_health_bars,
            bounce_to_missions_if_viewed_despawned,
        )
            .run_if(in_state(GameTab::MissionView)),
    );
}

/// Marker for the dungeon root entity.
#[derive(Component)]
struct DungeonRoot;

/// Marker for the mission view UI overlay.
#[derive(Component)]
struct MissionViewUi;

fn spawn_mission_view(
    mut commands: Commands,
    gameplay_root: Query<Entity, With<widgets::GameplayRoot>>,
    viewed: Option<Res<ViewedMission>>,
    missions: Query<(&MissionDungeon, &Children), With<Mission>>,
    hero_tokens: Query<(&HeroToken, &GridPosition, &CombatStats), Without<EnemyToken>>,
    enemy_tokens: Query<(&EnemyToken, &GridPosition, &CombatStats), Without<HeroToken>>,
    hero_info_q: Query<&HeroInfo, With<Hero>>,
    mut camera_q: Query<(&mut Transform, &mut Projection), With<Camera2d>>,
    tileset: Option<Res<crate::mission::tileset::DungeonTileset>>,
    char_sprites: Option<Res<CharacterSprites>>,
) {
    let Ok(root_entity) = gameplay_root.single() else { return };
    let Some(viewed) = viewed else { return };
    let Ok((dungeon, children)) = missions.get(viewed.0) else { return };
    let map = &dungeon.0;

    // Spawn dungeon tiles
    let tile_root = commands
        .spawn((
            Name::new("Dungeon Root"),
            DungeonRoot,
            Transform::default(),
            Visibility::default(),
        ))
        .id();

    for y in 0..map.height {
        for x in 0..map.width {
            let pos = tile_world_pos(x, y);
            let child = if let Some(ref tileset) = tileset {
                let tile_idx = crate::mission::tileset::autotile_index(map, x, y);
                commands
                    .spawn((
                        Name::new(format!("Tile({x},{y})")),
                        Sprite {
                            image: tileset.texture.clone(),
                            texture_atlas: Some(TextureAtlas {
                                layout: tileset.layout.clone(),
                                index: tile_idx as usize,
                            }),
                            ..default()
                        },
                        Transform::from_translation(pos),
                    ))
                    .id()
            } else {
                let tile = map.get(x, y);
                let color = tile_color(tile, map, x, y);
                commands
                    .spawn((
                        Name::new(format!("Tile({x},{y})")),
                        Sprite {
                            color,
                            custom_size: Some(Vec2::splat(TILE_SIZE)),
                            ..default()
                        },
                        Transform::from_translation(pos),
                    ))
                    .id()
            };
            commands.entity(tile_root).add_child(child);
        }
    }

    // Spawn render proxies for mission tokens
    spawn_proxies(
        &mut commands,
        children,
        &hero_tokens,
        &enemy_tokens,
        &hero_info_q,
        &char_sprites,
    );

    // Fit camera to dungeon
    fit_camera_to_dungeon(map, &mut camera_q);

    // Spawn UI overlay — abort button
    widgets::content_area("Mission View UI")
        .insert((MissionViewUi, GlobalZIndex(10)))
        .child(
            bevy_declarative::element::div::div()
                .absolute()
                .insert(Node {
                    bottom: bevy::ui::Val::Px(20.0),
                    ..default()
                })
                .child(widgets::game_button("Abort Mission", abort_mission)),
        )
        .spawn_as_child_of(&mut commands, root_entity);
}

/// Spawn render proxy entities for all tokens under a mission.
fn spawn_proxies(
    commands: &mut Commands,
    children: &Children,
    hero_tokens: &Query<(&HeroToken, &GridPosition, &CombatStats), Without<EnemyToken>>,
    enemy_tokens: &Query<(&EnemyToken, &GridPosition, &CombatStats), Without<HeroToken>>,
    hero_info_q: &Query<&HeroInfo, With<Hero>>,
    char_sprites: &Option<Res<CharacterSprites>>,
) {
    for child in children.iter() {
        if let Ok((hero_token, grid_pos, combat)) = hero_tokens.get(child) {
            if combat.hp <= 0 {
                continue;
            }
            let world_pos = tile_world_pos(grid_pos.x, grid_pos.y);
            let class = hero_info_q
                .get(hero_token.0)
                .map(|i| i.class)
                .unwrap_or(crate::hero::data::HeroClass::Warrior);

            if let Some(sprites) = char_sprites {
                let entry = &sprites.hero;
                commands.spawn((
                    Name::new("Hero Proxy"),
                    RenderProxyOf(child),
                    DungeonRoot, // so cleanup catches it
                    Sprite {
                        image: entry.texture.clone(),
                        texture_atlas: Some(TextureAtlas {
                            layout: entry.layout.clone(),
                            index: 0,
                        }),
                        ..default()
                    },
                    Transform::from_translation(world_pos.with_z(2.0)),
                    SpriteAnimation::new(entry.frame_count),
                ));
            } else {
                commands.spawn((
                    Name::new("Hero Proxy"),
                    RenderProxyOf(child),
                    DungeonRoot,
                    Sprite {
                        color: hero_color(&class),
                        custom_size: Some(Vec2::splat(TILE_SIZE * 0.6)),
                        ..default()
                    },
                    Transform::from_translation(world_pos.with_z(2.0)),
                ));
            }
        } else if let Ok((enemy_token, grid_pos, combat)) = enemy_tokens.get(child) {
            if combat.hp <= 0 {
                continue;
            }
            let world_pos = tile_world_pos(grid_pos.x, grid_pos.y);

            if let Some(sprites) = char_sprites {
                let entry = sprites.for_enemy(enemy_token.enemy_type);
                commands.spawn((
                    Name::new("Enemy Proxy"),
                    RenderProxyOf(child),
                    DungeonRoot,
                    Sprite {
                        image: entry.texture.clone(),
                        texture_atlas: Some(TextureAtlas {
                            layout: entry.layout.clone(),
                            index: 0,
                        }),
                        ..default()
                    },
                    Transform::from_translation(world_pos.with_z(1.5)),
                    SpriteAnimation::new(entry.frame_count),
                ));
            } else {
                commands.spawn((
                    Name::new("Enemy Proxy"),
                    RenderProxyOf(child),
                    DungeonRoot,
                    Sprite {
                        color: enemy_color(enemy_token.enemy_type),
                        custom_size: Some(Vec2::splat(TILE_SIZE * 0.5)),
                        ..default()
                    },
                    Transform::from_translation(world_pos.with_z(1.5)),
                ));
            }
        }
    }
}

fn cleanup_mission_view(
    mut commands: Commands,
    dungeon_q: Query<Entity, With<DungeonRoot>>,
    ui_q: Query<Entity, With<MissionViewUi>>,
    proxy_q: Query<Entity, With<RenderProxyOf>>,
    mut camera_q: Query<(&mut Transform, &mut Projection), With<Camera2d>>,
) {
    for entity in &dungeon_q {
        commands.entity(entity).despawn();
    }
    for entity in &ui_q {
        commands.entity(entity).despawn();
    }
    for entity in &proxy_q {
        commands.entity(entity).despawn();
    }
    // Reset camera
    if let Ok((mut transform, mut projection)) = camera_q.single_mut() {
        *transform = Transform::default();
        if let Projection::Orthographic(ref mut ortho) = *projection {
            ortho.scaling_mode = ScalingMode::WindowSize;
            ortho.scale = 1.0;
        }
    }
}

/// When `ViewedMission` changes, tear down and rebuild the view in-place.
fn rebuild_view_on_viewed_change(
    mut commands: Commands,
    viewed: Option<Res<ViewedMission>>,
    dungeon_q: Query<Entity, With<DungeonRoot>>,
    ui_q: Query<Entity, With<MissionViewUi>>,
    proxy_q: Query<Entity, With<RenderProxyOf>>,
    gameplay_root: Query<Entity, With<widgets::GameplayRoot>>,
    missions: Query<(&MissionDungeon, &Children), With<Mission>>,
    hero_tokens: Query<(&HeroToken, &GridPosition, &CombatStats), Without<EnemyToken>>,
    enemy_tokens: Query<(&EnemyToken, &GridPosition, &CombatStats), Without<HeroToken>>,
    hero_info_q: Query<&HeroInfo, With<Hero>>,
    mut camera_q: Query<(&mut Transform, &mut Projection), With<Camera2d>>,
    tileset: Option<Res<crate::mission::tileset::DungeonTileset>>,
    char_sprites: Option<Res<CharacterSprites>>,
) {
    let Some(viewed) = viewed else { return };
    if !viewed.is_changed() || viewed.is_added() {
        return;
    }

    // Cleanup old view
    for entity in &dungeon_q {
        commands.entity(entity).despawn();
    }
    for entity in &ui_q {
        commands.entity(entity).despawn();
    }
    for entity in &proxy_q {
        commands.entity(entity).despawn();
    }

    let Ok(root_entity) = gameplay_root.single() else { return };
    let Ok((dungeon, children)) = missions.get(viewed.0) else { return };
    let map = &dungeon.0;

    // Re-spawn tiles
    let tile_root = commands
        .spawn((
            Name::new("Dungeon Root"),
            DungeonRoot,
            Transform::default(),
            Visibility::default(),
        ))
        .id();

    for y in 0..map.height {
        for x in 0..map.width {
            let pos = tile_world_pos(x, y);
            let child = if let Some(ref tileset) = tileset {
                let tile_idx = crate::mission::tileset::autotile_index(map, x, y);
                commands
                    .spawn((
                        Name::new(format!("Tile({x},{y})")),
                        Sprite {
                            image: tileset.texture.clone(),
                            texture_atlas: Some(TextureAtlas {
                                layout: tileset.layout.clone(),
                                index: tile_idx as usize,
                            }),
                            ..default()
                        },
                        Transform::from_translation(pos),
                    ))
                    .id()
            } else {
                let tile = map.get(x, y);
                let color = tile_color(tile, map, x, y);
                commands
                    .spawn((
                        Name::new(format!("Tile({x},{y})")),
                        Sprite {
                            color,
                            custom_size: Some(Vec2::splat(TILE_SIZE)),
                            ..default()
                        },
                        Transform::from_translation(pos),
                    ))
                    .id()
            };
            commands.entity(tile_root).add_child(child);
        }
    }

    spawn_proxies(
        &mut commands,
        children,
        &hero_tokens,
        &enemy_tokens,
        &hero_info_q,
        &char_sprites,
    );

    fit_camera_to_dungeon(map, &mut camera_q);

    // Re-spawn UI overlay
    widgets::content_area("Mission View UI")
        .insert((MissionViewUi, GlobalZIndex(10)))
        .child(
            bevy_declarative::element::div::div()
                .absolute()
                .insert(Node {
                    bottom: bevy::ui::Val::Px(20.0),
                    ..default()
                })
                .child(widgets::game_button("Abort Mission", abort_mission)),
        )
        .spawn_as_child_of(&mut commands, root_entity);
}

/// If the viewed mission was despawned (completed/failed), bounce back.
fn bounce_to_missions_if_viewed_despawned(
    mut commands: Commands,
    viewed: Option<Res<ViewedMission>>,
    missions: Query<(), With<Mission>>,
    mut next_tab: ResMut<NextState<GameTab>>,
) {
    let Some(viewed) = viewed else { return };
    if missions.get(viewed.0).is_err() {
        commands.remove_resource::<ViewedMission>();
        next_tab.set(GameTab::Missions);
    }
}

/// Get the color for a tile based on its type and room context.
fn tile_color(
    tile: crate::mission::dungeon::Tile,
    map: &crate::mission::dungeon::DungeonMap,
    x: u32,
    y: u32,
) -> Color {
    use crate::mission::dungeon::{RoomType, Tile};
    match tile {
        Tile::Wall => Color::srgb(0.15, 0.15, 0.2),
        Tile::Floor => {
            if let Some(room_idx) = map.room_at(x, y) {
                match map.rooms[room_idx].room_type {
                    RoomType::Normal => Color::srgb(0.6, 0.5, 0.35),
                    RoomType::Entrance => Color::srgb(0.4, 0.6, 0.35),
                    RoomType::Boss => Color::srgb(0.65, 0.3, 0.3),
                    RoomType::Treasure => Color::srgb(0.6, 0.55, 0.2),
                }
            } else {
                Color::srgb(0.6, 0.5, 0.35)
            }
        }
        Tile::Door => Color::srgb(0.45, 0.3, 0.15),
        Tile::Corridor => Color::srgb(0.45, 0.38, 0.28),
    }
}

/// Fit the orthographic camera to show the full dungeon.
fn fit_camera_to_dungeon(
    map: &crate::mission::dungeon::DungeonMap,
    camera_q: &mut Query<(&mut Transform, &mut Projection), With<Camera2d>>,
) {
    let Ok((mut transform, mut projection)) = camera_q.single_mut() else {
        return;
    };
    let dungeon_width = map.width as f32 * TILE_SIZE;
    let dungeon_height = map.height as f32 * TILE_SIZE;
    transform.translation.x = dungeon_width / 2.0;
    transform.translation.y = -dungeon_height / 2.0;
    if let Projection::Orthographic(ref mut ortho) = *projection {
        let padding = 1.15;
        ortho.scaling_mode = ScalingMode::FixedVertical {
            viewport_height: dungeon_height * padding,
        };
        ortho.scale = 1.0;
    }
}

fn abort_mission(
    _: On<Pointer<Click>>,
    mut commands: Commands,
    viewed: Option<Res<crate::mission::ViewedMission>>,
    missions: Query<(Entity, &MissionParty), With<Mission>>,
    mut next_tab: ResMut<NextState<GameTab>>,
) {
    if let Some(viewed) = viewed {
        if let Ok((mission_entity, party)) = missions.get(viewed.0) {
            for &hero_entity in &party.0 {
                commands.entity(hero_entity).remove::<crate::mission::OnMission>();
            }
            commands.entity(mission_entity).despawn();
        }
        commands.remove_resource::<ViewedMission>();
    }
    next_tab.set(GameTab::Missions);
}

// ── Health bars ─────────────────────────────────────────────────────

/// Marker for health bar background sprite.
#[derive(Component)]
struct HealthBarBg;

/// Marker for health bar fill sprite. Stores the owning proxy entity.
#[derive(Component)]
struct HealthBarFill(#[allow(dead_code)] Entity);

const HEALTH_BAR_WIDTH: f32 = 24.0;
const HEALTH_BAR_HEIGHT: f32 = 3.0;
const HEALTH_BAR_Y_OFFSET: f32 = 16.0;

/// Spawn health bars for proxies, update existing ones.
fn update_health_bars(
    mut commands: Commands,
    proxies_without_bar: Query<
        (Entity, &RenderProxyOf),
        (Without<Children>,),
    >,
    proxies_with_bar: Query<
        (&RenderProxyOf, &Children),
    >,
    tokens: Query<&CombatStats, Or<(With<HeroToken>, With<EnemyToken>)>>,
    mut fills: Query<(&HealthBarFill, &mut Sprite, &mut Transform)>,
) {
    // Spawn health bars for proxies that don't have children yet
    for (proxy_entity, proxy_of) in &proxies_without_bar {
        let Ok(combat) = tokens.get(proxy_of.0) else {
            continue;
        };
        if combat.hp <= 0 {
            continue;
        }

        let bar_color = Color::srgb(0.2, 0.8, 0.2);

        let bg = commands
            .spawn((
                Name::new("HP Bar BG"),
                HealthBarBg,
                Sprite {
                    color: Color::srgba(0.0, 0.0, 0.0, 0.6),
                    custom_size: Some(Vec2::new(HEALTH_BAR_WIDTH, HEALTH_BAR_HEIGHT)),
                    ..default()
                },
                Transform::from_translation(Vec3::new(0.0, HEALTH_BAR_Y_OFFSET, 1.0)),
            ))
            .id();

        let fill = commands
            .spawn((
                Name::new("HP Bar Fill"),
                HealthBarFill(proxy_entity),
                Sprite {
                    color: bar_color,
                    custom_size: Some(Vec2::new(HEALTH_BAR_WIDTH, HEALTH_BAR_HEIGHT)),
                    ..default()
                },
                Transform::from_translation(Vec3::new(0.0, HEALTH_BAR_Y_OFFSET, 2.0)),
            ))
            .id();

        commands.entity(proxy_entity).add_children(&[bg, fill]);
    }

    // Update existing health bar fills
    for (proxy_of, children) in &proxies_with_bar {
        let Ok(combat) = tokens.get(proxy_of.0) else {
            continue;
        };
        let hp_pct = (combat.hp as f32 / combat.max_hp.max(1) as f32).clamp(0.0, 1.0);
        let bar_color = if hp_pct > 0.5 {
            Color::srgb(0.2, 0.8, 0.2)
        } else if hp_pct > 0.25 {
            Color::srgb(0.8, 0.7, 0.2)
        } else {
            Color::srgb(0.8, 0.2, 0.2)
        };

        for child in children.iter() {
            if let Ok((_fill, mut sprite, mut transform)) = fills.get_mut(child) {
                let fill_width = HEALTH_BAR_WIDTH * hp_pct;
                sprite.custom_size = Some(Vec2::new(fill_width, HEALTH_BAR_HEIGHT));
                sprite.color = bar_color;
                let offset_x = (HEALTH_BAR_WIDTH - fill_width) / -2.0;
                transform.translation.x = offset_x;
            }
        }
    }
}
