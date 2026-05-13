use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use stonepyre_content::default_item_defs;
use stonepyre_engine::plugins::interaction::WorldInteractionBlocker;
use stonepyre_engine::plugins::inventory::{PlayerBagSlotState, PlayerBagSlots};

const PANEL_WIDTH: f32 = 210.0;
const PANEL_PADDING: f32 = 8.0;
const SLOT_SIZE: f32 = 52.0;
const SLOT_GAP: f32 = 5.0;
const GRID_COLS: usize = 4;
const MENU_WIDTH: f32 = 180.0;
const ITEM_ICON_SIZE: f32 = 42.0;

// Bag panel sits to the left of the character/inventory panel.
const PANEL_RIGHT: f32 = 290.0;
const PANEL_BOTTOM: f32 = 88.0;

#[derive(Resource, Default)]
pub struct BagUiState {
    pub open_bag_slot: Option<u8>,
    pub root: Option<Entity>,
    pub context_menu_root: Option<Entity>,
    pub context_item: Option<BagContextItem>,
    pub needs_rebuild: bool,
}

#[derive(Clone, Debug)]
pub struct BagContextItem {
    pub bag_slot: u8,
    pub bag_item_slot_idx: usize,
    pub item_id: String,
    pub display_name: String,
    pub quantity: u32,
}

#[derive(Resource, Default)]
pub struct BagItemActionQueue {
    pub actions: Vec<BagItemAction>,
}

#[derive(Clone, Debug)]
pub enum BagItemAction {
    Take { bag_slot: u8, bag_item_slot_idx: usize },
    UnequipBag { bag_slot: u8 },
}

#[derive(Component)]
struct BagPanelRoot;

#[derive(Component)]
pub(crate) struct BagItemSlotButton {
    bag_slot: u8,
    slot_idx: usize,
}

#[derive(Component)]
struct BagSlotFallbackLabel {
    slot_idx: usize,
}

#[derive(Component)]
struct BagSlotIcon {
    slot_idx: usize,
}

#[derive(Component)]
struct BagContextMenuRoot;

