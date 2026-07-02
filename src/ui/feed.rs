use bevy::prelude::*;
use bevy::ecs::message::{Message, MessageReader};
use serde::{Deserialize, Serialize};
use rand::seq::IndexedRandom;
use crate::equipment::{GearRarity, BehavioralAffix};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect, Serialize, Deserialize, Default)]
pub enum LogKind {
    #[default]
    Combat,
    Heal,
    RoomEntry,
    Completion,
    Failure,
    Death,
    Info,
    Legendary,
}

#[derive(Debug, Clone, Reflect, Serialize, Deserialize, Default)]
pub struct MissionLogEntry {
    pub text: String,
    pub kind: LogKind,
    #[serde(default)]
    pub hero_name: Option<String>,
}

#[derive(Component, Debug, Clone, Default, Reflect, Serialize, Deserialize)]
#[reflect(Component)]
pub struct MissionLogHistory {
    pub logs: Vec<MissionLogEntry>,
}

#[derive(Resource, Debug, Clone, Deserialize)]
pub struct NarrativeTemplates {
    pub attack_hit: Vec<String>,
    pub attack_crit: Vec<String>,
    pub attack_miss: Vec<String>,
    pub heal: Vec<String>,
    pub death_hero: Vec<String>,
    pub death_enemy: Vec<String>,
    pub room_entry: Vec<String>,
    pub mission_complete: Vec<String>,
    pub mission_failed: Vec<String>,
}

#[derive(Message, Debug, Clone)]
pub struct MissionLogEvent {
    pub mission_entity: Entity,
    pub payload: MissionLogPayload,
}

#[derive(Debug, Clone)]
pub enum MissionLogPayload {
    Attack {
        attacker: String,
        defender: String,
        damage: i32,
        is_crit: bool,
        is_hit: bool,
    },
    Heal {
        healer: String,
        target: String,
        amount: i32,
    },
    Death {
        name: String,
        is_enemy: bool,
    },
    RoomEntry {
        hero_name: String,
        room_name: String,
    },
    Loot {
        gold: u32,
        xp: u32,
    },
    Failure,
    Ability {
        attacker: String,
        defender: String,
        ability_name: String,
        amount: i32,
        is_hit: bool,
        is_crit: bool,
        effect_type: String,
    },
    ChestOpened {
        hero_name: String,
        gold: u32,
    },
    TrapTriggered {
        hero_name: String,
        damage: i32,
    },
    EventTriggered {
        event_name: String,
        hero_name: String,
        description: String,
        outcome_text: String,
    },
    GearDrop {
        hero_name: String,
        item_name: String,
        rarity: GearRarity,
        affix: Option<BehavioralAffix>,
        stats_desc: String,
    },
}

pub(super) fn plugin(app: &mut App) {
    app.add_message::<MissionLogEvent>();
    app.add_systems(Update, process_log_events);
}

pub(crate) fn process_log_events(
    mut events: MessageReader<MissionLogEvent>,
    mut missions: Query<&mut MissionLogHistory>,
    templates: Option<Res<NarrativeTemplates>>,
) {
    let Some(templates) = templates else { return };
    let mut rng = rand::rng();

    for event in events.read() {
        let Ok(mut history) = missions.get_mut(event.mission_entity) else {
            continue;
        };

        let (text, kind) = format_payload(&event.payload, &templates, &mut rng);
        let hero_name = extract_hero_name(&event.payload);
        history.logs.push(MissionLogEntry { text, kind, hero_name });
    }
}

