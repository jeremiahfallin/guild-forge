# Hero Growth Rates Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Make level-up mechanically meaningful by giving every hero per-stat growth rates that raise their base `HeroStats` automatically on level-up. Growth rates are rolled at hire time with a class floor, a quality-weighted random portion, and a transient hidden quality value driven by reputation tier + `RecruitmentOffice` level.

**Architecture:** Two new components — `HeroGrowth` (per-stat f32 rates) and `HeroStatProgress` (per-stat fractional accumulators) — attached to every hero. A single `award_xp` helper in `src/hero/mod.rs` consolidates today's duplicated level-up loops in `training.rs` and `mission/combat.rs`; when it levels a hero, it pushes fractional growth into the accumulator and converts integer increments onto `HeroStats`. Generation helper `roll_growth` constructs the growth rates from class weights + a quality roll. `CombatStats` is already derived from `HeroStats` at mission dispatch, so no combat code changes are needed.

**Tech Stack:** Rust, Bevy 0.18, `rand`, `serde` (RON saves). Tests use built-in `cargo test` with `#[cfg(test)] mod tests` blocks (see `src/mission/pathfinding.rs` for the existing convention).

**Reference design:** `docs/plans/2026-04-14-hero-growth-rates-design.md`.

---

## Task 1: Add `HeroGrowth` and `HeroStatProgress` components

**Files:**
- Modify: `src/hero/mod.rs` (append new components alongside `HeroStats`)

**Step 1: Add the components**

At the bottom of the component block in `src/hero/mod.rs` (after `HeroTraits`), add:

```rust
/// Per-stat growth rate (stat points gained per level, as a float).
/// Rolled once at hire time; fixed for the hero's lifetime.
#[derive(Component, Debug, Clone, Reflect)]
#[reflect(Component)]
pub struct HeroGrowth {
    pub strength: f32,
    pub dexterity: f32,
    pub constitution: f32,
    pub intelligence: f32,
    pub wisdom: f32,
    pub charisma: f32,
}

/// Fractional accumulator per stat. On level-up, `growth_rate` is added to
/// the matching field; the integer part is applied to `HeroStats` and the
/// fractional remainder is kept here for the next level.
#[derive(Component, Debug, Clone, Default, Reflect)]
#[reflect(Component)]
pub struct HeroStatProgress {
    pub strength: f32,
    pub dexterity: f32,
    pub constitution: f32,
    pub intelligence: f32,
    pub wisdom: f32,
    pub charisma: f32,
}
```

**Step 2: Register types for reflection**

In the same file, find the `plugin` function and add the new registrations:

```rust
pub(super) fn plugin(app: &mut App) {
    app.register_type::<HeroGrowth>();
    app.register_type::<HeroStatProgress>();
    app.add_systems(Startup, load_hero_databases);
    app.add_systems(OnEnter(Screen::Gameplay), spawn_starter_heroes);
}
```

(If the existing `plugin` fn doesn't register `Hero`/`HeroInfo`/`HeroStats`, that's fine — leave existing setup alone and just add the two new `register_type` calls.)

**Step 3: Verify it compiles**

Run: `cargo build`
Expected: compiles clean (one new `unused` warning for `HeroGrowth`/`HeroStatProgress` is acceptable — the next tasks consume them).

**Step 4: Commit**

```bash
git add src/hero/mod.rs
git commit -m "feat(hero): add HeroGrowth and HeroStatProgress components"
```

---

## Task 2: Implement `roll_growth` helper with TDD

**Files:**
- Modify: `src/hero/mod.rs` (add `roll_growth` fn + unit tests)

**Step 1: Write the failing tests**

