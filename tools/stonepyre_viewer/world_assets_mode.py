from __future__ import annotations

import json
import os
import shutil
import subprocess
from dataclasses import dataclass, field
from pathlib import Path
from typing import Dict, List, Optional, Tuple

import pygame

try:
    import tkinter as tk
    from tkinter import filedialog
except Exception:  # pragma: no cover
    tk = None
    filedialog = None

from .config import PROJECT_ROOT, START_W, START_H, LOCK_TO_1080P
from .ui_common import draw_button

# Tool-owned source assets. These are editable/assignable from the viewer.
TOOL_WORLD_ASSETS_ROOT = PROJECT_ROOT / "libs" / "world_assets"
TOOL_SKILLS_ROOT = TOOL_WORLD_ASSETS_ROOT / "skills"

# Runtime Bevy asset root. Exports copy into this tree using canonical relative paths.
GAME_ASSETS_ROOT = PROJECT_ROOT / "game" / "assets"
GAME_WORLD_SKILLS_ROOT = GAME_ASSETS_ROOT / "world" / "skills"

ASSET_CATEGORY = "harvest_nodes"
AVAILABLE_NAME = "available.png"
DEPLETED_NAME = "depleted.png"
MANIFEST_NAME = "manifest.json"
DEFAULT_ANCHOR = (0.5, 0.88)

BG = (16, 16, 24)
PANEL = (26, 26, 38)
CARD = (34, 34, 50)
CARD_DARK = (20, 20, 30)
BORDER = (74, 74, 98)
BORDER_ACTIVE = (126, 156, 224)
TEXT = (236, 238, 248)
MUTED = (166, 168, 188)
DIM = (118, 120, 140)
WARN = (248, 204, 95)
OK = (130, 222, 154)
BAD = (242, 116, 128)
INFO = (132, 166, 235)
INPUT_BG = (24, 24, 36)
BUTTON_ACTIVE = (72, 94, 142)
MENU_BG = (28, 28, 42)
MENU_HOVER = (48, 52, 76)

NO_SKILLS = "(no skills yet)"
NO_NODES = "(no nodes yet)"


@dataclass
class WorldAssetState:
    skill_ids: List[str] = field(default_factory=list)
    node_ids: List[str] = field(default_factory=list)
    selected_skill_index: int = 0
    selected_node_index: int = 0

    skill_dropdown_open: bool = False
    node_dropdown_open: bool = False

    create_mode: Optional[str] = None  # "skill" | "node"
    draft_skill_id: str = "woodcutting"
    draft_node_id: str = "oak_tree"
    editing_field: Optional[str] = None  # "create_skill" | "create_node"

    status: str = "World Assets mode ready. Select or create a skill, then create a node."
    status_ok: bool = True
    surfaces: Dict[str, Optional[pygame.Surface]] = field(default_factory=lambda: {"available": None, "depleted": None})
    anchors: Dict[str, Tuple[float, float]] = field(default_factory=lambda: {"available": DEFAULT_ANCHOR, "depleted": DEFAULT_ANCHOR})

    def selected_skill_id(self) -> str:
        if not self.skill_ids:
            return ""
        return self.skill_ids[self.selected_skill_index % len(self.skill_ids)]

    def selected_node_id(self) -> str:
        if not self.node_ids:
            return ""
        return self.node_ids[self.selected_node_index % len(self.node_ids)]

    def assigned_count(self) -> int:
        skill = self.selected_skill_id()
        node = self.selected_node_id()
        if not skill or not node:
            return 0
        return int(available_tool_path(skill, node).exists()) + int(depleted_tool_path(skill, node).exists())


def main() -> None:
    pygame.init()
    pygame.display.set_caption("Stonepyre World Assets")

    flags = pygame.SCALED | pygame.RESIZABLE
    try:
        screen = pygame.display.set_mode((START_W, START_H), flags)
    except pygame.error:
        screen = pygame.display.set_mode((START_W, START_H), pygame.RESIZABLE)

    clock = pygame.time.Clock()
    font = pygame.font.SysFont("consolas", 18)
    font_sm = pygame.font.SysFont("consolas", 15)
    font_mid = pygame.font.SysFont("consolas", 22)
    font_big = pygame.font.SysFont("consolas", 30)

    state = WorldAssetState()
    refresh_all(state, quiet=True)
    running = True
    ui_rects: Dict[str, pygame.Rect] = {}

    while running:
        clock.tick(60)
        for ev in pygame.event.get():
            if ev.type == pygame.QUIT:
                running = False
                break
            if ev.type == pygame.VIDEORESIZE:
                if LOCK_TO_1080P:
                    screen = pygame.display.set_mode((START_W, START_H), pygame.RESIZABLE)
                else:
                    screen = pygame.display.set_mode((ev.w, ev.h), pygame.RESIZABLE)
            if ev.type == pygame.KEYDOWN:
                if ev.key == pygame.K_ESCAPE:
                    if state.create_mode:
                        close_create_panel(state)
                    else:
                        running = False
                    continue
                if state.editing_field:
                    handle_text_input_key(ev, state)
                    continue
                if ev.key == pygame.K_r:
                    refresh_all(state)
            if ev.type == pygame.TEXTINPUT and state.editing_field:
                apply_text_input(ev.text, state)
            if ev.type == pygame.MOUSEBUTTONDOWN and ev.button == 1:
                handle_click(ev.pos, state, ui_rects)

        ui_rects = render(screen, font, font_sm, font_mid, font_big, state)
        pygame.display.flip()

    pygame.quit()


