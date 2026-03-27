from __future__ import annotations

import shutil
import time
import json
from dataclasses import dataclass
from pathlib import Path
from typing import Dict, List, Optional, Tuple

import pygame

from .config import UI_BG, UI_TEXT, UI_WARN, UI_MUTED, DIRECTIONS
from .ui_common import Dropdown, draw_dropdown_header, draw_dropdown_menu
from .palettes import Palette, load_palette_json, iter_palette_files, ensure_baked_for_action
from .actions import ActionEntry, ActionGroups

THIS_FILE = Path(__file__).resolve()
VIEWER_DIR = THIS_FILE.parent
TOOLS_DIR = VIEWER_DIR.parent
PROJECT_ROOT = TOOLS_DIR.parent

TOOLS_TEMPLATE_ROOT = PROJECT_ROOT / "libs" / "templates" / "tools"
TOOLS_MANIFEST_DIR = TOOLS_TEMPLATE_ROOT / "manifests"
TOOLS_GENERATED_RUNTIME_ROOT = PROJECT_ROOT / "libs" / "generated_runtime" / "tools"


# ----------------------------
# Character scan helpers
# ----------------------------

def _list_pngs(folder: Path) -> List[Path]:
    if not folder.exists() or not folder.is_dir():
        return []
    return sorted([p for p in folder.iterdir() if p.is_file() and p.suffix.lower() == ".png"])


def _base_frame_map(base_dir: Path, action_rel: Path) -> Dict[str, List[str]]:
    out: Dict[str, List[str]] = {}
    action_dir = base_dir / action_rel
    for d in DIRECTIONS:
        frames = _list_pngs(action_dir / d)
        if frames:
            out[d] = [p.name for p in frames]
    return out


def _palette_is_baked_for_action(
    *,
    base_dir: Path,
    generated_dir: Path,
    action_rel: Path,
    palette_name: str,
) -> bool:
    base_map = _base_frame_map(base_dir, action_rel)
    if not base_map:
        return False

    for d, names in base_map.items():
        out_dir = generated_dir / action_rel / palette_name / d
        for nm in names:
            if not (out_dir / nm).exists():
                return False
    return True


def _discover_palettes(palettes_dir: Path) -> List[Palette]:
    pals: List[Palette] = []
    if palettes_dir.exists():
        for pf in list(iter_palette_files(palettes_dir)):
            try:
                pals.append(load_palette_json(pf))
            except Exception:
                pass
    return pals


# ----------------------------
# Tool helpers (manifest + leaf palettes + generated_runtime)
# ----------------------------

def _sanitize_id(s: str) -> str:
    out = (s or "").strip().lower().replace(" ", "_")
    out = "".join(c for c in out if (c.isalnum() or c == "_"))
    while "__" in out:
        out = out.replace("__", "_")
    return out.strip("_")


def _safe_clip_key(action_rel_path: str) -> str:
    s = (action_rel_path or "").replace("\\", "/").strip()
    if not s:
        return ""
    parts: List[str] = []
    for p in s.split("/"):
        p = p.strip()
        if not p or p in (".", ".."):
            continue
        parts.append(p)
    return "/".join(parts)


def _clip_leaf(clip_key: str) -> str:
    ck = _safe_clip_key(clip_key)
    if not ck:
        return ""
    return ck.split("/")[-1].strip()


def _clip_dir(clip_key: str) -> Path:
    ck = _safe_clip_key(clip_key)
    if not ck:
        return Path("_unknown")
    return Path(*ck.split("/"))


def _manifest_path_for_kind(tool_kind: str) -> Path:
    return TOOLS_MANIFEST_DIR / f"{_sanitize_id(tool_kind) or 'axe'}.json"


def _load_manifest(tool_kind: str) -> dict:
    p = _manifest_path_for_kind(tool_kind)
    if not p.exists():
        return {}
    try:
        with open(p, "r", encoding="utf-8") as f:
            return json.load(f) or {}
    except Exception:
        return {}


def _tool_template_dir_for(
    *,
    tool_kind: str,
    tool_id: str,
    clip_key: str,
    direction: str,
) -> Path:
    return (
        TOOLS_TEMPLATE_ROOT
        / _sanitize_id(tool_kind)
        / _sanitize_id(tool_id)
        / _clip_dir(clip_key)
        / direction
    )


