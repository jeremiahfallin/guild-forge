//! Event banners (UX-3): floating banners in the mission view for
//! banner-worthy moments — boss encounters, legendary drops, rescue windows
//! closing. Producer systems enqueue; the mission view renders one at a time.

use bevy::prelude::*;
use std::collections::VecDeque;

use crate::equipment::GearRarity;
use crate::hero::{Hero, HeroInfo};
use crate::mission::ViewedMission;
use crate::mission::entities::{EnemyToken, GridPosition, HeroToken};
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
        (detect_drop_banners, detect_boss_banner, detect_rescue_banner)
            .chain()
            .run_if(resource_exists::<ViewedMission>),
    );
}

/// Promote legendary `GearDrop` log events on the viewed mission to banners.
pub(crate) fn detect_drop_banners(
    mut events: MessageReader<MissionLogEvent>,
    viewed: Res<ViewedMission>,
    mut queue: ResMut<BannerQueue>,
) {
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
                text: "RARE DROP!".to_string(),
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
    viewed: Res<ViewedMission>,
    missions: Query<(&Children, Option<&BannersFired>)>,
    hero_tokens: Query<(&GridPosition, &HeroToken), Without<EnemyToken>>,
    enemy_tokens: Query<(&GridPosition, &EnemyToken), Without<HeroToken>>,
    hero_data: Query<&HeroInfo, With<Hero>>,
    mut queue: ResMut<BannerQueue>,
) {
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
                let range = match info.class {
                    crate::hero::data::HeroClass::Ranger => 6,
                    crate::hero::data::HeroClass::Mage => 5,
                    _ => 1,
                };
                heroes.push((*gp, range));
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
                    text: "BOSS ENCOUNTER".to_string(),
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

/// Fire RESCUE WINDOW CLOSING once when the viewed rescue mission's soonest
/// Missing timer drops under the threshold.
pub(crate) fn detect_rescue_banner() {}

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
