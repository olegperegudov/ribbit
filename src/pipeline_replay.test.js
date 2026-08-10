import { describe, it, expect } from "vitest";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";
import { applyVocab } from "./vocab.js";

// Replay recorded provider responses through the app's deterministic passes.
// The canary (scripts/canary.mjs) watches the LIVE pipeline nightly; these
// fixtures pin the offline half — a vocab mapping that regresses here fails
// on push, before any release is built. Recorded responses live in
// test/fixtures/provider-responses/; add one whenever a real provider output
// surprises us.
const FIXTURES = join(
  dirname(fileURLToPath(import.meta.url)),
  "../test/fixtures/provider-responses",
);
const readFixture = (name) => JSON.parse(readFileSync(join(FIXTURES, name), "utf8"));

// The mapping every Ribbit user eventually teaches: the phonetic spelling
// whisper returns for the anglicism must land as the real term.
const VOCAB = {
  merge: ["мёрж", "мерж"],
  DevOps: ["девопс"],
};

describe("pipeline replay (recorded provider responses)", () => {
  it("STT output: 'мёрж' becomes 'merge'", () => {
    const stt = readFixture("groq-stt-mixed-merge.json");
    expect(applyVocab(stt.text, VOCAB)).toBe("надо сделать merge этой ветки");
  });

  it("LLM edit output: strict vocab pass still enforces 'merge'", () => {
    // The app runs vocab::apply over the LLM-edited text — the model is
    // forbidden from touching terms, the strict pass owns them. If the LLM
    // edit fixture changes (new recorded response), this pinning still holds.
    const llm = readFixture("groq-llm-edit-merge.json");
    const edited = llm.choices[0].message.content;
    expect(applyVocab(edited, VOCAB)).toBe("Надо сделать merge этой ветки.");
  });

  it("'девопс' maps to the canonical Latin spelling", () => {
    expect(applyVocab("привет это девопс инженер", VOCAB)).toBe(
      "привет это DevOps инженер",
    );
  });

  it("word boundaries hold: 'мержу' inside another word is untouched", () => {
    // The alias 'мерж' must not match inside longer words.
    expect(applyVocab("смержу к вечеру", VOCAB)).toBe("смержу к вечеру");
  });

  it("runaway fixture is shaped like a real answer, not an edit", () => {
    // The JS side cannot run the Rust guards, but the fixture itself must
    // stay an answer-shaped response — the Rust replay test
    // (postprocess.rs::replay_tests) asserts the guards reject it.
    const runaway = readFixture("groq-llm-runaway.json");
    const content = runaway.choices[0].message.content;
    expect(content.length).toBeGreaterThan(100);
    expect(content).toContain("git");
  });
});
