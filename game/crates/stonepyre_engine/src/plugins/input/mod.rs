use bevy::prelude::*;

pub mod bindings;
pub use bindings::InputBindings;

use stonepyre_world::{world3d_to_tile, TilePos};

#[derive(Message, Clone, Copy, Debug)]
pub struct ClickMsg {
    pub button: MouseButton,
    pub tile: TilePos,
    pub cursor_screen: Vec2,
    /// XZ world coordinates of the cursor's intersection with the Y=0 ground plane.
    pub cursor_world: Vec2,
}

pub fn emit_click_messages(
    mouse_btn: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window>,
    cam_q: Query<(&Camera, &GlobalTransform), With<Camera3d>>,
    mut writer: MessageWriter<ClickMsg>,
) {
    let Ok(window) = windows.single() else { return; };
    let Ok((camera, cam_xform)) = cam_q.single() else { return; };

    let Some(cursor_screen) = window.cursor_position() else { return; };

    // Cast a ray from the camera into the scene and intersect with Y=0 (ground plane).
    let Ok(ray) = camera.viewport_to_world(cam_xform, cursor_screen) else { return; };

    // Avoid divide-by-zero if ray is parallel to ground.
    if ray.direction.y.abs() < 1e-6 {
        return;
    }

    let t = -ray.origin.y / ray.direction.y;
    if t < 0.0 {
        return; // intersection behind camera
    }
    let hit = ray.origin + t * *ray.direction;
    let cursor_world = Vec2::new(hit.x, hit.z);
    let tile = world3d_to_tile(hit);

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