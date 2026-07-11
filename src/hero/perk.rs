use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::localization::tr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Reflect)]
pub enum VeteranPerk {
    RescueSpecialist, // Survived 3 rescues given: +10% Max HP
    DeathDefier,      // Survived 3 near-deaths: +2 Defense
    Slayer,           // Defeated 20 enemies: +2 Attack
    DungeonVanguard,  // Completed 10 missions: +1 Speed
    TreasureHunter,   // Amassed 1000 lifetime gold: +1 Attack, +5% Max HP
}

impl VeteranPerk {
    pub fn name(&self) -> &'static str {
        match self {
            VeteranPerk::RescueSpecialist => tr("perk.rescue_specialist"),
            VeteranPerk::DeathDefier => tr("perk.death_defier"),
            VeteranPerk::Slayer => tr("perk.slayer"),
            VeteranPerk::DungeonVanguard => tr("perk.dungeon_vanguard"),
            VeteranPerk::TreasureHunter => tr("perk.treasure_hunter"),
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            VeteranPerk::RescueSpecialist => tr("perk.rescue_specialist.desc"),
            VeteranPerk::DeathDefier => tr("perk.death_defier.desc"),
            VeteranPerk::Slayer => tr("perk.slayer.desc"),
            VeteranPerk::DungeonVanguard => tr("perk.dungeon_vanguard.desc"),
            VeteranPerk::TreasureHunter => tr("perk.treasure_hunter.desc"),
        }
    }

    pub fn apply_bonuses(&self, hp: &mut i32, attack: &mut i32, defense: &mut i32, speed: &mut i32) {
        match self {
            VeteranPerk::RescueSpecialist => {
                *hp = (*hp as f32 * 1.10).floor() as i32;
            }
            VeteranPerk::DeathDefier => {
                *defense += 2;
            }
            VeteranPerk::Slayer => {
                *attack += 2;
            }
            VeteranPerk::DungeonVanguard => {
                *speed += 1;
            }
            VeteranPerk::TreasureHunter => {
                *attack += 1;
                *hp = (*hp as f32 * 1.05).floor() as i32;
            }
        }
    }
}

pub fn get_earned_perks(history: &crate::hero::history::HeroHistory) -> Vec<VeteranPerk> {
    let mut perks = Vec::new();
    if history.rescues_given >= 3 {
        perks.push(VeteranPerk::RescueSpecialist);
    }
    if history.near_deaths >= 3 {
        perks.push(VeteranPerk::DeathDefier);
    }
    if history.kills >= 20 {
        perks.push(VeteranPerk::Slayer);
    }
    if history.missions_run >= 10 {
        perks.push(VeteranPerk::DungeonVanguard);
    }
    if history.lifetime_gold >= 1000 {
        perks.push(VeteranPerk::TreasureHunter);
    }
    perks.truncate(3);
    perks
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hero::history::HeroHistory;

    #[test]
    fn test_perk_thresholds() {
        let mut history = HeroHistory::default();
        assert!(get_earned_perks(&history).is_empty());

        history.rescues_given = 3;
        assert_eq!(get_earned_perks(&history), vec![VeteranPerk::RescueSpecialist]);

        history.near_deaths = 3;
        assert_eq!(
            get_earned_perks(&history),
            vec![VeteranPerk::RescueSpecialist, VeteranPerk::DeathDefier]
        );

        history.kills = 20;
        assert_eq!(
            get_earned_perks(&history),
            vec![
                VeteranPerk::RescueSpecialist,
                VeteranPerk::DeathDefier,
                VeteranPerk::Slayer
            ]
        );
    }

    #[test]
    fn test_perk_cap() {
        let mut history = HeroHistory::default();
        history.rescues_given = 3;
        history.near_deaths = 3;
        history.kills = 20;
        history.missions_run = 10;
        history.lifetime_gold = 1000;

        let earned = get_earned_perks(&history);
        assert_eq!(earned.len(), 3);
        assert_eq!(
            earned,
            vec![
                VeteranPerk::RescueSpecialist,
                VeteranPerk::DeathDefier,
                VeteranPerk::Slayer
            ]
        );
    }

    #[test]
    fn test_perk_apply_bonuses() {
        let mut hp = 100;
        let mut attack = 10;
        let mut defense = 5;
        let mut speed = 3;

        VeteranPerk::RescueSpecialist.apply_bonuses(&mut hp, &mut attack, &mut defense, &mut speed);
        assert_eq!(hp, 110);
        assert_eq!(attack, 10);
        assert_eq!(defense, 5);
        assert_eq!(speed, 3);

        VeteranPerk::DeathDefier.apply_bonuses(&mut hp, &mut attack, &mut defense, &mut speed);
        assert_eq!(hp, 110);
        assert_eq!(attack, 10);
        assert_eq!(defense, 7);
        assert_eq!(speed, 3);

        VeteranPerk::Slayer.apply_bonuses(&mut hp, &mut attack, &mut defense, &mut speed);
        assert_eq!(hp, 110);
        assert_eq!(attack, 12);
        assert_eq!(defense, 7);
        assert_eq!(speed, 3);

        VeteranPerk::DungeonVanguard.apply_bonuses(&mut hp, &mut attack, &mut defense, &mut speed);
        assert_eq!(hp, 110);
        assert_eq!(attack, 12);
        assert_eq!(defense, 7);
        assert_eq!(speed, 4);

        VeteranPerk::TreasureHunter.apply_bonuses(&mut hp, &mut attack, &mut defense, &mut speed);
        assert_eq!(hp, 115); // 110 * 1.05 = 115.5 -> floor is 115
        assert_eq!(attack, 13);
        assert_eq!(defense, 7);
        assert_eq!(speed, 4);
    }
}