fn format_payload(
    payload: &MissionLogPayload,
    templates: &NarrativeTemplates,
    rng: &mut impl rand::Rng,
) -> (String, LogKind) {
    match payload {
        MissionLogPayload::Attack {
            attacker,
            defender,
            damage,
            is_crit,
            is_hit,
        } => {
            if *is_hit {
                let template_list = if *is_crit {
                    &templates.attack_crit
                } else {
                    &templates.attack_hit
                };
                let template = template_list
                    .choose(rng)
                    .map(|s| s.as_str())
                    .unwrap_or("{attacker} strikes {defender} for {damage} damage!");
                let text = template
                    .replace("{attacker}", attacker)
                    .replace("{defender}", defender)
                    .replace("{damage}", &damage.to_string());
                (text, LogKind::Combat)
            } else {
                let template = templates
                    .attack_miss
                    .choose(rng)
                    .map(|s| s.as_str())
                    .unwrap_or("{attacker} swings at {defender} but misses!");
                let text = template
                    .replace("{attacker}", attacker)
                    .replace("{defender}", defender);
                (text, LogKind::Combat)
            }
        }
        MissionLogPayload::Heal {
            healer,
            target,
            amount,
        } => {
            let template = templates
                .heal
                .choose(rng)
                .map(|s| s.as_str())
                .unwrap_or("{healer} heals {target} for {amount} HP!");
            let text = template
                .replace("{healer}", healer)
                .replace("{target}", target)
                .replace("{amount}", &amount.to_string());
            (text, LogKind::Heal)
        }
        MissionLogPayload::Death { name, is_enemy } => {
            let template_list = if *is_enemy {
                &templates.death_enemy
            } else {
                &templates.death_hero
            };
            let template = template_list
                .choose(rng)
                .map(|s| s.as_str())
                .unwrap_or("{name} has fallen!");
            let text = template.replace("{name}", name);
            (text, LogKind::Death)
        }
        MissionLogPayload::RoomEntry {
            hero_name,
            room_name,
        } => {
            let template = templates
                .room_entry
                .choose(rng)
                .map(|s| s.as_str())
                .unwrap_or("{hero} leads the party into the {room}.");
            let text = template
                .replace("{hero}", hero_name)
                .replace("{room}", room_name);
            (text, LogKind::RoomEntry)
        }
        MissionLogPayload::Loot { gold, xp } => {
            let template = templates
                .mission_complete
                .choose(rng)
                .map(|s| s.as_str())
                .unwrap_or("Mission complete! Earned {gold} gold and {xp} XP!");
            let text = template
                .replace("{gold}", &gold.to_string())
                .replace("{xp}", &xp.to_string());
            (text, LogKind::Completion)
        }
        MissionLogPayload::Failure => {
            let template = templates
                .mission_failed
                .choose(rng)
                .map(|s| s.as_str())
                .unwrap_or("Mission failed! All heroes have fallen.");
            (template.to_string(), LogKind::Failure)
        }
        MissionLogPayload::Ability {
            attacker,
            defender,
            ability_name,
            amount,
            is_hit,
            is_crit,
            effect_type,
        } => {
            if ability_name == "Boss Slam" && *amount == 0 {
                (format!("{} starts winding up a massive Boss Slam!", attacker), LogKind::Combat)
            } else if ability_name == "Boss Summon" {
                (format!("{} screeches and summons giant rats!", attacker), LogKind::Combat)
            } else if ability_name == "Boss Enrage" {
                (format!("{} grows ENRAGED! Its power doubles!", attacker), LogKind::Combat)
            } else if effect_type == "Damage" || effect_type == "Debuff" {
                if *is_hit {
                    let text = if *is_crit {
                        format!("Critical hit! {} hits {} with {} for {} damage!", attacker, defender, ability_name, amount)
                    } else {
                        format!("{} strikes {} with {} for {} damage!", attacker, defender, ability_name, amount)
                    };
                    (text, LogKind::Combat)
                } else {
                    let text = format!("{} uses {} on {} but misses!", attacker, ability_name, defender);
                    (text, LogKind::Combat)
                }
            } else if effect_type == "Heal" {
                let text = format!("{} casts {} on {} — healing for {} HP!", attacker, ability_name, defender, amount);
                (text, LogKind::Heal)
            } else { // Shield / Buff
                let text = format!("{} invokes {} — shielding for {} points!", attacker, ability_name, amount);
                (text, LogKind::Heal)
            }
        }
        MissionLogPayload::ChestOpened { hero_name, gold } => {
            let text = format!("{} opens a chest and finds {} gold!", hero_name, gold);
            (text, LogKind::Completion)
        }
        MissionLogPayload::TrapTriggered { hero_name, damage } => {
            let text = format!("Trap! {} triggers a pressure plate and takes {} damage!", hero_name, damage);
            (text, LogKind::Combat)
        }
        MissionLogPayload::EventTriggered { event_name, hero_name, description, outcome_text } => {
            let text = format!("[{}] {} spots {} — {} {}", event_name, hero_name, description, hero_name, outcome_text);
            (text, LogKind::Info)
        }
        MissionLogPayload::GearDrop {
            hero_name,
            item_name,
            rarity,
            affix,
            stats_desc,
        } => {
            let affix_str = match affix {
                Some(BehavioralAffix::Lifesteal) => " [Lifesteal]",
                Some(BehavioralAffix::Initiative) => " [+Initiative]",
                Some(BehavioralAffix::CleaveOnHit) => " [Cleave]",
                None => "",
            };
            let text = match rarity {
                GearRarity::Legendary => format!("★ LEGENDARY DROP! {} found the Legendary {} ({}{})! ★", hero_name, item_name, stats_desc, affix_str),
                GearRarity::Epic => format!("{} found an Epic {} ({}{})!", hero_name, item_name, stats_desc, affix_str),
                GearRarity::Rare => format!("{} found a Rare {} ({}{})!", hero_name, item_name, stats_desc, affix_str),
                GearRarity::Uncommon => format!("{} found an Uncommon {} ({}{})!", hero_name, item_name, stats_desc, affix_str),
                GearRarity::Common => format!("{} found a Common {} ({})!", hero_name, item_name, stats_desc),
            };
            let kind = match rarity {
                GearRarity::Legendary => LogKind::Legendary,
                _ => LogKind::Info,
            };
            (text, kind)
        }
    }
}

pub fn extract_hero_name(payload: &MissionLogPayload) -> Option<String> {
    match payload {
        MissionLogPayload::Attack { attacker, .. } => Some(attacker.clone()),
        MissionLogPayload::Heal { healer, .. } => Some(healer.clone()),
        MissionLogPayload::RoomEntry { hero_name, .. } => Some(hero_name.clone()),
        MissionLogPayload::ChestOpened { hero_name, .. } => Some(hero_name.clone()),
        MissionLogPayload::TrapTriggered { hero_name, .. } => Some(hero_name.clone()),
        MissionLogPayload::EventTriggered { hero_name, .. } => Some(hero_name.clone()),
        MissionLogPayload::GearDrop { hero_name, .. } => Some(hero_name.clone()),
        MissionLogPayload::Ability { attacker, .. } => Some(attacker.clone()),
        MissionLogPayload::Death { name, is_enemy } => {
            if !*is_enemy {
                Some(name.clone())
            } else {
                None
            }
        }
        _ => None,
    }
}
