'use strict';
// Cap's teleprompter (FEATURES-SPEC 4). Window is transparent; #root is the panel.
Object.assign(ICONS, {
  eyeoff: '<path d="M10.7 5.1A10 10 0 0 1 12 5c6 0 9.5 7 9.5 7a15 15 0 0 1-2.2 3M6.6 6.6C3.7 8.3 2.5 12 2.5 12s3.5 7 9.5 7a9.7 9.7 0 0 0 5.4-1.6M2 2l20 20M9.9 9.9a3 3 0 0 0 4.2 4.2"/>',
  gauge: '<path d="m12 14 4-4M3.3 19a10 10 0 1 1 17.4 0"/>',
  layers: '<path d="m12 3 9 5-9 5-9-5zM3 13l9 5 9-5"/>',
  minus: '<path d="M5 12h14"/>', plus: '<path d="M5 12h14M12 5v14"/>',
  sliders: '<path d="M4 6h10M18 6h2M4 12h4M12 12h8M4 18h12M20 18h0M14 4v4M8 10v4M16 16v4"/>',
  cr: '<path d="m9 6 6 6-6 6"/>', cl: '<path d="m15 6-6 6 6 6"/>',
  flip: '<path d="M12 3v18M8 7 3 12l5 5zM16 7l5 5-5 5z"/>',
});
const DEF = { script: '', fontSize: 30, wordsPerMinute: 150, lineHeight: 1.5, showCueMarkers: true, mirror: false, windowOpacityPercent: 92 };
let cfg = { ...DEF };
try { cfg = { ...DEF, ...JSON.parse(localStorage.getItem('teleprompter') || '{}') }; } catch (_) {}
const MAC = /Mac/i.test(navigator.platform), LINUX = /Linux/i.test(navigator.userAgent) && !/Android/i.test(navigator.userAgent);
let requested = false, recPaused = false, popOpen = false, raf = 0, last = 0, acc = 0;

let saveT = 0;
const save = () => { clearTimeout(saveT); saveT = 0; try { localStorage.setItem('teleprompter', JSON.stringify(cfg)); } catch (_) {} };
const set = (p) => { Object.assign(cfg, p); clearTimeout(saveT); saveT = setTimeout(save, 250); render(); };
addEventListener('pagehide', save); addEventListener('beforeunload', save);

const words = () => { const t = cfg.script.trim(); return t ? t.split(/\s+/).length : 0; };
const running = () => requested && !recPaused;
const maxScroll = () => sc.scrollHeight - sc.clientHeight;

function tick(now) {
  raf = 0;
  if (!running()) return;
  const max = maxScroll();
  if (max <= 1 || sc.scrollTop >= max - 0.5) { requested = false; render(); return; }
  const dt = Math.min((now - last) / 1000, 0.05); last = now;
  const pps = Math.max(max, 0) / Math.max(1, Math.max(1, words()) / Math.max(1, cfg.wordsPerMinute) * 60);
  if (Math.abs(acc - sc.scrollTop) > 1) acc = sc.scrollTop; // user scrolled by hand
  acc = Math.min(max, acc + pps * dt); sc.scrollTop = acc;
  raf = requestAnimationFrame(tick);
}
function sync() { if (running() && !raf) { last = performance.now(); acc = sc.scrollTop; raf = requestAnimationFrame(tick); } }
function play() {
  if (requested) requested = false;
  else {
    if (!words() || maxScroll() <= 1) return;
    if (sc.scrollTop >= maxScroll() - 1) sc.scrollTop = acc = 0;
    popOpen = false; requested = true;
  }
  render();
}

const sc = h('div#sc');
const ta = h('textarea#ta', { rows: 1, spellcheck: true, autofocus: true, placeholder: 'Paste or type your script…', 'aria-label': 'Teleprompter script', value: cfg.script });
ta.setSelectionRange(0, 0); // else autofocus scrolls a saved script to its end
const fit = () => { const st = sc.scrollTop; ta.style.height = '0'; ta.style.height = ta.scrollHeight + 'px'; sc.scrollTop = st; }; // collapsing the textarea clamps scrollTop
ta.oninput = () => { fit(); if (!ta.value.trim()) requested = false; set({ script: ta.value }); };
sc.append(h('div.sp'), ta, h('div.sp'));
sc.onclick = (e) => { if (e.target !== ta) ta.focus(); };
addEventListener('resize', fit);

const root = $('#root');
const stEl = h('span'), st = h('div#st', {}, h('span', { html: ico('eyeoff') }), stEl);
const hdr = h('header' + (MAC ? '.mac' : ''), { 'data-tauri-drag-region': true }, st,
  MAC ? null : h('button#x', { title: 'Close', 'aria-label': 'Close', html: ico('x', 12), onclick: () => { save(); curWin.close(); } }));
