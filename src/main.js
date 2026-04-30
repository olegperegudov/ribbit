const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;
const { getCurrentWindow } = window.__TAURI__.window;

const $ = (sel) => document.querySelector(sel);

// Sound playback is handled natively by Rust (rodio) — no browser autoplay issues

// --- Utilities ---

function formatTime(isoString) {
  const d = new Date(isoString);
  return d.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit", hour12: false });
}

function formatDate(isoString) {
  const d = new Date(isoString);
  const today = new Date();
  const yesterday = new Date(today);
  yesterday.setDate(yesterday.getDate() - 1);

  if (d.toDateString() === today.toDateString()) return "TODAY";
  if (d.toDateString() === yesterday.toDateString()) return "YESTERDAY";
  return d.toLocaleDateString([], { day: "numeric", month: "short", year: "numeric" }).toUpperCase();
}

function addLogEntry(text, ts) {
  const log = $("#log-entries");
  const time = ts ? formatTime(ts) : new Date().toLocaleTimeString([], { hour: "2-digit", minute: "2-digit", hour12: false });
  const dateLabel = ts ? formatDate(ts) : "TODAY";

  const lastDateSep = log.querySelector(".date-sep:first-child");
  if (!lastDateSep || lastDateSep.textContent !== dateLabel) {
    const sep = document.createElement("div");
    sep.className = "date-sep";
    sep.textContent = dateLabel;
    log.insertBefore(sep, log.firstChild);
  }

  const entry = document.createElement("div");
  entry.className = "log-entry";
  entry.innerHTML = `<span class="log-time">${time}</span><span class="log-text">${escapeHtml(text)}</span>`;
  entry.title = "click to copy";
  entry.addEventListener("click", () => {
    // Don't copy if user is selecting text (for vocab popup)
    const sel = window.getSelection();
    if (sel && sel.toString().trim()) return;
    const currentText = entry.querySelector(".log-text").textContent;
    navigator.clipboard.writeText(currentText).then(() => {
      entry.classList.add("copied");
      setTimeout(() => entry.classList.remove("copied"), 800);
    });
  });

  const firstSep = log.querySelector(".date-sep");
  if (firstSep && firstSep.nextSibling) {
    log.insertBefore(entry, firstSep.nextSibling);
  } else {
    log.appendChild(entry);
  }
}

function escapeHtml(text) {
  const div = document.createElement("div");
  div.textContent = text;
  return div.innerHTML;
}

// Show one panel at a time (log, settings, debug, vocab)
let savedWindowSize = null;

async function showPanel(name) {
  const win = getCurrentWindow();

  $("#log-entries").style.display = name === "log" ? "" : "none";
  $("#settings-panel").style.display = name === "settings" ? "flex" : "none";
  $("#debug-panel").style.display = name === "debug" ? "flex" : "none";
  $("#vocab-panel").style.display = name === "vocab" ? "flex" : "none";

  if (name === "settings") {
    // Save current size, then fit window to settings content
    const factor = await win.scaleFactor();
    const phys = await win.outerSize();
    savedWindowSize = { width: phys.width / factor, height: phys.height / factor };

    // Wait for layout, then measure
    await new Promise(r => setTimeout(r, 10));
    const needed = document.querySelector(".container").scrollHeight;
    const target = Math.max(needed + 2, 200);
    await win.setSize(new window.__TAURI__.dpi.LogicalSize(savedWindowSize.width, target));
  } else if (savedWindowSize) {
    // Restore previous size
    await win.setSize(new window.__TAURI__.dpi.LogicalSize(savedWindowSize.width, savedWindowSize.height));
    savedWindowSize = null;
  }
}

// --- Vocab helpers ---

let vocabData = {}; // target → [aliases]

function levenshtein(a, b) {
  a = a.toLowerCase(); b = b.toLowerCase();
  const m = a.length, n = b.length;
  const dp = Array.from({ length: m + 1 }, () => Array(n + 1).fill(0));
  for (let i = 0; i <= m; i++) dp[i][0] = i;
  for (let j = 0; j <= n; j++) dp[0][j] = j;
  for (let i = 1; i <= m; i++)
    for (let j = 1; j <= n; j++)
      dp[i][j] = a[i-1] === b[j-1] ? dp[i-1][j-1] : 1 + Math.min(dp[i-1][j], dp[i][j-1], dp[i-1][j-1]);
  return dp[m][n];
}

