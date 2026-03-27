use tokio::sync::broadcast;
use crate::game::protocol::ServerMsg;

/// Broadcast bus for server -> clients.
#[derive(Clone)]
pub struct GameHub {
    tx: broadcast::Sender<ServerMsg>,
}

impl GameHub {
    pub fn new() -> Self {
        let (tx, _rx) = broadcast::channel(1024);
        Self { tx }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<ServerMsg> {
        self.tx.subscribe()
    }

    pub fn broadcast(&self, msg: ServerMsg) {
        let _ = self.tx.send(msg);
    }
}