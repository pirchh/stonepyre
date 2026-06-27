#!/usr/bin/env python3
"""Refine a raw (grey, shape-only) axe GLB into a game-ready axe.

Hunyuan's shape stage outputs an untextured mesh that's also a little over-thick
front-to-back (single-image depth inflation). This script, in one pass:

  1. auto-detects the height axis (longest bbox extent) and depth axis (shortest),
  2. thins ONLY the head (top region) along the depth axis with a smooth taper,
     so the handle is left untouched,
  3. paints flat two-tone vertex colours — wood handle + per-tier metal head —
     to match the flat-shaded look the trees use.

Runs on the GLB the main pipeline already produced, so it's a cheap post-step
to iterate on (no Hunyuan re-run, no Blender).

Usage:
    python refine_axe.py <input.glb> <output.glb> <tier>
        [--thin 0.6] [--head 0.62] [--thin-start 0.5] [--thin-full 0.72]

Example:
    .venv/Scripts/python.exe refine_axe.py output/flint_axe_lowpoly.glb \\
        ../../game/assets/items/axes/axe_flint.glb flint
"""
from __future__ import annotations

import argparse
import sys
from pathlib import Path

import numpy as np
import trimesh


# Shared wood handle colour (linear-ish 0..1 RGB).
HANDLE_WOOD = (0.32, 0.20, 0.11)

# Smooth per-vertex brightness variation amplitude — breaks up the flat colour
# so it reads as a material instead of paint (the trees use the same trick).
COLOUR_VARIATION = 0.13

# Per-tier metal head colour, following the icon ladder's colour story.
HEAD_METAL = {
    "flint":       (0.30, 0.31, 0.35),  # dark slate stone
    "copper":      (0.74, 0.41, 0.17),  # orange copper
    "bronze":      (0.60, 0.40, 0.20),  # warm bronze
    "iron":        (0.42, 0.43, 0.47),  # dark iron grey
    "steel":       (0.66, 0.69, 0.74),  # bright steel
    "obsidian":    (0.12, 0.11, 0.15),  # near-black volcanic glass
    "cinderforge": (0.52, 0.22, 0.12),  # ember red
    "bloodstone":  (0.48, 0.10, 0.13),  # deep crimson
    "jadewrought": (0.18, 0.52, 0.38),  # jade green
    "stormforged": (0.28, 0.46, 0.66),  # storm blue
    "starfall":    (0.40, 0.34, 0.62),  # cosmic violet
    "pyreglass":   (0.80, 0.46, 0.16),  # amber fire-glass
    "aetherite":   (0.52, 0.74, 0.82),  # ethereal pale cyan
}


def _smoothstep(t: np.ndarray) -> np.ndarray:
    t = np.clip(t, 0.0, 1.0)
    return t * t * (3.0 - 2.0 * t)


