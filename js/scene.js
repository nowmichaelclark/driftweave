// The composer. A scene is which simulation to run, plus the numbers that
// simulation needs, plus a palette.
//
// The five styles are genuinely different models -- see core/src/lib.rs.
// What the spec fields *mean* is different per style, which is why each
// one's draw below is a full override of the shared spec rather than a
// delta to it. The RANGES table is the code format's, not the style's;
// the style decides what its own values should be.

import { Rng, randomSeed, seedName } from './rng.js';

// Twenty-four floats, fixed layout. Index constants are exported so the
// UI and the code format can both name them.
export const NSPEC = 24;

export const S = {
  style:   0,
  count:   1,
  grav:    2,
  drag:    3,
  bounce:  4,
  wind:    5,
  swirl:   6,
  jitter:  7,
  mutual:  8,
  spacing: 9,
  k:       10,
  fields:  11,
  size:    12,
  trail:   13,
  pulse:   14,
  tempo:   15,
  hue:     16,
  sat:     17,
  bri:     18,
  wall:    19,
  sub:     20,
  density: 21,
  scale:   22,
  aspect:  23,
};

// The range each spec value can be encoded in. The share code quantises
// to a byte inside this range, so nothing can go out of bounds and come
// back different. These are the code format's bounds, not the style's:
// a style can use a narrower range, but nothing can exceed these.
export const RANGES = [
  [0, 4],        // style index (five styles now, room for three more)
  [0, 1],        // count, as a fraction of the style's maximum
  [-1, 1],       // gravity / flow f-offset
  [0, 3],        // drag
  [0, 1],        // bounce
  [-1, 1],       // wind / flow k-offset
  [-1, 1],       // swirl (unused by the current styles, kept for future)
  [0, 1],        // jitter
  [0, 1],        // mutual
  [0, 40],       // spacing
  [0, 2],        // spring stiffness
  [0, 6],        // attractor fields
  [0.5, 10],     // body size
  [0, 1],        // trail
  [0, 1],        // pulse strength
  [0.25, 4],     // pulse tempo
  [0, 1],        // hue
  [0, 1],        // saturation
  [0, 1],        // brightness
  [0, 3],        // wall mode
  [1, 4],        // physics substeps
  [0, 1],        // density (UI hint)
  [0, 1],        // scale (reserved)
  [0.25, 4],     // author's aspect, width/height
];

// Wall modes, named for the readout. Kept as an exported list so a UI
// can show them without duplicating the strings.
export const WALLS = ['bounce', 'wrap', 'recycle', 'soft'];

