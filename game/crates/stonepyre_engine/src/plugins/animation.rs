use bevy::prelude::*;
use std::collections::HashMap;

use crate::plugins::movement::StepTo;
use crate::plugins::skills::{AnimClip, RequestedAnim};
use crate::plugins::world::{Facing, Player, TilePath, BASE_DIR, WALK_FRAMES, WALK_FPS};

/// Insert this when you want to HARD snap the player back to idle *this frame*
/// (because Commands-based removal of RequestedAnim applies end-of-frame).
#[derive(Component)]
pub struct ForceIdleOnce;

#[derive(Resource, Clone)]
pub struct HumanoidFrames {
    pub idle: HashMap<Facing, Handle<Image>>,
    pub walk: HashMap<Facing, Vec<Handle<Image>>>,
    pub woodcutting: HashMap<Facing, Vec<Handle<Image>>>,
}

impl HumanoidFrames {
    pub fn load(asset_server: &AssetServer) -> Self {
        fn dir_name(f: Facing) -> &'static str {
            match f {
                Facing::North => "north",
                Facing::East => "east",
                Facing::South => "south",
                Facing::West => "west",
            }
        }

        let mut idle = HashMap::new();
        let mut walk = HashMap::new();
        let mut woodcutting = HashMap::new();

        for facing in [Facing::North, Facing::East, Facing::South, Facing::West] {
            let dir = dir_name(facing);

            // idle
            let idle_path = format!("{}/idle/{dir}/{dir}_idle.png", BASE_DIR);
            idle.insert(facing, asset_server.load(idle_path));

            // walk
            let mut walk_frames = Vec::new();
            for frame_num in WALK_FRAMES {
                let p = format!("{}/walk/{dir}/{dir}_walk_{:02}.png", BASE_DIR, frame_num);
                walk_frames.push(asset_server.load(p));
            }
            walk.insert(facing, walk_frames);

            // woodcutting (uses WALK_FRAMES length for now)
            let mut wc_frames = Vec::new();
            for frame_num in WALK_FRAMES {
                let p = format!(
                    "{}/skills/woodcutting/{dir}/{dir}_woodcutting_{:02}.png",
                    BASE_DIR, frame_num
                );
                wc_frames.push(asset_server.load(p));
            }
            woodcutting.insert(facing, wc_frames);
        }

        Self {
            idle,
            walk,
            woodcutting,
        }
    }

    pub fn idle_for(&self, facing: Facing) -> Handle<Image> {
        self.idle
            .get(&facing)
            .cloned()
            .unwrap_or_else(|| self.idle[&Facing::South].clone())
    }

    pub fn walk_for(&self, facing: Facing, idx: usize) -> Handle<Image> {
        let frames = self
            .walk
            .get(&facing)
            .or_else(|| self.walk.get(&Facing::South))
            .expect("HumanoidFrames missing walk set");
        let i = idx.min(frames.len().saturating_sub(1));
        frames[i].clone()
    }

    pub fn clip_for(&self, clip: AnimClip, facing: Facing, idx: usize) -> Handle<Image> {
        let table = match clip {
            AnimClip::Woodcutting => &self.woodcutting,
        };

        let frames = table
            .get(&facing)
            .or_else(|| table.get(&Facing::South))
            .expect("HumanoidFrames missing clip set");

        let i = idx.min(frames.len().saturating_sub(1));
        frames[i].clone()
    }
}

#[derive(Component)]
pub struct HumanoidAnim {
    pub frame_timer: Timer,
    pub frame_idx: usize,

    // cache to avoid re-setting image unnecessarily
    pub last_walking: bool,
    pub last_facing: Facing,
    pub last_frame_idx: usize,
    pub last_clip: Option<AnimClip>,

    // optional “hold last frame a little longer” for nicer cadence
    pub hold_last: Option<Timer>,
}

impl HumanoidAnim {
    pub fn new() -> Self {
        Self {
            frame_timer: Timer::from_seconds(1.0 / WALK_FPS, TimerMode::Repeating),
            frame_idx: 0,
            last_walking: false,
            last_facing: Facing::South,
            last_frame_idx: usize::MAX,
            last_clip: None,
            hold_last: None,
        }
    }
}

// per-clip cadence knobs (we can expand later for fishing/mining/etc.)
fn clip_hold_last_secs(clip: AnimClip) -> f32 {
    match clip {
        AnimClip::Woodcutting => 0.04,
    }
}

