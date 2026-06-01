import { create } from 'zustand'
import type {
  LayerDef, AssetDef, AssetCategory, BrowserCategory, HarvestNodeDef,
  MapManifest, HarvestNodePlacement,
} from '../../../shared/types'
import { DEFAULT_LAYERS, MAX_LAYERS, ASSET_CATEGORIES, CATEGORY_ID_RANGES } from '../../../shared/types'
import { startFloodFill } from '../utils/floodFill'

export type Tool = 'pencil' | 'rect' | 'fill' | 'erase' | 'harvest_node'

export interface Camera {
  x: number    // world-tile origin of viewport top-left
  y: number
  zoom: number // pixels per tile
}

interface LayerState {
  visible: boolean
  locked: boolean
}

function chunkKey(cx: number, cy: number, layerId: string): string {
  return `${layerId}:${cx}:${cy}`
}

interface MapStore {
  // ── Loaded map ──────────────────────────────────────────────────────────────
  mapPath: string | null
  manifest: MapManifest | null
  harvestNodes: HarvestNodePlacement[]

  // ── Chunk cache ─────────────────────────────────────────────────────────────
  chunks: Map<string, Uint16Array>
  dirtyChunks: Set<string>

  // ── Layer runtime state (visible/locked per layer ID) ───────────────────────
  layerStates: Record<string, LayerState>

  // ── Asset library ───────────────────────────────────────────────────────────
  assetDefs: AssetDef[]                       // all tile/prop assets
  assetImages: Map<number, HTMLImageElement>  // loaded PNGs keyed by asset ID
  harvestNodeDefs: HarvestNodeDef[]           // harvest node type definitions
  activeHarvestNodeDefId: string | null       // selected harvest node type

  // ── Editor state ────────────────────────────────────────────────────────────
  activeLayerId: string
  activeTool: Tool
  activeAssetId: number              // 0 = nothing selected
  activeCategory: BrowserCategory    // current section in asset browser
  activeGroup: string                // current sub-category group ('all' = no filter)
  brushSize: number
  camera: Camera
  viewW: number   // canvas width in px (reported by Viewport)
  viewH: number   // canvas height in px

  // ── 3D preview ───────────────────────────────────────────────────────────────
  showPreview3d: boolean
  lastTouchedChunk: { cx: number; cy: number } | null
  togglePreview3d(): void

  // ── Fill progress ────────────────────────────────────────────────────────────
  fillInProgress: boolean
  fillTileCount: number
  fillCancelRef: { cancelled: boolean } | null

  // ── Undo/redo ───────────────────────────────────────────────────────────────
  undoStack: Array<{ key: string; before: Uint16Array; after: Uint16Array }>
  redoStack: Array<{ key: string; before: Uint16Array; after: Uint16Array }>

  // ── Actions: map lifecycle ──────────────────────────────────────────────────
  openMap(path: string, manifest: MapManifest, nodes: HarvestNodePlacement[]): void
  closeMap(): void
  saveManifest(): Promise<void>

  // ── Actions: layers ─────────────────────────────────────────────────────────
  setActiveLayer(id: string): void
  toggleLayerVisible(id: string): void
  toggleLayerLocked(id: string): void
  addLayer(label: string, category: AssetCategory): void
  removeLayer(id: string): void
  renameLayer(id: string, newLabel: string): void
  reorderLayer(id: string, direction: 'up' | 'down'): void

  // ── Actions: assets ──────────────────────────────────────────────────────────
  setAssetDefs(defs: AssetDef[]): void
  setAssetImage(id: number, img: HTMLImageElement): void
  setActiveAsset(id: number, category: AssetCategory): void
  setActiveCategory(cat: BrowserCategory): void
  setActiveGroup(group: string): void
  setHarvestNodeDefs(defs: HarvestNodeDef[]): void
  setActiveHarvestNode(defId: string): void

  // ── Actions: editor ──────────────────────────────────────────────────────────
  setTool(tool: Tool): void
  setBrushSize(size: number): void
  setCamera(cam: Partial<Camera>): void
  setViewSize(w: number, h: number): void
  fitToMap(): void

  // ── Actions: chunks ──────────────────────────────────────────────────────────
  getChunk(cx: number, cy: number, layerId: string): Uint16Array | null
  setChunk(cx: number, cy: number, layerId: string, data: Uint16Array): void
  paintTiles(positions: Array<{ x: number; y: number }>): void
  eraseTiles(positions: Array<{ x: number; y: number }>): void
  startFill(worldX: number, worldY: number): void
  cancelFill(): void
  markChunkDirty(cx: number, cy: number, layerId: string): void
  getDirtyChunks(): Array<{ layerId: string; cx: number; cy: number; data: Uint16Array }>
  clearDirty(): void

