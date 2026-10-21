'use strict';
// Main recorder panel: target tiles, device rows, and in-window picker / recordings views.
const R = { view: 'home', dev: { screens: [], cameras: [], mics: [] }, sel: [null, null, false], error: '', notice: '', q: '', recs: null, shots: null, wins: null, tile: null, cameraOnlyNext: false, mode: getMode(), level: null, mtr: null, th: new Map() };
const getDisplay = () => Math.min(parseInt(localStorage.getItem('precapDisplay') || '0', 10) || 0, Math.max(0, R.dev.screens.length - 1));
const setDisplay = (i) => localStorage.setItem('precapDisplay', String(i));

async function refresh() {
  try { R.dev = await invoke('devices'); R.sel = await invoke('selection'); } catch (e) { R.error = String(e); log('devices: ' + e); }
}
const camName = () => (R.dev.cameras.find((c) => c.id === R.sel[0]) || {}).name || (R.sel[0] ? 'Camera' : null);
const micName = () => (R.dev.mics.find((c) => c.id === R.sel[1]) || {}).name || (R.sel[1] ? 'Microphone' : null);

function header() {
  const hd = $('#hdr'); hd.innerHTML = '';
  hd.append(h('span.sp', { 'data-tauri-drag-region': '' }),
    h('button.hico', { title: 'Settings', html: ico('gear', 18), onclick: () => invoke('open_settings', { page: 'general' }).catch((e) => { R.error = String(e); render(); }) }),
    h('button.hico', { class: R.view === 'screenshots' ? 'on' : '', title: 'Screenshots', html: ico('shot', 18), onclick: () => go(R.view === 'screenshots' ? 'home' : 'screenshots') }),
    h('button.hico', { class: R.view === 'recordings' ? 'on' : '', title: 'Recordings', html: ico('rec', 18), onclick: () => go(R.view === 'recordings' ? 'home' : 'recordings') }),
    h('button.hico', { title: 'Teleprompter', html: ico('tele', 18), onclick: () => invoke('open_teleprompter').catch((e) => { R.error = String(e); render(); }) }));
}
async function go(v) {
  R.view = v; R.q = '';
  for (const p of R.th.values()) p.then(URL.revokeObjectURL, () => {}); R.th.clear();
  if (v === 'screenshots') { try { R.shots = await invoke('screenshots'); } catch (e) { R.shots = []; R.error = String(e); } }
  if (v === 'window') { try { R.wins = (await invoke('windows')).sort((a, b) => a.app.localeCompare(b.app) || a.title.localeCompare(b.title)); } catch (e) { R.wins = null; } }
  if (v === 'recordings') { try { R.recs = await invoke('recordings'); } catch (e) { R.recs = []; R.error = String(e); } }
  if (['display', 'camera', 'mic'].includes(v)) await refresh();
  render();
}
function notice() { return R.notice ? h('div.banner.ok', {}, R.notice) : null; }
function banner() {
  return R.error ? h('div.banner', {}, R.error, h('button', { title: 'Dismiss', html: ico('x', 14), onclick: () => { R.error = ''; render(); } })) : null;
}
async function openTarget(kind, display, mode) {
  R.tile = kind; render();
  try {
    let d = display ?? getDisplay();
    if (kind === 'window' && display == null) { // display holding the most windows, else the saved one
      localStorage.removeItem('precapWindow');
      const n = {}; for (const w of await invoke('windows')) n[w.display] = (n[w.display] || 0) + 1;
      let best = n[d] || 0; for (const k in n) if (n[k] > best) { best = n[k]; d = +k; }
    }
    await invoke('open_target', { kind, display: d, mode: mode || R.mode });
  } catch (e) { R.error = String(e); R.tile = null; render(); }
}
function pickMode(m) { R.mode = m; setMode(m); if (m === 'screenshot' && R.tile === 'camera') R.tile = null; render(); }
let tipEl = null, tipT = 0;
function hideTip() { clearTimeout(tipT); if (tipEl) { tipEl.remove(); tipEl = null; } }
function showTip(btn, label, desc) {
  clearTimeout(tipT);
  tipT = setTimeout(() => {
    hideTip(); const r = btn.getBoundingClientRect();
    tipEl = h('div.tip', { style: `top:${r.bottom + 6}px;right:${innerWidth - r.right}px` }, h('b', {}, label), h('span', {}, desc)); document.body.append(tipEl);
  }, tipEl ? 0 : 120);
}
const MODE_TIPS = {
  instant: ['Instant mode', 'No editing step — saves a ready-to-share MP4 and copies it to your clipboard the moment you stop.'],
  studio: ['Studio mode', 'Saves to your computer and opens the editor when you stop.'],
  screenshot: ['Screenshot mode', 'Capture and annotate stills.'],
};
const setLevel = (db) => { R.level = db; if (R.mtr) R.mtr.style.right = (R.sel[1] && db != null ? Math.max(0, 1 - Math.max(Math.min(db, 0) + 40, 0) / 40) ** 0.5 * 100 : 100) + '%'; };
async function toggleSys() {
  try { await invoke('set_system_audio', { on: !R.sel[2] }); } catch (e) { R.error = String(e); }
  await refresh(); render();
}
async function pickCamera(id) {
  try { await invoke('set_camera', { id }); } catch (e) { R.error = String(e); }
  await refresh(); R.view = 'home';
  const co = R.cameraOnlyNext && id; R.cameraOnlyNext = false;
  render(); if (co) openTarget('camera');
}
async function pickMic(id) {
  try { await invoke('set_mic', { id }); } catch (e) { R.error = String(e); }
  await refresh(); R.view = 'home'; render();
}

