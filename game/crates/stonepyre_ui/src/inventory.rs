use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use stonepyre_content::default_item_defs;
use stonepyre_engine::plugins::interaction::WorldInteractionBlocker;
use stonepyre_engine::plugins::inventory::Inventory;

use crate::config::UiBindings;

const PANEL_WIDTH: f32 = 270.0;
const PANEL_HEIGHT: f32 = 334.0;
const PANEL_PADDING: f32 = 10.0;
const PANEL_RIGHT: f32 = 10.0;
const PANEL_BOTTOM: f32 = 88.0;
const GRID_TOP_OFFSET: f32 = PANEL_PADDING;
const SLOT_SIZE: f32 = 58.0;
const SLOT_GAP: f32 = 6.0;
const GRID_COLS: usize = 4;
const GRID_ROWS: usize = 5;
const MENU_WIDTH: f32 = 220.0;
const ITEM_ICON_SIZE: f32 = 48.0;

#[derive(Resource, Default)]
pub struct InventoryUiState {
    pub open: bool,
    pub root: Option<Entity>,
    pub spawned: Vec<Entity>,
    pub needs_rebuild: bool,
    pub context_menu_root: Option<Entity>,
    pub context_item: Option<InventoryContextItem>,
    pub selected_use_item: Option<InventoryContextItem>,
    pub status_message: String,
}

#[derive(Resource, Default)]
pub struct InventoryItemActionQueue {
    pub actions: Vec<InventoryItemActionRequest>,
}

