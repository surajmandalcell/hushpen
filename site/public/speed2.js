// Speed, told for people who are not developers and kept vague on numbers: Hushpen is built in Rust,
// drawn by GPUI on the GPU, and has no Chromium, Electron, or Tauri web view inside. One big statement
// with three columns that light up in turn: Rust, the GPU, and the design care. It loops only while on screen; with Reduce Motion it shows its last frame.
(() => {
  const host = document.querySelector('[data-speed]');
  if (!host) return;
  const calm = matchMedia('(prefers-reduced-motion: reduce)').matches;
  const STOP = Symbol('stop');
  const h = s => { const t = document.createElement('template'); t.innerHTML = s.trim(); return t.content.firstElementChild; };
  const $ = (r, s) => r.querySelector(s), $$ = (r, s) => [...r.querySelectorAll(s)];
  const HP = '<img src="assets/icon-256.webp" alt="">';
  const CHECK = '<svg viewBox="0 0 20 20" aria-hidden="true"><path d="M4.6 10.4l3.5 3.5 7.3-7.8"/></svg>';
  const ICON = {
    chat: '<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M7.9 20A9 9 0 1 0 4 16.1L2 22Z"/></svg>',
    video: '<svg viewBox="0 0 24 24" aria-hidden="true"><path d="m16 13 5.2 3.5a.5.5 0 0 0 .8-.4V7.9a.5.5 0 0 0-.8-.4L16 11"/><rect x="2" y="6" width="14" height="12" rx="2"/></svg>',
    note: '<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M15 3H6a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8Z"/><path d="M14 3v4a1 1 0 0 0 1 1h5M8 13h8M8 17h5"/></svg>',
    no: '<svg viewBox="0 0 20 20" aria-hidden="true"><path d="M5.5 5.5l9 9M14.5 5.5l-9 9"/></svg>'
  };
  const bd = (k, t) => `<span class="sp-bd ${k}">${t}</span>`;

  // A new run starts each time the part comes on screen; the old run stops at its next beat
  const loop = (el, step, still, wait = 0) => {
    if (calm) { still(); return; }
    let gen = 0, first = true;
    const start = async my => {
      const alive = () => my === gen;
      const beat = ms => new Promise((res, rej) => setTimeout(() => alive() ? res() : rej(STOP), ms));
      try { if (first) { first = false; await beat(wait); } for (;;) await step({ beat, alive }); }
      catch (e) { if (e !== STOP) throw e; }
    };
    new IntersectionObserver(es => { gen++; if (es[es.length - 1].isIntersecting) start(gen); }, { threshold: .3 }).observe(el);
  };

  // The icon on a spec sheet: rulers on two edges and dotted construction circles. A mask draws each dotted circle around; a pathLength of 1 lets one dash pattern serve every length.
  const SHEET = `<span class="dg" aria-hidden="true"><i class="dg-rt"></i><i class="dg-rl"></i><img src="assets/icon-256.webp" alt="">
    <svg viewBox="0 0 100 68"><defs>
      <mask id="dg-a" maskUnits="userSpaceOnUse" x="0" y="0" width="100" height="68"><circle class="dg-w" style="--d:.45s" cx="54" cy="38" r="26" pathLength="1" transform="rotate(-90 54 38)"/></mask>
      <mask id="dg-b" maskUnits="userSpaceOnUse" x="0" y="0" width="100" height="68"><circle class="dg-w" style="--d:.3s" cx="54" cy="38" r="20" pathLength="1" transform="rotate(-90 54 38)"/></mask>
    </defs>
      <circle class="dg-c" cx="54" cy="38" r="26" pathLength="54" mask="url(#dg-a)"/>
      <circle class="dg-c" cx="54" cy="38" r="20" pathLength="42" mask="url(#dg-b)"/>
      <path class="dg-l" style="--d:.6s" d="M54 8V68" pathLength="1"/><path class="dg-l" style="--d:.7s" d="M8 38H100" pathLength="1"/>
      <path class="dg-m" d="M54 0V8M0 38H8"/>
    </svg></span>`;

  const o1 = $(host, '[data-sp="badges"]');
  if (o1) {
    const px = `<span class="sp-px" aria-hidden="true">${Array.from({ length: 24 }, (_, i) => `<i style="--d:${((i % 8) + Math.floor(i / 8)) * 110}ms"></i>`).join('')}</span>`;
    const COLS = [
      [bd('rs', 'Rust'), 'Built in Rust', 'Compiled code that runs straight on your computer.'],
      [bd('gp', 'GPUI') + px, 'Drawn on the GPU', 'A real native window, drawn by your graphics chip, like a game.'],
      [SHEET, 'Designed with care', 'Hushpen is free, with the polish of a paid app. Every screen got real care.', true]
    ];
    const el = h(`<div class="sp-card sp-b1"><h3 class="sp-big">No browser inside. <span>No Chromium, no Electron.</span></h3><div class="sp-cols">${COLS.map(([v, t, p, x]) => `<div class="sp-col${x ? ' design' : ''}"><div class="sp-cv">${v}</div><h4>${t}</h4><p>${p}</p></div>`).join('')}</div></div>`);
    o1.append(el);
    const cols = $$(el, '.sp-col');
    loop(el, async ({ beat }) => {
      for (let i = 0; i < cols.length; i++) { cols.forEach((c, j) => c.classList.toggle('on', j === i)); await beat(2800); }
    }, () => cols.forEach(c => c.classList.add('on', 'still')));
  }

})();
