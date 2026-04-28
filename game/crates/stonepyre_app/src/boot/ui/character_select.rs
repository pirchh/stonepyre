use bevy::prelude::*;
use bevy::text::LineHeight;

use super::common::*;
use super::super::fonts::UiFonts;
use super::super::net;
use super::super::state::{BootState, FocusField, Screen};

// Skin folder ids under assets/characters/humanoid/
const SKINS: [&str; 1] = [
    "base_greyscale",
    // later: "1_white", "2_white", "3_white", "1_black", "2_black", "1_brown", ...
];

const PANEL_BG: Color = Color::srgba(0.045, 0.045, 0.055, 0.96);
const PANEL_BORDER: Color = Color::srgba(0.13, 0.12, 0.11, 1.0);
const CARD_BG: Color = Color::srgba(0.065, 0.065, 0.078, 0.96);
const CARD_BG_SELECTED: Color = Color::srgba(0.18, 0.14, 0.055, 0.98);
const CARD_BORDER: Color = Color::srgba(0.18, 0.18, 0.22, 1.0);
const CARD_BORDER_SELECTED: Color = Color::srgba(0.75, 0.55, 0.18, 1.0);
const BUTTON_BG: Color = Color::srgba(0.15, 0.15, 0.20, 1.0);
const BUTTON_GOLD: Color = Color::srgba(0.42, 0.29, 0.08, 1.0);
const BUTTON_RED: Color = Color::srgba(0.36, 0.10, 0.08, 1.0);
const TEXT_MUTED: Color = Color::srgba(0.72, 0.72, 0.80, 1.0);
const TEXT_GOLD: Color = Color::srgba(0.98, 0.76, 0.22, 1.0);

#[derive(Component)]
pub(crate) struct SkinLabel;

#[derive(Component)]
pub(crate) struct CharacterListPanel;

#[derive(Component)]
pub(crate) struct CharacterCreatePanel;

#[derive(Component)]
pub(crate) struct SelectedCharacterPreview;

#[derive(Component)]
pub(crate) struct SelectedCharacterName;

#[derive(Component)]
pub(crate) struct SelectedCharacterMeta;

#[derive(Component)]
pub(crate) struct SlotRow {
    pub idx: usize,
}

#[derive(Component)]
pub(crate) struct SlotTitle {
    pub idx: usize,
}

#[derive(Component)]
pub(crate) struct SlotSubtitle {
    pub idx: usize,
}

#[derive(Component)]
pub(crate) struct ActionCreateBtn;

#[derive(Component)]
pub(crate) struct ActionPlayBtn;

#[derive(Component)]
pub(crate) struct ActionDeleteBtn;

fn skin_index_of(current: &str) -> usize {
    SKINS.iter().position(|s| *s == current).unwrap_or(0)
}

fn set_skin(st: &mut BootState, idx: usize) {
    let i = idx % SKINS.len().max(1);
    st.new_character_skin = SKINS[i].to_string();
}

fn button_radius() -> BorderRadius {
    BorderRadius {
        top_left: Val::Px(8.0),
        top_right: Val::Px(8.0),
        bottom_left: Val::Px(8.0),
        bottom_right: Val::Px(8.0),
    }
}

fn card_radius() -> BorderRadius {
    BorderRadius {
        top_left: Val::Px(14.0),
        top_right: Val::Px(14.0),
        bottom_left: Val::Px(14.0),
        bottom_right: Val::Px(14.0),
    }
}

fn spawn_label(
    commands: &mut Commands,
    parent: Entity,
    font: Handle<Font>,
    text: impl Into<String>,
    size: f32,
    color: Color,
) -> Entity {
    let e = commands
        .spawn((
            Text::new(text.into()),
            TextFont {
                font,
                font_size: size,
                ..default()
            },
            TextColor(color),
            LineHeight::default(),
        ))
        .id();

    commands.entity(parent).add_child(e);
    e
}

