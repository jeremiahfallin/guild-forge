//! The mission view screen — renders the dungeon tiles and spawns render
//! proxies for the viewed mission's tokens. The simulation itself runs
//! headlessly in `FixedUpdate`; this screen is purely a visual window.

use bevy::prelude::*;
use bevy::camera::ScalingMode;
use rand::Rng;

use bevy_declarative::style::styled::Styled;

use crate::{
    mission::{
        Mission, MissionDungeon, MissionInfo, MissionParty, ViewedMission,
        entities::{
            CombatStats, EnemyToken, GridPosition, HeroToken, RenderProxyOf,
            VisualPathQueue, hero_color, enemy_color, tile_world_pos, LootChest,
            VisualHit, ProcessedDeath,
        },
        tileset::{CharacterSprites, SpriteAnimation},
    },
    hero::{Hero, HeroInfo, portrait::HeroPortraitImage},
    screens::GameTab,
    theme::{widgets, palette::*},
    ui::feed::MissionLogHistory,
};
use bevy_declarative::style::values::px;

/// Tile size in world pixels.
const TILE_SIZE: f32 = 32.0;

pub(super) fn plugin(app: &mut App) {
    app.add_systems(OnEnter(GameTab::MissionView), spawn_mission_view);
    app.add_systems(OnExit(GameTab::MissionView), cleanup_mission_view);
    app.add_systems(
        Update,
        (
            rebuild_view_on_viewed_change,
            bounce_to_missions_if_viewed_despawned,
            update_mission_feed_ui,
            handle_visual_hits,
            update_floating_text,
            update_hit_flashes,
            update_visual_nudges,
            update_screen_shake,
            spawn_death_poofs,
            update_death_particles,
            // Flush despawns from rebuild/bounce before health bars try to
            // attach children to proxies that may have just been destroyed.
            ApplyDeferred,
            update_health_bars,
        )
            .chain()
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
    missions: Query<(&MissionDungeon, &Children, Option<&MissionLogHistory>, &MissionInfo), With<Mission>>,
    hero_tokens: Query<(&HeroToken, &GridPosition, &CombatStats), Without<EnemyToken>>,
    enemy_tokens: Query<(&EnemyToken, &GridPosition, &CombatStats), Without<HeroToken>>,
    chests: Query<(&GridPosition, &LootChest)>,
    hero_info_q: Query<&HeroInfo, With<Hero>>,
    hero_portraits: Query<(&HeroInfo, &HeroPortraitImage)>,
    mut camera_q: Query<(&mut Transform, &mut Projection), With<Camera2d>>,
    tileset: Option<Res<crate::mission::tileset::DungeonTileset>>,
    char_sprites: Option<Res<CharacterSprites>>,
) {
    let Ok(root_entity) = gameplay_root.single() else { return };
    let Some(viewed) = viewed else { return };
    let Ok((dungeon, children, maybe_history, info)) = missions.get(viewed.0) else { return };
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
                let tint_color = match info.biome {
                    crate::mission::data::BiomeType::Dungeon => Color::WHITE,
                    crate::mission::data::BiomeType::Crypt => Color::srgb(0.55, 0.45, 0.75),
                };
                commands
                    .spawn((
                        Name::new(format!("Tile({x},{y})")),
                        Sprite {
                            image: tileset.texture.clone(),
                            texture_atlas: Some(TextureAtlas {
                                layout: tileset.layout.clone(),
                                index: tile_idx as usize,
                            }),
                            color: tint_color,
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
        &chests,
        &hero_info_q,
        &char_sprites,
    );

    // Fit camera to dungeon
    fit_camera_to_dungeon(map, &mut camera_q);

    // Spawn UI overlay with feed panel
    let default_history = MissionLogHistory::default();
    let history = maybe_history.unwrap_or(&default_history);
    spawn_mission_view_ui(&mut commands, root_entity, history, &hero_portraits);
}

/// Spawn render proxy entities for all tokens under a mission.
fn spawn_proxies(
    commands: &mut Commands,
    children: &Children,
    hero_tokens: &Query<(&HeroToken, &GridPosition, &CombatStats), Without<EnemyToken>>,
    enemy_tokens: &Query<(&EnemyToken, &GridPosition, &CombatStats), Without<HeroToken>>,
    chests: &Query<(&GridPosition, &LootChest)>,
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
                    VisualPathQueue::default(),
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
                    VisualPathQueue::default(),
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
                    VisualPathQueue::default(),
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
                    VisualPathQueue::default(),
                    Sprite {
                        color: enemy_color(enemy_token.enemy_type),
                        custom_size: Some(Vec2::splat(TILE_SIZE * 0.5)),
                        ..default()
                    },
                    Transform::from_translation(world_pos.with_z(1.5)),
                ));
            }
        } else if let Ok((grid_pos, chest)) = chests.get(child) {
            let world_pos = tile_world_pos(grid_pos.x, grid_pos.y);
            commands.spawn((
                Name::new("Chest Proxy"),
                RenderProxyOf(child),
                VisualPathQueue::default(),
                Sprite {
                    color: if chest.opened { Color::srgb(0.4, 0.4, 0.4) } else { Color::srgb(0.85, 0.65, 0.15) },
                    custom_size: Some(Vec2::splat(TILE_SIZE * 0.4)),
                    ..default()
                },
                Transform::from_translation(world_pos.with_z(1.0)),
            ));
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
    missions: Query<(&MissionDungeon, &Children, Option<&MissionLogHistory>, &MissionInfo), With<Mission>>,
    hero_tokens: Query<(&HeroToken, &GridPosition, &CombatStats), Without<EnemyToken>>,
    enemy_tokens: Query<(&EnemyToken, &GridPosition, &CombatStats), Without<HeroToken>>,
    chests: Query<(&GridPosition, &LootChest)>,
    hero_info_q: Query<&HeroInfo, With<Hero>>,
    hero_portraits: Query<(&HeroInfo, &HeroPortraitImage)>,
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
    let Ok((dungeon, children, maybe_history, info)) = missions.get(viewed.0) else { return };
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
                let tint_color = match info.biome {
                    crate::mission::data::BiomeType::Dungeon => Color::WHITE,
                    crate::mission::data::BiomeType::Crypt => Color::srgb(0.55, 0.45, 0.75),
                };
                commands
                    .spawn((
                        Name::new(format!("Tile({x},{y})")),
                        Sprite {
                            image: tileset.texture.clone(),
                            texture_atlas: Some(TextureAtlas {
                                layout: tileset.layout.clone(),
                                index: tile_idx as usize,
                            }),
                            color: tint_color,
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
        &chests,
        &hero_info_q,
        &char_sprites,
    );

    fit_camera_to_dungeon(map, &mut camera_q);

    // Re-spawn UI overlay with feed panel
    let default_history = MissionLogHistory::default();
    let history = maybe_history.unwrap_or(&default_history);
    spawn_mission_view_ui(&mut commands, root_entity, history, &hero_portraits);
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

fn spawn_mission_view_ui(
    commands: &mut Commands,
    root_entity: Entity,
    history: &MissionLogHistory,
    hero_portraits: &Query<(&HeroInfo, &HeroPortraitImage)>,
) {
    let mut feed_container = bevy_declarative::element::div::div()
        .absolute()
        .w(px(340.0))
        .h(px(420.0))
        .col()
        .gap(px(8.0))
        .p(px(12.0))
        .bg(Color::srgba(0.08, 0.08, 0.12, 0.85)) // Premium glassmorphism
        .rounded(px(8.0))
        .overflow_y_scroll()
        .insert((
            ScrollPosition(Vec2::new(0.0, 999999.0)), // Auto-scroll to bottom
            BorderColor::all(BORDER_BRONZE),
        ));
    feed_container.style_mut().border = UiRect::all(Val::Px(1.5));
    feed_container.style_mut().right = Val::Px(20.0);
    feed_container.style_mut().top = Val::Px(20.0);

    feed_container = feed_container.child(
        bevy_declarative::element::text::text("Mission Log")
            .font_size(18.0)
            .color(HEADER_TEXT)
            .insert(Pickable::IGNORE),
    );

    // Add log entries
    for entry in &history.logs {
        let color = match entry.kind {
            crate::ui::feed::LogKind::Combat => Color::srgb(0.9, 0.9, 0.95), // Muted off-white
            crate::ui::feed::LogKind::Heal => Color::srgb(0.3, 0.8, 0.3),   // Green
            crate::ui::feed::LogKind::RoomEntry => Color::srgb(0.95, 0.75, 0.15), // Gold
            crate::ui::feed::LogKind::Completion => Color::srgb(0.95, 0.65, 0.15), // Vibrant gold
            crate::ui::feed::LogKind::Failure => Color::srgb(0.9, 0.2, 0.2), // Red
            crate::ui::feed::LogKind::Death => Color::srgb(0.85, 0.35, 0.35), // Soft red/pink
            crate::ui::feed::LogKind::Info => Color::srgb(0.65, 0.65, 0.7), // Muted grey
            crate::ui::feed::LogKind::Legendary => Color::srgb(1.0, 0.5, 0.0), // Vibrant orange
        };

        let mut row = bevy_declarative::element::div::div()
            .row()
            .items_center()
            .gap(px(6.0));

        let mut has_portrait = false;
        if let Some(ref name) = entry.hero_name {
            let mut portrait_handle = None;
            for (info, port_img) in hero_portraits.iter() {
                if info.name == *name || name.contains(&info.name) {
                    portrait_handle = Some(&port_img.0);
                    break;
                }
            }
            if let Some(handle) = portrait_handle {
                has_portrait = true;
                row = row.child(
                    bevy_declarative::element::div::div()
                        .size(px(20.0))
                        .bg(Color::srgb(0.08, 0.08, 0.1))
                        .rounded(px(2.0))
                        .insert(ImageNode {
                            image: handle.clone(),
                            ..default()
                        })
                );
            }
        }

        if !has_portrait {
            row = row.child(
                bevy_declarative::element::div::div()
                    .size(px(20.0))
            );
        }

        row = row.child(
            bevy_declarative::element::text::text(entry.text.clone())
                .font_size(13.0)
                .color(color)
                .insert(Pickable::IGNORE),
        );

        feed_container = feed_container.child(row);
    }

    widgets::content_area("Mission View UI")
        .insert((MissionViewUi, GlobalZIndex(10)))
        .child(feed_container)
        .child(
            bevy_declarative::element::div::div()
                .absolute()
                .w_full()
                .row()
                .justify_center()
                .insert(Node {
                    bottom: bevy::ui::Val::Px(20.0),
                    ..default()
                })
                .child(widgets::game_button("Abort Mission", abort_mission)),
        )
        .spawn_as_child_of(commands, root_entity);
}

fn update_mission_feed_ui(
    mut commands: Commands,
    viewed: Option<Res<ViewedMission>>,
    missions: Query<&MissionLogHistory, (With<Mission>, Changed<MissionLogHistory>)>,
    gameplay_root: Query<Entity, With<widgets::GameplayRoot>>,
    ui_q: Query<Entity, With<MissionViewUi>>,
    hero_portraits: Query<(&HeroInfo, &HeroPortraitImage)>,
) {
    let Some(viewed) = viewed else { return };
    let Ok(history) = missions.get(viewed.0) else { return };
    let Ok(root_entity) = gameplay_root.single() else { return };

    // Despawn old UI overlay
    for entity in &ui_q {
        commands.entity(entity).despawn();
    }

    // Spawn new UI overlay with updated feed
    spawn_mission_view_ui(&mut commands, root_entity, history, &hero_portraits);
}

// ── Visual Effects Components ──

#[derive(Component)]
pub struct FloatingText {
    pub velocity: Vec2,
    pub timer: f32,
    pub duration: f32,
}

#[derive(Component)]
pub struct HitFlash {
    pub timer: f32,
    pub duration: f32,
    pub original_color: Color,
}

#[derive(Component)]
pub struct VisualNudge {
    pub offset: Vec2,
    pub timer: f32,
    pub duration: f32,
}

#[derive(Component)]
pub struct CameraShake {
    pub intensity: f32,
    pub duration: f32,
    pub timer: f32,
    pub current_offset: Vec2,
}

#[derive(Component)]
pub struct DeathParticle {
    pub velocity: Vec2,
    pub timer: f32,
    pub duration: f32,
}

// ── Visual Effects Systems ──

fn handle_visual_hits(
    mut commands: Commands,
    tokens: Query<(Entity, &GridPosition), Or<(With<HeroToken>, With<EnemyToken>)>>,
    mut hit_tokens: Query<(Entity, &VisualHit, &GridPosition, Option<&Name>)>,
    mut proxies: Query<(Entity, &RenderProxyOf, &mut Transform, &mut Sprite, Option<&mut HitFlash>, Option<&mut VisualNudge>)>,
    camera_q: Query<Entity, With<Camera2d>>,
) {
    let camera_ent = camera_q.iter().next();

    for (token_entity, hit, token_gp, _maybe_name) in &mut hit_tokens {
        if let Some((proxy_entity, _, transform, mut sprite, maybe_flash, maybe_nudge)) =
            proxies.iter_mut().find(|(_, p, _, _, _, _)| p.0 == token_entity)
        {
            let text_pos = transform.translation + Vec3::new(0.0, 16.0, 10.0);

            let text_val = match hit.effect_type.as_str() {
                "Heal" => format!("+{}", hit.amount),
                "Shield" => format!("+{} Shield", hit.amount),
                _ => {
                    if hit.is_hit {
                        if hit.is_crit {
                            format!("{} Critical!", hit.amount)
                        } else {
                            format!("{}", hit.amount)
                        }
                    } else {
                        "Miss".to_string()
                    }
                }
            };

            let text_color = match hit.effect_type.as_str() {
                "Heal" => Color::srgb(0.2, 0.8, 0.2),
                "Shield" => Color::srgb(0.2, 0.7, 0.8),
                _ => {
                    if hit.is_hit {
                        if hit.is_crit {
                            Color::srgb(0.95, 0.5, 0.1)
                        } else {
                            Color::srgb(0.9, 0.2, 0.2)
                        }
                    } else {
                        Color::srgb(0.6, 0.6, 0.6)
                    }
                }
            };

            let font_size = if hit.is_crit { 18.0 } else { 12.0 };

            commands.spawn((
                Name::new("Floating Damage Number"),
                Text::new(text_val),
                TextFont {
                    font_size,
                    ..default()
                },
                TextColor(text_color),
                Transform::from_translation(text_pos),
                FloatingText {
                    velocity: Vec2::new(0.0, 30.0),
                    timer: 0.0,
                    duration: 0.8,
                },
            ));

            let flash_color = match hit.effect_type.as_str() {
                "Heal" => Color::srgb(0.2, 1.5, 0.2),
                "Shield" => Color::srgb(0.2, 1.2, 1.5),
                _ => {
                    if hit.is_hit {
                        if hit.is_crit {
                            Color::srgb(2.5, 2.5, 2.5)
                        } else {
                            Color::srgb(1.8, 0.2, 0.2)
                        }
                    } else {
                        Color::srgb(1.0, 1.0, 1.0)
                    }
                }
            };

            if hit.is_hit || hit.effect_type == "Heal" || hit.effect_type == "Shield" {
                if let Some(mut existing_flash) = maybe_flash {
                    existing_flash.timer = 0.0;
                } else {
                    commands.entity(proxy_entity).insert(HitFlash {
                        timer: 0.0,
                        duration: 0.15,
                        original_color: sprite.color,
                    });
                }
                sprite.color = flash_color;
            }

            if hit.is_hit && hit.effect_type == "Damage" {
                let mut direction = Vec2::ZERO;
                if let Some(source_entity) = hit.source {
                    if let Ok((_, source_gp)) = tokens.get(source_entity) {
                        let diff = Vec2::new(
                            token_gp.x as f32 - source_gp.x as f32,
                            token_gp.y as f32 - source_gp.y as f32,
                        );
                        if diff.length_squared() > 0.01 {
                            direction = diff.normalize();
                        }
                    }
                }
                if direction == Vec2::ZERO {
                    let angle = rand::rng().random_range(0.0..std::f32::consts::TAU);
                    direction = Vec2::new(angle.cos(), angle.sin());
                }

                let intensity = if hit.is_crit { 16.0 } else { 8.0 };
                let initial_offset = direction * intensity;

                if let Some(mut existing_nudge) = maybe_nudge {
                    existing_nudge.timer = 0.0;
                    existing_nudge.offset = initial_offset;
                } else {
                    commands.entity(proxy_entity).insert(VisualNudge {
                        offset: initial_offset,
                        timer: 0.0,
                        duration: 0.15,
                    });
                }
            }

            if hit.is_signature {
                if let Some(camera_ent) = camera_ent {
                    let shake_intensity = if hit.is_crit { 6.0 } else { 4.0 };
                    commands.entity(camera_ent).insert(CameraShake {
                        intensity: shake_intensity,
                        duration: 0.3,
                        timer: 0.0,
                        current_offset: Vec2::ZERO,
                    });
                }
            }
        }

        commands.entity(token_entity).remove::<VisualHit>();
    }
}

fn update_floating_text(
    mut commands: Commands,
    time: Res<Time>,
    mut q: Query<(Entity, &mut Transform, &mut TextColor, &mut FloatingText)>,
) {
    let dt = time.delta_secs();
    for (entity, mut transform, mut color, mut ft) in &mut q {
        transform.translation += ft.velocity.extend(0.0) * dt;
        ft.timer += dt;
        let alpha = 1.0 - (ft.timer / ft.duration).clamp(0.0, 1.0);
        color.0.set_alpha(alpha);
        if ft.timer >= ft.duration {
            commands.entity(entity).despawn();
        }
    }
}

fn update_hit_flashes(
    mut commands: Commands,
    time: Res<Time>,
    mut q: Query<(Entity, &mut Sprite, &mut HitFlash)>,
) {
    let dt = time.delta_secs();
    for (entity, mut sprite, mut flash) in &mut q {
        flash.timer += dt;
        if flash.timer >= flash.duration {
            sprite.color = flash.original_color;
            commands.entity(entity).remove::<HitFlash>();
        }
    }
}

fn update_visual_nudges(
    mut commands: Commands,
    time: Res<Time>,
    mut q: Query<(Entity, &mut Transform, &mut VisualNudge)>,
) {
    let dt = time.delta_secs();
    for (entity, mut transform, mut nudge) in &mut q {
        transform.translation -= nudge.offset.extend(0.0);
        
        nudge.timer += dt;
        if nudge.timer >= nudge.duration {
            commands.entity(entity).remove::<VisualNudge>();
        } else {
            let pct = 1.0 - (nudge.timer / nudge.duration);
            nudge.offset *= pct;
            transform.translation += nudge.offset.extend(0.0);
        }
    }
}

fn update_screen_shake(
    mut commands: Commands,
    time: Res<Time>,
    mut camera_q: Query<(Entity, &mut Transform, &mut CameraShake), With<Camera2d>>,
) {
    let dt = time.delta_secs();
    for (entity, mut transform, mut shake) in &mut camera_q {
        transform.translation -= shake.current_offset.extend(0.0);

        shake.timer += dt;
        if shake.timer >= shake.duration {
            commands.entity(entity).remove::<CameraShake>();
        } else {
            let decay = 1.0 - (shake.timer / shake.duration);
            let current_intensity = shake.intensity * decay;
            
            let mut rng = rand::rng();
            let ox = rng.random_range(-1.0..=1.0) * current_intensity;
            let oy = rng.random_range(-1.0..=1.0) * current_intensity;
            
            shake.current_offset = Vec2::new(ox, oy);
            transform.translation += shake.current_offset.extend(0.0);
        }
    }
}

fn spawn_death_poofs(
    mut commands: Commands,
    tokens: Query<(Entity, &CombatStats, &GridPosition), (Or<(With<HeroToken>, With<EnemyToken>)>, Without<ProcessedDeath>)>,
    proxies: Query<(&RenderProxyOf, &Transform)>,
) {
    for (token_entity, stats, grid_pos) in &tokens {
        if stats.hp <= 0 {
            commands.entity(token_entity).insert(ProcessedDeath);

            let mut pos = tile_world_pos(grid_pos.x, grid_pos.y).with_z(3.0);
            for (proxy_of, transform) in &proxies {
                if proxy_of.0 == token_entity {
                    pos = transform.translation.with_z(3.0);
                    break;
                }
            }

            let mut rng = rand::rng();
            for _ in 0..10 {
                let angle = rng.random_range(0.0..std::f32::consts::TAU);
                let speed = rng.random_range(15.0..45.0);
                let velocity = Vec2::new(angle.cos() * speed, angle.sin() * speed);
                
                commands.spawn((
                    Name::new("Death Particle"),
                    Sprite {
                        color: Color::srgba(0.8, 0.8, 0.8, 0.8),
                        custom_size: Some(Vec2::splat(rng.random_range(3.0..6.0))),
                        ..default()
                    },
                    Transform::from_translation(pos),
                    DeathParticle {
                        velocity,
                        timer: 0.0,
                        duration: rng.random_range(0.3..0.6),
                    },
                ));
            }
        }
    }
}

fn update_death_particles(
    mut commands: Commands,
    time: Res<Time>,
    mut q: Query<(Entity, &mut Transform, &mut Sprite, &mut DeathParticle)>,
) {
    let dt = time.delta_secs();
    for (entity, mut transform, mut sprite, mut particle) in &mut q {
        transform.translation += particle.velocity.extend(0.0) * dt;
        particle.velocity *= 0.93;
        
        particle.timer += dt;
        if particle.timer >= particle.duration {
            commands.entity(entity).despawn();
        } else {
            let pct = 1.0 - (particle.timer / particle.duration).clamp(0.0, 1.0);
            sprite.color.set_alpha(pct * 0.8);
        }
    }
}
