// Every scene is one seed. Same seed, same scene, forever -- which is
// what makes a forty-character code enough to carry one.

export function mulberry32(a) {
  return function () {
    a |= 0;
    a = (a + 0x6d2b79f5) | 0;
    let t = Math.imul(a ^ (a >>> 15), 1 | a);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

export class Rng {
  constructor(seed) {
    this.next = mulberry32((seed >>> 0) || 1);
  }
  f() { return this.next(); }
  range(a, b) { return a + this.next() * (b - a); }
  int(a, b) { return Math.floor(a + this.next() * (b - a + 1)); }
  pick(arr) { return arr[Math.floor(this.next() * arr.length)]; }
  chance(p) { return this.next() < p; }
  weighted(items) {
    let total = 0;
    for (const it of items) total += it[1];
    let r = this.next() * total;
    for (const [v, w] of items) { r -= w; if (r <= 0) return v; }
    return items[items.length - 1][0];
  }
}

export function randomSeed() {
  return (Math.random() * 4294967296) >>> 0;
}

const ONSETS = ['k', 's', 't', 'm', 'n', 'h', 'r', 'w', 'y', 'v', 'l', 'd', 'f', 'sh', 'th', 'br', 'gl', 'dr'];
const NUCLEI = ['a', 'e', 'i', 'o', 'u', 'ai', 'ei', 'oa', 'ui', 'ou'];
const CODAS = ['', '', '', 'n', 'm', 'l', 'r', 'sk', 'th', 'ng', 'lm', 'rk'];

export function seedName(seed) {
  const r = new Rng((seed >>> 0) ^ 0x9e3779b9);
  return `${r.pick(ONSETS)}${r.pick(NUCLEI)}${r.pick(CODAS)}-${r.pick(ONSETS)}${r.pick(NUCLEI)}${r.pick(CODAS)}`;
}
