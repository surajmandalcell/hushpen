// Devices: the desktop Devices page next to a paired Android phone. Pair with a QR code, turn the routes on,
// then dictate: the audio goes to the phone, and when the phone is gone This computer takes over. It follows
// DESIGN.md (Devices) and mobile/SCREENS.md. Plays only while seen, and loops. With Reduce Motion it shows
// the settled page. Needs ph.js (window.PK) for the phone.
(() => {
  const PK = window.PK, FB = window.FlowBar;
  if (!PK || !FB) return;
  const calm = FB.calm;

  const L = {
    house: '<path d="M15 21v-8a1 1 0 0 0-1-1h-4a1 1 0 0 0-1 1v8"/><path d="M3 10a2 2 0 0 1 .709-1.528l7-6a2 2 0 0 1 2.582 0l7 6A2 2 0 0 1 21 10v9a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"/>',
    clock: PK.IC.clock,
    files: '<path d="M15 2h-4a2 2 0 0 0-2 2v11a2 2 0 0 0 2 2h8a2 2 0 0 0 2-2V8"/><path d="M16.706 2.706A2.4 2.4 0 0 0 15 2v5a1 1 0 0 0 1 1h5a2.4 2.4 0 0 0-.706-1.706z"/><path d="M5 7a2 2 0 0 0-2 2v11a2 2 0 0 0 2 2h8a2 2 0 0 0 1.732-1"/>',
    bookA: '<path d="M4 19.5v-15A2.5 2.5 0 0 1 6.5 2H19a1 1 0 0 1 1 1v18a1 1 0 0 1-1 1H6.5a1 1 0 0 1 0-5H20"/><path d="m8 13 4-7 4 7"/><path d="M9.1 11h5.7"/>',
    cpu: '<path d="M12 20v2M12 2v2M17 20v2M17 2v2M2 12h2M2 17h2M2 7h2M20 12h2M20 17h2M20 7h2M7 20v2M7 2v2"/><rect x="4" y="4" width="16" height="16" rx="2"/><rect x="8" y="8" width="8" height="8" rx="1"/>',
    phone: PK.IC.phoneS,
    gear: '<path d="M9.671 4.136a2.34 2.34 0 0 1 4.659 0 2.34 2.34 0 0 0 3.319 1.915 2.34 2.34 0 0 1 2.33 4.033 2.34 2.34 0 0 0 0 3.831 2.34 2.34 0 0 1-2.33 4.033 2.34 2.34 0 0 0-3.319 1.915 2.34 2.34 0 0 1-4.659 0 2.34 2.34 0 0 0-3.32-1.915 2.34 2.34 0 0 1-2.33-4.033 2.34 2.34 0 0 0 0-3.831A2.34 2.34 0 0 1 6.35 6.051a2.34 2.34 0 0 0 3.319-1.915"/><circle cx="12" cy="12" r="3"/>',
    info: '<circle cx="12" cy="12" r="10"/><path d="M12 16v-4M12 8h.01"/>',
    laptop: PK.IC.laptop, up: PK.IC.up, down: PK.IC.down, check: PK.IC.check,
  };
  const ico = k => `<svg viewBox="0 0 24 24" aria-hidden="true">${L[k]}</svg>`;
  const NAV = [['house', 'Home'], ['clock', 'History'], ['files', 'Manual'], ['bookA', 'Dictionary'], ['cpu', 'Models'], ['phone', 'Devices'], ['gear', 'Settings'], ['info', 'About']];

  // A pairing code: finder squares at three corners and a fixed scatter of cells, white as DESIGN.md says
  const qr = () => {
    const n = 21, cells = [];
    let s = 11;
    const r = () => (s = (s * 9301 + 49297) % 233280) / 233280;
    const finder = (x, y) => `<rect x="${x}" y="${y}" width="7" height="7" fill="#111"/><rect x="${x + 1}" y="${y + 1}" width="5" height="5" fill="#fff"/><rect x="${x + 2}" y="${y + 2}" width="3" height="3" fill="#111"/>`;
    for (let y = 0; y < n; y++) for (let x = 0; x < n; x++) {
      const nearFinder = (x < 8 && y < 8) || (x > n - 9 && y < 8) || (x < 8 && y > n - 9);
      if (!nearFinder && r() > .52) cells.push(`<rect x="${x}" y="${y}" width="1" height="1"/>`);
    }
    return `<svg viewBox="-1 -1 23 23" aria-hidden="true"><rect x="-1" y="-1" width="23" height="23" fill="#fff"/><g fill="#111">${cells.join('')}</g>${finder(0, 0)}${finder(n - 7, 0)}${finder(0, n - 7)}</svg>`;
  };

  // rw: a route row. A row that waits for its device collapses to nothing, so the list never jumps.
  const rw = (id, k, name, o = {}) => `<div class="dv-rw${o.hid ? ' hid' : ''}" data-d="${id}"><div class="dv-r">${ico(k)}<span class="dv-nm">${name}</span><span class="dv-ms"></span><span class="dv-sv">${o.fb ? 'Fallback' : ''}</span><i class="dv-mv off">${ico('up')}</i><i class="dv-mv off">${ico('down')}</i></div></div>`;
  const sw = (id, t, s) => `<div class="dv-r dv-r2"><div class="dv-t"><b>${t}</b><small>${s}</small></div><i class="dv-sw" data-sw="${id}"></i></div>`;

  const section = host => {
    const { beat } = PK.watch(host);
    host.querySelector('.dv-st').innerHTML = `
      <div class="dv-win" data-n="off"><i class="dv-tl"><b></b><b></b></i>
        <div class="dv-sb"><b class="dv-name">Hushpen</b><div class="dv-nav">${NAV.map(([k, n]) => `<span${n === 'Devices' ? ' class="on"' : ''}>${ico(k)}${n}</span>`).join('')}</div></div>
        <div class="dv-pane"><div class="dv-tb"><b>Devices</b></div>
          <div class="dv-body">
            <div class="dv-g">${sw('on', 'Use models on other devices', 'Phones and servers run speech and cleanup for this computer.')}${sw('local', 'Do not load models on this computer', 'If no device answers, the dictation stops with a notice.')}</div>
            <div class="dv-rest">
              <h5>Speech to Text</h5><div class="dv-g">${rw('s-p', 'phone', 'Pixel 10', { hid: 1 })}${rw('s-c', 'laptop', 'This computer', { fb: 1 })}</div>
              <h5>Text cleanup</h5><div class="dv-g">${rw('c-p', 'phone', 'Pixel 10', { hid: 1 })}${rw('c-c', 'laptop', 'This computer', { fb: 1 })}</div>
              <p class="dv-ft">Hushpen uses the first device in each list that answers.</p>
              <h5 class="dv-h">Your devices<i class="dv-btn" data-add>Add Device…</i></h5>
              <div class="dv-g"><div class="dv-rw hid" data-d="y-p"><div class="dv-r">${ico('phone')}<span class="dv-nm">Pixel 10</span><span class="dv-sv"></span><i class="dv-inf">${ico('info')}</i></div></div></div>
            </div>
          </div>
        </div>
        <div class="dv-scrim"></div>
        <div class="dv-sheet" data-s="qr"><b class="dv-sh">Add a Device</b>
          <div class="dv-seg"><i></i><span class="on">Nearby</span><span>SSH</span><span>Tailscale</span></div>
          <div class="dv-sbody">
            <div class="dv-s1"><div class="dv-qr">${qr()}</div><p>Scan this code with Hushpen on your phone. It pairs over your local network, encrypted with Noise.</p></div>
            <div class="dv-s2"><i>${ico('check')}</i><b>Paired with Pixel 10</b><p>It is now in your routes.</p></div>
          </div>
          <div class="dv-sbt"><i class="dv-btn dv-done">Cancel</i></div>
        </div>
        <svg class="dv-cur" viewBox="0 0 19 28" aria-hidden="true"><path d="M2 2v20.5l5-4.7 3.2 7.4 3.4-1.5-3.2-7.3H17z" fill="#000" stroke="#fff" stroke-width="1.5" stroke-linejoin="round"/></svg>
      </div>
      <div class="dv-wire"><i class="l"></i><b></b><b></b><b></b></div>
      <div class="dv-ph"><div class="dv-pw">${PK.androidHome('None yet', 'Share models with your computer')}</div><p class="ph-cap"><span class="dv-pn">Pixel 10</span><span class="dv-pt">Server mode</span></p></div>
      <div class="fbh dv-fb"><div data-fb data-hidden></div></div>`;

    const st = host.querySelector('.dv-st'), win = st.querySelector('.dv-win'), cur = st.querySelector('.dv-cur');
    const phone = st.querySelector('.dv-ph'), wire = st.querySelector('.dv-wire'), sheet = win.querySelector('.dv-sheet');
    const steps = [...host.querySelectorAll('.ln-steps > li')];
    const q = s => win.querySelector(s), d = id => win.querySelector(`[data-d="${id}"]`);
    const P = PK.androidHomeCtl(phone, beat);
    const fb = FB.create(st.querySelector('[data-fb]'), { scale: 1.25, seed: 9, hidden: true });
    const pt = phone.querySelector('.dv-pt');

    // The pointer works in the window's own 940 by 600 coordinates, whatever the page scale is
    const spot = el => {
      const w = win.getBoundingClientRect(), r = el.getBoundingClientRect(), k = w.width / win.offsetWidth;
      return [(r.left - w.left + r.width * .5) / k, (r.top - w.top + r.height * .5) / k];
    };
    const point = async (el, dx = 0, dy = 0) => {
      const [x, y] = spot(el);
      cur.style.transform = `translate(${(x + dx).toFixed(1)}px, ${(y + dy).toFixed(1)}px)`;
      cur.classList.add('on');
      await beat(780);
    };
    const press = async el => {
      el.classList.add('dn');
      await beat(130);
      el.classList.remove('dn');
    };
    const rest = () => { cur.classList.remove('on'); };
    const park = () => {
      cur.style.transition = 'none';
      cur.style.transform = '';
      void cur.offsetWidth;
      cur.style.transition = '';
    };

    const setSv = (id, text, cls = '', ms = '') => {
      const r = d(id);
      r.querySelector('.dv-sv').textContent = text;
      r.querySelector('.dv-sv').className = 'dv-sv ' + cls;
      const m = r.querySelector('.dv-ms');
      if (m) m.textContent = ms;
    };
    const busy = ids => win.querySelectorAll('.dv-rw').forEach(r => r.classList.toggle('busy', ids.includes(r.dataset.d)));
    const wireMode = m => { wire.dataset.m = m; };
    const ready = () => { setSv('s-p', 'Ready', 'ok', '38 ms'); setSv('c-p', 'Ready', 'ok', '41 ms'); setSv('y-p', 'Ready', 'ok'); };
    const gone = on => {
      phone.classList.toggle('gone', on);
      pt.textContent = on ? 'Out of range' : 'Server mode';
      wireMode(on ? 'off' : 'on');
      if (on) { setSv('s-p', 'Not reachable', 'bad'); setSv('c-p', 'Not reachable', 'bad'); setSv('y-p', 'Not reachable', 'bad'); }
    };
    const act = i => steps.forEach((li, j) => li.classList.toggle('is-on', j === i));
    const open = on => { win.classList.toggle('sheet', on); };

    const reset = () => {
      win.dataset.n = 'off';
      win.querySelectorAll('.dv-sw').forEach(s => s.classList.remove('on'));
      ['s-p', 'c-p', 'y-p'].forEach(id => { d(id).classList.add('hid'); setSv(id, ''); });
      ['s-c', 'c-c'].forEach(id => setSv(id, 'Fallback'));
      busy([]); open(false); sheet.dataset.s = 'qr'; q('.dv-done').textContent = 'Cancel';
      P.reset(); P.q('[data-desk]').textContent = 'None yet';
      phone.classList.remove('gone'); pt.textContent = 'Server mode'; wireMode('off');
      fb.hide(); rest(); park();
    };
    // The settled page: paired, routes on, the phone answering
    const final = () => {
      win.dataset.n = 'on';
      win.querySelector('[data-sw="on"]').classList.add('on');
      ['s-p', 'c-p', 'y-p'].forEach(id => d(id).classList.remove('hid'));
      ready(); busy(['s-p']);
      P.final(); P.q('[data-desk]').textContent = '1 paired';
      wireMode('on');
    };

    const pair = async () => {
      await beat(400);
      await P.touch(P.q('.pk-sw'));
      P.q('.pk-sw').classList.add('on');
      await beat(700);
      P.q('.pk-asrv').classList.add('on');
      await beat(500);
      await point(q('[data-add]'), 14, 8);
      await press(q('[data-add]'));
      open(true);
      rest();
      await beat(1500);
      wireMode('pair');
      await beat(1400);
      sheet.dataset.s = 'done';
      q('.dv-done').textContent = 'Done';
      P.q('[data-desk]').textContent = '1 paired';
      wireMode('on');
      await beat(1000);
      await point(q('.dv-done'), 10, 6);
      await press(q('.dv-done'));
      open(false);
      rest();
      await beat(450);
      ['s-p', 'c-p', 'y-p'].forEach(id => d(id).classList.remove('hid'));
      await beat(700);
    };
    const route = async () => {
      await point(q('[data-sw="on"]'), 4, 4);
      q('[data-sw="on"]').classList.add('dn');
      await beat(120);
      q('[data-sw="on"]').classList.remove('dn');
      q('[data-sw="on"]').classList.add('on');
      win.dataset.n = 'on';
      rest();
      await beat(500);
      setSv('s-p', 'Checking'); setSv('c-p', 'Checking'); setSv('y-p', 'Checking');
      await beat(800);
      setSv('s-p', 'Ready', 'ok', '38 ms');
      await beat(300);
      setSv('c-p', 'Ready', 'ok', '41 ms');
      await beat(300);
      setSv('y-p', 'Ready', 'ok');
      await beat(1200);
    };
    const dictate = async () => {
      const run = async (via) => {
        fb.set('listening').show();
        await beat(1900);
        fb.set('transcribing');
        busy([via === 'phone' ? 's-p' : 's-c']);
        if (via === 'phone') { P.q('.pk-sv').textContent = 'Working'; wireMode('go'); }
        await beat(1300);
        if (via === 'phone') wireMode('back');
        await beat(via === 'phone' ? 600 : 100);
        fb.set('cleaning');
        busy([via === 'phone' ? 'c-p' : 'c-c']);
        if (via === 'phone') wireMode('go');
        await beat(1000);
        if (via === 'phone') { wireMode('back'); await beat(600); wireMode('on'); P.q('.pk-sv').textContent = 'Serving'; }
        fb.set('pasted');
        busy([]);
        await beat(1200);
        fb.hide();
        await beat(500);
      };
      await run('phone');
      gone(true);
      await beat(1300);
      await run('computer');
      await beat(700);
    };

    if (calm) { reset(); final(); busy([]); fb.set('undo').show(); steps.forEach(li => li.classList.add('is-on')); return; }
    reset();
    (async () => {
      for (;;) {
        reset();
        void st.offsetWidth;
        st.classList.remove('rs');
        await beat(400);
        for (const [i, f] of [pair, route, dictate].entries()) { act(i); await f(); }
        act(-1);
        await beat(500);
        st.classList.add('rs');
        await beat(450);
      }
    })();
  };

  // The Devices tile: the Speech to Text route. The phone answers, then it is gone and This computer takes over.
  const tile = host => {
    const { beat } = PK.watch(host);
    host.innerHTML = `<div class="dv-mini" data-n="off"><div class="dv-g">${sw('on', 'Use models on other devices', '').replace('<small></small>', '')}</div>
      <div class="dv-rest"><h5>Speech to Text</h5><div class="dv-g">${rw('s-p', 'phone', 'Pixel 10')}${rw('s-c', 'laptop', 'This computer', { fb: 1 })}</div></div>
      <p class="dv-cap">Pixel 10 answers first.</p></div>`;
    const m = host.firstElementChild, cap = m.querySelector('.dv-cap'), swt = m.querySelector('.dv-sw');
    const d = id => m.querySelector(`[data-d="${id}"]`);
    const sv = (id, text, cls = '', ms = '') => {
      const r = d(id);
      r.querySelector('.dv-sv').textContent = text;
      r.querySelector('.dv-sv').className = 'dv-sv ' + cls;
      r.querySelector('.dv-ms').textContent = ms;
    };
    const busy = id => m.querySelectorAll('.dv-rw').forEach(r => r.classList.toggle('busy', r.dataset.d === id));
    const say = t => { cap.textContent = t; cap.classList.remove('swap'); void cap.offsetWidth; cap.classList.add('swap'); };
    const reset = () => { m.dataset.n = 'off'; swt.classList.remove('on'); sv('s-p', ''); sv('s-c', 'Fallback'); busy(''); cap.textContent = 'Pixel 10 answers first.'; };
    if (calm) { m.dataset.n = 'on'; swt.classList.add('on'); sv('s-p', 'Ready', 'ok', '38 ms'); sv('s-c', 'Fallback'); busy('s-p'); return; }
    reset();
    (async () => {
      for (;;) {
        await beat(900);
        swt.classList.add('on'); m.dataset.n = 'on';
        await beat(500);
        sv('s-p', 'Checking');
        await beat(700);
        sv('s-p', 'Ready', 'ok', '38 ms');
        await beat(600);
        busy('s-p'); say('Pixel 10 is transcribing.');
        await beat(2000);
        busy('');
        sv('s-p', 'Not reachable', 'bad');
        say('Pixel 10 is out of range.');
        await beat(1400);
        busy('s-c'); say('This computer takes over.');
        await beat(2200);
        reset();
        await beat(400);
      }
    })();
  };

  document.querySelectorAll('[data-dv]').forEach(section);
  document.querySelectorAll('[data-dvtile]').forEach(tile);
})();
