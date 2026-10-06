//! Driftweave core: the simulation.
//!
//! The browser composes a recipe -- twenty-four numbers and a seed -- and
//! hands it here. Everything after that happens in this file: every force,
//! every spring, every wall. What leaves is a *draw list*: a flat array of
//! ten-float records, one per primitive, in the order they should be
//! painted.
//!
//! No dependencies, no wasm-bindgen, no allocator. Six exports and one
//! shared memory. The whole thing is about 40 KB and instantiates in a
//! few milliseconds anywhere JavaScript runs.

#![allow(static_mut_refs)]

// --------------------------------------------------------------- limits

const MAX_P: usize = 1600; // particles
const MAX_L: usize = 4800; // springs
const MAX_F: usize = 6;    // moving attractors
const MAX_PRIM: usize = 4000;
const PRIM: usize = 10;    // floats per primitive
const NPAL: usize = 5;     // palette stops

// The world is always this many units tall; the width follows the aspect
// ratio, so every scene is authored at the same scale no matter how the
// viewport is shaped.
const BASE_H: f32 = 1000.0;

// ------------------------------------------------------------ spec layout

const S_STYLE: usize   = 0;
const S_COUNT: usize   = 1;  // 0..1, mapped to a per-style particle count
const S_GRAV: usize    = 2;
const S_DRAG: usize    = 3;
const S_BOUNCE: usize  = 4;
const S_WIND: usize    = 5;
const S_SWIRL: usize   = 6;
const S_JITTER: usize  = 7;
const S_MUTUAL: usize  = 8;
const S_SPACING: usize = 9;
const S_K: usize       = 10; // spring stiffness
const S_FIELDS: usize  = 11;
const S_SIZE: usize    = 12;
const S_TRAIL: usize   = 13;
const S_PULSE: usize   = 14;
const S_TEMPO: usize   = 15;
const S_HUE: usize     = 16;
const S_SAT: usize     = 17;
const S_BRI: usize     = 18;
const S_WALL: usize    = 19; // 0 bounce  1 wrap  2 recycle  3 soft
const S_SUB: usize     = 20; // physics substeps a frame
const S_DENSITY: usize = 21; // live density (may differ from COUNT)
const S_SCALE: usize   = 22; // reserved
const S_ASPECT: usize  = 23;

/// How many particles each style can ask for at full density. The count
/// slider scales these; the world is always allocated for the maximum, so
/// raising the slider only wakes dormant bodies rather than re-seeding.
const STYLE_MAX: [usize; 8] = [900, 420, 900, 520, 240, 700, 900, 420];

// ------------------------------------------------------------------- rng

/// mulberry32, the same generator the rest of the app uses. Deterministic:
/// the same seed always produces the same scene, which is what makes a
/// forty-byte code enough to carry one.
#[inline]
fn rnd(s: &mut u32) -> f32 {
    *s = s.wrapping_add(0x6d2b79f5);
    let mut t = *s;
    t = (t ^ (t >> 15)).wrapping_mul(1 | t);
    t = t.wrapping_add((t ^ (t >> 7)).wrapping_mul(61 | t)) ^ t;
    ((t ^ (t >> 14)) as f32) / 4294967296.0
}

#[inline]
fn rr(s: &mut u32, a: f32, b: f32) -> f32 {
    a + rnd(s) * (b - a)
}

// ------------------------------------------------------------------ world

struct World {
    spec: [f32; 24],
    w: f32,
    h: f32,
    t: f32,
    rng: u32,

    np: usize,
    nl: usize,
    nf: usize,
    nprim: usize,

    px: [f32; MAX_P],
    py: [f32; MAX_P],
    vx: [f32; MAX_P],
    vy: [f32; MAX_P],
    ox: [f32; MAX_P], // previous position, for trails
    oy: [f32; MAX_P],
    pm: [f32; MAX_P], // mass
    ptone: [f32; MAX_P],
    pinned: [u8; MAX_P],

    la: [u32; MAX_L],
    lb: [u32; MAX_L],
    lrest: [f32; MAX_L],
    ltone: [f32; MAX_L],

