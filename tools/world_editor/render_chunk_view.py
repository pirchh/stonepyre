# tools/world_editor/render_chunk_view.py
from __future__ import annotations

from dataclasses import dataclass
from typing import Dict, Optional, Tuple

import pygame

from .camera import Camera
from .constants import (
    COLOR_BG,
    COLOR_GRID,
    COLOR_TEXT,
    TOP_BAR_H,
    COLOR_EMPTY_CHUNK,
    RIGHT_PANEL_W,
)
from .io_chunks import ChunkStore, get_tile, set_tile
from .io_manifest import Manifest
from .palette import PaletteState


ANCHOR_TINT = (90, 140, 220, 70)
NEIGHBOR_TINT = (255, 255, 255, 18)
HOVER_OUTLINE = (255, 255, 0)
BRUSH_OUTLINE = (255, 215, 0)
CHUNK_LABEL_BG = (20, 20, 24)
CHUNK_LABEL_BORDER = (80, 80, 90)
TILE_GRID_COLOR = (50, 50, 55)

CHUNK_SURFACE_SIZE = 256
INFO_Y_1 = TOP_BAR_H + 10
INFO_Y_2 = TOP_BAR_H + 34


@dataclass
class ChunkViewResult:
    painted: bool = False
    hovered_info: Optional[str] = None


@dataclass
class ChunkSurfaceCacheEntry:
    revision: int
    surface: pygame.Surface


_CHUNK_SURFACE_CACHE: Dict[Tuple[int, int], ChunkSurfaceCacheEntry] = {}


def _tile_color(manifest: Manifest, tile_id: int):
    td = manifest.tile_by_id.get(tile_id)
    if td is None:
        return (120, 0, 255)
    return td.rgb


def chunk_editor_region_bounds(center_chunk: Tuple[int, int], chunk_size: int, tile_px: int) -> Tuple[int, int, int, int]:
    ccx, ccy = center_chunk
    min_tx = (ccx - 1) * chunk_size
    min_ty = (ccy - 1) * chunk_size
    max_tx = (ccx + 2) * chunk_size
    max_ty = (ccy + 2) * chunk_size
    return (
        min_tx * tile_px,
        min_ty * tile_px,
        max_tx * tile_px,
        max_ty * tile_px,
    )


def fit_camera_to_region(
    camera: Camera,
    anchor_chunk: Tuple[int, int],
    chunk_size: int,
    tile_px: int,
    viewport_rect: pygame.Rect,
    padding: int = 24,
) -> None:
    min_wx, min_wy, max_wx, max_wy = chunk_editor_region_bounds(anchor_chunk, chunk_size, tile_px)

    region_w = max_wx - min_wx
    region_h = max_wy - min_wy

    usable_w = max(1, viewport_rect.w - padding * 2)
    usable_h = max(1, viewport_rect.h - padding * 2)

    zoom_x = usable_w / region_w
    zoom_y = usable_h / region_h
    camera.zoom = min(zoom_x, zoom_y)
    camera.clamp_zoom()

    region_center_x = (min_wx + max_wx) * 0.5
    region_center_y = (min_wy + max_wy) * 0.5

    camera.ox = viewport_rect.centerx - region_center_x * camera.zoom
    camera.oy = viewport_rect.centery - region_center_y * camera.zoom


def _get_chunk_surface(
    manifest: Manifest,
    store: ChunkStore,
    cx: int,
    cy: int,
) -> Optional[pygame.Surface]:
    entry = store.load_if_exists(cx, cy)
    if entry is None:
        return None

    cached = _CHUNK_SURFACE_CACHE.get((cx, cy))
    if cached is not None and cached.revision == entry.revision:
        return cached.surface

    cs = manifest.chunk_size
    surf = pygame.Surface((CHUNK_SURFACE_SIZE, CHUNK_SURFACE_SIZE))

    px_array = pygame.PixelArray(surf)
    try:
        for y in range(cs):
            row = y * cs
            sy = y * CHUNK_SURFACE_SIZE // cs
            for x in range(cs):
                tid = int(entry.chunk[row + x])
                color = _tile_color(manifest, tid)
                sx = x * CHUNK_SURFACE_SIZE // cs
                px_array[sx, sy] = surf.map_rgb(color)
    finally:
        del px_array

    _CHUNK_SURFACE_CACHE[(cx, cy)] = ChunkSurfaceCacheEntry(
        revision=entry.revision,
        surface=surf,
    )
    return surf


def _draw_chunk_overlay_tint(
    screen: pygame.Surface,
    camera: Camera,
    chunk_x: int,
    chunk_y: int,
    chunk_size: int,
    tile_px: int,
    view_rect: pygame.Rect,
    color_rgba: Tuple[int, int, int, int],
) -> None:
    left = chunk_x * chunk_size * tile_px
    top = chunk_y * chunk_size * tile_px
    right = (chunk_x + 1) * chunk_size * tile_px
    bottom = (chunk_y + 1) * chunk_size * tile_px

    sx1, sy1 = camera.world_to_screen(left, top)
    sx2, sy2 = camera.world_to_screen(right, bottom)

    rect = pygame.Rect(int(sx1), int(sy1), int(sx2 - sx1), int(sy2 - sy1))
    if rect.w <= 0 or rect.h <= 0 or not rect.colliderect(view_rect):
        return

    overlay = pygame.Surface((rect.w, rect.h), pygame.SRCALPHA)
    overlay.fill(color_rgba)
    screen.blit(overlay, rect.topleft)


