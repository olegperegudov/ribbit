const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;
const { getCurrentWindow } = window.__TAURI__.window;

// Sounds — load real quack sample, play with Web Audio API for pitch control
const audioCtx = new (window.AudioContext || window.webkitAudioContext)();
let quackBuffer = null;

// Resume audio context on any user interaction (required by autoplay policy)
["click", "keydown", "pointerdown"].forEach(evt => {
  document.addEventListener(evt, () => {
    if (audioCtx.state === "suspended") audioCtx.resume();
  }, { once: true });
});

fetch("quack.ogg")
  .then(r => r.arrayBuffer())
  .then(buf => audioCtx.decodeAudioData(buf))
  .then(decoded => { quackBuffer = decoded; })
  .catch(e => console.error("Failed to load quack.ogg:", e));

function playQuack(rate = 1.0, volume = 0.8) {
  if (!quackBuffer) return;
  const resume = audioCtx.state === "suspended" ? audioCtx.resume() : Promise.resolve();
  resume.then(() => {
    const src = audioCtx.createBufferSource();
    const gain = audioCtx.createGain();
    src.buffer = quackBuffer;
    src.playbackRate.value = rate;
    gain.gain.value = volume;
    src.connect(gain);
    gain.connect(audioCtx.destination);
    src.start();
  });
}

function playStartQuack() { playQuack(1.15, 0.8); }
function playStopQuack() { playQuack(0.85, 0.6); }

const $ = (sel) => document.querySelector(sel);

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
      playStartQuack();
    } else {
      icon.className = "idle";
      $("#status-detail").textContent = "";
      playStopQuack();
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
    playQuack(1.3, 0.4);
  });

  await listen("error", (event) => {
    $("#status-detail").textContent = event.payload.toLowerCase();
    setTimeout(() => { $("#status-detail").textContent = ""; }, 5000);
  });

  // Window controls
  $("#win-min").addEventListener("click", () => getCurrentWindow().minimize());
  $("#win-close").addEventListener("click", () => getCurrentWindow().hide());

  // Settings panel (gear toggles it)
  $("#settings-btn").addEventListener("click", () => {
    const visible = $("#settings-panel").style.display !== "none";
    showPanel(visible ? "log" : "settings");
  });

  // Always on top toggle
  $("#always-on-top").addEventListener("change", async (e) => {
    await invoke("set_always_on_top", { value: e.target.checked });
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
