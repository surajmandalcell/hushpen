// Shared by every direction: sortable tables and one hover card per app name.
(() => {
  // Sort: <table data-sort> with <th aria-sort><button> headers; cells sort by data-v, else by text.
  const val = (c) => c ? (c.dataset.v !== undefined ? +c.dataset.v : c.textContent.trim().toLowerCase()) : '';
  function sortBy(table, i, dir) {
    const tb = table.tBodies[0];
    const rows = [...tb.rows];
    rows.sort((a, b) => {
      const x = val(a.cells[i]), y = val(b.cells[i]);
      const ex = x === '' || Number.isNaN(x), ey = y === '' || Number.isNaN(y);
      if (ex !== ey) return ex ? 1 : -1; // gaps always last
      return (x > y ? 1 : x < y ? -1 : 0) * (dir === 'ascending' ? 1 : -1);
    });
    rows.forEach((r) => tb.appendChild(r));
    if (tb.querySelector(`td:nth-child(${i + 1}) .bx-bar`)) table.dataset.focus = i; // the bar follows the sort
    const seg = document.querySelector(`[data-seg="${table.id}"]`);
    if (seg && table.id) seg.querySelectorAll('button').forEach((x) => x.setAttribute('aria-pressed', +x.dataset.col === i));
    table.querySelectorAll('thead th').forEach((th, j) => {
      if (th.querySelector('button')) th.setAttribute('aria-sort', j === i ? dir : 'none');
    });
  }
  document.querySelectorAll('table[data-sort]').forEach((t) => {
    t.querySelectorAll('thead th').forEach((th, i) => {
      const b = th.querySelector('button');
      if (!b) return;
      b.addEventListener('click', () => {
        const cur = th.getAttribute('aria-sort');
        const best = th.dataset.best || 'descending'; // first click sorts best first
        const flip = best === 'ascending' ? 'descending' : 'ascending';
        sortBy(t, i, cur === best ? flip : best);
      });
    });
  });
  // Segmented control that re-sorts a table: <div data-seg="tableId"><button data-col="2">
  document.querySelectorAll('[data-seg]').forEach((seg) => {
    const t = document.getElementById(seg.dataset.seg);
    seg.querySelectorAll('button').forEach((b) => b.addEventListener('click', () => {
      const i = +b.dataset.col, th = t.tHead.rows[0].cells[i];
      sortBy(t, i, th.dataset.best || 'descending');
    }));
  });

  // App hover card: every [data-app] shows the same card from window.BX_APPS.
  const A = window.BX_APPS || {};
  const card = document.createElement('div');
  card.className = 'bx-card'; card.setAttribute('role', 'tooltip'); card.id = 'bx-card'; card.hidden = true;
  document.body.appendChild(card);
  let timer, cur = null;
  function show(el) {
    clearTimeout(timer); cur = el;
    const a = A[el.dataset.app]; if (!a) return;
    card.innerHTML = `<b>${a.name}</b><p>${a.what}</p><dl>${Object.entries(a.rows).map(([k, v]) => `<dt>${k}</dt><dd>${v}</dd>`).join('')}</dl>`;
    card.hidden = false;
    const r = el.getBoundingClientRect(), w = card.offsetWidth, h = card.offsetHeight;
    let x = Math.min(Math.max(12, r.left), innerWidth - w - 12), y = r.bottom + 8;
    if (y + h > innerHeight - 8) y = r.top - h - 8;
    card.style.left = x + scrollX + 'px'; card.style.top = y + scrollY + 'px';
    el.setAttribute('aria-describedby', 'bx-card');
  }
  const hide = () => { clearTimeout(timer); card.hidden = true; cur = null; };
  document.querySelectorAll('[data-app]').forEach((el) => {
    // Mouse: hover. Touch: tap opens, tap outside closes. Keyboard: focus.
    el.addEventListener('pointerenter', (ev) => { if (ev.pointerType === 'mouse') { clearTimeout(timer); timer = setTimeout(() => show(el), 120); } });
    el.addEventListener('pointerleave', (ev) => { if (ev.pointerType === 'mouse') hide(); });
    el.addEventListener('focus', () => { if (el.matches(':focus-visible')) show(el); });
    el.addEventListener('blur', () => { if (cur === el) hide(); });
    el.addEventListener('click', (ev) => {
      if (el.tagName !== 'BUTTON') return show(el); // a name inside a sort button: the click still sorts
      ev.preventDefault();
      cur === el && !card.hidden && ev.pointerType !== 'mouse' ? hide() : show(el);
    });
  });
  document.addEventListener('click', (ev) => { if (!ev.target.closest('[data-app]')) hide(); });
  addEventListener('keydown', (ev) => { if (ev.key === 'Escape') hide(); });
  addEventListener('scroll', hide, { passive: true });
})();
