import { useState, useMemo } from 'react'
import { useMapStore } from '../store/useMapStore'
import { CATEGORY_LABELS, ASSET_CATEGORIES } from '../../../shared/types'
import type { AssetCategory, AssetDef, HarvestNodeDef } from '../../../shared/types'

const sel: React.CSSProperties = {
  background: '#0e0e18', border: '1px solid #2a2a3a', borderRadius: 4,
  color: '#aab', fontSize: 11, padding: '4px 6px', outline: 'none', width: '100%',
  cursor: 'pointer',
}

export function AssetBrowser(): JSX.Element {
  const {
    assetDefs, assetImages, activeAssetId, activeCategory, activeGroup,
    harvestNodeDefs, activeHarvestNodeDefId,
    setActiveAsset, setActiveCategory, setActiveGroup,
    setActiveHarvestNode, manifest,
  } = useMapStore()

  const [search, setSearch] = useState('')

  // ── Derive groups for current category ──────────────────────────────────────

  const groups = useMemo(() => {
    if (activeCategory === 'harvest_nodes') {
      const skills = [...new Set(harvestNodeDefs.map(d => d.skill))].sort()
      return ['all', ...skills]
    }
    const cat = activeCategory as AssetCategory
    const g = [...new Set(
      assetDefs.filter(a => a.category === cat && a.group).map(a => a.group!)
    )].sort()
    return g.length > 0 ? ['all', ...g] : []
  }, [activeCategory, assetDefs, harvestNodeDefs])

  // ── Filtered assets / defs ───────────────────────────────────────────────────

  const filteredTiles = useMemo(() => {
    if (activeCategory === 'harvest_nodes') return []
    const cat = activeCategory as AssetCategory
    let list = assetDefs.filter(a => a.category === cat)
    if (activeGroup !== 'all') list = list.filter(a => a.group === activeGroup)
    if (search.trim()) {
      const q = search.toLowerCase()
      list = list.filter(a => a.name.toLowerCase().includes(q) || String(a.id).includes(q))
    }
    return list
  }, [assetDefs, activeCategory, activeGroup, search])

  const filteredNodes = useMemo(() => {
    if (activeCategory !== 'harvest_nodes') return []
    let list = harvestNodeDefs
    if (activeGroup !== 'all') list = list.filter(d => d.skill === activeGroup)
    if (search.trim()) {
      const q = search.toLowerCase()
      list = list.filter(d => d.name.toLowerCase().includes(q) || d.skill.toLowerCase().includes(q))
    }
    return list
  }, [harvestNodeDefs, activeCategory, activeGroup, search])

  if (!manifest) return (
    <div style={{ color: '#445', padding: 16, fontSize: 12, textAlign: 'center', marginTop: 24 }}>
      No map open
    </div>
  )

  const activeNodeDef = activeHarvestNodeDefId
    ? (harvestNodeDefs.find(d => d.node_def_id === activeHarvestNodeDefId) ?? null)
    : null

  const activeTileDef = activeCategory !== 'harvest_nodes'
    ? (assetDefs.find(a => a.id === activeAssetId) ?? null)
    : null

  return (
    <div style={{ display: 'flex', flexDirection: 'column', height: '100%', overflow: 'hidden' }}>

      {/* Category dropdown */}
      <div style={{ padding: '8px 8px 4px', borderBottom: '1px solid #1e1e2e', display: 'flex', flexDirection: 'column', gap: 4 }}>
        <select value={activeCategory} onChange={e => setActiveCategory(e.target.value as any)} style={sel}>
          <optgroup label="Tile Assets">
            {ASSET_CATEGORIES.map(cat => (
              <option key={cat} value={cat}>{CATEGORY_LABELS[cat]}</option>
            ))}
          </optgroup>
          <optgroup label="World Objects">
            <option value="harvest_nodes">Harvest Nodes</option>
          </optgroup>
        </select>

        {/* Sub-category / group dropdown */}
        {groups.length > 1 && (
          <select value={activeGroup} onChange={e => setActiveGroup(e.target.value)} style={sel}>
            {groups.map(g => (
              <option key={g} value={g}>
                {g === 'all' ? 'All groups' : g.charAt(0).toUpperCase() + g.slice(1)}
              </option>
            ))}
          </select>
        )}

        {/* Search */}
        <input
          value={search}
          onChange={e => setSearch(e.target.value)}
          placeholder="Search..."
          style={{ ...sel, cursor: 'text' }}
        />
      </div>

      {/* Asset / node list */}
      <div style={{ flex: 1, overflowY: 'auto', padding: '4px 6px', display: 'flex', flexDirection: 'column', gap: 2 }}>

        {activeCategory === 'harvest_nodes' ? (
          <>
            {filteredNodes.length === 0 && (
              <div style={{ color: '#334', fontSize: 11, textAlign: 'center', padding: 12 }}>
                {search ? 'No matches' : 'No harvest nodes defined'}
              </div>
            )}
            {filteredNodes.map(def => (
              <HarvestNodeCard
                key={def.node_def_id}
                def={def}
                isSelected={def.node_def_id === activeHarvestNodeDefId}
                onSelect={() => setActiveHarvestNode(def.node_def_id)}
              />
            ))}
          </>
        ) : (
          <>
            {filteredTiles.length === 0 && (
              <div style={{ color: '#334', fontSize: 11, textAlign: 'center', padding: 12 }}>
                {search ? 'No matches' : `Drop PNGs into game/assets/world/tiles/${activeCategory}/`}
              </div>
            )}
            {filteredTiles.map(asset => (
              <TileCard
                key={asset.id}
                asset={asset}
                isSelected={asset.id === activeAssetId}
                img={assetImages.get(asset.id) ?? null}
                onSelect={() => setActiveAsset(asset.id, asset.category as AssetCategory)}
              />
            ))}
          </>
        )}
      </div>

      {/* Footer: selected item info */}
      <div style={{ borderTop: '1px solid #1e1e2e', background: '#10101a', flexShrink: 0 }}>
        {activeCategory === 'harvest_nodes'
          ? <NodeFooter def={activeNodeDef} />
          : <TileFooter asset={activeTileDef} img={activeTileDef ? assetImages.get(activeTileDef.id) ?? null : null} />
        }
      </div>
    </div>
  )
}

