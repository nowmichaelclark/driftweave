//! Driftweave core: the simulations.
//!
//! Five genuinely different simulation models, not one model with five
//! sets of parameters. Body is an N-body gravitational system; Cloth is
//! Verlet with distance constraints projected iteratively; Flock is three
//! steering rules; Sand is a cellular automaton; Flow is a Gray-Scott
//! reaction-diffusion field. They share a spec, a palette, a draw-list
//! format and the wrapping around this file, and nothing else.
//!
//! What leaves is either a *draw list* -- a flat array of ten-float
//! records, one per primitive, in paint order -- or, for Flow, a raster
//! RGBA image. `is_raster()` says which; the caller reads accordingly.
//!
//! No dependencies, no wasm-bindgen, no allocator.

#![allow(static_mut_refs)]

use std::f32::consts::TAU;

// --------------------------------------------------------------- limits

const MAX_P: usize = 1600;
const MAX_L: usize = 8000;
const MAX_PRIM: usize = 6000;
const PRIM: usize = 10;
const NPAL: usize = 5;
const BASE_H: f32 = 1000.0;

const RW: usize = 200;
const RH: usize = 200;
const RD_N: usize = RW * RH;

const SAND_W: usize = 90;
const SAND_H: usize = 90;
const SAND_N: usize = SAND_W * SAND_H;
const CELL_EMPTY: u8 = 0;
const CELL_SAND: u8 = 1;
const CELL_WALL: u8 = 2;

// ------------------------------------------------------------ spec layout

const S_STYLE: usize   = 0;
const S_COUNT: usize   = 1;
const S_GRAV: usize    = 2;
const S_DRAG: usize    = 3;
const S_BOUNCE: usize  = 4;
const S_WIND: usize    = 5;
const S_SWIRL: usize   = 6;
const S_JITTER: usize  = 7;
const S_MUTUAL: usize  = 8;
const S_SPACING: usize = 9;
const S_K: usize       = 10;
const S_FIELDS: usize  = 11;
const S_SIZE: usize    = 12;
const S_TRAIL: usize   = 13;
const S_PULSE: usize   = 14;
const S_TEMPO: usize   = 15;
const S_HUE: usize     = 16;
const S_SAT: usize     = 17;
const S_BRI: usize     = 18;
const S_WALL: usize    = 19;
const S_SUB: usize     = 20;
const S_DENSITY: usize = 21;
const S_SCALE: usize   = 22;
const S_ASPECT: usize  = 23;

const STYLE_MAX: [usize; 5] = [
    420,     // Body
    900,     // Cloth
    460,     // Flock
    SAND_N,  // Sand
    0,       // Flow
];

// ------------------------------------------------------------------- rng

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
    nprim: usize,

    px: [f32; MAX_P], py: [f32; MAX_P],
    vx: [f32; MAX_P], vy: [f32; MAX_P],
    ox: [f32; MAX_P], oy: [f32; MAX_P],
    ax: [f32; MAX_P], ay: [f32; MAX_P],
    pm: [f32; MAX_P],
    ptone: [f32; MAX_P],
    pinned: [u8; MAX_P],

    la: [u32; MAX_L], lb: [u32; MAX_L],
    lrest: [f32; MAX_L], ltone: [f32; MAX_L],

    sand: [u8; SAND_N],
    sand_out: [u8; SAND_N],

    prim: [f32; MAX_PRIM * PRIM],
    pal: [[f32; 3]; NPAL],

    is_raster: u8,
}

impl World {
    const fn new() -> Self {
        World {
            spec: [0.0; 24],
            w: BASE_H, h: BASE_H, t: 0.0, rng: 1,
            np: 0, nl: 0, nprim: 0,
            px: [0.0; MAX_P], py: [0.0; MAX_P],
            vx: [0.0; MAX_P], vy: [0.0; MAX_P],
            ox: [0.0; MAX_P], oy: [0.0; MAX_P],
            ax: [0.0; MAX_P], ay: [0.0; MAX_P],
            pm: [1.0; MAX_P],
            ptone: [0.0; MAX_P],
            pinned: [0; MAX_P],
            la: [0; MAX_L], lb: [0; MAX_L],
            lrest: [0.0; MAX_L], ltone: [0.0; MAX_L],
            sand: [0; SAND_N],
            sand_out: [0; SAND_N],
            prim: [0.0; MAX_PRIM * PRIM],
            pal: [[0.0; 3]; NPAL],
            is_raster: 0,
        }
    }
}

static mut W: World = World::new();

static mut RD_A:  [f32; RD_N] = [0.0; RD_N];
static mut RD_B:  [f32; RD_N] = [0.0; RD_N];
static mut RD_A2: [f32; RD_N] = [0.0; RD_N];
static mut RD_B2: [f32; RD_N] = [0.0; RD_N];
static mut RASTER: [u8; RD_N * 4] = [0; RD_N * 4];

// -------------------------------------------------------------- palette

