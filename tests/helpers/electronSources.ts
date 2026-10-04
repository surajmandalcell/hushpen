import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

export interface ElectronSources {
  main: string;
  preload: string;
  overlayPreload: string;
  identity: string;
  runtime: string;
  whisperEngine: string;
}

const electronDir = fileURLToPath(new URL("../../electron/", import.meta.url));

export function readElectronSources(): ElectronSources {
  const read = (fileName: string) => readFileSync(new URL(`../../electron/${fileName}`, import.meta.url), "utf8");

  return {
    main: read("main.cjs"),
    preload: read("preload.cjs"),
    overlayPreload: read("overlay-preload.cjs"),
    identity: read("identity.cjs"),
    runtime: read("runtime.cjs"),
    whisperEngine: read("whisper-engine.cjs"),
  };
}

function findMatchingDelimiter(source: string, openIndex: number, open: string, close: string) {
  let depth = 0;
  for (let index = openIndex; index < source.length; index += 1) {
    if (source[index] === open) depth += 1;
    if (source[index] === close) {
      depth -= 1;
      if (depth === 0) return index;
    }
  }
  throw new Error(`Unbalanced ${open}${close} in electron source at index ${openIndex}`);
}

/**
 * Extracts one top-level declaration (`function name(...)` or
 * `const name = ...`) from a CommonJS source file, so characterization tests
 * can evaluate a helper without loading the Electron app bootstrap.
 */
export function extractDeclaration(source: string, name: string): string {
  const functionStart = source.indexOf(`function ${name}(`);
  const constStart = source.indexOf(`const ${name} =`);
  const isFunction = functionStart >= 0 && (constStart < 0 || functionStart < constStart);
  const start = isFunction ? functionStart : constStart;

  if (start < 0) {
    throw new Error(`Declaration ${name} not found in ${electronDir} source`);
  }

  if (isFunction) {
    const paramsStart = source.indexOf("(", start);
    const bodyStart = source.indexOf("{", findMatchingDelimiter(source, paramsStart, "(", ")"));
    return source.slice(start, findMatchingDelimiter(source, bodyStart, "{", "}") + 1);
  }

  const valueStart = source.indexOf("=", start);
  let depth = 0;
  for (let index = valueStart; index < source.length; index += 1) {
    const character = source[index];
    if (character === "{" || character === "[" || character === "(") depth += 1;
    if (character === "}" || character === "]" || character === ")") depth -= 1;
    if (depth === 0 && character === ";") {
      return source.slice(start, index + 1);
    }
  }
  throw new Error(`Declaration ${name} body never closes`);
}

/**
 * Evaluates extracted declarations with explicit injected dependencies and
 * returns them as an object. Only for pure helpers pinned by characterization
 * tests.
 */
export function evaluateDeclarations<T extends Record<string, unknown>>(
  declarations: string[],
  scope: Record<string, unknown>,
  names: string[],
): T {
  const factory = new Function(
    ...Object.keys(scope),
    `${declarations.join("\n")}\nreturn { ${names.join(", ")} };`,
  );

  return factory(...Object.values(scope)) as T;
}
