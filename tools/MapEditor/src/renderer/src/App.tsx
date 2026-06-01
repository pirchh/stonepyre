import { useEffect, useRef, useState } from 'react'
import { useMapStore } from './store/useMapStore'
import { Toolbar }      from './components/Toolbar'
import { Viewport }     from './components/Viewport'
import { LayerPanel }   from './components/LayerPanel'
import { AssetBrowser } from './components/AssetBrowser'
import type { AssetDef, MapInfo, WorldBounds } from '../../shared/types'

const LOAD_MARGIN = 2  // extra chunks beyond viewport to preload

// ── Asset loader: runs once on map open ───────────────────────────────────────

async function loadAllAssets(
  setAssetDefs: (defs: AssetDef[]) => void,
  setAssetImage: (id: number, img: HTMLImageElement) => void,
  setHarvestNodeDefs?: (defs: import('../../shared/types').HarvestNodeDef[]) => void,
): Promise<void> {
  if (setHarvestNodeDefs) {
    const defs = await window.api.loadHarvestNodeDefs()
    setHarvestNodeDefs(defs)
  }
  const catalogs = await window.api.loadAllCatalogs()
  const allDefs = catalogs.flatMap(c => c.assets)
  setAssetDefs(allDefs)

  // Load PNG data URLs in parallel, then create Image objects
  await Promise.all(allDefs.map(async def => {
    if (!def.file) return
    try {
      const dataUrl = await window.api.getAssetDataUrl(def.category, def.file)
      if (!dataUrl) return
      const img = new Image()
      img.src = dataUrl
      await new Promise<void>(resolve => {
        img.onload  = () => resolve()
        img.onerror = () => resolve()
      })
      setAssetImage(def.id, img)
    } catch { /* ignore */ }
  }))
}

// ── Chunk lazy loader: fires 150ms after camera/layer settles ─────────────────

function useChunkLoader(): void {
  const requestedRef = useRef(new Set<string>())
  const mapPath    = useMapStore(s => s.mapPath)
  const manifest   = useMapStore(s => s.manifest)
  const camera     = useMapStore(s => s.camera)
  const activeLayerId = useMapStore(s => s.activeLayerId)

  useEffect(() => { requestedRef.current = new Set() }, [mapPath])

  useEffect(() => {
    if (!mapPath || !manifest) return
    const timer = setTimeout(() => {
      const cs = manifest.chunk_size
      const vw = window.innerWidth
      const vh = window.innerHeight

      const cxMin = Math.floor(camera.x / cs) - LOAD_MARGIN
      const cxMax = Math.ceil((camera.x + vw / camera.zoom) / cs) + LOAD_MARGIN
      const cyMin = Math.floor(camera.y / cs) - LOAD_MARGIN
      const cyMax = Math.ceil((camera.y + vh / camera.zoom) / cs) + LOAD_MARGIN

      for (let cy = cyMin; cy <= cyMax; cy++) {
        for (let cx = cxMin; cx <= cxMax; cx++) {
          const key = `${activeLayerId}:${cx}:${cy}`
          if (requestedRef.current.has(key)) continue
          requestedRef.current.add(key)
          window.api.loadChunk(mapPath, activeLayerId, cx, cy).then((data: Uint16Array | null) => {
            if (data) useMapStore.getState().setChunk(cx, cy, activeLayerId, data)
          })
        }
      }
    }, 150)
    return () => clearTimeout(timer)
  }, [mapPath, manifest, camera, activeLayerId])
}

// ── Keyboard shortcuts ────────────────────────────────────────────────────────

function useKeyboardShortcuts(): void {
  useEffect(() => {
    const handler = (e: KeyboardEvent): void => {
      if (e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement) return
      const s = useMapStore.getState()
      const ctrl = e.ctrlKey || e.metaKey
      if (ctrl && e.key === 'z') { e.preventDefault(); s.undo(); return }
      if (ctrl && (e.key === 'y' || (e.shiftKey && e.key === 'z'))) { e.preventDefault(); s.redo(); return }
      if (ctrl && e.key === 's') { e.preventDefault(); /* save handled by toolbar */ return }
      if (e.key === 'Home') { e.preventDefault(); s.fitToMap(); return }
      switch (e.key.toLowerCase()) {
        case 'p': s.setTool('pencil');       break
        case 'r': s.setTool('rect');         break
        case 'f': s.setTool('fill');         break
        case 'e': s.setTool('erase');        break
        case 'h': s.setTool('harvest_node'); break
      }
    }
    window.addEventListener('keydown', handler)
    return () => window.removeEventListener('keydown', handler)
  }, [])
}