#[derive(Component)]
pub(crate) struct BagContextOptionButton {
    action: BagContextOption,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BagContextOption {
    Take,
    UnequipBag,
}

pub fn bag_panel_sync_system(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut state: ResMut<BagUiState>,
    mut blocker: ResMut<WorldInteractionBlocker>,
    bag_slots: Res<PlayerBagSlots>,
    windows: Query<&Window, With<PrimaryWindow>>,
) {
    let Some(open_slot) = state.open_bag_slot else {
        despawn_bag_panel(&mut commands, &mut state);
        return;
    };

    blocker.0 = blocker.0 || cursor_over_bag_panel(&windows);

    let slot_data = bag_slots.slots.iter().find(|s| s.bag_slot == open_slot);

    if state.root.is_none() || state.needs_rebuild {
        despawn_bag_panel(&mut commands, &mut state);
        if let Some(data) = slot_data {
            spawn_bag_panel(&mut commands, &asset_server, &mut state, data);
        }
        state.needs_rebuild = false;
    }
}

pub fn bag_context_menu_system(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut state: ResMut<BagUiState>,
    mut action_queue: ResMut<BagItemActionQueue>,
    bag_slots: Res<PlayerBagSlots>,
    mut slot_q: Query<(&Interaction, &BagItemSlotButton), (Changed<Interaction>, With<Button>)>,
    mut option_q: Query<(&Interaction, &BagContextOptionButton), (Changed<Interaction>, With<Button>)>,
) {
    let Some(open_slot) = state.open_bag_slot else {
        return;
    };

    if mouse.just_pressed(MouseButton::Left) && !cursor_over_bag_panel(&windows) {
        close_bag_context_menu(&mut commands, &mut state);
        return;
    }

    for (interaction, slot_btn) in &mut slot_q {
        if *interaction != Interaction::Pressed {
            continue;
        }

        let Some(slot_data) = bag_slots.slots.iter().find(|s| s.bag_slot == slot_btn.bag_slot) else {
            continue;
        };

        let Some(item) = slot_data.items.iter().find(|i| i.slot_idx == slot_btn.slot_idx) else {
            close_bag_context_menu(&mut commands, &mut state);
            continue;
        };

        let ctx = BagContextItem {
            bag_slot: open_slot,
            bag_item_slot_idx: item.slot_idx,
            item_id: item.item_id.clone(),
            display_name: item_display_name(&item.item_id),
            quantity: item.quantity,
        };

        let menu_pos = bag_slot_menu_pos(&windows, slot_btn.slot_idx, slot_data.items.len());
        open_bag_context_menu(&mut commands, &asset_server, &mut state, ctx, menu_pos, true);
    }

    if mouse.just_pressed(MouseButton::Right) {
        if let Some((slot_idx, menu_pos)) = bag_slot_at_cursor(&windows, open_slot, &bag_slots) {
            let Some(slot_data) = bag_slots.slots.iter().find(|s| s.bag_slot == open_slot) else {
                return;
            };
            match slot_data.items.iter().find(|i| i.slot_idx == slot_idx) {
                Some(item) => {
                    let ctx = BagContextItem {
                        bag_slot: open_slot,
                        bag_item_slot_idx: item.slot_idx,
                        item_id: item.item_id.clone(),
                        display_name: item_display_name(&item.item_id),
                        quantity: item.quantity,
                    };
                    open_bag_context_menu(&mut commands, &asset_server, &mut state, ctx, menu_pos, false);
                }
                None => close_bag_context_menu(&mut commands, &mut state),
            }
        }
    }

    for (interaction, option) in &mut option_q {
        if *interaction != Interaction::Pressed {
            continue;
        }

        let Some(item) = state.context_item.clone() else {
            close_bag_context_menu(&mut commands, &mut state);
            continue;
        };

        match option.action {
            BagContextOption::Take => {
                action_queue.actions.push(BagItemAction::Take {
                    bag_slot: item.bag_slot,
                    bag_item_slot_idx: item.bag_item_slot_idx,
                });
                close_bag_context_menu(&mut commands, &mut state);
                state.needs_rebuild = true;
            }
            BagContextOption::UnequipBag => {
                action_queue.actions.push(BagItemAction::UnequipBag {
                    bag_slot: item.bag_slot,
                });
                state.open_bag_slot = None;
                close_bag_context_menu(&mut commands, &mut state);
            }
        }
    }
}

fn spawn_bag_panel(
    commands: &mut Commands,
    asset_server: &Res<AssetServer>,
    state: &mut ResMut<BagUiState>,
    slot_data: &PlayerBagSlotState,
) {
    let item_count = slot_data.items.len();
    let rows = if item_count == 0 { 0 } else { (slot_data.slots_total + GRID_COLS - 1) / GRID_COLS };
    let grid_rows = rows.max(1);
    let panel_height = PANEL_PADDING * 2.0 + 24.0 + (grid_rows as f32 * (SLOT_SIZE + SLOT_GAP)) + SLOT_GAP;

    let root = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(PANEL_RIGHT),
                bottom: Val::Px(PANEL_BOTTOM),
                width: Val::Px(PANEL_WIDTH),
                height: Val::Px(panel_height),
                padding: UiRect::all(Val::Px(PANEL_PADDING)),
                display: Display::Flex,
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(6.0),
                border_radius: BorderRadius::all(Val::Px(8.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.030, 0.028, 0.025, 0.94)),
            BagPanelRoot,
            Name::new("bag_panel_root"),
        ))
        .id();

    let font = asset_server.load("fonts/ui.ttf");

    let bag_name = slot_data.equipped_item_id.as_deref()
        .and_then(|id| default_item_defs().get(id).map(|d| d.name.clone()))
        .unwrap_or_else(|| "Empty Bag Slot".to_string());

    let filter_suffix = slot_data.item_type_filter.as_deref()
        .map(|tag| format!(" ({})", tag))
        .unwrap_or_default();

    let title = commands
        .spawn((
            Text::new(format!("{}{}", bag_name, filter_suffix)),
            TextFont { font: font.clone(), font_size: 13.0, ..default() },
            TextColor(Color::srgb(0.92, 0.86, 0.64)),
            Name::new("bag_panel_title"),
        ))
        .id();
    commands.entity(root).add_child(title);

    let grid = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                display: Display::Flex,
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(SLOT_GAP),
                ..default()
            },
            Pickable::IGNORE,
            Name::new("bag_grid"),
        ))
        .id();
    commands.entity(root).add_child(grid);

    let bag_slot = slot_data.bag_slot;
    for row_idx in 0..grid_rows {
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
                Pickable::IGNORE,
                Name::new(format!("bag_row_{row_idx}")),
            ))
            .id();
        commands.entity(grid).add_child(row);

        for col_idx in 0..GRID_COLS {
            let slot_idx = row_idx * GRID_COLS + col_idx;
            if slot_idx >= slot_data.slots_total && slot_data.slots_total > 0 {
                break;
            }

            let item = slot_data.items.iter().find(|i| i.slot_idx == slot_idx);

            let slot = commands
                .spawn((
                    Button,
                    Node {
                        width: Val::Px(SLOT_SIZE),
                        height: Val::Px(SLOT_SIZE),
                        display: Display::Flex,
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::Center,
                        padding: UiRect::all(Val::Px(4.0)),
                        border: UiRect::all(Val::Px(1.0)),
                        border_radius: BorderRadius::all(Val::Px(4.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.070, 0.058, 0.047, 0.96)),
                    BagItemSlotButton { bag_slot, slot_idx },
                    Name::new(format!("bag_slot_{slot_idx}")),
                ))
                .id();

            if let Some(item_data) = item {
                let icon_path = inventory_icon_path(&item_data.item_id);
                if let Some(path) = icon_path {
                    let icon = commands
                        .spawn((
                            ImageNode::new(asset_server.load(path)),
                            Node { width: Val::Px(ITEM_ICON_SIZE), height: Val::Px(ITEM_ICON_SIZE), ..default() },
                            Pickable::IGNORE,
                            BagSlotIcon { slot_idx },
                            Name::new(format!("bag_slot_icon_{slot_idx}")),
                        ))
                        .id();
                    commands.entity(slot).add_child(icon);
                } else {
                    let label = commands
                        .spawn((
                            Text::new(item_display_name(&item_data.item_id)),
                            TextFont { font: font.clone(), font_size: 9.0, ..default() },
                            TextColor(Color::srgb(0.82, 0.78, 0.68)),
                            Pickable::IGNORE,
                            BagSlotFallbackLabel { slot_idx },
                            Name::new(format!("bag_slot_label_{slot_idx}")),
                        ))
                        .id();
                    commands.entity(slot).add_child(label);
                }
            }

            commands.entity(row).add_child(slot);
        }
    }

    state.root = Some(root);
}

