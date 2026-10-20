'use strict';
// Target overlay: display / area / camera-only. Holds the Start Recording card.
const Q = new URLSearchParams(location.search);
let KIND = Q.get('kind') || 'display', MODE = MODES[Q.get('mode')] ? Q.get('mode') : getMode();
const DISPLAY = parseInt(Q.get('display') || '0', 10);
const DW = parseFloat(Q.get('w')) || innerWidth, DH = parseFloat(Q.get('h')) || innerHeight, DNAME = Q.get('name') || 'Display';
const root = $('#root'); root.className = KIND;
let onCtl = null; // area: re-place the card when it changes size
let SET = { recordingCountdown: 3, confirmBeforeRecordingWithoutMicrophone: true }; // from get_settings
const S = { dev: { screens: [], cameras: [], mics: [] }, sel: [null, null] };
let area = null; // {x,y,w,h} in window px

addEventListener('keydown', (e) => { if (e.key === 'Escape') { if (guardEl) closeGuard(); else if (curMenu) closeMenu(); else curWin.close(); } });
document.addEventListener('pointerdown', (e) => { if (guardEl && !guardEl.contains(e.target)) closeGuard(); }, true);

async function refresh() { try { S.dev = await invoke('devices'); S.sel = await invoke('selection'); } catch (e) { log('devices: ' + e); } }
let toastT = 0;
function toast(msg) {
  let t = $('.etoast'); if (!t) { t = h('div.etoast'); document.body.append(t); }
  t.textContent = msg; t.style.display = 'block'; clearTimeout(toastT); toastT = setTimeout(() => (t.style.display = 'none'), 6000);
}
let guardEl = null;
function closeGuard() { if (guardEl) { guardEl.remove(); guardEl = null; } }
// No-mic guard popover under the Start capsule.
function guard(target) {
  closeGuard();
  const r = $('.sw').getBoundingClientRect(), cb = h('input', { type: 'checkbox' }), er = h('div.gerr');
  const go = async () => {
    if (cb.checked) { try { SET = await setSettings({ confirmBeforeRecordingWithoutMicrophone: false }); } catch (e) { er.textContent = 'Could not save your preference. Try again or leave the box unchecked.'; return; } }
    closeGuard(); start(target, true);
  };
  guardEl = h('div.gpop', {}, h('div.gr', {}, I('warn', 16), h('div', {}, h('p.gt', {}, 'No microphone detected'), h('p.gd', {}, 'This recording will not include your voice. Select a microphone, or continue without one.'))),
    h('label.gl', {}, cb, "Don't show again"), er,
    h('div.gf', {}, h('button.g1', { onclick: closeGuard }, 'Go back'), h('button.g2', { onclick: go }, 'Record without microphone')));
  document.body.append(guardEl);
  const w = guardEl.offsetWidth, hh = guardEl.offsetHeight;
  guardEl.style.left = Math.max(8, Math.min(innerWidth - w - 8, r.left + r.width / 2 - w / 2)) + 'px';
  guardEl.style.top = (r.bottom + 8 + hh > innerHeight ? Math.max(8, r.top - hh - 8) : r.bottom + 8) + 'px';
}
async function start(target, skipGuard) {
  if (MODE !== 'screenshot' && !skipGuard && SET.confirmBeforeRecordingWithoutMicrophone && !S.sel[1]) return guard(target);
  try {
    if (MODE === 'screenshot') await invoke('take_screenshot', { target });
    else await invoke('prepare_recording', { target, countdown: SET.recordingCountdown, mode: MODE });
  } catch (e) { log('start: ' + e); toast(String(e)); }
}

