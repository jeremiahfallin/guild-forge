//! Hero system: data, components, and generation.

pub mod data;
pub mod status;
pub mod status_tick;

use bevy::prelude::*;
use rand::Rng;

use crate::screens::Screen;
use data::*;

pub(super) fn plugin(app: &mut App) {
    app.register_type::<HeroGrowth>();
    app.register_type::<HeroStatProgress>();
    app.register_type::<Favorite>();
    app.register_type::<PersonallyManaged>();
    app.register_type::<status::Missing>();
    app.register_type::<status::Injured>();
    app.add_plugins(status_tick::plugin);
    app.add_systems(Startup, load_hero_databases);
    app.add_systems(OnEnter(Screen::Gameplay), spawn_starter_heroes);
}

/// Marker component for hero entities.
#[derive(Component, Debug, Reflect)]
#[reflect(Component)]
pub struct Hero;

/// UI-prominence flag. Favorited heroes are pinned at the top of lists,
/// highlighted in mission feeds, and surfaced as priority events in the
/// eventual Field Report dashboard. Purely cosmetic — does not affect
/// game rules.
#[derive(Component, Debug, Reflect)]
#[reflect(Component)]
pub struct Favorite;

/// Opt-in flag indicating the player wants to manage this hero by hand
/// rather than let the (future) Dispatcher auto-assign them. When the
/// Dispatcher lands, it will skip heroes with this component. Has no
/// behavioral effect yet — displayed only.
#[derive(Component, Debug, Reflect)]
#[reflect(Component)]
pub struct PersonallyManaged;

/// Core identity information for a hero.
#[derive(Component, Debug, Reflect)]
#[reflect(Component)]
pub struct HeroInfo {
    pub name: String,
    pub class: HeroClass,
    pub level: u32,
    pub xp: u32,
    pub xp_to_next: u32,
}

/// The six core stats for a hero.
#[derive(Component, Debug, Clone, Reflect)]
#[reflect(Component)]
pub struct HeroStats {
    pub strength: i32,
    pub dexterity: i32,
    pub constitution: i32,
    pub intelligence: i32,
    pub wisdom: i32,
    pub charisma: i32,
}

/// Personality traits that affect behavior and stat growth.
#[derive(Component, Debug, Reflect)]
#[reflect(Component)]
pub struct HeroTraits(pub Vec<HeroTrait>);

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

/// Load all hero databases from RON files at startup.
fn load_hero_databases(mut commands: Commands) {
    let classes_str = include_str!("../../assets/data/classes.ron");
    let classes: Vec<ClassDef> = ron::from_str(classes_str).expect("Failed to parse classes.ron");
    commands.insert_resource(ClassDatabase(classes));

    let traits_str = include_str!("../../assets/data/traits.ron");
    let traits: Vec<TraitDef> = ron::from_str(traits_str).expect("Failed to parse traits.ron");
    commands.insert_resource(TraitDatabase(traits));

    let names_str = include_str!("../../assets/data/names.ron");
    let names: NamePool = ron::from_str(names_str).expect("Failed to parse names.ron");
    commands.insert_resource(NameDatabase(names));

    info!("Hero databases loaded");
}

/// Spawn 3 starter heroes when entering gameplay for the first time.
fn spawn_starter_heroes(
    mut commands: Commands,
    existing_heroes: Query<(), With<Hero>>,
    class_db: Res<ClassDatabase>,
    trait_db: Res<TraitDatabase>,
    name_db: Res<NameDatabase>,
) {
    if !existing_heroes.is_empty() || crate::save::has_save_file() {
        return;
    }

    let mut rng = rand::rng();
    for _ in 0..3 {
        spawn_random_hero(&mut commands, &class_db, &trait_db, &name_db, &mut rng);
    }

    info!("Spawned 3 starter heroes");
}