def main() -> int:
    ap = argparse.ArgumentParser(prog="refine_axe")
    ap.add_argument("input", help="raw/grey axe GLB from the pipeline")
    ap.add_argument("output", help="path for the refined GLB")
    ap.add_argument("tier", help="axe tier (flint, copper, ... aetherite)")
    ap.add_argument("--thin", type=float, default=0.6,
                    help="head depth multiplier at the very top (0.6 = 40%% thinner)")
    ap.add_argument("--handle", type=float, default=1.0,
                    help="shaft thickness on the blade-face axis you view head-on "
                         "(1.0 = unchanged, 0.6 = 40%% slimmer)")
    ap.add_argument("--handle-depth", type=float, default=1.0,
                    help="shaft thickness on the front-to-back (depth) axis "
                         "(1.0 = unchanged)")
    ap.add_argument("--head", type=float, default=0.5,
                    help="min height fraction for the metal blade (below this stays wood)")
    ap.add_argument("--blade-radius", type=float, default=1.6,
                    help="how far past the shaft radius a vert must be to count as blade "
                         "(higher = only the widest flare is metal, more shaft stays wood)")
    ap.add_argument("--thin-start", type=float, default=0.5,
                    help="height fraction where thinning begins ramping in")
    ap.add_argument("--thin-full", type=float, default=0.72,
                    help="height fraction where thinning reaches full strength")
    args = ap.parse_args()

    head_rgb = HEAD_METAL.get(args.tier.lower())
    if head_rgb is None:
        print(f"WARNING: unknown tier '{args.tier}', using flint grey. "
              f"Known: {', '.join(HEAD_METAL)}")
        head_rgb = HEAD_METAL["flint"]

    # force='mesh' concatenates a multi-part scene into a single mesh.
    mesh = trimesh.load(args.input, force="mesh")
    v = np.asarray(mesh.vertices, dtype=np.float64).copy()
    if len(v) == 0:
        print("ERROR: mesh has no vertices")
        return 1

    extent = v.max(axis=0) - v.min(axis=0)
    h_axis = int(np.argmax(extent))                  # longest = handle/height direction
    cross = [a for a in (0, 1, 2) if a != h_axis]
    d_axis = cross[0] if extent[cross[0]] <= extent[cross[1]] else cross[1]  # thinnest = depth
    h_lo, h_hi = v[:, h_axis].min(), v[:, h_axis].max()
    h_norm = (v[:, h_axis] - h_lo) / max(h_hi - h_lo, 1e-9)

    # --- Classify the flint blade: the wide part that protrudes from the central
    #     shaft in the upper region. The wooden shaft is the narrow central column,
    #     which stays wood for its FULL length (the head mounts onto it) plus the
    #     pommel at the base. Same radial test the trees use for trunk vs canopy.
    a0, a1 = cross
    lower = h_norm < 0.40                             # clean handle = pure shaft
    cx = float(np.median(v[lower, a0])) if lower.any() else float(np.median(v[:, a0]))
    cz = float(np.median(v[lower, a1])) if lower.any() else float(np.median(v[:, a1]))
    radius = np.sqrt((v[:, a0] - cx) ** 2 + (v[:, a1] - cz) ** 2)
    shaft_r = float(np.percentile(radius[lower], 85)) if lower.any() else float(np.percentile(radius, 30))
    blade = (h_norm > args.head) & (radius > shaft_r * args.blade_radius)

    # --- 1. Thin the blade along the depth axis (smooth height taper). The central
    #        shaft sits at the depth centre, so it's barely touched.
    ramp = _smoothstep((h_norm - args.thin_start) / max(args.thin_full - args.thin_start, 1e-9))
    factor = 1.0 - ramp * (1.0 - args.thin)
    centre_d = float(np.median(v[blade, d_axis])) if blade.any() else float(v[:, d_axis].mean())
    v[:, d_axis] = centre_d + (v[:, d_axis] - centre_d) * factor

    # --- 1b. Slim the wooden shaft toward its centre line (both cross axes);
    #         the flint blade keeps its full width, so only the handle thins.
    if args.handle != 1.0 or args.handle_depth != 1.0:
        wood = ~blade
        v[wood, a0] = cx + (v[wood, a0] - cx) * args.handle        # blade-face viewing width
        v[wood, a1] = cz + (v[wood, a1] - cz) * args.handle_depth  # front-to-back depth
    mesh.vertices = v

    # --- 2. Two-tone vertex colours (metal blade, wood handle) with smooth
    #        per-vertex brightness variation so they read as a material rather
    #        than dead-flat paint.
    base = np.where(blade[:, None], np.array(head_rgb), np.array(HANDLE_WOOD))
    shade = 0.6 * np.sin(v[:, h_axis] * 5.0) + 0.4 * np.sin(v[:, a0] * 13.0 + 1.0)
    base = np.clip(base * (1.0 + COLOUR_VARIATION * shade[:, None]), 0.0, 1.0)
    colours = np.empty((len(v), 4), dtype=np.uint8)
    colours[:, :3] = np.round(base * 255.0).astype(np.uint8)
    colours[:, 3] = 255
    mesh.visual = trimesh.visual.color.ColorVisuals(mesh, vertex_colors=colours)
    head_sel = blade  # alias for the summary below

    out = Path(args.output)
    out.parent.mkdir(parents=True, exist_ok=True)
    mesh.export(str(out))

    print(
        f"refined -> {out}\n"
        f"  tier={args.tier}  head_colour={head_rgb}\n"
        f"  axes: height={'XYZ'[h_axis]}  depth={'XYZ'[d_axis]}  bbox={extent.round(3).tolist()}\n"
        f"  shaft_radius={shaft_r:.3f}  blade = >{args.blade_radius:g}x shaft & >{args.head:.0%} height\n"
        f"  thinned blade depth to {args.thin:.0%}; handle width/depth = {args.handle:.0%}/{args.handle_depth:.0%}\n"
        f"  verts={len(v)}  blade_verts={int(head_sel.sum())}  wood_verts={int((~head_sel).sum())}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
