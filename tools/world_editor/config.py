# tools/world_editor/config.py
from __future__ import annotations

import os
from dataclasses import dataclass
from pathlib import Path

from .constants import (
    CHUNK_SIZE_DEFAULT,
    TILE_PX_DEFAULT,
    OVERVIEW_CELL_PX_DEFAULT,
)

@dataclass(frozen=True)
class EditorConfig:
    # Repo-relative default world path (from /tools)
    world_dir: Path

    # Defaults (can later be put into a settings file)
    chunk_size: int = CHUNK_SIZE_DEFAULT
    tile_px: int = TILE_PX_DEFAULT
    overview_cell_px: int = OVERVIEW_CELL_PX_DEFAULT


def default_config() -> EditorConfig:
    """
    Assumes working directory is .../Stonepyre/tools when you run:
        python world.py
    """
    cwd = Path(os.getcwd())
    # tools/assets/worlds/world_01/
    world_dir = cwd / "assets" / "worlds" / "world_01"
    return EditorConfig(world_dir=world_dir)