/**
 * Preview3D — shows the last-painted chunk in 3D.
 * Updates whenever you paint or erase tiles.
 * All visible layers are rendered at that chunk position.
 */

import { useEffect, useRef, useState, useCallback } from 'react'
import * as THREE from 'three'
import { GLTFLoader } from 'three/examples/jsm/loaders/GLTFLoader.js'
import { useMapStore } from '../store/useMapStore'
import type { AssetDef } from '../../../shared/types'

const SKILL_COLORS: Record<string, string> = {
  woodcutting: '#2e7d32',
  fishing:     '#1565c0',
  mining:      '#546e7a',
  farming:     '#f57f17',
}

// GLB scene cache — keyed by absolute path, value is a cloneable THREE.Group
const glbCache = new Map<string, THREE.Group>()
const glbLoading = new Map<string, Promise<THREE.Group | null>>()

async function loadGlb(absolutePath: string): Promise<THREE.Group | null> {
  if (glbCache.has(absolutePath)) return glbCache.get(absolutePath)!
  if (glbLoading.has(absolutePath)) return glbLoading.get(absolutePath)!

  const promise = (async () => {
    try {
      const dataUrl = await window.api.getGlbDataUrl(absolutePath)
      if (!dataUrl) return null

      // Decode base64 → ArrayBuffer and use parse() — load() doesn't handle GLB data URLs
      const base64 = dataUrl.split(',')[1]
      const binary = atob(base64)
      const bytes = new Uint8Array(binary.length)
      for (let i = 0; i < binary.length; i++) bytes[i] = binary.charCodeAt(i)

      return await new Promise<THREE.Group | null>((resolve) => {
        new GLTFLoader().parse(
          bytes.buffer,
          '',
          gltf => { glbCache.set(absolutePath, gltf.scene); resolve(gltf.scene) },
          () => resolve(null),
        )
      })
    } catch {
      return null
    }
  })()

  glbLoading.set(absolutePath, promise)
  const result = await promise
  glbLoading.delete(absolutePath)
  return result
}

// ── Constants ─────────────────────────────────────────────────────────────────

const T = 64  // world units per tile — matches Stonepyre engine TILE_SIZE

// Layer → 3D geometry config
interface LayerCfg { type: 'plane' | 'box'; baseY: number; height: number }
const LAYER_CFG: Record<string, LayerCfg> = {
  ground:         { type: 'plane', baseY: 0,   height: 0  },
  ground_detail:  { type: 'plane', baseY: 1,   height: 0  },
  floor:          { type: 'plane', baseY: 2,   height: 0  },
  curb:           { type: 'box',   baseY: 0,   height: 8  },
  vegetation_low: { type: 'box',   baseY: 0,   height: 32 },
  vegetation_high:{ type: 'box',   baseY: 0,   height: 80 },
  props:          { type: 'box',   baseY: 0,   height: 24 },
  structure_low:  { type: 'box',   baseY: 0,   height: 64 },
  structure_high: { type: 'box',   baseY: 64,  height: 16 },
  overlay:        { type: 'plane', baseY: 3,   height: 0  },
}
function getCfg(id: string): LayerCfg {
  return LAYER_CFG[id] ?? { type: 'plane', baseY: 0, height: 0 }
}

// Material cache — keyed by asset ID (texture) or hex color (fallback)
const matCache = new Map<string, THREE.MeshLambertMaterial>()

function getMatForAsset(assetId: number, color: string, assetImages: Map<number, HTMLImageElement>): THREE.MeshLambertMaterial {
  const key = `id:${assetId}`
  let m = matCache.get(key)
  if (m) return m

  const img = assetImages.get(assetId)
  if (img && img.complete && img.naturalWidth > 0) {
    const tex = new THREE.Texture(img)
    tex.needsUpdate = true
    tex.colorSpace = THREE.SRGBColorSpace
    m = new THREE.MeshLambertMaterial({ map: tex })
  } else {
    m = new THREE.MeshLambertMaterial({ color: new THREE.Color(color) })
  }
  matCache.set(key, m)
  return m
}

function getMat(hex: string): THREE.MeshLambertMaterial {
  let m = matCache.get(hex)
  if (!m) { m = new THREE.MeshLambertMaterial({ color: new THREE.Color(hex) }); matCache.set(hex, m) }
  return m
}

