use bevy::prelude::*;

use super::common::*;
use super::super::fonts::UiFonts;
use super::super::state::{BootState, FocusField, Screen};

#[derive(Clone, Copy)]
struct UpdateRow {
    when: &'static str,
    title: &'static str,
    status: &'static str,
}

fn spawn_updates_panel(commands: &mut Commands, root: Entity, fonts: &UiFonts) {
    let container = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(36.0),
                top: Val::Px(36.0),
                width: Val::Px(560.0),
                height: Val::Px(820.0),
                ..default()
            },
            Name::new("main_menu_right_container"),
        ))
        .id();
    commands.entity(root).add_child(container);

    let panel = spawn_panel_fixed(commands, container, 560.0, 820.0);

    spawn_title(commands, panel, fonts, "Updates", 26.0);
    spawn_text(commands, panel, fonts, "Latest development notes (placeholder)", 14.0);

    let header = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(34.0),
                display: Display::Flex,
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(10.0),
                align_items: AlignItems::Center,
                padding: UiRect::horizontal(Val::Px(8.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.02, 0.02, 0.03, 0.65)),
            Name::new("updates_header"),
        ))
        .id();
    commands.entity(panel).add_child(header);

    let mut header_col = |w: f32, txt: &str| {
        let col = commands
            .spawn((
                Node {
                    width: Val::Px(w),
                    ..default()
                },
                Name::new("updates_header_col"),
            ))
            .id();
        commands.entity(header).add_child(col);
        spawn_mono(commands, col, fonts, txt, 14.0);
    };

    header_col(110.0, "DATE");
    header_col(330.0, "TITLE");
    header_col(80.0, "STATUS");

    let list = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                padding: UiRect::all(Val::Px(8.0)),
                display: Display::Flex,
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(6.0),
                overflow: Overflow::clip_y(),
                ..default()
            },
            BackgroundColor(Color::srgba(0.02, 0.02, 0.03, 0.35)),
            Name::new("updates_list"),
        ))
        .id();
    commands.entity(panel).add_child(list);

    let rows = [
        UpdateRow { when: "Mar 01", title: "Boot main menu layout pass", status: "OK" },
        UpdateRow { when: "Feb 28", title: "Auth + character selection wired", status: "WIP" },
        UpdateRow { when: "Feb 26", title: "Market sim candlesticks persisted", status: "OK" },
        UpdateRow { when: "Feb 23", title: "Paperdoll UI prototype started", status: "OK" },
        UpdateRow { when: "Feb 21", title: "Viewer tool-fit modernization", status: "OK" },
    ];

    for r in rows {
        let row = commands
            .spawn((
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Auto,
                    display: Display::Flex,
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(10.0),
                    align_items: AlignItems::Center,
                    padding: UiRect::horizontal(Val::Px(8.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.07, 0.07, 0.09, 0.55)),
                Name::new("updates_row"),
            ))
            .id();
        commands.entity(list).add_child(row);

        let add_cell = |commands: &mut Commands,
                        row: Entity,
                        fonts: &UiFonts,
                        w: f32,
                        txt: &str,
                        mono: bool| {
            let cell = commands
                .spawn((
                    Node {
                        width: Val::Px(w),
                        ..default()
                    },
                    Name::new("updates_cell"),
                ))
                .id();
            commands.entity(row).add_child(cell);

            if mono {
                spawn_mono(commands, cell, fonts, txt, 14.0);
            } else {
                spawn_text(commands, cell, fonts, txt, 14.0);
            }
        };

        add_cell(commands, row, fonts, 110.0, r.when, true);
        add_cell(commands, row, fonts, 330.0, r.title, false);
        add_cell(commands, row, fonts, 80.0, r.status, true);
    }
}

fn spawn_auth_pill(commands: &mut Commands, root: Entity, fonts: &UiFonts, st: &BootState) {
    let (label, ok) = if st.session.is_some() {
        let mut name = st.display_name.trim().to_string();
        if name.is_empty() {
            name = st.email.trim().to_string();
        }
        if name.is_empty() {
            name = "Player".to_string();
        }
        (format!("Logged in as {}", name), true)
    } else {
        ("Not logged in".to_string(), false)
    };

    let bg = if ok {
        Color::srgba(0.08, 0.14, 0.10, 0.88)
    } else {
        Color::srgba(0.14, 0.08, 0.08, 0.88)
    };

    let container = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(24.0),
                top: Val::Px(18.0),
                padding: UiRect {
                    left: Val::Px(12.0),
                    right: Val::Px(12.0),
                    top: Val::Px(8.0),
                    bottom: Val::Px(8.0),
                },
                ..default()
            },
            BackgroundColor(bg),
            Name::new("auth_pill"),
        ))
        .id();
    commands.entity(root).add_child(container);

    spawn_mono(commands, container, fonts, label, 14.0);
}

pub fn main_menu_enter(
    mut commands: Commands,
    fonts: Res<UiFonts>,
    asset_server: Res<AssetServer>,
    st: Res<BootState>,
) {
    let root = spawn_root(&mut commands);

    spawn_background(&mut commands, root, &asset_server, None);

    // Top-left status
    spawn_auth_pill(&mut commands, root, &fonts, &st);

    let left_container = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(90.0),
                top: Val::Px(0.0),
                width: Val::Px(720.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            Name::new("main_menu_left_container"),
        ))
        .id();
    commands.entity(root).add_child(left_container);

    let panel = spawn_panel(&mut commands, left_container, 720.0);

    spawn_title(&mut commands, panel, &fonts, "Stonepyre", 52.0);
    spawn_text(
        &mut commands,
        panel,
        &fonts,
        "A systems-first sandbox RPG (boot flow prototype)",
        16.0,
    );

    // spacer
    let spacer = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(10.0),
                ..default()
            },
            Name::new("spacer"),
        ))
        .id();
    commands.entity(panel).add_child(spacer);

    if st.session.is_none() {
        spawn_button(
            &mut commands,
            panel,
            &fonts,
            "Account Login",
            ButtonAction::GoAccountLogin,
        );
    } else {
        // ✅ Play goes straight to Character Select now
        spawn_button(&mut commands, panel, &fonts, "Play", ButtonAction::GoCharacterSelect);

        spawn_button(
            &mut commands,
            panel,
            &fonts,
            "Settings (soon)",
            ButtonAction::BackToMainMenu,
        );

        spawn_button(&mut commands, panel, &fonts, "Logout", ButtonAction::Logout);

        spawn_updates_panel(&mut commands, root, &fonts);
    }
}

pub fn main_menu_update(
    mut next: ResMut<NextState<Screen>>,
    mut q_btn: Query<(&Interaction, &ButtonAction), (Changed<Interaction>, With<Button>)>,
    mut st: ResMut<BootState>,
) {
    for (i, a) in &mut q_btn {
        if *i != Interaction::Pressed {
            continue;
        }

        st.clear_errors();

        match a {
            ButtonAction::GoAccountLogin => {
                st.login_mode_is_register = false;
                st.focus = FocusField::Email;
                next.set(Screen::AccountLogin);
            }

            ButtonAction::GoCharacterSelect => {
                if st.session.is_none() {
                    st.login_mode_is_register = false;
                    st.focus = FocusField::Email;
                    next.set(Screen::AccountLogin);
                } else {
                    next.set(Screen::CharacterSelect);
                }
            }

            ButtonAction::Logout => {
                st.session = None;
                st.busy = false;

                st.login_mode_is_register = false;
                st.focus = FocusField::Email;
                next.set(Screen::AccountLogin);
            }

            _ => {}
        }
    }
}