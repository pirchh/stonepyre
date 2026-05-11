// crates/stonepyre_ui/src/hud.rs
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use stonepyre_engine::plugins::interaction::WorldInteractionBlocker;

use crate::character_state::CharacterUiState;
use crate::config::UiBindings;
use crate::inventory::InventoryUiState;
use crate::GameUiEnabled;

const HUD_BTN_BG: Color = Color::srgba(0.10, 0.10, 0.12, 0.92);
const HUD_BTN_BG_HOVER: Color = Color::srgba(0.14, 0.13, 0.12, 0.96);
const HUD_BTN_BG_ACTIVE: Color = Color::srgba(0.20, 0.16, 0.10, 0.98);

const HUD_ICON_RENDER_PX: f32 = 52.0;
const HUD_PAD: f32 = 6.0;
const HUD_MARGIN_R: f32 = 10.0;
const HUD_MARGIN_B: f32 = 10.0;
const HUD_COLS: usize = 8;

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

    let count = buttons.len().min(HUD_COLS);

    let bar_w = hud_bar_width(count);
    let bar_h = hud_bar_height();

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
                right: Val::Px(HUD_MARGIN_R),
                bottom: Val::Px(HUD_MARGIN_B),
                width: Val::Px(bar_w),
                height: Val::Px(bar_h),
                padding: UiRect::all(Val::Px(HUD_PAD)),
                display: Display::Flex,
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(HUD_PAD),
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.05, 0.05, 0.06, 0.82)),
            Name::new("hud_bar"),
        ))
        .id();

    commands.entity(root).add_child(bar);

    for (idx, btn) in buttons.into_iter().take(HUD_COLS).enumerate() {
        let icon_path = format!("inventory/hud/{}.png", btn.icon_id);
        let icon = asset_server.load(icon_path);

        let btn_e = commands
            .spawn((
                Node {
                    width: Val::Px(HUD_ICON_RENDER_PX),
                    height: Val::Px(HUD_ICON_RENDER_PX),
                    ..default()
                },
                BackgroundColor(HUD_BTN_BG),
                Interaction::default(),
                btn,
                Name::new(format!("hud_btn_{idx}")),
            ))
            .id();

        let icon_e = commands
            .spawn((
                ImageNode::new(icon),
                Node {
                    width: Val::Px(HUD_ICON_RENDER_PX),
                    height: Val::Px(HUD_ICON_RENDER_PX),
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

/// Keeps HUD icon backgrounds in sync with the currently open tab.
/// This also handles keyboard toggles, not just mouse clicks.
pub(crate) fn hud_active_tab_highlight_system(
    inv_state: Res<InventoryUiState>,
    char_state: Res<CharacterUiState>,
    mut q: Query<(&HudButton, &Interaction, &mut BackgroundColor)>,
) {
    for (btn, interaction, mut bg) in q.iter_mut() {
        let active = match btn.action {
            HudAction::ToggleInventory => inv_state.open,
            HudAction::ToggleCharacter => char_state.open,
            _ => false,
        };

        *bg = if active {
            BackgroundColor(HUD_BTN_BG_ACTIVE)
        } else if *interaction == Interaction::Hovered {
            BackgroundColor(HUD_BTN_BG_HOVER)
        } else {
            BackgroundColor(HUD_BTN_BG)
        };
    }
}

/// Prevent HUD clicks from leaking through into world movement/interactions.
pub(crate) fn hud_world_interaction_blocker_system(
    windows: Query<&Window, With<PrimaryWindow>>,
    mut blocker: ResMut<WorldInteractionBlocker>,
) {
    blocker.0 = blocker.0 || cursor_over_hud_bar(&windows);
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

fn cursor_over_hud_bar(windows: &Query<&Window, With<PrimaryWindow>>) -> bool {
    let Ok(window) = windows.single() else {
        return false;
    };
    let Some(cursor) = window.cursor_position() else {
        return false;
    };

    let bar_w = hud_bar_width(HUD_COLS);
    let bar_h = hud_bar_height();
    let bar_left = (window.width() - bar_w - HUD_MARGIN_R).max(0.0);
    let bar_right = bar_left + bar_w;
    let bar_top = (window.height() - bar_h - HUD_MARGIN_B).max(0.0);
    let bar_bottom = bar_top + bar_h;

    cursor.x >= bar_left
        && cursor.x <= bar_right
        && cursor.y >= bar_top
        && cursor.y <= bar_bottom
}

fn hud_bar_width(count: usize) -> f32 {
    (count as f32) * HUD_ICON_RENDER_PX + (count as f32 + 1.0) * HUD_PAD
}

fn hud_bar_height() -> f32 {
    HUD_ICON_RENDER_PX + 2.0 * HUD_PAD
}
