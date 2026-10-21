'use strict';
// Shared helpers for the recorder pages (recorder / target / controls / camera).
const TAURI = window.__TAURI__;
const { invoke } = TAURI.core;
const curWin = TAURI.window.getCurrentWindow();
const $ = (s, r = document) => r.querySelector(s);
const log = (m) => { try { invoke('log', { msg: String(m) }); } catch (_) {} };
window.onerror = (m, s, l, c) => log(`error: ${m} @${l}:${c}`);
window.addEventListener('unhandledrejection', (e) => log('unhandled: ' + (e.reason && (e.reason.stack || e.reason.message || e.reason))));

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
const ICONS = {
  monitor: '<rect x="3" y="4" width="18" height="12" rx="2"/><path d="M8 20h8M12 16v4"/>',
  area: '<path d="M4 9V6a2 2 0 0 1 2-2h3M15 4h3a2 2 0 0 1 2 2v3M20 15v3a2 2 0 0 1-2 2h-3M9 20H6a2 2 0 0 1-2-2v-3"/>',
  cam: '<path d="m16 13 5.2 3.5a.5.5 0 0 0 .8-.4V7.9a.5.5 0 0 0-.8-.4L16 11"/><rect x="2" y="6" width="14" height="12" rx="2"/>',
  camoff: '<circle cx="12" cy="12" r="9"/><path d="m5.6 5.6 12.8 12.8"/>',
  mic: '<rect x="9" y="3" width="6" height="11" rx="3"/><path d="M5 11a7 7 0 0 0 14 0M12 18v3"/>',
  chev: '<path d="m6 9 6 6 6-6"/>',
  back: '<path d="M19 12H5M11 6l-6 6 6 6"/>',
  search: '<circle cx="11" cy="11" r="7"/><path d="m21 21-4.3-4.3"/>',
  check: '<path d="m5 12 5 5 9-10"/>',
  rec: '<rect x="3" y="4" width="18" height="16" rx="3"/><path d="m10 9 5 3-5 3z"/>',
  x: '<path d="M18 6 6 18M6 6l12 12"/>',
  clap: '<path d="M4 11h16v8a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2z"/><path d="m4 11-.9-4.2a1 1 0 0 1 .8-1.2l14.8-3.1 .9 4.2M8 7.5l2.5 3M13 6.5l2.5 3"/>',
  timer: '<circle cx="12" cy="13" r="8"/><path d="M12 9v4l2.5 2M9.5 2h5"/>',
  stop: '<circle cx="12" cy="12" r="9"/><rect class="f" x="9" y="9" width="6" height="6" rx="1"/>',
  pause: '<rect class="f" x="6.5" y="5" width="4" height="14" rx="1"/><rect class="f" x="13.5" y="5" width="4" height="14" rx="1"/>',
  play: '<path class="f" d="M7 4.5v15l13-7.5z"/>',
  restart: '<path d="M3 12a9 9 0 1 0 3-6.7L3 8"/><path d="M3 3v5h5"/>',
  trash: '<path d="M4 7h16M10 11v6M14 11v6M6 7l1 13a1 1 0 0 0 1 1h8a1 1 0 0 0 1-1l1-13M9 7V4h6v3"/>',
  grip: '<circle class="f" cx="9" cy="6" r="1.2"/><circle class="f" cx="15" cy="6" r="1.2"/><circle class="f" cx="9" cy="12" r="1.2"/><circle class="f" cx="15" cy="12" r="1.2"/><circle class="f" cx="9" cy="18" r="1.2"/><circle class="f" cx="15" cy="18" r="1.2"/>',
  reset: '<path d="M3 12a9 9 0 1 0 3-6.7L3 8"/><path d="M3 3v5h5"/>',
  fill: '<path d="M4 9V4h5M20 9V4h-5M4 15v5h5M20 15v5h-5"/>',
  expand: '<path d="M15 3h6v6M9 21H3v-6M21 3l-7 7M3 21l7-7"/>',
  blur: '<circle cx="12" cy="9" r="3"/><path d="M7 20c.8-3 2.8-4.5 5-4.5s4.2 1.5 5 4.5M3 6h.01M3 12h.01M3 18h.01M21 6h.01M21 12h.01M21 18h.01"/>',
  mirror: '<path d="M12 3v18M8 7 3 12l5 5zM16 7l5 5-5 5z"/>',
  shape: '<circle cx="12" cy="12" r="8"/>',
  size: '<path d="M15 3h6v6M9 21H3v-6M21 3l-7 7M3 21l7-7"/>',
  shot: '<rect x="3" y="3" width="18" height="18" rx="2"/><circle cx="9" cy="9" r="2"/><path d="m21 15-3.1-3.1a2 2 0 0 0-2.8 0L6 21"/>',
  snap: '<path d="M14.5 4h-5L7 7H4a2 2 0 0 0-2 2v9a2 2 0 0 0 2 2h16a2 2 0 0 0 2-2V9a2 2 0 0 0-2-2h-3z"/><circle cx="12" cy="13" r="3"/>',
  tele: '<rect x="4" y="3" width="16" height="14" rx="2"/><path d="M8 8h8M8 12h5M9 21h6M12 17v4"/>',
  win: '<rect x="3" y="4" width="18" height="16" rx="2"/><path d="M3 9h18M6.5 6.5h.01M9.5 6.5h.01"/>',
  gear: '<circle cx="12" cy="12" r="3"/><path d="M12 2v3M12 19v3M2 12h3M19 12h3M4.9 4.9 7 7M17 17l2.1 2.1M4.9 19.1 7 17M17 7l2.1-2.1"/>',
  warn: '<path d="M12 3 2 20h20z"/><path d="M12 10v4M12 17h.01"/>',
  zap: '<path d="M13 2 4 14h7l-1 8 9-12h-7z"/>',
};
const ico = (n, s) => `<svg class="i" viewBox="0 0 24 24"${s ? ` style="width:${s}px;height:${s}px"` : ''}>${ICONS[n]}</svg>`;
const I = (n, s) => h('span', { html: ico(n, s), style: 'display:contents' });

