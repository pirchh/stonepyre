"""
Mesh post-processing orchestrator.

Decides whether to use Blender (preferred, full-featured) or the
trimesh-only fallback (no Blender required) based on what is available.
"""

from __future__ import annotations

import logging
import subprocess
import sys
import json
from pathlib import Path
from typing import Optional

from stonepyre_asset_forge.config import StyleConfig, find_blender


def run_postprocess(
    raw_mesh_path: Path,
    output_path: Path,
    style: StyleConfig,
    target_tris: Optional[int],
    flat_shading: bool,
    logger: logging.Logger,
) -> Path:
    """
    Apply low-poly post-processing to the raw mesh.

    Tries Blender first. Falls back to trimesh-only if Blender is unavailable.
    Returns the path to the processed mesh.
    """
    effective_tris = target_tris if target_tris is not None else style.target_tris
    effective_flat = flat_shading or style.flat_shading

    blender_exe = find_blender()
    if blender_exe:
        logger.debug(f"Blender found at: {blender_exe}")
        return _run_blender_postprocess(
            blender_exe=blender_exe,
            raw_mesh_path=raw_mesh_path,
            output_path=output_path,
            style=style,
            target_tris=effective_tris,
            flat_shading=effective_flat,
            logger=logger,
        )
    else:
        logger.warning(
            "Blender not found — using trimesh fallback for post-processing.\n"
            "  For best results install Blender and set BLENDER_PATH if needed."
        )
        from stonepyre_asset_forge.mesh.trimesh_cleanup import trimesh_postprocess
        return trimesh_postprocess(
            mesh_path=raw_mesh_path,
            output_path=output_path,
            style=style,
            target_tris=effective_tris,
            flat_shading=effective_flat,
            logger=logger,
        )


def _run_blender_postprocess(
    blender_exe: str,
    raw_mesh_path: Path,
    output_path: Path,
    style: StyleConfig,
    target_tris: int,
    flat_shading: bool,
    logger: logging.Logger,
) -> Path:
    """Invoke the appropriate Blender post-process script via subprocess.

    Tree assets (tree_type set) are routed to blender_tree.py which uses
    procedural canopy generation for clean OSRS-style results.
    All other assets use blender_lowpoly.py.
    """
    blender_script = Path(__file__).parent.parent / "mesh" / "blender_lowpoly.py"
    args_json = json.dumps({
        "input": str(raw_mesh_path),
        "output": str(output_path),
        "target_tris": target_tris,
        "flat_shading": flat_shading,
        "normalize_scale": style.normalize_scale,
        "center_origin": style.center_origin,
        "normalize_height": style.normalize_height,
        "depth_scale": getattr(style, "depth_scale", 1.5),
        "remesh": getattr(style, "remesh", True),
        "remesh_voxel_size": getattr(style, "remesh_voxel_size", 0.05),
        "symmetrize": getattr(style, "symmetrize", False),
        "tree_type": getattr(style, "tree_type", None),
        "generate_stump": getattr(style, "generate_stump", False),
        "stump_height_ratio": getattr(style, "stump_height_ratio", 0.18),
        "max_footprint": getattr(style, "max_footprint", None),
        "max_root_radius": getattr(style, "max_root_radius", None),
        "smooth_iterations": getattr(style, "smooth_iterations", 1),
        "smooth_factor": getattr(style, "smooth_factor", 0.5),
    })

    if not blender_script.exists():
        raise FileNotFoundError(f"Blender script not found: {blender_script}")

    cmd = [
        blender_exe,
        "--background",
        "--python", str(blender_script),
        "--",
        args_json,
    ]

    logger.debug(f"Running Blender: {' '.join(cmd)}")

    result = subprocess.run(
        cmd,
        capture_output=True,
        text=True,
    )

    # Always surface stderr — Blender returns exit code 0 even on unhandled
    # Python exceptions, so we can't rely on returncode alone.
    if result.stderr:
        for line in result.stderr.strip().splitlines():
            logger.warning(f"  blender stderr: {line}")

    if result.returncode != 0:
        logger.error(f"Blender process failed (exit {result.returncode}).")
        raise RuntimeError(
            f"Blender post-processing failed. See logs above.\n"
            f"stdout tail:\n{result.stdout[-2000:]}"
        )

    if not output_path.exists():
        raise RuntimeError(
            f"Blender finished but output file was not created: {output_path}\n"
            f"Blender stdout:\n{result.stdout[-2000:]}\n"
            f"Blender stderr:\n{result.stderr[-2000:]}"
        )

    return output_path
