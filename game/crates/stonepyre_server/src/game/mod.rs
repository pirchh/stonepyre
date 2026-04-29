pub mod protocol;
pub mod sim;
pub mod hub;

use std::{collections::HashMap, sync::Arc};

use tokio::sync::RwLock;
use uuid::Uuid;

use self::{hub::GameHub, sim::GameSim};

#[derive(Clone)]
pub struct GameRuntime {
    pub hub: GameHub,
    pub sim: Arc<RwLock<GameSim>>,

    /// In-memory active world sessions.
    ///
    /// This is intentionally server-runtime only for now. It prevents one character
    /// from being represented by multiple active players in the same running game sim.
    pub sessions: Arc<RwLock<ActiveCharacterSessions>>,
}

impl GameRuntime {
    pub fn new(tick_hz: u32) -> Self {
        let hub = GameHub::new();
        let sim = Arc::new(RwLock::new(GameSim::new(tick_hz)));
        let sessions = Arc::new(RwLock::new(ActiveCharacterSessions::default()));

        Self {
            hub,
            sim,
            sessions,
        }
    }
}

#[derive(Default)]
pub struct ActiveCharacterSessions {
    character_to_player: HashMap<Uuid, Uuid>,
    player_to_character: HashMap<Uuid, Uuid>,
}

impl ActiveCharacterSessions {
    pub fn reserve_character(
        &mut self,
        player_id: Uuid,
        character_id: Uuid,
    ) -> Result<(), ActiveCharacterJoinError> {
        if let Some(existing_player_id) = self.character_to_player.get(&character_id).copied() {
            return Err(ActiveCharacterJoinError::CharacterAlreadyActive {
                character_id,
                existing_player_id,
            });
        }

        if let Some(existing_character_id) = self.player_to_character.get(&player_id).copied() {
            return Err(ActiveCharacterJoinError::PlayerAlreadyJoined {
                player_id,
                existing_character_id,
            });
        }

        self.character_to_player.insert(character_id, player_id);
        self.player_to_character.insert(player_id, character_id);

        Ok(())
    }

    pub fn release_player(&mut self, player_id: Uuid) -> Option<Uuid> {
        let character_id = self.player_to_character.remove(&player_id)?;
        self.character_to_player.remove(&character_id);
        Some(character_id)
    }

    pub fn player_for_character(&self, character_id: Uuid) -> Option<Uuid> {
        self.character_to_player.get(&character_id).copied()
    }
}

#[derive(Debug, Clone, Copy)]
pub enum ActiveCharacterJoinError {
    CharacterAlreadyActive {
        character_id: Uuid,
        existing_player_id: Uuid,
    },
    PlayerAlreadyJoined {
        player_id: Uuid,
        existing_character_id: Uuid,
    },
}
