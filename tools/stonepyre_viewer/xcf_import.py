# tools/stonepyre_viewer/xcf_importer.py
from __future__ import annotations

import re
import subprocess
from dataclasses import dataclass
from pathlib import Path
from typing import Dict, List, Optional, Tuple

from PIL import Image

from .config import (
    GIMP_CONSOLE_EXE,
    LAYERED_OUTPUTS_DIR,
    GREYSCALE_OUTPUTS_DIR,
    RAW_LAYER_EXPORTS_DIR,
    PET_CANVAS_W,
    PET_CANVAS_H,
    PET_BOTTOM_PAD,
    DIRECTIONS,
    PET_ACTIONS,
    PET_EXPECTED_FRAMES_PER_DIR,
    PETS_ROOT,
)
from .greyscale import stonepyre_greyscale
from .pet_tools import sanitize_pet_name

_LAYER_RE = re.compile(r"^(north|east|south|west)_(idle|walk)_(\d{2})$", re.IGNORECASE)


@dataclass(frozen=True)
class XcfImportResult:
    pet_name: str
    written_pairs: List[Tuple[Path, Path]]  # (structured_greyscale, template_png)

    exported_raw_count: int
    parsed_layer_count: int
    skipped_layer_count: int

    raw_export_dir: Path
    structured_greyscale_root: Path
    flat_greyscale_dir: Path

    greyscale_xcf_out: Optional[Path]
    greyscale_xcf_next_to_original: Optional[Path]


def derive_pet_name_from_xcf(xcf_path: Path) -> str:
    base = xcf_path.stem.lower().strip().replace(" ", "_")
    if base.endswith("_greyscale"):
        base = base[: -len("_greyscale")]
    if base.endswith("_gray"):
        base = base[: -len("_gray")]
    return sanitize_pet_name(base)


def _parse_layer_stem_strict(stem: str) -> Tuple[Optional[str], Optional[str], Optional[int]]:
    m = _LAYER_RE.match(stem.strip())
    if not m:
        return None, None, None

    direction = m.group(1).lower()
    action = m.group(2).lower()
    slot = int(m.group(3))

    if direction not in DIRECTIONS:
        return None, None, None
    if action not in PET_ACTIONS:
        return None, None, None
    if not (1 <= slot <= PET_EXPECTED_FRAMES_PER_DIR):
        return None, None, None

    return direction, action, slot


def _run(cmd: List[str]) -> Tuple[str, str]:
    proc = subprocess.run(cmd, capture_output=True, text=True)
    if proc.returncode != 0:
        raise RuntimeError(
            f"GIMP batch failed ({proc.returncode}).\n"
            f"CMD:\n{' '.join(cmd)}\n\n"
            f"STDOUT:\n{proc.stdout}\n\n"
            f"STDERR:\n{proc.stderr}"
        )
    return proc.stdout, proc.stderr


def _clear_pngs(folder: Path) -> None:
    folder.mkdir(parents=True, exist_ok=True)
    for p in folder.glob("*.png"):
        try:
            p.unlink()
        except Exception:
            pass


