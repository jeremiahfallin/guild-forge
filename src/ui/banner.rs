//! Event banners (UX-3): floating banners in the mission view for
//! banner-worthy moments — boss encounters, legendary drops, rescue windows
//! closing. Producer systems enqueue; the mission view renders one at a time.

use bevy::prelude::*;
use std::collections::VecDeque;

use crate::equipment::GearRarity;
use crate::mission::ViewedMission;
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

/// Fire BOSS ENCOUNTER the first time a boss-tier enemy on the viewed mission
/// comes within a hero's action range.
pub(crate) fn detect_boss_banner() {}

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
