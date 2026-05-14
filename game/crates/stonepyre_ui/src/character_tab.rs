use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use stonepyre_content::default_item_defs;
use stonepyre_engine::plugins::interaction::WorldInteractionBlocker;
use stonepyre_engine::plugins::inventory::{Equipment, PlayerBagSlots};
use stonepyre_engine::plugins::world::Player;

use crate::bag::BagUiState;
use crate::character_state::CharacterUiState;

const PANEL_WIDTH: f32 = 320.0;
const PANEL_HEIGHT: f32 = 390.0;
const PANEL_PADDING: f32 = 10.0;
const PANEL_RIGHT: f32 = 10.0;
const PANEL_BOTTOM: f32 = 88.0;

const SLOT_SIZE: f32 = 56.0;
const SLOT_GAP: f32 = 5.0;
const EQUIP_AREA_HEIGHT: f32 = PANEL_HEIGHT - (PANEL_PADDING * 2.0);

#[derive(Component)]
pub(crate) struct CharacterTabRoot;

#[derive(Component)]
pub(crate) struct BagSlotButton {
    pub bag_slot: u8,
}

#[derive(Component)]
pub(crate) struct CharacterTabSlotLabel {
    slot_id: &'static str,
}

pub(crate) fn character_tab_panel_sync_system(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut state: ResMut<CharacterUiState>,
    mut bag_ui_state: ResMut<BagUiState>,
    mut blocker: ResMut<WorldInteractionBlocker>,
    windows: Query<&Window, With<PrimaryWindow>>,
    player_q: Query<&Equipment, With<Player>>,
    bag_slots: Res<PlayerBagSlots>,
    children_q: Query<&Children>,
    mut slot_label_q: Query<(&CharacterTabSlotLabel, &mut Text)>,
    mut bag_btn_q: Query<(&BagSlotButton, &mut BackgroundColor, &Interaction), With<Button>>,
    interaction_q: Query<(&BagSlotButton, &Interaction), (Changed<Interaction>, With<Button>)>,
) {
    blocker.0 = blocker.0 || (state.open && cursor_over_character_panel(&windows));

    if !state.open {
        despawn_all(&mut commands, &mut state, &children_q);
        return;
    }

    let Ok(equip) = player_q.single() else {
        return;
    };

    if state.root.is_none() || state.needs_rebuild {
        despawn_all(&mut commands, &mut state, &children_q);
        spawn_character_tab_panel(&mut commands, &asset_server, &bag_slots, &mut state);
        state.needs_rebuild = false;
    }

    for (label, mut text) in slot_label_q.iter_mut() {
        text.0 = equipment_slot_text(equip, label.slot_id);
    }

    // Handle bag slot button clicks — only open if a bag is actually equipped.
    for (bag_btn, interaction) in interaction_q.iter() {
        if *interaction == Interaction::Pressed {
            let has_bag = bag_slots
                .slots
                .iter()
                .find(|s| s.bag_slot == bag_btn.bag_slot)
                .map(|s| s.equipped_item_id.is_some())
                .unwrap_or(false);

            if !has_bag {
                // Nothing equipped — clicking does nothing.
                continue;
            }

            if bag_ui_state.is_open(bag_btn.bag_slot) {
                bag_ui_state.open[bag_btn.bag_slot as usize] = false;
            } else {
                bag_ui_state.open[bag_btn.bag_slot as usize] = true;
            }
            bag_ui_state.needs_rebuild = true;
        }
    }

    // Update bag slot button backgrounds based on equipped state + open state.
    for (bag_btn, mut bg, _) in bag_btn_q.iter_mut() {
        let is_open = bag_ui_state.is_open(bag_btn.bag_slot);
        let is_equipped = bag_slots
            .slots
            .iter()
            .find(|s| s.bag_slot == bag_btn.bag_slot)
            .map(|s| s.equipped_item_id.is_some())
            .unwrap_or(false);

        *bg = if is_open {
            BackgroundColor(Color::srgba(0.16, 0.18, 0.30, 0.98))
        } else if is_equipped {
            BackgroundColor(Color::srgba(0.10, 0.14, 0.10, 0.96))
        } else {
            BackgroundColor(Color::srgba(0.070, 0.058, 0.047, 0.96))
        };
    }
}

