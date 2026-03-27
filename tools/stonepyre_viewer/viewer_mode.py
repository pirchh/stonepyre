from __future__ import annotations

from typing import Dict, List, Optional, Tuple
from pathlib import Path
import json

import pygame

from .config import (
    DIRECTIONS,
    UI_BG,
    UI_TEXT,
    UI_WARN,
    BASE_SPREAD,
    PEDESTAL_GAP,
    PEDESTAL_W,
    PEDESTAL_H,
    SHADOW_H,
    LABEL_GAP,
)
from .ui_common import Dropdown, draw_dropdown_header, draw_dropdown_menu, dropdown_menu_rect

from .pet_tools import create_pet_structure, sanitize_pet_name
from .xcf_import import run_xcf_import

THIS_FILE = Path(__file__).resolve()
VIEWER_DIR = THIS_FILE.parent
TOOLS_DIR = VIEWER_DIR.parent
PROJECT_ROOT = TOOLS_DIR.parent

TOOLS_MANIFEST_DIR = PROJECT_ROOT / "libs" / "templates" / "tools" / "manifests"
TOOLS_TEMPLATES_ROOT = PROJECT_ROOT / "libs" / "templates" / "tools"
TOOLS_GENERATED_RUNTIME_ROOT = PROJECT_ROOT / "libs" / "generated_runtime" / "tools"

# Tool palettes live at:
#   libs/palettes/<clip_leaf>/<tool_kind>/<tool_id>/*.json
TOOLS_PALETTES_ROOT = PROJECT_ROOT / "libs" / "palettes"

DEFAULT_TOOL_KIND = "axe"
PIVOT_FROM_BOTTOM_DEFAULT = 30

_tools_manifest_cache_by_kind: Dict[str, Dict[str, object]] = {}


# -------------------------------
# Small utilities
# -------------------------------

def clamp(v: float, lo: float, hi: float) -> float:
    return max(lo, min(hi, v))


def get_scaled(surface: pygame.Surface, scale: float, cache: dict) -> pygame.Surface:
    key = (id(surface), round(scale, 4))
    if key in cache:
        return cache[key]
    w = max(1, int(surface.get_width() * scale))
    h = max(1, int(surface.get_height() * scale))
    scaled = pygame.transform.smoothscale(surface, (w, h))
    cache[key] = scaled
    return scaled


def current_spread(zoom: float) -> int:
    return int(BASE_SPREAD * (zoom / 0.45) ** 1.0)


