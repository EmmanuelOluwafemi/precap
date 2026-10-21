'use strict';
// Settings window: General / Recording quality / Shortcuts. Every control writes straight away via set_settings.
Object.assign(ICONS, {
  sliders: '<path d="M4 7h10M18 7h2M4 17h2M10 17h10"/><circle cx="16" cy="7" r="2"/><circle cx="8" cy="17" r="2"/>',
  kbd: '<rect x="2" y="6" width="20" height="12" rx="2"/><path d="M6 10h.01M10 10h.01M14 10h.01M18 10h.01M7 14h10"/>',
  cok: '<circle cx="12" cy="12" r="9"/><path d="m8 12 3 3 5-6"/>',
  cx: '<circle cx="12" cy="12" r="9"/><path d="m9 9 6 6M15 9l-6 6"/>',
  plus: '<path d="M12 5v14M5 12h14"/>',
});
const PAGES = [['general', 'General', 'gear'], ['quality', 'Recording quality', 'sliders'], ['shortcuts', 'Shortcuts', 'kbd']];
const T0 = '{target_name} ({target_kind}) {date} {time}';
const q0 = new URLSearchParams(location.search).get('page');
// page, S = settings, E = inline errors by key, tpl = template draft, cap = action being recorded, cd = its pending binding
const V = { page: PAGES.some((p) => p[0] === q0) ? q0 : 'general', S: {}, os: 'macos', def: '', E: {}, tpl: null, cap: null, cd: null };

async function save(k, patch) {
  try { V.S = await setSettings(patch); delete V.E[k]; } catch (e) { V.E[k] = String(e); }
  render();
}
const err = (k) => (V.E[k] ? h('div.err', {}, V.E[k]) : null);
const row = (k, label, desc, ctl) => h('div.sr', {}, h('div.rt', {}, h('div.rl', {}, label), desc ? h('div.rd', {}, desc) : null, err(k)), h('div.rc', {}, ctl));
const sect = (title, desc, body, right) => h('section.ss', {}, h('header.sh', {}, h('div', {}, h('h3', {}, title), desc ? h('p', {}, desc) : null), right ? h('div.ra', {}, right) : null), body);
const btn = (label, fn, cls = '', dis = false) => h('button.btn', { class: cls, disabled: dis, onclick: fn }, label);
const toggle = (on, fn) => h('button.tg', { class: on ? 'on' : '', role: 'switch', 'aria-checked': String(!!on), onclick: () => fn(!on) }, h('i'));
function select(opts, cur, pick) {
  const b = h('button.sel', {}, (opts.find((o) => o[0] === cur) || [0, '-'])[1], I('chev', 14));
  b.onclick = () => openMenu(b, opts, cur, pick); return b;
}

function general() {
  const S = V.S, sel = (k, opts) => select(opts, S[k], (v) => save(k, { [k]: v })), tog = (k) => toggle(S[k], (v) => save(k, { [k]: v }));
  const rd = h('span.rv', {}, (+S.defaultZoomAmount).toFixed(1) + 'x');
  const sl = h('input', { type: 'range', min: 1, max: 4.5, step: 0.1, value: S.defaultZoomAmount, 'aria-label': 'Default zoom amount' });
  sl.oninput = () => (rd.textContent = (+sl.value).toFixed(1) + 'x');
  sl.onchange = () => save('defaultZoomAmount', { defaultZoomAmount: Math.round(+sl.value * 10) / 10 });
  const rec = h('div.sc-card', {},
    row('recordingCountdown', 'Countdown', 'Wait before the recording starts.', sel('recordingCountdown', COUNTDOWNS)),
    row('confirmBeforeRecordingWithoutMicrophone', 'Confirm before recording without a microphone', 'Require confirmation when no microphone is selected or the selected microphone is unavailable.', tog('confirmBeforeRecordingWithoutMicrophone')),
    row('mainWindowRecordingStartBehaviour', 'Main window when recording starts', 'What happens to the main window once a recording begins.', sel('mainWindowRecordingStartBehaviour', [['close', 'Close'], ['minimise', 'Minimise']])),
    row('postStudioRecordingBehaviour', 'After a Studio recording', 'What happens once you stop a Studio recording.', sel('postStudioRecordingBehaviour', [['openEditor', 'Open editor'], ['showOverlay', 'Show in overlay']])),
    row('postScreenshotBehaviour', 'After a screenshot', 'For every screenshot, from a shortcut or from Precap.', sel('postScreenshotBehaviour', [['edit', 'Edit — Annotate first'], ['copy', 'Copy — Paste anywhere'], ['copyAndEdit', 'Copy and edit — Both at once']])),
    row('autoZoomOnClicks', 'Auto zoom on clicks', 'Automatically add zoom segments around mouse clicks in Studio recordings.', tog('autoZoomOnClicks')),
    row('defaultZoomAmount', 'Default zoom amount', 'Zoom level for newly created and auto-generated zoom segments.', h('div.zr', {}, sl, rd)),
    row('maxFps', 'Max capture framerate', S.maxFps > 60 ? 'Maximum framerate for screen capture. Higher values may cause drops or increased CPU usage on some systems.' : 'Maximum framerate for screen capture.',
      sel('maxFps', [24, 25, 30, 60, 120].map((f) => [f, f + ' FPS' + (f === 30 ? ' (Recommended)' : '')]))));
  return [sect('Recording', 'Behaviour while you record and after you stop.', rec), storage(), nameSec(), exclSec()];
}

