//! Dungeon tileset loading, autotile bitmask logic, and sprite animation.

use bevy::{
    image::{ImageLoaderSettings, ImageSampler},
    prelude::*,
};

use super::data::EnemyType;
use super::dungeon::{DungeonMap, Tile};

// ── Constants ─────────────────────────────────────────────────────

const TILE_PX: u32 = 32;
const TILESET_COLUMNS: u32 = 12;
const TILESET_ROWS: u32 = 9;

// ── Resources ─────────────────────────────────────────────────────

/// Holds the dungeon tileset texture and atlas layout.
#[derive(Resource)]
pub struct DungeonTileset {
    pub texture: Handle<Image>,
    pub layout: Handle<TextureAtlasLayout>,
}

/// A single character sprite strip entry.
pub struct CharacterSpriteEntry {
    pub texture: Handle<Image>,
    pub layout: Handle<TextureAtlasLayout>,
    pub frame_count: u32,
}

/// Holds sprite entries for all character types.
#[derive(Resource)]
pub struct CharacterSprites {
    pub hero: CharacterSpriteEntry,
    pub skeleton: CharacterSpriteEntry,
    pub orc: CharacterSpriteEntry,
    pub slime: CharacterSpriteEntry,
    pub boss_rat: CharacterSpriteEntry,
    pub goblin: CharacterSpriteEntry,
}

impl CharacterSprites {
    /// Get the sprite entry for a given enemy type.
    pub fn for_enemy(&self, enemy_type: EnemyType) -> &CharacterSpriteEntry {
        match enemy_type {
            EnemyType::Skeleton => &self.skeleton,
            EnemyType::Orc => &self.orc,
            EnemyType::Slime => &self.slime,
            EnemyType::BossRat => &self.boss_rat,
            EnemyType::Goblin => &self.goblin,
        }
    }
}

// ── Sprite Loading ────────────────────────────────────────────────

/// Load all sprite assets at startup.
pub fn load_sprites(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut layouts: ResMut<Assets<TextureAtlasLayout>>,
) {
    // -- Dungeon tileset (12×9 grid, 32px tiles) --
    let tileset_texture = asset_server.load_with_settings(
        "sprites/tileset_dungeon.png",
        |s: &mut ImageLoaderSettings| {
            s.sampler = ImageSampler::nearest();
        },
    );
    let tileset_layout = layouts.add(TextureAtlasLayout::from_grid(
        UVec2::splat(TILE_PX),
        TILESET_COLUMNS,
        TILESET_ROWS,
        None,
        None,
    ));
    commands.insert_resource(DungeonTileset {
        texture: tileset_texture,
        layout: tileset_layout,
    });

    // -- Character sprites --
    let load_strip = |path: &str,
                      frames: u32,
                      asset_server: &AssetServer,
                      layouts: &mut Assets<TextureAtlasLayout>|
     -> CharacterSpriteEntry {
        let owned_path = path.to_string();
        let texture = asset_server.load_with_settings(
            owned_path,
            |s: &mut ImageLoaderSettings| {
                s.sampler = ImageSampler::nearest();
            },
        );
        let layout = layouts.add(TextureAtlasLayout::from_grid(
            UVec2::splat(TILE_PX),
            frames,
            1,
            None,
            None,
        ));
        CharacterSpriteEntry {
            texture,
            layout,
            frame_count: frames,
        }
    };

    commands.insert_resource(CharacterSprites {
        hero: load_strip(
            "sprites/hero_warrior_idle.png",
            4,
            &asset_server,
            &mut layouts,
        ),
        skeleton: load_strip(
            "sprites/skeleton_warrior_idle.png",
            4,
            &asset_server,
            &mut layouts,
        ),
        orc: load_strip(
            "sprites/skeleton_archer_idle.png",
            4,
            &asset_server,
            &mut layouts,
        ),
        slime: load_strip("sprites/slime_idle.png", 4, &asset_server, &mut layouts),
        boss_rat: load_strip("sprites/rat_idle.png", 5, &asset_server, &mut layouts),
        goblin: load_strip("sprites/skull_idle.png", 4, &asset_server, &mut layouts),
    });
}

// ── Autotiling ────────────────────────────────────────────────────

/// Floor tile variants for random visual variety (indices into tileset atlas).
const FLOOR_TILES: [u32; 12] = [13, 14, 15, 16, 25, 26, 27, 28, 37, 38, 39, 40];

/// Void tile (fully surrounded wall / outside dungeon).
const VOID_TILE: u32 = 19;

