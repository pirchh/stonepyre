# tools/world_editor/palette.py
from __future__ import annotations

from dataclasses import dataclass
from typing import Tuple

import pygame

from .constants import COLOR_PANEL, COLOR_PANEL_BORDER, COLOR_TEXT, PANEL_PAD, RIGHT_PANEL_W, TOP_BAR_H
from .io_manifest import Manifest


@dataclass
class PaletteState:
    selected_tile_id: int = 0
    brush_size: int = 1

    def set_selected(self, tile_id: int) -> None:
        self.selected_tile_id = int(tile_id)

    def set_brush_size(self, value: int) -> None:
        self.brush_size = max(1, min(16, int(value)))


def draw_right_panel(
    screen: pygame.Surface,
    font: pygame.font.Font,
    manifest: Manifest,
    palette: PaletteState,
    mouse_pos: Tuple[int, int],
    mouse_clicked: bool,
) -> Tuple[bool, int]:
    """
    Returns: (clicked_tile, tile_id_if_clicked)
    """
    w, h = screen.get_size()
    panel_x = w - RIGHT_PANEL_W
    panel_rect = pygame.Rect(panel_x, 0, RIGHT_PANEL_W, h)

    pygame.draw.rect(screen, COLOR_PANEL, panel_rect)
    pygame.draw.rect(screen, COLOR_PANEL_BORDER, panel_rect, 1)

    title = font.render("Palette", True, COLOR_TEXT)
    screen.blit(title, (panel_x + PANEL_PAD, TOP_BAR_H + 8))

    swatch_size = 28
    row_h = 40
    cols = 2
    x0 = panel_x + PANEL_PAD
    y0 = TOP_BAR_H + 42

    tiles = sorted(manifest.tiles, key=lambda t: t.id)

    clicked = False
    clicked_id = palette.selected_tile_id

    for i, t in enumerate(tiles):
        col = i % cols
        row = i // cols

        block_x = x0 + col * 108
        block_y = y0 + row * row_h

        swatch_rect = pygame.Rect(block_x, block_y, swatch_size, swatch_size)
        pygame.draw.rect(screen, t.rgb, swatch_rect)

        if t.id == palette.selected_tile_id:
            pygame.draw.rect(screen, (255, 255, 255), swatch_rect, 3)
        else:
            pygame.draw.rect(screen, (0, 0, 0), swatch_rect, 1)

        label = font.render(f"{t.id}: {t.name}", True, COLOR_TEXT)
        screen.blit(label, (block_x + swatch_size + 8, block_y + 6))

        click_rect = pygame.Rect(block_x, block_y, 100, swatch_size)
        if mouse_clicked and click_rect.collidepoint(mouse_pos):
            clicked = True
            clicked_id = t.id

    selected = manifest.tile_by_id.get(palette.selected_tile_id)
    selected_name = selected.name if selected else "unknown"

    selected_text = font.render(f"Selected: {palette.selected_tile_id} ({selected_name})", True, COLOR_TEXT)
    screen.blit(selected_text, (panel_x + PANEL_PAD, y0 + 5 * row_h + 16))

    brush_text = font.render(f"Brush Size: {palette.brush_size}", True, COLOR_TEXT)
    screen.blit(brush_text, (panel_x + PANEL_PAD, y0 + 5 * row_h + 44))

    help_y = h - 190
    lines = [
        "Hotkeys:",
        "1-9 => tiles 0-8",
        "0 => lava (9)",
        "[ / ] => brush -/+",
        "S => save all dirty",
        "ESC => back/menu",
        "Mouse wheel => zoom",
        "Arrow keys => pan",
    ]
    for j, line in enumerate(lines):
        surf = font.render(line, True, (190, 190, 200))
        screen.blit(surf, (panel_x + PANEL_PAD, help_y + j * 20))

    return clicked, clicked_id