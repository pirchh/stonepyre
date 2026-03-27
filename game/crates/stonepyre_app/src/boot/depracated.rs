use bevy::prelude::*;
use bevy::text::LineHeight;
use uuid::Uuid;

use super::fonts::UiFonts;
use super::net;
use super::state::{BootState, FocusField, Screen};

#[derive(Component)]
pub(super) struct ScreenRoot;

#[derive(Component)]
pub(super) struct ErrorBannerText;

#[derive(Component, Clone, Copy)]
pub(super) enum InputFieldKind {
    Email,
    Password,
    DisplayName,
    NewCharacterName,
}

#[derive(Component)]
pub(super) struct InputValueText {
    pub(super) field: InputFieldKind,
}

#[derive(Component)]
pub(super) struct SlotText {
    pub(super) idx: usize,
}

#[derive(Component)]
pub(super) enum ButtonAction {
    // Main Menu
    GoAccountLogin,
    GoSingleplayer,

    // Account Login
    GoRegister,
    SubmitAuth,
    BackToMainMenu,

    // Singleplayer Menu (placeholder)
    GoCharacterSelect,
    BackToSingleplayer,

    // Main Menu actions (future)
    Logout,
    DeleteAccount,

    // Character Select
    RefreshCharacters,
    CreateCharacter,
    DeleteCharacter(Uuid),
    PlayCharacter(Uuid),

    Focus(InputFieldKind),
}

/// Bevy 0.18: don't depend on `despawn_recursive()` existing.
pub fn despawn_screen(
    mut commands: Commands,
    roots: Query<Entity, With<ScreenRoot>>,
    q_children: Query<&Children>,
) {
    for root in &roots {
        let mut stack = vec![root];
        let mut all = Vec::<Entity>::new();

        while let Some(e) = stack.pop() {
            all.push(e);
            if let Ok(children) = q_children.get(e) {
                for c in children.iter() {
                    stack.push(*c);
                }
            }
        }

        for e in all.into_iter().rev() {
            commands.entity(e).despawn();
        }
    }
}

// ============================================================
// Root / Common Widgets
// ============================================================

fn spawn_root(commands: &mut Commands) -> Entity {
    commands
        .spawn((
            ScreenRoot,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..default()
            },
            BackgroundColor(Color::srgb(0.05, 0.05, 0.07)),
            Name::new("boot_screen_root"),
        ))
        .id()
}

/// Optional fullscreen background image (kept simple).
/// If `path` is None, you still get the root's background color.
fn spawn_background(commands: &mut Commands, root: Entity, asset_server: &AssetServer, path: Option<&str>) {
    if let Some(p) = path {
        let img = asset_server.load::<Image>(p);
        let bg = commands
            .spawn((
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    position_type: PositionType::Absolute,
                    left: Val::Px(0.0),
                    top: Val::Px(0.0),
                    ..default()
                },
                ImageNode::new(img),
                Name::new("boot_bg_image"),
            ))
            .id();
        commands.entity(root).add_child(bg);
    }
}

/// A panel with a modern-ish background. You can position it however you want.
fn spawn_panel(commands: &mut Commands, parent: Entity, w: f32) -> Entity {
    let panel = commands
        .spawn((
            Node {
                width: Val::Px(w),
                height: Val::Auto,
                padding: UiRect::all(Val::Px(24.0)),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(12.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.08, 0.08, 0.10, 0.92)),
            Name::new("boot_panel"),
        ))
        .id();

    commands.entity(parent).add_child(panel);
    panel
}

/// A fixed-size panel (useful for Updates window).
fn spawn_panel_fixed(commands: &mut Commands, parent: Entity, w: f32, h: f32) -> Entity {
    let panel = commands
        .spawn((
            Node {
                width: Val::Px(w),
                height: Val::Px(h),
                padding: UiRect::all(Val::Px(18.0)),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(10.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.08, 0.08, 0.10, 0.88)),
            Name::new("boot_panel_fixed"),
        ))
        .id();

    commands.entity(parent).add_child(panel);
    panel
}

fn spawn_text(
    commands: &mut Commands,
    parent: Entity,
    fonts: &UiFonts,
    s: impl Into<String>,
    size: f32,
) -> Entity {
    let e = commands
        .spawn((
            Text::new(s.into()),
            TextFont {
                font: fonts.regular.clone(),
                font_size: size,
                ..default()
            },
            TextColor(Color::WHITE),
            LineHeight::default(),
        ))
        .id();

    commands.entity(parent).add_child(e);
    e
}

