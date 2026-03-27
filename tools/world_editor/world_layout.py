# tools/world_editor/world_layout.py
from __future__ import annotations

import json
from dataclasses import dataclass
from pathlib import Path
from typing import Set, Tuple

ChunkCoord = Tuple[int, int]


@dataclass
class WorldLayout:
    min_cx: int
    max_cx: int
    min_cy: int
    max_cy: int
    active_chunks: Set[ChunkCoord]

    def contains(self, cx: int, cy: int) -> bool:
        return self.min_cx <= cx <= self.max_cx and self.min_cy <= cy <= self.max_cy

    def is_active(self, cx: int, cy: int) -> bool:
        return (cx, cy) in self.active_chunks

    def set_active(self, cx: int, cy: int, active: bool) -> None:
        if not self.contains(cx, cy):
            return
        if active:
            self.active_chunks.add((cx, cy))
        else:
            self.active_chunks.discard((cx, cy))

    @property
    def width_chunks(self) -> int:
        return self.max_cx - self.min_cx + 1

    @property
    def height_chunks(self) -> int:
        return self.max_cy - self.min_cy + 1

    @property
    def total_chunk_slots(self) -> int:
        return self.width_chunks * self.height_chunks


def layout_path(world_dir: Path) -> Path:
    return world_dir / "layout.json"


def default_layout(width_chunks: int = 128, height_chunks: int = 128) -> WorldLayout:
    half_w = width_chunks // 2
    half_h = height_chunks // 2
    return WorldLayout(
        min_cx=-half_w,
        max_cx=half_w - 1,
        min_cy=-half_h,
        max_cy=half_h - 1,
        active_chunks=set(),
    )


def save_layout(world_dir: Path, layout: WorldLayout) -> None:
    p = layout_path(world_dir)
    p.write_text(
        json.dumps(
            {
                "version": 1,
                "min_cx": layout.min_cx,
                "max_cx": layout.max_cx,
                "min_cy": layout.min_cy,
                "max_cy": layout.max_cy,
                "active_chunks": [[cx, cy] for (cx, cy) in sorted(layout.active_chunks)],
            },
            indent=2,
        ),
        encoding="utf-8",
    )


def load_layout(world_dir: Path) -> WorldLayout:
    p = layout_path(world_dir)
    if not p.exists():
        layout = default_layout()
        save_layout(world_dir, layout)
        return layout

    raw = json.loads(p.read_text(encoding="utf-8"))
    return WorldLayout(
        min_cx=int(raw["min_cx"]),
        max_cx=int(raw["max_cx"]),
        min_cy=int(raw["min_cy"]),
        max_cy=int(raw["max_cy"]),
        active_chunks={tuple(pair) for pair in raw.get("active_chunks", [])},
    )