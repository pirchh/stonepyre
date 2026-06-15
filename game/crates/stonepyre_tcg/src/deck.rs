use serde::{Deserialize, Serialize};

pub const DECK_SIZE: usize = 60;
pub const QUICK_DECK_SIZE: usize = 30;
pub const MAX_COPIES_PER_CARD: usize = 3;
pub const MAX_CHAMPIONS_PER_DECK: usize = 1;
pub const MAX_COPIES_PER_CHAMPION: usize = 1;
pub const MAX_MANA: u8 = 10;
pub const STARTING_HAND_SIZE: usize = 4;
pub const STARTING_HEALTH: i32 = 40;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeckDefinition {
    pub id: String,
    pub name: String,
    pub card_ids: Vec<String>,
}