def draw_pedestal_under(screen_surf: pygame.Surface, rect: pygame.Rect, zoom: float) -> Tuple[int, int]:
    cx = rect.centerx
    top_y = rect.bottom + PEDESTAL_GAP

    w = int(PEDESTAL_W * (zoom / 0.45))
    h = int(PEDESTAL_H * (zoom / 0.45))
    sh = int(SHADOW_H * (zoom / 0.45))

    pygame.draw.ellipse(
        screen_surf,
        (16, 16, 18),
        pygame.Rect(cx - (w // 2) - 10, top_y + (h // 2) + 8, w + 20, sh),
    )
    pygame.draw.ellipse(screen_surf, (60, 60, 66), pygame.Rect(cx - (w // 2), top_y, w, h))
    pygame.draw.ellipse(screen_surf, (82, 82, 92), pygame.Rect(cx - (w // 2) + 10, top_y + 4, w - 20, h - 10))

    return (cx, top_y + h // 2)


def _draw_button(
    screen: pygame.Surface,
    font: pygame.font.Font,
    rect: pygame.Rect,
    label: str,
    *,
    hovered: bool,
    disabled: bool = False,
) -> None:
    if disabled:
        bg = (50, 50, 60)
        border = (70, 70, 85)
        txt = (140, 140, 155)
    else:
        bg = (65, 65, 90) if hovered else (52, 52, 74)
        border = (95, 95, 125)
        txt = (235, 235, 245)

    pygame.draw.rect(screen, bg, rect, border_radius=8)
    pygame.draw.rect(screen, border, rect, width=1, border_radius=8)

    t = font.render(label, True, txt)
    screen.blit(t, t.get_rect(center=rect.center))


def _is_pet_selected(model_dd: Dropdown) -> bool:
    sel = (model_dd.selected() or "").strip().lower()
    return sel.startswith("pet:")


def _selected_pet_name(model_dd: Dropdown) -> Optional[str]:
    sel = (model_dd.selected() or "").strip()
    if sel.lower().startswith("pet:"):
        return sel.split(":", 1)[1].strip()
    return None


def _tk_ask_string(title: str, prompt: str, initial: str = "") -> Optional[str]:
    try:
        import tkinter as tk
        from tkinter import simpledialog

        root = tk.Tk()
        root.withdraw()
        return simpledialog.askstring(title, prompt, initialvalue=initial)
    except Exception:
        return None


def _tk_pick_xcf(*, pet_name: str) -> Optional[str]:
    try:
        import tkinter as tk
        from tkinter import filedialog

        root = tk.Tk()
        root.withdraw()
        return (
            filedialog.askopenfilename(
                title=f"Select XCF to Import for pet: {pet_name}",
                filetypes=[("GIMP XCF Files", "*.xcf")],
            )
            or None
        )
    except Exception:
        return None


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


def _clip_dir(clip_key: str) -> Path:
    ck = _safe_clip_key(clip_key)
    if not ck:
        return Path("_unknown")
    return Path(*ck.split("/"))


def _clip_leaf(clip_key: str) -> str:
    ck = _safe_clip_key(clip_key)
    if not ck:
        return ""
    return ck.split("/")[-1].strip()


def _sanitize_id(s: str) -> str:
    out = (s or "").strip().lower().replace(" ", "_")
    out = "".join(c for c in out if (c.isalnum() or c == "_"))
    while "__" in out:
        out = out.replace("__", "_")
    return out.strip("_")


# -------------------------------
# Tools manifest helpers (viewer read-only)
# -------------------------------

def _manifest_path_for_kind(tool_kind: str) -> Path:
    tk = _sanitize_id(tool_kind) or DEFAULT_TOOL_KIND
    return TOOLS_MANIFEST_DIR / f"{tk}.json"


def _load_tools_manifest_cached_for_kind(tool_kind: str) -> dict:
    """
    Cache by mtime. ToolFit mode writes these manifests; Viewer reads them.
    """
    try:
        tk = _sanitize_id(tool_kind) or DEFAULT_TOOL_KIND
        cache = _tools_manifest_cache_by_kind.get(tk)
        if cache is None:
            cache = {"mtime": 0.0, "data": None}
            _tools_manifest_cache_by_kind[tk] = cache

        path = _manifest_path_for_kind(tk)
        if not path.exists():
            cache["mtime"] = 0.0
            cache["data"] = {}
            return {}

        mtime = path.stat().st_mtime
        if cache["data"] is not None and cache["mtime"] == mtime:
            return cache["data"] or {}

        with open(path, "r", encoding="utf-8") as f:
            data = json.load(f) or {}

        cache["mtime"] = mtime
        cache["data"] = data
        return data
    except Exception:
        return {}


def _pivot_from_bottom_px(tool_kind: str) -> int:
    data = _load_tools_manifest_cached_for_kind(tool_kind)
    if isinstance(data, dict):
        pivot = data.get("pivot")
        if isinstance(pivot, dict):
            try:
                return int(pivot.get("from_bottom_px", PIVOT_FROM_BOTTOM_DEFAULT))
            except Exception:
                return PIVOT_FROM_BOTTOM_DEFAULT
    return PIVOT_FROM_BOTTOM_DEFAULT


def _tool_kind_for_clip(clip_key: str) -> str:
    ck = (clip_key or "").lower()
    if "woodcut" in ck:
        return "axe"
    if "mining" in ck:
        return "pickaxe"
    if "fishing" in ck:
        return "harpoon"
    return DEFAULT_TOOL_KIND


def _get_tool_pose_for_frame(
    *,
    tool_kind: str,
    tool_id: str,
    clip_key: str,
    direction: str,
    frame_index_0: int,
) -> Optional[Tuple[int, int, float, float]]:
    """
    Reads:
      tools/<tool_id>/clips/<clip_key>/frames/<direction>/<frame_key>
    """
    data = _load_tools_manifest_cached_for_kind(tool_kind)
    if not isinstance(data, dict):
        return None

    tools = data.get("tools")
    if not isinstance(tools, dict):
        return None

    t = tools.get(tool_id)
    if not isinstance(t, dict):
        return None

    clips = t.get("clips")
    if not isinstance(clips, dict):
        return None

    clip = clips.get(clip_key)
    if not isinstance(clip, dict):
        return None

    frames = clip.get("frames")
    if not isinstance(frames, dict):
        return None

    dir_data = frames.get(direction)
    if not isinstance(dir_data, dict):
        return None

    frame_key = f"{frame_index_0 + 1:02}"
    pose = dir_data.get(frame_key)
    if not isinstance(pose, dict):
        return None

    x = int(pose.get("x", 0))
    y = int(pose.get("y", 0))
    rot = float(pose.get("rot", 0.0))
    sc = float(pose.get("scale", 1.0))
    return x, y, rot, sc


# -------------------------------
# Discover functions used by app.py to populate dropdowns
# -------------------------------

def discover_tool_skins_for_clip(*, clip_key: str, tool_kind: Optional[str] = None) -> List[str]:
    """
    This actually returns TOOL VARIANTS (tool_id) that have this clip in the manifest.
    Kept name for compatibility with existing app.py wiring.
    """
    items: List[str] = ["none"]

    ck = _safe_clip_key(clip_key)
    if not ck:
        return items

    tk = _sanitize_id(tool_kind or _tool_kind_for_clip(ck)) or DEFAULT_TOOL_KIND
    data = _load_tools_manifest_cached_for_kind(tk)
    tools = data.get("tools") if isinstance(data, dict) else None
    if not isinstance(tools, dict):
        return items

    out: List[str] = []
    for tool_id, tool_obj in tools.items():
        if not isinstance(tool_id, str) or not isinstance(tool_obj, dict):
            continue
        clips = tool_obj.get("clips")
        if not isinstance(clips, dict):
            continue
        if ck in clips:
            out.append(tool_id)

    out.sort(key=lambda s: s.lower())
    return items + out


def discover_tool_variants_for_clip(*, clip_key: str, tool_kind: Optional[str] = None) -> List[str]:
    return discover_tool_skins_for_clip(clip_key=clip_key, tool_kind=tool_kind)


def discover_tool_palette_names_for(
    *,
    clip_key: str,
    tool_kind: str,
    tool_id: str,
) -> List[str]:
    """
    libs/palettes/<clip_leaf>/<tool_kind>/<tool_id>/*.json -> palette.name
    Viewer uses names only (ToolFit does baking).
    """
    if not tool_id or tool_id.lower() == "none":
        return ["match_character"]

    leaf = _sanitize_id(_clip_leaf(clip_key))
    tk = _sanitize_id(tool_kind) or DEFAULT_TOOL_KIND
    tid = _sanitize_id(tool_id)
    if not leaf or not tk or not tid:
        return ["match_character"]

    pals_dir = TOOLS_PALETTES_ROOT / leaf / tk / tid
    if not pals_dir.exists():
        return ["match_character"]

    names: List[str] = []
    for jf in sorted(pals_dir.glob("*.json")):
        try:
            obj = json.loads(jf.read_text(encoding="utf-8"))
            nm = (obj.get("name") or "").strip()
            names.append(nm if nm else jf.stem)
        except Exception:
            names.append(jf.stem)

    seen = set()
    out: List[str] = ["match_character"]
    for n in names:
        k = n.lower()
        if k in seen:
            continue
        seen.add(k)
        out.append(n)
    return out


# -------------------------------
# Tool image path resolution
# -------------------------------

def _tool_template_image_path_for(
    *,
    tool_kind: str,
    tool_id: str,
    clip_key: str,
    direction: str,
    frame_index_0: int,
) -> Optional[Path]:
    if tool_id == "none":
        return None

    ck = _safe_clip_key(clip_key)
    if not ck:
        return None

    clip_dir = _clip_dir(ck)
    if clip_dir.name == "_unknown":
        return None

    frame_name = f"{frame_index_0 + 1:02}.png"
    tk = _sanitize_id(tool_kind) or DEFAULT_TOOL_KIND
    tid = _sanitize_id(tool_id)
    if not tid:
        return None

    p = TOOLS_TEMPLATES_ROOT / tk / tid / clip_dir / direction / frame_name
    return p if p.exists() else None


def _tool_generated_image_path_for(
    *,
    tool_kind: str,
    tool_id: str,
    palette: str,
    clip_key: str,
    direction: str,
    frame_index_0: int,
) -> Optional[Path]:
    if tool_id == "none":
        return None

    ck = _safe_clip_key(clip_key)
    if not ck:
        return None

    clip_dir = _clip_dir(ck)
    if clip_dir.name == "_unknown":
        return None

    frame_name = f"{frame_index_0 + 1:02}.png"
    tk = _sanitize_id(tool_kind) or DEFAULT_TOOL_KIND
    tid = _sanitize_id(tool_id)
    pal = (palette or "").strip()
    if not tid or not pal:
        return None

    p = TOOLS_GENERATED_RUNTIME_ROOT / tk / tid / pal / clip_dir / direction / frame_name
    return p if p.exists() else None


def _resolve_tool_image_for_viewer(
    *,
    tool_kind: str,
    tool_id: str,
    clip_key: str,
    character_skin: str,  # "__greyscale__" or palette name
    tool_skin: str,       # "match_character" or palette name or "__greyscale__"
    direction: str,
    frame_index_0: int,
) -> Optional[Path]:
    if not tool_id or tool_id.lower() == "none":
        return None

    # Normalize dropdown strings
    tool_skin_norm = (tool_skin or "").strip()
    char_skin_norm = (character_skin or "").strip()

    # Tool skin can follow character
    effective = tool_skin_norm
    if tool_skin_norm.lower() == "match_character":
        effective = char_skin_norm

    # Greyscale -> template
    if not effective or effective == "__greyscale__" or effective.lower() == "greyscale":
        return _tool_template_image_path_for(
            tool_kind=tool_kind,
            tool_id=tool_id,
            clip_key=clip_key,
            direction=direction,
            frame_index_0=frame_index_0,
        )

    # Prefer baked runtime, fallback to template
    gen = _tool_generated_image_path_for(
        tool_kind=tool_kind,
        tool_id=tool_id,
        palette=effective,
        clip_key=clip_key,
        direction=direction,
        frame_index_0=frame_index_0,
    )
    if gen is not None:
        return gen

    return _tool_template_image_path_for(
        tool_kind=tool_kind,
        tool_id=tool_id,
        clip_key=clip_key,
        direction=direction,
        frame_index_0=frame_index_0,
    )


# -------------------------------
# Render
# -------------------------------

def draw_viewer(
    screen: pygame.Surface,
    font: pygame.font.Font,
    font_ui: pygame.font.Font,
    *,
    mode_dd: Dropdown,
    model_dd: Dropdown,
    group_dd: Dropdown,
    skin_dd: Dropdown,
    tool_dd: Dropdown,
    tool_skin_dd: Dropdown,  # tool palette selector
    action_dd: Dropdown,
    current_action_label: str,
    current_action_rel: str,
    bundle_surfaces: Dict[str, List[pygame.Surface]],
    idx: int,
    fps: int,
    zoom: float,
    scale_cache: dict,
    ui_menu_focus: Optional[str],
) -> Dict[str, pygame.Rect]:
    screen.fill(UI_BG)

    mw, mh = screen.get_width(), screen.get_height()
    cx, cy = mw // 2, mh // 2
    spread = current_spread(zoom)

    ui_x, ui_y = 12, 10
    row_h = 28
    gap = 12

    dd_w_model = 240
    dd_w_group = 200
    dd_w_skin = 260
    dd_w_action = 240
    dd_w_tool = 220
    dd_w_tool_skin = 220

    # top row
    draw_dropdown_header(screen, font_ui, model_dd, x=ui_x, y=ui_y, w=dd_w_model, h=row_h)
    draw_dropdown_header(screen, font_ui, group_dd, x=ui_x + dd_w_model + gap, y=ui_y, w=dd_w_group, h=row_h)
    draw_dropdown_header(
        screen,
        font_ui,
        skin_dd,
        x=ui_x + dd_w_model + gap + dd_w_group + gap,
        y=ui_y,
        w=dd_w_skin,
        h=row_h,
    )

    btn_w_new = 140
    btn_w_imp = 170
    btn_gap = 10
    new_pet_rect = pygame.Rect(ui_x + dd_w_model + gap + dd_w_group + gap + dd_w_skin + gap, ui_y, btn_w_new, row_h)
    import_xcf_rect = pygame.Rect(new_pet_rect.right + btn_gap, ui_y, btn_w_imp, row_h)

    mode_w = 170
    mode_x = mw - mode_w - 12
    draw_dropdown_header(screen, font_ui, mode_dd, x=mode_x, y=ui_y, w=mode_w, h=row_h)

    # row 2: action, tool, tool skin
    row2_y = ui_y + row_h + 8
    draw_dropdown_header(screen, font_ui, action_dd, x=ui_x, y=row2_y, w=dd_w_action, h=row_h)
    draw_dropdown_header(screen, font_ui, tool_dd, x=ui_x + dd_w_action + gap, y=row2_y, w=dd_w_tool, h=row_h)
    draw_dropdown_header(
        screen,
        font_ui,
        tool_skin_dd,
        x=ui_x + dd_w_action + gap + dd_w_tool + gap,
        y=row2_y,
        w=dd_w_tool_skin,
        h=row_h,
    )

    mouse_pos = pygame.mouse.get_pos()
    _draw_button(screen, font_ui, new_pet_rect, "+ New Pet", hovered=new_pet_rect.collidepoint(mouse_pos))
    pet_enabled = _is_pet_selected(model_dd)
    _draw_button(
        screen,
        font_ui,
        import_xcf_rect,
        "Import XCF",
        hovered=import_xcf_rect.collidepoint(mouse_pos),
        disabled=not pet_enabled,
    )

    menus = {
        "mode": mode_dd,
        "model": model_dd,
        "group": group_dd,
        "skin": skin_dd,
        "action": action_dd,
        "tool": tool_dd,
        "tool_skin": tool_skin_dd,
    }

    for key, dd in menus.items():
        if dd.open and key != ui_menu_focus:
            draw_dropdown_menu(screen, font_ui, dd)

    if ui_menu_focus in menus and menus[ui_menu_focus].open:
        draw_dropdown_menu(screen, font_ui, menus[ui_menu_focus])

    positions = {
        "north": (cx, cy - spread),
        "east": (cx + spread, cy),
        "south": (cx, cy + spread),
        "west": (cx - spread, cy),
    }

    clip_key = _safe_clip_key(current_action_rel)

    tool_id = (tool_dd.selected() or "none").strip()
    tool_kind = _tool_kind_for_clip(clip_key)
    pivot_from_bottom = _pivot_from_bottom_px(tool_kind)

    # Character skin selection
    sel_skin = (skin_dd.selected() or "greyscale").strip()
    character_skin = "__greyscale__" if sel_skin.lower() == "greyscale" else sel_skin

    # Tool skin selection (independent)
    tool_skin = (tool_skin_dd.selected() or "match_character").strip()

    if not bundle_surfaces:
        t = font.render("No frames loaded for current selection.", True, UI_WARN)
        screen.blit(t, (12, mh - 40))
    else:
        for d in DIRECTIONS:
            px, py = positions[d]
            if d not in bundle_surfaces:
                missing = font.render(f"{d.upper()} (missing)", True, (230, 120, 120))
                screen.blit(missing, missing.get_rect(center=(px, py)))
                continue

            frames = bundle_surfaces[d]
            if not frames:
                continue

            frame_i = idx % len(frames)
            frame = frames[frame_i]
            frame = get_scaled(frame, zoom, scale_cache)

            rect = frame.get_rect(center=(px, py))
            ped_cx, ped_cy = draw_pedestal_under(screen, rect, zoom)
            screen.blit(frame, rect)

            # Tool overlay if pose exists for this frame
            if clip_key and tool_id.lower() != "none":
                pose = _get_tool_pose_for_frame(
                    tool_kind=tool_kind,
                    tool_id=tool_id,
                    clip_key=clip_key,
                    direction=d,
                    frame_index_0=frame_i,
                )
                if pose is not None:
                    off_x, off_y, rot_deg, tool_scale = pose

                    pivot_x = rect.centerx
                    pivot_y = rect.bottom - int(pivot_from_bottom * zoom)
                    tool_x = pivot_x + int(off_x * zoom)
                    tool_y = pivot_y + int(off_y * zoom)

                    tool_path = _resolve_tool_image_for_viewer(
                        tool_kind=tool_kind,
                        tool_id=tool_id,
                        clip_key=clip_key,
                        character_skin=character_skin,
                        tool_skin=tool_skin,
                        direction=d,
                        frame_index_0=frame_i,
                    )
                    if tool_path is not None:
                        try:
                            # Cache raw loaded tool surfaces by path
                            key = ("tool_img", str(tool_path))
                            tool_img = scale_cache.get(key)
                            if tool_img is None:
                                tool_img = pygame.image.load(tool_path).convert_alpha()
                                scale_cache[key] = tool_img

                            tool_draw = pygame.transform.rotozoom(
                                tool_img,
                                -rot_deg,
                                max(0.001, zoom * tool_scale),
                            )
                            tool_rect = tool_draw.get_rect(center=(tool_x, tool_y))
                            screen.blit(tool_draw, tool_rect)
                        except Exception:
                            pass

            label = font.render(d.upper(), True, (200, 200, 210))
            label_rect = label.get_rect(
                center=(ped_cx, ped_cy + int(PEDESTAL_H * (zoom / 0.45)) // 2 + LABEL_GAP)
            )
            screen.blit(label, label_rect)

    overlay = (
        f"ACTION: {current_action_label} ({current_action_rel}) | frame {idx+1} | {fps} FPS | zoom {zoom:.2f} | "
        "R reload | TAB cycle modes | +/- zoom | ESC quit"
    )
    screen.blit(font.render(overlay, True, UI_TEXT), (10, mh - 26))

    return {
        "new_pet": new_pet_rect,
        "import_xcf": import_xcf_rect,
    }


# -------------------------------
# Click handling
# -------------------------------

def handle_viewer_click(
    pos: Tuple[int, int],
    *,
    mode_dd: Dropdown,
    model_dd: Dropdown,
    group_dd: Dropdown,
    skin_dd: Dropdown,
    tool_dd: Dropdown,
    tool_skin_dd: Dropdown,
    action_dd: Dropdown,
    ui_menu_focus: Optional[str],
    ui_rects: Dict[str, pygame.Rect],
) -> Tuple[Optional[Tuple[str, object]], Optional[str]]:
    menus = {
        "mode": mode_dd,
        "model": model_dd,
        "group": group_dd,
        "skin": skin_dd,
        "action": action_dd,
        "tool": tool_dd,
        "tool_skin": tool_skin_dd,
    }

    any_open = any(dd.open for dd in menus.values())
    if not any_open:
        if ui_rects["new_pet"].collidepoint(pos):
            raw = _tk_ask_string("New Pet", "Pet name:", "")
            if raw:
                pet_name = sanitize_pet_name(raw)
                if pet_name:
                    create_pet_structure(pet_name)
                    return ("pet_new", {"pet_name": pet_name}), None
            return None, None

        if ui_rects["import_xcf"].collidepoint(pos):
            pet = _selected_pet_name(model_dd)
            if not pet:
                return None, None

            xcf = _tk_pick_xcf(pet_name=pet)
            if not xcf:
                return None, None

            try:
                pet_name, _results = run_xcf_import(Path(xcf), pet_name=pet, scale=1)
                return ("pet_import_done", {"pet_name": pet_name}), None
            except Exception as e:
                print(f"[ERROR] XCF import failed: {e}")
                return None, None

    if any_open:
        focus = ui_menu_focus
        if focus not in menus or not menus[focus].open:
            focus = next((k for k, dd in menus.items() if dd.open), None)

        if focus is not None:
            dd = menus[focus]

            # click header again closes it
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
                    return (f"{focus}_changed", None), None

            # click outside -> close all
            for other in menus.values():
                other.open = False
            return None, None

        for other in menus.values():
            other.open = False
        return None, None

    # Open a dropdown if clicked
    for key, dd in menus.items():
        if dd.rect.collidepoint(pos):
            dd.open = True
            for other in menus.values():
                if other is not dd:
                    other.open = False
            return None, key

    return None, None