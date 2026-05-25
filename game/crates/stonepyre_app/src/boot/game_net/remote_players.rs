use bevy::prelude::*;
use std::collections::HashSet;
use uuid::Uuid;

use stonepyre_engine::plugins::movement::facing_from_dir;
use stonepyre_engine::plugins::world::{Facing, ARRIVE_EPS, MOVE_SPEED};
use stonepyre_world::TILE_SIZE;

use super::status::GameNetStatus;

#[derive(Component, Clone, Copy, Debug)]
pub struct RemoteNetPlayer {
    pub player_id: Uuid,
    pub character_id: Uuid,
    /// Authoritative world-space position from the latest server snapshot.
    pub target_pos: Vec2,
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

        let target_pos = Vec2::new(p.pos_x, p.pos_z);
        let mut found = false;

        for (_, mut remote, _) in remotes.iter_mut() {
            if remote.player_id == p.player_id {
                remote.target_pos = target_pos;
                found = true;
                break;
            }
        }

        if !found {
            // Spawn position comes directly from the server's continuous position.
            let spawn_y = TILE_SIZE * 0.75;
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
                Transform::from_translation(Vec3::new(p.pos_x, spawn_y, p.pos_z)),
                RemoteNetPlayer {
                    player_id: p.player_id,
                    character_id: p.character_id,
                    target_pos,
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

/// Smoothly move remote players toward their latest authoritative server position.
pub fn animate_remote_players_from_snapshots(
    time: Res<Time>,
    mut remotes: Query<(&mut Transform, &mut RemoteNetPlayer)>,
) {
    for (mut xform, mut remote) in remotes.iter_mut() {
        let cur = Vec2::new(xform.translation.x, xform.translation.z);
        let to = remote.target_pos - cur;
        let dist = to.length();

        if dist > ARRIVE_EPS {
            let dir = to / dist.max(f32::EPSILON);
            remote.facing = facing_from_dir(dir);

            let step = MOVE_SPEED * time.delta_secs();
            let delta = dir * step.min(dist);
            xform.translation.x += delta.x;
            xform.translation.z += delta.y;
        } else {
            xform.translation.x = remote.target_pos.x;
            xform.translation.z = remote.target_pos.y;
        }
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
