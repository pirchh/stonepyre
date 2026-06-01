// Shared types between main process and renderer.
// Must not import Node or browser APIs.

// ── Asset categories ──────────────────────────────────────────────────────────

export const ASSET_CATEGORIES = [
  'ground',
  'ground_detail',
  'floors',
  'curbs',
  'vegetation',
  'props',
  'structures',
  'overlays',
] as const

export type AssetCategory = (typeof ASSET_CATEGORIES)[number]

// BrowserCategory includes harvest_nodes as a special non-chunk category
export type BrowserCategory = AssetCategory | 'harvest_nodes'

export const CATEGORY_LABELS: Record<BrowserCategory, string> = {
  ground:        'Ground',
  ground_detail: 'Ground Detail',
  floors:        'Floors',
  curbs:         'Curbs',
  vegetation:    'Vegetation',
  props:         'Props',
  structures:    'Structures',
  overlays:      'Overlays',
  harvest_nodes: 'Harvest Nodes',
}

// ── Harvest node definitions ───────────────────────────────────────────────────

export interface HarvestNodeDef {
  node_def_id: string       // e.g. "oak_tree" — used as node_def_id in placements
  name: string
  skill: string             // sub-category group, e.g. "woodcutting"
  color: string             // placeholder hex color for editor swatch
  blocks_movement: boolean
  available_model: string | null   // absolute path to available-state GLB, null if not yet on disk
  depleted_model: string | null    // absolute path to depleted-state GLB
}

// Global unique u16 ID ranges per category (max 65535).
// IDs are stable — never re-assign a deleted ID.
export const CATEGORY_ID_RANGES: Record<AssetCategory, [number, number]> = {
  ground:        [1,     4999],
  ground_detail: [5000,  9999],
  floors:        [10000, 14999],
  curbs:         [15000, 19999],
  vegetation:    [20000, 29999],
  props:         [30000, 39999],
  structures:    [40000, 49999],
  overlays:      [50000, 54999],
  // custom layers use 55000-65534
}

export interface AssetDef {
  id: number              // globally unique u16, 0 = empty
  name: string
  file: string | null     // PNG filename relative to category folder; null = no art yet
  walkable: boolean
  color: string           // hex fallback color for editor when no PNG
  category: AssetCategory
  group?: string          // optional sub-category (e.g. "trees", "woodcutting")
}

// ── Layers ────────────────────────────────────────────────────────────────────

export interface LayerDef {
  id: string              // stable string ID used as folder name on disk
  label: string           // display name (user-editable)
  category: AssetCategory // default asset browser tab for this layer
  z_order: number         // render order, 0 = bottom
}

export const DEFAULT_LAYERS: LayerDef[] = [
  { id: 'ground',         label: 'Ground',          category: 'ground',        z_order: 0 },
  { id: 'ground_detail',  label: 'Ground Detail',   category: 'ground_detail', z_order: 1 },
  { id: 'floor',          label: 'Floor',           category: 'floors',     z_order: 2 },
  { id: 'curb',           label: 'Curb / Edge',     category: 'curbs',      z_order: 3 },
  { id: 'vegetation_low', label: 'Vegetation Low',  category: 'vegetation', z_order: 4 },
  { id: 'vegetation_high',label: 'Vegetation High', category: 'vegetation', z_order: 5 },
  { id: 'props',          label: 'Props',           category: 'props',      z_order: 6 },
  { id: 'structure_low',  label: 'Structures',      category: 'structures', z_order: 7 },
  { id: 'structure_high', label: 'Roofs',           category: 'structures', z_order: 8 },
  { id: 'overlay',        label: 'Overlay',         category: 'overlays',   z_order: 9 },
]

export const MAX_LAYERS = 15
export const MAX_CUSTOM_LAYERS = MAX_LAYERS - DEFAULT_LAYERS.length

// ── Map manifest (v2) ─────────────────────────────────────────────────────────

export interface WorldBounds {
  width: number   // tiles
  height: number  // tiles
}

export interface MapManifest {
  version: 2
  name: string
  chunk_size: number    // tiles per chunk side — default 32
  cell_chunks: number   // chunks per cell side — default 16 → 512×512 tiles/cell
  layers: LayerDef[]    // ordered by z_order ascending
  world_bounds?: WorldBounds  // soft hint — world is still infinite; used for viewport framing
}

// ── Map list entry ─────────────────────────────────────────────────────────────

export interface MapInfo {
  name: string
  path: string
  lastModified: number       // ms timestamp
  world_bounds?: WorldBounds
}

export const DEFAULT_MANIFEST: Omit<MapManifest, 'name'> = {
  version: 2,
  chunk_size: 32,
  cell_chunks: 16,
  layers: DEFAULT_LAYERS,
}

// ── Asset catalog (one per category folder) ────────────────────────────────────

export interface AssetCatalog {
  category: AssetCategory
  assets: AssetDef[]
}

// ── World types ───────────────────────────────────────────────────────────────

export interface TilePos {
  x: number
  y: number
}

export interface HarvestNodePlacement {
  node_id: string
  node_def_id: string
  tile: TilePos
  blocks_movement: boolean
  rotation_deg: number   // Y-axis rotation in degrees, snapped to 45° increments
}

export type ChunkData = Uint16Array

// ── IPC API (renderer ↔ main) ─────────────────────────────────────────────────

export interface IpcApi {
  // Map directory
  getMapsDir(): Promise<string>
  listMaps(): Promise<MapInfo[]>
  createMapInDir(name: string, bounds?: WorldBounds): Promise<{ path: string; manifest: MapManifest }>

  // Map lifecycle (for external/legacy maps)
  openMapDialog(): Promise<string | null>
  initMap(mapPath: string, name: string): Promise<MapManifest>
  loadManifest(mapPath: string): Promise<MapManifest | null>
  saveManifest(mapPath: string, manifest: MapManifest): Promise<void>

  // Chunks (world-absolute chunk coords; main process handles cell decomposition)
  loadChunk(mapPath: string, layer: string, cx: number, cy: number): Promise<Uint16Array | null>
  saveChunk(mapPath: string, layer: string, cx: number, cy: number, data: Uint16Array): Promise<void>
  listChunks(mapPath: string, layer: string): Promise<Array<{ cx: number; cy: number }>>

  // Harvest nodes (per cell, merged for the whole map on load)
  loadHarvestNodes(mapPath: string): Promise<HarvestNodePlacement[]>
  saveHarvestNodes(mapPath: string, nodes: HarvestNodePlacement[]): Promise<void>

  // Asset library (global, lives in tools/MapEditor/assets/map_assets/)
  loadAllCatalogs(): Promise<AssetCatalog[]>
  saveCatalog(catalog: AssetCatalog): Promise<void>
  getAssetDataUrl(category: AssetCategory, file: string): Promise<string | null>
  loadHarvestNodeDefs(): Promise<HarvestNodeDef[]>
  getGlbDataUrl(absolutePath: string): Promise<string | null>
  refreshAssets(): Promise<{ catalogs: AssetCatalog[]; harvestNodeDefs: HarvestNodeDef[] }>
}
