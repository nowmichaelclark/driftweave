// The composer. A scene is eight decisions about how to move bodies
// through a frame, plus a small palette. Everything else is the seed.

import { Rng, randomSeed, seedName } from './rng.js';

export const NSPEC = 24;

// Spec index. Keep in sync with core/src/lib.rs.
export const S = {
  style: 0, count: 1, grav: 2, drag: 3, bounce: 4, wind: 5, swirl: 6,
  jitter: 7, mutual: 8, spacing: 9, k: 10, fields: 11, size: 12, trail: 13,
  pulse: 14, tempo: 15, hue: 16, sat: 17, bri: 18, wall: 19, sub: 20,
  density: 21, scale: 22, aspect: 23,
};

// The range each spec value lives in. The share code quantises to a byte
// within these, so nothing can go out of range and come back different.
export const RANGES = [
  [0, 7], [0, 1], [-1, 1], [0, 3], [0, 1], [-1, 1], [-1, 1], [0, 1],
  [0, 1], [0, 40], [0, 2], [0, 6], [1, 10], [0, 1], [0, 1], [0.25, 4],
  [0, 1], [0, 1], [0, 1], [0, 3], [1, 4], [0, 1], [0, 1], [0.5, 3],
];

// Wall modes, named for the code and the readout.
export const WALLS = ['bounce', 'wrap', 'recycle', 'soft'];

// Eight styles. Each has a small table of overrides: the ranges the seed
// draws from when this style is chosen. Anything not listed uses the
// default range above.
//
// The name is what the readout shows. It is about the *motion*, not about
// what it looks like -- "Lattice" describes a net of springs, "Bloom" a
// heartbeat, and neither tells you what colour it is.

export const STYLES = [
  { id: 'lattice', label: 'Lattice',
    ranges: { grav: [0.35, 1.1], k: [0.55, 1.3], drag: [0.4, 1.1], bounce: [0.15, 0.55],
      wall: 0, count: [0.55, 1.0], size: [1.4, 3.0], trail: [0, 0.18],
      fields: [0, 2], spacing: [8, 16], sub: [2, 3] },
    hue: [[0.04, 0.11], [0.52, 0.60], [0.78, 0.90]] },

  { id: 'orbit', label: 'Orbit',
    ranges: { grav: [0, 0], swirl: [0.25, 0.95], drag: [0.05, 0.35],
      wall: 1, count: [0.6, 1.0], size: [1.6, 3.2], trail: [0.3, 0.8],
      fields: [1, 3], sub: [1, 2] },
    hue: [[0.5, 0.65], [0.72, 0.82], [0.36, 0.46]] },

  { id: 'rain', label: 'Rain',
    ranges: { grav: [0.35, 0.95], drag: [0.05, 0.3], bounce: [0.05, 0.35],
      wall: 2, count: [0.5, 1.0], size: [1.2, 2.4], trail: [0.5, 0.95],
      jitter: [0, 0.15], fields: [0, 1], sub: [1, 2] },
    hue: [[0.55, 0.7], [0.9, 1.0], [0.1, 0.2]] },

  { id: 'swarm', label: 'Swarm',
    ranges: { grav: [0, 0], mutual: [0.5, 1.0], spacing: [10, 22], drag: [0.4, 0.9],
      wall: 3, count: [0.4, 0.8], size: [1.4, 2.6], trail: [0.15, 0.55],
      swirl: [-0.2, 0.2], fields: [0, 2], sub: [1, 2] },
    hue: [[0.0, 0.1], [0.32, 0.42], [0.78, 0.92]] },

  { id: 'weave', label: 'Weave',
    ranges: { grav: [0.35, 0.85], k: [0.7, 1.5], drag: [0.5, 1.2], bounce: [0.05, 0.3],
      wall: 0, count: [0.35, 0.75], size: [1.6, 3.0], trail: [0.05, 0.4],
      wind: [-0.4, 0.4], swirl: [0.2, 0.7], fields: [1, 3], sub: [2, 3] },
    hue: [[0.42, 0.58], [0.02, 0.1], [0.86, 0.98]] },

  { id: 'bloom', label: 'Bloom',
    ranges: { grav: [0, 0], k: [0.4, 1.1], drag: [0.5, 1.1], pulse: [0.4, 1.0],
      tempo: [0.5, 2.4], wall: 3, count: [0.6, 1.0], size: [1.8, 3.6],
      trail: [0.2, 0.6], swirl: [-0.3, 0.3], fields: [0, 2], sub: [1, 2] },
    hue: [[0.9, 1.0], [0.06, 0.14], [0.76, 0.88]] },

  { id: 'drift', label: 'Drift',
    ranges: { grav: [0, 0], swirl: [-0.6, 0.6], drag: [0.1, 0.5],
      wall: 1, count: [0.5, 1.0], size: [1.4, 3.0], trail: [0.35, 0.85],
      jitter: [0, 0.06], fields: [2, 4], sub: [1, 2] },
    hue: [[0.14, 0.24], [0.5, 0.6], [0.82, 0.92]] },

  { id: 'web', label: 'Web',
    ranges: { grav: [0.4, 0.9], k: [0.55, 1.2], drag: [0.6, 1.3], bounce: [0.05, 0.25],
      wall: 3, count: [0.5, 0.95], size: [1.2, 2.4], trail: [0, 0.3],
      wind: [-0.5, 0.5], fields: [0, 2], sub: [2, 3] },
    hue: [[0.0, 0.08], [0.58, 0.68], [0.32, 0.44]] },
];

