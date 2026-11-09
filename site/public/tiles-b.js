// Shots for four feature tiles (History, Dictionary and word packs, Manual, Read aloud), played by TileMotion.
// Each shot is a crop of the Hushpen window in the Light appearance. Its base styles are the end state
// of the shot, so Reduce Motion (no animations) shows a settled, readable frame.
(() => {
  const P = {
    search: '<circle cx="11" cy="11" r="8"/><path d="m21 21-4.3-4.3"/>',
    clock: '<circle cx="12" cy="12" r="10"/><path d="M12 6v6l4 2"/>',
    volume: '<path d="M11 4.7a.7.7 0 0 0-1.2-.5L6.4 7.6a1.4 1.4 0 0 1-1 .4H3a1 1 0 0 0-1 1v6a1 1 0 0 0 1 1h2.4a1.4 1.4 0 0 1 1 .4l3.4 3.4a.7.7 0 0 0 1.2-.5z"/><path d="M16 9a5 5 0 0 1 0 6"/><path d="M19.4 18.4a9 9 0 0 0 0-12.8"/>',
    alert: '<circle cx="12" cy="12" r="10"/><path d="M12 8v4M12 16h.01"/>',
    x: '<path d="M18 6 6 18M6 6l12 12"/>',
    share: '<path d="M12 2v13M16 6l-4-4-4 4"/><path d="M4 12v8a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2v-8"/>',
    refresh: '<path d="M3 12a9 9 0 0 1 9-9 9.75 9.75 0 0 1 6.74 2.74L21 8"/><path d="M21 3v5h-5"/><path d="M21 12a9 9 0 0 1-9 9 9.75 9.75 0 0 1-6.74-2.74L3 16"/><path d="M8 16H3v5"/>',
    trash: '<path d="M3 6h18M19 6v14c0 1-1 2-2 2H7c-1 0-2-1-2-2V6M8 6V4c0-1 1-2 2-2h4c1 0 2 1 2 2v2"/>',
    copy: '<rect width="14" height="14" x="8" y="8" rx="2"/><path d="M4 16c-1.1 0-2-.9-2-2V4c0-1.1.9-2 2-2h10c1.1 0 2 .9 2 2"/>',
    clip: '<path d="M16 14v2.2l1.6 1M16 4h2a2 2 0 0 1 2 2v.8M8 4H6a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h2"/><circle cx="16" cy="16" r="6"/><rect x="8" y="2" width="8" height="4" rx="1"/>',
    play: '<path d="M6 4.5a1.5 1.5 0 0 1 2.3-1.3l11 7.5a1.5 1.5 0 0 1 0 2.6l-11 7.5A1.5 1.5 0 0 1 6 19.5z"/>',
    pause: '<rect x="14" y="4" width="4" height="16" rx="1"/><rect x="6" y="4" width="4" height="16" rx="1"/>',
    lines: '<path d="M2 10v3M6 6v11M10 3v18M14 8v7M18 5v13M22 10v3"/>',
    globe: '<circle cx="12" cy="12" r="10"/><path d="M12 2a14.5 14.5 0 0 0 0 20 14.5 14.5 0 0 0 0-20M2 12h20"/>',
    chev: '<path d="m6 9 6 6 6-6"/>',
    undo: '<path d="M9 14 4 9l5-5"/><path d="M4 9h10.5a5.5 5.5 0 0 1 0 11H11"/>',
  };
  const ico = (n, c = '') => `<svg class="tb-i ${c}" viewBox="0 0 24 24">${P[n]}</svg>`;
  const ib = (n, c = '') => `<span class="tb-ib ${c}">${ico(n)}</span>`;
  const btn = (t, c = '') => `<span class="tb-b ${c}">${t}</span>`;
  const pop = t => `<span class="tb-b tb-pop">${t}${ico('chev')}</span>`;
  const tt = (t, s) => `<div class="tb-tt"><b>${t}</b>${s ? `<small>${s}</small>` : ''}</div>`;
  const row = (inner, c = '', st = '') => `<div class="tb-r ${c}"${st ? ` style="${st}"` : ''}>${inner}</div>`;
  const kv = (k, v, t) => row(`<b>${k}</b><span>${v}</span>`, 'tb-kv tb-a', `--t:${t}ms`);
  const grp = (inner, c = '') => `<div class="tb-g ${c}">${inner}</div>`;
  const head = (t, right = '') => `<div class="tb-h"><span>${t}</span>${right}</div>`;
  const bar = (t, right = '') => `<div class="tb-tb"><b>${t}</b>${right}</div>`;
  const seg = (list, on) => `<span class="tb-seg">${list.map((s, i) => `<span${i === on ? ' class="on"' : ''}>${s}</span>`).join('')}</span>`;
  const cursor = (cx = 44, cy = 30) => `<i class="tb-cur" style="--cx:${cx}px;--cy:${cy}px"><svg viewBox="0 0 19 28"><path d="M2 2v20.5l5-4.7 3.2 7.4 3.4-1.5-3.2-7.3H17z"/></svg></i>`;
  const typed = (s, t, step = 70) => `<span class="tb-ty" style="--t:${t}ms;--s:${step}ms">${[...s].map((ch, j) => `<i style="--j:${j}">${ch === ' ' ? '&nbsp;' : ch}</i>`).join('')}</span>`;
  const words = (s, t, step, c = 'tb-wd') => s.split(' ').map((w, j) => `<span class="${c}" style="--t:${t}ms;--s:${step}ms;--j:${j}">${w}</span>`).join(' ');
  const swap = (was, now) => `<span class="tb-sx"><span class="tb-was">${was}</span><span class="tb-now">${now}</span></span>`;
  const shot = (w, h, inner, c = 0) => `<div class="tb" aria-hidden="true" data-w="${w}" data-h="${h}" style="--c:${c}ms"><div class="tb-c">${inner}</div></div>`;
  const pane = inner => `<div class="tb-p">${inner}</div>`;

  // History: search "invoice", open the row, see what each pass made, export SRT
  const HIST = [
    ['clock', 'Send the invoice to Elena tomorrow.', '2025-06-01 09:15 · Mail · 2.4 s · Pasted', 1],
    ['clock', 'Remind me to call the dentist on Thursday.', '2025-06-01 08:52 · Firefox · 2.7 s · Copied only'],
    ['volume', 'The meeting moved to Friday at ten.', '2025-06-01 08:30 · Read aloud · Kokoro · Heart with 30% Adam'],
    ['clock', 'Thanks for the quick reply. The invoice is attached.', '2025-06-01 07:15 · Slack · 3.4 s · Pasted', 1],
    ['alert', 'The recording could not be read', '2025-06-01 06:15 · Firefox · 4.1 s · Failed'],
    ['x', 'Not transcribed', '2025-06-01 04:15 · Firefox · 5.5 s · Canceled'],
    ['clock', 'The quarterly numbers look fine, but the chart needs a title.', '2025-06-01 03:15 · Numbers · 6.2 s · Pasted'],
  ];
  const hrow = ([i, t, s], c = '', inner = '') => row(`${ico(i, 'tb-lead')}${tt(t, s)}${inner}`, c);
  const field = (inner, c = '') => `<span class="tb-f ${c}">${ico('search')}${inner}</span>`;
  const searchRow = inner => row(`${field(inner, 'foc')}${btn('Select', 'tb-opt')}${btn('Clear all')}`, 'tb-sr');
  const META = '2025-06-01 09:15 · Mail · 2.4 s · Pasted';
  const topRow = row(`${btn('Back')}<small class="tb-meta">${META}</small>${ib('clip')}${ib('copy')}`, 'tb-dt');
  const RAW = 'um send the the invoice to elena tomorrow', FIN = 'Send the invoice to Elena tomorrow.';
  const hi = {
    shots: [
      { cap: 'Search', ms: 1700, focus: [32, 16], html: shot(460, 300, bar('History') + pane(grp(
        searchRow(`${typed('invoice', 250)}<i class="tb-k"></i>`) +
        HIST.map(r => hrow(r, r[3] ? '' : 'tb-dim')).join('')
      )), 900) },
      { cap: 'Found', ms: 1300, focus: [40, 30], html: shot(460, 300, bar('History') + pane(grp(
        searchRow('<span>invoice</span><i class="tb-k"></i>') +
        hrow(HIST[0], 'tb-pick', cursor(60, 44)) + hrow(HIST[3])
      )), 800) },
      { cap: 'Raw to final', ms: 1900, focus: [62, 58], html: shot(460, 300, bar('History') + pane(grp(
        topRow +
        row(`<b class="tb-rt">Recording</b>${ib('share')}${ib('refresh')}${ib('trash')}`, 'tb-a', '--t:40ms') +
        row(`${ib('pause')}<i class="tb-seek"><b></b></i><small class="tb-tm">0:01 / 0:02</small>`, 'tb-a tb-pl', '--t:80ms') +
        kv('Raw text', RAW, 160) + kv('After the rules', FIN, 300) + kv('Final text', FIN, 440) + kv('App', 'Mail', 520) + kv('Duration', '2.4 s', 580)
      ))) },
      { cap: 'Export', ms: 1800, focus: [70, 42], html: shot(460, 300, bar('History') + pane(grp(
        topRow +
        row(`<b class="tb-rt">Recording</b>${ib('share', 'on')}${ib('refresh')}${ib('trash')}`) +
        row(`<b class="tb-rt">Export as</b>${btn('TXT')}${btn('SRT' + cursor(40, 34), 'tb-hit tb-chosen')}${btn('VTT')}${btn('JSON')}`, 'tb-ex') +
        kv('Raw text', RAW, 0) + kv('After the rules', FIN, 0) + kv('Final text', FIN, 0)
      )) + `<div class="tb-card tb-ql" style="--t:850ms"><div class="tb-ch"><i class="tb-tl"><b></b><b></b><b></b></i><span>Send the invoice.srt</span></div><pre>1\n00:00:00,000 --> 00:00:02,400\nSend the invoice to Elena tomorrow.</pre></div>`, 650) },
    ],
  };

  // Dictionary: "Siobhan" is heard as "Shivon", the word is added, a pack goes on, and the next text is right
  const homeDictation = tx => bar('Home') + pane(grp(
    row(`${ico('lines', 'tb-lead')}<b class="tb-rt">Dictation</b><span class="tb-ok">Ready</span>${btn('Record')}`) +
    `<div class="tb-tx">${tx}</div>` +
    row(`${ico('globe', 'tb-lead')}${tt('Language', 'Detects the language you speak')}${pop('Auto')}`)
  ));
  const word = (w, h) => row(`${tt(w, `Heard as “${h}”`)}${btn('Edit', 'tb-opt')}${btn('Delete')}`);
  const pack = (n, s, on, c = '', extra = '') => row(`${tt(n, s)}${btn('Preview…', 'tb-opt')}<i class="tb-sw${on ? ' on' : ''} ${c}">${extra}</i>`);
  const di = {
    shots: [
      { cap: 'Heard', cam: [50, 46, 1.08], ms: 1500, focus: [50, 50], html: shot(360, 170, homeDictation(`<span class="tb-a" style="--t:120ms">Lunch with <u class="tb-bad" style="--t:700ms">Shivon</u> at one?</span>`)) },
      { cap: 'Add word', cam0: [34, 30, 1.35], cam: [64, 36, 1.35], ms: 1900, focus: [36, 26], html: shot(360, 170, bar('Dictionary') + pane(grp(
        row(`<span class="tb-f tb-fw foc"><span class="tb-was">${typed('Siobhan', 150, 60)}</span><span class="tb-ph tb-now">Word or phrase to write</span></span>` +
          `<span class="tb-f tb-fw"><span class="tb-was">${typed('Shivon', 600, 55)}</span><span class="tb-ph tb-now">Heard as (optional)</span></span>` +
          btn('Add' + cursor(34, 30), 'dft tb-hit'), 'tb-sr') +
        `<div class="tb-r tb-new tb-a" style="--t:var(--c)">${tt('Siobhan', 'Heard as “shivon”')}${btn('Edit', 'tb-opt')}${btn('Delete')}</div>` +
        word('Hushpen', 'hush pen') + word('Joaquín Ruiz', 'wah keen rooeth')
      )), 1250) },
      { cap: 'Word pack', cam0: [50, 50, 1], cam: [50, 54, 1.06], ms: 1500, focus: [78, 52], html: shot(360, 170, bar('Dictionary') + pane(head('Packs') + grp(
        pack('Your Dictionary', '3 words. Names and terms you added.', true) +
        pack('Tech terms', '221 words. Software, tools, and AI names with their official spelling.', true, 'tb-flip', cursor(40, 26))
      ) + '<p class="tb-ft">Words of a pack that is on count like your own. Your words come first.</p>'), 800) },
      { cap: 'Spelled right', cam: [50, 40, 1.06], ms: 1700, focus: [50, 50], html: shot(360, 170, homeDictation(`<span class="tb-a" style="--t:100ms">Lunch with <u class="tb-good" style="--t:520ms">Siobhan</u> at one? Then the <u class="tb-good" style="--t:700ms">Kubernetes</u> demo.</span>`) + '<span class="tb-hud" style="--t:180ms">Pasted</span>') },
    ],
  };

  // Manual: a file drops in, the queue runs it, the transcript is done, and Copy puts it in Notes
  const manualBar = on => bar('Manual', seg(['Transcribe', 'Read aloud'], on));
  const zoneSmall = `<div class="tb-dz sm">${ico('lines')}<span>Drop audio files here</span></div>`;
  const queue = rows => head('Queue', btn('Clear Finished')) + grp(rows);
  const snippet = 'Let us start with the roadmap for June.';
  const ma = {
    shots: [
      { cap: 'Drop', cam: [62, 48, 1.12], ms: 1500, focus: [50, 52], html: shot(360, 210, manualBar(0) + pane(
        `<div class="tb-dz">${ico('lines', 'tb-dzi')}<b>Drop audio files here</b><small>WAV, MP3, FLAC, M4A, and more</small>${btn('Choose Files…', 'dft')}` +
        `<span class="tb-fi"><i class="tb-doc">${ico('lines')}</i><em>Interview.wav</em>${cursor(0, 0)}</span></div>` +
        head('Options') + grp(row(`${tt('Model', 'Change it in Models')}<span class="tb-v">Parakeet v3 Q4</span>`))
      ), 560) },
      { cap: 'Queue', cam: [54, 52, 1.1], ms: 1600, focus: [50, 62], html: shot(360, 210, manualBar(0) + pane(zoneSmall + queue(
        row(`${tt('Interview.wav', '212 MB · 47:12 · About a minute left')}<i class="tb-pg" style="--t:120ms;--p:.64"><b></b></i><span class="tb-pc" style="--t:120ms;--tbn:64"></span>${btn('Cancel', 'tb-opt2')}`) +
        row(`${tt('Planning call.m4a', '3 MB · 0:04 · ~/Downloads')}<span class="tb-st">Waiting</span>${btn('Cancel', 'tb-opt2')}`)
      ))) },
      { cap: 'Transcript', cam: [46, 52, 1.1], ms: 1400, focus: [36, 62], html: shot(360, 210, manualBar(0) + pane(zoneSmall + queue(
        row(`<div class="tb-tt"><b>Interview.wav</b><small>${words(snippet + '…', 120, 70)}</small></div><span class="tb-ok tb-a" style="--t:60ms">Done</span>${btn('Copy')}${btn('Export' + ico('chev'), 'tb-pop tb-opt')}`) +
        row(`${tt('Planning call.m4a', '3 MB · 0:04 · Less than a minute left')}<i class="tb-pg" style="--t:200ms;--p:.18"><b></b></i><span class="tb-pc" style="--t:200ms;--tbn:18"></span>${btn('Cancel', 'tb-opt2')}`)
      ))) },
      { cap: 'Copy', cam0: [62, 40, 1.2], cam: [50, 50, 1], ms: 1700, focus: [66, 70], html: shot(360, 210, manualBar(0) + pane(queue(
        row(`${tt('Interview.wav', snippet + '…')}<span class="tb-ok">Done</span>${btn('Copy' + cursor(36, 26), 'tb-hit')}${btn('Export' + ico('chev'), 'tb-pop tb-opt')}`) +
        row(`${tt('Planning call.m4a', '3 MB · 0:04 · Let us move the review…')}<span class="tb-ok">Done</span>${btn('Copy')}${btn('Export' + ico('chev'), 'tb-pop tb-opt')}`)
      )) + `<div class="tb-card tb-nt" style="--t:calc(var(--c) + 180ms)"><div class="tb-ch"><i class="tb-tl"><b></b><b></b><b></b></i><span>Notes</span></div><p><span class="tb-a" style="--t:calc(var(--c) + 420ms)">${snippet} First, the new Dictionary view ships on Friday.</span><i class="tb-k"></i></p></div>`, 650) },
    ],
  };

  // Read aloud: the text, the voice "Heart with 30% Adam", then it plays and lands in History
  const TEXT = 'The meeting moved to Friday at ten. Bring the notes from Monday.';
  const readBar = manualBar(1);
  const status = (st, right) => row(`<b class="tb-rt">${st}</b>${right}`);
  const chars = `${TEXT.length} characters`;
  let seed = 7;
  const rnd = () => (seed = (seed * 16807) % 2147483647) / 2147483647;
  const wave = Array.from({ length: 22 }, (_, n) => `<i style="--n:${n};--r:${rnd().toFixed(2)};--h:${(0.3 + 0.7 * rnd()).toFixed(2)}"></i>`).join('');
  const ra = {
    shots: [
      { cap: 'Text', ms: 1500, focus: [44, 50], html: shot(270, 180, readBar + pane(grp(
        status('Ready', btn('Export Audio…', 'tb-opt') + btn('Play', 'dft')) +
        `<div class="tb-ta"><p>${words(TEXT, 100, 45)}</p></div>` +
        row(`<span class="tb-v tb-l">${chars}</span>${btn('Open Text File…')}`)
      ))) },
      { cap: 'Voice', ms: 1700, focus: [62, 70], html: shot(270, 180, readBar + pane(grp(
        row(`${tt('Voice', 'Default is the engine’s own choice')}${pop('Heart')}`) +
        row(`${tt('Mix with', 'Blends a second voice into this one')}<span class="tb-b tb-pop tb-hit" style="--c:300ms">${swap('None', 'Adam')}${ico('chev')}${cursor(30, 24)}</span>`, '', '--c:300ms') +
        row(`${tt('Mix', 'How much of Adam the voice has')}<span class="tb-sv" style="--t:620ms;--tbn:30"></span><i class="tb-sl" style="--t:620ms"><i></i><b></b></i>`)
      ))) },
      { cap: 'Heart with 30% Adam', ms: 2000, focus: [50, 40], html: shot(270, 180, readBar + pane(grp(
        status(swap('Ready', 'Reading'), `<span class="tb-wv tb-now">${wave}</span>${btn('Export Audio…', 'tb-opt')}<span class="tb-b dft tb-hit">${swap('Play', 'Stop')}${cursor(30, 30)}</span>`) +
        `<div class="tb-ta"><p>${words(TEXT, 520, 115, 'tb-wd tb-hl')}</p></div>` +
        row(`<span class="tb-v tb-l">Heart with 30% Adam</span><small class="tb-tm">0:01 / 0:04</small>`)
      )), 380) },
      { cap: 'In History', ms: 1400, focus: [42, 36], html: shot(270, 180, bar('History') + pane(grp(
        row(`${field('<span class="tb-ph">Search transcripts</span>')}${btn('Select', 'tb-opt')}`, 'tb-sr') +
        hrow(['volume', 'The meeting moved to Friday at ten.', '2025-06-01 09:41 · Read aloud · Kokoro · Heart with 30% Adam'], 'tb-new tb-a') +
        hrow(HIST[0]) + hrow(HIST[1])
      ))) },
    ],
    still: 2,
  };

  // Each shot canvas lays out at its design size and scales to its box, so it fits any tile and any mode.
  // A tall box (the History tile on a phone) lays the canvas out narrower, so its rows fill more of it.
  const fit = el => {
    const w = el.clientWidth, h = el.clientHeight;
    if (!w || !h) return;
    const dw = h > w * 1.1 ? Math.min(+el.dataset.w, 330) : +el.dataset.w;
    const k = Math.max(0.55, Math.min(1.15, w / dw, h / +el.dataset.h));
    el.style.setProperty('--k', k.toFixed(3));
  };
  const ro = new ResizeObserver(es => es.forEach(e => fit(e.target)));
  const seen = new WeakSet();
  let queued = false;
  const scan = () => {
    queued = false;
    document.querySelectorAll('.tb').forEach(el => { if (!seen.has(el)) { seen.add(el); ro.observe(el); fit(el); } });
  };
  new MutationObserver(() => { if (!queued) { queued = true; requestAnimationFrame(scan); } }).observe(document.documentElement, { childList: true, subtree: true });

  const go = () => {
    if (!window.TileMotion) return;
    [['.t.hi', hi], ['.t.di', di, 1], ['.t.ma', ma, 1], ['.t.ra', ra]].forEach(([sel, data, cine]) => {
      const el = document.querySelector('.bento ' + sel);
      if (el) TileMotion.register(el, { ...data, cine: !!cine, aim: '.tb-tb > b, .tb-r, .tb-card, .tb-hud, .tb-dz' });
    });
    scan();
  };
  window.TileMotion ? go() : addEventListener('DOMContentLoaded', go, { once: true });
})();
