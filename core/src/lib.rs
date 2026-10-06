//! Driftweave core: six particle simulations.
//!
//! Every style is a force applied to particles. The output is always the
//! same: an interleaved array of 8 floats per particle -- current x, y;
//! previous x, y (for a trail segment); r, g, b; and a size. The browser
//! paints every style with the same two calls.
//!
//! That uniformity is deliberate. One output format is one path through
//! the code, and one path can be reasoned about completely. The earlier
//! version had draw lists and rasters and rects and three primitive
//! kinds, and that was more surface area than the problem needed.
//!
//! Six styles:
//!
//!  0 Orbit    -- chaotic N-body, similar masses, toroidal
//!  1 Flock    -- three boid rules plus a slowly drifting wind
//!  2 Wells    -- attractors on Lissajous paths, particles bound to them
//!  3 Flow     -- particles following a time-varying curl field
//!  4 Chain    -- spring networks hanging in a varying wind
//!  5 Repel    -- mutual repulsion plus a breathing central force
//!
//! No dependencies. No wasm-bindgen. No allocator.

#![allow(static_mut_refs)]

use std::f32::consts::TAU;

const MAX_P: usize = 1400;
const NSPEC: usize = 16;
const NPAL: usize = 5;
const BASE_H: f32 = 1000.0;
const PFLOATS: usize = 8;
const MAX_LINKS: usize = 4096;

// Spec layout. Must match js/scene.js.
const S_STYLE:  usize = 0;
const S_COUNT:  usize = 1;
const S_SIZE:   usize = 2;
const S_ALPHA:  usize = 3;
const S_SPEED:  usize = 4;
const S_GRAV:   usize = 5;
const S_WIND:   usize = 6;
const S_SWIRL:  usize = 7;
const S_SPIN:   usize = 8;
const S_JITTER: usize = 9;
const S_TRAIL:  usize = 10;
const S_COHERE: usize = 11;
const S_HUE:    usize = 12;
const S_SAT:    usize = 13;
const S_BRI:    usize = 14;
const S_WALLS:  usize = 15;

// Full-density particle counts per style.
const STYLE_MAX: [usize; 6] = [420, 500, 800, 1200, 700, 900];

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

// ----------------------------------------------------------------- state

struct World {
    spec: [f32; NSPEC],
    w: f32,
    h: f32,
    t: f32,
    rng: u32,
    np: usize,

    x:    [f32; MAX_P],
    y:    [f32; MAX_P],
    ox:   [f32; MAX_P],
    oy:   [f32; MAX_P],
    vx:   [f32; MAX_P],
    vy:   [f32; MAX_P],
    mass: [f32; MAX_P],
    pin:  [u8;  MAX_P],
    tone: [f32; MAX_P],

    la:    [u16; MAX_LINKS],
    lb:    [u16; MAX_LINKS],
    lrest: [f32; MAX_LINKS],
    nl:    usize,

    out: [f32; MAX_P * PFLOATS],
    pal: [[f32; 3]; NPAL],
}

impl World {
    const fn new() -> Self {
        World {
            spec: [0.0; NSPEC],
            w: BASE_H, h: BASE_H, t: 0.0, rng: 1, np: 0,
            x:    [0.0; MAX_P], y:    [0.0; MAX_P],
            ox:   [0.0; MAX_P], oy:   [0.0; MAX_P],
            vx:   [0.0; MAX_P], vy:   [0.0; MAX_P],
            mass: [1.0; MAX_P],
            pin:  [0; MAX_P],
            tone: [0.0; MAX_P],
            la:   [0; MAX_LINKS], lb: [0; MAX_LINKS], lrest: [0.0; MAX_LINKS],
            nl: 0,
            out:  [0.0; MAX_P * PFLOATS],
            pal:  [[0.0; 3]; NPAL],
        }
    }
}

static mut W: World = World::new();

// -------------------------------------------------------------- palette

const RAMP: [(f32, f32, f32); NPAL] = [
    (0.08,  0.000, 0.70),
    (0.22,  0.050, 0.95),
    (0.44,  0.115, 0.85),
    (0.72, -0.050, 0.45),
    (0.96, -0.130, 0.18),
];

fn hsl_to_rgb(h: f32, s: f32, l: f32) -> [f32; 3] {
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
        let ll = (l * (0.55 + bri * 0.75)).clamp(0.02, 0.98);
        w.pal[i] = hsl_to_rgb(hue + dh, (sat * ds).clamp(0.0, 1.0), ll);
    }
}

