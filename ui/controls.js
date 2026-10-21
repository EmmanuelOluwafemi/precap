'use strict';
// Recording controls bar: countdown -> starting -> recording/paused. Delete/restart confirm inline.
const CD = parseInt(new URLSearchParams(location.search).get('countdown') || '0', 10) || 0;
const C = { mic: null, lv: 0, confirm: null, state: 'countdown', n: CD, acc: 0, t0: 0, cancelled: false, err: '', issue: null };
const bar = $('#bar');
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const elapsed = () => C.acc + (C.state === 'recording' ? performance.now() - C.t0 : 0);
const fmt = (ms) => { const s = Math.floor(ms / 1000); return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, '0')}`; };
let timeEl = null, digitEl = null, lvEl = null;
const setLv = () => { if (lvEl) lvEl.style.transform = `translateX(-${(1 - C.lv) * 100}%)`; };

function gb(icon, title, fn, cls = '') { return h('button.gb', { class: cls, title, html: ico(icon), onclick: fn }); }
function render() {
  $('#issue').replaceChildren(...(C.issue ? [C.issue.title ? h('b', {}, C.issue.title) : null, C.issue.text].filter(Boolean) : []));
  bar.innerHTML = ''; timeEl = digitEl = lvEl = null;
  bar.classList.toggle('paused', C.state === 'paused');
  const s = C.state;
  if (C.confirm) {
    const del = C.confirm === 'delete';
    bar.append(h('div.confirm', {}, h('span', {}, del ? 'Delete recording?' : 'Restart recording?'),
      h('button.cb', { onclick: () => { C.confirm = null; render(); } }, 'Cancel'),
      h('button.cb.red', { onclick: del ? doDelete : doRestart }, del ? 'Delete' : 'Restart')));
    return;
  }
  const slot = h('span.slot');
  let stopDisabled = true, stopCls = '';
  if (s === 'countdown') { slot.classList.add('digit'); digitEl = slot; digitEl.append(h('span.cd', {}, String(C.n))); }
  else if (s === 'starting') slot.append(h('span', { style: 'display:flex;align-items:center' }, h('i.sp'), 'Starting'));
  else if (s === 'stopping') slot.append(h('span', {}, 'Saving…'));
  else if (s === 'paused') { stopDisabled = false; stopCls = 'paused'; slot.append(h('span', {}, 'Paused')); }
  else { stopDisabled = false; timeEl = h('span', {}, fmt(elapsed())); slot.append(timeEl); }
  bar.append(h('button.stop', { class: stopCls, disabled: stopDisabled, title: 'Stop recording', onclick: doStop }, I('stop', 20), slot,
    C.err ? h('span.err', { title: C.err }, '!') : null));
  const live = s === 'recording' || s === 'paused';
  if (s === 'countdown' || s === 'starting') {
    bar.append(h('div.grp', {}, gb('x', 'Close recording controls', doCancel)));
  } else if (live) {
    lvEl = h('div.lv');
    bar.append(h('div.grp', {},
      C.mic ? h('div.micb', { title: 'Microphone: ' + C.mic }, I('mic', 20), h('div.strip', {}, lvEl)) : null,
      gb(s === 'paused' ? 'play' : 'pause', s === 'paused' ? 'Resume recording' : 'Pause recording', togglePause),
      gb('restart', 'Restart recording', () => { C.confirm = 'restart'; render(); }),
      gb('trash', 'Delete recording', () => { C.confirm = 'delete'; render(); }, 'del')),
      h('button.handle', { title: 'Move recording controls', html: ico('grip', 16), onpointerdown: (e) => { if (e.button === 0) curWin.startDragging(); } }));
    setLv();
  }
}
function rollTo(n) {
  if (!digitEl) return;
  const old = digitEl.firstChild, nu = h('span.cd', {}, String(n));
  digitEl.append(nu);
  const ease = 'cubic-bezier(0.16,1,0.3,1)';
  nu.animate([{ transform: 'translateY(-100%)' }, { transform: 'translateY(0)' }], { duration: 300, easing: ease });
  if (old) old.animate([{ transform: 'translateY(0)' }, { transform: 'translateY(100%)' }], { duration: 300, easing: ease, fill: 'forwards' }).onfinish = () => old.remove();
}

async function begin(restart) {
  C.confirm = null; C.state = 'starting'; C.err = ''; C.issue = null; render();
  try { await invoke(restart ? 'restart_recording' : 'start_recording'); }
  catch (e) { log('start: ' + e); return; } // backend closes this window and reports to main
  if (C.cancelled) return;
  C.acc = 0; C.t0 = performance.now(); C.state = 'recording'; render();
}
async function countdown() {
  render();
  for (let n = CD; n > 0 && !C.cancelled; n--) {
    if (n !== CD) rollTo(n);
    C.n = n; await sleep(1000);
  }
  if (!C.cancelled) await begin(false);
}
async function doCancel() { C.cancelled = true; try { await invoke('delete_recording'); } catch (e) { log('cancel: ' + e); } }
async function togglePause() {
  try {
    if (C.state === 'recording') { await invoke('pause_recording'); C.acc = elapsed(); C.state = 'paused'; }
    else { await invoke('resume_recording'); C.t0 = performance.now(); C.state = 'recording'; }
    C.err = '';
  } catch (e) { C.err = String(e); log('pause: ' + e); }
  render();
}
async function doStop() {
  if (C.state !== 'recording' && C.state !== 'paused') return;
  C.acc = elapsed(); C.state = 'stopping'; render();
  try { await invoke('stop_recording'); } catch (e) { C.err = String(e); C.state = 'recording'; C.t0 = performance.now(); log('stop: ' + e); render(); }
}
async function doDelete() { try { await invoke('delete_recording'); } catch (e) { C.err = String(e); log('delete: ' + e); C.confirm = null; render(); } }
async function doRestart() { await begin(true); }

setInterval(() => { if (timeEl) timeEl.textContent = fmt(elapsed()); }, 100);
(CD > 0 ? countdown : () => begin(false))();

// Display-only mic level (no mute): selected mic from the shared selection, level from the meter event.
(async () => {
  try {
    const [sel, dev] = await Promise.all([invoke('selection'), invoke('devices')]);
    C.mic = sel[1] ? ((dev.mics.find((m) => m.id === sel[1]) || {}).name || 'Microphone') : null;
    if (C.state === 'recording' || C.state === 'paused') render();
  } catch (e) { log('mic: ' + e); }
})();
TAURI.event.listen('audio-input-level-change', (e) => { const p = e.payload; C.lv = p == null ? 0 : Math.min(1, Math.max(0, (p + 60) / 60)); setLv(); });

// Low disk space: the backend pauses the recording and sends the notice (null clears it).
TAURI.event.listen('recording-event', (e) => { if (e.payload === 'Paused' && C.state === 'recording') { C.acc = elapsed(); C.state = 'paused'; render(); } });
curWin.listen('recording-issue', (e) => { C.issue = e.payload; render(); });

// Global shortcuts forwarded by Rust act like the buttons (restart skips the inline confirm); ignored outside recording/paused.
curWin.listen('hotkey-action', (e) => {
  if (C.state !== 'recording' && C.state !== 'paused') return;
  ({ stopRecording: doStop, togglePauseRecording: togglePause, restartRecording: doRestart })[e.payload]?.();
});
