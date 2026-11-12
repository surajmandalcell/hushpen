import assert from 'node:assert/strict';
import { afterEach, test } from 'node:test';
import worker, { buildReleases } from '../src/index.js';

const originalFetch = globalThis.fetch;
afterEach(() => { globalThis.fetch = originalFetch; delete globalThis.caches; });
const hash = 'a'.repeat(64);
const asset = name => ({ name, browser_download_url: `https://github.com/download/${name}`, size: 1234 });
function fixture(tag, prerelease = false, windows = false) {
  return {
    tag_name: tag, prerelease, draft: false, published_at: prerelease ? '2026-11-09T00:00:00Z' : '2026-11-08T00:00:00Z',
    html_url: `https://github.com/releases/tag/${tag}`, body: 'Intro\n\n## Highlights\n\n- A release highlight.\n## Fixes\nOther text',
    assets: [asset(`Hushpen-${tag.slice(1)}-macos-arm64.dmg`), ...(windows ? [asset(`Hushpen-${tag.slice(1)}-windows-x64-setup.exe`)] : []), asset('SHA256SUMS')],
  };
}
function github(stable, list, missing = false) {
  const calls = [];
  globalThis.fetch = async (url, options) => {
    calls.push(url);
    assert.equal(options.headers['User-Agent'], 'Hushpen-site');
    if (url.endsWith('/latest')) return Response.json(stable);
    if (url.endsWith('?per_page=20')) return Response.json(list);
    return new Response(missing ? '' : [stable, ...list].flatMap(item => item.assets.filter(a => a.name !== 'SHA256SUMS').map(a => `${hash}  ${a.name}`)).filter((line, i, all) => all.indexOf(line) === i).join('\n'));
  };
  return calls;
}
function cacheMock() {
  const values = new Map();
  globalThis.caches = { default: {
    async match(request) { return values.get(request.url)?.clone(); },
    async put(request, response) { values.set(request.url, response); },
  } };
  return values;
}
const request = () => new Request('https://hushpen.example/api/releases.json');
const context = () => {
  const waits = [];
  return { waits, waitUntil(promise) { waits.push(promise); } };
};

test('stable and Windows preview remain separate, with checksums and Highlights', async () => {
  const stable = fixture('v2.4.0'), preview = fixture('v2.5.0-rc.2', true, true);
  const draft = { ...fixture('v9.0.0', true, true), draft: true, published_at: '2027-01-01T00:00:00Z' };
  github(stable, [draft, stable, preview]);
  const data = await buildReleases();
  assert.equal(data.stable.tag, 'v2.4.0');
  assert.equal(data.windows.tag, 'v2.5.0-rc.2');
  assert.equal(data.windows.prerelease, true);
  assert.equal(data.stable.highlight, '- A release highlight.');
  assert.equal(data.windows.assets['windows-setup'].sha256, hash);
  assert.deepEqual(Object.keys(data.stable.assets), ['macos-dmg']);
});

test('stable Windows assets take priority and no Windows release returns null', async () => {
  const stable = fixture('v2.4.0', false, true);
  github(stable, [stable, fixture('v2.5.0-rc.2', true, true)]);
  assert.equal((await buildReleases()).windows.prerelease, false);
  const without = fixture('v2.4.0');
  without.body = '## Fixes\nA fix';
  github(without, [without]);
  const data = await buildReleases();
  assert.equal(data.windows, null);
  assert.equal(data.stable.highlight, null);
});

test('stable and Windows checksum downloads start together', async () => {
  const stable = fixture('v2.4.0'), preview = fixture('v2.5.0-rc.2', true, true);
  github(stable, [preview, stable]);
  const mock = globalThis.fetch;
  let count = 0, releaseDownloads, downloadsStarted;
  const gate = new Promise(resolve => { releaseDownloads = resolve; });
  const started = new Promise(resolve => { downloadsStarted = resolve; });
  globalThis.fetch = async (url, options) => {
    if (url.endsWith('SHA256SUMS')) {
      if (++count === 2) downloadsStarted();
      await gate;
    }
    return mock(url, options);
  };
  let timer;
  const build = buildReleases();
  try {
    await Promise.race([started, new Promise((_, reject) => { timer = setTimeout(() => reject(new Error('Checksum downloads were sequential')), 1000); })]);
  } finally {
    clearTimeout(timer);
    releaseDownloads();
    await build;
  }
});

