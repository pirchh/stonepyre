use bevy::prelude::*;

use stonepyre_engine::plugins::world::TargetMarker;
use stonepyre_world::tile_to_world_center;

use super::status::GameNetStatus;

/// Keep the green target marker tied to the selected server-side intent while
/// networked movement follows short server-snapshot paths.
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

    let target_tile = status.action_marker_target.or(status.last_move_sent);
    let Some(target_tile) = target_tile else {
        *marker_vis = Visibility::Hidden;
        return;
    };

    let arrived_server = status.server_tile == Some(target_tile);
    let arrived_local = status.local_tile == Some(target_tile) || status.drift_tiles == Some(0);
    let action_still_active = status.action_marker_target == Some(target_tile)
        && status.server_action.is_some();

    if arrived_server && arrived_local && !action_still_active {
        *marker_vis = Visibility::Hidden;
        return;
    }

    let center = tile_to_world_center(target_tile);
    marker_xform.translation.x = center.x;
    marker_xform.translation.y = center.y;
    *marker_vis = Visibility::Visible;
}