// Five styles. Each one's draw() sets every field the core reads for it;
// anything the core ignores for that style is set to a harmless value so
// the spec is always fully populated.
export const STYLES = [
  {
    id: 'body',
    label: 'Body',
    // N-body gravitational. Slow, evolving, no repetition. The disk is
    // seeded slightly sub-circular so it spirals inward over minutes.
    hue: [[0.02, 0.10], [0.55, 0.65], [0.78, 0.90]],
    draw(r, spec) {
      spec[S.count]   = r.range(0.35, 1.00);
      spec[S.grav]    = 0;
      spec[S.drag]    = r.range(0.05, 0.35);
      spec[S.trail]   = r.range(0.35, 0.95);
      spec[S.size]    = r.range(1.6, 3.2);
      spec[S.jitter]  = 0;
      spec[S.wind]    = 0;
      spec[S.swirl]   = 0;
      spec[S.k]       = 0;
      spec[S.spacing] = 0;
      spec[S.wall]    = 1;
      spec[S.sub]     = r.int(1, 2);
      spec[S.fields]  = 0;
      spec[S.bounce]  = 0;
      spec[S.mutual]  = 0;
      spec[S.pulse]   = 0;
      spec[S.tempo]   = 1;
      spec[S.density] = spec[S.count];
    },
  },
  {
    id: 'cloth',
    label: 'Cloth',
    // Verlet sheet, pinned at the top, dropped into wind. Reads as
    // fabric within a second of loading -- a flap no particle system
    // can imitate.
    hue: [[0.00, 0.08], [0.28, 0.42], [0.52, 0.62], [0.86, 0.98]],
    draw(r, spec) {
      spec[S.count]   = r.range(0.55, 1.00);
      spec[S.grav]    = r.range(0.45, 0.95);
      spec[S.wind]    = r.range(-0.9, 0.9);
      spec[S.drag]    = r.range(0.4, 0.9);
      spec[S.k]       = r.range(0.3, 1.0);
      spec[S.trail]   = r.range(0, 0.35);
      spec[S.size]    = r.range(1.2, 2.4);
      spec[S.bounce]  = 0;
      spec[S.jitter]  = 0;
      spec[S.swirl]   = 0;
      spec[S.spacing] = 0;
      spec[S.wall]    = 3;
      spec[S.sub]     = r.int(2, 3);
      spec[S.fields]  = 0;
      spec[S.mutual]  = 0;
      spec[S.pulse]   = 0;
      spec[S.tempo]   = 1;
      spec[S.density] = spec[S.count];
    },
  },
  {
    id: 'flock',
    label: 'Flock',
    // Boids. Three rules, no forces. Moves like a living thing, and is
    // unmistakably not the N-body simulation however similar it looks
    // in a still frame.
    hue: [[0.00, 0.10], [0.32, 0.46], [0.58, 0.68], [0.88, 1.00]],
    draw(r, spec) {
      spec[S.count]   = r.range(0.35, 0.95);
      spec[S.drag]    = r.range(0.05, 0.35);
      spec[S.trail]   = r.range(0.15, 0.75);
      spec[S.size]    = r.range(1.6, 3.0);
      spec[S.grav]    = 0;
      spec[S.wind]    = 0;
      spec[S.swirl]   = 0;
      spec[S.jitter]  = 0;
      spec[S.k]       = 0;
      spec[S.spacing] = 0;
      spec[S.wall]    = 3;
      spec[S.sub]     = r.int(1, 2);
      spec[S.fields]  = 0;
      spec[S.bounce]  = 0;
      spec[S.mutual]  = 0;
      spec[S.pulse]   = 0;
      spec[S.tempo]   = 1;
      spec[S.density] = spec[S.count];
    },
  },
  {
    id: 'sand',
    label: 'Sand',
    // Falling-sand cellular automaton. Discrete, granular, and the only
    // one here where the picture accumulates: over a minute the pile
    // grows and the shape is different every seed.
    hue: [[0.06, 0.14], [0.02, 0.08], [0.55, 0.62]],
    draw(r, spec) {
      spec[S.count]   = 1;
      spec[S.trail]   = 0;
      spec[S.size]    = 1;
      spec[S.grav]    = 0;
      spec[S.wind]    = 0;
      spec[S.swirl]   = 0;
      spec[S.jitter]  = 0;
      spec[S.k]       = 0;
      spec[S.spacing] = 0;
      spec[S.wall]    = 0;
      spec[S.sub]     = 1;
      spec[S.fields]  = 0;
      spec[S.bounce]  = 0;
      spec[S.mutual]  = 0;
      spec[S.pulse]   = 0;
      spec[S.tempo]   = 1;
      spec[S.density] = 1;
    },
  },
  {
    id: 'flow',
    label: 'Flow',
    // Gray-Scott reaction-diffusion. Not particles at all; the output
    // is a raster. The two parameters wander through the region of
    // (f, k) space that produces wormlike patterns rather than spots
    // or stripes, because worms are the ones that keep moving.
    hue: [[0.42, 0.56], [0.00, 0.08], [0.72, 0.86], [0.28, 0.42]],
    draw(r, spec) {
      spec[S.grav]    = r.range(-0.15, 0.15);
      spec[S.wind]    = r.range(-0.15, 0.15);
      spec[S.count]   = 1;
      spec[S.trail]   = 0;
      spec[S.size]    = 1;
      spec[S.drag]    = 0;
      spec[S.bounce]  = 0;
      spec[S.swirl]   = 0;
      spec[S.jitter]  = 0;
      spec[S.k]       = 0;
      spec[S.spacing] = 0;
      spec[S.wall]    = 0;
      spec[S.sub]     = 1;
      spec[S.fields]  = 0;
      spec[S.mutual]  = 0;
      spec[S.pulse]   = 0;
      spec[S.tempo]   = 1;
      spec[S.density] = 1;
    },
  },
];

