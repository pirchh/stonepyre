import { useState } from 'react'
import { useMapStore } from '../store/useMapStore'
import { ASSET_CATEGORIES, CATEGORY_LABELS, MAX_LAYERS, DEFAULT_LAYERS } from '../../../shared/types'
import type { AssetCategory } from '../../../shared/types'

const DEFAULT_IDS = new Set(DEFAULT_LAYERS.map(l => l.id))

export function LayerPanel(): JSX.Element {
  const {
    manifest, activeLayerId, layerStates,
    setActiveLayer, toggleLayerVisible, toggleLayerLocked,
    addLayer, removeLayer, renameLayer, reorderLayer, saveManifest,
  } = useMapStore()

  const [addingLayer, setAddingLayer] = useState(false)
  const [newLayerLabel, setNewLayerLabel] = useState('')
  const [newLayerCategory, setNewLayerCategory] = useState<AssetCategory>('tiles')
  const [renamingId, setRenamingId] = useState<string | null>(null)
  const [renameValue, setRenameValue] = useState('')

  if (!manifest) {
    return (
      <div style={styles.panel}>
        <div style={styles.header}>Layers</div>
      </div>
    )
  }

  const layers = [...manifest.layers].sort((a, b) => b.z_order - a.z_order) // top = high z_order
  const canAddMore = manifest.layers.length < MAX_LAYERS

  const handleAddConfirm = () => {
    if (!newLayerLabel.trim()) return
    addLayer(newLayerLabel.trim(), newLayerCategory)
    saveManifest()
    setAddingLayer(false)
    setNewLayerLabel('')
  }

  const handleRenameConfirm = (id: string) => {
    if (renameValue.trim()) {
      renameLayer(id, renameValue.trim())
      saveManifest()
    }
    setRenamingId(null)
  }

  return (
    <div style={styles.panel}>
      <div style={styles.header}>
        <span>Layers</span>
        <span style={{ color: '#445', fontWeight: 400, fontSize: 10 }}>
          {manifest.layers.length}/{MAX_LAYERS}
        </span>
        {canAddMore && (
          <button
            onClick={() => setAddingLayer(v => !v)}
            title="Add layer"
            style={styles.iconBtn}
          >
            ＋
          </button>
        )}
      </div>

      <div style={styles.list}>
        {layers.map(layer => {
          const ls = layerStates[layer.id] ?? { visible: true, locked: false }
          const isActive = layer.id === activeLayerId
          const isDefault = DEFAULT_IDS.has(layer.id)
          const isRenaming = renamingId === layer.id

          return (
            <div
              key={layer.id}
              style={rowStyle(isActive, ls.locked)}
              onClick={() => !ls.locked && setActiveLayer(layer.id)}
            >
              {/* Visibility */}
              <span
                style={styles.icon}
                title={ls.visible ? 'Hide' : 'Show'}
                onClick={e => { e.stopPropagation(); toggleLayerVisible(layer.id) }}
              >
                {ls.visible ? '👁' : '🔕'}
              </span>

              {/* Lock */}
              <span
                style={styles.icon}
                title={ls.locked ? 'Unlock' : 'Lock'}
                onClick={e => { e.stopPropagation(); toggleLayerLocked(layer.id) }}
              >
                {ls.locked ? '🔒' : '🔓'}
              </span>

              {/* Label / rename input */}
              <div style={{ flex: 1, minWidth: 0 }}>
                {isRenaming ? (
                  <input
                    autoFocus
                    value={renameValue}
                    onChange={e => setRenameValue(e.target.value)}
                    onBlur={() => handleRenameConfirm(layer.id)}
                    onKeyDown={e => {
                      if (e.key === 'Enter') handleRenameConfirm(layer.id)
                      if (e.key === 'Escape') setRenamingId(null)
                    }}
                    onClick={e => e.stopPropagation()}
                    style={styles.renameInput}
                  />
                ) : (
                  <span
                    style={styles.label}
                    onDoubleClick={e => {
                      e.stopPropagation()
                      setRenamingId(layer.id)
                      setRenameValue(layer.label)
                    }}
                    title="Double-click to rename"
                  >
                    {layer.label}
                  </span>
                )}
                <div style={styles.sublabel}>{CATEGORY_LABELS[layer.category]}</div>
              </div>

              {/* Reorder up/down */}
              <div style={{ display: 'flex', flexDirection: 'column', gap: 1 }}
                   onClick={e => e.stopPropagation()}>
                <span style={styles.orderBtn} onClick={() => { reorderLayer(layer.id, 'up'); saveManifest() }} title="Move up">▲</span>
                <span style={styles.orderBtn} onClick={() => { reorderLayer(layer.id, 'down'); saveManifest() }} title="Move down">▼</span>
              </div>

              {/* Remove (custom layers only) */}
              {!isDefault && (
                <span
                  style={{ ...styles.icon, color: '#663' }}
                  title="Remove layer"
                  onClick={e => { e.stopPropagation(); removeLayer(layer.id); saveManifest() }}
                >
                  ✕
                </span>
              )}
            </div>
          )
        })}

        {/* Add layer form */}
        {addingLayer && (
          <div style={styles.addForm}>
            <input
              autoFocus
              placeholder="Layer name..."
              value={newLayerLabel}
              onChange={e => setNewLayerLabel(e.target.value)}
              onKeyDown={e => { if (e.key === 'Enter') handleAddConfirm(); if (e.key === 'Escape') setAddingLayer(false) }}
              style={styles.renameInput}
            />
            <select
              value={newLayerCategory}
              onChange={e => setNewLayerCategory(e.target.value as AssetCategory)}
              style={styles.select}
            >
              {ASSET_CATEGORIES.map(cat => (
                <option key={cat} value={cat}>{CATEGORY_LABELS[cat]}</option>
              ))}
            </select>
            <button onClick={handleAddConfirm} style={styles.addBtn}>Add</button>
            <button onClick={() => setAddingLayer(false)} style={{ ...styles.addBtn, background: '#2a1a1a' }}>✕</button>
          </div>
        )}
      </div>
    </div>
  )
}

