#!/usr/bin/env python3
"""
Stonepyre Palette Bundle Viewer + Auto-Baker (pygame)
+ Asset Manager (modern, crisp UI) + Action Creator

VIEWER MODE:
- Plays an action in available directions around pedestals.
- Switch between greyscale + baked palettes.
- Auto-bakes missing outputs (and can force rebake).
- Dropdown UI for action selection:
    - Base (idle/walk/run/etc)
    - Skills (skills/<name>)
    - Combat (combat/<name>)
- Supports actions with partial directions (e.g. east-only woodcutting).

MANAGER MODE:
- Scans all actions (base/skills/combat) and shows completeness per direction.
- Completeness expects slots _01.._04 per direction.
- Expand an action row to see per-direction slot icons.
- Create new actions from UI (+ Add Action) which creates north/east/south/west folders.
- Search (Ctrl+F) + toggle sort (Incomplete First / Complete First)
- FIXED: click expands the correct row (layout-driven hit testing)
- FIXED: scrolling works even with expanded rows (variable height scroll model)
- Improved: manager text readability via manager-specific scaled fonts (not tiny)

Controls:
  TAB          : toggle Viewer / Manager
  SPACE        : pause/resume (Viewer)
  LEFT/RIGHT   : slower/faster FPS (Viewer)
  R            : reload frames + refresh palettes + refresh actions (both)
  B            : force rebake ALL palettes for current action (Viewer)
  [ / ]        : previous / next skin (Viewer)
  G            : quick toggle greyscale (Viewer)
  +/- or wheel : zoom in/out (Viewer)
  Mouse wheel  : scroll list (Manager)
  Ctrl+F       : focus search (Manager)
  ESC/Q        : quit (or closes modal / clears search focus)

Requires:
  pip install pygame pillow
"""

from __future__ import annotations

import json
import sys
import time
import re
from dataclasses import dataclass, field
from pathlib import Path
from typing import Dict, Iterable, List, Tuple, Optional

import pygame
from PIL import Image


# ---------------- Paths ----------------
BASE_DIR = Path(r"C:\Users\ryanj\Development\GameProjects\Stonepyre\libs\templates\humanoid\base_greyscale")
PALETTES_DIR = Path(r"C:\Users\ryanj\Development\GameProjects\Stonepyre\libs\palettes\humanoid")
GENERATED_DIR = Path(r"C:\Users\ryanj\Development\GameProjects\Stonepyre\libs\templates\humanoid\generated")

# ---------------- Content ----------------
DIRECTIONS = ["north", "east", "south", "west"]

EXPECTED_SOURCE_HEXES = [
    "#1E1E1E",
    "#3A3A3A",
    "#555555",
    "#707070",
    "#8C8C8C",
    "#A8A8A8",
]

# ---------------- Window / DPI ----------------
# SCALED can crash on some Windows/driver stacks: we try it, and auto-fallback safely.
FLAGS_PRIMARY = pygame.SCALED | pygame.RESIZABLE
FLAGS_FALLBACK = pygame.RESIZABLE
ACTIVE_FLAGS = FLAGS_PRIMARY

# Force a stable 1920x1080 window
START_W, START_H = 1920, 1080
LOCK_TO_1080P = True

# ---------------- View tuning (Viewer) ----------------
MIN_ZOOM = 0.15
MAX_ZOOM = 1.25
ZOOM_START = 0.28
BASE_SPREAD = 220
SAFE_MARGIN = 90

AUTO_RESIZE_WINDOW = False

# Pedestal tuning
PEDESTAL_GAP = 10
PEDESTAL_W = 140
PEDESTAL_H = 34
SHADOW_H = 28
LABEL_GAP = 12

# ---------------- Manager tuning (Modern UI) ----------------
EXPECTED_FRAMES_PER_DIR = 4  # completeness expects _01.._04
FRAME_SLOT_RE = re.compile(r".*?_(\d{2})\.png$", re.IGNORECASE)

# Render manager into a higher-res surface, then downscale = crisp UI.
# IMPORTANT: use manager-specific fonts too, otherwise the UI gets tiny when downscaled.
MANAGER_SCALE = 2.0

UI_BG = (20, 20, 28)
UI_TOP = (26, 26, 38)
UI_PANEL = (24, 24, 34)
UI_CARD = (32, 32, 46)
UI_CARD_HOVER = (40, 40, 58)
UI_BORDER = (86, 86, 115)

UI_TEXT = (240, 240, 248)
UI_MUTED = (175, 175, 196)
UI_OK = (72, 214, 140)
UI_BAD = (238, 110, 110)
UI_WARN = (255, 205, 105)

MANAGER_ROW_H = 58
MANAGER_PAD = 16
MANAGER_LIST_TOP = 90
MANAGER_LIST_BOTTOM_PAD = 56

DETAIL_H = 150
DETAIL_GAP = 12


def safe_set_mode(size: Tuple[int, int]) -> pygame.Surface:
    """
    Try to create a DPI-friendly SCALED window; if SDL renderer creation fails,
    fall back to a normal RESIZABLE window.
    """
    global ACTIVE_FLAGS
    try:
        return pygame.display.set_mode(size, ACTIVE_FLAGS)
    except pygame.error as e:
        msg = str(e).lower()
        if "failed to create renderer" in msg or "renderer" in msg:
            print(f"[WARN] SCALED renderer failed; falling back to RESIZABLE only. ({e})")
            ACTIVE_FLAGS = FLAGS_FALLBACK
            return pygame.display.set_mode(size, ACTIVE_FLAGS)
        raise


# ---------------- Palette baking helpers ----------------
def hex_to_rgb(hex_str: str) -> Tuple[int, int, int]:
    s = hex_str.strip()
    if not s.startswith("#"):
        raise ValueError(f"Hex color must start with '#': {hex_str}")
    s = s[1:]
    if len(s) != 6:
        raise ValueError(f"Hex color must be 6 digits: {hex_str}")
    return (int(s[0:2], 16), int(s[2:4], 16), int(s[4:6], 16))


def rgb_to_hex(rgb: Tuple[int, int, int]) -> str:
    return "#{:02X}{:02X}{:02X}".format(*rgb)


@dataclass(frozen=True)
class Palette:
    name: str
    mapping: Dict[Tuple[int, int, int], Tuple[int, int, int]]  # src_rgb -> dst_rgb