const cue = h('div#cue', {}, I('cr'), I('cl'));
const pp = h('button.pl#pp', { onclick: play });
const rng = (min, max, step, key, fmt, cls, label, title) => {
  const inp = h('input', { type: 'range', min, max, step, value: cfg[key], 'aria-label': label });
  const v = h('span.v'); inp.oninput = () => set({ [key]: +inp.value });
  return { el: h('div.pl' + cls, {}, I(cls ? 'layers' : 'gauge'), inp, v), upd: () => { inp.value = cfg[key]; v.textContent = fmt(cfg[key]); inp.parentNode.title = title(cfg[key]); } };
};
const spd = rng(60, 350, 5, 'wordsPerMinute', (n) => n + ' wpm', '', 'Scroll speed', (n) => 'Scroll speed: ' + n + ' wpm');
spd.el.style.marginLeft = '6px';
const opa = rng(45, 100, 5, 'windowOpacityPercent', (n) => n + '%', '.o', 'Window opacity', (n) => 'Window opacity: ' + n + '%');
const fv = h('span.v');
const fnt = h('div.pl.f', {}, h('button.ib', { title: 'Smaller text', 'aria-label': 'Smaller text', html: ico('minus'), onclick: () => set({ fontSize: Math.max(22, cfg.fontSize - 2) }) }), fv,
  h('button.ib', { title: 'Larger text', 'aria-label': 'Larger text', html: ico('plus'), onclick: () => set({ fontSize: Math.min(52, cfg.fontSize + 2) }) }));
const setBtn = h('button.ib', { title: 'Settings', 'aria-label': 'Settings', html: ico('sliders'), onclick: (e) => { e.stopPropagation(); popOpen = !popOpen; render(); } });
const pop = h('div#pop');
const tg = (icon, label, key) => h('button.pr', { onclick: () => set({ [key]: !cfg[key] }) }, I(icon), h('span', {}, label), h('i.tg' + (cfg[key] ? '.on' : '')));
document.addEventListener('pointerdown', (e) => { if (popOpen && !pop.contains(e.target) && !setBtn.contains(e.target)) { popOpen = false; render(); } });
document.addEventListener('keydown', (e) => { if (e.key === 'Escape' && popOpen) { popOpen = false; render(); } });
root.append(hdr, h('main', {}, sc, cue), h('footer', {}, pp, spd.el, h('div#r', {}, opa.el, fnt, setBtn)), pop);

function render() {
  const empty = !words();
  root.style.opacity = cfg.windowOpacityPercent / 100;
  sc.style.setProperty('--fs', cfg.fontSize); sc.style.setProperty('--lh', cfg.lineHeight);
  sc.classList.toggle('m', cfg.mirror);
  ta.style.fontSize = cfg.fontSize + 'px'; ta.style.lineHeight = cfg.lineHeight; fit();
  cue.classList.toggle('hidden', !cfg.showCueMarkers);
  stEl.textContent = recPaused ? 'Scrolling paused with recording' : LINUX ? 'This window may appear in recordings on Linux' : 'This window is hidden from recordings';
  const on = running() || (requested && recPaused);
  pp.disabled = empty; pp.setAttribute('aria-pressed', on);
  pp.setAttribute('aria-label', on ? 'Pause teleprompter' : 'Play teleprompter');
  pp.title = recPaused ? (requested ? 'Recording paused. Cancel automatic scroll resume' : 'Play when recording resumes') : on ? 'Pause' : 'Play';
  pp.innerHTML = ico(on ? 'pause' : 'play');
  spd.upd(); opa.upd(); fv.textContent = cfg.fontSize;
  setBtn.classList.toggle('on', popOpen);
  pop.classList.toggle('hidden', !popOpen);
  pop.replaceChildren(tg('cr', 'Cue markers', 'showCueMarkers'), tg('flip', 'Mirror text', 'mirror'));
  sync();
}
// Recording coupling. Scroll can be requested while recording is paused: it starts on Resumed.
const ev = TAURI.event;
ev.listen('recording-event', (e) => {
  if (e.payload === 'Paused') recPaused = true;
  else if (e.payload === 'Resumed' || e.payload === 'Started') recPaused = false;
  render();
});
ev.listen('recording-stopped', () => { requested = false; recPaused = false; render(); });
invoke('recording_state').then((s) => { recPaused = !!(s && s.recording && s.paused); render(); }).catch((e) => log('recording_state: ' + e));
render();
