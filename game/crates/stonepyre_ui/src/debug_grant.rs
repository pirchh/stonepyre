use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use stonepyre_content::default_item_defs;
use stonepyre_engine::plugins::interaction::WorldInteractionBlocker;

use crate::config::UiBindings;

// ── Layout constants ──────────────────────────────────────────────────────────

const PANEL_LEFT: f32 = 10.0;
const PANEL_TOP: f32 = 145.0; // below the game-net debug overlay
const PANEL_WIDTH: f32 = 440.0;
const PANEL_HEIGHT_EST: f32 = 420.0; // generous estimate for blocker check

// ── Categories ────────────────────────────────────────────────────────────────

const CATEGORIES: &[&str] = &["All", "Bags", "Woodcutting", "Upgrades"];

fn item_category(tags: &[String]) -> &'static str {
    if tags.iter().any(|t| t == "bag" || t == "bag_general" || t == "bag_typed") {
        "Bags"
    } else if tags.iter().any(|t| t == "log") {
        "Woodcutting"
    } else if tags.iter().any(|t| t == "bag_upgrade") {
        "Upgrades"
    } else {
        "Other"
    }
}

// ── Resources ─────────────────────────────────────────────────────────────────

/// Set by stonepyre_app when a session is established.
#[derive(Resource, Default)]
pub struct IsAdminAccount(pub bool);

#[derive(Resource)]
pub struct DebugGrantUiState {
    pub open: bool,
    pub root: Option<Entity>,
    pub spawned: Vec<Entity>,
    pub needs_rebuild: bool,
    pub selected_item: Option<String>,
    pub selected_category: String,
    pub quantity: u32,
    pub status: String,
}

impl Default for DebugGrantUiState {
    fn default() -> Self {
        Self {
            open: false,
            root: None,
            spawned: Vec::new(),
            needs_rebuild: false,
            selected_item: None,
            selected_category: "All".to_string(),
            quantity: 1,
            status: String::new(),
        }
    }
}

/// Written by the UI; consumed by stonepyre_app's send_debug_grant_actions.
#[derive(Resource, Default)]
pub struct DebugGrantActionQueue {
    pub pending_grant: Option<DebugGrantRequest>,
}

pub struct DebugGrantRequest {
    pub item_id: String,
    pub quantity: u32,
}

// ── Components ────────────────────────────────────────────────────────────────

#[derive(Component)]
struct DebugGrantRoot;

#[derive(Component)]
pub(crate) struct DebugGrantItemButton {
    pub item_id: String,
}

#[derive(Component)]
pub(crate) struct DebugGrantCategoryButton {
    pub category: String,
}

#[derive(Component)]
pub(crate) struct DebugGrantQtyButton {
    pub delta: i32,
}

#[derive(Component)]
pub(crate) struct DebugGrantConfirmButton;

#[derive(Component)]
pub(crate) struct DebugGrantStatusText;

// ── Systems ───────────────────────────────────────────────────────────────────

pub fn debug_grant_toggle_system(
    keys: Res<ButtonInput<KeyCode>>,
    binds: Res<UiBindings>,
    is_admin: Res<IsAdminAccount>,
    mut state: ResMut<DebugGrantUiState>,
) {
    if !is_admin.0 {
        return;
    }
    if keys.just_pressed(binds.toggle_debug_grant) {
        state.open = !state.open;
        state.needs_rebuild = true;
    }
}

