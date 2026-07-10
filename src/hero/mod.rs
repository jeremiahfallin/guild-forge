//! Hero system: data, components, and generation.

pub mod data;
pub mod history;
pub mod status;
pub mod status_tick;
pub mod epithet;
pub mod portrait;
pub mod perk;

pub use history::HeroHistory;
pub use epithet::{Epithet, format_hero_name, check_epithet};
pub use portrait::HeroPortrait;

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
    app.register_type::<Fatigue>();
    app.register_type::<history::HeroHistory>();
    app.register_type::<epithet::Epithet>();
    app.register_type::<perk::VeteranPerk>();
    app.add_plugins((status_tick::plugin, portrait::PortraitPlugin));
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

/// Stamina/Fatigue system component.
#[derive(Component, Debug, Clone, Copy, Reflect)]
#[reflect(Component)]
pub struct Fatigue {
    pub current: f32,
    pub max_base: f32,
}

impl Fatigue {
    pub fn max(&self, level: u32, constitution: i32) -> f32 {
        self.max_base + ((level.saturating_sub(1)) as f32 * 5.0) + (constitution as f32 * 2.0)
    }
}

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
#[derive(Component, Debug, Reflect, Clone)]
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

/// Spawn 2 starter heroes and 60 gold when entering gameplay for the first
/// time. The third hero is the tutorial's recruit beat (FT-1).
fn spawn_starter_heroes(
    mut commands: Commands,
    existing_heroes: Query<(), With<Hero>>,
    class_db: Res<ClassDatabase>,
    trait_db: Res<TraitDatabase>,
    name_db: Res<NameDatabase>,
    mut gold: ResMut<crate::economy::Gold>,
) {
    if !existing_heroes.is_empty() || crate::save::has_save_file() {
        return;
    }

    let mut rng = rand::rng();
    for _ in 0..2 {
        spawn_random_hero(&mut commands, &class_db, &trait_db, &name_db, &mut rng);
    }
    gold.0 = 60; // FT-1: enough to hire the clamped starter applicant

    info!("Spawned 2 starter heroes with 60g");
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

    let base_move_range = match class_def.id {
        HeroClass::Rogue | HeroClass::Ranger => 4,
        _ => 3,
    };

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
        Fatigue { current: 100.0, max_base: 100.0 },
        crate::mission::entities::MoveRange {
            base: base_move_range,
            bonus: 0,
        },
        history::HeroHistory::default(),
        Epithet(None),
        portrait::HeroPortrait::random(rng),
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
    mut fatigue: Option<&mut Fatigue>,
    xp: u32,
) -> u32 {
    let old_max = fatigue.as_ref().map(|f| f.max(info.level, stats.constitution)).unwrap_or(0.0);
    info.xp += xp;
    let mut level_ups = 0;
    while info.xp >= info.xp_to_next {
        info.xp -= info.xp_to_next;
        info.level += 1;
        info.xp_to_next = info.xp_to_next + info.xp_to_next / 2;
        apply_growth_tick(stats, growth, progress);
        level_ups += 1;
    }
    if level_ups > 0
        && let Some(ref mut f) = fatigue {
            let new_max = f.max(info.level, stats.constitution);
            let diff = new_max - old_max;
            if diff > 0.0 {
                f.current += diff;
            }
        }
    level_ups
}

