use bevy::prelude::*;
use bevy::text::LineHeight;

use super::common::*;
use super::super::fonts::UiFonts;
use super::super::net;
use super::super::state::{BootState, FocusField, Screen};

pub fn login_enter(mut commands: Commands, fonts: Res<UiFonts>, st: Res<BootState>) {
    let root = spawn_root(&mut commands);

    // Center container
    let container = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            Name::new("login_container"),
        ))
        .id();
    commands.entity(root).add_child(container);

    let panel = spawn_panel(&mut commands, container, 720.0);

    spawn_title(
        &mut commands,
        panel,
        &fonts,
        if st.login_mode_is_register { "Register" } else { "Login" },
        32.0,
    );
    spawn_text(
        &mut commands,
        panel,
        &fonts,
        "Click a field, type, press Enter to submit.",
        14.0,
    );

    let err = commands
        .spawn((
            Text::new(st.error_banner.clone().unwrap_or_default()),
            TextFont {
                font: fonts.mono.clone(),
                font_size: 14.0,
                ..default()
            },
            TextColor(Color::WHITE),
            LineHeight::default(),
            ErrorBannerText,
        ))
        .id();
    commands.entity(panel).add_child(err);

    spawn_input_row(
        &mut commands,
        panel,
        &fonts,
        "Email",
        &field_value(&st, InputFieldKind::Email),
        st.focus == FocusField::Email,
        InputFieldKind::Email,
    );
    spawn_input_row(
        &mut commands,
        panel,
        &fonts,
        "Password",
        &field_value(&st, InputFieldKind::Password),
        st.focus == FocusField::Password,
        InputFieldKind::Password,
    );

    if st.login_mode_is_register {
        spawn_input_row(
            &mut commands,
            panel,
            &fonts,
            "Display",
            &field_value(&st, InputFieldKind::DisplayName),
            st.focus == FocusField::DisplayName,
            InputFieldKind::DisplayName,
        );
    }

    if st.login_mode_is_register {
        spawn_button(&mut commands, panel, &fonts, "Switch to Login", ButtonAction::GoAccountLogin);
    } else {
        spawn_button(&mut commands, panel, &fonts, "Switch to Register", ButtonAction::GoRegister);
    }

    spawn_button(&mut commands, panel, &fonts, "Submit", ButtonAction::SubmitAuth);
    spawn_button(&mut commands, panel, &fonts, "Back", ButtonAction::BackToMainMenu);
}

pub fn login_update(
    mut next: ResMut<NextState<Screen>>,
    mut st: ResMut<BootState>,
    keys: Res<ButtonInput<KeyCode>>,
    mut q_btn: Query<(&Interaction, &ButtonAction), (Changed<Interaction>, With<Button>)>,
    net_runtime: Res<net::NetRuntime>,
    mut q_inputs: Query<(&mut Text, &InputValueText), Without<ErrorBannerText>>,
    mut q_error: Query<&mut Text, (With<ErrorBannerText>, Without<InputValueText>)>,
    mut q_row_bg: Query<(&mut BackgroundColor, &ButtonAction), With<Button>>,
) {
    // button clicks
    for (i, a) in &mut q_btn {
        if *i != Interaction::Pressed {
            continue;
        }
        st.clear_errors();

        match a {
            ButtonAction::GoRegister => {
                st.login_mode_is_register = true;
                st.focus = FocusField::Email;
                next.set(Screen::AccountLogin);
            }
            ButtonAction::GoAccountLogin => {
                st.login_mode_is_register = false;
                st.focus = FocusField::Email;
                next.set(Screen::AccountLogin);
            }
            ButtonAction::SubmitAuth => {
                if st.busy {
                    continue;
                }
                st.busy = true;

                let email = st.email.clone();
                let pw = st.password.clone();
                let display = st.display_name.clone();

                if st.login_mode_is_register {
                    net::spawn_register(&st, net_runtime.as_ref(), email, pw, display);
                } else {
                    net::spawn_login(&st, net_runtime.as_ref(), email, pw);
                }
            }
            ButtonAction::BackToMainMenu => {
                st.busy = false;
                next.set(Screen::MainMenu);
            }
            ButtonAction::Focus(field) => st.focus = focus_to_field(*field),
            _ => {}
        }
    }

    // tab focus cycle
    if keys.just_pressed(KeyCode::Tab) {
        st.focus = match st.focus {
            FocusField::Email => FocusField::Password,
            FocusField::Password => {
                if st.login_mode_is_register {
                    FocusField::DisplayName
                } else {
                    FocusField::Email
                }
            }
            FocusField::DisplayName => FocusField::Email,
            FocusField::NewCharacterName => FocusField::Email,
        };
    }

    // typing
    match st.focus {
        FocusField::Email => { push_typed_keys(&keys, &mut st.email); }
        FocusField::Password => { push_typed_keys(&keys, &mut st.password); }
        FocusField::DisplayName => { push_typed_keys(&keys, &mut st.display_name); }
        _ => {}
    }

    // enter submit
    if keys.just_pressed(KeyCode::Enter) && !st.busy {
        st.busy = true;

        let email = st.email.clone();
        let pw = st.password.clone();
        let display = st.display_name.clone();

        if st.login_mode_is_register {
            net::spawn_register(&st, net_runtime.as_ref(), email, pw, display);
        } else {
            net::spawn_login(&st, net_runtime.as_ref(), email, pw);
        }
    }

    // server says we have a session
    if st.session.is_some() {
        st.busy = false;
        next.set(Screen::MainMenu);
    }

    // update error banner
    if let Some(mut t) = q_error.iter_mut().next() {
        t.0 = st.error_banner.clone().unwrap_or_default();
    }

    // update input field text
    for (mut t, tag) in &mut q_inputs {
        t.0 = field_value(&st, tag.field);
    }

    // update focus border colors
    for (mut bg, action) in &mut q_row_bg {
        let ButtonAction::Focus(field) = action else { continue; };

        let focused = st.focus == focus_to_field(*field);
        let border = if focused {
            Color::srgb(0.35, 0.55, 1.0)
        } else {
            Color::srgb(0.25, 0.25, 0.32)
        };
        *bg = BackgroundColor(border);
    }
}