import type { MapManifest, AssetDef, WorldBounds } from '../../../shared/types'
import type { Camera } from '../store/useMapStore'

export interface RenderState {
  canvas: HTMLCanvasElement
  ctx: CanvasRenderingContext2D
  manifest: MapManifest
  camera: Camera
  chunks: Map<string, Uint16Array>
  layerStates: Record<string, { visible: boolean }>
  activeLayerId: string
  hoveredTile: { x: number; y: number } | null
  rectPreview: { x0: number; y0: number; x1: number; y1: number } | null
  harvestNodes: Array<{ tile: { x: number; y: number }; node_def_id: string; rotation_deg: number }>
  assetById: Map<number, AssetDef>
  assetImages: Map<number, HTMLImageElement>
  nodeDragTile: { x: number; y: number } | null
  nodeDragAngle: number | null
}

function chunkKey(cx: number, cy: number, layerId: string): string {
  return `${layerId}:${cx}:${cy}`
}

function hexToRgb(hex: string): [number, number, number] {
  const h = hex.replace('#', '')
  return [
    parseInt(h.slice(0, 2), 16),
    parseInt(h.slice(2, 4), 16),
    parseInt(h.slice(4, 6), 16),
  ]
}

export function render(state: RenderState): void {
  const {
    canvas, ctx, manifest, camera, chunks, layerStates, activeLayerId,
    hoveredTile, rectPreview, harvestNodes, assetById, assetImages,
    nodeDragTile, nodeDragAngle,
  } = state

  const { chunk_size: cs, cell_chunks: cc, layers } = manifest
  const w = canvas.width
  const h = canvas.height
  const tilePx = camera.zoom

  // Clear
  ctx.fillStyle = '#1a1a22'
  ctx.fillRect(0, 0, w, h)

  // Visible world tile range
  const worldLeft   = camera.x
  const worldTop    = camera.y
  const worldRight  = camera.x + w / tilePx
  const worldBottom = camera.y + h / tilePx

  const cxMin = Math.floor(worldLeft  / cs) - 1
  const cxMax = Math.ceil(worldRight  / cs) + 1
  const cyMin = Math.floor(worldTop   / cs) - 1
  const cyMax = Math.ceil(worldBottom / cs) + 1

  // ── LOD thresholds ───────────────────────────────────────────────────────
  //
  //  tilePx >= 3  →  full tile-by-tile render
  //  tilePx >= 0.3  →  chunk-color LOD  (1 rect per 32×32 chunk)
  //  tilePx <  0.3  →  cell-color LOD   (1 rect per 512×512 cell)

  const LOD_CHUNK = 2
  const LOD_CELL  = 0.3

  const cellTiles = (manifest.cell_chunks ?? 16) * cs  // tiles per cell side

  // Sorted layers for rendering
  const sorted = [...layers].sort((a, b) => a.z_order - b.z_order)

  if (tilePx >= LOD_CHUNK) {
    // ── Full tile detail ────────────────────────────────────────────────────
    for (const layer of sorted) {
      const ls = layerStates[layer.id]
      if (!ls?.visible) continue
      const isActive = layer.id === activeLayerId
      const alpha = isActive ? 1.0 : 0.45
      if (alpha < 1) ctx.globalAlpha = alpha

      for (let cy = cyMin; cy <= cyMax; cy++) {
        for (let cx = cxMin; cx <= cxMax; cx++) {
          const chunk = chunks.get(chunkKey(cx, cy, layer.id))
          if (!chunk) continue

          for (let ly = 0; ly < cs; ly++) {
            for (let lx = 0; lx < cs; lx++) {
              const assetId = chunk[ly * cs + lx]
              if (assetId === 0) continue

              const wx = cx * cs + lx
              const wy = cy * cs + ly
              const sx = Math.round((wx - camera.x) * tilePx)
              const sy = Math.round((wy - camera.y) * tilePx)
              const px = Math.ceil(tilePx)

              if (sx > w || sy > h || sx + px < 0 || sy + px < 0) continue

              const img = assetImages.get(assetId)
              if (img && img.complete && img.naturalWidth > 0) {
                ctx.drawImage(img, sx, sy, px, px)
              } else {
                const def = assetById.get(assetId)
                ctx.fillStyle = def ? def.color : '#ff00ff'
                ctx.fillRect(sx, sy, px, px)
              }
            }
          }
        }
      }
      ctx.globalAlpha = 1
    }

  } else if (tilePx >= LOD_CELL) {
    // ── Chunk-color LOD: 1 rect per chunk ──────────────────────────────────
    const chunkPx = cs * tilePx

    for (const layer of sorted) {
      const ls = layerStates[layer.id]
      if (!ls?.visible) continue
      const isActive = layer.id === activeLayerId
      const alpha = isActive ? 1.0 : 0.5
      if (alpha < 1) ctx.globalAlpha = alpha

      for (let cy = cyMin; cy <= cyMax; cy++) {
        for (let cx = cxMin; cx <= cxMax; cx++) {
          const chunk = chunks.get(chunkKey(cx, cy, layer.id))
          if (!chunk) continue

          const color = dominantChunkColor(chunk, cs, assetById)
          if (!color) continue

          const sx = Math.round((cx * cs - camera.x) * tilePx)
          const sy = Math.round((cy * cs - camera.y) * tilePx)
          const pw = Math.ceil(chunkPx)

          if (sx > w || sy > h || sx + pw < 0 || sy + pw < 0) continue

          ctx.fillStyle = color
          ctx.fillRect(sx, sy, pw, pw)
        }
      }
      ctx.globalAlpha = 1
    }

  } else {
    // ── Cell-color LOD: 1 rect per cell ────────────────────────────────────
    const cellPx = cellTiles * tilePx
    const cc = manifest.cell_chunks ?? 16

    const cellXMin = Math.floor(worldLeft  / cellTiles) - 1
    const cellXMax = Math.ceil(worldRight  / cellTiles) + 1
    const cellYMin = Math.floor(worldTop   / cellTiles) - 1
    const cellYMax = Math.ceil(worldBottom / cellTiles) + 1

    for (const layer of sorted) {
      const ls = layerStates[layer.id]
      if (!ls?.visible) continue

      for (let cellY = cellYMin; cellY <= cellYMax; cellY++) {
        for (let cellX = cellXMin; cellX <= cellXMax; cellX++) {
          // Sample 4 representative chunks from the cell corners
          const colors: string[] = []
          for (const [lcx, lcy] of [[0,0],[cc-1,0],[0,cc-1],[Math.floor(cc/2),Math.floor(cc/2)]]) {
            const absCx = cellX * cc + lcx
            const absCy = cellY * cc + lcy
            const chunk = chunks.get(chunkKey(absCx, absCy, layer.id))
            if (!chunk) continue
            const c = dominantChunkColor(chunk, cs, assetById)
            if (c) colors.push(c)
          }
          if (colors.length === 0) continue

          // Average the sampled colors
          const color = averageHexColors(colors)
          const sx = Math.round((cellX * cellTiles - camera.x) * tilePx)
          const sy = Math.round((cellY * cellTiles - camera.y) * tilePx)
          const pw = Math.ceil(cellPx)

          if (sx > w || sy > h || sx + pw < 0 || sy + pw < 0) continue

          ctx.fillStyle = color
          ctx.fillRect(sx, sy, pw, pw)
        }
      }
    }
  }

  // ── Grid overlays ────────────────────────────────────────────────────────

  // Tile grid (only when fully zoomed in)
  if (tilePx >= 8) {
    ctx.strokeStyle = 'rgba(255,255,255,0.05)'
    ctx.lineWidth = 0.5
    const txMin = Math.floor(worldLeft)
    const txMax = Math.ceil(worldRight)
    const tyMin = Math.floor(worldTop)
    const tyMax = Math.ceil(worldBottom)
    ctx.beginPath()
    for (let tx = txMin; tx <= txMax; tx++) {
      const sx = (tx - camera.x) * tilePx
      ctx.moveTo(sx, 0); ctx.lineTo(sx, h)
    }
    for (let ty = tyMin; ty <= tyMax; ty++) {
      const sy = (ty - camera.y) * tilePx
      ctx.moveTo(0, sy); ctx.lineTo(w, sy)
    }
    ctx.stroke()
  }

  // Chunk grid (skip at cell-level LOD — too dense to be useful)
  if (tilePx >= LOD_CELL) {
    ctx.strokeStyle = tilePx >= LOD_CHUNK ? 'rgba(80,140,255,0.15)' : 'rgba(80,140,255,0.25)'
    ctx.lineWidth = 1
    ctx.beginPath()
    for (let cx2 = cxMin; cx2 <= cxMax; cx2++) {
      const sx = (cx2 * cs - camera.x) * tilePx
      ctx.moveTo(sx, 0); ctx.lineTo(sx, h)
    }
    for (let cy2 = cyMin; cy2 <= cyMax; cy2++) {
      const sy = (cy2 * cs - camera.y) * tilePx
      ctx.moveTo(0, sy); ctx.lineTo(w, sy)
    }
    ctx.stroke()
  }

  // Cell grid (every cell_chunks * chunk_size tiles)
  {
    const cellPx = cellTiles * tilePx
    if (cellPx > 12) { // only draw when visible
      ctx.strokeStyle = 'rgba(255,200,80,0.25)'
      ctx.lineWidth = 1.5
      ctx.setLineDash([6, 4])
      const cellLeft  = Math.floor(worldLeft  / cellTiles)
      const cellRight = Math.ceil(worldRight  / cellTiles)
      const cellTop   = Math.floor(worldTop   / cellTiles)
      const cellBot   = Math.ceil(worldBottom / cellTiles)
      ctx.beginPath()
      for (let ccx = cellLeft; ccx <= cellRight; ccx++) {
        const sx = (ccx * cellTiles - camera.x) * tilePx
        ctx.moveTo(sx, 0); ctx.lineTo(sx, h)
      }
      for (let ccy = cellTop; ccy <= cellBot; ccy++) {
        const sy = (ccy * cellTiles - camera.y) * tilePx
        ctx.moveTo(0, sy); ctx.lineTo(w, sy)
      }
      ctx.stroke()
      ctx.setLineDash([])
    }
  }

  // ── Map boundary ─────────────────────────────────────────────────────────

  if (manifest.world_bounds) {
    drawMapBoundary(ctx, camera, manifest.world_bounds, w, h, tilePx)
  }

  // ── Harvest nodes ────────────────────────────────────────────────────────

  for (const node of harvestNodes) {
    const sx = (node.tile.x - camera.x) * tilePx
    const sy = (node.tile.y - camera.y) * tilePx
    if (sx < -tilePx * 2 || sy < -tilePx * 2 || sx > w + tilePx * 2 || sy > h + tilePx * 2) continue
    const cx2 = sx + tilePx / 2
    const cy2 = sy + tilePx / 2
    const r = Math.max(3, tilePx * 0.4)

    // Dot
    ctx.fillStyle = 'rgba(255,200,50,0.85)'
    ctx.beginPath()
    ctx.arc(cx2, cy2, r, 0, Math.PI * 2)
    ctx.fill()
    ctx.strokeStyle = '#fff'
    ctx.lineWidth = 1
    ctx.stroke()

    // Direction arrow (only visible when zoomed in enough)
    if (tilePx >= 6) {
      const rad = (node.rotation_deg - 90) * Math.PI / 180
      const arrowLen = Math.max(6, tilePx * 0.55)
      const ex = cx2 + Math.cos(rad) * arrowLen
      const ey = cy2 + Math.sin(rad) * arrowLen
      ctx.strokeStyle = 'rgba(255,255,255,0.9)'
      ctx.lineWidth = Math.max(1, tilePx * 0.08)
      ctx.beginPath()
      ctx.moveTo(cx2, cy2)
      ctx.lineTo(ex, ey)
      // Arrowhead
      const headLen = arrowLen * 0.35
      const headAngle = 0.5
      ctx.lineTo(ex - headLen * Math.cos(rad - headAngle), ey - headLen * Math.sin(rad - headAngle))
      ctx.moveTo(ex, ey)
      ctx.lineTo(ex - headLen * Math.cos(rad + headAngle), ey - headLen * Math.sin(rad + headAngle))
      ctx.stroke()
    }
  }

  // ── Harvest node drag preview ─────────────────────────────────────────────

  if (nodeDragTile) {
    const sx = (nodeDragTile.x - camera.x) * tilePx
    const sy = (nodeDragTile.y - camera.y) * tilePx
    const cx2 = sx + tilePx / 2
    const cy2 = sy + tilePx / 2
    const r = Math.max(4, tilePx * 0.45)

    // Ghost dot
    ctx.fillStyle = 'rgba(255,200,50,0.4)'
    ctx.strokeStyle = 'rgba(255,200,50,0.9)'
    ctx.lineWidth = 1.5
    ctx.beginPath()
    ctx.arc(cx2, cy2, r, 0, Math.PI * 2)
    ctx.fill()
    ctx.stroke()

    // Direction arrow during drag
    if (nodeDragAngle !== null && tilePx >= 4) {
      const rad = (nodeDragAngle - 90) * Math.PI / 180
      const arrowLen = Math.max(8, tilePx * 0.7)
      const ex = cx2 + Math.cos(rad) * arrowLen
      const ey = cy2 + Math.sin(rad) * arrowLen
      ctx.strokeStyle = 'rgba(255,220,80,1)'
      ctx.lineWidth = 2
      ctx.beginPath()
      ctx.moveTo(cx2, cy2)
      ctx.lineTo(ex, ey)
      const headLen = arrowLen * 0.35
      const headAngle = 0.5
      ctx.lineTo(ex - headLen * Math.cos(rad - headAngle), ey - headLen * Math.sin(rad - headAngle))
      ctx.moveTo(ex, ey)
      ctx.lineTo(ex - headLen * Math.cos(rad + headAngle), ey - headLen * Math.sin(rad + headAngle))
      ctx.stroke()

      // Angle label
      ctx.fillStyle = 'rgba(255,220,80,0.9)'
      ctx.font = `${Math.max(9, tilePx * 0.4)}px monospace`
      ctx.fillText(`${((nodeDragAngle % 360) + 360) % 360}°`, cx2 + r + 2, cy2 - r - 2)
    }
  }

  // ── Rect preview ─────────────────────────────────────────────────────────

  if (rectPreview) {
    const { x0, y0, x1, y1 } = rectPreview
    const rx = (Math.min(x0, x1) - camera.x) * tilePx
    const ry = (Math.min(y0, y1) - camera.y) * tilePx
    const rw = (Math.abs(x1 - x0) + 1) * tilePx
    const rh = (Math.abs(y1 - y0) + 1) * tilePx
    ctx.fillStyle = 'rgba(100,160,255,0.2)'
    ctx.fillRect(rx, ry, rw, rh)
    ctx.strokeStyle = 'rgba(100,160,255,0.9)'
    ctx.lineWidth = 1.5
    ctx.strokeRect(rx + 0.5, ry + 0.5, rw - 1, rh - 1)
  }

  // ── Hovered tile cursor ───────────────────────────────────────────────────

  if (hoveredTile) {
    const sx = (hoveredTile.x - camera.x) * tilePx
    const sy = (hoveredTile.y - camera.y) * tilePx
    ctx.strokeStyle = 'rgba(255,255,255,0.85)'
    ctx.lineWidth = 1.5
    ctx.strokeRect(Math.round(sx) + 0.5, Math.round(sy) + 0.5, Math.ceil(tilePx) - 1, Math.ceil(tilePx) - 1)
  }
}

