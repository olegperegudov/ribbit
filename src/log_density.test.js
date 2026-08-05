//! The log is meant to be a glance. Two things broke that: a one-minute
//! dictation rendered ~18 lines and buried everything else, and both blank
//! states (nothing dictated yet, search matched nothing) rendered as the same
//! empty region with no words in it.
import { readFileSync } from "node:fs";
import { describe, it, expect } from "vitest";

const css = readFileSync(new URL("./styles.css", import.meta.url), "utf8");
const js = readFileSync(new URL("./main.js", import.meta.url), "utf8");
const html = readFileSync(new URL("./index.html", import.meta.url), "utf8");

describe("a long dictation", () => {
  it("folds to the same number of lines the code measures against", () => {
    const cssLines = css.match(/-webkit-line-clamp:\s*(\d+)/);
    const jsLines = js.match(/LOG_CLAMP_LINES = (\d+)/);
    expect(cssLines, "the clamp rule is gone").not.toBeNull();
    expect(jsLines, "the measured line budget is gone").not.toBeNull();
    expect(cssLines[1]).toBe(jsLines[1]);
  });

  it("is measured against the line height, not against overflow", () => {
    // An unclamped element never overflows, so the earlier "is it cut off?"
    // check answered no for every entry and the control never appeared.
    const fn = js.slice(js.indexOf("function markClamped"), js.indexOf("function markAllClamped"));
    expect(fn).toMatch(/lineHeight \* LOG_CLAMP_LINES/);
  });

  it("still copies the whole text, not the visible part", () => {
    expect(js).toMatch(/entry\.querySelector\("\.log-text"\)\.textContent/);
  });
});

describe("an empty log", () => {
  it("has somewhere to put the words", () => {
    expect(html).toMatch(/id="log-empty"/);
  });

  it("tells the two blank states apart", () => {
    const fn = js.slice(js.indexOf("function refreshLogEmpty"), js.indexOf("function openSearch"));
    expect(fn).toMatch(/nothing yet/);
    expect(fn).toMatch(/no matches/);
  });
});

describe("the provider label", () => {
  it("carries no tooltip at render time", () => {
    // A hint baked into the markup fires on every row, repeating the words the
    // cursor is already on and covering the row below.
    const markup = js.slice(js.indexOf("const labelHtml"), js.indexOf("entry.innerHTML"));
    expect(markup).not.toMatch(/data-hint/);
  });

  it("offers one only when the text is actually cut off", () => {
    const fn = js.slice(js.indexOf("function markLabelClipped"), js.indexOf("function markEntryFailed"));
    expect(fn).toMatch(/scrollWidth > label\.clientWidth/);
    expect(fn).toMatch(/delete label\.dataset\.hint/);
  });
});

describe("the bundled type", () => {
  it("ships the face instead of hoping the system has it", () => {
    // Cyrillic fell back to a proportional face mid-sentence without this.
    expect(css).toMatch(/@font-face[\s\S]{0,200}JetBrainsMono-Regular\.woff2/);
    expect(css).toMatch(/@font-face[\s\S]{0,200}JetBrainsMono-Bold\.woff2/);
  });
});
