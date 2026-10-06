// The WebAssembly core, wrapped up.
//
// The module owns its own memory and never grows it, so once the views are
// made they stay valid for the life of the page. Reading the draw list is
// free: it is a Float32Array pointed straight at the wasm.

let exports = null;

export async function loadCore() {
  const res = await fetch('js/driftweave.wasm', { cache: 'no-cache' });
  if (!res.ok) throw new Error(`could not fetch the core (${res.status})`);
  const bytes = await res.arrayBuffer();
  const { instance } = await WebAssembly.instantiate(bytes, {});
  exports = instance.exports;
  if (!exports.memory) {
    throw new Error('the wasm module did not export memory');
  }
  return exports;
}

// The 24 spec floats. The browser writes them, then calls init().
export function spec() {
  return new Float32Array(exports.memory.buffer, exports.spec_ptr(), 24);
}

// The draw list: prim_count() records of 10 floats each. Do not keep a
// reference across frames -- just read it right after step().
export function prims() {
  const n = exports.prim_count();
  if (!n) return new Float32Array(0);
  return new Float32Array(exports.memory.buffer, exports.prim_ptr(), n * 10);
}

export function init(w, h, seed) { exports.init(w, h, seed); }
export function resize(w, h) { exports.resize(w, h); }
export function step(dt) { exports.step(dt); }
