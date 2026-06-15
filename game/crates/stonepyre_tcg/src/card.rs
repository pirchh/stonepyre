use serde::{Deserialize, Serialize};
use crate::champion::ChampionPowerDefinition;
use crate::color::CardColor;
use crate::effects::SpellEffect;
use crate::keyword::Keyword;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CardType {
    Unit,
    Spell,
    Relic,
    Champion,
}

/// How hard the card is to obtain — affects set design, not gameplay stats.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CardRarity {
    Common,
    Uncommon,
    Rare,
    Epic,
    Champion,
}

/// The visual treatment of a specific copy of a card.
/// Purely cosmetic — stats and abilities are always sourced from CardDefinition.
/// Normal → Rare → Mythic → Omnipotent in ascending visual prestige.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum CardForm {
    #[default]
    Normal,
    Rare,
    Mythic,
    Omnipotent,
}

impl CardForm {
    pub fn display_name(&self) -> &'static str {
        match self {
            CardForm::Normal => "Normal",
            CardForm::Rare => "Rare",
            CardForm::Mythic => "Mythic",
            CardForm::Omnipotent => "Omnipotent",
        }
    }

    /// CSS class for visual treatment overlays.
    pub fn css_class(&self) -> &'static str {
        match self {
            CardForm::Normal => "form-normal",
            CardForm::Rare => "form-rare",
            CardForm::Mythic => "form-mythic",
            CardForm::Omnipotent => "form-omnipotent",
        }
    }
}

/// A card in a player's collection: which card, which printing, how many.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectionEntry {
    pub card_def_id: String,
    #[serde(default)]
    pub form: CardForm,
    pub quantity: u32,
}

impl CollectionEntry {
    pub fn normal(card_def_id: impl Into<String>, quantity: u32) -> Self {
        Self { card_def_id: card_def_id.into(), form: CardForm::Normal, quantity }
    }

    pub fn with_form(card_def_id: impl Into<String>, form: CardForm, quantity: u32) -> Self {
        Self { card_def_id: card_def_id.into(), form, quantity }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CardDefinition {
    pub id: String,
    pub name: String,
    pub color: CardColor,
    #[serde(rename = "type")]
    pub card_type: CardType,
    pub cost: u8,
    /// Attack value. Required for Unit; optional but expected for Champion.
    pub attack: Option<i16>,
    /// Health value. Required for Unit; optional but expected for Champion.
    pub health: Option<i16>,
    /// Starting durability. Required for Relic.
    pub durability: Option<u8>,
    #[serde(default)]
    pub keywords: Vec<Keyword>,
    pub rules_text: String,
    #[serde(default)]
    pub tribes: Vec<String>,
    pub art_id: String,
    pub art_background_id: String,
    pub frame_id: String,
    pub rarity: CardRarity,
    pub set_id: String,
    /// Structured power for Champion cards.
    #[serde(default)]
    pub champion_power: Option<ChampionPowerDefinition>,
    /// Structured effect for Spell cards. Absent = effect handled by rules_text only (future implementation).
    #[serde(default)]
    pub spell_effect: Option<SpellEffect>,
    /// Colors permitted in a deck built around this champion. None = no color restriction.
    #[serde(default)]
    pub allowed_deck_colors: Option<Vec<CardColor>>,
}

impl CardDefinition {
    pub fn is_champion(&self) -> bool { self.card_type == CardType::Champion }
    pub fn is_unit(&self) -> bool { self.card_type == CardType::Unit }
    pub fn is_spell(&self) -> bool { self.card_type == CardType::Spell }
    pub fn is_relic(&self) -> bool { self.card_type == CardType::Relic }

    pub fn css_type_class(&self) -> &'static str {
        match self.card_type {
            CardType::Unit => "card-unit",
            CardType::Spell => "card-spell",
            CardType::Relic => "card-relic",
            CardType::Champion => "card-champion",
        }
    }
}