// ── Tile card ─────────────────────────────────────────────────────────────────

function TileCard({ asset, isSelected, img, onSelect }: {
  asset: AssetDef; isSelected: boolean
  img: HTMLImageElement | null; onSelect: () => void
}): JSX.Element {
  return (
    <div onClick={onSelect} style={{
      display: 'flex', alignItems: 'center', gap: 8, padding: '4px 6px',
      borderRadius: 4, cursor: 'pointer',
      background: isSelected ? '#1e2040' : 'transparent',
      border: isSelected ? '1px solid #4060cc' : '1px solid transparent',
      flexShrink: 0,
    }}>
      <Swatch color={asset.color} img={img} size={22} />
      <div style={{ flex: 1, minWidth: 0 }}>
        <div style={{ fontSize: 11, color: '#ccd', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
          {asset.name}
        </div>
        <div style={{ fontSize: 9, color: '#445' }}>#{asset.id} · {asset.walkable ? '🚶' : '🚫'}</div>
      </div>
    </div>
  )
}

// ── Harvest node card ─────────────────────────────────────────────────────────

function HarvestNodeCard({ def, isSelected, onSelect }: {
  def: HarvestNodeDef; isSelected: boolean; onSelect: () => void
}): JSX.Element {
  return (
    <div onClick={onSelect} style={{
      display: 'flex', alignItems: 'center', gap: 8, padding: '5px 6px',
      borderRadius: 4, cursor: 'pointer',
      background: isSelected ? '#1a2a1a' : 'transparent',
      border: isSelected ? '1px solid #3a7a44' : '1px solid transparent',
      flexShrink: 0,
    }}>
      <Swatch color={def.color} img={null} size={22} />
      <div style={{ flex: 1, minWidth: 0 }}>
        <div style={{ fontSize: 11, color: '#ccd', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
          {def.name}
        </div>
        <div style={{ fontSize: 9, color: '#445' }}>
          {def.skill} · {def.blocks_movement ? '🚫 blocks' : '🚶 passable'}
        </div>
      </div>
    </div>
  )
}

// ── Shared swatch ─────────────────────────────────────────────────────────────

function Swatch({ color, img, size }: { color: string; img: HTMLImageElement | null; size: number }) {
  if (img) return (
    <img src={img.src} width={size} height={size}
      style={{ imageRendering: 'pixelated', borderRadius: 3, flexShrink: 0, border: '1px solid rgba(255,255,255,0.08)' }} />
  )
  return (
    <div style={{
      width: size, height: size, borderRadius: 3, flexShrink: 0,
      background: color, border: '1px solid rgba(255,255,255,0.12)',
    }} />
  )
}

// ── Footer variants ───────────────────────────────────────────────────────────

function TileFooter({ asset, img }: { asset: AssetDef | null; img: HTMLImageElement | null }) {
  if (!asset) return <div style={{ height: 40, padding: '6px 10px', fontSize: 10, color: '#334' }}>No tile selected</div>
  return (
    <div style={{ height: 40, padding: '4px 8px', display: 'flex', alignItems: 'center', gap: 8 }}>
      <Swatch color={asset.color} img={img} size={28} />
      <div>
        <div style={{ fontSize: 11, color: '#ccd', fontWeight: 600 }}>{asset.name}</div>
        <div style={{ fontSize: 9, color: '#556' }}>
          #{asset.id} · {CATEGORY_LABELS[asset.category]}{asset.group ? ` › ${asset.group}` : ''} · {asset.walkable ? 'walkable' : 'blocks'}
        </div>
      </div>
    </div>
  )
}

function NodeFooter({ def }: { def: HarvestNodeDef | null }) {
  if (!def) return <div style={{ height: 40, padding: '6px 10px', fontSize: 10, color: '#334' }}>Select a node to place</div>
  return (
    <div style={{ height: 40, padding: '4px 8px', display: 'flex', alignItems: 'center', gap: 8 }}>
      <Swatch color={def.color} img={null} size={28} />
      <div>
        <div style={{ fontSize: 11, color: '#ccd', fontWeight: 600 }}>{def.name}</div>
        <div style={{ fontSize: 9, color: '#556' }}>
          {def.node_def_id} · {def.skill} · click to place
        </div>
      </div>
    </div>
  )
}
