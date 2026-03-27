# tools/stonepyre_viewer/import_pet_layers.py
from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path
from typing import Optional, Tuple

from .pet_tools import import_pet_frame, PetImportTarget, sanitize_pet_name


def parse_layer_stem(stem: str) -> Tuple[Optional[str], Optional[str], Optional[int]]:
    """
    Expected: north_walk_01 / south_idle_02
    Returns: (direction, action, slot)
    """
    s = stem.strip().lower().replace("-", "_")
    parts = [p for p in s.split("_") if p]

    direction = None
    action = None
    slot = None

    for d in ("north", "east", "south", "west"):
        if d in parts:
            direction = d
            break

    for a in ("idle", "walk"):
        if a in parts:
            action = a
            break

    # last numeric token becomes slot
    for p in reversed(parts):
        if p.isdigit():
            slot = int(p)
            break

    return direction, action, slot


@dataclass(frozen=True)
class ImportReport:
    pet_name: str
    imported: int
    skipped: int


def import_layer_folder(
    folder: Path,
    *,
    pet_name: str,
    scale: int = 1,
    greyscale_after_scale: bool = True,
) -> ImportReport:
    if not folder.exists() or not folder.is_dir():
        raise FileNotFoundError(folder)

    pet_name = sanitize_pet_name(pet_name)
    imported = 0
    skipped = 0

    for p in sorted(folder.glob("*.png")):
        direction, action, slot = parse_layer_stem(p.stem)
        if direction is None or action is None or slot is None:
            skipped += 1
            continue

        target = PetImportTarget(
            pet_name=pet_name,
            action=action,
            direction=direction,
            frame_slot=slot,
        )

        import_pet_frame(
            p,
            target,
            scale=scale,
            template_size=None,          # you said canvas is already 400x600
            greyscale_after_scale=greyscale_after_scale,
        )
        imported += 1

    return ImportReport(pet_name=pet_name, imported=imported, skipped=skipped)