def _tool_generated_dir_for(
    *,
    tool_kind: str,
    tool_id: str,
    palette: str,
    clip_key: str,
    direction: str,
) -> Path:
    return (
        TOOLS_GENERATED_RUNTIME_ROOT
        / _sanitize_id(tool_kind)
        / _sanitize_id(tool_id)
        / (palette or "").strip()
        / _clip_dir(clip_key)
        / direction
    )


def _tool_palette_dir_for(
    *,
    tool_kind: str,
    tool_id: str,
    clip_key: str,
) -> Path:
    # libs/palettes/<clip_leaf>/<kind>/<tool_id>/
    leaf = _sanitize_id(_clip_leaf(clip_key))
    return PROJECT_ROOT / "libs" / "palettes" / leaf / _sanitize_id(tool_kind) / _sanitize_id(tool_id)


def _tool_palettes_for(*, tool_kind: str, tool_id: str, clip_key: str) -> List[Palette]:
    pals_dir = _tool_palette_dir_for(tool_kind=tool_kind, tool_id=tool_id, clip_key=clip_key)
    pals: List[Palette] = []
    if pals_dir.exists():
        for pf in list(iter_palette_files(pals_dir)):
            try:
                pals.append(load_palette_json(pf))
            except Exception:
                pass

    # de-dupe by name
    uniq: Dict[str, Palette] = {}
    for p in pals:
        k = (p.name or "").strip().lower()
        if k and k not in uniq:
            uniq[k] = p
    return list(uniq.values())


def _tool_all_template_frames_exist(*, tool_kind: str, tool_id: str, clip_key: str) -> bool:
    for d in DIRECTIONS:
        src_dir = _tool_template_dir_for(tool_kind=tool_kind, tool_id=tool_id, clip_key=clip_key, direction=d)
        if src_dir.exists() and any(p.suffix.lower() == ".png" for p in src_dir.glob("*.png")):
            return True
    return False


def _tool_is_baked_for_palette(
    *,
    tool_kind: str,
    tool_id: str,
    clip_key: str,
    palette: str,
) -> bool:
    pal = (palette or "").strip()
    if not pal:
        return False

    for d in DIRECTIONS:
        src_dir = _tool_template_dir_for(tool_kind=tool_kind, tool_id=tool_id, clip_key=clip_key, direction=d)
        if not src_dir.exists():
            continue
        src_frames = sorted([p for p in src_dir.glob("*.png") if p.is_file()])
        if not src_frames:
            continue

        out_dir = _tool_generated_dir_for(tool_kind=tool_kind, tool_id=tool_id, palette=pal, clip_key=clip_key, direction=d)
        for sf in src_frames:
            if not (out_dir / sf.name).exists():
                return False

    return True


def _apply_palette_to_surface(src: pygame.Surface, pal: Palette) -> pygame.Surface:
    # Palette uses .mapping (rgb->rgb)
    rgb_map = dict(getattr(pal, "mapping", {}) or {})
    if not rgb_map:
        return src.copy().convert_alpha()

    out = src.copy().convert_alpha()
    w, h = out.get_size()
    out.lock()
    try:
        for y in range(h):
            for x in range(w):
                r, g, b, a = out.get_at((x, y))
                if a == 0:
                    continue
                dst = rgb_map.get((r, g, b))
                if dst is not None:
                    nr, ng, nb = dst
                    out.set_at((x, y), (nr, ng, nb, a))
    finally:
        out.unlock()
    return out


