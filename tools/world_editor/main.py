# tools/world_editor/main.py
from __future__ import annotations

import sys
from dataclasses import dataclass
from typing import Optional, Tuple

import pygame

from .camera import Camera
from .config import default_config
from .constants import (
    FPS,
    WINDOW_W,
    WINDOW_H,
    TOP_BAR_H,
    PAN_SPEED_PX,
    ZOOM_STEP,
    CHUNK_CACHE_MAX,
    COLOR_TEXT,
    RIGHT_PANEL_W,
)
from .io_chunks import ChunkStore
from .io_manifest import load_manifest, Manifest
from .palette import PaletteState, draw_right_panel
from .render_chunk_view import render_chunk_view, chunk_editor_region_bounds, fit_camera_to_region
from .render_generate import GenerateState, render_generate
from .render_layout import render_layout
from .render_menu import MenuState, render_menu
from .render_overview import render_overview
from .terrain_gen import bake_terrain_for_active_chunks, generate_continent_layout
from .world_layout import WorldLayout, load_layout, save_layout
from .world_registry import WorldEntry, create_world, delete_world, list_worlds


@dataclass
class EditorState:
    mode: str  # "menu" | "layout" | "generate" | "overview" | "chunk"
    selected_chunk: Tuple[int, int] = (0, 0)


@dataclass
class WorldSession:
    entry: WorldEntry
    manifest: Manifest
    store: ChunkStore
    layout: WorldLayout


def _handle_number_palette_keys(palette: PaletteState, key: int) -> None:
    if pygame.K_1 <= key <= pygame.K_9:
        palette.set_selected(key - pygame.K_1)
    elif key == pygame.K_0:
        palette.set_selected(9)
    elif key == pygame.K_LEFTBRACKET:
        palette.set_brush_size(palette.brush_size - 1)
    elif key == pygame.K_RIGHTBRACKET:
        palette.set_brush_size(palette.brush_size + 1)


def _draw_top_bar(
    screen: pygame.Surface,
    font: pygame.font.Font,
    editor: EditorState,
    world_name: Optional[str],
    mouse_pos,
    mouse_clicked,
) -> tuple[bool, bool]:
    pygame.draw.rect(screen, (22, 22, 26), pygame.Rect(0, 0, WINDOW_W, TOP_BAR_H))
    pygame.draw.line(screen, (60, 60, 70), (0, TOP_BAR_H), (WINDOW_W, TOP_BAR_H), 1)

    back_clicked = False
    menu_clicked = False

    if editor.mode in ("layout", "generate", "overview", "chunk"):
        menu_rect = pygame.Rect(10, 6, 70, 22)
        pygame.draw.rect(screen, (40, 40, 48), menu_rect, border_radius=4)
        pygame.draw.rect(screen, (70, 70, 82), menu_rect, 1, border_radius=4)
        menu_txt = font.render("Menu", True, COLOR_TEXT)
        screen.blit(menu_txt, (menu_rect.x + 16, menu_rect.y + 3))
        if mouse_clicked and menu_rect.collidepoint(mouse_pos):
            menu_clicked = True

    if editor.mode in ("generate", "overview", "chunk"):
        back_rect = pygame.Rect(90, 6, 70, 22)
        pygame.draw.rect(screen, (40, 40, 48), back_rect, border_radius=4)
        pygame.draw.rect(screen, (70, 70, 82), back_rect, 1, border_radius=4)
        back_txt = font.render("Back", True, COLOR_TEXT)
        screen.blit(back_txt, (back_rect.x + 16, back_rect.y + 3))
        if mouse_clicked and back_rect.collidepoint(mouse_pos):
            back_clicked = True

    if world_name:
        label = font.render(f"World: {world_name}", True, (190, 190, 200))
        screen.blit(label, (190, 8))

    return back_clicked, menu_clicked


def _clamp_chunk_camera(
    camera: Camera,
    center_chunk: Tuple[int, int],
    chunk_size: int,
    tile_px: int,
    viewport_rect: pygame.Rect,
) -> None:
    min_wx, min_wy, max_wx, max_wy = chunk_editor_region_bounds(center_chunk, chunk_size, tile_px)

    vw = viewport_rect.w / camera.zoom
    vh = viewport_rect.h / camera.zoom

    viewport_left = viewport_rect.left
    viewport_top = viewport_rect.top

    min_wx0 = min_wx
    max_wx0 = max_wx - vw
    min_wy0 = min_wy
    max_wy0 = max_wy - vh

    if max_wx0 < min_wx0:
        wx0 = (min_wx + max_wx - vw) * 0.5
    else:
        current_wx0 = (viewport_left - camera.ox) / camera.zoom
        wx0 = max(min_wx0, min(max_wx0, current_wx0))

    if max_wy0 < min_wy0:
        wy0 = (min_wy + max_wy - vh) * 0.5
    else:
        current_wy0 = (viewport_top - camera.oy) / camera.zoom
        wy0 = max(min_wy0, min(max_wy0, current_wy0))

    camera.ox = viewport_left - wx0 * camera.zoom
    camera.oy = viewport_top - wy0 * camera.zoom


