pub mod actions;
pub mod card;
pub mod champion;
pub mod color;
pub mod deck;
pub mod effects;
pub mod engine;
pub mod files;
pub mod keyword;
pub mod match_state;
pub mod registry;
pub mod render_schema;
pub mod validation;

pub use card::{CardDefinition, CardForm, CardRarity, CardType, CollectionEntry};
pub use champion::ChampionPowerDefinition;
pub use color::CardColor;
pub use deck::{
    DeckDefinition, DECK_SIZE, MAX_CHAMPIONS_PER_DECK, MAX_COPIES_PER_CARD,
    MAX_COPIES_PER_CHAMPION, MAX_MANA, QUICK_DECK_SIZE, STARTING_HAND_SIZE, STARTING_HEALTH,
};
pub use effects::{EffectDefinition, SpellEffect, TargetingRule, TokenDefinition};
pub use engine::{EngineError, GameEngine, GameEvent};
pub use keyword::Keyword;
pub use match_state::{ChampionInPlay, MatchState, PlayerId, PlayerState, RelicInPlay, UnitInPlay};
pub use registry::CardRegistry;
pub use validation::{validate_card, validate_deck, CardValidationError, DeckValidationError};
