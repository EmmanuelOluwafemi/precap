'use strict';
// First-run permission onboarding (macOS). Status is polled while open and on focus.
const ROWS = {
  screen: ['Screen Recording', 'Needed to capture your screen. Takes effect after Precap relaunches.'],
  microphone: ['Microphone', 'Record your voice.'],
  camera: ['Camera', 'Show your camera bubble.'],
  accessibility: ['Accessibility', 'Track mouse clicks for auto zoom and the cursor track.'],
};
// perms = last status, touched = ids the user already tried, screen0 = screen was missing when opened
const V = { perms: null, touched: new Set(), screen0: false };
const btn = (label, fn, cls = '') => h('button.btn', { class: cls, onclick: fn }, label);

async function act(id, cmd) { V.touched.add(id); await invoke(cmd, { id }); refresh(true); }

function row(p) {
  const [name, desc] = ROWS[p.id], ok = p.status === 'granted', tried = V.touched.has(p.id);
  // The system prompt shows once per permission; after that only System Settings can change it.
  const prompt = !tried && (p.status === 'notDetermined' || p.id === 'screen');
  const acts = [];
  if (!ok) acts.push(prompt ? btn('Grant', () => act(p.id, 'permission_request'), 'pri') : btn('Open System Settings', () => act(p.id, 'open_permission_pane')));
  if (p.id === 'screen' && (tried || (ok && V.screen0))) acts.push(btn('Quit & Reopen', () => invoke('relaunch_app'), 'pri'));
  const hint = !ok && tried && (p.id === 'screen' || p.id === 'accessibility') ? h('div.hint', {}, 'If Precap is not in the list, drag it in from Finder.') : null;
  return h('div.sr', {}, h('div.rt', {}, h('div.rl', {}, name, p.required ? null : h('span.tag', {}, 'Optional')), h('div.rd', {}, desc), hint),
    h('div.rc', {}, h('span.st', { class: ok ? 'ok' : '' }, ok ? 'Granted' : 'Not granted'), acts));
}

function render() {
  const app = $('#app'), screen = V.perms.find((p) => p.id === 'screen');
  app.innerHTML = '';
  app.append(h('div', { 'data-tauri-drag-region': '', style: 'position:fixed;top:0;left:0;right:0;height:36px' }),
    h('div', {}, h('h2', {}, 'Set up Precap'), h('p.note', {}, 'Precap needs a few macOS permissions to record. You can change them any time in System Settings.')),
    h('div.sc-card', {}, V.perms.map(row)),
    screen && screen.status !== 'granted' ? h('div.hint', {}, 'Without Screen Recording, recordings will be blank.') : null,
    h('div.sp'), h('div.fr', {}, btn('Continue', () => invoke('finish_onboarding'), 'pri')));
}

async function refresh(force) {
  let perms;
  try { perms = await invoke('permission_status'); } catch (e) { return log('permission_status: ' + e); }
  if (!V.perms) V.screen0 = perms.some((p) => p.id === 'screen' && p.status !== 'granted');
  // Re-render only on change so hover and focus survive the polling.
  if (force || !V.perms || JSON.stringify(perms) !== JSON.stringify(V.perms)) { V.perms = perms; render(); }
}
window.addEventListener('focus', () => refresh());
setInterval(refresh, 1500);
refresh();
