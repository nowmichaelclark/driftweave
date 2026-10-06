// Driftweave: the entry point.
//
// The recipe is a 24-float spec and a seed. The browser writes the spec,
// the Rust core simulates it and hands back either a draw list or a
// raster, this file paints it. Everything else -- buttons, sliders, code
// sharing, saves -- is plumbing.

import { loadCore, spec as specView, prims, init, resize, step,
         isRaster, rasterView, rasterW, rasterH } from './core.js';
import { newSpec, sceneOf, encodeScene, decodeScene, codeKind, STYLES, S, RANGES, WALLS } from './scene.js';
import { paint, paintRaster } from './render.js';
import * as store from './storage.js';

const canvas = document.getElementById('view');
const ctx = canvas.getContext('2d');

// World is always 1000 units tall. Width follows the aspect.
const WORLD_H = 1000;

const state = {
  ready: false,
  playing: false,
  scene: null,
  aspect: 1,
  aspectMode: 'fixed',
  worldW: 1000,
  worldH: WORLD_H,
  raf: null,
  lastT: 0,
  fpsAvg: 60,
  history: [],
  historyIndex: -1,
};

// --------------------------------------------------------------- wasm load

async function boot() {
  try {
    await loadCore();
  } catch (err) {
    console.error(err);
    toast(`Could not load the core: ${err.message}`);
    return;
  }
  state.ready = true;
  state.scene = newSpec();
  fitCanvas();
  pushHistory(state.scene);
  applySpec();
  init(state.worldW, state.worldH, state.scene.seed);
  drawOnce();
  renderReadout();
  renderSaved();
  wire();
  requestAnimationFrame(loop);
}

// --------------------------------------------------------------- the frame

function fitCanvas() {
  const dpr = Math.min(2, window.devicePixelRatio || 1);
  const baseH = 720;
  const h = Math.round(baseH * dpr);
  const w = Math.round(baseH * state.aspect * dpr);

  state.worldH = WORLD_H;
  state.worldW = WORLD_H * state.aspect;
  canvas.width = w;
  canvas.height = h;
}

function applySpec() {
  const view = specView();
  const src = state.scene.spec;
  for (let i = 0; i < src.length; i++) view[i] = src[i];
}

function drawOnce() {
  // Two output modes: a draw list of dots/lines/rects for the particle
  // styles, or a raster image for Flow. The core tells us which.
  if (isRaster()) {
    paintRaster(ctx, rasterView(), rasterW(), rasterH(), canvas.width, canvas.height);
  } else {
    paint(ctx, prims(), state.worldW, state.worldH, canvas.width, canvas.height);
  }
}

function loop(now) {
  state.raf = requestAnimationFrame(loop);
  if (!state.playing || !state.ready) return;

  const dt = Math.min(0.05, (now - state.lastT) / 1000 || 0.016);
  state.lastT = now;
  state.fpsAvg = state.fpsAvg * 0.92 + (1 / Math.max(0.001, dt)) * 0.08;

  step(dt);
  drawOnce();
}

// --------------------------------------------------------------- actions

function play() {
  if (!state.ready) return;
  if (state.playing) {
    state.playing = false;
    playBtn.setAttribute('aria-pressed', 'false');
    playLabel.textContent = 'Play';
  } else {
    state.playing = true;
    state.lastT = performance.now();
    playBtn.setAttribute('aria-pressed', 'true');
    playLabel.textContent = 'Stop';
  }
}

function newScene() {
  state.scene = newSpec();
  loadScene(state.scene, { pushToHistory: true });
  toast(`New scene: ${state.scene.name}`);
}

function rerollStyle() {
  const cur = state.scene;
  const next = (cur.style + 1 + Math.floor(Math.random() * (STYLES.length - 1))) % STYLES.length;
  const fresh = newSpec(cur.seed, next);
  fresh.name = cur.name;
  state.scene = fresh;
  loadScene(fresh, { pushToHistory: false, replaceHistory: true });
  toast(`Style: ${STYLES[next].label}`);
}