fn spawn_title(
    commands: &mut Commands,
    parent: Entity,
    fonts: &UiFonts,
    s: impl Into<String>,
    size: f32,
) -> Entity {
    let e = commands
        .spawn((
            Text::new(s.into()),
            TextFont {
                font: fonts.accent.clone(),
                font_size: size,
                ..default()
            },
            TextColor(Color::WHITE),
            LineHeight::default(),
        ))
        .id();

    commands.entity(parent).add_child(e);
    e
}

#[allow(dead_code)]
fn spawn_mono(
    commands: &mut Commands,
    parent: Entity,
    fonts: &UiFonts,
    s: impl Into<String>,
    size: f32,
) -> Entity {
    let e = commands
        .spawn((
            Text::new(s.into()),
            TextFont {
                font: fonts.mono.clone(),
                font_size: size,
                ..default()
            },
            TextColor(Color::WHITE),
            LineHeight::default(),
        ))
        .id();

    commands.entity(parent).add_child(e);
    e
}

fn spawn_button(
    commands: &mut Commands,
    parent: Entity,
    fonts: &UiFonts,
    label: &str,
    action: ButtonAction,
) -> Entity {
    let btn = commands
        .spawn((
            Button,
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(46.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgb(0.18, 0.18, 0.24)),
            action,
        ))
        .id();

    commands.entity(parent).add_child(btn);
    spawn_text(commands, btn, fonts, label, 18.0);
    btn
}

fn spawn_input_row(
    commands: &mut Commands,
    parent: Entity,
    fonts: &UiFonts,
    label: &str,
    value: &str,
    focus: bool,
    field: InputFieldKind,
) -> Entity {
    let border = if focus {
        Color::srgb(0.35, 0.55, 1.0)
    } else {
        Color::srgb(0.25, 0.25, 0.32)
    };

    let row = commands
        .spawn((
            Button,
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(44.0),
                justify_content: JustifyContent::FlexStart,
                align_items: AlignItems::Center,
                padding: UiRect::horizontal(Val::Px(12.0)),
                ..default()
            },
            BackgroundColor(border),
            ButtonAction::Focus(field),
        ))
        .id();

    commands.entity(parent).add_child(row);

    spawn_text(commands, row, fonts, format!("{label}: "), 16.0);

    let v = commands
        .spawn((
            Text::new(value.to_string()),
            TextFont {
                font: fonts.regular.clone(),
                font_size: 16.0,
                ..default()
            },
            TextColor(Color::WHITE),
            LineHeight::default(),
            InputValueText { field },
        ))
        .id();

    commands.entity(row).add_child(v);
    row
}

fn is_shift_down(keys: &ButtonInput<KeyCode>) -> bool {
    keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight)
}

