import { RGBA } from "@opentui/core"
import type { CliRenderer, OptimizedBuffer } from "@opentui/core"

// Animated theme switch: a wave sweeps out of a random point on a random
// screen edge and every cell flips to the new theme as the front passes it.
//
// It works as a renderer post-process, below React entirely: at trigger time
// we snapshot the last OLD-theme frame (char/fg/bg/attributes per cell), let
// React re-render the whole tree in the NEW theme underneath, and then — for
// the duration — repaint the snapshot onto every cell the wave hasn't reached
// yet. Cells just behind the front glow in the accent color for a beat.

const DURATION_MS = 500
const EDGE_WIDTH = 1.5
const X_SCALE = 0.55

let cancelActive: (() => void) | null = null

export function startThemeWipe(renderer: CliRenderer, accentHex: string): void {
  // re-triggering mid-wipe restarts from whatever is on screen right now
  cancelActive?.()

  const src = renderer.currentRenderBuffer
  const width = src.width
  const height = src.height
  const cells = width * height
  const snap = src.buffers
  const snapChar = snap.char.slice(0, cells)
  const snapAttr = snap.attributes.slice(0, cells)
  const snapFg = snap.fg.slice(0, cells * 4)
  const snapBg = snap.bg.slice(0, cells * 4)

  // a fresh origin per wipe: anywhere along any edge (0 top, 1 right, 2 bottom, 3 left)
  const edge = (Math.random() * 4) | 0
  const ox = edge === 1 ? width - 1 : edge === 3 ? 0 : Math.random() * (width - 1)
  const oy = edge === 0 ? 0 : edge === 2 ? height - 1 : Math.random() * (height - 1)

  // distance of every cell from the origin
  const dist = new Float32Array(cells)
  let maxDist = 0
  for (let y = 0; y < height; y++) {
    for (let x = 0; x < width; x++) {
      const dx = (x - ox) * X_SCALE
      const dy = y - oy
      const d = Math.sqrt(dx * dx + dy * dy)
      dist[y * width + x] = d
      if (d > maxDist) maxDist = d
    }
  }

  const accent = RGBA.fromHex(accentHex)
  const fgScratch = RGBA.fromValues(0, 0, 0, 1)
  const bgScratch = RGBA.fromValues(0, 0, 0, 1)
  const start = performance.now()

  let fn: (buffer: OptimizedBuffer, deltaTime: number) => void

  const cancel = () => {
    renderer.removePostProcessFn(fn)
    renderer.dropLive()
    if (cancelActive === cancel) cancelActive = null
  }

  fn = (buffer) => {
    // terminal resized mid-wipe: the snapshot grid no longer applies
    if (buffer.width !== width || buffer.height !== height) return cancel()

    const t = Math.min(1, (performance.now() - start) / DURATION_MS)
    const eased = 1 - (1 - t) * (1 - t) // ease-out: fast ignition, soft landing
    const r = eased * (maxDist + EDGE_WIDTH)
    const live = buffer.buffers

    for (let i = 0; i < cells; i++) {
      const d = dist[i]!
      if (d <= r - EDGE_WIDTH) continue // wave passed: new theme shows through

      const x = i % width
      const y = (i / width) | 0
      const o = i * 4

      if (d <= r) {
        // wavefront band: the just-flipped cell glows on an accent background
        fgScratch.buffer[0] = live.fg[o]!
        fgScratch.buffer[1] = live.fg[o + 1]!
        fgScratch.buffer[2] = live.fg[o + 2]!
        fgScratch.buffer[3] = live.fg[o + 3]!
        buffer.drawChar(live.char[i]!, x, y, fgScratch, accent, live.attributes[i]!)
        continue
      }

      // not reached yet: hold the old-theme snapshot in place
      fgScratch.buffer[0] = snapFg[o]!
      fgScratch.buffer[1] = snapFg[o + 1]!
      fgScratch.buffer[2] = snapFg[o + 2]!
      fgScratch.buffer[3] = snapFg[o + 3]!
      bgScratch.buffer[0] = snapBg[o]!
      bgScratch.buffer[1] = snapBg[o + 1]!
      bgScratch.buffer[2] = snapBg[o + 2]!
      bgScratch.buffer[3] = snapBg[o + 3]!
      buffer.drawChar(snapChar[i]!, x, y, fgScratch, bgScratch, snapAttr[i]!)
    }

    if (t >= 1) cancel()
  }

  cancelActive = cancel
  renderer.addPostProcessFn(fn)
  renderer.requestLive()
}
