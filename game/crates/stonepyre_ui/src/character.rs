use bevy::prelude::*;

use stonepyre_content::items::ItemId;
use stonepyre_engine::plugins::inventory::{Equipment, Toolbelt};
use stonepyre_engine::plugins::world::{Player, PlayerAppearance};

#[derive(Resource, Default)]
pub struct CharacterUiState {
    pub open: bool,
    pub root: Option<Entity>,
    pub spawned: Vec<Entity>,
    pub needs_rebuild: bool,
}

#[derive(Component)]
pub(crate) struct CharacterPanelRoot;

#[derive(Component)]
pub(crate) struct CharacterStatsText;

/// Tag placed on each gear slot value text so we can update it later.
#[derive(Component)]
pub(crate) struct EquipSlotLabel {
    pub(crate) slot_id: &'static str,
}

/// Square button marker (applies to gear + tool squares)
#[derive(Component)]
pub(crate) struct SlotSquareButton;

/// Gear slot button marker
#[derive(Component)]
pub(crate) struct EquipSlotButton;

/// Tool slot value text tag
#[derive(Component)]
pub(crate) struct ToolSlotLabel {
    pub(crate) tool_id: &'static str,
}

/// Tool slot button marker
#[derive(Component)]
pub(crate) struct ToolSlotButton;

#[derive(Component)]
pub(crate) struct PaperdollLayer {
    pub slot_key: Option<&'static str>,
}

// -------------------------
// Styling (WoW-ish)
// -------------------------

const PANEL_BG: Color = Color::srgba(0.03, 0.03, 0.035, 0.94);
const PANEL_BORDER: Color = Color::srgba(0.14, 0.11, 0.08, 0.98);

const HEADER_BG: Color = Color::srgba(0.06, 0.05, 0.04, 0.96);
const HEADER_BORDER: Color = Color::srgba(0.20, 0.16, 0.12, 0.98);

const SECTION_BG: Color = Color::srgba(0.06, 0.06, 0.075, 0.88);
const SECTION_BORDER: Color = Color::srgba(0.12, 0.10, 0.08, 0.98);

const SLOT_BG: Color = Color::srgba(0.06, 0.06, 0.07, 0.92);
const SLOT_BG_HOVER: Color = Color::srgba(0.10, 0.10, 0.12, 0.96);
const SLOT_BG_PRESSED: Color = Color::srgba(0.12, 0.12, 0.16, 0.98);

const SLOT_BORDER: Color = Color::srgba(0.22, 0.18, 0.13, 0.98);
const SLOT_BORDER_HOVER: Color = Color::srgba(0.34, 0.27, 0.18, 0.98);

const VIEWPORT_BG: Color = Color::srgba(0.03, 0.03, 0.04, 0.92);
const VIEWPORT_BORDER: Color = Color::srgba(0.26, 0.21, 0.15, 0.98);

// -------------------------
// Layout (shorter + denser)
// -------------------------

// Outer panel size (shortened)
const PANEL_W: f32 = 780.0;
const PANEL_H: f32 = 680.0;

// Baseline top padding inside the body row
const BODY_TOP_PAD: f32 = 10.0;

// Center gap between viewport and stats
const CENTER_GAP: f32 = 8.0;

// Gear slot sizing
const GEAR_SLOT_PX: f32 = 62.0;
const GEAR_BORDER_PX: f32 = 2.0;
const GEAR_ROW_GAP: f32 = 10.0;

// 6 rows per gear column
const GEAR_ROWS: f32 = 6.0;

// Rendered height of the gear slot *stack* (6 slots)
const GEAR_COL_H: f32 =
    (GEAR_ROWS * (GEAR_SLOT_PX + GEAR_BORDER_PX * 2.0)) + ((GEAR_ROWS - 1.0) * GEAR_ROW_GAP);