def handle_text_input_key(ev: pygame.event.Event, state: WorldAssetState) -> None:
    if ev.key == pygame.K_RETURN:
        if state.editing_field == "create_skill":
            ensure_skill_from_draft(state)
        elif state.editing_field == "create_node":
            ensure_node_from_draft(state)
        return
    if ev.key == pygame.K_BACKSPACE:
        if state.editing_field == "create_skill":
            state.draft_skill_id = state.draft_skill_id[:-1]
        elif state.editing_field == "create_node":
            state.draft_node_id = state.draft_node_id[:-1]
        return
    if ev.key == pygame.K_TAB:
        state.editing_field = None
        return

    # Embedded viewer path receives KEYDOWN unicode, not TEXTINPUT.
    text = getattr(ev, "unicode", "") or ""
    if text:
        apply_text_input(text, state)


def apply_text_input(text: str, state: WorldAssetState) -> None:
    cleaned = "".join(ch.lower() for ch in text if ch.isalnum() or ch in "_-")
    if not cleaned:
        return
    if state.editing_field == "create_skill":
        state.draft_skill_id += cleaned
    elif state.editing_field == "create_node":
        state.draft_node_id += cleaned


def handle_click(pos: Tuple[int, int], state: WorldAssetState, ui_rects: Dict[str, pygame.Rect]) -> None:
    # Dropdown options first, because they can overlap other cards.
    for i, _skill in enumerate(state.skill_ids[:10]):
        r = ui_rects.get(f"skill_option_{i}")
        if r and r.collidepoint(pos):
            state.selected_skill_index = i
            state.skill_dropdown_open = False
            state.node_dropdown_open = False
            state.create_mode = None
            refresh_nodes(state)
            reload_previews(state)
            set_status(state, f"Selected skill {state.selected_skill_id()}.", True)
            return

    for i, _node in enumerate(state.node_ids[:10]):
        r = ui_rects.get(f"node_option_{i}")
        if r and r.collidepoint(pos):
            state.selected_node_index = i
            state.node_dropdown_open = False
            state.skill_dropdown_open = False
            state.create_mode = None
            reload_previews(state)
            set_status(state, f"Selected node {state.selected_node_id()}.", True)
            return

    if ui_rects.get("skill_dropdown", pygame.Rect(0, 0, 0, 0)).collidepoint(pos):
        state.skill_dropdown_open = not state.skill_dropdown_open
        state.node_dropdown_open = False
        state.create_mode = None
        state.editing_field = None
        return
    if ui_rects.get("node_dropdown", pygame.Rect(0, 0, 0, 0)).collidepoint(pos):
        if not state.selected_skill_id():
            set_status(state, "Create or select a skill before choosing a node.", False)
            return
        state.node_dropdown_open = not state.node_dropdown_open
        state.skill_dropdown_open = False
        state.create_mode = None
        state.editing_field = None
        return

    if ui_rects.get("create_skill", pygame.Rect(0, 0, 0, 0)).collidepoint(pos):
        state.create_mode = "skill"
        state.editing_field = "create_skill"
        state.skill_dropdown_open = False
        state.node_dropdown_open = False
        if not state.draft_skill_id:
            state.draft_skill_id = "woodcutting"
        return
    if ui_rects.get("create_node", pygame.Rect(0, 0, 0, 0)).collidepoint(pos):
        if not state.selected_skill_id():
            set_status(state, "Create or select a skill before creating a node.", False)
            return
        state.create_mode = "node"
        state.editing_field = "create_node"
        state.skill_dropdown_open = False
        state.node_dropdown_open = False
        if not state.draft_node_id:
            state.draft_node_id = "oak_tree"
        return

    if ui_rects.get("create_input", pygame.Rect(0, 0, 0, 0)).collidepoint(pos):
        if state.create_mode == "skill":
            state.editing_field = "create_skill"
        elif state.create_mode == "node":
            state.editing_field = "create_node"
        return

    action_map = {
        "confirm_create": lambda: ensure_skill_from_draft(state) if state.create_mode == "skill" else ensure_node_from_draft(state),
        "cancel_create": lambda: close_create_panel(state),
        "assign_available": lambda: assign_sprite(state, "available"),
        "assign_depleted": lambda: assign_sprite(state, "depleted"),
        "send_to_game": lambda: send_to_game_assets(state),
        "open_tool_folder": lambda: open_folder(current_tool_folder(state), state),
        "open_game_folder": lambda: open_folder(current_game_folder(state), state),
        "copy_available_anchor": lambda: copy_available_anchor_to_depleted(state),
        "refresh": lambda: refresh_all(state),
    }
    for key, callback in action_map.items():
        rect = ui_rects.get(key)
        if rect and rect.collidepoint(pos):
            state.skill_dropdown_open = False
            state.node_dropdown_open = False
            callback()
            return

    if set_anchor_from_preview_click(state, ui_rects, pos, "available"):
        return
    if set_anchor_from_preview_click(state, ui_rects, pos, "depleted"):
        return

    # Click outside closes dropdowns but keeps create panels open.
    state.skill_dropdown_open = False
    state.node_dropdown_open = False


def render(
    screen: pygame.Surface,
    font: pygame.font.Font,
    font_sm: pygame.font.Font,
    font_mid: pygame.font.Font,
    font_big: pygame.font.Font,
    state: WorldAssetState,
) -> Dict[str, pygame.Rect]:
    screen.fill(BG)
    mouse = pygame.mouse.get_pos()
    rects: Dict[str, pygame.Rect] = {}

    draw_header(screen, font, font_big, state)

    left = pygame.Rect(28, 110, 500, 870)
    center = pygame.Rect(552, 110, 610, 870)
    right = pygame.Rect(1186, 110, 648, 870)
    draw_panel(screen, left)
    draw_panel(screen, center)
    draw_panel(screen, right)

    draw_library_column(screen, font, font_sm, font_mid, left, state, rects, mouse)
    draw_asset_column(screen, font, font_sm, font_mid, center, state, rects, mouse)
    draw_preview_column(screen, font, font_sm, font_mid, right, state, rects)

    return rects


