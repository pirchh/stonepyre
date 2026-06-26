#!/usr/bin/env python3
"""Turn raw icon renders (solid/white background) into game-ready inventory icons.

For each PNG in the input dir:
  1. remove the background with rembg  -> transparent RGBA
  2. crop tight to the subject
  3. scale to fit the target size *preserving aspect ratio*
  4. center on a transparent square canvas of the target size

Step 3/4 matter for non-square items like axes: a plain square resize would
stretch a tall axe; fit-and-pad keeps its proportions.

Requires the AssetForge venv (rembg + Pillow). First run downloads the rembg
model (~170 MB) if it isn't cached yet.

Usage:
    python prep_icons.py <input_dir> <output_dir> [size]

Example (axe renders -> live inventory icons at 64px):
    .venv/Scripts/python.exe prep_icons.py staging/axe_icons \\
        ../../game/assets/inventory/items 64
"""
import sys
from pathlib import Path

from PIL import Image
from rembg import remove


def main() -> int:
    if len(sys.argv) < 3:
        print("usage: prep_icons.py <input_dir> <output_dir> [size=64]")
        return 1

    in_dir = Path(sys.argv[1])
    out_dir = Path(sys.argv[2])
    size = int(sys.argv[3]) if len(sys.argv) > 3 else 64
    rotate = float(sys.argv[4]) if len(sys.argv) > 4 else 0.0  # clockwise degrees

    pngs = sorted(in_dir.glob("*.png"))
    if not pngs:
        print(f"no PNGs found in {in_dir}")
        return 1

    out_dir.mkdir(parents=True, exist_ok=True)
    for p in pngs:
        src = Image.open(p).convert("RGBA")

        cut = remove(src)  # rembg: background -> transparent
        if cut.mode != "RGBA":
            cut = cut.convert("RGBA")

        bbox = cut.getbbox()  # tight crop to the visible subject
        if bbox:
            cut = cut.crop(bbox)

        # Optional rotation (positive = clockwise) so a tall item sits on the
        # diagonal and fills a square slot; re-crop to the rotated bounds.
        if rotate:
            cut = cut.rotate(-rotate, expand=True, resample=Image.Resampling.BICUBIC)
            bbox = cut.getbbox()
            if bbox:
                cut = cut.crop(bbox)

        # Scale to fit `size` while preserving aspect ratio.
        cut.thumbnail((size, size), Image.Resampling.LANCZOS)

        # Center on a transparent square canvas.
        canvas = Image.new("RGBA", (size, size), (0, 0, 0, 0))
        x = (size - cut.width) // 2
        y = (size - cut.height) // 2
        canvas.paste(cut, (x, y), cut)
        canvas.save(out_dir / p.name)
        print(f"  {p.name}  ->  {size}x{size}  (bg removed, fit+centered)")

    print(f"done: {len(pngs)} icon(s) written to {out_dir}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