pub fn track_hero_history_system(
    mut events: bevy::ecs::message::MessageReader<crate::ui::feed::MissionLogEvent>,
    missions: Query<(&crate::mission::MissionParty, &crate::mission::MissionInfo, &Children)>,
    mut heroes: Query<(Entity, &HeroInfo, &mut history::HeroHistory, &mut Epithet)>,
    ability_db: Option<Res<crate::hero::data::AbilityDatabase>>,
    mut hero_tokens: Query<(&crate::mission::entities::HeroToken, &mut Name, &crate::mission::entities::CombatStats)>,
    mut low_hp_track: Local<std::collections::HashMap<Entity, std::collections::HashSet<Entity>>>,
) {
    let mut enemy_last_attacked_by: std::collections::HashMap<String, String> = std::collections::HashMap::new();

    for event in events.read() {
        match &event.payload {
            crate::ui::feed::MissionLogPayload::Attack { attacker, defender, is_hit, .. } => {
                if *is_hit {
                    enemy_last_attacked_by.insert(defender.clone(), attacker.clone());
                    
                    // If defender is a hero, let's see if their HP is <= 20%
                    if let Some(roster_ent) = find_hero_by_name(&heroes, defender) {
                        if let Some(stats) = find_token_by_roster(&hero_tokens, &missions, event.mission_entity, roster_ent) {
                            if stats.hp <= stats.max_hp / 5 {
                                low_hp_track.entry(event.mission_entity).or_default().insert(roster_ent);
                            }
                        }
                    }
                }
            }
            crate::ui::feed::MissionLogPayload::Ability { attacker, defender, ability_name, is_hit, effect_type, .. } => {
                if *is_hit && (effect_type == "Damage" || effect_type == "Debuff") {
                    enemy_last_attacked_by.insert(defender.clone(), attacker.clone());

                    // If defender is a hero, let's see if their HP is <= 20%
                    if let Some(roster_ent) = find_hero_by_name(&heroes, defender) {
                        if let Some(stats) = find_token_by_roster(&hero_tokens, &missions, event.mission_entity, roster_ent) {
                            if stats.hp <= stats.max_hp / 5 {
                                low_hp_track.entry(event.mission_entity).or_default().insert(roster_ent);
                            }
                        }
                    }
                }

                // If attacker is a hero and it's a signature ability
                if let Some(roster_ent) = find_hero_by_name(&heroes, attacker) {
                    if let Some(ability_db) = &ability_db {
                        if let Some(ability_def) = ability_db.get(ability_name) {
                            if ability_def.is_signature {
                                if let Ok((_, _, mut hist, _)) = heroes.get_mut(roster_ent) {
                                    hist.add_timeline_entry(format!("Landed signature move: {}", ability_def.name));
                                }
                            }
                        }
                    }
                }
            }
            crate::ui::feed::MissionLogPayload::ChestOpened { hero_name, gold } => {
                if let Some(roster_ent) = find_hero_by_name(&heroes, hero_name) {
                    if let Ok((_, _, mut hist, _)) = heroes.get_mut(roster_ent) {
                        hist.lifetime_gold += gold;
                        hist.add_timeline_entry(format!("Found {} gold in a loot chest", gold));
                    }
                }
            }
            crate::ui::feed::MissionLogPayload::EventTriggered { event_name, hero_name, outcome_text, .. } => {
                if let Some(roster_ent) = find_hero_by_name(&heroes, hero_name) {
                    if let Ok((_, _, mut hist, _)) = heroes.get_mut(roster_ent) {
                        hist.add_timeline_entry(format!("Event: {} — {}", event_name, outcome_text));
                    }
                }
            }
            crate::ui::feed::MissionLogPayload::GearDrop { hero_name, item_name, rarity, .. } => {
                if let Some(roster_ent) = find_hero_by_name(&heroes, hero_name) {
                    if let Ok((_, _, mut hist, _)) = heroes.get_mut(roster_ent) {
                        hist.add_timeline_entry(format!("Found a {:?} {}", rarity, item_name));
                    }
                }
            }
            crate::ui::feed::MissionLogPayload::Death { name, is_enemy } => {
                if *is_enemy {
                    // Defeated enemy: lookup who did it
                    if let Some(killer_name) = enemy_last_attacked_by.get(name) {
                        if let Some(roster_ent) = find_hero_by_name(&heroes, killer_name) {
                            if let Ok((_, _, mut hist, _)) = heroes.get_mut(roster_ent) {
                                hist.kills += 1;
                                if name == "Boss Rat" {
                                    if let Ok((_, info, _)) = missions.get(event.mission_entity) {
                                        hist.add_timeline_entry(format!("Defeated the boss Boss Rat in {}", info.name));
                                    } else {
                                        hist.add_timeline_entry("Defeated the boss Boss Rat".to_string());
                                    }
                                } else {
                                    hist.add_timeline_entry(format!("Defeated {}", name));
                                }
                            }
                        }
                    }
                }
            }
            crate::ui::feed::MissionLogPayload::Loot { gold, .. } => {
                // Success!
                if let Ok((party, info, children)) = missions.get(event.mission_entity) {
                    // Survivors have HP > 0
                    let mut survivors = std::collections::HashSet::new();
                    for child in children.iter() {
                        if let Ok((token, _, stats)) = hero_tokens.get(child) {
                            if stats.hp > 0 {
                                survivors.insert(token.0);
                            }
                        }
                    }

                    // Process everyone in the party
                    for &roster_ent in party.0.iter() {
                        if let Ok((_, _, mut hist, _)) = heroes.get_mut(roster_ent) {
                            hist.missions_run += 1;
                            if survivors.contains(&roster_ent) {
                                hist.lifetime_gold += gold;
                                
                                if survivors.len() == 1 && party.0.len() > 1 {
                                    hist.add_timeline_entry(format!("Survived the {} wipe — sole survivor", info.name));
                                } else {
                                    hist.add_timeline_entry(format!("Completed mission: {}", info.name));
                                }

                                // Check near-death
                                if let Some(low_hps) = low_hp_track.get(&event.mission_entity) {
                                    if low_hps.contains(&roster_ent) {
                                        hist.near_deaths += 1;
                                        hist.add_timeline_entry(format!("Survived a near-death experience in {}", info.name));
                                    }
                                }
                            } else {
                                hist.add_timeline_entry(format!("Went missing in mission: {}", info.name));
                            }
                        }
                    }
                }
                low_hp_track.remove(&event.mission_entity);
            }
            crate::ui::feed::MissionLogPayload::Failure => {
                // Failure!
                if let Ok((party, info, _)) = missions.get(event.mission_entity) {
                    for &roster_ent in party.0.iter() {
                        if let Ok((_, _, mut hist, _)) = heroes.get_mut(roster_ent) {
                            hist.missions_run += 1;
                            hist.add_timeline_entry(format!("Went missing in mission: {}", info.name));
                        }
                    }
                }
                low_hp_track.remove(&event.mission_entity);
            }
            _ => {}
        }
    }

    // Update epithets and active token names
    for (roster_ent, info, mut hist, mut epithet) in heroes.iter_mut() {
        if let Some(new_title) = check_epithet(&hist) {
            let has_new = epithet.0.as_ref() != Some(&new_title);
            if has_new {
                epithet.0 = Some(new_title.clone());
                hist.add_timeline_entry(format!("Earned the title: {}", new_title));
                
                // Update Name component of any active battlefield HeroToken
                for (token, mut token_name, _) in hero_tokens.iter_mut() {
                    if token.0 == roster_ent {
                        let formatted = format_hero_name(&info.name, Some(&epithet));
                        *token_name = Name::new(format!("Hero Token: {}", formatted));
                    }
                }
            }
        }
    }
}