def draw_header(screen: pygame.Surface, font: pygame.font.Font, font_big: pygame.font.Font, state: WorldAssetState) -> None:
    screen.blit(font_big.render("Stonepyre World Assets", True, TEXT), (28, 24))
    skill = state.selected_skill_id() or "No skill selected"
    node = state.selected_node_id() or "No node selected"
    assigned = state.assigned_count()
    summary = f"Skill: {skill}   |   Node: {node}   |   Sprites: {assigned}/2 assigned"
    screen.blit(font.render(summary, True, OK if assigned == 2 else MUTED), (30, 66))


def draw_library_column(screen, font, font_sm, font_mid, panel, state, rects, mouse):
    y = panel.y + 22
    screen.blit(font_mid.render("Library", True, TEXT), (panel.x + 22, y))
    y += 42

    skill_card = pygame.Rect(panel.x + 18, y, panel.w - 36, 136)
    draw_card(screen, skill_card)
    screen.blit(font.render("Skill", True, TEXT), (skill_card.x + 18, skill_card.y + 14))
    skill_dd = pygame.Rect(skill_card.x + 18, skill_card.y + 46, skill_card.w - 190, 42)
    create_skill = pygame.Rect(skill_dd.right + 14, skill_dd.y, 140, 42)
    rects["skill_dropdown"] = skill_dd
    rects["create_skill"] = create_skill
    draw_dropdown(screen, font, skill_dd, state.selected_skill_id() or NO_SKILLS, state.skill_dropdown_open, skill_dd.collidepoint(mouse))
    draw_button(screen, font_sm, create_skill, "+ Create Skill", hovered=create_skill.collidepoint(mouse), active=True)
    draw_inline_hint(screen, font_sm, skill_card.x + 18, skill_card.y + 100, "Source", compact_path(skill_tool_dir(state.selected_skill_id())) if state.selected_skill_id() else "No skill folder yet")

    if state.skill_dropdown_open:
        draw_dropdown_options(screen, font, skill_dd, state.skill_ids or [NO_SKILLS], "skill_option", rects, disabled=not state.skill_ids)

    y = skill_card.bottom + 18
    node_card = pygame.Rect(panel.x + 18, y, panel.w - 36, 150)
    draw_card(screen, node_card)
    screen.blit(font.render("Node Type", True, TEXT), (node_card.x + 18, node_card.y + 14))
    node_dd = pygame.Rect(node_card.x + 18, node_card.y + 46, node_card.w - 190, 42)
    create_node = pygame.Rect(node_dd.right + 14, node_dd.y, 140, 42)
    rects["node_dropdown"] = node_dd
    rects["create_node"] = create_node
    draw_dropdown(screen, font, node_dd, state.selected_node_id() or NO_NODES, state.node_dropdown_open, node_dd.collidepoint(mouse), disabled=not state.selected_skill_id())
    draw_button(screen, font_sm, create_node, "+ Create Node", hovered=create_node.collidepoint(mouse), active=bool(state.selected_skill_id()))
    draw_inline_hint(screen, font_sm, node_card.x + 18, node_card.y + 100, "Category", ASSET_CATEGORY)
    draw_inline_hint(screen, font_sm, node_card.x + 18, node_card.y + 124, "Source", compact_path(current_tool_folder(state)) if current_tool_folder(state) else "No node folder yet")

    if state.node_dropdown_open:
        draw_dropdown_options(screen, font, node_dd, state.node_ids or [NO_NODES], "node_option", rects, disabled=not state.node_ids)

    y = node_card.bottom + 18
    if state.create_mode:
        create_card = pygame.Rect(panel.x + 18, y, panel.w - 36, 164)
        draw_create_panel(screen, font, font_sm, create_card, state, rects, mouse)
        y = create_card.bottom + 18

    selection_h = min(190, panel.bottom - y - 18)
    selection = pygame.Rect(panel.x + 18, y, panel.w - 36, selection_h)
    draw_card(screen, selection)
    screen.blit(font_mid.render("Current Selection", True, TEXT), (selection.x + 18, selection.y + 16))
    sy = selection.y + 56
    draw_status_line(screen, font, selection.x + 18, sy, "Skill", state.selected_skill_id())
    sy += 30
    draw_status_line(screen, font, selection.x + 18, sy, "Node", state.selected_node_id())
    sy += 30
    draw_status_line(screen, font, selection.x + 18, sy, "Available", "assigned" if available_tool_path(state.selected_skill_id(), state.selected_node_id()).exists() else "missing")
    sy += 30
    draw_status_line(screen, font, selection.x + 18, sy, "Depleted", "assigned" if depleted_tool_path(state.selected_skill_id(), state.selected_node_id()).exists() else "missing")

    refresh = pygame.Rect(selection.right - 148, selection.bottom - 48, 130, 36)
    rects["refresh"] = refresh
    draw_button(screen, font_sm, refresh, "Refresh", hovered=refresh.collidepoint(mouse), subtle=True)


