use bevy::prelude::*;
use std::sync::{
    mpsc::{self, Receiver, Sender},
    Mutex,
};
use uuid::Uuid;

use stonepyre_world::TilePos;

use super::protocol::NetPlayerSnapshot;

#[derive(Debug)]
pub enum GameNetEvent {
    Connecting { url: String, character_id: Uuid },
    Connected,
    Welcome {
        player_id: Uuid,
        character_id: Uuid,
        tick_hz: u32,
    },
    Snapshot {
        server_tick: u64,
        players: Vec<NetPlayerSnapshot>,
        server_tile: Option<TilePos>,
    },
    MoveSent { tile: TilePos },
    Error(String),
    Disconnected,
}

#[derive(Debug)]
pub enum GameNetCommand {
    MoveTo { tile: TilePos },
}

#[derive(Resource, Debug, Clone)]
pub struct GameNetStatus {
    pub connected: bool,
    pub connecting: bool,
    pub player_id: Option<Uuid>,
    pub character_id: Option<Uuid>,
    pub tick_hz: Option<u32>,
    pub server_tick: Option<u64>,
    pub snapshot_players: usize,
    pub latest_players: Vec<NetPlayerSnapshot>,
    pub server_tile: Option<TilePos>,
    pub local_tile: Option<TilePos>,
    pub drift_tiles: Option<i32>,
    pub last_move_sent: Option<TilePos>,
    pub last_error: Option<String>,
    pub correction_count: u64,
    pub remote_player_count: usize,
}

impl Default for GameNetStatus {
    fn default() -> Self {
        Self {
            connected: false,
            connecting: false,
            player_id: None,
            character_id: None,
            tick_hz: None,
            server_tick: None,
            snapshot_players: 0,
            latest_players: Vec::new(),
            server_tile: None,
            local_tile: None,
            drift_tiles: None,
            last_move_sent: None,
            last_error: None,
            correction_count: 0,
            remote_player_count: 0,
        }
    }
}

/// Runtime bridge for the server-side game websocket.
#[derive(Resource)]
pub struct GameNetRuntime {
    pub tx: Sender<GameNetEvent>,
    pub rx: Mutex<Receiver<GameNetEvent>>,
    pub(super) command_tx: Mutex<Option<Sender<GameNetCommand>>>,
}

impl Default for GameNetRuntime {
    fn default() -> Self {
        let (tx, rx) = mpsc::channel();
        Self {
            tx,
            rx: Mutex::new(rx),
            command_tx: Mutex::new(None),
        }
    }
}
