import { describe, expect, it } from "vitest";
import { evaluateDeclarations, extractDeclaration, readElectronSources } from "../helpers/electronSources";

describe("readElectronSources", () => {
  it("reads every main-process source file", () => {
    const sources = readElectronSources();

    expect(sources.main).toContain('require("electron")');
    expect(sources.preload).toContain("contextBridge");
    expect(sources.overlayPreload).toContain("contextBridge");
    expect(sources.identity).toContain("com.surajmandal.asrpro");
    expect(sources.runtime).toContain("resolveContainedDataDir");
    expect(sources.whisperEngine).toContain("whisper");
  });
});

describe("extractDeclaration", () => {
  it("extracts a function whose parameters carry a default object", () => {
    const source = "function sample(value, options = {}) {\n  return options;\n}\n";

    expect(extractDeclaration(source, "sample")).toBe(source.trim());
  });

  it("extracts a multi-line const declaration", () => {
    const source = "const sample = Object.freeze({\n  a: 1,\n});\nconst other = 2;\n";

    expect(extractDeclaration(source, "sample")).toBe("const sample = Object.freeze({\n  a: 1,\n});");
  });

  it("rejects an unknown declaration", () => {
    expect(() => extractDeclaration("const other = 1;", "missing")).toThrow(/missing/);
  });
});

describe("evaluateDeclarations", () => {
  it("evaluates declarations with injected dependencies", () => {
    const { double } = evaluateDeclarations<{ double: (value: number) => number }>(
      ["function double(value) { return value * factor; }"],
      { factor: 2 },
      ["double"],
    );

    expect(double(21)).toBe(42);
  });
});
