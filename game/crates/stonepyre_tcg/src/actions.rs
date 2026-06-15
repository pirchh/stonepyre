use serde::{Deserialize, Serialize};
use uuid::Uuid;
use crate::match_state::PlayerId;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Target {
    Unit(Uuid),
    Hero(PlayerId),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CardAction {
    DrawCard,
    PlayCard { card_id: String, target: Option<Target> },
    SummonUnit { card_id: String },
    CastSpell { card_id: String, target: Option<Target> },
    PlayRelic { card_id: String },
    PlayChampion { card_id: String },
    AttackUnit { attacker_id: Uuid, defender_id: Uuid },
    AttackHero { attacker_id: Uuid },
    UseChampionPower { target: Option<Target> },
    EndTurn,
}