// Viewport sizing (slightly larger than your first, but not huge)
const VIEW_FRAME_W: f32 = 320.0;
const VIEW_FRAME_H: f32 = 392.0;
const VIEW_FRAME_PAD: f32 = 3.0;
const VIEW_INNER_W: f32 = VIEW_FRAME_W - (VIEW_FRAME_PAD * 2.0);
const VIEW_INNER_H: f32 = VIEW_FRAME_H - (VIEW_FRAME_PAD * 2.0);

// Paperdoll scale (fills viewport better)
const PAPERDOLL_SCALE: f32 = 1.08;

// Stats sizing (shorter)
const STATS_H: f32 = 170.0;

// Total body content height (this is the “green columns” height)
const CONTENT_H: f32 = VIEW_FRAME_H + CENTER_GAP + STATS_H;

// Column widths / house sizing (tightened)
const GEAR_HOUSE_W: f32 = 104.0;

const TOOL_COL_GAP: f32 = 10.0;

// Toolbelt sizing — 2 columns × 7 rows
const TOOL_COLS: usize = 2;
const TOOL_ROWS: usize = 7;

const TOOL_SLOT_PX: f32 = 52.0;
const TOOL_BORDER_PX: f32 = 2.0;

// House width for 2 tool columns (plus padding)
const TOOL_HOUSE_W: f32 = (TOOL_COLS as f32 * (TOOL_SLOT_PX + TOOL_BORDER_PX * 2.0))
    + ((TOOL_COLS as f32 - 1.0) * TOOL_COL_GAP)
    + 26.0;

// Bottom filler boxes (small, dense)
const BOTTOM_BOX_H: f32 = 92.0;

// -------------------------
// Fonts (consistent)
// -------------------------

const FONT_TITLE: f32 = 22.0;
const FONT_STATS: f32 = 15.0;
const FONT_TINY: f32 = 10.0;
const FONT_VALUE: f32 = 12.0;
const FONT_MICRO: f32 = 11.0;

// ------------------------------------------------------------
// Hover system (gear + tool squares)
// ------------------------------------------------------------

pub(crate) fn equip_slot_hover_system(
    mut q: Query<
        (&Interaction, &mut BackgroundColor, &Children),
        (Changed<Interaction>, With<SlotSquareButton>),
    >,
    mut border_q: Query<&mut BackgroundColor, Without<SlotSquareButton>>,
) {
    for (interaction, mut bg, children) in q.iter_mut() {
        // Button has border as first child
        let border_child = children.first().copied();

        match *interaction {
            Interaction::None => {
                *bg = BackgroundColor(SLOT_BG);
                if let Some(border) = border_child {
                    if let Ok(mut b) = border_q.get_mut(border) {
                        *b = BackgroundColor(SLOT_BORDER);
                    }
                }
            }
            Interaction::Hovered => {
                *bg = BackgroundColor(SLOT_BG_HOVER);
                if let Some(border) = border_child {
                    if let Ok(mut b) = border_q.get_mut(border) {
                        *b = BackgroundColor(SLOT_BORDER_HOVER);
                    }
                }
            }
            Interaction::Pressed => {
                *bg = BackgroundColor(SLOT_BG_PRESSED);
                if let Some(border) = border_child {
                    if let Ok(mut b) = border_q.get_mut(border) {
                        *b = BackgroundColor(SLOT_BORDER_HOVER);
                    }
                }
            }
        }
    }
}

// ------------------------------------------------------------
// Sync System
// ------------------------------------------------------------

