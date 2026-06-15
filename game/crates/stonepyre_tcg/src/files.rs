use std::fs;
use std::path::Path;
use crate::card::CardDefinition;
use crate::registry::CardRegistry;

#[derive(Debug, thiserror::Error)]
pub enum LoadError {
    #[error("IO error reading {path}: {source}")]
    Io { path: String, source: std::io::Error },
    #[error("JSON parse error in {path}: {source}")]
    Parse { path: String, source: serde_json::Error },
}

/// Reads all `*.json` files in `dir` and parses each as a `CardDefinition`.
pub fn load_cards_from_dir(dir: &Path) -> Result<Vec<CardDefinition>, LoadError> {
    let mut cards = Vec::new();

    let entries = fs::read_dir(dir).map_err(|e| LoadError::Io {
        path: dir.display().to_string(),
        source: e,
    })?;

    for entry in entries {
        let entry = entry.map_err(|e| LoadError::Io {
            path: dir.display().to_string(),
            source: e,
        })?;

        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("json") {
            continue;
        }

        let text = fs::read_to_string(&path).map_err(|e| LoadError::Io {
            path: path.display().to_string(),
            source: e,
        })?;

        let card: CardDefinition = serde_json::from_str(&text).map_err(|e| LoadError::Parse {
            path: path.display().to_string(),
            source: e,
        })?;

        cards.push(card);
    }

    Ok(cards)
}

/// Convenience: load all cards from `dir` into a `CardRegistry`.
pub fn load_registry_from_dir(dir: &Path) -> Result<CardRegistry, LoadError> {
    let cards = load_cards_from_dir(dir)?;
    Ok(CardRegistry::from_cards(cards))
}
