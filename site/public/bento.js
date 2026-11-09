// Feature tiles and the Alpine parts: the flow bar tile, the rule cleanup, the keys, Command Mode,
// Manual, Read aloud, the export chips, the AI profiles, the word packs, and the Screens viewer.
// Each loop runs only while its tile is on screen. Reduce Motion shows one readable frame each.
(() => {
  const calm = matchMedia('(prefers-reduced-motion: reduce)').matches;
  const sleep = ms => new Promise(r => setTimeout(r, ms));
  const bento = document.querySelector('.bento');

  bento?.addEventListener('vary', e => { [0, 1, 2].forEach(i => bento.classList.toggle('m' + i, i === e.detail)); });

  // A loop that holds while its element is off screen
  const live = new WeakMap();
  const watch = el => new IntersectionObserver(([e]) => { live.set(el, e.isIntersecting); el.closest('.t')?.classList.toggle('live', e.isIntersecting); }).observe(el);
  const hold = async (el, ms) => { await sleep(ms); while (!live.get(el)) await sleep(250); };
  const loop = (el, fn, still) => { if (!el) return; if (calm) { still?.(); return; } watch(el); (async () => { await hold(el, 300); for (;;) await fn(); })(); };

  // The flow bar tile: steps through the states by itself, or one by one when clicked
  const ft = document.querySelector('[data-fbtile]');
  if (ft && window.FlowBar) {
    const ORDER = ['listening', 'transcribing', 'cleaning', 'undo', 'pasted', 'copied', 'preparing', 'cancelled', 'error', 'idle'];
    const fb = FlowBar.create(ft.querySelector('[data-fb]'), { scale: 3, seed: 3 });
    const label = ft.querySelector('.fbt-n');
    let i = 0, until = 0;
    const go = n => {
      i = (n + ORDER.length) % ORDER.length;
      fb.set(ORDER[i]);
      label.textContent = FlowBar.STATES[ORDER[i]].name;
      label.classList.remove('swap'); void label.offsetWidth; label.classList.add('swap');
    };
    ft.setAttribute('role', 'button');
    ft.setAttribute('aria-label', 'Show the next flow bar state');
    ft.addEventListener('click', () => { go(i + 1); until = performance.now() + 9000; });
    go(0);
    loop(ft, async () => { await hold(ft, i === 0 ? 3000 : 2200); if (performance.now() > until) go(i + 1); });
  }

  // Rule cleanup: lines from the rule tests, and what the rule pass gives for them
  const ru = document.querySelector('[data-rules]');
  if (ru) {
    const EX = [
      ['meet at 3, no wait, 4', 'Meet at 4.'],
      ['Send it today. Scratch that. Send it tomorrow.', 'Send it tomorrow.'],
      ['um send the the draft to Elena before lunch, no wait, after lunch', 'Send the draft to Elena after lunch.'],
    ];
    const raw = ru.querySelector('.ru-raw'), res = ru.querySelector('.ru-res');
    let n = 0;
    loop(ru, async () => {
      const [a, b] = EX[n++ % EX.length];
      res.classList.add('pre'); raw.textContent = '';
      await hold(ru, 400);
      res.textContent = '';
      for (let c = 1; c <= a.length; c++) { raw.textContent = a.slice(0, c); await sleep(28 + (a[c - 1] === ' ' ? 30 : 0)); }
      await hold(ru, 650);
      res.textContent = b; res.classList.remove('pre');
      await hold(ru, 2800);
    }, () => { raw.textContent = EX[0][0]; res.textContent = EX[0][1]; });
  }

  // Keys: the combinations people know from dictation apps, with Hushpen's own first, each in turn with its
  // keys pressed down together; the label swaps with it. The press lasts as long as the action does: a hold
  // key stays down while you speak, the rest are taps. Each combo is [action, press ms, keys].
  const ks = document.querySelector('[data-keys]');
  if (ks) {
    const GLOBE = '<svg viewBox="0 0 24 24" aria-hidden="true"><circle cx="12" cy="12" r="9"/><path d="M3 12h18M12 3c2.4 2.4 3.6 5.4 3.6 9s-1.2 6.6-3.6 9c-2.4-2.4-3.6-5.4-3.6-9S9.6 5.4 12 3z"/></svg>';
    const key = (sym, name, o = {}) => `<span class="key${o.c ? ' ' + o.c : ''}">${o.side ? `<i>${o.side}</i>` : ''}${sym ? `<small>${sym}</small>` : ''}<b>${name}</b></span>`;
    const OPT = key('⌥', 'option');
    const COMBOS = [
      ['Hands-free', 260, OPT + key('⇥', 'tab', { c: 'k-tab' })],
      ['Hold to dictate', 1300, key(GLOBE, 'fn', { c: 'k-fn' })],
      ['Paste last', 260, OPT + key('', 'space', { c: 'k-sp' })],
      ['Read aloud', 260, key('⌘', 'command', { side: 'right' })],
      ['Cancel', 220, key('⌃', 'control') + OPT],
    ];
    const PRESS = COMBOS.map(c => c[1]);
    ks.querySelector('.sc-k').innerHTML = COMBOS.map((c, i) => `<div class="sc-c${i ? '' : ' on'}">${c[2]}</div>`).join('');
    ks.querySelector('.sc-say').innerHTML = COMBOS.map((c, i) => `<span${i ? '' : ' class="on"'}>${c[0]}</span>`).join('');
    const combos = [...ks.querySelectorAll('.sc-c')], says = [...ks.querySelectorAll('.sc-say span')];
    let at = 0;
    const show = i => {
      combos.forEach((c, j) => c.classList.toggle('on', i === j));
      says.forEach((s, j) => { s.classList.toggle('out', j === at && i !== at); s.classList.toggle('on', i === j); });
      at = i;
      if (!calm) combos[i].querySelectorAll('.key').forEach((k, n) => k.animate([{ opacity: 0, transform: 'translateY(10px)' }, { opacity: 1, transform: 'none' }], { duration: 420, delay: 60 + n * 55, easing: 'cubic-bezier(.22, 1, .36, 1)', fill: 'backwards' }));
    };
    loop(ks, async () => {
      for (let i = 0; i < combos.length; i++) {
        show(i); await hold(ks, 900);
        const kk = combos[i].querySelectorAll('.key');
        kk.forEach(k => k.classList.add('down')); ks.classList.add('dn');
        await sleep(PRESS[i]);
        kk.forEach(k => k.classList.remove('down')); ks.classList.remove('dn');
        await hold(ks, 1100);
      }
    });
  }

  // Command Mode: select, hold the keys, say the change, and the text is replaced
  const cm = document.querySelector('[data-cmd]');
  if (cm && window.FlowBar) {
    const s = cm.querySelector('.cm-s'), q = cm.querySelector('.cm-q');
    const fb = FlowBar.create(cm.querySelector('[data-fb]'), { scale: 1.4, seed: 5, hidden: true });
    const A = 'thanks for the update, i will look at it soon', B = 'Thank you for the update. I will review it shortly.';
    const SAY = '"Make it formal"';
    loop(cm, async () => {
      s.textContent = A; s.classList.remove('sel', 'swap'); q.textContent = ''; fb.hide();
      await hold(cm, 900);
      s.classList.add('sel');
      await hold(cm, 900);
      fb.set('listening').show();
      await hold(cm, 500);
      q.textContent = SAY;
      await hold(cm, 1700);
      fb.set('cleaning');
      await hold(cm, 1600);
      s.classList.add('swap');
      await sleep(200);
      s.style.transition = 'none';
      s.textContent = B; s.classList.remove('sel');
      void s.offsetWidth; s.style.transition = ''; s.classList.remove('swap');
      fb.set('pasted'); q.textContent = '';
      await hold(cm, 1400);
      fb.hide();
      await hold(cm, 2600);
    }, () => { s.textContent = B; q.textContent = 'Said: ' + SAY; fb.el.parentElement.style.display = 'none'; });
  }

  // Manual: a file drops in, waits, runs, and is done, with the same words as the app's queue
  const ma = document.querySelector('[data-manual]');
  if (ma) {
    const z = ma.querySelector('.ma-z'), f = ma.querySelector('.ma-f'), name = ma.querySelector('.ma-n'), st = ma.querySelector('.ma-s'), bar = ma.querySelector('.ma-p b');
    const FILES = ['Interview.wav', 'Lecture.m4a', 'Voice memo.mp3'];
    let n = 0;
    const set = (t, p, ok) => { st.textContent = t; st.classList.toggle('ok', !!ok); bar.style.setProperty('--p', p); };
    loop(ma, async () => {
      set('Waiting', 0); name.textContent = FILES[n++ % FILES.length];
      f.classList.remove('drop'); void f.offsetWidth; f.classList.add('drop');
      await sleep(250); z.classList.add('over');
      await hold(ma, 700); z.classList.remove('over');
      await hold(ma, 900);
      for (let p = 0; p <= 100; p += 4 + Math.round(Math.random() * 6)) { set(p + '%', p / 100); await hold(ma, 110); }
      set('Done', 1, true);
      await hold(ma, 2600);
    }, () => set('Done', 1, true));
  }

  // Read aloud: Kokoro's Mix with slider runs from 10% to 90% in steps of ten
  const ra = document.querySelector('[data-read]');
  if (ra) {
    const w = ra.querySelector('.ra-w'), sl = ra.querySelector('.ra-sl'), pct = ra.querySelector('.ra-v b');
    let seed = 9;
    const rnd = () => (seed = (seed * 16807) % 2147483647) / 2147483647;
    w.innerHTML = Array.from({ length: 36 }, (_, i) => `<i style="--n:${i};--r:${rnd().toFixed(2)};--h:${(0.25 + 0.75 * rnd()).toFixed(2)}"></i>`).join('');
    const mix = p => { pct.textContent = p + '%'; sl.style.setProperty('--v', ((p - 10) / 80).toFixed(3)); };
    const STEPS = [30, 50, 70, 50];
    let n = 0;
    mix(30);
    loop(ra, async () => { await hold(ra, 2400); mix(STEPS[++n % STEPS.length]); });
  }

  // History export formats, one after another
  const ex = [...document.querySelectorAll('.hi-ex span')];
  if (ex.length) {
    let n = 0;
    const on = () => ex.forEach((s, j) => s.classList.toggle('on', j === n % ex.length));
    on();
    loop(ex[0].parentElement, async () => { await hold(ex[0].parentElement, 1500); n++; on(); });
  }

  document.addEventListener('alpine:init', () => {
    // The AI cleanup profiles. Each follows its built-in prompt: Message keeps it casual and short,
    // Email puts the greeting and sign-off on their own lines, Notes lists three or more things.
    Alpine.data('aiProfiles', () => ({
      cur: 0,
      list: [
        { n: 'General', t: 'Hi team, the docs, the tests, and the numbers all look good. Can we ship Friday? Thanks.' },
        { n: 'Message', t: 'Hi team, docs, tests, and numbers all look good. Ship Friday? Thanks' },
        { n: 'Email', t: 'Hi team,\nThe docs, tests, and numbers all look good. Can we ship Friday?\nThanks' },
        { n: 'Notes', t: 'All look good:\n- The docs\n- The tests\n- The numbers\nCan we ship Friday?' },
      ],
      user: false,
      pick(i) { this.cur = i; this.user = true; },
      init() {
        if (calm) return;
        const el = this.$el;
        let seen = false;
        new IntersectionObserver(([e]) => { seen = e.isIntersecting; }).observe(el);
        setInterval(() => { if (seen && !this.user) this.cur = (this.cur + 1) % this.list.length; }, 3200);
      },
    }));

    // The word packs that ship with the app, with their counts of distinct words
    Alpine.data('packs', () => ({
      list: [
        { n: 'Your Dictionary', c: 3, on: true },
        { n: 'Tech terms', c: 221, on: false },
      ],
      timer: 0,
      stop() { clearInterval(this.timer); this.timer = 0; },
      init() {
        if (calm) return;
        const el = this.$el;
        let seen = false, n = 1;
        new IntersectionObserver(([e]) => { seen = e.isIntersecting; }).observe(el);
        this.timer = setInterval(() => { if (!seen) return; const p = this.list[n++ % this.list.length]; p.on = !p.on; }, 1900);
      },
    }));

    // Screens: the real captures, in Light or Dark. A capture loads the first time it is shown or hovered.
    Alpine.data('screens', () => ({
      list: [
        { k: 'home', n: 'Home', l: 'Dictate, and see the last text' },
        { k: 'history', n: 'History', l: 'Search, play, and export' },
        { k: 'files', n: 'Manual', l: 'Transcribe audio files' },
        { k: 'files-read-aloud', n: 'Read aloud', l: 'Text, read in a voice' },
        { k: 'dictionary', n: 'Dictionary', l: 'Your words and word packs' },
        { k: 'models', n: 'Models', l: 'Speech, cleanup, and voices' },
        { k: 'settings-top', n: 'Settings', l: 'Keys, cleanup, and engine' },
        { k: 'about', n: 'About', l: 'Version, license, and updates' },
      ],
      cur: 0, dark: false, mode: 0, live: false, seen: [],
      get key() { return this.list[this.cur].k + (this.dark ? '-dark' : '-light'); },
      name(i) { return this.list[i].k + (this.dark ? '-dark' : '-light'); },
      add(k) { if (!this.seen.includes(k)) this.seen.push(k); },
      init() {
        this.add(this.key);
        new IntersectionObserver(([e]) => { this.live = e.isIntersecting; }, { threshold: 0.2 }).observe(this.$el);
      },
      setMode(m) { this.mode = m; },
      pick(i) { this.cur = i; this.add(this.key); this.warm(i + 1); },
      warm(i) { this.add(this.name((i + this.list.length) % this.list.length)); },
      tourNext() { if (this.mode === 2 && this.live && !calm) this.pick((this.cur + 1) % this.list.length); },
      setDark(d) { this.dark = d; this.add(this.key); },
      alt(k) {
        const base = k.replace(/-(light|dark)$/, ''), s = this.list.find(x => x.k === base);
        return `The Hushpen ${s ? s.n : base} view, ${k.endsWith('-dark') ? 'Dark' : 'Light'} appearance`;
      },
    }));
  });
})();