pub(crate) fn character_panel_sync_system(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut state: ResMut<CharacterUiState>,

    player_q: Query<(&Equipment, Option<&Toolbelt>, &PlayerAppearance), With<Player>>,

    // ✅ disjoint mutable Text queries via Without<>
    mut equip_label_q: Query<
        (&EquipSlotLabel, &mut Text),
        (With<EquipSlotLabel>, Without<ToolSlotLabel>, Without<CharacterStatsText>),
    >,
    mut tool_label_q: Query<
        (&ToolSlotLabel, &mut Text),
        (With<ToolSlotLabel>, Without<EquipSlotLabel>, Without<CharacterStatsText>),
    >,
    mut stats_text_q: Query<
        &mut Text,
        (With<CharacterStatsText>, Without<EquipSlotLabel>, Without<ToolSlotLabel>),
    >,

    mut layer_q: Query<(Entity, &PaperdollLayer, Option<Mut<Visibility>>)>,
    children_q: Query<&Children>,
) {
    if !state.open {
        despawn_all(&mut commands, &mut state, &children_q);
        return;
    }

    let Ok((equip, toolbelt_opt, appearance)) = player_q.single() else {
        return;
    };

    if state.root.is_none() || state.needs_rebuild {
        despawn_all(&mut commands, &mut state, &children_q);
        spawn_character_panel(&mut commands, &asset_server, &mut state);
        state.needs_rebuild = false;
    }

    // Gear slot labels
    for (lab, mut text) in equip_label_q.iter_mut() {
        let v = match lab.slot_id {
            "Helm" => opt_id(&equip.helm),
            "Neck" => opt_id(&equip.neck),
            "Shoulders" => opt_id(&equip.shoulders),
            "Chest" => opt_id(&equip.chest),
            "Wrist" => opt_id(&equip.wrist),
            "Gloves" => opt_id(&equip.gloves),
            "Waist" => opt_id(&equip.waist),
            "Pants" => opt_id(&equip.pants),
            "Boots" => opt_id(&equip.boots),
            "Ring1" => opt_id(&equip.ring1),
            "Ring2" => opt_id(&equip.ring2),
            "Back" => opt_id(&equip.back),
            _ => String::new(),
        };
        text.0 = v;
    }

    // Toolbelt labels
    for (lab, mut text) in tool_label_q.iter_mut() {
        let v = toolbelt_opt
            .and_then(|tb| tb.get_by_id(lab.tool_id))
            .map(|id| id.to_string())
            .unwrap_or_default();
        text.0 = v;
    }

    // Paperdoll
    for (e, layer, vis_opt) in layer_q.iter_mut() {
        match layer.slot_key {
            None => {
                let base_path = paperdoll_base_path(&appearance.base_sprite_root);
                let handle: Handle<Image> = asset_server.load(base_path);
                commands.entity(e).insert(ImageNode::new(handle));
                set_visible(&mut commands, e, vis_opt);
            }
            Some(slot_key) => {
                let item_opt: Option<&ItemId> = match slot_key {
                    "Helm" => equip.helm.as_ref(),
                    "Neck" => equip.neck.as_ref(),
                    "Shoulders" => equip.shoulders.as_ref(),
                    "Chest" => equip.chest.as_ref(),
                    "Wrist" => equip.wrist.as_ref(),
                    "Gloves" => equip.gloves.as_ref(),
                    "Waist" => equip.waist.as_ref(),
                    "Pants" => equip.pants.as_ref(),
                    "Boots" => equip.boots.as_ref(),
                    "Ring1" => equip.ring1.as_ref(),
                    "Ring2" => equip.ring2.as_ref(),
                    "Back" => equip.back.as_ref(),
                    _ => None,
                };

                if let Some(item_id) = item_opt {
                    let overlay_path = wearable_idle_south_path(item_id);
                    let handle: Handle<Image> = asset_server.load(overlay_path);
                    commands.entity(e).insert(ImageNode::new(handle));
                    set_visible(&mut commands, e, vis_opt);
                } else {
                    commands.entity(e).insert(Visibility::Hidden);
                }
            }
        }
    }

    // Stats text (denser formatting so the box feels filled/intentional)
    if let Ok(mut t) = stats_text_q.single_mut() {
        t.0 = [
            "Stats (from gear)",
            "",
            "Primary",
            "  Strength:   —",
            "  Intellect:  —",
            "  Stamina:    —",
            "",
            "Secondary",
            "  Armor:      —",
            "  Damage:     —",
            "  Crit:       —",
            "  Move Speed: —",
            "  Carry:      — / —",
        ]
        .join("\n");
    }
}

// ------------------------------------------------------------
// Spawn Panel
// ------------------------------------------------------------