function loadScene(scene, { pushToHistory = false, replaceHistory = false } = {}) {
  state.scene = scene;
  applySpec();
  init(state.worldW, state.worldH, scene.seed);
  drawOnce();
  renderReadout();
  if (pushToHistory) pushHistory(scene);
  else if (replaceHistory && state.historyIndex >= 0) {
    state.history[state.historyIndex] = cloneScene(scene);
  }
  renderSaved();
}

function cloneScene(s) {
  return { spec: Array.from(s.spec), seed: s.seed, name: s.name, style: s.style };
}

function pushHistory(scene) {
  if (state.historyIndex < state.history.length - 1) {
    state.history = state.history.slice(0, state.historyIndex + 1);
  }
  state.history.push(cloneScene(scene));
  if (state.history.length > 8) state.history.shift();
  state.historyIndex = state.history.length - 1;
}

function goBack() {
  if (state.historyIndex <= 0) return;
  state.historyIndex--;
  loadScene(state.history[state.historyIndex]);
  toast(state.scene.name);
}

function goForward() {
  if (state.historyIndex >= state.history.length - 1) { newScene(); return; }
  state.historyIndex++;
  loadScene(state.history[state.historyIndex]);
  toast(state.scene.name);
}

function setAspect(a, mode) {
  state.aspect = a;
  state.aspectMode = mode;
  fitCanvas();
  resize(state.worldW, state.worldH);
  drawOnce();
  highlightAspect();
}

// -------------------------------------------------------------- rendering

const playBtn = document.getElementById('playBtn');
const playLabel = document.getElementById('playLabel');

function renderReadout() {
  const s = state.scene;
  const style = STYLES[s.style];
  document.getElementById('sceneName').textContent = s.name;
  const sp = s.spec;
  const lines = [
    style.label,
    `${WALLS[Math.round(sp[S.wall])]} walls · ${Math.round(sp[S.count] * 100)}% dense · ${Math.round(sp[S.sub])} substeps`,
    `gravity ${sp[S.grav].toFixed(2)} · swirl ${sp[S.swirl].toFixed(2)} · ${Math.round(state.fpsAvg)} fps`,
  ];
  document.getElementById('sceneDetail').textContent = lines.join('\n');
  document.getElementById('grav').value = sp[S.grav];
  document.getElementById('gravVal').textContent = sp[S.grav].toFixed(2);
  document.getElementById('trail').value = sp[S.trail];
  document.getElementById('trailVal').textContent = sp[S.trail].toFixed(2);
  document.getElementById('count').value = sp[S.count];
  document.getElementById('countVal').textContent = Math.round(sp[S.count] * 100) + '%';
}

function renderSaved() {
  const host = document.getElementById('savedList');
  host.innerHTML = '';
  const list = store.loadAll();
  if (!list.length) {
    const p = document.createElement('p');
    p.className = 'hint';
    p.textContent = 'Nothing saved yet.';
    host.appendChild(p);
    return;
  }
  for (const entry of list) {
    const row = document.createElement('div');
    row.className = 'row';
    const open = document.createElement('button');
    open.className = 'ghost';
    open.textContent = entry.scene.name;
    open.addEventListener('click', () => {
      const spec = new Float32Array(entry.scene.spec);
      const s = { spec, seed: entry.scene.seed, name: entry.scene.name, style: Math.round(spec[S.style]) };
      loadScene(s, { pushToHistory: true });
      if (!state.playing) play();
      toast(`Loaded ${s.name}`);
    });
    const del = document.createElement('button');
    del.className = 'ghost tiny';
    del.textContent = 'delete';
    del.addEventListener('click', () => {
      store.remove(entry.id);
      renderSaved();
    });
    row.append(open, del);
    host.appendChild(row);
  }
}

function highlightAspect() {
  for (const b of document.querySelectorAll('#aspectRow button')) {
    const a = parseFloat(b.dataset.aspect);
    const on = (a === 0 && state.aspectMode === 'fill')
      || (a !== 0 && state.aspectMode === 'fixed' && Math.abs(a - state.aspect) < 0.001);
    b.style.color = on ? 'var(--amber)' : '';
    b.style.borderColor = on ? 'var(--amber)' : '';
  }
}

