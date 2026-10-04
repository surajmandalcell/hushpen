import { describe, expect, it } from "vitest";
import { findView, views } from "./views";

describe("view registry", () => {
  it("lists the six views in sidebar order", () => {
    expect(views.map((view) => [view.id, view.label])).toEqual([
      ["home", "Home"],
      ["configuration", "Configuration"],
      ["sound", "Sound"],
      ["models", "Models library"],
      ["history", "History"],
      ["about", "About"],
    ]);
  });

  it("gives every view an icon, a sidebar tone, and a component", () => {
    for (const view of views) {
      expect(view.icon).toBeTruthy();
      expect(view.tone).toMatch(/^bg-\[#[0-9a-f]{6}\] text-white$/);
      expect(typeof view.component).toBe("function");
    }
  });

  it("keeps view ids unique", () => {
    expect(new Set(views.map((view) => view.id)).size).toBe(views.length);
  });

  it("finds a view by id and falls back to Home", () => {
    expect(findView("history").label).toBe("History");
    expect(findView("nope" as never).id).toBe("home");
  });
});