#[derive(Clone, Copy)]
struct RegionBox {
    outer: Entity,
    inner: Entity,
}

fn spawn_character_panel(
    commands: &mut Commands,
    asset_server: &Res<AssetServer>,
    state: &mut ResMut<CharacterUiState>,
) {
    let font = asset_server.load("fonts/ui.ttf");

    // Root overlay centered
    let root = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                display: Display::Flex,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            CharacterPanelRoot,
        ))
        .id();

    // Outer framed panel
    let panel_outer = commands
        .spawn((
            Node {
                width: Val::Px(PANEL_W),
                height: Val::Px(PANEL_H),
                padding: UiRect::all(Val::Px(12.0)),
                display: Display::Flex,
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(10.0),
                ..default()
            },
            BackgroundColor(PANEL_BORDER),
        ))
        .id();
    commands.entity(root).add_child(panel_outer);

    // Inner fill
    let panel = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                padding: UiRect::all(Val::Px(10.0)),
                display: Display::Flex,
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(10.0),
                ..default()
            },
            BackgroundColor(PANEL_BG),
        ))
        .id();
    commands.entity(panel_outer).add_child(panel);

    // Header
    let header_outer = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(46.0),
                padding: UiRect::all(Val::Px(2.0)),
                ..default()
            },
            BackgroundColor(HEADER_BORDER),
        ))
        .id();
    commands.entity(panel).add_child(header_outer);

    let header = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                padding: UiRect::horizontal(Val::Px(12.0)),
                display: Display::Flex,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(HEADER_BG),
        ))
        .id();
    commands.entity(header_outer).add_child(header);

    let title = commands
        .spawn((
            Text::new("Character"),
            TextFont {
                font: font.clone(),
                font_size: FONT_TITLE,
                ..default()
            },
            TextColor(Color::srgb(0.95, 0.90, 0.82)),
        ))
        .id();
    commands.entity(header).add_child(title);

    // Body row
    let body = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            display: Display::Flex,
            flex_direction: FlexDirection::Row,
            column_gap: Val::Px(14.0),
            padding: UiRect::top(Val::Px(BODY_TOP_PAD)),
            align_items: AlignItems::FlexStart,
            justify_content: JustifyContent::Center,
            ..default()
        })
        .id();
    commands.entity(panel).add_child(body);

    // -------------------------
    // LEFT — gear region (slots + filler)
    // -------------------------
    let left = spawn_region_box(commands, GEAR_HOUSE_W, CONTENT_H);
    commands.entity(body).add_child(left.outer);

    let left_slots = ["Helm", "Neck", "Shoulders", "Chest", "Wrist", "Gloves"];
    let left_gear_col = spawn_gear_slot_column(commands, &font, &left_slots);
    commands.entity(left.inner).add_child(left_gear_col);

    // Spacer pushes filler to bottom (E0499-safe)
    let left_spacer = spawn_flex_spacer(commands);
    commands.entity(left.inner).add_child(left_spacer);

    // Bottom filler
    let left_fill = spawn_bottom_info_box(
        commands,
        &font,
        "Summary",
        &["Armor: —", "Power: —", "Carry: — / —"],
    );
    commands.entity(left.inner).add_child(left_fill);

    // -------------------------
    // CENTER — viewport + stats
    // -------------------------
    let center_col = commands
        .spawn(Node {
            width: Val::Px(VIEW_FRAME_W),
            height: Val::Px(CONTENT_H),
            display: Display::Flex,
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(CENTER_GAP),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::FlexStart,
            ..default()
        })
        .id();

    // Viewport frame
    let viewport_frame = commands
        .spawn((
            Node {
                width: Val::Px(VIEW_FRAME_W),
                height: Val::Px(VIEW_FRAME_H),
                padding: UiRect::all(Val::Px(VIEW_FRAME_PAD)),
                ..default()
            },
            BackgroundColor(VIEWPORT_BORDER),
        ))
        .id();

    let viewport = commands
        .spawn((
            Node {
                width: Val::Px(VIEW_INNER_W),
                height: Val::Px(VIEW_INNER_H),
                padding: UiRect::all(Val::Px(4.0)),
                position_type: PositionType::Relative,
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(VIEWPORT_BG),
        ))
        .id();

    // paperdoll stack (scaled slightly)
    let stack = spawn_paperdoll_stack(commands);
    commands
        .entity(stack)
        .insert(Transform::from_scale(Vec3::splat(PAPERDOLL_SCALE)));

    commands.entity(viewport).add_child(stack);
    commands.entity(viewport_frame).add_child(viewport);
    commands.entity(center_col).add_child(viewport_frame);

    // Stats (shorter but filled)
    let stats_outer = commands
        .spawn((
            Node {
                width: Val::Px(VIEW_FRAME_W),
                height: Val::Px(STATS_H),
                padding: UiRect::all(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(SECTION_BORDER),
        ))
        .id();

    let stats_box = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                padding: UiRect::all(Val::Px(12.0)),
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(SECTION_BG),
        ))
        .id();
    commands.entity(stats_outer).add_child(stats_box);

    let stats_text = commands
        .spawn((
            Text::new("Loading..."),
            TextFont {
                font: font.clone(),
                font_size: FONT_STATS,
                ..default()
            },
            TextColor(Color::srgb(0.92, 0.92, 0.92)),
            CharacterStatsText,
        ))
        .id();
    commands.entity(stats_box).add_child(stats_text);

    commands.entity(center_col).add_child(stats_outer);
    commands.entity(body).add_child(center_col);

    // -------------------------
    // RIGHT — gear region (slots + filler)
    // -------------------------
    let right = spawn_region_box(commands, GEAR_HOUSE_W, CONTENT_H);
    commands.entity(body).add_child(right.outer);

    let right_slots = ["Back", "Waist", "Pants", "Boots", "Ring1", "Ring2"];
    let right_gear_col = spawn_gear_slot_column(commands, &font, &right_slots);
    commands.entity(right.inner).add_child(right_gear_col);

    // Spacer pushes filler to bottom (E0499-safe)
    let right_spacer = spawn_flex_spacer(commands);
    commands.entity(right.inner).add_child(right_spacer);

    let right_fill = spawn_bottom_info_box(
        commands,
        &font,
        "Bonuses",
        &["Resist: —", "Speed: —", "Luck: —"],
    );
    commands.entity(right.inner).add_child(right_fill);

    // -------------------------
    // TOOLS — tool region (grid + filler)
    // -------------------------
    let tool_region = spawn_toolbelt_house(commands, &font);
    commands.entity(body).add_child(tool_region);

    state.root = Some(root);
}

