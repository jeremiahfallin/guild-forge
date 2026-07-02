use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use super::history::HeroHistory;

#[derive(Component, Debug, Clone, Reflect, Serialize, Deserialize, Default, PartialEq, Eq)]
#[reflect(Component)]
pub struct Epithet(pub Option<String>);

pub fn format_hero_name(name: &str, epithet: Option<&Epithet>) -> String {
    if let Some(Epithet(Some(ep))) = epithet {
        if ep == "Hundred-Mission" {
            format!("{} {}", ep, name)
        } else {
            format!("{} {}", name, ep)
        }
    } else {
        name.to_string()
    }
}

pub fn check_epithet(history: &HeroHistory) -> Option<String> {
    if history.near_deaths >= 3 {
        Some("the Undying".to_string())
    } else if history.near_deaths >= 2 || history.rescues_received >= 2 {
        Some("the Twice-Lost".to_string())
    } else if history.missions_run >= 100 {
        Some("Hundred-Mission".to_string())
    } else if history.kills >= 10 {
        Some("Slimebane".to_string())
    } else if history.rescues_given >= 1 {
        Some("the Savior".to_string())
    } else if history.missions_run >= 5 {
        Some("the Veteran".to_string())
    } else if history.lifetime_gold >= 1000 {
        Some("Goldfinder".to_string())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_hero_name() {
        assert_eq!(format_hero_name("Sera", None), "Sera");
        assert_eq!(format_hero_name("Sera", Some(&Epithet(None))), "Sera");
        assert_eq!(
            format_hero_name("Sera", Some(&Epithet(Some("the Twice-Lost".to_string())))),
            "Sera the Twice-Lost"
        );
        assert_eq!(
            format_hero_name("Hera", Some(&Epithet(Some("Hundred-Mission".to_string())))),
            "Hundred-Mission Hera"
        );
        assert_eq!(
            format_hero_name("Garr", Some(&Epithet(Some("Slimebane".to_string())))),
            "Garr Slimebane"
        );
    }

    #[test]
    fn test_check_epithet() {
        let mut hist = HeroHistory::default();
        assert_eq!(check_epithet(&hist), None);

        // Kills milestone
        hist.kills = 10;
        assert_eq!(check_epithet(&hist), Some("Slimebane".to_string()));

        // Rescue given milestone ( Savior )
        hist.rescues_given = 1;
        assert_eq!(check_epithet(&hist), Some("Slimebane".to_string())); // Slimebane takes priority over Savior

        // Near death milestone ( Twice-Lost )
        hist.near_deaths = 2;
        assert_eq!(check_epithet(&hist), Some("the Twice-Lost".to_string())); // Twice-Lost takes priority over Slimebane

        // Near death milestone ( Undying )
        hist.near_deaths = 3;
        assert_eq!(check_epithet(&hist), Some("the Undying".to_string()));
    }
}