function storage() {
  const p = V.S.recordingsPath;
  const choose = async () => {
    try { const f = await invoke('pick_recordings_folder'); if (f) await save('path', { recordingsPath: f }); } catch (e) { V.E.path = String(e); render(); }
  };
  return sect('Storage', 'Where Precap saves your recordings.', h('div.sc-card.pad', {},
    h('div.pbox', {}, h('span', { title: p || V.def }, p || 'Default (Application Support)'), p || !V.def ? null : h('small', {}, V.def)),
    err('path'),
    h('div.fr', {}, p ? btn('Reset to Default', () => save('path', { recordingsPath: null })) : null, btn('Choose Folder', choose, 'pri'))));
}

function nameSec() {
  const stored = V.S.defaultProjectNameTemplate, cur = stored ?? T0;
  const inp = h('input.tin', { value: V.tpl ?? cur, spellcheck: false, placeholder: T0, 'aria-label': 'Default project name template' });
  const pv = h('span.pv'), saveB = btn('Save', null, 'pri'), resetB = btn('Reset', null);
  let seq = 0;
  const prev = async () => {
    const n = ++seq; let r;
    try { r = await invoke('format_project_name', { template: inp.value }); } catch (e) { r = String(e); }
    if (n === seq) pv.textContent = r;
  };
  const upd = () => { saveB.disabled = inp.value.length <= 3 || inp.value === cur; resetB.disabled = inp.value === T0 && stored == null; };
  inp.oninput = () => { V.tpl = inp.value; upd(); prev(); };
  saveB.onclick = () => { V.tpl = null; save('tpl', { defaultProjectNameTemplate: inp.value }); };
  resetB.onclick = () => { V.tpl = null; save('tpl', { defaultProjectNameTemplate: null }); };
  const ins = (t) => { inp.setRangeText(t, inp.selectionStart ?? inp.value.length, inp.selectionEnd ?? inp.value.length, 'end'); inp.focus(); inp.oninput(); };
  const toks = [['{mode}', 'Recording mode: studio, instant or screenshot'], ['{target_kind}', 'Display, Window or Area'], ['{target_name}', 'Monitor name or window title'], ['{date}', "Today's ISO date"], ['{time}', 'Current time'], ['{moment:%Y-%m-%d %H:%M}', 'Date and time with a custom strftime format']];
  upd(); prev();
  return sect('Default project name', 'Template used for new recordings and exported files.', h('div.sc-card.pad', {},
    h('div.tbox', {}, inp, I('zap', 16), pv), err('tpl'),
    h('div.note', {}, 'Available placeholders. Click one to insert it. Time and date accept custom formats, e.g. {date:%d %b %Y}.'),
    h('div.chips', {}, toks.map(([t, d]) => h('button.tk', { title: d, onclick: () => ins(t) }, t)))),
    [resetB, saveB]);
}