At the bottom of `src/hero/mod.rs`, add:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::hero::data::{ClassDef, HeroClass, StatWeights};
    use rand::SeedableRng;
    use rand::rngs::StdRng;

    fn test_class(weights: StatWeights) -> ClassDef {
        ClassDef {
            id: HeroClass::Warrior,
            name: "Test".into(),
            description: "".into(),
            stat_weights: weights,
            starting_abilities: vec![],
        }
    }

    fn warrior_weights() -> StatWeights {
        StatWeights { str: 3, dex: 1, con: 3, int: 0, wis: 1, cha: 1 }
    }

    #[test]
    fn roll_growth_at_quality_zero_equals_class_floor() {
        let class = test_class(warrior_weights());
        let mut rng = StdRng::seed_from_u64(42);
        let g = roll_growth(&class, 0.0, &mut rng);
        // FLOOR_PER_WEIGHT = 0.2
        assert!((g.strength - 0.6).abs() < 1e-5);
        assert!((g.dexterity - 0.2).abs() < 1e-5);
        assert!((g.constitution - 0.6).abs() < 1e-5);
        assert!((g.intelligence - 0.0).abs() < 1e-5);
        assert!((g.wisdom - 0.2).abs() < 1e-5);
        assert!((g.charisma - 0.2).abs() < 1e-5);
    }

    #[test]
    fn roll_growth_at_quality_one_caps_at_floor_plus_max_random() {
        let class = test_class(warrior_weights());
        // Many seeds to stress the ceiling.
        for seed in 0..200 {
            let mut rng = StdRng::seed_from_u64(seed);
            let g = roll_growth(&class, 1.0, &mut rng);
            assert!(g.strength >= 0.6 - 1e-5 && g.strength <= 0.6 + 1.0 + 1e-5);
            assert!(g.intelligence >= 0.0 - 1e-5 && g.intelligence <= 0.0 + 1.0 + 1e-5);
        }
    }

    #[test]
    fn roll_growth_at_quality_half_caps_at_floor_plus_half_max() {
        let class = test_class(warrior_weights());
        for seed in 0..200 {
            let mut rng = StdRng::seed_from_u64(seed);
            let g = roll_growth(&class, 0.5, &mut rng);
            assert!(g.strength <= 0.6 + 0.5 + 1e-5);
            assert!(g.intelligence <= 0.0 + 0.5 + 1e-5);
        }
    }
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test --lib hero::tests`
Expected: compile error, `roll_growth` not defined.

**Step 3: Implement `roll_growth`**

Near the other helpers in `src/hero/mod.rs` (before the `#[cfg(test)]` block), add:

```rust
/// Per-stat growth floor contributed by each point of class weight.
const FLOOR_PER_WEIGHT: f32 = 0.2;
/// Maximum random portion added on top of the class floor (pre-quality scaling).
const MAX_RANDOM_GROWTH: f32 = 1.0;

/// Roll a `HeroGrowth` for a newly-generated hero.
///
/// `quality` is a 0.0..=1.0 scalar (computed from reputation tier +
/// RecruitmentOffice level). It gates the ceiling of the random portion
/// but never reduces the class floor.
pub fn roll_growth(class_def: &data::ClassDef, quality: f32, rng: &mut impl Rng) -> HeroGrowth {
    let q = quality.clamp(0.0, 1.0);
    let w = &class_def.stat_weights;
    let roll = |weight: i32, rng: &mut dyn rand::RngCore| -> f32 {
        let floor = weight.max(0) as f32 * FLOOR_PER_WEIGHT;
        let random_portion = rng.random::<f32>() * MAX_RANDOM_GROWTH * q;
        floor + random_portion
    };
    HeroGrowth {
        strength: roll(w.str, rng),
        dexterity: roll(w.dex, rng),
        constitution: roll(w.con, rng),
        intelligence: roll(w.int, rng),
        wisdom: roll(w.wis, rng),
        charisma: roll(w.cha, rng),
    }
}
```

Note: the closure takes `&mut dyn rand::RngCore` so it works with any `Rng` without forcing a generic parameter on the closure itself. If `rand::RngCore` isn't in scope yet, add `use rand::RngCore;` at the top of the file.

**Step 4: Run tests to verify they pass**

Run: `cargo test --lib hero::tests`
Expected: 3 tests pass.

**Step 5: Commit**

```bash
git add src/hero/mod.rs
git commit -m "feat(hero): add roll_growth helper with class floor + quality-gated randomness"
```

---

## Task 3: Implement `apply_growth_tick` and `award_xp` with TDD

**Files:**
- Modify: `src/hero/mod.rs` (add two fns + unit tests)

**Step 1: Write the failing tests**

Append to the existing `mod tests` block in `src/hero/mod.rs`:

```rust
    fn zero_stats() -> HeroStats {
        HeroStats {
            strength: 0, dexterity: 0, constitution: 0,
            intelligence: 0, wisdom: 0, charisma: 0,
        }
    }

    fn zero_progress() -> HeroStatProgress {
        HeroStatProgress::default()
    }

    fn info_at(level: u32, xp: u32, xp_to_next: u32) -> HeroInfo {
        HeroInfo {
            name: "T".into(),
            class: HeroClass::Warrior,
            level,
            xp,
            xp_to_next,
        }
    }

    #[test]
    fn apply_growth_tick_rate_zero_never_grows() {
        let mut stats = zero_stats();
        let mut prog = zero_progress();
        let growth = HeroGrowth {
            strength: 0.0, dexterity: 0.0, constitution: 0.0,
            intelligence: 0.0, wisdom: 0.0, charisma: 0.0,
        };
        for _ in 0..50 {
            apply_growth_tick(&mut stats, &growth, &mut prog);
        }
        assert_eq!(stats.strength, 0);
        assert_eq!(stats.intelligence, 0);
    }

    #[test]
    fn apply_growth_tick_rate_half_gains_one_every_two_levels() {
        let mut stats = zero_stats();
        let mut prog = zero_progress();
        let growth = HeroGrowth {
            strength: 0.5, dexterity: 0.0, constitution: 0.0,
            intelligence: 0.0, wisdom: 0.0, charisma: 0.0,
        };
        apply_growth_tick(&mut stats, &growth, &mut prog);
        assert_eq!(stats.strength, 0); // 0.5 accumulated
        apply_growth_tick(&mut stats, &growth, &mut prog);
        assert_eq!(stats.strength, 1); // 1.0 → +1, remainder 0.0
        apply_growth_tick(&mut stats, &growth, &mut prog);
        assert_eq!(stats.strength, 1); // 0.5 accumulated
        apply_growth_tick(&mut stats, &growth, &mut prog);
        assert_eq!(stats.strength, 2); // +1 again
    }

    #[test]
    fn apply_growth_tick_rate_0_3_gains_three_over_ten_levels() {
        let mut stats = zero_stats();
        let mut prog = zero_progress();
        let growth = HeroGrowth {
            strength: 0.3, dexterity: 0.0, constitution: 0.0,
            intelligence: 0.0, wisdom: 0.0, charisma: 0.0,
        };
        for _ in 0..10 {
            apply_growth_tick(&mut stats, &growth, &mut prog);
        }
        assert_eq!(stats.strength, 3); // floor(3.0)
    }

    #[test]
    fn award_xp_multi_level_applies_growth_per_level() {
        let mut info = info_at(1, 0, 100);
        let mut stats = zero_stats();
        let mut prog = zero_progress();
        let growth = HeroGrowth {
            strength: 1.0, dexterity: 0.0, constitution: 0.0,
            intelligence: 0.0, wisdom: 0.0, charisma: 0.0,
        };
        // 100 + 150 + 225 = 475 xp required to reach level 4.
        let ups = award_xp(&mut info, &mut stats, &growth, &mut prog, 500);
        assert_eq!(ups, 3);
        assert_eq!(info.level, 4);
        assert_eq!(stats.strength, 3); // rate 1.0 × 3 level-ups
        // Remaining xp: 500 - 475 = 25
        assert_eq!(info.xp, 25);
    }

    #[test]
    fn award_xp_partial_accumulator_carries_forward() {
        let mut info = info_at(1, 0, 100);
        let mut stats = zero_stats();
        let mut prog = zero_progress();
        let growth = HeroGrowth {
            strength: 0.6, dexterity: 0.0, constitution: 0.0,
            intelligence: 0.0, wisdom: 0.0, charisma: 0.0,
        };
        // One level-up.
        award_xp(&mut info, &mut stats, &growth, &mut prog, 100);
        assert_eq!(info.level, 2);
        assert_eq!(stats.strength, 0);
        assert!((prog.strength - 0.6).abs() < 1e-5);
        // Second level-up — 0.6 + 0.6 = 1.2 → +1, remainder 0.2.
        award_xp(&mut info, &mut stats, &growth, &mut prog, 150);
        assert_eq!(info.level, 3);
        assert_eq!(stats.strength, 1);
        assert!((prog.strength - 0.2).abs() < 1e-5);
    }
```

**Step 2: Run tests to verify they fail**

Run: `cargo test --lib hero::tests`
Expected: compile error, `apply_growth_tick` / `award_xp` not defined.

**Step 3: Implement both functions**

Near `roll_growth` in `src/hero/mod.rs`, add:

