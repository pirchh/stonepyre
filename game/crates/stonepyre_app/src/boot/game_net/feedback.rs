//! Right-side feedback drop stack (RuneScape-style).
//!
//! Renders short-lived drops along the right edge of the screen for:
//! - XP gains      ("+10 Woodcutting XP")
//! - item gains    ("[icon] +1 Oak Log")
//! - status text   ("Need a Copper Axe")
//!
//! Drops are queued in `GameNetStatus::feedback_drops` (filled by the net pump
//! and the harvest-gate predictor), drained into floating UI entities here, and
//! rise + fade out before despawning.

use bevy::prelude::*;

use stonepyre_engine::plugins::inventory::ItemDb;

use super::status::{FeedbackDrop, GameNetStatus};

const DROP_LIFETIME: f32 = 2.0;
const DROP_RISE_PX: f32 = 22.0;
/// Drops sit just below the minimap in the top-right (RuneScape layout). Derived
/// from the minimap's footprint so the two stay aligned if the minimap resizes.
const DROP_BASE_TOP: f32 = super::minimap::FEEDBACK_DROP_TOP;
const DROP_RIGHT_PX: f32 = 28.0;
const DROP_ROW_GAP: f32 = 26.0;
/// Cap on how far simultaneous drops stagger, so an unusually large batch can't
/// push entries far down the screen.
const MAX_STAGGER_ROWS: usize = 3;
const ICON_SIZE: f32 = 22.0;

#[derive(Component)]
pub struct FeedbackDropToast {
    timer: Timer,
    base_top: f32,
    text_color: Color,
}

/// Enter-world hook. Drops are absolutely-positioned and self-contained, so no
/// persistent layer entity is needed.
pub fn spawn_feedback_layer() {}

pub fn despawn_feedback_layer(
    mut commands: Commands,
    drops_q: Query<Entity, With<FeedbackDropToast>>,
) {
    for entity in drops_q.iter() {
        if let Ok(mut ec) = commands.get_entity(entity) {
            ec.despawn();
        }
    }
}

pub fn update_feedback_drops(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    item_db: Res<ItemDb>,
    mut status: ResMut<GameNetStatus>,
) {
    if status.feedback_drops.is_empty() {
        return;
    }

    let font = asset_server.load("fonts/ui.ttf");
    let drops: Vec<FeedbackDrop> = status.feedback_drops.drain(..).collect();

    // Stagger drops queued on the same frame (e.g. an XP drop + an item drop from
    // one successful chop) so they don't overlap.
    for (i, drop) in drops.into_iter().enumerate() {
        let base_top = DROP_BASE_TOP + i.min(MAX_STAGGER_ROWS) as f32 * DROP_ROW_GAP;
        spawn_drop(&mut commands, &asset_server, &item_db, &font, drop, base_top);
    }
}

fn spawn_drop(
    commands: &mut Commands,
    asset_server: &AssetServer,
    item_db: &ItemDb,
    font: &Handle<Font>,
    drop: FeedbackDrop,
    base_top: f32,
) {
    let (icon, text, color) = match drop {
        FeedbackDrop::Xp { skill_display, amount } => (
            None,
            format!("+{amount} {skill_display} XP"),
            Color::srgb(0.96, 0.86, 0.48),
        ),
        FeedbackDrop::Item { item_id, quantity } => {
            let (name, icon_path) = item_db
                .get(&item_id)
                .map(|d| (d.name.clone(), d.inventory_icon.clone()))
                .unwrap_or((item_id.clone(), None));
            let icon = icon_path.map(|p| asset_server.load(p));
            (icon, format!("+{quantity} {name}"), Color::srgb(0.80, 0.93, 0.70))
        }
        FeedbackDrop::Message { text } => (None, text, Color::srgb(0.96, 0.56, 0.46)),
    };

    let root = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(DROP_RIGHT_PX),
                top: Val::Px(base_top),
                display: Display::Flex,
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(6.0),
                ..default()
            },
            FeedbackDropToast {
                timer: Timer::from_seconds(DROP_LIFETIME, TimerMode::Once),
                base_top,
                text_color: color,
            },
            Name::new("feedback_drop"),
        ))
        .id();

    if let Some(icon) = icon {
        let icon_e = commands
            .spawn((
                ImageNode::new(icon),
                Node {
                    width: Val::Px(ICON_SIZE),
                    height: Val::Px(ICON_SIZE),
                    ..default()
                },
            ))
            .id();
        commands.entity(root).add_child(icon_e);
    }

    let text_e = commands
        .spawn((
            Text::new(text),
            TextFont {
                font: font.clone(),
                font_size: 18.0,
                ..default()
            },
            TextColor(color),
        ))
        .id();
    commands.entity(root).add_child(text_e);
}

pub fn tick_feedback_drops(
    mut commands: Commands,
    time: Res<Time>,
    mut toasts: Query<(Entity, &mut Node, &mut FeedbackDropToast, &Children)>,
    mut text_q: Query<&mut TextColor>,
    mut image_q: Query<&mut ImageNode>,
) {
    for (entity, mut node, mut toast, children) in &mut toasts {
        toast.timer.tick(time.delta());
        let progress = (toast.timer.elapsed_secs() / DROP_LIFETIME).clamp(0.0, 1.0);

        node.top = Val::Px(toast.base_top - DROP_RISE_PX * progress);

        let alpha = 1.0 - progress;
        for child in children.iter() {
            if let Ok(mut tc) = text_q.get_mut(child) {
                tc.0 = toast.text_color.with_alpha(alpha);
            }
            if let Ok(mut img) = image_q.get_mut(child) {
                img.color = Color::WHITE.with_alpha(alpha);
            }
        }

        if toast.timer.is_finished() {
            if let Ok(mut ec) = commands.get_entity(entity) {
                ec.despawn();
            }
        }
    }
}