fn push_typed_keys(keys: &ButtonInput<KeyCode>, out: &mut String) -> bool {
    let mut changed = false;
    let shift = is_shift_down(keys);

    for key in keys.get_just_pressed() {
        match key {
            KeyCode::Backspace => {
                out.pop();
                changed = true;
            }

            // letters
            KeyCode::KeyA => { out.push(if shift { 'A' } else { 'a' }); changed = true; }
            KeyCode::KeyB => { out.push(if shift { 'B' } else { 'b' }); changed = true; }
            KeyCode::KeyC => { out.push(if shift { 'C' } else { 'c' }); changed = true; }
            KeyCode::KeyD => { out.push(if shift { 'D' } else { 'd' }); changed = true; }
            KeyCode::KeyE => { out.push(if shift { 'E' } else { 'e' }); changed = true; }
            KeyCode::KeyF => { out.push(if shift { 'F' } else { 'f' }); changed = true; }
            KeyCode::KeyG => { out.push(if shift { 'G' } else { 'g' }); changed = true; }
            KeyCode::KeyH => { out.push(if shift { 'H' } else { 'h' }); changed = true; }
            KeyCode::KeyI => { out.push(if shift { 'I' } else { 'i' }); changed = true; }
            KeyCode::KeyJ => { out.push(if shift { 'J' } else { 'j' }); changed = true; }
            KeyCode::KeyK => { out.push(if shift { 'K' } else { 'k' }); changed = true; }
            KeyCode::KeyL => { out.push(if shift { 'L' } else { 'l' }); changed = true; }
            KeyCode::KeyM => { out.push(if shift { 'M' } else { 'm' }); changed = true; }
            KeyCode::KeyN => { out.push(if shift { 'N' } else { 'n' }); changed = true; }
            KeyCode::KeyO => { out.push(if shift { 'O' } else { 'o' }); changed = true; }
            KeyCode::KeyP => { out.push(if shift { 'P' } else { 'p' }); changed = true; }
            KeyCode::KeyQ => { out.push(if shift { 'Q' } else { 'q' }); changed = true; }
            KeyCode::KeyR => { out.push(if shift { 'R' } else { 'r' }); changed = true; }
            KeyCode::KeyS => { out.push(if shift { 'S' } else { 's' }); changed = true; }
            KeyCode::KeyT => { out.push(if shift { 'T' } else { 't' }); changed = true; }
            KeyCode::KeyU => { out.push(if shift { 'U' } else { 'u' }); changed = true; }
            KeyCode::KeyV => { out.push(if shift { 'V' } else { 'v' }); changed = true; }
            KeyCode::KeyW => { out.push(if shift { 'W' } else { 'w' }); changed = true; }
            KeyCode::KeyX => { out.push(if shift { 'X' } else { 'x' }); changed = true; }
            KeyCode::KeyY => { out.push(if shift { 'Y' } else { 'y' }); changed = true; }
            KeyCode::KeyZ => { out.push(if shift { 'Z' } else { 'z' }); changed = true; }

            // digits (+ shifted)
            KeyCode::Digit0 => { out.push(if shift { ')' } else { '0' }); changed = true; }
            KeyCode::Digit1 => { out.push(if shift { '!' } else { '1' }); changed = true; }
            KeyCode::Digit2 => { out.push(if shift { '@' } else { '2' }); changed = true; }
            KeyCode::Digit3 => { out.push(if shift { '#' } else { '3' }); changed = true; }
            KeyCode::Digit4 => { out.push(if shift { '$' } else { '4' }); changed = true; }
            KeyCode::Digit5 => { out.push(if shift { '%' } else { '5' }); changed = true; }
            KeyCode::Digit6 => { out.push(if shift { '^' } else { '6' }); changed = true; }
            KeyCode::Digit7 => { out.push(if shift { '&' } else { '7' }); changed = true; }
            KeyCode::Digit8 => { out.push(if shift { '*' } else { '8' }); changed = true; }
            KeyCode::Digit9 => { out.push(if shift { '(' } else { '9' }); changed = true; }

            KeyCode::Space => { out.push(' '); changed = true; }
            KeyCode::Minus => { out.push(if shift { '_' } else { '-' }); changed = true; }
            KeyCode::Equal => { out.push(if shift { '+' } else { '=' }); changed = true; }
            KeyCode::Comma => { out.push(if shift { '<' } else { ',' }); changed = true; }
            KeyCode::Period => { out.push(if shift { '>' } else { '.' }); changed = true; }
            KeyCode::Slash => { out.push(if shift { '?' } else { '/' }); changed = true; }

            _ => {}
        }
    }

    changed
}

fn field_value(st: &BootState, field: InputFieldKind) -> String {
    match field {
        InputFieldKind::Email => st.email.clone(),
        InputFieldKind::Password => "*".repeat(st.password.len()),
        InputFieldKind::DisplayName => st.display_name.clone(),
        InputFieldKind::NewCharacterName => st.new_character_name.clone(),
    }
}

fn focus_to_field(field: InputFieldKind) -> FocusField {
    match field {
        InputFieldKind::Email => FocusField::Email,
        InputFieldKind::Password => FocusField::Password,
        InputFieldKind::DisplayName => FocusField::DisplayName,
        InputFieldKind::NewCharacterName => FocusField::NewCharacterName,
    }
}

// ============================================================
// MAIN MENU (professional layout + updates panel)
// ============================================================

#[derive(Clone, Copy)]
struct UpdateRow {
    when: &'static str,
    title: &'static str,
    status: &'static str,
}

