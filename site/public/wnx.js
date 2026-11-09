// The Windows install on a Windows 11 desktop (light theme): the browser downloads the setup file,
// SmartScreen asks once, setup installs Hushpen for this user, and Hushpen types the first dictation
// into Notepad. It runs on the Linux scene's camera and pointer (lnx.js) and the same logical desktop
// size, with the same step list under it. It plays only while seen. With Reduce Motion it shows its
// last frame.
(() => {
  const host = document.querySelector('#windows .ln-v');
  if (!host || !window.FlowBar || !window.LnScene) return;
  const { frame, cam, beat, move, click, front } = window.LnScene;
  const calm = FlowBar.calm;
  const size = n => window.HushpenReleases.size(n);
  let FILE = 'Hushpen-windows-x64-setup.exe', bytes = 0;
  const SAID = 'Hello from Windows. This is my first dictation, and it never left my computer.';
  const STEPS = [['Download', 'Get the setup file. One file.'], ['Install', 'Windows may warn because the app is not signed yet. Choose More info, then Run anyway.'], ['Talk', 'Press <kbd>Right Alt</kbd> twice or <kbd>Right Alt</kbd> + <kbd>Space</kbd>. Hold <kbd>Right Alt</kbd> to talk.']];
  const PROG = [.13, .33, .57, .8];

  // Windows 11 glyphs from Fluent UI System Icons, and the real app icons (assets/windows/README.md)
  const G = { back: 'arrow_left', reload: 'arrow_clockwise', dl: 'arrow_download', more: 'more_horizontal', lock: 'lock_closed', min: 'subtract', max: 'maximize', x: 'dismiss', plus: 'add', up: 'chevron_up', ok: 'checkmark', gear: 'settings', wifi: 'wifi_1', vol: 'speaker_2', bat: 'battery_10' };
  const g = k => `<img class="g" src="assets/windows/fluent/${G[k]}_20_regular.svg" alt="">`;
  const ib = (k, c = '') => `<span class="ln-ib${c}">${g(k)}</span>`;
  const pos = ([x, y, w, h]) => `left:${x}px;top:${y}px;width:${w}px;height:${h}px`;
  const HP = '<img src="assets/icon-256.webp" alt="">';
  // Taskbar icons: Start, Search, Edge, File Explorer, Notepad, then Hushpen once it is installed
  const img = f => `<img src="assets/windows/${f}.svg" alt="">`;
  const APP = { start: img('start'), search: img('fluent/search_20_regular'), web: img('edge'), fold: img('explorer'), note: img('notepad'), hp: HP };
  const DEFS = '<svg class="ln-defs" width="0" height="0" aria-hidden="true"><linearGradient id="wnx-pt" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#8EC8FF"/><stop offset="1" stop-color="#0B4FD0"/></linearGradient></svg>';
  // Windows 11 keeps minimize, maximize, and close at the right end of the title bar
  const WC = (max = true) => `<span class="wx-wc"><i>${g('min')}</i>${max ? `<i>${g('max')}</i>` : ''}<i>${g('x')}</i></span>`;
  // The Windows pointer: white with a black edge
  const CUR = '<svg class="ln-cur" viewBox="0 0 19 28" aria-hidden="true"><path d="M2 2v20.5l5-4.7 3.2 7.4 3.4-1.5-3.2-7.3H17z" fill="#fff" stroke="#000" stroke-width="1.3" stroke-linejoin="round"/></svg>';
  // A bloom of petals, after the Windows 11 wallpaper
  const WALL = `<svg class="wx-wall" viewBox="0 0 880 495" preserveAspectRatio="xMidYMid slice" aria-hidden="true"><g transform="translate(440 345)" fill="url(#wnx-pt)">${[-66, -33, 0, 33, 66].map(a => `<ellipse rx="74" ry="200" opacity=".55" transform="rotate(${a}) translate(0 -130)"/>`).join('')}</g></svg>`;

  const taskbar = () => `<div class="wx-tb"><div class="wx-tc">${Object.keys(APP).map(k => `<span class="wx-ti" data-a="${k}">${APP[k]}<i class="wx-run"></i></span>`).join('')}</div>
    <div class="wx-tr">${g('up')}<i class="ln-tray"></i><span class="wx-sys">${g('wifi')}${g('vol')}${g('bat')}</span><span class="wx-clk"><b>9:41 AM</b><b>11/2/2026</b></span></div></div>`;

  // The browser on this site's Windows section, with its downloads panel
  const web = r => `<div class="ln-win wx-web" style="${pos(r)}">
    <div class="wx-tabs"><span class="wx-tab">${HP}<span>Hushpen</span>${g('x')}</span>${ib('plus')}<span class="ln-sp"></span>${WC()}</div>
    <div class="wx-nav">${ib('back')}${ib('reload')}<span class="wx-url">${g('lock')}<span>hushpen.mandalsuraj.com/#windows</span></span><span class="ln-ib wx-dlb">${g('dl')}</span>${ib('more')}</div>
    <div class="wx-pg"><h1>The same app <span>on Windows.</span></h1><p>Windows x64. No admin rights needed.</p><span class="wx-pb">${g('dl')}Download setup.exe<small data-wx-meta>Latest version</small></span><i class="wx-ph"></i></div>
    <div class="wx-fly"><b>Downloads</b><div class="wx-it">${HP}<div class="wx-tx"><b data-wx-file>${FILE}</b><i class="wx-bar"><b></b></i><span class="wx-st">Starting…</span><u>Open file</u></div></div></div>
  </div>`;

  // The SmartScreen dialog for an app without a signature
  const ss = r => `<div class="ln-win wx-ss" style="${pos(r)}"><span class="wx-ssx">${g('x')}</span>
    <h3>Windows protected your PC</h3><p>Microsoft Defender SmartScreen prevented an unrecognized app from starting. Running this app might put your PC at risk.</p>
    <u class="wx-mi">More info</u><div class="wx-si"><span>App:</span><b data-wx-file>${FILE}</b><span>Publisher:</span><b>Unknown publisher</b></div>
    <div class="wx-sb"><span class="wx-ra">Run anyway</span><span>Don't run</span></div></div>`;

  // Hushpen Setup, from Installing to its last page
  const setup = r => `<div class="ln-win wx-set" style="${pos(r)}">
    <div class="wx-tt">${HP}<span>Hushpen Setup</span><span class="ln-sp"></span>${WC(false)}</div>
    <div class="wx-sh"><b class="wx-s1">Installing</b><span class="wx-s1">Please wait while Hushpen is being installed.</span><b class="wx-s2">Completing Hushpen Setup</b><span class="wx-s2">Hushpen has been installed on your computer.</span></div>
    <div class="wx-bd"><i class="wx-pr wx-s1"><b></b></i><span class="wx-s1">Extract: hushpen.exe</span><span class="wx-ck wx-s2"><i>${g('ok')}</i>Run Hushpen</span></div>
    <div class="wx-ft"><span class="wx-bt wx-s1 wx-off">Next</span><span class="wx-bt wx-fin wx-s2">Finish</span><span class="wx-bt">Cancel</span></div>
  </div>`;

  const notepad = r => `<div class="ln-win wx-np" style="${pos(r)}">
    <div class="wx-tabs"><span class="wx-tab">${APP.note}<span>Untitled</span>${g('x')}</span>${ib('plus')}<span class="ln-sp"></span>${WC()}</div>
    <div class="wx-menu"><span>File</span><span>Edit</span><span>View</span><span class="ln-sp"></span>${g('gear')}</div>
    <div class="wx-doc"><span class="ln-tx"></span><i class="ln-caret"></i></div>
    <div class="wx-sbar"><span>Ln 1, Col 1</span><span>100%</span><span>Windows (CRLF)</span><span>UTF-8</span></div>
  </div>`;
  const KC = '<div class="ln-kc" aria-hidden="true"><kbd>Right Alt</kbd><span>Press Right Alt twice</span></div>';

  // 16:9, the shape of the stage, so the whole desktop shows the taskbar. Notepad is no wider than the
  // part of the desktop a phone shows, so the dictated text is not cut there.
  const L = { w: 880, h: 495, fb: 1.25, web: [70, 24, 600, 410], ss: [225, 60, 430, 300], set: [250, 86, 390, 280], np: [240, 30, 400, 300], rest: [640, 400] };
  const scene = () => `<div class="ln-dk wx-dk" style="width:${L.w}px;height:${L.h}px">${WALL}${web(L.web)}${ss(L.ss)}${setup(L.set)}${notepad(L.np)}${taskbar()}${KC}<div class="fbh ln-fb" style="--s:${L.fb}"><div data-fb data-hidden></div></div>${CUR}</div>`;

  const ARIA = 'A Windows 11 desktop: the browser downloads the Hushpen setup file, SmartScreen asks once and More info then Run anyway starts setup, and Hushpen types the first dictation into Notepad.';
  host.insertAdjacentHTML('afterbegin', DEFS + `<div class="ln-st ln-wide" role="img" aria-label="${ARIA}"></div><ol class="ln-steps">${STEPS.map(([t, d], i) => `<li><b class="ln-n">${i + 1}</b><div><b class="ln-tt">${t}</b><span class="ln-ds">${d}</span></div></li>`).join('')}</ol>`);

  const $ = (S, sel) => S.el.querySelector(sel);
  const task = (S, k) => $(S, `.wx-ti[data-a=${k}]`);
  // The focused app gets the long blue mark on the taskbar
  const focus = (S, k, w) => {
    S.el.querySelectorAll('.wx-ti.is-act').forEach(n => n.classList.remove('is-act'));
    task(S, k).classList.add('is-in', 'is-run', 'is-act');
    if (w) front(S, w);
  };
  const close = w => w.classList.remove('is-open', 'is-fr');
  const fill = (S, b, ms) => { b.style.transition = S.instant ? 'none' : `transform ${ms}ms linear`; b.style.transform = 'scaleX(1)'; };

  const ST = ['is-open', 'is-fr', 'is-run', 'is-act', 'is-in', 'is-on', 'is-done', 'is-more', 'is-hov', 'is-down', 'is-press'];
  const reset = S => {
    const el = S.el;
    el.classList.add('ln-0');
    el.querySelectorAll(ST.map(c => '.' + c).join()).forEach(n => n.classList.remove(...ST));
    el.querySelectorAll('.ln-tx').forEach(n => { n.textContent = ''; });
    el.querySelectorAll('.wx-bar > b, .wx-pr > b').forEach(b => { b.style.transition = ''; b.style.transform = ''; });
    $(S, '.wx-st').textContent = 'Starting…';
    S.fb?.hide();
    focus(S, 'web', $(S, '.wx-web'));
    S.cur.style.transition = 'none';
    S.cur.style.transform = `translate(${S.L.rest[0]}px, ${S.L.rest[1]}px)`;
    S.focus = null;
    void el.offsetWidth;
    el.classList.remove('ln-0');
  };

  async function download(S) {
    const w = $(S, '.wx-web'), btn = w.querySelector('.wx-pb'), fly = w.querySelector('.wx-fly'), st = fly.querySelector('.wx-st');
    cam(S, btn, 360);
    await move(S, btn, 820);
    btn.classList.add('is-hov');
    await beat(S, 200);
    await click(S, btn);
    w.querySelector('.wx-dlb').classList.add('is-on');
    await beat(S, 160);
    fly.classList.add('is-open');
    cam(S, fly, 320);
    await beat(S, 240);
    fill(S, fly.querySelector('.wx-bar > b'), PROG.length * 300);
    for (const p of PROG) { st.textContent = bytes ? `${size(bytes * p)} of ${size(bytes)}` : 'Downloading'; await beat(S, 300); }
    st.textContent = bytes ? `Done · ${size(bytes)}` : 'Done';
    btn.classList.remove('is-hov');
    fly.classList.add('is-done');
  }

  async function install(S) {
    const fly = $(S, '.wx-fly'), open = fly.querySelector('u'), d = $(S, '.wx-ss'), set = $(S, '.wx-set');
    await beat(S, 300);
    await move(S, open, 620);
    await click(S, open);
    close(fly);
    await beat(S, 280);
    front(S, d);
    cam(S, d, 440);
    await beat(S, 600);
    await move(S, d.querySelector('.wx-mi'), 700);
    await click(S, d.querySelector('.wx-mi'));
    d.classList.add('is-more');
    await beat(S, 800);
    const run = d.querySelector('.wx-ra');
    await move(S, run, 560);
    await click(S, run);
    close(d);
    await beat(S, 260);
    front(S, set);
    cam(S, set, 400);
    await beat(S, 240);
    fill(S, set.querySelector('.wx-pr > b'), 1300);
    await beat(S, 1400);
    set.classList.add('is-done');
    await beat(S, 300);
    const fin = set.querySelector('.wx-fin');
    await move(S, fin, 600);
    await click(S, fin);
    close(set);
    // Finish runs Hushpen: it joins the taskbar and puts its glyph in the tray
    task(S, 'hp').classList.add('is-in', 'is-run');
    await beat(S, 260);
    $(S, '.ln-tray').classList.add('is-on');
    cam(S, [S.L.w - 500, S.L.h - 120, 500, 120], 360);
    await beat(S, 1000);
  }

  async function talk(S) {
    const kc = $(S, '.ln-kc'), tx = $(S, '.ln-tx'), np = $(S, '.wx-np'), [ex, ey, ew, eh] = S.L.np, ti = task(S, 'note');
    cam(S, ti, 440);
    await move(S, ti, 760);
    await click(S, ti);
    await beat(S, 220);
    focus(S, 'note', np);
    // The keys and the flow bar sit on the desktop's center line, so the camera stays centered on that line
    const mid = S.L.w / 2, half = Math.max(mid - ex, ex + ew - mid);
    cam(S, [mid - half, ey, half * 2, S.L.h - ey], 440);
    await beat(S, 260);
    await move(S, [ex + ew - 70, ey + eh - 60], 640);
    kc.classList.add('is-on');
    await beat(S, 420);
    // Right Alt twice starts hands-free, and twice again stops it
    const tap = async () => { kc.classList.add('is-down'); await beat(S, 100); kc.classList.remove('is-down'); };
    const press = async () => { await tap(); await beat(S, 90); await tap(); };
    await press();
    S.fb.set('listening').show();
    await beat(S, 2000);
    await press();
    S.fb.set('transcribing');
    await beat(S, 850);
    S.fb.set('undo');
    tx.textContent = SAID;
    kc.classList.remove('is-on');
    await beat(S, 1900);
    if (!S.instant) S.fb.hide();
    await beat(S, 400);
  }
  const ACTS = [download, install, talk];
  const D = [3800, 7600, 8300];

  const items = [...host.querySelectorAll('.ln-steps > li')];
  const set = i => items.forEach((c, j) => c.classList.toggle('is-on', j === i));
  const P = { vis: false };
  const st = host.querySelector('.ln-st');
  st.innerHTML = scene();
  const el = st.firstElementChild;
  const S = { P, st, el, L, z: 10, focus: null, instant: calm, cur: el.querySelector('.ln-cur') };
  S.fb = FlowBar.create(el.querySelector('[data-fb]'), { scale: L.fb, seed: 47, hidden: true });
  new ResizeObserver(() => frame(S, 0)).observe(st);

  window.HushpenReleases.subscribe(({ windows }) => {
    const a = windows?.assets['windows-setup'];
    if (!a) return;
    FILE = a.name; bytes = a.size;
    host.querySelectorAll('[data-wx-file]').forEach(n => { n.textContent = FILE; });
    host.querySelector('[data-wx-meta]').textContent = `${windows.tag} · ${size(bytes)}`;
    items[0].querySelector('.ln-ds').textContent = `Get the setup file. One file, ${size(bytes)}.`;
  });
  new IntersectionObserver(([e]) => {
    P.vis = e.isIntersecting;
    host.classList.toggle('ln-live', P.vis);
  }, { threshold: 0.15 }).observe(host);
  if (calm) {
    reset(S);
    (async () => { for (const act of ACTS) await act(S); frame(S, 0); })();
    set(-1);
    return;
  }
  const T = { P, instant: false };
  (async () => {
    for (let n = 0; ; n++) {
      if (n) { host.classList.add('ln-fade'); await beat(T, 380); }
      reset(S); set(-1);
      frame(S, n ? 700 : 0);
      host.classList.remove('ln-fade');
      await beat(T, n ? 500 : 300);
      for (let i = 0; i < 3; i++) {
        set(i);
        const t0 = performance.now();
        await beat(T, 160);
        await ACTS[i](S);
        await beat(T, Math.max(250, D[i] - (performance.now() - t0)));
      }
      set(3);
      await beat(T, 1600);
    }
  })();
})();
