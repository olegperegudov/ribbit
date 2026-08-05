//! Guards the settings panel's density. Two providers used to render ~226px of
//! url/model/key fields above the rows people actually open the panel for
//! (hotkey, vocabulary, history) — configuration touched once a year, given
//! permanent full height. Cards collapse now, and the rows are grouped.
import { readFileSync } from "node:fs";
import { describe, it, expect } from "vitest";

const css = readFileSync(new URL("./styles.css", import.meta.url), "utf8");
const js = readFileSync(new URL("./main.js", import.meta.url), "utf8");
const html = readFileSync(new URL("./index.html", import.meta.url), "utf8");

const block = (selector) => {
  const start = css.indexOf(selector + " {");
  if (start === -1) return "";
  return css.slice(start, css.indexOf("}", start));
};

describe("provider cards", () => {
  it("keep url, model and key folded away until asked", () => {
    expect(block(".provider-body")).toMatch(/display:\s*none/);
    expect(block(".provider-card.open .provider-body")).toMatch(/display:\s*flex/);
  });

  it("open themselves only when there is no key yet", () => {
    // A freshly added provider that stayed folded would look like nothing
    // happened; one that is already working has nothing to show.
    expect(js).toMatch(/entry\.has_key \? "provider-card" : "provider-card open"/);
  });

  it("say which service they are while folded", () => {
    expect(js).toMatch(/function providerHost/);
    expect(block(".provider-card.open .provider-summary")).toMatch(/display:\s*none/);
  });
});

describe("the settings panel", () => {
  it("is grouped instead of one flat list", () => {
    const heads = html.match(/class="settings-group-head"/g) || [];
    expect(heads.length).toBe(3);
  });

  it("keeps what is stored on this computer in one place", () => {
    const tail = html.slice(html.lastIndexOf('class="settings-group-head"'));
    expect(tail).toMatch(/history-days-input/);
    expect(tail).toMatch(/debug-btn/);
  });
});
