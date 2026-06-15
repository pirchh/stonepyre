use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Keyword {
    // White / Ward
    Guard,
    Shielded,
    Armor,
    Rally,
    // Red / Ember
    Charge,
    Burn,
    Fury,
    Kindle,
    Overheat,
    // Green / Wild
    Thorns,
    Rooted,
    Bloom,
    Regrow,
    Overgrowth,
    // Black / Rot
    LastBreath,
    Poison,
    Drain,
    Wither,
    // Blue / Tide
    Stun,
    Silence,
    Echo,
    Scry,
    // Purple / Veil
    Curse,
    Phantom,
    Hex,
    Possess,
    // Multi-color
    Leech,
}

impl Keyword {
    pub fn display_name(&self) -> &'static str {
        match self {
            Keyword::Guard => "Guard",
            Keyword::Shielded => "Shielded",
            Keyword::Armor => "Armor",
            Keyword::Rally => "Rally",
            Keyword::Charge => "Charge",
            Keyword::Burn => "Burn",
            Keyword::Fury => "Fury",
            Keyword::Kindle => "Kindle",
            Keyword::Overheat => "Overheat",
            Keyword::Thorns => "Thorns",
            Keyword::Rooted => "Rooted",
            Keyword::Bloom => "Bloom",
            Keyword::Regrow => "Regrow",
            Keyword::Overgrowth => "Overgrowth",
            Keyword::LastBreath => "Last Breath",
            Keyword::Poison => "Poison",
            Keyword::Drain => "Drain",
            Keyword::Wither => "Wither",
            Keyword::Stun => "Stun",
            Keyword::Silence => "Silence",
            Keyword::Echo => "Echo",
            Keyword::Scry => "Scry",
            Keyword::Curse => "Curse",
            Keyword::Phantom => "Phantom",
            Keyword::Hex => "Hex",
            Keyword::Possess => "Possess",
            Keyword::Leech => "Leech",
        }
    }
}
