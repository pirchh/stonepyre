from __future__ import annotations

import json
import re
import shutil
import time
from pathlib import Path
from typing import Dict, List, Tuple, Optional, Any

import pygame

from .config import UI_BG, UI_TEXT, UI_WARN
from .ui_common import Dropdown, draw_dropdown_header, draw_dropdown_menu
from .palettes import Palette, iter_palette_files, load_palette_json

THIS_FILE = Path(__file__).resolve()
VIEWER_DIR = THIS_FILE.parent               # .../tools/stonepyre_viewer
TOOLS_DIR = VIEWER_DIR.parent               # .../tools
PROJECT_ROOT = TOOLS_DIR.parent             # .../Stonepyre

STAGING_ROOT = PROJECT_ROOT / "assets" / "_staging" / "tools"

TOOLS_TEMPLATE_ROOT = PROJECT_ROOT / "libs" / "templates" / "tools"
TOOLS_MANIFEST_DIR = TOOLS_TEMPLATE_ROOT / "manifests"

# unified output root (copy/paste friendly)
TOOLS_GENERATED_RUNTIME_ROOT = PROJECT_ROOT / "libs" / "generated_runtime" / "tools"

PIVOT_FROM_BOTTOM_DEFAULT = 30
DIRECTIONS = ["north", "east", "south", "west"]

_FRAME_NAME_SAN = re.compile(r"[^a-zA-Z0-9_]+")


def sanitize_id(raw: str) -> str:
    s = (raw or "").strip().lower().replace(" ", "_")
    s = _FRAME_NAME_SAN.sub("", s)
    s = re.sub(r"_+", "_", s).strip("_")
    return s


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


def _clip_to_dir(clip_key: str) -> Path:
    ck = _safe_clip_key(clip_key)
    if not ck:
        return Path("_unknown")
    return Path(*ck.split("/"))


def _clip_leaf(clip_key: str) -> str:
    ck = _safe_clip_key(clip_key)
    if not ck:
        return ""
    return ck.split("/")[-1].strip()


# -------------------------------
# TOOL KINDS (folders + manifests)
# -------------------------------

def discover_tool_kinds() -> List[str]:
    """
    Kinds are discovered from:
      - libs/templates/tools/<kind>/
      - libs/templates/tools/manifests/<kind>.json
    """
    kinds = set()

    try:
        if TOOLS_TEMPLATE_ROOT.exists():
            for p in TOOLS_TEMPLATE_ROOT.iterdir():
                if not p.is_dir():
                    continue
                if p.name == "manifests":
                    continue
                if p.name.startswith("_"):
                    continue
                kinds.add(sanitize_id(p.name))
    except Exception:
        pass

    try:
        if TOOLS_MANIFEST_DIR.exists():
            for jf in TOOLS_MANIFEST_DIR.glob("*.json"):
                kinds.add(sanitize_id(jf.stem))
    except Exception:
        pass

    out = sorted(k for k in kinds if k)
    return out or ["axe"]


def ensure_tool_kind_exists(tool_kind: str) -> bool:
    """
    Ensures:
      libs/templates/tools/<tool_kind>/
      libs/templates/tools/manifests/<tool_kind>.json (seeded)
    """
    tk = sanitize_id(tool_kind)
    if not tk:
        return False
    try:
        (TOOLS_TEMPLATE_ROOT / tk).mkdir(parents=True, exist_ok=True)

        TOOLS_MANIFEST_DIR.mkdir(parents=True, exist_ok=True)
        mpath = manifest_path_for_kind(tk)
        if not mpath.exists():
            seed = ensure_manifest_shape(tk, {})
            with open(mpath, "w", encoding="utf-8") as f:
                json.dump(seed, f, indent=2)
        return True
    except Exception:
        return False


def create_new_tool_kind_dialog(state: "ToolFitState") -> Optional[str]:
    try:
        import tkinter as tk
        from tkinter import simpledialog

        root = tk.Tk()
        root.withdraw()

        raw = simpledialog.askstring(
            "New Tool Kind",
            "Tool kind id (ex: sword, staff, bow):",
            initialvalue="sword",
        )
        if not raw:
            state._msg("New Kind cancelled")
            return None

        tkid = sanitize_id(raw)
        if not tkid:
            state._msg("[ERR] Tool kind invalid after sanitizing")
            return None

        if ensure_tool_kind_exists(tkid):
            state._msg(f"Created Tool Kind: {tkid}")
            return tkid

        state._msg("[ERR] Failed creating tool kind")
        return None
    except Exception as e:
        state._msg(f"[ERR] New Kind failed: {e}")
        return None


# -------------------------------
# TOOL PALETTES (PER CLIP LEAF / KIND / TOOL)
# -------------------------------

def tool_palettes_dir_for_clip(*, clip_key: str, tool_kind: str, tool_id: str) -> Path:
    """
    FINAL:
      Stonepyre/libs/palettes/<clip_leaf>/<tool_kind>/<tool_id>/
    Example:
      libs/palettes/woodcutting/axe/splitting_axe/
    """
    leaf = sanitize_id(_clip_leaf(clip_key))
    return PROJECT_ROOT / "libs" / "palettes" / leaf / sanitize_id(tool_kind) / sanitize_id(tool_id)