fn spawn_character_tab_panel(
    commands: &mut Commands,
    asset_server: &Res<AssetServer>,
    bag_slots: &Res<PlayerBagSlots>,
    state: &mut ResMut<CharacterUiState>,
) {
    let root = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..default()
            },
            CharacterTabRoot,
            Name::new("character_tab_root"),
        ))
        .id();

    let panel = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(PANEL_RIGHT),
                bottom: Val::Px(PANEL_BOTTOM),
                width: Val::Px(PANEL_WIDTH),
                height: Val::Px(PANEL_HEIGHT),
                padding: UiRect::all(Val::Px(PANEL_PADDING)),
                display: Display::Flex,
                flex_direction: FlexDirection::Column,
                border_radius: BorderRadius::all(Val::Px(8.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.030, 0.028, 0.025, 0.94)),
            Name::new("character_tab_panel"),
        ))
        .id();
    commands.entity(root).add_child(panel);

    let font = asset_server.load("fonts/ui.ttf");

    let equip_area = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(EQUIP_AREA_HEIGHT),
                display: Display::Flex,
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(SLOT_GAP),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            Name::new("character_equipment_shape"),
        ))
        .id();
    commands.entity(panel).add_child(equip_area);

    let rows: [[Option<&'static str>; 5]; 6] = [
        [None, None, Some("Helm"), None, None],
        [None, Some("Shoulders"), None, Some("Neck"), None],
        [None, Some("Chest"), None, Some("Back"), None],
        [Some("Gloves"), None, Some("Waist"), None, Some("Wrist")],
        [None, Some("Ring1"), Some("Pants"), Some("Ring2"), None],
        [None, None, Some("Boots"), None, None],
    ];

    for (row_idx, row_slots) in rows.iter().enumerate() {
        let row = commands
            .spawn((
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Px(SLOT_SIZE),
                    display: Display::Flex,
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(SLOT_GAP),
                    justify_content: JustifyContent::Center,
                    ..default()
                },
                Name::new(format!("character_equipment_row_{row_idx}")),
            ))
            .id();
        commands.entity(equip_area).add_child(row);

        for slot_opt in row_slots.iter().copied() {
            let child = match slot_opt {
                Some(slot_id) => spawn_equipment_slot(commands, &font, slot_id),
                None => spawn_slot_spacer(commands),
            };
            commands.entity(row).add_child(child);
        }
    }

    // Bag slot 0 (general) — top-left of the panel.
    // Bag slot 1 (skill)   — top-right of the panel.
    let defs = default_item_defs();
    let bag_configs: [(u8, Val, Val, &str, &str); 2] = [
        (0, Val::Px(PANEL_PADDING), Val::Auto, "Gen\nBag", "character_bag_slot_0"),
        (1, Val::Auto, Val::Px(PANEL_PADDING), "Skill\nBag", "character_bag_slot_1"),
    ];
    for (bag_slot, left, right, slot_label_text, name) in bag_configs {
        let btn = commands
            .spawn((
                Button,
                Node {
                    position_type: PositionType::Absolute,
                    left,
                    right,
                    top: Val::Px(PANEL_PADDING),
                    width: Val::Px(SLOT_SIZE),
                    height: Val::Px(SLOT_SIZE),
                    display: Display::Flex,
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    padding: UiRect::all(Val::Px(3.0)),
                    border_radius: BorderRadius::all(Val::Px(4.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.070, 0.058, 0.047, 0.96)),
                BagSlotButton { bag_slot },
                Name::new(name),
            ))
            .id();

        // Show the bag's icon if one is equipped and it has an icon, otherwise show the fallback label.
        let equipped_icon: Option<String> = bag_slots
            .slots
            .iter()
            .find(|s| s.bag_slot == bag_slot)
            .and_then(|s| s.equipped_item_id.as_ref())
            .and_then(|id| defs.get(id.as_str()))
            .and_then(|def| def.inventory_icon.clone());

        if let Some(icon_path) = equipped_icon {
            let icon = commands
                .spawn((
                    ImageNode::new(asset_server.load(icon_path)),
                    Node {
                        width: Val::Px(SLOT_SIZE - 8.0),
                        height: Val::Px(SLOT_SIZE - 8.0),
                        ..default()
                    },
                    Name::new(format!("character_bag_slot_icon_{bag_slot}")),
                ))
                .id();
            commands.entity(btn).add_child(icon);
        } else {
            let label = commands
                .spawn((
                    Text::new(slot_label_text),
                    TextFont { font: font.clone(), font_size: 9.0, ..default() },
                    TextColor(Color::srgb(0.60, 0.56, 0.50)),
                    Name::new(format!("character_bag_slot_label_{bag_slot}")),
                ))
                .id();
            commands.entity(btn).add_child(label);
        }

        commands.entity(panel).add_child(btn);
    }

    state.root = Some(root);
    state.spawned.push(root);
}

