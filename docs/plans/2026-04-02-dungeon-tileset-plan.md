# Dungeon Tileset & Character Sprite Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Replace colored-rectangle placeholders with pixel-art sprites from the Dungeons and Pixels asset pack, using 4-bit bitmask autotiling for dungeon walls and idle animation strips for characters.

**Architecture:** A new `src/mission/tileset.rs` module owns all sprite resources (tileset atlas, character atlases), autotile logic, and animation systems. `mission_view.rs` and `entities.rs` swap their colored `Sprite` spawns for atlas-based sprites. The dungeon generation and simulation systems remain untouched.

**Tech Stack:** Bevy 0.18 `TextureAtlas` + `TextureAtlasLayout`, `AssetServer`, `Sprite` with atlas index. No new crate dependencies.

---

## Task 1: Copy Asset Files

**Files:**
- Create: `assets/sprites/tileset_dungeon.png`
- Create: `assets/sprites/hero_warrior_idle.png`
- Create: `assets/sprites/skeleton_warrior_idle.png`
- Create: `assets/sprites/skeleton_archer_idle.png`
- Create: `assets/sprites/slime_idle.png`
- Create: `assets/sprites/rat_idle.png`
- Create: `assets/sprites/skull_idle.png`

**Step 1: Create sprites directory and copy files**

```bash
mkdir -p assets/sprites

PACK="C:/Users/bullf/Downloads/Dungeons and Pixels — Premium Starter Pack v1.4"

cp "$PACK/Tilesets/Tileset_Dungeon.png" assets/sprites/tileset_dungeon.png
cp "$PACK/Characters/Hero_Warrior/Strips/Idle/down_strip.png" assets/sprites/hero_warrior_idle.png
cp "$PACK/Enemies/Skeleton warrior/Strips/Idle/down_strip.png" assets/sprites/skeleton_warrior_idle.png
cp "$PACK/Enemies/Skeleton archer/Strips/Idle/down_strip.png" assets/sprites/skeleton_archer_idle.png
cp "$PACK/Enemies/Slime/Strips/idle_strip.png" assets/sprites/slime_idle.png
cp "$PACK/Enemies/Rat/Strips/idle_strip.png" assets/sprites/rat_idle.png
cp "$PACK/Enemies/Skull/Strips/Idle/down_strip.png" assets/sprites/skull_idle.png
```

**Step 2: Verify files exist**

```bash
ls -la assets/sprites/
```

Expected: 7 PNG files.

**Step 3: Commit**

```bash
git add assets/sprites/
git commit -m "Add sprite assets from Dungeons and Pixels pack"
```

---

## Task 2: Create tileset module — resources and loading

**Files:**
- Create: `src/mission/tileset.rs`
- Modify: `src/mission/mod.rs` — add `pub mod tileset;` and register systems

**Step 1: Create `src/mission/tileset.rs` with resource definitions and plugin**

The module defines two resources:
- `DungeonTileset` — holds the tileset texture + atlas layout handle
- `CharacterSprites` — holds per-character texture + atlas layout handles

```rust
//! Dungeon tileset and character sprite loading, autotiling, and animation.

use bevy::prelude::*;
use bevy::image::ImageSampler;

use super::data::EnemyType;

/// Tileset grid dimensions.
const TILESET_COLUMNS: u32 = 12;
const TILESET_ROWS: u32 = 9;
const TILE_PX: u32 = 32;

/// Resource holding the dungeon tileset atlas.
#[derive(Resource)]
pub struct DungeonTileset {
    pub texture: Handle<Image>,
    pub layout: Handle<TextureAtlasLayout>,
}

/// One character sprite entry: texture, atlas layout, and frame count.
pub struct CharacterSpriteEntry {
    pub texture: Handle<Image>,
    pub layout: Handle<TextureAtlasLayout>,
    pub frame_count: u32,
}

/// Resource holding all character/enemy sprite atlases.
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
    /// Look up the sprite entry for an enemy type.
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
```

**Step 2: Add the loading system**

In the same file, add:

```rust
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
    let load_strip = |path: &str, frames: u32,
                      asset_server: &AssetServer,
                      layouts: &mut Assets<TextureAtlasLayout>| -> CharacterSpriteEntry {
        let texture = asset_server.load_with_settings(
            path,
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
        CharacterSpriteEntry { texture, layout, frame_count: frames }
    };

    commands.insert_resource(CharacterSprites {
        hero: load_strip("sprites/hero_warrior_idle.png", 4, &asset_server, &mut layouts),
        skeleton: load_strip("sprites/skeleton_warrior_idle.png", 4, &asset_server, &mut layouts),
        orc: load_strip("sprites/skeleton_archer_idle.png", 4, &asset_server, &mut layouts),
        slime: load_strip("sprites/slime_idle.png", 4, &asset_server, &mut layouts),
        boss_rat: load_strip("sprites/rat_idle.png", 5, &asset_server, &mut layouts),
        goblin: load_strip("sprites/skull_idle.png", 4, &asset_server, &mut layouts),
    });
}
```

Note: Check the Bevy 0.18 API for `load_with_settings` and `ImageLoaderSettings`. The import might be `bevy::image::ImageLoaderSettings`. If the API has changed, use whatever Bevy 0.18 provides for setting nearest-neighbor sampling on sprite images.

**Step 3: Register the module and system in `src/mission/mod.rs`**

Add `pub mod tileset;` to the module declarations. Add the startup system:

```rust
app.add_systems(Startup, tileset::load_sprites);
```

**Step 4: Build and verify**

```bash
cargo build
```

Expected: Compiles with no errors. The resources will be inserted at startup.

**Step 5: Commit**

```bash
git add src/mission/tileset.rs src/mission/mod.rs
git commit -m "Add tileset and character sprite loading"
```

---

## Task 3: Implement autotile bitmask logic

**Files:**
- Modify: `src/mission/tileset.rs` — add autotile functions

**Step 1: Add the bitmask-to-tile-index lookup table and autotile function**

Append to `tileset.rs`:

```rust
// ── Autotiling ─────────────────────────────────────────────────────

use super::dungeon::{DungeonMap, Tile};

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
    19, // 0b0000 ( 0) — void: no walkable neighbors
    49, // 0b0001 ( 1) — bottom wall: floor to north
    12, // 0b0010 ( 2) — left wall: floor to east
    48, // 0b0011 ( 3) — bottom-left corner: floor N+E
     1, // 0b0100 ( 4) — top wall: floor to south
    21, // 0b0101 ( 5) — vertical corridor wall: floor N+S
     0, // 0b0110 ( 6) — top-left corner: floor S+E
    10, // 0b0111 ( 7) — T-junction: floor N+S+E (left wall T)
    17, // 0b1000 ( 8) — right wall: floor to west
    53, // 0b1001 ( 9) — bottom-right corner: floor N+W
    20, // 0b1010 (10) — horizontal corridor wall: floor E+W
    34, // 0b1011 (11) — T-junction: floor N+E+W (bottom wall T)
     5, // 0b1100 (12) — top-right corner: floor S+W
    32, // 0b1101 (13) — T-junction: floor N+S+W (right wall T)
    11, // 0b1110 (14) — T-junction: floor S+E+W (top wall T)
    33, // 0b1111 (15) — cross/pillar: floor on all sides
];

/// Inner corner tiles for diagonal refinement.
/// Used when cardinal bitmask is 0 (void) but a diagonal has floor.
const INNER_CORNER_SE: u32 = 6;  // floor to south-east
const INNER_CORNER_SW: u32 = 8;  // floor to south-west
const INNER_CORNER_NE: u32 = 30; // floor to north-east
const INNER_CORNER_NW: u32 = 31; // floor to north-west

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
            if walkable(0, -1) { mask |= 1; } // North
            if walkable(1,  0) { mask |= 2; } // East
            if walkable(0,  1) { mask |= 4; } // South
            if walkable(-1, 0) { mask |= 8; } // West

            if mask != 0 {
                return WALL_CARDINAL[mask as usize];
            }

            // Diagonal refinement for inner corners (cardinal mask was 0)
            if walkable(1,  1) { return INNER_CORNER_SE; }
            if walkable(-1, 1) { return INNER_CORNER_SW; }
            if walkable(1, -1) { return INNER_CORNER_NE; }
            if walkable(-1,-1) { return INNER_CORNER_NW; }

            VOID_TILE
        }
    }
}
```

