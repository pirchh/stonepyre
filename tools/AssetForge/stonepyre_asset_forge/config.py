"""Config loading and style preset management."""

import json
import os
from dataclasses import dataclass, field
from pathlib import Path
from typing import Optional

_DEFAULT_STYLES_PATH = Path(__file__).parent.parent / "configs" / "styles.json"


@dataclass
class StyleConfig:
    target_tris: int = 1200
    flat_shading: bool = True
    texture_size: int = 256
    simplify_materials: bool = True
    normalize_scale: bool = True
    center_origin: bool = True
    normalize_height: Optional[float] = 1.8
    depth_scale: float = 1.5  # inflate Y-axis depth for flat single-image reconstructions
    remesh: bool = True          # voxel remesh before decimation for clean topology
    remesh_voxel_size: float = 0.05
    symmetrize: bool = False     # mirror +X onto -X for perfect left-right symmetry
    tree_type: Optional[str] = None          # oak/pine/willow/dead/magic/yew — enables vertex colour painting
    generate_stump: bool = False             # also export a stump-only version of the asset
    stump_height_ratio: float = 0.18        # stump height as fraction of normalize_height
    max_footprint: Optional[float] = None   # clamp XY to this width in Blender metres (e.g. 1.2 = 1 tile @ 53.3 scale)
    max_root_radius: Optional[float] = None # pull base verts inside this XY radius (e.g. 0.6 = 1 tile radius @ 53.3 scale)


@dataclass
class RunConfig:
    input_path: Path = field(default_factory=Path)
    output_path: Optional[Path] = None
    style_name: str = "osrs_character"
    style: StyleConfig = field(default_factory=StyleConfig)
    output_format: str = "glb"
    keep_temp: bool = False
    skip_bg_removal: bool = False
    no_texture: bool = False
    flat_shading: bool = True
    target_tris: Optional[int] = None
    seed: Optional[int] = None
    verbose: bool = False
    backend: str = "hunyuan3d"


def load_styles(styles_path: Path = _DEFAULT_STYLES_PATH) -> dict:
    if not styles_path.exists():
        raise FileNotFoundError(f"styles.json not found at {styles_path}")
    with open(styles_path, "r", encoding="utf-8") as f:
        return json.load(f)


def get_style(style_name: str, styles_path: Path = _DEFAULT_STYLES_PATH) -> StyleConfig:
    styles = load_styles(styles_path)
    if style_name not in styles:
        available = ", ".join(styles.keys())
        raise ValueError(f"Unknown style '{style_name}'. Available: {available}")
    data = styles[style_name]
    return StyleConfig(
        target_tris=data.get("target_tris", 1200),
        flat_shading=data.get("flat_shading", True),
        texture_size=data.get("texture_size", 256),
        simplify_materials=data.get("simplify_materials", True),
        normalize_scale=data.get("normalize_scale", True),
        center_origin=data.get("center_origin", True),
        normalize_height=data.get("normalize_height"),
        depth_scale=data.get("depth_scale", 1.5),
        remesh=data.get("remesh", True),
        remesh_voxel_size=data.get("remesh_voxel_size", 0.05),
        symmetrize=data.get("symmetrize", False),
        tree_type=data.get("tree_type", None),
        generate_stump=data.get("generate_stump", False),
        stump_height_ratio=data.get("stump_height_ratio", 0.18),
        max_footprint=data.get("max_footprint", None),
        max_root_radius=data.get("max_root_radius", None),
    )


def resolve_output_path(input_path: Path, output_arg: Optional[str], fmt: str) -> Path:
    if output_arg:
        p = Path(output_arg)
        p.parent.mkdir(parents=True, exist_ok=True)
        return p
    stem = input_path.stem
    out_dir = Path("output")
    out_dir.mkdir(exist_ok=True)
    return out_dir / f"{stem}_lowpoly.{fmt}"


def find_blender() -> Optional[str]:
    """Return the path to the Blender executable, or None if not found."""
    env_path = os.environ.get("BLENDER_PATH")
    if env_path and Path(env_path).exists():
        return env_path

    candidates = [
        r"C:\Program Files\Blender Foundation\Blender 4.2\blender.exe",
        r"C:\Program Files\Blender Foundation\Blender 4.1\blender.exe",
        r"C:\Program Files\Blender Foundation\Blender 4.0\blender.exe",
        r"C:\Program Files\Blender Foundation\Blender 3.6\blender.exe",
        "/usr/bin/blender",
        "/usr/local/bin/blender",
        "/Applications/Blender.app/Contents/MacOS/Blender",
    ]
    for c in candidates:
        if Path(c).exists():
            return c

    import shutil
    found = shutil.which("blender")
    return found if found else None