  // ── Actions: undo/redo ───────────────────────────────────────────────────────
  undo(): void
  redo(): void

  // ── Actions: objects ─────────────────────────────────────────────────────────
  addHarvestNode(node: HarvestNodePlacement): void
  removeHarvestNode(nodeId: string): void
  eraseHarvestNodesAt(tiles: Array<{ x: number; y: number }>): void
}

function defaultLayerStates(layers: LayerDef[]): Record<string, LayerState> {
  return Object.fromEntries(layers.map(l => [l.id, { visible: true, locked: false }]))
}

function sortedLayers(layers: LayerDef[]): LayerDef[] {
  return [...layers].sort((a, b) => a.z_order - b.z_order)
}

export const useMapStore = create<MapStore>((set, get) => ({
  mapPath: null,
  manifest: null,
  harvestNodes: [],
  chunks: new Map(),
  dirtyChunks: new Set(),
  layerStates: defaultLayerStates(DEFAULT_LAYERS),
  assetDefs: [],
  assetImages: new Map(),
  activeLayerId: 'ground',
  activeTool: 'pencil',
  activeAssetId: 0,
  activeCategory: 'ground' as BrowserCategory,
  activeGroup: 'all',
  harvestNodeDefs: [],
  activeHarvestNodeDefId: null,
  brushSize: 1,
  camera: { x: 0, y: 0, zoom: 16 },
  viewW: 800,
  viewH: 600,
  showPreview3d: false,
  lastTouchedChunk: null,
  togglePreview3d() { set(s => ({ showPreview3d: !s.showPreview3d })) },
  fillInProgress: false,
  fillTileCount: 0,
  fillCancelRef: null,
  undoStack: [],
  redoStack: [],

  // ── Map lifecycle ─────────────────────────────────────────────────────────

  openMap(path, manifest, nodes) {
    set({
      mapPath: path,
      manifest,
      harvestNodes: nodes,
      chunks: new Map(),
      dirtyChunks: new Set(),
      activeLayerId: manifest.layers[0]?.id ?? 'ground',
      layerStates: defaultLayerStates(manifest.layers),
      camera: { x: 0, y: 0, zoom: 16 },
      undoStack: [],
      redoStack: [],
    })
  },

  closeMap() {
    set({ mapPath: null, manifest: null, harvestNodes: [], chunks: new Map(), dirtyChunks: new Set() })
  },

  async saveManifest() {
    const { mapPath, manifest } = get()
    if (!mapPath || !manifest) return
    await window.api.saveManifest(mapPath, manifest)
  },

  // ── Layers ────────────────────────────────────────────────────────────────

  setActiveLayer(id) {
    const { manifest, activeCategory, assetDefs } = get()
    if (!manifest) return
    const layer = manifest.layers.find(l => l.id === id)
    const newCategory = layer?.category ?? activeCategory
    // Auto-select first asset of that category if none selected in it
    const catAssets = assetDefs.filter(a => a.category === newCategory)
    const activeAssetId = catAssets.length > 0 ? catAssets[0].id : 0
    set({ activeLayerId: id, activeCategory: newCategory, activeAssetId })
  },

  toggleLayerVisible(id) {
    set(s => ({
      layerStates: {
        ...s.layerStates,
        [id]: { ...s.layerStates[id], visible: !s.layerStates[id]?.visible },
      }
    }))
  },

  toggleLayerLocked(id) {
    set(s => ({
      layerStates: {
        ...s.layerStates,
        [id]: { ...s.layerStates[id], locked: !s.layerStates[id]?.locked },
      }
    }))
  },

  addLayer(label, category) {
    const { manifest } = get()
    if (!manifest) return
    if (manifest.layers.length >= MAX_LAYERS) return

    const existingCustom = manifest.layers.filter(l => l.id.startsWith('custom_'))
    let idx = 0
    while (existingCustom.some(l => l.id === `custom_${idx}`)) idx++

    const newLayer: LayerDef = {
      id: `custom_${idx}`,
      label,
      category,
      z_order: manifest.layers.length,
    }

    const newManifest: MapManifest = {
      ...manifest,
      layers: sortedLayers([...manifest.layers, newLayer]),
    }

    set(s => ({
      manifest: newManifest,
      layerStates: { ...s.layerStates, [newLayer.id]: { visible: true, locked: false } },
    }))
  },

  removeLayer(id) {
    // Cannot remove the default 10 layers
    if (!id.startsWith('custom_')) return
    set(s => {
      if (!s.manifest) return s
      return {
        manifest: { ...s.manifest, layers: s.manifest.layers.filter(l => l.id !== id) },
        activeLayerId: s.activeLayerId === id ? (s.manifest.layers[0]?.id ?? 'ground') : s.activeLayerId,
      }
    })
  },

  renameLayer(id, newLabel) {
    set(s => {
      if (!s.manifest) return s
      return {
        manifest: {
          ...s.manifest,
          layers: s.manifest.layers.map(l => l.id === id ? { ...l, label: newLabel } : l),
        }
      }
    })
  },

  reorderLayer(id, direction) {
    set(s => {
      if (!s.manifest) return s
      const layers = [...s.manifest.layers].sort((a, b) => a.z_order - b.z_order)
      const idx = layers.findIndex(l => l.id === id)
      if (idx < 0) return s
      const targetIdx = direction === 'up' ? idx + 1 : idx - 1
      if (targetIdx < 0 || targetIdx >= layers.length) return s
      // Swap z_orders
      const newLayers = layers.map((l, i) => {
        if (i === idx) return { ...l, z_order: layers[targetIdx].z_order }
        if (i === targetIdx) return { ...l, z_order: layers[idx].z_order }
        return l
      })
      return { manifest: { ...s.manifest, layers: sortedLayers(newLayers) } }
    })
  },

  // ── Assets ────────────────────────────────────────────────────────────────

  setAssetDefs(defs) { set({ assetDefs: defs }) },

  setAssetImage(id, img) {
    set(s => {
      const m = new Map(s.assetImages)
      m.set(id, img)
      return { assetImages: m }
    })
  },

  setActiveAsset(id, category) {
    const cur = get().activeTool
    const tool = cur === 'harvest_node' ? 'pencil' : cur
    set({ activeAssetId: id, activeCategory: category, activeTool: tool })
  },

  setActiveCategory(cat) {
    set({ activeCategory: cat, activeGroup: 'all' })
  },

  setActiveGroup(group) { set({ activeGroup: group }) },

  setHarvestNodeDefs(defs) { set({ harvestNodeDefs: defs }) },

  setActiveHarvestNode(defId) {
    set({ activeHarvestNodeDefId: defId, activeTool: 'harvest_node' })
  },

  // ── Editor ────────────────────────────────────────────────────────────────

  setTool(tool) {
    set({ activeTool: tool })
    if (tool === 'harvest_node') set({ activeCategory: 'harvest_nodes', activeGroup: 'all' })
  },
  setBrushSize(size) { set({ brushSize: Math.max(1, Math.min(10, size)) }) },

  setCamera(cam) {
    set(s => {
      const next = { ...s.camera, ...cam }
      next.zoom = Math.max(0.5, Math.min(128, next.zoom))

      // Clamp to world bounds with a small margin (32 tiles)
      const bounds = s.manifest?.world_bounds
      if (bounds) {
        const MARGIN = 32
        const tilesW = s.viewW / next.zoom
        const tilesH = s.viewH / next.zoom
        next.x = Math.max(-MARGIN, Math.min(bounds.width  - tilesW + MARGIN, next.x))
        next.y = Math.max(-MARGIN, Math.min(bounds.height - tilesH + MARGIN, next.y))
      }
      return { camera: next }
    })
  },

  setViewSize(w, h) { set({ viewW: w, viewH: h }) },

  fitToMap() {
    const { manifest, viewW, viewH } = get()
    const bounds = manifest?.world_bounds
    if (!bounds || viewW === 0 || viewH === 0) return
    const PADDING = 0.06  // 6% margin on each side
    const zoom = Math.max(0.5, Math.min(
      128,
      Math.min(viewW / bounds.width, viewH / bounds.height) * (1 - PADDING * 2)
    ))
    const x = (bounds.width  / 2) - (viewW  / zoom / 2)
    const y = (bounds.height / 2) - (viewH  / zoom / 2)
    set({ camera: { x, y, zoom } })
  },

  // ── Chunks ────────────────────────────────────────────────────────────────

  getChunk(cx, cy, layerId) {
    return get().chunks.get(chunkKey(cx, cy, layerId)) ?? null
  },

  setChunk(cx, cy, layerId, data) {
    const key = chunkKey(cx, cy, layerId)
    set(s => {
      const chunks = new Map(s.chunks)
      chunks.set(key, data)
      return { chunks }
    })
  },

  paintTiles(positions) {
    const { manifest, activeLayerId, layerStates, activeAssetId, chunks } = get()
    if (!manifest || activeAssetId === 0) return
    if (layerStates[activeLayerId]?.locked) return

    const cs = manifest.chunk_size
    const assetId = activeAssetId

    // ── Batch by chunk for O(chunks) dirty marks instead of O(tiles) ────────
    const byChunk = new Map<string, { cx: number; cy: number; locals: Array<number> }>()

    for (const { x, y } of positions) {
      const cx = Math.floor(x / cs)
      const cy = Math.floor(y / cs)
      const lx = ((x % cs) + cs) % cs
      const ly = ((y % cs) + cs) % cs
      const key = chunkKey(cx, cy, activeLayerId)
      let entry = byChunk.get(key)
      if (!entry) { entry = { cx, cy, locals: [] }; byChunk.set(key, entry) }
      entry.locals.push(ly * cs + lx)
    }

    const newChunks = new Map(chunks)
    const newDirty = new Set(get().dirtyChunks)

    for (const [key, { cx, cy, locals }] of byChunk) {
      let chunk = newChunks.get(key)
      if (!chunk) {
        chunk = new Uint16Array(cs * cs)
        newChunks.set(key, chunk)
      }
      for (const idx of locals) chunk[idx] = assetId
      newDirty.add(key)
    }

    // Track last touched chunk for 3D preview
    const firstEntry = byChunk.values().next().value as { cx: number; cy: number } | undefined
    set({ chunks: newChunks, dirtyChunks: newDirty,
      ...(firstEntry ? { lastTouchedChunk: { cx: firstEntry.cx, cy: firstEntry.cy } } : {}),
    })
  },

  eraseTiles(positions) {
    const { manifest, activeLayerId, layerStates, chunks } = get()
    if (!manifest) return
    if (layerStates[activeLayerId]?.locked) return

    const cs = manifest.chunk_size

    const byChunk = new Map<string, { cx: number; cy: number; locals: Array<number> }>()
    for (const { x, y } of positions) {
      const cx = Math.floor(x / cs)
      const cy = Math.floor(y / cs)
      const lx = ((x % cs) + cs) % cs
      const ly = ((y % cs) + cs) % cs
      const key = chunkKey(cx, cy, activeLayerId)
      let entry = byChunk.get(key)
      if (!entry) { entry = { cx, cy, locals: [] }; byChunk.set(key, entry) }
      entry.locals.push(ly * cs + lx)
    }

    const newChunks = new Map(chunks)
    const newDirty = new Set(get().dirtyChunks)

    for (const [key, { cx, cy, locals }] of byChunk) {
      const chunk = newChunks.get(key)
      if (!chunk) continue
      for (const idx of locals) chunk[idx] = 0
      newDirty.add(key)
    }

    const firstEntry2 = byChunk.values().next().value as { cx: number; cy: number } | undefined
    set({ chunks: newChunks, dirtyChunks: newDirty,
      ...(firstEntry2 ? { lastTouchedChunk: { cx: firstEntry2.cx, cy: firstEntry2.cy } } : {}),
    })
  },

  startFill(worldX, worldY) {
    const { manifest, activeLayerId, layerStates, activeAssetId, chunks, fillInProgress } = get()
    if (!manifest || activeAssetId === 0) return
    if (layerStates[activeLayerId]?.locked) return
    if (fillInProgress) return  // only one fill at a time

    const cs = manifest.chunk_size
    const targetId = chunks.get(chunkKey(
      Math.floor(worldX / cs), Math.floor(worldY / cs), activeLayerId
    ))?.[((((worldY % cs) + cs) % cs) * cs) + ((worldX % cs) + cs) % cs] ?? 0

    const cancelRef = { cancelled: false }
    set({ fillInProgress: true, fillTileCount: 0, fillCancelRef: cancelRef })

    startFloodFill(
      {
        startX: worldX, startY: worldY,
        chunkSize: cs, layerId: activeLayerId,
        targetId, activeAssetId,
        getChunkData: (cx, cy) => get().chunks.get(chunkKey(cx, cy, activeLayerId)),
      },
      {
        onBatch: (tiles) => {
          // Re-read activeAssetId from current state in case something changed
          const { activeAssetId: aid } = get()
          if (cancelRef.cancelled) return
          // Use a direct chunk-batched write without going through paintTiles
          // (avoids re-checking locked/etc on every batch)
          const { manifest: m, chunks: c, dirtyChunks: d, activeLayerId: lid } = get()
          if (!m) return
          const s = m.chunk_size
          const byChunk = new Map<string, { cx: number; cy: number; locals: number[] }>()
          for (const { x, y } of tiles) {
            const cx2 = Math.floor(x / s)
            const cy2 = Math.floor(y / s)
            const lx = ((x % s) + s) % s
            const ly = ((y % s) + s) % s
            const key = chunkKey(cx2, cy2, lid)
            let entry = byChunk.get(key)
            if (!entry) { entry = { cx: cx2, cy: cy2, locals: [] }; byChunk.set(key, entry) }
            entry.locals.push(ly * s + lx)
          }
          const nc = new Map(c)
          const nd = new Set(d)
          for (const [key, { locals }] of byChunk) {
            let chunk = nc.get(key)
            if (!chunk) { chunk = new Uint16Array(s * s); nc.set(key, chunk) }
            for (const idx of locals) chunk[idx] = aid
            nd.add(key)
          }
          set({ chunks: nc, dirtyChunks: nd })
        },
        onProgress: (count) => {
          if (!cancelRef.cancelled) set({ fillTileCount: count })
        },
        onDone: (total) => {
          set({ fillInProgress: false, fillTileCount: total, fillCancelRef: null })
        },
        isCancelled: () => cancelRef.cancelled,
      }
    )
  },

  cancelFill() {
    const { fillCancelRef } = get()
    if (fillCancelRef) fillCancelRef.cancelled = true
    set({ fillInProgress: false, fillCancelRef: null })
  },

  markChunkDirty(cx, cy, layerId) {
    set(s => {
      const dirty = new Set(s.dirtyChunks)
      dirty.add(chunkKey(cx, cy, layerId))
      return { dirtyChunks: dirty }
    })
  },

  getDirtyChunks() {
    const { dirtyChunks, chunks } = get()
    return Array.from(dirtyChunks).flatMap(key => {
      const parts = key.split(':')
      if (parts.length < 3) return []
      const layerId = parts.slice(0, parts.length - 2).join(':')
      const cx = parseInt(parts[parts.length - 2])
      const cy = parseInt(parts[parts.length - 1])
      const data = chunks.get(key)
      if (!data) return []
      return [{ layerId, cx, cy, data }]
    })
  },

  clearDirty() { set({ dirtyChunks: new Set() }) },

  // ── Undo / Redo ───────────────────────────────────────────────────────────

  undo() {
    const { undoStack, chunks } = get()
    if (undoStack.length === 0) return
    const entry = undoStack[undoStack.length - 1]
    const newChunks = new Map(chunks)
    newChunks.set(entry.key, entry.before)
    set(s => ({
      chunks: newChunks,
      undoStack: s.undoStack.slice(0, -1),
      redoStack: [...s.redoStack, entry],
    }))
  },

  redo() {
    const { redoStack, chunks } = get()
    if (redoStack.length === 0) return
    const entry = redoStack[redoStack.length - 1]
    const newChunks = new Map(chunks)
    newChunks.set(entry.key, entry.after)
    set(s => ({
      chunks: newChunks,
      redoStack: s.redoStack.slice(0, -1),
      undoStack: [...s.undoStack, entry],
    }))
  },

  // ── Objects ───────────────────────────────────────────────────────────────

  addHarvestNode(node) {
    const { manifest } = get()
    const cs = manifest?.chunk_size ?? 32
    const cx = Math.floor(node.tile.x / cs)
    const cy = Math.floor(node.tile.y / cs)
    set(s => ({ harvestNodes: [...s.harvestNodes, node], lastTouchedChunk: { cx, cy } }))
  },

  removeHarvestNode(nodeId) {
    set(s => {
      const removed = s.harvestNodes.find(n => n.node_id === nodeId)
      const cs = s.manifest?.chunk_size ?? 32
      const lastTouchedChunk = removed
        ? { cx: Math.floor(removed.tile.x / cs), cy: Math.floor(removed.tile.y / cs) }
        : s.lastTouchedChunk
      return { harvestNodes: s.harvestNodes.filter(n => n.node_id !== nodeId), lastTouchedChunk }
    })
  },

  eraseHarvestNodesAt(tiles) {
    set(s => {
      const tileSet = new Set(tiles.map(t => `${t.x},${t.y}`))
      const removed = s.harvestNodes.find(n => tileSet.has(`${n.tile.x},${n.tile.y}`))
      const cs = s.manifest?.chunk_size ?? 32
      const lastTouchedChunk = removed
        ? { cx: Math.floor(removed.tile.x / cs), cy: Math.floor(removed.tile.y / cs) }
        : s.lastTouchedChunk
      return {
        harvestNodes: s.harvestNodes.filter(n => !tileSet.has(`${n.tile.x},${n.tile.y}`)),
        lastTouchedChunk,
      }
    })
  },
}))
