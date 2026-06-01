import { useState } from 'react'
import { useMapStore, type Tool } from '../store/useMapStore'


const TOOLS: Array<{ id: Tool; label: string; icon: string; shortcut: string }> = [
  { id: 'pencil',       label: 'Pencil',    icon: '✏️',  shortcut: 'P' },
  { id: 'rect',         label: 'Rect',      icon: '▭',   shortcut: 'R' },
  { id: 'fill',         label: 'Fill',      icon: '🪣',  shortcut: 'F' },
  { id: 'erase',        label: 'Erase',     icon: '⌫',   shortcut: 'E' },
  { id: 'harvest_node', label: 'H-Node',    icon: '🌳',  shortcut: 'H' },
]

export function Toolbar(): JSX.Element {
  const {
    activeTool, setTool,
    brushSize, setBrushSize,
    mapPath, manifest,
    getDirtyChunks, clearDirty,
    harvestNodes,
    activeAssetId, assetDefs, assetImages,
    fitToMap,
    showPreview3d, togglePreview3d,
  } = useMapStore()

  const handleSave = async () => {
    if (!mapPath || !manifest) return
    const dirty = getDirtyChunks()
    await Promise.all(dirty.map(({ layerId, cx, cy, data }) =>
      window.api.saveChunk(mapPath, layerId, cx, cy, data)
    ))
    await window.api.saveManifest(mapPath, manifest)
    await window.api.saveHarvestNodes(mapPath, harvestNodes)
    clearDirty()
  }

  const [refreshing, setRefreshing] = useState(false)

  const handleRefresh = async () => {
    setRefreshing(true)
    try {
      const { catalogs, harvestNodeDefs } = await window.api.refreshAssets()
      useMapStore.getState().setAssetDefs(catalogs.flatMap(c => c.assets))
      useMapStore.getState().setHarvestNodeDefs(harvestNodeDefs)
      // Reload PNG images for any newly discovered assets
      for (const catalog of catalogs) {
        for (const def of catalog.assets) {
          if (!def.file) continue
          const dataUrl = await window.api.getAssetDataUrl(def.category, def.file)
          if (!dataUrl) continue
          const img = new Image()
          img.src = dataUrl
          await new Promise<void>(resolve => { img.onload = () => resolve(); img.onerror = () => resolve() })
          useMapStore.getState().setAssetImage(def.id, img)
        }
      }
    } finally {
      setRefreshing(false)
    }
  }

  const activeAsset = assetDefs.find(a => a.id === activeAssetId)
  const activeImg   = activeAssetId ? assetImages.get(activeAssetId) : undefined

  return (
    <div style={{
      display: 'flex', alignItems: 'center', gap: 4,
      padding: '4px 10px',
      background: '#16161f',
      borderBottom: '1px solid #2a2a3a',
      height: 44,
      flexShrink: 0,
    }}>

      {/* Tool buttons */}
      {TOOLS.map(tool => (
        <button
          key={tool.id}
          title={`${tool.label} (${tool.shortcut})`}
          onClick={() => setTool(tool.id)}
          style={{
            background: activeTool === tool.id ? '#2a2a4a' : '#1e1e2e',
            border: activeTool === tool.id ? '1px solid #5a7aff' : '1px solid #2a2a3a',
            borderRadius: 4, color: '#ccd',
            padding: '3px 9px', cursor: 'pointer', fontSize: 12,
            display: 'flex', alignItems: 'center', gap: 4,
          }}
        >
          <span>{tool.icon}</span>
          <span style={{ fontSize: 11 }}>{tool.label}</span>
        </button>
      ))}

      <div style={{ width: 1, background: '#2a2a3a', height: 22, margin: '0 4px' }} />

      {/* Brush size (only for pencil/erase/rect) */}
      {(activeTool === 'pencil' || activeTool === 'erase' || activeTool === 'rect') && (
        <label style={{ fontSize: 11, color: '#778', display: 'flex', alignItems: 'center', gap: 6 }}>
          Brush
          <input
            type="range" min={1} max={10} value={brushSize}
            onChange={e => setBrushSize(parseInt(e.target.value))}
            style={{ width: 70 }}
          />
          <span style={{ color: '#aab', minWidth: 14, textAlign: 'center' }}>{brushSize}</span>
        </label>
      )}

      {manifest?.world_bounds && (
        <>
          <div style={{ width: 1, background: '#2a2a3a', height: 22, margin: '0 4px' }} />
          <button
            onClick={fitToMap}
            title="Fit map to screen (Home)"
            style={{
              background: '#1a1a2e', border: '1px solid #2a2a3a', borderRadius: 4,
              color: '#778', padding: '3px 10px', cursor: 'pointer', fontSize: 12,
            }}
          >
            ⊞ Fit
          </button>
        </>
      )}

      {/* Active asset chip */}
      {activeAsset && (
        <div style={{
          display: 'flex', alignItems: 'center', gap: 6,
          padding: '2px 8px', borderRadius: 4,
          background: '#1a1a2e', border: '1px solid #2a3a6a',
          fontSize: 11, color: '#9ab',
        }}>
          {activeImg
            ? <img src={activeImg.src} width={16} height={16} style={{ imageRendering: 'pixelated', borderRadius: 2 }} />
            : <div style={{ width: 16, height: 16, borderRadius: 2, background: activeAsset.color }} />
          }
          {activeAsset.name}
        </div>
      )}

      <div style={{ flex: 1 }} />

      {/* Coord display and save */}
      {/* 3D preview toggle */}
      {mapPath && (
        <button
          onClick={togglePreview3d}
          title="Toggle 3D preview (shows current view in 3D)"
          style={{
            background: showPreview3d ? '#1e2a4a' : '#1a1a2e',
            border: showPreview3d ? '1px solid #4a7aff' : '1px solid #2a2a3a',
            borderRadius: 4, color: showPreview3d ? '#8ab8ff' : '#556',
            padding: '4px 12px', cursor: 'pointer', fontSize: 14,
          }}
        >
          👁 3D
        </button>
      )}

      <button
        onClick={handleRefresh}
        disabled={refreshing}
        title="Rescan game/assets/world/ for new PNGs and manifests"
        style={{
          background: '#1a1a2e', border: '1px solid #2a2a3a', borderRadius: 4,
          color: refreshing ? '#445' : '#667', padding: '4px 12px',
          cursor: refreshing ? 'wait' : 'pointer', fontSize: 12,
        }}
      >
        {refreshing ? '⟳ Refreshing…' : '⟳ Refresh Assets'}
      </button>

      <button
        onClick={handleSave}
        disabled={!mapPath}
        style={{
          background: '#1a3a1a', border: '1px solid #3a6a3a', borderRadius: 4,
          color: '#8f8', padding: '4px 16px', cursor: mapPath ? 'pointer' : 'not-allowed',
          fontSize: 12,
        }}
      >
        💾 Save All
      </button>
    </div>
  )
}
