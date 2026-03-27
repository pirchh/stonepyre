use bevy::prelude::*;
use bevy::text::LineHeight;
use uuid::Uuid;

use super::super::fonts::UiFonts;
use super::super::state::{BootState, FocusField};

// =======================
// Shared UI Components
// =======================

#[derive(Component)]
pub(crate) struct ScreenRoot;

#[derive(Component)]
pub(crate) struct ErrorBannerText;

#[derive(Component, Clone, Copy)]
pub(crate) enum InputFieldKind {
    Email,
    Password,
    DisplayName,
    NewCharacterName,
}

#[derive(Component)]
pub(crate) struct InputValueText {
    pub(crate) field: InputFieldKind,
}

#[derive(Component)]
pub(crate) struct SlotText {
    pub(crate) idx: usize,
}

#[derive(Component)]
pub(crate) enum ButtonAction {
    // Main Menu
    GoAccountLogin,
    GoCharacterSelect,

    // Account Login
    GoRegister,
    SubmitAuth,
    BackToMainMenu,

    // Main Menu actions (future)
    Logout,
    DeleteAccount,

    // Character Select
    RefreshCharacters,
    CreateCharacter,
    DeleteCharacter(Uuid),
    PlayCharacter(Uuid),

    PrevSkin,
    NextSkin,

    BackToMainMenuFromCharSelect,

    // NEW: click a pedestal slot
    SelectSlot(usize),

    Focus(InputFieldKind),
}

// ============================================================
// Root / Common Widgets
// ============================================================

pub(super) fn spawn_root(commands: &mut Commands) -> Entity {
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

/// Optional fullscreen background image.
pub(super) fn spawn_background(
    commands: &mut Commands,
    root: Entity,
    asset_server: &AssetServer,
    path: Option<&'static str>,
) {
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

pub(super) fn spawn_panel(commands: &mut Commands, parent: Entity, w: f32) -> Entity {
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

pub(super) fn spawn_panel_fixed(commands: &mut Commands, parent: Entity, w: f32, h: f32) -> Entity {
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

pub(super) fn spawn_text(
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

pub(super) fn spawn_title(
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

pub(super) fn spawn_mono(
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

pub(super) fn spawn_button(
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

pub(super) fn spawn_input_row(
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

// ============================================================
// Input helpers
// ============================================================

fn is_shift_down(keys: &ButtonInput<KeyCode>) -> bool {
    keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight)
}

pub(super) fn push_typed_keys(keys: &ButtonInput<KeyCode>, out: &mut String) -> bool {
    let mut changed = false;
    let shift = is_shift_down(keys);

    for key in keys.get_just_pressed() {
        match key {
            KeyCode::Backspace => {
                out.pop();
                changed = true;
            }

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

pub(super) fn field_value(st: &BootState, field: InputFieldKind) -> String {
    match field {
        InputFieldKind::Email => st.email.clone(),
        InputFieldKind::Password => "*".repeat(st.password.len()),
        InputFieldKind::DisplayName => st.display_name.clone(),
        InputFieldKind::NewCharacterName => st.new_character_name.clone(),
    }
}

pub(super) fn focus_to_field(field: InputFieldKind) -> FocusField {
    match field {
        InputFieldKind::Email => FocusField::Email,
        InputFieldKind::Password => FocusField::Password,
        InputFieldKind::DisplayName => FocusField::DisplayName,
        InputFieldKind::NewCharacterName => FocusField::NewCharacterName,
    }
}