fn pal(w: &World, t: f32) -> (f32, f32, f32) {
    let t = t.clamp(0.0, 0.9999);
    let x = t * (NPAL - 1) as f32;
    let i = x as usize;
    let f = x - i as f32;
    let a = w.pal[i];
    let b = w.pal[(i + 1).min(NPAL - 1)];
    (a[0] + (b[0] - a[0]) * f,
     a[1] + (b[1] - a[1]) * f,
     a[2] + (b[2] - a[2]) * f)
}

impl World {
    #[inline]
    fn add_link(&mut self, a: usize, b: usize) {
        if self.nl >= MAX_LINKS || a >= self.np || b >= self.np { return; }
        let dx = self.x[b] - self.x[a];
        let dy = self.y[b] - self.y[a];
        let rest = (dx * dx + dy * dy).sqrt().max(1.0);
        let n = self.nl;
        self.la[n] = a as u16;
        self.lb[n] = b as u16;
        self.lrest[n] = rest;
        self.nl += 1;
    }
}

// ---------------------------------------------------------------- sanity

// If a particle has wandered into NaN or infinity, reset it to the
// centre with a small random velocity. This is the one thing the earlier
// version did not do, and it is why a single numerical wobble -- an inf
// in a force, a NaN from a division -- would eventually reach every
// particle and stop the picture. Now a bad value is caught the frame it
// appears, and other particles are unaffected.
fn sanitize(w: &mut World) {
    let cx = w.w * 0.5;
    let cy = w.h * 0.5;
    for i in 0..w.np {
        let bad = !w.x[i].is_finite() || !w.y[i].is_finite()
               || !w.vx[i].is_finite() || !w.vy[i].is_finite()
               || w.x[i].abs() > 1.0e6 || w.y[i].abs() > 1.0e6;
        if !bad { continue; }
        w.x[i] = cx + rr(&mut w.rng, -30.0, 30.0);
        w.y[i] = cy + rr(&mut w.rng, -30.0, 30.0);
        w.ox[i] = w.x[i];
        w.oy[i] = w.y[i];
        w.vx[i] = rr(&mut w.rng, -40.0, 40.0);
        w.vy[i] = rr(&mut w.rng, -40.0, 40.0);
    }
}

// =============================================================== ORBIT ===

fn orbit_init(w: &mut World) {
    let n = w.np;
    let cx = w.w * 0.5;
    let cy = w.h * 0.5;
    let rmax = w.w.min(w.h) * 0.40;
    for i in 0..n {
        let a = rnd(&mut w.rng) * TAU;
        let r = rnd(&mut w.rng).sqrt() * rmax + 30.0;
        w.x[i] = cx + a.cos() * r;
        w.y[i] = cy + a.sin() * r;
        w.ox[i] = w.x[i];
        w.oy[i] = w.y[i];
        let sp = rr(&mut w.rng, 15.0, 45.0);
        let dir = a + TAU * 0.25 + rr(&mut w.rng, -0.7, 0.7);
        w.vx[i] = dir.cos() * sp;
        w.vy[i] = dir.sin() * sp;
        w.mass[i] = rr(&mut w.rng, 8.0, 18.0);
        w.tone[i] = rnd(&mut w.rng);
    }
}

