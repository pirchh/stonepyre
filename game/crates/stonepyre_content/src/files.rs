use serde::de::DeserializeOwned;
use std::fs;
use std::path::{Path, PathBuf};

use crate::items::ItemDef;
use crate::manifest::ContentDb;
use crate::objects::{HarvestNodeManifest, LootTableDef};

/// Overlay file-authored content on top of the Rust bootstrap defaults.
///
/// Missing directories are allowed so local/dev builds can continue to run from
/// the embedded defaults. Invalid files are skipped with a warning rather than
/// panicking; server-side validation still reports cross-reference issues after
/// the database is assembled.
pub fn overlay_content_files(db: &mut ContentDb) {
    let root = asset_root();
    let content_root = root.join("content");

    // Items
    for item in load_json_dir::<ItemDef>(&content_root.join("items")) {
        db.items.items.insert(item.id.clone(), item);
    }

    // Harvest nodes — scanned from world/harvest_objects/{skill}/{node}/manifest.json
    let harvest_objects_root = root.join("world").join("harvest_objects");
    for node in load_harvest_node_manifests(&harvest_objects_root) {
        db.harvest.nodes.insert(node.id.clone(), node);
    }

    // Loot tables
    for table in load_json_dir::<LootTableDef>(&content_root.join("loot_tables")) {
        db.harvest.loot_tables.insert(table.id.clone(), table);
    }
}

/// Scan `world/harvest_objects/{skill}/{node}/manifest.json` files and convert
/// each into a `HarvestNodeDef` with fully-resolved asset paths.
fn load_harvest_node_manifests(
    harvest_objects_root: &Path,
) -> Vec<crate::objects::HarvestNodeDef> {
    let Ok(skill_entries) = fs::read_dir(harvest_objects_root) else {
        return Vec::new();
    };

    let mut defs = Vec::new();

    for skill_entry in skill_entries.filter_map(Result::ok) {
        let skill_path = skill_entry.path();
        if !skill_path.is_dir() { continue; }
        let skill_name = match skill_path.file_name().and_then(|n| n.to_str()) {
            Some(s) => s.to_string(),
            None => continue,
        };

        let Ok(node_entries) = fs::read_dir(&skill_path) else { continue; };

        for node_entry in node_entries.filter_map(Result::ok) {
            let node_path = node_entry.path();
            if !node_path.is_dir() { continue; }
            let node_folder = match node_path.file_name().and_then(|n| n.to_str()) {
                Some(s) => s.to_string(),
                None => continue,
            };

            let manifest_path = node_path.join("manifest.json");
            if !manifest_path.exists() { continue; }

            match load_json_file::<HarvestNodeManifest>(&manifest_path) {
                Some(manifest) => {
                    defs.push(manifest.into_def(&skill_name, &node_folder));
                }
                None => { /* warning already printed by load_json_file */ }
            }
        }
    }

    defs.sort_by(|a, b| a.id.cmp(&b.id));
    defs
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
                "stonepyre_content: failed to read {}: {err}",
                path.display()
            );
            return None;
        }
    };

    match serde_json::from_str::<T>(&raw) {
        Ok(value) => Some(value),
        Err(err) => {
            eprintln!(
                "stonepyre_content: failed to parse {}: {err}",
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
