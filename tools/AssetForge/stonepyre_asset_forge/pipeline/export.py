"""
Final export step.

Converts the processed mesh to the desired output format(s) using trimesh.
The primary format is .glb. Optionally also writes .obj, .fbx, or .stl.
"""

from __future__ import annotations

import logging
from pathlib import Path

_SUPPORTED_FORMATS = {"glb", "obj", "stl", "fbx"}


def export_mesh(
    processed_path: Path,
    output_path: Path,
    fmt: str,
    logger: logging.Logger,
) -> Path:
    """
    Load the processed mesh and export it to the requested format.

    If processed_path is already in the target format this is a fast copy.
    """
    fmt = fmt.lower().lstrip(".")
    if fmt not in _SUPPORTED_FORMATS:
        raise ValueError(f"Unsupported export format '{fmt}'. Supported: {', '.join(_SUPPORTED_FORMATS)}")

    # If Blender already wrote the correct format we may just need to move/copy it
    if processed_path == output_path:
        return output_path

    if processed_path.suffix.lstrip(".").lower() == fmt:
        import shutil
        output_path.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(processed_path, output_path)
        return output_path

    try:
        import trimesh
    except ImportError:
        raise ImportError("trimesh is required for format conversion. Run: pip install trimesh")

    logger.debug(f"Loading mesh for format conversion: {processed_path}")
    scene_or_mesh = trimesh.load(str(processed_path), force="scene")

    output_path.parent.mkdir(parents=True, exist_ok=True)

    if fmt == "glb":
        data = scene_or_mesh.export(file_type="glb")
        output_path.write_bytes(data)
    elif fmt == "obj":
        data = scene_or_mesh.export(file_type="obj")
        if isinstance(data, bytes):
            output_path.write_bytes(data)
        else:
            output_path.write_text(data)
    elif fmt == "stl":
        data = scene_or_mesh.export(file_type="stl")
        output_path.write_bytes(data)
    elif fmt == "fbx":
        # trimesh does not support .fbx export natively; guide the user.
        raise NotImplementedError(
            "FBX export requires Blender or the FBX SDK.\n"
            "To export FBX: open the generated .glb in Blender and export as FBX,\n"
            "or use a converter like assimp."
        )

    return output_path
