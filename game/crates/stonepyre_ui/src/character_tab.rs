use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use stonepyre_engine::plugins::interaction::WorldInteractionBlocker;
use stonepyre_engine::plugins::inventory::{Equipment, Toolbelt};
use stonepyre_engine::plugins::world::Player;

use crate::character::CharacterUiState;

const PANEL_WIDTH: f32 = 270.0;
const PANEL_HEIGHT: f32 = 334.0;
const PANEL_PADDING: f32 = 10.0;
const PANEL_RIGHT: f32 = 10.0;
const PANEL_BOTTOM: f32 = 88.0;

const SLOT_SIZE: f32 = 52.0;
const SLOT_GAP: f32 = 7.0;

#[derive(Component)]
pub(crate) struct CharacterTabRoot;

#[derive(Component)]
pub(crate) struct CharacterTabSlotLabel {
    slot_id: &'static str,
}

#[derive(Component)]
pub(crate) struct CharacterTabStatsText;

#[derive(Component)]
pub(crate) struct CharacterTabToolsText;

pub(crate) fn character_tab_panel_sync_system(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut state: ResMut<CharacterUiState>,
    mut blocker: ResMut<WorldInteractionBlocker>,
    windows: Query<&Window, With<PrimaryWindow>>,
    player_q: Query<(&Equipment, Option<&Toolbelt>), With<Player>>,
    children_q: Query<&Children>,
    mut slot_label_q: Query<(&CharacterTabSlotLabel, &mut Text)>,
    mut stats_text_q: Query<&mut Text, With<CharacterTabStatsText>>,
    mut tools_text_q: Query<&mut Text, With<CharacterTabToolsText>>,
) {
    blocker.0 = blocker.0 || (state.open && cursor_over_character_panel(&windows));

    if !state.open {
        despawn_all(&mut commands, &mut state, &children_q);
        return;
    }

    let Ok((equip, toolbelt_opt)) = player_q.single() else {
        return;
    };

    if state.root.is_none() || state.needs_rebuild {
        despawn_all(&mut commands, &mut state, &children_q);
        spawn_character_tab_panel(&mut commands, &asset_server, &mut state);
        state.needs_rebuild = false;
    }

    for (label, mut text) in slot_label_q.iter_mut() {
        text.0 = equipment_slot_text(equip, label.slot_id);
    }

    if let Ok(mut text) = stats_text_q.single_mut() {
        text.0 = [
            "Stats",
            "Armor       —",
            "Damage      —",
            "Accuracy    —",
            "Carry       — / —",
        ]
        .join("\n");
    }

    if let Ok(mut text) = tools_text_q.single_mut() {
        let axe = tool_text(toolbelt_opt, "Axe");
        let pickaxe = tool_text(toolbelt_opt, "Pickaxe");
        let rod = tool_text(toolbelt_opt, "Rod");
        text.0 = format!("Tools\nAxe: {axe}\nPickaxe: {pickaxe}\nRod: {rod}");
    }
}

fn spawn_character_tab_panel(
    commands: &mut Commands,
    asset_server: &Res<AssetServer>,
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
                row_gap: Val::Px(9.0),
                border_radius: BorderRadius::all(Val::Px(8.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.030, 0.028, 0.025, 0.94)),
            Name::new("character_tab_panel"),
        ))
        .id();
    commands.entity(root).add_child(panel);

    let font = asset_server.load("fonts/ui.ttf");

    let grid = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Auto,
                display: Display::Flex,
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(SLOT_GAP),
                align_items: AlignItems::Center,
                ..default()
            },
            Name::new("character_equipment_grid"),
        ))
        .id();
    commands.entity(panel).add_child(grid);

    let rows: [[&'static str; 3]; 4] = [
        ["Helm", "Neck", "Back"],
        ["Shoulders", "Chest", "Wrist"],
        ["Gloves", "Waist", "Pants"],
        ["Boots", "Ring1", "Ring2"],
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
        commands.entity(grid).add_child(row);

        for slot_id in row_slots.iter().copied() {
            let slot = spawn_equipment_slot(commands, &font, slot_id);
            commands.entity(row).add_child(slot);
        }
    }

    let lower = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Auto,
                display: Display::Flex,
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(8.0),
                ..default()
            },
            Name::new("character_tab_lower"),
        ))
        .id();
    commands.entity(panel).add_child(lower);

    let stats = spawn_info_box(commands, &font, "", CharacterTabStatsText);
    commands.entity(lower).add_child(stats);

    let tools = spawn_tools_box(commands, &font);
    commands.entity(lower).add_child(tools);

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
                padding: UiRect::all(Val::Px(4.0)),
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
            TextColor(Color::srgb(0.82, 0.78, 0.68)),
            CharacterTabSlotLabel { slot_id },
            Name::new(format!("character_slot_label_{slot_id}")),
        ))
        .id();

    commands.entity(slot).add_child(label);
    slot
}

fn spawn_info_box<T: Component>(
    commands: &mut Commands,
    font: &Handle<Font>,
    text: &str,
    marker: T,
) -> Entity {
    let box_entity = commands
        .spawn((
            Node {
                width: Val::Px(121.0),
                height: Val::Px(84.0),
                padding: UiRect::all(Val::Px(8.0)),
                border_radius: BorderRadius::all(Val::Px(5.0)),
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(Color::srgba(0.055, 0.047, 0.040, 0.96)),
            Name::new("character_info_box"),
        ))
        .id();

    let text_entity = commands
        .spawn((
            Text::new(text),
            TextFont {
                font: font.clone(),
                font_size: 10.0,
                ..default()
            },
            TextColor(Color::srgb(0.78, 0.74, 0.66)),
            marker,
            Name::new("character_info_text"),
        ))
        .id();

    commands.entity(box_entity).add_child(text_entity);
    box_entity
}

fn spawn_tools_box(commands: &mut Commands, font: &Handle<Font>) -> Entity {
    let box_entity = commands
        .spawn((
            Node {
                width: Val::Px(121.0),
                height: Val::Px(84.0),
                padding: UiRect::all(Val::Px(8.0)),
                border_radius: BorderRadius::all(Val::Px(5.0)),
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(Color::srgba(0.055, 0.047, 0.040, 0.96)),
            Name::new("character_tools_box"),
        ))
        .id();

    let text_entity = commands
        .spawn((
            Text::new("Tools"),
            TextFont {
                font: font.clone(),
                font_size: 10.0,
                ..default()
            },
            TextColor(Color::srgb(0.78, 0.74, 0.66)),
            CharacterTabToolsText,
            Name::new("character_tools_text"),
        ))
        .id();

    commands.entity(box_entity).add_child(text_entity);
    box_entity
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

fn tool_text(toolbelt: Option<&Toolbelt>, tool_id: &'static str) -> String {
    toolbelt
        .and_then(|tb| tb.get_by_id(tool_id))
        .map(|id| compact_item_label(id.as_str()))
        .unwrap_or_else(|| "—".to_string())
}

fn compact_item_label(item_id: &str) -> String {
    item_id
        .replace("item_", "")
        .replace('_', "\n")
}

fn slot_label(slot_id: &str) -> &'static str {
    match slot_id {
        "Helm" => "Helm",
        "Neck" => "Neck",
        "Back" => "Back",
        "Shoulders" => "Shldr",
        "Chest" => "Chest",
        "Wrist" => "Wrist",
        "Gloves" => "Glove",
        "Waist" => "Waist",
        "Pants" => "Legs",
        "Boots" => "Boot",
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
    for root in state.spawned.drain(..) {
        despawn_ui_tree(root, children_q, commands);
    }

    if let Some(root) = state.root.take() {
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