fn open_bag_context_menu(
    commands: &mut Commands,
    asset_server: &Res<AssetServer>,
    state: &mut ResMut<BagUiState>,
    item: BagContextItem,
    menu_pos: Vec2,
    include_unequip: bool,
) {
    close_bag_context_menu(commands, state);

    let Some(root) = state.root else { return; };

    let menu = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(menu_pos.x),
                top: Val::Px(menu_pos.y),
                width: Val::Px(MENU_WIDTH),
                padding: UiRect::all(Val::Px(8.0)),
                display: Display::Flex,
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(4.0),
                border_radius: BorderRadius::all(Val::Px(8.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.025, 0.025, 0.030, 0.98)),
            BagContextMenuRoot,
            Name::new("bag_context_menu"),
        ))
        .id();
    commands.entity(root).add_child(menu);

    let font = asset_server.load("fonts/ui.ttf");
    let title = commands
        .spawn((
            Text::new(item.display_name.clone()),
            TextFont { font: font.clone(), font_size: 15.0, ..default() },
            TextColor(Color::srgb(0.92, 0.86, 0.64)),
            Name::new("bag_context_title"),
        ))
        .id();
    commands.entity(menu).add_child(title);

    let mut options: Vec<(&str, BagContextOption)> = vec![("Take", BagContextOption::Take)];
    if include_unequip {
        options.push(("Unequip Bag", BagContextOption::UnequipBag));
    }

    for (label, action) in options {
        let button = commands
            .spawn((
                Button,
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Px(30.0),
                    justify_content: JustifyContent::FlexStart,
                    align_items: AlignItems::Center,
                    padding: UiRect::horizontal(Val::Px(8.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.10, 0.10, 0.12, 0.95)),
                BagContextOptionButton { action },
                Name::new(format!("bag_context_option_{label}")),
            ))
            .id();

        let text = commands
            .spawn((
                Text::new(label),
                TextFont { font: font.clone(), font_size: 14.0, ..default() },
                TextColor(Color::srgb(0.88, 0.88, 0.88)),
                Pickable::IGNORE,
                Name::new(format!("bag_context_option_text_{label}")),
            ))
            .id();

        commands.entity(button).add_child(text);
        commands.entity(menu).add_child(button);
    }

    state.context_menu_root = Some(menu);
    state.context_item = Some(item);
}