/// ✅ FIXED: returns BOTH outer+inner so we always parent outer into the tree.
/// Prevents “orphan outer” nodes rendering at top-left.
fn spawn_region_box(commands: &mut Commands, w: f32, h: f32) -> RegionBox {
    let outer = commands
        .spawn((
            Node {
                width: Val::Px(w),
                height: Val::Px(h),
                padding: UiRect::all(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(SECTION_BORDER),
        ))
        .id();

    let inner = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                padding: UiRect::all(Val::Px(10.0)),
                display: Display::Flex,
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::FlexStart,
                ..default()
            },
            BackgroundColor(SECTION_BG),
        ))
        .id();

    commands.entity(outer).add_child(inner);

    RegionBox { outer, inner }
}

fn spawn_flex_spacer(commands: &mut Commands) -> Entity {
    commands
        .spawn(Node {
            width: Val::Percent(100.0),
            height: Val::Auto,
            flex_grow: 1.0,
            ..default()
        })
        .id()
}

/// Small “dense” filler box for the bottom of columns so the space feels intentional.
fn spawn_bottom_info_box(
    commands: &mut Commands,
    font: &Handle<Font>,
    title: &str,
    lines: &[&str],
) -> Entity {
    let outer = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(BOTTOM_BOX_H),
                padding: UiRect::all(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(SECTION_BORDER),
        ))
        .id();

    let inner = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                padding: UiRect::all(Val::Px(10.0)),
                ..default()
            },
            BackgroundColor(VIEWPORT_BG),
        ))
        .id();

    let mut text = String::new();
    text.push_str(title);
    text.push('\n');
    text.push('\n');
    for (i, l) in lines.iter().enumerate() {
        text.push_str(l);
        if i + 1 != lines.len() {
            text.push('\n');
        }
    }

    let t = commands
        .spawn((
            Text::new(text),
            TextFont {
                font: font.clone(),
                font_size: FONT_MICRO,
                ..default()
            },
            TextColor(Color::srgb(0.88, 0.88, 0.88)),
        ))
        .id();

    commands.entity(inner).add_child(t);
    commands.entity(outer).add_child(inner);
    outer
}