#[derive(Clone, Debug)]
pub struct InventoryItemActionRequest {
    pub action: InventoryItemAction,
    pub slot_idx: usize,
    pub item_id: String,
    pub quantity: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InventoryItemAction {
    Drop,
    EquipBag { bag_slot: u8 },
}

#[derive(Clone, Debug)]
pub struct InventoryContextItem {
    pub slot_idx: usize,
    pub item_id: String,
    pub display_name: String,
    pub quantity: u32,
}

#[derive(Component)]
struct InventoryPanelRoot;

#[derive(Component)]
pub(crate) struct SlotIcon {
    idx: usize,
}

#[derive(Component)]
pub(crate) struct SlotFallbackLabel {
    idx: usize,
}

#[derive(Component)]
pub(crate) struct InventorySlotButton {
    idx: usize,
}

#[derive(Component)]
struct InventoryContextMenuRoot;

#[derive(Component)]
pub(crate) struct InventoryContextOptionButton {
    action: InventoryContextOption,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum InventoryContextOption {
    Use,
    Drop,
    Examine,
    EquipToBag0,
    EquipToBag1,
}

pub fn inventory_toggle_system(
    keys: Res<ButtonInput<KeyCode>>,
    binds: Res<UiBindings>,
    mut state: ResMut<InventoryUiState>,
) {
    if keys.just_pressed(binds.toggle_inventory) {
        state.open = !state.open;
        state.needs_rebuild = true;
        state.context_item = None;
        state.context_menu_root = None;
    }
}

pub(crate) fn inventory_panel_sync_system(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut state: ResMut<InventoryUiState>,
    mut blocker: ResMut<WorldInteractionBlocker>,
    windows: Query<&Window, With<PrimaryWindow>>,
    player_q: Query<&Inventory>,
    slot_icon_q: Query<(Entity, &SlotIcon)>,
    slot_fallback_q: Query<(Entity, &SlotFallbackLabel)>,
    mut slot_bg_q: Query<(&InventorySlotButton, &mut BackgroundColor)>,
) {
    if !state.open {
        despawn_all(&mut commands, &mut state);
        return;
    }

    blocker.0 = blocker.0 || cursor_over_inventory_panel(&windows);

    let Ok(inv) = player_q.single() else { return; };

    if state.root.is_none() || state.needs_rebuild {
        despawn_all(&mut commands, &mut state);
        spawn_inventory_panel(&mut commands, &asset_server, &mut state);
        state.needs_rebuild = false;
    }

    update_slot_items(&mut commands, &asset_server, &inv, slot_icon_q, slot_fallback_q);
    update_slot_highlights(&state, &mut slot_bg_q);
}

pub(crate) fn inventory_item_context_menu_system(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut state: ResMut<InventoryUiState>,
    mut action_queue: ResMut<InventoryItemActionQueue>,
    player_q: Query<&Inventory>,
    mut slot_q: Query<(&Interaction, &InventorySlotButton), (Changed<Interaction>, With<Button>)>,
    mut option_q: Query<(&Interaction, &InventoryContextOptionButton), (Changed<Interaction>, With<Button>)>,
) {
    if !state.open {
        return;
    }

    let Ok(inv) = player_q.single() else { return; };

    // OSRS-ish fallback: clicking the world while an item is selected cancels Use,
    // but does not block the world click from walking/interacting.
    if mouse.just_pressed(MouseButton::Left) && !cursor_over_inventory_panel(&windows) {
        state.selected_use_item = None;
        state.status_message.clear();
        close_context_menu(&mut commands, &mut state);
        return;
    }

    // OSRS-ish primary action: left-click uses/selects the item.
    for (interaction, slot) in &mut slot_q {
        if *interaction != Interaction::Pressed {
            continue;
        }

        let Some(item) = inventory_item_for_slot(inv, slot.idx) else {
            close_context_menu(&mut commands, &mut state);
            continue;
        };

        state.selected_use_item = Some(item.clone());
        state.status_message = format!("Use {} ->", item.display_name);
        close_context_menu(&mut commands, &mut state);
    }

    // Secondary action: right-click opens the item context menu beside the slot.
    if mouse.just_pressed(MouseButton::Right) {
        if let Some((slot_idx, menu_pos)) = inventory_slot_at_cursor(&windows) {
            match inventory_item_for_slot(inv, slot_idx) {
                Some(item) => open_context_menu(&mut commands, &asset_server, &mut state, item, menu_pos),
                None => close_context_menu(&mut commands, &mut state),
            }
        } else if cursor_over_inventory_panel(&windows) {
            close_context_menu(&mut commands, &mut state);
        }
    }

    for (interaction, option) in &mut option_q {
        if *interaction != Interaction::Pressed {
            continue;
        }

        let Some(item) = state.context_item.clone() else {
            close_context_menu(&mut commands, &mut state);
            continue;
        };

        match option.action {
            InventoryContextOption::Use => {
                state.selected_use_item = Some(item.clone());
                state.status_message = format!("Use {} ->", item.display_name);
                close_context_menu(&mut commands, &mut state);
            }
            InventoryContextOption::Drop => {
                action_queue.actions.push(InventoryItemActionRequest {
                    action: InventoryItemAction::Drop,
                    slot_idx: item.slot_idx,
                    item_id: item.item_id.clone(),
                    quantity: 1,
                });
                state.status_message = format!("Dropping {}...", item.display_name);
                close_context_menu(&mut commands, &mut state);
            }
            InventoryContextOption::Examine => {
                state.status_message = examine_text(&item.item_id);
                close_context_menu(&mut commands, &mut state);
            }
            InventoryContextOption::EquipToBag0 => {
                action_queue.actions.push(InventoryItemActionRequest {
                    action: InventoryItemAction::EquipBag { bag_slot: 0 },
                    slot_idx: item.slot_idx,
                    item_id: item.item_id.clone(),
                    quantity: 1,
                });
                close_context_menu(&mut commands, &mut state);
            }
            InventoryContextOption::EquipToBag1 => {
                action_queue.actions.push(InventoryItemActionRequest {
                    action: InventoryItemAction::EquipBag { bag_slot: 1 },
                    slot_idx: item.slot_idx,
                    item_id: item.item_id.clone(),
                    quantity: 1,
                });
                close_context_menu(&mut commands, &mut state);
            }
        }
    }
}

fn spawn_inventory_panel(
    commands: &mut Commands,
    asset_server: &Res<AssetServer>,
    state: &mut ResMut<InventoryUiState>,
) {
    let root = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..default()
            },
            Pickable::IGNORE,
            InventoryPanelRoot,
            Name::new("inventory_panel_root".to_string()),
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
                row_gap: Val::Px(0.0),
                border_radius: BorderRadius::all(Val::Px(8.0)),
                ..default()
            },
            Pickable::IGNORE,
            BackgroundColor(Color::srgba(0.030, 0.028, 0.025, 0.94)),
            Name::new("inventory_tab_panel".to_string()),
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
            Pickable::IGNORE,
            Name::new("inventory_grid".to_string()),
        ))
        .id();

    commands.entity(panel).add_child(grid);

    let mut idx: usize = 0;
    for r in 0..GRID_ROWS {
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
                Name::new(format!("inv_row_{r}")),
            ))
            .id();
        commands.entity(grid).add_child(row);

        for c in 0..GRID_COLS {
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
                    InventorySlotButton { idx },
                    Name::new(format!("inv_slot_{r}_{c}")),
                ))
                .id();

            let icon = commands
                .spawn((
                    ImageNode::new(Handle::<Image>::default()),
                    Node {
                        width: Val::Px(ITEM_ICON_SIZE),
                        height: Val::Px(ITEM_ICON_SIZE),
                        ..default()
                    },
                    Pickable::IGNORE,
                    Visibility::Hidden,
                    SlotIcon { idx },
                    Name::new(format!("inv_slot_icon_{idx}")),
                ))
                .id();

            let fallback = commands
                .spawn((
                    Text::new(""),
                    TextFont {
                        font: font.clone(),
                        font_size: 10.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.82, 0.78, 0.68)),
                    Pickable::IGNORE,
                    Visibility::Hidden,
                    SlotFallbackLabel { idx },
                    Name::new(format!("inv_slot_fallback_label_{idx}")),
                ))
                .id();

            commands.entity(slot).add_child(icon);
            commands.entity(slot).add_child(fallback);
            commands.entity(row).add_child(slot);

            idx += 1;
        }
    }

    state.spawned.push(root);
    state.root = Some(root);
}

