use bevy::prelude::*;

use stonepyre_world::{world_to_tile, TilePos, WorldGrid};

use crate::plugins::input::ClickMsg;
use crate::plugins::skills::{AnimClip, RequestedAnim};
use crate::plugins::ui::{ContextMenuState, MenuSelectMsg};
use crate::plugins::world::*;

// ------------------------------------------------------------
// Verbs / Targets
// ------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verb {
    WalkHere,
    TalkTo,
    ChopDown,
    Examine,
}

#[derive(Clone, Copy, Debug)]
pub enum Target {
    Tile(TilePos),
    Entity(Entity),
}

#[derive(Clone, Debug)]
pub struct InteractionCandidate {
    pub verb: Verb,
    pub target: Target,
    pub priority: i32,
    pub range: i32,
}

// ------------------------------------------------------------
// Intent → Plan → Execute → Resolve
// ------------------------------------------------------------

#[derive(Clone, Copy, Debug)]
pub struct Intent {
    pub verb: Verb,
    pub target: Target,
    pub range: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActionPhase {
    Moving,
    Impact,
    Complete,
    Failed,
}

#[derive(Component, Debug)]
pub struct CurrentAction {
    pub intent: Intent,
    pub phase: ActionPhase,
    pub impact_armed: bool,
    pub looping: bool,
    pub cooldown: Timer,

