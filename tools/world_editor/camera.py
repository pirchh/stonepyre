# tools/world_editor/camera.py
from __future__ import annotations

from dataclasses import dataclass
from typing import Tuple

from .constants import ZOOM_MIN, ZOOM_MAX


@dataclass
class Camera:
    # offset in screen pixels
    ox: float = 0.0
    oy: float = 0.0
    zoom: float = 1.0

    def clamp_zoom(self) -> None:
        self.zoom = max(ZOOM_MIN, min(ZOOM_MAX, self.zoom))

    def screen_to_world(self, sx: float, sy: float) -> Tuple[float, float]:
        wx = (sx - self.ox) / self.zoom
        wy = (sy - self.oy) / self.zoom
        return wx, wy

    def world_to_screen(self, wx: float, wy: float) -> Tuple[float, float]:
        sx = wx * self.zoom + self.ox
        sy = wy * self.zoom + self.oy
        return sx, sy

    def zoom_at(self, factor: float, anchor_sx: float, anchor_sy: float) -> None:
        """
        Zoom around a screen-space anchor point.
        """
        before_wx, before_wy = self.screen_to_world(anchor_sx, anchor_sy)
        self.zoom *= factor
        self.clamp_zoom()
        after_sx, after_sy = self.world_to_screen(before_wx, before_wy)
        # shift offset so anchor remains stable
        self.ox += anchor_sx - after_sx
        self.oy += anchor_sy - after_sy

    def pan(self, dx: float, dy: float) -> None:
        self.ox += dx
        self.oy += dy