def discover_tool_palettes_for_clip(*, clip_key: str, tool_kind: str, tool_id: str) -> List[Palette]:
    pals_dir = tool_palettes_dir_for_clip(clip_key=clip_key, tool_kind=tool_kind, tool_id=tool_id)
    pals: List[Palette] = []
    if pals_dir.exists():
        for pf in list(iter_palette_files(pals_dir)):
            try:
                pals.append(load_palette_json(pf))
            except Exception:
                pass
    return pals


# -------------------------------
# MANIFEST IO (per tool_kind)
# -------------------------------

def manifest_path_for_kind(tool_kind: str) -> Path:
    return TOOLS_MANIFEST_DIR / f"{sanitize_id(tool_kind) or 'axe'}.json"


def load_manifest_for_kind(tool_kind: str) -> Dict[str, Any]:
    path = manifest_path_for_kind(tool_kind)
    if not path.exists():
        return {}
    try:
        with open(path, "r", encoding="utf-8") as f:
            return json.load(f) or {}
    except Exception:
        return {}


def save_manifest_for_kind(tool_kind: str, data: Dict[str, Any]) -> bool:
    try:
        TOOLS_MANIFEST_DIR.mkdir(parents=True, exist_ok=True)
        path = manifest_path_for_kind(tool_kind)
        with open(path, "w", encoding="utf-8") as f:
            json.dump(data, f, indent=2)
        return True
    except Exception:
        return False


def ensure_manifest_shape(tool_kind: str, data: Dict[str, Any]) -> Dict[str, Any]:
    out = dict(data or {})
    out.setdefault("version", 3)
    out["tool_kind"] = sanitize_id(out.get("tool_kind") or tool_kind or "axe") or "axe"
    pivot = out.get("pivot")
    if not isinstance(pivot, dict):
        pivot = {}
    pivot.setdefault("type", "bottom_center")
    pivot.setdefault("from_bottom_px", PIVOT_FROM_BOTTOM_DEFAULT)
    out["pivot"] = pivot

    tools = out.get("tools")
    if not isinstance(tools, dict):
        out["tools"] = {}
    return out


def pivot_from_bottom_px_for_kind(tool_kind: str) -> int:
    data = ensure_manifest_shape(tool_kind, load_manifest_for_kind(tool_kind))
    pivot = data.get("pivot")
    if isinstance(pivot, dict):
        try:
            return int(pivot.get("from_bottom_px", PIVOT_FROM_BOTTOM_DEFAULT))
        except Exception:
            return PIVOT_FROM_BOTTOM_DEFAULT
    return PIVOT_FROM_BOTTOM_DEFAULT


def ensure_tool_exists_in_manifest(*, tool_kind: str, tool_id: str, display_name: str) -> None:
    tool_id = sanitize_id(tool_id)
    tool_kind = sanitize_id(tool_kind) or "axe"
    if not tool_id:
        return

    ensure_tool_kind_exists(tool_kind)

    data = ensure_manifest_shape(tool_kind, load_manifest_for_kind(tool_kind))
    tools = data["tools"]
    if tool_id not in tools:
        tools[tool_id] = {"display_name": display_name or tool_id, "clips": {}}
        save_manifest_for_kind(tool_kind, data)


def ensure_clip_exists(*, tool_kind: str, tool_id: str, clip_key: str) -> None:
    tool_id = sanitize_id(tool_id)
    tool_kind = sanitize_id(tool_kind) or "axe"
    clip_key = _safe_clip_key(clip_key)
    if not tool_id or not clip_key:
        return

    ensure_tool_kind_exists(tool_kind)

    data = ensure_manifest_shape(tool_kind, load_manifest_for_kind(tool_kind))
    tools = data["tools"]
    t = tools.setdefault(tool_id, {"display_name": tool_id, "clips": {}})
    clips = t.setdefault("clips", {})
    clip_obj = clips.setdefault(clip_key, {})
    if "frames" not in clip_obj or not isinstance(clip_obj.get("frames"), dict):
        clip_obj["frames"] = {}
    save_manifest_for_kind(tool_kind, data)


# -------------------------------
# IMAGE PALETTE APPLY (EXACT COLOR REPLACE)
# -------------------------------

def _hex_to_rgb(h: str) -> Tuple[int, int, int]:
    h = (h or "").strip()
    if h.startswith("#"):
        h = h[1:]
    if len(h) != 6:
        return (0, 0, 0)
    return (int(h[0:2], 16), int(h[2:4], 16), int(h[4:6], 16))


