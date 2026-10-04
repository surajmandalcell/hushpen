import { afterEach, describe, expect, it } from "vitest";
import { swallowStrayFileDrops } from "./dropGuard";

let stop: (() => void) | undefined;
afterEach(() => stop?.());

function dragEvent(type: string, types: string[]) {
  const event = new Event(type, { bubbles: true, cancelable: true }) as Event & { dataTransfer: unknown };
  event.dataTransfer = { types, dropEffect: "copy" };
  return event;
}

describe("stray file drops", () => {
  it("cancels a file drop so the page never navigates to the file", () => {
    stop = swallowStrayFileDrops();

    expect(document.body.dispatchEvent(dragEvent("drop", ["Files"]))).toBe(false);
  });

  it("cancels a file dragover so the drop target does not look accepting", () => {
    stop = swallowStrayFileDrops();
    const event = dragEvent("dragover", ["Files"]);

    expect(document.body.dispatchEvent(event)).toBe(false);
    expect((event.dataTransfer as { dropEffect: string }).dropEffect).toBe("none");
  });

  it("leaves text drags alone", () => {
    stop = swallowStrayFileDrops();

    expect(document.body.dispatchEvent(dragEvent("drop", ["text/plain"]))).toBe(true);
  });

  it("stops swallowing after it is removed", () => {
    swallowStrayFileDrops()();

    expect(document.body.dispatchEvent(dragEvent("drop", ["Files"]))).toBe(true);
  });

  it("lets a handler that accepts the drop run first", () => {
    stop = swallowStrayFileDrops();
    let accepted = false;
    const zone = document.createElement("div");
    zone.addEventListener("drop", (event) => {
      event.preventDefault();
      accepted = true;
    });
    document.body.append(zone);

    zone.dispatchEvent(dragEvent("drop", ["Files"]));

    expect(accepted).toBe(true);
    zone.remove();
  });
});
