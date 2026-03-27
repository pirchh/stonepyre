use bevy::prelude::*;
use bevy::text::LineHeight;

use super::common::*;
use super::super::fonts::UiFonts;
use super::super::net;
use super::super::state::{BootState, FocusField, Screen};

// ✅ Skin “folder ids” under assets/characters/humanoid/
const SKINS: [&str; 1] = [
    "base_greyscale",
    // later: "1_white", "2_white", "3_white", "1_black", "2_black", "1_brown", ...
];

#[derive(Component)]
pub(crate) struct SkinLabel;

#[derive(Component)]
pub(crate) struct PedestalSlot {
    pub idx: usize,
}

#[derive(Component)]
pub(crate) struct PedestalTitle {
    pub idx: usize,
}

#[derive(Component)]
pub(crate) struct SlotPreviewImage {
    pub idx: usize,
}

#[derive(Component)]
pub(crate) struct SlotEmptyCta {
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

fn spawn_small_button(
    commands: &mut Commands,
    parent: Entity,
    fonts: &UiFonts,
    label: &str,
    action: ButtonAction,
    w: f32,
) -> Entity {
    let btn = commands
        .spawn((
            Button,
            Node {
                width: Val::Px(w),
                height: Val::Px(38.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgb(0.18, 0.18, 0.24)),
            action,
            Name::new("small_button"),
        ))
        .id();

    commands.entity(parent).add_child(btn);
    spawn_text(commands, btn, fonts, label, 16.0);
    btn
}

fn spawn_pedestal(
    commands: &mut Commands,
    parent: Entity,
    fonts: &UiFonts,
    asset_server: &AssetServer,
    st: &BootState,
    idx: usize,
) -> Entity {
    let selected = st.selected_slot == idx;
    let bg = if selected {
        Color::srgba(0.12, 0.12, 0.16, 0.96)
    } else {
        Color::srgba(0.08, 0.08, 0.10, 0.92)
    };

    // Clickable card
    let card = commands
        .spawn((
            Button,
            Node {
                width: Val::Px(250.0),
                height: Val::Px(520.0),
                padding: UiRect::all(Val::Px(16.0)),
                display: Display::Flex,
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::FlexStart,
                row_gap: Val::Px(10.0),
                ..default()
            },
            BackgroundColor(bg),
            PedestalSlot { idx },
            ButtonAction::SelectSlot(idx),
            Name::new("pedestal_card"),
        ))
        .id();
    commands.entity(parent).add_child(card);

    // Title line (updates)
    let title = commands
        .spawn((
            Text::new(""),
            TextFont {
                font: fonts.mono.clone(),
                font_size: 16.0,
                ..default()
            },
            TextColor(Color::WHITE),
            LineHeight::default(),
            PedestalTitle { idx },
            Name::new("pedestal_title"),
        ))
        .id();
    commands.entity(card).add_child(title);

    // Preview area
    let preview_wrap = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(280.0),
                display: Display::Flex,
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.02, 0.02, 0.03, 0.35)),
            Name::new("pedestal_preview_wrap"),
        ))
        .id();
    commands.entity(card).add_child(preview_wrap);

    // Preview image (exists, but we toggle display based on slot occupancy)
    let img_path = st.selected_skin_preview_path();
    let img: Handle<Image> = asset_server.load(img_path);
    let preview = commands
        .spawn((
            Node {
                width: Val::Px(180.0),
                height: Val::Px(180.0),
                ..default()
            },
            ImageNode::new(img),
            SlotPreviewImage { idx },
            Name::new("pedestal_preview_image"),
        ))
        .id();
    commands.entity(preview_wrap).add_child(preview);

    // Empty CTA (exists, but we toggle display based on slot occupancy)
    let cta = commands
        .spawn((
            Text::new("+ Create Character"),
            TextFont {
                font: fonts.accent.clone(),
                font_size: 22.0,
                ..default()
            },
            TextColor(Color::srgba(0.90, 0.90, 0.98, 1.0)),
            LineHeight::default(),
            SlotEmptyCta { idx },
            Name::new("slot_empty_cta"),
        ))
        .id();
    commands.entity(preview_wrap).add_child(cta);

    // Footer
    let footer = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Auto,
                display: Display::Flex,
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(8.0),
                ..default()
            },
            Name::new("pedestal_footer"),
        ))
        .id();
    commands.entity(card).add_child(footer);

    let hint = commands
        .spawn((
            Text::new("Click to select"),
            TextFont {
                font: fonts.regular.clone(),
                font_size: 13.0,
                ..default()
            },
            TextColor(Color::srgba(0.78, 0.78, 0.86, 1.0)),
            LineHeight::default(),
            Name::new("pedestal_hint"),
        ))
        .id();
    commands.entity(footer).add_child(hint);

    card
}

