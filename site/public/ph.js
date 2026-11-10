// Phones: the Android and iPhone scenes, the keyboards tile, and the pieces the Devices scene (dv.js)
// reuses. A phone is drawn at its own dp size and scaled with --pzv. Each scene plays only while seen and
// loops. With Reduce Motion it shows its last frame. The apps follow the real screens (mobile/SCREENS.md):
// the Android keyboard has Gboard keys with the voice bar above them, and the iPhone keyboard starts a flow
// session in the app and shows it in the Dynamic Island.
(() => {
  const FB = window.FlowBar;
  if (!FB) return;
  const calm = FB.calm;
  const sleep = ms => new Promise(r => setTimeout(r, ms));
  const TYPED = 'Friday works.', HEARD = " Let's do 2 pm, and I'll send the notes before.", WORDS = TYPED + HEARD;

  // Lucide icons, 24 px strokes
  const IC = {
    back: '<path d="m15 18-6-6 6-6"/>', more: '<circle cx="12" cy="12" r="1"/><circle cx="12" cy="5" r="1"/><circle cx="12" cy="19" r="1"/>',
    phone: '<path d="M22 16.92v3a2 2 0 0 1-2.18 2 19.79 19.79 0 0 1-8.63-3.07 19.5 19.5 0 0 1-6-6 19.79 19.79 0 0 1-3.07-8.67A2 2 0 0 1 4.11 2h3a2 2 0 0 1 2 1.72 12.84 12.84 0 0 0 .7 2.81 2 2 0 0 1-.45 2.11L8.09 9.91a16 16 0 0 0 6 6l1.27-1.27a2 2 0 0 1 2.11-.45 12.84 12.84 0 0 0 2.81.7A2 2 0 0 1 22 16.92z"/>',
    plus: '<path d="M5 12h14M12 5v14"/>', plusc: '<circle cx="12" cy="12" r="10"/><path d="M8 12h8M12 8v8"/>',
    smile: '<circle cx="12" cy="12" r="10"/><path d="M8 14s1.5 2 4 2 4-2 4-2M9 9h.01M15 9h.01"/>',
    image: '<rect width="18" height="18" x="3" y="3" rx="2"/><circle cx="9" cy="9" r="2"/><path d="m21 15-3.086-3.086a2 2 0 0 0-2.828 0L6 21"/>',
    send: '<path d="m5 12 7-7 7 7M12 19V5"/>', video: '<path d="m16 13 5.223 3.482a.5.5 0 0 0 .777-.416V7.87a.5.5 0 0 0-.752-.432L16 10.5"/><rect x="2" y="6" width="14" height="12" rx="2"/>',
    mic: '<path d="M12 2a3 3 0 0 0-3 3v7a3 3 0 0 0 6 0V5a3 3 0 0 0-3-3Z"/><path d="M19 10v2a7 7 0 0 1-14 0v-2M12 19v3"/>',
    check: '<path d="M20 6 9 17l-5-5"/>', x: '<path d="M18 6 6 18M6 6l12 12"/>',
    undo: '<path d="M9 14 4 9l5-5"/><path d="M4 9h10.5a5.5 5.5 0 0 1 5.5 5.5a5.5 5.5 0 0 1-5.5 5.5H11"/>',
    gear: '<path d="M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.08a2 2 0 0 1-1-1.74v-.5a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2z"/><circle cx="12" cy="12" r="3"/>',
    kbd: '<path d="M10 8h.01M12 12h.01M14 8h.01M16 12h.01M18 8h.01M6 8h.01M7 16h10M8 12h.01"/><rect width="20" height="16" x="2" y="4" rx="2"/>',
    shift: '<path d="M9 18v-6H5l7-7 7 7h-4v6H9z"/>', bksp: '<path d="M20 5H9l-7 7 7 7h11a2 2 0 0 0 2-2V7a2 2 0 0 0-2-2Z"/><path d="m12 9 6 6M18 9l-6 6"/>',
    enter: '<path d="M20 4v7a4 4 0 0 1-4 4H4"/><path d="m9 10-5 5 5 5"/>',
    globe: '<circle cx="12" cy="12" r="10"/><path d="M12 2a14.5 14.5 0 0 0 0 20 14.5 14.5 0 0 0 0-20M2 12h20"/>',
    user: '<path d="M19 21v-2a4 4 0 0 0-4-4H9a4 4 0 0 0-4 4v2"/><circle cx="12" cy="7" r="4"/>',
    radio: '<path d="M4.9 19.1C1 15.2 1 8.8 4.9 4.9"/><path d="M7.8 16.2c-2.3-2.3-2.3-6.1 0-8.5"/><circle cx="12" cy="12" r="2"/><path d="M16.2 7.8c2.3 2.3 2.3 6.1 0 8.5"/><path d="M19.1 4.9C23 8.8 23 15.1 19.1 19"/>',
    lines: '<path d="M2 10v3M6 6v11M10 3v18M14 8v7M18 5v13M22 10v3"/>', checkc: '<circle cx="12" cy="12" r="10"/><path d="m9 12 2 2 4-4"/>',
    bolt: '<path d="M4 14a1 1 0 0 1-.78-1.63l9.9-10.2a.5.5 0 0 1 .86.46l-1.92 6.02A1 1 0 0 0 13 10h7a1 1 0 0 1 .78 1.63l-9.9 10.2a.5.5 0 0 1-.86-.46l1.92-6.02A1 1 0 0 0 11 14z"/>',
    laptop: '<path d="M20 16V7a2 2 0 0 0-2-2H6a2 2 0 0 0-2 2v9m16 0H4m16 0 1.28 2.55a1 1 0 0 1-.9 1.45H3.62a1 1 0 0 1-.9-1.45L4 16"/>',
    server: '<rect width="20" height="8" x="2" y="2" rx="2" ry="2"/><rect width="20" height="8" x="2" y="14" rx="2" ry="2"/><path d="M6 6h.01M6 18h.01"/>',
    phoneS: '<rect width="14" height="20" x="5" y="2" rx="2" ry="2"/><path d="M12 18h.01"/>',
    qr: '<rect width="5" height="5" x="3" y="3" rx="1"/><rect width="5" height="5" x="16" y="3" rx="1"/><rect width="5" height="5" x="3" y="16" rx="1"/><path d="M21 16h-3a2 2 0 0 0-2 2v3M21 21v.01M12 7v3a2 2 0 0 1-2 2H7M3 12h.01M12 3h.01M12 16v.01M16 12h1M21 12v.01M12 21v-1"/>',
    link: '<path d="M10 13a5 5 0 0 0 7.54.54l3-3a5 5 0 0 0-7.07-7.07l-1.72 1.71"/><path d="M14 11a5 5 0 0 0-7.54-.54l-3 3a5 5 0 0 0 7.07 7.07l1.71-1.71"/>',
    box: '<path d="M21 8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73l7 4a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16Z"/><path d="m3.3 7 8.7 5 8.7-5M12 22V12"/>',
    up: '<path d="m18 15-6-6-6 6"/>', down: '<path d="m6 9 6 6 6-6"/>', right: '<path d="m9 18 6-6-6-6"/>',
    clock: '<circle cx="12" cy="12" r="10"/><path d="M12 6v6l4 2"/>',
  };
  const ic = (k, s = 20) => `<svg class="pk-i" width="${s}" height="${s}" viewBox="0 0 24 24" aria-hidden="true">${IC[k]}</svg>`;
  const SB = {
    sig: '<svg width="17" height="11" viewBox="0 0 17 11" aria-hidden="true"><path d="M1.5 9.8V7.6M5.5 9.8V5.4M9.5 9.8V3.2M13.5 9.8V1" stroke="currentColor" stroke-width="2.4" stroke-linecap="round" fill="none"/></svg>',
    wifi: '<svg width="16" height="12" viewBox="0 0 17 12" aria-hidden="true"><g fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round"><path d="M1.3 4.4a10.2 10.2 0 0 1 14.4 0"/><path d="M3.9 7a6.5 6.5 0 0 1 9.2 0"/></g><path fill="currentColor" d="M8.5 11.6 6.4 9.5a3 3 0 0 1 4.2 0z"/></svg>',
    bat: '<svg width="26" height="12" viewBox="0 0 26 12" aria-hidden="true"><rect x=".5" y=".5" width="22" height="11" rx="3.5" fill="none" stroke="currentColor" stroke-opacity=".42"/><rect x="2" y="2" width="16" height="8" rx="2" fill="currentColor"/><path d="M24 4.2v3.6a1.9 1.9 0 0 0 0-3.6Z" fill="currentColor" fill-opacity=".42"/></svg>',
  };
  const rnd = (() => { let s = 7; return () => (s = (s * 9301 + 49297) % 233280) / 233280; })();
  const wave = (n, style = '') => `<span class="pk-wv" aria-hidden="true"${style ? ` style="${style}"` : ''}>${Array.from({ length: n }, (_, i) => {
    const d = Math.abs(i - (n - 1) / 2) / ((n - 1) / 2), h = (.35 + .65 * Math.exp(-(d * d) * 1.6)) * (.55 + .45 * rnd());
    return `<i style="--n:${i};--r:${rnd().toFixed(2)};--hh:${h.toFixed(2)}"></i>`;
  }).join('')}</span>`;
  const mmss = s => `${Math.floor(s / 60)}:${String(s % 60).padStart(2, '0')}`;

  // The device: Android with a punch-hole camera, or iPhone with the Dynamic Island
  const frame = (os, inner) => `<div class="pk-w pk-${os}"><div class="pk-dev"><div class="pk-scr">${inner}<i class="pk-tap"></i></div></div></div>`;
  const sbA = '<div class="pk-sb"><span>12:30</span><i class="pk-cam"></i><span class="r">' + SB.wifi + SB.sig + SB.bat + '</span></div>';
  const sbI = '<div class="pk-sb"><span>9:41</span><span class="r">' + SB.sig + SB.wifi + SB.bat + '</span></div>' + `<i class="pk-isl">${wave(9, '--h:14px;--w:2.5px;--g:2px')}<b data-t>0:00</b></i>`;

  const bubbles = '<span class="pk-day">Today · 12:24</span><div class="pk-bb in">Are we still on for Thursday?</div><div class="pk-bb out">Yes, see you at 2.</div><div class="pk-bb in">Can we move the sync to Friday?</div>';
  const field = (ph, inner) => `<div class="pk-fd"><span class="pk-ph">${ph}</span><span class="pk-ft"><span class="pk-tx"></span><i class="pk-caret"></i></span>${inner}</div>`;

  // Android keyboard: Gboard keys, the voice bar above them
  const akey = (l, c = '', f = 1, sup = '') => `<span class="pk-k ${c}" style="flex-grow:${f}" data-k="${l}">${l}${sup ? `<sup>${sup}</sup>` : ''}</span>`;
  const aRows = () => {
    const r = ['qwertyuiop', 'asdfghjkl', 'zxcvbnm'];
    return `<div class="pk-row">${[...r[0]].map((c, i) => akey(c, '', 1, (i + 1) % 10)).join('')}</div>
      <div class="pk-row"><span class="pk-k gap"></span>${[...r[1]].map(c => akey(c)).join('')}<span class="pk-k gap"></span></div>
      <div class="pk-row"><span class="pk-k fn" style="flex-grow:1.5">${ic('shift', 22)}</span>${[...r[2]].map(c => akey(c)).join('')}<span class="pk-k fn" style="flex-grow:1.5">${ic('bksp', 22)}</span></div>
      <div class="pk-row"><span class="pk-k fn" style="flex-grow:1.45">?123</span>${akey(',', 'fn')}<span class="pk-k sp" style="flex-grow:5.1" data-k=" "></span>${akey('.', 'fn')}<span class="pk-k ent" style="flex-grow:1.45">${ic('enter', 22)}</span></div>`;
  };
  const androidKb = () => frame('a', `${sbA}<div class="pk-am"><div class="pk-atb"><span class="pk-ib">${ic('back', 22)}</span><span class="pk-av">P</span><b>Priya</b><span class="pk-sp"></span><span class="pk-ib">${ic('phone', 20)}</span><span class="pk-ib">${ic('more', 20)}</span></div>
    <div class="pk-th">${bubbles}</div>
    <div class="pk-cmp"><span class="pk-ib">${ic('plusc', 24)}</span>${field('Text message', `<span class="pk-ib" style="width:30px;height:22px">${ic('smile', 22)}</span>`)}<span class="pk-snd"><i class="a">${ic('image', 22)}</i><i class="b">${ic('send', 20)}</i></span></div></div>
    <div class="pk-akb"><div class="pk-tb" data-m="idle">
      <div class="pk-tl pk-t0"><span class="pk-ib pk-ti">${ic('gear', 22)}</span><span class="pk-vb">${ic('mic', 20)}Tap or hold to talk</span><span class="pk-ib pk-ti off">${ic('undo', 22)}</span></div>
      <div class="pk-tl pk-t1"><div class="fbh pk-fb"><div data-fb data-hidden></div></div></div>
      <div class="pk-tl pk-t2"><span class="pk-ib pk-ti">${ic('gear', 22)}</span><span class="pk-vb ok">${ic('check', 20)}Inserted</span><span class="pk-ib pk-ti">${ic('undo', 22)}</span></div>
    </div><div class="pk-rows">${aRows()}</div></div><i class="pk-gn"></i>`);

  // Android Home: the real list. The Server mode switch opens the dark Server screen.
  const arow = (i, t, s, tr = '') => `<div class="pk-r">${ic(i, 22)}<div class="tt"><b>${t}</b><span>${s}</span></div>${tr}</div>`;
  const srvRows = (desk = 'None yet') => [['Port', '7342'], ['Speech model', 'Whisper Base Q5'], ['Cleanup', 'Rules'], ['Desktops', `<span data-desk>${desk}</span>`], ['Unload', 'Keep loaded']].map(([k, v]) => `<div class="pk-sr"><span>${k}</span><b>${v}</b></div>`).join('');
  const androidSrv = desk => `<div class="pk-asrv"><div class="pk-sc"><div class="pk-sh"><span>Server mode</span><b class="pk-sv">Serving</b></div>${srvRows(desk)}<div class="pk-sf">${ic('bolt', 16)}Plug in to keep serving</div></div><p>Keeps serving with the screen off.</p><div class="pk-sb2">Stop Server mode</div></div>`;
  const androidHome = (desk, sub = 'Share models with your computer') => frame('a', `${sbA}<div class="pk-ahome"><div class="pk-ttl">Hushpen</div>
    <div class="pk-g">${arow('kbd', 'Keyboard', 'On and selected', `<span class="ck">${ic('checkc', 24)}</span>`)}${arow('mic', 'Microphone', 'Allowed', `<span class="ck">${ic('checkc', 24)}</span>`)}${arow('lines', 'Speech model', 'Whisper Base Q5 · Ready', `<span class="ck">${ic('checkc', 24)}</span>`)}</div>
    <div class="pk-g"><div class="pk-r">${ic('radio', 22)}<div class="tt"><b>Server mode</b><span>${sub}</span></div><span class="pk-sw" role="presentation"><i class="ck">${ic('check', 14)}</i></span></div></div>
    <div class="pk-g">${arow('box', 'Models', '2 speech models, system cleanup')}${arow('laptop', 'Devices', 'No paired desktops')}${arow('gear', 'Settings', 'Cleanup, haptics, licenses')}</div></div>${androidSrv(desk)}`);

  // iPhone: Messages, the Hushpen keyboard, the app's listening page, Home, and Server mode
  const ikey = (l, c = '', f = 1) => `<span class="pk-k ${c}" style="flex-grow:${f}">${l}</span>`;
  const iRows = () => {
    const r = ['qwertyuiop', 'asdfghjkl', 'zxcvbnm'];
    return `<div class="pk-row">${[...r[0]].map(c => ikey(c.toUpperCase())).join('')}</div>
      <div class="pk-row"><span class="pk-k gap"></span>${[...r[1]].map(c => ikey(c.toUpperCase())).join('')}<span class="pk-k gap"></span></div>
      <div class="pk-row"><span class="pk-k fn" style="flex-grow:1.4">${ic('shift', 20)}</span>${[...r[2]].map(c => ikey(c.toUpperCase())).join('')}<span class="pk-k fn" style="flex-grow:1.4">${ic('bksp', 22)}</span></div>
      <div class="pk-row"><span class="pk-k fn" style="flex-grow:1.3">123</span><span class="pk-k" style="flex-grow:5.1;font-size:16px">space</span><span class="pk-k mic" style="flex-grow:1.3" data-mic>${ic('mic', 22)}</span><span class="pk-k fn" style="flex-grow:2.3">return</span></div>`;
  };
  const iosMsg = (app = true) => frame('ios', `${sbI}<div class="pk-im"><div class="pk-imn"><span class="pk-gb">${ic('back', 22)}</span><div class="pk-who"><span class="pk-av">P</span>Priya</div><span class="pk-gb">${ic('video', 22)}</span></div>
    <div class="pk-th">${bubbles.replace('Today · 12:24', '<b>Today</b> 12:24')}</div>
    <div class="pk-cmp"><span class="pk-gb">${ic('plus', 22)}</span>${field('iMessage', `<span class="pk-snd"><i class="a"></i><i class="b">${ic('send', 18)}</i></span>`)}</div></div>
    <div class="pk-ikb"><div class="pk-tb" data-m="idle">
      <div class="pk-tl pk-t0"><span class="pk-it">Tap the mic to dictate</span></div>
      <div class="pk-tl pk-t1"><span class="pk-gb s">${ic('x', 18)}</span><span class="pk-it on">${wave(8, '--h:16px;--w:2.5px;--g:2px')}<span>Listening in Hushpen · <span data-t>0:00</span></span></span><span class="pk-gb s p">${ic('check', 18)}</span></div>
      <div class="pk-tl pk-t2"><span class="pk-undo">${ic('undo', 16)}Undo</span><span class="pk-it" style="text-align:left;color:#000;font-weight:500">Inserted</span></div>
    </div><div class="pk-rows">${iRows()}</div><span class="pk-ibot">${ic('globe', 26)}</span></div>
    ${app ? `<div class="pk-iapp">${wave(30, '--h:74px')}<div class="pk-lt">Listening</div><div class="pk-lm" data-t>0:00</div>
      <div class="pk-lc">${ic('back', 22)}<div><b>Go back and keep talking</b><span>Hushpen listens in the background and shows it in the Dynamic Island. Tap the check mark in the keyboard to insert.</span></div></div>
      <div class="pk-lh">Swipe right on the bar below to go back.</div></div>` : ''}<i class="pk-hi"></i>`);
  const irow = (i, t, v, tr = '') => `<div class="pk-r">${ic(i, 22)}<div class="tt"><b>${t}</b></div>${v ? `<span class="v">${v}</span>` : ''}${tr}</div>`;
  const iosHome = () => frame('ios', `${sbI}<div class="pk-ihome"><div class="pk-ilt">Hushpen</div><div class="pk-ih">Setup</div>
    <div class="pk-ig">${irow('kbd', 'Keyboard', 'Last used today')}${irow('mic', 'Microphone', 'Allowed')}${irow('lines', 'Speech Model', 'Parakeet v3 Q4')}</div>
    <div class="pk-ig" style="margin-top:22px">${irow('radio', 'Server Mode', '', '<span class="pk-isw"></span>')}</div>
    <div class="pk-ift">Keep Hushpen open and plugged in. Your computer can then use this phone's models.</div></div>
    <div class="pk-isrv"><div class="d">Monday 9 November</div><div class="c">9:41</div><div class="s">Serving</div><div class="g">${ic('bolt', 16)}Plug in to keep serving</div><div class="f">Server mode stops when you leave Hushpen.</div><div class="b">Stop Server Mode</div></div><i class="pk-hi"></i>`);

  const center = (el, root) => {
    let x = 0, y = 0, n = el;
    while (n && n !== root) { x += n.offsetLeft; y += n.offsetTop; n = n.offsetParent; }
    return [x + el.offsetWidth / 2, y + el.offsetHeight / 2];
  };

  // One phone's controls. beat() waits and holds while the scene is off screen.
  const ctl = (w, beat) => {
    const scr = w.querySelector('.pk-scr'), tap = scr.querySelector('.pk-tap'), q = s => scr.querySelector(s);
    const S = { w, scr, tap, q, beat, timer: 0, off: 0 };
    S.touch = async (t, hold = 150) => {
      clearTimeout(S.off);
      const [x, y] = Array.isArray(t) ? t : center(t, scr);
      tap.style.transform = `translate(${(x - 20).toFixed(1)}px, ${(y - 20).toFixed(1)}px)`;
      tap.classList.add('on');
      await beat(560);
      tap.classList.add('dn');
      await beat(hold);
      tap.classList.remove('dn');
      S.off = setTimeout(() => tap.classList.remove('on'), 420);
    };
    S.clock = on => {
      clearInterval(S.timer);
      const set = n => scr.querySelectorAll('[data-t]').forEach(e => { e.textContent = mmss(n); });
      set(0);
      if (!on) return;
      const t0 = Date.now();
      S.timer = setInterval(() => set(Math.floor((Date.now() - t0) / 1000)), 250);
    };
    return S;
  };
  const withField = S => {
    const fd = S.q('.pk-fd'), tx = S.q('.pk-tx'), snd = S.q('.pk-snd');
    const has = () => { const on = !!tx.textContent; fd.classList.toggle('has', on); snd.classList.toggle('has', on); };
    S.text = (s, fade) => { tx.textContent = ''; if (s) S.add(s, fade); has(); };
    S.add = (s, fade) => {
      const n = document.createElement('span');
      n.textContent = s;
      if (fade) n.className = 'n';
      tx.append(n);
      has();
    };
    S.mode = m => { S.q('.pk-tb').dataset.m = m; };
    return S;
  };

  const androidKbCtl = (w, beat, seed = 5) => {
    const S = withField(ctl(w, beat));
    const host = S.q('[data-fb]');
    S.fb = FB.create(host, { scale: 1.3, seed, hidden: true });
    S.key = async c => {
      const k = S.q(`.pk-k[data-k="${c.toLowerCase()}"]`);
      k?.classList.add('dn');
      S.add(c);
      await beat(70);
      k?.classList.remove('dn');
      await beat(40);
    };
    S.reset = (pre = '') => { S.text(pre); S.mode('idle'); S.fb.hide(); S.tap.classList.remove('on'); };
    S.final = () => { S.text(WORDS); S.mode('done'); };
    S.say = async () => {
      const bar = S.q('.pk-vb'), tb = S.q('.pk-tb');
      await S.touch(bar);
      S.mode('fb');
      S.fb.set('listening').show();
      await beat(2400);
      await S.touch([180 + 45 * 1.3, center(tb, S.scr)[1]]);
      S.fb.set('transcribing');
      await beat(1000);
      S.add(HEARD, true);
      S.mode('done');
      S.fb.hide();
      await beat(1700);
    };
    return S;
  };
  const androidHomeCtl = (w, beat) => {
    const S = ctl(w, beat), sw = S.q('.pk-sw'), srv = S.q('.pk-asrv');
    S.reset = () => { sw.classList.remove('on'); srv.classList.remove('on'); };
    S.final = () => { sw.classList.add('on'); srv.classList.add('on'); };
    S.serve = async () => {
      await beat(500);
      await S.touch(sw);
      sw.classList.add('on');
      await beat(900);
      srv.classList.add('on');
      await beat(3200);
    };
    return S;
  };
  const iosMsgCtl = (w, beat, seed = 5) => {
    const S = withField(ctl(w, beat)), isl = S.q('.pk-isl'), app = S.q('.pk-iapp');
    const pw = w.matches('.pk-w') ? w : w.querySelector('.pk-w');
    const la = on => { isl.classList.toggle('la', on); pw.classList.toggle('is-la', on); };
    S.reset = () => { S.text(''); S.mode('idle'); la(false); app?.classList.remove('on'); S.clock(false); S.tap.classList.remove('on'); };
    S.final = () => { S.text(WORDS); S.mode('done'); };
    // Tap the mic: the keyboard opens Hushpen, a swipe on the bar goes back, and the Island shows the session
    S.start = async () => {
      await S.touch(S.q('[data-mic]'));
      la(true); S.clock(true);
      app.classList.add('on');
      await beat(2300);
      clearTimeout(S.off);
      S.tap.style.transition = 'none';
      S.tap.style.transform = 'translate(120px, 740px)';
      void S.tap.offsetWidth;
      S.tap.style.transition = '';
      S.tap.classList.add('on', 'dn');
      await beat(40);
      S.tap.style.transform = 'translate(250px, 740px)';
      await beat(560);
      app.classList.remove('on');
      S.mode('fb');
      S.tap.classList.remove('dn', 'on');
      await beat(700);
    };
    S.say = async () => {
      await beat(1700);
      await S.touch(S.q('.pk-t1 .pk-gb.p'));
      la(false); S.clock(false);
      S.text(WORDS, true);
      S.mode('done');
      await beat(1900);
    };
    S.short = async () => {
      await S.touch(S.q('[data-mic]'));
      S.clock(true); S.mode('fb');
      await beat(2300);
      await S.touch(S.q('.pk-t1 .pk-gb.p'));
      S.clock(false);
      S.text(WORDS, true);
      S.mode('done');
      await beat(1900);
    };
    return S;
  };
  const iosHomeCtl = (w, beat) => {
    const S = ctl(w, beat), sw = S.q('.pk-isw'), srv = S.q('.pk-isrv');
    S.reset = () => { sw.classList.remove('on'); srv.classList.remove('on'); };
    S.final = () => { sw.classList.add('on'); srv.classList.add('on'); };
    S.serve = async () => {
      await beat(500);
      await S.touch(sw);
      sw.classList.add('on');
      await beat(1000);
      srv.classList.add('on');
      await beat(3200);
    };
    return S;
  };

  // A scene's clock: plays while the host is on screen
  const watch = host => {
    const P = { vis: false };
    new IntersectionObserver(([e]) => { P.vis = e.isIntersecting; host.classList.toggle('ln-live', P.vis); }, { threshold: 0.15 }).observe(host);
    return { P, beat: async ms => { await sleep(ms); while (!P.vis || document.hidden) await sleep(200); } };
  };

  // The Android and iPhone sections: one phone dictates, the other serves its models
  const narrow = () => matchMedia('(max-width: 560px)').matches;
  const section = (host, os) => {
    const st = host.querySelector('.ph-st'), steps = [...host.querySelectorAll('.ln-steps > li')];
    const android = os === 'android', { beat } = watch(host);
    const cap = (t, s) => `<p class="ph-cap">${t}<span>${s}</span></p>`;
    st.innerHTML = `<div class="ph-c is-act" data-c="a">${android ? androidKb() : iosMsg()}${cap('Dictate in any app', android ? 'The voice bar sits above the keys' : 'The keyboard starts a session in the app')}</div>
      <div class="ph-c" data-c="b">${android ? androidHome() : iosHome()}${cap('Server mode', android ? 'It keeps serving in the background' : 'App open, charging, screen dimmed')}</div>`;
    const [ca, cb] = st.children;
    const A = android ? androidKbCtl(ca, beat, 5) : iosMsgCtl(ca, beat, 5), B = android ? androidHomeCtl(cb, beat) : iosHomeCtl(cb, beat);
    const act = i => { ca.classList.toggle('is-act', i !== 2); cb.classList.toggle('is-act', i === 2); steps.forEach((li, j) => li.classList.toggle('is-on', j === i)); };
    if (calm) {
      A.reset(); B.reset(); A.final(); B.final();
      ca.classList.add('is-act'); cb.classList.add('is-act');
      return;
    }
    A.reset(); B.reset();
    const ACTS = android
      ? [async () => { await beat(500); for (const c of TYPED) await A.key(c); await beat(700); }, () => A.say(), () => B.serve()]
      : [() => A.start(), () => A.say(), () => B.serve()];
    const D = android ? [3600, 7600, 6400] : [8600, 5400, 6600];
    (async () => {
      for (let n = 0; ; n++) {
        for (let i = 0; i < 3; i++) {
          act(i);
          const t0 = performance.now();
          if (i === 0) { A.reset(); B.reset(); await beat(300); }
          await ACTS[i]();
          await beat(Math.max(250, D[i] - (performance.now() - t0)));
        }
        act(-1);
        ca.classList.toggle('is-act', !narrow());
        cb.classList.add('is-act');
        await beat(1400);
      }
    })();
  };

  // The keyboards tile: both keyboards, cropped to their lower part, dictating in turn
  const tile = host => {
    const { beat } = watch(host);
    host.innerHTML = `<div class="kb-ph a">${androidKb()}</div><div class="kb-ph b">${iosMsg(false)}</div>`;
    const [pa, pb] = host.children;
    const A = androidKbCtl(pa.firstElementChild, beat, 12), I = iosMsgCtl(pb.firstElementChild, beat, 12);
    host.classList.toggle('solo', narrow());
    if (calm) { A.reset(); I.reset(); A.final(); I.final(); pb.classList.toggle('off', narrow()); return; }
    A.reset(TYPED); I.reset();
    (async () => {
      for (let k = 0; ; k++) {
        const solo = narrow();
        host.classList.toggle('solo', solo);
        pa.classList.toggle('off', solo && k % 2 === 1);
        pb.classList.toggle('off', solo && k % 2 === 0);
        A.reset(TYPED); I.reset();
        await beat(700);
        await Promise.all([A.say(), (async () => { await beat(300); await I.short(); })()]);
        await beat(900);
      }
    })();
  };

  window.PK = { ic, IC, wave, frame, sbA, center, ctl, watch, androidHome, androidHomeCtl, SB };
  document.querySelectorAll('[data-phone]').forEach(h => section(h, h.dataset.phone));
  document.querySelectorAll('[data-kbtile]').forEach(tile);
})();
