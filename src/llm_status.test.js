//! The yellow dot has to answer "why not?", not just "no". Before this, a
//! dictation that came back unedited looked the same whether the provider had
//! timed out, run out of free tier, or was never given a key — three different
//! next steps behind one identical dot.
import { describe, it, expect } from "vitest";
import { logMeta } from "./llm_status.js";

describe("an entry the editor didn't rephrase", () => {
  it("says why, in the label and on hover", () => {
    const m = logMeta({
      edited: false,
      llmError: "rate limit / free tier",
      llmHost: "api.groq.com",
      llmModel: "llama-3.3-70b-versatile",
    });
    expect(m.dotClass).toBe("unedited");
    expect(m.label).toBe("api.groq.com | rate limit / free tier");
    expect(m.hint).toBe("not rephrased — rate limit / free tier");
  });

  it("distinguishes a timeout from a spent quota", () => {
    const timedOut = logMeta({ edited: false, llmError: "timed out", llmHost: "routerai.ru" });
    const spent = logMeta({ edited: false, llmError: "rate limit / free tier", llmHost: "routerai.ru" });
    expect(timedOut.label).not.toBe(spent.label);
  });

  it("falls back to 'the editor was off' when there is no reason to show", () => {
    // Entries logged before reasons existed, and every dictation taken with the
    // toggle off, arrive here — an empty label would read as a bug.
    const m = logMeta({ edited: false });
    expect(m.label).toBe("the editor was off");
    expect(m.hint).toBe("not rephrased — the editor was off");
  });

  it("still names the reason when the provider is unknown", () => {
    const m = logMeta({ edited: false, llmError: "no key set" });
    expect(m.label).toBe("no key set");
  });
});

describe("the other two dots", () => {
  it("keeps the model name on a rephrased entry", () => {
    const m = logMeta({
      edited: true,
      llmHost: "routerai.ru",
      llmModel: "google/gemma-4-26b-a4b-it",
    });
    expect(m.dotClass).toBe("edited");
    expect(m.label).toBe("routerai.ru | google/gemma-4-26b-a4b-it");
    expect(m.hint).toBe("rephrased");
  });

  it("lets 'never typed into the app' outrank the edit status", () => {
    // The words are only in the log — that outranks whether they were polished.
    const m = logMeta({
      edited: false,
      llmError: "timed out",
      insertError: "macOS secure input is on",
    });
    expect(m.dotClass).toBe("failed");
    expect(m.hint).toContain("macOS secure input is on");
    expect(m.hint).toContain("click the line to copy it");
  });
});
