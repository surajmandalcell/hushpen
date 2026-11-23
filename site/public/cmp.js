// Home speed chart: draws the medians that home_chart.py writes into data-cmp. Smooth monotone lines (a curve
// never dips past a real point), hover or tap one line or legend row to fade the rest. The loop is a quick race on
// the full 30 minute chart: each line draws left to right at 60x real time, its end label lights up as it lands,
// and a big block in the empty top-left counts each gap up until that app is done. Reduce Motion shows the end.
(() => {
  const host = document.querySelector('[data-cmp]');
  if (!host) return;
  const D = JSON.parse(host.dataset.cmp), XL = D.x, S = D.s, LAST = XL.length - 1, key = host.closest('figure').querySelector('.cmp-key');
  const svg = host.querySelector('svg'), calm = matchMedia('(prefers-reduced-motion: reduce)').matches;
  const NS = 'http://www.w3.org/2000/svg';
  const el = (t, a, p) => { const e = document.createElementNS(NS, t); for (const k in a) e.setAttribute(k, a[k]); if (p) p.appendChild(e); return e; };
  const at = (e, a) => { for (const k in a) e.setAttribute(k, a[k]); };
  const cl = (v, a = 0, b = 1) => Math.max(a, Math.min(b, v)), io = t => t < .5 ? 4 * t * t * t : 1 - (-2 * t + 2) ** 3 / 2;
  const clip = el('rect', {}, el('clipPath', { id: 'cmp-clip' }, el('defs', {}, svg)));
  const gG = el('g', {}, svg), gL = el('g', { 'clip-path': 'url(#cmp-clip)' }, svg), gO = el('g', {}, svg), gH = el('g', {}, svg);
  const fmt = v => v + 's', sec = v => (v < 10 ? v.toFixed(1) : Math.round(v)) + 's', ours = s => s.name.startsWith('Hushpen');
  const L = {};
  for (const s of S) {
    const g = el('g', { class: 'ser' + (ours(s) ? '' : ' rv'), 'data-s': s.id }, gL);
    L[s.id] = { s, g, gh: el('path', { fill: 'none', stroke: s.c, 'stroke-width': s.w, 'stroke-linecap': 'round', opacity: .16 }, g),
      ln: el('path', { fill: 'none', stroke: s.c, 'stroke-width': s.w, 'stroke-linejoin': 'round', 'stroke-linecap': 'round', pathLength: 1 }, g),
      dot: el('circle', { r: s.w + 1.2, fill: s.c }, g), hit: el('path', { class: 'hit', fill: 'none', 'data-s': s.id }, gH) };
  }

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

  // Gap block: a big MacWhisper multiple, VoiceInk and Buzz under it, and a bracket from Hushpen N to MacWhisper.
  // Multiples round down, like the perf tiles. Only rivals Hushpen N beats at 30 minutes are listed.
  const by = id => S.find(s => s.id === id), N = by('n'), MW = by('macwhisper'), RIV = ['voiceink', 'buzz'].map(by), nv = N.v[LAST];
  const mx = v => (Math.floor(v * 10 + 1e-9) / 10).toFixed(1) + 'x';
  const HALO = { 'paint-order': 'stroke', stroke: '#fff', 'stroke-width': 6, 'stroke-linejoin': 'round' };
  const br = el('path', { class: 'gap-br', fill: 'none', 'stroke-width': 1.6, 'stroke-linecap': 'round', 'stroke-linejoin': 'round' }, gO);
  const big = el('text', { class: 'gap-big', ...HALO }, gO), sub = el('text', { class: 'gap-sub', ...HALO }, gO);
  sub.textContent = 'faster than MacWhisper';
  const rows = RIV.map(r => { const t = el('text', { class: 'gap-row', ...HALO }, gO), n = el('tspan', { class: 'gap-n' }, t); el('tspan', {}, t).textContent = ' faster than ' + r.name; return { r, t, n }; });
  const items = S.filter(s => s.v[LAST] != null).map(s => ({ s, t: el('text', { class: 'end' + (ours(s) ? ' us' : '') + (s.id === 'n' ? ' n' : '') }, gO) }));

  // Geometry in real pixels, so text stays crisp; the svg's size comes from CSS (aspect-ratio).
  let W = 0, H = 0, PL, PR, PT, PB, narrow, X, Y;
  const ym = Math.max(...S.map(s => Math.max(...s.v.filter(v => v != null)))) * 1.15;
  // Monotone cubic (Fritsch-Carlson): tangents are a weighted harmonic mean of the neighbouring slopes, 0 at a
  // turn, so the curve between two points never goes above or below them.
  const mono = P => {
    const n = P.length, m = [], t = [];
    if (n < 3) return P.map((p, i) => (i ? 'L' : 'M') + p[0].toFixed(2) + ',' + p[1].toFixed(2)).join('');
    for (let i = 0; i < n - 1; i++) m[i] = (P[i + 1][1] - P[i][1]) / (P[i + 1][0] - P[i][0]);
    t[0] = m[0]; t[n - 1] = m[n - 2];
    for (let i = 1; i < n - 1; i++) { const a = P[i][0] - P[i - 1][0], b = P[i + 1][0] - P[i][0]; t[i] = m[i - 1] * m[i] <= 0 ? 0 : 3 * (a + b) / ((2 * b + a) / m[i - 1] + (b + 2 * a) / m[i]); }
    let d = `M${P[0][0].toFixed(2)},${P[0][1].toFixed(2)}`;
    for (let i = 0; i < n - 1; i++) { const h = (P[i + 1][0] - P[i][0]) / 3; d += `C${(P[i][0] + h).toFixed(2)},${(P[i][1] + t[i] * h).toFixed(2)},${(P[i + 1][0] - h).toFixed(2)},${(P[i + 1][1] - t[i + 1] * h).toFixed(2)},${P[i + 1][0].toFixed(2)},${P[i + 1][1].toFixed(2)}`; }
    return d;
  };
  const size = () => {
    const r = svg.getBoundingClientRect(); W = Math.round(r.width); H = Math.round(r.height); narrow = W < 600;
    PL = narrow ? 40 : 52; PR = narrow ? 96 : 150; PT = 10; PB = narrow ? 24 : 30;
    at(svg, { viewBox: `0 0 ${W} ${H}`, 'font-size': narrow ? 11 : 13 });
    at(clip, { x: PL - 6, y: PT - 6, width: W - PL - PR + 12, height: H - PT - PB + 8 });
    X = i => PL + i / LAST * (W - PL - PR); Y = v => PT + (1 - v / ym) * (H - PT - PB);
    gG.replaceChildren();
    for (let v = 0; v <= ym; v += 20) { el('line', { class: 'g', x1: PL, x2: W - PR, y1: Y(v), y2: Y(v) }, gG); el('text', { class: 't', 'text-anchor': 'end', x: PL - 8, y: Y(v) + (narrow ? 3.8 : 4.6) }, gG).textContent = fmt(v); }
    XL.forEach((s, i) => { if (!(narrow && i % 2 === 0)) el('text', { class: 't', 'text-anchor': 'middle', x: X(i), y: H - PB + (narrow ? 17 : 22) }, gG).textContent = s; });
    for (const s of S) {
      const l = L[s.id], P = s.v.map((v, i) => v == null ? null : [X(i), Y(v)]).filter(Boolean), d = mono(P), e = P[P.length - 1];
      l.ln.setAttribute('d', d); l.gh.setAttribute('d', d); l.hit.setAttribute('d', d); at(l.dot, { cx: e[0], cy: e[1] });
    }
    // end labels: "Hushpen " drops on a phone except for N; labels are pushed apart so none overlap
    for (const it of items) it.t.textContent = `${narrow && ours(it.s) && it.s.id !== 'n' ? it.s.name.slice(8) : it.s.name} ${sec(it.s.v[LAST])}`;
    const ys = items.map(it => ({ it, y: Y(it.s.v[LAST]) })).sort((a, b) => a.y - b.y), gap = narrow ? 12.5 : 16;
    for (let i = 1; i < ys.length; i++) ys[i].y = Math.max(ys[i].y, ys[i - 1].y + gap);
    for (const q of ys) at(q.it.t, { x: X(LAST) + (narrow ? 14 : 18), y: (q.y + 4).toFixed(1), 'font-size': narrow ? 10.5 : 13 });
    const bf = narrow ? 50 : 88, x0 = PL + (narrow ? 6 : 18), y = 6 + bf * .8, ys2 = y + (narrow ? 18 : 30);
    at(big, { x: x0, y, 'font-size': bf }); at(sub, { x: x0 + (narrow ? 1 : 3), y: ys2, 'font-size': narrow ? 13 : 19 });
    rows.forEach((r, i) => at(r.t, { x: x0 + (narrow ? 1 : 3), y: ys2 + (narrow ? 20 : 34) + i * (narrow ? 16 : 25), 'font-size': narrow ? 12 : 17 }));
  };

  // The loop, in ms: lines start at F0 and each takes v*1000/SPEED; after the last one lands the chart holds, then
  // everything fades back to faint ghosts and the race starts again from empty lines.
  const SPEED = 60, F0 = 250, HOLD = 5300, FADE = 450, fin = s => F0 + s.v[LAST] * 1000 / SPEED;
  const END = fin(MW) + HOLD, TOTAL = END + FADE + 150;
  let t = 0;
  function frame() {
    if (!W) return;
    const u = calm ? END : t % TOTAL, live = 1 - io(cl((u - END) / FADE));
    for (const s of S) {
      const l = L[s.id], v = s.v[LAST];
      if (v == null) { l.ln.setAttribute('opacity', .5); l.dot.setAttribute('opacity', .5); continue; } // Handy: no 30 minute result
      const p = cl((u - F0) / (v * 1000 / SPEED));
      at(l.ln, { 'stroke-dasharray': `${p} 2`, opacity: p > .005 ? live : 0 }); // 0 below .005 hides the round-cap dot
      l.dot.setAttribute('opacity', (cl((u - fin(s)) / 160) * live).toFixed(3));
    }
    // end labels sit faint from the start and light up as their line lands
    for (const it of items) it.t.setAttribute('opacity', (.3 + .7 * cl((u - fin(it.s)) / 280) * live).toFixed(3));
    // once Hushpen N lands, each gap counts up with the race clock and stops when that app lands
    const a = Math.min(live, cl((u - fin(N)) / 220)).toFixed(3), ev = r => cl((u - F0) * SPEED / 1000, nv, r.v[LAST]);
    big.textContent = mx(ev(MW) / nv);
    rows.forEach(r => { r.n.textContent = mx(ev(r.r) / nv); r.t.setAttribute('opacity', a); });
    big.setAttribute('opacity', a); sub.setAttribute('opacity', a);
    const x = X(LAST) + (narrow ? 7 : 9), c = narrow ? 3.5 : 5;
    at(br, { d: `M${x - c},${Y(nv).toFixed(2)}H${x}V${Y(ev(MW)).toFixed(2)}H${x - c}`, opacity: a });
  }

  let last = 0, raf = 0, seen = false, held = false;
  const loop = now => { const dt = Math.min(64, now - (last || now)); last = now; if (!held) t += dt; frame(); raf = requestAnimationFrame(loop); };
  const run = () => {
    const go = !calm && seen && !document.hidden;
    if (go && !raf) { last = 0; raf = requestAnimationFrame(loop); }
    if (!go && raf) { cancelAnimationFrame(raf); raf = 0; }
  };
  size(); frame();
  new ResizeObserver(() => { const w = Math.round(svg.getBoundingClientRect().width); if (w !== W) { size(); frame(); } }).observe(svg);
  new IntersectionObserver(([e]) => { seen = e.isIntersecting; run(); }).observe(svg);
  document.addEventListener('visibilitychange', run);
  host.addEventListener('pointerenter', () => { held = true; });
  host.addEventListener('pointerleave', () => { held = false; });
})();
