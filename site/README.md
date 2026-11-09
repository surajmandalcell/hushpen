# Hushpen website
The public folder is the site source; no build or dependencies are needed.
Static preview: `python3 -m http.server 4821 --bind 127.0.0.1 --directory site/public`
Open `http://127.0.0.1:4821/`; downloads fall back to GitHub without the API.
Full preview: `cd site/worker && npx wrangler dev --port 4822`
Open `http://127.0.0.1:4822/` for live release data and static assets.
Run Worker checks: `node --test site/worker/test/releases.test.mjs`
Deploy: `cd site/worker && CLOUDFLARE_ACCOUNT_ID=5dadb33ff9ed908a878e1564bfb89737 npx wrangler deploy`
Live site: <https://hushpen.mandalsuraj.com/>.
Release data caches for ten minutes; GitHub errors use the last good copy for up to seven days.