def apply_palette_to_surface(src: pygame.Surface, pal: Palette) -> pygame.Surface:
    out = src.copy().convert_alpha()
    replace_map = getattr(pal, "replace", None) or {}
    if not isinstance(replace_map, dict) or not replace_map:
        return out

    mapping: Dict[Tuple[int, int, int], Tuple[int, int, int]] = {}
    for k, v in replace_map.items():
        mapping[_hex_to_rgb(str(k))] = _hex_to_rgb(str(v))

    try:
        import pygame.surfarray as surfarray
        rgb = surfarray.pixels3d(out)
        for src_rgb, dst_rgb in mapping.items():
            sr, sg, sb = src_rgb
            dr, dg, db = dst_rgb
            mask = (rgb[:, :, 0] == sr) & (rgb[:, :, 1] == sg) & (rgb[:, :, 2] == sb)
            if mask.any():
                rgb[:, :, 0][mask] = dr
                rgb[:, :, 1][mask] = dg
                rgb[:, :, 2][mask] = db
        del rgb
        return out
    except Exception:
        px = pygame.PixelArray(out)
        for x in range(out.get_width()):
            for y in range(out.get_height()):
                c = out.unmap_rgb(px[x, y])
                src_rgb = (c.r, c.g, c.b)
                if src_rgb in mapping and c.a > 0:
                    dr, dg, db = mapping[src_rgb]
                    px[x, y] = out.map_rgb((dr, dg, db))
        del px
        return out


# -------------------------------
# PATH RESOLUTION (templates vs generated_runtime)
# -------------------------------

def tool_template_path(
    *,
    tool_kind: str,
    tool_id: str,
    clip_key: str,
    direction: str,
    frame_index: int,  # 0-based
) -> Path:
    clip_dir = _clip_to_dir(clip_key)
    frame_name = f"{frame_index + 1:02}.png"
    return TOOLS_TEMPLATE_ROOT / sanitize_id(tool_kind) / sanitize_id(tool_id) / clip_dir / direction / frame_name


def tool_baked_runtime_path(
    *,
    tool_kind: str,
    tool_id: str,
    palette: str,
    clip_key: str,
    direction: str,
    frame_index: int,
) -> Path:
    clip_dir = _clip_to_dir(clip_key)
    frame_name = f"{frame_index + 1:02}.png"
    return (
        TOOLS_GENERATED_RUNTIME_ROOT
        / sanitize_id(tool_kind)
        / sanitize_id(tool_id)
        / (palette or "").strip()
        / clip_dir
        / direction
        / frame_name
    )


def resolve_tool_overlay_path(
    *,
    tool_kind: str,
    tool_id: str,
    clip_key: str,
    skin: str,          # "__greyscale__" or palette name
    direction: str,
    frame_index: int,   # 0-based
) -> Path:
    if not skin or skin == "__greyscale__":
        return tool_template_path(
            tool_kind=tool_kind,
            tool_id=tool_id,
            clip_key=clip_key,
            direction=direction,
            frame_index=frame_index,
        )
    return tool_baked_runtime_path(
        tool_kind=tool_kind,
        tool_id=tool_id,
        palette=skin,
        clip_key=clip_key,
        direction=direction,
        frame_index=frame_index,
    )


# -------------------------------
# BAKING (per tool variant -> generated_runtime)
# -------------------------------

def bake_tool_overlays_for_clip(
    *,
    clip_key: str,
    tool_kind: str,
    tool_id: str,
    force: bool = False,
    bake_missing_only: bool = True,
) -> Tuple[int, int]:
    clip_key = _safe_clip_key(clip_key)
    tool_kind = sanitize_id(tool_kind) or "axe"
    tool_id = sanitize_id(tool_id)
    if not clip_key or not tool_id:
        return (0, 0)

    pals = discover_tool_palettes_for_clip(clip_key=clip_key, tool_kind=tool_kind, tool_id=tool_id)
    if not pals:
        return (0, 0)

    baked = 0
    skipped = 0

    for pal in pals:
        pal_name = (pal.name or "").strip()
        if not pal_name:
            continue

        for d in DIRECTIONS:
            src_dir = tool_template_path(
                tool_kind=tool_kind,
                tool_id=tool_id,
                clip_key=clip_key,
                direction=d,
                frame_index=0,
            ).parent
            if not src_dir.exists():
                continue

            out_dir = tool_baked_runtime_path(
                tool_kind=tool_kind,
                tool_id=tool_id,
                palette=pal_name,
                clip_key=clip_key,
                direction=d,
                frame_index=0,
            ).parent
            out_dir.mkdir(parents=True, exist_ok=True)

            for src_png in sorted(src_dir.glob("*.png")):
                out_png = out_dir / src_png.name
                if bake_missing_only and out_png.exists() and not force:
                    skipped += 1
                    continue
                try:
                    src_img = pygame.image.load(src_png).convert_alpha()
                    out_img = apply_palette_to_surface(src_img, pal)
                    pygame.image.save(out_img, out_png)
                    baked += 1
                except Exception:
                    continue

    return (baked, skipped)


# -------------------------------
# STATE
# -------------------------------