def bake_tool_variant_clip(
    *,
    tool_kind: str,
    tool_id: str,
    clip_key: str,
    palettes: List[Palette],
    force: bool,
    bake_missing_only: bool,
) -> Tuple[int, int]:
    """
    Writes to:
      libs/generated_runtime/tools/<kind>/<tool_id>/<palette>/<clip>/<dir>/<frame>.png

    Returns (baked_palettes, total_palettes)
    """
    if not _tool_all_template_frames_exist(tool_kind=tool_kind, tool_id=tool_id, clip_key=clip_key):
        return 0, len(palettes)

    baked = 0
    total = 0

    for pal in palettes:
        pal_name = (pal.name or "").strip()
        if not pal_name:
            continue
        total += 1

        if bake_missing_only and not force:
            if _tool_is_baked_for_palette(tool_kind=tool_kind, tool_id=tool_id, clip_key=clip_key, palette=pal_name):
                continue

        for d in DIRECTIONS:
            src_dir = _tool_template_dir_for(tool_kind=tool_kind, tool_id=tool_id, clip_key=clip_key, direction=d)
            if not src_dir.exists():
                continue
            frames = sorted([p for p in src_dir.glob("*.png") if p.is_file()])
            if not frames:
                continue

            dst_dir = _tool_generated_dir_for(
                tool_kind=tool_kind,
                tool_id=tool_id,
                palette=pal_name,
                clip_key=clip_key,
                direction=d,
            )
            dst_dir.mkdir(parents=True, exist_ok=True)

            for sf in frames:
                df = dst_dir / sf.name
                if bake_missing_only and df.exists() and not force:
                    continue
                img = pygame.image.load(str(sf)).convert_alpha()
                out = _apply_palette_to_surface(img, pal)
                pygame.image.save(out, str(df))

        baked += 1

    return baked, total


def _clean_tool_runtime_for_variant_clip(*, tool_kind: str, tool_id: str, clip_key: str) -> int:
    """
    Deletes baked palettes under:
      libs/generated_runtime/tools/<kind>/<tool_id>/<palette>/<clip>/

    Returns number of deleted palette folders.
    """
    root = TOOLS_GENERATED_RUNTIME_ROOT / _sanitize_id(tool_kind) / _sanitize_id(tool_id)
    if not root.exists():
        return 0

    clip_dir = _clip_dir(clip_key)
    deleted = 0
    for pal_dir in root.iterdir():
        if not pal_dir.is_dir():
            continue
        target = pal_dir / clip_dir
        if target.exists() and target.is_dir():
            try:
                shutil.rmtree(target)
                deleted += 1
            except Exception:
                pass
    return deleted


def _manifest_tool_variants_for_clip(*, tool_kind: str, clip_key: str) -> List[str]:
    data = _load_manifest(tool_kind)
    tools = data.get("tools") if isinstance(data, dict) else None
    if not isinstance(tools, dict):
        return []
    out: List[str] = []
    for tool_id, tool_obj in tools.items():
        if not isinstance(tool_id, str) or not isinstance(tool_obj, dict):
            continue
        clips = tool_obj.get("clips")
        if not isinstance(clips, dict):
            continue
        if clip_key in clips:
            out.append(tool_id)
    out.sort(key=lambda s: s.lower())
    return out


# ----------------------------
# State
# ----------------------------

@dataclass
class BakeRow:
    action_label: str
    action_rel: Path
    baked: int
    total: int


@dataclass
class ToolBakeRow:
    clip_label: str
    clip_rel: Path
    baked: int
    total: int


class BakeState:
    def __init__(self):
        self.scroll: int = 0
        self.rows: List[BakeRow] = []
        self.tool_rows: List[ToolBakeRow] = []
        self.last_msg: str = ""
        self.last_msg_ts: float = 0.0
        self.last_scan_ts: float = 0.0

    def refresh(self) -> None:
        self.scroll = max(0, self.scroll)

    def _msg(self, s: str) -> None:
        self.last_msg = s
        self.last_msg_ts = time.time()


# ----------------------------
# UI
# ----------------------------

def _draw_btn(screen: pygame.Surface, font: pygame.font.Font, r: pygame.Rect, label: str, *, hovered: bool) -> None:
    bg = (65, 65, 90) if hovered else (52, 52, 74)
    border = (95, 95, 125)
    pygame.draw.rect(screen, bg, r, border_radius=10)
    pygame.draw.rect(screen, border, r, width=1, border_radius=10)
    t = font.render(label, True, (235, 235, 245))
    screen.blit(t, t.get_rect(center=r.center))


def _draw_row_bar(screen: pygame.Surface, r: pygame.Rect, *, baked: int, total: int) -> None:
    pygame.draw.rect(screen, (40, 40, 56), r, border_radius=8)
    pygame.draw.rect(screen, (75, 75, 100), r, width=1, border_radius=8)
    if total <= 0:
        return
    frac = max(0.0, min(1.0, baked / float(total)))
    fill = pygame.Rect(r.x, r.y, int(r.w * frac), r.h)
    pygame.draw.rect(screen, (90, 160, 120), fill, border_radius=8)


