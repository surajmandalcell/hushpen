import { build } from "vite";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, describe, expect, it } from "vitest";
import { CSP_POLICY, cspMetaPlugin } from "../../vite.config.ts";

let dir: string;
afterEach(() => rmSync(dir, { recursive: true, force: true }));

async function buildPage(html: string) {
  dir = mkdtempSync(join(tmpdir(), "asrpro-csp-"));
  writeFileSync(join(dir, "index.html"), html);
  writeFileSync(join(dir, "main.js"), "console.log('x');\n");
  await build({
    root: dir,
    base: "./",
    logLevel: "silent",
    configFile: false,
    plugins: [cspMetaPlugin()],
    build: { outDir: join(dir, "out"), emptyOutDir: true },
  });
  return readFileSync(join(dir, "out", "index.html"), "utf8");
}

const page = `<!doctype html>
<html><head>
<meta charset="UTF-8" />
<title>t</title>
</head><body><script type="module" src="/main.js"></script></body></html>`;

describe("CSP meta tag", () => {
  it("is injected right after the charset tag in a production build", async () => {
    const html = await buildPage(page);
    const tag = `<meta http-equiv="Content-Security-Policy" content="${CSP_POLICY}" />`;

    expect(html).toContain(tag);
    expect(html.indexOf("<meta charset")).toBeLessThan(html.indexOf(tag));
    expect(html.indexOf(tag)).toBeLessThan(html.indexOf("<title>"));
    expect(html.match(/Content-Security-Policy/g)).toHaveLength(1);
  });

  it("applies to builds only, so the dev server is unaffected", () => {
    expect(cspMetaPlugin().apply).toBe("build");
  });

  it("carries the strict policy of the security model", () => {
    const directives = Object.fromEntries(CSP_POLICY.split("; ").map((entry) => {
      const [name, ...sources] = entry.split(" ");
      return [name, sources];
    }));

    expect(directives["default-src"]).toEqual(["'self'"]);
    expect(directives["script-src"]).toEqual(["'self'"]);
    expect(directives["connect-src"]).toEqual(["'self'", "asrpro-media:"]);
    expect(directives["object-src"]).toEqual(["'none'"]);
    expect(directives["base-uri"]).toEqual(["'none'"]);
    expect(directives["form-action"]).toEqual(["'none'"]);
    expect(CSP_POLICY).not.toContain("unsafe-eval");
    expect(directives["script-src"]).not.toContain("'unsafe-inline'");
  });
});