class ToolFitState:
    def __init__(self):
        self.direction = "south"
        self.frame_index = 0

        self.clip_key: str = "skills/woodcutting"
        self.tool_kind: str = "axe"
        self.tool_id: str = "fledgling_axe"

        self.frames: Dict[str, Dict[str, Dict[str, float]]] = {}

        self.dirty = False
        self.last_msg: str = ""
        self.last_msg_ts: float = 0.0

        self.dragging: bool = False
        self.drag_start_mouse: Tuple[int, int] = (0, 0)
        self.drag_start_pose_xy: Tuple[int, int] = (0, 0)
        self._last_tool_screen_pos: Tuple[int, int] = (0, 0)

        self._rot_hold_start_i: float = 0.0
        self._rot_hold_start_o: float = 0.0

        ensure_tool_kind_exists(self.tool_kind)
        self.load_from_manifest()

    def _msg(self, s: str) -> None:
        self.last_msg = s
        self.last_msg_ts = time.time()

    def set_clip(self, action_rel_path: str) -> None:
        """
        Clip changes should NOT infer/overwrite tool_kind anymore.
        Kind is selected explicitly from the UI.
        """
        ck = _safe_clip_key(action_rel_path)
        if not ck:
            return

        changed = (ck != self.clip_key)
        self.clip_key = ck

        if changed:
            if self.tool_id and self.tool_id.lower() != "none":
                ensure_tool_exists_in_manifest(tool_kind=self.tool_kind, tool_id=self.tool_id, display_name=self.tool_id)
                ensure_clip_exists(tool_kind=self.tool_kind, tool_id=self.tool_id, clip_key=self.clip_key)
            self.load_from_manifest()

    def set_tool_kind(self, tool_kind: str) -> None:
        tk = sanitize_id(tool_kind) or "axe"
        if tk != sanitize_id(self.tool_kind):
            self.tool_kind = tk
            ensure_tool_kind_exists(self.tool_kind)
            # ensure current tool+clip exist in this kind
            if self.tool_id and self.tool_id.lower() != "none":
                ensure_tool_exists_in_manifest(tool_kind=self.tool_kind, tool_id=self.tool_id, display_name=self.tool_id)
                ensure_clip_exists(tool_kind=self.tool_kind, tool_id=self.tool_id, clip_key=self.clip_key)
            self.load_from_manifest()

    def set_tool_id(self, tool_id: str) -> None:
        tid = sanitize_id(tool_id)
        if not tid or tid == "none":
            return
        if tid != self.tool_id:
            self.tool_id = tid
            ensure_tool_exists_in_manifest(tool_kind=self.tool_kind, tool_id=self.tool_id, display_name=self.tool_id)
            ensure_clip_exists(tool_kind=self.tool_kind, tool_id=self.tool_id, clip_key=self.clip_key)
            self.load_from_manifest()

    def load_from_manifest(self) -> None:
        self.frames = {}
        self.dirty = False

        self.tool_kind = sanitize_id(self.tool_kind) or "axe"
        ensure_tool_kind_exists(self.tool_kind)

        self.tool_id = sanitize_id(self.tool_id) or "fledgling_axe"
        self.clip_key = _safe_clip_key(self.clip_key) or "skills/woodcutting"

        data = ensure_manifest_shape(self.tool_kind, load_manifest_for_kind(self.tool_kind))
        tools = data.get("tools", {}) or {}
        t = tools.get(self.tool_id) or {}
        clips = t.get("clips", {}) or {}
        clip_obj = clips.get(self.clip_key) or {}
        frames = clip_obj.get("frames")

        if isinstance(frames, dict):
            self.frames = frames  # type: ignore[assignment]
            self._msg(f"Loaded: {self.tool_kind}/{self.tool_id} clip={self.clip_key}")
        else:
            self.frames = {}
            self._msg(f"No frames yet for {self.tool_kind}/{self.tool_id} clip={self.clip_key} (edit then Save)")

    def save_to_manifest(self) -> None:
        try:
            self.tool_kind = sanitize_id(self.tool_kind) or "axe"
            ensure_tool_kind_exists(self.tool_kind)

            self.tool_id = sanitize_id(self.tool_id)
            self.clip_key = _safe_clip_key(self.clip_key)

            if not self.tool_id or self.tool_id == "none" or not self.clip_key:
                self._msg("[ERR] Missing tool_id or clip_key")
                return

            data = ensure_manifest_shape(self.tool_kind, load_manifest_for_kind(self.tool_kind))
            tools = data["tools"]
            tool_obj = tools.setdefault(self.tool_id, {"display_name": self.tool_id, "clips": {}})
            clips = tool_obj.setdefault("clips", {})
            clip_obj = clips.setdefault(self.clip_key, {})
            clip_obj["frames"] = self.frames

            ok = save_manifest_for_kind(self.tool_kind, data)
            if ok:
                self.dirty = False
                self._msg(f"Saved: {manifest_path_for_kind(self.tool_kind).as_posix()}")
            else:
                self._msg("[ERR] Save failed")
        except Exception as e:
            self._msg(f"[ERR] Save failed: {e}")

    def get_pose(self) -> Tuple[int, int, float, float]:
        frames = self.frames
        dir_data = frames.setdefault(self.direction, {})
        frame_key = f"{self.frame_index + 1:02}"
        frame_data = dir_data.setdefault(frame_key, {"x": 0, "y": 0, "rot": 0.0, "scale": 1.0})

        x = int(frame_data.get("x", 0))
        y = int(frame_data.get("y", 0))
        rot = float(frame_data.get("rot", 0.0))
        scale = float(frame_data.get("scale", 1.0))
        return x, y, rot, scale

    def set_pose(self, x: int, y: int, rot: float, scale: float) -> None:
        frames = self.frames
        dir_data = frames.setdefault(self.direction, {})
        frame_key = f"{self.frame_index + 1:02}"
        dir_data[frame_key] = {"x": int(x), "y": int(y), "rot": float(rot), "scale": float(scale)}
        self.dirty = True
        self._msg(
            f"[{self.tool_kind}/{self.tool_id}] {self.clip_key} {self.direction} {frame_key}: "
            f"({int(x)}, {int(y)}) rot={rot:.1f} sc={scale:.2f}"
        )


