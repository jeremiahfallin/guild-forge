//! Applicant board: hero candidates arrive on a timer and can be hired.

use bevy::prelude::*;
use rand::Rng;

use crate::buildings::GuildBuildings;
use crate::economy::Gold;
use crate::equipment::HeroEquipment;
use crate::hero::data::*;
use crate::hero::{Hero, HeroInfo, HeroStats, HeroTraits};
use crate::reputation::Reputation;
use crate::screens::Screen;

/// How often (seconds) a new applicant arrives.
const ARRIVAL_INTERVAL: f32 = 3600.0;

/// Minimum availability window per applicant (seconds).
const MIN_AVAILABILITY: f32 = 14400.0;

/// Maximum availability window per applicant (seconds).
const MAX_AVAILABILITY: f32 = 28800.0;

/// Denominator for the quality base roll. Rep tier max (5) + RO max (3) - 1 = 6 + offset.
const MAX_QUALITY_INPUT: f32 = 6.0;
/// Random spread applied to the quality base before clamping.
const QUALITY_JITTER: f32 = 0.2;

/// A candidate hero waiting to be hired.
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

/// The applicant board resource tracking available candidates.
#[derive(Resource, Debug)]
pub struct ApplicantBoard {
    pub applicants: Vec<Applicant>,
    pub next_arrival_timer: f32,
}

impl Default for ApplicantBoard {
    fn default() -> Self {
        Self {
            applicants: Vec::new(),
            next_arrival_timer: ARRIVAL_INTERVAL,
        }
    }
}

/// Generate a random applicant, using reputation tier as a stat floor boost.
fn generate_applicant(
    reputation: &Reputation,
    buildings: &GuildBuildings,
    class_db: &ClassDatabase,
    trait_db: &TraitDatabase,
    name_db: &NameDatabase,
    rng: &mut impl Rng,
) -> Applicant {
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

    // Roll stats: base (8 + tier-1) + class weights * rand(1..=2) + trait modifiers
    let base = 8 + (reputation.tier() as i32 - 1);
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

    let stat_total = stats.strength + stats.dexterity + stats.constitution
        + stats.intelligence + stats.wisdom + stats.charisma;
    let hire_cost = 20 + stat_total as u32 * 2;

    let time_remaining = rng.random_range(MIN_AVAILABILITY..=MAX_AVAILABILITY);

    // ── Quality roll (transient; not stored on the hero) ──────────────
    let office_level = buildings.level(crate::buildings::BuildingType::RecruitmentOffice) as f32;
    let rep_tier = reputation.tier() as f32;
    let quality_base = ((rep_tier - 1.0) + office_level) / MAX_QUALITY_INPUT;
    let quality = (quality_base + rng.random_range(-QUALITY_JITTER..=QUALITY_JITTER))
        .clamp(0.0, 1.0);

    let growth = crate::hero::roll_growth(class_def, quality, rng);

    Applicant {
        name,
        class: class_def.id,
        traits: hero_traits,
        stats,
        growth,
        hire_cost,
        time_remaining,
    }
}

/// Tick the applicant board: decrement timers, remove expired, spawn new arrivals.
fn tick_applicant_board(
    time: Res<Time>,
    mut board: ResMut<ApplicantBoard>,
    buildings: Res<GuildBuildings>,
    reputation: Res<Reputation>,
    class_db: Res<ClassDatabase>,
    trait_db: Res<TraitDatabase>,
    name_db: Res<NameDatabase>,
) {
    let dt = time.delta_secs();

    // Decrement time on existing applicants, remove expired
    for applicant in &mut board.applicants {
        applicant.time_remaining -= dt;
    }
    board.applicants.retain(|a| a.time_remaining > 0.0);

    // Tick arrival timer
    board.next_arrival_timer -= dt;
    if board.next_arrival_timer <= 0.0 {
        board.next_arrival_timer = ARRIVAL_INTERVAL;

        let max = buildings.max_applicants() as usize;
        if board.applicants.len() < max {
            let mut rng = rand::rng();
            let applicant =
                generate_applicant(&reputation, &buildings, &class_db, &trait_db, &name_db, &mut rng);
            info!("New applicant arrived: {} ({})", applicant.name, applicant.class);
            board.applicants.push(applicant);
        }
    }
}

/// Seed the board with 2 initial applicants when entering gameplay (if empty).
fn seed_applicant_board(
    mut board: ResMut<ApplicantBoard>,
    reputation: Res<Reputation>,
    buildings: Res<GuildBuildings>,
    class_db: Res<ClassDatabase>,
    trait_db: Res<TraitDatabase>,
    name_db: Res<NameDatabase>,
) {
    if !board.applicants.is_empty() || crate::save::has_save_file() {
        return;
    }

    let mut rng = rand::rng();
    for _ in 0..2 {
        let applicant =
            generate_applicant(&reputation, &buildings, &class_db, &trait_db, &name_db, &mut rng);
        info!("Seeded applicant: {} ({})", applicant.name, applicant.class);
        board.applicants.push(applicant);
    }
}

/// Event: request to hire an applicant by index on the board.
#[derive(Event)]
pub struct HireApplicant(pub usize);

/// Observer handler for hiring an applicant.
fn handle_hire_applicant(
    trigger: On<HireApplicant>,
    mut commands: Commands,
    mut board: ResMut<ApplicantBoard>,
    mut gold: ResMut<Gold>,
    buildings: Res<GuildBuildings>,
    existing_heroes: Query<(), With<Hero>>,
) {
    let idx = trigger.event().0;

    // Validate index
    if idx >= board.applicants.len() {
        warn!("HireApplicant: invalid index {}", idx);
        return;
    }

    // Check roster cap
    let hero_count = existing_heroes.iter().count() as u32;
    if hero_count >= buildings.roster_cap() {
        warn!(
            "HireApplicant: roster full ({}/{})",
            hero_count,
            buildings.roster_cap()
        );
        return;
    }

    // Check gold
    let cost = board.applicants[idx].hire_cost;
    if gold.0 < cost {
        warn!("HireApplicant: not enough gold ({} < {})", gold.0, cost);
        return;
    }

    // Deduct gold and remove from board
    gold.0 -= cost;
    let applicant = board.applicants.remove(idx);

    let hero_name = applicant.name.clone();
    let hero_class = applicant.class;

    // Spawn hero entity
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

    commands.trigger(crate::ui::toast::ToastEvent {
        title: format!("{hero_name} joined the guild!"),
        body: format!("{hero_class} — hired for {cost}g"),
        kind: crate::ui::toast::ToastKind::Success,
    });

    info!("Hired applicant for {} gold", cost);
}

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<ApplicantBoard>();
    app.add_observer(handle_hire_applicant);
    app.add_systems(OnEnter(Screen::Gameplay), seed_applicant_board);
    app.add_systems(Update, tick_applicant_board);
}