pub fn character_select_enter(
    mut commands: Commands,
    fonts: Res<UiFonts>,
    asset_server: Res<AssetServer>,
    mut st: ResMut<BootState>,
    net_runtime: Res<net::NetRuntime>,
) {
    st.clamp_selected_slot();

    let root = spawn_root(&mut commands);

    // ✅ Auto-fetch characters immediately on enter
    if let Some(sess) = &st.session {
        net::spawn_list_characters(&st, net_runtime.as_ref(), sess.token.clone());
    }

    // Fullscreen center container
    let container = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            Name::new("character_select_container"),
        ))
        .id();
    commands.entity(root).add_child(container);

    // Wider panel to host 5 pedestals
    let panel = spawn_panel(&mut commands, container, 1400.0);
    spawn_title(&mut commands, panel, &fonts, "Character Select", 32.0);

    // Top row
    let top_row = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Auto,
                display: Display::Flex,
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                ..default()
            },
            Name::new("charselect_top_row"),
        ))
        .id();
    commands.entity(panel).add_child(top_row);

    // Left controls
    {
        let left = commands
            .spawn((
                Node {
                    width: Val::Px(240.0),
                    height: Val::Auto,
                    ..default()
                },
                Name::new("charselect_top_left"),
            ))
            .id();
        commands.entity(top_row).add_child(left);

        spawn_button(&mut commands, left, &fonts, "Refresh", ButtonAction::RefreshCharacters);
    }

    // Right skin selector
    {
        let right = commands
            .spawn((
                Node {
                    width: Val::Px(520.0),
                    height: Val::Auto,
                    display: Display::Flex,
                    flex_direction: FlexDirection::Row,
                    justify_content: JustifyContent::FlexEnd,
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(10.0),
                    ..default()
                },
                Name::new("skin_selector_row"),
            ))
            .id();
        commands.entity(top_row).add_child(right);

        spawn_mono(&mut commands, right, &fonts, "Skin:", 16.0);
        spawn_small_button(&mut commands, right, &fonts, "<", ButtonAction::PrevSkin, 44.0);

        let label = commands
            .spawn((
                Text::new(st.new_character_skin.clone()),
                TextFont {
                    font: fonts.mono.clone(),
                    font_size: 16.0,
                    ..default()
                },
                TextColor(Color::WHITE),
                LineHeight::default(),
                SkinLabel,
            ))
            .id();
        commands.entity(right).add_child(label);

        spawn_small_button(&mut commands, right, &fonts, ">", ButtonAction::NextSkin, 44.0);
    }

    // Pedestal row
    let ped_row = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Auto,
                display: Display::Flex,
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::FlexStart,
                column_gap: Val::Px(16.0),
                padding: UiRect::top(Val::Px(12.0)),
                ..default()
            },
            Name::new("pedestal_row"),
        ))
        .id();
    commands.entity(panel).add_child(ped_row);

    for idx in 0..5 {
        spawn_pedestal(&mut commands, ped_row, &fonts, &asset_server, &st, idx);
    }

    // Name input
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

    // Action row
    let action_row = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Auto,
                display: Display::Flex,
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                column_gap: Val::Px(12.0),
                padding: UiRect::top(Val::Px(8.0)),
                ..default()
            },
            Name::new("charselect_action_row"),
        ))
        .id();
    commands.entity(panel).add_child(action_row);

    // Left: Play/Delete
    let left_actions = commands
        .spawn((
            Node {
                width: Val::Px(420.0),
                height: Val::Auto,
                display: Display::Flex,
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::FlexStart,
                align_items: AlignItems::Center,
                column_gap: Val::Px(12.0),
                ..default()
            },
            Name::new("charselect_left_actions"),
        ))
        .id();
    commands.entity(action_row).add_child(left_actions);

    let play_btn = spawn_small_button(
        &mut commands,
        left_actions,
        &fonts,
        "Play",
        ButtonAction::PlayCharacter(uuid::Uuid::nil()),
        140.0,
    );
    commands.entity(play_btn).insert(ActionPlayBtn);

    let delete_btn = spawn_small_button(
        &mut commands,
        left_actions,
        &fonts,
        "Delete",
        ButtonAction::DeleteCharacter(uuid::Uuid::nil()),
        140.0,
    );
    commands.entity(delete_btn).insert(ActionDeleteBtn);

    // Middle: Create
    let create_wrap = commands
        .spawn((
            Node {
                width: Val::Px(420.0),
                height: Val::Auto,
                display: Display::Flex,
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            Name::new("charselect_create_wrap"),
        ))
        .id();
    commands.entity(action_row).add_child(create_wrap);

    let create_btn = spawn_small_button(
        &mut commands,
        create_wrap,
        &fonts,
        "Create Character",
        ButtonAction::CreateCharacter,
        220.0,
    );
    commands.entity(create_btn).insert(ActionCreateBtn);

    // Right: Back
    let right_actions = commands
        .spawn((
            Node {
                width: Val::Px(260.0),
                height: Val::Auto,
                display: Display::Flex,
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::FlexEnd,
                align_items: AlignItems::Center,
                ..default()
            },
            Name::new("charselect_right_actions"),
        ))
        .id();
    commands.entity(action_row).add_child(right_actions);

    spawn_small_button(
        &mut commands,
        right_actions,
        &fonts,
        "Back",
        ButtonAction::BackToMainMenuFromCharSelect,
        140.0,
    );
}