    fx: [f32; MAX_F],
    fy: [f32; MAX_F],
    fstr: [f32; MAX_F],
    frad: [f32; MAX_F],
    fcx: [f32; MAX_F],
    fcy: [f32; MAX_F],
    frx: [f32; MAX_F],
    fry: [f32; MAX_F],
    fspd: [f32; MAX_F],
    fphase: [f32; MAX_F],
    ftone: [f32; MAX_F],

    prim: [f32; MAX_PRIM * PRIM],
    pal: [[f32; 3]; NPAL],
}

impl World {
    const fn new() -> Self {
        World {
            spec: [0.0; 24],
            w: BASE_H,
            h: BASE_H,
            t: 0.0,
            rng: 1,
            np: 0, nl: 0, nf: 0, nprim: 0,
            px: [0.0; MAX_P], py: [0.0; MAX_P],
            vx: [0.0; MAX_P], vy: [0.0; MAX_P],
            ox: [0.0; MAX_P], oy: [0.0; MAX_P],
            pm: [1.0; MAX_P], ptone: [0.0; MAX_P],
            pinned: [0; MAX_P],
            la: [0; MAX_L], lb: [0; MAX_L],
            lrest: [0.0; MAX_L], ltone: [0.0; MAX_L],
            fx: [0.0; MAX_F], fy: [0.0; MAX_F],
            fstr: [0.0; MAX_F], frad: [0.0; MAX_F],
            fcx: [0.0; MAX_F], fcy: [0.0; MAX_F],
            frx: [0.0; MAX_F], fry: [0.0; MAX_F],
            fspd: [0.0; MAX_F], fphase: [0.0; MAX_F], ftone: [0.0; MAX_F],
            prim: [0.0; MAX_PRIM * PRIM],
            pal: [[0.0; 3]; NPAL],
        }
    }

    #[inline]
    fn add_link(&mut self, a: usize, b: usize) {
        if self.nl >= MAX_L || a >= self.np || b >= self.np { return; }
        let dx = self.px[b] - self.px[a];
        let dy = self.py[b] - self.py[a];
        let rest = (dx * dx + dy * dy).sqrt().max(1.0);
        let n = self.nl;
        self.la[n] = a as u32;
        self.lb[n] = b as u32;
        self.lrest[n] = rest;
        self.ltone[n] = (self.ptone[a] + self.ptone[b]) * 0.5;
        self.nl += 1;
    }

    #[inline]
    fn add_prim(&mut self, kind: f32, x1: f32, y1: f32, x2: f32, y2: f32,
                r: f32, g: f32, b: f32, a: f32, width: f32) {
        if self.nprim >= MAX_PRIM { return; }
        let o = self.nprim * PRIM;
        self.prim[o]     = kind;
        self.prim[o + 1] = x1;
        self.prim[o + 2] = y1;
        self.prim[o + 3] = x2;
        self.prim[o + 4] = y2;
        self.prim[o + 5] = r;
        self.prim[o + 6] = g;
        self.prim[o + 7] = b;
        self.prim[o + 8] = a;
        self.prim[o + 9] = width;
        self.nprim += 1;
    }
}

static mut W: World = World::new();

// --------------------------------------------------------------- palette

/// Lightness, hue offset and saturation scale for each stop of the ramp.
/// The same five stops for every scene; only the hue and saturation move,
/// so two scenes in different keys never look like the same picture.
const RAMP: [(f32, f32, f32); NPAL] = [
    (0.09,  0.000, 0.80),
    (0.22,  0.045, 1.00),
    (0.42,  0.105, 0.85),
    (0.68, -0.055, 0.50),
    (0.93, -0.130, 0.22),
];

fn hsl(h: f32, s: f32, l: f32) -> [f32; 3] {
    let h = h - h.floor();
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let hp = h * 6.0;
    let x = c * (1.0 - ((hp % 2.0) - 1.0).abs());
    let (r, g, b) = match hp as i32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = l - c * 0.5;
    [r + m, g + m, b + m]
}