def draw_create_panel(screen, font, font_sm, rect, state, rects, mouse):
    draw_card(screen, rect)
    is_skill = state.create_mode == "skill"
    title = "Create New Skill" if is_skill else f"Create Node under {state.selected_skill_id()}"
    screen.blit(font.render(title, True, TEXT), (rect.x + 18, rect.y + 16))
    label = "Skill ID" if is_skill else "Node ID"
    screen.blit(font_sm.render(label, True, MUTED), (rect.x + 18, rect.y + 54))
    inp = pygame.Rect(rect.x + 18, rect.y + 76, rect.w - 36, 42)
    rects["create_input"] = inp
    value = state.draft_skill_id if is_skill else state.draft_node_id
    active = state.editing_field == ("create_skill" if is_skill else "create_node")
    draw_input(screen, font, inp, value, active, "woodcutting" if is_skill else "oak_tree")
    confirm = pygame.Rect(rect.x + 18, rect.bottom - 52, 150, 38)
    cancel = pygame.Rect(confirm.right + 12, confirm.y, 120, 38)
    rects["confirm_create"] = confirm
    rects["cancel_create"] = cancel
    draw_button(screen, font_sm, confirm, "Create / Select", hovered=confirm.collidepoint(mouse), active=True)
    draw_button(screen, font_sm, cancel, "Cancel", hovered=cancel.collidepoint(mouse), subtle=True)


def draw_asset_column(screen, font, font_sm, font_mid, panel, state, rects, mouse):
    y = panel.y + 22
    screen.blit(font_mid.render("Asset Setup", True, TEXT), (panel.x + 22, y))
    y += 42
    skill = state.selected_skill_id()
    node = state.selected_node_id()
    available_path = available_tool_path(skill, node)
    depleted_path = depleted_tool_path(skill, node)

    slot_a = pygame.Rect(panel.x + 18, y, panel.w - 36, 144)
    draw_slot_card(screen, font, font_sm, slot_a, "Available Sprite", available_path, bool(skill and node and available_path.exists()), "Normal interactable node sprite.")
    assign_a = pygame.Rect(slot_a.right - 172, slot_a.bottom - 48, 150, 36)
    rects["assign_available"] = assign_a
    draw_button(screen, font_sm, assign_a, "Assign PNG", hovered=assign_a.collidepoint(mouse), active=bool(skill and node))

    y = slot_a.bottom + 16
    slot_d = pygame.Rect(panel.x + 18, y, panel.w - 36, 144)
    draw_slot_card(screen, font, font_sm, slot_d, "Depleted Sprite", depleted_path, bool(skill and node and depleted_path.exists()), "Stump/depleted sprite shown after charges hit zero.")
    assign_d = pygame.Rect(slot_d.right - 172, slot_d.bottom - 48, 150, 36)
    rects["assign_depleted"] = assign_d
    draw_button(screen, font_sm, assign_d, "Assign PNG", hovered=assign_d.collidepoint(mouse), active=bool(skill and node))

    y = slot_d.bottom + 18
    anchor_card = pygame.Rect(panel.x + 18, y, panel.w - 36, 106)
    draw_anchor_card(screen, font, font_sm, anchor_card, state, rects, mouse)

    y = anchor_card.bottom + 18
    export = pygame.Rect(panel.x + 18, y, panel.w - 36, 194)
    draw_card(screen, export)
    screen.blit(font_mid.render("Export", True, TEXT), (export.x + 20, export.y + 16))
    ready = bool(skill and node and available_path.exists() and depleted_path.exists())
    draw_chip(screen, font_sm, pygame.Rect(export.right - 140, export.y + 16, 104, 28), "ready" if ready else "incomplete", OK if ready else BAD)
    draw_inline_hint(screen, font_sm, export.x + 20, export.y + 58, "Game folder", compact_path(current_game_folder(state)) if current_game_folder(state) else "No export folder yet")
    draw_inline_hint(screen, font_sm, export.x + 20, export.y + 88, "Available", game_asset_rel_path(skill, node, "available") if skill and node else "world/skills/<skill>/harvest_nodes/<node>/available.png", WARN)
    draw_inline_hint(screen, font_sm, export.x + 20, export.y + 118, "Depleted", game_asset_rel_path(skill, node, "depleted") if skill and node else "world/skills/<skill>/harvest_nodes/<node>/depleted.png", WARN)
    send = pygame.Rect(export.x + 20, export.bottom - 54, 200, 38)
    open_tool = pygame.Rect(send.right + 12, send.y, 150, 38)
    open_game = pygame.Rect(open_tool.right + 12, send.y, 150, 38)
    rects["send_to_game"] = send
    rects["open_tool_folder"] = open_tool
    rects["open_game_folder"] = open_game
    draw_button(screen, font_sm, send, "Send To Game", hovered=send.collidepoint(mouse), active=ready)
    draw_button(screen, font_sm, open_tool, "Open Source", hovered=open_tool.collidepoint(mouse), subtle=True)
    draw_button(screen, font_sm, open_game, "Open Game", hovered=open_game.collidepoint(mouse), subtle=True)

    y = export.bottom + 18
    status = pygame.Rect(panel.x + 18, y, panel.w - 36, panel.bottom - y - 18)
    draw_card(screen, status)
    next_action = next_action_text(state)
    screen.blit(font_mid.render("Next Action", True, TEXT), (status.x + 20, status.y + 16))
    draw_wrapped_text(screen, font, next_action, status.x + 20, status.y + 50, status.w - 40, WARN if not ready else OK, line_h=21, max_lines=2)
    divider_y = status.y + 100
    pygame.draw.line(screen, BORDER, (status.x + 20, divider_y), (status.right - 20, divider_y), 1)
    screen.blit(font_mid.render("Status", True, TEXT), (status.x + 20, divider_y + 18))
    draw_wrapped_text(screen, font, state.status, status.x + 20, divider_y + 52, status.w - 40, OK if state.status_ok else BAD, line_h=21, max_lines=4)


