//! Guard for the one failure the user actually loses something to: a dictation
//! that was transcribed but never typed into the app. It happens while the
//! window is closed, so the evidence has to survive until the user looks — a red
//! dot on the row, a note above the panels, and a badge on the menu-bar icon.
//! The dot is the fragile part: the meta row it lives in is hidden whenever
//! post-processing is off, which would swallow the signal for anyone who never
//! turned the LLM editor on.
import { readFileSync } from "node:fs";
import { describe, it, expect } from "vitest";

const css = readFileSync(new URL("./styles.css", import.meta.url), "utf8");
const js = readFileSync(new URL("./main.js", import.meta.url), "utf8");
const html = readFileSync(new URL("./index.html", import.meta.url), "utf8");

describe("a dictation that never reached the app", () => {
  it("keeps its dot when the post-processing dots are off", () => {
    const gate = css.match(/#log-entries:not\(\.show-llm-dots\)([^{]*)\{[^}]*display:\s*none/);
    expect(gate, "the .log-meta gate rule is gone or was rewritten").not.toBeNull();
    expect(gate[1]).toMatch(/\.log-entry:not\(\.failed\)/);
  });

  it("draws that dot in the failure colour, not a status colour", () => {
    expect(css).toMatch(/\.log-llm-dot\.failed::before\s*\{\s*background:\s*#f87171/);
  });

  it("shows a note that wraps instead of ellipsizing", () => {
    // The message names a System Settings path the user has to walk; a nowrap
    // field cut it in half in every earlier version.
    expect(html).toMatch(/id="alert"/);
    const alert = css.slice(css.indexOf("#alert {"), css.indexOf("}", css.indexOf("#alert {")));
    expect(alert).toMatch(/color:\s*#f87171/);
    expect(alert).not.toMatch(/nowrap/);
  });

  it("does not lowercase the message it asks the user to follow", () => {
    // "System Settings → Privacy & Security → Accessibility" is a literal path.
    const alertCall = js.slice(js.indexOf("function showAlert"), js.indexOf("function hideAlert"));
    expect(alertCall).not.toMatch(/toLowerCase/);
  });

  it("clears the menu-bar badge when the note is dismissed", () => {
    expect(js).toMatch(/function hideAlert[\s\S]{0,200}invoke\("dismiss_alert"\)/);
  });
});