def create_new_tool_variant(*, state: ToolFitState) -> None:
    try:
        import tkinter as tk
        from tkinter import simpledialog

        root = tk.Tk()
        root.withdraw()

        name = simpledialog.askstring("New Tool", "Tool name (ex: Splitting Axe):", initialvalue="Splitting Axe")
        if not name:
            state._msg("New Tool cancelled")
            return

        tool_id = sanitize_id(name)
        if not tool_id:
            state._msg("[ERR] Tool name invalid after sanitizing")
            return

        tool_kind = sanitize_id(state.tool_kind) or "axe"
        ensure_tool_kind_exists(tool_kind)

        clip_key = _safe_clip_key(state.clip_key)
        if not clip_key:
            state._msg("[ERR] Current clip_key empty")
            return

        ensure_tool_exists_in_manifest(tool_kind=tool_kind, tool_id=tool_id, display_name=name)
        ensure_clip_exists(tool_kind=tool_kind, tool_id=tool_id, clip_key=clip_key)

        for d in DIRECTIONS:
            p = tool_template_path(
                tool_kind=tool_kind,
                tool_id=tool_id,
                clip_key=clip_key,
                direction=d,
                frame_index=0,
            ).parent
            p.mkdir(parents=True, exist_ok=True)

        state.tool_kind = tool_kind
        state.tool_id = tool_id
        state.load_from_manifest()
        state._msg(f"Created tool: {tool_kind}/{tool_id} (folders created for {clip_key})")

    except Exception as e:
        state._msg(f"[ERR] New Tool failed: {e}")


def import_png_for_slot(state: ToolFitState) -> None:
    try:
        import tkinter as tk
        from tkinter import filedialog

        root = tk.Tk()
        root.withdraw()
        path = filedialog.askopenfilename(
            title="Select Tool PNG (greyscale template frame)",
            initialdir=str(STAGING_ROOT),
            filetypes=[("PNG files", "*.png")],
        )
        if not path:
            state._msg("Import cancelled")
            return
    except Exception as e:
        state._msg(f"[ERR] Import dialog failed: {e}")
        return

    try:
        if not state.tool_id or state.tool_id == "none":
            state._msg("[ERR] Select a Tool first (tool dropdown)")
            return

        dest = tool_template_path(
            tool_kind=state.tool_kind,
            tool_id=state.tool_id,
            clip_key=state.clip_key,
            direction=state.direction,
            frame_index=state.frame_index,
        )
        dest.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy(path, dest)
        state._msg(f"Imported -> {dest.as_posix()}")
    except Exception as e:
        state._msg(f"[ERR] Import copy failed: {e}")


def _draw_button(screen: pygame.Surface, font: pygame.font.Font, rect: pygame.Rect, label: str, *, active: bool = False) -> None:
    bg = (70, 70, 95) if active else (52, 52, 74)
    border = (130, 130, 160) if active else (95, 95, 125)
    txt = (245, 245, 255)
    pygame.draw.rect(screen, bg, rect, border_radius=8)
    pygame.draw.rect(screen, border, rect, width=1, border_radius=8)
    t = font.render(label, True, txt)
    screen.blit(t, t.get_rect(center=rect.center))


def _available_dir(bundle: Dict[str, List[pygame.Surface]]) -> Optional[str]:
    for d in DIRECTIONS:
        if bundle.get(d):
            return d
    for d, frames in bundle.items():
        if frames:
            return d
    return None


def _ramp_multiplier(held_for: float) -> float:
    if held_for < 0.5:
        return 1.0
    if held_for < 1.0:
        return 3.0
    if held_for < 2.0:
        return 8.0
    return 15.0