// ------------------------------------------------------------
// Gear slots (6 rows)
// ------------------------------------------------------------

fn spawn_gear_slot_column(
    commands: &mut Commands,
    font: &Handle<Font>,
    slots: &[&'static str],
) -> Entity {
    let col = commands
        .spawn(Node {
            width: Val::Auto,
            height: Val::Px(GEAR_COL_H),
            display: Display::Flex,
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(GEAR_ROW_GAP),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::FlexStart,
            ..default()
        })
        .id();

    for slot_name in slots.iter().copied() {
        let btn = commands
            .spawn((
                Node {
                    width: Val::Px(GEAR_SLOT_PX + GEAR_BORDER_PX * 2.0),
                    height: Val::Px(GEAR_SLOT_PX + GEAR_BORDER_PX * 2.0),
                    position_type: PositionType::Relative,
                    ..default()
                },
                BackgroundColor(SLOT_BG),
                Interaction::default(),
                SlotSquareButton,
                EquipSlotButton,
            ))
            .id();

        let border = commands
            .spawn((
                Node {
                    width: Val::Px(GEAR_SLOT_PX + GEAR_BORDER_PX * 2.0),
                    height: Val::Px(GEAR_SLOT_PX + GEAR_BORDER_PX * 2.0),
                    padding: UiRect::all(Val::Px(GEAR_BORDER_PX)),
                    ..default()
                },
                BackgroundColor(SLOT_BORDER),
            ))
            .id();

        let face = commands
            .spawn(Node {
                width: Val::Px(GEAR_SLOT_PX),
                height: Val::Px(GEAR_SLOT_PX),
                position_type: PositionType::Relative,
                ..default()
            })
            .id();

        let tiny = commands
            .spawn((
                Text::new(slot_name),
                TextFont {
                    font: font.clone(),
                    font_size: FONT_TINY,
                    ..default()
                },
                TextColor(Color::srgb(0.78, 0.72, 0.62)),
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(6.0),
                    top: Val::Px(4.0),
                    ..default()
                },
            ))
            .id();

        let value = commands
            .spawn((
                Text::new(""),
                TextFont {
                    font: font.clone(),
                    font_size: FONT_VALUE,
                    ..default()
                },
                TextColor(Color::srgb(0.92, 0.92, 0.92)),
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(6.0),
                    right: Val::Px(6.0),
                    top: Val::Px(30.0),
                    ..default()
                },
                EquipSlotLabel { slot_id: slot_name },
            ))
            .id();

        commands.entity(face).add_child(tiny);
        commands.entity(face).add_child(value);
        commands.entity(border).add_child(face);
        commands.entity(btn).add_child(border);
        commands.entity(col).add_child(btn);
    }

    col
}

// ------------------------------------------------------------
// Toolbelt house — grid + bottom filler
// ------------------------------------------------------------

