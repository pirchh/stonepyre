# tools/world_editor/render_overview.py
from __future__ import annotations

from dataclasses import dataclass
from typing import Optional, Tuple

import pygame

from .camera import Camera
from .constants import COLOR_BG, COLOR_GRID, COLOR_TEXT, TOP_BAR_H
from .world_layout import WorldLayout


COLOR_INACTIVE = (36, 36, 42)
COLOR_ACTIVE = (90, 170, 110)
COLOR_HOVER = (255, 255, 255)

INFO_Y_1 = TOP_BAR_H + 10
GRID_TOP_PAD = 34


@dataclass
class OverviewResult:
    clicked_chunk: Optional[Tuple[int, int]] = None


def render_overview(
    screen: pygame.Surface,
    font: pygame.font.Font,
    camera: Camera,
    layout: WorldLayout,
    cell_px: int,
    mouse_pos: Tuple[int, int],
    mouse_clicked: bool,
) -> OverviewResult:
    res = OverviewResult()
    w, h = screen.get_size()

    view_rect = pygame.Rect(0, TOP_BAR_H, w, h - TOP_BAR_H)
    pygame.draw.rect(screen, COLOR_BG, view_rect)

    title = "Paint Overview — click an active chunk to enter it | Tab = layout / generate"
    screen.blit(font.render(title, True, COLOR_TEXT), (12, INFO_Y_1))

    grid_view_rect = pygame.Rect(0, TOP_BAR_H + GRID_TOP_PAD, w, h - (TOP_BAR_H + GRID_TOP_PAD))
    pygame.draw.rect(screen, COLOR_BG, grid_view_rect)

    left_w, top_w = camera.screen_to_world(grid_view_rect.left, grid_view_rect.top)
    right_w, bot_w = camera.screen_to_world(grid_view_rect.right, grid_view_rect.bottom)

    min_cx = max(layout.min_cx, int(left_w // cell_px) - 2)
    max_cx = min(layout.max_cx, int(right_w // cell_px) + 2)
    min_cy = max(layout.min_cy, int(top_w // cell_px) - 2)
    max_cy = min(layout.max_cy, int(bot_w // cell_px) + 2)

    hovered_chunk = None

    for cx in range(min_cx, max_cx + 1):
        for cy in range(min_cy, max_cy + 1):
            wx = cx * cell_px
            wy = cy * cell_px
            sx, sy = camera.world_to_screen(wx, wy)

            rect = pygame.Rect(
                int(sx),
                int(sy),
                max(1, int(cell_px * camera.zoom)),
                max(1, int(cell_px * camera.zoom)),
            )

            if not rect.colliderect(grid_view_rect):
                continue

            active = layout.is_active(cx, cy)
            color = COLOR_ACTIVE if active else COLOR_INACTIVE
            pygame.draw.rect(screen, color, rect)
            pygame.draw.rect(screen, COLOR_GRID, rect, 1)

            if rect.collidepoint(mouse_pos):
                hovered_chunk = (cx, cy)
                pygame.draw.rect(screen, COLOR_HOVER, rect, 2)

                if mouse_clicked and active:
                    res.clicked_chunk = (cx, cy)

    if hovered_chunk is not None:
        hover = f"Hovered chunk: {hovered_chunk}"
        hover_surf = font.render(hover, True, (190, 190, 200))
        screen.blit(hover_surf, (w - hover_surf.get_width() - 12, INFO_Y_1))

    return res