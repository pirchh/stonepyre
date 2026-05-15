// crates/stonepyre_ui/src/bank.rs
//
// OSRS-style bank panel.
//
// Layout:
//  ┌────────────────────────────────────────────┐
//  │  Bank                                  [X] │  ← title bar
//  ├──────────────────────────────────────────── │
//  │ [All] [Tab 1] [Tab 2] ... [+]              │  ← tab bar
//  ├──────────────────────────────────────────── │
//  │ [Search: _______________]  [Deposit All]   │  ← toolbar
//  ├──────────────────────────────────────────── │
//  │  item  item  item  item  item  item  item   │  ← 8-col item grid
//  │  item  ...                                  │
//  └────────────────────────────────────────────┘
//
// Tab 0 = "All" (virtual aggregate of every tab's items, read-only tab bar entry).
// Tabs 1-11 = physical tabs from the server.
//
// Right-click any item → context menu: Withdraw 1 / 5 / 10 / All.
// "Deposit All" deposits every inventory item to the bank.

use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use stonepyre_content::default_item_defs;
use stonepyre_engine::plugins::interaction::WorldInteractionBlocker;
use stonepyre_engine::plugins::inventory::Inventory;
use stonepyre_engine::plugins::world::Player;

// ── Layout constants ──────────────────────────────────────────────────────────

const PANEL_WIDTH: f32 = 740.0;
const PANEL_MIN_HEIGHT: f32 = 480.0;
const PANEL_PADDING: f32 = 12.0;

// Item grid
const GRID_COLS: usize = 8;
const SLOT_SIZE: f32 = 72.0;
const SLOT_GAP: f32 = 6.0;
const ITEM_ICON_SIZE: f32 = 56.0;

// Context menu
const MENU_WIDTH: f32 = 200.0;

// ── Resources ─────────────────────────────────────────────────────────────────

/// All state the bank panel needs to render and interact with.
#[derive(Resource)]
pub struct BankUiState {
    /// Whether the panel is visible.
    pub open: bool,
    /// Root entity of the bank panel (and all its children).
    pub root: Option<Entity>,
    /// Rebuild the panel DOM next frame.
    pub needs_rebuild: bool,
    /// Which tab is showing. 0 = "All" (virtual), 1-11 = physical.
    pub active_tab_idx: u8,
    /// Right-click context menu root.
    pub context_menu_root: Option<Entity>,
    /// Which bank item is being right-clicked.
    pub context_item: Option<BankContextItem>,
    /// Snapshot of tabs received from the server (written by bank_sync).
    pub tabs: Vec<BankTabData>,
    /// Last known inventory so we can populate a deposit-from-inv view.
    /// Filled by `bank_panel_sync_system` from the Player Inventory component.
    pub inv_slot_count: usize,
}

impl Default for BankUiState {
    fn default() -> Self {
        Self {
            open: false,
            root: None,
            needs_rebuild: false,
            active_tab_idx: 0,
            context_menu_root: None,
            context_item: None,
            tabs: Vec::new(),
            inv_slot_count: 0,
        }
    }
}

/// A single bank tab including its items — cached from server snapshots.
#[derive(Clone, Debug)]
pub struct BankTabData {
    pub tab_idx: u8,
    pub display_name: String,
    pub tag_filters: Vec<String>,
    pub items: Vec<BankItemData>,
}

/// A single bank item slot.
#[derive(Clone, Debug)]
pub struct BankItemData {
    pub slot_idx: usize,
    pub item_id: String,
    pub quantity: i64,
    /// The physical tab this item lives in (1-11). Never 0.
    /// Used to route withdrawals correctly when viewing the "All" aggregate tab.
    pub source_tab_idx: u8,
}

// ── Action queue ──────────────────────────────────────────────────────────────

#[derive(Resource, Default)]
pub struct BankItemActionQueue {
    pub actions: Vec<BankItemAction>,
}

#[derive(Clone, Debug)]
pub enum BankItemAction {
    /// Withdraw `quantity` copies of the item at `slot_idx` in `tab_idx`.
    Withdraw {
        tab_idx: u8,
        slot_idx: usize,
        item_id: String,
        quantity: u32,
    },
    /// Deposit inventory slot `inv_slot_idx` into the bank.
    DepositInvSlot {
        inv_slot_idx: usize,
        item_id: String,
        quantity: u32,
    },
    /// Deposit every item in inventory.
    DepositAll,
    /// Close the bank panel.
    Close,
}