def draw_preview_column(screen, font, font_sm, font_mid, panel, state, rects):
    y = panel.y + 22
    screen.blit(font_mid.render("Preview", True, TEXT), (panel.x + 22, y))
    y += 42
    card_w = panel.w - 36
    card_h = 330
    preview_a = pygame.Rect(panel.x + 18, y, card_w, card_h)
    draw_sprite_preview(screen, font, font_sm, preview_a, "Available", state.surfaces.get("available"), available_tool_path(state.selected_skill_id(), state.selected_node_id()), state, "available", rects=rects)
    y = preview_a.bottom + 18
    preview_d = pygame.Rect(panel.x + 18, y, card_w, card_h)
    draw_sprite_preview(screen, font, font_sm, preview_d, "Depleted", state.surfaces.get("depleted"), depleted_tool_path(state.selected_skill_id(), state.selected_node_id()), state, "depleted", rects=rects)
    y = preview_d.bottom + 18
    note = pygame.Rect(panel.x + 18, y, card_w, panel.bottom - y - 18)
    draw_card(screen, note)
    screen.blit(font_mid.render("Anchor Editing", True, TEXT), (note.x + 20, note.y + 16))
    draw_wrapped_text(screen, font_sm, "Click inside either preview image to set that sprite anchor. Use the bottom-center trunk/root point for trees and stumps.", note.x + 20, note.y + 58, note.w - 40, MUTED, line_h=18, max_lines=3)
    draw_inline_hint(screen, font_sm, note.x + 20, note.y + 128, "Open Source", "tool-owned working folder")
    draw_inline_hint(screen, font_sm, note.x + 20, note.y + 158, "Open Game", "export folder used by Bevy")


def draw_panel(screen, rect):
    pygame.draw.rect(screen, PANEL, rect, border_radius=16)
    pygame.draw.rect(screen, BORDER, rect, width=1, border_radius=16)


def draw_card(screen, rect):
    pygame.draw.rect(screen, CARD, rect, border_radius=14)
    pygame.draw.rect(screen, BORDER, rect, width=1, border_radius=14)


def draw_dropdown(screen, font, rect, value, open_, hovered=False, disabled=False):
    color = (31, 31, 44) if not disabled else (25, 25, 33)
    if hovered and not disabled:
        color = (39, 42, 62)
    pygame.draw.rect(screen, color, rect, border_radius=10)
    pygame.draw.rect(screen, BORDER_ACTIVE if open_ else BORDER, rect, width=1, border_radius=10)
    txt_color = DIM if disabled else TEXT
    screen.blit(font.render(value, True, txt_color), (rect.x + 12, rect.y + 11))
    arrow = "▲" if open_ else "▼"
    screen.blit(font.render(arrow, True, txt_color), (rect.right - 30, rect.y + 10))


def draw_dropdown_options(screen, font, anchor, items, key_prefix, rects, disabled=False):
    if disabled:
        return
    item_h = 38
    visible = items[:10]
    menu = pygame.Rect(anchor.x, anchor.bottom + 6, anchor.w, item_h * len(visible))
    pygame.draw.rect(screen, MENU_BG, menu, border_radius=10)
    pygame.draw.rect(screen, BORDER_ACTIVE, menu, width=1, border_radius=10)
    mouse = pygame.mouse.get_pos()
    for i, item in enumerate(visible):
        r = pygame.Rect(menu.x, menu.y + i * item_h, menu.w, item_h)
        rects[f"{key_prefix}_{i}"] = r
        if r.collidepoint(mouse):
            pygame.draw.rect(screen, MENU_HOVER, r)
        screen.blit(font.render(item, True, TEXT), (r.x + 12, r.y + 9))


def draw_input(screen, font, rect, value, active, placeholder):
    pygame.draw.rect(screen, INPUT_BG if not active else (37, 39, 58), rect, border_radius=10)
    pygame.draw.rect(screen, BORDER_ACTIVE if active else BORDER, rect, width=1, border_radius=10)
    shown = value + ("|" if active else "")
    text = shown if shown else placeholder
    screen.blit(font.render(text, True, TEXT if shown else MUTED), (rect.x + 12, rect.y + 11))


def draw_chip(screen, font, rect, label, color):
    pygame.draw.rect(screen, color, rect, border_radius=999)
    t = font.render(label, True, (18, 19, 26))
    screen.blit(t, t.get_rect(center=rect.center))


def draw_small_path(screen, font, x, y, label, value, value_color=TEXT):
    screen.blit(font.render(label, True, MUTED), (x, y))
    draw_wrapped_text(screen, font, value or "—", x, y + 21, 360, value_color, line_h=17, max_lines=2)


def draw_inline_hint(screen, font, x, y, label, value, value_color=TEXT):
    label_txt = font.render(f"{label}:", True, MUTED)
    screen.blit(label_txt, (x, y))
    draw_wrapped_text(screen, font, value or "—", x + 110, y, 410, value_color, line_h=17, max_lines=2)


def draw_status_line(screen, font, x, y, label, value):
    screen.blit(font.render(f"{label}:", True, MUTED), (x, y))
    shown = value or "none selected"
    color = TEXT if value else BAD
    screen.blit(font.render(shown, True, color), (x + 140, y))


def draw_slot_card(screen, font, font_sm, rect, title, path, exists, description):
    draw_card(screen, rect)
    screen.blit(font.render(title, True, TEXT), (rect.x + 20, rect.y + 18))
    draw_chip(screen, font_sm, pygame.Rect(rect.right - 126, rect.y + 16, 94, 28), "assigned" if exists else "missing", OK if exists else BAD)
    draw_wrapped_text(screen, font_sm, description, rect.x + 20, rect.y + 50, rect.w - 40, MUTED, line_h=18, max_lines=1)
    draw_wrapped_text(screen, font_sm, compact_path(path), rect.x + 20, rect.y + 82, rect.w - 220, DIM, line_h=17, max_lines=2)


