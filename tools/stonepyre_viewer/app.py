from __future__ import annotations

import time
from typing import Dict, List, Optional, Tuple

import pygame

from .config import (
    START_W,
    START_H,
    LOCK_TO_1080P,
    MIN_ZOOM,
    MAX_ZOOM,
    ZOOM_START,
    PROJECT_ROOT,
    discover_models,
)

from .ui_common import Dropdown, dropdown_menu_rect
from .actions import discover_action_groups_for_model, ActionGroups, ActionEntry
from .sprites import load_skin_bundle
from .palettes import iter_palette_files, load_palette_json

from .viewer_mode import (
    draw_viewer,
    handle_viewer_click,
    discover_tool_variants_for_clip,
    discover_tool_palette_names_for,  # NEW
)

from .bake_mode import (
    BakeState,
    render_bake_mode,
    handle_bake_click,
    handle_bake_scroll,
    scan_bake_rows,
    scan_tool_bake_rows,
    do_bake_operation,
    do_tool_bake_operation,
)

from .manager_mode import ManagerState, render_manager, handle_manager_click

from .tool_fit_mode import (
    ToolFitState,
    render_tool_fit,
    handle_tool_fit_event,
    handle_tool_fit_click,
    update_tool_fit_held_keys,
    handle_tool_fit_mouse,
    discover_tool_kinds,        # NEW
    ensure_tool_kind_exists,    # NEW
)

try:
    from .tcg_mode import render_tcg, handle_tcg_click, handle_tcg_event  # type: ignore
except Exception:
    render_tcg = None
    handle_tcg_click = None
    handle_tcg_event = None


FLAGS_PRIMARY = pygame.SCALED | pygame.RESIZABLE
FLAGS_FALLBACK = pygame.RESIZABLE
ACTIVE_FLAGS = FLAGS_PRIMARY


def safe_set_mode(size):
    global ACTIVE_FLAGS
    try:
        return pygame.display.set_mode(size, ACTIVE_FLAGS)
    except pygame.error as e:
        msg = str(e).lower()
        if "failed to create renderer" in msg or "renderer" in msg:
            print(f"[WARN] SCALED renderer failed; falling back to RESIZABLE only. ({e})")
            ACTIVE_FLAGS = FLAGS_FALLBACK
            return pygame.display.set_mode(size, ACTIVE_FLAGS)
        raise


def _rebuild_dropdown(dd: Dropdown, items: List[str], keep_selected_value: Optional[str] = None) -> None:
    dd.items = items[:]
    if keep_selected_value:
        lowered = [x.lower() for x in items]
        try:
            dd.selected_index = lowered.index(keep_selected_value.lower())
        except Exception:
            dd.selected_index = 0
    else:
        dd.selected_index = min(dd.selected_index, max(0, len(items) - 1))


def _current_action_entry(action_groups: ActionGroups, group: str, action_label: str) -> Optional[ActionEntry]:
    actions = action_groups.actions_for_group((group or "").strip().lower())
    for a in actions:
        if a.label == action_label:
            return a
    return None


def _discover_palette_names(palettes_dir) -> List[str]:
    names: List[str] = []
    if palettes_dir and palettes_dir.exists():
        for pf in iter_palette_files(palettes_dir):
            try:
                pal = load_palette_json(pf)
                if pal.name and pal.name.strip():
                    names.append(pal.name.strip())
            except Exception:
                pass

    seen = set()
    out: List[str] = []
    for n in names:
        k = n.lower()
        if k in seen:
            continue
        seen.add(k)
        out.append(n)
    return out


def _mode_from_dd(mode_dd: Dropdown) -> str:
    sel = (mode_dd.selected() or "Viewer").strip().lower()
    return {
        "viewer": "viewer",
        "bake": "bake",
        "manager": "manager",
        "tool fit": "tool_fit",
        "tcg": "tcg",
    }.get(sel, "viewer")


def _close_all_dropdowns(dds: List[Dropdown]) -> None:
    for dd in dds:
        dd.open = False


