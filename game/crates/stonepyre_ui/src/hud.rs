// crates/stonepyre_ui/src/hud.rs
use bevy::prelude::*;

use crate::character::CharacterUiState;
use crate::config::UiBindings;
use crate::inventory::InventoryUiState;
use crate::GameUiEnabled;

/// What a HUD button does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HudAction {
    ToggleInventory,
    ToggleCharacter,

    // Coming soon:
    ToggleSkills,
    ToggleTalents,
    ToggleQuests,
    ToggleMap,
    ToggleJournal,
    ToggleSettings,
}

/// Metadata for hover tooltip + icon selection.
#[derive(Component, Clone, Debug)]
pub struct HudButton {
    /// Used to load icon: inventory/hud/{icon_id}.png
    pub icon_id: String,
    pub name: String,
    pub description: String,
    pub action: HudAction,
}

#[derive(Component)]
struct HudRoot;

#[derive(Component)]
pub(crate) struct HudTooltipRoot;

#[derive(Component)]
pub(crate) struct HudTooltipText;

/// Keeps track of HUD entities.
#[derive(Resource, Default)]
pub struct HudState {
    pub root: Option<Entity>,
    pub tooltip_root: Option<Entity>,
    pub tooltip_text: Option<Entity>,
}

/// Bevy-version-proof UI tree despawn without extension traits.
/// Collects all descendants through `Children`, then despawns bottom-up.
fn despawn_ui_tree(root: Entity, children_q: &Query<&Children>, commands: &mut Commands) {
    let mut stack = vec![root];
    let mut all: Vec<Entity> = Vec::new();

    while let Some(e) = stack.pop() {
        all.push(e);

        if let Ok(children) = children_q.get(e) {
            for c in children.iter() {
                stack.push(c);
            }
        }
    }

    for e in all.into_iter().rev() {
        commands.entity(e).despawn();
    }
}

/// Ensures the HUD exists only when `GameUiEnabled(true)`.
pub(crate) fn ensure_hud_bar_system(
    mut commands: Commands,
    enabled: Res<GameUiEnabled>,
    asset_server: Res<AssetServer>,
    children_q: Query<&Children>,
    mut state: ResMut<HudState>,
) {
    if enabled.0 {
        if state.root.is_none() {
            spawn_hud_bar(&mut commands, &asset_server, &mut state);
        }
    } else {
        if let Some(root) = state.root.take() {
            despawn_ui_tree(root, &children_q, &mut commands);
        }
        state.tooltip_root = None;
        state.tooltip_text = None;
    }
}

/// Build the bottom-right HUD button row.
fn spawn_hud_bar(commands: &mut Commands, asset_server: &AssetServer, state: &mut HudState) {
    let buttons: Vec<HudButton> = vec![
        HudButton {
            icon_id: "inventory".to_string(),
            name: "Inventory".to_string(),
            description: "Items you are carrying. Move items, manage backpack.".to_string(),
            action: HudAction::ToggleInventory,
        },
        HudButton {
            icon_id: "character".to_string(),
            name: "Character".to_string(),
            description: "Equipment + stats. Gear modifies attributes + resource pools.".to_string(),
            action: HudAction::ToggleCharacter,
        },
        HudButton {
            icon_id: "skills".to_string(),
            name: "Skills".to_string(),
            description: "Skill levels and unlocks. (Coming soon)".to_string(),
            action: HudAction::ToggleSkills,
        },
        HudButton {
            icon_id: "talents".to_string(),
            name: "Talents".to_string(),
            description: "Abilities/spells. Weapon-driven. (Coming soon)".to_string(),
            action: HudAction::ToggleTalents,
        },
        HudButton {
            icon_id: "quests".to_string(),
            name: "Quests".to_string(),
            description: "Quest log and objectives. (Coming soon)".to_string(),
            action: HudAction::ToggleQuests,
        },
        HudButton {
            icon_id: "map".to_string(),
            name: "Map".to_string(),
            description: "World map. (Coming soon)".to_string(),
            action: HudAction::ToggleMap,
        },
        HudButton {
            icon_id: "journal".to_string(),
            name: "Journal".to_string(),
            description: "Codex/wiki unlocks. (Coming soon)".to_string(),
            action: HudAction::ToggleJournal,
        },
        HudButton {
            icon_id: "settings".to_string(),
            name: "Settings".to_string(),
            description: "Keybinds + UI options. (Coming soon)".to_string(),
            action: HudAction::ToggleSettings,
        },
    ];

    let icon_render_px: f32 = 52.0;
    let pad: f32 = 6.0;
    let margin_r: f32 = 10.0;
    let margin_b: f32 = 10.0;

    let cols: usize = 8;
    let count = buttons.len().min(cols);

    let bar_w = (count as f32) * icon_render_px + (count as f32 + 1.0) * pad;
    let bar_h = icon_render_px + 2.0 * pad;

    let root = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..default()
            },
            HudRoot,
            Name::new("hud_ui_root"),
        ))
        .id();

    let bar = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(margin_r),
                bottom: Val::Px(margin_b),
                width: Val::Px(bar_w),
                height: Val::Px(bar_h),
                padding: UiRect::all(Val::Px(pad)),
                display: Display::Flex,
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(pad),
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.05, 0.05, 0.06, 0.82)),
            Name::new("hud_bar"),
        ))
        .id();

    commands.entity(root).add_child(bar);

    for (idx, btn) in buttons.into_iter().take(cols).enumerate() {
        let icon_path = format!("inventory/hud/{}.png", btn.icon_id);
        let icon = asset_server.load(icon_path);

        let btn_e = commands
            .spawn((
                Node {
                    width: Val::Px(icon_render_px),
                    height: Val::Px(icon_render_px),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.10, 0.10, 0.12, 0.92)),
                Interaction::default(),
                btn,
                Name::new(format!("hud_btn_{idx}")),
            ))
            .id();

        let icon_e = commands
            .spawn((
                ImageNode::new(icon),
                Node {
                    width: Val::Px(icon_render_px),
                    height: Val::Px(icon_render_px),
                    ..default()
                },
                Name::new("hud_icon"),
            ))
            .id();

        commands.entity(btn_e).add_child(icon_e);
        commands.entity(bar).add_child(btn_e);
    }

    let tooltip_root = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                width: Val::Px(360.0),
                height: Val::Auto,
                padding: UiRect::all(Val::Px(10.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.02, 0.02, 0.03, 0.95)),
            Visibility::Hidden,
            HudTooltipRoot,
            Name::new("hud_tooltip_root"),
        ))
        .id();

    let font = asset_server.load("fonts/ui.ttf");

    let tooltip_text = commands
        .spawn((
            Text::new(""),
            TextFont {
                font,
                font_size: 18.0,
                ..default()
            },
            TextColor(Color::srgb(0.92, 0.92, 0.92)),
            HudTooltipText,
            Name::new("hud_tooltip_text"),
        ))
        .id();

    commands.entity(tooltip_root).add_child(tooltip_text);
    commands.entity(root).add_child(tooltip_root);

    state.root = Some(root);
    state.tooltip_root = Some(tooltip_root);
    state.tooltip_text = Some(tooltip_text);
}

