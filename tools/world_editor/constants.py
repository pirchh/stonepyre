# tools/world_editor/constants.py
from __future__ import annotations

WINDOW_W = 1400
WINDOW_H = 900
FPS = 60

CHUNK_SIZE_DEFAULT = 256

# Important:
# This is "world units per tile" for editor camera math,
# not "draw every tile as its own rect every frame".
TILE_PX_DEFAULT = 4

OVERVIEW_CELL_PX_DEFAULT = 28

CHUNK_CACHE_MAX = 64

COLOR_BG = (18, 18, 20)
COLOR_PANEL = (26, 26, 30)
COLOR_PANEL_BORDER = (55, 55, 62)
COLOR_TEXT = (230, 230, 235)

COLOR_EMPTY_CHUNK = (95, 95, 100)
COLOR_GRID = (35, 35, 40)
COLOR_NEIGHBOR_TINT = (255, 255, 255, 55)

RIGHT_PANEL_W = 260
TOP_BAR_H = 34
PANEL_PAD = 12

PAN_SPEED_PX = 22
ZOOM_STEP = 1.12
ZOOM_MIN = 0.25
ZOOM_MAX = 12.0