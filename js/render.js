// The painter. The draw list is flat: kind, x1, y1, x2, y2, r, g, b, a,
// width -- ten floats a record, in paint order. Everything here is in
// world coordinates; the caller sets the transform before calling draw().

const TAU = Math.PI * 2;

// Background is drawn flat, not in world space. A vertical gradient keeps
// the empty part of the frame from looking like a dead panel.
const BG_TOP = '#08131a';
const BG_BOTTOM = '#040a0d';

export function paint(ctx, prims, worldW, worldH, canvasW, canvasH) {
  // Background, in canvas pixels.
  ctx.setTransform(1, 0, 0, 1, 0, 0);
  const g = ctx.createLinearGradient(0, 0, 0, canvasH);
  g.addColorStop(0, BG_TOP);
  g.addColorStop(1, BG_BOTTOM);
  ctx.fillStyle = g;
  ctx.fillRect(0, 0, canvasW, canvasH);

  // Everything from here on is in world units.
  ctx.setTransform(canvasW / worldW, 0, 0, canvasH / worldH, 0, 0);
  ctx.lineCap = 'round';
  ctx.lineJoin = 'round';

  // Two passes: lines first (they are the springs and trails and belong
  // behind), then dots. Because the list is already in the right order,
  // this is just a reorganisation, not a reorder.
  for (let i = 0; i < prims.length; i += 10) {
    if (prims[i] !== 1) continue;
    const x1 = prims[i + 1], y1 = prims[i + 2];
    const x2 = prims[i + 3], y2 = prims[i + 4];
    const r = prims[i + 5] * 255 | 0;
    const g = prims[i + 6] * 255 | 0;
    const b = prims[i + 7] * 255 | 0;
    const a = prims[i + 8];
    const w = prims[i + 9];
    ctx.strokeStyle = `rgba(${r},${g},${b},${a})`;
    ctx.lineWidth = w;
    ctx.beginPath();
    ctx.moveTo(x1, y1);
    ctx.lineTo(x2, y2);
    ctx.stroke();
  }

  for (let i = 0; i < prims.length; i += 10) {
    if (prims[i] !== 0) continue;
    const x = prims[i + 1], y = prims[i + 2];
    const r = prims[i + 5] * 255 | 0;
    const g = prims[i + 6] * 255 | 0;
    const b = prims[i + 7] * 255 | 0;
    const a = prims[i + 8];
    const rad = prims[i + 9];
    ctx.fillStyle = `rgba(${r},${g},${b},${a})`;
    ctx.beginPath();
    ctx.arc(x, y, rad, 0, TAU);
    ctx.fill();
  }
}