// ── Status bar ────────────────────────────────────────────────────────────────

function StatusBar(): JSX.Element {
  const activeLayerId = useMapStore(s => s.activeLayerId)
  const activeTool    = useMapStore(s => s.activeTool)
  const zoom          = useMapStore(s => s.camera.zoom)
  const mapPath       = useMapStore(s => s.mapPath)
  const manifest      = useMapStore(s => s.manifest)
  const layer         = manifest?.layers.find(l => l.id === activeLayerId)

  return (
    <div style={{
      height: 22, background: '#0e0e18', borderTop: '1px solid #1e1e2e',
      display: 'flex', alignItems: 'center', padding: '0 12px', gap: 20,
      fontSize: 10, color: '#556', flexShrink: 0,
    }}>
      <span style={{ color: '#5a6a8a', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap', maxWidth: 300 }}>
        {mapPath ?? '—'}
      </span>
      <span>Layer: <span style={{ color: '#8a9aaa' }}>{layer?.label ?? activeLayerId}</span></span>
      <span>Tool: <span style={{ color: '#8a9aaa' }}>{activeTool}</span></span>
      <span>Zoom: <span style={{ color: '#8a9aaa' }}>{zoom.toFixed(0)}px</span></span>
      <span>Chunk: <span style={{ color: '#8a9aaa' }}>{manifest?.chunk_size ?? '?'}×{manifest?.chunk_size ?? '?'}</span></span>
      <span>Cell: <span style={{ color: '#8a9aaa' }}>{manifest ? manifest.cell_chunks * manifest.chunk_size : '?'}t</span></span>
      <div style={{ flex: 1 }} />
      <span style={{ color: '#2a3a3a' }}>P·R·F·E·H — tools · Alt+drag — pan · Ctrl+Z/Y — undo/redo</span>
    </div>
  )
}

// ── Editor layout ─────────────────────────────────────────────────────────────

function Editor(): JSX.Element {
  useChunkLoader()
  useKeyboardShortcuts()

  // Auto-fit to map bounds on first mount — wait a tick for Viewport to report its size
  useEffect(() => {
    const t = setTimeout(() => useMapStore.getState().fitToMap(), 80)
    return () => clearTimeout(t)
  }, [])

  return (
    <div style={{ display: 'flex', flexDirection: 'column', height: '100vh', overflow: 'hidden', background: '#12121a' }}>
      <Toolbar />
      <div style={{ display: 'flex', flex: 1, overflow: 'hidden' }}>

        {/* Left panel: asset browser */}
        <div style={{
          width: 220, flexShrink: 0, display: 'flex', flexDirection: 'column',
          background: '#1a1a26', borderRight: '1px solid #2a2a3a',
        }}>
          <AssetBrowser />
        </div>

        {/* Centre: canvas viewport */}
        <div style={{ flex: 1, position: 'relative', overflow: 'hidden' }}>
          <Viewport />
        </div>

        {/* Right panel: layer stack */}
        <LayerPanel />
      </div>
      <StatusBar />
    </div>
  )
}

// ── Size presets ──────────────────────────────────────────────────────────────

const MAX_WORLD_TILES = 50_000

const SIZE_PRESETS = [
  { id: 'tiny',   label: 'Tiny',    sub: '512 × 512',        w: 512,   h: 512   },
  { id: 'small',  label: 'Small',   sub: '2,048 × 2,048',    w: 2048,  h: 2048  },
  { id: 'medium', label: 'Medium',  sub: '8,192 × 8,192',    w: 8192,  h: 8192  },
  { id: 'osrs',   label: 'OSRS',    sub: '12,800 × 12,800',  w: 12800, h: 12800 },
  { id: 'osrs2x', label: 'OSRS ×2', sub: '25,600 × 25,600',  w: 25600, h: 25600 },
  { id: 'osrs3x', label: 'OSRS ×3', sub: '38,400 × 38,400',  w: 38400, h: 38400 },
  { id: 'max',    label: 'Max',     sub: '50,000 × 50,000',  w: 50000, h: 50000 },
  { id: 'custom', label: 'Custom',  sub: `up to ${MAX_WORLD_TILES.toLocaleString()}`, w: 0, h: 0 },
]

function fmtTiles(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`
  if (n >= 1_000)     return `${(n / 1_000).toFixed(0)}k`
  return String(n)
}

// ── New Map wizard ────────────────────────────────────────────────────────────

function NewMapWizard({ onCancel, onCreated }: {
  onCancel: () => void
  onCreated: (path: string, manifest: import('../../shared/types').MapManifest) => void
}): JSX.Element {
  const [name, setName]       = useState('')
  const [preset, setPreset]   = useState('medium')
  const [customW, setCustomW] = useState(8192)
  const [customH, setCustomH] = useState(8192)
  const [creating, setCreating] = useState(false)
  const [error, setError]     = useState('')

  const sel = SIZE_PRESETS.find(p => p.id === preset)!
  const bounds: WorldBounds = preset === 'custom'
    ? { width: customW, height: customH }
    : { width: sel.w, height: sel.h }
  const cells = Math.ceil(bounds.width / 512) * Math.ceil(bounds.height / 512)

  const handleCreate = async () => {
    if (!name.trim()) { setError('Name is required'); return }
    setError(''); setCreating(true)
    try {
      const { path, manifest } = await window.api.createMapInDir(name.trim(), bounds)
      onCreated(path, manifest)
    } catch (e) { setError(String(e)); setCreating(false) }
  }

  return (
    <div style={ws.card}>
      <div style={ws.cardTitle}>New Map</div>

      <label style={ws.label}>Map Name</label>
      <input
        autoFocus value={name}
        onChange={e => { setName(e.target.value); setError('') }}
        onKeyDown={e => { if (e.key === 'Enter') handleCreate(); if (e.key === 'Escape') onCancel() }}
        placeholder="e.g. Ironveil Continent"
        style={ws.input}
      />

      <label style={ws.label}>
        World Size <span style={{ color: '#445', fontWeight: 400 }}>— soft bounds, world stays infinite</span>
      </label>
      <div style={{ display: 'grid', gridTemplateColumns: 'repeat(4, 1fr)', gap: 5, marginBottom: 12 }}>
        {SIZE_PRESETS.map(p => (
          <button key={p.id} onClick={() => setPreset(p.id)} style={{
            padding: '6px 4px', borderRadius: 5, cursor: 'pointer', textAlign: 'center',
            border: preset === p.id ? '1px solid #5a7aff' : '1px solid #2a2a3a',
            background: preset === p.id ? '#1e2040' : '#14141e',
          }}>
            <div style={{ fontSize: 11, color: preset === p.id ? '#a0b4ff' : '#aab', fontWeight: 600 }}>{p.label}</div>
            <div style={{ fontSize: 9, color: '#445', marginTop: 1 }}>{p.sub}</div>
          </button>
        ))}
      </div>

      {preset === 'custom' && (
        <div style={{ display: 'flex', gap: 8, alignItems: 'center', marginBottom: 12 }}>
          <input type="number" min={512} max={MAX_WORLD_TILES} step={512} value={customW}
            onChange={e => setCustomW(Math.max(512, Math.min(MAX_WORLD_TILES, parseInt(e.target.value) || 512)))}
            style={{ ...ws.input, width: 110, marginBottom: 0 }}
          />
          <span style={{ color: '#445' }}>×</span>
          <input type="number" min={512} max={MAX_WORLD_TILES} step={512} value={customH}
            onChange={e => setCustomH(Math.max(512, Math.min(MAX_WORLD_TILES, parseInt(e.target.value) || 512)))}
            style={{ ...ws.input, width: 110, marginBottom: 0 }}
          />
          <span style={{ color: '#556', fontSize: 11 }}>tiles</span>
        </div>
      )}

      <div style={{ fontSize: 11, color: '#445', marginBottom: 16, lineHeight: 1.9 }}>
        <span style={{ color: '#5a6a8a' }}>{fmtTiles(bounds.width)} × {fmtTiles(bounds.height)} tiles</span>
        {' · '}~{cells.toLocaleString()} cells{' · '}chunks 32×32{' · '}cells 512×512
      </div>

      {error && <div style={{ color: '#f88', fontSize: 12, marginBottom: 10 }}>{error}</div>}

      <div style={{ display: 'flex', gap: 10 }}>
        <button onClick={onCancel} disabled={creating} style={ws.btnSecondary}>Cancel</button>
        <button onClick={handleCreate} disabled={creating || !name.trim()} style={ws.btnPrimary}>
          {creating ? 'Creating…' : 'Create Map'}
        </button>
      </div>
    </div>
  )
}

// ── Map card ──────────────────────────────────────────────────────────────────

function MapCard({ info, onOpen }: { info: MapInfo; onOpen: () => void }): JSX.Element {
  const b = info.world_bounds
  const sizeLabel = b ? `${fmtTiles(b.width)} × ${fmtTiles(b.height)} tiles` : 'unbounded'
  const d = new Date(info.lastModified)
  const dateLabel = d.toLocaleDateString(undefined, { month: 'short', day: 'numeric', year: 'numeric' })

  return (
    <div onClick={onOpen} style={ws.mapCard}
      onMouseEnter={e => (e.currentTarget.style.borderColor = '#4a5aaa')}
      onMouseLeave={e => (e.currentTarget.style.borderColor = '#2a2a3a')}
    >
      <div style={{ fontSize: 22, opacity: 0.45 }}>🗺️</div>
      <div style={{ flex: 1, minWidth: 0 }}>
        <div style={{ fontSize: 13, color: '#d0ccff', fontWeight: 600, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
          {info.name}
        </div>
        <div style={{ fontSize: 10, color: '#445', marginTop: 2 }}>
          {sizeLabel} · {dateLabel}
        </div>
      </div>
      <div style={{ fontSize: 11, color: '#3a5a8a' }}>Open →</div>
    </div>
  )
}

// ── Welcome / map browser ─────────────────────────────────────────────────────

function WelcomeScreen(): JSX.Element {
  const { openMap, setAssetDefs, setAssetImage, setHarvestNodeDefs } = useMapStore()
  const [maps, setMaps]     = useState<MapInfo[]>([])
  const [mode, setMode]     = useState<'browse' | 'new'>('browse')
  const [loading, setLoading] = useState(false)
  const [mapsDir, setMapsDir] = useState('')

  useEffect(() => {
    window.api.getMapsDir().then(dir => { setMapsDir(dir); return window.api.listMaps() }).then(setMaps)
  }, [])

  const doOpen = async (path: string) => {
    const manifest = await window.api.loadManifest(path)
    if (!manifest) { alert('Manifest not found in that folder.'); return }
    const nodes = await window.api.loadHarvestNodes(path)
    setLoading(true)
    openMap(path, manifest, nodes)
    const existing = await window.api.listChunks(path, 'ground')
    for (const { cx, cy } of existing) {
      const data = await window.api.loadChunk(path, 'ground', cx, cy)
      if (data) useMapStore.getState().setChunk(cx, cy, 'ground', data)
    }
    await loadAllAssets(setAssetDefs, setAssetImage, setHarvestNodeDefs)
    setLoading(false)
  }

  const handleCreated = async (path: string, manifest: import('../../shared/types').MapManifest) => {
    const nodes = await window.api.loadHarvestNodes(path)
    setLoading(true)
    openMap(path, manifest, nodes)
    await loadAllAssets(setAssetDefs, setAssetImage, setHarvestNodeDefs)
    setLoading(false)
  }

  if (loading) return (
    <div style={{ ...ws.screen, gap: 16 }}>
      <div style={{ color: '#6a8aaa', fontSize: 14 }}>Loading map…</div>
    </div>
  )

  if (mode === 'new') return (
    <div style={ws.screen}>
      <NewMapWizard onCancel={() => setMode('browse')} onCreated={handleCreated} />
    </div>
  )

  return (
    <div style={ws.screen}>
      <div style={{ textAlign: 'center', marginBottom: 4 }}>
        <div style={{ fontSize: 30, fontWeight: 700, letterSpacing: 4, color: '#d4ccff' }}>Stonepyre</div>
        <div style={{ fontSize: 11, letterSpacing: 3, color: '#445', marginTop: 4 }}>MAP EDITOR</div>
      </div>

      <div style={ws.card}>
        <div style={{ display: 'flex', alignItems: 'center', marginBottom: 14, gap: 8 }}>
          <div style={{ flex: 1 }}>
            <div style={{ fontSize: 13, color: '#778', fontWeight: 600 }}>Maps</div>
            <div style={{ fontSize: 9, color: '#334', marginTop: 1, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
              {mapsDir}
            </div>
          </div>
          <button onClick={() => { window.api.openMapDialog().then(p => { if (p) doOpen(p) }) }} style={ws.btnSecondary}>
            📁 Browse…
          </button>
          <button onClick={() => setMode('new')} style={ws.btnPrimary}>＋ New Map</button>
        </div>

        <div style={{ display: 'flex', flexDirection: 'column', gap: 6, maxHeight: 380, overflowY: 'auto' }}>
          {maps.length === 0 && (
            <div style={{ textAlign: 'center', padding: '28px 0', color: '#334', fontSize: 12 }}>
              No maps yet — click <span style={{ color: '#8ab8ff' }}>＋ New Map</span> to get started
            </div>
          )}
          {maps.map(info => (
            <MapCard key={info.path} info={info} onOpen={() => doOpen(info.path)} />
          ))}
        </div>
      </div>
    </div>
  )
}

// ── Welcome screen styles ─────────────────────────────────────────────────────

const ws = {
  screen: {
    display: 'flex', flexDirection: 'column', alignItems: 'center', justifyContent: 'center',
    height: '100vh', background: '#12121a', color: '#ccd', gap: 20, padding: 24,
  } as React.CSSProperties,
  card: {
    background: '#16161f', border: '1px solid #2a2a3a', borderRadius: 8,
    padding: '20px 24px', width: '100%', maxWidth: 600,
  } as React.CSSProperties,
  cardTitle: {
    fontSize: 17, fontWeight: 700, color: '#d0ccff', marginBottom: 18,
  } as React.CSSProperties,
  mapCard: {
    padding: '10px 14px', borderRadius: 6, border: '1px solid #2a2a3a',
    background: '#14141e', cursor: 'pointer',
    display: 'flex', alignItems: 'center', gap: 14,
  } as React.CSSProperties,
  label: {
    display: 'block', fontSize: 10, color: '#667', fontWeight: 600,
    letterSpacing: 0.5, textTransform: 'uppercase' as const, marginBottom: 6,
  } as React.CSSProperties,
  input: {
    width: '100%', background: '#0e0e18', border: '1px solid #2a2a3a',
    borderRadius: 5, color: '#aab', fontSize: 13, padding: '7px 12px',
    outline: 'none', marginBottom: 14, boxSizing: 'border-box' as const,
  } as React.CSSProperties,
  btnPrimary: {
    padding: '7px 18px', fontSize: 12, borderRadius: 5, cursor: 'pointer', fontWeight: 600,
    background: '#1e3a24', border: '1px solid #3a7a44', color: '#80ee90',
  } as React.CSSProperties,
  btnSecondary: {
    padding: '7px 14px', fontSize: 11, borderRadius: 5, cursor: 'pointer',
    background: '#1a1a28', border: '1px solid #2a2a3a', color: '#667',
  } as React.CSSProperties,
}

// ── Root ──────────────────────────────────────────────────────────────────────

export default function App(): JSX.Element {
  const mapPath = useMapStore(s => s.mapPath)
  return mapPath ? <Editor /> : <WelcomeScreen />
}