**Step 2: Build and verify**

```bash
cargo build
```

Expected: Compiles with no errors.

**Step 3: Commit**

```bash
git add src/mission/tileset.rs
git commit -m "Add 4-bit bitmask autotile logic with diagonal inner corners"
```

---

## Task 4: Add sprite animation component and system

**Files:**
- Modify: `src/mission/tileset.rs` — add `SpriteAnimation` component and tick system

**Step 1: Add the animation component and system**

Append to `tileset.rs`:

```rust
// ── Sprite Animation ───────────────────────────────────────────────

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
pub fn animate_sprites(
    time: Res<Time>,
    mut query: Query<(&mut SpriteAnimation, &mut Sprite)>,
) {
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
```

**Step 2: Register the animation system in `src/mission/mod.rs`**

Add to the existing mission view systems section (with the `GameTab::MissionView` run condition):

```rust
app.add_systems(
    Update,
    tileset::animate_sprites
        .run_if(in_state(GameTab::MissionView)),
);
```

**Step 3: Build and verify**

```bash
cargo build
```

Expected: Compiles.

**Step 4: Commit**

```bash
git add src/mission/tileset.rs src/mission/mod.rs
git commit -m "Add sprite animation component and system"
```

---

## Task 5: Replace dungeon tile rendering in mission_view.rs

**Files:**
- Modify: `src/screens/mission_view.rs` — use `DungeonTileset` + `autotile_index` instead of colored sprites

**Step 1: Update `spawn_mission_view` tile spawning**

Replace the tile-spawning loop (the `for y in 0..map.height { for x in 0..map.width { ... } }` block) with atlas-based sprites.

The current code spawns tiles like:
```rust
let color = tile_color(tile, &map, x, y);
let child = commands.spawn((
    Name::new(...),
    Sprite { color, custom_size: Some(Vec2::splat(TILE_SIZE)), ..default() },
    Transform::from_translation(pos),
)).id();
```

Replace with:
```rust
let tile_idx = crate::mission::tileset::autotile_index(&map, x, y);
let child = commands.spawn((
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
)).id();
```

The system needs access to the `DungeonTileset` resource — add it as an `Option<Res<DungeonTileset>>` parameter to `spawn_mission_view`. Fall back to the old colored-sprite approach if the resource isn't available yet (shouldn't happen since it loads at Startup, but defensive).

Remove the `tile_color` function since it's no longer needed.

**Step 2: Build and test visually**

```bash
cargo run
```

Navigate to a mission view and confirm dungeon tiles render with the tileset sprites. Walls should have correct edge variants and floors should show stone texture variety.

**Step 3: Commit**

```bash
git add src/screens/mission_view.rs
git commit -m "Render dungeon tiles with autotiled sprites"
```

---

## Task 6: Replace hero/enemy token rendering in entities.rs

**Files:**
- Modify: `src/mission/entities.rs` — use `CharacterSprites` for hero/enemy tokens

**Step 1: Update hero token spawning**

In `spawn_mission_entities`, the hero spawn block currently does:
```rust
commands.spawn((
    ...,
    Sprite { color, custom_size: Some(Vec2::splat(24.0)), ..default() },
    Transform::from_translation(pos.with_z(5.0)),
));
```

Replace with atlas sprite + animation. Add `Option<Res<CharacterSprites>>` parameter to the system. If available:

```rust
let entry = &char_sprites.hero;
commands.spawn((
    Name::new(format!("Hero Token: {}", info.name)),
    MissionEntity,
    HeroToken(hero_entity),
    GridPosition { x: hx, y: hy },
    InRoom(map.room_at(hx, hy)),
    CombatStats { hp, max_hp: hp, attack, defense },
    Sprite {
        image: entry.texture.clone(),
        texture_atlas: Some(TextureAtlas {
            layout: entry.layout.clone(),
            index: 0,
        }),
        ..default()
    },
    Transform::from_translation(pos.with_z(5.0)),
    tileset::SpriteAnimation::new(entry.frame_count),
));
```