def _draw_chunk_label(
    screen: pygame.Surface,
    font: pygame.font.Font,
    camera: Camera,
    chunk_x: int,
    chunk_y: int,
    chunk_size: int,
    tile_px: int,
    view_rect: pygame.Rect,
) -> None:
    left = chunk_x * chunk_size * tile_px
    top = chunk_y * chunk_size * tile_px

    sx, sy = camera.world_to_screen(left, top)
    sx_i = int(sx) + 8
    sy_i = int(sy) + 8

    label = font.render(f"({chunk_x},{chunk_y})", True, COLOR_TEXT)
    box = pygame.Rect(sx_i - 4, sy_i - 2, label.get_width() + 8, label.get_height() + 4)

    if not box.colliderect(view_rect):
        return

    pygame.draw.rect(screen, CHUNK_LABEL_BG, box, border_radius=4)
    pygame.draw.rect(screen, CHUNK_LABEL_BORDER, box, 1, border_radius=4)
    screen.blit(label, (sx_i, sy_i))


def _apply_square_brush(
    manifest: Manifest,
    store: ChunkStore,
    anchor_chunk: Tuple[int, int],
    center_gtx: int,
    center_gty: int,
    brush_size: int,
    tile_id: int,
) -> bool:
    acx, acy = anchor_chunk
    cs = manifest.chunk_size

    radius = max(0, brush_size - 1)
    changed_any = False

    for gty in range(center_gty - radius, center_gty + radius + 1):
        for gtx in range(center_gtx - radius, center_gtx + radius + 1):
            hcx = gtx // cs
            hcy = gty // cs

            editable = (acx - 1 <= hcx <= acx + 1) and (acy - 1 <= hcy <= acy + 1)
            if not editable:
                continue

            in_x = gtx - hcx * cs
            in_y = gty - hcy * cs

            if not (0 <= in_x < cs and 0 <= in_y < cs):
                continue

            entry = store.get_or_create(hcx, hcy, fill_id=0)
            changed = set_tile(entry.chunk, in_x, in_y, cs, tile_id)
            if changed:
                store.mark_chunk_modified(hcx, hcy)
                changed_any = True

    return changed_any