def draw_tool_fit_header(
    screen: pygame.Surface,
    font_ui: pygame.font.Font,
    *,
    mode_dd: Dropdown,
    model_dd: Dropdown,
    group_dd: Dropdown,
    skin_dd: Dropdown,
    action_dd: Dropdown,
    tool_kind_dd: Dropdown,   # NEW
    tool_dd: Dropdown,
    ui_menu_focus: Optional[str],
    state: ToolFitState,
) -> Dict[str, pygame.Rect]:
    mw, _mh = screen.get_width(), screen.get_height()

    ui_x, ui_y = 12, 10
    row_h = 28
    gap = 12

    dd_w_model = 240
    dd_w_group = 200
    dd_w_skin = 260

    dd_w_action = 240
    dd_w_kind = 160   # NEW
    dd_w_tool = 240

    draw_dropdown_header(screen, font_ui, model_dd, x=ui_x, y=ui_y, w=dd_w_model, h=row_h)
    draw_dropdown_header(screen, font_ui, group_dd, x=ui_x + dd_w_model + gap, y=ui_y, w=dd_w_group, h=row_h)
    draw_dropdown_header(
        screen, font_ui, skin_dd,
        x=ui_x + dd_w_model + gap + dd_w_group + gap, y=ui_y, w=dd_w_skin, h=row_h
    )

    mode_w = 170
    mode_x = mw - mode_w - 12
    draw_dropdown_header(screen, font_ui, mode_dd, x=mode_x, y=ui_y, w=mode_w, h=row_h)

    row2_y = ui_y + row_h + 8
    draw_dropdown_header(screen, font_ui, action_dd, x=ui_x, y=row2_y, w=dd_w_action, h=row_h)
    draw_dropdown_header(screen, font_ui, tool_kind_dd, x=ui_x + dd_w_action + gap, y=row2_y, w=dd_w_kind, h=row_h)
    draw_dropdown_header(
        screen,
        font_ui,
        tool_dd,
        x=ui_x + dd_w_action + gap + dd_w_kind + gap,
        y=row2_y,
        w=dd_w_tool,
        h=row_h,
    )

    rects: Dict[str, pygame.Rect] = {}

    btn_new_kind = pygame.Rect(
        ui_x + dd_w_action + gap + dd_w_kind + gap + dd_w_tool + gap,
        row2_y,
        120,
        row_h,
    )
    _draw_button(screen, font_ui, btn_new_kind, "New Kind")
    rects["new_kind"] = btn_new_kind

    btn_new_tool = pygame.Rect(btn_new_kind.right + gap, row2_y, 120, row_h)
    _draw_button(screen, font_ui, btn_new_tool, "New Tool")
    rects["new_tool"] = btn_new_tool

    dir_btn_w = 52
    dir_btn_gap = 8
    dir_x0 = btn_new_tool.right + gap

    labels = [("north", "N"), ("east", "E"), ("south", "S"), ("west", "W")]
    for i, (d, lab) in enumerate(labels):
        r = pygame.Rect(dir_x0 + i * (dir_btn_w + dir_btn_gap), row2_y, dir_btn_w, row_h)
        _draw_button(screen, font_ui, r, lab, active=(state.direction == d))
        rects[f"dir:{d}"] = r

    menus = {
        "mode": mode_dd,
        "model": model_dd,
        "group": group_dd,
        "skin": skin_dd,
        "action": action_dd,
        "tool_kind": tool_kind_dd,  # NEW
        "tool": tool_dd,
    }

    for key, dd in menus.items():
        if dd.open and key != ui_menu_focus:
            draw_dropdown_menu(screen, font_ui, dd)
    if ui_menu_focus in menus and menus[ui_menu_focus].open:
        draw_dropdown_menu(screen, font_ui, menus[ui_menu_focus])

    return rects


def update_tool_fit_held_keys(state: ToolFitState, *, zoom: float, dt: float) -> None:
    keys = pygame.key.get_pressed()
    mods = pygame.key.get_mods()
    now = time.time()

    step = 1
    if mods & pygame.KMOD_SHIFT:
        step = 10
    if mods & pygame.KMOD_CTRL:
        step = 50

    base_speed = 14.0
    speed = base_speed * step

    dx = 0.0
    dy = 0.0
    if keys[pygame.K_LEFT]:
        dx -= speed * dt
    if keys[pygame.K_RIGHT]:
        dx += speed * dt
    if keys[pygame.K_UP]:
        dy -= speed * dt
    if keys[pygame.K_DOWN]:
        dy += speed * dt

    base_deg_per_sec = 40.0
    if mods & pygame.KMOD_SHIFT:
        base_deg_per_sec = 90.0
    if mods & pygame.KMOD_CTRL:
        base_deg_per_sec = 180.0

    rot_delta = 0.0

    if keys[pygame.K_i]:
        if state._rot_hold_start_i <= 0.0:
            state._rot_hold_start_i = now
        held_for = now - state._rot_hold_start_i
        rot_delta -= base_deg_per_sec * _ramp_multiplier(held_for) * dt
    else:
        state._rot_hold_start_i = 0.0

    if keys[pygame.K_o]:
        if state._rot_hold_start_o <= 0.0:
            state._rot_hold_start_o = now
        held_for = now - state._rot_hold_start_o
        rot_delta += base_deg_per_sec * _ramp_multiplier(held_for) * dt
    else:
        state._rot_hold_start_o = 0.0

    if dx != 0.0 or dy != 0.0 or rot_delta != 0.0:
        x, y, rot, sc = state.get_pose()
        state.set_pose(int(round(x + dx)), int(round(y + dy)), rot + rot_delta, sc)


