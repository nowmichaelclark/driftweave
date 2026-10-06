// The composer. A scene is which simulation to run, plus the numbers that
// simulation needs, plus a palette.
//
// The five styles are genuinely different models -- see core/src/lib.rs.
// What the spec fields *mean* is different per style, which is why each
// one's draw below is a full override of the shared spec rather than a
// delta to it. The ranges in RANGES are the code format's, not the
// style's; the style decides what its own values should be.

export const STYLES = [
  {
    id: 'body',
    label: 'Body',
    // N-body gravitational. Slow, evolving, no repetition. The disk is
    // seeded slightly sub-circular so it spirals in over a few minutes.
    max: 420,
    hue: [[0.02, 0.10], [0.55, 0.65], [0.78, 0.90]],
    draw(r, spec) {
      spec[S_COUNT]   = r.range(0.35, 1.00);
      spec[S_GRAV]    = 0;                          // gravity is intrinsic
      spec[S_DRAG]    = r.range(0.05, 0.35);
      spec[S_TRAIL]   = r.range(0.35, 0.95);
      spec[S_SIZE]    = r.range(1.6, 3.2);
      spec[S_JITTER]  = 0;
      spec[S_WIND]    = 0;
      spec[S_SWIRL]   = 0;
      spec[S_K]       = 0;
      spec[S_SPACING] = 0;
      spec[S_WALL]    = 1;                          // toroidal
      spec[S_SUB]     = r.int(1, 2);
      spec[S_FIELDS]  = 0;
      spec[S_BOUNCE]  = 0;
      spec[S_MUTUAL]  = 0;
      spec[S_PULSE]   = 0;
      spec[S_TEMPO]   = 1;
    },
  },
  {
    id: 'cloth',
    label: 'Cloth',
    // Verlet sheet, pinned at the top, dropped into wind. Reads as fabric
    // within a second of loading -- a flap that no particle system can
    // imitate.
    max: 900,
    hue: [[0.0, 0.08], [0.28, 0.42], [0.52, 0.62], [0.86, 0.98]],
    draw(r, spec) {
      spec[S_COUNT]   = r.range(0.55, 1.00);
      spec[S_GRAV]    = r.range(0.45, 0.95);
      spec[S_WIND]    = r.range(-0.9, 0.9);
      spec[S_DRAG]    = r.range(0.4, 0.9);
      spec[S_K]       = r.range(0.3, 1.0);
      spec[S_TRAIL]   = r.range(0, 0.35);
      spec[S_SIZE]    = r.range(1.2, 2.4);
      spec[S_BOUNCE]  = 0;
      spec[S_JITTER]  = 0;
      spec[S_SWIRL]   = 0;
      spec[S_SPACING] = 0;
      spec[S_WALL]    = 3;
      spec[S_SUB]     = r.int(2, 3);
      spec[S_FIELDS]  = 0;
      spec[S_MUTUAL]  = 0;
      spec[S_PULSE]   = 0;
      spec[S_TEMPO]   = 1;
    },
  },
  {
    id: 'flock',
    label: 'Flock',
    // Boids. Three rules, no forces. Moves like a living thing and is
    // unmistakably not the N-body simulation, however similar it looks in
    // a still frame.
    max: 460,
    hue: [[0.0, 0.10], [0.32, 0.46], [0.58, 0.68], [0.88, 1.0]],
    draw(r, spec) {
      spec[S_COUNT]   = r.range(0.35, 0.95);
      spec[S_DRAG]    = r.range(0.05, 0.35);
      spec[S_TRAIL]   = r.range(0.15, 0.75);
      spec[S_SIZE]    = r.range(1.6, 3.0);
      spec[S_GRAV]    = 0;
      spec[S_WIND]    = 0;
      spec[S_SWIRL]   = 0;
      spec[S_JITTER]  = 0;
      spec[S_K]       = 0;
      spec[S_SPACING] = 0;
      spec[S_WALL]    = 3;
      spec[S_SUB]     = r.int(1, 2);
      spec[S_FIELDS]  = 0;
      spec[S_BOUNCE]  = 0;
      spec[S_MUTUAL]  = 0;
      spec[S_PULSE]   = 0;
      spec[S_TEMPO]   = 1;
    },
  },
  {
    id: 'sand',
    label: 'Sand',
    // Falling-sand cellular automaton. Discrete, granular, and the only
    // one here where the picture accumulates: over a minute the pile
    // grows and the maze fills in and the shape is different every seed.
    max: 1,
    hue: [[0.06, 0.14], [0.02, 0.08], [0.55, 0.62]],
    draw(r, spec) {
      spec[S_COUNT]   = 1;
      spec[S_TRAIL]   = 0;
      spec[S_SIZE]    = 1;
      spec[S_GRAV]    = 0;
      spec[S_WIND]    = 0;
      spec[S_SWIRL]   = 0;
      spec[S_JITTER]  = 0;
      spec[S_K]       = 0;
      spec[S_SPACING] = 0;
      spec[S_WALL]    = 0;
      spec[S_SUB]     = 1;
      spec[S_FIELDS]  = 0;
      spec[S_BOUNCE]  = 0;
      spec[S_MUTUAL]  = 0;
      spec[S_PULSE]   = 0;
      spec[S_TEMPO]   = 1;
    },
  },
  {
    id: 'flow',
    label: 'Flow',
    // Gray-Scott reaction-diffusion. Not particles at all; the output is
    // a raster. The two parameters wander through the region of (f, k)
    // space that produces wormlike patterns rather than spots or
    // stripes, because worms are the ones that keep moving.
    max: 1,
    hue: [[0.42, 0.56], [0.0, 0.08], [0.72, 0.86], [0.28, 0.42]],
    draw(r, spec) {
      spec[S_GRAV]    = r.range(-0.15, 0.15);
      spec[S_WIND]    = r.range(-0.15, 0.15);
      spec[S_COUNT]   = 1;
      spec[S_TRAIL]   = 0;
      spec[S_SIZE]    = 1;
      spec[S_DR
