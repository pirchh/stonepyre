use std::collections::VecDeque;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use crate::deck::{MAX_MANA, STARTING_HEALTH};
use crate::keyword::Keyword;

pub type PlayerId = u8; // 0 or 1

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchState {
    /// Global turn counter. Increments every time end_turn is called.
    /// P0 acts on odd turns (1, 3, 5…), P1 on even turns (2, 4, 6…).
    pub turn: u32,
    pub active_player: PlayerId,
    pub players: [PlayerState; 2],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerState {
    pub id: PlayerId,
    pub health: i32,
    pub max_health: i32,
    pub mana: u8,
    pub max_mana: u8,
    /// How many turns this player has taken (starts at 0, becomes 1 on their first turn).
    pub turns_taken: u32,
    pub deck: VecDeque<String>,
    pub hand: Vec<String>,
    pub board: Vec<UnitInPlay>,
    pub relics: Vec<RelicInPlay>,
    pub discard: Vec<String>,
    pub champion_in_play: Option<ChampionInPlay>,
    pub champion_power_used: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnitInPlay {
    pub instance_id: Uuid,
    pub card_def_id: String,
    pub display_name: String,
    pub current_attack: i16,
    pub current_health: i16,
    pub max_health: i16,
    pub keywords: Vec<Keyword>,
    pub played_this_turn: bool,
    pub has_attacked: bool,
    pub is_shielded: bool,
}

impl UnitInPlay {
    pub fn can_attack(&self) -> bool {
        !self.has_attacked
            && (!self.played_this_turn || self.keywords.contains(&Keyword::Charge))
    }

    pub fn has_guard(&self) -> bool {
        self.keywords.contains(&Keyword::Guard)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelicInPlay {
    pub instance_id: Uuid,
    pub card_def_id: String,
    pub display_name: String,
    pub remaining_durability: u8,
}

impl RelicInPlay {
    pub fn is_exhausted(&self) -> bool {
        self.remaining_durability == 0
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChampionInPlay {
    pub instance_id: Uuid,
    pub card_def_id: String,
    pub display_name: String,
    pub current_attack: Option<i16>,
    pub current_health: Option<i16>,
    pub max_health: Option<i16>,
}

impl MatchState {
    pub fn new(deck_p0: VecDeque<String>, deck_p1: VecDeque<String>) -> Self {
        Self {
            turn: 1,
            active_player: 0,
            players: [PlayerState::new(0, deck_p0), PlayerState::new(1, deck_p1)],
        }
    }

    pub fn active_player_state(&self) -> &PlayerState {
        &self.players[self.active_player as usize]
    }

    pub fn active_player_state_mut(&mut self) -> &mut PlayerState {
        &mut self.players[self.active_player as usize]
    }

    pub fn opponent_state(&self) -> &PlayerState {
        &self.players[(self.active_player ^ 1) as usize]
    }

    pub fn opponent_state_mut(&mut self) -> &mut PlayerState {
        let idx = (self.active_player ^ 1) as usize;
        &mut self.players[idx]
    }

    /// Mana for the given player turn count.
    /// Each player independently gains +1 mana each time *their* turn begins.
    /// turns_taken 1 → 1 mana, 2 → 2 mana, … 10 → 10 mana (max).
    pub fn mana_for_player_turn(turns_taken: u32) -> u8 {
        turns_taken.min(MAX_MANA as u32) as u8
    }
}

impl PlayerState {
    pub fn new(id: PlayerId, deck: VecDeque<String>) -> Self {
        Self {
            id,
            health: STARTING_HEALTH,
            max_health: STARTING_HEALTH,
            mana: 0,
            max_mana: 0,
            turns_taken: 0,
            deck,
            hand: Vec::new(),
            board: Vec::new(),
            relics: Vec::new(),
            discard: Vec::new(),
            champion_in_play: None,
            champion_power_used: false,
        }
    }

    pub fn is_defeated(&self) -> bool {
        self.health <= 0
    }

    pub fn has_guard_unit(&self) -> bool {
        self.board.iter().any(|u| u.has_guard())
    }
}