fn spawn_action_button(
    commands: &mut Commands,
    parent: Entity,
    fonts: &UiFonts,
    label: &str,
    action: ButtonAction,
    width: f32,
    color: Color,
) -> Entity {
    let btn = commands
        .spawn((
            Button,
            Node {
                width: Val::Px(width),
                height: Val::Px(44.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                border_radius: button_radius(),
                ..default()
            },
            BackgroundColor(color),
            action,
            Name::new(format!("charselect_button_{label}")),
        ))
        .id();

    commands.entity(parent).add_child(btn);
    spawn_label(
        commands,
        btn,
        fonts.regular.clone(),
        label,
        16.0,
        Color::WHITE,
    );

    btn
}

fn spawn_slot_row(
    commands: &mut Commands,
    parent: Entity,
    fonts: &UiFonts,
    st: &BootState,
    idx: usize,
) -> Entity {
    let selected = st.selected_slot == idx;
    let row_bg = if selected { CARD_BG_SELECTED } else { CARD_BG };
    let border_bg = if selected { CARD_BORDER_SELECTED } else { CARD_BORDER };

    let outer = commands
        .spawn((
            Button,
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(74.0),
                padding: UiRect::all(Val::Px(2.0)),
                border_radius: card_radius(),
                ..default()
            },
            BackgroundColor(border_bg),
            ButtonAction::SelectSlot(idx),
            SlotRow { idx },
            Name::new(format!("character_slot_row_{idx}")),
        ))
        .id();

    commands.entity(parent).add_child(outer);

    let inner = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                padding: UiRect::all(Val::Px(12.0)),
                display: Display::Flex,
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(12.0),
                border_radius: card_radius(),
                ..default()
            },
            BackgroundColor(row_bg),
        ))
        .id();

    commands.entity(outer).add_child(inner);

    let portrait = commands
        .spawn((
            Node {
                width: Val::Px(44.0),
                height: Val::Px(50.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                border_radius: button_radius(),
                ..default()
            },
            BackgroundColor(Color::srgba(0.02, 0.02, 0.025, 0.95)),
        ))
        .id();

    commands.entity(inner).add_child(portrait);

    spawn_label(
        commands,
        portrait,
        fonts.accent.clone(),
        if st.slots[idx].is_some() { "◆" } else { "+" },
        24.0,
        if st.slots[idx].is_some() { TEXT_GOLD } else { TEXT_MUTED },
    );

    let text_col = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            height: Val::Auto,
            display: Display::Flex,
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(4.0),
            ..default()
        })
        .id();

    commands.entity(inner).add_child(text_col);

    let title = if let Some(c) = &st.slots[idx] {
        c.name.clone()
    } else {
        format!("Empty Slot {}", idx + 1)
    };

    spawn_label(
        commands,
        text_col,
        fonts.accent.clone(),
        title,
        18.0,
        Color::WHITE,
    )
    .pipe(|e| {
        commands.entity(e).insert(SlotTitle { idx });
        e
    });

    let subtitle = if let Some(c) = &st.slots[idx] {
        format!("Cash ${:.2}", c.cash)
    } else {
        "Create a new character here".to_string()
    };

    spawn_label(
        commands,
        text_col,
        fonts.regular.clone(),
        subtitle,
        13.0,
        TEXT_MUTED,
    )
    .pipe(|e| {
        commands.entity(e).insert(SlotSubtitle { idx });
        e
    });

    let status = if selected { "Selected" } else { "Select" };
    spawn_label(
        commands,
        inner,
        fonts.mono.clone(),
        status,
        12.0,
        if selected { TEXT_GOLD } else { TEXT_MUTED },
    );

    outer
}

trait Pipe: Sized {
    fn pipe<R>(self, f: impl FnOnce(Self) -> R) -> R {
        f(self)
    }
}
impl<T> Pipe for T {}

