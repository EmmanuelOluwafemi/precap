'use strict';
const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;
const $ = (s, r = document) => r.querySelector(s);
const log = (m) => { try { invoke('log', { msg: String(m) }); } catch (_) {} };
window.onerror = (m, src, l, c) => log(`error: ${m} @${l}:${c}`);
window.addEventListener('unhandledrejection', (e) => log('unhandled: ' + (e.reason && (e.reason.stack || e.reason.message || e.reason))));

// ---------- tiny DOM helper ----------
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
    else if (k in e && k !== 'list') { try { e[k] = v; } catch (_) { e.setAttribute(k, v); } }
    else e.setAttribute(k, v === true ? '' : v);
  }
  for (const c of kids.flat()) if (c != null && c !== false) e.append(c.nodeType ? c : document.createTextNode(c));
  return e;
}
const ICONS = {
  folder: '<path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"/>',
  undo: '<path d="M9 14 4 9l5-5"/><path d="M4 9h10.5a5.5 5.5 0 0 1 0 11H11"/>',
  redo: '<path d="m15 14 5-5-5-5"/><path d="M20 9H9.5a5.5 5.5 0 0 0 0 11H13"/>',
  upload: '<path d="M12 15V4"/><path d="m7 9 5-5 5 5"/><path d="M5 15v3a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2v-3"/>',
  layout: '<rect x="3" y="3" width="18" height="18" rx="2"/><path d="M3 12h18M12 3v18"/>',
  chev: '<path d="m6 9 6 6 6-6"/>',
  skipb: '<path class="f" d="M19 5v14L9 12z"/><path d="M6 5v14"/>',
  skipf: '<path class="f" d="M5 5v14l10-7z"/><path d="M18 5v14"/>',
  play: '<path class="f" d="M7 4.5v15l13-7.5z"/>',
  pause: '<rect class="f" x="6" y="4" width="4" height="16" rx="1"/><rect class="f" x="14" y="4" width="4" height="16" rx="1"/>',
  scissors: '<circle cx="6" cy="6" r="3"/><circle cx="6" cy="18" r="3"/><path d="M20 4 8.1 15.9M14.5 14.5 20 20M8.1 8.1 12 12"/>',
  zin: '<circle cx="11" cy="11" r="8"/><path d="m21 21-4.3-4.3M8 11h6M11 8v6"/>',
  zout: '<circle cx="11" cy="11" r="8"/><path d="m21 21-4.3-4.3M8 11h6"/>',
  image: '<rect x="3" y="3" width="18" height="18" rx="3"/><circle cx="9" cy="9" r="2"/><path d="m21 15-3.1-3.1a2 2 0 0 0-2.8 0L6 21"/>',
  cam: '<path d="m16 13 5.2 3.5a.5.5 0 0 0 .8-.4V7.9a.5.5 0 0 0-.8-.4L16 11"/><rect x="2" y="6" width="14" height="12" rx="2"/>',
  vol: '<path d="M11 5 6 9H2v6h4l5 4z"/><path d="M15.5 8.5a5 5 0 0 1 0 7M19 5a10 10 0 0 1 0 14"/>',
  ptr: '<path d="M4 4l7 17 2.5-7.5L21 11z"/>',
  x: '<path d="M18 6 6 18M6 6l12 12"/>',
  search: '<circle cx="11" cy="11" r="7"/><path d="m21 21-4.3-4.3"/>',
  film: '<rect x="3" y="4" width="18" height="16" rx="2"/><path d="M3 9h18M7 4v5M12 4v5M17 4v5"/>',
  gear: '<circle cx="12" cy="12" r="3"/><path d="M12 2.5v3M12 18.5v3M4.2 4.2l2.1 2.1M17.7 17.7l2.1 2.1M2.5 12h3M18.5 12h3M4.2 19.8l2.1-2.1M17.7 6.3l2.1-2.1"/>',
  back: '<path d="M19 12H5M11 6l-6 6 6 6"/>',
  file: '<path d="M14 3H7a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V8z"/><path d="M14 3v5h5"/>',
  monitor: '<rect x="3" y="4" width="18" height="12" rx="2"/><path d="M8 20h8M12 16v4"/>',
  gauge: '<path d="M4.5 18a9 9 0 1 1 15 0"/><path d="m12 14 4-5"/>',
  gem: '<path d="M6 3h12l4 6-10 12L2 9z"/><path d="M2 9h20M12 21 8 9l4-6 4 6z"/>',
  mic: '<rect x="9" y="3" width="6" height="11" rx="3"/><path d="M5 11a7 7 0 0 0 14 0M12 18v3"/>',
  info: '<circle cx="12" cy="12" r="9"/><path d="M12 11v5M12 8h.01"/>',
  vid: '<rect x="2" y="6" width="14" height="12" rx="2"/><path d="m16 11 6-3v8l-6-3"/>',
};
const ico = (n, s) => `<svg class="i" viewBox="0 0 24 24"${s ? ` style="width:${s}px;height:${s}px"` : ''}>${ICONS[n]}</svg>`;
const I = (n, s) => h('span', { html: ico(n, s), style: 'display:contents' });

// ---------- state ----------
const PRESETS = { // cursor movement styles: add more here ({tension, mass, friction})
  slow: { label: 'Slow', desc: 'Relaxed easing with a gentle follow and higher inertia.', tension: 200, mass: 2.25, friction: 40 },
  smooth: { label: 'Smooth', desc: 'Ultra-smooth cinematic feel with high damping.', tension: 80, mass: 2.5, friction: 28 },
  mellow: { label: 'Mellow', desc: 'Balanced smoothing for everyday tutorials and walkthroughs.', tension: 470, mass: 3, friction: 70 },
  fast: { label: 'Fast', desc: 'Quick, responsive smoothing for fast-paced content.', tension: 380, mass: 1, friction: 30 },
};
const ASPECTS = [[null, 'Auto'], ['wide', 'Wide 16:9'], ['vertical', 'Vertical 9:16'], ['square', 'Square 1:1'], ['classic', 'Classic 4:3']];
const ASPECT_R = { wide: 16 / 9, vertical: 9 / 16, square: 1, classic: 4 / 3 };
const S = {
  proj: null, cfg: null, hist: [], hi: 0, t: 0, playing: false, tab: 'background', sel: null,
  q: 0.65, splitMode: false, zoomV: 0.715, bmp: null, wf: {}, audioReady: false, useAudio: false, bgPending: null, curCustom: false, advShadow: false,
  menu: null, audioSig: '', audioTok: 0,
};
const pps = () => 80 * Math.pow(2, (S.zoomV - 0.715) * 4);
const tl = () => S.cfg.timeline;
function clips() {
  let o = 0;
  return tl().segments.map((s, i) => { const len = (s.end - s.start) / s.timescale; const c = { s, i, off: o, len }; o += len; return c; });
}
const total = () => tl().segments.reduce((a, s) => a + (s.end - s.start) / s.timescale, 0);
const clamp = (v, a, b) => Math.min(b, Math.max(a, v));
function fmtT(t) { t = Math.max(0, t) + 1e-6; const m = Math.floor(t / 60), s = Math.floor(t % 60), f = Math.min(59, Math.floor((t % 1) * 60)); return `${m}:${String(s).padStart(2, '0')}.${String(f).padStart(2, '0')}`; }
function fix(c) {
  c.background ||= {}; c.background.source ||= { type: 'color', value: [255, 255, 255] };
  c.background.advancedShadow ||= { size: 50, opacity: 50, blur: 50 };
  c.camera ||= {}; c.camera.position ||= { x: 'right', y: 'bottom' };
  c.cursor ||= {}; c.audio ||= {}; c.timeline ||= { segments: [], zoomSegments: [] };
  c.timeline.zoomSegments ||= [];
  return c;
}

