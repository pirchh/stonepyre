#!/usr/bin/env python3
"""Batch-resize inventory-icon PNGs to the game's icon size (default 64x64).

AssetForge exports icons at high resolution (e.g. 1024x1024); the game renders
inventory/HUD icons at ~22-64px, so this downscales them to a consistent square
size while preserving transparency. Reusable for any skill's icon set.

Usage:
    python resize_icons.py <input_dir> <output_dir> [size]

Example (resize the staged woodcutting logs straight into the live assets dir):
    python resize_icons.py staging/inventory_icons \\
        ../../game/assets/inventory/items 64
"""
import sys
from pathlib import Path

from PIL import Image


def main() -> int:
    if len(sys.argv) < 3:
        print("usage: resize_icons.py <input_dir> <output_dir> [size=64]")
        return 1

    in_dir = Path(sys.argv[1])
    out_dir = Path(sys.argv[2])
    size = int(sys.argv[3]) if len(sys.argv) > 3 else 64

    pngs = sorted(in_dir.glob("*.png"))
    if not pngs:
        print(f"no PNGs found in {in_dir}")
        return 1

    out_dir.mkdir(parents=True, exist_ok=True)
    for p in pngs:
        img = Image.open(p).convert("RGBA")
        if img.size != (size, size):
            img = img.resize((size, size), Image.Resampling.LANCZOS)
        img.save(out_dir / p.name)
        print(f"  {p.name}  ->  {size}x{size}")

    print(f"done: {len(pngs)} icon(s) written to {out_dir} @ {size}x{size}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