    // deterministic clip gating
    pub clip_started: bool,
}

#[derive(Message, Clone, Copy, Debug)]
pub struct IntentMsg {
    pub actor: Entity,
    pub intent: Intent,
}

#[derive(Message, Clone, Copy, Debug)]
pub struct ActionResolvedMsg {
    pub actor: Entity,
    pub intent: Intent,
}

// cadence knobs
const CHOP_PERIOD_SECS: f32 = 0.75; // time between chops

// ------------------------------------------------------------
// 1) Click -> candidates -> intent
// ------------------------------------------------------------

pub fn handle_clicks_build_candidates(
    mut click_reader: MessageReader<ClickMsg>,
    menu: Option<ResMut<ContextMenuState>>,
    mut intent_writer: MessageWriter<IntentMsg>,
    player_q: Query<Entity, With<Player>>,
    interactables: Query<(Entity, &GridPos, &InteractableKind)>,
) {
    let Some(mut menu) = menu else { return; };
    let Ok(player_ent) = player_q.single() else { return; };

    for ev in click_reader.read() {
        // If the context menu is open, do NOT treat left-click as a world click.
        if menu.open && ev.button == MouseButton::Left {
            continue;
        }

        let clicked_tile = ev.tile;

        let mut cands: Vec<InteractionCandidate> = vec![InteractionCandidate {
            verb: Verb::WalkHere,
            target: Target::Tile(clicked_tile),
            priority: 0,
            range: 0,
        }];

        for (ent, gp, kind) in interactables.iter() {
            if gp.0 != clicked_tile {
                continue;
            }
            match kind {
                InteractableKind::Tree => {
                    cands.push(InteractionCandidate {
                        verb: Verb::ChopDown,
                        target: Target::Entity(ent),
                        priority: 100,
                        range: 1,
                    });
                    cands.push(InteractionCandidate {
                        verb: Verb::Examine,
                        target: Target::Entity(ent),
                        priority: -10,
                        range: 1,
                    });
                }
                InteractableKind::Npc => {
                    cands.push(InteractionCandidate {
                        verb: Verb::TalkTo,
                        target: Target::Entity(ent),
                        priority: 90,
                        range: 1,
                    });
                    cands.push(InteractionCandidate {
                        verb: Verb::Examine,
                        target: Target::Entity(ent),
                        priority: -10,
                        range: 1,
                    });
                }
            }
        }

        cands.sort_by(|a, b| b.priority.cmp(&a.priority));

        match ev.button {
            MouseButton::Left => {
                menu.open = false;
                menu.dirty = true;

                if let Some(best) = cands.first().cloned() {
                    intent_writer.write(IntentMsg {
                        actor: player_ent,
                        intent: Intent {
                            verb: best.verb,
                            target: best.target,
                            range: best.range,
                        },
                    });
                }
            }
            MouseButton::Right => {
                menu.open = true;
                menu.screen_pos = ev.cursor_screen;
                menu.candidates = cands;
                menu.dirty = true;
            }
            _ => {}
        }
    }
}

pub fn handle_menu_selection_emit_intent(
    mut sel_reader: MessageReader<MenuSelectMsg>,
    menu: Option<ResMut<ContextMenuState>>,
    mut intent_writer: MessageWriter<IntentMsg>,
    player_q: Query<Entity, With<Player>>,
) {
    let Some(mut menu) = menu else { return; };
    let Ok(player_ent) = player_q.single() else { return; };

    for ev in sel_reader.read() {
        let Some(chosen) = menu.candidates.get(ev.idx).cloned() else { continue; };

        intent_writer.write(IntentMsg {
            actor: player_ent,
            intent: Intent {
                verb: chosen.verb,
                target: chosen.target,
                range: chosen.range,
            },
        });

        menu.open = false;
        menu.dirty = true;
    }
}

// ------------------------------------------------------------
// 2) Plan (Intent -> TilePath + CurrentAction)
// ------------------------------------------------------------

pub fn plan_intents_to_actions(
    mut intents: MessageReader<IntentMsg>,
    world: Option<Res<WorldGrid>>,
    interactables: Query<(Entity, &GridPos, &InteractableKind)>,
    mut player_q: Query<(Entity, &Transform, &mut TilePath, &mut Facing), With<Player>>,
    mut commands: Commands,
) {
    let Some(world) = world else { return; };

    let Ok((player_ent, player_xform, mut path, mut facing)) = player_q.single_mut() else {
        return;
    };
    let start_tile = world_to_tile(player_feet_world(player_xform));

    for ev in intents.read() {
        if ev.actor != player_ent {
            continue;
        }

        let intent = ev.intent;

        let target_tile = match intent.target {
            Target::Tile(t) => t,
            Target::Entity(e) => {
                let Ok((_, gp, _)) = interactables.get(e) else {
                    commands.entity(player_ent).insert(CurrentAction {
                        intent,
                        phase: ActionPhase::Failed,
                        impact_armed: false,
                        looping: false,
                        cooldown: Timer::from_seconds(0.25, TimerMode::Once),
                        clip_started: false,
                    });
                    commands.entity(player_ent).remove::<RequestedAnim>();
                    continue;
                };
                gp.0
            }
        };

        match intent.verb {
            Verb::WalkHere => {
                let goal_tile = if world.is_blocked(target_tile) {
                    pick_best_adjacent_goal_unblocked(&world, start_tile, target_tile, 1)
                        .unwrap_or(start_tile)
                } else {
                    target_tile
                };

                path.tiles = world.find_path_bfs(start_tile, goal_tile);

                if let Some(first) = path.tiles.front().copied() {
                    *facing = facing_from_step(start_tile, first);
                } else if goal_tile != start_tile {
                    *facing = facing_toward(start_tile, goal_tile, *facing);
                }

                commands.entity(player_ent).remove::<CurrentAction>();
                commands.entity(player_ent).remove::<RequestedAnim>();
            }

            Verb::TalkTo | Verb::ChopDown | Verb::Examine => {
                let range = intent.range.max(1);
                let Some(goal_tile) =
                    pick_best_adjacent_goal_unblocked(&world, start_tile, target_tile, range)
                else {
                    commands.entity(player_ent).insert(CurrentAction {
                        intent,
                        phase: ActionPhase::Failed,
                        impact_armed: false,
                        looping: false,
                        cooldown: Timer::from_seconds(0.25, TimerMode::Once),
                        clip_started: false,
                    });
                    commands.entity(player_ent).remove::<RequestedAnim>();
                    continue;
                };

                path.tiles = world.find_path_bfs(start_tile, goal_tile);

                if let Some(first) = path.tiles.front().copied() {
                    *facing = facing_from_step(start_tile, first);
                } else if goal_tile != start_tile {
                    *facing = facing_toward(start_tile, goal_tile, *facing);
                }

                let looping = intent.verb == Verb::ChopDown;

                // cadence timer: ready immediately for first impact
                let mut cooldown = Timer::from_seconds(
                    if looping { CHOP_PERIOD_SECS } else { 0.05 },
                    TimerMode::Once,
                );
                cooldown.set_elapsed(cooldown.duration());

                commands.entity(player_ent).insert(CurrentAction {
                    intent,
                    phase: ActionPhase::Moving,
                    impact_armed: false,
                    looping,
                    cooldown,
                    clip_started: false,
                });

                commands.entity(player_ent).remove::<RequestedAnim>();
            }
        }
    }
}

// ------------------------------------------------------------
// 3) Execute (Moving -> Impact once movement done + in range + cooldown)
// ------------------------------------------------------------

pub fn advance_action_to_impact_when_ready(
    time: Res<Time>,
    mut player_q: Query<(&Transform, &TilePath, &mut Facing, &mut CurrentAction), With<Player>>,
    interactables: Query<(Entity, &GridPos, &InteractableKind)>,
    harvest_q: Query<&crate::plugins::skills::HarvestNode>,
) {
    let Ok((player_xform, path, mut facing, mut action)) = player_q.single_mut() else { return; };

    if action.phase != ActionPhase::Moving {
        return;
    }
    if !path.tiles.is_empty() {
        return;
    }

    action.cooldown.tick(time.delta());
    if !timer_done(&action.cooldown) {
        return;
    }

    let player_tile = world_to_tile(player_feet_world(player_xform));

    let target_tile = match action.intent.target {
        Target::Tile(t) => t,
        Target::Entity(e) => {
            let Ok((_, gp, _)) = interactables.get(e) else {
                action.phase = ActionPhase::Failed;
                return;
            };

            // Gate chopdown by depletion
            if action.intent.verb == Verb::ChopDown {
                if let Ok(node) = harvest_q.get(e) {
                    if node.is_depleted() {
                        info!("[interaction] yo thats depleted.");
                        action.phase = ActionPhase::Failed;
                        return;
                    }
                }
            }

            gp.0
        }
    };

    let dx = (player_tile.x - target_tile.x).abs();
    let dy = (player_tile.y - target_tile.y).abs();
    let dist = dx.max(dy);

    if dist <= action.intent.range {
        *facing = facing_toward(player_tile, target_tile, *facing);
        action.phase = ActionPhase::Impact;
        action.impact_armed = false;
        action.clip_started = false;
    } else {
        action.phase = ActionPhase::Failed;
    }
}

// ------------------------------------------------------------
// 3.5) Impact -> start clip ONCE (OneShot) for chopdown
// ------------------------------------------------------------

pub fn drive_action_clip_on_impact(
    time: Res<Time>,
    mut commands: Commands,
    mut q: Query<(Entity, &mut CurrentAction, Option<&mut RequestedAnim>), With<Player>>,
) {
    let Ok((ent, mut action, req_opt)) = q.single_mut() else { return; };

    if action.phase != ActionPhase::Impact {
        return;
    }

    // Only ChopDown uses a swing clip for now.
    let verb_uses_clip = matches!(action.intent.verb, Verb::ChopDown);
    if !verb_uses_clip {
        return;
    }

    // Ensure we only start the clip once per impact cycle.
    if action.clip_started {
        return;
    }

    // If there is already a RequestedAnim for some reason, don't insert again.
    if req_opt.is_some() {
        action.clip_started = true;
        return;
    }

    // One swing duration: N frames at WALK_FPS plus a small hold.
    // (Matches animation.rs cadence: hold_last_secs for woodcutting is ~0.12)
    let frames_len = WALK_FRAMES.len().max(1) as f32;
    let swing_secs = (frames_len / WALK_FPS) + 0.12;

    commands.entity(ent).insert(crate::plugins::skills::RequestedAnim {
        clip: AnimClip::Woodcutting,
        mode: crate::plugins::skills::RequestedAnimMode::OneShot {
            timer: Timer::from_seconds(swing_secs, TimerMode::Once),
        },
    });

    action.clip_started = true;

    // Make sure cooldown doesn’t instantly re-arm while we’re mid impact.
    // (Not strictly required, but helps keep impact stable.)
    action.cooldown.tick(time.delta());
}

// ------------------------------------------------------------
// 4) Resolve (after clip ends)
// ------------------------------------------------------------

pub fn resolve_actions_on_impact(
    mut commands: Commands,
    mut player_q: Query<(Entity, &mut CurrentAction, Option<&mut RequestedAnim>), With<Player>>,
    mut resolved_writer: MessageWriter<ActionResolvedMsg>,
) {
    let Ok((player_ent, mut action, req_opt)) = player_q.single_mut() else { return; };

    // If we failed, clean up and bail.
    if action.phase == ActionPhase::Failed {
        commands.entity(player_ent).remove::<CurrentAction>();
        commands.entity(player_ent).remove::<RequestedAnim>();
        return;
    }

    if action.phase != ActionPhase::Impact {
        return;
    }

    // One-frame arming so we never resolve on the same tick we enter Impact.
    if !action.impact_armed {
        action.impact_armed = true;
        return;
    }

    let verb_uses_clip = matches!(action.intent.verb, Verb::ChopDown);

    if verb_uses_clip {
        // If clip never started, don't resolve.
        if !action.clip_started {
            return;
        }

        // While the oneshot is still running, don't resolve yet.
        if let Some(req) = req_opt.as_ref() {
            if !req.mode.just_finished() {
                return;
            }
        } else {
            // If there is no RequestedAnim, we treat it as "clip already ended".
            // (This also supports external cancellation.)
        }

        // Clip is done -> ensure it gets removed (Commands applies end-of-frame)
        commands.entity(player_ent).remove::<RequestedAnim>();
    }

    // Now we can resolve the action (this triggers woodcutting.rs etc.)
    resolved_writer.write(ActionResolvedMsg {
        actor: player_ent,
        intent: action.intent,
    });

    // Looping actions (ChopDown) go back to Moving for the next cycle.
    if action.looping {
        action.phase = ActionPhase::Moving;
        action.impact_armed = false;
        action.clip_started = false;

        // Reset cooldown so we don’t instantly re-impact in the same frame.
        action.cooldown.reset();
        return;
    }

    // Non-looping: complete and clear.
    action.phase = ActionPhase::Complete;
    commands.entity(player_ent).remove::<CurrentAction>();
}

pub fn debug_print_resolved_actions(mut reader: MessageReader<ActionResolvedMsg>) {
    for ev in reader.read() {
        info!(
            "[resolve] actor={:?} verb={:?} target={:?}",
            ev.actor, ev.intent.verb, ev.intent.target
        );
    }
}

// ------------------------------------------------------------
// Helpers
// ------------------------------------------------------------

fn timer_done(t: &Timer) -> bool {
    t.elapsed() >= t.duration()
}

fn facing_from_step(from: TilePos, to: TilePos) -> Facing {
    if to.x > from.x {
        Facing::East
    } else if to.x < from.x {
        Facing::West
    } else if to.y > from.y {
        Facing::North
    } else {
        Facing::South
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

fn pick_best_adjacent_goal_unblocked(
    world: &WorldGrid,
    start: TilePos,
    target: TilePos,
    range: i32,
) -> Option<TilePos> {
    let mut best: Option<TilePos> = None;
    let mut best_score = i32::MAX;

    for dx in -range..=range {
        for dy in -range..=range {
            if dx.abs() + dy.abs() > range {
                continue;
            }
            if dx == 0 && dy == 0 {
                continue;
            }

            let cand = TilePos::new(target.x + dx, target.y + dy);

            if world.is_blocked(cand) {
                continue;
            }

            let dist = (cand.x - start.x).abs() + (cand.y - start.y).abs();
            if dist < best_score {
                best_score = dist;
                best = Some(cand);
            }
        }
    }

    best
}