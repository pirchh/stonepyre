use bevy::prelude::*;
use std::collections::HashSet;
use uuid::Uuid;

use stonepyre_engine::plugins::world::{Facing, ARRIVE_EPS, MOVE_SPEED};
use stonepyre_world::{tile_to_world3d, world3d_to_tile, TilePos, TILE_SIZE};

use super::status::GameNetStatus;

#[derive(Component, Clone, Copy, Debug)]
pub struct RemoteNetPlayer {
    pub player_id: Uuid,
    pub character_id: Uuid,
    pub target_tile: TilePos,
    pub facing: Facing,
}

/// Spawn/update/despawn remote players based on the authoritative server snapshot.
pub fn sync_remote_players_from_snapshots(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut status: ResMut<GameNetStatus>,
    mut remotes: Query<(Entity, &mut RemoteNetPlayer, &Transform)>,
) {
    let Some(local_player_id) = status.player_id else {
        status.remote_player_count = 0;
        return;
    };

    let mut expected: HashSet<Uuid> = HashSet::new();

    for p in status.latest_players.iter() {
        if p.player_id == local_player_id {
            continue;
        }

        expected.insert(p.player_id);

        let mut found = false;
        for (_, mut remote, _) in remotes.iter_mut() {
            if remote.player_id == p.player_id {
                if remote.target_tile != p.tile {
                    remote.facing = Facing::South; // placeholder; real facing updated in animate system
                    remote.target_tile = p.tile;
                }
                found = true;
                break;
            }
        }

        if !found {
            let world_pos = tile_to_world3d(p.tile);
            // Spawn a simple teal box as a placeholder for remote player 3-D models.
            let mat = materials.add(StandardMaterial {
                base_color: Color::srgb(0.1, 0.7, 0.6),
                ..default()
            });
            commands.spawn((
                Mesh3d(meshes.add(Cuboid::new(
                    TILE_SIZE * 0.5,
                    TILE_SIZE * 1.5,
                    TILE_SIZE * 0.5,
                ))),
                MeshMaterial3d(mat),
                Transform::from_translation(
                    world_pos + Vec3::new(0.0, TILE_SIZE * 0.75, 0.0),
                ),
                RemoteNetPlayer {
                    player_id: p.player_id,
                    character_id: p.character_id,
                    target_tile: p.tile,
                    facing: Facing::South,
                },
                Name::new(format!("remote_player_{}", p.player_id)),
            ));
        }
    }

    // Despawn players that have left the snapshot.
    for (entity, remote, _) in remotes.iter_mut() {
        if !expected.contains(&remote.player_id) {
            if let Ok(mut ec) = commands.get_entity(entity) {
                ec.despawn();
            }
        }
    }

    status.remote_player_count = expected.len();
}

/// Smoothly move remote players toward their latest authoritative server tile.
pub fn animate_remote_players_from_snapshots(
    time: Res<Time>,
    mut remotes: Query<(&mut Transform, &mut RemoteNetPlayer)>,
) {
    for (mut xform, mut remote) in remotes.iter_mut() {
        let target = tile_to_world3d(remote.target_tile);
        let cur = Vec2::new(xform.translation.x, xform.translation.z);
        let tgt = Vec2::new(target.x, target.z);
        let to = tgt - cur;
        let dist = to.length();

        if dist > ARRIVE_EPS {
            let current_tile = world3d_to_tile(xform.translation);
            remote.facing = facing_toward(current_tile, remote.target_tile, remote.facing);

            let dir = to / dist.max(0.0001);
            let step = MOVE_SPEED * time.delta_secs();
            let delta = dir * step.min(dist);

            xform.translation.x += delta.x;
            xform.translation.z += delta.y;
        } else {
            xform.translation.x = target.x;
            xform.translation.z = target.z;
        }
    }
}

fn facing_toward(from: TilePos, to: TilePos, current: Facing) -> Facing {
    let dx = to.x - from.x;
    let dy = to.y - from.y;

    if dx == 0 && dy == 0 {
        return current;
    }

    if dx.abs() >= dy.abs() {
        if dx > 0 { Facing::East } else { Facing::West }
    } else if dy > 0 {
        Facing::North
    } else {
        Facing::South
    }
}

pub fn despawn_remote_players(
    mut commands: Commands,
    remotes: Query<Entity, With<RemoteNetPlayer>>,
) {
    for e in remotes.iter() {
        if let Ok(mut ec) = commands.get_entity(e) {
            ec.despawn();
        }
    }
}