```rust
/// Apply one level's worth of growth: accumulator += rate, integer part flows
/// into `HeroStats`, fractional part stays in the accumulator.
pub fn apply_growth_tick(
    stats: &mut HeroStats,
    growth: &HeroGrowth,
    progress: &mut HeroStatProgress,
) {
    fn tick(stat: &mut i32, rate: f32, acc: &mut f32) {
        *acc += rate;
        let gained = acc.floor() as i32;
        *stat += gained;
        *acc -= gained as f32;
    }
    tick(&mut stats.strength, growth.strength, &mut progress.strength);
    tick(&mut stats.dexterity, growth.dexterity, &mut progress.dexterity);
    tick(&mut stats.constitution, growth.constitution, &mut progress.constitution);
    tick(&mut stats.intelligence, growth.intelligence, &mut progress.intelligence);
    tick(&mut stats.wisdom, growth.wisdom, &mut progress.wisdom);
    tick(&mut stats.charisma, growth.charisma, &mut progress.charisma);
}

/// Award XP to a hero and apply any resulting level-ups (including stat growth).
/// Returns the number of level-ups that occurred.
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
```

**Step 4: Run tests to verify they pass**

Run: `cargo test --lib hero::tests`
Expected: 5+3 = 8 tests pass.

**Step 5: Commit**

```bash
git add src/hero/mod.rs
git commit -m "feat(hero): add apply_growth_tick and award_xp helpers"
```

---

## Task 4: Attach growth to starter heroes

**Files:**
- Modify: `src/hero/mod.rs` — `spawn_random_hero`

**Step 1: Update `spawn_random_hero`**

