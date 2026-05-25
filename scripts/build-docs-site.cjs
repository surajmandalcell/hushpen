const { readFileSync, writeFileSync } = require("node:fs");
const path = require("node:path");
const { spawnSync } = require("node:child_process");

const repoRoot = path.resolve(__dirname, "..");
const viteBin = path.join(repoRoot, "node_modules", "vite", "bin", "vite.js");

const build = spawnSync(process.execPath, [viteBin, "build", "--config", "vite.docs.config.ts"], {
  cwd: repoRoot,
  stdio: "inherit",
});

if (build.status !== 0) {
  process.exit(build.status ?? 1);
}

const docsDir = path.join(repoRoot, "docs");
const indexPath = path.join(docsDir, "index.html");
let html = readFileSync(indexPath, "utf8");

const scriptMatch = html.match(/<script type="module" crossorigin src="(.+?)"><\/script>/);
const styleMatch = html.match(/<link rel="stylesheet" crossorigin href="(.+?)">/);

if (!scriptMatch || !styleMatch) {
  throw new Error("Docs build output did not contain the expected script and stylesheet tags.");
}

const scriptPath = path.join(docsDir, scriptMatch[1]);
const stylePath = path.join(docsDir, styleMatch[1]);
const scriptSource = readFileSync(scriptPath, "utf8")
  .replace(/<\/script/gi, "<\\/script")
  .replace(/<!--/g, "<\\!--");
const styleSource = readFileSync(stylePath, "utf8");

html = html.replace(styleMatch[0], () => `<style>\n${styleSource}\n</style>`);
html = html.replace(scriptMatch[0], () => `<script type="module">\n${scriptSource}\n</script>`);

writeFileSync(indexPath, html);