def draw_anchor_card(screen, font, font_sm, rect, state, rects, mouse):
    draw_card(screen, rect)
    screen.blit(font.render("Sprite Anchors", True, TEXT), (rect.x + 20, rect.y + 16))
    draw_wrapped_text(
        screen,
        font_sm,
        "Click the preview image at the trunk/stump base. Anchors are saved in manifest.json and exported with the sprites.",
        rect.x + 20,
        rect.y + 48,
        rect.w - 210,
        MUTED,
        line_h=17,
        max_lines=2,
    )
    ax, ay = state.anchors.get("available", DEFAULT_ANCHOR)
    dx, dy = state.anchors.get("depleted", DEFAULT_ANCHOR)
    screen.blit(font_sm.render(f"Available: x={ax:.2f}, y={ay:.2f}", True, OK), (rect.right - 196, rect.y + 18))
    screen.blit(font_sm.render(f"Depleted:  x={dx:.2f}, y={dy:.2f}", True, OK), (rect.right - 196, rect.y + 42))
    btn = pygame.Rect(rect.right - 196, rect.bottom - 40, 176, 30)
    rects["copy_available_anchor"] = btn
    draw_button(screen, font_sm, btn, "Copy Avail -> Depl", hovered=btn.collidepoint(mouse), subtle=True)


def draw_sprite_preview(screen, font, font_sm, rect, title, surface, path, state, slot, rects):
    draw_card(screen, rect)
    screen.blit(font.render(title, True, TEXT), (rect.x + 20, rect.y + 18))
    draw_chip(screen, font_sm, pygame.Rect(rect.right - 126, rect.y + 16, 94, 28), "assigned" if surface else "missing", OK if surface else BAD)
    preview = pygame.Rect(rect.x + 24, rect.y + 58, rect.w - 48, rect.h - 118)
    pygame.draw.rect(screen, CARD_DARK, preview, border_radius=12)
    pygame.draw.rect(screen, BORDER, preview, width=1, border_radius=12)

    image_rect = preview
    if surface is None:
        t = font.render("Missing PNG", True, BAD)
        screen.blit(t, t.get_rect(center=preview.center))
    else:
        fitted = fit_surface(surface, preview.w - 28, preview.h - 28)
        image_rect = fitted.get_rect(center=preview.center)
        screen.blit(fitted, image_rect)
        if rects is not None:
            rects[f"{slot}_image_rect"] = image_rect
        draw_anchor_marker(screen, image_rect, state.anchors.get(slot, DEFAULT_ANCHOR))

    ax, ay = state.anchors.get(slot, DEFAULT_ANCHOR)
    anchor_label = f"anchor x={ax:.2f}, y={ay:.2f}"
    screen.blit(font_sm.render(anchor_label, True, OK), (rect.x + 20, rect.bottom - 64))
    draw_wrapped_text(screen, font_sm, compact_path(path), rect.x + 20, rect.bottom - 42, rect.w - 40, DIM, line_h=16, max_lines=2)


def draw_anchor_marker(screen, image_rect, anchor):
    ax, ay = anchor
    px = int(image_rect.x + ax * image_rect.w)
    py = int(image_rect.y + ay * image_rect.h)
    pygame.draw.circle(screen, (255, 226, 92), (px, py), 8, width=2)
    pygame.draw.line(screen, (255, 226, 92), (px - 14, py), (px + 14, py), 2)
    pygame.draw.line(screen, (255, 226, 92), (px, py - 14), (px, py + 14), 2)
    pygame.draw.circle(screen, (20, 20, 28), (px, py), 3)

def draw_wrapped_text(screen, font, text, x, y, width, color, line_h=22, max_lines=None):
    words = str(text).split()
    line = ""
    cy = y
    lines_drawn = 0
    for word in words:
        candidate = f"{line} {word}".strip()
        if font.size(candidate)[0] <= width:
            line = candidate
        else:
            if line:
                if max_lines is not None and lines_drawn >= max_lines:
                    return
                screen.blit(font.render(line, True, color), (x, cy))
                lines_drawn += 1
                cy += line_h
            line = word
    if line:
        if max_lines is None or lines_drawn < max_lines:
            screen.blit(font.render(line, True, color), (x, cy))


def fit_surface(surface, max_w, max_h):
    w, h = surface.get_size()
    if w <= 0 or h <= 0:
        return surface
    scale = min(max_w / w, max_h / h, 8.0)
    size = (max(1, int(w * scale)), max(1, int(h * scale)))
    return pygame.transform.scale(surface, size)


def set_anchor_from_preview_click(state, ui_rects, pos, slot):
    skill_id = state.selected_skill_id()
    node_id = state.selected_node_id()
    if not skill_id or not node_id:
        return False
    rect = ui_rects.get(f"{slot}_image_rect")
    if rect is None or not rect.collidepoint(pos):
        return False
    x = (pos[0] - rect.x) / max(1, rect.w)
    y = (pos[1] - rect.y) / max(1, rect.h)
    state.anchors[slot] = (clamp01(x), clamp01(y))
    write_manifest(skill_id, node_id, state)
    set_status(state, f"Set {slot} anchor to x={state.anchors[slot][0]:.2f}, y={state.anchors[slot][1]:.2f}.", True)
    return True


def copy_available_anchor_to_depleted(state):
    skill_id = state.selected_skill_id()
    node_id = state.selected_node_id()
    if not skill_id or not node_id:
        set_status(state, "Create/select a skill and node before copying anchors.", False)
        return
    state.anchors["depleted"] = state.anchors.get("available", DEFAULT_ANCHOR)
    write_manifest(skill_id, node_id, state)
    set_status(state, "Copied available anchor to depleted sprite.", True)


def clamp01(value):
    return max(0.0, min(1.0, float(value)))


