//! The mission view screen — renders the dungeon and shows the botwatch.

use bevy::prelude::*;
use bevy::camera::ScalingMode;

use bevy_declarative::style::styled::Styled;

use crate::{
    mission::{
        Mission, MissionParty,
        dungeon::{DungeonMap, Tile, RoomType, generate_dungeon},
        entities::{CombatStats, MissionEntity, RoomStatus, SimulationSpeed, SimulationTimer},
    },
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
        update_health_bars.run_if(in_state(GameTab::MissionView)),
    );
    // Respawn chain: cleanup → flush commands → re-spawn → consume trigger.
    // Uses apply_deferred so despawns take effect before spawns run.
    app.add_systems(
        Update,
        (
            respawn_cleanup.run_if(resource_exists::<RespawnMissionView>),
            ApplyDeferred,
            spawn_mission_view.run_if(resource_exists::<NeedsMissionSpawn>),
            crate::mission::entities::spawn_mission_entities
                .run_if(resource_exists::<NeedsMissionSpawn>),
            consume_needs_spawn.run_if(resource_exists::<NeedsMissionSpawn>),
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

/// Resource holding the active dungeon map for the current mission.
#[derive(Resource)]
pub struct ActiveDungeon(pub DungeonMap);

/// Marker resource: triggers cleanup of the current mission view, then respawn.
#[derive(Resource)]
pub struct RespawnMissionView;

/// Inserted after cleanup completes; triggers the spawn systems to re-run.
#[derive(Resource)]
pub struct NeedsMissionSpawn;

fn spawn_mission_view(
    mut commands: Commands,
    gameplay_root: Query<Entity, With<widgets::GameplayRoot>>,
    active_dungeon: Option<Res<ActiveDungeon>>,
    mut camera_q: Query<(&mut Transform, &mut Projection), With<Camera2d>>,
    tileset: Option<Res<crate::mission::tileset::DungeonTileset>>,
) {
    let Ok(root_entity) = gameplay_root.single() else { return };
    // Generate a dungeon if none was provided (temp dev path)
    let map = if let Some(dungeon) = active_dungeon {
        dungeon.0.clone()
    } else {
        let mut rng = rand::rng();
        let map = generate_dungeon(40, 30, 6, &mut rng);
        commands.insert_resource(ActiveDungeon(map.clone()));
        map
    };

    // Spawn dungeon tiles as sprites
    let root = commands
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
                let tile_idx = crate::mission::tileset::autotile_index(&map, x, y);
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
                let color = tile_color(tile, &map, x, y);
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

            commands.entity(root).add_child(child);
        }
    }

    // Fit camera to dungeon
    fit_camera_to_dungeon(&map, &mut camera_q);

    // Spawn UI overlay — just the abort button
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

fn cleanup_mission_view(
    mut commands: Commands,
    dungeon_q: Query<Entity, With<DungeonRoot>>,
    ui_q: Query<Entity, With<MissionViewUi>>,
    mut camera_q: Query<(&mut Transform, &mut Projection), With<Camera2d>>,
) {
    for entity in &dungeon_q {
        commands.entity(entity).despawn();
    }
    for entity in &ui_q {
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

/// Phase 1 of mission view respawn: clean up old view, then signal for re-spawn.
fn respawn_cleanup(
    mut commands: Commands,
    dungeon_q: Query<Entity, With<DungeonRoot>>,
    ui_q: Query<Entity, With<MissionViewUi>>,
    token_q: Query<Entity, With<MissionEntity>>,
    mut camera_q: Query<(&mut Transform, &mut Projection), With<Camera2d>>,
) {
    // Consume trigger
    commands.remove_resource::<RespawnMissionView>();

    // Despawn old dungeon tiles, UI overlay, and mission tokens
    for entity in &dungeon_q {
        commands.entity(entity).despawn();
    }
    for entity in &ui_q {
        commands.entity(entity).despawn();
    }
    for entity in &token_q {
        commands.entity(entity).despawn();
    }

    // Remove RoomStatus so spawn_mission_entities rebuilds it for the new dungeon.
    // Do NOT remove SimulationTimer/SimulationSpeed: the simulation chain in
    // mission/mod.rs uses them and runs in parallel with this chain. A set-level
    // `run_if(resource_exists::<SimulationTimer>)` is evaluated once per frame for
    // the set, so removing the resource here can cause hero_combat_system's
    // parameter validation to panic when it actually executes. Leaving the timer
    // alone is safe — spawn_mission_entities re-runs `init_resource` which is a
    // no-op when the resource already exists.
    commands.remove_resource::<crate::mission::entities::RoomStatus>();

    // Reset camera (spawn_mission_view will reconfigure it)
    if let Ok((mut transform, mut projection)) = camera_q.single_mut() {
        *transform = Transform::default();
        if let Projection::Orthographic(ref mut ortho) = *projection {
            ortho.scaling_mode = ScalingMode::WindowSize;
            ortho.scale = 1.0;
        }
    }

    // Signal for spawn systems to run after commands flush
    commands.insert_resource(NeedsMissionSpawn);
}

/// Phase 2 cleanup: consume the NeedsMissionSpawn resource after spawn systems run.
fn consume_needs_spawn(mut commands: Commands) {
    commands.remove_resource::<NeedsMissionSpawn>();
}

/// Convert grid coordinates to world position (centered on tile).
pub fn tile_world_pos(x: u32, y: u32) -> Vec3 {
    Vec3::new(
        x as f32 * TILE_SIZE + TILE_SIZE / 2.0,
        -(y as f32 * TILE_SIZE + TILE_SIZE / 2.0), // Y-down in grid, Y-up in world
        0.0,
    )
}

/// Get the color for a tile based on its type and room context.
fn tile_color(tile: Tile, map: &DungeonMap, x: u32, y: u32) -> Color {
    match tile {
        Tile::Wall => Color::srgb(0.15, 0.15, 0.2),
        Tile::Floor => {
            // Tint based on room type
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
    map: &DungeonMap,
    camera_q: &mut Query<(&mut Transform, &mut Projection), With<Camera2d>>,
) {
    let Ok((mut transform, mut projection)) = camera_q.single_mut() else {
        return;
    };

    let dungeon_width = map.width as f32 * TILE_SIZE;
    let dungeon_height = map.height as f32 * TILE_SIZE;

    // Center camera on dungeon
    transform.translation.x = dungeon_width / 2.0;
    transform.translation.y = -dungeon_height / 2.0;

    // Use fixed vertical height so dungeon fits regardless of window size
    if let Projection::Orthographic(ref mut ortho) = *projection {
        let padding = 1.15;
        let world_height = dungeon_height * padding;
        ortho.scaling_mode = ScalingMode::FixedVertical {
            viewport_height: world_height,
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
    // Despawn the viewed mission entity and free its heroes.
    // Token/resource cleanup is handled by OnExit(MissionView) systems.
    if let Some(viewed) = viewed {
        if let Ok((mission_entity, party)) = missions.get(viewed.0) {
            for &hero_entity in &party.0 {
                commands.entity(hero_entity).remove::<crate::mission::OnMission>();
            }
            commands.entity(mission_entity).despawn();
        }
    }

    next_tab.set(GameTab::Missions);
}

// ── Health bars ─────────────────────────────────────────────────────

/// Marker for health bar background sprite.
#[derive(Component)]
struct HealthBarBg;

/// Marker for health bar fill sprite.
#[derive(Component)]
struct HealthBarFill(Entity);

const HEALTH_BAR_WIDTH: f32 = 24.0;
const HEALTH_BAR_HEIGHT: f32 = 3.0;
const HEALTH_BAR_Y_OFFSET: f32 = 16.0;

/// Spawn health bars for new mission entities, update existing ones.
fn update_health_bars(
    mut commands: Commands,
    entities_without_bar: Query<
        (Entity, &CombatStats),
        (With<MissionEntity>, Without<Children>),
    >,
    entities_with_bar: Query<
        (&CombatStats, &Children),
        With<MissionEntity>,
    >,
    mut fills: Query<(&HealthBarFill, &mut Sprite, &mut Transform)>,
) {
    // Spawn health bars for entities that don't have children yet
    for (entity, combat) in &entities_without_bar {
        if combat.hp <= 0 {
            continue;
        }

        let bar_color = if combat.hp > 0 {
            Color::srgb(0.2, 0.8, 0.2)
        } else {
            Color::srgb(0.8, 0.2, 0.2)
        };

        // Background
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

        // Fill
        let fill = commands
            .spawn((
                Name::new("HP Bar Fill"),
                HealthBarFill(entity),
                Sprite {
                    color: bar_color,
                    custom_size: Some(Vec2::new(HEALTH_BAR_WIDTH, HEALTH_BAR_HEIGHT)),
                    ..default()
                },
                Transform::from_translation(Vec3::new(0.0, HEALTH_BAR_Y_OFFSET, 2.0)),
            ))
            .id();

        commands.entity(entity).add_children(&[bg, fill]);
    }

    // Update existing health bar fills
    for (combat, children) in &entities_with_bar {
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
                // Offset so bar shrinks from right
                let offset_x = (HEALTH_BAR_WIDTH - fill_width) / -2.0;
                transform.translation.x = offset_x;
            }
        }
    }
}