// Controls card: X, Start Recording, countdown, camera + mic selects.
function ctlCard(opts) {
  const el = h('div.ctl', {});
  const draw = () => {
    el.innerHTML = '';
    const cn = (S.dev.cameras.find((c) => c.id === S.sel[0]) || {}).name || (S.sel[0] ? 'Camera' : null);
    const mn = (S.dev.mics.find((c) => c.id === S.sel[1]) || {}).name || (S.sel[1] ? 'Microphone' : null);
    const shot = MODE === 'screenshot', need = opts.needCamera && (!S.sel[0] || shot);
    const cd = SET.recordingCountdown;
    if (guardEl && S.sel[1]) closeGuard();
    const cdb = h('button.cdbtn', { title: 'Recording countdown', html: cd ? `<b style="font-weight:600;font-size:13px">${cd}</b>` : ico('timer', 17) });
    cdb.onclick = () => openMenu(cdb, COUNTDOWNS, cd, (v) => setSettings({ recordingCountdown: v }).then((n) => { SET = n; draw(); }, (e) => toast(String(e))), 'Recording Countdown');
    const row = h('div.r1', {},
      h('button.xbtn', { title: 'Close', html: ico('x', 17), onclick: () => curWin.close() }),
      h('div.sw', { class: need ? 'dis' : '' },
        h('button.start', { disabled: need, onclick: () => start(opts.target()) }, I(MODES[MODE][1]), h('div', {}, h('b', {}, shot ? 'Take Screenshot' : 'Start Recording'), h('small', {}, MODES[MODE][0]))),
        h('button.caret', { title: 'Choose recording mode', 'aria-label': 'Choose recording mode', html: ico('chev', 14),
          onclick: (e) => openMenu(e.currentTarget, MENU_MODES.map((m) => [m, MODES[m][0]]), MODE, (v) => { MODE = v; setMode(v); draw(); }) })),
      cdb);
    const dev = (icon, name, none, id, items, set) => {
      const b = h('button.dev', {}, I(icon, 16), h('span.nm', {}, name || none), h('span.pill', { class: id ? 'on' : '' }, id ? 'On' : 'Off'));
      b.onclick = () => openMenu(b, [[null, none]].concat(items.map((d) => [d.id, d.name])), id, async (v) => { try { await set(v); } catch (e) { log(String(e)); } await refresh(); draw(); });
      return b;
    };
    el.append(h('div.panel.glass', {}, row),
      ...(shot ? [] : [h('div.panel.glass', {}, h('div.devs', {},
        dev('cam', cn, 'No Camera', S.sel[0], S.dev.cameras, (id) => invoke('set_camera', { id })),
        dev('mic', mn, 'No Microphone', S.sel[1], S.dev.mics, (id) => invoke('set_mic', { id }))))]),
      ...(need ? [h('div.hint', {}, shot ? 'Camera Only is not available for screenshots' : 'Select a camera to record with Camera Only')] : []));
    if (opts.after) opts.after();
    if (onCtl) onCtl();
  };
  el.redraw = draw; el.target = opts.target; draw(); return el;
}

// Live camera preview (Camera Only): polls the shared camera_frame.
function previewLoop(box) {
  const cv = h('canvas', { width: 480, height: 270 }); const msg = h('span', {}, 'Loading camera…');
  box.append(msg, cv); cv.style.display = 'none';
  let stop = false;
  (async function tick() {
    if (stop || !document.body.contains(box)) return;
    try {
      if (S.sel[0]) {
        const buf = await invoke('camera_frame');
        if (buf && buf.byteLength) {
          const bmp = await createImageBitmap(new Blob([buf], { type: 'image/jpeg' }));
          const g = cv.getContext('2d'), s = Math.min(bmp.width, bmp.height * 16 / 9), sh = s * 9 / 16;
          g.drawImage(bmp, (bmp.width - s) / 2, (bmp.height - sh) / 2, s, sh, 0, 0, cv.width, cv.height);
          cv.style.display = ''; msg.style.display = 'none';
        }
      } else { cv.style.display = 'none'; msg.style.display = ''; msg.textContent = 'No camera selected'; }
    } catch (_) {}
    setTimeout(tick, 60);
  })();
}

