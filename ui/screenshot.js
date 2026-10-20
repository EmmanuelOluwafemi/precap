'use strict';
const { invoke } = window.__TAURI__.core;
const log = (m) => { try { invoke('log', { msg: String(m) }); } catch (_) {} };
window.onerror = (m, s, l, c) => log(`ss error: ${m} @${l}:${c}`);
window.addEventListener('unhandledrejection', (e) => log('ss unhandled: ' + (e.reason && (e.reason.stack || e.reason.message || e.reason))));

// ---------- helpers ----------
function h(tag, p = {}, ...kids) {
  const m = tag.match(/^([a-z0-9]+)((?:[.#][\w-]+)*)$/i);
  const e = document.createElement(m[1]);
  for (const part of m[2].match(/[.#][\w-]+/g) || []) { if (part[0] === '#') e.id = part.slice(1); else e.classList.add(part.slice(1)); }
  for (const k in p) {
    const v = p[k];
    if (v == null || v === false) continue;
    if (k === 'class') e.className += ' ' + v;
    else if (k === 'html') e.innerHTML = v;
    else if (k === 'style') e.style.cssText = v;
    else if (k.startsWith('on')) e[k] = v;
    else if (k in e) { try { e[k] = v; } catch (_) { e.setAttribute(k, v); } }
    else e.setAttribute(k, v === true ? '' : v);
  }
  for (const c of kids.flat()) if (c != null && c !== false) e.append(c.nodeType ? c : document.createTextNode(c));
  return e;
}
const NS = 'http://www.w3.org/2000/svg';
function sv(tag, a = {}) { const e = document.createElementNS(NS, tag); for (const k in a) e.setAttribute(k, a[k]); return e; }
const ICONS = {
  ptr: '<path d="M4 4l7 17 2.5-7.5L21 11z"/>',
  pen: '<path d="M12 20h9"/><path d="M16.5 3.5a2.1 2.1 0 0 1 3 3L7 19l-4 1 1-4z"/>',
  arrow: '<path d="M5 19 19 5"/><path d="M9 5h10v10"/>',
  rect: '<rect x="3" y="5" width="18" height="14" rx="2"/>',
  mask: '<rect x="3" y="3" width="18" height="18" rx="2"/><path d="M3 9h18M3 15h18M9 3v18M15 3v18"/>',
  circle: '<circle cx="12" cy="12" r="9"/>',
  type: '<path d="M4 7V5h16v2M9 19h6M12 5v14"/>',
  layers: '<path d="m12 3 9 5-9 5-9-5z"/><path d="m3 13 9 5 9-5"/>',
  crop: '<path d="M6 2v14a2 2 0 0 0 2 2h14"/><path d="M18 22V8a2 2 0 0 0-2-2H2"/>',
  layout: '<rect x="3" y="3" width="18" height="18" rx="2"/><path d="M3 12h18M12 3v18"/>',
  chev: '<path d="m6 9 6 6 6-6"/>',
  copy: '<rect x="9" y="9" width="12" height="12" rx="2"/><path d="M5 15V5a2 2 0 0 1 2-2h10"/>',
  save: '<path d="M5 3h11l3 3v13a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2z"/><path d="M7 3v6h8V3M7 21v-7h10v7"/>',
  dots: '<circle class="f" cx="5" cy="12" r="1.6"/><circle class="f" cx="12" cy="12" r="1.6"/><circle class="f" cx="19" cy="12" r="1.6"/>',
  folder: '<path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"/>',
  trash: '<path d="M4 7h16M10 11v6M14 11v6M6 7l1 13h10l1-13M9 7V4h6v3"/>',
  x: '<path d="M18 6 6 18M6 6l12 12"/>',
  minus: '<path d="M5 12h14"/>', plus: '<path d="M5 12h14M12 5v14"/>',
  check: '<path d="m5 12 5 5L20 7"/>',
  grip: '<circle class="f" cx="9" cy="6" r="1.4"/><circle class="f" cx="15" cy="6" r="1.4"/><circle class="f" cx="9" cy="12" r="1.4"/><circle class="f" cx="15" cy="12" r="1.4"/><circle class="f" cx="9" cy="18" r="1.4"/><circle class="f" cx="15" cy="18" r="1.4"/>',
  bg: '<rect x="3" y="3" width="18" height="18" rx="3"/><circle cx="9" cy="9" r="2"/><path d="m21 15-3.1-3.1a2 2 0 0 0-2.8 0L6 21"/>',
  pad: '<rect x="3" y="3" width="18" height="18" rx="2"/><rect x="8" y="8" width="8" height="8" rx="1"/>',
  round: '<path d="M4 20V10a6 6 0 0 1 6-6h10"/>',
  shadow: '<rect x="4" y="4" width="12" height="12" rx="2"/><path d="M8 20h10a2 2 0 0 0 2-2V8"/>',
  border: '<rect x="3" y="3" width="18" height="18" rx="2" stroke-dasharray="3 3"/>',
  upload: '<path d="M12 15V4"/><path d="m7 9 5-5 5 5"/><path d="M5 15v3a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2v-3"/>',
};
const ico = (n, s) => h('span', { html: `<svg class="i" viewBox="0 0 24 24"${s ? ` style="width:${s}px;height:${s}px"` : ''}>${ICONS[n]}</svg>`, style: 'display:contents' });
const clone = (o) => JSON.parse(JSON.stringify(o));
const clamp = (v, a, b) => Math.min(b, Math.max(a, v));
const toBuf = (b) => (b instanceof ArrayBuffer ? b : new Uint8Array(b));
const uid = () => (crypto.randomUUID ? crypto.randomUUID() : 'a' + Date.now().toString(36) + Math.random().toString(36).slice(2));
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const hex2 = (n) => clamp(Math.round(n), 0, 255).toString(16).padStart(2, '0');
const toHex = (rgb) => '#' + rgb.map(hex2).join('').toUpperCase();
const fromHex = (s) => [1, 3, 5].map((i) => parseInt(s.slice(i, i + 2), 16));
const rgbCss = (c) => `rgb(${c[0]},${c[1]},${c[2]})`;
const PALETTE = ['#FF0000', '#FF4500', '#FF8C00', '#FFD700', '#FFFF00', '#ADFF2F', '#32CD32', '#008000', '#00CED1', '#4785FF', '#0000FF', '#4B0082', '#800080', '#A9A9A9', '#FFFFFF', '#000000'];
const GRADS = [[[15,52,67],[52,232,158]],[[34,193,195],[253,187,45]],[[29,253,251],[195,29,253]],[[69,104,220],[176,106,179]],[[106,130,251],[252,92,125]],[[131,58,180],[253,29,29]],[[249,212,35],[255,78,80]],[[255,94,0],[255,42,104]],[[255,0,150],[0,204,255]],[[0,242,96],[5,117,230]],[[238,205,163],[239,98,159]],[[44,62,80],[52,152,219]],[[168,239,255],[238,205,163]],[[74,0,224],[143,0,255]],[[252,74,26],[247,183,51]],[[0,255,255],[255,20,147]],[[255,127,0],[255,255,0]],[[255,0,255],[0,255,0]]];
const ASPECTS = [[null, 'Auto', ''], ['wide', 'Wide', '16:9'], ['vertical', 'Vertical', '9:16'], ['square', 'Square', '1:1'], ['classic', 'Classic', '4:3'], ['tall', 'Tall', '3:4']];
const TOOLS = [['select', 'Select', 'V', 'ptr'], ['draw', 'Draw', 'D', 'pen'], ['arrow', 'Arrow', 'A', 'arrow'], ['rectangle', 'Rectangle', 'R', 'rect'], ['mask', 'Mask', 'M', 'mask'], ['circle', 'Circle', 'C', 'circle'], ['text', 'Text', 'T', 'type']];
const TNAME = { draw: 'Draw', arrow: 'Arrow', rectangle: 'Rectangle', mask: 'Mask', circle: 'Circle', text: 'Text' };
const TICON = { draw: 'pen', arrow: 'arrow', rectangle: 'rect', mask: 'mask', circle: 'circle', text: 'type' };

// ---------- state ----------
const S = { cfg: null, name: '', ow: 0, oh: 0, lay: null, bmp: null, zoom: 1, pan: { x: 0, y: 0 }, scale: 1, tool: 'select', sel: null,
  pop: null, layers: false, undo: [], redo: [], editing: null, busy: false, again: false, deleted: false, clip: null, lastDraw: null };
let el = {};

// ---------- toast ----------
let toastT;
function toast(msg) {
  let t = document.getElementById('toast');
  if (!t) { t = h('div#toast'); document.body.append(t); }
  t.textContent = msg; t.hidden = false; clearTimeout(toastT);
  toastT = setTimeout(() => { t.hidden = true; }, 3000);
}

// ---------- history ----------
const snapshot = () => ({ cfg: JSON.stringify(S.cfg), lay: S.lay });
function commitSnap(s) { S.undo.push(s); if (S.undo.length > 100) S.undo.shift(); S.redo = []; }
const push = () => commitSnap(snapshot());
function restore(s) {
  S.cfg = JSON.parse(s.cfg); S.lay = s.lay;
  if (S.sel && !S.cfg.annotations.some((a) => a.id === S.sel)) S.sel = null;
  refreshAll(); changed();
}
function undo() { if (!S.undo.length) return; S.redo.push(snapshot()); restore(S.undo.pop()); }
function redo() { if (!S.redo.length) return; S.undo.push(snapshot()); restore(S.redo.pop()); }

// ---------- persistence + render loop ----------
let saveT;
function scheduleSave() { clearTimeout(saveT); saveT = setTimeout(saveNow, 1000); }
function saveNow() { clearTimeout(saveT); if (S.deleted || !S.cfg) return Promise.resolve(); return invoke('ss_save', { config: S.cfg }).catch((e) => log('ss_save: ' + e)); }
window.addEventListener('beforeunload', () => { saveNow(); });
function changed() { scheduleSave(); sync(); }

function remap(o, n) {
  if (!o || !S.cfg.annotations.length) return;
  const a = o.image, b = n.image;
  if (a.x === b.x && a.y === b.y && a.w === b.w && a.h === b.h) return;
  const sx = b.w / a.w, sy = b.h / a.h;
  for (const t of S.cfg.annotations) {
    t.x = b.x + (t.x - a.x) * sx; t.y = b.y + (t.y - a.y) * sy;
    t.width *= sx; t.height *= sy;
  }
}
async function sync() {
  if (S.busy) { S.again = true; return; }
  S.busy = true;
  try {
    do {
      S.again = false;
      const cfg = clone(S.cfg);
      const lay = await invoke('ss_layout', { config: cfg });
      remap(S.lay, lay); S.lay = lay;
      const maxWidth = Math.min(lay.frameW, Math.max(64, Math.ceil(el.stage.clientWidth * (window.devicePixelRatio || 1))));
      const buf = await invoke('ss_frame', { config: cfg, maxWidth });
      S.bmp = await createImageBitmap(new Blob([toBuf(buf)], { type: 'image/png' }));
      drawFrame();
    } while (S.again);
  } catch (e) { toast(String(e)); }
  S.busy = false;
}
async function settle() { while (S.busy) await sleep(30); }

function drawFrame() {
  const b = S.bmp; if (!b) return;
  for (const c of [el.cv, el.mk]) { c.width = b.width; c.height = b.height; }
  const x = el.cv.getContext('2d'); x.clearRect(0, 0, b.width, b.height); x.drawImage(b, 0, 0);
  drawMasks(); applyView(); renderSvg(); renderLayers(); el.frame.style.visibility = 'visible';
}
function drawMasks() {
  if (!S.bmp) return;
  const x = el.mk.getContext('2d'); x.clearRect(0, 0, el.mk.width, el.mk.height);
  const k = S.bmp.width / S.lay.frameW;
  for (const a of S.cfg.annotations) if (a.type === 'mask') drawMask(x, S.bmp, a, k);
}
function drawMask(dst, src, a, k) {
  const x = a.x * k, y = a.y * k, w = a.width * k, hh = a.height * k;
  if (w < 1 || hh < 1) return;
  const lvl = a.maskLevel || 16, blur = a.maskType === 'blur';
  const d = Math.max(1, (blur ? Math.max(2, Math.round(lvl / 4)) : Math.max(2, Math.round(lvl))) * k);
  const t = document.createElement('canvas'); t.width = Math.max(1, Math.ceil(w / d)); t.height = Math.max(1, Math.ceil(hh / d));
  const tc = t.getContext('2d'); tc.imageSmoothingEnabled = blur; tc.drawImage(src, x, y, w, hh, 0, 0, t.width, t.height);
  dst.imageSmoothingEnabled = blur; dst.drawImage(t, 0, 0, t.width, t.height, x, y, w, hh);
}

// ---------- view (zoom / pan) ----------
function applyView() {
  if (!S.lay) return;
  const sw = el.stage.clientWidth, sh = el.stage.clientHeight, W = S.lay.frameW, H = S.lay.frameH;
  const fit = Math.max(0.01, Math.min((sw - 40) / W, (sh - 40) / H));
  S.scale = fit * S.zoom;
  const w = W * S.scale, hh = H * S.scale;
  Object.assign(el.frame.style, { width: w + 'px', height: hh + 'px', left: (sw - w) / 2 + S.pan.x + 'px', top: (sh - hh) / 2 + S.pan.y + 'px' });
  el.frame.classList.toggle('sh', S.zoom <= 1);
  el.ov.setAttribute('viewBox', `0 0 ${W} ${H}`);
  el.zr.value = S.zoom; el.zr.style.setProperty('--p', ((S.zoom - 0.1) / 2.9) * 100 + '%'); el.zr.title = Math.round(S.zoom * 100) + '%';
  placeOcr();
  if (S.editing) placeEdit();
}
function setZoom(z, cx, cy) {
  z = clamp(Math.round(z * 100) / 100, 0.1, 3);
  if (cx != null) { const r = el.stage.getBoundingClientRect(); const dx = cx - r.left - r.width / 2, dy = cy - r.top - r.height / 2, k = z / S.zoom; S.pan.x = dx - (dx - S.pan.x) * k; S.pan.y = dy - (dy - S.pan.y) * k; }
  S.zoom = z; applyView(); renderSvg();
}
const pt = (e) => { const r = el.frame.getBoundingClientRect(); return { x: (e.clientX - r.left) / r.width * S.lay.frameW, y: (e.clientY - r.top) / r.height * S.lay.frameH }; };
function drag(onMove, onUp) {
  const mv = (e) => onMove(e);
  const up = (e) => { window.removeEventListener('pointermove', mv); window.removeEventListener('pointerup', up); window.removeEventListener('pointercancel', up); onUp && onUp(e); };
  window.addEventListener('pointermove', mv); window.addEventListener('pointerup', up); window.addEventListener('pointercancel', up);
}
function startPan(e) {
  const sx = e.clientX, sy = e.clientY, p0 = { ...S.pan }; el.stage.style.cursor = 'grabbing';
  drag((ev) => { S.pan = { x: p0.x + ev.clientX - sx, y: p0.y + ev.clientY - sy }; applyView(); }, () => { el.stage.style.cursor = S.tool === 'select' ? '' : 'crosshair'; });
}

// ---------- OCR text layer (invisible, selectable) ----------
function setOcr(lines) {
  el.ocr.replaceChildren(...(lines || []).filter((l) => l && l.text && l.bounds).map((l) => { const e = h('span', {}, l.text); e.b = l.bounds; return e; }));
  placeOcr();
}
function placeOcr() {
  if (!el.ocr || !S.lay || !el.ocr.firstChild) return;
  const c = S.cfg.background.crop, cr = c ? { x: c.position.x, y: c.position.y, w: c.size.x, h: c.size.y } : { x: 0, y: 0, w: S.ow, h: S.oh };
  const im = S.lay.image, kx = im.w / cr.w * S.scale, ky = im.h / cr.h * S.scale;
  for (const e of el.ocr.children) {
    const b = e.b, cx = b.x + b.width / 2, cy = b.y + b.height / 2;
    if (cx < cr.x || cx > cr.x + cr.w || cy < cr.y || cy > cr.y + cr.h) { e.hidden = true; continue; }
    e.hidden = false;
    const hh = b.height * ky, fs = Math.max(1, hh * 0.78);
    if (e.fs !== fs) { e.fs = fs; e.style.fontSize = fs + 'px'; e.nat = measure(e.textContent, fs, 1); }
    Object.assign(e.style, { left: (im.x * S.scale + (b.x - cr.x) * kx) + 'px', top: (im.y * S.scale + (b.y - cr.y) * ky) + 'px', height: hh + 'px', lineHeight: hh + 'px', transform: `scaleX(${b.width * kx / e.nat})` });
  }
}

// ---------- annotation geometry ----------
const bbox = (a) => a.type === 'arrow'
  ? { x: Math.min(a.x, a.x + a.width), y: Math.min(a.y, a.y + a.height), w: Math.abs(a.width), h: Math.abs(a.height) }
  : { x: a.x, y: a.y, w: a.width, h: a.height };
function arrowGeom(a) {
  const ex = a.x + a.width, ey = a.y + a.height, len = Math.hypot(a.width, a.height) || 1;
  const L = Math.min(Math.max(20, a.strokeWidth * 6), len), W = Math.max(14, a.strokeWidth * 5);
  const ux = a.width / len, uy = a.height / len, bx = ex - ux * L, by = ey - uy * L;
  return { line: [a.x, a.y, bx, by], tri: [[ex, ey], [bx - uy * W / 2, by + ux * W / 2], [bx + uy * W / 2, by - ux * W / 2]] };
}
function drawPath(a) {
  const p = (a.points || []).map((q) => [a.x + q[0] * a.width, a.y + q[1] * a.height]);
  if (p.length < 2) return p.length ? `M${p[0]} L${p[0][0] + .01},${p[0][1]}` : '';
  let d = `M${p[0]}`;
  for (let i = 1; i < p.length - 1; i++) d += ` Q${p[i]} ${(p[i][0] + p[i + 1][0]) / 2},${(p[i][1] + p[i + 1][1]) / 2}`;
  return d + ` L${p[p.length - 1]}`;
}
function setDrawPts(a, raw) {
  const xs = raw.map((p) => p[0]), ys = raw.map((p) => p[1]);
  a.x = Math.min(...xs); a.y = Math.min(...ys); a.width = Math.max(...xs) - a.x; a.height = Math.max(...ys) - a.y;
  a.points = raw.map((p) => [(p[0] - a.x) / (a.width || 1), (p[1] - a.y) / (a.height || 1)]);
}
const stroke = (a) => (a.strokeColor === 'transparent' || !a.strokeWidth ? 'none' : a.strokeColor);
const fillOf = (a) => (!a.fillColor || a.fillColor === 'transparent' ? 'none' : a.fillColor);

// ---------- svg overlay ----------
function renderSvg() {
  if (!el.ov || !S.lay) return;
  const o = el.ov; o.replaceChildren();
  const sel = S.tool === 'select', pe = sel ? 'all' : 'none';
  for (const a of S.cfg.annotations) {
    const g = sv('g', { opacity: a.type === 'mask' ? 1 : a.opacity ?? 1 });
    if (a.id === S.editing) g.setAttribute('opacity', 0);
    const hit = (shape) => { shape.style.pointerEvents = pe; shape.style.cursor = 'move'; shape.addEventListener('pointerdown', (e) => annDown(e, a)); g.append(shape); };
    const common = { stroke: stroke(a), 'stroke-width': a.strokeWidth, fill: fillOf(a) };
    if (a.type === 'rectangle') { g.append(sv('rect', { x: a.x, y: a.y, width: a.width, height: a.height, ...common })); hit(sv('rect', { x: a.x, y: a.y, width: a.width, height: a.height, fill: 'transparent' })); }
    else if (a.type === 'circle') { g.append(sv('ellipse', { cx: a.x + a.width / 2, cy: a.y + a.height / 2, rx: a.width / 2, ry: a.height / 2, ...common })); hit(sv('rect', { x: a.x, y: a.y, width: a.width, height: a.height, fill: 'transparent' })); }
    else if (a.type === 'mask') hit(sv('rect', { x: a.x, y: a.y, width: a.width, height: a.height, fill: 'transparent', stroke: a.id === S.sel ? '#3b82f6' : 'none', 'stroke-dasharray': '6 4', 'stroke-width': 2 / S.scale }));
    else if (a.type === 'arrow') {
      const gm = arrowGeom(a), c = a.strokeColor === 'transparent' ? 'none' : a.strokeColor;
      g.append(sv('line', { x1: gm.line[0], y1: gm.line[1], x2: gm.line[2], y2: gm.line[3], stroke: c, 'stroke-width': a.strokeWidth, 'stroke-linecap': 'round' }));
      g.append(sv('polygon', { points: gm.tri.join(' '), fill: c }));
      hit(sv('line', { x1: a.x, y1: a.y, x2: a.x + a.width, y2: a.y + a.height, stroke: 'transparent', 'stroke-width': Math.max(a.strokeWidth, 12 / S.scale), 'stroke-linecap': 'round' }));
    } else if (a.type === 'draw') {
      const d = drawPath(a);
      g.append(sv('path', { d, fill: 'none', stroke: stroke(a), 'stroke-width': a.strokeWidth, 'stroke-linecap': 'round', 'stroke-linejoin': 'round' }));
      hit(sv('path', { d, fill: 'none', stroke: 'transparent', 'stroke-width': Math.max(a.strokeWidth, 12 / S.scale), 'stroke-linecap': 'round' }));
    } else if (a.type === 'text') {
      const t = sv('text', { x: a.x, y: a.y + a.height, 'font-size': a.height, 'font-family': 'sans-serif', fill: a.strokeColor === 'transparent' ? 'none' : a.strokeColor });
      t.textContent = a.text || ''; g.append(t);
      hit(sv('rect', { x: a.x, y: a.y, width: a.width, height: a.height, fill: 'transparent' }));
      g.lastChild.addEventListener('dblclick', () => startEdit(a));
    }
    o.append(g);
  }
  const a = S.cfg.annotations.find((t) => t.id === S.sel);
  if (a && sel && a.id !== S.editing) selectionUi(a);
}
function selectionUi(a) {
  const b = bbox(a), hs = 10 / S.scale, g = sv('g');
  if (a.type !== 'arrow') g.append(sv('rect', { x: b.x, y: b.y, width: b.w, height: b.h, fill: 'none', stroke: '#3b82f6', 'stroke-width': 1.5 / S.scale, 'pointer-events': 'none' }));
  const handle = (id, cx, cy, round) => {
    const e = round ? sv('circle', { cx, cy, r: hs * .6 }) : sv('rect', { x: cx - hs / 2, y: cy - hs / 2, width: hs, height: hs, rx: 2 / S.scale });
    e.setAttribute('fill', '#fff'); e.setAttribute('stroke', '#3b82f6'); e.setAttribute('stroke-width', 1.5 / S.scale);
    e.style.pointerEvents = 'all'; e.style.cursor = round ? 'crosshair' : id + '-resize';
    e.addEventListener('pointerdown', (ev) => handleDown(ev, a, id)); g.append(e);
  };
  if (a.type === 'arrow') { handle('start', a.x, a.y, true); handle('end', a.x + a.width, a.y + a.height, true); }
  else {
    const xs = [b.x, b.x + b.w / 2, b.x + b.w], ys = [b.y, b.y + b.h / 2, b.y + b.h];
    const all = { nw: [0, 0], n: [1, 0], ne: [2, 0], e: [2, 1], se: [2, 2], s: [1, 2], sw: [0, 2], w: [0, 1] };
    for (const id in all) { if (a.type === 'text' && id.length === 1) continue; handle(id, xs[all[id][0]], ys[all[id][1]]); }
  }
  el.ov.append(g);
}

// ---------- select / move / resize ----------
function select(id) { if (S.sel !== id) { S.sel = id; refreshAll(); } }
function annDown(e, a) {
  if (e.button !== 0 || S.tool !== 'select') return;
  e.stopPropagation(); select(a.id);
  const p0 = pt(e), o = { x: a.x, y: a.y }; let snap = null;
  const img = S.lay.image;
  drag((ev) => {
    const p = pt(ev); let dx = p.x - p0.x, dy = p.y - p0.y;
    if (!snap) { if (Math.hypot(dx, dy) * S.scale < 3) return; snap = snapshot(); }
    a.x = o.x + dx; a.y = o.y + dy;
    if (a.type === 'mask') { a.x = clamp(a.x, img.x, img.x + img.w - a.width); a.y = clamp(a.y, img.y, img.y + img.h - a.height); }
    renderSvg(); drawMasks();
  }, () => { if (snap) { commitSnap(snap); scheduleSave(); } renderLayers(); });
}
function handleDown(e, a, id) {
  e.stopPropagation(); if (e.button !== 0) return;
  const snap = snapshot(), b = bbox(a), img = S.lay.image, orig = a.points ? clone(a.points) : null, h0 = a.height;
  const start = { x: a.x, y: a.y, w: a.width, h: a.height }; let moved = false;
  drag((ev) => {
    moved = true; let p = pt(ev); const shift = ev.shiftKey;
    if (a.type === 'mask') p = { x: clamp(p.x, img.x, img.x + img.w), y: clamp(p.y, img.y, img.y + img.h) };
    if (a.type === 'arrow') {
      if (id === 'start') { const ex = start.x + start.w, ey = start.y + start.h; a.x = p.x; a.y = p.y; a.width = ex - p.x; a.height = ey - p.y; }
      else { let dx = p.x - a.x, dy = p.y - a.y; if (shift) [dx, dy] = snap45(dx, dy); a.width = dx; a.height = dy; }
    } else if (a.type === 'text') {
      const dir = id.includes('n') ? -1 : 1, nh = clamp(Math.round(h0 + dir * (p.y - (dir > 0 ? b.y + b.h : b.y))), 12, 100);
      a.height = nh; a.width = measure(a.text, nh);
    } else {
      let l = b.x, r = b.x + b.w, t = b.y, bt = b.y + b.h;
      if (id.includes('w')) l = p.x; if (id.includes('e')) r = p.x; if (id.includes('n')) t = p.y; if (id.includes('s')) bt = p.y;
      if (id.length === 2 && ((a.type === 'rectangle' && shift) || (a.type === 'circle' && !shift) || (a.type === 'mask' && shift))) {
        const w = r - l, hh = bt - t, m = Math.max(Math.abs(w), Math.abs(hh)), sx = (w < 0 ? -1 : 1) * m, sy = (hh < 0 ? -1 : 1) * m;
        if (id.includes('e')) r = l + sx; else l = r - sx;
        if (id.includes('s')) bt = t + sy; else t = bt - sy;
      }
      a.x = Math.min(l, r); a.y = Math.min(t, bt); a.width = Math.abs(r - l); a.height = Math.abs(bt - t);
      if (orig) { const fx = l > r, fy = t > bt; a.points = orig.map((q) => [fx ? 1 - q[0] : q[0], fy ? 1 - q[1] : q[1]]); }
    }
    renderSvg(); drawMasks();
  }, () => { if (moved) { commitSnap(snap); scheduleSave(); } renderLayers(); });
}
function snap45(dx, dy) { const L = Math.hypot(dx, dy), an = Math.round(Math.atan2(dy, dx) / (Math.PI / 4)) * (Math.PI / 4); return [Math.cos(an) * L, Math.sin(an) * L]; }
function measure(text, hh, min = 20) { const c = measure.c || (measure.c = document.createElement('canvas').getContext('2d')); c.font = `${hh}px sans-serif`; return Math.max(min, Math.ceil(c.measureText(text || '').width)); }

// ---------- creation ----------
function newAnn(type, p) {
  const a = { id: uid(), type, x: p.x, y: p.y, width: 0, height: 0, strokeColor: '#F05656', strokeWidth: 4, fillColor: 'transparent', opacity: 1, rotation: 0, text: null, maskType: null, maskLevel: null, points: null };
  if (type === 'draw') { const s = S.cfg.annotations.find((t) => t.id === S.sel && t.type === 'draw') || S.lastDraw; if (s) Object.assign(a, { strokeColor: s.strokeColor, strokeWidth: s.strokeWidth, opacity: s.opacity }); }
  if (type === 'mask') Object.assign(a, { strokeColor: 'transparent', strokeWidth: 0, maskType: 'pixelate', maskLevel: 7 });
  if (type === 'text') Object.assign(a, { text: 'Text', width: 150, height: 40 });
  return a;
}
function stageDown(e) {
  if (e.button === 1) { e.preventDefault(); return startPan(e); }
  if (e.button !== 0 || e.target.closest('#zoom,#bar')) return;
  if (S.editing) return;
  if (e.target.closest('#ocr')) { if (S.sel) select(null); return; }
  if (S.tool === 'select') { if (S.sel) select(null); return startPan(e); }
  if (!el.frame.contains(e.target)) return;
  create(e);
}
function create(e) {
  const tool = S.tool, img = S.lay.image, snap = snapshot();
  let p0 = pt(e);
  if (tool === 'mask') p0 = { x: clamp(p0.x, img.x, img.x + img.w), y: clamp(p0.y, img.y, img.y + img.h) };
  const a = newAnn(tool, p0);
  if (tool === 'text') {
    a.y = p0.y - a.height / 2; S.cfg.annotations.push(a); S.sel = a.id; S.tool = 'select'; refreshAll();
    setTimeout(() => startEdit(a, snap), 30); return; // after mousedown's default focus change
  }
  S.cfg.annotations.push(a); S.sel = a.id;
  const raw = [[p0.x, p0.y]]; let moved = false;
  drag((ev) => {
    let p = pt(ev); moved = true;
    if (tool === 'mask') p = { x: clamp(p.x, img.x, img.x + img.w), y: clamp(p.y, img.y, img.y + img.h) };
    let dx = p.x - p0.x, dy = p.y - p0.y;
    if (tool === 'draw') {
      const l = raw[raw.length - 1]; if (Math.hypot(p.x - l[0], p.y - l[1]) * S.scale < 2) return;
      raw.push([p.x, p.y]); if (raw.length >= 2) setDrawPts(a, raw);
    } else if (tool === 'arrow') { if (ev.shiftKey) [dx, dy] = snap45(dx, dy); a.width = dx; a.height = dy; }
    else {
      const sq = (tool === 'rectangle' && ev.shiftKey) || (tool === 'circle' && !ev.shiftKey) || (tool === 'mask' && ev.shiftKey);
      if (sq) { const m = Math.max(Math.abs(dx), Math.abs(dy)); dx = (dx < 0 ? -1 : 1) * m; dy = (dy < 0 ? -1 : 1) * m; }
      a.x = Math.min(p0.x, p0.x + dx); a.y = Math.min(p0.y, p0.y + dy); a.width = Math.abs(dx); a.height = Math.abs(dy);
    }
    renderSvg(); drawMasks();
  }, () => {
    const big = tool === 'draw' ? raw.length >= 2 : tool === 'arrow' ? Math.hypot(a.width, a.height) >= 5 : a.width >= 5 && a.height >= 5;
    if (!moved || !big) { S.cfg.annotations = S.cfg.annotations.filter((t) => t !== a); S.sel = null; }
    else { commitSnap(snap); if (tool === 'draw') S.lastDraw = a; else S.tool = 'select'; scheduleSave(); }
    refreshAll();
  });
}

// ---------- text editing ----------
function startEdit(a, snap) {
  const isNew = !!snap; snap = snap || snapshot(); const before = a.text;
  S.editing = a.id; select(a.id); renderSvg();
  const d = h('div#edit', { contentEditable: 'plaintext-only' }); d.textContent = a.text || '';
  el.frame.append(d); el.edit = d; placeEdit(); d.focus();
  const r = document.createRange(); r.selectNodeContents(d); const s = getSelection(); s.removeAllRanges(); s.addRange(r);
  let done = false;
  const end = () => {
    if (done) return; done = true;
    const t = d.innerText.replace(/\n+$/, ''); d.remove(); el.edit = null; S.editing = null;
    if (!t.trim()) { S.cfg.annotations = S.cfg.annotations.filter((x) => x !== a); S.sel = null; if (!isNew) commitSnap(snap); }
    else { a.text = t; a.width = measure(t, a.height); if (isNew || t !== before) commitSnap(snap); }
    refreshAll(); scheduleSave();
  };
  d.addEventListener('keydown', (e) => { e.stopPropagation(); if ((e.key === 'Enter' && !e.shiftKey) || e.key === 'Escape') { e.preventDefault(); end(); } });
  d.addEventListener('input', () => { a.width = measure(d.innerText, a.height); });
  d.addEventListener('blur', end);
}
function placeEdit() {
  const a = S.cfg.annotations.find((t) => t.id === S.editing); if (!a || !el.edit) return;
  Object.assign(el.edit.style, { left: a.x * S.scale + 'px', top: (a.y + a.height * .2) * S.scale + 'px', fontSize: a.height * S.scale + 'px', color: a.strokeColor === 'transparent' ? '#000' : a.strokeColor });
}

// ---------- colour popup ----------
let cpop = null;
function closeColor() { if (cpop) { cpop.remove(); cpop = null; } }
// get() -> '#RRGGBB'|'transparent'; set(hex|'transparent', alpha0..255)
function colorBtn(get, allowT, set, onOpen) {
  const b = h('button.sw', { type: 'button' });
  const paint = () => { const v = get(); b.classList.toggle('ck', v === 'transparent'); b.style.background = v === 'transparent' ? '' : v; };
  paint();
  b.onclick = (e) => {
    e.stopPropagation(); if (cpop && cpop.owner === b) return closeColor(); closeColor(); onOpen && onOpen();
    const cur = () => get();
    const pick = h('input', { type: 'color', value: cur() === 'transparent' ? '#000000' : cur().slice(0, 7).toLowerCase() });
    const hx = h('input', { type: 'text', value: cur() === 'transparent' ? '' : cur(), spellcheck: false });
    const apply = (v, a) => { set(v, a); paint(); hx.value = v === 'transparent' ? '' : v; if (v !== 'transparent') pick.value = v.toLowerCase(); sel(); };
    pick.oninput = () => apply(pick.value.toUpperCase());
    const commit = () => { const m = hx.value.trim().replace(/^#/, ''); if (/^[0-9a-f]{6}([0-9a-f]{2})?$/i.test(m)) apply('#' + m.slice(0, 6).toUpperCase(), m.length === 8 ? parseInt(m.slice(6), 16) : 255); else hx.value = cur() === 'transparent' ? '' : cur(); };
    hx.onkeydown = (ev) => { ev.stopPropagation(); if (ev.key === 'Enter') commit(); }; hx.onblur = commit;
    const sws = PALETTE.map((c) => h('button', { type: 'button', style: `background:${c}`, title: c, onclick: () => apply(c) }));
    if (allowT) sws.push(h('button.ck', { type: 'button', title: 'Transparent', onclick: () => apply('transparent') }));
    const sel = () => sws.forEach((s, i) => s.classList.toggle('on', (PALETTE[i] || 'transparent') === cur()));
    sel();
    const el2 = h('div#cpop.pop', {}, h('div.top', {}, pick, hx), h('div.pal', {}, sws));
    el2.owner = b; document.body.append(el2);
    const r = b.getBoundingClientRect();
    el2.style.left = clamp(r.left, 8, innerWidth - 236) + 'px'; el2.style.top = Math.min(r.bottom + 6, innerHeight - el2.offsetHeight - 8) + 'px';
    cpop = el2;
  };
  return b;
}

// ---------- ui primitives ----------
function sld(min, max, step, val, oninput, onstart, fmt) {
  const r = h('input', { type: 'range', min, max, step, value: val });
  const p = () => r.style.setProperty('--p', ((r.value - min) / (max - min)) * 100 + '%'); p();
  let lbl = null; const upd = () => { if (lbl) lbl.textContent = fmt ? fmt(+r.value) : r.value; };
  r.addEventListener('pointerdown', () => onstart && onstart());
  r.addEventListener('input', () => { p(); upd(); oninput(+r.value); });
  r.bind = (e) => { lbl = e; upd(); return r; };
  return r;
}
function frow(icon, name, control, valEl) { return h('div.fr', {}, h('div.t', {}, icon && ico(icon), name, h('span.sp'), valEl), control); }
function sliderRow(name, min, max, step, val, fn, fmt, icon) {
  const v = h('span.vl'); const r = sld(min, max, step, val, fn, push, fmt).bind(v); return frow(icon, name, r, v);
}
function toggle(on, fn) { const t = h('button.tg', { type: 'button' }); t.classList.toggle('on', !!on); t.onclick = () => { const n = !t.classList.contains('on'); t.classList.toggle('on', n); fn(n); }; return t; }

// ---------- popovers ----------
const bgc = () => S.cfg.background;
function coerceWhite(wasZero) { const s = bgc().source; if (wasZero && s.type === 'color' && s.alpha === 0) bgc().source = { type: 'color', value: [255, 255, 255], alpha: 255 }; }
function seedDefaults() { const b = bgc(), p0 = !b.padding; if (p0) { b.padding = 10; if (!b.rounding) b.rounding = 8; } if (!b.shadow) b.shadow = 73.6; }
const hasWP = () => typeof WALLPAPERS !== 'undefined' && WALLPAPERS.length;
let wpCat = null;
function bgPop() {
  const b = bgc(), s = b.source, wp = hasWP(), type = s.type === 'wallpaper' && !wp ? 'image' : (S.wpTab && wp && s.type !== 'wallpaper' ? 'wallpaper' : s.type);
  const body = h('div.pb', { style: 'width:400px;max-height:600px;overflow-y:auto' });
  const setSrc = (src, seed) => { push(); if (seed) seedDefaults(); b.source = src; changed(); openPop('bg'); };
  const tabs = h('div.tabs', {}, [...(wp ? [['wallpaper', 'Wallpaper']] : []), ['image', 'Image'], ['color', 'Color'], ['gradient', 'Gradient']].map(([k, l]) => h('button.tab' + (type === k ? '.on' : ''), { type: 'button', onclick: () => {
    if (type === k) return;
    S.wpTab = k === 'wallpaper';
    if (k === 'wallpaper') return openPop('bg');
    b.source = k === 'color' ? { type: 'color', value: [71, 133, 255], alpha: 255 } : k === 'gradient' ? { type: 'gradient', from: [71, 133, 255], to: [255, 71, 102], angle: 90 } : { type: 'image', path: null };
    if (k === 'gradient') seedDefaults(); changed(); openPop('bg');
  } }, l)));
  const content = h('div', { style: 'display:flex;flex-direction:column;gap:12px' });
  if (type === 'wallpaper') {
    const cat = WALLPAPERS.find((c) => c.id === wpCat) || WALLPAPERS.find((c) => c.items.some((i) => s.path === `wallpapers/${i}.jpg`)) || WALLPAPERS[0];
    content.append(h('div.wpt', {}, WALLPAPERS.map((c) => h('button.tab' + (c === cat ? '.on' : ''), { type: 'button', onclick: () => { wpCat = c.id; openPop('bg'); } }, c.name))),
      h('div.wpg', {}, cat.items.map((i) => h('button' + (s.path === `wallpapers/${i}.jpg` ? '.on' : ''), { type: 'button', title: i, onclick: () => { S.wpTab = false; setSrc({ type: 'wallpaper', path: `wallpapers/${i}.jpg` }, true); } },
        h('img', { src: `wallpapers/thumbs/${i}.jpg`, loading: 'eager', draggable: false })))));
  } else if (type === 'image') {
    const inp = h('input', { type: 'file', accept: 'image/apng,image/avif,image/jpeg,image/png,image/webp', style: 'display:none', onchange: async () => {
      const f = inp.files[0]; if (!f) return;
      try { const path = await invoke('save_asset', new Uint8Array(await f.arrayBuffer()), { headers: { name: f.name } }); setSrc({ type: 'image', path }, true); }
      catch (e) { toast('Could not load image: ' + e); }
    } });
    content.append(inp, h('button.drop', { type: 'button', onclick: () => inp.click() }, ico('upload'), s.path ? 'Replace image' : 'Click to select an image'));
    if (s.path) content.append(h('div.cl', {}, s.path.split(/[\\/]/).pop(), h('button.hb', { type: 'button', title: 'Clear', style: 'height:22px;min-width:22px', onclick: () => setSrc({ type: 'image', path: null }) }, ico('x', 12))));
  } else if (type === 'color') {
    const cur = () => (s.alpha === 0 ? 'transparent' : toHex(s.value));
    const grid = h('div.pal', {}, [...PALETTE, 'transparent'].map((c) => h('button' + (c === cur() ? '.on' : '') + (c === 'transparent' ? '.ck' : ''), { type: 'button', title: c, style: c === 'transparent' ? '' : `background:${c}`, onclick: () => { push(); b.source = c === 'transparent' ? { type: 'color', value: s.value, alpha: 0 } : { type: 'color', value: fromHex(c), alpha: 255 }; changed(); openPop('bg'); } })));
    content.append(h('div', { style: 'display:flex;gap:8px;align-items:center' }, 'Color', colorBtn(cur, true, (v, a) => { b.source = v === 'transparent' ? { type: 'color', value: s.value, alpha: 0 } : { type: 'color', value: fromHex(v), alpha: a ?? 255 }; changed(); }, push)), grid);
  } else {
    const g = s;
    content.append(h('div', { style: 'display:flex;gap:16px;align-items:center' }, 'From', colorBtn(() => toHex(g.from), false, (v) => { b.source = { ...b.source, from: fromHex(v) }; changed(); }, push), 'To', colorBtn(() => toHex(g.to), false, (v) => { b.source = { ...b.source, to: fromHex(v) }; changed(); }, push)),
      h('div.grid', { style: 'grid-template-columns:repeat(6,1fr)' }, GRADS.map(([f, t]) => h('button.pre', { type: 'button', style: `background:linear-gradient(${g.angle ?? 90}deg,${rgbCss(f)},${rgbCss(t)})`, onclick: () => setSrc({ type: 'gradient', from: f, to: t, angle: g.angle ?? 90 }, true) }))));
  }
  body.append(frow('bg', 'Background Image', null), tabs, h('div.dash'), content, sliderRow('Background Blur', 0, 100, 0.1, b.blur || 0, (v) => { b.blur = v; changed(); }, (v) => v.toFixed(1) + '%'));
  return body;
}
function padPop() { return h('div.pb', { style: 'width:200px' }, sliderRow('Padding', 0, 100, 1, bgc().padding, (v) => { const z = !bgc().padding; bgc().padding = v; if (v > 0) coerceWhite(z); changed(); }, (v) => v + 'px')); }
function roundPop() {
  const b = bgc(), seg = h('div.tabs');
  const fill = () => { seg.replaceChildren(...[['squircle', 'Squircle'], ['rounded', 'Rounded']].map(([k, l]) => h('button.tab' + (b.roundingType === k || (!b.roundingType && k === 'squircle') ? '.on' : ''), { type: 'button', onclick: () => { b.roundingType = k; changed(); fill(); } }, l))); };
  fill();
  return h('div.pb', { style: 'width:240px' }, sliderRow('Rounding', 0, 100, 1, b.rounding, (v) => { const z = !b.rounding; b.rounding = v; if (v > 0) coerceWhite(z); changed(); }, (v) => v + 'px'),
    h('div.fr', {}, h('div.t', { style: 'font-size:10px;letter-spacing:.05em;text-transform:uppercase;color:var(--g11)' }, 'Corner Style'), seg));
}
function shadowPop() {
  const b = bgc(); const adv = h('div', { style: 'display:none;flex-direction:column;gap:12px' });
  const A = () => b.advancedShadow || (b.advancedShadow = { size: 50, opacity: 18, blur: 50 });
  const arow = (n, k) => sliderRow(n, 0, 100, 0.1, A()[k], (v) => { A()[k] = v; changed(); }, (v) => v.toFixed(1));
  const cl = h('button.cl', { type: 'button' }, 'Advanced shadow settings', ico('chev', 12));
  cl.onclick = () => { const o = adv.style.display === 'none'; adv.style.display = o ? 'flex' : 'none'; cl.classList.toggle('open', o); if (o) adv.replaceChildren(arow('Size', 'size'), arow('Opacity', 'opacity'), arow('Blur', 'blur')); };
  return h('div.pb', { style: 'width:280px' }, sliderRow('Shadow', 0, 100, 1, b.shadow, (v) => { if (v > 0 && !b.advancedShadow) b.advancedShadow = { size: 50, opacity: 18, blur: 50 }; b.shadow = v; changed(); }, (v) => v + '%'), cl, adv);
}
function borderPop() {
  const b = bgc(); const B = () => b.border || (b.border = { enabled: false, width: 5, color: [0, 0, 0], opacity: 50 });
  const rest = h('div', { style: 'display:flex;flex-direction:column;gap:16px' });
  const fill = () => { rest.style.display = B().enabled ? 'flex' : 'none'; };
  const tg = toggle(B().enabled, (v) => { push(); B().enabled = v; fill(); changed(); });
  rest.append(sliderRow('Width', 1, 20, 0.1, B().width, (v) => { B().width = v; changed(); }, (v) => v.toFixed(1) + 'px'),
    h('div.fr', {}, h('div.t', {}, 'Color', h('span.sp'), colorBtn(() => toHex(B().color), false, (v) => { B().color = fromHex(v); changed(); }, push))),
    sliderRow('Opacity', 0, 100, 0.1, B().opacity, (v) => { B().opacity = v; changed(); }, (v) => v.toFixed(1) + '%'));
  fill();
  return h('div.pb', { style: 'width:280px' }, h('div.t', { style: 'display:flex;align-items:center;gap:6px;font-size:13px' }, ico('border'), 'Border', h('span', { style: 'flex:1' }), tg), rest);
}
function aspectMenu() {
  return h('div.menu', { style: 'width:200px' }, ASPECTS.map(([k, n, r]) => h('button.mi', { type: 'button', onclick: () => { if (S.cfg.aspectRatio !== k) { push(); S.cfg.aspectRatio = k; changed(); } closePop(); refreshHeader(); } },
    h('span.ck2', {}, S.cfg.aspectRatio === k && ico('check', 14)), r ? `${n} ⋅ ${r}` : n)));
}
function moreMenu() {
  return h('div.menu', { style: 'width:200px' },
    h('button.mi', { type: 'button', onclick: () => { closePop(); invoke('ss_reveal').catch((e) => toast(String(e))); } }, ico('folder'), 'Open Folder'),
    h('button.mi.danger', { type: 'button', onclick: () => { closePop(); confirmDelete(); } }, ico('trash'), 'Delete'));
}
const POPS = { aspect: aspectMenu, bg: bgPop, pad: padPop, round: roundPop, shadow: shadowPop, border: borderPop, more: moreMenu };
function closePop() { closeColor(); if (S.pop) { S.pop.el.remove(); el.hb[S.pop.name] && el.hb[S.pop.name].classList.remove('open'); S.pop = null; } }
function openPop(name, toggleIt) {
  const same = S.pop && S.pop.name === name; const scroll = same && S.pop.el.firstChild ? S.pop.el.firstChild.scrollTop : 0;
  const was = same; closePop(); if (toggleIt && was) return;
  const anchor = el.hb[name]; const p = h('div.pop', {}, POPS[name]());
  document.body.append(p); const r = anchor.getBoundingClientRect();
  p.style.left = clamp(name === 'more' ? r.right - p.offsetWidth : r.left, 8, innerWidth - p.offsetWidth - 8) + 'px'; p.style.top = r.bottom + 6 + 'px';
  if (scroll && p.firstChild) p.firstChild.scrollTop = scroll;
  anchor.classList.add('open'); S.pop = { name, el: p };
}
document.addEventListener('pointerdown', (e) => {
  if (cpop && !cpop.contains(e.target) && !e.target.closest('.sw')) closeColor();
  if (S.pop && !S.pop.el.contains(e.target) && !(cpop && cpop.contains(e.target)) && !e.target.closest('[data-pop]')) closePop();
}, true);

// ---------- header ----------
function refreshHeader() {
  const a = ASPECTS.find((x) => x[0] === S.cfg.aspectRatio) || ASPECTS[0];
  el.aspectTxt.textContent = a[2] || 'Auto';
  el.hb.layers.classList.toggle('on', S.layers);
  for (const [k] of TOOLS) el.hb['t_' + k].classList.toggle('on', S.tool === k);
  el.stage.style.cursor = S.tool === 'select' ? '' : 'crosshair';
  el.ocr.classList.toggle('on', S.tool === 'select');
}
function buildHeader() {
  el.hb = {};
  const btn = (key, icon, title, fn, extra) => { const b = h('button.hb', { type: 'button', title, onclick: fn, ...extra }, ico(icon)); el.hb[key] = b; return b; };
  const pbtn = (name, icon, title) => { const b = btn(name, icon, title, () => openPop(name, true)); b.dataset.pop = name; return b; };
  el.aspectTxt = h('span', { style: 'font-size:14px;width:32px;text-align:left' }, 'Auto');
  const asp = h('button.hb', { type: 'button', title: 'Aspect Ratio', style: 'width:80px;justify-content:space-between;padding:0 8px', onclick: () => openPop('aspect', true) }, ico('layout'), el.aspectTxt, ico('chev', 12)); asp.dataset.pop = 'aspect'; el.hb.aspect = asp;
  const crop = btn('crop', 'crop', 'Crop Image', openCrop); crop.append(h('span', { style: 'font-size:14px;padding-right:2px' }, 'Crop Image')); crop.disabled = true; el.cropBtn = crop;
  const tools = TOOLS.map(([k, n, key, ic]) => btn('t_' + k, ic, `${n} (${key})`, () => setTool(k)));
  const more = btn('more', 'dots', 'More Actions', () => openPop('more', true)); more.dataset.pop = 'more';
  const hd = h('header', { 'data-tauri-drag-region': true },
    h('div.spacer', { 'data-tauri-drag-region': true }),
    h('div.mid', { 'data-tauri-drag-region': true }, asp, crop, h('div.vd'),
      h('div.grp', {}, btn('layers', 'layers', 'Layers (L)', toggleLayers), h('div.vd.s'), ...tools), h('div.vd'),
      h('div.grp', {}, pbtn('bg', 'bg', 'Background (B)'), pbtn('pad', 'pad', 'Padding (P)'), pbtn('round', 'round', 'Corner Rounding'), pbtn('shadow', 'shadow', 'Shadow (H)'), pbtn('border', 'border', 'Border (E)'))),
    h('div.right', { 'data-tauri-drag-region': true }, h('div.vd'), btn('copy', 'copy', 'Copy to Clipboard', copyImage), btn('save', 'save', 'Save (⌘S)', saveImage), more));
  return hd;
}
function setTool(t) { if (S.editing) return; S.tool = t; if (t !== 'select') S.sel = null; refreshAll(); }
function toggleLayers() { S.layers = !S.layers; el.layers.classList.toggle('hidden', !S.layers); refreshHeader(); applyView(); renderSvg(); }

// ---------- layers panel + property bar ----------
function renderLayers() {
  if (!el.llist) return;
  const L = el.llist, anns = S.cfg.annotations; L.replaceChildren();
  if (!anns.length) { L.append(h('div.lempty', {}, ico('layers', 32), h('b', {}, 'No layers yet'), 'Use the tools above to add annotations')); return; }
  const rows = [];
  [...anns].reverse().forEach((a) => {
    const lbl = a.type === 'text' ? (a.text || '').slice(0, 12) + ((a.text || '').length > 12 ? '...' : '') : TNAME[a.type];
    const grip = h('span.grip', { style: 'touch-action:none' }, ico('grip', 14));
    const row = h('div.lr' + (a.id === S.sel ? '.sel' : ''), { onclick: () => { S.tool = 'select'; S.sel = a.id; refreshAll(); } },
      grip, h('span.chip', {}, ico(TICON[a.type], 14)), h('span.lbl', {}, lbl),
      h('button.del', { type: 'button', title: 'Delete', onclick: (e) => { e.stopPropagation(); delAnn(a.id); } }, ico('x', 12)));
    grip.onpointerdown = (e) => reorder(e, rows, a);
    rows.push(row); L.append(row);
  });
}
function reorder(e, rows, a) {
  e.preventDefault(); e.stopPropagation();
  const n = rows.length, from = n - 1 - S.cfg.annotations.indexOf(a); let to = from;
  const mark = () => rows.forEach((r, i) => { r.classList.toggle('db', i === to && to < n); r.classList.toggle('da', to === n && i === n - 1); });
  drag((ev) => { to = rows.findIndex((r) => { const b = r.getBoundingClientRect(); return ev.clientY < b.top + b.height / 2; }); if (to < 0) to = n; mark(); }, () => {
    const list = [...S.cfg.annotations].reverse(), [it] = list.splice(from, 1); list.splice(to > from ? to - 1 : to, 0, it);
    const next = list.reverse();
    if (next.some((x, i) => x !== S.cfg.annotations[i])) { push(); S.cfg.annotations = next; scheduleSave(); }
    refreshAll();
  });
}
function delAnn(id) { push(); S.cfg.annotations = S.cfg.annotations.filter((a) => a.id !== id); if (S.sel === id) S.sel = null; refreshAll(); scheduleSave(); }
function renderBar() {
  const bar = el.bar, a = S.cfg.annotations.find((t) => t.id === S.sel);
  if (!a) { bar.classList.add('hidden'); return; }
  bar.classList.remove('hidden'); bar.style.left = S.layers ? '224px' : '0'; bar.replaceChildren();
  const upd = () => { renderSvg(); drawMasks(); renderLayers(); scheduleSave(); };
  const field = (name, ctl, val) => h('div.f', {}, h('label', {}, name, val), ctl);
  const slider = (name, min, max, step, key, fmt, w) => { const v = h('span.v'); const r = sld(min, max, step, a[key], (x) => { a[key] = x; upd(); }, push, fmt).bind(v); r.style.width = (w || 96) + 'px'; return field(name, r, v); };
  if (a.type === 'mask') {
    const seg = h('div.f', { style: 'gap:4px' });
    const fill = () => seg.replaceChildren(...[['blur', 'Blur'], ['pixelate', 'Pixelate']].map(([k, l]) => h('button.seg' + (a.maskType === k ? '.on' : ''), { type: 'button', onclick: () => { push(); a.maskType = k; fill(); upd(); } }, l)));
    fill();
    bar.append(field('Style', seg), slider('Intensity', 4, 50, 1, 'maskLevel', null, 96));
    a.maskLevel = a.maskLevel || 7;
  } else {
    bar.append(field(a.type === 'text' ? 'Color' : 'Stroke', colorBtn(() => a.strokeColor, false, (v) => { a.strokeColor = v; upd(); }, push)));
    if (a.type !== 'text') bar.append(slider('Width', 1, 20, 1, 'strokeWidth', (v) => v + 'px'));
    if (a.type === 'rectangle' || a.type === 'circle') bar.append(field('Fill', colorBtn(() => a.fillColor || 'transparent', true, (v) => { a.fillColor = v; upd(); }, push)));
    bar.append(slider('Opacity', 0.1, 1, 0.1, 'opacity', (v) => Math.round(v * 100) + '%'));
    if (a.type === 'text') {
      const v = h('span.v'); const r = sld(12, 100, 1, a.height, (x) => { a.height = x; a.width = measure(a.text, x); upd(); }, push, (x) => x + 'px').bind(v); r.style.width = '96px'; bar.append(field('Size', r, v));
    }
  }
  bar.append(h('div.vd', { style: 'height:20px' }), h('button.done', { type: 'button', onclick: () => select(null) }, 'Done'));
}
function refreshAll() { if (!S.cfg) return; refreshHeader(); renderSvg(); drawMasks(); renderLayers(); renderBar(); }

// ---------- export ----------
async function renderPng() {
  await settle();
  const cfg = clone(S.cfg);
  const bmp = await createImageBitmap(new Blob([toBuf(await invoke('ss_render', { config: cfg }))], { type: 'image/png' }));
  const lay = S.lay, c = document.createElement('canvas'); c.width = bmp.width; c.height = bmp.height;
  const x = c.getContext('2d'); x.drawImage(bmp, 0, 0);
  const k = c.width / lay.frameW;
  for (const a of cfg.annotations) if (a.type === 'mask') drawMask(x, c, a, k);
  for (const a of cfg.annotations) if (a.type !== 'mask') drawAnn(x, a, k);
  return new Promise((res, rej) => c.toBlob((b) => (b ? res(b) : rej(new Error('PNG encode failed'))), 'image/png'));
}
function drawAnn(x, a, k) {
  x.save(); x.scale(k, k); x.globalAlpha = a.opacity ?? 1; x.lineCap = 'round'; x.lineJoin = 'round';
  const st = () => { if (stroke(a) !== 'none') { x.strokeStyle = a.strokeColor; x.lineWidth = a.strokeWidth; x.stroke(); } };
  const fl = () => { if (fillOf(a) !== 'none') { x.fillStyle = a.fillColor; x.fill(); } };
  if (a.type === 'rectangle') { x.beginPath(); x.rect(a.x, a.y, a.width, a.height); fl(); st(); }
  else if (a.type === 'circle') { x.beginPath(); x.ellipse(a.x + a.width / 2, a.y + a.height / 2, Math.abs(a.width / 2), Math.abs(a.height / 2), 0, 0, Math.PI * 2); fl(); st(); }
  else if (a.type === 'arrow' && a.strokeColor !== 'transparent') {
    const g = arrowGeom(a); x.strokeStyle = x.fillStyle = a.strokeColor; x.lineWidth = a.strokeWidth;
    x.beginPath(); x.moveTo(g.line[0], g.line[1]); x.lineTo(g.line[2], g.line[3]); x.stroke();
    x.beginPath(); g.tri.forEach((p, i) => (i ? x.lineTo(p[0], p[1]) : x.moveTo(p[0], p[1]))); x.closePath(); x.fill();
  } else if (a.type === 'draw') { const d = drawPath(a); if (d) { x.strokeStyle = a.strokeColor === 'transparent' ? 'rgba(0,0,0,0)' : a.strokeColor; x.lineWidth = a.strokeWidth; x.stroke(new Path2D(d)); } }
  else if (a.type === 'text' && a.strokeColor !== 'transparent') { x.fillStyle = a.strokeColor; x.font = `${a.height}px sans-serif`; x.textBaseline = 'alphabetic'; x.fillText(a.text || '', a.x, a.y + a.height); }
  x.restore();
}
let exporting = false;
async function withExport(fn) {
  if (exporting) return; exporting = true; el.hb.copy.disabled = el.hb.save.disabled = true;
  try { await fn(await renderPng()); } catch (e) { toast('Failed to export: ' + (e && e.message || e)); }
  exporting = false; el.hb.copy.disabled = el.hb.save.disabled = false;
}
const copyImage = () => withExport(async (blob) => {
  try { await navigator.clipboard.write([new ClipboardItem({ 'image/png': blob })]); }
  catch (_) { await invoke('ss_copy_png', new Uint8Array(await blob.arrayBuffer())); }
  toast('Screenshot copied to clipboard!');
});
const saveImage = () => withExport(async (blob) => {
  const path = await invoke('ss_save_png', new Uint8Array(await blob.arrayBuffer()));
  toast('Screenshot saved! ' + String(path).split(/[\\/]/).pop());
});
function confirmDelete() {
  const close = () => m.remove();
  const m = h('div.modal', { onpointerdown: (e) => { if (e.target === m) close(); } }, h('div.card', {}, h('h3', {}, 'Delete screenshot'), h('p', {}, 'Are you sure you want to delete this screenshot?'),
    h('div.btns', {}, h('button.btn', { type: 'button', onclick: close }, 'Cancel'),
      h('button.btn.dng', { type: 'button', onclick: async () => { close(); S.deleted = true; try { await invoke('ss_delete'); } catch (e) { S.deleted = false; toast('Failed to delete: ' + e); } } }, 'Delete'))));
  document.body.append(m);
}

// ---------- crop dialog ----------
async function openCrop() {
  closePop();
  const plain = clone(S.cfg); Object.assign(plain.background, { padding: 0, rounding: 0, shadow: 0, advancedShadow: null, border: null, crop: null, blur: 0 }); plain.aspectRatio = null; plain.annotations = [];
  let url;
  try { url = URL.createObjectURL(new Blob([toBuf(await invoke('ss_frame', { config: plain, maxWidth: 1600 }))], { type: 'image/png' })); } catch (e) { return toast(String(e)); }
  const ow = S.ow, oh = S.oh, c0 = S.cfg.background.crop;
  const init = c0 ? { x: c0.position.x, y: c0.position.y, w: c0.size.x, h: c0.size.y } : { x: 0, y: 0, w: ow, h: oh };
  let c = { ...init };
  const maxW = Math.min(innerWidth * .8, 768), maxH = innerHeight * .65, ds = Math.min(maxW / ow, maxH / oh);
  const img = h('img', { src: url, style: `width:${ow * ds}px;height:${oh * ds}px` });
  const rect = h('div.crect', {}, ['nw', 'n', 'ne', 'e', 'se', 's', 'sw', 'w'].map((id) => { const i = h('i', { style: `left:${id.includes('w') ? 0 : id.includes('e') ? 100 : 50}%;top:${id.includes('n') ? 0 : id.includes('s') ? 100 : 50}%;cursor:${id}-resize` }); i.dataset.h = id; return i; }));
  const cbox = h('div.cbox', {}, img, h('div.cclip', {}, rect));
  const num = (k) => h('input.num', { type: 'number', min: 0, onchange: () => { set(k, +nums[k].value); } });
  const nums = { w: num('w'), h: num('h'), x: num('x'), y: num('y') };
  const full = h('button.btn', { type: 'button', onclick: () => { c = { x: 0, y: 0, w: ow, h: oh }; paint(); } }, 'Full');
  const reset = h('button.btn', { type: 'button', onclick: () => { c = { ...init }; paint(); } }, 'Reset');
  const ratio = h('select', { onchange: () => { const v = ratio.value; if (!v) return; const [rw, rh] = v.split(':').map(Number), r = rw / rh; let w = ow, hh = w / r; if (hh > oh) { hh = oh; w = hh * r; } c = { x: Math.round((ow - w) / 2), y: Math.round((oh - hh) / 2), w: Math.round(w), h: Math.round(hh) }; ratio.value = ''; paint(); } },
    h('option', { value: '' }, 'Ratio'), ...ASPECTS.slice(1).map((a) => h('option', { value: a[2] }, a[2])));
  function paint() {
    c.w = clamp(Math.round(c.w), 1, ow); c.h = clamp(Math.round(c.h), 1, oh); c.x = clamp(Math.round(c.x), 0, ow - c.w); c.y = clamp(Math.round(c.y), 0, oh - c.h);
    Object.assign(rect.style, { left: c.x * ds + 'px', top: c.y * ds + 'px', width: c.w * ds + 'px', height: c.h * ds + 'px' });
    for (const k in nums) nums[k].value = c[k]; full.disabled = c.w === ow && c.h === oh && !c.x && !c.y;
  }
  function set(k, v) { c[k] = v; paint(); }
  rect.onpointerdown = (e) => {
    e.preventDefault(); const id = e.target.dataset.h, p0 = { x: e.clientX, y: e.clientY }, o = { ...c };
    drag((ev) => {
      const dx = (ev.clientX - p0.x) / ds, dy = (ev.clientY - p0.y) / ds;
      if (!id) { c.x = clamp(o.x + dx, 0, ow - o.w); c.y = clamp(o.y + dy, 0, oh - o.h); }
      else {
        let l = o.x, r = o.x + o.w, t = o.y, b = o.y + o.h;
        if (id.includes('w')) l = clamp(o.x + dx, 0, r - 8); if (id.includes('e')) r = clamp(r + dx, l + 8, ow);
        if (id.includes('n')) t = clamp(o.y + dy, 0, b - 8); if (id.includes('s')) b = clamp(b + dy, t + 8, oh);
        c = { x: l, y: t, w: r - l, h: b - t };
      }
      paint();
    });
  };
  const close = () => { URL.revokeObjectURL(url); m.remove(); };
  const m = h('div.modal', { onpointerdown: (e) => { if (e.target === m) close(); } }, h('div.card.crop', {},
    h('div.hd', {}, 'Size', nums.w, '×', nums.h, h('span', { style: 'width:8px' }), 'Position', nums.x, '×', nums.y, h('span.sp'), ratio, full, reset),
    h('div.cbody', {}, cbox), h('div.btns', { style: 'margin-top:12px' }, h('button.btn', { type: 'button', onclick: close }, 'Cancel'),
      h('button.btn.pri', { type: 'button', onclick: () => {
        const isFull = c.w === ow && c.h === oh; const nv = isFull ? null : { position: { x: c.x, y: c.y }, size: { x: c.w, y: c.h } };
        if (JSON.stringify(nv) !== JSON.stringify(S.cfg.background.crop || null)) { push(); S.cfg.background.crop = nv; changed(); }
        close();
      } }, 'Save'))));
  document.body.append(m); paint();
}

// ---------- keyboard ----------
document.addEventListener('keydown', (e) => {
  const t = e.target;
  if (t && (t.tagName === 'INPUT' || t.tagName === 'TEXTAREA' || t.tagName === 'SELECT' || t.isContentEditable) && !(t.type === 'range')) return;
  if (!S.cfg || document.querySelector('.modal')) return;
  const k = e.key.toLowerCase(), mod = e.metaKey || e.ctrlKey;
  if (mod) {
    if (k === 'z') { e.preventDefault(); e.shiftKey ? redo() : undo(); }
    else if (k === 'y') { e.preventDefault(); redo(); }
    else if (k === 's') { e.preventDefault(); saveImage(); }
    else if (k === 'c') { const txt = String(getSelection()); if (txt.length) { e.preventDefault(); navigator.clipboard.writeText(txt).catch(() => document.execCommand('copy')); return; } e.preventDefault(); const a = S.cfg.annotations.find((x) => x.id === S.sel); if (a) S.clip = clone(a); else copyImage(); }
    else if (k === 'v') { if (!S.clip) return; e.preventDefault(); const a = clone(S.clip); a.id = uid(); a.x += 16; a.y += 16; push(); S.cfg.annotations.push(a); S.sel = a.id; S.clip = clone(a); S.tool = 'select'; refreshAll(); scheduleSave(); }
    else if (k === '-') { e.preventDefault(); S.pan = { x: 0, y: 0 }; setZoom(S.zoom - 0.1); }
    else if (k === '=' || k === '+') { e.preventDefault(); S.pan = { x: 0, y: 0 }; setZoom(S.zoom + 0.1); }
    return;
  }
  if (e.altKey) return;
  const tool = { a: 'arrow', r: 'rectangle', m: 'mask', c: 'circle', o: 'circle', d: 'draw', t: 'text', v: 'select', s: 'select' }[k];
  if (tool) { e.preventDefault(); S.sel = null; return setTool(tool); }
  if (k === 'escape') { if (S.pop) closePop(); else { S.sel = null; setTool('select'); } return; }
  const pop = { p: 'pad', b: 'bg', h: 'shadow', e: 'border' }[k];
  if (pop) { e.preventDefault(); return openPop(pop, true); }
  if (k === 'l') return toggleLayers();
  if ((k === 'delete' || k === 'backspace') && S.sel) { e.preventDefault(); delAnn(S.sel); }
});

// ---------- boot ----------
function build() {
  el.cv = h('canvas'); el.mk = h('canvas#mk'); el.ov = sv('svg', { id: 'ov', preserveAspectRatio: 'none' });
  el.ocr = h('div#ocr'); el.frame = h('div#frame', {}, el.cv, el.mk, el.ocr, el.ov);
  el.zr = h('input', { type: 'range', min: 0.1, max: 3, step: 0.1, value: 1, oninput: () => { setZoom(+el.zr.value); } });
  el.stage = h('div#stage', { onpointerdown: stageDown, onauxclick: (e) => e.preventDefault() }, el.frame);
  const zoom = h('div#zoom', {}, h('button.hb', { type: 'button', title: 'Zoom Out (⌘-)', style: 'height:24px;min-width:24px', onclick: () => setZoom(S.zoom - 0.1) }, ico('minus', 14)), el.zr,
    h('button.hb', { type: 'button', title: 'Zoom In (⌘+)', style: 'height:24px;min-width:24px', onclick: () => setZoom(S.zoom + 0.1) }, ico('plus', 14)));
  el.llist = h('div#llist');
  el.layers = h('aside#layers.hidden', {}, h('div.lh', {}, ico('layers', 16), 'Layers', h('span.sp'), h('button', { type: 'button', title: 'Close', onclick: toggleLayers }, ico('x', 14))), el.llist, h('div.lf', {}, 'Drag to reorder • Top = front'));
  el.bar = h('div#bar.hidden');
  const app = document.getElementById('app');
  app.replaceChildren(buildHeader(), h('div#row', {}, el.layers, h('div#stagebox', {}, el.stage, el.bar, zoom)));
  el.stage.addEventListener('wheel', (e) => {
    e.preventDefault();
    if (e.ctrlKey || e.metaKey) setZoom(S.zoom - e.deltaY * 0.005, e.clientX, e.clientY);
    else { S.pan.x -= e.deltaX; S.pan.y -= e.deltaY; applyView(); }
  }, { passive: false });
  let g0 = 1; el.stage.addEventListener('gesturestart', (e) => { e.preventDefault(); g0 = S.zoom; });
  el.stage.addEventListener('gesturechange', (e) => { e.preventDefault(); setZoom(g0 * e.scale, e.clientX, e.clientY); });
  new ResizeObserver(() => applyView()).observe(el.stage);
}
(async function main() {
  try {
    const p = await invoke('ss_project');
    S.cfg = p.config; S.name = p.name; S.ow = p.width; S.oh = p.height;
    S.cfg.annotations = S.cfg.annotations || [];
    S.cfg.background = S.cfg.background || {};
    build(); refreshAll(); el.cropBtn.disabled = false;
    await sync();
    invoke('ss_ocr').then((r) => setOcr(r && r.lines), (e) => log('ss_ocr: ' + e));
  } catch (e) {
    document.getElementById('app').replaceChildren(h('div#err', {}, String((e && e.message) || e)));
  }
})();
