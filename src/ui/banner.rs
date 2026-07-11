//! Event banners (UX-3): floating banners in the mission view for
//! banner-worthy moments — boss encounters, legendary drops, rescue windows
//! closing. Producer systems enqueue; the mission view renders one at a time.

use bevy::prelude::*;
use std::collections::VecDeque;

use crate::equipment::GearRarity;
use crate::hero::status::Missing;
use crate::localization::tr;
use crate::hero::{Hero, HeroInfo};
use crate::mission::entities::{EnemyToken, GridPosition, HeroToken};
use crate::mission::{RescueMission, ViewedMission};
use crate::ui::feed::{MissionLogEvent, MissionLogPayload};
use bevy::ecs::message::MessageReader;

/// Category of banner — drives styling in the mission view.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BannerKind {
    Boss,
    RareDrop,
    RescueClosing,
}

/// A queued banner announcement.
#[derive(Debug, Clone)]
pub struct BannerRequest {
    pub text: String,
    pub subtitle: Option<String>,
    pub kind: BannerKind,
}

/// Pending banners plus the one currently showing (with its elapsed real
/// seconds). `active` is promoted from `pending` as banners expire.
#[derive(Resource, Debug, Default)]
pub struct BannerQueue {
    pub pending: VecDeque<BannerRequest>,
    pub active: Option<(BannerRequest, f32)>,
    /// Mission the queue currently belongs to — reset when the view changes.
    pub mission: Option<Entity>,
}

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<BannerQueue>();
    app.add_systems(
        Update,
        (
            detect_drop_banners,
            detect_boss_banner,
            detect_rescue_banner,
            tick_banner_queue,
        )
            .chain()
            .run_if(resource_exists::<ViewedMission>),
    );
}

/// Promote legendary `GearDrop` log events on the viewed mission to banners.
pub(crate) fn detect_drop_banners(
    mut events: MessageReader<MissionLogEvent>,
    viewed: Option<Res<ViewedMission>>,
    mut queue: ResMut<BannerQueue>,
) {
    // Option: ViewedMission can vanish mid-frame when the watched mission
    // resolves — after the chain's run_if already passed.
    let Some(viewed) = viewed else { return };
    for event in events.read() {
        if event.mission_entity != viewed.0 {
            continue;
        }
        if let MissionLogPayload::GearDrop {
            item_name, rarity, ..
        } = &event.payload
            && *rarity == GearRarity::Legendary
        {
            queue.pending.push_back(BannerRequest {
                text: tr("banner.rare_drop").to_string(),
                subtitle: Some(item_name.clone()),
                kind: BannerKind::RareDrop,
            });
        }
    }
}

/// One-shot bookkeeping so boss / rescue banners fire once per mission.
#[derive(Component, Debug, Default)]
pub struct BannersFired {
    pub boss: bool,
    pub rescue: bool,
}

/// Fire BOSS ENCOUNTER the first time a boss-tier enemy on the viewed mission
/// comes within a hero's action range — the same overlap test that flips the
/// sim into combat tempo (`update_simulation_tempo`).
pub(crate) fn detect_boss_banner(
    mut commands: Commands,
    viewed: Option<Res<ViewedMission>>,
    missions: Query<(&Children, Option<&BannersFired>)>,
    hero_tokens: Query<(&GridPosition, &HeroToken), Without<EnemyToken>>,
    enemy_tokens: Query<(&GridPosition, &EnemyToken), Without<HeroToken>>,
    hero_data: Query<&HeroInfo, With<Hero>>,
    mut queue: ResMut<BannerQueue>,
) {
    let Some(viewed) = viewed else { return };
    let Ok((children, fired)) = missions.get(viewed.0) else {
        return;
    };
    if fired.is_some_and(|f| f.boss) {
        return;
    }

    let mut heroes = Vec::new();
    let mut bosses = Vec::new();
    for &child in children {
        if let Ok((gp, token)) = hero_tokens.get(child) {
            if let Ok(info) = hero_data.get(token.0) {
                heroes.push((*gp, crate::mission::hero_action_range(&info.class)));
            }
        } else if let Ok((gp, token)) = enemy_tokens.get(child)
            && token.enemy_type.is_boss()
        {
            bosses.push((*gp, token.enemy_type));
        }
    }

    for (h_gp, h_range) in &heroes {
        for (b_gp, boss_type) in &bosses {
            let dist = h_gp.x.abs_diff(b_gp.x) + h_gp.y.abs_diff(b_gp.y);
            if dist <= *h_range {
                queue.pending.push_back(BannerRequest {
                    text: tr("banner.boss").to_string(),
                    subtitle: Some(boss_type.to_string()),
                    kind: BannerKind::Boss,
                });
                commands
                    .entity(viewed.0)
                    .entry::<BannersFired>()
                    .and_modify(|mut f| f.boss = true)
                    .or_insert(BannersFired {
                        boss: true,
                        ..default()
                    });
                return;
            }
        }
    }
}

