use bevy::prelude::*;
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SkillId {
    Woodcutting,
    // Mining,
    // Fishing,
    // Combat,
}

#[derive(Component, Default)]
pub struct Skills {
    xp: HashMap<SkillId, u32>,
}

impl Skills {
    pub fn xp(&self, skill: SkillId) -> u32 {
        *self.xp.get(&skill).unwrap_or(&0)
    }

    pub fn level(&self, skill: SkillId) -> u32 {
        // v1: dumb linear curve. We'll swap to proper curve later.
        // Level 1 at 0 xp, then +100 xp per level.
        self.xp(skill) / 100 + 1
    }

    pub fn add_xp(&mut self, skill: SkillId, amount: u32) {
        let cur = self.xp(skill);
        self.xp.insert(skill, cur.saturating_add(amount));
    }
}

#[derive(Message, Clone, Copy, Debug)]
pub struct GainXpMsg {
    pub entity: Entity,
    pub skill: SkillId,
    pub amount: u32,
}

pub fn apply_xp_system(
    mut reader: MessageReader<GainXpMsg>,
    mut q: Query<&mut Skills>,
) {
    for ev in reader.read() {
        if let Ok(mut skills) = q.get_mut(ev.entity) {
            let before = skills.level(ev.skill);
            skills.add_xp(ev.skill, ev.amount);
            let after = skills.level(ev.skill);

            if after > before {
                info!(
                    "[xp] {:?} leveled {:?}: {} -> {}",
                    ev.entity, ev.skill, before, after
                );
            }
        }
    }
}