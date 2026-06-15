use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CardColor {
    Red,
    Green,
    Black,
    White,
    Blue,
    Purple,
    Neutral,
}

impl CardColor {
    pub fn stonepyre_name(&self) -> &'static str {
        match self {
            CardColor::Red => "Ember",
            CardColor::Green => "Wild",
            CardColor::Black => "Rot",
            CardColor::White => "Ward",
            CardColor::Blue => "Tide",
            CardColor::Purple => "Veil",
            CardColor::Neutral => "Stone",
        }
    }

    pub fn css_class(&self) -> &'static str {
        match self {
            CardColor::Red => "card-red",
            CardColor::Green => "card-green",
            CardColor::Black => "card-black",
            CardColor::White => "card-white",
            CardColor::Blue => "card-blue",
            CardColor::Purple => "card-purple",
            CardColor::Neutral => "card-neutral",
        }
    }
}