fn close_bag_context_menu(commands: &mut Commands, state: &mut ResMut<BagUiState>) {
    if let Some(menu) = state.context_menu_root.take() {
        if let Ok(mut ec) = commands.get_entity(menu) {
            ec.despawn();
        }
    }
    state.context_item = None;
}

fn despawn_bag_panel(commands: &mut Commands, state: &mut ResMut<BagUiState>) {
    if let Some(root) = state.root.take() {
        if let Ok(mut ec) = commands.get_entity(root) {
            ec.despawn();
        }
    }
    state.context_menu_root = None;
    state.context_item = None;
    state.needs_rebuild = false;
}

fn cursor_over_bag_panel(windows: &Query<&Window, With<PrimaryWindow>>) -> bool {
    let Ok(window) = windows.single() else { return false; };
    let Some(cursor) = window.cursor_position() else { return false; };

    let panel_left = (window.width() - PANEL_RIGHT - PANEL_WIDTH).max(0.0);
    let panel_right = panel_left + PANEL_WIDTH;
    let panel_top = (window.height() - PANEL_BOTTOM - 350.0).max(0.0);
    let panel_bottom = panel_top + 350.0;

    cursor.x >= panel_left && cursor.x <= panel_right && cursor.y >= panel_top && cursor.y <= panel_bottom
}

fn bag_slot_at_cursor(
    windows: &Query<&Window, With<PrimaryWindow>>,
    _open_slot: u8,
    bag_slots: &PlayerBagSlots,
) -> Option<(usize, Vec2)> {
    let window = windows.single().ok()?;
    let cursor = window.cursor_position()?;

    let panel_left = (window.width() - PANEL_RIGHT - PANEL_WIDTH).max(0.0);
    let panel_top = (window.height() - PANEL_BOTTOM - 350.0).max(0.0);

    let grid_left = panel_left + PANEL_PADDING;
    let grid_top = panel_top + PANEL_PADDING + 24.0 + 6.0;

    let local_x = cursor.x - grid_left;
    let local_y = cursor.y - grid_top;
    if local_x < 0.0 || local_y < 0.0 {
        return None;
    }

    let pitch = SLOT_SIZE + SLOT_GAP;
    let col = (local_x / pitch).floor() as usize;
    let row = (local_y / pitch).floor() as usize;
    if col >= GRID_COLS {
        return None;
    }

    let slot_idx = row * GRID_COLS + col;
    let menu_x = (panel_left + PANEL_WIDTH + 8.0).min((window.width() - MENU_WIDTH).max(0.0));
    let menu_y = (grid_top + row as f32 * pitch).max(0.0);

    let _ = bag_slots;
    Some((slot_idx, Vec2::new(menu_x, menu_y)))
}

fn bag_slot_menu_pos(
    windows: &Query<&Window, With<PrimaryWindow>>,
    _slot_idx: usize,
    _item_count: usize,
) -> Vec2 {
    let Ok(window) = windows.single() else { return Vec2::ZERO; };
    let panel_left = (window.width() - PANEL_RIGHT - PANEL_WIDTH).max(0.0);
    Vec2::new(panel_left + PANEL_WIDTH + 8.0, window.height() * 0.3)
}

fn inventory_icon_path(item_id: &str) -> Option<String> {
    default_item_defs().get(item_id).and_then(|def| def.inventory_icon.clone())
}

fn item_display_name(item_id: &str) -> String {
    default_item_defs()
        .get(item_id)
        .map(|def| def.name.clone())
        .unwrap_or_else(|| item_id.to_string())
}