fn build_palette(w: &mut World) {
    let hue = w.spec[S_HUE];
    let sat = w.spec[S_SAT];
    let bri = w.spec[S_BRI];
    for i in 0..NPAL {
        let (l, dh, ds) = RAMP[i];
        let l2 = (l * (0.55 + bri * 0.75)).clamp(0.02, 0.98);
        w.pal[i] = hsl(hue + dh, (sat * ds).clamp(0.0, 1.0), l2);
    }
}

/// Sample the palette at `t`, mixed with `alpha`. Only three channels are
/// returned because alpha is a separate decision the caller makes.
fn pal(w: &World, t: f32, alpha: f32) -> (f32, f32, f32, f32) {
    let t = t.clamp(0.0, 0.9999);
    let x = t * (NPAL - 1) as f32;
    let i = x as usize;
    let f = x - i as f32;
    let a = w.pal[i];
    let b = w.pal[(i + 1).min(NPAL - 1)];
    (
        a[0] + (b[0] - a[0]) * f,
        a[1] + (b[1] - a[1]) * f,
        a[2] + (b[2] - a[2]) * f,
        alpha.clamp(0.0, 1.0),
    )
}

// ------------------------------------------------------------ scene setup

fn init_scene(w: &mut World) {
    let style = (w.spec[S_STYLE] as i32).clamp(0, 7) as usize;
    let frac = w.spec[S_COUNT].clamp(0.0, 1.0);
    let want = ((STYLE_MAX[style] as f32) * (0.25 + frac * 0.75)) as usize;
    let n = want.max(24).min(MAX_P);

    w.np = n;
    w.nl = 0;
    w.nf = 0;
    w.nprim = 0;
    w.t = 0.0;

    // Every body gets a tone in 0..1. What that means depends on the
    // style: a grid uses it for depth, a swarm for which clump a body
    // belongs to, orbit for its radius.
    for i in 0..MAX_P {
        w.pm[i] = 1.0;
        w.pinned[i] = 0;
        w.ptone[i] = rnd(&mut w.rng);
        w.ox[i] = 0.0;
        w.oy[i] = 0.0;
    }

    let cx = w.w * 0.5;
    let cy = w.h * 0.5;

    match style {
        // 0 -- lattice. A net of springs, sagging under its own weight.
        0 => {
            let cols = ((n as f32 * (w.w / w.h)).sqrt() as usize).max(2);
            let rows = (n / cols).max(2);
            let cw = w.w / (cols as f32 + 1.0);
            let ch = w.h / (rows as f32 + 1.0);
            let mut i = 0;
            'grid: for r in 0..rows {
                for c in 0..cols {
                    if i >= n { break 'grid; }
                    let x = cw * (c as f32 + 1.0);
                    let y = ch * (r as f32 + 1.0);
                    w.px[i] = x; w.py[i] = y;
                    w.vx[i] = 0.0; w.vy[i] = 0.0;
                    w.ox[i] = x; w.oy[i] = y;
                    w.ptone[i] = r as f32 / rows as f32;
                    i += 1;
                }
            }
            for r in 0..rows {
                for c in 0..cols {
                    let i = r * cols + c;
                    if i >= n { continue; }
                    if c + 1 < cols && i + 1 < n { w.add_link(i, i + 1); }
                    if r + 1 < rows && i + cols < n { w.add_link(i, i + cols); }
                }
            }
        }

        // 1 -- orbit. A swarm around a bright centre, each on its own
        // eccentric path.
        1 => {
            for i in 0..n {
                let a = rnd(&mut w.rng) * std::f32::consts::TAU;
                let r = rr(&mut w.rng, 60.0, w.w.min(w.h) * 0.42);
                let x = cx + a.cos() * r;
                let y = cy + a.sin() * r;
                w.px[i] = x; w.py[i] = y;
                w.ox[i] = x; w.oy[i] = y;
                // Tangential velocity: perpendicular to the radius.
                let speed = rr(&mut w.rng, 90.0, 220.0);
                w.vx[i] = -a.sin() * speed;
                w.vy[i] =  a.cos() * speed;
                w.ptone[i] = (r / (w.w.min(w.h) * 0.42)).clamp(0.0, 1.0);
            }
        }

        // 2 -- rain. Sparks that fall, land softly, and are recycled.
        2 => {
            for i in 0..n {
                w.px[i] = rnd(&mut w.rng) * w.w;
                w.py[i] = rr(&mut w.rng, -w.h, 0.0);
                w.ox[i] = w.px[i];
                w.oy[i] = w.py[i];
                w.vx[i] = rr(&mut w.rng, -20.0, 20.0);
                w.vy[i] = rr(&mut w.rng, 0.0, 60.0);
                w.ptone[i] = rnd(&mut w.rng);
            }
        }

        // 3 -- swarm. Mutual pull with a short-range push: a murmuration.
        3 => {
            for i in 0..n {
                let a = rnd(&mut w.rng) * std::f32::consts::TAU;
                let r = rnd(&mut w.rng).sqrt() * w.w.min(w.h) * 0.35;
                w.px[i] = cx + a.cos() * r;
                w.py[i] = cy + a.sin() * r;
                w.ox[i] = w.px[i];
                w.oy[i] = w.py[i];
                w.vx[i] = rr(&mut w.rng, -30.0, 30.0);
                w.vy[i] = rr(&mut w.rng, -30.0, 30.0);
                w.ptone[i] = r / (w.w.min(w.h) * 0.35);
            }
        }

        // 4 -- weave. One long ribbon, pinned at the top, falling.
        4 => {
            let seg = (w.h * 0.55) / n as f32;
            for i in 0..n {
                w.px[i] = cx;
                w.py[i] = (i as f32 + 1.0) * seg;
                w.ox[i] = w.px[i];
                w.oy[i] = w.py[i];
                w.ptone[i] = i as f32 / n as f32;
                if i > 0 { w.add_link(i - 1, i); }
            }
            w.pinned[0] = 1;
            w.px[0] = cx;
            w.py[0] = 40.0;
        }

        // 5 -- bloom. A heartbeat of light: bodies pulled to the centre,
        // thrown out on every pulse, drawn back.
        5 => {
            for i in 0..n {
                let a = rnd(&mut w.rng) * std::f32::consts::TAU;
                let r = rnd(&mut w.rng).sqrt() * 120.0;
                w.px[i] = cx + a.cos() * r;
                w.py[i] = cy + a.sin() * r;
                w.ox[i] = w.px[i];
                w.oy[i] = w.py[i];
                w.vx[i] = 0.0; w.vy[i] = 0.0;
                w.ptone[i] = r / 120.0;
            }
        }

        // 6 -- drift. Ink in water. No gravity, a slow rotating flow.
        6 => {
            for i in 0..n {
                w.px[i] = rnd(&mut w.rng) * w.w;
                w.py[i] = rnd(&mut w.rng) * w.h;
                w.ox[i] = w.px[i];
                w.oy[i] = w.py[i];
                w.vx[i] = 0.0; w.vy[i] = 0.0;
                w.ptone[i] = rnd(&mut w.rng);
            }
        }

        // 7 -- web. Several ribbons, cross-linked at every few segments.
        _ => {
            let strands = 4usize;
            let per = n / strands;
            for s in 0..strands {
                let x0 = w.w * (s as f32 + 1.0) / (strands as f32 + 1.0);
                for k in 0..per {
                    let i = s * per + k;
                    if i >= n { break; }
                    w.px[i] = x0;
                    w.py[i] = (k as f32 + 1.0) * (w.h * 0.7 / per as f32);
                    w.ox[i] = w.px[i];
                    w.oy[i] = w.py[i];
                    w.ptone[i] = (s as f32 + k as f32 / per as f32) / strands as f32;
                    if k > 0 { w.add_link(i - 1, i); }
                    if s > 0 && k % 3 == 0 {
                        let other = (s - 1) * per + k;
                        if other < i { w.add_link(other, i); }
                    }
                }
                if s * per < n { w.pinned[s * per] = 1; }
            }
        }
    }

    // Moving attractors. Common to every style, because even the styles
    // that ignore them by default read better with one or two drifting
    // through the frame.
    let nf = (w.spec[S_FIELDS].round() as i32).clamp(0, MAX_F as i32) as usize;
    w.nf = nf;
    for f in 0..nf {
        let rx = rr(&mut w.rng, w.w * 0.12, w.w * 0.38);
        let ry = rr(&mut w.rng, w.h * 0.10, w.h * 0.34);
        w.fcx[f] = cx + rr(&mut w.rng, -w.w * 0.18, w.w * 0.18);
        w.fcy[f] = cy + rr(&mut w.rng, -w.h * 0.18, w.h * 0.18);
        w.frx[f] = rx;
        w.fry[f] = ry;
        w.frad[f] = rr(&mut w.rng, 90.0, 260.0);
        w.fstr[f] = rr(&mut w.rng, -1.0, 1.0) * 1.4;
        w.fspd[f] = rr(&mut w.rng, 0.10, 0.42) * if rnd(&mut w.rng) < 0.5 { -1.0 } else { 1.0 };
        w.fphase[f] = rnd(&mut w.rng) * std::f32::consts::TAU;
        w.ftone[f] = rnd(&mut w.rng);
        w.fx[f] = w.fcx[f];
        w.fy[f] = w.fcy[f];
    }
}

