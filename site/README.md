# Hushpen website
The public folder is the site source; no build or dependencies are needed.
Static preview: `python3 site/preview.py 4821` (serves `/benchmarks` from `benchmarks.html`, like the live site)
Open `http://127.0.0.1:4821/`; downloads fall back to GitHub without the API.
Full preview: `cd site/worker && npx wrangler dev --port 4822`
Open `http://127.0.0.1:4822/` for live release data and static assets.
Run Worker checks: `node --test site/worker/test/releases.test.mjs`
Deploy: commit, then run the samhq website deploy (`samhq_backend/web/deploy.zsh`). It ships the committed `site/public/` and runs `site/worker/src/index.js` for `/api/releases.json`.
Live site: <https://hushpen.samhq.app/>.
Release data caches for ten minutes; GitHub errors use the last good copy for up to seven days.