fn spawn_equipment_slot(commands: &mut Commands, font: &Handle<Font>, slot_id: &'static str) -> Entity {
    let slot = commands
        .spawn((
            Node {
                width: Val::Px(SLOT_SIZE),
                height: Val::Px(SLOT_SIZE),
                display: Display::Flex,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                padding: UiRect::all(Val::Px(3.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.070, 0.058, 0.047, 0.96)),
            Name::new(format!("character_slot_{slot_id}")),
        ))
        .id();

    let label = commands
        .spawn((
            Text::new(slot_label(slot_id)),
            TextFont {
                font: font.clone(),
                font_size: 9.0,
                ..default()
            },
            TextColor(Color::srgb(0.84, 0.80, 0.70)),
            CharacterTabSlotLabel { slot_id },
            Name::new(format!("character_slot_label_{slot_id}")),
        ))
        .id();

    commands.entity(slot).add_child(label);
    slot
}

fn spawn_slot_spacer(commands: &mut Commands) -> Entity {
    commands
        .spawn((
            Node {
                width: Val::Px(SLOT_SIZE),
                height: Val::Px(SLOT_SIZE),
                ..default()
            },
            Name::new("character_slot_spacer"),
        ))
        .id()
}

fn equipment_slot_text(equip: &Equipment, slot_id: &str) -> String {
    let item = match slot_id {
        "Helm" => equip.helm.as_ref(),
        "Neck" => equip.neck.as_ref(),
        "Back" => equip.back.as_ref(),
        "Shoulders" => equip.shoulders.as_ref(),
        "Chest" => equip.chest.as_ref(),
        "Wrist" => equip.wrist.as_ref(),
        "Gloves" => equip.gloves.as_ref(),
        "Waist" => equip.waist.as_ref(),
        "Pants" => equip.pants.as_ref(),
        "Boots" => equip.boots.as_ref(),
        "Ring1" => equip.ring1.as_ref(),
        "Ring2" => equip.ring2.as_ref(),
        _ => None,
    };

    item.map(|id| compact_item_label(id.as_str()))
        .unwrap_or_else(|| slot_label(slot_id).to_string())
}

fn compact_item_label(item_id: &str) -> String {
    item_id.replace("item_", "").replace('_', "\n")
}

fn slot_label(slot_id: &str) -> &'static str {
    match slot_id {
        "Helm" => "Helm",
        "Neck" => "Neck",
        "Back" => "Back",
        "Shoulders" => "Shoulder",
        "Chest" => "Chest",
        "Wrist" => "Wrist",
        "Gloves" => "Gloves",
        "Waist" => "Waist",
        "Pants" => "Legs",
        "Boots" => "Boots",
        "Ring1" => "Ring",
        "Ring2" => "Ring",
        _ => "Slot",
    }
}

fn cursor_over_character_panel(windows: &Query<&Window, With<PrimaryWindow>>) -> bool {
    let Ok(window) = windows.single() else {
        return false;
    };
    let Some(cursor) = window.cursor_position() else {
        return false;
    };

    let panel_left = (window.width() - PANEL_WIDTH - PANEL_RIGHT).max(0.0);
    let panel_right = panel_left + PANEL_WIDTH;
    let panel_top = (window.height() - PANEL_HEIGHT - PANEL_BOTTOM).max(0.0);
    let panel_bottom = panel_top + PANEL_HEIGHT;

    cursor.x >= panel_left
        && cursor.x <= panel_right
        && cursor.y >= panel_top
        && cursor.y <= panel_bottom
}

fn despawn_all(
    commands: &mut Commands,
    state: &mut ResMut<CharacterUiState>,
    children_q: &Query<&Children>,
) {
    let mut roots: Vec<Entity> = state.spawned.drain(..).collect();

    if let Some(root) = state.root.take() {
        if !roots.contains(&root) {
            roots.push(root);
        }
    }

    for root in roots {
        despawn_ui_tree(root, children_q, commands);
    }

    state.needs_rebuild = false;
}

fn despawn_ui_tree(root: Entity, children_q: &Query<&Children>, commands: &mut Commands) {
    let mut stack = vec![root];
    let mut all: Vec<Entity> = Vec::new();

    while let Some(e) = stack.pop() {
        all.push(e);
        if let Ok(children) = children_q.get(e) {
            for child in children.iter() {
                stack.push(child);
            }
        }
    }

    for e in all.into_iter().rev() {
        if let Ok(mut ec) = commands.get_entity(e) {
            ec.despawn();
        }
    }
}
