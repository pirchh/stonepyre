use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use stonepyre_content::default_item_defs;

use crate::config::UiBindings;

// ── Resources ────────────────────────────────────────────────────────────────

/// Set by stonepyre_app when the session is established. Drives whether
/// the debug grant panel is accessible at all.
#[derive(Resource, Default)]
pub struct IsAdminAccount(pub bool);

#[derive(Resource, Default)]
pub struct DebugGrantUiState {
    pub open: bool,
    pub root: Option<Entity>,
    pub spawned: Vec<Entity>,
    pub needs_rebuild: bool,
    pub selected_item: Option<String>,
    pub quantity: u32,
    pub status: String,
}

/// Written by the UI system; consumed by stonepyre_app's send_debug_grant_actions.
#[derive(Resource, Default)]
pub struct DebugGrantActionQueue {
    pub pending_grant: Option<DebugGrantRequest>,
}

pub struct DebugGrantRequest {
    pub item_id: String,
    pub quantity: u32,
}

// ── Components ───────────────────────────────────────────────────────────────

#[derive(Component)]
struct DebugGrantRoot;

#[derive(Component)]
pub(crate) struct DebugGrantItemButton {
    pub item_id: String,
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
        if state.open && state.quantity == 0 {
            state.quantity = 1;
        }
    }
}

pub fn debug_grant_panel_sync_system(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    is_admin: Res<IsAdminAccount>,
    mut state: ResMut<DebugGrantUiState>,
    mut action_queue: ResMut<DebugGrantActionQueue>,
    mut item_btn_q: Query<
        (&Interaction, &DebugGrantItemButton, &mut BackgroundColor),
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

    if state.root.is_none() || state.needs_rebuild {
        despawn_all(&mut commands, &mut state);
        spawn_panel(&mut commands, &asset_server, &mut state);
        state.needs_rebuild = false;
        return;
    }

    // Handle item selection.
    for (interaction, btn, mut bg) in item_btn_q.iter_mut() {
        if *interaction == Interaction::Pressed {
            state.selected_item = Some(btn.item_id.clone());
            state.status.clear();
        }
        let is_selected = state.selected_item.as_deref() == Some(btn.item_id.as_str());
        *bg = if is_selected {
            BackgroundColor(Color::srgba(0.18, 0.28, 0.18, 0.98))
        } else {
            BackgroundColor(Color::srgba(0.08, 0.08, 0.10, 0.95))
        };
    }

    // Handle quantity adjustment.
    for (interaction, qty_btn) in qty_btn_q.iter_mut() {
        if *interaction == Interaction::Pressed {
            let new_qty = (state.quantity as i32 + qty_btn.delta).max(1).min(9999) as u32;
            state.quantity = new_qty;
            state.needs_rebuild = true;
        }
    }

    // Handle grant.
    for interaction in confirm_q.iter_mut() {
        if *interaction == Interaction::Pressed {
            if let Some(ref item_id) = state.selected_item.clone() {
                action_queue.pending_grant = Some(DebugGrantRequest {
                    item_id: item_id.clone(),
                    quantity: state.quantity,
                });
                state.status = format!("Granting {}x {}...", state.quantity, item_id);
                state.needs_rebuild = true;
            } else {
                state.status = "Select an item first.".to_string();
                state.needs_rebuild = true;
            }
        }
    }

    // Push status text update.
    for mut text in status_q.iter_mut() {
        text.0 = state.status.clone();
    }
}

// ── Panel construction ────────────────────────────────────────────────────────

