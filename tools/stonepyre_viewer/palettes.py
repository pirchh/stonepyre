from __future__ import annotations

import json
from dataclasses import dataclass
from pathlib import Path
from typing import Dict, Iterable, List, Tuple, Optional

from PIL import Image

from .config import EXPECTED_SOURCE_HEXES, DIRECTIONS


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
    source_path: Optional[Path] = None  # used for stale detection (palette json mtime)


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
        normalized[hex_to_rgb(k.strip().upper())] = hex_to_rgb(v.strip().upper())

    return Palette(name=name, mapping=normalized, source_path=path)


def iter_palette_files(palettes_dir: Path) -> Iterable[Path]:
    if not palettes_dir.exists():
        return []
    return sorted(palettes_dir.glob("*.json"))


def _validate_palette_strict(palette: Palette) -> None:
    expected_src_rgbs = {hex_to_rgb(h.upper()) for h in EXPECTED_SOURCE_HEXES}

    missing = expected_src_rgbs - set(palette.mapping.keys())
    extra = set(palette.mapping.keys()) - expected_src_rgbs
    if missing:
        missing_hex = ", ".join(sorted(rgb_to_hex(x) for x in missing))
        raise ValueError(f"Palette '{palette.name}' missing mappings for: {missing_hex}")
    if extra:
        extra_hex = ", ".join(sorted(rgb_to_hex(x) for x in extra))
        raise ValueError(f"Palette '{palette.name}' has unknown source keys: {extra_hex}")


def bake_image_with_palette(base_img: Image.Image, palette: Palette, *, strict: bool = True) -> Image.Image:
    """
    FAST path:
      - converts to RGBA bytes
      - remaps only exact source RGB keys (preserve alpha)
    """
    if strict:
        _validate_palette_strict(palette)

    img = base_img.convert("RGBA")
    w, h = img.size
    raw = img.tobytes()  # RGBA
    out = bytearray(raw)

    # mapping is small (6 keys). We do direct tuple checks.
    # For speed: build a dict from packed int -> packed int.
    # Pack RGB into 24-bit int.
    lut: Dict[int, int] = {}
    for (sr, sg, sb), (dr, dg, db) in palette.mapping.items():
        sk = (sr << 16) | (sg << 8) | sb
        dv = (dr << 16) | (dg << 8) | db
        lut[sk] = dv

    # iterate pixels
    # raw layout: [r,g,b,a, r,g,b,a, ...]
    for i in range(0, len(out), 4):
        a = out[i + 3]
        if a == 0:
            continue
        sk = (out[i] << 16) | (out[i + 1] << 8) | out[i + 2]
        dv = lut.get(sk)
        if dv is None:
            continue
        out[i] = (dv >> 16) & 0xFF
        out[i + 1] = (dv >> 8) & 0xFF
        out[i + 2] = dv & 0xFF

    return Image.frombytes("RGBA", (w, h), bytes(out))


def _list_frames(folder: Path) -> List[Path]:
    if not folder.exists() or not folder.is_dir():
        return []
    return sorted(p for p in folder.iterdir() if p.is_file() and p.suffix.lower() == ".png")


def _load_action_base_paths(base_dir: Path, action_rel: Path) -> Dict[str, List[Path]]:
    action_dir = base_dir / action_rel
    if not action_dir.exists():
        raise FileNotFoundError(f"Action folder not found: {action_dir}")

    out: Dict[str, List[Path]] = {}
    for d in DIRECTIONS:
        frames = _list_frames(action_dir / d)
        if frames:
            out[d] = frames

    if not out:
        raise FileNotFoundError(f"No direction frames found under: {action_dir}")

    return out


def _is_outdated(out_path: Path, *, src_paths: List[Path]) -> bool:
    """
    outdated if:
      - output missing
      - any src mtime newer than output mtime
    """
    if not out_path.exists():
        return True

    try:
        out_m = out_path.stat().st_mtime
    except Exception:
        return True

    for sp in src_paths:
        try:
            if sp.stat().st_mtime > out_m:
                return True
        except Exception:
            # if we can't stat a source, be safe and rebuild
            return True

    return False


def ensure_baked_for_action(
    base_dir: Path,
    generated_dir: Path,
    action_rel: Path,
    palettes: List[Palette],
    *,
    force: bool = False,
    bake_missing_only: bool = True,
) -> None:
    """
    Writes ONLY to generated_dir.

    bake_missing_only=True:
      - bakes missing OR outdated outputs
    force=True:
      - rebakes everything
    """
    base_paths = _load_action_base_paths(base_dir, action_rel)

    for pal in palettes:
        # include palette json mtime in stale detection
        pal_srcs: List[Path] = []
        if pal.source_path is not None:
            pal_srcs.append(pal.source_path)

        for d, in_paths in base_paths.items():
            out_dir = generated_dir / action_rel / pal.name / d
            out_dir.mkdir(parents=True, exist_ok=True)

            for in_path in in_paths:
                out_path = out_dir / in_path.name

                if not force and bake_missing_only:
                    # outdated check uses input png + palette json
                    if not _is_outdated(out_path, src_paths=[in_path, *pal_srcs]):
                        continue
                elif not force and not bake_missing_only:
                    # old behavior: skip if exists
                    if out_path.exists():
                        continue

                base_img = Image.open(in_path).convert("RGBA")
                baked = bake_image_with_palette(base_img, pal, strict=True)
                baked.save(out_path)