// ------------------------------------------------------------------ step

fn substep(w: &mut World, dt: f32) {
    let style = (w.spec[S_STYLE] as i32).clamp(0, 7) as usize;
    let grav = w.spec[S_GRAV];
    let wind = w.spec[S_WIND];
    let swirl = w.spec[S_SWIRL];
    let jitter = w.spec[S_JITTER];
    let mutual = w.spec[S_MUTUAL];
    let spacing = w.spec[S_SPACING];
    let k = w.spec[S_K];
    let drag = w.spec[S_DRAG];
    let bounce = w.spec[S_BOUNCE];
    let wall = (w.spec[S_WALL].round() as i32).clamp(0, 3) as usize;

    let n = w.np;
    let cx = w.w * 0.5;
    let cy = w.h * 0.5;

    // --- forces ---------------------------------------------------------
    for i in 0..n {
        if w.pinned[i] != 0 { continue; }

        let mut ax = 0.0f32;
        let mut ay = 0.0f32;

        // Gravity. The drift style is buoyant and ignores it entirely.
        if style != 6 { ay += grav; }

        // Wind, varying a little with height so it reads as a current
        // rather than a conveyor belt.
        ax += wind * (1.0 + 0.35 * (w.py[i] * 0.006).sin());

        // Swirl around the centre: a tangential force that falls off with
        // radius, so the middle turns faster than the edge.
        if swirl != 0.0 {
            let dx = w.px[i] - cx;
            let dy = w.py[i] - cy;
            let r2 = (dx * dx + dy * dy).max(400.0);
            ax += -dy * swirl * 90000.0 / r2;
            ay +=  dx * swirl * 90000.0 / r2;
        }

        // Jitter. A small random kick, for grit.
        if jitter > 0.0 {
            ax += (rnd(&mut w.rng) - 0.5) * jitter * 900.0;
            ay += (rnd(&mut w.rng) - 0.5) * jitter * 900.0;
        }

        // Attractors.
        for f in 0..w.nf {
            let dx = w.fx[f] - w.px[i];
            let dy = w.fy[f] - w.py[i];
            let d2 = dx * dx + dy * dy + 100.0;
            let d = d2.sqrt();
            if d < w.frad[f] * 3.0 {
                // Inverse-square, clamped so nothing explodes at the core.
                let s = w.fstr[f] * 400000.0 / d2;
                ax += dx / d * s;
                ay += dy / d * s;
            }
        }

        // Bloom's own restoring force: a spring to the centre.
        if style == 5 {
            let dx = w.px[i] - cx;
            let dy = w.py[i] - cy;
            ax -= dx * k * 0.020;
            ay -= dy * k * 0.020;
        }

        w.vx[i] += ax * dt;
        w.vy[i] += ay * dt;
    }

    // --- mutual attraction ---------------------------------------------
    //
    // O(n²), so it is only run for the styles that ask for it and only
    // below a ceiling where it stays cheap. Swarm is the only style that
    // sets S_MUTUAL by default; the slider can turn it on anywhere.
    if mutual > 0.001 && n <= 600 {
        let r_in = spacing.max(6.0);
        let r_out = r_in * 4.0;
        for i in 0..n {
            for j in (i + 1)..n {
                let dx = w.px[j] - w.px[i];
                let dy = w.py[j] - w.py[i];
                let d2 = dx * dx + dy * dy + 4.0;
                if d2 > r_out * r_out { continue; }
                let d = d2.sqrt();
                let force = if d < r_in {
                    // Short range: push apart, hard.
                    -r_in * r_in * 60.0 / d2
                } else {
                    // Long range: pull together, gently.
                    mutual * 12.0 * (1.0 - d / r_out)
                };
                let fx = dx / d * force * dt;
                let fy = dy / d * force * dt;
                w.vx[i] += fx; w.vy[i] += fy;
                w.vx[j] -= fx; w.vy[j] -= fy;
            }
        }
    }

    // --- springs -------------------------------------------------------
    if w.nl > 0 {
        let damp = 9.0;
        for l in 0..w.nl {
            let a = w.la[l] as usize;
            let b = w.lb[l] as usize;
            if a >= n || b >= n { continue; }

            let dx = w.px[b] - w.px[a];
            let dy = w.py[b] - w.py[a];
            let d = (dx * dx + dy * dy).sqrt().max(0.5);
            let ux = dx / d;
            let uy = dy / d;

            // Spring, clamped so a tangled frame cannot fling anything.
            let stretch = d - w.lrest[l];
            let f = (k * stretch).clamp(-4000.0, 4000.0);

            // Damping along the link axis only, so a spring resists being
            // stretched without acting as a brake on the whole system.
            let rvx = w.vx[b] - w.vx[a];
            let rvy = w.vy[b] - w.vy[a];
            let along = rvx * ux + rvy * uy;
            let fd = along * damp;
            let ft = (f + fd) * dt;

            if w.pinned[a] == 0 { w.vx[a] += ft * ux; w.vy[a] += ft * uy; }
            if w.pinned[b] == 0 { w.vx[b] -= ft * ux; w.vy[b] -= ft * uy; }
        }
    }

    // --- integrate -----------------------------------------------------
    let dfac = (1.0 - drag * dt).max(0.0);
    for i in 0..n {
        if w.pinned[i] != 0 { continue; }
        w.vx[i] *= dfac;
        w.vy[i] *= dfac;

        // Speed cap. A single runaway body is more distracting than any
        // amount of well-behaved motion is pleasant.
        let sp2 = w.vx[i] * w.vx[i] + w.vy[i] * w.vy[i];
        if sp2 > 640000.0 {
            let s = 800.0 / sp2.sqrt();
            w.vx[i] *= s;
            w.vy[i] *= s;
        }

        w.ox[i] = w.px[i];
        w.oy[i] = w.py[i];
        w.px[i] += w.vx[i] * dt;
        w.py[i] += w.vy[i] * dt;
    }

    // --- walls ---------------------------------------------------------
    for i in 0..n {
        if w.pinned[i] != 0 { continue; }
        match wall {
            // Bounce. Used by the net and the ribbon.
            0 => {
                if w.px[i] < 0.0 { w.px[i] = 0.0; w.vx[i] = -w.vx[i] * bounce; }
                if w.px[i] > w.w { w.px[i] = w.w; w.vx[i] = -w.vx[i] * bounce; }
                if w.py[i] < 0.0 { w.py[i] = 0.0; w.vy[i] = -w.vy[i] * bounce; }
                if w.py[i] > w.h { w.py[i] = w.h; w.vy[i] = -w.vy[i] * bounce; }
            }
            // Wrap. Used by the orbit, so nothing escapes its frame.
            1 => {
                if w.px[i] < 0.0 { w.px[i] += w.w; w.ox[i] += w.w; }
                if w.px[i] > w.w { w.px[i] -= w.w; w.ox[i] -= w.w; }
                if w.py[i] < 0.0 { w.py[i] += w.h; w.oy[i] += w.h; }
                if w.py[i] > w.h { w.py[i] -= w.h; w.oy[i] -= w.h; }
            }
            // Recycle. Rain's wall: anything that leaves the bottom comes
            // back at the top, fresh.
            2 => {
                if w.py[i] > w.h + 40.0 {
                    let s = &mut w.rng;
                    w.px[i] = rr(s, 0.0, w.w);
                    w.py[i] = -30.0;
                    w.ox[i] = w.px[i];
                    w.oy[i] = w.py[i];
                    w.vx[i] = rr(s, -20.0, 20.0);
                    w.vy[i] = rr(s, 0.0, 40.0);
                }
                if w.px[i] < 0.0 { w.px[i] = 0.0; w.vx[i] = -w.vx[i] * bounce; }
                if w.px[i] > w.w { w.px[i] = w.w; w.vx[i] = -w.vx[i] * bounce; }
            }
            // Soft. A spring to the frame, for anything that should stay
            // inside without ever looking like it hit something.
            _ => {
                let m = 80.0;
                if w.px[i] < m { w.vx[i] += (m - w.px[i]) * 6.0 * dt; }
                if w.px[i] > w.w - m { w.vx[i] -= (w.px[i] - (w.w - m)) * 6.0 * dt; }
                if w.py[i] < m { w.vy[i] += (m - w.py[i]) * 6.0 * dt; }
                if w.py[i] > w.h - m { w.vy[i] -= (w.py[i] - (w.h - m)) * 6.0 * dt; }
            }
        }
    }

    // --- attractors move -----------------------------------------------
    for f in 0..w.nf {
        w.fphase[f] += w.fspd[f] * dt;
        let a = w.fphase[f];
        w.fx[f] = w.fcx[f] + a.cos() * w.frx[f];
        w.fy[f] = w.fcy[f] + (a * 1.31).sin() * w.fry[f];
    }

    w.t += dt;
}

