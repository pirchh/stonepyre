"""
trimesh-based mesh cleanup and low-poly processing.

Used as a fallback when Blender is not available.
For best low-poly results, prefer the Blender path (blender_lowpoly.py).
"""

from __future__ import annotations

import logging
from pathlib import Path
from typing import Optional

from stonepyre_asset_forge.config import StyleConfig


def trimesh_postprocess(
    mesh_path: Path,
    output_path: Path,
    style: StyleConfig,
    target_tris: int,
    flat_shading: bool,
    logger: logging.Logger,
) -> Path:
    """
    Apply post-processing using trimesh only (no Blender required).

    Steps:
      1. Load mesh
      2. Merge scene into a single mesh
      3. Remove degenerate/duplicate geometry
      4. Decimate toward target_tris
      5. Normalize scale and center origin
      6. Export .glb
    """
    try:
        import trimesh
        import numpy as np
    except ImportError:
        raise ImportError("trimesh and numpy are required. Run: pip install trimesh numpy")

    logger.debug(f"Loading mesh: {mesh_path}")
    loaded = trimesh.load(str(mesh_path), force="scene")

    # Flatten scene → single mesh
    if isinstance(loaded, trimesh.Scene):
        meshes = [g for g in loaded.geometry.values() if isinstance(g, trimesh.Trimesh)]
        if not meshes:
            raise ValueError("No triangle meshes found in the scene.")
        mesh = trimesh.util.concatenate(meshes)
    else:
        mesh = loaded

    logger.debug(f"Loaded mesh: {len(mesh.faces)} triangles, {len(mesh.vertices)} vertices")

    # Basic cleanup
    mesh.remove_degenerate_faces()
    mesh.remove_duplicate_faces()
    mesh.remove_unreferenced_vertices()
    mesh.fill_holes()

    # Decimate toward target triangle count
    if target_tris > 0 and len(mesh.faces) > target_tris:
        ratio = target_tris / len(mesh.faces)
        logger.debug(f"Decimating: {len(mesh.faces)} → ~{target_tris} triangles (ratio={ratio:.3f})")
        try:
            import pymeshlab  # type: ignore
            mesh = _decimate_pymeshlab(mesh, target_tris, logger)
        except ImportError:
            logger.debug("pymeshlab not available, using trimesh simplification")
            mesh = mesh.simplify_quadric_decimation(target_tris)

    # Center origin
    if style.center_origin:
        mesh.vertices -= mesh.centroid

    # Normalize scale
    if style.normalize_scale and style.normalize_height:
        current_height = mesh.bounds[1][2] - mesh.bounds[0][2]
        if current_height > 0:
            scale = style.normalize_height / current_height
            mesh.apply_scale(scale)

    # Shift so the base sits at Z=0
    if style.center_origin:
        min_z = mesh.bounds[0][2]
        mesh.vertices[:, 2] -= min_z

    logger.debug(f"Final mesh: {len(mesh.faces)} triangles")

    # Export
    output_path.parent.mkdir(parents=True, exist_ok=True)
    mesh.export(str(output_path))
    return output_path


def _decimate_pymeshlab(mesh, target_tris: int, logger: logging.Logger):
    """Use pymeshlab for higher-quality decimation if available."""
    import pymeshlab
    import numpy as np
    import trimesh

    ms = pymeshlab.MeshSet()
    m = pymeshlab.Mesh(
        vertex_matrix=mesh.vertices.astype(np.float64),
        face_matrix=mesh.faces.astype(np.int32),
    )
    ms.add_mesh(m)
    ms.meshing_decimation_quadric_edge_collapse(targetfacenum=target_tris, preservenormal=True)
    result = ms.current_mesh()
    return trimesh.Trimesh(
        vertices=result.vertex_matrix(),
        faces=result.face_matrix(),
        process=False,
    )