function findBestMatch(word) {
  // Find the closest existing target by checking against all aliases AND targets
  let best = null, bestDist = Infinity;
  const w = word.toLowerCase();
  for (const [target, aliases] of Object.entries(vocabData)) {
    // Check distance to target itself
    const dt = levenshtein(w, target);
    if (dt < bestDist) { bestDist = dt; best = target; }
    // Check distance to each alias
    for (const alias of aliases) {
      const da = levenshtein(w, alias);
      if (da < bestDist) { bestDist = da; best = target; }
    }
  }
  // Only suggest if reasonably close (distance <= half the word length)
  if (best && bestDist <= Math.max(2, Math.ceil(w.length / 2))) return best;
  return null;
}

// Apply vocab replacements to text (JS mirror of Rust vocab::apply)
function applyVocab(text) {
  const lookup = {};
  for (const [target, aliases] of Object.entries(vocabData)) {
    for (const alias of aliases) lookup[alias.toLowerCase()] = target;
  }
  if (Object.keys(lookup).length === 0) return text;

  function matchCase(source, target) {
    if (source === source.toUpperCase()) return target.toUpperCase();
    if (source[0] === source[0].toUpperCase()) return target[0].toUpperCase() + target.slice(1);
    return target;
  }

  // Phase 1: multi-word phrases (longest first)
  const multi = Object.entries(lookup).filter(([a]) => a.includes(" ")).sort((a, b) => b[0].length - a[0].length);
  let result = text;
  for (const [alias, target] of multi) {
    const re = new RegExp("(?<=^|[^\\w\u0400-\u04FF])" + alias.replace(/[.*+?^${}()|[\]\\]/g, "\\$&") + "(?=$|[^\\w\u0400-\u04FF])", "gi");
    result = result.replace(re, (m) => matchCase(m, target));
  }

  // Phase 2: single words
  result = result.replace(/[\w\u0400-\u04FF]+/g, (word) => {
    const target = lookup[word.toLowerCase()];
    if (!target) return word;
    return matchCase(word, target);
  });

  return result;
}

// Re-scan all visible log entries and apply vocab replacements
function rescanLogEntries() {
  for (const entry of document.querySelectorAll("#log-entries .log-entry")) {
    const textEl = entry.querySelector(".log-text");
    if (!textEl) continue;
    const original = textEl.textContent;
    const replaced = applyVocab(original);
    if (replaced !== original) {
      textEl.textContent = replaced;
    }
  }
}

function startEditingKey(span, oldKey) {
  const input = document.createElement("input");
  input.type = "text";
  input.value = oldKey;
  input.className = "vocab-key-edit";
  span.replaceWith(input);
  input.focus();
  input.select();

  let done = false;
  async function finish(save) {
    if (done) return;
    done = true;
    const newKey = input.value.trim();
    if (!save || !newKey || newKey === oldKey) {
      // Restore original
      const restored = document.createElement("span");
      restored.className = "vocab-target";
      restored.textContent = oldKey;
      restored.title = "click to rename";
      restored.style.cursor = "pointer";
      restored.addEventListener("click", () => startEditingKey(restored, oldKey));
      input.replaceWith(restored);
      return;
    }
    // Check for duplicate key
    const existingKey = Object.keys(vocabData).find(k => k.toLowerCase() === newKey.toLowerCase() && k !== oldKey);
    if (existingKey) {
      // Show merge popup above the input
      showMergePopup(input, oldKey, existingKey);
      return;
    }
    // Rename: move aliases from old key to new key
    const aliases = vocabData[oldKey] || [];
    delete vocabData[oldKey];
    vocabData[newKey] = aliases;
    try {
      await invoke("set_vocab", { vocabData: vocabData });
      rescanLogEntries();
    } catch (e) { console.error(e); }
    renderVocabList($("#vocab-search").value);
  }

  input.addEventListener("keydown", (e) => {
    if (e.key === "Enter") { e.preventDefault(); finish(true); }
    if (e.key === "Escape") { e.preventDefault(); finish(false); }
  });
  input.addEventListener("blur", () => setTimeout(() => finish(false), 150));
}