/// The bloom's pulse: every `tempo` seconds, every body is thrown outward
/// from the centre. Its restoring spring then draws them back in, which is
/// the whole shape of the scene.
fn bloom_pulse(w: &mut World, dt: f32) {
    let tempo = w.spec[S_TEMPO].max(0.25);
    let now = w.t;
    let before = (now - dt) / tempo;
    let after = now / tempo;
    if after.floor() == before.floor() { return; }

    let pulse = w.spec[S_PULSE];
    let cx = w.w * 0.5;
    let cy = w.h * 0.5;
    for i in 0..w.np {
        let dx = w.px[i] - cx;
        let dy = w.py[i] - cy;
        let d = (dx * dx + dy * dy).sqrt().max(1.0);
        w.vx[i] += dx / d * pulse * 320.0;
        w.vy[i] += dy / d * pulse * 320.0;
    }
}

// ------------------------------------------------------------------ emit

fn emit(w: &mut World) {
    w.nprim = 0;
    let n = w.np;
    let trail = w.spec[S_TRAIL];
    let bri = w.spec[S_BRI];
    let size = w.spec[S_SIZE];

    // Springs first, so they sit behind the bodies.
    for l in 0..w.nl {
        let a = w.la[l] as usize;
        let b = w.lb[l] as usize;
        if a >= n || b >= n { continue; }

        let dx = w.px[b] - w.px[a];
        let dy = w.py[b] - w.py[a];
        let d = (dx * dx + dy * dy).sqrt();
        let stretch = if w.lrest[l] > 0.5 {
            ((d - w.lrest[l]) / w.lrest[l]).abs()
        } else { 0.0 };

        // A stretched link brightens toward the top of the ramp, so the
        // picture shows where the tension is.
        let t = (w.ltone[l] * 0.65 + stretch * 1.6).clamp(0.0, 1.0);
        let (cr, cg, cb, ca) = pal(w, t, 0.55 * bri);
         w.add_prim(1.0, w.px[a], w.py[a], w.px[b], w.py[b], cr, cg, cb, ca, 1.0);
    }

    // Bodies.
    for i in 0..n {
        let sp = (w.vx[i] * w.vx[i] + w.vy[i] * w.vy[i]).sqrt();
        let t = (w.ptone[i] * 0.60 + (sp / 500.0).min(0.40)).clamp(0.0, 1.0);
        let (r, g, b, a) = pal(w, t, bri);

        // Trail: one segment, from where the body was last frame to where
        // it is now. At sixty frames a second that reads as motion blur
        // rather than as a line, which is what a trail should be.
        if trail > 0.02 {
            let dx = w.px[i] - w.ox[i];
            let dy = w.py[i] - w.oy[i];
            if dx * dx + dy * dy > 0.20 {
                let (tr, tg, tb, ta) = pal(w, t, bri * 0.35 * trail);
                w.add_prim(1.0, w.ox[i], w.oy[i], w.px[i], w.py[i],
                           tr, tg, tb, ta, size * 0.55);
            }
        }

        w.add_prim(0.0, w.px[i], w.py[i], 0.0, 0.0, r, g, b, a, size);
    }
}

