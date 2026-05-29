"""
Blender tree generation script — OSRS-style low-poly trees.

Produces clean OSRS-style trees by:
  1. Importing the AI mesh (Hunyuan3D) and extracting only the trunk (lower portion
     determined by trunk_height_ratio), giving it organic shape variation per asset.
  2. Generating a clean procedural canopy (displaced UV-sphere) from style parameters
     so the shape is always intentional and controllable per tree type.
  3. Painting separate vertex colours on trunk and canopy.
  4. Joining, decimating, and exporting as a single GLB.
  5. Optionally exporting a stump GLB for depletion state.

This is far cleaner than trying to smooth the raw AI mesh, which produces
spiky fractal geometry when reconstructing leaf silhouettes.

Args (JSON string passed after --):
    input               : path to raw AI mesh (.glb)
    output              : path for exported .glb
    tree_type           : oak | willow | pine | dead | magic | yew
    normalize_height    : target height in Blender metres
    trunk_height_ratio  : fraction of AI mesh height to keep as trunk (e.g. 0.35)
    canopy_shape        : sphere | conical
    canopy_radius       : canopy sphere radius in Blender metres
    canopy_z_ratio      : canopy centre Z as fraction of normalize_height
    canopy_height_scale : Z scale of canopy sphere (1.0 = sphere, 0.7 = flat, 1.3 = tall)
    canopy_lumpiness    : displace strength — how bumpy (0.2 gentle, 0.5 very lumpy)
    canopy_noise_scale  : lump size (0.3 big lumps, 0.6 small)
    target_tris         : total triangle budget (split ~25% trunk / 75% canopy)
    flat_shading        : bool
    generate_stump      : bool — also export a stump GLB
    stump_height_ratio  : stump height as fraction of normalize_height
    max_root_radius     : clamp trunk base verts inside this XY radius (metres)
    center_origin       : bool
"""

import sys
import json
import os
import math
import random

# ---------------------------------------------------------------------------
# Vertex-colour palettes
# ---------------------------------------------------------------------------
TREE_PALETTES = {
    "oak": {
        "trunk":  [(0.29, 0.18, 0.10), (0.38, 0.24, 0.13), (0.32, 0.21, 0.11)],
        "canopy": [(0.17, 0.33, 0.10), (0.22, 0.44, 0.13), (0.28, 0.52, 0.16), (0.15, 0.30, 0.09)],
        "variation": 0.04,
    },
    "pine": {
        "trunk":  [(0.32, 0.19, 0.09), (0.25, 0.15, 0.07), (0.38, 0.22, 0.10)],
        "canopy": [(0.09, 0.22, 0.07), (0.11, 0.28, 0.09), (0.07, 0.18, 0.06)],
        "variation": 0.03,
    },
    "willow": {
        "trunk":  [(0.20, 0.12, 0.06), (0.16, 0.10, 0.05), (0.24, 0.15, 0.07)],
        "canopy": [(0.10, 0.22, 0.05), (0.08, 0.18, 0.04), (0.13, 0.27, 0.06), (0.09, 0.20, 0.05)],
        "variation": 0.03,
    },
    "dead": {
        "trunk":  [(0.38, 0.34, 0.29), (0.30, 0.27, 0.23), (0.44, 0.40, 0.34)],
        "canopy": [(0.35, 0.31, 0.27), (0.28, 0.25, 0.22)],
        "variation": 0.03,
    },
    "magic": {
        "trunk":  [(0.12, 0.09, 0.22), (0.16, 0.11, 0.30), (0.10, 0.08, 0.18)],
        "canopy": [(0.22, 0.11, 0.48), (0.30, 0.16, 0.62), (0.18, 0.22, 0.55), (0.35, 0.12, 0.70)],
        "variation": 0.05,
    },
    "yew": {
        "trunk":  [(0.28, 0.20, 0.12), (0.22, 0.16, 0.09), (0.34, 0.24, 0.14)],
        "canopy": [(0.06, 0.20, 0.06), (0.08, 0.25, 0.08), (0.05, 0.18, 0.05)],
        "variation": 0.03,
    },
}


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

