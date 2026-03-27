# tools/world_editor/render_menu.py
from __future__ import annotations

from dataclasses import dataclass
from typing import List, Optional, Tuple

import pygame

from .world_registry import WorldEntry


BG = (16, 16, 20)
PANEL = (24, 24, 30)
PANEL_BORDER = (60, 60, 72)
TEXT = (230, 230, 235)
SUBTEXT = (180, 180, 190)
BUTTON = (48, 48, 58)
BUTTON_HOVER = (62, 62, 76)
BUTTON_BORDER = (92, 92, 110)
DANGER = (92, 42, 42)
DANGER_HOVER = (120, 52, 52)
INPUT_BG = (20, 20, 24)
SELECT_BG = (56, 72, 110)


@dataclass
class MenuState:
    new_world_name: str = ""
    selected_index: int = 0
    message: str = ""
    input_active: bool = True


@dataclass
class MenuResult:
    create_requested: bool = False
    load_requested: bool = False
    delete_requested: bool = False
    selected_world_index: Optional[int] = None


def _draw_button(
    screen: pygame.Surface,
    font: pygame.font.Font,
    rect: pygame.Rect,
    label: str,
    mouse_pos: Tuple[int, int],
    mouse_clicked: bool,
    danger: bool = False,
) -> bool:
    hovered = rect.collidepoint(mouse_pos)
    base = DANGER if danger else BUTTON
    hover = DANGER_HOVER if danger else BUTTON_HOVER

    pygame.draw.rect(screen, hover if hovered else base, rect, border_radius=6)
    pygame.draw.rect(screen, BUTTON_BORDER, rect, 1, border_radius=6)

    surf = font.render(label, True, TEXT)
    sx = rect.x + (rect.w - surf.get_width()) // 2
    sy = rect.y + (rect.h - surf.get_height()) // 2
    screen.blit(surf, (sx, sy))

    return hovered and mouse_clicked


def render_menu(
    screen: pygame.Surface,
    font: pygame.font.Font,
    big_font: pygame.font.Font,
    state: MenuState,
    worlds: List[WorldEntry],
    mouse_pos: Tuple[int, int],
    mouse_clicked: bool,
) -> MenuResult:
    result = MenuResult()

    w, h = screen.get_size()
    screen.fill(BG)

    title = big_font.render("Stonepyre World Editor", True, TEXT)
    screen.blit(title, (40, 30))

    subtitle = font.render("Create, load, or delete worlds.", True, SUBTEXT)
    screen.blit(subtitle, (42, 74))

    # Left panel
    left_rect = pygame.Rect(40, 120, 460, h - 180)
    pygame.draw.rect(screen, PANEL, left_rect, border_radius=8)
    pygame.draw.rect(screen, PANEL_BORDER, left_rect, 1, border_radius=8)

    left_title = font.render("Create World", True, TEXT)
    screen.blit(left_title, (left_rect.x + 16, left_rect.y + 16))

    name_label = font.render("World name", True, SUBTEXT)
    screen.blit(name_label, (left_rect.x + 16, left_rect.y + 56))

    input_rect = pygame.Rect(left_rect.x + 16, left_rect.y + 82, left_rect.w - 32, 42)
    pygame.draw.rect(screen, INPUT_BG, input_rect, border_radius=6)
    pygame.draw.rect(screen, (120, 120, 140) if state.input_active else PANEL_BORDER, input_rect, 1, border_radius=6)

    input_text = state.new_world_name if state.new_world_name else "Enter world name..."
    input_color = TEXT if state.new_world_name else (120, 120, 130)
    screen.blit(font.render(input_text, True, input_color), (input_rect.x + 12, input_rect.y + 10))

    create_rect = pygame.Rect(left_rect.x + 16, left_rect.y + 140, 180, 40)
    if _draw_button(screen, font, create_rect, "Create World", mouse_pos, mouse_clicked):
        result.create_requested = True

    helper_lines = [
        "Suggested examples:",
        "stonepyre_alpha",
        "world_01",
        "eastern_realm_test",
    ]
    for i, line in enumerate(helper_lines):
        color = TEXT if i == 0 else SUBTEXT
        screen.blit(font.render(line, True, color), (left_rect.x + 16, left_rect.y + 210 + i * 24))

    # Right panel
    right_rect = pygame.Rect(540, 120, w - 580, h - 180)
    pygame.draw.rect(screen, PANEL, right_rect, border_radius=8)
    pygame.draw.rect(screen, PANEL_BORDER, right_rect, 1, border_radius=8)

    right_title = font.render("Worlds", True, TEXT)
    screen.blit(right_title, (right_rect.x + 16, right_rect.y + 16))

    list_rect = pygame.Rect(right_rect.x + 16, right_rect.y + 52, right_rect.w - 32, right_rect.h - 120)
    pygame.draw.rect(screen, INPUT_BG, list_rect, border_radius=6)
    pygame.draw.rect(screen, PANEL_BORDER, list_rect, 1, border_radius=6)

    if not worlds:
        empty = font.render("No worlds found yet.", True, SUBTEXT)
        screen.blit(empty, (list_rect.x + 12, list_rect.y + 12))
    else:
        row_h = 34
        for i, world in enumerate(worlds):
            row_rect = pygame.Rect(list_rect.x + 8, list_rect.y + 8 + i * row_h, list_rect.w - 16, row_h - 4)
            selected = (i == state.selected_index)

            if selected:
                pygame.draw.rect(screen, SELECT_BG, row_rect, border_radius=4)
            elif row_rect.collidepoint(mouse_pos):
                pygame.draw.rect(screen, (36, 36, 42), row_rect, border_radius=4)

            label = font.render(world.name, True, TEXT)
            screen.blit(label, (row_rect.x + 10, row_rect.y + 6))

            if mouse_clicked and row_rect.collidepoint(mouse_pos):
                result.selected_world_index = i

    load_rect = pygame.Rect(right_rect.x + 16, right_rect.bottom - 52, 160, 36)
    delete_rect = pygame.Rect(right_rect.x + 188, right_rect.bottom - 52, 160, 36)

    if _draw_button(screen, font, load_rect, "Load Selected", mouse_pos, mouse_clicked):
        result.load_requested = True

    if _draw_button(screen, font, delete_rect, "Delete Selected", mouse_pos, mouse_clicked, danger=True):
        result.delete_requested = True

    if state.message:
        msg = font.render(state.message, True, (210, 180, 120))
        screen.blit(msg, (40, h - 42))

    return result