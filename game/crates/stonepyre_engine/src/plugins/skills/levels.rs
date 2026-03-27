use bevy::prelude::*;
use std::collections::HashMap;

#[derive(Resource, Debug)]
pub struct SkillLevels {
    levels: HashMap<&'static str, u32>, // "woodcutting" -> level
}

impl Default for SkillLevels {
    fn default() -> Self {
        let mut levels = HashMap::new();
        levels.insert("woodcutting", 1);
        Self { levels }
    }
}

impl SkillLevels {
    pub fn level(&self, skill_id: &'static str) -> u32 {
        *self.levels.get(skill_id).unwrap_or(&1)
    }

    pub fn set_level(&mut self, skill_id: &'static str, level: u32) {
        self.levels.insert(skill_id, level.max(1));
    }
}