// ── Supporting types ──────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct BankContextItem {
    pub tab_idx: u8,
    pub slot_idx: usize,
    pub item_id: String,
    pub display_name: String,
    pub quantity: i64,
}

// ── Marker components ─────────────────────────────────────────────────────────

#[derive(Component)]
pub(crate) struct BankPanelRoot;

#[derive(Component)]
pub(crate) struct BankCloseButton;

#[derive(Component)]
pub(crate) struct BankTabButton {
    tab_idx: u8,
}

#[derive(Component)]
pub(crate) struct BankItemSlotButton {
    /// The physical tab this item is stored in (never 0).
    source_tab_idx: u8,
    slot_idx: usize,
    item_id: String,
}

#[derive(Component)]
pub(crate) struct BankDepositAllButton;

#[derive(Component)]
pub(crate) struct BankContextMenuRoot;

#[derive(Component)]
pub(crate) struct BankContextOptionButton {
    action: WithdrawAmount,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum WithdrawAmount {
    One,
    Five,
    Ten,
    All,
}

// ── Systems ───────────────────────────────────────────────────────────────────

/// Main sync system: spawns / despawns / rebuilds the bank panel.
pub fn bank_panel_sync_system(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut state: ResMut<BankUiState>,
    mut blocker: ResMut<WorldInteractionBlocker>,
    windows: Query<&Window, With<PrimaryWindow>>,
    player_q: Query<&Inventory, With<Player>>,
) {
    if !state.open {
        despawn_bank_panel(&mut commands, &mut state);
        return;
    }

    // Latch inventory slot count.
    if let Ok(inv) = player_q.single() {
        state.inv_slot_count = inv.container.slots.len();
    }

    // Block world interaction when cursor is over the panel.
    blocker.0 = blocker.0 || cursor_over_bank_panel(&windows, &state);

    if !state.needs_rebuild && state.root.is_some() {
        return;
    }

    // Rebuild.
    despawn_bank_panel(&mut commands, &mut state);

    let tabs_snapshot = state.tabs.clone();
    let active_tab = state.active_tab_idx;

    let root = spawn_bank_panel(&mut commands, &asset_server, &tabs_snapshot, active_tab);
    state.root = Some(root);
    state.needs_rebuild = false;
}

/// Handles button clicks inside the bank panel.
pub fn bank_interaction_system(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut state: ResMut<BankUiState>,
    mut action_queue: ResMut<BankItemActionQueue>,
    _player_q: Query<&Inventory, With<Player>>,
    // Interaction queries
    mut close_q: Query<&Interaction, (Changed<Interaction>, With<BankCloseButton>)>,
    mut tab_q: Query<(&Interaction, &BankTabButton), (Changed<Interaction>, With<Button>)>,
    mut deposit_all_q: Query<&Interaction, (Changed<Interaction>, With<BankDepositAllButton>)>,
    mut item_slot_q: Query<(&Interaction, &BankItemSlotButton), (Changed<Interaction>, With<Button>)>,
    mut ctx_q: Query<(&Interaction, &BankContextOptionButton), (Changed<Interaction>, With<Button>)>,
) {
    if !state.open {
        return;
    }

    // X button: close.
    for interaction in close_q.iter_mut() {
        if *interaction == Interaction::Pressed {
            action_queue.actions.push(BankItemAction::Close);
            state.open = false;
            state.needs_rebuild = true;
            return;
        }
    }

    // Deposit All button.
    for interaction in deposit_all_q.iter_mut() {
        if *interaction == Interaction::Pressed {
            action_queue.actions.push(BankItemAction::DepositAll);
            close_bank_context_menu(&mut commands, &mut state);
            return;
        }
    }

    // Tab buttons.
    for (interaction, tab_btn) in tab_q.iter_mut() {
        if *interaction == Interaction::Pressed && tab_btn.tab_idx != state.active_tab_idx {
            state.active_tab_idx = tab_btn.tab_idx;
            state.needs_rebuild = true;
            close_bank_context_menu(&mut commands, &mut state);
            return;
        }
    }

    // Left-click bank item slot → withdraw 1.
    for (interaction, slot_btn) in item_slot_q.iter_mut() {
        if *interaction == Interaction::Pressed {
            close_bank_context_menu(&mut commands, &mut state);
            action_queue.actions.push(BankItemAction::Withdraw {
                tab_idx: slot_btn.source_tab_idx,
                slot_idx: slot_btn.slot_idx,
                item_id: slot_btn.item_id.clone(),
                quantity: 1,
            });
            return;
        }
    }

    // Right-click item slots: open withdraw context menu.
    if mouse.just_pressed(MouseButton::Right) {
        // Detect right-click via manual hit-testing (Bevy's Changed<Interaction> doesn't fire on right-click).
        if let Some((_display_tab, slot_idx, menu_pos)) = bank_slot_at_cursor(&windows, &state) {
            let active = state.active_tab_idx;
            let items = items_for_active_tab(&state, active);
            if let Some(item) = items.into_iter().find(|i| i.slot_idx == slot_idx) {
                let ctx = BankContextItem {
                    tab_idx: item.source_tab_idx, // always the real physical tab
                    slot_idx,
                    item_id: item.item_id.clone(),
                    display_name: item_display_name(&item.item_id),
                    quantity: item.quantity,
                };
                open_bank_context_menu(&mut commands, &asset_server, &mut state, ctx, menu_pos);
            }
        }
    }

    // Dismiss context menu on left-click outside it.
    if mouse.just_pressed(MouseButton::Left) {
        if state.context_menu_root.is_some() {
            // If no ctx_q interaction fired (pressed outside), dismiss.
            let mut any_ctx_pressed = false;
            for (interaction, _) in ctx_q.iter_mut() {
                if *interaction == Interaction::Pressed {
                    any_ctx_pressed = true;
                }
            }
            if !any_ctx_pressed {
                close_bank_context_menu(&mut commands, &mut state);
            }
        }
    }

    // Context menu option buttons.
    for (interaction, opt) in ctx_q.iter_mut() {
        if *interaction != Interaction::Pressed {
            continue;
        }

        let Some(item) = state.context_item.clone() else {
            close_bank_context_menu(&mut commands, &mut state);
            continue;
        };

        let qty: u32 = match opt.action {
            WithdrawAmount::One => 1,
            WithdrawAmount::Five => 5,
            WithdrawAmount::Ten => 10,
            WithdrawAmount::All => item.quantity.max(0) as u32,
        };

        action_queue.actions.push(BankItemAction::Withdraw {
            tab_idx: item.tab_idx,
            slot_idx: item.slot_idx,
            item_id: item.item_id.clone(),
            quantity: qty,
        });

        close_bank_context_menu(&mut commands, &mut state);
    }
}

// ── Panel construction ────────────────────────────────────────────────────────

fn spawn_bank_panel(
    commands: &mut Commands,
    asset_server: &Res<AssetServer>,
    tabs: &[BankTabData],
    active_tab: u8,
) -> Entity {
    let font: Handle<Font> = asset_server.load("fonts/ui.ttf");

    // Determine items to display.
    let display_items: Vec<BankItemData> = if active_tab == 0 {
        // "All" tab: flatten all tab items, keeping unique item_ids (sum quantities).
        aggregate_all_items(tabs)
    } else {
        tabs.iter()
            .find(|t| t.tab_idx == active_tab)
            .map(|t| t.items.clone())
            .unwrap_or_default()
    };

    let grid_rows = (display_items.len().max(1) + GRID_COLS - 1) / GRID_COLS;
    let grid_h = grid_rows as f32 * (SLOT_SIZE + SLOT_GAP) + SLOT_GAP;
    let panel_h = (PANEL_PADDING * 2.0
        + 40.0  // title bar
        + 8.0   // gap
        + 36.0  // tab bar
        + 8.0   // gap
        + 36.0  // toolbar (search + deposit all)
        + 8.0   // gap
        + grid_h)
        .max(PANEL_MIN_HEIGHT);

    // Root — centered on screen using absolute positioning tricks.
    // We rely on percentage sizing and Flexbox centering on a full-screen invisible wrapper.
    let wrapper = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                display: Display::Flex,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            // Let clicks pass through the invisible wrapper (only the inner panel blocks them).
            Pickable::IGNORE,
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.35)),
            BankPanelRoot,
            Name::new("bank_panel_wrapper"),
        ))
        .id();

    let panel = commands
        .spawn((
            Node {
                width: Val::Px(PANEL_WIDTH),
                height: Val::Px(panel_h),
                padding: UiRect::all(Val::Px(PANEL_PADDING)),
                display: Display::Flex,
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(8.0),
                border_radius: BorderRadius::all(Val::Px(8.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.05, 0.04, 0.035, 0.97)),
            Name::new("bank_panel"),
        ))
        .id();

    commands.entity(wrapper).add_child(panel);

    // ── Title bar ──────────────────────────────────────────────────────────────
    let title_row = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(32.0),
                display: Display::Flex,
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                ..default()
            },
            Pickable::IGNORE,
            Name::new("bank_title_row"),
        ))
        .id();

    let title_text = commands
        .spawn((
            Text::new("Bank"),
            TextFont { font: font.clone(), font_size: 20.0, ..default() },
            TextColor(Color::srgb(0.95, 0.88, 0.60)),
            Pickable::IGNORE,
            Name::new("bank_title_text"),
        ))
        .id();

    let close_btn = commands
        .spawn((
            Button,
            Node {
                width: Val::Px(28.0),
                height: Val::Px(28.0),
                display: Display::Flex,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.30, 0.06, 0.06, 0.92)),
            BankCloseButton,
            Name::new("bank_close_btn"),
        ))
        .id();

    let close_text = commands
        .spawn((
            Text::new("X"),
            TextFont { font: font.clone(), font_size: 16.0, ..default() },
            TextColor(Color::srgb(0.92, 0.65, 0.65)),
            Pickable::IGNORE,
            Name::new("bank_close_text"),
        ))
        .id();

    commands.entity(close_btn).add_child(close_text);
    commands.entity(title_row).add_child(title_text);
    commands.entity(title_row).add_child(close_btn);
    commands.entity(panel).add_child(title_row);

    // ── Tab bar ────────────────────────────────────────────────────────────────
    let tab_bar = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(34.0),
                display: Display::Flex,
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(4.0),
                align_items: AlignItems::Center,
                overflow: Overflow::clip(),
                ..default()
            },
            Pickable::IGNORE,
            Name::new("bank_tab_bar"),
        ))
        .id();

    // "All" tab (tab_idx = 0).
    let all_tab = spawn_tab_button(commands, &font, 0, "All", active_tab == 0);
    commands.entity(tab_bar).add_child(all_tab);

    // Physical tabs from the server.
    for tab in tabs.iter() {
        let btn = spawn_tab_button(commands, &font, tab.tab_idx, &tab.display_name, active_tab == tab.tab_idx);
        commands.entity(tab_bar).add_child(btn);
    }

    commands.entity(panel).add_child(tab_bar);

    // ── Toolbar (search placeholder + Deposit All) ─────────────────────────────
    let toolbar = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(34.0),
                display: Display::Flex,
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(8.0),
                align_items: AlignItems::Center,
                ..default()
            },
            Pickable::IGNORE,
            Name::new("bank_toolbar"),
        ))
        .id();

    // Search placeholder (static label for now — proper text input in a later pass).
    let search_box = commands
        .spawn((
            Node {
                flex_grow: 1.0,
                height: Val::Px(30.0),
                padding: UiRect::horizontal(Val::Px(8.0)),
                display: Display::Flex,
                align_items: AlignItems::Center,
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.08, 0.07, 0.06, 0.95)),
            Pickable::IGNORE,
            Name::new("bank_search_box"),
        ))
        .id();

    let search_label = commands
        .spawn((
            Text::new("Search..."),
            TextFont { font: font.clone(), font_size: 13.0, ..default() },
            TextColor(Color::srgba(0.55, 0.52, 0.48, 1.0)),
            Pickable::IGNORE,
            Name::new("bank_search_label"),
        ))
        .id();
    commands.entity(search_box).add_child(search_label);

    let deposit_all_btn = commands
        .spawn((
            Button,
            Node {
                height: Val::Px(30.0),
                padding: UiRect::horizontal(Val::Px(14.0)),
                display: Display::Flex,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.12, 0.22, 0.12, 0.95)),
            BankDepositAllButton,
            Name::new("bank_deposit_all_btn"),
        ))
        .id();

    let deposit_all_text = commands
        .spawn((
            Text::new("Deposit All"),
            TextFont { font: font.clone(), font_size: 13.0, ..default() },
            TextColor(Color::srgb(0.72, 0.92, 0.72)),
            Pickable::IGNORE,
            Name::new("bank_deposit_all_text"),
        ))
        .id();
    commands.entity(deposit_all_btn).add_child(deposit_all_text);

    commands.entity(toolbar).add_child(search_box);
    commands.entity(toolbar).add_child(deposit_all_btn);
    commands.entity(panel).add_child(toolbar);

    // ── Item grid ──────────────────────────────────────────────────────────────
    let grid = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                display: Display::Flex,
                flex_direction: FlexDirection::Row,
                flex_wrap: FlexWrap::Wrap,
                column_gap: Val::Px(SLOT_GAP),
                row_gap: Val::Px(SLOT_GAP),
                align_content: AlignContent::FlexStart,
                ..default()
            },
            Pickable::IGNORE,
            Name::new("bank_item_grid"),
        ))
        .id();

    for item in display_items.iter() {
        let slot_ent = spawn_bank_item_slot(commands, asset_server, &font, item);
        commands.entity(grid).add_child(slot_ent);
    }

    commands.entity(panel).add_child(grid);

    wrapper
}