// -------------------------------------------------------------- exports

/// Where the browser writes the twenty-four spec floats. It writes them
/// first, then calls `init`, which reads them.
#[no_mangle]
pub extern "C" fn spec_ptr() -> *const f32 {
    unsafe { W.spec.as_ptr() }
}

/// The draw list. `prim_count()` records follow, each ten floats:
/// `[kind, x1, y1, x2, y2, r, g, b, a, width]`. `kind` is 0 for a dot at
/// `(x1, y1)` with radius `width`, and 1 for a line from `(x1, y1)` to
/// `(x2, y2)`.
#[no_mangle]
pub extern "C" fn prim_ptr() -> *const f32 {
    unsafe { W.prim.as_ptr() }
}

#[no_mangle]
pub extern "C" fn prim_count() -> u32 {
    unsafe { W.nprim as u32 }
}

/// Build a scene from whatever is in the spec buffer, at this frame size
/// and with this seed.
#[no_mangle]
pub extern "C" fn init(w: f32, h: f32, seed: u32) {
    unsafe {
        let ww = &mut W;
        ww.w = w.max(64.0);
        ww.h = h.max(64.0);
        ww.rng = seed | 1;
        ww.t = 0.0;
        build_palette(ww);
        init_scene(ww);
        emit(ww);
    }
}

