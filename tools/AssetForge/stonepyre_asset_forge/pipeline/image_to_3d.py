"""
Image-to-3D backend abstraction.

Concrete backends:
  - TripoSRBackend   — uses the TripoSR open-source model (default, recommended)
  - StableFast3DBackend — Stable Fast 3D (stub, ready to implement)
  - Hunyuan3DBackend    — Hunyuan3D (stub, ready to implement)
  - StubBackend         — generates a placeholder cube mesh for pipeline testing

Select a backend by name via --backend (triposr | stable_fast_3d | hunyuan3d | stub).
"""

from __future__ import annotations

import abc
import logging
import sys
from pathlib import Path
from typing import Optional

# TripoSR is not a pip package — it's cloned as a sibling folder called TripoSR/.
# We add it to sys.path so `from tsr.system import TSR` resolves correctly.
_TRIPOSR_REPO = Path(__file__).parent.parent.parent / "TripoSR"

# Hunyuan3D-2 is similarly cloned as a sibling folder called Hunyuan3D-2/.
_HUNYUAN3D_REPO = Path(__file__).parent.parent.parent / "Hunyuan3D-2"


def _ensure_triposr_on_path() -> None:
    """Add the TripoSR repo directory to sys.path if present."""
    if _TRIPOSR_REPO.exists() and str(_TRIPOSR_REPO) not in sys.path:
        sys.path.insert(0, str(_TRIPOSR_REPO))


def _ensure_hunyuan3d_on_path() -> None:
    """Add the Hunyuan3D-2 repo directory to sys.path if present."""
    if _HUNYUAN3D_REPO.exists() and str(_HUNYUAN3D_REPO) not in sys.path:
        sys.path.insert(0, str(_HUNYUAN3D_REPO))


# ---------------------------------------------------------------------------
# Abstract base
# ---------------------------------------------------------------------------

class ImageTo3DBackend(abc.ABC):
    """Interface every backend must implement."""

    @abc.abstractmethod
    def generate(self, image_path: Path, output_path: Path, options: dict) -> Path:
        """
        Convert a 2D image into a 3D mesh file.

        Args:
            image_path:  Path to the preprocessed input image (RGBA PNG).
            output_path: Desired path for the output mesh (.glb or .obj).
            options:     Dict of generation options (seed, texture_size, etc.)

        Returns:
            Path to the generated mesh file.
        """
        ...


# ---------------------------------------------------------------------------
# TripoSR backend
# ---------------------------------------------------------------------------

