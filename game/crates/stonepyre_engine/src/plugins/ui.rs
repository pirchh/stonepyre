use bevy::prelude::*;

use crate::plugins::interaction::InteractionCandidate;
use crate::plugins::input::ClickMsg;

#[derive(Message, Clone, Copy, Debug)]
pub struct MenuSelectMsg {
    pub idx: usize,
}

#[derive(Resource, Default)]
pub struct ContextMenuState {
    pub open: bool,
    pub screen_pos: Vec2,
    pub candidates: Vec<InteractionCandidate>,

    /// Parent/root menu entities that should be despawned when the menu closes/rebuilds.
    /// Child text entities are intentionally not tracked here because despawning the parent row
    /// also despawns its children. Tracking both causes double-despawn warnings in Bevy.
    pub spawned: Vec<Entity>,

    pub dirty: bool,

    /// Set when the context menu consumes a left click this frame.
    /// The world interaction planner uses this to avoid also treating the same click
    /// as a WalkHere/cell click behind the menu.
    pub consumed_left_click: bool,
}

#[derive(Component)]
pub(crate) struct ContextMenuOverlayItem {
    idx: usize,
    // world-space bounds for hit testing
    min: Vec2,
    max: Vec2,
}

const MENU_Z: f32 = 900.0;
const ITEM_H: f32 = 22.0;
const ITEM_W: f32 = 160.0;
const PAD: f32 = 6.0;

fn screen_to_world_2d(
    cam: &Camera,
    cam_xform: &GlobalTransform,
    wnd: &Window,
    screen: Vec2,
) -> Option<Vec2> {
    cam.viewport_to_world_2d(cam_xform, screen)
        .or_else(|_| {
            let logical = screen / wnd.scale_factor() as f32;
            cam.viewport_to_world_2d(cam_xform, logical)
        })
        .ok()
}

fn despawn_context_menu_entities(commands: &mut Commands, menu: &mut ContextMenuState) {
    for e in menu.spawned.drain(..) {
        if let Ok(mut ec) = commands.get_entity(e) {
            ec.despawn();
        }
    }
}

pub(crate) fn context_menu_overlay_system(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    menu: Option<ResMut<ContextMenuState>>,
    cam_q: Query<(&Camera, &GlobalTransform)>,
    wnd_q: Query<&Window>,
) {
    let Some(mut menu) = menu else { return; };

    // Close: despawn once.
    if !menu.open {
        if !menu.spawned.is_empty() {
            despawn_context_menu_entities(&mut commands, &mut menu);
        }
        menu.dirty = false;
        return;
    }

    // Open: only rebuild when dirty.
    if !menu.dirty && !menu.spawned.is_empty() {
        return;
    }

    // Clear old overlay before rebuild.
    if !menu.spawned.is_empty() {
        despawn_context_menu_entities(&mut commands, &mut menu);
    }
    menu.dirty = false;

    let Ok((cam, cam_xform)) = cam_q.single() else { return; };
    let Ok(wnd) = wnd_q.single() else { return; };

    let Some(cursor_world) = screen_to_world_2d(cam, cam_xform, wnd, menu.screen_pos) else {
        return;
    };

    let font: Handle<Font> = asset_server.load("fonts/ui.ttf");

    let candidates = menu.candidates.clone();
    let count = candidates.len().max(1) as f32;

    let panel_w = ITEM_W + PAD * 2.0;
    let panel_h = count * ITEM_H + PAD * 2.0;

    // Slightly offset from cursor.
    let cursor_offset = Vec2::new(10.0, -10.0);

    // Panel center and top-left.
    let panel_center = cursor_world + cursor_offset + Vec2::new(panel_w * 0.5, -panel_h * 0.5);
    let panel_top_left = Vec2::new(panel_center.x - panel_w * 0.5, panel_center.y + panel_h * 0.5);

    // Root panel (world).
    let panel = commands
        .spawn((
            Sprite::from_color(Color::srgba(0.08, 0.08, 0.10, 0.92), Vec2::new(panel_w, panel_h)),
            Transform::from_xyz(panel_center.x, panel_center.y, MENU_Z),
        ))
        .id();
    menu.spawned.push(panel);

    // Rows (world), with text as a child in local space.
    for (i, cand) in candidates.iter().enumerate() {
        let row_top = panel_top_left.y - PAD - (i as f32) * ITEM_H;
        let row_bottom = row_top - ITEM_H;
        let row_center_y = (row_top + row_bottom) * 0.5;

        let row_left = panel_top_left.x + PAD;
        let row_right = row_left + ITEM_W;

        let row_ent = commands
            .spawn((
                Sprite::from_color(
                    Color::srgba(0.14, 0.14, 0.18, 0.95),
                    Vec2::new(ITEM_W, ITEM_H),
                ),
                Transform::from_xyz((row_left + row_right) * 0.5, row_center_y, MENU_Z + 1.0),
                ContextMenuOverlayItem {
                    idx: i,
                    min: Vec2::new(row_left, row_bottom),
                    max: Vec2::new(row_right, row_top),
                },
            ))
            .id();
        menu.spawned.push(row_ent);

        // Text is a child of the row and is intentionally NOT pushed to menu.spawned.
        // Despawning the row will despawn this child too.
        let label = format!("{:?}", cand.verb);
        let text_ent = commands
            .spawn((
                Text2d::new(label),
                TextFont {
                    font: font.clone(),
                    font_size: 16.0,
                    ..default()
                },
                TextColor(Color::srgba(0.92, 0.92, 0.95, 1.0)),
                Transform::from_xyz(0.0, -7.0, 1.0),
            ))
            .id();

        commands.entity(row_ent).add_child(text_ent);
    }
}

pub(crate) fn handle_context_menu_overlay_clicks(
    mut click_reader: MessageReader<ClickMsg>,
    menu: Option<ResMut<ContextMenuState>>,
    cam_q: Query<(&Camera, &GlobalTransform)>,
    wnd_q: Query<&Window>,
    items_q: Query<&ContextMenuOverlayItem>,
    mut writer: MessageWriter<MenuSelectMsg>,
) {
    let Some(mut menu) = menu else {
        for _ in click_reader.read() {}
        return;
    };

    if !menu.open {
        for _ in click_reader.read() {}
        return;
    }

    let Ok((cam, cam_xform)) = cam_q.single() else { return; };
    let Ok(wnd) = wnd_q.single() else { return; };

    for ev in click_reader.read() {
        if ev.button != MouseButton::Left {
            continue;
        }

        // The context menu owns this left click while open, even if it lands outside
        // a menu row. This prevents the same click from falling through into the world.
        menu.consumed_left_click = true;

        let Some(world) = screen_to_world_2d(cam, cam_xform, wnd, ev.cursor_screen) else {
            menu.open = false;
            menu.dirty = true;
            continue;
        };

        let mut chosen: Option<usize> = None;
        for item in items_q.iter() {
            if world.x >= item.min.x
                && world.x <= item.max.x
                && world.y >= item.min.y
                && world.y <= item.max.y
            {
                chosen = Some(item.idx);
                break;
            }
        }

        if let Some(idx) = chosen {
            writer.write(MenuSelectMsg { idx });
        }

        menu.open = false;
        menu.dirty = true;
    }
}

/// Clears one-frame menu input consumption after world/input systems have had a chance
/// to observe it.
pub(crate) fn clear_context_menu_consumed_click(menu: Option<ResMut<ContextMenuState>>) {
    let Some(mut menu) = menu else { return; };
    menu.consumed_left_click = false;
}
