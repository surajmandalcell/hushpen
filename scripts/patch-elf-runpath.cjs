#!/usr/bin/env node
// Rewrites DT_RUNPATH / DT_RPATH of prebuilt Linux ELF binaries to "$ORIGIN".
//
// The upstream @kutalia/whisper-node-addon Linux binaries ship with a RUNPATH
// pointing at the CI runner build directory (/home/runner/work/...). That makes
// the dynamic loader unable to find libwhisper.so.1 / libggml*.so that sit right
// next to whisper.node. Patching in place (instead of requiring patchelf) keeps
// this working when Linux artifacts are cross-built from macOS or Windows.
//
// The replacement string is written over the existing one inside .dynstr and
// NUL padded, which is safe because "$ORIGIN" is never longer than the
// original path.

const fs = require("node:fs");
const path = require("node:path");

const ELF_MAGIC = Buffer.from([0x7f, 0x45, 0x4c, 0x46]);
const PT_LOAD = 1;
const PT_DYNAMIC = 2;
const DT_NULL = 0n;
const DT_STRTAB = 5n;
const DT_RPATH = 15n;
const DT_RUNPATH = 29n;
const ORIGIN_RUNPATH = "$ORIGIN";

function readElfHeader(buffer) {
  if (buffer.length < 64 || !buffer.subarray(0, 4).equals(ELF_MAGIC)) return null;
  const elfClass = buffer[4];
  const dataEncoding = buffer[5];
  if (elfClass !== 2 || dataEncoding !== 1) {
    throw new Error("Only 64-bit little-endian ELF binaries are supported.");
  }

  return {
    phoff: Number(buffer.readBigUInt64LE(0x20)),
    phentsize: buffer.readUInt16LE(0x36),
    phnum: buffer.readUInt16LE(0x38),
  };
}

function readProgramHeaders(buffer, header) {
  const headers = [];
  for (let index = 0; index < header.phnum; index += 1) {
    const base = header.phoff + index * header.phentsize;
    headers.push({
      type: buffer.readUInt32LE(base),
      offset: Number(buffer.readBigUInt64LE(base + 0x08)),
      vaddr: Number(buffer.readBigUInt64LE(base + 0x10)),
      filesz: Number(buffer.readBigUInt64LE(base + 0x20)),
      memsz: Number(buffer.readBigUInt64LE(base + 0x28)),
    });
  }
  return headers;
}

function vaddrToOffset(programHeaders, vaddr) {
  const segment = programHeaders.find((candidate) => (
    candidate.type === PT_LOAD && vaddr >= candidate.vaddr && vaddr < candidate.vaddr + candidate.memsz
  ));
  if (!segment) throw new Error(`Virtual address 0x${vaddr.toString(16)} is not mapped by a PT_LOAD segment.`);
  return vaddr - segment.vaddr + segment.offset;
}

function readCString(buffer, offset) {
  const end = buffer.indexOf(0, offset);
  if (end < 0) throw new Error("Unterminated string in ELF string table.");
  return { value: buffer.toString("latin1", offset, end), length: end - offset };
}

function findRunpathEntries(buffer) {
  const header = readElfHeader(buffer);
  if (!header) return null;

  const programHeaders = readProgramHeaders(buffer, header);
  const dynamic = programHeaders.find((candidate) => candidate.type === PT_DYNAMIC);
  if (!dynamic) return { entries: [], strtabOffset: 0 };

  let strtabVaddr = null;
  const pathEntries = [];
  for (let cursor = dynamic.offset; cursor + 16 <= dynamic.offset + dynamic.filesz; cursor += 16) {
    const tag = buffer.readBigInt64LE(cursor);
    const value = buffer.readBigUInt64LE(cursor + 8);
    if (tag === DT_NULL) break;
    if (tag === DT_STRTAB) strtabVaddr = Number(value);
    if (tag === DT_RUNPATH || tag === DT_RPATH) pathEntries.push({ tag, stringIndex: Number(value) });
  }

  if (strtabVaddr === null) return { entries: [], strtabOffset: 0 };
  const strtabOffset = vaddrToOffset(programHeaders, strtabVaddr);

  return {
    strtabOffset,
    entries: pathEntries.map((entry) => {
      const offset = strtabOffset + entry.stringIndex;
      return { ...entry, offset, ...readCString(buffer, offset) };
    }),
  };
}

function readRunpaths(filePath) {
  const parsed = findRunpathEntries(fs.readFileSync(filePath));
  return parsed ? parsed.entries.map((entry) => entry.value) : null;
}

function patchRunpath(filePath, nextRunpath = ORIGIN_RUNPATH) {
  const buffer = fs.readFileSync(filePath);
  const parsed = findRunpathEntries(buffer);
  if (!parsed) return { filePath, changed: false, reason: "not-elf" };

  let changed = false;
  for (const entry of parsed.entries) {
    if (entry.value === nextRunpath) continue;
    if (Buffer.byteLength(nextRunpath, "latin1") > entry.length) {
      throw new Error(`Cannot patch ${filePath}: replacement RUNPATH is longer than the original.`);
    }
    buffer.fill(0, entry.offset, entry.offset + entry.length);
    buffer.write(nextRunpath, entry.offset, "latin1");
    changed = true;
  }

  if (changed) fs.writeFileSync(filePath, buffer);
  return { filePath, changed, runpaths: parsed.entries.map((entry) => entry.value) };
}

function isLinuxNativeFile(fileName) {
  return fileName.endsWith(".node") || /\.so(\.\d+)*$/.test(fileName);
}

function patchLinuxAddonDir(dir) {
  if (!fs.existsSync(dir)) return [];
  return fs.readdirSync(dir, { withFileTypes: true })
    .filter((entry) => entry.isFile() && isLinuxNativeFile(entry.name))
    .map((entry) => patchRunpath(path.join(dir, entry.name)));
}

module.exports = {
  ORIGIN_RUNPATH,
  isLinuxNativeFile,
  patchLinuxAddonDir,
  patchRunpath,
  readRunpaths,
};

if (require.main === module) {
  const targets = process.argv.slice(2);
  for (const target of targets) {
    for (const result of patchLinuxAddonDir(target)) {
      console.log(`${result.changed ? "patched" : "ok     "} ${result.filePath}`);
    }
  }
}