// ---------- display ----------
function displayMode() {
  const scr = S.dev.screens[DISPLAY], sc = scr ? scr.scale : 1;
  const card = ctlCard({ target: () => ({ display: DISPLAY, area: null, window: null, camera_only: false }) });
  root.append(h('div.center', {}, h('div.ic', { html: ico('monitor') }), h('h1', {}, DNAME),
    h('p', {}, `${Math.round(DW * sc)}x${Math.round(DH * sc)}`), card));
}
// ---------- camera only ----------
function cameraMode() {
  const pv = h('div.pv');
  const card = ctlCard({ needCamera: true, target: () => ({ display: DISPLAY, area: null, window: null, camera_only: true }), after: () => { pv.innerHTML = ''; previewLoop(pv); } });
  root.append(h('div.center', {}, h('h2', {}, 'Camera Only'), h('p', { style: 'margin:4px 0 0' }, 'Record using only your camera and microphone'), pv, card));
}

// ---------- area ----------
const mn = () => (MODE === 'screenshot' ? 1 : 150);
let ratio = 0; // 0 = free
const RATIOS = [[0, 'Free'], [16 / 9, '16:9'], [4 / 3, '4:3'], [1, '1:1']];
function areaMode(init) {
  const dim = h('div#dim'), sel = h('div#sel'), hint = h('div#ahint', {}, 'Click and drag to select an area'), err = h('div#err');
  const ro = h('div.ro', {}, 'Draw an area');
  const chips = h('div.chips');
  const tb = h('div#toolbar.glass', {}, ro, chips,
    h('div.sep'),
    h('button.ib', { title: 'Reset selection', html: ico('reset', 17), onclick: () => { area = null; sync(); } }),
    h('button.ib', { title: 'Fill display', html: ico('fill', 17), onclick: () => { area = { x: 0, y: 0, w: innerWidth, h: innerHeight }; sync(); } }));
  const card = ctlCard({ target: () => { const k = DW / innerWidth; return { display: DISPLAY, area: { x: Math.round(area.x * k), y: Math.round(area.y * k), w: Math.round(area.w * k), h: Math.round(area.h * k) }, window: null, camera_only: false }; } });
  const actl = h('div#actl', {}, card);
  for (const hn of ['nw', 'n', 'ne', 'e', 'se', 's', 'sw', 'w']) {
    const hd = h('div.hd', { 'data-h': hn, style: `cursor:${hn === 'n' || hn === 's' ? 'ns' : hn === 'e' || hn === 'w' ? 'ew' : hn === 'nw' || hn === 'se' ? 'nwse' : 'nesw'}-resize` });
    hd.onpointerdown = (e) => { e.stopPropagation(); begin(e, hn); };
    sel.append(hd);
  }
  const pillsz = h('div.pillsz'); sel.append(pillsz);
  sel.onpointerdown = (e) => { if (e.target === sel || e.target === pillsz) begin(e, 'move'); };
  root.append(dim, sel, hint, err, tb, actl);
  const drawChips = () => { chips.innerHTML = ''; for (const [r, l] of RATIOS) chips.append(h('button', { class: ratio === r ? 'on' : '', onclick: () => { ratio = r; if (area && r) applyRatio(); drawChips(); sync(); } }, l)); };
  drawChips();

  const clampRect = (r) => {
    r.w = Math.min(r.w, innerWidth); r.h = Math.min(r.h, innerHeight);
    r.x = Math.max(0, Math.min(r.x, innerWidth - r.w)); r.y = Math.max(0, Math.min(r.y, innerHeight - r.h)); return r;
  };
  function applyRatio() { // refit the existing selection to the ratio, keeping its centre
    const cx = area.x + area.w / 2, cy = area.y + area.h / 2;
    let w = area.w, hh = w / ratio;
    if (hh > innerHeight) { hh = innerHeight; w = hh * ratio; }
    if (w > innerWidth) { w = innerWidth; hh = w / ratio; }
    area = clampRect({ x: cx - w / 2, y: cy - hh / 2, w, h: hh });
  }
  function sync(small) {
    sel.style.display = area ? 'block' : 'none';
    hint.style.display = area ? 'none' : '';
    ro.textContent = area ? `${Math.round(area.w * DW / innerWidth)} × ${Math.round(area.h * DH / innerHeight)}` : 'Draw an area';
    sel.classList.toggle('bad', !!small);
    if (area) {
      Object.assign(sel.style, { left: area.x + 'px', top: area.y + 'px', width: area.w + 'px', height: area.h + 'px' });
      const P = { nw: [0, 0], n: [.5, 0], ne: [1, 0], e: [1, .5], se: [1, 1], s: [.5, 1], sw: [0, 1], w: [0, .5] };
      for (const hd of sel.querySelectorAll('.hd')) { const [fx, fy] = P[hd.dataset.h]; hd.style.left = `calc(${fx * 100}% - 7px)`; hd.style.top = `calc(${fy * 100}% - 7px)`; }
      pillsz.textContent = ro.textContent;
      err.style.display = small ? 'block' : 'none';
      err.textContent = small ? `Minimum size is ${mn()} x ${mn()}` : '';
    }
    placeCard();
  }
  function placeCard() {
    if (!area || drag) { actl.style.display = 'none'; return; }
    actl.style.display = 'block';
    const cw = actl.offsetWidth, ch = actl.offsetHeight;
    let top = area.y + area.h + 16;
    if (top + ch > innerHeight - 8) top = area.y - ch - 16;
    if (top < 104) top = Math.min(innerHeight - ch - 8, area.y + area.h - ch - 16); // keep clear of the toolbar
    actl.style.left = Math.max(8, Math.min(innerWidth - cw - 8, area.x + area.w / 2 - cw / 2)) + 'px';
    actl.style.top = Math.max(8, top) + 'px';
  }
  let drag = null, drag0 = null;
  function begin(e, mode) {
    e.preventDefault(); closeMenu();
    const o = area ? { ...area } : null, p0 = { x: e.clientX, y: e.clientY };
    drag = { mode }; drag0 = mode;
    const move = (ev) => {
      const px = Math.max(0, Math.min(innerWidth, ev.clientX)), py = Math.max(0, Math.min(innerHeight, ev.clientY));
      if (mode === 'draw') area = fromCorner(p0.x, p0.y, px, py, false);
      else if (mode === 'move') area = clampRect({ ...o, x: o.x + ev.clientX - p0.x, y: o.y + ev.clientY - p0.y });
      else area = resize(o, mode, px, py);
      sync(mode === 'draw' && area && (area.w < mn() || area.h < mn()));
    };
    const up = () => {
      removeEventListener('pointermove', move); removeEventListener('pointerup', up);
      drag = null;
      const m0 = drag0; drag0 = null;
      if (area && (area.w < mn() || area.h < mn())) { // too small: discard (a bare click clears silently)
        const tooSmall = Math.hypot(area.w, area.h) > 6; const bad = tooSmall;
        area = null; sync(); if (bad) { err.style.display = 'block'; err.textContent = `Minimum size is ${mn()} x ${mn()}`; setTimeout(() => (err.style.display = 'none'), 1600); }
      } else { sync(); if (area && MODE === 'screenshot' && m0) start(card.target()); } // screenshot fires on release
    };
    addEventListener('pointermove', move); addEventListener('pointerup', up);
  }
  dim.onpointerdown = (e) => { area = null; sync(); begin(e, 'draw'); };
  function fromCorner(ax, ay, px, py, noMin) {
    let w = Math.abs(px - ax), hh = Math.abs(py - ay);
    if (ratio) { hh = w / ratio; const maxH = py >= ay ? innerHeight - ay : ay; if (hh > maxH) { hh = maxH; w = hh * ratio; } }
    return { x: px >= ax ? ax : ax - w, y: py >= ay ? ay : ay - hh, w, h: hh };
  }
  function resize(o, hn, px, py) {
    let l = o.x, t = o.y, r = o.x + o.w, b = o.y + o.h;
    if (hn.includes('w')) l = Math.min(px, r - mn()); if (hn.includes('e')) r = Math.max(px, l + mn());
    if (hn.includes('n')) t = Math.min(py, b - mn()); if (hn.includes('s')) b = Math.max(py, t + mn());
    let w = r - l, hh = b - t;
    if (ratio) {
      if (hn.length === 2) { hh = w / ratio; if (hn.includes('n')) t = b - hh; else b = t + hh; }
      else if (hn === 'n' || hn === 's') { w = hh * ratio; const cx = o.x + o.w / 2; l = cx - w / 2; r = cx + w / 2; }
      else { hh = w / ratio; const cy = o.y + o.h / 2; t = cy - hh / 2; b = cy + hh / 2; }
    }
    const res = { x: l, y: t, w: r - l, h: b - t };
    if (res.x < 0 || res.y < 0 || res.x + res.w > innerWidth || res.y + res.h > innerHeight) return clampRect(ratio ? o && { ...o } : res);
    return res;
  }
  if (init) area = clampRect({ ...init });
  onCtl = placeCard; sync();
}

