#!/usr/bin/env python3
"""
Palette baker for Stonepyre humanoid templates.

- Reads a base greyscale (ID map) PNG.
- Reads one or more palette JSON files.
- Replaces exact source hex colors with target hex colors.
- Writes recolored PNGs to an output folder.

Requires:
  pip install pillow
"""

from __future__ import annotations

import json
import os
from dataclasses import dataclass
from pathlib import Path
from typing import Dict, Tuple, Iterable, Optional

from PIL import Image


# --- Your fixed project paths (edit if needed) ---
BASE_IMAGE_PATH = Path(r"C:\Users\ryanj\Development\GameProjects\Stonepyre\libs\templates\humanoid\basegreyscaleidle.png")
PALETTES_DIR    = Path(r"C:\Users\ryanj\Development\GameProjects\Stonepyre\libs\palettes\humanoid")
OUTPUT_DIR      = Path(r"C:\Users\ryanj\Development\GameProjects\Stonepyre\libs\templates\humanoid\generated")


# --- The exact greys you said exist in the base ---
EXPECTED_SOURCE_HEXES = [
    "#1E1E1E",
    "#3A3A3A",
    "#555555",
    "#707070",
    "#8C8C8C",
    "#A8A8A8",
]


def hex_to_rgb(hex_str: str) -> Tuple[int, int, int]:
    s = hex_str.strip()
    if not s.startswith("#"):
        raise ValueError(f"Hex color must start with '#': {hex_str}")
    s = s[1:]
    if len(s) != 6:
        raise ValueError(f"Hex color must be 6 digits: {hex_str}")
    r = int(s[0:2], 16)
    g = int(s[2:4], 16)
    b = int(s[4:6], 16)
    return (r, g, b)


def rgb_to_hex(rgb: Tuple[int, int, int]) -> str:
    return "#{:02X}{:02X}{:02X}".format(*rgb)


@dataclass(frozen=True)
class Palette:
    name: str
    mapping: Dict[Tuple[int, int, int], Tuple[int, int, int]]  # src_rgb -> dst_rgb


def load_palette_json(path: Path) -> Palette:
    """
    JSON format options supported:

    Option A (recommended):
    {
      "name": "white_var01_humanoid",
      "replace": {
        "#1E1E1E": "#101010",
        "#3A3A3A": "#2A2A2A",
        "#555555": "#B07050",
        ...
      }
    }

    Option B (simple):
    {
      "#1E1E1E": "#101010",
      "#3A3A3A": "#2A2A2A",
      ...
    }
    """
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

    # Normalize keys/values to uppercase hex
    normalized: Dict[Tuple[int, int, int], Tuple[int, int, int]] = {}
    for k, v in replace_dict.items():
        if not isinstance(k, str) or not isinstance(v, str):
            raise ValueError(f"Palette entries must be strings: {path} -> {k}:{v}")
        src_hex = k.strip().upper()
        dst_hex = v.strip().upper()
        normalized[hex_to_rgb(src_hex)] = hex_to_rgb(dst_hex)

    return Palette(name=name, mapping=normalized)


def iter_palette_files(palettes_dir: Path, *, only: Optional[Iterable[str]] = None) -> Iterable[Path]:
    """
    Yields palette JSON files.
    If 'only' is provided, it matches by stem (filename without extension).
    """
    if not palettes_dir.exists():
        raise FileNotFoundError(f"Palettes dir does not exist: {palettes_dir}")

    only_set = {s.lower() for s in only} if only else None

    for p in sorted(palettes_dir.glob("*.json")):
        if only_set is None or p.stem.lower() in only_set:
            yield p


def bake_image_with_palette(
    base_img: Image.Image,
    palette: Palette,
    *,
    strict: bool = True,
) -> Image.Image:
    """
    Recolors base_img using palette mapping. Preserves alpha.

    strict=True:
      - Ensures all EXPECTED_SOURCE_HEXES are present in palette mapping.
      - Ensures palette does not contain unknown source keys.
    """
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

    # Apply per-pixel replacement using exact RGB match, preserving alpha
    for y in range(h):
        for x in range(w):
            r, g, b, a = pixels[x, y]
            if a == 0:
                continue  # transparent pixel
            dst = palette.mapping.get((r, g, b))
            if dst is not None:
                dr, dg, db = dst
                pixels[x, y] = (dr, dg, db, a)

    return img


def main() -> None:
    if not BASE_IMAGE_PATH.exists():
        raise FileNotFoundError(f"Base image not found: {BASE_IMAGE_PATH}")

    OUTPUT_DIR.mkdir(parents=True, exist_ok=True)

    base = Image.open(BASE_IMAGE_PATH)

    # Bake ALL palettes in the directory
    palette_files = list(iter_palette_files(PALETTES_DIR))
    if not palette_files:
        raise FileNotFoundError(f"No palette JSON files found in: {PALETTES_DIR}")

    print(f"Base: {BASE_IMAGE_PATH}")
    print(f"Palettes: {PALETTES_DIR} ({len(palette_files)} files)")
    print(f"Output: {OUTPUT_DIR}")
    print("----")

    for pf in palette_files:
        pal = load_palette_json(pf)
        out_img = bake_image_with_palette(base, pal, strict=True)

        out_name = f"{BASE_IMAGE_PATH.stem}__{pal.name}.png"
        out_path = OUTPUT_DIR / out_name
        out_img.save(out_path)

        print(f"✔ baked {pf.name} -> {out_path.name}")

    print("Done.")


if __name__ == "__main__":
    main()
