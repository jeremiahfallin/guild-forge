# Hero Growth Rates — Design

## Problem

Leveling up a hero currently only bumps `level` and adds +5 HP at mission start. Stats don't grow, so a level-10 hero is mechanically identical to a level-1 hero with the same rolled stats. Level is cosmetic.

Additionally, the `RecruitmentOffice` building promises "More applicants with better quality," but only raises the applicant cap — the quality half of the promise is unfulfilled.

## Goals

1. Make level-up meaningful: heroes get stronger as they level.
2. Give each hero long-term identity: growth rates vary per hero, not just per class.
3. Redeem the `RecruitmentOffice` quality promise via a hidden quality roll that affects growth rates.
4. Preserve the "sleeper hero" discovery moment — unimpressive starting stats can hide great growth.
5. Keep the player out of per-level decisions (automatic growth, no popup).

## Non-goals

- No player choice at level-up (no stat-picking UI).
- No visible quality indicator on the applicant card — quality stays implicit.
- No level-up ability unlocks, milestone perks, or trait slots (separate future design).
- No stat caps.

## Design summary

At applicant generation, roll a hidden `quality` float from reputation tier + Recruitment Office level + jitter. Use it to weight the random portion of per-stat growth rates. Store growth rates as a `HeroGrowth` component on the hero. On level-up (from training or mission XP), accumulate fractional growth into `HeroStatProgress` and apply integer stat increases to `HeroStats`.

## Data model

Two new per-hero components added at hire:

```rust
#[derive(Component, Clone, Debug, Reflect)]
pub struct HeroGrowth {
    pub strength: f32,
    pub dexterity: f32,
    pub constitution: f32,
    pub intelligence: f32,
    pub wisdom: f32,
    pub charisma: f32,
}

#[derive(Component, Clone, Debug, Default, Reflect)]
pub struct HeroStatProgress {
    pub strength: f32,
    pub dexterity: f32,
    pub constitution: f32,
    pub intelligence: f32,
    pub wisdom: f32,
    pub charisma: f32,
}
```

Quality is **not** stored — it's a transient local during generation used to weight the growth-rate rolls, then discarded. No save surface, no UI surface, no cheat peek.

## Generation formulas

Computed at applicant generation (in `recruiting.rs::generate_applicant`) and for debug-spawned heroes (`hero::spawn_random_hero`). Shared helper: `roll_growth(class_def, quality, rng) -> HeroGrowth`.

**Constants** (tunable, live in `recruiting.rs`):

- `FLOOR_PER_WEIGHT = 0.2`
- `MAX_QUALITY_INPUT = 6` (rep tier 5 contributes +4, office level 3 contributes +3 — capped at 6)
- `MAX_RANDOM = 1.0`
- `QUALITY_JITTER = 0.2`

**Per-stat class floor:**

```
class_floor = class_weight * FLOOR_PER_WEIGHT
```

Existing `stat_weights` are 0–3. Floors range 0.0–0.6. A Warrior (`str:3, dex:1, con:3, int:0, wis:1, cha:1`) floors at `0.6, 0.2, 0.6, 0.0, 0.2, 0.2`. INT cannot grow for a Warrior unless the random roll adds to it — class identity preserved.

**Quality roll (transient, per applicant):**

```
quality_base = (rep_tier - 1 + office_level) / MAX_QUALITY_INPUT
quality      = (quality_base + rng(-QUALITY_JITTER..=QUALITY_JITTER)).clamp(0.0, 1.0)
```

Starts near 0 at tier-1 / office-0, approaches 1.0 at tier-5 + office-3. The ±0.2 jitter lets new guilds occasionally roll a prodigy and lets maxed guilds occasionally hire a dud.

**Random portion (per stat, independent roll):**

```
random_portion = rng(0.0..=MAX_RANDOM) * quality
```

Quality gates the ceiling of the random roll but does not subtract from the floor. Each stat rolls its own random portion, so even a high-quality hero has variance across stats.

**Final:**

```
growth_rate = class_floor + random_portion
```

**Example ranges:**

| Hero | Quality | STR growth | INT growth |
|---|---|---|---|
| Warrior, fresh guild (low Q) | 0.10 | 0.6–0.7 | 0.0–0.1 |
| Warrior, endgame guild (high Q) | 0.90 | 0.6–1.5 | 0.0–0.9 |
| Warrior, hidden gem (lucky) | 0.15 | 1.4 | 0.6 |
| Warrior, dud (unlucky) | 0.80 | 0.7 | 0.1 |