fn spawn_tab_button(
    commands: &mut Commands,
    font: &Handle<Font>,
    tab_idx: u8,
    label: &str,
    is_active: bool,
) -> Entity {
    let bg = if is_active {
        Color::srgba(0.18, 0.14, 0.08, 0.98)
    } else {
        Color::srgba(0.09, 0.08, 0.07, 0.90)
    };
    let text_color = if is_active {
        Color::srgb(0.95, 0.86, 0.55)
    } else {
        Color::srgb(0.70, 0.68, 0.60)
    };

    let btn = commands
        .spawn((
            Button,
            Node {
                height: Val::Px(28.0),
                padding: UiRect::horizontal(Val::Px(10.0)),
                display: Display::Flex,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(bg),
            BankTabButton { tab_idx },
            Name::new(format!("bank_tab_{}", tab_idx)),
        ))
        .id();

    let text = commands
        .spawn((
            Text::new(label.to_string()),
            TextFont { font: font.clone(), font_size: 13.0, ..default() },
            TextColor(text_color),
            Pickable::IGNORE,
            Name::new(format!("bank_tab_text_{}", tab_idx)),
        ))
        .id();

    commands.entity(btn).add_child(text);
    btn
}

fn spawn_bank_item_slot(
    commands: &mut Commands,
    asset_server: &Res<AssetServer>,
    font: &Handle<Font>,
    item: &BankItemData,
) -> Entity {
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
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.07, 0.06, 0.05, 0.96)),
            BankItemSlotButton {
                source_tab_idx: item.source_tab_idx,
                slot_idx: item.slot_idx,
                item_id: item.item_id.clone(),
            },
            Name::new(format!("bank_item_slot_{}", item.slot_idx)),
        ))
        .id();

    // Try to show an icon; fall back to text label.
    let icon_path = inventory_icon_path(&item.item_id);
    if let Some(path) = icon_path {
        let icon = commands
            .spawn((
                ImageNode::new(asset_server.load(path)),
                Node {
                    width: Val::Px(ITEM_ICON_SIZE),
                    height: Val::Px(ITEM_ICON_SIZE),
                    ..default()
                },
                Pickable::IGNORE,
                Name::new(format!("bank_item_icon_{}", item.slot_idx)),
            ))
            .id();
        commands.entity(slot).add_child(icon);
    } else {
        let label = commands
            .spawn((
                Text::new(item_display_name(&item.item_id)),
                TextFont { font: font.clone(), font_size: 8.0, ..default() },
                TextColor(Color::srgb(0.88, 0.84, 0.72)),
                Pickable::IGNORE,
                Name::new(format!("bank_item_fallback_{}", item.slot_idx)),
            ))
            .id();
        commands.entity(slot).add_child(label);
    }

    // Quantity overlay (bottom-right corner).
    if item.quantity != 1 {
        let qty_text = if item.quantity >= 10_000_000 {
            let m = item.quantity / 1_000_000;
            format!("{}M", m)
        } else if item.quantity >= 100_000 {
            let k = item.quantity / 1_000;
            format!("{}K", k)
        } else {
            item.quantity.to_string()
        };

        let text_color = if item.quantity >= 10_000_000 {
            Color::srgb(0.0, 1.0, 0.6) // green-ish for large stacks
        } else if item.quantity >= 100_000 {
            Color::srgb(1.0, 1.0, 0.3) // yellow for medium stacks
        } else {
            Color::WHITE
        };

        let qty_label = commands
            .spawn((
                Text::new(qty_text),
                TextFont { font: font.clone(), font_size: 10.0, ..default() },
                TextColor(text_color),
                Pickable::IGNORE,
                Name::new(format!("bank_item_qty_{}", item.slot_idx)),
            ))
            .id();
        commands.entity(slot).add_child(qty_label);
    }

    slot
}