def load_palette_json(path: Path) -> Palette:
    with path.open("r", encoding="utf-8") as f:
        data = json.load(f)

    if isinstance(data, dict) and "replace" in data:
        name = data.get("name") or path.stem
        replace_dict = data["replace"]
    elif isinstance(data, dict):
        name = path.stem
        replace_dict = data
    else:
        raise ValueError(f"Unexpected JSON structure in {path}")

    normalized: Dict[Tuple[int, int, int], Tuple[int, int, int]] = {}
    for k, v in replace_dict.items():
        if not isinstance(k, str) or not isinstance(v, str):
            raise ValueError(f"Palette entries must be strings: {path} -> {k}:{v}")
        src_hex = k.strip().upper()
        dst_hex = v.strip().upper()
        normalized[hex_to_rgb(src_hex)] = hex_to_rgb(dst_hex)

    return Palette(name=name, mapping=normalized)


def iter_palette_files(palettes_dir: Path) -> Iterable[Path]:
    if not palettes_dir.exists():
        return []
    return sorted(palettes_dir.glob("*.json"))


def bake_image_with_palette(base_img: Image.Image, palette: Palette, *, strict: bool = True) -> Image.Image:
    expected_src_rgbs = {hex_to_rgb(h.upper()) for h in EXPECTED_SOURCE_HEXES}

    if strict:
        missing = expected_src_rgbs - set(palette.mapping.keys())
        extra = set(palette.mapping.keys()) - expected_src_rgbs
        if missing:
            missing_hex = ", ".join(sorted(rgb_to_hex(x) for x in missing))
            raise ValueError(f"Palette '{palette.name}' missing mappings for: {missing_hex}")
        if extra:
            extra_hex = ", ".join(sorted(rgb_to_hex(x) for x in extra))
            raise ValueError(f"Palette '{palette.name}' has unknown source keys: {extra_hex}")

    img = base_img.convert("RGBA")
    pixels = img.load()
    w, h = img.size

    for y in range(h):
        for x in range(w):
            r, g, b, a = pixels[x, y]
            if a == 0:
                continue
            dst = palette.mapping.get((r, g, b))
            if dst is not None:
                dr, dg, db = dst
                pixels[x, y] = (dr, dg, db, a)

    return img


# ---------------- Sprite loading helpers ----------------
def list_frames(folder: Path) -> List[Path]:
    if not folder.exists() or not folder.is_dir():
        return []
    return sorted(p for p in folder.iterdir() if p.is_file() and p.suffix.lower() == ".png")


def pil_to_surface_rgba(pil_img: Image.Image) -> pygame.Surface:
    pil_img = pil_img.convert("RGBA")
    data = pil_img.tobytes()
    surf = pygame.image.frombuffer(data, pil_img.size, "RGBA")
    return surf.convert_alpha()


def load_surfaces_from_paths(paths: List[Path]) -> Tuple[List[pygame.Surface], Tuple[int, int]]:
    surfaces: List[pygame.Surface] = []
    max_w = 0
    max_h = 0
    for p in paths:
        img = Image.open(p).convert("RGBA")
        max_w = max(max_w, img.width)
        max_h = max(max_h, img.height)
        surfaces.append(pil_to_surface_rgba(img))
    return surfaces, (max_w, max_h)


def load_action_base_paths(action_rel: Path) -> Dict[str, List[Path]]:
    """
    Supports partial directions. Only returns directions that exist and contain pngs.
    """
    action_dir = BASE_DIR / action_rel
    if not action_dir.exists():
        raise FileNotFoundError(f"Action folder not found: {action_dir}")

    out: Dict[str, List[Path]] = {}
    for d in DIRECTIONS:
        frames = list_frames(action_dir / d)
        if frames:
            out[d] = frames

    if not out:
        raise FileNotFoundError(f"No direction frames found under: {action_dir}")

    return out


def ensure_baked_for_action(action_rel: Path, palettes: List[Palette], *, force: bool = False) -> None:
    """
    Bake only the directions that exist for this action.
    Output:
      GENERATED_DIR/action_rel/palette/direction/<same filename>.png
    """
    base_paths = load_action_base_paths(action_rel)

    for pal in palettes:
        for d, in_paths in base_paths.items():
            out_dir = GENERATED_DIR / action_rel / pal.name / d
            out_dir.mkdir(parents=True, exist_ok=True)

            for in_path in in_paths:
                out_path = out_dir / in_path.name
                if out_path.exists() and not force:
                    continue
                base_img = Image.open(in_path).convert("RGBA")
                baked = bake_image_with_palette(base_img, pal, strict=True)
                baked.save(out_path)


def load_skin_bundle(action_rel: Path, skin: str) -> Tuple[Dict[str, List[pygame.Surface]], Tuple[int, int]]:
    """
    Returns surfaces_by_dir (partial dirs allowed) and max sprite size.
    """
    max_w = 0
    max_h = 0
    surfaces_by_dir: Dict[str, List[pygame.Surface]] = {}

    if skin == "__greyscale__":
        base_paths = load_action_base_paths(action_rel)
        for d, paths in base_paths.items():
            surfaces, (w, h) = load_surfaces_from_paths(paths)
            surfaces_by_dir[d] = surfaces
            max_w = max(max_w, w)
            max_h = max(max_h, h)
        return surfaces_by_dir, (max_w, max_h)

    for d in DIRECTIONS:
        folder = GENERATED_DIR / action_rel / skin / d
        paths = list_frames(folder)
        if not paths:
            continue
        surfaces, (w, h) = load_surfaces_from_paths(paths)
        surfaces_by_dir[d] = surfaces
        max_w = max(max_w, w)
        max_h = max(max_h, h)

    if not surfaces_by_dir:
        raise FileNotFoundError(f"No baked frames found for skin '{skin}' and action '{action_rel.as_posix()}'")

    return surfaces_by_dir, (max_w, max_h)


# ---------------- Action discovery (grouped) ----------------
@dataclass(frozen=True)
class ActionEntry:
    label: str
    rel_path: Path


@dataclass
class ActionGroups:
    base: List[ActionEntry]
    skills: List[ActionEntry]
    combat: List[ActionEntry]


def _has_any_direction_frames(action_dir: Path) -> bool:
    for d in DIRECTIONS:
        if list_frames(action_dir / d):
            return True
    return False


def discover_group(base_dir: Path, subdir: Optional[str]) -> List[ActionEntry]:
    root = base_dir if subdir is None else (base_dir / subdir)
    if not root.exists():
        return []

    out: List[ActionEntry] = []
    for child in sorted([p for p in root.iterdir() if p.is_dir()], key=lambda p: p.name.lower()):
        if not _has_any_direction_frames(child):
            continue
        rel = child.relative_to(base_dir)
        out.append(ActionEntry(label=child.name, rel_path=rel))
    return out


def discover_actions(base_dir: Path) -> ActionGroups:
    base = discover_group(base_dir, None)
    base = [a for a in base if a.label.lower() not in ("skills", "combat", "movement", "emotes")]

    skills = discover_group(base_dir, "skills")
    combat = discover_group(base_dir, "combat")
    return ActionGroups(base=base, skills=skills, combat=combat)


