// The hero: one Mac desktop on the page that plays the windows of hw-win.js in a row and loops. For each
// window a camera starts wide on the desktop, closes in on the text field and the flow bar while Option + Tab
// starts hands-free and the words come in, then pulls back to the window for the clean text and what the
// app does with it. The flow bar is the real 28 pt capsule at the window's scale, so it looks big only
// while the camera is close. The two key caps show above it only while a press happens. The scene plays
// only while it is on screen. With Reduce Motion it is one still: the first window, close on its final text.
(() => {
  const st = document.querySelector('.hw-st');
  const list = (window.HW || []).filter(w => w && w.html && w.field && w.raw && w.final);
  if (!st || !window.FlowBar || !list.length) return;

  const EASE = 'cubic-bezier(.22,1,.36,1)';
  const calm = FlowBar.calm;
  // The desktop's own size in points; the camera scales and moves it inside the stage
  const W = 1280, H = 720;
  const dk = st.querySelector('.hw-dk'), stage = dk.querySelector('.hw-stage'), mb = dk.querySelector('.mb'), keys = dk.querySelector('.hw-keys');
  // The wave canvas is drawn for the closest shot, so it stays sharp when the camera is in
  const fb = FlowBar.create(dk.querySelector('[data-fb]'), { scale: 1, zoom: 2, seed: 11, hidden: true });
  const esc = s => String(s).replace(/&/g, '&amp;').replace(/</g, '&lt;');
  const CARET = '<i class="hx-c"></i>';
  const narrow = () => st.clientWidth < 560;
  let visible = false, win = null, shot = null;

  const make = spec => {
    const t = document.createElement('template');
    t.innerHTML = spec.html.trim();
    return t.content.firstElementChild;
  };
  // The bold app name and its menus go between the Apple menu and the status items
  const menus = (spec, fade) => {
    if (!mb) return;
    const old = () => mb.querySelectorAll(':scope > b, :scope > span:not(.mb-st):not(.mb-clock)');
    old().forEach(n => n.remove());
    mb.querySelector('.mb-apple').insertAdjacentHTML('afterend', `<b>${esc(spec.app)}</b>${(spec.menus || []).map(m => `<span>${esc(m)}</span>`).join('')}`);
    if (fade) old().forEach(n => n.animate([{ opacity: 0 }, { opacity: 1 }], { duration: 320, easing: EASE }));
  };

  // A box in the desktop's own points. It holds while the camera moves, since the camera scales and
  // moves the whole desktop.
  const box = n => {
    const d = dk.getBoundingClientRect(), k = d.width / W || 1, a = n.getBoundingClientRect();
    return [(a.left - d.left) / k, (a.top - d.top) / k, a.width / k, a.height / k];
  };
  const join = (...bs) => {
    const x = Math.min(...bs.map(b => b[0])), y = Math.min(...bs.map(b => b[1]));
    return [x, y, Math.max(...bs.map(b => b[0] + b[2])) - x, Math.max(...bs.map(b => b[1] + b[3])) - y];
  };
  // The text lines of a field with the given text in it, not the field box: a field that fills its row is
  // wider than its words
  const lines = (f, text) => {
    const keep = f.innerHTML;
    f.textContent = text;
    const rg = document.createRange();
    rg.selectNodeContents(f);
    const d = dk.getBoundingClientRect(), k = d.width / W || 1, rs = [...rg.getClientRects()];
    f.innerHTML = keep;
    if (!rs.length) return box(f);
    const lo = Math.min(...rs.map(r => r.left)), hi = Math.max(...rs.map(r => r.right)), top = Math.min(...rs.map(r => r.top)), bot = Math.max(...rs.map(r => r.bottom));
    return [(lo - d.left) / k, (top - d.top) / k, (hi - lo) / k, (bot - top) / k];
  };

  // The camera. With no shot the desktop fills the stage. A shot is a box to show whole, as close as the
  // stage allows, but never so close that the window text grows past about twice its size on a wide stage.
  // The desktop always covers the stage, so no edge shows.
  const frame = ms => {
    const sw = st.clientWidth, sh = st.clientHeight;
    if (!sw || !sh) return;
    const base = Math.max(sw / W, sh / H);
    let s = base, cx = W / 2, cy = H / 2;
    if (shot) {
      const [x, y, w, h] = shot, p = narrow() ? 20 : 32;
      s = Math.max(base, Math.min(sw / (w + 2 * p), sh / (h + 2 * p), Math.max(1, sw / 740)));
      cx = x + w / 2; cy = y + h / 2;
    }
    const tx = Math.min(0, Math.max(sw - W * s, sw / 2 - cx * s)), ty = Math.min(0, Math.max(sh - H * s, sh / 2 - cy * s));
    dk.style.transition = ms && !calm ? `transform ${ms}ms ${EASE}` : 'none';
    dk.style.transform = `translate(${tx.toFixed(1)}px, ${ty.toFixed(1)}px) scale(${s.toFixed(4)})`;
  };
  const cam = (b, ms) => { shot = b; frame(ms); };
  // A close shot keeps the flow bar and the key caps, which sit on the screen's center line, in the middle
  // of the frame: the box reaches as far to the left of that line as to the right
  const mid = ([x, y, w, h]) => { const r = Math.max(W / 2 - x, x + w - W / 2); return [W / 2 - r, y, 2 * r, h]; };
  // A window can sit anywhere on the screen. Each one sits with the part the close shot holds over the flow
  // bar, so that shot stays close; on a small stage that part is the heard text.
  const place = (spec, w) => {
    const f = w.querySelector(spec.field), z = spec.zone && w.querySelector(spec.zone);
    const t = narrow() ? f && lines(f, spec.raw) : z && box(z);
    if (!t) return;
    const b = box(w), dx = Math.max(16 - b[0], Math.min(W - 16 - b[0] - b[2], W / 2 - t[0] - t[2] / 2));
    w.style.translate = `${Math.round(dx)}px 0`;
  };
  new ResizeObserver(() => frame(0)).observe(st);

  // A pause that also waits while the hero is off screen or the tab is hidden
  const beat = async ms => {
    await new Promise(r => setTimeout(r, ms));
    while (!visible || document.hidden) await new Promise(r => setTimeout(r, 250));
  };

  // Puts window i on the stage, with its app in the menu bar. On a change the new window rises and fades
  // in over the old one, which stays under it until it is covered, so the wallpaper never shows through
  // between the two.
  const show = (i, swap) => {
    const spec = list[i];
    const old = [...stage.children];
    if (!swap) old.forEach(c => c.remove());
    fb.hide();
    menus(spec, swap);
    st.setAttribute('aria-label', `Hushpen types what you say into ${spec.app}: ${spec.final}`);
    win = make(spec);
    stage.append(win);
    place(spec, win);
    if (!swap) return;
    const done = () => old.forEach(c => c.remove());
    // The windows sit in different places, so the old one also fades while the new one comes in
    old.forEach(c => c.animate([{ opacity: 1 }, { opacity: 0 }], { duration: 360, delay: 200, easing: EASE, fill: 'forwards' }));
    win.animate([{ opacity: 0, transform: 'translateY(14px) scale(.985)' }, { opacity: 1, transform: 'none' }], { duration: 560, easing: EASE }).finished.then(done, done);
  };

  // Fades the struck words, removes each one with one space, and slides the words that stay into
  // place. offsetLeft and offsetTop are in the desktop's own points, so the slide is right at any zoom.
  // The box around the words also eases to its new height, so a line that goes away does not jump the
  // thread or the feed around it.
  const collapse = async (field, gone) => {
    gone.forEach(w => w.animate([{ opacity: 1 }, { opacity: 0 }], { duration: 170, easing: EASE, fill: 'forwards' }));
    await beat(180);
    const blk = getComputedStyle(field).display === 'inline' ? field.parentElement : field, h0 = blk.offsetHeight;
    const keep = [...field.children].filter(c => !gone.includes(c));
    const at = keep.map(c => [c.offsetLeft, c.offsetTop]);
    gone.forEach(w => {
      const n = w.nextSibling, p = w.previousSibling;
      const sp = n && n.nodeType === 3 ? n : p && p.nodeType === 3 ? p : null;
      if (sp) sp.remove();
      w.remove();
    });
    keep.forEach((c, i) => {
      const dx = at[i][0] - c.offsetLeft, dy = at[i][1] - c.offsetTop;
      if (dx || dy) c.animate([{ transform: `translate(${dx}px, ${dy}px)` }, { transform: 'none' }], { duration: 360, easing: EASE });
    });
    const h1 = blk.offsetHeight;
    if (h1 !== h0) blk.animate([{ height: h0 + 'px', overflow: 'hidden' }, { height: h1 + 'px', overflow: 'hidden' }], { duration: 360, easing: EASE });
    await beat(380);
  };

  // Option + Tab, the hands-free shortcut: the caps come up above the flow bar, go down together as the
  // flow bar changes, let go, and fade
  const press = async state => {
    keys.classList.add('on');
    await beat(220);
    keys.classList.add('dn');
    fb.set(state).show();
    await beat(180);
    keys.classList.remove('dn');
    await beat(300);
    keys.classList.remove('on');
  };

  const story = async (spec, w) => {
    const field = w.querySelector(spec.field);
    if (!field) throw new Error(`Hero window "${spec.name}": no field ${spec.field}`);
    const raw = spec.raw.split(' '), gone = new Set(spec.gone || []);
    fb.set('listening');
    // The close shot holds the text box, the key caps, and the flow bar under them. A small stage holds
    // only the heard text, the longest the field gets, so the words stay large enough to read.
    const zone = !narrow() && spec.zone && w.querySelector(spec.zone);
    cam(mid(join(zone ? box(zone) : lines(field, spec.raw), box(keys), box(fb.el))), 1100);
    await beat(640);
    await press('listening');
    field.innerHTML = CARET;
    const caret = field.firstChild;
    for (let i = 0; i < raw.length; i++) {
      const s = document.createElement('span');
      s.className = 'w';
      s.textContent = raw[i];
      if (i) caret.before(' ');
      caret.before(s);
      await beat(96 + raw[i].length * 5 + (/[,.]$/.test(raw[i]) ? 200 : 0));
    }
    await beat(240);
    await press('transcribing');
    const ws = [...field.querySelectorAll('.w')];
    for (const i of [...gone].sort((a, b) => a - b)) {
      if (ws[i]) ws[i].classList.add('x');
      await beat(130);
    }
    await beat(380);
    await collapse(field, ws.filter((_, i) => gone.has(i)));
    fb.set('undo');
    field.innerHTML = esc(spec.final) + CARET;
    field.classList.add('done');
    await beat(1100);
    // Back to the window, or on a small stage to the part of it where the app answers. A wide stage leaves
    // the flow bar below the frame here; a square one still shows it, so it stays in the middle.
    const part = narrow() && spec.shot && w.querySelector(spec.shot), back = part ? box(part) : box(w);
    cam(st.clientHeight > st.clientWidth * .75 ? mid(back) : back, 1100);
    await beat(700);
    let hold;
    if (spec.after) {
      try { hold = await spec.after(w, beat); } catch (e) { console.error(e); }
    }
    fb.hide();
    await beat(typeof hold === 'number' ? hold : 1300);
  };

  new IntersectionObserver(([e]) => { visible = e.isIntersecting; }, { threshold: 0.15 }).observe(st);

  show(0, false);
  if (calm) {
    const spec = list[0], f = win.querySelector(spec.field);
    fb.set('undo').show();
    if (f) { f.textContent = spec.final; f.classList.add('done'); }
    if (f) requestAnimationFrame(() => cam(mid(join(lines(f, spec.final), box(fb.el))), 0));
    return;
  }
  frame(0);
  (async () => {
    for (let n = 0; ; n++) {
      const i = n % list.length;
      if (n) {
        cam(null, 1000);
        await beat(360);
        show(i, true);
      }
      await beat(n ? 760 : 900);
      await story(list[i], win);
    }
  })().catch(e => console.error(e));
})();
