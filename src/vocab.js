// Pure vocab helpers — no DOM, no Tauri, no IO. Importable from main.js
// (browser webview) and from vitest (node + jsdom).
//
// Keep this file a faithful JS mirror of src-tauri/src/vocab.rs `apply_with`.
// When fixing a bug in one side, mirror the change here too.

export function levenshtein(a, b) {
  a = a.toLowerCase();
  b = b.toLowerCase();
  const m = a.length, n = b.length;
  const dp = Array.from({ length: m + 1 }, () => Array(n + 1).fill(0));
  for (let i = 0; i <= m; i++) dp[i][0] = i;
  for (let j = 0; j <= n; j++) dp[0][j] = j;
  for (let i = 1; i <= m; i++)
    for (let j = 1; j <= n; j++)
      dp[i][j] = a[i - 1] === b[j - 1]
        ? dp[i - 1][j - 1]
        : 1 + Math.min(dp[i - 1][j], dp[i][j - 1], dp[i - 1][j - 1]);
  return dp[m][n];
}

// Find the closest existing target by distance to its key OR any of its aliases.
// Returns null if nothing reasonably close (distance ≤ max(2, half word length)).
export function findBestMatch(word, vocabData) {
  let best = null, bestDist = Infinity;
  const w = word.toLowerCase();
  for (const [target, aliases] of Object.entries(vocabData)) {
    const dt = levenshtein(w, target);
    if (dt < bestDist) { bestDist = dt; best = target; }
    for (const alias of aliases) {
      const da = levenshtein(w, alias);
      if (da < bestDist) { bestDist = da; best = target; }
    }
  }
  if (best && bestDist <= Math.max(2, Math.ceil(w.length / 2))) return best;
  return null;
}

function matchCase(source, target) {
  if (source === source.toUpperCase()) return target.toUpperCase();
  if (source[0] === source[0].toUpperCase()) return target[0].toUpperCase() + target.slice(1);
  return target;
}

// Apply vocab replacements to text. Phase 1 handles any alias with a
// non-word character (space, dot, hyphen, slash, ...) — those need literal
// phrase matching because phase 2 tokenizes on word boundaries and would
// split them. Pure word-character aliases go to phase 2 for fast lookup.
// Mirrors src-tauri/src/vocab.rs `apply_with`.
export function applyVocab(text, vocabData) {
  const lookup = {};
  for (const [target, aliases] of Object.entries(vocabData)) {
    for (const alias of aliases) lookup[alias.toLowerCase()] = target;
  }
  if (Object.keys(lookup).length === 0) return text;

  // Phase 1: phrase-match anything with a non-word char, longest first
  const multi = Object.entries(lookup)
    .filter(([a]) => /[^\wЀ-ӿ]/.test(a))
    .sort((a, b) => b[0].length - a[0].length);
  let result = text;
  for (const [alias, target] of multi) {
    const escaped = alias.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
    const re = new RegExp(
      "(?<=^|[^\\wЀ-ӿ])" + escaped + "(?=$|[^\\wЀ-ӿ])",
      "gi",
    );
    result = result.replace(re, (m) => matchCase(m, target));
  }

  // Phase 2: single words
  result = result.replace(/[\wЀ-ӿ]+/g, (word) => {
    const target = lookup[word.toLowerCase()];
    if (!target) return word;
    return matchCase(word, target);
  });

  return result;
}