fn open_bank_context_menu(
    commands: &mut Commands,
    asset_server: &Res<AssetServer>,
    state: &mut ResMut<BankUiState>,
    item: BankContextItem,
    menu_pos: Vec2,
) {
    close_bank_context_menu(commands, state);

    let Some(root) = state.root else { return; };

    let font: Handle<Font> = asset_server.load("fonts/ui.ttf");

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
            ZIndex(10),
            BankContextMenuRoot,
            Name::new("bank_context_menu"),
        ))
        .id();
    commands.entity(root).add_child(menu);

    let title = commands
        .spawn((
            Text::new(item.display_name.clone()),
            TextFont { font: font.clone(), font_size: 15.0, ..default() },
            TextColor(Color::srgb(0.92, 0.86, 0.64)),
            Pickable::IGNORE,
            Name::new("bank_ctx_title"),
        ))
        .id();
    commands.entity(menu).add_child(title);

    let options: &[(&str, WithdrawAmount)] = &[
        ("Withdraw 1", WithdrawAmount::One),
        ("Withdraw 5", WithdrawAmount::Five),
        ("Withdraw 10", WithdrawAmount::Ten),
        ("Withdraw All", WithdrawAmount::All),
    ];

    for (label, amount) in options {
        let btn = commands
            .spawn((
                Button,
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Px(30.0),
                    justify_content: JustifyContent::FlexStart,
                    align_items: AlignItems::Center,
                    padding: UiRect::horizontal(Val::Px(8.0)),
                    border_radius: BorderRadius::all(Val::Px(4.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.10, 0.10, 0.12, 0.95)),
                BankContextOptionButton { action: *amount },
                Name::new(format!("bank_ctx_opt_{label}")),
            ))
            .id();

        let btn_text = commands
            .spawn((
                Text::new(*label),
                TextFont { font: font.clone(), font_size: 14.0, ..default() },
                TextColor(Color::srgb(0.88, 0.88, 0.88)),
                Pickable::IGNORE,
                Name::new(format!("bank_ctx_opt_text_{label}")),
            ))
            .id();

        commands.entity(btn).add_child(btn_text);
        commands.entity(menu).add_child(btn);
    }

    state.context_menu_root = Some(menu);
    state.context_item = Some(item);
}

