//! Guard for the window "skin" — the window is borderless and transparent, so the
//! page is what the user sees as the app. If the document can scroll or bounce, a
//! two-finger swipe drags that skin around and bares the desktop behind it (reads
//! as white gaps along the edges). Only reproduces on a real macOS build, so the
//! CSS that prevents it is pinned here.
import { readFileSync } from "node:fs";
import { describe, it, expect } from "vitest";

const css = readFileSync(new URL("./styles.css", import.meta.url), "utf8");

const block = (selector) => {
  const start = css.indexOf(selector + " {");
  if (start === -1) return "";
  return css.slice(start, css.indexOf("}", start));
};

describe("window chrome", () => {
  it("pins the root so the document never scrolls or bounces", () => {
    const root = block("html, body");
    expect(root).toMatch(/height:\s*100%/);
    expect(root).toMatch(/overflow:\s*hidden/);
    expect(root).toMatch(/overscroll-behavior:\s*none/);
    expect(root).toMatch(/background:\s*transparent/);
  });

  it("keeps every scroll region's overscroll to itself", () => {
    for (const selector of ["#log-entries", ".settings-content", "#debug-content", "#vocab-list"]) {
      expect(block(selector)).toMatch(/overscroll-behavior:\s*contain/);
    }
  });
});