pub fn character_select_update(
    mut next: ResMut<NextState<Screen>>,
    mut st: ResMut<BootState>,
    keys: Res<ButtonInput<KeyCode>>,
    mut q_btn: Query<
        (&Interaction, &ButtonAction),
        (
            Changed<Interaction>,
            With<Button>,
            Without<ActionPlayBtn>,
            Without<ActionDeleteBtn>,
            Without<ActionCreateBtn>,
        ),
    >,
    net_runtime: Res<net::NetRuntime>,
    asset_server: Res<AssetServer>,

    // Input value text (Name field)
    mut q_inputs: Query<(&mut Text, &InputValueText), (Without<SkinLabel>, Without<PedestalTitle>)>,

    // Skin label text
    mut q_skin_label: Query<
        &mut Text,
        (With<SkinLabel>, Without<InputValueText>, Without<PedestalTitle>),
    >,

    // Pedestal titles
    mut q_ped_titles: Query<(&mut Text, &PedestalTitle), (Without<InputValueText>, Without<SkinLabel>)>,

    // Pedestal backgrounds
    mut q_slot_bg: Query<(&PedestalSlot, &mut BackgroundColor)>,

    // 🔥 Fix B0001: all the Node-mut stuff lives in a ParamSet
    mut ui: ParamSet<(
        Query<(&SlotPreviewImage, &mut ImageNode, &mut Node)>,
        Query<(&SlotEmptyCta, &mut TextColor, &mut Node)>,
        Query<&mut Node, With<ActionCreateBtn>>,
        Query<(&mut Node, &mut ButtonAction), With<ActionPlayBtn>>,
        Query<(&mut Node, &mut ButtonAction), With<ActionDeleteBtn>>,
    )>,

    // Focus border coloring for input rows
    mut q_row_bg: Query<
        (&mut BackgroundColor, &ButtonAction),
        (With<Button>, Without<PedestalSlot>, Without<ActionPlayBtn>, Without<ActionDeleteBtn>, Without<ActionCreateBtn>),
    >,
) {
    st.clamp_selected_slot();

    for (i, a) in &mut q_btn {
        if *i != Interaction::Pressed {
            continue;
        }

        match *a {
            ButtonAction::BackToMainMenuFromCharSelect => {
                next.set(Screen::MainMenu);
            }

            ButtonAction::RefreshCharacters => {
                if let Some(sess) = &st.session {
                    net::spawn_list_characters(&st, net_runtime.as_ref(), sess.token.clone());
                }
            }

            ButtonAction::SelectSlot(idx) => {
                st.selected_slot = idx.min(st.slots.len().saturating_sub(1));
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
                }
            }

            ButtonAction::DeleteCharacter(id) => {
                if let Some(sess) = &st.session {
                    net::spawn_delete_character(&st, net_runtime.as_ref(), sess.token.clone(), id);
                }
            }

            ButtonAction::PlayCharacter(id) => {
                st.pending_start_world = Some(id);
                next.set(Screen::InWorld);
            }

            ButtonAction::Focus(field) => {
                st.focus = focus_to_field(field);
            }

            _ => {}
        }
    }

    // Keyboard typing for name entry
    if st.focus == FocusField::NewCharacterName {
        push_typed_keys(&keys, &mut st.new_character_name);
    }

    if keys.just_pressed(KeyCode::Enter) && st.focus == FocusField::NewCharacterName {
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
            }
        }
    }

    // Update input field text
    for (mut t, tag) in &mut q_inputs {
        t.0 = field_value(&st, tag.field);
    }

    // Update focus border coloring
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

    // Update skin label (0.18 safe)
    if let Some(mut label) = q_skin_label.iter_mut().next() {
        label.0 = st.new_character_skin.clone();
    }

    // Update pedestal titles
    for (mut t, tag) in &mut q_ped_titles {
        let idx = tag.idx;
        t.0 = if let Some(c) = &st.slots[idx] {
            format!("Slot {} • {}  (${:.2})", idx + 1, c.name, c.cash)
        } else {
            format!("Slot {} • <empty>", idx + 1)
        };
    }

    // Highlight selected slot background
    for (slot, mut bg) in &mut q_slot_bg {
        let selected = st.selected_slot == slot.idx;
        let col = if selected {
            Color::srgba(0.12, 0.12, 0.16, 0.96)
        } else {
            Color::srgba(0.08, 0.08, 0.10, 0.92)
        };
        *bg = BackgroundColor(col);
    }

    // Show/hide preview vs CTA based on occupancy
    let preview_path = st.selected_skin_preview_path();
    let img: Handle<Image> = asset_server.load(preview_path);

    // ui.p0: slot preview
    {
        let mut q = ui.p0();
        for (tag, mut node, mut style) in &mut q {
            let occupied = st.slots[tag.idx].is_some();
            style.display = if occupied { Display::Flex } else { Display::None };
            if occupied {
                *node = ImageNode::new(img.clone());
            }
        }
    }

    // ui.p1: slot CTA
    {
        let mut q = ui.p1();
        for (tag, mut color, mut style) in &mut q {
            let occupied = st.slots[tag.idx].is_some();
            style.display = if occupied { Display::None } else { Display::Flex };

            let selected = st.selected_slot == tag.idx;
            *color = if selected {
                TextColor(Color::srgba(0.95, 0.95, 1.0, 1.0))
            } else {
                TextColor(Color::srgba(0.75, 0.75, 0.86, 1.0))
            };
        }
    }

    // Action bar toggling (ONLY selected slot)
    let selected_idx = st.selected_slot;
    let selected_char = st.slots[selected_idx].as_ref();

    // Create visible iff selected is empty
    {
        let mut q = ui.p2();
        if let Some(mut create_style) = q.iter_mut().next() {
            create_style.display = if selected_char.is_none() {
                Display::Flex
            } else {
                Display::None
            };
        }
    }

    // Play visible iff selected is occupied, and update UUID action
    {
        let mut q = ui.p3();
        if let Some((mut play_style, mut play_action)) = q.iter_mut().next() {
            if let Some(c) = selected_char {
                play_style.display = Display::Flex;
                *play_action = ButtonAction::PlayCharacter(c.character_id);
            } else {
                play_style.display = Display::None;
            }
        }
    }

    // Delete visible iff selected is occupied, and update UUID action
    {
        let mut q = ui.p4();
        if let Some((mut del_style, mut del_action)) = q.iter_mut().next() {
            if let Some(c) = selected_char {
                del_style.display = Display::Flex;
                *del_action = ButtonAction::DeleteCharacter(c.character_id);
            } else {
                del_style.display = Display::None;
            }
        }
    }
}