def scan_bake_rows(
    *,
    state: BakeState,
    base_dir: Path,
    generated_dir: Path,
    palettes_dir: Path,
    actions: List[ActionEntry],
) -> Tuple[List[BakeRow], List[Palette]]:
    pals = _discover_palettes(palettes_dir)
    total = len(pals)

    rows: List[BakeRow] = []
    for a in actions:
        baked = 0
        for pal in pals:
            if _palette_is_baked_for_action(
                base_dir=base_dir,
                generated_dir=generated_dir,
                action_rel=a.rel_path,
                palette_name=pal.name,
            ):
                baked += 1
        rows.append(BakeRow(action_label=a.label, action_rel=a.rel_path, baked=baked, total=total))

    state.last_scan_ts = time.time()
    state._msg(f"Scanned {len(actions)} actions, {len(pals)} palettes")
    return rows, pals


def scan_tool_bake_rows(
    *,
    state: BakeState,
    project_root: Path,
    clip_rels: List[Path],
    tool_kind: str,
) -> Tuple[List[ToolBakeRow], List[Palette]]:
    """
    Tool scan rows are per (tool_id + clip).
    Uses manifest to know which tool variants apply to each clip.
    """
    rows: List[ToolBakeRow] = []

    for clip_rel in clip_rels:
        clip_key = _safe_clip_key(clip_rel.as_posix())
        if not clip_key:
            continue

        tool_ids = _manifest_tool_variants_for_clip(tool_kind=tool_kind, clip_key=clip_key)
        if not tool_ids:
            continue

        for tool_id in tool_ids:
            pals = _tool_palettes_for(tool_kind=tool_kind, tool_id=tool_id, clip_key=clip_key)
            total = len(pals)
            baked = 0
            for pal in pals:
                if _tool_is_baked_for_palette(tool_kind=tool_kind, tool_id=tool_id, clip_key=clip_key, palette=pal.name):
                    baked += 1

            rows.append(
                ToolBakeRow(
                    clip_label=f"{tool_kind}/{tool_id} | {clip_key}",
                    clip_rel=clip_rel,
                    baked=baked,
                    total=total,
                )
            )

    state._msg(f"Scanned tools: {len(rows)} variant-clip rows")
    return rows, []