def export_xcf_layers_to_folder(xcf_path: Path, out_dir: Path) -> List[Path]:
    # (same scheme-based exporter you already have — unchanged)
    # KEEP YOUR EXISTING IMPLEMENTATION HERE.
    # ---- START: (paste your existing export_xcf_layers_to_folder body) ----
    if not GIMP_CONSOLE_EXE.exists():
        raise FileNotFoundError(f"GIMP console not found: {GIMP_CONSOLE_EXE}")

    xcf_path = xcf_path.resolve()
    out_dir = out_dir.resolve()
    _clear_pngs(out_dir)

    xcf_arg = str(xcf_path).replace("\\", "/")
    out_arg = str(out_dir).replace("\\", "/")
    cw = int(PET_CANVAS_W)
    ch = int(PET_CANVAS_H)
    pad = int(PET_BOTTOM_PAD)

    scheme = f"""
(begin
  (define (sp-sanitize-filename s)
    (let* ((n (string-length s)))
      (define (bad-char? c)
        (or (char=? c #\\/) (char=? c #\\\\) (char=? c #\\:) (char=? c #\\*)
            (char=? c #\\?) (char=? c #\\") (char=? c #\\<) (char=? c #\\>)
            (char=? c #\\|) (char=? c #\\space)))
      (define (loop i acc)
        (if (= i n)
          acc
          (let* ((c (string-ref s i))
                 (c2 (if (bad-char? c) #\\_ c)))
            (loop (+ i 1) (string-append acc (make-string 1 c2))))))
      (loop 0 "")))

  (define (sp-set-visible-rec item keep)
    (let* ((isgroup (car (gimp-item-is-group item))))
      (gimp-item-set-visible item keep)
      (if (= isgroup TRUE)
        (let* ((children-info (gimp-item-get-children item))
               (num (car children-info))
               (arr (cadr children-info)))
          (let loop ((i 0))
            (if (< i num)
              (begin
                (sp-set-visible-rec (aref arr i) keep)
                (loop (+ i 1)))))))))

  (define (sp-hide-all img)
    (let* ((layers-info (gimp-image-get-layers img))
           (num (car layers-info))
           (layers (cadr layers-info)))
      (let loop ((i 0))
        (if (< i num)
          (begin
            (sp-set-visible-rec (aref layers i) FALSE)
            (loop (+ i 1)))))))

  (define (sp-enable-items-by-name img nm)
    (let* ((layers-info (gimp-image-get-layers img))
           (num (car layers-info))
           (layers (cadr layers-info)))

      (define (sp-enable-by-name it)
        (let* ((isgroup (car (gimp-item-is-group it)))
               (name2 (car (gimp-item-get-name it))))
          (if (string=? name2 nm)
            (gimp-item-set-visible it TRUE))
          (if (= isgroup TRUE)
            (let* ((children-info (gimp-item-get-children it))
                   (cnum (car children-info))
                   (carr (cadr children-info)))
              (let loop2 ((j 0))
                (if (< j cnum)
                  (begin
                    (sp-enable-by-name (aref carr j))
                    (loop2 (+ j 1)))))))))

      (let loop ((i 0))
        (if (< i num)
          (begin
            (sp-enable-by-name (aref layers i))
            (loop (+ i 1)))))))

  (define (sp-baseline-align layer imgH pad)
    (let* ((bb (gimp-drawable-mask-bounds layer))
           (nonempty (car bb))
           (y2 (list-ref bb 4)))
      (if (= nonempty TRUE)
        (let* ((dy (- (- imgH pad) y2))
               (off (gimp-drawable-offsets layer))
               (ox (car off))
               (oy (cadr off)))
          (gimp-layer-set-offsets layer ox (+ oy dy))))))

  (define (sp-export-leaf img leaf-name out-path)
    (let* ((dup (car (gimp-image-duplicate img)))
           (W {cw})
           (H {ch}))
      (sp-hide-all dup)
      (sp-enable-items-by-name dup leaf-name)

      (let* ((merged (car (gimp-image-merge-visible-layers dup CLIP-TO-IMAGE))))
        (gimp-layer-resize-to-image-size merged)
        (gimp-layer-set-offsets merged 0 0)
        (gimp-edit-copy merged))

      (gimp-image-delete dup)

      (let* ((newimg (car (gimp-image-new W H RGB)))
             (base (car (gimp-layer-new newimg W H RGBA-IMAGE leaf-name 100 NORMAL-MODE))))
        (gimp-image-insert-layer newimg base 0 0)
        (gimp-drawable-fill base TRANSPARENT-FILL)

        (let* ((float (car (gimp-edit-paste base FALSE))))
          (gimp-floating-sel-anchor float))

        (sp-baseline-align base H {pad})

        (gimp-layer-resize-to-image-size base)
        (gimp-layer-set-offsets base 0 0)

        (file-png-save2 RUN-NONINTERACTIVE newimg base out-path out-path
          0 9 0 0 0 0 0 0 0)

        (gimp-image-delete newimg))))

  (define (sp-export-item-recursive img item out-dir)
    (let* ((isgroup (car (gimp-item-is-group item))))
      (if (= isgroup TRUE)
        (let* ((children-info (gimp-item-get-children item))
               (num (car children-info))
               (arr (cadr children-info)))
          (let loop ((i 0))
            (if (< i num)
              (begin
                (sp-export-item-recursive img (aref arr i) out-dir)
                (loop (+ i 1))))))

        (let* ((nm (car (gimp-item-get-name item)))
               (nm2 (sp-sanitize-filename nm))
               (out-path (string-append out-dir "/" nm2 ".png")))
          (sp-export-leaf img nm out-path)))))

  (define (sp-export-xcf-layers-recursive xcf-path out-dir)
    (let* ((img (car (gimp-file-load RUN-NONINTERACTIVE xcf-path xcf-path))))
      (let* ((layers-info (gimp-image-get-layers img))
             (num (car layers-info))
             (layers (cadr layers-info)))
        (let loop ((i 0))
          (if (< i num)
            (begin
              (sp-export-item-recursive img (aref layers i) out-dir)
              (loop (+ i 1))))))

      (gimp-image-delete img)))

  (sp-export-xcf-layers-recursive "{xcf_arg}" "{out_arg}")
)
"""
    cmd = [str(GIMP_CONSOLE_EXE), "-i", "-b", scheme, "-b", "(gimp-quit 0)"]
    stdout, stderr = _run(cmd)

    exported = sorted([p for p in out_dir.iterdir() if p.is_file() and p.suffix.lower() == ".png"])
    if not exported:
        raise RuntimeError(
            "GIMP export produced 0 PNGs.\n\n"
            f"XCF: {xcf_path}\nOUT: {out_dir}\n\n"
            f"STDOUT:\n{stdout}\n\n"
            f"STDERR:\n{stderr}\n"
        )

    return exported
    # ---- END ----


def _greyscale_png_to(src_png: Path, dst_png: Path) -> None:
    img = Image.open(src_png).convert("RGBA")
    g = stonepyre_greyscale(img)
    dst_png.parent.mkdir(parents=True, exist_ok=True)
    g.save(dst_png)


