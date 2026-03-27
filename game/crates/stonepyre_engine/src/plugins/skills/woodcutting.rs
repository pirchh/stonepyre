use bevy::prelude::*;

use crate::plugins::animation::ForceIdleOnce;
use crate::plugins::interaction::{ActionPhase, ActionResolvedMsg, CurrentAction, Target, Verb};
use crate::plugins::rng::GameRng;
use crate::plugins::skills::{HarvestDb, HarvestNode, RequestedAnim};
use crate::plugins::world::InteractableKind;

pub fn on_action_resolved_apply_woodcutting(
    mut commands: Commands,
    mut resolved: MessageReader<ActionResolvedMsg>,
    mut rng: ResMut<GameRng>,
    db: Option<Res<HarvestDb>>,
    mut trees_q: Query<(Entity, &mut HarvestNode, &mut Visibility, &InteractableKind)>,
    mut actor_action_q: Query<&mut CurrentAction>,
) {
    for ev in resolved.read() {
        if ev.intent.verb != Verb::ChopDown {
            continue;
        }

        let Target::Entity(target_ent) = ev.intent.target else { continue; };

        let Ok((_tree_e, mut node, mut vis, kind)) = trees_q.get_mut(target_ent) else {
            // target vanished -> stop looping action + stop anim
            if let Ok(mut act) = actor_action_q.get_mut(ev.actor) {
                act.looping = false;
                act.phase = ActionPhase::Failed;
            }
            commands.entity(ev.actor).remove::<RequestedAnim>();
            commands.entity(ev.actor).insert(ForceIdleOnce);
            continue;
        };

        if *kind != InteractableKind::Tree {
            if let Ok(mut act) = actor_action_q.get_mut(ev.actor) {
                act.looping = false;
                act.phase = ActionPhase::Failed;
            }
            commands.entity(ev.actor).remove::<RequestedAnim>();
            commands.entity(ev.actor).insert(ForceIdleOnce);
            continue;
        }

        // If depleted, hard stop and tell the logs
        if node.is_depleted() {
            info!("[woodcutting] yo thats depleted.");
            *vis = Visibility::Hidden;

            if let Ok(mut act) = actor_action_q.get_mut(ev.actor) {
                act.looping = false;
                act.phase = ActionPhase::Failed;
            }

            commands.entity(ev.actor).remove::<RequestedAnim>();
            commands.entity(ev.actor).insert(ForceIdleOnce);
            continue;
        }

        // (optional) prove we can access the def if needed
        if let Some(db) = db.as_ref() {
            let _def = db.get(node.def_id);
            // later: use _def for level req, loot tables, etc.
        }

        // chance scales with level later; for now fixed placeholder
        let p_success = 0.55;
        let roll = rng.roll_f32();
        let success = roll < p_success;

        if success {
            let before = node.charges_left;

            // IMPORTANT: consume_one resets regen timer so regen doesn't instantly refill
            node.consume_one();

            info!(
                "[woodcutting] success roll={:.2} < {:.2} charges_left={} -> {}",
                roll, p_success, before, node.charges_left
            );

            if node.is_depleted() {
                info!("[woodcutting] tree depleted -> stopping action loop");
                *vis = Visibility::Hidden;

                if let Ok(mut act) = actor_action_q.get_mut(ev.actor) {
                    act.looping = false;
                    act.phase = ActionPhase::Failed;
                }

                commands.entity(ev.actor).remove::<RequestedAnim>();
                commands.entity(ev.actor).insert(ForceIdleOnce);
            }
        } else {
            info!(
                "[woodcutting] fail roll={:.2} >= {:.2} charges_left={}",
                roll, p_success, node.charges_left
            );
        }
    }
}