use bevy::prelude::*;

#[derive(Resource, Default)]
pub struct CharacterUiState {
    pub open: bool,
    pub root: Option<Entity>,
    pub spawned: Vec<Entity>,
    pub needs_rebuild: bool,
    /// Active bag-slot context menu (right-click on an equipped bag slot button).
    pub context_menu_root: Option<Entity>,
    pub context_bag_slot: Option<u8>,
}