fn open_context_menu(
    commands: &mut Commands,
    asset_server: &Res<AssetServer>,
    state: &mut ResMut<InventoryUiState>,
    item: InventoryContextItem,
    menu_pos: Vec2,
) {
    close_context_menu(commands, state);

    let Some(root) = state.root else {
        return;
    };

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
            InventoryContextMenuRoot,
            Name::new("inventory_context_menu"),
        ))
        .id();

    commands.entity(root).add_child(menu);

    let font = asset_server.load("fonts/ui.ttf");
    let title = commands
        .spawn((
            Text::new(item.display_name.clone()),
            TextFont {
                font: font.clone(),
                font_size: 15.0,
                ..default()
            },
            TextColor(Color::srgb(0.92, 0.86, 0.64)),
            Name::new("inventory_context_title"),
        ))
        .id();
    commands.entity(menu).add_child(title);

    let is_bag = default_item_defs()
        .get(&item.item_id)
        .map(|def| def.tags.iter().any(|t| t == "bag"))
        .unwrap_or(false);

    let mut options: Vec<(&'static str, InventoryContextOption)> = vec![
        ("Use", InventoryContextOption::Use),
        ("Drop", InventoryContextOption::Drop),
        ("Examine", InventoryContextOption::Examine),
    ];

    if is_bag {
        options.push(("Equip (Slot 1)", InventoryContextOption::EquipToBag0));
        options.push(("Equip (Slot 2)", InventoryContextOption::EquipToBag1));
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
                InventoryContextOptionButton { action },
                Name::new(format!("inventory_context_option_{label}")),
            ))
            .id();

        let text = commands
            .spawn((
                Text::new(label),
                TextFont {
                    font: font.clone(),
                    font_size: 14.0,
                    ..default()
                },
                TextColor(Color::srgb(0.88, 0.88, 0.88)),
                Pickable::IGNORE,
                Name::new(format!("inventory_context_option_text_{label}")),
            ))
            .id();

        commands.entity(button).add_child(text);
        commands.entity(menu).add_child(button);
    }

    state.context_menu_root = Some(menu);
    state.context_item = Some(item);
}

fn close_context_menu(commands: &mut Commands, state: &mut ResMut<InventoryUiState>) {
    if let Some(menu) = state.context_menu_root.take() {
        if let Ok(mut ec) = commands.get_entity(menu) {
            ec.despawn();
        }
    }
    state.context_item = None;
}

fn update_slot_items(
    commands: &mut Commands,
    asset_server: &Res<AssetServer>,
    inv: &Inventory,
    slot_icon_q: Query<(Entity, &SlotIcon)>,
    slot_fallback_q: Query<(Entity, &SlotFallbackLabel)>,
) {
    for (e, icon) in slot_icon_q.iter() {
        match inv.container.slots.get(icon.idx) {
            Some(Some(stk)) => match inventory_icon_path(&stk.id) {
                Some(path) => {
                    commands.entity(e).insert(ImageNode::new(asset_server.load(path)));
                    commands.entity(e).insert(Visibility::Visible);
                }
                None => {
                    commands.entity(e).insert(Visibility::Hidden);
                }
            },
            _ => {
                commands.entity(e).insert(Visibility::Hidden);
            }
        }
    }

    for (e, label) in slot_fallback_q.iter() {
        let text = match inv.container.slots.get(label.idx) {
            Some(Some(stk)) if inventory_icon_path(&stk.id).is_none() => item_display_name(&stk.id),
            _ => "".to_string(),
        };
        let visible = if text.is_empty() {
            Visibility::Hidden
        } else {
            Visibility::Visible
        };
        commands.entity(e).insert(Text::new(text));
        commands.entity(e).insert(visible);
    }
}

fn update_slot_highlights(
    state: &InventoryUiState,
    slot_bg_q: &mut Query<(&InventorySlotButton, &mut BackgroundColor)>,
) {
    let selected_slot_idx = state.selected_use_item.as_ref().map(|item| item.slot_idx);

    for (slot, mut bg) in slot_bg_q.iter_mut() {
        *bg = if Some(slot.idx) == selected_slot_idx {
            BackgroundColor(Color::srgba(0.16, 0.18, 0.30, 0.98))
        } else {
            BackgroundColor(Color::srgba(0.070, 0.058, 0.047, 0.96))
        };
    }
}

