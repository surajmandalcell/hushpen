// Page chrome: the download toast, the nav pill, reveal on scroll, fitted scenes, the strips, copy
// buttons, and the menu bars. The FAQ and footer links are plain HTML in index.html.
(() => {
  const calm = matchMedia('(prefers-reduced-motion: reduce)').matches;
  document.documentElement.classList.toggle('calm', calm);
  const sleep = ms => new Promise(r => setTimeout(r, ms));
  const whenSeen = (el, fn) => { const io = new IntersectionObserver(es => { if (es.some(e => e.isIntersecting)) { io.disconnect(); fn(); } }, { threshold: 0.3 }); io.observe(el); };
  window.HP = { calm, sleep, whenSeen };

  const GH = 'https://github.com/surajmandalcell/hushpen';
  document.querySelectorAll('[data-gh]').forEach(a => { a.href = GH; });
  const LX = ['AppImage', 'deb', 'rpm', 'tar.gz'];
  document.querySelectorAll('[data-lx]').forEach(el => {
    el.innerHTML = ['x64', 'arm64'].map(a => `<div><b>${a}</b>${LX.map(f => `<a data-lx-kind="linux-${a}-${f === 'AppImage' ? 'appimage' : f === 'tar.gz' ? 'tar' : f}" href="${GH}/releases/latest">${f}</a>`).join('')}</div>`).join('');
  });
  window.HushpenReleases.subscribe(({ stable }) => {
    document.querySelectorAll('[data-lx-kind]').forEach(a => {
      const file = stable.assets[a.dataset.lxKind];
      a.href = file?.url || GH + '/releases/latest';
    });
    document.querySelectorAll('[data-notes]').forEach(a => { a.href = stable.notesUrl; });
  });

  const dtoast = document.querySelector('.dtoast');
  let timer = 0;
  document.querySelectorAll('[data-dl]').forEach(a => {
    a.addEventListener('click', () => {
      a.classList.remove('going'); void a.offsetWidth; a.classList.add('going');
      setTimeout(() => a.classList.remove('going'), 1200);
      if (document.documentElement.dataset.os === 'windows') return;
      if (dtoast) { dtoast.classList.add('show'); clearTimeout(timer); timer = setTimeout(() => dtoast.classList.remove('show'), 4800); }
    });
  });

  // Nav: a hairline once scrolled, and a pill under the section in view
  const nav = document.getElementById('nav'), links = [...nav.querySelectorAll('nav a')], ind = nav.querySelector('.ind');
  const secs = links.map(a => document.querySelector(a.getAttribute('href')));
  nav.querySelector('.brand').addEventListener('click', e => { e.preventDefault(); scrollTo({ top: 0, behavior: calm ? 'auto' : 'smooth' }); });
  let queued = false;
  const navTick = () => {
    queued = false;
    nav.classList.toggle('scrolled', scrollY > 8);
    let on = -1;
    secs.forEach((s, i) => { if (s && s.getBoundingClientRect().top < 140) on = i; });
    if (innerHeight + scrollY >= document.documentElement.scrollHeight - 4) on = links.length - 1;
    links.forEach((a, i) => a.classList.toggle('on', i === on));
    if (on >= 0) { const a = links[on]; ind.style.width = a.offsetWidth + 'px'; ind.style.transform = `translateX(${a.offsetLeft}px)`; ind.style.opacity = 1; }
    else ind.style.opacity = 0;
  };
  addEventListener('scroll', () => { if (!queued) { queued = true; requestAnimationFrame(navTick); } }, { passive: true });
  addEventListener('resize', navTick);
  navTick();

  // Scales a fixed-size scene to its parent's width. --fit (CSS, per breakpoint) or data-fit is the design width.
  const fits = [...document.querySelectorAll('[data-fit]')];
  const fit = () => fits.forEach(el => {
    const pw = el.parentElement.clientWidth, w = parseFloat(getComputedStyle(el).getPropertyValue('--fit')) || +el.dataset.fit;
    if (pw && w) el.style.zoom = Math.min(1, pw / w).toFixed(4);
  });
  addEventListener('resize', fit); fit();

  document.querySelectorAll('[data-reveal]').forEach(el => { const io = new IntersectionObserver(es => { if (es[0].isIntersecting) { el.classList.add('in'); io.disconnect(); } }, { threshold: 0.12 }); io.observe(el); });

  const COPY_SVG = '<svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linejoin="round"><rect x="5.5" y="5.5" width="8" height="8" rx="2"/><path d="M10.5 5.5V4a1.5 1.5 0 0 0-1.5-1.5H4A1.5 1.5 0 0 0 2.5 4v5A1.5 1.5 0 0 0 4 10.5h1.5"/></svg><svg class="ok" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M3.5 8.5 6.5 11.5 12.5 4.5"/></svg>';
  document.querySelectorAll('[data-copy]').forEach(b => {
    b.innerHTML = COPY_SVG;
    b.addEventListener('click', () => { navigator.clipboard?.writeText(b.dataset.copy).catch(() => {}); b.classList.add('done'); setTimeout(() => b.classList.remove('done'), 1500); });
  });

  // Strips. Each marquee row is doubled so the loop closes.
  const esc = s => s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/"/g, '&quot;');
  const CHIPS = [['Mail', 'Thanks, I will send it today.'], ['Slack', 'On my way, five minutes.'], ['Notes', 'Buy milk. Call the bank.'], ['VS Code', '// retry the sync twice'], ['Terminal', 'git commit -m "fix the sync"'], ['Messages', 'Running late, start without me.'], ['Chrome', 'best pizza near me'], ['Pages', 'Chapter two begins at dawn.']];
  document.querySelectorAll('[data-chips]').forEach(el => {
    const row = copy => CHIPS.map(([app, p], i) => `<span class="chip"${copy ? ' aria-hidden="true"' : ''}><b>${app}</b><span class="ph" style="--c:${p.length};--dl:${(i * 0.85).toFixed(2)}s">${esc(p)}</span></span>`).join('');
    el.innerHTML = `<div class="k-marq-in" style="--md:56s">${row(0)}${row(1)}</div>`;
  });
  // Parakeet v3's 25 languages, then some that Whisper Large v3 Turbo adds
  const LANGS = ['English', 'Deutsch', 'Français', 'Español', 'Italiano', 'Português', 'Nederlands', 'Polski', 'Čeština', 'Slovenčina', 'Slovenščina', 'Hrvatski', 'Български', 'Русский', 'Українська', 'Ελληνικά', 'Magyar', 'Română', 'Svenska', 'Dansk', 'Suomi', 'Eesti', 'Latviešu', 'Lietuvių', 'Malti', 'हिन्दी', 'தமிழ்', 'বাংলা', '日本語', '한국어', '中文', 'العربية'];
  document.querySelectorAll('[data-langs]').forEach(el => {
    const row = copy => LANGS.map(l => `<span class="lang"${copy ? ' aria-hidden="true"' : ''}>${l}</span>`).join('');
    el.innerHTML = `<div class="k-marq-in" style="--mg:34px;--md:70s">${row(0)}${row(1)}</div>`;
  });
  // The macOS defaults, from the shortcut registry
  const KEYS = [[['⌥', '⇥'], 'Hands-free'], [['Right ⌥', 'Right ⇧'], 'Command Mode'], [['⌃', '⌘', 'V'], 'Paste last'], [['⌃', '⌘', 'S'], 'Read aloud'], [['esc'], 'Cancel']];
  document.querySelectorAll('[data-keystrip]').forEach(el => {
    el.innerHTML = KEYS.map(([ks, l]) => `<span>${ks.map(k => k === '×2' ? '<i>×2</i>' : `<kbd>${k}</kbd>`).join('')}${l}</span>`).join('');
    if (calm) return;
    const items = [...el.children];
    let n = 0, live = false;
    new IntersectionObserver(([e]) => { live = e.isIntersecting; }).observe(el);
    setInterval(async () => {
      if (!live) return;
      const it = items[n++ % items.length], kb = [...it.querySelectorAll('kbd')];
      const twice = !!it.querySelector('i');
      for (let r = 0; r < (twice ? 2 : 1); r++) { kb.forEach(k => k.classList.add('down')); await sleep(twice ? 140 : 420); kb.forEach(k => k.classList.remove('down')); await sleep(140); }
    }, 1300);
  });

  // Menu bars. The Hushpen glyph sits left of the system items, as macOS orders them.
  const MB = {
    bat: '<svg class="mb-bat" viewBox="0 0 26 12" aria-hidden="true"><rect x=".5" y=".5" width="22" height="11" rx="3.5" fill="none" stroke="currentColor" stroke-opacity=".42"/><rect x="2" y="2" width="16" height="8" rx="2" fill="currentColor"/><path d="M24 4.2v3.6a1.9 1.9 0 0 0 0-3.6Z" fill="currentColor" fill-opacity=".42"/></svg>',
    wifi: '<svg class="mb-wifi" viewBox="0 0 17 12" aria-hidden="true"><g fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round"><path d="M1.3 4.4a10.2 10.2 0 0 1 14.4 0"/><path d="M3.9 7a6.5 6.5 0 0 1 9.2 0"/></g><path fill="currentColor" d="M8.5 11.6 6.4 9.5a3 3 0 0 1 4.2 0z"/></svg>',
    cc: '<svg class="mb-cc" viewBox="0 0 16 14" aria-hidden="true"><g fill="none" stroke="currentColor" stroke-width="1.25"><rect x=".8" y="1" width="14.4" height="5.2" rx="2.6"/><rect x=".8" y="7.8" width="14.4" height="5.2" rx="2.6"/></g><circle cx="12.6" cy="3.6" r="1.6" fill="currentColor"/><circle cx="3.4" cy="10.4" r="1.6" fill="currentColor"/></svg>',
  };
  const MENUS = { Finder: ['File', 'Edit', 'View', 'Go', 'Window', 'Help'], Mail: ['File', 'Edit', 'View', 'Mailbox', 'Message', 'Format', 'Window', 'Help'], Notes: ['File', 'Edit', 'Format', 'View', 'Window', 'Help'], Hushpen: ['File', 'Edit', 'View', 'Window', 'Help'] };
  document.querySelectorAll('[data-mbar]').forEach(el => {
    const app = el.dataset.app || 'Finder';
    el.classList.add('mb');
    el.setAttribute('aria-hidden', 'true');
    el.innerHTML = `<i class="mb-apple"></i><b>${app}</b>${MENUS[app].map(m => `<span>${m}</span>`).join('')}<i class="mb-sp"></i><span class="mb-st"><i class="mb-hp"></i></span>${MB.bat}${MB.wifi}${MB.cc}<span class="mb-clock">Mon Nov 2&nbsp; 9:41 AM</span>`;
  });

})();
