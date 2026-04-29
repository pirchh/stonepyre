use bevy::prelude::*;
use std::collections::HashSet;
use uuid::Uuid;

use stonepyre_engine::plugins::animation::{HumanoidAnim, HumanoidFrames};
use stonepyre_engine::plugins::world::{
    Facing, ARRIVE_EPS, FOOT_OFFSET_Y, MOVE_TILES_PER_SEC, PLAYER_SCALE, WALK_FRAMES,
};
use stonepyre_world::{tile_to_world_center, world_to_tile, TilePos, TILE_SIZE};

use super::status::GameNetStatus;

#[derive(Component, Clone, Copy, Debug)]
pub struct RemoteNetPlayer {
    pub player_id: Uuid,
    pub character_id: Uuid,
    pub target_tile: TilePos,
    pub facing: Facing,
}

/// Spawn/update/despawn remote players based on the authoritative server snapshot.
///
/// Snapshot updates choose each remote player's target tile. A separate animation system
/// smooths visible remotes toward those targets so they read as movement instead of teleporting.
pub fn sync_remote_players_from_snapshots(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
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

        let center = tile_to_world_center(p.tile);
        let target_translation = Vec3::new(center.x, center.y + FOOT_OFFSET_Y, 9.5);

        let mut found = false;
        for (_, mut remote, xform) in remotes.iter_mut() {
            if remote.player_id == p.player_id {
                let current_tile = world_to_tile(Vec2::new(
                    xform.translation.x,
                    xform.translation.y - FOOT_OFFSET_Y,
                ));

                if remote.target_tile != p.tile {
                    remote.facing = facing_toward(current_tile, p.tile, remote.facing);
                    remote.target_tile = p.tile;
                }

                found = true;
                break;
            }
        }

        if !found {
            let image: Handle<Image> = asset_server
                .load("characters/humanoid/base_greyscale/idle/south/south_idle.png");

            commands.spawn((
                Sprite {
                    image,
                    ..default()
                },
                Transform::from_translation(target_translation)
                    .with_scale(Vec3::splat(PLAYER_SCALE)),
                RemoteNetPlayer {
                    player_id: p.player_id,
                    character_id: p.character_id,
                    target_tile: p.tile,
                    facing: Facing::South,
                },
                HumanoidAnim::new(),
                Name::new(format!("remote_player_{}", p.player_id)),
            ));
        }
    }

    for (entity, remote, _) in remotes.iter_mut() {
        if !expected.contains(&remote.player_id) {
            if let Ok(mut ec) = commands.get_entity(entity) {
                ec.despawn();
            }
        }
    }

    status.remote_player_count = expected.len();
}

/// Smoothly move and animate remote players toward their latest authoritative server tile.
///
/// Remote players are display-only mirrors. The server remains authoritative, and this system
/// gives snapshots enough visual continuity to read as movement instead of teleporting.
pub fn animate_remote_players_from_snapshots(
    time: Res<Time>,
    frames: Option<Res<HumanoidFrames>>,
    mut remotes: Query<(
        &mut Transform,
        &mut Sprite,
        &mut RemoteNetPlayer,
        &mut HumanoidAnim,
    )>,
) {
    let Some(frames) = frames else { return; };

    for (mut xform, mut sprite, mut remote, mut anim) in remotes.iter_mut() {
        let target_center = tile_to_world_center(remote.target_tile);
        let target_feet = Vec2::new(target_center.x, target_center.y);
        let cur_feet = Vec2::new(xform.translation.x, xform.translation.y - FOOT_OFFSET_Y);
        let to = target_feet - cur_feet;
        let dist = to.length();

        if dist > ARRIVE_EPS {
            let current_tile = world_to_tile(cur_feet);
            remote.facing = facing_toward(current_tile, remote.target_tile, remote.facing);

            let speed = TILE_SIZE * MOVE_TILES_PER_SEC;
            let dir = to / dist.max(0.0001);
            let step = speed * time.delta_secs();
            let delta = dir * step.min(dist);

            xform.translation.x += delta.x;
            xform.translation.y += delta.y;

            anim.frame_timer.tick(time.delta());
            if anim.frame_timer.just_finished() {
                anim.frame_idx = (anim.frame_idx + 1) % WALK_FRAMES.len().max(1);
            }

            if !anim.last_walking
                || anim.last_facing != remote.facing
                || anim.last_frame_idx != anim.frame_idx
            {
                sprite.image = frames.walk_for(remote.facing, anim.frame_idx);
                anim.last_walking = true;
                anim.last_facing = remote.facing;
                anim.last_frame_idx = anim.frame_idx;
                anim.last_clip = None;
            }
        } else {
            xform.translation.x = target_feet.x;
            xform.translation.y = target_feet.y + FOOT_OFFSET_Y;

            if anim.last_walking || anim.last_facing != remote.facing {
                sprite.image = frames.idle_for(remote.facing);
                anim.last_walking = false;
                anim.last_facing = remote.facing;
                anim.last_frame_idx = usize::MAX;
                anim.frame_idx = 0;
                anim.last_clip = None;
            }
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
        if dx > 0 {
            Facing::East
        } else {
            Facing::West
        }
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