fn orbit_step(w: &mut World, dt: f32) {
    let n = w.np;
    if n < 2 { return; }
    let g = 180.0;
    let soft = 18.0;
    let soft2 = soft * soft;

    // Pairwise gravity. Symmetric, so one pass over i<j.
    for i in 0..n {
        let xi = w.x[i];
        let yi = w.y[i];
        let mi = w.mass[i];
        let mut axi = 0.0f32;
        let mut ayi = 0.0f32;
        for j in (i + 1)..n {
            let dx = w.x[j] - xi;
            let dy = w.y[j] - yi;
            let d2 = dx * dx + dy * dy + soft2;
            let inv = 1.0 / d2.sqrt();
            let inv3 = inv / d2;
            let f = g * inv3;
            let fj = f * w.mass[j];
            let fi = f * mi;
            axi += dx * fj;
            ayi += dy * fj;
            w.vx[j] -= dx * fi * dt;
            w.vy[j] -= dy * fi * dt;
        }
        w.vx[i] += axi * dt;
        w.vy[i] += ayi * dt;
    }

    let pad = 40.0;
    for i in 0..n {
        let s2 = w.vx[i] * w.vx[i] + w.vy[i] * w.vy[i];
        if s2 > 400000.0 {
            let s = 632.0 / s2.sqrt();
            w.vx[i] *= s;
            w.vy[i] *= s;
        }
        w.ox[i] = w.x[i];
        w.oy[i] = w.y[i];
        w.x[i] += w.vx[i] * dt;
        w.y[i] += w.vy[i] * dt;
        // Toroidal wrap. On a wrap, snap the previous position too so
        // the trail segment does not draw across the frame.
        let mut wrapped = false;
        if w.x[i] < -pad { w.x[i] += w.w + pad * 2.0; wrapped = true; }
        if w.x[i] > w.w + pad { w.x[i] -= w.w + pad * 2.0; wrapped = true; }
        if w.y[i] < -pad { w.y[i] += w.h + pad * 2.0; wrapped = true; }
        if w.y[i] > w.h + pad { w.y[i] -= w.h + pad * 2.0; wrapped = true; }
        if wrapped { w.ox[i] = w.x[i]; w.oy[i] = w.y[i]; }
    }
}

// =============================================================== FLOCK ===

fn flock_init(w: &mut World) {
    let n = w.np;
    let cx = w.w * 0.5;
    let cy = w.h * 0.5;
    for i in 0..n {
        let a = rnd(&mut w.rng) * TAU;
        let r = rnd(&mut w.rng).sqrt() * w.w.min(w.h) * 0.3;
        w.x[i] = cx + a.cos() * r;
        w.y[i] = cy + a.sin() * r;
        w.ox[i] = w.x[i];
        w.oy[i] = w.y[i];
        let v = rr(&mut w.rng, 100.0, 180.0);
        let d = rnd(&mut w.rng) * TAU;
        w.vx[i] = d.cos() * v;
        w.vy[i] = d.sin() * v;
        w.tone[i] = rnd(&mut w.rng);
    }
}

fn flock_step(w: &mut World, dt: f32) {
    let n = w.np;
    if n < 2 { return; }
    let sep_r2: f32 = 45.0 * 45.0;
    let ali_r2: f32 = 120.0 * 120.0;
    let coh_r2: f32 = 180.0 * 180.0;
    let sep_w = 1.6;
    let ali_w = 1.1;
    let coh_w = 0.9;
    let max_speed = 260.0;
    let min_speed = 80.0;
    let max_force = 700.0;

    // Wind drifts slowly so the flock is never left to align perfectly.
    let phase = w.t * 0.27;
    let wind_x = phase.sin() * 40.0;
    let wind_y = (phase * 1.4 + 1.2).cos() * 35.0;

    for i in 0..n {
        let xi = w.x[i];
        let yi = w.y[i];
        let vxi = w.vx[i];
        let vyi = w.vy[i];
        let mut sepx = 0.0f32; let mut sepy = 0.0f32;
        let mut alix = 0.0f32; let mut aliy = 0.0f32;
        let mut cohx = 0.0f32; let mut cohy = 0.0f32;
        let mut n_sep = 0u32; let mut n_ali = 0u32; let mut n_coh = 0u32;
        for j in 0..n {
            if i == j { continue; }
            let dx = w.x[j] - xi;
            let dy = w.y[j] - yi;
            let d2 = dx * dx + dy * dy;
            if d2 < sep_r2 && d2 > 0.01 {
                let d = d2.sqrt();
                sepx -= dx / (d * d);
                sepy -= dy / (d * d);
                n_sep += 1;
            }
            if d2 < ali_r2 {
                alix += w.vx[j];
                aliy += w.vy[j];
                n_ali += 1;
            }
            if d2 < coh_r2 {
                cohx += w.x[j];
                cohy += w.y[j];
                n_coh += 1;
            }
        }
        let mut ax = 0.0f32;
        let mut ay = 0.0f32;
        if n_sep > 0 {
            let m = (sepx * sepx + sepy * sepy).sqrt().max(0.001);
            ax += sepx / m * max_force * sep_w;
            ay += sepy / m * max_force * sep_w;
        }
        if n_ali > 0 {
            alix /= n_ali as f32;
            aliy /= n_ali as f32;
            let m = (alix * alix + aliy * aliy).sqrt().max(0.001);
            ax += (alix / m * max_speed - vxi) * ali_w;
            ay += (aliy / m * max_speed - vyi) * ali_w;
        }
        if n_coh > 0 {
            cohx /= n_coh as f32;
            cohy /= n_coh as f32;
            let dx = cohx - xi;
            let dy = cohy - yi;
            let m = (dx * dx + dy * dy).sqrt().max(0.001);
            ax += (dx / m * max_speed - vxi) * coh_w;
            ay += (dy / m * max_speed - vyi) * coh_w;
        }
        ax += wind_x;
        ay += wind_y;

        let fm = (ax * ax + ay * ay).sqrt();
        if fm > max_force {
            let s = max_force / fm;
            ax *= s; ay *= s;
        }
        w.vx[i] = vxi + ax * dt;
        w.vy[i] = vyi + ay * dt;
    }

    for i in 0..n {
        let s2 = w.vx[i] * w.vx[i] + w.vy[i] * w.vy[i];
        let s = s2.sqrt();
        if s > max_speed {
            w.vx[i] *= max_speed / s;
            w.vy[i] *= max_speed / s;
        } else if s < min_speed && s > 0.001 {
            w.vx[i] *= min_speed / s;
            w.vy[i] *= min_speed / s;
        }
        w.ox[i] = w.x[i];
        w.oy[i] = w.y[i];
        w.x[i] += w.vx[i] * dt;
        w.y[i] += w.vy[i] * dt;
        let m = 90.0;
        if w.x[i] < m { w.vx[i] += (m - w.x[i]) * 5.0 * dt; }
        if w.x[i] > w.w - m { w.vx[i] -= (w.x[i] - (w.w - m)) * 5.0 * dt; }
        if w.y[i] < m { w.vy[i] += (m - w.y[i]) * 5.0 * dt; }
        if w.y[i] > w.h - m { w.vy[i] -= (w.y[i] - (w.h - m)) * 5.0 * dt; }
    }
}