// ---------- history / save / audio ----------
let saveTimer = 0, audioTimer = 0;
function commit() {
  const s = JSON.stringify(S.cfg);
  if (s === S.hist[S.hi]) return;
  S.hist = S.hist.slice(0, S.hi + 1); S.hist.push(s); S.hi++;
  if (S.hist.length > 300) { S.hist.shift(); S.hi--; }
  afterChange();
}
function afterChange() {
  clearTimeout(saveTimer);
  saveTimer = setTimeout(() => invoke('save', { config: S.cfg }).catch((e) => log('save: ' + e)), 400);
  const sig = JSON.stringify([tl().segments, S.cfg.audio]);
  if (sig !== S.audioSig) { S.audioSig = sig; scheduleAudio(); }
  updUndo(); requestFrame();
}
function restore(i) {
  S.hi = i; S.cfg = JSON.parse(S.hist[i]);
  S.t = clamp(S.t, 0, total());
  if (S.sel && !(S.sel.type === 'zoom' ? tl().zoomSegments[S.sel.i] : tl().segments[S.sel.i])) S.sel = null;
  renderAll(); afterChange();
}
const undo = () => { if (S.hi > 0) restore(S.hi - 1); };
const redo = () => { if (S.hi < S.hist.length - 1) restore(S.hi + 1); };

const aud = $('#aud');
// The backend's audio() renders to one shared temp file, so calls must never overlap (race => ENOENT).
let audioBusy = false, audioPending = false;
function scheduleAudio() {
  S.audioReady = false;
  if (S.playing) switchClock(false);
  clearTimeout(audioTimer);
  S.audioTok++;
  audioTimer = setTimeout(runAudio, 600);
}
async function runAudio() {
  if (audioBusy) { audioPending = true; return; }
  audioBusy = true;
  const tok = S.audioTok;
  try {
    const buf = await invoke('audio', { config: S.cfg });
    if (tok === S.audioTok) {
      const old = aud.src;
      aud.src = URL.createObjectURL(new Blob([toBuf(buf)], { type: 'audio/mp4' }));
      if (old) URL.revokeObjectURL(old);
      await new Promise((res) => { aud.oncanplay = res; aud.onerror = res; setTimeout(res, 3000); });
      if (tok === S.audioTok) {
        S.audioReady = true;
        if (S.playing) { aud.currentTime = S.t; switchClock(true); }
      }
    }
  } catch (e) { log('audio: ' + e); }
  audioBusy = false;
  if (audioPending) { audioPending = false; runAudio(); }
}
const toBuf = (b) => (b instanceof ArrayBuffer ? b : new Uint8Array(b));

// ---------- frame rendering ----------
let inflight = false, dirty = false;
function requestFrame() { dirty = true; pump(); }
function aspectNow() { return S.bmp ? S.bmp.width / S.bmp.height : (ASPECT_R[S.cfg.aspectRatio] || S.proj.source_width / S.proj.source_height); }
async function pump() {
  if (inflight || !dirty || !S.proj) return;
  dirty = false; inflight = true;
  try {
    const w = $('#cwrap'), dpr = window.devicePixelRatio || 1;
    const a = aspectNow();
    const hh = Math.min(w.clientHeight, w.clientWidth / a) * dpr * S.q;
    const buf = await invoke('frame', { t: clamp(S.t, 0, Math.max(0, total() - 0.001)), config: S.cfg, height: clamp(Math.round(hh), 90, 2160) });
    S.bmp = await createImageBitmap(new Blob([toBuf(buf)], { type: 'image/jpeg' }));
    drawFrame();
  } catch (e) { log('frame: ' + e); }
  inflight = false;
  if (dirty) pump();
}
function drawFrame() {
  const cv = $('#cv'), w = $('#cwrap');
  if (!S.bmp) return;
  const dpr = window.devicePixelRatio || 1, a = S.bmp.width / S.bmp.height;
  const cw = Math.max(10, w.clientWidth - 24), ch = Math.max(10, w.clientHeight);
  let fw = Math.min(cw, ch * a), fh = fw / a;
  fw = Math.floor(fw); fh = Math.floor(fh);
  if (cv.style.width !== fw + 'px') cv.style.width = fw + 'px';
  if (cv.style.height !== fh + 'px') cv.style.height = fh + 'px';
  cv.width = Math.round(fw * dpr); cv.height = Math.round(fh * dpr);
  const g = cv.getContext('2d'); g.imageSmoothingQuality = 'high';
  g.drawImage(S.bmp, 0, 0, cv.width, cv.height);
}
let rzT = 0;
new ResizeObserver(() => { drawFrame(); clearTimeout(rzT); rzT = setTimeout(() => { requestFrame(); renderTimeline(); }, 120); }).observe($('#cwrap'));

// ---------- playback ----------
let t0 = 0, tbase = 0;
function switchClock(useAudio) {
  if (useAudio) { S.useAudio = true; aud.play().catch(() => { S.useAudio = false; t0 = performance.now(); tbase = S.t; }); }
  else { S.useAudio = false; aud.pause(); t0 = performance.now(); tbase = S.t; }
}
function play() {
  if (S.playing) return;
  if (S.t >= total() - 0.03) S.t = 0;
  S.playing = true; t0 = performance.now(); tbase = S.t;
  if (S.audioReady) { aud.currentTime = S.t; switchClock(true); } else S.useAudio = false;
  updPlayBtn(); requestAnimationFrame(tick);
}
function pause() { if (!S.playing) return; S.playing = false; aud.pause(); updPlayBtn(); }
function tick() {
  if (!S.playing) return;
  const D = total();
  let t = S.useAudio && !aud.paused && !aud.ended ? aud.currentTime : tbase + (performance.now() - t0) / 1000;
  if (S.useAudio && aud.ended) t = D;
  if (t >= D - 0.01) { S.t = D; pause(); updTime(); updPlayhead(); requestFrame(); return; }
  S.t = t; updTime(); updPlayhead(true); requestFrame();
  requestAnimationFrame(tick);
}
function seek(t) {
  S.t = clamp(t, 0, total());
  if (S.playing) { t0 = performance.now(); tbase = S.t; if (S.useAudio) aud.currentTime = S.t; }
  else if (S.audioReady) aud.currentTime = S.t;
  updTime(); updPlayhead(); requestFrame();
}

