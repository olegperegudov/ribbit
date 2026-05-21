import { applyVocab, findBestMatch } from "./vocab.js";

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

// English ordinal suffix: 1st, 2nd, 3rd, 4th, 21st, ...
function ordinal(n) {
  const s = ["th", "st", "nd", "rd"];
  const v = n % 100;
  return s[(v - 20) % 10] || s[v] || s[0];
}

// Day-separator label, e.g. "tu, may 5th". Weekday + month + ordinal day —
// easier to place a transcript in the week than a bare date. null = today.
function formatDate(isoString) {
  const d = isoString ? new Date(isoString) : new Date();
  const wd = ["su", "mo", "tu", "we", "th", "fr", "sa"][d.getDay()];
  const mon = d.toLocaleDateString("en-US", { month: "short" }).toLowerCase();
  const day = d.getDate();
  return `${wd}, ${mon} ${day}${ordinal(day)}`;
}

function addLogEntry(text, ts, edited) {
  const log = $("#log-entries");
  const time = ts ? formatTime(ts) : new Date().toLocaleTimeString([], { hour: "2-digit", minute: "2-digit", hour12: false });
  const dateLabel = formatDate(ts);

  const lastDateSep = log.querySelector(".date-sep:first-child");
  if (!lastDateSep || lastDateSep.dataset.day !== dateLabel) {
    const sep = document.createElement("div");
    sep.className = "date-sep";
    sep.dataset.day = dateLabel;
    const label = document.createElement("span");
    label.className = "date-sep-label";
    label.textContent = dateLabel;
    sep.appendChild(label);
    log.insertBefore(sep, log.firstChild);
  }

  const entry = document.createElement("div");
  entry.className = "log-entry";
  const dotClass = edited === true ? "edited" : "unedited";
  const dotHint = edited === true ? "rephrased" : "not rephrased";
  entry.innerHTML = `<span class="log-time">${time}</span><span class="log-text">${escapeHtml(text)}</span><span class="log-llm-dot ${dotClass}" data-hint="${dotHint}" tabindex="0"></span>`;
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

  // A transcript arriving while search is active must obey the active filter.
  if (searchQuery.trim()) applySearchFilter();
}

function escapeHtml(text) {
  const div = document.createElement("div");
  div.textContent = text;
  return div.innerHTML;
}

// Show one panel at a time (log, settings, debug, vocab).
// We don't resize the window on panel switch anymore — settings now scrolls
// internally if it overflows, which avoids the previous bug where buttons
// near the bottom (e.g. "check update") were clipped out of view.
function showPanel(name) {
  $("#log-entries").style.display = name === "log" ? "" : "none";
  $("#settings-panel").style.display = name === "settings" ? "flex" : "none";
  $("#debug-panel").style.display = name === "debug" ? "flex" : "none";
  $("#vocab-panel").style.display = name === "vocab" ? "flex" : "none";
  // Leaving the log hides the search popup and drops the filter — the filtered
  // view is meaningless when the log isn't on screen.
  if (name !== "log") closeSearch();
}

// --- Quick search ---
// A transcript matches when any of its words *starts with* the query
// (prefix match, case-insensitive, Cyrillic-aware). Typing "маш" surfaces
// both "машина" and "Маша" — anything beginning with those letters.

let searchQuery = "";

function entryMatchesQuery(text, query) {
  if (!query) return true;
  const q = query.toLowerCase();
  const words = text.toLowerCase().match(/[\p{L}\p{N}]+/gu) || [];
  return words.some((w) => w.startsWith(q));
}

// Render the log text with the matched part of every matching word wrapped
// in a highlight span. The query matches a word by prefix (entryMatchesQuery),
// so searching "алр" marks the first three letters of "Алросы".
function highlightMatches(text, query) {
  const q = query.toLowerCase();
  let out = "";
  let last = 0;
  for (const m of text.matchAll(/[\p{L}\p{N}]+/gu)) {
    const word = m[0];
    if (!word.toLowerCase().startsWith(q)) continue;
    out += escapeHtml(text.slice(last, m.index));
    out += `<mark class="search-hit">${escapeHtml(word.slice(0, q.length))}</mark>`;
    out += escapeHtml(word.slice(q.length));
    last = m.index + word.length;
  }
  out += escapeHtml(text.slice(last));
  return out;
}