def _paint_vertex_colors(obj, colors, variation):
    """Paint per-polygon vertex colours from a colour list + random variation."""
    import bpy
    mesh = obj.data
    if "Col" in mesh.color_attributes:
        mesh.color_attributes.remove(mesh.color_attributes["Col"])
    attr = mesh.color_attributes.new(name="Col", type='BYTE_COLOR', domain='CORNER')
    mesh.color_attributes.active_color_index = 0
    for poly in mesh.polygons:
        base = random.choice(colors)
        for loop_idx in poly.loop_indices:
            var = random.uniform(-variation, variation)
            attr.data[loop_idx].color = (
                max(0.0, min(1.0, base[0] + var)),
                max(0.0, min(1.0, base[1] + var)),
                max(0.0, min(1.0, base[2] + var)),
                1.0,
            )


def _vcol_material(name):
    """Matte Principled BSDF wired to vertex colour attribute 'Col'."""
    import bpy
    mat = bpy.data.materials.new(name=name)
    mat.use_nodes = True
    nodes = mat.node_tree.nodes
    links = mat.node_tree.links
    nodes.clear()
    out  = nodes.new('ShaderNodeOutputMaterial'); out.location  = ( 400, 0)
    bsdf = nodes.new('ShaderNodeBsdfPrincipled'); bsdf.location = ( 100, 0)
    vcol = nodes.new('ShaderNodeVertexColor');    vcol.location = (-200, 0)
    vcol.layer_name = "Col"
    bsdf.inputs['Roughness'].default_value = 1.0
    bsdf.inputs['Metallic'].default_value  = 0.0
    for spec in ('Specular', 'Specular IOR Level'):
        if spec in bsdf.inputs:
            bsdf.inputs[spec].default_value = 0.0
    links.new(vcol.outputs['Color'], bsdf.inputs['Base Color'])
    links.new(bsdf.outputs['BSDF'],  out.inputs['Surface'])
    return mat


def _apply_shading(obj, flat):
    """Apply flat or smooth-by-angle shading to obj (must be active + selected)."""
    import bpy
    bpy.ops.object.select_all(action="DESELECT")
    obj.select_set(True)
    bpy.context.view_layer.objects.active = obj
    bpy.ops.object.mode_set(mode="OBJECT")

    if flat:
        bpy.ops.object.shade_flat()
    else:
        if bpy.app.version >= (4, 1, 0):
            bpy.ops.object.shade_smooth_by_angle(angle=math.radians(60))
        else:
            bpy.ops.object.shade_smooth()
            obj.data.use_auto_smooth = True
            obj.data.auto_smooth_angle = math.radians(60)
        wn = obj.modifiers.new("WeightedNormal", "WEIGHTED_NORMAL")
        wn.mode = 'FACE_AREA'
        wn.weight = 50
        wn.keep_sharp = False
        bpy.ops.object.modifier_apply(modifier=wn.name)


def _export_glb(path, selection_only=False):
    import bpy
    os.makedirs(os.path.dirname(os.path.abspath(path)), exist_ok=True)
    bpy.ops.export_scene.gltf(
        filepath=path,
        export_format="GLB",
        export_apply=True,
        export_materials="EXPORT",
        export_normals=True,
        export_colors=True,
        use_selection=selection_only,
    )


# ---------------------------------------------------------------------------
# Canopy generation
# ---------------------------------------------------------------------------

def _make_canopy_sphere(radius, z_center, height_scale, lumpiness, noise_scale):
    """Displaced UV-sphere — used for oak, willow, yew, magic."""
    import bpy
    bpy.ops.mesh.primitive_uv_sphere_add(
        radius=radius,
        location=(0.0, 0.0, z_center),
        segments=22,
        ring_count=14,
    )
    obj = bpy.context.active_object
    obj.name = "Canopy"
    obj.scale.z = height_scale
    bpy.ops.object.transform_apply(scale=True)

    tex = bpy.data.textures.new("CanopyNoise", type='CLOUDS')
    tex.noise_scale = noise_scale
    tex.noise_depth = 2
    mod = obj.modifiers.new("Displace", "DISPLACE")
    mod.texture = tex
    mod.strength = lumpiness
    mod.texture_coords = 'OBJECT'
    bpy.ops.object.modifier_apply(modifier=mod.name)
    return obj


