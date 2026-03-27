from __future__ import annotations

import re
from dataclasses import dataclass
from pathlib import Path
from typing import Dict, List, Tuple, Optional, Any, Iterable

import pygame

from .config import (
    BASE_DIR,
    DIRECTIONS,
    EXPECTED_FRAMES_PER_DIR,
    MANAGER_SCALE,
    UI_BG,
    UI_TOP,
    UI_PANEL,
    UI_CARD,
    UI_CARD_HOVER,
    UI_BORDER,
    UI_TEXT,
    UI_MUTED,
    UI_OK,
    UI_BAD,
    UI_WARN,
    MANAGER_ROW_H,
    MANAGER_PAD,
    MANAGER_LIST_TOP,
    MANAGER_LIST_BOTTOM_PAD,
    DETAIL_H,
    DETAIL_GAP,
)

from .manager_scan import ActionStatus, discover_all_actions_for_manager

from .ui_common import (
    Dropdown,
    draw_round_rect,
    draw_button,
    draw_pill,
    draw_icon_folder,
    draw_icon_chevron,
    draw_icon_check,
    draw_icon_x,
    draw_search_box,
    clamp,
    draw_dropdown_header,
    draw_dropdown_menu,
    dropdown_menu_rect,
)

FRAME_NAME_SAN = re.compile(r"[^a-zA-Z0-9_]+")


def sanitize_action_name(raw: str) -> str:
    s = raw.strip()
    s = s.replace(" ", "_")
    s = FRAME_NAME_SAN.sub("", s)
    s = re.sub(r"_+", "_", s)
    s = s.strip("_")
    return s.lower()


def create_action_structure(group: str, name: str) -> Path:
    if group not in ("base", "skills", "combat"):
        raise ValueError(f"Unknown group: {group}")

    safe = sanitize_action_name(name)
    if not safe:
        raise ValueError("Action name is empty after sanitization.")

    action_dir = BASE_DIR / safe if group == "base" else (BASE_DIR / group / safe)
    for d in DIRECTIONS:
        (action_dir / d).mkdir(parents=True, exist_ok=True)
    return action_dir


@dataclass
class ActionCreateModal:
    open: bool = False
    group: str = "skills"
    raw_name: str = ""
    error: str = ""

    def reset(self):
        self.open = False
        self.group = "skills"
        self.raw_name = ""
        self.error = ""


@dataclass
class ManagerState:
    scroll: int = 0
    expanded: Dict[Tuple[str, str], bool] = None
    cache: List[ActionStatus] = None
    needs_refresh: bool = True

    filter_text: str = ""
    focus_search: bool = False
    incomplete_first: bool = True

    layout_rows: List[Tuple[ActionStatus, pygame.Rect]] = None
    total_content_h: int = 0

    modal: ActionCreateModal = None

    # dropdown focus (manager owns its own)
    ui_menu_focus: Optional[str] = None

    def __post_init__(self):
        if self.expanded is None:
            self.expanded = {}
        if self.cache is None:
            self.cache = []
        if self.layout_rows is None:
            self.layout_rows = []
        if self.modal is None:
            self.modal = ActionCreateModal()


def refresh_manager_cache(ms: ManagerState) -> None:
    ms.cache = discover_all_actions_for_manager(BASE_DIR, expected=EXPECTED_FRAMES_PER_DIR)
    ms.needs_refresh = False


def build_display_list(ms: ManagerState) -> List[ActionStatus]:
    if ms.needs_refresh:
        refresh_manager_cache(ms)

    q = ms.filter_text.strip().lower()
    display = ms.cache
    if q:
        display = [st for st in ms.cache if q in st.name.lower() or q in st.rel_path.as_posix().lower()]

    # order
    if ms.incomplete_first:
        display = sorted(display, key=lambda st: (st.overall_complete, st.group, st.name.lower()))
    else:
        display = sorted(display, key=lambda st: (not st.overall_complete, st.group, st.name.lower()))
    return display


