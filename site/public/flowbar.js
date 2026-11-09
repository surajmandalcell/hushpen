// The flow bar: one capsule that eases between states. Width eases about 280 ms, the old content
// fades out in 100 ms, the new content fades in over 160 ms, and the marks grow from 90% once the
// size has settled. The wave is drawn on a canvas sized for the host's scale, so it stays sharp.
(() => {
  const calm = matchMedia('(prefers-reduced-motion: reduce)').matches;
  const FONT = '500 12px -apple-system, BlinkMacSystemFont, "SF Pro Text", "Inter", system-ui, sans-serif';
  const mctx = document.createElement('canvas').getContext('2d');
  const tw = s => { mctx.font = FONT; return Math.ceil(mctx.measureText(s).width); };
  const svg = p => `<svg viewBox="0 0 11 11" aria-hidden="true">${p}</svg>`;
  const G = {
    x: '<path d="M3.4 3.4 7.6 7.6M7.6 3.4 3.4 7.6"/>',
    tick: '<path d="M2.7 5.9 4.6 7.7 8.4 3.5"/>',
    undo: '<path d="M4.1 2.4 2.2 4.3l1.9 1.9"/><path d="M2.4 4.3h4.1a2.3 2.3 0 0 1 0 4.6H5.2"/>',
    clock: '<circle cx="5.5" cy="5.5" r="3.7"/><path d="M5.5 3.5v2.1l1.3.8"/>',
    bang: '<path d="M5.5 2.7v3.6"/><path d="M5.5 8.3v.05"/>',
  };
  const M = {
    cancel: { g: G.x, c: 'btn', n: 'Cancel' },
    finish: { g: G.tick, c: 'btn fin', n: 'Finish' },
    done: { g: G.tick, c: 'ok' },
    undo: { g: G.undo, c: 'btn', n: 'Undo paste', h: 'Undo' },
    xmark: { g: G.x, c: 'gray' },
    recover: { g: G.undo, c: 'btn', n: 'Recover', h: 'Recover' },
    err: { g: G.bang, c: 'err' },
    hist: { g: G.clock, c: 'btn', n: 'Open history', h: 'Open history' },
  };
  const STATES = {
    idle: { wave: 'idle', w: 67, h: 22, name: 'Show when idle' },
    listening: { L: 'cancel', R: 'finish', wave: 'listen', w: 118, name: 'Listening' },
    transcribing: { wave: 'ripple', w: 118, name: 'Transcribing' },
    preparing: { wave: 'ripple', text: 'Preparing the AI model', lead: 1, name: 'Preparing the AI model' },
    cleaning: { wave: 'ribbon', w: 118, name: 'Cleaning up' },
    undo: { L: 'done', R: 'undo', text: 'Pasted', name: 'Pasted, with Undo' },
    pasted: { text: 'Pasted', name: 'Pasted' },
    copied: { text: 'Copied', name: 'Copied' },
    cancelled: { L: 'xmark', R: 'recover', text: 'Cancelled', name: 'Cancelled, with Recover' },
    error: { L: 'err', R: 'hist', text: 'No speech heard', name: 'Nothing heard' },
  };
  const size = S => {
    if (S.w) return [S.w, S.h || 28];
    const line = Math.max(tw(S.text), S.R && M[S.R].h ? tw(M[S.R].h) : 0);
    const w = S.lead ? 12 + 50 + 8 + line + 12 : S.L ? 64 + line : 24 + line;
    return [Math.min(366, w), 28];
  };

  // A speech envelope: syllables of 110 to 240 ms in words of one to three, short gaps between
  // words and a longer pause now and then. Seeded, so each bar talks the same way every loop.
  const rng = seed => () => { seed |= 0; seed = seed + 0x6D2B79F5 | 0; let t = Math.imul(seed ^ seed >>> 15, 1 | seed); t = t + Math.imul(t ^ t >>> 7, 61 | t) ^ t; return ((t ^ t >>> 14) >>> 0) / 4294967296; };
  function speech(seed) {
    const r = rng(seed), segs = [];
    let end = 0.08;
    const word = () => {
      const n = 1 + Math.floor(r() * 3);
      for (let s = 0; s < n; s++) { const d = 0.11 + r() * 0.13; segs.push([end, end + d, 0.5 + r() * 0.5]); end += d + 0.02 + r() * 0.05; }
      end += r() < 0.16 ? 0.32 + r() * 0.3 : 0.06 + r() * 0.12;
    };
    return t => {
      while (end < t + 1) word();
      while (segs.length && segs[0][1] < t - 1) segs.shift();
      let v = 0.04;
      for (const [a, b, p] of segs) if (t >= a && t <= b) v = Math.max(v, p * Math.pow(Math.sin(Math.PI * (t - a) / (b - a)), 0.6));
      return v;
    };
  }

  const N = 13, WW = 50, WH = 16, TAPER = [0.7, 0.85, 0.95];
  const taper = i => TAPER[Math.min(i, N - 1 - i)] ?? 1;
  const bars = new Set();
  let raf = 0;
  const frame = now => {
    raf = 0;
    let any = false;
    bars.forEach(b => { if (b.live) { b.tick(now); any = true; } });
    if (any) raf = requestAnimationFrame(frame);
  };
  const kick = () => { if (!raf && !calm) raf = requestAnimationFrame(frame); };

  function create(el, opt = {}) {
    const scale = opt.scale || 1;
    el.classList.add('fb');
    el.setAttribute('role', 'status');
    el.innerHTML = '<div class="fb-w off"><canvas></canvas></div>';
    const wl = el.firstChild, cv = wl.firstChild, ctx = cv.getContext('2d');
    const k = Math.min(8, (devicePixelRatio || 1) * scale * (opt.zoom || 1));
    cv.width = Math.round(WW * k); cv.height = Math.round(WH * k);
    ctx.setTransform(k, 0, 0, k, 0, 0);
    const h = new Float32Array(N), y = new Float32Array(N), a = new Float32Array(N), ring = new Float32Array(N);
    const talk = speech(opt.seed || 7), jit = rng((opt.seed || 7) + 99);
    let mode = null, since = 0, last = 0, lastSample = 0, layer = null, visible = false, t0 = performance.now();
    const api = { el, state: null, quiet: false, live: false };
    const target = (i, now) => {
      const s = (now - since) / 1000, d = Math.abs(i - 6), g = Math.exp(-((d / 3.3) ** 2));
      if (mode === 'listen') return [2 + 12 * ring[i] * taper(i), 0, 1];
      if (mode === 'ripple') {
        const slow = s > 2, w = 0.5 + 0.5 * Math.sin(s * (slow ? 3.4 : 7.5) - d * 1.15);
        return [2 + 11 * g * (0.4 + 0.6 * w) * (slow ? 0.8 : 1) * taper(i), 0, 0.42 + 0.58 * g];
      }
      if (mode === 'ribbon') {
        if (s > 2) return [2 + 9 * g * (0.45 + 0.55 * (0.5 + 0.5 * Math.sin(s * 2.6))) * taper(i), 0, 0.5 + 0.5 * g];
        return [2.4, 4.6 * Math.sin(s * 4.8 + i * 0.62), 0.45 + 0.55 * (0.5 + 0.5 * Math.sin(s * 4.8 + i * 0.62 + 1.2))];
      }
      if (mode === 'idle') return i >= 2 && i <= 10 ? [2, 0, 0.6] : [2, 0, 0];
      return [2, 0, 0];
    };
    const draw = () => {
      ctx.clearRect(0, 0, WW, WH);
      ctx.lineCap = 'round'; ctx.lineWidth = 2;
      for (let i = 0; i < N; i++) {
        if (a[i] < 0.02) continue;
        const cx = i * 4 + 1, cy = WH / 2 + y[i], half = Math.max(0, h[i] / 2 - 1);
        ctx.strokeStyle = `rgba(255,255,255,${a[i].toFixed(3)})`;
        ctx.beginPath(); ctx.moveTo(cx, cy - half); ctx.lineTo(cx, cy + half + 0.001); ctx.stroke();
      }
    };
    const sample = now => {
      const t = (now - t0) / 1000;
      for (let i = 0; i < N - 1; i++) ring[i] = ring[i + 1];
      ring[N - 1] = api.quiet ? 0.03 : Math.min(1, talk(t) * (0.82 + 0.36 * jit()));
    };
    api.tick = now => {
      if (mode === 'listen' && now - lastSample > 52) { sample(now); lastSample = now; }
      const dt = Math.min(0.05, (now - (last || now)) / 1000); last = now;
      const f = Math.min(1, dt * 14);
      for (let i = 0; i < N; i++) { const [th, ty, ta] = target(i, now); h[i] += (th - h[i]) * f; y[i] += (ty - y[i]) * f; a[i] += (ta - a[i]) * f; }
      draw();
    };
    const still = () => {
      const now = since + 1300;
      if (mode === 'listen') for (let i = 0; i < N; i++) ring[i] = [0.3, 0.55, 0.8, 0.5, 0.95, 0.7, 0.4, 0.85, 1, 0.6, 0.75, 0.45, 0.3][i];
      for (let i = 0; i < N; i++) [h[i], y[i], a[i]] = target(i, now);
      draw();
    };
    const sync = () => { api.live = visible && !!mode && !calm; if (api.live) kick(); };
    new IntersectionObserver(([e]) => { visible = e.isIntersecting; sync(); }).observe(el);
    const mark = id => { const m = M[id]; return `<span class="fb-m ${m.c}"${m.n ? ` role="button" aria-label="${m.n}"` : ' aria-hidden="true"'}${m.h ? ` data-h="${m.h}"` : ''}>${svg(m.g)}</span>`; };
    api.set = name => {
      if (name === api.state) return api;
      const S = STATES[name], P = STATES[api.state] || {};
      api.state = name; since = performance.now(); last = 0;
      const [w, hh] = size(S);
      el.style.width = w + 'px'; el.style.height = hh + 'px';
      el.setAttribute('aria-label', S.text || S.name);
      mode = S.wave || null;
      if (S.wave) {
        wl.style.transform = `translateX(${S.lead ? 12 : (w - WW) / 2}px)`;
        wl.style.transitionDelay = P.wave ? '0s' : '0s, .1s';
        wl.classList.remove('off');
        if (!P.wave) { h.fill(2); y.fill(0); a.fill(0); }
      } else { wl.style.transitionDelay = '0s'; wl.classList.add('off'); }
      if (calm) still();
      if (layer) { const old = layer; old.classList.add('out'); setTimeout(() => old.remove(), calm ? 0 : 110); }
      layer = null;
      if (S.text || S.L) {
        const n = document.createElement('div');
        n.className = 'fb-l' + (S.L ? ' c2' : '') + (S.lead ? ' lead' : '') + (calm ? '' : ' pre');
        n.innerHTML = (S.L ? mark(S.L) : '') + (S.text ? `<span class="fb-t">${S.text}</span>` : '<span></span>') + (S.R ? mark(S.R) : '');
        const t = n.querySelector('.fb-t');
        n.querySelectorAll('[data-h]').forEach(b => {
          b.addEventListener('mouseenter', () => { t.textContent = b.dataset.h; });
          b.addEventListener('mouseleave', () => { t.textContent = S.text; });
        });
        el.append(n); layer = n;
        if (!calm) {
          setTimeout(() => { n.classList.remove('pre'); n.classList.add('grow'); }, P.text || P.L ? 100 : 60);
          setTimeout(() => n.classList.remove('grow'), 300);
        }
      }
      sync();
      return api;
    };
    api.show = () => { el.classList.remove('gone'); return api; };
    api.hide = () => { el.classList.add('gone'); return api; };
    if (opt.hidden) el.classList.add('gone');
    bars.add(api);
    api.set(opt.state || 'listening');
    return api;
  }
  window.FlowBar = { create, STATES, calm };
})();