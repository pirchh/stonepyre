#!/usr/bin/env python3
"""Sample a representative head (metal) colour from each axe render.

For each staging render we estimate the background from the corners, mask it
out, take the top ~38% of the subject (the head), drop shadow/highlight
extremes, and report the median colour. Prints ready-to-paste 0..1 RGB tuples
for refine_axe.py's HEAD_METAL table.

Usage: .venv/Scripts/python.exe sample_axe_colors.py [staging/inventory_icons/axes]
"""
import glob
import sys
from pathlib import Path

import numpy as np
from PIL import Image


def sample_head_colour(path: str):
    img = Image.open(path).convert("RGBA")
    arr = np.asarray(img).astype(np.float32)
    rgb, alpha = arr[..., :3], arr[..., 3]
    h, w = rgb.shape[:2]

    # Background estimate from the four corners.
    c = 12
    corners = np.concatenate([
        rgb[:c, :c].reshape(-1, 3), rgb[:c, -c:].reshape(-1, 3),
        rgb[-c:, :c].reshape(-1, 3), rgb[-c:, -c:].reshape(-1, 3),
    ])
    bg = np.median(corners, axis=0)

    not_bg = (np.linalg.norm(rgb - bg, axis=2) > 40) & (alpha > 128)
    ys, xs = np.where(not_bg)
    if len(ys) == 0:
        return None

    # Head = top 38% of the subject's vertical extent.
    ymin, ymax = ys.min(), ys.max()
    head_cut = ymin + 0.38 * (ymax - ymin)
    rows = np.arange(h)[:, None]
    head = not_bg & (rows < head_cut)

    px = rgb[head]
    if len(px) == 0:
        return None
    lum = px.mean(axis=1)
    keep = (lum > 30) & (lum < 235)  # drop outline shadow + specular highlight
    if keep.any():
        px = px[keep]
    return np.median(px, axis=0) / 255.0


def main() -> int:
    base = sys.argv[1] if len(sys.argv) > 1 else "staging/inventory_icons/axes"
    print("    # sampled head colours (paste into HEAD_METAL):")
    for p in sorted(glob.glob(str(Path(base) / "*_axe.png"))):
        tier = Path(p).stem.replace("_axe", "")
        col = sample_head_colour(p)
        if col is None:
            print(f'    "{tier}":  # could not sample')
        else:
            print(f'    "{tier}": ({col[0]:.2f}, {col[1]:.2f}, {col[2]:.2f}),')
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