function applySearchFilter() {
  const log = $("#log-entries");
  const q = searchQuery.trim();
  for (const entry of log.querySelectorAll(".log-entry")) {
    const textEl = entry.querySelector(".log-text");
    const text = textEl?.textContent || "";
    entry.style.display = entryMatchesQuery(text, q) ? "" : "none";
    // Re-render the text: matched prefixes highlighted while searching,
    // plain (no markup) once the query is cleared.
    if (textEl) textEl.innerHTML = q ? highlightMatches(text, q) : escapeHtml(text);
  }
  // A day separator is only useful if it still heads a visible entry.
  for (const sep of log.querySelectorAll(".date-sep")) {
    let visible = false;
    for (let n = sep.nextElementSibling; n && !n.classList.contains("date-sep"); n = n.nextElementSibling) {
      if (n.classList.contains("log-entry") && n.style.display !== "none") { visible = true; break; }
    }
    sep.style.display = visible ? "" : "none";
  }
}

function openSearch() {
  const btn = $("#search-btn");
  const popup = $("#search-popup");
  popup.style.display = "block";
  // Anchor the popup's right edge under the magnifier so it never overflows.
  const r = btn.getBoundingClientRect();
  const w = popup.offsetWidth;
  let left = Math.min(r.right - w, window.innerWidth - w - 8);
  popup.style.left = `${Math.max(8, left)}px`;
  popup.style.top = `${r.bottom + 6}px`;
  $("#search-input").focus();
}

function closeSearch() {
  const popup = $("#search-popup");
  if (!popup) return;
  popup.style.display = "none";
  const input = $("#search-input");
  if (input) input.value = "";
  searchQuery = "";
  applySearchFilter();
}

// --- Vocab state (logic lives in ./vocab.js) ---

let vocabData = {}; // target → [aliases]

function dlog(msg) {
  try { invoke("js_debug_log", { msg: String(msg) }); } catch (_) {}
}