// ---------- header ----------
function buildHeader() {
  const hd = $('#hdr'); hd.innerHTML = '';
  const title = h('input#title', { value: S.proj.name, spellcheck: false });
  const fit = () => { const c = fit.c || (fit.c = document.createElement('canvas').getContext('2d')); c.font = '500 13px -apple-system, sans-serif'; title.style.width = Math.ceil(c.measureText(title.value || ' ').width + 10) + 'px'; };
  fit();
  let orig = S.proj.name;
  title.oninput = fit;
  title.onkeydown = (e) => { if (e.key === 'Enter') title.blur(); if (e.key === 'Escape') { title.value = orig; title.blur(); } e.stopPropagation(); };
  title.onblur = () => { const v = title.value.trim(); if (!v) title.value = orig; else if (v !== orig) { orig = v; invoke('rename', { name: v }).catch((e) => log('rename: ' + e)); } fit(); };
  hd.append(title, h('span.ext', { 'data-tauri-drag-region': '' }, '.cap'),
    h('button.ibtn', { title: 'Show in Finder', onclick: () => invoke('reveal', { path: S.proj.path }), html: ico('folder', 17) }),
    h('span.sp', { 'data-tauri-drag-region': '' }),
    h('button.ibtn#undo', { title: 'Undo', onclick: undo, html: ico('undo', 20) }),
    h('button.ibtn#redo', { title: 'Redo', onclick: redo, html: ico('redo', 20) }),
    h('span.vdiv', { 'data-tauri-drag-region': '' }),
    h('button.btn-primary', { onclick: openExport }, I('upload', 16), 'Export'));
  updUndo();
}
function updUndo() { const u = $('#undo'), r = $('#redo'); if (u) u.disabled = S.hi <= 0; if (r) r.disabled = S.hi >= S.hist.length - 1; }

// ---------- menus ----------
function closeMenu() { if (S.menu) { S.menu.remove(); S.menu = null; } }
document.addEventListener('pointerdown', (e) => { if (S.menu && !S.menu.contains(e.target)) closeMenu(); }, true);
function openMenu(anchor, items, cur, pick) {
  closeMenu();
  const r = anchor.getBoundingClientRect();
  const m = h('div.menu', {}, items.map(([v, l]) => h('button', { class: v === cur ? 'on' : '', onclick: () => { closeMenu(); pick(v); } }, l)));
  document.body.append(m);
  const mw = m.offsetWidth, mh = m.offsetHeight;
  m.style.left = clamp(r.left, 6, innerWidth - mw - 6) + 'px';
  m.style.top = (r.bottom + 4 + mh > innerHeight ? r.top - mh - 4 : r.bottom + 4) + 'px';
  S.menu = m;
}
function popup(items, cur, pick, cls = 'pop sel') {
  const b = h('button', { class: cls }, h('span', {}, (items.find(([v]) => v === cur) || items[0])[1]), I('chev', 13));
  b.onclick = () => openMenu(b, items, cur, pick);
  return b;
}

// ---------- preview toolbar + player bar ----------
function buildPreview() {
  const pt = $('#ptool'); pt.innerHTML = '';
  const asp = h('button.pop', {}, I('layout', 16), h('span', {}, (ASPECTS.find(([v]) => v === S.cfg.aspectRatio) || ASPECTS[0])[1]), I('chev', 13));
  asp.onclick = () => openMenu(asp, ASPECTS, S.cfg.aspectRatio, (v) => { S.cfg.aspectRatio = v; commit(); buildPreview(); });
  const q = h('div.seg', {}, [['Full', 1], ['Half', .65], ['Quarter', .25]].map(([l, m]) =>
    h('button', { class: S.q === m ? 'on' : '', onclick: () => { S.q = m; buildPreview(); requestFrame(); } }, l)));
  pt.append(asp, h('span', { style: 'flex:1' }), h('span.lbl', {}, 'Preview'), q);

  const pb = $('#pbar'); pb.innerHTML = '';
  const zs = h('input#zoomsl', { type: 'range', min: 0, max: 1, step: 0.005, value: S.zoomV });
  const paint = () => zs.style.setProperty('--p', zs.value * 100 + '%'); paint();
  zs.oninput = () => { S.zoomV = +zs.value; paint(); renderTimeline(); };
  pb.append(h('span#time'),
    h('div#pctl', {},
      h('button.skip.ibtn', { title: 'Previous clip', onclick: () => { const b = bounds().filter((x) => x < S.t - 0.05); seek(b.length ? b[b.length - 1] : 0); }, html: ico('skipb') }),
      h('button#play', { onclick: () => (S.playing ? pause() : play()) }),
      h('button.skip.ibtn', { title: 'Next clip', onclick: () => { const b = bounds().filter((x) => x > S.t + 0.05); seek(b.length ? b[0] : total()); }, html: ico('skipf') })),
    h('div#pright', {},
      h('button.ibtn#splitbtn', { class: S.splitMode ? 'act' : '', title: 'Toggle split (S)', onclick: () => setSplit(!S.splitMode), html: ico('scissors', 16) }),
      h('span.vdiv', { style: 'margin:0 6px' }),
      h('button.ibtn', { title: 'Zoom out', onclick: () => { zs.value = Math.max(0, zs.value - 0.1); zs.oninput(); }, html: ico('zout', 17) }),
      zs,
      h('button.ibtn', { title: 'Zoom in', onclick: () => { zs.value = Math.min(1, +zs.value + 0.1); zs.oninput(); }, html: ico('zin', 17) })));
  updTime(); updPlayBtn();
}
const bounds = () => clips().map((c) => c.off).concat(total()).filter((x) => x > 0 && x < total() + 1e-6);
function updTime() { const e = $('#time'); if (e) e.innerHTML = `<b>${fmtT(S.t)}</b> / ${fmtT(total())}`; }
function updPlayBtn() { const b = $('#play'); if (b) b.innerHTML = ico(S.playing ? 'pause' : 'play'); }

// ---------- sidebar controls ----------
function sw(on, set) {
  const e = h('button.switch', { class: on ? 'on' : '' });
  e.onclick = () => { on = !on; e.classList.toggle('on', on); set(on); };
  return e;
}
function swRow(label, get, set) { return h('div.row.sw', {}, h('span.l', {}, label), sw(get(), (v) => { set(v); commit(); requestFrame(); })); }
function slider(label, get, set, min, max, step, fmt, extra = {}) {
  const inp = h('input', { type: 'range', min, max, step, value: get() });
  const val = h('span.val', {}, fmt(get()));
  const paint = () => inp.style.setProperty('--p', ((inp.value - min) / (max - min)) * 100 + '%'); paint();
  inp.oninput = () => { const v = +inp.value; set(v); val.textContent = fmt(v); paint(); requestFrame(); if (extra.onInput) extra.onInput(); };
  inp.onchange = () => { commit(); inp.blur(); };
  return h('div.row' + (extra.wide ? '.wide2' : ''), { style: extra.dim ? 'opacity:.45' : '' }, h('span.l', {}, label), inp, val);
}
const pct = (d) => (v) => (+v).toFixed(d) + '%';
const sub = (title, ...k) => h('div.sec', {}, h('div.h', {}, title), ...k);
function seg(items, cur, pick, cls = '') {
  return h('div.seg' + (cls ? '.' + cls : ''), {}, items.map(([v, l]) => h('button', { class: v === cur ? 'on' : '', onclick: () => pick(v) }, l)));
}
const hex = (a) => '#' + a.map((x) => clamp(Math.round(x), 0, 255).toString(16).padStart(2, '0')).join('');
const unhex = (s) => [1, 3, 5].map((i) => parseInt(s.slice(i, i + 2), 16));
function colorIn(get, set) {
  const c = h('input', { type: 'color', value: hex(get()) });
  c.oninput = () => { set(unhex(c.value)); requestFrame(); };
  c.onchange = () => { commit(); renderSidebar(); };
  return c;
}
const cornerPopup = (obj) => popup([['squircle', 'Squircle'], ['rounded', 'Rounded']], obj.roundingType || 'squircle', (v) => { obj.roundingType = v; commit(); renderSidebar(); }, 'pop sel');