// ── LOD helpers ───────────────────────────────────────────────────────────────

/**
 * Sample a 5×5 grid of tiles from a chunk and return the most common
 * non-empty asset color.  Fast enough to call every frame at low zoom.
 */
function dominantChunkColor(
  chunk: Uint16Array,
  cs: number,
  assetById: Map<number, AssetDef>,
): string | null {
  const step = Math.max(1, Math.floor(cs / 5))
  const counts = new Map<string, number>()

  for (let sy = 0; sy < cs; sy += step) {
    for (let sx = 0; sx < cs; sx += step) {
      const id = chunk[sy * cs + sx]
      if (id === 0) continue
      const def = assetById.get(id)
      if (!def) continue
      counts.set(def.color, (counts.get(def.color) ?? 0) + 1)
    }
  }

  if (counts.size === 0) return null

  let best = ''
  let bestN = 0
  for (const [color, n] of counts) {
    if (n > bestN) { best = color; bestN = n }
  }
  return best
}

/**
 * Average an array of hex colors by channel.
 */
function averageHexColors(colors: string[]): string {
  let r = 0, g = 0, b = 0
  for (const h of colors) {
    const [cr, cg, cb] = hexToRgb(h)
    r += cr; g += cg; b += cb
  }
  const n = colors.length
  return `rgb(${Math.round(r/n)},${Math.round(g/n)},${Math.round(b/n)})`
}