def render_bake_mode(
    *,
    screen: pygame.Surface,
    font: pygame.font.Font,
    font_ui: pygame.font.Font,
    mode_dd: Dropdown,
    model_dd: Dropdown,
    group_dd: Dropdown,
    action_dd: Dropdown,
    bake_scope_dd: Dropdown,
    bake_palette_dd: Dropdown,
    ui_menu_focus: Optional[str],
    state: BakeState,
    rows: List[BakeRow],
    tool_rows: List[ToolBakeRow],
) -> Dict[str, pygame.Rect]:
    screen.fill(UI_BG)
    mw, mh = screen.get_width(), screen.get_height()

    ui_x, ui_y = 12, 10
    row_h = 28
    gap = 12

    dd_w_model = 240
    dd_w_group = 200
    dd_w_action = 260

    draw_dropdown_header(screen, font_ui, model_dd, x=ui_x, y=ui_y, w=dd_w_model, h=row_h)
    draw_dropdown_header(screen, font_ui, group_dd, x=ui_x + dd_w_model + gap, y=ui_y, w=dd_w_group, h=row_h)
    draw_dropdown_header(
        screen, font_ui, action_dd,
        x=ui_x + dd_w_model + gap + dd_w_group + gap,
        y=ui_y,
        w=dd_w_action,
        h=row_h,
    )

    mode_w = 170
    mode_x = mw - mode_w - 12
    draw_dropdown_header(screen, font_ui, mode_dd, x=mode_x, y=ui_y, w=mode_w, h=row_h)

    scope_w = 220
    pal_w = 240
    y2 = ui_y + row_h + 8

    draw_dropdown_header(screen, font_ui, bake_scope_dd, x=ui_x, y=y2, w=scope_w, h=row_h)
    draw_dropdown_header(screen, font_ui, bake_palette_dd, x=ui_x + scope_w + gap, y=y2, w=pal_w, h=row_h)

    mp = pygame.mouse.get_pos()

    bx = ui_x + scope_w + gap + pal_w + gap
    btn_scan = pygame.Rect(bx, y2, 120, row_h)
    btn_bake = pygame.Rect(btn_scan.right + 10, y2, 170, row_h)
    btn_force = pygame.Rect(btn_bake.right + 10, y2, 170, row_h)
    btn_clean = pygame.Rect(btn_force.right + 10, y2, 170, row_h)

    _draw_btn(screen, font_ui, btn_scan, "Scan", hovered=btn_scan.collidepoint(mp))
    _draw_btn(screen, font_ui, btn_bake, "Bake Missing", hovered=btn_bake.collidepoint(mp))
    _draw_btn(screen, font_ui, btn_force, "Force Rebake", hovered=btn_force.collidepoint(mp))
    _draw_btn(screen, font_ui, btn_clean, "Clean Generated", hovered=btn_clean.collidepoint(mp))

    y3 = y2 + row_h + 8
    btn_tool_scan = pygame.Rect(ui_x, y3, 160, row_h)
    btn_tool_bake = pygame.Rect(btn_tool_scan.right + 10, y3, 200, row_h)
    btn_tool_force = pygame.Rect(btn_tool_bake.right + 10, y3, 200, row_h)
    btn_tool_clean = pygame.Rect(btn_tool_force.right + 10, y3, 200, row_h)

    _draw_btn(screen, font_ui, btn_tool_scan, "Scan Tools", hovered=btn_tool_scan.collidepoint(mp))
    _draw_btn(screen, font_ui, btn_tool_bake, "Bake Tools Missing", hovered=btn_tool_bake.collidepoint(mp))
    _draw_btn(screen, font_ui, btn_tool_force, "Force Tool Rebake", hovered=btn_tool_force.collidepoint(mp))
    _draw_btn(screen, font_ui, btn_tool_clean, "Clean Tool Runtime", hovered=btn_tool_clean.collidepoint(mp))

    menus = {
        "mode": mode_dd,
        "model": model_dd,
        "group": group_dd,
        "action": action_dd,
        "scope": bake_scope_dd,
        "palette": bake_palette_dd,
    }

    for key, dd in menus.items():
        if dd.open and key != ui_menu_focus:
            draw_dropdown_menu(screen, font_ui, dd)
    if ui_menu_focus in menus and menus[ui_menu_focus].open:
        draw_dropdown_menu(screen, font_ui, menus[ui_menu_focus])

    top = y3 + row_h + 14
    left = 12
    right = mw - 12
    bottom = mh - 60
    list_h = max(0, bottom - top)

    header = font.render("Character Palette Bake Status (baked / total palettes)", True, UI_TEXT)
    screen.blit(header, (left, top))
    top += 26

    row_h2 = 54
    state.scroll = max(0, state.scroll)

    combined_count = len(rows) + 1 + len(tool_rows)
    max_scroll = max(0, combined_count * row_h2 - list_h)
    state.scroll = min(state.scroll, max_scroll)

    y = top - state.scroll

    for r0 in rows:
        rr = pygame.Rect(left, y, right - left, row_h2 - 8)
        if rr.bottom < top or rr.top > bottom:
            y += row_h2
            continue

        pygame.draw.rect(screen, (32, 32, 46), rr, border_radius=12)
        pygame.draw.rect(screen, (72, 72, 96), rr, width=1, border_radius=12)

        title = font.render(r0.action_label, True, UI_TEXT)
        screen.blit(title, (rr.x + 14, rr.y + 10))

        sub = font.render(r0.action_rel.as_posix(), True, UI_MUTED)
        screen.blit(sub, (rr.x + 14, rr.y + 28))

        bar = pygame.Rect(rr.right - 260, rr.y + 16, 160, 18)
        _draw_row_bar(screen, bar, baked=r0.baked, total=r0.total)

        txt = font.render(f"{r0.baked}/{r0.total}", True, UI_TEXT if r0.baked == r0.total else UI_WARN)
        screen.blit(txt, (rr.right - 90, rr.y + 14))

        y += row_h2

    sep = pygame.Rect(left, y + 6, right - left, 1)
    pygame.draw.rect(screen, (70, 70, 90), sep)
    y += row_h2

    tool_header = font.render("Tool Runtime Bake Status (per tool variant + clip) (baked / total palettes)", True, UI_TEXT)
    screen.blit(tool_header, (left, y - 34))

    for tr in tool_rows:
        rr = pygame.Rect(left, y, right - left, row_h2 - 8)
        if rr.bottom < top or rr.top > bottom:
            y += row_h2
            continue

        pygame.draw.rect(screen, (30, 30, 42), rr, border_radius=12)
        pygame.draw.rect(screen, (72, 72, 96), rr, width=1, border_radius=12)

        title = font.render(tr.clip_label, True, UI_TEXT)
        screen.blit(title, (rr.x + 14, rr.y + 10))

        bar = pygame.Rect(rr.right - 260, rr.y + 16, 160, 18)
        _draw_row_bar(screen, bar, baked=tr.baked, total=tr.total)

        txt = font.render(f"{tr.baked}/{tr.total}", True, UI_TEXT if tr.baked == tr.total else UI_WARN)
        screen.blit(txt, (rr.right - 90, rr.y + 14))

        y += row_h2

    if state.last_msg and (time.time() - state.last_msg_ts) < 6.0:
        screen.blit(font.render(state.last_msg, True, UI_TEXT), (12, mh - 55))

    help_line = "Mousewheel scroll | Character: Scan/Bake | Tools: Scan Tools / Bake Tools | TAB back"
    screen.blit(font.render(help_line, True, UI_MUTED), (12, mh - 28))

    return {
        "btn_scan": btn_scan,
        "btn_bake": btn_bake,
        "btn_force": btn_force,
        "btn_clean": btn_clean,
        "btn_tool_scan": btn_tool_scan,
        "btn_tool_bake": btn_tool_bake,
        "btn_tool_force": btn_tool_force,
        "btn_tool_clean": btn_tool_clean,
    }


