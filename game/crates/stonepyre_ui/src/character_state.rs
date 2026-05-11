use bevy::prelude::*;

#[derive(Resource, Default)]
pub struct CharacterUiState {
    pub open: bool,
    pub root: Option<Entity>,
    pub spawned: Vec<Entity>,
    pub needs_rebuild: bool,
}