fn close_bank_context_menu(commands: &mut Commands, state: &mut ResMut<BankUiState>) {
    if let Some(menu) = state.context_menu_root.take() {
        if let Ok(mut ec) = commands.get_entity(menu) {
            ec.despawn();
        }
    }
    state.context_item = None;
}

fn despawn_bank_panel(commands: &mut Commands, state: &mut ResMut<BankUiState>) {
    if let Some(root) = state.root.take() {
        if let Ok(mut ec) = commands.get_entity(root) {
            ec.despawn();
        }
    }
    state.context_menu_root = None;
    state.context_item = None;
    state.needs_rebuild = false;
}

// ── Hit-testing ───────────────────────────────────────────────────────────────

fn cursor_over_bank_panel(
    windows: &Query<&Window, With<PrimaryWindow>>,
    state: &BankUiState,
) -> bool {
    if !state.open || state.root.is_none() {
        return false;
    }
    let Ok(window) = windows.single() else { return false; };
    let Some(cursor) = window.cursor_position() else { return false; };

    // Approximate: panel is centered.
    let cx = window.width() * 0.5;
    let cy = window.height() * 0.5;
    cursor.x >= cx - PANEL_WIDTH * 0.5
        && cursor.x <= cx + PANEL_WIDTH * 0.5
        && cursor.y >= cy - PANEL_MIN_HEIGHT * 0.5
        && cursor.y <= cy + PANEL_MIN_HEIGHT * 0.5
}

