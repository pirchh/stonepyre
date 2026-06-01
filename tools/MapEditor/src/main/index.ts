import { app, BrowserWindow, ipcMain, dialog, nativeImage } from 'electron'
import { join, basename } from 'path'
import {
  existsSync, mkdirSync, readFileSync, writeFileSync,
  readdirSync, statSync, rmSync,
} from 'fs'
import type {
  AssetCategory, AssetCatalog, AssetDef,
  MapManifest, MapInfo, WorldBounds, HarvestNodePlacement, HarvestNodeDef,
} from '../shared/types'
import { ASSET_CATEGORIES, DEFAULT_LAYERS, DEFAULT_MANIFEST, CATEGORY_ID_RANGES } from '../shared/types'

// ── Paths ─────────────────────────────────────────────────────────────────────
//
// __dirname in Electron main = {project}/out/main/ (both dev and prod via electron-vite)
// ../../       = tools/MapEditor/
// ../../../..  = repo root (Stonepyre/)

const PROJECT_ROOT  = join(__dirname, '..', '..')          // tools/MapEditor/
const REPO_ROOT     = join(PROJECT_ROOT, '..', '..')       // Stonepyre/
const GAME_WORLD    = join(REPO_ROOT, 'game', 'assets', 'world')

// Tile assets live inside the game repo so art auto-appears in the editor.
// game/assets/world/tiles/{category}/   — editor writes catalog.json here
const TILE_ASSETS = app.isPackaged
  ? join(process.resourcesPath, 'assets', 'tiles')  // packaged: bundled copy
  : join(GAME_WORLD, 'tiles')

// Harvest objects: game/assets/world/harvest_objects/{skill}/{node}/manifest.json
const HARVEST_OBJECTS_ROOT = app.isPackaged
  ? join(process.resourcesPath, 'assets', 'harvest_objects')
  : join(GAME_WORLD, 'harvest_objects')

// Maps storage: tools/MapEditor/Maps/
const MAPS_ROOT = app.isPackaged
  ? join(app.getPath('userData'), 'Maps')
  : join(PROJECT_ROOT, 'Maps')

function categoryDir(cat: AssetCategory): string {
  return join(TILE_ASSETS, cat)
}

// ── Helpers ───────────────────────────────────────────────────────────────────

function toDisplayName(id: string): string {
  return id.replace(/[_-]/g, ' ').replace(/\b\w/g, c => c.toUpperCase())
}

const SKILL_COLORS: Record<string, string> = {
  woodcutting: '#2e7d32',
  fishing:     '#1565c0',
  mining:      '#546e7a',
  farming:     '#f57f17',
}
function skillColor(skill: string): string {
  return SKILL_COLORS[skill] ?? '#9e9e9e'
}

function ensureDir(p: string): void {
  if (!existsSync(p)) mkdirSync(p, { recursive: true })
}

// ── Cached manifest (needed for cell decomposition during chunk ops) ───────────

let _manifest: MapManifest | null = null

function cellChunks(): number {
  return _manifest?.cell_chunks ?? 16
}

function chunkSize(): number {
  return _manifest?.chunk_size ?? 32
}

// ── Cell/chunk path helpers ────────────────────────────────────────────────────
//
// World-absolute chunk (cx, cy) is stored at:
//   <mapPath>/cells/<cellX>_<cellY>/<layerId>/<lcx>_<lcy>.bin
//
// where cellX = floor(cx / cellChunks), lcx = cx mod cellChunks.

function cellOf(cx: number): number {
  const cc = cellChunks()
  return Math.floor(cx / cc)
}

function localOf(cx: number): number {
  const cc = cellChunks()
  return ((cx % cc) + cc) % cc
}

function chunkPath(mapPath: string, layer: string, cx: number, cy: number): string {
  const cellX = cellOf(cx)
  const cellY = cellOf(cy)
  const lcx   = localOf(cx)
  const lcy   = localOf(cy)
  return join(mapPath, 'cells', `${cellX}_${cellY}`, layer, `${lcx}_${lcy}.bin`)
}

