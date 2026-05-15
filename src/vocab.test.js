import { describe, it, expect } from "vitest";
import { applyVocab, findBestMatch, levenshtein } from "./vocab.js";

// Vocab shape mirrors disk: { target: [aliases...] }. JS side should behave
// identically to Rust apply_with — if a test here passes but Rust fails on
// the same case, that divergence itself is a bug.

describe("applyVocab", () => {
  it("returns text unchanged with empty vocab", () => {
    expect(applyVocab("hello world", {})).toBe("hello world");
  });

  it("replaces a single-word alias", () => {
    expect(applyVocab("the def team", { dev: ["def"] })).toBe("the dev team");
  });

  it("replaces a Cyrillic alias", () => {
    expect(applyVocab("приветствую деф", { дев: ["деф"] })).toBe("приветствую дев");
  });

  it("preserves case pattern from the source word", () => {
    const v = { dev: ["def"] };
    expect(applyVocab("Def is here", v)).toBe("Dev is here");
    expect(applyVocab("DEF day", v)).toBe("DEV day");
    expect(applyVocab("def night", v)).toBe("dev night");
  });

  it("does not match inside another word (word boundaries)", () => {
    const v = { dev: ["def"] };
    expect(applyVocab("define and default", v)).toBe("define and default");
  });

  it("replaces a multi-word phrase", () => {
    const v = { "machine learning": ["mashine lerning"] };
    expect(applyVocab("we use mashine lerning", v)).toBe("we use machine learning");
  });

  it("prefers longer multi-word alias over shorter", () => {
    const v = { "OK Computer": ["okay computer"], OK: ["okay"] };
    expect(applyVocab("listen to okay computer", v)).toBe("listen to OK Computer");
  });

  it("returns input verbatim when no alias matches", () => {
    expect(applyVocab("nothing to replace", { dev: ["def"] })).toBe("nothing to replace");
  });

  it("supports multiple aliases for the same target", () => {
    const v = { dev: ["def", "deph", "дев"] };
    expect(applyVocab("def and deph", v)).toBe("dev and dev");
  });

  it("matches alias at start, middle, and end of text", () => {
    const v = { dev: ["def"] };
    expect(applyVocab("def", v)).toBe("dev");
    expect(applyVocab("def is here", v)).toBe("dev is here");
    expect(applyVocab("hello def", v)).toBe("hello dev");
  });

  it("matches case-insensitively while preserving original case", () => {
    const v = { dev: ["def"] };
    expect(applyVocab("DEF Def def", v)).toBe("DEV Dev dev");
  });
});

describe("findBestMatch", () => {
  it("returns null on empty vocab", () => {
    expect(findBestMatch("anything", {})).toBeNull();
  });

  it("finds exact alias match", () => {
    expect(findBestMatch("def", { dev: ["def"] })).toBe("dev");
  });

  it("finds close alias within distance budget", () => {
    expect(findBestMatch("deph", { dev: ["def"] })).toBe("dev");
  });

  it("returns null when nothing is reasonably close", () => {
    expect(findBestMatch("photosynthesis", { dev: ["def"] })).toBeNull();
  });

  it("matches against target name itself, not just aliases", () => {
    expect(findBestMatch("dev", { dev: ["def"] })).toBe("dev");
  });
});

describe("levenshtein", () => {
  it("is zero for identical strings", () => {
    expect(levenshtein("abc", "abc")).toBe(0);
  });

  it("counts single character differences", () => {
    expect(levenshtein("abc", "abd")).toBe(1);
    expect(levenshtein("abc", "ab")).toBe(1);
    expect(levenshtein("abc", "abcd")).toBe(1);
  });

  it("is case-insensitive", () => {
    expect(levenshtein("ABC", "abc")).toBe(0);
  });

  it("handles empty strings", () => {
    expect(levenshtein("", "abc")).toBe(3);
    expect(levenshtein("abc", "")).toBe(3);
    expect(levenshtein("", "")).toBe(0);
  });
});
