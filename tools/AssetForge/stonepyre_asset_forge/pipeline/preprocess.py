"""Image preprocessing: validation, background removal, and crop/center."""

import shutil
from pathlib import Path
from typing import Optional

from PIL import Image


def validate_image(path: Path) -> None:
    """Raise if the path does not point to a readable image file."""
    if not path.exists():
        raise FileNotFoundError(f"Input file not found: {path}")
    if not path.is_file():
        raise ValueError(f"Input path is not a file: {path}")
    try:
        with Image.open(path) as img:
            img.verify()
    except Exception as exc:
        raise ValueError(f"Cannot read image '{path}': {exc}") from exc


def copy_to_temp(src: Path, temp_dir: Path) -> Path:
    """Copy the source image into the temp working directory."""
    temp_dir.mkdir(parents=True, exist_ok=True)
    dest = temp_dir / src.name
    shutil.copy2(src, dest)
    return dest


def remove_background(image_path: Path, output_path: Optional[Path] = None) -> Path:
    """
    Remove the background using rembg.
    Returns the path to the RGBA PNG with background removed.
    """
    try:
        from rembg import remove as rembg_remove
    except ImportError:
        raise ImportError(
            "rembg is not installed. Run: pip install rembg\n"
            "Or skip background removal with --skip-bg-removal."
        )

    if output_path is None:
        output_path = image_path.parent / (image_path.stem + "_nobg.png")

    with open(image_path, "rb") as f:
        input_data = f.read()

    output_data = rembg_remove(input_data)

    with open(output_path, "wb") as f:
        f.write(output_data)

    return output_path


def crop_to_subject(image_path: Path, padding: float = 0.05) -> Path:
    """
    Crop the image tightly around the non-transparent subject.
    Only meaningful on RGBA images (e.g. after background removal).
    Returns the path to the cropped image (overwrites in-place).
    """
    with Image.open(image_path) as img:
        if img.mode != "RGBA":
            return image_path

        bbox = img.getbbox()
        if bbox is None:
            return image_path

        w, h = img.size
        x0, y0, x1, y1 = bbox
        pad_x = int((x1 - x0) * padding)
        pad_y = int((y1 - y0) * padding)

        x0 = max(0, x0 - pad_x)
        y0 = max(0, y0 - pad_y)
        x1 = min(w, x1 + pad_x)
        y1 = min(h, y1 + pad_y)

        cropped = img.crop((x0, y0, x1, y1))
        cropped.save(image_path)

    return image_path


def ensure_rgba_png(image_path: Path) -> Path:
    """Convert the image to RGBA PNG if it is not already, saving alongside the original."""
    with Image.open(image_path) as img:
        if img.format == "PNG" and img.mode == "RGBA":
            return image_path
        rgba = img.convert("RGBA")
        out = image_path.parent / (image_path.stem + ".png")
        rgba.save(out, "PNG")
        return out