pub fn character_select_enter(
    mut commands: Commands,
    fonts: Res<UiFonts>,
    asset_server: Res<AssetServer>,
    mut st: ResMut<BootState>,
    net_runtime: Res<net::NetRuntime>,
) {
    st.clamp_selected_slot();
    st.creating_character = false;
    st.focus = FocusField::NewCharacterName;

    let root = spawn_root(&mut commands);

    if let Some(sess) = &st.session {
        net::spawn_list_characters(&st, net_runtime.as_ref(), sess.token.clone());
    }

    let shell = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                display: Display::Flex,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                padding: UiRect::all(Val::Px(32.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.015, 0.015, 0.02, 1.0)),
            Name::new("character_select_shell"),
        ))
        .id();

    commands.entity(root).add_child(shell);

    let panel_outer = commands
        .spawn((
            Node {
                width: Val::Px(1340.0),
                height: Val::Px(840.0),
                padding: UiRect::all(Val::Px(2.0)),
                border_radius: BorderRadius {
                    top_left: Val::Px(20.0),
                    top_right: Val::Px(20.0),
                    bottom_left: Val::Px(20.0),
                    bottom_right: Val::Px(20.0),
                },
                ..default()
            },
            BackgroundColor(PANEL_BORDER),
            Name::new("character_select_panel_outer"),
        ))
        .id();

    commands.entity(shell).add_child(panel_outer);

    let panel = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                padding: UiRect::all(Val::Px(28.0)),
                display: Display::Flex,
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(22.0),
                border_radius: BorderRadius {
                    top_left: Val::Px(18.0),
                    top_right: Val::Px(18.0),
                    bottom_left: Val::Px(18.0),
                    bottom_right: Val::Px(18.0),
                },
                ..default()
            },
            BackgroundColor(PANEL_BG),
            Name::new("character_select_panel"),
        ))
        .id();

    commands.entity(panel_outer).add_child(panel);

    let header = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            height: Val::Px(72.0),
            display: Display::Flex,
            flex_direction: FlexDirection::Row,
            justify_content: JustifyContent::SpaceBetween,
            align_items: AlignItems::Center,
            ..default()
        })
        .id();

    commands.entity(panel).add_child(header);

    let heading = commands
        .spawn(Node {
            width: Val::Px(560.0),
            height: Val::Auto,
            display: Display::Flex,
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(6.0),
            ..default()
        })
        .id();

    commands.entity(header).add_child(heading);

    spawn_label(
        &mut commands,
        heading,
        fonts.accent.clone(),
        "Choose Your Character",
        34.0,
        Color::WHITE,
    );
    spawn_label(
        &mut commands,
        heading,
        fonts.regular.clone(),
        "Select a hero to enter Stonepyre, or create a new one from an empty slot.",
        14.0,
        TEXT_MUTED,
    );

    let header_actions = commands
        .spawn(Node {
            width: Val::Px(420.0),
            height: Val::Auto,
            display: Display::Flex,
            flex_direction: FlexDirection::Row,
            justify_content: JustifyContent::FlexEnd,
            align_items: AlignItems::Center,
            column_gap: Val::Px(10.0),
            ..default()
        })
        .id();

    commands.entity(header).add_child(header_actions);

    spawn_action_button(
        &mut commands,
        header_actions,
        &fonts,
        "Refresh",
        ButtonAction::RefreshCharacters,
        120.0,
        BUTTON_BG,
    );

    spawn_action_button(
        &mut commands,
        header_actions,
        &fonts,
        "Back",
        ButtonAction::BackToMainMenuFromCharSelect,
        110.0,
        BUTTON_BG,
    );

    let body = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            height: Val::Px(660.0),
            display: Display::Flex,
            flex_direction: FlexDirection::Row,
            column_gap: Val::Px(28.0),
            ..default()
        })
        .id();

    commands.entity(panel).add_child(body);

    // Left: large preview, kept at 2:3 aspect ratio to avoid squashing the 400x600 source.
    let preview_card = commands
        .spawn((
            Node {
                width: Val::Px(500.0),
                height: Val::Percent(100.0),
                padding: UiRect::all(Val::Px(18.0)),
                display: Display::Flex,
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::FlexStart,
                row_gap: Val::Px(18.0),
                border_radius: card_radius(),
                ..default()
            },
            BackgroundColor(Color::srgba(0.035, 0.035, 0.045, 0.98)),
            Name::new("selected_character_preview_card"),
        ))
        .id();

    commands.entity(body).add_child(preview_card);

    let preview_text = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            height: Val::Auto,
            display: Display::Flex,
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(6.0),
            ..default()
        })
        .id();

    commands.entity(preview_card).add_child(preview_text);

    spawn_label(
        &mut commands,
        preview_text,
        fonts.mono.clone(),
        "SELECTED CHARACTER",
        12.0,
        TEXT_GOLD,
    );

    let selected_name = spawn_label(
        &mut commands,
        preview_text,
        fonts.accent.clone(),
        "",
        28.0,
        Color::WHITE,
    );
    commands.entity(selected_name).insert(SelectedCharacterName);

    let selected_meta = spawn_label(
        &mut commands,
        preview_text,
        fonts.regular.clone(),
        "",
        14.0,
        TEXT_MUTED,
    );
    commands.entity(selected_meta).insert(SelectedCharacterMeta);

    let preview_frame = commands
        .spawn((
            Node {
                width: Val::Px(360.0),
                height: Val::Px(540.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                padding: UiRect::all(Val::Px(14.0)),
                border_radius: card_radius(),
                ..default()
            },
            BackgroundColor(Color::srgba(0.01, 0.01, 0.014, 0.98)),
            Name::new("selected_character_preview_frame"),
        ))
        .id();

    commands.entity(preview_card).add_child(preview_frame);

    let img = asset_server.load(st.selected_skin_preview_path());
    let preview_img = commands
        .spawn((
            Node {
                width: Val::Px(300.0),
                height: Val::Px(450.0),
                ..default()
            },
            ImageNode::new(img),
            SelectedCharacterPreview,
            Name::new("selected_character_preview_image"),
        ))
        .id();

    commands.entity(preview_frame).add_child(preview_img);


    // Right: Select mode panel.
    let select_panel = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                display: Display::Flex,
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(14.0),
                ..default()
            },
            CharacterListPanel,
            Name::new("character_list_panel"),
        ))
        .id();

    commands.entity(body).add_child(select_panel);

    let list_header = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            height: Val::Auto,
            display: Display::Flex,
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(4.0),
            ..default()
        })
        .id();

    commands.entity(select_panel).add_child(list_header);

    spawn_label(
        &mut commands,
        list_header,
        fonts.accent.clone(),
        "Saved Characters",
        24.0,
        Color::WHITE,
    );

    spawn_label(
        &mut commands,
        list_header,
        fonts.regular.clone(),
        "Pick a slot. Empty slots can be used to create a new character.",
        13.0,
        TEXT_MUTED,
    );

    let slot_list = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            height: Val::Auto,
            display: Display::Flex,
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(10.0),
            ..default()
        })
        .id();

    commands.entity(select_panel).add_child(slot_list);

    for idx in 0..5 {
        spawn_slot_row(&mut commands, slot_list, &fonts, &st, idx);
    }

    let select_spacer = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            height: Val::Auto,
            flex_grow: 1.0,
            ..default()
        })
        .id();

    commands.entity(select_panel).add_child(select_spacer);

    let select_actions = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(64.0),
                padding: UiRect::all(Val::Px(10.0)),
                display: Display::Flex,
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                column_gap: Val::Px(12.0),
                border_radius: card_radius(),
                ..default()
            },
            BackgroundColor(Color::srgba(0.035, 0.035, 0.045, 0.98)),
        ))
        .id();

    commands.entity(select_panel).add_child(select_actions);

    let create_btn = spawn_action_button(
        &mut commands,
        select_actions,
        &fonts,
        "Create New",
        ButtonAction::BeginCharacterCreate,
        160.0,
        BUTTON_GOLD,
    );
    commands.entity(create_btn).insert(ActionCreateBtn);

    let play_btn = spawn_action_button(
        &mut commands,
        select_actions,
        &fonts,
        "Enter World",
        ButtonAction::PlaySelectedCharacter,
        180.0,
        BUTTON_GOLD,
    );
    commands.entity(play_btn).insert(ActionPlayBtn);

    let delete_btn = spawn_action_button(
        &mut commands,
        select_actions,
        &fonts,
        "Delete",
        ButtonAction::DeleteSelectedCharacter,
        130.0,
        BUTTON_RED,
    );
    commands.entity(delete_btn).insert(ActionDeleteBtn);

    // Right: Create mode panel.
    let create_panel = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                display: Display::None,
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(16.0),
                ..default()
            },
            CharacterCreatePanel,
            Name::new("character_create_panel"),
        ))
        .id();

    commands.entity(body).add_child(create_panel);

    spawn_label(
        &mut commands,
        create_panel,
        fonts.accent.clone(),
        "Create Character",
        26.0,
        Color::WHITE,
    );

    spawn_label(
        &mut commands,
        create_panel,
        fonts.regular.clone(),
        "Tune the base skin, name the character, then create them in the selected empty slot.",
        14.0,
        TEXT_MUTED,
    );

    let skin_card = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(120.0),
                padding: UiRect::all(Val::Px(16.0)),
                display: Display::Flex,
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(12.0),
                border_radius: card_radius(),
                ..default()
            },
            BackgroundColor(CARD_BG),
        ))
        .id();

    commands.entity(create_panel).add_child(skin_card);

    spawn_label(
        &mut commands,
        skin_card,
        fonts.mono.clone(),
        "BASE SKIN",
        12.0,
        TEXT_GOLD,
    );

    let skin_row = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            height: Val::Px(48.0),
            display: Display::Flex,
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(12.0),
            ..default()
        })
        .id();

    commands.entity(skin_card).add_child(skin_row);

    spawn_action_button(
        &mut commands,
        skin_row,
        &fonts,
        "<",
        ButtonAction::PrevSkin,
        48.0,
        BUTTON_BG,
    );

    let skin_label_card = commands
        .spawn((
            Node {
                width: Val::Px(260.0),
                height: Val::Px(44.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                border_radius: button_radius(),
                ..default()
            },
            BackgroundColor(Color::srgba(0.02, 0.02, 0.03, 0.98)),
        ))
        .id();

    commands.entity(skin_row).add_child(skin_label_card);

    let skin_label = spawn_label(
        &mut commands,
        skin_label_card,
        fonts.mono.clone(),
        st.new_character_skin.clone(),
        15.0,
        Color::WHITE,
    );
    commands.entity(skin_label).insert(SkinLabel);

    spawn_action_button(
        &mut commands,
        skin_row,
        &fonts,
        ">",
        ButtonAction::NextSkin,
        48.0,
        BUTTON_BG,
    );

    let name_card = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(118.0),
                padding: UiRect::all(Val::Px(16.0)),
                display: Display::Flex,
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(10.0),
                border_radius: card_radius(),
                ..default()
            },
            BackgroundColor(CARD_BG),
        ))
        .id();

    commands.entity(create_panel).add_child(name_card);

    spawn_label(
        &mut commands,
        name_card,
        fonts.mono.clone(),
        "NAME",
        12.0,
        TEXT_GOLD,
    );

    spawn_input_row(
        &mut commands,
        name_card,
        &fonts,
        "Name",
        &field_value(&st, InputFieldKind::NewCharacterName),
        true,
        InputFieldKind::NewCharacterName,
    );

    let create_spacer = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            height: Val::Auto,
            flex_grow: 1.0,
            ..default()
        })
        .id();

    commands.entity(create_panel).add_child(create_spacer);

    let create_actions = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            height: Val::Px(56.0),
            display: Display::Flex,
            flex_direction: FlexDirection::Row,
            justify_content: JustifyContent::FlexEnd,
            align_items: AlignItems::Center,
            column_gap: Val::Px(12.0),
            ..default()
        })
        .id();

    commands.entity(create_panel).add_child(create_actions);

    spawn_action_button(
        &mut commands,
        create_actions,
        &fonts,
        "Cancel",
        ButtonAction::CancelCharacterCreate,
        130.0,
        BUTTON_BG,
    );

    spawn_action_button(
        &mut commands,
        create_actions,
        &fonts,
        "Create Character",
        ButtonAction::CreateCharacter,
        190.0,
        BUTTON_GOLD,
    );
}