function renderTabs() {
  const tabs = [['background', 'image', 'Background'], ['camera', 'cam', 'Camera'], ['audio', 'vol', 'Audio'], ['cursor', 'ptr', 'Cursor']]
    .filter(([k]) => k !== 'camera' || S.proj.segments.some((s) => s.has_camera));
  if (!tabs.some(([k]) => k === S.tab)) S.tab = 'background';
  const el = $('#tabs'); el.innerHTML = '';
  tabs.forEach(([k, ic, name]) => el.append(h('button', { class: S.tab === k && !S.sel ? 'on' : '', title: name, onclick: () => { S.tab = k; S.sel = null; renderAll(true); } }, h('span', { html: ico(ic, 18) }))));
}
function renderSidebar() {
  const b = $('#sbody'), st = b.scrollTop; b.innerHTML = '';
  if (S.sel && S.sel.type === 'zoom') zoomPanel(b);
  else ({ background: bgTab, camera: camTab, audio: audioTab, cursor: cursorTab })[S.tab](b);
  b.scrollTop = st;
}
function shadowBlock(b, obj, label = 'Shadow') {
  const adv = obj.advancedShadow ||= { size: 50, opacity: 50, blur: 50 };
  b.append(slider(label, () => obj.shadow || 0, (v) => (obj.shadow = v), 0, 100, 0.1, pct(1)));
  const head = h('button.advhead' + (S.advShadow ? '.open' : ''), { onclick: () => { S.advShadow = !S.advShadow; renderSidebar(); } }, h('span', {}, 'Advanced shadow settings'), I('chev', 14));
  b.append(head);
  if (S.advShadow) ['size', 'opacity', 'blur'].forEach((k) => b.append(slider(k[0].toUpperCase() + k.slice(1), () => adv[k], (v) => (adv[k] = v), 0, 100, 0.1, pct(1))));
}