fn find_hero_by_name(
    heroes: &Query<(Entity, &HeroInfo, &mut history::HeroHistory, &mut Epithet)>,
    name: &str,
) -> Option<Entity> {
    for (entity, info, _, _) in heroes.iter() {
        if info.name == name || name.contains(&info.name) {
            return Some(entity);
        }
    }
    None
}

fn find_token_by_roster<'a>(
    hero_tokens: &'a Query<(&crate::mission::entities::HeroToken, &mut Name, &crate::mission::entities::CombatStats)>,
    missions: &Query<(&crate::mission::MissionParty, &crate::mission::MissionInfo, &Children)>,
    mission_entity: Entity,
    roster_entity: Entity,
) -> Option<&'a crate::mission::entities::CombatStats> {
    if let Ok((_, _, children)) = missions.get(mission_entity) {
        for child in children.iter() {
            if let Ok((token, _, stats)) = hero_tokens.get(child) {
                if token.0 == roster_entity {
                    return Some(stats);
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;
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
        let ups = award_xp(&mut info, &mut stats, &growth, &mut prog, None, 500);
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
        award_xp(&mut info, &mut stats, &growth, &mut prog, None, 100);
        assert_eq!(info.level, 2);
        assert_eq!(stats.strength, 0);
        assert!((prog.strength - 0.6).abs() < 1e-5);
        award_xp(&mut info, &mut stats, &growth, &mut prog, None, 150);
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

    #[test]
    fn fatigue_max_scaling_and_levelup_proportionate() {
        let mut fatigue = Fatigue { current: 50.0, max_base: 100.0 };
        // level = 1, constitution = 10.
        // max = 100 + (1 - 1)*5 + 10*2 = 120.
        assert_eq!(fatigue.max(1, 10), 120.0);

        // level = 2, constitution = 10.
        // max = 100 + (2 - 1)*5 + 10*2 = 125.
        assert_eq!(fatigue.max(2, 10), 125.0);

        // Test award_xp scaling fatigue current proportionally on level up.
        let mut info = info_at(1, 0, 100);
        let mut stats = HeroStats {
            strength: 10, dexterity: 10, constitution: 10,
            intelligence: 10, wisdom: 10, charisma: 10,
        };
        let mut prog = zero_progress();
        let growth = HeroGrowth {
            strength: 0.0,
            dexterity: 0.0,
            constitution: 0.0,
            intelligence: 0.0,
            wisdom: 0.0,
            charisma: 0.0,
        };

        // Let's level up once.
        // At level 1, max is 120. fatigue current is 50.
        // Level up to 2: new max is 125.
        // Expected new fatigue current = 50 + (125 - 120) = 55.0
        award_xp(&mut info, &mut stats, &growth, &mut prog, Some(&mut fatigue), 100);

        assert_eq!(info.level, 2);
        assert_eq!(fatigue.current, 55.0);
    }

    #[test]
    fn test_chronicle_history_accumulation() {
        use crate::hero::HeroHistory;
        use crate::mission::{Mission, MissionParty, MissionInfo};
        use crate::mission::entities::{HeroToken, CombatStats};
        use crate::ui::feed::{MissionLogEvent, MissionLogPayload};

        let mut world = World::new();
        // Register messages resource
        world.init_resource::<bevy::ecs::message::Messages<MissionLogEvent>>();
        
        // Spawn roster hero
        let hero_ent = world.spawn((
            Hero,
            HeroInfo {
                name: "Torgar Strong".to_string(),
                class: HeroClass::Warrior,
                level: 1,
                xp: 0,
                xp_to_next: 100,
            },
            HeroStats { strength: 10, dexterity: 10, constitution: 10, intelligence: 10, wisdom: 10, charisma: 10 },
            HeroTraits(vec![]),
            HeroHistory::default(),
            Epithet(None),
            HeroPortrait { base_idx: 0, hair_idx: 0, hair_color: Color::WHITE, gear_idx: 0 },
        )).id();

        // Spawn mission
        let mission_ent = world.spawn((
            Mission,
            MissionParty(vec![hero_ent]),
            MissionInfo {
                template_id: "test".to_string(),
                name: "Test Caves".to_string(),
                difficulty: 1,
                modifiers: vec![],
                biome: crate::mission::data::BiomeType::Dungeon,
            },
        )).id();

        // Spawn token child
        let token_ent = world.spawn((
            Name::new("Hero Token: Torgar Strong"),
            HeroToken(hero_ent),
            CombatStats { hp: 50, max_hp: 50, attack: 10, defense: 5, speed: 10 },
            ChildOf(mission_ent),
        )).id();
        world.entity_mut(mission_ent).add_child(token_ent);

        // Verify initial history
        {
            let hist = world.get::<HeroHistory>(hero_ent).unwrap();
            assert_eq!(hist.missions_run, 0);
            assert_eq!(hist.timeline.len(), 1);
            assert_eq!(hist.timeline[0], "Joined the guild");
        }

        // Write some events: Attack, Death, ChestOpened, Loot (success)
        use bevy::ecs::message::MessageWriter;
        let _ = world.run_system_once(move |mut w: MessageWriter<MissionLogEvent>| {
            w.write(MissionLogEvent {
                mission_entity: mission_ent,
                payload: MissionLogPayload::Attack {
                    attacker: "Torgar Strong".to_string(),
                    defender: "Orc".to_string(),
                    damage: 10,
                    is_crit: false,
                    is_hit: true,
                },
            });
            w.write(MissionLogEvent {
                mission_entity: mission_ent,
                payload: MissionLogPayload::Death {
                    name: "Orc".to_string(),
                    is_enemy: true,
                },
            });
            w.write(MissionLogEvent {
                mission_entity: mission_ent,
                payload: MissionLogPayload::ChestOpened {
                    hero_name: "Torgar Strong".to_string(),
                    gold: 50,
                },
            });
            w.write(MissionLogEvent {
                mission_entity: mission_ent,
                payload: MissionLogPayload::Loot {
                    gold: 100,
                    xp: 20,
                },
            });
        });

        // Run the system
        let _ = world.run_system_once(track_hero_history_system);

        // Verify accumulated history
        {
            let hist = world.get::<HeroHistory>(hero_ent).unwrap();
            assert_eq!(hist.missions_run, 1);
            assert_eq!(hist.kills, 1);
            assert_eq!(hist.lifetime_gold, 150); // 50 (chest) + 100 (loot)
            assert_eq!(hist.timeline.len(), 4);
            assert_eq!(hist.timeline[1], "Defeated Orc");
            assert_eq!(hist.timeline[2], "Found 50 gold in a loot chest");
            assert_eq!(hist.timeline[3], "Completed mission: Test Caves");
        }
    }

    #[test]
    fn test_epithet_mid_session_trigger() {
        use crate::mission::{Mission, MissionParty, MissionInfo};
        use crate::mission::entities::{HeroToken, CombatStats};
        use crate::ui::feed::{MissionLogEvent, MissionLogPayload};

        let mut world = World::new();
        // Register messages resource
        world.init_resource::<bevy::ecs::message::Messages<MissionLogEvent>>();
        
        // Spawn hero with 9 kills initially (1 away from Slimebane)
        let mut hist = HeroHistory::default();
        hist.kills = 9;

        let hero_ent = world.spawn((
            Hero,
            HeroInfo {
                name: "Torgar Strong".to_string(),
                class: HeroClass::Warrior,
                level: 1,
                xp: 0,
                xp_to_next: 100,
            },
            HeroStats { strength: 10, dexterity: 10, constitution: 10, intelligence: 10, wisdom: 10, charisma: 10 },
            HeroTraits(vec![]),
            hist,
            Epithet(None),
            HeroPortrait { base_idx: 0, hair_idx: 0, hair_color: Color::WHITE, gear_idx: 0 },
        )).id();

        // Spawn mission
        let mission_ent = world.spawn((
            Mission,
            MissionParty(vec![hero_ent]),
            MissionInfo {
                template_id: "test".to_string(),
                name: "Test Caves".to_string(),
                difficulty: 1,
                modifiers: vec![],
                biome: crate::mission::data::BiomeType::Dungeon,
            },
        )).id();

        // Spawn active combat token on the field with standard Name
        let token_name = Name::new("Hero Token: Torgar Strong");
        let token_ent = world.spawn((
            token_name,
            HeroToken(hero_ent),
            CombatStats { hp: 50, max_hp: 50, attack: 10, defense: 5, speed: 10 },
            ChildOf(mission_ent),
        )).id();
        world.entity_mut(mission_ent).add_child(token_ent);

        // Simulate the 10th kill
        use bevy::ecs::message::MessageWriter;
        let _ = world.run_system_once(move |mut w: MessageWriter<MissionLogEvent>| {
            w.write(MissionLogEvent {
                mission_entity: mission_ent,
                payload: MissionLogPayload::Attack {
                    attacker: "Torgar Strong".to_string(),
                    defender: "Orc".to_string(),
                    damage: 10,
                    is_crit: false,
                    is_hit: true,
                },
            });
            w.write(MissionLogEvent {
                mission_entity: mission_ent,
                payload: MissionLogPayload::Death {
                    name: "Orc".to_string(),
                    is_enemy: true,
                },
            });
        });

        // Run system to process kill, trigger Slimebane, and propagate title
        let _ = world.run_system_once(track_hero_history_system);

        // Verify that the hero has Slimebane title and the token's name is updated
        {
            let updated_epithet = world.get::<Epithet>(hero_ent).unwrap();
            assert_eq!(updated_epithet.0, Some("Slimebane".to_string()));

            let updated_hist = world.get::<HeroHistory>(hero_ent).unwrap();
            assert_eq!(updated_hist.kills, 10);
            assert!(updated_hist.timeline.contains(&"Earned the title: Slimebane".to_string()));

            let updated_token_name = world.get::<Name>(token_ent).unwrap();
            assert_eq!(updated_token_name.as_str(), "Hero Token: Torgar Strong Slimebane");
        }
    }

    #[test]
    fn test_perk_combat_stats_application() {
        use bevy::ecs::system::RunSystemOnce;
        use crate::hero::{HeroHistory, HeroStats, HeroInfo, Fatigue, Epithet, HeroPortrait};
        use crate::mission::entities::{spawn_tokens_for_mission, CombatStats, MoveRange};
        use crate::mission::MissionParty;
        use crate::equipment::{EquipmentDatabase, HeroEquipment};
        use crate::mission::data::{MissionTemplateDatabase, EnemyDatabase};

        let mut world = World::new();

        // 1. Create a hero with history that unlocks perks:
        // - rescues_given = 3: RescueSpecialist (+10% HP)
        // - kills = 20: Slayer (+2 Attack)
        let mut history = HeroHistory::default();
        history.rescues_given = 3;
        history.kills = 20;

        let hero_ent = world.spawn((
            Hero,
            HeroInfo {
                name: "Veteran Bob".to_string(),
                class: HeroClass::Warrior,
                level: 1,
                xp: 0,
                xp_to_next: 100,
            },
            // con = 10, str = 10, dex = 10
            HeroStats { strength: 10, dexterity: 10, constitution: 10, intelligence: 10, wisdom: 10, charisma: 10 },
            HeroTraits(vec![]),
            history,
            Epithet(None),
            HeroPortrait { base_idx: 0, hair_idx: 0, hair_color: Color::WHITE, gear_idx: 0 },
            Fatigue { current: 100.0, max_base: 100.0 },
            HeroEquipment::default(),
        )).id();

        // 2. Set up local variables for the capture
        let mission_ent = world.spawn_empty().id();
        let dungeon = crate::mission::dungeon::generate_dungeon(40, 30, 3, &mut rand::rng());
        let party = MissionParty(vec![hero_ent]);
        let equipment_db = EquipmentDatabase(vec![]);
        let templates = MissionTemplateDatabase(vec![]);
        let enemy_db = EnemyDatabase(vec![]);

        // System to execute spawn_tokens_for_mission
        let run_spawn = move |
            mut commands: Commands,
            hero_q: Query<(
                &HeroInfo,
                &HeroStats,
                Option<&HeroEquipment>,
                &Fatigue,
                Option<&MoveRange>,
                Option<&Epithet>,
                Option<&HeroHistory>,
            ), With<Hero>>,
            injured_q: Query<(), With<crate::hero::status::Injured>>,
        | {
            spawn_tokens_for_mission(
                &mut commands,
                mission_ent,
                &dungeon,
                &party,
                &hero_q,
                &equipment_db,
                &templates,
                &enemy_db,
                "test_template",
                &injured_q,
                None,
                &[],
            );
        };

        // Run the system
        let _ = world.run_system_once(run_spawn);

        // Verify that the HeroToken was spawned and has boosted stats
        let mut token_query = world.query::<(&crate::mission::entities::HeroToken, &CombatStats)>();
        let mut found = false;
        for (token, combat_stats) in token_query.iter(&world) {
            if token.0 == hero_ent {
                found = true;
                // Base HP = con(10) * 3 + level(1) * 5 = 35.
                // With RescueSpecialist (+10% HP), HP = 35 * 1.10 = 38 (floor).
                assert_eq!(combat_stats.max_hp, 38);
                assert_eq!(combat_stats.hp, 38);

                // Base Attack = (str(10) + dex(10)) / 2 = 10.
                // With Slayer (+2 Attack), Attack = 12.
                assert_eq!(combat_stats.attack, 12);
            }
        }
        assert!(found, "Hero token should have been spawned");
    }
}