fn inventory_item_for_slot(inv: &Inventory, idx: usize) -> Option<InventoryContextItem> {
    let stk = inv.container.slots.get(idx)?.as_ref()?;
    Some(InventoryContextItem {
        slot_idx: idx,
        item_id: stk.id.clone(),
        display_name: item_display_name(&stk.id),
        quantity: stk.qty,
    })
}

fn inventory_slot_at_cursor(windows: &Query<&Window, With<PrimaryWindow>>) -> Option<(usize, Vec2)> {
    let window = windows.single().ok()?;
    let cursor = window.cursor_position()?;

    let panel_left = inventory_panel_left(window);
    let panel_top = inventory_panel_top(window);
    let grid_width = (GRID_COLS as f32 * SLOT_SIZE) + ((GRID_COLS - 1) as f32 * SLOT_GAP);
    let grid_left = panel_left + ((PANEL_WIDTH - grid_width) * 0.5);
    let grid_top = panel_top + GRID_TOP_OFFSET;

    let local_x = cursor.x - grid_left;
    let local_y = cursor.y - grid_top;
    if local_x < 0.0 || local_y < 0.0 {
        return None;
    }

    let pitch = SLOT_SIZE + SLOT_GAP;
    let col = (local_x / pitch).floor() as usize;
    let row = (local_y / pitch).floor() as usize;
    if col >= GRID_COLS || row >= GRID_ROWS {
        return None;
    }

    let within_slot_x = local_x - (col as f32 * pitch);
    let within_slot_y = local_y - (row as f32 * pitch);
    if within_slot_x > SLOT_SIZE || within_slot_y > SLOT_SIZE {
        return None;
    }

    let slot_idx = row * GRID_COLS + col;
    let slot_right = grid_left + (col as f32 * pitch) + SLOT_SIZE;
    let slot_top = grid_top + (row as f32 * pitch);
    let menu_x = (slot_right + 8.0).min((window.width() - MENU_WIDTH).max(0.0));
    let menu_y = slot_top.max(0.0);

    Some((slot_idx, Vec2::new(menu_x, menu_y)))
}

fn inventory_icon_path(item_id: &str) -> Option<String> {
    default_item_defs()
        .get(item_id)
        .and_then(|def| def.inventory_icon.clone())
}

fn item_display_name(item_id: &str) -> String {
    default_item_defs()
        .get(item_id)
        .map(|def| def.name.clone())
        .unwrap_or_else(|| item_id.to_string())
}

fn examine_text(item_id: &str) -> String {
    let defs = default_item_defs();
    let Some(def) = defs.get(item_id) else {
        return format!("You see {}.", item_id);
    };

    if def.tags.iter().any(|tag| tag == "log") {
        format!("A sturdy {}.", def.name.to_lowercase())
    } else if def.tags.iter().any(|tag| tag == "bag") {
        format!("{} can be equipped in a bag slot.", def.name)
    } else if def.tags.iter().any(|tag| tag == "bag_upgrade") {
        format!("{} can upgrade a compatible backpack.", def.name)
    } else {
        format!("You see {}.", def.name)
    }
}

fn cursor_over_inventory_panel(windows: &Query<&Window, With<PrimaryWindow>>) -> bool {
    let Ok(window) = windows.single() else {
        return false;
    };
    let Some(cursor) = window.cursor_position() else {
        return false;
    };

    let panel_left = inventory_panel_left(window);
    let panel_right = panel_left + PANEL_WIDTH;
    let panel_top = inventory_panel_top(window);
    let panel_bottom = panel_top + PANEL_HEIGHT;

    cursor.x >= panel_left
        && cursor.x <= panel_right
        && cursor.y >= panel_top
        && cursor.y <= panel_bottom
}

fn inventory_panel_left(window: &Window) -> f32 {
    (window.width() - PANEL_WIDTH - PANEL_RIGHT).max(0.0)
}

fn inventory_panel_top(window: &Window) -> f32 {
    (window.height() - PANEL_HEIGHT - PANEL_BOTTOM).max(0.0)
}

fn despawn_all(commands: &mut Commands, state: &mut ResMut<InventoryUiState>) {
    for e in state.spawned.drain(..) {
        if let Ok(mut ec) = commands.get_entity(e) {
            ec.despawn();
        }
    }
    state.root = None;
    state.context_menu_root = None;
    state.context_item = None;
    state.needs_rebuild = false;
}