# ---------------- Manager: file-structure scan ----------------
@dataclass
class DirectionStatus:
    direction: str
    expected: int
    present_slots: Dict[int, Path]  # slot -> file path
    missing_slots: List[int]
    extra_paths: List[Path]
    all_pngs: List[Path]

    @property
    def present_count(self) -> int:
        return len(self.present_slots)

    @property
    def label(self) -> str:
        return f"{self.present_count}/{self.expected}"

    @property
    def complete(self) -> bool:
        return self.present_count == self.expected and len(self.missing_slots) == 0


@dataclass
class ActionStatus:
    group: str
    name: str
    rel_path: Path
    dir_status: Dict[str, DirectionStatus]
    any_pngs: bool

    @property
    def overall_complete(self) -> bool:
        return all(self.dir_status[d].complete for d in DIRECTIONS)

    @property
    def overall_progress(self) -> Tuple[int, int]:
        present = sum(self.dir_status[d].present_count for d in DIRECTIONS)
        expected = sum(self.dir_status[d].expected for d in DIRECTIONS)
        return present, expected


def _parse_slot(p: Path) -> Optional[int]:
    m = FRAME_SLOT_RE.match(p.name)
    if not m:
        return None
    try:
        return int(m.group(1))
    except Exception:
        return None


def scan_action_folder(action_dir: Path, *, expected: int) -> Dict[str, DirectionStatus]:
    out: Dict[str, DirectionStatus] = {}
    for d in DIRECTIONS:
        folder = action_dir / d
        pngs = list_frames(folder)

        present_slots: Dict[int, Path] = {}
        extra_paths: List[Path] = []

        for p in pngs:
            slot = _parse_slot(p)
            if slot is None:
                extra_paths.append(p)
                continue
            if 1 <= slot <= expected:
                if slot not in present_slots:
                    present_slots[slot] = p
                else:
                    extra_paths.append(p)
            else:
                extra_paths.append(p)

        missing_slots = [i for i in range(1, expected + 1) if i not in present_slots]
        out[d] = DirectionStatus(
            direction=d,
            expected=expected,
            present_slots=present_slots,
            missing_slots=missing_slots,
            extra_paths=extra_paths,
            all_pngs=pngs,
        )
    return out


def discover_all_actions_for_manager(base_dir: Path, *, expected: int) -> List[ActionStatus]:
    reserved = {"skills", "combat", "movement", "emotes"}
    statuses: List[ActionStatus] = []

    def add_group(group: str, root: Path, rel_prefix: Path):
        if not root.exists():
            return
        for child in sorted([p for p in root.iterdir() if p.is_dir()], key=lambda p: p.name.lower()):
            name = child.name
            if group == "base" and name.lower() in reserved:
                continue
            rel = (rel_prefix / name) if str(rel_prefix) != "." else Path(name)
            ds = scan_action_folder(child, expected=expected)
            any_pngs = any(len(ds[d].all_pngs) > 0 for d in DIRECTIONS)
            statuses.append(
                ActionStatus(
                    group=group,
                    name=name,
                    rel_path=rel,
                    dir_status=ds,
                    any_pngs=any_pngs,
                )
            )

    add_group("base", base_dir, Path("."))
    add_group("skills", base_dir / "skills", Path("skills"))
    add_group("combat", base_dir / "combat", Path("combat"))

    return statuses


def sanitize_action_name(raw: str) -> str:
    s = raw.strip()
    s = s.replace(" ", "_")
    s = re.sub(r"[^a-zA-Z0-9_]+", "", s)
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


# ---------------- Viewer UI dropdowns ----------------
@dataclass
class Dropdown:
    title: str
    items: List[str]
    selected_index: int = 0
    open: bool = False
    rect: pygame.Rect = field(default_factory=lambda: pygame.Rect(0, 0, 0, 0))

    def selected(self) -> str:
        if not self.items:
            return ""
        return self.items[self.selected_index % len(self.items)]


def draw_dropdown(
    screen: pygame.Surface,
    font: pygame.font.Font,
    dd: Dropdown,
    *,
    x: int,
    y: int,
    w: int,
    item_h: int,
    max_items: int = 14,
) -> Tuple[pygame.Rect, List[Tuple[pygame.Rect, int]]]:
    header_rect = pygame.Rect(x, y, w, item_h)
    dd.rect = header_rect

    pygame.draw.rect(screen, (45, 45, 60), header_rect, border_radius=6)
    pygame.draw.rect(screen, (80, 80, 100), header_rect, width=1, border_radius=6)

    label = f"{dd.title}: {dd.selected() if dd.items else '(none)'}"
    txt = font.render(label, True, (230, 230, 240))
    screen.blit(txt, (x + 10, y + 6))

    items_rects: List[Tuple[pygame.Rect, int]] = []
    if dd.open and dd.items:
        visible = dd.items[:max_items]
        menu_h = item_h * len(visible)
        menu_rect = pygame.Rect(x, y + item_h + 6, w, menu_h)

        pygame.draw.rect(screen, (38, 38, 50), menu_rect, border_radius=8)
        pygame.draw.rect(screen, (80, 80, 100), menu_rect, width=1, border_radius=8)

        for i, name in enumerate(visible):
            r = pygame.Rect(x, menu_rect.y + i * item_h, w, item_h)
            if i == dd.selected_index:
                pygame.draw.rect(screen, (60, 60, 85), r)

            t = font.render(name, True, (220, 220, 230))
            screen.blit(t, (r.x + 10, r.y + 6))
            items_rects.append((r, i))

    return header_rect, items_rects


def point_in_rect(pos: Tuple[int, int], rect: pygame.Rect) -> bool:
    return rect.collidepoint(pos)


def clamp(v: float, lo: float, hi: float) -> float:
    return max(lo, min(hi, v))


# ---- Modern UI primitives (Manager) ----
def draw_round_rect(surf: pygame.Surface, rect: pygame.Rect, color: Tuple[int, int, int], radius: int):
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
):
    if active:
        bg = (54, 54, 78)
    else:
        bg = (42, 42, 62) if hovered else (36, 36, 52)
        if subtle:
            bg = (34, 34, 48) if not hovered else (40, 40, 58)

    draw_round_rect(surf, rect, bg, radius=12)
    pygame.draw.rect(surf, UI_BORDER, rect, width=1, border_radius=12)

    t = font.render(label, True, UI_TEXT)
    surf.blit(t, t.get_rect(center=rect.center))