def _make_canopy_conical(radius, z_bottom, total_height, lumpiness, noise_scale, num_tiers=3):
    """Stacked flattened spheres — used for pine."""
    import bpy
    tier_objs = []
    for i in range(num_tiers):
        frac = i / max(num_tiers - 1, 1)
        t_radius = radius * (1.0 - frac * 0.55)
        t_z = z_bottom + total_height * (0.15 + frac * 0.70)

        bpy.ops.mesh.primitive_uv_sphere_add(
            radius=t_radius,
            location=(0.0, 0.0, t_z),
            segments=16,
            ring_count=10,
        )
        t_obj = bpy.context.active_object
        t_obj.scale.z = 0.40
        bpy.ops.object.transform_apply(scale=True)

        tex = bpy.data.textures.new(f"TierNoise{i}", type='CLOUDS')
        tex.noise_scale = noise_scale
        mod = t_obj.modifiers.new("Displace", "DISPLACE")
        mod.texture = tex
        mod.strength = lumpiness * 0.65
        mod.texture_coords = 'OBJECT'
        bpy.ops.object.modifier_apply(modifier=mod.name)
        tier_objs.append(t_obj)

    bpy.ops.object.select_all(action="DESELECT")
    for o in tier_objs:
        o.select_set(True)
    bpy.context.view_layer.objects.active = tier_objs[0]
    if len(tier_objs) > 1:
        bpy.ops.object.join()
    obj = bpy.context.active_object
    obj.name = "Canopy"
    return obj


# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------