// HARVEST_NODE_SCALE from stonepyre_app: models exported in metres, 53.3 wu/m
const HARVEST_NODE_SCALE = 53.3

// Place a GLB group using the engine's fixed scale, sitting on y=0
function fitAndPlaceGlb(group: THREE.Group, wx: number, wz: number): void {
  try {
    group.scale.setScalar(HARVEST_NODE_SCALE)
    const box = new THREE.Box3().setFromObject(group)
    if (box.isEmpty()) { group.position.set(wx, 0, wz); return }
    group.position.set(wx, -box.min.y, wz)
  } catch {
    group.position.set(wx, 0, wz)
  }
}

// ── Panel dimensions ──────────────────────────────────────────────────────────

const W = 580
const H = 420

// ── Component ─────────────────────────────────────────────────────────────────

export function Preview3D(): JSX.Element | null {
  const show = useMapStore(s => s.showPreview3d)
  if (!show) return null
  return <PreviewPanel />
}

function PreviewPanel(): JSX.Element {
  const canvasRef   = useRef<HTMLCanvasElement>(null)
  const rendererRef = useRef<THREE.WebGLRenderer | null>(null)
  const sceneRef    = useRef<THREE.Scene | null>(null)
  const camRef      = useRef<THREE.PerspectiveCamera | null>(null)
  const meshesRef   = useRef<THREE.Object3D[]>([])

  // Camera state: isometric angle (theta/phi), pan offset, zoom
  const cam3d    = useRef({ panX: 0, panZ: 0, zoom: 1.0, theta: Math.PI / 4, phi: Math.PI / 3 })
  const focusSizeRef = useRef(T * 8)  // updated each rebuild to match current camera framing
  const isPan    = useRef(false)
  const isOrbit  = useRef(false)
  const panStart  = useRef({ mx: 0, my: 0, panX: 0, panZ: 0 })
  const orbitStart = useRef({ mx: 0, my: 0, theta: 0, phi: 0 })

  // Panel position
  const [pos, setPos] = useState({ right: 8, top: 8 })
  const isDrag  = useRef(false)
  const dragOff = useRef({ mx: 0, my: 0, right: 0, top: 0 })

  const lastTouched = useMapStore(s => s.lastTouchedChunk)
  const store = useMapStore()

  // ── Three.js init ─────────────────────────────────────────────────────────

  useEffect(() => {
    const canvas = canvasRef.current
    if (!canvas) return
    const renderer = new THREE.WebGLRenderer({ canvas, antialias: true })
    renderer.setSize(W, H)
    renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2))
    rendererRef.current = renderer

    const scene = new THREE.Scene()
    scene.background = new THREE.Color(0x141420)
    scene.fog = new THREE.FogExp2(0x141420, 0.000025)
    sceneRef.current = scene

    scene.add(new THREE.AmbientLight(0xffffff, 0.6))
    const sun = new THREE.DirectionalLight(0xfff5d0, 0.85)
    sun.position.set(1, 2, 0.5)
    scene.add(sun)

    // Reference ground plane
    const gp = new THREE.Mesh(
      new THREE.PlaneGeometry(1, 1),
      new THREE.MeshLambertMaterial({ color: 0x1a1a2a }),
    )
    gp.rotation.x = -Math.PI / 2
    gp.position.y = -0.5
    gp.name = '__ground__'
    scene.add(gp)

    const cam = new THREE.PerspectiveCamera(50, W / H, 1, 200_000)
    camRef.current = cam

    return () => { renderer.dispose() }
  }, [])

  // ── Rebuild scene from last touched chunk ────────────────────────────────

  const rebuild = useCallback(async () => { try {
    const scene    = sceneRef.current
    const cam      = camRef.current
    const renderer = rendererRef.current
    if (!scene || !cam || !renderer) return

    const { manifest, chunks, assetDefs, assetImages, layerStates, lastTouchedChunk, harvestNodes, harvestNodeDefs } = useMapStore.getState()

    // Remove old tile meshes
    for (const m of meshesRef.current) scene.remove(m)
    meshesRef.current = []

    if (!manifest || !lastTouchedChunk) {
      // No chunk painted yet — just show the reference ground and render
      const gp = scene.getObjectByName('__ground__')
      const chunkSz = (manifest?.chunk_size ?? 32) * T
      if (gp) gp.scale.set(chunkSz, 1, chunkSz)
      positionCamera(cam, 0, 0, chunkSz)
      renderer.render(scene, cam)
      return
    }

    const { cx, cy } = lastTouchedChunk
    const cs = manifest.chunk_size
    const chunkWorldSz = cs * T
    const centerX = (cx * cs + cs / 2) * T
    const centerZ = -((cy * cs + cs / 2) * T)

    // Resize reference ground to chunk footprint
    const gp = scene.getObjectByName('__ground__')
    if (gp) {
      gp.scale.set(chunkWorldSz, 1, chunkWorldSz)
      gp.position.set(centerX, -0.5, centerZ)
    }

    const assetById = new Map<number, AssetDef>(assetDefs.map(a => [a.id, a]))
    const sorted = [...manifest.layers].sort((a, b) => a.z_order - b.z_order)

    // Group tile instances by (layerId + assetId) for InstancedMesh per texture
    type Group = { cfg: LayerCfg; assetId: number; color: string; indices: number[] }
    const groups = new Map<string, Group>()

    for (const layer of sorted) {
      if (!layerStates[layer.id]?.visible) continue
      const cfg = getCfg(layer.id)
      const chunk = chunks.get(`${layer.id}:${cx}:${cy}`)
      if (!chunk) continue

      for (let ly = 0; ly < cs; ly++) {
        for (let lx = 0; lx < cs; lx++) {
          const id = chunk[ly * cs + lx]
          if (id === 0) continue
          const def = assetById.get(id)
          if (!def) continue
          const gk = `${layer.id}|${id}`
          let g = groups.get(gk)
          if (!g) { g = { cfg, assetId: id, color: def.color, indices: [] }; groups.set(gk, g) }
          g.indices.push(ly * cs + lx)
        }
      }
    }

    // Build InstancedMesh per group — full T size for seamless tiling
    const dummy = new THREE.Object3D()
    for (const { cfg, assetId, color, indices } of groups.values()) {
      const yPos = cfg.type === 'box' ? cfg.baseY + cfg.height / 2 : cfg.baseY
      const geo  = cfg.type === 'box'
        ? new THREE.BoxGeometry(T, cfg.height, T)
        : (() => { const g = new THREE.PlaneGeometry(T, T); g.rotateX(-Math.PI / 2); return g })()

      const mat = getMatForAsset(assetId, color, assetImages)
      const mesh = new THREE.InstancedMesh(geo, mat, indices.length)
      indices.forEach((idx, i) => {
        const lx = idx % cs
        const ly = Math.floor(idx / cs)
        dummy.position.set(
          (cx * cs + lx + 0.5) * T,
          yPos,
          -((cy * cs + ly + 0.5) * T),
        )
        dummy.updateMatrix()
        mesh.setMatrixAt(i, dummy.matrix)
      })
      mesh.instanceMatrix.needsUpdate = true
      scene.add(mesh)
      meshesRef.current.push(mesh)
    }

    // ── Harvest nodes in this chunk ──────────────────────────────────────────
    const nodeDefMap = new Map(harvestNodeDefs.map(d => [d.node_def_id, d]))
    const nodesInChunk = harvestNodes.filter(n => {
      const ncx = Math.floor(n.tile.x / cs)
      const ncy = Math.floor(n.tile.y / cs)
      return ncx === cx && ncy === cy
    })

    // Render pass: place each node — GLB if cached, colored box if not yet loaded
    const glbPromises: Promise<void>[] = []

    for (const node of nodesInChunk) {
      const def = nodeDefMap.get(node.node_def_id)
      const wx = (node.tile.x + 0.5) * T
      const wz = -((node.tile.y + 0.5) * T)

      const glbPath = def?.available_model ?? null

      const rotRad = ((node as any).rotation_deg ?? 0) * Math.PI / 180

      if (glbPath && glbCache.has(glbPath)) {
        // Already cached — clone and place immediately
        const clone = glbCache.get(glbPath)!.clone()
        fitAndPlaceGlb(clone, wx, wz)
        clone.rotation.y = rotRad
        scene.add(clone)
        meshesRef.current.push(clone)
      } else {
        // Placeholder box while GLB loads (or if no GLB)
        const color = def ? (SKILL_COLORS[def.skill] ?? def.color) : '#9e9e9e'
        const geo = new THREE.BoxGeometry(T * 0.6, T * 1.2, T * 0.6)
        const mesh = new THREE.Mesh(geo, getMat(color))
        mesh.position.set(wx, T * 0.6, wz)
        scene.add(mesh)
        meshesRef.current.push(mesh)

        if (glbPath) {
          glbPromises.push(
            loadGlb(glbPath).then(glbScene => {
              if (!glbScene || !sceneRef.current || !rendererRef.current || !camRef.current) return
              // Swap placeholder for real GLB
              scene.remove(mesh)
              const clone = glbScene.clone()
              fitAndPlaceGlb(clone, wx, wz)
              clone.rotation.y = rotRad
              scene.add(clone)
              meshesRef.current.push(clone)
              rendererRef.current.render(scene, camRef.current)
            })
          )
        }
      }
    }

    // If nodes are present, fit camera tightly around them with 2-tile padding.
    // Otherwise show the full chunk.
    let focusX = centerX
    let focusZ = centerZ
    let focusSize = chunkWorldSz
    if (nodesInChunk.length > 0) {
      let minTX = Infinity, maxTX = -Infinity, minTY = Infinity, maxTY = -Infinity
      for (const n of nodesInChunk) {
        minTX = Math.min(minTX, n.tile.x)
        maxTX = Math.max(maxTX, n.tile.x)
        minTY = Math.min(minTY, n.tile.y)
        maxTY = Math.max(maxTY, n.tile.y)
      }
      const padTiles = 2
      const spanX = (maxTX - minTX + 1 + padTiles * 2) * T
      const spanZ = (maxTY - minTY + 1 + padTiles * 2) * T
      focusSize = Math.max(spanX, spanZ, T * 3)  // minimum 3-tile view
      focusX = ((minTX + maxTX) / 2 + 0.5) * T
      focusZ = -(((minTY + maxTY) / 2 + 0.5) * T)
    }

    focusSizeRef.current = focusSize
    positionCamera(cam, focusX, focusZ, focusSize)
    renderer.render(scene, cam)

    // Re-render once all GLBs have loaded in (if any were async)
    if (glbPromises.length > 0) {
      Promise.all(glbPromises).then(() => {
        if (rendererRef.current && sceneRef.current && camRef.current) {
          positionCamera(camRef.current, focusX, focusZ, focusSize)
          rendererRef.current.render(sceneRef.current, camRef.current)
        }
      })
    }
  } catch (err) {
    console.error('[Preview3D] rebuild error:', err)
  }}, [])

  function positionCamera(cam: THREE.PerspectiveCamera, cx: number, cz: number, size: number) {
    const { panX, panZ, zoom, theta, phi } = cam3d.current
    const halfFov = (50 / 2) * Math.PI / 180
    const baseR   = (size / 2) / Math.tan(halfFov)
    const r = (baseR * zoom) / Math.sin(phi)
    const targetX = cx + panX
    const targetZ = cz + panZ
    cam.position.set(
      targetX + r * Math.sin(phi) * Math.sin(theta),
      r * Math.cos(phi),
      targetZ + r * Math.sin(phi) * Math.cos(theta),
    )
    cam.lookAt(targetX, 0, targetZ)
    cam.updateProjectionMatrix()
  }

  // Rebuild when last touched chunk or chunk data changes
  // Clear texture cache when asset images change so new textures are picked up
  useEffect(() => {
    for (const [key] of matCache) {
      if (key.startsWith('id:')) matCache.delete(key)
    }
  }, [store.assetImages])

  useEffect(() => {
    cam3d.current.panX = 0
    cam3d.current.panZ = 0
  }, [lastTouched])

  useEffect(() => { rebuild() },
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [lastTouched, store.chunks, store.layerStates, store.manifest, store.harvestNodes, store.assetImages, rebuild])

  // ── Pan (left drag) · Orbit (middle drag) · Zoom (scroll) ───────────────

  const onCanvasDown = useCallback((e: React.MouseEvent) => {
    if (e.button === 0) {
      isPan.current = true
      panStart.current = { mx: e.clientX, my: e.clientY, panX: cam3d.current.panX, panZ: cam3d.current.panZ }
    } else if (e.button === 1) {
      isOrbit.current = true
      orbitStart.current = { mx: e.clientX, my: e.clientY, theta: cam3d.current.theta, phi: cam3d.current.phi }
    }
    e.preventDefault(); e.stopPropagation()
  }, [])

  useEffect(() => {
    const onMove = (e: MouseEvent) => {
      if (isPan.current) {
        // world units visible ≈ focusSize * zoom; 1 pixel = that / canvas pixels
        const worldVisible = focusSizeRef.current * cam3d.current.zoom
        const scaleX = worldVisible / W
        const scaleZ = worldVisible / H
        cam3d.current.panX = panStart.current.panX - (e.clientX - panStart.current.mx) * scaleX
        cam3d.current.panZ = panStart.current.panZ + (e.clientY - panStart.current.my) * scaleZ
        rebuild()
      } else if (isOrbit.current) {
        cam3d.current.theta = orbitStart.current.theta - (e.clientX - orbitStart.current.mx) * 0.008
        cam3d.current.phi   = Math.max(0.1, Math.min(Math.PI / 2 - 0.05,
          orbitStart.current.phi + (e.clientY - orbitStart.current.my) * 0.008))
        rebuild()
      }
    }
    const onUp = () => { isPan.current = false; isOrbit.current = false }
    const onWheel = (e: WheelEvent) => {
      if (!(e.target as HTMLElement)?.closest?.('.p3d-canvas')) return
      cam3d.current.zoom = Math.max(0.2, Math.min(8, cam3d.current.zoom * (e.deltaY > 0 ? 1.12 : 0.89)))
      rebuild()
      e.preventDefault()
    }
    window.addEventListener('mousemove', onMove)
    window.addEventListener('mouseup',   onUp)
    window.addEventListener('wheel',     onWheel, { passive: false })
    return () => {
      window.removeEventListener('mousemove', onMove)
      window.removeEventListener('mouseup',   onUp)
      window.removeEventListener('wheel',     onWheel)
    }
  }, [rebuild])

  // ── Panel drag ────────────────────────────────────────────────────────────

  const onHeaderDown = useCallback((e: React.MouseEvent) => {
    isDrag.current = true
    dragOff.current = { mx: e.clientX, my: e.clientY, right: pos.right, top: pos.top }
    e.preventDefault()
  }, [pos])

  useEffect(() => {
    const onMove = (e: MouseEvent) => {
      if (!isDrag.current) return
      setPos({
        right: dragOff.current.right - (e.clientX - dragOff.current.mx),
        top:   dragOff.current.top   + (e.clientY - dragOff.current.my),
      })
    }
    const onUp = () => { isDrag.current = false }
    window.addEventListener('mousemove', onMove)
    window.addEventListener('mouseup',   onUp)
    return () => { window.removeEventListener('mousemove', onMove); window.removeEventListener('mouseup', onUp) }
  }, [])

  const chunkLabel = lastTouched
    ? `chunk ${lastTouched.cx},${lastTouched.cy}`
    : 'paint a tile to preview'

  return (
    <div style={{
      position: 'absolute', right: pos.right, top: pos.top,
      width: W, zIndex: 20, borderRadius: 8, overflow: 'hidden',
      border: '1px solid #2a3a5a', boxShadow: '0 4px 24px rgba(0,0,0,0.6)',
      userSelect: 'none',
    }}>
      <div onMouseDown={onHeaderDown} style={{
        background: '#12121e', borderBottom: '1px solid #1e2a3a',
        padding: '5px 10px', display: 'flex', alignItems: 'center', gap: 8, cursor: 'grab',
      }}>
        <span style={{ fontSize: 11, color: '#6a8aaa', fontWeight: 600, flex: 1 }}>
          3D PREVIEW <span style={{ color: '#334', fontWeight: 400 }}>· {chunkLabel}</span>
        </span>
        <span style={{ fontSize: 9, color: '#2a3a4a' }}>drag to pan · middle-drag to orbit · scroll to zoom</span>
        <button onClick={() => useMapStore.getState().togglePreview3d()}
          style={{ background: 'none', border: 'none', color: '#556', cursor: 'pointer', fontSize: 14, lineHeight: 1, padding: 0 }}>
          ✕
        </button>
      </div>

      <canvas
        ref={canvasRef}
        className="p3d-canvas"
        width={W} height={H}
        onMouseDown={onCanvasDown}
        style={{ display: 'block', cursor: 'grab' }}
      />

      <div style={{
        background: '#0e0e18', borderTop: '1px solid #1a1a2a',
        padding: '3px 10px', fontSize: 9, color: '#2a3a3a',
      }}>
        All layers at this chunk · placeholder geometry · real GLBs load via asset catalog
      </div>
    </div>
  )
}
