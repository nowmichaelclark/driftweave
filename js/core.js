// The WebAssembly core, wrapped up.
//
// The module owns its memory and never grows it, so views onto that
// memory stay valid for the life of the page.

let ex = null;

export const NSPEC = 16;
export const PFLOATS = 8;

export async function loadCore() {
  const res = await fetch('js/driftweave.wasm', { cache: 'no-cache' });
  if (!res.ok) throw new Error(`could not fetch the core (${res.status})`);
  const bytes = await res.arrayBuffer();
  const { instance } = await WebAssembly.instantiate(bytes, {});
  ex = instance.exports;
  if (!ex.memory) throw new Error('the wasm module did not export memory');
  return ex;
}

export function specView() {
  return new Float32Array(ex.memory.buffer, ex.spec_ptr(), NSPEC);
}

export function partsView() {
  const n = ex.part_count();
  if (!n) return new Float32Array(0);
  return new Float32Array(ex.memory.buffer, ex.part_ptr(), n * PFLOATS);
}

export function init(w, h, seed) { ex.init(w, h, seed); }
export function resize(w, h) { ex.resize(w, h); }
export function step(dt) { ex.step(dt); }