// Draw a scene. The style is either drawn from the seed or forced with
// `styleIndex`. The spec is fully populated -- every field -- before
// returning, so the core sees a well-formed recipe on every call.
//
// The draw order is deterministic: the same seed with the same style
// produces the same spec, and therefore the same scene. That is what
// makes a share code enough to carry one.
export function newSpec(seed = randomSeed(), styleIndex = null) {
  const r = new Rng(seed);
  const style = styleIndex == null
    ? r.int(0, STYLES.length - 1)
    : Math.max(0, Math.min(STYLES.length - 1, styleIndex));
  const s = STYLES[style];

  const spec = new Float32Array(NSPEC);
  spec[S.style] = style;

  // The style sets everything it cares about.
  s.draw(r, spec);

  // Palette. One hue family per style, so two Bodies never land on the
  // same colour by accident, and the same hue family in different
  // families of the style's own list reads as different moods.
  const family = r.pick(s.hue);
  spec[S.hue] = r.range(family[0], family[1]);
  spec[S.sat] = r.range(0.55, 0.95);
  spec[S.bri] = r.range(0.70, 1.00);

  // The aspect is the author's preference. The UI can override; the
  // code carries it so a scene shared from a 9:16 phone opens as a
  // 9:16 scene by default.
  spec[S.aspect] = 1;

  return { spec, style, seed: seed >>> 0, name: seedName(seed >>> 0) };
}

// A scene bundled with the seed and name it carries through save and
// share. The spec is a plain array here, not a Float32Array, so it
// survives JSON round-trips intact.
export function sceneOf(spec, seed, name) {
  return {
    spec: Array.from(spec),
    seed: seed >>> 0,
    name: name || seedName(seed >>> 0),
    style: Math.round(spec[S.style]),
  };
}

// ------------------------------------------------------------ share codes

// Crockford base32: no I, L, O or U, so there is no 1/l or 0/O
// confusion, and it is case-insensitive.
const ALPHABET = '0123456789ABCDEFGHJKMNPQRSTVWXYZ';
const DECODE = (() => {
  const m = {};
  for (let i = 0; i < ALPHABET.length; i++) m[ALPHABET[i]] = i;
  m.I = 1; m.L = 1; m.O = 0; m.U = 0;
  return m;
})();

const FORMAT = 1;
export const PREFIX = 'DW1-';

// Quantise a spec value into a byte, inside its declared range.
function quantise(v, i) {
  const [lo, hi] = RANGES[i];
  const t = hi > lo ? (v - lo) / (hi - lo) : 0;
  return Math.max(0, Math.min(255, Math.round(t * 255)));
}

function dequantise(b, i) {
  const [lo, hi] = RANGES[i];
  return lo + (b / 255) * (hi - lo);
}

// Fletcher-16: catches transpositions, which a plain sum does not, and
// transposing two characters is exactly what happens when a code is
// copied out by hand.
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

// encodeScene: version, flags, name (if custom), seed, spec, checksum.
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

// Which kind of code is this? Only scenes for now; kept as a function
// so a future album code has somewhere to land.
export function codeKind(text) {
  const t = String(text || '').trim().toUpperCase();
  if (t.startsWith(PREFIX)) return 'scene';
  return null;
}
