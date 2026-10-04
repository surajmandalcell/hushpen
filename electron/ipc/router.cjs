const registry = require("../../shared/ipc-channels.json");
const { AppError, fail, ok, toErrorShape } = require("../core/errors.cjs");
const { validate } = require("./validate.cjs");

function describeChannel(channel, kind) {
  const entry = registry.channels[channel];
  if (!entry || entry.kind !== kind) {
    throw new Error(`Channel ${channel} is not declared as ${kind} in shared/ipc-channels.json`);
  }
  return entry;
}

function isTrustedSender(ctx, event, roles) {
  const sender = event && event.sender;
  if (!sender || !event.senderFrame || event.senderFrame !== sender.mainFrame) return false;

  return roles.some((role) => {
    const win = role === "main-window" ? ctx.windows.main : role === "overlay" ? ctx.windows.overlay : undefined;
    return Boolean(win) && !win.isDestroyed() && win.webContents.id === sender.id;
  });
}

function logFailure(ctx, channel, error) {
  const shape = toErrorShape(error);
  const params = shape.params ? ` ${JSON.stringify(shape.params)}` : "";
  ctx.log.error("ipc", shape.code, `${channel}${params}${shape.detail ? `: ${shape.detail}` : ""}`);
}

/**
 * Registers IPC handlers that check the sender, validate the payload, and
 * reply with an envelope. Handlers never throw to the renderer: errors become
 * `{ ok: false, error: { code, params?, detail? } }` and are logged locally.
 */
function createRouter({ ctx, ipc = require("electron").ipcMain }) {
  function handle(channel, schema, handler) {
    const { roles } = describeChannel(channel, "invoke");

    ipc.handle(channel, async (event, payload) => {
      try {
        if (!isTrustedSender(ctx, event, roles)) {
          throw new AppError("FORBIDDEN_SENDER", undefined, "Sender is not a registered window for this channel.");
        }
        const input = validate(schema, payload);
        return ok(await handler(input, event));
      } catch (error) {
        logFailure(ctx, channel, error);
        return fail(error);
      }
    });
  }

  function on(channel, schema, handler) {
    const { roles } = describeChannel(channel, "send");

    ipc.on(channel, (event, payload) => {
      try {
        if (!isTrustedSender(ctx, event, roles)) {
          throw new AppError("FORBIDDEN_SENDER", undefined, "Sender is not a registered window for this channel.");
        }
        handler(validate(schema, payload), event);
      } catch (error) {
        logFailure(ctx, channel, error);
      }
    });
  }

  return { handle, on };
}

module.exports = { createRouter, isTrustedSender };
