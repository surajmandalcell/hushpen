import { EventEmitter } from "node:events";
import { createRequire } from "node:module";
import { describe, expect, it, vi } from "vitest";

const require = createRequire(import.meta.url);
const { guardWebContents, installNavigationGuard } = require("../../electron/windows/navigationGuard.cjs") as {
  guardWebContents: (contents: unknown, appUrl: string) => void;
  installNavigationGuard: (options: { app: EventEmitter; appUrl: string }) => void;
};

const appUrl = "file:///app/dist/index.html";

function fakeContents() {
  const contents = new EventEmitter() as EventEmitter & { setWindowOpenHandler: (handler: (details: { url: string }) => { action: string }) => void };
  let openHandler: ((details: { url: string }) => { action: string }) | undefined;
  contents.setWindowOpenHandler = (handler) => { openHandler = handler; };
  const fire = (name: string, url: string) => {
    const event = { preventDefault: vi.fn(), url };
    contents.emit(name, event, url);
    return event;
  };
  return { contents, fire, open: (url: string) => openHandler?.({ url }) };
}

describe("navigation guard", () => {
  it.each(["will-navigate", "will-frame-navigate", "will-redirect"])("blocks %s to another page", (name) => {
    const { contents, fire } = fakeContents();
    guardWebContents(contents, appUrl);

    expect(fire(name, "https://example.com/").preventDefault).toHaveBeenCalled();
    expect(fire(name, "file:///assets/fixtures/speech-short.wav").preventDefault).toHaveBeenCalled();
    expect(fire(name, "javascript:alert(1)").preventDefault).toHaveBeenCalled();
  });

  it("lets the app page reload itself", () => {
    const { contents, fire } = fakeContents();
    guardWebContents(contents, appUrl);

    expect(fire("will-navigate", appUrl).preventDefault).not.toHaveBeenCalled();
  });

  it("blocks every webview attach", () => {
    const { contents, fire } = fakeContents();
    guardWebContents(contents, appUrl);

    expect(fire("will-attach-webview", "https://example.com/").preventDefault).toHaveBeenCalled();
  });

  it("denies window.open for every URL", () => {
    const { contents, open } = fakeContents();
    guardWebContents(contents, appUrl);

    expect(open("https://example.com")).toEqual({ action: "deny" });
    expect(open(appUrl)).toEqual({ action: "deny" });
    expect(open("file:///etc/passwd")).toEqual({ action: "deny" });
  });

  it("guards each web contents created later, including windows opened by tests", () => {
    const app = new EventEmitter();
    installNavigationGuard({ app, appUrl });
    const { contents, fire } = fakeContents();

    app.emit("web-contents-created", {}, contents);

    expect(fire("will-navigate", "https://example.com/").preventDefault).toHaveBeenCalled();
  });
});