const exName = (w) => w.ownerName ?? w.windowTitle ?? w.bundleIdentifier ?? 'Unknown';
function exclSec() {
  const L = V.S.excludedWindows || [], mac = V.os === 'macos';
  const add = async (e) => {
    let ws = []; try { ws = await invoke('windows'); } catch (x) { V.E.excl = String(x); return render(); }
    const taken = (w) => L.some((x) => (w.bundle ? x.bundleIdentifier === w.bundle : x.windowTitle === w.title && x.ownerName === w.app));
    const seen = new Set(), items = [];
    for (const w of ws) { const lab = w.app + (w.title ? ' • ' + w.title : ''); if (taken(w) || seen.has(lab)) continue; seen.add(lab); items.push([w, lab]); }
    if (!items.length) { V.E.excl = 'No more open windows to exclude.'; return render(); }
    delete V.E.excl;
    openMenu(e.currentTarget, items, null, (w) => save('excl', { excludedWindows: L.concat([{ bundleIdentifier: w.bundle || null, ownerName: w.app || null, windowTitle: w.bundle ? null : w.title }]) }));
  };
  const chips = L.length ? h('div.chips', {}, L.map((w, i) => h('div.xc', {}, h('div', {}, h('b', {}, exName(w)), w.ownerName && w.windowTitle ? h('small', {}, w.windowTitle) : w.bundleIdentifier && w.ownerName ? h('small', {}, w.bundleIdentifier) : null),
    h('button', { 'aria-label': 'Remove excluded window', html: ico('x', 12), onclick: () => save('excl', { excludedWindows: L.filter((_, j) => j !== i) }) }))))
    : h('div.note', {}, 'No windows are currently excluded.');
  return sect('Excluded windows', 'Hide windows from recordings.', h('div.sc-card.pad', {}, mac ? [chips, err('excl')] : h('div.note', {}, 'Window exclusion is only available on macOS.')),
    mac ? [btn('Reset', () => save('excl', { excludedWindows: [] })), h('button.btn.pri', { onclick: add }, I('plus', 14), 'Add')] : null);
}

const STUDIO = [['balanced', 'Balanced', 'Clear, detailed recordings with a practical file size. Best for everyday use.'], ['compatibility', 'Smaller files', 'Uses less disk space. Can reduce detail, especially when recording with a camera.'], ['ultra', 'Maximum detail', 'Preserves more detail for demanding edits. Creates larger files and needs more disk space.']];
const INSTANT = [[1280, '720p', 'Smaller files. Good for quick updates.'], [1920, '1080p', 'Clear text and a practical file size.'], [2560, '1440p', 'More detail for larger screens. Creates larger files.'], [3840, '4K', 'The most detail and the largest files.']];
function quality() {
  const S = V.S, cur = INSTANT.find((r) => r[0] === S.instantModeMaxResolution);
  return [sect('Recording quality', 'Choose how new recordings look and how much space they use.'),
    sect('Studio', 'Saved to your computer, ready to edit.', h('div.sc-card.pad', {},
      h('div.rc3', { role: 'group', 'aria-label': 'Studio recording quality' }, STUDIO.map(([v, t, d]) => h('button.rad', { class: S.studioRecordingQuality === v ? 'on' : '', 'aria-pressed': String(S.studioRecordingQuality === v), onclick: () => save('studio', { studioRecordingQuality: v }) },
        h('span.dot'), h('span', {}, h('b', {}, t, v === 'balanced' ? h('span.tag', {}, 'Recommended') : null), h('small', {}, d))))),
      err('studio'), h('div.note', {}, 'These options affect the original recording. Choose your final export resolution and file size in the editor.'))),
    sect('Instant', 'Quick-record MP4s that are ready the moment you stop.', h('div.sc-card.pad', {},
      h('div.rl2', {}, 'Maximum resolution'),
      h('div.res', { role: 'group', 'aria-label': 'Instant recording resolution' }, INSTANT.map(([v, t]) => h('button', { class: S.instantModeMaxResolution === v ? 'on' : '', 'aria-pressed': String(S.instantModeMaxResolution === v), onclick: () => save('instant', { instantModeMaxResolution: v }) }, t))),
      err('instant'), h('div.note', {}, (cur ? cur[2] + ' ' : '') + 'Resolution is limited by the screen or area you record.')))];
}

const HK = [['startStudioRecording', 'Start studio recording'], ['startInstantRecording', 'Start instant recording'], ['screenshotDisplay', 'Screenshot current display'], ['screenshotWindow', 'Screenshot current window'], ['screenshotArea', 'Screenshot area picker'],
  ['openRecordingPicker', 'Open recording picker'], ['stopRecording', 'Stop recording'], ['restartRecording', 'Restart recording'], ['togglePauseRecording', 'Pause/resume recording'], ['cycleRecordingMode', 'Cycle recording mode'],
  ['openRecordingPickerDisplay', 'Record display'], ['openRecordingPickerWindow', 'Record window'], ['openRecordingPickerArea', 'Record area']];