const COLOR_PRESETS = ['#ff0000', '#ff4500', '#ff8c00', '#ffd700', '#ffff00', '#adff2f', '#32cd32', '#008000', '#00ced1', '#4785ff', '#0000ff', '#4b0082', '#800080', '#a9a9a9', '#ffffff', '#000000'];
const _unused = ['#ffffff', '#000000', '#1b1b1e', '#3b82f7', '#8b5cf6', '#ec4899', '#f97316', '#22c55e'];
const GRAD_PRESETS = [[[71, 133, 255], [255, 71, 102], 90], [[255, 138, 76], [255, 61, 129], 45], [[34, 211, 238], [99, 102, 241], 135], [[74, 222, 128], [16, 120, 100], 90], [[250, 204, 21], [244, 63, 94], 60], [[30, 30, 40], [90, 90, 120], 180]];
const hasWP = () => typeof WALLPAPERS !== 'undefined' && WALLPAPERS.length;
function bgTab(b) {
  const bg = S.cfg.background, src = bg.source;
  const type = S.bgPending || src.type;
  b.append(h('div.h.top', {}, 'Background'));
  const wp = hasWP(), items = [['image', 'Image'], ['color', 'Color'], ['gradient', 'Gradient']];
  if (wp) items.unshift(['wallpaper', 'Wallpaper']);
  b.append(seg(items, wp || type !== 'wallpaper' ? type : 'image', (v) => {
    S.bgPending = null;
    if (v === 'wallpaper') { if (src.type !== 'wallpaper') { S.bgPending = 'wallpaper'; renderSidebar(); return; } }
    else if (v === 'image') { if (src.type !== 'image') { S.bgPending = 'image'; renderSidebar(); return; } }
    else if (v === 'color' && src.type !== 'color') bg.source = { type: 'color', value: [255, 255, 255] };
    else if (v === 'gradient' && src.type !== 'gradient') bg.source = { type: 'gradient', from: [71, 133, 255], to: [255, 71, 102], angle: 90 };
    commit(); renderSidebar();
  }));
  b.append(h('div', { style: 'height:16px' }));
  const s = bg.source;
  if (type === 'wallpaper' && wp) {
    const cat = WALLPAPERS.find((c) => c.id === S.wpCat) || WALLPAPERS.find((c) => c.items.some((i) => s.path === `wallpapers/${i}.jpg`)) || WALLPAPERS[0];
    b.append(h('div.wpcats', {}, WALLPAPERS.map((c) => h('button', { class: c === cat ? 'on' : '', onclick: () => { S.wpCat = c.id; renderSidebar(); } }, c.name))),
      h('div.wpgrid', {}, cat.items.map((i) => h('button', { class: s.path === `wallpapers/${i}.jpg` ? 'on' : '', title: i, onclick: () => {
        bg.source = { type: 'wallpaper', path: `wallpapers/${i}.jpg` }; S.bgPending = null;
        if (!bg.padding || !bg.rounding) { bg.padding = 10; bg.rounding = 7.5; }
        commit(); renderSidebar();
      } }, h('img', { src: `wallpapers/thumbs/${i}.jpg`, loading: 'eager', draggable: false })))));
  } else if (type === 'image' || type === 'wallpaper') {
    const file = h('input', { type: 'file', accept: 'image/*', class: 'hidden' });
    file.onchange = async () => {
      const f = file.files[0]; if (!f) return;
      try {
        const path = await invoke('save_asset', new Uint8Array(await f.arrayBuffer()), { headers: { name: f.name } });
        bg.source = { type: 'image', path }; S.bgPending = null; commit(); renderSidebar();
      } catch (e) { log('save_asset: ' + e); toast('Could not import image'); }
    };
    const cur = s.path ? s.path.split('/').pop() : '';
    b.append(file, h('button.filebtn', { onclick: () => file.click() }, I('image', 20), h('span', {}, cur ? cur : 'Choose image…'), cur ? h('span', { style: 'font-size:11px' }, 'Click to replace') : null));
  } else if (type === 'color') {
    b.append(h('div.swatches', {}, COLOR_PRESETS.map((c) => h('button.sw1', { class: hex(s.value) === c ? 'on' : '', style: `background:${c}`, onclick: () => { s.value = unhex(c); commit(); renderSidebar(); } })), colorIn(() => s.value, (v) => (s.value = v))));
  } else if (type === 'gradient') {
    b.append(h('div.row.wide', {}, h('span.l', {}, 'From'), colorIn(() => s.from, (v) => (s.from = v))), h('div.row.wide', {}, h('span.l', {}, 'To'), colorIn(() => s.to, (v) => (s.to = v))));
    b.append(slider('Angle', () => s.angle ?? 90, (v) => (s.angle = v), 0, 360, 1, (v) => Math.round(v) + '°'));
    b.append(h('div.swatches', {}, GRAD_PRESETS.map(([f, t, a]) => h('button.sw1', { style: `background:linear-gradient(${a}deg,${hex(f)},${hex(t)})`, onclick: () => { bg.source = { type: 'gradient', from: f, to: t, angle: a }; commit(); renderSidebar(); } }))));
  }
  const lay = sub('Layout');
  lay.append(slider('Padding', () => bg.padding || 0, (v) => (bg.padding = v), 0, 40, 0.1, pct(1)),
    slider('Corners', () => bg.rounding || 0, (v) => (bg.rounding = v), 0, 100, 0.1, pct(1)),
    h('div.row.sw', {}, h('span.l', {}, 'Corner Style'), cornerPopup(bg)));
  shadowBlock(lay, bg);
  b.append(lay);
}
function camTab(b) {
  const c = S.cfg.camera;
  b.append(h('div.h.top', {}, 'Camera'), h('div.h', { style: 'color:var(--txt);margin-bottom:12px' }, 'Position'));
  const pos = c.position, manual = !!c.manualPosition;
  const g = h('div.grid6');
  for (const y of ['top', 'bottom']) for (const x of ['left', 'center', 'right'])
    g.append(h('button', { class: !manual && pos.x === x && pos.y === y ? 'on' : '', title: `${y} ${x}`, onclick: () => { c.position = { x, y }; c.manualPosition = null; commit(); renderSidebar(); } }, h('i')));
  b.append(g);
  if (manual) b.append(h('div.row.wide', {}, h('span.l', { style: 'color:var(--mut);font-size:12px' }, 'Custom position (dragged on canvas)'), h('button.rowbtn', { onclick: () => { c.manualPosition = null; commit(); renderSidebar(); } }, 'Reset')));
  b.append(swRow('Hide Camera', () => c.hide, (v) => (c.hide = v)), swRow('Mirror Camera', () => c.mirror, (v) => (c.mirror = v)),
    slider('Size', () => c.size ?? 30, (v) => (c.size = v), 20, 80, 0.1, pct(1)),
    slider('Corners', () => c.rounding ?? 100, (v) => (c.rounding = v), 0, 100, 0.1, pct(1)),
    h('div.row.sw', {}, h('span.l', {}, 'Corner Style'), cornerPopup(c)));
}
function audioTab(b) {
  const a = S.cfg.audio;
  b.append(h('div.h.top', {}, 'Audio'), swRow('Mute Audio', () => a.mute, (v) => (a.mute = v)));
  b.append(h('div', { style: 'background:#202023;border-radius:12px;padding:14px;margin-bottom:18px;display:flex;gap:12px;align-items:flex-start' },
    h('div', { style: 'width:34px;height:34px;border-radius:9px;background:#1e2c4f;color:#6e97f0;display:flex;align-items:center;justify-content:center;flex:none', html: ico('mic', 17) }),
    h('div', { style: 'flex:1' }, h('div', { style: 'font-size:14px;margin-bottom:3px' }, 'Studio Sound'), h('div', { style: 'color:var(--mut);font-size:12.5px;line-height:1.4' }, 'Reduces background noise and balances your voice level.')),
    sw(!!a.improve, (v) => { a.improve = v; commit(); })));
  b.append(slider('Microphone Volume', () => a.micVolumeDb ?? 0, (v) => (a.micVolumeDb = v), -30, 10, 0.1, (v) => (v <= -30 ? 'Muted' : (v > 0 ? '+' : '') + (+v).toFixed(1) + ' dB'), { dim: a.mute }));
  if (S.proj.segments.some((g) => g.has_system_audio)) b.append(slider('System Audio Volume', () => a.systemVolumeDb ?? 0, (v) => (a.systemVolumeDb = v), -30, 10, 0.1, (v) => (v <= -30 ? 'Muted' : (v > 0 ? '+' : '') + (+v).toFixed(1) + ' dB'), { dim: a.mute }));
}
function curStyle() {
  const c = S.cfg.cursor;
  if (S.curCustom) return 'custom';
  for (const k in PRESETS) if (Math.abs(c.tension - PRESETS[k].tension) <= 1 && Math.abs(c.mass - PRESETS[k].mass) <= 0.05 && Math.abs(c.friction - PRESETS[k].friction) <= 0.2) return k;
  return 'custom';
}
function cursorTab(b) {
  const c = S.cfg.cursor;
  b.append(h('div.row.sw', { style: 'margin-bottom:22px' }, h('span.l', {}, 'Show cursor'), sw(!c.hide, (v) => { c.hide = !v; commit(); requestFrame(); })));
  b.append(slider('Size', () => c.size ?? 100, (v) => (c.size = v), 20, 300, 1, (v) => Math.round(v) + '%'));
  b.append(h('div.h', { style: 'margin-top:24px' }, 'Cursor Movement Style'));
  const cs = curStyle();
  const cards = Object.entries(PRESETS).map(([k, p]) => [k, p.label, p.desc]).concat([['custom', 'Custom', 'Tune tension, friction, and mass manually']]);
  for (const [k, l, d] of cards) b.append(h('button.card', { class: cs === k ? 'on' : '', onclick: () => {
    if (k === 'custom') S.curCustom = true; else { S.curCustom = false; Object.assign(c, { tension: PRESETS[k].tension, mass: PRESETS[k].mass, friction: PRESETS[k].friction }); }
    commit(); renderSidebar(); requestFrame();
  } }, h('b', {}, l), h('span', {}, d)));
  b.append(h('div', { style: 'height:12px' }), h('div.row.sw', {}, h('span.l', {}, 'Smooth Movement'), sw(!c.raw, (v) => { c.raw = !v; commit(); renderSidebar(); requestFrame(); })));
  const onIn = () => { S.curCustom = false; };
  const cardsRefresh = () => { const k = curStyle(); b.querySelectorAll('.card').forEach((el, i) => el.classList.toggle('on', cards[i][0] === k)); };
  const dim = { dim: c.raw, onInput: () => { onIn(); cardsRefresh(); } };
  b.append(slider('Tension', () => c.tension ?? 470, (v) => (c.tension = v), 1, 600, 1, (v) => String(Math.round(v)), dim),
    slider('Friction', () => c.friction ?? 70, (v) => (c.friction = v), 0, 200, 0.1, (v) => (+v).toFixed(1), dim),
    slider('Mass', () => c.mass ?? 3, (v) => (c.mass = v), 0.1, 15, 0.01, (v) => (+v).toFixed(2), dim));
}
function zoomPanel(b) {
  const z = tl().zoomSegments[S.sel.i];
  b.append(h('div.h.top', {}, 'Zoom'));
  b.append(slider('Amount', () => z.amount, (v) => (z.amount = v), 1, 4.5, 0.01, (v) => (+v).toFixed(2) + 'x', { onInput: renderTimelineSoft }));
  b.append(h('div.row', {}, h('span.l', {}, 'Mode'), seg([['auto', 'Auto (follow cursor)'], ['manual', 'Manual']], z.mode === 'auto' || !z.mode ? 'auto' : 'manual', (v) => {
    z.mode = v === 'auto' ? 'auto' : { manual: { x: 0.5, y: 0.5 } }; commit(); renderSidebar();
  })));
  if (z.mode && z.mode.manual) {
    const m = z.mode.manual;
    b.append(slider('X', () => m.x, (v) => (m.x = v), 0, 1, 0.01, (v) => Math.round(v * 100) + '%'), slider('Y', () => m.y, (v) => (m.y = v), 0, 1, 0.01, (v) => Math.round(v * 100) + '%'));
  }
  b.append(h('button.btn2.danger', { onclick: deleteSel }, 'Delete zoom'));
}

