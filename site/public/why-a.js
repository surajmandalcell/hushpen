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

  // Works offline: a dictation goes into Notes, the pointer turns Wi-Fi off in its menu, and the next
  // dictation goes in the same way
  const wo = document.querySelector('[data-wo]');
  if (wo && FB) {
    const v = wo.closest('.why-v'), dk = wo.querySelector('.wo-dk'), tx = wo.querySelector('.wo-tx'), cur = wo.querySelector('.wo-cur');
    const wf = wo.querySelector('.wo-wfx'), sw = wo.querySelector('.wo-sw'), DW = 520 / 1.5;
    const fb = FB.create(wo.querySelector('[data-fb]'), { seed: 11, hidden: true, zoom: 2.2 });
    const SAID = ['Boarding now at gate 14.', 'Wi-Fi is off, and it still types.'];
    // Only the sentence that was just pasted fades in; the ones before it stay put
    const put = (n, fresh) => {
      tx.innerHTML = SAID.slice(0, n).map((s, i) => s.split(' ').map((w, k) => `<span class="w${fresh && i === n - 1 ? ' nw' : ''}" style="--n:${k}">${w}</span>`).join(' ')).join('<br>') + '<i class="wo-c"></i>';
    };
    // The desktop is zoomed; both boxes are measured the same way, so the desktop's width in points turns them into points
    const at = (el, fx = 0.5, fy = 0.5) => {
      const d = dk.getBoundingClientRect(), r = el.getBoundingClientRect(), k = d.width / DW;
      return [(r.left - d.left + r.width * fx) / k, (r.top - d.top + r.height * fy) / k];
    };
    // The translate property, not transform: the press scale then shrinks the pointer at its tip and leaves its place alone
    const move = ([x, y], dx = 0, dy = 0) => { cur.style.translate = `${(x + dx - 1.3).toFixed(1)}px ${(y + dy - 1.3).toFixed(1)}px`; };
    const click = async () => { cur.classList.add('dn'); await sleep(130); cur.classList.remove('dn'); };
    const dictate = async n => {
      fb.set('listening').show();
      await hold(v, 1900);
      fb.set('transcribing');
      await hold(v, 650);
      fb.set('pasted'); put(n, true);
      await hold(v, 1000);
      fb.hide();
    };
    if (calm) {
      wo.classList.add('off', 'menu'); put(2); fb.set('pasted').show();
    } else run(v, async () => {
      wo.classList.remove('off', 'menu', 'fade'); put(0);
      await hold(v, 500);
      await dictate(1);
      await hold(v, 500);
      const c = wo.querySelector('.wo-c');
      cur.style.transition = 'none'; move(at(c), 34, 26); void cur.offsetWidth; cur.style.transition = '';
      cur.classList.add('on'); move(at(wf));
      await hold(v, 850);
      await click(); wo.classList.add('menu');
      await hold(v, 650);
      move(at(sw, 0.35));
      await hold(v, 800);
      await click(); wo.classList.add('off');
      await hold(v, 1000);
      move(at(c), 2, 4);
      await hold(v, 850);
      await click(); wo.classList.remove('menu');
      await hold(v, 300);
      cur.classList.remove('on');
      await hold(v, 400);
      await dictate(2);
      await hold(v, 2400);
      wo.classList.add('fade');
      await hold(v, 450);
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