function home(b) {
  const dispOn = R.tile === 'display', shot = R.mode === 'screenshot';
  b.append(
    h('div.brand', {}, h('div.logo', { html: '<svg viewBox="0 0 24 24" width="22" height="22" fill="none" stroke="#fff" stroke-width="2.2" stroke-linecap="round"><path d="M17.5 8.2A7 7 0 1 0 17.5 15.8"/><circle cx="12" cy="12" r="2.2" fill="#fff" stroke="none"/></svg>' }), h('b', {}, 'Precap'),
      h('div.modes', {}, Object.keys(MODES).map((m) => h('button.mbtn', { class: R.mode === m ? 'on' : '', 'aria-label': MODE_TIPS[m][0], onclick: () => pickMode(m),
        onmouseenter: (e) => showTip(e.currentTarget, ...MODE_TIPS[m]), onmouseleave: () => { clearTimeout(tipT); tipT = setTimeout(hideTip, 80); } }, I(MODES[m][1], 16))))),
    h('div.tiles', {},
      h('div.split', { class: dispOn ? 'on' : '' },
        h('button.tile', { onclick: () => openTarget('display') }, I('monitor'), h('span', {}, 'Display')),
        h('button.chev', { title: 'Choose display', html: ico('chev', 14), onclick: () => go('display') })),
      h('div.split', { class: R.tile === 'window' ? 'on' : '' },
        h('button.tile', { onclick: () => openTarget('window') }, I('win'), h('span', {}, 'Window')),
        h('button.chev', { title: 'Choose window', html: ico('chev', 14), onclick: () => go('window') })),
      h('button.tile', { class: R.tile === 'area' ? 'on' : '', onclick: () => openTarget('area') }, I('area'), h('span', {}, 'Area')),
      h('button.tile', { disabled: shot, class: R.tile === 'camera' ? 'on' : '', onclick: () => { if (!R.sel[0]) { R.cameraOnlyNext = true; go('camera'); } else openTarget('camera'); } }, I('cam'), h('span', {}, 'Camera Only'))),
    ...(shot ? [] : [h('div', { style: 'display:flex;flex-direction:column;gap:8px' },
      devRow('cam', camName(), 'No Camera', R.sel[0], () => go('camera'), () => pickCamera(null)),
      devRow('mic', micName(), 'No Microphone', R.sel[1], () => go('mic'), () => pickMic(null), true),
      devRow('monitor', R.sel[2] ? 'Record System Audio' : null, 'No System Audio', R.sel[2], toggleSys, toggleSys))]),
    ...(R.error ? [banner()] : []), ...(R.notice ? [notice()] : []));
}
function devRow(icon, name, none, on, open, off, meter) {
  const pill = h('span.pill', { class: on ? 'on' : '' }, on ? 'On' : 'Off');
  if (on) pill.onclick = (e) => { e.stopPropagation(); off(); };
  if (meter) { R.mtr = h('div.mtr'); setLevel(R.level); }
  return h('div.row', { role: 'button', style: 'cursor:pointer', onclick: open }, meter ? R.mtr : null, I(icon), h('span.nm', { class: on ? '' : 'none' }, name || none), pill);
}