// ------------------------------------------------------------------ toast

let toastTimer = null;
function toast(msg) {
  const t = document.getElementById('toast');
  t.textContent = msg;
  t.classList.add('show');
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => t.classList.remove('show'), 2200);
}

// -------------------------------------------------------------------- UI

function wire() {
  playBtn.addEventListener('click', play);
  document.getElementById('newBtn').addEventListener('click', newScene);
  document.getElementById('rerollBtn').addEventListener('click', rerollStyle);
  document.getElementById('prevBtn').addEventListener('click', goBack);
  document.getElementById('nextBtn').addEventListener('click', goForward);

  document.getElementById('renameBtn').addEventListener('click', () => {
    const name = prompt('Name this scene', state.scene.name);
    if (!name) return;
    state.scene.name = name.trim().slice(0, 40);
    renderReadout();
    toast('Renamed');
  });

  document.getElementById('grav').addEventListener('input', (e) => {
    const v = parseFloat(e.target.value);
    state.scene.spec[S.grav] = v;
    specView()[S.grav] = v;
    document.getElementById('gravVal').textContent = v.toFixed(2);
  });

  document.getElementById('trail').addEventListener('input', (e) => {
    const v = parseFloat(e.target.value);
    state.scene.spec[S.trail] = v;
    specView()[S.trail] = v;
    document.getElementById('trailVal').textContent = v.toFixed(2);
  });

  document.getElementById('count').addEventListener('input', (e) => {
    const v = parseFloat(e.target.value);
    state.scene.spec[S.count] = v;
    specView()[S.count] = v;
    document.getElementById('countVal').textContent = Math.round(v * 100) + '%';
    init(state.worldW, state.worldH, state.scene.seed);
    drawOnce();
  });

  for (const b of document.querySelectorAll('#aspectRow button')) {
    b.addEventListener('click', () => {
      const a = parseFloat(b.dataset.aspect);
      if (a === 0) {
        const stage = document.querySelector('.stage');
        const r = stage.getBoundingClientRect();
        setAspect(r.width / r.height, 'fill');
      } else {
        setAspect(a, 'fixed');
      }
    });
  }

  document.getElementById('saveBtn').addEventListener('click', () => {
    const entry = store.save(state.scene);
    if (!entry) { toast('Could not save'); return; }
    renderSaved();
    toast(`Saved ${state.scene.name}`);
  });

  document.getElementById('copyCode').addEventListener('click', async () => {
    const code = encodeScene(state.scene);
    const out = document.getElementById('shareOut');
    out.hidden = false;
    out.textContent = code;
    try {
      await navigator.clipboard.writeText(code);
      toast('Code copied');
    } catch {
      toast('Copy it from the box below');
    }
  });

  document.getElementById('pasteCode').addEventListener('click', async () => {
    let text = '';
    try { text = await navigator.clipboard.readText(); } catch { /* fall through */ }
    if (!codeKind(text)) text = prompt('Paste a Driftweave code') || '';
    if (!codeKind(text)) { if (text.trim()) toast('That does not look like a Driftweave code'); return; }
    try {
      const scene = decodeScene(text);
      const spec = new Float32Array(scene.spec);
      const s = { spec, seed: scene.seed, name: scene.name, style: Math.round(spec[S.style]) };
      loadScene(s, { pushToHistory: true });
      if (!state.playing) play();
      toast(`Loaded ${s.name}`);
    } catch (err) {
      toast(err.message || 'Could not read that code');
    }
  });

  document.addEventListener('keydown', (e) => {
    if (e.target.matches('input, textarea')) return;
    if (e.code === 'Space') { e.preventDefault(); play(); }
    else if (e.key === 'n') newScene();
    else if (e.key === 'r') rerollStyle();
  });

  setInterval(() => {
    if (state.scene) renderReadout();
  }, 1000);

  highlightAspect();
}

boot();