// =============================================================== WELLS ===

fn wells_init(w: &mut World) {
    let n = w.np;
    for i in 0..n {
        w.x[i] = rnd(&mut w.rng) * w.w;
        w.y[i] = rnd(&mut w.rng) * w.h;
        w.ox[i] = w.x[i];
        w.oy[i] = w.y[i];
        w.vx[i] = 0.0;
        w.vy[i] = 0.0;
        w.tone[i] = rnd(&mut w.rng);
    }
}

fn wells_step(w: &mut World, dt: f32) {
    let n = w.np;
    if n == 0 { return; }
    let cx = w.w * 0.5;
    let cy = w.h * 0.5;
    let spin = w.spec[S_SPIN] * 0.4 + 0.25;
    let speed = w.spec[S_SPEED] * 1.2 + 0.4;
    let rx = w.w * 0.35;
    let ry = w.h * 0.32;
    let t = w.t * speed;

    let mut wx: [f32; 3] = [0.0; 3];
    let mut wy: [f32; 3] = [0.0; 3];
    for k in 0..3 {
        let ph = k as f32 * TAU / 3.0;
        wx[k] = cx + (t * spin + ph).cos() * rx;
        wy[k] = cy + (t * spin * 1.37 + ph * 0.7).sin() * ry;
    }

    let strength = 1_500_000.0;
    for i in 0..n {
        let mut ax = 0.0f32;
        let mut ay = 0.0f32;
        for k in 0..3 {
            let dx = wx[k] - w.x[i];
            let dy = wy[k] - w.y[i];
            let d2 = dx * dx + dy * dy + 900.0;
            let d = d2.sqrt();
            let f = strength / d2;
            // Alternate attract / repel, so particles are trapped
            // between wells instead of piling into one.
            let s = if k % 2 == 0 { f } else { -f * 0.75 };
            ax += dx / d * s;
            ay += dy / d * s;
        }
        let dx = w.x[i] - cx;
        let dy = w.y[i] - cy;
        let r2 = (dx * dx + dy * dy).max(400.0);
        let sw = w.spec[S_SWIRL] * 800_000.0;
        ax += -dy * sw / r2;
        ay +=  dx * sw / r2;

        w.vx[i] += ax * dt;
        w.vy[i] += ay * dt;
    }

    for i in 0..n {
        let s2 = w.vx[i] * w.vx[i] + w.vy[i] * w.vy[i];
        if s2 > 250000.0 {
            let s = 500.0 / s2.sqrt();
            w.vx[i] *= s; w.vy[i] *= s;
        }
        w.ox[i] = w.x[i];
        w.oy[i] = w.y[i];
        w.x[i] += w.vx[i] * dt;
        w.y[i] += w.vy[i] * dt;
        let mut wrapped = false;
        if w.x[i] < 0.0 { w.x[i] += w.w; wrapped = true; }
        if w.x[i] > w.w { w.x[i] -= w.w; wrapped = true; }
        if w.y[i] < 0.0 { w.y[i] += w.h; wrapped = true; }
        if w.y[i] > w.h { w.y[i] -= w.h; wrapped = true; }
        if wrapped { w.ox[i] = w.x[i]; w.oy[i] = w.y[i]; }
    }
}