def _actions_in_scope(
    *,
    scope: str,
    group: str,
    action: Optional[ActionEntry],
    action_groups: ActionGroups,
) -> List[ActionEntry]:
    scope = scope.lower().strip()
    group = group.lower().strip()

    if scope == "current action":
        return [action] if action else []
    if scope == "current group":
        return action_groups.actions_for_group(group)
    return action_groups.base + action_groups.skills + action_groups.combat


def _selected_palettes(bake_palette_dd: Dropdown, palettes: List[Palette]) -> List[Palette]:
    sel = (bake_palette_dd.selected() or "all").strip().lower()
    if sel == "all":
        return palettes
    for p in palettes:
        if p.name.strip().lower() == sel:
            return [p]
    return palettes


def handle_bake_click(
    pos: Tuple[int, int],
    *,
    ui_rects: Dict[str, pygame.Rect],
    ui_menu_focus: Optional[str],
    mode_dd: Dropdown,
    model_dd: Dropdown,
    group_dd: Dropdown,
    action_dd: Dropdown,
    bake_scope_dd: Dropdown,
    bake_palette_dd: Dropdown,
) -> Tuple[Optional[Tuple[str, object]], Optional[str]]:
    menus = {
        "mode": mode_dd,
        "model": model_dd,
        "group": group_dd,
        "action": action_dd,
        "scope": bake_scope_dd,
        "palette": bake_palette_dd,
    }

    any_open = any(dd.open for dd in menus.values())
    if any_open:
        focus = ui_menu_focus
        if focus not in menus or not menus[focus].open:
            focus = next((k for k, dd in menus.items() if dd.open), None)

        if focus is not None:
            dd = menus[focus]
            if dd.rect.collidepoint(pos):
                dd.open = False
                return None, None

            menu_rect = pygame.Rect(dd.rect.x, dd.rect.bottom + 4, dd.rect.w, min(16, len(dd.items)) * dd.rect.h)
            if menu_rect.collidepoint(pos):
                i = (pos[1] - menu_rect.y) // dd.rect.h
                if 0 <= i < min(16, len(dd.items)):
                    dd.selected_index = int(i)
                    for other in menus.values():
                        other.open = False
                    return (f"{focus}_changed", None), None

            for other in menus.values():
                other.open = False
            return None, None

        for other in menus.values():
            other.open = False
        return None, None

    for key, dd in menus.items():
        if dd.rect.collidepoint(pos):
            dd.open = True
            for other in menus.values():
                if other is not dd:
                    other.open = False
            return None, key

    if ui_rects.get("btn_scan") and ui_rects["btn_scan"].collidepoint(pos):
        return ("bake_scan", None), None
    if ui_rects.get("btn_bake") and ui_rects["btn_bake"].collidepoint(pos):
        return ("bake_missing", None), None
    if ui_rects.get("btn_force") and ui_rects["btn_force"].collidepoint(pos):
        return ("bake_force", None), None
    if ui_rects.get("btn_clean") and ui_rects["btn_clean"].collidepoint(pos):
        return ("bake_clean", None), None

    if ui_rects.get("btn_tool_scan") and ui_rects["btn_tool_scan"].collidepoint(pos):
        return ("tool_scan", None), None
    if ui_rects.get("btn_tool_bake") and ui_rects["btn_tool_bake"].collidepoint(pos):
        return ("tool_bake_missing", None), None
    if ui_rects.get("btn_tool_force") and ui_rects["btn_tool_force"].collidepoint(pos):
        return ("tool_bake_force", None), None
    if ui_rects.get("btn_tool_clean") and ui_rects["btn_tool_clean"].collidepoint(pos):
        return ("tool_clean", None), None

    return None, None


