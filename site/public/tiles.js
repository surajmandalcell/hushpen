// Tile motion: each feature tile plays its demo as a short looping scene, a slow camera push toward
// each shot's focus with crossfades between shots.
//   TileMotion.register(tile, { shots: [{ cap, html, ms, focus: [x%, y%] }] })
// A tile can register at any time; calls made before this file runs can wait in window.TileMotionQ.
// Inside shot html, <div data-fb data-state="pasted" data-from="cleaning" data-after="300" data-enter>
// becomes a live flow bar (TileMotion.bar builds it). A scene runs only while its stage is on screen;
// with Reduce Motion it shows one still frame.
// Camera: a tile registered with { cine: true } keeps the light tile look and gets one camera that zooms
// and pans to the action, with no chip. A shot adds
//   cam: [x%, y%, zoom] where the camera ends, cam0: where it starts (default: where the last shot ended),
//   join: true for a seamless cut from the last shot (same scene, new state).
(() => {
  const calm = matchMedia('(prefers-reduced-motion: reduce)').matches;
  const E = 'cubic-bezier(.22,1,.36,1)';
  const STOP = Symbol('stop');
  const sleep = ms => new Promise(r => setTimeout(r, ms));
  const clamp = (v, a, b) => Math.min(b, Math.max(a, v));
  // The camera view: the stage zoomed by k with the point (x%, y%) at its center, never past the stage edge.
  // Percent translates are of the stage itself, so a resize keeps the view.
  const pose = ([x, y, k]) => {
    const t = c => (clamp(.5 - c / 100 * k, 1 - k, 0) * 100).toFixed(2);
    return `translate(${t(x)}%, ${t(y)}%) scale(${k})`;
  };
  let ready = false, seed = 11;
  const queue = [], scenes = new Map();

  class Scene {
    constructor(tile, o) {
      this.tile = tile; this.o = o; this.tok = 0; this.live = false; this.skip = false; this.cur = null;
      const host = this.host = tile.querySelector('.demo') || tile;
      host.classList.add('tmo-host');
      const el = this.el = document.createElement('div');
      this.cine = !!o.cine;
      el.className = this.cine ? 'tmo cine' : 'tmo';
      el.setAttribute('role', 'img');
      el.setAttribute('aria-label', `${tile.querySelector('h3')?.textContent || ''}: ${o.shots.map(s => s.cap).filter(Boolean).join(', ')}`);
      el.innerHTML = `<div class="tmo-cam">${o.shots.map(s => `<div class="tmo-shot"><div class="tmo-in">${s.html}</div></div>`).join('')}</div>` +
        (this.cine ? '' : '<span class="tmo-cap"></span>');
      host.append(el);
      [this.cam, this.capL] = el.children;
      this.shots = [...this.cam.children].map((n, i) => {
        const s = { ...o.shots[i], n, inn: n.firstChild, fbs: [] };
        if (window.FlowBar) n.querySelectorAll('[data-fb]').forEach(f => {
          const d = f.dataset;
          s.fbs.push({ state: d.state || 'listening', from: d.from, after: +d.after || 160, enter: 'enter' in d, fb: FlowBar.create(f, { scale: 3, seed: seed++, state: d.from || d.state || 'listening' }) });
        });
        return s;
      });
      this.io = new IntersectionObserver(([e]) => { this.live = e.isIntersecting; el.classList.toggle('live', this.live); this.sync(); });
      this.io.observe(el);
      document.addEventListener('visibilitychange', () => this.sync());
      if (host.hasAttribute('data-noedit') || this.cine) el.addEventListener('click', () => { this.skip = true; });
    }

    destroy() { this.tok++; this.io.disconnect(); this.el.remove(); this.host.classList.remove('tmo-host'); }

    // The camera is a script animation, which the stylesheet cannot pause off screen, so a scene
    // that scrolls away holds it until it is back.
    sync() {
      if (!this.cine) return;
      const on = this.live && !document.hidden;
      this.cam.getAnimations({ subtree: true })
        .filter(a => a.constructor === Animation)
        .forEach(a => { if (!on && a.playState === 'running') a.pause(); else if (on && a.playState === 'paused') a.play(); });
    }

    start() {
      const tok = ++this.tok;
      this.reset();
      if (calm) return this.still();
      (async () => { try { for (;;) await this.push(tok); } catch (e) { if (e !== STOP) console.error(e); } })();
    }

    reset() {
      this.el.getAnimations({ subtree: true }).forEach(a => { if (a.constructor === Animation) a.cancel(); });
      this.shots.forEach(s => { s.n.classList.remove('on', 'out'); s.n.style.cssText = ''; s.inn.style.transformOrigin = ''; s.inn.style.transform = ''; s.fade = null; });
      if (this.capL) { this.capL.textContent = ''; delete this.capL.dataset.t; }
      this.cur = null; this.skip = false;
    }

    // Waits ms of on-screen time. A newer start() ends the loop; a click on the stage ends the wait.
    async beat(tok, ms) {
      let left = ms;
      while (left > 0) {
        const t = performance.now();
        await sleep(Math.min(left, 100));
        if (tok !== this.tok) throw STOP;
        if (this.skip) { this.skip = false; return; }
        if (this.live && !document.hidden) left -= performance.now() - t;
      }
    }

    cap(t, hard) {
      const c = this.capL;
      if (!c || c.dataset.t === (t || '')) return;
      c.dataset.t = t || '';
      c.textContent = t || '';
      if (!hard && t) c.animate([{ opacity: 0, transform: 'translateY(3px)' }, { opacity: 1, transform: 'none' }], { duration: 260, easing: E });
    }

    // A flow bar takes its first state while its shot is still hidden, so it does not morph into view
    prepFb(s) { s.fbs.forEach(x => x.fb.set(calm ? x.state : x.from || x.state)); }
    playFb(s) {
      if (calm) return;
      const tok = this.tok;
      s.fbs.forEach(x => {
        if (x.enter) { x.fb.hide(); requestAnimationFrame(() => requestAnimationFrame(() => tok === this.tok && x.fb.show())); } else x.fb.show();
        if (x.from) setTimeout(() => { if (tok === this.tok && s.n.classList.contains('on')) x.fb.set(x.state); }, x.after);
      });
    }

    enter(s) {
      s.fade?.cancel(); s.fade = null;
      this.prepFb(s);
      s.n.classList.remove('on', 'out'); void s.n.offsetWidth; s.n.classList.add('on');
      this.playFb(s);
    }

    show(s, how = 'cut', dur = 480) {
      const p = this.cur;
      this.cur = s;
      if (p && p !== s) {
        p.n.classList.remove('on');
        if (how === 'fade') {
          p.n.classList.add('out');
          p.n.animate([{ filter: 'blur(0)' }, { opacity: 0, filter: 'blur(5px)' }], { duration: dur, easing: E });
          setTimeout(() => { if (this.cur !== p) p.n.classList.remove('out'); }, dur);
        }
      }
      this.enter(s);
      // A soft blur dissolve, so two texts never read on top of each other
      if (how === 'fade' && p && p !== s) s.fade = s.n.animate([{ opacity: 0, filter: 'blur(5px)' }, { opacity: 1, filter: 'blur(0)' }], { duration: dur, easing: E });
    }

    // The largest zoom up to max, and an origin near the shot's focus (on the flow bar's center line when
    // the shot has a flow bar), that keep the shot's content
    // inside the stage and out of the chip band at the top. A side the content already bleeds off
    // (a window cut by the stage edge) is free. The content is the shot's top elements, or the
    // tile's aim selector, without the wallpaper.
    aim(s, max) {
      s.inn.getAnimations().forEach(a => a.cancel());
      const S = this.el.getBoundingClientRect(), W = S.width, H = S.height;
      let l = Infinity, t = Infinity, r = -Infinity, b = -Infinity;
      const parts = this.o.aim ? s.inn.querySelectorAll(this.o.aim) : [];
      // A paragraph is as wide as its column, so it is measured by the lines of text it holds
      const box = c => {
        if (!c.matches('p')) return c.getBoundingClientRect();
        const g = document.createRange();
        g.selectNodeContents(c);
        return g.getBoundingClientRect();
      };
      for (const c of parts.length ? parts : s.inn.children) {
        if (c.matches('.tmf-bg')) continue;
        const q = box(c);
        if (!q.width || !q.height) continue;
        l = Math.min(l, q.left - S.left); t = Math.min(t, q.top - S.top); r = Math.max(r, q.right - S.left); b = Math.max(b, q.bottom - S.top);
      }
      const [fx, fy] = s.focus || [50, 50];
      if (l === Infinity) return [max, W * fx / 100, H * fy / 100];
      // The camera pivots on the flow bar, so it stays on the line it is centered on while the camera closes in
      const pb = s.inn.querySelector('.tmo-fb')?.getBoundingClientRect();
      const px = pb?.width ? pb.left + pb.width / 2 - S.left : null;
      // Each side keeps its margin, or the gap it has now when that is smaller
      const side = (a, z, size, m0, m1) => [a > 1 ? Math.min(a, m0) : null, z < size - 1 ? Math.min(size - z, m1) : null];
      const [ml, mr] = side(l, r, W, 8, 8), [mt, mb] = side(t, b, H, 36, 8);
      const fit = (a, z, size, m0, m1) => (m0 != null && m1 != null ? (size - m0 - m1) / (z - a) : Infinity);
      const kx = px == null ? fit(l, r, W, ml, mr)
        : Math.min(ml != null && l < px ? (px - ml) / (px - l) : Infinity, mr != null && r > px ? (W - mr - px) / (r - px) : Infinity);
      const k = Math.max(1, Math.min(max, kx, fit(t, b, H, mt, mb)));
      const org = (a, z, size, m0, m1, f) => {
        if (k < 1.001) return f;
        const hi = m0 != null ? (k * a - m0) / (k - 1) : Infinity, lo = m1 != null ? (k * z - size + m1) / (k - 1) : -Infinity;
        return lo > hi ? (lo + hi) / 2 : clamp(f, lo, hi);
      };
      return [k, px ?? org(l, r, W, ml, mr, W * fx / 100), org(t, b, H, mt, mb, H * fy / 100)];
    }

    still() {
      const s = this.shots[this.o.still ?? this.shots.length - 1];
      this.show(s);
      this.cap(s.cap, true);
      if (this.cine) s.inn.style.transform = pose(this.lim(s.cam || [...(s.focus || [50, 50]), 1.4]));
    }

    // A narrow stage (a phone) closes in less, so the action keeps its edges
    lim([x, y, k]) { return [x, y, this.cam.clientWidth < 420 ? Math.min(k, 1.3) : k]; }

    // One camera that carries over from shot to shot. It starts each shot where the last one
    // ended and glides to the shot's own pose, then holds while the action plays.
    async film(tok) {
      let prev = [50, 50, 1];
      for (const s of this.shots) {
        const d = Math.max(1500, (s.ms || 1400) * 1.25);
        const to = s.cam || [...(s.focus || [50, 50]), 1.4];
        s.inn.getAnimations().forEach(a => a.cancel());
        this.show(s, 'fade', s.join ? 180 : 420);
        // A shot that names its start pose is a pan: the camera holds on the first action, then follows to the next
        const a = pose(this.lim(s.cam0 || prev)), b = pose(this.lim(to));
        s.inn.animate(s.cam0 ? [{ transform: a }, { transform: a, offset: .22, easing: E }, { transform: b }] : [{ transform: a }, { transform: b }],
          { duration: s.cam0 ? d : Math.min(d, 1600), easing: s.cam0 ? 'linear' : E, fill: 'forwards' });
        await this.beat(tok, d);
        prev = to;
      }
    }

    // Camera push: a slow scale toward each shot's focus point, with crossfades
    async push(tok) {
      if (this.cine) return this.film(tok);
      for (const s of this.shots) {
        const d = Math.max(1500, (s.ms || 1400) * 1.25);
        this.show(s, 'fade', 520);
        const [k, ox, oy] = this.aim(s, 1.15);
        s.inn.style.transformOrigin = `${ox}px ${oy}px`;
        s.inn.animate([{ transform: 'scale(1)' }, { transform: `scale(${k})` }], { duration: d + 520, easing: 'cubic-bezier(.33,1,.68,1)', fill: 'forwards' });
        this.cap(s.cap);
        await this.beat(tok, d);
      }
    }
  }

  function mount(tile, o) {
    scenes.get(tile)?.destroy();
    const s = new Scene(tile, o);
    scenes.set(tile, s);
    s.start();
  }
  function register(tile, o) {
    if (typeof tile === 'string') tile = document.querySelector(tile);
    if (!tile || !o?.shots?.length) return;
    if (!ready) { queue.push([tile, o]); return; }
    mount(tile, o);
  }
  function init() {
    ready = true;
    queue.splice(0).forEach(a => mount(...a));
  }

  // Markup helpers for shots: words that come in one by one, and a flow bar
  const words = (text, cls = 'tmo-w') => text.split(' ').map((w, n) => `<span class="${cls}" style="--n:${n}">${w}</span>`).join(' ');
  const slot = b => `<div class="tmo-slot">${b || ''}</div>`;
  const bar = (state, o = {}) => `<div class="fbh tmo-fb"${o.w ? ` style="--w:${o.w}px"` : ''}><div data-fb data-state="${state}"${o.from ? ` data-from="${o.from}" data-after="${o.after || 160}"` : ''}${o.enter ? ' data-enter' : ''}></div></div>`;

  window.TileMotion = { register, words, bar };
  (window.TileMotionQ || []).forEach(a => register(...a));
  window.TileMotionQ = { push: a => register(...a) };
  if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', init); else init();

  // The four tiles of this section: the flow bar, rule cleanup, AI cleanup, and Command Mode
  const SAID = 'Booked the big room for Thursday at 3.';
  // The flow bar tile: the Notes app with its folder column hidden, over the wallpaper, and the flow bar
  // floating at the bottom of the screen. Every size is in em, so the window keeps the proportions of the app.
  const NI = {
    side: '<rect x="3" y="4.5" width="18" height="15" rx="3"/><path d="M9.5 4.5v15"/>',
    trash: '<path d="M4 7h16M10 3.5h4M6.5 7l.8 12a1.5 1.5 0 0 0 1.5 1.4h6.4a1.5 1.5 0 0 0 1.5-1.4l.8-12M10 11v6M14 11v6"/>',
    pen: '<path d="M11 4H6.5A2.5 2.5 0 0 0 4 6.5v11A2.5 2.5 0 0 0 6.5 20h11a2.5 2.5 0 0 0 2.5-2.5V13"/><path d="M18.4 3.6a1.8 1.8 0 0 1 2.5 2.5L12.5 14.5 9 15.4l.9-3.5z"/>',
    check: '<path d="m4 6.5 1.5 1.5L8.5 5M4 12.5 5.5 14l3-3M4 18.5 5.5 20l3-3M12 6.5h8M12 12.5h8M12 18.5h8"/>',
    table: '<rect x="3.5" y="5" width="17" height="14" rx="2.5"/><path d="M3.5 10h17M3.5 14.5h17M9.2 5v14M14.8 5v14"/>',
    photo: '<rect x="3.5" y="5" width="17" height="14" rx="2.5"/><circle cx="9" cy="10" r="1.5"/><path d="m4 17 5-4.5 3 2.5 3-3 5 4.5"/>',
    share: '<path d="M12 14V3.5M8.5 7 12 3.5 15.5 7M8 10.5H6.5A2.5 2.5 0 0 0 4 13v5.5A2.5 2.5 0 0 0 6.5 21h11a2.5 2.5 0 0 0 2.5-2.5V13a2.5 2.5 0 0 0-2.5-2.5H16"/>',
    lock: '<rect x="5" y="10.5" width="14" height="10" rx="2.5"/><path d="M8.5 10.5V8a3.5 3.5 0 0 1 7 0v2.5"/>',
    search: '<circle cx="11" cy="11" r="6.5"/><path d="m20 20-4.2-4.2"/>',
  };
  const ni = n => `<svg class="tmn-i" viewBox="0 0 24 24" aria-hidden="true">${NI[n]}</svg>`;
  const NOTES = [
    ['Today', [['Team sync', '9:41 AM'], ['Groceries', '8:12 AM', 'Oat milk, lemons, basil']]],
    ['Yesterday', [['Marco’s birthday', 'Yesterday', 'Order the cake by Friday']]],
    ['Previous 7 Days', [['Packing list', 'Monday', 'Rain shell, adapter, wool socks'], ['Book club', 'Sunday', 'Noah picks the next one']]],
  ];
  const desk = (tx, b) => {
    const rows = NOTES.map(([h, rs]) => `<h5>${h}</h5>` + rs.map(([t, d, p], i) => {
      const sel = h === 'Today' && !i;
      return `<div class="tmn-n${sel ? ' sel' : ''}"><b>${t}</b><span><em>${d}</em> ${sel ? (tx ? SAID : 'No additional text') : p}</span></div>`;
    }).join('')).join('');
    return `<i class="tmf-bg"></i><div class="tmn"><div class="tmn-tb"><div class="tmn-lc"><span class="tmn-lt"><i></i><i></i><i></i></span><span class="tmn-g">${ni('side')}${ni('trash')}${ni('pen')}</span></div>` +
      `<div class="tmn-ec"><span class="tmn-g"><u>Aa</u>${ni('check')}${ni('table')}${ni('photo')}</span><span class="tmn-g tmn-r">${ni('share')}${ni('lock')}</span><span class="tmn-sr">${ni('search')}<span>Search</span></span></div></div>` +
      `<div class="tmn-l">${rows}</div><div class="tmn-e"><small>June 3, 2025 at 9:41 AM</small><h4>Team sync</h4><p>${tx}<i class="tmo-caret"></i></p></div></div>${b}`;
  };
  register('.t.fbt', {
    shots: [
      { cap: 'Listening', ms: 1500, focus: [50, 84], cam: [50, 76, 1.7], html: desk('', bar('listening', { enter: 1 })) },
      { cap: 'Transcribing', ms: 1100, focus: [50, 84], cam: [50, 76, 1.75], join: 1, html: desk('', bar('transcribing', { from: 'listening' })) },
      { cap: 'Cleaning up', ms: 1100, focus: [50, 84], cam: [50, 76, 1.8], join: 1, html: desk('', bar('cleaning', { from: 'transcribing' })) },
      { cap: 'Pasted', ms: 2000, focus: [50, 38], cam0: [50, 50, 1.05], cam: [50, 50, 1.1], join: 1, html: desk(words(SAID), bar('undo', { from: 'cleaning' })) },
    ],
    cine: true,
  });

  // Rule cleanup, from the rule tests: the rules mark each word they drop, cut them, and the rest closes up.
  // A dropped group keeps its trailing space inside it, so the cut closes the gap. --w is its width in em.
  const RU = ['um', 'call', 'the', 'the', 'team', 'at', '3, no wait,', '4'];
  const CUT = { 0: ['Filler', 1.65], 3: ['Repeat', 2.05], 6: ['Take-back', 5.6] };
  const ru = mode => {
    let n = 0, k = 0;
    return RU.map((w, i) => {
      const c = CUT[i], cut = mode === 'cut';
      let t = mode === 'heard' ? w.split(' ').map(x => `<span class="tmo-w" style="--n:${n++}">${x}</span>`).join(' ') : w;
      if (cut && i === 1) t = '<span class="tmr-cap"><i>c</i><i>C</i></span>all';
      if (cut && i === RU.length - 1) t += '<span class="tmr-dot">.</span>';
      if (!c) return t + (i < RU.length - 1 ? ' ' : '');
      return `<span class="tmr-g${mode === 'heard' ? '' : ' tmr-m'}${cut ? ' tmr-c' : ''}" data-k="${c[0]}" style="--k:${k++};--w:${c[1]}em">${t}${cut ? ' ' : ''}</span>${cut ? '' : ' '}`;
    }).join('');
  };
  register('.t.ru', {
    shots: [
      { cap: 'Heard', ms: 1500, focus: [40, 46], html: `<p class="tmr-tx">${ru('heard')}</p>${slot(bar('listening', { enter: 1 }))}` },
      { cap: 'Rules', ms: 1800, focus: [30, 40], html: `<p class="tmr-tx">${ru('rules')}</p>${slot(bar('transcribing', { from: 'listening' }))}` },
      { cap: 'Cut', ms: 1300, focus: [36, 44], html: `<p class="tmr-tx">${ru('cut')}</p>${slot(bar('transcribing'))}` },
      { cap: 'Pasted', ms: 1700, focus: [44, 46], html: `<div class="tmo-card tmr-out"><small>#team</small><p>Call the team at 4.<i class="tmo-caret"></i></p></div>${slot(bar('pasted', { from: 'transcribing', after: 120 }))}` },
    ],
    aim: '.tmr-tx, .tmr-out, .tmo-slot .fb',
  });

  const HEARD = 'hi team the docs the tests and the numbers all look good can we ship friday thanks';
  const seg = (to, from) => `<div class="tms${from != null ? ' go' : ''}" style="--to:${to};--from:${from ?? to}"><i></i>${['General', 'Message', 'Email', 'Notes'].map((p, j) => `<b${j === to ? ' class="to"' : ''}>${p}</b>`).join('')}</div>`;
  // A wide stage shows what was heard beside the result
  const card = (app, ls) => `<div class="tma-pair"><p class="tma-src">${HEARD}</p><svg class="tma-arr" viewBox="0 0 24 24" aria-hidden="true"><path d="M4 12h15M13 6l6 6-6 6"/></svg><div class="tmo-card tma-out${app ? '' : ' tma-nt'}">${app ? `<small>${app}</small>` : ''}${ls.map((l, n) => `<span class="tmo-w tmo-l" style="--n:${n * 2 + 1}">${l}</span>`).join('')}</div></div>`;
  register('.t.ai', {
    shots: [
      { cap: 'Heard', ms: 1500, focus: [50, 44], html: `<p class="tma-raw" style="--st:24ms">${words(HEARD)}</p>${slot(bar('listening', { enter: 1 }))}` },
      { cap: 'Profile', ms: 1400, focus: [64, 44], html: `${seg(2, 0)}${slot(bar('cleaning', { from: 'transcribing', after: 200 }))}` },
      { cap: 'Email', ms: 1700, focus: [36, 44], html: card('To: Team', ['Hi team,', 'The docs, tests, and numbers all look good. Can we ship Friday?', 'Thanks']) },
      { cap: 'Notes', ms: 1700, focus: [36, 50], html: card('', ['All look good:', '- The docs', '- The tests', '- The numbers', 'Can we ship Friday?']) },
    ],
    aim: '.tma-raw, .tms, .tma-src, .tma-out, .tmo-slot .fb',
  });

  // Command Mode: one reply draft stays in place while the flow bar row under it changes
  const A = 'thanks for the update, i will look at it soon', B = 'Thank you for the update. I will review it shortly.';
  const doc = t => `<div class="tmo-card tmc-doc"><small>Reply to Elena</small><p>${t}</p></div>`;
  const say = (b, q = '') => slot(`<div class="tmo-row">${b}${q}</div>`);
  register('.t.cm', {
    shots: [
      { cap: 'Select', ms: 1400, focus: [34, 40], html: `${doc(`<span class="tmo-sel sweep">${A}</span>`)}${say('')}` },
      { cap: 'Say it', ms: 1700, focus: [40, 70], html: `${doc(`<span class="tmo-sel">${A}</span>`)}${say(bar('listening', { w: 118, enter: 1 }), `<b class="tmo-q">${words('"Make it formal"')}</b>`)}` },
      { cap: 'Rewriting', ms: 1200, focus: [40, 50], html: `${doc(`<span class="tmo-sel tmo-dim">${A}</span>`)}${say(bar('cleaning', { w: 118, from: 'listening' }))}` },
      { cap: 'Replaced', ms: 1900, focus: [40, 40], html: `${doc(words(B))}${say(bar('pasted', { w: 66, from: 'cleaning', after: 120 }))}` },
    ],
  });
})();

// Features heading: the letters of "Messy" wobble, then "Clean text." wipes in, each time it comes on screen.
(() => {
  const calm = matchMedia('(prefers-reduced-motion: reduce)').matches;
  const E = 'cubic-bezier(.22,1,.36,1)';
  const seen = (el, on) => new IntersectionObserver(([e]) => e.isIntersecting && on()).observe(el);
  if (calm) return;
  const once = (el, fn) => { let last = -1e9; seen(el, () => { if (performance.now() - last > 4000) { last = performance.now(); fn(); } }); };
  document.querySelectorAll('.fh-mess').forEach(m => once(m, () => {
    [...m.children].forEach((l, i) => l.animate([{ transform: 'none', offset: 0 }, { transform: `translateY(${i % 2 ? -5 : 4}px) rotate(${i % 2 ? 12 : -12}deg)`, offset: .45 }], { duration: 900, delay: 250 + i * 40, easing: E }));
    m.parentElement.querySelector('.fh-clean')?.animate([{ clipPath: 'inset(-20% 100% -20% -5%)' }, { clipPath: 'inset(-20% -5% -20% -5%)' }], { duration: 800, delay: 650, easing: E, fill: 'backwards' });
  }));
})();
