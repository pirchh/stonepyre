from __future__ import annotations

import re
from dataclasses import dataclass
from pathlib import Path
from typing import Dict, List

# ---------------- Project root ----------------
PROJECT_ROOT = Path(r"C:\Users\ryanj\Development\GameProjects\Stonepyre")


def _p(*parts: str) -> Path:
    return PROJECT_ROOT.joinpath(*parts)


# ---------------- Tools output roots (legacy - still used by xcf import/export utilities) ----------------
TOOLS_ROOT = _p("tools")
LAYERED_OUTPUTS_DIR = TOOLS_ROOT / "layered_outputs"
GREYSCALE_OUTPUTS_DIR = TOOLS_ROOT / "greyscale_outputs"
RAW_LAYER_EXPORTS_DIR = TOOLS_ROOT / "raw_layer_exports"

# Pet canvas expectation (used during XCF export)
PET_CANVAS_W = 400
PET_CANVAS_H = 600
PET_BOTTOM_PAD = 18

# GIMP console (for headless .xcf layer export)
GIMP_CONSOLE_EXE = Path(r"C:\Program Files\GIMP 2\bin\gimp-console-2.10.exe")

# ---------------- Content ----------------
DIRECTIONS: List[str] = ["north", "east", "south", "west"]

EXPECTED_SOURCE_HEXES = [
    "#1E1E1E",
    "#3A3A3A",
    "#555555",
    "#707070",
    "#8C8C8C",
    "#A8A8A8",
]

# frame slot parser for manager scan (expects _01.._04 etc)
FRAME_SLOT_RE = re.compile(r".*_(\d{2})\.png$", re.IGNORECASE)

# ---------------- Window ----------------
START_W, START_H = 1920, 1080
LOCK_TO_1080P = True

# ---------------- Viewer tuning ----------------
MIN_ZOOM = 0.15
MAX_ZOOM = 1.25
ZOOM_START = 0.28
BASE_SPREAD = 220

PEDESTAL_GAP = 10
PEDESTAL_W = 140
PEDESTAL_H = 34
SHADOW_H = 28
LABEL_GAP = 12

# ---------------- Colors (shared) ----------------
UI_BG = (28, 28, 40)
UI_TEXT = (230, 230, 240)
UI_MUTED = (175, 175, 196)
UI_WARN = (255, 205, 105)

DD_BG = (45, 45, 60)
DD_BORDER = (80, 80, 100)
DD_MENU_BG = (38, 38, 50)
DD_SELECTED_BG = (60, 60, 85)

# ---------------- Manager UI defaults (needed by manager_mode.py) ----------------
MANAGER_SCALE = 1.0
UI_TOP = (24, 24, 34)
UI_PANEL = (32, 32, 46)
UI_CARD = (36, 36, 54)
UI_CARD_HOVER = (42, 42, 64)
UI_BORDER = (72, 72, 96)

UI_OK = (120, 220, 140)
UI_BAD = (240, 110, 120)

MANAGER_ROW_H = 76
MANAGER_PAD = 18
MANAGER_LIST_TOP = 96
MANAGER_LIST_BOTTOM_PAD = 60
DETAIL_H = 140
DETAIL_GAP = 14

# ---------------- Content roots ----------------
# Humanoid actions live here (for Manager completeness scan)
BASE_DIR = _p("libs", "templates", "humanoid", "base_greyscale")
EXPECTED_FRAMES_PER_DIR = 4  # humanoids

# ---------------- Pets: roots + conventions ----------------
PETS_ROOT = _p("libs", "templates", "pets")  # pets/<pet_name>/(idle|walk)/<dir>/...
PETS_GENERATED_ROOT = _p("libs", "templates", "pets", "generated")  # generated/<pet_name>/<action>/<skin>/<dir>/...
PETS_PALETTES_ROOT = _p("libs", "palettes", "pets")  # palettes/pets/<pet_name>/...