**Step 2: Update enemy token spawning**

In `spawn_enemies`, replace the colored sprite with the enemy's atlas sprite:

```rust
let entry = char_sprites.for_enemy(enemy_type);
commands.spawn((
    Name::new(format!("Enemy: {}", enemy_def.name)),
    MissionEntity,
    EnemyToken { enemy_type, xp_reward: enemy_def.xp_reward },
    GridPosition { x: ex, y: ey },
    InRoom(Some(room_idx)),
    CombatStats { hp: enemy_def.hp, max_hp: enemy_def.hp, attack: enemy_def.attack, defense: enemy_def.defense },
    Sprite {
        image: entry.texture.clone(),
        texture_atlas: Some(TextureAtlas {
            layout: entry.layout.clone(),
            index: 0,
        }),
        ..default()
    },
    Transform::from_translation(pos.with_z(4.0)),
    tileset::SpriteAnimation::new(entry.frame_count),
));
```

Pass `CharacterSprites` through to `spawn_enemies` (add parameter or pass reference).

Remove `hero_color` and `enemy_color` helper functions since they're no longer used.

**Step 3: Build and test visually**

```bash
cargo run
```

Dispatch a mission and verify hero tokens show the warrior sprite and enemies show their respective sprites, all with idle animation cycling.

**Step 4: Commit**

```bash
git add src/mission/entities.rs
git commit -m "Render hero and enemy tokens with animated sprites"
```

---

## Task 7: Visual polish and cleanup

**Files:**
- Modify: `src/screens/mission_view.rs` — set camera to pixel-perfect mode
- Modify: `src/mission/tileset.rs` — any index tweaks after visual testing

**Step 1: Set camera sampling to nearest-neighbor**

In `fit_camera_to_dungeon` or `spawn_mission_view`, ensure the camera uses nearest-neighbor sampling so pixel art stays crisp. In Bevy 0.18, this may be handled by `Msaa::Off` or camera settings. Check the Bevy 0.18 API.

If needed, insert `Msaa::Off` resource or configure the camera's `Camera` component.

**Step 2: Visual test and adjust tile indices**

Run the game, enter a mission, and visually inspect:
- Do wall edges look correct? Adjust `WALL_CARDINAL` indices if any edges are wrong.
- Do inner corners render properly? Adjust `INNER_CORNER_*` constants.
- Do floor tiles look good with enough variety?
- Do character sprites appear at the right size relative to tiles?

Iterate on the const lookup table values until the dungeon looks correct. This is the tuning step — the bitmask logic is correct, but the specific tile index assignments may need adjustment based on how the tileset image is actually laid out.

**Step 3: Remove dead code**

Remove any now-unused functions: `tile_color` in `mission_view.rs`, `hero_color` and `enemy_color` in `entities.rs`.

**Step 4: Final build + run**

```bash
cargo build && cargo run
```

Verify no warnings about unused code from the removed functions.

**Step 5: Commit**

```bash
git add -A
git commit -m "Polish tileset rendering and remove placeholder color code"
```

---

## Notes

- The `WALL_CARDINAL` and `INNER_CORNER_*` constants are best-effort mappings from studying the tileset image and Tiled example. They will almost certainly need tuning in Task 7 after seeing the actual rendered result. The bitmask algorithm itself is correct.
- Bevy 0.18's `Sprite` API may differ slightly from what's shown. Check if `texture_atlas` is a field on `Sprite` or a separate component (`TextureAtlas`). Adjust accordingly.
- The `ImageLoaderSettings` import path may be `bevy::image::ImageLoaderSettings` or `bevy::render::texture::ImageLoaderSettings` depending on Bevy 0.18's module structure.
- Character sprites use `custom_size: None` (natural 32×32 from the atlas) rather than the old `custom_size: Some(Vec2::splat(24.0))`. If sprites look too large, add `custom_size` back with a smaller value.
