// ?os=mac|linux-x64|linux-arm64|windows forces a download target for testing.
(() => {
  const { size, fallback, subscribe } = window.HushpenReleases;
  const mac = { kind: 'macos-dmg', name: 'Mac', sub: 'Free · Apple silicon', icon: '#i-apple', req: ['macOS 13 or later'], toast: 'Open it from Downloads, then drag Hushpen to Applications.' };
  const linux = arch => ({ kind: `linux-${arch}-appimage`, name: 'Linux', sub: `Free · AppImage · ${arch}`, icon: '#i-tux', req: [`Linux ${arch}`, 'X11 desktop'], arch, toast: 'Make it executable, then double-click it.' });
  const systems = { mac, 'linux-x64': linux('x64'), 'linux-arm64': linux('arm64'), windows: { kind: 'windows-setup', name: 'Windows', sub: 'Free · x64 · Per-user setup', icon: '#i-windows', req: ['Windows x64'], toast: 'Open the setup file from Downloads.' }, other: { name: 'your computer', sub: 'Free · Mac, Linux and Windows', icon: '#i-dl', req: [] } };
  const aliases = { mac: 'mac', macos: 'mac', linux: 'linux-x64', 'linux-x64': 'linux-x64', 'linux-arm64': 'linux-arm64', windows: 'windows', other: 'other' };
  const ua = navigator.userAgent, uad = navigator.userAgentData, platform = uad?.platform || navigator.platform || '';
  const forced = aliases[(new URLSearchParams(location.search).get('os') || '').toLowerCase()];
  const guess = () => {
    if (/Android|iPhone|iPad|CrOS/.test(ua) || /Android|Chrome OS/.test(platform)) return 'other';
    if (/Win/i.test(platform) || /Windows/.test(ua)) return 'windows';
    if (/Mac/i.test(platform) || /Macintosh/.test(ua)) return navigator.maxTouchPoints > 1 ? 'other' : 'mac';
    if (/Linux|X11/.test(platform + ua)) return /aarch64|\barm/i.test(ua) ? 'linux-arm64' : 'linux-x64';
    return 'other';
  };
  let current = forced || guess(), releases = null;
  const apply = () => {
    const system = systems[current], release = current === 'windows' ? releases?.windows : releases?.stable;
    const asset = release?.assets[system.kind];
    document.documentElement.dataset.os = current;
    document.querySelectorAll('[data-dl]').forEach(el => {
      el.href = asset?.url || fallback(current === 'windows' || current === 'other');
      el.setAttribute('aria-label', `Download Hushpen for ${system.name}`);
      el.querySelector('use:not([data-keep])')?.setAttribute('href', system.icon);
      const title = el.querySelector('[data-dl-t]'), sub = el.querySelector('[data-dl-s]');
      if (title) title.textContent = system.kind ? `Download for ${system.name}` : 'Download';
      if (sub) sub.textContent = system.sub;
    });
    document.querySelectorAll('[data-dl-meta]').forEach(el => {
      el.replaceChildren();
      const labels = [asset ? release.tag : 'Latest version', asset ? size(asset.size) : '', ...system.req].filter(Boolean);
      if (asset && release.prerelease) labels.splice(1, 0, 'Preview');
      labels.forEach(label => { const li = document.createElement('li'); li.textContent = label; el.append(li); });
      const li = document.createElement('li'), link = document.createElement('a');
      link.href = fallback(true); link.textContent = 'Other downloads'; li.append(link); el.append(li);
    });
    const arch = system.arch || 'x64', image = releases?.stable.assets[`linux-${arch}-appimage`];
    const download = document.querySelector('[data-linux-download]');
    if (download) {
      download.href = image?.url || fallback();
      download.querySelector('small').textContent = image ? `${releases.stable.tag} · ${arch} · ${size(image.size)}` : 'Latest version';
    }
    const toast = document.querySelector('.dtoast');
    if (toast) {
      toast.querySelector('b').textContent = `Downloading Hushpen${asset ? ' ' + release.version : ''}`;
      toast.querySelector('span').textContent = [asset && size(asset.size), system.toast].filter(Boolean).join('. ');
    }
  };
  apply();
  subscribe(value => { releases = value; apply(); });
  if (!forced && current.startsWith('linux') && uad?.getHighEntropyValues) {
    uad.getHighEntropyValues(['architecture', 'bitness']).then(value => {
      if (value.architecture) { current = value.architecture === 'arm' ? 'linux-arm64' : 'linux-x64'; apply(); }
    }).catch(() => {});
  }
})();
