# StonepyreAssetForge

Local CLI tool that converts a 2D image into a rough low-poly 3D asset (.glb) for the Stonepyre game project.

Goal: OSRS-style, low-poly, prototyping-quality assets. Not Meshy.ai. Intentionally rough.

---

## Quick start

```
python generate_asset.py --input ./input/goblin.png
```

Full example:

```
python generate_asset.py ^
  --input ./input/goblin.png ^
  --output ./output/goblin_lowpoly.glb ^
  --target-tris 1000 ^
  --style osrs_character ^
  --flat-shading
```

Test the pipeline without an AI model installed:

```
python generate_asset.py --input ./input/goblin.png --backend stub --skip-bg-removal
```

---

## Installation

### 1. Python environment

```
python -m venv .venv
.venv\Scripts\activate       # Windows
pip install -r requirements.txt
```

### 2. PyTorch (CUDA — RTX 5060 Ti, sm_120)

```
pip install torch torchvision torchaudio --index-url https://download.pytorch.org/whl/cu128
```

### 3. TripoSR (image-to-3D model)

```
git clone https://github.com/VAST-AI-Research/TripoSR
cd TripoSR
pip install -e .
cd ..
```

Model weights (~1 GB) are downloaded automatically on first run via Hugging Face.

### 4. Blender

Download from https://www.blender.org/download/ and install.

If Blender is not on the default path, set the environment variable:

```
set BLENDER_PATH=C:\Program Files\Blender Foundation\Blender 4.2\blender.exe
```

Blender is used for high-quality mesh decimation, flat shading, and export.
If not available the pipeline falls back to trimesh-only processing.

---

## CLI reference

| Argument | Default | Description |
|---|---|---|
| `--input` | *(required)* | Path to the input image |
| `--output` | `output/<stem>_lowpoly.glb` | Output file path |
| `--style` | `osrs_character` | Style preset name |
| `--target-tris` | from style | Override triangle count |
| `--format` | `glb` | Output format: `glb`, `obj`, `stl` |
| `--backend` | `triposr` | Image-to-3D backend: `triposr`, `stub` |
| `--skip-bg-removal` | off | Skip rembg background removal |
| `--no-texture` | off | Strip textures from output |
| `--flat-shading` | off | Force flat shading |
| `--keep-temp` | off | Keep `temp/` files after generation |
| `--seed` | random | Seed for the image-to-3D model |
| `--verbose` | off | Enable debug logging |

---

## Style presets

Defined in `configs/styles.json`. Edit freely.

| Style | Target tris | Texture | Notes |
|---|---|---|---|
| `osrs_character` | 1200 | 256 | Humanoid characters |
| `osrs_creature` | 1000 | 256 | Monsters, animals |
| `osrs_prop` | 800 | 128 | Items, furniture |
| `osrs_tree` | 600 | 128 | Vegetation |
| `osrs_building` | 1500 | 256 | Structures |
| `raw` | none | 512 | No processing — raw model out |

---

## Backends

| Name | Status | Notes |
|---|---|---|
| `triposr` | **Ready** | Requires TripoSR installed from source |
| `stub` | **Ready** | Placeholder box mesh — for pipeline testing |
| `stable_fast_3d` | Stub | Not yet implemented |
| `hunyuan3d` | Stub | Not yet implemented |

---

## Pipeline

```
Input image
  ↓ validate + copy to temp/
  ↓ background removal (rembg)
  ↓ crop to subject
  ↓ image-to-3D (TripoSR / stub)
  ↓ save raw mesh to temp/
  ↓ Blender: decimate, flat shade, recalc normals, normalize scale, center origin
    (trimesh fallback if no Blender)
  ↓ export .glb to output/
```

---

## Folder structure

```
StonepyreAssetForge/
  generate_asset.py          CLI entry point
  requirements.txt
  README.md
  configs/
    styles.json              Style preset definitions
  input/                     Drop source images here
  output/                    Generated assets land here
  temp/                      Working files (auto-cleaned unless --keep-temp)
  stonepyre_asset_forge/
    cli.py                   Argument parsing + pipeline orchestration
    config.py                Config dataclasses + style loading
    logging_utils.py         Coloured step logger
    pipeline/
      preprocess.py          Image validation, bg removal, crop
      image_to_3d.py         Backend abstraction + implementations
      postprocess.py         Blender/trimesh post-processing orchestrator
      export.py              Final format export
    mesh/
      blender_lowpoly.py     Blender Python script (runs inside Blender)
      trimesh_cleanup.py     trimesh fallback post-processor
    styles/
      osrs.py                OSRS style constants
```

---

## Hardware target

- GPU: NVIDIA RTX 5060 Ti (16 GB VRAM) — primary AI workload
- CUDA 12.x required for sm_120 architecture
