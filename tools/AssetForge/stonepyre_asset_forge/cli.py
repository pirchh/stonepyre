"""
StonepyreAssetForge CLI — argument parsing and main pipeline orchestration.

Entry point: generate_asset.py imports and calls cli.run()
"""

from __future__ import annotations

import argparse
import shutil
import sys
import time
from pathlib import Path
from typing import Optional

from stonepyre_asset_forge.config import RunConfig, get_style, resolve_output_path
from stonepyre_asset_forge.logging_utils import get_logger, log_step


TOTAL_STEPS = 6


def build_parser() -> argparse.ArgumentParser:
    p = argparse.ArgumentParser(
        prog="generate_asset",
        description="StonepyreAssetForge — convert a 2D image into a low-poly 3D asset.",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Examples:
  python generate_asset.py --input ./input/goblin.png
  python generate_asset.py --input ./input/character.png --style osrs_character --target-tris 1200
  python generate_asset.py --input ./input/tree.png --style osrs_tree_oak --format glb
  python generate_asset.py --input ./input/tree.png --style osrs_tree_oak --tree-type oak --generate-stump
  python generate_asset.py --input ./input/prop.png --backend stub --skip-bg-removal --verbose

Available styles:
  Characters : osrs_character, osrs_creature
  Props/envs : osrs_prop, osrs_tree, osrs_building
  Trees      : osrs_tree_oak, osrs_tree_pine, osrs_tree_willow,
               osrs_tree_dead, osrs_tree_magic, osrs_tree_yew
  Raw        : raw

Available backends: triposr, hunyuan3d, stub  (stable_fast_3d: not yet implemented)

Tree notes:
  --tree-type overrides the palette; --generate-stump exports <name>_stump.<ext> alongside the tree.
  Stump-only heights can be tuned with --stump-height-ratio (e.g. 0.20 = 20% of tree height).
""",
    )
    p.add_argument("--input", "-i", required=True, help="Path to the input image.")
    p.add_argument("--output", "-o", default=None, help="Path for the output file. Default: output/<stem>_lowpoly.glb")
    p.add_argument("--target-tris", type=int, default=None, help="Override triangle count. Default comes from --style.")
    p.add_argument("--style", default="osrs_character", help="Style preset name. Default: osrs_character")
    p.add_argument("--format", default="glb", choices=["glb", "obj", "stl", "fbx"], help="Output format. Default: glb")
    p.add_argument("--backend", default="hunyuan3d", choices=["triposr", "stable_fast_3d", "hunyuan3d", "stub"],
                   help="Image-to-3D backend. Default: hunyuan3d")
    p.add_argument("--keep-temp", action="store_true", help="Keep the temp working directory after generation.")
    p.add_argument("--skip-bg-removal", action="store_true", help="Skip background removal step.")
    p.add_argument("--no-texture", action="store_true", help="Strip textures from the final mesh.")
    p.add_argument("--flat-shading", action="store_true", help="Force flat shading (overrides style default).")
    p.add_argument("--seed", type=int, default=None, help="Random seed for the image-to-3D model.")
    p.add_argument("--tree-type", default=None,
                   choices=[
                       "oak", "willow", "yew", "pine", "magic", "dead",
                       "hickory", "cherry", "beech", "maple", "ash", "birch",
                       "cedar", "spruce", "fir", "elm", "poplar", "sycamore",
                       "walnut", "chestnut", "alder", "rowan", "hawthorn",
                       "cypress", "mahogany", "teak", "ebony", "ironwood",
                       "elder", "redwood", "sequoia", "aspen", "juniper",
                       "mangrove", "baobab", "palm", "bloodwood", "silverleaf",
                       "moonwillow", "dragonwood",
                   ],
                   help="Enable tree vertex colour painting and set palette. Overrides style default.")
    p.add_argument("--generate-stump", action="store_true",
                   help="Also export a stump-only version of the tree asset (<name>_stump.<ext>).")
    p.add_argument("--stump-height-ratio", type=float, default=None,
                   help="Stump height as a fraction of normalize_height (default from style, usually 0.18).")
    p.add_argument("--verbose", "-v", action="store_true", help="Enable debug-level logging.")
    return p


def run(argv: Optional[list[str]] = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)

    logger = get_logger(verbose=args.verbose)
    logger.info("Starting generation")

    # --- Resolve config ---
    input_path = Path(args.input)
    try:
        style = get_style(args.style)
    except (FileNotFoundError, ValueError) as e:
        logger.error(str(e))
        return 1

    if args.target_tris is not None:
        style.target_tris = args.target_tris
    if args.flat_shading:
        style.flat_shading = True
    if args.tree_type is not None:
        style.tree_type = args.tree_type
    if args.generate_stump:
        style.generate_stump = True
    if args.stump_height_ratio is not None:
        style.stump_height_ratio = args.stump_height_ratio

    output_path = resolve_output_path(input_path, args.output, args.format, tree_type=style.tree_type)
    temp_dir = Path("temp") / input_path.stem

    cfg = RunConfig(
        input_path=input_path,
        output_path=output_path,
        style_name=args.style,
        style=style,
        output_format=args.format,
        keep_temp=args.keep_temp,
        skip_bg_removal=args.skip_bg_removal,
        no_texture=args.no_texture,
        flat_shading=style.flat_shading,
        target_tris=style.target_tris,
        seed=args.seed,
        verbose=args.verbose,
        backend=args.backend,
    )

    start = time.monotonic()

    try:
        _run_pipeline(cfg, temp_dir, logger)
    except (FileNotFoundError, ValueError, ImportError, RuntimeError, NotImplementedError) as e:
        logger.error(str(e))
        return 1
    finally:
        if not cfg.keep_temp and temp_dir.exists():
            shutil.rmtree(temp_dir, ignore_errors=True)

    elapsed = time.monotonic() - start
    logger.info(f"Done. ({elapsed:.1f}s)  →  {cfg.output_path}")
    return 0


def _run_pipeline(cfg: RunConfig, temp_dir: Path, logger) -> None:
    from stonepyre_asset_forge.pipeline import preprocess, image_to_3d, postprocess, export
    from stonepyre_asset_forge.logging_utils import log_step

    temp_dir.mkdir(parents=True, exist_ok=True)

    # Step 1 — Load & validate image
    log_step(logger, 1, TOTAL_STEPS, f"Loading image: {cfg.input_path}")
    preprocess.validate_image(cfg.input_path)
    working_image = preprocess.copy_to_temp(cfg.input_path, temp_dir)
    working_image = preprocess.ensure_rgba_png(working_image)

    # Step 2 — Background removal
    if not cfg.skip_bg_removal:
        log_step(logger, 2, TOTAL_STEPS, "Removing background")
        working_image = preprocess.remove_background(working_image)
        working_image = preprocess.crop_to_subject(working_image)
    else:
        log_step(logger, 2, TOTAL_STEPS, "Skipping background removal")

    # Step 3 — Image to 3D
    log_step(logger, 3, TOTAL_STEPS, f"Running image-to-3D backend: {cfg.backend}")
    backend = image_to_3d.get_backend(
        cfg.backend,
        device="cuda",
        logger=logger,
    )
    raw_mesh_path = temp_dir / f"{cfg.input_path.stem}_raw.glb"
    options = {
        "seed": cfg.seed,
        "texture_size": cfg.style.texture_size,
    }
    raw_mesh_path = backend.generate(working_image, raw_mesh_path, options)

    # Step 4 — Save raw mesh confirmation
    log_step(logger, 4, TOTAL_STEPS, f"Saving raw mesh: {raw_mesh_path}")

    # Step 5 — Low-poly post-processing
    log_step(logger, 5, TOTAL_STEPS, f"Applying low-poly post-processing (style: {cfg.style_name})")
    # For FBX, have Blender write the format directly (trimesh can't export FBX)
    proc_ext = "fbx" if cfg.output_format == "fbx" else "glb"
    processed_path = temp_dir / f"{cfg.input_path.stem}_processed.{proc_ext}"
    postprocess.run_postprocess(
        raw_mesh_path=raw_mesh_path,
        output_path=processed_path,
        style=cfg.style,
        target_tris=cfg.target_tris,
        flat_shading=cfg.flat_shading,
        logger=logger,
    )

    # Step 6 — Export
    log_step(logger, 6, TOTAL_STEPS, f"Export complete: {cfg.output_path}")
    export.export_mesh(
        processed_path=processed_path,
        output_path=cfg.output_path,
        fmt=cfg.output_format,
        logger=logger,
    )

    # Step 6b — Copy stump if Blender generated one.
    # Blender names it <stem>_stump.glb, replacing _tree if present.
    def _stump_name(p: Path) -> Path:
        s = p.stem
        base = s[:-5] if s.endswith("_tree") else s
        return p.with_name(base + "_stump" + p.suffix)

    stump_processed = _stump_name(processed_path)
    if stump_processed.exists():
        import shutil
        stump_output = _stump_name(cfg.output_path)
        shutil.copy2(stump_processed, stump_output)
        logger.info(f"Stump exported  →  {stump_output}")
