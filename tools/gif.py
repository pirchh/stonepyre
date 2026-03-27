#!/usr/bin/env python3
"""
Folder -> GIF

Pick a folder containing PNG frames (e.g. 4 frames), and export an animated GIF.

Requirements:
  pip install pillow tkinterdnd2  (tkinter is built-in on Windows/macOS usually)
  (only Pillow is actually required here)

Usage:
  python tools/folder_to_gif.py
  python tools/folder_to_gif.py --folder "C:\path\to\frames" --fps 6 --loop 0
"""

from __future__ import annotations

import argparse
import os
from pathlib import Path
from typing import List, Optional

from PIL import Image

# tkinter is stdlib; on some minimal linux builds it may be missing
def pick_folder_dialog(title: str = "Select folder with PNG frames") -> Optional[Path]:
    try:
        import tkinter as tk
        from tkinter import filedialog
    except Exception as e:
        print(f"[ERROR] tkinter not available for folder picker: {e}")
        return None

    root = tk.Tk()
    root.withdraw()
    root.wm_attributes("-topmost", 1)
    folder = filedialog.askdirectory(title=title)
    root.destroy()
    if not folder:
        return None
    return Path(folder)

def list_pngs(folder: Path) -> List[Path]:
    # sorted by filename; works well if you use 0.png, 1.png... or frame_01.png...
    pngs = sorted([p for p in folder.iterdir() if p.is_file() and p.suffix.lower() == ".png"])
    return pngs

def load_frames(paths: List[Path]) -> List[Image.Image]:
    frames: List[Image.Image] = []
    for p in paths:
        img = Image.open(p).convert("RGBA")
        frames.append(img)
    return frames

def save_gif(
    frames: List[Image.Image],
    out_path: Path,
    fps: float,
    loop: int,
) -> None:
    if not frames:
        raise ValueError("No frames to save.")

    # Pillow expects duration per frame in milliseconds
    duration_ms = int(round(1000.0 / max(fps, 0.01)))

    # ensure output directory exists
    out_path.parent.mkdir(parents=True, exist_ok=True)

    first, rest = frames[0], frames[1:]
    first.save(
        out_path,
        save_all=True,
        append_images=rest,
        duration=duration_ms,
        loop=loop,          # 0 = forever
        disposal=2,         # clear between frames (helps with transparency artifacts)
        optimize=False,
    )

def main() -> None:
    ap = argparse.ArgumentParser(description="Pick a folder of PNGs and export a GIF.")
    ap.add_argument("--folder", type=str, default=None, help="Folder containing PNG frames.")
    ap.add_argument("--out", type=str, default=None, help="Output gif path. Defaults to <folder>/<foldername>.gif")
    ap.add_argument("--fps", type=float, default=6.0, help="Frames per second.")
    ap.add_argument("--loop", type=int, default=0, help="GIF loop count. 0 = infinite.")
    args = ap.parse_args()

    folder = Path(args.folder) if args.folder else pick_folder_dialog()
    if folder is None:
        print("[CANCELLED] No folder selected.")
        return
    folder = folder.resolve()

    if not folder.exists() or not folder.is_dir():
        print(f"[ERROR] Folder not found or not a directory: {folder}")
        return

    pngs = list_pngs(folder)
    if not pngs:
        print(f"[ERROR] No .png files found in: {folder}")
        return

    print(f"[INFO] Found {len(pngs)} PNG(s):")
    for p in pngs:
        print(f"  - {p.name}")

    frames = load_frames(pngs)

    # default output: <folder>/<foldername>.gif
    out_path = Path(args.out).resolve() if args.out else (folder / f"{folder.name}.gif")

    # sanity: all frames same size (common for sprite frames)
    w0, h0 = frames[0].size
    for i, fr in enumerate(frames[1:], start=1):
        if fr.size != (w0, h0):
            print(f"[WARN] Frame size mismatch: {pngs[i].name} is {fr.size}, expected {(w0, h0)}")
            # You can choose to resize instead; for v1 we just warn.

    save_gif(frames, out_path, fps=args.fps, loop=args.loop)
    print(f"[DONE] Wrote: {out_path}")

if __name__ == "__main__":
    main()
