# Dungeon Tileset & Character Sprite Integration

**Date:** 2026-04-02
**Asset pack:** Dungeons and Pixels — Premium Starter Pack v1.4

## Overview

Replace colored-rectangle placeholders with proper pixel-art sprites for dungeon tiles, heroes, and enemies. The dungeon tileset uses full 8-neighbor autotiling for correct wall edges and corners. Character sprites use idle animation strips.

## Asset Organization

Copy needed assets into the project:

```
assets/sprites/
├── tileset_dungeon.png          (384×288, 12×9 grid of 32×32 tiles)
├── hero_warrior_idle.png        (idle down strip, 4 frames)
├── skeleton_warrior_idle.png    (idle down strip, 4 frames)
├── skeleton_archer_idle.png     (idle down strip, 4 frames)
├── slime_idle.png               (idle strip, 4 frames)
├── rat_idle.png                 (idle strip, 5 frames)
└── skull_idle.png               (idle down strip, 4 frames)
```

Only idle strips for now. PSD files, individual frames, and unused enemies (Ghost, Spider, Blue Slime, Skeleton Mage) are skipped. Easy to add attack/death/run strips later.

## Dungeon Tileset

### Loading

A `DungeonTileset` resource holds the texture handle and `TextureAtlas` layout (12 columns × 9 rows, 32×32 tiles). Loaded at startup via the existing `ResourceHandles` pipeline.

### Autotiling Algorithm

For each wall tile, compute a 4-bit cardinal bitmask from neighbors:

```
bit 0 (1) = North is wall
bit 1 (2) = East is wall
bit 2 (4) = South is wall
bit 3 (8) = West is wall
```

16 possible patterns, each mapping to a tile index via a const lookup table. Examples:
- `0b0000` (no adjacent walls) = isolated wall pillar
- `0b1111` (walls on all sides) = void/solid fill
- `0b0101` (N+S) = vertical wall segment
- `0b1010` (E+W) = horizontal wall segment

**Diagonal refinement:** After the cardinal pass, check diagonal neighbors for inner-corner variants where a wall has floor on two adjacent cardinal sides plus floor on the diagonal between them.

**Floor tiles:** Skip bitmask, randomly pick from 4-6 floor variants seeded by grid position (deterministic).

**Corridor tiles:** Same floor variants (walkable ground, same visual treatment).

**Door tiles:** Floor tile (door prop overlay deferred to future work).

### Rendering

In `spawn_mission_view`, replace colored `Sprite` spawns with `Sprite` + `TextureAtlas` index from the autotiler.

## Character & Enemy Sprites

### Loading

A `CharacterSprites` resource holds texture handles and `TextureAtlas` layouts for each strip. Each strip is a horizontal row of 32×32 frames. Loaded at startup alongside the dungeon tileset.

### Sprite Mapping

| Game Type | Strip File | Frames |
|-----------|-----------|--------|
| All hero classes | `hero_warrior_idle.png` | 4 |
| `EnemyType::Skeleton` | `skeleton_warrior_idle.png` | 4 |
| `EnemyType::Orc` | `skeleton_archer_idle.png` | 4 |
| `EnemyType::Slime` | `slime_idle.png` | 4 |
| `EnemyType::BossRat` | `rat_idle.png` | 5 |
| `EnemyType::Goblin` | `skull_idle.png` | 4 |

### Idle Animation

A `SpriteAnimation` component stores frame count, current frame, and a timer. A single system advances all animated sprites every ~400ms (matching the pack's Tiled animation durations) and updates the `TextureAtlas` index. Runs with `GameTab::MissionView` run condition.

### Rendering

In `spawn_mission_entities`, hero/enemy tokens swap `Sprite { color, custom_size }` for `Sprite` + `TextureAtlas` + `SpriteAnimation`. Existing `GridPosition` → `Transform` sync and health bar systems remain untouched.

## What Stays The Same

- BSP dungeon generation — untouched
- Grid simulation — `GridPosition`, `MoveTarget`, `InRoom`, `SimulationTimer`, AI, combat
- Health bars — same child sprite positioning
- Camera fitting — `fit_camera_to_dungeon` unchanged, still 32px tiles
- `tile_world_pos` — same coordinate math in both `entities.rs` and `mission_view.rs`

## Files Changed

- **`mission_view.rs`** — tile spawning swaps colored sprites for atlas sprites
- **`entities.rs`** — hero/enemy spawning swaps colored sprites for atlas sprites + animation
- **New: `src/mission/tileset.rs`** — autotile logic, `DungeonTileset` resource, `CharacterSprites` resource, `SpriteAnimation` component + system