function showMergePopup(anchorEl, oldKey, existingKey) {
  // Remove any existing merge popup
  document.querySelector(".vocab-merge-popup")?.remove();

  const popup = document.createElement("div");
  popup.className = "vocab-merge-popup";
  popup.innerHTML = `similar key found, merge keys? : <button class="merge-yes">yes</button> | <button class="merge-no">no</button>`;

  // Position above the input
  const rect = anchorEl.getBoundingClientRect();
  popup.style.left = `${rect.left}px`;
  popup.style.top = `${rect.top - 28}px`;
  document.body.appendChild(popup);

  popup.querySelector(".merge-yes").addEventListener("click", async () => {
    popup.remove();
    // Merge: combine aliases from oldKey into existingKey
    const oldAliases = vocabData[oldKey] || [];
    const existing = vocabData[existingKey] || [];
    for (const alias of oldAliases) {
      if (!existing.some(a => a.toLowerCase() === alias.toLowerCase())) {
        existing.push(alias);
      }
    }
    vocabData[existingKey] = existing;
    delete vocabData[oldKey];
    try {
      await invoke("set_vocab", { vocabData: vocabData });
      rescanLogEntries();
    } catch (e) { console.error(e); }
    renderVocabList($("#vocab-search").value);
  });

  popup.querySelector(".merge-no").addEventListener("click", () => {
    popup.remove();
    renderVocabList($("#vocab-search").value);
  });

  // Auto-close after 5s
  setTimeout(() => { if (popup.parentNode) { popup.remove(); renderVocabList($("#vocab-search").value); } }, 5000);
}

function renderVocabList(filter = "") {
  const list = $("#vocab-list");
  list.innerHTML = "";
  const fl = filter.toLowerCase();

  // Sort: latin first, then cyrillic, alphabetical within each
  const keys = Object.keys(vocabData).sort((a, b) => {
    const aLat = /^[a-z]/i.test(a), bLat = /^[a-z]/i.test(b);
    if (aLat !== bLat) return aLat ? -1 : 1;
    return a.localeCompare(b);
  });

  for (const target of keys) {
    const aliases = vocabData[target];
    // Filter
    if (fl && !target.toLowerCase().includes(fl) && !aliases.some(a => a.toLowerCase().includes(fl))) continue;

    const row = document.createElement("div");
    row.className = "vocab-row";

    const targetSpan = document.createElement("span");
    targetSpan.className = "vocab-target";
    targetSpan.textContent = target;
    targetSpan.title = "click to rename";
    targetSpan.style.cursor = "pointer";
    targetSpan.addEventListener("click", () => startEditingKey(targetSpan, target));

    const aliasesSpan = document.createElement("span");
    aliasesSpan.className = "vocab-aliases";
    for (const alias of aliases) {
      const chip = document.createElement("span");
      chip.className = "vocab-alias-chip";
      chip.innerHTML = `${escapeHtml(alias)}<button class="vocab-alias-x" data-target="${escapeHtml(target)}" data-alias="${escapeHtml(alias)}">&times;</button>`;
      aliasesSpan.appendChild(chip);
    }

    const removeBtn = document.createElement("button");
    removeBtn.className = "vocab-row-x";
    removeBtn.textContent = "\u00d7";
    removeBtn.dataset.target = target;

    row.appendChild(targetSpan);
    row.appendChild(document.createTextNode(" \u2190 "));
    row.appendChild(aliasesSpan);
    row.appendChild(removeBtn);
    list.appendChild(row);
  }

  if (list.children.length === 0) {
    const empty = document.createElement("div");
    empty.className = "vocab-empty";
    empty.textContent = filter ? "no matches" : "no words yet \u2014 add from here or select text in the log";
    list.appendChild(empty);
  }
}

