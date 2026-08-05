//! The text ramp had two steps that failed WCAG AA on the app's own background
//! — #4b5563 at 2.47:1 and #6b7280 at 3.86:1 — and they carried load-bearing
//! content: timestamps, the "?" hints, provider field labels, the hotkey
//! display. They merged into one step that passes. This test recomputes the
//! ratios from the stylesheet, so a new faint grey cannot creep back in.
import { readFileSync } from "node:fs";
import { describe, it, expect } from "vitest";

const css = readFileSync(new URL("./styles.css", import.meta.url), "utf8");

/** WCAG relative luminance. */
function luminance(hex) {
  const [r, g, b] = [1, 3, 5].map((i) => parseInt(hex.slice(i, i + 2), 16) / 255);
  const lin = [r, g, b].map((c) => (c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4));
  return 0.2126 * lin[0] + 0.7152 * lin[1] + 0.0722 * lin[2];
}

function contrast(fg, bg) {
  const [a, b] = [luminance(fg), luminance(bg)].sort((x, y) => y - x);
  return (a + 0.05) / (b + 0.05);
}

// Every surface a text colour can land on, darkest first.
const SURFACES = ["#0a0e12", "#0b0e13", "#0f1318", "#0f141b", "#1a1f28"];

describe("the text ramp", () => {
  it("has no colour left that fails AA on the surfaces it is used on", () => {
    // Greys only: the accents (green, amber, red, magenta) are checked by eye
    // against their own washes and carry state, not prose.
    const greys = new Set(
      (css.match(/color:\s*(#[0-9a-f]{6})/gi) || [])
        .map((m) => m.split(/\s+/).pop().toLowerCase())
        .filter((c) => {
          const [r, g, b] = [1, 3, 5].map((i) => parseInt(c.slice(i, i + 2), 16));
          return Math.max(r, g, b) - Math.min(r, g, b) < 30; // near-neutral
        })
    );
    expect(greys.size).toBeGreaterThan(0);
    const failing = [...greys].filter((c) =>
      SURFACES.some((bg) => contrast(c, bg) < 4.5 && contrast(c, bg) > 1.2)
    );
    expect(failing, `these greys fail AA: ${failing.join(", ")}`).toEqual([]);
  });

  it("keeps functional text at or above the 11px floor", () => {
    const rem = 16; // the page never overrides the root font size
    const tooSmall = (css.match(/font-size:\s*([\d.]+)rem/g) || [])
      .map((m) => parseFloat(m.match(/([\d.]+)rem/)[1]))
      .filter((size) => size * rem < 11);
    expect([...new Set(tooSmall)], "sub-11px type is back").toEqual([]);
  });
});
