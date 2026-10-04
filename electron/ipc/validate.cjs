const { AppError } = require("../core/errors.cjs");

// Messages name the failing path only, never the offending value, because
// validation details are written to the local log.
function reject(path, expectation) {
  throw new AppError("INVALID_ARGUMENT", { path }, `${path || "payload"} must be ${expectation}`);
}

const isPlainObject = (value) => Boolean(value) && typeof value === "object" && !Array.isArray(value);

const v = {
  none: () => (value, path) => {
    if (value !== undefined && value !== null) reject(path, "empty");
    return undefined;
  },
  any: () => (value) => value,
  boolean: () => (value, path) => {
    if (typeof value !== "boolean") reject(path, "a boolean");
    return value;
  },
  string: ({ min = 0, max = 4096 } = {}) => (value, path) => {
    if (typeof value !== "string" || value.length < min || value.length > max) {
      reject(path, `a string of ${min} to ${max} characters`);
    }
    return value;
  },
  oneOf: (allowed) => (value, path) => {
    if (!allowed.includes(value)) reject(path, `one of ${allowed.join(", ")}`);
    return value;
  },
  binary: ({ maxBytes = 512 * 1024 * 1024 } = {}) => (value, path) => {
    const length = ArrayBuffer.isView(value) || value instanceof ArrayBuffer ? value.byteLength : -1;
    if (length < 0 || length > maxBytes) reject(path, "binary data within the size limit");
    return value;
  },
  numberArray: ({ max = 1024 } = {}) => (value, path) => {
    if (!Array.isArray(value) || value.length > max || value.some((item) => typeof item !== "number")) {
      reject(path, "an array of numbers within the length limit");
    }
    return value;
  },
  optional: (inner) => (value, path) => (value === undefined ? undefined : inner(value, path)),
  object: (shape) => (value, path) => {
    if (!isPlainObject(value)) reject(path, "an object");
    for (const key of Object.keys(value)) {
      if (!Object.hasOwn(shape, key)) reject(path ? `${path}.${key}` : key, "a known field");
    }
    const output = {};
    for (const [key, validator] of Object.entries(shape)) {
      const next = validator(value[key], path ? `${path}.${key}` : key);
      if (next !== undefined) output[key] = next;
    }
    return output;
  },
};

function validate(schema, value) {
  return schema(value, "");
}

module.exports = { v, validate };
