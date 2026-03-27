from __future__ import annotations

from dataclasses import dataclass, field
from typing import List, Tuple

import pygame

from .config import DD_BG, DD_BORDER, DD_MENU_BG, DD_SELECTED_BG, UI_TEXT, UI_MUTED, UI_BORDER

# ---------------- Dropdown ----------------

@dataclass
class Dropdown:
    title: str
    items: List[str]
    selected_index: int = 0
    open: bool = False
    rect: pygame.Rect = field(default_factory=lambda: pygame.Rect(0, 0, 0, 0))
    max_items: int = 16  # NEW: per-dropdown cap (no scroll yet, just a nicer UX control)

    def selected(self) -> str:
        if not self.items:
            return ""
        return self.items[self.selected_index % len(self.items)]


def draw_dropdown_header(
    screen: pygame.Surface,
    font: pygame.font.Font,
    dd: Dropdown,
    *,
    x: int,
    y: int,
    w: int,
    h: int,
) -> pygame.Rect:
    header = pygame.Rect(x, y, w, h)
    dd.rect = header
    pygame.draw.rect(screen, DD_BG, header, border_radius=6)
    pygame.draw.rect(screen, DD_BORDER, header, width=1, border_radius=6)

    label = f"{dd.title}: {dd.selected() if dd.items else '(none)'}"
    txt = font.render(label, True, (230, 230, 240))
    screen.blit(txt, (x + 10, y + (h - txt.get_height()) // 2))
    return header


def dropdown_menu_rect(dd: Dropdown, *, gap: int = 6, max_items: int | None = None) -> pygame.Rect:
    if not dd.open or not dd.items:
        return pygame.Rect(0, 0, 0, 0)
    item_h = dd.rect.h
    cap = dd.max_items if max_items is None else max_items
    visible = min(len(dd.items), cap)
    menu_h = item_h * visible
    return pygame.Rect(dd.rect.x, dd.rect.y + dd.rect.h + gap, dd.rect.w, menu_h)


def draw_dropdown_menu(
    screen: pygame.Surface,
    font: pygame.font.Font,
    dd: Dropdown,
    *,
    max_items: int | None = None,
) -> List[Tuple[pygame.Rect, int]]:
    if not dd.open or not dd.items:
        return []

    item_h = dd.rect.h
    cap = dd.max_items if max_items is None else max_items
    visible = dd.items[:cap]
    menu = dropdown_menu_rect(dd, max_items=cap)

    pygame.draw.rect(screen, DD_MENU_BG, menu, border_radius=8)
    pygame.draw.rect(screen, DD_BORDER, menu, width=1, border_radius=8)

    out: List[Tuple[pygame.Rect, int]] = []
    for i, name in enumerate(visible):
        r = pygame.Rect(menu.x, menu.y + i * item_h, menu.w, item_h)
        if i == dd.selected_index:
            pygame.draw.rect(screen, DD_SELECTED_BG, r)

        t = font.render(name, True, (220, 220, 230))
        screen.blit(t, (r.x + 10, r.y + (item_h - t.get_height()) // 2))
        out.append((r, i))
    return out


# ---------------- Manager helpers (were missing) ----------------

def clamp(v: float, lo: float, hi: float) -> float:
    return max(lo, min(hi, v))


def draw_round_rect(surf: pygame.Surface, rect: pygame.Rect, color, radius: int = 12) -> None:
    pygame.draw.rect(surf, color, rect, border_radius=radius)


def draw_button(
    surf: pygame.Surface,
    font: pygame.font.Font,
    rect: pygame.Rect,
    label: str,
    *,
    hovered: bool = False,
    active: bool = False,
    subtle: bool = False,
) -> None:
    if active:
        bg = (70, 90, 130)
    else:
        bg = (54, 54, 72) if hovered else (44, 44, 60)
        if subtle:
            bg = (46, 46, 58) if hovered else (38, 38, 50)

    pygame.draw.rect(surf, bg, rect, border_radius=10)
    pygame.draw.rect(surf, UI_BORDER, rect, width=1, border_radius=10)

    t = font.render(label, True, UI_TEXT)
    surf.blit(t, t.get_rect(center=rect.center))


def draw_pill(
    surf: pygame.Surface,
    font: pygame.font.Font,
    rect: pygame.Rect,
    label: str,
    *,
    color=(140, 170, 255),
) -> None:
    pygame.draw.rect(surf, color, rect, border_radius=999)
    pygame.draw.rect(surf, UI_BORDER, rect, width=1, border_radius=999)
    t = font.render(label, True, (20, 20, 26))
    surf.blit(t, t.get_rect(center=rect.center))


def draw_search_box(
    surf: pygame.Surface,
    font: pygame.font.Font,
    rect: pygame.Rect,
    text: str,
    *,
    focused: bool = False,
) -> None:
    bg = (36, 36, 50) if not focused else (44, 44, 66)
    pygame.draw.rect(surf, bg, rect, border_radius=10)
    pygame.draw.rect(surf, UI_BORDER, rect, width=1, border_radius=10)
    t = font.render(text if text else "Search...", True, UI_TEXT if text else UI_MUTED)
    surf.blit(t, (rect.x + 12, rect.y + (rect.h - t.get_height()) // 2))


def draw_icon_folder(surf: pygame.Surface, rect: pygame.Rect, color) -> None:
    pygame.draw.rect(surf, color, rect, width=2, border_radius=3)
    flap = pygame.Rect(rect.x + 2, rect.y - 4, rect.w // 2, 6)
    pygame.draw.rect(surf, color, flap, width=0, border_radius=2)


def draw_icon_chevron(surf: pygame.Surface, center, size: int, color, *, down: bool) -> None:
    cx, cy = center
    s = size // 2
    if down:
        pts = [(cx - s, cy - s // 2), (cx + s, cy - s // 2), (cx, cy + s)]
    else:
        pts = [(cx - s, cy + s // 2), (cx + s, cy + s // 2), (cx, cy - s)]
    pygame.draw.polygon(surf, color, pts)


def draw_icon_check(surf: pygame.Surface, center, size: int, color) -> None:
    cx, cy = center
    s = size // 2
    pygame.draw.lines(surf, color, False, [(cx - s, cy), (cx - s // 3, cy + s), (cx + s, cy - s)], 3)


def draw_icon_x(surf: pygame.Surface, center, size: int, color) -> None:
    cx, cy = center
    s = size // 2
    pygame.draw.line(surf, color, (cx - s, cy - s), (cx + s, cy + s), 3)
    pygame.draw.line(surf, color, (cx + s, cy - s), (cx - s, cy + s), 3)