use bevy::prelude::*;

use super::status::{GameNetStatus, SkillXpFeedbackEntry};

const XP_TOAST_SECONDS: f32 = 1.65;

#[derive(Component)]
pub struct XpFeedbackRoot;

#[derive(Component)]
pub struct XpFeedbackToast {
    timer: Timer,
}

pub fn spawn_xp_feedback_layer(mut commands: Commands) {
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            right: Val::Px(24.0),
            top: Val::Px(118.0),
            width: Val::Px(310.0),
            height: Val::Auto,
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::FlexEnd,
            row_gap: Val::Px(8.0),
            ..default()
        },
        XpFeedbackRoot,
        Name::new("xp_feedback_layer"),
    ));
}

pub fn despawn_xp_feedback_layer(
    mut commands: Commands,
    roots: Query<Entity, With<XpFeedbackRoot>>,
) {
    for root in roots.iter() {
        if let Ok(mut entity) = commands.get_entity(root) {
            entity.despawn();
        }
    }
}

pub fn update_xp_feedback_layer(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut status: ResMut<GameNetStatus>,
    roots: Query<Entity, With<XpFeedbackRoot>>,
) {
    if status.xp_feedback_queue.is_empty() {
        return;
    }

    let Ok(root) = roots.single() else {
        status.xp_feedback_queue.clear();
        return;
    };

    let font = asset_server.load("fonts/ui.ttf");
    let queued: Vec<SkillXpFeedbackEntry> = status.xp_feedback_queue.drain(..).collect();

    for entry in queued {
        let toast = commands
            .spawn((
                Node {
                    width: Val::Px(300.0),
                    height: Val::Auto,
                    padding: UiRect::axes(Val::Px(12.0), Val::Px(8.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.04, 0.05, 0.06, 0.82)),
                XpFeedbackToast {
                    timer: Timer::from_seconds(XP_TOAST_SECONDS, TimerMode::Once),
                },
                Name::new("xp_feedback_toast"),
            ))
            .id();

        let text = commands
            .spawn((
                Text::new(format!(
                    "+{} {} XP",
                    entry.xp_delta,
                    entry.display_name
                )),
                TextFont {
                    font: font.clone(),
                    font_size: 20.0,
                    ..default()
                },
                TextColor(Color::srgb(0.96, 0.86, 0.48)),
                Name::new("xp_feedback_toast_text"),
            ))
            .id();

        commands.entity(toast).add_child(text);
        commands.entity(root).add_child(toast);
    }
}

pub fn tick_xp_feedback_toasts(
    mut commands: Commands,
    time: Res<Time>,
    mut toasts: Query<(Entity, &mut XpFeedbackToast)>,
) {
    for (entity, mut toast) in &mut toasts {
        toast.timer.tick(time.delta());

        if toast.timer.is_finished() {
            if let Ok(mut entity_commands) = commands.get_entity(entity) {
                entity_commands.despawn();
            }
        }
    }
}