/// Returns (tab_idx, slot_idx, menu_pos) for the bank item the cursor is over.
/// Used for right-click context menu positioning.
fn bank_slot_at_cursor(
    windows: &Query<&Window, With<PrimaryWindow>>,
    state: &BankUiState,
) -> Option<(u8, usize, Vec2)> {
    let window = windows.single().ok()?;
    let cursor = window.cursor_position()?;

    // Approximate panel center.
    let cx = window.width() * 0.5;
    let cy = window.height() * 0.5;

    let panel_left = cx - PANEL_WIDTH * 0.5 + PANEL_PADDING;
    let panel_top_approx = cy - PANEL_MIN_HEIGHT * 0.5
        + PANEL_PADDING
        + 40.0   // title
        + 8.0
        + 34.0   // tab bar
        + 8.0
        + 34.0   // toolbar
        + 8.0;

    let local_x = cursor.x - panel_left;
    let local_y = cursor.y - panel_top_approx;
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

    // Verify the slot exists in the active tab's items.
    let active = state.active_tab_idx;
    let items = items_for_active_tab(state, active);
    if !items.iter().any(|i| i.slot_idx == slot_idx) {
        return None;
    }

    let menu_x = (panel_left + col as f32 * pitch + SLOT_SIZE + 4.0)
        .min(window.width() - MENU_WIDTH - 4.0)
        .max(0.0);
    let menu_y = (panel_top_approx + row as f32 * pitch)
        .min(window.height() - 160.0)
        .max(0.0);

    Some((active, slot_idx, Vec2::new(menu_x, menu_y)))
}

