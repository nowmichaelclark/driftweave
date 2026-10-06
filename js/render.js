// The painter.
//
// Three primitive kinds, dispatched on the first float of each record:
//   0 -- dot at (x1, y1), radius = width
//   1 -- line from (x1, y1) to (x2, y2), thickness = width
//   2 -- rect centred at (x1, y1), half-size (x2, y2)
//
// The raster path is separate: Flow writes RGBA into wasm memory and the
// caller reads it as a Uint8ClampedArray, blits to an offscreen canvas,
// and scales it up.

const TAU = Math.PI * 2;

const BG_TOP = '#08131a';
const BG_BOTTOM = '#040a0d';

export function paint(ctx, prims, worldW, worldH, canvasW, canvasH) {
  ctx.setTransform(1, 0, 0, 1, 0, 0);
  const g = ctx.createLinearGradient(0, 0, 0, canvasH);
  g.addColorStop(0, BG_TOP);
  g.addColorStop(1, BG_BOTTOM);
  ctx.fillStyle = g;
  ctx.fillRect(0, 0, canvasW, canvasH);

  ctx.setTransform(canvasW / worldW, 0, 0, canvasH / worldH, 0, 0);
  ctx.lineCap = 'round';
  ctx.lineJoin = 'round';

  // Rect pass first, so grid-like styles (Sand) sit under the rest.
  for (let i = 0; i < prims.length; i += 10) {
    if (prims[i] !== 2) continue;
    const cx = prims[i + 1], cy = prims[i + 2];
    const hw = prims[i + 3], hh = prims[i + 4];
    const r = prims[i + 5] * 255 | 0;
    const g = prims[i + 6] * 255 | 0;
    const b = prims[i + 7] * 255 | 0;
    const a = prims[i + 8];
    ctx.fillStyle = `rgba(${r},${g},${b},${a})`;
    ctx.fillRect(cx - hw, cy - hh, hw * 2, hh * 2);
  }

  // Lines next.
  for (let i = 0; i < prims.length; i += 10) {
    if (prims[i] !== 1) continue;
    const r = prims[i + 5] * 255 | 0;
    const g = prims[i + 6] * 255 | 0;
    const b = prims[i + 7] * 255 | 0;
    const a = prims[i + 8];
    ctx.strokeStyle = `rgba(${r},${g},${b},${a})`;
    ctx.lineWidth = prims[i + 9];
    ctx.beginPath();
    ctx.moveTo(prims[i + 1], prims[i + 2]);
    ctx.lineTo(prims[i + 3], prims[i + 4]);
    ctx.stroke();
  }

  // Dots last, on top.
  for (let i = 0; i < prims.length; i += 10) {
    if (prims[i] !== 0) continue;
    const r = prims[i + 5] * 255 | 0;
    const g = prims[i + 6] * 255 | 0;
    const b = prims[i + 7] * 255 | 0;
    const a = prims[i + 8];
    ctx.fillStyle = `rgba(${r},${g},${b},${a})`;
    ctx.beginPath();
    ctx.arc(prims[i + 1], prims[i + 2], prims[i + 9], 0, TAU);
    ctx.fill();
  }
}

// The raster path, used by Flow.
//
// The pixel view is put into a small ImageData, painted to an offscreen
// canvas at its native resolution, and then drawn scaled to fill the
// visible canvas. Two reasons for the two-stage route: `putImageData`
// ignores the current transform (so it cannot scale on its own), and the
// browser's image smoothing on `drawImage` is both faster and prettier
// than anything done by hand at this size.

let off = null;
let offCtx = null;
let offData = null;

export function paintRaster(ctx, view, rw, rh, canvasW, canvasH) {
  if (!off) {
    off = document.createElement('canvas');
    offCtx = off.getContext('2d');
  }
  if (off.width !== rw || off.height !== rh) {
    off.width = rw;
    off.height = rh;
    offData = null;
  }
  if (!offData) offData = offCtx.createImageData(rw, rh);
  offData.data.set(view);
  offCtx.putImageData(offData, 0, 0);

  ctx.setTransform(1, 0, 0, 1, 0, 0);
  ctx.fillStyle = '#000';
  ctx.fillRect(0, 0, canvasW, canvasH);
  ctx.imageSmoothingEnabled = true;
  ctx.imageSmoothingQuality = 'high';
  // The raster is square; the canvas may not be. Letterbox rather than
  // stretch, so the pattern keeps its proportions however the frame is
  // shaped.
  const scale = Math.min(canvasW / rw, canvasH / rh);
  const dw = rw * scale;
  const dh = rh * scale;
  const dx = (canvasW - dw) * 0.5;
  const dy = (canvasH - dh) * 0.5;
  ctx.drawImage(off, 0, 0, rw, rh, dx, dy, dw, dh);
}
