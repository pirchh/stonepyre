# tools/viewer.py
#!/usr/bin/env python3
from __future__ import annotations

import sys


def main() -> None:
    if "--world-assets" in sys.argv:
        from stonepyre_viewer.world_assets_mode import main as world_assets_main

        world_assets_main()
        return

    from stonepyre_viewer import app as viewer_app
    from stonepyre_viewer import world_assets_mode

    patch_world_assets_mode_into_dropdown(viewer_app, world_assets_mode)
    viewer_app.main()


def patch_world_assets_mode_into_dropdown(viewer_app, world_assets_mode) -> None:
    original_dropdown = viewer_app.Dropdown
    original_mode_from_dd = viewer_app._mode_from_dd

    class PatchedDropdown(original_dropdown):
        def __init__(self, *args, **kwargs):
            super().__init__(*args, **kwargs)
            if (self.title or "").strip().lower() == "mode":
                items = list(self.items)
                if "World Assets" not in items:
                    if "TCG" in items:
                        items[items.index("TCG")] = "World Assets"
                    else:
                        items.append("World Assets")
                self.items = items
                self.selected_index = min(self.selected_index, max(0, len(self.items) - 1))

    def patched_mode_from_dd(mode_dd):
        selected = (mode_dd.selected() or "Viewer").strip().lower()
        if selected == "world assets":
            # Reuse app.py's existing optional TCG mode branch as a safe extension slot.
            return "tcg"
        return original_mode_from_dd(mode_dd)

    state_holder = {"state": None}

    def get_state():
        state = state_holder.get("state")
        if state is None:
            state = world_assets_mode.WorldAssetState()
            world_assets_mode.refresh_skills(state)
            world_assets_mode.refresh_nodes(state)
            world_assets_mode.reload_previews(state)
            state_holder["state"] = state
        return state

    def render_world_assets(screen, font, font_ui, **_kwargs):
        state = get_state()
        font_sm = viewer_app.pygame.font.SysFont("consolas", 15)
        font_mid = viewer_app.pygame.font.SysFont("consolas", 22)
        font_big = viewer_app.pygame.font.SysFont("consolas", 30)
        return world_assets_mode.render(screen, font, font_sm, font_mid, font_big, state)

    def handle_world_assets_click(pos, *, ui_rects, ui_menu_focus):
        state = get_state()
        world_assets_mode.handle_click(pos, state, ui_rects or {})
        return None, ui_menu_focus

    def handle_world_assets_event(ev):
        state = get_state()

        if ev.type == viewer_app.pygame.KEYDOWN:
            if state.editing_field:
                if ev.key in (
                    viewer_app.pygame.K_RETURN,
                    viewer_app.pygame.K_BACKSPACE,
                    viewer_app.pygame.K_TAB,
                ):
                    world_assets_mode.handle_text_input_key(ev, state)
                    return

                text = getattr(ev, "unicode", "") or ""
                world_assets_mode.apply_text_input(text, state)
                return

            if ev.key == viewer_app.pygame.K_r:
                world_assets_mode.refresh_all(state)
                return

    viewer_app.Dropdown = PatchedDropdown
    viewer_app._mode_from_dd = patched_mode_from_dd
    viewer_app.render_tcg = render_world_assets
    viewer_app.handle_tcg_click = handle_world_assets_click
    viewer_app.handle_tcg_event = handle_world_assets_event


if __name__ == "__main__":
    main()