fn spawn_toolbelt_house(commands: &mut Commands, font: &Handle<Font>) -> Entity {
    let outer_radius = BorderRadius {
        top_left: Val::Px(10.0),
        top_right: Val::Px(10.0),
        bottom_left: Val::Px(10.0),
        bottom_right: Val::Px(10.0),
    };

    let inner_radius = BorderRadius {
        top_left: Val::Px(8.0),
        top_right: Val::Px(8.0),
        bottom_left: Val::Px(8.0),
        bottom_right: Val::Px(8.0),
    };

    let outer = commands
        .spawn((
            Node {
                width: Val::Px(TOOL_HOUSE_W),
                height: Val::Px(CONTENT_H),
                padding: UiRect::all(Val::Px(3.0)),
                border_radius: outer_radius,
                ..default()
            },
            BackgroundColor(SECTION_BORDER),
        ))
        .id();

    let inner = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                padding: UiRect::all(Val::Px(10.0)),
                border_radius: inner_radius,
                display: Display::Flex,
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::FlexStart,
                ..default()
            },
            BackgroundColor(SECTION_BG),
        ))
        .id();
    commands.entity(outer).add_child(inner);

    // Tool grid container: lock its height to the gear pixel stack
    let grid = commands
        .spawn(Node {
            width: Val::Auto,
            height: Val::Px(GEAR_COL_H),
            display: Display::Flex,
            flex_direction: FlexDirection::Row,
            column_gap: Val::Px(TOOL_COL_GAP),
            align_items: AlignItems::Stretch,
            justify_content: JustifyContent::Center,
            ..default()
        })
        .id();
    commands.entity(inner).add_child(grid);

    let tools: [&'static str; 14] = [
        "Axe", "Pickaxe", "Rod", "Knife", "Hammer", "Chisel", "Needle", "Saw", "Tongs", "Mortar",
        "Brush", "Sickle", "Pan", "Lantern",
    ];

    // Compute stable row gap so the whole column equals GEAR_COL_H
    let slot_outer_h = TOOL_SLOT_PX + TOOL_BORDER_PX * 2.0;
    let total_slots_h = TOOL_ROWS as f32 * slot_outer_h;
    let remaining = (GEAR_COL_H - total_slots_h).max(0.0);
    let computed_gap = if TOOL_ROWS > 1 {
        remaining / (TOOL_ROWS as f32 - 1.0)
    } else {
        0.0
    };

    let left_col = spawn_tool_slot_column(commands, font, &tools[0..7], computed_gap);
    let right_col = spawn_tool_slot_column(commands, font, &tools[7..14], computed_gap);

    commands.entity(grid).add_child(left_col);
    commands.entity(grid).add_child(right_col);

    // Spacer pushes bottom filler to bottom (E0499-safe)
    let tool_spacer = spawn_flex_spacer(commands);
    commands.entity(inner).add_child(tool_spacer);

    let bottom = spawn_bottom_info_box(
        commands,
        font,
        "Toolbelt",
        &["Active: —", "Proficiency: —", "Hotkeys: 1-7 / Shift+1-7"],
    );
    commands.entity(inner).add_child(bottom);

    outer
}

