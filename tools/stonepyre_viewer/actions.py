from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path
from typing import Dict, List, Optional

from .config import DIRECTIONS


@dataclass(frozen=True)
class ActionEntry:
    label: str
    rel_path: Path  # relative to model.base_dir


@dataclass(frozen=True)
class ActionGroups:
    base: List[ActionEntry]
    skills: List[ActionEntry]
    combat: List[ActionEntry]

    def actions_for_group(self, group: str) -> List[ActionEntry]:
        g = group.lower().strip()
        if g == "base":
            return self.base
        if g == "skills":
            return self.skills
        if g == "combat":
            return self.combat
        return self.base


def _list_frames(folder: Path) -> List[Path]:
    if not folder.exists() or not folder.is_dir():
        return []
    return sorted(p for p in folder.iterdir() if p.is_file() and p.suffix.lower() == ".png")


def _has_any_direction_frames(action_dir: Path) -> bool:
    for d in DIRECTIONS:
        if _list_frames(action_dir / d):
            return True
    return False


def _discover_group(root_dir: Path, rel_prefix: Path) -> List[ActionEntry]:
    """
    root_dir is the folder whose immediate children are actions:
      root_dir/walk/north/*.png
      root_dir/idle/south/*.png

    rel_prefix is what to prepend to the action rel path:
      base:   rel_prefix="."
      skills: rel_prefix="skills"
      combat: rel_prefix="combat"
    """
    if not root_dir.exists():
        return []

    out: List[ActionEntry] = []
    for child in sorted([p for p in root_dir.iterdir() if p.is_dir()], key=lambda p: p.name.lower()):
        if _has_any_direction_frames(child):
            rel = (rel_prefix / child.name) if str(rel_prefix) != "." else Path(child.name)
            out.append(ActionEntry(label=child.name, rel_path=rel))
    return out


def discover_action_groups_for_model(base_dir: Path) -> ActionGroups:
    """
    Discovers actions for a model in these places:
      base_dir/<action>/
      base_dir/skills/<action>/
      base_dir/combat/<action>/
    """
    base_actions = _discover_group(base_dir, Path("."))
    skills_actions = _discover_group(base_dir / "skills", Path("skills"))
    combat_actions = _discover_group(base_dir / "combat", Path("combat"))

    return ActionGroups(
        base=base_actions,
        skills=skills_actions,
        combat=combat_actions,
    )