def compute_layout(
    screen: pygame.Surface,
    ms: ManagerState,
    display_list: List[ActionStatus],
) -> Tuple[List[Tuple[ActionStatus, pygame.Rect]], int, int]:
    list_rect = pygame.Rect(
        MANAGER_PAD,
        MANAGER_LIST_TOP,
        screen.get_width() - MANAGER_PAD * 2,
        screen.get_height() - MANAGER_LIST_TOP - MANAGER_LIST_BOTTOM_PAD,
    )

    header_h = 44
    y = list_rect.y + header_h
    total = 0
    rows: List[Tuple[ActionStatus, pygame.Rect]] = []

    for st in display_list:
        row_h = MANAGER_ROW_H
        row = pygame.Rect(list_rect.x + 12, y, list_rect.w - 24, row_h - 6)
        rows.append((st, row))

        total += row_h
        y += row_h

        if ms.expanded.get((st.group, st.name), False):
            total += DETAIL_H + DETAIL_GAP
            y += DETAIL_H + DETAIL_GAP

    view_h = list_rect.h - header_h
    return rows, total, view_h


def clamp_scroll(screen: pygame.Surface, ms: ManagerState, display_list: List[ActionStatus]) -> None:
    rows, total_h, view_h = compute_layout(screen, ms, display_list)
    ms.layout_rows = rows
    ms.total_content_h = total_h
    max_scroll = max(0, total_h - view_h)
    ms.scroll = int(clamp(ms.scroll, 0, max_scroll))


