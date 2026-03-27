#!/usr/bin/env python3
"""
Stonepyre Greyscale Quantizer

- Opens file browser to select input PNG
- Forces image to 400x600 (cover crop)
- Converts all visible pixels to Stonepyre 6-tone greyscale
- Saves automatically to tools/greyscale_outputs/

Palette:
1E1E1E
3A3A3A
555555
707070
8C8C8C
A8A8A8

Requires:
pip install pillow
"""

from __future__ import annotations

import os
from pathlib import Path
from typing import Tuple, List
from PIL import Image
import tkinter as tk
from tkinter import filedialog


# ---------------- CONFIG ----------------

TARGET_W = 400
TARGET_H = 600

OUTPUT_DIR = Path(
    r"C:\Users\ryanj\Development\GameProjects\Stonepyre\tools\greyscale_outputs"
)

PALETTE_HEX = [
    "1E1E1E",
    "3A3A3A",
    "555555",
    "707070",
    "8C8C8C",
    "A8A8A8",
]


# ---------------- HELPERS ----------------

def hex_to_rgb(h: str) -> Tuple[int, int, int]:
    h = h.strip().lstrip("#")
    return (int(h[0:2], 16), int(h[2:4], 16), int(h[4:6], 16))


PALETTE_RGB: List[Tuple[int, int, int]] = [hex_to_rgb(x) for x in PALETTE_HEX]


def nearest_palette_color(rgb: Tuple[int, int, int]) -> Tuple[int, int, int]:
    r, g, b = rgb
    best = PALETTE_RGB[0]
    best_dist = 10**18

    for pr, pg, pb in PALETTE_RGB:
        dr = r - pr
        dg = g - pg
        db = b - pb
        dist = dr * dr + dg * dg + db * db
        if dist < best_dist:
            best_dist = dist
            best = (pr, pg, pb)

    return best


def resize_cover(img: Image.Image, tw: int, th: int) -> Image.Image:
    w, h = img.size
    scale = max(tw / w, th / h)
    nw, nh = int(w * scale), int(h * scale)

    resized = img.resize((nw, nh), resample=Image.Resampling.LANCZOS)

    left = (nw - tw) // 2
    top = (nh - th) // 2

    return resized.crop((left, top, left + tw, top + th))


def quantize_to_palette(img: Image.Image) -> Image.Image:
    img = img.convert("RGBA")
    pixels = img.load()
    w, h = img.size

    for y in range(h):
        for x in range(w):
            r, g, b, a = pixels[x, y]

            if a == 0:
                continue

            nr, ng, nb = nearest_palette_color((r, g, b))
            pixels[x, y] = (nr, ng, nb, a)

    return img


# ---------------- MAIN ----------------

def main():
    OUTPUT_DIR.mkdir(parents=True, exist_ok=True)

    # Hide root tkinter window
    root = tk.Tk()
    root.withdraw()

    file_path = filedialog.askopenfilename(
        title="Select PNG to Convert",
        filetypes=[("PNG Files", "*.png")]
    )

    if not file_path:
        print("No file selected.")
        return

    input_path = Path(file_path)
    img = Image.open(input_path).convert("RGBA")

    # Resize to 400x600
    fitted = resize_cover(img, TARGET_W, TARGET_H)

    # Quantize to 6 hex palette
    quantized = quantize_to_palette(fitted)

    output_name = input_path.stem + "_greyscale.png"
    output_path = OUTPUT_DIR / output_name

    quantized.save(output_path)

    print(f"\n✔ Saved to: {output_path}")
    print(f"Resolution: {TARGET_W}x{TARGET_H}")
    print("Palette:", ", ".join(PALETTE_HEX))


if __name__ == "__main__":
    main()