def _handle_dropdown_click_generic(
    pos: Tuple[int, int],
    *,
    menus: Dict[str, Dropdown],
    ui_menu_focus: Optional[str],
) -> Tuple[Optional[Tuple[str, object]], Optional[str]]:
    any_open = any(dd.open for dd in menus.values())

    if any_open:
        focus = ui_menu_focus
        if focus not in menus or not menus[focus].open:
            focus = next((k for k, dd in menus.items() if dd.open), None)

        if focus is not None:
            dd = menus[focus]

            if dd.rect.collidepoint(pos):
                dd.open = False
                return None, None

            menu = dropdown_menu_rect(dd)
            if menu.w > 0 and menu.collidepoint(pos):
                item_h = dd.rect.h
                max_items = min(len(dd.items), 16)
                i = (pos[1] - menu.y) // item_h
                if 0 <= i < max_items:
                    dd.selected_index = int(i)
                    for other in menus.values():
                        other.open = False
                    return (f"{focus}_changed", None), None

            for other in menus.values():
                other.open = False
            return None, None

        for other in menus.values():
            other.open = False
        return None, None

    for key, dd in menus.items():
        if dd.rect.collidepoint(pos):
            dd.open = True
            for other in menus.values():
                if other is not dd:
                    other.open = False
            return None, key

    return None, None