function chunkDir(mapPath: string, layer: string, cx: number, cy: number): string {
  const cellX = cellOf(cx)
  const cellY = cellOf(cy)
  return join(mapPath, 'cells', `${cellX}_${cellY}`, layer)
}

// ── Harvest nodes (global file — one JSON for whole map) ──────────────────────

function harvestNodesPath(mapPath: string): string {
  return join(mapPath, 'harvest_nodes.json')
}

// ── IPC: Maps directory ───────────────────────────────────────────────────────

ipcMain.handle('get-maps-dir', (): string => {
  ensureDir(MAPS_ROOT)
  return MAPS_ROOT
})

ipcMain.handle('list-maps', (): MapInfo[] => {
  ensureDir(MAPS_ROOT)
  const results: MapInfo[] = []

  for (const entry of readdirSync(MAPS_ROOT)) {
    const mapPath = join(MAPS_ROOT, entry)
    try {
      if (!statSync(mapPath).isDirectory()) continue
      const manifestPath = join(mapPath, 'manifest.json')
      if (!existsSync(manifestPath)) continue
      const manifest: MapManifest = JSON.parse(readFileSync(manifestPath, 'utf-8'))
      results.push({
        name: manifest.name || entry,
        path: mapPath,
        lastModified: statSync(manifestPath).mtimeMs,
        world_bounds: manifest.world_bounds,
      })
    } catch { /* skip unreadable entries */ }
  }

  return results.sort((a, b) => b.lastModified - a.lastModified)
})

ipcMain.handle('create-map-in-dir', (_e, name: string, bounds?: WorldBounds): { path: string; manifest: MapManifest } => {
  ensureDir(MAPS_ROOT)

  // Sanitise the name into a safe folder name
  const slug = name.trim().replace(/[^a-zA-Z0-9_\-. ]/g, '').replace(/\s+/g, '_') || 'map'

  // If a folder with this name already exists, append a number
  let folderName = slug
  let attempt = 1
  while (existsSync(join(MAPS_ROOT, folderName))) {
    folderName = `${slug}_${attempt++}`
  }

  const mapPath = join(MAPS_ROOT, folderName)
  ensureDir(mapPath)
  ensureDir(join(mapPath, 'cells'))

  const manifest: MapManifest = {
    ...DEFAULT_MANIFEST,
    name: name.trim(),
    world_bounds: bounds,
  }

  writeFileSync(join(mapPath, 'manifest.json'), JSON.stringify(manifest, null, 2), 'utf-8')
  writeFileSync(join(mapPath, 'harvest_nodes.json'), '[]', 'utf-8')
  _manifest = manifest

  return { path: mapPath, manifest }
})

// ── IPC: map lifecycle ────────────────────────────────────────────────────────

ipcMain.handle('open-map-dialog', async () => {
  const result = await dialog.showOpenDialog({
    title: 'Open Map Folder',
    properties: ['openDirectory'],
  })
  return result.canceled ? null : result.filePaths[0]
})

ipcMain.handle('create-map-dialog', async (_e, defaultPath: string) => {
  const result = await dialog.showOpenDialog({
    title: 'Select or Create Map Folder',
    defaultPath: defaultPath || undefined,
    properties: ['openDirectory', 'createDirectory'],
  })
  return result.canceled ? null : result.filePaths[0]
})

ipcMain.handle('init-map', (_e, mapPath: string, name: string): MapManifest => {
  ensureDir(mapPath)
  const manifestPath = join(mapPath, 'manifest.json')
  let manifest: MapManifest
  if (existsSync(manifestPath)) {
    manifest = JSON.parse(readFileSync(manifestPath, 'utf-8'))
  } else {
    manifest = { ...DEFAULT_MANIFEST, name: name || basename(mapPath) }
    writeFileSync(manifestPath, JSON.stringify(manifest, null, 2), 'utf-8')
  }
  if (!existsSync(harvestNodesPath(mapPath))) {
    writeFileSync(harvestNodesPath(mapPath), '[]', 'utf-8')
  }
  _manifest = manifest
  return manifest
})

