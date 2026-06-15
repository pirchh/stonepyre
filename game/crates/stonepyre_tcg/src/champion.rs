use serde::{Deserialize, Serialize};
use crate::effects::{EffectDefinition, TargetingRule};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChampionPowerDefinition {
    pub id: String,
    pub cost: u8,
    pub targeting: TargetingRule,
    pub effect: EffectDefinition,
    pub description: String,
}