def next_action_text(state):
    if not state.selected_skill_id():
        return "Create a skill, or select one from the Skill dropdown."
    if not state.selected_node_id():
        return "Create a node under the selected skill."
    if not available_tool_path(state.selected_skill_id(), state.selected_node_id()).exists():
        return "Assign the available sprite PNG."
    if not depleted_tool_path(state.selected_skill_id(), state.selected_node_id()).exists():
        return "Assign the depleted/stump sprite PNG."
    return "Both sprites are assigned. Check anchors in preview, then send to game/assets."


def close_create_panel(state):
    state.create_mode = None
    state.editing_field = None


def refresh_all(state, quiet=False):
    refresh_skills(state)
    refresh_nodes(state)
    reload_previews(state)
    if not quiet:
        set_status(state, "Refreshed skills and node folders.", True)


def refresh_skills(state):
    TOOL_SKILLS_ROOT.mkdir(parents=True, exist_ok=True)
    current = state.selected_skill_id()
    state.skill_ids = sorted([p.name for p in TOOL_SKILLS_ROOT.iterdir() if p.is_dir()], key=lambda s: s.lower())
    if current and current in state.skill_ids:
        state.selected_skill_index = state.skill_ids.index(current)
    elif state.skill_ids:
        state.selected_skill_index = min(state.selected_skill_index, len(state.skill_ids) - 1)
    else:
        state.selected_skill_index = 0


def refresh_nodes(state):
    skill_id = state.selected_skill_id()
    if not skill_id:
        state.node_ids = []
        state.selected_node_index = 0
        return
    root = skill_harvest_root(skill_id)
    root.mkdir(parents=True, exist_ok=True)
    current = state.selected_node_id()
    state.node_ids = sorted([p.name for p in root.iterdir() if p.is_dir()], key=lambda s: s.lower())
    if current and current in state.node_ids:
        state.selected_node_index = state.node_ids.index(current)
    elif state.node_ids:
        state.selected_node_index = min(state.selected_node_index, len(state.node_ids) - 1)
    else:
        state.selected_node_index = 0


def ensure_skill_from_draft(state):
    skill_id = sanitize_id(state.draft_skill_id)
    if not skill_id:
        set_status(state, "Skill id cannot be empty. Use lowercase letters, numbers, underscores, or hyphens.", False)
        return
    skill_harvest_root(skill_id).mkdir(parents=True, exist_ok=True)
    refresh_skills(state)
    if skill_id in state.skill_ids:
        state.selected_skill_index = state.skill_ids.index(skill_id)
    refresh_nodes(state)
    reload_previews(state)
    state.create_mode = None
    state.editing_field = None
    set_status(state, f"Created/selected skill {skill_id}.", True)


def ensure_node_from_draft(state):
    skill_id = state.selected_skill_id()
    if not skill_id:
        set_status(state, "Create or select a skill before creating a node.", False)
        return
    node_id = sanitize_id(state.draft_node_id)
    if not node_id:
        set_status(state, "Node id cannot be empty. Use lowercase letters, numbers, underscores, or hyphens.", False)
        return
    node_tool_dir(skill_id, node_id).mkdir(parents=True, exist_ok=True)
    write_manifest(skill_id, node_id, state)
    refresh_nodes(state)
    if node_id in state.node_ids:
        state.selected_node_index = state.node_ids.index(node_id)
    reload_previews(state)
    state.create_mode = None
    state.editing_field = None
    set_status(state, f"Created/selected {skill_id}/{node_id}.", True)


def assign_sprite(state, slot):
    skill_id = state.selected_skill_id()
    node_id = state.selected_node_id()
    if not skill_id or not node_id:
        set_status(state, "Create/select a skill and node before assigning sprites.", False)
        return
    src = pick_png_file()
    if src is None:
        set_status(state, "PNG assignment cancelled.", True)
        return
    dst = available_tool_path(skill_id, node_id) if slot == "available" else depleted_tool_path(skill_id, node_id)
    dst.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(src, dst)
    write_manifest(skill_id, node_id, state)
    reload_previews(state)
    set_status(state, f"Assigned {slot} sprite from {safe_rel(src)}.", True)


def send_to_game_assets(state):
    skill_id = state.selected_skill_id()
    node_id = state.selected_node_id()
    if not skill_id or not node_id:
        set_status(state, "Create/select a skill and node before exporting.", False)
        return
    src_available = available_tool_path(skill_id, node_id)
    src_depleted = depleted_tool_path(skill_id, node_id)
    missing = [p.name for p in [src_available, src_depleted] if not p.exists()]
    if missing:
        set_status(state, f"Cannot export yet. Missing: {', '.join(missing)}.", False)
        return
    game_dir = node_game_dir(skill_id, node_id)
    game_dir.mkdir(parents=True, exist_ok=True)
    shutil.copy2(src_available, game_dir / AVAILABLE_NAME)
    shutil.copy2(src_depleted, game_dir / DEPLETED_NAME)
    write_manifest(skill_id, node_id, state)
    shutil.copy2(node_tool_dir(skill_id, node_id) / MANIFEST_NAME, game_dir / MANIFEST_NAME)
    set_status(state, f"Copied {skill_id}/{node_id} sprites to game/assets.", True)


def open_folder(path, state):
    if path is None:
        set_status(state, "Create/select a skill and node before opening folders.", False)
        return
    path.mkdir(parents=True, exist_ok=True)
    try:
        if os.name == "nt":
            os.startfile(str(path))  # type: ignore[attr-defined]
        elif sys_platform_is_mac():
            subprocess.Popen(["open", str(path)])
        else:
            subprocess.Popen(["xdg-open", str(path)])
        set_status(state, f"Opened {safe_rel(path)}.", True)
    except Exception as e:
        set_status(state, f"Could not open folder: {e}", False)