pub fn character_select_update(
    mut next: ResMut<NextState<Screen>>,
    mut st: ResMut<BootState>,
    keys: Res<ButtonInput<KeyCode>>,
    mut q_btn: Query<(&Interaction, &ButtonAction), (Changed<Interaction>, With<Button>)>,
    net_runtime: Res<net::NetRuntime>,
    asset_server: Res<AssetServer>,

    mut q_inputs: Query<(&mut Text, &InputValueText), (Without<SkinLabel>, Without<SlotTitle>, Without<SlotSubtitle>, Without<SelectedCharacterName>, Without<SelectedCharacterMeta>)>,

    mut q_skin_label: Query<
        &mut Text,
        (With<SkinLabel>, Without<InputValueText>, Without<SlotTitle>, Without<SlotSubtitle>, Without<SelectedCharacterName>, Without<SelectedCharacterMeta>),
    >,

    mut q_slot_titles: Query<(&mut Text, &SlotTitle), (Without<InputValueText>, Without<SkinLabel>, Without<SlotSubtitle>, Without<SelectedCharacterName>, Without<SelectedCharacterMeta>)>,

    mut q_slot_subtitles: Query<(&mut Text, &SlotSubtitle), (Without<InputValueText>, Without<SkinLabel>, Without<SlotTitle>, Without<SelectedCharacterName>, Without<SelectedCharacterMeta>)>,

    mut q_slot_bg: Query<(&SlotRow, &mut BackgroundColor)>,

    mut q_preview_img: Query<&mut ImageNode, With<SelectedCharacterPreview>>,

    mut q_selected_name: Query<&mut Text, (With<SelectedCharacterName>, Without<SelectedCharacterMeta>, Without<SkinLabel>, Without<InputValueText>, Without<SlotTitle>, Without<SlotSubtitle>)>,

    mut q_selected_meta: Query<&mut Text, (With<SelectedCharacterMeta>, Without<SelectedCharacterName>, Without<SkinLabel>, Without<InputValueText>, Without<SlotTitle>, Without<SlotSubtitle>)>,

    mut ui: ParamSet<(
        Query<&mut Node, With<CharacterListPanel>>,
        Query<&mut Node, With<CharacterCreatePanel>>,
        Query<&mut Node, With<ActionCreateBtn>>,
        Query<&mut Node, With<ActionPlayBtn>>,
        Query<&mut Node, With<ActionDeleteBtn>>,
    )>,

    mut q_row_bg: Query<
        (&mut BackgroundColor, &ButtonAction),
        (With<Button>, Without<SlotRow>, Without<ActionCreateBtn>, Without<ActionPlayBtn>, Without<ActionDeleteBtn>),
    >,
) {
    st.clamp_selected_slot();

    for (interaction, action) in &mut q_btn {
        if *interaction != Interaction::Pressed {
            continue;
        }

        match *action {
            ButtonAction::BackToMainMenuFromCharSelect => {
                st.creating_character = false;
                next.set(Screen::MainMenu);
                return;
            }

            ButtonAction::RefreshCharacters => {
                if let Some(sess) = &st.session {
                    net::spawn_list_characters(&st, net_runtime.as_ref(), sess.token.clone());
                }
            }

            ButtonAction::SelectSlot(idx) => {
                st.selected_slot = idx.min(st.slots.len().saturating_sub(1));
            }

            ButtonAction::BeginCharacterCreate => {
                if st.slots[st.selected_slot].is_none() {
                    st.creating_character = true;
                    st.focus = FocusField::NewCharacterName;
                }
            }

            ButtonAction::CancelCharacterCreate => {
                st.creating_character = false;
            }

            ButtonAction::PrevSkin => {
                let cur = skin_index_of(&st.new_character_skin);
                let next_i = if cur == 0 { SKINS.len() - 1 } else { cur - 1 };
                set_skin(&mut st, next_i);
            }

            ButtonAction::NextSkin => {
                let cur = skin_index_of(&st.new_character_skin);
                set_skin(&mut st, cur + 1);
            }

            ButtonAction::CreateCharacter => {
                if let Some(sess) = &st.session {
                    let mut name = st.new_character_name.trim().to_string();
                    if name.is_empty() {
                        let used = st.slots.iter().filter(|s| s.is_some()).count();
                        name = format!("Adventurer{}", used + 1);
                    }

                    net::spawn_create_character(
                        &st,
                        net_runtime.as_ref(),
                        sess.token.clone(),
                        name,
                        st.new_character_skin.clone(),
                    );

                    st.creating_character = false;
                    st.new_character_name.clear();
                }
            }

            ButtonAction::DeleteSelectedCharacter => {
                if let (Some(sess), Some(c)) = (&st.session, st.slots[st.selected_slot].as_ref()) {
                    net::spawn_delete_character(&st, net_runtime.as_ref(), sess.token.clone(), c.character_id);
                }
            }

            ButtonAction::PlaySelectedCharacter => {
                if let Some(c) = st.slots[st.selected_slot].as_ref() {
                    st.pending_start_world = Some(c.character_id);
                    st.creating_character = false;
                    next.set(Screen::InWorld);
                    return;
                }
            }

            ButtonAction::Focus(field) => {
                st.focus = focus_to_field(field);
            }

            _ => {}
        }
    }

    if st.focus == FocusField::NewCharacterName {
        push_typed_keys(&keys, &mut st.new_character_name);
    }

    if keys.just_pressed(KeyCode::Enter) && st.creating_character {
        if let Some(sess) = &st.session {
            let name = st.new_character_name.trim().to_string();
            if !name.is_empty() {
                net::spawn_create_character(
                    &st,
                    net_runtime.as_ref(),
                    sess.token.clone(),
                    name,
                    st.new_character_skin.clone(),
                );
                st.creating_character = false;
                st.new_character_name.clear();
            }
        }
    }

    for (mut text, tag) in &mut q_inputs {
        text.0 = field_value(&st, tag.field);
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

    if let Some(mut label) = q_skin_label.iter_mut().next() {
        label.0 = st.new_character_skin.clone();
    }

    for (mut text, tag) in &mut q_slot_titles {
        text.0 = if let Some(c) = &st.slots[tag.idx] {
            c.name.clone()
        } else {
            format!("Empty Slot {}", tag.idx + 1)
        };
    }

    for (mut text, tag) in &mut q_slot_subtitles {
        text.0 = if let Some(c) = &st.slots[tag.idx] {
            format!("Cash ${:.2}", c.cash)
        } else {
            "Create a new character here".to_string()
        };
    }

    for (slot, mut bg) in &mut q_slot_bg {
        let selected = st.selected_slot == slot.idx;
        *bg = BackgroundColor(if selected { CARD_BORDER_SELECTED } else { CARD_BORDER });
    }

    let selected_char = st.slots[st.selected_slot].as_ref();
    let preview_path = st.selected_skin_preview_path();
    let img: Handle<Image> = asset_server.load(preview_path);

    if let Some(mut preview) = q_preview_img.iter_mut().next() {
        *preview = ImageNode::new(img);
    }

    if let Some(mut name) = q_selected_name.iter_mut().next() {
        name.0 = if st.creating_character {
            "New Character".to_string()
        } else if let Some(c) = selected_char {
            c.name.clone()
        } else {
            format!("Empty Slot {}", st.selected_slot + 1)
        };
    }

    for mut meta in &mut q_selected_meta {
        meta.0 = if st.creating_character {
            format!("Skin: {}", st.new_character_skin)
        } else if let Some(c) = selected_char {
            format!("Slot {} • Cash ${:.2}", st.selected_slot + 1, c.cash)
        } else {
            "Select Create New to begin.".to_string()
        };
    }

    {
        let mut q = ui.p0();
        for mut node in &mut q {
            node.display = if st.creating_character { Display::None } else { Display::Flex };
        }
    }

    {
        let mut q = ui.p1();
        for mut node in &mut q {
            node.display = if st.creating_character { Display::Flex } else { Display::None };
        }
    }

    {
        let mut q = ui.p2();
        for mut node in &mut q {
            node.display = if !st.creating_character && selected_char.is_none() {
                Display::Flex
            } else {
                Display::None
            };
        }
    }

    {
        let mut q = ui.p3();
        for mut node in &mut q {
            node.display = if !st.creating_character && selected_char.is_some() {
                Display::Flex
            } else {
                Display::None
            };
        }
    }

    {
        let mut q = ui.p4();
        for mut node in &mut q {
            node.display = if !st.creating_character && selected_char.is_some() {
                Display::Flex
            } else {
                Display::None
            };
        }
    }
}