// ================================================================ FLOW ===

fn flow_init(w: &mut World) {
    let n = w.np;
    for i in 0..n {
        w.x[i] = rnd(&mut w.rng) * w.w;
        w.y[i] = rnd(&mut w.rng) * w.h;
        w.ox[i] = w.x[i];
        w.oy[i] = w.y[i];
        w.vx[i] = 0.0;
        w.vy[i] = 0.0;
        w.tone[i] = rnd(&mut w.rng);
    }
}

fn flow_step(w: &mut World, dt: f32) {
    let n = w.np;
    if n == 0 { return; }
    let t = w.t;
    let speed = w.spec[S_SPEED] * 300.0 + 90.0;
    let scale = 0.004;
    for i in 0..n {
        let a = (w.x[i] * scale + t * 0.5).sin()
              + (w.y[i] * scale * 1.7 - t * 0.31).cos();
        let angle = a * 3.14159;
        let tx = angle.cos() * speed;
        let ty = angle.sin() * speed;
        w.vx[i] += (tx - w.vx[i]) * dt * 2.5;
        w.vy[i] += (ty - w.vy[i]) * dt * 2.5;
        w.ox[i] = w.x[i];
        w.oy[i] = w.y[i];
        w.x[i] += w.vx[i] * dt;
        w.y[i] += w.vy[i] * dt;
        let mut wrapped = false;
        if w.x[i] < 0.0 { w.x[i] += w.w; wrapped = true; }
        if w.x[i] > w.w { w.x[i] -= w.w; wrapped = true; }
        if w.y[i] < 0.0 { w.y[i] += w.h; wrapped = true; }
        if w.y[i] > w.h { w.y[i] -= w.h; wrapped = true; }
        if wrapped { w.ox[i] = w.x[i]; w.oy[i] = w.y[i]; }
    }
}

// =============================================================== CHAIN ===

fn chain_init(w: &mut World) {
    let n = w.np;
    if n < 8 { return; }
    let chains = 5usize;
    let per = n / chains;
    if per < 2 { return; }
    let spacing = w.w / (chains as f32 + 1.0);
    for s in 0..chains {
        let x0 = spacing * (s as f32 + 1.0);
        for k in 0..per {
            let i = s * per + k;
            if i >= n { break; }
            w.x[i] = x0 + (rnd(&mut w.rng) - 0.5) * 6.0;
            w.y[i] = 80.0 + k as f32 * 30.0;
            w.ox[i] = w.x[i];
            w.oy[i] = w.y[i];
            w.tone[i] = k as f32 / per as f32;
            w.pin[i] = if k == 0 { 1 } else { 0 };
            w.mass[i] = 1.0;
            if k > 0 { w.add_link(i - 1, i); }
        }
    }
}

