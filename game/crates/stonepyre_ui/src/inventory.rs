use bevy::prelude::*;

use stonepyre_content::default_item_defs;
use stonepyre_engine::plugins::inventory::{Inventory, ItemStack};

use crate::config::UiBindings;

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
    pub item_id: String,
    pub quantity: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InventoryItemAction {
    Drop,
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
pub(crate) struct SlotLabel {
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
}

#[derive(Component)]
pub(crate) struct InventoryStatusLabel;

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
    player_q: Query<&Inventory>,
    slot_text_q: Query<(Entity, &SlotLabel)>,
    status_text_q: Query<Entity, With<InventoryStatusLabel>>,
) {
    if !state.open {
        despawn_all(&mut commands, &mut state);
        return;
    }

    let Ok(inv) = player_q.single() else { return; };

    if state.root.is_none() || state.needs_rebuild {
        despawn_all(&mut commands, &mut state);
        spawn_inventory_panel(&mut commands, &asset_server, &mut state);
        state.needs_rebuild = false;
    }

    update_slot_labels(&mut commands, &inv, slot_text_q);
    update_status_label(&mut commands, &state, status_text_q);
}

pub(crate) fn inventory_item_context_menu_system(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mouse: Res<ButtonInput<MouseButton>>,
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

    for (interaction, slot) in &mut slot_q {
        if *interaction != Interaction::Pressed || !mouse.just_pressed(MouseButton::Right) {
            continue;
        }

        let Some(item) = inventory_item_for_slot(inv, slot.idx) else {
            close_context_menu(&mut commands, &mut state);
            continue;
        };

        open_context_menu(&mut commands, &asset_server, &mut state, item);
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
            InventoryPanelRoot,
            Name::new("inventory_panel_root".to_string()),
        ))
        .id();

    let panel = commands
        .spawn((
            Node {
                width: Val::Px(560.0),
                height: Val::Px(680.0),
                margin: UiRect::all(Val::Auto),
                padding: UiRect::all(Val::Px(16.0)),
                display: Display::Flex,
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(12.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.05, 0.05, 0.06, 0.92)),
            Name::new("inventory_panel".to_string()),
        ))
        .id();

    commands.entity(root).add_child(panel);

    let font = asset_server.load("fonts/ui.ttf");

    let title = commands
        .spawn((
            Text::new("Inventory (I to close)"),
            TextFont {
                font: font.clone(),
                font_size: 26.0,
                ..default()
            },
            TextColor(Color::srgb(0.95, 0.95, 0.95)),
            Name::new("inventory_title".to_string()),
        ))
        .id();
    commands.entity(panel).add_child(title);

    let grid = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Auto,
                display: Display::Flex,
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(10.0),
                ..default()
            },
            Name::new("inventory_grid".to_string()),
        ))
        .id();

    commands.entity(panel).add_child(grid);

    let slot_px: f32 = 104.0;
    let gap: f32 = 10.0;

    let mut idx: usize = 0;
    for r in 0..4 {
        let row = commands
            .spawn((
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Px(slot_px),
                    display: Display::Flex,
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(gap),
                    ..default()
                },
                Name::new(format!("inv_row_{r}")),
            ))
            .id();
        commands.entity(grid).add_child(row);

        for c in 0..4 {
            let slot = commands
                .spawn((
                    Button,
                    Node {
                        width: Val::Px(slot_px),
                        height: Val::Px(slot_px),
                        display: Display::Flex,
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::Center,
                        padding: UiRect::all(Val::Px(8.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.10, 0.10, 0.12, 0.95)),
                    InventorySlotButton { idx },
                    Name::new(format!("inv_slot_{r}_{c}")),
                ))
                .id();

            let label = commands
                .spawn((
                    Text::new("Empty"),
                    TextFont {
                        font: font.clone(),
                        font_size: 14.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.85, 0.85, 0.85)),
                    SlotLabel { idx },
                    Name::new(format!("inv_slot_label_{idx}")),
                ))
                .id();

            commands.entity(slot).add_child(label);
            commands.entity(row).add_child(slot);

            idx += 1;
        }
    }

    let hint = commands
        .spawn((
            Text::new("Right-click an item for Use / Drop / Examine."),
            TextFont {
                font: font.clone(),
                font_size: 13.0,
                ..default()
            },
            TextColor(Color::srgb(0.72, 0.72, 0.74)),
            Name::new("inventory_hint".to_string()),
        ))
        .id();
    commands.entity(panel).add_child(hint);

    let status = commands
        .spawn((
            Text::new(""),
            TextFont {
                font,
                font_size: 14.0,
                ..default()
            },
            TextColor(Color::srgb(0.90, 0.82, 0.58)),
            InventoryStatusLabel,
            Name::new("inventory_status".to_string()),
        ))
        .id();
    commands.entity(panel).add_child(status);

    state.spawned.push(root);
    state.root = Some(root);
}

fn open_context_menu(
    commands: &mut Commands,
    asset_server: &Res<AssetServer>,
    state: &mut ResMut<InventoryUiState>,
    item: InventoryContextItem,
) {
    close_context_menu(commands, state);

    let Some(root) = state.root else {
        return;
    };

    let menu = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(620.0),
                top: Val::Px(220.0 + (item.slot_idx as f32 % 4.0) * 18.0),
                width: Val::Px(220.0),
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

    for (label, action) in [
        ("Use", InventoryContextOption::Use),
        ("Drop", InventoryContextOption::Drop),
        ("Examine", InventoryContextOption::Examine),
    ] {
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

fn update_slot_labels(
    commands: &mut Commands,
    inv: &Inventory,
    slot_text_q: Query<(Entity, &SlotLabel)>,
) {
    for (e, lab) in slot_text_q.iter() {
        let txt = match inv.container.slots.get(lab.idx) {
            None => "Empty".to_string(),
            Some(None) => "Empty".to_string(),
            Some(Some(stk)) => item_stack_label(stk),
        };
        commands.entity(e).insert(Text::new(txt));
    }
}

fn update_status_label(
    commands: &mut Commands,
    state: &InventoryUiState,
    status_text_q: Query<Entity, With<InventoryStatusLabel>>,
) {
    for e in status_text_q.iter() {
        commands.entity(e).insert(Text::new(state.status_message.clone()));
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

fn item_stack_label(stk: &ItemStack) -> String {
    let name = item_display_name(&stk.id);
    if stk.qty > 1 {
        format!("{}\nx{}", name, stk.qty)
    } else {
        name
    }
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
    } else if def.tags.iter().any(|tag| tag == "backpack") {
        format!("{} can be worn on your back.", def.name)
    } else if def.tags.iter().any(|tag| tag == "bag_upgrade") {
        format!("{} can upgrade a compatible backpack.", def.name)
    } else {
        format!("You see {}.", def.name)
    }
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