// Draw a value from one entry of RANGES, with an optional override range.
function drawRange(r, override, specIdx) {
  const [lo, hi] = override || RANGES[specIdx];
  // A single-valued override (like `wall: 0` or `grav: [0, 0]`) is a
  // constant, not a range. It still consumes one random number so the
  // stream stays the same length.
  const v = r.f();
  if (lo === hi) return lo;
  return lo + v * (hi - lo);
}

export function newSpec(seed = randomSeed(), styleIndex = null) {
  const r = new Rng(seed);
  const style = styleIndex == null
    ? r.int(0, STYLES.length - 1)
    : Math.max(0, Math.min(STYLES.length - 1, styleIndex));
  const s = STYLES[style];

  const spec = new Float32Array(NSPEC);
  spec[S.style] = style;

  // Style-independent range overrides.
  const override = (key) => s.ranges[key];

  // Draw every value in a fixed order, regardless of whether the style
  // overrides it. Fixed order means the same seed always produces the same
  // stream, so adding a new style later cannot change existing scenes.
  for (let i = 1; i < NSPEC; i++) {
    const key = Object.keys(S).find((k) => S[k] === i);
    // Wall, style and sub are drawn as integers where they are integers.
    const value = drawRange(r, override(key), i);
    spec[i] = value;
  }

  // A style can pin a value after the draw, which is how `wall: 0` sets
  // the lattice to bounce walls without removing the draw.
  if (override('wall') != null) spec[S.wall] = override('wall');
  if (override('grav') && override('grav')[0] === override('grav')[1]) spec[S.grav] = override('grav')[0];

  // Style 0 wants positive gravity; style 1 wants none. The draw already
  // respects that, but a couple of these need a final snap because the
  // override is a single value rather than a range.
  spec[S.wall] = Math.round(spec[S.wall]);
  spec[S.sub] = Math.round(spec[S.sub]);
  spec[S.fields] = Math.round(spec[S.fields]);

  // Palette: drawn from the style's hue families, so two Lattices never
  // land on the same colour by accident.
  const family = r.pick(s.hue);
  spec[S.hue] = r.range(family[0], family[1]);
  spec[S.sat] = r.range(0.55, 0.95);
  spec[S.bri] = r.range(0.7, 1.0);

  // Density is separate from count: it is a hint to the UI about how full
  // the scene wants to be, so the density slider can be moved without
  // rebuilding.
  spec[S.density] = spec[S.count];

  // Author's preferred aspect. The UI can override; the code carries it so
  // a scene shared from a 9:16 phone opens as a 9:16 scene.
  spec[S.aspect] = 1;

  return { spec, style, seed, name: seedName(seed) };
}

// A scene bundled up with its seed and name, which is what the code and
// the save list actually carry.
export function sceneOf(spec, seed, name) {
  return { spec: Array.from(spec), seed, name };
}

// ------------------------------------------------------------ share codes

// Crockford base32: no I, L, O or U, so there is no 1/l or 0/O confusion,
// and it is case-insensitive.
const ALPHABET = '0123456789ABCDEFGHJKMNPQRSTVWXYZ';
const DECODE = (() => {
  const m = {};
  for (let i = 0; i < ALPHABET.length; i++) m[ALPHABET[i]] = i;
  m.I = 1; m.L = 1; m.O = 0; m.U = 0;
  return m;
})();