class TripoSRBackend(ImageTo3DBackend):
    """
    Runs TripoSR (https://github.com/VAST-AI-Research/TripoSR) locally.

    Installation:
        git clone https://github.com/VAST-AI-Research/TripoSR   (into the project root)
        pip install omegaconf==2.3.0 einops==0.7.0 transformers==4.35.0 huggingface-hub imageio moderngl xatlas
        pip install git+https://github.com/tatsy/torchmcubes.git
        # model weights (~1 GB) are downloaded automatically on first run via HuggingFace

    Requires:
        torch (CUDA), torchvision, huggingface_hub, trimesh, einops, omegaconf, torchmcubes
    """

    def __init__(self, device: str = "cuda", logger: Optional[logging.Logger] = None):
        self.device = device
        self.logger = logger or logging.getLogger(__name__)
        self._model = None

    def _load_model(self) -> None:
        if self._model is not None:
            return
        self.logger.debug("Loading TripoSR model weights (first run may download ~1 GB)...")
        _ensure_triposr_on_path()
        if not _TRIPOSR_REPO.exists():
            raise ImportError(
                "TripoSR repo not found. Expected it at:\n"
                f"  {_TRIPOSR_REPO}\n\n"
                "Clone it with:\n"
                "  git clone https://github.com/VAST-AI-Research/TripoSR\n"
                "Then install its dependencies:\n"
                "  pip install omegaconf==2.3.0 einops==0.7.0 transformers==4.35.0 huggingface-hub imageio moderngl xatlas\n"
                "  pip install git+https://github.com/tatsy/torchmcubes.git\n"
                "Or use --backend stub to test the pipeline without a model."
            )
        try:
            import torch
            from tsr.system import TSR  # type: ignore

            self._model = TSR.from_pretrained(
                "stabilityai/TripoSR",
                config_name="config.yaml",
                weight_name="model.ckpt",
            )
            self._model = self._model.to(self.device)
            self._model.renderer.set_chunk_size(131072)
        except ImportError as exc:
            raise ImportError(
                "TripoSR is not installed.\n"
                "Clone https://github.com/VAST-AI-Research/TripoSR and run: pip install -e .\n"
                "Or use --backend stub to test the rest of the pipeline."
            ) from exc

    def generate(self, image_path: Path, output_path: Path, options: dict) -> Path:
        import torch
        from PIL import Image

        _ensure_triposr_on_path()
        self._load_model()

        from tsr.utils import resize_foreground  # type: ignore — must import after path is set

        # Load RGBA (background should already be removed by the pipeline).
        raw = Image.open(image_path).convert("RGBA")

        # TripoSR's own resize_foreground: crops to the subject bbox,
        # pads to square, and adds headroom (0.85 = subject fills 85% of frame).
        raw = resize_foreground(raw, 0.85)

        # Composite onto a neutral mid-gray background and convert to RGB.
        # Do NOT use white — the subject may be light-coloured and become invisible.
        bg = Image.new("RGBA", raw.size, (127, 127, 127, 255))
        bg.paste(raw, mask=raw.split()[3])
        image = bg.convert("RGB")

        with torch.no_grad():
            scene_codes = self._model([image], device=self.device)

        # Extract mesh
        meshes = self._model.extract_mesh(scene_codes, has_vertex_color=False, resolution=512)
        mesh = meshes[0]

        # TripoSR's coordinate system puts the model on its side.
        # Apply the same orientation fix their gradio app uses to stand it upright.
        import numpy as np
        import trimesh as _trimesh
        mesh.apply_transform(_trimesh.transformations.rotation_matrix(-np.pi / 2, [1, 0, 0]))
        mesh.apply_transform(_trimesh.transformations.rotation_matrix(np.pi / 2, [0, 1, 0]))

        output_path.parent.mkdir(parents=True, exist_ok=True)
        mesh.export(str(output_path))
        return output_path


# ---------------------------------------------------------------------------
# Stable Fast 3D backend (stub — ready to implement)
# ---------------------------------------------------------------------------

class StableFast3DBackend(ImageTo3DBackend):
    """
    Stable Fast 3D backend.

    TODO: Implement once SF3D is available.

    Installation reference:
        https://github.com/Stability-AI/stable-fast-3d
        pip install stable-fast-3d  (or install from source)
    """

    def generate(self, image_path: Path, output_path: Path, options: dict) -> Path:
        raise NotImplementedError(
            "StableFast3DBackend is not yet implemented.\n"
            "See stonepyre_asset_forge/pipeline/image_to_3d.py to add it.\n"
            "Use --backend triposr or --backend stub instead."
        )


# ---------------------------------------------------------------------------
# Hunyuan3D-2 backend
# ---------------------------------------------------------------------------