def main():
    pygame.init()
    pygame.display.set_caption("Stonepyre Viewer")

    screen = safe_set_mode((START_W, START_H))
    clock = pygame.time.Clock()

    font = pygame.font.SysFont("consolas", 18)
    font_ui = pygame.font.SysFont("consolas", 18)

    zoom = ZOOM_START
    fps = 8
    paused = False
    frame_idx = 0
    scale_cache: Dict[object, object] = {}

    mode = "viewer"

    mode_dd = Dropdown(title="Mode", items=["Viewer", "Bake", "Manager", "Tool Fit", "TCG"], selected_index=0)

    models = discover_models()
    model_keys = list(models.keys())
    model_dd = Dropdown(title="Model", items=model_keys, selected_index=0)

    current_model_key = model_dd.selected() or (model_keys[0] if model_keys else "")
    action_groups = (
        discover_action_groups_for_model(models[current_model_key].base_dir)
        if current_model_key
        else ActionGroups([], [], [])
    )
    group_dd = Dropdown(title="Group", items=["Base", "Skills", "Combat"], selected_index=0)

    def _actions_for_group_label(g: str) -> List[ActionEntry]:
        gl = (g or "").strip().lower()
        if gl == "base":
            return action_groups.base
        if gl == "skills":
            return action_groups.skills
        if gl == "combat":
            return action_groups.combat
        return action_groups.base

    action_items = [a.label for a in _actions_for_group_label(group_dd.selected() or "Base")]
    action_dd = Dropdown(title="Action", items=action_items or ["(none)"], selected_index=0)

    skin_dd = Dropdown(title="Skin", items=["greyscale"], selected_index=0)

    tool_dd = Dropdown(title="Tool", items=["none"], selected_index=0)

    # NEW: independent tool palette selection (iron/gold/…)
    tool_skin_dd = Dropdown(title="Tool Skin", items=["match_character"], selected_index=0)

    # NEW: Tool Kind dropdown (Tool Fit mode)
    tool_kind_items = discover_tool_kinds()
    if not tool_kind_items:
        tool_kind_items = ["axe"]
    tool_kind_dd = Dropdown(title="Tool Kind", items=tool_kind_items, selected_index=0)

    bake_scope_dd = Dropdown(title="Bake Scope", items=["All Actions", "Current Group", "Current Action"], selected_index=0)
    bake_palette_dd = Dropdown(title="Palette", items=["All"], selected_index=0)

    bake_state = BakeState()
    bake_rows = []
    tool_rows = []

    manager_state = ManagerState()
    tool_fit_state = ToolFitState()

    # keep dropdown selection aligned with state at boot
    if tool_fit_state.tool_kind:
        _rebuild_dropdown(tool_kind_dd, tool_kind_items, keep_selected_value=tool_fit_state.tool_kind)
        ensure_tool_kind_exists(tool_kind_dd.selected() or tool_fit_state.tool_kind)

    ui_menu_focus: Optional[str] = None

    # IMPORTANT: initialize viewer ui_rects so click handler never KeyErrors before first draw
    ui_rects: Dict[str, pygame.Rect] = {
        "new_pet": pygame.Rect(-10_000, -10_000, 1, 1),
        "import_xcf": pygame.Rect(-10_000, -10_000, 1, 1),
        # tool_fit buttons may also be checked before first draw; safe placeholders:
        "new_kind": pygame.Rect(-10_000, -10_000, 1, 1),
        "new_tool": pygame.Rect(-10_000, -10_000, 1, 1),
    }

    bundle_surfaces: Dict[str, List[pygame.Surface]] = {}
    current_action_rel_for_view: str = ""

    def rebuild_action_dd():
        g = group_dd.selected() or "Base"
        items = [a.label for a in _actions_for_group_label(g)]
        keep = action_dd.selected() or ""
        _rebuild_dropdown(action_dd, items or ["(none)"], keep_selected_value=keep)

    def rebuild_skin_dd():
        nonlocal models, current_model_key
        if not current_model_key:
            _rebuild_dropdown(skin_dd, ["greyscale"], keep_selected_value="greyscale")
            return

        model = models[current_model_key]
        palette_names = _discover_palette_names(model.palettes_dir)
        items = ["greyscale"] + palette_names
        keep = skin_dd.selected() or "greyscale"
        _rebuild_dropdown(skin_dd, items, keep_selected_value=keep)

    def rebuild_tool_dd_for_current_clip():
        clip_key = current_action_rel_for_view
        items = discover_tool_variants_for_clip(clip_key=clip_key)
        keep = tool_dd.selected() or "none"
        _rebuild_dropdown(tool_dd, items, keep_selected_value=keep)

    def rebuild_tool_skin_dd_for_current_clip():
        """
        Populates tool_skin_dd from:
          libs/palettes/<clip_leaf>/<tool_kind>/<tool_id>/*.json

        Always includes 'match_character' as a stable default.
        """
        clip_key = current_action_rel_for_view
        tool_id = (tool_dd.selected() or "none").strip()

        # Infer tool kind from clip; keep simple + stable.
        ck = (clip_key or "").lower()
        tool_kind = "axe"
        if "mining" in ck:
            tool_kind = "pickaxe"
        elif "fishing" in ck:
            tool_kind = "harpoon"

        items = discover_tool_palette_names_for(
            clip_key=clip_key,
            tool_kind=tool_kind,
            tool_id=tool_id,
        )

        keep = tool_skin_dd.selected() or "match_character"
        _rebuild_dropdown(tool_skin_dd, items or ["match_character"], keep_selected_value=keep)

    def rebuild_tool_kind_dd_keep_state():
        keep = tool_fit_state.tool_kind or (tool_kind_dd.selected() or "axe")
        items = discover_tool_kinds()
        if not items:
            items = ["axe"]
        _rebuild_dropdown(tool_kind_dd, items, keep_selected_value=keep)
        ensure_tool_kind_exists(tool_kind_dd.selected() or keep)

    def reload_current_bundle():
        nonlocal bundle_surfaces, scale_cache, current_action_rel_for_view

        scale_cache.clear()
        current_action_rel_for_view = ""

        g = group_dd.selected() or "Base"
        actions = _actions_for_group_label(g)
        if not actions:
            bundle_surfaces = {}
            rebuild_tool_dd_for_current_clip()
            rebuild_tool_skin_dd_for_current_clip()
            return

        a_label = action_dd.selected() or actions[0].label
        action = _current_action_entry(action_groups, g, a_label)
        if not action:
            bundle_surfaces = {}
            rebuild_tool_dd_for_current_clip()
            rebuild_tool_skin_dd_for_current_clip()
            return

        current_action_rel_for_view = action.rel_path.as_posix()

        sel_skin = (skin_dd.selected() or "greyscale").strip()
        skin_key = "__greyscale__" if sel_skin.lower() == "greyscale" else sel_skin

        try:
            model = models[current_model_key]
            surfaces_by_dir, _size = load_skin_bundle(
                base_dir=model.base_dir,
                generated_dir=model.generated_dir,
                action_rel=action.rel_path,
                skin=skin_key,
            )
            bundle_surfaces = surfaces_by_dir
        except Exception as e:
            print(f"[WARN] failed loading bundle: {e}")
            bundle_surfaces = {}

        rebuild_tool_dd_for_current_clip()
        rebuild_tool_skin_dd_for_current_clip()
        tool_fit_state.set_clip(current_action_rel_for_view)

    rebuild_action_dd()
    rebuild_skin_dd()
    if current_model_key:
        reload_current_bundle()

    last_tick = time.time()
    running = True

    while running:
        dt = clock.tick(60) / 1000.0
        now = time.time()

        if not paused and (now - last_tick) >= (1.0 / max(1, fps)):
            frame_idx += 1
            last_tick = now

        if mode == "tool_fit":
            update_tool_fit_held_keys(tool_fit_state, zoom=zoom, dt=dt)

        for ev in pygame.event.get():
            if mode == "tool_fit":
                handle_tool_fit_mouse(ev, tool_fit_state, zoom=zoom)

            if ev.type == pygame.QUIT:
                running = False
                break

            if ev.type == pygame.VIDEORESIZE:
                if LOCK_TO_1080P:
                    screen = safe_set_mode((START_W, START_H))
                else:
                    screen = safe_set_mode((ev.w, ev.h))

            if ev.type == pygame.MOUSEWHEEL:
                if mode == "bake":
                    handle_bake_scroll(bake_state, ev.y)

            if ev.type == pygame.KEYDOWN:
                if ev.key == pygame.K_ESCAPE:
                    running = False
                    break

                if ev.key == pygame.K_TAB:
                    mode_dd.selected_index = (mode_dd.selected_index + 1) % len(mode_dd.items)
                    mode = _mode_from_dd(mode_dd)
                    ui_menu_focus = None
                    _close_all_dropdowns(
                        [
                            mode_dd,
                            model_dd,
                            group_dd,
                            action_dd,
                            skin_dd,
                            tool_dd,
                            tool_skin_dd,
                            tool_kind_dd,  # NEW
                            bake_scope_dd,
                            bake_palette_dd,
                        ]
                    )

                if mode == "viewer":
                    if ev.key == pygame.K_SPACE:
                        paused = not paused
                    if ev.key == pygame.K_LEFT:
                        fps = max(1, fps - 1)
                    if ev.key == pygame.K_RIGHT:
                        fps = min(30, fps + 1)
                    if ev.key in (pygame.K_EQUALS, pygame.K_PLUS, pygame.K_KP_PLUS):
                        zoom = min(MAX_ZOOM, zoom + 0.03)
                    if ev.key in (pygame.K_MINUS, pygame.K_KP_MINUS):
                        zoom = max(MIN_ZOOM, zoom - 0.03)
                    if ev.key == pygame.K_r:
                        reload_current_bundle()

                elif mode == "tool_fit":
                    handle_tool_fit_event(ev, tool_fit_state, zoom)
                    rebuild_tool_dd_for_current_clip()

                elif mode == "tcg":
                    if handle_tcg_event is not None:
                        handle_tcg_event(ev)

            if ev.type == pygame.MOUSEBUTTONDOWN and ev.button == 1:
                pos = ev.pos

                if mode == "viewer":
                    action, ui_menu_focus = handle_viewer_click(
                        pos,
                        mode_dd=mode_dd,
                        model_dd=model_dd,
                        group_dd=group_dd,
                        skin_dd=skin_dd,
                        tool_dd=tool_dd,
                        tool_skin_dd=tool_skin_dd,  # NEW
                        action_dd=action_dd,
                        ui_menu_focus=ui_menu_focus,
                        ui_rects=ui_rects or {},
                    )

                    if action:
                        kind = action[0]

                        if kind == "mode_changed":
                            mode = _mode_from_dd(mode_dd)

                        if kind == "model_changed":
                            current_model_key = model_dd.selected() or (model_keys[0] if model_keys else "")
                            action_groups = discover_action_groups_for_model(models[current_model_key].base_dir)
                            rebuild_action_dd()
                            rebuild_skin_dd()
                            reload_current_bundle()

                        if kind == "group_changed":
                            rebuild_action_dd()
                            reload_current_bundle()

                        if kind in ("action_changed", "skin_changed"):
                            rebuild_skin_dd()
                            reload_current_bundle()

                        if kind == "tool_changed":
                            rebuild_tool_skin_dd_for_current_clip()

                        if kind == "tool_skin_changed":
                            # Viewer reads tool_skin_dd live; nothing else needed.
                            pass

                        if kind in ("pet_new", "pet_import_done"):
                            models = discover_models()
                            model_keys = list(models.keys())
                            keep = model_dd.selected() or (model_keys[0] if model_keys else "")
                            _rebuild_dropdown(model_dd, model_keys, keep_selected_value=keep)

                            current_model_key = model_dd.selected() or (model_keys[0] if model_keys else "")
                            action_groups = discover_action_groups_for_model(models[current_model_key].base_dir)
                            rebuild_action_dd()
                            rebuild_skin_dd()
                            reload_current_bundle()

                elif mode == "bake":
                    action, ui_menu_focus = handle_bake_click(
                        pos,
                        ui_rects=ui_rects,
                        ui_menu_focus=ui_menu_focus,
                        mode_dd=mode_dd,
                        model_dd=model_dd,
                        group_dd=group_dd,
                        action_dd=action_dd,
                        bake_scope_dd=bake_scope_dd,
                        bake_palette_dd=bake_palette_dd,
                    )

                    if action:
                        kind = action[0]

                        if kind == "mode_changed":
                            mode = _mode_from_dd(mode_dd)

                        if kind == "model_changed":
                            current_model_key = model_dd.selected() or (model_keys[0] if model_keys else "")
                            action_groups = discover_action_groups_for_model(models[current_model_key].base_dir)
                            rebuild_action_dd()
                            rebuild_skin_dd()
                            reload_current_bundle()

                        if kind == "group_changed":
                            rebuild_action_dd()
                            reload_current_bundle()

                        if kind == "action_changed":
                            reload_current_bundle()

                        if kind == "bake_scan":
                            model = models[current_model_key]
                            actions = action_groups.base + action_groups.skills + action_groups.combat
                            bake_rows, pals = scan_bake_rows(
                                state=bake_state,
                                base_dir=model.base_dir,
                                generated_dir=model.generated_dir,
                                palettes_dir=model.palettes_dir,
                                actions=actions,
                            )
                            _rebuild_dropdown(
                                bake_palette_dd,
                                ["All"] + [p.name for p in pals],
                                keep_selected_value=bake_palette_dd.selected() or "All",
                            )
                            rebuild_skin_dd()

                        if kind == "bake_missing":
                            model = models[current_model_key]
                            current_action = _current_action_entry(
                                action_groups,
                                group_dd.selected() or "Base",
                                action_dd.selected() or "",
                            )
                            do_bake_operation(
                                state=bake_state,
                                base_dir=model.base_dir,
                                generated_dir=model.generated_dir,
                                palettes_dir=model.palettes_dir,
                                action_groups=action_groups,
                                group=(group_dd.selected() or "Base"),
                                current_action=current_action,
                                bake_scope_dd=bake_scope_dd,
                                bake_palette_dd=bake_palette_dd,
                                force=False,
                                clean=False,
                            )
                            rebuild_skin_dd()

                        if kind == "bake_force":
                            model = models[current_model_key]
                            current_action = _current_action_entry(
                                action_groups,
                                group_dd.selected() or "Base",
                                action_dd.selected() or "",
                            )
                            do_bake_operation(
                                state=bake_state,
                                base_dir=model.base_dir,
                                generated_dir=model.generated_dir,
                                palettes_dir=model.palettes_dir,
                                action_groups=action_groups,
                                group=(group_dd.selected() or "Base"),
                                current_action=current_action,
                                bake_scope_dd=bake_scope_dd,
                                bake_palette_dd=bake_palette_dd,
                                force=True,
                                clean=False,
                            )
                            rebuild_skin_dd()

                        if kind == "bake_clean":
                            model = models[current_model_key]
                            current_action = _current_action_entry(
                                action_groups,
                                group_dd.selected() or "Base",
                                action_dd.selected() or "",
                            )
                            do_bake_operation(
                                state=bake_state,
                                base_dir=model.base_dir,
                                generated_dir=model.generated_dir,
                                palettes_dir=model.palettes_dir,
                                action_groups=action_groups,
                                group=(group_dd.selected() or "Base"),
                                current_action=current_action,
                                bake_scope_dd=bake_scope_dd,
                                bake_palette_dd=bake_palette_dd,
                                force=False,
                                clean=True,
                            )
                            rebuild_skin_dd()

                        if kind == "tool_scan":
                            clip_rels = [a.rel_path for a in (action_groups.base + action_groups.skills + action_groups.combat)]
                            tool_rows, _ = scan_tool_bake_rows(
                                state=bake_state,
                                project_root=PROJECT_ROOT,
                                clip_rels=clip_rels,
                                tool_kind="axe",
                            )

                        if kind == "tool_bake_missing":
                            clip_rels = [a.rel_path for a in (action_groups.base + action_groups.skills + action_groups.combat)]
                            do_tool_bake_operation(
                                state=bake_state,
                                project_root=PROJECT_ROOT,
                                clip_rels=clip_rels,
                                tool_kind="axe",
                                force=False,
                                clean=False,
                            )

                        if kind == "tool_bake_force":
                            clip_rels = [a.rel_path for a in (action_groups.base + action_groups.skills + action_groups.combat)]
                            do_tool_bake_operation(
                                state=bake_state,
                                project_root=PROJECT_ROOT,
                                clip_rels=clip_rels,
                                tool_kind="axe",
                                force=True,
                                clean=False,
                            )

                        if kind == "tool_clean":
                            clip_rels = [a.rel_path for a in (action_groups.base + action_groups.skills + action_groups.combat)]
                            do_tool_bake_operation(
                                state=bake_state,
                                project_root=PROJECT_ROOT,
                                clip_rels=clip_rels,
                                tool_kind="axe",
                                force=False,
                                clean=True,
                            )

                elif mode == "manager":
                    action, ui_menu_focus = handle_manager_click(
                        pos,
                        ms=manager_state,
                        mode_dd=mode_dd,
                    )
                    if action and action[0] == "mode_changed":
                        mode = _mode_from_dd(mode_dd)

                elif mode == "tool_fit":
                    # First: header buttons (New Kind / New Tool / direction buttons)
                    if handle_tool_fit_click(pos, ui_rects=ui_rects, state=tool_fit_state):
                        # If New Kind was created, refresh tool kinds + select state.tool_kind
                        rebuild_tool_kind_dd_keep_state()
                        rebuild_tool_dd_for_current_clip()
                    else:
                        # Then: dropdowns in tool_fit
                        menus = {
                            "mode": mode_dd,
                            "model": model_dd,
                            "group": group_dd,
                            "skin": skin_dd,
                            "action": action_dd,
                            "tool_kind": tool_kind_dd,  # NEW
                            "tool": tool_dd,
                        }
                        action, ui_menu_focus = _handle_dropdown_click_generic(pos, menus=menus, ui_menu_focus=ui_menu_focus)
                        if action:
                            kind = action[0]
                            if kind == "mode_changed":
                                mode = _mode_from_dd(mode_dd)

                            if kind == "model_changed":
                                current_model_key = model_dd.selected() or (model_keys[0] if model_keys else "")
                                action_groups = discover_action_groups_for_model(models[current_model_key].base_dir)
                                rebuild_action_dd()
                                rebuild_skin_dd()
                                reload_current_bundle()

                            if kind == "group_changed":
                                rebuild_action_dd()
                                reload_current_bundle()

                            if kind in ("action_changed", "skin_changed"):
                                rebuild_skin_dd()
                                reload_current_bundle()

                            if kind == "tool_kind_changed":
                                # NEW: switch tool_fit_state kind, ensure folders/manifest exist,
                                # and refresh tool dropdown for current clip
                                selected_kind = (tool_kind_dd.selected() or "").strip()
                                if selected_kind:
                                    ensure_tool_kind_exists(selected_kind)
                                    tool_fit_state.tool_kind = selected_kind
                                    tool_fit_state.load_from_manifest()
                                    rebuild_tool_dd_for_current_clip()

                            if kind == "tool_changed":
                                # ToolFit render reads tool_dd live -> ToolFitState.set_tool_id() handles ensure in manifest.
                                pass

                elif mode == "tcg":
                    if handle_tcg_click is not None:
                        action, ui_menu_focus = handle_tcg_click(pos, ui_rects=ui_rects, ui_menu_focus=ui_menu_focus)
                        if action and action[0] == "mode_changed":
                            mode = _mode_from_dd(mode_dd)
                    else:
                        menus = {"mode": mode_dd, "model": model_dd}
                        action, ui_menu_focus = _handle_dropdown_click_generic(pos, menus=menus, ui_menu_focus=ui_menu_focus)
                        if action and action[0] == "mode_changed":
                            mode = _mode_from_dd(mode_dd)

        if mode == "viewer":
            current_label = action_dd.selected() or ""
            ui_rects = draw_viewer(
                screen,
                font,
                font_ui,
                mode_dd=mode_dd,
                model_dd=model_dd,
                group_dd=group_dd,
                skin_dd=skin_dd,
                tool_dd=tool_dd,
                tool_skin_dd=tool_skin_dd,  # NEW
                action_dd=action_dd,
                current_action_label=current_label,
                current_action_rel=current_action_rel_for_view,
                bundle_surfaces=bundle_surfaces,
                idx=frame_idx,
                fps=fps,
                zoom=zoom,
                scale_cache=scale_cache,
                ui_menu_focus=ui_menu_focus,
            )

        elif mode == "bake":
            ui_rects = render_bake_mode(
                screen=screen,
                font=font,
                font_ui=font_ui,
                mode_dd=mode_dd,
                model_dd=model_dd,
                group_dd=group_dd,
                action_dd=action_dd,
                bake_scope_dd=bake_scope_dd,
                bake_palette_dd=bake_palette_dd,
                ui_menu_focus=ui_menu_focus,
                state=bake_state,
                rows=bake_rows,
                tool_rows=tool_rows,
            )

        elif mode == "manager":
            ui_rects = render_manager(
                screen=screen,
                ms=manager_state,
                mouse_pos=pygame.mouse.get_pos(),
                mode_dd=mode_dd,
            ) or {}

        elif mode == "tool_fit":
            ui_rects = render_tool_fit(
                screen=screen,
                font=font,
                font_ui=font_ui,
                mode_dd=mode_dd,
                model_dd=model_dd,
                group_dd=group_dd,
                skin_dd=skin_dd,
                action_dd=action_dd,
                tool_kind_dd=tool_kind_dd,   # NEW
                tool_dd=tool_dd,
                humanoid_bundle=bundle_surfaces,
                zoom=zoom,
                state=tool_fit_state,
                ui_menu_focus=ui_menu_focus,
                action_rel_path=current_action_rel_for_view,
            )

        elif mode == "tcg":
            if render_tcg is not None:
                ui_rects = render_tcg(
                    screen=screen,
                    font=font,
                    font_ui=font_ui,
                    mode_dd=mode_dd,
                    model_dd=model_dd,
                    group_dd=group_dd,
                    action_dd=action_dd,
                    skin_dd=skin_dd,
                    ui_menu_focus=ui_menu_focus,
                )
            else:
                screen.fill((18, 18, 22))
                screen.blit(font.render("TCG mode not wired yet (tcg_mode.py missing).", True, (230, 200, 140)), (20, 20))
                screen.blit(font.render("Mode dropdown is still usable.", True, (180, 180, 190)), (20, 45))
                ui_rects = {}

        pygame.display.flip()

    pygame.quit()


if __name__ == "__main__":
    main()