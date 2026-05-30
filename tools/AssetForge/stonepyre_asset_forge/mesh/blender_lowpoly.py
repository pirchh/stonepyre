"""
Blender Python post-processing script for low-poly asset generation.

This script is run INSIDE Blender via:
    blender --background --python blender_lowpoly.py -- '{"input": "...", "output": "...", ...}'

It must be self-contained (no imports from stonepyre_asset_forge) because
Blender runs it in its own Python environment.

Args (JSON string passed after --):
    input          : path to the raw mesh file
    output         : path for the exported .glb
    target_tris    : target triangle count (0 = skip decimation)
    flat_shading   : apply flat shading (bool)
    normalize_scale: rescale the object (bool)
    center_origin  : set origin to geometry and move to world origin (bool)
    normalize_height: target height in Blender units (float or null)
    depth_scale    : multiplier for the Y axis (depth) to fatten flat single-image reconstructions (default 1.5)
    tree_type      : palette name for vertex colour painting (oak/pine/willow/dead/magic/yew or null)
    generate_stump : also export a stump version of the tree (bool)
    stump_height_ratio: stump height as fraction of total tree height (default 0.18)
"""

import sys
import json
import os
import random

# ---------------------------------------------------------------------------
# Tree vertex-colour palettes
# ---------------------------------------------------------------------------
TREE_PALETTES = {
    "oak": {
        "trunk":  [(0.29, 0.18, 0.10), (0.38, 0.24, 0.13), (0.32, 0.21, 0.11)],
        "canopy": [(0.17, 0.33, 0.10), (0.22, 0.44, 0.13), (0.28, 0.52, 0.16), (0.15, 0.30, 0.09)],
        "trunk_ratio": 0.46,
        "variation": 0.04,
    },
    "pine": {
        "trunk":  [(0.32, 0.19, 0.09), (0.25, 0.15, 0.07), (0.38, 0.22, 0.10)],
        "canopy": [(0.09, 0.22, 0.07), (0.11, 0.28, 0.09), (0.07, 0.18, 0.06)],
        "trunk_ratio": 0.22,
        "variation": 0.03,
    },
    "willow": {
        "trunk":  [(0.20, 0.12, 0.06), (0.16, 0.10, 0.05), (0.24, 0.15, 0.07)],
        "canopy": [(0.06, 0.15, 0.03), (0.05, 0.12, 0.02), (0.08, 0.18, 0.04), (0.05, 0.14, 0.02)],
        "trunk_ratio": 0.52,
        "variation": 0.03,
    },
    "dead": {
        "trunk":  [(0.38, 0.34, 0.29), (0.30, 0.27, 0.23), (0.44, 0.40, 0.34)],
        "canopy": [(0.35, 0.31, 0.27), (0.28, 0.25, 0.22)],
        "trunk_ratio": 0.55,
        "variation": 0.03,
    },
    "magic": {
        "trunk":  [(0.12, 0.09, 0.22), (0.16, 0.11, 0.30), (0.10, 0.08, 0.18)],
        "canopy": [(0.22, 0.11, 0.48), (0.30, 0.16, 0.62), (0.18, 0.22, 0.55), (0.35, 0.12, 0.70)],
        "trunk_ratio": 0.28,
        "variation": 0.05,
    },
    "yew": {
        "trunk":  [(0.28, 0.20, 0.12), (0.22, 0.16, 0.09), (0.34, 0.24, 0.14)],
        "canopy": [(0.06, 0.20, 0.06), (0.08, 0.25, 0.08), (0.05, 0.18, 0.05)],
        "trunk_ratio": 0.25,
        "variation": 0.03,
    },
    # -----------------------------------------------------------------------
    # Extended production tree palette library
    # -----------------------------------------------------------------------
    "hickory": {
        "trunk":  [(0.30, 0.20, 0.11), (0.25, 0.17, 0.09), (0.36, 0.24, 0.13)],
        "canopy": [(0.18, 0.34, 0.10), (0.22, 0.42, 0.12), (0.15, 0.29, 0.08)],
        "trunk_ratio": 0.40,
        "variation": 0.04,
    },
    "cherry": {
        # Canopy blends light green with pink-tinted tones for a blossom hint
        "trunk":  [(0.18, 0.08, 0.06), (0.22, 0.10, 0.07), (0.15, 0.07, 0.05)],
        "canopy": [(0.22, 0.34, 0.12), (0.28, 0.22, 0.16), (0.20, 0.28, 0.13), (0.32, 0.20, 0.17)],
        "trunk_ratio": 0.38,
        "variation": 0.05,
    },
    "beech": {
        "trunk":  [(0.40, 0.40, 0.36), (0.44, 0.44, 0.40), (0.36, 0.36, 0.32)],
        "canopy": [(0.24, 0.44, 0.13), (0.30, 0.54, 0.17), (0.20, 0.38, 0.11)],
        "trunk_ratio": 0.36,
        "variation": 0.04,
    },
    "maple": {
        "trunk":  [(0.28, 0.18, 0.10), (0.22, 0.14, 0.08), (0.34, 0.22, 0.12)],
        "canopy": [(0.20, 0.38, 0.11), (0.16, 0.32, 0.09), (0.24, 0.44, 0.13)],
        "trunk_ratio": 0.38,
        "variation": 0.04,
    },
    "ash": {
        "trunk":  [(0.28, 0.25, 0.20), (0.22, 0.20, 0.16), (0.34, 0.30, 0.24)],
        "canopy": [(0.12, 0.28, 0.10), (0.10, 0.24, 0.08), (0.14, 0.32, 0.12)],
        "trunk_ratio": 0.40,
        "variation": 0.03,
    },
    "birch": {
        # Consistently pale silver-white trunk with subtle face-to-face variation.
        # Avoid mixing very dark + very light entries — that causes speckle artefacts
        # with random face assignment; birch should read as uniform pale bark.
        "trunk":  [(0.38, 0.38, 0.35), (0.46, 0.46, 0.43), (0.30, 0.30, 0.28)],
        "canopy": [(0.26, 0.50, 0.14), (0.32, 0.60, 0.18), (0.22, 0.44, 0.12)],
        "trunk_ratio": 0.44,
        "variation": 0.04,
    },
    "cedar": {
        "trunk":  [(0.28, 0.14, 0.08), (0.22, 0.11, 0.06), (0.34, 0.17, 0.10)],
        "canopy": [(0.06, 0.16, 0.08), (0.08, 0.20, 0.10), (0.05, 0.14, 0.07)],
        "trunk_ratio": 0.32,
        "variation": 0.03,
    },
    "spruce": {
        "trunk":  [(0.22, 0.14, 0.08), (0.18, 0.11, 0.06), (0.26, 0.16, 0.09)],
        "canopy": [(0.04, 0.12, 0.06), (0.03, 0.10, 0.05), (0.05, 0.14, 0.07)],
        "trunk_ratio": 0.22,
        "variation": 0.02,
    },
    "fir": {
        "trunk":  [(0.24, 0.16, 0.09), (0.20, 0.13, 0.07), (0.28, 0.19, 0.11)],
        "canopy": [(0.06, 0.16, 0.07), (0.05, 0.13, 0.06), (0.07, 0.18, 0.08)],
        "trunk_ratio": 0.18,
        "variation": 0.02,
    },
    "elm": {
        "trunk":  [(0.26, 0.22, 0.16), (0.22, 0.18, 0.13), (0.30, 0.26, 0.19)],
        "canopy": [(0.14, 0.30, 0.09), (0.18, 0.38, 0.11), (0.11, 0.26, 0.07)],
        "trunk_ratio": 0.40,
        "variation": 0.03,
    },
    "poplar": {
        "trunk":  [(0.44, 0.42, 0.36), (0.50, 0.48, 0.42), (0.38, 0.36, 0.30)],
        "canopy": [(0.26, 0.50, 0.15), (0.22, 0.44, 0.13), (0.30, 0.56, 0.17)],
        "trunk_ratio": 0.55,
        "variation": 0.04,
    },
    "sycamore": {
        "trunk":  [(0.44, 0.40, 0.30), (0.50, 0.46, 0.34), (0.38, 0.34, 0.26)],
        "canopy": [(0.20, 0.38, 0.11), (0.16, 0.32, 0.09), (0.24, 0.44, 0.13)],
        "trunk_ratio": 0.36,
        "variation": 0.05,
    },
    "walnut": {
        "trunk":  [(0.14, 0.08, 0.04), (0.10, 0.06, 0.03), (0.18, 0.10, 0.05)],
        "canopy": [(0.10, 0.22, 0.07), (0.08, 0.18, 0.06), (0.12, 0.26, 0.08)],
        "trunk_ratio": 0.40,
        "variation": 0.03,
    },
    "chestnut": {
        "trunk":  [(0.28, 0.16, 0.08), (0.24, 0.13, 0.07), (0.32, 0.19, 0.10)],
        "canopy": [(0.18, 0.36, 0.10), (0.22, 0.44, 0.13), (0.15, 0.30, 0.08)],
        "trunk_ratio": 0.38,
        "variation": 0.04,
    },
    "alder": {
        "trunk":  [(0.18, 0.14, 0.10), (0.14, 0.11, 0.08), (0.22, 0.17, 0.12)],
        "canopy": [(0.10, 0.22, 0.08), (0.08, 0.18, 0.07), (0.12, 0.26, 0.09)],
        "trunk_ratio": 0.38,
        "variation": 0.03,
    },
    "rowan": {
        "trunk":  [(0.32, 0.28, 0.22), (0.28, 0.24, 0.19), (0.36, 0.32, 0.25)],
        "canopy": [(0.18, 0.36, 0.10), (0.22, 0.44, 0.13), (0.15, 0.30, 0.08)],
        "trunk_ratio": 0.38,
        "variation": 0.04,
    },
    "hawthorn": {
        "trunk":  [(0.22, 0.18, 0.14), (0.18, 0.15, 0.11), (0.26, 0.21, 0.17)],
        "canopy": [(0.16, 0.32, 0.10), (0.20, 0.40, 0.12), (0.13, 0.27, 0.08)],
        "trunk_ratio": 0.35,
        "variation": 0.04,
    },
    "cypress": {
        "trunk":  [(0.22, 0.16, 0.10), (0.18, 0.13, 0.08), (0.26, 0.19, 0.12)],
        "canopy": [(0.06, 0.16, 0.08), (0.05, 0.13, 0.07), (0.07, 0.19, 0.09)],
        "trunk_ratio": 0.28,
        "variation": 0.02,
    },
    "mahogany": {
        "trunk":  [(0.22, 0.08, 0.04), (0.18, 0.06, 0.03), (0.26, 0.10, 0.05)],
        "canopy": [(0.12, 0.26, 0.08), (0.10, 0.22, 0.07), (0.14, 0.30, 0.09)],
        "trunk_ratio": 0.40,
        "variation": 0.03,
    },
    "teak": {
        "trunk":  [(0.36, 0.24, 0.12), (0.30, 0.20, 0.10), (0.42, 0.28, 0.14)],
        "canopy": [(0.12, 0.26, 0.08), (0.10, 0.22, 0.07), (0.14, 0.30, 0.09)],
        "trunk_ratio": 0.38,
        "variation": 0.03,
    },
    "ebony": {
        "trunk":  [(0.08, 0.06, 0.05), (0.06, 0.05, 0.04), (0.10, 0.08, 0.06)],
        "canopy": [(0.06, 0.14, 0.04), (0.05, 0.11, 0.03), (0.07, 0.17, 0.05)],
        "trunk_ratio": 0.40,
        "variation": 0.02,
    },
    "ironwood": {
        "trunk":  [(0.28, 0.30, 0.32), (0.24, 0.26, 0.28), (0.32, 0.34, 0.36)],
        "canopy": [(0.08, 0.18, 0.10), (0.06, 0.14, 0.08), (0.10, 0.22, 0.12)],
        "trunk_ratio": 0.42,
        "variation": 0.03,
    },
    "elder": {
        "trunk":  [(0.36, 0.32, 0.26), (0.32, 0.28, 0.22), (0.40, 0.36, 0.30)],
        "canopy": [(0.26, 0.46, 0.12), (0.22, 0.40, 0.10), (0.30, 0.52, 0.14)],
        "trunk_ratio": 0.36,
        "variation": 0.04,
    },
    "redwood": {
        "trunk":  [(0.30, 0.10, 0.05), (0.24, 0.08, 0.04), (0.36, 0.12, 0.06)],
        "canopy": [(0.08, 0.18, 0.06), (0.06, 0.14, 0.05), (0.10, 0.22, 0.07)],
        "trunk_ratio": 0.50,
        "variation": 0.03,
    },
    "sequoia": {
        "trunk":  [(0.26, 0.09, 0.05), (0.22, 0.07, 0.04), (0.30, 0.11, 0.06)],
        "canopy": [(0.08, 0.18, 0.06), (0.06, 0.14, 0.05), (0.10, 0.22, 0.07)],
        "trunk_ratio": 0.55,
        "variation": 0.02,
    },
    "aspen": {
        "trunk":  [(0.60, 0.62, 0.54), (0.55, 0.57, 0.49), (0.65, 0.67, 0.59)],
        "canopy": [(0.30, 0.56, 0.16), (0.26, 0.50, 0.14), (0.34, 0.62, 0.18)],
        "trunk_ratio": 0.44,
        "variation": 0.05,
    },
    "juniper": {
        "trunk":  [(0.20, 0.14, 0.09), (0.16, 0.11, 0.07), (0.24, 0.17, 0.11)],
        "canopy": [(0.06, 0.14, 0.07), (0.05, 0.11, 0.06), (0.07, 0.17, 0.08)],
        "trunk_ratio": 0.28,
        "variation": 0.03,
    },
    "mangrove": {
        "trunk":  [(0.20, 0.15, 0.09), (0.16, 0.12, 0.07), (0.24, 0.18, 0.11)],
        "canopy": [(0.10, 0.22, 0.08), (0.08, 0.18, 0.07), (0.12, 0.26, 0.09)],
        "trunk_ratio": 0.35,
        "variation": 0.03,
    },
    "baobab": {
        "trunk":  [(0.50, 0.42, 0.30), (0.44, 0.36, 0.26), (0.56, 0.48, 0.34)],
        "canopy": [(0.18, 0.36, 0.10), (0.14, 0.30, 0.08), (0.22, 0.42, 0.12)],
        "trunk_ratio": 0.28,
        "variation": 0.04,
    },
    "palm": {
        "trunk":  [(0.42, 0.32, 0.18), (0.36, 0.28, 0.16), (0.48, 0.36, 0.20)],
        "canopy": [(0.20, 0.44, 0.08), (0.16, 0.38, 0.06), (0.24, 0.50, 0.10)],
        "trunk_ratio": 0.72,
        "variation": 0.04,
    },
    # Fantasy trees
    "bloodwood": {
        "trunk":  [(0.22, 0.05, 0.03), (0.18, 0.04, 0.02), (0.26, 0.06, 0.04)],
        "canopy": [(0.20, 0.08, 0.05), (0.24, 0.06, 0.04), (0.16, 0.10, 0.06)],
        "trunk_ratio": 0.38,
        "variation": 0.03,
    },
    "silverleaf": {
        "trunk":  [(0.50, 0.52, 0.55), (0.44, 0.46, 0.50), (0.56, 0.58, 0.62)],
        "canopy": [(0.36, 0.52, 0.30), (0.42, 0.58, 0.36), (0.30, 0.46, 0.26)],
        "trunk_ratio": 0.38,
        "variation": 0.05,
    },
    "moonwillow": {
        "trunk":  [(0.46, 0.48, 0.52), (0.42, 0.44, 0.48), (0.50, 0.52, 0.56)],
        "canopy": [(0.12, 0.28, 0.22), (0.10, 0.24, 0.18), (0.14, 0.32, 0.26)],
        "trunk_ratio": 0.35,
        "variation": 0.04,
    },
    "dragonwood": {
        # Near-black charcoal trunk, orange/amber/gold fire canopy.
        # spike_color="trunk": thin elongated faces AND outer protruding pieces
        # both get painted charcoal so branch sticks read as dark.
        "trunk":  [(0.06, 0.04, 0.02), (0.04, 0.02, 0.01), (0.10, 0.06, 0.03)],
        "canopy": [(0.40, 0.14, 0.02), (0.52, 0.26, 0.03), (0.28, 0.08, 0.01), (0.58, 0.35, 0.05)],
        "trunk_ratio": 0.46,
        "variation": 0.06,
        "spike_color": "trunk",   # paint elongated faces as charcoal branch
        "spike_ar_paint": 2.8,    # aspect-ratio threshold for thin elongated faces
    },
}