def render_manager(
    screen: pygame.Surface,
    ms: ManagerState,
    mouse_pos: Tuple[int, int],
    *,
    mode_dd: Dropdown,
    # Backwards-compat: old app.py may pass these. Ignore safely.
    font: Optional[pygame.font.Font] = None,
    font_ui: Optional[pygame.font.Font] = None,
    **_ignored: Any,
) -> Dict[str, pygame.Rect]:
    mw, mh = screen.get_width(), screen.get_height()
    rw, rh = int(mw * MANAGER_SCALE), int(mh * MANAGER_SCALE)
    msurf = pygame.Surface((rw, rh), pygame.SRCALPHA)
    msurf.fill(UI_BG)

    def S(v: int) -> int:
        return int(v * MANAGER_SCALE)

    m_font_ui = pygame.font.SysFont("Segoe UI", max(16, int(20 * MANAGER_SCALE)))
    m_font_ui_big = pygame.font.SysFont("Segoe UI Semibold", max(18, int(26 * MANAGER_SCALE)))
    m_font_ui_huge = pygame.font.SysFont("Segoe UI Semibold", max(22, int(34 * MANAGER_SCALE)))

    # top bar
    top_h = 70
    top = pygame.Rect(0, 0, rw, S(top_h))
    draw_round_rect(msurf, top, UI_TOP, radius=0)
    pygame.draw.line(msurf, UI_BORDER, (0, top.bottom), (rw, top.bottom), 1)

    title = m_font_ui_huge.render("Asset Manager", True, UI_TEXT)
    msurf.blit(title, (S(MANAGER_PAD), S(12)))

    subtitle = m_font_ui.render("Completeness by action / direction (expects _01.._04).", True, UI_MUTED)
    msurf.blit(subtitle, (S(MANAGER_PAD), S(44)))

    # Mode dropdown (top-right) — match viewer header placement (screen coords; scaled onto msurf)
    mode_w, mode_h = 170, 40
    mode_x, mode_y = mw - mode_w - 18, 16
    draw_dropdown_header(msurf, m_font_ui, mode_dd, x=S(mode_x), y=S(mode_y), w=S(mode_w), h=S(mode_h))

    # search + sort (screen coords; scaled)
    tab_y = 16
    search_rect = pygame.Rect(MANAGER_PAD, tab_y, 420, 40)
    sort_rect = pygame.Rect(search_rect.right + 10, tab_y, 210, 40)

    draw_search_box(
        msurf, m_font_ui,
        pygame.Rect(S(search_rect.x), S(search_rect.y), S(search_rect.w), S(search_rect.h)),
        ms.filter_text,
        focused=ms.focus_search and (not ms.modal.open),
    )

    sort_label = "Incomplete First" if ms.incomplete_first else "Complete First"
    draw_button(
        msurf, m_font_ui,
        pygame.Rect(S(sort_rect.x), S(sort_rect.y), S(sort_rect.w), S(sort_rect.h)),
        sort_label,
        hovered=sort_rect.collidepoint(mouse_pos),
        subtle=True,
    )

    # list panel
    list_rect = pygame.Rect(
        MANAGER_PAD,
        MANAGER_LIST_TOP,
        mw - MANAGER_PAD * 2,
        mh - MANAGER_LIST_TOP - MANAGER_LIST_BOTTOM_PAD,
    )
    list_rect_s = pygame.Rect(S(list_rect.x), S(list_rect.y), S(list_rect.w), S(list_rect.h))
    draw_round_rect(msurf, list_rect_s, UI_PANEL, radius=18)
    pygame.draw.rect(msurf, UI_BORDER, list_rect_s, width=1, border_radius=18)

    # headers
    header_y = list_rect.y + 12
    msurf.blit(m_font_ui.render("Action", True, UI_MUTED), (S(list_rect.x + 18), S(header_y)))
    msurf.blit(m_font_ui.render("Group", True, UI_MUTED), (S(list_rect.x + 340), S(header_y)))
    msurf.blit(m_font_ui.render("Progress", True, UI_MUTED), (S(list_rect.x + 455), S(header_y)))
    msurf.blit(m_font_ui.render("N / E / S / W", True, UI_MUTED), (S(list_rect.x + 565), S(header_y)))

    display_list = build_display_list(ms)
    clamp_scroll(screen, ms, display_list)

    for st, row_base in ms.layout_rows:
        row = row_base.move(0, -ms.scroll)

        if row.bottom < list_rect.y + 34:
            continue
        if row.top > list_rect.bottom - 12:
            break

        hovered = row.collidepoint(mouse_pos)
        key = (st.group, st.name)
        expanded = ms.expanded.get(key, False)

        row_s = pygame.Rect(S(row.x), S(row.y), S(row.w), S(row.h))
        draw_round_rect(msurf, row_s, UI_CARD_HOVER if hovered else UI_CARD, radius=16)
        pygame.draw.rect(msurf, UI_BORDER, row_s, width=1, border_radius=16)

        folder_rect = pygame.Rect(row.x + 16, row.y + 18, 20, 18)
        draw_icon_folder(
            msurf,
            pygame.Rect(S(folder_rect.x), S(folder_rect.y), S(folder_rect.w), S(folder_rect.h)),
            UI_MUTED,
        )

        name_txt = m_font_ui_big.render(st.name, True, UI_TEXT)
        msurf.blit(name_txt, (S(row.x + 46), S(row.y + 10)))

        rel_txt = m_font_ui.render(st.rel_path.as_posix(), True, UI_MUTED)
        msurf.blit(rel_txt, (S(row.x + 46), S(row.y + 34)))

        pill = pygame.Rect(row.x + 340, row.y + 16, 92, 30)
        group_color = (130, 190, 255) if st.group == "base" else (255, 200, 120) if st.group == "skills" else (210, 150, 255)
        draw_pill(
            msurf,
            m_font_ui,
            pygame.Rect(S(pill.x), S(pill.y), S(pill.w), S(pill.h)),
            st.group,
            color=group_color,
        )

        present, expected = st.overall_progress
        prog_col = UI_OK if st.overall_complete else UI_WARN if present > 0 else UI_BAD
        prog = m_font_ui.render(f"{present}/{expected}", True, prog_col)
        msurf.blit(prog, (S(row.x + 460), S(row.y + 20)))

        dx = row.x + 565
        for d in DIRECTIONS:
            ds = st.dir_status[d]
            col = UI_OK if ds.complete else UI_WARN if ds.present_count > 0 else UI_BAD
            t = m_font_ui.render(f"{d[0].upper()} {ds.label}", True, col)
            msurf.blit(t, (S(dx), S(row.y + 20)))
            dx += 95

        chev_center = (row.right - 22, row.y + row.h // 2 + 1)
        draw_icon_chevron(msurf, (S(chev_center[0]), S(chev_center[1])), S(18), UI_MUTED, down=expanded)

        if expanded:
            detail = pygame.Rect(row.x, row.y + MANAGER_ROW_H, row.w, DETAIL_H)
            detail_s = pygame.Rect(S(detail.x), S(detail.y), S(detail.w), S(detail.h))
            draw_round_rect(msurf, detail_s, (28, 28, 40), radius=16)
            pygame.draw.rect(msurf, UI_BORDER, detail_s, width=1, border_radius=16)

            px = detail.x + 18
            py = detail.y + 14
            icon_size = S(16)

            for d in DIRECTIONS:
                ds = st.dir_status[d]
                head = m_font_ui.render(d.upper(), True, UI_TEXT)
                msurf.blit(head, (S(px), S(py)))

                sx = px
                sy = py + 32
                for slot in range(1, ds.expected + 1):
                    ok = slot in ds.present_slots
                    col = UI_OK if ok else UI_BAD
                    icon_center = (sx + 10, sy + 10)
                    if ok:
                        draw_icon_check(msurf, (S(icon_center[0]), S(icon_center[1])), icon_size, col)
                    else:
                        draw_icon_x(msurf, (S(icon_center[0]), S(icon_center[1])), icon_size, col)

                    lab = m_font_ui.render(f"{slot:02d}", True, UI_TEXT)
                    msurf.blit(lab, (S(sx + 26), S(sy)))
                    sy += 26

                px += 170

    # dropdown menus last
    menus = {"mode": mode_dd}
    for key, dd in menus.items():
        if dd.open and key != ms.ui_menu_focus:
            draw_dropdown_menu(msurf, m_font_ui, dd)
    if ms.ui_menu_focus in menus and menus[ms.ui_menu_focus].open:
        draw_dropdown_menu(msurf, m_font_ui, menus[ms.ui_menu_focus])

    screen.blit(pygame.transform.smoothscale(msurf, (mw, mh)), (0, 0))
    return {}


def _extract_from_any(args: Iterable[Any], kwargs: Dict[str, Any]) -> Tuple[Optional[ManagerState], Optional[Dropdown]]:
    """
    Pull ms + mode_dd from:
      - kwargs: ms / manager_state / state
      - args: first ManagerState instance, first Dropdown instance
    """
    ms: Optional[ManagerState] = None
    mode_dd: Optional[Dropdown] = None

    for k in ("ms", "manager_state", "state"):
        v = kwargs.get(k)
        if isinstance(v, ManagerState):
            ms = v
            break

    v = kwargs.get("mode_dd")
    if isinstance(v, Dropdown):
        mode_dd = v

    if ms is None:
        for a in args:
            if isinstance(a, ManagerState):
                ms = a
                break

    if mode_dd is None:
        for a in args:
            if isinstance(a, Dropdown):
                mode_dd = a
                break

    return ms, mode_dd


def handle_manager_click(
    pos: Tuple[int, int],
    *args: Any,
    **kwargs: Any,
) -> Tuple[Optional[Tuple[str, object]], Optional[str]]:
    """
    DEFENSIVE SIGNATURE:
    Accepts ALL old calling styles and ignores extras, so app.py can't break it.

    Supported:
      handle_manager_click(pos, ms=..., mode_dd=...)
      handle_manager_click(pos, state=..., mode_dd=...)
      handle_manager_click(pos, manager_state, mode_dd)  # positional legacy
      handle_manager_click(pos, ui_rects=..., ui_menu_focus=..., model_dd=..., etc)  # ignored
    """
    ms, mode_dd = _extract_from_any(args, kwargs)
    if ms is None or mode_dd is None:
        # Can't act; just don't crash.
        return None, None

    # Only thing we really care about for dropdown interactions:
    menus = {"mode": mode_dd}

    any_open = any(dd.open for dd in menus.values())
    if any_open:
        focus = ms.ui_menu_focus
        if focus not in menus or not menus[focus].open:
            focus = next((k for k, dd in menus.items() if dd.open), None)

        if focus:
            dd = menus[focus]

            if dd.rect.collidepoint(pos):
                dd.open = False
                return None, None

            menu = dropdown_menu_rect(dd)
            if menu.w > 0 and menu.collidepoint(pos):
                item_h = dd.rect.h
                max_items = min(len(dd.items), 16)
                i = (pos[1] - menu.y) // item_h
                if 0 <= i < max_items:
                    dd.selected_index = int(i)
                    for other in menus.values():
                        other.open = False
                    return ("mode_changed", None), None

            for other in menus.values():
                other.open = False
            return None, None

        for other in menus.values():
            other.open = False
        return None, None

    # open dropdown
    if mode_dd.rect.collidepoint(pos):
        mode_dd.open = True
        ms.ui_menu_focus = "mode"
        return None, "mode"

    # row expand/collapse (use ms.layout_rows computed during render)
    for st, row_base in ms.layout_rows:
        row = row_base.move(0, -ms.scroll)
        if row.collidepoint(pos):
            key = (st.group, st.name)
            ms.expanded[key] = not ms.expanded.get(key, False)
            return None, None

    return None, None