const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

// Audio feedback — short click sounds via Web Audio API
const audioCtx = new (window.AudioContext || window.webkitAudioContext)();

function playBeep(freq, durationMs) {
  const osc = audioCtx.createOscillator();
  const gain = audioCtx.createGain();
  osc.connect(gain);
  gain.connect(audioCtx.destination);
  osc.frequency.value = freq;
  osc.type = "sine";
  gain.gain.value = 0.15;
  gain.gain.exponentialRampToValueAtTime(0.001, audioCtx.currentTime + durationMs / 1000);
  osc.start();
  osc.stop(audioCtx.currentTime + durationMs / 1000);
}

function playStartSound() { playBeep(880, 120); } // high short beep
function playStopSound() { playBeep(440, 150); }  // lower beep

const $ = (sel) => document.querySelector(sel);

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

  // Check if we need a date separator
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

  // Insert after the date separator
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

window.addEventListener("DOMContentLoaded", async () => {
  // Check config
  const config = await invoke("get_config");
  if (!config.has_api_key) {
    $("#setup").style.display = "block";
    $("#status-detail").textContent = "API key required";
  } else {
    $("#status-detail").textContent = `Key: ${config.api_key_preview}`;
    setTimeout(() => {
      $("#status-detail").textContent = "Ready";
    }, 2000);
  }

  // Load history from log files
  try {
    const history = await invoke("get_log_history", { limit: 50 });
    // history is newest-first, but we want to add oldest-first so newest ends up on top
    for (const entry of history.reverse()) {
      addLogEntry(entry.text, entry.ts);
    }
  } catch (e) {
    console.error("Failed to load history:", e);
  }

  // API key form
  $("#key-form")?.addEventListener("submit", async (e) => {
    e.preventDefault();
    const key = $("#api-key-input").value.trim();
    if (key) {
      await invoke("set_api_key", { key });
      $("#setup").style.display = "none";
      $("#status-detail").textContent = "Key saved. Ready!";
    }
  });

  // Recording status
  await listen("recording-status", (event) => {
    const icon = $("#status-icon");
    if (event.payload) {
      icon.className = "recording";
      playStartSound();
    } else {
      icon.className = "idle";
      playStopSound();
    }
  });

  // Detailed status messages
  await listen("status-detail", (event) => {
    $("#status-detail").textContent = event.payload;
  });

  // Transcribing animation
  let ribbitInterval = null;
  await listen("transcribing", (event) => {
    if (event.payload) {
      $("#status-icon").className = "transcribing";
      let dots = 0;
      ribbitInterval = setInterval(() => {
        dots = (dots + 1) % 4;
        const base = $("#status-detail").textContent.split("...")[0].split("..")[0].split(".")[0];
        if (base.includes("Ribbit")) {
          $("#status-detail").textContent = "Ribbiting" + ".".repeat(dots + 1);
        }
      }, 400);
    } else {
      if (ribbitInterval) { clearInterval(ribbitInterval); ribbitInterval = null; }
      $("#status-icon").className = "idle";
    }
  });

  // Transcription result — add to visible log
  await listen("transcription", (event) => {
    addLogEntry(event.payload);
    playBeep(660, 80); // success chirp
  });

  // Errors — show prominently
  await listen("error", (event) => {
    $("#status-detail").textContent = event.payload;
    $("#status-detail").classList.add("error");
    setTimeout(() => {
      $("#status-detail").classList.remove("error");
      $("#status-detail").textContent = "Ready";
    }, 8000);
  });

  // Debug log panel
  $("#debug-btn").addEventListener("click", async () => {
    const panel = $("#debug-panel");
    if (panel.style.display === "none") {
      const log = await invoke("get_debug_log");
      $("#debug-content").textContent = log;
      panel.style.display = "flex";
      $("#log-entries").style.display = "none";
      // Scroll to bottom
      $("#debug-content").scrollTop = $("#debug-content").scrollHeight;
    } else {
      panel.style.display = "none";
      $("#log-entries").style.display = "";
    }
  });

  $("#debug-close").addEventListener("click", () => {
    $("#debug-panel").style.display = "none";
    $("#log-entries").style.display = "";
  });
});