ipcMain.handle('load-manifest', (_e, mapPath: string): MapManifest | null => {
  const p = join(mapPath, 'manifest.json')
  if (!existsSync(p)) return null
  try {
    const raw: MapManifest = JSON.parse(readFileSync(p, 'utf-8'))
    // Migrate old 'tiles' category → 'ground'
    if (raw.layers) {
      raw.layers = raw.layers.map(l =>
        (l.category as string) === 'tiles' ? { ...l, category: 'ground' as any } : l
      )
    }
    _manifest = raw
    return _manifest
  } catch {
    return null
  }
})

ipcMain.handle('save-manifest', (_e, mapPath: string, manifest: MapManifest) => {
  ensureDir(mapPath)
  writeFileSync(join(mapPath, 'manifest.json'), JSON.stringify(manifest, null, 2), 'utf-8')
  _manifest = manifest
})

// ── IPC: chunks ───────────────────────────────────────────────────────────────

ipcMain.handle('load-chunk', (_e, mapPath: string, layer: string, cx: number, cy: number): Uint16Array | null => {
  const p = chunkPath(mapPath, layer, cx, cy)
  if (!existsSync(p)) return null
  try {
    const buf = readFileSync(p)
    return new Uint16Array(buf.buffer, buf.byteOffset, buf.byteLength / 2)
  } catch {
    return null
  }
})

ipcMain.handle('save-chunk', (_e, mapPath: string, layer: string, cx: number, cy: number, data: Uint16Array) => {
  const dir = chunkDir(mapPath, layer, cx, cy)
  ensureDir(dir)
  const p = chunkPath(mapPath, layer, cx, cy)
  const buf = Buffer.from(data.buffer, data.byteOffset, data.byteLength)
  writeFileSync(p, buf)
})

ipcMain.handle('list-chunks', (_e, mapPath: string, layer: string): Array<{ cx: number; cy: number }> => {
  const cc = cellChunks()
  const cellsDir = join(mapPath, 'cells')
  if (!existsSync(cellsDir)) return []

  const results: Array<{ cx: number; cy: number }> = []

  for (const cellEntry of readdirSync(cellsDir)) {
    const parts = cellEntry.split('_')
    if (parts.length !== 2) continue
    const cellX = parseInt(parts[0])
    const cellY = parseInt(parts[1])
    if (isNaN(cellX) || isNaN(cellY)) continue

    const layerDir = join(cellsDir, cellEntry, layer)
    if (!existsSync(layerDir)) continue

    for (const chunkFile of readdirSync(layerDir)) {
      if (!chunkFile.endsWith('.bin')) continue
      const cp = chunkFile.replace('.bin', '').split('_')
      if (cp.length !== 2) continue
      const lcx = parseInt(cp[0])
      const lcy = parseInt(cp[1])
      if (isNaN(lcx) || isNaN(lcy)) continue
      results.push({ cx: cellX * cc + lcx, cy: cellY * cc + lcy })
    }
  }

  return results
})

// ── IPC: harvest nodes ────────────────────────────────────────────────────────

ipcMain.handle('load-harvest-nodes', (_e, mapPath: string): HarvestNodePlacement[] => {
  const p = harvestNodesPath(mapPath)
  if (!existsSync(p)) return []
  try { return JSON.parse(readFileSync(p, 'utf-8')) }
  catch { return [] }
})

ipcMain.handle('save-harvest-nodes', (_e, mapPath: string, nodes: HarvestNodePlacement[]) => {
  ensureDir(mapPath)
  writeFileSync(harvestNodesPath(mapPath), JSON.stringify(nodes, null, 2), 'utf-8')
})

// ── IPC: asset library ────────────────────────────────────────────────────────