def handle_tool_fit_mouse(event: pygame.event.Event, state: ToolFitState, *, zoom: float) -> None:
    tx, ty = state._last_tool_screen_pos

    if event.type == pygame.MOUSEBUTTONDOWN and event.button == 1:
        mx, my = event.pos
        if (mx - tx) ** 2 + (my - ty) ** 2 <= (18 ** 2):
            state.dragging = True
            state.drag_start_mouse = (mx, my)
            x, y, _rot, _sc = state.get_pose()
            state.drag_start_pose_xy = (x, y)
            state._msg("Drag: tool")
        return

    if event.type == pygame.MOUSEBUTTONUP and event.button == 1:
        state.dragging = False
        return

    if event.type == pygame.MOUSEMOTION and state.dragging:
        mx, my = event.pos
        sx, sy = state.drag_start_mouse

        z = max(zoom, 0.0001)
        dx = (mx - sx) / z
        dy = (my - sy) / z

        x0, y0 = state.drag_start_pose_xy
        x, y, rot, sc = state.get_pose()
        state.set_pose(int(round(x0 + dx)), int(round(y0 + dy)), rot, sc)
        return


def render_tool_fit(
    *,
    screen: pygame.Surface,
    font: pygame.font.Font,
    font_ui: pygame.font.Font,
    mode_dd: Dropdown,
    model_dd: Dropdown,
    group_dd: Dropdown,
    skin_dd: Dropdown,
    action_dd: Dropdown,
    tool_kind_dd: Dropdown,  # NEW
    tool_dd: Dropdown,
    humanoid_bundle: Dict[str, List[pygame.Surface]],
    zoom: float,
    state: ToolFitState,
    ui_menu_focus: Optional[str],
    action_rel_path: str,
) -> Dict[str, pygame.Rect]:
    # keep clip in sync, but do NOT infer kind
    state.set_clip(action_rel_path or "")

    # apply selected kind from dropdown
    sel_kind = (tool_kind_dd.selected() or "axe").strip()
    sel_kind = sanitize_id(sel_kind) or "axe"
    if sel_kind != sanitize_id(state.tool_kind):
        state.set_tool_kind(sel_kind)

    sel_tool = (tool_dd.selected() or "").strip()
    if sel_tool and sel_tool.lower() != "none":
        if sanitize_id(sel_tool) != sanitize_id(state.tool_id):
            state.set_tool_id(sel_tool)

    screen.fill(UI_BG)

    ui_rects = draw_tool_fit_header(
        screen,
        font_ui,
        mode_dd=mode_dd,
        model_dd=model_dd,
        group_dd=group_dd,
        skin_dd=skin_dd,
        action_dd=action_dd,
        tool_kind_dd=tool_kind_dd,  # NEW
        tool_dd=tool_dd,
        ui_menu_focus=ui_menu_focus,
        state=state,
    )

    w, h = screen.get_width(), screen.get_height()
    cx, cy = w // 2, h // 2 + 40

    if not humanoid_bundle.get(state.direction):
        d = _available_dir(humanoid_bundle)
        if d and d != state.direction:
            state.direction = d
            state.frame_index = 0
            state._msg(f"Auto dir -> {d}")

    frames = humanoid_bundle.get(state.direction) or []
    if not frames:
        screen.blit(font.render("No frames loaded for this direction/action.", True, UI_WARN), (20, 90))
        if state.last_msg:
            screen.blit(font.render(state.last_msg, True, UI_TEXT), (20, 120))
        return ui_rects

    state.frame_index = max(0, min(state.frame_index, len(frames) - 1))

    frame = frames[state.frame_index]
    scaled = pygame.transform.smoothscale(
        frame,
        (max(1, int(frame.get_width() * zoom)), max(1, int(frame.get_height() * zoom))),
    )
    rect = scaled.get_rect(center=(cx, cy))
    screen.blit(scaled, rect)

    pivot_from_bottom = pivot_from_bottom_px_for_kind(state.tool_kind)
    pivot_x = rect.centerx
    pivot_y = rect.bottom - int(pivot_from_bottom * zoom)
    pygame.draw.circle(screen, (255, 0, 0), (pivot_x, pivot_y), 4)

    off_x, off_y, rot_deg, tool_scale = state.get_pose()
    tool_x = pivot_x + int(off_x * zoom)
    tool_y = pivot_y + int(off_y * zoom)

    state._last_tool_screen_pos = (tool_x, tool_y)

    pygame.draw.circle(screen, (120, 200, 255), (tool_x, tool_y), 5)
    pygame.draw.line(screen, (120, 200, 255), (tool_x - 10, tool_y), (tool_x + 10, tool_y), 2)
    pygame.draw.line(screen, (120, 200, 255), (tool_x, tool_y - 10), (tool_x, tool_y + 10), 2)

    # draw tool template (editing greyscale)
    if state.tool_id and state.tool_id != "none":
        tool_path = resolve_tool_overlay_path(
            tool_kind=state.tool_kind,
            tool_id=state.tool_id,
            clip_key=state.clip_key,
            skin="__greyscale__",
            direction=state.direction,
            frame_index=state.frame_index,
        )
        if tool_path.exists():
            try:
                tool_img = pygame.image.load(tool_path).convert_alpha()
                tool_draw = pygame.transform.rotozoom(tool_img, -rot_deg, max(0.001, zoom * tool_scale))
                tool_rect = tool_draw.get_rect(center=(tool_x, tool_y))
                screen.blit(tool_draw, tool_rect)
            except Exception as e:
                state._msg(f"[ERR] Tool load failed: {e}")

    dirty = " *UNSAVED*" if state.dirty else ""
    hud = (
        f"Tool Fit | Kind: {state.tool_kind} | Tool: {state.tool_id} | Clip: {state.clip_key} | "
        f"Dir: {state.direction} | Frame: {state.frame_index+1}/{len(frames)}{dirty} "
        f"| Pose: ({off_x},{off_y}) rot={rot_deg:.1f} sc={tool_scale:.2f} "
        f"| S=Save  P=Import  N=New Tool  K=New Kind  B=Bake  Shift+B=Force Bake  "
        f"Arrows=Move  Shift=10 Ctrl=50  I/O=Rotate  [ ]=Scale  LMB drag=Move  ,/. Frame  1-4 Dir"
    )
    screen.blit(font.render(hud, True, UI_TEXT), (20, h - 30))

    if state.last_msg and (time.time() - state.last_msg_ts) < 5.0:
        screen.blit(font.render(state.last_msg, True, UI_TEXT), (20, h - 55))

    return ui_rects


