// The WebAssembly core, wrapped up.
//
// Same idea as before, with the raster path added. The module owns its
// own memory and never grows it, so once the views are made they stay
// valid for the life of the page.

let exports = null;

export async function loadCore() {
  const res = await fetch('js/driftweave.wasm', { cache: 'no-cache' });
  if (!res.ok) throw new Error(`could not fetch the core (${res.status})`);
  const bytes = await res.arrayBuffer();
  const { instance } = await WebAssembly.instantiate(bytes, {});
  exports = instance.exports;
  if (!exports.memory) throw new Error('the wasm module did not export memory');
  return exports;
}

export function spec() {
  return new Float32Array(exports.memory.buffer, exports.spec_ptr(), 24);
}

export function prims() {
  const n = exports.prim_count();
  if (!n) return new Float32Array(0);
  return new Float32Array(exports.memory.buffer, exports.prim_ptr(), n * 10);
}

// Raster path, used by Flow. `isRaster()` is the check JS branches on.
export function isRaster() { return exports.is_raster() === 1; }
export function rasterW() { return exports.raster_w(); }
export function rasterH() { return exports.raster_h(); }
export function rasterView() {
  const n = exports.raster_w() * exports.raster_h() * 4;
  return new Uint8ClampedArray(exports.memory.buffer, exports.raster_ptr(), n);
}

export function init(w, h, seed) { exports.init(w, h, seed); }
export function resize(w, h) { exports.resize(w, h); }
export function step(dt) { exports.step(dt); }