pub fn animate_humanoid(
    time: Res<Time>,
    frames: Option<Res<HumanoidFrames>>,
    mut q: Query<(
        Entity,
        &mut Sprite,
        &Facing,
        &TilePath,
        Option<&StepTo>,
        &mut HumanoidAnim,
        Option<&mut RequestedAnim>,
        Option<&ForceIdleOnce>,
    ), With<Player>>,
    mut commands: Commands,
) {
    let Some(frames) = frames else { return; };
    let Ok((ent, mut sprite, facing, path, step_to, mut anim, req_opt, force_idle)) =
        q.single_mut()
    else {
        return;
    };

    // ------------------------------------------------------------
    // 0) Hard snap to idle this frame (prevents “stuck on last chop frame”)
    // ------------------------------------------------------------
    if force_idle.is_some() {
        // Components removed by Commands apply end-of-frame, but we can swap sprite NOW.
        commands.entity(ent).remove::<ForceIdleOnce>();
        commands.entity(ent).remove::<RequestedAnim>();

        anim.frame_idx = 0;
        anim.frame_timer.reset();
        anim.hold_last = None;
        anim.last_clip = None;
        anim.last_walking = false;
        anim.last_frame_idx = usize::MAX;
        anim.last_facing = *facing;

        sprite.image = frames.idle_for(*facing);
        return;
    }

    // walking if we have queued tiles OR we are currently stepping
    let walking = step_to.is_some() || !path.tiles.is_empty();

    // ------------------------------------------------------------
    // 1) Requested clip overrides idle/walk
    // ------------------------------------------------------------
    if let Some(mut req) = req_opt {

        // If the requested clip changed (or we just entered RequestedAnim), reset playback
        if anim.last_clip != Some(req.clip) {
            anim.frame_idx = 0;
            anim.frame_timer.reset();
            anim.hold_last = None;
            anim.last_frame_idx = usize::MAX; // force sprite update
        }
        
        // tick the request’s timer (loop/oneshot)
        req.mode.tick(time.delta());

        // tick frame timer (controls frame flipping)
        anim.frame_timer.tick(time.delta());

        let frames_len = WALK_FRAMES.len().max(1);
        let last_idx = frames_len - 1;

        // if we’re on last frame, hold briefly (per-clip)
        if anim.frame_idx == last_idx {
            let hold_secs = clip_hold_last_secs(req.clip);
            if hold_secs > 0.0 {
                if anim.hold_last.is_none() {
                    anim.hold_last = Some(Timer::from_seconds(hold_secs, TimerMode::Once));
                }
                if let Some(t) = anim.hold_last.as_mut() {
                    t.tick(time.delta());

                    // while holding, do NOT advance frames
                    if !t.just_finished() {
                        if anim.last_clip != Some(req.clip)
                            || anim.last_facing != *facing
                            || anim.last_frame_idx != anim.frame_idx
                        {
                            sprite.image = frames.clip_for(req.clip, *facing, anim.frame_idx);
                            anim.last_clip = Some(req.clip);
                            anim.last_facing = *facing;
                            anim.last_frame_idx = anim.frame_idx;
                            anim.last_walking = false;
                        }
                        return;
                    }
                }
                anim.hold_last = None;
            }
        }

        // advance on frame timer
        if anim.frame_timer.just_finished() {
            anim.frame_idx = (anim.frame_idx + 1) % frames_len;
        }

        // set sprite if needed
        if anim.last_clip != Some(req.clip)
            || anim.last_facing != *facing
            || anim.last_frame_idx != anim.frame_idx
        {
            sprite.image = frames.clip_for(req.clip, *facing, anim.frame_idx);
            anim.last_clip = Some(req.clip);
            anim.last_facing = *facing;
            anim.last_frame_idx = anim.frame_idx;
            anim.last_walking = false;
        }

        // If this was a oneshot and it finished, remove it and reset to clean state.
        if req.mode.is_one_shot() && req.mode.just_finished() {
            commands.entity(ent).remove::<RequestedAnim>();
            anim.frame_idx = 0;
            anim.last_frame_idx = usize::MAX;
            anim.last_clip = None;
            anim.hold_last = None;
        }

        return;
    }

    // ------------------------------------------------------------
    // 2) Normal idle/walk
    // ------------------------------------------------------------
    anim.last_clip = None;
    anim.hold_last = None;

    if walking {
        anim.frame_timer.tick(time.delta());
        if anim.frame_timer.just_finished() {
            anim.frame_idx = (anim.frame_idx + 1) % WALK_FRAMES.len().max(1);
        }

        if !anim.last_walking || anim.last_facing != *facing || anim.last_frame_idx != anim.frame_idx
        {
            sprite.image = frames.walk_for(*facing, anim.frame_idx);
            anim.last_walking = true;
            anim.last_facing = *facing;
            anim.last_frame_idx = anim.frame_idx;
        }
    } else {
        if anim.last_walking || anim.last_facing != *facing {
            sprite.image = frames.idle_for(*facing);
            anim.last_walking = false;
            anim.last_facing = *facing;
            anim.last_frame_idx = usize::MAX;
            anim.frame_idx = 0;
        }
    }
}