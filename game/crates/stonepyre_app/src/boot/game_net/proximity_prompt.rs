use bevy::prelude::*;

use stonepyre_engine::plugins::world::NearbyInteractable;

/// Marker for the proximity prompt root node.
#[derive(Component)]
pub struct ProximityPrompt;

/// Marker for the text inside the proximity prompt.
#[derive(Component)]
pub struct ProximityPromptText;

pub fn spawn_proximity_prompt(mut commands: Commands) {
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                bottom: Val::Px(80.0),
                left: Val::Percent(50.0),
                // Shift left by ~half the expected text width so it centres.
                margin: UiRect::left(Val::Px(-150.0)),
                padding: UiRect::axes(Val::Px(18.0), Val::Px(8.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.55)),
            Visibility::Hidden,
            ProximityPrompt,
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new(""),
                TextFont {
                    font_size: 17.0,
                    ..default()
                },
                TextColor(Color::srgb(1.0, 0.92, 0.6)),
                ProximityPromptText,
            ));
        });
}

pub fn despawn_proximity_prompt(
    mut commands: Commands,
    q: Query<Entity, With<ProximityPrompt>>,
) {
    for e in q.iter() {
        commands.entity(e).despawn();
    }
}

pub fn update_proximity_prompt(
    nearby: Res<NearbyInteractable>,
    mut prompt_q: Query<&mut Visibility, With<ProximityPrompt>>,
    mut text_q: Query<&mut Text, With<ProximityPromptText>>,
) {
    let Ok(mut vis) = prompt_q.single_mut() else { return };
    let Ok(mut text) = text_q.single_mut() else { return };

    match &nearby.kind {
        Some(kind) => {
            **text = kind.proximity_prompt().to_string();
            *vis = Visibility::Visible;
        }
        None => {
            *vis = Visibility::Hidden;
        }
    }
}
