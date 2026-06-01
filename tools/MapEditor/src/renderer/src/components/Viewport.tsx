import { useEffect, useRef, useCallback } from 'react'
import { useMapStore } from '../store/useMapStore'
import { render, screenToTile } from '../canvas/tileRenderer'
import { Preview3D } from './Preview3D'
import type { AssetDef } from '../../../shared/types'

function FillProgress(): JSX.Element | null {
  const fillInProgress = useMapStore(s => s.fillInProgress)
  const fillTileCount  = useMapStore(s => s.fillTileCount)
  const cancelFill     = useMapStore(s => s.cancelFill)

  if (!fillInProgress) return null

  return (
    <div style={{
      position: 'absolute', bottom: 10, left: '50%', transform: 'translateX(-50%)',
      background: 'rgba(10,10,24,0.92)', border: '1px solid #3a5a8a',
      borderRadius: 6, padding: '6px 16px',
      display: 'flex', alignItems: 'center', gap: 12,
      fontSize: 12, color: '#9ab', zIndex: 10, pointerEvents: 'all',
    }}>
      <span>🪣 Filling… {fillTileCount.toLocaleString()} tiles</span>
      <button
        onClick={cancelFill}
        style={{
          background: '#3a1a1a', border: '1px solid #8a3a3a',
          borderRadius: 4, color: '#f88', fontSize: 11,
          padding: '2px 8px', cursor: 'pointer',
        }}
      >
        Cancel
      </button>
    </div>
  )
}

