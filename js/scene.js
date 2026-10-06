// The composer. A scene is which simulation to run, plus the numbers
// that simulation needs, plus a palette.
//
// Six styles, all built on the same 16-float spec. The style's own draw()
// is the authority on what each field means for it; anything the style
// does not care about is left at whatever the previous field's default
// was, which for the Rust core is harmless.

import { Rng, randomSeed, seedName } from './rng.js';

export const NSPEC = 16;

export const S = {
  style: 0, count: 1, size: 2, alpha: 3,
  speed: 4, grav: 5, wind: 6, swirl: 7,
  spin: 8, jitter: 9, trail: 10, cohere: 11,
  hue: 12, sat: 13, bri: 14, walls: 15,
};

export const RANGES = [
  [0, 5],   // style
  [0, 1],   // count
  [0, 1],   // size
  [0, 1],   // alpha
  [0, 1],   // speed
  [0, 1],   // grav
  [0, 1],   // wind
  [0, 1],   // swirl
  [0, 1],   // spin
  [0, 1],   // jitter
  [0, 1],   // trail
  [0, 1],   // cohere
  [0, 1],   // hue
  [0, 1],   // sat
  [0, 1],   // bri
  [0, 3],   // walls
];

export const WALLS = ['wrap', 'bounce', 'soft', 'none'];

// Six styles. Each style's draw() writes every field it cares about; the
// remaining fields are set to zero by the initial Float32Array, which is
// the correct default for the ones that are unused.
export const STYLES = [
  {
    id: 'orbit', label: 'Orbit',
    // Chaotic N-body with no central mass. Bodies start on a slowly
    // rotating disk and are then governed only by their mutual gravity.
    // The system is chaotic by nature -- there is no equilibrium state
    // for more than two similar masses -- so it never comes to rest.
    hue: [[0.02, 0.10], [0.55, 0.65], [0.78, 0.90], [0.42, 0.52]],
    draw(r, spec) {
      spec[S.count] = r.range(0.4, 1.0);
      spec[S.size] = r.range(0.3, 0.9);
      spec[S.alpha] = r.range(0.6, 1.0);
      spec[S.trail] = r.range(0.5, 1.0);
      spec[S.walls] = 0;
    },
  },
  {
    id: 'flock', label: 'Flock',
    // Boids. Three local rules, no global force; the flock's shape is
    // emergent. A slowly drifting wind keeps the swarm from ever being
    // able to align perfectly.
    hue: [[0.00, 0.10], [0.32, 0.46], [0.58, 0.68], [0.88, 1.00]],
    draw(r, spec) {
      spec[S.count] = r.range(0.4, 0.9);
      spec[S.size] = r.range(0.25, 0.6);
      spec[S.alpha] = r.range(0.7, 1.0);
      spec[S.trail] = r.range(0.2, 0.7);
      spec[S.cohere] = 1.0;
      spec[S.walls] = 2;
    },
  },
  {
    id: 'wells', label: 'Wells',
    // Three attractors move on independent Lissajous paths; the middle
    // one repels. Particles swirl around the moving wells and are
    // caught in the changing fields, so no configuration is ever stable.
    hue: [[0.50, 0.65], [0.72, 0.85], [0.00, 0.12], [0.28, 0.42]],
    draw(r, spec) {
      spec[S.count] = r.range(0.5, 1.0);
      spec[S.size] = r.range(0.2, 0.5);
      spec[S.alpha] = r.range(0.5, 0.9);
      spec[S.speed] = r.range(0.3, 0.8);
      spec[S.swirl] = r.range(0.3, 0.9);
      spec[S.spin] = r.range(0.3, 1.0);
      spec[S.trail] = r.range(0.5, 1.0);
      spec[S.walls] = 0;
    },
  },
  {
    id: 'flow', label: 'Flow',
    // Particles following a smooth vector field whose angle depends on
    // position AND on time. The field is never the same twice, so
    // particles have no fixed point to settle into.
    hue: [[0.42, 0.56], [0.00, 0.08], [0.72, 0.86], [0.28, 0.42]],
    draw(r, spec) {
      spec[S.count] = r.range(0.6, 1.0);
      spec[S.size] = r.range(0.3, 0.6);
      spec[S.alpha] = r.range(0.6, 1.0);
      spec[S.speed] = r.range(0.4, 1.0);
      spec[S.trail] = r.range(0.6, 1.0);
      spec[S.walls] = 0;
    },
  },
  {
    id: 'chain', label: 'Chain',
    // Spring networks hanging from points along the top edge. Gravity
    // holds them down; a wind that varies in time and space keeps them
    // swaying. Even at wind=0 there is an ambient force, so the chains
    // never settle.
    hue: [[0.00, 0.08], [0.28, 0.42], [0.52, 0.62], [0.86, 0.98]],
    draw(r, spec) {
      spec[S.count] = r.range(0.5, 0.9);
      spec[S.size] = r.range(0.35, 0.7);
      spec[S.alpha] = r.range(0.7, 1.0);
      spec[S.grav] = 0.5;
      spec[S.wind] = r.range(0.3, 1.0);
      spec[S.trail] = r.range(0.1, 0.5);
      spec[S.walls] = 3;
    },
  },
  {
    id: 'repel', label: 'Repel',
    // Mutual repulsion plus a central force that alternates between
    // attracting and repelling on a ~6 second cycle. The cloud is
    // forever expanding, contracting, and finding a new shape.
    hue: [[0.08, 0.18], [0.55, 0.65], [0.85, 0.95]],
    draw(r, spec) {
      spec[S.count] = r.range(0.5, 1.0);
      spec[S.size] = r.range(0.25, 0.6);
      spec[S.alpha] = r.range(0.5, 0.9);
      spec[S.trail] = r.range(0.4, 0.9);
      spec[S.cohere] = r.range(0.2, 1.0);
      spec[S.walls] = 2;
    },
  },
];

