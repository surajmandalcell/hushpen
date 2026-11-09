// One request and one store for the page and its animated release views.
(() => {
  const GH = 'https://github.com/surajmandalcell/hushpen/releases';
  const listeners = [];
  let data = null;
  const size = bytes => bytes ? `${(bytes / 1000000).toFixed(1).replace(/\.0$/, '')} MB` : '';
  const fallback = windows => GH + (windows ? '' : '/latest');
  const subscribe = fn => { listeners.push(fn); if (data) fn(data); };
  window.HushpenReleases = { size, fallback, subscribe };
  subscribe(value => {
    document.querySelectorAll('[data-asset]').forEach(el => {
      const release = el.dataset.asset.startsWith('windows-') ? value.windows : value.stable;
      const asset = release?.assets[el.dataset.asset];
      if (asset) el.href = asset.url;
    });
    document.querySelectorAll('[data-version]').forEach(el => {
      const release = el.dataset.version === 'windows' ? value.windows : value.stable;
      el.textContent = release?.tag || 'Latest version';
    });
    document.querySelectorAll('[data-asset-meta]').forEach(el => {
      const release = el.dataset.assetMeta.startsWith('windows-') ? value.windows : value.stable;
      const asset = release?.assets[el.dataset.assetMeta];
      el.textContent = asset ? `${release.tag} · ${size(asset.size)}` : 'Latest version';
    });
    document.querySelectorAll('[data-preview]').forEach(el => { el.hidden = !value.windows?.prerelease; });
    document.querySelectorAll('[data-notes]').forEach(el => { el.href = value.stable.notesUrl; });
    document.querySelectorAll('[data-release-note]').forEach(el => {
      el.textContent = value.stable.highlight || '';
      el.hidden = !value.stable.highlight;
    });
  });
  document.addEventListener('alpine:init', () => {
    Alpine.store('releases', { stable: null, windows: null, fetchedAt: null });
    fetch('/api/releases.json', { signal: AbortSignal.timeout(45000) })
      .then(response => { if (!response.ok) throw new Error('Release data unavailable'); return response.json(); })
      .then(value => {
        if (!value.stable?.assets || !value.stable.notesUrl) throw new Error('Invalid release data');
        Object.assign(Alpine.store('releases'), value);
        data = Alpine.store('releases');
        listeners.forEach(fn => fn(data));
      })
      .catch(() => {}); // HTML keeps working release-page links when the API is unavailable.
  });
})();
