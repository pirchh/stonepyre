use bevy::prelude::*;

use stonepyre_engine::plugins::world::{
    player_feet_world, Player, TilePath, FOOT_OFFSET_Y,
};
use stonepyre_world::{tile_to_world_center, world_to_tile};

use super::status::GameNetStatus;

/// Track the visible player tile, compare it to the authoritative server tile,
/// and only hard-correct if the local player drifts too far away.
///
/// The client still predicts immediately by using the existing local TilePath. This system is
/// reconciliation, not the only movement driver.
pub fn reconcile_local_player_to_server(
    mut status: ResMut<GameNetStatus>,
    mut player_q: Query<(&mut Transform, &mut TilePath), With<Player>>,
) {
    let Ok((mut xform, mut path)) = player_q.single_mut() else {
        status.local_tile = None;
        status.drift_tiles = None;
        return;
    };

    let local_tile = world_to_tile(player_feet_world(&xform));
    status.local_tile = Some(local_tile);

    let Some(server_tile) = status.server_tile else {
        status.drift_tiles = None;
        return;
    };

    let drift = (local_tile.x - server_tile.x).abs() + (local_tile.y - server_tile.y).abs();
    status.drift_tiles = Some(drift);

    // Small differences are normal while prediction and snapshots are in flight.
    // If we drift too far, resync locally to the authoritative tile.
    if drift >= 3 {
        let center = tile_to_world_center(server_tile);
        xform.translation.x = center.x;
        xform.translation.y = center.y + FOOT_OFFSET_Y;
        path.tiles.clear();
        status.correction_count += 1;
        warn!(
            "game net corrected local player to server tile {},{} drift={}",
            server_tile.x, server_tile.y, drift
        );
    }
}
