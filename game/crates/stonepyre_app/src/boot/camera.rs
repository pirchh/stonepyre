use bevy::prelude::*;

/// Tag for the Boot-only UI camera.
#[derive(Component)]
pub struct BootUiCamera;

/// Spawn a camera that can render UI during BootFlow.
///
/// Bevy 0.18: use `Camera2d` (Camera2dBundle no longer exists).
pub fn spawn_boot_ui_camera(mut commands: Commands, existing: Query<Entity, With<BootUiCamera>>) {
    // Only ever spawn OUR boot camera once.
    if !existing.is_empty() {
        return;
    }

    // Spawn the 2D camera (required for UI to render).
    let e = commands.spawn((BootUiCamera, Camera2d)).id();

    // Ensure this camera renders above typical world cameras while BootFlow is active.
    // Camera2d already includes a Camera component; inserting overwrites/updates it.
    commands.entity(e).insert(Camera {
        order: 100,
        ..default()
    });
}

/// Remove the Boot UI camera once we enter the actual in-world state.
pub fn despawn_boot_ui_camera(mut commands: Commands, q: Query<Entity, With<BootUiCamera>>) {
    for e in &q {
        commands.entity(e).despawn();
    }
}