fn chain_step(w: &mut World, dt: f32) {
    let n = w.np;
    if n == 0 { return; }
    let g = 500.0;
    let t = w.t;
    let wind_x = w.spec[S_WIND] * 250.0;
    let wind_y = w.spec[S_WIND] * 60.0;

    for i in 0..n {
        if w.pin[i] != 0 { continue; }
        let lwx = wind_x * (1.0 + 0.5 * (w.y[i] * 0.006 + t * 1.1).sin());
        let lwy = wind_y * (0.5 * (w.y[i] * 0.008 + t * 1.7).cos());
        // A small extra kick that is always present, so wind=0 does not
        // mean the chain goes still.
        let ambient = 80.0 * (t * 0.5 + w.x[i] * 0.01).sin();
        w.vx[i] += (lwx + ambient) * dt;
        w.vy[i] += (g + lwy) * dt;
    }

    let k = 200.0;
    let damp = 3.0;
    for l in 0..w.nl {
        let a = w.la[l] as usize;
        let b = w.lb[l] as usize;
        if a >= n || b >= n { continue; }
        let dx = w.x[b] - w.x[a];
        let dy = w.y[b] - w.y[a];
        let d = (dx * dx + dy * dy).sqrt().max(0.001);
        let ux = dx / d;
        let uy = dy / d;
        let stretch = d - w.lrest[l];
        let rvx = w.vx[b] - w.vx[a];
        let rvy = w.vy[b] - w.vy[a];
        let along = rvx * ux + rvy * uy;
        let ft = (k * stretch + damp * along) * dt;
        if w.pin[a] == 0 { w.vx[a] += ft * ux; w.vy[a] += ft * uy; }
        if w.pin[b] == 0 { w.vx[b] -= ft * ux; w.vy[b] -= ft * uy; }
    }

    for i in 0..n {
        if w.pin[i] != 0 { continue; }
        w.vx[i] *= 0.995;
        w.vy[i] *= 0.995;
        w.ox[i] = w.x[i];
        w.oy[i] = w.y[i];
        w.x[i] += w.vx[i] * dt;
        w.y[i] += w.vy[i] * dt;
        if w.x[i] < 10.0 { w.x[i] = 10.0; w.vx[i] = -w.vx[i] * 0.4; }
        if w.x[i] > w.w - 10.0 { w.x[i] = w.w - 10.0; w.vx[i] = -w.vx[i] * 0.4; }
        if w.y[i] > w.h - 10.0 { w.y[i] = w.h - 10.0; w.vy[i] = -w.vy[i] * 0.4; }
    }
}

// =============================================================== REPEL ===

fn repel_init(w: &mut World) {
    let n = w.np;
    for i in 0..n {
        w.x[i] = rnd(&mut w.rng) * w.w;
        w.y[i] = rnd(&mut w.rng) * w.h;
        w.ox[i] = w.x[i];
        w.oy[i] = w.y[i];
        w.vx[i] = 0.0;
        w.vy[i] = 0.0;
        w.tone[i] = rnd(&mut w.rng);
    }
}

fn repel_step(w: &mut World, dt: f32) {
    let n = w.np;
    if n < 2 { return; }
    let cx = w.w * 0.5;
    let cy = w.h * 0.5;
    // A "breathing" central force that alternates attract / repel with a
    // period of ~6 seconds. Combined with pairwise repulsion, this keeps
    // the cloud from ever settling.
    let pulse = (w.t * 0.33 * TAU).sin();
    let central = pulse * (w.spec[S_COHERE] * 200.0 + 80.0);

    let k = 2400.0;
    let soft = 30.0;
    let soft2 = soft * soft;

    for i in 0..n {
        let xi = w.x[i];
        let yi = w.y[i];
        let mut ax = 0.0f32;
        let mut ay = 0.0f32;
        for j in (i + 1)..n {
            let dx = w.x[j] - xi;
            let dy = w.y[j] - yi;
            let d2 = dx * dx + dy * dy + soft2;
            let inv = 1.0 / d2.sqrt();
            let s = -k * inv / d2;
            ax += dx * s;
            ay += dy * s;
            w.vx[j] -= dx * s * dt;
            w.vy[j] -= dy * s * dt;
        }
        let dx = cx - xi;
        let dy = cy - yi;
        let d = (dx * dx + dy * dy).sqrt().max(1.0);
        ax += dx / d * central;
        ay += dy / d * central;

        w.vx[i] += ax * dt;
        w.vy[i] += ay * dt;
    }

    for i in 0..n {
        let s2 = w.vx[i] * w.vx[i] + w.vy[i] * w.vy[i];
        if s2 > 250000.0 {
            let s = 500.0 / s2.sqrt();
            w.vx[i] *= s; w.vy[i] *= s;
        }
        w.vx[i] *= 0.99;
        w.vy[i] *= 0.99;
        w.ox[i] = w.x[i];
        w.oy[i] = w.y[i];
        w.x[i] += w.vx[i] * dt;
        w.y[i] += w.vy[i] * dt;
        let m = 40.0;
        if w.x[i] < m { w.vx[i] += (m - w.x[i]) * 6.0 * dt; }
        if w.x[i] > w.w - m { w.vx[i] -= (w.x[i] - (w.w - m)) * 6.0 * dt; }
        if w.y[i] < m { w.vy[i] += (m - w.y[i]) * 6.0 * dt; }
        if w.y[i] > w.h - m { w.vy[i] -= (w.y[i] - (w.h - m)) * 6.0 * dt; }
    }
}