const isF = (c) => /^F([1-9]|1\d|2[0-4])$/.test(c);
function kbds(k) {
  const mac = V.os === 'macos', m = mac ? [['meta', '⌘'], ['ctrl', '⌃'], ['alt', '⌥'], ['shift', '⇧']] : [['ctrl', 'Ctrl'], ['alt', 'Alt'], ['shift', 'Shift'], ['meta', 'Win']];
  return h('div.kb', {}, m.filter(([f]) => k[f]).map(([, l]) => h('kbd', {}, l)), h('kbd', {}, k.code.replace(/^(Key|Digit)/, '')));
}
function endCap() { V.cap = V.cd = null; render(); }
function commit(a, hk) {
  const m = { ...V.S.hotkeys }; if (hk) m[a] = hk; else delete m[a];
  V.cap = V.cd = null; return save('hk:' + a, { hotkeys: m });
}
function shortcuts() {
  return [sect('Shortcuts', 'Configure system-wide keyboard shortcuts to control Precap.', h('div.sc-card.pad', {}, HK.map(([a, label]) => {
    const k = (V.S.hotkeys || {})[a], on = V.cap === a;
    const ctl = on
      ? h('div.capping', {}, V.cd ? kbds(V.cd) : h('span', {}, 'Set hotkeys...'),
        V.cd ? h('button', { title: 'Save shortcut', 'aria-label': 'Save shortcut', html: ico('cok'), onclick: () => commit(a, V.cd) }) : null,
        h('button.cx', { title: 'Clear shortcut', 'aria-label': 'Clear shortcut', html: ico('cx'), onclick: () => commit(a, null) }))
      : h('button', { onclick: () => { V.E = {}; V.cap = a; V.cd = k || null; render(); } }, k ? kbds(k) : h('span.none', {}, 'None'));
    return h('div.hkw', {}, h('div.hk', {}, h('span.l', {}, label), ctl), err('hk:' + a));
  })))];
}
addEventListener('keydown', (e) => {
  if (!V.cap) { if ((e.metaKey || e.ctrlKey) && e.key === 'w') curWin.close(); return; }
  e.preventDefault(); e.stopPropagation();
  if (e.key === 'Escape') return endCap();
  if (/^(Meta|Control|Shift|Alt|OS)(Left|Right)$/.test(e.code)) return;
  const hk = { code: e.code, ctrl: e.ctrlKey, shift: e.shiftKey, alt: e.altKey, meta: e.metaKey };
  if (!(hk.ctrl || hk.shift || hk.alt || hk.meta) && !isF(e.code)) { V.E['hk:' + V.cap] = 'Use at least one modifier key (Ctrl, Alt, Shift or ' + (V.os === 'macos' ? '⌘' : 'Win') + '), or an F1-F24 key.'; return render(); }
  delete V.E['hk:' + V.cap]; V.cd = hk; render();
}, true);
document.addEventListener('pointerdown', (e) => { if (V.cap && !e.target.closest('.capping')) endCap(); }, true);

function render() {
  const app = $('#app'), st = $('.sc') ? $('.sc').scrollTop : 0;
  closeMenu(); app.innerHTML = '';
  const body = V.fatal ? [h('div.err', {}, V.fatal)] : ({ general, quality, shortcuts })[V.page]();
  app.append(h('nav.sb', { 'data-tauri-drag-region': '' }, V.os === 'macos' ? h('div.sp44', { 'data-tauri-drag-region': '' }) : null,
    h('ul', {}, PAGES.map(([id, label, ic]) => h('li', {}, h('button', { class: V.page === id ? 'on' : '', onclick: () => { V.page = id; V.cap = null; render(); } }, I(ic, 16), label))))),
    h('main.sc', {}, h('div.pg', {}, body)));
  $('.sc').scrollTop = st;
}

(async function init() {
  const ok = (p, d) => p.catch((e) => { log('settings init: ' + e); return d; });
  [V.os, V.def, V.S] = await Promise.all([ok(invoke('os'), 'macos'), ok(invoke('default_recordings_folder'), ''), getSettings().catch((e) => { V.fatal = String(e); return {}; })]);
  render();
  TAURI.event.listen('settings-changed', (e) => { V.S = e.payload; const a = document.activeElement; if (!(a && a.matches && a.matches('input'))) render(); });
  curWin.listen('settings-page', (e) => { if (PAGES.some((p) => p[0] === e.payload)) { V.page = e.payload; V.cap = null; render(); } });
})();
