const KEY = 'driftweave.saves.v1';
const PREFS = 'driftweave.prefs.v1';

export function loadAll() {
  try {
    const raw = localStorage.getItem(KEY);
    if (!raw) return [];
    const list = JSON.parse(raw);
    return Array.isArray(list) ? list : [];
  } catch { return []; }
}

function persist(list) {
  try { localStorage.setItem(KEY, JSON.stringify(list)); return true; }
  catch { return false; }
}

export function save(scene) {
  const list = loadAll();
  const entry = {
    id: `${Date.now().toString(36)}${Math.floor(Math.random() * 1e4).toString(36)}`,
    savedAt: new Date().toISOString(),
    scene: {
      spec: Array.from(scene.spec),
      seed: scene.seed >>> 0,
      name: scene.name,
    },
  };
  list.unshift(entry);
  return persist(list) ? entry : null;
}

export function remove(id) {
  const list = loadAll().filter((e) => e.id !== id);
  return persist(list) ? list : null;
}

export function rename(id, name) {
  const list = loadAll();
  const e = list.find((x) => x.id === id);
  if (e) { e.scene.name = name; persist(list); }
  return list;
}

export function getPrefs() {
  try { return JSON.parse(localStorage.getItem(PREFS) || '{}'); }
  catch { return {}; }
}

export function setPrefs(p) {
  try { localStorage.setItem(PREFS, JSON.stringify({ ...getPrefs(), ...p })); }
  catch { /* private mode */ }
}
