use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use tokio::sync::{broadcast, mpsc};
use uuid::Uuid;

use crate::game::protocol::ServerMsg;

/// Server -> client message bus.
///
/// Two delivery paths share each connection's outbound queue (`out_tx`):
/// - [`broadcast`](Self::broadcast): fan-out to every connection — notices, action
///   states, harvest results, etc. (already player-filtered client-side or
///   genuinely global).
/// - [`send_to`](Self::send_to): direct delivery to one player's connection, used
///   for per-client interest-managed (AOI) world snapshots so each client only
///   receives the entities near it. Backed by a registry of `player_id -> out_tx`,
///   populated on JoinWorld and cleared on disconnect.
#[derive(Clone)]
pub struct GameHub {
    tx: broadcast::Sender<ServerMsg>,
    clients: Arc<RwLock<HashMap<Uuid, mpsc::UnboundedSender<ServerMsg>>>>,
}

impl GameHub {
    pub fn new() -> Self {
        let (tx, _rx) = broadcast::channel(1024);
        Self {
            tx,
            clients: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<ServerMsg> {
        self.tx.subscribe()
    }

    pub fn broadcast(&self, msg: ServerMsg) {
        let _ = self.tx.send(msg);
    }

    /// Register a joined player's per-connection outbound sender so the server can
    /// deliver per-client (AOI-filtered) messages directly, bypassing the
    /// broadcast fan-out.
    pub fn register_client(&self, player_id: Uuid, tx: mpsc::UnboundedSender<ServerMsg>) {
        self.clients.write().unwrap().insert(player_id, tx);
    }

    /// Drop a player's outbound sender on disconnect.
    pub fn unregister_client(&self, player_id: Uuid) {
        self.clients.write().unwrap().remove(&player_id);
    }

    /// Deliver a message to a single player's connection. No-op if that player is
    /// not currently connected.
    pub fn send_to(&self, player_id: Uuid, msg: ServerMsg) {
        if let Some(tx) = self.clients.read().unwrap().get(&player_id) {
            let _ = tx.send(msg);
        }
    }
}