def _handle_text_input(menu_state: MenuState, event: pygame.event.Event) -> None:
    if event.key == pygame.K_BACKSPACE:
        menu_state.new_world_name = menu_state.new_world_name[:-1]
    elif event.key == pygame.K_SPACE:
        menu_state.new_world_name += " "
    else:
        ch = event.unicode
        if ch and ch.isprintable():
            menu_state.new_world_name += ch


def _load_world_session(world_entry: WorldEntry) -> WorldSession:
    manifest = load_manifest(world_entry.path)
    store = ChunkStore(world_entry.path, manifest.chunk_size, max_cache=CHUNK_CACHE_MAX)
    layout = load_layout(world_entry.path)
    return WorldSession(entry=world_entry, manifest=manifest, store=store, layout=layout)


def run() -> None:
    cfg = default_config()
    worlds_root = cfg.world_dir.parent

    pygame.init()
    pygame.display.set_caption("Stonepyre World Editor (Python)")
    screen = pygame.display.set_mode((WINDOW_W, WINDOW_H))
    clock = pygame.time.Clock()

    font = pygame.font.SysFont("consolas", 16)
    big_font = pygame.font.SysFont("consolas", 28)

    editor = EditorState(mode="menu", selected_chunk=(0, 0))
    palette = PaletteState(selected_tile_id=0, brush_size=1)
    menu_state = MenuState()
    gen_state = GenerateState()

    cam_layout = Camera(ox=220, oy=160, zoom=1.0)
    cam_overview = Camera(ox=220, oy=160, zoom=1.0)
    cam_chunk = Camera(ox=120, oy=120, zoom=1.0)

    worlds = list_worlds(worlds_root)
    if worlds and menu_state.selected_index >= len(worlds):
        menu_state.selected_index = 0

    session: Optional[WorldSession] = None
    running = True
    mouse_down_left = False
    mouse_down_right = False

    while running:
        clock.tick(FPS)
        mouse_pos = pygame.mouse.get_pos()
        mouse_clicked = False
        wheel_y = 0

        for event in pygame.event.get():
            if event.type == pygame.QUIT:
                running = False

            elif event.type == pygame.MOUSEBUTTONDOWN:
                if event.button == 1:
                    mouse_down_left = True
                    mouse_clicked = True
                elif event.button == 3:
                    mouse_down_right = True
                elif event.button == 4:
                    wheel_y = +1
                elif event.button == 5:
                    wheel_y = -1

            elif event.type == pygame.MOUSEBUTTONUP:
                if event.button == 1:
                    mouse_down_left = False
                elif event.button == 3:
                    mouse_down_right = False

            elif event.type == pygame.KEYDOWN:
                if editor.mode == "menu":
                    if event.key == pygame.K_ESCAPE:
                        running = False
                    elif event.key == pygame.K_RETURN:
                        try:
                            created = create_world(worlds_root, menu_state.new_world_name)
                            worlds = list_worlds(worlds_root)
                            menu_state.selected_index = next(i for i, w in enumerate(worlds) if w.name == created.name)
                            menu_state.message = f"Created world: {created.name}"
                            menu_state.new_world_name = ""
                        except Exception as e:
                            menu_state.message = str(e)
                    else:
                        _handle_text_input(menu_state, event)

                else:
                    if event.key == pygame.K_ESCAPE:
                        if editor.mode == "chunk":
                            editor.mode = "overview"
                        elif editor.mode == "overview":
                            editor.mode = "generate"
                        elif editor.mode == "generate":
                            editor.mode = "layout"
                        else:
                            editor.mode = "menu"
                            if session is not None:
                                session.store.flush_all()
                                save_layout(session.entry.path, session.layout)

                    if event.key == pygame.K_TAB and editor.mode in ("layout", "generate", "overview"):
                        if editor.mode == "layout":
                            editor.mode = "generate"
                        elif editor.mode == "generate":
                            editor.mode = "overview"
                        else:
                            editor.mode = "layout"

                    if event.key == pygame.K_RETURN:
                        if editor.mode == "layout":
                            editor.mode = "generate"
                        elif editor.mode == "generate":
                            editor.mode = "overview"

                    _handle_number_palette_keys(palette, event.key)

                    if event.key == pygame.K_s and session is not None:
                        session.store.flush_all()
                        save_layout(session.entry.path, session.layout)
                        gen_state.message = "Saved world layout and dirty chunks."

                    if editor.mode == "generate" and session is not None:
                        if event.key == pygame.K_q:
                            gen_state.seed += 1
                        elif event.key == pygame.K_a:
                            gen_state.seed -= 1
                        elif event.key == pygame.K_w:
                            gen_state.land_scale = min(128.0, gen_state.land_scale + 1.0)
                        elif event.key == pygame.K_s:
                            gen_state.land_scale = max(2.0, gen_state.land_scale - 1.0)
                        elif event.key == pygame.K_e:
                            gen_state.edge_falloff = min(4.0, gen_state.edge_falloff + 0.05)
                        elif event.key == pygame.K_d:
                            gen_state.edge_falloff = max(0.1, gen_state.edge_falloff - 0.05)
                        elif event.key == pygame.K_r:
                            gen_state.land_bias = min(0.90, gen_state.land_bias + 0.01)
                        elif event.key == pygame.K_f:
                            gen_state.land_bias = max(0.05, gen_state.land_bias - 0.01)
                        elif event.key == pygame.K_c:
                            count = generate_continent_layout(
                                layout=session.layout,
                                seed=gen_state.seed,
                                land_scale=gen_state.land_scale,
                                edge_falloff=gen_state.edge_falloff,
                                land_bias=gen_state.land_bias,
                            )
                            save_layout(session.entry.path, session.layout)
                            gen_state.message = f"Generated continent mask with {count} active chunks."
                        elif event.key == pygame.K_t:
                            baked = bake_terrain_for_active_chunks(
                                layout=session.layout,
                                store=session.store,
                                chunk_size=session.manifest.chunk_size,
                                seed=gen_state.seed,
                            )
                            session.store.flush_all()
                            gen_state.message = f"Baked terrain for {baked} active chunks."

        screen.fill((12, 12, 14))

        current_world_name = session.entry.name if session is not None else None
        back_clicked, menu_clicked = _draw_top_bar(screen, font, editor, current_world_name, mouse_pos, mouse_clicked)

        if menu_clicked:
            if session is not None:
                session.store.flush_all()
                save_layout(session.entry.path, session.layout)
            worlds = list_worlds(worlds_root)
            editor.mode = "menu"

        if back_clicked:
            if editor.mode == "chunk":
                editor.mode = "overview"
            elif editor.mode == "overview":
                editor.mode = "generate"
            elif editor.mode == "generate":
                editor.mode = "layout"

        if editor.mode == "menu":
            result = render_menu(
                screen=screen,
                font=font,
                big_font=big_font,
                state=menu_state,
                worlds=worlds,
                mouse_pos=mouse_pos,
                mouse_clicked=mouse_clicked,
            )

            if result.selected_world_index is not None and worlds:
                menu_state.selected_index = result.selected_world_index

            if result.create_requested:
                try:
                    created = create_world(worlds_root, menu_state.new_world_name)
                    worlds = list_worlds(worlds_root)
                    menu_state.selected_index = next(i for i, w in enumerate(worlds) if w.name == created.name)
                    menu_state.message = f"Created world: {created.name}"
                    menu_state.new_world_name = ""
                except Exception as e:
                    menu_state.message = str(e)

            if result.delete_requested and worlds:
                try:
                    victim = worlds[menu_state.selected_index]
                    delete_world(victim)
                    worlds = list_worlds(worlds_root)
                    if worlds:
                        menu_state.selected_index = min(menu_state.selected_index, len(worlds) - 1)
                    else:
                        menu_state.selected_index = 0
                    menu_state.message = f"Deleted world: {victim.name}"
                except Exception as e:
                    menu_state.message = str(e)

            if result.load_requested and worlds:
                chosen = worlds[menu_state.selected_index]
                session = _load_world_session(chosen)
                editor.mode = "layout"
                menu_state.message = ""

                cam_layout.ox = 220
                cam_layout.oy = 160
                cam_layout.zoom = 1.0

                cam_overview.ox = 220
                cam_overview.oy = 160
                cam_overview.zoom = 1.0

        elif editor.mode == "layout":
            assert session is not None

            keys = pygame.key.get_pressed()
            if keys[pygame.K_w] or keys[pygame.K_UP]:
                cam_layout.pan(0, PAN_SPEED_PX)
            if keys[pygame.K_s] or keys[pygame.K_DOWN]:
                cam_layout.pan(0, -PAN_SPEED_PX)
            if keys[pygame.K_a] or keys[pygame.K_LEFT]:
                cam_layout.pan(PAN_SPEED_PX, 0)
            if keys[pygame.K_d] or keys[pygame.K_RIGHT]:
                cam_layout.pan(-PAN_SPEED_PX, 0)

            if wheel_y != 0:
                factor = ZOOM_STEP if wheel_y > 0 else 1.0 / ZOOM_STEP
                cam_layout.zoom_at(factor, mouse_pos[0], mouse_pos[1])
                cam_layout.zoom = max(0.25, min(6.0, cam_layout.zoom))

            result = render_layout(
                screen=screen,
                font=font,
                camera=cam_layout,
                layout=session.layout,
                cell_px=cfg.overview_cell_px,
                mouse_pos=mouse_pos,
                left_down=mouse_down_left,
                right_down=mouse_down_right,
            )
            if result.changed:
                save_layout(session.entry.path, session.layout)

        elif editor.mode == "generate":
            assert session is not None
            render_generate(
                screen=screen,
                font=font,
                big_font=big_font,
                layout=session.layout,
                state=gen_state,
            )

        elif editor.mode == "overview":
            assert session is not None

            keys = pygame.key.get_pressed()
            if keys[pygame.K_w] or keys[pygame.K_UP]:
                cam_overview.pan(0, PAN_SPEED_PX)
            if keys[pygame.K_s] or keys[pygame.K_DOWN]:
                cam_overview.pan(0, -PAN_SPEED_PX)
            if keys[pygame.K_a] or keys[pygame.K_LEFT]:
                cam_overview.pan(PAN_SPEED_PX, 0)
            if keys[pygame.K_d] or keys[pygame.K_RIGHT]:
                cam_overview.pan(-PAN_SPEED_PX, 0)

            if wheel_y != 0:
                factor = ZOOM_STEP if wheel_y > 0 else 1.0 / ZOOM_STEP
                cam_overview.zoom_at(factor, mouse_pos[0], mouse_pos[1])
                cam_overview.zoom = max(0.25, min(6.0, cam_overview.zoom))

            out = render_overview(
                screen=screen,
                font=font,
                camera=cam_overview,
                layout=session.layout,
                cell_px=cfg.overview_cell_px,
                mouse_pos=mouse_pos,
                mouse_clicked=mouse_clicked,
            )

            if out.clicked_chunk is not None:
                editor.selected_chunk = out.clicked_chunk
                editor.mode = "chunk"

                chunk_viewport = pygame.Rect(0, TOP_BAR_H, WINDOW_W - RIGHT_PANEL_W, WINDOW_H - TOP_BAR_H)
                fit_camera_to_region(
                    camera=cam_chunk,
                    anchor_chunk=editor.selected_chunk,
                    chunk_size=session.manifest.chunk_size,
                    tile_px=cfg.tile_px,
                    viewport_rect=chunk_viewport,
                    padding=24,
                )

        elif editor.mode == "chunk":
            assert session is not None

            chunk_viewport = pygame.Rect(0, TOP_BAR_H, WINDOW_W - RIGHT_PANEL_W, WINDOW_H - TOP_BAR_H)

            keys = pygame.key.get_pressed()
            if keys[pygame.K_LEFT]:
                cam_chunk.pan(PAN_SPEED_PX, 0)
            if keys[pygame.K_RIGHT]:
                cam_chunk.pan(-PAN_SPEED_PX, 0)
            if keys[pygame.K_UP]:
                cam_chunk.pan(0, PAN_SPEED_PX)
            if keys[pygame.K_DOWN]:
                cam_chunk.pan(0, -PAN_SPEED_PX)

            if wheel_y != 0:
                factor = ZOOM_STEP if wheel_y > 0 else 1.0 / ZOOM_STEP
                cam_chunk.zoom_at(factor, mouse_pos[0], mouse_pos[1])

            render_chunk_view(
                screen=screen,
                font=font,
                manifest=session.manifest,
                store=session.store,
                layout=session.layout,
                camera=cam_chunk,
                anchor_chunk=editor.selected_chunk,
                tile_px=cfg.tile_px,
                palette=palette,
                mouse_pos=mouse_pos,
                mouse_down_left=mouse_down_left,
            )

            _clamp_chunk_camera(
                camera=cam_chunk,
                center_chunk=editor.selected_chunk,
                chunk_size=session.manifest.chunk_size,
                tile_px=cfg.tile_px,
                viewport_rect=chunk_viewport,
            )

            clicked, new_id = draw_right_panel(
                screen=screen,
                font=font,
                manifest=session.manifest,
                palette=palette,
                mouse_pos=mouse_pos,
                mouse_clicked=mouse_clicked,
            )
            if clicked:
                palette.set_selected(new_id)

        pygame.display.flip()

    if session is not None:
        session.store.flush_all()
        save_layout(session.entry.path, session.layout)

    pygame.quit()
    sys.exit(0)