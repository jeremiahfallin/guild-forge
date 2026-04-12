# Save System & Offline Time Bank — Design

## Core Loop

```
Player quits → game saves state + timestamp
Player returns → load save, calculate elapsed time
elapsed time (capped at 24h) added to offline bank
Player spends banked time via 1x/2x/3x speed control
Speed control uses Bevy's Time<Virtual>::set_relative_speed()
All systems (missions, training, recruiting) scale uniformly
```

## Save System

### Storage
- Single save slot, stored as a RON file.
- Platform path via Bevy's standard data directory (e.g., `dirs::data_dir()/guild-forge/save.ron`).
- Serialized/deserialized via serde + RON (both already in Cargo.toml).

### Save Triggers
- **Manual**: Save button in the pause menu.
- **Autosave**: Every 5 minutes on a timer.
- **On quit**: Triggered by Bevy's `AppExit` event.

### SaveData Structure

Top-level struct serialized to the save file:

```
SaveData {
    // Timestamp
    last_save_timestamp: u64,         // unix epoch seconds

    // Resources
    gold: u32,
    reputation: u32,
    banked_seconds: f32,              // offline time bank, capped at 86400

    // Materials
    materials: HashMap<MaterialType, u32>,

    // Buildings
    buildings: HashMap<BuildingType, u32>,

    // Hero roster
    heroes: Vec<HeroSaveDto>,

    // Applicant board
    applicants: Vec<ApplicantSaveDto>,
    next_arrival_timer: f32,

    // Training
    training_timer: f32,

    // Missions in progress
    missions: Vec<MissionSaveDto>,
}
```

### Hero DTO

```
HeroSaveDto {
    name: String,
    class: HeroClass,
    level: u32,
    xp: u32,
    xp_to_next: u32,
    stats: HeroStatsSave,            // str, dex, con, int, wis, cha
    traits: Vec<HeroTrait>,
    equipment: HeroEquipmentSave,    // weapon_tier, armor_tier, accessory_tier
    on_mission: bool,                // whether currently deployed
}
```

### Applicant DTO

```
ApplicantSaveDto {
    name: String,
    class: HeroClass,
    traits: Vec<HeroTrait>,
    stats: HeroStatsSave,
    hire_cost: u32,
    time_remaining: f32,
}
```

### Mission DTO

```
MissionSaveDto {
    template_id: String,
    name: String,
    difficulty: u32,
    progress: MissionProgress,
    rng_seed: u64,                   // RNG state for deterministic replay

    // Party: indices into the heroes vec in SaveData
    party_hero_indices: Vec<usize>,

    // Dungeon
    dungeon_map: DungeonMap,         // already Clone, can derive Serialize
    room_status_visited: Vec<bool>,
    room_status_cleared: Vec<bool>,

    // Tokens
    hero_tokens: Vec<HeroTokenDto>,
    enemy_tokens: Vec<EnemyTokenDto>,
}

HeroTokenDto {
    roster_index: usize,             // index into heroes vec
    grid_x: u32,
    grid_y: u32,
    in_room: Option<usize>,
    hp: i32,
    max_hp: i32,
    attack: i32,
    defense: i32,
    path: Option<Vec<(u32, u32)>>,
    path_index: usize,
}

EnemyTokenDto {
    enemy_type: EnemyType,
    xp_reward: u32,
    grid_x: u32,
    grid_y: u32,
    in_room: Option<usize>,
    hp: i32,
    max_hp: i32,
    attack: i32,
    defense: i32,
}
```

### Save Flow
1. Collect all ECS state into `SaveData` struct.
2. Serialize to RON string.
3. Write to save file path.
4. Fire toast: "Game saved."

### Load Flow
1. On entering `Screen::Gameplay`, check for save file.
2. If found:
   - Deserialize `SaveData`.
   - Spawn hero entities from `heroes` vec.
   - Restore resources: Gold, Materials, Reputation, GuildBuildings.
   - Restore ApplicantBoard from applicants + timer.
   - Restore TrainingTimer.
   - Spawn mission entities from `missions` vec, reconstructing tokens as children.
   - Calculate offline time: `elapsed = now - last_save_timestamp`.
   - Add to bank: `banked_seconds = min(banked_seconds + elapsed, 86400)`.
   - Skip normal `spawn_starter_heroes` (heroes already exist).
3. If not found: normal startup (spawn 3 starter heroes, empty state).

## Offline Time Bank

### Resource
`OfflineTimeBank { banked_seconds: f32 }` — capped at 86,400.0 (24 hours).

### Speed Control
Three discrete speeds: **1x, 2x, 3x**.

Stored as a `GameSpeed` resource with the current multiplier.

### Mechanism
Uses `Time<Virtual>::set_relative_speed(multiplier)`. This naturally scales:
- `FixedUpdate` tick rate (missions, combat, AI, movement)
- `time.delta_secs()` in Update systems (training, applicant timers)

All game systems speed up uniformly with zero code changes.

### Fuel Consumption
- **1x**: Free. No bank drain.
- **2x**: Consumes 1 banked second per real second.
- **3x**: Consumes 2 banked seconds per real second.

Formula: `drain_per_real_second = speed - 1.0`

### Bank Depletion
When `banked_seconds` reaches 0:
- Auto-reset speed to 1x.
- Set `Time<Virtual>::set_relative_speed(1.0)`.
- Disable 2x/3x buttons.

### Tick System
Runs in Update, using `Time<Real>::delta_secs()` (real wall-clock time, unaffected by virtual speed):

```
fn tick_offline_bank(
    real_time: Res<Time<Real>>,
    mut bank: ResMut<OfflineTimeBank>,
    game_speed: Res<GameSpeed>,
    mut virtual_time: ResMut<Time<Virtual>>,
) {
    let drain_rate = game_speed.0 - 1.0;
    if drain_rate <= 0.0 { return; }

    let dt = real_time.delta_secs();
    bank.banked_seconds -= drain_rate * dt;

    if bank.banked_seconds <= 0.0 {
        bank.banked_seconds = 0.0;
        game_speed.0 = 1.0;
        virtual_time.set_relative_speed(1.0);
    }
}
```

## Speed Control UI

Small bar in the sidebar, below the reputation display:

```
[1x] [2x] [3x]  Bank: 2h 15m
```

- Active speed highlighted.
- 2x/3x greyed out when bank is empty.
- Bank displayed as "Xh Ym" (or "Xm Ys" if under 1 hour).
- Updates reactively as bank drains.

## Serialization Requirements

The following types need `Serialize` + `Deserialize` derives added:
- `DungeonMap` and its sub-types (Room, RoomType, Tile, TileType)
- `MissionProgress`
- `HeroClass`, `HeroTrait` (already have Deserialize, need Serialize)
- `EnemyType` (already has Deserialize, need Serialize)
- `MaterialType` (already has Deserialize, need Serialize)
- `BuildingType` (already has Deserialize, need Serialize)

## File Dependencies

- `serde` with `derive` feature (already in Cargo.toml)
- `ron` (already in Cargo.toml)
- `std::time::SystemTime` for unix timestamps
- `dirs` crate or Bevy's built-in path helpers for save file location
