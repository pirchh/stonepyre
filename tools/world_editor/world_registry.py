# tools/world_editor/world_registry.py
from __future__ import annotations

import re
import shutil
from dataclasses import dataclass
from pathlib import Path
from typing import List

from .io_manifest import ensure_world_layout
from .world_layout import default_layout, save_layout


WORLD_NAME_RE = re.compile(r"^[a-zA-Z0-9_\- ]+$")


@dataclass(frozen=True)
class WorldEntry:
    name: str
    path: Path


def sanitize_world_name(name: str) -> str:
    cleaned = name.strip()
    cleaned = cleaned.replace("\\", "_").replace("/", "_")
    cleaned = re.sub(r"\s+", "_", cleaned)
    return cleaned


def list_worlds(worlds_root: Path) -> List[WorldEntry]:
    worlds_root.mkdir(parents=True, exist_ok=True)

    worlds: List[WorldEntry] = []
    for child in worlds_root.iterdir():
        if not child.is_dir():
            continue
        if (child / "manifest.json").exists() and (child / "chunks").exists():
            worlds.append(WorldEntry(name=child.name, path=child))

    worlds.sort(key=lambda w: w.name.lower())
    return worlds


def create_world(worlds_root: Path, world_name: str) -> WorldEntry:
    safe_name = sanitize_world_name(world_name)
    if not safe_name:
        raise ValueError("World name cannot be empty.")

    world_path = worlds_root / safe_name
    if world_path.exists():
        raise ValueError(f"World '{safe_name}' already exists.")

    ensure_world_layout(world_path)
    save_layout(world_path, default_layout(width_chunks=128, height_chunks=128))
    return WorldEntry(name=safe_name, path=world_path)


def delete_world(world_entry: WorldEntry) -> None:
    if not world_entry.path.exists():
        raise ValueError(f"World '{world_entry.name}' does not exist.")
    shutil.rmtree(world_entry.path)