def draw_pill(surf: pygame.Surface, font: pygame.font.Font, rect: pygame.Rect, label: str, *, color: Tuple[int, int, int]):
    pygame.draw.rect(surf, color, rect, border_radius=999)
    t = font.render(label, True, (20, 20, 20))
    surf.blit(t, t.get_rect(center=rect.center))


def draw_icon_check(surf: pygame.Surface, center: Tuple[int, int], size: int, color: Tuple[int, int, int]):
    cx, cy = center
    s = size
    w = max(2, int(s * 0.18))
    pts = [
        (cx - int(0.48 * s), cy + int(0.05 * s)),
        (cx - int(0.18 * s), cy + int(0.35 * s)),
        (cx + int(0.55 * s), cy - int(0.35 * s)),
    ]
    pygame.draw.lines(surf, color, False, pts, width=w)


def draw_icon_x(surf: pygame.Surface, center: Tuple[int, int], size: int, color: Tuple[int, int, int]):
    cx, cy = center
    s = size
    w = max(2, int(s * 0.18))
    pygame.draw.line(surf, color, (cx - s // 2, cy - s // 2), (cx + s // 2, cy + s // 2), w)
    pygame.draw.line(surf, color, (cx + s // 2, cy - s // 2), (cx - s // 2, cy + s // 2), w)


def draw_icon_folder(surf: pygame.Surface, rect: pygame.Rect, color: Tuple[int, int, int]):
    tab_h = max(6, int(rect.h * 0.35))
    tab_w = max(10, int(rect.w * 0.45))
    body = pygame.Rect(rect.x, rect.y + tab_h // 2, rect.w, rect.h - tab_h // 2)
    tab = pygame.Rect(rect.x + int(rect.w * 0.08), rect.y, tab_w, tab_h)
    pygame.draw.rect(surf, color, body, border_radius=max(4, int(rect.h * 0.22)))
    pygame.draw.rect(surf, color, tab, border_radius=max(4, int(rect.h * 0.22)))


def draw_icon_chevron(surf: pygame.Surface, center: Tuple[int, int], size: int, color: Tuple[int, int, int], down: bool):
    cx, cy = center
    s = size
    w = max(2, int(s * 0.18))
    if down:
        pts = [(cx - s // 2, cy - s // 4), (cx, cy + s // 4), (cx + s // 2, cy - s // 4)]
    else:
        pts = [(cx - s // 4, cy - s // 2), (cx + s // 4, cy), (cx - s // 4, cy + s // 2)]
    pygame.draw.lines(surf, color, False, pts, width=w)


def draw_search_box(
    surf: pygame.Surface,
    font: pygame.font.Font,
    rect: pygame.Rect,
    text: str,
    *,
    focused: bool,
    placeholder: str = "Search actions... (Ctrl+F)",
):
    bg = (18, 18, 26) if not focused else (22, 22, 32)
    draw_round_rect(surf, rect, bg, radius=12)
    pygame.draw.rect(surf, UI_BORDER, rect, width=2 if focused else 1, border_radius=12)

    show = text if text.strip() else placeholder
    col = UI_TEXT if text.strip() else UI_MUTED
    t = font.render(show, True, col)
    surf.blit(t, (rect.x + 14, rect.y + (rect.h - t.get_height()) // 2))

    if focused:
        # caret at end
        caret_on = (int(time.time() * 2) % 2 == 0)
        if caret_on:
            caret_x = rect.x + 14 + t.get_width() + 4
            pygame.draw.line(surf, UI_TEXT, (caret_x, rect.y + 10), (caret_x, rect.bottom - 10), 2)


# ---------------- Viewer helpers ----------------
def get_scaled(surface: pygame.Surface, scale: float, cache: dict) -> pygame.Surface:
    key = (id(surface), round(scale, 4))
    if key in cache:
        return cache[key]
    w = max(1, int(surface.get_width() * scale))
    h = max(1, int(surface.get_height() * scale))
    scaled = pygame.transform.smoothscale(surface, (w, h))
    cache[key] = scaled
    return scaled


# ---------------- Modal input (Manager) ----------------
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


def draw_modal_create_action(
    surf: pygame.Surface,
    font_title: pygame.font.Font,
    font: pygame.font.Font,
    modal: ActionCreateModal,
    mouse_pos_scaled: Tuple[int, int],
) -> None:
    w, h = surf.get_width(), surf.get_height()

    overlay = pygame.Surface((w, h), pygame.SRCALPHA)
    overlay.fill((0, 0, 0, 155))
    surf.blit(overlay, (0, 0))

    card_w = min(740, int(w - 140))
    card_h = 360
    card = pygame.Rect((w - card_w) // 2, (h - card_h) // 2, card_w, card_h)

    draw_round_rect(surf, card, UI_TOP, radius=18)
    pygame.draw.rect(surf, UI_BORDER, card, width=1, border_radius=18)

    title = font_title.render("Create New Action", True, UI_TEXT)
    surf.blit(title, (card.x + 22, card.y + 18))

    subtitle = font.render("Creates north/east/south/west folders automatically.", True, UI_MUTED)
    surf.blit(subtitle, (card.x + 22, card.y + 56))

    bx = card.x + 22
    by = card.y + 108
    bw = 150
    bh = 44
    gap = 12

    r_base = pygame.Rect(bx, by, bw, bh)
    r_skills = pygame.Rect(bx + (bw + gap), by, bw, bh)
    r_combat = pygame.Rect(bx + (bw + gap) * 2, by, bw, bh)

    for r, label, key in [(r_base, "Base", "base"), (r_skills, "Skills", "skills"), (r_combat, "Combat", "combat")]:
        hovered = r.collidepoint(mouse_pos_scaled)
        active = (modal.group == key)
        draw_button(surf, font, r, label, hovered=hovered, active=active)

    iy = by + 74
    input_rect = pygame.Rect(card.x + 22, iy, card_w - 44, 50)
    draw_round_rect(surf, input_rect, (18, 18, 26), radius=12)
    pygame.draw.rect(surf, UI_BORDER, input_rect, width=1, border_radius=12)

    label = font.render("Action name (e.g., woodcutting, mining, 1h_attack)", True, UI_MUTED)
    surf.blit(label, (card.x + 22, iy - 24))

    caret = "|" if (int(time.time() * 2) % 2 == 0) else ""
    value = font.render((modal.raw_name or "") + caret, True, UI_TEXT)
    surf.blit(value, (input_rect.x + 14, input_rect.y + 15))

    if modal.error:
        err = font.render(modal.error, True, UI_BAD)
        surf.blit(err, (card.x + 22, input_rect.bottom + 12))

    close_rect = pygame.Rect(card.right - 46, card.y + 16, 30, 30)
    draw_round_rect(surf, close_rect, (48, 48, 66), radius=10)
    pygame.draw.rect(surf, UI_BORDER, close_rect, width=1, border_radius=10)
    draw_icon_x(surf, close_rect.center, 14, UI_TEXT)

    create_rect = pygame.Rect(card.right - 190, card.bottom - 70, 168, 48)
    draw_button(surf, font, create_rect, "Create", hovered=create_rect.collidepoint(mouse_pos_scaled))

    hint = font.render("Enter = Create • Esc = Close", True, UI_MUTED)
    surf.blit(hint, (card.x + 22, card.bottom - 52))


def main() -> None:
    pygame.init()
    pygame.display.set_caption("Stonepyre Palette Bundle Viewer")

    screen = safe_set_mode((START_W, START_H))

    # palettes
    palette_files = list(iter_palette_files(PALETTES_DIR))
    palettes: List[Palette] = []
    for pf in palette_files:
        try:
            palettes.append(load_palette_json(pf))
        except Exception as e:
            print(f"[WARN] Skipping palette {pf.name}: {e}")

    skins: List[str] = ["__greyscale__"] + [p.name for p in palettes]
    skin_index = 0

    # grouped actions (viewer dropdowns)
    groups = discover_actions(BASE_DIR)
    dd_base = Dropdown("Base", [a.label for a in groups.base], selected_index=0)
    dd_skills = Dropdown("Skills", [a.label for a in groups.skills], selected_index=0)
    dd_combat = Dropdown("Combat", [a.label for a in groups.combat], selected_index=0)

    # default action (viewer)
    current_action: Optional[ActionEntry] = None
    if groups.base:
        current_action = groups.base[0]
    elif groups.skills:
        current_action = groups.skills[0]
    elif groups.combat:
        current_action = groups.combat[0]

    # bake current
    if current_action and palettes:
        ensure_baked_for_action(current_action.rel_path, palettes, force=False)

    zoom = float(ZOOM_START)
    scale_cache: Dict[Tuple[int, float], pygame.Surface] = {}

    bundle_surfaces: Dict[str, List[pygame.Surface]] = {}
    sprite_max_w, sprite_max_h = (1, 1)
    if current_action:
        bundle_surfaces, (sprite_max_w, sprite_max_h) = load_skin_bundle(current_action.rel_path, skins[skin_index])

    clock = pygame.time.Clock()

    # viewer fonts (normal)
    font = pygame.font.SysFont("Segoe UI", 18)
    font_ui = pygame.font.SysFont("Segoe UI", 20)
    font_ui_big = pygame.font.SysFont("Segoe UI Semibold", 26)
    font_ui_huge = pygame.font.SysFont("Segoe UI Semibold", 34)

    paused = False
    fps = 5
    idx = 0
    last_advance = time.time()

    MODE_VIEWER = "viewer"
    MODE_MANAGER = "manager"
    mode = MODE_VIEWER

    # manager state
    manager_scroll = 0
    manager_expanded: Dict[Tuple[str, str], bool] = {}
    manager_cache: List[ActionStatus] = []
    manager_needs_refresh = True

    # manager search + sort
    manager_filter = ""
    manager_focus_search = False
    manager_incomplete_first = True

    # layout-driven manager caches (critical for click/scroll correctness)
    manager_layout_rows: List[Tuple[ActionStatus, pygame.Rect]] = []
    manager_total_content_h = 0

    # modal
    modal = ActionCreateModal(open=False, group="skills", raw_name="", error="")

    def current_spread() -> int:
        return int(BASE_SPREAD * (zoom / 0.45) ** 1.0)

    def maybe_resize_window():
        nonlocal screen
        if LOCK_TO_1080P:
            if (screen.get_width(), screen.get_height()) != (START_W, START_H):
                screen = safe_set_mode((START_W, START_H))
            return

        if mode != MODE_VIEWER:
            return

        if not AUTO_RESIZE_WINDOW:
            return

        # left here in case you re-enable later
        screen = safe_set_mode((START_W, START_H))

    def draw_pedestal_under(screen_surf: pygame.Surface, rect: pygame.Rect) -> Tuple[int, int]:
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

    def refresh_groups():
        nonlocal groups
        groups = discover_actions(BASE_DIR)
        dd_base.items = [a.label for a in groups.base]
        dd_skills.items = [a.label for a in groups.skills]
        dd_combat.items = [a.label for a in groups.combat]
        dd_base.selected_index = min(dd_base.selected_index, max(0, len(dd_base.items) - 1))
        dd_skills.selected_index = min(dd_skills.selected_index, max(0, len(dd_skills.items) - 1))
        dd_combat.selected_index = min(dd_combat.selected_index, max(0, len(dd_combat.items) - 1))

    def find_action(group_name: str, label: str) -> Optional[ActionEntry]:
        table = {"base": groups.base, "skills": groups.skills, "combat": groups.combat}[group_name]
        return next((a for a in table if a.label == label), None)

    def set_action(group_name: str, label: str, *, force_rebake: bool = False):
        nonlocal current_action, bundle_surfaces, sprite_max_w, sprite_max_h, idx, last_advance
        found = find_action(group_name, label)
        if not found:
            return
        current_action = found

        if palettes:
            ensure_baked_for_action(current_action.rel_path, palettes, force=force_rebake)

        bundle_surfaces, (sprite_max_w, sprite_max_h) = load_skin_bundle(current_action.rel_path, skins[skin_index])
        scale_cache.clear()
        idx = 0
        last_advance = time.time()
        maybe_resize_window()

    def refresh_manager_cache():
        nonlocal manager_cache, manager_needs_refresh
        manager_cache = discover_all_actions_for_manager(BASE_DIR, expected=EXPECTED_FRAMES_PER_DIR)
        manager_needs_refresh = False

    def reload_everything(force_rebake: bool = False):
        nonlocal palettes, skins, skin_index, bundle_surfaces, sprite_max_w, sprite_max_h, idx, last_advance, manager_needs_refresh

        refresh_groups()

        palette_files2 = list(iter_palette_files(PALETTES_DIR))
        palettes = []
        for pf in palette_files2:
            try:
                palettes.append(load_palette_json(pf))
            except Exception as e:
                print(f"[WARN] Skipping palette {pf.name}: {e}")

        skins = ["__greyscale__"] + [p.name for p in palettes]
        skin_index = min(skin_index, len(skins) - 1)

        if current_action and palettes:
            ensure_baked_for_action(current_action.rel_path, palettes, force=force_rebake)

        if current_action:
            bundle_surfaces, (sprite_max_w, sprite_max_h) = load_skin_bundle(current_action.rel_path, skins[skin_index])
            scale_cache.clear()
            idx = 0
            last_advance = time.time()

        manager_needs_refresh = True
        maybe_resize_window()

    def close_all_dropdowns():
        dd_base.open = False
        dd_skills.open = False
        dd_combat.open = False

    def build_manager_display_list() -> List[ActionStatus]:
        if manager_needs_refresh:
            refresh_manager_cache()

        q = manager_filter.strip().lower()
        display = manager_cache
        if q:
            display = [st for st in manager_cache if q in st.name.lower() or q in st.rel_path.as_posix().lower()]

        # sort: incomplete first or complete first
        if manager_incomplete_first:
            display = sorted(display, key=lambda st: (st.overall_complete, st.group, st.name.lower()))
        else:
            display = sorted(display, key=lambda st: (not st.overall_complete, st.group, st.name.lower()))
        return display

    def compute_manager_layout(display_list: List[ActionStatus]) -> Tuple[List[Tuple[ActionStatus, pygame.Rect]], int, int]:
        """
        Layout-driven geometry for drawing + hit-testing + scrolling.
        Returns:
          rows: [(ActionStatus, row_rect_screen_space_without_scroll)]
          total_content_h: height of scrollable content (expanded rows count)
          view_h: visible list viewport height (excluding header)
        """
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

            if manager_expanded.get((st.group, st.name), False):
                total += DETAIL_H + DETAIL_GAP
                y += DETAIL_H + DETAIL_GAP

        view_h = list_rect.h - header_h
        return rows, total, view_h

    def clamp_manager_scroll(display_list: List[ActionStatus]) -> None:
        nonlocal manager_scroll, manager_layout_rows, manager_total_content_h
        rows, total_h, view_h = compute_manager_layout(display_list)
        manager_layout_rows = rows
        manager_total_content_h = total_h
        max_scroll = max(0, total_h - view_h)
        manager_scroll = int(clamp(manager_scroll, 0, max_scroll))

    maybe_resize_window()

    running = True
    while running:
        mouse_pos = pygame.mouse.get_pos()

        # --------- Draw ---------
        if mode == MODE_VIEWER:
            screen.fill((28, 28, 40))
            cx, cy = screen.get_width() // 2, screen.get_height() // 2
            spread = current_spread()

            ui_x, ui_y = 12, 10
            item_h = 28
            gap = 12
            dd_w = 240

            base_x = ui_x
            skills_x = ui_x + dd_w + gap
            combat_x = ui_x + (dd_w + gap) * 2

            draw_dropdown(screen, font_ui, dd_base, x=base_x, y=ui_y, w=dd_w, item_h=item_h)
            draw_dropdown(screen, font_ui, dd_skills, x=skills_x, y=ui_y, w=dd_w, item_h=item_h)
            draw_dropdown(screen, font_ui, dd_combat, x=combat_x, y=ui_y, w=dd_w, item_h=item_h)

            positions = {
                "north": (cx, cy - spread),
                "east": (cx + spread, cy),
                "south": (cx, cy + spread),
                "west": (cx - spread, cy),
            }

            if not current_action:
                t = font_ui_big.render("No playable actions found (need PNG frames).", True, UI_WARN)
                screen.blit(t, t.get_rect(center=(cx, cy)))
            else:
                for d in DIRECTIONS:
                    if d not in bundle_surfaces:
                        px, py = positions[d]
                        missing = font.render(f"{d.upper()} (missing)", True, (230, 120, 120))
                        screen.blit(missing, missing.get_rect(center=(px, py)))
                        continue

                    frames = bundle_surfaces[d]
                    frame = frames[idx % len(frames)]
                    frame = get_scaled(frame, zoom, scale_cache)

                    rect = frame.get_rect(center=positions[d])
                    ped_cx, ped_cy = draw_pedestal_under(screen, rect)
                    screen.blit(frame, rect)

                    label = font.render(d.upper(), True, (200, 200, 210))
                    label_rect = label.get_rect(
                        center=(ped_cx, ped_cy + int(PEDESTAL_H * (zoom / 0.45)) // 2 + LABEL_GAP)
                    )
                    screen.blit(label, label_rect)

                skin_name = "greyscale" if skins[skin_index] == "__greyscale__" else skins[skin_index]
                overlay = (
                    f"[TAB: Manager] ACTION: {current_action.rel_path.as_posix()} | skin: {skin_name} | "
                    f"frame {idx+1} | {fps} FPS | zoom {zoom:.2f} | "
                    "click dropdowns | [] skin | B rebake | R reload | +/- or wheel zoom | ESC quit"
                )
                screen.blit(font.render(overlay, True, (220, 220, 220)), (10, screen.get_height() - 26))

        else:
            # --------- MANAGER (supersampled -> downscale) ---------
            mw, mh = screen.get_width(), screen.get_height()
            rw, rh = int(mw * MANAGER_SCALE), int(mh * MANAGER_SCALE)

            msurf = pygame.Surface((rw, rh), pygame.SRCALPHA)
            msurf.fill(UI_BG)

            def S(v: int) -> int:
                return int(v * MANAGER_SCALE)

            # manager-specific fonts (big + readable after downscale)
            m_font_small = pygame.font.SysFont("Segoe UI", max(14, int(18 * MANAGER_SCALE)))
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

            # top-right buttons (hit test in screen-space)
            tab_w, tab_h, tab_y = 150, 40, 16
            tab_manager = pygame.Rect(mw - tab_w - MANAGER_PAD, tab_y, tab_w, tab_h)
            tab_viewer = pygame.Rect(mw - tab_w * 2 - MANAGER_PAD - 10, tab_y, tab_w, tab_h)
            add_rect = pygame.Rect(mw - tab_w * 3 - MANAGER_PAD - 20, tab_y, 170, tab_h)

            draw_button(
                msurf, m_font_ui,
                pygame.Rect(S(tab_viewer.x), S(tab_viewer.y), S(tab_viewer.w), S(tab_viewer.h)),
                "Viewer",
                hovered=tab_viewer.collidepoint(mouse_pos),
                active=False
            )
            draw_button(
                msurf, m_font_ui,
                pygame.Rect(S(tab_manager.x), S(tab_manager.y), S(tab_manager.w), S(tab_manager.h)),
                "Manager",
                hovered=tab_manager.collidepoint(mouse_pos),
                active=True
            )
            draw_button(
                msurf, m_font_ui,
                pygame.Rect(S(add_rect.x), S(add_rect.y), S(add_rect.w), S(add_rect.h)),
                "+ Add Action",
                hovered=add_rect.collidepoint(mouse_pos)
            )

            # search + sort toggle (screen-space rects)
            search_rect = pygame.Rect(MANAGER_PAD, tab_y, 420, tab_h)
            sort_rect = pygame.Rect(search_rect.right + 10, tab_y, 210, tab_h)

            draw_search_box(
                msurf,
                m_font_ui,
                pygame.Rect(S(search_rect.x), S(search_rect.y), S(search_rect.w), S(search_rect.h)),
                manager_filter,
                focused=manager_focus_search and (not modal.open),
            )

            sort_label = "Incomplete First" if manager_incomplete_first else "Complete First"
            draw_button(
                msurf,
                m_font_ui,
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

            # build display list + clamp scroll based on variable-height layout
            display_list = build_manager_display_list()
            clamp_manager_scroll(display_list)

            # draw rows (using cached layout positions)
            for st, row_base in manager_layout_rows:
                row = row_base.move(0, -manager_scroll)

                # cull
                if row.bottom < list_rect.y + 34:
                    continue
                if row.top > list_rect.bottom - 12:
                    break

                hovered = row.collidepoint(mouse_pos)
                key = (st.group, st.name)
                expanded = manager_expanded.get(key, False)

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
                draw_pill(msurf, m_font_ui, pygame.Rect(S(pill.x), S(pill.y), S(pill.w), S(pill.h)), st.group, color=group_color)

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

            hint = m_font_ui.render(
                "[TAB] Viewer • Click row to expand • Mouse wheel scroll • Ctrl+F search • + Add Action",
                True,
                UI_MUTED,
            )
            msurf.blit(hint, (S(MANAGER_PAD), S(mh - 30)))

            if modal.open:
                modal_mouse_scaled = (int(mouse_pos[0] * MANAGER_SCALE), int(mouse_pos[1] * MANAGER_SCALE))
                draw_modal_create_action(msurf, m_font_ui_big, m_font_ui, modal, modal_mouse_scaled)

            screen.blit(pygame.transform.smoothscale(msurf, (mw, mh)), (0, 0))

        pygame.display.flip()

        # --------- Events ---------
        for event in pygame.event.get():
            if event.type == pygame.QUIT:
                running = False

            if event.type == pygame.VIDEORESIZE:
                if LOCK_TO_1080P:
                    screen = safe_set_mode((START_W, START_H))
                else:
                    screen = safe_set_mode((event.w, event.h))
                manager_needs_refresh = True

            if event.type == pygame.KEYDOWN:
                # universal exits
                if event.key in (pygame.K_ESCAPE, pygame.K_q):
                    if mode == MODE_MANAGER and modal.open:
                        modal.reset()
                    else:
                        # if search is focused, ESC just unfocuses it (unless modal open)
                        if mode == MODE_MANAGER and manager_focus_search:
                            manager_focus_search = False
                        else:
                            running = False

                # toggle mode
                elif event.key == pygame.K_TAB:
                    mode = MODE_MANAGER if mode == MODE_VIEWER else MODE_VIEWER
                    close_all_dropdowns()
                    manager_focus_search = False
                    maybe_resize_window()

                elif event.key == pygame.K_r:
                    reload_everything(force_rebake=False)

                # Ctrl+F focus search (manager)
                if mode == MODE_MANAGER and not modal.open:
                    if (event.key == pygame.K_f) and (event.mod & pygame.KMOD_CTRL):
                        manager_focus_search = True
                        continue

                # modal typing
                if mode == MODE_MANAGER and modal.open:
                    if event.key == pygame.K_RETURN:
                        try:
                            if not modal.raw_name.strip():
                                modal.error = "Please enter an action name."
                            else:
                                create_action_structure(modal.group, modal.raw_name)
                                modal.reset()
                                reload_everything(force_rebake=False)
                                manager_needs_refresh = True
                        except Exception as e:
                            modal.error = f"{e}"
                    elif event.key == pygame.K_BACKSPACE:
                        modal.raw_name = modal.raw_name[:-1]
                    elif event.unicode and len(event.unicode) == 1:
                        if len(modal.raw_name) < 48:
                            modal.raw_name += event.unicode
                    continue

                # search typing (manager)
                if mode == MODE_MANAGER and manager_focus_search and not modal.open:
                    if event.key == pygame.K_RETURN:
                        manager_focus_search = False
                    elif event.key == pygame.K_BACKSPACE:
                        manager_filter = manager_filter[:-1]
                        manager_scroll = 0
                    elif event.unicode and len(event.unicode) == 1:
                        # allow basic printable
                        if 32 <= ord(event.unicode) <= 126 and len(manager_filter) < 64:
                            manager_filter += event.unicode
                            manager_scroll = 0
                    continue

                # viewer-only keys
                if mode == MODE_VIEWER:
                    if event.key == pygame.K_SPACE:
                        paused = not paused
                    elif event.key == pygame.K_LEFT:
                        fps = max(1, fps - 1)
                    elif event.key == pygame.K_RIGHT:
                        fps = min(60, fps + 1)
                    elif event.key in (pygame.K_MINUS, pygame.K_KP_MINUS):
                        zoom = clamp(zoom * 0.9, MIN_ZOOM, MAX_ZOOM)
                        scale_cache.clear()
                        maybe_resize_window()
                    elif event.key in (pygame.K_EQUALS, pygame.K_PLUS, pygame.K_KP_PLUS):
                        zoom = clamp(zoom * 1.1, MIN_ZOOM, MAX_ZOOM)
                        scale_cache.clear()
                        maybe_resize_window()
                    elif event.key == pygame.K_b:
                        reload_everything(force_rebake=True)
                    elif event.key == pygame.K_g:
                        skin_index = 0
                        if current_action:
                            bundle_surfaces, (sprite_max_w, sprite_max_h) = load_skin_bundle(current_action.rel_path, skins[skin_index])
                            scale_cache.clear()
                            maybe_resize_window()
                    elif event.key == pygame.K_LEFTBRACKET:
                        if skins:
                            skin_index = (skin_index - 1) % len(skins)
                            if current_action:
                                bundle_surfaces, (sprite_max_w, sprite_max_h) = load_skin_bundle(current_action.rel_path, skins[skin_index])
                                scale_cache.clear()
                                maybe_resize_window()
                    elif event.key == pygame.K_RIGHTBRACKET:
                        if skins:
                            skin_index = (skin_index + 1) % len(skins)
                            if current_action:
                                bundle_surfaces, (sprite_max_w, sprite_max_h) = load_skin_bundle(current_action.rel_path, skins[skin_index])
                                scale_cache.clear()
                                maybe_resize_window()

            if event.type == pygame.MOUSEWHEEL:
                if mode == MODE_VIEWER:
                    if event.y > 0:
                        zoom = clamp(zoom * 1.1, MIN_ZOOM, MAX_ZOOM)
                    elif event.y < 0:
                        zoom = clamp(zoom * 0.9, MIN_ZOOM, MAX_ZOOM)
                    scale_cache.clear()
                    maybe_resize_window()
                else:
                    if not modal.open:
                        # FIX: variable-height scroll clamp based on actual displayed layout
                        manager_scroll = int(manager_scroll - event.y * 72)
                        display_list = build_manager_display_list()
                        clamp_manager_scroll(display_list)

            if event.type == pygame.MOUSEBUTTONDOWN and event.button == 1:
                mx, my = event.pos

                if mode == MODE_VIEWER:
                    ui_x, ui_y = 12, 10
                    item_h = 28
                    gap = 12
                    dd_w = 240

                    base_x = ui_x
                    skills_x = ui_x + dd_w + gap
                    combat_x = ui_x + (dd_w + gap) * 2

                    base_header = pygame.Rect(base_x, ui_y, dd_w, item_h)
                    skills_header = pygame.Rect(skills_x, ui_y, dd_w, item_h)
                    combat_header = pygame.Rect(combat_x, ui_y, dd_w, item_h)

                    if point_in_rect((mx, my), base_header):
                        dd_base.open = not dd_base.open
                        dd_skills.open = False
                        dd_combat.open = False
                    elif point_in_rect((mx, my), skills_header):
                        dd_skills.open = not dd_skills.open
                        dd_base.open = False
                        dd_combat.open = False
                    elif point_in_rect((mx, my), combat_header):
                        dd_combat.open = not dd_combat.open
                        dd_base.open = False
                        dd_skills.open = False
                    else:
                        clicked_any = False

                        _, base_items = draw_dropdown(screen, font_ui, dd_base, x=base_x, y=ui_y, w=dd_w, item_h=item_h)
                        _, skills_items = draw_dropdown(screen, font_ui, dd_skills, x=skills_x, y=ui_y, w=dd_w, item_h=item_h)
                        _, combat_items = draw_dropdown(screen, font_ui, dd_combat, x=combat_x, y=ui_y, w=dd_w, item_h=item_h)

                        if dd_base.open:
                            for r, i in base_items:
                                if point_in_rect((mx, my), r):
                                    dd_base.selected_index = i
                                    dd_base.open = False
                                    set_action("base", dd_base.items[i], force_rebake=False)
                                    clicked_any = True
                                    break

                        if not clicked_any and dd_skills.open:
                            for r, i in skills_items:
                                if point_in_rect((mx, my), r):
                                    dd_skills.selected_index = i
                                    dd_skills.open = False
                                    set_action("skills", dd_skills.items[i], force_rebake=False)
                                    clicked_any = True
                                    break

                        if not clicked_any and dd_combat.open:
                            for r, i in combat_items:
                                if point_in_rect((mx, my), r):
                                    dd_combat.selected_index = i
                                    dd_combat.open = False
                                    set_action("combat", dd_combat.items[i], force_rebake=False)
                                    clicked_any = True
                                    break

                        if not clicked_any:
                            close_all_dropdowns()

                else:
                    # Manager clicks
                    tab_w, tab_h, tab_y = 150, 40, 16
                    tab_viewer = pygame.Rect(screen.get_width() - tab_w * 2 - MANAGER_PAD - 10, tab_y, tab_w, tab_h)
                    add_rect = pygame.Rect(screen.get_width() - tab_w * 3 - MANAGER_PAD - 20, tab_y, 170, tab_h)
                    search_rect = pygame.Rect(MANAGER_PAD, tab_y, 420, tab_h)
                    sort_rect = pygame.Rect(search_rect.right + 10, tab_y, 210, tab_h)

                    if modal.open:
                        # modal controls (screen coords)
                        w, h = screen.get_width(), screen.get_height()
                        card_w = min(740, w - 140)
                        card_h = 360
                        card = pygame.Rect((w - card_w) // 2, (h - card_h) // 2, card_w, card_h)

                        bx = card.x + 22
                        by = card.y + 108
                        bw = 150
                        bh = 44
                        gap = 12
                        r_base = pygame.Rect(bx, by, bw, bh)
                        r_skills = pygame.Rect(bx + (bw + gap), by, bw, bh)
                        r_combat = pygame.Rect(bx + (bw + gap) * 2, by, bw, bh)
                        close_rect = pygame.Rect(card.right - 46, card.y + 16, 30, 30)
                        create_rect = pygame.Rect(card.right - 190, card.bottom - 70, 168, 48)

                        if close_rect.collidepoint((mx, my)):
                            modal.reset()
                        elif create_rect.collidepoint((mx, my)):
                            try:
                                if not modal.raw_name.strip():
                                    modal.error = "Please enter an action name."
                                else:
                                    create_action_structure(modal.group, modal.raw_name)
                                    modal.reset()
                                    reload_everything(force_rebake=False)
                                    manager_needs_refresh = True
                            except Exception as e:
                                modal.error = f"{e}"
                        elif r_base.collidepoint((mx, my)):
                            modal.group = "base"
                        elif r_skills.collidepoint((mx, my)):
                            modal.group = "skills"
                        elif r_combat.collidepoint((mx, my)):
                            modal.group = "combat"
                        continue

                    # top bar controls
                    if tab_viewer.collidepoint((mx, my)):
                        mode = MODE_VIEWER
                        manager_focus_search = False
                        maybe_resize_window()
                        continue

                    if add_rect.collidepoint((mx, my)):
                        modal.open = True
                        modal.error = ""
                        modal.raw_name = ""
                        manager_focus_search = False
                        continue

                    if search_rect.collidepoint((mx, my)):
                        manager_focus_search = True
                        continue
                    else:
                        # click elsewhere unfocuses search (unless you clicked sort)
                        if not sort_rect.collidepoint((mx, my)):
                            manager_focus_search = False

                    if sort_rect.collidepoint((mx, my)):
                        manager_incomplete_first = not manager_incomplete_first
                        manager_scroll = 0
                        continue

                    # row expansion toggle (FIXED: layout-driven hit test)
                    display_list = build_manager_display_list()
                    clamp_manager_scroll(display_list)

                    for st, row_base in manager_layout_rows:
                        row = row_base.move(0, -manager_scroll)
                        if row.collidepoint((mx, my)):
                            k = (st.group, st.name)
                            manager_expanded[k] = not manager_expanded.get(k, False)
                            # after toggle, clamp again so scroll remains valid
                            clamp_manager_scroll(display_list)
                            break

        # --------- Advance frames (viewer) ---------
        now = time.time()
        if mode == MODE_VIEWER and current_action:
            if not paused and (now - last_advance) >= (1.0 / fps):
                idx += 1
                last_advance = now

        clock.tick(120)

    pygame.quit()


if __name__ == "__main__":
    try:
        main()
    except Exception as e:
        print(f"[ERROR] {e}")
        sys.exit(1)
