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
        "trunk_ratio": 0.34,
        "variation": 0.04,
    },
    "pine": {
        "trunk":  [(0.32, 0.19, 0.09), (0.25, 0.15, 0.07), (0.38, 0.22, 0.10)],
        "canopy": [(0.09, 0.22, 0.07), (0.11, 0.28, 0.09), (0.07, 0.18, 0.06)],
        "trunk_ratio": 0.22,
        "variation": 0.03,
    },
    "willow": {
        "trunk":  [(0.38, 0.30, 0.18), (0.30, 0.23, 0.13), (0.42, 0.33, 0.20)],
        "canopy": [(0.28, 0.50, 0.16), (0.22, 0.42, 0.13), (0.32, 0.55, 0.18)],
        "trunk_ratio": 0.18,
        "variation": 0.05,
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
    # 3d. Geometry smooth — gentle pass to round out voxel faceting
    # -----------------------------------------------------------------------
    smooth_mod = obj.modifiers.new(name="Smooth", type="SMOOTH")
    smooth_mod.factor = 0.5
    smooth_mod.iterations = 1
    bpy.ops.object.modifier_apply(modifier=smooth_mod.name)
    print("[Blender] Geometry smooth applied (1 iteration, factor 0.5)")

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
    print("[Blender] Holes filled (2 passes) and patch normals fixed.")

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
    # 9c. Vertex colour painting (tree types only)
    # -----------------------------------------------------------------------
    if tree_type:
        palette = TREE_PALETTES.get(tree_type, TREE_PALETTES["oak"])
        trunk_colors  = palette["trunk"]
        canopy_colors = palette["canopy"]
        trunk_ratio   = palette["trunk_ratio"]
        variation     = palette["variation"]

        mesh = obj.data
        min_z_vc  = min(v.co.z for v in mesh.vertices)
        max_z_vc  = max(v.co.z for v in mesh.vertices)
        height_vc = max(max_z_vc - min_z_vc, 0.001)

        # Create (or replace) the colour attribute
        if "Col" in mesh.color_attributes:
            mesh.color_attributes.remove(mesh.color_attributes["Col"])
        color_attr = mesh.color_attributes.new(name="Col", type='BYTE_COLOR', domain='CORNER')
        mesh.color_attributes.active_color_index = 0

        for poly in mesh.polygons:
            for loop_idx in poly.loop_indices:
                v_idx  = mesh.loops[loop_idx].vertex_index
                z_norm = (mesh.vertices[v_idx].co.z - min_z_vc) / height_vc
                base   = random.choice(trunk_colors if z_norm < trunk_ratio else canopy_colors)
                var    = random.uniform(-variation, variation)
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

        # Export stump as a separate file alongside the main asset
        base_out_s, ext_out_s = os.path.splitext(output_path)
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
