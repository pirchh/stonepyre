# tools/world_editor/io_chunks.py
from __future__ import annotations

import sys
from array import array
from dataclasses import dataclass
from pathlib import Path
from typing import Dict, Optional, Tuple


def chunk_filename(cx: int, cy: int) -> str:
    return f"{cx}_{cy}.bin"


def chunk_path(world_dir: Path, cx: int, cy: int) -> Path:
    return world_dir / "chunks" / chunk_filename(cx, cy)


def create_empty_chunk(chunk_size: int, fill_id: int = 0) -> array:
    return array("H", [fill_id] * (chunk_size * chunk_size))


def _ensure_little_endian_u16(a: array) -> array:
    if sys.byteorder != "little":
        a.byteswap()
    return a


def load_chunk(world_dir: Path, cx: int, cy: int, chunk_size: int) -> Optional[array]:
    p = chunk_path(world_dir, cx, cy)
    if not p.exists():
        return None

    data = p.read_bytes()
    a = array("H")
    a.frombytes(data)

    if sys.byteorder != "little":
        a.byteswap()

    expected = chunk_size * chunk_size
    if len(a) != expected:
        raise ValueError(f"Chunk {cx},{cy} has {len(a)} u16s, expected {expected} ({p})")

    return a


def save_chunk(world_dir: Path, cx: int, cy: int, chunk_size: int, chunk: array) -> None:
    if len(chunk) != chunk_size * chunk_size:
        raise ValueError("Refusing to save chunk with wrong length.")

    p = chunk_path(world_dir, cx, cy)
    p.parent.mkdir(parents=True, exist_ok=True)

    out = array("H", chunk)
    _ensure_little_endian_u16(out)

    tmp = p.with_suffix(".bin.tmp")
    tmp.write_bytes(out.tobytes())
    tmp.replace(p)


def chunk_index(x: int, y: int, chunk_size: int) -> int:
    return y * chunk_size + x


def get_tile(chunk: array, x: int, y: int, chunk_size: int) -> int:
    return int(chunk[chunk_index(x, y, chunk_size)])


def set_tile(chunk: array, x: int, y: int, chunk_size: int, tile_id: int) -> bool:
    idx = chunk_index(x, y, chunk_size)
    old = int(chunk[idx])
    new = int(tile_id)
    if old == new:
        return False
    chunk[idx] = new
    return True


@dataclass
class ChunkCacheEntry:
    chunk: array
    dirty: bool
    dominant_tile: int
    revision: int = 0


class ChunkStore:
    def __init__(self, world_dir: Path, chunk_size: int, max_cache: int = 64) -> None:
        self.world_dir = world_dir
        self.chunk_size = chunk_size
        self.max_cache = max_cache

        self._cache: Dict[Tuple[int, int], ChunkCacheEntry] = {}
        self._lru: list[Tuple[int, int]] = []

    def _touch(self, key: Tuple[int, int]) -> None:
        if key in self._lru:
            self._lru.remove(key)
        self._lru.append(key)

    def _evict_if_needed(self) -> None:
        while len(self._lru) > self.max_cache:
            old = self._lru.pop(0)
            entry = self._cache.get(old)
            if entry is None:
                continue
            if entry.dirty:
                cx, cy = old
                save_chunk(self.world_dir, cx, cy, self.chunk_size, entry.chunk)
                entry.dirty = False
            del self._cache[old]

    def _compute_dominant(self, chunk: array) -> int:
        cs = self.chunk_size
        step = max(1, cs // 32)
        counts: Dict[int, int] = {}
        for y in range(0, cs, step):
            base = y * cs
            for x in range(0, cs, step):
                tid = int(chunk[base + x])
                counts[tid] = counts.get(tid, 0) + 1

        best = 0
        bestc = -1
        for tid, c in counts.items():
            if c > bestc:
                bestc = c
                best = tid
        return best

    def peek_exists(self, cx: int, cy: int) -> bool:
        return chunk_path(self.world_dir, cx, cy).exists()

    def load_if_exists(self, cx: int, cy: int) -> Optional[ChunkCacheEntry]:
        key = (cx, cy)
        if key in self._cache:
            self._touch(key)
            return self._cache[key]

        chunk = load_chunk(self.world_dir, cx, cy, self.chunk_size)
        if chunk is None:
            return None

        entry = ChunkCacheEntry(
            chunk=chunk,
            dirty=False,
            dominant_tile=self._compute_dominant(chunk),
            revision=0,
        )
        self._cache[key] = entry
        self._touch(key)
        self._evict_if_needed()
        return entry

    def get_or_create(self, cx: int, cy: int, fill_id: int = 0) -> ChunkCacheEntry:
        key = (cx, cy)
        if key in self._cache:
            self._touch(key)
            return self._cache[key]

        chunk = load_chunk(self.world_dir, cx, cy, self.chunk_size)
        if chunk is None:
            chunk = create_empty_chunk(self.chunk_size, fill_id=fill_id)

        entry = ChunkCacheEntry(
            chunk=chunk,
            dirty=False,
            dominant_tile=self._compute_dominant(chunk),
            revision=0,
        )
        self._cache[key] = entry
        self._touch(key)
        self._evict_if_needed()
        return entry

    def mark_chunk_modified(self, cx: int, cy: int) -> None:
        entry = self.get_or_create(cx, cy)
        entry.dirty = True
        entry.revision += 1
        entry.dominant_tile = self._compute_dominant(entry.chunk)

    def recompute_dominant(self, cx: int, cy: int) -> None:
        entry = self._cache.get((cx, cy))
        if entry is None:
            return
        entry.dominant_tile = self._compute_dominant(entry.chunk)

    def save_one(self, cx: int, cy: int) -> None:
        entry = self.get_or_create(cx, cy)
        save_chunk(self.world_dir, cx, cy, self.chunk_size, entry.chunk)
        entry.dirty = False

    def flush_all(self) -> None:
        for (cx, cy), entry in list(self._cache.items()):
            if entry.dirty:
                save_chunk(self.world_dir, cx, cy, self.chunk_size, entry.chunk)
                entry.dirty = False