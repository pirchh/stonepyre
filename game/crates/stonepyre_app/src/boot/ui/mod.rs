use bevy::prelude::*;

pub mod common;
pub mod main_menu;
pub mod login;
pub mod character_select;

pub use main_menu::{main_menu_enter, main_menu_update};
pub use login::{login_enter, login_update};
pub use character_select::{character_select_enter, character_select_update};

// ✅ keep crate-only
pub(crate) use common::ScreenRoot;

/// Bevy 0.18-friendly recursive despawn (no despawn_recursive).
pub fn despawn_screen(
    mut commands: Commands,
    roots: Query<Entity, With<ScreenRoot>>,
    q_children: Query<&Children>,
) {
    for root in &roots {
        let mut stack = vec![root];
        let mut all = Vec::<Entity>::new();

        while let Some(e) = stack.pop() {
            all.push(e);
            if let Ok(children) = q_children.get(e) {
                for c in children.iter() {
                    stack.push(c);
                }
            }
        }

        for e in all.into_iter().rev() {
            commands.entity(e).despawn();
        }
    }
}