// ---------- timeline ----------
const TL_L = 104;
function renderTimelineSoft() { renderTimeline(); }
function renderTimeline() {
  if (!S.cfg) return;
  const sc = $('#tlscroll'), inner = $('#tlinner'), P = pps(), D = total(), L = clips();
  const rowW = Math.max(sc.clientWidth - 24, TL_L + D * P + 30), laneW = rowW - TL_L;
  inner.innerHTML = ''; inner.style.width = rowW + 'px';
  // ruler
  const step = [1, 2, 5, 10, 15, 30, 60, 120, 300, 600].find((s) => s * P >= 56) || 600;
  const rl = h('div.lane', { style: `width:${laneW}px` });
  const maxT = laneW / P;
  for (let s = 0; s <= maxT; s++) {
    if (P < 14 && s % step) continue;
    const mj = s % step === 0;
    rl.append(h('i.tick' + (mj ? '.mj' : ''), { style: `left:${s * P}px` }));
    if (mj) rl.append(h('span.tlab', { style: `left:${s * P}px` }, `${Math.floor(s / 60)}:${String(s % 60).padStart(2, '0')}`));
  }
  const ruler = h('div.tlrow.ruler', { style: `width:${rowW}px` }, h('div', { style: `width:${TL_L}px;flex:none` }), rl);
  rl.onpointerdown = (e) => { S.sel = null; renderAll(true); scrub(e, rl); };
  // video
  const vl = h('div.lane', { style: `width:${laneW}px` });
  vl.onpointerdown = (e) => { if (e.target === vl) { S.sel = null; renderAll(true); scrub(e, vl); } };
  for (const c of L) vl.append(clipEl(c, P));
  const vrow = h('div.tlrow', { style: `width:${rowW}px` }, h('div.lab', {}, h('i', { html: ico('film') }), 'Video'), vl);
  // zoom
  const zl = h('div.lane', { style: `width:${laneW}px` });
  const zs = tl().zoomSegments;
  zs.forEach((z, i) => zl.append(zoomEl(z, i, P)));
  if (!zs.length) zl.append(h('div.zhint', { style: `left:${sc.scrollLeft}px;width:${sc.clientWidth - 24 - TL_L}px` }, 'Generate zoom segments automatically', h('span', {}, '·'), h('button', { onclick: (e) => { e.stopPropagation(); generateZooms(); } }, 'Generate')));
  zl.onpointerdown = (e) => {
    if (!(e.target === zl || e.target.classList.contains('zhint'))) return;
    const lx = zl.getBoundingClientRect().left, ta = (e.clientX - lx) / P;
    drag(e, () => {}, (moved, ev) => {
      if (!moved) return addZoom(ta);
      const tb = (ev.clientX - lx) / P, arr = tl().zoomSegments;
      let a = clamp(Math.min(ta, tb), 0, total()), b = clamp(Math.max(ta, tb), 0, total());
      const lo = Math.max(0, ...arr.filter((q) => q.end <= ta).map((q) => q.end)), hi = Math.min(total(), ...arr.filter((q) => q.start >= ta).map((q) => q.start));
      if (arr.some((q) => ta > q.start && ta < q.end)) return;
      a = Math.max(a, lo); b = Math.min(b, hi);
      if (b - a < 0.3) return;
      arr.push({ start: a, end: b, amount: zoomAmt(), mode: 'auto' }); arr.sort((x, y) => x.start - y.start);
      S.sel = { type: 'zoom', i: arr.findIndex((q) => q.start === a) }; commit(); renderAll(true);
    });
  };
  const zrow = h('div.tlrow', { style: `width:${rowW}px` }, h('div.lab.z', {}, h('i', { html: ico('search') }), 'Zoom'), zl);
  const ph = h('div#ph', {}, h('div#phh'));
  const sl = h('div#splitline'); inner.append(sl); inner.onpointermove = (e) => { const r = inner.getBoundingClientRect(), x = e.clientX - r.left; sl.style.left = x + 'px'; sl.classList.toggle('show', x > TL_L && e.target.closest && !!e.target.closest('.clip')); };
  ph.firstChild.onpointerdown = (e) => scrub(e, rl);
  inner.append(ruler, vrow, zrow, ph);
  updPlayhead();
}
$('#tlscroll').addEventListener('scroll', () => { const z = $('.zhint'); if (z) z.style.left = $('#tlscroll').scrollLeft + 'px'; });
function updPlayhead(follow) {
  const ph = $('#ph'); if (!ph) return;
  const x = TL_L + S.t * pps();
  ph.style.left = x + 'px';
  if (follow) { const sc = $('#tlscroll'); if (x > sc.scrollLeft + sc.clientWidth - 40) sc.scrollLeft = x - 150; else if (x < sc.scrollLeft + 116) sc.scrollLeft = Math.max(0, x - 150); }
}
function drag(e, move, up) {
  e.preventDefault();
  const x0 = e.clientX; let moved = false;
  const mv = (ev) => { const dx = ev.clientX - x0; if (Math.abs(dx) > 2) moved = true; move(dx, ev, moved); };
  const end = (ev) => { removeEventListener('pointermove', mv); removeEventListener('pointerup', end); up && up(moved, ev); };
  addEventListener('pointermove', mv); addEventListener('pointerup', end);
}
function scrub(e, lane) {
  const at = (x) => seek((x - lane.getBoundingClientRect().left) / pps());
  at(e.clientX); drag(e, (dx, ev) => at(ev.clientX));
}
function clipEl(c, P) {
  const { s } = c;
  const el = h('div.seg-block.clip', { class: S.sel && S.sel.type === 'clip' && S.sel.i === c.i ? 'sel' : '', style: `left:${c.off * P}px;width:${c.len * P}px;--pps:${P}px` });
  const cv = h('canvas');
  const chip = h('button.chip', { title: 'Clip settings' }, I('gear', 13), `${+s.timescale.toFixed(2)}x`);
  chip.onpointerdown = (e) => e.stopPropagation();
  chip.onclick = (e) => { e.stopPropagation(); openMenu(chip, [0.25, 0.5, 1, 1.5, 2, 4, 8].map((v) => [v, v + 'x']), s.timescale, (v) => { s.timescale = v; clampZooms(); commit(); S.t = clamp(S.t, 0, total()); renderTimeline(); updTime(); }); };
  el.append(cv, h('div.meta', {}, h('span.nm', {}, `Clip ${s.recordingSegment}`), h('span.du', {}, Math.round(c.len) + 's'), chip));
  const hl = h('div.hd.l'), hr = h('div.hd.r');
  el.append(hl, hr);
  hl.onpointerdown = (e) => { e.stopPropagation(); trim(e, c, 'l'); };
  hr.onpointerdown = (e) => { e.stopPropagation(); trim(e, c, 'r'); };
  el.onpointerdown = (e) => {
    if (S.splitMode) { e.preventDefault(); splitAt((e.clientX - el.parentNode.getBoundingClientRect().left) / P); return; }
    if (e.target !== el && !e.target.classList.contains('meta') && !e.target.classList.contains('nm') && !e.target.classList.contains('du')) return;
    S.sel = { type: 'clip', i: c.i }; renderAll(true);
    scrub(e, el.parentNode);
  };
  requestAnimationFrame(() => drawWave(cv, c, P));
  return el;
}
async function drawWave(cv, c, P) {
  const seg = c.s.recordingSegment;
  if (!S.proj.segments[seg] || !S.proj.segments[seg].has_mic) return;
  if (!(seg in S.wf)) {
    S.wf[seg] = null;
    try { S.wf[seg] = await invoke('waveform', { segment: seg }) || false; } catch (e) { log('waveform: ' + e); S.wf[seg] = false; }
    renderTimeline(); return;
  }
  const w = S.wf[seg]; if (!w || !cv.isConnected) return;
  const W = Math.max(1, Math.round(c.len * P)), H = 20, dpr = 2;
  cv.width = Math.min(W, 8000) * dpr; cv.height = H * dpr;
  const g = cv.getContext('2d'); g.scale(dpr * Math.min(W, 8000) / W, dpr);
  g.fillStyle = 'rgba(72,120,235,.6)';
  for (let x = 0; x < W; x++) {
    const a = Math.floor((c.s.start + (x / P) * c.s.timescale + w.offset) / w.step), b = Math.max(a, Math.floor((c.s.start + ((x + 1) / P) * c.s.timescale + w.offset) / w.step));
    let m = 0; for (let k = a; k <= b; k++) m = Math.max(m, w.peaks[k] || 0);
    const hh = Math.min(H, Math.pow(m, 0.6) * H * 1.3);
    if (hh > 0.5) g.fillRect(x, H - hh, 1, hh);
  }
}
function trim(e, c, side) {
  const s = c.s, P = pps(), os = s.start, oe = s.end, dur = S.proj.segments[s.recordingSegment].duration || oe;
  S.sel = { type: 'clip', i: c.i };
  drag(e, (dx) => {
    const d = (dx / P) * s.timescale;
    if (side === 'l') s.start = clamp(os + d, 0, oe - 0.2); else s.end = clamp(oe + d, os + 0.2, dur);
    clampZooms(); S.t = clamp(S.t, 0, total()); renderTimeline(); updTime(); requestFrame();
  }, () => { commit(); renderAll(); });
}
function clampZooms() {
  const D = total(), z = tl().zoomSegments;
  for (let i = z.length - 1; i >= 0; i--) { z[i].end = Math.min(z[i].end, D); if (z[i].start >= D || z[i].end - z[i].start < 0.1) z.splice(i, 1); }
  if (S.sel && S.sel.type === 'zoom' && !z[S.sel.i]) S.sel = null;
}
function zoomEl(z, i, P) {
  const el = h('div.seg-block.zseg', { class: S.sel && S.sel.type === 'zoom' && S.sel.i === i ? 'sel' : '', style: `left:${z.start * P}px;width:${(z.end - z.start) * P}px` },
    h('span.nm', {}, 'Zoom'), h('span.du', {}, `${(+z.amount).toFixed(1)}x`), h('div.hd.l'), h('div.hd.r'));
  el.onclick = (e) => e.stopPropagation();
  el.onpointerdown = (e) => {
    e.stopPropagation();
    const arr = tl().zoomSegments, mode = e.target.classList.contains('hd') ? (e.target.classList.contains('l') ? 'l' : 'r') : 'm';
    const o = { s: z.start, e: z.end }, prev = arr[i - 1] ? arr[i - 1].end : 0, next = arr[i + 1] ? arr[i + 1].start : total(), len = z.end - z.start;
    S.sel = { type: 'zoom', i }; renderSidebar(); renderTabs(); el.classList.add('sel');
    drag(e, (dx) => {
      const d = dx / P;
      if (mode === 'm') { z.start = clamp(o.s + d, prev, next - len); z.end = z.start + len; }
      else if (mode === 'l') z.start = clamp(o.s + d, prev, o.e - 0.3);
      else z.end = clamp(o.e + d, o.s + 0.3, next);
      el.style.left = z.start * P + 'px'; el.style.width = (z.end - z.start) * P + 'px'; requestFrame();
    }, (moved) => { if (moved) commit(); });
  };
  return el;
}
const zoomAmt = () => +S.proj.defaultZoomAmount || 1.5;
function addZoom(tc) {
  const D = total(), z = tl().zoomSegments;
  let st = tc; if (st + 2 > D) st = D - 2;
  if (st < 0 || z.some((q) => st < q.end && st + 2 > q.start)) return;
  z.push({ start: st, end: st + 2, amount: zoomAmt(), mode: 'auto' });
  z.sort((a, b) => a.start - b.start);
  S.sel = { type: 'zoom', i: z.findIndex((q) => q.start === st) };
  commit(); renderAll(true);
}
async function generateZooms() {
  try {
    const r = await invoke('generate_zooms', { config: S.cfg });
    if (!r || !r.length) { toast('No zooms generated'); return; }
    tl().zoomSegments = r; S.sel = null; commit(); renderAll(true);
  } catch (e) { log('generate_zooms: ' + e); toast('Could not generate zooms'); }
}
function setSplit(on) {
  S.splitMode = on; document.body.classList.toggle('splitmode', on);
  const b = $('#splitbtn'); if (b) b.classList.toggle('act', on);
}
function split() { splitAt(S.t); }
function splitAt(t) {
  const c = clips().find((c) => t >= c.off && t < c.off + c.len + 1e-9);
  if (!c) return;
  const sp = c.s.start + (t - c.off) * c.s.timescale;
  setSplit(false);
  if (sp - c.s.start < 0.1 || c.s.end - sp < 0.1) return;
  const segs = tl().segments;
  segs.splice(c.i, 1, { ...c.s, end: sp }, { ...c.s, start: sp });
  S.sel = { type: 'clip', i: c.i + 1 };
  commit(); renderAll(true);
}
function deleteSel() {
  if (!S.sel) return;
  if (S.sel.type === 'zoom') tl().zoomSegments.splice(S.sel.i, 1);
  else { if (tl().segments.length <= 1) return; tl().segments.splice(S.sel.i, 1); clampZooms(); S.t = clamp(S.t, 0, total()); }
  S.sel = null; commit(); renderAll(true);
}