// --- Main ---

window.addEventListener("DOMContentLoaded", async () => {
  const config = await invoke("get_config");
  if (!config.has_api_key) {
    $("#setup").style.display = "block";
  }

  // Open Groq console link in default browser
  $("#groq-link")?.addEventListener("click", async (e) => {
    e.preventDefault();
    const { openUrl } = window.__TAURI__.opener;
    await openUrl("https://console.groq.com/keys");
  });

  // Load history
  try {
    const history = await invoke("get_log_history", { limit: 50 });
    for (const entry of history.reverse()) {
      addLogEntry(entry.text, entry.ts);
    }
  } catch (e) {
    console.error("Failed to load history:", e);
  }

  $("#key-form")?.addEventListener("submit", async (e) => {
    e.preventDefault();
    const key = $("#api-key-input").value.trim();
    if (key) {
      await invoke("set_api_key", { key });
      $("#setup").style.display = "none";
    }
  });

  let micReady = false;
  let meterHeight = 0;

  await listen("recording-status", (event) => {
    const icon = $("#status-icon");
    const meter = $("#audio-meter");
    const bar = $("#audio-meter-bar");
    if (event.payload) {
      icon.className = "recording";
      micReady = false;
      $("#status-detail").textContent = "starting mic...";
      meterHeight = 0;
      bar.style.height = "0%";
      meter.classList.add("active");
    } else {
      icon.className = "idle";
      $("#status-detail").textContent = "";
      meter.classList.remove("active");
    }
  });

  await listen("audio-level", (event) => {
    if (!micReady) {
      micReady = true;
      $("#status-detail").textContent = "listening...";
    }
    // RMS → dB → 0..100% via -60dB floor; peak-hold with decay so bar doesn't
    // flicker on inter-syllable nulls.
    const rms = Math.max(Number(event.payload) || 0, 1e-6);
    const db = 20 * Math.log10(rms);
    const target = Math.max(0, Math.min(100, ((db + 60) / 60) * 100));
    meterHeight = target > meterHeight ? target : meterHeight * 0.85;
    $("#audio-meter-bar").style.height = meterHeight.toFixed(1) + "%";
  });

  let hasError = false;

  await listen("transcribing", (event) => {
    if (event.payload) {
      hasError = false;
      $("#status-icon").className = "transcribing";
      $("#status-detail").textContent = "ribbiting...";
    } else {
      $("#status-icon").className = "idle";
      if (!hasError) {
        $("#status-detail").textContent = "";
      }
    }
  });

  await listen("transcription", (event) => {
    const { text } = event.payload;
    addLogEntry(text);
  });

  await listen("error", (event) => {
    hasError = true;
    $("#status-detail").textContent = event.payload.toLowerCase();
    setTimeout(() => { $("#status-detail").textContent = ""; hasError = false; }, 5000);
  });

  // Window controls: _ = minimize to taskbar, X = hide to tray
  $("#win-min").addEventListener("click", () => getCurrentWindow().minimize());
  $("#win-close").addEventListener("click", () => invoke("hide_to_tray"));

  // Settings panel (gear toggles it)
  $("#settings-btn").addEventListener("click", () => {
    const visible = $("#settings-panel").style.display !== "none";
    showPanel(visible ? "log" : "settings");
  });

  // Always on top toggle
  $("#always-on-top").addEventListener("change", async (e) => {
    await invoke("set_always_on_top", { value: e.target.checked });
  });

  // Shortcut customization
  const shortcutEl = $("#shortcut-display");
  const shortcutLabel = shortcutEl.closest(".setting-row").querySelector(".setting-label");
  let capturing = false;
  let capturedKeys = "";
  let savedShortcut = "";

  // Load current shortcut from backend
  try {
    savedShortcut = await invoke("get_shortcut");
    shortcutEl.textContent = savedShortcut;
  } catch (e) {
    console.error("Failed to load shortcut:", e);
  }

  shortcutEl.addEventListener("click", () => {
    if (capturing) return;
    capturing = true;
    capturedKeys = "";
    shortcutEl.textContent = "press shortcut...";
    shortcutEl.classList.add("capturing");
    shortcutLabel.textContent = "enter=save  esc=cancel";
  });

  document.addEventListener("keydown", async (e) => {
    if (!capturing) return;
    e.preventDefault();
    e.stopPropagation();

    if (e.key === "Escape") {
      capturing = false;
      shortcutEl.classList.remove("capturing");
      shortcutEl.textContent = savedShortcut;
      shortcutLabel.textContent = "hotkey";
      return;
    }

    if (e.key === "Enter") {
      capturing = false;
      shortcutEl.classList.remove("capturing");
      shortcutLabel.textContent = "hotkey";
      if (capturedKeys) {
        try {
          await invoke("set_shortcut", { shortcut: capturedKeys });
          savedShortcut = capturedKeys;
          shortcutEl.textContent = savedShortcut;
        } catch (err) {
          shortcutEl.textContent = savedShortcut;
          $("#status-detail").textContent = "invalid shortcut";
          setTimeout(() => { $("#status-detail").textContent = ""; }, 3000);
        }
      } else {
        shortcutEl.textContent = savedShortcut;
      }
      return;
    }

    const parts = [];
    if (e.ctrlKey) parts.push("ctrl");
    if (e.altKey) parts.push("alt");
    if (e.shiftKey) parts.push("shift");

    const key = e.key.toLowerCase();
    if (!["control", "alt", "shift", "meta"].includes(key)) {
      const keyMap = { " ": "space", "arrowup": "up", "arrowdown": "down",
        "arrowleft": "left", "arrowright": "right" };
      parts.push(keyMap[key] || key);
    }

    const hasModifier = parts.some(p => ["ctrl", "alt", "shift"].includes(p));
    const hasKey = parts.some(p => !["ctrl", "alt", "shift"].includes(p));
    if (hasModifier && hasKey) {
      capturedKeys = parts.join("+");
      shortcutEl.textContent = capturedKeys;
    }
  });

  // Language selector
  const langSelect = $("#lang-select");
  const langChips = $("#lang-chips");
  const langLabels = {};
  for (const opt of langSelect.options) {
    if (opt.value) langLabels[opt.value] = opt.textContent;
  }

  let selectedLangs = [];

  function renderLangChips() {
    langChips.innerHTML = "";
    for (const code of selectedLangs) {
      const chip = document.createElement("span");
      chip.className = "lang-chip";
      chip.innerHTML = `${escapeHtml(langLabels[code] || code)}<button class="lang-chip-x" data-lang="${code}">&times;</button>`;
      langChips.appendChild(chip);
    }
    // Hide already-selected languages from dropdown
    for (const opt of langSelect.options) {
      if (opt.value) opt.hidden = selectedLangs.includes(opt.value);
    }
  }

  try {
    selectedLangs = await invoke("get_languages");
    renderLangChips();
  } catch (e) {}

  langSelect.addEventListener("change", async () => {
    const code = langSelect.value;
    if (code && !selectedLangs.includes(code)) {
      selectedLangs.push(code);
      renderLangChips();
      await invoke("set_languages", { languages: selectedLangs });
    }
    langSelect.value = "";
  });

  langChips.addEventListener("click", async (e) => {
    const btn = e.target.closest(".lang-chip-x");
    if (!btn) return;
    const code = btn.dataset.lang;
    selectedLangs = selectedLangs.filter(l => l !== code);
    renderLangChips();
    await invoke("set_languages", { languages: selectedLangs });
  });

  // Sound pack selector
  const soundSelect = $("#sound-select");
  try {
    soundSelect.value = await invoke("get_sound_pack");
  } catch (e) {}
  soundSelect.addEventListener("change", async () => {
    await invoke("set_sound_pack", { pack: soundSelect.value });
    invoke("test_sound");
  });

  // Version display + changelog link
  try {
    const ver = await invoke("get_current_version");
    const versionEl = $("#version-label");
    versionEl.textContent = `v${ver}`;
    versionEl.addEventListener("click", async (e) => {
      e.preventDefault();
      const { openUrl } = window.__TAURI__.opener;
      await openUrl(`https://github.com/olegperegudov/ribbit/releases/tag/v${ver}`);
    });
  } catch (e) {}

  // Update check
  const updateBtn = $("#update-btn");
  updateBtn.addEventListener("click", async () => {
    updateBtn.textContent = "checking...";
    updateBtn.disabled = true;
    try {
      const result = await invoke("check_for_update");
      if (result.available) {
        updateBtn.textContent = `update to v${result.version}`;
        updateBtn.classList.add("update-available");
        updateBtn.disabled = false;
        // Re-bind: next click installs
        updateBtn.onclick = async () => {
          updateBtn.textContent = "downloading...";
          updateBtn.disabled = true;
          try {
            await invoke("install_update");
          } catch (e) {
            updateBtn.textContent = "update failed";
            setTimeout(() => {
              updateBtn.textContent = "check update";
              updateBtn.classList.remove("update-available");
              updateBtn.disabled = false;
              updateBtn.onclick = null; // restore original handler
            }, 3000);
          }
        };
      } else {
        updateBtn.textContent = "up to date";
        setTimeout(() => {
          updateBtn.textContent = "check update";
          updateBtn.disabled = false;
        }, 2000);
      }
    } catch (e) {
      updateBtn.textContent = "check failed";
      setTimeout(() => {
        updateBtn.textContent = "check update";
        updateBtn.disabled = false;
      }, 3000);
    }
  });

  // Update available indicator (from auto-check or manual check)
  await listen("update-available", (event) => {
    const ver = event.payload;
    updateBtn.textContent = `update to v${ver}`;
    updateBtn.classList.add("update-available");
    $("#settings-btn").classList.add("update-available");
    updateBtn.disabled = false;
    updateBtn.onclick = async () => {
      updateBtn.textContent = "downloading...";
      updateBtn.disabled = true;
      try {
        await invoke("install_update");
      } catch (e) {
        updateBtn.textContent = "update failed";
        setTimeout(() => {
          updateBtn.textContent = `update to v${ver}`;
          updateBtn.disabled = false;
        }, 3000);
      }
    };
  });

  await listen("update-progress", (event) => {
    const pct = event.payload;
    updateBtn.textContent = `downloading ${pct}%`;
  });

  // Debug log (from settings)
  $("#debug-btn").addEventListener("click", async () => {
    const log = await invoke("get_debug_log");
    $("#debug-content").textContent = log;
    showPanel("debug");
    $("#debug-content").scrollTop = $("#debug-content").scrollHeight;
  });
  $("#debug-close").addEventListener("click", () => showPanel("log"));

  // --- Vocab panel ---
  try { vocabData = await invoke("get_vocab"); } catch (e) {}

  $("#vocab-btn").addEventListener("click", () => {
    renderVocabList();
    showPanel("vocab");
  });
  $("#vocab-close").addEventListener("click", () => showPanel("log"));

  // Search filter
  $("#vocab-search").addEventListener("input", (e) => {
    renderVocabList(e.target.value);
  });

  // Add new entry from panel
  $("#vocab-add-btn").addEventListener("click", async () => {
    const target = $("#vocab-add-target").value.trim();
    const alias = $("#vocab-add-alias").value.trim();
    if (!target || !alias) return;
    try {
      vocabData = await invoke("add_vocab_entry", { target, alias });
      $("#vocab-add-target").value = "";
      $("#vocab-add-alias").value = "";
      renderVocabList($("#vocab-search").value);
    } catch (e) { console.error("vocab add failed:", e); }
  });

  // Enter to add in either input
  for (const id of ["#vocab-add-target", "#vocab-add-alias"]) {
    $(id).addEventListener("keydown", (e) => {
      if (e.key === "Enter") { e.preventDefault(); $("#vocab-add-btn").click(); }
    });
  }

  // Remove alias (x on chip) or remove whole entry (x on row)
  $("#vocab-list").addEventListener("click", async (e) => {
    const aliasBtn = e.target.closest(".vocab-alias-x");
    const rowBtn = e.target.closest(".vocab-row-x");
    if (aliasBtn) {
      const { target, alias } = aliasBtn.dataset;
      try {
        vocabData = await invoke("remove_vocab_alias", { target, alias });
        renderVocabList($("#vocab-search").value);
      } catch (err) { console.error(err); }
    } else if (rowBtn) {
      const target = rowBtn.dataset.target;
      try {
        vocabData = await invoke("remove_vocab_entry", { target });
        renderVocabList($("#vocab-search").value);
      } catch (err) { console.error(err); }
    }
  });

  // --- Selection popup for quick vocab add ---
  const popup = $("#vocab-popup");
  const popupInput = $("#vocab-popup-input");
  const popupSuggestion = $("#vocab-popup-suggestion");
  let popupSelectedWord = "";
  let popupHighlightedSpan = null;

  function hidePopup() {
    popup.style.display = "none";
    popupSelectedWord = "";
    popupInput.value = "";
    popupSuggestion.style.display = "none";
    // Remove fuchsia highlight
    if (popupHighlightedSpan) {
      const parent = popupHighlightedSpan.parentNode;
      if (parent) {
        parent.replaceChild(document.createTextNode(popupHighlightedSpan.textContent), popupHighlightedSpan);
        parent.normalize();
      }
      popupHighlightedSpan = null;
    }
  }

  // Show popup when text is selected in log entries
  document.addEventListener("mouseup", (e) => {
    // Only from log entries
    if (!e.target.closest("#log-entries")) { return; }

    const sel = window.getSelection();
    const text = sel?.toString().trim();
    if (!text) { hidePopup(); return; }

    popupSelectedWord = text;
    popupInput.value = "";

    // Wrap selected text in a highlight span so it stays visible after focus moves
    const range = sel.getRangeAt(0);
    const rect = range.getBoundingClientRect();

    // Remove previous highlight if any
    if (popupHighlightedSpan) {
      const p = popupHighlightedSpan.parentNode;
      if (p) {
        p.replaceChild(document.createTextNode(popupHighlightedSpan.textContent), popupHighlightedSpan);
        p.normalize();
      }
      popupHighlightedSpan = null;
    }

    const highlight = document.createElement("span");
    highlight.className = "vocab-selection-highlight";
    highlight.textContent = text;
    range.deleteContents();
    range.insertNode(highlight);
    popupHighlightedSpan = highlight;
    sel.removeAllRanges();

    popup.style.left = `${rect.left}px`;
    popup.style.top = `${rect.bottom + 4}px`;
    popup.style.display = "flex";

    // Find best match from existing vocab
    const suggestion = findBestMatch(text);
    if (suggestion) {
      popupSuggestion.textContent = suggestion;
      popupSuggestion.style.display = "block";
    } else {
      popupSuggestion.style.display = "none";
    }

    // Focus input after a tick (so mouseup doesn't steal focus)
    setTimeout(() => popupInput.focus(), 10);
  });

  // Click suggestion to add alias to existing entry
  popupSuggestion.addEventListener("click", async () => {
    if (!popupSelectedWord) return;
    const target = popupSuggestion.textContent;
    try {
      vocabData = await invoke("add_vocab_entry", { target, alias: popupSelectedWord });
      rescanLogEntries();
    } catch (e) { console.error(e); }
    hidePopup();
  });

  // Enter in popup input to create new entry or add to existing
  popupInput.addEventListener("keydown", async (e) => {
    if (e.key === "Escape") { hidePopup(); return; }
    if (e.key !== "Enter") return;
    e.preventDefault();
    const target = popupInput.value.trim();
    if (!target || !popupSelectedWord) return;
    try {
      vocabData = await invoke("add_vocab_entry", { target, alias: popupSelectedWord });
      rescanLogEntries();
    } catch (err) { console.error(err); }
    hidePopup();
  });

  // Hide popup on click outside
  document.addEventListener("mousedown", (e) => {
    if (popup.style.display !== "none" && !popup.contains(e.target)) {
      hidePopup();
    }
  });
});
