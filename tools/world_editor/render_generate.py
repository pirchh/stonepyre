# tools/world_editor/render_generate.py
from __future__ import annotations

from dataclasses import dataclass

import pygame

from .constants import COLOR_BG, COLOR_TEXT, TOP_BAR_H
from .world_layout import WorldLayout


@dataclass
class GenerateState:
    seed: int = 1337
    land_scale: float = 18.0
    edge_falloff: float = 1.15
    land_bias: float = 0.50
    message: str = ""


def render_generate(
    screen: pygame.Surface,
    font: pygame.font.Font,
    big_font: pygame.font.Font,
    layout: WorldLayout,
    state: GenerateState,
) -> None:
    w, h = screen.get_size()
    view_rect = pygame.Rect(0, TOP_BAR_H, w, h - TOP_BAR_H)
    pygame.draw.rect(screen, COLOR_BG, view_rect)

    title = big_font.render("Generate Mode", True, COLOR_TEXT)
    screen.blit(title, (40, TOP_BAR_H + 16))

    lines = [
        f"Seed: {state.seed}",
        f"Land Scale: {state.land_scale:.2f}",
        f"Edge Falloff: {state.edge_falloff:.2f}",
        f"Land Bias: {state.land_bias:.2f}",
        "",
        f"Active chunks currently: {len(layout.active_chunks)}",
        f"World bounds: {layout.width_chunks} x {layout.height_chunks} chunks",
        "",
        "Hotkeys:",
        "Q / A  => seed + / -",
        "W / S  => land scale + / -",
        "E / D  => edge falloff + / -",
        "R / F  => land bias + / -",
        "",
        "C => generate continent layout",
        "T => bake terrain into active chunks",
        "Tab => toggle layout / generate / overview",
        "Enter => go to paint overview",
        "Arrow keys / WASD => pan other modes",
        "S => save",
    ]

    y = TOP_BAR_H + 64
    for line in lines:
        surf = font.render(line, True, COLOR_TEXT)
        screen.blit(surf, (42, y))
        y += 24

    if state.message:
        msg = font.render(state.message, True, (210, 180, 120))
        screen.blit(msg, (42, y + 18))