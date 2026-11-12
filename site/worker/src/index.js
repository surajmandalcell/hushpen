const API = 'https://api.github.com/repos/surajmandalcell/hushpen/releases';
const HEADERS = { 'User-Agent': 'Hushpen-site', Accept: 'application/vnd.github+json' };
const KINDS = [
  ['macos-dmg', /-macos-arm64\.dmg$/], ['macos-zip', /-macos-arm64\.zip$/],
  ...['x64', 'arm64'].flatMap(arch => [
    [`linux-${arch}-appimage`, new RegExp(`-linux-${arch}\\.AppImage$`)],
    [`linux-${arch}-deb`, new RegExp(`-linux-${arch}\\.deb$`)],
    [`linux-${arch}-rpm`, new RegExp(`-linux-${arch}\\.rpm$`)],
    [`linux-${arch}-tar`, new RegExp(`-linux-${arch}\\.tar\\.gz$`)],
  ]),
  ['windows-setup', /-windows-x64-setup\.exe$/], ['windows-zip', /-windows-x64\.zip$/],
];

async function get(url, json = true) {
  try {
    const response = await fetch(url, { headers: HEADERS, signal: AbortSignal.timeout(30000) });
    if (!response.ok) throw new Error(`GitHub returned ${response.status}`);
    return json ? response.json() : response.text();
  } catch (error) {
    throw new Error(`Could not fetch ${url}: ${error.message}`, { cause: error });
  }
}

async function release(data) {
  const sumsAsset = data.assets.find(asset => asset.name === 'SHA256SUMS');
  if (!sumsAsset) throw new Error('Release has no SHA256SUMS');
  const sums = new Map();
  for (const line of (await get(sumsAsset.browser_download_url, false)).split(/\r?\n/)) {
    const match = line.match(/^([a-fA-F0-9]{64})[ \t]+\*?(.+)$/);
    if (!match) continue;
    if (sums.has(match[2])) throw new Error('Duplicate checksum');
    sums.set(match[2], match[1].toLowerCase());
  }
  const assets = {};
  for (const [kind, pattern] of KINDS) {
    const matches = data.assets.filter(asset => !asset.name.includes('-host-linux-') && pattern.test(asset.name));
    if (matches.length > 1) throw new Error(`Duplicate ${kind} asset`);
    const asset = matches[0];
    if (!asset) continue;
    const sha256 = sums.get(asset.name);
    if (!sha256) throw new Error(`Missing checksum for ${asset.name}`);
    assets[kind] = { name: asset.name, url: asset.browser_download_url, size: asset.size, sha256 };
  }
  const lines = (data.body || '').split(/\r?\n/);
  const heading = lines.findIndex(line => /^##\s+Highlights\s*#*\s*$/i.test(line.trim()));
  const first = heading < 0 ? '' : lines.slice(heading + 1).find(line => line.trim());
  const highlight = first && !/^#/.test(first.trim()) ? first.trim() : null;
  return {
    tag: data.tag_name, version: data.tag_name.replace(/^v/, ''), prerelease: data.prerelease,
    publishedAt: data.published_at, notesUrl: data.html_url, highlight, assets,
  };
}

export async function buildReleases() {
  const list = await get(`${API}?per_page=20`);
  const newest = (a, b) => Date.parse(b.published_at) - Date.parse(a.published_at);
  const latest = list.filter(item => !item.draft && !item.prerelease).sort(newest)[0];
  if (!latest) throw new Error('No stable release');
  const hasWindows = item => item.assets.some(asset => /-windows-x64-setup\.exe$/.test(asset.name));
  const windows = hasWindows(latest) ? latest : list.filter(item => !item.draft && hasWindows(item))
    .sort(newest)[0];
  const stablePromise = release(latest);
  const [stable, windowsRelease] = await Promise.all([
    stablePromise, windows ? (windows.tag_name === latest.tag_name ? stablePromise : release(windows)) : null,
  ]);
  return { stable, windows: windowsRelease, fetchedAt: new Date().toISOString() };
}

function outward(response, stale = false) {
  const headers = new Headers(response.headers);
  headers.set('cache-control', 'public, max-age=300');
  if (stale) headers.set('x-hushpen-stale', '1');
  return new Response(response.body, { status: response.status, headers });
}

export default {
  async fetch(request, env, ctx) {
    const url = new URL(request.url);
    if (request.method !== 'GET' || url.pathname !== '/api/releases.json') return env.ASSETS.fetch(request);
    const cache = caches.default;
    const freshKey = new Request(`${url.origin}/api/releases.json?cache=fresh`);
    const goodKey = new Request(`${url.origin}/api/releases.json?cache=last-good`);
    const fresh = await cache.match(freshKey);
    if (fresh) return outward(fresh);
    try {
      const body = JSON.stringify(await buildReleases());
      const response = ttl => new Response(body, { headers: { 'content-type': 'application/json; charset=utf-8', 'cache-control': `public, max-age=${ttl}` } });
      ctx.waitUntil(Promise.all([
        cache.put(freshKey, response(600)), cache.put(goodKey, response(604800)), env.RELEASES?.put('last-good', body),
      ]));
      return outward(response(600));
    } catch (error) {
      console.error('Release refresh failed:', error);
      const lastGood = await cache.match(goodKey);
      if (lastGood) return outward(lastGood, true);
      // The Cache API is per data center, so a cold one falls back to the copy every data center shares.
      const shared = await env.RELEASES?.get('last-good');
      if (shared) {
        return outward(new Response(shared, { headers: { 'content-type': 'application/json; charset=utf-8' } }), true);
      }
      return new Response(JSON.stringify({ error: 'Release data is temporarily unavailable.' }), {
        status: 503, headers: { 'content-type': 'application/json; charset=utf-8', 'cache-control': 'no-store' },
      });
    }
  },
};
