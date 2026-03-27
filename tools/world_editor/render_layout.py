# tools/world_editor/render_layout.py
from __future__ import annotations

from dataclasses import dataclass
from typing import Optional, Tuple

import pygame

from .camera import Camera
from .constants import COLOR_BG, COLOR_GRID, COLOR_TEXT, TOP_BAR_H
from .world_layout import WorldLayout


COLOR_INACTIVE = (50, 50, 58)
COLOR_ACTIVE = (90, 170, 110)
COLOR_HOVER = (255, 255, 255)

INFO_Y_1 = TOP_BAR_H + 10
INFO_Y_2 = TOP_BAR_H + 34
GRID_TOP_PAD = 54


@dataclass
class LayoutResult:
    hovered_chunk: Optional[Tuple[int, int]] = None
    changed: bool = False


def render_layout(
    screen: pygame.Surface,
    font: pygame.font.Font,
    camera: Camera,
    layout: WorldLayout,
    cell_px: int,
    mouse_pos: Tuple[int, int],
    left_down: bool,
    right_down: bool,
) -> LayoutResult:
    res = LayoutResult()

    w, h = screen.get_size()
    view_rect = pygame.Rect(0, TOP_BAR_H, w, h - TOP_BAR_H)
    pygame.draw.rect(screen, COLOR_BG, view_rect)

    info_line_1 = "Layout Mode — LMB activate, RMB deactivate, Enter = generate, Tab = toggle"
    screen.blit(font.render(info_line_1, True, COLOR_TEXT), (12, INFO_Y_1))

    if res.hovered_chunk is not None:
        pass

    summary = f"Bounds: {layout.width_chunks}x{layout.height_chunks} chunks | Active: {len(layout.active_chunks)} / {layout.total_chunk_slots}"
    screen.blit(font.render(summary, True, (190, 190, 200)), (12, INFO_Y_2))

    grid_view_rect = pygame.Rect(0, TOP_BAR_H + GRID_TOP_PAD, w, h - (TOP_BAR_H + GRID_TOP_PAD))
    pygame.draw.rect(screen, COLOR_BG, grid_view_rect)

    left_w, top_w = camera.screen_to_world(grid_view_rect.left, grid_view_rect.top)
    right_w, bot_w = camera.screen_to_world(grid_view_rect.right, grid_view_rect.bottom)

    min_cx = max(layout.min_cx, int(left_w // cell_px) - 2)
    max_cx = min(layout.max_cx, int(right_w // cell_px) + 2)
    min_cy = max(layout.min_cy, int(top_w // cell_px) - 2)
    max_cy = min(layout.max_cy, int(bot_w // cell_px) + 2)

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
                res.hovered_chunk = (cx, cy)
                pygame.draw.rect(screen, COLOR_HOVER, rect, 2)

                if left_down and not active:
                    layout.set_active(cx, cy, True)
                    res.changed = True
                elif right_down and active:
                    layout.set_active(cx, cy, False)
                    res.changed = True

    if res.hovered_chunk is not None:
        hover = f"Hovered chunk: {res.hovered_chunk}"
        hover_surf = font.render(hover, True, (190, 190, 200))
        screen.blit(hover_surf, (w - hover_surf.get_width() - 12, INFO_Y_1))

    return res