fn spawn_updates_panel(commands: &mut Commands, root: Entity, fonts: &UiFonts) {
    // Right anchored panel
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

    // "table" header
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
                    height: Val::Auto,
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

    // List container (later: swap for real scroll)
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

        let add_cell = |commands: &mut Commands, row: Entity, fonts: &UiFonts, w: f32, txt: &str, mono: bool| {
            let cell = commands
                .spawn((
                    Node {
                        width: Val::Px(w),
                        height: Val::Auto,
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

pub fn main_menu_enter(mut commands: Commands, fonts: Res<UiFonts>, asset_server: Res<AssetServer>) {
    let root = spawn_root(&mut commands);

    // Optional PNG later:
    // spawn_background(&mut commands, root, &asset_server, Some("ui/main_menu_bg.png"));
    // For now: no image, just the root bg color.
    spawn_background(&mut commands, root, &asset_server, None);

    // Left container (center-left, not centered overall)
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
    commands.entity(panel).add_child(
        commands
            .spawn((
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Px(10.0),
                    ..default()
                },
                Name::new("spacer"),
            ))
            .id(),
    );

    spawn_button(
        &mut commands,
        panel,
        &fonts,
        "Account Login",
        ButtonAction::GoAccountLogin,
    );

    spawn_button(
        &mut commands,
        panel,
        &fonts,
        "Singleplayer",
        ButtonAction::GoSingleplayer,
    );

    // Right updates panel
    spawn_updates_panel(&mut commands, root, &fonts);
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
            ButtonAction::GoSingleplayer => {
                next.set(Screen::SingleplayerMenu);
            }
            _ => {}
        }
    }
}

// ============================================================
// ACCOUNT LOGIN / REGISTER
// ============================================================

pub fn login_enter(mut commands: Commands, fonts: Res<UiFonts>, st: Res<BootState>) {
    let root = spawn_root(&mut commands);
    let panel = spawn_panel(&mut commands, root, 720.0);

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
        spawn_button(
            &mut commands,
            panel,
            &fonts,
            "Switch to Login",
            ButtonAction::GoAccountLogin,
        );
    } else {
        spawn_button(
            &mut commands,
            panel,
            &fonts,
            "Switch to Register",
            ButtonAction::GoRegister,
        );
    }

    spawn_button(&mut commands, panel, &fonts, "Submit", ButtonAction::SubmitAuth);
    spawn_button(
        &mut commands,
        panel,
        &fonts,
        "Back",
        ButtonAction::BackToMainMenu,
    );
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

    match st.focus {
        FocusField::Email => {
            push_typed_keys(&keys, &mut st.email);
        }
        FocusField::Password => {
            push_typed_keys(&keys, &mut st.password);
        }
        FocusField::DisplayName => {
            push_typed_keys(&keys, &mut st.display_name);
        }
        _ => {}
    }

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

    if st.session.is_some() {
        st.busy = false;
        next.set(Screen::MainMenu);
    }

    if let Some(mut t) = q_error.iter_mut().next() {
        t.0 = st.error_banner.clone().unwrap_or_default();
    }

    for (mut t, tag) in &mut q_inputs {
        t.0 = field_value(&st, tag.field);
    }

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

// ============================================================
// SINGLEPLAYER MENU (PLACEHOLDER)
// ============================================================

pub fn singleplayer_enter(mut commands: Commands, fonts: Res<UiFonts>) {
    let root = spawn_root(&mut commands);
    let panel = spawn_panel(&mut commands, root, 720.0);

    spawn_title(&mut commands, panel, &fonts, "Singleplayer", 32.0);
    spawn_text(&mut commands, panel, &fonts, "Placeholder menu.", 14.0);

    spawn_button(
        &mut commands,
        panel,
        &fonts,
        "Character Selection",
        ButtonAction::GoCharacterSelect,
    );

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

// ============================================================
// CHARACTER SELECT
// ============================================================

pub fn character_select_enter(mut commands: Commands, fonts: Res<UiFonts>, st: Res<BootState>) {
    let root = spawn_root(&mut commands);
    let panel = spawn_panel(&mut commands, root, 720.0);

    spawn_title(&mut commands, panel, &fonts, "Character Select", 32.0);
    spawn_button(
        &mut commands,
        panel,
        &fonts,
        "Refresh",
        ButtonAction::RefreshCharacters,
    );

    for idx in 0..5 {
        let slot = commands
            .spawn((
                Text::new(String::new()),
                TextFont {
                    font: fonts.mono.clone(),
                    font_size: 18.0,
                    ..default()
                },
                TextColor(Color::WHITE),
                LineHeight::default(),
                SlotText { idx },
            ))
            .id();
        commands.entity(panel).add_child(slot);

        if let Some(c) = &st.slots[idx] {
            spawn_button(
                &mut commands,
                panel,
                &fonts,
                "Play",
                ButtonAction::PlayCharacter(c.character_id),
            );
            spawn_button(
                &mut commands,
                panel,
                &fonts,
                "Delete",
                ButtonAction::DeleteCharacter(c.character_id),
            );
        }
    }

    spawn_text(&mut commands, panel, &fonts, "New character name:", 16.0);
    spawn_input_row(
        &mut commands,
        panel,
        &fonts,
        "Name",
        &field_value(&st, InputFieldKind::NewCharacterName),
        st.focus == FocusField::NewCharacterName,
        InputFieldKind::NewCharacterName,
    );

    spawn_button(
        &mut commands,
        panel,
        &fonts,
        "Create Character",
        ButtonAction::CreateCharacter,
    );

    spawn_button(&mut commands, panel, &fonts, "Back", ButtonAction::BackToSingleplayer);
}

pub fn character_select_update(
    mut next: ResMut<NextState<Screen>>,
    mut st: ResMut<BootState>,
    keys: Res<ButtonInput<KeyCode>>,
    mut q_btn: Query<(&Interaction, &ButtonAction), (Changed<Interaction>, With<Button>)>,
    net_runtime: Res<net::NetRuntime>,
    mut q_inputs: Query<(&mut Text, &InputValueText), Without<SlotText>>,
    mut q_slot_text: Query<(&mut Text, &SlotText), Without<InputValueText>>,
    mut q_row_bg: Query<(&mut BackgroundColor, &ButtonAction), With<Button>>,
) {
    for (i, a) in &mut q_btn {
        if *i != Interaction::Pressed {
            continue;
        }
        match a {
            ButtonAction::BackToSingleplayer => next.set(Screen::SingleplayerMenu),

            ButtonAction::RefreshCharacters => {
                if let Some(sess) = &st.session {
                    net::spawn_list_characters(&st, net_runtime.as_ref(), sess.token.clone());
                }
            }

            ButtonAction::CreateCharacter => {
                if let Some(sess) = &st.session {
                    let name = st.new_character_name.trim().to_string();
                    if !name.is_empty() {
                        net::spawn_create_character(
                            &st,
                            net_runtime.as_ref(),
                            sess.token.clone(),
                            name,
                        );
                    }
                }
            }

            ButtonAction::DeleteCharacter(id) => {
                if let Some(sess) = &st.session {
                    net::spawn_delete_character(&st, net_runtime.as_ref(), sess.token.clone(), *id);
                }
            }

            ButtonAction::PlayCharacter(id) => {
                st.pending_start_world = Some(*id);
                next.set(Screen::InWorld);
            }

            ButtonAction::Focus(field) => {
                st.focus = focus_to_field(*field);
            }

            _ => {}
        }
    }

    if st.focus == FocusField::NewCharacterName {
        push_typed_keys(&keys, &mut st.new_character_name);
    }

    if keys.just_pressed(KeyCode::Enter) && st.focus == FocusField::NewCharacterName {
        if let Some(sess) = &st.session {
            let name = st.new_character_name.trim().to_string();
            if !name.is_empty() {
                net::spawn_create_character(&st, net_runtime.as_ref(), sess.token.clone(), name);
            }
        }
    }

    for (mut t, tag) in &mut q_inputs {
        t.0 = field_value(&st, tag.field);
    }

    for (mut t, tag) in &mut q_slot_text {
        let idx = tag.idx;
        t.0 = if let Some(c) = &st.slots[idx] {
            format!("Slot {}: {} (${:.2})", idx + 1, c.name, c.cash)
        } else {
            format!("Slot {}: <empty>", idx + 1)
        };
    }

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

// =======================
// IN WORLD (BOOT UI SHOULD DO NOTHING)
// =======================

pub fn in_world_enter() {
    // Intentionally empty.
    // In-world UI is owned by stonepyre_ui + engine.
}