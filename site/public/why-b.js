// Why Hushpen, part B: the Read aloud scene. It loops while it is on screen and stops where it is
// when it leaves; with Reduce Motion it shows one still frame.
(() => {
  const calm = matchMedia('(prefers-reduced-motion: reduce)').matches;
  const eo = p => 1 - Math.pow(1 - Math.min(1, Math.max(0, p)), 4);

  // A loop of `period` ms. Each beat fires once per pass; frame(t) runs on every animation frame.
  function loop(el, { period, beats = [], frame, still }) {
    if (calm) { still(); return; }
    let t = -1, last = 0, raf = 0;
    const fire = (a, b) => beats.forEach(([at, fn]) => { if (at > a && at <= b) fn(); });
    const tick = now => {
      const dt = last ? Math.min(50, now - last) : 0;
      last = now;
      let b = t + dt;
      if (b >= period) { fire(t, period); b -= period; fire(-1, b); } else fire(t, b);
      t = b;
      if (frame) frame(Math.max(0, t));
      raf = requestAnimationFrame(tick);
    };
    new IntersectionObserver(([e]) => {
      if (e.isIntersecting && !raf) { last = 0; raf = requestAnimationFrame(tick); }
      else if (!e.isIntersecting && raf) { cancelAnimationFrame(raf); raf = 0; }
    }, { threshold: 0.2 }).observe(el);
  }

  // Offset of el inside the stage, in stage pixels
  const at = (el, st) => { let x = 0, y = 0; for (let n = el; n && n !== st; n = n.offsetParent) { x += n.offsetLeft; y += n.offsetTop; } return { x, y, w: el.offsetWidth, h: el.offsetHeight }; };

  const MIC = '<svg viewBox="0 0 24 24"><path d="M12 2a3 3 0 0 0-3 3v7a3 3 0 0 0 6 0V5a3 3 0 0 0-3-3Z"/><path d="M19 10v2a7 7 0 0 1-14 0v-2M12 19v3"/></svg>';

  // Read aloud. Three sentences, each in another voice: Heart, then Heart with 30% Adam, then a
  // cloned voice. The capsule names the voice, the text lights up word by word.
  function voice(st) {
    const LINES = [
      { name: 'Heart', text: 'Hi Elena, the draft is ready.', at: 520, step: 250 },
      { name: 'Heart with 30% Adam', text: 'I moved the demo to ten, so we have time to test.', at: 2900, step: 205 },
      { name: 'Your voice', text: 'Thanks, talk soon.', at: 5750, step: 280 },
    ];
    const T = { play: 300, mix: 2420, me: 5300, stop: 6750, back: 7250, period: 8000 };
    const BARS = 22;
    let id = 0;
    const words = [];
    const html = LINES.map((l, s) => l.text.split(' ').map((w, j) => { words.push({ at: l.at + j * l.step, d: l.step, s, n: id }); return `<span class="wv-wd" data-i="${id++}">${w}</span>`; }).join(' ')).join(' ');
    st.innerHTML = `<div class="wv-hud"><i class="wv-pb"><svg class="wv-pl" viewBox="0 0 14 14"><path d="M3.5 1.8v10.4c0 .6.6.9 1.1.6l8-5.2c.5-.3.5-.9 0-1.2l-8-5.2c-.5-.3-1.1 0-1.1.6Z"/></svg><svg class="wv-sq" viewBox="0 0 14 14"><rect x="2.5" y="2.5" width="9" height="9" rx="1.6"/></svg></i>` +
      `<span class="wv-w">${'<i></i>'.repeat(BARS)}</span><span class="wv-n">${LINES.map(l => `<span>${l.name}</span>`).join('')}</span></div>` +
      `<p class="wv-tx"><i class="wv-hl"></i>${html}</p>` +
      `<div class="wv-vo"><div class="wv-ch"><span class="wv-c" data-v="heart">Heart</span><span class="wv-c" data-v="adam">Adam<em>&nbsp;0%</em></span><span class="wv-c">Anna</span><span class="wv-c" data-v="me">${MIC}Your voice</span></div>` +
      `<div class="wv-mx"><span>Mix with Adam</span><i class="wv-tr"><b></b><i></i></i><em>0%</em></div></div>`;
    const q = s => st.querySelector(s);
    const tx = q('.wv-tx'), hl = q('.wv-hl'), bars = [...st.querySelectorAll('.wv-w i')], nm = q('.wv-n'), names = [...nm.children];
    const spans = [...tx.querySelectorAll('.wv-wd')], chips = { heart: q('[data-v=heart]'), adam: q('[data-v=adam]'), me: q('[data-v=me]') };
    const mx = q('.wv-mx'), tr = q('.wv-tr'), pct = q('.wv-mx em'), cpct = q('[data-v=adam] em');
    let seed = 7;
    const rnd = () => (seed = (seed * 16807) % 2147483647) / 2147483647;
    const base = bars.map(() => 0.45 + 0.55 * rnd()), amp = words.map(() => 0.7 + 0.3 * rnd());
    const S = { on: null, s: -1, cur: -2, mix: -1, y: -1, read: words.map(() => false) };

    const place = i => {
      if (i < 0) { hl.classList.remove('on'); return; }
      const b = at(spans[i], tx), jump = !hl.classList.contains('on') || b.y !== S.y;
      S.y = b.y;
      // On a new line, or back from hidden, the mark lands on the word instead of sweeping across the text
      if (jump) hl.style.transition = 'opacity .25s';
      hl.style.transform = `translate(${b.x - 6}px, ${b.y - 1}px)`;
      hl.style.width = b.w + 12 + 'px'; hl.style.height = b.h + 2 + 'px';
      hl.classList.add('on');
      if (jump) { void hl.offsetWidth; hl.style.transition = ''; }
    };
    const frame = t => {
      const on = t >= T.play && t < T.stop;
      if (on !== S.on) { st.classList.toggle('on', on); S.on = on; }
      const s = t >= T.back ? 0 : t >= T.me ? 2 : t >= T.mix ? 1 : 0;
      if (s !== S.s) {
        names.forEach((n, j) => { n.classList.toggle('was', j === S.s); n.classList.toggle('on', j === s); });
        nm.style.width = names[s].offsetWidth + 'px';
        chips.heart.classList.toggle('on', s < 2); chips.me.classList.toggle('on', s === 2);
        chips.adam.classList.toggle('mx', s === 1); mx.classList.toggle('off', s === 2);
        S.s = s;
      }
      let cur = -1;
      words.forEach((w, j) => {
        const r = t >= w.at && t < T.back;
        if (r !== S.read[j]) { spans[j].classList.toggle('r', r); S.read[j] = r; }
        if (t >= w.at && t < w.at + w.d) cur = j;
      });
      if (cur !== S.cur) { if (S.cur >= 0) spans[S.cur].classList.remove('c'); if (cur >= 0) spans[cur].classList.add('c'); place(cur); S.cur = cur; }
      const m = t < T.mix ? 0 : t < T.back ? 0.3 * eo((t - T.mix) / 650) : 0.3 * (1 - eo((t - T.back) / 650));
      if (m !== S.mix) { tr.style.setProperty('--v', m.toFixed(4)); pct.textContent = Math.round(m * 100) + '%'; if (s === 1) cpct.textContent = '\u00A0' + Math.round(m * 100) + '%'; S.mix = m; }
      // Bars swell once per word, so the wave follows the speech
      let env = on ? 0.08 : 0;
      if (cur >= 0) { const p = (t - words[cur].at) / words[cur].d; env = Math.pow(Math.sin(Math.PI * p), 0.7) * amp[cur]; }
      const sp = [1, 0.8, 1.15][s];
      bars.forEach((b, i) => {
        const n = Math.abs(Math.sin(i * 0.83 + t * 0.011 * sp) * 0.6 + Math.sin(i * 0.37 - t * 0.017 * sp) * 0.4);
        b.style.transform = `scaleY(${Math.min(1, 0.14 + env * (0.3 + 0.7 * n) * base[i]).toFixed(3)})`;
      });
    };
    // The width of the capsule follows the voice name, so measure once the fonts are in
    const ready = () => { if (S.s >= 0) nm.style.width = names[S.s].offsetWidth + 'px'; if (S.cur >= 0) place(S.cur); };
    document.fonts?.ready.then(ready);
    addEventListener('resize', ready);
    frame(0);
    loop(st, { period: T.period, frame, still: () => frame(4300) });
  }

  document.querySelectorAll('[data-wb="voice"]').forEach(voice);
})();