function searchBar(ph) {
  const inp = h('input', { placeholder: ph, value: R.q, spellcheck: false });
  inp.oninput = () => { R.q = inp.value; renderList(); };
  return h('div.vtop', {}, h('button.back', { onclick: () => go('home') }, I('back', 15), 'Back'), h('div.search', {}, I('search', 15), inp));
}
let listFn = null;
function renderList() { const l = $('#list'); if (!l) return; l.innerHTML = ''; listFn(l); }
function view(b, ph, fn, cls = 'list') {
  listFn = fn;
  b.append(h('div.view', {}, searchBar(ph), h('div#list', { class: cls })), ...(R.error ? [banner()] : []));
  renderList(); const i = $('.search input'); if (i) i.focus();
}
const match = (s) => s.toLowerCase().includes(R.q.trim().toLowerCase());

function devPicker(b, kind) {
  const cam = kind === 'camera', items = cam ? R.dev.cameras : R.dev.mics, cur = R.sel[cam ? 0 : 1];
  view(b, cam ? 'Search cameras' : 'Search microphones', (l) => {
    const none = cam ? 'No Camera' : 'No Microphone';
    const pick = (id) => (cam ? pickCamera(id) : pickMic(id));
    if (match(none)) l.append(h('button.item', { class: cur ? '' : 'on', onclick: () => pick(null) }, I(cam ? 'camoff' : 'mic', 17), h('div.t', {}, h('b', {}, none)), cur ? null : I('check', 17)));
    const f = items.filter((d) => match(d.name));
    for (const d of f) l.append(h('button.item', { class: cur === d.id ? 'on' : '', onclick: () => pick(d.id) }, I(cam ? 'cam' : 'mic', 17), h('div.t', {}, h('b', {}, d.name)), cur === d.id ? I('check', 17) : null));
    if (!f.length && !match(none)) l.append(h('div.empty', {}, R.q ? (cam ? 'No matching cameras' : 'No matching microphones') : cam ? 'No cameras found' : 'No microphones found'));
    else if (!items.length) l.append(h('div.empty', {}, cam ? 'No cameras found' : 'No microphones found'));
  });
}
function displayPicker(b) {
  view(b, 'Search displays', (l) => {
    const f = R.dev.screens.map((s, i) => [s, i]).filter(([s]) => match(s.name || 'Display'));
    if (!f.length) { l.append(h('div.empty', {}, R.q ? 'No matching displays' : 'No displays found')); return; }
    const g = h('div.cards'); l.append(g);
    for (const [s, i] of f) g.append(h('button.card', { class: i === getDisplay() ? 'on' : '', onclick: async () => { setDisplay(i); R.view = 'home'; render(); openTarget('display'); } },
      h('div.th', {}, I('monitor')), h('div.tx', {}, h('b', {}, s.name || `Display ${i + 1}`), h('small', {}, `${Math.round(s.w * s.scale)}×${Math.round(s.h * s.scale)}`))));
  });
}
function thumb(th, key, cmd, args) {
  th._load = async () => {
    if (!R.th.has(key)) R.th.set(key, invoke(cmd, args).then((b) => URL.createObjectURL(new Blob([b], { type: 'image/jpeg' }))));
    try { th.append(h('img', { src: await R.th.get(key) })); th.classList.add('ok'); } catch (_) { R.th.delete(key); th.classList.add('fail'); }
  };
  io.observe(th);
}
const io = new IntersectionObserver((es) => es.forEach((e) => { if (e.isIntersecting) { io.unobserve(e.target); e.target._load(); } }));
function windowPicker(b) {
  view(b, 'Search windows', (l) => {
    if (!R.wins) { l.append(h('div.empty', {}, 'Unable to load windows. Try using the Window button.')); return; }
    const f = R.wins.filter((w) => match(w.title) || match(w.app));
    if (!f.length) { l.append(h('div.empty', {}, R.q ? 'No matching windows' : 'No windows found')); return; }
    const g = h('div.cards'); l.append(g);
    for (const w of f) {
      const th = h('div.th', {}, I('win')); thumb(th, 'w' + w.id, 'window_thumbnail', { id: w.id });
      g.append(h('button.card', { onclick: () => { localStorage.setItem('precapWindow', String(w.id)); R.view = 'home'; openTarget('window', w.display); } }, th,
        h('div.tx', {}, h('b', {}, w.title || w.app), h('small', {}, w.app), h('small', {}, `${Math.round(w.w)}×${Math.round(w.h)}`))));
    }
  }, 'list');
}
function screenshots(b) {
  view(b, 'Search screenshots', (l) => {
    const f = (R.shots || []).filter((r) => match(r.name));
    if (!f.length) { l.append(h('div.empty', {}, R.q ? 'No matching screenshots' : h('div', {}, h('b', { style: 'display:block;color:var(--g12);font-size:13px' }, 'No screenshots yet'), 'Your screenshots will appear here. Take a screenshot to get started!'))); return; }
    const g = h('div.cards'); l.append(g);
    for (const r of f) {
      const th = h('div.th', {}, I('shot')); thumb(th, 's' + r.path, 'screenshot_thumbnail', { path: r.path });
      g.append(h('button.card', { onclick: () => invoke('open_screenshot', { path: r.path }).catch((e) => { R.error = String(e); render(); }) }, th,
        h('div.tx', {}, h('b', {}, r.name), h('small', {}, new Date(r.modified * 1000).toLocaleString([], { dateStyle: 'medium', timeStyle: 'short' })))));
    }
  });
}
function recordings(b) {
  view(b, 'Search recordings', (l) => {
    const f = (R.recs || []).filter((r) => match(r.name));
    if (!f.length) { l.append(h('div.empty', {}, R.q ? 'No matching recordings' : 'No recordings yet. Your screen recordings will appear here. Start recording to get started!')); return; }
    for (const r of f) l.append(h('button.item', { onclick: () => invoke('open_editor', { path: r.path }).catch((e) => { R.error = String(e); render(); }) },
      I('rec', 17), h('div.t', {}, h('b', {}, r.name), h('small', {}, new Date(r.modified * 1000).toLocaleString([], { dateStyle: 'medium', timeStyle: 'short' })))));
  });
}
function render() {
  header(); hideTip(); R.mtr = null;
  const b = $('#body'); b.innerHTML = '';
  ({ home, camera: (x) => devPicker(x, 'camera'), mic: (x) => devPicker(x, 'mic'), display: displayPicker, window: windowPicker, screenshots, recordings })[R.view](b);
}

(async function init() {
  await refresh(); render();
  TAURI.event.listen('audio-input-level-change', (e) => setLevel(e.payload));
  curWin.listen('recording-error', (e) => { R.error = String(e.payload); R.view = 'home'; R.tile = null; refresh().then(render); });
  curWin.listen('screenshot-copied', () => { R.notice = 'Screenshot copied to clipboard'; render(); clearTimeout(R.nt); R.nt = setTimeout(() => { R.notice = ''; render(); }, 2500); });
  curWin.listen('hotkey-action', (e) => {
    const a = e.payload, k = { openRecordingPickerDisplay: 'display', openRecordingPickerWindow: 'window', openRecordingPickerArea: 'area' }[a];
    if (a === 'cycleRecordingMode') { const ms = Object.keys(MODES); R.mode = ms[(ms.indexOf(getMode()) + 1) % ms.length]; pickMode(R.mode); }
    else if (k) { R.mode = getMode(); openTarget(k); }
    else if (a === 'screenshotArea') openTarget('area', undefined, 'screenshot');
  });
  window.addEventListener('focus', async () => { R.tile = null; R.mode = getMode(); if (R.view === 'home') { await refresh(); render(); } });
})();
