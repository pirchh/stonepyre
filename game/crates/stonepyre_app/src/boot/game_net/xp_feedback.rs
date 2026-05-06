use bevy::prelude::*;
use stonepyre_world::{tile_to_world_center, TILE_SIZE};

use super::protocol::SkillXpSource;
use super::status::{GameNetStatus, SkillXpFeedbackEntry};

const XP_FLOAT_SECONDS: f32 = 1.45;
const XP_FLOAT_DISTANCE: f32 = 42.0;
const XP_TEXT_Z: f32 = 100.0;
const XP_TEXT_START_Y_OFFSET: f32 = TILE_SIZE * 0.45;

#[derive(Component)]
pub struct XpFeedbackRoot;

#[derive(Component)]
pub struct XpFeedbackToast {
    timer: Timer,
    base_y: f32,
}

/// Kept as the enter-world hook for XP feedback setup.
///
/// The old implementation spawned a fixed-position UI layer here. World-space
/// XP feedback does not need a UI root, so this hook intentionally stays empty.
pub fn spawn_xp_feedback_layer() {}

pub fn despawn_xp_feedback_layer(
    mut commands: Commands,
    feedback_q: Query<Entity, With<XpFeedbackToast>>,
) {
    for entity in feedback_q.iter() {
        if let Ok(mut entity_commands) = commands.get_entity(entity) {
            entity_commands.despawn();
        }
    }
}

pub fn update_xp_feedback_layer(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut status: ResMut<GameNetStatus>,
) {
    if status.xp_feedback_queue.is_empty() {
        return;
    }

    let font = asset_server.load("fonts/ui.ttf");
    let queued: Vec<SkillXpFeedbackEntry> = status.xp_feedback_queue.drain(..).collect();

    for entry in queued {
        let Some(world_pos) = xp_feedback_world_position(&entry) else {
            continue;
        };

        let start_y = world_pos.y + XP_TEXT_START_Y_OFFSET;

        commands.spawn((
            Text2d::new(format!(
                "+{} {} XP",
                entry.xp_delta,
                entry.display_name
            )),
            TextFont {
                font: font.clone(),
                font_size: 22.0,
                ..default()
            },
            TextColor(Color::srgba(0.96, 0.86, 0.48, 1.0)),
            Transform::from_xyz(world_pos.x, start_y, XP_TEXT_Z),
            XpFeedbackToast {
                timer: Timer::from_seconds(XP_FLOAT_SECONDS, TimerMode::Once),
                base_y: start_y,
            },
            Name::new("world_space_xp_feedback_text"),
        ));
    }
}

pub fn tick_xp_feedback_toasts(
    mut commands: Commands,
    time: Res<Time>,
    mut toasts: Query<(Entity, &mut Transform, &mut TextColor, &mut XpFeedbackToast)>,
) {
    for (entity, mut transform, mut text_color, mut toast) in &mut toasts {
        toast.timer.tick(time.delta());

        let duration = toast.timer.duration().as_secs_f32().max(f32::EPSILON);
        let progress = (toast.timer.elapsed_secs() / duration).clamp(0.0, 1.0);

        transform.translation.y = toast.base_y + (XP_FLOAT_DISTANCE * progress);

        let alpha = (1.0 - progress).clamp(0.0, 1.0);
        text_color.0 = Color::srgba(0.96, 0.86, 0.48, alpha);

        if toast.timer.is_finished() {
            if let Ok(mut entity_commands) = commands.get_entity(entity) {
                entity_commands.despawn();
            }
        }
    }
}

fn xp_feedback_world_position(entry: &SkillXpFeedbackEntry) -> Option<Vec2> {
    match entry.source.as_ref()? {
        SkillXpSource::HarvestNode { tile, .. } => Some(tile_to_world_center(*tile)),
    }
}
