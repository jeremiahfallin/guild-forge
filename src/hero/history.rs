use bevy::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Component, Debug, Clone, Reflect, Serialize, Deserialize)]
#[reflect(Component)]
pub struct HeroHistory {
    pub missions_run: u32,
    pub kills: u32,
    pub near_deaths: u32,
    pub rescues_given: u32,
    pub rescues_received: u32,
    pub lifetime_gold: u32,
    pub timeline: Vec<String>,
}

impl Default for HeroHistory {
    fn default() -> Self {
        Self {
            missions_run: 0,
            kills: 0,
            near_deaths: 0,
            rescues_given: 0,
            rescues_received: 0,
            lifetime_gold: 0,
            timeline: vec!["Joined the guild".to_string()],
        }
    }
}

impl HeroHistory {
    pub fn add_timeline_entry(&mut self, entry: impl Into<String>) {
        self.timeline.push(entry.into());
    }
}