def do_bake_operation(
    *,
    state: BakeState,
    base_dir: Path,
    generated_dir: Path,
    palettes_dir: Path,
    action_groups: ActionGroups,
    group: str,
    current_action: Optional[ActionEntry],
    bake_scope_dd: Dropdown,
    bake_palette_dd: Dropdown,
    force: bool,
    clean: bool,
) -> None:
    palettes = _discover_palettes(palettes_dir)
    pals = _selected_palettes(bake_palette_dd, palettes)

    scope = (bake_scope_dd.selected() or "All Actions").strip().lower()
    actions = _actions_in_scope(scope=scope, group=group, action=current_action, action_groups=action_groups)

    if clean:
        cleaned = 0
        for a in actions:
            if (generated_dir / a.rel_path).exists():
                try:
                    shutil.rmtree(generated_dir / a.rel_path)
                    cleaned += 1
                except Exception:
                    pass
        state._msg(f"Cleaned generated for {cleaned} actions")
        return

    if not pals:
        state._msg("No palettes found to bake")
        return
    if not actions:
        state._msg("No actions in scope to bake")
        return

    t0 = time.time()
    baked_actions = 0
    for a in actions:
        ensure_baked_for_action(
            base_dir=base_dir,
            generated_dir=generated_dir,
            action_rel=a.rel_path,
            palettes=pals,
            force=force,
            bake_missing_only=True,
        )
        baked_actions += 1

    dt = time.time() - t0
    state._msg(f"Baked {baked_actions} actions ({len(pals)} palette(s)) in {dt:.2f}s")


def do_tool_bake_operation(
    *,
    state: BakeState,
    project_root: Path,
    clip_rels: List[Path],
    tool_kind: str,
    force: bool,
    clean: bool,
) -> None:
    if not clip_rels:
        state._msg("No clips found for tool baking")
        return

    if clean:
        deleted = 0
        for clip_rel in clip_rels:
            clip_key = _safe_clip_key(clip_rel.as_posix())
            if not clip_key:
                continue
            tool_ids = _manifest_tool_variants_for_clip(tool_kind=tool_kind, clip_key=clip_key)
            for tool_id in tool_ids:
                deleted += _clean_tool_runtime_for_variant_clip(tool_kind=tool_kind, tool_id=tool_id, clip_key=clip_key)
        state._msg(f"Cleaned tool runtime: removed {deleted} palette-clip folder(s)")
        return

    total_baked = 0
    total_total = 0
    t0 = time.time()

    for clip_rel in clip_rels:
        clip_key = _safe_clip_key(clip_rel.as_posix())
        if not clip_key:
            continue

        tool_ids = _manifest_tool_variants_for_clip(tool_kind=tool_kind, clip_key=clip_key)
        for tool_id in tool_ids:
            pals = _tool_palettes_for(tool_kind=tool_kind, tool_id=tool_id, clip_key=clip_key)
            if not pals:
                continue

            baked, total = bake_tool_variant_clip(
                tool_kind=tool_kind,
                tool_id=tool_id,
                clip_key=clip_key,
                palettes=pals,
                force=force,
                bake_missing_only=True,
            )
            total_baked += baked
            total_total += total

    dt = time.time() - t0
    state._msg(f"Tool bake done: baked {total_baked}/{total_total} palette(s) in {dt:.2f}s")
    state.last_scan_ts = time.time()


def handle_bake_scroll(state: BakeState, wheel_y: int) -> None:
    state.scroll -= int(wheel_y * 60)
    state.scroll = max(0, state.scroll)