def _copy_to_templates(*, pet_name: str, direction: str, action: str, slot: int, greyscale_png: Path) -> Path:
    pet_name = sanitize_pet_name(pet_name)
    dest_dir = PETS_ROOT / pet_name / action / direction
    dest_dir.mkdir(parents=True, exist_ok=True)
    dest = dest_dir / f"{action}_{slot:02d}.png"
    dest.write_bytes(greyscale_png.read_bytes())
    return dest


def rebuild_greyscale_xcf_from_folder(*, greyscale_flat_dir: Path, out_xcf: Path) -> None:
    # KEEP YOUR EXISTING IMPLEMENTATION HERE (unchanged).
    # You already have _write_build_scm + rebuild_greyscale_xcf_from_folder; just keep it.
    from .xcf_rebuild import rebuild_greyscale_xcf_from_folder as _impl  # if you split it
    _impl(greyscale_flat_dir=greyscale_flat_dir, out_xcf=out_xcf)


def run_xcf_import(
    xcf_path: Path,
    *,
    pet_name: Optional[str] = None,
    scale: int = 1,  # kept for compatibility
) -> XcfImportResult:
    xcf_path = xcf_path.resolve()
    if pet_name is None:
        pet_name = derive_pet_name_from_xcf(xcf_path)
    pet_name = sanitize_pet_name(pet_name)

    raw_dir = (RAW_LAYER_EXPORTS_DIR / pet_name).resolve()
    structured_root = (GREYSCALE_OUTPUTS_DIR / pet_name).resolve()
    flat_grey_dir = (GREYSCALE_OUTPUTS_DIR / pet_name / "_flat").resolve()
    xcf_out_dir = (LAYERED_OUTPUTS_DIR / pet_name).resolve()

    _clear_pngs(raw_dir)
    _clear_pngs(flat_grey_dir)

    exported_raw = export_xcf_layers_to_folder(xcf_path, raw_dir)

    walk01_by_dir: Dict[str, Path] = {}
    results: List[Tuple[Path, Path]] = []

    parsed = 0
    skipped = 0

    for raw_png in exported_raw:
        direction, action, slot = _parse_layer_stem_strict(raw_png.stem)
        if not direction or not action or not slot:
            skipped += 1
            continue

        parsed += 1

        grey_flat = flat_grey_dir / f"{raw_png.stem}.png"
        _greyscale_png_to(raw_png, grey_flat)

        grey_struct = structured_root / action / direction / f"{action}_{slot:02d}.png"
        grey_struct.parent.mkdir(parents=True, exist_ok=True)
        grey_struct.write_bytes(grey_flat.read_bytes())

        template_png = _copy_to_templates(
            pet_name=pet_name,
            direction=direction,
            action=action,
            slot=slot,
            greyscale_png=grey_flat,
        )
        results.append((grey_struct, template_png))

        if action == "walk" and slot == 1:
            walk01_by_dir[direction] = template_png

    # Auto idle_01 from walk_01 per direction
    for d in DIRECTIONS:
        walk01 = walk01_by_dir.get(d)
        if not walk01 or not walk01.exists():
            continue

        idle_dir = (PETS_ROOT / pet_name / "idle" / d)
        idle_dir.mkdir(parents=True, exist_ok=True)
        idle01 = idle_dir / "idle_01.png"
        idle01.write_bytes(walk01.read_bytes())

        idle_struct = structured_root / "idle" / d / "idle_01.png"
        idle_struct.parent.mkdir(parents=True, exist_ok=True)
        idle_struct.write_bytes(walk01.read_bytes())

        idle_flat = flat_grey_dir / f"{d}_idle_01.png"
        idle_flat.write_bytes(walk01.read_bytes())

        results.append((idle_struct, idle01))

    # Rebuild greyscale XCF from flat greyscale PNGs
    greyscale_xcf_out: Optional[Path] = None
    greyscale_xcf_next_to_original: Optional[Path] = None

    xcf_out_dir.mkdir(parents=True, exist_ok=True)
    out1 = xcf_out_dir / f"{pet_name}_greyscale_out.xcf"
    out2 = xcf_path.parent / f"{pet_name}_greyscale_out.xcf"
    try:
        rebuild_greyscale_xcf_from_folder(greyscale_flat_dir=flat_grey_dir, out_xcf=out1)
        out2.write_bytes(out1.read_bytes())
        greyscale_xcf_out = out1
        greyscale_xcf_next_to_original = out2
    except Exception:
        pass

    return XcfImportResult(
        pet_name=pet_name,
        written_pairs=results,
        exported_raw_count=len(exported_raw),
        parsed_layer_count=parsed,
        skipped_layer_count=skipped,
        raw_export_dir=raw_dir,
        structured_greyscale_root=structured_root,
        flat_greyscale_dir=flat_grey_dir,
        greyscale_xcf_out=greyscale_xcf_out,
        greyscale_xcf_next_to_original=greyscale_xcf_next_to_original,
    )