/// Wall tile lookup: index by 4-bit cardinal bitmask.
///
/// Bitmask bits represent which cardinal neighbors are walkable:
///   bit 0 (1) = North (y-1) is walkable
///   bit 1 (2) = East  (x+1) is walkable
///   bit 2 (4) = South (y+1) is walkable
///   bit 3 (8) = West  (x-1) is walkable
const WALL_CARDINAL: [u32; 16] = [
    19, // 0b0000 ( 0) -- void: no walkable neighbors
    49, // 0b0001 ( 1) -- bottom wall: floor to north
    12, // 0b0010 ( 2) -- left wall: floor to east
    48, // 0b0011 ( 3) -- bottom-left corner: floor N+E
    1,  // 0b0100 ( 4) -- top wall: floor to south
    21, // 0b0101 ( 5) -- vertical corridor wall: floor N+S
    0,  // 0b0110 ( 6) -- top-left corner: floor S+E
    10, // 0b0111 ( 7) -- T-junction: floor N+S+E (left wall T)
    17, // 0b1000 ( 8) -- right wall: floor to west
    53, // 0b1001 ( 9) -- bottom-right corner: floor N+W
    20, // 0b1010 (10) -- horizontal corridor wall: floor E+W
    34, // 0b1011 (11) -- T-junction: floor N+E+W (bottom wall T)
    5,  // 0b1100 (12) -- top-right corner: floor S+W
    32, // 0b1101 (13) -- T-junction: floor N+S+W (right wall T)
    11, // 0b1110 (14) -- T-junction: floor S+E+W (top wall T)
    33, // 0b1111 (15) -- cross/pillar: floor on all sides
];

/// Inner corner tiles for diagonal refinement.
const INNER_CORNER_SE: u32 = 6;
const INNER_CORNER_SW: u32 = 8;
const INNER_CORNER_NE: u32 = 30;
const INNER_CORNER_NW: u32 = 31;

/// Pick the atlas tile index for a given dungeon position.
pub fn autotile_index(map: &DungeonMap, x: u32, y: u32) -> u32 {
    let tile = map.get(x, y);

    match tile {
        Tile::Floor | Tile::Corridor | Tile::Door => {
            // Deterministic pseudo-random floor variant based on position
            let hash = (x.wrapping_mul(7) ^ y.wrapping_mul(13)).wrapping_add(x ^ y);
            FLOOR_TILES[(hash as usize) % FLOOR_TILES.len()]
        }
        Tile::Wall => {
            let walkable = |dx: i32, dy: i32| -> bool {
                let nx = x as i32 + dx;
                let ny = y as i32 + dy;
                if nx < 0 || ny < 0 || nx >= map.width as i32 || ny >= map.height as i32 {
                    return false;
                }
                map.is_walkable(nx as u32, ny as u32)
            };

            // 4-bit cardinal bitmask
            let mut mask: u32 = 0;
            if walkable(0, -1) {
                mask |= 1;
            } // North
            if walkable(1, 0) {
                mask |= 2;
            } // East
            if walkable(0, 1) {
                mask |= 4;
            } // South
            if walkable(-1, 0) {
                mask |= 8;
            } // West

            if mask != 0 {
                return WALL_CARDINAL[mask as usize];
            }

            // Diagonal refinement for inner corners (cardinal mask was 0)
            if walkable(1, 1) {
                return INNER_CORNER_SE;
            }
            if walkable(-1, 1) {
                return INNER_CORNER_SW;
            }
            if walkable(1, -1) {
                return INNER_CORNER_NE;
            }
            if walkable(-1, -1) {
                return INNER_CORNER_NW;
            }

            VOID_TILE
        }
    }
}

// ── Sprite Animation ──────────────────────────────────────────────

/// Drives idle frame cycling on character sprites.
#[derive(Component)]
pub struct SpriteAnimation {
    pub frame_count: u32,
    pub current_frame: u32,
    pub timer: Timer,
}

impl SpriteAnimation {
    pub fn new(frame_count: u32) -> Self {
        Self {
            frame_count,
            current_frame: 0,
            timer: Timer::from_seconds(0.4, TimerMode::Repeating),
        }
    }
}

/// Advance sprite animations each frame.
pub fn animate_sprites(time: Res<Time>, mut query: Query<(&mut SpriteAnimation, &mut Sprite)>) {
    for (mut anim, mut sprite) in &mut query {
        anim.timer.tick(time.delta());
        if anim.timer.just_finished() {
            anim.current_frame = (anim.current_frame + 1) % anim.frame_count;
            if let Some(atlas) = &mut sprite.texture_atlas {
                atlas.index = anim.current_frame as usize;
            }
        }
    }
}