/// Game-seconds remaining on the Missing window below which the
/// RESCUE WINDOW CLOSING banner fires (window is 120s — last quarter).
pub const RESCUE_BANNER_THRESHOLD_SECS: f64 = 30.0;

pub const BANNER_SLIDE_SECS: f32 = 0.3;
pub const BANNER_HOLD_SECS: f32 = 2.5;
pub const BANNER_FADE_SECS: f32 = 0.5;
pub const BANNER_TOTAL_SECS: f32 = BANNER_SLIDE_SECS + BANNER_HOLD_SECS + BANNER_FADE_SECS;

/// Opacity over the banner's lifetime: ramp up during slide-in, solid hold,
/// ramp down during fade.
pub fn banner_alpha(elapsed: f32) -> f32 {
    if elapsed <= 0.0 {
        0.0
    } else if elapsed < BANNER_SLIDE_SECS {
        elapsed / BANNER_SLIDE_SECS
    } else if elapsed < BANNER_SLIDE_SECS + BANNER_HOLD_SECS {
        1.0
    } else if elapsed < BANNER_TOTAL_SECS {
        1.0 - (elapsed - BANNER_SLIDE_SECS - BANNER_HOLD_SECS) / BANNER_FADE_SECS
    } else {
        0.0
    }
}

/// Advance the active banner, promote pending ones, and reset the queue when
/// the viewed mission changes.
pub(crate) fn tick_banner_queue(
    viewed: Option<Res<ViewedMission>>,
    time: Res<Time>,
    mut queue: ResMut<BannerQueue>,
) {
    let Some(viewed) = viewed else { return };
    if queue.mission != Some(viewed.0) {
        queue.pending.clear();
        queue.active = None;
        queue.mission = Some(viewed.0);
        return;
    }
    if let Some((_, ref mut elapsed)) = queue.active {
        *elapsed += time.delta_secs();
        if *elapsed >= BANNER_TOTAL_SECS {
            queue.active = None;
        }
    }
    if queue.active.is_none()
        && let Some(next) = queue.pending.pop_front()
    {
        queue.active = Some((next, 0.0));
    }
}