class Hunyuan3DBackend(ImageTo3DBackend):
    """
    Runs Hunyuan3D-2 (https://github.com/Tencent/Hunyuan3D-2) locally.

    Installation:
        git clone https://github.com/Tencent/Hunyuan3D-2   (into the project root)
        cd Hunyuan3D-2 && pip install -e .
        # model weights (~8 GB) are downloaded automatically on first run via HuggingFace

    Requires:
        torch (CUDA), diffusers, transformers>=4.48.0, trimesh, einops, accelerate
    """

    def __init__(self, device: str = "cuda", logger: Optional[logging.Logger] = None):
        self.device = device
        self.logger = logger or logging.getLogger(__name__)
        self._pipeline = None

    def _load_model(self) -> None:
        if self._pipeline is not None:
            return

        _ensure_hunyuan3d_on_path()

        if not _HUNYUAN3D_REPO.exists():
            raise ImportError(
                "Hunyuan3D-2 repo not found. Expected it at:\n"
                f"  {_HUNYUAN3D_REPO}\n\n"
                "Clone it with:\n"
                "  git clone https://github.com/Tencent/Hunyuan3D-2\n"
                "Then install its dependencies:\n"
                "  cd Hunyuan3D-2 && pip install -e .\n"
                "Or use --backend triposr to fall back to TripoSR."
            )

        self.logger.debug("Loading Hunyuan3D-2 model weights (first run downloads ~8 GB)...")

        try:
            from hy3dgen.shapegen import Hunyuan3DDiTFlowMatchingPipeline  # type: ignore

            self._pipeline = Hunyuan3DDiTFlowMatchingPipeline.from_pretrained(
                "tencent/Hunyuan3D-2",
                subfolder="hunyuan3d-dit-v2-0",
                variant="fp16",
            )
        except ImportError as exc:
            raise ImportError(
                "hy3dgen not found. Install it with:\n"
                "  cd Hunyuan3D-2 && pip install -e .\n"
                "Or use --backend triposr instead."
            ) from exc

    def generate(self, image_path: Path, output_path: Path, options: dict) -> Path:
        import torch
        from PIL import Image

        _ensure_hunyuan3d_on_path()
        self._load_model()

        # Load the background-removed RGBA image produced by the pipeline
        image = Image.open(image_path).convert("RGBA")

        seed = options.get("seed")
        generator = torch.manual_seed(seed) if seed is not None else None

        self.logger.debug(
            f"Hunyuan3D-2 generating mesh (seed={seed}, steps=50, octree_resolution=380) ..."
        )

        mesh = self._pipeline(
            image=image,
            num_inference_steps=50,
            octree_resolution=380,
            num_chunks=20000,
            generator=generator,
            output_type="trimesh",
        )[0]

        output_path.parent.mkdir(parents=True, exist_ok=True)
        mesh.export(str(output_path))
        self.logger.debug(f"Hunyuan3D-2 mesh exported: {output_path}")
        return output_path


# ---------------------------------------------------------------------------
# Stub backend — for testing the pipeline without a real AI model
# ---------------------------------------------------------------------------

class StubBackend(ImageTo3DBackend):
    """
    Generates a simple placeholder cube mesh so the full pipeline can be
    exercised without any AI model installed.

    This is intentionally a rough stand-in. Replace with a real backend
    for actual asset generation.
    """

    def generate(self, image_path: Path, output_path: Path, options: dict) -> Path:
        try:
            import trimesh
            import numpy as np
        except ImportError:
            raise ImportError("trimesh and numpy are required for StubBackend. Run: pip install trimesh numpy")

        import numpy as np

        # Build a simple box mesh that vaguely represents a character silhouette
        mesh = trimesh.creation.box(extents=[0.6, 0.3, 1.8])

        # Attempt to read the dominant colour from the input image for a rough texture feel
        try:
            from PIL import Image
            img = Image.open(image_path).convert("RGB").resize((16, 16))
            colours = list(img.getdata())
            avg_r = int(sum(c[0] for c in colours) / len(colours))
            avg_g = int(sum(c[1] for c in colours) / len(colours))
            avg_b = int(sum(c[2] for c in colours) / len(colours))
            vertex_colours = np.tile([avg_r, avg_g, avg_b, 255], (len(mesh.vertices), 1))
            mesh.visual.vertex_colors = vertex_colours
        except Exception:
            pass  # colour sampling is best-effort

        output_path.parent.mkdir(parents=True, exist_ok=True)
        mesh.export(str(output_path))
        return output_path


# ---------------------------------------------------------------------------
# Factory
# ---------------------------------------------------------------------------

_BACKENDS: dict[str, type[ImageTo3DBackend]] = {
    "triposr": TripoSRBackend,
    "stable_fast_3d": StableFast3DBackend,
    "hunyuan3d": Hunyuan3DBackend,
    "stub": StubBackend,
}


def get_backend(name: str, **kwargs) -> ImageTo3DBackend:
    """Return an instantiated backend by name."""
    key = name.lower().replace("-", "_")
    if key not in _BACKENDS:
        available = ", ".join(_BACKENDS.keys())
        raise ValueError(f"Unknown backend '{name}'. Available: {available}")
    cls = _BACKENDS[key]
    if cls is StableFast3DBackend:
        return cls()
    if cls in (TripoSRBackend, Hunyuan3DBackend):
        return cls(**kwargs)
    return cls()