def main():
    argv = sys.argv
    if "--" not in argv:
        print("ERROR: No arguments passed after '--'.")
        sys.exit(1)
    raw = argv[argv.index("--") + 1:]
    if not raw:
        print("ERROR: Empty argument list.")
        sys.exit(1)
    try:
        args = json.loads(raw[0])
    except json.JSONDecodeError as e:
        print(f"ERROR: Bad args JSON: {e}")
        sys.exit(1)

    input_path          = args["input"]
    output_path         = args["output"]
    tree_type           = args.get("tree_type", "oak")
    normalize_height    = float(args.get("normalize_height", 4.2))
    trunk_height_ratio  = float(args.get("trunk_height_ratio", 0.35))
    canopy_shape        = args.get("canopy_shape", "sphere")
    canopy_radius       = float(args.get("canopy_radius", 1.2))
    canopy_z_ratio      = float(args.get("canopy_z_ratio", 0.60))
    canopy_height_scale = float(args.get("canopy_height_scale", 0.85))
    canopy_lumpiness    = float(args.get("canopy_lumpiness", 0.35))
    canopy_noise_scale  = float(args.get("canopy_noise_scale", 0.45))
    target_tris         = int(args.get("target_tris", 1200))
    flat_shading        = bool(args.get("flat_shading", False))
    generate_stump      = bool(args.get("generate_stump", False))
    stump_height_ratio  = float(args.get("stump_height_ratio", 0.18))
    max_root_radius     = args.get("max_root_radius")
    if max_root_radius is not None:
        max_root_radius = float(max_root_radius)

    if not os.path.exists(input_path):
        print(f"ERROR: Input not found: {input_path}")
        sys.exit(1)

    palette = TREE_PALETTES.get(tree_type, TREE_PALETTES["oak"])

    import bpy

    # -----------------------------------------------------------------------
    # 1. Reset scene and import AI mesh
    # -----------------------------------------------------------------------
    bpy.ops.wm.read_factory_settings(use_empty=True)

    ext = os.path.splitext(input_path)[1].lower()
    if ext in (".glb", ".gltf"):
        bpy.ops.import_scene.gltf(filepath=input_path)
    elif ext == ".obj":
        bpy.ops.wm.obj_import(filepath=input_path)
    else:
        print(f"ERROR: Unsupported format '{ext}'")
        sys.exit(1)

    bpy.ops.object.select_all(action="DESELECT")
    meshes = [o for o in bpy.context.scene.objects if o.type == "MESH"]
    if not meshes:
        print("ERROR: No mesh objects found in imported file.")
        sys.exit(1)
    for m in meshes:
        m.select_set(True)
    bpy.context.view_layer.objects.active = meshes[0]
    if len(meshes) > 1:
        bpy.ops.object.join()
    ai_obj = bpy.context.active_object
    ai_obj.name = "AITree"
    bpy.ops.object.transform_apply(location=True, rotation=True, scale=True)
    print(f"[BlenderTree] Imported AI mesh.")

    # -----------------------------------------------------------------------
    # 2. Remove loose geometry and merge coincident verts
    #    Hunyuan3D sometimes produces outlier verts far from the main mesh.
    #    These blow up the voxel remesh bounding box → OOM crash.
    # -----------------------------------------------------------------------
    bpy.ops.object.mode_set(mode="EDIT")
    bpy.ops.mesh.select_all(action="SELECT")
    bpy.ops.mesh.delete_loose(use_verts=True, use_edges=True, use_faces=False)
    bpy.ops.mesh.remove_doubles(threshold=0.002)
    bpy.ops.object.mode_set(mode="OBJECT")
    print("[BlenderTree] Loose geometry removed.")

    # -----------------------------------------------------------------------
    # 3. Sit on z=0 and scale to normalize_height BEFORE remeshing.
    #    This guarantees a known bounding box so voxel_size is predictable.
    # -----------------------------------------------------------------------
    bpy.ops.object.transform_apply(location=True, rotation=False, scale=False)
    verts = [ai_obj.matrix_world @ v.co for v in ai_obj.data.vertices]
    min_z = min(v.z for v in verts)
    max_z = max(v.z for v in verts)
    current_height = max(max_z - min_z, 1e-6)

    ai_obj.location.z -= min_z
    bpy.ops.object.transform_apply(location=True, rotation=False, scale=False)

    # Scale to final target height
    sf = normalize_height / current_height
    ai_obj.scale = (sf, sf, sf)
    bpy.ops.object.transform_apply(scale=True)
    print(f"[BlenderTree] Mesh normalised to {normalize_height}m.")

    # -----------------------------------------------------------------------
    # 4. Voxel remesh — now safe because bbox = normalize_height (~4–5 m).
    #    voxel_size 0.05 on a 4.2 m tree → ~84³ grid → totally reasonable.
    # -----------------------------------------------------------------------
    rem = ai_obj.modifiers.new("Remesh", type="REMESH")
    rem.mode = 'VOXEL'
    rem.voxel_size = 0.05
    bpy.ops.object.modifier_apply(modifier=rem.name)
    print("[BlenderTree] Voxel remesh applied (voxel=0.05).")

    # -----------------------------------------------------------------------
    # 5. Bisect trunk at fraction of normalize_height
    # -----------------------------------------------------------------------
    trunk_cut_z = normalize_height * trunk_height_ratio
    print(f"[BlenderTree] Bisecting trunk at z={trunk_cut_z:.3f}m ({trunk_height_ratio*100:.0f}% of {normalize_height}m)")

    bpy.ops.object.mode_set(mode="EDIT")
    bpy.ops.mesh.select_all(action="SELECT")
    bpy.ops.mesh.bisect(
        plane_co=(0.0, 0.0, trunk_cut_z),
        plane_no=(0.0, 0.0, 1.0),
        use_fill=True,
        clear_outer=True,
        clear_inner=False,
        threshold=0.001,
    )
    bpy.ops.mesh.select_all(action="SELECT")
    bpy.ops.mesh.delete_loose(use_verts=True, use_edges=True, use_faces=False)
    bpy.ops.mesh.normals_make_consistent(inside=False)
    bpy.ops.object.mode_set(mode="OBJECT")
    print("[BlenderTree] Trunk extracted.")

    # Mesh already at normalize_height — trunk top is simply:
    trunk_top_z = normalize_height * trunk_height_ratio
    print(f"[BlenderTree] Trunk top at z={trunk_top_z:.3f}m.")

    # -----------------------------------------------------------------------
    # 5. Root radius clamp — tuck base verts inside tile footprint
    # -----------------------------------------------------------------------
    if max_root_radius:
        threshold = trunk_top_z * 0.35
        clamped = 0
        for v in ai_obj.data.vertices:
            if v.co.z < threshold:
                xy = math.sqrt(v.co.x**2 + v.co.y**2)
                if xy > max_root_radius:
                    f = max_root_radius / xy
                    v.co.x *= f
                    v.co.y *= f
                    clamped += 1
        if clamped:
            print(f"[BlenderTree] Root radius: {clamped} verts clamped to {max_root_radius}m.")

    # -----------------------------------------------------------------------
    # 6. Decimate trunk (~25% of total tris budget)
    # -----------------------------------------------------------------------
    trunk_budget = max(200, target_tris // 4)
    cur = sum(len(p.vertices) - 2 for p in ai_obj.data.polygons)
    if cur > trunk_budget:
        ratio = max(0.01, min(1.0, trunk_budget / cur))
        dec = ai_obj.modifiers.new("Decimate", "DECIMATE")
        dec.ratio = ratio
        dec.use_collapse_triangulate = True
        bpy.ops.object.modifier_apply(modifier=dec.name)
    trunk_obj = ai_obj
    trunk_obj.name = "Trunk"
    print(f"[BlenderTree] Trunk ready (~{trunk_budget} tris).")

    # -----------------------------------------------------------------------
    # 7. Vertex colours + material for trunk
    # -----------------------------------------------------------------------
    _paint_vertex_colors(trunk_obj, palette["trunk"], palette["variation"])
    trunk_mat = _vcol_material("TrunkMat")
    trunk_obj.data.materials.clear()
    trunk_obj.data.materials.append(trunk_mat)
    print(f"[BlenderTree] Trunk colours applied ({tree_type}).")

    # -----------------------------------------------------------------------
    # 8. Duplicate trunk for stump BEFORE joining canopy
    # -----------------------------------------------------------------------
    stump_obj = None
    if generate_stump:
        bpy.ops.object.select_all(action="DESELECT")
        trunk_obj.select_set(True)
        bpy.context.view_layer.objects.active = trunk_obj
        bpy.ops.object.duplicate()
        stump_obj = bpy.context.active_object
        stump_obj.name = "Stump"
        stump_obj.select_set(False)
        print("[BlenderTree] Stump duplicate saved.")

    # -----------------------------------------------------------------------
    # 9. Generate procedural canopy
    # -----------------------------------------------------------------------
    bpy.ops.object.select_all(action="DESELECT")
    canopy_z = normalize_height * canopy_z_ratio
    print(f"[BlenderTree] Generating canopy ({canopy_shape}) — centre z={canopy_z:.2f}m, r={canopy_radius}m")

    if canopy_shape == "conical":
        canopy_total_height = normalize_height * (1.0 - trunk_height_ratio) * canopy_height_scale
        canopy_obj = _make_canopy_conical(
            radius=canopy_radius,
            z_bottom=trunk_top_z * 0.85,
            total_height=canopy_total_height,
            lumpiness=canopy_lumpiness,
            noise_scale=canopy_noise_scale,
        )
    else:
        canopy_obj = _make_canopy_sphere(
            radius=canopy_radius,
            z_center=canopy_z,
            height_scale=canopy_height_scale,
            lumpiness=canopy_lumpiness,
            noise_scale=canopy_noise_scale,
        )

    # -----------------------------------------------------------------------
    # 10. Decimate canopy (~75% of budget)
    # -----------------------------------------------------------------------
    canopy_budget = target_tris - trunk_budget
    c_cur = sum(len(p.vertices) - 2 for p in canopy_obj.data.polygons)
    if c_cur > canopy_budget:
        ratio = max(0.01, min(1.0, canopy_budget / c_cur))
        cdec = canopy_obj.modifiers.new("Decimate", "DECIMATE")
        cdec.ratio = ratio
        cdec.use_collapse_triangulate = True
        bpy.ops.object.modifier_apply(modifier=cdec.name)
    print(f"[BlenderTree] Canopy ready (~{canopy_budget} tris).")

    # -----------------------------------------------------------------------
    # 11. Vertex colours + material for canopy
    # -----------------------------------------------------------------------
    _paint_vertex_colors(canopy_obj, palette["canopy"], palette["variation"])
    canopy_mat = _vcol_material("CanopyMat")
    canopy_obj.data.materials.clear()
    canopy_obj.data.materials.append(canopy_mat)
    print(f"[BlenderTree] Canopy colours applied ({tree_type}).")

    # -----------------------------------------------------------------------
    # 12. Join trunk + canopy
    # -----------------------------------------------------------------------
    bpy.ops.object.select_all(action="DESELECT")
    trunk_obj.select_set(True)
    canopy_obj.select_set(True)
    bpy.context.view_layer.objects.active = trunk_obj
    bpy.ops.object.join()
    tree_obj = bpy.context.active_object
    tree_obj.name = "Tree"

    # Sit on floor
    bpy.ops.object.transform_apply(location=True, rotation=False, scale=False)
    min_z_f = min(v.co.z for v in tree_obj.data.vertices)
    tree_obj.location.z -= min_z_f
    bpy.ops.object.transform_apply(location=True, rotation=False, scale=False)
    print("[BlenderTree] Trunk + canopy joined.")

    # -----------------------------------------------------------------------
    # 13. Shading
    # -----------------------------------------------------------------------
    _apply_shading(tree_obj, flat_shading)
    print("[BlenderTree] Shading applied.")

    # -----------------------------------------------------------------------
    # 14. Export tree GLB
    # -----------------------------------------------------------------------
    _export_glb(output_path)
    print(f"[BlenderTree] Tree export complete: {output_path}")

    # -----------------------------------------------------------------------
    # 15. Stump export
    # -----------------------------------------------------------------------
    if generate_stump and stump_obj is not None:
        stump_cut_z = normalize_height * stump_height_ratio
        print(f"[BlenderTree] Generating stump at z={stump_cut_z:.3f}m ({stump_height_ratio*100:.0f}% of {normalize_height}m)")

        bpy.ops.object.select_all(action="DESELECT")
        stump_obj.select_set(True)
        bpy.context.view_layer.objects.active = stump_obj

        bpy.ops.object.mode_set(mode="EDIT")
        bpy.ops.mesh.select_all(action="SELECT")
        bpy.ops.mesh.bisect(
            plane_co=(0.0, 0.0, stump_cut_z),
            plane_no=(0.0, 0.0, 1.0),
            use_fill=True,
            clear_outer=True,
            clear_inner=False,
            threshold=0.001,
        )
        bpy.ops.object.mode_set(mode="OBJECT")

        # Slight noise on the cut face for a natural jagged look
        for v in stump_obj.data.vertices:
            if abs(v.co.z - stump_cut_z) < 0.08:
                v.co.z += random.uniform(-0.02, 0.01)

        bpy.ops.object.mode_set(mode="EDIT")
        bpy.ops.mesh.select_all(action="SELECT")
        bpy.ops.mesh.normals_make_consistent(inside=False)
        bpy.ops.object.mode_set(mode="OBJECT")

        _apply_shading(stump_obj, flat=True)

        base_out, ext_out = os.path.splitext(output_path)
        stump_path = base_out + "_stump" + ext_out

        bpy.ops.object.select_all(action="DESELECT")
        stump_obj.select_set(True)
        bpy.context.view_layer.objects.active = stump_obj
        _export_glb(stump_path, selection_only=True)
        print(f"[BlenderTree] Stump export complete: {stump_path}")


if __name__ == "__main__":
    main()