export function Viewport(): JSX.Element {
  const canvasRef    = useRef<HTMLCanvasElement>(null)
  const isPainting   = useRef(false)
  const isPanning    = useRef(false)
  const lastMouse    = useRef({ x: 0, y: 0 })
  const hoveredTile  = useRef<{ x: number; y: number } | null>(null)
  const rectStart    = useRef<{ x: number; y: number } | null>(null)
  const rectCurrent  = useRef<{ x: number; y: number } | null>(null)
  // Harvest node direction drag
  const nodePlace    = useRef<{ screenX: number; screenY: number; tile: { x: number; y: number } } | null>(null)
  const nodeDragAngle = useRef<number | null>(null)  // current drag angle in degrees
  const lastRotation = useRef(0)  // persists between placements

  const store = useMapStore()

  // Pre-compute asset lookup maps for the renderer
  const assetById = useRef(new Map<number, AssetDef>())
  useEffect(() => {
    assetById.current = new Map(store.assetDefs.map(a => [a.id, a]))
  }, [store.assetDefs])

  const draw = useCallback(() => {
    const canvas = canvasRef.current
    if (!canvas || !store.manifest) return
    const ctx = canvas.getContext('2d')!

    const layerStates: Record<string, { visible: boolean }> = {}
    for (const [id, ls] of Object.entries(store.layerStates)) {
      layerStates[id] = { visible: ls.visible }
    }

    const rectPreview = (store.activeTool === 'rect' && rectStart.current && rectCurrent.current)
      ? { x0: rectStart.current.x, y0: rectStart.current.y, x1: rectCurrent.current.x, y1: rectCurrent.current.y }
      : null

    render({
      canvas, ctx,
      manifest: store.manifest,
      camera: store.camera,
      chunks: store.chunks,
      layerStates,
      activeLayerId: store.activeLayerId,
      hoveredTile: hoveredTile.current,
      rectPreview,
      harvestNodes: store.harvestNodes,
      assetById: assetById.current,
      assetImages: store.assetImages,
      nodeDragTile: nodePlace.current?.tile ?? null,
      nodeDragAngle: nodeDragAngle.current,
    })
  }, [store])

  useEffect(() => { draw() }, [draw])

  useEffect(() => {
    const canvas = canvasRef.current
    if (!canvas) return
    const ro = new ResizeObserver(() => {
      canvas.width  = canvas.offsetWidth
      canvas.height = canvas.offsetHeight
      store.setViewSize(canvas.offsetWidth, canvas.offsetHeight)
      draw()
    })
    ro.observe(canvas)
    return () => ro.disconnect()
  }, [draw, store])

  // ── Brush helpers ────────────────────────────────────────────────────────

  function tilesForBrush(tile: { x: number; y: number }): Array<{ x: number; y: number }> {
    const { brushSize } = store
    const tiles: Array<{ x: number; y: number }> = []
    const half = Math.floor(brushSize / 2)
    for (let dy = -half; dy <= half; dy++)
      for (let dx = -half; dx <= half; dx++)
        tiles.push({ x: tile.x + dx, y: tile.y + dy })
    return tiles
  }

  function tilesForRect(start: { x: number; y: number }, end: { x: number; y: number }): Array<{ x: number; y: number }> {
    const tiles: Array<{ x: number; y: number }> = []
    const x0 = Math.min(start.x, end.x)
    const x1 = Math.max(start.x, end.x)
    const y0 = Math.min(start.y, end.y)
    const y1 = Math.max(start.y, end.y)
    for (let y = y0; y <= y1; y++)
      for (let x = x0; x <= x1; x++)
        tiles.push({ x, y })
    return tiles
  }

  // ── Auto-save dirty chunks ────────────────────────────────────────────────

  function flushDirty(): void {
    const { mapPath, getDirtyChunks, clearDirty } = store
    if (!mapPath) return
    const dirty = getDirtyChunks()
    if (dirty.length === 0) return
    Promise.all(dirty.map(({ layerId, cx, cy, data }) =>
      window.api.saveChunk(mapPath, layerId, cx, cy, data)
    )).then(() => clearDirty())
  }

  // ── Mouse handlers ────────────────────────────────────────────────────────

  const handleMouseDown = useCallback((e: React.MouseEvent) => {
    if (e.button === 1 || (e.button === 0 && e.altKey)) {
      isPanning.current = true
    } else if (e.button === 0) {
      isPainting.current = true
      const tile = screenToTile(e.nativeEvent.offsetX, e.nativeEvent.offsetY, store.camera)

      if (store.activeTool === 'pencil') {
        store.paintTiles(tilesForBrush(tile))
      } else if (store.activeTool === 'erase') {
        if (store.activeCategory === 'harvest_nodes') {
          store.eraseHarvestNodesAt(tilesForBrush(tile))
        } else {
          store.eraseTiles(tilesForBrush(tile))
        }
      } else if (store.activeTool === 'fill') {
        store.startFill(tile.x, tile.y)
      } else if (store.activeTool === 'rect') {
        rectStart.current = tile
        rectCurrent.current = tile
      } else if (store.activeTool === 'harvest_node') {
        if (store.activeHarvestNodeDefId) {
          nodePlace.current = { screenX: e.nativeEvent.offsetX, screenY: e.nativeEvent.offsetY, tile }
          nodeDragAngle.current = null
        }
      }
    }
    lastMouse.current = { x: e.clientX, y: e.clientY }
  }, [store])

  const handleMouseMove = useCallback((e: React.MouseEvent) => {
    const dx = e.clientX - lastMouse.current.x
    const dy = e.clientY - lastMouse.current.y
    lastMouse.current = { x: e.clientX, y: e.clientY }

    if (isPanning.current) {
      store.setCamera({
        x: store.camera.x - dx / store.camera.zoom,
        y: store.camera.y - dy / store.camera.zoom,
      })
    } else if (isPainting.current) {
      const tile = screenToTile(e.nativeEvent.offsetX, e.nativeEvent.offsetY, store.camera)
      if (store.activeTool === 'pencil') store.paintTiles(tilesForBrush(tile))
      if (store.activeTool === 'erase') {
        if (store.activeCategory === 'harvest_nodes') store.eraseHarvestNodesAt(tilesForBrush(tile))
        else store.eraseTiles(tilesForBrush(tile))
      }
      if (store.activeTool === 'rect')   rectCurrent.current = tile
    }

    hoveredTile.current = screenToTile(e.nativeEvent.offsetX, e.nativeEvent.offsetY, store.camera)

    // Update direction arrow while dragging a harvest node placement
    if (nodePlace.current) {
      const dx = e.nativeEvent.offsetX - nodePlace.current.screenX
      const dy = e.nativeEvent.offsetY - nodePlace.current.screenY
      if (Math.sqrt(dx * dx + dy * dy) > 8) {
        // atan2 gives angle from +X axis; convert to compass degrees (0=North=up)
        const rawDeg = Math.atan2(dy, dx) * 180 / Math.PI + 90
        // Snap to nearest 45°
        nodeDragAngle.current = Math.round(rawDeg / 45) * 45
      }
    }

    draw()
  }, [store, draw])

  const handleMouseUp = useCallback(() => {
    // Finalise rect
    if (isPainting.current && store.activeTool === 'rect' && rectStart.current && rectCurrent.current) {
      const tiles = tilesForRect(rectStart.current, rectCurrent.current)
      store.paintTiles(tiles)
      rectStart.current = null
      rectCurrent.current = null
    }

    // Finalise harvest node placement
    if (nodePlace.current && store.activeHarvestNodeDefId) {
      const defId = store.activeHarvestNodeDefId
      const { tile } = nodePlace.current
      const rotation_deg = nodeDragAngle.current ?? lastRotation.current
      lastRotation.current = rotation_deg
      const nodeId = `${defId}_${tile.x}_${tile.y}`
      store.removeHarvestNode(nodeId)
      store.addHarvestNode({
        node_id: nodeId,
        node_def_id: defId,
        tile,
        blocks_movement: store.harvestNodeDefs.find(d => d.node_def_id === defId)?.blocks_movement ?? true,
        rotation_deg,
      })
      nodePlace.current = null
      nodeDragAngle.current = null
    }

    isPainting.current = false
    isPanning.current  = false
    flushDirty()
  }, [store])

  // Wheel must be attached directly (passive: false) — React 18 registers onWheel
  // as passive, making e.preventDefault() a no-op and letting Electron zoom the window.
  useEffect(() => {
    const canvas = canvasRef.current
    if (!canvas) return
    const handler = (e: WheelEvent) => {
      e.preventDefault()
      const s = useMapStore.getState()
      const factor = e.deltaY < 0 ? 1.15 : 1 / 1.15
      const newZoom = Math.max(1, Math.min(128, s.camera.zoom * factor))
      const rect = canvas.getBoundingClientRect()
      const mx = e.clientX - rect.left
      const my = e.clientY - rect.top
      const worldX = mx / s.camera.zoom + s.camera.x
      const worldY = my / s.camera.zoom + s.camera.y
      s.setCamera({ zoom: newZoom, x: worldX - mx / newZoom, y: worldY - my / newZoom })
    }
    canvas.addEventListener('wheel', handler, { passive: false })
    return () => canvas.removeEventListener('wheel', handler)
  }, [])

  const handleMouseLeave = useCallback(() => {
    isPainting.current = false
    isPanning.current  = false
    hoveredTile.current = null
    nodePlace.current = null
    nodeDragAngle.current = null
    draw()
  }, [draw])

  return (
    <div style={{ width: '100%', height: '100%', position: 'relative' }}>
      <canvas
        ref={canvasRef}
        style={{ width: '100%', height: '100%', display: 'block', cursor: 'crosshair' }}
        onMouseDown={handleMouseDown}
        onMouseMove={handleMouseMove}
        onMouseUp={handleMouseUp}
        onMouseLeave={handleMouseLeave}
      />
      <FillProgress />
      <Preview3D />
    </div>
  )
}
