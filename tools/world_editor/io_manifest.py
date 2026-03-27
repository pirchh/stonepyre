# tools/world_editor/io_manifest.py
from __future__ import annotations

import json
from dataclasses import dataclass
from pathlib import Path
from typing import Dict, List, Tuple


@dataclass(frozen=True)
class TileDef:
    id: int
    name: str
    color_hex: str
    walkable: bool

    @property
    def rgb(self) -> Tuple[int, int, int]:
        h = self.color_hex.lstrip("#")
        return (int(h[0:2], 16), int(h[2:4], 16), int(h[4:6], 16))


@dataclass(frozen=True)
class Manifest:
    version: int
    chunk_size: int
    tiles: List[TileDef]

    @property
    def tile_by_id(self) -> Dict[int, TileDef]:
        return {t.id: t for t in self.tiles}


DEFAULT_MANIFEST = {
    "version": 1,
    "chunk_size": 256,
    "tiles": [
        {"id": 0, "name": "grass", "color": "#4caf50", "walkable": True},
        {"id": 1, "name": "dirt", "color": "#795548", "walkable": True},
        {"id": 2, "name": "sand", "color": "#e6c27a", "walkable": True},
        {"id": 3, "name": "water", "color": "#2196f3", "walkable": False},
        {"id": 4, "name": "stone", "color": "#9e9e9e", "walkable": True},
        {"id": 5, "name": "cliff", "color": "#555555", "walkable": False},
        {"id": 6, "name": "snow", "color": "#ffffff", "walkable": True},
        {"id": 7, "name": "swamp", "color": "#3f6f3f", "walkable": True},
        {"id": 8, "name": "road", "color": "#bba27a", "walkable": True},
        {"id": 9, "name": "lava", "color": "#ff4500", "walkable": False},
    ],
}


def ensure_world_layout(world_dir: Path) -> None:
    (world_dir / "chunks").mkdir(parents=True, exist_ok=True)
    manifest_path = world_dir / "manifest.json"
    if not manifest_path.exists():
        manifest_path.write_text(json.dumps(DEFAULT_MANIFEST, indent=2), encoding="utf-8")


def load_manifest(world_dir: Path) -> Manifest:
    ensure_world_layout(world_dir)
    manifest_path = world_dir / "manifest.json"
    raw = json.loads(manifest_path.read_text(encoding="utf-8"))

    tiles = [
        TileDef(
            id=int(t["id"]),
            name=str(t["name"]),
            color_hex=str(t["color"]),
            walkable=bool(t["walkable"]),
        )
        for t in raw["tiles"]
    ]

    return Manifest(
        version=int(raw.get("version", 1)),
        chunk_size=int(raw.get("chunk_size", 256)),
        tiles=tiles,
    )