function drawMapBoundary(
  ctx: CanvasRenderingContext2D,
  camera: Camera,
  bounds: WorldBounds,
  canvasW: number,
  canvasH: number,
  tilePx: number,
): void {
  const bx = Math.round(-camera.x * tilePx)
  const by = Math.round(-camera.y * tilePx)
  const bw = Math.round(bounds.width  * tilePx)
  const bh = Math.round(bounds.height * tilePx)

  // Shade outside the map bounds — 4 rects around the boundary
  ctx.fillStyle = 'rgba(0,0,0,0.45)'
  // Top
  if (by > 0)               ctx.fillRect(0,       0,       canvasW, by)
  // Bottom
  if (by + bh < canvasH)    ctx.fillRect(0,       by + bh, canvasW, canvasH - (by + bh))
  // Left (between top/bottom)
  if (bx > 0)               ctx.fillRect(0,       by,      bx,      bh)
  // Right
  if (bx + bw < canvasW)    ctx.fillRect(bx + bw, by,      canvasW - (bx + bw), bh)

  // Boundary border
  ctx.strokeStyle = 'rgba(255, 160, 40, 0.9)'
  ctx.lineWidth = 2
  ctx.setLineDash([])
  ctx.strokeRect(bx + 1, by + 1, bw - 2, bh - 2)

  // Corner markers — small orange squares at each corner
  const CORNER = Math.max(4, Math.min(12, tilePx * 2))
  ctx.fillStyle = 'rgba(255,160,40,0.9)'
  const corners: Array<[number, number]> = [
    [bx, by], [bx + bw - CORNER, by],
    [bx, by + bh - CORNER], [bx + bw - CORNER, by + bh - CORNER],
  ]
  for (const [cx2, cy2] of corners) ctx.fillRect(cx2, cy2, CORNER, CORNER)
}

export function screenToTile(
  screenX: number,
  screenY: number,
  camera: Camera,
): { x: number; y: number } {
  return {
    x: Math.floor(screenX / camera.zoom + camera.x),
    y: Math.floor(screenY / camera.zoom + camera.y),
  }
}