// ── Data helpers ──────────────────────────────────────────────────────────────

/// Returns the items to display for the given tab_idx (0 = all).
fn items_for_active_tab(state: &BankUiState, tab_idx: u8) -> Vec<BankItemData> {
    if tab_idx == 0 {
        aggregate_all_items(&state.tabs)
    } else {
        state.tabs.iter()
            .find(|t| t.tab_idx == tab_idx)
            .map(|t| t.items.clone())
            .unwrap_or_default()
    }
}

/// Flatten all tabs' items, summing quantities for duplicate item_ids.
/// Each entry retains `source_tab_idx` from whichever tab it first appeared in,
/// so that withdrawals issued from the "All" view are routed to the correct physical tab.
fn aggregate_all_items(tabs: &[BankTabData]) -> Vec<BankItemData> {
    use std::collections::BTreeMap;

    // Map item_id → first-seen item (with its real source_tab_idx), summing quantities.
    let mut by_id: BTreeMap<String, BankItemData> = BTreeMap::new();

    for tab in tabs.iter() {
        for item in tab.items.iter() {
            by_id.entry(item.item_id.clone())
                .and_modify(|e| e.quantity += item.quantity)
                .or_insert_with(|| item.clone());
        }
    }

    // Re-number slot_idx sequentially so the grid renders contiguously.
    by_id.into_values()
        .enumerate()
        .map(|(i, mut item)| { item.slot_idx = i; item })
        .collect()
}

fn inventory_icon_path(item_id: &str) -> Option<String> {
    default_item_defs().get(item_id).and_then(|d| d.inventory_icon.clone())
}

fn item_display_name(item_id: &str) -> String {
    default_item_defs()
        .get(item_id)
        .map(|d| d.name.clone())
        .unwrap_or_else(|| item_id.to_string())
}
