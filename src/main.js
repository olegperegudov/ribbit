const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;
const { getCurrentWindow } = window.__TAURI__.window;

const $ = (sel) => document.querySelector(sel);

// Sound playback is handled natively by Rust (rodio) — no browser autoplay issues

// --- Utilities ---

function formatTime(isoString) {
  const d = new Date(isoString);
  return d.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
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
  const time = ts ? formatTime(ts) : new Date().toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
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
    navigator.clipboard.writeText(text).then(() => {
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

// Show one panel at a time (log, settings, debug)
function showPanel(name) {
  $("#log-entries").style.display = name === "log" ? "" : "none";
  $("#settings-panel").style.display = name === "settings" ? "flex" : "none";
  $("#debug-panel").style.display = name === "debug" ? "flex" : "none";
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

  await listen("recording-status", (event) => {
    const icon = $("#status-icon");
    if (event.payload) {
      icon.className = "recording";
      micReady = false;
      $("#status-detail").textContent = "starting mic...";
    } else {
      icon.className = "idle";
      $("#status-detail").textContent = "";
    }
  });

  await listen("audio-level", (event) => {
    if (!micReady) {
      micReady = true;
      $("#status-detail").textContent = "listening...";
    }
  });

  await listen("transcribing", (event) => {
    if (event.payload) {
      $("#status-icon").className = "transcribing";
      $("#status-detail").textContent = "ribbiting...";
    } else {
      $("#status-icon").className = "idle";
      $("#status-detail").textContent = "";
    }
  });

  await listen("transcription", (event) => {
    const { text } = event.payload;
    addLogEntry(text);
  });

  await listen("error", (event) => {
    $("#status-detail").textContent = event.payload.toLowerCase();
    setTimeout(() => { $("#status-detail").textContent = ""; }, 5000);
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
});