def render_chunk_view(
    screen: pygame.Surface,
    font: pygame.font.Font,
    manifest: Manifest,
    store: ChunkStore,
    camera: Camera,
    anchor_chunk: Tuple[int, int],
    tile_px: int,
    palette: PaletteState,
    mouse_pos: Tuple[int, int],
    mouse_down_left: bool,
) -> ChunkViewResult:
    res = ChunkViewResult()
    w, h = screen.get_size()
    view_rect = pygame.Rect(0, TOP_BAR_H, w - RIGHT_PANEL_W, h - TOP_BAR_H)

    pygame.draw.rect(screen, COLOR_BG, view_rect)

    acx, acy = anchor_chunk
    cs = manifest.chunk_size

    title = f"Chunk Edit — anchor=({acx}, {acy}) | paint across visible 3x3"
    screen.blit(font.render(title, True, COLOR_TEXT), (12, INFO_Y_1))

    region_min_tx = (acx - 1) * cs
    region_min_ty = (acy - 1) * cs
    region_max_tx = (acx + 2) * cs - 1
    region_max_ty = (acy + 2) * cs - 1

    grid_view_rect = pygame.Rect(0, TOP_BAR_H + 54, w - RIGHT_PANEL_W, h - (TOP_BAR_H + 54))
    pygame.draw.rect(screen, COLOR_BG, grid_view_rect)

    for cy in range(acy - 1, acy + 2):
        for cx in range(acx - 1, acx + 2):
            left = cx * cs * tile_px
            top = cy * cs * tile_px
            right = (cx + 1) * cs * tile_px
            bottom = (cy + 1) * cs * tile_px

            sx1, sy1 = camera.world_to_screen(left, top)
            sx2, sy2 = camera.world_to_screen(right, bottom)

            rect = pygame.Rect(int(sx1), int(sy1), int(sx2 - sx1), int(sy2 - sy1))
            if rect.w <= 0 or rect.h <= 0 or not rect.colliderect(grid_view_rect):
                continue

            chunk_surface = _get_chunk_surface(manifest, store, cx, cy)
            if chunk_surface is None:
                pygame.draw.rect(screen, COLOR_EMPTY_CHUNK, rect)
            else:
                scaled = pygame.transform.scale(chunk_surface, (rect.w, rect.h))
                screen.blit(scaled, rect.topleft)

    if tile_px * camera.zoom >= 18:
        left_w, top_w = camera.screen_to_world(grid_view_rect.left, grid_view_rect.top)
        right_w, bot_w = camera.screen_to_world(grid_view_rect.right, grid_view_rect.bottom)

        min_tx = max(int(left_w // tile_px) - 2, region_min_tx)
        max_tx = min(int(right_w // tile_px) + 2, region_max_tx)
        min_ty = max(int(top_w // tile_px) - 2, region_min_ty)
        max_ty = min(int(bot_w // tile_px) + 2, region_max_ty)

        for gtx in range(min_tx, max_tx + 2):
            wx = gtx * tile_px
            sx, _ = camera.world_to_screen(wx, 0)
            pygame.draw.line(screen, TILE_GRID_COLOR, (int(sx), grid_view_rect.top), (int(sx), grid_view_rect.bottom), 1)

        for gty in range(min_ty, max_ty + 2):
            wy = gty * tile_px
            _, sy = camera.world_to_screen(0, wy)
            pygame.draw.line(screen, TILE_GRID_COLOR, (grid_view_rect.left, int(sy)), (grid_view_rect.right, int(sy)), 1)

    for cy in range(acy - 1, acy + 2):
        for cx in range(acx - 1, acx + 2):
            tint = ANCHOR_TINT if (cx, cy) == (acx, acy) else NEIGHBOR_TINT
            _draw_chunk_overlay_tint(screen, camera, cx, cy, cs, tile_px, grid_view_rect, tint)
            _draw_chunk_label(screen, font, camera, cx, cy, cs, tile_px, grid_view_rect)

            left = cx * cs * tile_px
            top = cy * cs * tile_px
            right = (cx + 1) * cs * tile_px
            bottom = (cy + 1) * cs * tile_px

            sx1, sy1 = camera.world_to_screen(left, top)
            sx2, sy2 = camera.world_to_screen(right, bottom)
            rect = pygame.Rect(int(sx1), int(sy1), int(sx2 - sx1), int(sy2 - sy1))

            if rect.colliderect(grid_view_rect):
                border_color = (255, 255, 255) if (cx, cy) == (acx, acy) else COLOR_GRID
                border_width = 3 if (cx, cy) == (acx, acy) else 2
                pygame.draw.rect(screen, border_color, rect, border_width)

    hovered_tile_rect: Optional[pygame.Rect] = None
    hovered_brush_rect: Optional[pygame.Rect] = None

    if grid_view_rect.collidepoint(mouse_pos):
        mx, my = mouse_pos
        wx, wy = camera.screen_to_world(mx, my)
        gtx = int(wx // tile_px)
        gty = int(wy // tile_px)

        if region_min_tx <= gtx <= region_max_tx and region_min_ty <= gty <= region_max_ty:
            hcx = gtx // cs
            hcy = gty // cs
            in_x = gtx - hcx * cs
            in_y = gty - hcy * cs

            entry = store.load_if_exists(hcx, hcy)
            if entry is None:
                res.hovered_info = f"tile=({gtx},{gty}) chunk=({hcx},{hcy}) in=({in_x},{in_y}) empty | brush={palette.brush_size}"
            else:
                tid = get_tile(entry.chunk, in_x, in_y, cs)
                tdef = manifest.tile_by_id.get(tid)
                name = tdef.name if tdef else f"unknown({tid})"
                res.hovered_info = f"tile=({gtx},{gty}) chunk=({hcx},{hcy}) in=({in_x},{in_y}) id={tid} {name} | brush={palette.brush_size}"

            hsx, hsy = camera.world_to_screen(gtx * tile_px, gty * tile_px)
            hovered_tile_rect = pygame.Rect(
                int(hsx),
                int(hsy),
                max(1, int(tile_px * camera.zoom)),
                max(1, int(tile_px * camera.zoom)),
            )

            radius = max(0, palette.brush_size - 1)
            bx1, by1 = camera.world_to_screen((gtx - radius) * tile_px, (gty - radius) * tile_px)
            bx2, by2 = camera.world_to_screen((gtx + radius + 1) * tile_px, (gty + radius + 1) * tile_px)
            hovered_brush_rect = pygame.Rect(int(bx1), int(by1), int(bx2 - bx1), int(by2 - by1))

            if mouse_down_left:
                changed = _apply_square_brush(
                    manifest=manifest,
                    store=store,
                    anchor_chunk=anchor_chunk,
                    center_gtx=gtx,
                    center_gty=gty,
                    brush_size=palette.brush_size,
                    tile_id=palette.selected_tile_id,
                )
                if changed:
                    res.painted = True

    if hovered_brush_rect and hovered_brush_rect.colliderect(grid_view_rect):
        pygame.draw.rect(screen, BRUSH_OUTLINE, hovered_brush_rect, 2)

    if hovered_tile_rect and hovered_tile_rect.colliderect(grid_view_rect):
        pygame.draw.rect(screen, HOVER_OUTLINE, hovered_tile_rect, 1)

    if res.hovered_info:
        info = font.render(res.hovered_info, True, (190, 190, 200))
        screen.blit(info, (12, INFO_Y_2))

    return res