Over 20 levels a rate-1.0 stat adds ~+20, roughly doubling. Meaningful without snowballing.

## Level-up application

Consolidates duplicated logic currently in `training.rs` and `mission/combat.rs` into a single helper on `src/hero/mod.rs`.

```rust
pub fn award_xp(
    info: &mut HeroInfo,
    stats: &mut HeroStats,
    growth: &HeroGrowth,
    progress: &mut HeroStatProgress,
    xp: u32,
) -> u32 {
    info.xp += xp;
    let mut level_ups = 0;
    while info.xp >= info.xp_to_next {
        info.xp -= info.xp_to_next;
        info.level += 1;
        info.xp_to_next = (info.xp_to_next as f32 * 1.5) as u32;
        apply_growth_tick(stats, growth, progress);
        level_ups += 1;
    }
    level_ups
}

fn apply_growth_tick(
    stats: &mut HeroStats,
    growth: &HeroGrowth,
    progress: &mut HeroStatProgress,
) {
    // Per stat:
    progress.strength += growth.strength;
    let gained = progress.strength.floor() as i32;
    stats.strength += gained;
    progress.strength -= gained as f32;
    // ...repeat for dex, con, int, wis, cha
}
```

The accumulator makes sub-1.0 rates work correctly and eventually: a 0.3/level stat gains +1 every ~3 levels. A 0.0-rate stat never grows, which is desired (class identity).

## Integration points

- **`src/recruiting.rs`** — `Applicant` gains `growth: HeroGrowth`. `generate_applicant` calls `roll_growth`. `handle_hire_applicant` spawns the hero with both `HeroGrowth` and `HeroStatProgress::default()`.
- **`src/hero/mod.rs`** — home of `award_xp`, `apply_growth_tick`, and `roll_growth`. `spawn_random_hero` also attaches the two components via the shared helper.
- **`src/training.rs`** — query picks up `&mut HeroStats, &HeroGrowth, &mut HeroStatProgress`; inline level-up loop replaced with `award_xp`.
- **`src/mission/combat.rs`** — same substitution in the mission-completion survivor loop.
- **`src/save.rs`** — add `HeroGrowthSave` and `HeroStatProgressSave` DTOs. Serialize alongside existing hero data. On load, use `#[serde(default)]` so old saves deserialize; a post-load backfill rolls a neutral `HeroGrowth` (quality 0.5) for any hero missing one.
- **UI** — no changes. Stats already surface on the roster screen and automatically reflect growth.

`CombatStats` derivation at mission dispatch does not change — growth mutates `HeroStats` directly, and the existing derivation from `HeroStats` picks up the new values for free.

## Testing strategy

**Unit tests** in `src/hero/` / `src/recruiting.rs`:

- `roll_growth` at quality 0.0 yields exactly `class_weight * FLOOR_PER_WEIGHT` per stat.
- `roll_growth` at quality 1.0 keeps per-stat growth ≤ `class_floor + MAX_RANDOM`.
- `apply_growth_tick` with rate 0.0 never grows the stat, even over 50 level-ups.
- Accumulator: rate 0.5 → +1 every 2 levels; rate 0.3 → +3 over 10 levels.
- `award_xp` with enough XP for 3 levels applies growth 3 times and leaves correct residual XP.

**Save round-trip:**

- Known growth + partial progress → serialize → deserialize → field equality.
- Old-format save (no growth fields) loads, backfill produces a valid `HeroGrowth` and zero `HeroStatProgress`.

**Not tested** (YAGNI): quality distribution statistics, UI rendering, balance tuning.

## Migration

Old save files omit the new fields. `#[serde(default)]` on save DTOs plus a post-load system that inserts `HeroGrowth` (rolled at neutral quality 0.5 using the hero's current class) and `HeroStatProgress::default()` for any hero missing them. Single-shot, invisible to the player.

## Tuning knobs

All of these are `const`s in `recruiting.rs` and can be adjusted without structural changes:

- `FLOOR_PER_WEIGHT` — how strong the class floor is
- `MAX_RANDOM` — how high the random portion can climb
- `MAX_QUALITY_INPUT` — how many rep/office points count toward quality
- `QUALITY_JITTER` — how often low-quality guilds get lucky and vice versa

Playtesting will drive specific values.
