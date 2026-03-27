# tools/stonepyre_viewer/manager_scan.py
from __future__ import annotations

import re
from dataclasses import dataclass
from pathlib import Path
from typing import Dict, List, Set

from .config import DIRECTIONS


# Match *_01.png, *_02.png ... anywhere in the filename (stem)
# Example: walk_01.png, woodcutting_north_03.png, attack-04.png
FRAME_SLOT_RE = re.compile(r"(?:^|[_\-])(\d{2})(?:$|[_\-])")


@dataclass(frozen=True)
class DirStatus:
    present_slots: Set[int]
    expected: int

    @property
    def present_count(self) -> int:
        return len(self.present_slots)

    @property
    def complete(self) -> bool:
        return self.present_count >= self.expected

    @property
    def label(self) -> str:
        # renders like 4/4, 1/4, 0/4
        return f"{self.present_count}/{self.expected}"


@dataclass(frozen=True)
class ActionStatus:
    group: str              # "base" | "skills" | "combat"
    name: str               # action folder name
    rel_path: Path          # relative path from scan root
    dir_status: Dict[str, DirStatus]

    @property
    def overall_progress(self) -> tuple[int, int]:
        present = sum(self.dir_status[d].present_count for d in DIRECTIONS)
        expected = sum(self.dir_status[d].expected for d in DIRECTIONS)
        return present, expected

    @property
    def overall_complete(self) -> bool:
        return all(self.dir_status[d].complete for d in DIRECTIONS)


def _extract_slot_from_filename(p: Path) -> int | None:
    """
    Pulls '01'..'99' from the filename stem.
    Returns int slot or None if not found.
    """
    stem = p.stem.lower().replace(" ", "_")
    # check trailing token first (most common)
    m = re.search(r"(\d{2})$", stem)
    if m:
        try:
            return int(m.group(1))
        except Exception:
            return None

    # fallback: tokenized match
    m = FRAME_SLOT_RE.search(f"_{stem}_")
    if not m:
        return None
    try:
        return int(m.group(1))
    except Exception:
        return None


def _scan_action_dir(action_dir: Path, expected: int) -> Dict[str, DirStatus]:
    """
    action_dir contains direction subfolders:
      <action>/north/*.png
      <action>/east/*.png
      ...
    """
    out: Dict[str, DirStatus] = {}
    for d in DIRECTIONS:
        slots: Set[int] = set()
        ddir = action_dir / d
        if ddir.exists() and ddir.is_dir():
            for fp in ddir.glob("*.png"):
                slot = _extract_slot_from_filename(fp)
                if slot is not None:
                    slots.add(slot)
        out[d] = DirStatus(present_slots=slots, expected=expected)
    return out


def discover_all_actions_for_manager(scan_root: Path, *, expected: int) -> List[ActionStatus]:
    """
    Scans:
      scan_root/<action>/<dir>/*.png                        -> group 'base'
      scan_root/skills/<action>/<dir>/*.png                 -> group 'skills'
      scan_root/combat/<action>/<dir>/*.png                 -> group 'combat'
    """
    results: List[ActionStatus] = []

    def add_group(group: str, group_dir: Path):
        if not group_dir.exists():
            return
        for action_dir in sorted([p for p in group_dir.iterdir() if p.is_dir()], key=lambda p: p.name.lower()):
            name = action_dir.name
            dir_status = _scan_action_dir(action_dir, expected=expected)
            rel_path = action_dir.relative_to(scan_root)
            results.append(ActionStatus(group=group, name=name, rel_path=rel_path, dir_status=dir_status))

    # base actions are direct children of scan_root (excluding skills/combat folders)
    if scan_root.exists():
        for action_dir in sorted([p for p in scan_root.iterdir() if p.is_dir()], key=lambda p: p.name.lower()):
            if action_dir.name.lower() in ("skills", "combat"):
                continue
            name = action_dir.name
            dir_status = _scan_action_dir(action_dir, expected=expected)
            rel_path = action_dir.relative_to(scan_root)
            results.append(ActionStatus(group="base", name=name, rel_path=rel_path, dir_status=dir_status))

    add_group("skills", scan_root / "skills")
    add_group("combat", scan_root / "combat")

    return results