test('missing and duplicate checksum entries reject the new release', async () => {
  const stable = fixture('v2.4.0');
  github(stable, [stable], true);
  await assert.rejects(buildReleases(), /Missing checksum/);
  const mock = globalThis.fetch;
  globalThis.fetch = async (url, options) => url.endsWith('SHA256SUMS')
    ? new Response(`${hash}  ${stable.assets[0].name}\n${hash}  ${stable.assets[0].name}`) : mock(url, options);
  await assert.rejects(buildReleases(), /Duplicate checksum/);
});

test('fresh cache lasts ten minutes, recovery copy seven days, clients five minutes', async () => {
  const stable = fixture('v2.4.0');
  const calls = github(stable, [stable]);
  const values = cacheMock(), ctx = context();
  const response = await worker.fetch(request(), {}, ctx);
  await Promise.all(ctx.waits);
  assert.equal(response.status, 200);
  assert.equal(response.headers.get('cache-control'), 'public, max-age=300');
  assert.equal(response.headers.get('access-control-allow-origin'), null);
  assert.equal(values.get('https://hushpen.example/api/releases.json?cache=fresh').headers.get('cache-control'), 'public, max-age=600');
  assert.equal(values.get('https://hushpen.example/api/releases.json?cache=last-good').headers.get('cache-control'), 'public, max-age=604800');
  const count = calls.length;
  await worker.fetch(request(), {}, context());
  assert.equal(calls.length, count);
  values.delete('https://hushpen.example/api/releases.json?cache=fresh');
  globalThis.fetch = async () => new Response('Rate limited', { status: 403 });
  const stale = await worker.fetch(request(), {}, context());
  assert.equal(stale.status, 200);
  assert.equal(stale.headers.get('x-hushpen-stale'), '1');
  assert.equal((await stale.json()).stable.tag, 'v2.4.0');
  values.clear();
  const down = await worker.fetch(request(), {}, context());
  assert.equal(down.status, 503);
  assert.equal(down.headers.get('cache-control'), 'no-store');
});

test('a cold data center serves the shared copy when GitHub fails', async () => {
  const stable = fixture('v2.4.0');
  github(stable, [stable]);
  const store = new Map();
  const env = { RELEASES: { async get(key) { return store.get(key) ?? null; }, async put(key, value) { store.set(key, value); } } };
  const values = cacheMock(), ctx = context();
  await worker.fetch(request(), env, ctx);
  await Promise.all(ctx.waits);
  assert.ok(store.get('last-good'));
  values.clear();
  globalThis.fetch = async () => new Response('Rate limited', { status: 403 });
  const shared = await worker.fetch(request(), env, context());
  assert.equal(shared.status, 200);
  assert.equal(shared.headers.get('x-hushpen-stale'), '1');
  assert.equal((await shared.json()).stable.tag, 'v2.4.0');
});

test('all other routes and methods go through the asset binding', async () => {
  const env = { ASSETS: { fetch: async request => new Response(`${request.method} ${new URL(request.url).pathname}`) } };
  for (const [path, method] of [['/about', 'GET'], ['/api/releases.json', 'POST'], ['/api/other', 'GET']]) {
    const response = await worker.fetch(new Request(`https://hushpen.example${path}`, { method }), env, {});
    assert.equal(await response.text(), `${method} ${path}`);
  }
});

test('host tarballs do not keep a new stable release on the stale feed', async () => {
  const stable = fixture('v2.5.0'), preview = fixture('v2.5.1-rc.1', true, true);
  for (const release of [stable, preview]) {
    for (const arch of ['x64', 'arm64']) {
      const version = release.tag_name.slice(1);
      release.assets.push(asset(`Hushpen-${version}-linux-${arch}.tar.gz`));
      release.assets.push(asset(`Hushpen-${version}-host-linux-${arch}.tar.gz`));
    }
  }
  github(stable, [stable, preview]);
  const values = cacheMock(), ctx = context();
  values.set('https://hushpen.example/api/releases.json?cache=last-good',
    Response.json({ stable: { tag: 'v2.4.0' } }));
  const response = await worker.fetch(request(), {}, ctx);
  await Promise.all(ctx.waits);
  const data = await response.json();
  assert.equal(data.stable.tag, 'v2.5.0');
  assert.equal(response.headers.get('x-hushpen-stale'), null);
  for (const [release, tag] of [[data.stable, stable.tag_name], [data.windows, preview.tag_name]]) {
    for (const arch of ['x64', 'arm64']) {
      assert.equal(release.assets[`linux-${arch}-tar`].name, `Hushpen-${tag.slice(1)}-linux-${arch}.tar.gz`);
      assert.equal(release.assets[`linux-${arch}-tar`].sha256, hash);
    }
    assert.ok(Object.values(release.assets).every(item => !item.name.includes('-host-linux-')));
  }
});
