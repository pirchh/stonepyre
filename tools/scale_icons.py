"""
scale_icons.py — Batch downscale game icons from high-res (e.g. 1024x1024) to
a game-ready size using Lanczos resampling + unsharp mask.

Drop source PNGs into tools/assets/staging/ (any subfolder structure is fine).
The script writes scaled copies to game/assets/inventory/items/ by default.

Usage:
    python scale_icons.py                    # staging → game/assets/inventory/items
    python scale_icons.py --size 128         # override output size
    python scale_icons.py --input path/to/icons --output path/to/out --size 128

Requirements:
    pip install Pillow
"""

import argparse
import sys
from pathlib import Path

# ── Defaults ──────────────────────────────────────────────────────────────────

# Drop source images here (relative to this script).
DEFAULT_INPUT = "assets/staging"

# Where the processed runtime assets land (relative to this script).
DEFAULT_OUTPUT = "../game/assets/inventory/items"

# Output pixel size (square). 128 is the sweet spot:
#   • Large enough that Bevy's linear filter stays clean at 52-62 px render size.
#   • Small enough that the 1024→128 downscale is handled cleanly by Lanczos.
DEFAULT_SIZE = 64

# Unsharp mask — recovers crispness lost during downscaling.
# At 64px (near render size) this can be subtle. Tweak USM_PERCENT if needed.
USM_RADIUS    = 0.8   # blur radius before edge detection (px)
USM_PERCENT   = 60    # sharpening strength (%)
USM_THRESHOLD = 3     # min brightness delta to sharpen (0-255)

# ── Core ──────────────────────────────────────────────────────────────────────

def process_image(src: Path, dst: Path, size: int) -> None:
    from PIL import Image, ImageFilter

    img = Image.open(src).convert("RGBA")
    original_size = img.size

    if original_size == (size, size):
        print(f"  skip   {src.name}  (already {size}x{size})")
        return

    img = img.resize((size, size), Image.LANCZOS)
    img = img.filter(ImageFilter.UnsharpMask(
        radius=USM_RADIUS,
        percent=USM_PERCENT,
        threshold=USM_THRESHOLD,
    ))

    dst.parent.mkdir(parents=True, exist_ok=True)
    img.save(dst, format="PNG", optimize=True)
    print(f"  {original_size[0]}x{original_size[1]} → {size}x{size}   {dst.relative_to(dst.parents[3]) if len(dst.parts) > 3 else dst.name}")


def run(input_dir: str, output_dir: str, size: int) -> None:
    script_dir = Path(__file__).parent

    in_dir  = Path(input_dir)  if Path(input_dir).is_absolute()  else (script_dir / input_dir).resolve()
    out_dir = Path(output_dir) if Path(output_dir).is_absolute() else (script_dir / output_dir).resolve()

    pngs = sorted(in_dir.rglob("*.png"))
    if not pngs:
        print(f"[warn] No PNGs found in {in_dir}")
        return

    print(f"\nInput:  {in_dir}")
    print(f"Output: {out_dir}")
    print(f"Size:   {size}x{size}\n")

    for src in pngs:
        dst = out_dir / src.name
        process_image(src, dst, size)

    print(f"\nDone. Processed {len(pngs)} file(s).")
    print(f"Runtime assets updated at:\n  {out_dir}")


# ── CLI ───────────────────────────────────────────────────────────────────────

if __name__ == "__main__":
    try:
        from PIL import Image  # noqa: F401
    except ImportError:
        print("Pillow not installed.  Run:  pip install Pillow")
        sys.exit(1)

    parser = argparse.ArgumentParser(description="Downscale staging icons to game-ready PNGs.")
    parser.add_argument("--input",  "-i", default=DEFAULT_INPUT,  help=f"Source folder (default: {DEFAULT_INPUT})")
    parser.add_argument("--output", "-o", default=DEFAULT_OUTPUT, help=f"Dest folder   (default: {DEFAULT_OUTPUT})")
    parser.add_argument("--size",   "-s", type=int, default=DEFAULT_SIZE, help=f"Output px size (default: {DEFAULT_SIZE})")
    args = parser.parse_args()

    run(args.input, args.output, args.size)