function ensureDefaultCatalog(cat: AssetCategory): AssetCatalog {
  const dir = categoryDir(cat)
  ensureDir(dir)
  const catalogPath = join(dir, 'catalog.json')
  if (existsSync(catalogPath)) {
    try { return JSON.parse(readFileSync(catalogPath, 'utf-8')) as AssetCatalog }
    catch { /* fall through to rebuild */ }
  }

  // Auto-discover any PNGs already in the folder
  const pngs = existsSync(dir)
    ? readdirSync(dir).filter(f => f.toLowerCase().endsWith('.png'))
    : []

  const [idStart] = CATEGORY_ID_RANGES[cat]
  const existingAssets: AssetDef[] = pngs.map((file, i) => ({
    id: idStart + i,
    name: file.replace(/\.png$/i, '').replace(/[_-]/g, ' '),
    file,
    walkable: cat !== 'structures',
    color: placeholderColor(cat, i),
    category: cat,
  }))

  const catalog: AssetCatalog = { category: cat, assets: existingAssets }
  writeFileSync(catalogPath, JSON.stringify(catalog, null, 2), 'utf-8')
  return catalog
}

function placeholderColor(cat: AssetCategory, index: number): string {
  const palettes: Record<AssetCategory, string[]> = {
    ground:        ['#4caf50','#795548','#e6c27a','#3f6f3f','#bba27a','#5d4037','#9e9e9e','#558b2f','#33691e','#ff4500'],
    ground_detail: ['#33691e','#558b2f','#6d4c41','#78909c','#9e9e9e'],
    floors:        ['#8d6e63','#a1887f','#bcaaa4','#d7ccc8','#efebe9'],
    curbs:         ['#616161','#757575','#9e9e9e','#bdbdbd'],
    vegetation:    ['#2e7d32','#388e3c','#43a047','#66bb6a','#a5d6a7','#81c784','#558b2f','#33691e'],
    props:         ['#f57f17','#f9a825','#fbc02d','#f57c00','#e65100'],
    structures:    ['#37474f','#455a64','#546e7a','#607d8b','#78909c','#90a4ae'],
    overlays:      ['#7e57c2','#9575cd','#b39ddb','#d1c4e9'],
  }
  const pal = palettes[cat]
  return pal[index % pal.length]
}

ipcMain.handle('load-all-catalogs', (): AssetCatalog[] => {
  return ASSET_CATEGORIES.map(cat => {
    // Merge catalog.json with any new PNGs not yet registered
    const dir = categoryDir(cat)
    ensureDir(dir)
    const catalogPath = join(dir, 'catalog.json')

    let catalog = ensureDefaultCatalog(cat)

    // Find PNGs in the folder that aren't in the catalog yet
    const registeredFiles = new Set(catalog.assets.map(a => a.file).filter(Boolean))
    const pngs = existsSync(dir)
      ? readdirSync(dir).filter(f => f.toLowerCase().endsWith('.png') && !registeredFiles.has(f))
      : []

    if (pngs.length > 0) {
      const [idStart, idEnd] = CATEGORY_ID_RANGES[cat]
      const usedIds = new Set(catalog.assets.map(a => a.id))
      let nextId = idStart
      const newAssets: AssetDef[] = []
      for (const file of pngs) {
        while (usedIds.has(nextId) && nextId <= idEnd) nextId++
        if (nextId > idEnd) break
        usedIds.add(nextId)
        newAssets.push({
          id: nextId,
          name: file.replace(/\.png$/i, '').replace(/[_-]/g, ' '),
          file,
          walkable: cat !== 'structures',
          color: placeholderColor(cat, catalog.assets.length + newAssets.length),
          category: cat,
        })
      }
      catalog.assets = [...catalog.assets, ...newAssets]
      writeFileSync(catalogPath, JSON.stringify(catalog, null, 2), 'utf-8')
    }

    return catalog
  })
})

ipcMain.handle('refresh-assets', () => {
  // Delete all catalog.json files so they get rebuilt fresh from disk
  for (const cat of ASSET_CATEGORIES) {
    const catalogPath = join(categoryDir(cat), 'catalog.json')
    if (existsSync(catalogPath)) {
      try { rmSync(catalogPath) } catch { /* ignore */ }
    }
  }
  const catalogs = ASSET_CATEGORIES.map(cat => {
    const dir = categoryDir(cat)
    ensureDir(dir)
    return ensureDefaultCatalog(cat)
  })
  const harvestNodeDefs = loadHarvestNodeDefsFromDisk()
  return { catalogs, harvestNodeDefs }
})

