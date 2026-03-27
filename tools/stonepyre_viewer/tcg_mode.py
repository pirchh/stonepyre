# tools/stonepyre_viewer/tcg_mode.py
from __future__ import annotations

from dataclasses import dataclass
from typing import Tuple

import pygame

from .config import UI_BG, UI_TOP, UI_TEXT, UI_MUTED, UI_BORDER
from .ui_common import draw_round_rect, draw_button


@dataclass
class TcgState:
    # stub state for now; we’ll wire pet browsing + compositor next
    pass


def render_tcg(screen: pygame.Surface, mouse_pos: Tuple[int, int]) -> None:
    screen.fill(UI_BG)

    w, h = screen.get_width(), screen.get_height()

    top = pygame.Rect(0, 0, w, 70)
    draw_round_rect(screen, top, UI_TOP, radius=0)
    pygame.draw.line(screen, UI_BORDER, (0, top.bottom), (w, top.bottom), 1)

    font_title = pygame.font.SysFont("Segoe UI Semibold", 34)
    font_sub = pygame.font.SysFont("Segoe UI", 18)

    title = font_title.render("TCG Viewer (WIP)", True, UI_TEXT)
    screen.blit(title, (16, 14))

    sub = font_sub.render("Next: pet browser + crop/scale portrait + compose background/panel/border/icons/text.", True, UI_MUTED)
    screen.blit(sub, (16, 48))

    # Placeholder card preview rectangle
    card = pygame.Rect(w // 2 - 220, h // 2 - 320, 440, 640)
    pygame.draw.rect(screen, (0, 0, 0), card)  # border
    inner = card.inflate(-18, -18)
    pygame.draw.rect(screen, (180, 60, 60), pygame.Rect(inner.x, inner.y, inner.w, int(inner.h * 0.35)))  # top bg
    pygame.draw.rect(screen, (60, 150, 80), pygame.Rect(inner.x, inner.y + int(inner.h * 0.35), inner.w, int(inner.h * 0.65)))  # panel

    # back-of-card placeholder
    back = pygame.Rect(60, h // 2 - 240, 280, 480)
    pygame.draw.rect(screen, (220, 140, 40), back)

    # mat placeholder
    mat = pygame.Rect(w - 420, h // 2 - 240, 340, 480)
    pygame.draw.rect(screen, (230, 230, 230), mat)

    # mode buttons hints (visual only; real switching handled in app)
    btn_font = pygame.font.SysFont("Segoe UI", 18)
    b1 = pygame.Rect(w - 520, 16, 150, 40)
    b2 = pygame.Rect(w - 360, 16, 150, 40)
    draw_button(screen, btn_font, b1, "Viewer", hovered=b1.collidepoint(mouse_pos), subtle=True)
    draw_button(screen, btn_font, b2, "Manager", hovered=b2.collidepoint(mouse_pos), subtle=True)