def main():
    # Blender passes script args after "--"
    argv = sys.argv
    if "--" not in argv:
        print("ERROR: No arguments passed. Expected JSON after '--'.")
        sys.exit(1)

    raw_args = argv[argv.index("--") + 1:]
    if not raw_args:
        print("ERROR: Empty argument list after '--'.")
        sys.exit(1)

    try:
        args = json.loads(raw_args[0])
    except json.JSONDecodeError as e:
        print(f"ERROR: Could not parse args JSON: {e}")
        sys.exit(1)

    input_path = args.get("input")
    output_path = args.get("output")
    target_tris = int(args.get("target_tris", 1200))
    flat_shading = bool(args.get("flat_shading", True))
    normalize_scale = bool(args.get("normalize_scale", True))
    center_origin = bool(args.get("center_origin", True))
    normalize_height = args.get("normalize_height")  # float or None
    depth_scale = float(args.get("depth_scale", 1.5))  # fatten flat reconstructions
    remesh = bool(args.get("remesh", True))
    remesh_voxel_size = float(args.get("remesh_voxel_size", 0.05))
    symmetrize = bool(args.get("symmetrize", False))
    tree_type = args.get("tree_type")  # str or None
    generate_stump = bool(args.get("generate_stump", False))
    stump_height_ratio = float(args.get("stump_height_ratio", 0.18))
    max_footprint = args.get("max_footprint")  # float in Blender metres, or None
    if max_footprint is not None:
        max_footprint = float(max_footprint)
    max_root_radius = args.get("max_root_radius")  # float in Blender metres, or None
    if max_root_radius is not None:
        max_root_radius = float(max_root_radius)
    smooth_iterations = int(args.get("smooth_iterations", 1))
    smooth_factor = float(args.get("smooth_factor", 0.5))
    spike_ar_threshold = float(args.get("spike_ar_threshold", 7.0))
    trunk_base_ratio = float(args.get("trunk_base_ratio", 0.08))
    trunk_radius_frac = float(args.get("trunk_radius_frac", 0.20))

    if not input_path or not output_path:
        print("ERROR: 'input' and 'output' are required in the args JSON.")
        sys.exit(1)

    if not os.path.exists(input_path):
        print(f"ERROR: Input file not found: {input_path}")
        sys.exit(1)

    import bpy

    # -----------------------------------------------------------------------
    # 1. Reset the scene
    # -----------------------------------------------------------------------
    bpy.ops.wm.read_factory_settings(use_empty=True)

    # -----------------------------------------------------------------------
    # 2. Import the raw mesh
    # -----------------------------------------------------------------------
    ext = os.path.splitext(input_path)[1].lower()
    if ext in (".glb", ".gltf"):
        bpy.ops.import_scene.gltf(filepath=input_path)
    elif ext == ".obj":
        bpy.ops.wm.obj_import(filepath=input_path)
    elif ext == ".fbx":
        bpy.ops.import_scene.fbx(filepath=input_path)
    elif ext == ".stl":
        bpy.ops.import_mesh.stl(filepath=input_path)
    elif ext == ".ply":
        bpy.ops.wm.ply_import(filepath=input_path)
    else:
        print(f"ERROR: Unsupported input format '{ext}'")
        sys.exit(1)

    # -----------------------------------------------------------------------
    # 3. Select and join all mesh objects
    # -----------------------------------------------------------------------
    bpy.ops.object.select_all(action="DESELECT")
    mesh_objects = [o for o in bpy.context.scene.objects if o.type == "MESH"]

    if not mesh_objects:
        print("ERROR: No mesh objects found in the imported file.")
        sys.exit(1)

    for obj in mesh_objects:
        obj.select_set(True)
    bpy.context.view_layer.objects.active = mesh_objects[0]

    if len(mesh_objects) > 1:
        bpy.ops.object.join()

    obj = bpy.context.active_object

    # -----------------------------------------------------------------------
    # 3b. Depth inflation — scale Y axis to fatten flat single-image meshes
    # -----------------------------------------------------------------------
    if depth_scale != 1.0:
        bpy.ops.object.transform_apply(location=False, rotation=True, scale=True)
        obj.scale.y *= depth_scale
        bpy.ops.object.transform_apply(location=False, rotation=False, scale=True)
        print(f"[Blender] Depth scale applied: Y × {depth_scale}")

    # -----------------------------------------------------------------------
    # 3c. Voxel remesh — replaces TripoSR's jagged topology with clean quads
    # -----------------------------------------------------------------------
    if remesh:
        remesh_mod = obj.modifiers.new(name="Remesh", type="REMESH")
        remesh_mod.mode = 'VOXEL'
        remesh_mod.voxel_size = remesh_voxel_size
        remesh_mod.use_smooth_shade = True
        bpy.ops.object.modifier_apply(modifier=remesh_mod.name)
        print(f"[Blender] Voxel remesh applied (voxel size {remesh_voxel_size})")

    # -----------------------------------------------------------------------
    # 3c-post. Reassign a neutral material (voxel remesh wipes vertex colours/UVs)
    # -----------------------------------------------------------------------
    if remesh:
        mat = bpy.data.materials.new(name="AssetMat")
        mat.use_nodes = True
        bsdf = mat.node_tree.nodes.get("Principled BSDF")
        if bsdf:
            bsdf.inputs["Base Color"].default_value = (0.6, 0.55, 0.5, 1.0)  # warm neutral
            bsdf.inputs["Roughness"].default_value = 0.9
        if obj.data.materials:
            obj.data.materials[0] = mat
        else:
            obj.data.materials.append(mat)
        print("[Blender] Neutral material assigned after remesh.")

    # -----------------------------------------------------------------------
    # 3d. Geometry smooth — rounds out voxel faceting; more iterations = rounder
    # -----------------------------------------------------------------------
    smooth_mod = obj.modifiers.new(name="Smooth", type="SMOOTH")
    smooth_mod.factor = smooth_factor
    smooth_mod.iterations = smooth_iterations
    bpy.ops.object.modifier_apply(modifier=smooth_mod.name)
    print(f"[Blender] Geometry smooth applied ({smooth_iterations} iterations, factor {smooth_factor})")

    # -----------------------------------------------------------------------
    # 4. Decimate toward target triangle count
    # -----------------------------------------------------------------------
    if target_tris > 0:
        # Count current tris
        bpy.ops.object.mode_set(mode="OBJECT")
        current_tris = sum(len(p.vertices) - 2 for p in obj.data.polygons)
        print(f"[Blender] Current triangles: {current_tris}")

        if current_tris > target_tris:
            ratio = target_tris / max(current_tris, 1)
            ratio = max(0.001, min(ratio, 1.0))
            print(f"[Blender] Decimating to ratio {ratio:.4f} (target {target_tris} tris)")

            mod = obj.modifiers.new(name="Decimate", type="DECIMATE")
            mod.ratio = ratio
            mod.use_collapse_triangulate = True

            bpy.ops.object.modifier_apply(modifier=mod.name)

            final_tris = sum(len(p.vertices) - 2 for p in obj.data.polygons)
            print(f"[Blender] Triangles after decimation: {final_tris}")

    # -----------------------------------------------------------------------
    # 5. Close holes left by decimation, then triangulate everything uniformly
    # -----------------------------------------------------------------------
    bpy.ops.object.mode_set(mode="EDIT")
    bpy.ops.mesh.select_all(action="SELECT")

    # Merge only truly coincident vertices (tight threshold — avoid collapsing real faces)
    bpy.ops.mesh.remove_doubles(threshold=0.0001)

    # Remove loose disconnected geometry — safe for ears/fingers
    bpy.ops.mesh.delete_loose(use_verts=True, use_edges=True, use_faces=False)

    # First hole-fill pass
    bpy.ops.mesh.select_all(action="DESELECT")
    bpy.ops.mesh.select_non_manifold(extend=False, use_wire=False, use_boundary=True,
                                      use_multi_face=False, use_non_contiguous=False,
                                      use_verts=False)
    bpy.ops.mesh.fill_holes(sides=0)
    # Fix normals on only the newly filled faces (still selected)
    bpy.ops.mesh.normals_make_consistent(inside=False)

    bpy.ops.object.mode_set(mode="OBJECT")

    # Triangulate AFTER filling so patch faces are treated the same as the rest
    tri_mod = obj.modifiers.new(name="Triangulate", type="TRIANGULATE")
    bpy.ops.object.modifier_apply(modifier=tri_mod.name)

    # Second hole-fill pass — catch anything the triangulate reopened
    bpy.ops.object.mode_set(mode="EDIT")
    bpy.ops.mesh.select_all(action="DESELECT")
    bpy.ops.mesh.select_non_manifold(extend=False, use_wire=False, use_boundary=True,
                                      use_multi_face=False, use_non_contiguous=False,
                                      use_verts=False)
    bpy.ops.mesh.fill_holes(sides=0)
    bpy.ops.mesh.normals_make_consistent(inside=False)
    bpy.ops.object.mode_set(mode="OBJECT")

    # BMesh hole-fill pass — catches complex boundary loops the operator misses,
    # then recalculates all normals so no patched face is inside-out.
    import bmesh as _bmesh_holes
    bm_h = _bmesh_holes.new()
    bm_h.from_mesh(obj.data)
    boundary_edges = [e for e in bm_h.edges if e.is_boundary]
    if boundary_edges:
        _bmesh_holes.ops.holes_fill(bm_h, edges=boundary_edges, sides=0)
        _bmesh_holes.ops.recalc_face_normals(bm_h, faces=bm_h.faces)
        print(f"[Blender] BMesh hole-fill: closed {len(boundary_edges)} boundary edges.")
    bm_h.to_mesh(obj.data)
    bm_h.free()
    obj.data.update()

    print("[Blender] Holes filled (2 operator passes + BMesh pass) and normals fixed.")

    # -----------------------------------------------------------------------
    # 6. Symmetrize AFTER decimate so decimation can't re-introduce asymmetry
    # -----------------------------------------------------------------------
    if symmetrize:
        # Center mesh on X=0 so the mirror axis is exact
        bpy.ops.object.transform_apply(location=False, rotation=True, scale=True)
        bpy.ops.object.origin_set(type="ORIGIN_GEOMETRY", center="BOUNDS")
        obj.location.x = 0.0
        bpy.ops.object.transform_apply(location=True, rotation=False, scale=False)

        bpy.ops.object.mode_set(mode="EDIT")
        bpy.ops.mesh.select_all(action="SELECT")
        bpy.ops.mesh.symmetrize(direction='POSITIVE_X', threshold=0.001)

        # Weld only truly coincident centre-seam vertices (tight threshold)
        bpy.ops.mesh.select_all(action="SELECT")
        bpy.ops.mesh.remove_doubles(threshold=0.0001)

        # Fill any remaining open boundary loops
        bpy.ops.mesh.select_all(action="DESELECT")
        bpy.ops.mesh.select_non_manifold(extend=False, use_wire=False, use_boundary=True,
                                          use_multi_face=False, use_non_contiguous=False,
                                          use_verts=False)
        bpy.ops.mesh.fill_holes(sides=0)
        bpy.ops.mesh.normals_make_consistent(inside=False)

        bpy.ops.object.mode_set(mode="OBJECT")
        print("[Blender] Symmetrized, seam welded, holes filled.")

    # -----------------------------------------------------------------------
    # 7. Apply shading
    # Re-select and activate the object explicitly — edit-mode passes above
    # can leave the operator context in an undefined state in Blender 4.x.
    # -----------------------------------------------------------------------
    bpy.ops.object.mode_set(mode="OBJECT")
    bpy.ops.object.select_all(action="DESELECT")
    obj.select_set(True)
    bpy.context.view_layer.objects.active = obj

    if flat_shading:
        bpy.ops.object.shade_flat()
        # customdata_custom_splitnormals_clear requires EDIT mode
        bpy.ops.object.mode_set(mode="EDIT")
        bpy.ops.mesh.customdata_custom_splitnormals_clear()
        bpy.ops.object.mode_set(mode="OBJECT")
        print("[Blender] Flat shading applied.")
    else:
        # shade_smooth_by_angle was added in Blender 4.1.
        # Blender 4.0 and earlier use shade_smooth() + use_auto_smooth.
        # export_normals=True in the GLTF export ensures custom split normals
        # (including auto_smooth) are baked and exported correctly in all versions.
        import math
        if bpy.app.version >= (4, 1, 0):
            bpy.ops.object.shade_smooth_by_angle(angle=math.radians(60))
        else:
            bpy.ops.object.shade_smooth()
            obj.data.use_auto_smooth = True
            obj.data.auto_smooth_angle = math.radians(60)
        print("[Blender] Smooth shading applied (auto-smooth 60°).")

    # -----------------------------------------------------------------------
    # 7b. Weighted normals — large clean faces dominate, kills dark patch artifacts
    # -----------------------------------------------------------------------
    if not flat_shading:
        wn_mod = obj.modifiers.new(name="WeightedNormal", type="WEIGHTED_NORMAL")
        wn_mod.mode = 'FACE_AREA'
        wn_mod.weight = 50
        wn_mod.keep_sharp = False
        bpy.ops.object.modifier_apply(modifier=wn_mod.name)
        print("[Blender] Weighted normals applied.")

    # -----------------------------------------------------------------------
    # 8. Set origin to geometry centre, then move to world origin
    # -----------------------------------------------------------------------
    if center_origin:
        bpy.ops.object.origin_set(type="ORIGIN_GEOMETRY", center="BOUNDS")
        obj.location = (0.0, 0.0, 0.0)

    # -----------------------------------------------------------------------
    # 9. Normalize scale
    # -----------------------------------------------------------------------
    if normalize_scale and normalize_height:
        bpy.ops.object.transform_apply(location=False, rotation=True, scale=True)
        dims = obj.dimensions
        current_height = dims.z
        if current_height > 1e-6:
            scale_factor = float(normalize_height) / current_height
            obj.scale = (scale_factor, scale_factor, scale_factor)
            bpy.ops.object.transform_apply(location=False, rotation=False, scale=True)
            print(f"[Blender] Scale normalised: height {current_height:.3f} → {normalize_height}")

    # Sit the object on the floor (Z min = 0)
    if center_origin:
        bpy.ops.object.transform_apply(location=True, rotation=False, scale=False)
        min_z = min(v.co.z for v in obj.data.vertices)
        obj.location.z -= min_z
        bpy.ops.object.transform_apply(location=True, rotation=False, scale=False)

    # -----------------------------------------------------------------------
    # 9b. Footprint clamp — squeeze XY to fit within one tile, leave Z alone
    #     In Blender: X=right, Y=depth, Z=up. Ground footprint = dims.x × dims.y
    # -----------------------------------------------------------------------
    if max_footprint:
        bpy.ops.object.transform_apply(location=False, rotation=True, scale=True)
        dims = obj.dimensions
        max_xy = max(dims.x, dims.y)
        if max_xy > max_footprint:
            squeeze = max_footprint / max_xy
            obj.scale.x *= squeeze
            obj.scale.y *= squeeze
            bpy.ops.object.transform_apply(location=False, rotation=False, scale=True)
            print(f"[Blender] Footprint clamped: {max_xy:.3f}m → {max_footprint}m "
                  f"(squeeze {squeeze:.3f}x XY, Z unchanged)")

    # -----------------------------------------------------------------------
    # 9c-pre. Root radius clamp — tuck base verts inside one-tile footprint
    #   Affects only the bottom root_z_frac of the tree height.
    #   Vertices beyond max_root_radius in XY are projected inward radially.
    #   Canopy is completely untouched.
    # -----------------------------------------------------------------------
    if max_root_radius and normalize_height:
        import math
        root_z_threshold = float(normalize_height) * 0.25  # bottom 25% of tree
        clamped = 0
        for vert in obj.data.vertices:
            if vert.co.z < root_z_threshold:
                xy_dist = math.sqrt(vert.co.x ** 2 + vert.co.y ** 2)
                if xy_dist > max_root_radius:
                    factor = max_root_radius / xy_dist
                    vert.co.x *= factor
                    vert.co.y *= factor
                    clamped += 1
        if clamped:
            print(f"[Blender] Root radius clamped: {clamped} verts pulled inside "
                  f"{max_root_radius}m radius (bottom {root_z_threshold:.2f}m).")

    # -----------------------------------------------------------------------
    # 9c-pre2. Shard removal — delete small disconnected mesh islands.
    #   AI reconstructions sometimes produce thin floating shards.
    #   We keep only islands that contain at least 2 % of total faces
    #   (or 10 faces, whichever is larger).
    # -----------------------------------------------------------------------
    if tree_type:
        import bmesh as _bmesh
        bm = _bmesh.new()
        bm.from_mesh(obj.data)

        # Pre-compute Z bounds — used by both island removal and spike filter
        _vz = [v.co.z for v in bm.verts]
        min_z_vc  = min(_vz)
        height_vc = max(max(_vz) - min_z_vc, 0.001)

        # Walk every face and group into connected islands
        unvisited = set(bm.faces)
        islands   = []
        while unvisited:
            seed  = next(iter(unvisited))
            stack = [seed]
            isle  = set()
            while stack:
                f = stack.pop()
                if f in isle:
                    continue
                isle.add(f)
                unvisited.discard(f)
                for edge in f.edges:
                    for nbr in edge.link_faces:
                        if nbr not in isle:
                            stack.append(nbr)
            islands.append(isle)

        total_faces  = len(bm.faces)
        min_keep     = max(10, int(total_faces * 0.02))
        shard_faces  = [f for isle in islands if len(isle) < min_keep for f in isle]

        if shard_faces:
            _bmesh.ops.delete(bm, geom=shard_faces, context='FACES')
            print(f"[Blender] Shard removal: deleted {len(shard_faces)} faces "
                  f"across {sum(1 for isle in islands if len(isle) < min_keep)} island(s).")

        # Aspect-ratio filter — catches thin spike faces that are connected to the
        # main mesh (so island size can't detect them).
        # Ratio = longest_edge² / (2 × area). A perfect equilateral triangle ≈ 1.15;
        # a very thin spike can be 20–100+. Threshold of 18 is safe for tree geometry.
        import math as _math_ar
        def _aspect_ratio(f):
            area = f.calc_area()
            if area < 1e-12:
                return float('inf')
            longest = max(e.calc_length() for e in f.edges)
            return (longest * longest) / (2.0 * area)

        # Spike removal — single pass, then a single hole-fill to patch any
        # legitimate trunk/canopy faces that were also caught by the filter.
        # No iteration — avoids the cascade that previously ate trunk geometry.
        spike_faces = [f for f in bm.faces if _aspect_ratio(f) > spike_ar_threshold]
        if spike_faces:
            _bmesh.ops.delete(bm, geom=spike_faces, context='FACES')
            print(f"[Blender] Spike removal: deleted {len(spike_faces)} face(s).")

        # Always fill boundary edges at this stage — the root-radius clamp
        # (which runs before this block) can create new open edges that the
        # earlier hole-fill passes never saw.  This runs whether or not any
        # spikes were deleted.
        final_boundary = [e for e in bm.edges if e.is_boundary]
        if final_boundary:
            _bmesh.ops.holes_fill(bm, edges=final_boundary, sides=0)
            _bmesh.ops.recalc_face_normals(bm, faces=bm.faces)
            print(f"[Blender] Post-clamp hole-fill: closed {len(final_boundary)} boundary edges.")

        bm.to_mesh(obj.data)
        bm.free()
        obj.data.update()

    # -----------------------------------------------------------------------
    # 9c. Vertex colour painting (tree types only)
    # -----------------------------------------------------------------------
    if tree_type:
        palette = TREE_PALETTES.get(tree_type, TREE_PALETTES["oak"])
        trunk_colors  = palette["trunk"]
        canopy_colors = palette["canopy"]
        trunk_ratio   = palette["trunk_ratio"]
        variation     = palette["variation"]

        import math as _math

        mesh = obj.data
        min_z_vc  = min(v.co.z for v in mesh.vertices)
        max_z_vc  = max(v.co.z for v in mesh.vertices)
        height_vc = max(max_z_vc - min_z_vc, 0.001)

        # Estimate trunk radius: trunk is a narrow column at the centre.
        # Hanging pieces (e.g. willow tendrils) extend far in XY even at low Z,
        # so we use XY distance to distinguish them from the true trunk.
        max_xy_vc     = max(_math.sqrt(v.co.x**2 + v.co.y**2) for v in mesh.vertices)
        trunk_radius  = max(max_xy_vc * trunk_radius_frac, 0.05)

        # Optional: pre-compute which faces are "spikes" (thin elongated branches)
        # so their vertex colours can be overridden to trunk/branch colour.
        spike_color_mode  = palette.get("spike_color")
        spike_ar_paint    = float(palette.get("spike_ar_paint", 5.0))
        outer_frac_paint  = palette.get("outer_frac_paint")  # float or None
        face_is_spike     = {}
        if spike_color_mode == "trunk":
            def _face_aspect(poly_):
                verts_ = [mesh.vertices[mesh.loops[li].vertex_index].co
                          for li in poly_.loop_indices]
                if len(verts_) < 3:
                    return 0.0
                a = (verts_[1] - verts_[0]).length
                b = (verts_[2] - verts_[1]).length
                c = (verts_[0] - verts_[2]).length
                longest = max(a, b, c)
                cross   = (verts_[1] - verts_[0]).cross(verts_[2] - verts_[0])
                area    = cross.length * 0.5
                return (longest * longest) / (2.0 * area) if area > 1e-10 else float('inf')

            outer_radius = (max_xy_vc * float(outer_frac_paint)) if outer_frac_paint else None

            for poly_ in mesh.polygons:
                is_high_ar = _face_aspect(poly_) > spike_ar_paint
                is_outer   = False
                if outer_radius is not None:
                    n_verts = len(poly_.loop_indices)
                    cx = sum(mesh.vertices[mesh.loops[li].vertex_index].co.x
                             for li in poly_.loop_indices) / n_verts
                    cy = sum(mesh.vertices[mesh.loops[li].vertex_index].co.y
                             for li in poly_.loop_indices) / n_verts
                    xy_dist_c = _math.sqrt(cx*cx + cy*cy)
                    is_outer  = xy_dist_c > outer_radius
                face_is_spike[poly_.index] = is_high_ar or is_outer

        # Create (or replace) the colour attribute
        if "Col" in mesh.color_attributes:
            mesh.color_attributes.remove(mesh.color_attributes["Col"])
        color_attr = mesh.color_attributes.new(name="Col", type='BYTE_COLOR', domain='CORNER')
        mesh.color_attributes.active_color_index = 0

        for poly in mesh.polygons:
            for loop_idx in poly.loop_indices:
                v_idx   = mesh.loops[loop_idx].vertex_index
                vco     = mesh.vertices[v_idx].co
                z_norm  = (vco.z - min_z_vc) / height_vc
                xy_dist = _math.sqrt(vco.x**2 + vco.y**2)

                # Spike faces (thin elongated branches in canopy) → trunk colour
                if face_is_spike.get(poly.index, False):
                    is_trunk = True
                    base = random.choice(trunk_colors)
                else:
                    # Very base of the tree (bottom trunk_base_ratio %) is always
                    # trunk — catches root bumps that spread beyond trunk_radius.
                    # Above that, require BOTH low Z AND close to the centre axis
                    # so hanging/drooping canopy pieces stay green.
                    is_trunk = z_norm < trunk_base_ratio or (z_norm < trunk_ratio and xy_dist < trunk_radius)
                    base     = random.choice(trunk_colors if is_trunk else canopy_colors)
                var      = random.uniform(-variation, variation)

                if not is_trunk:
                    # Position-based pseudo-noise for canopy texture variation.
                    # Layered sin waves create dappled-light colour bands that
                    # read as texture without needing a UV map.
                    freq  = 4.5
                    noise = (
                        _math.sin(vco.x * freq       ) * _math.cos(vco.y * freq * 1.3) +
                        _math.sin(vco.z * freq * 0.9 ) * _math.cos(vco.x * freq * 0.7)
                    ) * 0.25  # -0.25 … +0.25
                    noise_amp = 0.045
                    r = max(0.0, min(1.0, base[0] + var + noise * noise_amp * 0.7))
                    g = max(0.0, min(1.0, base[1] + var + noise * noise_amp * 1.3))
                    b = max(0.0, min(1.0, base[2] + var + noise * noise_amp * 0.5))
                else:
                    r = max(0.0, min(1.0, base[0] + var))
                    g = max(0.0, min(1.0, base[1] + var))
                    b = max(0.0, min(1.0, base[2] + var))

                color_attr.data[loop_idx].color = (r, g, b, 1.0)

        # Principled BSDF — fully matte, zero specular, exports cleanly to GLB
        mat = bpy.data.materials.new(name="TreeMat")
        mat.use_nodes = True
        nodes = mat.node_tree.nodes
        links = mat.node_tree.links
        nodes.clear()
        out_node  = nodes.new('ShaderNodeOutputMaterial'); out_node.location  = ( 400, 0)
        bsdf_node = nodes.new('ShaderNodeBsdfPrincipled'); bsdf_node.location = ( 100, 0)
        vcol_node = nodes.new('ShaderNodeVertexColor');    vcol_node.location = (-200, 0)
        vcol_node.layer_name = "Col"
        bsdf_node.inputs['Roughness'].default_value = 1.0
        bsdf_node.inputs['Metallic'].default_value  = 0.0
        # Kill specular — socket renamed in Blender 4.x, handle both
        for spec_name in ('Specular', 'Specular IOR Level'):
            if spec_name in bsdf_node.inputs:
                bsdf_node.inputs[spec_name].default_value = 0.0
        links.new(vcol_node.outputs['Color'], bsdf_node.inputs['Base Color'])
        links.new(bsdf_node.outputs['BSDF'],  out_node.inputs['Surface'])
        obj.data.materials.clear()
        obj.data.materials.append(mat)
        print(f"[Blender] Vertex colours applied ({tree_type}), matte material.")

    # -----------------------------------------------------------------------
    # 10. Export .glb
    # -----------------------------------------------------------------------
    os.makedirs(os.path.dirname(os.path.abspath(output_path)), exist_ok=True)

    ext_out = os.path.splitext(output_path)[1].lower()
    if ext_out in (".glb", ".gltf"):
        bpy.ops.export_scene.gltf(
            filepath=output_path,
            export_format="GLB",
            export_apply=True,
            export_materials="EXPORT",
            export_normals=True,
        )
    elif ext_out == ".obj":
        bpy.ops.wm.obj_export(
            filepath=output_path,
            export_triangulated_mesh=True,
        )
    elif ext_out == ".fbx":
        bpy.ops.export_scene.fbx(filepath=output_path, use_mesh_modifiers=True)
    elif ext_out == ".stl":
        bpy.ops.export_mesh.stl(filepath=output_path)
    else:
        print(f"ERROR: Unsupported output format '{ext_out}'")
        sys.exit(1)

    print(f"[Blender] Export complete: {output_path}")

    # -----------------------------------------------------------------------
    # 10b. Stump generation (optional — tree assets only)
    # -----------------------------------------------------------------------
    if generate_stump and normalize_height:
        stump_cut_z = float(normalize_height) * stump_height_ratio
        print(f"[Blender] Generating stump — cutting at Z={stump_cut_z:.4f} "
              f"({stump_height_ratio*100:.0f}% of {normalize_height})")

        # Duplicate the already-exported tree object
        bpy.ops.object.select_all(action="DESELECT")
        obj.select_set(True)
        bpy.context.view_layer.objects.active = obj
        bpy.ops.object.duplicate()
        stump_obj = bpy.context.active_object
        stump_obj.name = "Stump"

        # Bisect: keep everything BELOW stump_cut_z, fill the top face
        bpy.ops.object.mode_set(mode="EDIT")
        bpy.ops.mesh.select_all(action="SELECT")
        bpy.ops.mesh.bisect(
            plane_co=(0.0, 0.0, stump_cut_z),
            plane_no=(0.0, 0.0, 1.0),
            use_fill=True,
            clear_outer=True,    # delete everything above the cut plane
            clear_inner=False,
            threshold=0.001,
        )
        bpy.ops.object.mode_set(mode="OBJECT")

        # Add slight Z noise to the top ring for a natural, jagged look
        for vert in stump_obj.data.vertices:
            if abs(vert.co.z - stump_cut_z) < 0.08:
                vert.co.z += random.uniform(-0.025, 0.015)

        # Fix normals after bisect + noise
        bpy.ops.object.mode_set(mode="EDIT")
        bpy.ops.mesh.select_all(action="SELECT")
        bpy.ops.mesh.normals_make_consistent(inside=False)
        bpy.ops.object.mode_set(mode="OBJECT")

        # Shading
        if flat_shading:
            bpy.ops.object.shade_flat()

        # Vertex colours — trunk palette only (no canopy on a stump)
        if tree_type:
            palette     = TREE_PALETTES.get(tree_type, TREE_PALETTES["oak"])
            s_trunks    = palette["trunk"]
            s_variation = palette["variation"]

            smesh = stump_obj.data
            if "Col" in smesh.color_attributes:
                smesh.color_attributes.remove(smesh.color_attributes["Col"])
            s_color_attr = smesh.color_attributes.new(
                name="Col", type='BYTE_COLOR', domain='CORNER'
            )
            smesh.color_attributes.active_color_index = 0

            for poly in smesh.polygons:
                for loop_idx in poly.loop_indices:
                    base = random.choice(s_trunks)
                    var  = random.uniform(-s_variation, s_variation)
                    r = max(0.0, min(1.0, base[0] + var))
                    g = max(0.0, min(1.0, base[1] + var))
                    b = max(0.0, min(1.0, base[2] + var))
                    s_color_attr.data[loop_idx].color = (r, g, b, 1.0)

            s_mat   = bpy.data.materials.new(name="StumpMat")
            s_mat.use_nodes = True
            s_nodes = s_mat.node_tree.nodes
            s_links = s_mat.node_tree.links
            s_nodes.clear()
            s_out  = s_nodes.new('ShaderNodeOutputMaterial'); s_out.location  = ( 400, 0)
            s_bsdf = s_nodes.new('ShaderNodeBsdfPrincipled'); s_bsdf.location = ( 100, 0)
            s_vcol = s_nodes.new('ShaderNodeVertexColor');    s_vcol.location = (-200, 0)
            s_vcol.layer_name = "Col"
            s_bsdf.inputs['Roughness'].default_value = 1.0
            s_bsdf.inputs['Metallic'].default_value  = 0.0
            for spec_name in ('Specular', 'Specular IOR Level'):
                if spec_name in s_bsdf.inputs:
                    s_bsdf.inputs[spec_name].default_value = 0.0
            s_links.new(s_vcol.outputs['Color'], s_bsdf.inputs['Base Color'])
            s_links.new(s_bsdf.outputs['BSDF'],  s_out.inputs['Surface'])
            stump_obj.data.materials.clear()
            stump_obj.data.materials.append(s_mat)
            print(f"[Blender] Stump vertex colours applied ({tree_type} trunk palette).")

        # Export stump as a separate file alongside the main asset.
        # If the main output ends with _tree (e.g. oak_tree.glb) produce
        # oak_stump.glb; otherwise fall back to appending _stump.
        base_out_s, ext_out_s = os.path.splitext(output_path)
        if base_out_s.endswith("_tree"):
            stump_path = base_out_s[:-5] + "_stump" + ext_out_s
        else:
            stump_path = base_out_s + "_stump" + ext_out_s

        bpy.ops.object.select_all(action="DESELECT")
        stump_obj.select_set(True)
        bpy.context.view_layer.objects.active = stump_obj

        if ext_out_s in (".glb", ".gltf"):
            bpy.ops.export_scene.gltf(
                filepath=stump_path,
                export_format="GLB",
                export_apply=True,
                export_materials="EXPORT",
                use_selection=True,
            )
        elif ext_out_s == ".obj":
            bpy.ops.wm.obj_export(
                filepath=stump_path,
                export_triangulated_mesh=True,
                export_selected_objects=True,
            )
        elif ext_out_s == ".fbx":
            bpy.ops.export_scene.fbx(
                filepath=stump_path,
                use_selection=True,
                use_mesh_modifiers=True,
            )
        elif ext_out_s == ".stl":
            bpy.ops.export_mesh.stl(filepath=stump_path, use_selection=True)

        print(f"[Blender] Stump export complete: {stump_path}")


if __name__ == "__main__":
    main()