ipcMain.handle('save-catalog', (_e, catalog: AssetCatalog) => {
  const dir = categoryDir(catalog.category)
  ensureDir(dir)
  writeFileSync(join(dir, 'catalog.json'), JSON.stringify(catalog, null, 2), 'utf-8')
})

function loadHarvestNodeDefsFromDisk(): HarvestNodeDef[] {
  if (!existsSync(HARVEST_OBJECTS_ROOT)) return []
  const defs: HarvestNodeDef[] = []

  for (const skill of readdirSync(HARVEST_OBJECTS_ROOT)) {
    const skillDir = join(HARVEST_OBJECTS_ROOT, skill)
    if (!statSync(skillDir).isDirectory()) continue

    for (const nodeFolder of readdirSync(skillDir)) {
      const nodeDir = join(skillDir, nodeFolder)
      if (!statSync(nodeDir).isDirectory()) continue
      const manifestPath = join(nodeDir, 'manifest.json')
      if (!existsSync(manifestPath)) continue

      try {
        const m = JSON.parse(readFileSync(manifestPath, 'utf-8'))
        const availGlb = m.models?.available ? join(nodeDir, m.models.available) : null
        const depGlb   = m.models?.depleted  ? join(nodeDir, m.models.depleted)  : null
        defs.push({
          node_def_id:     m.id ?? nodeFolder,
          name:            m.display_name ?? toDisplayName(m.id ?? nodeFolder),
          skill:           m.skill_id ?? skill,
          color:           skillColor(m.skill_id ?? skill),
          blocks_movement: m.blocks_movement ?? true,
          available_model: availGlb && existsSync(availGlb) ? availGlb : null,
          depleted_model:  depGlb  && existsSync(depGlb)  ? depGlb  : null,
        })
      } catch { /* skip bad manifests */ }
    }
  }

  return defs.sort((a, b) => a.skill.localeCompare(b.skill) || a.name.localeCompare(b.name))
}

ipcMain.handle('load-harvest-node-defs', (): HarvestNodeDef[] => loadHarvestNodeDefsFromDisk())

ipcMain.handle('get-glb-data-url', (_e, absolutePath: string): string | null => {
  if (!absolutePath || !existsSync(absolutePath)) return null
  try {
    const buf = readFileSync(absolutePath)
    return `data:model/gltf-binary;base64,${buf.toString('base64')}`
  } catch {
    return null
  }
})

ipcMain.handle('get-asset-data-url', (_e, category: AssetCategory, file: string): string | null => {
  const p = join(categoryDir(category), file)
  if (!existsSync(p)) return null
  try {
    const img = nativeImage.createFromPath(p)
    return img.isEmpty() ? null : img.toDataURL()
  } catch {
    return null
  }
})

// ── Window ────────────────────────────────────────────────────────────────────

function createWindow(): void {
  const win = new BrowserWindow({
    width: 1920,
    height: 1080,
    minWidth: 1280,
    minHeight: 720,
    title: 'Stonepyre Map Editor',
    backgroundColor: '#12121a',
    webPreferences: {
      preload: join(__dirname, '../preload/index.js'),
      contextIsolation: true,
      nodeIntegration: false,
    },
  })

  if (process.env['ELECTRON_RENDERER_URL']) {
    win.loadURL(process.env['ELECTRON_RENDERER_URL'])
    win.webContents.openDevTools({ mode: 'detach' })
  } else {
    win.loadFile(join(__dirname, '../renderer/index.html'))
  }
}

app.whenReady().then(() => {
  // Ensure all tile category folders exist in game assets
  for (const cat of ASSET_CATEGORIES) ensureDir(categoryDir(cat))

  createWindow()
  app.on('activate', () => {
    if (BrowserWindow.getAllWindows().length === 0) createWindow()
  })
})

app.on('window-all-closed', () => {
  if (process.platform !== 'darwin') app.quit()
})
