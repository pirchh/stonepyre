from __future__ import annotations

import re
import shutil
from dataclasses import dataclass
from pathlib import Path
from typing import Optional, Tuple

from PIL import Image

from .config import (
    DIRECTIONS,
    PET_ACTIONS,
    PET_EXPECTED_FRAMES_PER_DIR,
    PETS_ROOT,
    GREYSCALE_OUTPUTS_DIR,
)
from .greyscale import stonepyre_greyscale, scale_nearest

_NAME_SAN = re.compile(r"[^a-zA-Z0-9_]+")

def sanitize_pet_name(raw: str) -> str:
    s = raw.strip().replace(" ", "_")
    s = _NAME_SAN.sub("", s)
    s = re.sub(r"_+", "_", s).strip("_")
    return s.lower()

def create_pet_structure(pet_name: str) -> Path:
    safe = sanitize_pet_name(pet_name)
    if not safe:
        raise ValueError("Pet name is empty after sanitization.")

    root = PETS_ROOT / safe
    for action in PET_ACTIONS:
        for d in DIRECTIONS:
            (root / action / d).mkdir(parents=True, exist_ok=True)
    return root

@dataclass(frozen=True)
class PetImportTarget:
    pet_name: str
    action: str          # "idle" | "walk"
    direction: str       # "north"|"east"|"south"|"west"
    frame_slot: int      # 1..2

def pet_target_path(t: PetImportTarget) -> Path:
    safe = sanitize_pet_name(t.pet_name)
    if t.action not in PET_ACTIONS:
        raise ValueError(f"Invalid pet action: {t.action}")
    if t.direction not in DIRECTIONS:
        raise ValueError(f"Invalid direction: {t.direction}")
    if not (1 <= t.frame_slot <= PET_EXPECTED_FRAMES_PER_DIR):
        raise ValueError("Pet frame slot must be 1..2")

    return PETS_ROOT / safe / t.action / t.direction / f"{t.action}_{t.frame_slot:02d}.png"

def greyscale_output_path(t: PetImportTarget) -> Path:
    safe = sanitize_pet_name(t.pet_name)
    return GREYSCALE_OUTPUTS_DIR / safe / t.action / t.direction / f"{t.action}_{t.frame_slot:02d}.png"

def import_pet_frame(
    src_png: Path,
    target: PetImportTarget,
    *,
    scale: int,
    template_size: Optional[Tuple[int, int]] = None,
    greyscale_after_scale: bool = True,
) -> Tuple[Path, Path]:
    """
    Pipeline:
      1) load exported layer PNG (src)
      2) greyscale quantize (Stonepyre palette)
      3) optional nearest scale
      4) optional re-quantize
      5) save to tools/greyscale_outputs/<pet>/...
      6) copy into libs/templates/pets/<pet>/...
    Returns: (greyscale_path, template_path)
    """
    if not src_png.exists():
        raise FileNotFoundError(src_png)

    create_pet_structure(target.pet_name)

    img = Image.open(src_png).convert("RGBA")

    # 1) Greyscale (and optional fit-to-template)
    img = stonepyre_greyscale(img, target_size=template_size)

    # 2) Upscale pixel-crisp (your workflow)
    if scale and scale > 1:
        img = scale_nearest(img, scale)

    # 3) Optional re-quantize
    if greyscale_after_scale:
        img = stonepyre_greyscale(img, target_size=None)

    # Save greyscale output
    g_path = greyscale_output_path(target)
    g_path.parent.mkdir(parents=True, exist_ok=True)
    img.save(g_path)

    # Copy into templates
    t_path = pet_target_path(target)
    t_path.parent.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(g_path, t_path)

    return g_path, t_path
