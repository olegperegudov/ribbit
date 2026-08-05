import { readFileSync } from "node:fs";
import { describe, it, expect, vi } from "vitest";
import { armConfirm, CONFIRM_WINDOW_MS } from "./confirm.js";

// A button, reduced to the three things armConfirm actually touches — so the
// behaviour is testable without a browser.
function fakeButton(label = "×") {
  const classes = new Set();
  return {
    textContent: label,
    dataset: {},
    classList: {
      add: (c) => classes.add(c),
      remove: (c) => classes.delete(c),
      contains: (c) => classes.has(c),
    },
  };
}

describe("two-step confirmation", () => {
  it("refuses the first click and asks instead", () => {
    const btn = fakeButton();
    expect(armConfirm(btn, "remove word?")).toBe(false);
    expect(btn.textContent).toBe("remove word?");
    expect(btn.classList.contains("confirming")).toBe(true);
  });

  it("lets the second click through", () => {
    const btn = fakeButton();
    armConfirm(btn, "remove word?");
    expect(armConfirm(btn, "remove word?")).toBe(true);
  });

  it("answers no by itself after the window closes", () => {
    vi.useFakeTimers();
    const btn = fakeButton();
    armConfirm(btn, "remove word?");
    vi.advanceTimersByTime(CONFIRM_WINDOW_MS);
    expect(btn.textContent).toBe("×");
    expect(btn.classList.contains("confirming")).toBe(false);
    // And a click after that is a first click again, not a delete.
    expect(armConfirm(btn, "remove word?")).toBe(false);
    vi.useRealTimers();
  });

  it("does not undo a button that was already re-rendered or consumed", () => {
    vi.useFakeTimers();
    const btn = fakeButton();
    armConfirm(btn, "remove?");
    btn.dataset.armed = "";        // the list re-rendered under us
    btn.textContent = "something else";
    vi.advanceTimersByTime(CONFIRM_WINDOW_MS);
    expect(btn.textContent).toBe("something else");
    vi.useRealTimers();
  });
});

describe("the controls that use it", () => {
  const js = readFileSync(new URL("./main.js", import.meta.url), "utf8");

  it("guards the delete that takes a word and every alias under it", () => {
    const handler = js.slice(js.indexOf("} else if (rowBtn) {"), js.indexOf("remove_vocab_entry"));
    expect(handler).toMatch(/armConfirm\(rowBtn/);
  });

  it("guards the delete that takes a provider and its saved key", () => {
    const handler = js.slice(js.indexOf('const del = miniBtn'), js.indexOf("remove_provider"));
    expect(handler).toMatch(/armConfirm\(btn/);
  });

  it("leaves removing a single alias a one-click job", () => {
    // Cheap to re-add, and the chip × is the fast path for fixing a typo.
    const handler = js.slice(js.indexOf("if (aliasBtn) {"), js.indexOf("remove_vocab_alias"));
    expect(handler).not.toMatch(/armConfirm/);
  });
});
