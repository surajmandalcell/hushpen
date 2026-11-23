// Home speed chart: draws the medians that home_chart.py writes into data-cmp. Smooth monotone lines (a curve
// never dips past a real point), hover or tap one line or legend row to fade the rest, and one long camera
// pull-out: it starts on the short clips, zoomed close, and pulls out to the 30 minute meeting, then eases back.
(() => {
  const host = document.querySelector('[data-cmp]');
  if (!host) return;
  const D = JSON.parse(host.dataset.cmp), XL = D.x, S = D.s, fig = host.closest('figure'), key = fig.querySelector('.cmp-key');
  const svg = host.querySelector('svg'), cap = host.querySelector('.cmp-cap');
  const calm = matchMedia('(prefers-reduced-motion: reduce)').matches;
  const NS = 'http://www.w3.org/2000/svg';
  const el = (t, a, p) => { const e = document.createElementNS(NS, t); for (const k in a) e.setAttribute(k, a[k]); if (p) p.appendChild(e); return e; };
  const defs = el('defs', {}, svg);
  const clip = el('rect', {}, el('clipPath', { id: 'cmp-clip' }, defs)), clipA = el('rect', {}, el('clipPath', { id: 'cmp-axes' }, defs));
  const gY = el('g', { 'clip-path': 'url(#cmp-axes)' }, svg), gX = el('g', { 'clip-path': 'url(#cmp-axes)' }, svg);
  const gL = el('g', { 'clip-path': 'url(#cmp-clip)' }, svg), gT = el('g', {}, svg), gH = el('g', {}, svg);
  const L = {};
  for (const s of S) {
    const g = el('g', { class: 'ser', 'data-s': s.id }, gL);
    const top = el('g', { class: 'top', opacity: 0 }, gT);
    el('line', { x1: 0, x2: 10, y1: -4, y2: -4, stroke: s.c, 'stroke-width': 2.4, 'stroke-linecap': 'round' }, top);
    el('text', { x: 15, class: 'tt' }, top).textContent = `${s.name} ${s.v[0]} s ↑`;
    L[s.id] = { s, g, top, to: 0, ln: el('path', { fill: 'none', stroke: s.c, 'stroke-width': s.w, 'stroke-linejoin': 'round', 'stroke-linecap': 'round' }, g),
      dot: el('circle', { r: s.w + 1.2, fill: s.c }, g), hit: el('path', { class: 'hit', fill: 'none', 'data-s': s.id }, gH) };
  }
  function fmt(v) { return (Number.isInteger(v) ? v : v < 10 ? v.toFixed(1) : Math.round(v)) + ' s'; }

  // Highlight one line: the rest drop to low opacity, and the picked one comes to the front.
  let pin = null;
  const hl = id => {
    svg.classList.toggle('dim', !!id); key.classList.toggle('dim', !!id);
    for (const s of S) { L[s.id].g.classList.toggle('on', s.id === id); gL.appendChild(L[s.id].g); }
    // .r: the row sits in the right half, so its model tooltip opens leftward and stays on screen
    const kr = key.getBoundingClientRect();
    key.querySelectorAll('li').forEach(li => { const r = li.getBoundingClientRect(); li.classList.toggle('on', li.dataset.s === id); li.classList.toggle('r', r.left + r.width / 2 - kr.left > kr.width / 2); });
    if (id) gL.appendChild(L[id].g);
  };
  const over = e => { const t = e.target.closest('[data-s]'); if (t) hl(t.dataset.s); };
  const tap = e => { const t = e.target.closest('[data-s]'); if (t) { pin = pin === t.dataset.s ? null : t.dataset.s; hl(pin); } };
  for (const n of [key, gH]) { n.addEventListener('pointerover', over); n.addEventListener('pointerleave', () => hl(pin)); n.addEventListener('click', tap); }

  // Geometry in real pixels, so text stays crisp; the svg's size comes from CSS (aspect-ratio).
  let W = 0, H = 0, PL, PR, PT, PB, narrow;
  const size = () => {
    const r = svg.getBoundingClientRect(); W = Math.round(r.width); H = Math.round(r.height); narrow = W < 600;
    PL = narrow ? 40 : 52; PR = narrow ? 18 : 26; PT = 10; PB = narrow ? 24 : 30;
    svg.setAttribute('viewBox', `0 0 ${W} ${H}`); svg.setAttribute('font-size', narrow ? 11 : 13);
    clip.setAttribute('x', PL - 6); clip.setAttribute('y', PT - 6); clip.setAttribute('width', W - PL - PR + 12); clip.setAttribute('height', H - PT - PB + 8);
    clipA.setAttribute('x', -2); clipA.setAttribute('y', PT - 12); clipA.setAttribute('width', W + 2); clipA.setAttribute('height', H - PT + 12);
  };

  // Monotone cubic (Fritsch-Carlson): tangents are a weighted harmonic mean of the neighbouring slopes, 0 at a
  // turn, so the curve between two points never goes above or below them.
  const mono = P => {
    const n = P.length, m = [], t = [];
    if (n < 3) return P.map((p, i) => (i ? 'L' : 'M') + p[0].toFixed(2) + ',' + p[1].toFixed(2)).join('');
    for (let i = 0; i < n - 1; i++) m[i] = (P[i + 1][1] - P[i][1]) / (P[i + 1][0] - P[i][0]);
    t[0] = m[0]; t[n - 1] = m[n - 2];
    for (let i = 1; i < n - 1; i++) {
      const a = P[i][0] - P[i - 1][0], b = P[i + 1][0] - P[i][0];
      t[i] = m[i - 1] * m[i] <= 0 ? 0 : 3 * (a + b) / ((2 * b + a) / m[i - 1] + (b + 2 * a) / m[i]);
    }
    let d = `M${P[0][0].toFixed(2)},${P[0][1].toFixed(2)}`;
    for (let i = 0; i < n - 1; i++) {
      const h = (P[i + 1][0] - P[i][0]) / 3;
      d += `C${(P[i][0] + h).toFixed(2)},${(P[i][1] + t[i] * h).toFixed(2)},${(P[i + 1][0] - h).toFixed(2)},${(P[i + 1][1] - t[i + 1] * h).toFixed(2)},${P[i + 1][0].toFixed(2)},${P[i + 1][1].toFixed(2)}`;
    }
    return d;
  };

  const ticks = new Map(), xls = XL.map(s => { const t = el('text', { class: 't', 'text-anchor': 'middle' }, gX); t.textContent = s; return { t, o: 0 }; });
  const top = (xm, drop) => { // tallest visible value; with drop, a line far above the rest (Buzz on short clips) may run off the top
    const mx = S.map(s => Math.max(...s.v.filter((v, i) => v != null && i <= xm + 1e-6))).sort((a, b) => b - a);
    while (drop && mx[0] > 2.5 * mx[1]) mx.shift();
    return mx[0] * 1.15;
  };
  let view = { x: XL.length - 1, y: top(XL.length - 1) };

  function draw(snap, dt = 16) {
    if (!W) return;
    const { x: xm, y: ym } = view, pw = W - PL - PR, ph = H - PT - PB;
    const X = i => PL + i / xm * pw, Y = v => PT + (1 - v / ym) * ph;
    const k = snap ? 1 : 1 - Math.exp(-dt / 110);
    // y ticks are keyed by value, so they glide with the scale and fade in or out when the step changes.
    const step = [0.5, 1, 2, 5, 10, 20, 25, 50].find(s => ym / s <= 5.5);
    const want = new Set();
    for (let v = 0; v <= ym + 1e-6; v += step) want.add(+v.toFixed(2));
    for (const v of want) if (!ticks.has(v)) {
      const g = el('g', { opacity: 0 }, gY); el('line', { class: 'g' }, g);
      el('text', { class: 't', 'text-anchor': 'end' }, g).textContent = fmt(v);
      ticks.set(v, { g, o: 0 });
    }
    for (const [v, tk] of ticks) {
      const y = Y(v), tgt = want.has(v) && y >= PT - 1 ? 1 : 0;
      tk.o += (tgt - tk.o) * k;
      if (!tgt && tk.o < 0.02) { tk.g.remove(); ticks.delete(v); continue; }
      tk.g.setAttribute('opacity', tk.o.toFixed(3)); tk.g.setAttribute('transform', `translate(0 ${y.toFixed(2)})`);
      const [ln, tx] = tk.g.children;
      ln.setAttribute('x1', PL); ln.setAttribute('x2', W - PR); tx.setAttribute('x', PL - 8); tx.setAttribute('y', narrow ? 3.8 : 4.6);
    }
    // x labels fade out past the right edge, and every other one fades when they get crowded.
    const crowd = pw / xm < (narrow ? 58 : 70);
    xls.forEach((xl, i) => {
      const tgt = i <= xm + 0.02 && !(crowd && i % 2 === 0) ? 1 : 0;
      xl.o += (tgt - xl.o) * k;
      xl.t.setAttribute('x', X(i).toFixed(2)); xl.t.setAttribute('y', H - PB + (narrow ? 17 : 22)); xl.t.setAttribute('opacity', xl.o.toFixed(3));
    });
    let row = 0;
    for (const s of S) {
      const l = L[s.id], P = s.v.map((v, i) => v == null ? null : [X(i), Y(v)]).filter(Boolean), d = mono(P);
      l.ln.setAttribute('d', d); l.hit.setAttribute('d', d);
      const e = P[P.length - 1]; l.dot.setAttribute('cx', e[0].toFixed(2)); l.dot.setAttribute('cy', e[1].toFixed(2));
      // A line that is above the top on every visible length gets a small label at the top edge instead.
      const off = s.v.every((v, i) => v == null || i > xm + 1e-6 || v > ym);
      l.to += ((off ? 1 : 0) - l.to) * k;
      l.top.setAttribute('opacity', l.to.toFixed(3));
      l.top.setAttribute('transform', `translate(${PL + 10} ${PT + 16 + row * 18})`);
      if (off) row++;
    }
  }

  // The loop: hold on the short clips, one long pull-out that starts quick and settles slowly, hold on the full
  // chart, then ease back. x and y move in log space with the same easing, so the zoom rate feels even.
  const bez = (x1, y1, x2, y2) => t => { // CSS cubic-bezier: solve x for t by Newton steps, then return y
    let u = t; for (let i = 0; i < 6; i++) { const x = 3 * x1 * u * (1 - u) ** 2 + 3 * x2 * u * u * (1 - u) + u ** 3 - t, dx = 3 * x1 * (1 - u) ** 2 + 6 * (x2 - x1) * u * (1 - u) + 3 * (1 - x2) * u * u; if (Math.abs(dx) < 1e-6) break; u -= x / dx; }
    return 3 * y1 * u * (1 - u) ** 2 + 3 * y2 * u * u * (1 - u) + u ** 3;
  };
  const out = bez(0.3, 0.15, 0.2, 1), back = bez(0.65, 0, 0.35, 1), lg = (a, b, e) => Math.exp(Math.log(a) + (Math.log(b) - Math.log(a)) * e);
  const A = { x: 1, y: top(1, true), say: 'A 5 second note' }, Z = { x: XL.length - 1, y: top(XL.length - 1), say: 'Up to a 30 minute meeting' };
  const SEG = [[A, A, 500], [A, Z, 6500, out], [Z, Z, 3000], [Z, A, 1700, back]], TOTAL = SEG.reduce((a, s) => a + s[2], 0);
  const at = ms => {
    let r = ms % TOTAL;
    for (const [a, b, d, f] of SEG) {
      if (r <= d) { const e = f ? f(r / d) : 0; return { x: lg(a.x, b.x, e), y: lg(a.y, b.y, e), say: (e < 0.2 ? a : b).say }; }
      r -= d;
    }
  };
  let said = '';
  const say = txt => {
    if (txt === said) return; said = txt;
    const [a, b] = cap.children; a.textContent = txt; a.className = 'in'; b.className = ''; cap.append(a);
  };
  let t = 0, last = 0, raf = 0, seen = false, held = false;
  const frame = now => {
    const dt = Math.min(64, now - (last || now)); last = now;
    if (!held) t += dt;
    const v = at(t); view = { x: v.x, y: v.y }; say(v.say);
    draw(false, dt);
    raf = requestAnimationFrame(frame);
  };
  const run = () => {
    const go = !calm && seen && !document.hidden;
    if (go && !raf) { last = 0; raf = requestAnimationFrame(frame); }
    if (!go && raf) { cancelAnimationFrame(raf); raf = 0; }
  };
  if (!calm) { const v = at(0); view = { x: v.x, y: v.y }; say(v.say); }
  size(); draw(true);
  new ResizeObserver(() => { const w = Math.round(svg.getBoundingClientRect().width); if (w !== W) { size(); draw(true); } }).observe(svg);
  new IntersectionObserver(([e]) => { seen = e.isIntersecting; run(); }).observe(svg);
  document.addEventListener('visibilitychange', run);
  host.addEventListener('pointerenter', () => { held = true; });
  host.addEventListener('pointerleave', () => { held = false; });
})();