// ================================================================= EMIT ===

fn emit(w: &mut World) {
    let size_base = w.spec[S_SIZE] * 6.0 + 1.2;
    let trail = w.spec[S_TRAIL];
    let n = w.np;
    let mut o = 0;
    for i in 0..n {
        let sp = (w.vx[i] * w.vx[i] + w.vy[i] * w.vy[i]).sqrt();
        let t = (w.tone[i] * 0.6 + (sp / 250.0).min(1.0) * 0.4).clamp(0.0, 1.0);
        let (r, g, b) = pal(w, t);

        // Trails. If the trail slider is zero, set prev == current so the
        // painter draws only a dot. If the segment is enormous -- a wrap
        // -- do the same, so no line crosses the frame.
        let (mut ox, mut oy) = (w.ox[i], w.oy[i]);
        if trail <= 0.02 { ox = w.x[i]; oy = w.y[i]; }
        else {
            let dx = w.x[i] - ox;
            let dy = w.y[i] - oy;
            if dx.abs() > w.w * 0.4 || dy.abs() > w.h * 0.4 {
                ox = w.x[i]; oy = w.y[i];
            }
        }

        w.out[o]     = w.x[i];
        w.out[o + 1] = w.y[i];
        w.out[o + 2] = ox;
        w.out[o + 3] = oy;
        w.out[o + 4] = r;
        w.out[o + 5] = g;
        w.out[o + 6] = b;
        w.out[o + 7] = size_base * (0.7 + w.mass[i].min(3.0) * 0.1);
        o += PFLOATS;
    }
}

// --------------------------------------------------------------- exports

#[no_mangle]
pub extern "C" fn spec_ptr() -> *const f32 { unsafe { W.spec.as_ptr() } }

#[no_mangle]
pub extern "C" fn part_ptr() -> *const f32 { unsafe { W.out.as_ptr() } }

#[no_mangle]
pub extern "C" fn part_count() -> u32 { unsafe { W.np as u32 } }

#[no_mangle]
pub extern "C" fn init(w: f32, h: f32, seed: u32) {
    unsafe {
        let ww = &mut W;
        ww.w = w.max(64.0);
        ww.h = h.max(64.0);
        ww.rng = seed | 1;
        ww.t = 0.0;
        ww.nl = 0;

        let style = (ww.spec[S_STYLE].round() as i32).clamp(0, 5) as usize;
        let frac = ww.spec[S_COUNT].clamp(0.0, 1.0);
        let max = STYLE_MAX[style];
        let want = ((max as f32) * (0.30 + frac * 0.70)) as usize;
        ww.np = want.max(20).min(MAX_P);

        for i in 0..MAX_P {
            ww.pin[i] = 0;
            ww.mass[i] = 1.0;
            ww.tone[i] = rnd(&mut ww.rng);
            ww.vx[i] = 0.0;
            ww.vy[i] = 0.0;
            ww.ox[i] = 0.0;
            ww.oy[i] = 0.0;
        }

        build_palette(ww);

        match style {
            0 => orbit_init(ww),
            1 => flock_init(ww),
            2 => wells_init(ww),
            3 => flow_init(ww),
            4 => chain_init(ww),
            _ => repel_init(ww),
        }

        emit(ww);
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
            ww.x[i] *= sx; ww.y[i] *= sy;
            ww.ox[i] *= sx; ww.oy[i] *= sy;
        }
        ww.w = w;
        ww.h = h;
    }
}

#[no_mangle]
pub extern "C" fn step(dt: f32) {
    unsafe {
        let w = &mut W;
        let dt = dt.clamp(0.0, 0.033);
        let style = (w.spec[S_STYLE].round() as i32).clamp(0, 5) as usize;

        match style {
            0 => orbit_step(w, dt),
            1 => flock_step(w, dt),
            2 => wells_step(w, dt),
            3 => flow_step(w, dt),
            4 => chain_step(w, dt),
            _ => repel_step(w, dt),
        }

        sanitize(w);
        emit(w);
        w.t += dt;
    }
}