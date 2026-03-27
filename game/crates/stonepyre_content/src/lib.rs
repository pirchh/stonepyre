pub mod atlas;
pub mod defs;
pub mod items;
pub mod manifest;
pub mod objects;
pub mod recipes;
pub mod skills;

pub use manifest::{
    default_content_db,
    default_container_defs,
    default_item_defs,
    ContentDb,
};

// ✅ Re-export these directly from items (they live there)
pub use items::{ContainerDefs, ItemDefs};

// Convenience: harvest defaults (already used)
pub use manifest::default_harvest_defs;