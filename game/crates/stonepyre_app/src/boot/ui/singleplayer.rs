use bevy::prelude::*;

use super::common::*;
use super::super::fonts::UiFonts;
use super::super::state::Screen;

pub fn singleplayer_enter(mut commands: Commands, fonts: Res<UiFonts>) {
    let root = spawn_root(&mut commands);

    let container = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            Name::new("singleplayer_container"),
        ))
        .id();
    commands.entity(root).add_child(container);

    let panel = spawn_panel(&mut commands, container, 720.0);

    spawn_title(&mut commands, panel, &fonts, "Singleplayer", 32.0);
    spawn_text(&mut commands, panel, &fonts, "Placeholder menu.", 14.0);

    spawn_button(&mut commands, panel, &fonts, "Character Selection", ButtonAction::GoCharacterSelect);
    spawn_button(&mut commands, panel, &fonts, "Back", ButtonAction::BackToMainMenu);
}

pub fn singleplayer_update(
    mut next: ResMut<NextState<Screen>>,
    mut q_btn: Query<(&Interaction, &ButtonAction), (Changed<Interaction>, With<Button>)>,
) {
    for (i, a) in &mut q_btn {
        if *i != Interaction::Pressed {
            continue;
        }
        match a {
            ButtonAction::GoCharacterSelect => next.set(Screen::CharacterSelect),
            ButtonAction::BackToMainMenu => next.set(Screen::MainMenu),
            _ => {}
        }
    }
}