def sys_platform_is_mac():
    import sys
    return sys.platform == "darwin"


def reload_previews(state):
    skill_id = state.selected_skill_id()
    node_id = state.selected_node_id()
    state.anchors = read_manifest_anchors(skill_id, node_id) if skill_id and node_id else {"available": DEFAULT_ANCHOR, "depleted": DEFAULT_ANCHOR}
    state.surfaces["available"] = load_surface(available_tool_path(skill_id, node_id)) if skill_id and node_id else None
    state.surfaces["depleted"] = load_surface(depleted_tool_path(skill_id, node_id)) if skill_id and node_id else None


def load_surface(path):
    if not path.exists():
        return None
    try:
        return pygame.image.load(str(path)).convert_alpha()
    except Exception:
        return None


def pick_png_file():
    if tk is None or filedialog is None:
        print("Tk file dialog is unavailable. Enter PNG path manually:")
        typed = input("> ").strip().strip('"')
        return Path(typed) if typed else None
    root = tk.Tk()
    root.withdraw()
    root.attributes("-topmost", True)
    selected = filedialog.askopenfilename(title="Select PNG sprite", filetypes=[("PNG files", "*.png"), ("All files", "*.*")])
    root.destroy()
    return Path(selected) if selected else None


def read_manifest_anchors(skill_id, node_id):
    out = {"available": DEFAULT_ANCHOR, "depleted": DEFAULT_ANCHOR}
    if not skill_id or not node_id:
        return out
    path = node_tool_dir(skill_id, node_id) / MANIFEST_NAME
    if not path.exists():
        return out
    try:
        data = json.loads(path.read_text(encoding="utf-8"))
        visuals = data.get("visuals", {})
        for slot in ("available", "depleted"):
            anchor = visuals.get(slot, {}).get("anchor", {})
            x = clamp01(anchor.get("x", out[slot][0]))
            y = clamp01(anchor.get("y", out[slot][1]))
            out[slot] = (x, y)
    except Exception:
        pass
    return out


def write_manifest(skill_id, node_id, state=None):
    node_dir = node_tool_dir(skill_id, node_id)
    node_dir.mkdir(parents=True, exist_ok=True)
    anchors = (state.anchors if state is not None else read_manifest_anchors(skill_id, node_id))
    data = {
        "skill_id": skill_id,
        "node_id": node_id,
        "asset_type": "harvest_node",
        "asset_category": ASSET_CATEGORY,
        "slots": ["available", "depleted"],
        "tool_paths": {
            "available": safe_rel(available_tool_path(skill_id, node_id)),
            "depleted": safe_rel(depleted_tool_path(skill_id, node_id)),
        },
        "game_asset_paths": {
            "available": game_asset_rel_path(skill_id, node_id, "available"),
            "depleted": game_asset_rel_path(skill_id, node_id, "depleted"),
        },
        "visuals": {
            "available": {
                "file": AVAILABLE_NAME,
                "anchor": {
                    "x": round(anchors.get("available", DEFAULT_ANCHOR)[0], 4),
                    "y": round(anchors.get("available", DEFAULT_ANCHOR)[1], 4),
                },
                "scale": 1.0,
            },
            "depleted": {
                "file": DEPLETED_NAME,
                "anchor": {
                    "x": round(anchors.get("depleted", DEFAULT_ANCHOR)[0], 4),
                    "y": round(anchors.get("depleted", DEFAULT_ANCHOR)[1], 4),
                },
                "scale": 1.0,
            },
        },
    }
    (node_dir / MANIFEST_NAME).write_text(json.dumps(data, indent=2) + "\n", encoding="utf-8")


def sanitize_id(value):
    cleaned = "".join(ch.lower() for ch in value.strip() if ch.isalnum() or ch in "_-")
    return cleaned.strip("_-")


def set_status(state, message, ok):
    state.status = message
    state.status_ok = ok


def skill_tool_dir(skill_id):
    return TOOL_SKILLS_ROOT / skill_id


def skill_harvest_root(skill_id):
    return skill_tool_dir(skill_id) / ASSET_CATEGORY


def node_tool_dir(skill_id, node_id):
    return skill_harvest_root(skill_id) / node_id


def node_game_dir(skill_id, node_id):
    return GAME_WORLD_SKILLS_ROOT / skill_id / ASSET_CATEGORY / node_id


def current_tool_folder(state):
    skill_id = state.selected_skill_id()
    node_id = state.selected_node_id()
    if not skill_id or not node_id:
        return None
    return node_tool_dir(skill_id, node_id)


def current_game_folder(state):
    skill_id = state.selected_skill_id()
    node_id = state.selected_node_id()
    if not skill_id or not node_id:
        return None
    return node_game_dir(skill_id, node_id)


def available_tool_path(skill_id, node_id):
    return node_tool_dir(skill_id, node_id) / AVAILABLE_NAME


def depleted_tool_path(skill_id, node_id):
    return node_tool_dir(skill_id, node_id) / DEPLETED_NAME


def game_asset_rel_path(skill_id, node_id, slot):
    filename = AVAILABLE_NAME if slot == "available" else DEPLETED_NAME
    return f"world/skills/{skill_id}/{ASSET_CATEGORY}/{node_id}/{filename}"


def safe_rel(path):
    try:
        return path.relative_to(PROJECT_ROOT).as_posix()
    except Exception:
        return path.as_posix()


def compact_path(path):
    if path is None:
        return "None yet"
    s = safe_rel(path)
    parts = s.split("/")
    if len(parts) <= 5:
        return s
    return ".../" + "/".join(parts[-4:])


if __name__ == "__main__":
    main()
