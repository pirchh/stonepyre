from __future__ import annotations

from pathlib import Path
from typing import Dict, List, Tuple

import pygame
from PIL import Image

from .config import DIRECTIONS


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


def _load_action_base_paths(base_dir: Path, action_rel: Path) -> Dict[str, List[Path]]:
    action_dir = base_dir / action_rel
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


def load_skin_bundle(
    base_dir: Path,
    generated_dir: Path,
    action_rel: Path,
    skin: str,
) -> Tuple[Dict[str, List[pygame.Surface]], Tuple[int, int]]:
    """
    IMPORTANT: Always returns a BRAND NEW dict containing ONLY the directions loaded.
    This prevents "leaking" old directions from prior loads.
    """
    max_w = 0
    max_h = 0
    surfaces_by_dir: Dict[str, List[pygame.Surface]] = {}

    if skin == "__greyscale__":
        base_paths = _load_action_base_paths(base_dir, action_rel)
        for d, paths in base_paths.items():
            surfaces, (w, h) = load_surfaces_from_paths(paths)
            surfaces_by_dir[d] = surfaces
            max_w = max(max_w, w)
            max_h = max(max_h, h)
        return surfaces_by_dir, (max_w, max_h)

    for d in DIRECTIONS:
        folder = generated_dir / action_rel / skin / d
        paths = list_frames(folder)
        if not paths:
            continue
        surfaces, (w, h) = load_surfaces_from_paths(paths)
        surfaces_by_dir[d] = surfaces
        max_w = max(max_w, w)
        max_h = max(max_h, h)

    if not surfaces_by_dir:
        raise FileNotFoundError(
            f"No baked frames found for skin '{skin}' and action '{action_rel.as_posix()}'"
        )

    return surfaces_by_dir, (max_w, max_h)