`spawn_random_hero` is the debug/starter spawner. Starter heroes have no applicant-level quality input, so roll their growth at a **neutral quality of 0.5** (they're a gift from the engine, not a recruitment outcome).

In `src/hero/mod.rs`, modify `spawn_random_hero`: just before `commands.spawn((...))`, add:

```rust
    // Roll growth at neutral quality (starter heroes have no recruitment context).
    let growth = roll_growth(class_def, 0.5, rng);
```

Then extend the `commands.spawn((...))` tuple to include the new components:

```rust
    commands.spawn((
        Name::new(name.clone()),
        Hero,
        HeroInfo { /* unchanged */ },
        stats,
        HeroTraits(hero_traits),
        crate::equipment::HeroEquipment::default(),
        growth,
        HeroStatProgress::default(),
    ));
```

**Step 2: Verify build + existing tests still pass**

Run: `cargo build && cargo test --lib`
Expected: clean build; all tests pass.

**Step 3: Commit**

```bash
git add src/hero/mod.rs
git commit -m "feat(hero): attach HeroGrowth + HeroStatProgress to starter heroes"
```

---

## Task 5: Roll quality and growth during applicant generation; spawn with growth on hire

**Files:**
- Modify: `src/recruiting.rs`

**Step 1: Add constants and extend `Applicant`**

At the top of `src/recruiting.rs`, near the existing constants, add:

```rust
/// Denominator for the quality base roll. Rep tier max (5) + RO max (3) - 1 = 6 + offset.
const MAX_QUALITY_INPUT: f32 = 6.0;
/// Random spread applied to the quality base before clamping.
const QUALITY_JITTER: f32 = 0.2;
```

Extend the `Applicant` struct to carry the pre-rolled growth rates:

```rust
#[derive(Debug, Clone)]
pub struct Applicant {
    pub name: String,
    pub class: HeroClass,
    pub traits: Vec<HeroTrait>,
    pub stats: HeroStats,
    pub growth: crate::hero::HeroGrowth,
    pub hire_cost: u32,
    pub time_remaining: f32,
}
```

**Step 2: Wire quality into `generate_applicant`**

`generate_applicant` currently takes `&Reputation` but not `&GuildBuildings`. It needs the `RecruitmentOffice` level too. Update its signature:

```rust
fn generate_applicant(
    reputation: &Reputation,
    buildings: &GuildBuildings,
    class_db: &ClassDatabase,
    trait_db: &TraitDatabase,
    name_db: &NameDatabase,
    rng: &mut impl Rng,
) -> Applicant {
```

Inside, after stats are rolled and trait modifiers applied, compute quality and growth:

```rust
    // ── Quality roll (transient; not stored on the hero) ──────────────
    let office_level = buildings.level(crate::buildings::BuildingType::RecruitmentOffice) as f32;
    let rep_tier = reputation.tier() as f32;
    let quality_base = ((rep_tier - 1.0) + office_level) / MAX_QUALITY_INPUT;
    let quality = (quality_base + rng.random_range(-QUALITY_JITTER..=QUALITY_JITTER))
        .clamp(0.0, 1.0);

    let growth = crate::hero::roll_growth(class_def, quality, rng);
```

And include `growth` in the returned `Applicant`:

```rust
    Applicant {
        name,
        class: class_def.id,
        traits: hero_traits,
        stats,
        growth,
        hire_cost,
        time_remaining,
    }
```

**Step 3: Update both call sites of `generate_applicant`**

Both `tick_applicant_board` and `seed_applicant_board` call `generate_applicant`. Thread `&buildings` through. `tick_applicant_board` already takes `buildings: Res<GuildBuildings>` — just pass `&buildings`. For `seed_applicant_board`, add `buildings: Res<GuildBuildings>` to its parameters and pass `&buildings`.

Both call sites become:

```rust
let applicant = generate_applicant(&reputation, &buildings, &class_db, &trait_db, &name_db, &mut rng);
```

**Step 4: Spawn the hero with growth on hire**

In `handle_hire_applicant`, update the hero spawn to include the growth components:

```rust
    commands.spawn((
        Name::new(applicant.name.clone()),
        Hero,
        HeroInfo {
            name: applicant.name,
            class: applicant.class,
            level: 1,
            xp: 0,
            xp_to_next: 100,
        },
        applicant.stats,
        HeroTraits(applicant.traits),
        HeroEquipment::default(),
        applicant.growth,
        crate::hero::HeroStatProgress::default(),
    ));
```

**Step 5: Verify build**

Run: `cargo build`
Expected: clean build (the old `Applicant` literal in save-load deserialization will now need a `growth` field — Task 7 fixes that. If this task is committed separately, provide a throwaway default there temporarily, OR merge Tasks 5 and 7 into one commit. Recommended: proceed straight to Task 7 before committing Task 5.)

Actually: the save DTO reconstructs an `Applicant` in `load_save`, which will now fail to compile because `Applicant` requires a `growth` field. To keep this task compilable in isolation, add a temporary stopgap in `src/save.rs`'s applicant reconstruction:

```rust
    let applicants: Vec<Applicant> = save_data
        .applicants
        .iter()
        .map(|a| Applicant {
            // ...existing fields...
            growth: crate::hero::HeroGrowth {
                strength: 0.6, dexterity: 0.2, constitution: 0.6,
                intelligence: 0.0, wisdom: 0.2, charisma: 0.2,
            }, // TEMP: replaced in Task 7
            // ...
        })
        .collect();
```

Task 7 replaces this with a real deserialized field.

**Step 6: Commit**

```bash
git add src/recruiting.rs src/save.rs
git commit -m "feat(recruiting): roll hidden quality and per-hero growth rates on applicants"
```

---

## Task 6: Replace inline level-up loops with `award_xp`

**Files:**
- Modify: `src/training.rs`
- Modify: `src/mission/combat.rs`

**Step 1: Update `tick_training` in `src/training.rs`**

Change the heroes query to pick up the new components:

```rust
    mut heroes: Query<
        (&mut HeroInfo, &mut HeroStats, &crate::hero::HeroGrowth, &mut crate::hero::HeroStatProgress),
        (With<Hero>, Without<OnMission>),
    >,
```

Replace the inline level-up loop body. The whole `for mut info in &mut heroes { ... }` block becomes:

```rust
    for (mut info, mut stats, growth, mut progress) in &mut heroes {
        crate::hero::award_xp(&mut info, &mut stats, growth, &mut progress, xp_per_tick);
    }
```

**Step 2: Update `check_mission_completion` in `src/mission/combat.rs`**

The existing code (around line 315–326) runs per-survivor. Change the `hero_infos` query to include the new components:

```rust
    // In the system signature — update the hero-write query:
    mut hero_infos: Query<
        (&mut HeroInfo, &mut HeroStats, &crate::hero::HeroGrowth, &mut crate::hero::HeroStatProgress),
        With<Hero>,
    >,
```

Replace the survivor XP loop with:

```rust
        let mut level_ups = 0u32;
        for hero_entity in &survivors {
            if let Ok((mut hinfo, mut hstats, hgrowth, mut hprog)) = hero_infos.get_mut(*hero_entity) {
                level_ups += crate::hero::award_xp(
                    &mut hinfo,
                    &mut hstats,
                    hgrowth,
                    &mut hprog,
                    total_xp,
                );
            }
        }
```

(If `hero_infos` was previously queried with a narrower filter, make sure the new query doesn't conflict with other queries in the same system. Run the next step — `cargo check` — to catch borrow conflicts early.)

**Step 3: Verify build + existing tests**

Run: `cargo build && cargo test --lib`
Expected: clean build; tests pass.

**Step 4: Play-test manually (optional but quick)**

Run: `cargo run`
Expected: Training Grounds bumps a hero's stats when you let it tick enough; roster screen reflects the stat change. Mission completion shows the existing "N level ups!" toast and stats grew on the survivors.

**Step 5: Commit**

```bash
git add src/training.rs src/mission/combat.rs
git commit -m "feat(hero): apply stat growth on level-up in training and mission XP"
```

---

## Task 7: Save / load the new components (with migration)

**Files:**
- Modify: `src/save.rs`

**Step 1: Add save DTOs**

In the DTO section of `src/save.rs`, add:

```rust
#[derive(Serialize, Deserialize, Default)]
pub struct HeroGrowthSave {
    pub strength: f32,
    pub dexterity: f32,
    pub constitution: f32,
    pub intelligence: f32,
    pub wisdom: f32,
    pub charisma: f32,
}

#[derive(Serialize, Deserialize, Default)]
pub struct HeroStatProgressSave {
    pub strength: f32,
    pub dexterity: f32,
    pub constitution: f32,
    pub intelligence: f32,
    pub wisdom: f32,
    pub charisma: f32,
}
```

**Step 2: Extend `HeroSaveDto` and `ApplicantSaveDto`**

Add two fields to `HeroSaveDto` with `#[serde(default)]` so old saves deserialize:

```rust
#[derive(Serialize, Deserialize)]
pub struct HeroSaveDto {
    // ...existing fields...
    pub equipment: HeroEquipmentSave,
    pub on_mission: bool,
    #[serde(default)]
    pub growth: HeroGrowthSave,
    #[serde(default)]
    pub progress: HeroStatProgressSave,
}
```

Add one field to `ApplicantSaveDto`:

```rust
#[derive(Serialize, Deserialize)]
pub struct ApplicantSaveDto {
    // ...existing fields...
    pub time_remaining: f32,
    #[serde(default)]
    pub growth: HeroGrowthSave,
}
```

**Step 3: Serialize the new data in `handle_save`**

Find `handle_save`. The heroes query needs the new components:

```rust
    heroes: Query<
        (
            Entity,
            &HeroInfo,
            &HeroStats,
            &HeroTraits,
            &HeroEquipment,
            &crate::hero::HeroGrowth,
            &crate::hero::HeroStatProgress,
            Option<&OnMission>,
        ),
        With<Hero>,
    >,
```

Update the destructuring and DTO construction:

```rust
    for (entity, info, stats, traits, equipment, growth, progress, on_mission) in &heroes {
        // ...existing...
        hero_dtos.push(HeroSaveDto {
            // ...existing fields...
            on_mission: on_mission.is_some(),
            growth: HeroGrowthSave {
                strength: growth.strength,
                dexterity: growth.dexterity,
                constitution: growth.constitution,
                intelligence: growth.intelligence,
                wisdom: growth.wisdom,
                charisma: growth.charisma,
            },
            progress: HeroStatProgressSave {
                strength: progress.strength,
                dexterity: progress.dexterity,
                constitution: progress.constitution,
                intelligence: progress.intelligence,
                wisdom: progress.wisdom,
                charisma: progress.charisma,
            },
        });
    }
```

For applicants, extend the DTO builder similarly:

```rust
    let applicant_dtos: Vec<ApplicantSaveDto> = applicant_board
        .applicants
        .iter()
        .map(|a| ApplicantSaveDto {
            // ...existing fields...
            time_remaining: a.time_remaining,
            growth: HeroGrowthSave {
                strength: a.growth.strength,
                dexterity: a.growth.dexterity,
                constitution: a.growth.constitution,
                intelligence: a.growth.intelligence,
                wisdom: a.growth.wisdom,
                charisma: a.growth.charisma,
            },
        })
        .collect();
```

**Step 4: Deserialize the new data in `load_save`**

Remove the temporary stopgap `growth: HeroGrowth { ... TEMP ... }` from Task 5. Replace with real deserialization:

```rust
    let applicants: Vec<Applicant> = save_data
        .applicants
        .iter()
        .map(|a| Applicant {
            // ...existing fields...
            time_remaining: a.time_remaining,
            growth: crate::hero::HeroGrowth {
                strength: a.growth.strength,
                dexterity: a.growth.dexterity,
                constitution: a.growth.constitution,
                intelligence: a.growth.intelligence,
                wisdom: a.growth.wisdom,
                charisma: a.growth.charisma,
            },
        })
        .collect();
```

For heroes, extend the spawn bundle:

```rust
    for dto in &save_data.heroes {
        let entity = commands
            .spawn((
                Name::new(dto.name.clone()),
                Hero,
                HeroInfo { /* unchanged */ },
                HeroStats { /* unchanged */ },
                HeroTraits(dto.traits.clone()),
                HeroEquipment { /* unchanged */ },
                crate::hero::HeroGrowth {
                    strength: dto.growth.strength,
                    dexterity: dto.growth.dexterity,
                    constitution: dto.growth.constitution,
                    intelligence: dto.growth.intelligence,
                    wisdom: dto.growth.wisdom,
                    charisma: dto.growth.charisma,
                },
                crate::hero::HeroStatProgress {
                    strength: dto.progress.strength,
                    dexterity: dto.progress.dexterity,
                    constitution: dto.progress.constitution,
                    intelligence: dto.progress.intelligence,
                    wisdom: dto.progress.wisdom,
                    charisma: dto.progress.charisma,
                },
            ))
            .id();
        hero_entities.push(entity);
    }
```

**Step 5: Backfill for legacy saves**

Because `HeroGrowthSave` derives `Default` and the field is `#[serde(default)]`, old saves deserialize with all-zero growth. An all-zero growth means the hero never gains stats — functionally a bug-like regression for existing players. Backfill: if the deserialized growth is all zeros, roll a neutral-quality (0.5) growth for that hero's class.

Add a small helper in `src/save.rs` near `load_save`:

```rust
fn is_zero_growth(g: &HeroGrowthSave) -> bool {
    g.strength == 0.0 && g.dexterity == 0.0 && g.constitution == 0.0
        && g.intelligence == 0.0 && g.wisdom == 0.0 && g.charisma == 0.0
}
```

Then in `load_save`, add a one-shot post-load system OR do the backfill inline: before spawning a hero, if `is_zero_growth(&dto.growth)`, call `roll_growth` using a fresh `rand::rng()` and the hero's class. Pseudocode:

```rust
    let class_db = ... // Need ClassDatabase to look up the class def.
```

`load_save` currently does not take `Res<ClassDatabase>`. Easiest path: add `class_db: Res<crate::hero::data::ClassDatabase>` to the `load_save` signature. Then:

```rust
    let growth = if is_zero_growth(&dto.growth) {
        let Some(class_def) = class_db.0.iter().find(|c| c.id == dto.class) else {
            // Fallback to explicit defaults if the class isn't found.
            crate::hero::HeroGrowth {
                strength: 0.0, dexterity: 0.0, constitution: 0.0,
                intelligence: 0.0, wisdom: 0.0, charisma: 0.0,
            }
        };
        let mut rng = rand::rng();
        crate::hero::roll_growth(class_def, 0.5, &mut rng)
    } else {
        crate::hero::HeroGrowth {
            strength: dto.growth.strength,
            // ...etc
        }
    };
```

Do the same check for applicants (they're also serialized, and legacy applicant DTOs will have zero growth).

**Step 6: Save round-trip test**

Add a test at the bottom of `src/save.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hero_save_dto_round_trips_with_growth() {
        let dto = HeroSaveDto {
            name: "A".into(),
            class: HeroClass::Warrior,
            level: 3,
            xp: 42,
            xp_to_next: 200,
            stats: HeroStatsSave { strength: 12, dexterity: 10, constitution: 14,
                                   intelligence: 8, wisdom: 9, charisma: 10 },
            traits: vec![],
            equipment: HeroEquipmentSave { weapon_tier: 0, armor_tier: 0, accessory_tier: 0 },
            on_mission: false,
            growth: HeroGrowthSave { strength: 1.1, dexterity: 0.3, constitution: 0.8,
                                     intelligence: 0.0, wisdom: 0.4, charisma: 0.2 },
            progress: HeroStatProgressSave { strength: 0.5, dexterity: 0.0, constitution: 0.2,
                                             intelligence: 0.0, wisdom: 0.1, charisma: 0.0 },
        };
        let s = ron::ser::to_string(&dto).unwrap();
        let back: HeroSaveDto = ron::from_str(&s).unwrap();
        assert_eq!(back.growth.strength, 1.1);
        assert_eq!(back.progress.strength, 0.5);
    }

    #[test]
    fn legacy_hero_save_dto_without_growth_deserializes_with_defaults() {
        // A RON string missing `growth` and `progress` fields.
        let legacy = r#"(
            name: "L",
            class: Warrior,
            level: 2, xp: 0, xp_to_next: 150,
            stats: (strength: 10, dexterity: 10, constitution: 10,
                    intelligence: 10, wisdom: 10, charisma: 10),
            traits: [],
            equipment: (weapon_tier: 0, armor_tier: 0, accessory_tier: 0),
            on_mission: false,
        )"#;
        let dto: HeroSaveDto = ron::from_str(legacy).unwrap();
        assert!(is_zero_growth(&dto.growth));
        assert_eq!(dto.progress.strength, 0.0);
    }
}
```

**Step 7: Run the new tests**

Run: `cargo test --lib save::tests`
Expected: both tests pass.

**Step 8: Verify the full build + all tests**

Run: `cargo build && cargo test --lib`
Expected: clean build; all tests pass.

**Step 9: Commit**

```bash
git add src/save.rs
git commit -m "feat(save): persist HeroGrowth and HeroStatProgress with legacy backfill"
```

---

## Task 8: End-to-end smoke test and cleanup

**Files:**
- None — this is a manual verification + cleanup pass.

**Step 1: Manual smoke test**

Run: `cargo run`

Verify the following in a fresh (deleted save) session:

1. Hire an applicant; no visible UI change (quality is hidden — correct).
2. Dispatch the hired hero on a mission; after completion, if they leveled, check the roster — their `HeroStats` should have changed for stats with non-zero growth rates. (Warriors should see STR/CON growth; rarely INT.)
3. Let the Training Grounds tick a few times (accelerate if you have a speed button). Same check.
4. Save and reload the game; stats and XP persist, and a hero who was mid-accumulator (e.g., STR 0.6 progress) still progresses normally after the first reload-induced level-up.

**Step 2: Legacy save migration smoke test**

Copy your *pre-Task-5* save file (if you kept one) into the data dir, launch, and verify:
- Heroes load without crashing.
- Their growth rates are non-zero after one level-up (the backfill kicked in).

If no legacy save is handy, fabricate one by editing the current save.ron to remove all `growth:` and `progress:` fields from heroes and applicants, then load.

**Step 3: Quick grep for stale references**

Run: `cargo build 2>&1 | grep -i warning | grep -vE "never (read|constructed|used)"`
Expected: no *new* warnings introduced by this feature (pre-existing `fields never read` warnings in `src/hero/data.rs` are fine).

**Step 4: Run the whole test suite one more time**

Run: `cargo test`
Expected: all tests pass.

**Step 5: Optional tuning commit**

The design doc suggests all constants are tunable. If playtesting reveals growth feels too weak or too strong, tweak `FLOOR_PER_WEIGHT`, `MAX_RANDOM_GROWTH`, `MAX_QUALITY_INPUT`, or `QUALITY_JITTER` and commit separately as `tune(hero): adjust growth rate constants`.

**Step 6: Final push**

No additional commit required — commits from Tasks 1–7 are all the functional change. Push when ready:

```bash
git push
```

---

## Summary

- **7 commits of functional work** (Task 8 is verification).
- **Only 4 files changed**: `src/hero/mod.rs`, `src/recruiting.rs`, `src/training.rs`, `src/mission/combat.rs`, `src/save.rs`.
- **Zero UI changes** — quality is implicit by design.
- **Zero combat changes** — `CombatStats` already derives from `HeroStats`.
- **Migration is transparent** — legacy saves backfill on load.
