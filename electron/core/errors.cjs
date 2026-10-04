const { codes } = require("../../shared/error-codes.json");

const ERROR_CODES = Object.freeze(Object.keys(codes));
const DETAIL_LIMIT = 500;

class AppError extends Error {
  constructor(code, params, detail) {
    if (!Object.hasOwn(codes, code)) {
      throw new TypeError(`Unknown error code: ${code}`);
    }
    super(detail || code);
    this.name = "AppError";
    this.code = code;
    this.params = params;
    this.detail = detail;
  }
}

function toErrorShape(error) {
  if (error instanceof AppError) {
    const shape = { code: error.code };
    if (error.params && Object.keys(error.params).length > 0) shape.params = error.params;
    if (error.detail) shape.detail = String(error.detail).slice(0, DETAIL_LIMIT);
    return shape;
  }

  const detail = error instanceof Error ? error.message : typeof error === "string" ? error : "";
  const shape = { code: "INTERNAL" };
  if (detail) shape.detail = detail.slice(0, DETAIL_LIMIT);
  return shape;
}

function ok(value) {
  return { ok: true, value };
}

function fail(error) {
  return { ok: false, error: toErrorShape(error) };
}

module.exports = { AppError, ERROR_CODES, fail, ok, toErrorShape };