// Re-scan all visible log entries and apply vocab replacements
function rescanLogEntries() {
  const entries = document.querySelectorAll("#log-entries .log-entry");
  dlog(`rescan start: entries=${entries.length}, vocab_keys=${Object.keys(vocabData).join("|")}`);
  let changed = 0, skipped = 0, no_change = 0;
  for (const entry of entries) {
    const textEl = entry.querySelector(".log-text");
    if (!textEl) { skipped++; continue; }
    const original = textEl.textContent;
    const replaced = applyVocab(original, vocabData);
    if (replaced !== original) {
      dlog(`replace: ${JSON.stringify(original.slice(0, 100))} -> ${JSON.stringify(replaced.slice(0, 100))}`);
      textEl.textContent = replaced;
      changed++;
    } else {
      no_change++;
    }
  }
  dlog(`rescan done: changed=${changed} no_change=${no_change} skipped_no_textel=${skipped}`);
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
    // limit 0 = load everything inside the retention window (search needs all days).
    const history = await invoke("get_log_history", { limit: 0 });
    for (const entry of history.reverse()) {
      addLogEntry(entry.text, entry.ts, entry.edited);
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
    const { text, edited } = event.payload;
    addLogEntry(text, null, edited);
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

  // Quick search (magnifier toggles a small popup; filtering is live)
  $("#search-btn").addEventListener("click", () => {
    if ($("#search-popup").style.display === "none") openSearch();
    else closeSearch();
  });
  $("#search-input").addEventListener("input", (e) => {
    searchQuery = e.target.value;
    applySearchFilter();
  });
  $("#search-input").addEventListener("keydown", (e) => {
    if (e.key === "Escape") { e.preventDefault(); closeSearch(); }
  });

  // Always on top toggle
  $("#always-on-top").addEventListener("change", async (e) => {
    await invoke("set_always_on_top", { value: e.target.checked });
  });

  // Generic "API key" cell: input ↔ "✓ saved · change" chip swap.
  // Used for both the Groq audio key and the per-provider LLM key.
  function makeKeyCell(inputEl, statusEl, editEl) {
    function showInput() {
      inputEl.style.display = "";
      inputEl.value = "";
      statusEl.style.display = "none";
    }
    function showSaved(flash) {
      inputEl.style.display = "none";
      inputEl.value = "";
      statusEl.style.display = "";
      if (flash) {
        statusEl.classList.remove("flash");
        void statusEl.offsetWidth;
        statusEl.classList.add("flash");
      }
    }
    editEl.addEventListener("click", () => { showInput(); inputEl.focus(); });
    return { showInput, showSaved };
  }

  // Groq (audio) key — always visible, re-editable from settings.
  const groqCell = makeKeyCell($("#groq-key-input"), $("#groq-key-status"), $("#groq-key-edit"));
  if (config.has_groq_key === true) groqCell.showSaved(false); else groqCell.showInput();
  $("#groq-key-input").addEventListener("change", async (e) => {
    const key = e.target.value.trim();
    if (!key) return;
    try {
      await invoke("set_api_key", { key, provider: "groq" });
      config.has_groq_key = true;
      groqCell.showSaved(true);
    } catch (err) {
      console.error("save groq key failed:", err);
    }
  });

  // LLM post-processing: toggle reveals provider dropdown + key row.
  // Provider dropdown picks which LLM service edits transcripts. The key cell
  // below shows the saved-state for THAT provider — switching providers
  // re-renders the chip/input from the cached `llm_provider_keys` map.
  const postprocessToggle = $("#postprocess-toggle");
  const llmProviderRow = $("#llm-provider-row");
  const llmKeyRow = $("#llm-key-row");
  const llmKeyLabel = $("#llm-key-label");
  const llmProviderSelect = $("#llm-provider-select");
  const logEntries = $("#log-entries");
  const llmCell = makeKeyCell($("#llm-key-input"), $("#llm-key-status"), $("#llm-key-edit"));

  // Populate provider dropdown from backend (single source of truth).
  let providerLabels = {}; // name → label, for the key-row label
  try {
    const providers = await invoke("list_llm_providers");
    for (const p of providers) {
      const opt = document.createElement("option");
      opt.value = p.name;
      opt.textContent = p.label;
      llmProviderSelect.appendChild(opt);
      providerLabels[p.name] = p.label;
    }
  } catch (e) { console.error("list_llm_providers failed:", e); }

  const llmProviderKeys = config.llm_provider_keys || {};
  llmProviderSelect.value = config.llm_provider || "routerai";

  function refreshLlmKeyCell() {
    const prov = llmProviderSelect.value;
    llmKeyLabel.firstChild.textContent = `${providerLabels[prov] || prov} key `;
    if (llmProviderKeys[prov] === true) llmCell.showSaved(false); else llmCell.showInput();
  }

  function setLlmSectionVisible(enabled) {
    llmProviderRow.style.display = enabled ? "" : "none";
    llmKeyRow.style.display = enabled ? "" : "none";
    logEntries.classList.toggle("show-llm-dots", enabled);
    if (enabled) refreshLlmKeyCell();
  }

  postprocessToggle.checked = config.postprocess_enabled === true;
  setLlmSectionVisible(postprocessToggle.checked);

  postprocessToggle.addEventListener("change", async (e) => {
    const enabled = e.target.checked;
    setLlmSectionVisible(enabled);
    await invoke("set_postprocess_enabled", { enabled });
  });

  llmProviderSelect.addEventListener("change", async (e) => {
    const prov = e.target.value;
    try {
      await invoke("set_llm_provider", { provider: prov });
      refreshLlmKeyCell();
    } catch (err) { console.error("set_llm_provider failed:", err); }
  });

  $("#llm-key-input").addEventListener("change", async (e) => {
    const key = e.target.value.trim();
    if (!key) return;
    const prov = llmProviderSelect.value;
    try {
      await invoke("set_api_key", { key, provider: prov });
      llmProviderKeys[prov] = true;
      llmCell.showSaved(true);
    } catch (err) {
      console.error(`save ${prov} key failed:`, err);
    }
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
    if (e.metaKey) parts.push("cmd");
    if (e.ctrlKey) parts.push("ctrl");
    if (e.altKey) parts.push("alt");
    if (e.shiftKey) parts.push("shift");

    const key = e.key.toLowerCase();
    if (!["control", "alt", "shift", "meta"].includes(key)) {
      const keyMap = { " ": "space", "arrowup": "up", "arrowdown": "down",
        "arrowleft": "left", "arrowright": "right" };
      parts.push(keyMap[key] || key);
    }

    const hasModifier = parts.some(p => ["cmd", "ctrl", "alt", "shift"].includes(p));
    const hasKey = parts.some(p => !["cmd", "ctrl", "alt", "shift"].includes(p));
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

  // History retention — rolling window of days kept on disk and searchable.
  const historyInput = $("#history-days-input");
  historyInput.value = config.history_days ?? 7;
  historyInput.addEventListener("change", async () => {
    let v = parseInt(historyInput.value, 10);
    if (!Number.isFinite(v) || v < 1) v = 1;
    if (v > 365) v = 365;
    historyInput.value = v;
    await invoke("set_history_days", { days: v });
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

    // Multi-click can extend selection across entries; wrapping such a range
    // flattens the log via deleteContents+insertNode below. Require both
    // endpoints inside the same .log-text.
    const startTxt = (range.startContainer.nodeType === 3
      ? range.startContainer.parentElement
      : range.startContainer)?.closest(".log-text");
    const endTxt = (range.endContainer.nodeType === 3
      ? range.endContainer.parentElement
      : range.endContainer)?.closest(".log-text");
    if (!startTxt || startTxt !== endTxt) {
      sel.removeAllRanges();
      hidePopup();
      return;
    }

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
    const suggestion = findBestMatch(text, vocabData);
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
      const alias_cp = [...popupSelectedWord].map(c => c.codePointAt(0).toString(16)).join(",");
      dlog(`add_vocab_entry call: target=${JSON.stringify(target)} alias=${JSON.stringify(popupSelectedWord)} alias_codepoints=${alias_cp}`);
      vocabData = await invoke("add_vocab_entry", { target, alias: popupSelectedWord });
      dlog(`add_vocab_entry OK, vocab_keys=${Object.keys(vocabData).join("|")}`);
      rescanLogEntries();
    } catch (err) { dlog(`add_vocab_entry FAILED: ${err}`); }
    hidePopup();
  });

  // Hide popup on click outside
  document.addEventListener("mousedown", (e) => {
    if (popup.style.display !== "none" && !popup.contains(e.target)) {
      hidePopup();
    }
  });

  initTooltips();
});

// --- Tooltips ---
// Custom hover/focus/tap tooltip for [data-hint] elements. Replaces native
// title="" which is unreliable across webview engines (silent on WKWebView/macOS,
// slow on WebView2/Windows). Pure DOM, identical behaviour on both platforms.
function initTooltips() {
  const tip = document.getElementById("tooltip");
  if (!tip) return;

  let tapHideTimer = null;

  function show(el) {
    const text = el.getAttribute("data-hint");
    if (!text) return;
    tip.textContent = text;
    tip.classList.add("visible");
    tip.setAttribute("aria-hidden", "false");
    position(el);
  }

  function hide() {
    tip.classList.remove("visible");
    tip.setAttribute("aria-hidden", "true");
    clearTimeout(tapHideTimer);
    tapHideTimer = null;
  }

  // Position under the icon, centered. Clamp to viewport so it never clips off-screen.
  function position(el) {
    const r = el.getBoundingClientRect();
    const tipRect = tip.getBoundingClientRect();
    const margin = 6;
    const vw = window.innerWidth;
    const vh = window.innerHeight;

    let top = r.bottom + margin;
    let left = r.left + r.width / 2 - tipRect.width / 2;

    // Flip above if no room below.
    if (top + tipRect.height > vh - 4) {
      top = r.top - tipRect.height - margin;
    }
    // Clamp horizontally.
    left = Math.max(4, Math.min(left, vw - tipRect.width - 4));

    tip.style.top = `${top}px`;
    tip.style.left = `${left}px`;
  }

  // Pointer events cover mouse + touch + pen uniformly across both webviews.
  document.addEventListener("pointerover", (e) => {
    const el = e.target.closest("[data-hint]");
    if (el) show(el);
  });
  document.addEventListener("pointerout", (e) => {
    const el = e.target.closest("[data-hint]");
    if (!el) return;
    // Ignore moves between child nodes of the same hint icon.
    if (e.relatedTarget && el.contains(e.relatedTarget)) return;
    hide();
  });

  // Keyboard navigation (Tab to icon → show; Esc → hide).
  document.addEventListener("focusin", (e) => {
    const el = e.target.closest("[data-hint]");
    if (el) show(el);
  });
  document.addEventListener("focusout", (e) => {
    if (e.target.closest("[data-hint]")) hide();
  });
  document.addEventListener("keydown", (e) => {
    if (e.key === "Escape") hide();
  });

  // Tap on touch / trackpad click — toggle with auto-hide so it works without hover.
  document.addEventListener("click", (e) => {
    const el = e.target.closest("[data-hint]");
    if (!el) return;
    if (tip.classList.contains("visible") && tip.textContent === el.getAttribute("data-hint")) {
      hide();
    } else {
      show(el);
      clearTimeout(tapHideTimer);
      tapHideTimer = setTimeout(hide, 3500);
    }
  });

  // Reposition / hide when layout shifts.
  window.addEventListener("scroll", hide, true);
  window.addEventListener("resize", hide);
}
