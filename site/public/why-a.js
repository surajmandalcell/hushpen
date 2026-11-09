// Scenes for the Why sections: Works offline, Dictionary and word packs, and Command Mode. Also zooms
// every .why-v > .why-st stage (520 by 390) to fill its box and marks a box .live while it is on
// screen. Each scene loops only while its box is on screen; Reduce Motion shows one still frame.
(() => {
  const calm = matchMedia('(prefers-reduced-motion: reduce)').matches;
  const sleep = ms => new Promise(r => setTimeout(r, ms));
  const SW = 520;

  const boxes = [...document.querySelectorAll('.why-v')];
  const fit = v => { if (v.querySelector(':scope > .why-st')) v.style.setProperty('--z', (v.clientWidth / SW).toFixed(4)); };
  const ro = new ResizeObserver(es => es.forEach(e => fit(e.target)));
  const live = new WeakMap();
  boxes.forEach(v => {
    fit(v); ro.observe(v);
    new IntersectionObserver(([e]) => { live.set(v, e.isIntersecting); v.classList.toggle('live', e.isIntersecting); }, { threshold: 0.15 }).observe(v);
  });
  const hold = async (v, ms) => { await sleep(ms); while (!live.get(v)) await sleep(200); };
  const run = (v, fn) => (async () => { await hold(v, 300); for (;;) await fn(); })();
  const FB = window.FlowBar;

  // Works offline
  const wo = document.querySelector('[data-wo]');
  if (wo && FB) {
    const v = wo.closest('.why-v');
    const g = wo.querySelector('.wo-wv .a');
    // amplitude, wavelength, phase, stroke, opacity, seconds per wavelength
    const WAVES = [[22, 34, 0, 2.6, 1, 0.9], [15, 46, 1.3, 1.8, 0.5, 1.25], [9, 27, 2.4, 1.4, 0.28, 0.75]];
    g.innerHTML = WAVES.map(([a, l, p, sw, o, d]) => {
      let path = '';
      for (let x = -2 * l; x <= 128; x += 2) path += (path ? 'L' : 'M') + x + ' ' + (50 + a * Math.sin(x / l * 2 * Math.PI + p)).toFixed(2);
      return `<path d="${path}" stroke-width="${sw}" opacity="${o}" style="--l:${l}px;--d:${d}s"/>`;
    }).join('');
    const fb = FB.create(wo.querySelector('[data-fb]'), { scale: 0.9, seed: 11, hidden: true, zoom: 1.6 });
    const lines = [...wo.querySelectorAll('.wo-ln u')], car = wo.querySelector('.wo-car'), cur = wo.querySelector('.wo-cur'), pk = wo.querySelector('.wo-pk');
    const MAX = [178, 152, 104];
    const setLine = (i, w) => { lines[i].style.width = w + 'px'; car.style.transform = `translate(${14 + w + 4}px, ${30 + i * 18}px)`; };
    const type = async i => { let w = 0; while (w < MAX[i]) { w = Math.min(MAX[i], w + 16 + Math.random() * 22); setLine(i, Math.round(w)); await hold(v, 110 + Math.random() * 70); } };
    const off = on => wo.classList.toggle('off', on);
    const spark = (cls, y, frames, ms, host) => { const e = document.createElement('i'); e.className = cls; e.style.top = y + 'px'; host.append(e); e.animate(frames, { duration: ms, easing: 'cubic-bezier(.22,1,.36,1)' }).finished.then(() => e.remove()); };
    // A packet runs at the wall at an even speed, then bounces back into the laptop and fades
    const shoot = async y => {
      const p = document.createElement('i');
      p.className = 'wo-p'; p.style.top = y + 'px'; pk.append(p);
      await p.animate([{ transform: 'none', opacity: 1 }, { transform: 'translateX(118px)', opacity: 1 }], { duration: 560, easing: 'linear', fill: 'forwards' }).finished;
      spark('wo-ring', y + 6, [{ transform: 'scale(.25)', opacity: 1 }, { transform: 'scale(1.5)', opacity: 0 }], 650, pk);
      spark('wo-flash', y + 6, [{ opacity: 1 }, { opacity: 0 }], 750, wo);
      await p.animate([{ transform: 'translateX(118px)', opacity: 1 }, { transform: 'translateX(40px)', opacity: 0 }], { duration: 700, easing: 'cubic-bezier(.22,1,.36,1)', fill: 'forwards' }).finished;
      p.remove();
    };
    // The translate property, not transform: the press scale then shrinks the pointer at its tip and leaves its place alone
    const move = (x, y) => { cur.style.translate = `${x}px ${y}px`; };
    if (calm) {
      off(true); wo.classList.add('talk'); setLine(0, MAX[0]); setLine(1, MAX[1]); setLine(2, 64); fb.set('listening').show();
    } else run(v, async () => {
      off(false); wo.classList.remove('talk', 'clear'); fb.hide();
      lines.forEach((u, i) => setLine(i, 0)); setLine(0, 0);
      await hold(v, 300);
      wo.classList.add('talk'); fb.set('listening').show();
      await Promise.all([type(0), (async () => { for (const [ms, y] of [[250, 150], [520, 184], [460, 216]]) { await hold(v, ms); shoot(y); } })()]);
      await hold(v, 600);
      cur.style.transition = 'none'; move(170, 140); void cur.offsetWidth; cur.style.transition = '';
      cur.classList.add('on'); move(47, 49);
      await hold(v, 850);
      cur.classList.add('dn'); await sleep(130); off(true); cur.classList.remove('dn');
      await hold(v, 350);
      move(150, 104); cur.classList.remove('on');
      await type(1); await type(2);
      wo.classList.remove('talk'); fb.set('transcribing');
      await hold(v, 700);
      fb.set('pasted');
      await hold(v, 1000);
      fb.hide(); wo.classList.add('clear');
      await hold(v, 700);
    });
  }

  // Dictionary and word packs: [right, as heard, where the spelling comes from, punctuation]
  const wd = document.querySelector('[data-wd]');
  if (wd) {
    const v = wd.closest('.why-v'), tx = wd.querySelector('.wd-tx'), chips = [...wd.querySelectorAll('[data-pk]')];
    const TOK = [['Ask'], ['Siobhan', 'Shivon', 'dict'], ['to'], ['book'], ['a'], ['flight'], ['to'], ['Reykjavík', 'Rakeyavik', 'dict', '.'], ['\n'],
      ['Mañana', 'Manyana', 'dict'], ['tenemos'], ['reunión', 'reunion', 'dict', ','], ["let's"], ['postpone'], ['the'], ['Kubernetes', 'Cubernetis', 'tech'], ['demo.']];
    const chars = s => [...s].map((c, n) => `<i style="--n:${n}">${c}</i>`).join('');
    // Punctuation flips with its word, so it never stands apart while the spelling changes
    tx.innerHTML = TOK.map(([r, h, from, p = '']) => r === '\n' ? '<br>' : `<span class="wd-w">${h ? `<span class="wdf" data-from="${from}"><span class="wdf-a"${p ? ' style="--pr:.3em"' : ''}>${chars(h + p)}</span><span class="wdf-b">${chars(r + p)}</span></span>` : r}</span>`).join(' ');
    const ws = [...tx.querySelectorAll('.wd-w')], flips = [...tx.querySelectorAll('.wdf')];
    // The stage is zoomed; offsetWidth may or may not include the zoom, so divide by the stage's own ratio
    // Measured again each loop, so a late font or a new zoom never leaves a gap
    const measure = () => { const k = wd.offsetWidth / SW || 1; flips.forEach(f => { f.a = f.firstChild.offsetWidth / k; f.b = f.lastChild.offsetWidth / k; }); };
    // A shorter right spelling waits for the old letters to fade, so it never runs into the next word
    const fix = (f, on) => { f.classList.toggle('fix', on); f.style.transitionDelay = on && f.b < f.a ? '.24s' : '0s'; f.style.width = (on ? f.b : f.a).toFixed(1) + 'px'; };
    const caret = document.createElement('span');
    caret.className = 'wd-c';
    const from = f => chips.find(c => c.dataset.pk === f.dataset.from)?.classList.add('on');
    measure();
    if (calm) {
      ws.forEach(w => w.classList.add('in')); flips.forEach(f => fix(f, true)); chips.forEach(c => c.classList.add('on'));
    } else {
      flips.forEach(f => fix(f, false));
      run(v, async () => {
        ws.forEach(w => w.classList.remove('in')); measure(); flips.forEach(f => fix(f, false)); chips.forEach(c => c.classList.remove('on')); caret.remove();
        await hold(v, 80);
        wd.classList.remove('fade');
        await hold(v, 420);
        for (const w of ws) {
          w.classList.add('in'); w.after(caret);
          const f = w.querySelector('.wdf');
          if (f) hold(v, 520).then(() => { fix(f, true); from(f); });
          await hold(v, f ? 430 : 140);
          if (f && w.textContent.endsWith('.')) await hold(v, 520);
        }
        await hold(v, 2600);
        wd.classList.add('fade');
        await hold(v, 520);
      });
    }
  }

  // Command Mode
  const we = document.querySelector('[data-we]');
  if (we && FB) {
    const v = we.closest('.why-v'), tx = we.querySelector('.we-tx'), say = [...we.querySelectorAll('.we-say span')], keys = [...we.querySelectorAll('.key')];
    const OLD = 'Send me the report by Friday. No excuses.', NEW = 'Could you send me the report by Friday? Thanks so much!';
    const words = s => s.split(' ').map((w, n, a) => `<span class="cw" style="--n:${n}">${w}${n < a.length - 1 ? ' ' : ''}</span>`).join('');
    const put = (s, cls) => { tx.className = 'we-tx' + (cls ? ' ' + cls : ''); tx.innerHTML = words(s); };
    const fb = FB.create(we.querySelector('[data-fb]'), { scale: 1.5, seed: 21, hidden: true, zoom: 1.4 });
    const press = on => keys.forEach(k => k.classList.toggle('down', on));
    if (calm) {
      put(NEW); say.forEach(s => s.classList.add('in')); fb.set('pasted').show();
    } else {
      put(OLD);
      run(v, async () => {
        await hold(v, 500);
        tx.classList.add('sel');
        await hold(v, 1000);
        press(true); fb.set('listening').show();
        await hold(v, 450);
        for (const [i, ms] of [[0, 260], [1, 220], [2, 900]]) { say[i].classList.add('in'); await hold(v, ms); }
        press(false); fb.set('cleaning');
        await hold(v, 1100);
        tx.classList.add('out');
        await hold(v, 300);
        put(NEW, 'new');
        await hold(v, 700);
        fb.set('pasted');
        await hold(v, 1300);
        fb.hide(); say.forEach(s => s.classList.remove('in'));
        await hold(v, 1400);
        tx.classList.add('out');
        await hold(v, 320);
        put(OLD, 'back');
      });
    }
  }
})();