pub fn debug_grant_panel_sync_system(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    is_admin: Res<IsAdminAccount>,
    mut state: ResMut<DebugGrantUiState>,
    mut action_queue: ResMut<DebugGrantActionQueue>,
    mut blocker: ResMut<WorldInteractionBlocker>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut item_btn_q: Query<
        (&Interaction, &DebugGrantItemButton),
        (Changed<Interaction>, With<Button>),
    >,
    mut cat_btn_q: Query<
        (&Interaction, &DebugGrantCategoryButton),
        (Changed<Interaction>, With<Button>),
    >,
    mut qty_btn_q: Query<
        (&Interaction, &DebugGrantQtyButton),
        (Changed<Interaction>, With<Button>),
    >,
    mut confirm_q: Query<
        &Interaction,
        (Changed<Interaction>, With<DebugGrantConfirmButton>, With<Button>),
    >,
    mut status_q: Query<&mut Text, With<DebugGrantStatusText>>,
) {
    if !is_admin.0 || !state.open {
        if state.root.is_some() {
            despawn_all(&mut commands, &mut state);
        }
        return;
    }

    blocker.0 = blocker.0 || cursor_over_panel(&windows);

    if state.root.is_none() || state.needs_rebuild {
        despawn_all(&mut commands, &mut state);
        spawn_panel(&mut commands, &asset_server, &mut state);
        state.needs_rebuild = false;
        return;
    }

    // Category selection.
    for (interaction, btn) in cat_btn_q.iter_mut() {
        if *interaction == Interaction::Pressed && btn.category != state.selected_category {
            state.selected_category = btn.category.clone();
            state.selected_item = None;
            state.needs_rebuild = true;
            return;
        }
    }

    // Item selection — only one at a time, force rebuild to re-colour buttons.
    for (interaction, btn) in item_btn_q.iter_mut() {
        if *interaction == Interaction::Pressed {
            if state.selected_item.as_deref() == Some(btn.item_id.as_str()) {
                // Clicking the already-selected item deselects it.
                state.selected_item = None;
            } else {
                state.selected_item = Some(btn.item_id.clone());
            }
            state.status.clear();
            state.needs_rebuild = true;
            return;
        }
    }

    // Quantity adjustment.
    for (interaction, qty_btn) in qty_btn_q.iter_mut() {
        if *interaction == Interaction::Pressed {
            let new_qty = (state.quantity as i32 + qty_btn.delta).max(1).min(9999) as u32;
            if new_qty != state.quantity {
                state.quantity = new_qty;
                state.needs_rebuild = true;
            }
            return;
        }
    }

    // Grant confirmation.
    for interaction in confirm_q.iter_mut() {
        if *interaction == Interaction::Pressed {
            if let Some(ref item_id) = state.selected_item.clone() {
                let defs = default_item_defs();
                let display = defs
                    .get(item_id)
                    .map(|d| d.name.as_str())
                    .unwrap_or(item_id.as_str())
                    .to_string();
                action_queue.pending_grant = Some(DebugGrantRequest {
                    item_id: item_id.clone(),
                    quantity: state.quantity,
                });
                state.status = format!("Granting {}x {}...", state.quantity, display);
                state.needs_rebuild = true;
            } else {
                state.status = "Select an item first.".to_string();
                state.needs_rebuild = true;
            }
            return;
        }
    }

    // Live status text update (without full rebuild).
    for mut text in status_q.iter_mut() {
        if text.0 != state.status {
            text.0 = state.status.clone();
        }
    }
}

// ── Panel construction ────────────────────────────────────────────────────────