// Small popup menu (used inside overlay windows).
let curMenu = null;
function closeMenu() { if (curMenu) { curMenu.remove(); curMenu = null; } }
document.addEventListener('pointerdown', (e) => { if (curMenu && !curMenu.contains(e.target)) closeMenu(); }, true);
function openMenu(anchor, items, cur, pick, header) {
  closeMenu();
  const r = anchor.getBoundingClientRect();
  const m = h('div.menu', {}, header ? h('div.mh', {}, header) : null,
    items.map(([v, l]) => h('button', { class: v === cur ? 'on' : '', onclick: () => { closeMenu(); pick(v); } }, l)));
  document.body.append(m);
  const mw = m.offsetWidth, mh = m.offsetHeight;
  m.style.left = Math.min(Math.max(6, r.left), innerWidth - mw - 6) + 'px';
  m.style.top = (r.bottom + 4 + mh > innerHeight ? Math.max(6, r.top - mh - 4) : r.bottom + 4) + 'px';
  curMenu = m;
}

const COUNTDOWNS = [[0, 'Off'], [3, '3 seconds'], [5, '5 seconds'], [10, '10 seconds']];
const getSettings = () => invoke('get_settings');
const setSettings = (patch) => invoke('set_settings', { patch });

// Key order = Cap's mode pill (Instant, Studio, Screenshot); its Start menu lists Studio first.
const MODES = { instant: ['Instant Mode', 'zap'], studio: ['Studio Mode', 'clap'], screenshot: ['Screenshot Mode', 'snap'] };
const MENU_MODES = ['studio', 'instant', 'screenshot'];
const getMode = () => { const v = localStorage.getItem('precapMode'); return MODES[v] ? v : 'studio'; };
const setMode = (v) => localStorage.setItem('precapMode', v);