PET_ACTIONS = ["idle", "walk"]
PET_EXPECTED_FRAMES_PER_DIR = 2  # slot 01..02

# ---------------- Tools (overlays): unified roots ----------------
# Templates:
#   libs/templates/tools/<tool_kind>/<tool_id>/<clip_dir>/<direction>/01.png
# Manifests:
#   libs/templates/tools/manifests/<tool_kind>.json
# Generated runtime (baked):
#   libs/generated_runtime/tools/<tool_kind>/<tool_id>/<palette>/<clip_dir>/<direction>/01.png
# Tool palettes:
#   libs/palettes/<clip_leaf>/<tool_kind>/<tool_id>/*.json
TOOLS_TEMPLATES_ROOT = _p("libs", "templates", "tools")
TOOLS_MANIFEST_DIR = TOOLS_TEMPLATES_ROOT / "manifests"
TOOLS_GENERATED_RUNTIME_ROOT = _p("libs", "generated_runtime", "tools")
TOOLS_PALETTES_ROOT = _p("libs", "palettes")

DEFAULT_TOOL_KIND = "axe"

# Order matters: first match wins
TOOL_KIND_RULES = [
    ("skills/woodcutting", "axe"),
    ("woodcutting", "axe"),
    ("skills/mining", "pickaxe"),
    ("mining", "pickaxe"),
    ("skills/fishing", "harpoon"),
    ("fishing", "harpoon"),
]


def tool_kind_for_clip(clip_key: str) -> str:
    """
    Single source of truth. Viewer + ToolFit + Bake should call this.
    """
    ck = (clip_key or "").replace("\\", "/").lower()
    for needle, kind in TOOL_KIND_RULES:
        if needle in ck:
            return kind
    return DEFAULT_TOOL_KIND


def discover_tool_kinds() -> List[str]:
    """
    Tool kinds are defined by manifests: libs/templates/tools/manifests/<kind>.json
    """
    out: List[str] = []
    if TOOLS_MANIFEST_DIR.exists():
        for p in sorted(TOOLS_MANIFEST_DIR.glob("*.json"), key=lambda x: x.stem.lower()):
            k = (p.stem or "").strip().lower()
            if k:
                out.append(k)

    if DEFAULT_TOOL_KIND not in out:
        out.insert(0, DEFAULT_TOOL_KIND)

    # de-dupe but preserve order
    seen = set()
    uniq: List[str] = []
    for k in out:
        kk = k.lower()
        if kk in seen:
            continue
        seen.add(kk)
        uniq.append(k)
    return uniq


# ---------------- Models ----------------
@dataclass(frozen=True)
class ModelSpec:
    key: str
    label: str
    base_dir: Path
    palettes_dir: Path
    generated_dir: Path
    kind: str  # "humanoid" | "pet"


def discover_models() -> Dict[str, ModelSpec]:
    """
    Humanoid is fixed; pets are discovered by scanning libs/templates/pets/<pet_name>.
    """
    models: Dict[str, ModelSpec] = {}

    models["humanoid"] = ModelSpec(
        key="humanoid",
        label="Humanoid",
        kind="humanoid",
        base_dir=_p("libs", "templates", "humanoid", "base_greyscale"),
        palettes_dir=_p("libs", "palettes", "humanoid"),
        generated_dir=_p("libs", "templates", "humanoid", "generated"),
    )

    if PETS_ROOT.exists():
        for pet_dir in sorted([p for p in PETS_ROOT.iterdir() if p.is_dir()], key=lambda p: p.name.lower()):
            pet_name = pet_dir.name
            if pet_name.lower() == "generated":
                continue

            models[f"pet:{pet_name}"] = ModelSpec(
                key=f"pet:{pet_name}",
                label=f"Pet: {pet_name}",
                kind="pet",
                base_dir=pet_dir,
                palettes_dir=PETS_PALETTES_ROOT / pet_name,
                generated_dir=PETS_GENERATED_ROOT / pet_name,
            )

    return models