/// Handle clicking HUD buttons.
/// Inventory and Character are mutually exclusive.
pub(crate) fn hud_interactions_system(
    mut q: Query<(&Interaction, &HudButton), Changed<Interaction>>,
    mut inv_state: ResMut<InventoryUiState>,
    mut char_state: ResMut<CharacterUiState>,
) {
    for (interaction, btn) in q.iter_mut() {
        if *interaction != Interaction::Pressed {
            continue;
        }

        match btn.action {
            HudAction::ToggleInventory => open_inventory(&mut inv_state, &mut char_state),
            HudAction::ToggleCharacter => open_character(&mut inv_state, &mut char_state),
            _ => {}
        }
    }
}

/// Tooltip on hover: shows name + description and follows cursor.
pub(crate) fn hud_tooltip_system(
    windows: Query<&Window>,
    hovered: Query<(&Interaction, &HudButton)>,
    state: Res<HudState>,
    mut tooltip_root_q: Query<(&mut Node, &mut Visibility), With<HudTooltipRoot>>,
    mut tooltip_text_q: Query<&mut Text, With<HudTooltipText>>,
) {
    let (Some(root_e), Some(text_e)) = (state.tooltip_root, state.tooltip_text) else {
        return;
    };

    let Ok(win) = windows.single() else { return; };
    let Some(cursor) = win.cursor_position() else {
        if let Ok((_node, mut vis)) = tooltip_root_q.get_mut(root_e) {
            *vis = Visibility::Hidden;
        }
        return;
    };

    let mut hovered_btn: Option<&HudButton> = None;
    for (interaction, btn) in hovered.iter() {
        if *interaction == Interaction::Hovered {
            hovered_btn = Some(btn);
            break;
        }
    }

    let Ok((mut node, mut vis)) = tooltip_root_q.get_mut(root_e) else { return; };
    let Ok(mut text) = tooltip_text_q.get_mut(text_e) else { return; };

    let Some(btn) = hovered_btn else {
        *vis = Visibility::Hidden;
        return;
    };

    node.left = Val::Px(cursor.x + 16.0);
    node.top = Val::Px(cursor.y + 16.0);

    *text = Text::new(format!("{}\n\n{}", btn.name, btn.description));
    *vis = Visibility::Visible;
}

/// Keyboard toggles mirror HUD clicks.
pub(crate) fn hud_keyboard_toggles(
    keys: Res<ButtonInput<KeyCode>>,
    binds: Res<UiBindings>,
    mut inv_state: ResMut<InventoryUiState>,
    mut char_state: ResMut<CharacterUiState>,
) {
    if keys.just_pressed(binds.toggle_inventory) {
        open_inventory(&mut inv_state, &mut char_state);
    }
    if keys.just_pressed(binds.toggle_character) {
        open_character(&mut inv_state, &mut char_state);
    }
}

fn open_inventory(inv_state: &mut InventoryUiState, char_state: &mut CharacterUiState) {
    inv_state.open = !inv_state.open;
    if inv_state.open {
        char_state.open = false;
    }
    inv_state.needs_rebuild = true;
    char_state.needs_rebuild = true;
}

fn open_character(inv_state: &mut InventoryUiState, char_state: &mut CharacterUiState) {
    char_state.open = !char_state.open;
    if char_state.open {
        inv_state.open = false;
    }
    inv_state.needs_rebuild = true;
    char_state.needs_rebuild = true;
}