/// Fire RESCUE WINDOW CLOSING once when the viewed rescue mission's soonest
/// Missing timer drops under the threshold.
pub(crate) fn detect_rescue_banner(
    mut commands: Commands,
    viewed: Option<Res<ViewedMission>>,
    missions: Query<(&RescueMission, Option<&BannersFired>)>,
    missing_q: Query<&Missing>,
    time: Res<Time<Virtual>>,
    mut queue: ResMut<BannerQueue>,
) {
    let Some(viewed) = viewed else { return };
    let Ok((rescue, fired)) = missions.get(viewed.0) else {
        return;
    };
    if fired.is_some_and(|f| f.rescue) {
        return;
    }
    let now = time.elapsed_secs_f64();
    let soonest = rescue
        .rescue_heroes
        .iter()
        .filter_map(|&h| missing_q.get(h).ok())
        .map(|m| m.expires_at - now)
        .fold(f64::INFINITY, f64::min);
    if soonest.is_finite() && soonest < RESCUE_BANNER_THRESHOLD_SECS {
        queue.pending.push_back(BannerRequest {
            text: tr("banner.rescue_closing").to_string(),
            subtitle: None,
            kind: BannerKind::RescueClosing,
        });
        commands
            .entity(viewed.0)
            .entry::<BannersFired>()
            .and_modify(|mut f| f.rescue = true)
            .or_insert(BannersFired {
                rescue: true,
                ..default()
            });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::equipment::GearRarity;
    use crate::ui::feed::{MissionLogEvent, MissionLogPayload};
    use bevy::ecs::message::{MessageWriter, Messages};
    use bevy::ecs::system::RunSystemOnce;

    fn send_drop(world: &mut World, mission: Entity, rarity: GearRarity) {
        let _ = world.run_system_once(move |mut w: MessageWriter<MissionLogEvent>| {
            w.write(MissionLogEvent {
                mission_entity: mission,
                payload: MissionLogPayload::GearDrop {
                    hero_name: "Sera".to_string(),
                    item_name: "Doomblade".to_string(),
                    rarity,
                    affix: None,
                    stats_desc: "+5 ATK".to_string(),
                },
            });
        });
    }

    #[test]
    fn banner_systems_survive_missing_viewed_mission() {
        // Regression: mission completion removes ViewedMission mid-frame,
        // after the chain's run_if already passed — a bare Res<ViewedMission>
        // then fails param validation, which panics at runtime in Bevy 0.18.
        let mut world = World::new();
        world.init_resource::<BannerQueue>();
        world.init_resource::<Messages<MissionLogEvent>>();
        world.insert_resource(Time::<()>::default());
        world.insert_resource(Time::<Virtual>::default());

        assert!(world.run_system_once(detect_drop_banners).is_ok());
        assert!(world.run_system_once(detect_boss_banner).is_ok());
        assert!(world.run_system_once(detect_rescue_banner).is_ok());
        assert!(world.run_system_once(tick_banner_queue).is_ok());
    }

    #[test]
    fn legendary_drop_on_viewed_mission_enqueues_banner() {
        let mut world = World::new();
        world.init_resource::<Messages<MissionLogEvent>>();
        world.init_resource::<BannerQueue>();
        let mission = world.spawn_empty().id();
        world.insert_resource(ViewedMission(mission));

        send_drop(&mut world, mission, GearRarity::Legendary);
        let _ = world.run_system_once(detect_drop_banners);

        let queue = world.resource::<BannerQueue>();
        assert_eq!(queue.pending.len(), 1);
        let req = &queue.pending[0];
        assert_eq!(req.kind, BannerKind::RareDrop);
        assert_eq!(req.text, "RARE DROP!");
        assert_eq!(req.subtitle.as_deref(), Some("Doomblade"));
    }

    #[test]
    fn epic_drop_does_not_enqueue_banner() {
        let mut world = World::new();
        world.init_resource::<Messages<MissionLogEvent>>();
        world.init_resource::<BannerQueue>();
        let mission = world.spawn_empty().id();
        world.insert_resource(ViewedMission(mission));

        send_drop(&mut world, mission, GearRarity::Epic);
        let _ = world.run_system_once(detect_drop_banners);

        assert!(world.resource::<BannerQueue>().pending.is_empty());
    }

    use crate::hero::{Hero, HeroInfo};
    use crate::mission::data::EnemyType;
    use crate::mission::entities::{EnemyToken, GridPosition, HeroToken};

    fn spawn_boss_fixture(world: &mut World, boss_pos: (u32, u32)) -> Entity {
        let hero = world
            .spawn((
                Hero,
                HeroInfo {
                    name: "Alice".to_string(),
                    class: crate::hero::data::HeroClass::Warrior,
                    level: 1,
                    xp: 0,
                    xp_to_next: 100,
                },
            ))
            .id();
        let mission = world.spawn_empty().id();
        let hero_token = world
            .spawn((HeroToken(hero), GridPosition { x: 5, y: 5 }))
            .id();
        let boss_token = world
            .spawn((
                EnemyToken {
                    enemy_type: EnemyType::BossRat,
                    xp_reward: 50,
                },
                GridPosition {
                    x: boss_pos.0,
                    y: boss_pos.1,
                },
            ))
            .id();
        world
            .entity_mut(mission)
            .add_children(&[hero_token, boss_token]);
        world.insert_resource(ViewedMission(mission));
        mission
    }

    #[test]
    fn boss_in_combat_range_enqueues_banner_once() {
        let mut world = World::new();
        world.init_resource::<BannerQueue>();
        let _mission = spawn_boss_fixture(&mut world, (6, 5)); // adjacent, warrior range 1

        let _ = world.run_system_once(detect_boss_banner);
        let _ = world.run_system_once(detect_boss_banner); // second tick must not refire

        let queue = world.resource::<BannerQueue>();
        assert_eq!(queue.pending.len(), 1);
        assert_eq!(queue.pending[0].kind, BannerKind::Boss);
        assert_eq!(queue.pending[0].text, "BOSS ENCOUNTER");
        assert_eq!(queue.pending[0].subtitle.as_deref(), Some("Boss Rat"));
    }

    #[test]
    fn boss_out_of_range_does_not_enqueue() {
        let mut world = World::new();
        world.init_resource::<BannerQueue>();
        let _mission = spawn_boss_fixture(&mut world, (20, 20));

        let _ = world.run_system_once(detect_boss_banner);

        assert!(world.resource::<BannerQueue>().pending.is_empty());
    }

    #[test]
    fn non_boss_enemy_in_range_does_not_enqueue() {
        let mut world = World::new();
        world.init_resource::<BannerQueue>();
        let hero = world
            .spawn((
                Hero,
                HeroInfo {
                    name: "Alice".to_string(),
                    class: crate::hero::data::HeroClass::Warrior,
                    level: 1,
                    xp: 0,
                    xp_to_next: 100,
                },
            ))
            .id();
        let mission = world.spawn_empty().id();
        let ht = world
            .spawn((HeroToken(hero), GridPosition { x: 5, y: 5 }))
            .id();
        let et = world
            .spawn((
                EnemyToken {
                    enemy_type: EnemyType::Goblin,
                    xp_reward: 10,
                },
                GridPosition { x: 6, y: 5 },
            ))
            .id();
        world.entity_mut(mission).add_children(&[ht, et]);
        world.insert_resource(ViewedMission(mission));

        let _ = world.run_system_once(detect_boss_banner);

        assert!(world.resource::<BannerQueue>().pending.is_empty());
    }

    use crate::hero::status::Missing;
    use crate::mission::RescueMission;

    fn spawn_rescue_fixture(world: &mut World, expires_at: f64) -> Entity {
        world.init_resource::<Time<Virtual>>(); // elapsed starts at 0.0
        let missing_hero = world
            .spawn(Missing {
                expires_at,
                dropped_equipment: None,
            })
            .id();
        let mission = world
            .spawn(RescueMission {
                rescue_heroes: vec![missing_hero],
                gear_recovered: false,
            })
            .id();
        world.insert_resource(ViewedMission(mission));
        mission
    }

    #[test]
    fn rescue_under_30s_enqueues_banner_once() {
        let mut world = World::new();
        world.init_resource::<BannerQueue>();
        let _mission = spawn_rescue_fixture(&mut world, 20.0); // 20s left at t=0

        let _ = world.run_system_once(detect_rescue_banner);
        let _ = world.run_system_once(detect_rescue_banner);

        let queue = world.resource::<BannerQueue>();
        assert_eq!(queue.pending.len(), 1);
        assert_eq!(queue.pending[0].kind, BannerKind::RescueClosing);
        assert_eq!(queue.pending[0].text, "RESCUE WINDOW CLOSING");
    }

    #[test]
    fn rescue_over_30s_does_not_enqueue() {
        let mut world = World::new();
        world.init_resource::<BannerQueue>();
        let _mission = spawn_rescue_fixture(&mut world, 90.0);

        let _ = world.run_system_once(detect_rescue_banner);

        assert!(world.resource::<BannerQueue>().pending.is_empty());
    }

    #[test]
    fn tick_promotes_pending_to_active_and_expires_it() {
        let mut world = World::new();
        world.init_resource::<Time>(); // real time, starts at 0
        let mission = world.spawn_empty().id();
        world.insert_resource(ViewedMission(mission));
        let mut queue = BannerQueue {
            mission: Some(mission), // steady state: queue already bound to view
            ..default()
        };
        queue.pending.push_back(BannerRequest {
            text: "BOSS ENCOUNTER".to_string(),
            subtitle: None,
            kind: BannerKind::Boss,
        });
        queue.pending.push_back(BannerRequest {
            text: "RARE DROP!".to_string(),
            subtitle: None,
            kind: BannerKind::RareDrop,
        });
        world.insert_resource(queue);

        let _ = world.run_system_once(tick_banner_queue);
        assert_eq!(
            world.resource::<BannerQueue>().active.as_ref().unwrap().0.kind,
            BannerKind::Boss
        );

        // advance past the banner's total lifetime
        world
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_secs_f32(BANNER_TOTAL_SECS + 0.1));
        let _ = world.run_system_once(tick_banner_queue);
        assert_eq!(
            world.resource::<BannerQueue>().active.as_ref().unwrap().0.kind,
            BannerKind::RareDrop,
            "expired banner should be replaced by the next pending one"
        );
        assert!(world.resource::<BannerQueue>().pending.is_empty());
    }

    #[test]
    fn view_change_clears_queue() {
        let mut world = World::new();
        world.init_resource::<Time>();
        let mission_a = world.spawn_empty().id();
        let mission_b = world.spawn_empty().id();
        world.insert_resource(ViewedMission(mission_a));
        let mut queue = BannerQueue {
            mission: Some(mission_a),
            ..default()
        };
        queue.pending.push_back(BannerRequest {
            text: "BOSS ENCOUNTER".to_string(),
            subtitle: None,
            kind: BannerKind::Boss,
        });
        world.insert_resource(queue);

        let _ = world.run_system_once(tick_banner_queue); // activates on mission_a
        assert!(world.resource::<BannerQueue>().active.is_some());

        world.insert_resource(ViewedMission(mission_b));
        let _ = world.run_system_once(tick_banner_queue);

        let queue = world.resource::<BannerQueue>();
        assert!(queue.active.is_none(), "active banner must clear on view change");
        assert!(queue.pending.is_empty());
    }

    #[test]
    fn banner_alpha_phases() {
        assert_eq!(banner_alpha(0.0), 0.0); // slide-in start
        assert_eq!(banner_alpha(0.3), 1.0); // hold
        assert_eq!(banner_alpha(2.0), 1.0); // still holding
        assert!(banner_alpha(3.1) < 1.0); // fading
        assert_eq!(banner_alpha(BANNER_TOTAL_SECS), 0.0);
    }

    #[test]
    fn legendary_drop_on_unviewed_mission_does_not_enqueue() {
        let mut world = World::new();
        world.init_resource::<Messages<MissionLogEvent>>();
        world.init_resource::<BannerQueue>();
        let viewed = world.spawn_empty().id();
        let other = world.spawn_empty().id();
        world.insert_resource(ViewedMission(viewed));

        send_drop(&mut world, other, GearRarity::Legendary);
        let _ = world.run_system_once(detect_drop_banners);

        assert!(world.resource::<BannerQueue>().pending.is_empty());
    }
}