// ---------- toast ----------
let toastT = 0;
function toast(m) { const t = $('#toast'); t.textContent = m; t.classList.add('show'); clearTimeout(toastT); toastT = setTimeout(() => t.classList.remove('show'), 2200); }

// ---------- layout / splitter ----------
$('#split').onpointerdown = (e) => {
  const tlEl = $('#tl'), h0 = tlEl.offsetHeight;
  drag(e, (dx, ev) => {
    tlEl.style.height = clamp(h0 - (ev.clientY - e.clientY), 120, innerHeight - 40 - 12 - 8 - 200) + 'px';
  }, () => { renderTimeline(); drawFrame(); requestFrame(); });
};
function renderAll(keepPreview) {
  renderTabs(); renderSidebar(); renderTimeline(); updTime(); updUndo();
  if (!keepPreview) buildPreview();
}

// ---------- keyboard ----------
addEventListener('keydown', (e) => {
  if ($('#export') && !$('#export').classList.contains('hidden')) return;
  const el = e.target, typing = el.tagName === 'TEXTAREA' || (el.tagName === 'INPUT' && !['range', 'color', 'checkbox'].includes(el.type));
  if (typing) return;
  const mod = e.metaKey || e.ctrlKey, k = e.key;
  let used = true;
  if (mod && k.toLowerCase() === 'z') (e.shiftKey ? redo : undo)();
  else if (mod && k.toLowerCase() === 'y') redo();
  else if (mod && (k === '=' || k === '+' || k === '-')) { const z = $('#zoomsl'); z.value = clamp(+z.value + (k === '-' ? -0.08 : 0.08), 0, 1); z.oninput(); }
  else if (mod) used = false;
  else if (k === ' ') S.playing ? pause() : play();
  else if (k.toLowerCase() === 's') setSplit(!S.splitMode);
  else if (k.toLowerCase() === 'c') split();
  else if (k === 'Delete' || k === 'Backspace') deleteSel();
  else if (k === 'ArrowLeft') seek(S.t - (e.shiftKey ? 1 : 1 / S.proj.fps));
  else if (k === 'ArrowRight') seek(S.t + (e.shiftKey ? 1 : 1 / S.proj.fps));
  else if (k === 'Escape') { closeMenu(); if (S.splitMode) setSplit(false); else if (S.sel) { S.sel = null; renderAll(true); } }
  else used = false;
  if (used) e.preventDefault();
});

