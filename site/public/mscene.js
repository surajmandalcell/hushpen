// Models scene: one story in four beats in the Hushpen window, drawn as the app draws it (DESIGN.md,
// Light) at 940 by 600 points on a 1184 by 720 desktop. Parakeet downloads (416 MB) and passes its
// check, dictation works, AI cleanup with S1-mini is turned on in Settings, and the same spoken words
// come out cleaner. The desktop zooms to fit the column; below a zoom of CAM it keeps that zoom, so the
// text stays readable, and pans to follow the pointer. The beats loop only while the scene is on
// screen; with Reduce Motion it shows one still, final frame. The step rail follows the beats.
(() => {
  const region = document.querySelector('.msn');
  if (!region || !window.FlowBar) return;
  const calm = matchMedia('(prefers-reduced-motion: reduce)').matches;
  const STOP = Symbol('stop');
  const DW = 1184, CAM = .62, WL = 122, WR = 1062;

  // Rule cleanup drops "um" and the repeated "the". S1-mini also drops "so basically" and rewrites "sorry".
  const RAW = ['um', 'so', 'basically', 'the', 'the', 'report', 'is', 'late,', 'sorry'];
  const RULE_X = [0, 4];
  const AI_X = [0, 1, 2, 4, 8];
  const RULES = ['So', 'basically', 'the', 'report', 'is', 'late,', 'sorry.'];
  const CLEAN = 'The report is late. Sorry about that.';
  const STEPS = [
    ['Download Parakeet', '416 MB, checked when it lands'],
    ['Dictate', 'Press Option + Tab and talk'],
    ['Turn on S1-mini', 'AI cleanup on your Mac'],
    ['Cleaner text', 'Same words, tidied up'],
  ];

  // The lucide icons the app ships (gpui-kit assets)
  const P = {
    house: '<path d="M15 21v-8a1 1 0 0 0-1-1h-4a1 1 0 0 0-1 1v8"/><path d="M3 10a2 2 0 0 1 .709-1.528l7-6a2 2 0 0 1 2.582 0l7 6A2 2 0 0 1 21 10v9a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"/>',
    clock: '<circle cx="12" cy="12" r="10"/><path d="M12 6v6l4 2"/>',
    files: '<path d="M15 2h-4a2 2 0 0 0-2 2v11a2 2 0 0 0 2 2h8a2 2 0 0 0 2-2V8"/><path d="M16.706 2.706A2.4 2.4 0 0 0 15 2v5a1 1 0 0 0 1 1h5a2.4 2.4 0 0 0-.706-1.706z"/><path d="M5 7a2 2 0 0 0-2 2v11a2 2 0 0 0 2 2h8a2 2 0 0 0 1.732-1"/>',
    bookA: '<path d="M4 19.5v-15A2.5 2.5 0 0 1 6.5 2H19a1 1 0 0 1 1 1v18a1 1 0 0 1-1 1H6.5a1 1 0 0 1 0-5H20"/><path d="m8 13 4-7 4 7"/><path d="M9.1 11h5.7"/>',
    cpu: '<path d="M12 20v2M12 2v2M17 20v2M17 2v2M2 12h2M2 17h2M2 7h2M20 12h2M20 17h2M20 7h2M7 20v2M7 2v2"/><rect x="4" y="4" width="16" height="16" rx="2"/><rect x="8" y="8" width="8" height="8" rx="1"/>',
    settings: '<path d="M9.671 4.136a2.34 2.34 0 0 1 4.659 0 2.34 2.34 0 0 0 3.319 1.915 2.34 2.34 0 0 1 2.33 4.033 2.34 2.34 0 0 0 0 3.831 2.34 2.34 0 0 1-2.33 4.033 2.34 2.34 0 0 0-3.319 1.915 2.34 2.34 0 0 1-4.659 0 2.34 2.34 0 0 0-3.32-1.915 2.34 2.34 0 0 1-2.33-4.033 2.34 2.34 0 0 0 0-3.831A2.34 2.34 0 0 1 6.35 6.051a2.34 2.34 0 0 0 3.319-1.915"/><circle cx="12" cy="12" r="3"/>',
    info: '<circle cx="12" cy="12" r="10"/><path d="M12 16v-4M12 8h.01"/>',
    audio: '<path d="M2 10v3M6 6v11M10 3v18M14 8v7M18 5v13M22 10v3"/>',
    globe: '<circle cx="12" cy="12" r="10"/><path d="M12 2a14.5 14.5 0 0 0 0 20 14.5 14.5 0 0 0 0-20M2 12h20"/>',
    mic: '<path d="M12 19v3M19 10v2a7 7 0 0 1-14 0v-2"/><rect x="9" y="2" width="6" height="13" rx="3"/>',
    search: '<path d="m21 21-4.34-4.34"/><circle cx="11" cy="11" r="8"/>',
    chev: '<path d="m6 9 6 6 6-6"/>',
  };
  const ic = (k, c) => `<svg${c ? ` class="${c}"` : ''} viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">${P[k]}</svg>`;
  const CURSOR = '<svg class="msa-cur" viewBox="0 0 19 28" aria-hidden="true"><path d="M2 2v20.5l5-4.7 3.2 7.4 3.4-1.5-3.2-7.3H17z" fill="#000" stroke="#fff" stroke-width="1.5" stroke-linejoin="round"/></svg>';
  // The macOS menu bar, as site.js draws it
  const MB = {
    bat: '<svg class="mb-bat" viewBox="0 0 26 12" aria-hidden="true"><rect x=".5" y=".5" width="22" height="11" rx="3.5" fill="none" stroke="currentColor" stroke-opacity=".42"/><rect x="2" y="2" width="16" height="8" rx="2" fill="currentColor"/><path d="M24 4.2v3.6a1.9 1.9 0 0 0 0-3.6Z" fill="currentColor" fill-opacity=".42"/></svg>',
    wifi: '<svg class="mb-wifi" viewBox="0 0 17 12" aria-hidden="true"><g fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round"><path d="M1.3 4.4a10.2 10.2 0 0 1 14.4 0"/><path d="M3.9 7a6.5 6.5 0 0 1 9.2 0"/></g><path fill="currentColor" d="M8.5 11.6 6.4 9.5a3 3 0 0 1 4.2 0z"/></svg>',
    cc: '<svg class="mb-cc" viewBox="0 0 16 14" aria-hidden="true"><g fill="none" stroke="currentColor" stroke-width="1.25"><rect x=".8" y="1" width="14.4" height="5.2" rx="2.6"/><rect x=".8" y="7.8" width="14.4" height="5.2" rx="2.6"/></g><circle cx="12.6" cy="3.6" r="1.6" fill="currentColor"/><circle cx="3.4" cy="10.4" r="1.6" fill="currentColor"/></svg>',
  };
  const mbar = (app, menus) => `<div class="mb" aria-hidden="true"><i class="mb-apple"></i><b>${app}</b>${menus.map(m => `<span>${m}</span>`).join('')}<i class="mb-sp"></i><span class="mb-st"><i class="mb-hp"></i></span>${MB.bat}${MB.wifi}${MB.cc}<span class="mb-clock">Mon Nov 2&nbsp; 9:41 AM</span></div>`;

  // A wait that throws STOP once the loop is stopped, so a beat ends at its next wait
  function token() {
    const t = { alive: true, pend: new Set() };
    t.wait = ms => new Promise((res, rej) => {
      if (!t.alive) { rej(STOP); return; }
      const p = { rej, h: setTimeout(() => { t.pend.delete(p); if (t.alive) res(); else rej(STOP); }, ms) };
      t.pend.add(p);
    });
    t.kill = () => { t.alive = false; t.pend.forEach(p => { clearTimeout(p.h); p.rej(STOP); }); t.pend.clear(); };
    return t;
  }
  async function say(el, list, w, ms) {
    el.classList.remove('done');
    el.innerHTML = '';
    for (let i = 0; i < list.length; i++) { el.insertAdjacentHTML('beforeend', `${i ? ' ' : ''}<span class="w">${list[i]}</span>`); await w(ms); }
  }
  const strike = (el, idx) => { const ws = el.querySelectorAll('.w'); idx.forEach(i => ws[i] && ws[i].classList.add('x')); };
  const land = (el, inner) => { el.classList.remove('done'); el.innerHTML = inner; void el.offsetWidth; el.classList.add('done'); };
  const smooth = p => p * p * (3 - 2 * p);
  async function ramp(w, ms, n, fn) { for (let k = 1; k <= n; k++) { await w(ms / n); fn(k / n); } }
  // Applies a state change with no transitions, so a reset lands at once
  const instant = (el, fn) => { el.classList.add('msn-nt'); fn(); void el.offsetWidth; el.classList.remove('msn-nt'); };

  // Rows as the app's shared row builders draw them
  const model = (cls, name, rec, desc, size) => `<div class="msa-r${cls ? ` ${cls}` : ''}"><div class="msa-t"><b>${name}${rec ? '<em>Recommended</em>' : ''}</b><small>${desc}</small></div><span class="msa-sz">${size}</span><span class="msa-st">Not downloaded</span>${ic('info', 'msa-i')}<span class="msa-bc"><i class="msa-btn">Download</i></span></div>`;
  const row = (title, desc, ctl) => `<div class="msa-r"><div class="msa-t"><b>${title}</b><small>${desc}</small></div>${ctl}</div>`;
  const pop = v => `<span class="msa-pop">${v}${ic('chev')}</span>`;
  const sw = (on, c) => `<i class="msa-sw${on ? ' on' : ''}${c ? ` ${c}` : ''}"></i>`;
  const sec = (h, rows, ft) => `<h5>${h}</h5><div class="msa-g">${rows}</div>${ft ? `<p class="msa-ft">${ft}</p>` : ''}`;
  const NAV = [['house', 'Home'], ['clock', 'History'], ['files', 'Manual'], ['bookA', 'Dictionary'], ['cpu', 'Models'], ['settings', 'Settings'], ['info', 'About']];

  region.innerHTML = `<div class="msa" role="img" aria-label="The Hushpen window. Parakeet downloads and passes its check, dictation types the words, AI cleanup with S1-mini is turned on in Settings, and the same words come out cleaner."><div class="msa-desk">${mbar('Hushpen', ['File', 'Edit', 'View', 'Window', 'Help'])}
    <div class="msa-win"><i class="msa-tl"><b></b><b></b></i>
      <div class="msa-sb"><b class="msa-name">Hushpen</b><div class="msa-nav">${NAV.map(([k, n]) => `<span data-n="${n.toLowerCase()}">${ic(k)}${n}</span>`).join('')}</div></div>
      <div class="msa-pane">
        <div class="msa-tb"><b class="msa-ti">Models</b><span class="msa-q">${ic('search')}Search</span></div>
        <div class="msa-v" data-v="models">${sec('Speech recognition',
          model('msa-pk', 'Parakeet v3 Q4', 1, 'Accurate dictation in English and 24 other European languages. No translation.', '416 MB') +
          model('', 'Whisper Large v3 Turbo Q5', 0, 'For Hindi and other languages outside Parakeet&rsquo;s 25. No translation.', '574 MB') +
          model('', 'Whisper Small Q5', 0, 'Transcribes many languages and translates speech to English.', '190 MB') +
          model('', 'Whisper Base Q5', 0, 'Fast and small, for older or slower computers.', '60 MB'),
          'Parakeet is recommended. Use Whisper for other languages or translation.')}${sec('Text cleanup',
          model('', 'S1-mini by Superwhisper', 1, 'Fast cleanup of English dictation. It cannot do Command Mode.', '484 MB') +
          model('', 'Qwen3.5 2B', 0, 'Cleanup in many languages, and edits for Command Mode.', '1.3 GB'),
          'Fixes grammar and punctuation when AI cleanup is on. Choose the model in Settings.')}${sec('Voices',
          model('', 'Kokoro 82M', 1, 'Natural English voices for Read aloud, made on this computer.', '148 MB') +
          model('', 'Chatterbox Turbo', 0, 'Reads English aloud in a voice it copies from a short clip.', '726 MB') +
          model('', 'Chatterbox Nano', 0, 'A smaller Chatterbox: copies a voice and reads English aloud.', '496 MB'))}</div>
        <div class="msa-v" data-v="home"><div class="msa-g">
          <div class="msa-r">${ic('audio')}<span class="msa-t"><b>Dictation</b></span><span class="msa-ds">Ready</span><i class="msa-btn msa-rec">Record</i></div>
          <div class="msa-tr"><p class="msa-ph">Your words appear here and are copied to the clipboard.</p><p class="msa-tx"></p></div>
          <div class="msa-r">${ic('globe')}<div class="msa-t"><b>Language</b><small>Detects the language you speak</small></div>${pop('Auto')}</div>
          </div><div class="msa-g"><div class="msa-r">${ic('mic')}<div class="msa-t"><b>Microphone</b><small>Dictation listens on this microphone</small></div>${pop('Default (Built-in Microphone)')}<i class="msa-btn">Test microphone</i></div></div></div>
        <div class="msa-v" data-v="settings">${sec('Cleanup',
          row('Clean up text', 'Fillers and stutters are removed before the text is pasted.', sw(1)) +
          row('Spoken punctuation', 'Saying "comma" or "period" writes the mark.', sw(0)) +
          row('AI cleanup', 'A language model fixes self-corrections.', sw(0, 'msa-ai')) +
          row('AI source', 'Where the model runs.', pop('This computer')) +
          row('AI model', '484 MB download. Chosen for this computer.', pop('S1-mini by Superwhisper')) +
          `<div class="msa-r msa-n">${ic('info')}<span class="msa-t msa-nt"></span></div>` +
          row('Profile', 'How the cleaned text reads.', pop('General')) +
          row('Prompt', 'S1-mini: Casual, lists, general.', '<i class="msa-btn">View&hellip;</i>'),
          'AI cleanup stays off until you turn it on. An endpoint is a server you choose, such as Ollama.')}</div>
      </div>
    </div>${CURSOR}</div><div class="fbh msa-fb"><div></div></div></div>`;

  const $ = s => region.querySelector(s);
  const stage = $('.msa'), desk = $('.msa-desk'), ti = $('.msa-ti'), q = $('.msa-q'), cur = $('.msa-cur');
  const pkSt = $('.msa-pk .msa-st'), pkBtn = $('.msa-pk .msa-btn'), ai = $('.msa-ai'), nt = $('.msa-nt');
  const ds = $('.msa-ds'), rec = $('.msa-rec'), tr = $('.msa-tr'), tx = $('.msa-tx');
  const views = [...region.querySelectorAll('.msa-v')], navs = {};
  region.querySelectorAll('.msa-nav span').forEach(s => { navs[s.dataset.n] = s; });
  const fb = FlowBar.create($('.msa-fb > div'), { hidden: true, state: 'listening', scale: 1.5, seed: 11 });
  const TITLES = { models: 'Models', home: 'Home', settings: 'Settings' };

  // The pointer and the camera work in the desk's own points, so undo the fit zoom when measuring
  const at = (el, fx = .5) => {
    const d = desk.getBoundingClientRect(), r = el.getBoundingClientRect(), k = d.width / desk.offsetWidth || 1;
    return [(r.left + r.width * fx - d.left) / k, (r.top + r.height / 2 - d.top) / k];
  };
  let vw = DW, focus = pkBtn;
  function cam(el) {
    focus = el;
    if (vw >= DW - 1) { desk.style.transform = ''; return; }
    const c = (DW - vw) / 2, lo = Math.min(WL - 16, c), hi = Math.max(WR + 16 - vw, c);
    const pan = Math.min(hi, Math.max(lo, at(el)[0] - vw / 2));
    desk.style.transform = `translateX(${(-pan).toFixed(1)}px)`;
  }
  function fit() {
    const pw = region.clientWidth;
    if (!pw) return;
    const z = Math.min(1, Math.max(pw / DW, CAM));
    vw = Math.min(DW, pw / z);
    stage.style.zoom = z.toFixed(4);
    stage.style.width = vw.toFixed(1) + 'px';
    instant(desk, () => cam(focus));
  }

  function view(n) {
    views.forEach(v => v.classList.toggle('on', v.dataset.v === n));
    Object.entries(navs).forEach(([k, s]) => s.classList.toggle('on', k === n));
    ti.textContent = TITLES[n];
    q.hidden = n === 'home';
  }
  // The status column of a Models row: the word, the percent while it downloads, then Active
  function pk(s, p = 0) {
    pkSt.className = 'msa-st' + (s === 'dl' ? ' dl' : s === 'ok' ? ' ok' : '');
    pkSt.textContent = { none: 'Not downloaded', dl: `${Math.round(p * 100)}%`, check: 'Checking', ok: 'Active' }[s];
    pkBtn.textContent = { none: 'Download', dl: 'Cancel', ok: 'Delete' }[s] || '';
    pkBtn.hidden = s === 'check';
  }
  const s1 = (s, p = 0) => { nt.textContent = { none: 'Downloads when you turn AI cleanup on.', dl: `Downloading ${Math.round(p * 100)}%`, check: 'Checking the download', ok: 'Ready' }[s]; };
  function talk(on) {
    ds.textContent = on ? 'Listening' : 'Ready';
    ds.classList.toggle('on', on);
    rec.textContent = on ? 'Stop' : 'Record';
  }
  // The translate property, not transform: the press scale then shrinks the pointer at its tip and leaves its place alone.
  // A longer way takes longer, so the pointer glides across the window instead of flicking.
  let pt = [0, 0];
  const place = (x, y, ms) => { pt = [x, y]; if (ms) cur.style.transitionDuration = `${ms}ms, .1s`; cur.style.translate = `${(x - 3).toFixed(1)}px ${(y - 3).toFixed(1)}px`; };
  async function moveTo(el, w, fx) {
    cam(el);
    const to = at(el, fx), ms = Math.round(Math.min(700, Math.max(420, 200 + .6 * Math.hypot(to[0] - pt[0], to[1] - pt[1]))));
    place(...to, ms);
    await w(ms);
  }
  async function click(el, w) { cur.classList.add('dn'); el.classList.add('press'); await w(130); cur.classList.remove('dn'); el.classList.remove('press'); }
  async function go(n, w, then) { await moveTo(navs[n], w, .25); await click(navs[n], w); view(n); if (then) cam(then); }

  function reset(anim) {
    const body = () => {
      view('models'); pk('none'); s1('none');
      ai.classList.remove('on'); talk(false);
      tr.classList.remove('has'); tx.className = 'msa-tx'; tx.innerHTML = '';
      fb.hide();
      cam(pkBtn);
    };
    if (anim) body();
    else { instant(desk, body); const [x, y] = at(pkBtn); place(x - 120, y + 150); }
  }
  function still() {
    view('home'); pk('ok'); s1('ok'); ai.classList.add('on');
    tr.classList.add('has'); tx.className = 'msa-tx'; tx.textContent = CLEAN;
    fb.set('copied').show(); cur.style.display = 'none';
    instant(desk, () => cam(tr));
  }
  // Each beat lasts about its ms, so the rail fills in step with it. Copied stays up for a second.
  const beats = [
    async w => {
      await moveTo(pkBtn, w);
      await click(pkBtn, w);
      pk('dl', 0);
      await ramp(w, 1000, 20, p => pk('dl', smooth(p)));
      pk('check'); await w(360);
      pk('ok');
    },
    async w => {
      await go('home', w, tr);
      fb.set('listening').show(); talk(true); await w(160);
      tr.classList.add('has');
      await say(tx, RAW, w, 80);
      fb.set('transcribing'); talk(false); strike(tx, RULE_X); await w(420);
      land(tx, RULES.join(' ')); fb.set('copied');
      await w(1100);
    },
    async w => {
      fb.hide();
      await go('settings', w);
      await moveTo(ai, w, .4); await click(ai, w);
      ai.classList.add('on');
      s1('dl', 0);
      await ramp(w, 800, 16, p => s1('dl', smooth(p)));
      s1('check'); await w(320);
      s1('ok');
    },
    async w => {
      await go('home', w, tr);
      fb.set('listening').show(); talk(true); await w(140);
      await say(tx, RAW, w, 70);
      fb.set('cleaning'); talk(false); strike(tx, AI_X); await w(640);
      land(tx, CLEAN); fb.set('copied');
      await w(1300); fb.hide(); await w(300);
    },
  ];
  const MS = [2000, 3000, 2300, 3600];

  const rail = document.createElement('ol');
  rail.className = 'msn-rail';
  rail.innerHTML = STEPS.map(([t, s], i) => `<li><i><b></b></i><strong><span>${i + 1}</span>${t}</strong><small>${s}</small></li>`).join('');
  region.append(rail);
  const lis = [...rail.children];
  const step = (i, ms = 0) => lis.forEach((li, j) => {
    li.classList.toggle('done', j < i);
    li.classList.toggle('on', j === i);
    li.style.setProperty('--d', ms + 'ms');
  });

  addEventListener('resize', fit);
  fit();
  if (calm) { reset(false); still(); step(4); return; }

  async function loop(t) {
    let first = true;
    try {
      for (;;) {
        step(-1);
        reset(!first);
        first = false;
        void rail.offsetWidth;
        for (let i = 0; i < beats.length; i++) {
          const t0 = performance.now();
          step(i, MS[i]);
          await beats[i](t.wait);
          const left = MS[i] - (performance.now() - t0);
          if (left > 0) await t.wait(left);
        }
      }
    } catch (e) { if (e !== STOP) throw e; }
  }
  let live = false, tok = null;
  function sync() {
    const want = live && !document.hidden;
    if (want === !!tok) return;
    if (tok) { tok.kill(); tok = null; }
    if (want) { tok = token(); loop(tok); }
  }
  reset(false);
  new IntersectionObserver(([e]) => { live = e.isIntersecting; sync(); }, { threshold: 0.2 }).observe(region);
  document.addEventListener('visibilitychange', sync);
})();