const FORMAT = 1;
export const PREFIX = 'DW1-';

function quantise(v, i) {
  const [lo, hi] = RANGES[i];
  const t = hi > lo ? (v - lo) / (hi - lo) : 0;
  return Math.max(0, Math.min(255, Math.round(t * 255)));
}

function dequantise(b, i) {
  const [lo, hi] = RANGES[i];
  return lo + (b / 255) * (hi - lo);
}

function checksum(bytes) {
  let a = 0, b = 0;
  for (const x of bytes) { a = (a + x) % 255; b = (b + a) % 255; }
  return (b << 8) | a;
}

function toBase32(bytes) {
  let out = '', buf = 0, bits = 0;
  for (const x of bytes) {
    buf = (buf << 8) | x;
    bits += 8;
    while (bits >= 5) {
      out += ALPHABET[(buf >> (bits - 5)) & 31];
      bits -= 5;
    }
  }
  if (bits > 0) out += ALPHABET[(buf << (5 - bits)) & 31];
  return out;
}

function fromBase32(text) {
  const clean = text.toUpperCase().replace(/[^0-9A-Z]/g, '');
  const out = [];
  let buf = 0, bits = 0;
  for (const ch of clean) {
    const v = DECODE[ch];
    if (v === undefined) throw new Error('bad character in code');
    buf = (buf << 5) | v;
    bits += 5;
    if (bits >= 8) {
      out.push((buf >> (bits - 8)) & 0xff);
      bits -= 8;
    }
  }
  return Uint8Array.from(out);
}

// encodeScene: version, flags, seed, name, spec, checksum.
export function encodeScene(scene) {
  const name = (scene.name || '').slice(0, 24);
  const hasName = name !== seedName(scene.seed);
  const encodedName = hasName ? new TextEncoder().encode(name) : new Uint8Array(0);
  const payload = new Uint8Array(
    2 + (hasName ? 1 + encodedName.length : 0) + 4 + NSPEC
  );
  let o = 0;
  payload[o++] = FORMAT;
  payload[o++] = hasName ? 1 : 0;
  if (hasName) {
    payload[o++] = encodedName.length;
    payload.set(encodedName, o);
    o += encodedName.length;
  }
  const seed = scene.seed >>> 0;
  payload[o++] = (seed >>> 24) & 0xff;
  payload[o++] = (seed >>> 16) & 0xff;
  payload[o++] = (seed >>> 8) & 0xff;
  payload[o++] = seed & 0xff;
  for (let i = 0; i < NSPEC; i++) payload[o++] = quantise(scene.spec[i], i);

  const sum = checksum(payload);
  const all = Uint8Array.from([...payload, (sum >> 8) & 0xff, sum & 0xff]);
  const body = toBase32(all);
  const grouped = body.match(/.{1,5}/g).join('-');
  return PREFIX + grouped;
}

export function decodeScene(code) {
  const t = String(code || '').trim().toUpperCase();
  const head = t.startsWith(PREFIX) ? t.slice(PREFIX.length) : t;
  const all = fromBase32(head);
  if (all.length < 3) throw new Error('code is too short');
  const payload = all.slice(0, all.length - 2);
  const given = (all[all.length - 2] << 8) | all[all.length - 1];
  if (checksum(payload) !== given) throw new Error('that code has a typo in it');

  let o = 0;
  const version = payload[o++];
  if (version > FORMAT) throw new Error('that code was made by a newer version');
  const hasName = payload[o++] === 1;
  let name = null;
  if (hasName) {
    const n = payload[o++];
    name = new TextDecoder().decode(payload.slice(o, o + n));
    o += n;
  }
  const seed = ((payload[o] << 24) | (payload[o + 1] << 16) | (payload[o + 2] << 8) | payload[o + 3]) >>> 0;
  o += 4;
  const spec = new Float32Array(NSPEC);
  for (let i = 0; i < NSPEC; i++) spec[i] = dequantise(payload[o++], i);
  return { spec: Array.from(spec), seed, name: name || seedName(seed) };
}

export function codeKind(text) {
  const t = String(text || '').trim().toUpperCase();
  return t.startsWith(PREFIX) ? 'scene' : null;
}