fn spawn_panel(
    commands: &mut Commands,
    asset_server: &Res<AssetServer>,
    state: &mut ResMut<DebugGrantUiState>,
) {
    let font = asset_server.load("fonts/ui.ttf");
    let items: Vec<String> = {
        let mut ids: Vec<String> = default_item_defs().items.keys().cloned().collect();
        ids.sort();
        ids
    };

    let root = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(20.0),
                top: Val::Px(20.0),
                width: Val::Px(480.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(12.0)),
                row_gap: Val::Px(8.0),
                border_radius: BorderRadius::all(Val::Px(8.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.06, 0.06, 0.08, 0.97)),
            DebugGrantRoot,
            Name::new("debug_grant_root"),
        ))
        .id();

    // Title
    let title = commands
        .spawn((
            Text::new("Debug Item Grant  [F2 to close]"),
            TextFont { font: font.clone(), font_size: 14.0, ..default() },
            TextColor(Color::srgb(0.80, 0.70, 0.40)),
        ))
        .id();
    commands.entity(root).add_child(title);

    // Item list grid (4 columns)
    let grid = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                display: Display::Flex,
                flex_direction: FlexDirection::Row,
                flex_wrap: FlexWrap::Wrap,
                column_gap: Val::Px(4.0),
                row_gap: Val::Px(4.0),
                ..default()
            },
            Name::new("debug_grant_item_grid"),
        ))
        .id();
    commands.entity(root).add_child(grid);

    for item_id in &items {
        let is_selected = state.selected_item.as_deref() == Some(item_id.as_str());
        let bg = if is_selected {
            Color::srgba(0.18, 0.28, 0.18, 0.98)
        } else {
            Color::srgba(0.08, 0.08, 0.10, 0.95)
        };

        let btn = commands
            .spawn((
                Button,
                Node {
                    padding: UiRect::axes(Val::Px(6.0), Val::Px(4.0)),
                    border_radius: BorderRadius::all(Val::Px(4.0)),
                    ..default()
                },
                BackgroundColor(bg),
                DebugGrantItemButton { item_id: item_id.clone() },
                Name::new(format!("debug_grant_item_{item_id}")),
            ))
            .id();
        let label = commands
            .spawn((
                Text::new(item_id.clone()),
                TextFont { font: font.clone(), font_size: 11.0, ..default() },
                TextColor(Color::srgb(0.85, 0.85, 0.85)),
                Pickable::IGNORE,
            ))
            .id();
        commands.entity(btn).add_child(label);
        commands.entity(grid).add_child(btn);
    }

    // Quantity row
    let qty_row = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(8.0),
                ..default()
            },
            Name::new("debug_grant_qty_row"),
        ))
        .id();
    commands.entity(root).add_child(qty_row);

    let qty_label = commands
        .spawn((
            Text::new("Quantity:"),
            TextFont { font: font.clone(), font_size: 13.0, ..default() },
            TextColor(Color::srgb(0.75, 0.75, 0.75)),
        ))
        .id();
    commands.entity(qty_row).add_child(qty_label);

    for (label, delta) in [("-10", -10i32), ("-1", -1), ("+1", 1), ("+10", 10)] {
        let btn = commands
            .spawn((
                Button,
                Node {
                    width: Val::Px(36.0),
                    height: Val::Px(26.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    border_radius: BorderRadius::all(Val::Px(4.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.14, 0.14, 0.18, 0.95)),
                DebugGrantQtyButton { delta },
            ))
            .id();
        let txt = commands
            .spawn((
                Text::new(label),
                TextFont { font: font.clone(), font_size: 12.0, ..default() },
                TextColor(Color::srgb(0.88, 0.88, 0.88)),
                Pickable::IGNORE,
            ))
            .id();
        commands.entity(btn).add_child(txt);
        commands.entity(qty_row).add_child(btn);
    }

    let qty_display = commands
        .spawn((
            Text::new(format!("{}", state.quantity)),
            TextFont { font: font.clone(), font_size: 13.0, ..default() },
            TextColor(Color::srgb(1.0, 1.0, 1.0)),
        ))
        .id();
    commands.entity(qty_row).add_child(qty_display);

    // Grant button
    let grant_btn = commands
        .spawn((
            Button,
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(34.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                border_radius: BorderRadius::all(Val::Px(6.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.15, 0.30, 0.15, 0.98)),
            DebugGrantConfirmButton,
            Name::new("debug_grant_confirm_btn"),
        ))
        .id();
    let grant_label = commands
        .spawn((
            Text::new("Grant Item"),
            TextFont { font: font.clone(), font_size: 14.0, ..default() },
            TextColor(Color::srgb(0.80, 1.0, 0.80)),
            Pickable::IGNORE,
        ))
        .id();
    commands.entity(grant_btn).add_child(grant_label);
    commands.entity(root).add_child(grant_btn);

    // Status line
    let status = commands
        .spawn((
            Text::new(state.status.clone()),
            TextFont { font: font.clone(), font_size: 11.0, ..default() },
            TextColor(Color::srgb(0.65, 0.65, 0.65)),
            DebugGrantStatusText,
            Name::new("debug_grant_status"),
        ))
        .id();
    commands.entity(root).add_child(status);

    state.root = Some(root);
    state.spawned.push(root);
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
