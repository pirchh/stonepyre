use serde::{Deserialize, Serialize};
use crate::keyword::Keyword;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TargetingRule {
    AnyUnit,
    FriendlyUnit,
    EnemyUnit,
    AnyCharacter,   // unit or hero, either side
    FriendlyHero,
    EnemyHero,
    AllEnemyUnits,   // AOE — no target selection required
    AllFriendlyUnits, // AOE buff — no target selection required
    NoTarget,
}

impl TargetingRule {
    pub fn requires_target(&self) -> bool {
        !matches!(
            self,
            TargetingRule::AllEnemyUnits
                | TargetingRule::AllFriendlyUnits
                | TargetingRule::NoTarget
        )
    }
}

/// Structured effect used by champion powers and optionally by spells.
/// Spell and relic rules are also described by `rules_text` for display.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum EffectDefinition {
    DealDamage { amount: u16 },
    Heal { amount: u16 },
    /// Deal `damage` and restore `healing` to the acting player's hero.
    Drain { damage: u16, healing: u16 },
    BuffUnit { attack_delta: i16, health_delta: i16 },
    GiveKeyword { keyword: Keyword },
    GiveShielded,
    DrawCards { count: u8 },
    DiscardCards { count: u8 },
    /// Look at the top `depth` cards; player may put any on the bottom.
    FilterTopCard { depth: u8 },
    SummonToken { token: TokenDefinition },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TokenDefinition {
    pub name: String,
    pub attack: i16,
    pub health: i16,
    #[serde(default)]
    pub keywords: Vec<Keyword>,
}

/// Structured effect attached to a spell card for engine execution.
/// Cards without this field require manual/future implementation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpellEffect {
    pub targeting: TargetingRule,
    pub effect: EffectDefinition,
}
