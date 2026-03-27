use bevy::prelude::*;

use stonepyre_engine::plugins::inventory::Inventory;

use crate::config::UiBindings;

#[derive(Resource, Default)]
pub struct InventoryUiState {
    pub open: bool,
    pub root: Option<Entity>,
    pub spawned: Vec<Entity>,
    pub needs_rebuild: bool,
}

#[derive(Component)]
struct InventoryPanelRoot;

#[derive(Component)]
pub(crate) struct SlotLabel {
    idx: usize,
}

pub fn inventory_toggle_system(
    keys: Res<ButtonInput<KeyCode>>,
    binds: Res<UiBindings>,
    mut state: ResMut<InventoryUiState>,
) {
    if keys.just_pressed(binds.toggle_inventory) {
        state.open = !state.open;
        state.needs_rebuild = true;
    }
}

pub fn inventory_panel_sync_system(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut state: ResMut<InventoryUiState>,
    player_q: Query<&Inventory>,
    slot_text_q: Query<(Entity, &SlotLabel)>,
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

    // Centered panel (Minecraft-ish)
    let panel = commands
        .spawn((
            Node {
                width: Val::Px(560.0),
                height: Val::Px(620.0),
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
                    Node {
                        width: Val::Px(slot_px),
                        height: Val::Px(slot_px),
                        display: Display::Flex,
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::Center,
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.10, 0.10, 0.12, 0.95)),
                    Name::new(format!("inv_slot_{r}_{c}")),
                ))
                .id();

            let label = commands
                .spawn((
                    Text::new("Empty"),
                    TextFont {
                        font: font.clone(),
                        font_size: 16.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.85, 0.85, 0.85)),
                    SlotLabel { idx },
                    Name::new(format!("inv_slot_label_{idx}")),
                ))
                .id();

            commands.entity(slot).add_child(label);
            commands.entity(row).add_child(slot);

            state.spawned.push(slot);
            state.spawned.push(label);

            idx += 1;
        }
    }

    state.spawned.push(root);
    state.spawned.push(panel);
    state.spawned.push(title);
    state.spawned.push(grid);

    state.root = Some(root);
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
            Some(Some(stk)) => format!("{}", stk.id),
        };
        commands.entity(e).insert(Text::new(txt));
    }
}

fn despawn_all(commands: &mut Commands, state: &mut ResMut<InventoryUiState>) {
    for e in state.spawned.drain(..) {
        if let Ok(mut ec) = commands.get_entity(e) {
            ec.despawn();
        }
    }
    state.root = None;
    state.needs_rebuild = false;
}