const RAMP: [(f32, f32, f32); NPAL] = [
    (0.10,  0.000, 0.75),
    (0.24,  0.045, 1.00),
    (0.46,  0.105, 0.85),
    (0.72, -0.055, 0.50),
    (0.95, -0.130, 0.20),
];

fn hsl(h: f32, s: f32, l: f32) -> [f32; 3] {
    let h = h - h.floor();
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let hp = h * 6.0;
    let x = c * (1.0 - ((hp % 2.0) - 1.0).abs());
    let (r, g, b) = match hp as i32 {
        0 => (c, x, 0.0), 1 => (x, c, 0.0), 2 => (0.0, c, x),
        3 => (0.0, x, c), 4 => (x, 0.0, c), _ => (c, 0.0, x),
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

fn pal(w: &World, t: f32, alpha: f32) -> (f32, f32, f32, f32) {
    let t = t.clamp(0.0, 0.9999);
    let x = t * (NPAL - 1) as f32;
    let i = x as usize;
    let f = x - i as f32;
    let a = w.pal[i];
    let b = w.pal[(i + 1).min(NPAL - 1)];
    (a[0] + (b[0] - a[0]) * f,
     a[1] + (b[1] - a[1]) * f,
     a[2] + (b[2] - a[2]) * f,
     alpha.clamp(0.0, 1.0))
}

impl World {
    #[inline]
    fn add_link(&mut self, a: usize, b: usize, stiffness: f32) {
        if self.nl >= MAX_L || a >= self.np || b >= self.np { return; }
        let dx = self.px[b] - self.px[a];
        let dy = self.py[b] - self.py[a];
        let rest = (dx * dx + dy * dy).sqrt().max(0.5);
        let n = self.nl;
        self.la[n] = a as u32;
        self.lb[n] = b as u32;
        self.lrest[n] = rest;
        self.ltone[n] = stiffness;
        self.nl += 1;
    }

    #[inline]
    fn prim_dot(&mut self, x: f32, y: f32, r: f32, g: f32, b: f32, a: f32, size: f32) {
        if self.nprim >= MAX_PRIM { return; }
        let o = self.nprim * PRIM;
        self.prim[o] = 0.0;
        self.prim[o + 1] = x;
        self.prim[o + 2] = y;
        self.prim[o + 5] = r;
        self.prim[o + 6] = g;
        self.prim[o + 7] = b;
        self.prim[o + 8] = a;
        self.prim[o + 9] = size;
        self.nprim += 1;
    }

    #[inline]
    fn prim_line(&mut self, x1: f32, y1: f32, x2: f32, y2: f32,
                 r: f32, g: f32, b: f32, a: f32, width: f32) {
        if self.nprim >= MAX_PRIM { return; }
        let o = self.nprim * PRIM;
        self.prim[o] = 1.0;
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

    #[inline]
    fn prim_rect(&mut self, cx: f32, cy: f32, hw: f32, hh: f32,
                 r: f32, g: f32, b: f32, a: f32) {
        if self.nprim >= MAX_PRIM { return; }
        let o = self.nprim * PRIM;
        self.prim[o] = 2.0;
        self.prim[o + 1] = cx;
        self.prim[o + 2] = cy;
        self.prim[o + 3] = hw;
        self.prim[o + 4] = hh;
        self.prim[o + 5] = r;
        self.prim[o + 6] = g;
        self.prim[o + 7] = b;
        self.prim[o + 8] = a;
        self.prim[o + 9] = 0.0;
        self.nprim += 1;
    }
}

// ============================================================ BODY =========
//
// Chaotic N-body. Every body attracts every other. There is no central
// star; all masses are in a narrow range so no single body dominates the
// dynamics. The initial disk has a little rotation, and gravity then does
// the rest: the cloud collapses toward the centre, the bodies swing past
// each other, some are flung out and wrapped back around, and the whole
// thing churns forever.
//
// This is why there is no drag and no reset: an N-body system with more
// than two similar masses has no stable configuration. It is chaotic by
// nature, and the churn is the picture.

fn body_init(w: &mut World) {
    let n = w.np;
    let cx = w.w * 0.5;
    let cy = w.h * 0.5;
    let r_max = w.w.min(w.h) * 0.42;
    for i in 0..n {
        let a = rnd(&mut w.rng) * TAU;
        let r = rnd(&mut w.rng).sqrt() * r_max;
        w.px[i] = cx + a.cos() * r;
        w.py[i] = cy + a.sin() * r;
        w.ox[i] = w.px[i];
        w.oy[i] = w.py[i];
        // A small tangential velocity with a random direction offset, so
        // the initial disk has some rotation but no orbit is circular.
        let speed = rr(&mut w.rng, 5.0, 40.0);
        let dir = a + TAU * 0.25 + rr(&mut w.rng, -0.9, 0.9);
        w.vx[i] = dir.cos() * speed;
        w.vy[i] = dir.sin() * speed;
        w.pm[i] = rr(&mut w.rng, 8.0, 20.0);
        w.ptone[i] = rnd(&mut w.rng);
    }
}

fn body_step(w: &mut World, dt: f32) {
    let n = w.np;
    if n < 2 { return; }
    // G is tuned so the initial cloud's free-fall time is roughly ten to
    // fifteen seconds: fast enough to be visible, slow enough to read.
    let g = 200.0;
    // Softening keeps two bodies that happen to meet from producing a
    // singularity. Larger than a body's visual radius, so the picture
    // never shows a division by zero either.
    let soft = 22.0;
    let soft2 = soft * soft;

    for i in 0..n {
        w.ax[i] = 0.0;
        w.ay[i] = 0.0;
    }

    // Pairwise gravity. The force is symmetric, so one pass over i<j
    // accumulates both halves.
    for i in 0..n {
        let xi = w.px[i];
        let yi = w.py[i];
        let mi = w.pm[i];
        for j in (i + 1)..n {
            let dx = w.px[j] - xi;
            let dy = w.py[j] - yi;
            let d2 = dx * dx + dy * dy + soft2;
            let inv_d = 1.0 / d2.sqrt();
            let inv_d3 = inv_d / d2;
            let f = g * inv_d3;
            let fj = f * w.pm[j];
            let fi = f * mi;
            w.ax[i] += dx * fj;
            w.ay[i] += dy * fj;
            w.ax[j] -= dx * fi;
            w.ay[j] -= dy * fi;
        }
    }

    for i in 0..n {
        w.vx[i] += w.ax[i] * dt;
        w.vy[i] += w.ay[i] * dt;
        w.ox[i] = w.px[i];
        w.oy[i] = w.py[i];
        w.px[i] += w.vx[i] * dt;
        w.py[i] += w.vy[i] * dt;
    }

    // Toroidal wrap. Nothing accumulates at the edges, and anything
    // ejected comes back to disturb the cluster again -- which is what
    // keeps the system from settling.
    let ww = w.w + 80.0;
    let hh = w.h + 80.0;
    for i in 0..n {
        if w.px[i] < -40.0 { w.px[i] += ww; w.ox[i] += ww; }
        if w.px[i] > w.w + 40.0 { w.px[i] -= ww; w.ox[i] -= ww; }
        if w.py[i] < -40.0 { w.py[i] += hh; w.oy[i] += hh; }
        if w.py[i] > w.h + 40.0 { w.py[i] -= hh; w.oy[i] -= hh; }
    }
}

fn body_emit(w: &mut World) {
    let n = w.np;
    let bri = w.spec[S_BRI];
    let size = w.spec[S_SIZE];
    let trail = w.spec[S_TRAIL];

    // Trails. A body that has just wrapped has moved most of the frame in
    // one step; drawing its trail would smear a line across the picture,
    // so those steps are skipped.
    if trail > 0.02 {
        for i in 0..n {
            let dx = w.px[i] - w.ox[i];
            let dy = w.py[i] - w.oy[i];
            if dx.abs() > w.w * 0.4 || dy.abs() > w.h * 0.4 { continue; }
            if dx * dx + dy * dy < 0.5 { continue; }
            let (r, g, b, a) = pal(w, w.ptone[i], bri * 0.5 * trail);
            w.prim_line(w.ox[i], w.oy[i], w.px[i], w.py[i], r, g, b, a, size * 0.5);
        }
    }

    // Bodies. Colour by current speed: bodies flung around are bright,
    // ones settled near the centre are dim. This is the figure the eye
    // reads as "the system is alive".
    for i in 0..n {
        let sp = (w.vx[i] * w.vx[i] + w.vy[i] * w.vy[i]).sqrt();
        let t = (w.ptone[i] * 0.4 + (sp / 220.0).min(1.0) * 0.6).clamp(0.0, 1.0);
        let (r, g, b, a) = pal(w, t, bri);
        w.prim_dot(w.px[i], w.py[i], r, g, b, a, size * 3.2);
    }
}

// =========================================================== CLOTH =========

fn cloth_init(w: &mut World) {
    let n = w.np;
    if n < 16 { return; }
    let aspect = w.w / w.h;
    let cols = ((n as f32 * aspect).sqrt().ceil() as usize).max(4);
    let rows = (n / cols).max(4);
    let sheet_w = w.w * 0.78;
    let sheet_h = w.h * 0.62;
    let cw = sheet_w / (cols as f32 - 1.0).max(1.0);
    let ch = sheet_h / (rows as f32 - 1.0).max(1.0);
    let x0 = w.w * 0.11;
    let y0 = w.h * 0.16;

    let mut k = 0;
    'outer: for r in 0..rows {
        for c in 0..cols {
            if k >= n { break 'outer; }
            let x = x0 + c as f32 * cw;
            let y = y0 + r as f32 * ch;
            w.px[k] = x; w.py[k] = y;
            w.ox[k] = x; w.oy[k] = y;
            w.vx[k] = 0.0; w.vy[k] = 0.0;
            w.ptone[k] = r as f32 / rows as f32;
            w.pinned[k] = if r == 0 && (c as i32 - (cols as i32) / 2).abs() <= 1 { 1 } else { 0 };
            k += 1;
        }
    }

    for r in 0..rows {
        for c in 0..cols {
            let i = r * cols + c;
            if i >= n { continue; }
            if c + 1 < cols && i + 1 < n { w.add_link(i, i + 1, 1.0); }
            if r + 1 < rows && i + cols < n { w.add_link(i, i + cols, 1.0); }
        }
    }
}

fn cloth_step(w: &mut World, dt: f32) {
    let n = w.np;
    if n == 0 { return; }
    // Gravity, in units per frame at 60fps, scaled the same way the
    // position update below uses h * h. The sheet is ~620 units tall, so
    // this has to stay near single digits or the sheet falls off-screen
    // before the constraints can catch it.
    let g = w.spec[S_GRAV] * 8.0;

    // The wind the scene asked for, plus an intrinsic breeze that never
    // stops. Without the second term, a scene with wind=0 settles within
    // a second or two and the sheet hangs still forever. Two sine terms
    // of different periods, so the pattern does not repeat on a short
    // cycle either.
    let asked = w.spec[S_WIND];
    let breeze = (w.t * 0.42).sin() * 0.55 + (w.t * 1.37).sin() * 0.22;
    let wind = asked * 0.65 + breeze;

    let damp = 1.0 - w.spec[S_DRAG] * 0.08;
    let h = dt * 60.0;
    let hh = h * h;

    for i in 0..n {
        if w.pinned[i] != 0 { continue; }
        let z = w.py[i] * 0.007 + w.t * 1.3;
        let wx = wind * (1.0 + 0.6 * (z).sin()) * 8.0;
        let wy = wind * 0.3 * (z * 0.7 + 1.7).cos() * 8.0;

        let vx = (w.px[i] - w.ox[i]) * damp;
        let vy = (w.py[i] - w.oy[i]) * damp;
        w.ox[i] = w.px[i];
        w.oy[i] = w.py[i];
        w.px[i] += vx + wx * hh;
        w.py[i] += vy + g * hh + wy * hh;
    }

    let iters = (2 + (w.spec[S_K] * 4.0) as i32).clamp(2, 6);
    for _ in 0..iters {
        for l in 0..w.nl {
            let a = w.la[l] as usize;
            let b = w.lb[l] as usize;
            if a >= n || b >= n { continue; }
            let dx = w.px[b] - w.px[a];
            let dy = w.py[b] - w.py[a];
            let d2 = dx * dx + dy * dy;
            if d2 < 1e-6 { continue; }
            let d = d2.sqrt();
            let diff = (d - w.lrest[l]) / d * 0.5;
            let cx = dx * diff;
            let cy = dy * diff;
            if w.pinned[a] == 0 { w.px[a] += cx; w.py[a] += cy; }
            if w.pinned[b] == 0 { w.px[b] -= cx; w.py[b] -= cy; }
        }
    }
}

fn cloth_emit(w: &mut World) {
    let bri = w.spec[S_BRI];
    let size = w.spec[S_SIZE];
    let trail = w.spec[S_TRAIL];

    if trail > 0.05 {
        for i in 0..w.np {
            if w.pinned[i] != 0 { continue; }
            let dx = w.px[i] - w.ox[i];
            let dy = w.py[i] - w.oy[i];
            if dx * dx + dy * dy < 0.5 { continue; }
            let (r, g, b, a) = pal(w, w.ptone[i], bri * 0.22 * trail);
            w.prim_line(w.ox[i], w.oy[i], w.px[i], w.py[i], r, g, b, a, size * 0.4);
        }
    }

    for l in 0..w.nl {
        let a = w.la[l] as usize;
        let b = w.lb[l] as usize;
        if a >= w.np || b >= w.np { continue; }
        let dx = w.px[b] - w.px[a];
        let dy = w.py[b] - w.py[a];
        let d = (dx * dx + dy * dy).sqrt();
        let rest = w.lrest[l].max(0.5);
        let stretch = ((d - rest) / rest).abs().min(1.0);
        let t = (w.ptone[a] * 0.55 + stretch * 0.9).clamp(0.0, 1.0);
        let (cr, cg, cb, ca) = pal(w, t, bri * (0.55 + stretch * 0.4));
        w.prim_line(w.px[a], w.py[a], w.px[b], w.py[b], cr, cg, cb, ca, size * 0.75);
    }
}

// =========================================================== FLOCK =========

fn flock_init(w: &mut World) {
    let n = w.np;
    let cx = w.w * 0.5;
    let cy = w.h * 0.5;
    for i in 0..n {
        let a = rnd(&mut w.rng) * TAU;
        let r = rnd(&mut w.rng).sqrt() * w.w.min(w.h) * 0.32;
        w.px[i] = cx + a.cos() * r;
        w.py[i] = cy + a.sin() * r;
        w.ox[i] = w.px[i];
        w.oy[i] = w.py[i];
        let v = rr(&mut w.rng, 120.0, 220.0);
        let vdir = rnd(&mut w.rng) * TAU;
        w.vx[i] = vdir.cos() * v;
        w.vy[i] = vdir.sin() * v;
        w.ptone[i] = rnd(&mut w.rng);
        w.pm[i] = 1.0;
    }
}

fn flock_step(w: &mut World, dt: f32) {
    let n = w.np;
    if n < 2 { return; }
    let sep_r2 = 42.0_f32 * 42.0;
    let ali_r2 = 110.0_f32 * 110.0;
    let coh_r2 = 170.0_f32 * 170.0;

    let w_sep = 1.7;
    let w_ali = 1.15;
    let w_coh = 0.9;
    let max_speed = 300.0;
    let min_speed = 90.0;
    let max_force = 800.0;

    for i in 0..n {
        let xi = w.px[i];
        let yi = w.py[i];
        let vxi = w.vx[i];
        let vyi = w.vy[i];

        let mut sepx = 0.0; let mut sepy = 0.0;
        let mut alix = 0.0; let mut aliy = 0.0;
        let mut cohx = 0.0; let mut cohy = 0.0;
        let mut n_sep = 0u32;
        let mut n_ali = 0u32;
        let mut n_coh = 0u32;

        for j in 0..n {
            if i == j { continue; }
            let dx = w.px[j] - xi;
            let dy = w.py[j] - yi;
            let d2 = dx * dx + dy * dy;
            if d2 < sep_r2 && d2 > 0.0001 {
                let d = d2.sqrt();
                sepx -= dx / d / d;
                sepy -= dy / d / d;
                n_sep += 1;
            }
            if d2 < ali_r2 {
                alix += w.vx[j];
                aliy += w.vy[j];
                n_ali += 1;
            }
            if d2 < coh_r2 {
                cohx += w.px[j];
                cohy += w.py[j];
                n_coh += 1;
            }
        }

        let mut ax = 0.0f32;
        let mut ay = 0.0f32;

        if n_sep > 0 {
            let m = (sepx * sepx + sepy * sepy).sqrt().max(0.001);
            ax += sepx / m * max_force * w_sep;
            ay += sepy / m * max_force * w_sep;
        }
        if n_ali > 0 {
            alix /= n_ali as f32;
            aliy /= n_ali as f32;
            let m = (alix * alix + aliy * aliy).sqrt().max(0.001);
            let tx = alix / m * max_speed;
            let ty = aliy / m * max_speed;
            ax += (tx - vxi) * w_ali;
            ay += (ty - vyi) * w_ali;
        }
        if n_coh > 0 {
            cohx /= n_coh as f32;
            cohy /= n_coh as f32;
            let dx = cohx - xi;
            let dy = cohy - yi;
            let m = (dx * dx + dy * dy).sqrt().max(0.001);
            let tx = dx / m * max_speed;
            let ty = dy / m * max_speed;
            ax += (tx - vxi) * w_coh;
            ay += (ty - vyi) * w_coh;
        }

        let fm = (ax * ax + ay * ay).sqrt();
        if fm > max_force {
            let s = max_force / fm;
            ax *= s;
            ay *= s;
        }

        w.ax[i] = ax;
        w.ay[i] = ay;
    }

    let drag = (1.0 - w.spec[S_DRAG] * 0.4 * dt).max(0.85);
    for i in 0..n {
        w.vx[i] = (w.vx[i] + w.ax[i] * dt) * drag;
        w.vy[i] = (w.vy[i] + w.ay[i] * dt) * drag;
        let s2 = w.vx[i] * w.vx[i] + w.vy[i] * w.vy[i];
        let s = s2.sqrt();
        if s > max_speed {
            w.vx[i] *= max_speed / s;
            w.vy[i] *= max_speed / s;
        } else if s < min_speed && s > 0.001 {
            w.vx[i] *= min_speed / s;
            w.vy[i] *= min_speed / s;
        }
        w.ox[i] = w.px[i];
        w.oy[i] = w.py[i];
        w.px[i] += w.vx[i] * dt;
        w.py[i] += w.vy[i] * dt;

        let m = 100.0;
        if w.px[i] < m { w.vx[i] += (m - w.px[i]) * 4.0 * dt; }
        if w.px[i] > w.w - m { w.vx[i] -= (w.px[i] - (w.w - m)) * 4.0 * dt; }
        if w.py[i] < m { w.vy[i] += (m - w.py[i]) * 4.0 * dt; }
        if w.py[i] > w.h - m { w.vy[i] -= (w.py[i] - (w.h - m)) * 4.0 * dt; }
    }
}

fn flock_emit(w: &mut World) {
    let bri = w.spec[S_BRI];
    let size = w.spec[S_SIZE];
    let trail = w.spec[S_TRAIL];

    if trail > 0.02 {
        for i in 0..w.np {
            let dx = w.px[i] - w.ox[i];
            let dy = w.py[i] - w.oy[i];
            if dx * dx + dy * dy < 0.5 { continue; }
            let (r, g, b, a) = pal(w, w.ptone[i], bri * 0.45 * trail);
            w.prim_line(w.ox[i], w.oy[i], w.px[i], w.py[i], r, g, b, a, size * 0.5);
        }
    }

    for i in 0..w.np {
        let vx = w.vx[i];
        let vy = w.vy[i];
        let sp = (vx * vx + vy * vy).sqrt().max(1.0);
        let len = size * (1.0 + sp * 0.018);
        let x2 = w.px[i] + vx / sp * len;
        let y2 = w.py[i] + vy / sp * len;
        let (r, g, b, a) = pal(w, w.ptone[i], bri * 0.65);
        w.prim_line(w.px[i], w.py[i], x2, y2, r, g, b, a, size * 0.4);
        w.prim_dot(w.px[i], w.py[i], r, g, b, a, size);
    }
}

// ============================================================ SAND =========
//
// Falling-sand cellular automaton. The obstacle course is built once at
// init; the source at the top emits grains continuously; the bottom of
// the pile is drained continuously. The pile is therefore never in a
// settled state: it grows from the top, shrinks from the bottom, and the
// shape is always in flux.

fn sand_init(w: &mut World) {
    for i in 0..SAND_N { w.sand[i] = CELL_EMPTY; }

    // Source: a horizontal bar with a hole. Grains fall through the hole
    // in a stream.
    let sr = 6;
    let hole_l = SAND_W * 4 / 10;
    let hole_r = SAND_W * 6 / 10;
    for c in 0..SAND_W {
        if c < hole_l || c >= hole_r {
            w.sand[sr * SAND_W + c] = CELL_WALL;
        }
    }

    // Obstacle: a diagonal plate, so the falling stream splits into two
    // and the two streams pile against each other.
    let x0 = SAND_W / 6;
    let y0 = SAND_H / 2 - 10;
    for k in 0..(SAND_W * 2 / 3) {
        let c = x0 + k;
        let r = y0 + k / 3;
        if c < SAND_W && r < SAND_H - 2 {
            w.sand[r * SAND_W + c] = CELL_WALL;
        }
    }

    // Floor.
    for c in 0..SAND_W {
        w.sand[(SAND_H - 1) * SAND_W + c] = CELL_WALL;
    }

    // Some initial sand so the scene starts with motion rather than an
    // empty grid filling in.
    for _ in 0..(SAND_W * 3) {
        let c = (rnd(&mut w.rng) * SAND_W as f32) as usize % SAND_W;
        let r = (rnd(&mut w.rng) * (SAND_H / 3) as f32) as usize;
        let i = r * SAND_W + c;
        if w.sand[i] == CELL_EMPTY { w.sand[i] = CELL_SAND; }
    }
}

fn sand_step(w: &mut World, _dt: f32) {
    for i in 0..SAND_N { w.sand_out[i] = w.sand[i]; }

    let _ = rnd(&mut w.rng);
    let lean_left = rnd(&mut w.rng) < 0.5;

    for r in (0..SAND_H - 1).rev() {
        let row = r * SAND_W;
        let next = (r + 1) * SAND_W;
        if lean_left {
            for c in 0..SAND_W {
                let i = row + c;
                if w.sand[i] != CELL_SAND { continue; }
                let below = next + c;
                if w.sand_out[below] == CELL_EMPTY {
                    w.sand_out[below] = CELL_SAND;
                    w.sand_out[i] = CELL_EMPTY;
                    continue;
                }
                let dl = if c > 0 { next + c - 1 } else { 0 };
                let dr = if c + 1 < SAND_W { next + c + 1 } else { 0 };
                let try_left = c > 0 && w.sand_out[dl] == CELL_EMPTY;
                let try_right = c + 1 < SAND_W && w.sand_out[dr] == CELL_EMPTY;
                if try_left && try_right {
                    let pick = (c * 2654435761) % 2 == 0;
                    let target = if pick { dl } else { dr };
                    w.sand_out[target] = CELL_SAND;
                    w.sand_out[i] = CELL_EMPTY;
                } else if try_left {
                    w.sand_out[dl] = CELL_SAND;
                    w.sand_out[i] = CELL_EMPTY;
                } else if try_right {
                    w.sand_out[dr] = CELL_SAND;
                    w.sand_out[i] = CELL_EMPTY;
                }
            }
        } else {
            for c in (0..SAND_W).rev() {
                let i = row + c;
                if w.sand[i] != CELL_SAND { continue; }
                let below = next + c;
                if w.sand_out[below] == CELL_EMPTY {
                    w.sand_out[below] = CELL_SAND;
                    w.sand_out[i] = CELL_EMPTY;
                    continue;
                }
                let dl = if c > 0 { next + c - 1 } else { 0 };
                let dr = if c + 1 < SAND_W { next + c + 1 } else { 0 };
                let try_left = c > 0 && w.sand_out[dl] == CELL_EMPTY;
                let try_right = c + 1 < SAND_W && w.sand_out[dr] == CELL_EMPTY;
                if (c * 2654435761) % 2 == 0 {
                    if try_left { w.sand_out[dl] = CELL_SAND; w.sand_out[i] = CELL_EMPTY; continue; }
                    if try_right { w.sand_out[dr] = CELL_SAND; w.sand_out[i] = CELL_EMPTY; continue; }
                } else {
                    if try_right { w.sand_out[dr] = CELL_SAND; w.sand_out[i] = CELL_EMPTY; continue; }
                    if try_left { w.sand_out[dl] = CELL_SAND; w.sand_out[i] = CELL_EMPTY; continue; }
                }
            }
        }
    }

    // Emit new grains from the source gap.
    let sr = 7;
    let c0 = SAND_W * 4 / 10;
    let c1 = SAND_W * 6 / 10;
    for c in c0..c1 {
        let i = sr * SAND_W + c;
        if w.sand_out[i] == CELL_EMPTY && rnd(&mut w.rng) < 0.35 {
            w.sand_out[i] = CELL_SAND;
        }
    }

    // Drain the bottom. The cell just above the floor loses sand to a
    // constant trickle, so the pile never stops moving and never fills
    // the frame. Without this the source fills the grid in under a
    // minute and the picture becomes a still life.
    let bottom = (SAND_H - 2) * SAND_W;
    for c in 0..SAND_W {
        if w.sand_out[bottom + c] == CELL_SAND && rnd(&mut w.rng) < 0.05 {
            w.sand_out[bottom + c] = CELL_EMPTY;
        }
    }

    for i in 0..SAND_N { w.sand[i] = w.sand_out[i]; }
}

fn sand_emit(w: &mut World) {
    let bri = w.spec[S_BRI];
    let cw = w.w / SAND_W as f32;
    let ch = w.h / SAND_H as f32;
    let hw = cw * 0.5;
    let hh = ch * 0.5;

    let (wr, wg, wb, _) = pal(w, 0.08, 1.0);

    for r in 0..SAND_H {
        let cy = (r as f32 + 0.5) * ch;
        let row = r * SAND_W;
        for c in 0..SAND_W {
            let cell = w.sand[row + c];
            if cell == CELL_EMPTY { continue; }
            let cx = (c as f32 + 0.5) * cw;
            if cell == CELL_WALL {
                w.prim_rect(cx, cy, hw, hh, wr, wg, wb, 0.9);
            } else {
                let depth = r as f32 / SAND_H as f32;
                let t = (0.35 + depth * 0.55).clamp(0.0, 1.0);
                let j = ((c * 73 + r * 151) % 17) as f32 / 170.0 - 0.05;
                let (sr, sg, sb, _) = pal(w, (t + j).clamp(0.0, 1.0), 1.0);
                w.prim_rect(cx, cy, hw * 0.92, hh * 0.92, sr, sg, sb, bri * 0.95);
            }
        }
    }
}

// ============================================================ FLOW =========
//
// Gray-Scott reaction-diffusion. The pattern is a fixed point if f and k
// are fixed; the whole trick for keeping it moving is to let f and k
// wander. The two wander on different periods, tracing a Lissajous path
// through the region of parameter space that produces spots, worms,
// stripes and back. The pattern is then never allowed to reach a steady
// state; it is always reorganising itself.

fn flow_init(w: &mut World) {
    unsafe {
        for i in 0..RD_N {
            RD_A[i] = 1.0;
            RD_B[i] = 0.0;
        }
        let seeds = 12 + (rnd(&mut w.rng) * 12.0) as usize;
        for _ in 0..seeds {
            let cx = (rnd(&mut w.rng) * RW as f32) as usize;
            let cy = (rnd(&mut w.rng) * RH as f32) as usize;
            let r = 2 + (rnd(&mut w.rng) * 5.0) as usize;
            for dy in 0..(r * 2 + 1) {
                for dx in 0..(r * 2 + 1) {
                    let x = (cx + dx + RW - r) % RW;
                    let y = (cy + dy + RH - r) % RH;
                    let i = y * RW + x;
                    RD_B[i] = 1.0;
                }
            }
        }
    }
}

fn flow_step(w: &mut World, _dt: f32) {
    unsafe {
        // f and k drift on a Lissajous path with periods of ~21s and
        // ~30s. The scene's own grav and wind draw the centre of that
        // path. The result is that the pattern cycles through regimes:
        // spots that grow, worm out, merge into stripes, fragment.
        let phase = w.t * 0.30;
        let f = 0.038 + w.spec[S_GRAV] * 0.014 + phase.sin() * 0.010;
        let k = 0.061 + w.spec[S_WIND] * 0.010 + (phase * 0.7 + 1.1).cos() * 0.0025;

        let da = 1.0;
        let db = 0.5;
        let dt = 1.0;

        for y in 0..RH {
            for x in 0..RW {
                let i = y * RW + x;
                let l = y * RW + ((x + RW - 1) % RW);
                let r = y * RW + ((x + 1) % RW);
                let u = ((y + RH - 1) % RH) * RW + x;
                let d = ((y + 1) % RH) * RW + x;
                let a = RD_A[i];
                let b = RD_B[i];
                let la = RD_A[l] + RD_A[r] + RD_A[u] + RD_A[d] - 4.0 * a;
                let lb = RD_B[l] + RD_B[r] + RD_B[u] + RD_B[d] - 4.0 * b;
                let abb = a * b * b;
                let na = a + (da * la - abb + f * (1.0 - a)) * dt;
                let nb = b + (db * lb + abb - (k + f) * b) * dt;
                RD_A2[i] = na;
                RD_B2[i] = nb;
            }
        }
        for i in 0..RD_N {
            RD_A[i] = RD_A2[i].clamp(0.0, 1.0);
            RD_B[i] = RD_B2[i].clamp(0.0, 1.0);
        }

        for i in 0..RD_N {
            let b = RD_B[i];
            let t = (b * 4.0).clamp(0.0, 1.0);
            let (r, g, bl, _) = pal(w, t, 1.0);
            RASTER[i * 4] = (r * 255.0) as u8;
            RASTER[i * 4 + 1] = (g * 255.0) as u8;
            RASTER[i * 4 + 2] = (bl * 255.0) as u8;
            RASTER[i * 4 + 3] = 255;
        }
    }
}

// ============================================================ SCENE ========

fn scene_init(w: &mut World) {
    let style = (w.spec[S_STYLE] as i32).clamp(0, 4) as usize;
    let frac = w.spec[S_COUNT].clamp(0.0, 1.0);
    let want = ((STYLE_MAX[style] as f32) * (0.25 + frac * 0.75)) as usize;
    let n = want.max(16).min(MAX_P);

    w.np = n;
    w.nl = 0;
    w.nprim = 0;
    w.t = 0.0;
    w.is_raster = 0;

    for i in 0..MAX_P {
        w.pm[i] = 1.0;
        w.pinned[i] = 0;
        w.ptone[i] = rnd(&mut w.rng);
        w.ox[i] = 0.0;
        w.oy[i] = 0.0;
        w.vx[i] = 0.0;
        w.vy[i] = 0.0;
    }

    match style {
        0 => body_init(w),
        1 => cloth_init(w),
        2 => flock_init(w),
        3 => sand_init(w),
        _ => { w.is_raster = 1; flow_init(w); }
    }
}

// --------------------------------------------------------------- exports

#[no_mangle]
pub extern "C" fn spec_ptr() -> *const f32 { unsafe { W.spec.as_ptr() } }

#[no_mangle]
pub extern "C" fn prim_ptr() -> *const f32 { unsafe { W.prim.as_ptr() } }

#[no_mangle]
pub extern "C" fn prim_count() -> u32 { unsafe { W.nprim as u32 } }

#[no_mangle]
pub extern "C" fn is_raster() -> u32 { unsafe { W.is_raster as u32 } }

#[no_mangle]
pub extern "C" fn raster_ptr() -> *const u8 { unsafe { RASTER.as_ptr() } }

#[no_mangle]
pub extern "C" fn raster_w() -> u32 { RW as u32 }

#[no_mangle]
pub extern "C" fn raster_h() -> u32 { RH as u32 }

#[no_mangle]
pub extern "C" fn init(w: f32, h: f32, seed: u32) {
    unsafe {
        let ww = &mut W;
        ww.w = w.max(64.0);
        ww.h = h.max(64.0);
        ww.rng = seed | 1;
        ww.t = 0.0;
        build_palette(ww);
        scene_init(ww);
        if ww.is_raster == 0 {
            match ww.spec[S_STYLE] as usize {
                0 => body_emit(ww),
                1 => cloth_emit(ww),
                2 => flock_emit(ww),
                3 => sand_emit(ww),
                _ => {}
            }
        }
    }
}

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
        ww.w = w;
        ww.h = h;
    }
}

#[no_mangle]
pub extern "C" fn step(dt: f32) {
    unsafe {
        let w = &mut W;
        let dt = dt.clamp(0.0, 0.05);
        let sub = (w.spec[S_SUB].round() as i32).clamp(1, 4) as usize;
        let sdt = dt / sub as f32;
        let style = (w.spec[S_STYLE] as i32).clamp(0, 4) as usize;

        match style {
            0 => { for _ in 0..sub { body_step(w, sdt); } body_emit(w); }
            1 => { for _ in 0..sub { cloth_step(w, sdt); } cloth_emit(w); }
            2 => { for _ in 0..sub { flock_step(w, sdt); } flock_emit(w); }
            3 => { sand_step(w, sdt); sand_emit(w); }
            _ => { flow_step(w, sdt); }
        }

        w.t += dt;
    }
}