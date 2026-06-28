use bevy::prelude::*;
use std::collections::HashSet;
use uuid::Uuid;

use stonepyre_engine::plugins::movement::facing_from_dir;
use stonepyre_engine::plugins::world::Facing;
use stonepyre_world::TILE_SIZE;

use super::status::GameNetStatus;

/// How far in the past we render remote players, in seconds. Snapshots arrive at
/// ~10Hz (100ms), so rendering ~1.5 intervals behind means we almost always have
/// two authoritative samples bracketing the render time and can interpolate
/// smoothly between them — tolerant of a late/jittery snapshot — at the cost of a
/// small, constant visual latency on other players.
const INTERP_DELAY: f32 = 0.15;

#[derive(Component, Clone, Copy, Debug)]
pub struct RemoteNetPlayer {
    pub player_id: Uuid,
    pub character_id: Uuid,
    pub facing: Facing,
    /// The two most recent authoritative samples — `(client-receive time, world
    /// pos)` — used for snapshot interpolation. `prev` is older, `curr` newest.
    /// We render at `now - INTERP_DELAY`, lerping between them, instead of chasing
    /// the latest at a fixed speed (which lags while moving and snaps on stop).
    prev: (f32, Vec2),
    curr: (f32, Vec2),
}

impl RemoteNetPlayer {
    fn new(player_id: Uuid, character_id: Uuid, now: f32, pos: Vec2) -> Self {
        Self {
            player_id,
            character_id,
            facing: Facing::South,
            prev: (now, pos),
            curr: (now, pos),
        }
    }

    /// Fold a fresh authoritative sample into the buffer, shifting the previous
    /// newest to `prev`. Updates facing from the travel direction.
    fn push_sample(&mut self, now: f32, pos: Vec2) {
        self.prev = self.curr;
        self.curr = (now, pos);
        let delta = self.curr.1 - self.prev.1;
        if delta.length_squared() > 1e-6 {
            self.facing = facing_from_dir(delta.normalize());
        }
    }

    /// Position at `render_time`, interpolated between the two samples. Clamps to
    /// the newest when render_time is past it (snapshot starvation → hold, don't
    /// extrapolate into walls), and to the oldest before it.
    fn interpolated(&self, render_time: f32) -> Vec2 {
        let (t0, p0) = self.prev;
        let (t1, p1) = self.curr;
        if t1 <= t0 {
            return p1; // only one sample so far
        }
        let t = ((render_time - t0) / (t1 - t0)).clamp(0.0, 1.0);
        p0.lerp(p1, t)
    }
}

/// Spawn/update/despawn remote players from the authoritative server snapshot.
/// A new authoritative sample is folded into each remote's interpolation buffer
/// only when a fresh snapshot actually arrives (server_tick advances), not every
/// frame — otherwise the two samples would collapse to the same timestamp.
pub fn sync_remote_players_from_snapshots(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    time: Res<Time>,
    mut status: ResMut<GameNetStatus>,
    mut remotes: Query<(Entity, &mut RemoteNetPlayer)>,
    mut last_tick: Local<Option<u64>>,
) {
    let Some(local_player_id) = status.player_id else {
        status.remote_player_count = 0;
        return;
    };

    let now = time.elapsed_secs();
    let new_snapshot = status.server_tick != *last_tick;
    *last_tick = status.server_tick;

    let mut expected: HashSet<Uuid> = HashSet::new();

    for p in status.latest_players.iter() {
        if p.player_id == local_player_id {
            continue;
        }

        expected.insert(p.player_id);
        let pos = Vec2::new(p.pos_x, p.pos_z);

        let mut found = false;
        for (_, mut remote) in remotes.iter_mut() {
            if remote.player_id == p.player_id {
                if new_snapshot {
                    remote.push_sample(now, pos);
                }
                found = true;
                break;
            }
        }

        if !found {
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
                Transform::from_translation(Vec3::new(pos.x, spawn_y, pos.y)),
                RemoteNetPlayer::new(p.player_id, p.character_id, now, pos),
                Name::new(format!("remote_player_{}", p.player_id)),
            ));
        }
    }

    // Despawn players that have left the snapshot.
    for (entity, remote) in remotes.iter_mut() {
        if !expected.contains(&remote.player_id) {
            if let Ok(mut ec) = commands.get_entity(entity) {
                ec.despawn();
            }
        }
    }

    status.remote_player_count = expected.len();
}

/// Render remote players at `now - INTERP_DELAY`, interpolating between their two
/// latest authoritative samples. Smooth between snapshots and on stop, with no
/// chase-speed lag.
pub fn animate_remote_players_from_snapshots(
    time: Res<Time>,
    mut remotes: Query<(&mut Transform, &RemoteNetPlayer)>,
) {
    let render_time = time.elapsed_secs() - INTERP_DELAY;
    for (mut xform, remote) in remotes.iter_mut() {
        let pos = remote.interpolated(render_time);
        xform.translation.x = pos.x;
        xform.translation.z = pos.y;
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