fn spawn_tool_slot_column(
    commands: &mut Commands,
    font: &Handle<Font>,
    tools: &[&'static str],
    row_gap_px: f32,
) -> Entity {
    let col = commands
        .spawn(Node {
            width: Val::Auto,
            height: Val::Px(GEAR_COL_H),
            display: Display::Flex,
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(row_gap_px),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::FlexStart,
            ..default()
        })
        .id();

    for tool_id in tools.iter().copied() {
        let btn = commands
            .spawn((
                Node {
                    width: Val::Px(TOOL_SLOT_PX + TOOL_BORDER_PX * 2.0),
                    height: Val::Px(TOOL_SLOT_PX + TOOL_BORDER_PX * 2.0),
                    position_type: PositionType::Relative,
                    ..default()
                },
                BackgroundColor(SLOT_BG),
                Interaction::default(),
                SlotSquareButton,
                ToolSlotButton,
            ))
            .id();

        let border = commands
            .spawn((
                Node {
                    width: Val::Px(TOOL_SLOT_PX + TOOL_BORDER_PX * 2.0),
                    height: Val::Px(TOOL_SLOT_PX + TOOL_BORDER_PX * 2.0),
                    padding: UiRect::all(Val::Px(TOOL_BORDER_PX)),
                    ..default()
                },
                BackgroundColor(SLOT_BORDER),
            ))
            .id();

        let face = commands
            .spawn(Node {
                width: Val::Px(TOOL_SLOT_PX),
                height: Val::Px(TOOL_SLOT_PX),
                position_type: PositionType::Relative,
                ..default()
            })
            .id();

        let tiny = commands
            .spawn((
                Text::new(tool_id),
                TextFont {
                    font: font.clone(),
                    font_size: FONT_TINY,
                    ..default()
                },
                TextColor(Color::srgb(0.78, 0.72, 0.62)),
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(5.0),
                    top: Val::Px(4.0),
                    ..default()
                },
            ))
            .id();

        let value = commands
            .spawn((
                Text::new(""),
                TextFont {
                    font: font.clone(),
                    font_size: FONT_VALUE,
                    ..default()
                },
                TextColor(Color::srgb(0.92, 0.92, 0.92)),
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(5.0),
                    right: Val::Px(5.0),
                    top: Val::Px(26.0),
                    ..default()
                },
                ToolSlotLabel { tool_id },
            ))
            .id();

        commands.entity(face).add_child(tiny);
        commands.entity(face).add_child(value);
        commands.entity(border).add_child(face);
        commands.entity(btn).add_child(border);
        commands.entity(col).add_child(btn);
    }

    col
}

// ------------------------------------------------------------
// Paperdoll stack (base + overlays)
// ------------------------------------------------------------

fn spawn_paperdoll_stack(commands: &mut Commands) -> Entity {
    let stack = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            position_type: PositionType::Relative,
            ..default()
        })
        .id();

    let base = commands
        .spawn((
            PaperdollLayer { slot_key: None },
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                ..default()
            },
        ))
        .id();
    commands.entity(stack).add_child(base);

    let overlay_order: [&'static str; 12] = [
        "Back", "Chest", "Shoulders", "Helm", "Neck", "Gloves", "Wrist", "Waist", "Pants", "Boots",
        "Ring1", "Ring2",
    ];

    for slot_key in overlay_order {
        let layer = commands
            .spawn((
                PaperdollLayer {
                    slot_key: Some(slot_key),
                },
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    position_type: PositionType::Absolute,
                    left: Val::Px(0.0),
                    top: Val::Px(0.0),
                    ..default()
                },
                Visibility::Hidden,
            ))
            .id();

        commands.entity(stack).add_child(layer);
    }

    stack
}

// ------------------------------------------------------------
// Helpers
// ------------------------------------------------------------

fn paperdoll_base_path(base_root: &str) -> String {
    format!("{}/idle/south/south_idle.png", base_root)
}

fn wearable_idle_south_path(item_id: &ItemId) -> String {
    format!("items/wearables/{}/idle/south/south_idle.png", item_id)
}

fn opt_id(v: &Option<ItemId>) -> String {
    v.as_deref().unwrap_or("").to_string()
}

fn set_visible(commands: &mut Commands, e: Entity, vis_opt: Option<Mut<Visibility>>) {
    if let Some(mut vis) = vis_opt {
        *vis = Visibility::Visible;
    } else {
        commands.entity(e).insert(Visibility::Visible);
    }
}

// Manual recursive despawn
fn despawn_all(
    commands: &mut Commands,
    state: &mut ResMut<CharacterUiState>,
    children_q: &Query<&Children>,
) {
    fn despawn_tree(commands: &mut Commands, children_q: &Query<&Children>, e: Entity) {
        if let Ok(children) = children_q.get(e) {
            let kids: Vec<Entity> = children.to_vec();
            for c in kids {
                despawn_tree(commands, children_q, c);
            }
        }
        commands.entity(e).despawn();
    }

    if let Some(root) = state.root.take() {
        despawn_tree(commands, children_q, root);
    }

    state.spawned.clear();
    state.needs_rebuild = false;
}
