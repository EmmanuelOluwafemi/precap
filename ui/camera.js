'use strict';
// Camera bubble: live preview + hover toolbar (close, size, shape, mirror).
const CamS = { size: 230, shape: 'round', mirror: false, frames: 0 };
const pv = $('#pv'), cv = $('#cv'), msg = $('#msg'), tools = $('#tools');
const { LogicalSize, PhysicalPosition } = TAURI.dpi;

// Resize the window but keep its bottom-right corner where it was.
async function resizeAnchored(w, hh) {
  const [pos, inner, sf] = await Promise.all([curWin.outerPosition(), curWin.innerSize(), curWin.scaleFactor()]);
  await curWin.setSize(new LogicalSize(w, hh));
  await curWin.setPosition(new PhysicalPosition(Math.round(pos.x + inner.width - w * sf), Math.round(pos.y + inner.height - hh * sf)));
}
function layout(resize = true) {
  const { size, shape } = CamS;
  const w = shape === 'full' ? Math.round(size * 16 / 9) : size;
  pv.style.width = w + 'px'; pv.style.height = size + 'px';
  pv.style.borderRadius = shape === 'round' ? '9999px' : `${3 + (size - 150) / 450 * 1.5}rem`;
  cv.classList.toggle('m', CamS.mirror); // mirror affects this preview only, not the recording
  const dpr = devicePixelRatio || 1; cv.width = Math.round(w * dpr); cv.height = Math.round(size * dpr);
  if (resize) resizeAnchored(w, size + 56).catch((e) => log('resize: ' + e));
  invoke('camera_style', { shape, mirror: CamS.mirror }).catch((e) => log('camera_style: ' + e)); // baked into Instant videos
}
function drawTools() {
  tools.innerHTML = '';
  const b = (icon, title, fn, on) => h('button', { title, class: on ? 'on' : '', html: ico(icon), onclick: fn });
  tools.append(
    b('x', 'Close camera', () => invoke('set_camera', { id: null }).catch((e) => log('set_camera: ' + e))),
    b('size', CamS.size < 400 ? 'Larger' : 'Smaller', () => { CamS.size = CamS.size < 400 ? 400 : 230; layout(); drawTools(); }),
    b('shape', 'Shape: ' + CamS.shape, () => { CamS.shape = { round: 'square', square: 'full', full: 'round' }[CamS.shape]; layout(); drawTools(); }),
    b('mirror', 'Mirror', () => { CamS.mirror = !CamS.mirror; layout(false); drawTools(); }, CamS.mirror));
}
// Hover shows the toolbar; leaving hides it.
let hovT = 0;
addEventListener('pointermove', () => { document.body.classList.add('hov'); clearTimeout(hovT); hovT = setTimeout(() => document.body.classList.remove('hov'), 2500); });
document.documentElement.addEventListener('pointerleave', () => document.body.classList.remove('hov'));
pv.addEventListener('pointerdown', (e) => { if (e.button === 0) curWin.startDragging(); });

(async function loop() {
  try {
    const buf = await invoke('camera_frame');
    if (buf && buf.byteLength) {
      const bmp = await createImageBitmap(new Blob([buf], { type: 'image/jpeg' }));
      const g = cv.getContext('2d'), k = Math.max(cv.width / bmp.width, cv.height / bmp.height), dw = bmp.width * k, dh = bmp.height * k;
      g.drawImage(bmp, (cv.width - dw) / 2, (cv.height - dh) / 2, dw, dh);
      if (!CamS.frames++) msg.style.display = 'none';
    }
  } catch (e) { if (!loop.warned) { loop.warned = 1; log('camera_frame: ' + e); } }
  setTimeout(loop, 40);
})();
layout(false); drawTools();