// ---------- export page ----------
const E = { fmt: 'mp4', res: 1080, fps: 30, q: 'Maximum', path: '', running: false, done: null, err: '', prog: [0, 1], built: false };
const QUALITY = { Potato: [32, 2], Web: [27, 4], Social: [23, 8], Maximum: [18, 16] }; // [crf, ~Mbps @1080p30]
function outSize() {
  // Same math as the backend (export.rs output_size), not the preview bitmap, which is rounded at a smaller size.
  const a = ASPECT_R[S.cfg.aspectRatio] || S.proj.source_width / S.proj.source_height; const hh = E.res; const w = Math.max(2, Math.round((hh * a) / 2) * 2); return [w, hh];
}
function fmtSize(mb) { return mb >= 1000 ? (mb / 1000).toFixed(1) + ' GB' : Math.max(1, Math.round(mb)) + ' MB'; }
function exSeg(items, cur, pick) { return seg(items, cur, pick, E.running ? 'dis' : ''); }
function renderExport() {
  const ex = $('#export'); ex.innerHTML = '';
  const [w, hh] = outSize(), D = total();
  const gif = E.fmt === 'gif';
  // GIF: ~0.04 byte per pixel per frame (a palette per frame, so screen content is cheap but not free).
  const mb = gif ? (0.04 * w * hh * E.fps * D) / 1e6 : (QUALITY[E.q][1] * (w * hh) / (1920 * 1080) * Math.pow(E.fps / 30, 0.7) * D) / 8;
  const hd = h('header', { 'data-tauri-drag-region': '' },
    h('button.back', { onclick: closeExport }, I('back', 16), 'Back to editor'), h('div.ttl', {}, 'Export'));
  const sec = (ic, t) => h('h4', {}, I(ic, 17), t);
  const re = (fn) => (v) => { fn(v); renderExport(); };
  const opts = h('div#exopts', {},
    sec('upload', 'Destination'), exSeg([['file', 'File']], 'file', () => {}),
    h('input#expath', { value: E.path, disabled: E.running, spellcheck: false, oninput: (e) => (E.path = e.target.value) }),
    sec('vid', 'Format'), exSeg([['mp4', 'MP4'], ['gif', 'GIF']], E.fmt, re((v) => { E.fmt = v; E.path = E.path.replace(/(\.\w+)?$/, '.' + v); E.fps = v === 'gif' ? 15 : Math.max(E.fps, 15); })),
    sec('monitor', 'Resolution'), exSeg([[720, '720p'], [1080, '1080p'], [2160, '4K']], E.res, re((v) => (E.res = v))),
    sec('gauge', 'Frame rate'), exSeg((gif ? [10, 15, 30] : [15, 30, 60]).map((f) => [f, f + ' FPS']), E.fps, re((v) => (E.fps = v))),
    ...(gif ? [] : [sec('gem', 'Quality'), exSeg(Object.keys(QUALITY).map((k) => [k, k]), E.q, re((v) => (E.q = v))),
      h('div.cap', {}, h('span', {}, 'Smaller file'), h('span', {}, 'Larger file'))]));
  const foot = h('div#exfoot', {});
  if (E.running) {
    const p = Math.round((E.prog[0] / Math.max(1, E.prog[1])) * 100);
    foot.append(h('div#exprog', {}, h('div', { style: 'display:flex;justify-content:space-between' }, h('span', {}, E.fmt === 'gif' ? 'Rendering GIF' : 'Exporting…'), h('span#expct', {}, p + '%')), h('div.bar', {}, h('i', { style: `width:${p}%` }))));
  } else if (E.err) foot.append(h('div#exprog.err', {}, E.err));
  else if (E.done) foot.append(h('div#exprog.ok', {}, h('span', {}, 'Exported'), h('button.btn2', { onclick: () => invoke('reveal', { path: E.done }) }, 'Show in Finder')));
  foot.append(h('button#exgo', { disabled: E.running, onclick: runExport }, I('file', 17), 'Export to File'));
  const stats = h('div#stats', {}, [['Duration', fmtT(D).replace(/\.\d+$/, '')], ['Output', `${w}×${hh} · ${E.fps} fps`], ['Estimated size', '~' + fmtSize(mb)]].map(([a, b]) => h('div', {}, h('small', {}, a), h('b', {}, b))));
  const left = h('div#exleft', {}, h('div.pv', {}, 'Preview'), h('div#excv', {}, h('canvas#excanvas')), stats);
  ex.append(hd, h('div#exbody', {}, left, h('div#exright', {}, opts, foot)));
  if (S.exBmp) drawEx();
}
function drawEx() {
  const cv = $('#excanvas'), box = $('#excv'); if (!cv || !S.exBmp) return;
  const a = S.exBmp.width / S.exBmp.height, dpr = devicePixelRatio || 1;
  const fw = Math.floor(Math.min(box.clientWidth, box.clientHeight * a)), fh = Math.floor(fw / a);
  cv.style.width = fw + 'px'; cv.style.height = fh + 'px'; cv.width = fw * dpr; cv.height = fh * dpr;
  cv.getContext('2d').drawImage(S.exBmp, 0, 0, cv.width, cv.height);
}
async function openExport() {
  pause(); closeMenu();
  $('#editor').classList.add('hidden'); $('#export').classList.remove('hidden');
  if (!E.path) { try { E.path = await invoke('default_export_path'); } catch (e) { log('default_export_path: ' + e); } }
  if (!E.running) { E.done = null; E.err = ''; }
  renderExport();
  try {
    const buf = await invoke('frame', { t: clamp(S.t, 0, Math.max(0, total() - 0.001)), config: S.cfg, height: Math.min(E.res, 1080) });
    S.exBmp = await createImageBitmap(new Blob([toBuf(buf)], { type: 'image/jpeg' })); drawEx();
  } catch (e) { log('export preview: ' + e); }
}
function closeExport() { $('#export').classList.add('hidden'); $('#editor').classList.remove('hidden'); drawFrame(); renderTimeline(); requestFrame(); }
async function runExport() {
  if (E.running) return;
  E.running = true; E.done = null; E.err = ''; E.prog = [0, 1]; renderExport();
  try {
    E.done = await invoke('export', { config: S.cfg, fps: E.fps, height: E.res, crf: QUALITY[E.q][0], path: E.path });
  } catch (e) { E.err = String(e); log('export: ' + e); }
  E.running = false; renderExport();
}
listen('export-progress', (e) => {
  E.prog = e.payload; if (!E.running) return;
  const p = Math.round((E.prog[0] / Math.max(1, E.prog[1])) * 100);
  const a = $('#expct'), b = $('#exprog .bar i'); if (a) a.textContent = p + '%'; if (b) b.style.width = p + '%';
});
addEventListener('resize', () => { drawEx(); });

// ---------- boot ----------
(async function init() {
  try {
    S.proj = await invoke('project');
    S.cfg = fix(S.proj.config);
    S.hist = [JSON.stringify(S.cfg)]; S.hi = 0;
    S.audioSig = JSON.stringify([tl().segments, S.cfg.audio]);
    buildHeader(); renderAll();
    requestFrame(); scheduleAudio();
  } catch (e) { log('init: ' + (e && e.stack || e)); }
})();
