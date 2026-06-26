pub mod atlas;
pub mod defs;
pub mod files;
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

use std::sync::OnceLock;

/// Process-wide cached content DB (Rust bootstrap + asset-file overlay), built
/// once on first use. Prefer this over `default_content_db()` in hot paths —
/// the latter re-reads asset files on every call.
pub fn content_db() -> &'static ContentDb {
    static DB: OnceLock<ContentDb> = OnceLock::new();
    DB.get_or_init(default_content_db)
}

/// Item defs from the cached full content DB (bootstrap + overlay). Use this for
/// item lookups in UI/runtime code instead of `default_item_defs()`, which only
/// contains the handful of Rust-seeded items and misses everything authored in
/// JSON (most logs, all axes, …).
pub fn all_item_defs() -> &'static ItemDefs {
    &content_db().items
}
