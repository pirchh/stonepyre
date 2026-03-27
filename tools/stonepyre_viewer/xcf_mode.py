# tools/stonepyre_viewer/xcf_mode.py
from __future__ import annotations

import time
from dataclasses import dataclass
from pathlib import Path
from typing import Optional, Tuple

import pygame

from .config import UI_BG, UI_TOP, UI_TEXT, UI_MUTED, UI_BORDER, UI_PANEL
from .ui_common import draw_round_rect, draw_button
from .xcf_importer import run_xcf_import, XcfImportResult


@dataclass
class XcfState:
    last_xcf: Optional[Path] = None
    last_result: Optional[XcfImportResult] = None
    last_msg: str = ""
    last_msg_ts: float = 0.0

    def msg(self, s: str):
        self.last_msg = s
        self.last_msg_ts = time.time()


def _pick_xcf_dialog(initial_dir: Optional[Path] = None) -> Optional[Path]:
    try:
        import tkinter as tk
        from tkinter import filedialog

        root = tk.Tk()
        root.withdraw()
        p = filedialog.askopenfilename(
            title="Select Pet XCF",
            initialdir=str(initial_dir) if initial_dir else None,
            filetypes=[("GIMP XCF", "*.xcf")],
        )
        if not p:
            return None
        return Path(p)
    except Exception:
        return None


def render_xcf(screen: pygame.Surface, mouse_pos: Tuple[int, int], *, xs: XcfState) -> None:
    screen.fill(UI_BG)
    w, h = screen.get_width(), screen.get_height()

    # top bar
    top = pygame.Rect(0, 0, w, 70)
    draw_round_rect(screen, top, UI_TOP, radius=0)
    pygame.draw.line(screen, UI_BORDER, (0, top.bottom), (w, top.bottom), 1)

    font_title = pygame.font.SysFont("Segoe UI Semibold", 34)
    font_sub = pygame.font.SysFont("Segoe UI", 18)
    font_ui = pygame.font.SysFont("Segoe UI", 18)

    screen.blit(font_title.render("XCF Import", True, UI_TEXT), (16, 14))
    screen.blit(font_sub.render("Import pet sheets from GIMP layers -> greyscale outputs + templates.", True, UI_MUTED), (16, 48))

    # panel
    panel = pygame.Rect(16, 90, w - 32, h - 130)
    draw_round_rect(screen, panel, UI_PANEL, radius=18)
    pygame.draw.rect(screen, UI_BORDER, panel, width=1, border_radius=18)

    # buttons
    btn_pick = pygame.Rect(40, 120, 220, 44)
    btn_run = pygame.Rect(270, 120, 220, 44)

    draw_button(screen, font_ui, btn_pick, "Choose XCF...", hovered=btn_pick.collidepoint(mouse_pos), subtle=False)
    draw_button(screen, font_ui, btn_run, "Run Import", hovered=btn_run.collidepoint(mouse_pos), subtle=xs.last_xcf is None)

    # status text
    y = 190
    if xs.last_xcf:
        screen.blit(font_ui.render(f"Selected: {xs.last_xcf.as_posix()}", True, UI_TEXT), (40, y))
        y += 28
    else:
        screen.blit(font_ui.render("Selected: (none)", True, UI_MUTED), (40, y))
        y += 28

    if xs.last_result:
        r = xs.last_result
        lines = [
            f"pet_name: {r.pet_name}",
            f"exported_raw: {r.exported_raw_count}   parsed_layers: {r.parsed_layer_count}   skipped: {r.skipped_layer_count}",
            f"written_pairs: {len(r.written_pairs)}",
            f"raw_export_dir: {r.raw_export_dir.as_posix()}",
            f"greyscale_structured_root: {r.structured_greyscale_root.as_posix()}",
            f"flat_greyscale_dir: {r.flat_greyscale_dir.as_posix()}",
        ]
        if r.greyscale_xcf_out:
            lines.append(f"greyscale_xcf_out: {r.greyscale_xcf_out.as_posix()}")
        if r.greyscale_xcf_next_to_original:
            lines.append(f"greyscale_xcf_next_to_original: {r.greyscale_xcf_next_to_original.as_posix()}")

        y += 16
        for ln in lines:
            screen.blit(font_ui.render(ln, True, UI_TEXT), (40, y))
            y += 24

    # ephemeral message
    if xs.last_msg and (time.time() - xs.last_msg_ts) < 5.0:
        screen.blit(font_ui.render(xs.last_msg, True, UI_TEXT), (40, h - 32))


def handle_xcf_click(pos: Tuple[int, int], *, xs: XcfState) -> bool:
    w, _h = pygame.display.get_surface().get_size()  # safe enough
    btn_pick = pygame.Rect(40, 120, 220, 44)
    btn_run = pygame.Rect(270, 120, 220, 44)

    if btn_pick.collidepoint(pos):
        picked = _pick_xcf_dialog(xs.last_xcf.parent if xs.last_xcf else None)
        if picked:
            xs.last_xcf = picked
            xs.msg("XCF selected")
        else:
            xs.msg("Pick cancelled")
        return True

    if btn_run.collidepoint(pos):
        if not xs.last_xcf:
            xs.msg("No XCF selected")
            return True
        try:
            xs.last_result = run_xcf_import(xs.last_xcf)
            xs.msg(f"Import complete: {xs.last_result.pet_name}")
        except Exception as e:
            xs.msg(f"[ERR] Import failed: {e}")
        return True

    return False