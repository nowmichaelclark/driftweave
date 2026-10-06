// The painter.
//
// Every particle is 8 floats: current x, y; previous x, y; r, g, b; size.
// Two draws per particle -- a line from prev to current (the trail), and
// a small square at current (the head). If prev == current, the line is a
// no-op and only the head is drawn.
//
// One path, one look. Every style produces this format.

const BG_TOP = '#08131a';
const BG_BOTTOM = '#040a0d';

export function paint(ctx, parts, worldW, worldH, canvasW, canvasH) {
  ctx.setTransform(1, 0, 0, 1, 0, 0);
  const grad = ctx.createLinearGradient(0, 0, 0, canvasH);
  grad.addColorStop(0, BG_TOP);
  grad.addColorStop(1, BG_BOTTOM);
  ctx.fillStyle = grad;
  ctx.fillRect(0, 0, canvasW, canvasH);

  ctx.setTransform(canvasW / worldW, 0, 0, canvasH / worldH, 0, 0);
  ctx.lineCap = 'round';
  ctx.lineJoin = 'round';

  const n = parts.length;
  for (let i = 0; i < n; i += 8) {
    const x = parts[i];
    const y = parts[i + 1];
    const px = parts[i + 2];
    const py = parts[i + 3];
    const r = (parts[i + 4] * 255) | 0;
    const g = (parts[i + 5] * 255) | 0;
    const b = (parts[i + 6] * 255) | 0;
    const size = parts[i + 7];
    if (size <= 0.05) continue;

    const dx = x - px;
    const dy = y - py;
    const hasTrail = (dx > 0.15 || dx < -0.15 || dy > 0.15 || dy < -0.15);

    const color = `rgb(${r},${g},${b})`;
    if (hasTrail) {
      ctx.strokeStyle = color;
      ctx.lineWidth = size;
      ctx.beginPath();
      ctx.moveTo(px, py);
      ctx.lineTo(x, y);
      ctx.stroke();
    }
    ctx.fillStyle = color;
    ctx.fillRect(x - size * 0.5, y - size * 0.5, size, size);
  }
}