// ── Styles ────────────────────────────────────────────────────────────────────

const styles = {
  panel: {
    width: 220,
    background: '#1a1a26',
    borderLeft: '1px solid #2a2a3a',
    display: 'flex',
    flexDirection: 'column',
    overflow: 'hidden',
  } as React.CSSProperties,
  header: {
    padding: '8px 10px',
    fontSize: 11,
    fontWeight: 700,
    letterSpacing: 1,
    color: '#6a6a8a',
    borderBottom: '1px solid #2a2a3a',
    textTransform: 'uppercase',
    display: 'flex',
    alignItems: 'center',
    gap: 6,
  } as React.CSSProperties,
  list: {
    flex: 1,
    overflowY: 'auto',
  } as React.CSSProperties,
  icon: {
    fontSize: 12,
    padding: '0 2px',
    cursor: 'pointer',
    color: '#667',
    userSelect: 'none',
    flexShrink: 0,
  } as React.CSSProperties,
  orderBtn: {
    fontSize: 8,
    color: '#445',
    cursor: 'pointer',
    padding: '0 2px',
    userSelect: 'none',
    lineHeight: 1,
  } as React.CSSProperties,
  iconBtn: {
    marginLeft: 'auto',
    background: 'transparent',
    border: 'none',
    color: '#778',
    cursor: 'pointer',
    fontSize: 14,
    lineHeight: 1,
    padding: '0 2px',
  } as React.CSSProperties,
  label: {
    fontSize: 12,
    color: '#ccd',
    overflow: 'hidden',
    textOverflow: 'ellipsis',
    whiteSpace: 'nowrap',
    display: 'block',
    cursor: 'text',
  } as React.CSSProperties,
  sublabel: {
    fontSize: 9,
    color: '#445',
    letterSpacing: 0.3,
  } as React.CSSProperties,
  renameInput: {
    background: '#0e0e18',
    border: '1px solid #4a5aff',
    borderRadius: 3,
    color: '#ccd',
    fontSize: 11,
    padding: '2px 4px',
    width: '100%',
    outline: 'none',
  } as React.CSSProperties,
  select: {
    background: '#0e0e18',
    border: '1px solid #2a2a3a',
    borderRadius: 3,
    color: '#aab',
    fontSize: 10,
    padding: '2px 4px',
    outline: 'none',
    width: '100%',
  } as React.CSSProperties,
  addForm: {
    padding: '8px 8px',
    borderTop: '1px solid #2a2a3a',
    display: 'flex',
    flexDirection: 'column',
    gap: 4,
  } as React.CSSProperties,
  addBtn: {
    background: '#1e2a1e',
    border: '1px solid #3a5a3a',
    borderRadius: 3,
    color: '#8f8',
    fontSize: 11,
    cursor: 'pointer',
    padding: '3px 8px',
  } as React.CSSProperties,
}

function rowStyle(active: boolean, locked: boolean): React.CSSProperties {
  return {
    display: 'flex',
    alignItems: 'center',
    gap: 3,
    padding: '5px 8px',
    cursor: locked ? 'not-allowed' : 'pointer',
    background: active ? '#1e2038' : 'transparent',
    borderLeft: active ? '2px solid #5a7aff' : '2px solid transparent',
    opacity: locked ? 0.5 : 1,
    minHeight: 40,
  }
}