export function newSpec(seed = randomSeed(), styleIndex = null) {
  const r = new Rng(seed);
  const style = styleIndex == null
    ? r.int(0, STYLES.length - 1)
    : Math.max(0, Math.min(STYLES.length - 1, styleIndex));
  const s = STYLES[style];

  const spec = new Float32Array(NSPEC);
  spec[S.style] = style;

  s.draw(r, spec);

  const family = r.pick(s.hue);
  spec[S.hue] = r.range(family[0], family[1]);
  spec[S.sat] = r.range(0.55, 0.95);
  spec[S.bri] = r.range(0.70, 1.00);

  return { spec, style, seed: seed >>> 0, name: seedName(seed >>> 0) };
}

// ------------------------------------------------------------ share codes

const ALPHABET = '0123456789ABCDEFGHJKMNPQRSTVWXYZ';
const DECODE = (() => {
  const m = {};
  for (let i = 0; i < ALPHABET.length; i++) m[ALPHABET[i]] = i;
  m.I = 1; m.L = 1; m.O = 0; m.U = 0;
  return m;
})();

const FORMAT = 2;
export const PREFIX = 'DW2-';

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
  for (const x of bytes) {
    a = (a + x) % 255;
    b = (b + a) % 255;
  }
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

export function encodeScene(scene) {
  const name = (scene.name || '').slice(0, 24);
  const generated = seedName(scene.seed >>> 0);
  const hasName = name.length > 0 && name !== generated;
  const encodedName = hasName ? new TextEncoder().encode(name) : new Uint8Array(0);

  const payload = new Uint8Array(2 + (hasName ? 1 + encodedName.length : 0) + 4 + NSPEC);
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

  return {
    spec: Array.from(spec),
    seed,
    name: name || seedName(seed),
    style: Math.round(spec[S.style]),
  };
}

export function codeKind(text) {
  const t = String(text || '').trim().toUpperCase();
  if (t.startsWith(PREFIX)) return 'scene';
  return null;
}
