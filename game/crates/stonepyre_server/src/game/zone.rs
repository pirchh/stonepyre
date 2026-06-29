//! World sharding (#9).
//!
//! The map is partitioned into **zones**, each an independent [`GameSim`] behind
//! its own lock with its own tick + snapshot loop, so zones simulate in parallel
//! with no cross-zone lock contention. Today there is exactly one zone
//! ([`DEFAULT_ZONE`]); this module is the *seam* so that adding zones is additive
//! rather than a sim-wide retrofit. Cross-zone handoff is a documented stub
//! ([`ZoneManager::handoff`]) — it can't be built or tested until a second zone
//! and boundary data exist.

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::RwLock;
use uuid::Uuid;

use super::sim::GameSim;

/// Identifier for a world shard.
pub type ZoneId = u32;

/// The single zone every player lives in until sharding is wired further.
pub const DEFAULT_ZONE: ZoneId = 0;

/// One world shard: an independent simulation behind its own lock, ticked and
/// snapshotted by its own loops.
pub struct Zone {
    pub id: ZoneId,
    pub sim: Arc<RwLock<GameSim>>,
}

/// Owns the world's zones and the player → zone routing table. Cheap to clone
/// (everything is `Arc`-backed), so it rides inside the cloneable `GameRuntime`.
#[derive(Clone)]
pub struct ZoneManager {
    zones: Arc<HashMap<ZoneId, Arc<Zone>>>,
    /// Which zone each connected player currently occupies. A player's commands
    /// route to this zone's sim; a handoff repoints it. Populated on join,
    /// cleared on disconnect.
    player_zone: Arc<RwLock<HashMap<Uuid, ZoneId>>>,
}

impl ZoneManager {
    /// Build a single-zone world (the only topology today).
    pub fn single(tick_hz: u32) -> Self {
        let mut zones = HashMap::new();
        zones.insert(
            DEFAULT_ZONE,
            Arc::new(Zone {
                id: DEFAULT_ZONE,
                sim: Arc::new(RwLock::new(GameSim::new(tick_hz))),
            }),
        );
        Self {
            zones: Arc::new(zones),
            player_zone: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// All zones — used to spawn one tick + snapshot loop per zone.
    pub fn all(&self) -> Vec<Arc<Zone>> {
        self.zones.values().cloned().collect()
    }

    /// The sim behind a given zone. Falls back to the default zone if the id is
    /// unknown, which can't happen with a static single-zone world.
    pub fn sim(&self, zone: ZoneId) -> Arc<RwLock<GameSim>> {
        self.zones
            .get(&zone)
            .or_else(|| self.zones.get(&DEFAULT_ZONE))
            .expect("default zone always exists")
            .sim
            .clone()
    }

    /// Record which zone a player joined into.
    pub async fn assign(&self, player_id: Uuid, zone: ZoneId) {
        self.player_zone.write().await.insert(player_id, zone);
    }

    /// The zone a player currently occupies (defaults until assigned). The seam
    /// for routing commands off the live table once handoff lands — today the ws
    /// handlers use the connection's fixed `zone_id`, so this isn't called yet.
    #[allow(dead_code)]
    pub async fn zone_of(&self, player_id: Uuid) -> ZoneId {
        self.player_zone
            .read()
            .await
            .get(&player_id)
            .copied()
            .unwrap_or(DEFAULT_ZONE)
    }

    /// Drop a player's zone assignment on disconnect.
    pub async fn forget(&self, player_id: Uuid) {
        self.player_zone.write().await.remove(&player_id);
    }

    /// Move a player from one zone to another: lift their entity out of the
    /// source sim and insert it into the destination, then repoint routing so
    /// their future commands and snapshots use the new zone.
    ///
    /// **Stub — intentionally unwired.** With a single zone there is no boundary
    /// to cross, so nothing calls this yet. It is the documented seam for
    /// cross-zone handoff (#9). Implementing it for real needs (a) a second zone,
    /// (b) boundary data to detect a crossing, and (c) transfer of the player's
    /// full sim entity (position, inventory, in-flight action) between the two
    /// sims. None of that can be written or tested meaningfully until real
    /// adjacent zones exist, so it is deferred rather than shipped dark. See the
    /// PR follow-ups.
    #[allow(dead_code)]
    pub async fn handoff(&self, _player_id: Uuid, _from: ZoneId, _to: ZoneId) {
        // TODO(#9 handoff): src_sim.remove_player(player) -> dst_sim.add_player
        // with transferred state; then player_zone.insert(player_id, to).
    }
}
