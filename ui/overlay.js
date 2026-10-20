'use strict';
// Post-recording card for local Instant mode (RECORDER-SPEC E / INSTANT-ACCOUNT-SPEC 3).
Object.assign(ICONS, {
  copy: '<rect x="9" y="9" width="12" height="12" rx="2"/><path d="M5 15V5a2 2 0 0 1 2-2h8"/>',
  edit: '<path d="M12 20h9M16.5 3.5a2.1 2.1 0 0 1 3 3L7 19l-4 1 1-4z"/>',
  folder: '<path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"/>',
});
const path = new URLSearchParams(location.search).get('path');
const OS = /Win/i.test(navigator.platform) ? 'Explorer' : /Mac/i.test(navigator.platform) ? 'Finder' : 'Folder';
const REVEAL = OS === 'Folder' ? 'Show in Folder' : 'Show in ' + OS;
const card = $('#card');
let info = null, prog = null, saved = null;
const chips = h('div.chips');
const th = h('img#th', { alt: '', draggable: false });
const pill = h('div#pill', { html: ico('check') + 'Copied to clipboard' });
setTimeout(() => (pill.style.opacity = 0), 3000);
const btn = (cls, label, fn, icon, text) => h('button.act.' + cls, { title: label, 'aria-label': label, onclick: fn, html: (icon ? ico(icon) : '') + (text ? `<span>${text}</span>` : '') });
const err = (e) => { prog = { err: true, text: String(e && e.message || e) }; draw(); setTimeout(() => { prog = null; draw(); }, 4000); };
async function run(first, done, call, after) {
  if (prog) return;
  prog = { text: first, pct: 50 }; draw();
  try { const r = await call(); prog = { text: done, pct: 100, link: after && after(r) }; draw(); setTimeout(() => { prog = null; draw(); }, 2000); }
  catch (e) { err(e); }
}
const close = () => invoke('close_overlay').catch(err);
function draw() {
  const b = (c, l, f, i, t) => btn(c, l, f, i, t);
  card.replaceChildren(th, h('div#sh'), chips, pill,
    b('ic.tl', 'Close', close, 'x'),
    b('ic.tr', 'Copy to Clipboard', () => run('Copying to clipboard', 'Copied to clipboard', () => invoke('copy_video', { path })), 'copy'),
    b('lb.bl', 'Edit', async () => { try { await invoke('open_editor', { path }); await invoke('close_overlay'); } catch (e) { err(e); } }, 'edit', 'Edit'),
    b('lb.br', REVEAL, () => info && invoke('reveal', { path: info.output }).catch(err), 'folder', REVEAL),
    b('ctr', 'Export', () => run('Saving video', 'Saved video', () => invoke('export_video', { path }), (r) => { saved = r; return r; }), null, 'Export'));
  if (prog) card.append(h('div#prog' + (prog.err ? '.err' : ''), {},
    prog.link ? h('button', { title: 'Show saved file', onclick: () => invoke('reveal', { path: prog.link }).catch(err) }, prog.text) : prog.text,
    prog.err ? null : h('div.bar', { style: `width:${prog.pct}%` })));
}
draw();
requestAnimationFrame(() => requestAnimationFrame(() => card.classList.add('in')));
(async () => {
  try {
    info = await invoke('instant_info', { path });
    const d = Math.round(info.duration), mb = (info.size / 1048576).toFixed(1);
    chips.append(h('span.chip', {}, `${Math.floor(d / 60)}:${String(d % 60).padStart(2, '0')}`), h('span.chip', {}, mb + ' MB'));
  } catch (e) { err(e); }
  try { th.src = URL.createObjectURL(new Blob([await invoke('instant_thumbnail', { path })], { type: 'image/jpeg' })); } catch (e) { log('thumb: ' + e); }
})();
