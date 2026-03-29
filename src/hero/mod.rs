//! Hero system: data, components, and generation.

pub mod data;

use bevy::prelude::*;
use rand::Rng;

use crate::screens::Screen;
use data::*;

pub(super) fn plugin(app: &mut App) {
    app.add_systems(Startup, load_hero_databases);
    app.add_systems(OnEnter(Screen::Gameplay), spawn_starter_heroes);
}

/// Marker component for hero entities.
#[derive(Component, Debug, Reflect)]
#[reflect(Component)]
pub struct Hero;

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
#[derive(Component, Debug, Reflect)]
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
    if !existing_heroes.is_empty() {
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
    ));
}
