# tools/world_editor/terrain_gen.py
from __future__ import annotations

import math
from typing import Iterable, Tuple

from .io_chunks import ChunkStore, set_tile
from .world_layout import WorldLayout


def _fade(t: float) -> float:
    return t * t * (3.0 - 2.0 * t)


def _lerp(a: float, b: float, t: float) -> float:
    return a + (b - a) * t


def _hash2i(x: int, y: int, seed: int) -> int:
    n = x * 374761393 + y * 668265263 + seed * 1442695040888963407
    n = (n ^ (n >> 13)) & 0xFFFFFFFFFFFFFFFF
    n = (n * 1274126177) & 0xFFFFFFFFFFFFFFFF
    n = n ^ (n >> 16)
    return n & 0xFFFFFFFF


def _rand01(x: int, y: int, seed: int) -> float:
    return _hash2i(x, y, seed) / 0xFFFFFFFF


def value_noise_2d(x: float, y: float, seed: int) -> float:
    x0 = math.floor(x)
    y0 = math.floor(y)
    x1 = x0 + 1
    y1 = y0 + 1

    tx = x - x0
    ty = y - y0

    sx = _fade(tx)
    sy = _fade(ty)

    v00 = _rand01(x0, y0, seed)
    v10 = _rand01(x1, y0, seed)
    v01 = _rand01(x0, y1, seed)
    v11 = _rand01(x1, y1, seed)

    ix0 = _lerp(v00, v10, sx)
    ix1 = _lerp(v01, v11, sx)
    return _lerp(ix0, ix1, sy)


def fbm_2d(x: float, y: float, seed: int, octaves: int = 4, lacunarity: float = 2.0, gain: float = 0.5) -> float:
    amp = 1.0
    freq = 1.0
    total = 0.0
    norm = 0.0

    for i in range(octaves):
        total += value_noise_2d(x * freq, y * freq, seed + i * 1013) * amp
        norm += amp
        amp *= gain
        freq *= lacunarity

    return total / norm if norm > 0 else 0.0


def generate_continent_layout(
    layout: WorldLayout,
    seed: int,
    land_scale: float = 18.0,
    edge_falloff: float = 1.15,
    land_bias: float = 0.50,
) -> int:
    """
    Fills layout.active_chunks using chunk-level noise.

    land_scale:
      higher = larger smoother continental features
    edge_falloff:
      higher = stronger ocean tendency near world edges
    land_bias:
      higher = more land
    """
    new_active = set()

    cx_center = (layout.min_cx + layout.max_cx) * 0.5
    cy_center = (layout.min_cy + layout.max_cy) * 0.5
    half_w = layout.width_chunks * 0.5
    half_h = layout.height_chunks * 0.5

    for cx in range(layout.min_cx, layout.max_cx + 1):
        for cy in range(layout.min_cy, layout.max_cy + 1):
            nx = (cx - cx_center) / max(1.0, half_w)
            ny = (cy - cy_center) / max(1.0, half_h)

            radial = math.sqrt(nx * nx + ny * ny)
            radial = min(1.5, radial)

            n1 = fbm_2d(cx / land_scale, cy / land_scale, seed, octaves=4)
            n2 = fbm_2d(cx / (land_scale * 0.45), cy / (land_scale * 0.45), seed + 999, octaves=3)

            val = n1 * 0.75 + n2 * 0.25
            val -= max(0.0, radial) ** edge_falloff * 0.45

            if val >= land_bias:
                new_active.add((cx, cy))

    layout.active_chunks = new_active
    return len(new_active)


def _classify_tile(elev: float, moist: float, temp: float, slope: float) -> int:
    # water
    if elev < 0.42:
        return 3  # water

    # shoreline
    if elev < 0.455:
        return 2  # sand

    # cliffs
    if slope > 0.10 and elev > 0.54:
        return 5  # cliff

    # snowy mountains / cold north-south extremes
    if elev > 0.78 or (temp < 0.24 and elev > 0.62):
        return 6  # snow

    # stone highlands
    if elev > 0.68:
        return 4  # stone

    # swampy low wet regions
    if elev < 0.52 and moist > 0.70:
        return 7  # swamp

    # dirt patches in mid dryness
    if moist < 0.28:
        return 1  # dirt

    return 0  # grass


def bake_terrain_for_active_chunks(
    layout: WorldLayout,
    store: ChunkStore,
    chunk_size: int,
    seed: int,
) -> int:
    """
    Bakes all active chunks using continuous world-coordinate noise.
    Returns number of baked chunks.
    """
    baked = 0

    for cx, cy in sorted(layout.active_chunks):
        entry = store.get_or_create(cx, cy, fill_id=0)

        for ly in range(chunk_size):
            gy = cy * chunk_size + ly
            wy = gy / 220.0

            for lx in range(chunk_size):
                gx = cx * chunk_size + lx
                wx = gx / 220.0

                elev = fbm_2d(wx * 1.6, wy * 1.6, seed + 17, octaves=5)
                elev2 = fbm_2d(wx * 3.4, wy * 3.4, seed + 31, octaves=3)
                elev = elev * 0.75 + elev2 * 0.25

                moist = fbm_2d(wx * 1.2, wy * 1.2, seed + 101, octaves=4)
                temp_noise = fbm_2d(wx * 0.9, wy * 0.9, seed + 202, octaves=3)

                # latitude-like cooling toward top/bottom extremes
                lat = abs((gy / max(1.0, (layout.height_chunks * chunk_size))) * 2.0 - 1.0)
                temp = (1.0 - lat) * 0.65 + temp_noise * 0.35

                # approximate slope from neighboring samples
                elev_dx = fbm_2d((wx + 1.0 / 220.0) * 1.6, wy * 1.6, seed + 17, octaves=5)
                elev_dy = fbm_2d(wx * 1.6, (wy + 1.0 / 220.0) * 1.6, seed + 17, octaves=5)
                slope = abs(elev_dx - elev) + abs(elev_dy - elev)

                tile_id = _classify_tile(elev, moist, temp, slope)
                changed = set_tile(entry.chunk, lx, ly, chunk_size, tile_id)
                if changed:
                    pass

        store.mark_chunk_modified(cx, cy)
        baked += 1

    return baked