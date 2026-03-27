use bevy::prelude::*;

pub mod bindings;
pub use bindings::InputBindings;

use stonepyre_world::{world_to_tile, TilePos};

#[derive(Message, Clone, Copy, Debug)]
pub struct ClickMsg {
    pub button: MouseButton,
    pub tile: TilePos,
    pub cursor_screen: Vec2,
    pub cursor_world: Vec2,
}

pub fn emit_click_messages(
    mouse_btn: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window>,
    cam_q: Query<(&Camera, &GlobalTransform), With<Camera2d>>,
    mut writer: MessageWriter<ClickMsg>,
) {
    let Ok(window) = windows.single() else { return; };
    let Ok((camera, cam_xform)) = cam_q.single() else { return; };

    let Some(cursor_screen) = window.cursor_position() else { return; };

    // Bevy 0.18: viewport_to_world_2d returns Result<Vec2, _>
    let Ok(cursor_world) = camera.viewport_to_world_2d(cam_xform, cursor_screen) else { return; };

    let tile = world_to_tile(cursor_world);

    for &btn in [MouseButton::Left, MouseButton::Right].iter() {
        if mouse_btn.just_pressed(btn) {
            writer.write(ClickMsg {
                button: btn,
                tile,
                cursor_screen,
                cursor_world,
            });
        }
    }
}