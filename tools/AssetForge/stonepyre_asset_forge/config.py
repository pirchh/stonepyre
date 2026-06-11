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
    smooth_iterations: int = 1              # geometry smooth passes after remesh (more = rounder canopy)
    smooth_factor: float = 0.5             # smooth strength per pass (0.0–1.0)
    spike_ar_threshold: float = 7.0        # aspect-ratio cutoff for shard removal (lower = more aggressive)
    trunk_base_ratio: float = 0.08         # bottom X fraction of tree always painted trunk colour
    trunk_radius_frac: float = 0.20        # trunk column radius as fraction of mesh max XY extent
    trunk_column_radius: Optional[float] = None  # absolute trunk column radius in Blender metres (overrides trunk_radius_frac)
    use_orig_col_paint: bool = True        # use image-derived colours to guide trunk/canopy classification
    symmetrize_colors: bool = False        # mirror +X face colours onto -X counterparts
    # Tree-specific (used by blender_tree.py, ignored for non-tree styles)
    trunk_height_ratio: float = 0.35       # fraction of AI mesh height to keep as trunk
    canopy_shape: str = "sphere"           # sphere | conical
    canopy_radius: float = 1.2            # canopy sphere radius in Blender metres
    canopy_z_ratio: float = 0.60          # canopy centre Z as fraction of normalize_height
    canopy_height_scale: float = 0.85     # Z scale of canopy (1.0 = sphere, <1 flat, >1 tall)
    canopy_lumpiness: float = 0.35        # displace strength — bumpiness of canopy surface
    canopy_noise_scale: float = 0.45      # noise texture scale — smaller = bigger lumps


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
        smooth_iterations=data.get("smooth_iterations", 1),
        smooth_factor=data.get("smooth_factor", 0.5),
        spike_ar_threshold=data.get("spike_ar_threshold", 7.0),
        trunk_base_ratio=data.get("trunk_base_ratio", 0.08),
        trunk_radius_frac=data.get("trunk_radius_frac", 0.20),
        trunk_column_radius=data.get("trunk_column_radius", None),
        use_orig_col_paint=data.get("use_orig_col_paint", True),
        symmetrize_colors=data.get("symmetrize_colors", False),
        trunk_height_ratio=data.get("trunk_height_ratio", 0.35),
        canopy_shape=data.get("canopy_shape", "sphere"),
        canopy_radius=data.get("canopy_radius", 1.2),
        canopy_z_ratio=data.get("canopy_z_ratio", 0.60),
        canopy_height_scale=data.get("canopy_height_scale", 0.85),
        canopy_lumpiness=data.get("canopy_lumpiness", 0.35),
        canopy_noise_scale=data.get("canopy_noise_scale", 0.45),
    )


def resolve_output_path(
    input_path: Path,
    output_arg: Optional[str],
    fmt: str,
    tree_type: Optional[str] = None,
) -> Path:
    if output_arg:
        p = Path(output_arg)
        p.parent.mkdir(parents=True, exist_ok=True)
        return p
    stem = input_path.stem
    out_dir = Path("output")
    out_dir.mkdir(exist_ok=True)
    suffix = "tree" if tree_type else "lowpoly"
    return out_dir / f"{stem}_{suffix}.{fmt}"


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