/// Generate and spawn a random hero entity.
fn spawn_random_hero(
    commands: &mut Commands,
    class_db: &ClassDatabase,
    trait_db: &TraitDatabase,
    name_db: &NameDatabase,
    rng: &mut impl Rng,
) {
    // Pick random class
    let class_idx = rng.random_range(0..class_db.0.len());
    let class_def = &class_db.0[class_idx];

    // Pick 1-2 random traits (no duplicates)
    let num_traits = rng.random_range(1..=2);
    let mut trait_indices: Vec<usize> = Vec::new();
    while trait_indices.len() < num_traits {
        let idx = rng.random_range(0..trait_db.0.len());
        if !trait_indices.contains(&idx) {
            trait_indices.push(idx);
        }
    }
    let hero_traits: Vec<HeroTrait> = trait_indices.iter().map(|&i| trait_db.0[i].id).collect();

    // Generate name
    let first = &name_db.0.first_names[rng.random_range(0..name_db.0.first_names.len())];
    let surname = &name_db.0.surnames[rng.random_range(0..name_db.0.surnames.len())];
    let name = format!("{first} {surname}");

    // Roll stats: base 8 + class weights * rand(1..=2) + trait modifiers
    let base = 8;
    let w = &class_def.stat_weights;
    let mut stats = HeroStats {
        strength: base + w.str * rng.random_range(1..=2),
        dexterity: base + w.dex * rng.random_range(1..=2),
        constitution: base + w.con * rng.random_range(1..=2),
        intelligence: base + w.int * rng.random_range(1..=2),
        wisdom: base + w.wis * rng.random_range(1..=2),
        charisma: base + w.cha * rng.random_range(1..=2),
    };

    // Apply trait modifiers
    for hero_trait in &hero_traits {
        if let Some(trait_def) = trait_db.get(*hero_trait) {
            let m = &trait_def.stat_modifiers;
            stats.strength += m.str;
            stats.dexterity += m.dex;
            stats.constitution += m.con;
            stats.intelligence += m.int;
            stats.wisdom += m.wis;
            stats.charisma += m.cha;
        }
    }

    // XP to next level: 100 for level 1 → 2
    let xp_to_next = 100;

    // Roll growth at neutral quality (starter heroes have no recruitment context).
    let growth = roll_growth(class_def, 0.5, rng);

    commands.spawn((
        Name::new(name.clone()),
        Hero,
        HeroInfo {
            name,
            class: class_def.id,
            level: 1,
            xp: 0,
            xp_to_next,
        },
        stats,
        HeroTraits(hero_traits),
        crate::equipment::HeroEquipment::default(),
        growth,
        HeroStatProgress::default(),
    ));
}

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
    let mut roll = |weight: i32| -> f32 {
        let floor = weight.max(0) as f32 * FLOOR_PER_WEIGHT;
        let random_portion = rng.random::<f32>() * MAX_RANDOM_GROWTH * q;
        floor + random_portion
    };
    HeroGrowth {
        strength: roll(w.str),
        dexterity: roll(w.dex),
        constitution: roll(w.con),
        intelligence: roll(w.int),
        wisdom: roll(w.wis),
        charisma: roll(w.cha),
    }
}

/// Apply one level's worth of growth: accumulator += rate, integer part flows
/// into `HeroStats`, fractional part stays in the accumulator.
pub fn apply_growth_tick(
    stats: &mut HeroStats,
    growth: &HeroGrowth,
    progress: &mut HeroStatProgress,
) {
    fn tick(stat: &mut i32, rate: f32, acc: &mut f32) {
        *acc += rate.max(0.0);
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
        info.xp_to_next = info.xp_to_next + info.xp_to_next / 2;
        apply_growth_tick(stats, growth, progress);
        level_ups += 1;
    }
    level_ups
}

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
        assert_eq!(stats.strength, 0);
        apply_growth_tick(&mut stats, &growth, &mut prog);
        assert_eq!(stats.strength, 1);
        apply_growth_tick(&mut stats, &growth, &mut prog);
        assert_eq!(stats.strength, 1);
        apply_growth_tick(&mut stats, &growth, &mut prog);
        assert_eq!(stats.strength, 2);
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
        assert_eq!(stats.strength, 3);
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
        let ups = award_xp(&mut info, &mut stats, &growth, &mut prog, 500);
        assert_eq!(ups, 3);
        assert_eq!(info.level, 4);
        assert_eq!(stats.strength, 3);
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
        award_xp(&mut info, &mut stats, &growth, &mut prog, 100);
        assert_eq!(info.level, 2);
        assert_eq!(stats.strength, 0);
        assert!((prog.strength - 0.6).abs() < 1e-5);
        award_xp(&mut info, &mut stats, &growth, &mut prog, 150);
        assert_eq!(info.level, 3);
        assert_eq!(stats.strength, 1);
        assert!((prog.strength - 0.2).abs() < 1e-5);
    }

    #[test]
    fn apply_growth_tick_advances_all_six_stats() {
        let mut stats = zero_stats();
        let mut prog = zero_progress();
        let growth = HeroGrowth {
            strength: 1.0, dexterity: 1.0, constitution: 1.0,
            intelligence: 1.0, wisdom: 1.0, charisma: 1.0,
        };
        apply_growth_tick(&mut stats, &growth, &mut prog);
        assert_eq!(stats.strength, 1);
        assert_eq!(stats.dexterity, 1);
        assert_eq!(stats.constitution, 1);
        assert_eq!(stats.intelligence, 1);
        assert_eq!(stats.wisdom, 1);
        assert_eq!(stats.charisma, 1);
    }
}