fn spawn_panel(
    commands: &mut Commands,
    asset_server: &Res<AssetServer>,
    state: &mut ResMut<DebugGrantUiState>,
) {
    let font = asset_server.load("fonts/ui.ttf");

    // Build filtered + sorted item list.
    let defs = default_item_defs();
    let mut items: Vec<(String, String)> = defs
        .items
        .iter()
        .filter(|(_, def)| {
            state.selected_category == "All"
                || item_category(&def.tags) == state.selected_category
        })
        .map(|(id, def)| (id.clone(), def.name.clone()))
        .collect();
    items.sort_by(|a, b| a.1.cmp(&b.1)); // sort by friendly name

    let root = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(PANEL_LEFT),
                top: Val::Px(PANEL_TOP),
                width: Val::Px(PANEL_WIDTH),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(10.0)),
                row_gap: Val::Px(6.0),
                border_radius: BorderRadius::all(Val::Px(8.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.06, 0.06, 0.08, 0.97)),
            DebugGrantRoot,
            Name::new("debug_grant_root"),
        ))
        .id();

    // Title
    spawn_text(commands, root, &font, "Debug Item Grant  [F2 to close]", 13.0,
        Color::srgb(0.80, 0.70, 0.40));

    // Category buttons
    let cat_row = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(4.0),
                ..default()
            },
        ))
        .id();
    commands.entity(root).add_child(cat_row);

    for &cat in CATEGORIES {
        let is_active = cat == state.selected_category;
        let bg = if is_active {
            Color::srgba(0.20, 0.20, 0.30, 0.98)
        } else {
            Color::srgba(0.10, 0.10, 0.14, 0.95)
        };
        let btn = commands
            .spawn((
                Button,
                Node {
                    padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)),
                    border_radius: BorderRadius::all(Val::Px(4.0)),
                    ..default()
                },
                BackgroundColor(bg),
                DebugGrantCategoryButton { category: cat.to_string() },
            ))
            .id();
        let lbl = commands
            .spawn((
                Text::new(cat),
                TextFont { font: font.clone(), font_size: 11.0, ..default() },
                TextColor(if is_active { Color::WHITE } else { Color::srgb(0.70, 0.70, 0.70) }),
                Pickable::IGNORE,
            ))
            .id();
        commands.entity(btn).add_child(lbl);
        commands.entity(cat_row).add_child(btn);
    }

    // Item grid
    let grid = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                flex_wrap: FlexWrap::Wrap,
                column_gap: Val::Px(4.0),
                row_gap: Val::Px(4.0),
                ..default()
            },
        ))
        .id();
    commands.entity(root).add_child(grid);

    if items.is_empty() {
        let empty = commands
            .spawn((
                Text::new("No items in this category."),
                TextFont { font: font.clone(), font_size: 11.0, ..default() },
                TextColor(Color::srgb(0.55, 0.55, 0.55)),
            ))
            .id();
        commands.entity(grid).add_child(empty);
    } else {
        for (item_id, display_name) in &items {
            let is_selected = state.selected_item.as_deref() == Some(item_id.as_str());
            let bg = if is_selected {
                Color::srgba(0.18, 0.32, 0.18, 0.98)
            } else {
                Color::srgba(0.10, 0.10, 0.13, 0.95)
            };
            let btn = commands
                .spawn((
                    Button,
                    Node {
                        padding: UiRect::axes(Val::Px(7.0), Val::Px(4.0)),
                        border_radius: BorderRadius::all(Val::Px(4.0)),
                        ..default()
                    },
                    BackgroundColor(bg),
                    DebugGrantItemButton { item_id: item_id.clone() },
                ))
                .id();
            let lbl = commands
                .spawn((
                    Text::new(display_name.clone()),
                    TextFont { font: font.clone(), font_size: 11.0, ..default() },
                    TextColor(if is_selected { Color::WHITE } else { Color::srgb(0.82, 0.82, 0.82) }),
                    Pickable::IGNORE,
                ))
                .id();
            commands.entity(btn).add_child(lbl);
            commands.entity(grid).add_child(btn);
        }
    }

    // Selected item display
    let sel_text = if let Some(ref id) = state.selected_item {
        let name = defs.get(id).map(|d| d.name.as_str()).unwrap_or(id.as_str());
        format!("Selected: {}", name)
    } else {
        "Selected: —".to_string()
    };
    spawn_text(commands, root, &font, &sel_text, 11.0, Color::srgb(0.65, 0.85, 0.65));

    // Quantity row
    let qty_row = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(6.0),
                ..default()
            },
        ))
        .id();
    commands.entity(root).add_child(qty_row);

    spawn_text_child(commands, qty_row, &font, "Qty:", 12.0, Color::srgb(0.72, 0.72, 0.72));

    for (label, delta) in [("-10", -10i32), ("-1", -1), ("+1", 1), ("+10", 10)] {
        let btn = commands
            .spawn((
                Button,
                Node {
                    width: Val::Px(34.0),
                    height: Val::Px(24.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    border_radius: BorderRadius::all(Val::Px(3.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.14, 0.14, 0.18, 0.95)),
                DebugGrantQtyButton { delta },
            ))
            .id();
        let txt = commands
            .spawn((
                Text::new(label),
                TextFont { font: font.clone(), font_size: 11.0, ..default() },
                TextColor(Color::srgb(0.88, 0.88, 0.88)),
                Pickable::IGNORE,
            ))
            .id();
        commands.entity(btn).add_child(txt);
        commands.entity(qty_row).add_child(btn);
    }

    let qty_val = commands
        .spawn((
            Text::new(format!("{}", state.quantity)),
            TextFont { font: font.clone(), font_size: 12.0, ..default() },
            TextColor(Color::WHITE),
        ))
        .id();
    commands.entity(qty_row).add_child(qty_val);

    // Grant button
    let grant_btn = commands
        .spawn((
            Button,
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(32.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                border_radius: BorderRadius::all(Val::Px(5.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.14, 0.28, 0.14, 0.98)),
            DebugGrantConfirmButton,
        ))
        .id();
    let grant_lbl = commands
        .spawn((
            Text::new("Grant Item"),
            TextFont { font: font.clone(), font_size: 13.0, ..default() },
            TextColor(Color::srgb(0.75, 1.0, 0.75)),
            Pickable::IGNORE,
        ))
        .id();
    commands.entity(grant_btn).add_child(grant_lbl);
    commands.entity(root).add_child(grant_btn);

    // Status line
    let status = commands
        .spawn((
            Text::new(state.status.clone()),
            TextFont { font: font.clone(), font_size: 10.0, ..default() },
            TextColor(Color::srgb(0.60, 0.60, 0.60)),
            DebugGrantStatusText,
        ))
        .id();
    commands.entity(root).add_child(status);

    state.root = Some(root);
    state.spawned.push(root);
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn spawn_text(commands: &mut Commands, parent: Entity, font: &Handle<Font>, text: &str, size: f32, color: Color) {
    let e = commands
        .spawn((
            Text::new(text.to_string()),
            TextFont { font: font.clone(), font_size: size, ..default() },
            TextColor(color),
        ))
        .id();
    commands.entity(parent).add_child(e);
}

fn spawn_text_child(commands: &mut Commands, parent: Entity, font: &Handle<Font>, text: &str, size: f32, color: Color) {
    spawn_text(commands, parent, font, text, size, color);
}

fn cursor_over_panel(windows: &Query<&Window, With<PrimaryWindow>>) -> bool {
    let Ok(window) = windows.single() else { return false; };
    let Some(cursor) = window.cursor_position() else { return false; };
    cursor.x >= PANEL_LEFT
        && cursor.x <= PANEL_LEFT + PANEL_WIDTH
        && cursor.y >= PANEL_TOP
        && cursor.y <= PANEL_TOP + PANEL_HEIGHT_EST
}

fn despawn_all(commands: &mut Commands, state: &mut ResMut<DebugGrantUiState>) {
    for e in state.spawned.drain(..) {
        if let Ok(mut ec) = commands.get_entity(e) {
            ec.despawn();
        }
    }
    state.root = None;
    state.needs_rebuild = false;
}