def handle_tool_fit_click(pos: Tuple[int, int], *, ui_rects: Dict[str, pygame.Rect], state: ToolFitState) -> bool:
    r = ui_rects.get("new_kind")
    if r and r.collidepoint(pos):
        create_new_tool_kind_dialog(state)
        return True

    r = ui_rects.get("new_tool")
    if r and r.collidepoint(pos):
        create_new_tool_variant(state=state)
        return True

    for d in DIRECTIONS:
        r = ui_rects.get(f"dir:{d}")
        if r and r.collidepoint(pos):
            state.direction = d
            state.frame_index = 0
            state._msg(f"Dir -> {d}")
            return True
    return False


def handle_tool_fit_event(event: pygame.event.Event, state: ToolFitState, zoom: float) -> None:
    if event.type != pygame.KEYDOWN:
        return

    mods = pygame.key.get_mods()

    if event.key == pygame.K_s:
        state.save_to_manifest()
        return

    if event.key == pygame.K_p:
        import_png_for_slot(state)
        return

    if event.key == pygame.K_n:
        create_new_tool_variant(state=state)
        return

    # NEW: keyboard shortcut for kind creation
    if event.key == pygame.K_k:
        create_new_tool_kind_dialog(state)
        return

    if event.key == pygame.K_b:
        if not state.tool_id or state.tool_id == "none":
            state._msg("[ERR] Select a Tool first (tool dropdown)")
            return

        force = bool(mods & pygame.KMOD_SHIFT)
        baked, skipped = bake_tool_overlays_for_clip(
            clip_key=state.clip_key,
            tool_kind=state.tool_kind,
            tool_id=state.tool_id,
            force=force,
            bake_missing_only=True,
        )
        state._msg(
            f"Baked tool overlays -> libs/generated_runtime/tools: baked={baked} skipped={skipped} "
            f"(palettes in {tool_palettes_dir_for_clip(clip_key=state.clip_key, tool_kind=state.tool_kind, tool_id=state.tool_id).as_posix()})"
        )
        return

    if event.key == pygame.K_LEFTBRACKET:
        x, y, rot, sc = state.get_pose()
        state.set_pose(x, y, rot, max(0.10, sc - 0.05))
        return

    if event.key == pygame.K_RIGHTBRACKET:
        x, y, rot, sc = state.get_pose()
        state.set_pose(x, y, rot, min(4.00, sc + 0.05))
        return

    if event.key == pygame.K_COMMA:
        state.frame_index = max(0, state.frame_index - 1)
        state._msg(f"Frame -> {state.frame_index+1}")
        return

    if event.key == pygame.K_PERIOD:
        state.frame_index = min(state.frame_index + 1, 10_000)
        state._msg(f"Frame -> {state.frame_index+1}")
        return

    if event.key == pygame.K_1:
        state.direction = "north"
        state.frame_index = 0
        state._msg("Dir -> north")
        return
    if event.key == pygame.K_2:
        state.direction = "east"
        state.frame_index = 0
        state._msg("Dir -> east")
        return
    if event.key == pygame.K_3:
        state.direction = "south"
        state.frame_index = 0
        state._msg("Dir -> south")
        return
    if event.key == pygame.K_4:
        state.direction = "west"
        state.frame_index = 0
        state._msg("Dir -> west")
        return