// ---------- window ----------
function windowMode() {
  const scr = S.dev.screens[DISPLAY] || { x: 0, y: 0 }, U = /Mac/.test(navigator.platform) ? 1 : 1 / devicePixelRatio;
  const loc = (w) => ({ ...w, lx: (w.x - scr.x) * U, ly: (w.y - scr.y) * U, lw: w.w * U, lh: w.h * U });
  let wins = [], locked = null, hover = null, pt = null, tid = 0;
  const card = ctlCard({ target: () => ({ display: DISPLAY, area: null, window: (locked || hover).id, camera_only: false }) });
  const nm = h('div', { style: 'font-size:30px;font-weight:600;margin-bottom:8px;text-align:center;max-width:90%' }), sz = h('div', { style: 'font-size:12px;margin-bottom:8px' });
  const adj = h('button', { style: 'margin-top:10px;height:28px;padding:0 12px;border-radius:8px;background:#1c1c1c;color:#eee;font-size:12px;border:1px solid #ffffff1a', onclick: (e) => {
    e.stopPropagation(); const w = locked || hover; clearInterval(tid); root.innerHTML = ''; root.className = KIND = 'area';
    areaMode({ x: w.lx, y: w.ly, w: w.lw, h: w.lh });
  } }, 'Adjust recording area');
  const box = h('div', { style: 'position:absolute;display:none;flex-direction:column;align-items:center;justify-content:center;background:rgba(37,99,235,.4);color:#fff' }, nm, sz, card, adj);
  root.append(box);
  const hit = (x, y) => wins.find((w) => x >= w.lx && x < w.lx + w.lw && y >= w.ly && y < w.ly + w.lh) || null;
  const show = () => {
    const w = locked || hover; box.style.display = w ? 'flex' : 'none'; if (!w) return;
    Object.assign(box.style, { left: w.lx + 'px', top: w.ly + 'px', width: w.lw + 'px', height: w.lh + 'px' });
    nm.textContent = w.app; sz.textContent = `${Math.round(w.w)}x${Math.round(w.h)}`;
  };
  const load = async () => {
    try { wins = (await invoke('windows')).filter((w) => w.display === DISPLAY).map(loc); } catch (e) { log('windows: ' + e); }
    if (!locked && pt) hover = hit(pt.x, pt.y);
    show();
  };
  root.addEventListener('pointermove', (e) => { pt = { x: e.clientX, y: e.clientY }; if (!locked && !e.target.closest('.ctl')) { hover = hit(pt.x, pt.y); show(); } });
  root.addEventListener('click', (e) => { if (e.target.closest('.ctl')) return; const w = hit(e.clientX, e.clientY); if (w) { locked = w; show(); } });
  (async () => {
    await load();
    const id = parseInt(localStorage.getItem('precapWindow'), 10); localStorage.removeItem('precapWindow');
    if (id) { locked = wins.find((w) => w.id === id) || null; show(); }
    tid = setInterval(() => { if (!locked) load(); }, 1500);
  })();
}

(async function init() {
  await refresh();
  try { SET = await getSettings(); } catch (e) { log('settings: ' + e); }
  TAURI.event.listen('settings-changed', (e) => { SET = e.payload; $('.ctl') && $('.ctl').redraw && $('.ctl').redraw(); });
  ({ display: displayMode, camera: cameraMode, area: areaMode, window: windowMode })[KIND]();
})();