/// A new frame size, keeping the scene in progress. Positions are scaled
/// so nothing jumps when the viewport changes shape.
#[no_mangle]
pub extern "C" fn resize(w: f32, h: f32) {
    unsafe {
        let ww = &mut W;
        let w = w.max(64.0);
        let h = h.max(64.0);
        if (ww.w - w).abs() < 0.5 && (ww.h - h).abs() < 0.5 { return; }
        let sx = w / ww.w.max(1.0);
        let sy = h / ww.h.max(1.0);
        for i in 0..ww.np {
            ww.px[i] *= sx;
            ww.py[i] *= sy;
            ww.ox[i] *= sx;
            ww.oy[i] *= sy;
        }
        for f in 0..ww.nf {
            ww.fcx[f] *= sx;
            ww.fcy[f] *= sy;
            ww.frx[f] *= sx;
            ww.fry[f] *= sy;
        }
        ww.w = w;
        ww.h = h;
    }
}

/// Advance the simulation by `dt` seconds and rebuild the draw list.
#[no_mangle]
pub extern "C" fn step(dt: f32) {
    unsafe {
        let w = &mut W;
        let dt = dt.clamp(0.0, 0.05);
        let sub = (w.spec[S_SUB].round() as i32).clamp(1, 4) as usize;
        let sdt = dt / sub as f32;

        let style = (w.spec[S_STYLE] as i32).clamp(0, 7) as usize;
        if style == 5 { bloom_pulse(w, dt); }

        for _ in 0..sub {
            substep(w, sdt);
        }
        emit(w);
    }
}
