use serde::de::DeserializeOwned;
use std::fs;
use std::path::{Path, PathBuf};

use crate::items::ItemDef;
use crate::manifest::ContentDb;
use crate::objects::{HarvestNodeDef, LootTableDef};

/// Overlay file-authored content on top of the Rust bootstrap defaults.
///
/// Missing directories are allowed so local/dev builds can continue to run from
/// the embedded defaults. Invalid files are skipped with a warning rather than
/// panicking; server-side validation still reports cross-reference issues after
/// the database is assembled.
pub fn overlay_content_files(db: &mut ContentDb) {
    let content_root = asset_root().join("content");

    for item in load_json_dir::<ItemDef>(&content_root.join("items")) {
        db.items.items.insert(item.id.clone(), item);
    }

    for node in load_json_dir::<HarvestNodeDef>(&content_root.join("harvest_nodes")) {
        db.harvest.nodes.insert(node.id.clone(), node);
    }

    for table in load_json_dir::<LootTableDef>(&content_root.join("loot_tables")) {
        db.harvest.loot_tables.insert(table.id.clone(), table);
    }
}

fn load_json_dir<T>(dir: &Path) -> Vec<T>
where
    T: DeserializeOwned,
{
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };

    let mut paths = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("json"))
        .collect::<Vec<_>>();

    paths.sort();

    paths
        .into_iter()
        .filter_map(|path| load_json_file::<T>(&path))
        .collect()
}

fn load_json_file<T>(path: &Path) -> Option<T>
where
    T: DeserializeOwned,
{
    let raw = match fs::read_to_string(path) {
        Ok(raw) => raw,
        Err(err) => {
            eprintln!(
                "stonepyre_content: failed to read content file {}: {err}",
                path.display()
            );
            return None;
        }
    };

    match serde_json::from_str::<T>(&raw) {
        Ok(value) => Some(value),
        Err(err) => {
            eprintln!(
                "stonepyre_content: failed to parse content file {}: {err}",
                path.display()
            );
            None
        }
    }
}

fn asset_root() -> PathBuf {
    std::env::var("STONEPYRE_ASSET_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets"))
}
