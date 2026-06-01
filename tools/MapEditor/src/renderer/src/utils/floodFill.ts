/**
 * Async flood-fill (BFS) for continent-scale tile maps.
 *
 * Key design choices for large fills:
 *  - Bit-array visited tracking per chunk: 128 bytes per touched chunk
 *    instead of ~80 bytes per tile with string keys.  A 1M-tile fill
 *    spanning ~1000 chunks costs ~128 KB of visited state.
 *  - Index-pointer queue (never shifts): O(1) dequeue.
 *  - Yields via setTimeout(0) every BATCH_SIZE tiles so the browser
 *    stays responsive.
 *  - No hard cap — bounded only by available RAM.
 */

export interface FillParams {
  startX: number
  startY: number
  chunkSize: number
  layerId: string
  targetId: number
  activeAssetId: number
  getChunkData: (cx: number, cy: number) => Uint16Array | undefined
}

export interface FillCallbacks {
  onBatch: (tiles: Array<{ x: number; y: number }>) => void
  onProgress: (tilesFilledSoFar: number) => void
  onDone: (total: number) => void
  isCancelled: () => boolean
}

const BATCH_SIZE = 25_000

export function startFloodFill(params: FillParams, callbacks: FillCallbacks): void {
  const { startX, startY, chunkSize: cs, layerId, targetId, activeAssetId, getChunkData } = params
  const { onBatch, onProgress, onDone, isCancelled } = callbacks

  if (targetId === activeAssetId) { onDone(0); return }

  // ── Bit-array visited set (per chunk) ────────────────────────────────────
  // Each chunk has cs*cs tiles; we pack bits into Uint32 words.
  const WORDS_PER_CHUNK = Math.ceil((cs * cs) / 32)
  const visitedByChunk = new Map<string, Uint32Array>()

  function chunkVisitedKey(cx: number, cy: number): string {
    return `${cx},${cy}`
  }

  function isVisited(x: number, y: number): boolean {
    const cx = Math.floor(x / cs)
    const cy = Math.floor(y / cs)
    const lx = ((x % cs) + cs) % cs
    const ly = ((y % cs) + cs) % cs
    const bitIdx = ly * cs + lx
    const bits = visitedByChunk.get(chunkVisitedKey(cx, cy))
    if (!bits) return false
    return (bits[bitIdx >> 5] & (1 << (bitIdx & 31))) !== 0
  }

  function markVisited(x: number, y: number): void {
    const cx = Math.floor(x / cs)
    const cy = Math.floor(y / cs)
    const lx = ((x % cs) + cs) % cs
    const ly = ((y % cs) + cs) % cs
    const bitIdx = ly * cs + lx
    const key = chunkVisitedKey(cx, cy)
    let bits = visitedByChunk.get(key)
    if (!bits) {
      bits = new Uint32Array(WORDS_PER_CHUNK)
      visitedByChunk.set(key, bits)
    }
    bits[bitIdx >> 5] |= (1 << (bitIdx & 31))
  }

  // ── Tile ID reader ────────────────────────────────────────────────────────
  function getAt(x: number, y: number): number {
    const cx = Math.floor(x / cs)
    const cy = Math.floor(y / cs)
    const lx = ((x % cs) + cs) % cs
    const ly = ((y % cs) + cs) % cs
    return getChunkData(cx, cy)?.[ly * cs + lx] ?? 0
  }

  // ── Queue (index-pointer; never shifts — O(1) dequeue) ───────────────────
  // Encode (x, y) as a pair of Int32 in a growing typed array for speed.
  // We start with 256k slots and grow by doubling.
  let queueBuf = new Int32Array(512_000)  // 256k tile slots × 2 ints = 2MB initial
  let qHead = 0
  let qTail = 0

  function enqueue(x: number, y: number): void {
    if (qTail + 2 > queueBuf.length) {
      // Grow — copy to a new double-size buffer
      const next = new Int32Array(queueBuf.length * 2)
      next.set(queueBuf)
      queueBuf = next
    }
    queueBuf[qTail]     = x
    queueBuf[qTail + 1] = y
    qTail += 2
  }

  // Seed the queue — only enqueue if the start tile matches
  if (getAt(startX, startY) !== targetId) { onDone(0); return }
  markVisited(startX, startY)
  enqueue(startX, startY)

  let totalFilled = 0

  // ── BFS batch loop ────────────────────────────────────────────────────────
  function runBatch(): void {
    if (isCancelled()) { onDone(totalFilled); return }

    const batch: Array<{ x: number; y: number }> = []

    for (let i = 0; i < BATCH_SIZE && qHead < qTail; i++) {
      const x = queueBuf[qHead]
      const y = queueBuf[qHead + 1]
      qHead += 2

      if (getAt(x, y) !== targetId) continue
      batch.push({ x, y })

      // Enqueue 4 neighbours (mark visited on enqueue to avoid duplicates)
      const neighbours: Array<[number, number]> = [[x+1,y],[x-1,y],[x,y+1],[x,y-1]]
      for (const [nx, ny] of neighbours) {
        if (!isVisited(nx, ny)) {
          markVisited(nx, ny)
          enqueue(nx, ny)
        }
      }
    }

    if (batch.length > 0) {
      onBatch(batch)
      totalFilled += batch.length
      onProgress(totalFilled)
    }

    if (qHead < qTail) {
      setTimeout(runBatch, 0)  // yield to browser, resume next tick
    } else {
      onDone(totalFilled)
    }
  }

  setTimeout(runBatch, 0)
}
