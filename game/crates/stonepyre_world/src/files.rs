use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::{demo_harvest_node_placements, HarvestNodePlacement, TilePos};

#[derive(Clone, Debug, Deserialize)]
struct HarvestNodePlacementFile {
    node_id: String,
    node_def_id: String,
    tile: TilePos,
    #[serde(default = "default_blocks_movement")]
    blocks_movement: bool,
}

fn default_blocks_movement() -> bool {
    true
}

/// Loads the current demo map's harvest-node placements from assets.
///
/// If the file is missing or invalid, this falls back to the Rust bootstrap
/// placements so local/dev startup remains forgiving while the map format is
/// still evolving.
pub fn load_demo_harvest_node_placements() -> Vec<HarvestNodePlacement> {
    let path = default_demo_harvest_nodes_path();

    match harvest_node_placements_from_file(&path) {
        Ok(placements) => placements,
        Err(err) => {
            eprintln!(
                "stonepyre_world: failed to load demo harvest placements from {}: {}; using Rust fallback",
                path.display(),
                err
            );
            demo_harvest_node_placements()
        }
    }
}

/// Loads harvest-node placements from a JSON file.
///
/// Expected JSON shape:
///
/// ```json
/// [
///   {
///     "node_id": "demo_tree_2_0",
///     "node_def_id": "oak_tree",
///     "tile": { "x": 2, "y": 0 },
///     "blocks_movement": true
///   }
/// ]
/// ```
pub fn harvest_node_placements_from_file(path: impl AsRef<Path>) -> Result<Vec<HarvestNodePlacement>, String> {
    let path = path.as_ref();
    let raw = std::fs::read_to_string(path)
        .map_err(|err| format!("read failed: {err}"))?;

    let parsed: Vec<HarvestNodePlacementFile> = serde_json::from_str(&raw)
        .map_err(|err| format!("json parse failed: {err}"))?;

    Ok(parsed
        .into_iter()
        .map(|placement| HarvestNodePlacement {
            node_id: leak_str(placement.node_id),
            node_def_id: leak_str(placement.node_def_id),
            tile: placement.tile,
            blocks_movement: placement.blocks_movement,
        })
        .collect())
}

fn default_demo_harvest_nodes_path() -> PathBuf {
    asset_root()
        .join("world")
        .join("maps")
        .join("demo")
        .join("harvest_nodes.json")
}

fn asset_root() -> PathBuf {
    std::env::var("STONEPYRE_ASSET_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets"))
}

fn leak_str(value: String) -> &'static str {
    Box::leak(value.into_boxed_str())
}
