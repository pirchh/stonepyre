use bevy::prelude::*;

use stonepyre_engine::plugins::world::TargetMarker;
use stonepyre_world::tile_to_world_center;

use super::status::GameNetStatus;

/// Keep the green target marker tied to the selected WalkHere destination while
/// networked movement follows short server-snapshot paths.
///
/// The engine's default target marker uses TilePath::back(), which is correct for
/// offline/local pathing. In networked mode, TilePath is also used as a short
/// visual follow path toward the latest authoritative server tile, so using it for
/// the marker makes the marker hop across every tile the player crosses. This
/// system runs after the engine marker system and pins the marker to the last
/// MoveTo target instead.
pub fn sync_network_target_marker_from_last_move(
    status: Res<GameNetStatus>,
    mut marker_q: Query<(&mut Transform, &mut Visibility), With<TargetMarker>>,
) {
    let Ok((mut marker_xform, mut marker_vis)) = marker_q.single_mut() else {
        return;
    };

    if !status.connected {
        return;
    }

    let Some(target_tile) = status.last_move_sent else {
        *marker_vis = Visibility::Hidden;
        return;
    };

    let arrived_server = status.server_tile == Some(target_tile);
    let arrived_local = status.local_tile == Some(target_tile) || status.drift_tiles == Some(0);

    if arrived_server && arrived_local {
        *marker_vis = Visibility::Hidden;
        return;
    }

    let center = tile_to_world_center(target_tile);
    marker_xform.translation.x = center.x;
    marker_xform.translation.y = center.y;
    *marker_vis = Visibility::Visible;
}
